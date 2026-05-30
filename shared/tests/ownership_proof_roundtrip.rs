//! v1.4 Phase 15 Plan 15-01 — OwnershipProof wire-format roundtrip cases.
//!
//! The 5 D-13 cases + 1 sibling case for corrupted-base64 robustness.
//!
//! Ships as its OWN atomic commit per CD-10 / v1.3 REPAIR-01 lesson #1 BEFORE
//! Plan 15-02 introduces the `bip322` crate dep and the dispatcher API. This file
//! is intentionally crate-free (no `bip322`, no `thiserror`, no `proptest`) so
//! `git bisect` can identify a wire-format regression by walking back to this
//! commit cleanly without confounding the deps.
//!
//! D-13 case enumeration (CONTEXT verbatim):
//!   1. v2 self-roundtrip × 3 script types (P2WPKH, P2TR, P2SH-P2WPKH)
//!   2. v1 legacy decode of array-of-hex JSON
//!   3. unknown version (e.g. `version: 3`) → permissive decode (the verify-
//!      dispatch rejection layer lands in Plan 15-02 per D-25)
//!   4. (sibling) corrupted base64 in `psbt_input_b64` → permissive decode (the
//!      downstream base64-decode failure surfaces as Bip322Error::DecodeError in
//!      Plan 15-02 / Phase 16)

use shared::bip322::ScriptType;
use shared::protocol::OwnershipProof;

/// A small, syntactically valid base64 string used as a placeholder for the
/// `psbt_input_b64` field on the three v2 roundtrip cases. Plan 15-01 does NOT
/// decode this payload — Plan 15-02 / Phase 16 own the PSBT-input decode step
/// and surface base64 failures as Bip322Error::DecodeError. Six raw bytes
/// (`psbt` magic prefix `psbt\xff` then a trailing 0) is enough to look
/// realistic without claiming to be a valid PSBT byte stream.
const FIXTURE_PSBT_B64: &str = "cHNidP8BAAA=";

// --- D-13 Case 1: v2 self-roundtrip for all 3 script types -----------------

#[test]
fn v2_roundtrip_p2wpkh() {
    let proof = OwnershipProof {
        version: 2,
        witness_stack: vec![],
        psbt_input_b64: Some(FIXTURE_PSBT_B64.to_string()),
        script_type: Some(ScriptType::P2wpkh),
    };
    let json = proof.to_json_hex_str();
    // Sanity check: the encoded JSON carries both the version envelope and the
    // expected script_type wire string (snake_case "p2wpkh" per the Plan 15-01
    // ScriptType derive).
    assert!(
        json.contains("\"version\":2"),
        "encoded JSON missing version envelope: {json}"
    );
    assert!(
        json.contains("\"p2wpkh\""),
        "encoded JSON missing p2wpkh script_type tag: {json}"
    );

    let parsed = OwnershipProof::from_json_hex_str(&json).expect("v2 P2WPKH roundtrip decodes");
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.witness_stack, Vec::<Vec<u8>>::new());
    assert_eq!(parsed.psbt_input_b64.as_deref(), Some(FIXTURE_PSBT_B64));
    assert_eq!(parsed.script_type, Some(ScriptType::P2wpkh));
}

#[test]
fn v2_roundtrip_p2tr() {
    let proof = OwnershipProof {
        version: 2,
        witness_stack: vec![],
        psbt_input_b64: Some(FIXTURE_PSBT_B64.to_string()),
        script_type: Some(ScriptType::P2tr),
    };
    let json = proof.to_json_hex_str();
    assert!(
        json.contains("\"version\":2"),
        "encoded JSON missing version envelope: {json}"
    );
    assert!(
        json.contains("\"p2tr\""),
        "encoded JSON missing p2tr script_type tag: {json}"
    );

    let parsed = OwnershipProof::from_json_hex_str(&json).expect("v2 P2TR roundtrip decodes");
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.psbt_input_b64.as_deref(), Some(FIXTURE_PSBT_B64));
    assert_eq!(parsed.script_type, Some(ScriptType::P2tr));
}

#[test]
fn v2_roundtrip_p2sh_p2wpkh() {
    let proof = OwnershipProof {
        version: 2,
        witness_stack: vec![],
        psbt_input_b64: Some(FIXTURE_PSBT_B64.to_string()),
        script_type: Some(ScriptType::P2shP2wpkh),
    };
    let json = proof.to_json_hex_str();
    assert!(
        json.contains("\"version\":2"),
        "encoded JSON missing version envelope: {json}"
    );
    // KEY ASSERTION: the wire form uses kebab-case "p2sh-p2wpkh" — matches
    // ADVERT-02's wire shape per RESEARCH Open Question #3 RESOLVED. The
    // serde rename on the P2shP2wpkh variant carries this; if the rename is
    // removed the test goes red.
    assert!(
        json.contains("\"p2sh-p2wpkh\""),
        "encoded JSON missing kebab-case p2sh-p2wpkh script_type tag: {json}"
    );
    // Defensive: snake_case "p2sh_p2wpkh" MUST NOT appear (would indicate the
    // explicit rename was dropped and we fell back to rename_all = snake_case).
    assert!(
        !json.contains("\"p2sh_p2wpkh\""),
        "encoded JSON wrongly contains snake_case p2sh_p2wpkh: {json}"
    );

    let parsed =
        OwnershipProof::from_json_hex_str(&json).expect("v2 P2SH-P2WPKH roundtrip decodes");
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.psbt_input_b64.as_deref(), Some(FIXTURE_PSBT_B64));
    assert_eq!(parsed.script_type, Some(ScriptType::P2shP2wpkh));
}

// --- D-13 Case 2: v1 legacy array-of-hex JSON decode -----------------------

#[test]
fn v1_legacy_decode_array_of_hex() {
    // Bit-exact v1.3 wire form: a JSON array of hex strings (two witness items).
    // The CD-7 two-phase try-parse Phase 1 catches this and synthesizes an
    // OwnershipProof with version = 1, the decoded witness items, and both
    // Option fields = None.
    let v1_wire = r#"["3045022100abcd","02ab1234"]"#;
    let parsed = OwnershipProof::from_json_hex_str(v1_wire).expect("v1 array-of-hex must decode");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.witness_stack.len(), 2);
    assert_eq!(parsed.witness_stack[0], vec![0x30, 0x45, 0x02, 0x21, 0x00, 0xab, 0xcd]);
    assert_eq!(parsed.witness_stack[1], vec![0x02, 0xab, 0x12, 0x34]);
    assert!(parsed.psbt_input_b64.is_none());
    assert!(parsed.script_type.is_none());
}

// --- D-13 Case 3: unknown version → permissive decode ----------------------

#[test]
fn unknown_version_permissive_decode() {
    // version = 3 is NOT a known v1.4 envelope (1 = v1.3 shape, 2 = v1.4 PSBT
    // shape per ADR Decision #3). The decode layer is PERMISSIVE per D-25 — it
    // returns Ok with version = 3 and lets the verify-dispatch layer (lands in
    // Plan 15-02) reject with Bip322Error::UnsupportedProofVersion.
    let wire = r#"{"version":3,"witness_stack":[]}"#;
    let parsed =
        OwnershipProof::from_json_hex_str(wire).expect("permissive decode of unknown version");
    assert_eq!(parsed.version, 3);
    assert!(parsed.witness_stack.is_empty());
    assert!(parsed.psbt_input_b64.is_none());
    assert!(parsed.script_type.is_none());
    // NOTE: the version=3 → UnsupportedProofVersion rejection is exercised at
    // the verify-dispatch layer (Plan 15-02); this test confirms decode is
    // permissive at the wire-shape layer.
}

// --- Sibling case: corrupted base64 in psbt_input_b64 → permissive decode --

#[test]
fn corrupted_base64_in_psbt_input_permissive_decode() {
    // The JSON layer is permissive: it does not validate the base64 payload at
    // decode time. The downstream base64 decode step (Plan 15-02 dispatcher /
    // Phase 16 coordinator) is responsible for surfacing the failure as
    // Bip322Error::DecodeError. This test asserts the wire-decode layer does
    // NOT panic and surfaces the raw (corrupt) string as-is.
    let wire = r#"{"version":2,"psbt_input_b64":"not-base64-!!!","script_type":"p2wpkh"}"#;
    let parsed = OwnershipProof::from_json_hex_str(wire).expect("JSON decode itself is OK");
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.psbt_input_b64.as_deref(), Some("not-base64-!!!"));
    assert_eq!(parsed.script_type, Some(ScriptType::P2wpkh));
}
