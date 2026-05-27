# Requirements: blindjoin

**Defined:** 2026-05-26
**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## v1.3 Requirements

Requirements for the **Test Infrastructure & Operational Hardening** milestone. Each maps to roadmap phases (phases 9+, continuing numbering from v1.2 Phase 8). The goal is to make the integration test feedback loop trustworthy so regressions are caught on every PR — not surfaced months later when someone runs the suite locally for the first time.

### Test Infrastructure (TEST)

- [x] **TEST-01**: CI installs a pinned `bitcoind` binary (cached between runs) so integration tests can spawn it without per-job download cost
- [x] **TEST-02**: Integration tests that require bitcoind actually execute in CI on every PR — no silent graceful-skips. The CI test count includes them.
- [ ] **TEST-03**: `cargo test` for integration tests produces output that streams to a log file (no buffering pipes) and the suite exits cleanly even if individual tests panic, without blocking on leaked child processes
- [ ] **TEST-04**: `corepc-node` test fixtures release their spawned `bitcoind` on test completion (no `Box::leak` side effect keeping the daemon alive across test boundaries)
- [ ] **TEST-05**: `CONTRIBUTING.md` documents the canonical integration-test invocation pattern (which command, where output goes, how to interpret pass/fail), so future contributors don't repeat today's pipe-buffering fight

### Test Repair (REPAIR)

- [ ] **REPAIR-01**: `tests/integration/full_round.rs` is either repaired (all 15 tests pass against the pinned bitcoind version, including the 6 currently failing on `listunspent`/RPC schema drift) **OR** explicitly retired with rationale captured in TODO.md and the file deleted from the repo
- [ ] **REPAIR-02**: Any test that uses `corepc-node`'s typed `Client` API enables the appropriate version feature explicitly (e.g. `features = ["30_2"]`), never relies on the silent `0_17_2` default

## Future Requirements

(Deferred items that may surface in v1.4+)

- **Tor-Mode Verification Harness** — deferred from v1.3 scope as too speculative for this focused milestone. Two requirements ready when scoped: (a) a Tor-mode integration harness that spawns the coordinator with `tor_mode=true` and a test client opening ≥257 concurrent `.onion` streams; (b) the connection-cap test asserting the 257th connection parks until a permit is released (closes Phase 8 HUMAN-UAT item 3, currently `result: deferred` in `08-HUMAN-UAT.md`).
- Mainnet enablement as a first-class config (v1.x signet/regtest only)
- Multiple denominations per coordinator (single-denomination simplifies blame protocol; multi-denom is a larger protocol surface)
- BIP-322 multi-script support (P2TR, P2SH-P2WPKH) — currently P2WPKH only
- Dynamic fee estimation (mempool-aware, safety margin, RBF) — currently static fee_rate_sat_per_vbyte
- GUI client — CLI-first
- Sybil resistance beyond participant minimum (e.g., stake/PoW gating)

## Out of Scope (Explicit)

- WabiSabi variable-amount credentials — no production Rust implementation
- PayJoin mode — post-v1 protocol extension
- Cross-coordinator multi-hop rounds — post-v1
- Mobile clients — CLI-first
- OAuth / accounts — no identity layer by design
- Metrics dashboards (Prometheus / Grafana) — optional, not a blocker

## Traceability

| REQ-ID | Phase | Plan | Status |
|--------|-------|------|--------|
| TEST-01 | Phase 9 | TBD | active |
| TEST-02 | Phase 9 | TBD | active |
| TEST-03 | Phase 9 | TBD | active |
| TEST-04 | Phase 9 | TBD | active |
| TEST-05 | Phase 9 | TBD | active |
| REPAIR-01 | Phase 10 | TBD | active |
| REPAIR-02 | Phase 10 | TBD | active |
