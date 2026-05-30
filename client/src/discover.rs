//! PKARR resolver layer — Phase 17 17-03 (WALLET-03 + WALLET-04 discovery).
//!
//! Replaces the v1.3 `parse_onion_from_rr` + `discover_coordinator` pair with
//! the extended `parse_blindjoin_record` decoder, `CoordinatorCapabilities`
//! struct, and typed `DiscoveryError` enum.
//!
//! **WALLET-03 (pre-Tor fail-fast):** `discover_coordinator` takes the
//! caller's wallet `ScriptType` as `required_script_type`; rejects the
//! coordinator at the resolver boundary (BEFORE any Tor circuit opens) on
//! either an input-script-type allowlist miss (`UnsupportedScriptType`) OR an
//! output-script-type mismatch (`UnsupportedOutputScriptType` per CD-23
//! split-variant discipline + D-76).
//!
//! **WALLET-04 (compat-shim discovery side):** detects pre-`0.2.0` PKARR
//! records via `record.version != "0.2.0" || record.sst.is_none()` →
//! `capabilities.is_legacy == true`. Downstream `register_input` reads
//! `info.capabilities.is_legacy` to select between the v=1 OwnershipProof
//! envelope (byte-identical to v1.3 via the CD-7 branch) and the v=2
//! envelope (default for v1.4 coordinators).
//!
//! **Pitfall 5 correction (load-bearing):** the PKARR record's version field
//! is named `v` on the wire (NOT `version`). Phase 16-03 commit `d1a1912`
//! compactified the wire field name for byte-budget reasons. The
//! `#[serde(rename = "v", default = "default_legacy_version")]` annotation is
//! the LOAD-BEARING discipline — without it every v1.4 coordinator would
//! silently appear legacy on every connection, breaking WALLET-04 in the
//! wrong direction. Source of truth:
//! `coordinator/src/discovery/pkarr_pub.rs::build_coordinator_packet`.

use pkarr::{Client, PublicKey};
use serde::Deserialize;
use shared::bip322::ScriptType;

/// Coordinator info returned by `discover_coordinator`. Phase 17 17-03 D-71
/// extends with `capabilities: CoordinatorCapabilities`.
#[derive(Debug, Clone)]
pub struct CoordinatorInfo {
    pub coordinator_url: String,
    pub capabilities: CoordinatorCapabilities,
}

/// Capability flags derived from the PKARR `_blindjoin` record.
///
/// Phase 17 17-03 D-71 + CD-21 (public struct — `main.rs` reads
/// `info.capabilities.is_legacy` directly to log the legacy-coordinator WARN
/// + `register_input` reads it to select the v1/v2 envelope branch).
#[derive(Debug, Clone)]
pub struct CoordinatorCapabilities {
    /// Wire-form schema version. `"0.1.0"` for v1.3 records (or any record
    /// missing the `v` field); `"0.2.0"` for v1.4 records (Phase 16-03
    /// onward). `"manual"` for the non-PKARR `--coordinator-url` direct path
    /// (operator out-of-band trust per T-17-03-05).
    pub record_version: String,
    /// True iff `record_version != "0.2.0"` OR `sst.is_none()`. Either
    /// condition fires the WALLET-04 compat shim — defensive against
    /// partial v0.2.0 records that bumped `v` but failed to populate `sst`.
    pub is_legacy: bool,
    /// Script types the coordinator accepts for input registration. Legacy
    /// records default to `vec![ScriptType::P2wpkh]` (v1.3 was P2WPKH-only).
    /// v1.4 records parse from the CSV `sst` field; preserved in the order
    /// the coordinator advertised (alphabetical canonical per CD-11 from the
    /// emit side, but the resolver does NOT re-sort).
    pub supported_script_types: Vec<ScriptType>,
    /// Single script type the coordinator's CoinJoin output will use.
    /// Legacy records default to `ScriptType::P2wpkh`. Per D-07 mixed-output
    /// is OUT OF SCOPE; the wallet's output type must match this.
    pub output_script_type: ScriptType,
}

/// Typed errors from `discover_coordinator`.
///
/// Phase 17 17-03 D-72 + CD-23 (split `UnsupportedScriptType` vs
/// `UnsupportedOutputScriptType` for user-actionable diagnostics — the
/// fix differs by mismatch side).
///
/// **PII discipline:** every variant names ONLY public DHT data
/// (pubkey z32 string), `ScriptType` enum values (public protocol), and
/// structural reason strings. No IP, no UTXO outpoint, no key bytes,
/// no amounts. Symmetric with `coordinator/src/bitcoin/utxo.rs::UtxoError`
/// (PRIV-02 discipline).
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("Invalid PKARR public key: {0}")]
    InvalidPubkey(String),
    #[error("Coordinator not found in DHT for key '{pubkey}'")]
    NotFound { pubkey: String },
    #[error("No 'onion' field found in PKARR record for key '{pubkey}'")]
    MissingOnion { pubkey: String },
    #[error("Malformed PKARR record: {reason}")]
    MalformedRecord { reason: String },
    /// **ROADMAP SC#3 wording — load-bearing:** the literal substring
    /// `does not support` is asserted by `multi_script_client::v13_pkarr_record_with_p2tr_wallet_rejects_before_tor`.
    #[error("coordinator {pubkey} does not support {required:?} ownership proofs (supports: {supported:?})")]
    UnsupportedScriptType {
        pubkey: String,
        required: ScriptType,
        supported: Vec<ScriptType>,
    },
    #[error("coordinator {pubkey} CoinJoin output is {advertised:?} but wallet requires {wanted:?}")]
    UnsupportedOutputScriptType {
        pubkey: String,
        advertised: ScriptType,
        wanted: ScriptType,
    },
}

/// PKARR `_blindjoin` TXT record decoded shape.
///
/// **Pitfall 5 — LOAD-BEARING — DO NOT REMOVE THE `rename = "v"` ANNOTATION.**
/// Phase 16-03 commit `d1a1912` (2026-05-30) compactified the PKARR wire
/// field name from `version` to `v` for byte-budget reasons. A struct without
/// the rename would miss the field on every v1.4 coordinator record, the
/// `#[serde(default)]` would substitute `"0.1.0"`, and EVERY v1.4
/// coordinator would falsely appear as legacy on every connection. The
/// compat shim would fire universally; WALLET-04 would silently break in
/// the WRONG direction (v1.4 client emits v=1 envelopes to v1.4 coordinators
/// expecting v=2, the v=1 arm validates against the wrong sighash math).
///
/// Source of truth: `coordinator/src/discovery/pkarr_pub.rs:89-108`
/// (lines `"v": "0.2.0", "onion": ..., "sst": supported.join(","), "ost": output_script_type`).
#[derive(Debug, Deserialize)]
struct BlindjoinRecord {
    #[serde(rename = "v", default = "default_legacy_version")]
    version: String,
    onion: String,
    #[serde(default)]
    sst: Option<String>,
    #[serde(default)]
    ost: Option<String>,
}

fn default_legacy_version() -> String {
    "0.1.0".to_string()
}

/// Parse a single `ScriptType` token via the LOCKED Phase 15 serde wire form
/// (`#[serde(rename_all = "snake_case")]` + `#[serde(rename = "p2sh-p2wpkh")]`
/// on the `P2shP2wpkh` variant). Mirrors `client/src/config.rs::parse_script_type`
/// (the CLI flag parser) — the enum's serde impl is the single source of truth
/// for accepted tokens.
fn parse_script_type_token(s: &str) -> Result<ScriptType, DiscoveryError> {
    let quoted = format!("\"{}\"", s);
    serde_json::from_str::<ScriptType>(&quoted).map_err(|_| DiscoveryError::MalformedRecord {
        reason: format!("invalid sst/ost token '{s}'"),
    })
}

/// Derive `CoordinatorCapabilities` from a decoded `BlindjoinRecord`. Public
/// (with `#[doc(hidden)]`) so `tests/integration/multi_script_client.rs`
/// can exercise the legacy/v1.4 derivation branches without a live DHT
/// roundtrip — mirrors Phase 16-02's same Rule-3 visibility escalation on
/// `coordinator::bitcoin::utxo::validate_ownership_proof_typed`.
#[doc(hidden)]
pub fn capabilities_from_record_v(
    record_version: &str,
    sst: Option<&str>,
    ost: Option<&str>,
) -> Result<CoordinatorCapabilities, DiscoveryError> {
    // is_legacy fires on EITHER condition — defensive against partial
    // v0.2.0 records that bumped `v` without populating `sst`.
    let is_legacy = record_version != "0.2.0" || sst.is_none();

    if is_legacy {
        return Ok(CoordinatorCapabilities {
            record_version: record_version.to_string(),
            is_legacy: true,
            supported_script_types: vec![ScriptType::P2wpkh],
            output_script_type: ScriptType::P2wpkh,
        });
    }

    // v0.2.0 record: parse the CSV `sst` and the scalar `ost`.
    let sst_str = sst.expect("is_legacy false implies sst.is_some()");
    let supported_script_types: Vec<ScriptType> = sst_str
        .split(',')
        .map(parse_script_type_token)
        .collect::<Result<Vec<_>, _>>()?;
    if supported_script_types.is_empty() {
        return Err(DiscoveryError::MalformedRecord {
            reason: "v=0.2.0 record has empty sst CSV".to_string(),
        });
    }

    let output_script_type = match ost {
        Some(s) => parse_script_type_token(s)?,
        None => {
            return Err(DiscoveryError::MalformedRecord {
                reason: "v=0.2.0 record missing ost field".to_string(),
            });
        }
    };

    Ok(CoordinatorCapabilities {
        record_version: record_version.to_string(),
        is_legacy: false,
        supported_script_types,
        output_script_type,
    })
}

/// Extract a `BlindjoinRecord` from a DNS TXT resource record.
///
/// Phase 17 17-03 replacement for the v1.3 `parse_onion_from_rr`. Mirrors
/// the existing decode shape (`RData::TXT(txt) => txt; String::try_from;
/// serde_json::from_str`) — the only delta is the richer struct shape +
/// the Pitfall 5 `rename = "v"` annotation.
fn parse_blindjoin_record(rr: &pkarr::dns::ResourceRecord<'_>) -> Option<BlindjoinRecord> {
    use pkarr::dns::rdata::RData;
    let RData::TXT(txt) = &rr.rdata else { return None };
    let s = String::try_from(txt.clone()).ok()?;
    serde_json::from_str::<BlindjoinRecord>(&s).ok()
}

/// Resolve a coordinator URL + capabilities from a PKARR public key string
/// (z32 format), enforcing WALLET-03 pre-Tor fail-fast on script-type
/// mismatch.
///
/// The fail-fast runs INSIDE the resolver, BEFORE returning, so callers
/// cannot accidentally bypass it. Per RESEARCH Pitfall 4 the structural
/// pre-Tor ordering is enforced by code position at `client/src/main.rs`
/// (discover call site at ~line 58 runs UNCONDITIONALLY before the
/// `if cfg.use_tor` branch at ~line 67) — see the inline comment at the
/// call site documenting D-74.
///
/// **Latency:** 500ms–2s typical (PKARR `resolve_most_recent` forces a
/// fresh DHT query, no stale cache). Acceptable for one-time coordinator
/// discovery.
pub async fn discover_coordinator(
    pkarr_pubkey: &str,
    required_script_type: ScriptType,
) -> Result<CoordinatorInfo, DiscoveryError> {
    // (a) Validate pubkey
    let public_key: PublicKey = pkarr_pubkey
        .try_into()
        .map_err(|e: pkarr::errors::PublicKeyError| {
            DiscoveryError::InvalidPubkey(format!("'{pkarr_pubkey}': {e}"))
        })?;

    // (b) Build pkarr client + resolve_most_recent
    let client = Client::builder()
        .build()
        .map_err(|e| DiscoveryError::MalformedRecord {
            reason: format!("Failed to build PKARR client: {e}"),
        })?;
    let packet = client
        .resolve_most_recent(&public_key)
        .await
        .ok_or_else(|| DiscoveryError::NotFound {
            pubkey: pkarr_pubkey.to_string(),
        })?;

    // (c) Extract the first _blindjoin TXT record decoded as BlindjoinRecord.
    // A record missing the (non-optional) `onion` field fails serde decode →
    // find_map returns None → MissingOnion. The error name carries the v1.3
    // "no onion field" semantic for backwards-compatibility of operator
    // troubleshooting.
    let record = packet
        .resource_records("_blindjoin")
        .find_map(parse_blindjoin_record)
        .ok_or_else(|| DiscoveryError::MissingOnion {
            pubkey: pkarr_pubkey.to_string(),
        })?;

    // (d) Derive capabilities (handles v0.1.0 vs v0.2.0 + sst CSV parsing).
    let capabilities = capabilities_from_record_v(
        &record.version,
        record.sst.as_deref(),
        record.ost.as_deref(),
    )?;

    // (e) WALLET-03 fail-fast: input script-type allowlist miss.
    if !capabilities.supported_script_types.contains(&required_script_type) {
        return Err(DiscoveryError::UnsupportedScriptType {
            pubkey: pkarr_pubkey.to_string(),
            required: required_script_type,
            supported: capabilities.supported_script_types.clone(),
        });
    }

    // (f) D-76 sibling check: output script-type mismatch ALSO fails fast.
    // Per CD-23 split for actionable diagnostics — the user-facing fix
    // differs (input-mismatch user picks a different coordinator OR
    // generates a different-script wallet; output-mismatch user picks a
    // coordinator with matching `ost` OR generates a wallet with the
    // matching descriptor).
    if capabilities.output_script_type != required_script_type {
        return Err(DiscoveryError::UnsupportedOutputScriptType {
            pubkey: pkarr_pubkey.to_string(),
            advertised: capabilities.output_script_type,
            wanted: required_script_type,
        });
    }

    // (g) Build coordinator_url with http:// prefix (carried unchanged from v1.3).
    let coordinator_url = if record.onion.starts_with("http") {
        record.onion.clone()
    } else {
        format!("http://{}", record.onion)
    };

    // (h) Log resolution (no PII — coordinator_url is public DHT data).
    tracing::info!(
        coordinator_url = %coordinator_url,
        record_version = %capabilities.record_version,
        is_legacy = capabilities.is_legacy,
        "Resolved coordinator via PKARR DHT"
    );

    Ok(CoordinatorInfo {
        coordinator_url,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: v0.2.0 record decodes via the compact `v` field name AND
    /// preserves the CSV `sst` + scalar `ost`.
    #[test]
    fn parse_blindjoin_record_decodes_v0_2_0_record_with_sst_and_ost() {
        let json = r#"{"type":"blindjoin-coordinator","v":"0.2.0","onion":"x.onion","sst":"p2sh-p2wpkh,p2tr,p2wpkh","ost":"p2wpkh"}"#;
        let r: BlindjoinRecord = serde_json::from_str(json).expect("valid v0.2.0 record");
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.onion, "x.onion");
        assert_eq!(r.sst.as_deref(), Some("p2sh-p2wpkh,p2tr,p2wpkh"));
        assert_eq!(r.ost.as_deref(), Some("p2wpkh"));
    }

    /// Test 2: v0.1.0 record (no `v`/`sst`/`ost`) decodes with the legacy
    /// `version = "0.1.0"` default. Demonstrates the Pitfall 5 default branch.
    #[test]
    fn parse_blindjoin_record_decodes_legacy_v0_1_0_record_via_default() {
        let json = r#"{"onion":"y.onion"}"#;
        let r: BlindjoinRecord = serde_json::from_str(json).expect("valid v0.1.0 record");
        assert_eq!(r.version, "0.1.0", "default_legacy_version must fire when `v` absent");
        assert_eq!(r.onion, "y.onion");
        assert!(r.sst.is_none());
        assert!(r.ost.is_none());
    }

    /// Test 3: explicit regression test for Pitfall 5 — the compact `v` field
    /// wins over a legacy `version` field. WITHOUT the rename annotation,
    /// serde would parse `version: "BOGUS"` and the v1.4 coordinator would
    /// look like a legacy one.
    #[test]
    fn parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field() {
        let json = r#"{"version":"BOGUS","v":"0.2.0","onion":"z.onion"}"#;
        let r: BlindjoinRecord = serde_json::from_str(json).expect("valid mixed record");
        assert_eq!(
            r.version, "0.2.0",
            "Pitfall 5: rename = \"v\" must make `v` (compact) win over `version` (legacy)"
        );
    }

    /// Test 4: legacy record → is_legacy=true + P2WPKH-only defaults.
    #[test]
    fn capabilities_is_legacy_true_for_v0_1_0() {
        let caps = capabilities_from_record_v("0.1.0", None, None).expect("legacy ok");
        assert!(caps.is_legacy);
        assert_eq!(caps.record_version, "0.1.0");
        assert_eq!(caps.supported_script_types, vec![ScriptType::P2wpkh]);
        assert_eq!(caps.output_script_type, ScriptType::P2wpkh);
    }

    /// Test 5: v0.2.0 record with sst+ost → is_legacy=false + parsed types.
    #[test]
    fn capabilities_is_legacy_false_for_v0_2_0_with_sst() {
        let caps = capabilities_from_record_v("0.2.0", Some("p2tr,p2wpkh"), Some("p2tr"))
            .expect("v0.2.0 ok");
        assert!(!caps.is_legacy);
        assert_eq!(caps.record_version, "0.2.0");
        // Preserved in declared order (alphabetical canonical per CD-11
        // from the emit side; resolver does NOT re-sort).
        assert_eq!(
            caps.supported_script_types,
            vec![ScriptType::P2tr, ScriptType::P2wpkh]
        );
        assert_eq!(caps.output_script_type, ScriptType::P2tr);
    }

    /// Test 6: malformed sst token → MalformedRecord with the bad token named.
    #[test]
    fn capabilities_returns_malformed_record_for_invalid_sst_token() {
        let err = capabilities_from_record_v("0.2.0", Some("p2tr,invalid-token,p2wpkh"), Some("p2tr"))
            .expect_err("invalid sst token must error");
        match err {
            DiscoveryError::MalformedRecord { reason } => {
                assert!(
                    reason.contains("invalid-token"),
                    "MalformedRecord reason must name the bad token, got: {reason}"
                );
            }
            other => panic!("expected MalformedRecord, got: {other:?}"),
        }
    }

    /// Test 7: invalid pubkey returns DiscoveryError::InvalidPubkey (carried
    /// from the v1.3 test at lines 87-96, adapted for the new signature).
    #[tokio::test]
    async fn discover_coordinator_rejects_invalid_pubkey() {
        let err = discover_coordinator("not-a-valid-pkarr-key", ScriptType::P2wpkh)
            .await
            .unwrap_err();
        assert!(
            matches!(err, DiscoveryError::InvalidPubkey(_)),
            "expected DiscoveryError::InvalidPubkey, got: {err:?}"
        );
        // Display impl must include the bad pubkey for user-actionable
        // diagnostics.
        let display = format!("{err}");
        assert!(
            display.contains("not-a-valid-pkarr-key"),
            "InvalidPubkey error must name the bad pubkey, got: {display}"
        );
    }

    /// Test 8: UnsupportedScriptType Display names pubkey + required + supported set.
    #[test]
    fn unsupported_script_type_error_message_names_pubkey_and_required_and_supported() {
        let err = DiscoveryError::UnsupportedScriptType {
            pubkey: "abcdefghijklmn".to_string(),
            required: ScriptType::P2tr,
            supported: vec![ScriptType::P2wpkh],
        };
        let display = format!("{err}");
        assert!(
            display.contains("abcdefghijklmn"),
            "error must name pubkey, got: {display}"
        );
        assert!(
            display.contains("P2tr"),
            "error must name required script type, got: {display}"
        );
        assert!(
            display.contains("P2wpkh"),
            "error must name supported set, got: {display}"
        );
        // ROADMAP SC#3 wording check — load-bearing literal substring.
        assert!(
            display.contains("does not support"),
            "error must contain ROADMAP SC#3 wording 'does not support', got: {display}"
        );
    }

    // Note: test for "not found in DHT" requires a real DHT query with an unknown key.
    // This is a live network test; run manually with:
    //   cargo test --lib -p client -- discover::tests --nocapture
}
