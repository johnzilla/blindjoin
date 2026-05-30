# blindjoin

## What This Is

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet. Uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. Ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. MIT licensed. No fees. No company. No terms of service.

## Core Value

Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## Current State

v1.4 Phase 14 (Sprint-0 Spikes + Discuss-Phase Decisions) complete 2026-05-30 — gating ADR phase. Two timeboxed spikes on quarantined branches resolved all 4 v1.4 Open Decisions: Decision #1 ACCEPTED ADOPT `bip322 = "=0.0.10"` (transitive `bitcoin v0.32.8` at depth 1, `cargo audit` clean, 26-LOC adapter sketch with zero lossy conversions); Decision #2 ACCEPTED mixed rounds; Decision #3 ACCEPTED B2 PSBT-input wire shape with `version: u8` envelope; Decision #4 ACCEPTED bdk path (bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness in `psbt.inputs[0].final_script_witness[0]` and verifies via `secp256k1::verify_schnorr` against the BIP-341 keyspend sighash). v1.4 ADR ratified at `.planning/decisions/v1.4-adr.md`. D-21 structural invariant held: zero production-code commits on main from Phase 14 — spike branches `spike/14-A-bip322-cargo-tree` and `spike/14-B-bdk-p2tr-poc` pushed to origin for reproducibility and never merged. Phase 15 (Shared Crate Multi-Script Contract) unblocked.

v1.3 Test Infrastructure & Operational Hardening shipped 2026-05-29 (5 phases, 13 plans, 4 days). Made the integration test feedback loop trustworthy: CI now installs a pinned bitcoind v30.2 (`actions/cache@v4` + PGP-verified install), the entire `tests/integration/` tree has zero `Box::leak` (clean process lifecycle via `BitcoindGuard` RAII + `require_bitcoind!()` macro), `CONTRIBUTING.md` documents the canonical invocation, and any test using corepc-node's typed `Client` must declare an explicit `features = ["NN_M"]` (CI grep gate). REPAIR-01 closed locally — all 8 `full_round::*` tests green on pinned brew bitcoind v31 via a chain of direct fixes (RSA SPKI handshake, bdk_wallet 2.3 `trust_witness_utxo`, wire-format `Witness` consensus encoding, real on-chain `witness_utxo` values, ban-check ordering, error-body surfacing). Full PR observation closure pending v1.4 cut.

Shipped to date: v1.0 MVP → v1.1 Security & Availability → v1.2 Production Readiness → v1.3 Test Infrastructure (10 phases total across 4 milestones; 6,490 Rust LOC across coordinator/client/shared).
Tech stack: axum 0.8, arti-client 0.41, blind-rsa-signatures, bdk_wallet 2.3, pkarr, tokio, tower_governor 0.8, tower-http 0.6, corepc-node 0.12 (features pinned), bitcoind v30.2.
Coordinator runs as Tor v3 hidden service. Client uses per-phase isolated Tor circuits.
PKARR DHT discovery live. Docker Compose stack operational.
CI/CD: PR-triggered test/clippy/audit gates, release and Docker workflows gated on check jobs, actions SHA-pinned, bitcoind cached + PGP-verified, corepc-node feature pin enforced.
Coordinator hardened: RPC outside write lock, RSA signer cached per-round, address pre-validation, blinded token bounds, public-endpoint DoS resistance, real on-chain witness_utxo values in PSBT assembly.

## Current Milestone: v1.4 BIP-322 Multi-Script Support

**Goal:** Broaden CoinJoin participation to P2TR and P2SH-P2WPKH UTXOs, eliminating the P2WPKH-only registration gate and making the "forward-compatible with all address types" claim match reality.

**Target features:**
- Adopt the official `bip322` crate (rust-bitcoin org) in place of `shared/src/bip322.rs` — discuss-phase verifies crate version stability before commit; fallback is to extend the custom impl
- Remove the `is_p2wpkh()` hard gate at [coordinator/src/bitcoin/utxo.rs:119](coordinator/src/bitcoin/utxo.rs:119)
- Add P2TR (BIP-86 single-key Taproot) and P2SH-P2WPKH support across client signing, wire types, and coordinator verification
- Coordinator advertises supported script types via PKARR record + `/round/info` so clients reject mismatched coordinators before registration
- Liquidity bot updated to generate test UTXOs across all supported script types
- Per-script-type property tests over BIP-322 spec vectors
- End-to-end integration test: full CoinJoin round with mixed P2WPKH + P2TR + P2SH-P2WPKH participants on regtest

**Out of v1.4 scope (deferred to v1.5+):**
- Tor-mode verification harness (Phase 8 HUMAN-UAT item 3 carry-forward)
- REPAIR-01 PR observation closure (currently closed-local only)
- P2WSH multisig (multi-key sighash complexity — stretch goal)
- B-03 dynamic fee estimation

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

- ✓ CI installs a pinned `bitcoind` binary (cached, PGP+SHA256-verified) so integration tests can spawn it without per-job download cost — v1.3 Phase 9 (TEST-01)
- ✓ Integration tests that require bitcoind actually execute in CI on every PR — no silent graceful-skips — v1.3 Phase 9 (TEST-02)
- ✓ `cargo test` output streams cleanly (no buffering pipes) and exits on test panic without blocking on leaked child processes — v1.3 Phase 9 (TEST-03)
- ✓ `corepc-node` test fixtures release their spawned `bitcoind` on test end (zero `Box::leak` across `tests/integration/`) — v1.3 Phase 9 (TEST-04)
- ✓ `CONTRIBUTING.md` documents the canonical integration-test invocation pattern — v1.3 Phase 9 (TEST-05)
- ✓ `full_round.rs` repaired — all 8 tests pass against pinned bitcoind v31 via direct code commits (RSA SPKI handshake, bdk_wallet 2.3 `trust_witness_utxo`, wire-format `Witness` consensus encoding, real on-chain `witness_utxo`, ban-check ordering, error-body surfacing) — v1.3 Phases 10-13 (REPAIR-01 closed-local; full PR observation pending v1.4 cut)
- ✓ Any test using corepc-node's typed `Client` declares an explicit version feature (CI grep gate enforces) — v1.3 Phase 10 (REPAIR-02)

### Active

v1.4 BIP-322 Multi-Script Support (requirements detailed in `.planning/REQUIREMENTS.md`):

- [ ] Adopt or pin a production-viable BIP-322 Simple implementation covering P2WPKH, P2TR, P2SH-P2WPKH
- [ ] Remove coordinator P2WPKH-only registration gate; verify ownership proofs for all supported script types
- [ ] Coordinator publishes supported script types via PKARR + `/round/info`; clients reject mismatched coordinators pre-registration
- [ ] Client signs ownership proofs for P2TR and P2SH-P2WPKH UTXOs alongside existing P2WPKH path
- [ ] Liquidity bot generates test UTXOs across all supported script types
- [ ] End-to-end integration test: full CoinJoin round with mixed-script-type participants on regtest
- [ ] Per-script-type property tests against BIP-322 spec vectors

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
- **v1.2 shipped:** Public-endpoint DoS resistance via tower_governor + tower-http (per-route rate limits, request timeouts, Tor accept-loop semaphore cap), 4 operator-tunable config knobs validated at startup, release clearnet refusal.
- **v1.3 shipped:** Trustworthy integration-test feedback loop — pinned bitcoind v30.2 in CI (cached + PGP-verified), `BitcoindGuard` RAII + `require_bitcoind!()` macro eliminate `Box::leak` and the pipe-buffer stdout-hang, `CONTRIBUTING.md` documents the canonical invocation, corepc-node feature pin enforced by CI grep gate, REPAIR-01 closed locally (all 8 `full_round::*` tests green) via direct fixes for RSA SPKI handshake, bdk_wallet 2.3 segwit signing, partial-sig wire format, and coordinator witness_utxo correctness.

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
| Pin bitcoind v30.2 in CI via `.bitcoind-version` + actions/cache + PGP-verified install | Defeats keyserver flake and a hostile main HEAD; cache-then-verify-on-miss preserves the integrity gate even with cache hits | ✓ Good — TEST-01/02 closed, CI install cost amortized |
| `BitcoindGuard` RAII + `require_bitcoind!()` macro over Box::leak + skip blocks | Macro returns from the calling test scope (a plain fn cannot); RAII guarantees graceful `node.stop()` then process.kill() fallback | ✓ Good — `tests/integration/` tree has zero Box::leak and zero inline skip blocks |
| `view_stdout=false` + `-printtoconsole=0` belt-and-suspenders | view_stdout=false is corepc-node 0.12 default; explicit guards a future default flip silently re-introducing the pipe-hang | ✓ Good — root cause cannot resurface silently |
| corepc-node feature pin enforced by CI grep gate | Silent dependence on the 0_17_2 default would mean tests target the wrong Bitcoin Core RPC surface | ✓ Good — REPAIR-02 closed via 4026f50 |
| Close REPAIR-01 via direct commits rather than re-executing halted Plans 11-13 | After 6 orthogonal blockers and 3 escape-valve halts, Plan.md execution had ceased to be the load-bearing path; direct bisectable commits delivered REPAIR-01 with full forensic audit trail preserved | ⚠ Revisit — closed-local but full PR observation pending; preserve the forensic trace as a reference for "when to abandon the structured path" |
| client signs PSBT with `SignOptions { trust_witness_utxo: true }` (bdk_wallet 2.3) | Client populates witness_utxo from its own trusted regtest RPC; full BIP-143 non_witness_utxo path was heavier and unnecessary given trust boundary | ✓ Good — narrowly scoped fix with D-08 block comment explaining safety boundary |
| Partial-sig wire format = consensus-serialized `bitcoin::Witness` (not raw DER) | Coordinator deserializes via `bitcoin::Witness` consensus encoding; raw DER on the wire produced silent HTTP 400 | ✓ Good — wire-format mismatch was the root of the 6th orthogonal blocker |
| Coordinator uses real on-chain UTXO values in PSBT `witness_utxo` (not zero placeholders) | bdk_wallet sighash computation requires the actual amount; zero placeholders produced invalid signatures | ✓ Good — closed the last input-amount source of REPAIR-01 failures |

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
*Last updated: 2026-05-30 after v1.4 Phase 14 (Sprint-0 Spikes + Discuss-Phase Decisions) complete*
