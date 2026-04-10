# Requirements: blindjoin

**Defined:** 2026-04-09
**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## v1.1 Requirements

Requirements for Security & Availability Hardening milestone. Each maps to roadmap phases.

### CI/CD Security Pipeline

- [ ] **CICD-01**: CI runs `cargo test --workspace` before any build or publish
- [ ] **CICD-02**: CI runs `cargo audit` to scan dependencies for known vulnerabilities
- [ ] **CICD-03**: CI runs `cargo clippy --workspace -- -D warnings` to enforce lint quality
- [ ] **CICD-04**: All CI checks run on every pull request, not just release builds

### Coordinator Availability

- [ ] **AVAIL-01**: post_input performs async bitcoind RPC call before acquiring RoundState write lock
- [ ] **AVAIL-02**: RSA private key is parsed once at round creation; handlers reuse the parsed RsaBlindSigner

## Future Requirements

None currently deferred.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Rate limiting / abuse prevention | Separate hardening milestone |
| Prometheus/Grafana metrics | Post-v1.1, not a vulnerability |
| UTXO ban list persistence improvements | Working correctly in v1.0 |
| Protocol changes | This is hardening, not protocol evolution |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CICD-01 | — | Pending |
| CICD-02 | — | Pending |
| CICD-03 | — | Pending |
| CICD-04 | — | Pending |
| AVAIL-01 | — | Pending |
| AVAIL-02 | — | Pending |

**Coverage:**
- v1.1 requirements: 6 total
- Mapped to phases: 0
- Unmapped: 6

---
*Requirements defined: 2026-04-09*
*Last updated: 2026-04-09 after initial definition*
