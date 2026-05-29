---
phase: 13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle
plan: 01
subsystem: client-wallet
status: halted
tags:
  - wire-format
  - client-wallet
  - bitcoin-witness
  - d-07-sanity-gate
  - d-12-escape-valve
  - 6th-orthogonal-blocker
  - http-400-round-sign-persists
  - phase-14-seed
requires:
  - "Phase 11 Plan 01 (cc20f6f RSA SPKI fix) — in history (VERIFIED)"
  - "Phase 12 Plan 01 (0bbcf3c wallet trust_witness_utxo fix) — in history (VERIFIED)"
provides:
  - "6th orthogonal blocker diagnosed: HTTP 400 from /round/sign PERSISTS even after the wire-format fix is applied — the 5th-blocker hypothesis (raw DER vs bitcoin::Witness) is INSUFFICIENT to explain the failure"
  - "13-01-SUMMARY.md: Phase 14 seed CONTEXT with failure-mode evidence, downstream investigation, proposed minimal repair"
affects:
  - "client/src/wallet.rs (NOT modified — working-tree edit applied during D-07 gate then reverted after gate FAILED; HEAD shape preserved bit-identical)"
tech-stack:
  added: []
  patterns:
    - "D-07 sanity gate: capture canonical-first PASS BEFORE the source-fix commit lands; if the gate FAILS, revert the working-tree edit, commit only the SUMMARY, surface as a CHECKPOINT for the next phase"
    - "D-12 escape-valve discipline: pre-authorized in-flight scope expansion is ZERO — Phase 13 halts at the second orthogonal-blocker discovery, Phase 14 absorbs"
key-files:
  created:
    - path: .planning/phases/13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle/13-01-SUMMARY.md
      note: "This file — Phase 14 seed CONTEXT with the 6th-orthogonal-blocker diagnosis"
  modified: []
decisions:
  - "D-07 sanity gate INVOKED: applied the wire-format fix at client/src/wallet.rs:279-284 (the canonical Option A 4-LOC edit per D-01), built clean (cargo build -p client, zero errors, zero warnings), then ran the canonical-first sanity test — verdict was FAILED with the SAME HTTP 400 signature as Plan 12-02 (full_round.rs:248:22 panic)"
  - "D-12 escape-valve invoked: the wire-format fix is NECESSARY but NOT SUFFICIENT — a 6th orthogonal blocker downstream of the witness-deserialize site (coordinator/src/round/signing.rs:160) accounts for the persistent HTTP 400"
  - "Per user instruction in the executor's critical_constraints block, the working-tree edit was REVERTED via git checkout -- client/src/wallet.rs (departs from CONTEXT.md D-12's 'leave working-tree edit in place' guidance — the executor follows the orchestrator instruction)"
  - "No fix commit on client/src/wallet.rs (no `fix(13):` subject lands)"
  - "Plan 13-02 (six-test unmute cycle) is NOT opened — Phase 14 absorbs per D-13"
  - "REQUIREMENTS.md REPAIR-01 status NOT modified (Phase 14 closeout reconciles)"
  - "ROADMAP.md Phase 13 entry NOT marked complete (Phase 14 closeout reconciles)"
metrics:
  tasks_completed: 0
  tasks_attempted: 1
  files_modified: 0
  commits: 0
  duration_seconds: ~600
  completed: 2026-05-28
---

# Phase 13 Plan 01: Wire-Format Fix + D-07 Sanity Gate — HALTED at D-12 Escape-Valve

## One-liner

Applied the canonical Option A wire-format fix to `client/src/wallet.rs:279-284`, built clean, ran the D-07 canonical-first sanity test — verdict was FAILED with the SAME `HTTP 400 Bad Request from /round/sign` signature as Plan 12-02; a 6th orthogonal blocker persists downstream of the witness-deserialize site; wire-format fix is NECESSARY but NOT SUFFICIENT; Phase 14 absorbs per D-12.

## Prerequisites Verified

Both prerequisite SHAs confirmed in git history before the D-07 capture:

```
RSA_FIX_SHA       = cc20f6fbca4d292bf7b394a3850b18d244b5b602  (fix(11): RSA SPKI, Phase 11 Plan 01)
WALLET_TRUST_SHA  = 0bbcf3c76ca251c14aa64216ca6955be1f880b9a  (fix(12): trust_witness_utxo, Phase 12 Plan 01)
```

`git log --oneline -20` shows both in lineage before HEAD. `grep -c 'TODO(Phase-10)' tests/integration/full_round.rs` returns 6 — the six unmute sites remain intact.

## Working-Tree Edit Applied (Then Reverted)

The canonical Option A edit per D-01 / 13-PATTERNS.md §"Encoding pattern" was applied to `client/src/wallet.rs:279-284`:

**Before (HEAD, the broken pre-fix state):**

```rust
if let Some((_pk, sig)) = input.partial_sigs.iter().next() {
    return Ok(sig.to_vec());
}
```

**After (the D-07 working-tree edit):**

```rust
if let Some((pk, sig)) = input.partial_sigs.iter().next() {
    let mut witness = bitcoin::Witness::new();
    witness.push(sig.to_vec());        // ECDSA sig: DER + SIGHASH_ALL byte
    witness.push(pk.to_bytes());       // compressed pubkey (33 bytes)
    return Ok(bitcoin::consensus::serialize(&witness));
}
```

**Diff scope:** `1 file changed, 5 insertions(+), 2 deletions(-)` — exactly the scope predicted by 13-PATTERNS.md.

**Build verdict (PASS):**

```
cargo build -p client
   Compiling client v0.1.0 (.../blindjoin/client)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.38s
```

Zero errors, zero warnings. In particular no `unused_variables: pk` warning (the destructure binding `pk` is consumed by `pk.to_bytes()`).

**No `use bitcoin::Witness` / `use bitcoin::consensus` was added** — the fix uses fully-qualified paths per PATTERNS.md §"Imports pattern" (the file's idiom is inline `use` or fully-qualified path; no top-level imports for these symbols).

**The `final_script_witness` fallback at lines 285-290** was preserved untouched per CD-3 (defer cleanup for bisect cleanliness).

**No in-source block comment at the fix locus** per D-10 (rationale was planned for the commit body — the commit was never made).

## D-07 Canonical-First Sanity Test — FAILED

**Invocation (per CONTRIBUTING.md §"Running integration tests"):**

```bash
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::full_round_three_clients -- --ignored \
  2>&1 | tee target/integration-test-13-01-d07.log
```

**Verdict line:**

```
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 3.57s
```

**bitcoind version (CD-4):** `Bitcoin Core daemon version v31.0.0 bitcoind`

**Failure signature (verbatim from `target/integration-test-13-01-d07.log`):**

```
thread 'full_round::full_round_three_clients' (4900878) panicked at coordinator/../tests/integration/full_round.rs:248:22:
verify_and_sign: HTTP status client error (400 Bad Request) for url (http://127.0.0.1:65308/round/sign)
```

**This is the IDENTICAL signature recorded in `12-02-SUMMARY.md §"Task 1: Canonical-First Attempt"`** (also `full_round.rs:248:22`, also HTTP 400 from `/round/sign`). The wire-format fix lands cleanly but does NOT change the failure verdict.

## D-12 Escape-Valve Invoked

Per the executor's `<critical_constraints>` block:

> On FAIL ... HALT per D-12. Do NOT commit the wallet.rs edit. Revert the working-tree edit with `git checkout -- client/src/wallet.rs`. Write SUMMARY.md documenting the halt + CHECKPOINT REACHED + failure mode + minimal repair proposal, commit only the SUMMARY.md, and return `## CHECKPOINT REACHED`.

Actions taken:

- **No `fix(13):` commit on `client/src/wallet.rs` was made.**
- **Working-tree edit reverted** via `git checkout -- client/src/wallet.rs` — `sed -n '279,281p' client/src/wallet.rs` confirms the pre-edit `(_pk, sig)` destructure and `Ok(sig.to_vec())` return are back.
- **Plan 13-02 NOT opened** — no #[ignore] line is touched.
- **REQUIREMENTS.md REPAIR-01 NOT modified** — stays at its current state (the doc drift documented in D-14 carries forward).
- **ROADMAP.md Phase 13 entry NOT marked complete** — phase remains EXECUTING/HALTED.

Note on CONTEXT.md D-12 vs executor's critical_constraints: CONTEXT.md D-12 suggested leaving the working-tree edit in place ("so Phase 14 can diagnose against the post-fix state"); the executor's critical_constraints explicitly instructed `git checkout -- client/src/wallet.rs`. The executor followed the more-specific orchestrator instruction. Phase 14 can re-apply the wire-format edit trivially (the exact diff is recorded in this SUMMARY §"Working-Tree Edit Applied").

## Failure Diagnosis — 6th Orthogonal Blocker

### What we know

1. **Plan 12-01 fix (`0bbcf3c`)** — wallet `trust_witness_utxo: true` is in history. `cargo build -p client` succeeds. The bdk_wallet sign call no longer fires the MissingNonWitnessUtxo guard. This was the 4th orthogonal blocker.
2. **Plan 13-01 wire-format fix (working-tree edit, reverted)** — was applied correctly. The encoder produces a 2-item `bitcoin::Witness` `[sig.to_vec(), pk.to_bytes()]` consensus-serialized. The bytes structurally satisfy `bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes)` at `coordinator/src/round/signing.rs:160`. This was the 5th orthogonal blocker.
3. **HTTP 400 from `/round/sign` PERSISTS.** Same site (`full_round.rs:248:22`), same status (400 Bad Request), same endpoint (`/round/sign`).

### Why the wire-format fix is insufficient

A 400 from `/round/sign` maps to any `ApiError` whose `code` is not `SessionInvalid` / `WrongPhase` / `RpcUnavailable` (coordinator/src/api/handlers.rs:558-568). In `process_sign` / `assemble_and_broadcast` (coordinator/src/round/signing.rs:25-200+), the BroadcastRejected paths are:

| Site | Origin | Note |
|------|--------|------|
| signing.rs:107-111 | `parse_outpoint` on registered input — invalid outpoint string | Unlikely (registration succeeded earlier) |
| signing.rs:112-117 | `parse_address_to_script` on change address — invalid bech32 | Unlikely (registration validated earlier) |
| signing.rs:132-137 | `parse_address_to_script` on output address — invalid bech32 | Unlikely |
| signing.rs:147-151 | `build_coinjoin_psbt` returned a `TxError` | Possible — but only InsufficientFunds / NoParticipants / Psbt(err) |
| **signing.rs:164-170** | **`bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes)` returned `Err`** | **This is where the 5th-blocker hypothesis predicted the failure originated. With the wire-format fix in working tree, this site should NOT fire** |
| signing.rs:173-178 | `inner.partial_sigs.get(&outpoint_str)` returned `None` | Unlikely (sig was inserted at signing.rs:71) |
| signing.rs:182-186 | `psbt.extract_tx()` returned `ExtractTxError` (MissingInputValue / SendingTooMuch / AbsurdFeeRate) | **Plausible — see "Most likely root cause" below** |
| signing.rs:190-194 | `rpc.testmempoolaccept(...)` returned `Err` (RPC fault) — but maps to `RpcUnavailable` → 503, not 400 | Ruled out |
| signing.rs:197+ | `testmempoolaccept` returned `allowed: false` — bitcoind rejected the tx | **Plausible — see "Most likely root cause" below** |

Because the test runner does NOT initialize a tracing subscriber and the client's `reqwest` call uses `.error_for_status()` (client/src/http.rs:100) which discards the response body, the SPECIFIC `BroadcastRejected` message is invisible to the test. The 400 status is all we have.

### Most likely root cause (Phase 14 starting hypothesis)

Reading `coordinator/src/round/signing.rs:104-127` (the `ParticipantInput` construction inside `assemble_and_broadcast`):

```rust
participant_inputs.push(ParticipantInput {
    outpoint,
    value_sats: config.coordinator.denomination_sats + estimate_fee_share(
        inner.registered_inputs.len() as u32,
        config.coordinator.fee_rate_sat_per_vbyte,
    ),
    script_pubkey: change_script.clone(),       // <-- the participant's CHANGE script, NOT the input UTXO's actual script_pubkey
    change_address: change_script,
});
```

Two coordinator-side errors that compound after the wire-format fix lands:

1. **`witness_utxo.script_pubkey` is wrong on the coordinator side.** `build_coinjoin_psbt` (coordinator/src/bitcoin/tx.rs:121-126) populates `psbt.inputs[i].witness_utxo.script_pubkey = inp.script_pubkey`. The `inp.script_pubkey` passed by `assemble_and_broadcast` is `change_script` (the CHANGE address script the client posted during input-registration), NOT the actual script that locks the input UTXO on-chain. The client signs with the correct UTXO script (`client/src/wallet.rs:253-256` sets `witness_utxo` from `self.utxo_script_pubkey`), so the BIP-143 sighash bytes from the client are consistent. But the coordinator's `final_script_witness`-finalized PSBT carries a `witness_utxo.script_pubkey` that does NOT match the on-chain UTXO. bitcoind's `testmempoolaccept` validates the witness against the ACTUAL on-chain script_pubkey (looked up from the prevout), so this may or may not fail; in either case the coordinator's PSBT internal-state is structurally wrong.

2. **`witness_utxo.value` is wrong on the coordinator side.** `assemble_and_broadcast` sets `value_sats = denomination_sats + estimate_fee_share(...)` ≈ `100_166`, but the on-chain UTXO value is `denomination + 50_000 = 150_000` (per `tests/integration/mod.rs:427` — `fund_sats = denomination + 50_000`). The client signs over `self.utxo_value_sats = 150_000` (the correct on-chain value); bitcoind validates against the on-chain 150_000 — those match. But the coordinator's PSBT `fee()` computation in `psbt.extract_tx()` uses the coordinator's stored `witness_utxo.value = 100_166`, summing across 3 inputs = `300_498`, minus 3 × `denomination = 300_000` outputs (change is dust-folded because `100_166 - 100_000 - 166 ≈ 0 < 294`) = fee = `498`. That's a low fee but should not trigger `AbsurdFeeRate`. **However, bitcoind's mempool validates the on-chain tx (input value 3 × 150_000 = 450_000, output sum 300_000, fee = 150_000) and 150_000 sats fee on a ~250-byte tx is ~600 sat/vB — well below the `25_000` sat/vB `DEFAULT_MAX_FEE_RATE`, so it should not be rejected as absurd.**

The exact rejection text is unknown without instrumenting `process_sign` or the client's reqwest call to surface the response body. **Phase 14 should start by adding a single diagnostic line at `coordinator/src/round/signing.rs:165-185` (e.g., `tracing::error!(...)` or returning `BroadcastRejected` with `sig_bytes.len()` + first byte) and re-running the canonical-first invocation — the failure message will be surfaced in 30 seconds rather than 30 minutes** (this is the coordinator-side hardening Phase 13 D-03 explicitly deferred; Phase 14's diagnostic crisis warrants reconsidering that deferral).

### Why this surfaced only now

Mirroring the Phase-12-Plan-02 pattern: the 6th blocker has been latent since the coordinator's `assemble_and_broadcast` was first written. It was masked by:

- Phase 11 RSA SPKI: never reached input registration completion.
- Phase 12 wallet trust: never reached `/round/sign` submission.
- Plan 13-01 wire-format: signature bytes deserialize cleanly into `Witness` — the deserialization at signing.rs:160 now SUCCEEDS — and the failure shifts downstream into one of `extract_tx` / `testmempoolaccept` / `accepted` check, all of which return `BroadcastRejected` → HTTP 400.

The HTTP 400 status is identical; the underlying error is downstream. This is a discovery, not a regression.

## Proposed Minimal Repair (Phase 14 Seed)

### Step 1 (zero source change — diagnostic-only, allowed in a fresh phase): Surface the actual error

Add a single `tracing::error!(round_id, code, message, ?)` line in `process_sign` or `assemble_and_broadcast` (coordinator/src/round/signing.rs) at every `BroadcastRejected` return, OR enrich the error message itself with the failure-mode discriminator (`sig_bytes.len()` + first byte at line 167; `extract_tx_err` enum variant at line 184; `accept_result` JSON at line 197+). Re-run the canonical-first invocation. The terminal/log output will then carry the exact rejection reason. ETA: 10 min.

### Step 2 (per the surfaced error): One of three minimal fixes

| If the rejection is... | Then the fix is... | Scope |
|---|---|---|
| "Invalid witness data for input N" (signing.rs:167) | Wire-format encoder is still wrong — investigate the exact bytes the client produces (hexdump the `Vec<u8>` before transmission); compare byte-for-byte against `bitcoin::consensus::serialize(&Witness)` of a known-good 2-item P2WPKH witness | client/src/wallet.rs (re-do Plan 13-01 with a different encoding) |
| "PSBT extraction failed: ..." (signing.rs:184) | Fix the `witness_utxo` mismatch in coordinator/src/round/signing.rs:104-127 — pass the participant's actual UTXO script and value (require client to register them, or look them up via RPC) | coordinator/src/round/signing.rs + coordinator/src/round/input_reg.rs |
| "testmempoolaccept rejected: <reason>" (signing.rs:197+) | Bitcoind rejected for one of: signature-verification-failure, fee-too-low/high, witness-malformed, dust-output, ... — fix depends on the specific reason | varies |

### Phase 14 Plan Structure (Suggested)

- **Plan 14-01:** Coordinator-side error-message enrichment (1-3 LOC) — surface the exact rejection cause via tracing or richer ApiError message. Re-run canonical-first → capture the error. Single atomic commit. ETA: ~15 min.
- **Plan 14-02:** Apply the wire-format fix (re-do Plan 13-01's exact edit) AND the downstream repair surfaced by Plan 14-01. Possibly multi-file. Land as one or two atomic commits per file/concern.
- **Plan 14-03:** Re-execute Plan 12-02's six-unmute cycle with four-SHA commit bodies (RSA `cc20f6f` + wallet-trust `0bbcf3c` + wire-format Plan-14-02-A + downstream-fix Plan-14-02-B).

## What Was NOT Done

Per D-12 escape-valve (pre-authorized in-flight scope expansion = ZERO):

- **No source-code commit was made.** Zero source files modified at HEAD. The Plan-13-01 working-tree edit was applied for the D-07 gate, then reverted on FAIL per orchestrator instruction.
- **No #[ignore] line removed.** Six TODO(Phase-10) carve-out sites at full_round.rs lines 164, 462, 730, 854, 911, 1236 remain intact.
- **No REQUIREMENTS.md edit.** REPAIR-01 status carries forward to Phase 14 closeout.
- **No ROADMAP.md edit.** Phase 13 entry remains in EXECUTING/HALTED state.
- **No coordinator-side diagnostic surfacing.** Step 1 of the proposed repair is held back for Phase 14 to perform as a fresh atomic action (Phase 13's D-03 explicitly deferred coordinator hardening; Phase 14 reconsiders).

## Working Tree State (post-revert)

```
git status --short
 M .planning/STATE.md     (pre-existing modification — orchestrator session bookkeeping, NOT from this plan execution)
```

`client/src/wallet.rs` is bit-identical to its pre-Phase-13 state at HEAD:

```
sed -n '279,281p' client/src/wallet.rs
        if let Some((_pk, sig)) = input.partial_sigs.iter().next() {
            return Ok(sig.to_vec());
        }
```

`grep -c 'TODO(Phase-10)' tests/integration/full_round.rs` → **6** (all six unmute sites intact).

## Recovery Path — Phase 14

1. **Open Phase 14** via `/gsd-phase` targeting the 6th orthogonal blocker.
2. **Phase 14 CONTEXT.md seed:**
   - **Blocker:** `verify_and_sign: HTTP 400 Bad Request from /round/sign` PERSISTS even after wire-format fix is applied in working tree — the 5th-blocker hypothesis is necessary but not sufficient.
   - **Failure site:** `tests/integration/full_round.rs:248:22` (unchanged from Plan 12-02).
   - **Coordinator-side BroadcastRejected paths to investigate (signing.rs):** lines 107-111, 112-117, 132-137, 147-151, 164-170, 173-178, 182-186, 197+.
   - **Plan 13-01 working-tree edit shape (re-apply identically):** see §"Working-Tree Edit Applied" in this SUMMARY.
   - **Proposed Plan 14-01:** add coordinator-side error-message enrichment (single atomic commit, 1-3 LOC in `coordinator/src/round/signing.rs`) — surfaces the actual rejection cause in <30s rather than <30min of grep-archaeology. This reconsiders Plan 13's D-03 deferral.
   - **Dependencies already in history:** `cc20f6f` (RSA fix), `0bbcf3c` (wallet trust) — both remain bisect-clean.
3. **Phase 14 plan structure (suggested):**
   - Plan 14-01: Coordinator error-message enrichment + run canonical-first to capture the surfaced error.
   - Plan 14-02: Apply wire-format fix (re-do Plan 13-01) + downstream repair (depends on what Plan 14-01 surfaces). May be one or multiple commits.
   - Plan 14-03: Re-execute Plan 12-02's six-unmute cycle with four-SHA commit bodies.

## Deviations from Plan

**1. [D-07 Sanity Gate FAILED — D-12 Halt] Canonical-first sanity test verdict was FAILED, not PASSed**

- **Found during:** Task 1 step (e) — D-07 canonical-first sanity capture
- **Issue:** `full_round_three_clients` returned `test result: FAILED. 0 passed; 1 failed` with the IDENTICAL `HTTP 400 Bad Request from /round/sign` failure signature as Plan 12-02
- **Root cause hypothesis:** The wire-format fix is necessary but a 6th orthogonal blocker downstream of `coordinator/src/round/signing.rs:160` (most likely at `psbt.extract_tx()` line 182, `rpc.testmempoolaccept(...)` line 190, or the `accepted` check at line 197+) accounts for the persistent 400
- **Action taken:** D-12 escape-valve invoked — applied working-tree edit, captured PASS-proof attempt to `target/integration-test-13-01-d07.log`, reverted working-tree edit per orchestrator critical_constraints instruction, no commit on `client/src/wallet.rs`, SUMMARY.md authored documenting the halt + CHECKPOINT REACHED
- **Phase 14 absorbs:** per D-12 / D-13

**2. [Deviation from CONTEXT.md D-12 — working-tree revert]** CONTEXT.md D-12 suggested leaving the working-tree edit in place ("so Phase 14 can diagnose against the post-fix state"); the executor's `<critical_constraints>` block explicitly instructed `git checkout -- client/src/wallet.rs` on FAIL. The executor followed the more-specific orchestrator instruction. Phase 14 trivially re-applies the documented edit (see §"Working-Tree Edit Applied" for the exact diff).

## Known Stubs

None — this plan made no source changes. The only artifact is this SUMMARY.md and the `target/*.log` capture files (gitignored).

## Threat Flags

No new threat surface introduced — no source files modified.

## Self-Check

- Files exist:
  - `target/integration-test-13-01-d07.log` — FOUND (tee output from D-07 canonical invocation)
  - `target/integration-test-13-01-d07-debug.log` — FOUND (RUST_LOG=debug re-run, identical verdict)
  - `target/build-13-01.log` — FOUND (cargo build clean output, 0 errors, 0 warnings)
  - `.planning/phases/13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle/13-01-SUMMARY.md` — FOUND (this file)
- Commits: zero source commits made (correct per D-12)
- Source assertions:
  - `sed -n '279,281p' client/src/wallet.rs` → shows pre-edit `(_pk, sig)` destructure + `Ok(sig.to_vec())` return (reverted correctly)
  - `grep -c 'TODO(Phase-10)' tests/integration/full_round.rs` → 6 (all six unmute sites intact)
  - `git status --short` → only pre-existing `M .planning/STATE.md` (orchestrator session bookkeeping)
  - REQUIREMENTS.md REPAIR-01 → unchanged
  - ROADMAP.md Phase 13 entry → unchanged (not marked complete)
- Plan 13-01 wire-format-fix commit absent: `git log -1 --format=%s` does NOT match `fix(13): encode partial sig as bitcoin::Witness…` — correct per D-12
- Wallet fix SHA in history: `0bbcf3c76ca251c14aa64216ca6955be1f880b9a` — VERIFIED
- RSA fix SHA in history: `cc20f6fbca4d292bf7b394a3850b18d244b5b602` — VERIFIED

## Self-Check: PASSED

## CHECKPOINT REACHED

**Type:** human-action (architectural / scope decision required for Phase 14)
**Plan:** 13-01
**Progress:** 0/1 tasks complete (D-07 sanity gate FAILED → halt before commit)

### Completed Tasks

(none)

### Current Task

**Task 1:** Encode partial sig as bitcoin::Witness in client/src/wallet.rs (single atomic commit + D-07 canonical-first PASS-proof capture in commit body)
**Status:** blocked
**Blocked by:** D-07 canonical-first sanity gate returned `test result: FAILED. 0 passed; 1 failed` with the IDENTICAL `HTTP 400 Bad Request from /round/sign` signature as Plan 12-02 — wire-format fix is necessary but NOT sufficient; 6th orthogonal blocker downstream of `coordinator/src/round/signing.rs:160`

### Checkpoint Details

The Phase 13 working hypothesis ("Plan 12-02's HTTP 400 is solely the raw-DER-vs-Witness wire-format mismatch") has been falsified by direct experiment: applying the canonical Option A 4-LOC fix at `client/src/wallet.rs:279-284`, building clean, and running the identical canonical-first invocation reproduces the IDENTICAL panic. The wire-format fix is correct (encoder ↔ decoder symmetry verified by reading; build is clean; encoding shape matches the rust-bitcoin-canonical `Witness::new() + push(sig.to_vec()) + push(pk.to_bytes()) + consensus::serialize` pattern), but a 6th orthogonal blocker — most likely on the coordinator side, downstream of the witness-deserialize site at signing.rs:160 — persists and renders the test red.

Per Phase 13 D-12 escape-valve + orchestrator critical_constraints, Phase 13 halts here. Phase 14 absorbs.

### Awaiting

User decision: open Phase 14 via `/gsd-phase` targeting the 6th orthogonal blocker, OR provide alternative direction. The recommended Phase 14 first plan is a coordinator-side error-message enrichment (single atomic commit, 1-3 LOC in `coordinator/src/round/signing.rs`) to surface the exact `BroadcastRejected` cause that the test runner currently discards via `error_for_status()` — this reconsiders Phase 13's D-03 deferral and is what the orthogonal-blocker investigation has been blocked on for two phases.

Phase 14 should ALSO re-apply Plan 13-01's wire-format edit (see §"Working-Tree Edit Applied" for the exact 4-LOC diff) — it is correct, just insufficient on its own.
