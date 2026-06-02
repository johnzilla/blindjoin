---
workstream: fix-round-bootstrap
slug: fix-round-bootstrap
created: 2026-05-25
status: root_cause_found_awaiting_approval
trigger: External code reviewer claims v1.1 ships a non-functional coordinator — no production code path transitions RoundState out of Idle
goal: find_root_cause_only (paused before fix per user request)
tdd_mode: false
---

# Debug Session: fix-round-bootstrap

## Current Focus
**Hypothesis (CONFIRMED):** The v1.1 coordinator binary contains no production code path that constructs `RoundStateInner` or transitions `Phase::Idle → Phase::InputReg`. Every such transition lives inside `#[cfg(test)]` or in `tests/integration/full_round.rs`. The integration tests bypass the gap by hand-rolling a pre-initialized round state and handing it to the router — they never exercise `main.rs`'s startup path.

**Next action:** Await user approval on the proposed fix shape (see `root-cause-report.md`), then continue this session in `find_and_fix` mode.

## Symptoms (as reported)
- Any client POST to `/round/register_input` against a running coordinator returns `WRONG_PHASE "Expected input_reg, got idle"` or `WRONG_PHASE "Round inner state not initialized"` forever.
- No round ever starts.
- Headline project claim ("zero to a working CoinJoin round in under five minutes on Bitcoin signet") cannot be true on a real deployment.

## Evidence

- timestamp: 2026-05-25 — `main.rs:68` only calls `RoundState::new_idle()`; nothing else mutates round state at startup. Confirmed by reading `coordinator/src/main.rs` start-to-finish.
- timestamp: 2026-05-25 — Phase monitor (`main.rs:96-164`) arms only `OutputReg` and `Signing` timeout timers. No arm for `InputReg`, no round-creation logic, no re-armer on round completion.
- timestamp: 2026-05-25 — `coordinator/src/round/state.rs:80-83` contains a load-bearing self-incriminating comment:
  > "ARCH NOTE: RoundStateInner is currently only constructed in tests (full_round.rs, signing.rs). **Production round-start logic (future phase) MUST also populate this field at that time.** See phase 07 CONTEXT.md for details."
- timestamp: 2026-05-25 — `grep -rn "RoundStateInner" --include='*.rs'` finds construction sites only in:
  - `coordinator/src/round/state.rs:233` (test)
  - `coordinator/src/round/input_reg.rs:122` (test helper `make_input_reg_state`)
  - `coordinator/src/round/signing.rs:283, 398, 441, 466, 501` (tests)
  - `tests/integration/full_round.rs:39` (test helper `build_input_reg_round_state`)
- timestamp: 2026-05-25 — `grep -rn "transition_to(Phase::InputReg)"` returns 4 matches, all inside `#[cfg(test)]` or `tests/integration/`.
- timestamp: 2026-05-25 — `grep -rn "fn start_round|fn create_round|fn begin_round|fn new_round|fn bootstrap"` returns **zero** matches anywhere in the crate. There is no production round-creation function at any name.
- timestamp: 2026-05-25 — `coordinator/src/api/handlers.rs:224-231` guards every `post_input` call with `if guard.inner.is_none() { return WRONG_PHASE ... }`. With production never populating `inner`, the guard fires unconditionally.
- timestamp: 2026-05-25 — Phase 7.1 plan (`.planning/milestones/v1.1-phases/07-coordinator-dos-hardening/07-01-PLAN.md:152, 189`) explicitly notes "Production round-start logic (to be added in a future phase) MUST also populate this." That future phase was never added — `ROADMAP.md` shows only phases 1-7 and they are all complete.
- timestamp: 2026-05-25 — Phase 1.2 (`.planning/milestones/v1.0-phases/01-core-protocol/01-02-PLAN.md`) scoped the FSM enum, `RoundState`, `RoundStateInner`, `run_phase_timer`, and `generate_session_token` — but never assigned "who calls `transition_to(Phase::InputReg)` in production." This is the original scoping gap.
- timestamp: 2026-05-25 — **Verification gap explained.** `tests/integration/full_round.rs:9-11` claims compliance with T-06-02 (no test-only backdoors), but lines 97-98, 487, 853, and 885 all bypass the missing production round-start path:
  ```rust
  let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
  ```
  The integration tests never call `main.rs::main()`. They build the router directly via `api::build_router(...)` with a pre-initialized state, so the missing startup wiring is invisible to test coverage.
- timestamp: 2026-05-25 — `.planning/milestones/v1.1-phases/07-coordinator-dos-hardening/07-VERIFICATION.md:95` line-items the test helper as "Test-only setup code simulating round initialization; not in production hot path. **Acceptable.**" The verifier saw the test helper, accepted it, and did not flag that *production never simulates round initialization at all*. This is the verification gap.

## Root Cause

See `root-cause-report.md` in this directory for full analysis, validated fix shape, and forensics for the audit-uat pass.

## Resolution

**Status:** Root cause confirmed; fix not yet applied. Awaiting user approval on fix shape before code changes land.

- **Root cause:** Coordinator startup never constructs `RoundStateInner` or transitions the round out of `Phase::Idle`. The production round-creation path was scoped for "a future phase" (per Phase 7.1 plan and state.rs:80-83 ARCH NOTE) but never planned or implemented. The phase monitor in `main.rs` arms only OutputReg and Signing timers — it never arms InputReg and never creates a round. Integration tests bypass the gap by hand-rolling a pre-initialized `RoundStateInner` and handing it directly to `api::build_router(...)`, which is why v1.1's verification phase passed without detecting that the shipped binary cannot run a round.
- **Fix:** Not yet applied.

## Specialist Review

Skipped — Rust + tokio + axum stack has no matching specialist skill in the configured set. The fix shape was validated by direct code reading of `RoundState`, `RoundStateInner`, `RsaBlindSigner`, `CoordinatorConfig`, and the existing phase-monitor pattern in `main.rs:96-164`.
