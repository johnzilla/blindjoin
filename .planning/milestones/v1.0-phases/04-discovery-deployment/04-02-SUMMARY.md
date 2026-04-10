---
phase: 04-discovery-deployment
plan: 02
subsystem: liquidity-bot
tags: [liquidity-bot, signet, polling-loop, join-strategy, tdd]
dependency_graph:
  requires: [04-01]
  provides: [DEPL-02]
  affects: [docker-compose-stack]
tech_stack:
  added: [liquidity-bot workspace member]
  patterns: [env-var config, signet safety guard, UTXO-per-run exit pattern]
key_files:
  created:
    - liquidity-bot/Cargo.toml
    - liquidity-bot/src/main.rs
    - liquidity-bot/src/strategy.rs
  modified:
    - Cargo.toml
decisions:
  - Liquidity bot exits after one successful round (UTXO spent); Docker restart policy handles re-run — simpler than UTXO rotation logic in-process
  - No clap dependency — all config from env vars for Docker-native ergonomics
  - JoinStrategy::should_join() written test-first (TDD) — 5 unit tests covering all decision branches
duration_secs: ~150
completed_date: "2026-04-09"
---

# Phase 4 Plan 2: Liquidity Bot Summary

## One-liner

Signet liquidity bot polling GET /info every 5s and auto-joining CoinJoin rounds via the full client round participation flow, with hard bail! safety guard against non-signet networks.

## What Was Built

### liquidity-bot/ workspace member

A new Cargo workspace member (`liquidity-bot`) providing a standalone binary that fills the CoinJoin anonymity set on signet automatically. Without this bot, a fresh Docker Compose stack has no participants and rounds never start.

**liquidity-bot/src/strategy.rs** — `JoinStrategy` with `should_join()`:
- Returns true only when: `round_state == "input_reg"` AND `denomination_sats` matches configured target AND `participants_registered < join_threshold`
- All three rejection cases have dedicated unit tests (wrong phase, wrong denomination, threshold exceeded)

**liquidity-bot/src/main.rs** — main polling loop:
- Reads all config from environment variables (BLINDJOIN_COORDINATOR_URL, BLINDJOIN_NETWORK, BLINDJOIN_UTXO, BLINDJOIN_UTXO_VALUE_SATS, BLINDJOIN_UTXO_WIF, plus optional BLINDJOIN_TARGET_DENOMINATION_SATS and BLINDJOIN_JOIN_THRESHOLD)
- Hard `bail!` if BLINDJOIN_NETWORK is not "signet" — T-04-08 mitigation against mainnet fund loss
- Polls GET /info every 5s; only attempts round participation when `should_join()` returns true
- Calls full three-phase client round flow via client library: `register_input` → `register_output` → `verify_and_sign`
- Exits after one successful round (UTXO is spent; operator updates env vars and Docker restart policy re-launches)
- After `max_consecutive_failures` (default 5), sleeps 300s before retrying — T-04-09 mitigation

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | JoinStrategy module with unit tests (TDD) | 4669c19 | Cargo.toml, liquidity-bot/Cargo.toml, liquidity-bot/src/strategy.rs, liquidity-bot/src/main.rs (stub) |
| 2 | Main polling loop with signet safety guard | 69fd713 | liquidity-bot/src/main.rs |

## Verification Results

- `cargo test -p liquidity-bot -- strategy`: 5/5 tests pass
- `cargo build -p liquidity-bot`: compiles clean (1 harmless unused_assignments warning on consecutive_failures = 0 before return)
- `cargo build --workspace`: no errors (coordinator integration test failures are pre-existing from plan 04-01, out of scope)
- Binary exists: `target/debug/liquidity-bot` (190MB debug build)
- Signet safety guard present in source: `"Liquidity bot refuses to start: BLINDJOIN_NETWORK='{}' is not 'signet'"`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] InfoResponse has no Default derive — explicit field construction in tests**
- Found during: Task 1
- Issue: Plan template used `..Default::default()` in test helper, but `InfoResponse` in `shared/src/protocol.rs` does not derive `Default`. Also, `InfoResponse` has `max_participants` and `rsa_pubkey_hash`/`rsa_pubkey_der_b64` fields not shown in the plan's interface docs.
- Fix: Constructed all `InfoResponse` fields explicitly in `make_info()` test helper including `max_participants: 10`, `rsa_pubkey_hash: None`, `rsa_pubkey_der_b64: None`.
- Files modified: liquidity-bot/src/strategy.rs

**2. [Rule 1 - Bug] round participation function signatures differ from plan**
- Found during: Task 2
- Issue: Plan template used `reg: &InputRegResult` but actual return type is `InputRegState` (from `client::round::mod.rs`). The `register_output` and `verify_and_sign` signatures take `state: &InputRegState` not `&InputRegResult`.
- Fix: Used `state` variable name and `InputRegState` type throughout `participate_in_round()`.
- Files modified: liquidity-bot/src/main.rs

## Known Stubs

None — the bot implements the full round participation flow using the production client library.

## Threat Flags

No new threat surface introduced beyond what was documented in the plan's threat model (T-04-07 through T-04-10).

## Self-Check: PASSED
