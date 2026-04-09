pub mod bitcoin;
pub mod config;

mod api;
mod blind;
mod discovery;
mod round;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};

use config::CoordinatorConfig;
use bitcoin::rpc::BitcoinRpc;
use round::state::{Phase, RoundState};
use round::blame::{BanList, BlameOutcome};

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

    // Initialize PKARR keypair and publisher (DISC-01, DISC-03)
    let pkarr_keypair = discovery::pkarr_pub::load_or_generate_keypair(
        &cfg.discovery.pkarr_key_file,
    )?;
    info!(
        pubkey = %pkarr_keypair.public_key().to_z32(),
        "PKARR identity ready — share this key with clients"
    );
    let publisher = Arc::new(
        discovery::pkarr_pub::PkarrPublisher::new(pkarr_keypair.clone())?,
    );

    // Publish initial record immediately (best-effort; coordinator is "idle" at startup)
    {
        let p = Arc::clone(&publisher);
        let addr = cfg.discovery.coordinator_public_addr.clone();
        let denom = cfg.coordinator.denomination_sats;
        let min_p = cfg.coordinator.min_participants;
        if let Ok(packet) = discovery::pkarr_pub::build_coordinator_packet(
            &pkarr_keypair, &addr, denom, min_p, "idle",
        ) {
            tokio::spawn(async move {
                if let Err(e) = p.publish_record(packet).await {
                    tracing::warn!("Initial PKARR publish failed: {e}");
                }
            });
        }
    }

    // Initialize shared round state
    let round_state: Arc<RwLock<RoundState>> = Arc::new(RwLock::new(RoundState::new_idle()));

    // Load unexpired ban entries from ban file on startup (BLAME-05, BLAME-06).
    // Missing file is not an error (first startup). Malformed lines are skipped (T-02-06).
    let ban_list: Arc<RwLock<BanList>> = Arc::new(RwLock::new(BanList::new()));
    {
        let now = round::blame::now_unix_secs();
        let ban_entries = round::blame::load_unexpired_entries(
            &cfg.coordinator.ban_file_path, now
        ).unwrap_or_else(|e| {
            tracing::warn!("Failed to load ban file: {e} — starting with empty ban list");
            vec![]
        });
        let entry_count = ban_entries.len();
        let mut bl = ban_list.write().await;
        for (utxo_hash, entry) in ban_entries {
            bl.load_entry(utxo_hash, entry);
        }
        info!(loaded = entry_count, "Loaded unexpired ban entries from ban file");
    }

    // blame_round_count tracks consecutive blame rounds for the cap (T-02-07).
    let blame_round_count: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));

    // Spawn signing phase timer — fires after round_timeout_signing_secs.
    // When it fires: detect non-signers, ban them, append to ban file, Blame→Idle.
    // BLAME-04.
    {
        let round_clone = Arc::clone(&round_state);
        let ban_list_clone = Arc::clone(&ban_list);
        let blame_count_clone = Arc::clone(&blame_round_count);
        let ban_file = cfg.coordinator.ban_file_path.clone();
        let ban_duration = cfg.coordinator.blame_ban_duration_secs;
        let signing_timeout = Duration::from_secs(cfg.coordinator.round_timeout_signing_secs);

        tokio::spawn(async move {
            tokio::time::sleep(signing_timeout).await;

            let mut round = round_clone.write().await;
            if round.phase != Phase::Signing {
                return; // already advanced — no-op
            }

            let mut bl = ban_list_clone.write().await;
            let count = blame_count_clone.load(Ordering::Relaxed);
            let outcome = round::blame::on_signing_timeout(
                &mut round, &mut bl, &ban_file, ban_duration, count,
            );
            match outcome {
                BlameOutcome::FullAbort => {
                    blame_count_clone.store(0, Ordering::Relaxed);
                }
                BlameOutcome::RestartWithout { .. } => {
                    blame_count_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }

    // Spawn output registration phase timer — fires after round_timeout_output_reg_secs.
    // When it fires: check if all outputs registered; if not, Blame→Idle.
    // BLAME-04.
    {
        let round_clone = Arc::clone(&round_state);
        let output_reg_timeout = Duration::from_secs(cfg.coordinator.round_timeout_output_reg_secs);

        tokio::spawn(async move {
            tokio::time::sleep(output_reg_timeout).await;

            let mut round = round_clone.write().await;
            if round.phase != Phase::OutputReg {
                return; // already advanced — no-op
            }

            round::output_reg::on_output_reg_timeout(&mut round);
        });
    }

    // Spawn PKARR heartbeat task — re-publishes every heartbeat_interval_secs (DISC-03).
    // Reads current round phase so the published status stays current.
    {
        let p = Arc::clone(&publisher);
        let round_clone = Arc::clone(&round_state);
        let keypair = pkarr_keypair.clone();
        let addr = cfg.discovery.coordinator_public_addr.clone();
        let denom = cfg.coordinator.denomination_sats;
        let min_p = cfg.coordinator.min_participants;
        let interval_secs = cfg.discovery.heartbeat_interval_secs;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(
                Duration::from_secs(interval_secs),
            );
            loop {
                ticker.tick().await;
                let status = {
                    let round = round_clone.read().await;
                    round.phase.as_str().to_string()
                };
                if let Ok(packet) = discovery::pkarr_pub::build_coordinator_packet(
                    &keypair, &addr, denom, min_p, &status,
                ) {
                    if let Err(e) = p.publish_record(packet).await {
                        tracing::warn!("PKARR heartbeat publish failed: {e}");
                    } else {
                        tracing::debug!(status, "PKARR heartbeat published");
                    }
                }
            }
        });
    }

    // Build and run the axum server (passes pre-loaded ban list)
    let app = api::build_router_with_ban_list(
        round_state.clone(),
        Arc::new(rpc),
        Arc::new(cfg.clone()),
        ban_list,
    );
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
