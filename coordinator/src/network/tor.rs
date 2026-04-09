//! Tor hidden service transport for the coordinator.
//!
//! When `tor_mode = true`, this module bootstraps an arti TorClient,
//! launches a v3 onion service, and serves the axum router over it
//! using hyper's HTTP/1.1 server. No TCP listener is created.
//!
//! The function sends the .onion address via a oneshot channel so that
//! main.rs can wait for it before publishing the initial PKARR record.

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

pub async fn serve_onion_service(
    app: axum::Router,
    addr_tx: tokio::sync::oneshot::Sender<String>,
) -> anyhow::Result<()> {
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
    let _ = addr_tx.send(onion_addr);

    // handle_rend_requests converts the RendRequest stream → StreamRequest stream,
    // accepting all rendezvous handshakes automatically.
    let stream_requests = handle_rend_requests(rend_requests);
    tokio::pin!(stream_requests);

    while let Some(stream_req) = stream_requests.next().await {
        // T-05-01: Only accept BEGIN (HTTP) streams; non-BEGIN variants are rejected
        // by handle_rend_requests filter before reaching here — all items are StreamRequests
        // which already correspond to accepted BEGIN messages.

        // Accept the stream, sending a CONNECTED cell to the client.
        // Connected::new_empty() sends a CONNECTED cell with no address hint — correct for HS.
        let data_stream = match stream_req.accept(Connected::new_empty()).await {
            Ok(ds) => ds,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to accept HS stream");
                continue;
            }
        };

        let io = TokioIo::new(data_stream);
        // Wrap axum::Router (tower::Service) into a hyper-compatible service.
        let svc = TowerToHyperService::new(app.clone());
        tokio::spawn(async move {
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
