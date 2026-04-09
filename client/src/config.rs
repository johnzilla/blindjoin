use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "blindjoin-client", about = "CoinJoin client for blindjoin coordinator")]
pub struct ClientConfig {
    /// Coordinator URL (clearnet for Phase 1)
    #[arg(long, env = "BLINDJOIN_COORDINATOR_URL", default_value = "http://127.0.0.1:8080")]
    pub coordinator_url: String,

    /// UTXO to register (format: txid:vout). Required for round participation.
    #[arg(long, env = "BLINDJOIN_UTXO")]
    pub utxo: Option<String>,

    /// UTXO value in satoshis. Required for round participation.
    #[arg(long, env = "BLINDJOIN_UTXO_VALUE_SATS")]
    pub utxo_value_sats: Option<u64>,

    /// WIF private key for the UTXO (insecure — for testing only).
    /// Mutually exclusive with --descriptor.
    #[arg(long, env = "BLINDJOIN_UTXO_WIF")]
    pub utxo_wif: Option<String>,

    /// Descriptor wallet (BIP-84 xprv). Mutually exclusive with --utxo-wif.
    /// Example: "wpkh(xprv.../84'/0'/0'/0/*)"
    #[arg(long, env = "BLINDJOIN_DESCRIPTOR")]
    pub descriptor: Option<String>,

    /// bech32 address of the UTXO to register (required when using --descriptor).
    /// Used to derive the script_pubkey for BIP-322 ownership proof and PSBT signing.
    #[arg(long, env = "BLINDJOIN_UTXO_ADDRESS")]
    pub utxo_address: Option<String>,

    /// Generate a new BIP-84 wallet, print descriptors to stdout, write descriptors.txt, and exit.
    /// When this flag is set, --utxo, --utxo-value-sats, and --utxo-wif are not required.
    #[arg(long)]
    pub generate_wallet: bool,

    /// Bitcoin network: signet | testnet4 | mainnet
    #[arg(long, default_value = "signet")]
    pub network: String,

    /// Polling interval in milliseconds for GET /info
    #[arg(long, default_value_t = 1000)]
    pub poll_interval_ms: u64,

    /// Discover coordinator via PKARR DHT using a public key (z32 format, starts with "pk:").
    /// If set, overrides --coordinator-url with the resolved endpoint.
    #[arg(long, env = "BLINDJOIN_PKARR_PUBKEY")]
    pub pkarr_pubkey: Option<String>,
}
