---
phase: 07-coordinator-dos-hardening
plan: "01"
subsystem: coordinator/round
tags: [dos-hardening, avail-02, rsa, blind-signatures, performance]
dependency_graph:
  requires: []
  provides: [AVAIL-02-cached-rsa-signer]
  affects: [coordinator/src/round/state.rs, coordinator/src/api/handlers.rs, coordinator/src/round/input_reg.rs]
tech_stack:
  added: []
  patterns: [cached-signer-in-state, clone-public-key-for-borrow-split]
key_files:
  created: []
  modified:
    - coordinator/src/round/state.rs
    - coordinator/src/api/handlers.rs
    - coordinator/src/round/input_reg.rs
    - coordinator/src/round/signing.rs
    - tests/integration/full_round.rs
decisions:
  - "Remove signer param from register_input() — access inner.rsa_signer directly to avoid borrow conflict"
  - "Clone BjPublicKey in post_output to release immutable borrow before mutable guard borrow"
metrics:
  duration_seconds: 404
  completed_date: "2026-04-10"
  tasks_completed: 4
  files_modified: 5
---

# Phase 7 Plan 1: Cache RSA Blind Signer in RoundStateInner (AVAIL-02) Summary

**One-liner:** Eliminated per-request 2048-bit RSA key deserialization by caching `RsaBlindSigner` once in `RoundStateInner`, with `BjPublicKey::clone()` pattern for borrow-split in `post_output`.

## What Was Built

Added `rsa_signer: RsaBlindSigner` field to `RoundStateInner` so that `post_input` and `post_output` handlers no longer call `RsaBlindSigner::from_der_secret_key()` on every request. The raw `rsa_signing_key: Vec<u8>` bytes are retained for zeroize-on-drop (D-07). A new unit test `rsa_signer_consistent_with_key_bytes` guards against key/signer mismatch regressions.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add rsa_signer field to RoundStateInner | 2a23e18 | coordinator/src/round/state.rs |
| 2 | Update all test construction sites | 885bfbe | coordinator/src/round/signing.rs, tests/integration/full_round.rs |
| 3 | Replace per-request from_der_secret_key calls | dc24922 | coordinator/src/api/handlers.rs, coordinator/src/round/input_reg.rs |
| 4 | Verify workspace tests pass | (no commit — verification only) | — |

## Verification Results

- `cargo build --package coordinator --lib` — zero errors
- `cargo test --package coordinator --lib` — 48 passed, 0 failed
- `cargo test --workspace --lib` — 58 passed (48 coordinator + 10 shared), 0 failed
- `grep -n "from_der_secret_key" coordinator/src/api/handlers.rs` — zero matches
- `grep -n "rsa_signer" coordinator/src/round/state.rs` — field in struct + test

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Borrow conflict: &RsaBlindSigner from guard while &mut guard passed to register_input**

- **Found during:** Task 3
- **Issue:** Plan specified `let signer = &guard.inner.as_ref().unwrap().rsa_signer` then passing `&mut guard` to `register_input`. Rust borrow checker rejects this: the reference into `guard.inner` conflicts with the mutable reborrow of `guard`.
- **Fix:** Removed the `signer: &RsaBlindSigner` parameter from `register_input()` entirely. The function now accesses `inner.rsa_signer` directly from the `state` it already mutably borrows. This is cleaner than the plan's approach and eliminates the conflict.
- **Files modified:** `coordinator/src/round/input_reg.rs`, `coordinator/src/api/handlers.rs`
- **Commit:** dc24922

**2. [Rule 3 - Blocking] Missing libsqlite3.so symlink for binary linking**

- **Found during:** Task 1 verification
- **Issue:** `cargo build --package coordinator` (binary) failed with `unable to find library -lsqlite3`. The system has `libsqlite3.so.0` but not the unversioned `libsqlite3.so` symlink (sqlite-devel not installed).
- **Fix:** Created `~/.local/lib/libsqlite3.so -> /usr/lib64/libsqlite3.so.0` and used `RUSTFLAGS="-L /home/john/.local/lib"` for all build/test commands. All tests are run via `--lib` flag (lib tests don't link the binary) or with the RUSTFLAGS workaround for full binary builds.
- **Files modified:** None (system-level workaround)
- **Commit:** N/A (runtime environment fix)

### Pre-existing Issues (Out of Scope)

Integration tests in `tests/integration/full_round.rs` have pre-existing compile errors unrelated to this plan:
- `missing field tor_mode in CoordinatorSection` (lines 80, 468, 832)
- `missing field discovery in CoordinatorConfig` (lines 73, 461, 825)
- `this method takes 3 arguments but 2 arguments were supplied` for `BitcoinRpc::new` (lines 323, 333, 343, 657, 666)

These existed before this plan and are logged to deferred-items.md scope.

## Known Stubs

None. The `rsa_signer` field is populated at all construction sites. No placeholder values flow to production paths.

## Threat Flags

None. No new network endpoints, auth paths, or trust boundary changes introduced. The `rsa_signer` field is accessed only under the existing `RwLock<RoundState>` guard.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| coordinator/src/round/state.rs | FOUND |
| coordinator/src/api/handlers.rs | FOUND |
| coordinator/src/round/input_reg.rs | FOUND |
| 07-01-SUMMARY.md | FOUND |
| commit 2a23e18 (Task 1) | FOUND |
| commit 885bfbe (Task 2) | FOUND |
| commit dc24922 (Task 3) | FOUND |
