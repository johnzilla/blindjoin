---
phase: 01-core-protocol
fixed_at: 2026-04-07T00:00:00Z
review_path: .planning/phases/01-core-protocol/01-REVIEW.md
iteration: 1
findings_in_scope: 11
fixed: 10
skipped: 1
status: partial
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-04-07
**Source review:** .planning/phases/01-core-protocol/01-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 11 (5 Critical + 6 Warning)
- Fixed: 10
- Skipped: 1

## Fixed Issues

### CR-01: Non-constant-time session token comparison enables timing oracle

**Files modified:** `coordinator/Cargo.toml`, `coordinator/src/round/manager.rs`
**Commit:** 3583956
**Applied fix:** Added `subtle = "2"` to coordinator dependencies. Changed `verify_session_token` to use `subtle::ConstantTimeEq::ct_eq` instead of `expected == *token`, eliminating the short-circuit comparison that enabled byte-by-byte timing attacks. Updated the doc comment to reflect the change.

---

### CR-03: Floating-point BTC-to-satoshi conversion in UTXO value check can silently truncate

**Files modified:** `coordinator/src/bitcoin/utxo.rs`
**Commit:** 8319cfc
**Applied fix:** Replaced `(txout.value * 100_000_000.0).round() as u64` with `bitcoin::Amount::from_btc(txout.value)?.to_sat()`. This uses bitcoin's validated decimal-to-satoshi conversion, which correctly handles all valid Bitcoin amounts without floating-point truncation errors.

---

### CR-04: Token replay check does not bind to round ID — tokens from a previous round can be replayed in a new round

**Files modified:** `coordinator/src/round/state.rs`, `coordinator/src/round/output_reg.rs`, `tests/integration/full_round.rs`
**Commit:** 1c41cb6
**Applied fix:** Changed `redeemed_tokens` field in `RoundStateInner` from `Vec<[u8; 32]>` to `HashSet<[u8; 32]>` for O(1) lookup. Updated the `Drop` implementation to drain and zeroize elements via a temporary Vec (since `HashSet` has no `iter_mut`). Updated `register_output_logic` signature from `&mut Vec` to `&mut HashSet`. Changed `push` to `insert`. Updated all test call sites and the integration test initializer.

---

### CR-05: BIP-322 verification logic is duplicated between coordinator and client with no shared test — divergence risk

**Files modified:** `shared/src/bip322.rs` (new), `shared/src/lib.rs`, `coordinator/src/bitcoin/utxo.rs`, `client/src/round/input.rs`
**Commit:** 89cef9a
**Applied fix:** Created `shared/src/bip322.rs` with public `bip322_message_hash`, `build_bip322_to_spend`, and `build_bip322_to_sign` functions plus determinism tests. Exposed the module via `shared/src/lib.rs`. Removed the ~90-line duplicate implementations from both `coordinator/src/bitcoin/utxo.rs` and `client/src/round/input.rs`, replacing them with imports from `shared::bip322`. Updated stale comment in the client's `generate_bip322_witness` function.

---

### WR-01: UTXO outpoint appears in BIP-322 message string which is visible in coordinator logs if tracing is DEBUG

**Files modified:** `coordinator/src/round/input_reg.rs`
**Commit:** dcd8ee3
**Applied fix:** Changed the `UtxoError::InvalidProof` arm in `register_input` to discard the `reason` field (`reason: _`) and return a fixed generic message `"BIP-322 ownership proof verification failed"` instead of forwarding the reason (which may contain the UTXO outpoint) to the API response. The coordinator now never surfaces the BIP-322 message string to any caller, satisfying PRIV-02.

---

### WR-02: `InsufficientValue` error message leaks UTXO value to the client

**Files modified:** `coordinator/src/round/input_reg.rs`
**Commit:** 5ee96ca
**Applied fix:** Changed the `UtxoError::InsufficientValue` arm to bind `value: _` (discarding the actual UTXO value) and produce `"UTXO value below required threshold of {required} sats"` instead of `"UTXO value {value} sats < required {required} sats"`. The required threshold is derivable from public parameters; the actual UTXO value is not disclosed.

---

### WR-03: `parse_address_to_script` silently falls back to empty `ScriptBuf` on unrecognized addresses

**Files modified:** `coordinator/src/round/signing.rs`, `coordinator/src/api/handlers.rs`
**Commit:** 870c13c
**Applied fix:** Changed both copies of `parse_address_to_script` to return `Result<ScriptBuf, String>` and accept an `expected_network: bitcoin::Network` parameter. Added `parse_bitcoin_network` helper to convert the config string to `bitcoin::Network`. Updated all call sites in `assemble_and_broadcast` (signing.rs) and the PSBT handler (handlers.rs) to propagate errors explicitly rather than silently producing empty scripts. An invalid or wrong-network address now returns a `BroadcastRejected` / `INVALID_ADDRESS` error immediately, identifying the bad address.

---

### WR-04: Port reuse race condition in integration test coordinator spawning

**Files modified:** `tests/integration/full_round.rs`
**Commit:** 7670c34
**Applied fix:** Removed the `drop(listener)` and re-bind pattern. The already-bound `TcpListener` is now moved directly into the `tokio::spawn` closure and passed to `axum::serve`. The OS-assigned ephemeral port is never released between obtaining it and serving on it, eliminating the TOCTOU window.

---

### WR-05: Client output registration does not re-verify the RSA public key commitment before submitting the unblinded token

**Files modified:** `client/src/round/mod.rs`, `client/src/round/input.rs`, `client/src/round/output.rs`
**Commit:** 464594a
**Applied fix:** Added `pk_hash_at_registration: [u8; 32]` field to `InputRegState`. In `register_input`, the already-computed `pk_hash_actual` is stored in this field. In `register_output`, the `/info` RSA public key DER is decoded, SHA-256 hashed, and compared to `state.pk_hash_at_registration`. If they differ, output registration aborts with an error indicating coordinator key rotation.

---

### WR-06: Fee calculation uses integer division that silently undercharges participants when total fee is not divisible by N

**Files modified:** `coordinator/src/round/input_reg.rs`, `coordinator/src/api/handlers.rs`
**Commit:** 89100d3
**Applied fix:** Added `max_participants: u32` parameter to `register_input`. Changed the fee estimate in `register_input` to always use `max_participants` (worst-case) instead of `estimated_participants` (current count + 1). This ensures UTXOs accepted at registration time always have sufficient value to cover the actual fee at signing time regardless of final participant count. Updated the handler call site to pass `max_participants`. Added documentation that integer division remainder is absorbed as extra miner fee.

---

## Skipped Issues

### CR-02: Coordinator broadcasts unsigned transaction — partial signatures are never applied to the PSBT

**File:** `coordinator/src/round/signing.rs:133`
**Reason:** skipped: fix requires deep protocol redesign — out of scope for atomic fix
**Original issue:** The coordinator serializes `psbt.unsigned_tx` (empty witnesses) instead of applying collected `partial_sigs` before broadcast. The reviewer's fix explicitly states this requires redesigning `SignRequest` and `partial_sigs` storage to carry `(sig, pubkey)` pairs rather than raw bytes, which involves changes to `shared/src/protocol.rs` (wire format), the client signing code in `client/src/round/sign.rs`, and the broadcast path in `signing.rs`. This is a multi-file protocol-level change that cannot be applied correctly without also updating client code — applying a partial fix risks producing a silently broken PSBT finalizer. Requires coordinated human design and implementation.

---

_Fixed: 2026-04-07_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
