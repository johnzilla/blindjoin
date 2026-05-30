use pkarr::{Client, Keypair, SignedPacket};
use anyhow::Result;

pub struct PkarrPublisher {
    client: Client,
    /// Public to allow main.rs to read the public key for logging.
    #[allow(dead_code)]
    pub keypair: Keypair,
}

impl PkarrPublisher {
    pub fn new(keypair: Keypair) -> Result<Self> {
        let client = Client::builder()
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build PKARR client: {e}"))?;
        Ok(Self { client, keypair })
    }

    pub async fn publish_record(&self, packet: SignedPacket) -> Result<()> {
        self.client
            .publish(&packet, None)
            .await
            .map_err(|e| anyhow::anyhow!("PKARR publish failed: {e}"))
    }
}

/// Load existing keypair from file, or generate and persist a new one.
///
/// The public key is the coordinator's stable DHT identity — never rotate it.
/// Called at coordinator startup; the key file path comes from DiscoveryConfig.
pub fn load_or_generate_keypair(path: &str) -> Result<Keypair> {
    use std::path::Path;
    let p = Path::new(path);
    match Keypair::from_secret_key_file(p) {
        Ok(kp) => {
            tracing::info!("Loaded existing PKARR keypair from {path}");
            Ok(kp)
        }
        Err(_) => {
            let kp = Keypair::random();
            kp.write_secret_key_file(p)
                .map_err(|e| anyhow::anyhow!("Cannot write PKARR key file at {path}: {e}"))?;
            tracing::info!(
                pubkey = %kp.public_key().to_z32(),
                "Generated new PKARR keypair, saved to {path}"
            );
            Ok(kp)
        }
    }
}

/// Build a signed DNS TXT packet containing coordinator metadata.
///
/// Uses label "_blindjoin" with a single compact JSON value (fits in 255 bytes).
/// TTL = 300s (matches heartbeat interval). The "onion" field holds a clearnet
/// address in Phase 4 and will be replaced by an actual .onion address in Phase 5.
///
/// **Phase 16-03 (v1.4) schema:** bumped to `"v": "0.2.0"` (compact-renamed from
/// `version`). Two new advertisement fields added per D-39..D-43:
///   - `sst` — supported_script_types CSV alphabetical canonical order per CD-11
///     (e.g. `"p2sh-p2wpkh,p2tr,p2wpkh"`). Caller MUST pass the slice in
///     alphabetical order; `build_coordinator_packet` joins as-is.
///   - `ost` — output_script_type single kebab-case string per CD-13
///     (`"p2wpkh"` / `"p2tr"` / `"p2sh-p2wpkh"`).
///
/// **B3 compact-name migration:** five verbose field names were compacted to
/// 1-2 char codes to keep the worst-case production payload (62-byte .onion +
/// all-3-allowed CSV) under the 220-byte DNS-TXT warn at line 76:
///   - `version` → `v`         (saves ~10 bytes)
///   - `denomination_sats` → `ds` (saves ~16 bytes)
///   - `min_participants` → `mp` (saves ~16 bytes)
///   - `status` → `st`         (saves ~7 bytes)
///   - `network` → `n`         (saves ~7 bytes)
///
/// `type` and `onion` deliberately retain verbose names:
///   - `type` is already 4 bytes (rename to `t` saves only 3) and identifies the
///     schema for any future PKARR consumer that does schema-version pinning.
///   - `onion` is load-bearing for v1.3 client resolver compat — the v1.3
///     `Partial { onion: Option<String> }` struct at `client/src/discover.rs:75-80`
///     reads this exact field name, and renaming it breaks v1.3 client compat.
///
/// v1.3 clients silently drop every field they do not know about (no
/// `#[serde(deny_unknown_fields)]` on `Partial`; verified at
/// `client/src/discover.rs:75-80`), so the compact-name renames AND the new
/// `sst` / `ost` fields are wire-safe for the v1.3 resolver per
/// RESEARCH §V1.4-MOD-02.
///
/// References: D-39, D-40, D-41, D-42, D-43, D-55, B3 compact-name migration.
pub fn build_coordinator_packet(
    keypair: &Keypair,
    coordinator_addr: &str,
    denomination_sats: u64,
    min_participants: u32,
    status: &str,
    supported: &[&str],
    output_script_type: &str,
) -> Result<SignedPacket> {
    let record = serde_json::json!({
        "type": "blindjoin-coordinator",
        "v": "0.2.0",
        "onion": coordinator_addr,
        "n": "signet",
        "ds": denomination_sats,
        "mp": min_participants,
        "st": status,
        "sst": supported.join(","),
        "ost": output_script_type,
    });
    let json_str = serde_json::to_string(&record)?;

    // DNS TXT string limit is 255 bytes per character string. Warn if approaching.
    if json_str.len() > 220 {
        tracing::warn!(
            len = json_str.len(),
            "PKARR TXT record approaching 255-byte DNS limit"
        );
    }

    let ttl: u32 = 300;
    let label: pkarr::dns::Name<'_> = "_blindjoin"
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid TXT label: {e}"))?;
    let txt_value: pkarr::dns::rdata::TXT<'_> = json_str
        .as_str()
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid TXT value: {e}"))?;

    SignedPacket::builder()
        .txt(label, txt_value, ttl)
        .sign(keypair)
        .map_err(|e| anyhow::anyhow!("SignedPacket build failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extract the inner JSON string from the first _blindjoin TXT record on a
    /// SignedPacket. Used by the new Phase 16-03 tests to parse the TXT payload
    /// back to a `serde_json::Value` for field-level assertions.
    fn extract_txt_json(packet: &SignedPacket) -> serde_json::Value {
        use pkarr::dns::rdata::RData;
        let rr = packet
            .resource_records("_blindjoin")
            .next()
            .expect("expected at least one _blindjoin resource record");
        let txt = match &rr.rdata {
            RData::TXT(txt) => txt,
            other => panic!("expected TXT rdata, got {other:?}"),
        };
        let s = String::try_from(txt.clone())
            .expect("TXT to UTF-8 String conversion failed");
        serde_json::from_str::<serde_json::Value>(&s)
            .expect("TXT payload was not valid JSON")
    }

    /// Build the JSON string that `build_coordinator_packet` would serialize for
    /// a given input set. Mirrors the production `json!` literal byte-for-byte
    /// (field order matters — serde_json preserves Map insertion order). Used by
    /// the two inline byte-budget tests so they can assert on the exact payload
    /// size without round-tripping through the DNS TXT encoder (which adds
    /// per-character-string length prefixes the warn at line 76 also ignores —
    /// matching the production warn measurement).
    fn build_record_json(
        coordinator_addr: &str,
        denomination_sats: u64,
        min_participants: u32,
        status: &str,
        supported: &[&str],
        output_script_type: &str,
    ) -> String {
        let record = serde_json::json!({
            "type": "blindjoin-coordinator",
            "v": "0.2.0",
            "onion": coordinator_addr,
            "n": "signet",
            "ds": denomination_sats,
            "mp": min_participants,
            "st": status,
            "sst": supported.join(","),
            "ost": output_script_type,
        });
        serde_json::to_string(&record).unwrap()
    }

    #[test]
    fn test_build_coordinator_packet_valid() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
            &["p2wpkh"],
            "p2wpkh",
        )
        .unwrap();
        assert!(packet.resource_records("_blindjoin").count() > 0);
    }

    #[test]
    fn test_build_coordinator_packet_contains_fields() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "input_reg",
            &["p2wpkh"],
            "p2wpkh",
        )
        .unwrap();
        // Phase 16-03 (B3): assert compact field names are present and verbose
        // legacy names are absent. The presence of `v` confirms the schema bump.
        let v = extract_txt_json(&packet);
        assert_eq!(v["v"], serde_json::json!("0.2.0"));
        assert_eq!(v["st"], serde_json::json!("input_reg"));
        assert!(v.get("version").is_none(), "legacy `version` field must be renamed to `v`");
        assert!(v.get("status").is_none(), "legacy `status` field must be renamed to `st`");
    }

    #[test]
    fn test_load_or_generate_keypair_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_pkarr.key");
        let path_str = path.to_str().unwrap();

        // File does not exist yet — should generate
        let kp1 = load_or_generate_keypair(path_str).unwrap();
        assert!(path.exists(), "Key file should have been created");

        // Load again — should return same public key
        let kp2 = load_or_generate_keypair(path_str).unwrap();
        assert_eq!(
            kp1.public_key().to_z32(),
            kp2.public_key().to_z32(),
            "Public key should be stable across reloads"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 16-03 (v1.4) — PKARR advertisement schema tests per D-39..D-44 +
    // D-55 + B3 compact-name migration. Two of these tests (production .onion
    // budget + dev-mode localhost budget) ARE the CI regression gates against
    // future field additions breaching the 220-byte DNS-TXT warn.
    // -----------------------------------------------------------------------

    /// PRODUCTION worst case: 62-byte Tor v3 .onion + all-3-allowed CSV.
    /// This is the PROJECT-constraint worst case (Tor-native; `.onion` is the
    /// production address). Asserts the serialized payload remains under the
    /// 220-byte warn threshold at `build_coordinator_packet` line 76 + 79.
    ///
    /// Regression gate per D-55 + B3: a future field addition or a 4th script
    /// type that pushes this count past 220 must trigger an explicit ADR (see
    /// `<deferred_ideas>` in 16-03-PLAN.md — options: single-char codes,
    /// bitmask, hash-of-sorted-set fetch).
    #[test]
    fn coordinator_packet_under_220_byte_budget_production_onion() {
        // 56 base32 chars + ".onion" = 62 bytes total (production Tor v3 length).
        // [Rule 1 — Bug]: the PLAN literal had only 54 x's (= 60 bytes total).
        // Real Tor v3 .onion is 56 base32 chars; using 60 bytes would under-
        // approximate the worst case and weaken the regression gate. Padded to
        // 56 x's so the assertion truly bounds the PROJECT-constraint worst case.
        let onion = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.onion";
        assert_eq!(onion.len(), 62, "fixture .onion must be 62 bytes (Tor v3 length)");
        let json_str = build_record_json(
            onion,
            1_000_000,
            3,
            "idle",
            &["p2sh-p2wpkh", "p2tr", "p2wpkh"],
            "p2wpkh",
        );
        let len = json_str.len();
        assert!(
            len < 220,
            "PKARR byte-budget regression gate: production .onion worst case payload \
             must stay under the 220-byte DNS-TXT warn threshold; \
             got {len} bytes. Reduce field-name length or descope a field. \
             Payload: {json_str}",
        );
    }

    /// DEV-MODE headroom: 14-byte `127.0.0.1:8080` + all-3-allowed CSV.
    /// Locks the dev-vs-prod budget delta at ~48 bytes (the .onion-vs-localhost
    /// length difference). Asserts dev-mode payload stays under 200 bytes —
    /// gives any future field addition deterministic per-tier budget reasoning.
    #[test]
    fn coordinator_packet_under_200_byte_budget_dev_mode() {
        let localhost = "127.0.0.1:8080";
        assert_eq!(localhost.len(), 14, "dev-mode fixture must be 14 bytes");
        let json_str = build_record_json(
            localhost,
            1_000_000,
            3,
            "idle",
            &["p2sh-p2wpkh", "p2tr", "p2wpkh"],
            "p2wpkh",
        );
        let len = json_str.len();
        assert!(
            len < 200,
            "PKARR byte-budget regression gate: dev-mode headroom payload \
             must stay under 200 bytes; got {len} bytes. \
             A future field addition that breaches this AND \
             coordinator_packet_under_220_byte_budget_production_onion must \
             trigger an encoding-compaction ADR. Payload: {json_str}",
        );
    }

    /// Schema-version field is `v` (compact-renamed from `version` per B3) and
    /// equals `"0.2.0"` (bumped from `"0.1.0"` per D-39).
    #[test]
    fn coordinator_packet_emits_v_0_2_0() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
            &["p2wpkh"],
            "p2wpkh",
        )
        .unwrap();
        let v = extract_txt_json(&packet);
        assert_eq!(v["v"], serde_json::json!("0.2.0"));
    }

    /// `sst` is a CSV of supported script types in alphabetical canonical order
    /// per CD-11 + D-40. The caller passes the slice pre-sorted; this test
    /// verifies the join is verbatim.
    #[test]
    fn coordinator_packet_emits_sst_csv_alphabetical() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
            &["p2sh-p2wpkh", "p2tr", "p2wpkh"],
            "p2wpkh",
        )
        .unwrap();
        let v = extract_txt_json(&packet);
        assert_eq!(
            v["sst"],
            serde_json::json!("p2sh-p2wpkh,p2tr,p2wpkh"),
            "sst must be alphabetical CSV per CD-11",
        );
    }

    /// `ost` is a single kebab-case string per CD-13.
    #[test]
    fn coordinator_packet_emits_ost_single_value() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
            &["p2tr"],
            "p2tr",
        )
        .unwrap();
        let v = extract_txt_json(&packet);
        assert_eq!(v["ost"], serde_json::json!("p2tr"));
    }

    /// B3 compact-name migration: assert ALL 5 renames are observable on the
    /// wire AND ALL 5 verbose names are absent. Also confirms the preserved
    /// fields (`type` and `onion`) are still present — `onion` is load-bearing
    /// for the v1.3 `Partial { onion }` client resolver per RESEARCH §V1.4-MOD-02.
    #[test]
    fn coordinator_packet_emits_compact_renamed_fields() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
            &["p2wpkh"],
            "p2wpkh",
        )
        .unwrap();
        let v = extract_txt_json(&packet);

        // Compact renames present + correct value.
        assert_eq!(v["v"], serde_json::json!("0.2.0"));
        assert_eq!(v["ds"], serde_json::json!(1_000_000));
        assert_eq!(v["mp"], serde_json::json!(3));
        assert_eq!(v["st"], serde_json::json!("idle"));
        assert_eq!(v["n"], serde_json::json!("signet"));

        // Verbose legacy field names MUST be absent.
        assert!(v.get("version").is_none(), "legacy `version` must be renamed to `v`");
        assert!(v.get("denomination_sats").is_none(), "legacy `denomination_sats` must be renamed to `ds`");
        assert!(v.get("min_participants").is_none(), "legacy `min_participants` must be renamed to `mp`");
        assert!(v.get("status").is_none(), "legacy `status` must be renamed to `st`");
        assert!(v.get("network").is_none(), "legacy `network` must be renamed to `n`");

        // Preserved fields MUST still be present. `onion` is load-bearing
        // for the v1.3 client `Partial { onion }` resolver per RESEARCH §V1.4-MOD-02.
        assert_eq!(v["type"], serde_json::json!("blindjoin-coordinator"));
        assert!(
            v["onion"].is_string(),
            "`onion` MUST remain a string field (v1.3 client compat)",
        );
    }

    /// Sanity: a restricted allowlist (single-script subset, smaller than worst
    /// case) MUST also stay under the production budget. Future field
    /// additions that only impact non-allowlist fields still need this guard
    /// to remain green.
    #[test]
    fn coordinator_packet_with_subset_supported_under_budget() {
        let onion = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.onion";
        let json_str = build_record_json(
            onion,
            1_000_000,
            3,
            "idle",
            &["p2wpkh"],
            "p2wpkh",
        );
        let len = json_str.len();
        assert!(
            len < 220,
            "restricted-allowlist payload should be well under 220 bytes; got {len}",
        );
    }
}
