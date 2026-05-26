---
status: partial
phase: 08-public-endpoint-hardening
source: [08-VERIFICATION.md]
started: 2026-05-26T00:00:00Z
updated: 2026-05-26T00:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Runtime proof of HTTP 429 + Retry-After + RATE_LIMITED JSON envelope on /info under flood
expected: With bitcoind in PATH, `cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture --include-ignored` runs end-to-end and emits `info_endpoint_returns_429_when_flooded PASSED: 429 + retry-after + JSON envelope (code=RATE_LIMITED) observed`. In the verification environment bitcoind is absent and the test graceful-skipped — the assertion code is wired correctly but the runtime behavior was not exercised.
result: [pending]
why_human: Requires bitcoind binary on PATH or via `$BITCOIND_EXE`. Verifier confirmed compile, registration, and graceful-skip path. End-to-end execution must occur in a CI/local environment with bitcoind installed.

### 2. Runtime proof of HTTP 408 REQUEST_TIMEOUT on slow-body request
expected: With bitcoind in PATH, `cargo test --test integration rate_limiting::request_timeout_returns_408 -- --nocapture --include-ignored` runs end-to-end and emits `request_timeout_returns_408 PASSED: HTTP 408 REQUEST_TIMEOUT observed within 5s of a request that paused mid-body for 3s against request_timeout_secs=1`. WR-02 fix means the test also asserts time-to-first-byte < 1750 ms (proves the layer fires near the deadline, not after the body completes).
result: [pending]
why_human: Requires bitcoind binary on PATH or via `$BITCOIND_EXE`. Verifier confirmed compile, registration, and graceful-skip path. End-to-end execution must occur in a CI/local environment with bitcoind installed.

### 3. Tor connection-cap runtime behavior (N+1 streams park beyond max_concurrent_connections)
expected: An attacker opening 257 simultaneous .onion streams sees only 256 served; the 257th parks until an earlier connection finishes. Plan 04 explicitly defers this assertion to a future-phase Tor-mode harness (`TODO(Phase-8 Q3, A4)` in `tests/integration/rate_limiting.rs:70-74`).
result: [pending]
why_human: Clearnet test infra cannot drive the Tor-only semaphore. Coverage stands via Plan 03 grep audits and the in-source ConnectionGuard contract. Real end-to-end proof requires a future Tor-mode integration harness.

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
