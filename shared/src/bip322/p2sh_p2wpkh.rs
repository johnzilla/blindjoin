//! P2SH-P2WPKH BIP-322 Simple verify + sign per Phase 15 CONTEXT D-04.
//!
//! Both `verify` and `sign` are `pub(crate)` per D-27. Phase 19 Plan 19-01
//! ships the production `sign` body (BIP-143 sighash over the UNWRAPPED
//! P2WPKH redeem, 2-item `[sig, pubkey]` witness) per CONTEXT D-116, lifted
//! from the prior `sign_for_tests` helper with the D-111 spk↔key cross-check
//! at the top and D-117 `spk`-used-directly after the cross-check (removes
//! the rebuild-from-key footgun where the test signer silently ignored its
//! `_spk` argument). `sign` does NOT depend on `bdk_wallet` (Phase 14 ADR
//! Decision #4 + Phase 15 CD-6 preserve the shared-crate boundary). The
//! `sign_for_tests` test-only alias was deleted in Plan 19-02 (BIP322-07) —
//! production `sign` is now the only sign path.

use bitcoin::secp256k1::SecretKey;
use bitcoin::{Network, Script, Witness};

/// Verify a P2SH-P2WPKH BIP-322 Simple proof.
///
/// Arity pre-flight: P2SH-P2WPKH witnesses are `[sig, pubkey]` (2 items),
/// identical shape to P2WPKH because the unwrapped redeem IS a P2WPKH SPK.
/// The `bip322 = "=0.0.10"` crate's `verify_simple` (`verify.rs:62-99` ->
/// `verify_full_p2wpkh(is_p2sh=true)` at `verify.rs:167-169`) reconstructs
/// the unwrapped P2WPKH from `witness[1].wpubkey_hash()` and HASH160-cross-
/// checks against the P2SH SPK, so non-P2WPKH-wrapped P2SH scripts reject
/// at verify time.
pub(crate) fn verify(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 2 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 2,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}

/// Production sign for P2SH-P2WPKH — BIP-143 over the UNWRAPPED P2WPKH
/// redeem, returning a 2-item `[der_sig+SIGHASH_ALL, compressed_pubkey]`
/// witness.
///
/// Body lifted near-verbatim from the prior `sign_for_tests` helper per
/// Phase 19 CONTEXT D-116, with two production-only transforms:
/// - **D-111 spk↔key cross-check** at the TOP — rejects mismatched
///   `(spk, key)` pairs BEFORE any sighash work, returning
///   [`super::Bip322Error::ScriptTypeMismatch`] (variant reused per D-112;
///   P2SH-P2WPKH algorithm per D-113).
/// - **D-117 spk-used-directly** — the cross-check above proves
///   `expected_spk == spk` byte-equal, so `build_bip322_to_spend(spk, ...)`
///   consumes the caller-supplied parameter directly. The prior
///   `sign_for_tests` rebuilt the outer P2SH SPK from the key (silent
///   footgun: ignored `_spk` argument).
///
/// Determinism: ECDSA via `sign_ecdsa` is RFC 6979 deterministic — the
/// `client/tests/wallet_sign_roundtrip.rs::p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte`
/// parity test (Phase 19 Plan 19-01 D-119) asserts byte-equality with
/// `bdk_wallet` 2.3's BIP-322 sign path.
///
/// Sighash is computed against the UNWRAPPED P2WPKH redeem (not the outer
/// P2SH SPK) — this is structural per BIP-143; the bip322 crate's
/// `verify_full_p2wpkh(is_p2sh=true)` at `verify.rs:167-169` reconstructs
/// the same redeem from `witness[1].wpubkey_hash()` for the verify side.
///
/// `pub(crate)` per D-27 — callers reach this only through `sign_simple`.
pub(crate) fn sign(
    spk: &Script,
    key: &SecretKey,
    message: &[u8],
) -> Result<Witness, super::Bip322Error> {
    // Plan 19-01 Task 2 — BIP322-06 production body; lifted from sign_for_tests
    // per D-116 + D-111 cross-check + D-117 spk-used-directly.
    use bitcoin::hashes::Hash;
    use bitcoin::secp256k1::{Message, Secp256k1};
    use bitcoin::sighash::{EcdsaSighashType, SighashCache};
    use bitcoin::{Amount, PublicKey, ScriptBuf};

    let secp = Secp256k1::new();
    let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, key);
    let compressed = PublicKey::new(pubkey);
    // Derive the UNWRAPPED P2WPKH SPK from the pubkey (this is the sighash SPK).
    let unwrapped_p2wpkh =
        ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed key"));

    // D-111 spk↔key cross-check (P2SH-P2WPKH algorithm per D-113): the
    // expected outer P2SH SPK is `OP_HASH160 <HASH160(redeem)> OP_EQUAL`
    // where `redeem` is the unwrapped P2WPKH SPK above. Reject if it does
    // not byte-equal the caller-supplied `spk`.
    let expected_spk = ScriptBuf::new_p2sh(&unwrapped_p2wpkh.script_hash());
    if expected_spk.as_script() != spk {
        // WR-01 (Phase 19 review): the dispatcher rustdoc on `sign_simple`
        // promises that a (spk, key) mismatch returns
        // `Bip322Error::ScriptTypeMismatch` BEFORE any sighash work. The
        // earlier `detect_script_type(spk)?` form propagated
        // `UnsupportedScriptType` for SPK shapes outside the
        // P2WPKH/P2TR/P2SH trio (e.g. P2WSH, OP_RETURN, bare multisig),
        // breaking that promise. Falling back to the variant the caller
        // invoked (here: P2SH-P2WPKH) preserves the ScriptTypeMismatch
        // contract for non-standard SPK shapes.
        let declared =
            super::detect_script_type(spk).unwrap_or(super::ScriptType::P2shP2wpkh);
        return Err(super::Bip322Error::ScriptTypeMismatch {
            declared,
            derived: super::ScriptType::P2shP2wpkh,
        });
    }

    let msg_hash = super::bip322_message_hash(message);
    // D-117: `spk` is load-bearing here (the cross-check above proves it
    // byte-equals the derived outer P2SH SPK). Removes the rebuild-from-key
    // footgun present in the prior sign_for_tests helper.
    let to_spend = super::build_bip322_to_spend(spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .p2wpkh_signature_hash(
            0,
            &unwrapped_p2wpkh,
            Amount::ZERO,
            EcdsaSighashType::All,
        )
        .expect("sighash on well-formed to_sign");

    let secp_msg = Message::from_digest(sighash.to_byte_array());
    let sig = secp.sign_ecdsa(&secp_msg, key);
    let mut sig_bytes = sig.serialize_der().to_vec();
    sig_bytes.push(0x01); // SIGHASH_ALL

    let mut w = Witness::new();
    w.push(sig_bytes);
    w.push(pubkey.serialize());
    Ok(w)
}

