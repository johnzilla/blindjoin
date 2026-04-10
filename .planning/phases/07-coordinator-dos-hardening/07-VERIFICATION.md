---
phase: 07-coordinator-dos-hardening
verified: 2026-04-10T12:00:00Z
status: passed
score: 9/9 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 6/9
  gaps_closed:
    - "post_input uses the cached signer from RoundStateInner instead of calling from_der_secret_key()"
    - "RSA key DER bytes remain in RoundStateInner for zeroize-on-drop (no per-request clone)"
  gaps_remaining: []
  regressions: []
---

# Phase 7: Coordinator DoS Hardening Verification Report

**Phase Goal:** Input registration handlers cannot serialize concurrent participants behind each other's RPC latency or key deserialization cost
**Verified:** 2026-04-10T12:00:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure (07-03)

## Goal Achievement

### Observable Truths

**From 07-01-PLAN.md (AVAIL-02):**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | RsaBlindSigner is constructed once per round at round initialization, not per-request | VERIFIED | `rsa_signer: RsaBlindSigner` field at state.rs:84; all test construction sites populate it at setup. `grep "from_der_secret_key" handlers.rs` returns zero matches in production code. |
| 2 | post_input uses the cached signer from RoundStateInner instead of calling from_der_secret_key() | VERIFIED | handlers.rs:208 calls `register_input(&mut guard, &utxo, ...)` with no signer argument. No `from_der_secret_key` anywhere in handlers.rs. `register_input` accesses `inner.rsa_signer.blind_sign` directly at input_reg.rs:64. |
| 3 | post_output uses the cached signer from RoundStateInner instead of calling from_der_secret_key() | VERIFIED | handlers.rs:318-321: `guard.inner.as_ref()...rsa_signer.public_key.clone()` — cached signer used, no from_der_secret_key call. |
| 4 | RSA key DER bytes remain in RoundStateInner for zeroize-on-drop | VERIFIED | `rsa_signing_key: Vec<u8>` retained in RoundStateInner (state.rs:77). Drop impl zeroes it (state.rs:104). `grep "rsa_signing_key" handlers.rs` returns zero matches — no per-request clone. |
| 5 | All existing tests pass with no behavioral change | VERIFIED | `rsa_signer_consistent_with_key_bytes` test substantive at state.rs:266-293. Test call sites updated in input_reg.rs:162,207 (no signer arg). `make_input_reg_state` correctly populates `rsa_signer` field. |

**From 07-02-PLAN.md (AVAIL-01):**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 6 | post_input completes the bitcoind RPC call before acquiring the RoundState write lock | VERIFIED | handlers.rs: `validate_utxo` called at line 142 (`.await`), `state.round.write().await` at line 169 — RPC clearly precedes write lock. |
| 7 | A slow or hung bitcoind does not block other participants from calling post_input concurrently | VERIFIED | Follows from truth 6 — concurrent participants are not queued behind each other's RPC latency. |
| 8 | Phase and double-registration are re-checked under write lock after RPC completes (TOCTOU prevention) | VERIFIED | handlers.rs:172 re-checks phase under write lock. input_reg.rs:54-60: `contains_key` re-check under caller's write lock is authoritative TOCTOU guard. |
| 9 | register_input no longer performs async I/O — it only mutates state under caller's write lock | VERIFIED | `pub fn register_input` at input_reg.rs:34 — plain fn, not async. No `.await` in function body. Pure state mutation + `inner.rsa_signer.blind_sign`. |

**Score: 9/9 truths verified**

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `coordinator/src/round/state.rs` | RoundStateInner with rsa_signer field | VERIFIED | `pub rsa_signer: RsaBlindSigner` at line 84. Arch note comment present (lines 78-83). AVAIL-02 consistency test at line 266. |
| `coordinator/src/api/handlers.rs` | post_input and post_output using cached signer | VERIFIED | post_input: no signer arg to register_input, no from_der_secret_key, no rsa_signing_key reference. post_output: `rsa_signer.public_key.clone()` at line 321. |
| `coordinator/src/round/input_reg.rs` | register_input with no signer param, accesses inner.rsa_signer directly | VERIFIED | Signature at line 34: no signer param. `inner.rsa_signer.blind_sign` at line 64. Both test call sites updated (no signer arg at lines 162, 207). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| coordinator/src/round/state.rs | coordinator/src/round/input_reg.rs | inner.rsa_signer.blind_sign | VERIFIED | input_reg.rs:64: `inner.rsa_signer.blind_sign(&blind_msg)` — direct field access, no signer parameter. |
| coordinator/src/round/state.rs | coordinator/src/api/handlers.rs | guard.inner.as_ref().rsa_signer | VERIFIED | handlers.rs:318-321: `inner.as_ref()...rsa_signer.public_key.clone()` in post_output. post_input accesses via register_input's direct inner access. |
| coordinator/src/blind/rsa.rs | coordinator/src/round/state.rs | RsaBlindSigner stored in RoundStateInner | VERIFIED | Field at state.rs:84, all construction sites in test helpers populate it. |
| coordinator/src/api/handlers.rs | coordinator/src/bitcoin/utxo.rs | validate_utxo called before write lock | VERIFIED | validate_utxo at line 142, round.write() at line 169. |

### Data-Flow Trace (Level 4)

Not applicable — these are handler/state-mutation functions, not data-rendering components.

### Behavioral Spot-Checks

| Behavior | Check | Status |
|----------|-------|--------|
| from_der_secret_key absent in handlers | `grep "from_der_secret_key" coordinator/src/api/handlers.rs` → zero matches | PASS |
| rsa_signing_key not cloned in handlers | `grep "rsa_signing_key" coordinator/src/api/handlers.rs` → zero matches | PASS |
| inner.rsa_signer.blind_sign in input_reg | `grep "inner.rsa_signer" coordinator/src/round/input_reg.rs` → line 64 match | PASS |
| register_input is sync fn | No `async fn register_input` in input_reg.rs | PASS |
| validate_utxo before write lock | validate_utxo line 142, round.write() line 169 | PASS |
| post_output uses cached signer | handlers.rs:321 uses `.rsa_signer.public_key.clone()` | PASS |
| rsa_signer field in state | state.rs:84 field present, test at state.rs:266 substantive | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| AVAIL-01 | 07-02-PLAN.md | post_input performs async bitcoind RPC call before acquiring RoundState write lock | SATISFIED | validate_utxo at handlers.rs:142; round.write() at handlers.rs:169. |
| AVAIL-02 | 07-01-PLAN.md | RSA private key is parsed once at round creation; handlers reuse the parsed RsaBlindSigner | SATISFIED | rsa_signer field in RoundStateInner. post_input: register_input accesses inner.rsa_signer directly at input_reg.rs:64. post_output: rsa_signer.public_key.clone() at handlers.rs:321. No from_der_secret_key in any production handler path. |

No orphaned requirements. AVAIL-01 and AVAIL-02 are the only Phase 7 requirements in REQUIREMENTS.md, both satisfied.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| coordinator/src/round/input_reg.rs | 122 | `from_der_secret_key` in test helper `make_input_reg_state` | Info | Test-only setup code simulating round initialization; not in production hot path. Acceptable. |

No blockers. No production-path anti-patterns.

### Human Verification Required

None — all critical behaviors are verifiable programmatically from the codebase.

### Gaps Summary

All gaps from the previous verification are closed:

**Gap 1 (post_input from_der_secret_key) — CLOSED.** Plan 07-03 restructured `register_input` to remove the `signer: &RsaBlindSigner` parameter entirely. The function now accesses `inner.rsa_signer.blind_sign` directly at input_reg.rs:64. The handler at handlers.rs:208 calls `register_input` with no signer argument. `grep "from_der_secret_key" handlers.rs` returns zero matches.

**Gap 2 (rsa_signing_key.clone per request) — CLOSED.** By removing the signer reconstruction block from post_input, the per-request `rsa_signing_key.clone()` was also eliminated. `grep "rsa_signing_key" handlers.rs` returns zero matches. The single DER copy in RoundStateInner is the only one, and it is zeroed by the Drop impl.

**AVAIL-01 and AVAIL-02 are both fully satisfied. Phase 7 goal achieved.**

---

_Verified: 2026-04-10T12:00:00Z_
_Verifier: Claude (gsd-verifier)_
