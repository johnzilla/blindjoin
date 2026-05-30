---
gsd_state_version: 1.0
milestone: v1.4
milestone_name: BIP-322 Multi-Script Support
current_plan: Not started
status: planning
stopped_at: Phase 15 context gathered
last_updated: "2026-05-30T00:35:51.594Z"
last_activity: 2026-05-30
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 3
  completed_plans: 3
  percent: 20
---

# Project State

## Current Position

Phase: 15
Plan: none — Phase 15 awaiting `/gsd:plan-phase 15`
Status: Ready to plan
Last activity: 2026-05-30

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-29 after v1.3 close)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 15 — shared crate multi script contract

## Progress

**Phases Complete:** 1 of 5 (v1.4 milestone)
**Plans Complete:** 3 of 3 (Phase 14)
**Current Plan:** Not started

**v1.4 phase map:**

- ✅ Phase 14 — Sprint-0 Spikes + Discuss-Phase Decisions (closed 2026-05-29; ADR ratified)
- Phase 15 — Shared Crate Multi-Script Contract (BIP322-01..04, ADVERT-04)
- Phase 16 — Coordinator Integration & Advertisement (ADVERT-01, ADVERT-02, ADVERT-03)
- Phase 17 — Client Multi-Script Wallet & Discovery (WALLET-01, WALLET-02, WALLET-03, WALLET-04)
- Phase 18 — Mixed-Script E2E + Liquidity Bot (INTEG-01, INTEG-02)

## Session Continuity

**Stopped At:** Phase 15 context gathered
**Resume File:** .planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md

## Blockers

- None at the roadmap level. Phase 14 must produce ADR resolutions for Open Decisions #1 (crate adopt vs extend), #2 (mixed vs segregated rounds), #3 (P2SH-P2WPKH wire format B1 vs B2), and #4 (bdk_wallet 2.3 P2TR sign path) before Phase 15 plan-phase can derive tasks for BIP322-02/03 and ADVERT-04.

## Performance Metrics

(v1.4 metrics will accumulate per-phase. Cumulative trends live in `RETROSPECTIVE.md`. v1.3 milestone-scoped metrics live in `milestones/v1.3-*` archives.)

| Phase | Plan | Duration | Tasks | Files | Notes |
|-------|------|----------|-------|-------|-------|
| 14 | 01 | ~5 min | 3 | 3 | Sprint-0-A cargo-tree + audit probe; verdict GO (< 2 hours, well under D-18 2-day cap) |
| 14 | 02 | ~6 min | 3 | 2 | Sprint-0-B bdk_wallet 2.3 P2TR PoC; verdict PASS / bdk path; well within D-18 2-day cap |
| 14 | 03 | ~10 min | 3 | 3 | v1.4 ADR ratification (4 decisions, Michael Nygard template); separate doc commit per CD-5; D-21 invariant holds |

## Phase 14 Decisions (recorded from plan execution)

- **Plan 14-01 / Sprint-0-A → GO** (2026-05-29): bip322 v0.0.10 pulls in `bitcoin v0.32.8` at depth 1 (gate 1 PASS), `cargo audit` reports 0 vulnerabilities + 0 warnings on the spike-branch lockfile (gate 2 PASS), and the wire-shape adapter sketches at 26 LOC with zero `unwrap_or*` and zero field-shape squashing (gate 3 PASS). **ADR Decision #1 flips from default ACCEPTED-EXTEND to ACCEPTED-ADOPT** for Plan 14-03 to consume. Three new transitive crates accepted into the v1.4 dependency graph: `bip322 v0.0.10`, `snafu v0.8.9`, `snafu-derive v0.8.9` (proc-macro, compile-time only). Spike branch `spike/14-A-bip322-cargo-tree` pushed to `origin` (HEAD `9ce2ff9`) for reproducibility per D-19; NOT merged to main per D-19/D-21.

- **Plan 14-02 / Sprint-0-B → PASS** (2026-05-29): bdk_wallet 2.3 PSBT signer produces a valid 64-byte Schnorr keypath witness for a BIP-322 P2TR descriptor (BIP-86 `tr(.../86'/1'/0'/0/*)`) when `SignOptions { trust_witness_utxo: true }` is set and `witness_utxo` is populated with the canonical BIP-322 to_spend output (value = 0, script_pubkey = P2TR SPK). `wallet.sign` returned `Ok(finalized=true)`; recovered the 64-byte sig from `psbt.inputs[0].final_script_witness[0]` (bdk cleared `tap_key_sig` during finalize); `secp256k1::verify_schnorr` returned `Ok(())`. **ADR Decision #4 STATUS → ACCEPTED (bdk path)** for Plan 14-03 to consume. Phase 17 WALLET-02 uses bdk path; D-15's 80-LOC manual fallback (`shared/src/bip322/p2tr.rs::sign_p2tr_keypath`) does not fire in v1.4. Phase 17 implementation note: bdk finalizes single-key taproot, so witness extraction must check both `tap_key_sig` and `final_script_witness` (parallels client/src/wallet.rs:277-285 P2WPKH branch). Spike branch `spike/14-B-bdk-p2tr-poc` pushed to `origin` (HEAD `9ff73cd`) for reproducibility per D-19; NOT merged to main per D-19/D-21.

- **Plan 14-03 / ADR ratification → COMPLETE** (2026-05-29): v1.4 ADR at `.planning/decisions/v1.4-adr.md` ratifies all 4 Open Decisions per the Michael Nygard template (D-20): Decision #1 = ACCEPTED (ADOPT `bip322 = "=0.0.10"`) per Sprint-0-A GO; Decision #2 = ACCEPTED (mixed rounds, single output type per round, no per-script-type minimum-participant gate) per D-06..D-10; Decision #3 = ACCEPTED (B2 base64 PSBT-input shape with `version: u8` envelope) per D-11..D-13; Decision #4 = ACCEPTED (bdk path) per Sprint-0-B PASS. D-21 structural invariant verified empty at the ADR commit boundary — Phase 14 produces zero production-code commits (`git diff main -- coordinator/ client/ shared/ liquidity-bot/` empty). ADR committed alone per CD-5 (`58f477e`); STATE/ROADMAP flip in separate doc commit. Phase 15-17 planners read the ADR by anchor `#decision-1`, `#decision-2`, `#decision-3`, `#decision-4` for task derivation without re-litigating.

## Accumulated Context

### Phase 14 close (2026-05-29)

- v1.4 ADR ratified at `.planning/decisions/v1.4-adr.md` — resolves Open Decisions #1 (ADOPT `bip322 = "=0.0.10"` per Sprint-0-A GO), #2 (MIXED rounds, single output type per round, no per-script-type min-participants gate), #3 (B2 base64 PSBT-input wire format with `version: u8` envelope), #4 (bdk path for P2TR sign per Sprint-0-B PASS).
- Sprint-0-A verdict (sprint-0-A.md:199): `GO: all three D-02 gates PASS — bip322 v0.0.10 pulls in bitcoin v0.32.8 (gate 1), cargo audit clean (gate 2), adapter 26 LOC zero-lossy (gate 3).`
- Sprint-0-B verdict (sprint-0-B.md:315): `PASS: bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness for a BIP-322 to_sign PSBT under a BIP-86 descriptor; secp256k1::verify_schnorr returned Ok(()).`
- Structural D-21 invariant verified: `git diff main -- coordinator/ client/ shared/ liquidity-bot/` is empty at the ADR commit boundary. Phase 14 produced zero production-code commits.
- Spike branches (`spike/14-A-bip322-cargo-tree` HEAD `9ce2ff9`, `spike/14-B-bdk-p2tr-poc` HEAD `9ff73cd`) pushed to `origin` for reproducibility per D-19; NOT merged to main.
- Phase 15 planner reads ADR by anchor `#decision-1`, `#decision-3` for BIP322-01..04 + ADVERT-04 task derivation; Phase 16 reads `#decision-2` for ADVERT-01..03 + CRIT-01; Phase 17 reads `#decision-4` for WALLET-02 sign-path implementation note.
- Phase 17 implementation note (carried forward from Sprint-0-B): bdk_wallet 2.3 finalizes single-key taproot keyspend into `psbt.inputs[0].final_script_witness[0]` (64-byte witness element), NOT `tap_key_sig`. WALLET-02 witness extraction must check both fields and prefer whichever bdk populated (parallels existing P2WPKH fallback at `client/src/wallet.rs:277-285`).
- D-15 manual fallback (`shared/src/bip322/p2tr.rs::sign_p2tr_keypath`, 80-LOC budget) retired for v1.4; stays on the books as a v1.5 swap target if bdk_wallet ever regresses on taproot keyspend.

### Carry-forward constraints from v1.3 REPAIR-01 forensics

1. **Wire-format roundtrip test ships FIRST** — any wire-format change (Phase 15 `OwnershipProof` extension) must have a `shared/` roundtrip serialization test passing BEFORE coordinator or client uses the new shape.
2. **bdk_wallet 2.3 segwit signing requires `SignOptions { trust_witness_utxo: true }`** and real on-chain `witness_utxo` values — do not retry zero placeholders for P2SH-P2WPKH.
3. **Exact-pin every dependency** referenced in test fixtures; CI-enforce (bdk_wallet, corepc-node feature pin, and `bip322` crate if adopted).
4. **If 2-3 carry-forward plans appear with the same shape, abandon Plan.md and pivot to `/gsd:debug`** — the structured path has ceased to be load-bearing.
5. **REPAIR-01 PR observation closure is v1.5, not v1.4** — the v1.4 cut PR is the natural moment to discharge it but is NOT a v1.4 code deliverable.

### Cross-phase invariant

v1.3 P2WPKH-only `full_round::*` integration tests must remain green at EVERY v1.4 phase boundary. This is the rollback safety net encoded in every phase's success criteria.

### Open Decisions for Phase 14 discuss-phase — ALL RESOLVED 2026-05-29

All 4 Open Decisions are resolved in `.planning/decisions/v1.4-adr.md`. Downstream phases read the ADR by anchor and derive tasks without re-litigating.

- **#1 RESOLVED → ACCEPTED (ADOPT `bip322 = "=0.0.10"`)** — Sprint-0-A returned GO across all 3 D-02 gates. Phase 15 lands the 26-LOC adapter; three new transitive crates accepted (`bip322`, `snafu`, `snafu-derive`). ADR §`#decision-1`.
- **#2 RESOLVED → ACCEPTED (mixed rounds)** — One queue accepts P2WPKH + P2TR + P2SH-P2WPKH inputs together; outputs remain single-script-type per round (operator-configured `[bip] output_script_type` in `coordinator.toml`); no per-script-type minimum-participant gate; coordinator advertises supported set only (not per-round counts). Heterogeneous-input chain-analysis fingerprint documented as known limitation (Phase 18 README copy). ADR §`#decision-2`.
- **#3 RESOLVED → ACCEPTED (B2 base64 PSBT-input shape, `version: u8` envelope)** — v1.4 `OwnershipProof` carries `psbt_input_b64: String` + `version: u8` (1 = v1.3 shape, 2 = v1.4 PSBT shape). Phase 15 ships wire-format roundtrip test FIRST per v1.3 REPAIR-01 lesson #1. ADR §`#decision-3`.
- **#4 RESOLVED → ACCEPTED (bdk path)** — Sprint-0-B returned PASS; bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness for BIP-322 P2TR PSBTs. Phase 17 WALLET-02 uses bdk path; D-15 manual fallback retired for v1.4. ADR §`#decision-4`.

### Deferred to v1.5+ (NOT v1.4 scope)

- CARRY-TOR-UAT (Tor-mode verification harness, Phase 8 HUMAN-UAT item 3)
- CARRY-REPAIR-01-PR (REPAIR-01 PR observation closure)
- B-03 (dynamic fee estimation)
- TEST-EXT-01/02/03 (cross-impl differential fixtures, on-chain anchor test, automated compat matrix)
