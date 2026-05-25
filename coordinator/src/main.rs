pub mod bitcoin;
pub mod config;

mod api;
mod blind;
mod discovery;
mod network;
mod round;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::oneshot;
use tracing::{error, info};

use config::CoordinatorConfig;
use bitcoin::rpc::BitcoinRpc;
use round::state::{Phase, RoundState};
use round::blame::{BanList, BlameOutcome};
use network::tor::serve_onion_service;

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
        tor_mode = cfg.coordinator.tor_mode,
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

    // Spawn a phase monitor that re-arms output_reg and signing timeout timers on
    // every new round, not just the first. The original one-shot spawn approach only
    // fired for round 1; this monitor polls every 500ms and arms a fresh timer task
    // the first time it sees each (round_id, phase) pair. (WR-02)
    {
        let round_clone = Arc::clone(&round_state);
        let ban_list_clone = Arc::clone(&ban_list);
        let blame_count_clone = Arc::clone(&blame_round_count);
        let ban_file = cfg.coordinator.ban_file_path.clone();
        let ban_duration = cfg.coordinator.blame_ban_duration_secs;
        let signing_timeout = Duration::from_secs(cfg.coordinator.round_timeout_signing_secs);
        let output_reg_timeout = Duration::from_secs(cfg.coordinator.round_timeout_output_reg_secs);

        tokio::spawn(async move {
            // Track the last (round_id, phase) for which we armed a timer so we never
            // double-arm the same phase of the same round.
            let mut last_output_reg_round: Option<uuid::Uuid> = None;
            let mut last_signing_round: Option<uuid::Uuid> = None;

            let mut ticker = tokio::time::interval(Duration::from_millis(500));
            loop {
                ticker.tick().await;

                let (phase, round_id) = {
                    let guard = round_clone.read().await;
                    (guard.phase.clone(), guard.round_id)
                };

                match phase {
                    Phase::OutputReg if last_output_reg_round != Some(round_id) => {
                        last_output_reg_round = Some(round_id);
                        let round_c = Arc::clone(&round_clone);
                        tracing::debug!(%round_id, "Arming output_reg timeout timer");
                        tokio::spawn(async move {
                            tokio::time::sleep(output_reg_timeout).await;
                            let mut round = round_c.write().await;
                            if round.round_id == round_id && round.phase == Phase::OutputReg {
                                round::output_reg::on_output_reg_timeout(&mut round);
                            }
                        });
                    }
                    Phase::Signing if last_signing_round != Some(round_id) => {
                        last_signing_round = Some(round_id);
                        let round_c = Arc::clone(&round_clone);
                        let ban_list_c = Arc::clone(&ban_list_clone);
                        let blame_count_c = Arc::clone(&blame_count_clone);
                        let ban_file_c = ban_file.clone();
                        tracing::debug!(%round_id, "Arming signing timeout timer");
                        tokio::spawn(async move {
                            tokio::time::sleep(signing_timeout).await;
                            let mut round = round_c.write().await;
                            if round.round_id != round_id || round.phase != Phase::Signing {
                                return; // already advanced — no-op
                            }
                            let mut bl = ban_list_c.write().await;
                            let count = blame_count_c.load(Ordering::Relaxed);
                            let outcome = round::blame::on_signing_timeout(
                                &mut round, &mut bl, &ban_file_c, ban_duration, count,
                            );
                            match outcome {
                                BlameOutcome::FullAbort => {
                                    blame_count_c.store(0, Ordering::Relaxed);
                                }
                                BlameOutcome::RestartWithout { .. } => {
                                    blame_count_c.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
        });
    }

    // Determine public address and start transport.
    // Tor mode: bootstrap Tor, launch onion service, receive .onion addr via channel.
    // Clearnet mode: bind TCP listener — identical to Phase 4 behaviour for tests/dev.
    // T-05-05: code paths are mutually exclusive — no TCP listener in tor_mode.
    let public_addr: String = if cfg.coordinator.tor_mode {
        let (addr_tx, addr_rx) = oneshot::channel::<String>();

        let app = api::build_router_with_ban_list(
            round_state.clone(),
            Arc::new(rpc),
            Arc::new(cfg.clone()),
            ban_list,
        );

        // Spawn the hidden service — it bootstraps Tor, sends the onion address, then
        // serves forever. T-05-04: on fatal error the process exits 1.
        tokio::spawn(async move {
            if let Err(e) = serve_onion_service(app, addr_tx).await {
                error!(error = %e, "Onion service fatal error");
                std::process::exit(1);
            }
        });

        // Wait for the .onion address (arrives within ~1s of Tor bootstrap completing).
        addr_rx.await.map_err(|_| anyhow::anyhow!(
            "Onion service task exited before sending address"
        ))?
    } else {
        // Clearnet path — Phase 4 compatible (default for tests/dev).
        let app = api::build_router_with_ban_list(
            round_state.clone(),
            Arc::new(rpc),
            Arc::new(cfg.clone()),
            ban_list,
        );
        let addr = cfg.coordinator.listen_addr.clone();
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!(addr = %addr, "Listening (clearnet)");
        // Spawn clearnet server so execution falls through to PKARR publish.
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        cfg.discovery.coordinator_public_addr.clone()
    };

    // Publish initial PKARR record using the resolved public address.
    // In tor_mode this is the .onion address; in clearnet mode it is coordinator_public_addr.
    {
        let p = Arc::clone(&publisher);
        let addr = public_addr.clone();
        let keypair = pkarr_keypair.clone();
        let denom = cfg.coordinator.denomination_sats;
        let min_p = cfg.coordinator.min_participants;
        if let Ok(packet) = discovery::pkarr_pub::build_coordinator_packet(
            &keypair, &addr, denom, min_p, "idle",
        ) {
            tokio::spawn(async move {
                if let Err(e) = p.publish_record(packet).await {
                    tracing::warn!("Initial PKARR publish failed: {e}");
                }
            });
        }
    }

    // Spawn PKARR heartbeat task — re-publishes every heartbeat_interval_secs (DISC-03).
    // Reads current round phase so the published status stays current.
    {
        let p = Arc::clone(&publisher);
        let round_clone = Arc::clone(&round_state);
        let keypair = pkarr_keypair.clone();
        let addr = public_addr.clone();   // resolved address (onion or clearnet)
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

    // In tor_mode the hidden service runs indefinitely in its own task.
    // In clearnet mode the TCP server runs in its own task.
    // Both cases: park the main task here rather than exit.
    std::future::pending::<()>().await;

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
