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
///
/// The SOCKS5 proxy tasks are started at construction time and their JoinHandles are
/// stored here. When `TorHandle` is dropped, the tasks are aborted so no OS port
/// allocations or TorClient handles are leaked.
pub struct TorHandle {
    /// The coordinator's base URL (scheme + host, no trailing slash)
    coordinator_url: String,
    /// Proxy URL for the Alice (input registration) circuit.
    alice_proxy: String,
    /// Proxy URL for the Bob (output registration) circuit.
    bob_proxy: String,
    /// Background task handle for the Alice SOCKS5 proxy listener; aborted on drop.
    _alice_task: tokio::task::JoinHandle<()>,
    /// Background task handle for the Bob SOCKS5 proxy listener; aborted on drop.
    _bob_task: tokio::task::JoinHandle<()>,
}

impl TorHandle {
    /// Bootstrap Tor and create two isolated client handles.
    ///
    /// `coordinator_url` must be the full coordinator URL, e.g. `http://xyz.onion`.
    ///
    /// SOCKS5 proxies are started immediately so that the handles returned by
    /// `alice_proxy_url` / `bob_proxy_url` are ready to use without an extra await.
    pub async fn new(coordinator_url: String) -> anyhow::Result<Self> {
        tracing::info!("Bootstrapping Tor — this may take 10-30 seconds");
        let base = TorClient::create_bootstrapped(TorClientConfig::default())
            .await
            .context("Tor bootstrap failed — check network connectivity")?;

        // isolated_client() guarantees distinct guard nodes + circuits (CLI-05).
        // Source: docs.rs/arti-client/0.41.0/arti_client/struct.TorClient.html#method.isolated_client
        let alice = base.isolated_client();
        let bob = base.isolated_client();

        // Start SOCKS5 proxies at construction time and store JoinHandles.
        // This ensures each TorHandle owns exactly one proxy pair, which is
        // cleaned up (aborted) when the TorHandle is dropped.
        let (alice_port, alice_task) = launch_socks5_proxy(alice).await?;
        let (bob_port, bob_task) = launch_socks5_proxy(bob).await?;

        tracing::info!("Tor ready — two isolated circuits allocated");
        Ok(Self {
            coordinator_url,
            alice_proxy: format!("socks5h://127.0.0.1:{alice_port}"),
            bob_proxy: format!("socks5h://127.0.0.1:{bob_port}"),
            _alice_task: alice_task,
            _bob_task: bob_task,
        })
    }

    /// Returns the coordinator URL for the Alice (input registration) circuit.
    pub fn alice_url(&self) -> &str {
        &self.coordinator_url
    }

    /// Returns the coordinator URL for the Bob (output registration) circuit.
    pub fn bob_url(&self) -> &str {
        &self.coordinator_url
    }

    /// Returns `socks5h://127.0.0.1:<port>` for the Alice SOCKS5 proxy.
    /// The proxy was started during `TorHandle::new` — no additional await needed.
    pub fn alice_proxy_url(&self) -> &str {
        &self.alice_proxy
    }

    /// Returns `socks5h://127.0.0.1:<port>` for the Bob SOCKS5 proxy.
    pub fn bob_proxy_url(&self) -> &str {
        &self.bob_proxy
    }
}

impl Drop for TorHandle {
    fn drop(&mut self) {
        // Abort the listener tasks so that OS port allocations and TorClient handles
        // are released when this TorHandle goes out of scope.
        self._alice_task.abort();
        self._bob_task.abort();
    }
}

/// Convenience: bootstrap Tor and return a TorHandle.
pub async fn init_tor(coordinator_url: String) -> anyhow::Result<TorHandle> {
    TorHandle::new(coordinator_url).await
}

/// Bind a TCP listener on 127.0.0.1:0, spawn a SOCKS5 server task routing through
/// the given TorClient, and return the assigned port along with the task JoinHandle.
///
/// The caller must store the returned JoinHandle for the lifetime of the proxy.
/// Dropping the handle aborts the listener task and releases the OS port allocation.
///
/// The SOCKS5 implementation handles the subset required by reqwest:
/// - RFC 1928 no-auth greeting
/// - CONNECT command with hostname (0x03) and IPv4 (0x01) address types
async fn launch_socks5_proxy(
    tor: TorClient<PreferredRuntime>,
) -> anyhow::Result<(u16, tokio::task::JoinHandle<()>)> {
    // T-05-08: loopback-only, OS-assigned ephemeral port
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind SOCKS5 listener")?;
    let port = listener.local_addr()?.port();

    let handle = tokio::spawn(async move {
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

    Ok((port, handle))
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
    let version = buf[0];
    if version != 0x05 {
        anyhow::bail!("Unsupported SOCKS version: 0x{version:02x} (expected 0x05)");
    }
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
    let cmd = req_hdr[1];
    if cmd != 0x01 {
        // Send SOCKS5 error reply: command not supported (0x07)
        stream.write_all(&[0x05, 0x07, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
        anyhow::bail!("Unsupported SOCKS5 command: 0x{cmd:02x} (only CONNECT 0x01 is supported)");
    }
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
            // IPv6 — use plain address string without brackets so that arti's
            // IntoTorAddr parser accepts it. The bracketed URI form ([::1]) is
            // not guaranteed to be accepted by arti's host:port string parser.
            let mut ipv6 = [0u8; 16];
            stream.read_exact(&mut ipv6).await?;
            std::net::Ipv6Addr::from(ipv6).to_string()
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
