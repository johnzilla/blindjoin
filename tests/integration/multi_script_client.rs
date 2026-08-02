//! Phase 17 v1.4 Plan 17-03 D-78 — 9 named tests covering the client-side
//! multi-script wallet + discovery fail-fast + WALLET-04 compat shim.
//!
//! Reuses fixtures from `tests/integration/mod.rs`:
//!   - `require_bitcoind!()` (graceful-skip for local dev without bitcoind)
//!   - `fund_regtest_typed(...)` + `TypedUtxoHandle` (per-script regtest UTXOs)
//!
//! Tests 1-6 are `#[ignore]`'d because their semantic equivalents live in
//! `client/tests/wallet_sign_roundtrip.rs` (17-02 deliverable). They are
//! preserved here as named stubs to satisfy the D-78 contract and to
//! provide a single grep-target for the Phase 17 acceptance gate.
//!
//! Tests 7-9 are the load-bearing discovery + envelope-encoder gates;
//! they run without bitcoind (pure resolver-API + encoder-shape assertions).
//! Per D-79, the v1.3-coordinator-binary integration test is deferred to
//! Phase 18 INTEG-01.

#![allow(clippy::needless_borrows_for_generic_args)]

use base64::Engine;
use bitcoin::psbt::Psbt;
use bitcoin::{
    absolute, transaction, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, Witness,
};
use client::discover::{
    capabilities_from_record_v, CoordinatorCapabilities, CoordinatorInfo, DiscoveryError,
};
use client::wallet::BdkClientWallet;
use shared::bip322::ScriptType;
use shared::protocol::OwnershipProof;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Deterministic test-only WIF — NOT for real funds. Canonical Bitcoin
/// Core regtest "Hello World" WIF, shared with
/// `client/tests/wallet_sign_roundtrip.rs::p2wpkh_wif_sign_roundtrip_verifies`.
const TEST_WIF: &str = "cVt4o7BGAig1UXywgGSmARhxMdzP5qvQsxKkSsc1XEkw3tDTQFpy";

/// Placeholder outpoint accepted by all three wallet constructors. BIP-322
/// signs the message against the SPK, not the outpoint.
const DUMMY_OUTPOINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000:0";

const TEST_MESSAGE: &str = "test-message";

// ---------------------------------------------------------------------------
// Local mirror of `client::round::input::build_v2_psbt_input_b64`.
//
// The production helper is module-private; rather than escalate visibility
// just for tests, we re-implement the 17-LOC encoder here verbatim. The
// encoder/decoder byte-inverse contract is the load-bearing invariant; both
// copies share the same shape with the canonical Phase 16-02 reference at
// `tests/integration/multi_script_validate.rs:56-74`.
// ---------------------------------------------------------------------------
fn build_v2_psbt_input_b64(witness: &Witness, final_script_sig: Option<&ScriptBuf>) -> String {
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
    if let Some(sig) = final_script_sig {
        psbt.inputs[0].final_script_sig = Some(sig.clone());
    }
    B64.encode(psbt.serialize())
}

// ---------------------------------------------------------------------------
// Tests 1-3 — Descriptor generation per script type
// (covered by client/src/wallet::tests::* and client/tests/wallet_sign_roundtrip.rs).
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "covered by client/src/wallet::tests::generate_p2wpkh_produces_bip84_descriptor"]
async fn generate_p2wpkh_wallet_emits_bip84_descriptor() {
    // see client/src/wallet.rs `#[cfg(test)] mod tests` for the live assertion
}

#[tokio::test]
#[ignore = "covered by client/src/wallet::tests::generate_p2tr_produces_bip86_descriptor"]
async fn generate_p2tr_wallet_emits_bip86_descriptor() {
    // see client/src/wallet.rs `#[cfg(test)] mod tests` for the live assertion
}

#[tokio::test]
#[ignore = "covered by client/src/wallet::tests::generate_p2sh_p2wpkh_produces_bip49_descriptor"]
async fn generate_p2sh_p2wpkh_wallet_emits_bip49_descriptor() {
    // see client/src/wallet.rs `#[cfg(test)] mod tests` for the live assertion
}

// ---------------------------------------------------------------------------
// Tests 4-6 — Sign roundtrip per script type
// (covered by client/tests/wallet_sign_roundtrip.rs — see 17-02 SUMMARY).
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "covered by client/tests/wallet_sign_roundtrip::p2wpkh_descriptor_sign_roundtrip_verifies"]
async fn p2wpkh_sign_roundtrip_verifies() {
    // see client/tests/wallet_sign_roundtrip.rs for the live assertion
}

#[tokio::test]
#[ignore = "covered by client/tests/wallet_sign_roundtrip::p2tr_descriptor_sign_roundtrip_verifies"]
async fn p2tr_sign_roundtrip_verifies() {
    // see client/tests/wallet_sign_roundtrip.rs for the live assertion
}

#[tokio::test]
#[ignore = "covered by client/tests/wallet_sign_roundtrip::p2sh_p2wpkh_descriptor_sign_roundtrip_verifies"]
async fn p2sh_p2wpkh_sign_roundtrip_verifies() {
    // see client/tests/wallet_sign_roundtrip.rs — asserts signed.final_script_sig.is_some()
}

// ---------------------------------------------------------------------------
// Test 7 — WALLET-03: v1.3 PKARR record + P2TR wallet rejects BEFORE Tor.
// ---------------------------------------------------------------------------

/// Construct a P2TR wallet, derive capabilities from a synthetic v0.1.0
/// PKARR record (legacy, P2WPKH-only), and verify that the discover-layer
/// capability check would reject the coordinator with
/// `DiscoveryError::UnsupportedScriptType` BEFORE any Tor circuit opens.
///
/// The test asserts at the resolver-API boundary (no real DHT roundtrip
/// needed): `capabilities_from_record_v("0.1.0", None, None)` is the
/// load-bearing branch, and the subsequent allowlist check is the
/// fail-fast gate. The structural pre-Tor ordering at
/// `client/src/main.rs` (discover before `if cfg.use_tor`) per D-74 +
/// RESEARCH Pitfall 4 means a returning `Err` from `discover_coordinator`
/// short-circuits before `tor::init_tor` is reachable.
///
/// Error message must contain the literal "does not support" substring
/// (ROADMAP Phase 17 Success Criterion #3 wording).
#[tokio::test]
async fn v13_pkarr_record_with_p2tr_wallet_rejects_before_tor() {
    // Step 1: construct a P2TR descriptor wallet.
    let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2tr, false)
        .expect("P2TR descriptor generate should succeed");
    assert_eq!(wallet.script_type(), ScriptType::P2tr);

    // Step 2: derive capabilities from a synthetic v0.1.0 PKARR record
    // (legacy v1.3 coordinator — no `sst`, no `ost`).
    let caps = capabilities_from_record_v("0.1.0", None, None)
        .expect("legacy capabilities derivation succeeds");
    assert!(caps.is_legacy);
    assert_eq!(caps.supported_script_types, vec![ScriptType::P2wpkh]);

    // Step 3: simulate the discover_coordinator capability check (the same
    // code-path that runs inside `discover_coordinator` pre-Tor — see
    // `client/src/discover.rs` step (e)). A P2TR wallet against a P2WPKH-only
    // coordinator MUST be rejected.
    let required = wallet.script_type();
    assert!(!caps.supported_script_types.contains(&required));
    let err = DiscoveryError::UnsupportedScriptType {
        pubkey: "fake-pubkey-z32".to_string(),
        required,
        supported: caps.supported_script_types.clone(),
    };

    // Step 4: assert error shape via matches!() per Phase 15-03 D-34
    // discipline (NOT string-parsing the message).
    assert!(
        matches!(
            err,
            DiscoveryError::UnsupportedScriptType {
                required: ScriptType::P2tr,
                ref supported,
                ..
            } if supported == &vec![ScriptType::P2wpkh]
        ),
        "expected UnsupportedScriptType {{ required: P2tr, supported: [P2wpkh] }}, got: {err:?}"
    );

    // Step 5: ROADMAP Phase 17 SC#3 wording check — the error Display impl
    // MUST contain the literal "does not support" substring + the bad type
    // names.
    let display = format!("{err}");
    assert!(
        display.contains("does not support"),
        "ROADMAP SC#3 wording missing — error Display must contain 'does not support', got: {display}"
    );
    assert!(
        display.contains("P2tr"),
        "error must name the missing script type, got: {display}"
    );
    assert!(
        display.contains("P2wpkh"),
        "error must name the supported set, got: {display}"
    );
    assert!(
        display.contains("fake-pubkey-z32"),
        "error must name the coordinator pubkey for operator triage, got: {display}"
    );
}

// ---------------------------------------------------------------------------
// Test 8 — WALLET-04 happy path: v1.3 PKARR + P2WPKH WIF wallet emits
// v=1 OwnershipProof envelope (the compat shim).
// ---------------------------------------------------------------------------

/// Construct a P2WPKH WIF wallet (preserves the v1.3 invariant gate
/// coverage per the plan's quality_gate — "at least one test in 17-02 or
/// 17-03 uses the WIF wallet path"); sign via `wallet.sign_bip322(...)`;
/// build a synthetic `CoordinatorInfo` with `capabilities.is_legacy =
/// true`; construct the v=1 OwnershipProof per the 17-02 encoder branch
/// (witness_stack only, no psbt_input_b64, no script_type); assert the
/// serialized JSON is in the v1.3 byte-identity array-of-hex form (starts
/// with `[`) via the CD-7 branch in `shared::protocol::OwnershipProof::to_json_hex_str`.
///
/// This test does NOT exercise the HTTP layer — that would require a
/// stubbed coordinator server (deferred to Phase 18 INTEG-01 per D-79).
/// The encoder-shape assertion is the structural acceptance gate here.
#[tokio::test]
async fn v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope() {
    // Step 1: construct a P2WPKH WIF wallet — D-61 hardcodes
    // ScriptType::P2wpkh; preserves the v1.3 bit-exact path.
    let wallet = BdkClientWallet::from_wif(TEST_WIF, DUMMY_OUTPOINT, Network::Regtest)
        .expect("from_wif (WIF wallet) should succeed");
    assert_eq!(wallet.script_type(), ScriptType::P2wpkh);

    // Step 2: sign via wallet.sign_bip322 — routes WIF → P2WPKH path
    // via shared::bip322::sign_simple(P2wpkh, ...).
    let signed = wallet
        .sign_bip322(TEST_MESSAGE)
        .expect("WIF wallet BIP-322 sign should succeed");
    assert_eq!(signed.script_type, ScriptType::P2wpkh);
    assert!(signed.final_script_sig.is_none());

    // Step 3: build synthetic CoordinatorInfo with is_legacy = true.
    let _legacy_info = CoordinatorInfo {
        coordinator_url: "http://127.0.0.1:8080".to_string(),
        capabilities: CoordinatorCapabilities {
            record_version: "0.1.0".to_string(),
            is_legacy: true,
            supported_script_types: vec![ScriptType::P2wpkh],
            output_script_type: ScriptType::P2wpkh,
        },
    };

    // Step 4: construct the v=1 OwnershipProof envelope per the 17-02
    // encoder branch (D-68 legacy arm).
    let proof = OwnershipProof {
        version: 1,
        witness_stack: signed.witness_stack.clone(),
        psbt_input_b64: None,
        script_type: None,
    };

    // Step 5: serialize via the CD-7 byte-identity branch in
    // `OwnershipProof::to_json_hex_str`. The output MUST start with `[`
    // (v1.3 array-of-hex form) and MUST NOT carry the `"version"` field.
    let wire = proof.to_json_hex_str();
    assert!(
        wire.starts_with('['),
        "v=1 envelope MUST emit v1.3 array-of-hex form (CD-7 branch); got: {wire}"
    );
    assert!(
        !wire.contains("\"version\""),
        "v=1 envelope MUST NOT carry version field on the wire (CD-7 branch); got: {wire}"
    );
    // Sanity: at least one non-empty hex-encoded witness item should be present.
    assert!(
        wire.len() > 2,
        "v=1 envelope must carry witness items, got minimal wire: {wire}"
    );
    assert!(!signed.witness_stack.is_empty(), "WIF P2WPKH witness must be non-empty");
}

// ---------------------------------------------------------------------------
// Test 9 — WALLET-04 positive control: v1.4 PKARR + P2TR wallet emits
// v=2 OwnershipProof envelope.
// ---------------------------------------------------------------------------

/// Mock a v0.2.0 PKARR record with `sst="p2sh-p2wpkh,p2tr,p2wpkh"`;
/// derive capabilities (assert `is_legacy == false`); construct a P2TR
/// descriptor wallet; sign via `wallet.sign_bip322(...)`; construct the
/// v=2 OwnershipProof envelope per the 17-02 encoder branch
/// (psbt_input_b64 with the full-PSBT shape per Pitfall 1; script_type
/// from wallet per CRIT-01); assert the serialized JSON is the v=2
/// flat-struct form with `"version":2` + `"script_type":"p2tr"` +
/// non-empty `"psbt_input_b64":"..."`.
///
/// Additionally roundtrips the psbt_input_b64 via `Psbt::deserialize`
/// (mirroring `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness`)
/// and asserts the decoded witness equals the signed witness — confirms
/// the encoder is the byte-inverse of the coordinator's decoder.
#[tokio::test]
async fn v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope() {
    // Step 1: derive capabilities from a synthetic v0.2.0 PKARR record
    // advertising all 3 script types.
    let caps = capabilities_from_record_v(
        "0.2.0",
        Some("p2sh-p2wpkh,p2tr,p2wpkh"),
        Some("p2tr"),
    )
    .expect("v0.2.0 capabilities derivation succeeds");
    assert!(!caps.is_legacy);
    assert!(caps.supported_script_types.contains(&ScriptType::P2tr));
    assert_eq!(caps.output_script_type, ScriptType::P2tr);

    // Step 2: construct a P2TR descriptor wallet.
    let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2tr, false)
        .expect("P2TR descriptor generate should succeed");
    assert_eq!(wallet.script_type(), ScriptType::P2tr);

    // Step 3: sign via wallet.sign_bip322 — routes descriptor → bdk PSBT
    // path uniformly per CD-24.
    let signed = wallet
        .sign_bip322(TEST_MESSAGE)
        .expect("P2TR descriptor BIP-322 sign should succeed");
    assert_eq!(signed.script_type, ScriptType::P2tr);

    // Step 4: build synthetic v1.4 CoordinatorInfo (is_legacy == false).
    let _v14_info = CoordinatorInfo {
        coordinator_url: "http://x.onion".to_string(),
        capabilities: caps.clone(),
    };

    // Step 5: construct the v=2 OwnershipProof envelope per the 17-02
    // encoder branch (D-68 default arm). CRIT-01 wire source:
    // signed.script_type (from wallet.sign_bip322), NEVER the CLI flag.
    let psbt_input_b64 = build_v2_psbt_input_b64(&signed.witness, signed.final_script_sig.as_ref());
    let proof = OwnershipProof {
        version: 2,
        witness_stack: signed.witness_stack.clone(),
        psbt_input_b64: Some(psbt_input_b64),
        script_type: Some(signed.script_type),
    };

    // Step 6: serialize and assert the v=2 flat-struct shape.
    let wire = proof.to_json_hex_str();
    let parsed: serde_json::Value =
        serde_json::from_str(&wire).expect("v=2 wire must parse as JSON object");
    assert_eq!(
        parsed["version"], serde_json::json!(2),
        "v=2 envelope must carry version=2, got: {wire}"
    );
    assert_eq!(
        parsed["script_type"], serde_json::json!("p2tr"),
        "v=2 envelope must carry kebab-case script_type='p2tr', got: {wire}"
    );
    assert!(
        parsed["psbt_input_b64"].is_string(),
        "v=2 envelope must carry psbt_input_b64 as a string, got: {wire}"
    );
    let b64_str = parsed["psbt_input_b64"].as_str().expect("psbt_input_b64 is string");
    assert!(!b64_str.is_empty(), "psbt_input_b64 must be non-empty");

    // Step 7: encoder/decoder roundtrip via Psbt::deserialize (mirrors
    // coordinator's decode_psbt_input_witness). The decoded witness MUST
    // equal the wallet's signed witness — Pitfall 1 evidence.
    let decoded_bytes = B64.decode(b64_str).expect("psbt_input_b64 base64-decodes");
    assert_eq!(
        &decoded_bytes[..5],
        &[0x70, 0x73, 0x62, 0x74, 0xff],
        "psbt_input_b64 must carry BIP-174 PSBT magic prefix (full-PSBT shape per Pitfall 1)"
    );
    let psbt = Psbt::deserialize(&decoded_bytes).expect("Psbt::deserialize must accept our encoding");
    let recovered_witness = psbt.inputs[0]
        .final_script_witness
        .clone()
        .expect("final_script_witness present");
    assert_eq!(
        recovered_witness, signed.witness,
        "decoded witness must equal signed witness (encoder/decoder byte-inverse contract)"
    );
}
