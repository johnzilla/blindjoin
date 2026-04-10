use bitcoin::OutPoint;
use shared::errors::{ApiError, ErrorCode};
use crate::round::state::{RegisteredInput, RoundState};
use crate::round::manager::generate_session_token;
use crate::blind::rsa::RsaBlindSigner;
use sha2::{Sha256, Digest};
use blind_rsa_signatures::BlindMessage;

/// Result of a successful input registration.
#[derive(Debug)]
pub struct InputRegResult {
    /// base64-encoded blind signature bytes
    pub blind_signature_b64: String,
    /// base64-encoded [u8;32] session token
    pub session_token_b64: String,
}

/// Core input registration logic. Called from handler with write-locked state.
///
/// Pure synchronous state mutation — no async I/O, no RPC calls.
/// The caller (post_input handler) MUST call validate_utxo() before acquiring
/// the write lock and before calling this function (AVAIL-01).
///
/// # Arguments
/// - `state`              — mutable round state (write-locked by caller)
/// - `utxo`               — the UTXO being registered (already validated by caller pre-lock)
/// - `blinded_token_bytes`— base64-decoded blinded message from client
/// - `change_address`     — bech32 change address string
/// - `round_id_str`       — current round_id as string (for error messages)
///
/// # TOCTOU
/// This function re-checks double-registration under the caller's write lock (D-02).
/// The pre-lock validation snapshot may be stale; this check is authoritative.
pub fn register_input(
    state: &mut RoundState,
    utxo: &OutPoint,
    blinded_token_bytes: &[u8],
    change_address: &str,
    round_id_str: &str,
) -> Result<InputRegResult, ApiError> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;

    let inner = state.inner.as_mut().ok_or_else(|| ApiError {
        code: ErrorCode::WrongPhase,
        message: "Round not in input registration phase".into(),
        round_id: Some(round_id_str.to_string()),
    })?;

    // TOCTOU re-check: UTXO must not be double-registered under write lock (D-02, AVAIL-01)
    // The caller (post_input) performed RPC validation before acquiring the lock.
    // This in-lock re-check is authoritative.
    let utxo_str = format!("{}:{}", utxo.txid, utxo.vout);
    if inner.registered_inputs.contains_key(&utxo_str) {
        return Err(ApiError {
            code: ErrorCode::UtxoAlreadyRegistered,
            message: "UTXO already registered in this round".into(),
            round_id: Some(round_id_str.to_string()),
        });
    }

    // Blind-sign the blinded message using the cached signer (AVAIL-02: no per-request key deserialization)
    let blind_msg = BlindMessage(blinded_token_bytes.to_vec());
    let blind_sig = inner.rsa_signer.blind_sign(&blind_msg).map_err(|e| ApiError {
        code: ErrorCode::InvalidToken,
        message: format!("Blind signing failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Generate session token for this (round_secret, utxo) pair
    let session_token = generate_session_token(&inner.round_secret, utxo);

    // Compute blind_sig_hash for double-registration detection
    let blind_sig_hash: [u8; 32] = Sha256::digest(
        <blind_rsa_signatures::BlindSignature as AsRef<[u8]>>::as_ref(&blind_sig)
    ).into();

    // Register the input
    inner.registered_inputs.insert(utxo_str.clone(), RegisteredInput {
        utxo_str: utxo_str.clone(),
        change_address: change_address.to_string(),
        blind_sig_hash,
    });
    inner.change_addresses.insert(utxo_str, change_address.to_string());
    state.participant_count += 1;

    Ok(InputRegResult {
        blind_signature_b64: b64.encode(
            <blind_rsa_signatures::BlindSignature as AsRef<[u8]>>::as_ref(&blind_sig)
        ),
        session_token_b64: b64.encode(session_token),
    })
}

/// Parse "txid:vout" string into OutPoint.
pub fn parse_outpoint(s: &str) -> Option<OutPoint> {
    let mut parts = s.rsplitn(2, ':');
    let vout: u32 = parts.next()?.parse().ok()?;
    let txid_str = parts.next()?;
    use std::str::FromStr;
    let txid = bitcoin::Txid::from_str(txid_str).ok()?;
    Some(OutPoint::new(txid, vout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::state::{Phase, RoundState, RoundStateInner};
    use crate::blind::rsa::RsaBlindSigner;
    use std::collections::HashMap;

    fn make_input_reg_state() -> (RoundState, RsaBlindSigner) {
        let signer = RsaBlindSigner::generate().unwrap();
        let sk_der = signer.secret_key_der().unwrap();
        let pk_der = signer.public_key_spki_der().unwrap();
        let pk_hash = signer.public_key_hash();
        let round_secret = [0x42u8; 32];

        let mut state = RoundState::new_idle();
        state.rsa_pubkey_hash = Some(pk_hash);
        state.rsa_pubkey_der = Some(pk_der);
        let signer_for_inner = RsaBlindSigner::from_der_secret_key(&sk_der).unwrap();
        state.inner = Some(RoundStateInner {
            rsa_signing_key: sk_der,
            rsa_signer: signer_for_inner,
            round_secret,
            registered_inputs: HashMap::new(),
            redeemed_tokens: std::collections::HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(),
            change_addresses: HashMap::new(),
        });
        state.transition_to(Phase::InputReg).unwrap();
        (state, signer)
    }

    /// AVAIL-01: register_input is synchronous — no .await, compiles as plain fn.
    /// This test verifies a successful registration completes and increments participant_count.
    #[test]
    fn register_input_is_sync_and_succeeds() {
        use bitcoin::{OutPoint, Txid};
        use std::str::FromStr;
        use blind_rsa_signatures::DefaultRng;

        let (mut state, signer) = make_input_reg_state();
        let pk = &signer.public_key;

        // Build a blinded token (client-side blinding)
        let msg = b"test-token-message-32-bytes-xxx!";
        let blinding_result = pk.blind(&mut DefaultRng, msg).unwrap();
        let blinded_bytes = <blind_rsa_signatures::BlindMessage as AsRef<[u8]>>::as_ref(
            &blinding_result.blind_message
        ).to_vec();

        let txid = Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ).unwrap();
        let utxo = OutPoint::new(txid, 0);

        // register_input is a plain fn — no .await needed (AVAIL-01)
        // It accesses inner.rsa_signer directly (AVAIL-02: no per-request key deserialization)
        let result = register_input(
            &mut state,
            &utxo,
            &blinded_bytes,
            "tb1qtest000000000000000000000000000000000000",
            "test-round-id",
        );

        assert!(result.is_ok(), "register_input should succeed: {:?}", result);
        assert_eq!(state.participant_count, 1, "participant_count must increment");
        assert!(
            state.inner.as_ref().unwrap().registered_inputs.contains_key(
                "0000000000000000000000000000000000000000000000000000000000000001:0"
            ),
            "UTXO must be recorded in registered_inputs"
        );
    }

    /// AVAIL-01: TOCTOU double-registration re-check under write lock.
    #[test]
    fn register_input_rejects_double_registration() {
        use bitcoin::{OutPoint, Txid};
        use std::str::FromStr;
        use crate::round::state::RegisteredInput;

        let (mut state, _signer) = make_input_reg_state();
        let txid = Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000002"
        ).unwrap();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:{}", txid, 0);

        // Pre-populate the UTXO as already registered (simulates another participant winning the race)
        state.inner.as_mut().unwrap().registered_inputs.insert(
            utxo_str.clone(),
            RegisteredInput {
                utxo_str: utxo_str.clone(),
                change_address: "tb1qother".to_string(),
                blind_sig_hash: [0u8; 32],
            },
        );

        // dummy bytes — will fail at blind_sign if double-reg not caught first
        let blinded_bytes = vec![0u8; 256];

        let result = register_input(
            &mut state,
            &utxo,
            &blinded_bytes,
            "tb1qdouble",
            "test-round-id",
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, shared::errors::ErrorCode::UtxoAlreadyRegistered,
            "AVAIL-01: double-registration must return UtxoAlreadyRegistered");
    }

    /// parse_outpoint regression test — keep existing behavior.
    #[test]
    fn parse_outpoint_valid() {
        let op = parse_outpoint("0000000000000000000000000000000000000000000000000000000000000001:0");
        assert!(op.is_some());
        assert_eq!(op.unwrap().vout, 0);
    }

    #[test]
    fn parse_outpoint_invalid() {
        assert!(parse_outpoint("notvalid").is_none());
        assert!(parse_outpoint("").is_none());
    }
}
