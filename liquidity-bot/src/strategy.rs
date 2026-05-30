use anyhow::{anyhow, Result};
use shared::bip322::ScriptType;
use shared::protocol::InfoResponse;
use std::path::PathBuf;

/// Determines when the liquidity bot should attempt to join a round.
///
/// Safety constraints:
/// - Only joins rounds with the configured denomination (avoids wasted fees)
/// - Only joins during input_reg phase (cannot join later phases)
/// - Does not compete with real users once round is nearly full (join_threshold)
pub struct JoinStrategy {
    /// Target denomination in satoshis. Bot skips rounds with different denominations.
    pub target_denomination_sats: u64,
    /// Maximum consecutive round failures before entering long backoff (300s).
    pub max_consecutive_failures: u32,
    /// Join if participants_registered < this threshold. Avoids crowding real users.
    /// Default: 10 (join until round can start, then back off).
    pub join_threshold: u32,
    /// Seconds to sleep between polling loops.
    pub poll_interval_secs: u64,
}

impl JoinStrategy {
    pub fn new(target_denomination_sats: u64) -> Self {
        Self {
            target_denomination_sats,
            max_consecutive_failures: 5,
            join_threshold: 10, // generous; real deployments may tune this lower
            poll_interval_secs: 5,
        }
    }

    /// Returns true if the bot should attempt to join the current round.
    pub fn should_join(&self, info: &InfoResponse) -> bool {
        info.round_state == "input_reg"
            && info.denomination_sats == self.target_denomination_sats
            && info.participants_registered < self.join_threshold
    }
}

// ---------------------------------------------------------------------------
// RotationState — per-round script-type round-robin counter (CD-27)
// ---------------------------------------------------------------------------

/// Per-run state for the bot's round-robin script-type rotation.
///
/// The counter is persisted to `counter_file` between bot runs. Docker's
/// `restart: unless-stopped` re-launches the bot, which re-reads the counter
/// and advances to the next type in `enabled` (D-95).
///
/// Missing file → counter 0 (fresh install behaviour).
/// Malformed file → operator-facing bail with file path + malformed content.
pub struct RotationState {
    counter_file: PathBuf,
    enabled: Vec<ScriptType>,
}

impl RotationState {
    /// Construct a RotationState. Rejects an empty `enabled` vec with an
    /// operator-facing error (BLINDJOIN_BOT_SCRIPT_TYPES must not be empty).
    pub fn new(counter_file: PathBuf, enabled: Vec<ScriptType>) -> Result<Self> {
        if enabled.is_empty() {
            return Err(anyhow!(
                "RotationState: enabled types must be non-empty \
                 (BLINDJOIN_BOT_SCRIPT_TYPES parsed to zero entries)"
            ));
        }
        Ok(Self { counter_file, enabled })
    }

    /// Read the persisted counter and return `enabled[counter % enabled.len()]`.
    ///
    /// Missing file → counter 0. Malformed file → operator-facing bail.
    pub async fn pick_script_type(&self) -> Result<ScriptType> {
        let counter = self.read_counter().await?;
        let idx = (counter as usize) % self.enabled.len();
        Ok(self.enabled[idx])
    }

    /// Read current counter, increment, atomically write back.
    ///
    /// Atomic write idiom per RESEARCH §Q4: `tokio::fs::write` to `${path}.tmp`
    /// then `tokio::fs::rename` to `${path}` — POSIX same-fs atomic; not BLAME-05's
    /// append-only pattern.
    pub async fn bump_counter(&self) -> Result<()> {
        let current = self.read_counter().await?;
        let next = current.saturating_add(1);
        self.write_counter_atomic(next).await?;
        Ok(())
    }

    async fn read_counter(&self) -> Result<u64> {
        match tokio::fs::read_to_string(&self.counter_file).await {
            Ok(s) => s.trim().parse::<u64>().map_err(|e| anyhow!(
                "BLINDJOIN_BOT_COUNTER_FILE = '{}' contains malformed counter \
                 (line 1): '{}' ({e})",
                self.counter_file.display(), s.trim()
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(anyhow!(
                "BLINDJOIN_BOT_COUNTER_FILE read failed at '{}': {e}",
                self.counter_file.display()
            )),
        }
    }

    async fn write_counter_atomic(&self, counter: u64) -> Result<()> {
        let tmp = self.counter_file.with_extension("tmp");
        tokio::fs::write(&tmp, format!("{}\n", counter)).await?;
        tokio::fs::rename(&tmp, &self.counter_file).await?;
        Ok(())
    }
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn rotation_state_round_robin_advances_counter() {
        let dir = tempdir().unwrap();
        let counter_file = dir.path().join("counter");
        let enabled = vec![ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh];
        let state = RotationState::new(counter_file.clone(), enabled).unwrap();

        assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2wpkh);
        state.bump_counter().await.unwrap();
        assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2tr);
        state.bump_counter().await.unwrap();
        assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2shP2wpkh);
        state.bump_counter().await.unwrap();
        // Wraps around: counter 3 % 3 = 0 → P2wpkh
        assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2wpkh);
    }

    #[tokio::test]
    async fn rotation_state_single_type_does_not_rotate() {
        let dir = tempdir().unwrap();
        let state = RotationState::new(
            dir.path().join("counter"),
            vec![ScriptType::P2wpkh],
        )
        .unwrap();
        for _ in 0..3 {
            assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2wpkh);
            state.bump_counter().await.unwrap();
        }
    }

    #[tokio::test]
    async fn rotation_state_empty_enabled_returns_err() {
        let dir = tempdir().unwrap();
        assert!(RotationState::new(dir.path().join("counter"), vec![]).is_err());
    }

    #[tokio::test]
    async fn rotation_state_counter_file_roundtrip() {
        let dir = tempdir().unwrap();
        let counter_file = dir.path().join("counter");
        let enabled = vec![ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh];

        // Missing file → counter 0 → P2wpkh
        let state = RotationState::new(counter_file.clone(), enabled.clone()).unwrap();
        assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2wpkh);

        // Pre-seeded "5\n" → counter 5 → 5 % 3 = 2 → P2shP2wpkh
        tokio::fs::write(&counter_file, b"5\n").await.unwrap();
        assert_eq!(state.pick_script_type().await.unwrap(), ScriptType::P2shP2wpkh);

        // Malformed → Err
        tokio::fs::write(&counter_file, b"abc\n").await.unwrap();
        assert!(state.pick_script_type().await.is_err());
    }

    #[tokio::test]
    async fn rotation_state_atomic_write_via_tmp_then_rename() {
        let dir = tempdir().unwrap();
        let counter_file = dir.path().join("counter");
        let state = RotationState::new(counter_file.clone(), vec![ScriptType::P2wpkh]).unwrap();

        state.bump_counter().await.unwrap();

        // After atomic write, final file exists with "1\n" and .tmp sibling does NOT exist.
        assert_eq!(
            tokio::fs::read_to_string(&counter_file).await.unwrap().trim(),
            "1"
        );
        let tmp_path = counter_file.with_extension("tmp");
        assert!(
            !tmp_path.exists(),
            "tmp sibling should have been renamed away"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::InfoResponse;

    fn make_info(round_state: &str, denomination_sats: u64, participants_registered: u32) -> InfoResponse {
        // Phase 16 Plan 16-01 (Rule 3 — Blocker): InfoResponse gained 2 new
        // wire-format fields. Populate them with the legacy P2WPKH-only
        // defaults so this test fixture reproduces the v1.3 InfoResponse
        // shape byte-exactly (no behaviour change in the strategy under test).
        use shared::bip322::ScriptType;
        InfoResponse {
            version: "0.1.0".to_string(),
            network: "signet".to_string(),
            denomination_sats,
            min_participants: 3,
            max_participants: 10,
            round_state: round_state.to_string(),
            participants_registered,
            rsa_pubkey_hash: None,
            rsa_pubkey_der_b64: None,
            round_id: None,
            supported_script_types: vec![ScriptType::P2wpkh],
            output_script_type: ScriptType::P2wpkh,
        }
    }

    #[test]
    fn test_should_join_true_when_input_reg_and_denomination_matches() {
        let strategy = JoinStrategy::new(1_000_000);
        let info = make_info("input_reg", 1_000_000, 2);
        assert!(strategy.should_join(&info));
    }

    #[test]
    fn test_should_join_false_wrong_denomination() {
        let strategy = JoinStrategy::new(1_000_000);
        let info = make_info("input_reg", 500_000, 2);
        assert!(!strategy.should_join(&info));
    }

    #[test]
    fn test_should_join_false_wrong_phase_idle() {
        let strategy = JoinStrategy::new(1_000_000);
        let info = make_info("idle", 1_000_000, 0);
        assert!(!strategy.should_join(&info));
    }

    #[test]
    fn test_should_join_false_wrong_phase_output_reg() {
        let strategy = JoinStrategy::new(1_000_000);
        let info = make_info("output_reg", 1_000_000, 3);
        assert!(!strategy.should_join(&info));
    }

    #[test]
    fn test_should_join_false_threshold_reached() {
        let mut strategy = JoinStrategy::new(1_000_000);
        strategy.join_threshold = 3;
        let info = make_info("input_reg", 1_000_000, 3);
        assert!(!strategy.should_join(&info));
    }
}
