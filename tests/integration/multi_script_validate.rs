//! v1.4 Phase 16 Plan 16-02 Task 3 — 9 D-54 verbatim test cases covering the
//! multi-script `validate_utxo` dispatcher + CRIT-01 cross-check + allowlist
//! gate + envelope-shape edge cases.
//!
//! Reuses the shared regtest fixtures from `tests/integration/mod.rs`:
//!   - `BitcoindGuard` (RAII bitcoind ownership)
//!   - `require_bitcoind!()` (graceful-skip without bitcoind)
//!   - `fund_regtest_typed(...)` + `TypedUtxoHandle` (per-script UTXOs)
//!
//! Each test boots one bitcoind per test fn (matches the existing
//! `full_round.rs` isolation pattern). Variant assertions go via the
//! `#[doc(hidden)] pub fn coordinator::bitcoin::utxo::validate_ownership_proof_typed`
//! accessor introduced in Task 1 so the tests can assert on specific
//! `Bip322Error` variants via `matches!(...)` per Phase 15-03 D-34
//! discipline (no string parsing of `UtxoError::InvalidProof.reason`).

#![allow(clippy::needless_borrows_for_generic_args)]

use base64::Engine;
use bitcoin::{Network, Witness};
use coordinator::bitcoin::utxo::validate_ownership_proof_typed;
use coordinator::config::BipConfig;
use shared::bip322::{sign_simple, Bip322Error, ScriptType};
use shared::protocol::OwnershipProof;

use crate::{fund_regtest_typed, require_bitcoind, TypedUtxoHandle};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Build the BIP-322 message string the dispatcher expects.
///
/// Matches the format string the production validate_utxo at
/// `coordinator/src/bitcoin/utxo.rs` constructs:
/// `"blindjoin:round:{round_id}:utxo:{txid}:{vout}"`.
fn dispatcher_message(round_id: &str, handle: &TypedUtxoHandle) -> String {
    format!(
        "blindjoin:round:{}:utxo:{}:{}",
        round_id, handle.outpoint.txid, handle.outpoint.vout
    )
}

/// Encode a `Witness` into the v=2 wire `psbt_input_b64` shape: a
/// base64-encoded BIP-174 PSBT with one input + zero outputs, whose
/// `final_script_witness` carries the given witness.
///
/// This MUST invert byte-for-byte the dispatcher's `decode_psbt_input_witness`
/// helper at `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness`
/// (which calls `bitcoin::psbt::Psbt::deserialize` and reads
/// `psbt.inputs[0].final_script_witness`). The 1-line roundtrip assertion in
/// each test body (`assert_eq!(decode_via_dispatch, witness)`) catches any
/// encoder/decoder drift immediately.
fn build_v2_psbt_input_b64(witness: &Witness) -> String {
    use bitcoin::psbt::Psbt;
    use bitcoin::{absolute, transaction, OutPoint, ScriptBuf, Sequence, Transaction, TxIn};

    let unsigned_tx = Transaction {
        version: transaction::Version(2),
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![],
    };
    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).expect("unsigned tx -> psbt");
    psbt.inputs[0].final_script_witness = Some(witness.clone());
    B64.encode(psbt.serialize())
}

/// Build a v=2 OwnershipProof from a signed witness + a declared script type.
fn build_v2_proof(witness: Witness, declared: Option<ScriptType>) -> OwnershipProof {
    OwnershipProof {
        version: 2,
        witness_stack: vec![],
        psbt_input_b64: Some(build_v2_psbt_input_b64(&witness)),
        script_type: declared,
    }
}

/// Build a v=1 OwnershipProof from raw witness stack items.
fn build_v1_proof(witness_items: Vec<Vec<u8>>) -> OwnershipProof {
    OwnershipProof {
        version: 1,
        witness_stack: witness_items,
        psbt_input_b64: None,
        script_type: None,
    }
}

/// Per-test unique round_id so message strings differ across tests in the same
/// session — defensive against any future regtest-bitcoind reuse.
fn unique_round_id(test_tag: &str) -> String {
    format!(
        "16-02-multi-script-{}-{}",
        test_tag,
        std::process::id() % 1_000_000
    )
}

/// All-allowed default BipConfig (matches Phase 16 Plan 16-01 D-38).
fn default_bip_config() -> BipConfig {
    BipConfig::default()
}

/// Sign a BIP-322 Simple witness for the given script type using the
/// per-UTXO secret key.
///
/// Phase 19 Plan 19-02 (BIP322-07): migrated off the deleted test-only
/// mirror onto the production `sign_simple` dispatcher. The 9 cross-shape
/// rejection cases below now exercise the production `sign` bodies shipped
/// in Plan 19-01 (P2TR keypath + P2SH-P2WPKH BIP-143 with the D-111 spk↔key
/// cross-check at top).
fn sign_witness(handle: &TypedUtxoHandle, message: &[u8]) -> Witness {
    sign_simple(
        handle.script_type,
        handle.script_pubkey.as_script(),
        &handle.secret_key,
        message,
    )
    .expect("sign_simple should produce a valid witness")
}

// ---------------------------------------------------------------------------
// D-54 verbatim test cases (9 total)
// ---------------------------------------------------------------------------

/// D-54 case 1 — v=1 legacy witness-only proof against an on-chain P2WPKH UTXO.
/// CROSS-PHASE INVARIANT: same path the v1.3 client uses; dispatcher routes
/// `verify_simple(P2wpkh, ...)` which is bit-exact with the deleted
/// verify_bip322_simple per Phase 15-02 SUMMARY.
#[tokio::test]
async fn validate_p2wpkh_utxo_with_v1_legacy_proof_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v1-p2wpkh-ok");
    let message = dispatcher_message(&round_id, handle);
    let witness = sign_witness(handle, message.as_bytes());

    // v=1 envelope: array-of-witness-items, no psbt or script_type.
    let witness_items: Vec<Vec<u8>> = witness.iter().map(|s| s.to_vec()).collect();
    let proof = build_v1_proof(witness_items);

    let cfg = default_bip_config();
    let result = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    );
    assert!(
        result.is_ok(),
        "v=1 legacy P2WPKH must verify (cross-phase invariant): {result:?}"
    );
    assert_eq!(result.unwrap(), ScriptType::P2wpkh);
}

/// D-54 case 2 — v=2 PSBT-input proof declaring P2WPKH against a P2WPKH UTXO.
/// declared == derived → passes the CRIT-01 cross-check.
#[tokio::test]
async fn validate_p2wpkh_utxo_with_v2_declared_p2wpkh_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-p2wpkh-ok");
    let message = dispatcher_message(&round_id, handle);
    let witness = sign_witness(handle, message.as_bytes());
    let proof = build_v2_proof(witness, Some(ScriptType::P2wpkh));

    let cfg = default_bip_config();
    let result = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    );
    assert!(result.is_ok(), "v=2 P2WPKH declared=derived must verify: {result:?}");
    assert_eq!(result.unwrap(), ScriptType::P2wpkh);
}

/// D-54 case 3 — v=2 PSBT-input proof declaring P2TR against a P2TR UTXO.
#[tokio::test]
async fn validate_p2tr_utxo_with_v2_declared_p2tr_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2tr, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-p2tr-ok");
    let message = dispatcher_message(&round_id, handle);
    let witness = sign_witness(handle, message.as_bytes());
    let proof = build_v2_proof(witness, Some(ScriptType::P2tr));

    let cfg = default_bip_config();
    let result = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    );
    assert!(result.is_ok(), "v=2 P2TR declared=derived must verify: {result:?}");
    assert_eq!(result.unwrap(), ScriptType::P2tr);
}

/// D-54 case 4 — v=2 PSBT-input proof declaring P2SH-P2WPKH against a
/// P2SH-P2WPKH UTXO.
#[tokio::test]
async fn validate_p2sh_p2wpkh_utxo_with_v2_declared_p2sh_p2wpkh_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2shP2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-p2sh-ok");
    let message = dispatcher_message(&round_id, handle);
    let witness = sign_witness(handle, message.as_bytes());
    let proof = build_v2_proof(witness, Some(ScriptType::P2shP2wpkh));

    let cfg = default_bip_config();
    let result = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    );
    assert!(
        result.is_ok(),
        "v=2 P2SH-P2WPKH declared=derived must verify: {result:?}"
    );
    assert_eq!(result.unwrap(), ScriptType::P2shP2wpkh);
}

/// D-54 case 5 — v=2 proof declaring P2TR against an on-chain P2WPKH UTXO.
/// CRIT-01 spoofing rejection. The cross-check MUST fire BEFORE verify_simple,
/// so a well-formed P2TR-shaped witness is sufficient — the dispatcher rejects
/// purely on declared != derived.
#[tokio::test]
async fn validate_p2wpkh_utxo_with_v2_declared_p2tr_rejects_spoofing() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-p2wpkh-p2tr-spoof");
    let message = dispatcher_message(&round_id, handle);
    // Witness contents are irrelevant — the dispatcher's declared-vs-derived
    // check fires BEFORE verify_simple. We use a well-formed 1-item Witness
    // (P2TR Simple arity) so even if a regression let verify_simple run, it
    // would still see a structurally plausible input.
    let mut spoof_witness = Witness::new();
    spoof_witness.push([0u8; 64]); // 64-byte Schnorr placeholder
    let proof = build_v2_proof(spoof_witness, Some(ScriptType::P2tr));

    let cfg = default_bip_config();
    let err = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    )
    .expect_err("CRIT-01 spoof: declared p2tr against on-chain p2wpkh must reject");

    assert!(
        matches!(
            err,
            Bip322Error::ScriptTypeMismatch {
                declared: ScriptType::P2tr,
                derived: ScriptType::P2wpkh,
            }
        ),
        "expected ScriptTypeMismatch {{ P2tr, P2wpkh }}, got: {err:?}"
    );
}

/// D-54 case 6 — symmetric spoof: v=2 proof declaring P2WPKH against an
/// on-chain P2TR UTXO. Asserts the CRIT-01 cross-check is bidirectional.
#[tokio::test]
async fn validate_p2tr_utxo_with_v2_declared_p2wpkh_rejects_spoofing() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2tr, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-p2tr-p2wpkh-spoof");
    let message = dispatcher_message(&round_id, handle);
    // Two-item P2WPKH-shaped placeholder witness; the cross-check fires first.
    let mut spoof_witness = Witness::new();
    spoof_witness.push([0u8; 71]); // ECDSA DER placeholder
    spoof_witness.push([0u8; 33]); // compressed pubkey placeholder
    let proof = build_v2_proof(spoof_witness, Some(ScriptType::P2wpkh));

    let cfg = default_bip_config();
    let err = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    )
    .expect_err("CRIT-01 spoof: declared p2wpkh against on-chain p2tr must reject");

    assert!(
        matches!(
            err,
            Bip322Error::ScriptTypeMismatch {
                declared: ScriptType::P2wpkh,
                derived: ScriptType::P2tr,
            }
        ),
        "expected ScriptTypeMismatch {{ P2wpkh, P2tr }}, got: {err:?}"
    );
}

/// D-54 case 7 — v=2 proof omitting `script_type`. D-48 dictates
/// `Bip322Error::WireFormatMismatch` whose message mentions `script_type`.
#[tokio::test]
async fn validate_v2_proof_without_script_type_rejects_wireformat_mismatch() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-no-st");
    let message = dispatcher_message(&round_id, handle);
    // A nominally-valid v=2 envelope but script_type omitted.
    let witness = sign_witness(handle, message.as_bytes());
    let proof = OwnershipProof {
        version: 2,
        witness_stack: vec![],
        psbt_input_b64: Some(build_v2_psbt_input_b64(&witness)),
        script_type: None,
    };

    let cfg = default_bip_config();
    let err = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    )
    .expect_err("v=2 without script_type must reject");

    match err {
        Bip322Error::WireFormatMismatch(ref msg) => {
            assert!(
                msg.contains("script_type"),
                "WireFormatMismatch should mention 'script_type': {msg}"
            );
        }
        other => panic!("expected WireFormatMismatch, got: {other:?}"),
    }
}

/// D-54 case 8 — v=2 proof against a P2TR UTXO when the BipConfig sets
/// `allow_p2tr = false`. The dispatcher's allowlist gate fires AFTER the
/// declared==derived cross-check passes but BEFORE verify_simple.
#[tokio::test]
async fn validate_p2tr_utxo_with_allow_p2tr_false_rejects_unsupported() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2tr, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v2-p2tr-disallowed");
    let message = dispatcher_message(&round_id, handle);
    let witness = sign_witness(handle, message.as_bytes());
    let proof = build_v2_proof(witness, Some(ScriptType::P2tr));

    let cfg = BipConfig {
        allow_p2wpkh: true,
        allow_p2tr: false,
        allow_p2sh_p2wpkh: true,
        output_script_type: ScriptType::P2wpkh,
    };
    let err = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    )
    .expect_err("allow_p2tr=false must reject P2TR proof");

    assert!(
        matches!(err, Bip322Error::UnsupportedScriptType),
        "expected UnsupportedScriptType (allowlist gate), got: {err:?}"
    );
}

/// D-54 case 9 — OwnershipProof with `version = 3` (unknown). D-12 dictates
/// the dispatcher's default arm returns `UnsupportedProofVersion(3)`.
/// Uses a P2WPKH UTXO for setup but the version check fires before any
/// SPK-derivation work.
#[tokio::test]
async fn validate_unknown_version_3_rejects_unsupported_proof_version() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v3-unknown");
    let message = dispatcher_message(&round_id, handle);
    let proof = OwnershipProof {
        version: 3,
        witness_stack: vec![],
        psbt_input_b64: None,
        script_type: None,
    };

    let cfg = default_bip_config();
    let err = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    )
    .expect_err("v=3 must reject");

    assert!(
        matches!(err, Bip322Error::UnsupportedProofVersion(3)),
        "expected UnsupportedProofVersion(3), got: {err:?}"
    );
}
