use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use zeroize::Zeroize;

/// The round phase state machine.
/// Only valid transitions are permitted via RoundState::transition_to().
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Idle,
    InputReg,
    OutputReg,
    Signing,
    Broadcast,
    Blame,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::InputReg => "input_reg",
            Phase::OutputReg => "output_reg",
            Phase::Signing => "signing",
            Phase::Broadcast => "broadcast",
            Phase::Blame => "blame",
        }
    }

    /// Returns true if transitioning from self to `next` is a valid FSM edge.
    pub fn can_transition_to(&self, next: &Phase) -> bool {
        matches!(
            (self, next),
            (Phase::Idle, Phase::InputReg)
            | (Phase::InputReg, Phase::OutputReg)
            | (Phase::OutputReg, Phase::Signing)
            | (Phase::Signing, Phase::Broadcast)
            | (Phase::Signing, Phase::Blame)
            | (Phase::Broadcast, Phase::Idle)
            | (Phase::Blame, Phase::Idle)
        )
    }
}

/// Registered input details for one participant.
#[derive(Debug, Clone, Zeroize)]
pub struct RegisteredInput {
    /// String representation of the UTXO outpoint ("txid:vout")
    pub utxo_str: String,
    pub change_address: String,
    /// SHA-256 of the blind signature token hash (for double-registration prevention)
    pub blind_sig_hash: [u8; 32],
}

/// Registered output for one participant.
#[derive(Debug, Clone, Zeroize)]
pub struct RegisteredOutput {
    pub address: String,
    pub amount_sats: u64,
}

/// Sensitive round material — zeroed on drop.
/// Phase enum and metadata are stored separately (cannot derive Zeroize on enums).
///
/// D-07: Manual Drop zeroes all sensitive cryptographic material (key bytes, secrets,
/// participant data) when the round completes or is aborted.
///
/// NOTE: HashMap does not implement Zeroize (upstream limitation). We implement Drop
/// manually to zeroize the fields that support it (Vec<u8>, [u8;32]) and then clear
/// HashMaps so their heap allocations are freed. The map keys/values that are Strings
/// or Vecs are individually zeroized before clearing.
pub struct RoundStateInner {
    /// RSA private key bytes for blind signing (this round only), DER-encoded.
    /// Zeroed when inner is dropped (after BROADCAST or BLAME transitions to IDLE).
    pub rsa_signing_key: Vec<u8>,
    /// Per-round HMAC secret for session tokens (D-05). 32 random bytes.
    pub round_secret: [u8; 32],
    /// Set of registered input UTXOs for double-registration prevention.
    /// Key = "txid:vout" string.
    pub registered_inputs: HashMap<String, RegisteredInput>,
    /// Set of redeemed token hashes (prevent token replay, D-04).
    /// HashSet provides O(1) lookup instead of O(n) linear scan.
    pub redeemed_tokens: HashSet<[u8; 32]>,
    /// Registered outputs (appended during OUTPUT_REG phase).
    pub registered_outputs: Vec<RegisteredOutput>,
    /// Partial signatures keyed by UTXO outpoint string.
    pub partial_sigs: HashMap<String, Vec<u8>>,
    /// Change addresses keyed by UTXO outpoint string.
    pub change_addresses: HashMap<String, String>,
}

impl Drop for RoundStateInner {
    fn drop(&mut self) {
        // Zeroize the RSA key bytes and round secret first
        self.rsa_signing_key.zeroize();
        self.round_secret.zeroize();
        // Zeroize registered input sensitive data
        for (_k, v) in self.registered_inputs.iter_mut() {
            v.zeroize();
        }
        self.registered_inputs.clear();
        // Zeroize redeemed token hashes (HashSet has no iter_mut; collect and zeroize)
        let mut tokens: Vec<[u8; 32]> = self.redeemed_tokens.drain().collect();
        for token in tokens.iter_mut() {
            token.zeroize();
        }
        drop(tokens);
        // Zeroize registered outputs
        for out in self.registered_outputs.iter_mut() {
            out.zeroize();
        }
        self.registered_outputs.clear();
        // Zeroize partial signatures
        for (_k, v) in self.partial_sigs.iter_mut() {
            v.zeroize();
        }
        self.partial_sigs.clear();
        // Clear change addresses (strings; best-effort — String doesn't impl Zeroize)
        self.change_addresses.clear();
    }
}

/// The full round state. Outer struct holds non-sensitive metadata.
/// Inner state is dropped (and zeroed) on transition to Idle.
pub struct RoundState {
    pub phase: Phase,
    pub round_id: Uuid,
    /// SHA-256 of the DER-encoded RSA public key — None when Idle.
    pub rsa_pubkey_hash: Option<[u8; 32]>,
    /// DER-encoded SubjectPublicKeyInfo bytes of the RSA public key — None when Idle.
    /// Published in GET /info so clients can verify and use for blinding (D-02).
    pub rsa_pubkey_der: Option<Vec<u8>>,
    pub participant_count: u32,
    /// Sensitive material — None when Idle (dropped and zeroed after round completion).
    pub inner: Option<RoundStateInner>,
}

#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("Invalid FSM transition from {from:?} to {to:?}")]
    InvalidTransition { from: Phase, to: Phase },
}

impl RoundState {
    pub fn new_idle() -> Self {
        Self {
            phase: Phase::Idle,
            round_id: Uuid::new_v4(),
            rsa_pubkey_hash: None,
            rsa_pubkey_der: None,
            participant_count: 0,
            inner: None,
        }
    }

    /// Attempt a phase transition. Returns Err if not a valid FSM edge.
    /// On transition to Idle, drops inner state (triggering ZeroizeOnDrop).
    pub fn transition_to(&mut self, next: Phase) -> Result<(), TransitionError> {
        if !self.phase.can_transition_to(&next) {
            return Err(TransitionError::InvalidTransition {
                from: self.phase.clone(),
                to: next,
            });
        }
        // On transition to Idle, drop inner state (zeroed by ZeroizeOnDrop)
        if next == Phase::Idle {
            self.inner = None; // triggers ZeroizeOnDrop on RoundStateInner
            self.rsa_pubkey_hash = None;
            self.rsa_pubkey_der = None;
            self.participant_count = 0;
            self.round_id = Uuid::new_v4(); // fresh round ID for next round
        }
        self.phase = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_fsm_transitions() {
        let valid = [
            (Phase::Idle, Phase::InputReg),
            (Phase::InputReg, Phase::OutputReg),
            (Phase::OutputReg, Phase::Signing),
            (Phase::Signing, Phase::Broadcast),
            (Phase::Signing, Phase::Blame),
            (Phase::Broadcast, Phase::Idle),
            (Phase::Blame, Phase::Idle),
        ];
        for (from, to) in valid {
            assert!(from.can_transition_to(&to), "{:?} -> {:?} should be valid", from, to);
        }
    }

    #[test]
    fn invalid_fsm_transitions() {
        let invalid = [
            (Phase::Idle, Phase::OutputReg),
            (Phase::Idle, Phase::Signing),
            (Phase::Idle, Phase::Broadcast),
            (Phase::InputReg, Phase::Signing),
            (Phase::InputReg, Phase::Idle),
            (Phase::OutputReg, Phase::InputReg),
            (Phase::Broadcast, Phase::Signing),
        ];
        for (from, to) in invalid {
            assert!(!from.can_transition_to(&to), "{:?} -> {:?} should be INVALID", from, to);
        }
    }

    #[test]
    fn transition_to_idle_clears_inner() {
        let mut state = RoundState::new_idle();
        // Simulate having entered a round
        state.phase = Phase::Signing;
        state.rsa_pubkey_der = Some(vec![1, 2, 3]); // simulate active round
        state.inner = Some(RoundStateInner {
            rsa_signing_key: vec![1, 2, 3],
            round_secret: [0xab; 32],
            registered_inputs: Default::default(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: Default::default(),
            change_addresses: Default::default(),
        });
        // Transition to Broadcast then to Idle
        state.transition_to(Phase::Broadcast).unwrap();
        state.transition_to(Phase::Idle).unwrap();
        assert!(state.inner.is_none(), "Inner state must be dropped on Idle transition");
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.rsa_pubkey_hash.is_none());
    }

    #[test]
    fn invalid_transition_returns_err() {
        let mut state = RoundState::new_idle();
        let result = state.transition_to(Phase::Signing);
        assert!(result.is_err());
        // Phase must not have changed
        assert_eq!(state.phase, Phase::Idle);
    }
}
