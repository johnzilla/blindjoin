use shared::protocol::InfoResponse;

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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::InfoResponse;

    fn make_info(round_state: &str, denomination_sats: u64, participants_registered: u32) -> InfoResponse {
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
