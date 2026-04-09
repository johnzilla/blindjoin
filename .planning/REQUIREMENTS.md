# Requirements: blindjoin

**Defined:** 2026-04-07
**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Protocol Core

- [x] **PROTO-01**: Coordinator runs CoinJoin rounds using RSA blind signatures (RFC 9474, blind-rsa-signatures crate) ensuring cryptographic input-output unlinkability
- [x] **PROTO-02**: Round state machine (enum FSM): IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE with configurable timeouts per phase
- [x] **PROTO-03**: Fixed-denomination equal outputs (configurable denomination, default 0.01 BTC) creating the anonymity set
- [x] **PROTO-04**: Per-round ephemeral RSA-2048 keys with pre-commitment (coordinator publishes key hash before accepting registrations, clients verify)
- [x] **PROTO-05**: Domain-separated blind token format: SHA-256("blindjoin-v1" || scriptPubKey || amount_sats_le64)
- [x] **PROTO-06**: Shared protocol message types (serde structs) with forward compatibility (allow unknown fields, no deny_unknown_fields)
- [x] **PROTO-07**: Session token issued during input registration (HMAC-based: coordinator secret + UTXO outpoint) for signing phase reconnection

### UTXO Validation

- [x] **UTXO-01**: UTXO existence and unspent status verified via Bitcoin Core RPC (thin reqwest client, ~5 RPC methods)
- [x] **UTXO-02**: UTXO value >= denomination + estimated fee share
- [x] **UTXO-03**: UTXO not already registered in current round (double-registration prevention)
- [x] **UTXO-04**: UTXO ownership proof via BIP-322 Simple verification implemented directly (~50 lines using rust-bitcoin primitives, not bip322 crate)
- [x] **UTXO-05**: Graceful error handling when bitcoind is unreachable (retry logic + round abort)

### Blame & Banning

- [x] **BLAME-01**: Non-signer detection: identify which UTXOs did not provide signatures after signing timeout
- [x] **BLAME-02**: Missing output detection: identify participants who registered input but never registered output
- [x] **BLAME-03**: Temporary UTXO ban after misbehavior (configurable duration, default 1 hour)
- [x] **BLAME-04**: Round restart with remaining participants after blame
- [x] **BLAME-05**: Append-only ban file persistence (UTXO hashes + timestamps, survives coordinator restarts)
- [x] **BLAME-06**: Ban expiry: banned UTXOs can rejoin after ban duration

### Transaction Construction

- [x] **TX-01**: CoinJoin transaction construction with all registered inputs, equal denomination outputs, change outputs, and fee allocation
- [x] **TX-02**: Fee split equally among participants
- [x] **TX-03**: Change outputs returned to participant's pre-registered change address (linkable to input, documented)
- [x] **TX-04**: Dust threshold handling for change outputs (fold into fee if below dust)
- [x] **TX-05**: PSBT construction and distribution to participants for verification
- [x] **TX-06**: Partial signature collection keyed by UTXO outpoint (not input index)
- [x] **TX-07**: Final transaction assembly and broadcast via bitcoind RPC
- [x] **TX-08**: Graceful handling of bitcoind broadcast rejection

### Client CLI

- [x] **CLI-01**: Discover coordinator via direct .onion address or PKARR DHT lookup
- [x] **CLI-02**: Wallet management via bdk_wallet 1.0 (key management, UTXO selection, PSBT signing)
- [x] **CLI-03**: Full round participation: input registration → blind token → output registration → verify TX → sign
- [x] **CLI-04**: Transaction verification before signing: own output present, fee reasonable, output count matches participant count
- [ ] **CLI-05**: Fresh Tor circuit per phase (input registration circuit ≠ output registration circuit)

### Privacy & Security

- [x] **PRIV-01**: All round state zeroed from memory after transaction broadcast (zeroize crate, ZeroizeOnDrop on all round-state structs)
- [x] **PRIV-02**: No logging of PII, IP addresses, or input-output mappings
- [ ] **PRIV-03**: Coordinator runs as Tor hidden service via arti-client (no clearnet endpoint in production)
- [x] **PRIV-04**: Polling GET /info at 5s intervals for phase notifications (Tor-safe, no persistent connections)

### Discovery

- [x] **DISC-01**: Coordinator publishes PKARR record to DHT (.onion address, round parameters, RSA public key hash, status, uptime)
- [x] **DISC-02**: Client discovers coordinators via PKARR DHT lookup or direct .onion address
- [x] **DISC-03**: Coordinator heartbeat: re-publish PKARR record every 5 minutes and on state transitions

### Deployment

- [ ] **DEPL-01**: Docker Compose stack: bitcoind (signet) + coordinator + liquidity bot, zero to CoinJoin in 5 minutes
- [ ] **DEPL-02**: Liquidity bot: auto-joins rounds on signet for testing and cold-start
- [ ] **DEPL-03**: Pre-built Linux/macOS binaries via GitHub Releases (GitHub Actions CI)
- [ ] **DEPL-04**: Docker images published to GitHub Container Registry (ghcr.io)
- [x] **DEPL-05**: Configurable: network (signet/testnet4/mainnet), denomination, min/max participants, timeouts, fee rate

### Testing

- [x] **TEST-01**: Unit tests for blind signature round-trip, unlinkability, invalid key, tampered blind
- [x] **TEST-02**: Unit tests for FSM transitions, timeouts, concurrent registration, max participants
- [x] **TEST-03**: Unit tests for all UTXO validation paths (double reg, spent, insufficient, bad proof)
- [x] **TEST-04**: Unit tests for output registration (replay token, wrong denomination, invalid sig, late)
- [x] **TEST-05**: Unit tests for TX construction (valid, equal outputs, fee calc, change, dust)
- [x] **TEST-06**: Unit tests for signing (valid sig, invalid sig, wrong outpoint)
- [x] **TEST-07**: Unit tests for blame (non-signer, missing output, ban expiry, restart)
- [x] **TEST-08**: Unit tests for protocol message serialization round-trip and forward compat
- [x] **TEST-09**: Integration test: 3+ clients complete CoinJoin on signet, TX confirms
- [x] **TEST-10**: Integration test: blame protocol (non-signer detected, banned, round restarts)
- [x] **TEST-11**: Integration test: adversarial scenarios (replay token, invalid UTXO, wrong denomination, tampered PSBT)
- [x] **TEST-12**: Integration test: round restart after blame + successful completion

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Protocol Extensions

- **WABI-01**: WabiSabi variable-amount credentials (when Rust implementation available)
- **PAYJOIN-01**: PayJoin mode: outputs look like normal 2-in/2-out transactions
- **CASCADE-01**: Cross-coordinator multi-hop CoinJoin cascade via PKARR discovery
- **CONCURRENT-01**: Concurrent rounds: multiple rounds simultaneously without state leakage

### Client Extensions

- **MOBILE-01**: iOS/Android client using same protocol
- **FEE-01**: Dynamic fee estimation from mempool (required for mainnet)

### Infrastructure

- **METRICS-01**: Optional Prometheus/Grafana metrics dashboard (aggregate stats only)
- **FAUCET-01**: Signet faucet integration for first-time client setup
- **EXPLORER-01**: Block explorer link after round completion

## Out of Scope

| Feature | Reason |
|---------|--------|
| Token/coin-based incentives | Bitcoin-only, no token economics |
| User accounts / identity | No identity layer, coordinator is stateless |
| Web UI | CLI-first, no frontend |
| Mainnet as default | Signet-first for safety; mainnet is config flag |
| Custom cryptography | All crypto uses audited crates; no rolling own |
| Persistent participant data | Privacy by design; only ban list (UTXO hashes) persists |
| OAuth / SSO | No accounts, no auth |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PROTO-01 | Phase 1 | Complete |
| PROTO-02 | Phase 1 | Complete |
| PROTO-03 | Phase 1 | Complete |
| PROTO-04 | Phase 1 | Complete |
| PROTO-05 | Phase 1 | Complete |
| PROTO-06 | Phase 1 | Complete |
| PROTO-07 | Phase 1 | Complete |
| UTXO-01 | Phase 1 | Complete |
| UTXO-02 | Phase 1 | Complete |
| UTXO-03 | Phase 1 | Complete |
| UTXO-04 | Phase 1 | Complete |
| UTXO-05 | Phase 1 | Complete |
| BLAME-01 | Phase 2 | Complete |
| BLAME-02 | Phase 2 | Complete |
| BLAME-03 | Phase 2 | Complete |
| BLAME-04 | Phase 2 | Complete |
| BLAME-05 | Phase 2 | Complete |
| BLAME-06 | Phase 2 | Complete |
| TX-01 | Phase 1 | Complete |
| TX-02 | Phase 1 | Complete |
| TX-03 | Phase 1 | Complete |
| TX-04 | Phase 1 | Complete |
| TX-05 | Phase 1 | Complete |
| TX-06 | Phase 1 | Complete |
| TX-07 | Phase 1 | Complete |
| TX-08 | Phase 1 | Complete |
| CLI-01 | Phase 4 | Complete |
| CLI-02 | Phase 3 | Complete |
| CLI-03 | Phase 3 | Complete |
| CLI-04 | Phase 3 | Complete |
| CLI-05 | Phase 5 | Pending |
| PRIV-01 | Phase 2 | Complete |
| PRIV-02 | Phase 1 | Complete |
| PRIV-03 | Phase 5 | Pending |
| PRIV-04 | Phase 1 | Complete |
| DISC-01 | Phase 4 | Complete |
| DISC-02 | Phase 4 | Complete |
| DISC-03 | Phase 4 | Complete |
| DEPL-01 | Phase 4 | Pending |
| DEPL-02 | Phase 4 | Pending |
| DEPL-03 | Phase 5 | Pending |
| DEPL-04 | Phase 5 | Pending |
| DEPL-05 | Phase 1 | Complete |
| TEST-01 | Phase 1 | Complete |
| TEST-02 | Phase 1 | Complete |
| TEST-03 | Phase 1 | Complete |
| TEST-04 | Phase 1 | Complete |
| TEST-05 | Phase 1 | Complete |
| TEST-06 | Phase 1 | Complete |
| TEST-07 | Phase 2 | Complete |
| TEST-08 | Phase 1 | Complete |
| TEST-09 | Phase 3 | Complete |
| TEST-10 | Phase 3 | Complete |
| TEST-11 | Phase 3 | Complete |
| TEST-12 | Phase 3 | Complete |

**Coverage:**
- v1 requirements: 52 total
- Mapped to phases: 52
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-07*
*Last updated: 2026-04-07 — traceability finalized to 5-phase coarse roadmap (DEPL-03, DEPL-04 moved Phase 6 → Phase 5; TEST-06 moved Phase 2 → Phase 1)*
