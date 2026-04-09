---
phase: 05-tor-release
reviewed: 2026-04-09T20:51:15Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - Cargo.toml
  - client/Cargo.toml
  - client/src/config.rs
  - client/src/http.rs
  - client/src/lib.rs
  - client/src/main.rs
  - client/src/tor.rs
  - coordinator/Cargo.toml
  - coordinator/src/config.rs
  - coordinator/src/lib.rs
  - coordinator/src/main.rs
  - coordinator/src/network/mod.rs
  - coordinator/src/network/tor.rs
  - docker/Dockerfile.client
  - .github/workflows/docker.yml
  - .github/workflows/release.yml
findings:
  critical: 2
  warning: 5
  info: 3
  total: 10
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-04-09T20:51:15Z
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

This phase adds the Tor transport layer: the coordinator can now run as a v3 onion hidden service (via arti), and the client uses two isolated Tor circuits (alice/bob) to prevent input-output linkage. The overall architecture is sound and the circuit isolation design is correct. However, there are two critical issues — one is an information disclosure bug that partially defeats the unlinkability guarantee, and one is a goroutine / resource leak in the SOCKS5 proxy. Five additional warnings cover reliability and correctness risks.

---

## Critical Issues

### CR-01: SOCKS5 proxy leak — listener orphaned when `TorHandle` is dropped

**File:** `client/src/tor.rs:96-125`

`launch_socks5_proxy` spawns a `tokio::spawn` loop that holds a `TcpListener` and a cloned `TorClient`. Neither the spawned task handle nor a shutdown signal is stored. The `TorHandle` struct has no `Drop` implementation. When `TorHandle` goes out of scope the two spawned SOCKS5 listener tasks continue running indefinitely, keeping two OS port allocations and two `TorClient` handles alive for the lifetime of the process. In a long-running client (e.g., participant retries a round) new proxy ports are created on each `init_tor` call while old ones are never cleaned up.

**Fix:** Return the `JoinHandle` and store it in `TorHandle`, or use a `CancellationToken` (tokio-util) to signal shutdown on drop:

```rust
pub struct TorHandle {
    alice: TorClient<PreferredRuntime>,
    bob: TorClient<PreferredRuntime>,
    coordinator_url: String,
    // store handles so they are cancelled/aborted on drop
    _alice_task: tokio::task::JoinHandle<()>,
    _bob_task: tokio::task::JoinHandle<()>,
}

// abort on drop
impl Drop for TorHandle {
    fn drop(&mut self) {
        self._alice_task.abort();
        self._bob_task.abort();
    }
}
```

Alternatively, call `launch_socks5_proxy` once each for alice and bob, store the handles at construction time in `TorHandle::new`, and expose the already-resolved port strings instead of spawning lazily in `alice_proxy_url` / `bob_proxy_url`.

---

### CR-02: `addr_tx.send()` return value silently discarded — onion address may never reach PKARR publisher

**File:** `coordinator/src/network/tor.rs:63`

```rust
let _ = addr_tx.send(onion_addr);
```

The `oneshot::Sender::send` return value is deliberately discarded. If the receiver (`addr_rx` in `main.rs`) has already been dropped (e.g., due to a timeout or a race during shutdown), the onion address is lost silently. In `main.rs` the code then calls `addr_rx.await` — if `send` failed because the channel was closed, `addr_rx.await` returns `Err(RecvError)` and the process surfaces a generic `"Onion service task exited before sending address"` error that obscures the real problem.

More critically: after the `send`, `serve_onion_service` enters the accept loop and starts serving connections. The onion service is live and accepting client connections, but PKARR has published nothing (because `main.rs` bailed on the error). Clients who resolve the DHT record from a previous run will be connecting to a coordinator that has not published an updated record for this session. The hidden service runs in a zombie state: serving connections but invisible to new clients.

**Fix:** Propagate the error instead of swallowing it, and stop the accept loop if the address could not be delivered:

```rust
addr_tx.send(onion_addr).map_err(|_| {
    anyhow::anyhow!("main task dropped the address receiver before onion address was delivered")
})?;
```

---

## Warnings

### WR-01: `poll_until_phase` has no timeout — client hangs indefinitely

**File:** `client/src/http.rs:105-113`

```rust
pub async fn poll_until_phase(&self, expected_phase: &str, interval_ms: u64) -> Result<InfoResponse> {
    loop {
        let info = self.get_info().await?;
        if info.round_state == expected_phase {
            return Ok(info);
        }
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
    }
}
```

If the coordinator crashes, the round never advances, or the wrong phase name is passed, this loop never terminates. There is no maximum wait time. The client process will hang forever with no user-visible indication of progress. This is called three times in `main.rs` (lines 78, 84, 90).

**Fix:** Add a deadline parameter or a default timeout (e.g., 10 minutes) and return an error when exceeded:

```rust
use tokio::time::{timeout, Duration};

pub async fn poll_until_phase(
    &self,
    expected_phase: &str,
    interval_ms: u64,
    max_wait: Duration,
) -> Result<InfoResponse> {
    timeout(max_wait, async {
        loop {
            let info = self.get_info().await?;
            if info.round_state == expected_phase {
                return Ok(info);
            }
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Timed out waiting for phase: {expected_phase}"))?
}
```

---

### WR-02: Signing timeout timer fires once and is never restarted — blame logic broken across rounds

**File:** `coordinator/src/main.rs:103-125`

The signing timeout `tokio::spawn` block fires once, waits `round_timeout_signing_secs`, and then fires the blame handler. It is not restarted when a new round begins. This means:

- Round 1: timeout fires correctly after `round_timeout_signing_secs`.
- Round 2 (and all subsequent rounds): no signing timeout fires. Non-signers in round 2+ are never banned.

The same issue applies to the output-reg timeout spawned at lines 132-144.

**Fix:** Move the timeout timers into the round state machine loop (the task that advances phases), or restart them as part of the phase transition logic when entering `Signing` / `OutputReg`. A common pattern is to use a `tokio::sync::watch` or `broadcast` channel to signal phase entry, and spawn a fresh timeout task on each entry.

---

### WR-03: SOCKS5 handshake does not validate SOCKS version byte or CMD byte

**File:** `client/src/tor.rs:136-153`

The SOCKS5 handshake reads `buf[0]` as version but never checks it equals `0x05`. Similarly, `req_hdr[1]` (the CMD byte) is read but never checked — the code proceeds as if it were always `CONNECT (0x01)`. A malformed or misdirected TCP connection will silently attempt to `tor.connect()` to a garbage target.

While this is an in-process loopback-only proxy so external attackers cannot reach it, the missing validation means programming errors (e.g., reqwest sending a different SOCKS version) produce confusing Tor connection errors instead of a clear protocol mismatch error.

**Fix:**

```rust
if buf[0] != 0x05 {
    anyhow::bail!("Unsupported SOCKS version: 0x{:02x}", buf[0]);
}
// ...
let cmd = req_hdr[1];
if cmd != 0x01 {
    // Send SOCKS5 error reply: command not supported (0x07)
    stream.write_all(&[0x05, 0x07, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).await?;
    anyhow::bail!("Unsupported SOCKS5 command: 0x{cmd:02x}");
}
```

---

### WR-04: IPv6 SOCKS5 target formatted with brackets but arti `connect()` may not accept bracketed form

**File:** `client/src/tor.rs:169-175`

When the SOCKS5 address type is `0x04` (IPv6), the code formats the address as `[{addr}]`:

```rust
format!("[{addr}]")
```

Then at line 185 this is joined with the port as `format!("{target_host}:{port}")`, producing `[::1]:8080`. The arti `TorClient::connect()` API takes a string in `host:port` or `hostname:port` form. Whether it accepts the bracketed IPv6 form (RFC 2732 URI notation) depends on arti's internal address parser. If arti expects a raw IPv6 address without brackets (e.g., `::1:8080`), all IPv6 connections will fail with a confusing parse error.

Coordinator .onion addresses are domain names (ATYP 0x03), so this code path is unlikely to be exercised in practice — but it is reachable and the behavior is untested.

**Fix:** Use the `std::net::SocketAddrV6` for the final format, which arti's `IntoTorAddr` blanket impl supports:

```rust
0x04 => {
    let mut ipv6 = [0u8; 16];
    stream.read_exact(&mut ipv6).await?;
    std::net::Ipv6Addr::from(ipv6).to_string()  // no brackets
}
// ...
// then connect using SocketAddr directly to avoid string parsing ambiguity
```

---

### WR-05: Docker workflow pushes images on every `main` branch push, including non-release commits

**File:** `.github/workflows/docker.yml:5-6`

```yaml
on:
  push:
    tags: ['v*']
    branches: [main]
```

Every commit to `main` triggers a Docker push with the `latest` tag. For an open-source infrastructure tool where users are expected to pull `latest` for the "zero to CoinJoin in five minutes" story, this means a broken or partially-completed commit can update the public `latest` image before tests pass. There is no `CI` / `test` step as a prerequisite in this workflow.

**Fix:** Either restrict pushes to tags only, or add a `needs: [ci]` dependency on a test job:

```yaml
on:
  push:
    tags: ['v*']
# Remove branches: [main] trigger, or add:
jobs:
  docker:
    needs: [test]  # reference a separate test job
```

---

## Info

### IN-01: `network` field in `client/src/config.rs` uses free-form `String` — late validation in `main.rs`

**File:** `client/src/config.rs:39-41`, `client/src/main.rs:23-28`

The `network` field is parsed as a plain `String` and validated with a `match` inside `main()`. An invalid value like `"testnet"` (vs the correct `"testnet4"`) is only discovered at runtime after all other initialization has run. Using a typed enum with `clap`'s `ValueEnum` derive gives a better error at argument-parse time.

**Fix:**

```rust
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum Network { Signet, Testnet4, Mainnet }

#[arg(long, value_enum, default_value_t = Network::Signet)]
pub network: Network,
```

---

### IN-02: `release.yml` installs `cross` from git HEAD on every run

**File:** `.github/workflows/release.yml:48`

```yaml
run: cargo install cross --git https://github.com/cross-rs/cross
```

Installing `cross` from the default branch of the upstream repo on every release build means a breaking change in `cross` can silently break arm64 release builds. It also adds several minutes to each release run.

**Fix:** Pin to a specific release tag:

```yaml
run: cargo install cross --git https://github.com/cross-rs/cross --tag v0.2.5
```

Or use a pre-built binary via the `cross-rs/cross` GitHub Action if available.

---

### IN-03: `Dockerfile.client` installs `curl` in the runtime image — unnecessary attack surface

**File:** `docker/Dockerfile.client:22-25`

```dockerfile
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
```

`curl` is not needed by the client binary at runtime. The Dockerfile comment does not explain why it is present. It adds ~2 MB and a package with historical CVEs to the minimal runtime image.

**Fix:** Remove `curl` unless it is explicitly used in an entrypoint health check or wrapper script. `ca-certificates` alone is sufficient for TLS.

---

_Reviewed: 2026-04-09T20:51:15Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
