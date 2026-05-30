---
phase: 14-sprint-0-spikes-discuss-phase-decisions
verified: 2026-05-29T20:30:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: null
  previous_score: null
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions Verification Report

**Phase Goal:** "Resolve every load-bearing v1.4 decision before any production code is written, so downstream phases have unambiguous specifications."
**Verified:** 2026-05-29T20:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria + PLAN must_haves)

| # | Truth (Success Criterion) | Status | Evidence |
|---|---------------------------|--------|----------|
| 1 | `cargo tree -p bip322` output checked into `.planning/research/sprint-0-A.md` and shows whether `bip322 0.0.10` pins to `bitcoin 0.32.x`; GO/NO-GO call recorded on Open Decision #1 | VERIFIED | sprint-0-A.md exists (210 lines); embeds full 74-line `cargo tree -p bip322 -e normal --format "{p} {f}"` output verbatim (lines 14-88); `bitcoin v0.32.8` present at depth 1 directly under `bip322 v0.0.10` (line 26); verdict `GO:` at column 0 of line 199; ADR Decision #1 STATUS = ACCEPTED (ADOPT) confirms the conditional flip per D-01 |
| 2 | Throwaway bdk_wallet 2.3 P2TR descriptor + BIP-322 message signing PoC has been run; result recorded in `.planning/research/sprint-0-B.md`; resolves Open Decision #4 | VERIFIED | sprint-0-B.md exists (364 lines); embeds full PoC source verbatim (263 lines from `client/examples/spike-p2tr.rs`); captured stdout shows `STEP_6_BDK_SIGN: Ok(finalized=true)`, 64-byte witness hex `295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069` (128 hex chars), `STEP_8_VERIFY_SCHNORR: Ok`; `PASS:` verdict line at column 0 of line 315; recommendation `bdk path` at line 323; PoC source contains `trust_witness_utxo: true` (line 160), `build_bip322_to_spend` and `build_bip322_to_sign` invocations, real on-chain `witness_utxo` (line 142-148) — no v1.3 Phase 12 strawman |
| 3 | ADR checked into `.planning/decisions/v1.4-adr.md` records resolutions of Open Decisions #1, #2, #3, #4 with chosen option and one-paragraph rationale | VERIFIED | v1.4-adr.md exists (209 lines); contains exactly 4 `^## Decision #` headers; each decision has all 5 Michael Nygard subsections (Status / Context / Decision / Consequences / Rejected Alternatives) per D-20. Decision #1 STATUS = `ACCEPTED (ADOPT bip322 = "=0.0.10")`; Decision #2 STATUS = `ACCEPTED (mixed rounds)`; Decision #3 STATUS = `ACCEPTED (B2 base64 PSBT-input shape, version: u8 envelope)`; Decision #4 STATUS = `ACCEPTED (bdk path)`. D-05 asymmetry surfaced in Decision #1 Consequences (section-scoped grep `BIP-322 signing` PASS, 1 hit). D-08 chain-analysis fingerprint surfaced in Decision #2 Consequences (section-scoped grep `chain-analysis fingerprint` PASS, 2 hits). `## Spike Outputs` section links both canonical records by name and spike-branch HEAD SHA (`9ce2ff9`, `9ff73cd`) |
| 4 | v1.3 `full_round::*` integration tests still pass at this phase boundary (no production code touched by Phase 14 spikes) | VERIFIED | Verified structurally per D-21 (cross-phase invariant defined in phase frontmatter context). `git diff --name-only 7a60554..HEAD -- . ':!.planning/' ':!ROADMAP.md' ':!STATE.md' ':!*-SUMMARY.md' ':!*-VERIFICATION.md' ':!*-PLAN.md'` returns empty output. `git log --since="2026-05-29" --oneline main -- coordinator/ client/ shared/ liquidity-bot/` returns empty. Most recent production-path commit on main is `9dad19f` from 2026-05-28 23:12:23 — predates Phase 14 context-session commit `396c605` (2026-05-29 18:29:43). v1.3 codepath is byte-identical to its v1.3 ship state by construction; no test-suite re-run needed |
| 5 | Each spike was capped at 2 days of effort or explicitly escalated; spike branches not merged into `main` (POC code lives in branches, not the trunk) | VERIFIED | Both spike branches exist locally AND on origin: `spike/14-A-bip322-cargo-tree` and `spike/14-B-bdk-p2tr-poc` (verified via `git branch -a`). Cherry-pick discipline: only doc-only commits land in main (`f925352` for sprint-0-A.md, `efd8d59` for sprint-0-B.md — both show ONLY `.planning/research/sprint-*.md` in `git show --stat`). PoC binary check: `test -f client/examples/spike-p2tr.rs` = MISSING (correct — file lives only on spike branch). `shared/Cargo.toml` on main: no `bip322` line (the `=0.0.10` pin stays on the spike branch). Sprint times per SUMMARYs: Plan 14-01 ~5 min, Plan 14-02 ~6 min, Plan 14-03 ~10 min — well under the 2-day D-18 cap |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `.planning/research/sprint-0-A.md` | Canonical Sprint-0-A record with cargo tree + cargo audit + 3-gate verdict | VERIFIED | 210 lines; contains literal `cargo tree -p bip322` (2 occurrences in non-`#` lines), `cargo audit` (5 occurrences in non-`#` lines), verdict `GO:` at column 0 of line 199, adapter LOC count `26`, lossy-conversion audit explicit (`unwrap_or*: 0`, field squashing: no with justification), spike HEAD SHA `e3756b7a5320d6ca15c1d37b852db40dc47cd9bd` |
| `.planning/research/sprint-0-B.md` | Canonical Sprint-0-B record with PoC source, witness hex, verdict, recommendation | VERIFIED | 364 lines; contains `trust_witness_utxo: true`, `build_bip322_to_spend`, `build_bip322_to_sign`, `verify_schnorr`, recommendation `bdk path`, 128-char hex witness, verdict `PASS:` at column 0 of line 315, spike HEAD SHA `9ff73cd286920d1e9fcac1e6506e7e3300b7abe7`. Embedded PoC source is 263 lines (well over the >15 line acceptance threshold) |
| `.planning/decisions/v1.4-adr.md` | v1.4 ADR with 4 Decision sections per Michael Nygard template + Spike Outputs section | VERIFIED | 209 lines; exactly 4 `## Decision #` headers; `## Spike Outputs` section present with both sprint-0-*.md links + spike HEAD SHAs; all 4 decisions have all 5 Michael Nygard subsections (Status / Context / Decision / Consequences / Rejected Alternatives); STATUS lines mapped deterministically to Sprint-0 verdicts |
| `.planning/STATE.md` | Phase 14 status flipped to Complete | VERIFIED | Frontmatter shows `completed_phases: 1`, `total_plans: 3`, `completed_plans: 3`, `percent: 20` (1/5 phases of v1.4 milestone) |
| `.planning/ROADMAP.md` | Phase 14 row updated to 3/3 plans Complete with completion date | VERIFIED | Progress table row: `\| 14. Sprint-0 Spikes + Discuss-Phase Decisions \| v1.4 \| 3/3 \| Complete \| 2026-05-29 \|`. v1.4 section bullet flipped to `[x] **Phase 14: ...** ... ✅ completed 2026-05-29` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| sprint-0-A.md verdict line | ADR Decision #1 STATUS | Plan 14-03 grep `^(GO\|NO-GO\|INCONCLUSIVE):` head -1 | WIRED | sprint-0-A.md:199 reads `GO:` → ADR Decision #1 STATUS = `ACCEPTED (ADOPT bip322 = "=0.0.10")` per D-01 conditional-flip. Deterministic mapping confirmed. |
| sprint-0-B.md verdict line + recommendation | ADR Decision #4 STATUS | Plan 14-03 grep `^(PASS\|FAIL\|INCONCLUSIVE):` head -1 + `bdk path` | WIRED | sprint-0-B.md:315 reads `PASS:` + line 323 reads `bdk path` → ADR Decision #4 STATUS = `ACCEPTED (bdk path)` per D-14. Deterministic mapping confirmed. |
| spike branch `spike/14-A-bip322-cargo-tree` | origin remote | `git push -u origin spike/14-A-bip322-cargo-tree` | WIRED | `git branch -a` shows `remotes/origin/spike/14-A-bip322-cargo-tree`; reproducibility per D-19 satisfied |
| spike branch `spike/14-B-bdk-p2tr-poc` | origin remote | `git push -u origin spike/14-B-bdk-p2tr-poc` | WIRED | `git branch -a` shows `remotes/origin/spike/14-B-bdk-p2tr-poc`; reproducibility per D-19 satisfied |
| ADR | Phase 15/16/17 planners | Anchor references `#decision-1`, `#decision-2`, `#decision-3`, `#decision-4` | WIRED (input contract) | ADR file at `.planning/decisions/v1.4-adr.md` lists downstream consumers in header; each decision is independently grep-anchorable; Phase 15-18 phases exist in ROADMAP as "Not started" awaiting plan-phase |

### Data-Flow Trace (Level 4)

Not applicable — this is a documentation/decision-artifact phase. No dynamic data rendering. All artifacts are static markdown produced from spike outputs.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| ADR has exactly 4 Decision sections | `grep -c '^## Decision #' .planning/decisions/v1.4-adr.md` | `4` | PASS |
| Section-scoped D-05 asymmetry surfaced in Decision #1 | `awk '/^## Decision #1/,/^## Decision #2/' .planning/decisions/v1.4-adr.md \| grep -c 'BIP-322 signing'` | `1` | PASS |
| Section-scoped chain-analysis fingerprint surfaced in Decision #2 | `awk '/^## Decision #2/,/^## Decision #3/' .planning/decisions/v1.4-adr.md \| grep -c 'chain-analysis fingerprint'` | `2` | PASS |
| sprint-0-A.md verdict at column 0 | `grep -E '^(GO\|NO-GO\|INCONCLUSIVE):' .planning/research/sprint-0-A.md` | `GO: all three D-02 gates PASS...` | PASS |
| sprint-0-B.md verdict at column 0 | `grep -E '^(PASS\|FAIL\|INCONCLUSIVE):' .planning/research/sprint-0-B.md` | `PASS: bdk_wallet 2.3's PSBT signer...` | PASS |
| sprint-0-B.md recommendation present | `grep -E '(bdk path\|manual fallback per D-15)' .planning/research/sprint-0-B.md` | `bdk path` + 3 other matches | PASS |
| Both spike branches on origin | `git branch -a \| grep spike` | All 4 expected refs present (local + origin × 2 branches) | PASS |
| D-21 invariant: no production code on main | `git diff --name-only 7a60554..HEAD -- . ':!.planning/' ':!ROADMAP.md' ':!STATE.md' ':!*-SUMMARY.md' ':!*-VERIFICATION.md' ':!*-PLAN.md'` | empty | PASS |
| PoC binary not on main | `test -f client/examples/spike-p2tr.rs` | MISSING | PASS |
| bip322 dep not on main | `grep 'bip322' shared/Cargo.toml` | empty | PASS |

### Probe Execution

Not applicable — Phase 14 is a doc-only ADR-producing phase by D-21 design. No probes defined in PLANs; no conventional probes (`find scripts -path '*/tests/probe-*.sh'` returns empty). Phase produces zero runnable code on main.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| (none) | 14-01, 14-02, 14-03 | All three plans have `requirements: []` in frontmatter | N/A | REQUIREMENTS.md line confirms intentional: "Phase 14 (Sprint-0 + Discuss-Phase Decisions) maps zero requirements — it is a gating spike/decision phase that produces an ADR resolving Open Decisions #1, #2, #3, #4 before Phase 15 plan-phase can derive tasks. This is intentional, not an orphan." |

No orphan requirements detected. REQUIREMENTS.md table confirms `Phase 14 — Sprint-0 Spikes + Discuss-Phase Decisions | (none) | Gating ADR-producing phase`.

### Anti-Patterns Found

No anti-patterns detected.

- Debt-marker scan across `.planning/research/sprint-0-A.md`, `.planning/research/sprint-0-B.md`, `.planning/decisions/v1.4-adr.md`: zero hits for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER`.
- Phase 14 is doc-only by D-21 construction; no source files to scan for stub patterns, hardcoded empty values, or empty implementations.
- Anti-patterns in PoC code (`client/examples/spike-p2tr.rs`) are not relevant: the file lives only on the throwaway spike branch and is explicitly NOT in main.

### Human Verification Required

None. All Phase 14 must-haves are programmatically verifiable from the codebase state:

- File existence and structure: confirmed via `test -f` + line counts.
- Verdict-line determinism: confirmed via `grep -E '^...:'` at column 0.
- Michael Nygard subsection completeness: confirmed via `awk` range + `grep`.
- D-21 structural invariant: confirmed via `git diff` and `git log` on production paths.
- Spike branch reproducibility: confirmed via `git branch -a`.
- Deterministic verdict→STATUS mapping in ADR: confirmed by reading both Sprint-0 verdict lines and matching ADR STATUS lines.

There are no visual, UX, real-time, or external-service behaviors to evaluate. The PoC's correctness was self-validated by `secp256k1::verify_schnorr` returning `Ok(())` and the witness hex being byte-deterministic from the `[0u8; 32]` seed (reproducible by any reviewer running `cargo run -p client --example spike-p2tr` on the spike branch).

### Gaps Summary

No gaps. Every load-bearing decision (#1 crate adopt vs extend, #2 mixed vs segregated rounds, #3 P2SH-P2WPKH wire format, #4 bdk_wallet 2.3 P2TR sign path) has an explicit ACCEPTED status with full Michael Nygard rationale in the ADR. Both spike verdicts are deterministically mapped to ADR STATUS lines (Sprint-0-A `GO` → ADOPT; Sprint-0-B `PASS` → bdk path). The D-21 structural invariant — zero production code touched on main during Phase 14 — holds across all 3 plans (verified via `git diff` against `7a60554`, the Phase 14 plan-phase finalize commit). The cross-phase invariant (v1.3 `full_round::*` tests remain green) holds by construction: no production code changed, so the v1.3 codepath is byte-identical to its v1.3 ship state.

Phase 15-18 have unambiguous input contracts via anchor references (`#decision-1` for BIP322-01..04 + ADVERT-04, `#decision-2` for ADVERT-01..03 + CRIT-01, `#decision-3` for the B2 wire format with D-13 test discipline, `#decision-4` for WALLET-02 bdk path including the bdk-finalisation implementation note that the witness lands in `final_script_witness[0]`).

The phase goal — "Resolve every load-bearing v1.4 decision before any production code is written, so downstream phases have unambiguous specifications" — is achieved.

---

_Verified: 2026-05-29T20:30:00Z_
_Verifier: Claude (gsd-verifier)_
