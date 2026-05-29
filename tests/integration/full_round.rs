//! Integration test: 3-client CoinJoin round on regtest via corepc-node.
//!
//! This test requires `bitcoind` in PATH (or BITCOIND_EXE env var set).
//! It is automatically skipped if bitcoind is not available (graceful skip).
//!
//! Run: cargo test --test integration full_round -- --nocapture
//!
//! Threat model compliance:
//!   T-06-02: Integration tests use the same HTTP API as real clients —
//!            no test-only backdoors in coordinator code paths.
//!   T-06-03: Uses port 0 (OS assigns free port) to avoid conflicts.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

// Phase 9 plan 09-03 migration: shared regtest fixtures from
// `tests/integration/mod.rs`. Each bitcoind-dependent test below uses
// `require_bitcoind!()` for the graceful local-dev skip, then
// `crate::fund_regtest()` to bring up the daemon and fund 3 P2WPKH
// UTXOs, holding the returned `BitcoindGuard` for the test's full
// duration. This replaces the historical leak-the-Node pattern with
// deterministic RAII shutdown.
//
// Plan 10-01 D-06 promotion: `fund_regtest` + `FundedSetup` moved from
// file-private to `crate::*`. Callsites invoke `crate::fund_regtest`
// for grep-ability; the `use crate::{...}` line below names the same
// items so the dependency contract is explicit at the top of the file.
// `bootstrap_regtest_bitcoind`, `BitcoindGuard`, `FundedSetup`, and
// `RpcCreds` are no longer referenced unqualified in this file (the
// Plan 10-01 collapse to `crate::fund_regtest` removed their last
// unqualified uses), but stay in the import list so the file's
// dependency surface remains self-documenting alongside the call sites.
#[allow(unused_imports)]
use crate::{bootstrap_regtest_bitcoind, fund_regtest, require_bitcoind, BitcoindGuard, FundedSetup, RpcCreds};

// ---------------------------------------------------------------------------
// Helper: initialise a round state in InputReg with a fresh RSA key.
// ---------------------------------------------------------------------------
/// Bootstrap a fresh round in InputReg using the production code path —
/// the same `coordinator::round::manager::start_round` that `coordinator::run`
/// invokes at startup. No hand-rolled `RoundStateInner` — eliminates the
/// T-06-02 test-only backdoor that hid the v1.1 round-bootstrap regression.
fn build_input_reg_round_state() -> coordinator::round::state::RoundState {
    use coordinator::round::manager::start_round;
    use coordinator::round::state::RoundState;

    let mut state = RoundState::new_idle();
    start_round(&mut state).expect("start_round must succeed from Idle");
    state
}

// ---------------------------------------------------------------------------
// Helper: spawn coordinator server in-process and return its listen URL
// plus a TempDir guard that owns the ban-file's parent directory.
// ---------------------------------------------------------------------------
/// Phase 8 WR-06: returns `(url, tempdir_guard)`. The caller MUST bind the
/// guard to a local so the temp directory survives for the test's duration
/// — `cargo test`'s parallel mode previously raced multiple tests on a
/// hard-coded `ban_list.jsonl` path resolved against the test runner's cwd.
async fn spawn_coordinator(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
) -> (String, tempfile::TempDir) {
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig};

    // Bind to port 0 (OS assigns an ephemeral port). Keep the listener open —
    // pass it directly into axum::serve to avoid the TOCTOU race where the port
    // could be claimed by another process between drop() and re-bind.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let listen_addr = addr.to_string();

    // WR-06: per-test temp dir so parallel tests cannot race the ban file.
    let tmp = tempfile::tempdir().expect("create temp dir");
    let ban_file_path = tmp
        .path()
        .join("ban_list.jsonl")
        .to_string_lossy()
        .into_owned();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: rpc_url.clone(),
            bitcoin_rpc_user: rpc_user.clone(),
            bitcoin_rpc_pass: rpc_pass.clone(),
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000, // small denomination for test speed
            min_participants: 3,
            max_participants: 3,
            round_timeout_input_reg_secs: 30,
            round_timeout_output_reg_secs: 30,
            round_timeout_signing_secs: 15,
            blame_ban_duration_secs: 60,
            fee_rate_sat_per_vbyte: 1,
            listen_addr: listen_addr.clone(),
            ban_file_path,
            rate_limit_info_per_min: 60,
            rate_limit_writes_per_min: 30,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            tor_mode: false,
        },
        discovery: DiscoveryConfig::default(),
    });

    let rpc = Arc::new(BitcoinRpc::new(rpc_url, rpc_user, rpc_pass));
    // Start coordinator in InputReg so clients can register immediately
    let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
    let app = coordinator::api::build_router(round_state, rpc, cfg);

    // Use the already-bound listener directly — no drop/re-bind race.
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), tmp)
}

// ---------------------------------------------------------------------------
// Helper: wait for coordinator /info to return 200 OK.
// ---------------------------------------------------------------------------
async fn wait_for_coordinator(coordinator_url: &str) {
    let http_client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let ok = http_client
            .get(format!("{}/info", coordinator_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Coordinator did not start within 5 seconds"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ---------------------------------------------------------------------------
// Main integration test: full 3-client CoinJoin round on regtest.
// ---------------------------------------------------------------------------

/// Full 3-client CoinJoin round on regtest.
///
/// Steps:
/// 1. Skip if bitcoind unavailable (graceful skip — not a failure)
/// 2. Start regtest bitcoind via corepc-node
/// 3. Mine 101 blocks + fund 3 test P2WPKH addresses
/// 4. Mine 1 confirmation block
/// 5. Spawn coordinator in-process (InputReg phase, denomination=100_000)
/// 6. Run 3 concurrent client tasks (input→output→sign)
/// 7. Assert CoinJoin tx appears in bitcoind mempool
/// 8. Verify the transaction has 3 outputs of 100_000 sats
#[tokio::test]
async fn full_round_three_clients() {
    // ----- Step 1: skip gracefully if bitcoind missing (local-dev), panic in CI -----
    // require_bitcoind!() routes the skip path through `return` from this fn.
    // We forward the resolved exe to bootstrap_regtest_bitcoind so the fixture
    // does not re-resolve (WR-03: single source of truth per test invocation).
    let exe = require_bitcoind!();

    // ----- Steps 2-4: bring up regtest bitcoind + fund 3 UTXOs via the
    // shared fixture. The returned guard owns the Node and shuts it down
    // deterministically on drop (RPC `stop` + Node::Drop SIGKILL fallback).
    // It MUST stay in scope for the test's full duration. `crate::fund_regtest`
    // composes `bootstrap_regtest_bitcoind` + the wallet-agnostic vout
    // discovery (10-01 D-06 promotion — single locus for funded regtest setup).
    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    let _bitcoind_guard = bitcoind_guard;

    let denomination: u64 = 100_000;

    // ----- Step 5: spawn coordinator in-process -----
    // WR-06: keep `_tmp_dir` bound for the test's full duration so the
    // ban-file's parent directory is not cleaned up early.
    let (coordinator_url, _tmp_dir) = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    )
    .await;
    wait_for_coordinator(&coordinator_url).await;

    // ----- Step 6: run 3 concurrent client tasks -----
    let test_wifs = [
        "cPyRhf56BjNjMMmijQQvUeNG2VPkmxvBf6iYpygDu6DWR8UqkZGQ",
        "cQExMWoJTPmEFT131NAnkTKSGUb8JDV7wV6U7yx4SDzNMvrfNPLz",
        "cRh8UTgSFtzpWVSLZF5cQL2HN3awKze49MPiLurQ9KL4h71ah15F",
    ];

    let handles: Vec<_> = test_wifs
        .iter()
        .enumerate()
        .map(|(i, wif)| {
            let url = coordinator_url.clone();
            let wif = wif.to_string();
            let (utxo_str, utxo_value) = setup.utxos[i].clone();

            tokio::spawn(async move {
                use bitcoin::Network;
                use client::http::CoordinatorClient;
                use client::round;
                use client::wallet::ClientWallet;

                let wallet =
                    ClientWallet::from_wif(&wif, &utxo_str, utxo_value, Network::Regtest)
                        .expect("ClientWallet creation");
                let coordinator_client = CoordinatorClient::new(url);

                // Poll until INPUT_REG (coordinator already starts in input_reg)
                let info = coordinator_client
                    .poll_until_phase("input_reg", 100, Duration::from_secs(600))
                    .await
                    .expect("poll for input_reg");

                let reg = round::input::register_input(&coordinator_client, &wallet, &info)
                    .await
                    .expect("register_input");

                // Poll until OUTPUT_REG
                coordinator_client
                    .poll_until_phase("output_reg", 100, Duration::from_secs(600))
                    .await
                    .expect("poll for output_reg");

                round::output::register_output(&coordinator_client, &wallet, &reg, &info)
                    .await
                    .expect("register_output");

                // Poll until SIGNING
                coordinator_client
                    .poll_until_phase("signing", 100, Duration::from_secs(600))
                    .await
                    .expect("poll for signing");

                round::sign::verify_and_sign(&coordinator_client, &wallet, &reg, 100)
                    .await
                    .expect("verify_and_sign");
            })
        })
        .collect();

    // Wait for all 3 clients to complete
    for handle in handles {
        handle.await.expect("client task panicked");
    }

    // ----- Step 7: wait for broadcast, then check mempool -----
    // Coordinator broadcasts after all 3 signatures are collected.
    // WR-05 (Plan 10-02): poll-until-deadline replaces the previous bare
    // 2s sleep. Predicate: mempool contains at least one txid. Deadline:
    // 10s (5x original sleep budget) — sized per 10-RESEARCH.md Pitfall 4.
    // 100ms poll cadence matches round_bootstrap.rs:141. Each iteration
    // re-clones the rpc creds into the spawn_blocking closure because
    // spawn_blocking takes 'static + Send and the loop owns the originals.
    let mempool_txids: Vec<String> = {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let rpc_url_c = setup.rpc_url.clone();
            let rpc_user_c = setup.rpc_user.clone();
            let rpc_pass_c = setup.rpc_pass.clone();
            let txids: Vec<String> = tokio::task::spawn_blocking(move || {
                use corepc_node::client::client_sync::Auth;
                let auth = Auth::UserPass(rpc_user_c, rpc_pass_c);
                let client = corepc_node::Client::new_with_auth(&rpc_url_c, auth)
                    .expect("create rpc client for mempool check");
                // GetRawMempool(Vec<String>) — txids as hex strings
                client.get_raw_mempool().expect("get_raw_mempool").0
            })
            .await
            .expect("mempool check spawn_blocking");

            if !txids.is_empty() {
                break txids;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "CoinJoin tx never appeared in mempool within 10s after all 3 clients \
                     submitted signatures. The coordinator broadcasts in signing.rs \
                     assemble_and_broadcast after collecting all partial sigs. \
                     Last observation: mempool empty."
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    assert!(
        !mempool_txids.is_empty(),
        "CoinJoin tx must appear in mempool after all 3 clients submit signatures. \
         The coordinator broadcasts in signing.rs assemble_and_broadcast after collecting \
         all partial sigs. If this assertion fails, check that testmempoolaccept accepts \
         the transaction (Phase 1 broadcasts unsigned tx — see signing.rs)."
    );

    let coinjoin_txid_str = mempool_txids[0].clone();
    eprintln!("CoinJoin txid in mempool: {}", coinjoin_txid_str);

    // ----- Step 8: verify the transaction has 3 denomination outputs -----
    let rpc_url2 = setup.rpc_url.clone();
    let rpc_user2 = setup.rpc_user.clone();
    let rpc_pass2 = setup.rpc_pass.clone();
    let txid_for_verify = coinjoin_txid_str.clone();

    let denom_output_count: usize = tokio::task::spawn_blocking(move || {
        use std::str::FromStr;
        use corepc_node::client::client_sync::Auth;
        let auth = Auth::UserPass(rpc_user2, rpc_pass2);
        let client = corepc_node::Client::new_with_auth(&rpc_url2, auth)
            .expect("create rpc client for tx verify");
        let txid = bitcoin::Txid::from_str(&txid_for_verify).unwrap();
        // GetRawTransactionVerbose has field `outputs: Vec<RawTransactionOutput>`
        // RawTransactionOutput has `value: f64` (in BTC)
        let tx = client
            .get_raw_transaction_verbose(txid)
            .expect("get_raw_transaction_verbose");
        tx.outputs
            .iter()
            .filter(|out| {
                // value is BTC (f64); convert to sats
                let sats = (out.value * 100_000_000.0).round() as u64;
                sats == denomination
            })
            .count()
    })
    .await
    .expect("tx verify spawn_blocking");

    assert_eq!(
        denom_output_count, 3,
        "CoinJoin tx must have exactly 3 denomination outputs of {} sats; got {}",
        denomination, denom_output_count
    );

    eprintln!(
        "Integration test PASSED: CoinJoin round complete, txid={}, {} denomination outputs",
        coinjoin_txid_str, denom_output_count
    );
}

// ---------------------------------------------------------------------------
// Integration test: blame protocol — non-signer timeout.
// TEST-07: 3 clients register inputs+outputs; only 2 submit signatures;
// signing timeout fires; non-signer UTXO is banned.
// ---------------------------------------------------------------------------

/// Spawn a coordinator configured for the blame test:
/// - denomination=100_000, min_participants=2, max_participants=3
/// - signing timeout = 2s (fast for test)
/// - ban_duration = 3600s
///
/// Returns (coordinator_url, ban_list) where ban_list is shared with the
/// timeout task so the integration test can query ban status directly.
async fn spawn_coordinator_with_blame(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
) -> (String, Arc<tokio::sync::RwLock<coordinator::round::blame::BanList>>) {
    use coordinator::api::build_router_with_ban_list;
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig};
    use coordinator::round::blame::BanList;
    use std::sync::atomic::{AtomicU32, Ordering};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let listen_addr = addr.to_string();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: rpc_url.clone(),
            bitcoin_rpc_user: rpc_user.clone(),
            bitcoin_rpc_pass: rpc_pass.clone(),
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000,
            min_participants: 2,  // 2 of 3 can advance round
            max_participants: 3,
            round_timeout_input_reg_secs: 30,
            round_timeout_output_reg_secs: 30,
            round_timeout_signing_secs: 2, // fast timeout for test
            blame_ban_duration_secs: 3600,
            fee_rate_sat_per_vbyte: 1,
            listen_addr: listen_addr.clone(),
            ban_file_path: "/dev/null".into(), // no persistence in test
            rate_limit_info_per_min: 60,
            rate_limit_writes_per_min: 30,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            tor_mode: false,
        },
        discovery: DiscoveryConfig::default(),
    });

    let rpc = Arc::new(BitcoinRpc::new(rpc_url, rpc_user, rpc_pass));
    let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
    let ban_list: Arc<RwLock<BanList>> = Arc::new(RwLock::new(BanList::new()));
    let blame_round_count: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));

    // Spawn the signing timeout task (mirrors main.rs)
    {
        let round_clone = Arc::clone(&round_state);
        let ban_list_clone = Arc::clone(&ban_list);
        let blame_count_clone = Arc::clone(&blame_round_count);
        let signing_timeout = Duration::from_secs(cfg.coordinator.round_timeout_signing_secs);
        let ban_file = cfg.coordinator.ban_file_path.clone();
        let ban_duration = cfg.coordinator.blame_ban_duration_secs;

        tokio::spawn(async move {
            use coordinator::round::blame::{on_signing_timeout, BlameOutcome};
            use coordinator::round::state::Phase;

            tokio::time::sleep(signing_timeout).await;
            let mut round = round_clone.write().await;
            if round.phase != Phase::Signing {
                return;
            }
            let mut bl = ban_list_clone.write().await;
            let count = blame_count_clone.load(Ordering::Relaxed);
            let outcome = on_signing_timeout(&mut round, &mut bl, &ban_file, ban_duration, count);
            match outcome {
                BlameOutcome::FullAbort => { blame_count_clone.store(0, Ordering::Relaxed); }
                BlameOutcome::RestartWithout { .. } => { blame_count_clone.fetch_add(1, Ordering::Relaxed); }
            }
        });
    }

    let app = build_router_with_ban_list(
        Arc::clone(&round_state),
        rpc,
        Arc::clone(&cfg),
        Arc::clone(&ban_list),
    );

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), ban_list)
}

/// Integration test: blame protocol — TEST-07
///
/// 3 clients register inputs and outputs; only 2 submit signatures.
/// The coordinator's signing timeout (2s) fires.
/// After timeout, coordinator returns to Idle phase and the non-signer UTXO
/// receives HTTP 403 UTXO_BANNED on the next input registration attempt.
///
/// Skips gracefully if bitcoind is not available.
#[tokio::test]
async fn blame_non_signer_timeout() {
    // Skip gracefully if bitcoind missing (local-dev); panic in CI.
    let exe = require_bitcoind!();

    // Shared regtest bring-up + fund 3 UTXOs (10-01 D-06 promotion —
    // wallet-agnostic vout discovery via get_raw_transaction_verbose;
    // replaces the broken descriptor-wallet-incompatible
    // wallet-ownership scan; see tests/integration/mod.rs::fund_regtest
    // doc block for the v30 schema rationale).
    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    // Hold bitcoind_guard for the rest of the test — bitcoind must remain
    // alive while the coordinator drives RPC calls below.
    let _bitcoind_guard = bitcoind_guard;

    // ----- Spawn coordinator (min_participants=2, signing timeout=2s) -----
    let (coordinator_url, ban_list) = spawn_coordinator_with_blame(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;

    let test_wifs = [
        "cPyRhf56BjNjMMmijQQvUeNG2VPkmxvBf6iYpygDu6DWR8UqkZGQ",
        "cQExMWoJTPmEFT131NAnkTKSGUb8JDV7wV6U7yx4SDzNMvrfNPLz",
        "cRh8UTgSFtzpWVSLZF5cQL2HN3awKze49MPiLurQ9KL4h71ah15F",
    ];

    // ----- All 3 clients register inputs + outputs; only 2 sign -----
    // Client 0 is the designated non-signer: registers input+output, then stops.
    // Clients 1 and 2 complete all phases including signing.
    let non_signer_utxo = setup.utxos[0].0.clone();

    let client_handles: Vec<_> = test_wifs.iter().enumerate().map(|(i, wif)| {
        let url = coordinator_url.clone();
        let wif = wif.to_string();
        let (utxo_str, utxo_value) = setup.utxos[i].clone();
        let should_sign = i != 0; // client 0 is the non-signer

        tokio::spawn(async move {
            use bitcoin::Network;
            use client::http::CoordinatorClient;
            use client::round;
            use client::wallet::ClientWallet;

            let wallet = ClientWallet::from_wif(&wif, &utxo_str, utxo_value, Network::Regtest)
                .expect("ClientWallet creation");
            let coordinator_client = CoordinatorClient::new(url);

            let info = coordinator_client
                .poll_until_phase("input_reg", 100, Duration::from_secs(600))
                .await
                .expect("poll for input_reg");

            let reg = round::input::register_input(&coordinator_client, &wallet, &info)
                .await
                .expect("register_input");

            coordinator_client
                .poll_until_phase("output_reg", 100, Duration::from_secs(600))
                .await
                .expect("poll for output_reg");

            round::output::register_output(&coordinator_client, &wallet, &reg, &info)
                .await
                .expect("register_output");

            if should_sign {
                coordinator_client
                    .poll_until_phase("signing", 100, Duration::from_secs(600))
                    .await
                    .expect("poll for signing");

                // verify_and_sign may return Err if round times out before all sigs — acceptable
                let _ = round::sign::verify_and_sign(&coordinator_client, &wallet, &reg, 50)
                    .await;
            }
            // Client 0 stops here — does not sign, triggering blame on timeout
        })
    }).collect();

    for handle in client_handles {
        handle.await.expect("client task panicked");
    }

    // ----- Wait for signing timeout (2s) + blame to complete -----
    // WR-05 (Plan 10-02): poll-until-deadline replaces the previous bare
    // 4s sleep. Predicate: coordinator /info reports round_state=="idle"
    // AND ban_list contains non_signer_utxo. Deadline: 10s (2.5x original
    // sleep budget). 100ms poll cadence matches round_bootstrap.rs:141.
    // Final last-observation diagnostic captures both fields so a
    // timeout makes the failure mode obvious from the log alone.
    let http_client = reqwest::Client::new();
    let (info, banned) = {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut last_round_state: Option<String> = None;
        let mut last_banned: bool;
        loop {
            // Fetch /info. Transient connect/decode errors are ignored
            // here — the deadline check below converts persistent errors
            // into a clear panic. last_round_state retains the most
            // recent observation for diagnostics.
            let info_now: Option<shared::protocol::InfoResponse> = match http_client
                .get(format!("{}/info", coordinator_url))
                .send()
                .await
            {
                Ok(r) => r.json().await.ok(),
                Err(_) => None,
            };

            let now = coordinator::round::blame::now_unix_secs();
            let banned_now = {
                let bl = ban_list.read().await;
                bl.is_banned(&non_signer_utxo, now)
            };

            if let Some(ref i) = info_now {
                if i.round_state == "idle" && banned_now {
                    break (i.clone(), banned_now);
                }
                last_round_state = Some(i.round_state.clone());
            }
            last_banned = banned_now;

            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "After signing timeout, coordinator did not return to idle AND ban \
                     non-signer UTXO within 10s. Last /info round_state: {:?}, last \
                     ban_list.is_banned({}): {}",
                    last_round_state, non_signer_utxo, last_banned
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    assert_eq!(
        info.round_state, "idle",
        "After signing timeout + blame, coordinator must return to idle; got '{}'",
        info.round_state
    );

    // ----- Non-signer UTXO must be in ban list -----
    assert!(
        banned,
        "Non-signer UTXO must be banned in BanList after signing timeout; utxo={}",
        non_signer_utxo
    );

    eprintln!(
        "blame_non_signer_timeout PASSED: round returned to idle, non-signer banned (utxo={})",
        non_signer_utxo
    );
}

// ---------------------------------------------------------------------------
// Helper: spawn coordinator with blame and auto-restart into InputReg.
// Used exclusively by round_restart_and_completion_after_blame.
// ---------------------------------------------------------------------------
async fn spawn_coordinator_with_blame_and_restart(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
) -> (String, Arc<tokio::sync::RwLock<coordinator::round::blame::BanList>>) {
    use coordinator::api::build_router_with_ban_list;
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig};
    use coordinator::round::blame::BanList;
    use std::sync::atomic::{AtomicU32, Ordering};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let listen_addr = addr.to_string();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: rpc_url.clone(),
            bitcoin_rpc_user: rpc_user.clone(),
            bitcoin_rpc_pass: rpc_pass.clone(),
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000,
            min_participants: 2,
            max_participants: 3,
            round_timeout_input_reg_secs: 30,
            round_timeout_output_reg_secs: 30,
            round_timeout_signing_secs: 2, // fast for test
            blame_ban_duration_secs: 3600,
            fee_rate_sat_per_vbyte: 1,
            listen_addr: listen_addr.clone(),
            ban_file_path: "/dev/null".into(),
            // round_restart_and_completion_after_blame runs TWO rounds back-to-back
            // with 3 clients each polling at 100ms cadence = ~30 req/sec sustained.
            // GlobalKeyExtractor means one shared bucket across all clients, so
            // need substantial headroom to avoid 429 over the test's lifetime.
            rate_limit_info_per_min: 6000,
            rate_limit_writes_per_min: 300,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            tor_mode: false,
        },
        discovery: DiscoveryConfig::default(),
    });

    let rpc = Arc::new(BitcoinRpc::new(rpc_url, rpc_user, rpc_pass));
    let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
    let ban_list: Arc<RwLock<BanList>> = Arc::new(RwLock::new(BanList::new()));
    let blame_round_count: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));

    // Signing timeout task that bans non-signers, then restarts round to InputReg.
    // After restart, also arms an input_reg timer that advances to OutputReg once
    // min_participants is reached — the production monitor (coordinator::run::run)
    // does this in real deployments, but this helper builds a minimal coordinator
    // so we replicate just the bit Round 2 needs.
    {
        let round_clone = Arc::clone(&round_state);
        let ban_list_clone = Arc::clone(&ban_list);
        let blame_count_clone = Arc::clone(&blame_round_count);
        let signing_timeout = Duration::from_secs(cfg.coordinator.round_timeout_signing_secs);
        let input_reg_timeout = Duration::from_secs(cfg.coordinator.round_timeout_input_reg_secs);
        let min_participants = cfg.coordinator.min_participants;
        let ban_file = cfg.coordinator.ban_file_path.clone();
        let ban_duration = cfg.coordinator.blame_ban_duration_secs;

        tokio::spawn(async move {
            use coordinator::round::blame::{on_signing_timeout, BlameOutcome};
            use coordinator::round::state::Phase;

            tokio::time::sleep(signing_timeout).await;
            let mut round = round_clone.write().await;
            if round.phase != Phase::Signing {
                return;
            }
            let mut bl = ban_list_clone.write().await;
            let count = blame_count_clone.load(Ordering::Relaxed);
            let outcome = on_signing_timeout(&mut round, &mut bl, &ban_file, ban_duration, count);
            match outcome {
                BlameOutcome::FullAbort => {
                    blame_count_clone.store(0, Ordering::Relaxed);
                }
                BlameOutcome::RestartWithout { .. } => {
                    blame_count_clone.fetch_add(1, Ordering::Relaxed);
                    // Restart the round in InputReg so remaining clients can re-register
                    *round = build_input_reg_round_state();
                    let new_round_id = round.round_id;
                    drop(round);
                    drop(bl);

                    // Arm an input_reg timer for Round 2 — the minimal coordinator
                    // doesn't have the production monitor loop, so without this Round 2
                    // would hang forever when partial quorum (min < count < max) registers.
                    let round_c = Arc::clone(&round_clone);
                    tokio::spawn(async move {
                        tokio::time::sleep(input_reg_timeout).await;
                        let mut r = round_c.write().await;
                        if r.round_id != new_round_id || r.phase != Phase::InputReg {
                            return; // already advanced — no-op
                        }
                        if r.participant_count >= min_participants {
                            let _ = r.transition_to(Phase::OutputReg);
                        }
                    });
                }
            }
        });
    }

    let app = build_router_with_ban_list(
        Arc::clone(&round_state),
        rpc,
        Arc::clone(&cfg),
        Arc::clone(&ban_list),
    );

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), ban_list)
}

// ---------------------------------------------------------------------------
// Adversarial tests (TEST-11)
// ---------------------------------------------------------------------------

/// TEST-11 sub-scenario 1: Replay token rejected.
///
/// Client 0 successfully registers output (first call → 200).
/// Client 0 attempts to register output a SECOND time with the same unblinded token.
/// Assert: second POST /round/output returns a 4xx error.
///
/// Skips gracefully if bitcoind is not available.
#[tokio::test]
async fn adversarial_replay_token() {
    let exe = require_bitcoind!();

    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    // WR-06: keep `_tmp_dir` bound for the test's full duration. The
    // bitcoind_guard (RAII) keeps the daemon alive — drop = shutdown.
    let (coordinator_url, _tmp_dir) = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;
    let _bitcoind_guard = bitcoind_guard;

    let test_wifs = [
        "cPyRhf56BjNjMMmijQQvUeNG2VPkmxvBf6iYpygDu6DWR8UqkZGQ",
        "cQExMWoJTPmEFT131NAnkTKSGUb8JDV7wV6U7yx4SDzNMvrfNPLz",
        "cRh8UTgSFtzpWVSLZF5cQL2HN3awKze49MPiLurQ9KL4h71ah15F",
    ];

    // All 3 clients register inputs so round advances to output_reg
    let mut reg_states = Vec::new();
    let mut infos = Vec::new();
    for (i, wif) in test_wifs.iter().enumerate() {
        use bitcoin::Network;
        use client::http::CoordinatorClient;
        use client::round;
        use client::wallet::ClientWallet;

        let wallet = ClientWallet::from_wif(wif, &setup.utxos[i].0, setup.utxos[i].1, Network::Regtest)
            .expect("ClientWallet creation");
        let coordinator_client = CoordinatorClient::new(coordinator_url.clone());

        let info = coordinator_client
            .poll_until_phase("input_reg", 100, Duration::from_secs(600))
            .await
            .expect("poll for input_reg");

        let reg = round::input::register_input(&coordinator_client, &wallet, &info)
            .await
            .expect("register_input");

        reg_states.push(reg);
        infos.push(info);
    }

    // Wait for output_reg phase
    {
        use client::http::CoordinatorClient;
        let coordinator_client = CoordinatorClient::new(coordinator_url.clone());
        coordinator_client
            .poll_until_phase("output_reg", 100, Duration::from_secs(600))
            .await
            .expect("poll for output_reg");
    }

    // Client 0: register output via client library (succeeds)
    {
        use bitcoin::Network;
        use client::http::CoordinatorClient;
        use client::round;
        use client::wallet::ClientWallet;

        let wallet = ClientWallet::from_wif(
            test_wifs[0], &setup.utxos[0].0, setup.utxos[0].1, Network::Regtest
        ).expect("ClientWallet creation");
        let coordinator_client = CoordinatorClient::new(coordinator_url.clone());

        round::output::register_output(&coordinator_client, &wallet, &reg_states[0], &infos[0])
            .await
            .expect("first output registration must succeed");
    }

    // Client 0: attempt replay — send the same unblinded_token again via raw reqwest
    // The coordinator has already marked this token in redeemed_tokens, so it must reject
    {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use shared::protocol::OutputRegRequest;

        let reg = &reg_states[0];
        let unblinded_token_b64 = B64.encode(reg.message_bytes);
        let signature_b64 = B64.encode(reg.unblinded_sig_bytes());
        let msg_randomizer_b64 = reg.msg_randomizer.as_ref().map(|m| B64.encode(m.0));

        // Derive the output address (same wallet, same output script)
        use bitcoin::{Address, Network};
        let output_address = Address::from_script(
            &reg.output_script,
            Network::Regtest,
        ).map(|a| a.to_string())
         .unwrap_or_else(|_| "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080".to_string());

        let replay_req = OutputRegRequest {
            unblinded_token: unblinded_token_b64,
            signature: signature_b64,
            output_address,
            amount_sats: 100_000,
            msg_randomizer: msg_randomizer_b64,
        };

        let resp = reqwest::Client::new()
            .post(format!("{}/round/output", coordinator_url))
            .json(&replay_req)
            .send()
            .await
            .expect("HTTP request sent");

        assert!(
            resp.status().is_client_error(),
            "Replay token must be rejected with 4xx; got status {}",
            resp.status()
        );
        eprintln!("adversarial_replay_token PASSED: replay rejected with status {}", resp.status());
    }
}

/// TEST-11 sub-scenario 2: Invalid UTXO rejected.
///
/// Submit a fabricated (non-existent) UTXO outpoint to POST /round/input.
/// Coordinator calls bitcoind get_tx_out, gets None, returns 4xx.
///
/// Requires bitcoind (RPC validation). Skips gracefully if unavailable.
#[tokio::test]
async fn adversarial_invalid_utxo() {
    let exe = require_bitcoind!();

    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    // WR-06: keep `_tmp_dir` bound for the test's full duration. The
    // bitcoind_guard (RAII) keeps the daemon alive — drop = shutdown.
    let (coordinator_url, _tmp_dir) = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;
    let _bitcoind_guard = bitcoind_guard;

    // Send POST /round/input with a fabricated non-existent outpoint
    // We need a plausible-looking InputRegRequest with a fake txid:0
    let fake_utxo = "a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0:0";

    // We need a valid-looking blinded_token and ownership_proof (the RPC check should
    // fire before full signature verification, but we still need syntactically valid fields)
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let fake_blinded_token = B64.encode([0u8; 64]); // syntactically valid base64
    // Fake ownership proof: valid JSON array with one hex item
    let fake_ownership_proof = "[\"00\"]";
    let fake_change_addr = "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080";

    let req_body = shared::protocol::InputRegRequest {
        utxo_outpoint: fake_utxo.to_string(),
        ownership_proof: fake_ownership_proof.to_string(),
        blinded_token: fake_blinded_token,
        change_address: fake_change_addr.to_string(),
    };

    let resp = reqwest::Client::new()
        .post(format!("{}/round/input", coordinator_url))
        .json(&req_body)
        .send()
        .await
        .expect("HTTP request sent");

    assert!(
        resp.status().is_client_error(),
        "Non-existent UTXO must be rejected with 4xx; got status {}",
        resp.status()
    );
    eprintln!("adversarial_invalid_utxo PASSED: fake UTXO rejected with status {}", resp.status());
}

/// TEST-11 sub-scenario 3: Wrong denomination rejected.
///
/// 3 clients register inputs (round advances to output_reg).
/// Client 0 submits POST /round/output with amount_sats=50_000 (not denomination=100_000).
/// Assert: 4xx response (coordinator enforces denomination equality).
///
/// Requires bitcoind (input registration validates UTXOs). Skips gracefully if unavailable.
#[tokio::test]
async fn adversarial_wrong_denomination() {
    let exe = require_bitcoind!();

    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    // WR-06: keep `_tmp_dir` bound for the test's full duration. The
    // bitcoind_guard (RAII) keeps the daemon alive — drop = shutdown.
    let (coordinator_url, _tmp_dir) = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;
    let _bitcoind_guard = bitcoind_guard;

    let test_wifs = [
        "cPyRhf56BjNjMMmijQQvUeNG2VPkmxvBf6iYpygDu6DWR8UqkZGQ",
        "cQExMWoJTPmEFT131NAnkTKSGUb8JDV7wV6U7yx4SDzNMvrfNPLz",
        "cRh8UTgSFtzpWVSLZF5cQL2HN3awKze49MPiLurQ9KL4h71ah15F",
    ];

    // All 3 clients register inputs to advance to output_reg
    let mut reg_states = Vec::new();
    let mut infos = Vec::new();
    for (i, wif) in test_wifs.iter().enumerate() {
        use bitcoin::Network;
        use client::http::CoordinatorClient;
        use client::round;
        use client::wallet::ClientWallet;

        let wallet = ClientWallet::from_wif(wif, &setup.utxos[i].0, setup.utxos[i].1, Network::Regtest)
            .expect("ClientWallet creation");
        let coordinator_client = CoordinatorClient::new(coordinator_url.clone());

        let info = coordinator_client
            .poll_until_phase("input_reg", 100, Duration::from_secs(600))
            .await
            .expect("poll for input_reg");

        let reg = round::input::register_input(&coordinator_client, &wallet, &info)
            .await
            .expect("register_input");

        reg_states.push(reg);
        infos.push(info);
    }

    // Wait for output_reg phase
    {
        use client::http::CoordinatorClient;
        let coordinator_client = CoordinatorClient::new(coordinator_url.clone());
        coordinator_client
            .poll_until_phase("output_reg", 100, Duration::from_secs(600))
            .await
            .expect("poll for output_reg");
    }

    // Client 0 sends wrong denomination (50_000 instead of 100_000)
    {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use shared::protocol::OutputRegRequest;

        let reg = &reg_states[0];
        let unblinded_token_b64 = B64.encode(reg.message_bytes);
        let signature_b64 = B64.encode(reg.unblinded_sig_bytes());
        let msg_randomizer_b64 = reg.msg_randomizer.as_ref().map(|m| B64.encode(m.0));

        use bitcoin::{Address, Network};
        let output_address = Address::from_script(
            &reg.output_script,
            Network::Regtest,
        ).map(|a| a.to_string())
         .unwrap_or_else(|_| "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080".to_string());

        let wrong_denom_req = OutputRegRequest {
            unblinded_token: unblinded_token_b64,
            signature: signature_b64,
            output_address,
            amount_sats: 50_000, // wrong denomination
            msg_randomizer: msg_randomizer_b64,
        };

        let resp = reqwest::Client::new()
            .post(format!("{}/round/output", coordinator_url))
            .json(&wrong_denom_req)
            .send()
            .await
            .expect("HTTP request sent");

        assert!(
            resp.status().is_client_error(),
            "Wrong denomination must be rejected with 4xx; got status {}",
            resp.status()
        );
        eprintln!("adversarial_wrong_denomination PASSED: wrong denomination rejected with status {}", resp.status());
    }
}

/// TEST-11 sub-scenario 4: Tampered PSBT (fewer denomination outputs) rejected.
///
/// Pure in-memory test — does NOT require bitcoind.
/// Calls check_psbt_denomination_outputs directly with a PSBT that has
/// fewer denomination outputs than participants_registered.
/// Assert: returns Err containing "output censorship".
#[tokio::test]
async fn adversarial_tampered_psbt_rejected() {
    use bitcoin::{
        absolute::LockTime, transaction::Version, Amount, OutPoint, ScriptBuf,
        Transaction, TxIn, TxOut,
    };
    use bitcoin::psbt::Psbt;
    use client::round::sign::check_psbt_denomination_outputs;
    use client::round::InputRegState;

    // Build a minimal InputRegState with 3 participants and denomination=100_000
    let participants_registered: u32 = 3;
    let denomination_sats: u64 = 100_000;

    // Build a PSBT with only 2 denomination outputs (not 3)
    let outputs: Vec<TxOut> = vec![
        TxOut { value: Amount::from_sat(denomination_sats), script_pubkey: ScriptBuf::new() },
        TxOut { value: Amount::from_sat(denomination_sats), script_pubkey: ScriptBuf::new() },
        // Third output has WRONG amount (simulating tampering)
        TxOut { value: Amount::from_sat(denomination_sats - 1000), script_pubkey: ScriptBuf::new() },
    ];
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn { previous_output: OutPoint::null(), ..Default::default() }],
        output: outputs,
    };
    let tampered_psbt = Psbt::from_unsigned_tx(tx).expect("valid PSBT");

    // Build a minimal InputRegState (we need a valid blind sig — borrow test helper pattern)
    use blind_rsa_signatures::{Sha384, PSS, Randomized, DefaultRng};
    type BjKeyPair = blind_rsa_signatures::KeyPair<Sha384, PSS, Randomized>;
    type BjPublicKey = blind_rsa_signatures::PublicKey<Sha384, PSS, Randomized>;
    type BjSecretKey = blind_rsa_signatures::SecretKey<Sha384, PSS, Randomized>;

    let kp = BjKeyPair::generate(&mut DefaultRng, 2048).expect("keygen");
    let pk = BjPublicKey::from_der(&kp.pk.to_der().unwrap()).unwrap();
    let message_bytes = [0u8; 32];
    let blinding_result = pk.blind(&mut DefaultRng, message_bytes).expect("blind");
    let sk_der = kp.sk.to_der().unwrap();
    let sk = BjSecretKey::from_der(&sk_der).unwrap();
    let blind_sig = sk.blind_sign(&blinding_result.blind_message).unwrap();
    let sig = pk.finalize(&blind_sig, &blinding_result, message_bytes).unwrap();

    let state = InputRegState {
        round_id: uuid::Uuid::new_v4(),
        session_token: vec![0u8; 32],
        blinding_secret: blinding_result.secret,
        msg_randomizer: blinding_result.msg_randomizer,
        message_bytes,
        output_script: ScriptBuf::new(),
        unblinded_sig: sig,
        pk_hash_at_registration: [0u8; 32],
        participants_registered,
        denomination_sats,
    };

    let result = check_psbt_denomination_outputs(&tampered_psbt, &state);
    assert!(
        result.is_err(),
        "Tampered PSBT with 2 denom outputs for 3 participants must be rejected; got Ok"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("output censorship"),
        "Error must mention 'output censorship'; got: {err_msg}"
    );
    eprintln!("adversarial_tampered_psbt_rejected PASSED: tampered PSBT rejected with: {err_msg}");
}

// ---------------------------------------------------------------------------
// Smoke test: /info endpoint fields, no bitcoind required.
// ---------------------------------------------------------------------------

/// Verify /info response fields when coordinator is in Idle state.
/// GET /info reads only in-memory round state — no bitcoind RPC calls needed.
///
/// **WR-04: deliberately unbindable bitcoind RPC target.**
/// This test exercises the `/info` codepath without standing up a real
/// bitcoind. `build_router` is invoked directly (not `coordinator::run`),
/// so `startup_health_check` does not fire and no route this test hits
/// touches the `BitcoinRpc` Arc. We still need to construct one to satisfy
/// the router's type, so we point it at a sentinel host
/// (`invalid-rpc-not-running.localhost:1`) that will fail DNS resolution
/// AND TCP connect — that way if a future `/info` handler quietly grows
/// an RPC dependency (e.g. reporting current block height), this test
/// fails fast and obviously instead of silently touching whatever happens
/// to be bound to `127.0.0.1:18443` on the CI runner.
#[tokio::test]
async fn coordinator_info_endpoint_fields() {
    use coordinator::api::build_router;
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig};
    use coordinator::round::state::RoundState;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // WR-06: per-test temp dir for ban file so parallel tests cannot race.
    let _tmp_dir = tempfile::tempdir().expect("create temp dir");
    let ban_file_path = _tmp_dir
        .path()
        .join("ban_list.jsonl")
        .to_string_lossy()
        .into_owned();

    // WR-04: sentinel URL — see test docstring. Any accidental RPC use
    // fails with a connect/resolve error, not a silent connect to a
    // co-tenant on the CI runner.
    let sentinel_rpc_url = "http://invalid-rpc-not-running.localhost:1";
    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: sentinel_rpc_url.into(),
            bitcoin_rpc_user: String::new(),
            bitcoin_rpc_pass: String::new(),
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000,
            min_participants: 3,
            max_participants: 3,
            round_timeout_input_reg_secs: 30,
            round_timeout_output_reg_secs: 30,
            round_timeout_signing_secs: 15,
            blame_ban_duration_secs: 60,
            fee_rate_sat_per_vbyte: 1,
            listen_addr: addr.to_string(),
            ban_file_path,
            rate_limit_info_per_min: 60,
            rate_limit_writes_per_min: 30,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            tor_mode: false,
        },
        discovery: DiscoveryConfig::default(),
    });

    let rpc = Arc::new(BitcoinRpc::new(
        sentinel_rpc_url.into(),
        String::new(),
        String::new(),
    ));
    let round_state = Arc::new(RwLock::new(RoundState::new_idle()));
    let app = build_router(round_state, rpc, cfg.clone());

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for server to be ready
    let http_client = reqwest::Client::new();
    let coordinator_url = format!("http://{}", addr);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let ok = http_client
            .get(format!("{}/info", coordinator_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Coordinator smoke test server did not start within 3s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Fetch and verify /info fields
    let info: shared::protocol::InfoResponse = http_client
        .get(format!("{}/info", coordinator_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(info.round_state, "idle", "Coordinator starts in idle");
    assert_eq!(info.denomination_sats, 100_000);
    assert_eq!(info.min_participants, 3);
    assert_eq!(info.max_participants, 3);
    assert_eq!(info.network, "regtest");
    assert!(
        info.rsa_pubkey_hash.is_none(),
        "rsa_pubkey_hash must be None when idle"
    );
    assert!(
        info.rsa_pubkey_der_b64.is_none(),
        "rsa_pubkey_der_b64 must be None when idle"
    );
    assert!(info.round_id.is_some(), "round_id must always be present");
    assert!(!info.version.is_empty(), "version must be non-empty");

    eprintln!(
        "coordinator_info_endpoint_fields PASSED: round_state={}",
        info.round_state
    );
}

// ---------------------------------------------------------------------------
// TEST-12: Round restart after blame — non-signer banned, remaining 2 clients
// complete a fresh round and CoinJoin tx appears in mempool.
// ---------------------------------------------------------------------------

/// TEST-12: Round restart and completion after blame.
///
/// Flow:
///   1. Fund 3 UTXOs on regtest.
///   2. Spawn coordinator (min=2, max=3, signing_timeout=2s) with auto-restart.
///   3. All 3 clients register inputs + outputs in round 1. Client 0 does NOT sign.
///   4. Wait 4s for signing timeout + blame to fire and round to restart in InputReg.
///   5. Assert coordinator is back in input_reg phase.
///   6. Assert client 0's UTXO is banned (HTTP 403 on re-registration attempt).
///   7. Clients 1 and 2 complete a full round: input → output → sign.
///   8. Assert CoinJoin tx appears in mempool with exactly 2 denomination outputs.
///
/// Skips gracefully if bitcoind is not available.
#[tokio::test]
async fn round_restart_and_completion_after_blame() {
    let exe = require_bitcoind!();

    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    let denomination: u64 = 100_000;

    // Spawn coordinator with blame + auto-restart (min_participants=2, signing_timeout=2s)
    let (coordinator_url, ban_list) = spawn_coordinator_with_blame_and_restart(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;
    // Hold bitcoind_guard for the full test duration; dropping ends the daemon.
    let _bitcoind_guard = bitcoind_guard;

    let test_wifs = [
        "cPyRhf56BjNjMMmijQQvUeNG2VPkmxvBf6iYpygDu6DWR8UqkZGQ",
        "cQExMWoJTPmEFT131NAnkTKSGUb8JDV7wV6U7yx4SDzNMvrfNPLz",
        "cRh8UTgSFtzpWVSLZF5cQL2HN3awKze49MPiLurQ9KL4h71ah15F",
    ];

    let non_signer_utxo = setup.utxos[0].0.clone();

    // ----- Round 1: all 3 register inputs + outputs; client 0 does NOT sign -----
    let client_handles: Vec<_> = test_wifs.iter().enumerate().map(|(i, wif)| {
        let url = coordinator_url.clone();
        let wif = wif.to_string();
        let (utxo_str, utxo_value) = setup.utxos[i].clone();
        let should_sign = i != 0;

        tokio::spawn(async move {
            use bitcoin::Network;
            use client::http::CoordinatorClient;
            use client::round;
            use client::wallet::ClientWallet;

            let wallet = ClientWallet::from_wif(&wif, &utxo_str, utxo_value, Network::Regtest)
                .expect("ClientWallet creation");
            let coordinator_client = CoordinatorClient::new(url);

            let info = coordinator_client
                .poll_until_phase("input_reg", 100, Duration::from_secs(600))
                .await
                .expect("poll for input_reg");

            let reg = round::input::register_input(&coordinator_client, &wallet, &info)
                .await
                .expect("register_input");

            coordinator_client
                .poll_until_phase("output_reg", 100, Duration::from_secs(600))
                .await
                .expect("poll for output_reg");

            round::output::register_output(&coordinator_client, &wallet, &reg, &info)
                .await
                .expect("register_output");

            if should_sign {
                coordinator_client
                    .poll_until_phase("signing", 100, Duration::from_secs(600))
                    .await
                    .expect("poll for signing");

                // verify_and_sign may error if timeout fires before all sigs — acceptable
                let _ = round::sign::verify_and_sign(&coordinator_client, &wallet, &reg, 50)
                    .await;
            }
            // Client 0 stops here — does not sign, triggering blame
        })
    }).collect();

    for handle in client_handles {
        handle.await.expect("client task panicked");
    }

    // ----- Wait for signing timeout + blame + auto-restart to input_reg -----
    // WR-05 (Plan 10-02): poll-until-deadline replaces the previous bare
    // 4s sleep. Predicate: coordinator /info reports round_state=="input_reg"
    // AND ban_list contains non_signer_utxo. Deadline: 10s (2.5x original
    // sleep budget). 100ms poll cadence matches round_bootstrap.rs:141.
    let http_client = reqwest::Client::new();
    let (info, banned) = {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut last_round_state: Option<String> = None;
        let mut last_banned: bool;
        loop {
            let info_now: Option<shared::protocol::InfoResponse> = match http_client
                .get(format!("{}/info", coordinator_url))
                .send()
                .await
            {
                Ok(r) => r.json().await.ok(),
                Err(_) => None,
            };

            let now = coordinator::round::blame::now_unix_secs();
            let banned_now = {
                let bl = ban_list.read().await;
                bl.is_banned(&non_signer_utxo, now)
            };

            if let Some(ref i) = info_now {
                if i.round_state == "input_reg" && banned_now {
                    break (i.clone(), banned_now);
                }
                last_round_state = Some(i.round_state.clone());
            }
            last_banned = banned_now;

            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "After blame, coordinator did not restart to input_reg AND ban \
                     non-signer UTXO within 10s. Last /info round_state: {:?}, last \
                     ban_list.is_banned({}): {}",
                    last_round_state, non_signer_utxo, last_banned
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    assert_eq!(
        info.round_state, "input_reg",
        "After blame restart, coordinator must be in input_reg; got '{}'",
        info.round_state
    );

    // ----- Non-signer UTXO must be in ban list (already polled above) -----
    assert!(
        banned,
        "Non-signer UTXO must be banned after blame; utxo={}",
        non_signer_utxo
    );

    // ----- Assert banned UTXO gets HTTP 403 on re-registration -----
    {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};

        let banned_req = shared::protocol::InputRegRequest {
            utxo_outpoint: non_signer_utxo.clone(),
            ownership_proof: "[\"00\"]".to_string(),
            blinded_token: B64.encode([0u8; 64]),
            change_address: "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080".to_string(),
        };

        let resp = reqwest::Client::new()
            .post(format!("{}/round/input", coordinator_url))
            .json(&banned_req)
            .send()
            .await
            .expect("HTTP request sent");

        assert_eq!(
            resp.status().as_u16(), 403,
            "Banned UTXO must receive HTTP 403; got {}",
            resp.status()
        );
        eprintln!("round_restart_and_completion_after_blame: banned UTXO got 403 as expected");
    }

    // ----- Round 2: clients 1 and 2 complete a full round -----
    let round2_handles: Vec<_> = test_wifs[1..].iter().enumerate().map(|(idx, wif)| {
        let i = idx + 1; // client index (1 or 2)
        let url = coordinator_url.clone();
        let wif = wif.to_string();
        let (utxo_str, utxo_value) = setup.utxos[i].clone();

        tokio::spawn(async move {
            use bitcoin::Network;
            use client::http::CoordinatorClient;
            use client::round;
            use client::wallet::ClientWallet;

            let wallet = ClientWallet::from_wif(&wif, &utxo_str, utxo_value, Network::Regtest)
                .expect("ClientWallet creation");
            let coordinator_client = CoordinatorClient::new(url);

            // Coordinator is already in input_reg after restart
            let info = coordinator_client
                .poll_until_phase("input_reg", 100, Duration::from_secs(600))
                .await
                .expect("round 2 poll for input_reg");

            let reg = round::input::register_input(&coordinator_client, &wallet, &info)
                .await
                .expect("round 2 register_input");

            coordinator_client
                .poll_until_phase("output_reg", 100, Duration::from_secs(600))
                .await
                .expect("round 2 poll for output_reg");

            round::output::register_output(&coordinator_client, &wallet, &reg, &info)
                .await
                .expect("round 2 register_output");

            coordinator_client
                .poll_until_phase("signing", 100, Duration::from_secs(600))
                .await
                .expect("round 2 poll for signing");

            round::sign::verify_and_sign(&coordinator_client, &wallet, &reg, 100)
                .await
                .expect("round 2 verify_and_sign");
        })
    }).collect();

    for handle in round2_handles {
        handle.await.expect("round 2 client task panicked");
    }

    // ----- Wait for round-2 broadcast -----
    // WR-05 (Plan 10-02): poll-until-deadline replaces the previous bare
    // 2s sleep. Same shape as the first mempool wait above —
    // predicate: !get_raw_mempool().is_empty(). Deadline: 10s
    // (5x original sleep budget). 100ms poll cadence.
    let mempool_txids: Vec<String> = {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let rpc_url_c = setup.rpc_url.clone();
            let rpc_user_c = setup.rpc_user.clone();
            let rpc_pass_c = setup.rpc_pass.clone();
            let txids: Vec<String> = tokio::task::spawn_blocking(move || {
                use corepc_node::client::client_sync::Auth;
                let auth = Auth::UserPass(rpc_user_c, rpc_pass_c);
                let client = corepc_node::Client::new_with_auth(&rpc_url_c, auth)
                    .expect("create rpc client for mempool check");
                client.get_raw_mempool().expect("get_raw_mempool").0
            })
            .await
            .expect("mempool check spawn_blocking");

            if !txids.is_empty() {
                break txids;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!(
                    "Round-2 CoinJoin tx never appeared in mempool within 10s after round 2 \
                     completed. Last observation: mempool empty."
                );
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    assert!(
        !mempool_txids.is_empty(),
        "CoinJoin tx must appear in mempool after round 2 completes"
    );

    let coinjoin_txid = mempool_txids[0].clone();

    let rpc_url2 = setup.rpc_url.clone();
    let rpc_user2 = setup.rpc_user.clone();
    let rpc_pass2 = setup.rpc_pass.clone();

    let denom_output_count: usize = tokio::task::spawn_blocking(move || {
        use std::str::FromStr;
        use corepc_node::client::client_sync::Auth;
        let auth = Auth::UserPass(rpc_user2, rpc_pass2);
        let client = corepc_node::Client::new_with_auth(&rpc_url2, auth)
            .expect("create rpc client for tx verify");
        let txid = bitcoin::Txid::from_str(&coinjoin_txid).unwrap();
        let tx = client.get_raw_transaction_verbose(txid)
            .expect("get_raw_transaction_verbose");
        tx.outputs.iter()
            .filter(|out| {
                let sats = (out.value * 100_000_000.0).round() as u64;
                sats == denomination
            })
            .count()
    }).await.expect("tx verify spawn_blocking");

    assert_eq!(
        denom_output_count, 2,
        "Round 2 CoinJoin tx must have exactly 2 denomination outputs; got {}",
        denom_output_count
    );

    eprintln!(
        "round_restart_and_completion_after_blame PASSED: blame fired, client 0 banned, \
         round 2 completed with {} denomination outputs",
        denom_output_count
    );
}
