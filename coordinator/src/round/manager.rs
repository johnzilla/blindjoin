use std::sync::Arc;
use std::time::Duration;
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use bitcoin::OutPoint;
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;
use rand::RngCore;
use crate::blind::rsa::RsaBlindSigner;
use crate::round::state::{Phase, RoundState, RoundStateInner};

type HmacSha256 = Hmac<Sha256>;

/// Errors returned by `start_round`.
#[derive(Debug, thiserror::Error)]
pub enum StartRoundError {
    #[error("RSA keygen failed: {0}")]
    KeyGen(String),
    #[error("Key serialization failed: {0}")]
    KeySerialize(String),
    #[error("Cannot start a round from phase {0:?} — must be Idle")]
    WrongPhase(Phase),
    #[error("FSM transition Idle → InputReg rejected: {0}")]
    Transition(#[from] crate::round::state::TransitionError),
}

/// Start a new CoinJoin round in production.
///
/// Generates a fresh RSA blind-signing keypair (D-02: per-round ephemeral key),
/// populates a fresh `RoundStateInner` (D-04 cached signer, D-05 fresh
/// round_secret), and transitions Idle → InputReg.
///
/// Caller MUST hold a write lock on `RoundState` and MUST verify the FSM is
/// currently `Phase::Idle`. Returns `StartRoundError::WrongPhase` otherwise.
///
/// This is the single production-grade analog of the test-only
/// `build_input_reg_round_state` helper used by integration tests. Both
/// production startup (see `coordinator::run::run`) and the continuous-rounds
/// re-armer call this function — no `#[cfg(test)]` branches.
pub fn start_round(state: &mut RoundState) -> Result<(), StartRoundError> {
    if state.phase != Phase::Idle {
        return Err(StartRoundError::WrongPhase(state.phase.clone()));
    }

    // Fresh RSA-2048 keypair for this round (D-02)
    let signer = RsaBlindSigner::generate()
        .map_err(|e| StartRoundError::KeyGen(format!("{e}")))?;
    let sk_der = signer.secret_key_der()
        .map_err(|e| StartRoundError::KeySerialize(format!("secret key: {e}")))?;
    let pk_der = signer.public_key_spki_der()
        .map_err(|e| StartRoundError::KeySerialize(format!("public key: {e}")))?;
    let pk_hash = signer.public_key_hash();

    // Per-round HMAC secret for session-token derivation (D-05). 32 random bytes
    // from a CSPRNG. Zeroized when the inner state is dropped.
    let mut round_secret = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut round_secret);

    state.rsa_pubkey_der = Some(pk_der);
    state.rsa_pubkey_hash = Some(pk_hash);
    state.inner = Some(RoundStateInner {
        rsa_signing_key: sk_der,
        rsa_signer: Some(signer),
        round_secret,
        registered_inputs: HashMap::new(),
        redeemed_tokens: HashSet::new(),
        registered_outputs: Vec::new(),
        partial_sigs: HashMap::new(),
        change_addresses: HashMap::new(),
    });

    state.transition_to(Phase::InputReg)?;
    Ok(())
}

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

    /// start_round happy path: from Idle, produces a populated InputReg state.
    #[tokio::test]
    async fn start_round_from_idle_populates_inner() {
        let mut state = RoundState::new_idle();
        let original_round_id = state.round_id;
        start_round(&mut state).expect("start_round from Idle must succeed");

        assert_eq!(state.phase, Phase::InputReg);
        assert!(state.inner.is_some(), "inner must be populated");
        assert!(state.rsa_pubkey_der.is_some(), "rsa_pubkey_der must be set for /info");
        assert!(state.rsa_pubkey_hash.is_some(), "rsa_pubkey_hash must be set");
        assert_eq!(state.participant_count, 0);
        // round_id is NOT changed when entering InputReg from Idle — it's only
        // reassigned on transition_to(Idle). So it equals the value at start.
        assert_eq!(state.round_id, original_round_id);

        // The cached signer must agree with the stored DER bytes (AVAIL-02).
        let inner = state.inner.as_ref().unwrap();
        let pk_hash_from_signer = inner.rsa_signer.as_ref().expect("test fixture: rsa_signer is Some").public_key_hash();
        assert_eq!(
            Some(pk_hash_from_signer),
            state.rsa_pubkey_hash,
            "cached rsa_signer must hash to the published rsa_pubkey_hash"
        );
    }

    /// start_round refuses to clobber an already-running round.
    #[tokio::test]
    async fn start_round_refuses_non_idle_phase() {
        let mut state = RoundState::new_idle();
        start_round(&mut state).expect("first start_round must succeed");
        // Now we're in InputReg — a second start_round must fail.
        let err = start_round(&mut state)
            .expect_err("start_round must fail when phase != Idle");
        match err {
            StartRoundError::WrongPhase(p) => assert_eq!(p, Phase::InputReg),
            other => panic!("expected WrongPhase, got {other:?}"),
        }
    }

    /// Two consecutive rounds get distinct keypairs (per-round ephemeral key, D-02).
    #[tokio::test]
    async fn start_round_generates_fresh_key_each_round() {
        let mut state = RoundState::new_idle();
        start_round(&mut state).unwrap();
        let first_hash = state.rsa_pubkey_hash.unwrap();
        let first_secret = state.inner.as_ref().unwrap().round_secret;

        // Drive the FSM back to Idle the way the timeout-abort path does.
        state.transition_to(Phase::Idle).unwrap();

        start_round(&mut state).unwrap();
        let second_hash = state.rsa_pubkey_hash.unwrap();
        let second_secret = state.inner.as_ref().unwrap().round_secret;

        assert_ne!(first_hash, second_hash, "fresh RSA keypair per round (D-02)");
        assert_ne!(first_secret, second_secret, "fresh round_secret per round (D-05)");
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
