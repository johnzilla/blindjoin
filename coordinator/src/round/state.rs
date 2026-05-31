use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use zeroize::Zeroize;
use bitcoin::ScriptBuf;
use crate::blind::rsa::RsaBlindSigner;

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
            // InputReg → Idle: quorum failure on input-reg timeout. Nobody (or too
            // few) registered, so no participant is blamed; the round simply
            // resets and start_round() re-arms a fresh one. This edge is required
            // by the continuous-rounds policy (see coordinator/src/run.rs).
            | (Phase::InputReg, Phase::Idle)
            | (Phase::OutputReg, Phase::Signing)
            | (Phase::OutputReg, Phase::Blame)  // BLAME-02: missing output → blame from OutputReg
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
    /// On-chain script_pubkey of this UTXO, as returned by Bitcoin Core gettxout
    /// during validate_utxo. Used by signing.rs to populate the correct witness_utxo
    /// in the PSBT — clients no longer need to overwrite it locally with unverified values.
    /// Public chain data, no privacy concern, skipped from zeroize.
    #[zeroize(skip)]
    pub script_pubkey: ScriptBuf,
    /// On-chain value of this UTXO in satoshis, as returned by gettxout.
    pub value_sats: u64,
    /// Coordinator-derived script type (FEE-02 plumbing). Mirrors `script_pubkey`
    /// in provenance and zeroize policy: public chain data, derivable from
    /// `script_pubkey`, no key material, no privacy concern.
    #[zeroize(skip)]
    pub script_type: shared::bip322::ScriptType,
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
    /// D-07: raw bytes kept alongside parsed signer so zeroize-on-drop still fires.
    pub rsa_signing_key: Vec<u8>,
    /// Parsed RSA blind signer wrapped in `Option` per AUDIT-03 — `Some(_)` during
    /// an active round, `None` when Idle. The bounded lifetime (the Option is set
    /// to `None` at `state.rs:195` inside `transition_to(Phase::Idle)`) is the
    /// structural mitigation for the RUSTSEC-2023-0071 Marvin Attack timing-
    /// sidechannel exposure (long-lived-key + unlimited-measurements preconditions
    /// do not obtain when the key is per-round and dropped at the FSM chokepoint).
    /// See `coordinator/src/blind/rsa.rs::RoundSecretKey` for the full Drop chain
    /// that transitively reaches `rsa::RsaPrivateKey::drop`
    /// (`rsa-0.9.10/src/key.rs:76-82`).
    ///
    /// Constructed in production by `round::manager::start_round`. Tests may construct
    /// directly via the public field; they should prefer `start_round` where possible
    /// to keep production and test bootstrap aligned.
    pub rsa_signer: Option<RsaBlindSigner>,
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
            (Phase::InputReg, Phase::Idle),  // quorum-failure abort (continuous rounds)
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
            (Phase::OutputReg, Phase::InputReg),
            (Phase::Broadcast, Phase::Signing),
        ];
        for (from, to) in invalid {
            assert!(!from.can_transition_to(&to), "{:?} -> {:?} should be INVALID", from, to);
        }
    }

    /// InputReg → Idle is a valid abort edge (continuous-rounds policy: when InputReg
    /// times out below quorum, the round resets without blame and a fresh one starts).
    /// Verifies the transition also clears inner state and assigns a fresh round_id.
    #[test]
    fn input_reg_to_idle_resets_round() {
        let mut state = RoundState::new_idle();
        state.transition_to(Phase::InputReg).unwrap();
        let original_round_id = state.round_id;
        state.participant_count = 0; // quorum failure scenario
        state.transition_to(Phase::Idle).expect("InputReg → Idle must be permitted");
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.inner.is_none());
        assert_ne!(state.round_id, original_round_id, "Idle transition must assign fresh round_id");
        assert_eq!(state.participant_count, 0);
    }

    /// PRIV-01: Verify inner is None (dropped + zeroed) after Broadcast→Idle transition.
    /// Drop impl on RoundStateInner calls .zeroize() on all sensitive fields before clearing.
    /// This is the correctness assertion confirming memory zeroing runs when round completes.
    #[test]
    fn transition_to_idle_clears_inner() {
        use crate::blind::rsa::RsaBlindSigner;
        let mut state = RoundState::new_idle();
        // Simulate having entered a round
        state.phase = Phase::Signing;
        state.rsa_pubkey_der = Some(vec![1, 2, 3]); // simulate active round
        state.inner = Some(RoundStateInner {
            rsa_signing_key: vec![0xAA; 32],
            rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
            round_secret: [0xBB; 32],
            registered_inputs: Default::default(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: Default::default(),
            change_addresses: Default::default(),
        });
        // Transition to Broadcast then to Idle
        state.transition_to(Phase::Broadcast).unwrap();
        state.transition_to(Phase::Idle).unwrap();
        // PRIV-01: inner MUST be None after Idle transition (Drop was called → zeroize ran)
        assert!(state.inner.is_none(), "PRIV-01: RoundStateInner must be dropped on Idle transition");
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.rsa_pubkey_hash.is_none());
        assert!(state.rsa_pubkey_der.is_none());
    }

    /// AUDIT-03 (D-131): structural FSM test — load-bearing claim per REQUIREMENTS
    /// AUDIT-03 ("the structural lifetime bound is the load-bearing claim"). Mirrors
    /// `transition_to_idle_clears_inner` above with an additional pre-transition
    /// assertion that `rsa_signer` is `Some(_)` — guaranteeing the Drop chain on
    /// the Idle transition fires on a non-None `RoundSecretKey` (which transitively
    /// zeroizes the wrapped `rsa::RsaPrivateKey` at `rsa-0.9.10/src/key.rs:76-82`).
    ///
    /// The sibling best-effort scrub test
    /// `coordinator::blind::rsa::tests::round_secret_key_buffer_overwritten_on_drop`
    /// is a sanity check that may be ignored on non-Linux platforms; THIS test is
    /// the unconditional CI gate.
    #[test]
    fn round_secret_key_dropped_on_round_end() {
        use crate::blind::rsa::RsaBlindSigner;
        let mut state = RoundState::new_idle();
        state.phase = Phase::Signing;
        state.rsa_pubkey_der = Some(vec![1, 2, 3]);
        state.inner = Some(RoundStateInner {
            rsa_signing_key: vec![0xAA; 32],
            rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
            round_secret: [0xBB; 32],
            registered_inputs: Default::default(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: Default::default(),
            change_addresses: Default::default(),
        });
        // Pre-transition: rsa_signer is Some, so the Drop chain has a target.
        assert!(
            state.inner.as_ref().unwrap().rsa_signer.is_some(),
            "fixture must construct with Some(RsaBlindSigner)"
        );

        // Drive the FSM through Signing → Broadcast → Idle (the success path).
        state.transition_to(Phase::Broadcast).unwrap();
        state.transition_to(Phase::Idle).unwrap();

        // AUDIT-03: inner MUST be None — Drop chain has fired, RoundSecretKey
        // dropped, rsa::RsaPrivateKey zeroized transitively
        // (rsa-0.9.10/src/key.rs:76-82).
        assert!(
            state.inner.is_none(),
            "AUDIT-03: RoundStateInner must be dropped on Idle transition"
        );
        assert_eq!(state.phase, Phase::Idle);
    }

    #[test]
    fn invalid_transition_returns_err() {
        let mut state = RoundState::new_idle();
        let result = state.transition_to(Phase::Signing);
        assert!(result.is_err());
        // Phase must not have changed
        assert_eq!(state.phase, Phase::Idle);
    }

    /// AVAIL-02: Verify rsa_signer in RoundStateInner is consistent with rsa_signing_key bytes.
    /// If these disagree, a signer-key mismatch would cause all blind signature verifications
    /// to fail — catching this at test time prevents a silent regression.
    #[test]
    fn rsa_signer_consistent_with_key_bytes() {
        use crate::blind::rsa::RsaBlindSigner;

        let signer = RsaBlindSigner::generate().unwrap();
        let sk_der = signer.secret_key_der().unwrap();
        let expected_hash = signer.public_key_hash();

        // Simulate what round creation stores: signer moved into inner, raw bytes also stored
        let inner = RoundStateInner {
            rsa_signing_key: sk_der.clone(),
            rsa_signer: Some(signer),
            round_secret: [0u8; 32],
            registered_inputs: Default::default(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: Default::default(),
            change_addresses: Default::default(),
        };

        // The cached signer's public key hash must match what was generated
        assert_eq!(
            inner.rsa_signer.as_ref().expect("test fixture: rsa_signer is Some").public_key_hash(),
            expected_hash,
            "AVAIL-02: cached rsa_signer must match the generated key");

        // The raw key bytes must round-trip to the same public key hash
        let reconstructed = RsaBlindSigner::from_der_secret_key(&inner.rsa_signing_key).unwrap();
        assert_eq!(reconstructed.public_key_hash(), expected_hash,
            "AVAIL-02: raw rsa_signing_key bytes must decode to same key");
    }
}
