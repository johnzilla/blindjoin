use serde::{Deserialize, Serialize};

use crate::bip322::ScriptType;

// NO #[serde(deny_unknown_fields)] on any struct — forward compat per D-06 / T-01-04.
// All structs silently drop unknown fields, allowing protocol evolution without breaking
// older clients or coordinators.

/// GET /info response — coordinator status and round parameters.
///
/// rsa_pubkey_hash: hex SHA-256(DER pubkey bytes) — client MUST verify
///   SHA-256(decode(rsa_pubkey_der_b64)) == rsa_pubkey_hash before blinding (D-02).
/// rsa_pubkey_der_b64: base64 DER SubjectPublicKeyInfo; None when coordinator is Idle
///   (no key has been generated yet for the upcoming round).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoResponse {
    pub version: String,
    pub network: String,
    pub denomination_sats: u64,
    pub min_participants: u32,
    pub max_participants: u32,
    /// "idle" | "input_reg" | "output_reg" | "signing" | "broadcast" | "blame".
    ///
    /// `"broadcast"` is a **transient in-flight** state (M5b): the round enters it the
    /// instant the final transaction is assembled — BEFORE the coordinator submits it
    /// to Bitcoin Core — and leaves it (to `"idle"` on success, or via `"blame"` on
    /// failure) once the broadcast RPC returns. It is normally observable for well
    /// under a second and never longer than the RPC timeout window; observing it does
    /// NOT by itself mean the CoinJoin was accepted by the network. Clients that need
    /// broadcast confirmation should watch the mempool/chain for the tx, not `/info`.
    pub round_state: String,
    pub participants_registered: u32,
    /// hex SHA-256(DER pubkey bytes); None when Idle
    pub rsa_pubkey_hash: Option<String>,
    /// base64 DER SubjectPublicKeyInfo; None when Idle
    pub rsa_pubkey_der_b64: Option<String>,
    pub round_id: Option<uuid::Uuid>,
    /// v1.4 ADVERT-01 wire-form extension (Phase 16 Plan 16-01 / D-42):
    /// canonical list of script types this coordinator currently accepts on
    /// input registration. Wire form: JSON array of kebab-case strings — e.g.
    /// `["p2sh-p2wpkh", "p2tr", "p2wpkh"]` (alphabetical canonical order per
    /// CD-11). Populated from `state.config.bip.supported()`.
    ///
    /// v1.3↔v1.4 bidirectional compat:
    /// - A v1.3 wire `InfoResponse` OMITS this field; `#[serde(default =
    ///   "default_legacy_supported")]` fires `vec![ScriptType::P2wpkh]` so a
    ///   v1.4 coordinator decoding a v1.3 response sees the legacy P2WPKH-only
    ///   set.
    /// - A v1.3 client decoding a v1.4 wire response silently drops the field
    ///   because `InfoResponse` does NOT carry `#[serde(deny_unknown_fields)]`
    ///   per the file-top invariant.
    #[serde(default = "default_legacy_supported")]
    pub supported_script_types: Vec<ScriptType>,
    /// v1.4 ADVERT-01 wire-form extension (Phase 16 Plan 16-01 / D-42):
    /// the single script type this coordinator will use for the round-output
    /// addresses it advertises (D-07 — one output script type per round, even
    /// when inputs are mixed per D-06). Populated from
    /// `state.config.bip.output_script_type`. Wire form: kebab-case string
    /// (e.g. `"p2wpkh"`, `"p2tr"`, `"p2sh-p2wpkh"`).
    ///
    /// v1.3↔v1.4 bidirectional compat: same scheme as
    /// `supported_script_types` — `#[serde(default = "default_legacy_output")]`
    /// fires `ScriptType::P2wpkh` when a v1.3 response is decoded.
    #[serde(default = "default_legacy_output")]
    pub output_script_type: ScriptType,
}

/// Legacy default for `InfoResponse.supported_script_types` — locks v1.3
/// wire-compat per D-42. A v1.3 coordinator emits no such field; a v1.4
/// decoder reads `vec![ScriptType::P2wpkh]` (the only script type v1.3
/// supported on the input-registration path).
fn default_legacy_supported() -> Vec<ScriptType> {
    vec![ScriptType::P2wpkh]
}

/// Legacy default for `InfoResponse.output_script_type` — locks v1.3
/// wire-compat per D-42. v1.3 only produced P2WPKH outputs.
fn default_legacy_output() -> ScriptType {
    ScriptType::P2wpkh
}

/// POST /round/input request — register a UTXO for an upcoming CoinJoin round.
///
/// ownership_proof uses canonical wire format: JSON array of hex strings.
/// Use OwnershipProof::from_json_hex_str / to_json_hex_str helpers — never ad-hoc encoding (T-01-05).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRegRequest {
    /// "txid:vout" format
    pub utxo_outpoint: String,
    /// Canonical wire format: JSON array of hex strings, e.g. "[\"3045...\",\"02ab...\"]"
    /// Coordinator decodes via OwnershipProof::from_json_hex_str.
    pub ownership_proof: String,
    /// base64-encoded blinded message
    pub blinded_token: String,
    /// bech32 address for change output (linkable, documented)
    pub change_address: String,
}

/// POST /round/input response — blind signature and session token for this registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRegResponse {
    /// base64-encoded blind signature
    pub blind_signature: String,
    pub round_id: uuid::Uuid,
    /// base64-encoded [u8; 32] HMAC session token (D-05)
    pub session_token: String,
}

/// POST /round/output request — register the CoinJoin output using the unblinded token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRegRequest {
    /// base64-encoded original message M (the 32-byte hash from compute_blind_token_message)
    pub unblinded_token: String,
    /// base64-encoded unblinded RSA signature
    pub signature: String,
    /// bech32 address for CoinJoin output
    pub output_address: String,
    pub amount_sats: u64,
    /// base64-encoded 32-byte MessageRandomizer from BlindingResult (required for Randomized mode).
    /// The client obtains this from BlindingResult.msg_randomizer during the blinding step.
    /// RSABSSA-SHA384-PSS-Randomized (RFC 9474 §3.3.2) requires this for signature verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_randomizer: Option<String>,
}

/// POST /round/output response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRegResponse {
    pub accepted: bool,
    pub round_id: uuid::Uuid,
}

/// POST /round/sign request — submit a partial PSBT signature.
///
/// Uses utxo_outpoint (NOT input_index) per design doc correction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    pub round_id: uuid::Uuid,
    /// "txid:vout" — NOT input_index (design doc correction)
    pub utxo_outpoint: String,
    /// base64-encoded PSBT partial sig bytes
    pub partial_signature: String,
    /// base64-encoded session token from InputRegResponse (D-05)
    pub session_token: String,
}

/// GET /round/tx response — the assembled PSBT for participant signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundTxResponse {
    pub round_id: uuid::Uuid,
    /// base64-encoded PSBT
    pub psbt: String,
    pub fee_total_sats: u64,
    pub fee_per_participant_sats: u64,
}

/// Canonical wire representation of a BIP-322 ownership proof — v1.4 v2 envelope.
///
/// v1.4 Phase 15 (Plan 15-01) evolves this from the v1.3 two-field witness-only
/// shape to the four-field flat envelope per ADR Decision #3 + CONTEXT D-22..D-25.
/// The struct is a SINGLE flat struct (NOT a serde tagged enum — ADR Decision #3
/// explicitly rejected B1) with an explicit `version: u8` envelope:
///
/// - `version = 1` — v1.3 witness-only path. `psbt_input_b64` and `script_type`
///   are both `None`. Encode emits the v1.3 array-of-hex JSON wire form per CD-7
///   so v1.3 coordinators that have not yet read this struct's flat-struct shape
///   still decode bit-exactly.
/// - `version = 2` — v1.4 PSBT-input path. `psbt_input_b64` carries
///   `base64(bitcoin::psbt::Input)` and `script_type` carries the client-declared
///   script type (sibling field per D-24 — NOT inferred from PSBT contents).
///   Encode emits the flat-struct JSON.
///
/// `version` is permissive at decode (unknown versions deserialise per D-25); the
/// verify dispatch layer (Plan 15-02) is responsible for rejecting `version >= 3`
/// with `UnsupportedProofVersion`. The InputRegRequest wire transport stays a
/// `String` (D-23 — preserves T-01-05 "never pass raw bytes" invariant).
///
/// Two-phase try-parse per CD-7: `from_json_hex_str` first attempts the v1.3
/// array-of-hex shape, then falls back to the flat-struct shape — preserves
/// bit-exact v1.3 wire compatibility for the cross-phase invariant.
///
/// NO `#[serde(deny_unknown_fields)]` per T-01-04 / D-06 (file-top invariant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipProof {
    /// Wire-format version. Defaults to 1 (v1.3 shape) when absent per D-25 so
    /// v1.3 clients that omit the field deserialise as `version = 1`.
    #[serde(default = "default_proof_version")]
    pub version: u8,
    /// Raw witness stack items. v1 path uses this exclusively; v2 path leaves
    /// it empty (the witness lives inside the base64-encoded PSBT input).
    #[serde(default)]
    pub witness_stack: Vec<Vec<u8>>,
    /// v1.4 v2 path: base64(bitcoin::psbt::Input). None on the v1 path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psbt_input_b64: Option<String>,
    /// v1.4 v2 path: client-declared script type (sibling envelope field per
    /// D-24, NOT inferred from PSBT contents). Coordinator (Phase 16) cross-
    /// checks against `detect_script_type(on_chain_spk)` per CRIT-01.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_type: Option<crate::bip322::ScriptType>,
}

/// Default for `OwnershipProof.version` — locks v1.3 wire-compat per D-25.
fn default_proof_version() -> u8 {
    1
}

impl OwnershipProof {
    /// Decode from the v1.4 wire envelope, accepting BOTH legacy shapes per CD-7.
    ///
    /// Phase 1: try `serde_json::from_str::<Vec<String>>` (v1.3 array-of-hex
    /// shape). On success, hex-decode each item into `witness_stack` and return
    /// `Self { version: 1, witness_stack, psbt_input_b64: None, script_type: None }`.
    ///
    /// Phase 2: fall back to `serde_json::from_str::<Self>` (flat-struct shape,
    /// covers v1-explicit envelopes and v2 envelopes).
    ///
    /// Return type stays `Result<Self, String>` per RESEARCH Pitfall 7 — typing
    /// this as `Bip322Error` would force `protocol.rs` to import from
    /// `shared::bip322` for the typed error and create a module cycle. The typed
    /// `Bip322Error` lives at the verify-dispatch layer (Plan 15-02).
    pub fn from_json_hex_str(s: &str) -> Result<Self, String> {
        // Phase 1: legacy v1.3 array-of-hex shape.
        if let Ok(items) = serde_json::from_str::<Vec<String>>(s) {
            let witness_stack = items
                .iter()
                .map(|h| hex::decode(h).map_err(|e| format!("OwnershipProof: hex decode error: {e}")))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Self {
                version: 1,
                witness_stack,
                psbt_input_b64: None,
                script_type: None,
            });
        }
        // Phase 2: flat-struct shape (covers both v1-explicit and v2 envelopes).
        serde_json::from_str::<Self>(s)
            .map_err(|e| format!("OwnershipProof: JSON parse error: {e}"))
    }

    /// Encode to the wire envelope, emitting the v1.3 array-of-hex shape when
    /// `version == 1 && psbt_input_b64.is_none() && script_type.is_none()` per
    /// CD-7 — preserves bit-exact v1.3 wire compatibility (cross-phase invariant).
    /// Otherwise emits the flat-struct JSON (covers explicit v1 envelopes carrying
    /// the v2 fields and v2 envelopes).
    pub fn to_json_hex_str(&self) -> String {
        if self.version == 1 && self.psbt_input_b64.is_none() && self.script_type.is_none() {
            let hex_items: Vec<String> = self.witness_stack.iter().map(hex::encode).collect();
            return serde_json::to_string(&hex_items).expect("Vec<String> always serializes");
        }
        serde_json::to_string(self).expect("OwnershipProof serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bip322::ScriptType;

    #[test]
    fn default_via_serde_from_empty_object() {
        // D-25: missing version defaults to 1; missing fields all default to None / empty.
        let proof: OwnershipProof = serde_json::from_str("{}").unwrap();
        assert_eq!(proof.version, 1);
        assert!(proof.witness_stack.is_empty());
        assert!(proof.psbt_input_b64.is_none());
        assert!(proof.script_type.is_none());
    }

    #[test]
    fn v2_flat_struct_roundtrip_via_serde_json() {
        let raw = r#"{"version":2,"witness_stack":[],"psbt_input_b64":"AA==","script_type":"p2wpkh"}"#;
        let proof: OwnershipProof = serde_json::from_str(raw).unwrap();
        assert_eq!(proof.version, 2);
        assert!(proof.witness_stack.is_empty());
        assert_eq!(proof.psbt_input_b64.as_deref(), Some("AA=="));
        assert_eq!(proof.script_type, Some(ScriptType::P2wpkh));
        // Round-trip: re-encode and re-decode.
        let encoded = serde_json::to_string(&proof).unwrap();
        let proof2: OwnershipProof = serde_json::from_str(&encoded).unwrap();
        assert_eq!(proof2.version, 2);
        assert_eq!(proof2.script_type, Some(ScriptType::P2wpkh));
    }

    #[test]
    fn to_json_hex_str_emits_v1_array_when_version1_and_no_v2_fields() {
        // CD-7: v1 wire-compat branch.
        let proof = OwnershipProof {
            version: 1,
            witness_stack: vec![vec![0x30, 0x45], vec![0x02, 0xab]],
            psbt_input_b64: None,
            script_type: None,
        };
        let wire = proof.to_json_hex_str();
        assert_eq!(wire, r#"["3045","02ab"]"#);
    }

    #[test]
    fn from_json_hex_str_decodes_v1_array_of_hex() {
        let wire = r#"["3045022100abcd","02ab1234"]"#;
        let proof = OwnershipProof::from_json_hex_str(wire).expect("v1 wire decodes");
        assert_eq!(proof.version, 1);
        assert_eq!(proof.witness_stack.len(), 2);
        assert!(proof.psbt_input_b64.is_none());
        assert!(proof.script_type.is_none());
    }

    #[test]
    fn from_json_hex_str_decodes_v2_flat_struct() {
        let wire =
            r#"{"version":2,"witness_stack":[],"psbt_input_b64":"AA==","script_type":"p2tr"}"#;
        let proof = OwnershipProof::from_json_hex_str(wire).expect("v2 wire decodes");
        assert_eq!(proof.version, 2);
        assert_eq!(proof.psbt_input_b64.as_deref(), Some("AA=="));
        assert_eq!(proof.script_type, Some(ScriptType::P2tr));
    }

    #[test]
    fn from_json_hex_str_is_permissive_on_unknown_version() {
        // D-25: decode is permissive; verify-dispatch (Plan 15-02) rejects.
        let wire = r#"{"version":3,"witness_stack":[]}"#;
        let proof = OwnershipProof::from_json_hex_str(wire).expect("decode is permissive");
        assert_eq!(proof.version, 3);
    }

    // ---------- Phase 16 Plan 16-01 Task 2: InfoResponse v1.3↔v1.4 compat ----------

    /// v1.4 coordinator decoding a v1.3 wire response: the 2 new fields are
    /// absent, so the serde_default hooks fire and produce legacy P2WPKH-only
    /// values. This is the load-bearing forward-compat invariant for D-42.
    #[test]
    fn info_response_v1_3_wire_decodes_with_legacy_defaults() {
        let v1_3_wire = r#"{
            "version": "0.1.0",
            "network": "signet",
            "denomination_sats": 1000000,
            "min_participants": 3,
            "max_participants": 20,
            "round_state": "idle",
            "participants_registered": 0,
            "rsa_pubkey_hash": null,
            "rsa_pubkey_der_b64": null,
            "round_id": null
        }"#;
        let info: InfoResponse =
            serde_json::from_str(v1_3_wire).expect("v1.3 wire must decode under v1.4");
        assert_eq!(info.supported_script_types, vec![ScriptType::P2wpkh]);
        assert_eq!(info.output_script_type, ScriptType::P2wpkh);
        // Existing fields preserved
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.network, "signet");
        assert_eq!(info.denomination_sats, 1_000_000);
        assert_eq!(info.round_state, "idle");
    }

    /// v1.4 coordinator emitting + re-decoding a fully-populated v1.4
    /// InfoResponse round-trips bit-exact on the 2 new fields.
    #[test]
    fn info_response_v1_4_roundtrip_preserves_new_fields() {
        let original = InfoResponse {
            version: "0.2.0".to_string(),
            network: "signet".to_string(),
            denomination_sats: 1_000_000,
            min_participants: 3,
            max_participants: 20,
            round_state: "input_reg".to_string(),
            participants_registered: 2,
            rsa_pubkey_hash: None,
            rsa_pubkey_der_b64: None,
            round_id: None,
            supported_script_types: vec![
                ScriptType::P2shP2wpkh,
                ScriptType::P2tr,
                ScriptType::P2wpkh,
            ],
            output_script_type: ScriptType::P2tr,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: InfoResponse =
            serde_json::from_str(&json).expect("re-decode v1.4 wire");
        assert_eq!(
            decoded.supported_script_types,
            vec![
                ScriptType::P2shP2wpkh,
                ScriptType::P2tr,
                ScriptType::P2wpkh,
            ]
        );
        assert_eq!(decoded.output_script_type, ScriptType::P2tr);
    }

    /// v1.4 wire JSON emits kebab-case for the P2SH-P2WPKH variant per Phase
    /// 15's ScriptType serde rename. This locks the ADVERT-02 wire form on
    /// the new InfoResponse fields.
    #[test]
    fn info_response_v1_4_emits_kebab_case_on_wire() {
        let info = InfoResponse {
            version: "0.2.0".to_string(),
            network: "signet".to_string(),
            denomination_sats: 1_000_000,
            min_participants: 3,
            max_participants: 20,
            round_state: "idle".to_string(),
            participants_registered: 0,
            rsa_pubkey_hash: None,
            rsa_pubkey_der_b64: None,
            round_id: None,
            supported_script_types: vec![
                ScriptType::P2shP2wpkh,
                ScriptType::P2tr,
                ScriptType::P2wpkh,
            ],
            output_script_type: ScriptType::P2shP2wpkh,
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(
            json.contains("\"p2sh-p2wpkh\""),
            "kebab-case rename must appear on the wire: {json}"
        );
        // Also assert the output_script_type field name appears (defensive
        // — catches an accidental serde(skip) or rename on the field).
        assert!(json.contains("\"output_script_type\""), "field name missing: {json}");
        assert!(
            json.contains("\"supported_script_types\""),
            "field name missing: {json}"
        );
    }

    /// v1.3 client decoding a v1.4 wire response: the 2 new fields are present
    /// but the v1.3 struct doesn't know about them. The file-top invariant
    /// (no `#[serde(deny_unknown_fields)]`) means unknown fields are silently
    /// dropped, so deserialization succeeds. This locks T-16-MOD-01 wire-format
    /// evolution mitigation.
    #[test]
    fn info_response_v1_3_decoder_against_v1_4_wire_tolerates_extras() {
        // Local shadow struct mirroring the v1.3 shape (10 existing fields
        // only — does NOT include supported_script_types / output_script_type).
        // The name uses v1_3 (snake_case) to keep the v1.3-vs-v1.4 narrative
        // visually obvious; allow the non-camel-case lint locally.
        #[allow(non_camel_case_types)]
        #[derive(Debug, Deserialize)]
        struct V1_3_InfoResponse {
            version: String,
            network: String,
            denomination_sats: u64,
            min_participants: u32,
            max_participants: u32,
            round_state: String,
            participants_registered: u32,
            rsa_pubkey_hash: Option<String>,
            rsa_pubkey_der_b64: Option<String>,
            round_id: Option<uuid::Uuid>,
        }

        let v1_4_wire = serde_json::to_string(&InfoResponse {
            version: "0.2.0".to_string(),
            network: "signet".to_string(),
            denomination_sats: 1_000_000,
            min_participants: 3,
            max_participants: 20,
            round_state: "idle".to_string(),
            participants_registered: 0,
            rsa_pubkey_hash: None,
            rsa_pubkey_der_b64: None,
            round_id: None,
            supported_script_types: vec![
                ScriptType::P2shP2wpkh,
                ScriptType::P2tr,
                ScriptType::P2wpkh,
            ],
            output_script_type: ScriptType::P2tr,
        })
        .expect("serialize v1.4");

        let v1_3: V1_3_InfoResponse =
            serde_json::from_str(&v1_4_wire).expect("v1.3 decoder must tolerate extras");
        // Existing fields preserved; new fields silently dropped.
        assert_eq!(v1_3.version, "0.2.0");
        assert_eq!(v1_3.network, "signet");
        assert_eq!(v1_3.denomination_sats, 1_000_000);
        assert_eq!(v1_3.round_state, "idle");
        // Silence the dead-code lint for the otherwise-unused parsed-only
        // fields; the test's purpose is decode-without-error, not field
        // population verification beyond the spot-checks above.
        let _ = (
            v1_3.min_participants,
            v1_3.max_participants,
            v1_3.participants_registered,
            &v1_3.rsa_pubkey_hash,
            &v1_3.rsa_pubkey_der_b64,
            &v1_3.round_id,
        );
    }
}
