//! Integration test: ban list persists across coordinator restart (BLAME-05, BLAME-06).
//!
//! This test guards the Docker deployment fix: without a persistent volume mount
//! at `coordinator.ban_file_path`, the ban list silently evaporated on every
//! `docker compose restart coordinator`. The fix mounts `/app/data` as a named
//! volume and points `BLINDJOIN__COORDINATOR__BAN_FILE_PATH` at it.
//!
//! This test exercises the Rust-side contract that the Docker change relies on:
//!
//! 1. Write a ban entry through the public append API
//!    (`coordinator::round::blame::append_ban_entry`) — same call path used by
//!    `on_signing_timeout` in production (`coordinator/src/round/blame.rs:213`).
//! 2. Simulate a coordinator restart by constructing a *fresh* in-memory
//!    `BanList` and calling `load_unexpired_entries` against the same file —
//!    same call path used by `coordinator::run` on startup
//!    (`coordinator/src/run.rs:70`).
//! 3. Assert `BanList::is_banned` recognises the previously banned UTXO after
//!    the simulated restart.
//!
//! If a future refactor drops the persist-on-write or load-on-startup path,
//! this test fails — catching the regression before it ships.
//!
//! No bitcoind, no HTTP server, no Tor. Pure persistence-layer test.

use std::time::Duration;

use coordinator::round::blame::{
    append_ban_entry, hash_utxo_str, load_unexpired_entries, now_unix_secs, BanEntry, BanList,
};

/// End-to-end persistence: append → simulate restart → reload → assert ban survives.
#[test]
fn ban_list_persists_across_coordinator_restart() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let ban_file = tmp
        .path()
        .join("ban_list.jsonl")
        .to_str()
        .expect("temp path is utf-8")
        .to_owned();

    let utxo_str = "deadbeef00000000000000000000000000000000000000000000000000000000:0";
    let now = now_unix_secs();
    let ban_duration = Duration::from_secs(3600);

    // ----- Phase 1: simulate the live coordinator. Ban a UTXO and persist it.
    {
        let mut bl = BanList::new();
        bl.ban(utxo_str, now, ban_duration);
        assert!(
            bl.is_banned(utxo_str, now),
            "in-memory BanList must recognise the just-banned UTXO"
        );

        // Persist exactly the way `on_signing_timeout` does
        // (coordinator/src/round/blame.rs:213).
        let entry = BanEntry {
            banned_at: now,
            expires_at: now + ban_duration.as_secs(),
        };
        append_ban_entry(&ban_file, utxo_str, &entry).expect("append_ban_entry must succeed");
    }

    // ----- Phase 2: drop the in-memory state to simulate a coordinator restart.
    //       In the real bug, this is exactly what `docker compose restart
    //       coordinator` did before the volume mount fix landed.

    // ----- Phase 3: bootstrap a fresh BanList from the file — same path
    //       `coordinator::run` uses on startup (coordinator/src/run.rs:65-82).
    let mut reloaded = BanList::new();
    let loaded_entries =
        load_unexpired_entries(&ban_file, now).expect("load_unexpired_entries must succeed");
    assert_eq!(
        loaded_entries.len(),
        1,
        "exactly one unexpired ban entry must be loaded from disk"
    );
    for (utxo_hash, entry) in loaded_entries {
        reloaded.load_entry(utxo_hash, entry);
    }

    // ----- Phase 4: the previously banned UTXO must still be recognised.
    assert!(
        reloaded.is_banned(utxo_str, now),
        "BanList must recognise the UTXO as banned after restart (BLAME-05, BLAME-06)"
    );

    // Sanity-check: a never-banned UTXO is still not banned.
    let other = "feedface00000000000000000000000000000000000000000000000000000000:1";
    assert!(
        !reloaded.is_banned(other, now),
        "never-banned UTXO must not be reported as banned after reload"
    );
}

/// Expired entries on disk must NOT come back to life after restart (BLAME-06).
/// This guards the time-based filter in `load_unexpired_entries`.
#[test]
fn expired_bans_do_not_persist_across_restart() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let ban_file = tmp
        .path()
        .join("ban_list.jsonl")
        .to_str()
        .expect("temp path is utf-8")
        .to_owned();

    // Write an entry that expired at t=1000.
    let expired = BanEntry {
        banned_at: 500,
        expires_at: 1000,
    };
    append_ban_entry(&ban_file, "tx:0", &expired).expect("append must succeed");

    // Reload at t=5000 → entry is expired and must be filtered out.
    let mut reloaded = BanList::new();
    let loaded =
        load_unexpired_entries(&ban_file, 5000).expect("load_unexpired_entries must succeed");
    assert!(
        loaded.is_empty(),
        "expired ban entries must not be reloaded after restart"
    );
    for (hash, entry) in loaded {
        reloaded.load_entry(hash, entry);
    }
    assert!(
        !reloaded.is_banned("tx:0", 5000),
        "expired UTXO must not be reported banned"
    );
}

/// On a fresh coordinator (first startup), the ban file does not exist yet.
/// `load_unexpired_entries` must return Ok(empty) rather than erroring —
/// this is the documented behaviour in `coordinator/src/run.rs:65-82` which
/// otherwise emits a warning and starts with an empty ban list.
#[test]
fn missing_ban_file_yields_empty_ban_list() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let absent = tmp.path().join("never_created.jsonl");

    let loaded = load_unexpired_entries(absent.to_str().unwrap(), 1_000)
        .expect("missing file must NOT be an error");
    assert!(
        loaded.is_empty(),
        "missing ban file must surface as an empty entry list (first-startup case)"
    );
}

/// Defensive: confirm the on-disk hash matches what `BanList` keys by, so a
/// future change to either hashing or storage doesn't silently desynchronise
/// the persisted and in-memory key formats.
#[test]
fn persisted_hash_matches_in_memory_key_format() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let ban_file = tmp
        .path()
        .join("ban_list.jsonl")
        .to_str()
        .expect("temp path is utf-8")
        .to_owned();

    let utxo_str = "abc123:7";
    let entry = BanEntry {
        banned_at: 100,
        expires_at: 9_999_999,
    };
    append_ban_entry(&ban_file, utxo_str, &entry).expect("append must succeed");

    let loaded = load_unexpired_entries(&ban_file, 100).expect("load must succeed");
    assert_eq!(loaded.len(), 1);

    let (loaded_hash, _) = &loaded[0];
    assert_eq!(
        loaded_hash,
        &hash_utxo_str(utxo_str),
        "on-disk hash must equal hash_utxo_str(utxo_str) — used as BanList key"
    );
}
