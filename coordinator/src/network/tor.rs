//! Tor hidden service transport for the coordinator.
//!
//! When `tor_mode = true`, this module bootstraps an arti TorClient,
//! launches a v3 onion service, and serves the axum router over it
//! using hyper's HTTP/1.1 server. No TCP listener is created.
//!
//! The function sends the .onion address via a oneshot channel so that
//! main.rs can wait for it before publishing the initial PKARR record.

use std::sync::Arc;

use arti_client::{TorClient, TorClientConfig};
use arti_client::config::onion_service::OnionServiceConfigBuilder;
use tor_hsservice::handle_rend_requests;
use tor_cell::relaycell::msg::Connected;
use safelog::DisplayRedacted as _;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use hyper::server::conn::http1;
use futures_util::StreamExt;
use anyhow::Context;
use tokio::sync::Semaphore;

pub async fn serve_onion_service(
    app: axum::Router,
    addr_tx: tokio::sync::oneshot::Sender<String>,
    max_concurrent_connections: u32,
) -> anyhow::Result<()> {
    // Phase 8 CR-02 defense-in-depth: refuse to start the accept loop with a
    // zero-capacity semaphore. `CoordinatorConfig::validate()` is the primary
    // fence; this `ensure!` makes the requirement local so a future caller
    // that bypasses `run::run` (custom embedding, direct test invocation)
    // still gets an actionable error instead of a silent deadlock on the
    // first `Semaphore::acquire_owned().await`.
    anyhow::ensure!(
        max_concurrent_connections >= 1,
        "max_concurrent_connections must be >= 1; got 0. \
         Set BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS to a positive value \
         (recommended minimum 8).",
    );

    // Bootstrap a Tor client — this connects to the Tor network.
    // T-05-04: runs inside tokio::spawn in main.rs; if this fails the process exits 1.
    let tor_client = TorClient::create_bootstrapped(TorClientConfig::default())
        .await
        .context("Failed to bootstrap Tor — check network connectivity")?;

    // Build onion service config with a fixed nickname for key persistence.
    let hs_config = OnionServiceConfigBuilder::default()
        .nickname(
            "blindjoin"
                .to_owned()
                .try_into()
                .context("Invalid HS nickname")?,
        )
        .build()
        .context("Failed to build onion service config")?;

    // Launch the onion service. Returns Ok(None) if disabled in config,
    // but we never disable it here — the caller gated on tor_mode = true.
    let (onion_service, rend_requests) = tor_client
        .launch_onion_service(hs_config)?
        .context("Onion service returned None — onion-service-service feature must be enabled")?;

    // onion_address() returns Option<HsId>; HsId implements DisplayRedacted (not Display).
    // display_unredacted() gives the full .onion domain string.
    let onion_addr = onion_service
        .onion_address()
        .context("Onion address not available after launch_onion_service")?
        .display_unredacted()
        .to_string();

    tracing::info!(
        addr = %onion_addr,
        "Tor hidden service launched — descriptor propagation may take 30-60s"
    );

    // Send onion address to main.rs BEFORE entering the accept loop,
    // so PKARR publish can proceed immediately (T-05-03: clients have retry logic).
    // Propagate error: if the receiver was dropped before we could send, stop here
    // rather than entering the accept loop in a zombie state (serving connections
    // but with no PKARR record published for this session).
    addr_tx.send(onion_addr).map_err(|_| {
        anyhow::anyhow!("main task dropped the address receiver before onion address was delivered")
    })?;

    // handle_rend_requests converts the RendRequest stream → StreamRequest stream,
    // accepting all rendezvous handshakes automatically.
    let stream_requests = handle_rend_requests(rend_requests);
    tokio::pin!(stream_requests);

    // T-08-03-01: cap concurrent in-flight HS streams via a tokio semaphore.
    // RESEARCH §"Pattern 3" + PATTERNS §"coordinator/src/network/tor.rs (semaphore
    // around the accept loop)". The permit is acquired BEFORE `stream_req.accept(...)`
    // (RESEARCH Anti-Pattern: acquiring after accept defeats the cap), released on
    // accept failure (T-08-03-03), and moved into the spawned task body so it drops
    // when the connection's HTTP serve loop exits (RESEARCH Pitfall 5; T-08-03-02).
    // `Semaphore::new` is infallible; `.expect("semaphore never closed")` only fires
    // on a closed semaphore, which never happens here (we never call `.close()`).
    let conn_sem = Arc::new(Semaphore::new(max_concurrent_connections as usize));
    tracing::info!(
        cap = max_concurrent_connections,
        "Connection cap configured on Tor accept loop"
    );

    while let Some(stream_req) = stream_requests.next().await {
        // T-05-01: Only accept BEGIN (HTTP) streams; non-BEGIN variants are rejected
        // by handle_rend_requests filter before reaching here — all items are StreamRequests
        // which already correspond to accepted BEGIN messages.

        // T-08-03-04: acquire the connection-cap permit BEFORE accepting the stream.
        // When `max_concurrent_connections` streams are in flight, the (N+1)th call
        // parks here — Tor sees no BEGIN ack until an earlier connection finishes.
        let permit = Arc::clone(&conn_sem)
            .acquire_owned()
            .await
            .expect("semaphore never closed");

        // Accept the stream, sending a CONNECTED cell to the client.
        // Connected::new_empty() sends a CONNECTED cell with no address hint — correct for HS.
        let data_stream = match stream_req.accept(Connected::new_empty()).await {
            Ok(ds) => ds,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to accept HS stream");
                // T-08-03-03: release the slot on accept failure so it does not leak.
                drop(permit);
                continue;
            }
        };

        let io = TokioIo::new(data_stream);
        // Wrap axum::Router (tower::Service) into a hyper-compatible service.
        let svc = TowerToHyperService::new(app.clone());
        tokio::spawn(async move {
            // T-08-03-02 / RESEARCH Pitfall 5: hold the permit for the connection's
            // full lifetime. Dropping the permit before the spawned task starts would
            // effectively make the cap unlimited; the `_` prefix prevents an
            // unused-variable warning while keeping the binding alive.
            let _permit = permit;
            if let Err(e) = http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!(error = %e, "HS connection closed");
            }
        });
    }

    Ok(())
}
