use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

/// A single ban record stored under SHA-256(utxo outpoint string) hex key in BanList.
#[derive(Debug, Clone)]
pub struct BanEntry {
    /// Unix timestamp (seconds) when the ban was recorded
    pub banned_at: u64,
    /// Unix timestamp (seconds) when the ban expires
    pub expires_at: u64,
}

/// In-memory ban list. Key = SHA-256(utxo outpoint string) hex-encoded.
/// All entries — runtime bans and file-loaded bans — use the same hashed key,
/// preventing is_banned() misses after coordinator restart.
/// Checked on every input registration attempt.
#[derive(Debug, Default)]
pub struct BanList {
    entries: HashMap<String, BanEntry>,
}

impl BanList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a ban for `utxo_str`. Hashes utxo_str (SHA-256) before storage so
    /// runtime bans use the same key format as file-loaded bans. Overwrites existing
    /// entry (refreshes expiry). `now_secs` = current Unix timestamp.
    pub fn ban(&mut self, utxo_str: &str, now_secs: u64, ban_duration: Duration) {
        use sha2::{Sha256, Digest};
        let key = hex::encode(Sha256::digest(utxo_str.as_bytes()));
        self.entries.insert(key, BanEntry {
            banned_at: now_secs,
            expires_at: now_secs + ban_duration.as_secs(),
        });
    }

    /// Returns true if the UTXO is currently banned (entry exists AND not expired).
    /// Hashes utxo_str (SHA-256) before lookup to match the storage key format.
    pub fn is_banned(&self, utxo_str: &str, now_secs: u64) -> bool {
        use sha2::{Sha256, Digest};
        let key = hex::encode(Sha256::digest(utxo_str.as_bytes()));
        self.entries.get(&key)
            .map(|entry| now_secs < entry.expires_at)
            .unwrap_or(false)
    }

    /// Expose entries for persistence (plan 02).
    #[allow(dead_code)]
    pub fn entries(&self) -> &HashMap<String, BanEntry> {
        &self.entries
    }

    /// Merge a ban entry loaded from the ban file (used on startup by plan 02).
    /// The key passed here is already SHA-256 hashed (from the on-disk record).
    pub fn load_entry(&mut self, utxo_hash: String, entry: BanEntry) {
        // Only load if not expired at load time (caller filters before calling)
        self.entries.insert(utxo_hash, entry);
    }
}

/// Returns the utxo outpoint strings that registered an input but never submitted
/// a partial signature. Called by the signing timeout handler.
///
/// BLAME-01: Non-signer detection.
pub fn detect_non_signers(
    registered_inputs: &std::collections::HashMap<String, crate::round::state::RegisteredInput>,
    partial_sigs: &std::collections::HashMap<String, Vec<u8>>,
) -> Vec<String> {
    registered_inputs
        .keys()
        .filter(|utxo_str| !partial_sigs.contains_key(*utxo_str))
        .cloned()
        .collect()
}

/// Returns true if the output count is less than the input count, indicating at
/// least one participant failed to register an output.
///
/// BLAME-02: Missing output detection (aggregate only).
/// Since outputs are anonymous (blind token), individual identification is impossible.
/// The coordinator detects the gap and transitions to Blame; no per-UTXO banning
/// occurs for missing outputs (outputs are unlinkable by design).
pub fn has_missing_outputs(
    input_count: usize,
    output_count: usize,
) -> bool {
    output_count < input_count
}

/// On-disk record format (one JSON line per ban event).
/// utxo_hash = hex(SHA-256(utxo_str.as_bytes())) — raw outpoints are NOT persisted.
/// This matches the in-memory BanList key format (established in plan 01).
/// PRIV-02 extension: hashing prevents outpoint disclosure even if the ban file leaks.
#[derive(Debug, Serialize, Deserialize)]
pub struct BanRecord {
    pub utxo_hash: String,
    pub banned_at: u64,
    pub expires_at: u64,
}

/// Hash a utxo_str for on-disk storage.
/// Centralised here so callers never need to hash manually.
pub fn hash_utxo_str(utxo_str: &str) -> String {
    use sha2::{Sha256, Digest};
    hex::encode(Sha256::digest(utxo_str.as_bytes()))
}

/// Append a single ban record to the ban file (append-only, newline-delimited JSON).
/// Creates the file if it does not exist.
/// BLAME-05.
pub fn append_ban_entry(path: &str, utxo_str: &str, entry: &BanEntry) -> std::io::Result<()> {
    use std::io::Write;
    let record = BanRecord {
        utxo_hash: hash_utxo_str(utxo_str),
        banned_at: entry.banned_at,
        expires_at: entry.expires_at,
    };
    let line = serde_json::to_string(&record)
        .map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)
}

/// Load all unexpired ban entries from the ban file.
/// Returns Ok(vec![]) if the file does not exist (first startup).
/// Skips unparseable lines with a tracing::warn!.
/// Returns (utxo_hash, BanEntry) pairs — the hash is stored directly in BanList.load_entry.
/// BLAME-05, BLAME-06.
pub fn load_unexpired_entries(path: &str, now_secs: u64) -> std::io::Result<Vec<(String, BanEntry)>> {
    use std::io::{BufRead, BufReader};
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let reader = BufReader::new(file);
    let mut result = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() { continue; }
        match serde_json::from_str::<BanRecord>(line) {
            Ok(record) if now_secs < record.expires_at => {
                result.push((record.utxo_hash.clone(), BanEntry {
                    banned_at: record.banned_at,
                    expires_at: record.expires_at,
                }));
            }
            Ok(_) => {} // expired — skip silently
            Err(_) => {
                tracing::warn!("ban_file: skipping unparseable line");
            }
        }
    }
    Ok(result)
}

/// Returns the current Unix timestamp in seconds.
pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Outcome returned by on_signing_timeout, consumed by the caller in main.rs.
pub enum BlameOutcome {
    /// Round aborted — coordinator returns to Idle without restart.
    FullAbort,
    /// Round should restart excluding the listed UTXOs.
    RestartWithout {
        #[allow(dead_code)]
        banned_utxos: Vec<String>,
    },
}

/// Called when the signing timeout fires. Detects non-signers, bans them,
/// appends to ban file, and transitions Signing→Blame→Idle.
///
/// `ban_list` is the &mut BanList from AppState (caller holds write lock).
/// `blame_round_count` is the current consecutive-blame count; if it reaches 2,
/// triggers FullAbort per Pitfall 3 in PITFALLS.md (T-02-07).
/// `ban_file_path` is from config (BLAME-05).
///
/// BLAME-01, BLAME-04.
pub fn on_signing_timeout(
    state: &mut crate::round::state::RoundState,
    ban_list: &mut BanList,
    ban_file_path: &str,
    ban_duration_secs: u64,
    blame_round_count: u32,
) -> BlameOutcome {
    use crate::round::state::Phase;

    let non_signers = if let Some(inner) = &state.inner {
        detect_non_signers(&inner.registered_inputs, &inner.partial_sigs)
    } else {
        vec![]
    };

    let now = now_unix_secs();
    let ban_duration = Duration::from_secs(ban_duration_secs);

    for utxo_str in &non_signers {
        ban_list.ban(utxo_str, now, ban_duration);
        let entry = BanEntry { banned_at: now, expires_at: now + ban_duration_secs };
        if let Err(e) = append_ban_entry(ban_file_path, utxo_str, &entry) {
            tracing::warn!(ban_file = ban_file_path, "Failed to append ban entry: {e}");
        }
    }

    // Transition Signing→Blame→Idle (zeroes round state)
    let _ = state.transition_to(Phase::Blame);
    let _ = state.transition_to(Phase::Idle);

    // Per Pitfall 3 in PITFALLS.md: cap consecutive blame rounds at 2 (T-02-07)
    if blame_round_count >= 2 {
        tracing::warn!("blame round cap reached — full round abort");
        return BlameOutcome::FullAbort;
    }

    if non_signers.is_empty() {
        // Signing timeout fired but nobody was missing — full abort (no restart benefit)
        BlameOutcome::FullAbort
    } else {
        BlameOutcome::RestartWithout { banned_utxos: non_signers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::round::state::RegisteredInput;

    #[test]
    fn ban_and_is_banned() {
        let mut bl = BanList::new();
        bl.ban("abc:0", 1000, Duration::from_secs(3600));
        assert!(bl.is_banned("abc:0", 1001));
    }

    #[test]
    fn ban_expired_not_banned() {
        let mut bl = BanList::new();
        bl.ban("abc:0", 1000, Duration::from_secs(3600));
        // Check at expiry time — not banned (strictly less than)
        assert!(!bl.is_banned("abc:0", 4600));
    }

    #[test]
    fn unknown_utxo_not_banned() {
        let bl = BanList::new();
        assert!(!bl.is_banned("xyz:1", 1000));
    }

    #[test]
    fn ban_refresh_extends_expiry() {
        let mut bl = BanList::new();
        bl.ban("abc:0", 1000, Duration::from_secs(3600));
        // Re-ban with longer duration
        bl.ban("abc:0", 2000, Duration::from_secs(7200));
        // Old expiry was 4600; new expiry is 9200
        assert!(bl.is_banned("abc:0", 5000));
        assert!(!bl.is_banned("abc:0", 9300));
    }

    #[test]
    fn detect_non_signers_finds_missing() {
        let mut inputs: HashMap<String, RegisteredInput> = HashMap::new();
        inputs.insert("tx1:0".to_string(), RegisteredInput {
            utxo_str: "tx1:0".to_string(),
            change_address: "addr1".to_string(),
            blind_sig_hash: [0u8; 32],
        });
        inputs.insert("tx2:0".to_string(), RegisteredInput {
            utxo_str: "tx2:0".to_string(),
            change_address: "addr2".to_string(),
            blind_sig_hash: [0u8; 32],
        });
        let mut sigs: HashMap<String, Vec<u8>> = HashMap::new();
        sigs.insert("tx1:0".to_string(), vec![1, 2, 3]);
        // tx2:0 did not sign
        let mut non_signers = detect_non_signers(&inputs, &sigs);
        non_signers.sort(); // sort for deterministic comparison
        assert_eq!(non_signers, vec!["tx2:0".to_string()]);
    }

    #[test]
    fn detect_non_signers_all_signed() {
        let mut inputs: HashMap<String, RegisteredInput> = HashMap::new();
        inputs.insert("tx1:0".to_string(), RegisteredInput {
            utxo_str: "tx1:0".to_string(),
            change_address: "addr1".to_string(),
            blind_sig_hash: [0u8; 32],
        });
        let mut sigs: HashMap<String, Vec<u8>> = HashMap::new();
        sigs.insert("tx1:0".to_string(), vec![1, 2, 3]);
        assert!(detect_non_signers(&inputs, &sigs).is_empty());
    }

    #[test]
    fn has_missing_outputs_detects_gap() {
        assert!(has_missing_outputs(3, 2));
        assert!(!has_missing_outputs(3, 3));
    }

    // --- Ban file persistence tests (BLAME-05, BLAME-06) ---

    #[test]
    fn ban_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ban_list.jsonl");
        let path_str = path.to_str().unwrap();

        let entry1 = BanEntry { banned_at: 1000, expires_at: 5000 };
        let entry2 = BanEntry { banned_at: 2000, expires_at: 9000 };

        append_ban_entry(path_str, "tx1:0", &entry1).unwrap();
        append_ban_entry(path_str, "tx2:1", &entry2).unwrap();

        // Load with now=500 (both entries unexpired)
        let loaded = load_unexpired_entries(path_str, 500).unwrap();
        assert_eq!(loaded.len(), 2, "Both entries should be loaded");

        // Verify hashes match what hash_utxo_str produces
        let hash1 = hash_utxo_str("tx1:0");
        let hash2 = hash_utxo_str("tx2:1");
        let loaded_hashes: Vec<&str> = loaded.iter().map(|(h, _)| h.as_str()).collect();
        assert!(loaded_hashes.contains(&hash1.as_str()), "tx1:0 hash must be present");
        assert!(loaded_hashes.contains(&hash2.as_str()), "tx2:1 hash must be present");

        // Verify BanEntry fields
        let e1 = loaded.iter().find(|(h, _)| h == &hash1).map(|(_, e)| e).unwrap();
        assert_eq!(e1.banned_at, 1000);
        assert_eq!(e1.expires_at, 5000);
    }

    #[test]
    fn ban_file_expired_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ban_list.jsonl");
        let path_str = path.to_str().unwrap();

        // expires_at = 1000; load with now = 2000 → should be filtered
        let entry = BanEntry { banned_at: 500, expires_at: 1000 };
        append_ban_entry(path_str, "tx3:0", &entry).unwrap();

        let loaded = load_unexpired_entries(path_str, 2000).unwrap();
        assert!(loaded.is_empty(), "Expired entry must not be loaded");
    }

    #[test]
    fn ban_file_missing_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("does_not_exist.jsonl");
        let loaded = load_unexpired_entries(nonexistent.to_str().unwrap(), 1000).unwrap();
        assert!(loaded.is_empty(), "Missing file must return empty vec, not error");
    }

    #[test]
    fn ban_file_skips_corrupt_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ban_list.jsonl");
        let path_str = path.to_str().unwrap();

        // Write a valid entry, then a corrupt line, then another valid entry
        let good = BanEntry { banned_at: 100, expires_at: 9999 };
        append_ban_entry(path_str, "tx4:0", &good).unwrap();

        // Manually append a corrupt line
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(path_str).unwrap();
        writeln!(f, "{{not valid json}}").unwrap();

        append_ban_entry(path_str, "tx5:0", &good).unwrap();

        let loaded = load_unexpired_entries(path_str, 50).unwrap();
        assert_eq!(loaded.len(), 2, "Corrupt line must be skipped, valid entries loaded");
    }
}
