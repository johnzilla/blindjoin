---
phase: 07-coordinator-dos-hardening
fixed_at: 2026-04-10T13:14:53Z
review_path: .planning/phases/07-coordinator-dos-hardening/07-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 07: Code Review Fix Report

**Fixed at:** 2026-04-10T13:14:53Z
**Source review:** .planning/phases/07-coordinator-dos-hardening/07-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4
- Fixed: 4
- Skipped: 0

## Fixed Issues

### WR-01: Duplicate partial-signature submission silently overwrites previous entry

**Files modified:** `coordinator/src/round/signing.rs`
**Commit:** bf94148
**Applied fix:** Added a `contains_key` check for `utxo_str` in `inner.partial_sigs` before the `insert` call in `process_sign`. Returns `ErrorCode::SessionInvalid` with message "Partial signature already submitted for this input" if a duplicate submission is detected. The check is placed after the UTXO registration check so the session token has already been validated.

---

### WR-02: No size bound on `blinded_token` before RSA blind-sign operation

**Files modified:** `coordinator/src/api/handlers.rs`
**Commit:** fa96220
**Applied fix:** Added a size check in `post_input` immediately after the base64 decode of `blinded_token_bytes`. Rejects any token whose length is not exactly 256 bytes (RSA-2048 modulus) or 512 bytes (RSA-4096 modulus) with a `BAD_REQUEST` / `INVALID_TOKEN` error. The check runs before the ban check and before the write lock is acquired, so oversized blobs are rejected at minimal CPU cost.

---

### WR-03: Output and change addresses stored unvalidated; format errors abort the entire round at PSBT build time

**Files modified:** `coordinator/src/api/handlers.rs`
**Commit:** ad84f38
**Applied fix:** Two validation sites added, both using the existing `parse_address_to_script` helper with the network from config:
- **`post_input`**: `req.change_address` is validated before the write lock is acquired. Returns `BAD_REQUEST` / `INVALID_ADDRESS` if the address fails network validation.
- **`post_output`**: `req.output_address` is validated before the write lock is acquired. Returns `BAD_REQUEST` / `INVALID_ADDRESS` if the address fails network validation.
Both validations occur pre-lock so a single bad-address client gets an immediate 400 without affecting in-progress round state.

---

### WR-04: Fee-per-participant calculation is duplicated in three places with diverging call sites

**Files modified:** `coordinator/src/bitcoin/fee.rs` (new), `coordinator/src/bitcoin/mod.rs`, `coordinator/src/api/handlers.rs`, `coordinator/src/round/signing.rs`
**Commit:** 5c57091
**Applied fix:** Created `coordinator/src/bitcoin/fee.rs` with the single canonical `estimate_fee_share(n: u32, fee_rate: u64) -> u64` function. Added `pub mod fee;` to `coordinator/src/bitcoin/mod.rs`. Removed the private `estimate_fee_share` function from `handlers.rs` and the private `estimate_fee_share_per_participant` function from `signing.rs`. Both files now import `crate::bitcoin::fee::estimate_fee_share`. Also replaced the inline vsize/fee computation in `get_tx` (handlers.rs) with a call to the canonical function. `cargo check` passes with no new errors.

---

_Fixed: 2026-04-10T13:14:53Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
