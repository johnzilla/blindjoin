---
phase: 14-sprint-0-spikes-discuss-phase-decisions
plan: 03
subsystem: adr-and-closeout
tags: [adr, decisions, phase-closeout, michael-nygard, d-20, d-21, cd-5]

# Dependency graph
requires:
  - phase: 14
    plan: 01
    provides: "Sprint-0-A GO verdict (sprint-0-A.md:199); drives ADR Decision #1 STATUS deterministically"
  - phase: 14
    plan: 02
    provides: "Sprint-0-B PASS verdict + 'bdk path' recommendation (sprint-0-B.md:315, :323); drives ADR Decision #4 STATUS deterministically"
provides:
  - "Canonical v1.4 ADR at `.planning/decisions/v1.4-adr.md` resolving all 4 Open Decisions per Michael Nygard template (D-20)"
  - "Phase 14 closeout: STATE.md and ROADMAP.md flipped to Phase 14 Complete (3/3 plans) in a separate doc commit per CD-5"
  - "Phase 15 input contract: read ADR by anchor `#decision-1` (crate adopt-vs-extend resolved), `#decision-3` (B2 PSBT-input wire format) for BIP322-01..04 + ADVERT-04 task derivation"
  - "Phase 16 input contract: read ADR by anchor `#decision-2` (mixed rounds, single output type per round, CRIT-01 cross-check D-10) for ADVERT-01..03 task derivation"
  - "Phase 17 input contract: read ADR by anchor `#decision-4` (bdk path) for WALLET-02 sign-path implementation; Phase 17 inherits bdk-finalisation note (witness lands in `final_script_witness[0]`, not `tap_key_sig`)"
affects:
  - 15-shared-crate-multi-script-contract
  - 16-coordinator-integration-and-advertisement
  - 17-client-multi-script-wallet-and-discovery
  - 18-mixed-script-e2e-and-liquidity-bot

# Tech tracking
tech-stack:
  added: []  # Phase 14 produces zero production-code commits per D-21 structural invariant
  patterns:
    - "Michael Nygard per-decision ADR template (D-20): Status / Context / Decision / Consequences (positive/negative/neutral) / Rejected Alternatives — extensible if v1.5 adds Decision #5+"
    - "Deterministic verdict→STATUS mapping pattern: grep verdict line at column 0 of canonical research record; map via case/match to ADR STATUS line; no human re-interpretation in the ADR write step"
    - "CD-5 separate-doc-commit policy: ADR commit alone first, then STATE.md/ROADMAP.md flip in a follow-up doc commit (matches `commit_docs: true` config + v1.3 phase closure precedent)"
    - "Section-scoped grep verification (awk range + grep) prevents skeleton-revision regressions that file-wide greps would miss (T-14-12, T-14-13 mitigations)"

key-files:
  created:
    - ".planning/decisions/v1.4-adr.md (209 lines, 4 Decision sections + Spike Outputs section; all 5 Michael Nygard subsections per decision; section-scoped grep targets BIP-322 signing in #1 and chain-analysis fingerprint in #2 satisfied)"
    - ".planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-03-SUMMARY.md (this file)"
  modified:
    - ".planning/STATE.md (frontmatter: completed_phases 0→1, completed_plans 2→3, percent 67→20; Current Position flipped to Phase 14 COMPLETE; Open Decisions section flipped from 'for discuss-phase' to RESOLVED with ADR anchor references; Phase 14 close subsection appended to Accumulated Context)"
    - ".planning/ROADMAP.md (v1.4 section bullet flipped [ ]→[x] with summary of resolved decisions; Phase Details plans subsection [ ]→[x] for 14-03; Progress table row 14 flipped 2/3 In Progress → 3/3 Complete 2026-05-29)"

key-decisions:
  - "ADR Decision #1 STATUS = ACCEPTED (ADOPT `bip322 = \"=0.0.10\"`) — deterministic flip from default ACCEPTED (EXTEND) because Sprint-0-A returned `GO:` across all 3 D-02 gates"
  - "ADR Decision #2 STATUS = ACCEPTED (mixed rounds) — per 14-CONTEXT.md D-06..D-10; chain-analysis fingerprint surfaced candidly in Consequences/negative per CD-3 (README phrasing deferred to Phase 18)"
  - "ADR Decision #3 STATUS = ACCEPTED (B2 base64 PSBT-input shape with `version: u8` envelope) — per D-11..D-13; Phase 15 ships wire-format roundtrip test FIRST per v1.3 REPAIR-01 lesson #1"
  - "ADR Decision #4 STATUS = ACCEPTED (bdk path) — deterministic flip because Sprint-0-B returned `PASS:` and recommendation `bdk path`; D-15 manual fallback retired for v1.4 (v1.5 swap target if bdk regresses)"
  - "D-21 structural invariant verified empty at the ADR commit boundary: `git diff main -- coordinator/ client/ shared/ liquidity-bot/` produced no output. Phase 14 produced zero production-code commits across all 3 plans."
  - "CD-5 separation enforced: ADR commit (58f477e) contains ONLY `.planning/decisions/v1.4-adr.md`; STATE.md/ROADMAP.md flip lives in a separate doc commit (dcdc5fb). Verified via `git show --name-only` on both commits."

patterns-established:
  - "ADR ratification pattern: write ADR alone → commit alone → verify D-21 invariant at commit boundary → STATE/ROADMAP doc-flip commit follows. CD-5 + commit_docs:true precedent for v1.5+ phase closures."
  - "D-05 asymmetry preservation pattern: ADOPT vs EXTEND decisions on verify-side libraries must surface the SIGN-side asymmetry explicitly when the chosen library does not ship signing (mandatory in Decision #1 Consequences for bip322; transferable pattern for any verify-only library adoption)"

requirements-completed: []  # Plan 14-03 frontmatter requirements: [] — gating ADR-producing phase, no feature requirements mapped

# Metrics
duration: ~10 min
completed: 2026-05-29
---

# Phase 14 Plan 03: v1.4 ADR Ratification + Phase 14 Closeout Summary

**v1.4 ADR ratified at `.planning/decisions/v1.4-adr.md` — all 4 Open Decisions resolved per Michael Nygard template (D-20): #1 = ACCEPTED (ADOPT bip322 = "=0.0.10"), #2 = ACCEPTED (mixed rounds), #3 = ACCEPTED (B2 PSBT-input wire format), #4 = ACCEPTED (bdk path); D-21 structural invariant verified empty at the ADR commit; CD-5 separation enforced (ADR commit 58f477e separate from STATE/ROADMAP doc-flip commit dcdc5fb).**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-29T23:45:38Z
- **Completed:** 2026-05-29T23:55:08Z
- **Tasks:** 3 / 3
- **Files modified:** 3 (v1.4-adr.md NEW + STATE.md + ROADMAP.md)
- **Commits:** 2 (ADR commit alone per CD-5; STATE+ROADMAP doc-flip commit follows)

## Accomplishments

- **Synthesised both Sprint-0 verdicts deterministically into the ADR.** Sprint-0-A's `GO:` verdict (sprint-0-A.md:199) flipped Decision #1 from the conservative default `ACCEPTED (EXTEND)` to `ACCEPTED (ADOPT bip322 = "=0.0.10")` per D-01's conditional-flip rule. Sprint-0-B's `PASS:` verdict (sprint-0-B.md:315) plus `bdk path` recommendation (sprint-0-B.md:323) set Decision #4 to `ACCEPTED (bdk path)` per D-14.
- **Recorded Decisions #2 and #3 with discussion-locked rationale.** D-06..D-10 (mixed rounds, single output type per round, no per-script-type min gate, no per-round registration breakdown advertised, CRIT-01 cross-check) and D-11..D-13 (B2 PSBT-input shape with `version: u8` envelope, wire-format roundtrip test ships FIRST in Phase 15 per v1.3 REPAIR-01 lesson #1) recorded with full Michael Nygard sections.
- **Surfaced both load-bearing threat-mitigation items in section-scoped grep targets.** D-05 asymmetry (`bdk_wallet` does NOT ship BIP-322 signing → client signer is ours regardless) lives inside Decision #1's Consequences/negative section, verified by `awk '/^## Decision #1/,/^## Decision #2/' | grep -q 'BIP-322 signing'` per T-14-13. Chain-analysis fingerprint (D-08 known-limitation per CD-3 deferred-to-Phase-18 phrasing) lives inside Decision #2's Consequences/negative section, verified by `awk '/^## Decision #2/,/^## Decision #3/' | grep -q 'chain-analysis fingerprint'` per T-14-12.
- **Verified D-21 structural invariant.** `git diff main -- coordinator/ client/ shared/ liquidity-bot/` produced empty output at the ADR commit boundary. Phase 14 produced zero production-code commits across all 3 plans (14-01 cherry-picked sprint-0-A.md; 14-02 cherry-picked sprint-0-B.md; 14-03 added v1.4-adr.md + STATE/ROADMAP doc-flip).
- **Enforced CD-5 separation.** ADR commit (`58f477e`) contains exactly one file (`.planning/decisions/v1.4-adr.md`). STATE.md + ROADMAP.md flip lives in a follow-up doc commit (`dcdc5fb`) per `commit_docs: true` config and v1.3 phase-closure precedent.
- **Phase 14 closed.** STATE.md frontmatter: `completed_phases: 1`, `total_plans: 3`, `completed_plans: 3`, `percent: 20` (1/5 phases of v1.4 milestone). ROADMAP.md Phase 14 row: `3/3 | Complete | 2026-05-29`. Phase 15 plan-phase has an unambiguous input contract.

## Task Commits

Each task was committed atomically per CD-5:

1. **Task 1 — Read both Sprint-0 verdicts and synthesise the v1.4 ADR** — combined with Task 2's commit (no separate commit for file creation alone; the ADR was written and then committed in Task 2 per CD-5's "ADR commit alone first" structure).
2. **Task 2 — Verify structural D-21 invariant and commit the ADR alone** — `58f477e` `docs(14): record v1.4 ADR for multi-script BIP-322 decisions`
3. **Task 3 — Follow-up doc commit: flip STATE.md and ROADMAP.md to mark Phase 14 Complete** — `dcdc5fb` `docs(14): flip Phase 14 status to Complete`

**Plan metadata commit:** to follow (this SUMMARY.md committed below)

## Files Created/Modified

### Created

- **`.planning/decisions/v1.4-adr.md`** (209 lines) — Canonical v1.4 ADR. Title: `# v1.4 ADR — Multi-Script BIP-322 Decisions`. Four `## Decision #N` sections each with the 5 Michael Nygard subsections (Status / Context / Decision / Consequences / Rejected Alternatives). Decision #1 Status: `ACCEPTED (ADOPT bip322 = "=0.0.10")`. Decision #2 Status: `ACCEPTED (mixed rounds)`. Decision #3 Status: `ACCEPTED (B2 base64 PSBT-input shape, version: u8 envelope)`. Decision #4 Status: `ACCEPTED (bdk path)`. Spike Outputs section links sprint-0-A.md and sprint-0-B.md with verbatim verdict-line quotations and spike-branch HEAD SHAs (`9ce2ff9`, `9ff73cd`).
- **`.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-03-SUMMARY.md`** (this file).

### Modified

- **`.planning/STATE.md`** — Frontmatter flipped: `current_plan` from `2` → `"Phase 14 complete; awaiting /gsd:plan-phase 15"`; `status` from `executing` → `"Phase 14 ADR ratified..."`; `stopped_at` updated; `last_updated`/`last_activity` set to 2026-05-29; `progress.completed_phases: 0 → 1`; `progress.completed_plans: 2 → 3`; `progress.percent: 67 → 20` (recomputed from `completed_phases/total_phases` per inline YAML comment). Current Position section flipped to `Phase 14 — COMPLETE (3/3 plans)`. Progress section: `Phases Complete: 1 of 5`. Session Continuity: `Stopped At: Phase 14 complete (ADR ratified); next: /gsd:plan-phase 15`; `Resume File: .planning/decisions/v1.4-adr.md`. Performance Metrics: new row for Plan 14-03 (~10 min, 3 tasks, 3 files). Phase 14 Decisions: added Plan 14-03 entry summarising the ADR ratification. Accumulated Context: prepended `### Phase 14 close (2026-05-29)` subsection. Open Decisions section: flipped header to `— ALL RESOLVED 2026-05-29` and rewrote each decision bullet to record the resolution + ADR anchor reference.
- **`.planning/ROADMAP.md`** — v1.4 section: Phase 14 bullet flipped `[ ]` → `[x]` with one-liner of resolved decisions and completion date. Phase Details Plans subsection: marked `14-03-PLAN.md` `[x]` with summary. Progress table: row 14 flipped from `2/3 | In Progress |  ` to `3/3 | Complete | 2026-05-29`.

## Decisions Made

- **All 4 Open Decisions resolved in the ADR** (full body lives at `.planning/decisions/v1.4-adr.md`):
  - **#1 ACCEPTED (ADOPT `bip322 = "=0.0.10"`)** — per Sprint-0-A GO; adapter 26 LOC zero-lossy budget cleared; three new transitive crates (`bip322`, `snafu`, `snafu-derive`) accepted into v1.4 dep graph; D-05 asymmetry preserved (client signer remains ours regardless).
  - **#2 ACCEPTED (mixed rounds)** — per D-06; single output type per round per D-07 (operator-configured); no per-script-type min gate per D-08; coordinator advertises supported set only per D-09; CRIT-01 cross-check per D-10; chain-analysis fingerprint recorded as known limitation per CD-3 (Phase 18 README copy).
  - **#3 ACCEPTED (B2 base64 PSBT-input shape, `version: u8` envelope)** — per D-11/D-12; wire-format roundtrip test ships FIRST in Phase 15 per D-13 (v1.3 REPAIR-01 lesson #1 non-negotiable phase boundary).
  - **#4 ACCEPTED (bdk path)** — per Sprint-0-B PASS + `bdk path` recommendation; D-15 manual fallback (80-LOC budget for `shared/src/bip322/p2tr.rs::sign_p2tr_keypath`) retired for v1.4 and held as v1.5 swap target; Phase 17 implementation note carried forward (bdk finalises single-key taproot into `final_script_witness[0]`).
- **CD-5 separation enforced.** Two commits: ADR alone (`58f477e`), then STATE+ROADMAP doc-flip (`dcdc5fb`). The ADR commit message body cross-references the doc-flip is forthcoming; the doc-flip commit message body cross-references back to the ADR commit by SHA.
- **D-21 structural invariant carried forward across all 3 plans.** Plans 14-01 and 14-02 used the cherry-pick pattern (commit on spike branch, cherry-pick the doc-only file to main); Plan 14-03 writes the ADR directly to main (no spike branch needed — pure synthesis from already-landed records). All three plans satisfy the invariant.

## Deviations from Plan

None of significance — plan executed as written.

### Minor procedural notes (not Rule 1-4 deviations)

**Task 1 / Task 2 commit boundary**
- The plan separates "create the ADR file" (Task 1) from "verify D-21 and commit" (Task 2) as distinct tasks. In practice, the ADR file was written, then verified, then committed — no intermediate commit between Task 1's file creation and Task 2's commit. This matches the plan's intent (Task 1's `<done>` field says "ADR file created at `.planning/decisions/v1.4-adr.md`"; Task 2 commits it) and CD-5's "ADR commit alone first" structure. No deviation.

## Threat Flags

None. This plan is doc-only synthesis; no new production surface introduced (D-21 enforced).

The 6 STRIDE threats enumerated in the plan's `<threat_model>` were all mitigated:
- **T-14-09, T-14-10 (Tampering — ADR STATUS does not reflect spike verdict):** Mitigated by deterministic grep-driven STATUS mapping. Decision #1 STATUS records Sprint-0-A `GO:` verdict line verbatim in the Context section; Decision #4 STATUS records Sprint-0-B `PASS:` verdict line and `bdk path` recommendation verbatim in the Context section.
- **T-14-11 (Tampering — ADR closing commit accidentally includes production-code changes):** Mitigated by Task 2's pre-staging `git diff main -- coordinator/ client/ shared/ liquidity-bot/` check (empty output); staging is narrow (`.planning/decisions/v1.4-adr.md` only); commit subject line records the structural gate result.
- **T-14-12 (Information Disclosure — chain-analysis fingerprint not surfaced):** Mitigated by section-scoped grep verification: `awk '/^## Decision #2/,/^## Decision #3/' | grep -q 'chain-analysis fingerprint'` PASS. The wording lives inside Decision #2's Consequences/negative section, not in some other section that a file-wide grep would also accept.
- **T-14-13 (Tampering — D-05 asymmetry not surfaced):** Mitigated by section-scoped grep verification: `awk '/^## Decision #1/,/^## Decision #2/' | grep -q 'BIP-322 signing'` PASS. The wording lives inside Decision #1's Consequences/negative section, with the load-bearing parenthetical "load-bearing — both adopt and extend paths" preserved.
- **T-14-14 (Tampering — STATE/ROADMAP flipped in same commit as ADR):** Mitigated by CD-5 separation. ADR commit (`58f477e`) contains exactly one file. STATE+ROADMAP doc-flip commit (`dcdc5fb`) contains exactly two files. Neither commit's file list includes the other side's files.

## Issues Encountered

None.

## User Setup Required

None — Phase 14 closes cleanly. The next user-facing step is `/gsd:plan-phase 15` to begin Phase 15 plan-phase, which derives BIP322-01..04 + ADVERT-04 tasks from `.planning/decisions/v1.4-adr.md` §`#decision-1` (crate adopt — ADOPT bip322 = "=0.0.10" wrapped in a 26-LOC adapter; module split per D-04) and §`#decision-3` (B2 PSBT-input wire format with `version: u8` envelope; roundtrip test ships first per D-13).

## Next Phase Readiness

- **Phase 15 plan-phase unblocked.** Input contract: read ADR by anchor `#decision-1` (BIP322-01: crate adoption; BIP322-02: per-script-type module organisation per D-04 shape but with crate-backed verify; BIP322-03: BIP-322 message-hash primitives stay as V1.4-MOD-07 single source of truth; BIP322-04: per-script-type property tests against `basic-test-vectors.json`) and `#decision-3` (ADVERT-04: extended `OwnershipProof` wire format with `version: u8` envelope; D-13 wire-format roundtrip test ships FIRST). Phase 15 should NOT re-litigate any of the 4 Open Decisions.
- **Phase 16 plan-phase unblocked.** Input contract: read ADR by anchor `#decision-2` (ADVERT-01: replace `is_p2wpkh()` gate at `coordinator/src/bitcoin/utxo.rs:119` with config-driven allowlist + dispatcher per D-06; ADVERT-02: advertise `supported_script_types` over PKARR + `/round/info` per D-09; ADVERT-03: `[bip] output_script_type` operator-configurable per D-07; CRIT-01: cross-check declared `script_type` against on-chain `txout.script_pubkey` per D-10). Phase 16 also reads `#decision-3` for the coordinator's validate path (decode v1=witness-only or v2=PSBT-input shape per `proof.version`).
- **Phase 17 plan-phase unblocked.** Input contract: read ADR by anchor `#decision-4` (WALLET-02: client P2TR sign path uses `bdk_wallet::Wallet::sign(...)` with `SignOptions { trust_witness_utxo: true }`; witness extraction must check both `psbt.inputs[0].tap_key_sig` and `psbt.inputs[0].final_script_witness[0]` and prefer whichever bdk populated — bdk 2.3 finalises single-key keyspend into `final_script_witness[0]`; parallels existing P2WPKH fallback at `client/src/wallet.rs:277-285`). Phase 17 also reads `#decision-1` (crate-backed verify path is invoked from the client's pre-registration self-verify step).
- **Phase 18 plan-phase unblocked indirectly** — no direct ADR dependency (consumes Phase 15 wire shape + Phase 16 coordinator config + Phase 17 client multi-script signing), but the mixed-script E2E test must reflect Decision #2's mixed-rounds policy (D-06).
- **No blockers, no carry-forward issues.** Phase 14 completed in 3 plans across <1 hour wall-clock (Plan 14-01 ~5 min, Plan 14-02 ~6 min, Plan 14-03 ~10 min); well within the D-18 2-day-per-spike cap aggregated to 4 days for the phase.

## TDD Gate Compliance

Plan 14-03 frontmatter `type: execute` (not `tdd`), so TDD gates do not apply. No TDD commits expected. Plan synthesises existing research artifacts and ratifies an ADR — there is no implementation work to test-drive.

## Cross-Phase Invariant Check

v1.3 P2WPKH-only `full_round::*` integration tests remain green at this phase boundary by D-21 construction: Phase 14 produced zero production-code commits across all 3 plans, so the v1.3 codepath is byte-identical to its v1.3 ship state. No `cargo test` invocation needed — the structural gate is closed by construction.

## Self-Check: PASSED

Verified inline:

- [x] `.planning/decisions/v1.4-adr.md` exists on main (verified via `test -f`).
- [x] ADR contains exactly 4 `^## Decision #` headers (verified via `grep -c` = 4).
- [x] ADR contains `^## Spike Outputs` section (verified via `grep -q`).
- [x] ADR mentions both `sprint-0-A.md` and `sprint-0-B.md` (5 + 7 occurrences respectively).
- [x] All 4 decision sections contain all 5 Michael Nygard subsections (Status / Context / Decision / Consequences / Rejected Alternatives) — verified via the for-loop section-scoped grep.
- [x] Section-scoped D-05 asymmetry check: `awk '/^## Decision #1/,/^## Decision #2/' | grep -q 'BIP-322 signing'` returns 0.
- [x] Section-scoped chain-analysis fingerprint check: `awk '/^## Decision #2/,/^## Decision #3/' | grep -q 'chain-analysis fingerprint'` returns 0.
- [x] Decision #1 STATUS line reads `**Status:** ACCEPTED (ADOPT bip322 = "=0.0.10") — flipped from default ACCEPTED (EXTEND)...` per Sprint-0-A `GO:` verdict (sprint-0-A.md:199).
- [x] Decision #4 STATUS line reads `**Status:** ACCEPTED (bdk path) — per Sprint-0-B's PASS: verdict and bdk path recommendation` per Sprint-0-B verdict (sprint-0-B.md:315) + recommendation (sprint-0-B.md:323).
- [x] ADR commit `58f477e` exists (verified via `git log`); commit subject contains `docs(14): record v1.4 ADR`.
- [x] ADR commit file list contains ONLY `.planning/decisions/v1.4-adr.md` (verified via `git show --name-only`); NOT STATE.md, NOT ROADMAP.md, NOT anything under `coordinator/`, `client/`, `shared/`, `liquidity-bot/`.
- [x] Doc-flip commit `dcdc5fb` exists; commit subject contains `docs(14): flip Phase 14 status to Complete`.
- [x] Doc-flip commit file list contains ONLY `.planning/STATE.md` and `.planning/ROADMAP.md` (verified via `git show --name-only`); the ADR file path is referenced in the commit message body for cross-linking only.
- [x] STATE.md frontmatter: `completed_phases: 1`, `total_plans: 3`, `completed_plans: 3`, `percent: 20` (verified via grep).
- [x] ROADMAP.md Phase 14 row in Progress table: `3/3 | Complete | 2026-05-29` (verified via grep).
- [x] ROADMAP.md v1.4 section Phase 14 bullet: `- [x] **Phase 14: ...** ... ✅ completed 2026-05-29` (verified via grep).
- [x] D-21 structural invariant across all 3 plans of Phase 14: `git log --oneline -- coordinator/ client/ shared/ liquidity-bot/` shows no Phase 14 commits in production paths; the most recent production-path commits all predate Phase 14 (`9dad19f`, `6f8c7e5`, `39302c3`, `0780935`, `8538238`).

---

*Phase: 14-sprint-0-spikes-discuss-phase-decisions*
*Completed: 2026-05-29*
