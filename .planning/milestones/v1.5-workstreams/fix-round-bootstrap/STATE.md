---
workstream: fix-round-bootstrap
created: 2026-05-25
status: completed
resolved: 2026-05-26
---

# Project State

## Current Position
**Status:** Completed (out of band — workstream STATE was not updated at the time)
**Resolution date:** 2026-05-26
**Last Activity:** 2026-05-29 -- audit confirmed work shipped; STATE updated

## Progress
**Phases Complete:** N/A (resolved via direct work, not as a phase)

## Resolution

Workstream scope shipped as part of v1.1+ work — `pub fn start_round` exists
at [coordinator/src/round/manager.rs:40](../../../coordinator/src/round/manager.rs)
as the production round-creation entry point, called from `coordinator::run::run`
on Idle→InputReg transitions. The blocking integration test exists at
[tests/integration/round_bootstrap.rs](../../../tests/integration/round_bootstrap.rs)
(205 lines, runs in CI under `BLINDJOIN_REQUIRE_BITCOIND=1`).
