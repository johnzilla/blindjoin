use clap::Parser;
use tracing::info;

mod config;
mod discover;
mod http;
mod round;
mod tor;
mod wallet;

use config::ClientConfig;
use http::CoordinatorClient;
use wallet::ClientWallet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("client=info".parse().unwrap()))
        .init();

    let cfg = ClientConfig::parse();
    let network = match cfg.network.as_str() {
        "signet" => bitcoin::Network::Signet,
        "testnet4" => bitcoin::Network::Testnet,
        "mainnet" => bitcoin::Network::Bitcoin,
        other => anyhow::bail!("Unknown network: {other}"),
    };

    // --generate-wallet: print descriptors and exit (no round participation needed)
    if cfg.generate_wallet {
        let utxo = cfg.utxo.as_deref().unwrap_or("0000000000000000000000000000000000000000000000000000000000000000:0");
        ClientWallet::generate(utxo, network)?;
        std::process::exit(0);
    }

    // Require --utxo for round participation. --utxo-value-sats is no longer
    // required: the coordinator queries Bitcoin Core's gettxout at registration
    // and supplies the real value via the PSBT's witness_utxo. The CLI flag is
    // accepted for backward compat but ignored.
    let utxo = cfg.utxo.as_deref()
        .ok_or_else(|| anyhow::anyhow!("--utxo is required for round participation"))?;

    let wallet = if let Some(descriptor) = cfg.descriptor.as_deref() {
        // --descriptor path: requires --utxo-address
        let utxo_address = cfg.utxo_address.as_deref()
            .ok_or_else(|| anyhow::anyhow!("--utxo-address is required when using --descriptor"))?;
        ClientWallet::from_descriptor(descriptor, utxo, utxo_address, network)?
    } else {
        // WIF path (backward compat, default for testing)
        let wif = cfg.utxo_wif.as_deref()
            .ok_or_else(|| anyhow::anyhow!("--utxo-wif is required when not using --descriptor or --generate-wallet"))?;
        ClientWallet::from_wif(wif, utxo, network)?
    };

    // CLI-01: If --pkarr-pubkey is provided, resolve coordinator URL from DHT.
    let coordinator_url = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
        let info = discover::discover_coordinator(pkarr_key)
            .await
            .map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))?;
        info.coordinator_url
    } else {
        cfg.coordinator_url.clone()
    };
    // CLI-05: when --tor is set, use two isolated Tor circuits (alice for input reg,
    // bob for output reg). Otherwise fall back to plain clearnet reqwest.
    let client = if cfg.use_tor {
        let handle = tor::init_tor(coordinator_url.clone()).await
            .map_err(|e| anyhow::anyhow!("Tor initialization failed: {e}"))?;
        let alice_proxy = handle.alice_proxy_url().to_owned();
        let bob_proxy = handle.bob_proxy_url().to_owned();
        CoordinatorClient::new_tor(coordinator_url, alice_proxy, bob_proxy)?
    } else {
        CoordinatorClient::new(coordinator_url)
    };

    // 10-minute ceiling per phase — prevents infinite hangs if coordinator crashes.
    let phase_timeout = tokio::time::Duration::from_secs(600);

    info!("Polling for INPUT_REG phase");
    let info = client.poll_until_phase("input_reg", cfg.poll_interval_ms, phase_timeout).await?;
    info!(round_id = ?info.round_id, "INPUT_REG phase detected");

    let reg_result = round::input::register_input(&client, &wallet, &info).await?;
    info!("Input registered successfully");

    client.poll_until_phase("output_reg", cfg.poll_interval_ms, phase_timeout).await?;
    info!("OUTPUT_REG phase detected");

    round::output::register_output(&client, &wallet, &reg_result, &info).await?;
    info!("Output registered successfully");

    client.poll_until_phase("signing", cfg.poll_interval_ms, phase_timeout).await?;
    info!("SIGNING phase detected");

    round::sign::verify_and_sign(&client, &wallet, &reg_result, cfg.poll_interval_ms).await?;
    info!("Signed successfully — round complete");

    Ok(())
}
