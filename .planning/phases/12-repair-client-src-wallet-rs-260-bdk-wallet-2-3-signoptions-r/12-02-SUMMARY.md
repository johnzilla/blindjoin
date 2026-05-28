---
phase: 12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r
plan: 02
subsystem: integration-tests
status: halted
tags:
  - integration-tests
  - full-round
  - unmute
  - d-11-escape-valve
  - 5th-orthogonal-blocker
  - http-400-round-sign
  - phase-13-seed
requires:
  - "Phase 12 Plan 01 (0bbcf3c wallet fix) — in history (VERIFIED)"
  - "Phase 11 Plan 01 (cc20f6f RSA SPKI fix) — in history (VERIFIED)"
provides:
  - "5th orthogonal blocker diagnosed: client sends raw DER sig bytes; coordinator expects bitcoin::Witness consensus-serialized bytes — format mismatch at coordinator/src/round/signing.rs:160"
  - "12-02-SUMMARY.md: Phase 13 seed context with failure signature, failure site, proposed minimal repair"
affects:
  - "tests/integration/full_round.rs (NOT modified — #[ignore] attributes all remain intact, working tree clean)"
tech-stack:
  added: []
  patterns:
    - "D-11 escape-valve: halt at first canonical-first failure, emit CHECKPOINT REACHED, surface Phase 13 seed"
key-files:
  created:
    - path: .planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-02-SUMMARY.md
      note: "This file — Phase 13 seed CONTEXT with failure diagnosis"
  modified: []
decisions:
  - "D-11 escape-valve invoked: canonical-first full_round_three_clients failed — Plan 12-02 HALTS, Phase 13 absorbs (D-12)"
  - "Pre-authorized in-flight scope expansion: ZERO — Tasks 2-6 (remaining 5 unmutes) NOT attempted"
  - "REQUIREMENTS.md REPAIR-01 status remains [ ] (Phase 13 closes)"
  - "ROADMAP.md Phase 12 entry NOT marked complete"
  - "Working tree clean: no #[ignore] lines removed, git status shows zero changes"
metrics:
  tasks_completed: 0
  tasks_attempted: 1
  files_modified: 0
  commits: 0
  duration_seconds: ~120
  completed: 2026-05-28
---

# Phase 12 Plan 02: Six-Test Unmute Cycle — HALTED at D-11 Escape-Valve

## One-liner

Plan halted at canonical-first per D-11 — 5th orthogonal blocker (HTTP 400 from /round/sign) diagnosed as a client↔coordinator partial-signature wire format mismatch; Phase 13 absorbs per D-12.

## Prerequisites Verified

Both dependency SHAs confirmed in git history before the canonical-first invocation:

```
WALLET_FIX_SHA = 0bbcf3c76ca251c14aa64216ca6955be1f880b9a  (fix(12): trust_witness_utxo, Plan 12-01)
RSA_FIX_SHA    = cc20f6fbca4d292bf7b394a3850b18d244b5b602  (fix(11): switch client RSA pubkey decode, Phase 11 Plan 01)
```

`grep -c 'TODO(Phase-10)' tests/integration/full_round.rs` → 6 (all six #[ignore] attributes intact before invocation).

## Task 1: Canonical-First Attempt

**Invocation (per plan step c — used --include-ignored per objective step 5):**

```
RUST_LOG=debug BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::full_round_three_clients -- --include-ignored --nocapture \
  2>&1 | tee target/integration-test-12-02-canonical.log
```

**Verdict:** FAILED

```
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 3.65s
```

**bitcoind version (CD-4):** `Bitcoin Core daemon version v31.0.0 bitcoind`

**Failure signature:**

```
thread 'full_round::full_round_three_clients' panicked at tests/integration/full_round.rs:248:22:
verify_and_sign: HTTP status client error (400 Bad Request) for url (http://127.0.0.1:63691/round/sign)
```

This is NOT `Missing non-witness UTXO` — Plan 12-01's wallet repair is confirmed bisect-clean (the bdk_wallet signing call no longer fires the MissingNonWitnessUtxo guard). The 400 is a **NEW (5th) orthogonal blocker** downstream of the sign call.

## D-11 Escape-Valve Invoked

Per CONTEXT.md D-11 and the plan's Task 1 step (d):

> "If the verdict line is NOT a PASS, HALT IMMEDIATELY per D-11 escape-valve. Do NOT delete the #[ignore] line. Do NOT commit. Do NOT proceed to Task 2."

- No #[ignore] line deleted.
- No commit made for the canonical-first attempt.
- Tasks 2-6 (remaining 5 unmutes) NOT attempted.
- Working tree clean: `git status --short` returned empty output.

## Failure Diagnosis

### HTTP Status: 400 Bad Request

The coordinator's `/round/sign` handler (coordinator/src/api/handlers.rs:558-568) maps error codes to HTTP status:
- `SessionInvalid` → 401 UNAUTHORIZED
- `WrongPhase` → 409 CONFLICT
- `RpcUnavailable` → 503 SERVICE_UNAVAILABLE
- All others (including `BroadcastRejected`) → **400 BAD_REQUEST**

A 400 from `/round/sign` is definitively `BroadcastRejected` — meaning the rejection occurs inside `assemble_and_broadcast`, not in the session-token or UTXO-registration checks.

### Root Cause: Partial Signature Wire Format Mismatch

**Client sends** (client/src/wallet.rs:279-281):
```rust
if let Some((_pk, sig)) = input.partial_sigs.iter().next() {
    return Ok(sig.to_vec());  // raw DER-encoded ECDSA signature bytes (71-72 bytes)
}
```
The client extracts the raw `bitcoin::ecdsa::Signature` bytes from the bdk_wallet-populated `psbt.inputs[idx].partial_sigs` map. This is raw DER + SIGHASH_ALL byte: e.g. `304402...0141`.

**Coordinator expects** (coordinator/src/round/signing.rs:160-170):
```rust
match bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes) {
    Ok(witness) => {
        psbt.inputs[i].final_script_witness = Some(witness);
    }
    Err(_) => {
        return Err(ApiError {
            code: ErrorCode::BroadcastRejected,
            message: format!("Invalid witness data for input {}", i),
            ...
        });
    }
}
```
The coordinator calls `consensus::deserialize::<bitcoin::Witness>()` on the bytes it received. A `bitcoin::Witness` in consensus encoding is a vector-of-vectors prefixed by a CompactInt count. Raw DER signature bytes fail this deserialization: the CompactInt length prefix (expected to be the number of witness stack items) will not match a DER signature's `0x30` tag byte, so deserialization returns `Err`.

**The divergence:** The client returns raw DER bytes; the coordinator deserializes as a serialized `bitcoin::Witness`. These formats are incompatible.

### Why This Broke

This mismatch was latent before Phase 12. It was never exercised because:
1. Before Phase 11, the test hit the RSA SPKI decode error at input registration.
2. After Phase 11 (cc20f6f), the test got past input registration but hit `Missing non-witness UTXO` at signing.
3. After Phase 12-01 (0bbcf3c), signing now completes — and the partial signature is submitted to the coordinator for the first time, revealing the format mismatch.

The mismatch pre-exists all Phase 11+12 work; it was simply never reached. This is a latent wire-format bug that surfaces only when the entire CoinJoin round executes end-to-end.

## Proposed Minimal Repair (Phase 13 Seed)

### Option A: Client encodes as bitcoin::Witness (RECOMMENDED)

In `client/src/wallet.rs::sign_psbt_input`, instead of returning raw DER bytes, encode the signature + pubkey as a `bitcoin::Witness` (the P2WPKH witness stack: [sig_bytes, pubkey_bytes]), then consensus-serialize it:

**Fix locus:** `client/src/wallet.rs` lines 276-291 (the partial signature extraction block).

```rust
// Current (BROKEN):
if let Some((_pk, sig)) = input.partial_sigs.iter().next() {
    return Ok(sig.to_vec());  // raw DER bytes — coordinator can't deserialize as Witness
}

// Proposed fix (Option A):
if let Some((pk, sig)) = input.partial_sigs.iter().next() {
    let mut witness = bitcoin::Witness::new();
    witness.push(sig.to_vec());        // sig bytes (DER + SIGHASH_ALL)
    witness.push(pk.to_bytes());       // compressed pubkey (33 bytes)
    return Ok(bitcoin::consensus::serialize(&witness));
}
```

The coordinator already handles a 2-item P2WPKH witness stack correctly: it inserts the deserialized `Witness` into `psbt.inputs[i].final_script_witness` and calls `psbt.extract_tx()`.

**Why this is minimal:** One extraction block changes in `client/src/wallet.rs`. No coordinator changes. No shared protocol changes. No new dependencies.

**Why the coordinator side is correct:** `signing.rs:160` calls `consensus::deserialize::<bitcoin::Witness>()` and sets `psbt.inputs[i].final_script_witness`. This is the correct coordinator-side approach for finalizing a P2WPKH PSBT. The coordinator then calls `psbt.extract_tx()` (line 182), which requires `final_script_witness` to be set (not `partial_sigs`). The client, not the coordinator, needs fixing.

### Option B: Coordinator deserializes as raw DER + reconstructs Witness

In `coordinator/src/round/signing.rs::assemble_and_broadcast`, parse the incoming bytes as a raw DER signature, construct a `bitcoin::Witness` server-side by fetching the pubkey from the registered input. This requires storing the pubkey at input registration time, which is currently not done. Option B is heavier and requires coordinator-side changes.

**Option A is the recommended repair** — single file, client-only, no coordinator protocol change.

### Verification After Option A Fix

After applying Option A to `client/src/wallet.rs`, the canonical-first invocation should:
1. Pass the bdk_wallet sign call (trust_witness_utxo: true — already fixed in Phase 12-01).
2. Submit a properly-encoded `bitcoin::Witness` to `/round/sign`.
3. Coordinator deserializes successfully, sets `final_script_witness`, broadcasts.
4. `full_round_three_clients` passes.

## What Was NOT Done

Per D-11 escape-valve (pre-authorized in-flight scope expansion = ZERO):

- **No unmute commits made.** Zero #[ignore] attributes removed. The six carve-out sites at full_round.rs lines 164, 462, 730, 854, 911, 1236 remain intact.
- **No D-13 bookkeeping commit.** REQUIREMENTS.md REPAIR-01 status remains `[ ]`. ROADMAP.md Phase 12 entry is NOT marked complete.
- **Tasks 2-6 not attempted.** Only the canonical-first (Task 1) invocation was run.

## Working Tree State

Working tree is clean. The canonical-first invocation used `--include-ignored` (which runs #[ignore] tests without editing the source file). No source file was modified during Plan 12-02 execution.

```
git status --short  →  (empty — no changes)
grep -c 'TODO(Phase-10)' tests/integration/full_round.rs  →  6
```

## Recovery Path — Phase 13

1. Open Phase 13 via `/gsd-phase` targeting the partial-signature wire format repair.
2. Phase 13 CONTEXT.md seed:
   - **Blocker:** `verify_and_sign: HTTP 400 Bad Request from /round/sign` — 5th orthogonal blocker discovered during Plan 12-02 canonical-first gate.
   - **Root cause:** `client/src/wallet.rs::sign_psbt_input` returns raw DER bytes; coordinator's `signing.rs:160` calls `consensus::deserialize::<bitcoin::Witness>()` — format mismatch.
   - **Fix locus:** `client/src/wallet.rs` lines 276-291 (the partial_sigs extraction block, after the `self.inner.sign()` call).
   - **Proposed repair:** Option A — encode signature + pubkey as `bitcoin::Witness`, return `consensus::serialize(&witness)`.
   - **Dependencies already in history:** 0bbcf3c (wallet fix), cc20f6f (RSA fix) — both remain bisect-clean.
   - **After Phase 13's fix lands:** Re-run Plan 12-02's canonical-first gate. If it passes, proceed with the 6-unmute cycle per Plan 12-02's spec (verbatim from 11-02-PLAN.md with two-SHA commit bodies). If it fails with another new signature, a 6th orthogonal blocker has appeared.
3. Phase 13 plan structure (suggested):
   - Plan 13-01: Apply Option A wire-format fix to `client/src/wallet.rs` + local sanity with `full_round_three_clients`.
   - Plan 13-02: Re-execute Plan 12-02's six-unmute cycle (depends_on: 13-01) — identical spec to Plan 12-02 with commit subjects shifted to `test(13):`.

## Deviations from Plan

**1. [Rule None — D-11 Halt] Canonical-first gate failed with 5th orthogonal blocker**

- **Found during:** Task 1 step (c) — canonical-first invocation
- **Issue:** `full_round_three_clients` returned HTTP 400 Bad Request from `/round/sign`, not PASS
- **Action taken:** D-11 escape-valve invoked — no edit, no commit, no later tasks attempted
- **Root cause diagnosed:** Client sends raw DER signature bytes; coordinator expects `bitcoin::Witness` consensus encoding
- **Phase 13 absorbs:** per D-12

## Known Stubs

None — this plan made no source changes. The only artifact is this SUMMARY.md.

## Threat Flags

No new threat surface introduced — no source files modified.

## Self-Check

- Files exist:
  - `target/integration-test-12-02-canonical.log` — FOUND (tee output from canonical invocation)
  - `.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-02-SUMMARY.md` — FOUND (this file)
- Commits: zero unmute commits made (correct per D-11)
- Source assertions:
  - `grep -c 'TODO(Phase-10)' tests/integration/full_round.rs` = 6 — all six #[ignore] attributes intact
  - `git status --short` = empty — working tree clean
  - REQUIREMENTS.md REPAIR-01 = `[ ]` — unchanged
- Wallet fix SHA in history: `0bbcf3c76ca251c14aa64216ca6955be1f880b9a` — VERIFIED
- RSA fix SHA in history: `cc20f6fbca4d292bf7b394a3850b18d244b5b602` — VERIFIED

## Self-Check: PASSED
