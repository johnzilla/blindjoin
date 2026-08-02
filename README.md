# blindjoin

[![Release](https://img.shields.io/github/v/release/johnzilla/blindjoin?logo=github&color=blue)](https://github.com/johnzilla/blindjoin/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/johnzilla/blindjoin/ci.yml?branch=main&logo=githubactions&logoColor=white&label=CI)](https://github.com/johnzilla/blindjoin/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?logo=opensourceinitiative&logoColor=white)](LICENSE)
[![Rust 1.95](https://img.shields.io/badge/rust-1.95.0-CE412B?logo=rust&logoColor=white)](rust-toolchain.toml)
[![Bitcoin signet](https://img.shields.io/badge/Bitcoin-signet-F7931A?logo=bitcoin&logoColor=white)](#what-this-does)
[![Tor: arti](https://img.shields.io/badge/Tor-arti--client-7E4798?logo=torproject&logoColor=white)](https://gitlab.torproject.org/tpo/core/arti)
[![Build provenance: SLSA v1.0](https://img.shields.io/badge/build_provenance-SLSA_v1.0-2E7D32?logo=sigstore&logoColor=white)](SECURITY.md#supply-chain-status)

A standalone CoinJoin coordinator and client for Bitcoin signet. Uses RSA blind signatures ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) so the coordinator cryptographically cannot link transaction inputs to outputs. Coordinators are discoverable via [PKARR](https://github.com/pubky/pkarr) DHT and all production traffic flows over Tor hidden services.

MIT licensed. No fees. No company. No terms of service.

---

> [!CAUTION]
> ## ⚠️ WORK IN PROGRESS — NOT PRODUCTION READY — DO NOT USE WITH REAL FUNDS ⚠️
>
> **blindjoin is experimental, unaudited software under active development.** It has
> **not** received a professional security or cryptography audit. It is intended for
> **signet and testnet only** — coins with **no monetary value**.
>
> - **🚫 Do NOT use this with real bitcoin (mainnet) or any funds you are not prepared to lose entirely.**
> - **🚫 Do NOT rely on it for privacy, unlinkability, or anonymity** in any situation where a mistake or bug would harm you. The privacy properties are design goals, not guarantees, and have not been independently verified.
> - **🧪 Expect bugs, breaking changes, protocol changes, and data loss.** Interfaces, wire formats, and behavior can change without notice.
> - **🔍 Mainnet is a config flag, not an endorsement.** The fact that a network can be set to `mainnet` does **not** mean it is safe to do so.
>
> Use it to learn, experiment, test, and contribute — **on worthless coins**. You assume
> all risk. See the [MIT License](LICENSE): the software is provided "AS IS", without
> warranty of any kind.

---

## What This Does

1. Coordinator announces a fixed-denomination CoinJoin round (default: 0.01 BTC) and the input script types it accepts (P2WPKH, P2TR, P2SH-P2WPKH)
2. Participants discover the coordinator via [PKARR](https://github.com/pubky/pkarr) DHT and reject mismatched coordinators **before** opening a Tor circuit
3. Participants register inputs with BIP-322 ownership proofs (one of the three supported script types) and receive blind-signed tokens
4. Participants register outputs using unblinded tokens (on a fresh Tor circuit)
5. Coordinator builds the transaction, participants verify and sign
6. Coordinator broadcasts the final CoinJoin transaction
7. Non-signers are detected, banned, and the round restarts with remaining participants

The blind signature scheme ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) makes it cryptographically impossible for the coordinator to determine which input produced which output. Each round uses an ephemeral RSA key whose lifetime is bounded by a Rust type signature: `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` is set to `None` at the SOLE FSM chokepoint `RoundState::transition_to(Phase::Idle)` (see [docs/AUDIT-CHARTER.md §5](docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window)), which triggers the transitive `rsa::RsaPrivateKey` `ZeroizeOnDrop` chain. The remainder of the round state — registered inputs, partial signatures, blinded tokens — is zeroized from memory by the same Drop sequence.

As of v1.4, the coordinator accepts mixed-script-type rounds (any combination of P2WPKH + P2TR + P2SH-P2WPKH inputs) under an operator-configurable allowlist. The script type of every input is derived from on-chain `script_pubkey` and cross-checked against the client's declaration — a malicious client cannot bypass the per-script-type sighash verification by lying about which type its input is.

## Documentation

- **[Security policy](SECURITY.md)** — how to report a vulnerability (`johnturner@gmail.com`), audit-readiness status, and the v1.6 supply-chain posture: cosign keyless signing + SLSA provenance + SPDX SBOM on every image and release tarball, base-image digest pinning.
- **[Changelog](CHANGELOG.md)** — release notes per milestone, Keep-a-Changelog format.
- **[FAQ](FAQ.md)** — common questions about what blindjoin is, what it protects against, and when to use it.
- **[Protocol specification (draft)](docs/PROTOCOL.md)** — BIP-style normative spec of the coordinator–client wire protocol. Work-in-progress; review and issue feedback welcome.
- **[Technical design](blindjoin-technical-spec.md)** — architectural background and design rationale.
- **[External audit charter](docs/AUDIT-CHARTER.md)** — in-scope modules with file:symbol refs, threat models per module, 9 cross-shape rejection properties, v=2 PSBT handling boundary, RSA SecretKey zeroization window, out-of-scope dependencies, residual risks accepted with rationale, and glossary mapping project terms to plain audit language. Read this first if you're auditing the codebase.
- **[Contributing](CONTRIBUTING.md)** — local prerequisites + how to run the integration test suite (where output lands, how to interpret pass/fail/skip/ignored).

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

The coordinator will start, publish its address to the [PKARR](https://github.com/pubky/pkarr) DHT, and wait for participants. The liquidity bot joins rounds automatically to fill the anonymity set.

**Back up the `coordinator-keys` volume** (or `coordinator_pkarr.key` if you're not using Docker). Losing it creates a new DHT identity; participants holding your old `pk:...` will no longer discover you. See the volume note in [`docker/docker-compose.yml`](docker/docker-compose.yml).

To get a signet UTXO for the bot, use the [signet faucet](https://signet.bc-2.jp/).

**Running just the coordinator** (your own bitcoind / remote RPC, no bot): one `docker run` against the signed `:latest` image. This runs a clearnet listener bound to loopback, intended to sit behind your own Tor hidden service or reverse proxy (the `COORDINATOR_PUBLIC_ADDR` is what clients discover). `BLINDJOIN_ALLOW_CLEARNET=1` acknowledges the WR-04 guardrail — release builds refuse clearnet otherwise. For a built-in arti hidden service with no external Tor to manage, set `BLINDJOIN__COORDINATOR__TOR_MODE=true` instead and drop the `-p`, `LISTEN_ADDR`, and `ALLOW_CLEARNET` lines (arti creates the `.onion` and publishes it to PKARR automatically).

```bash
docker run -d \
  -e BLINDJOIN__NETWORK__BITCOIN_RPC_URL=http://YOUR_BITCOIND_HOST:38332 \
  -e BLINDJOIN__NETWORK__BITCOIN_RPC_USER=YOUR_RPC_USER \
  -e BLINDJOIN__NETWORK__BITCOIN_RPC_PASS=YOUR_RPC_PASS \
  -e BLINDJOIN__NETWORK__BITCOIN_NETWORK=signet \
  -e BLINDJOIN__DISCOVERY__COORDINATOR_PUBLIC_ADDR=YOUR_ONION_OR_HOST:8080 \
  -e BLINDJOIN__COORDINATOR__LISTEN_ADDR=0.0.0.0:8080 \
  -e BLINDJOIN__DISCOVERY__PKARR_KEY_FILE=/app/keys/coordinator_pkarr.key \
  -e BLINDJOIN__COORDINATOR__BAN_FILE_PATH=/app/data/ban_list.jsonl \
  -e BLINDJOIN_ALLOW_CLEARNET=1 \
  -v coordinator-keys:/app/keys \
  -v coordinator-data:/app/data \
  -p 127.0.0.1:8080:8080 \
  --restart unless-stopped \
  ghcr.io/johnzilla/blindjoin-coordinator:latest
```

Pin `:latest` → `:1.7.0` (or whatever shipped) if you want reproducible deploys. Back up the `coordinator-keys` volume.

## Privacy Considerations

blindjoin accepts mixed input script types (P2WPKH, P2TR, P2SH-P2WPKH) in a
single round. This maximizes the anonymity set across address types but
creates a chain-analysis signal: a CoinJoin transaction with a wildly
heterogeneous input set is visually distinguishable from a uniform-script
CoinJoin. Privacy-sensitive users who require uniform-script rounds can run
a dedicated coordinator with a single `allow_*` flag enabled.

The bundled liquidity bot rotates the script type it submits across rounds.
This prevents the bot's UTXOs from forming a uniform-script-type fingerprint
(which would otherwise identify the bot's participation by cross-round
correlation). Rotation is round-robin across the operator-configured
`BLINDJOIN_BOT_SCRIPT_TYPES`; each run is single-shot and uses a fresh
wallet, so output addresses do not cluster across rounds.

**Choosing an anonymity floor (important).** The client refuses to sign unless the
CoinJoin has at least `--min-anonymity-set` distinct denomination outputs, counted
from the PSBT itself (never from a coordinator-reported number — a malicious
coordinator running one real victim plus its own sybils can claim any participant
count). **The default of 3 is a floor, not a strong guarantee: a coordinator that
sybils the round can trivially reach 3 outputs (one victim + two of its own), so
your effective anonymity against a hostile coordinator can be as low as 1.** For
meaningful privacy, raise the floor — and note it is coupled to the coordinator: a
client that requires N outputs will not complete a round on a coordinator whose
`coordinator.min_participants` is below N. To run rounds with a higher real floor,
raise **both** `coordinator.min_participants` (on the coordinator you trust or run)
**and** the client's `--min-anonymity-set` / `BLINDJOIN_MIN_ANONYMITY_SET` together.
Larger sets cost more coordination time and require more honest participants, so pick
the largest value your liquidity can actually fill.

## Build from Source

Requires Rust 1.89+ and cargo (the floor is set by `arti-client` 0.41).

```bash
cargo build --workspace
cargo test --workspace --all-targets   # unit + integration tests
```

Integration tests that require a live `bitcoind` graceful-skip locally when one isn't on `BITCOIND_EXE` / `PATH`. Under `BLINDJOIN_REQUIRE_BITCOIND=1` (which CI sets) they panic-on-miss instead — see [CONTRIBUTING.md](CONTRIBUTING.md) for the canonical local invocation.

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

When `tor_mode` is enabled, the coordinator runs as a Tor v3 hidden service via arti-client. No clearnet listener is created. The .onion address is published to the [PKARR](https://github.com/pubky/pkarr) DHT automatically.

When `tor_mode` is disabled (default), the coordinator listens on `0.0.0.0:8080` for development and testing. **Release builds (`cargo build --release`) refuse to start in clearnet mode** unless `BLINDJOIN_ALLOW_CLEARNET=1` is set — this is a deliberate guardrail against accidentally exposing a production-built coordinator on the open internet without Tor. Debug builds (`cargo run`, `cargo test`) emit a warning and continue.

### Configuration

See `blindjoin.toml.example` for all options. All settings can be overridden with `BLINDJOIN_*` environment variables.

| Setting | Default | Description | Startup-validated |
|---------|---------|-------------|:-:|
| `network.bitcoin_network` | signet | signet, testnet4, regtest, mainnet — an unrecognized value is rejected at startup (no silent fallback to signet) | ✓ |
| `network.bitcoin_rpc_url` | 127.0.0.1:38332 | Bitcoin Core RPC endpoint |  |
| `coordinator.denomination_sats` | 1,000,000 | Fixed output amount (0.01 BTC); must be ≥ 546 (dust) | ✓ |
| `coordinator.min_participants` | 3 | Minimum to start a round; must be ≥ 2 and ≤ `max_participants` | ✓ |
| `coordinator.max_participants` | 20 | Maximum per round; must be ≥ `min_participants` | ✓ |
| `coordinator.fee_rate_sat_per_vbyte` | 2 | Fee rate for the CoinJoin tx; must be ≥ 1 | ✓ |
| `coordinator.round_timeout_input_reg_secs` / `_output_reg_secs` / `_signing_secs` | 60 / 60 / 30 | Phase deadlines; each must be ≥ 1 | ✓ |
| `coordinator.listen_addr` | 0.0.0.0:8080 | HTTP listen address (clearnet mode) |  |
| `coordinator.tor_mode` | false | Run as Tor hidden service |  |
| `coordinator.blame_ban_duration_secs` | 3600 | Ban duration for misbehaving UTXOs |  |
| `coordinator.blame_full_abort_backoff_secs` | 300 | After the consecutive-blame cap trips (round repeatedly failing to complete), pause the round re-armer this long instead of instantly restarting into the same wedge; 0 restarts immediately |  |
| `coordinator.rate_limit_info_per_min` | 600 | Per-route limit for read endpoints (`/info`, `/round/tx`); 1..=60_000. Global bucket (Tor forces `GlobalKeyExtractor`); sized so honest polling doesn't self-throttle — clients tolerate 429 with `Retry-After` backoff | ✓ |
| `coordinator.rate_limit_writes_per_min` | 120 | Per-route limit for write endpoints (`/round/input`, `/round/output`, `/round/sign`); 1..=60_000. Sized to cover one full `max_participants` round's write load | ✓ |
| `coordinator.request_timeout_secs` | 30 | Uniform per-request handler deadline; clients see HTTP 408 on stall | ✓ |
| `coordinator.max_concurrent_connections` | 256 | Cap on simultaneous Tor hidden-service streams; excess connections park | ✓ |
| `discovery.pkarr_key_file` | coordinator_pkarr.key | Ed25519 keypair for DHT identity |  |
| `discovery.heartbeat_interval_secs` | 300 | [PKARR](https://github.com/pubky/pkarr) re-publish interval |  |
| `bip.allow_p2wpkh` | true | Accept BIP-84 P2WPKH inputs (env: `BLINDJOIN__BIP__ALLOW_P2WPKH`) | ✓ |
| `bip.allow_p2tr` | true | Accept BIP-86 P2TR inputs (env: `BLINDJOIN__BIP__ALLOW_P2TR`) | ✓ |
| `bip.allow_p2sh_p2wpkh` | true | Accept BIP-49 P2SH-P2WPKH inputs (env: `BLINDJOIN__BIP__ALLOW_P2SH_P2WPKH`) | ✓ |
| `bip.output_script_type` | p2wpkh | Script type of round outputs; one of `p2wpkh` / `p2tr` / `p2sh-p2wpkh`. MUST match an enabled `allow_*` flag (env: `BLINDJOIN__BIP__OUTPUT_SCRIPT_TYPE`) | ✓ |

**✓ Startup-validated** entries are checked by `CoordinatorConfig::validate()` at boot — out-of-range or nonsensical values (e.g. `min_participants: 1`, `max_participants` below `min_participants`, a dust `denomination_sats`, a `fee_rate` of 0, a zero phase timeout, `request_timeout_secs: 0`, an unrecognized `bitcoin_network`, or `max_concurrent_connections` above the OS file-descriptor cap) cause the coordinator to refuse to start with an actionable error, rather than silently booting hardcoded defaults or panicking later under load. **A config that fails to load or validate is a fatal startup error — the coordinator never falls back to defaults**, so a mainnet-intended daemon can't accidentally boot as signet. The unmarked entries get type-level validation only (TOML/serde parses an integer as an integer, but no semantic bounds are enforced). Marked `coordinator.*` entries cover the round-parameter sanity checks plus the v1.2 Phase 8 DoS-hardening knobs; the four `bip.*` entries are the v1.4 multi-script allowlist (an all-`false` `bip.*` combination, or an `output_script_type` whose matching `allow_*` flag is `false`, refuses to start).

### API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/info` | Coordinator status, round state, RSA public key, `supported_script_types` (v1.4+), `output_script_type` (v1.4+) |
| POST | `/round/input` | Register a UTXO input + receive blind signature (accepts BIP-322 ownership proofs for all enabled script types) |
| POST | `/round/output` | Register an output using unblinded token |
| GET | `/round/tx` | Retrieve unsigned PSBT for verification |
| POST | `/round/sign` | Submit partial signature |

**Script-type advertisement.** `/info` includes `supported_script_types` (a JSON array such as `["p2sh-p2wpkh","p2tr","p2wpkh"]`) and `output_script_type` (a single kebab-case string) so clients can fail-fast on a mismatch before opening a Tor circuit. The same data is published in compact form (`sst`/`ost` fields) in the coordinator's [PKARR](https://github.com/pubky/pkarr) record (`v0.2.0`) so even DHT-discovery callers can skip mismatched coordinators without ever connecting. Both `supported_script_types` and `output_script_type` are `#[serde(default)]` on the wire — pre-v1.4 coordinators with no advertised set are interpreted as `["p2wpkh"]` and v1.4 clients fall back to the legacy witness-only `OwnershipProof` envelope when they detect one.

Errors return a structured JSON envelope:

```json
{
  "error": {
    "code": "UTXO_SPENT",
    "message": "Referenced UTXO is already spent on-chain",
    "round_id": "abc123..."
  }
}
```

**Rate limiting and timeouts.** All endpoints are rate-limited per route — reads default to 600 req/min, writes to 120 req/min. These are **global** buckets (under Tor the coordinator cannot key on peer identity), sized so ordinary protocol load stays off the limiter; the client treats a 429 as retryable and backs off per `Retry-After` rather than aborting the round. When a route is flooded, the coordinator returns **HTTP 429** with a `Retry-After` header and the same JSON envelope, with `code: "RATE_LIMITED"`:

```json
{
  "error": {
    "code": "RATE_LIMITED",
    "message": "Rate limit exceeded for this endpoint",
    "round_id": "..."
  }
}
```

Handlers that stall past `request_timeout_secs` return **HTTP 408**. Clients SHOULD back off according to `Retry-After` and retry on a fresh Tor circuit. Per-peer throttling is intentionally impossible on Tor (`GlobalKeyExtractor` — all Tor connections look identical to the coordinator); sybil resistance comes from BIP-322 ownership proofs and the per-round denomination, not from per-IP rate limits.

## Run the Client

```bash
# Generate a wallet of a given script type (writes descriptors.txt, mode 0600)
cargo run -p client -- --generate-wallet --type p2tr            # BIP-86 Taproot
cargo run -p client -- --generate-wallet --type p2sh-p2wpkh     # BIP-49 wrapped-segwit
cargo run -p client -- --generate-wallet --type p2wpkh          # BIP-84 native segwit (default)

# Direct connection (development) — uses a WIF P2WPKH key (legacy v1.3 path)
cargo run -p client -- --coordinator-url http://127.0.0.1:8080 \
  --wif <your-private-key-wif> \
  --output-address <destination-address>

# Direct connection with a descriptor wallet (works for all 3 script types)
cargo run -p client -- --coordinator-url http://127.0.0.1:8080 \
  --descriptor descriptors.txt \
  --type p2tr \
  --output-address <destination-address>

# Via Tor (production) — coordinator discovery via [PKARR](https://github.com/pubky/pkarr) DHT
cargo run -p client -- --pkarr-pubkey <coordinator-public-key> \
  --descriptor descriptors.txt \
  --type p2tr \
  --output-address <destination-address> \
  --tor
```

The `--type` flag (also settable via the `BLINDJOIN_SCRIPT_TYPE` env var) accepts `p2wpkh`, `p2tr`, or `p2sh-p2wpkh` and selects the BIP descriptor template at wallet generation (BIP-84 / BIP-86 / BIP-49 respectively). It also tells the discovery layer which script type to require — pointing a `--type p2tr` client at a coordinator that has `bip.allow_p2tr = false` fails fast with `DiscoveryError::UnsupportedScriptType` **before** any Tor circuit opens, naming both the coordinator and the missing script type.

When `--tor` is enabled, the client uses per-phase Tor circuit isolation: input registration flows through one circuit (alice) and output registration flows through a different circuit (bob). This prevents the coordinator from correlating phases by Tor circuit.

**Backwards compatibility.** A v1.4 client pointed at a pre-`0.2.0` (v1.3) coordinator detects the absence of `supported_script_types` on `/info` and falls back to the legacy witness-only `OwnershipProof` wire format. A v1.3 client unaware of v1.4 advertisement happily registers a P2WPKH UTXO against a v1.4 coordinator — the v1.4 coordinator's `OwnershipProof` decoder is a two-phase try-parse that accepts v1.3 array-of-hex shape as `version = 1`.

## Pre-built Binaries

Download pre-built binaries from [GitHub Releases](https://github.com/johnzilla/blindjoin/releases) (Linux x86_64). Other platforms: build from source with `cargo build --release`.

Docker images are also published to `ghcr.io/johnzilla/blindjoin-coordinator`, `ghcr.io/johnzilla/blindjoin-client`, and `ghcr.io/johnzilla/blindjoin-bot`. Every tagged image push (v1.6+) carries a cosign keyless signature, a SLSA v1.0 provenance attestation, and an SPDX SBOM attestation. Release tarballs ship the cosign `.bundle` + SLSA `.sigstore` companion assets. Verify recipes: [SECURITY.md § Supply-chain status](SECURITY.md#supply-chain-status).

## CI/CD

Every pull request — and every push to `main` — runs four independent CI jobs:

| Job | Command | Blocks merge? |
|-----|---------|---------------|
| `cargo test` | `cargo test --workspace --all-targets` | Yes |
| `cargo clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | Yes |
| `cargo audit` | `cargo audit` | Yes |
| `coordinator binary builds` | `cargo build --release --bin coordinator` | Yes |

The `cargo test` step runs `--all-targets`, so integration tests under `tests/integration/` execute alongside unit tests. **As of v1.3 Phase 9, CI provisions a pinned Bitcoin Core v30.2 binary** (cached via `actions/cache`, integrity-verified against achow101's release signature pulled from a SHA-pinned `guix.sigs` commit, then sha256-checked against the signed `SHA256SUMS`). The version pin lives in [`.bitcoind-version`](.bitcoind-version) — a single-line bump is all it takes to roll forward. With bitcoind available, CI sets `BLINDJOIN_REQUIRE_BITCOIND=1` so any missing bitcoind in CI fails fast rather than silently graceful-skipping. The `coordinator binary builds` smoke job validates that the production binary links cleanly — it does not start the coordinator (that requires bitcoind).

The `cargo audit` step uses [`.cargo/audit.toml`](.cargo/audit.toml) to declare accepted residual risks. Each ignored advisory carries a written rationale in that file; an ignore without a rationale is a code-review-blocking change.

Release and Docker workflows also run test+clippy as a prerequisite before building. All GitHub Actions are pinned to immutable commit SHAs. Release archives include SHA-256 checksums.

**Base-image digest pinning:** `docker/Dockerfile` pins both base images (`debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`) by sha256 digest directly in the `FROM` lines. To check for upstream drift and bump via a one-line PR, see [CONTRIBUTING.md §Bumping base-image digests](CONTRIBUTING.md#bumping-base-image-digests).

For branch protection setup, see [docs/branch-protection.md](docs/branch-protection.md).

## Project Structure

```
blindjoin/
  coordinator/       # CoinJoin coordinator binary
    src/
      api/           # HTTP handlers (axum)
      bitcoin/       # RPC client, UTXO validation, BIP-322, PSBT builder, fee estimation
      blind/         # RSA blind signature engine
      round/         # State machine, input/output reg, signing, blame
      discovery/     # [PKARR](https://github.com/pubky/pkarr) DHT publisher
      network/       # Tor hidden service (arti-client)
      config.rs      # TOML + env var configuration
      main.rs        # Startup, health checks, server
  client/            # CLI participant client
    src/
      round/         # Input registration, output registration, signing
      wallet.rs      # bdk_wallet key management, BIP-322 proofs, PSBT signing
      http.rs        # Coordinator HTTP client (clearnet + Tor)
      tor.rs         # Per-phase Tor circuit isolation (alice/bob)
      discover.rs    # [PKARR](https://github.com/pubky/pkarr) DHT coordinator discovery
      config.rs      # CLI argument parsing (clap)
  shared/            # Protocol types shared between coordinator and client
    src/
      protocol.rs    # Wire message structs (serde, forward-compatible)
      token.rs       # Blind token message computation (domain-separated SHA-256)
      bip322/        # BIP-322 Simple dispatcher + per-script-type sign/verify (v1.4)
        mod.rs       # ScriptType enum, dispatcher, 26-LOC bip322-crate adapter
        p2wpkh.rs    # BIP-143 ECDSA sign/verify
        p2tr.rs      # BIP-341 Schnorr keypath sign/verify
        p2sh_p2wpkh.rs # BIP-49-wrapped P2WPKH sign/verify
      errors.rs      # Structured error codes
      types.rs       # Common types (RoundId, Denomination)
    tests/
      bip322_cross_shape.rs    # 9 cross-shape rejection tests (V1.4-CRIT-01 mitigation)
      per_script_vectors.rs    # Vendored official BIP-322 vectors per script type
      ownership_proof_roundtrip.rs # v1.3 array-of-hex ↔ v1.4 flat-struct compat
  liquidity-bot/     # Auto-joins rounds for testing and cold-start
    src/
      main.rs        # Polling loop, signet safety guard
      strategy.rs    # Join strategy and round participation
  docker/            # Docker Compose stack
    docker-compose.yml
    Dockerfile        # Multi-target Dockerfile (coordinator, client, liquidity-bot)
    digests.txt       # Canonical base-image digest manifest (v1.6) — bumped only via CODEOWNERS-gated PR
    bitcoind/bitcoin.conf
  docs/
    PROTOCOL.md           # BIP-style wire-protocol spec (draft; Milestone 1)
    branch-protection.md  # GitHub Rulesets setup + CODEOWNERS gate
  tests/
    integration/     # End-to-end CoinJoin round tests
      mod.rs         # Shared: require_bitcoind!() macro, BitcoindGuard RAII, bootstrap_regtest_bitcoind()
  .cargo/
    audit.toml       # Declared residual risks (each ignore documented inline)
  .github/
    actions/
      install-bitcoind/   # Composite action: pinned bitcoind install with PGP verification
    workflows/
      ci.yml                 # PR-triggered test, clippy --all-targets, audit gates + pinned-bitcoind install
      release.yml            # Cross-compiled binary releases (gated on test+clippy)
      docker.yml             # Multi-arch Docker image publishing (gated on test+clippy)
  .bitcoind-version  # Pinned Bitcoin Core version used by CI's integration test job
  CONTRIBUTING.md    # Local prerequisites + how to run integration tests
  TODO.md            # Open and recently-resolved tech-debt items
```

## Security Model

**Reporting a vulnerability:** email `johnturner@gmail.com` with subject `[blindjoin security]`. Full policy: [SECURITY.md](SECURITY.md). The section below describes the protocol-level guarantees and operator-facing hardening.

The coordinator **cannot**:
- Link inputs to outputs (RSA blind signatures, [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html))
- Steal funds (participants sign their own inputs)
- Reconstruct round data after completion (round-end zeroization is structurally bounded — see [docs/AUDIT-CHARTER.md §5](docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window))
- Correlate input and output registration by Tor circuit (client uses isolated circuits)

The coordinator **can**:
- Refuse to complete rounds (participants detect and switch coordinators via DHT)
- Register sybil inputs (fixed minimum participant count dilutes impact)
- See which UTXOs registered (observable on-chain anyway)

Session tokens use HMAC with constant-time comparison. BIP-322 ownership proofs verified for all inputs. Banned UTXOs persisted to disk (SHA-256 hashed, append-only JSONL). No PII logging.

**Availability hardening (v1.1):** Async RPC calls execute before the write lock so slow bitcoind cannot serialize participants. RSA keys are parsed once per round (not per request). Blinded tokens are size-bounded to the RSA modulus. Addresses are validated at registration time (not at PSBT build). Duplicate partial signatures are rejected.

**Public-endpoint hardening (v1.2 Phase 8):** Per-route rate limits via `tower_governor` (reads 600/min, writes 120/min by default) return HTTP 429 with `Retry-After` and a `RATE_LIMITED` JSON envelope. These are **global** buckets — under Tor the coordinator cannot key on peer identity (`GlobalKeyExtractor`), so no finite limit can single out an abuser; the defaults are sized so ordinary protocol load stays off the limiter, and the reference client treats a 429 as retryable (honoring `Retry-After`, bounded retries) on **every** read and write call rather than aborting the round — so bucket contention degrades gracefully instead of starving a mid-round client into a non-signer ban. A uniform request timeout (default 30s) caps handler runtime — slow clients see HTTP 408 rather than tying up worker slots. Concurrent Tor hidden-service streams are bounded by a `tokio::sync::Semaphore` (default 256) wrapping the accept loop. These knobs are operator-tunable in `coordinator.toml` and validated at startup so a misconfigured value fails fast rather than panicking under load. Per-peer throttling is impossible on Tor by design (all streams share an effective IP); sybil resistance lives in BIP-322 proofs and the per-round denomination, not the rate limiter.

**Supply-chain hygiene:** TLS is pure-Rust [rustls](https://github.com/rustls/rustls) across the entire dependency tree; the openssl crate chain is not pulled in. `cargo audit` blocks merge on any advisory not declared in [`.cargo/audit.toml`](.cargo/audit.toml), where each accepted residual risk carries a written rationale. `cargo clippy --all-targets` blocks merge on any lint, including in integration-test code. CI's `bitcoind` install (v1.3+) verifies the Bitcoin Core tarball against achow101's PGP signature (key fingerprint `152812300785C96444D3334D17565732E08E5E41`, from a SHA-pinned `guix.sigs` commit). v1.6 adds: base-image digest pinning (sha256 digests inlined into `docker/Dockerfile`'s `FROM` lines); cosign keyless OIDC signing + SLSA v1.0 provenance + SPDX SBOM on every ghcr.io image push and on every release tarball; pinned cosign-installer at v3.10.1 (cosign 2.6.3); a `sigstore-pin-check` CI gate that fails the build on a floating sigstore-action tag. Verify recipes: [SECURITY.md § Supply-chain status](SECURITY.md#supply-chain-status).

**External audit charter (v1.5):** [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md) enumerates in-scope modules with file:symbol refs, threat models per module, the 9 cross-shape rejection properties locked at v1.4, the v=2 OwnershipProof PSBT handling boundary, the RSA SecretKey zeroization window (RoundSecretKey + bounded lifetime per AUDIT-03), out-of-scope dependencies, residual risks accepted with rationale, and a glossary mapping project terms to plain audit language.

**Test infrastructure (v1.3 Phase 9):** Integration tests under `tests/integration/` no longer silently graceful-skip in CI — under `BLINDJOIN_REQUIRE_BITCOIND=1` (workflow-level env), tests that can't find bitcoind PANIC, surfacing the misconfiguration immediately. The historical `Box::leak(node)` pattern that blocked cargo's stdout pipe behind orphan bitcoind processes is replaced with a `BitcoindGuard` RAII type whose `Drop::drop` runs `node.stop()` via `tokio::spawn_blocking`. All 8 `full_round::*` end-to-end tests run by default (six former `#[ignore = "TODO(Phase-10)..."]` carve-outs were closed once the wire-format mismatch in client/server witness encoding was repaired). See [CONTRIBUTING.md](CONTRIBUTING.md) for local invocation.

**Multi-script script-type integrity (v1.4):** The coordinator's `validate_utxo` derives the `ScriptType` of every input from the on-chain `script_pubkey` (not from the client-declared field on the wire) and cross-checks against the declaration; mismatch returns `Bip322Error::ScriptTypeMismatch` **before** the per-script verifier ever runs. The `shared::bip322` dispatcher is the only public verifier surface — per-script verify/sign functions are `pub(crate)`-only, so a caller cannot reach `p2wpkh::verify` from outside the crate to bypass dispatch. A `bip322-pin-check` CI job enforces the `bip322 = "=0.0.10"` exact pin (the crate is pre-1.0 and any minor release can break the adapter at `shared/src/bip322/mod.rs`). 9 cross-shape rejection tests in `shared/tests/bip322_cross_shape.rs` lock the V1.4-CRIT-01 spoofing-vector closure at the `shared/` crate boundary.

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `blind-rsa-signatures` | [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) RSA blind signatures (jedisct1) |
| `bitcoin` (rust-bitcoin) | Bitcoin primitives, PSBT, scripts |
| `bip322` | BIP-322 Simple verifier (rust-bitcoin org). Exact-pinned to `=0.0.10` and enforced by a `bip322-pin-check` CI gate — the crate is pre-1.0 and any minor release can break the wire format. Wrapped behind a 26-LOC zero-lossy adapter so a future swap is mechanical. |
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

