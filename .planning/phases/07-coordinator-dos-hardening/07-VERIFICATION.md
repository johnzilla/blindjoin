---
phase: 07-coordinator-dos-hardening
verified: 2026-04-10T00:00:00Z
status: gaps_found
score: 6/9 must-haves verified
overrides_applied: 0
gaps:
  - truth: "post_input uses the cached signer from RoundStateInner instead of calling from_der_secret_key()"
    status: failed
    reason: "handlers.rs line 210 calls RsaBlindSigner::from_der_secret_key(&signer_der) inside the write lock on every post_input request. This re-introduces per-request RSA key deserialization, directly contradicting AVAIL-02. The 07-02-SUMMARY acknowledged this as a deviation but accepted it; however the PLAN must-have is explicit and unmet."
    artifacts:
      - path: "coordinator/src/api/handlers.rs"
        issue: "from_der_secret_key called at line 210 inside write lock — rsa_signer field in inner is ignored in post_input"
    missing:
      - "Use inner.rsa_signer directly in post_input, or restructure to avoid the borrow conflict without deserializing the key again. The borrow conflict can be resolved by: (a) restructuring register_input to not take a &RsaBlindSigner param and instead access inner.rsa_signer directly (which is what register_input's doc comment says it does), or (b) cloning the public key before the mutable borrow (as done in post_output)."
  - truth: "RSA key DER bytes remain in RoundStateInner for zeroize-on-drop"
    status: failed
    reason: "Technically the rsa_signing_key Vec<u8> is still in RoundStateInner and the Drop impl does zeroize it (state.rs lines 101-130). However, per-request clone of rsa_signing_key at handlers.rs line 209 means the raw key material leaks to a stack-allocated local on every request, living until the frame unwinds. This is a degraded security posture compared to what the plan intended — the bytes exist in more heap locations than expected."
    artifacts:
      - path: "coordinator/src/api/handlers.rs"
        issue: "Line 209: rsa_signing_key.clone() creates an unzeroized heap allocation per request"
    missing:
      - "Eliminate the per-request clone of rsa_signing_key. Solving the from_der_secret_key gap above also resolves this."
---

# Phase 7: Coordinator DoS Hardening Verification Report

**Phase Goal:** Input registration handlers cannot serialize concurrent participants behind each other's RPC latency or key deserialization cost
**Verified:** 2026-04-10T00:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

**From 07-01-PLAN.md (AVAIL-02):**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | RsaBlindSigner is constructed once per round at round initialization, not per-request | PARTIAL | `rsa_signer: RsaBlindSigner` field exists in `RoundStateInner` (state.rs:84). However, `post_input` reconstructs it via `from_der_secret_key` on every request (handlers.rs:210) — the field is populated but ignored in the hot path. |
| 2 | post_input uses the cached signer from RoundStateInner instead of calling from_der_secret_key() | FAILED | handlers.rs line 210 calls `RsaBlindSigner::from_der_secret_key(&signer_der)` inside the write lock on every `post_input` invocation. The `rsa_signer` field in `inner` is not used here. |
| 3 | post_output uses the cached signer from RoundStateInner instead of calling from_der_secret_key() | VERIFIED | handlers.rs line 327-330: `guard.inner.as_ref()...rsa_signer.public_key.clone()` — uses cached signer, no from_der_secret_key call. |
| 4 | RSA key DER bytes remain in RoundStateInner for zeroize-on-drop | PARTIAL | `rsa_signing_key: Vec<u8>` is in `RoundStateInner` and Drop zeroes it (state.rs:104). But handlers.rs:209 clones it per-request (`rsa_signing_key.clone()`) creating additional unzeroized heap allocations. The drop-level guarantee is weakened. |
| 5 | All existing tests pass with no behavioral change | VERIFIED | Summary reports 52 lib tests pass (48 coordinator + additional). No test failures documented. `rsa_signer_consistent_with_key_bytes` test exists and is substantive (state.rs:266-293). |

**From 07-02-PLAN.md (AVAIL-01):**

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 6 | post_input completes the bitcoind RPC call before acquiring the RoundState write lock | VERIFIED | handlers.rs: `validate_utxo` called at line 142 (`.await`), `state.round.write().await` at line 170 — RPC is clearly before write lock. |
| 7 | A slow or hung bitcoind does not block other participants from calling post_input concurrently | VERIFIED | Follows directly from truth 6 — RPC happens before write lock, so concurrent participants are not queued behind each other's RPC latency. |
| 8 | Phase and double-registration are re-checked under write lock after RPC completes (TOCTOU prevention) | VERIFIED | handlers.rs line 173: phase re-check under write lock. input_reg.rs lines 52-62: `contains_key` check for double-registration under caller's write lock (authoritative TOCTOU guard). |
| 9 | register_input no longer performs async I/O — it only mutates state under caller's write lock | VERIFIED | input_reg.rs: `pub fn register_input` (not async). No `.await` in production code. No `validate_utxo` import. Function body is pure state mutation + blind signing. |

**Score: 6/9 truths verified** (2 failed, 1 partial counted as failed for scoring)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `coordinator/src/round/state.rs` | RoundStateInner with rsa_signer field | VERIFIED | `pub rsa_signer: RsaBlindSigner` at line 84. Arch note comment present. |
| `coordinator/src/api/handlers.rs` | post_input and post_output using cached signer | PARTIAL | `post_output` uses `inner.rsa_signer.public_key.clone()` (correct). `post_input` uses `from_der_secret_key` on cloned DER bytes (not cached signer). Pattern `inner.rsa_signer` appears only in post_output. |
| `coordinator/src/round/input_reg.rs` | register_input as pure state mutation (no async RPC) | VERIFIED | Sync fn, no RPC imports, TOCTOU re-check present. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| coordinator/src/round/state.rs | coordinator/src/api/handlers.rs | guard.inner.as_ref().rsa_signer | PARTIAL | post_output uses `inner.rsa_signer.public_key.clone()` (line 330). post_input does NOT — it calls `from_der_secret_key` instead (line 210). Pattern "inner.rsa_signer" appears only once in handlers.rs. |
| coordinator/src/blind/rsa.rs | coordinator/src/round/state.rs | RsaBlindSigner stored in RoundStateInner | VERIFIED | `rsa_signer: RsaBlindSigner` field exists and is populated in all test construction sites. |
| coordinator/src/api/handlers.rs | coordinator/src/bitcoin/utxo.rs | validate_utxo called before write lock | VERIFIED | validate_utxo at line 142, round.write() at line 170. |
| coordinator/src/api/handlers.rs | coordinator/src/round/input_reg.rs | register_input called under write lock with pre-validated utxo | VERIFIED | register_input called at line 216 after write lock acquired at line 170. |

### Data-Flow Trace (Level 4)

Not applicable — these are handler/mutation functions, not data-rendering components.

### Behavioral Spot-Checks

| Behavior | Check | Status |
|----------|-------|--------|
| register_input is sync fn | `grep "async fn register_input" coordinator/src/round/input_reg.rs` → zero matches | PASS |
| validate_utxo before write lock | validate_utxo line 142, round.write() line 170 | PASS |
| from_der_secret_key absent in handlers | `grep "from_der_secret_key" coordinator/src/api/handlers.rs` → line 210 match | FAIL — still present in post_input |
| rsa_signer field in state | grep shows field at state.rs:84 and test at state.rs:266 | PASS |
| post_output uses cached signer | handlers.rs:330 uses `.rsa_signer.public_key.clone()` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| AVAIL-01 | 07-02-PLAN.md | post_input performs async bitcoind RPC call before acquiring RoundState write lock | SATISFIED | validate_utxo at handlers.rs:142; round.write() at handlers.rs:170 |
| AVAIL-02 | 07-01-PLAN.md | RSA private key is parsed once at round creation; handlers reuse the parsed RsaBlindSigner | PARTIALLY SATISFIED | RsaBlindSigner is cached in RoundStateInner and used in post_output. But post_input still calls from_der_secret_key per request (handlers.rs:210). AVAIL-02 is not fully satisfied for the input registration hot path. |

No orphaned requirements: AVAIL-01 and AVAIL-02 are the only Phase 7 requirements in REQUIREMENTS.md, both claimed by the plans.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| coordinator/src/api/handlers.rs | 209-213 | `from_der_secret_key` called inside write lock per request | Blocker | Per-request 2048-bit RSA key deserialization continues under write lock — this is the exact DoS vector AVAIL-02 was intended to eliminate. An attacker sending parallel post_input requests still forces RSA deserialization under the write lock on each. |
| coordinator/src/api/handlers.rs | 209 | `rsa_signing_key.clone()` | Warning | Creates unzeroized key material per-request on heap. Weakens the zeroize-on-drop guarantee by creating additional key byte copies that are not tracked by the Drop impl. |

### Human Verification Required

None — all critical behaviors are verifiable programmatically from the codebase.

### Gaps Summary

**AVAIL-02 is only half implemented.** The `rsa_signer` field was correctly added to `RoundStateInner` and is correctly used in `post_output`. However, `post_input` — the higher-traffic handler and the primary target of the DoS concern — still calls `RsaBlindSigner::from_der_secret_key(&signer_der)` inside the write lock on every request (handlers.rs lines 209-213). This was documented as a deviation in 07-02-SUMMARY ("one RSA key deserialization per post_input call — acceptable overhead") but it directly contradicts the plan's must-have truth and leaves the AVAIL-02 attack surface open for the input registration handler.

**Root cause:** The borrow conflict documented in both summaries — `&inner.rsa_signer` (immutable) cannot coexist with `&mut guard` passed to `register_input`. The plan spec for 07-01 suggested `let signer = &guard.inner.as_ref().unwrap().rsa_signer` followed by passing `&signer` to `register_input`. The actual fix in 07-01 removed `signer` from `register_input`'s signature and had the function access `inner.rsa_signer` directly — but when 07-02 restructured the handler, the signer param was added back to `register_input` for the new call site, re-introducing the conflict.

**Resolution path:** `register_input` already has access to `state.inner.as_mut()` and therefore `inner.rsa_signer`. Remove the `signer: &RsaBlindSigner` parameter from `register_input` entirely (matching the 07-01 fix intention), and have the function use `inner.rsa_signer` directly. This eliminates both the borrow conflict and the per-request deserialization.

AVAIL-01 (RPC before write lock) is **fully implemented and verified**.

---

_Verified: 2026-04-10T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
