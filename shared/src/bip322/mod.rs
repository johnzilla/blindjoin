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
/// - nVersion = 0 (per BIP-322 spec + bip322 = "=0.0.10" crate's
///   `util::create_to_sign` at `src/util.rs:62`). This is the value the
///   crate's `verify_full_p2wpkh` / `verify_full_p2tr` reconstruct
///   internally when computing the verify sighash, so OUR sign-side
///   sighash MUST be computed against this same Version to roundtrip.
/// - One input spending the to_spend output at vout=0, nSequence=0, empty scriptSig/witness
/// - One output: OP_RETURN (empty)
///
/// History: Phase 15 Plan 15-03 Task 2 [Rule 1 — Bug] fix. The prior
/// implementation used `Version::TWO`, which produced a different sighash
/// than the bip322 crate's verify path expected — the v1.3 local
/// `coordinator::bitcoin::utxo::verify_bip322_simple` used Version::TWO
/// on BOTH sides (sign + verify), so the mismatch was masked. When the
/// 15-03 per-script sign↔verify roundtrip tests routed through the
/// crate adapter (`shared::bip322::verify_via_bip322_crate` →
/// `bip322::verify_simple` → `verify_full_p2wpkh`), the crate's internal
/// `create_to_sign` produced Version(0) and the sighashes diverged,
/// surfacing as `Bip322Error::CrateVerifyFailed { SignatureInvalid }`.
/// Aligning to Version(0) here makes our sign side match the crate's
/// verify side AND aligns to the BIP-322 spec letter. The v1.3
/// coordinator local-verify path remains consistent because both sign
/// and verify call this same `build_bip322_to_sign` helper.
pub fn build_bip322_to_sign(to_spend: &Transaction) -> Transaction {
    let to_spend_txid = to_spend.compute_txid();
    // Output script: BARE `OP_RETURN` (1 byte, opcode 0x6a).
    //
    // [Rule 1 — Bug] Phase 15 Plan 15-03 Task 2 fix: the prior
    // `ScriptBuf::new_op_return([])` produces TWO bytes (`OP_RETURN` +
    // `OP_PUSHBYTES_0`, i.e., `0x6a 0x00`) because `new_op_return` always
    // pushes its data slice — even when empty. The bip322 = "=0.0.10"
    // crate's `util::create_to_sign` at `src/util.rs:65-69` writes just
    // `OP_RETURN` alone, so OUR script_pubkey differed by the trailing
    // `0x00` byte, which propagated into the to_sign txid and the BIP-143
    // sighash. The verify path computed sighash against the bare
    // `OP_RETURN`, so signatures we produced did not validate. v1.3
    // masked this because both sides used the same wrong bytes. Aligning
    // to bare `OP_RETURN` here matches the crate AND the BIP-322 spec
    // text ("scriptPubKey OP_RETURN" with no further pushdata).
    let op_return_only = bitcoin::blockdata::script::Builder::new()
        .push_opcode(bitcoin::opcodes::all::OP_RETURN)
        .into_script();
    Transaction {
        version: bitcoin::transaction::Version(0),
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(to_spend_txid, 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::ZERO,
            script_pubkey: op_return_only,
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
    /// Script-type derivation mismatch — dual meaning per Phase 19 CONTEXT
    /// D-112 (CD-36 default = doc-comment update for discoverability):
    /// - **Verify side:** declared (CLI/wire-supplied) script type does not
    ///   match the script type derived from the on-chain `script_pubkey` via
    ///   `detect_script_type`. The original Phase 15 use (V1.4-CRIT-01
    ///   spoofing rejection).
    /// - **Sign side** (Phase 19 Plan 19-01 reuse): in `p2tr::sign` and
    ///   `p2sh_p2wpkh::sign`, the caller-supplied `script_pubkey` does not
    ///   match the derivation from the caller-supplied `SecretKey`. Here
    ///   `declared` = the script type derived from the on-chain
    ///   `script_pubkey` (via `detect_script_type`), and `derived` = the
    ///   script type the SecretKey corresponds to (per D-111 + D-112 + D-113).
    ///
    /// PII safety unchanged: Display interpolates only the two ScriptType
    /// enum values — no key, address, or pubkey bytes appear in the message.
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
        source: Box<bip322::Error>,
    },
    /// The witness public key is not the one committed by the scriptPubKey being
    /// proven (SECURITY — BIP-322 key-binding guard). `bip322 = "=0.0.10"` verifies
    /// that the witness signature is valid for the key carried IN the witness, but
    /// for P2WPKH / P2SH-P2WPKH it does not verify that key is the one the address's
    /// HASH160 commits to — so a valid signature by ANY key would otherwise "prove"
    /// ownership of anyone's UTXO. We reject a witness whose pubkey is not related to
    /// the address. Distinct from `CrateVerifyFailed` so the guard is independently
    /// observable in tests and logs. PII-safe: no key/address bytes in Display.
    #[error("witness public key does not match the proven scriptPubKey")]
    WitnessKeyMismatch,
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
/// for any P2SH SPK. The witness pubkey is bound to the address by the
/// key-binding guard in [`verify_via_bip322_crate`] (via
/// `Address::is_related_to_pubkey`), NOT by the `bip322 = "=0.0.10"` crate —
/// which does not perform that HASH160 cross-check (the exact soundness gap the
/// guard closes). A witness whose key is unrelated to the P2SH SPK is rejected
/// there with [`Bip322Error::WitnessKeyMismatch`].
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
/// All three per-script sign bodies ship production code:
/// - **P2WPKH** since Phase 15 (carried over from the v1.3 path).
/// - **P2TR** since Phase 19 Plan 19-01 (BIP-341 Schnorr keypath,
///   SIGHASH_DEFAULT, deterministic via `sign_schnorr_no_aux_rand`).
/// - **P2SH-P2WPKH** since Phase 19 Plan 19-01 (BIP-143 ECDSA over the
///   UNWRAPPED P2WPKH redeem, RFC 6979 deterministic).
///
/// The P2TR and P2SH-P2WPKH bodies cross-check that `spk` matches the
/// derivation from `key` (D-111 defense-in-depth) — a mismatch returns
/// [`Bip322Error::ScriptTypeMismatch`] BEFORE any sighash work.
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

/// Build the `final_script_sig` for a P2SH-P2WPKH input spending a UTXO
/// controlled by `pubkey`. Per BIP-141 nested-SegWit:
/// `scriptSig = OP_PUSHBYTES_22 <redeem>` where
/// `redeem = OP_0 OP_PUSHBYTES_20 <HASH160(pubkey)>` (22 bytes).
///
/// Output bytes: `0x16 0x00 0x14 <20-byte HASH160(pubkey)>` — 23 bytes total
/// (1-byte push opcode + 22-byte redeem). Phase 19 Plan 19-01 D-109 sibling
/// to [`sign_simple`] — surfacing the script-specific helper without widening
/// the dispatcher contract (no `match script_type` here).
///
/// Infallible: takes a 33-byte compressed `secp256k1::PublicKey`, so
/// `bitcoin::PublicKey::new(_).wpubkey_hash()` always returns `Some(_)`.
/// Lowest-privilege input — no secret material crosses the function boundary.
pub fn p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf {
    let compressed = bitcoin::PublicKey::new(*pubkey);
    let wpkh = compressed
        .wpubkey_hash()
        .expect("compressed pubkey always has wpubkey_hash");
    let redeem = ScriptBuf::new_p2wpkh(&wpkh);
    bitcoin::blockdata::script::Builder::new()
        .push_slice(
            <&bitcoin::script::PushBytes>::try_from(redeem.as_bytes())
                .expect("22-byte redeem fits push limit (520 bytes)"),
        )
        .into_script()
}

// ---------------------------------------------------------------------------
// 26-LOC bip322 crate adapter — Sprint-0-A:145-175 verbatim per D-26.
// Wraps `bip322::verify_simple(&Address, message, Witness)` into our
// `(spk, witness, message, network)` wire shape. Error mapping preserves
// the underlying `bip322::error::Error` via `#[source]` (no string collapse).
// ---------------------------------------------------------------------------

/// Sign an ECDSA digest, retrying with deterministic counter-derived entropy
/// until the resulting DER signature + SIGHASH_ALL byte is 71 or 72 bytes.
///
/// `bip322 = "=0.0.10"` (see verify.rs:138-153) hardcodes
/// `match signature_length { 71 | 72 => ... else SignatureLength }` for the
/// witness sig length, but valid Bitcoin ECDSA DER signatures can be 70
/// bytes (S naturally 31 bytes) or 73 bytes (both R and S padded with a
/// leading 0x00) too. Without this retry loop, ~5% of RFC 6979 deterministic
/// signatures fall outside 71/72 and the upstream verifier rejects them as
/// malformed even though they're cryptographically valid — surfacing as
/// intermittent CI failures in any test that signs random keys.
///
/// The retry uses a u32 counter as the noncedata seed so the helper is itself
/// deterministic: same (key, message) always converges to the same final
/// signature. Strict RFC 6979 determinism is broken, but BIP-322 doesn't
/// require it — only verifiability and key binding, both of which hold.
pub(crate) fn sign_ecdsa_compat_bip322_length(
    secp: &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All>,
    msg: &bitcoin::secp256k1::Message,
    key: &bitcoin::secp256k1::SecretKey,
) -> bitcoin::secp256k1::ecdsa::Signature {
    let mut sig = secp.sign_ecdsa(msg, key);
    for counter in 1u32..=256 {
        let total_len = sig.serialize_der().len() + 1; // + SIGHASH_ALL byte
        if total_len == 71 || total_len == 72 {
            return sig;
        }
        let mut entropy = [0u8; 32];
        entropy[..4].copy_from_slice(&counter.to_le_bytes());
        sig = secp.sign_ecdsa_with_noncedata(msg, key, &entropy);
    }
    // 256 deterministic retries failing is statistically impossible (~1e-617);
    // returning the last sig keeps the cryptographic binding so the caller
    // surfaces a real verifier error rather than panicking inside the signer.
    sig
}

pub(crate) fn verify_via_bip322_crate(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), Bip322Error> {
    let address = bitcoin::Address::from_script(spk, network)
        .map_err(|source| Bip322Error::UnrecognisedScriptPubkey { source })?;

    // SECURITY — BIP-322 key-binding guard. KEEP THIS EVEN AFTER THE UPSTREAM CRATE
    // IS PATCHED. `bip322` verifies the witness signature against the key carried IN
    // the witness, but for P2WPKH / P2SH-P2WPKH it does NOT verify that key is the one
    // the scriptPubKey's HASH160 commits to (its "key-mismatch" check compares the
    // witness key with itself). Affected: P2WPKH 0.0.6–0.0.10, P2SH-P2WPKH
    // 0.0.7–0.0.10 (we pin =0.0.10); P2TR unaffected. Reported upstream via private
    // disclosure; no patched crates.io release yet.
    //
    // Impact without this guard: an attacker signs the BIP-322 challenge with THEIR
    // OWN key and has it accepted as ownership of a victim's UTXO. It does NOT let
    // them spend the coin (they still lack its real key), but it defeats the
    // ownership GATE — letting them register UTXOs they do not control, disrupt
    // CoinJoin rounds, and degrade availability/privacy. We re-bind the witness pubkey
    // to the address here so proof soundness does not depend on the crate. Both
    // OwnershipProof envelopes (v1 and v2) funnel through this one point.
    //
    // Gated `!is_p2tr`: P2TR key-spend commits the output key in the scriptPubKey
    // itself and the witness carries only a Schnorr signature (no pubkey to bind), so
    // BIP-341 verification is already key-bound.
    if !spk.is_p2tr() {
        if witness.len() != 2 {
            return Err(Bip322Error::InvalidWitnessLength {
                expected: 2,
                got: witness.len(),
            });
        }
        let pk_bytes = witness.nth(1).ok_or(Bip322Error::WitnessKeyMismatch)?;
        let pubkey =
            bitcoin::PublicKey::from_slice(pk_bytes).map_err(|_| Bip322Error::WitnessKeyMismatch)?;
        // Compressed-only: P2WPKH / P2SH-P2WPKH commit to a compressed-key HASH160;
        // an uncompressed key hashes differently and would not be `is_related`, but
        // we reject it explicitly so the intent is unambiguous.
        if !pubkey.compressed || !address.is_related_to_pubkey(&pubkey) {
            return Err(Bip322Error::WitnessKeyMismatch);
        }
    }

    bip322::verify_simple(&address, message, witness.clone())
        .map_err(|source| Bip322Error::CrateVerifyFailed { source: Box::new(source) })
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

    // --- SECURITY: BIP-322 key-binding guard regression tests ---
    //
    // The pinned `bip322 = "=0.0.10"` crate verifies a witness signature against the
    // key carried in the witness but does NOT bind that key to the address's HASH160
    // for P2WPKH / P2SH-P2WPKH. The guard in `verify_via_bip322_crate` re-binds it.
    // These tests assert the guard rejects an unrelated key (the forgery) and still
    // accepts the honest key — for both single-key script types.

    fn p2wpkh_spk_for(pk: &PublicKey) -> ScriptBuf {
        ScriptBuf::new_p2wpkh(&pk.wpubkey_hash().unwrap())
    }
    fn p2sh_p2wpkh_spk_for(pk: &PublicKey) -> ScriptBuf {
        let redeem = ScriptBuf::new_p2wpkh(&pk.wpubkey_hash().unwrap());
        ScriptBuf::new_p2sh(&redeem.script_hash())
    }

    #[test]
    fn p2wpkh_unrelated_witness_key_is_rejected() {
        use bitcoin::secp256k1::PublicKey as SecpPublicKey;
        let secp = Secp256k1::new();
        let victim = SecpSecretKey::from_slice(&[0x11u8; 32]).unwrap();
        let attacker = SecpSecretKey::from_slice(&[0x22u8; 32]).unwrap();
        let victim_pk = PublicKey::new(SecpPublicKey::from_secret_key(&secp, &victim));
        let victim_spk = p2wpkh_spk_for(&victim_pk);
        let msg = b"blindjoin:round:1:utxo:abc:0";
        // Forgery: p2wpkh::sign uses the passed spk for the sighash and the passed key
        // for signing, so signing the VICTIM's spk with the ATTACKER's key yields a
        // witness [attacker_sig, attacker_pubkey] — a valid BIP-322 signature that the
        // vulnerable crate would accept against the victim's address.
        let attack = super::p2wpkh::sign(&victim_spk, &attacker, msg).unwrap();
        let res = verify_via_bip322_crate(&victim_spk, &attack, msg, Network::Signet);
        assert!(
            matches!(res, Err(Bip322Error::WitnessKeyMismatch)),
            "unrelated-key P2WPKH witness must be rejected by the guard; got {res:?}",
        );
    }

    #[test]
    fn p2wpkh_related_witness_key_verifies() {
        use bitcoin::secp256k1::PublicKey as SecpPublicKey;
        let secp = Secp256k1::new();
        let victim = SecpSecretKey::from_slice(&[0x11u8; 32]).unwrap();
        let victim_pk = PublicKey::new(SecpPublicKey::from_secret_key(&secp, &victim));
        let victim_spk = p2wpkh_spk_for(&victim_pk);
        let msg = b"blindjoin:round:1:utxo:abc:0";
        let honest = super::p2wpkh::sign(&victim_spk, &victim, msg).unwrap();
        let res = verify_via_bip322_crate(&victim_spk, &honest, msg, Network::Signet);
        assert!(res.is_ok(), "honest P2WPKH proof must still verify: {res:?}");
    }

    #[test]
    fn p2sh_p2wpkh_unrelated_witness_key_is_rejected() {
        use bitcoin::secp256k1::PublicKey as SecpPublicKey;
        let secp = Secp256k1::new();
        let victim = SecpSecretKey::from_slice(&[0x33u8; 32]).unwrap();
        let attacker = SecpSecretKey::from_slice(&[0x44u8; 32]).unwrap();
        let victim_pk = PublicKey::new(SecpPublicKey::from_secret_key(&secp, &victim));
        let attacker_pk = PublicKey::new(SecpPublicKey::from_secret_key(&secp, &attacker));
        let victim_spk = p2sh_p2wpkh_spk_for(&victim_pk);
        let msg = b"blindjoin:round:1:utxo:def:0";
        // p2sh_p2wpkh::sign has a spk↔key cross-check, so build the attack witness by
        // hand: [dummy sig, attacker pubkey]. The guard runs before the crate verify,
        // so the signature bytes are irrelevant — the unrelated key alone must trigger
        // rejection.
        let mut attack = Witness::new();
        attack.push(vec![0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x01]);
        attack.push(attacker_pk.to_bytes());
        let res = verify_via_bip322_crate(&victim_spk, &attack, msg, Network::Signet);
        assert!(
            matches!(res, Err(Bip322Error::WitnessKeyMismatch)),
            "unrelated-key P2SH-P2WPKH witness must be rejected by the guard; got {res:?}",
        );
    }

    #[test]
    fn p2sh_p2wpkh_related_witness_key_verifies() {
        use bitcoin::secp256k1::PublicKey as SecpPublicKey;
        let secp = Secp256k1::new();
        let victim = SecpSecretKey::from_slice(&[0x33u8; 32]).unwrap();
        let victim_pk = PublicKey::new(SecpPublicKey::from_secret_key(&secp, &victim));
        let victim_spk = p2sh_p2wpkh_spk_for(&victim_pk);
        let msg = b"blindjoin:round:1:utxo:def:0";
        let honest = super::p2sh_p2wpkh::sign(&victim_spk, &victim, msg).unwrap();
        // Also confirms `Address::is_related_to_pubkey` accepts a P2SH-P2WPKH address
        // + its key (the guard must not reject honest wrapped-segwit proofs).
        let res = verify_via_bip322_crate(&victim_spk, &honest, msg, Network::Signet);
        assert!(res.is_ok(), "honest P2SH-P2WPKH proof must still verify: {res:?}");
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

    // --- Plan 19-01 Task 3 — p2sh_p2wpkh_final_script_sig helper (D-108) + ---
    // --- D-111 cross-check rejection unit tests (CD-37 default = yes).     ---

    #[test]
    fn p2sh_p2wpkh_final_script_sig_derives_correctly() {
        use bitcoin::secp256k1::PublicKey as SecpPublicKey;

        // BIP-141 nested-SegWit shape:
        //   scriptSig = OP_PUSHBYTES_22 || redeem
        //   redeem    = OP_0 || OP_PUSHBYTES_20 || HASH160(pubkey)
        // Total = 1 (push opcode) + 22 (redeem) = 23 bytes.
        // Per Phase 19 RESEARCH §Q3 the byte count is 23, NOT 24 (CONTEXT
        // D-110 off-by-one corrected — the redeem is the PUSHED data, the
        // push opcode itself is a separate 1-byte prefix).
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let pk = SecpPublicKey::from_secret_key(&secp, &sk);

        let script_sig = p2sh_p2wpkh_final_script_sig(&pk);
        let bytes = script_sig.as_bytes();

        assert_eq!(
            bytes.len(),
            23,
            "scriptSig must be 23 bytes (1-byte push opcode + 22-byte redeem)"
        );
        assert_eq!(bytes[0], 0x16, "first byte must be OP_PUSHBYTES_22");
        assert_eq!(bytes[1], 0x00, "redeem byte 0 must be OP_0");
        assert_eq!(bytes[2], 0x14, "redeem byte 1 must be OP_PUSHBYTES_20");

        let compressed = PublicKey::new(pk);
        let expected_wpkh = compressed.wpubkey_hash().expect("compressed");
        let expected_hash160: &[u8] = <WPubkeyHash as AsRef<[u8]>>::as_ref(&expected_wpkh);
        assert_eq!(
            &bytes[3..23],
            expected_hash160,
            "trailing 20 bytes = HASH160(pubkey)"
        );
    }

    #[test]
    fn p2tr_sign_rejects_p2sh_p2wpkh_spk_with_p2tr_key() {
        // D-111 cross-check (P2TR side): the supplied secret key derives a
        // P2TR output key, but the caller passed a P2SH-P2WPKH spk. The
        // sign body must reject with ScriptTypeMismatch BEFORE any sighash
        // work, with `declared = P2shP2wpkh` (from detect_script_type) and
        // `derived = P2tr` (from the key's script type).
        let spk = fixture_p2sh_spk();
        let key = fixture_secret_key();
        let err = sign_simple(ScriptType::P2tr, &spk, &key, b"x")
            .expect_err("P2TR sign with P2SH-P2WPKH spk must reject");
        assert!(
            matches!(
                err,
                Bip322Error::ScriptTypeMismatch {
                    declared: ScriptType::P2shP2wpkh,
                    derived: ScriptType::P2tr,
                }
            ),
            "expected ScriptTypeMismatch{{P2shP2wpkh, P2tr}}, got: {err:?}"
        );
    }

    #[test]
    fn p2sh_p2wpkh_sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key() {
        // D-111 cross-check (P2SH-P2WPKH side): the supplied secret key
        // derives the unwrapped P2WPKH redeem (and thus the P2SH outer SPK),
        // but the caller passed a P2TR spk. The sign body must reject with
        // ScriptTypeMismatch BEFORE any sighash work.
        let spk = fixture_p2tr_spk();
        let key = fixture_secret_key();
        let err = sign_simple(ScriptType::P2shP2wpkh, &spk, &key, b"x")
            .expect_err("P2SH-P2WPKH sign with P2TR spk must reject");
        assert!(
            matches!(
                err,
                Bip322Error::ScriptTypeMismatch {
                    declared: ScriptType::P2tr,
                    derived: ScriptType::P2shP2wpkh,
                }
            ),
            "expected ScriptTypeMismatch{{P2tr, P2shP2wpkh}}, got: {err:?}"
        );
    }
}
