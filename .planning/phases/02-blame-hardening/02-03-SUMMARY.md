---
phase: 02-blame-hardening
plan: "03"
subsystem: testing
tags: [rust, unit-tests, integration-tests, blame, zeroize, fsm]

# Dependency graph
requires:
  - phase: 02-blame-hardening/02-01
    provides: blame.rs with BanList, detect_non_signers, on_signing_timeout, BlameOutcome
  - phase: 02-blame-hardening/02-02
    provides: ban file persistence, signing timeout wiring in main.rs, build_router_with_ban_list
provides:
  - "TEST-06: 3 signing unit tests (invalid token, wrong outpoint, records partial sig)"
  - "PRIV-01: zeroize confirmation test in state.rs (transition_to_idle_clears_inner annotated)"
  - "TEST-07: 4 blame unit tests in signing.rs (non_signer_banned, cap_triggers_full_abort, missing_output_triggers_blame, round_restart_after_blame)"
  - "TEST-07 integration test: blame_non_signer_timeout in full_round.rs (3 clients, 1 non-signer, signing timeout fires, banned UTXO confirmed)"
  - "FSM bug fix: OutputReg→Blame transition added to can_transition_to"
affects: [03-client, 04-docker, testing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Blame unit tests live in signing.rs test block using crate::round::blame imports (no import cycle)"
    - "Integration blame test mirrors main.rs timer wiring: spawn_coordinator_with_blame shares ban_list Arc for direct assertion"
    - "FSM transitions: OutputReg→Blame now valid (required by on_output_reg_timeout)"

key-files:
  created: []
  modified:
    - coordinator/src/round/signing.rs
    - coordinator/src/round/state.rs
    - tests/integration/full_round.rs

key-decisions:
  - "Blame unit tests (TEST-07) placed in signing.rs test block rather than blame.rs — avoids circular module imports; all blame imports use crate::round::blame::{on_signing_timeout, BlameOutcome, ...}"
  - "Integration test uses shared Arc<RwLock<BanList>> rather than HTTP polling for ban assertion — faster, no need for /round/input retry after Idle restart"
  - "spawn_coordinator_with_blame helper mirrors main.rs signing timeout task — reuses same on_signing_timeout path as production"

patterns-established:
  - "Integration test helpers share internal state (ban_list Arc) for direct assertion of protocol invariants"

requirements-completed: [PRIV-01, TEST-06, TEST-07]

# Metrics
duration: 4min
completed: 2026-04-07
---

# Phase 02 Plan 03: Blame & Signing Test Suite Summary

**7 new tests (3 TEST-06 signing + 4 TEST-07 blame unit + 1 blame integration) verify non-signer banning, FSM zeroing (PRIV-01), and end-to-end blame timeout via shared BanList Arc**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-07T00:39:05Z
- **Completed:** 2026-04-07T00:43:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- TEST-06: process_sign unit tests covering invalid token, wrong outpoint, and Recorded partial sig path
- PRIV-01: `transition_to_idle_clears_inner` annotated with PRIV-01 contract comments in state.rs
- TEST-07: 4 blame unit tests confirm non-signer banning, cap→FullAbort, missing output→BlameRestart, post-blame state=Idle
- TEST-07 integration: `blame_non_signer_timeout` — 3 clients, 1 never signs, 2s timeout fires, coordinator returns to Idle, non-signer UTXO confirmed banned in shared BanList
- Bug fix: `OutputReg→Blame` FSM edge added — `on_output_reg_timeout` was silently failing (Rule 1)

## Task Commits

1. **Task 1: Signing unit tests (TEST-06) + zeroize confirmation (PRIV-01)** - `3948a38` (test)
2. **Task 2: Blame unit tests (TEST-07) + blame integration test** - `17db37e` (test)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `coordinator/src/round/signing.rs` — Added `#[derive(Debug)]` to SignResult; added 3 TEST-06 signing unit tests + 4 TEST-07 blame unit tests in `#[cfg(test)]` block
- `coordinator/src/round/state.rs` — Annotated `transition_to_idle_clears_inner` with PRIV-01 comments; added `OutputReg→Blame` FSM transition
- `tests/integration/full_round.rs` — Added `blame_non_signer_timeout` integration test and `spawn_coordinator_with_blame` helper

## Decisions Made
- Blame unit tests (TEST-07) placed in signing.rs test block rather than blame.rs — avoids circular module imports; all blame imports use `crate::round::blame::{on_signing_timeout, BlameOutcome, ...}`
- Integration test uses shared `Arc<RwLock<BanList>>` for direct ban assertion rather than HTTP retry — cleaner and faster
- `spawn_coordinator_with_blame` mirrors main.rs signing timeout task — tests the exact same production code path

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing OutputReg→Blame FSM transition**
- **Found during:** Task 1 (`test_missing_output_triggers_blame`)
- **Issue:** `on_output_reg_timeout` calls `state.transition_to(Phase::Blame)` from OutputReg phase, but `can_transition_to` only allowed `Signing→Blame`. The `let _ =` silently swallowed the Err, leaving state stuck in OutputReg.
- **Fix:** Added `(Phase::OutputReg, Phase::Blame)` edge to `can_transition_to` in state.rs with comment "BLAME-02: missing output → blame from OutputReg"
- **Files modified:** coordinator/src/round/state.rs
- **Verification:** `test_missing_output_triggers_blame` passes; existing FSM tests still pass
- **Committed in:** 3948a38 (Task 1 commit)

**2. [Rule 1 - Bug] Added `#[derive(Debug)]` to SignResult**
- **Found during:** Task 1 (compile error)
- **Issue:** `Result::unwrap_err()` requires `T: Debug`; SignResult had no Debug impl
- **Fix:** Added `#[derive(Debug)]` to `SignResult` enum
- **Files modified:** coordinator/src/round/signing.rs
- **Verification:** Compile succeeds; all tests pass
- **Committed in:** 3948a38 (Task 1 commit)

**3. [Rule 1 - Bug] Restructured blame integration test to avoid non-Clone InputRegState**
- **Found during:** Task 2 (design issue)
- **Issue:** Plan outline collected `InputRegState` from all 3 clients then tried to clone them for signers 1+2. `InputRegState` does not implement `Clone` (contains `blind_rsa_signatures::Secret`).
- **Fix:** Each client task runs its full phase sequence end-to-end with a `should_sign` boolean flag — no clone needed
- **Files modified:** tests/integration/full_round.rs
- **Verification:** `blame_non_signer_timeout` compiles and passes
- **Committed in:** 17db37e (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (2 Rule 1 bugs, 1 Rule 1 design correction)
**Impact on plan:** All fixes necessary for correctness. The FSM fix corrects a pre-existing silent bug in on_output_reg_timeout. No scope creep.

## Issues Encountered
None beyond the auto-fixed deviations above.

## Known Stubs
None — all tests assert real behavior against in-process coordinator with live BanList.

## Next Phase Readiness
- Phase 02 blame hardening is complete: ban list, file persistence, timeout wiring, and full test coverage
- All 47 tests pass (44 coordinator unit + 3 integration)
- Ready for Phase 03 (client CLI) or Phase 04 (Docker Compose stack)

---
*Phase: 02-blame-hardening*
*Completed: 2026-04-07*
