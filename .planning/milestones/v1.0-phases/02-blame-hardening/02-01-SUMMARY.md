---
phase: 02-blame-hardening
plan: 01
subsystem: api
tags: [rust, sha2, hex, ban-list, blame, coinjoin, axum]

# Dependency graph
requires:
  - phase: 01-core-protocol
    provides: RoundState, RegisteredInput, partial_sigs HashMap, AppState, ErrorCode enum, input_reg handler path

provides:
  - BanList struct with SHA-256-keyed entries, configurable expiry, idempotent ban refresh
  - detect_non_signers() comparing registered_inputs vs partial_sigs
  - has_missing_outputs() aggregate output gap detection (BLAME-02)
  - ErrorCode::UtxoBanned serializing as UTXO_BANNED
  - Ban check wired into POST /round/input returning HTTP 403 before round write lock

affects: [02-blame-hardening, 03-persistence, integration-tests]

# Tech tracking
tech-stack:
  added: []  # sha2 and hex were already workspace dependencies
  patterns:
    - SHA-256 keyed ban entries: raw UTXO outpoint never stored in BanList (PRIV-02 compliance)
    - Ban check before write lock: fast rejection path in post_input reduces lock contention
    - BanList survives rounds: stored in AppState as Arc<RwLock<BanList>> not in RoundStateInner

key-files:
  created:
    - coordinator/src/round/blame.rs
  modified:
    - coordinator/src/round/mod.rs
    - coordinator/src/api/mod.rs
    - coordinator/src/api/handlers.rs
    - shared/src/errors.rs

key-decisions:
  - "Ban check placed at handler layer (not input_reg.rs logic layer) — consistent with how phase checks work"
  - "BanList stored in AppState not RoundStateInner — must survive round transitions and state zeroing"
  - "detect_non_signers_finds_missing test sorts result before asserting — HashMap iteration order is non-deterministic"

patterns-established:
  - "BanList pattern: SHA-256 keyed entries with banned_at + expires_at fields for persistence compatibility"
  - "Fast rejection: ban_list read lock acquired and released before round write lock to minimize contention"

requirements-completed: [BLAME-01, BLAME-02, BLAME-03]

# Metrics
duration: 2min
completed: 2026-04-07
---

# Phase 2 Plan 1: Blame Hardening — BanList + Non-Signer Detection Summary

**SHA-256-keyed in-memory BanList with configurable expiry wired into POST /round/input (HTTP 403), plus detect_non_signers() diffing registered_inputs vs partial_sigs for BLAME-01/02 coverage**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-04-07T00:00:00Z
- **Completed:** 2026-04-07T00:02:03Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Created `coordinator/src/round/blame.rs` with BanList, BanEntry, detect_non_signers, has_missing_outputs, now_unix_secs — all tested with 7 unit tests
- Wired BanList into AppState and post_input handler: banned UTXOs receive HTTP 403 UTXO_BANNED before any RPC or lock contention
- Added ErrorCode::UtxoBanned to shared errors (SCREAMING_SNAKE_CASE serialization as "UTXO_BANNED")

## Task Commits

1. **Task 1: BanList module + non-signer detection** — `4b4e30a` (feat)
2. **Task 2: Wire ban check into input registration path** — `578146a` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `coordinator/src/round/blame.rs` — BanList, BanEntry, detect_non_signers, has_missing_outputs, now_unix_secs; 7 unit tests
- `coordinator/src/round/mod.rs` — Added `pub mod blame;`
- `coordinator/src/api/mod.rs` — Added `ban_list: Arc<RwLock<BanList>>` field to AppState; initialized in build_router
- `coordinator/src/api/handlers.rs` — Ban check in post_input before write lock; returns HTTP 403 UTXO_BANNED
- `shared/src/errors.rs` — Added `UtxoBanned` variant to ErrorCode enum

## Decisions Made

- Ban check placed at handler layer (not input_reg.rs logic layer) — consistent with how phase checks work in post_input
- BanList stored in AppState (not RoundStateInner) so it survives round transitions and the state zeroing in Drop
- detect_non_signers result sorted before assertion in test — HashMap iteration is non-deterministic, test was fixed to be deterministic

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Sorted detect_non_signers test result for determinism**
- **Found during:** Task 1 (BanList module)
- **Issue:** The plan's test asserted `assert_eq!(non_signers, vec!["tx2:0"])` but HashMap iteration order is non-deterministic — test would be flaky
- **Fix:** Added `non_signers.sort()` before the assertion in `detect_non_signers_finds_missing`
- **Files modified:** coordinator/src/round/blame.rs
- **Verification:** Test passes consistently
- **Committed in:** 4b4e30a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — bug fix for test determinism)
**Impact on plan:** Minimal — only affects test assertion ordering, not production code. No scope creep.

## Issues Encountered

None — build and all tests passed on first attempt.

## Known Stubs

None — BanList is fully functional; ban/is_banned/detect_non_signers all wired and tested.

## Threat Flags

No new threat surface introduced beyond what is documented in the plan's threat model. The `ban_list` field in AppState is not exposed via any HTTP endpoint.

## Next Phase Readiness

- BanList infrastructure is ready for plan 02-02 (ban file persistence across restarts)
- detect_non_signers() is ready to be called from signing timeout handler (plan 02-02)
- AppState.ban_list is accessible from all handlers — ban entries can be written by timeout handlers in future plans

## Self-Check: PASSED

- coordinator/src/round/blame.rs — FOUND
- coordinator/src/round/mod.rs — FOUND
- coordinator/src/api/mod.rs — FOUND
- coordinator/src/api/handlers.rs — FOUND
- shared/src/errors.rs — FOUND
- .planning/phases/02-blame-hardening/02-01-SUMMARY.md — FOUND
- commit 4b4e30a — FOUND
- commit 578146a — FOUND

---
*Phase: 02-blame-hardening*
*Completed: 2026-04-07*
