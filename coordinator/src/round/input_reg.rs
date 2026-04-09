use std::collections::HashSet;
use bitcoin::OutPoint;
use shared::errors::{ApiError, ErrorCode};
use crate::blind::rsa::RsaBlindSigner;
use crate::bitcoin::rpc::BitcoinRpc;
use crate::bitcoin::utxo::validate_utxo;
use crate::round::state::{RegisteredInput, RoundState};
use crate::round::manager::generate_session_token;
use sha2::{Sha256, Digest};
use blind_rsa_signatures::BlindMessage;

/// Result of a successful input registration.
pub struct InputRegResult {
    /// base64-encoded blind signature bytes
    pub blind_signature_b64: String,
    /// base64-encoded [u8;32] session token
    pub session_token_b64: String,
}

/// Core input registration logic. Called from handler with write-locked state.
///
/// Validates the UTXO, blind-signs the blinded_token, generates session token,
/// and records the registration in round state.
///
/// # Arguments
/// - `state`          — mutable round state (write-locked by caller)
/// - `signer`         — RSA blind signer for this round
/// - `rpc`            — Bitcoin Core RPC client
/// - `utxo`           — the UTXO being registered
/// - `ownership_proof_json` — canonical JSON-array-of-hex-strings ownership proof
/// - `blinded_token_bytes`  — base64-decoded blinded message from client
/// - `change_address` — bech32 change address string
/// - `denomination_sats`    — configured denomination
/// - `fee_rate_sat_per_vbyte` — configured fee rate
/// - `max_participants`     — configured max participants (used for conservative fee estimate)
/// - `round_id_str`   — current round_id as string (for BIP-322 message)
pub async fn register_input(
    state: &mut RoundState,
    signer: &RsaBlindSigner,
    rpc: &BitcoinRpc,
    utxo: &OutPoint,
    ownership_proof_json: &str,
    blinded_token_bytes: &[u8],
    change_address: &str,
    denomination_sats: u64,
    fee_rate_sat_per_vbyte: u64,
    max_participants: u32,
    round_id_str: &str,
) -> Result<InputRegResult, ApiError> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;

    let inner = state.inner.as_mut().ok_or_else(|| ApiError {
        code: ErrorCode::WrongPhase,
        message: "Round not in input registration phase".into(),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Build the set of already-registered UTXOs for double-reg check
    let registered_set: HashSet<OutPoint> = inner.registered_inputs.keys()
        .filter_map(|s| parse_outpoint(s))
        .collect();

    // Decode ownership proof
    let ownership_proof = shared::protocol::OwnershipProof::from_json_hex_str(ownership_proof_json)
        .map_err(|e| ApiError {
            code: ErrorCode::InvalidOwnershipProof,
            message: e,
            round_id: Some(round_id_str.to_string()),
        })?;

    // Validate UTXO (existence, value, double-reg, BIP-322)
    // Use max_participants for a conservative (worst-case) fee estimate so that
    // UTXOs accepted at registration time always cover the fee at signing time,
    // even if additional participants join later (WR-06).
    // Integer division remainder is absorbed as extra miner fee — documented behaviour.
    let fee_share = estimate_fee_share(max_participants, fee_rate_sat_per_vbyte);
    let _utxo_details = validate_utxo(
        rpc,
        utxo,
        &registered_set,
        denomination_sats,
        fee_share,
        &ownership_proof,
        round_id_str,
    ).await.map_err(|e| {
        use crate::bitcoin::utxo::UtxoError;
        match e {
            UtxoError::NotFound => ApiError {
                code: ErrorCode::UtxoNotFound,
                message: "UTXO not found or already spent".into(),
                round_id: Some(round_id_str.to_string()),
            },
            UtxoError::AlreadyRegistered => ApiError {
                code: ErrorCode::UtxoAlreadyRegistered,
                message: "UTXO already registered in this round".into(),
                round_id: Some(round_id_str.to_string()),
            },
            UtxoError::InsufficientValue { value: _, required } => ApiError {
                code: ErrorCode::UtxoInsufficientValue,
                message: format!("UTXO value below required threshold of {required} sats"),
                round_id: Some(round_id_str.to_string()),
            },
            UtxoError::InvalidProof { reason: _ } => ApiError {
                code: ErrorCode::InvalidOwnershipProof,
                // Do not forward reason — it may contain the UTXO outpoint (PRIV-02).
                // The coordinator logs nothing here; the client receives only this generic message.
                message: "BIP-322 ownership proof verification failed".into(),
                round_id: Some(round_id_str.to_string()),
            },
            UtxoError::RpcUnavailable(msg) => ApiError {
                code: ErrorCode::RpcUnavailable,
                message: msg,
                round_id: Some(round_id_str.to_string()),
            },
        }
    })?;

    // Blind-sign the blinded message
    let blind_msg = BlindMessage(blinded_token_bytes.to_vec());
    let blind_sig = signer.blind_sign(&blind_msg).map_err(|e| ApiError {
        code: ErrorCode::InvalidToken,
        message: format!("Blind signing failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Generate session token for this (round_secret, utxo) pair
    let session_token = generate_session_token(&inner.round_secret, utxo);

    // Compute blind_sig_hash for double-registration detection
    let blind_sig_hash: [u8; 32] = Sha256::digest(<blind_rsa_signatures::BlindSignature as AsRef<[u8]>>::as_ref(&blind_sig)).into();

    // Register the input
    let utxo_str = format!("{}:{}", utxo.txid, utxo.vout);
    inner.registered_inputs.insert(utxo_str.clone(), RegisteredInput {
        utxo_str: utxo_str.clone(),
        change_address: change_address.to_string(),
        blind_sig_hash,
    });
    inner.change_addresses.insert(utxo_str, change_address.to_string());
    state.participant_count += 1;

    Ok(InputRegResult {
        blind_signature_b64: b64.encode(<blind_rsa_signatures::BlindSignature as AsRef<[u8]>>::as_ref(&blind_sig)),
        session_token_b64: b64.encode(session_token),
    })
}

/// Estimate fee share per participant (pessimistic upper bound).
fn estimate_fee_share(n_participants: u32, fee_rate: u64) -> u64 {
    let n = n_participants as u64;
    let estimated_vsize = 10 + n * 68 + n * 2 * 31; // overhead + N inputs + 2N outputs
    let total_fee = estimated_vsize * fee_rate;
    total_fee / n
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
