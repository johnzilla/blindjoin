use clap::Parser;
use tracing::info;

mod config;
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

    let wallet = ClientWallet::from_wif(
        &cfg.utxo_wif,
        &cfg.utxo,
        cfg.utxo_value_sats,
        network,
    )?;

    let client = CoordinatorClient::new(cfg.coordinator_url.clone());

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
