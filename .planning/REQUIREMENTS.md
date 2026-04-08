# Requirements: blindjoin

**Defined:** 2026-04-07
**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Protocol Core

- [ ] **PROTO-01**: Coordinator runs CoinJoin rounds using RSA blind signatures (RFC 9474, blind-rsa-signatures crate) ensuring cryptographic input-output unlinkability
- [ ] **PROTO-02**: Round state machine (enum FSM): IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE with configurable timeouts per phase
- [ ] **PROTO-03**: Fixed-denomination equal outputs (configurable denomination, default 0.01 BTC) creating the anonymity set
- [ ] **PROTO-04**: Per-round ephemeral RSA-2048 keys with pre-commitment (coordinator publishes key hash before accepting registrations, clients verify)
- [ ] **PROTO-05**: Domain-separated blind token format: SHA-256("blindjoin-v1" || scriptPubKey || amount_sats_le64)
- [ ] **PROTO-06**: Shared protocol message types (serde structs) with forward compatibility (allow unknown fields, no deny_unknown_fields)
- [ ] **PROTO-07**: Session token issued during input registration (HMAC-based: coordinator secret + UTXO outpoint) for signing phase reconnection

### UTXO Validation

- [ ] **UTXO-01**: UTXO existence and unspent status verified via Bitcoin Core RPC (thin reqwest client, ~5 RPC methods)
- [ ] **UTXO-02**: UTXO value >= denomination + estimated fee share
- [ ] **UTXO-03**: UTXO not already registered in current round (double-registration prevention)
- [ ] **UTXO-04**: UTXO ownership proof via BIP-322 Simple verification implemented directly (~50 lines using rust-bitcoin primitives, not bip322 crate)
- [ ] **UTXO-05**: Graceful error handling when bitcoind is unreachable (retry logic + round abort)

### Blame & Banning

- [ ] **BLAME-01**: Non-signer detection: identify which UTXOs did not provide signatures after signing timeout
- [ ] **BLAME-02**: Missing output detection: identify participants who registered input but never registered output
- [ ] **BLAME-03**: Temporary UTXO ban after misbehavior (configurable duration, default 1 hour)
- [ ] **BLAME-04**: Round restart with remaining participants after blame
- [ ] **BLAME-05**: Append-only ban file persistence (UTXO hashes + timestamps, survives coordinator restarts)
- [ ] **BLAME-06**: Ban expiry: banned UTXOs can rejoin after ban duration

### Transaction Construction

- [ ] **TX-01**: CoinJoin transaction construction with all registered inputs, equal denomination outputs, change outputs, and fee allocation
- [ ] **TX-02**: Fee split equally among participants
- [ ] **TX-03**: Change outputs returned to participant's pre-registered change address (linkable to input, documented)
- [ ] **TX-04**: Dust threshold handling for change outputs (fold into fee if below dust)
- [ ] **TX-05**: PSBT construction and distribution to participants for verification
- [ ] **TX-06**: Partial signature collection keyed by UTXO outpoint (not input index)
- [ ] **TX-07**: Final transaction assembly and broadcast via bitcoind RPC
- [ ] **TX-08**: Graceful handling of bitcoind broadcast rejection

### Client CLI

- [ ] **CLI-01**: Discover coordinator via direct .onion address or PKARR DHT lookup
- [ ] **CLI-02**: Wallet management via bdk_wallet 1.0 (key management, UTXO selection, PSBT signing)
- [ ] **CLI-03**: Full round participation: input registration → blind token → output registration → verify TX → sign
- [ ] **CLI-04**: Transaction verification before signing: own output present, fee reasonable, output count matches participant count
- [ ] **CLI-05**: Fresh Tor circuit per phase (input registration circuit ≠ output registration circuit)

### Privacy & Security

- [ ] **PRIV-01**: All round state zeroed from memory after transaction broadcast (zeroize crate, ZeroizeOnDrop on all round-state structs)
- [ ] **PRIV-02**: No logging of PII, IP addresses, or input-output mappings
- [ ] **PRIV-03**: Coordinator runs as Tor hidden service via arti-client (no clearnet endpoint in production)
- [ ] **PRIV-04**: Polling GET /info at 5s intervals for phase notifications (Tor-safe, no persistent connections)

### Discovery

- [ ] **DISC-01**: Coordinator publishes PKARR record to DHT (.onion address, round parameters, RSA public key hash, status, uptime)
- [ ] **DISC-02**: Client discovers coordinators via PKARR DHT lookup or direct .onion address
- [ ] **DISC-03**: Coordinator heartbeat: re-publish PKARR record every 5 minutes and on state transitions

### Deployment

- [ ] **DEPL-01**: Docker Compose stack: bitcoind (signet) + coordinator + liquidity bot, zero to CoinJoin in 5 minutes
- [ ] **DEPL-02**: Liquidity bot: auto-joins rounds on signet for testing and cold-start
- [ ] **DEPL-03**: Pre-built Linux/macOS binaries via GitHub Releases (GitHub Actions CI)
- [ ] **DEPL-04**: Docker images published to GitHub Container Registry (ghcr.io)
- [ ] **DEPL-05**: Configurable: network (signet/testnet4/mainnet), denomination, min/max participants, timeouts, fee rate

### Testing

- [ ] **TEST-01**: Unit tests for blind signature round-trip, unlinkability, invalid key, tampered blind
- [ ] **TEST-02**: Unit tests for FSM transitions, timeouts, concurrent registration, max participants
- [ ] **TEST-03**: Unit tests for all UTXO validation paths (double reg, spent, insufficient, bad proof)
- [ ] **TEST-04**: Unit tests for output registration (replay token, wrong denomination, invalid sig, late)
- [ ] **TEST-05**: Unit tests for TX construction (valid, equal outputs, fee calc, change, dust)
- [ ] **TEST-06**: Unit tests for signing (valid sig, invalid sig, wrong outpoint)
- [ ] **TEST-07**: Unit tests for blame (non-signer, missing output, ban expiry, restart)
- [ ] **TEST-08**: Unit tests for protocol message serialization round-trip and forward compat
- [ ] **TEST-09**: Integration test: 3+ clients complete CoinJoin on signet, TX confirms
- [ ] **TEST-10**: Integration test: blame protocol (non-signer detected, banned, round restarts)
- [ ] **TEST-11**: Integration test: adversarial scenarios (replay token, invalid UTXO, wrong denomination, tampered PSBT)
- [ ] **TEST-12**: Integration test: round restart after blame + successful completion

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
| PROTO-01 | Phase 1 | Pending |
| PROTO-02 | Phase 1 | Pending |
| PROTO-03 | Phase 1 | Pending |
| PROTO-04 | Phase 1 | Pending |
| PROTO-05 | Phase 1 | Pending |
| PROTO-06 | Phase 1 | Pending |
| PROTO-07 | Phase 1 | Pending |
| UTXO-01 | Phase 1 | Pending |
| UTXO-02 | Phase 1 | Pending |
| UTXO-03 | Phase 1 | Pending |
| UTXO-04 | Phase 1 | Pending |
| UTXO-05 | Phase 1 | Pending |
| BLAME-01 | Phase 2 | Pending |
| BLAME-02 | Phase 2 | Pending |
| BLAME-03 | Phase 2 | Pending |
| BLAME-04 | Phase 2 | Pending |
| BLAME-05 | Phase 2 | Pending |
| BLAME-06 | Phase 2 | Pending |
| TX-01 | Phase 1 | Pending |
| TX-02 | Phase 1 | Pending |
| TX-03 | Phase 1 | Pending |
| TX-04 | Phase 1 | Pending |
| TX-05 | Phase 1 | Pending |
| TX-06 | Phase 1 | Pending |
| TX-07 | Phase 1 | Pending |
| TX-08 | Phase 1 | Pending |
| CLI-01 | Phase 4 | Pending |
| CLI-02 | Phase 3 | Pending |
| CLI-03 | Phase 3 | Pending |
| CLI-04 | Phase 3 | Pending |
| CLI-05 | Phase 5 | Pending |
| PRIV-01 | Phase 2 | Pending |
| PRIV-02 | Phase 1 | Pending |
| PRIV-03 | Phase 5 | Pending |
| PRIV-04 | Phase 1 | Pending |
| DISC-01 | Phase 4 | Pending |
| DISC-02 | Phase 4 | Pending |
| DISC-03 | Phase 4 | Pending |
| DEPL-01 | Phase 4 | Pending |
| DEPL-02 | Phase 4 | Pending |
| DEPL-03 | Phase 5 | Pending |
| DEPL-04 | Phase 5 | Pending |
| DEPL-05 | Phase 1 | Pending |
| TEST-01 | Phase 1 | Pending |
| TEST-02 | Phase 1 | Pending |
| TEST-03 | Phase 1 | Pending |
| TEST-04 | Phase 1 | Pending |
| TEST-05 | Phase 1 | Pending |
| TEST-06 | Phase 1 | Pending |
| TEST-07 | Phase 2 | Pending |
| TEST-08 | Phase 1 | Pending |
| TEST-09 | Phase 3 | Pending |
| TEST-10 | Phase 3 | Pending |
| TEST-11 | Phase 3 | Pending |
| TEST-12 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 52 total
- Mapped to phases: 52
- Unmapped: 0 ✓

---
*Requirements defined: 2026-04-07*
*Last updated: 2026-04-07 — traceability finalized to 5-phase coarse roadmap (DEPL-03, DEPL-04 moved Phase 6 → Phase 5; TEST-06 moved Phase 2 → Phase 1)*
