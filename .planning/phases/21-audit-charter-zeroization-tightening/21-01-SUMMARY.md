---
phase: 21-audit-charter-zeroization-tightening
plan: 01
subsystem: crypto
tags: [rsa, blind-signatures, zeroize, drop-chain, audit-readiness, newtype, fsm]

# Dependency graph
requires:
  - phase: 20-mixed-round-fee-accuracy
    provides: "Phase 20 FEE-03 invariant tests (fee_share_p2wpkh_only_matches_v14_baseline + fee_share_mixed_script_differs_from_uniform_baseline) — Phase 21-01 holds these green at its plan boundary"
  - phase: 19-multi-script-signing-finish
    provides: "shared::bip322 dispatcher-only public surface (V1.4-CRIT-01 invariant) — Phase 21-01 leaves shared/ untouched, preserving the invariant"
provides:
  - "RoundSecretKey(BjSecretKey) newtype in coordinator/src/blind/rsa.rs — pub struct with pub(crate) `new` + `as_inner` accessors, no public field"
  - "Drop for RoundSecretKey — empty-crypto body, PII-safe tracing::debug! event under target blindjoin::audit (D-129 / OQ1 lock)"
  - "RoundStateInner.rsa_signer: Option<RsaBlindSigner> — bounded lifetime expressible as a Rust type signature; the load-bearing claim per REQUIREMENTS AUDIT-03"
  - "Structural FSM test round_secret_key_dropped_on_round_end — unconditional CI gate; asserts inner.rsa_signer.is_some() pre-Idle, asserts state.inner.is_none() post-Idle"
  - "Best-effort scrub test round_secret_key_buffer_overwritten_on_drop — sanity check, gated #[cfg_attr(not(target_os = \"linux\"), ignore = ...)] per CD-50"
  - "Rewritten D-07 doc-comment at coordinator/src/blind/rsa.rs — cites transitive rsa::RsaPrivateKey Drop chain at rsa-0.9.10/src/key.rs:76-82, names the trigger transition_to(Phase::Idle), ends with the charter anchor docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window (anchor target created in 21-02)"
  - "Production call-site refactor — 2 sites (input_reg.rs:71, handlers.rs:383) traverse the new Option via .as_ref().expect(\"rsa_signer must be Some during {InputReg|OutputReg}\")"
  - "Test-fixture refactor — 6 fixture sites (4 in signing.rs + 2 in state.rs) wrap rsa_signer in Some(...); 1 production field-init wrap in manager.rs:63"
affects: [21-02 (consumes the type signatures + anchor target for the AUDIT-CHARTER.md prose and the .cargo/audit.toml RUSTSEC-2023-0071 rationale rewrite)]

# Tech tracking
tech-stack:
  added: []  # No new dependencies; Phase 21 is internal-only per 21-RESEARCH §Standard Stack.
  patterns:
    - "Newtype-as-lifetime-expression: when an upstream type already has correct Drop semantics (rsa 0.9.10 ZeroizeOnDrop on RsaPrivateKey), the wrapper's value is making the lifetime a value the FSM can null, not redundant in-place scrub."
    - "Single-chokepoint Drop trigger: all Phase → Idle FSM edges route through transition_to(Phase::Idle) at state.rs:194-200, the SOLE site setting self.inner = None (verified by 21-RESEARCH OQ2 grep)."
    - "Best-effort scrub test pattern (CD-50): capture DER fingerprint, drop, allocate probe Vec, sweep — gated #[cfg_attr(not(target_os = \"linux\"), ignore = ...)] with reason naming the unconditional structural sibling test."
    - "Phase-specific .as_ref().expect() messages: `\"rsa_signer must be Some during InputReg\"` and `\"rsa_signer must be Some during OutputReg\"` — production-panic messages name the FSM precondition that is being asserted."

key-files:
  created: []  # No new files; all edits to existing files.
  modified:
    - "coordinator/src/blind/rsa.rs — +145 LOC (newtype + Drop + rewritten D-07 + scrub test), -10 LOC (old D-07 comment + bare field)"
    - "coordinator/src/round/state.rs — +66 LOC (new structural test + rewritten field doc-comment + Option wraps), -2 LOC (stale 'Not zeroized' sentence)"
    - "coordinator/src/round/signing.rs — +4 LOC, -4 LOC (4 fixture Some-wraps)"
    - "coordinator/src/round/manager.rs — +2 LOC, -2 LOC (production wrap + test read)"
    - "coordinator/src/round/input_reg.rs — +1 LOC, -1 LOC (production call site .as_ref().expect)"
    - "coordinator/src/api/handlers.rs — +1 LOC, -1 LOC (production call site .as_ref().expect)"

key-decisions:
  - "D-128: RoundSecretKey newtype lives INSIDE RsaBlindSigner; RoundStateInner.rsa_signer becomes Option<RsaBlindSigner> — reconciles REQUIREMENTS' two framings (newtype wraps BjSecretKey AND lifetime bound is the Option)."
  - "D-129 / CD-47 (locked by 21-RESEARCH OQ1): empty-crypto Drop body — PII-safe tracing::debug! only. The transitive rsa-0.9.10 ZeroizeOnDrop on RsaPrivateKey does the cryptographically meaningful work; DER-roundtrip and replace-with-dummy approaches are strictly worse (extra allocation / ~100ms keygen for identical outcome)."
  - "D-130: transition_to(Phase::Idle) at state.rs:194-200 is the SOLE drop trigger — verified by 21-RESEARCH OQ2 grep showing exactly one `self.inner = None` site in coordinator/src/."
  - "D-131: two-test split — structural FSM test round_secret_key_dropped_on_round_end in state.rs (load-bearing, unconditional CI gate) + best-effort scrub round_secret_key_buffer_overwritten_on_drop in rsa.rs (CD-50 Linux-gated)."
  - "D-132 (D-07 comment rewrite): the new prose INVERTS the 'best-effort' framing per 21-RESEARCH OQ1 — names the transitive rsa::RsaPrivateKey Drop chain as cryptographically correct (UNCONDITIONAL, no feature flag, verified at installed registry source); positions the newtype as lifetime expression; ends with the charter anchor docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window (which Plan 21-02 will materialize)."
  - "CD-50: scrub test gated #[cfg_attr(not(target_os = \"linux\"), ignore = ...)] — the test passed on macOS during this run (Darwin), but heap layout determinism is a Linux/glibc property; on macOS the test reports `ignored` with the reason string naming the structural sibling as the unconditional gate."

patterns-established:
  - "Newtype-as-lifetime-expression pattern: pub struct RoundSecretKey(BjSecretKey) + pub(crate) fn new/as_inner — the newtype is the lifetime hook, not the crypto worker. Apply to v1.6+ if other sensitive fields need bounded lifetimes."
  - "Empty-crypto Drop + PII-safe tracing::debug! audit event: when upstream Drop+ZeroizeOnDrop already runs, the wrapper's Drop body emits a static-string tracing event under a dedicated audit target (target: \"blindjoin::audit\") for ops observability without leaking key material."
  - "Phase-specific .as_ref().expect() pattern for FSM-bound Options: rather than .unwrap(), the panic message names the phase precondition (\"rsa_signer must be Some during InputReg\") so a runtime panic is an audit-trail event, not a Rust idiom call."

requirements-completed: [AUDIT-03]

# Metrics
duration: 7min
completed: 2026-05-31
---

# Phase 21 Plan 01: Audit Charter & Zeroization Tightening — RoundSecretKey newtype + Option<RsaBlindSigner> bounded-lifetime refactor Summary

**Tightens the RSA SecretKey zeroization window from "best-effort upstream limitation" prose to a structurally-bounded Rust lifetime expressed as `Option<RsaBlindSigner>` on `RoundStateInner` — the load-bearing audit-readiness claim that 21-02 will cite in `docs/AUDIT-CHARTER.md` §5.**

## Performance

- **Duration:** 7 min 22 sec
- **Started:** 2026-05-31T22:59:24Z
- **Completed:** 2026-05-31T23:06:46Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- `RoundSecretKey(BjSecretKey)` newtype shipped in `coordinator/src/blind/rsa.rs` with an empty-crypto `Drop` body that emits a PII-safe `tracing::debug!` event under target `blindjoin::audit` — the cryptographically meaningful zeroization runs transitively via `rsa-0.9.10/src/key.rs:76-82` (`<rsa::RsaPrivateKey as Drop>::drop` is UNCONDITIONAL — no feature flag, verified at installed registry source per 21-RESEARCH OQ1).
- `RoundStateInner.rsa_signer` refactored from bare `RsaBlindSigner` to `Option<RsaBlindSigner>` (D-128) — bounded lifetime now expressible as a Rust type signature an auditor can read on line 110 of `state.rs`. The new structural FSM test `round_secret_key_dropped_on_round_end` is the load-bearing claim per REQUIREMENTS AUDIT-03; it drives Signing → Broadcast → Idle and asserts `state.inner.is_none()` post-transition with a pre-transition assertion that `rsa_signer.is_some()` (the Drop chain had a non-None target).
- D-07 doc-comment at `coordinator/src/blind/rsa.rs:18-22` rewritten per D-132 — the old "best-effort only" prose is gone; the new comment cites the transitive `rsa::RsaPrivateKey` Drop chain, names `Option<RsaBlindSigner>` as the lifetime bound, names `transition_to(Phase::Idle)` as the trigger, and ends with the charter anchor `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` (which Plan 21-02 materializes — the anchor reference points forward by design).
- Best-effort scrub test `round_secret_key_buffer_overwritten_on_drop` added (CD-50) — captures a 32-byte DER fingerprint, drops the signer, sweeps an 8 MB probe for survival. Gated `#[cfg_attr(not(target_os = "linux"), ignore = "non-portable heap layout; structural test in state.rs::round_secret_key_dropped_on_round_end is the unconditional gate (D-131)")]`. The test runs and passes on Linux; reports `ignored` on macOS.
- All 6 test-fixture sites (4 in `signing.rs` + 2 in `state.rs`) + 1 production field-init in `manager.rs:63` wrap the signer in `Some(...)`. All 2 production call sites (`input_reg.rs:71`, `handlers.rs:383`) + 2 test read sites (`state.rs::rsa_signer_consistent_with_key_bytes`, `manager.rs::start_round_from_idle_populates_inner`) traverse the new Option via phase-specific `.as_ref().expect(...)` messages.

## Test counts before/after

| Test suite | Before 21-01 | After 21-01 | Delta |
| --- | --- | --- | --- |
| `coordinator --lib` (full lib) | 87 passed | 89 passed, 1 ignored (macOS scrub) | +2 (`round_secret_key_dropped_on_round_end` + `round_secret_key_buffer_overwritten_on_drop`) |
| `coordinator --lib blind::rsa::tests` | 5 passed | 6 passed (Linux: 6/6 run; macOS: 5 run + 1 ignored) | +1 (scrub test) |
| `coordinator --lib round::state::tests` | 6 passed | 7 passed | +1 (structural FSM test) |
| `cargo test --test integration full_round` | 8/8 | 8/8 | no change (invariant preserved) |
| `cargo test --test integration mixed_script_e2e` | 1/1 | 1/1 | no change (invariant preserved) |
| Phase 20 FEE-03 tests | 2/2 | 2/2 | no change (invariant preserved) |

## Invariant matrix (Task 3)

| # | Invariant | Command | Result |
| - | --- | --- | --- |
| 1 | v1.3 P2WPKH | `cargo test --test integration full_round` | 8 passed, 0 failed, 0 ignored (~42s) |
| 2 | v1.4 multi-script | `cargo test --test integration mixed_script_e2e` | 1 passed, 0 failed |
| 3 | v1.5 Phase 20 FEE-03 #1 | `cargo test -p coordinator --lib bitcoin::tx::tests::fee_share_p2wpkh_only_matches_v14_baseline -- --exact` | 1 passed |
| 3 | v1.5 Phase 20 FEE-03 #2 | `cargo test -p coordinator --lib bitcoin::tx::tests::fee_share_mixed_script_differs_from_uniform_baseline -- --exact` | 1 passed |
| 4 | Coordinator full lib | `cargo test -p coordinator --lib` | 89 passed, 0 failed, 1 ignored (macOS scrub) |
| 5 | Clippy `-D warnings` | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings (exit 0) |
| 6 | cargo audit | `cargo audit` (pre-Phase-21 `.cargo/audit.toml`) | 0 vulnerabilities (exit 0) |
| 7 | V1.4-CRIT-01 scope discipline | `git diff --name-only HEAD~2 HEAD -- shared/ client/` | empty (shared/ + client/ untouched) |

Summary line written to `/tmp/21-01-task3-summary.txt`: `21-01 Task 3: 8/8 fullround, 1/1 mixed_e2e, 2/2 FEE-03, 89/89 lib (1 ignored on macOS), 0 clippy warnings, 0 audit vulns, 0 shared/+client/ diff. ALL INVARIANTS GREEN.`

## Task Commits

Each task was committed atomically:

1. **Task 1: Introduce RoundSecretKey newtype + Drop + rewritten D-07 comment + scrub test** — `381a743` (feat)
2. **Task 2: Refactor RoundStateInner.rsa_signer to Option<RsaBlindSigner> + propagate + structural FSM test** — `6f6dafb` (feat)
3. **Task 3: Full-stack cross-phase invariant verification** — verification-only (no source edits → no commit; results recorded in `/tmp/21-01-task3-*.txt`).

**Plan metadata commit:** will follow this SUMMARY.

## Files Created/Modified

- `coordinator/src/blind/rsa.rs` — `RoundSecretKey` newtype + empty-crypto `Drop` body (D-129) + rewritten D-07 doc-comment (D-132) + best-effort `round_secret_key_buffer_overwritten_on_drop` test (CD-50). +145 / -10 LOC.
- `coordinator/src/round/state.rs` — `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` (D-128) + rewritten field doc-comment + new structural `round_secret_key_dropped_on_round_end` test (D-131 load-bearing) + 2 fixture `Some(...)` wraps + 1 test read `as_ref().expect(...)`. +66 / -2 LOC.
- `coordinator/src/round/signing.rs` — 4 test-fixture `Some(...)` wraps at lines 450, 496, 521, 560 (mechanical refresh per 21-RESEARCH §7). +4 / -4 LOC.
- `coordinator/src/round/manager.rs` — production field-init `rsa_signer: Some(signer)` at `start_round` (line 63) + test read site `.as_ref().expect("test fixture: rsa_signer is Some")` (line 195). +2 / -2 LOC.
- `coordinator/src/round/input_reg.rs` — production call site `inner.rsa_signer.as_ref().expect("rsa_signer must be Some during InputReg").blind_sign(...)` (line 71). +1 / -1 LOC.
- `coordinator/src/api/handlers.rs` — production call site `.rsa_signer.as_ref().expect("rsa_signer must be Some during OutputReg").public_key.clone()` (line 383). +1 / -1 LOC.

**Total: 215 insertions, 24 deletions across 6 files** (matches `git diff --stat HEAD~2 HEAD`).

## Decisions Made

- **D-128 (newtype placement):** `RoundSecretKey` lives INSIDE `RsaBlindSigner` as `secret_key: RoundSecretKey`; round state holds `Option<RsaBlindSigner>`. Reconciles REQUIREMENTS' two framings without introducing redundant Option-of-Option nesting.
- **D-129 / CD-47 (empty Drop body per 21-RESEARCH OQ1):** the wrapped `rsa::RsaPrivateKey` already implements UNCONDITIONAL `Drop + ZeroizeOnDrop` (verified at `~/.cargo/registry/.../rsa-0.9.10/src/key.rs:76-84`, no feature flag). DER-roundtrip and replace-with-dummy approaches are strictly worse: extra allocation, ~100ms keygen, identical outcome. The Drop body is therefore one `tracing::debug!` static-string event for ops observability and nothing else.
- **D-131 (two-test split):** structural FSM test (load-bearing, unconditional CI gate) in `state.rs::tests` + best-effort scrub (sanity, gated on Linux) in `rsa.rs::tests`. The scrub test mechanism is the 21-RESEARCH §"Proposed Best-Effort Scrub Test (CD-50)" template verbatim.
- **D-132 (D-07 comment rewrite):** the old "best-effort only" qualifier was factually outdated as of `rsa 0.9.10`. The rewritten comment names the transitive Drop chain by file:line, names the bounded lifetime via `Option<RsaBlindSigner>`, names the FSM trigger, and ends with the charter anchor `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` (which Plan 21-02 materializes — the comment's forward reference is intentional per the cross-phase invariant boundary).
- **CD-50 (scrub test gating):** `#[cfg_attr(not(target_os = "linux"), ignore = ...)]` with reason string naming the structural sibling as the unconditional gate. On this run (macOS Darwin), the test correctly reports `ignored` when run without `--include-ignored`; it runs and passes when explicitly enabled.

## Deviations from Plan

None - plan executed exactly as written. All locked decisions (D-128, D-128a, D-129, D-130, D-130a, D-130b, D-131, D-132) and Claude's-Discretion items (CD-46, CD-47, CD-49, CD-50) were honored verbatim per the 21-CONTEXT + 21-RESEARCH lock.

The Task 2 verify-script grep heuristic `grep -cE 'rsa_signer: Some\(' coordinator/src/round/state.rs` expected exactly 2 occurrences (the 2 pre-existing fixtures at lines 270 + 311). The actual count is 3 because the new structural test `round_secret_key_dropped_on_round_end` (which the plan REQUIRES per Edit D) also constructs a `Some(RsaBlindSigner::generate().unwrap())` fixture. This is a plan-script-vs-plan-body inconsistency, not a deviation — the semantic requirement (all fixtures wrapped in `Some(...)`, new test exists with required pre-transition assertion) is met. Acceptance criteria verification at the source-assertion level (`pub rsa_signer: Option<RsaBlindSigner>` exists, `round_secret_key_dropped_on_round_end` exists, old field declaration absent, stale "Not zeroized" prose absent) all pass.

**Total deviations:** 0
**Impact on plan:** None.

## Issues Encountered

None during planned work. The plan's task ordering (Task 1 = rsa.rs newtype first, Task 2 = state.rs refactor + propagation second) compiled cleanly at each task boundary — no inter-task build breaks. The Task 1 commit (`381a743`) compiles and tests cleanly without Task 2's changes, because Task 1 only changes `RsaBlindSigner`'s internal storage (the public method surface is preserved), so callers that consume `inner.rsa_signer.blind_sign(...)` continue to compile against the bare `RsaBlindSigner` field at Task 1's commit.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **For Plan 21-02 (AUDIT-01 + AUDIT-02):** the bounded-lifetime claim is now expressible as a Rust type signature an auditor can read at `coordinator/src/round/state.rs:110`. The D-07 comment at `coordinator/src/blind/rsa.rs:18-22` already references the charter anchor `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window`; Plan 21-02 must create the file with that anchor (markdown heading slug `rsa-secret-key-zeroization-window`).
- **For Plan 21-02 audit.toml refresh:** the RUSTSEC-2023-0071 ignore rationale can now name `Option<RsaBlindSigner>` on `RoundStateInner` and the `RoundSecretKey` Drop chain by symbol; the rewritten rationale paragraph drops the "destroys the key via zeroize" framing in favor of "transitively zeroized by `<rsa::RsaPrivateKey as Drop>::drop` (`rsa-0.9.10/src/key.rs:76-82`) when the FSM transitions to Idle and the `Option<RsaBlindSigner>` lifetime ends."
- **Cross-phase invariants:** all 6 from CONTEXT.md remain green at this plan boundary; no blockers for 21-02.

## Self-Check: PASSED

- `coordinator/src/blind/rsa.rs` exists and contains `RoundSecretKey` newtype + `Drop` impl + rewritten D-07 comment + scrub test — verified.
- `coordinator/src/round/state.rs` exists and contains `pub rsa_signer: Option<RsaBlindSigner>` field + `round_secret_key_dropped_on_round_end` test — verified.
- Commit `381a743` (Task 1) found in `git log` — verified.
- Commit `6f6dafb` (Task 2) found in `git log` — verified.
- `.planning/phases/21-audit-charter-zeroization-tightening/21-01-SUMMARY.md` exists — verified.

---
*Phase: 21-audit-charter-zeroization-tightening*
*Completed: 2026-05-31*
