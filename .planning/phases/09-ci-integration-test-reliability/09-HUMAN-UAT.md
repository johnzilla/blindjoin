---
status: partial
phase: 09-ci-integration-test-reliability
source: [09-VERIFICATION.md]
started: 2026-05-27T03:00:00Z
updated: 2026-05-27T03:00:00Z
---

## Current Test

[awaiting human testing]

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
result: [pending]

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
result: [pending]

### 3. No orphan bitcoind processes after the suite completes
expected: |
  On macOS or Linux, run `ps aux | grep '[b]itcoind'` BEFORE and AFTER
  `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration`.
  The two listings should match exactly — no bitcoind PID present after the
  suite that wasn't present before, even when individual tests panic. If a
  PID lingers, `kill -9 <pid>` and investigate which test failed to drop its
  BitcoindGuard.
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
