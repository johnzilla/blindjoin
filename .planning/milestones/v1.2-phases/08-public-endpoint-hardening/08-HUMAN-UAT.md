---
status: partial
phase: 08-public-endpoint-hardening
source: [08-VERIFICATION.md]
started: 2026-05-26T00:00:00Z
updated: 2026-05-26T20:30:00Z
---

## Current Test

Items 1 and 2 PASSED on local bitcoind v31.0.0. Item 3 remains deferred per Plan 04 A4.

## Tests

### 1. Runtime proof of HTTP 429 + Retry-After + RATE_LIMITED JSON envelope on /info under flood
expected: With bitcoind in PATH, `cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture --include-ignored` runs end-to-end and emits `info_endpoint_returns_429_when_flooded PASSED: 429 + retry-after + JSON envelope (code=RATE_LIMITED) observed`.
result: passed
evidence: Ran locally 2026-05-26 against `bitcoind v31.0.0` (Homebrew). Test output: `info_endpoint_returns_429_when_flooded PASSED: 429 + retry-after + JSON envelope (code=RATE_LIMITED) observed; Plan 02 D-02/D-03/A5 runtime proof complete.`. Test suite line: `test result: ok. 2 passed; 0 failed; ... finished in 7.23s`. Required two unrelated fixes to reach this state: production code `coordinator/src/bitcoin/rpc.rs` bumped from JSON-RPC 1.1 to 2.0 (Bitcoin Core 31 rejects 1.1), and test dep `corepc-node` bumped 0.10 → 0.12 with feature `30_2` (the harness defaults to a Bitcoin Core 0.17.2 RPC schema from 2018).

### 2. Runtime proof of HTTP 408 REQUEST_TIMEOUT on slow-body request
expected: With bitcoind in PATH, `cargo test --test integration rate_limiting::request_timeout_returns_408 -- --nocapture --include-ignored` runs end-to-end and emits `request_timeout_returns_408 PASSED: HTTP 408 REQUEST_TIMEOUT observed within 5s of a request that paused mid-body for 3s against request_timeout_secs=1`. WR-02 fix means the test also asserts time-to-first-byte < 1750 ms.
result: passed
evidence: Same run as item 1. Test output: `request_timeout_returns_408 PASSED: HTTP 408 REQUEST_TIMEOUT observed within 5s of a request that paused mid-body for 3s against request_timeout_secs=1; Plan 02 D-04 runtime proof complete.`. The WR-02 time-to-first-byte assertion is included in the test body and was exercised by this run.

### 3. Tor connection-cap runtime behavior (N+1 streams park beyond max_concurrent_connections)
expected: An attacker opening 257 simultaneous .onion streams sees only 256 served; the 257th parks until an earlier connection finishes. Plan 04 explicitly defers this assertion to a future-phase Tor-mode harness (`TODO(Phase-8 Q3, A4)` in `tests/integration/rate_limiting.rs:70-74`).
result: deferred
why_human: Clearnet test infra cannot drive the Tor-only semaphore. Coverage stands via Plan 03 grep audits and the in-source ConnectionGuard contract. Real end-to-end proof requires a future Tor-mode integration harness — captured as a follow-up in TODO.md "Integration test harness reliability" item.

## Summary

total: 3
passed: 2
issues: 0
pending: 0
deferred: 1
skipped: 0
blocked: 0

## Gaps
