---
phase: 02-blame-hardening
plan: "02"
subsystem: blame
tags: [rust, blame, ban-list, persistence, jsonl, atomicu32, timeout]

# Dependency graph
requires:
  - phase: 02-blame-hardening/02-01
    provides: BanList struct with ban/is_banned/detect_non_signers/has_missing_outputs

provides:
  - append_ban_entry() writes JSONL ban records to disk (SHA-256 hashed utxo keys)
  - load_unexpired_entries() reads and filters ban file on startup
  - on_signing_timeout() detects non-signers, bans them, transitions Signing→Blame→Idle
  - on_output_reg_timeout() detects missing outputs, transitions OutputReg→Blame→Idle
  - BlameOutcome enum (FullAbort | RestartWithout)
  - OutputRegOutcome enum (AdvanceToSigning | BlameRestart)
  - blame_round_count AtomicU32 in AppState caps consecutive blame rounds at 2
  - Coordinator startup loads unexpired ban entries from ban_list.jsonl

affects: [03-transaction-assembly, main.rs timeout wiring, integration tests]

# Tech tracking
tech-stack:
  added: [tempfile = "3" (dev-dependency for test isolation)]
  patterns:
    - Append-only JSONL ban file with SHA-256 hashed utxo keys (PRIV-02)
    - AtomicU32 for blame round cap (Relaxed ordering, single-coordinator)
    - Blame outcomes as enum consumed by caller (not side-effects)
    - build_router_with_ban_list() separates ban list init from router construction

key-files:
  created:
    - .planning/phases/02-blame-hardening/02-02-SUMMARY.md
  modified:
    - coordinator/src/round/blame.rs
    - coordinator/src/round/output_reg.rs
    - coordinator/src/api/mod.rs
    - coordinator/src/main.rs
    - coordinator/src/config.rs
    - coordinator/Cargo.toml
    - tests/integration/full_round.rs

key-decisions:
  - "on_signing_timeout and BlameOutcome placed in blame.rs not signing.rs — avoids import cycle, keeps blame logic co-located"
  - "build_router_with_ban_list() added alongside build_router() — keeps integration tests using simple build_router() without change"
  - "blame_round_count stored in AppState as Arc<AtomicU32> — shared between timer tasks and AppState without additional locks"

patterns-established:
  - "Timeout handlers return outcome enums, not void — caller in main.rs controls state mutation sequence"
  - "Ban file I/O failures are warn-and-continue, never crash — T-02-09 accept disposition"
  - "Startup ban load uses unwrap_or_else to degrade to empty list on I/O error"

requirements-completed: [BLAME-04, BLAME-05, BLAME-06]

# Metrics
duration: 4min
completed: 2026-04-09
---

# Phase 2 Plan 02: Blame Wiring and Ban File Persistence Summary

**JSONL ban file persistence with SHA-256 hashed utxo keys wired into signing/output-reg timeouts and coordinator startup**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-09T13:32:44Z
- **Completed:** 2026-04-09T13:36:34Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Ban file persistence: `append_ban_entry` creates/appends JSONL records; `load_unexpired_entries` reads and filters at startup; corrupt lines skipped with warn (T-02-06)
- Signing timeout wired: `on_signing_timeout` detects non-signers via `detect_non_signers`, bans them in BanList, appends to ban file, transitions Signing→Blame→Idle
- Output-reg timeout wired: `on_output_reg_timeout` detects missing outputs via `has_missing_outputs`, transitions OutputReg→Blame→Idle (no individual banning — outputs anonymous)
- Blame round cap: `blame_round_count: Arc<AtomicU32>` in AppState; capped at 2 consecutive blame rounds per Pitfall 3 (T-02-07)
- Startup loads unexpired ban entries before serving requests (BLAME-05, BLAME-06)

## Task Commits

1. **Task 1: Ban file persistence (append + load on startup)** - `9d2682e` (feat) — TDD
2. **Task 2: Wire signing timeout and output-reg timeout to blame + round restart** - `89df055` (feat)

## Files Created/Modified

- `coordinator/src/round/blame.rs` — Added BanRecord, hash_utxo_str, append_ban_entry, load_unexpired_entries, BlameOutcome, on_signing_timeout; 4 new persistence tests
- `coordinator/src/round/output_reg.rs` — Added OutputRegOutcome, on_output_reg_timeout
- `coordinator/src/api/mod.rs` — Added blame_round_count to AppState; added build_router_with_ban_list()
- `coordinator/src/main.rs` — Startup ban load; signing timeout task; output-reg timeout task; uses build_router_with_ban_list()
- `coordinator/src/config.rs` — Added ban_file_path to CoordinatorSection (serde default = "ban_list.jsonl")
- `coordinator/Cargo.toml` — Added tempfile = "3" to dev-dependencies
- `tests/integration/full_round.rs` — Added ban_file_path to both CoordinatorSection initializers

## Decisions Made

- `on_signing_timeout` and `BlameOutcome` placed in `blame.rs`, not `signing.rs` — avoids import cycle (signing.rs would depend on blame types defined in signing.rs)
- `build_router_with_ban_list()` added alongside existing `build_router()` — integration tests continue using `build_router()` unchanged
- `blame_round_count` uses `AtomicU32` with `Relaxed` ordering — single-coordinator process, no distributed coordination needed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Integration test CoordinatorSection initializers missing ban_file_path**
- **Found during:** Task 1 (compilation after adding ban_file_path field to CoordinatorSection)
- **Issue:** Two struct initializers in `tests/integration/full_round.rs` did not include the new `ban_file_path` field, causing E0063 compilation errors
- **Fix:** Added `ban_file_path: "ban_list.jsonl".into()` to both initializers
- **Files modified:** `tests/integration/full_round.rs`
- **Verification:** `cargo build -p coordinator` exits 0
- **Committed in:** `9d2682e` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - compilation error from new struct field)
**Impact on plan:** Necessary fix for compilation; no scope creep.

## Issues Encountered

None — both tasks compiled and tested on first attempt after the integration test fix.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Blame detection is fully wired: signing timeout → detect_non_signers → ban → file → Blame→Idle
- Output-reg timeout → has_missing_outputs → Blame→Idle
- Startup loads persisted bans; coordinator restart-safe
- Ready for Phase 3: transaction assembly and broadcast

---
*Phase: 02-blame-hardening*
*Completed: 2026-04-09*
