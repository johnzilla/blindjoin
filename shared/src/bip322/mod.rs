//! BIP-322 Simple message signing and verification primitives.
//!
//! This module is the shared/ crate's single source of truth for BIP-322
//! multi-script verification (V1.4-MOD-07). The public surface is
//! **dispatcher-only** per Phase 15 CONTEXT D-27 — callers go through
//! [`verify_simple`] / [`sign_simple`] / [`detect_script_type`], and the
//! per-script implementation files (`p2wpkh`, `p2tr`, `p2sh_p2wpkh`) are
//! crate-private. This makes the V1.4-CRIT-01 spoofing vector statically
//! unreachable: a caller cannot bypass dispatch because no per-script
//! `pub fn` exists.
//!
//! v1.4 Phase 15 Plan 15-02:
//! - Splits the prior flat `shared/src/bip322.rs` into the four-file
//!   directory module per D-04.
//! - Ports the 26-LOC `bip322 = "=0.0.10"` crate adapter from
//!   `sprint-0-A.md:145-175` verbatim as the crate-private
//!   [`verify_via_bip322_crate`] helper (D-26).
//! - Replaces the prior stub `ScriptType` with the full dispatcher + 10-variant
//!   [`Bip322Error`] taxonomy per D-31.
//!
//! The script-NEUTRAL primitives ([`bip322_message_hash`],
//! [`build_bip322_to_spend`], [`build_bip322_to_sign`]) are carried over from
//! the flat file unchanged so the wire-format anchors stay byte-identical
//! across the module split.

use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::{
    Amount, Network, OutPoint, Script, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use serde::{Deserialize, Serialize};

mod p2sh_p2wpkh;
mod p2tr;
mod p2wpkh;

// ---------------------------------------------------------------------------
// Script-type-NEUTRAL primitives (V1.4-MOD-07 single source of truth).
// Carried over verbatim from the prior flat `shared/src/bip322.rs`.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ScriptType — carried over from the flat file's Plan 15-01 stub.
// Wire form (per ADVERT-02 + RESEARCH Open Question #3 RESOLVED):
// - `ScriptType::P2wpkh`     → `"p2wpkh"`
// - `ScriptType::P2tr`       → `"p2tr"`
// - `ScriptType::P2shP2wpkh` → `"p2sh-p2wpkh"` (kebab-case, explicit rename)
// ---------------------------------------------------------------------------

/// Script type tag carried in the v1.4 `OwnershipProof` wire envelope and used
/// by the [`verify_simple`] / [`sign_simple`] dispatchers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    P2wpkh,
    P2tr,
    #[serde(rename = "p2sh-p2wpkh")]
    P2shP2wpkh,
}

// ---------------------------------------------------------------------------
// Bip322Error — 10-variant taxonomy per Phase 15 CONTEXT D-31.
// Each variant maps 1:1 to a specific D-13 / D-34 test case. PII-safe by
// construction: every variant's Display interpolates only enum payload
// metadata (u8 version, usize lengths, ScriptType, bitcoin::Network) — no
// outpoint, address, key bytes, or amount appears in any error message.
// ---------------------------------------------------------------------------

/// Unified BIP-322 verify/sign error type returned by the dispatcher and the
/// per-script verifiers.
#[derive(Debug, thiserror::Error)]
pub enum Bip322Error {
    #[error("unsupported OwnershipProof version: {0}")]
    UnsupportedProofVersion(u8),
    #[error("wire-format mismatch: {0}")]
    WireFormatMismatch(String),
    #[error("PSBT/base64 decode error: {0}")]
    DecodeError(String),
    #[error("script_pubkey is not a recognised single-key address (P2WPKH / P2TR / P2SH-P2WPKH)")]
    UnrecognisedScriptPubkey {
        #[source]
        source: bitcoin::address::FromScriptError,
    },
    #[error("unsupported script type")]
    UnsupportedScriptType,
    #[error("declared script_type {declared:?} does not match on-chain {derived:?}")]
    ScriptTypeMismatch {
        declared: ScriptType,
        derived: ScriptType,
    },
    #[error("invalid witness length: expected {expected}, got {got}")]
    InvalidWitnessLength { expected: usize, got: usize },
    #[error("BIP-322 crate verification failed")]
    CrateVerifyFailed {
        #[source]
        source: bip322::Error,
    },
    #[error("network mismatch: address decoded for {decoded:?}, configured for {configured:?}")]
    NetworkMismatch {
        decoded: Network,
        configured: Network,
    },
    #[error("legacy v1 script mismatch")]
    ScriptMismatch,
}

// ---------------------------------------------------------------------------
// Dispatcher — public surface per D-27. Per-script verifier and signer fns
// are `pub(crate)` only. A caller cannot reach `p2wpkh::verify` from outside
// the crate; the only route is through `verify_simple` / `sign_simple`.
// ---------------------------------------------------------------------------

/// Detect the script type of an on-chain script_pubkey.
///
/// Routes via `Script::is_p2wpkh` / `is_p2tr` / `is_p2sh` with NO fallthrough
/// default arm. Unknown shapes return [`Bip322Error::UnsupportedScriptType`].
///
/// NOTE: `is_p2sh()` alone cannot distinguish P2SH-P2WPKH from raw
/// P2SH-multisig — the on-chain SPK is only the HASH160 of the redeem.
/// `detect_script_type` optimistically returns [`ScriptType::P2shP2wpkh`]
/// for any P2SH SPK; the per-script verifier in `p2sh_p2wpkh.rs` delegates
/// to the bip322 crate which performs the HASH160 cross-check internally
/// (`verify.rs:167-169`), so non-P2WPKH-wrapped P2SH scripts reject at
/// verify time with [`Bip322Error::CrateVerifyFailed`].
pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error> {
    if spk.is_p2wpkh() {
        Ok(ScriptType::P2wpkh)
    } else if spk.is_p2tr() {
        Ok(ScriptType::P2tr)
    } else if spk.is_p2sh() {
        Ok(ScriptType::P2shP2wpkh)
    } else {
        Err(Bip322Error::UnsupportedScriptType)
    }
}

/// Verify a BIP-322 Simple proof for the given script type.
///
/// Routes to the per-script verifier; per-script verifiers perform arity
/// pre-flight then delegate to [`verify_via_bip322_crate`]. The
/// `bip322 = "=0.0.10"` crate's `verify_simple` handles BIP-143 (P2WPKH,
/// P2SH-P2WPKH), BIP-341 (P2TR keyspend), and the 64-byte / 65-byte Schnorr
/// branching internally per BIP322-02.
pub fn verify_simple(
    script_type: ScriptType,
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), Bip322Error> {
    match script_type {
        ScriptType::P2wpkh => p2wpkh::verify(spk, witness, message, network),
        ScriptType::P2tr => p2tr::verify(spk, witness, message, network),
        ScriptType::P2shP2wpkh => p2sh_p2wpkh::verify(spk, witness, message, network),
    }
}

/// Sign a BIP-322 Simple proof for the given script type.
///
/// Per CD-6: P2WPKH ships a full production body in Phase 15 (carried over
/// from the v1.3 path); P2TR and P2SH-P2WPKH bodies are `todo!()` and are
/// filled in by Phase 17 WALLET-02 (bdk_wallet sign path per ADR Decision #4).
pub fn sign_simple(
    script_type: ScriptType,
    spk: &Script,
    key: &bitcoin::secp256k1::SecretKey,
    message: &[u8],
) -> Result<Witness, Bip322Error> {
    match script_type {
        ScriptType::P2wpkh => p2wpkh::sign(spk, key, message),
        ScriptType::P2tr => p2tr::sign(spk, key, message),
        ScriptType::P2shP2wpkh => p2sh_p2wpkh::sign(spk, key, message),
    }
}

// ---------------------------------------------------------------------------
// 26-LOC bip322 crate adapter — Sprint-0-A:145-175 verbatim per D-26.
// Wraps `bip322::verify_simple(&Address, message, Witness)` into our
// `(spk, witness, message, network)` wire shape. Error mapping preserves
// the underlying `bip322::error::Error` via `#[source]` (no string collapse).
// ---------------------------------------------------------------------------

pub(crate) fn verify_via_bip322_crate(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), Bip322Error> {
    let address = bitcoin::Address::from_script(spk, network)
        .map_err(|source| Bip322Error::UnrecognisedScriptPubkey { source })?;
    bip322::verify_simple(&address, message, witness.clone())
        .map_err(|source| Bip322Error::CrateVerifyFailed { source })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::key::TapTweak;
    use bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey as SecpSecretKey, XOnlyPublicKey};
    use bitcoin::sighash::{EcdsaSighashType, SighashCache};
    use bitcoin::{PublicKey, ScriptBuf, WPubkeyHash};

    // --- pre-existing primitive determinism tests (carried over verbatim) ---

    fn make_p2wpkh_script_and_witness(message: &str) -> (ScriptBuf, Vec<Vec<u8>>) {
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
        let (script, _) = make_p2wpkh_script_and_witness(msg);
        let msg_hash = bip322_message_hash(msg.as_bytes());
        let tx1 = build_bip322_to_spend(&script, &msg_hash);
        let tx2 = build_bip322_to_spend(&script, &msg_hash);
        assert_eq!(tx1.compute_txid(), tx2.compute_txid());
    }

    // --- ScriptType wire-form tests (Plan 15-01 Task 1, lifted) ---

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
        let a = ScriptType::P2wpkh;
        let _b = a;
        let _c: ScriptType = a;
        assert_eq!(a, ScriptType::P2wpkh);
        assert_eq!(a.clone(), ScriptType::P2wpkh);
        let _dbg = format!("{:?}", a);
    }

    // --- Plan 15-02 Task 1 sanity tests for the dispatcher + Bip322Error ---

    fn fixture_secret_key() -> SecpSecretKey {
        SecpSecretKey::from_slice(&[0x42_u8; 32]).unwrap()
    }

    fn fixture_p2wpkh_spk() -> ScriptBuf {
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let compressed = PublicKey::new(pk);
        ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().unwrap())
    }

    fn fixture_p2tr_spk() -> ScriptBuf {
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let keypair = Keypair::from_secret_key(&secp, &sk);
        let (xonly, _parity) = XOnlyPublicKey::from_keypair(&keypair);
        // BIP-341 output key with empty merkle root (keyspend-only).
        let tweaked = keypair.tap_tweak(&secp, None);
        let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
        let _ = xonly; // suppress unused (sanity: untweaked key derivable too)
        ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked())
    }

    fn fixture_p2sh_spk() -> ScriptBuf {
        // Any 20-byte script hash is a syntactically valid P2SH SPK.
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let compressed = PublicKey::new(pk);
        let wpkh: WPubkeyHash = compressed.wpubkey_hash().unwrap();
        let redeem = ScriptBuf::new_p2wpkh(&wpkh);
        ScriptBuf::new_p2sh(&redeem.script_hash())
    }

    #[test]
    fn detect_script_type_returns_p2wpkh_for_p2wpkh_spk() {
        let spk = fixture_p2wpkh_spk();
        assert_eq!(detect_script_type(&spk).unwrap(), ScriptType::P2wpkh);
    }

    #[test]
    fn detect_script_type_returns_p2tr_for_p2tr_spk() {
        let spk = fixture_p2tr_spk();
        assert_eq!(detect_script_type(&spk).unwrap(), ScriptType::P2tr);
    }

    #[test]
    fn detect_script_type_returns_p2sh_p2wpkh_for_p2sh_spk() {
        let spk = fixture_p2sh_spk();
        assert_eq!(detect_script_type(&spk).unwrap(), ScriptType::P2shP2wpkh);
    }

    #[test]
    fn detect_script_type_rejects_op_return_with_unsupported_script_type() {
        let spk = ScriptBuf::new_op_return([0x01, 0x02, 0x03]);
        let err = detect_script_type(&spk).expect_err("OP_RETURN must reject");
        assert!(matches!(err, Bip322Error::UnsupportedScriptType));
    }

    #[test]
    fn bip322_error_display_is_non_empty() {
        let s = format!("{}", Bip322Error::UnsupportedProofVersion(3));
        assert!(!s.is_empty());
        // PII-safe sanity: the Display string contains the version byte and
        // nothing else — no address, outpoint, or key material is interpolated
        // anywhere in the 10-variant taxonomy by construction (D-31).
        assert!(s.contains("3"));
    }

    #[test]
    fn bip322_error_display_does_not_leak_pii_substrings() {
        // PROJECT.md no-PII-logging invariant. Spot-check each variant's
        // Display by inspection — no outpoint/address/pubkey/utxo_id is
        // interpolated. The InvalidWitnessLength variant carries usize counts
        // only; NetworkMismatch carries bitcoin::Network enum values only;
        // ScriptTypeMismatch carries ScriptType enum values only.
        let cases = [
            format!("{}", Bip322Error::UnsupportedProofVersion(2)),
            format!("{}", Bip322Error::WireFormatMismatch("foo".into())),
            format!("{}", Bip322Error::DecodeError("bar".into())),
            format!("{}", Bip322Error::UnsupportedScriptType),
            format!(
                "{}",
                Bip322Error::ScriptTypeMismatch {
                    declared: ScriptType::P2wpkh,
                    derived: ScriptType::P2tr,
                }
            ),
            format!(
                "{}",
                Bip322Error::InvalidWitnessLength { expected: 2, got: 1 }
            ),
            format!(
                "{}",
                Bip322Error::NetworkMismatch {
                    decoded: Network::Bitcoin,
                    configured: Network::Signet,
                }
            ),
            format!("{}", Bip322Error::ScriptMismatch),
        ];
        for case in &cases {
            let lower = case.to_lowercase();
            assert!(
                !lower.contains("outpoint"),
                "PII leak: 'outpoint' in {case}"
            );
            assert!(!lower.contains("utxo_id"), "PII leak: 'utxo_id' in {case}");
            assert!(
                !lower.contains("pubkey:"),
                "PII leak: 'pubkey:' in {case}"
            );
            assert!(
                !lower.contains("script_pubkey:"),
                "PII leak: 'script_pubkey:' in {case}"
            );
            // "address" is permitted in the generic phrase
            // "single-key address" of UnrecognisedScriptPubkey; we only flag
            // address followed by a colon (indicating an interpolated value).
            assert!(
                !lower.contains("address:"),
                "PII leak: 'address:' in {case}"
            );
        }
    }
}
