use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "blindjoin-client", about = "CoinJoin client for blindjoin coordinator")]
pub struct ClientConfig {
    /// Coordinator URL (clearnet for Phase 1)
    #[arg(long, env = "BLINDJOIN_COORDINATOR_URL", default_value = "http://127.0.0.1:8080")]
    pub coordinator_url: String,

    /// UTXO to register (format: txid:vout)
    #[arg(long, env = "BLINDJOIN_UTXO")]
    pub utxo: String,

    /// UTXO value in satoshis
    #[arg(long, env = "BLINDJOIN_UTXO_VALUE_SATS")]
    pub utxo_value_sats: u64,

    /// WIF private key for the UTXO (insecure — for testing only)
    #[arg(long, env = "BLINDJOIN_UTXO_WIF")]
    pub utxo_wif: String,

    /// Bitcoin network: signet | testnet4 | mainnet
    #[arg(long, default_value = "signet")]
    pub network: String,

    /// Polling interval in milliseconds for GET /info
    #[arg(long, default_value_t = 1000)]
    pub poll_interval_ms: u64,
}
