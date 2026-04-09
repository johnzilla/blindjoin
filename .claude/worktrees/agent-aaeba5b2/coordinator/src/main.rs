pub mod bitcoin;
pub mod config;

mod api;
mod blind;
mod round;

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use config::CoordinatorConfig;
use bitcoin::rpc::BitcoinRpc;
use round::state::RoundState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging — never log PII (PRIV-02)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("coordinator=info".parse().unwrap()),
        )
        .init();

    // Load configuration (D-11)
    let cfg = CoordinatorConfig::load().unwrap_or_else(|e| {
        error!(error = %e, "Config load failed — using defaults");
        CoordinatorConfig::with_defaults()
    });

    info!(
        network = %cfg.network.bitcoin_network,
        denomination_sats = cfg.coordinator.denomination_sats,
        min_participants = cfg.coordinator.min_participants,
        "Coordinator starting"
    );

    // Fail-fast startup health checks (D-12)
    let rpc = BitcoinRpc::new(
        cfg.network.bitcoin_rpc_url.clone(),
        cfg.network.bitcoin_rpc_user.clone(),
        cfg.network.bitcoin_rpc_pass.clone(),
    );
    startup_health_check(&rpc).await?;

    // Initialize shared round state
    let round_state: Arc<RwLock<RoundState>> = Arc::new(RwLock::new(RoundState::new_idle()));

    // Build and run the axum server
    let app = api::build_router(round_state.clone(), Arc::new(rpc), Arc::new(cfg.clone()));
    let listener = tokio::net::TcpListener::bind(&cfg.coordinator.listen_addr).await?;
    info!(addr = %cfg.coordinator.listen_addr, "Listening");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn startup_health_check(rpc: &BitcoinRpc) -> anyhow::Result<()> {
    // 1. Verify bitcoind reachable
    let block_count = rpc.getblockcount().await.map_err(|e| {
        anyhow::anyhow!("Bitcoin Core unreachable at startup: {e}. Is bitcoind running?")
    })?;
    info!(block_count, "Bitcoin Core reachable");

    // 2. Block count > 0 means not in a trivially bad state
    if block_count == 0 {
        anyhow::bail!("Bitcoin Core reports 0 blocks — may be misconfigured or not synced");
    }

    Ok(())
}
