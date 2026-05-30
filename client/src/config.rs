use clap::Parser;

/// Parse a `--type` CLI value into a [`shared::bip322::ScriptType`].
///
/// Routes the input string through the same serde wire form used by Phase 15
/// (`#[serde(rename_all = "snake_case")]` + `#[serde(rename = "p2sh-p2wpkh")]`
/// on the `P2shP2wpkh` variant) so the accepted tokens are the SINGLE source of
/// truth — no manual string match in the client crate. Per CD-17 only lowercase
/// kebab-case forms are accepted (`p2wpkh`, `p2tr`, `p2sh-p2wpkh`).
fn parse_script_type(s: &str) -> Result<shared::bip322::ScriptType, String> {
    // Wrap string in JSON quotes so serde_json::from_str fires the enum's serde impl.
    let quoted = format!("\"{}\"", s);
    serde_json::from_str::<shared::bip322::ScriptType>(&quoted)
        .map_err(|e| format!("invalid --type value '{s}': expected p2wpkh, p2tr, or p2sh-p2wpkh ({e})"))
}

#[derive(Parser, Debug, Clone)]
#[command(name = "blindjoin-client", about = "CoinJoin client for blindjoin coordinator")]
pub struct ClientConfig {
    /// Coordinator URL (clearnet for Phase 1)
    #[arg(long, env = "BLINDJOIN_COORDINATOR_URL", default_value = "http://127.0.0.1:8080")]
    pub coordinator_url: String,

    /// UTXO to register (format: txid:vout). Required for round participation.
    #[arg(long, env = "BLINDJOIN_UTXO")]
    pub utxo: Option<String>,

    /// UTXO value in satoshis. **Deprecated:** the coordinator now queries Bitcoin
    /// Core's gettxout at input registration and supplies the real value via the
    /// PSBT's witness_utxo. The CLI flag is accepted for backward compat but is
    /// ignored; safe to remove from scripts.
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

    /// Script type for wallet descriptor generation. Selects BIP-84 (p2wpkh),
    /// BIP-86 (p2tr), or BIP-49 (p2sh-p2wpkh). Default p2wpkh for v1.3 backwards
    /// compatibility — existing wallets continue working unchanged.
    #[arg(long = "type", env = "BLINDJOIN_SCRIPT_TYPE", default_value = "p2wpkh", value_parser = parse_script_type)]
    pub script_type: shared::bip322::ScriptType,

    /// Generate a new BIP-84 wallet, print descriptors to stdout, write descriptors.txt, and exit.
    /// When this flag is set, --utxo, --utxo-value-sats, and --utxo-wif are not required.
    ///
    /// Important: if you later use this wallet to participate in a round (via --descriptor),
    /// the UTXO you register MUST be at the wallet's first external address
    /// (derivation path m/84'/0'/0'/0/0). Funds sent to a different derivation
    /// will not produce valid signatures. The generate command prints the
    /// exact address to fund.
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

    /// Use Tor hidden service routing with isolated circuits per phase (CLI-05).
    /// When set, input registration flows through one Tor circuit (alice) and
    /// output registration flows through a separate, unlinkable circuit (bob).
    /// Requires the coordinator to be reachable as a Tor hidden service (.onion URL).
    #[arg(long, env = "BLINDJOIN_USE_TOR", default_value_t = false)]
    pub use_tor: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::bip322::ScriptType;

    // Phase 17 17-01 Task 1 — value_parser exhaustively covers the 3 LOCKED
    // ScriptType wire forms (D-57) AND rejects out-of-band variants per CD-17
    // (lowercase kebab-case only).

    #[test]
    fn parse_script_type_accepts_p2wpkh() {
        let result = parse_script_type("p2wpkh");
        assert_eq!(result, Ok(ScriptType::P2wpkh));
    }

    #[test]
    fn parse_script_type_accepts_p2tr() {
        let result = parse_script_type("p2tr");
        assert_eq!(result, Ok(ScriptType::P2tr));
    }

    #[test]
    fn parse_script_type_accepts_p2sh_p2wpkh() {
        let result = parse_script_type("p2sh-p2wpkh");
        assert_eq!(result, Ok(ScriptType::P2shP2wpkh));
    }

    #[test]
    fn parse_script_type_rejects_uppercase() {
        let result = parse_script_type("P2TR");
        assert!(
            result.is_err(),
            "expected uppercase 'P2TR' to be rejected per CD-17 lowercase-only, got: {result:?}"
        );
    }

    #[test]
    fn parse_script_type_rejects_unknown() {
        let result = parse_script_type("p2pkh");
        let err = result.expect_err("expected unknown token 'p2pkh' to be rejected");
        assert!(
            err.contains("p2pkh"),
            "error message should name the rejected token, got: {err}"
        );
    }

    #[test]
    fn client_config_defaults_to_p2wpkh() {
        // Default-value test: no --type → ScriptType::P2wpkh per D-57 backwards-compat.
        let cfg = ClientConfig::try_parse_from(["client", "--utxo-wif", "DUMMY_WIF_VALUE"])
            .expect("clap parse with no --type flag should succeed");
        assert_eq!(cfg.script_type, ScriptType::P2wpkh);
    }
}
