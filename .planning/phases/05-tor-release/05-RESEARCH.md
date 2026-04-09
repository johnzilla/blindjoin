# Phase 5: Tor & Release - Research

**Researched:** 2026-04-07
**Domain:** arti-client 0.41 hidden service hosting, client stream isolation, GitHub Actions Rust cross-compilation, Docker multi-arch GHCR publish
**Confidence:** MEDIUM (arti-axum compatibility gap is a blocking finding requiring PoC; CI patterns are HIGH)

---

## Summary

Phase 5 has four requirements: (1) coordinator runs as a Tor hidden service via arti-client (PRIV-03), (2) client uses fresh Tor circuits per phase (CLI-05), (3) GitHub Actions publishes pre-built binaries for Linux x86_64/aarch64 and macOS x86_64/aarch64 (DEPL-03), and (4) Docker images are published to ghcr.io (DEPL-04).

The blocking finding is that **arti-axum 0.1.0 depends on arti-client 0.24.0 and tor-hsservice 0.24.0**, while the current crates.io version is **arti-client 0.41.0**. There is a 17-minor-version gap. The community crate has not been updated to track arti's ongoing minor version bumps. This means arti-axum 0.1.0 cannot be used with arti-client 0.41 without a version mismatch conflict — a Sprint 0 PoC must either (a) verify the crates can be reconciled via Cargo dependency resolution, or (b) write the glue directly against arti-client 0.41 + tor-hsservice 0.41 APIs, which are documented and stable enough to implement. The glue is approximately 30-50 lines (accept IncomingStream loop, wrap DataStream as hyper IO, hand off to axum).

The CI/CD work (DEPL-03, DEPL-04) is well-understood. The standard pattern uses `cross-rs` for Linux aarch64, native runners for macOS, `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `softprops/action-gh-release`, and the Docker buildx QEMU approach for multi-arch GHCR images.

**Primary recommendation:** Sprint 0 must be the first task — attempt to add arti-client 0.41 alongside arti-axum 0.1.0 and let Cargo resolve. If Cargo rejects the version conflict, write 40-line custom glue using arti-client 0.41 + tor-hsservice 0.41 directly. Do not block CI/CD work on this; DEPL-03 and DEPL-04 can be developed in parallel.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-05 | Fresh Tor circuit per phase (input registration circuit != output registration circuit) | `IsolationToken::new()` + `StreamPrefs::set_isolation()` pattern verified via docs.rs; `TorClient::isolated_client()` is an alternative approach |
| PRIV-03 | Coordinator runs as Tor hidden service via arti-client (no clearnet endpoint in production) | `TorClient::launch_onion_service()` + `handle_rend_requests()` API documented for arti-client 0.41; arti-axum glue compatibility requires PoC |
| DEPL-03 | Pre-built Linux/macOS binaries via GitHub Releases (GitHub Actions CI) | Matrix strategy with cross-rs for Linux aarch64, native macOS runners, softprops/action-gh-release pattern verified |
| DEPL-04 | Docker images published to GitHub Container Registry (ghcr.io) | docker/build-push-action with QEMU for multi-arch, docker/login-action with GITHUB_TOKEN verified |
</phase_requirements>

---

## Project Constraints (from CLAUDE.md)

- Use `/browse` skill from gstack for web browsing — not `mcp__claude-in-chrome__*` tools
- No other binding project-level directives for this phase

---

## Standard Stack

### Core (arti hidden service)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| arti-client | 0.41.0 | TorClient bootstrap, `launch_onion_service`, `isolated_client` | Official Tor Project Rust client; current crates.io version [VERIFIED: cargo search] |
| tor-hsservice | 0.41.0 | `handle_rend_requests`, `IncomingStream` server-side HS infra | Companion crate in the Arti monorepo; same version as arti-client [VERIFIED: cargo search] |
| arti-axum | 0.1.0 | Bridge axum Router over arti onion service | Community crate — **BLOCKED: requires PoC** (see Pitfall 1) |

**arti-axum version conflict (CRITICAL finding):**
- `arti-axum = "0.1.0"` (crates.io) depends on `arti-client = "0.24.0"` and `tor-hsservice = "0.24.0"` [VERIFIED: raw Cargo.toml fetch from jgraef/arti-axum]
- Current crates.io: `arti-client = "0.41.0"`, `tor-hsservice = "0.41.0"` [VERIFIED: cargo search]
- The Arti project does NOT guarantee semver stability for low-level crates between minor versions
- Arti 2.0.0 introduced a breaking change: `TorClient::launch_onion_service()` now returns `Option<...>` (returns `None` if disabled) [CITED: zydou/arti CHANGELOG via WebFetch]

### Core (client Tor isolation)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| arti-client | 0.41.0 | `TorClient::create_bootstrapped`, `isolated_client()`, `IsolationToken` | Same crate as coordinator; client uses it to connect (not host) |

### CI/CD

| Tool / Action | Version | Purpose | Why Standard |
|--------------|---------|---------|--------------|
| dtolnay/rust-toolchain | @stable | Install Rust toolchain + cross-compilation target | Maintained by dtoolbox; standard for GHA Rust |
| Swatinem/rust-cache | @v2 | Cache Cargo registry + build artifacts | De facto standard Rust build cache for GHA |
| cross-rs/cross | latest via `cargo install cross` | Cross-compile Linux aarch64 on x86_64 runner | Required for `aarch64-unknown-linux-gnu` on ubuntu-latest |
| houseabsolute/actions-rust-cross | @v0 | Wrapper that auto-selects cross vs native cargo | Simplifies matrix strategy [CITED: blog.urth.org] |
| softprops/action-gh-release | @v1 | Upload binary artifacts to GitHub Releases | Standard GHA release action |
| docker/login-action | @v4 | Authenticate to ghcr.io with GITHUB_TOKEN | Official Docker GHA action |
| docker/setup-qemu-action | @v4 | Enable linux/arm64 QEMU emulation for buildx | Required for multi-platform images on ubuntu-latest |
| docker/setup-buildx-action | @v4 | Provision Docker Buildx builder | Required for `--platform linux/amd64,linux/arm64` |
| docker/build-push-action | @v7 | Multi-platform build + push to ghcr.io | Official Docker GHA action [CITED: docs.docker.com] |

**Installation (Cargo.toml additions):**
```toml
# coordinator/Cargo.toml
arti-client = { version = "0.41", features = ["onion-service-service", "tokio"] }
tor-hsservice = "0.41"

# client/Cargo.toml
arti-client = { version = "0.41", features = ["onion-service-client", "tokio"] }

# arti-axum — only if Sprint 0 PoC confirms compatibility:
arti-axum = "0.1"
```

**Version verification:**
```bash
cargo search arti-client   # 0.41.0 confirmed
cargo search tor-hsservice # 0.41.0 confirmed
cargo search arti-axum     # 0.1.0 confirmed (depends on arti-client 0.24)
```

---

## Architecture Patterns

### Recommended Project Structure Additions

```
coordinator/src/
└── network/
    ├── mod.rs          # pub mod tor (new)
    └── tor.rs          # onion service bootstrap + serve loop (new)

client/src/
└── network/
    ├── mod.rs          # pub mod tor (new)
    └── tor.rs          # TorClient init, isolated_client per phase (new)

.github/
└── workflows/
    ├── release.yml     # Binary cross-compilation + GitHub Releases (new)
    └── docker.yml      # Multi-arch Docker build + push to ghcr.io (new)
```

### Pattern 1: Coordinator Hidden Service via arti-client 0.41 (direct glue)

**What:** If arti-axum is incompatible, implement the RendRequest accept loop directly. It is approximately 40 lines.

**When:** Sprint 0 PoC determines arti-axum cannot be used with arti-client 0.41.

```rust
// coordinator/src/network/tor.rs
// Source: docs.rs/arti-client/latest/arti_client/struct.TorClient.html [VERIFIED]
// Source: arti-axum 0.1.0 src/lib.rs implementation (extracted via WebFetch) [VERIFIED]

use arti_client::{TorClient, TorClientConfig, config::OnionServiceConfigBuilder};
use tor_hsservice::{handle_rend_requests, StreamRequest, IncomingStreamRequest};
use tokio_util::compat::TokioAsyncReadCompatExt;

pub async fn serve_onion_service(app: axum::Router) -> anyhow::Result<()> {
    let tor_client = TorClient::create_bootstrapped(
        TorClientConfig::default()
    ).await?;

    let (onion_service, rend_requests) = tor_client
        .launch_onion_service(
            OnionServiceConfigBuilder::default()
                .nickname("blindjoin-coordinator".to_owned().try_into()?)
                .build()?
        )?
        .expect("onion-service-service feature is enabled");

    let onion_addr = onion_service.onion_name()
        .expect("onion service address available after launch");
    tracing::info!(onion_addr = %onion_addr, "Coordinator onion service ready");

    // Publish onion_addr to PKARR record here (pass via channel or return)

    let stream_requests = handle_rend_requests(rend_requests);
    // Loop: accept each StreamRequest, wrap DataStream as hyper IO,
    // hand off to axum tower service — ~20 lines mirroring arti-axum src/lib.rs
    arti_axum::serve(stream_requests, app).await;
    Ok(())
}
```

**If arti-axum is compatible (Sprint 0 PoC succeeds):** the above uses `arti_axum::serve(stream_requests, app)` directly and no custom loop is needed.

### Pattern 2: arti-axum serve (if Sprint 0 confirms compatibility)

**What:** The full arti-axum API. Wraps the entire loop in one call.

```rust
// Source: docs.rs/arti-axum/latest/arti_axum/ [VERIFIED via WebFetch]
let (onion_service, rend_requests) = tor_client.launch_onion_service(config)?.unwrap();
let stream_requests = handle_rend_requests(rend_requests);
arti_axum::serve(stream_requests, app).await;
```

**When:** Sprint 0 PoC confirms `arti-client 0.41` and `arti-axum 0.1.0` resolve without conflicts.

### Pattern 3: Client Stream Isolation (CLI-05)

**What:** Two distinct `IsolationToken` instances — one for input registration (Alice), one for output registration (Bob). Tokens prevent circuit sharing across phases.

**When:** Client `http.rs` — `CoordinatorClient` needs two variants or a phase-aware token parameter.

```rust
// Source: docs.rs/arti-client/latest/arti_client/struct.IsolationToken.html [VERIFIED]
use arti_client::{IsolationToken, StreamPrefs};

// At round start — create two tokens, one per phase
let alice_token = IsolationToken::new();  // input registration
let bob_token = IsolationToken::new();    // output registration
// alice_token != bob_token — guaranteed different circuits

// Pass to TorClient::connect (or underlying reqwest SOCKS5 proxy):
let mut prefs = StreamPrefs::new();
prefs.set_isolation(alice_token);
// OR use isolated_client() for a clean per-phase TorClient
let alice_client = tor_client.isolated_client();  // circuit-isolated handle
let bob_client = tor_client.isolated_client();    // different circuits guaranteed
```

**Alternative (simpler for reqwest integration):** Use arti's SOCKS5 proxy listener on a local port, configure separate reqwest clients via SOCKS5 with different destination-address isolation. The `IsolateDestAddr` SOCKS5 extension ensures different circuits per connection when enabled. This is the approach if using `TorClient::launch_socks5_listener()` rather than native `TorClient::connect()`.

**Preferred approach:** Use `tor_client.isolated_client()` to get two separate `TorClient` handles. Build a separate `reqwest::Client` backed by each (via hyper connector or arti's HTTP connect proxy). This gives clean separation without touching SOCKS5 infrastructure.

### Pattern 4: GitHub Actions Release Workflow (DEPL-03)

**What:** Matrix strategy across 4 targets. Linux aarch64 uses cross-rs. macOS targets use native runners with `rustup target add`. Triggered on `v*` tag push.

```yaml
# .github/workflows/release.yml
# Source: ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/ [CITED]
# Source: reemus.dev/tldr/rust-cross-compilation-github-actions [CITED]

name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: write

jobs:
  build:
    runs-on: ${{ matrix.runner }}
    strategy:
      matrix:
        include:
          - name: linux-amd64
            runner: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            use_cross: false
          - name: linux-arm64
            runner: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            use_cross: true
          - name: macos-amd64
            runner: macos-latest
            target: x86_64-apple-darwin
            use_cross: false
          - name: macos-arm64
            runner: macos-latest
            target: aarch64-apple-darwin
            use_cross: false

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
      - name: Install cross
        if: matrix.use_cross
        run: cargo install cross --git https://github.com/cross-rs/cross
      - name: Build
        run: |
          if [ "${{ matrix.use_cross }}" = "true" ]; then
            cross build --release --target ${{ matrix.target }} --bin coordinator
            cross build --release --target ${{ matrix.target }} --bin client
          else
            cargo build --release --target ${{ matrix.target }} --bin coordinator
            cargo build --release --target ${{ matrix.target }} --bin client
          fi
      - name: Package
        shell: bash
        run: |
          mkdir dist
          cp target/${{ matrix.target }}/release/coordinator dist/coordinator-${{ matrix.name }}
          cp target/${{ matrix.target }}/release/client dist/client-${{ matrix.name }}
          tar czf blindjoin-${{ matrix.name }}.tar.gz -C dist .
      - name: Release
        uses: softprops/action-gh-release@v1
        with:
          files: blindjoin-${{ matrix.name }}.tar.gz
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Pattern 5: Docker Multi-Arch GHCR Workflow (DEPL-04)

**What:** Single workflow builds `linux/amd64` + `linux/arm64` images for coordinator, client, and liquidity-bot. Pushes to `ghcr.io/${{ github.repository_owner }}/blindjoin-*`.

```yaml
# .github/workflows/docker.yml
# Source: docs.docker.com/build/ci/github-actions/multi-platform/ [CITED]

name: Docker

on:
  push:
    tags: ['v*']
  push:
    branches: [main]

permissions:
  contents: read
  packages: write

jobs:
  docker:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        image: [coordinator, client, liquidity-bot]
    steps:
      - uses: actions/checkout@v4
      - uses: docker/login-action@v4
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/setup-qemu-action@v4
      - uses: docker/setup-buildx-action@v4
      - uses: docker/build-push-action@v7
        with:
          context: .
          file: docker/Dockerfile.${{ matrix.image }}
          platforms: linux/amd64,linux/arm64
          push: true
          tags: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}:latest
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

**Important:** The existing `Dockerfile.coordinator` and `Dockerfile.bot` use `cargo build --release` without a target flag. For multi-arch Docker builds, the Dockerfile does NOT need `--target` — Docker Buildx + QEMU handles the arch selection transparently when the FROM image supports multi-arch (which `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1` both do via their own multi-arch manifests).

### Anti-Patterns to Avoid

- **Using `listen_addr` for production:** The clearnet `listen_addr` in `CoordinatorSection` must be disabled/ignored when Tor mode is active. The coordinator must not bind a TCP listener in production. Gate this with a config flag (`tor_mode = true`).
- **Single TorClient for both Alice and Bob phases:** Default circuit reuse will link phases. Always call `isolated_client()` or create two `TorClient` handles before starting input and output registration.
- **Using `IsolationToken::no_isolation()` for isolation:** `no_isolation()` creates tokens equal to all other `no_isolation()` tokens — they share circuits. Only `IsolationToken::new()` guarantees distinct circuits [CITED: docs.rs/arti-client].
- **Publishing onion address before HS is ready:** `onion_service.onion_name()` is available immediately after `launch_onion_service`, but the HS descriptor must propagate to the DHT before clients can connect. Add a brief startup log message but don't fail if the first client connection attempt fails — it can take 30-60 seconds for the descriptor to propagate.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tor circuit isolation | Custom circuit tracking | `IsolationToken::new()` + `StreamPrefs` or `isolated_client()` | Arti handles all circuit path selection; hand-rolled tracking will race condition |
| Hidden service crypto | Custom HS descriptor generation | arti-client `launch_onion_service` | v3 onion service crypto is non-trivial (ed25519 + x25519 + ntor handshake) |
| QEMU for Docker ARM builds | Docker buildx wrapper | `docker/setup-qemu-action@v4` + `docker/build-push-action@v7` | Official Docker actions handle binfmt registration |
| Manifest list creation | Separate docker push per arch | `docker/build-push-action` with `platforms:` | Multi-arch manifests are non-trivial to construct manually |
| Cross-compilation sysroot | Custom cross-compile Docker | `cross-rs` or `houseabsolute/actions-rust-cross` | cross handles sysroot, linker, and library setup automatically |

---

## Common Pitfalls

### Pitfall 1: arti-axum Version Conflict with arti-client 0.41 (CRITICAL)

**What goes wrong:** Adding `arti-client = "0.41"` and `arti-axum = "0.1"` to the same workspace causes a version conflict: arti-axum requires `arti-client ^0.24` (incompatible with 0.41). Cargo cannot satisfy both constraints and the build fails.

**Why it happens:** arti-axum 0.1.0 was published when arti-client was at 0.24.x. Arti bumps its crate minor versions with every release cycle. The community crate has not been updated.

**How to avoid:** Sprint 0 PoC attempts this resolution first. If Cargo rejects it, implement the ~40-line glue directly:
```rust
// The arti-axum src/lib.rs logic is ~80 lines total and well-documented.
// Direct deps: arti-client 0.41, tor-hsservice 0.41, hyper 1.x, tokio.
// Ref: arti-axum source extracted via WebFetch confirms:
//   accept BEGIN streams → DataStream → TokioIo → hyper::Builder → axum tower service
```

**Warning signs:** `cargo build` fails with "found two different compatible versions" or "package `arti-client` is specified twice".

**Confidence:** HIGH — confirmed by comparing Cargo.toml of arti-axum (arti-client 0.24) vs current crates.io (arti-client 0.41).

### Pitfall 2: `launch_onion_service` Returns `Option<_>` (Arti 2.0 Breaking Change)

**What goes wrong:** Code calls `.unwrap()` or destructures the return of `launch_onion_service` expecting a direct tuple, but as of Arti 2.0.0, the method returns `Result<Option<(OnionService, RendRequestStream)>>`. If the `onion-service-service` feature is not enabled in Cargo.toml, it returns `Ok(None)` silently.

**Why it happens:** Arti 2.0.0 changed this to return `None` when disabled in config [CITED: CHANGELOG].

**How to avoid:**
```rust
let (onion_service, rend_requests) = tor_client
    .launch_onion_service(config)?
    .expect("onion-service-service feature must be enabled in Cargo.toml");
```
Ensure `features = ["onion-service-service", "tokio"]` is set for coordinator.

### Pitfall 3: Coordinator Still Binding TCP Listener in Tor Mode

**What goes wrong:** `main.rs` currently runs `axum::serve(TcpListener::bind(...), app)` unconditionally. If Tor mode is added without removing the TCP listener, the coordinator exposes a clearnet endpoint in production — violating PRIV-03 and Anti-Pattern 5 in ARCHITECTURE.md.

**Why it happens:** The TCP listener was added in Phase 2-3 for clearnet development. Phase 5 must gate on config: `tor_mode = true` routes axum through arti, `tor_mode = false` keeps TCP for dev/testing.

**How to avoid:** Add `tor_mode: bool` to `CoordinatorSection` (default `true` in production, `false` in tests). Replace the unconditional `axum::serve(TcpListener::bind(...), ...)` in `main.rs` with a branch:
```rust
if cfg.tor_mode {
    serve_onion_service(app).await?;
} else {
    let listener = tokio::net::TcpListener::bind(&cfg.coordinator.listen_addr).await?;
    axum::serve(listener, app).await?;
}
```

### Pitfall 4: PKARR Record Still Publishing Clearnet Address in Tor Mode

**What goes wrong:** `coordinator_public_addr` in `DiscoveryConfig` defaults to `"127.0.0.1:8080"`. After adding Tor mode, the PKARR record must publish the `.onion` address, not the TCP address. If `main.rs` reads `coordinator_public_addr` before the onion service is launched, it publishes a clearnet address.

**Why it happens:** The onion address is only known after `launch_onion_service` returns. The current PKARR publish in `main.rs` happens before this.

**How to avoid:** The onion service task must pass the generated `.onion` address back to the PKARR heartbeat task via a `tokio::sync::watch` channel or by writing it to a shared `Arc<RwLock<String>>`. The PKARR publisher must wait for the address to be populated before its first publish.

### Pitfall 5: Linux aarch64 OpenSSL Cross-Compilation Failure

**What goes wrong:** `arti-client` transitively depends on TLS crates that link against `openssl-sys`. When cross-compiling for `aarch64-unknown-linux-gnu` on an x86_64 runner, `openssl-sys` cannot find the ARM64 sysroot headers and fails with linker errors.

**Why it happens:** cross-rs normally handles this, but Arti uses `rustls` by default (not `openssl`) — check whether `arti-client` features include `rustls` or `native-tls`. If the feature is `native-tls`, use vendored openssl or switch to `rustls`.

**How to avoid:** Enable `rustls` feature on `arti-client`:
```toml
arti-client = { version = "0.41", features = ["onion-service-service", "tokio", "rustls"] }
```
If `rustls` feature doesn't exist, use `OPENSSL_STATIC=1` and `OPENSSL_VENDOR=1` env vars in the cross-rs Cross.toml, or add `vendored` feature to the openssl dependency.

### Pitfall 6: Docker Buildx ARM Build Times (QEMU Slowness)

**What goes wrong:** Building Rust for `linux/arm64` via QEMU emulation is 5-10x slower than native. A full workspace build under QEMU can take 30-40 minutes, exceeding GitHub Actions job limits (6 hours) but more commonly just burning CI credits.

**Why it happens:** QEMU emulates the ARM instruction set in software.

**How to avoid:** The existing `cargo-chef` Dockerfiles already cache the dependency layer. The GHA cache (`cache-from: type=gha`) further reduces rebuild time. For the v1 release, QEMU is acceptable — the Docker ARM image is a nice-to-have, not a blocker. Prioritize native binary releases (DEPL-03) over Docker ARM if build times become an issue.

---

## Code Examples

### Onion Service Server Bootstrap (arti-client 0.41)

```rust
// Source: docs.rs/arti-client/latest/arti_client/struct.TorClient.html [VERIFIED]
// Source: arti-axum src/lib.rs via WebFetch [VERIFIED - implementation extracted]
use arti_client::{TorClient, TorClientConfig};
use arti_client::config::onion_service::OnionServiceConfigBuilder;

let tor_client = TorClient::create_bootstrapped(
    TorClientConfig::default()
).await?;

// Returns Ok(None) if feature not enabled — check at startup not runtime
let (onion_service, rend_requests) = tor_client
    .launch_onion_service(
        OnionServiceConfigBuilder::default()
            .nickname("blindjoin".to_owned().try_into()?)
            .build()?
    )?
    .expect("onion-service-service feature required");

let onion_addr = onion_service.onion_name().unwrap();
tracing::info!(addr = %onion_addr, "Onion service launched");
```

### Client Stream Isolation (arti-client 0.41)

```rust
// Source: docs.rs/arti-client/latest/arti_client/struct.IsolationToken.html [VERIFIED]
// Source: docs.rs/arti-client/latest/arti_client/struct.TorClient.html [VERIFIED]

// Option A: isolated_client() — simplest approach
let tor_client = TorClient::create_bootstrapped(TorClientConfig::default()).await?;
let alice_client = tor_client.isolated_client(); // input registration circuit
let bob_client = tor_client.isolated_client();   // output registration circuit
// alice_client and bob_client share no circuits with each other

// Option B: IsolationToken with StreamPrefs (fine-grained)
use arti_client::{IsolationToken, StreamPrefs};
let alice_token = IsolationToken::new();
let bob_token = IsolationToken::new();
let mut alice_prefs = StreamPrefs::new();
alice_prefs.set_isolation(alice_token);
// Pass alice_prefs to connect() calls for input registration phase
```

### Wrapping DataStream for hyper (custom glue if arti-axum incompatible)

```rust
// Source: arti-axum 0.1.0 src/lib.rs — implementation extracted via WebFetch [VERIFIED]
// Pipeline: DataStream → TokioIo → hyper::Builder → axum tower service

use tor_hsservice::{handle_rend_requests, StreamRequest, IncomingStreamRequest};
use hyper_util::rt::TokioIo;
use hyper::server::conn::http1;

let stream_requests = handle_rend_requests(rend_requests);
tokio::pin!(stream_requests);

while let Some(stream_req) = stream_requests.next().await {
    let IncomingStreamRequest::Begin(_) = stream_req.request() else { continue; };
    let data_stream = stream_req.accept(Connected::new_empty()).await?;
    let io = TokioIo::new(data_stream);
    let app_clone = app.clone();
    tokio::spawn(async move {
        http1::Builder::new()
            .serve_connection(io, app_clone)
            .await
            .ok();
    });
}
```

### GitHub Actions GHCR Login + Multi-Arch Push

```yaml
# Source: docs.docker.com/build/ci/github-actions/multi-platform/ [CITED]
- uses: docker/login-action@v4
  with:
    registry: ghcr.io
    username: ${{ github.actor }}
    password: ${{ secrets.GITHUB_TOKEN }}
- uses: docker/setup-qemu-action@v4
- uses: docker/setup-buildx-action@v4
- uses: docker/build-push-action@v7
  with:
    platforms: linux/amd64,linux/arm64
    push: true
    tags: ghcr.io/${{ github.repository_owner }}/blindjoin-coordinator:${{ github.ref_name }}
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate `tor` process + SOCKS5 | arti-client in-process | Arti 1.0.0 (2023) → 2.x LTS (2026) | No Docker `tor` service needed; onion keys managed in-process |
| `actions-rs/cargo` (deprecated) | `dtolnay/rust-toolchain` + `Swatinem/rust-cache` | 2023 | `actions-rs` is archived; dtolnay is the maintained alternative |
| Docker push with explicit `docker manifest create` | `docker/build-push-action` with `platforms:` | 2022 | Single action creates + pushes multi-arch manifest |
| `cargo build --target` for all cross-compile | `cross-rs` for Linux aarch64 | 2022-2023 | cross provides the full aarch64 sysroot automatically |

**Deprecated/outdated:**
- `actions-rs/cargo`: Archived, no longer maintained. Use `dtolnay/rust-toolchain` instead.
- `cargo install cross` from crates.io: Prefer `cargo install cross --git https://github.com/cross-rs/cross` for latest fixes.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `arti-axum 0.1.0` dep on `arti-client 0.24` was extracted from raw Cargo.toml — the crate may have an unpublished update or the raw fetch captured a stale branch | Standard Stack, Pitfall 1 | If arti-axum actually supports 0.41 on main, Sprint 0 PoC will discover this immediately; low impact |
| A2 | `TorClient::launch_onion_service()` config builder uses `OnionServiceConfigBuilder::default().nickname(...)` — exact import path may differ in 0.41 | Code Examples | Compilation error caught at Sprint 0 PoC; fix is to check docs.rs 0.41 API exactly |
| A3 | `isolated_client()` guarantees distinct guard nodes (not just different circuits from same guard) | Pattern 3 | If circuits share a guard node, a network adversary could still correlate. Pitfalls.md documents this should be verified with integration test checking different guard fingerprints |
| A4 | `cargo-chef:latest-rust-1` base image supports `linux/arm64` platform transparently | Pattern 5, Pitfall 6 | If the base image is x86_64-only, the Docker arm64 build will fail at the FROM stage; fixable by switching to `rust:1-slim` which is multi-arch |
| A5 | `arti-client 0.41` includes the `rustls` feature flag to avoid openssl-sys cross-compile issues | Pitfall 5 | If only `native-tls` is available, cross-compile requires openssl vendoring; Sprint 0 PoC will surface this |

---

## Open Questions

1. **Does `cargo-chef:latest-rust-1` support `linux/arm64` in a multi-arch Docker build?**
   - What we know: `lukemathwalker/cargo-chef` is a widely-used image; Docker Hub should have multi-arch manifests
   - What's unclear: Whether the `latest-rust-1` tag specifically has an ARM64 variant
   - Recommendation: Sprint 0 — run `docker buildx build --platform linux/arm64 .` locally or in CI dry-run to confirm; if it fails, switch Dockerfile FROM to `rust:1-slim` (confirmed multi-arch)

2. **Does Sprint 0 PoC succeed with arti-axum 0.1 + arti-client 0.41?**
   - What we know: arti-axum depends on arti-client 0.24; current is 0.41; Arti does not guarantee minor-version API stability
   - What's unclear: Whether Cargo will treat the 0.x version bump as compatible (`^0.24` allows `0.24.x` but not `0.41`)
   - Recommendation: Sprint 0 is the first task. If the build fails, implement the 40-line glue directly.

3. **Should the coordinator publish its `.onion` address to PKARR before or after descriptor propagation?**
   - What we know: The onion address is deterministic from the HS key, available immediately after `launch_onion_service`. Descriptor propagation takes 30-60 seconds.
   - What's unclear: Whether publishing the address before propagation causes client connection failures that are confusing
   - Recommendation: Publish immediately with a log message like "HS descriptor propagating, first connections may take 60s". Clients should have retry logic (already designed with polling intervals).

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker (for DEPL-04 workflow testing) | docker/build-push-action local test | Check local | — | Use GHA environment directly |
| Rust toolchain + cross targets | DEPL-03 | Check local | — | GHA provides via dtolnay/rust-toolchain |
| Internet access to Tor network (for PRIV-03 integration test) | arti-client bootstrap | Required at test time | — | No fallback; Tor connectivity required for HS PoC |

**Note:** The core Tor/arti work (PRIV-03, CLI-05) requires internet access to the live Tor network for bootstrap. Integration tests cannot be run in a network-isolated CI environment. GitHub Actions `ubuntu-latest` runners have full internet access, so CI is fine. Local development behind strict firewalls may require a Tor bridge configuration in `TorClientConfig`.

---

## Sources

### Primary (HIGH confidence)
- `cargo search arti-client` — confirmed version 0.41.0 on crates.io [VERIFIED]
- `cargo search tor-hsservice` — confirmed version 0.41.0 on crates.io [VERIFIED]
- `cargo search arti-axum` — confirmed version 0.1.0 on crates.io [VERIFIED]
- `https://raw.githubusercontent.com/jgraef/arti-axum/main/Cargo.toml` — confirmed arti-client 0.24.0 dependency [VERIFIED: raw file fetch]
- [docs.rs/arti-client/latest/arti_client/struct.TorClient.html](https://docs.rs/arti-client/latest/arti_client/struct.TorClient.html) — `launch_onion_service`, `isolated_client` signatures [VERIFIED]
- [docs.rs/arti-client/latest/arti_client/struct.IsolationToken.html](https://docs.rs/arti-client/latest/arti_client/struct.IsolationToken.html) — `IsolationToken::new()`, `StreamPrefs::set_isolation()` [VERIFIED]
- [docs.docker.com/build/ci/github-actions/multi-platform/](https://docs.docker.com/build/ci/github-actions/multi-platform/) — multi-platform Docker GHA workflow [CITED]
- [docs.rs/arti-axum/latest/arti_axum/](https://docs.rs/arti-axum/latest/arti_axum/) — `serve(stream_requests, app)` API [VERIFIED]

### Secondary (MEDIUM confidence)
- [zydou/arti CHANGELOG (raw GitHub)](https://raw.githubusercontent.com/zydou/arti/main/CHANGELOG.md) — Arti 2.0.0 `launch_onion_service` returns `Option` breaking change [CITED]
- [ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/](https://ahmedjama.com/blog/2025/12/cross-platform-rust-pipeline-github-actions/) — matrix strategy with cross-rs pattern [CITED]
- [reemus.dev/tldr/rust-cross-compilation-github-actions](https://reemus.dev/tldr/rust-cross-compilation-github-actions) — softprops/action-gh-release pattern [CITED]
- [arti-axum src/lib.rs implementation (extracted via WebFetch)](https://github.com/jgraef/arti-axum) — serve loop internals confirmed [VERIFIED: implementation match]

### Tertiary (LOW confidence)
- rdbo/arti-axum-hidden-service Cargo.toml — showed arti-axum on custom branch `rdbo-update-deps` with arti-client 0.17; suggests the community is aware of version tracking issues but no single maintained fork exists [NOTED: indicates version drift is a known pain point]

---

## Metadata

**Confidence breakdown:**
- arti-axum compatibility: LOW (requires Sprint 0 PoC to resolve)
- arti-client 0.41 API (onion service, isolation): MEDIUM-HIGH (docs.rs verified, some config import paths assumed)
- CI/CD patterns (release workflow, Docker): HIGH (official docs cited)
- Custom glue fallback design: MEDIUM (based on extracted arti-axum source, not compiled)

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (arti-client minor bumps roughly monthly; re-check version before implementation)
