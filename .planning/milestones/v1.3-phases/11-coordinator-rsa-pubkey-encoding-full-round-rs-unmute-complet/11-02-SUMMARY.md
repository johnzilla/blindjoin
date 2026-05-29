---
phase: 11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet
plan: 02
subsystem: integration-tests
tags:
  - integration-tests
  - full-round
  - unmute
  - bitcoind-pinned
  - phase-10-carve-out
  - per-test-commit-cycle
  - halted
  - d-08-escape-valve
  - 4th-orthogonal-blocker
  - bdk-wallet-non-witness-utxo
requires:
  - "Plan 11-01 (cc20f6f fix + 13da4b5 test) — landed"
  - "Phase 10 Fix A (d99b3a4) + Fix WIF-D (e02ce55) — landed"
  - "brew bitcoind v31.0.0 (BITCOIND_EXE=$(brew --prefix)/bin/bitcoind)"
provides: []
affects:
  - "tests/integration/full_round.rs — no changes (HALTED before any edit)"
tech-stack:
  added: []
  patterns: []
key-files:
  created:
    - .planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md
  modified: []
decisions:
  - "D-08 escape-valve INVOKED: canonical-first test (full_round_three_clients) failed; HALTED before any unmute commit"
  - "D-09 deferral: 4th orthogonal blocker (bdk_wallet 2.3 SignOptions::default() now demands non_witness_utxo) absorbed by Phase 12 — not in-scope for Phase 11"
  - "Pre-authorized in-flight scope expansion HONORED: ZERO"
metrics:
  tasks_completed: 0
  files_modified: 0
  lines_added: 0
  lines_removed: 0
  commits: 1  # this summary commit; ZERO unmute commits
  duration_seconds: ~160
  completed: 2026-05-28
  status: halted
---

# Phase 11 Plan 02: full_round.rs Unmute Cycle Summary — HALTED PER D-08

## One-line verdict

**Plan halted before the 1st unmute commit per D-08 escape-valve: `full_round_three_clients` (the canonical-first gate at line 164) FAILED against pinned brew bitcoind v31 with a 4th orthogonal blocker — bdk_wallet 2.3 `SignOptions::default()` now demands `non_witness_utxo` even for segwit signing.** Zero unmute commits made. Zero source edits. Pre-authorized in-flight scope expansion ZERO honored.

## What Was Attempted

### Task 1 step (a): Capture RSA fix SHA — PASSED

`git log --grep="^fix(11): switch client RSA pubkey decode" --format=%H -1` → `cc20f6fbca4d292bf7b394a3850b18d244b5b602` (Plan 11-01 RSA SPKI fix landed).

### Task 1 step (b): Per-test PASS-proof invocation — FAILED

```
BLINDJOIN_REQUIRE_BITCOIND=1 \
  BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::full_round_three_clients -- --ignored 2>&1 \
  | tee target/integration-test-11-02-1.log
```

**Verdict line:**

```
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 3.84s
```

**Panic trace (verbatim from `target/integration-test-11-02-1.log`):**

```
thread 'full_round::full_round_three_clients' (4096104) panicked at coordinator/../tests/integration/full_round.rs:248:22:
verify_and_sign: bdk_wallet signing failed: Missing non-witness UTXO
```

Three nested panics in the same task (one per concurrent client): the same error string in each. Line 248 of `full_round.rs` is the `.expect("verify_and_sign")` on the `round::sign::verify_and_sign(...)` call inside the per-client async task body.

### Task 1 step (c): D-08 escape-valve INVOKED

Per D-08 ("if a 4th orthogonal blocker appears … the executor halts after the first encounter and emits a checkpoint with the failure mode and a proposed minimal repair. Pre-authorized in-flight scope expansion is **zero**"):

- The `#[ignore]` line at line 164 was **NOT** deleted.
- No unmute commit was made for `full_round_three_clients`.
- Tasks 2-6 (the remaining 5 carve-outs at lines 462, 730, 854, 911, 1236) were **NOT** attempted.
- All six `#[ignore = "TODO(Phase-10): ..."]` attribute lines remain in `tests/integration/full_round.rs`.

## Failure Diagnosis (for Phase 12 planning)

### Failure signature (NEW — not one of the three already-cleared blockers)

| Blocker | Status | Resolution commit |
|--------|--------|-------------------|
| vout-after-mine (RPC -5 on `get_raw_transaction_verbose` after confirmation block) | Cleared in Phase 10 | d99b3a4 (Fix A in `tests/integration/mod.rs::fund_regtest`) |
| bdk 2.3 wallet API rename (`from_wif` → `Wallet::create_single`) | Cleared in Phase 10 | e02ce55 (Fix WIF-D in client wallet) |
| RSA SPKI decode asymmetry (`from_der` rejected PSS-flavored SPKI) | Cleared in Phase 11 Plan 01 | cc20f6f (fix) + 13da4b5 (regression test) |
| **bdk_wallet 2.3 SignOptions demands `non_witness_utxo`** | **NEW (4th orthogonal blocker)** | **Phase 12 absorbs (D-09)** |

The new failure is a distinct signature: it surfaces in the SIGNING phase, not INPUT_REG (where Phase 11-01's RSA fix lives), and is wallet-side, not crypto-side.

### Root-cause hypothesis (high-confidence, for Phase 12 to validate)

**Locus:** `client/src/wallet.rs:260` — `self.inner.sign(psbt, SignOptions::default())`.

In bdk_wallet 2.3.0, `SignOptions::default()` sets `trust_witness_utxo: false` (the safer default — the BIP-143 sighash uses the input value, and an attacker who has only `witness_utxo` set could spoof a higher value than the actual previous output, tricking the signer into authorizing more fee than expected). With `trust_witness_utxo: false`, bdk demands `non_witness_utxo` (the full previous transaction bytes) to validate that the `witness_utxo.value` matches the actual chain value.

**Current code** (`client/src/wallet.rs:243-261`):

```rust
pub fn sign_psbt_input(&self, psbt: &mut Psbt) -> Result<Vec<u8>> {
    // ...
    psbt.inputs[input_idx].witness_utxo = Some(TxOut {
        value: Amount::from_sat(self.utxo_value_sats),
        script_pubkey: self.utxo_script_pubkey.clone(),
    });
    self.inner.sign(psbt, SignOptions::default())
        .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;
    // ...
}
```

Only `witness_utxo` is populated; `non_witness_utxo` is left None. bdk_wallet 2.3 rejects this combination with `Missing non-witness UTXO`.

### Two minimal-repair candidates for Phase 12

**Option A (preferred — single-line change, no extra RPC):**

Change `SignOptions::default()` to `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }`. This is safe in our context because the client itself populated `witness_utxo` from its own `ClientWallet::utxo_value_sats` (set at wallet construction from the same regtest RPC that we trust as the ground-truth source); there is no attacker-controlled value to spoof. This mirrors how many BDK examples handle CLI-controlled PSBTs.

**Option B (heavier — populate `non_witness_utxo` from RPC):**

In `sign_psbt_input`, also set `psbt.inputs[input_idx].non_witness_utxo = Some(<prev-tx>)` by fetching the full previous transaction bytes via `get_raw_transaction(<txid>)`. Requires plumbing a bitcoind RPC handle into the wallet (currently the wallet does not have one). More work; defensible if Phase 12 prefers byte-for-byte chain validation in the client.

Phase 12 picks; Phase 11 must NOT touch `client/src/wallet.rs` (out of file scope per D-07/D-09).

## Working-Tree State (verification — all clean)

| Check | Expected | Actual |
|-------|----------|--------|
| `git status --short` | empty (no staged or unstaged changes) | empty (verified) |
| `sed -n '164p' tests/integration/full_round.rs` | `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]` | identical (verified) |
| `grep -c "TODO(Phase-10)" tests/integration/full_round.rs` | `6` (all carve-outs preserved) | `6` (verified) |
| Plan-11-02 commits made | `0` | `0` (only the post-summary commit follows this file write) |
| Plan 11-01 commits in history | `cc20f6f`, `13da4b5` | both present (verified) |

## CHECKPOINT REACHED (D-08 marker)

**Type:** halted-and-surfaced (4th orthogonal blocker)
**Plan:** 11-02
**Progress:** 0/6 unmute commits made — HALTED at Task 1 step (b)

### Failing test

- **Test fn:** `full_round_three_clients` (the canonical-first happy-path gate per D-05)
- **Test file:** `tests/integration/full_round.rs:165` (with `#[ignore]` still attached at line 164)
- **Panic message:** `verify_and_sign: bdk_wallet signing failed: Missing non-witness UTXO`
- **Panic site:** `tests/integration/full_round.rs:248:22` (the `.expect("verify_and_sign")` call in the per-client task)
- **Cargo verdict:** `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out`
- **Iteration:** 1/6
- **Reproducer:** `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round::full_round_three_clients -- --ignored`

### Proposed minimal repair (Phase 12 input — NOT applied here)

- **Locus:** `client/src/wallet.rs:260` — change `SignOptions::default()` to `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }` (Option A above). Rationale: bdk_wallet 2.3 tightened the default to require `non_witness_utxo` for segwit signing as a BIP-143 fee-attack mitigation; the client populates `witness_utxo` from its own trusted regtest RPC, so `trust_witness_utxo: true` is safe in this context.
- **Out-of-scope here:** `client/src/wallet.rs` is NOT in Plan 11-02's file scope (which is `tests/integration/full_round.rs` only). D-09 explicitly defers any non-unmute fix to Phase 12.

### Resume protocol

1. Phase 12 plans the minimal client-wallet repair (Option A or B above).
2. Phase 12 Plan 02 (or whatever number it lands) re-runs the per-test PASS-proof for `full_round_three_clients`.
3. If it passes, Phase 12's Plan 02 (or a follow-up plan) executes the 6 unmute commits in canonical-first then file order, exactly per the locked-in Plan 11-02 task spec — the spec remains valid; only its first per-test step now requires the wallet repair to be in history.
4. The exact six commit subjects, ordering, and CD-1 body shape from Plan 11-02 should be reused without modification when Phase 12 unmutes the carve-outs.

## REPAIR-01 / REPAIR-02 Status

- **REPAIR-01:** NOT CLOSED. Phase 11 Plan 02 was the closure trigger (D-10) — green local suite + 6 unmute commits. Neither was delivered. REQUIREMENTS.md's REPAIR-01 status remains `[ ]` until Phase 12 lands the wallet repair AND the 6 unmutes pass.
- **REPAIR-02:** NOT TOUCHED by this plan. Closure remains tied to PR observation per D-11 (the `corepc-node-feature-pin-check` CI job, already landed at 4026f50). The Phase 11 PR observation moment is deferred until Phase 12 produces the green suite — at that point a single PR carries both Phase 11 (RSA fix) and Phase 12 (wallet repair + unmutes), and REPAIR-02 closes on its CI green observation.

## v1.3 Ship Notes

Not in scope per D-12. Deferred to wrap-up phase or `/gsd-ship` after Phase 12 closes REPAIR-01.

## Deviations from Plan

**None in the auto-fix sense.** The plan's halt-and-surface protocol (D-08) executed exactly as written. The "deviation" is the discovery of a 4th orthogonal blocker, which the plan explicitly anticipates with D-09's "Phase 12 absorbs any newly-discovered blocker or non-unmute scope." Rule 1-3 auto-fixes were NOT applied because:

- The fix locus (`client/src/wallet.rs`) is OUTSIDE this plan's file scope.
- D-08 explicitly forbids pre-authorized in-flight scope expansion ("ZERO").
- The PRD's <files_modified> contract restricts Plan 11-02 to `tests/integration/full_round.rs` only.

Any attempt to auto-fix the wallet from inside this plan would have been a scope violation per D-07.

## Threat Surface Scan

No new threat surface introduced. Zero source edits in this plan. The discovered bdk_wallet behavior is a mitigation tightening (not a regression) — it now prevents fee-spoofing attacks that BIP-143 historically allowed. The repair-direction for Phase 12 (`trust_witness_utxo: true`) is contextually safe in our regtest-RPC-trusted setup but should be documented inline when applied.

## Self-Check: PASSED

- Files exist:
  - `tests/integration/full_round.rs` — UNCHANGED from pre-plan (all 6 #[ignore] sites present at original line numbers 164/462/730/854/911/1236) — VERIFIED.
  - `.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md` — FOUND (this file).
  - `target/integration-test-11-02-1.log` — FOUND (per-test failure log preserved in worktree for diagnostic; not checked in per .gitignore).
- Commits exist:
  - `cc20f6f` (Plan 11-01 RSA fix) — FOUND in `git log`.
  - `13da4b5` (Plan 11-01 SPKI roundtrip test) — FOUND in `git log`.
  - Zero Plan 11-02 unmute commits — VERIFIED (`git status --short` is empty; `git log --grep='^test(11): unmute' --oneline | wc -l` returns 0).
- Halt-state cleanliness:
  - No source modifications anywhere in the worktree — VERIFIED (`git status --short` empty).
  - All 6 `#[ignore]` lines present at original line numbers — VERIFIED (`grep -c "TODO(Phase-10)" tests/integration/full_round.rs` returns 6).
  - Bisect cleanliness: trivial — zero new commits in this plan other than the summary commit that follows this file.
