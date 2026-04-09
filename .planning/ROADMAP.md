# Roadmap: blindjoin

## Overview

blindjoin builds in five coarse phases following Approach B (Prove-Then-Layer): get a working CoinJoin transaction on signet first on clearnet, then layer in blame/hardening, a client CLI, PKARR discovery and Docker deployment, and finally the Tor hidden service and release artifacts. Each phase delivers a coherent, independently verifiable capability so protocol bugs and network bugs are never entangled.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Core Protocol** - Coordinator completes a real CoinJoin round on signet (clearnet transport) (completed 2026-04-09)
- [x] **Phase 2: Blame & Hardening** - Non-signers detected, banned, round restarts; memory zeroed; adversarial tests pass (completed 2026-04-09)
- [ ] **Phase 3: Client CLI** - End-to-end round participation from a standalone CLI tool on signet
- [ ] **Phase 4: Discovery & Deployment** - PKARR DHT discovery live; Docker Compose stack goes from zero to round in 5 minutes
- [ ] **Phase 5: Tor & Release** - Coordinator runs as Tor hidden service; pre-built binaries and Docker images published

## Phase Details

### Phase 1: Core Protocol
**Goal**: The coordinator completes a real CoinJoin round — inputs registered, outputs blinded, PSBT assembled, signed, and broadcast — on Bitcoin signet over a clearnet TCP transport
**Depends on**: Nothing (first phase)
**Requirements**: PROTO-01, PROTO-02, PROTO-03, PROTO-04, PROTO-05, PROTO-06, PROTO-07, UTXO-01, UTXO-02, UTXO-03, UTXO-04, UTXO-05, TX-01, TX-02, TX-03, TX-04, TX-05, TX-06, TX-07, TX-08, PRIV-02, PRIV-04, DEPL-05, TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06, TEST-08
**Success Criteria** (what must be TRUE):
  1. Running the coordinator binary against a signet bitcoind produces a broadcast CoinJoin transaction with a valid txid
  2. The RSA blind signature round-trip passes: coordinator cannot observe which blinded output corresponds to which input
  3. A UTXO with insufficient value, already registered, or with an invalid BIP-322 proof is rejected at registration time
  4. Unit tests for blind sig, FSM transitions, UTXO validation, TX construction, and protocol serialization all pass
  5. Coordinator emits no IP addresses, UTXOs, or input-output pairs in its logs
**Plans**: 6 plans

Plans:
- [x] 01-01-PLAN.md — Cargo workspace + shared crate (protocol types, blind token hasher, error shapes)
- [x] 01-02-PLAN.md — RSA blind signer + round FSM + HMAC session tokens + unit tests
- [x] 01-03-PLAN.md — Bitcoin RPC client + UTXO validation + BIP-322 + CoinJoin PSBT construction
- [x] 01-04-PLAN.md — HTTP handlers (5 endpoints) + coordinator config + startup health checks
- [x] 01-05-PLAN.md — Client binary (bdk_wallet, blind blinding, round participation flow)
- [x] 01-06-PLAN.md — Integration test scaffold + full unit test suite run + phase verification

### Phase 2: Blame & Hardening
**Goal**: The coordinator detects non-signers, bans their UTXOs with persistence across restarts, restarts the round with remaining participants, and wipes all round state from memory after broadcast
**Depends on**: Phase 1
**Requirements**: BLAME-01, BLAME-02, BLAME-03, BLAME-04, BLAME-05, BLAME-06, PRIV-01, TEST-06, TEST-07
**Success Criteria** (what must be TRUE):
  1. A participant that registers an input but never signs is detected by name (UTXO outpoint), banned for the configured duration, and the round restarts without them
  2. A participant that registers an input but never registers an output is detected and handled identically to a non-signer
  3. Banned UTXOs survive coordinator restart (append-only ban file) and are rejected on next registration attempt; after ban expiry they can rejoin
  4. After a round completes or blame fires, no round-state struct containing keys, tokens, or mappings remains in process memory (zeroize confirmed by unit test)
**Plans**: 3 plans

Plans:
- [x] 02-01-PLAN.md — BanList module, non-signer and missing-output detection, ban check in input registration
- [x] 02-02-PLAN.md — Ban file persistence (append-only JSONL), signing/output-reg timeout wired to blame + round restart
- [x] 02-03-PLAN.md — Signing unit tests (TEST-06), zeroize confirmation (PRIV-01), blame unit tests (TEST-07), blame integration test

### Phase 3: Client CLI
**Goal**: A user can run a single CLI binary to discover a coordinator by direct address, participate in a complete CoinJoin round using bdk_wallet, and verify the transaction before signing
**Depends on**: Phase 2
**Requirements**: CLI-02, CLI-03, CLI-04, TEST-09, TEST-10, TEST-11, TEST-12
**Success Criteria** (what must be TRUE):
  1. Given a coordinator address, the CLI registers a UTXO, receives and uses a blind token, registers an output, receives the PSBT, verifies own output is present and fee is reasonable, and submits a partial signature — all in one command invocation
  2. The integration test (3+ clients, signet) produces a confirmed transaction with a txid
  3. The integration test for blame (one non-signing client) completes with the non-signer banned and the remaining clients successfully completing a round
  4. Adversarial integration tests pass: replay token rejected, invalid UTXO rejected, wrong denomination rejected, tampered PSBT refused by client
**Plans**: TBD

### Phase 4: Discovery & Deployment
**Goal**: Coordinators are discoverable via PKARR DHT and the full stack (bitcoind + coordinator + liquidity bot) runs from a single docker compose up command
**Depends on**: Phase 3
**Requirements**: CLI-01, DISC-01, DISC-02, DISC-03, DEPL-01, DEPL-02
**Success Criteria** (what must be TRUE):
  1. A coordinator publishes its .onion address, denomination, and status to the PKARR DHT and re-publishes on state transitions and every 5 minutes
  2. A client can discover that coordinator using only a PKARR public key (no hardcoded address) and complete a round
  3. docker compose up on a fresh machine reaches a completed CoinJoin round within 5 minutes using the liquidity bot to fill the anonymity set
**Plans**: TBD

### Phase 5: Tor & Release
**Goal**: The coordinator runs as a Tor v3 hidden service with no clearnet endpoint; participants use fresh Tor circuits per phase; pre-built binaries and container images are publicly available
**Depends on**: Phase 4
**Requirements**: CLI-05, PRIV-03, DEPL-03, DEPL-04
**Success Criteria** (what must be TRUE):
  1. The coordinator binds exclusively to an arti-client-managed .onion address; no clearnet listener exists in the production configuration
  2. The client uses a distinct Tor circuit for input registration and a different circuit for output registration (verified by integration test against a logging Tor relay)
  3. GitHub Releases contains downloadable Linux and macOS binaries produced by GitHub Actions CI
  4. ghcr.io hosts a coordinator Docker image that passes a signet smoke test after docker pull
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Protocol | 6/6 | Complete    | 2026-04-09 |
| 2. Blame & Hardening | 3/3 | Complete   | 2026-04-09 |
| 3. Client CLI | 0/TBD | Not started | - |
| 4. Discovery & Deployment | 0/TBD | Not started | - |
| 5. Tor & Release | 0/TBD | Not started | - |
