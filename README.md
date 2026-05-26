# blindjoin

A standalone CoinJoin coordinator and client for Bitcoin signet. Uses RSA blind signatures (RFC 9474) so the coordinator cryptographically cannot link transaction inputs to outputs. Coordinators are discoverable via PKARR DHT and all production traffic flows over Tor hidden services.

MIT licensed. No fees. No company. No terms of service.

## What This Does

1. Coordinator announces a fixed-denomination CoinJoin round (default: 0.01 BTC)
2. Participants register inputs with BIP-322 ownership proofs and receive blind-signed tokens
3. Participants register outputs using unblinded tokens (on a fresh Tor circuit)
4. Coordinator builds the transaction, participants verify and sign
5. Coordinator broadcasts the final CoinJoin transaction
6. Non-signers are detected, banned, and the round restarts with remaining participants

The blind signature scheme (RFC 9474) makes it cryptographically impossible for the coordinator to determine which input produced which output. Each round uses ephemeral RSA keys that are destroyed after broadcast. All round state is zeroed from memory.

## Documentation

- **[FAQ](FAQ.md)** — common questions about what blindjoin is, what it protects against, and when to use it.
- **[Protocol specification (draft)](docs/PROTOCOL.md)** — BIP-style normative spec of the coordinator–client wire protocol. Work-in-progress; review and issue feedback welcome.
- **[Technical design](blindjoin-technical-spec.md)** — architectural background and design rationale.

## Quick Start (Docker)

The fastest way to run blindjoin is with Docker Compose. This starts bitcoind (signet), the coordinator, and a liquidity bot that auto-joins rounds.

```bash
# Clone and configure
git clone https://github.com/johnzilla/blindjoin.git
cd blindjoin
cp .env.example .env
# Edit .env with your signet UTXO details (see below)

# Start the stack
docker compose -f docker/docker-compose.yml up
```

The coordinator will start, publish its address to the PKARR DHT, and wait for participants. The liquidity bot joins rounds automatically to fill the anonymity set.

To get a signet UTXO for the bot, use the [signet faucet](https://signet.bc-2.jp/).

## Build from Source

Requires Rust 1.89+ and cargo (the floor is set by `arti-client` 0.41).

```bash
cargo build --workspace
cargo test --workspace --all-targets   # unit + integration tests (integration tests skip gracefully without bitcoind)
```

## Run the Coordinator

Requires a running Bitcoin Core node (signet, testnet, or regtest).

```bash
# Copy and edit config
cp blindjoin.toml.example blindjoin.toml

# Start coordinator (clearnet, for development)
cargo run -p coordinator

# Start coordinator (Tor hidden service, for production)
BLINDJOIN_COORDINATOR_TOR_MODE=true cargo run -p coordinator

# Release builds refuse to start in clearnet mode unless explicitly acknowledged
BLINDJOIN_ALLOW_CLEARNET=1 ./target/release/coordinator   # only if you know what you're doing
```

When `tor_mode` is enabled, the coordinator runs as a Tor v3 hidden service via arti-client. No clearnet listener is created. The .onion address is published to the PKARR DHT automatically.

When `tor_mode` is disabled (default), the coordinator listens on `0.0.0.0:8080` for development and testing. **Release builds (`cargo build --release`) refuse to start in clearnet mode** unless `BLINDJOIN_ALLOW_CLEARNET=1` is set — this is a deliberate guardrail against accidentally exposing a production-built coordinator on the open internet without Tor. Debug builds (`cargo run`, `cargo test`) emit a warning and continue.

### Configuration

See `blindjoin.toml.example` for all options. All settings can be overridden with `BLINDJOIN_*` environment variables.

| Setting | Default | Description |
|---------|---------|-------------|
| `network.bitcoin_network` | signet | signet, testnet4, regtest, mainnet |
| `network.bitcoin_rpc_url` | 127.0.0.1:38332 | Bitcoin Core RPC endpoint |
| `coordinator.denomination_sats` | 1,000,000 | Fixed output amount (0.01 BTC) |
| `coordinator.min_participants` | 3 | Minimum to start a round |
| `coordinator.max_participants` | 20 | Maximum per round |
| `coordinator.listen_addr` | 0.0.0.0:8080 | HTTP listen address (clearnet mode) |
| `coordinator.tor_mode` | false | Run as Tor hidden service |
| `coordinator.blame_ban_duration_secs` | 3600 | Ban duration for misbehaving UTXOs |
| `coordinator.rate_limit_info_per_min` | 60 | Per-route limit for read endpoints (`/info`, `/round/tx`); 1..=60_000 |
| `coordinator.rate_limit_writes_per_min` | 30 | Per-route limit for write endpoints (`/round/input`, `/round/output`, `/round/sign`); 1..=60_000 |
| `coordinator.request_timeout_secs` | 30 | Uniform per-request handler deadline; clients see HTTP 408 on stall |
| `coordinator.max_concurrent_connections` | 256 | Cap on simultaneous Tor hidden-service streams; excess connections park |
| `discovery.pkarr_key_file` | coordinator_pkarr.key | Ed25519 keypair for DHT identity |
| `discovery.heartbeat_interval_secs` | 300 | PKARR re-publish interval |

All four hardening knobs are validated at startup — out-of-range values are rejected with an actionable error before any subsystem reads them.

### API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/info` | Coordinator status, round state, RSA public key |
| POST | `/round/input` | Register a UTXO input + receive blind signature |
| POST | `/round/output` | Register an output using unblinded token |
| GET | `/round/tx` | Retrieve unsigned PSBT for verification |
| POST | `/round/sign` | Submit partial signature |

Errors return structured JSON: `{"error": {"code": "UTXO_SPENT", "message": "...", "round_id": "..."}}`.

**Rate limiting and timeouts.** All endpoints are rate-limited per route — reads default to 60 req/min, writes to 30 req/min. When a route is flooded, the coordinator returns **HTTP 429** with a `Retry-After` header and the same JSON envelope (`code: "RATE_LIMITED"`). Handlers that stall past `request_timeout_secs` return **HTTP 408**. Clients SHOULD back off according to `Retry-After` and retry on a fresh Tor circuit. Per-peer throttling is intentionally impossible on Tor (`GlobalKeyExtractor` — all Tor connections look identical to the coordinator); sybil resistance comes from BIP-322 ownership proofs and the per-round denomination, not from per-IP rate limits.

## Run the Client

```bash
# Direct connection (development)
cargo run -p client -- --coordinator-url http://127.0.0.1:8080 \
  --wif <your-private-key-wif> \
  --output-address <destination-address>

# Via Tor (production)
cargo run -p client -- --coordinator-url http://<onion-address> \
  --wif <your-private-key-wif> \
  --output-address <destination-address> \
  --tor

# Discover coordinator via PKARR DHT
cargo run -p client -- --pkarr-pubkey <coordinator-public-key> \
  --wif <your-private-key-wif> \
  --output-address <destination-address>
```

When `--tor` is enabled, the client uses per-phase Tor circuit isolation: input registration flows through one circuit (alice) and output registration flows through a different circuit (bob). This prevents the coordinator from correlating phases by Tor circuit.

## Pre-built Binaries

Download pre-built binaries from [GitHub Releases](https://github.com/johnzilla/blindjoin/releases) (Linux x86_64). Other platforms: build from source with `cargo build --release`.

Docker images are also published to `ghcr.io/johnzilla/blindjoin-coordinator`, `ghcr.io/johnzilla/blindjoin-client`, and `ghcr.io/johnzilla/blindjoin-bot`.

## CI/CD

Every pull request — and every push to `main` — runs four independent CI jobs:

| Job | Command | Blocks merge? |
|-----|---------|---------------|
| `cargo test` | `cargo test --workspace --all-targets` | Yes |
| `cargo clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | Yes |
| `cargo audit` | `cargo audit` | Yes |
| `coordinator binary builds` | `cargo build --release --bin coordinator` | Yes |

The `cargo test` step runs `--all-targets`, so integration tests under `tests/integration/` execute alongside unit tests. Tests that require a live bitcoind skip gracefully when one isn't available. The `coordinator binary builds` smoke job validates that the production binary links cleanly — it does not start the coordinator (that requires bitcoind).

The `cargo audit` step uses [`.cargo/audit.toml`](.cargo/audit.toml) to declare accepted residual risks. Each ignored advisory carries a written rationale in that file; an ignore without a rationale is a code-review-blocking change.

Release and Docker workflows also run test+clippy as a prerequisite before building. All GitHub Actions are pinned to immutable commit SHAs. Release archives include SHA-256 checksums.

To enable branch protection, see [docs/branch-protection.md](docs/branch-protection.md).

## Project Structure

```
blindjoin/
  coordinator/       # CoinJoin coordinator binary
    src/
      api/           # HTTP handlers (axum)
      bitcoin/       # RPC client, UTXO validation, BIP-322, PSBT builder, fee estimation
      blind/         # RSA blind signature engine
      round/         # State machine, input/output reg, signing, blame
      discovery/     # PKARR DHT publisher
      network/       # Tor hidden service (arti-client)
      config.rs      # TOML + env var configuration
      main.rs        # Startup, health checks, server
  client/            # CLI participant client
    src/
      round/         # Input registration, output registration, signing
      wallet.rs      # bdk_wallet key management, BIP-322 proofs, PSBT signing
      http.rs        # Coordinator HTTP client (clearnet + Tor)
      tor.rs         # Per-phase Tor circuit isolation (alice/bob)
      discover.rs    # PKARR DHT coordinator discovery
      config.rs      # CLI argument parsing (clap)
  shared/            # Protocol types shared between coordinator and client
    src/
      protocol.rs    # Wire message structs (serde, forward-compatible)
      token.rs       # Blind token message computation (domain-separated SHA-256)
      bip322.rs      # BIP-322 Simple message signing primitives
      errors.rs      # Structured error codes
      types.rs       # Common types (RoundId, Denomination)
  liquidity-bot/     # Auto-joins rounds for testing and cold-start
    src/
      main.rs        # Polling loop, signet safety guard
      strategy.rs    # Join strategy and round participation
  docker/            # Docker Compose stack
    docker-compose.yml
    Dockerfile        # Multi-target Dockerfile (coordinator, client, liquidity-bot)
    bitcoind/bitcoin.conf
  docs/
    PROTOCOL.md           # BIP-style wire-protocol spec (draft; Milestone 1)
    branch-protection.md  # GitHub branch protection setup guide
  tests/
    integration/     # End-to-end CoinJoin round tests (skip gracefully w/o bitcoind)
  .cargo/
    audit.toml       # Declared residual risks (each ignore documented inline)
  .github/workflows/
    ci.yml           # PR-triggered test, clippy --all-targets, audit gates
    release.yml      # Cross-compiled binary releases (gated on test+clippy)
    docker.yml       # Multi-arch Docker image publishing (gated on test+clippy)
  TODO.md            # Open and recently-resolved tech-debt items
```

## Security Model

The coordinator **cannot**:
- Link inputs to outputs (RSA blind signatures, RFC 9474)
- Steal funds (participants sign their own inputs)
- Reconstruct round data after completion (all state zeroed from memory via zeroize)
- Correlate input and output registration by Tor circuit (client uses isolated circuits)

The coordinator **can**:
- Refuse to complete rounds (participants detect and switch coordinators via DHT)
- Register sybil inputs (fixed minimum participant count dilutes impact)
- See which UTXOs registered (observable on-chain anyway)

Session tokens use HMAC with constant-time comparison. BIP-322 ownership proofs verified for all inputs. Banned UTXOs persisted to disk (SHA-256 hashed, append-only JSONL). No PII logging.

**Availability hardening (v1.1):** Async RPC calls execute before the write lock so slow bitcoind cannot serialize participants. RSA keys are parsed once per round (not per request). Blinded tokens are size-bounded to the RSA modulus. Addresses are validated at registration time (not at PSBT build). Duplicate partial signatures are rejected.

**Public-endpoint hardening (v1.2 Phase 8):** Per-route rate limits via `tower_governor` (reads 60/min, writes 30/min by default) return HTTP 429 with `Retry-After` and a `RATE_LIMITED` JSON envelope. A uniform request timeout (default 30s) caps handler runtime — slow clients see HTTP 408 rather than tying up worker slots. Concurrent Tor hidden-service streams are bounded by a `tokio::sync::Semaphore` (default 256) wrapping the accept loop. All four knobs are operator-tunable in `coordinator.toml` and validated at startup so a misconfigured value fails fast rather than panicking under load. Per-peer throttling is impossible on Tor by design (all streams share an effective IP), so the coordinator deliberately uses `GlobalKeyExtractor`; sybil resistance lives in BIP-322 proofs and the per-round denomination, not the rate limiter.

**Supply-chain hygiene:** TLS is pure-Rust [rustls](https://github.com/rustls/rustls) across the entire dependency tree; the openssl crate chain is not pulled in. The `cargo audit` CI step blocks merge on any advisory not declared in [`.cargo/audit.toml`](.cargo/audit.toml), where each accepted residual risk carries a written rationale. The `cargo clippy --all-targets` CI step blocks merge on any lint, including in integration-test code.

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `blind-rsa-signatures` | RFC 9474 RSA blind signatures (jedisct1) |
| `bitcoin` (rust-bitcoin) | Bitcoin primitives, PSBT, scripts |
| `bdk_wallet` | Client wallet: key management, UTXO selection, PSBT signing |
| `arti-client` | Tor hidden service (coordinator) and circuit isolation (client). Configured `default-features = false` with the `rustls` feature so the TLS backend is pure-Rust and the openssl chain is not in the dep tree. |
| `pkarr` | Coordinator discovery via Mainline DHT |
| `axum` | HTTP framework for coordinator API |
| `tower_governor`, `tower-http` | Per-route rate limiting (GovernorLayer) and uniform request timeouts (TimeoutLayer) on the coordinator API |
| `tokio` | Async runtime |
| `zeroize` | Memory zeroing for sensitive round state |
| `reqwest` | HTTP client with SOCKS5 proxy support for Tor |

## License

[MIT](LICENSE)

