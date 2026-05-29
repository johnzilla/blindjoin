---
gsd_state_version: 1.0
milestone: v1.4
milestone_name: BIP-322 Multi-Script Support
current_plan: 2
status: executing
stopped_at: Completed 14-02-PLAN.md (Sprint-0-B PASS verdict; bdk path)
last_updated: "2026-05-29T23:45:38.761Z"
last_activity: 2026-05-29 -- Plan 14-02 complete; ready for Plan 14-03 (ADR)
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 3
  completed_plans: 2
  percent: 67
---

# Project State

## Current Position

Phase: 14 (sprint-0-spikes-discuss-phase-decisions) — EXECUTING
Plan: 3 of 3 (Plans 14-01 + 14-02 complete; Sprint-0-A GO + Sprint-0-B PASS)
Status: Ready to execute Plan 14-03 (ADR — v1.4-adr.md)
Last activity: 2026-05-29 -- Plan 14-02 complete; ready for Plan 14-03 (ADR)

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-29 after v1.3 close)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 14 — sprint-0-spikes-discuss-phase-decisions

## Progress

**Phases Complete:** 0 of 5 (v1.4 milestone)
**Plans Complete:** 2 of 3 (Phase 14)
**Current Plan:** 3

**v1.4 phase map:**

- Phase 14 — Sprint-0 Spikes + Discuss-Phase Decisions (gating; no requirements mapped; produces ADR artifact)
- Phase 15 — Shared Crate Multi-Script Contract (BIP322-01..04, ADVERT-04)
- Phase 16 — Coordinator Integration & Advertisement (ADVERT-01, ADVERT-02, ADVERT-03)
- Phase 17 — Client Multi-Script Wallet & Discovery (WALLET-01, WALLET-02, WALLET-03, WALLET-04)
- Phase 18 — Mixed-Script E2E + Liquidity Bot (INTEG-01, INTEG-02)

## Session Continuity

**Stopped At:** Completed 14-02-PLAN.md (Sprint-0-B PASS verdict; bdk path)
**Resume File:** None

## Blockers

- None at the roadmap level. Phase 14 must produce ADR resolutions for Open Decisions #1 (crate adopt vs extend), #2 (mixed vs segregated rounds), #3 (P2SH-P2WPKH wire format B1 vs B2), and #4 (bdk_wallet 2.3 P2TR sign path) before Phase 15 plan-phase can derive tasks for BIP322-02/03 and ADVERT-04.

## Performance Metrics

(v1.4 metrics will accumulate per-phase. Cumulative trends live in `RETROSPECTIVE.md`. v1.3 milestone-scoped metrics live in `milestones/v1.3-*` archives.)

| Phase | Plan | Duration | Tasks | Files | Notes |
|-------|------|----------|-------|-------|-------|
| 14 | 01 | ~5 min | 3 | 3 | Sprint-0-A cargo-tree + audit probe; verdict GO (< 2 hours, well under D-18 2-day cap) |
| 14 | 02 | ~6 min | 3 | 2 | Sprint-0-B bdk_wallet 2.3 P2TR PoC; verdict PASS / bdk path; well within D-18 2-day cap |

## Phase 14 Decisions (recorded from plan execution)

- **Plan 14-01 / Sprint-0-A → GO** (2026-05-29): bip322 v0.0.10 pulls in `bitcoin v0.32.8` at depth 1 (gate 1 PASS), `cargo audit` reports 0 vulnerabilities + 0 warnings on the spike-branch lockfile (gate 2 PASS), and the wire-shape adapter sketches at 26 LOC with zero `unwrap_or*` and zero field-shape squashing (gate 3 PASS). **ADR Decision #1 flips from default ACCEPTED-EXTEND to ACCEPTED-ADOPT** for Plan 14-03 to consume. Three new transitive crates accepted into the v1.4 dependency graph: `bip322 v0.0.10`, `snafu v0.8.9`, `snafu-derive v0.8.9` (proc-macro, compile-time only). Spike branch `spike/14-A-bip322-cargo-tree` pushed to `origin` (HEAD `9ce2ff9`) for reproducibility per D-19; NOT merged to main per D-19/D-21.

- **Plan 14-02 / Sprint-0-B → PASS** (2026-05-29): bdk_wallet 2.3 PSBT signer produces a valid 64-byte Schnorr keypath witness for a BIP-322 P2TR descriptor (BIP-86 `tr(.../86'/1'/0'/0/*)`) when `SignOptions { trust_witness_utxo: true }` is set and `witness_utxo` is populated with the canonical BIP-322 to_spend output (value = 0, script_pubkey = P2TR SPK). `wallet.sign` returned `Ok(finalized=true)`; recovered the 64-byte sig from `psbt.inputs[0].final_script_witness[0]` (bdk cleared `tap_key_sig` during finalize); `secp256k1::verify_schnorr` returned `Ok(())`. **ADR Decision #4 STATUS → ACCEPTED (bdk path)** for Plan 14-03 to consume. Phase 17 WALLET-02 uses bdk path; D-15's 80-LOC manual fallback (`shared/src/bip322/p2tr.rs::sign_p2tr_keypath`) does not fire in v1.4. Phase 17 implementation note: bdk finalizes single-key taproot, so witness extraction must check both `tap_key_sig` and `final_script_witness` (parallels client/src/wallet.rs:277-285 P2WPKH branch). Spike branch `spike/14-B-bdk-p2tr-poc` pushed to `origin` (HEAD `9ff73cd`) for reproducibility per D-19; NOT merged to main per D-19/D-21.

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
