# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- ✅ **v1.1 Security & Availability Hardening** — Phases 6-7 (shipped 2026-04-10)
- ✅ **v1.2 Production Readiness** — Phase 8 (shipped 2026-05-26)
- ✅ **v1.3 Test Infrastructure & Operational Hardening** — Phases 9-13 (shipped 2026-05-29)
- ✅ **v1.4 BIP-322 Multi-Script Support** — Phases 14-18 (shipped 2026-05-31)
- ✅ **v1.5 Audit-Readiness & Multi-Script Finish** — Phases 19-21 (shipped 2026-06-01)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-04-09</summary>

- [x] Phase 1: Core Protocol (6/6 plans) — completed 2026-04-09
- [x] Phase 2: Blame & Hardening (3/3 plans) — completed 2026-04-09
- [x] Phase 3: Client CLI (2/2 plans) — completed 2026-04-09
- [x] Phase 4: Discovery & Deployment (3/3 plans) — completed 2026-04-09
- [x] Phase 5: Tor & Release (3/3 plans) — completed 2026-04-09

</details>

<details>
<summary>✅ v1.1 Security & Availability Hardening (Phases 6-7) — SHIPPED 2026-04-10</summary>

- [x] Phase 6: CI/CD Security Pipeline (1/1 plans) — completed 2026-04-10
- [x] Phase 7: Coordinator DoS Hardening (3/3 plans) — completed 2026-04-10

</details>

<details>
<summary>✅ v1.2 Production Readiness (Phase 8) — SHIPPED 2026-05-26</summary>

- [x] Phase 8: Public-endpoint hardening (4/4 plans) — completed 2026-05-26

</details>

<details>
<summary>✅ v1.3 Test Infrastructure & Operational Hardening (Phases 9-13) — SHIPPED 2026-05-29</summary>

- [x] Phase 9: CI integration-test reliability (5/5 plans) — completed 2026-05-27
- [x] Phase 10: full_round.rs decision + execution (2/2 plans; Task 3 carry-forward) — completed 2026-05-28
- [x] Phase 11: RSA SPKI handshake + unmute (carry-forward from 10) — closed via direct commits 2026-05-28
- [x] Phase 12: bdk_wallet 2.3 trust_witness_utxo (carry-forward from 11) — closed via direct commits 2026-05-28
- [x] Phase 13: Wire-format Witness encoding + unmute (carry-forward from 12) — closed via direct commits 2026-05-29

</details>

<details>
<summary>✅ v1.4 BIP-322 Multi-Script Support (Phases 14-18) — SHIPPED 2026-05-31</summary>

- [x] Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions (3/3 plans) — completed 2026-05-29
- [x] Phase 15: Shared Crate Multi-Script Contract (3/3 plans) — completed 2026-05-30
- [x] Phase 16: Coordinator Integration & Advertisement (3/3 plans) — completed 2026-05-30
- [x] Phase 17: Client Multi-Script Wallet & Discovery (3/3 plans) — completed 2026-05-30
- [x] Phase 18: Mixed-Script E2E + Liquidity Bot (3/3 plans) — completed 2026-05-31

</details>

<details>
<summary>✅ v1.5 Audit-Readiness & Multi-Script Finish (Phases 19-21) — SHIPPED 2026-06-01</summary>

- [x] Phase 19: Multi-Script Signing Finish (2/2 plans) — completed 2026-05-31
- [x] Phase 20: Mixed-Round Fee Accuracy (1/1 plan) — completed 2026-05-31
- [x] Phase 21: Audit Charter & Zeroization Tightening (2/2 plans) — completed 2026-05-31

</details>

### 📋 v1.6+ Planned

No v1.6 phases defined yet. Carry-forward items (deferred from v1.5) live in `.planning/PROJECT.md` §Carry-Forward Items. Start v1.6 with `/gsd:new-milestone`.

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Protocol | v1.0 | 6/6 | Complete | 2026-04-09 |
| 2. Blame & Hardening | v1.0 | 3/3 | Complete | 2026-04-09 |
| 3. Client CLI | v1.0 | 2/2 | Complete | 2026-04-09 |
| 4. Discovery & Deployment | v1.0 | 3/3 | Complete | 2026-04-09 |
| 5. Tor & Release | v1.0 | 3/3 | Complete | 2026-04-09 |
| 6. CI/CD Security Pipeline | v1.1 | 1/1 | Complete | 2026-04-10 |
| 7. Coordinator DoS Hardening | v1.1 | 3/3 | Complete | 2026-04-10 |
| 8. Public-endpoint hardening | v1.2 | 4/4 | Complete | 2026-05-26 |
| 9. CI integration-test reliability | v1.3 | 5/5 | Complete | 2026-05-27 |
| 10. full_round.rs decision + execution | v1.3 | 2/2 | Complete | 2026-05-28 |
| 11-13. REPAIR-01 carry-forward (shipped as direct commits) | v1.3 | n/a | Closed-local | 2026-05-29 |
| 14. Sprint-0 Spikes + Discuss-Phase Decisions | v1.4 | 3/3 | Complete | 2026-05-29 |
| 15. Shared Crate Multi-Script Contract | v1.4 | 3/3 | Complete | 2026-05-30 |
| 16. Coordinator Integration & Advertisement | v1.4 | 3/3 | Complete | 2026-05-30 |
| 17. Client Multi-Script Wallet & Discovery | v1.4 | 3/3 | Complete | 2026-05-30 |
| 18. Mixed-Script E2E + Liquidity Bot | v1.4 | 3/3 | Complete | 2026-05-31 |
| 19. Multi-Script Signing Finish | v1.5 | 2/2 | Complete | 2026-05-31 |
| 20. Mixed-Round Fee Accuracy | v1.5 | 1/1 | Complete | 2026-05-31 |
| 21. Audit Charter & Zeroization Tightening | v1.5 | 2/2 | Complete | 2026-05-31 |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
Full v1.1 details: `.planning/milestones/v1.1-ROADMAP.md`
Full v1.2 details: `.planning/milestones/v1.2-ROADMAP.md`
Full v1.3 details: `.planning/milestones/v1.3-ROADMAP.md`
Full v1.4 details: `.planning/milestones/v1.4-ROADMAP.md`
Full v1.5 details: `.planning/milestones/v1.5-ROADMAP.md`
