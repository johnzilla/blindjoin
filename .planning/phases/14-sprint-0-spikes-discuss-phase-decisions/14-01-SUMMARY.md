---
phase: 14-sprint-0-spikes-discuss-phase-decisions
plan: 01
subsystem: research
tags: [bip322, cargo-tree, cargo-audit, spike, adr-input]

# Dependency graph
requires:
  - phase: 14
    provides: "Open Decision #1 framing (adopt vs extend `bip322` crate); D-02 three-gate criteria"
provides:
  - "Canonical Sprint-0-A record: .planning/research/sprint-0-A.md with verbatim cargo tree + cargo audit output, 3-gate verdict, adapter LOC sketch"
  - "Verdict line `GO:` at column 0 — input contract for Plan 14-03's ADR Decision #1 STATUS"
  - "Spike branch spike/14-A-bip322-cargo-tree pushed to origin for reproducibility (HEAD 9ce2ff9)"
affects: [14-03 ADR Decision #1, 15-shared-crate-multi-script-contract]

# Tech tracking
tech-stack:
  added: []  # Spike branch only; main is unchanged
  patterns:
    - "Throwaway spike branch (spike/<phase>-<letter>-<topic>); doc-only commit cherry-picked to main; deps NEVER land in main from spike"

key-files:
  created:
    - ".planning/research/sprint-0-A.md (canonical Sprint-0-A record — verdict GO across 3 D-02 gates)"
  modified:
    - ".planning/STATE.md (mark plan 14-01 execution started)"
    - "shared/Cargo.toml (spike branch ONLY — never on main; added `bip322 = \"=0.0.10\"`)"

key-decisions:
  - "Verdict: GO across all 3 D-02 gates. ADR Decision #1 flips from default ACCEPTED-EXTEND to ACCEPTED-ADOPT for Plan 14-03 to consume."
  - "bip322 v0.0.10 resolves bitcoin v0.32.8 at depth 1 — satisfies workspace pin `bitcoin = 0.32`."
  - "Three new transitive crates introduced by adoption: bip322 v0.0.10, snafu v0.8.9, snafu-derive v0.8.9 (proc-macro, compile-time only). All cargo-audit clean."
  - "Adapter sketch: 26 LOC, zero `unwrap_or*`, witness.clone() is byte-exact deep clone (not lossy). Well under 50-LOC D-02 gate-3 budget."
  - "Sprint-0-A.md commit was cherry-picked onto main via `git cherry-pick 9ce2ff9` after committing on the spike branch — only `.planning/research/sprint-0-A.md` landed in main; Cargo.toml change stayed on the spike branch per D-19/D-21."

patterns-established:
  - "Pattern: throwaway spike branch + cherry-pick the doc-only commit to main (preserves D-19 branch hygiene and D-21 production-path structural invariant simultaneously)"
  - "Pattern: when cargo audit clean run produces 0-4 lines without an explicit summary, supplement with `cargo audit --json` to capture the explicit `vulnerabilities.found: false / count: 0` summary for auditability (mitigates T-14-03 repudiation threat)"

requirements-completed: []  # Plan 14-01 maps zero requirements per PLAN frontmatter (gating spike, not a feature ship)

# Metrics
duration: 5min
completed: 2026-05-29
---

# Phase 14 Plan 01: Sprint-0-A bip322 cargo tree + cargo audit + adapter LOC probe Summary

**bip322 v0.0.10 cleared all three D-02 gates (bitcoin = 0.32.8 transitive pin, cargo audit zero advisories, adapter sketch 26 LOC zero-lossy) — verdict GO; ADR Decision #1 flips from default EXTEND to ADOPT for Plan 14-03.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-05-29T23:25:57Z
- **Completed:** 2026-05-29T23:30:46Z
- **Tasks:** 3 / 3
- **Files modified:** 3 (sprint-0-A.md created, STATE.md updated on main, shared/Cargo.toml updated on spike branch only)
- **Sprint cap:** 2 days (D-18); used: < 2 hours — well under cap

## Accomplishments

- Sprint-0-A canonical record landed in main as `.planning/research/sprint-0-A.md` (210 lines, verbatim cargo tree + cargo audit JSON summary + adapter sketch + 3-gate verdict).
- Spike branch `spike/14-A-bip322-cargo-tree` (HEAD `9ce2ff9`) pushed to origin for reproducibility per D-19.
- D-21 structural invariant verified: zero commits on main touch `coordinator/`, `client/`, `shared/`, or `liquidity-bot/` source as a result of this plan. The latest commit on main's `shared/Cargo.toml` is `5970cc9 feat(01-01): Cargo workspace + shared crate scaffold` (pre-v1.4) — untouched by this plan.
- Verdict line `GO:` is at column 0 of sprint-0-A.md so Plan 14-03's grep can detect it deterministically.

## Task Commits

Each task was committed atomically:

1. **Pre-Task 0 — Record plan 14-01 execution start in STATE.md (committed on main)** — `310d106` (docs)
2. **Task 1 — Create throwaway spike branch and add `bip322 = "=0.0.10"` to `shared/Cargo.toml` (committed on spike branch ONLY)** — `e3756b7` (spike) — on `spike/14-A-bip322-cargo-tree`, not on main
3. **Task 2 — Capture cargo tree + cargo audit + adapter LOC sketch and write sprint-0-A.md verdict (committed on spike branch FIRST, then cherry-picked to main)** — `9ce2ff9` on spike branch; cherry-picked to main as `f925352`
4. **Task 3 — Push spike branch to origin and verify structural D-21 invariant (NO file modifications; verification-only task; no commit)** — verified successfully, branch pushed to `origin` HEAD `9ce2ff9`

**Plan metadata commit:** TBD (this SUMMARY.md + STATE.md/ROADMAP.md updates committed below)

_Note: The plan is structurally unusual — only one task (Task 2) lands a file in main; Task 1's commit lives only on the throwaway spike branch (per D-19); Task 3 is structurally-verifying-only and has no commit. This is by design._

## Files Created/Modified

### On main
- `.planning/research/sprint-0-A.md` — created (Task 2) — canonical Sprint-0-A record. Verdict line `GO:` at column 0. Embeds verbatim cargo tree (74-line tree) + cargo audit textual output + cargo audit JSON summary line (`vulnerabilities.found: false`) + 26-LOC adapter sketch + lossy-conversion audit. 210 lines total.
- `.planning/STATE.md` — modified (pre-Task 0) — `current_plan: 1`, `status: Executing Phase 14`, `last_activity: Phase 14 execution started`.

### On spike branch only (NOT in main)
- `shared/Cargo.toml` — added line `bip322 = "=0.0.10"` (Task 1). Committed as `e3756b7` on `spike/14-A-bip322-cargo-tree`. This commit is deliberately quarantined from main per D-19/D-21.

## Decisions Made

- **GO across all 3 D-02 gates** — see sprint-0-A.md for verbatim evidence. Concretely:
  - Gate 1: `bip322 v0.0.10 → bitcoin v0.32.8` at depth 1. Pins workspace `bitcoin = "0.32"` cleanly. No 0.31.x or earlier present in transitive graph.
  - Gate 2: `cargo audit` exit 0; `cargo audit --json` reports `vulnerabilities: {found: false, count: 0, list: []}` and `warnings: {}` against 710-crate lockfile (was 707 on main + 3 new transitive crates). The three `settings.ignore` advisories pre-date this spike (audit.toml entries from v1.3 commit `d71e592`, 2026-05-26).
  - Gate 3: adapter sketch lands at 26 LOC (4 imports + 13 error enum + 9 function) — well under 50-LOC budget. Zero occurrences of `unwrap_or` / `unwrap_or_default` / `unwrap_or_else`. `witness.clone()` is the correct (and required by API signature) byte-exact deep clone; `Address::from_script` returns the full Address variant; `message: &[u8]` flows to `impl AsRef<[u8]>` with no copy or truncation.
- **Cherry-pick over recreate** — Used `git cherry-pick 9ce2ff9` to land sprint-0-A.md in main rather than rewriting the file. Diff confirms only `.planning/research/sprint-0-A.md` came over (no `shared/Cargo.toml` or `Cargo.lock` contamination). Pre-cherry-pick step: discarded the working-tree Cargo.lock changes (which reflected the bip322 dep adding 3 lockfile entries) before checking out main so main's Cargo.lock stays canonical.

## Deviations from Plan

None of significance — plan executed as written.

### Minor procedural notes (not Rule 1-4 deviations)

**Pre-Task 0 setup commit**
- STATE.md had an uncommitted edit from the orchestrator marking `current_plan: 1` (execution start). To keep the spike branch starting from a clean working tree, this STATE.md edit was committed on main first as `310d106 docs(14-01): mark plan 14-01 execution started in STATE.md`. This is a procedural commit, not a deviation from the plan's intent — STATE.md was always going to need an execution-start record.

**cargo audit clean-run output is only 4 lines (vs the plan's `>5 lines OR explicit summary` acceptance criterion)**
- `cargo audit` 0.22.1 emits no advisory-summary section on a clean run (no `Vulnerable Crates Found: 0` line). The terminal stdout is just `Fetching... / Loaded N advisories / Scanning Cargo.lock` (4 lines).
- To satisfy the acceptance criterion's "explicit 0-advisories terminal line" branch, sprint-0-A.md additionally embeds the verbatim `cargo audit --json` output, which includes the explicit `"vulnerabilities":{"found":false,"count":0,"list":[]}` summary field. This is the explicit summary the acceptance criterion was reaching for; the JSON output is a faithful machine-readable rendering of the same audit.
- Mitigates T-14-03 (repudiation): future reviewers can re-derive Gate 2 PASS from the embedded JSON summary alone.

## Threat Flags

None. This plan is paper-analysis only; no production code surface introduced.

## Spike Branch Reference Card (for Plan 14-03)

- Branch name: `spike/14-A-bip322-cargo-tree`
- Branch HEAD SHA (on origin): `9ce2ff9` (`spike(14-A): capture cargo tree, audit, adapter sketch — verdict GO`)
- Branch HEAD SHA (parent): `e3756b7` (`spike(14-A): add bip322 = "=0.0.10" dep for cargo tree probe`)
- Branch base: `310d106` on main (`docs(14-01): mark plan 14-01 execution started in STATE.md`)
- Sprint-0-A.md in main: commit `f925352` (`spike(14-A): capture cargo tree, audit, adapter sketch — verdict GO`)
- Verdict for Plan 14-03 ADR Decision #1 STATUS: **ACCEPTED (ADOPT bip322 = "=0.0.10")** (per D-02 GO verdict and D-01's "default EXTEND, adopt only if Sprint-0-A passes all gates" conditional flip)

## Self-Check: PASSED

- [x] `.planning/research/sprint-0-A.md` exists in main (verified via `git log main -- .planning/research/sprint-0-A.md` returning `f925352`)
- [x] Verdict line `^GO:` present at column 0 (verified via `grep -E '^(GO|NO-GO|INCONCLUSIVE):' .planning/research/sprint-0-A.md`)
- [x] cargo tree command string appears in non-`#` lines (2 occurrences)
- [x] cargo audit command string appears in non-`#` lines (5 occurrences)
- [x] LOC count is a concrete integer `26` (verified via grep)
- [x] Lossy-conversion audit is explicit (`unwrap_or*: 0`, field squashing: `no` with justification)
- [x] Spike branch on origin: `git ls-remote --heads origin spike/14-A-bip322-cargo-tree` returns `9ce2ff93f1596ec9d8450588fb7daaa7e81f3150 refs/heads/spike/14-A-bip322-cargo-tree`
- [x] main's `shared/Cargo.toml` latest commit is `5970cc9 feat(01-01): Cargo workspace + shared crate scaffold` — NOT a `spike(14-A)` commit (D-21 invariant holds)
- [x] `git log main --oneline --since="2026-05-29T23:25:00Z" -- coordinator/ client/ shared/ liquidity-bot/` returns empty (no production-path commits during this plan window)
- [x] On-main commit `f925352` touches exactly one file (`.planning/research/sprint-0-A.md`) per `git show --stat HEAD`
