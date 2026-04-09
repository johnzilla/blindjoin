# blindjoin

## What This Is

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet. Uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. Ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. MIT licensed. No fees. No company. No terms of service.

## Core Value

Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## Requirements

### Validated

- [x] Client uses fresh Tor circuit for each phase (input reg vs. output reg) — Validated in Phase 5: Tor & Release
- [x] Tor-native: coordinator runs as Tor hidden service via arti-client, no clearnet endpoint in production — Validated in Phase 5: Tor & Release
- [x] Pre-built binaries via GitHub Releases, Docker images via ghcr.io — Validated in Phase 5: Tor & Release (CI workflows created, awaiting first tag push)

### Active

- [ ] Coordinator runs CoinJoin rounds with RSA blind signatures (RFC 9474) ensuring unlinkability
- [ ] Fixed-denomination CoinJoin with configurable denomination (default: 0.01 BTC)
- [ ] Round state machine: IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE
- [ ] Blame protocol: non-signers detected, temporarily banned, round restarts with remaining participants
- [ ] Client CLI: discover coordinator, register input, blind token, register output, verify TX, sign
- [ ] UTXO ownership proof via BIP-322 generic message signing
- [ ] PKARR record publishing: coordinator announces .onion address, round parameters, status to DHT
- [ ] PKARR discovery: client finds coordinators via DHT lookup or direct .onion address
- [ ] Docker Compose stack: bitcoind + coordinator + liquidity bot, zero to CoinJoin in 5 minutes
- [ ] Liquidity bot: auto-joins rounds on signet for testing and cold-start
- [ ] All round state zeroed from memory after transaction broadcast
- [ ] No logging of PII, IP addresses, or input-output mappings
- [ ] Integration tests: full round (3+ participants), blame protocol, adversarial scenarios on signet

### Out of Scope

- WabiSabi variable-amount credentials — no production Rust implementation exists yet
- PayJoin mode — post-v1 protocol extension
- Cross-coordinator rounds (multi-hop cascade) — post-v1, captured as future work
- Mobile client (iOS/Android) — CLI-first for v1
- Mainnet as default — signet-first, mainnet is a config flag
- OAuth, accounts, user management — no identity layer, no accounts
- Metrics dashboard (Prometheus/Grafana) — optional post-v1

## Context

- **Gap in ecosystem:** zkSNACKs (Wasabi Wallet's coordinator) shut down CoinJoin service in June 2024. No standalone coordinator exists that anyone can run.
- **No Rust implementation:** Existing CoinJoin implementations are C# (Wasabi) and Python (JoinMarket). This is the first Rust coordinator.
- **PKARR as novel contribution:** The round protocol is battle-tested (Wasabi v1). The PKARR discovery layer making coordinators disposable and replaceable is the thesis.
- **Dependency maturity:** Arti 2.0.0 (Feb 2026) stabilized Tor hidden services in Rust. blind-rsa-signatures by jedisct1 is RFC 9474 compliant.
- **Builder background:** Owner has Rust + Bitcoin experience (arbstr-vault treasury service with Cashu ecash + Lightning).

## Constraints

- **Cryptography**: No custom crypto — blind-rsa-signatures (jedisct1), rust-bitcoin, bdk, secp256k1 only
- **Network**: Tor-native in production; development/testing may use clearnet TCP
- **Scope**: Signet-first; mainnet is a config flag, not a code change
- **Privacy**: No PII logging; round state zeroed after broadcast
- **License**: MIT — public good, not a business

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| RSA blind signatures (RFC 9474) over WabiSabi | WabiSabi lacks production Rust implementation; RSA blind sigs are proven (Wasabi v1) | — Pending |
| Approach B: Prove-Then-Layer build order | Get a txid on signet first, layer Tor/PKARR after; isolates protocol bugs from network bugs | — Pending |
| arti-client for native Tor hidden services | Arti 2.0.0 stable; avoids separate Tor process dependency; Sprint 0 PoC to verify | Validated Phase 5 |
| BIP-322 for UTXO ownership proofs | Forward compatible with all address types (P2WPKH, P2TR) | — Pending |
| bitcoincore-rpc for coordinator, bdk for client | Coordinator doesn't need wallet ops; client needs key management and PSBT signing | — Pending |
| PKARR for coordinator discovery | Decentralized; makes coordinators replaceable; no hardcoded addresses | — Pending |
| Docker Tor container optional (arti handles HS) | Coordinator uses arti-client natively; Docker Tor only needed as SOCKS proxy fallback | — Pending |

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
*Last updated: 2026-04-09 after Phase 5 (Tor & Release) completion*
