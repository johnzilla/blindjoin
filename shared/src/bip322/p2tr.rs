//! P2TR BIP-322 Simple verify + sign per Phase 15 CONTEXT D-04.
//!
//! Both `verify` and `sign` are `pub(crate)` per D-27. Phase 19 Plan 19-01
//! ships the production `sign` body (BIP-341 Schnorr keypath, SIGHASH_DEFAULT)
//! per CONTEXT D-116, lifted from the prior `sign_for_tests` helper with the
//! D-111 spk↔key cross-check inserted at the top. `sign` does NOT depend on
//! `bdk_wallet` (Phase 14 ADR Decision #4 + Phase 15 CD-6 preserve the
//! shared-crate boundary). The `sign_for_tests` alias remains for Plan 15-03
//! integration tests; Plan 19-02 deletes it.

use bitcoin::secp256k1::SecretKey;
use bitcoin::{Network, Script, Witness};

/// Verify a P2TR BIP-322 Simple proof.
///
/// Arity pre-flight: P2TR Simple witnesses carry a single Schnorr signature
/// (1 item, either 64 bytes SIGHASH_DEFAULT or 65 bytes SIGHASH_ALL). The
/// `bip322 = "=0.0.10"` crate's `verify_full_p2tr` (`verify.rs:187-258`)
/// handles both byte lengths internally.
pub(crate) fn verify(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 1 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 1,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}

/// Production sign for P2TR — BIP-341 Schnorr keypath, SIGHASH_DEFAULT.
///
/// Body lifted near-verbatim from the prior `sign_for_tests` helper per Phase
/// 19 CONTEXT D-116 (the test signer was already correct — it produced the
/// witnesses the existing positive-vector + cross-shape integration tests
/// verify against the bip322 crate). Defense-in-depth: D-111 spk↔key
/// cross-check at the TOP rejects mismatched (spk, key) pairs BEFORE any
/// sighash work, returning [`super::Bip322Error::ScriptTypeMismatch`] with
/// `declared = detect_script_type(spk)` and `derived = ScriptType::P2tr`
/// (D-112 variant reuse + D-113 P2TR algorithm).
///
/// Determinism: uses `sign_schnorr_no_aux_rand` per D-114 (BIP-340 §3.3 — no
/// auxiliary randomness). Verified by Phase 19 RESEARCH §Q1 to match
/// `bdk_wallet` 2.3's BIP-322 sign path bit-exactly — the
/// `client/tests/wallet_sign_roundtrip.rs::p2tr_shared_sign_matches_bdk_sign_byte_for_byte`
/// parity test pins this byte-equality.
///
/// `pub(crate)` per D-27 — callers reach this only through `sign_simple`.
pub(crate) fn sign(
    spk: &Script,
    key: &SecretKey,
    message: &[u8],
) -> Result<Witness, super::Bip322Error> {
    // Plan 19-01 Task 1 — BIP322-05 production body; lifted from sign_for_tests
    // per D-116 + D-111 cross-check.
    use bitcoin::hashes::Hash;
    use bitcoin::key::{Keypair, TapTweak};
    use bitcoin::secp256k1::{Message, Secp256k1};
    use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
    use bitcoin::{Amount, ScriptBuf, TxOut};

    // D-111 spk↔key cross-check (P2TR algorithm per D-113): derive the
    // expected output-key SPK from the supplied SecretKey and reject if it
    // does not byte-equal the caller-supplied spk.
    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, key);
    let tweaked = keypair.tap_tweak(&secp, None);
    let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
    let expected_spk = ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked());
    if expected_spk.as_script() != spk {
        return Err(super::Bip322Error::ScriptTypeMismatch {
            declared: super::detect_script_type(spk)?,
            derived: super::ScriptType::P2tr,
        });
    }

    let msg_hash = super::bip322_message_hash(message);
    let to_spend = super::build_bip322_to_spend(spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .taproot_key_spend_signature_hash(
            0,
            &Prevouts::All(&[TxOut {
                value: Amount::ZERO,
                script_pubkey: spk.to_owned(),
            }]),
            TapSighashType::Default,
        )
        .expect("sighash on well-formed to_sign");

    let sig = secp.sign_schnorr_no_aux_rand(
        &Message::from_digest(sighash.to_byte_array()),
        &tweaked.to_keypair(),
    );

    let mut w = Witness::new();
    w.push(sig.as_ref());
    Ok(w)
}

/// Test-only signer producing a valid 64-byte Schnorr keyspend witness.
///
/// Uses the 8-step BIP-341 sequence from Phase 14 Sprint-0-B (and RESEARCH
/// Pattern 4): build to_spend → build to_sign → Keypair::from_secret_key →
/// tap_tweak → taproot_key_spend_signature_hash → sign_schnorr_no_aux_rand →
/// push 64 bytes into Witness. This is the load-bearing test signer that
/// Plan 15-03 consumes for per-script positive-vector tests.
///
/// Plan 15-03 promotes this from `#[cfg(test)] pub(crate)` to `pub(crate)`
/// (no cfg gate) so the integration-test dispatcher mirror
/// `super::sign_simple_test_only` (a `#[doc(hidden)] pub fn` in `mod.rs`)
/// can reach it from external test crates at `shared/tests/*.rs`. Production
/// callers cannot invoke this fn directly because the per-script module is
/// `pub(crate)`-only per D-27.
pub(crate) fn sign_for_tests(spk: &Script, key: &SecretKey, message: &[u8]) -> Witness {
    use bitcoin::hashes::Hash;
    use bitcoin::key::{Keypair, TapTweak};
    use bitcoin::secp256k1::{Message, Secp256k1};
    use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
    use bitcoin::{Amount, TxOut};

    let msg_hash = super::bip322_message_hash(message);
    let to_spend = super::build_bip322_to_spend(spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, key);
    let tweaked = keypair.tap_tweak(&secp, None).to_keypair();

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .taproot_key_spend_signature_hash(
            0,
            &Prevouts::All(&[TxOut {
                value: Amount::ZERO,
                script_pubkey: spk.to_owned(),
            }]),
            TapSighashType::Default,
        )
        .expect("sighash on well-formed to_sign");

    let sig = secp.sign_schnorr_no_aux_rand(
        &Message::from_digest(sighash.to_byte_array()),
        &tweaked,
    );

    let mut w = Witness::new();
    w.push(sig.as_ref());
    w
}
