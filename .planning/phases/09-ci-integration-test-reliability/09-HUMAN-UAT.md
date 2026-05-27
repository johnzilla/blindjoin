---
status: complete
phase: 09-ci-integration-test-reliability
source: [09-VERIFICATION.md]
started: 2026-05-27T03:00:00Z
updated: 2026-05-27T13:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Fresh-PR CI log shows bitcoind-dependent integration test PASS verdict
expected: |
  Push a no-op PR to a branch off `main`. The `cargo test` job in
  `.github/workflows/ci.yml` runs `cargo test --workspace --all-targets`
  with `BLINDJOIN_REQUIRE_BITCOIND=1` and
  `BITCOIND_EXE=$HOME/.local/bin/bitcoind` exported. The CI log MUST contain
  a PASS line for at least one of: `rate_limiting::info_endpoint_returns_429_when_flooded`,
  `rate_limiting::request_timeout_returns_408`, or
  `round_bootstrap::run_bootstraps_round_into_input_reg` — and zero
  `bitcoind not found (...), skipping (local-dev mode; ...)` notices. The six
  `full_round.rs` carve-out tests should appear in the `ignored` column
  without executing.
result: pass
observed: |
  PR #7 CI run 26512029044, `cargo test` job 78079728275 (3m49s, exit 0).
  VALIDSIG 152812300785C96444D3334D17565732E08E5E41 (achow101) matched.
  9 passed; 0 failed; 6 ignored. 4 bitcoind-dependent tests PASS:
    rate_limiting::info_endpoint_returns_429_when_flooded ... ok
    rate_limiting::request_timeout_returns_408 ... ok
    round_bootstrap::run_bootstraps_round_into_input_reg ... ok
    full_round::adversarial_tampered_psbt_rejected ... ok
  All 6 Phase-10 carve-outs in `ignored` column with TODO(Phase-10) reason.
  Zero `bitcoind not found (...), skipping` lines.
  Required 2 fix commits along the way (ea16787, 6d10d05) to harden the
  PGP verify against multi-signer SHA256SUMS.asc — exactly the kind of
  finding the live-CI UAT was designed to surface.

### 2. Suite exits within bounded time when an individual test panics — no leaked bitcoind blocks the cargo pipe
expected: |
  Locally, force a panic in one bitcoind-using integration test (e.g., add
  `panic!("uat-2")` after `bootstrap_regtest_bitcoind` in
  `run_bootstraps_round_into_input_reg`). Run:
    BLINDJOIN_REQUIRE_BITCOIND=1 \
      BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
      cargo test --test integration 2>&1 | tee target/integration-test.log
  The suite process MUST exit within ~10 seconds of the panic — not hang
  waiting for bitcoind shutdown. `target/integration-test.log` should
  contain the `panicked at` line.
result: pass
observed: |
  Injected `panic!("UAT-2")` after `_bitcoind_guard` binding in
  tests/integration/round_bootstrap.rs:67. Ran:
    BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
      cargo test --test integration round_bootstrap::run_bootstraps_round_into_input_reg \
      -- --nocapture 2>&1 | tee target/integration-test-uat2.log
  Result: 8s total wallclock (includes 4.77s compile). cargo exited cleanly
  with `test result: FAILED. 0 passed; 1 failed`. Log contains exact line:
    thread '...' panicked at .../round_bootstrap.rs:70:5: UAT-2: synthetic panic ...
  NO hang. NO orphan bitcoind survived past cargo exit (separately confirmed in test 3).
  Panic reverted after test; round_bootstrap.rs is back to clean state.
  Log preserved at target/integration-test-uat2.log.

### 3. No orphan bitcoind processes after the suite completes
expected: |
  On macOS or Linux, run `ps aux | grep '[b]itcoind'` BEFORE and AFTER
  `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration`.
  The two listings should match exactly — no bitcoind PID present after the
  suite that wasn't present before, even when individual tests panic. If a
  PID lingers, `kill -9 <pid>` and investigate which test failed to drop its
  BitcoindGuard.
result: pass
observed: |
  Local check on macOS arm64 against brew bitcoind v31.0.0.
  Full integration suite ran: 9 passed; 0 failed; 6 ignored; in 7.51s.
  BEFORE (pgrep -lf '/bitcoind\b'): (none).
  AFTER (with 5s settling, pgrep -lf '/bitcoind\b'): (none).
  ps -axo pid,comm | awk '$2 ~ /bitcoind/': empty.
  Result: zero orphan bitcoind processes. All 4 spawned daemons (one per
  non-ignored bitcoind test) terminated cleanly via BitcoindGuard::drop
  (CR-01 fix routes node.stop() through spawn_blocking so the kill happens
  off the tokio runtime thread).

## Summary

total: 3
passed: 3
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps
