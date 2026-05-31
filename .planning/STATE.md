---
gsd_state_version: 1.0
milestone: null
milestone_name: null
current_plan: null
status: between_milestones
stopped_at: v1.4 milestone shipped 2026-05-31; v1.5 not yet scoped
last_updated: 2026-05-31
last_activity: 2026-05-31
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Current Position

Milestone: Between milestones (v1.4 shipped 2026-05-31)
Status: Ready for `/gsd:new-milestone` to scope v1.5

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-31 after v1.4 ship)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Planning v1.5

## Shipped Milestones

- ✅ v1.0 MVP (2026-04-09) — Phases 1-5
- ✅ v1.1 Security & Availability Hardening (2026-04-10) — Phases 6-7
- ✅ v1.2 Production Readiness (2026-05-26) — Phase 8
- ✅ v1.3 Test Infrastructure & Operational Hardening (2026-05-29) — Phases 9-13
- ✅ v1.4 BIP-322 Multi-Script Support (2026-05-31) — Phases 14-18

## Blockers

None at the roadmap level. Run `/gsd:new-milestone` to scope v1.5.

## Carry-Forward Items (v1.5+ candidates)

- **CARRY-TOR-UAT** — Tor-mode verification harness (Phase 8 HUMAN-UAT item 3)
- **CARRY-REPAIR-01-PR** — v1.3 REPAIR-01 PR observation closure (the v1.4 cut PR is the natural moment)
- **B-03** — Dynamic fee estimation (mempool-aware polling + RBF strategy)
- **TEST-EXT-01/02/03** — Cross-implementation differential fixtures, on-chain anchor test, automated v1.3↔v1.4 backwards-compat integration matrix
- **P2WSH multisig BIP-322 support** (v1.4 stretch dropped for scope discipline)
- **Mixed output script types** (Wasabi 2.0.3-style per-participant output choice)
- **14 deferred clippy lints** in `shared/src/bip322/*` (12× `clippy::result_large_err` + 2× `clippy::unnecessary_to_owned`) — see `.planning/phases/16-coordinator-integration-advertisement/deferred-items.md`

## Performance Metrics

Cumulative trends live in `RETROSPECTIVE.md`. Per-milestone metrics archived under `.planning/milestones/v{X.Y}-*`.
