---
gsd_state_version: 1.0
milestone: v1.3
milestone_name: Test Infrastructure & Operational Hardening
status: planning
last_updated: "2026-05-27T00:55:25.722Z"
last_activity: 2026-05-27
progress:
  total_phases: 2
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
current_phase: 9
---

# Project State

## Current Position

Phase: 9 — CI integration-test reliability (not yet planned)
Plan: —
Status: Roadmap drafted; awaiting `/gsd-plan-phase 9`
Last activity: 2026-05-27 — v1.3 roadmap created (Phases 9-10)

## Progress

**Phases Complete:** 0 of 2
**Current Plan:** Not started

## Session Continuity

**Stopped At:** v1.3 roadmap written; Phase 9 ready to plan
**Resume File:** None

## Decisions

- v1.3 phase shape: 2 phases (9 + 10), not 3. Phase 9 bundles all 5 TEST-* requirements because the pieces interlock — TEST-02 (no silent skips) requires TEST-01 (bitcoind on the runner) to be observable; TEST-03 (clean exit on panic) and TEST-04 (no leaked daemons) are the same root cause (corepc-node Box::leak); TEST-05 (CONTRIBUTING.md) documents the canonical pattern the other four enable. Splitting 9a/9b would create a phase whose success criteria can't be observed end-to-end until the other half lands.
- v1.3 Phase 10 sequenced after Phase 9 because REPAIR-01's success criterion ("all 15 tests pass against pinned bitcoind") only becomes observable once Phase 9's CI infrastructure exists. REPAIR-02 (explicit corepc-node version features) naturally falls out of any repair path taken in REPAIR-01.
- 08-04 Plan: 408-test uses Path B (slow body via raw tokio TCP slow-write) — no new dependencies introduced. Path A (slow handler) was infeasible without forbidden test-only handler injection; planner-suggested reqwest::Body::wrap_stream requires futures-util/async-stream which are not in dev-deps.
- 08-04 Plan: connection-cap (max_concurrent_connections) end-to-end runtime test DEFERRED per A4 (clearnet test infra cannot exercise the tor-only semaphore). Documented inline with a TODO(Phase-8 Q3, A4) comment. Coverage stands via Plan 03's grep audits.
- 08-04 Plan: neither test attaches #[ignore]. The verify command still uses --include-ignored for CI forward-compatibility (if a future change needs to mark a test ignored).
- 08-04 Plan: three-condition 429 assertion (status + retry-after header + JSON envelope code RATE_LIMITED) — proves the full response envelope shape, not just the status code.

## Performance Metrics

| Phase-Plan | Duration | Tasks | Files | Completed |
|------------|----------|-------|-------|-----------|
| 08-04 | ~5min | 2 | 2 | 2026-05-26 |

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260526-d7m | CI hygiene: bump rand 0.8.5→0.8.6 (RUSTSEC-2026-0097) and force Node 24 runtime for JS actions in CI workflows | 2026-05-26 | (pending) | [260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos](./quick/260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos/) |
