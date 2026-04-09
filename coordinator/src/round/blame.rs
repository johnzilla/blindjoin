use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// Returns the current Unix timestamp in seconds.
pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
}
