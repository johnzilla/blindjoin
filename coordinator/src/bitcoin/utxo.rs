use std::collections::HashSet;
use bitcoin::{OutPoint, Script, ScriptBuf, hashes::Hash};
use bitcoin::secp256k1::{Secp256k1, Message as SecpMessage, ecdsa::Signature};
use shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign, Bip322Error};
use shared::protocol::OwnershipProof;
use crate::bitcoin::rpc::{BitcoinRpc, RpcError};

#[derive(Debug, thiserror::Error)]
pub enum UtxoError {
    #[error("UTXO not found or already spent")]
    NotFound,
    #[error("UTXO already registered in this round")]
    AlreadyRegistered,
    #[error("UTXO value {value} sats insufficient (need {required} sats)")]
    InsufficientValue { value: u64, required: u64 },
    #[error("Invalid BIP-322 ownership proof: {reason}")]
    InvalidProof { reason: String },
    #[error("Bitcoin Core unreachable: {0}")]
    RpcUnavailable(String),
}

impl From<RpcError> for UtxoError {
    fn from(e: RpcError) -> Self {
        UtxoError::RpcUnavailable(e.to_string())
    }
}

pub struct UtxoDetails {
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,
}

/// Validate a UTXO registration request.
///
/// Checks in order:
/// 1. Not already registered in this round (double-registration prevention, UTXO-03)
/// 2. Exists and is unspent (via Bitcoin Core RPC, UTXO-01)
/// 3. Value >= denomination + fee_share_sats (UTXO-02)
/// 4. BIP-322 ownership proof is valid (UTXO-04)
pub async fn validate_utxo(
    rpc: &BitcoinRpc,
    utxo: &OutPoint,
    registered_inputs: &HashSet<OutPoint>,
    denomination_sats: u64,
    fee_share_sats: u64,
    ownership_proof: &OwnershipProof,
    round_id: &str,
) -> Result<UtxoDetails, UtxoError> {
    // 1. Double-registration check (T-03-02: prevents double-spend of same UTXO)
    if registered_inputs.contains(utxo) {
        return Err(UtxoError::AlreadyRegistered);
    }

    // 2. Existence + unspent check (UTXO-01)
    let txout = rpc.gettxout(&utxo.txid, utxo.vout).await?;
    let txout = txout.ok_or(UtxoError::NotFound)?;

    // 3. Value check (UTXO-02)
    // corepc_types GetTxOut (v17/v26) has value as f64 BTC; convert to sats
    // Use bitcoin::Amount::from_btc for correct decimal-to-satoshi conversion
    // without floating-point truncation errors.
    let value_sats = bitcoin::Amount::from_btc(txout.value)
        .map_err(|e| UtxoError::InvalidProof { reason: format!("BTC amount parse: {e}") })?
        .to_sat();
    let required = denomination_sats + fee_share_sats;
    if value_sats < required {
        return Err(UtxoError::InsufficientValue { value: value_sats, required });
    }

    // 4. BIP-322 ownership proof (UTXO-04, T-03-01)
    let script_pubkey = parse_script_pubkey_from_txout(&txout)
        .map_err(|e| UtxoError::InvalidProof { reason: e })?;
    let message = format!("blindjoin:round:{}:utxo:{}:{}", round_id, utxo.txid, utxo.vout);
    verify_bip322_simple(&script_pubkey, &ownership_proof.witness_stack, &message)
        .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;

    Ok(UtxoDetails { value_sats, script_pubkey })
}

fn parse_script_pubkey_from_txout(txout: &corepc_types::v26::GetTxOut) -> Result<ScriptBuf, String> {
    // corepc_types v17/v26 GetTxOut has script_pubkey.hex field
    let hex_str = &txout.script_pubkey.hex;
    let bytes = hex::decode(hex_str).map_err(|e| format!("hex decode: {e}"))?;
    Ok(ScriptBuf::from_bytes(bytes))
}

// v1.4 Phase 15 Plan 15-02: the local Bip322Error enum at this location was
// deleted per CONTEXT D-29 / 15-PATTERNS.md "coordinator/src/bitcoin/utxo.rs:87-101"
// section. The shared::bip322::Bip322Error import at the top of this file is
// now the single source of truth for the verify_bip322_simple body's typed
// errors. Wire mapping at coordinator/src/api/handlers.rs is unchanged —
// every Bip322Error variant maps to ErrorCode::InvalidOwnershipProof per
// D-32 (no per-script-type fingerprint leak).
//
// The verify_bip322_simple body below remains in place at Phase 15 and gets
// its Err(...) returns remapped to the shared variant taxonomy. Phase 16
// ADVERT-03 swaps the call site at lines 74-75 of validate_utxo to the new
// shared::bip322::verify_simple dispatcher, at which point this whole
// verify_bip322_simple body becomes dead code and is removed.

/// BIP-322 Simple verification for P2WPKH outputs (~50 lines).
///
/// Per BIP-322 Section 4: construct a virtual to_spend transaction with:
///   - One input: nSequence=0, nVersion=0, scriptSig=OP_0 <sha256(message)>
///   - One output: scriptPubKey=<the UTXO's scriptPubKey>, value=0
///
/// Per BIP-322 Section 5: the to_sign transaction spends the to_spend output.
/// For P2WPKH: the witness is [sig, pubkey]. Verify the ECDSA signature over
/// the to_sign sighash.
///
/// Only P2WPKH is required for Phase 1. (D-04)
pub fn verify_bip322_simple(
    script_pubkey: &Script,
    witness_stack: &[Vec<u8>],
    message: &str,
) -> Result<(), Bip322Error> {
    if !script_pubkey.is_p2wpkh() {
        return Err(Bip322Error::UnsupportedScriptType);
    }

    if witness_stack.len() != 2 {
        return Err(Bip322Error::InvalidWitnessLength {
            expected: 2,
            got: witness_stack.len(),
        });
    }

    let sig_bytes = &witness_stack[0];
    let pubkey_bytes = &witness_stack[1];

    // 1. Construct the BIP-322 message hash (tagged hash)
    let msg_hash = bip322_message_hash(message.as_bytes());

    // 2. Construct the to_spend transaction per BIP-322 Section 4
    let to_spend = build_bip322_to_spend(script_pubkey, &msg_hash);

    // 3. Construct the to_sign transaction per BIP-322 Section 5
    let to_sign = build_bip322_to_sign(&to_spend);

    // 4. Compute the P2WPKH sighash for the to_sign input
    use bitcoin::sighash::{SighashCache, EcdsaSighashType};
    use bitcoin::Amount;
    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache.p2wpkh_signature_hash(
        0,  // input index in to_sign
        script_pubkey,
        Amount::from_sat(0),  // to_spend output value is 0 per spec
        EcdsaSighashType::All,
    ).map_err(|e| Bip322Error::DecodeError(format!("p2wpkh sighash: {e}")))?;

    // 5. Verify ECDSA signature
    let secp = Secp256k1::verification_only();
    let secp_msg = SecpMessage::from_digest(sighash.to_byte_array());

    // sig_bytes may include the sighash type byte at the end — strip it
    let sig_der = if sig_bytes.last().copied() == Some(0x01) {
        &sig_bytes[..sig_bytes.len() - 1]
    } else {
        sig_bytes.as_slice()
    };
    let sig = Signature::from_der(sig_der)
        .map_err(|e| Bip322Error::DecodeError(format!("ecdsa signature parse: {e}")))?;
    let pubkey = bitcoin::secp256k1::PublicKey::from_slice(pubkey_bytes)
        .map_err(|e| Bip322Error::DecodeError(format!("pubkey parse: {e}")))?;
    secp.verify_ecdsa(&secp_msg, &sig, &pubkey)
        .map_err(|e| Bip322Error::DecodeError(format!("ecdsa verify: {e}")))?;

    // 6. Verify pubkey matches the script_pubkey (hash160 check for P2WPKH)
    let compressed = bitcoin::PublicKey::new(pubkey);
    let wpkh = compressed.wpubkey_hash()
        .map_err(|e| Bip322Error::DecodeError(format!("wpubkey_hash: {e}")))?;
    let expected_wpkh = ScriptBuf::new_p2wpkh(&wpkh);
    if expected_wpkh != *script_pubkey {
        return Err(Bip322Error::ScriptMismatch);
    }

    Ok(())
}

// bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign are now
// provided by shared::bip322 (imported at the top of this file).

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey as SecpSecretKey};
    use bitcoin::sighash::{SighashCache, EcdsaSighashType};
    use bitcoin::{Amount, PublicKey};

    fn make_p2wpkh_and_witness(message: &str) -> (ScriptBuf, Vec<Vec<u8>>) {
        let secp = Secp256k1::new();
        let secret_key = SecpSecretKey::from_slice(&[0x01_u8; 32]).unwrap();
        let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
        let compressed = PublicKey::new(pubkey);
        let script_pubkey = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().unwrap());

        // Build the BIP-322 transactions
        let msg_hash = bip322_message_hash(message.as_bytes());
        let to_spend = build_bip322_to_spend(&script_pubkey, &msg_hash);
        let to_sign = build_bip322_to_sign(&to_spend);

        // Sign
        let mut cache = SighashCache::new(&to_sign);
        let sighash = cache.p2wpkh_signature_hash(0, &script_pubkey, Amount::ZERO, EcdsaSighashType::All).unwrap();
        let secp_msg = bitcoin::secp256k1::Message::from_digest(sighash.to_byte_array());
        let sig = secp.sign_ecdsa(&secp_msg, &secret_key);
        let mut sig_bytes = sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL

        let witness_stack = vec![sig_bytes, pubkey.serialize().to_vec()];
        (script_pubkey, witness_stack)
    }

    #[test]
    fn bip322_valid_p2wpkh() {
        let msg = "blindjoin:round:test-round-id:utxo:abc:0";
        let (script, witness) = make_p2wpkh_and_witness(msg);
        assert!(verify_bip322_simple(&script, &witness, msg).is_ok());
    }

    #[test]
    fn bip322_wrong_witness_length() {
        let msg = "blindjoin:round:test:utxo:abc:0";
        let (script, _witness) = make_p2wpkh_and_witness(msg);
        let result = verify_bip322_simple(&script, &[vec![0x01]], msg);
        assert!(matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 2, got: 1 })
        ));
    }

    #[test]
    fn bip322_wrong_message_fails() {
        let msg = "blindjoin:round:test:utxo:abc:0";
        let wrong_msg = "blindjoin:round:test:utxo:abc:1";
        let (script, witness) = make_p2wpkh_and_witness(msg);
        // Witness was signed for `msg`, verifying against `wrong_msg` must fail
        let result = verify_bip322_simple(&script, &witness, wrong_msg);
        assert!(result.is_err());
    }
}
