# Phase 4: Discovery & Deployment - Research

**Researched:** 2026-04-07
**Domain:** PKARR DHT publishing/resolution, Docker Compose orchestration, Rust multi-stage builds, liquidity bot design
**Confidence:** MEDIUM-HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | Discover coordinator via direct .onion address or PKARR DHT lookup | pkarr 5.0.4 Client + resolve API; PublicKey from z-base32 string; direct URL fallback |
| DISC-01 | Coordinator publishes PKARR record to DHT with .onion, round params, RSA pubkey hash, status, uptime | SignedPacket::builder().txt() API; Keypair::from_secret_key_file(); publish() signature |
| DISC-02 | Client discovers coordinators via PKARR DHT lookup or direct .onion address | Client::resolve(); PublicKey TryFrom &str; resource_records() iteration |
| DISC-03 | Coordinator heartbeat: re-publish PKARR record every 5 minutes and on state transitions | tokio::time::interval loop; state-change hook in round state manager |
| DEPL-01 | Docker Compose stack: bitcoind (signet) + coordinator + liquidity bot, zero to CoinJoin in 5 minutes | depends_on/service_healthy; bitcoind RPC healthcheck; cargo-chef multi-stage build |
| DEPL-02 | Liquidity bot: auto-joins rounds on signet for testing and cold-start | client lib reuse; polling GET /info; strategy module; separate binary crate |
</phase_requirements>

---

## Summary

Phase 4 adds coordinator discoverability and the deployment stack. The three work streams are largely independent and can be parallelized: (1) PKARR publisher in the coordinator and resolver in the client CLI, (2) a liquidity bot binary that wraps the existing client library, and (3) a Docker Compose stack that wires everything together with proper startup ordering.

**pkarr API surface:** The pkarr crate is at version 5.0.4 (March 2026), a major version jump from the "2.x" referenced in the stack research. The API is stable: `Client::builder().build()`, `publish(&signed_packet, None)`, and `resolve(&public_key)` are the three core calls. DNS TXT records are built via `SignedPacket::builder().txt(label, value, ttl).sign(&keypair)`. The Ed25519 keypair must be persisted across restarts (so the coordinator's public key stays stable) using `Keypair::write_secret_key_file()` and `Keypair::from_secret_key_file()`.

**Liquidity bot:** Structurally identical to the client CLI. It should live as a separate binary in the workspace (`liquidity-bot/src/main.rs`) that depends on the `client` library crate. It polls `GET /info`, detects InputReg phase, and fires a full join attempt. A `strategy.rs` module decides join criteria (denomination match, min participants threshold, backoff when rounds fail repeatedly).

**Docker Compose:** The pattern is: bitcoind with a `healthcheck` using `bitcoin-cli -signet getblockchaininfo`, coordinator with `depends_on: bitcoind: condition: service_healthy`, and liquidity-bot with `depends_on: coordinator: condition: service_healthy`. Multi-stage Rust builds use `cargo-chef` for layer caching so iterative rebuilds are fast. The spec's current `docker-compose.yml` uses a separate `goldy/tor-hidden-service` container — Phase 4 does not include Tor (that is Phase 5), so the compose stack targets clearnet for now with a `tor` service placeholder that can be activated later.

**Primary recommendation:** Add `pkarr = "5"` to workspace dependencies. Add a `discovery/` module to the coordinator. Add a `discover.rs` to the client. Create `liquidity-bot/` as a new workspace member. Write `docker/docker-compose.yml` with healthcheck-gated startup ordering.

---

## Standard Stack

### Core (new additions for Phase 4)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pkarr | 5.0.4 | Publish and resolve signed DNS packets over Mainline DHT | Official Pubky/Synonymous crate; only Rust implementation of PKARR |

**Version verification:**
```bash
# Verified against crates.io API 2026-04-07
# latest: 5.0.4   updated: 2026-03-25
```

The workspace `STACK.md` referenced `pkarr = "2"`. The actual current version is **5.0.4**. Update the workspace Cargo.toml. [VERIFIED: crates.io API]

### Existing Stack (no changes needed)

All other dependencies (tokio, axum, reqwest, serde, clap, tracing, config) are already in `Cargo.toml` and serve Phase 4 needs unchanged.

### New Workspace Member

The `liquidity-bot` crate does not yet exist. It must be added to `[workspace] members` in the root `Cargo.toml`.

**Installation additions:**
```toml
# workspace Cargo.toml — add:
pkarr = { version = "5", default-features = true }

# liquidity-bot/Cargo.toml — new crate depending on:
client = { path = "../client" }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
clap = { workspace = true }
anyhow = "1"
```

---

## Architecture Patterns

### Recommended Project Structure (additions only)

```
coordinator/src/
└── discovery/
    ├── mod.rs           # pub mod pkarr_pub
    └── pkarr_pub.rs     # PkarrPublisher struct, heartbeat task, publish_record()

client/src/
└── discover.rs          # discover_coordinator() → CoordinatorInfo

liquidity-bot/           # NEW workspace member
├── Cargo.toml
└── src/
    ├── main.rs          # CLI entry, polling loop
    └── strategy.rs      # JoinStrategy: when to join, backoff, denomination filter

docker/
├── Dockerfile.coordinator   # cargo-chef multi-stage
├── Dockerfile.bot           # cargo-chef multi-stage
└── docker-compose.yml       # bitcoind + coordinator + liquidity-bot
```

### Pattern 1: PKARR Publisher (coordinator side)

**What:** A long-running Tokio task that publishes the coordinator's DNS record on startup and then re-publishes on a 5-minute interval plus whenever round state changes.

**When to use:** Coordinator startup in `main.rs`.

**Example:**
```rust
// Source: docs.rs/pkarr 5.0.4 [VERIFIED]
use pkarr::{Client, Keypair, SignedPacket};

pub struct PkarrPublisher {
    client: Client,
    keypair: Keypair,
}

impl PkarrPublisher {
    pub fn new(keypair: Keypair) -> anyhow::Result<Self> {
        let client = Client::builder().build()?;
        Ok(Self { client, keypair })
    }

    pub async fn publish_record(&self, packet: SignedPacket) -> anyhow::Result<()> {
        self.client.publish(&packet, None).await
            .map_err(|e| anyhow::anyhow!("PKARR publish failed: {e}"))
    }
}

// Build the DNS packet with coordinator metadata
pub fn build_coordinator_packet(
    keypair: &Keypair,
    onion: &str,
    denomination_sats: u64,
    min_participants: u32,
    status: &str,
) -> anyhow::Result<SignedPacket> {
    let ttl = 300u32; // 5 minutes — matches heartbeat interval
    SignedPacket::builder()
        .txt("_coordinator".try_into()?, format!("onion={onion}").try_into()?, ttl)
        .txt("_coordinator".try_into()?, format!("denomination={denomination_sats}").try_into()?, ttl)
        .txt("_coordinator".try_into()?, format!("min_participants={min_participants}").try_into()?, ttl)
        .txt("_coordinator".try_into()?, format!("status={status}").try_into()?, ttl)
        .txt("_coordinator".try_into()?, "version=1".try_into()?, ttl)
        .sign(keypair)
        .map_err(|e| anyhow::anyhow!("SignedPacket build failed: {e}"))
}
```

**Heartbeat task:**
```rust
// Spawn in main.rs alongside other timer tasks
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        let round = round_state.read().await;
        let status = round.phase.as_str(); // "idle" | "input_reg" | etc.
        drop(round);
        if let Ok(packet) = build_coordinator_packet(&keypair, &onion, denom, min_p, status) {
            let _ = publisher.publish_record(packet).await;
        }
    }
});
```

**State-transition publishing:** Hook publish calls into `round::manager` after each phase transition. The simplest approach is a `tokio::sync::watch::Sender<Phase>` that the heartbeat task reads, triggering an immediate re-publish when the value changes. Alternatively, call publish directly from the phase transition handler — acceptable since publish is async and fast on relay path.

### Pattern 2: Keypair Persistence

**What:** The coordinator's Ed25519 keypair must survive restarts. The public key IS the coordinator's stable identity (clients look it up by public key). If the keypair rotates, the coordinator becomes undiscoverable to clients who bookmarked the old key.

**When to use:** Coordinator startup.

```rust
// Source: docs.rs/pkarr 5.0.4 Keypair::from_secret_key_file [VERIFIED]
const PKARR_KEY_FILE: &str = "coordinator_pkarr.key";

pub fn load_or_generate_keypair(path: &str) -> anyhow::Result<Keypair> {
    match Keypair::from_secret_key_file(path) {
        Ok(kp) => {
            tracing::info!("Loaded existing PKARR keypair from {path}");
            Ok(kp)
        }
        Err(_) => {
            let kp = Keypair::random();
            kp.write_secret_key_file(path)
                .map_err(|e| anyhow::anyhow!("Cannot write PKARR key file: {e}"))?;
            tracing::info!(
                pubkey = %kp.public_key().to_z32(),
                "Generated new PKARR keypair, saved to {path}"
            );
            Ok(kp)
        }
    }
}
```

The key file path should be configurable (add `pkarr_key_file` to `CoordinatorConfig`). In Docker, mount a named volume at the key file path so the keypair persists across container restarts.

### Pattern 3: PKARR Resolution (client side)

**What:** The client CLI accepts either a raw coordinator URL (existing behavior) or a PKARR public key string. If a public key is given, it resolves the coordinator's current endpoint via DHT before running the round.

**When to use:** Client `main.rs` before constructing `CoordinatorClient`.

```rust
// Source: docs.rs/pkarr 5.0.4 Client::resolve [VERIFIED]
use pkarr::{Client, PublicKey, SignedPacket};

pub struct CoordinatorInfo {
    pub coordinator_url: String,
}

pub async fn discover_coordinator(pkarr_pubkey: &str) -> anyhow::Result<CoordinatorInfo> {
    let public_key: PublicKey = pkarr_pubkey.try_into()
        .map_err(|e| anyhow::anyhow!("Invalid PKARR public key: {e}"))?;

    let client = Client::builder().build()?;
    let packet = client.resolve(&public_key).await
        .ok_or_else(|| anyhow::anyhow!("Coordinator not found in DHT for key {pkarr_pubkey}"))?;

    // Extract onion= value from TXT records labeled _coordinator
    let onion = packet
        .resource_records("_coordinator")
        .filter_map(|rr| {
            // TXT record data is &[&[u8]] — flatten and parse
            // Implementation depends on simple string parsing of TXT RDATA
            parse_txt_value(rr, "onion")
        })
        .next()
        .ok_or_else(|| anyhow::anyhow!("No onion= field in coordinator PKARR record"))?;

    // Phase 4 is clearnet — coordinator_url is http://... or the onion address
    // Phase 5 will handle actual Tor routing
    Ok(CoordinatorInfo {
        coordinator_url: format!("http://{onion}"),
    })
}
```

**Client CLI flag addition:** Add `--pkarr-pubkey` (optional) to `ClientConfig`. If set, call `discover_coordinator` before the round loop. If `--coordinator-url` is set, use it directly (existing behavior). These are mutually exclusive.

### Pattern 4: Liquidity Bot Design

**What:** A standalone binary that uses the `client` library. It polls `GET /info`, detects InputReg phase, and fires a full round participation using pre-funded signet wallet keys injected via environment variables.

**Strategy module (`strategy.rs`):**
```rust
pub struct JoinStrategy {
    pub target_denomination_sats: u64,
    pub max_consecutive_failures: u32,
    pub retry_delay_secs: u64,
}

impl JoinStrategy {
    pub fn should_join(&self, info: &RoundInfo) -> bool {
        info.denomination_sats == self.target_denomination_sats
            && info.phase == "input_reg"
            && info.registered_inputs < info.min_participants
    }
}
```

**Main loop:**
```rust
loop {
    let info = client.get_info().await?;
    if strategy.should_join(&info) {
        match participate_in_round(&client, &wallet, &info).await {
            Ok(_) => { consecutive_failures = 0; }
            Err(e) => {
                consecutive_failures += 1;
                tracing::warn!(err = %e, failures = consecutive_failures, "Round participation failed");
                if consecutive_failures >= strategy.max_consecutive_failures {
                    tracing::error!("Too many failures; sleeping for longer");
                    tokio::time::sleep(Duration::from_secs(300)).await;
                    consecutive_failures = 0;
                }
            }
        }
    }
    tokio::time::sleep(Duration::from_secs(5)).await; // PRIV-04 pattern
}
```

The `participate_in_round` function calls the existing `client::round::input`, `output`, and `sign` modules from Phase 3.

### Pattern 5: Docker Compose with Healthcheck-Gated Startup

**What:** bitcoind must be fully synced (or at least accepting RPC) before coordinator starts. Coordinator must be healthy before liquidity-bot connects. Use `depends_on` with `condition: service_healthy`.

**When to use:** `docker/docker-compose.yml`.

```yaml
# Source: docs.docker.com/compose/how-tos/startup-order/ [VERIFIED]
version: "3.9"

services:
  bitcoind:
    image: bitcoin/bitcoin:27
    command: >
      -signet
      -server
      -rpcuser=blindjoin
      -rpcpassword=blindjoin
      -rpcallowip=0.0.0.0/0
      -rpcbind=0.0.0.0
    ports:
      - "38332:38332"
    volumes:
      - bitcoin-data:/home/bitcoin/.bitcoin
    healthcheck:
      # bitcoin-cli -signet getblockchaininfo exits 0 when RPC is up
      test: ["CMD-SHELL", "bitcoin-cli -signet -rpcuser=blindjoin -rpcpassword=blindjoin -rpcconnect=127.0.0.1 getblockchaininfo || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 15
      start_period: 30s

  coordinator:
    build:
      context: ..
      dockerfile: docker/Dockerfile.coordinator
    environment:
      BLINDJOIN__NETWORK__BITCOIN_RPC_URL: "http://bitcoind:38332"
      BLINDJOIN__NETWORK__BITCOIN_NETWORK: "signet"
      BLINDJOIN__COORDINATOR__LISTEN_ADDR: "0.0.0.0:8080"
    depends_on:
      bitcoind:
        condition: service_healthy
    healthcheck:
      test: ["CMD-SHELL", "curl -sf http://localhost:8080/info || exit 1"]
      interval: 10s
      timeout: 5s
      retries: 10
      start_period: 15s
    volumes:
      - coordinator-keys:/app/keys  # persist PKARR keypair

  liquidity-bot:
    build:
      context: ..
      dockerfile: docker/Dockerfile.bot
    environment:
      BLINDJOIN_COORDINATOR_URL: "http://coordinator:8080"
      BLINDJOIN_NETWORK: "signet"
      BLINDJOIN_UTXO: "${BOT_UTXO}"
      BLINDJOIN_UTXO_VALUE_SATS: "${BOT_UTXO_VALUE_SATS}"
      BLINDJOIN_UTXO_WIF: "${BOT_WIF}"
    depends_on:
      coordinator:
        condition: service_healthy

volumes:
  bitcoin-data:
  coordinator-keys:
```

### Pattern 6: Rust Multi-Stage Dockerfile (cargo-chef)

**What:** Three-stage Dockerfile using `cargo-chef` to cache dependency compilation. Rebuilds after code changes do not re-compile all dependencies.

```dockerfile
# Source: lpalmieri.com/posts/fast-rust-docker-builds/ [CITED]
# docker/Dockerfile.coordinator

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Cache dependency compilation (invalidated only when Cargo.lock changes)
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin coordinator

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/coordinator /usr/local/bin/coordinator
ENTRYPOINT ["/usr/local/bin/coordinator"]
```

**Why `debian:bookworm-slim` over `alpine`:** Rust binaries dynamically linked against musl require cross-compilation setup. `debian:bookworm-slim` works with standard Rust toolchain out of the box. If static musl binary is needed, add `--target x86_64-unknown-linux-musl` to the build stage (requires separate toolchain install). [ASSUMED — common practice, not verified for this workspace]

### Anti-Patterns to Avoid

- **Rotating the PKARR keypair on restart:** The coordinator public key is its stable identity. If the keypair is regenerated each time, clients who know the coordinator by public key will not find it. Always persist the keypair to a file/volume.
- **Publishing every 30 seconds:** Mainline DHT has a TTL of several hours and relays cache records. Over-publishing wastes bandwidth and can trigger rate limiting. 5-minute interval matches the spec.
- **Parsing TXT records as raw bytes:** pkarr TXT record data is `&[&[u8]]` (DNS wire format allows multi-string TXT). Join all strings before parsing key=value pairs.
- **Bot joining rounds already at max_participants:** The `should_join` strategy must check `registered_inputs < min_participants` (or use a configured threshold) to avoid competing with real users once a round is filling up.
- **Running liquidity-bot with mainnet wallet keys:** The bot's WIF key must be signet-only. Add a startup guard: if `BLINDJOIN_NETWORK != "signet"`, refuse to start (bot is a testing tool).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| DHT packet signing | Custom Ed25519 + BEP44 encoding | pkarr `SignedPacket::builder().sign(&keypair)` | BEP44 mutable item encoding is subtle; TTL handling, sequence numbers, and signature format are non-trivial |
| Keypair persistence | Custom hex file read/write | `Keypair::write_secret_key_file()` / `from_secret_key_file()` | Already in pkarr crate |
| Dependency caching in Docker | Install all deps from scratch on every build | `cargo-chef` | Without chef, every code change re-compiles all dependencies (5+ minute penalty) |
| Startup ordering without health | `sleep 30` hacks in entrypoint scripts | Docker Compose `depends_on` + `condition: service_healthy` | Sleep is fragile under slow hardware; healthcheck is deterministic |
| DHT relay configuration | Custom HTTP relay server | pkarr default relays (Pubky-operated) | Default relays are production-grade; custom relay adds operational burden with no benefit for signet testing |

**Key insight:** The pkarr crate handles all DHT complexity (BEP44 encoding, relay HTTP, signature verification). The coordinator only needs to build a `SignedPacket` and call `publish()`. The client only needs to call `resolve()` and parse TXT records.

---

## Common Pitfalls

### Pitfall 1: pkarr Version Mismatch

**What goes wrong:** Cargo.toml says `pkarr = "2"` (per STACK.md) but crates.io current is 5.0.4. The `"2"` version constraint will fail to resolve.

**Why it happens:** The stack research was done April 2026 when the crate was described as "2.x" — it has since had major version bumps.

**How to avoid:** Use `pkarr = "5"` in the workspace dependency. The API shape (Client, Keypair, SignedPacket, builder pattern) is consistent across documented versions.

**Warning signs:** `cargo build` fails with "no matching package named pkarr found" or selects an ancient version. [VERIFIED: crates.io API 2026-04-07]

### Pitfall 2: TXT Record Label Collisions

**What goes wrong:** If you publish multiple TXT records under the same label (e.g., `_coordinator`), clients must handle multiple matching records when calling `packet.resource_records("_coordinator")`. Using a single big JSON blob in one TXT record avoids this but hits the 255-byte DNS TXT string limit.

**Why it happens:** DNS TXT records are designed for one key=value pair per record, but pkarr allows multiple records per label.

**How to avoid:** Use a separate label per field (`_onion`, `_denomination`, `_status`) OR encode a compact JSON string that fits in 255 bytes and use a single `_blindjoin` label. The JSON approach from the spec (`blindjoin-technical-spec.md`) is cleaner.

**Warning signs:** Client parsing returns multiple conflicting values for the same field. [ASSUMED — based on DNS spec and pkarr data model]

### Pitfall 3: bitcoind Signet Startup Delay

**What goes wrong:** bitcoind on signet needs to connect to peers and may take 30-120 seconds before `getblockchaininfo` returns successfully. Coordinator starts first, fails the health check to bitcoind, and keeps restarting.

**Why it happens:** `start_period` in the Compose healthcheck is insufficient on slow networks.

**How to avoid:** Set `start_period: 60s` and `retries: 20` on the bitcoind healthcheck. Coordinator's `startup_health_check()` already has fail-fast logic that produces a clear error message.

**Warning signs:** Coordinator exits with "Bitcoin Core unreachable at startup." **Solution:** Increase bitcoind `start_period` in Compose. [ASSUMED — common Docker+Bitcoin pattern]

### Pitfall 4: Liquidity Bot Wallet State Between Rounds

**What goes wrong:** The bot's BDK wallet is initialized with a fixed UTXO. After the first successful round, that UTXO is spent. The bot attempts to register the same UTXO in the next round and fails UTXO validation.

**Why it happens:** Phase 3 client CLI is designed for single-round use. The liquidity bot runs indefinitely.

**How to avoid:** After a successful round, the bot must detect the new change UTXO from the broadcast transaction and update its wallet state before joining the next round. Alternatively, provision the bot with multiple UTXOs and cycle through them. For signet testing, the simpler approach is: one join attempt per bot run, or re-scan the wallet via BDK before each round.

**Warning signs:** Bot logs "UTXO already spent" or coordinator returns 422 on input registration in the second round. [ASSUMED — BDK wallet behavior inference]

### Pitfall 5: pkarr resolve() Returns Stale Cache

**What goes wrong:** `client.resolve()` returns a cached packet even if it has expired. A client may connect to a stale coordinator endpoint.

**Why it happens:** pkarr docs say: "Returns cached packets even if expired; performs background DHT queries for fresher versions." The background query result is only available on the next call.

**How to avoid:** For initial discovery (not latency-sensitive), use `client.resolve_most_recent()` which forces a synchronous DHT/relay query. The extra latency (~500ms-2s) is acceptable for coordinator discovery. [VERIFIED: docs.rs/pkarr Client::resolve_most_recent]

---

## Code Examples

Verified patterns from official sources:

### Full Publish Round-Trip
```rust
// Source: github.com/pubky/pkarr examples/publish.rs [VERIFIED]
use pkarr::{Client, Keypair, SignedPacket};

let keypair = Keypair::random();

let packet = SignedPacket::builder()
    .txt("_foo".try_into()?, "bar=baz".try_into()?, 300)
    .sign(&keypair)?;

let client = Client::builder().build()?;
client.publish(&packet, None).await?;

println!("Published as: {}", keypair.public_key().to_z32());
```

### Full Resolve Round-Trip
```rust
// Source: github.com/pubky/pkarr examples/resolve.rs [VERIFIED]
use pkarr::{Client, PublicKey};

let public_key: PublicKey = "pk:o4dksfbqk85ogzdb5osziw6befigbuxmuxkuxq8434q89uj56uyy"
    .try_into()?;

let client = Client::builder().build()?;

// Use resolve_most_recent() for coordinator discovery (not cached)
if let Some(packet) = client.resolve_most_recent(&public_key).await {
    for rr in packet.resource_records("_coordinator") {
        // Parse rr.rdata for TXT content
        println!("{rr:?}");
    }
}
```

### Docker Compose healthcheck pattern
```yaml
# Source: docs.docker.com/compose/how-tos/startup-order/ [VERIFIED]
healthcheck:
  test: ["CMD-SHELL", "bitcoin-cli -signet -rpcuser=blindjoin -rpcpassword=blindjoin getblockchaininfo || exit 1"]
  interval: 10s
  timeout: 5s
  retries: 15
  start_period: 60s
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| pkarr 2.x (per STACK.md) | pkarr 5.0.4 | March 2026 | Must use `pkarr = "5"` in Cargo.toml |
| Separate `tor` Docker container (goldy image) | Not in Phase 4 scope | Phase 5 | Phase 4 compose stack is clearnet only |
| cargo build without caching | cargo-chef for dependency layer caching | 2021+ (still current) | 5x faster iterative builds |

**Deprecated/outdated:**
- `pkarr = "2"` in the stack research doc: the crate has reached 5.0.4. API shape is documented as stable (builder pattern unchanged).
- Separate `goldy/tor-hidden-service` in the spec's compose example: Phase 4 stays clearnet per roadmap (Approach B). Remove the `tor` service from Phase 4 compose; it belongs in Phase 5.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | pkarr 5.0.4 builder API is backward-compatible with 2.x API shape documented in STACK.md | Standard Stack | If API changed structurally, code examples need revision. Mitigation: check docs.rs before writing code. |
| A2 | `debian:bookworm-slim` works without musl cross-compile for this workspace | Architecture Patterns | If binary links musl-incompatible libs, need `--target x86_64-unknown-linux-musl` or alpine with musl toolchain |
| A3 | Bot's UTXO becomes spent after first round; bot needs UTXO rotation logic | Common Pitfalls | If BDK auto-updates UTXO state from broadcast tx, simpler than assumed |
| A4 | Multiple TXT records under same label are handled as separate resource records by `resource_records()` | Common Pitfalls | If pkarr merges them, parsing strategy may differ |
| A5 | `bitcoin-cli -signet getblockchaininfo` is available inside the `bitcoin/bitcoin:27` image | Architecture Patterns | If image lacks bitcoin-cli, healthcheck command needs adjustment (e.g., `wget` to RPC endpoint) |

---

## Open Questions

1. **TXT record encoding: multiple records vs. JSON blob**
   - What we know: DNS TXT strings have 255-byte limit per string; pkarr allows multiple TXT values
   - What's unclear: Does the spec's JSON format (from `blindjoin-technical-spec.md`) fit in 255 bytes? A compact coordinator record is ~200 bytes for the JSON shown.
   - Recommendation: Use a single `_blindjoin` TXT label with compact JSON. If the coordinator record exceeds 255 bytes, split into multiple records with different labels (`_onion`, `_status`, etc.).

2. **Liquidity bot multi-UTXO provisioning**
   - What we know: Bot needs fresh UTXOs after each round; signet faucet can fund an address
   - What's unclear: Should the bot auto-request signet coins via faucet API, or require pre-funding?
   - Recommendation: Require pre-funding for Phase 4 (simpler). Document faucet integration as a Phase 5 enhancement (FAUCET-01 requirement).

3. **pkarr ClientBuilder `no_dht()` for relay-only mode**
   - What we know: DHT requires Mainline DHT connectivity; relays are HTTP-based
   - What's unclear: For signet testing behind NAT, does DHT work? Relays are more reliable.
   - Recommendation: Default to both DHT + relays (pkarr default). Add a `pkarr_relays_only` config flag for environments where DHT is blocked.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker | DEPL-01 (Compose stack) | [ASSUMED] | — | Manual binary invocation |
| Docker Compose | DEPL-01 | [ASSUMED] | — | Manual process management |
| bitcoin-cli (in PATH or Docker image) | bitcoind healthcheck | [ASSUMED: inside image] | — | HTTP curl healthcheck to RPC |
| cargo-chef | Fast Docker builds | Not required | — | Plain `cargo build` (slower) |

**Missing dependencies with no fallback:** None that block execution.

**Missing dependencies with fallback:** cargo-chef is optional — the Dockerfile works without it but rebuilds are slower. Use plain `cargo build` if chef installation adds complexity.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + tokio::test |
| Config file | none (workspace-level Cargo.toml) |
| Quick run command | `cargo test --lib -p coordinator discovery` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DISC-01 | Coordinator publishes PKARR record with correct TXT fields | unit | `cargo test --lib -p coordinator -- discovery` | No — Wave 0 |
| DISC-02 | Client resolves coordinator via public key string | unit (mock) | `cargo test --lib -p client -- discover` | No — Wave 0 |
| DISC-03 | Heartbeat re-publishes every 5 min; publishes on phase transition | unit | `cargo test --lib -p coordinator -- heartbeat` | No — Wave 0 |
| DEPL-01 | Docker Compose starts all services cleanly | smoke (manual) | `docker compose up -d && docker compose ps` | No — manual |
| DEPL-02 | Liquidity bot joins a round | integration | `cargo test --test integration bot_joins` | No — Wave 0 |
| CLI-01 | `--pkarr-pubkey` flag resolves coordinator before round | unit | `cargo test --lib -p client -- discover::tests` | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --lib -p coordinator && cargo test --lib -p client`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green + Docker Compose smoke test before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `coordinator/src/discovery/mod.rs` — PKARR publisher module skeleton
- [ ] `coordinator/src/discovery/pkarr_pub.rs` — publisher tests
- [ ] `client/src/discover.rs` — resolver module + unit tests
- [ ] `liquidity-bot/src/main.rs` — new binary crate
- [ ] `liquidity-bot/src/strategy.rs` — join strategy + unit tests
- [ ] `liquidity-bot/Cargo.toml` — workspace member declaration
- [ ] Root `Cargo.toml` update: add `liquidity-bot` to members, add `pkarr = "5"`
- [ ] `docker/Dockerfile.coordinator` — multi-stage build
- [ ] `docker/Dockerfile.bot` — multi-stage build
- [ ] `docker/docker-compose.yml` — full stack with healthchecks
- [ ] `tests/integration/bot_joins.rs` — DEPL-02 integration test

---

## Sources

### Primary (HIGH confidence)
- [crates.io/crates/pkarr](https://crates.io/crates/pkarr) — version 5.0.4 confirmed via API, updated 2026-03-25
- [docs.rs/pkarr/latest/pkarr/struct.Client.html](https://docs.rs/pkarr/latest/pkarr/struct.Client.html) — publish/resolve method signatures
- [docs.rs/pkarr/latest/pkarr/struct.SignedPacket.html](https://docs.rs/pkarr/latest/pkarr/struct.SignedPacket.html) — builder API, txt() method
- [docs.rs/pkarr/latest/pkarr/struct.ClientBuilder.html](https://docs.rs/pkarr/latest/pkarr/struct.ClientBuilder.html) — builder configuration methods
- [docs.rs/pkarr/latest/pkarr/struct.PublicKey.html](https://docs.rs/pkarr/latest/pkarr/struct.PublicKey.html) — TryFrom &str, to_z32()
- [docs.docker.com/compose/how-tos/startup-order/](https://docs.docker.com/compose/how-tos/startup-order/) — depends_on condition: service_healthy syntax

### Secondary (MEDIUM confidence)
- [github.com/pubky/pkarr](https://github.com/pubky/pkarr) — README examples, DHT TTL behavior, relay description
- [github.com/pubky/pkarr examples/publish.rs](https://github.com/pubky/pkarr/blob/main/pkarr/examples/publish.rs) — canonical publish example
- [github.com/pubky/pkarr examples/resolve.rs](https://github.com/pubky/pkarr/blob/main/pkarr/examples/resolve.rs) — canonical resolve example
- [lpalmieri.com — cargo-chef pattern](https://lpalmieri.com/posts/fast-rust-docker-builds/) — three-stage Dockerfile for Rust

### Tertiary (LOW confidence / ASSUMED)
- bitcoind signet healthcheck command form — inferred from Docker + bitcoin-cli documentation pattern
- Liquidity bot UTXO rotation problem — inferred from BDK single-use wallet design

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — pkarr version verified against crates.io API; API methods verified against docs.rs
- Architecture: MEDIUM-HIGH — patterns confirmed from official docs; Docker patterns standard; some implementation details assumed
- Pitfalls: MEDIUM — DHT behavior and TXT parsing pitfalls verified; UTXO rotation and stale cache from docs; bitcoind startup timing assumed

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (pkarr moves fast; re-verify version before coding)
