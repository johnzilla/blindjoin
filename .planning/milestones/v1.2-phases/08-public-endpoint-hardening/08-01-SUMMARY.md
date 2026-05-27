---
phase: 08-public-endpoint-hardening
plan: 01
subsystem: config
tags: [config, foundation, tower, tower-governor, rate-limit, dos-mitigation, cargo-deps]

# Dependency graph
requires: []
provides:
  - "CoordinatorSection extended with 4 operator-tunable DoS-mitigation knobs (D-04): rate_limit_info_per_min (60), rate_limit_writes_per_min (30), request_timeout_secs (30), max_concurrent_connections (256)"
  - "BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN, ...__RATE_LIMIT_WRITES_PER_MIN, ...__REQUEST_TIMEOUT_SECS, ...__MAX_CONCURRENT_CONNECTIONS env-var overlays (automatic via existing Environment::with_prefix source)"
  - "tower_governor 0.8.0 dependency added to coordinator/Cargo.toml — ready for use by Plan 02"
  - "tower-http \"timeout\" feature enabled — ready for use by Plan 02"
affects: [08-02-rate-limit-and-timeout, 08-03-connection-cap, 08-04-integration-test]

# Tech tracking
tech-stack:
  added:
    - "tower_governor = \"0.8\" (per-route rate limiting via GCRA/token-bucket; tower-native; axum 0.8 compatible)"
    - "tower-http \"timeout\" feature flag (enables tower_http::timeout::TimeoutLayer for HTTP-response-shaped timeouts)"
  patterns:
    - "Config field + default-fn pairing extended (mirror of existing default_ban_file_path pattern)"

key-files:
  created: []
  modified:
    - "coordinator/Cargo.toml — added tower_governor = \"0.8\"; extended tower-http features [\"limit\"] → [\"limit\", \"timeout\"]"
    - "coordinator/src/config.rs — added 4 fields + 4 default-fns to CoordinatorSection; extended with_defaults() literal"
    - "tests/integration/round_bootstrap.rs — extended CoordinatorSection literal at line 117 with 4 new fields"
    - "tests/integration/full_round.rs — extended CoordinatorSection literals at 4 sites (lines 58, 448, 814, 1279)"

key-decisions:
  - "Field defaults locked at CONTEXT D-02/D-04 values: 60/30/30/256 — no Claude discretion"
  - "Fields inserted immediately before tor_mode in CoordinatorSection (keeps tor_mode last, matches PATTERNS shape)"
  - "Test literals get the SAME default values (60/30/30/256) — not loosened — to preserve test semantics; Plan 04 will override with TIGHT limits in the new rate_limiting.rs test"
  - "No changes to load() in config.rs — env-var overlay is fully automatic via existing Environment::with_prefix(\"BLINDJOIN\").separator(\"__\") source"
  - "human-verify checkpoint for tower_governor legitimacy deferred to Plan 02 Task 1 (per RESEARCH §Package Legitimacy Audit) — this plan only declares the dep, no use yet"

patterns-established:
  - "DoS-mitigation knob naming: rate_limit_<route_class>_per_min for per-minute global rate limits; <secs|connections> suffix for non-rate knobs"
  - "Config-foundation pattern: when downstream plans need new struct fields, foundation plan adds them with defaults and updates all in-tree literals so cargo build --all-targets keeps passing"

requirements-completed: []

# Metrics
duration: 5min
completed: 2026-05-26
---

# Phase 8 Plan 01: Public-endpoint hardening foundation Summary

**Added tower_governor 0.8 dependency, enabled tower-http timeout feature, and extended CoordinatorSection with four DoS-mitigation knobs (60/30/30/256) — config surface ready for Plans 02 and 03 to wire middleware.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-05-26T03:44:29Z
- **Completed:** 2026-05-26T03:49:15Z
- **Tasks:** 3 of 3 complete
- **Files modified:** 4

## Accomplishments

- `tower_governor = "0.8"` declared in `coordinator/Cargo.toml`; `cargo tree -p coordinator` confirms `tower_governor v0.8.0` resolves cleanly. Compiles transitively through the existing axum 0.8 + tower 0.5 graph with no version conflicts.
- `tower-http` features extended from `["limit"]` to `["limit", "timeout"]`. `tower_http::timeout::TimeoutLayer` is now importable for Plan 02.
- `CoordinatorSection` gained four operator-tunable DoS knobs per CONTEXT D-04, each with `#[serde(default = "default_xxx")]` annotation pointing at a matching default-fn returning the locked literal (60/30/30/256). `BLINDJOIN__COORDINATOR__*` env-var overlays are inherited automatically via the existing `Environment::with_prefix("BLINDJOIN").separator("__")` source — zero changes to `load()`.
- All 5 in-tree `CoordinatorSection { ... }` struct literals (1 in `round_bootstrap.rs`, 4 in `full_round.rs`) extended with the same default values so `cargo build --all-targets` keeps exiting 0.
- No runtime behavior change. Coordinator binary builds and the 56 coordinator-lib unit tests all pass.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add tower_governor dependency and tower-http timeout feature** — `abfcf78` (chore)
2. **Task 2: Add four operator-tunable knobs to CoordinatorSection with serde defaults** — `fdabf37` (chore)
3. **Task 3: Update existing CoordinatorSection literals in tests so they keep compiling** — `0dfa789` (chore)

## Files Created/Modified

- `coordinator/Cargo.toml` — Added `tower_governor = "0.8"` line; extended `tower-http` features to `["limit", "timeout"]`.
- `Cargo.lock` — Updated lockfile to record `tower_governor v0.8.0` and its transitive deps.
- `coordinator/src/config.rs` — Added 4 fields to `CoordinatorSection` (`rate_limit_info_per_min: u32`, `rate_limit_writes_per_min: u32`, `request_timeout_secs: u64`, `max_concurrent_connections: u32`), 4 matching default-fns, and 4 new entries in the `with_defaults()` literal.
- `tests/integration/round_bootstrap.rs` — Extended the single `CoordinatorSection { ... }` literal (line ~117) with the 4 new fields.
- `tests/integration/full_round.rs` — Extended 4 `CoordinatorSection { ... }` literals (lines ~58, ~448, ~814, ~1279) with the 4 new fields.

## Env-var overlays operators can now use

These reach `CoordinatorSection` automatically (no `load()` change required) via the existing prefix + `__` separator:

- `BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN` → `rate_limit_info_per_min` (default 60)
- `BLINDJOIN__COORDINATOR__RATE_LIMIT_WRITES_PER_MIN` → `rate_limit_writes_per_min` (default 30)
- `BLINDJOIN__COORDINATOR__REQUEST_TIMEOUT_SECS` → `request_timeout_secs` (default 30)
- `BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS` → `max_concurrent_connections` (default 256)

## Cargo.toml diff applied

```diff
 [dependencies]
 # ...
-tower-http = { version = "0.6", features = ["limit"] }
+tower-http = { version = "0.6", features = ["limit", "timeout"] }
+tower_governor = "0.8"
 # ...
```

## Verification evidence

- `cargo build --all-targets` exits 0 (clean after Task 3).
- `cargo test --no-run` exits 0 — all test executables compile, including the existing integration tests.
- `cargo test -p coordinator --lib` — 56 passed, 0 failed.
- `cargo tree -p coordinator | grep tower_governor` → `├── tower_governor v0.8.0`.
- `grep -c "rate_limit_info_per_min\|rate_limit_writes_per_min\|request_timeout_secs\|max_concurrent_connections" coordinator/src/config.rs` → **16** (4 fields × 4 sites: struct decl + serde-default attr + default-fn name + with_defaults literal); exceeds the ≥12 acceptance criterion.
- `grep -c "rate_limit_info_per_min" tests/integration/round_bootstrap.rs` → **1** (the single site).
- `grep -c "rate_limit_info_per_min" tests/integration/full_round.rs` → **4** (one per site).

## Decisions Made

None — plan executed exactly as written. All field names, types, and default values were locked by CONTEXT D-02/D-04; placement (before `tor_mode`) was prescribed by PATTERNS; test-literal values follow the same locked defaults.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None. The `cargo build --all-targets` E0063 errors after Task 2 were expected — they are the precise signal that Task 3 work is needed (5 missing-field errors, one per `CoordinatorSection { ... }` test literal site, exactly matching the planner-listed sites).

## User Setup Required

None — no external service configuration required. The four new env-var overlays are opt-in; defaults match the production-safe values from CONTEXT D-02/D-04.

## Next Phase Readiness

**Plan 02 (rate-limit + timeout middleware) can now compile:** it can `use tower_governor::*` and `use tower_http::timeout::TimeoutLayer` without import errors, and can read `cfg.coordinator.rate_limit_info_per_min`, `cfg.coordinator.rate_limit_writes_per_min`, and `cfg.coordinator.request_timeout_secs` from the extended config struct.

**Plan 03 (connection cap) can now compile:** it can read `cfg.coordinator.max_concurrent_connections` from the extended config struct and thread the value into `coordinator/src/network/tor.rs` for the accept-loop semaphore.

**No runtime behavior has changed yet.** The dep graph and config surface are wired; layer construction and accept-loop modification land in Plans 02 and 03 respectively.

**Reminder (deferred from this plan):** Plan 02 Task 1 MUST insert a `checkpoint:human-verify` (per RESEARCH §"Package Legitimacy Audit" — slopcheck unavailable in env) before the first `use tower_governor::*` import lands, so the operator eyeballs the crate page (https://crates.io/crates/tower_governor, author benwis, MIT/Apache) once before any runtime use. Plan 01 declared the dependency but no code imports it yet, so deferring the check is correct.

## Self-Check

- Created files: none planned (this plan is modification-only).
- Modified files exist:
  - `coordinator/Cargo.toml` — FOUND
  - `coordinator/src/config.rs` — FOUND
  - `tests/integration/round_bootstrap.rs` — FOUND
  - `tests/integration/full_round.rs` — FOUND
- Commits:
  - `abfcf78` — FOUND (Task 1)
  - `fdabf37` — FOUND (Task 2)
  - `0dfa789` — FOUND (Task 3)

## Self-Check: PASSED

---
*Phase: 08-public-endpoint-hardening*
*Completed: 2026-05-26*
