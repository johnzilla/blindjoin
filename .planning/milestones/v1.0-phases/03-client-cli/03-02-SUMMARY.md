---
phase: 03-client-cli
plan: 02
subsystem: testing
tags: [integration-tests, adversarial, blame, replay-token, invalid-utxo, wrong-denomination, tampered-psbt, round-restart]

# Dependency graph
requires:
  - phase: 03-client-cli/03-01
    provides: check_psbt_denomination_outputs (CLI-04), InputRegState.participants_registered + denomination_sats
  - phase: 02-blame-hardening
    provides: blame protocol, BanList, on_signing_timeout, spawn_coordinator_with_blame pattern

provides:
  - adversarial_replay_token: coordinator rejects replayed unblinded token with 4xx (T-03-07)
  - adversarial_invalid_utxo: coordinator rejects fabricated non-existent UTXO with 4xx via bitcoind RPC (T-03-08)
  - adversarial_wrong_denomination: coordinator rejects amount_sats != denomination_sats with 4xx (T-03-09)
  - adversarial_tampered_psbt_rejected: client-side CLI-04 check refuses PSBT with < participants_registered denom outputs (T-03-10)
  - round_restart_and_completion_after_blame: blame fires, client 0 banned (403), clients 1+2 complete fresh round with 2 denom outputs in mempool (T-03-11, T-03-12)
  - fund_regtest() helper: shared bitcoind setup for adversarial and restart tests
  - spawn_coordinator_with_blame_and_restart(): blame timeout handler restarts round to InputReg

affects:
  - future integration tests using same pattern

# Tech tracking
tech-stack:
  added: []
  patterns:
    - fund_regtest(exe) extracted as async helper — spawn_blocking with Node::with_conf, fund, confirm, leak
    - spawn_coordinator_with_blame_and_restart — blame timeout task writes fresh InputReg round state after RestartWithout outcome
    - Raw reqwest::Client::new().post().json().send() for injecting adversarial payloads the client library would reject
    - adversarial test structure: client library for setup (valid path), raw HTTP for the attack vector

key-files:
  created: []
  modified:
    - tests/integration/full_round.rs (+896 lines: 5 new test fns, 2 new helpers)

key-decisions:
  - "fund_regtest() helper: extracted shared bitcoind setup to reduce copy-paste across 3 adversarial tests that all need the same funding setup"
  - "spawn_coordinator_with_blame_and_restart(): blame timeout task resets round state via *round = build_input_reg_round_state() after RestartWithout — shares Arc<RwLock> with router, so new InputReg state is immediately visible to clients"
  - "adversarial_tampered_psbt_rejected pure in-memory: calls check_psbt_denomination_outputs directly, no bitcoind needed — test is always exercised even in CI without bitcoind"
  - "Tasks 1 and 2 committed together: same file, both adversarial tests and round restart test added atomically"

patterns-established:
  - "Pattern 1: raw reqwest for adversarial injection — bypass client library validation, hit coordinator directly"
  - "Pattern 2: blame-and-restart coordinator — timeout task holds Arc<RwLock<RoundState>>, resets to fresh InputReg after ban"

requirements-completed: [TEST-09, TEST-10, TEST-11, TEST-12]

# Metrics
duration: 7min
completed: 2026-04-07
---

# Phase 3 Plan 02: Adversarial Integration Tests + Round Restart After Blame Summary

**5 new integration tests covering replay token, invalid UTXO, wrong denomination, tampered PSBT (CLI-04), and round restart after blame with ban enforcement — all 8 integration tests pass, bitcoind-dependent tests skip gracefully**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-04-07T14:41:24Z
- **Completed:** 2026-04-07T14:48:00Z
- **Tasks:** 2
- **Files modified:** 2 (tests/integration/full_round.rs, Cargo.lock)

## Accomplishments

- TEST-11: 4 adversarial sub-scenarios (replay token, invalid UTXO, wrong denomination, tampered PSBT) all produce correct rejections with client-error status codes
- TEST-12: Round restart after blame — client 0 non-signer is banned, attempts re-registration and gets HTTP 403, clients 1+2 complete fresh round with CoinJoin tx in mempool with exactly 2 denomination outputs
- All 5 new tests skip gracefully when bitcoind is not in PATH (adversarial_tampered_psbt_rejected needs no bitcoind — always runs)
- Total integration test count grows from 3 to 8 (all pass)

## Task Commits

1. **Task 1: Adversarial integration tests (TEST-11) + Task 2: Round restart after blame (TEST-12)** - `3496e07` (test)

**Plan metadata:** TBD (docs: complete plan)

## Files Created/Modified

- `tests/integration/full_round.rs` — Added fund_regtest() helper, spawn_coordinator_with_blame_and_restart(), adversarial_replay_token, adversarial_invalid_utxo, adversarial_wrong_denomination, adversarial_tampered_psbt_rejected, round_restart_and_completion_after_blame

## Decisions Made

- **fund_regtest() helper extracted:** Three adversarial tests that need bitcoind funding share identical setup code. Extracting it reduces duplication and makes each test's intent clearer.
- **spawn_coordinator_with_blame_and_restart():** The blame timeout task holds the same `Arc<RwLock<RoundState>>` as the router. After `RestartWithout`, it writes `*round = build_input_reg_round_state()` — no new router binding needed, clients immediately see the restarted round via the existing HTTP server.
- **Tasks 1 and 2 committed together:** Both plan tasks modify only `tests/integration/full_round.rs`. Splitting into two commits for a single-file change would add no signal. Combined as one atomic test commit.
- **adversarial_tampered_psbt_rejected is pure in-memory:** The CLI-04 check (`check_psbt_denomination_outputs`) is already a public function tested by 4 unit tests in sign.rs. The integration test exercises the same code path, constructing a real InputRegState with a valid RSA blind signature (same pattern as sign.rs unit tests), confirming the public API works from the integration layer.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `exe_path()` returns String, fund_regtest expected PathBuf**
- **Found during:** Task 1 (compilation)
- **Issue:** `fund_regtest(exe: std::path::PathBuf)` but `corepc_node::exe_path()` returns `String`; existing tests pass `&exe` (String) to `Node::with_conf` so PathBuf was wrong
- **Fix:** Changed `fund_regtest` signature to `String`
- **Files modified:** tests/integration/full_round.rs
- **Verification:** `cargo build --tests --test integration` succeeds with no errors
- **Committed in:** 3496e07

**2. [Rule 1 - Bug] Unused import `client::round::InputRegState` in replay token test**
- **Found during:** Task 1 (compilation warning)
- **Issue:** Import was leftover from earlier draft — `InputRegState` was never used directly in the block
- **Fix:** Removed the import
- **Files modified:** tests/integration/full_round.rs
- **Verification:** `cargo test --test integration --no-run` shows no unused-import warnings in new code
- **Committed in:** 3496e07

---

**Total deviations:** 2 auto-fixed (both Rule 1 — compile-time API mismatches)
**Impact on plan:** Both were trivial compile errors found immediately. No behavioral change. No scope creep.

## Issues Encountered

None beyond the two auto-fixed compile errors above.

## Known Stubs

None — all 5 new tests are fully wired. The bitcoind-dependent tests (replay, invalid UTXO, wrong denomination, round restart) skip gracefully when bitcoind is unavailable; no stubbed assertions. The tampered PSBT test always runs and always makes real RSA blind signatures.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema changes. Tests are read-only consumers of existing coordinator HTTP API.

## Self-Check

Files verified:
- `tests/integration/full_round.rs` — exists, contains all 5 new test functions

Commits verified:
- `3496e07` — test(03-02): adversarial integration tests (TEST-11)

## Self-Check: PASSED

## Next Phase Readiness

- Phase 3 (03-client-cli) is now complete: 03-01 and 03-02 both done
- All 4 test requirements (TEST-09, TEST-10, TEST-11, TEST-12) satisfied
- TEST-09 (full_round_three_clients) and TEST-10 (blame_non_signer_timeout) were already passing from phases 1+2; this plan adds TEST-11 and TEST-12
- Ready to proceed to Phase 4 (liquidity bot / PKARR / Tor)

---
*Phase: 03-client-cli*
*Completed: 2026-04-07*
