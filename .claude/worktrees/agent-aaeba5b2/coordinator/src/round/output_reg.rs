use shared::errors::{ApiError, ErrorCode};
use crate::blind::rsa::BjPublicKey;
use blind_rsa_signatures::{MessageRandomizer, Signature};

/// Pure output registration logic — callable from tests without axum/state machinery.
///
/// Arguments:
///   pk             — round RSA public key (for signature verification)
///   redeemed       — mutable token replay set (appended if token accepted)
///   token_msg      — the 32-byte unblinded token message M
///   sig_bytes      — the unblinded RSA signature bytes
///   msg_randomizer — the 32-byte MessageRandomizer from BlindingResult (required for Randomized mode)
///   denomination   — configured denomination_sats
///   amount_sats    — amount_sats from the request
pub fn register_output_logic(
    pk: &BjPublicKey,
    redeemed: &mut Vec<[u8; 32]>,
    token_msg: &[u8; 32],
    sig_bytes: &[u8],
    msg_randomizer: Option<MessageRandomizer>,
    denomination: u64,
    amount_sats: u64,
) -> Result<(), ApiError> {
    // 1. Check denomination match (PROTO-03)
    if amount_sats != denomination {
        return Err(ApiError {
            code: ErrorCode::WrongDenomination,
            message: format!("Expected {denomination} sats, got {amount_sats}"),
            round_id: None,
        });
    }

    // 2. Replay check (PROTO-04)
    if redeemed.contains(token_msg) {
        return Err(ApiError {
            code: ErrorCode::TokenAlreadyUsed,
            message: "Token already redeemed".into(),
            round_id: None,
        });
    }

    // 3. RSA signature verification
    // RSABSSA-SHA384-PSS-Randomized (RFC 9474 §3.3.2): verify requires msg_randomizer.
    // The client provides this from BlindingResult.msg_randomizer in OutputRegRequest.
    let sig = Signature(sig_bytes.to_vec());
    pk.verify(&sig, msg_randomizer, token_msg.as_slice())
        .map_err(|_| ApiError {
            code: ErrorCode::InvalidToken,
            message: "Token signature verification failed".into(),
            round_id: None,
        })?;

    // 4. Mark token as redeemed
    redeemed.push(*token_msg);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blind::rsa::RsaBlindSigner;
    use shared::token::compute_blind_token_message;
    use bitcoin::ScriptBuf;

    /// Create a valid (token_msg, unblinded_sig_bytes, msg_randomizer) triple.
    /// Simulates the client blinding M, coordinator blind-signing, client unblinding.
    fn make_valid_token_sig(signer: &RsaBlindSigner, amount: u64) -> ([u8; 32], Vec<u8>, Option<MessageRandomizer>) {
        use blind_rsa_signatures::DefaultRng;

        // Build a dummy P2WPKH output script: OP_0 <20 bytes>
        let mut script_bytes = vec![0x00u8, 0x14];
        script_bytes.extend([0xab_u8; 20]);
        let script = ScriptBuf::from_bytes(script_bytes);
        let msg = compute_blind_token_message(&script, amount);

        // Client blinds msg using DefaultRng (from blind-rsa-signatures crate)
        let blinding_result = signer.public_key.blind(&mut DefaultRng, msg.as_slice()).unwrap();
        let msg_randomizer = blinding_result.msg_randomizer;

        // Coordinator blind-signs
        let blind_sig = signer.blind_sign(&blinding_result.blind_message).unwrap();

        // Client unblinds (finalize also verifies internally)
        let sig = signer.public_key
            .finalize(&blind_sig, &blinding_result, msg.as_slice())
            .unwrap();

        (msg, sig.0, msg_randomizer)
    }

    #[test]
    fn output_reg_accepts_valid_token() {
        let signer = RsaBlindSigner::generate().unwrap();
        let denom = 1_000_000u64;
        let (msg, sig, randomizer) = make_valid_token_sig(&signer, denom);
        let result = register_output_logic(
            &signer.public_key, &mut vec![], &msg, &sig, randomizer, denom, denom,
        );
        assert!(result.is_ok(), "Valid token must be accepted: {:?}", result);
    }

    #[test]
    fn output_reg_rejects_replay() {
        let signer = RsaBlindSigner::generate().unwrap();
        let denom = 1_000_000u64;
        let (msg, sig, randomizer) = make_valid_token_sig(&signer, denom);
        let mut redeemed = vec![msg];
        let result = register_output_logic(
            &signer.public_key, &mut redeemed, &msg, &sig, randomizer, denom, denom,
        );
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err().code, ErrorCode::TokenAlreadyUsed),
            "Must return TOKEN_ALREADY_USED"
        );
    }

    #[test]
    fn output_reg_rejects_wrong_denomination() {
        let signer = RsaBlindSigner::generate().unwrap();
        let denom = 1_000_000u64;
        let (msg, sig, randomizer) = make_valid_token_sig(&signer, denom);
        let result = register_output_logic(
            &signer.public_key, &mut vec![], &msg, &sig, randomizer, denom, 500_000,
        );
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err().code, ErrorCode::WrongDenomination),
            "Must return WRONG_DENOMINATION"
        );
    }

    #[test]
    fn output_reg_rejects_invalid_signature() {
        let signer = RsaBlindSigner::generate().unwrap();
        let denom = 1_000_000u64;
        let (msg, _, randomizer) = make_valid_token_sig(&signer, denom);
        let bad_sig = vec![0xde, 0xad, 0xbe, 0xef];
        let result = register_output_logic(
            &signer.public_key, &mut vec![], &msg, &bad_sig, randomizer, denom, denom,
        );
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err().code, ErrorCode::InvalidToken),
            "Must return INVALID_TOKEN"
        );
    }
}
