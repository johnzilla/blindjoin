# blindjoin — Technical Specification

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet, discoverable via Pubky ([PKARR](https://github.com/pubky/pkarr)), private via Tor, built in Rust. MIT licensed. No fees. No company. No terms of service.

> **Note:** This document is the architectural and design-rationale companion to the normative wire-protocol specification at [`docs/PROTOCOL.md`](docs/PROTOCOL.md). It covers the higher-level system design — architecture, threat model, and the reasoning behind key choices. Concrete, drift-prone detail is intentionally **not** duplicated here; it lives in the canonical sources:
>
> - **Wire message formats & round-lifecycle MUST/SHOULD:** [`docs/PROTOCOL.md`](docs/PROTOCOL.md) and the Rust types in [`shared/src/protocol.rs`](shared/src/protocol.rs).
> - **Configuration:** [`blindjoin.toml.example`](blindjoin.toml.example) and the config table in the [README](README.md#configuration).
> - **Deployment stack:** [`docker/docker-compose.yml`](docker/docker-compose.yml).

---

## Executive Summary

blindjoin is a lightweight CoinJoin coordinator that anyone can run. It uses RSA blind signatures ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT ([PKARR](https://github.com/pubky/pkarr)), and all protocol traffic flows over Tor hidden services. The project ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. The coordinator software is MIT licensed and designed for signet/testnet. What anyone does with it beyond that is their own decision.

---

## Design Principles

- **Never roll your own crypto.** Every cryptographic operation uses an audited, maintained, reputable library. The project code handles orchestration only.
- **Tor-native, not Tor-optional.** The coordinator has no clearnet endpoint. Ever.
- **Stateless after rounds.** No persistent data about participants between rounds. Round data is zeroed after broadcast.
- **No logging by design.** The coordinator does not log IP addresses, input-output mappings, or participant identifiers. The only emitted data is aggregate round statistics (participant count, denomination, timestamp).
- **Signet-first.** Default configuration targets Bitcoin signet. Mainnet is a config flag, not a code change.

---

## Architecture Overview

```
DISCOVERY LAYER — Pubky / [PKARR](https://github.com/pubky/pkarr)
┌──────────────────────────────────────────────────────┐
│  Coordinator publishes [PKARR](https://github.com/pubky/pkarr) record to DHT:           │
│    • .onion address                                   │
│    • Round parameters (denomination, participants)     │
│    • Wire version, network, supported script types     │
│  Participants resolve coordinator via [PKARR](https://github.com/pubky/pkarr) lookup     │
└────────────────────┬─────────────────────────────────┘
                     │ DHT lookup
                     ▼
TRANSPORT LAYER — Tor Hidden Service
┌──────────────────────────────────────────────────────┐
│  All CoinJoin protocol messages over Tor              │
│    • Each phase uses a fresh Tor circuit              │
│    • JSON-RPC over HTTP (axum)                        │
│    • No clearnet fallback                             │
└────────────────────┬─────────────────────────────────┘
                     │
                     ▼
COORDINATOR — Rust Binary
┌──────────────────────────────────────────────────────┐
│  ┌──────────────┐  ┌─────────────────────────────┐   │
│  │ Round Manager │  │ Blind Signature Engine      │   │
│  │  • State FSM  │  │  • blind-rsa-signatures     │   │
│  │  • Timeouts   │  │  • [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) compliant       │   │
│  │  • Blame      │  │  • No custom crypto         │   │
│  └──────────────┘  └─────────────────────────────┘   │
│  ┌──────────────┐  ┌─────────────────────────────┐   │
│  │ Bitcoin RPC  │  │ [PKARR](https://github.com/pubky/pkarr) Publisher              │   │
│  │  • signet    │  │  • Announce rounds           │   │
│  │  • testnet4  │  │  • Update .onion address     │   │
│  │  • mainnet   │  │  • Heartbeat / status        │   │
│  └──────────────┘  └─────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
                     │
                     ▼
BITCOIN NETWORK — Signet (default)
┌──────────────────────────────────────────────────────┐
│  Bitcoin Core node (bitcoind)                         │
│    • Full signet validation                           │
│    • Mempool monitoring                               │
│    • Transaction broadcast                            │
└──────────────────────────────────────────────────────┘
```

---

## Round Protocol

blindjoin uses a fixed-denomination CoinJoin with RSA blind signatures for unlinkability. This is the same general approach used by early Wasabi Wallet (v1) and JoinMarket, with the blind signature scheme upgraded to the IETF [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) standard.

### Denomination Selection

The coordinator announces a fixed denomination per round (e.g., 0.01 sBTC). All privacy-preserving outputs are this exact amount. Change outputs are returned to participants but are considered linkable — they do not contribute to the anonymity set.

### Phase 1: Input Registration

```
Participant → Coordinator (over Tor circuit A):
  1. Prove ownership of a UTXO (provide signed message with UTXO outpoint)
  2. Coordinator verifies:
     a. UTXO exists and is unspent (Bitcoin RPC)
     b. UTXO value >= denomination + estimated fee share
     c. UTXO has not already been registered this round
  3. Participant generates a random message M (their future output info)
  4. Participant blinds M using coordinator's RSA public key → M_blinded
  5. Participant sends M_blinded to coordinator
  6. Coordinator blind-signs M_blinded → S_blinded (coordinator never sees M)
  7. Coordinator returns S_blinded to participant
  8. Participant unblinds S_blinded → S (valid signature on M, unlinkable to this input)
```

**What the coordinator knows after Phase 1:** A set of valid UTXOs have been registered. It holds blind-signed tokens but cannot link them to any future output registration.

### Phase 2: Output Registration

```
Participant → Coordinator (over Tor circuit B — different from circuit A):
  1. Present the unblinded signature S and the message M
  2. M contains: output address + denomination amount
  3. Coordinator verifies S is a valid signature on M under its RSA key
  4. Coordinator verifies this token has not been redeemed before
  5. Coordinator registers the output
```

**Critical property:** The coordinator issued the blind signature during Phase 1 but cannot determine which input registration produced this particular token. The input-output link is cryptographically broken.

### Phase 3: Transaction Construction & Signing

```
Coordinator:
  1. Constructs a Bitcoin transaction with:
     - All registered inputs
     - All registered outputs (equal denomination)
     - Change outputs (returned to input owners via pre-registered change addresses)
     - Fee allocation (split equally among participants)
  2. Broadcasts unsigned transaction to all participants

Each Participant:
  1. Verifies their output is present and correct
  2. Verifies the fee is reasonable
  3. Signs their input(s)
  4. Returns partial signature to coordinator

Coordinator:
  1. Assembles all partial signatures into the final transaction
  2. Broadcasts to Bitcoin network via Bitcoin Core RPC
  3. Emits round statistics (participant count, denomination, txid)
  4. Zeroes all round state from memory
```

### Phase 4: Blame Protocol

If a participant fails to sign (either malicious or disconnected):

```
  1. Round times out after signing deadline
  2. Coordinator identifies which input(s) did not provide signatures
  3. Those UTXOs are temporarily banned (configurable: 1 hour default)
  4. Round restarts with remaining participants
  5. Banned UTXOs cannot register for new rounds until ban expires
```

The blame protocol does not compromise privacy — the coordinator already knows which UTXOs were registered (from Phase 1). It only learns that a specific UTXO's owner refused to sign, which is observable behavior.

---

## Round State Machine

```
                    ┌──────────────┐
                    │   IDLE       │
                    │ (waiting for │
                    │  participants)│
                    └──────┬───────┘
                           │ min_participants reached
                           ▼
                    ┌──────────────┐
                    │   INPUT_REG  │ ← timeout: 60s (configurable)
                    │              │
                    └──────┬───────┘
                           │ all inputs registered OR timeout
                           ▼
                    ┌──────────────┐
                    │  OUTPUT_REG  │ ← timeout: 60s (configurable)
                    │              │
                    └──────┬───────┘
                           │ all outputs registered OR timeout
                           ▼
                    ┌──────────────┐
                    │   SIGNING    │ ← timeout: 30s (configurable)
                    │              │
                    └──────┬───────┘
                     ╱            ╲
              all signed       missing sigs
                 ╱                    ╲
        ┌────────────┐        ┌──────────────┐
        │  BROADCAST  │        │    BLAME     │
        │  (success)  │        │  (ban + retry)│
        └─────┬──────┘        └──────┬───────┘
              │                      │
              ▼                      ▼
        ┌──────────────┐      ┌──────────────┐
        │    IDLE       │      │   IDLE       │
        └──────────────┘      └──────────────┘
```

---

## Pubky / [PKARR](https://github.com/pubky/pkarr) Discovery

### Coordinator Announcement

The coordinator publishes a signed [PKARR](https://github.com/pubky/pkarr) record to the Mainline DHT announcing its `.onion` address, advertised Bitcoin network, round parameters (denomination, participant bounds, status), and the supported input/output script types. The record is signed by the coordinator's [PKARR](https://github.com/pubky/pkarr) key and resolvable by anyone querying the DHT; it is refreshed on a heartbeat (every 5 minutes) and on state transitions.

> **Wire detail (canonical):** The exact compact field names (`v`, `onion`, `n`, `ds`, `mp`, `st`, `sst`, `ost`) and their semantics are specified in [`docs/PROTOCOL.md` § Discovery](docs/PROTOCOL.md) and produced by [`coordinator/src/discovery/pkarr_pub.rs`](coordinator/src/discovery/pkarr_pub.rs); the client-side resolver and its fail-fast checks (network mismatch, unsupported script type) live in [`client/src/discover.rs`](client/src/discover.rs).

### Client Discovery

The CLI client can discover coordinators by:

1. **Direct key:** User provides a known coordinator [PKARR](https://github.com/pubky/pkarr) public key
2. **Crawl:** Client queries known seed keys that maintain lists of active coordinators (a lightweight, decentralized directory)
3. **Manual .onion:** User provides a Tor .onion address directly, bypassing [PKARR](https://github.com/pubky/pkarr) entirely (useful for testing)

---

## Security Model

### What the Coordinator Cannot Do

| Action | Why |
|--------|-----|
| Link inputs to outputs | RSA blind signatures ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) make this cryptographically impossible |
| Steal funds | The coordinator never holds private keys; participants sign their own inputs |
| Identify participants by IP | All connections are Tor-only; each phase uses a fresh circuit |
| Reconstruct round data after completion | All round state is zeroed from memory post-broadcast |
| Censor specific outputs | The coordinator cannot distinguish which blind token came from which participant |

### What the Coordinator Can Do (Threat Model)

| Action | Mitigation |
|--------|-----------|
| Refuse to complete rounds (DoS) | Participants detect and switch to another coordinator |
| Collude with a blockchain analysis firm to register many sybil inputs | Fixed minimum participant count; larger rounds dilute sybil impact |
| Record which UTXOs registered for a round (but NOT which outputs they map to) | This is observable on-chain anyway — the CoinJoin transaction is public |
| Selectively exclude certain UTXOs | Participants detect exclusion and can report/switch coordinators |

### Availability Threat Model (volume-based DoS — v1.2 Phase 8)

Distinct from coordinator-misbehavior threats above, the coordinator's *own* availability against external flood traffic is bounded by tower middleware on the HTTP layer:

| Attack | Mitigation | Observable behavior |
|--------|-----------|---------------------|
| Flood `/info` or `/round/tx` past the read limit | Per-route `tower_governor::GovernorLayer` with `GlobalKeyExtractor` (Tor-safe — see below) | HTTP 429 + `Retry-After: <seconds>` + JSON body `{"error":{"code":"RATE_LIMITED",...}}` |
| Flood `/round/input`, `/round/output`, or `/round/sign` past the write limit | Same, separate write-bucket `GovernorConfig` (default 120 req/min/route) | HTTP 429 with the same envelope and header |
| Open a request and stall the body / never close | Uniform Router-scope `tower_http::timeout::TimeoutLayer` honoring `request_timeout_secs` | HTTP 408 emitted at the deadline (not after body completion) |
| Open more concurrent Tor streams than the coordinator can serve | `tokio::sync::Semaphore` (`max_concurrent_connections`) gating the arti accept loop; permit acquired *before* `accept`, released via `ConnectionGuard` RAII on close | New streams park at the listener until a permit is released |

**Why `GlobalKeyExtractor` and not `PeerIpKeyExtractor`.** All Tor hidden-service streams arrive at the coordinator from the local arti listener and therefore share an effective peer IP. A per-IP key extractor would either lump every participant into one bucket (functionally identical to the global one) or fail to extract a key and surface as HTTP 500. The protocol therefore makes a deliberate trade: rate limits are global per route, and sybil resistance lives in BIP-322 ownership proofs plus the per-round denomination. Per-peer throttling on Tor is not a v1 goal.

All four hardening knobs (`rate_limit_info_per_min`, `rate_limit_writes_per_min`, `request_timeout_secs`, `max_concurrent_connections`) are validated by `CoordinatorConfig::validate()` at startup — out-of-range values are rejected with an actionable error before any subsystem boots, so misconfiguration cannot panic the coordinator under load. Release builds additionally refuse to start in `tor_mode = false` unless `BLINDJOIN_ALLOW_CLEARNET=1` is set in the environment.

### What Participants Must Do

- Use a fresh Tor circuit for each phase of the round
- Never reuse the same Tor circuit between input registration and output registration
- Verify their output appears in the constructed transaction before signing
- Verify fee is within acceptable bounds before signing

---

## Dependency Map

This table captures *why* each dependency was chosen. Exact pinned versions are not reproduced here (they drift) — the authoritative versions live in the workspace `Cargo.toml` / `Cargo.lock` and the versioned table in [`docs/PROTOCOL.md` § Cryptographic primitives](docs/PROTOCOL.md).

### Cryptography (no custom implementations)

| Crate | Purpose | Author / Provenance |
|-------|---------|---------------------|
| `blind-rsa-signatures` | [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) RSA blind signatures | Frank Denis (jedisct1), author of libsodium |
| `bitcoin` (rust-bitcoin) | Bitcoin primitives, script, serialization | Rust Bitcoin community, widely audited |
| `bdk_wallet` (Bitcoin Dev Kit) | Wallet operations, PSBT, coin selection | Spiral (fka Square Crypto) funded |
| `secp256k1` | ECDSA signing/verification (via rust-bitcoin) | Rust wrapper around Bitcoin Core's libsecp256k1 |

### Networking

| Crate | Purpose | Author / Provenance |
|-------|---------|---------------------|
| `arti-client` | Tor client in Rust | The Tor Project (official) |
| `axum` | HTTP framework for coordinator API | Tokio team |
| `tokio` | Async runtime | Tokio team |

### Discovery

| Crate | Purpose | Author / Provenance |
|-------|---------|---------------------|
| `pkarr` | [PKARR](https://github.com/pubky/pkarr) DHT record publishing/resolution | Synonym (Pubky team) |

### Infrastructure

| Crate | Purpose |
|-------|---------|
| `serde` / `serde_json` | Serialization |
| `tracing` | Structured logging (round stats only, no PII) |
| `clap` | CLI argument parsing |
| `toml` | Configuration file parsing |

---

## Repository Structure

A Cargo workspace of four crates. (Only the crate-level layout is given here; the exact file tree is best read from the repository itself, which stays authoritative as modules move.)

- **`coordinator/`** — the coordinator binary: round state machine + manager, the blind-signature engine (`blind/`), Bitcoin RPC and CoinJoin transaction construction (`bitcoin/`), the axum HTTP API (`api/`), the embedded-Tor adapter (`network/tor.rs`), and PKARR publishing (`discovery/`).
- **`client/`** — the CLI participant: coordinator discovery (`discover.rs`), BDK wallet (`wallet.rs`), the per-phase round flow (`round/`), and per-phase Tor circuits.
- **`liquidity-bot/`** — an auto-joining bot that fills rounds with signet coins during testing/early adoption.
- **`shared/`** — wire types shared by coordinator and client: `protocol.rs` (request/response + `WIRE_VERSION`), `bip322/` (ownership-proof dispatcher), `token.rs`, `errors.rs`.

Integration tests live under `tests/integration/`; crate unit tests live inline (`#[cfg(test)]`). The container build is a single multi-stage [`docker/Dockerfile`](docker/Dockerfile) with per-binary `target` stages (there is no per-service Dockerfile and no Tor sidecar — Tor is in-process via `arti-client`), orchestrated by [`docker/docker-compose.yml`](docker/docker-compose.yml).

---

## Configuration

The coordinator is configured by a TOML file (layered with `BLINDJOIN__*` environment overrides), covering the Bitcoin network and RPC endpoint, the round parameters (denomination, participant bounds, phase timeouts, fee rate), the public-endpoint hardening knobs (rate limits, request timeout, connection cap), the allowed input/output script types (`[bip]`), Tor mode, and PKARR discovery settings.

> **Canonical:** the full, current set of keys with defaults and validation rules is [`blindjoin.toml.example`](blindjoin.toml.example) and the annotated config table in the [README](README.md#configuration); the parser and startup validation are in [`coordinator/src/config.rs`](coordinator/src/config.rs). (Note: Tor is embedded via `arti-client` — there is no `[tor]` socks/control-port section.)

---

## API Specification

The coordinator exposes a small HTTP/JSON API (served over its Tor hidden service in production): `GET /info` for status + round parameters + the round's RSA public key, `POST /round/input` / `POST /round/output` / `POST /round/sign` for the three participant phases, and `GET /round/tx` for the assembled PSBT to verify before signing.

> **Canonical:** the exact request/response bodies, field names, and the versioned `OwnershipProof` envelope are normative in [`docs/PROTOCOL.md`](docs/PROTOCOL.md) and defined as Rust types in [`shared/src/protocol.rs`](shared/src/protocol.rs); the handlers are in [`coordinator/src/api/handlers.rs`](coordinator/src/api/handlers.rs). The reproduced JSON that previously lived here drifted from the wire types, which is exactly what this pointer avoids.

---

## Docker Compose Stack

The project ships a Docker Compose stack that brings up `bitcoind` (signet) plus the coordinator from a single multi-stage [`docker/Dockerfile`](docker/Dockerfile). There is no Tor sidecar container — the coordinator runs its onion service in-process via `arti-client`. The coordinator's API is published on loopback only (`127.0.0.1:8080`), reachable over its `.onion` in production.

> **Canonical:** the live, authoritative stack is [`docker/docker-compose.yml`](docker/docker-compose.yml); the operator quickstart is in the [README](README.md). The compose YAML is not reproduced here to avoid the drift this slim-down removed.

---

## Testing Strategy

The suite has three layers: **unit tests** (inline `#[cfg(test)]`) for the blind-signature round-trip and unlinkability, the FSM transitions/timeouts, fee math, and CoinJoin transaction construction; **signet integration tests** under `tests/integration/` that drive full end-to-end rounds, the blame/ban path, rate limiting, ban-list persistence, and v1.3↔v1.4 binary compatibility against a live `bitcoind`; and **adversarial/property cases** (token replay, invalid/spent UTXO, wrong denomination, tampered PSBT, cross-script-type spoofing). CI runs the lot with `cargo test --workspace` under `BLINDJOIN_REQUIRE_BITCOIND=1` so a missing `bitcoind` fails loudly rather than skipping.

> **Canonical:** the actual tests live in [`tests/integration/`](tests/integration/) and inline in each crate; see [CONTRIBUTING.md](CONTRIBUTING.md) for the local integration-test loop.

---

## Development Roadmap

The original v1.0 build plan (foundation → core protocol → client → discovery/deployment → hardening) has shipped, along with later milestones (v1.2 public-endpoint hardening, v1.3 CI/test reliability, v1.4 BIP-322 multi-script support, v1.5/v1.6 audit charter and supply-chain provenance). For what shipped when, see [`CHANGELOG.md`](CHANGELOG.md) and the git history; for current open work see [`TODO.md`](TODO.md).

---

## Open Questions & Future Work

### Protocol Extensions (Post-v1)

- **PayJoin mode:** Coordinator optionally produces outputs that look like normal 2-input/2-output transactions instead of obvious equal-denomination CoinJoins. This defeats chain analysis heuristics that flag CoinJoin patterns.
- **Variable denominations via WabiSabi:** If a Rust implementation of WabiSabi credentials becomes available, migrate from fixed-denomination RSA blind signatures to WabiSabi's arbitrary-amount credential scheme.
- **Cross-coordinator rounds:** Multiple coordinators collaborate to run a single round, further distributing trust.
- **Mobile client:** iOS/Android client using the same protocol, for users who don't run CLI tools.

### Infrastructure

- **Signet faucet integration:** Client auto-requests signet coins for first-time setup.
- **Block explorer integration:** After round completion, output a link to the CoinJoin TX on a signet explorer.
- **Metrics dashboard:** Optional Prometheus/Grafana stack for coordinator operators (aggregate stats only, no PII).

---

## License

MIT License. Use it, fork it, run it, improve it.
