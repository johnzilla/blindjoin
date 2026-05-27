---
gsd_state_version: 1.0
milestone: v1.3
milestone_name: Test Infrastructure & Operational Hardening
current_plan: 1
status: executing
stopped_at: Phase 9 context gathered
last_updated: "2026-05-27T02:27:41.582Z"
last_activity: 2026-05-27
progress:
  total_phases: 2
  completed_phases: 0
  total_plans: 5
  completed_plans: 1
  percent: 0
---

# Project State

## Current Position

Phase: 09 (ci-integration-test-reliability) — EXECUTING
Plan: 2 of 5
Status: Ready to execute
Last activity: 2026-05-27

## Progress

**Phases Complete:** 0 of 2
**Current Plan:** 1

## Session Continuity

**Stopped At:** Phase 9 context gathered
**Resume File:** None

## Decisions

- v1.3 phase shape: 2 phases (9 + 10), not 3. Phase 9 bundles all 5 TEST-* requirements because the pieces interlock — TEST-02 (no silent skips) requires TEST-01 (bitcoind on the runner) to be observable; TEST-03 (clean exit on panic) and TEST-04 (no leaked daemons) are the same root cause (corepc-node Box::leak); TEST-05 (CONTRIBUTING.md) documents the canonical pattern the other four enable. Splitting 9a/9b would create a phase whose success criteria can't be observed end-to-end until the other half lands.
- v1.3 Phase 10 sequenced after Phase 9 because REPAIR-01's success criterion ("all 15 tests pass against pinned bitcoind") only becomes observable once Phase 9's CI infrastructure exists. REPAIR-02 (explicit corepc-node version features) naturally falls out of any repair path taken in REPAIR-01.
- 08-04 Plan: 408-test uses Path B (slow body via raw tokio TCP slow-write) — no new dependencies introduced. Path A (slow handler) was infeasible without forbidden test-only handler injection; planner-suggested reqwest::Body::wrap_stream requires futures-util/async-stream which are not in dev-deps.
- 08-04 Plan: connection-cap (max_concurrent_connections) end-to-end runtime test DEFERRED per A4 (clearnet test infra cannot exercise the tor-only semaphore). Documented inline with a TODO(Phase-8 Q3, A4) comment. Coverage stands via Plan 03's grep audits.
- 08-04 Plan: neither test attaches #[ignore]. The verify command still uses --include-ignored for CI forward-compatibility (if a future change needs to mark a test ignored).
- 08-04 Plan: three-condition 429 assertion (status + retry-after header + JSON envelope code RATE_LIMITED) — proves the full response envelope shape, not just the status code.
- [Phase ?]: 09-01 Plan: re-verified actions/cache@v4 SHA at execution time (0057852bfaa89a56745cba8c7296529d2fc39830 — matches CONTEXT.md/RESEARCH.md, no drift)
- [Phase ?]: 09-01 Plan: pinned bitcoin-core/guix.sigs commit 893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59 (main HEAD on 2026-05-26) for the achow101 PGP key fetch in the CI install step
- [Phase ?]: 09-01 Plan: CI Run tests command kept verbatim — cargo test --workspace --all-targets, no --include-ignored — so the 6 Phase-10 carve-out tests list as ignored without executing (per amended D-10)

## Performance Metrics

| Phase-Plan | Duration | Tasks | Files | Completed |
|------------|----------|-------|-------|-----------|
| 08-04 | ~5min | 2 | 2 | 2026-05-26 |
| Phase 09 P01 | 20min | 3 tasks | 2 files |

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260526-d7m | CI hygiene: bump rand 0.8.5→0.8.6 (RUSTSEC-2026-0097) and force Node 24 runtime for JS actions in CI workflows | 2026-05-26 | (pending) | [260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos](./quick/260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos/) |
