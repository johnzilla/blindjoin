use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use blind_rsa_signatures::{
    BlindSignature, DefaultRng,
    Sha384, PSS, Randomized,
};
use bitcoin::hashes::Hash;
use shared::token::compute_blind_token_message;
use shared::protocol::{InputRegRequest, InfoResponse};
use crate::wallet::ClientWallet;
use crate::http::CoordinatorClient;
use super::InputRegState;

/// Type alias matching the coordinator's parameter set (SHA-384, PSS, Randomized).
type BjPublicKey = blind_rsa_signatures::PublicKey<Sha384, PSS, Randomized>;

pub async fn register_input(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    info: &InfoResponse,
) -> Result<InputRegState> {
    // 1. Decode and verify coordinator RSA public key (T-05-01 mitigation: D-02)
    let pk_der_b64 = info.rsa_pubkey_der_b64.as_ref()
        .ok_or_else(|| anyhow!("Coordinator did not provide RSA public key in /info"))?;
    let pk_der = B64.decode(pk_der_b64)?;

    // Verify SHA-256(pk_der) == announced rsa_pubkey_hash
    let pk_hash_actual: [u8; 32] = {
        use sha2::{Sha256, Digest};
        Sha256::digest(&pk_der).into()
    };
    let announced_hash = info.rsa_pubkey_hash.as_ref()
        .ok_or_else(|| anyhow!("Coordinator did not announce RSA key hash"))?;
    let announced_bytes = hex::decode(announced_hash)?;
    if announced_bytes != pk_hash_actual {
        return Err(anyhow!("RSA public key hash mismatch — coordinator key commitment violated"));
    }

    let pk = BjPublicKey::from_der(&pk_der)
        .map_err(|e| anyhow!("Failed to parse coordinator RSA public key: {e}"))?;

    // 2. Compute blind token message M = compute_blind_token_message(output_script, denomination)
    let output_address = wallet.coinjoin_output_address();
    let output_script = output_address.script_pubkey();
    let denomination = info.denomination_sats;
    let message_bytes = compute_blind_token_message(&output_script, denomination);

    // 3. Blind the message (D-03) using DefaultRng from blind-rsa-signatures
    let blinding_result = pk.blind(&mut DefaultRng, &message_bytes)
        .map_err(|e| anyhow!("Blinding failed: {e}"))?;

    // 4. Generate BIP-322 ownership proof for the UTXO
    let round_id_str = info.round_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let bip322_message = format!(
        "blindjoin:round:{}:utxo:{}:{}",
        round_id_str,
        wallet.utxo_outpoint.txid,
        wallet.utxo_outpoint.vout,
    );
    let witness_stack = generate_bip322_witness(wallet, &bip322_message)?;
    let ownership_proof_obj = shared::protocol::OwnershipProof { witness_stack };
    let ownership_proof = ownership_proof_obj.to_json_hex_str();

    // 5. POST /round/input
    let req = InputRegRequest {
        utxo_outpoint: format!("{}:{}", wallet.utxo_outpoint.txid, wallet.utxo_outpoint.vout),
        ownership_proof,
        blinded_token: B64.encode::<&[u8]>(blinding_result.blind_message.as_ref()),
        change_address: wallet.change_address().to_string(),
    };
    let resp = client.post_input(req).await?;

    // 6. Decode and verify blind signature — unblind immediately to catch bad sigs early
    let blind_sig_bytes = B64.decode(&resp.blind_signature)?;
    let blind_sig = BlindSignature(blind_sig_bytes);

    // finalize() also internally verifies the unblinded signature
    let sig = pk.finalize(&blind_sig, &blinding_result, &message_bytes)
        .map_err(|e| anyhow!("Unblinding/finalization failed — blind signature invalid: {e}"))?;

    let session_token = B64.decode(&resp.session_token)?;

    Ok(InputRegState {
        round_id: resp.round_id,
        session_token,
        blinding_secret: blinding_result.secret,
        msg_randomizer: blinding_result.msg_randomizer,
        message_bytes,
        output_script,
        unblinded_sig: sig,
    })
}

/// Generate a BIP-322 Simple witness stack for our P2WPKH UTXO.
///
/// Returns the raw witness stack (not yet hex-encoded) — caller wraps in OwnershipProof.
/// Duplicates the BIP-322 logic from coordinator/src/bitcoin/utxo.rs for Phase 1.
/// In Phase 3, these helpers move to shared/.
fn generate_bip322_witness(wallet: &ClientWallet, message: &str) -> Result<Vec<Vec<u8>>> {
    use bitcoin::sighash::{SighashCache, EcdsaSighashType};
    use bitcoin::secp256k1::{Secp256k1, Message};
    use bitcoin::Amount;

    let secp = Secp256k1::new();
    let script_pubkey = wallet.script_pubkey();
    let sk = wallet.secret_key_for_signing();

    // BIP-322 message hash (tagged hash per spec)
    let msg_hash = bip322_message_hash(message.as_bytes());

    // Build to_spend and to_sign transactions per BIP-322
    let to_spend = build_bip322_to_spend(&script_pubkey, &msg_hash);
    let to_sign = build_bip322_to_sign(&to_spend);

    // Sign the to_sign sighash
    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache.p2wpkh_signature_hash(
        0,
        &script_pubkey,
        Amount::ZERO,
        EcdsaSighashType::All,
    )?;
    let secp_msg = Message::from_digest(sighash.to_byte_array());
    let sig = secp.sign_ecdsa(&secp_msg, &sk);
    let mut sig_bytes = sig.serialize_der().to_vec();
    sig_bytes.push(0x01); // SIGHASH_ALL

    // Compressed public key bytes
    let raw_pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let pubkey_bytes = raw_pk.serialize().to_vec();

    Ok(vec![sig_bytes, pubkey_bytes])
}

/// BIP-322 tagged message hash (identical to coordinator's bip322_message_hash).
fn bip322_message_hash(message: &[u8]) -> [u8; 32] {
    use bitcoin::hashes::{sha256, HashEngine, Hash};
    // BIP-322 uses double-SHA256 of "BIP0322-signed-message" tag + message
    let tag = b"BIP0322-signed-message";
    let tag_hash: [u8; 32] = sha256::Hash::hash(tag).to_byte_array();

    let mut engine = sha256::Hash::engine();
    engine.input(&tag_hash);
    engine.input(&tag_hash);
    engine.input(message);
    sha256::Hash::from_engine(engine).to_byte_array()
}

/// Build BIP-322 to_spend transaction (Section 4).
fn build_bip322_to_spend(script_pubkey: &bitcoin::Script, msg_hash: &[u8; 32]) -> bitcoin::Transaction {
    use bitcoin::{Transaction, TxIn, TxOut, Sequence, Witness, Amount, OutPoint};
    let script_sig = bitcoin::blockdata::script::Builder::new()
        .push_opcode(bitcoin::opcodes::OP_0)
        .push_slice(msg_hash)
        .into_script();
    Transaction {
        version: bitcoin::transaction::Version(0),
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig,
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::ZERO,
            script_pubkey: script_pubkey.to_owned(),
        }],
    }
}

/// Build BIP-322 to_sign transaction (Section 5).
fn build_bip322_to_sign(to_spend: &bitcoin::Transaction) -> bitcoin::Transaction {
    use bitcoin::{Transaction, TxIn, TxOut, Sequence, Witness, Amount, ScriptBuf, OutPoint};
    let to_spend_txid = to_spend.compute_txid();
    Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(to_spend_txid, 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::new_op_return(&[]),
        }],
    }
}
