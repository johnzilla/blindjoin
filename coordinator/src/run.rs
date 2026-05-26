//! Coordinator process entry point as a library function.
//!
//! `main.rs` is a thin wrapper that loads `CoordinatorConfig` and calls `run(cfg)`.
//! Integration tests can invoke `run(cfg)` directly to exercise the same startup
//! path as the production binary — there is no separate test bootstrap.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use anyhow::Context;
use tokio::sync::RwLock;
use tokio::sync::oneshot;
use tracing::{error, info};

use crate::bitcoin::rpc::BitcoinRpc;
use crate::config::CoordinatorConfig;
use crate::network::tor::serve_onion_service;
use crate::round::blame::{BanList, BlameOutcome};
use crate::round::manager::start_round;
use crate::round::state::{Phase, RoundState};
use crate::{api, discovery};

/// Run the coordinator with the supplied configuration. Runs forever; returns
/// an error only if startup fails.
///
/// This is the canonical startup path used by both `main.rs` and integration tests.
/// No `#[cfg(test)]` branches — production and test bootstrap the round identically.
///
/// **Continuous-rounds policy:** the first round starts immediately after the
/// HTTP transport is up. When the FSM returns to Idle (Broadcast→Idle,
/// Blame→Idle, or InputReg-quorum-failure→Idle), the phase monitor re-arms a
/// fresh round with a new RSA keypair. The coordinator never stays Idle.
pub async fn run(cfg: CoordinatorConfig) -> anyhow::Result<()> {
    info!(
        network = %cfg.network.bitcoin_network,
        denomination_sats = cfg.coordinator.denomination_sats,
        min_participants = cfg.coordinator.min_participants,
        tor_mode = cfg.coordinator.tor_mode,
        "Coordinator starting"
    );

    // Phase 8 CR-01 / CR-02: validate hardening knobs once, up front, so
    // misconfiguration produces a single structured error here instead of a
    // deep-stack panic from `GovernorConfigBuilder::finish().expect(..)` or a
    // silent deadlock from `Semaphore::new(0).acquire_owned().await`.
    cfg.validate().context("Invalid coordinator configuration")?;

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

    // Initialize shared round state — starts Idle. The phase monitor below will
    // call start_round() on the next tick to bootstrap the first round.
    let round_state: Arc<RwLock<RoundState>> = Arc::new(RwLock::new(RoundState::new_idle()));

    // Load unexpired ban entries from ban file on startup (BLAME-05, BLAME-06).
    // Missing file is not an error (first startup). Malformed lines are skipped (T-02-06).
    let ban_list: Arc<RwLock<BanList>> = Arc::new(RwLock::new(BanList::new()));
    {
        let now = crate::round::blame::now_unix_secs();
        let ban_entries = crate::round::blame::load_unexpired_entries(
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

    // Spawn a phase monitor that:
    //   1. On Idle (including initial boot) → call start_round() to begin a new round.
    //   2. On InputReg → arm a timeout that advances to OutputReg if quorum met,
    //      or aborts back to Idle (continuous-rounds policy) otherwise.
    //   3. On OutputReg → arm a timeout that advances to Signing (or Blame on
    //      missing outputs).
    //   4. On Signing → arm a timeout that runs blame (ban non-signers, decide
    //      FullAbort vs RestartWithout).
    //
    // The monitor polls every 500ms and arms a fresh timer task the first time
    // it sees each (round_id, phase) pair, so timeouts re-arm every round (WR-02).
    {
        let round_clone = Arc::clone(&round_state);
        let ban_list_clone = Arc::clone(&ban_list);
        let blame_count_clone = Arc::clone(&blame_round_count);
        let ban_file = cfg.coordinator.ban_file_path.clone();
        let ban_duration = cfg.coordinator.blame_ban_duration_secs;
        let min_participants = cfg.coordinator.min_participants;
        let input_reg_timeout = Duration::from_secs(cfg.coordinator.round_timeout_input_reg_secs);
        let signing_timeout = Duration::from_secs(cfg.coordinator.round_timeout_signing_secs);
        let output_reg_timeout = Duration::from_secs(cfg.coordinator.round_timeout_output_reg_secs);

        tokio::spawn(async move {
            // Track the last (round_id, phase) for which we acted so we never
            // double-arm the same phase of the same round (or re-start a round
            // we've already started).
            let mut last_idle_start_round: Option<uuid::Uuid> = None;
            let mut last_input_reg_round: Option<uuid::Uuid> = None;
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
                    Phase::Idle if last_idle_start_round != Some(round_id) => {
                        // First time we've seen this Idle round_id — start a round.
                        // `transition_to(Phase::Idle)` always assigns a fresh round_id,
                        // so this branch fires exactly once per Idle cycle.
                        last_idle_start_round = Some(round_id);
                        let mut guard = round_clone.write().await;
                        // Re-check under write lock — concurrent handler may have
                        // moved the FSM forward while we waited for the lock.
                        if guard.phase != Phase::Idle || guard.round_id != round_id {
                            continue;
                        }
                        match start_round(&mut guard) {
                            Ok(()) => {
                                info!(
                                    round_id = %guard.round_id,
                                    "New round started in input_reg"
                                );
                            }
                            Err(e) => {
                                error!(error = %e, "start_round failed — will retry next tick");
                                // Allow retry on the next tick.
                                last_idle_start_round = None;
                            }
                        }
                    }
                    Phase::InputReg if last_input_reg_round != Some(round_id) => {
                        last_input_reg_round = Some(round_id);
                        let round_c = Arc::clone(&round_clone);
                        tracing::debug!(%round_id, "Arming input_reg timeout timer");
                        tokio::spawn(async move {
                            tokio::time::sleep(input_reg_timeout).await;
                            let mut round = round_c.write().await;
                            if round.round_id != round_id || round.phase != Phase::InputReg {
                                return; // already advanced — no-op
                            }
                            if round.participant_count >= min_participants {
                                // Quorum reached — advance to OutputReg.
                                if let Err(e) = round.transition_to(Phase::OutputReg) {
                                    tracing::warn!(
                                        %round_id, error = %e,
                                        "InputReg → OutputReg transition rejected"
                                    );
                                } else {
                                    tracing::info!(
                                        %round_id,
                                        participant_count = round.participant_count,
                                        "InputReg quorum reached — advancing to OutputReg"
                                    );
                                }
                            } else {
                                // Quorum failure — no blame (nobody to blame), just
                                // reset to Idle. The monitor's Idle branch will pick
                                // up the fresh round_id on its next tick and call
                                // start_round() again with a brand-new RSA keypair.
                                tracing::info!(
                                    %round_id,
                                    participant_count = round.participant_count,
                                    min_participants,
                                    "InputReg quorum NOT reached — aborting back to Idle"
                                );
                                if let Err(e) = round.transition_to(Phase::Idle) {
                                    tracing::warn!(
                                        %round_id, error = %e,
                                        "InputReg → Idle transition rejected"
                                    );
                                }
                            }
                        });
                    }
                    Phase::OutputReg if last_output_reg_round != Some(round_id) => {
                        last_output_reg_round = Some(round_id);
                        let round_c = Arc::clone(&round_clone);
                        tracing::debug!(%round_id, "Arming output_reg timeout timer");
                        tokio::spawn(async move {
                            tokio::time::sleep(output_reg_timeout).await;
                            let mut round = round_c.write().await;
                            if round.round_id == round_id && round.phase == Phase::OutputReg {
                                crate::round::output_reg::on_output_reg_timeout(&mut round);
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
                            let outcome = crate::round::blame::on_signing_timeout(
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

        // T-08-03-01: thread the connection-cap value through to the accept loop.
        // The semaphore inside serve_onion_service bounds in-flight HS streams.
        let max_concurrent_connections = cfg.coordinator.max_concurrent_connections;

        // Spawn the hidden service — it bootstraps Tor, sends the onion address, then
        // serves forever. T-05-04: on fatal error the process exits 1.
        tokio::spawn(async move {
            if let Err(e) =
                serve_onion_service(app, addr_tx, max_concurrent_connections).await
            {
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
        // T-08-03-05 (accept): the max_concurrent_connections cap is enforced only
        // in tor_mode = true. The clearnet path uses axum::serve which has its own
        // internal accept loop and is intentionally NOT capped per Phase 8 A4
        // resolution — clearnet is dev/test only per CONTEXT D-01.
        tracing::warn!(
            max_concurrent_connections = cfg.coordinator.max_concurrent_connections,
            "Clearnet mode: max_concurrent_connections is NOT enforced — clearnet is dev/test only. Production deployments must use tor_mode = true."
        );
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
