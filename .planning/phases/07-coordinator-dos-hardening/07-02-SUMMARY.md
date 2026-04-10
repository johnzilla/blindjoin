---
phase: 07-coordinator-dos-hardening
plan: "02"
subsystem: coordinator/api+round
tags: [dos-hardening, avail-01, lock-ordering, rpc, blind-signatures, performance]
dependency_graph:
  requires: [AVAIL-02-cached-rsa-signer]
  provides: [AVAIL-01-rpc-before-write-lock]
  affects: [coordinator/src/round/input_reg.rs, coordinator/src/api/handlers.rs]
tech_stack:
  added: []
  patterns: [validate-then-lock, toctou-recheck-under-write-lock, pre-lock-snapshot]
key_files:
  created: []
  modified:
    - coordinator/src/round/input_reg.rs
    - coordinator/src/api/handlers.rs
decisions:
  - "validate_utxo called pre-lock under read-lock snapshot; TOCTOU re-check inside register_input is authoritative (D-02)"
  - "Signer reconstructed from DER bytes inside post_input before write lock to avoid borrow conflict with &mut guard"
metrics:
  duration_seconds: 178
  completed_date: "2026-04-10"
  tasks_completed: 3
  files_modified: 2
---

# Phase 7 Plan 2: Move validate_utxo Pre-Lock (AVAIL-01) Summary

**One-liner:** Eliminated bitcoind RPC serialization bottleneck by moving `validate_utxo` out of the `RoundState` write lock — concurrent `post_input` calls no longer queue behind each other's RPC latency.

## What Was Built

Restructured `post_input` so the async bitcoind RPC call (`validate_utxo`) runs before acquiring `state.round.write()`. `register_input` became a pure synchronous state mutation function — no `async`, no `BitcoinRpc` parameter, no RPC I/O. A TOCTOU double-registration re-check remains inside `register_input` under the write lock (D-02). Unit tests confirm both AVAIL-01 behavior (sync function) and the TOCTOU guard.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Refactor register_input to pure synchronous state mutation | 2615027 | coordinator/src/round/input_reg.rs |
| 2 | Restructure post_input to validate UTXO before acquiring write lock | 2615027 | coordinator/src/api/handlers.rs |
| 3 | Unit tests — verify register_input is synchronous and lock ordering | 2615027 | coordinator/src/round/input_reg.rs |

Note: Tasks 1-3 committed together because the handler change (Task 2) was required to make the new signature compile.

## Verification Results

- `cargo check --package coordinator` — zero errors
- `cargo test --package coordinator --lib` — 52 passed, 0 failed
- `cargo test --package coordinator --lib -- input_reg::tests` — 4 passed, 0 failed
- `grep -n "validate_utxo(" coordinator/src/round/input_reg.rs` — zero production code matches (only in comments)
- `grep -n "async fn register_input" coordinator/src/round/input_reg.rs` — zero matches (function is sync)
- `validate_utxo` at handlers.rs:142, `round.write()` at handlers.rs:170 — RPC call confirmed before lock

## Lock Ordering (AVAIL-01)

```
post_input flow after this plan:
  1. read lock: phase check → drop
  2. parse utxo, decode blinded_token, ban check
  3. decode ownership proof
  4. read lock: build registered snapshot + capture round_id → drop
  5. compute fee_share (no lock)
  6. validate_utxo(...).await  ← RPC HERE, no lock held (AVAIL-01)
  7. write lock acquired
  8. re-check phase (TOCTOU)
  9. check round not full, check inner.is_some()
 10. reconstruct signer from DER (no RPC)
 11. register_input() → sync state mutation + TOCTOU double-reg check (D-02)
 12. advance phase if full
 13. return response
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Missing `#[derive(Debug)]` on `InputRegResult`**

- **Found during:** Task 3 (test compilation)
- **Issue:** `unwrap_err()` requires `T: Debug` on `Result<T, E>`. `InputRegResult` had no `Debug` derive, so test assertions with `assert!(result.is_ok(), ...)` failed to compile.
- **Fix:** Added `#[derive(Debug)]` to `InputRegResult` struct.
- **Files modified:** `coordinator/src/round/input_reg.rs`
- **Commit:** 2615027

**2. [Rule 1 - Deviation] Signer borrow pattern differs from plan spec**

- **Found during:** Task 2 (implementing handlers.rs changes)
- **Issue:** Plan spec suggested accessing `inner.rsa_signer` directly (from 07-01 pattern). After making `register_input` take a `&RsaBlindSigner` param, the borrow conflict re-emerged: `&inner.rsa_signer` (immutable) cannot coexist with `&mut guard` (mutable) passed to `register_input`.
- **Fix:** Reconstruct signer from `guard.inner.as_ref().unwrap().rsa_signing_key.clone()` (DER bytes) before the write lock body, matching the working pattern established in 07-01. This adds one RSA key deserialization per `post_input` call — acceptable overhead compared to the RPC latency it replaces.
- **Files modified:** `coordinator/src/api/handlers.rs`
- **Commit:** 2615027

### Pre-existing Issues (Out of Scope, not touched)

Integration tests in `tests/integration/full_round.rs` continue to have pre-existing compile errors (missing `tor_mode`, `discovery` fields, wrong `poll_until_phase` arity). Unchanged from 07-01. Logged to deferred-items.

## Known Stubs

None. All production paths are fully wired. The pre-lock registered snapshot is intentionally approximate (stale-safe by design) with the TOCTOU re-check as the authoritative guard.

## Threat Flags

None. No new network endpoints or trust boundary surfaces introduced. The RPC call was moved earlier in the same handler — not exposed to new attack surface.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| coordinator/src/round/input_reg.rs | FOUND |
| coordinator/src/api/handlers.rs | FOUND |
| 07-02-SUMMARY.md | FOUND |
| commit 2615027 | FOUND |
| register_input is sync fn | CONFIRMED (no async, no .await in production code) |
| validate_utxo before round.write() | CONFIRMED (line 142 vs 170 in handlers.rs) |
| TOCTOU re-check in register_input | CONFIRMED (utxo_str contains_key check before blind_sign) |
| 52 lib tests pass | CONFIRMED |
