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
}

fn default_ban_file_path() -> String {
    "ban_list.jsonl".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoordinatorConfig {
    pub network: NetworkConfig,
    pub coordinator: CoordinatorSection,
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
            },
        }
    }
}
