---
workstream: fix-round-bootstrap
created: 2026-05-25
status: awaiting_user_approval
---

# Root Cause Report: v1.1 Coordinator Cannot Run a CoinJoin Round

## TL;DR

The external reviewer's claim is **fully confirmed, with no surprises**. The v1.1 coordinator binary genuinely cannot run a CoinJoin round in production. The bug is "production round-creation logic was deferred to a phase that never got planned" — not a wiring bug, not a feature flag, not a config-gated task. The integration test suite hides the gap by hand-building a pre-initialized round state and giving it directly to `api::build_router(...)`, never exercising `main.rs::main()`. v1.1 verification looked at the test helper, called it "acceptable," and did not notice that production has no analog.

This is **not** a structural problem with the round state machine. The FSM is sound, `RoundStateInner` is well-designed, the phase monitor pattern is reusable. The fix is additive: add the missing production builder + bootstrap task + InputReg timer arm + on-completion re-armer. Existing code does not need to be reworked.

## Root cause (single sentence)

`main.rs` calls `RoundState::new_idle()` at startup and then never advances the FSM out of `Phase::Idle`, because no production function exists that constructs `RoundStateInner` (RSA keypair, round secret, fresh hash maps) and calls `transition_to(Phase::InputReg)` — every such code path lives inside `#[cfg(test)]` or in `tests/integration/`.

## Confirmed evidence

| Claim | Evidence |
|---|---|
| `main.rs` startup never advances out of Idle | `coordinator/src/main.rs:68` is the only line that touches the initial state; nothing else writes the round state until handlers do. Phase monitor at `:96-164` matches only `Phase::OutputReg` and `Phase::Signing`. |
| Idle→InputReg transitions are test-only | `grep -rn "transition_to(Phase::InputReg)"` returns 4 hits: `coordinator/src/round/manager.rs:120, 146` (both inside `#[tokio::test]` blocks), `coordinator/src/round/input_reg.rs:132` (inside `#[cfg(test)] mod tests`), `tests/integration/full_round.rs:50` (inside the integration test helper `build_input_reg_round_state`). Zero hits in production code paths. |
| No production round-creation function of any name | `grep -rn "fn start_round\|fn create_round\|fn begin_round\|fn new_round\|fn bootstrap"` returns **zero matches** anywhere in the crate. |
| RoundStateInner is only constructed in tests | `grep -rn "RoundStateInner"` shows 9 construction sites; all are inside `#[cfg(test)]` blocks or in `tests/integration/full_round.rs`. |
| Handler explicitly fails when inner is missing | `coordinator/src/api/handlers.rs:224-231` — `if guard.inner.is_none() { return WRONG_PHASE "Round inner state not initialized" }`. With production never populating `inner`, this fires for every request. |
| The omission was acknowledged in code | `coordinator/src/round/state.rs:80-83` ARCH NOTE: "RoundStateInner is currently only constructed in tests. Production round-start logic (future phase) MUST also populate this field at that time." That future phase was never added. |
| The omission was acknowledged in planning | `.planning/milestones/v1.1-phases/07-coordinator-dos-hardening/07-01-PLAN.md:152, 189` repeats the same comment verbatim and explicitly defers it. No subsequent phase was ever scoped to take the deferral. |

## Forensics: why v1.1 verification missed this

Three concrete failure points in the verification process:

1. **Integration tests bypass the production startup path.** `tests/integration/full_round.rs` builds the router with a hand-crafted, pre-initialized `RoundStateInner`:

   ```rust
   // tests/integration/full_round.rs:97-99
   // Start coordinator in InputReg so clients can register immediately
   let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
   let app = coordinator::api::build_router(round_state, rpc, cfg);
   ```

   The test never calls `coordinator::main()` or anything equivalent. Every test that "exercises a full round" begins with a router that has already had production's missing work done for it by `build_input_reg_round_state()` (lines 20-53). The test then claims the coordinator works.

2. **Threat-model invariant T-06-02 was self-violated.** The same test file (lines 9-11) declares:

   > "T-06-02: Integration tests use the same HTTP API as real clients — no test-only backdoors in coordinator code paths."

   But `build_input_reg_round_state()` is itself the backdoor. It pokes a fully-populated `RoundStateInner` into the FSM from outside the API surface — exactly the scenario T-06-02 was supposed to prevent. The test invariant was lampshaded in the doc comment and contradicted ten lines later.

3. **Verification accepted the test-only helper without checking for a production analog.** `.planning/milestones/v1.1-phases/07-coordinator-dos-hardening/07-VERIFICATION.md:95`:

   > "Test-only setup code simulating round initialization; not in production hot path. **Acceptable.**"

   The verifier saw `from_der_secret_key` being called inside a test helper, correctly noted it was outside the production hot path (which was about AVAIL-02's per-request signer caching), and stopped there. There was no follow-up check of the form "what does the production hot path do instead of this?" — which would have surfaced the gap immediately, because the answer is "nothing."

The blame `RestartWithout` test (`spawn_coordinator_with_blame_and_restart`, lines 814-903) compounds the problem: when the test wants to simulate a round restarting after blame, it does `*round = build_input_reg_round_state();` (line 885). Production has no equivalent of that either — even if a first round somehow got created, it could never restart.

## Proposed fix shape (validated against actual code)

The reviewer's sketch in `CONTEXT.md` is essentially correct. After reading `RoundState`, `RoundStateInner`, `RsaBlindSigner`, `CoordinatorConfig`, and the existing phase-monitor in `main.rs:96-164`, the fix is straightforward and additive:

### 1. New production constructor in `coordinator/src/round/manager.rs`

```rust
pub fn start_round(state: &mut RoundState) -> Result<(), anyhow::Error> {
    // Generate fresh RSA signing keypair for this round
    let signer = RsaBlindSigner::generate()?;
    let sk_der = signer.secret_key_der()?;
    let pk_der = signer.public_key_spki_der()?;
    let pk_hash = signer.public_key_hash();

    // Fresh round secret for session-token HMAC
    let mut round_secret = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut round_secret);

    state.rsa_pubkey_der = Some(pk_der);
    state.rsa_pubkey_hash = Some(pk_hash);
    state.inner = Some(RoundStateInner {
        rsa_signing_key: sk_der,
        rsa_signer: signer,
        round_secret,
        registered_inputs: Default::default(),
        redeemed_tokens: Default::default(),
        registered_outputs: Vec::new(),
        partial_sigs: Default::default(),
        change_addresses: Default::default(),
    });
    state.transition_to(Phase::InputReg)
        .map_err(|e| anyhow::anyhow!("invalid FSM transition: {e}"))?;
    Ok(())
}
```

**Notes from cross-checking the existing API surface:**
- All struct fields are public — no constructor changes needed.
- `RsaBlindSigner::generate()`, `secret_key_der()`, `public_key_spki_der()`, `public_key_hash()` already exist and are exercised in the test helpers — copy-paste shape is safe.
- `transition_to` already enforces FSM legality (Idle→InputReg is allowed). Caller MUST verify `state.phase == Phase::Idle` before calling, or accept the `TransitionError`.
- Configuration fields are read at the caller, not inside `start_round` — keeps the function easy to test.
- The reviewer's sketch mentioned "fee_rate, deadlines from config" — those don't actually go into `RoundStateInner`; the FSM doesn't store deadlines (timers are external). `denomination_sats` and `fee_rate_sat_per_vbyte` are already on `CoordinatorConfig` and accessed via `state.config` in handlers. So the fix does **not** need to plumb config into `RoundStateInner`.

### 2. Bootstrap call in `coordinator/src/main.rs`

Immediately after the `RoundState::new_idle()` call at line 68, but before the phase monitor is spawned, run the first round-start:

```rust
{
    let mut guard = round_state.write().await;
    if let Err(e) = round::manager::start_round(&mut guard) {
        error!(error = %e, "Initial round start failed");
        std::process::exit(1);
    }
    info!(round_id = %guard.round_id, "Initial round started in input_reg");
}
```

### 3. Extend the phase monitor at `main.rs:96-164` to handle two more cases

- **Arm an InputReg timeout timer** when entering `Phase::InputReg` — uses `cfg.coordinator.round_timeout_input_reg_secs`. On timeout, evaluate quorum: if `participant_count >= min_participants`, advance to `OutputReg`; otherwise abort back to `Idle` and start a new round. There is already a helper-shaped function (`round::input_reg`) that can host the `on_input_reg_timeout` logic, mirroring `round::output_reg::on_output_reg_timeout`.
- **Re-arm `start_round`** when the FSM returns to `Phase::Idle` (after Broadcast→Idle or Blame→Idle). The 500ms poll loop already in place can detect the transition via `last_idle_round != round_id` and call `start_round(&mut guard)` again.

### 4. New integration test exercising the production startup path

Add `tests/integration/round_bootstrap.rs` that:
- Spawns the coordinator using a thin wrapper around `main()` (or factor `main`'s body into a `pub async fn run(cfg) -> Result<()>` and call that from the test — this is also what makes the "no test-only backdoors" claim honest).
- Hits `GET /round/info` and asserts `phase == "input_reg"` and `rsa_public_key` is non-null.
- This single test would have caught the bug pre-ship.

### 5. Optional, but recommended: delete or guard `build_input_reg_round_state`

Once `start_round()` exists, `tests/integration/full_round.rs:20-53` (`build_input_reg_round_state`) should be replaced by a call to the production builder. Same for the inline `*round = build_input_reg_round_state();` "restart" in `spawn_coordinator_with_blame_and_restart`. Otherwise the tests will continue to pass even if `start_round` regresses.

### 6. Docker quickstart validation

`docker compose up`, run the client against signet, observe a full round complete. This is the "zero to working round in five minutes" headline claim — without this validation, no fix is shippable.

## Is this more than a "bootstrap is missing" bug?

**No.** I looked specifically for structural issues that would change the fix shape:

- The FSM transitions are correct and well-tested in `state.rs:192-251`.
- `Drop for RoundStateInner` correctly zeroizes sensitive material on round end.
- `transition_to(Phase::Idle)` correctly resets `rsa_pubkey_hash`, `rsa_pubkey_der`, `participant_count`, and assigns a fresh `round_id`. So once `start_round()` exists, "continuous rounds" is just "call `start_round` again when phase observes Idle."
- The blame/restart pathway (`coordinator/src/round/blame.rs::on_signing_timeout`) already handles the FullAbort/RestartWithout decision — fix only needs to add round re-start, not re-design blame.
- `handlers.rs` and `input_reg.rs`/`output_reg.rs`/`signing.rs` all read from `RoundStateInner` correctly; they were always waiting for someone to populate it.

The fix is additive (~80 LOC of new code + ~40 LOC of test) and does not require reworking the state machine.

## What needs user approval before code changes

1. **Bootstrap timing**: start the first round immediately at process boot (recommended), or on a configurable delay?
2. **InputReg timeout abort behaviour**: if `participant_count < min_participants` at timeout, abort back to Idle and immediately start a new round — confirm this matches the desired UX. Alternative: stay in Idle, only restart on client poll. The first option matches the "always-on coordinator" framing in CONTEXT.md.
3. **Test refactor scope**: are we OK refactoring `main.rs` to expose `pub async fn run(cfg: CoordinatorConfig) -> Result<()>` so the new integration test can spawn the real startup path without copy-pasting? Strongly recommended; alternative is a separate `coordinator-lib` re-shape (out of scope for this fix).
4. **Should the audit-uat pass be a separate workstream** (e.g. `fix-verification-gap` covering the T-06-02 self-violation and the verification doc's "Acceptable" judgment), or folded into this fix's SUMMARY?

## Pointers for the audit-uat pass

Three concrete deliverables for the verification-gap audit:

1. **Test-only backdoor inventory.** Audit every integration test under `tests/integration/` for code that mutates coordinator state outside the HTTP API. `build_input_reg_round_state`, the inline timeout task in `spawn_coordinator_with_blame`, and the `*round = build_input_reg_round_state();` restart in `spawn_coordinator_with_blame_and_restart` are the known offenders. There may be more.
2. **Verification-doc heuristic.** Add a rule to the verification template: "When marking test-only setup code as 'Acceptable,' you MUST also identify and cite the production code path that performs the equivalent setup. If there isn't one, that's a verification failure." This would have caught 07-VERIFICATION.md:95 at v1.1 review time.
3. **Smoke test in CI.** A 5-line CI step that runs the coordinator binary, polls `GET /round/info`, and asserts `phase != "idle"` within 10 seconds. Cheaper than any unit test; would have caught this regression instantly.
