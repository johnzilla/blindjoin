//! BIP-322 Simple message signing and verification primitives.
//!
//! This module is shared between coordinator (verification) and client (signing)
//! so both sides use byte-identical transaction construction.
//!
//! Per BIP-322 Section 4/5: constructs virtual to_spend and to_sign transactions
//! for P2WPKH outputs. Only P2WPKH is required for Phase 1 (D-04).
//!
//! Moving this to shared/ ensures a single source of truth — any change to the
//! BIP-322 message format is automatically reflected on both sides.
//!
//! v1.4 Phase 15 note: This file currently contains the stub `ScriptType` enum
//! (introduced in plan 15-01) so the v2 `OwnershipProof` envelope can type-check
//! without taking a `bip322` crate dependency yet. Plan 15-02 will split this
//! flat file into the directory module `bip322/{mod,p2wpkh,p2tr,p2sh_p2wpkh}.rs`
//! and replace the stub with the full dispatcher-backed enum.

use bitcoin::{OutPoint, Script, ScriptBuf, Sequence, Witness, Amount, Transaction, TxIn, TxOut};
use bitcoin::hashes::{sha256, HashEngine, Hash};
use serde::{Deserialize, Serialize};

/// BIP-340 tagged hash for BIP-322 messages.
///
/// Format: SHA256(SHA256(tag) || SHA256(tag) || message)
/// where tag = b"BIP0322-signed-message"
pub fn bip322_message_hash(message: &[u8]) -> [u8; 32] {
    let tag = b"BIP0322-signed-message";
    let tag_hash = sha256::Hash::hash(tag);
    let mut engine = sha256::HashEngine::default();
    engine.input(tag_hash.as_ref());
    engine.input(tag_hash.as_ref());
    engine.input(message);
    sha256::Hash::from_engine(engine).to_byte_array()
}

/// Build the BIP-322 to_spend transaction (Section 4).
///
/// - nVersion = 0
/// - One input: nSequence=0, scriptSig = OP_0 <sha256(message)>, previous_output = null
/// - One output: scriptPubKey = <the UTXO's scriptPubKey>, value = 0
pub fn build_bip322_to_spend(script_pubkey: &Script, msg_hash: &[u8; 32]) -> Transaction {
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

/// Build the BIP-322 to_sign transaction (Section 5).
///
/// - nVersion = 2
/// - One input spending the to_spend output at vout=0, nSequence=0, empty scriptSig/witness
/// - One output: OP_RETURN (empty)
pub fn build_bip322_to_sign(to_spend: &Transaction) -> Transaction {
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
            script_pubkey: ScriptBuf::new_op_return([]),
        }],
    }
}

/// Script type tag carried in the v1.4 `OwnershipProof` wire envelope.
///
/// Phase 15 Plan 15-01 ships this as a STUB enum so the v2 `OwnershipProof`
/// envelope (`script_type: Option<ScriptType>`) type-checks without pulling in
/// the `bip322` crate or the per-script dispatcher (both land in Plan 15-02).
/// At this plan boundary `ScriptType` exists ONLY to satisfy the wire-format
/// roundtrip test and the serde derive; no dispatcher logic uses it yet.
///
/// Wire form (per ADVERT-02 + RESEARCH Open Question #3 RESOLVED):
/// - `ScriptType::P2wpkh` → `"p2wpkh"`
/// - `ScriptType::P2tr` → `"p2tr"`
/// - `ScriptType::P2shP2wpkh` → `"p2sh-p2wpkh"` (kebab-case, explicit rename)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    P2wpkh,
    P2tr,
    #[serde(rename = "p2sh-p2wpkh")]
    P2shP2wpkh,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{Secp256k1, SecretKey as SecpSecretKey};
    use bitcoin::sighash::{SighashCache, EcdsaSighashType};
    use bitcoin::{PublicKey};

    /// Generate a valid BIP-322 P2WPKH witness stack for testing.
    pub fn make_bip322_witness(message: &str) -> (ScriptBuf, Vec<Vec<u8>>) {
        let secp = Secp256k1::new();
        let secret_key = SecpSecretKey::from_slice(&[0x01_u8; 32]).unwrap();
        let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
        let compressed = PublicKey::new(pubkey);
        let script_pubkey = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().unwrap());

        let msg_hash = bip322_message_hash(message.as_bytes());
        let to_spend = build_bip322_to_spend(&script_pubkey, &msg_hash);
        let to_sign = build_bip322_to_sign(&to_spend);

        let mut cache = SighashCache::new(&to_sign);
        let sighash = cache
            .p2wpkh_signature_hash(0, &script_pubkey, Amount::ZERO, EcdsaSighashType::All)
            .unwrap();
        let secp_msg = bitcoin::secp256k1::Message::from_digest(*sighash.as_byte_array());
        let sig = secp.sign_ecdsa(&secp_msg, &secret_key);
        let mut sig_bytes = sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL

        let witness_stack = vec![sig_bytes, pubkey.serialize().to_vec()];
        (script_pubkey, witness_stack)
    }

    #[test]
    fn bip322_message_hash_is_deterministic() {
        let h1 = bip322_message_hash(b"test message");
        let h2 = bip322_message_hash(b"test message");
        assert_eq!(h1, h2);
    }

    #[test]
    fn bip322_message_hash_differs_for_different_messages() {
        let h1 = bip322_message_hash(b"message A");
        let h2 = bip322_message_hash(b"message B");
        assert_ne!(h1, h2);
    }

    #[test]
    fn to_spend_txid_is_deterministic() {
        let msg = "blindjoin:round:test:utxo:abc:0";
        let (script, _) = make_bip322_witness(msg);
        let msg_hash = bip322_message_hash(msg.as_bytes());
        let tx1 = build_bip322_to_spend(&script, &msg_hash);
        let tx2 = build_bip322_to_spend(&script, &msg_hash);
        assert_eq!(tx1.compute_txid(), tx2.compute_txid());
    }

    // --- ScriptType wire-form tests (Plan 15-01 Task 1) ---
    // Asserts the snake_case + kebab-case serde shape locked by RESEARCH Open
    // Question #3 RESOLVED. These run via `cargo test -p shared --lib`.

    #[test]
    fn scripttype_serializes_p2wpkh_snake_case() {
        assert_eq!(
            serde_json::to_string(&ScriptType::P2wpkh).unwrap(),
            "\"p2wpkh\"",
        );
    }

    #[test]
    fn scripttype_serializes_p2tr_snake_case() {
        assert_eq!(
            serde_json::to_string(&ScriptType::P2tr).unwrap(),
            "\"p2tr\"",
        );
    }

    #[test]
    fn scripttype_serializes_p2sh_p2wpkh_kebab_case() {
        // ADVERT-02 wire form uses kebab-case "p2sh-p2wpkh", NOT snake_case
        // "p2sh_p2wpkh" — explicit #[serde(rename = "p2sh-p2wpkh")] on the
        // variant carries this. RESEARCH Open Question #3 RESOLVED.
        assert_eq!(
            serde_json::to_string(&ScriptType::P2shP2wpkh).unwrap(),
            "\"p2sh-p2wpkh\"",
        );
    }

    #[test]
    fn scripttype_deserializes_p2sh_p2wpkh_kebab_case() {
        let decoded: ScriptType = serde_json::from_str("\"p2sh-p2wpkh\"").unwrap();
        assert_eq!(decoded, ScriptType::P2shP2wpkh);
    }

    #[test]
    fn scripttype_derives_copy_clone_eq() {
        // Compile-time bounds check via the let pattern (asserts Copy + PartialEq).
        let a = ScriptType::P2wpkh;
        let _b = a; // Copy semantics; a still usable below
        let _c: ScriptType = a;
        assert_eq!(a, ScriptType::P2wpkh);
        // Clone must also produce an equal value.
        assert_eq!(a.clone(), ScriptType::P2wpkh);
        // Debug must be implemented.
        let _dbg = format!("{:?}", a);
    }
}
