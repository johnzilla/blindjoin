# blindjoin

## What This Is

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet. Uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. Ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. MIT licensed. No fees. No company. No terms of service.

## Core Value

Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## Current State

v1.2 Production Readiness shipped (1 phase, Phase 8 "Public-endpoint hardening"). v1.3 Test Infrastructure & Operational Hardening started 2026-05-26 — focused on making CI catch integration regressions automatically. Phase 8 delivered DoS resistance on the coordinator HTTP API: per-route rate limits (HTTP 429 + Retry-After, RATE_LIMITED JSON envelope), uniform request timeouts (HTTP 408), and Tor accept-loop connection cap (tokio::sync::Semaphore + ConnectionGuard). 4 operator-tunable knobs in `coordinator.toml`; release builds refuse clearnet without `BLINDJOIN_ALLOW_CLEARNET=1`.

Shipped v1.1 Security & Availability Hardening (5,918 LOC Rust, 8 phases total across 3 milestones to date); v1.3 in progress.
Tech stack: axum 0.8, arti-client 0.41, blind-rsa-signatures, bdk_wallet 2.3, pkarr, tokio, tower_governor 0.8, tower-http 0.6.
Coordinator runs as Tor v3 hidden service. Client uses per-phase isolated Tor circuits.
PKARR DHT discovery live. Docker Compose stack operational.
CI/CD: PR-triggered test/clippy/audit gates, release and Docker workflows gated on check jobs, actions SHA-pinned.
Coordinator hardened: RPC outside write lock, RSA signer cached per-round, address pre-validation, blinded token bounds, public-endpoint DoS resistance.

## Requirements

### Validated

- ✓ Coordinator runs CoinJoin rounds with RSA blind signatures (RFC 9474) ensuring unlinkability — v1.0
- ✓ Fixed-denomination CoinJoin with configurable denomination (default: 0.01 BTC) — v1.0
- ✓ Round state machine: IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE — v1.0
- ✓ Blame protocol: non-signers detected, temporarily banned, round restarts with remaining participants — v1.0
- ✓ Client CLI: discover coordinator, register input, blind token, register output, verify TX, sign — v1.0
- ✓ Client uses fresh Tor circuit for each phase (input reg vs. output reg) — v1.0
- ✓ UTXO ownership proof via BIP-322 generic message signing — v1.0
- ✓ PKARR record publishing: coordinator announces .onion address, round parameters, status to DHT — v1.0
- ✓ PKARR discovery: client finds coordinators via DHT lookup or direct .onion address — v1.0
- ✓ Tor-native: coordinator runs as Tor hidden service via arti-client, no clearnet endpoint in production — v1.0
- ✓ Docker Compose stack: bitcoind + coordinator + liquidity bot, zero to CoinJoin in 5 minutes — v1.0
- ✓ Liquidity bot: auto-joins rounds on signet for testing and cold-start — v1.0
- ✓ All round state zeroed from memory after transaction broadcast — v1.0
- ✓ No logging of PII, IP addresses, or input-output mappings — v1.0
- ✓ Integration tests: full round (3+ participants), blame protocol, adversarial scenarios on signet — v1.0
- ✓ Pre-built binaries via GitHub Releases, Docker images via ghcr.io — v1.0

- ✓ CI/CD security pipeline: cargo test, cargo audit, cargo clippy as gates on PRs and releases — v1.1
- ✓ Eliminate write-lock DoS: move async RPC call outside RoundState write lock in post_input — v1.1
- ✓ Eliminate key deserialization DoS: parse RSA key once at round creation, reuse across requests — v1.1

- ✓ Per-route rate limiting on coordinator HTTP API (read split 60/min, write split 30/min) with HTTP 429 + Retry-After + RATE_LIMITED JSON envelope — v1.2 Phase 8
- ✓ Uniform request timeout (HTTP 408) honoring `request_timeout_secs` — v1.2 Phase 8
- ✓ Tor accept-loop connection cap via `tokio::sync::Semaphore` (default 256) with ConnectionGuard RAII — v1.2 Phase 8
- ✓ Operator-tunable knobs via `coordinator.toml` and `BLINDJOIN__COORDINATOR__*` env vars; validated at startup — v1.2 Phase 8
- ✓ GlobalKeyExtractor for rate limiting (Tor-safe; PeerIpKeyExtractor would break under Tor) — v1.2 Phase 8

### Active

## Current Milestone: v1.3 Test Infrastructure & Operational Hardening

**Goal:** Move the project from "tests compile" to "tests actually run end-to-end in CI" so integration regressions are caught on every PR — not surfaced months later when someone runs them locally for the first time.

**Target features:**
- CI installs bitcoind and the integration suite actually executes (no more silent graceful-skips)
- Cargo-test stdout-hang pattern eliminated (no more leaked-bitcoind pipe blocks; output streams to file)
- `full_round.rs` either repaired (corepc-node v30 client API port) or retired (delete if unit coverage suffices)
- Tor-mode integration harness for connection-cap test (closes Phase 8 HUMAN-UAT item 3)

**Key context:** Phase 8 surfaced multiple structural problems with the test infrastructure that had been silently rotting — tests that always graceful-skipped in CI, `cargo test` stdout pipes blocked behind leaked bitcoind processes, hardcoded test WIFs that were never valid, RPC schema drift in `full_round.rs`. The milestone exists to stop the fire-fighting cycle by making the test feedback loop trustworthy. Scope is deliberately narrow: no new product features, no protocol changes; just durable test infrastructure.

(Requirements list will be populated in the next workflow step.)

### Out of Scope

- WabiSabi variable-amount credentials — no production Rust implementation exists yet
- PayJoin mode — post-v1 protocol extension
- Cross-coordinator rounds (multi-hop cascade) — post-v1, captured as future work
- Mobile client (iOS/Android) — CLI-first for v1
- Mainnet as default — signet-first, mainnet is a config flag
- OAuth, accounts, user management — no identity layer, no accounts
- Metrics dashboard (Prometheus/Grafana) — optional post-v1
- Offline mode — real-time coordination is fundamental to the protocol

## Context

- **Gap in ecosystem:** zkSNACKs (Wasabi Wallet's coordinator) shut down CoinJoin service in June 2024. No standalone coordinator exists that anyone can run.
- **No Rust implementation:** Existing CoinJoin implementations are C# (Wasabi) and Python (JoinMarket). This is the first Rust coordinator.
- **PKARR as novel contribution:** The round protocol is battle-tested (Wasabi v1). The PKARR discovery layer making coordinators disposable and replaceable is the thesis.
- **Dependency maturity:** Arti 2.0.0 (Feb 2026) stabilized Tor hidden services in Rust. blind-rsa-signatures by jedisct1 is RFC 9474 compliant.
- **Builder background:** Owner has Rust + Bitcoin experience (arbstr-vault treasury service with Cashu ecash + Lightning).
- **v1.0 shipped:** Full protocol working on signet with Tor, PKARR discovery, Docker deployment, and CI/CD.
- **v1.1 shipped:** CI/CD security gates (test/clippy/audit on PRs and releases), coordinator DoS hardening (RPC before lock, RSA caching), supply-chain hardening (SHA-pinned actions, release checksums).

## Constraints

- **Cryptography**: No custom crypto — blind-rsa-signatures (jedisct1), rust-bitcoin, bdk, secp256k1 only
- **Network**: Tor-native in production; development/testing may use clearnet TCP
- **Scope**: Signet-first; mainnet is a config flag, not a code change
- **Privacy**: No PII logging; round state zeroed after broadcast
- **License**: MIT — public good, not a business

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| RSA blind signatures (RFC 9474) over WabiSabi | WabiSabi lacks production Rust implementation; RSA blind sigs are proven (Wasabi v1) | ✓ Good — delivered full unlinkability |
| Approach B: Prove-Then-Layer build order | Get a txid on signet first, layer Tor/PKARR after; isolates protocol bugs from network bugs | ✓ Good — clean 5-phase progression |
| arti-client for native Tor hidden services | Arti 2.0.0 stable; avoids separate Tor process dependency | ✓ Good — working v3 .onion service |
| BIP-322 for UTXO ownership proofs | Forward compatible with all address types (P2WPKH, P2TR) | ✓ Good — Simple verification working |
| corepc-types over bitcoincore-rpc | bitcoincore-rpc archived Nov 2025; thin reqwest RPC client | ✓ Good — 5 RPC methods, no dep issues |
| bdk_wallet for client key management | Descriptor-based wallet, PSBT signing, HD derivation | ✓ Good — clean integration |
| PKARR for coordinator discovery | Decentralized; makes coordinators replaceable; no hardcoded addresses | ✓ Good — DHT publish + resolve working |
| In-process SOCKS5 proxy for client Tor | arti-client 0.41 has no launch_socks5_listener(); minimal RFC 1928 bridge | ⚠ Revisit — works but adds ~80 LOC |
| Docker Compose with cargo-chef builds | Multi-stage Dockerfiles, separate images per binary | ✓ Good — clean separation |
| Validate-then-lock for post_input | RPC before write lock eliminates DoS from slow bitcoind | ✓ Good — AVAIL-01 verified |
| Cache RsaBlindSigner in RoundStateInner | Parse RSA key once per round, not per request | ✓ Good — AVAIL-02 verified |
| SHA-pin all GitHub Actions | Immutable commit SHAs prevent supply-chain tampering | ✓ Good — CR-01 resolved |
| cargo audit deny high+critical only | Low/medium advisories are informational, not blockers | ✓ Good — reduces false-positive friction |
| tower_governor 0.8 + tower-http TimeoutLayer (over rolling your own) | Audited tower middleware; saved DoS-window correctness from scratch | ✓ Good — Phase 8 shipped clean |
| GlobalKeyExtractor over PeerIpKeyExtractor | Tor-safe by design; per-peer throttling impossible on Tor (single IP) | ✓ Good — would have been a CRITICAL bug |
| Validate hardening knobs at startup (`config.validate()`) | Fail-fast at boot beats panic-at-first-request or silent deadlock | ✓ Good — CR-01 & CR-02 fixed pre-ship |
| ConnectionGuard RAII for Tor permits | Load-bearing `let _permit = permit;` is one careless cleanup away from disabling the cap | ✓ Good — WR-01 hardened |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-26 — v1.3 Test Infrastructure & Operational Hardening started*
