use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use bitcoin::OutPoint;
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;
use crate::round::state::{Phase, RoundState};

type HmacSha256 = Hmac<Sha256>;

/// Spawn a phase timer that fires after `timeout`.
/// If the round is still in `expected_phase` when the timer fires,
/// calls `on_timeout` to evaluate quorum and perform the transition.
///
/// The returned future should be spawned via tokio::spawn. The JoinHandle
/// can be aborted if the phase advances early (e.g., max_participants reached).
///
/// Per D-16: input_reg and output_reg timeouts are 60s, signing timeout is 30s.
#[allow(dead_code)]
pub async fn run_phase_timer(
    round: Arc<RwLock<RoundState>>,
    expected_phase: Phase,
    timeout: Duration,
    on_timeout: impl FnOnce(&mut RoundState) + Send + 'static,
) {
    tokio::time::sleep(timeout).await;
    let mut guard = round.write().await;
    if guard.phase == expected_phase {
        on_timeout(&mut guard);
    }
    // If phase already advanced — no-op. Timer task exits cleanly.
}

/// Generate an HMAC-SHA256 session token for a given (round_secret, UTXO) pair.
///
/// Format: HMAC-SHA256(key=round_secret, data=txid_bytes || vout_le32)
///
/// Deterministic: same inputs always produce the same 32-byte token.
/// Binds signing-phase reconnection to a specific (round, UTXO) pair.
/// Per D-05.
pub fn generate_session_token(round_secret: &[u8; 32], utxo: &OutPoint) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(round_secret)
        .expect("HMAC accepts any key length");
    mac.update(utxo.txid.as_ref()); // 32 bytes, txid in internal byte order
    mac.update(&utxo.vout.to_le_bytes()); // 4 bytes, little-endian vout
    mac.finalize().into_bytes().into()
}

/// Verify a session token against the expected value.
///
/// Uses constant-time comparison (subtle::ConstantTimeEq) to prevent timing
/// oracle attacks that could allow byte-by-byte token recovery via response
/// latency measurement.
pub fn verify_session_token(round_secret: &[u8; 32], utxo: &OutPoint, token: &[u8; 32]) -> bool {
    use subtle::ConstantTimeEq;
    let expected = generate_session_token(round_secret, utxo);
    expected.ct_eq(token).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{OutPoint, Txid};
    use std::str::FromStr;

    fn test_outpoint() -> OutPoint {
        let txid = Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ).unwrap();
        OutPoint::new(txid, 0)
    }

    #[test]
    fn session_token_deterministic() {
        let secret = [0xab_u8; 32];
        let utxo = test_outpoint();
        let t1 = generate_session_token(&secret, &utxo);
        let t2 = generate_session_token(&secret, &utxo);
        assert_eq!(t1, t2);
    }

    #[test]
    fn session_token_different_utxos_differ() {
        let secret = [0xab_u8; 32];
        let txid = Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ).unwrap();
        let utxo0 = OutPoint::new(txid, 0);
        let utxo1 = OutPoint::new(txid, 1);
        assert_ne!(
            generate_session_token(&secret, &utxo0),
            generate_session_token(&secret, &utxo1)
        );
    }

    #[test]
    fn session_token_verify_ok() {
        let secret = [0xcd_u8; 32];
        let utxo = test_outpoint();
        let token = generate_session_token(&secret, &utxo);
        assert!(verify_session_token(&secret, &utxo, &token));
    }

    #[test]
    fn session_token_verify_wrong_token() {
        let secret = [0xcd_u8; 32];
        let utxo = test_outpoint();
        let wrong_token = [0x00_u8; 32];
        assert!(!verify_session_token(&secret, &utxo, &wrong_token));
    }

    #[tokio::test]
    async fn phase_timer_fires_on_expected_phase() {
        use crate::round::state::RoundState;

        let state = Arc::new(RwLock::new(RoundState::new_idle()));
        // Put state into InputReg
        {
            let mut guard = state.write().await;
            guard.transition_to(Phase::InputReg).unwrap();
        }

        let state_clone = Arc::clone(&state);
        run_phase_timer(
            state_clone,
            Phase::InputReg,
            Duration::from_millis(10),
            |round_state| {
                // Simulate timeout: advance to OutputReg
                round_state.transition_to(Phase::OutputReg).unwrap();
            },
        ).await;

        let guard = state.read().await;
        assert_eq!(guard.phase, Phase::OutputReg, "Timer should have advanced phase");
    }

    #[tokio::test]
    async fn phase_timer_noop_when_phase_already_advanced() {
        use crate::round::state::RoundState;

        let state = Arc::new(RwLock::new(RoundState::new_idle()));
        // Put state into OutputReg (already past InputReg)
        {
            let mut guard = state.write().await;
            guard.transition_to(Phase::InputReg).unwrap();
            guard.transition_to(Phase::OutputReg).unwrap();
        }

        let state_clone = Arc::clone(&state);
        // Timer fires expecting InputReg, but we're already in OutputReg — should no-op
        run_phase_timer(
            state_clone,
            Phase::InputReg,
            Duration::from_millis(10),
            |round_state| {
                // This should NOT be called
                round_state.transition_to(Phase::OutputReg).unwrap();
            },
        ).await;

        let guard = state.read().await;
        assert_eq!(guard.phase, Phase::OutputReg, "Phase should remain OutputReg (timer was no-op)");
    }
}
