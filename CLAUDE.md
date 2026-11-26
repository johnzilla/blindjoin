## Project

**blindjoin**

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet. Uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. Ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. MIT licensed. No fees. No company. No terms of service.

**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

### Constraints

- **Cryptography**: No custom crypto — blind-rsa-signatures (jedisct1), rust-bitcoin, bdk, secp256k1 only
- **Network**: Tor-native in production; development/testing may use clearnet TCP
- **Scope**: Signet-first; mainnet is a config flag, not a code change
- **Privacy**: No PII logging; round state zeroed after broadcast
- **License**: MIT — public good, not a business

## Technology Stack

Rust workspace, `tokio` async. The authoritative dependency list is the workspace [`Cargo.toml`](Cargo.toml) and each crate's manifest; the module map is in [ARCHITECTURE.md](ARCHITECTURE.md). The load-bearing choices:

| Area | Crate(s) | Notes |
|---|---|---|
| Async runtime | `tokio` 1.51 | channels, timers, `RwLock<RoundState>`, phase-deadline timeouts |
| Bitcoin primitives | `bitcoin` 0.32 | transactions, scripts, PSBT, addresses |
| Client wallet | `bdk_wallet` 2.3 | descriptor keys, UTXO selection, signing — **client only** (the coordinator holds no Bitcoin keys) |
| Bitcoin Core RPC | `corepc-types` 0.11 + `reqwest` 0.13 | async JSON-RPC; the replacement for the archived `bitcoincore-rpc` |
| Blind signatures | `blind-rsa-signatures` 0.17 (jedisct1) | RFC 9474 — the unlinkability primitive; no alternative, no custom crypto |
| Ownership proofs | `bip322` `=0.0.11` | BIP-322; exact-pinned (pre-1.0) and CI-gated |
| Tor | `arti-client` 0.41 + `tor-hsservice` 0.41 | in-process onion service (coordinator) + isolated per-phase circuits (client); no separate `tor` daemon |
| HTTP API | `axum` 0.8 + `tower` 0.5 | coordinator endpoints + per-route rate limiting |
| Discovery | `pkarr` 5 | signed coordinator record on the Mainline DHT |
| Serialization | `serde` / `serde_json` 1 | wire types |
| Crypto helpers | `hmac` 0.13, `sha2` 0.11, `zeroize` 1.8 | session tokens, hashing, round-state zeroization |
| Config | `config` 0.15 | layered TOML + env |
| Client CLI | `clap` 4 | subcommands |
| Observability | `tracing` 0.1 + `tracing-subscriber` 0.3 | structured, PII-free fields |
| Testing | `corepc-node` 0.12 (regtest Bitcoin Core), `proptest` 1 | integration + property tests |

**No database:** the UTXO ban list is an append-only, `fsync`'d JSONL file (`coordinator/src/round/blame.rs`); round state is in-memory and zeroed at round end.

## Conventions

See [ARCHITECTURE.md § Conventions](ARCHITECTURE.md#conventions) — no custom crypto, PII discipline in logs, ephemeral round state, additive wire evolution, exact pins for fragile deps, `file::symbol` anchors, size-matched process.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the crate/module map, round data flow, and the invariants ledger (each invariant → where it is enforced → the test that pins it). Normative wire behavior is in [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
