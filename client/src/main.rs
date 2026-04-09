use clap::Parser;
use tracing::info;

mod config;
mod discover;
mod http;
mod round;
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
        let utxo_value = cfg.utxo_value_sats.unwrap_or(0);
        ClientWallet::generate(utxo, utxo_value, network)?;
        std::process::exit(0);
    }

    // Require --utxo and --utxo-value-sats for round participation
    let utxo = cfg.utxo.as_deref()
        .ok_or_else(|| anyhow::anyhow!("--utxo is required for round participation"))?;
    let utxo_value_sats = cfg.utxo_value_sats
        .ok_or_else(|| anyhow::anyhow!("--utxo-value-sats is required for round participation"))?;

    let wallet = if let Some(descriptor) = cfg.descriptor.as_deref() {
        // --descriptor path: requires --utxo-address
        let utxo_address = cfg.utxo_address.as_deref()
            .ok_or_else(|| anyhow::anyhow!("--utxo-address is required when using --descriptor"))?;
        ClientWallet::from_descriptor(descriptor, utxo, utxo_value_sats, utxo_address, network)?
    } else {
        // WIF path (backward compat, default for testing)
        let wif = cfg.utxo_wif.as_deref()
            .ok_or_else(|| anyhow::anyhow!("--utxo-wif is required when not using --descriptor or --generate-wallet"))?;
        ClientWallet::from_wif(wif, utxo, utxo_value_sats, network)?
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
    let client = CoordinatorClient::new(coordinator_url);

    info!("Polling for INPUT_REG phase");
    let info = client.poll_until_phase("input_reg", cfg.poll_interval_ms).await?;
    info!(round_id = ?info.round_id, "INPUT_REG phase detected");

    let reg_result = round::input::register_input(&client, &wallet, &info).await?;
    info!("Input registered successfully");

    client.poll_until_phase("output_reg", cfg.poll_interval_ms).await?;
    info!("OUTPUT_REG phase detected");

    round::output::register_output(&client, &wallet, &reg_result, &info).await?;
    info!("Output registered successfully");

    client.poll_until_phase("signing", cfg.poll_interval_ms).await?;
    info!("SIGNING phase detected");

    round::sign::verify_and_sign(&client, &wallet, &reg_result, cfg.poll_interval_ms).await?;
    info!("Signed successfully — round complete");

    Ok(())
}
