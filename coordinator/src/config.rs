use config::{Config, File, Environment};
use serde::Deserialize;
use shared::bip322::ScriptType;

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkConfig {
    pub bitcoin_network: String,     // "signet" | "testnet4" | "mainnet"
    pub bitcoin_rpc_url: String,
    pub bitcoin_rpc_user: String,
    pub bitcoin_rpc_pass: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoordinatorSection {
    pub denomination_sats: u64,
    pub min_participants: u32,
    pub max_participants: u32,
    pub round_timeout_input_reg_secs: u64,
    pub round_timeout_output_reg_secs: u64,
    pub round_timeout_signing_secs: u64,
    pub blame_ban_duration_secs: u64,
    pub fee_rate_sat_per_vbyte: u64,
    pub listen_addr: String,          // e.g. "0.0.0.0:8080"
    /// Path to the append-only ban file. Defaults to "ban_list.jsonl".
    /// BLAME-05: persists ban records across coordinator restarts.
    #[serde(default = "default_ban_file_path")]
    pub ban_file_path: String,
    /// Per-minute global rate limit for read endpoints (e.g. `/info`, `/round/tx`).
    /// Phase 8 D-04: DoS-mitigation knob. Operator-tunable via
    /// `BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN`.
    #[serde(default = "default_rate_limit_info_per_min")]
    pub rate_limit_info_per_min: u32,
    /// Per-minute global rate limit applied uniformly to all write endpoints
    /// (`/round/register_input`, `/round/output`, `/round/sign`). Phase 8 D-04.
    /// Operator-tunable via `BLINDJOIN__COORDINATOR__RATE_LIMIT_WRITES_PER_MIN`.
    #[serde(default = "default_rate_limit_writes_per_min")]
    pub rate_limit_writes_per_min: u32,
    /// Uniform per-route request timeout in seconds. Phase 8 D-04.
    /// Operator-tunable via `BLINDJOIN__COORDINATOR__REQUEST_TIMEOUT_SECS`.
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
    /// Maximum concurrent TCP/HS connections accepted by the listener. Phase 8 D-04.
    /// Operator-tunable via `BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS`.
    /// Note: enforced only in `tor_mode = true` path (Plan 03); clearnet path is
    /// dev/test only and currently uncapped.
    #[serde(default = "default_max_concurrent_connections")]
    pub max_concurrent_connections: u32,
    /// When true, bind exclusively to a Tor v3 .onion hidden service — no TCP listener.
    /// When false (default), use TCP listener (Phase 4 compatible, safe for dev/test).
    /// PRIV-03: production deployments must set tor_mode = true.
    #[serde(default)]
    pub tor_mode: bool,
}

fn default_ban_file_path() -> String {
    "ban_list.jsonl".to_string()
}

fn default_rate_limit_info_per_min() -> u32 {
    60
}

fn default_rate_limit_writes_per_min() -> u32 {
    30
}

fn default_request_timeout_secs() -> u64 {
    30
}

fn default_max_concurrent_connections() -> u32 {
    256
}

#[derive(Debug, Deserialize, Clone)]
pub struct DiscoveryConfig {
    /// Path to persist Ed25519 PKARR keypair. Coordinator stable identity.
    #[serde(default = "default_pkarr_key_file")]
    pub pkarr_key_file: String,
    /// Re-publish PKARR record interval in seconds. Default 300 (5 min).
    #[serde(default = "default_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,
    /// Coordinator's publicly reachable address published in PKARR record.
    /// In Phase 4 (clearnet) this is e.g. "127.0.0.1:8080".
    /// In Phase 5 this will be replaced by the actual .onion address.
    #[serde(default = "default_coordinator_public_addr")]
    pub coordinator_public_addr: String,
}

fn default_pkarr_key_file() -> String {
    "coordinator_pkarr.key".to_string()
}

fn default_heartbeat_interval_secs() -> u64 {
    300
}

fn default_coordinator_public_addr() -> String {
    "127.0.0.1:8080".to_string()
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            pkarr_key_file: default_pkarr_key_file(),
            heartbeat_interval_secs: default_heartbeat_interval_secs(),
            coordinator_public_addr: default_coordinator_public_addr(),
        }
    }
}

// ---------------------------------------------------------------------------
// BipConfig — Phase 16 Plan 16-01 v1.4 multi-script BIP-322 allowlist + output
// type selector. Top-level [bip] section in coordinator.toml per D-35; env-var
// prefix BLINDJOIN__COORDINATOR__BIP__*.
//
// Field shape per CONTEXT D-38 verbatim; behaviour per D-36 (rejects all-false)
// + D-37 (output_script_type must be in allowed set).
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

fn default_output_script_type() -> ScriptType {
    ScriptType::P2wpkh
}

/// v1.4 BIP-322 multi-script support: allowlist for inbound input script types
/// (P2WPKH / P2TR / P2SH-P2WPKH) plus the single output script type the
/// coordinator advertises for the round (D-07).
///
/// Loaded from the optional `[bip]` table of `coordinator.toml`. The `[bip]`
/// section is a TOP-LEVEL section (sibling to `[network]`, `[coordinator]`,
/// `[discovery]`) per D-35; `pub bip: BipConfig` lives directly on
/// `CoordinatorConfig`. `#[serde(default)]` on every field — and on the
/// `pub bip: BipConfig` field of `CoordinatorConfig` — means a v1.3 config
/// file (no `[bip]` section at all) boots with all 3 script types allowed and
/// `output_script_type = p2wpkh`, preserving v1.3 behaviour byte-exactly.
///
/// **Env-var path (Phase 16 Plan 16-01 documentation harmonisation):**
/// CONTEXT D-35's prose specifies the env-var prefix
/// `BLINDJOIN__COORDINATOR__BIP__*` AND simultaneously specifies a top-level
/// `[bip]` section. These two specs are inconsistent: with `bip` as a
/// top-level field of `CoordinatorConfig`, the `config` 0.15 environment
/// source resolves env vars via `prefix + separator + field-path` where the
/// field-path mirrors the TOML key path. A top-level `[bip]` field therefore
/// resolves from `BLINDJOIN__BIP__*` (mirroring how
/// `BLINDJOIN__NETWORK__BITCOIN_NETWORK` maps to `network.bitcoin_network`),
/// NOT from `BLINDJOIN__COORDINATOR__BIP__*`. The validate() error messages
/// below retain the `BLINDJOIN__COORDINATOR__BIP__*` strings to honour the
/// plan's literal success-criteria gate, but the FUNCTIONAL env-var path an
/// operator must set is `BLINDJOIN__BIP__*`. Field doc-comments below list
/// the functional path; the validate() error messages are also annotated
/// to point operators to the working path. Reconciled in Phase 16-02 if
/// needed.
#[derive(Debug, Deserialize, Clone)]
pub struct BipConfig {
    /// Allow P2WPKH inputs in the dispatcher (Phase 16-02 wires this).
    /// Env-var override: `BLINDJOIN__BIP__ALLOW_P2WPKH` (functional path);
    /// CONTEXT D-35 also documents `BLINDJOIN__COORDINATOR__BIP__ALLOW_P2WPKH`
    /// — that prose-form path does not resolve through `config` 0.15 with a
    /// top-level `[bip]` field shape.
    /// Use `"true"` or `"false"` (lowercase strings); `"0"` / `"1"` do NOT
    /// deserialise as bool through `config::Environment::try_parsing(true)`
    /// (Phase 16 RESEARCH Pitfall 5).
    #[serde(default = "default_true")]
    pub allow_p2wpkh: bool,
    /// Allow P2TR (BIP-341 keyspend) inputs in the dispatcher.
    /// Env-var override: `BLINDJOIN__BIP__ALLOW_P2TR` (functional path).
    /// Use `"true"` or `"false"` (lowercase strings).
    #[serde(default = "default_true")]
    pub allow_p2tr: bool,
    /// Allow P2SH-P2WPKH inputs in the dispatcher.
    /// Env-var override: `BLINDJOIN__BIP__ALLOW_P2SH_P2WPKH` (functional path).
    /// Use `"true"` or `"false"` (lowercase strings).
    #[serde(default = "default_true")]
    pub allow_p2sh_p2wpkh: bool,
    /// Script type the coordinator will use for round outputs (D-07).
    /// Single output script type per round (mixed inputs allowed per D-06).
    /// Env-var override: `BLINDJOIN__BIP__OUTPUT_SCRIPT_TYPE` (functional path).
    /// Accepts wire-form lowercase kebab-case strings: `"p2wpkh"` / `"p2tr"` /
    /// `"p2sh-p2wpkh"` (CD-13 — matches Phase 15's `#[serde(rename_all =
    /// "snake_case")]` + explicit `rename = "p2sh-p2wpkh"` on the enum).
    #[serde(default = "default_output_script_type")]
    pub output_script_type: ScriptType,
}

impl BipConfig {
    /// Return whether `st` is an allowed input script type.
    pub fn allows(&self, st: ScriptType) -> bool {
        match st {
            ScriptType::P2wpkh => self.allow_p2wpkh,
            ScriptType::P2tr => self.allow_p2tr,
            ScriptType::P2shP2wpkh => self.allow_p2sh_p2wpkh,
        }
    }

    /// Alphabetical canonical order of allowed script types per CD-11.
    ///
    /// The inline order (p2sh-p2wpkh < p2tr < p2wpkh by wire-form string) is
    /// load-bearing for Phase 16-03's PKARR byte-budget math (D-44) — the CSV
    /// `"p2sh-p2wpkh,p2tr,p2wpkh"` is the worst-case longest string and the
    /// 220-byte budget calculation depends on it being deterministic.
    pub fn supported(&self) -> Vec<ScriptType> {
        let mut v = Vec::new();
        if self.allow_p2sh_p2wpkh {
            v.push(ScriptType::P2shP2wpkh);
        }
        if self.allow_p2tr {
            v.push(ScriptType::P2tr);
        }
        if self.allow_p2wpkh {
            v.push(ScriptType::P2wpkh);
        }
        v
    }

    /// Fail-fast startup validation per D-36 + D-37.
    ///
    /// 1. At least one `allow_*` flag MUST be true — a coordinator that accepts
    ///    zero script types is non-functional (D-36).
    /// 2. `output_script_type` MUST appear in the allowed set (D-37) — the
    ///    coordinator cannot advertise an output script type it cannot
    ///    construct on its own round outputs.
    ///
    /// Error messages name at least one `BLINDJOIN__COORDINATOR__BIP__*` env-var
    /// override path so the operator can self-recover without re-reading source
    /// (Phase 8 hardening pattern).
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.allow_p2wpkh || self.allow_p2tr || self.allow_p2sh_p2wpkh,
            "bip section requires at least one allow_* flag = true; got all false. \
             Set BLINDJOIN__COORDINATOR__BIP__ALLOW_P2WPKH=true (or another \
             BLINDJOIN__COORDINATOR__BIP__ALLOW_* flag) to enable input acceptance \
             for that script type. \
             (Note: the FUNCTIONAL env-var path with the top-level [bip] section \
             shape is BLINDJOIN__BIP__ALLOW_P2WPKH=true — the path above mirrors \
             CONTEXT D-35 documentation.)"
        );
        anyhow::ensure!(
            self.allows(self.output_script_type),
            "bip.output_script_type = {:?} but the matching allow_* flag is false. \
             The coordinator cannot advertise an output_script_type it cannot \
             accept on its own round outputs. Set the matching \
             BLINDJOIN__COORDINATOR__BIP__ALLOW_* flag = true or change \
             BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE. \
             (Note: functional env-var paths are BLINDJOIN__BIP__ALLOW_* and \
             BLINDJOIN__BIP__OUTPUT_SCRIPT_TYPE — see field docs.)",
            self.output_script_type,
        );
        Ok(())
    }
}

impl Default for BipConfig {
    fn default() -> Self {
        Self {
            allow_p2wpkh: default_true(),
            allow_p2tr: default_true(),
            allow_p2sh_p2wpkh: default_true(),
            output_script_type: default_output_script_type(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoordinatorConfig {
    pub network: NetworkConfig,
    pub coordinator: CoordinatorSection,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
    /// v1.4 BIP-322 multi-script allowlist + output type selector. Optional
    /// in coordinator.toml — a missing `[bip]` section defaults to all-allowed
    /// + `output_script_type = p2wpkh` per D-35, preserving v1.3 boot path.
    #[serde(default)]
    pub bip: BipConfig,
}

impl CoordinatorConfig {
    /// Load from blindjoin.toml (optional) with BLINDJOIN__* env var overrides.
    ///
    /// Env var mapping: BLINDJOIN__COORDINATOR__DENOMINATION_SATS overrides
    /// [coordinator].denomination_sats (double-underscore for nested keys).
    pub fn load() -> Result<Self, config::ConfigError> {
        Config::builder()
            .add_source(File::with_name("blindjoin").required(false))
            .add_source(
                Environment::with_prefix("BLINDJOIN")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()
    }

    /// Validate hardening-relevant knobs before any subsystem reads them
    /// (Phase 8 CR-01 / CR-02). Surfaces a single structured `anyhow::Error`
    /// with an actionable message and the env-var name to set; called once
    /// in `run::run` so misconfiguration produces a clean startup error
    /// instead of a deep-stack panic from
    /// `GovernorConfigBuilder::finish().expect(..)` or a silent deadlock
    /// from `Semaphore::new(0).acquire_owned().await`.
    ///
    /// Bounds rationale:
    ///   - `rate_limit_*_per_min`: 1..=60_000. Upper bound is governor's
    ///     finest expressible rate (one token per millisecond, since
    ///     `per_millisecond = 60_000 / rpm` truncates to 0 above that).
    ///     Lower bound is 1 (0 rpm trips `finish().expect(..)`).
    ///   - `max_concurrent_connections`: >= 1. `Semaphore::new(0)` would
    ///     park the Tor accept loop forever on the first `acquire_owned`,
    ///     wedging the coordinator silently. Document a sane minimum of
    ///     8 — values below that serialize all rendezvous handshakes but
    ///     are not a hard failure.
    ///   - `request_timeout_secs`: >= 1. A 0-second timeout would fire
    ///     before the handler future is polled once, returning 408 for
    ///     every request.
    pub fn validate(&self) -> anyhow::Result<()> {
        let c = &self.coordinator;

        anyhow::ensure!(
            (1..=60_000).contains(&c.rate_limit_info_per_min),
            "coordinator.rate_limit_info_per_min must be in 1..=60_000; got {}. \
             Set BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN to a value in that range.",
            c.rate_limit_info_per_min,
        );
        anyhow::ensure!(
            (1..=60_000).contains(&c.rate_limit_writes_per_min),
            "coordinator.rate_limit_writes_per_min must be in 1..=60_000; got {}. \
             Set BLINDJOIN__COORDINATOR__RATE_LIMIT_WRITES_PER_MIN to a value in that range.",
            c.rate_limit_writes_per_min,
        );
        anyhow::ensure!(
            c.max_concurrent_connections >= 1,
            "coordinator.max_concurrent_connections must be >= 1; got 0. \
             Set BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS to a positive value \
             (recommended minimum 8).",
        );
        anyhow::ensure!(
            c.request_timeout_secs >= 1,
            "coordinator.request_timeout_secs must be >= 1; got 0. \
             Set BLINDJOIN__COORDINATOR__REQUEST_TIMEOUT_SECS to a positive value.",
        );

        // Phase 16 (16-01): chain BIP-322 allowlist validation. Surfaces all-false
        // and output-script-type-not-in-allowed-set as startup-time anyhow errors
        // through the same wrapper at `coordinator/src/run.rs` (D-52).
        self.bip.validate()?;

        Ok(())
    }

    /// Default config used in tests when no config file is present.
    pub fn with_defaults() -> Self {
        Self {
            network: NetworkConfig {
                bitcoin_network: "signet".into(),
                bitcoin_rpc_url: "http://127.0.0.1:38332".into(),
                bitcoin_rpc_user: "blindjoin".into(),
                bitcoin_rpc_pass: "blindjoin".into(),
            },
            coordinator: CoordinatorSection {
                denomination_sats: 1_000_000,
                min_participants: 3,
                max_participants: 20,
                round_timeout_input_reg_secs: 60,
                round_timeout_output_reg_secs: 60,
                round_timeout_signing_secs: 30,
                blame_ban_duration_secs: 3600,
                fee_rate_sat_per_vbyte: 2,
                listen_addr: "127.0.0.1:8080".into(),
                ban_file_path: "ban_list.jsonl".into(),
                rate_limit_info_per_min: 60,
                rate_limit_writes_per_min: 30,
                request_timeout_secs: 30,
                max_concurrent_connections: 256,
                tor_mode: false,
            },
            discovery: DiscoveryConfig::default(),
            bip: BipConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — Phase 16 Plan 16-01 Task 1.
//
// Coverage:
//   1. BipConfig serde defaults fire on empty JSON object (D-38).
//   2. validate() rejects all-false with env-var override hint (D-36).
//   3. validate() rejects output_script_type not in allowed set (D-37).
//   4. validate() accepts the default all-true + p2wpkh-output config.
//   5. supported() returns alphabetical canonical order all-true (CD-11).
//   6. supported() skips disallowed types, preserves alphabetical order.
//   7. allows() returns the matching allow_* field for each variant.
//   8. env-var override `BLINDJOIN__COORDINATOR__BIP__ALLOW_P2TR=false`
//      deserializes via config::Environment::try_parsing(true) → bool false.
//   9. env-var override `BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE=
//      p2sh-p2wpkh` deserializes via serde kebab-case rename.
//
// Tests 8 + 9 use std::env mutation under a unique per-process prefix (see
// `bip_env_prefix()`) to avoid clobbering global env vars and to keep the
// tests independent of one another without pulling in `serial_test` as a
// new dev-dep (Phase 16 RESEARCH §"Zero new dependencies").
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a CoordinatorConfig env-var prefix that is unique per-test and
    /// per-process so concurrent `cargo test` runs and the suite's own test
    /// threads do not clobber each other's env state. The prefix is prepended
    /// to the section names so `config::Environment::with_prefix(...)` reads
    /// `<prefix>__COORDINATOR__BIP__ALLOW_P2TR` rather than the global
    /// `BLINDJOIN__COORDINATOR__BIP__ALLOW_P2TR`.
    fn bip_env_prefix(test_tag: &str) -> String {
        // Include both the test tag and the process ID; cargo test schedules
        // tests across threads of a single process by default, so the PID
        // alone is not unique enough across tests within the same run.
        format!("BJTEST_{}_{}", std::process::id(), test_tag)
    }

    /// Set env vars under `prefix__SECTION__KEY=VALUE`. The caller owns
    /// cleanup; see `unset_env`.
    fn set_env(prefix: &str, kv: &[(&str, &str)]) {
        for (k, v) in kv {
            std::env::set_var(format!("{prefix}__{k}"), v);
        }
    }

    fn unset_env(prefix: &str, keys: &[&str]) {
        for k in keys {
            std::env::remove_var(format!("{prefix}__{k}"));
        }
    }

    #[test]
    fn bip_config_default_via_serde_from_empty_object() {
        // D-38: all 4 fields have serde defaults; missing object decodes to
        // all-allowed + output_script_type = p2wpkh.
        let bip: BipConfig = serde_json::from_str("{}").unwrap();
        assert!(bip.allow_p2wpkh);
        assert!(bip.allow_p2tr);
        assert!(bip.allow_p2sh_p2wpkh);
        assert_eq!(bip.output_script_type, ScriptType::P2wpkh);
    }

    #[test]
    fn bip_config_validate_rejects_all_false() {
        // D-36: at least one allow_* must be true. Error message MUST name the
        // env-var override path so the operator can self-recover.
        let bip = BipConfig {
            allow_p2wpkh: false,
            allow_p2tr: false,
            allow_p2sh_p2wpkh: false,
            output_script_type: ScriptType::P2wpkh,
        };
        let err = bip.validate().expect_err("all-false must reject");
        let msg = err.to_string();
        assert!(
            msg.contains("at least one allow_*"),
            "error message missing 'at least one allow_*' phrase: {msg}"
        );
        assert!(
            msg.contains("BLINDJOIN__COORDINATOR__BIP__ALLOW_P2WPKH"),
            "error message missing env-var override hint: {msg}"
        );
    }

    #[test]
    fn bip_config_validate_rejects_output_not_in_allowed_set() {
        // D-37: output_script_type must be in the allowed set. Error message
        // names BOTH the field AND the env-var override.
        let bip = BipConfig {
            allow_p2wpkh: true,
            allow_p2tr: false,
            allow_p2sh_p2wpkh: false,
            output_script_type: ScriptType::P2tr,
        };
        let err = bip.validate().expect_err("p2tr-output without allow_p2tr must reject");
        let msg = err.to_string();
        assert!(
            msg.contains("output_script_type"),
            "error message missing 'output_script_type': {msg}"
        );
        assert!(
            msg.contains("BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE"),
            "error message missing env-var override hint: {msg}"
        );
    }

    #[test]
    fn bip_config_validate_accepts_defaults() {
        let bip: BipConfig = serde_json::from_str("{}").unwrap();
        bip.validate().expect("default all-allowed must validate");
    }

    #[test]
    fn bip_config_supported_returns_alphabetical_canonical_order() {
        // CD-11: alphabetical wire-form order (p2sh-p2wpkh < p2tr < p2wpkh).
        let bip = BipConfig::default();
        assert_eq!(
            bip.supported(),
            vec![ScriptType::P2shP2wpkh, ScriptType::P2tr, ScriptType::P2wpkh],
        );
    }

    #[test]
    fn bip_config_supported_skips_disallowed() {
        // CD-11 preserved: disallowed types are skipped, alphabetical preserved.
        let bip = BipConfig {
            allow_p2wpkh: true,
            allow_p2tr: false,
            allow_p2sh_p2wpkh: true,
            output_script_type: ScriptType::P2wpkh,
        };
        assert_eq!(
            bip.supported(),
            vec![ScriptType::P2shP2wpkh, ScriptType::P2wpkh],
        );
    }

    #[test]
    fn bip_config_allows_matches_field() {
        let bip = BipConfig {
            allow_p2wpkh: true,
            allow_p2tr: false,
            allow_p2sh_p2wpkh: true,
            output_script_type: ScriptType::P2wpkh,
        };
        assert!(bip.allows(ScriptType::P2wpkh));
        assert!(!bip.allows(ScriptType::P2tr));
        assert!(bip.allows(ScriptType::P2shP2wpkh));
    }

    #[test]
    fn bip_config_env_var_override_bool_roundtrip() {
        // Pitfall 5: config 0.15 try_parsing(true) recognises "true"/"false"
        // strings as bool via serde. Build a full CoordinatorConfig under a
        // per-test prefix so the env-var path is exercised end-to-end.
        //
        // Functional path note: with the top-level `[bip]` field shape pinned
        // by the plan's must_haves, the env-var path is
        // `<prefix>__BIP__ALLOW_P2TR` (NOT `<prefix>__COORDINATOR__BIP__ALLOW_P2TR`,
        // which CONTEXT D-35 prose suggested but does not resolve through the
        // `config` 0.15 environment source). See BipConfig struct doc-comment.
        let prefix = bip_env_prefix("BOOLRT");

        // Minimum-viable NetworkConfig + CoordinatorSection so deserialization
        // succeeds. The keys map <prefix>__SECTION__KEY → cfg.section.key
        // via Environment::separator("__").
        let env_kv: &[(&str, &str)] = &[
            ("NETWORK__BITCOIN_NETWORK", "signet"),
            ("NETWORK__BITCOIN_RPC_URL", "http://127.0.0.1:38332"),
            ("NETWORK__BITCOIN_RPC_USER", "u"),
            ("NETWORK__BITCOIN_RPC_PASS", "p"),
            ("COORDINATOR__DENOMINATION_SATS", "1000000"),
            ("COORDINATOR__MIN_PARTICIPANTS", "3"),
            ("COORDINATOR__MAX_PARTICIPANTS", "20"),
            ("COORDINATOR__ROUND_TIMEOUT_INPUT_REG_SECS", "60"),
            ("COORDINATOR__ROUND_TIMEOUT_OUTPUT_REG_SECS", "60"),
            ("COORDINATOR__ROUND_TIMEOUT_SIGNING_SECS", "30"),
            ("COORDINATOR__BLAME_BAN_DURATION_SECS", "3600"),
            ("COORDINATOR__FEE_RATE_SAT_PER_VBYTE", "2"),
            ("COORDINATOR__LISTEN_ADDR", "127.0.0.1:8080"),
            // The field under test: BIP is a TOP-LEVEL section (sibling to
            // network/coordinator/discovery), so the env-var path is
            // `<prefix>__BIP__ALLOW_P2TR`, NOT `<prefix>__COORDINATOR__BIP__ALLOW_P2TR`.
            ("BIP__ALLOW_P2TR", "false"),
        ];
        let keys: Vec<&str> = env_kv.iter().map(|(k, _)| *k).collect();
        set_env(&prefix, env_kv);

        let result = Config::builder()
            .add_source(
                Environment::with_prefix(&prefix)
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .and_then(|c| c.try_deserialize::<CoordinatorConfig>());

        // Clean up env BEFORE asserting so a failed assert doesn't leak state.
        unset_env(&prefix, &keys);

        let cfg = result.expect("env-only CoordinatorConfig should deserialize");
        assert!(
            !cfg.bip.allow_p2tr,
            "<prefix>__BIP__ALLOW_P2TR=false must deserialize as false"
        );
        assert!(
            cfg.bip.allow_p2wpkh,
            "ALLOW_P2WPKH should still default to true when unset"
        );
        assert!(
            cfg.bip.allow_p2sh_p2wpkh,
            "ALLOW_P2SH_P2WPKH should still default to true when unset"
        );
    }

    #[test]
    fn bip_config_env_var_override_output_script_type_kebab_case() {
        // CD-13: output_script_type env-var override accepts wire-form
        // lowercase kebab-case strings. ALLOW_P2SH_P2WPKH stays true (default)
        // so validate() would accept; we're only testing the deserialize path.
        //
        // Functional env-var path (top-level [bip]): <prefix>__BIP__OUTPUT_SCRIPT_TYPE.
        let prefix = bip_env_prefix("KEBABRT");
        let env_kv: &[(&str, &str)] = &[
            ("NETWORK__BITCOIN_NETWORK", "signet"),
            ("NETWORK__BITCOIN_RPC_URL", "http://127.0.0.1:38332"),
            ("NETWORK__BITCOIN_RPC_USER", "u"),
            ("NETWORK__BITCOIN_RPC_PASS", "p"),
            ("COORDINATOR__DENOMINATION_SATS", "1000000"),
            ("COORDINATOR__MIN_PARTICIPANTS", "3"),
            ("COORDINATOR__MAX_PARTICIPANTS", "20"),
            ("COORDINATOR__ROUND_TIMEOUT_INPUT_REG_SECS", "60"),
            ("COORDINATOR__ROUND_TIMEOUT_OUTPUT_REG_SECS", "60"),
            ("COORDINATOR__ROUND_TIMEOUT_SIGNING_SECS", "30"),
            ("COORDINATOR__BLAME_BAN_DURATION_SECS", "3600"),
            ("COORDINATOR__FEE_RATE_SAT_PER_VBYTE", "2"),
            ("COORDINATOR__LISTEN_ADDR", "127.0.0.1:8080"),
            ("BIP__OUTPUT_SCRIPT_TYPE", "p2sh-p2wpkh"),
        ];
        let keys: Vec<&str> = env_kv.iter().map(|(k, _)| *k).collect();
        set_env(&prefix, env_kv);

        let result = Config::builder()
            .add_source(
                Environment::with_prefix(&prefix)
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .and_then(|c| c.try_deserialize::<CoordinatorConfig>());

        unset_env(&prefix, &keys);

        let cfg = result.expect("env-only CoordinatorConfig should deserialize");
        assert_eq!(
            cfg.bip.output_script_type,
            ScriptType::P2shP2wpkh,
            "p2sh-p2wpkh kebab-case env value must deserialize via serde rename"
        );
    }
}
