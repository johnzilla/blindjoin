---
phase: 07-coordinator-dos-hardening
reviewed: 2026-04-09T00:00:00Z
depth: standard
files_reviewed: 5
files_reviewed_list:
  - coordinator/src/api/handlers.rs
  - coordinator/src/round/input_reg.rs
  - coordinator/src/round/signing.rs
  - coordinator/src/round/state.rs
  - tests/integration/full_round.rs
findings:
  critical: 0
  warning: 4
  info: 3
  total: 7
status: issues_found
---

# Phase 07: Code Review Report

**Reviewed:** 2026-04-09
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

This phase hardens the coordinator against DoS attacks. The five reviewed files cover the full HTTP API surface (`handlers.rs`), the two core round-logic modules (`input_reg.rs`, `signing.rs`), the round state machine (`state.rs`), and a comprehensive integration test suite (`full_round.rs`).

The state machine and cryptographic primitives are solid: FSM transitions are enforced, sensitive state is zeroized on drop, blind-RSA signing is correctly deferred to a cached signer, TOCTOU double-registration is prevented under the write lock, and session tokens gate the signing phase.

Four warnings require attention before shipping:

1. A duplicate partial-signature submission is silently accepted and overwrites the previous entry — a client can replay its own (or a stolen) signing submission without error.
2. The `blinded_token` passed to blind-signing has no size check, so a client can send an arbitrarily large blob under the 64 KB body limit.
3. The `output_address` and `change_address` strings are stored unvalidated at registration time; address-format errors are deferred to `get_tx` / `assemble_and_broadcast`, where a failure aborts the entire round.
4. The fee-per-participant formula appears in three separate copies with slightly different names, creating a maintenance hazard that could produce inconsistent fee estimates at PSBT-build time.

---

## Warnings

### WR-01: Duplicate partial-signature submission silently overwrites previous entry

**File:** `coordinator/src/round/signing.rs:60`

**Issue:** `process_sign` checks that the UTXO is registered and that the session token is valid, but it does not check whether a partial signature for that UTXO has already been recorded. The line `inner.partial_sigs.insert(utxo_str, partial_signature.to_vec())` unconditionally overwrites any existing entry. A participant can therefore re-submit a different (malformed) witness after their first honest submission, potentially replacing a valid signature with garbage and causing `assemble_and_broadcast` to fail for all participants. Even if the attacker has no other participant's session token, they can corrupt their own slot and force a blame round.

**Fix:**
```rust
// Before inserting, check for duplicate submission
if inner.partial_sigs.contains_key(&utxo_str) {
    return Err(ApiError {
        code: ErrorCode::SessionInvalid,
        message: "Partial signature already submitted for this input".into(),
        round_id: Some(round_id_str.to_string()),
    });
}
inner.partial_sigs.insert(utxo_str, partial_signature.to_vec());
```

---

### WR-02: No size bound on `blinded_token` before RSA blind-sign operation

**File:** `coordinator/src/api/handlers.rs:96-99` / `coordinator/src/round/input_reg.rs:65`

**Issue:** The handler decodes the base64 `blinded_token` field and passes the raw bytes directly to `BlindMessage(blinded_token_bytes.to_vec())` and then into the RSA blind-sign operation with no length validation. The 64 KB body limit caps total request size, but a client can craft a request that places the full ~65 000 bytes into this single field. RSA blind-signing of an arbitrarily long message imposes CPU cost proportional to the padded message size and may panic or produce an error depending on the `blind-rsa-signatures` implementation. The correct size for an RSABSSA-SHA384-PSS blinded message is exactly the RSA key modulus size (256 bytes for 2048-bit, 512 bytes for 4096-bit). Reject anything outside that range before acquiring the write lock.

**Fix:**
```rust
// After base64-decoding blinded_token_bytes, before passing to register_input:
const RSA_2048_BYTES: usize = 256;
const RSA_4096_BYTES: usize = 512;
if blinded_token_bytes.len() != RSA_2048_BYTES && blinded_token_bytes.len() != RSA_4096_BYTES {
    return Err(api_error(
        StatusCode::BAD_REQUEST,
        "INVALID_TOKEN",
        format!(
            "blinded_token must be {} or {} bytes (RSA modulus size), got {}",
            RSA_2048_BYTES, RSA_4096_BYTES, blinded_token_bytes.len()
        ),
        None,
    ));
}
```

If you pin to a specific key size at coordinator startup, the constant can be derived from the actual modulus length and the check can be exact.

---

### WR-03: Output and change addresses stored unvalidated; format errors abort the entire round at PSBT build time

**File:** `coordinator/src/api/handlers.rs:358-362` (output address stored raw) and `coordinator/src/round/input_reg.rs:83` (change address stored raw)

**Issue:** Both `post_output` (line 360: `address: req.output_address.clone()`) and `register_input` (line 83: `change_address: change_address.to_string()`) store address strings without parsing or network-validation. The network check only fires later — in `get_tx` for the output registration path and in `assemble_and_broadcast` for the signing path. If any single participant submits a syntactically invalid or wrong-network address, the error surfaces at PSBT construction time, which fails the entire round and forces a blame cycle even though no participant was necessarily malicious. Validating at registration time keeps the error local and allows an immediate 400 response to the offending client.

**Fix for `post_output` (handlers.rs ~line 335):** Parse `req.output_address` through `parse_address_to_script` before acquiring the write lock (mirrors the AVAIL-01 pattern already used for UTXO validation). Return `BAD_REQUEST` if it fails, then store the pre-validated string.

**Fix for `post_input` (handlers.rs ~line 140):** Similarly, validate `req.change_address` before calling `register_input`. The `parse_address_to_script` helper already exists in `handlers.rs`; pass the bitcoin network from config the same way `get_tx` does.

---

### WR-04: Fee-per-participant calculation is duplicated in three places with diverging call sites

**File:** `coordinator/src/api/handlers.rs:577-582`, `coordinator/src/round/signing.rs:249-254`, `coordinator/src/api/handlers.rs:419`

**Issue:** The function body is identical in both modules but the function is defined twice under two different names (`estimate_fee_share` in handlers, `estimate_fee_share_per_participant` in signing). Additionally, `get_tx` at line 419 calls `estimate_fee_share` with `inner.registered_inputs.len() as u32` while `assemble_and_broadcast` calls the other copy with the same argument — but these two code paths produce the transaction and its pre-image independently. Any future change to the fee formula (e.g., switching from a linear vsize model to a proper segwit weight model) must be applied in both places. A discrepancy would cause `get_tx` and `assemble_and_broadcast` to produce PSBTs with mismatched input values, leading to invalid transactions.

**Fix:** Move the function to `shared` or a new `crate::bitcoin::fee` module and import it in both `handlers.rs` and `signing.rs`. Delete the two private copies.

---

## Info

### IN-01: `get_tx` rebuilds and re-estimates the PSBT independently from `assemble_and_broadcast`

**File:** `coordinator/src/api/handlers.rs:384-463`

**Issue:** The `get_tx` handler reconstructs the PSBT from registered inputs/outputs for display to clients. The `assemble_and_broadcast` function in `signing.rs` does the same reconstruction at broadcast time. If any state changes between the two calls (which should not happen under the write lock, but the `get_tx` handler uses a read lock), the PSBT the client signs and the PSBT the coordinator finalizes could theoretically differ. This is currently safe because state is immutable between SIGNING transitions, but it is fragile. Consider caching the canonical PSBT bytes in `RoundStateInner` after the transition to Signing so both paths use the same bytes.

---

### IN-02: `Box::leak` of `corepc_node::Node` in integration tests causes resource accumulation when running the full test suite

**File:** `tests/integration/full_round.rs:271-272`, `611`, `794-795`

**Issue:** The integration tests use `Box::leak` to keep the `corepc_node::Node` alive for the duration of the test. Three separate test functions each leak a node. When the test suite is run with `cargo test --test integration`, all three leaked nodes (each running a separate `bitcoind` process) accumulate for the lifetime of the process. This is intentional and documented in comments, but on CI agents with limited file descriptors or PIDs it can cause later tests to fail when spawning additional processes. Consider using `std::sync::OnceLock` or a test fixture that holds the node in a `static` and drops it via `atexit` / `drop_guard`.

---

### IN-03: Hard-coded regtest WIF keys appear in three test functions

**File:** `tests/integration/full_round.rs:193-197`, `566-570`, `751-755`

**Issue:** The three test WIF private keys are duplicated verbatim across `full_round_three_clients`, `blame_non_signer_timeout`, and `fund_regtest`. Extracting them into a module-level constant array would make future key rotation a single-site change. These are regtest-only keys with zero monetary value, so this is a maintainability issue rather than a security risk.

---

_Reviewed: 2026-04-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
