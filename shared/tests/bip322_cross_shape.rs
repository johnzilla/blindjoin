//! V1.4-CRIT-01 mitigation — cross-shape rejection matrix.
//!
//! Plan 15-03 Task 3 — closes the V1.4-CRIT-01 (script-type spoofing) vector
//! at the shared/ crate boundary. EXACTLY 9 #[test] fns per CONTEXT D-34
//! verbatim, each asserting a SPECIFIC `Bip322Error` variant via
//! `matches!()` so silent acceptance of the wrong rejection class is
//! statically impossible (RESEARCH A3 — `assert!(result.is_err())`
//! shortcuts are forbidden).
//!
//! Each #[test] fn:
//! 1. Constructs a known SPK of one type (P2WPKH / P2TR / P2SH-P2WPKH)
//! 2. Constructs a known witness of a DIFFERENT type (or empty)
//! 3. Calls `shared::bip322::verify_simple(declared_type, &spk, &witness, b"test", Network::Regtest)`
//! 4. Asserts a specific `Bip322Error` variant via `matches!()`
//!
//! Diagonal (matching) entries — p2wpkh × p2wpkh, p2tr × p2tr,
//! p2sh_p2wpkh × p2sh_p2wpkh — are the positive sign↔verify roundtrip tests
//! in `per_script_vectors.rs`, NOT here.

use bitcoin::key::TapTweak;
use bitcoin::secp256k1::{Keypair, PublicKey as SecpPubkey, Secp256k1, SecretKey, XOnlyPublicKey};
use bitcoin::{Network, PublicKey, ScriptBuf, Witness};
use shared::bip322::{verify_simple, Bip322Error, ScriptType};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_known_p2wpkh_spk() -> ScriptBuf {
    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[0x10; 32]).expect("deterministic p2wpkh key");
    let pk = SecpPubkey::from_secret_key(&secp, &key);
    let compressed = PublicKey::new(pk);
    ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed key"))
}

fn make_known_p2tr_spk() -> ScriptBuf {
    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[0x11; 32]).expect("deterministic p2tr key");
    let keypair = Keypair::from_secret_key(&secp, &key);
    let (xonly, _parity) = XOnlyPublicKey::from_keypair(&keypair);
    let tweaked = keypair.tap_tweak(&secp, None);
    let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
    let _ = xonly; // suppress unused
    ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked())
}

fn make_known_p2sh_p2wpkh_spk() -> ScriptBuf {
    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[0x12; 32]).expect("deterministic p2sh-p2wpkh key");
    let pk = SecpPubkey::from_secret_key(&secp, &key);
    let compressed = PublicKey::new(pk);
    let inner_p2wpkh =
        ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed key"));
    ScriptBuf::new_p2sh(&inner_p2wpkh.script_hash())
}

/// P2WPKH-shaped witness: 2 elements `[sig (72 bytes), pubkey (33 bytes)]`.
/// Dummy bytes — arity is what matters; underlying ECDSA verify will fail
/// downstream when the dispatcher delegates to the bip322 crate.
fn make_p2wpkh_shaped_witness() -> Witness {
    let mut w = Witness::new();
    w.push([0u8; 72]); // 72-byte dummy DER sig
    w.push([0u8; 33]); // 33-byte dummy compressed pubkey
    w
}

/// P2TR-shaped witness: 1 element of 64 bytes (Schnorr SIGHASH_DEFAULT).
fn make_p2tr_shaped_witness() -> Witness {
    let mut w = Witness::new();
    w.push([0u8; 64]); // 64-byte dummy Schnorr sig
    w
}

/// P2SH-P2WPKH-shaped witness: 2 elements, same shape as P2WPKH.
fn make_p2sh_p2wpkh_shaped_witness() -> Witness {
    make_p2wpkh_shaped_witness()
}

fn make_empty_witness() -> Witness {
    Witness::new()
}

// ---------------------------------------------------------------------------
// D-34 verbatim — 9 #[test] fns (6 cross-shape + 3 empty-witness arity)
// Each asserts a SPECIFIC `Bip322Error` variant via `matches!()` per RESEARCH A3.
// ---------------------------------------------------------------------------

#[test]
fn reject_p2wpkh_spk_with_p2tr_witness() {
    let spk = make_known_p2wpkh_spk();
    let witness = make_p2tr_shaped_witness(); // 1 element
    let result = verify_simple(ScriptType::P2wpkh, &spk, &witness, b"test", Network::Regtest);
    // Arity pre-flight in p2wpkh::verify (expects 2 items) fires first.
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 2, got: 1 })
        ),
        "expected InvalidWitnessLength {{ expected: 2, got: 1 }}, got {result:?}",
    );
}

#[test]
fn reject_p2wpkh_spk_with_p2sh_p2wpkh_witness() {
    let spk = make_known_p2wpkh_spk();
    let witness = make_p2sh_p2wpkh_shaped_witness(); // 2 elements — arity matches
    let result = verify_simple(ScriptType::P2wpkh, &spk, &witness, b"test", Network::Regtest);
    // Arity passes (2 == 2); the bip322 crate's verify_full_p2wpkh fails on
    // the dummy signature/pubkey, surfacing as CrateVerifyFailed.
    assert!(
        matches!(result, Err(Bip322Error::CrateVerifyFailed { .. })),
        "expected CrateVerifyFailed, got {result:?}",
    );
}

#[test]
fn reject_p2tr_spk_with_p2wpkh_witness() {
    let spk = make_known_p2tr_spk();
    let witness = make_p2wpkh_shaped_witness(); // 2 elements
    let result = verify_simple(ScriptType::P2tr, &spk, &witness, b"test", Network::Regtest);
    // Arity pre-flight in p2tr::verify (expects 1 item) fires first.
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 1, got: 2 })
        ),
        "expected InvalidWitnessLength {{ expected: 1, got: 2 }}, got {result:?}",
    );
}

#[test]
fn reject_p2tr_spk_with_p2sh_p2wpkh_witness() {
    let spk = make_known_p2tr_spk();
    let witness = make_p2sh_p2wpkh_shaped_witness(); // 2 elements
    let result = verify_simple(ScriptType::P2tr, &spk, &witness, b"test", Network::Regtest);
    // Arity pre-flight in p2tr::verify (expects 1 item) fires first.
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 1, got: 2 })
        ),
        "expected InvalidWitnessLength {{ expected: 1, got: 2 }}, got {result:?}",
    );
}

#[test]
fn reject_p2sh_p2wpkh_spk_with_p2wpkh_witness() {
    let spk = make_known_p2sh_p2wpkh_spk();
    let witness = make_p2wpkh_shaped_witness(); // 2 elements — arity matches
    let result = verify_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &witness,
        b"test",
        Network::Regtest,
    );
    // Arity passes (2 == 2); the bip322 crate's verify_full_p2wpkh(is_p2sh=true)
    // fails on the HASH160 cross-check at verify.rs:167-169 (the dummy pubkey
    // does not hash to the on-chain P2SH SPK's script-hash). Surfaces as
    // CrateVerifyFailed.
    assert!(
        matches!(result, Err(Bip322Error::CrateVerifyFailed { .. })),
        "expected CrateVerifyFailed (HASH160 cross-check fails), got {result:?}",
    );
}

#[test]
fn reject_p2sh_p2wpkh_spk_with_p2tr_witness() {
    let spk = make_known_p2sh_p2wpkh_spk();
    let witness = make_p2tr_shaped_witness(); // 1 element
    let result = verify_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &witness,
        b"test",
        Network::Regtest,
    );
    // Arity pre-flight in p2sh_p2wpkh::verify (expects 2 items) fires first.
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 2, got: 1 })
        ),
        "expected InvalidWitnessLength {{ expected: 2, got: 1 }}, got {result:?}",
    );
}

#[test]
fn reject_p2wpkh_spk_with_empty_witness() {
    let spk = make_known_p2wpkh_spk();
    let witness = make_empty_witness(); // 0 elements
    let result = verify_simple(ScriptType::P2wpkh, &spk, &witness, b"test", Network::Regtest);
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 2, got: 0 })
        ),
        "expected InvalidWitnessLength {{ expected: 2, got: 0 }}, got {result:?}",
    );
}

#[test]
fn reject_p2tr_spk_with_empty_witness() {
    let spk = make_known_p2tr_spk();
    let witness = make_empty_witness(); // 0 elements
    let result = verify_simple(ScriptType::P2tr, &spk, &witness, b"test", Network::Regtest);
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 1, got: 0 })
        ),
        "expected InvalidWitnessLength {{ expected: 1, got: 0 }}, got {result:?}",
    );
}

#[test]
fn reject_p2sh_p2wpkh_spk_with_empty_witness() {
    let spk = make_known_p2sh_p2wpkh_spk();
    let witness = make_empty_witness(); // 0 elements
    let result = verify_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &witness,
        b"test",
        Network::Regtest,
    );
    assert!(
        matches!(
            result,
            Err(Bip322Error::InvalidWitnessLength { expected: 2, got: 0 })
        ),
        "expected InvalidWitnessLength {{ expected: 2, got: 0 }}, got {result:?}",
    );
}
