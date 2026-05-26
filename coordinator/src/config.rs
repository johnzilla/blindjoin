use config::{Config, File, Environment};
use serde::Deserialize;

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

#[derive(Debug, Deserialize, Clone)]
pub struct CoordinatorConfig {
    pub network: NetworkConfig,
    pub coordinator: CoordinatorSection,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
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
        }
    }
}
