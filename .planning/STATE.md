---
gsd_state_version: 1.0
milestone: v1.4
milestone_name: BIP-322 Multi-Script Support
current_plan: 1
status: executing
stopped_at: Phase 14 context gathered
last_updated: "2026-05-29T23:25:05.754Z"
last_activity: 2026-05-29 -- Phase 14 execution started
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 3
  completed_plans: 0
  percent: 0
---

# Project State

## Current Position

Phase: 14 (sprint-0-spikes-discuss-phase-decisions) — EXECUTING
Plan: 1 of 3
Status: Executing Phase 14
Last activity: 2026-05-29 -- Phase 14 execution started

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-29 after v1.3 close)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 14 — sprint-0-spikes-discuss-phase-decisions

## Progress

**Phases Complete:** 0 of 5 (v1.4 milestone)
**Current Plan:** 1

**v1.4 phase map:**

- Phase 14 — Sprint-0 Spikes + Discuss-Phase Decisions (gating; no requirements mapped; produces ADR artifact)
- Phase 15 — Shared Crate Multi-Script Contract (BIP322-01..04, ADVERT-04)
- Phase 16 — Coordinator Integration & Advertisement (ADVERT-01, ADVERT-02, ADVERT-03)
- Phase 17 — Client Multi-Script Wallet & Discovery (WALLET-01, WALLET-02, WALLET-03, WALLET-04)
- Phase 18 — Mixed-Script E2E + Liquidity Bot (INTEG-01, INTEG-02)

## Session Continuity

**Stopped At:** Phase 14 context gathered
**Resume File:** .planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md

## Blockers

- None at the roadmap level. Phase 14 must produce ADR resolutions for Open Decisions #1 (crate adopt vs extend), #2 (mixed vs segregated rounds), #3 (P2SH-P2WPKH wire format B1 vs B2), and #4 (bdk_wallet 2.3 P2TR sign path) before Phase 15 plan-phase can derive tasks for BIP322-02/03 and ADVERT-04.

## Performance Metrics

(v1.4 metrics will accumulate per-phase. Cumulative trends live in `RETROSPECTIVE.md`. v1.3 milestone-scoped metrics live in `milestones/v1.3-*` archives.)

## Accumulated Context

### Carry-forward constraints from v1.3 REPAIR-01 forensics

1. **Wire-format roundtrip test ships FIRST** — any wire-format change (Phase 15 `OwnershipProof` extension) must have a `shared/` roundtrip serialization test passing BEFORE coordinator or client uses the new shape.
2. **bdk_wallet 2.3 segwit signing requires `SignOptions { trust_witness_utxo: true }`** and real on-chain `witness_utxo` values — do not retry zero placeholders for P2SH-P2WPKH.
3. **Exact-pin every dependency** referenced in test fixtures; CI-enforce (bdk_wallet, corepc-node feature pin, and `bip322` crate if adopted).
4. **If 2-3 carry-forward plans appear with the same shape, abandon Plan.md and pivot to `/gsd:debug`** — the structured path has ceased to be load-bearing.
5. **REPAIR-01 PR observation closure is v1.5, not v1.4** — the v1.4 cut PR is the natural moment to discharge it but is NOT a v1.4 code deliverable.

### Cross-phase invariant

v1.3 P2WPKH-only `full_round::*` integration tests must remain green at EVERY v1.4 phase boundary. This is the rollback safety net encoded in every phase's success criteria.

### Open Decisions for Phase 14 discuss-phase

- **#1**: Adopt `bip322` crate vs extend custom `shared/src/bip322.rs` (changes Phase 15 effort estimate 3-5x; gated by Sprint-0-A `cargo tree` check).
- **#2**: Mixed vs segregated script-type rounds (changes PKARR record schema and coordinator round-state machine if overturned in favor of segregated).
- **#3**: P2SH-P2WPKH wire format — B1 tagged enum vs B2 base64 PSBT-input shape.
- **#4**: bdk_wallet 2.3 multi-descriptor sign path (resolved by Sprint-0-B PoC).

### Deferred to v1.5+ (NOT v1.4 scope)

- CARRY-TOR-UAT (Tor-mode verification harness, Phase 8 HUMAN-UAT item 3)
- CARRY-REPAIR-01-PR (REPAIR-01 PR observation closure)
- B-03 (dynamic fee estimation)
- TEST-EXT-01/02/03 (cross-impl differential fixtures, on-chain anchor test, automated compat matrix)
