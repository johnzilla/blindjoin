//! Tor circuit isolation for blindjoin client.
//!
//! CLI-05: input registration and output registration use distinct Tor circuits.
//! Isolation is achieved via `TorClient::isolated_client()`, which gives two TorClient
//! handles that share no guard nodes or circuits.
//!
//! Architecture: each TorClient handle is wrapped in an in-process SOCKS5 proxy
//! (bound to 127.0.0.1:<ephemeral_port>). reqwest::Client instances are then
//! configured with `Proxy::all("socks5h://127.0.0.1:<port>")` so that all HTTP
//! traffic flows through the appropriate isolated Tor circuit.
//!
//! The SOCKS5 server implements the subset of RFC 1928 required by reqwest:
//! - No-auth greeting (0x05 / 0x00)
//! - CONNECT command for hostname (domain) targets (ATYP 0x03)
//! - CONNECT command for IPv4 targets (ATYP 0x01)
//!
//! Source: docs.rs/arti-client/latest/arti_client/struct.TorClient.html [VERIFIED]

use anyhow::Context;
use arti_client::{TorClient, TorClientConfig};
use tor_rtcompat::PreferredRuntime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Holds the bootstrapped TorClient with two isolated handles:
/// - `alice`: used for input registration (links operator to UTXO — must be isolated)
/// - `bob`: used for output registration (links to fresh output — must be isolated)
///
/// Both handles are obtained via `isolated_client()` so they provably share no circuits.
pub struct TorHandle {
    /// Isolated client for input registration phase (Alice role)
    alice: TorClient<PreferredRuntime>,
    /// Isolated client for output registration phase (Bob role)
    bob: TorClient<PreferredRuntime>,
    /// The coordinator's base URL (scheme + host, no trailing slash)
    coordinator_url: String,
}

impl TorHandle {
    /// Bootstrap Tor and create two isolated client handles.
    ///
    /// `coordinator_url` must be the full coordinator URL, e.g. `http://xyz.onion`.
    pub async fn new(coordinator_url: String) -> anyhow::Result<Self> {
        tracing::info!("Bootstrapping Tor — this may take 10-30 seconds");
        let base = TorClient::create_bootstrapped(TorClientConfig::default())
            .await
            .context("Tor bootstrap failed — check network connectivity")?;

        // isolated_client() guarantees distinct guard nodes + circuits (CLI-05).
        // Source: docs.rs/arti-client/0.41.0/arti_client/struct.TorClient.html#method.isolated_client
        let alice = base.isolated_client();
        let bob = base.isolated_client();

        tracing::info!("Tor ready — two isolated circuits allocated");
        Ok(Self { alice, bob, coordinator_url })
    }

    /// Returns the coordinator URL for the Alice (input registration) circuit.
    pub fn alice_url(&self) -> &str {
        &self.coordinator_url
    }

    /// Returns the coordinator URL for the Bob (output registration) circuit.
    pub fn bob_url(&self) -> &str {
        &self.coordinator_url
    }

    /// Spawn an in-process SOCKS5 proxy for the Alice circuit.
    /// Returns `socks5h://127.0.0.1:<port>` — the URL to pass to reqwest Proxy::all().
    ///
    /// T-05-08: binds 127.0.0.1:0 (ephemeral, loopback-only — not network-accessible).
    pub async fn alice_proxy_url(&self) -> anyhow::Result<String> {
        let port = launch_socks5_proxy(self.alice.clone()).await?;
        Ok(format!("socks5h://127.0.0.1:{port}"))
    }

    /// Spawn an in-process SOCKS5 proxy for the Bob circuit.
    /// Returns `socks5h://127.0.0.1:<port>` — the URL to pass to reqwest Proxy::all().
    pub async fn bob_proxy_url(&self) -> anyhow::Result<String> {
        let port = launch_socks5_proxy(self.bob.clone()).await?;
        Ok(format!("socks5h://127.0.0.1:{port}"))
    }
}

/// Convenience: bootstrap Tor and return a TorHandle.
pub async fn init_tor(coordinator_url: String) -> anyhow::Result<TorHandle> {
    TorHandle::new(coordinator_url).await
}

/// Bind a TCP listener on 127.0.0.1:0, spawn a SOCKS5 server task routing through
/// the given TorClient, and return the assigned port.
///
/// The SOCKS5 implementation handles the subset required by reqwest:
/// - RFC 1928 no-auth greeting
/// - CONNECT command with hostname (0x03) and IPv4 (0x01) address types
async fn launch_socks5_proxy(
    tor: TorClient<PreferredRuntime>,
) -> anyhow::Result<u16> {
    // T-05-08: loopback-only, OS-assigned ephemeral port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind SOCKS5 listener")?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _peer)) => {
                    let tor = tor.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_socks5(stream, tor).await {
                            tracing::debug!(error = %e, "SOCKS5 connection error");
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "SOCKS5 listener accept error");
                    break;
                }
            }
        }
    });

    Ok(port)
}

/// Handle one SOCKS5 connection: perform handshake, parse CONNECT request,
/// open a Tor stream to the target, then relay bytes bidirectionally.
async fn handle_socks5(
    mut stream: TcpStream,
    tor: TorClient<PreferredRuntime>,
) -> anyhow::Result<()> {
    // --- Greeting ---
    // Client: \x05 <nmethods> <methods...>
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    let _version = buf[0]; // 0x05
    let nmethods = buf[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;
    // We only support no-auth (0x00).
    stream.write_all(&[0x05, 0x00]).await?;

    // --- Request ---
    // Client: \x05 <cmd> \x00 <atyp> <addr...> <port_hi> <port_lo>
    let mut req_hdr = [0u8; 4];
    stream.read_exact(&mut req_hdr).await?;
    // req_hdr[0] = 0x05 (version)
    // req_hdr[1] = cmd (0x01 = CONNECT)
    // req_hdr[2] = 0x00 (reserved)
    // req_hdr[3] = atyp
    let atyp = req_hdr[3];

    let target_host: String = match atyp {
        0x01 => {
            // IPv4
            let mut ipv4 = [0u8; 4];
            stream.read_exact(&mut ipv4).await?;
            format!("{}.{}.{}.{}", ipv4[0], ipv4[1], ipv4[2], ipv4[3])
        }
        0x03 => {
            // Domain name
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut name = vec![0u8; len[0] as usize];
            stream.read_exact(&mut name).await?;
            String::from_utf8(name).context("Invalid UTF-8 in SOCKS5 hostname")?
        }
        0x04 => {
            // IPv6
            let mut ipv6 = [0u8; 16];
            stream.read_exact(&mut ipv6).await?;
            let addr = std::net::Ipv6Addr::from(ipv6);
            format!("[{addr}]")
        }
        other => {
            anyhow::bail!("Unsupported SOCKS5 address type: 0x{other:02x}");
        }
    };

    let mut port_bytes = [0u8; 2];
    stream.read_exact(&mut port_bytes).await?;
    let port = u16::from_be_bytes(port_bytes);

    let target = format!("{target_host}:{port}");

    // Open a Tor stream to the target using the isolated TorClient.
    let tor_stream = tor
        .connect(target.as_str())
        .await
        .with_context(|| format!("Tor CONNECT to {target} failed"))?;

    // Reply: success (0x00), bind addr 0.0.0.0:0 (not meaningful for CONNECT)
    // \x05 \x00 \x00 \x01 <4-byte-ip> <2-byte-port>
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        .await?;

    // Relay bytes bidirectionally between the SOCKS client and the Tor stream.
    // DataStream implements AsyncRead + AsyncWrite.
    let (mut local_rx, mut local_tx) = stream.into_split();
    let (mut tor_rx, mut tor_tx) = tokio::io::split(tor_stream);

    let client_to_tor = tokio::io::copy(&mut local_rx, &mut tor_tx);
    let tor_to_client = tokio::io::copy(&mut tor_rx, &mut local_tx);

    // Run both directions concurrently; finish when either side closes.
    tokio::select! {
        _ = client_to_tor => {},
        _ = tor_to_client => {},
    }

    Ok(())
}
