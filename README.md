# blindjoin

A standalone CoinJoin coordinator and client for Bitcoin. Uses RSA blind signatures (RFC 9474) so the coordinator cryptographically cannot link transaction inputs to outputs. MIT licensed. No fees. No company.

**Status:** Phase 1 complete (core protocol). Coordinator and client compile and pass 38 tests. Integration test scaffolded for regtest. Signet/mainnet rounds require a running bitcoind.

## What This Does

1. Coordinator announces a fixed-denomination CoinJoin round (default: 0.01 BTC)
2. Participants register inputs and receive blind-signed tokens
3. Participants register outputs using unblinded tokens (on a fresh connection)
4. Coordinator builds the transaction, participants verify and sign
5. Coordinator broadcasts the final CoinJoin transaction

The blind signature scheme (RFC 9474) makes it cryptographically impossible for the coordinator to determine which input produced which output. Each round uses ephemeral RSA keys that are destroyed after broadcast.

## Build

Requires Rust 1.75+ and cargo.

```bash
cargo build --workspace
cargo test --workspace
```

## Run the Coordinator

Requires a running Bitcoin Core node (signet, testnet, or regtest).

```bash
# Copy and edit config
cp blindjoin.toml.example blindjoin.toml

# Start coordinator
cargo run -p coordinator
```

The coordinator listens on `0.0.0.0:8080` by default. All settings can be overridden with `BLINDJOIN_*` environment variables.

### Configuration

See `blindjoin.toml.example` for all options:

- `network.bitcoin_network` — signet (default), testnet4, regtest, mainnet
- `network.bitcoin_rpc_url` — Bitcoin Core RPC endpoint
- `coordinator.denomination_sats` — fixed output amount (default: 1,000,000 sats)
- `coordinator.min_participants` — minimum to start a round (default: 3)
- `coordinator.listen_addr` — HTTP listen address (default: 0.0.0.0:8080)

### API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/info` | Coordinator status, round state, RSA public key |
| POST | `/round/input` | Register a UTXO input + receive blind signature |
| POST | `/round/output` | Register an output using unblinded token |
| GET | `/round/tx` | Retrieve unsigned PSBT for verification |
| POST | `/round/sign` | Submit partial signature |

Errors return structured JSON: `{"error": {"code": "UTXO_SPENT", "message": "...", "round_id": "..."}}`.

## Run the Client

```bash
cargo run -p client -- --coordinator-url http://127.0.0.1:8080 \
  --wif <your-private-key-wif> \
  --output-address <destination-address>
```

The client handles the full round participation flow: input registration, token blinding, output registration, transaction verification, and signing.

## Project Structure

```
blindjoin/
  coordinator/     # CoinJoin coordinator binary
    src/
      api/         # HTTP handlers (axum)
      bitcoin/     # RPC client, UTXO validation, BIP-322, PSBT builder
      blind/       # RSA blind signature engine
      round/       # State machine, input/output registration, signing, blame
      config.rs    # TOML + env var configuration
      main.rs      # Startup, health checks, server
  client/          # CLI participant client
    src/
      round/       # Input registration, output registration, signing
      wallet.rs    # Key management, BIP-322 proofs, PSBT signing
      http.rs      # Coordinator HTTP client
      config.rs    # CLI argument parsing
  shared/          # Protocol types shared between coordinator and client
    src/
      protocol.rs  # Wire message structs (serde, forward-compatible)
      token.rs     # Blind token message computation (domain-separated SHA-256)
      bip322.rs    # BIP-322 Simple message signing primitives
      errors.rs    # Structured error codes
      types.rs     # Common types (RoundId, Denomination)
  tests/
    integration/   # End-to-end CoinJoin round tests
```

## Security Model

The coordinator **cannot**:
- Link inputs to outputs (RSA blind signatures, RFC 9474)
- Steal funds (participants sign their own inputs)
- Reconstruct round data after completion (all state zeroed from memory)

The coordinator **can**:
- Refuse to complete rounds (participants detect and switch coordinators)
- Register sybil inputs (fixed minimum participant count dilutes impact)
- See which UTXOs registered (observable on-chain anyway)

Session tokens use HMAC with constant-time comparison. BIP-322 ownership proofs verified for all inputs. No PII logging.

## Roadmap

- [x] **Phase 1: Core Protocol** — Round state machine, blind signatures, UTXO validation, PSBT construction, HTTP API, client CLI (38 tests)
- [ ] **Phase 2: Blame & Hardening** — Non-signer detection, ban persistence, memory zeroing audit
- [ ] **Phase 3: Client CLI** — End-to-end integration tests on signet
- [ ] **Phase 4: Discovery & Deployment** — PKARR DHT coordinator discovery, Docker Compose
- [ ] **Phase 5: Tor & Release** — Tor hidden service via arti, pre-built binaries

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `blind-rsa-signatures` | RFC 9474 RSA blind signatures (jedisct1) |
| `bitcoin` (rust-bitcoin) | Bitcoin primitives, PSBT, scripts |
| `axum` | HTTP framework for coordinator API |
| `tokio` | Async runtime |
| `zeroize` | Memory zeroing for sensitive round state |
| `subtle` | Constant-time comparison for session tokens |

## License

MIT
