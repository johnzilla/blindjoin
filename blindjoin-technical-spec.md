# blindjoin — Technical Specification

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet, discoverable via Pubky (PKARR), private via Tor, built in Rust. MIT licensed. No fees. No company. No terms of service.

> **Note:** This document is the architectural and design-rationale companion to the normative wire-protocol specification at [`docs/PROTOCOL.md`](docs/PROTOCOL.md). For implementation-level message formats, field semantics, and round-lifecycle MUST/SHOULD requirements, refer to PROTOCOL.md. This document covers the higher-level system design.

---

## Executive Summary

blindjoin is a lightweight CoinJoin coordinator that anyone can run. It uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. The project ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

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
DISCOVERY LAYER — Pubky / PKARR
┌──────────────────────────────────────────────────────┐
│  Coordinator publishes PKARR record to DHT:           │
│    • .onion address                                   │
│    • Round parameters (denomination, min participants) │
│    • Software version, uptime, rounds completed        │
│  Participants resolve coordinator via PKARR lookup     │
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
│  │  • Timeouts   │  │  • RFC 9474 compliant       │   │
│  │  • Blame      │  │  • No custom crypto         │   │
│  └──────────────┘  └─────────────────────────────┘   │
│  ┌──────────────┐  ┌─────────────────────────────┐   │
│  │ Bitcoin RPC  │  │ PKARR Publisher              │   │
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

blindjoin uses a fixed-denomination CoinJoin with RSA blind signatures for unlinkability. This is the same general approach used by early Wasabi Wallet (v1) and JoinMarket, with the blind signature scheme upgraded to the IETF RFC 9474 standard.

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

## Pubky / PKARR Discovery

### Coordinator Announcement

The coordinator publishes a PKARR record to the Mainline DHT containing:

```json
{
  "type": "blindjoin-coordinator",
  "version": "0.1.0",
  "onion": "abc123...xyz.onion",
  "network": "signet",
  "denomination_sats": 1000000,
  "min_participants": 3,
  "max_participants": 20,
  "status": "idle",
  "rounds_completed": 42,
  "uptime_hours": 168,
  "updated_at": "2026-04-07T21:00:00Z"
}
```

This record is signed by the coordinator's PKARR key and resolvable by anyone querying the DHT. The coordinator refreshes this record on a heartbeat (every 5 minutes) and on state transitions (round start, round complete).

### Client Discovery

The CLI client can discover coordinators by:

1. **Direct key:** User provides a known coordinator PKARR public key
2. **Crawl:** Client queries known seed keys that maintain lists of active coordinators (a lightweight, decentralized directory)
3. **Manual .onion:** User provides a Tor .onion address directly, bypassing PKARR entirely (useful for testing)

---

## Security Model

### What the Coordinator Cannot Do

| Action | Why |
|--------|-----|
| Link inputs to outputs | RSA blind signatures (RFC 9474) make this cryptographically impossible |
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
| Flood `/round/input`, `/round/output`, or `/round/sign` past the write limit | Same, separate write-bucket `GovernorConfig` (default 30 req/min/route) | HTTP 429 with the same envelope and header |
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

### Cryptography (no custom implementations)

| Crate | Purpose | Author / Provenance |
|-------|---------|---------------------|
| `blind-rsa-signatures` | RFC 9474 RSA blind signatures | Frank Denis (jedisct1), author of libsodium |
| `bitcoin` (rust-bitcoin) | Bitcoin primitives, script, serialization | Rust Bitcoin community, widely audited |
| `bdk` (Bitcoin Dev Kit) | Wallet operations, PSBT, coin selection | Spiral (fka Square Crypto) funded |
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
| `pkarr` | PKARR DHT record publishing/resolution | Synonym (Pubky team) |

### Infrastructure

| Crate | Purpose |
|-------|---------|
| `serde` / `serde_json` | Serialization |
| `tracing` | Structured logging (round stats only, no PII) |
| `clap` | CLI argument parsing |
| `toml` | Configuration file parsing |

---

## Repository Structure

```
blindjoin/
├── Cargo.toml
├── LICENSE                        # MIT
├── README.md
├── blindjoin.toml.example             # Example configuration
│
├── coordinator/                   # Coordinator binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # Entry point: Tor setup, PKARR publish, start API
│       ├── config.rs              # Configuration: network, denomination, timeouts
│       ├── round/
│       │   ├── mod.rs
│       │   ├── state.rs           # Round state machine (FSM)
│       │   ├── input_reg.rs       # Phase 1: input registration + blind signing
│       │   ├── output_reg.rs      # Phase 2: output registration + token verification
│       │   ├── signing.rs         # Phase 3: PSBT construction + signature collection
│       │   └── blame.rs           # Phase 4: blame protocol + banning
│       ├── blind/
│       │   ├── mod.rs
│       │   └── rsa.rs             # Thin wrapper around blind-rsa-signatures crate
│       ├── bitcoin/
│       │   ├── mod.rs
│       │   ├── rpc.rs             # Bitcoin Core JSON-RPC client
│       │   ├── utxo.rs            # UTXO validation and tracking
│       │   └── tx.rs              # CoinJoin transaction construction
│       ├── network/
│       │   ├── mod.rs
│       │   ├── tor.rs             # Tor hidden service setup via arti
│       │   └── api.rs             # JSON-RPC API handlers (axum)
│       └── discovery/
│           ├── mod.rs
│           └── pkarr.rs           # PKARR record publishing + heartbeat
│
├── client/                        # CLI participant client
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # CLI entry point
│       ├── config.rs              # Client configuration
│       ├── discover.rs            # Find coordinators via PKARR or direct .onion
│       ├── wallet.rs              # Key management, UTXO selection (via bdk)
│       ├── round/
│       │   ├── mod.rs
│       │   ├── input.rs           # Phase 1: register input, blind token
│       │   ├── output.rs          # Phase 2: register output with unblinded token
│       │   └── sign.rs            # Phase 3: verify TX, sign input, submit
│       └── network/
│           ├── mod.rs
│           └── tor.rs             # Tor client connection (new circuit per phase)
│
├── liquidity-bot/                 # Auto-joining bot for testing
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # Watches for rounds, auto-joins with signet coins
│       └── strategy.rs            # Join logic: which rounds to join, when
│
├── shared/                        # Shared types between coordinator and client
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── protocol.rs            # Message types: InputRegRequest, OutputRegRequest, etc.
│       ├── types.rs               # Common types: RoundId, Denomination, ParticipantId
│       └── errors.rs              # Error types
│
├── tests/
│   ├── integration/
│   │   ├── full_round.rs          # End-to-end: 5 clients complete a CoinJoin on signet
│   │   ├── blame.rs               # Client refuses to sign, blame protocol activates
│   │   ├── sybil.rs               # Coordinator with majority sybil inputs
│   │   └── discovery.rs           # Client finds coordinator via PKARR
│   └── unit/
│       ├── blind_sig.rs           # Blind signature round-trip tests
│       ├── state_machine.rs       # FSM transition tests
│       └── tx_construction.rs     # CoinJoin TX validity tests
│
└── docker/
    ├── Dockerfile.coordinator     # Multi-stage Rust build
    ├── Dockerfile.client          # Client image
    ├── Dockerfile.bot             # Liquidity bot image
    ├── docker-compose.yml         # Full stack: bitcoind + tor + coordinator + bot
    ├── bitcoind/
    │   └── bitcoin.conf           # Signet configuration
    └── tor/
        └── torrc                  # Hidden service configuration
```

---

## Configuration

### Coordinator (`blindjoin.toml`)

```toml
[network]
bitcoin_network = "signet"             # signet | testnet4 | mainnet
bitcoin_rpc_url = "http://127.0.0.1:38332"
bitcoin_rpc_user = "blindjoin"
bitcoin_rpc_pass = "blindjoin"

[coordinator]
denomination_sats = 1_000_000          # 0.01 BTC fixed denomination
min_participants = 3                    # Minimum to start a round
max_participants = 20                   # Maximum per round
round_timeout_input_reg_secs = 60
round_timeout_output_reg_secs = 60
round_timeout_signing_secs = 30
blame_ban_duration_secs = 3600         # 1 hour ban for non-signers
fee_rate_sat_per_vbyte = 2             # Coordinator-suggested fee rate

# Public-endpoint hardening (v1.2 Phase 8); all validated at startup
rate_limit_info_per_min = 60           # GET /info, GET /round/tx; 429 + Retry-After on flood
rate_limit_writes_per_min = 30         # POST /round/input, /round/output, /round/sign
request_timeout_secs = 30              # Uniform handler deadline; HTTP 408 on stall
max_concurrent_connections = 256       # Tor accept-loop semaphore cap (tor_mode only)

[tor]
hidden_service_dir = "./tor/hidden_service"
socks_port = 9050
control_port = 9051

[discovery]
pkarr_enabled = true
heartbeat_interval_secs = 300           # Re-publish PKARR record every 5 min
seed_keys = []                          # Optional: known coordinator directory keys
```

---

## API Specification

All endpoints are served over Tor hidden service only. JSON-RPC style over HTTP POST.

### Coordinator Endpoints

#### `GET /info`
Returns coordinator status, current round state, and parameters.

```json
{
  "version": "0.1.0",
  "network": "signet",
  "denomination_sats": 1000000,
  "min_participants": 3,
  "round_state": "idle",
  "participants_registered": 0,
  "pkarr_key": "pk:abc123..."
}
```

#### `POST /round/input`
Register an input for the current round.

**Request:**
```json
{
  "utxo_outpoint": "txid:vout",
  "ownership_proof": "<signed message proving UTXO ownership>",
  "blinded_token": "<base64-encoded blinded message>"
}
```

**Response:**
```json
{
  "blind_signature": "<base64-encoded blind signature>",
  "round_id": "uuid",
  "change_address_index": 0
}
```

#### `POST /round/output`
Register an output using an unblinded token.

**Request:**
```json
{
  "unblinded_token": "<base64-encoded original message>",
  "signature": "<base64-encoded unblinded signature>",
  "output_address": "tb1q...",
  "amount_sats": 1000000
}
```

**Response:**
```json
{
  "accepted": true,
  "round_id": "uuid"
}
```

#### `POST /round/sign`
Submit a partial signature for the constructed transaction.

**Request:**
```json
{
  "round_id": "uuid",
  "input_index": 0,
  "partial_signature": "<base64-encoded signature>"
}
```

#### `GET /round/tx`
Retrieve the unsigned transaction for verification before signing.

**Response:**
```json
{
  "round_id": "uuid",
  "psbt": "<base64-encoded PSBT>",
  "fee_total_sats": 1200,
  "fee_per_participant_sats": 240
}
```

---

## Docker Compose Stack

```yaml
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

  tor:
    image: goldy/tor-hidden-service
    environment:
      COORDINATOR_PORTS: "80:8080"
    volumes:
      - tor-keys:/var/lib/tor/hidden_service
    depends_on:
      - coordinator

  coordinator:
    build:
      context: .
      dockerfile: docker/Dockerfile.coordinator
    environment:
      BLINDJOIN_BITCOIN_RPC_URL: "http://bitcoind:38332"
      BLINDJOIN_NETWORK: "signet"
    depends_on:
      - bitcoind

  liquidity-bot:
    build:
      context: .
      dockerfile: docker/Dockerfile.bot
    environment:
      BLINDJOIN_COORDINATOR_ONION: "file:///var/lib/tor/hidden_service/hostname"
      BLINDJOIN_NETWORK: "signet"
    depends_on:
      - coordinator
      - tor
    volumes:
      - tor-keys:/var/lib/tor/hidden_service:ro

volumes:
  bitcoin-data:
  tor-keys:
```

---

## Testing Strategy

### Unit Tests

| Test | Validates |
|------|-----------|
| `blind_sig::round_trip` | Blind, sign, unblind, verify cycle using blind-rsa-signatures |
| `blind_sig::unlinkability` | Two tokens from same signer cannot be correlated |
| `state_machine::transitions` | FSM only allows valid state transitions |
| `state_machine::timeouts` | Phases timeout correctly and trigger appropriate next state |
| `tx_construction::valid` | Constructed CoinJoin TX passes Bitcoin consensus validation |
| `tx_construction::equal_outputs` | All privacy outputs are exactly the denomination amount |
| `utxo::double_register` | Same UTXO cannot register twice in one round |

### Integration Tests

| Test | Validates |
|------|-----------|
| `full_round` | 5 simulated clients complete a CoinJoin on signet; TX confirms |
| `blame_non_signer` | 1 of 5 clients refuses to sign; blame protocol bans them; round restarts with 4 |
| `blame_no_output` | Client registers input but never registers output; round times out gracefully |
| `sybil_majority` | 3 of 5 participants are sybils from same operator; CoinJoin still valid (reduced anonymity set but protocol integrity maintained) |
| `discovery_pkarr` | Client discovers coordinator via PKARR DHT lookup; connects; completes round |
| `tor_circuit_isolation` | Input registration and output registration use different Tor circuits (verified via Tor control protocol) |
| `concurrent_rounds` | Two rounds execute simultaneously without state leakage |

### Adversarial Tests

| Test | Validates |
|------|-----------|
| `replay_token` | Client tries to reuse a blind signature token; coordinator rejects |
| `invalid_utxo` | Client claims a non-existent or already-spent UTXO; coordinator rejects |
| `wrong_denomination` | Client registers output with wrong amount; coordinator rejects |
| `tampered_psbt` | Coordinator sends modified PSBT; client detects and refuses to sign |

---

## Development Roadmap

### Phase 1: Foundation (Weeks 1-3)

- [ ] Cargo workspace setup with coordinator, client, shared, liquidity-bot crates
- [ ] Shared protocol message types and error types
- [ ] Round state machine (FSM) with unit tests
- [ ] Blind signature wrapper around blind-rsa-signatures with round-trip tests
- [ ] Bitcoin RPC client for signet (UTXO queries, TX broadcast)

### Phase 2: Core Protocol (Weeks 4-6)

- [ ] Input registration handler (UTXO validation + blind signing)
- [ ] Output registration handler (token verification + output recording)
- [ ] CoinJoin transaction construction (PSBT building)
- [ ] Signing phase (partial signature collection + assembly)
- [ ] Blame protocol (non-signer detection + banning)
- [ ] Coordinator API (axum JSON-RPC endpoints)

### Phase 3: Client (Weeks 7-8)

- [ ] CLI client with wallet management (bdk)
- [ ] Round participation flow (input → blind → output → sign)
- [ ] Tor circuit isolation between phases
- [ ] Transaction verification before signing

### Phase 4: Discovery & Deployment (Weeks 9-10)

- [ ] PKARR record publishing (coordinator)
- [ ] PKARR discovery (client)
- [ ] Tor hidden service setup
- [ ] Docker Compose stack (bitcoind + tor + coordinator + bot)
- [ ] Liquidity bot (auto-join rounds)

### Phase 5: Hardening (Weeks 11-12)

- [ ] Full integration test suite on signet
- [ ] Adversarial test suite
- [ ] Memory zeroing audit (round state cleanup)
- [ ] README, documentation, usage guide
- [ ] Reproducible builds (Docker multi-stage)
- [ ] GitHub release with pre-built binaries

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
