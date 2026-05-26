---
workstream: fix-round-bootstrap
priority: P0
created: 2026-05-25
trigger: External code review identified that v1.1 ships a non-functional coordinator
blocks: [fix-ban-list-persistence, backlog-deferred-items]
---

# Context

## Why this exists
External code review of v1.1 (shipped 2026-04-10) found that the coordinator
binary can never run a real CoinJoin round. Every transition from
`RoundState::Idle` to `RoundState::InputReg` lives inside `#[cfg(test)]`
blocks. No production code path generates the RSA keypair, builds
`RoundStateInner`, or advances the round out of Idle.

This directly contradicts the project's headline claim: "zero to a working
CoinJoin round in under five minutes on Bitcoin signet." On a real
deployment, every API request hits `WRONG_PHASE` or "inner not initialized."

## Root cause locations
- `coordinator/src/main.rs:68` — only `RoundState::new_idle()` is called at startup
- `coordinator/src/round/manager.rs:120, 146` — Idle→InputReg transitions are inside `#[tokio::test]` blocks
- `coordinator/src/api/handlers.rs:224` — `post_input` checks `if guard.inner.is_none()` and errors; nothing populates inner
- Phase monitor in `main.rs:96-164` only arms `output_reg` and `signing` timers — never `input_reg` and never round creation

## Scope of fix
1. Promote the round-creation logic out of `#[cfg(test)]` in `round/manager.rs` into a production `start_round()` method on `RoundManager` that:
   - Generates the RSA signing keypair
   - Constructs `RoundStateInner` (denomination, fee_rate, deadlines from config)
   - Transitions Idle → InputReg
   - Broadcasts the phase change on the existing tokio broadcast channel
   - Arms the input_reg timeout via the phase monitor
2. Wire a startup task in `coordinator/src/main.rs` (after server boot at line ~68) that invokes `start_round()` — either immediately or on a configurable delay.
3. Extend the phase monitor (`main.rs:96-164`) to:
   - Arm the input_reg timer when entering InputReg
   - Re-invoke `start_round()` when a round terminates (success or abort) so the coordinator runs rounds continuously
4. Add an integration test (`tests/round_bootstrap.rs` or similar):
   - Start the coordinator binary in a tokio test
   - Hit `GET /round/info`
   - Assert phase is `InputReg` and `rsa_public_key` is non-null
5. Validate the Docker quickstart end-to-end ("zero to working round in 5 minutes"):
   - `docker compose up`
   - Run the client against signet
   - Observe a full round complete

## Forensics open question
v1.1's verification phase passed without catching this. Worth a `/gsd-audit-uat` pass to understand what the gap was — likely a missing E2E test in the validation milestone.

## Entry
Recommend `/gsd-debug` since this is a discovered regression in shipped code, not a fresh feature.

## Dependencies
- Blocks both `fix-ban-list-persistence` and `backlog-deferred-items` — the other workstreams need a runnable coordinator for regression testing.
