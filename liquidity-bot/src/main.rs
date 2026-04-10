//! Liquidity bot for blindjoin.
//!
//! Polls a coordinator and auto-joins CoinJoin rounds using pre-funded signet UTXOs.
//! All configuration via environment variables (Docker-friendly, no config file).
//!
//! Required env vars:
//!   BLINDJOIN_COORDINATOR_URL   — coordinator HTTP URL (e.g. "http://coordinator:8080")
//!   BLINDJOIN_NETWORK           — must be "signet" (safety guard)
//!   BLINDJOIN_UTXO              — UTXO to register (format: "txid:vout")
//!   BLINDJOIN_UTXO_VALUE_SATS   — UTXO value in satoshis
//!   BLINDJOIN_UTXO_WIF          — WIF private key for the UTXO
//!
//! Optional:
//!   BLINDJOIN_TARGET_DENOMINATION_SATS — denomination to join (default: 1000000)
//!   BLINDJOIN_JOIN_THRESHOLD           — stop joining above this participant count (default: 10)

mod strategy;

use std::time::Duration;
use anyhow::{bail, Context, Result};
use tracing::{error, info, warn};
use client::http::CoordinatorClient;
use client::wallet::ClientWallet;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("liquidity_bot=info".parse().unwrap()),
        )
        .init();

    // --- Configuration from environment ---
    let coordinator_url = std::env::var("BLINDJOIN_COORDINATOR_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let network_str = std::env::var("BLINDJOIN_NETWORK")
        .unwrap_or_else(|_| "signet".to_string());

    // SAFETY GUARD: refuse to run on non-signet.
    // The bot is a testing tool; running with mainnet keys would risk real funds.
    if network_str != "signet" {
        bail!(
            "Liquidity bot refuses to start: BLINDJOIN_NETWORK='{}' is not 'signet'. \
             The bot is a signet-only testing tool.",
            network_str
        );
    }
    let network = bitcoin::Network::Signet;

    let utxo = std::env::var("BLINDJOIN_UTXO")
        .context("BLINDJOIN_UTXO env var required (format: txid:vout)")?;
    let utxo_value_sats: u64 = std::env::var("BLINDJOIN_UTXO_VALUE_SATS")
        .context("BLINDJOIN_UTXO_VALUE_SATS env var required")?
        .parse()
        .context("BLINDJOIN_UTXO_VALUE_SATS must be a u64")?;
    let utxo_wif = std::env::var("BLINDJOIN_UTXO_WIF")
        .context("BLINDJOIN_UTXO_WIF env var required")?;

    let target_denomination_sats: u64 = std::env::var("BLINDJOIN_TARGET_DENOMINATION_SATS")
        .unwrap_or_else(|_| "1000000".to_string())
        .parse()
        .context("BLINDJOIN_TARGET_DENOMINATION_SATS must be a u64")?;

    let join_threshold: u32 = std::env::var("BLINDJOIN_JOIN_THRESHOLD")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .context("BLINDJOIN_JOIN_THRESHOLD must be a u32")?;

    // --- Setup ---
    let mut join_strategy = strategy::JoinStrategy::new(target_denomination_sats);
    join_strategy.join_threshold = join_threshold;

    let http = CoordinatorClient::new(coordinator_url.clone());

    info!(
        coordinator_url = %coordinator_url,
        utxo = %utxo,
        target_denomination_sats,
        "Liquidity bot starting (signet only)"
    );

    // --- Main polling loop ---
    // NOTE (RESEARCH.md Pitfall 3 — UTXO rotation):
    // After a successful round the bot's UTXO is spent. For Phase 4, the bot
    // performs ONE join attempt per run and then exits. Docker Compose restarts it
    // (restart: unless-stopped) which re-reads env vars. The operator must update
    // BLINDJOIN_UTXO/WIF after each round. A future enhancement (FAUCET-01) could
    // auto-detect the change UTXO from the broadcast tx.
    let mut consecutive_failures: u32 = 0;
    loop {
        // Respect long backoff after repeated failures
        if consecutive_failures >= join_strategy.max_consecutive_failures {
            warn!(
                consecutive_failures,
                "Too many consecutive failures — sleeping 300s before retry"
            );
            tokio::time::sleep(Duration::from_secs(300)).await;
            consecutive_failures = 0;
        }

        // Poll coordinator info
        let info = match http.get_info().await {
            Ok(i) => i,
            Err(e) => {
                warn!(error = %e, "Failed to GET /info — coordinator may be starting up");
                consecutive_failures += 1;
                tokio::time::sleep(Duration::from_secs(join_strategy.poll_interval_secs)).await;
                continue;
            }
        };

        if !join_strategy.should_join(&info) {
            // Not the right time to join; poll again
            tokio::time::sleep(Duration::from_secs(join_strategy.poll_interval_secs)).await;
            continue;
        }

        info!(
            round_state = %info.round_state,
            participants_registered = info.participants_registered,
            denomination_sats = info.denomination_sats,
            "Joining round"
        );

        // Build wallet for this round. Re-created each loop to handle UTXO state reset.
        let wallet = match ClientWallet::from_wif(&utxo_wif, &utxo, utxo_value_sats, network) {
            Ok(w) => w,
            Err(e) => {
                error!(error = %e, "Failed to build wallet from WIF");
                consecutive_failures += 1;
                tokio::time::sleep(Duration::from_secs(join_strategy.poll_interval_secs)).await;
                continue;
            }
        };

        // Full round participation using client library
        match participate_in_round(&http, &wallet, &info).await {
            Ok(()) => {
                info!("Round participation complete");
                consecutive_failures = 0;
                // Exit after one successful round (UTXO is now spent).
                // Docker Compose restart policy will re-launch with updated env vars.
                info!("Bot exiting — UTXO spent. Update BLINDJOIN_UTXO/WIF and restart.");
                return Ok(());
            }
            Err(e) => {
                warn!(error = %e, "Round participation failed");
                consecutive_failures += 1;
            }
        }

        tokio::time::sleep(Duration::from_secs(join_strategy.poll_interval_secs)).await;
    }
}

async fn participate_in_round(
    http: &CoordinatorClient,
    wallet: &ClientWallet,
    info: &shared::protocol::InfoResponse,
) -> Result<()> {
    // Input registration
    let state = client::round::input::register_input(http, wallet, info).await
        .context("Input registration failed")?;
    info!("Input registered");

    // Wait for output_reg phase
    http.poll_until_phase("output_reg", 1000, tokio::time::Duration::from_secs(600)).await
        .context("Timeout waiting for output_reg phase")?;
    info!("OUTPUT_REG phase detected");

    // Output registration
    client::round::output::register_output(http, wallet, &state, info).await
        .context("Output registration failed")?;
    info!("Output registered");

    // Wait for signing phase
    http.poll_until_phase("signing", 1000, tokio::time::Duration::from_secs(600)).await
        .context("Timeout waiting for signing phase")?;
    info!("SIGNING phase detected");

    // Verify PSBT and sign
    client::round::sign::verify_and_sign(http, wallet, &state, 1000).await
        .context("Sign phase failed")?;
    info!("Signed successfully");

    Ok(())
}
