---
phase: 14-sprint-0-spikes-discuss-phase-decisions
plan: 02
subsystem: infra
tags: [bdk-wallet, p2tr, bip322, taproot, schnorr, spike, sprint-0]

# Dependency graph
requires:
  - phase: 14-sprint-0-spikes-discuss-phase-decisions
    provides: "Plan 14-01 (Sprint-0-A GO verdict on bip322 0.0.10 ADOPT) — independent spike per D-17 but precedes 14-02 by execution order"
provides:
  - "Sprint-0-B verdict PASS (Open Decision #4): bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness for BIP-322 P2TR descriptors"
  - ".planning/research/sprint-0-B.md as canonical record (in main; PoC source verbatim, witness hex, verify_schnorr Ok(()) result)"
  - "Spike branch spike/14-B-bdk-p2tr-poc pushed to origin at SHA 9ff73cd286920d1e9fcac1e6506e7e3300b7abe7 for reproducibility (NOT merged to main per D-19)"
  - "Implementation note for Phase 17: bdk finalizes single-key taproot, sig lives in final_script_witness (not tap_key_sig)"
affects:
  - 14-03 (ADR Decision #4 STATUS → ACCEPTED (bdk path))
  - 17 (WALLET-02 BIP-322 signing for P2TR uses bdk path; D-15 manual fallback does not fire)

# Tech tracking
tech-stack:
  added: []  # No new dependencies on main; bdk_wallet 2.3 + bitcoin 0.32 already in workspace
  patterns:
    - "Throwaway PoC in client/examples/ (per CD-4): Cargo auto-discovery, excluded from cargo build --release, invoked via `cargo run -p <crate> --example <name>`"
    - "Spike-branch isolation pattern: PoC binary committed only to spike/14-B-bdk-p2tr-poc; doc-record (.planning/research/sprint-0-B.md) cherry-picked to main; production code paths untouched (D-21)"
    - "Single-key taproot witness extraction: prefer psbt.inputs[0].tap_key_sig; fall back to psbt.inputs[0].final_script_witness[0] (64-byte slice) when bdk finalizes"

key-files:
  created:
    - ".planning/research/sprint-0-B.md (on main via cherry-pick — canonical record)"
    - "client/examples/spike-p2tr.rs (on spike branch ONLY — throwaway PoC)"
  modified: []

key-decisions:
  - "Open Decision #4 → PASS / bdk path: bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness for BIP-322 P2TR; verify_schnorr returned Ok(()); Phase 17 WALLET-02 uses bdk path, D-15 manual fallback does not fire"
  - "bdk 2.3 finalizes single-key taproot — sig ends up in final_script_witness (1 element, 64 bytes), not tap_key_sig; Phase 17 extraction must look in both fields (parallel to client/src/wallet.rs:277-285 P2WPKH branch)"

patterns-established:
  - "Spike PoC location: client/examples/<spike-name>.rs (CD-4 locked); inherits client crate deps; invoked via `cargo run -p client --example <spike-name>`"
  - "Stdout-parseable verdict shape: STEP_<N>_<NAME>: <Ok|Err: ...>, WITNESS_HEX: <hex>, VERDICT: <PASS|FAIL> — enables byte-deterministic grep by downstream planners"

requirements-completed: []  # Plan has no requirements (frontmatter requirements: []); Phase 14 is gating, ADR-producing

# Metrics
duration: 6min
completed: 2026-05-29
---

# Phase 14 Plan 02: Sprint-0-B bdk_wallet 2.3 P2TR PoC Summary

**PoC PASS — bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness for BIP-322 P2TR descriptors; verify_schnorr returned Ok(()); Phase 17 WALLET-02 uses the bdk path and D-15's manual fallback does not need to fire.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-05-29T23:36:48Z
- **Completed:** 2026-05-29T23:43:13Z
- **Tasks:** 3 / 3
- **Files modified:** 2 (1 on main: sprint-0-B.md; 1 on spike branch: spike-p2tr.rs)
- **Sprint cap:** 2 days (D-18) — completed in ~6 min, well within cap

## Accomplishments

- **Resolved Open Decision #4 with a binary PASS verdict.** Eight-step PoC in `client/examples/spike-p2tr.rs` exercised the full sign + verify cycle: BIP-86 P2TR descriptor → BIP-322 `to_spend`/`to_sign` via `shared::bip322` primitives → PSBT with `SignOptions { trust_witness_utxo: true }` and real on-chain `witness_utxo` → `wallet.sign(...)` returned `Ok(finalized=true)` → recovered 64-byte Schnorr witness from `final_script_witness` → `secp256k1::verify_schnorr(...)` returned `Ok(())`.
- **Captured the recovered witness hex deterministically.** Schnorr sig `295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069` (128 hex chars = 64 bytes; sighash_type Default, no trailing byte) reproducible byte-for-byte from the hardcoded `[0u8; 32]` seed.
- **Surfaced the bdk 2.3 single-key taproot finalize quirk for Phase 17.** bdk moves the sig from `tap_key_sig` into `final_script_witness` after finalizing single-key keyspend inputs; sprint-0-B.md documents the dual-path extraction and notes Phase 17 WALLET-02 inherits this (parallels the existing client/src/wallet.rs:277-285 P2WPKH fallback).
- **Maintained D-21 structural invariant.** Zero production code (`coordinator/`, `client/src/`, `shared/`, `liquidity-bot/`) committed to main from this plan; PoC binary lives only on the spike branch; doc-only `.planning/research/sprint-0-B.md` cherry-picked to main as the canonical record per D-19.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create spike branch with client/examples/spike-p2tr.rs PoC** — `9ff73cd` (spike, on `spike/14-B-bdk-p2tr-poc` branch only)
2. **Task 2: Write sprint-0-B.md with PoC excerpt, witness hex, verdict, recommendation** — committed on spike branch as `8aca00a`; cherry-picked to main as `efd8d59`
3. **Task 3: Push spike branch to origin and verify structural D-21 invariant** — no commits (verification only); `git push -u origin spike/14-B-bdk-p2tr-poc` succeeded with `=== pre-push: all checks passed ===`

**Plan metadata commit:** to follow (this SUMMARY.md + STATE.md/ROADMAP.md updates)

## Files Created/Modified

- **`.planning/research/sprint-0-B.md`** (NEW on main, 364 lines) — Canonical Sprint-0-B record. Embeds full PoC source verbatim, captured stdout, witness hex, verify_schnorr result (`Ok(())`), overall verdict (`PASS: ...`), recommendation (`bdk path`), and reproducibility metadata (spike SHA, toolchain pins).
- **`client/examples/spike-p2tr.rs`** (NEW on spike branch ONLY, 263 lines) — Throwaway PoC. Lives only at `spike/14-B-bdk-p2tr-poc` HEAD `9ff73cd`; deliberately NOT in main. Uses `cargo run -p client --example spike-p2tr` invocation (the `-p client` selector is required because the workspace root has no package).

## Decisions Made

- **PoC location: `client/examples/spike-p2tr.rs`** (CD-4 default; locked at Plan write-time). Cargo auto-discovers; excluded from `cargo build --release` by convention; reuses `client` crate's existing dev/runtime deps (no `client/Cargo.toml` edits needed).
- **Step 7 extraction broadened to also read `final_script_witness`.** The plan's literal wording ("Extract `psbt.inputs[0].tap_key_sig`") would have produced a misleading FAIL because bdk 2.3 finalizes single-key taproot keyspend, moving the sig out of `tap_key_sig` into `final_script_witness[0]`. The PoC correctly answers Open Decision #4's substantive question ("did bdk produce a valid 64-byte Schnorr witness?") by checking both fields and pinning down which path bdk takes. Documented inline in sprint-0-B.md as a Phase 17 implementation note. (See Deviations §1 below.)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Step 7 must also recover sig from `final_script_witness` after bdk finalizes**

- **Found during:** Task 1 (initial PoC run)
- **Issue:** The plan's step 7 wording read literally was `let tap_key_sig = psbt.inputs[0].tap_key_sig;` followed by `Some(_)` → PASS / `None` → FAIL. But bdk_wallet 2.3 finalizes single-key P2TR keyspend inputs, which means after `wallet.sign(...)` returns `Ok(finalized=true)`, `psbt.inputs[0].tap_key_sig` is cleared and the 64-byte Schnorr sig lives in `psbt.inputs[0].final_script_witness[0]`. A strict-literal Step 7 read would have emitted `VERDICT: FAIL` even though bdk successfully produced and verified the canonical taproot witness — a strawman FAIL that would poison Phase 17 planning.
- **Fix:** Step 7 now checks `tap_key_sig` first, falls back to `final_script_witness` (validates `elems.len() == 1 && elems[0].len() ∈ {64, 65}` per BIP-341 keyspend witness shape), and extracts the first 64 bytes as the canonical Schnorr sig. Step 8 verifies the recovered sig against the BIP-341 keyspend sighash via `secp256k1::verify_schnorr` regardless of which PSBT field held it. The dual-path behavior is documented inline in the PoC source (lines 149-166) and surfaced as a Phase 17 implementation note in sprint-0-B.md so WALLET-02 inherits the dual extraction.
- **Files modified:** `client/examples/spike-p2tr.rs` (Steps 7 + 8, on spike branch only); `.planning/research/sprint-0-B.md` reflects the broadened extraction
- **Verification:** PoC now emits `STEP_6_BDK_SIGN: Ok(finalized=true)` + `STEP_7_EXTRACT_TAP_KEY_SIG: Some(final_script_witness elems=1 first_len=64 sig64=...)` + `STEP_8_VERIFY_SCHNORR: Ok` + `VERDICT: PASS`. The verdict is substantive — bdk did the work; the PoC correctly recognized it.
- **Committed in:** `9ff73cd` (Task 1's PoC commit on spike branch — the extraction logic landed in the initial PoC commit; no separate fix commit needed)

**2. [Rule 1 - Bug] Step 8 sighash byte-array → `secp256k1::Message` conversion**

- **Found during:** Task 1 (compile failure)
- **Issue:** The plan's step 8 sketch passed `expected_sighash.as_byte_array()` directly to `verify_schnorr`. In secp256k1 0.29.1 (the version transitively pinned via bitcoin 0.32.8), `verify_schnorr` takes `msg: &Message`, not `&[u8; 32]`. Also, the `bitcoin::TapSighash` type does not expose `as_byte_array` (the `bitcoin::hashes::Hash` trait method is named `to_byte_array`, and the trait must be in scope).
- **Fix:** Imported `bitcoin::hashes::Hash` (for `to_byte_array`) and `bitcoin::secp256k1::Message`; converted via `let msg = Message::from_digest(expected_sighash.to_byte_array());` then called `secp.verify_schnorr(&sig, &msg, &xonly)`.
- **Files modified:** `client/examples/spike-p2tr.rs` (on spike branch only)
- **Verification:** PoC compiles and `verify_schnorr` returns `Ok(())`.
- **Committed in:** `9ff73cd` (Task 1's PoC commit on spike branch)

**3. [Rule 3 - Blocking] Silence deprecation warnings on `SignOptions`**

- **Found during:** Task 1 (compile output)
- **Issue:** bdk_wallet 2.3 marked `SignOptions` deprecated with a "PSBT signing was moved to bitcoin::psbt" note, even though the deprecation target migration is not yet wired up in 2.3 itself and the production client wallet still uses the same struct (client/src/wallet.rs:269 with `#[allow(deprecated)]`). Compiler emitted three warnings on the PoC. Not a true blocker, but produced noise in the captured stdout, complicating the parser-friendly grep target.
- **Fix:** Added `#![allow(deprecated)]` at the crate root of the example with an inline comment explaining the parallel with client/src/wallet.rs:269. Kept the import path identical (`bdk_wallet::signer::SignOptions`) so Phase 17's eventual migration is a one-line change in both places.
- **Files modified:** `client/examples/spike-p2tr.rs` (on spike branch only)
- **Verification:** `cargo run -p client --example spike-p2tr` produces clean stdout (no warnings between the cargo `Finished`/`Running` lines and the verdict block).
- **Committed in:** `9ff73cd` (Task 1's PoC commit on spike branch)

---

**Total deviations:** 3 auto-fixed (2 bug, 1 blocking)
**Impact on plan:** All three fixes were necessary to make the PoC test the substantive question (does bdk produce a valid keypath witness?) rather than a strawman shape. None expanded scope — all stayed inside the spike branch and inside Task 1's single commit. Sprint-0-B's overall verdict (PASS, bdk path) is unchanged from what the plan's literal-shape verdict would have been if bdk's finalization quirk had been pre-known; the fixes only make the PoC robust to bdk's actual code path.

## Issues Encountered

- **Initial Step 7 read produced a misleading `VERDICT: FAIL`.** bdk reported `Ok(finalized=true)` but `tap_key_sig` was `None`. Investigation showed bdk had cleared `tap_key_sig` because the PSBT was finalizable and the sig had been moved into `final_script_witness`. Resolved via Deviation #1 (broaden Step 7 extraction). The investigation surfaced a useful Phase 17 implementation note that is now in sprint-0-B.md.

## User Setup Required

None — the spike branch + canonical sprint-0-B.md are sufficient. No external services, no env vars, no manual configuration. To reproduce locally:
```
git fetch origin spike/14-B-bdk-p2tr-poc
git checkout spike/14-B-bdk-p2tr-poc
cargo run -p client --example spike-p2tr
```

## Next Phase Readiness

- **Plan 14-03 (ADR) unblocked.** It can now set ADR Decision #4 STATUS line to `ACCEPTED (bdk path)` deterministically — sprint-0-B.md contains the grep target `bdk path` at column 0 in the Recommendation section and `PASS:` at column 0 in the Overall verdict section.
- **Phase 17 WALLET-02 unblocked.** No new `shared/src/bip322/p2tr.rs::sign_p2tr_keypath` is needed. The client's P2TR sign path reuses `bdk_wallet::Wallet::sign(...)` with `SignOptions { trust_witness_utxo: true }`. The witness extraction must check `psbt.inputs[0].tap_key_sig` first and fall back to `psbt.inputs[0].final_script_witness[0]` (64 bytes) when bdk finalizes — same dual-path pattern as the existing P2WPKH branch.
- **D-15 fallback retired for v1.4.** The 80-LOC `shared/src/bip322/p2tr.rs::sign_p2tr_keypath` budget is freed; v1.4 does not need to write it. (D-15 stays on the books for a v1.5 reconsideration if bdk_wallet ever regresses on taproot keyspend.)
- **No blockers, no carry-forward issues.** Sprint completed in ~6 min (well inside the D-18 2-day cap).

## TDD Gate Compliance

Plan 14-02 frontmatter type is `execute` (not `tdd`), so TDD gates do not apply. No TDD commits expected.

## Self-Check

PASSED. Verified inline:

- `.planning/research/sprint-0-B.md` exists on main: confirmed via `git log main -- .planning/research/sprint-0-B.md | head -1` → commit `efd8d59`.
- `client/examples/spike-p2tr.rs` exists on spike branch only: confirmed via `test -f client/examples/spike-p2tr.rs` on main → file MISSING (correct); checked out on spike branch via `git show spike/14-B-bdk-p2tr-poc:client/examples/spike-p2tr.rs` → file present at 263 lines.
- Commit `9ff73cd286920d1e9fcac1e6506e7e3300b7abe7` exists on spike branch.
- Commit `8aca00aff81862b29564d296f431123ce52e7955` exists on spike branch (sprint-0-B.md on spike).
- Commit `efd8d59` exists on main (cherry-picked sprint-0-B.md).
- Spike branch on origin: confirmed via `git ls-remote --heads origin spike/14-B-bdk-p2tr-poc` → ref present.
- D-21 invariant: `git log 9414a23..main --oneline -- coordinator/ client/ shared/ liquidity-bot/` is empty.

---
*Phase: 14-sprint-0-spikes-discuss-phase-decisions*
*Completed: 2026-05-29*
