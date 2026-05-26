---
phase: 08-public-endpoint-hardening
reviewed: 2026-05-26T00:00:00Z
depth: standard
files_reviewed: 10
files_reviewed_list:
  - coordinator/Cargo.toml
  - coordinator/src/api/middleware.rs
  - coordinator/src/api/mod.rs
  - coordinator/src/config.rs
  - coordinator/src/network/tor.rs
  - coordinator/src/run.rs
  - tests/integration/full_round.rs
  - tests/integration/mod.rs
  - tests/integration/rate_limiting.rs
  - tests/integration/round_bootstrap.rs
findings:
  critical: 2
  warning: 6
  info: 4
  total: 12
status: issues_found
---

# Phase 8: Code Review Report

**Reviewed:** 2026-05-26
**Depth:** standard
**Files Reviewed:** 10
**Status:** issues_found

## Summary

Phase 8 wires per-route `tower_governor` rate limits with `GlobalKeyExtractor`
(correctly avoiding the Tor PeerIp panic), a uniform
`tower_http::timeout::TimeoutLayer`, and a `tokio::sync::Semaphore`-based
connection cap on the Tor accept loop. The structural shape of the code matches
the plan, comments are unusually detailed, and the chosen layer ordering and
semaphore semantics are defended against the exact pitfalls called out in the
research notes (Pitfall 1, 3, 4, 5).

That said, the hardening primitives are missing the cheapest defense the project
could give them: input validation on their own knobs. **Two BLOCKER-class
defects** stem from the fact that any operator can silently configure the
coordinator into a non-serving state via env vars that pass the `try_parsing`
type check but blow up `governor`/`Semaphore` semantics at runtime — and one of
them is a panic that takes the binary down. Several WARNINGs concern test
fragility (the 408 test makes timing assumptions that are easily wrong in CI),
dead/duplicated permit-drop branches, and a tonic dep silently in the build
graph. Findings below.

## Critical Issues

### CR-01: `rate_limit_*_per_min > 60_000` panics the coordinator at startup

**File:** `coordinator/src/api/middleware.rs:74-77, 99-100, 109-110`
**Issue:** `per_min_to_governor` computes `period_ms = 60_000u64 / rpm.max(1) as u64`. For any `rpm > 60_000` (legitimately settable via `BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN=60001` — a `u32` field with no upper bound), integer division yields `period_ms = 0`. `GovernorConfigBuilder::finish()` (verified at `tower_governor-0.8.0/src/governor.rs:234`) returns `None` when `period.as_nanos() == 0`, so the `.expect("non-zero rate_limit_info_per_min and burst — check coordinator config")` panics — **with a misleading message that points at the wrong root cause**, since the operator *did* set a non-zero value.

This is operator-induced startup denial of service via configuration: the binary refuses to start, but `tracing` is initialized only by the caller, so the panic surfaces as an unstructured stderr message with no actionable hint. Tor mode loses its onion descriptor publish; clients cannot discover the coordinator.

There is no integration test for "valid but out-of-range" rpm values — the unit tests at `middleware.rs:197-205` only exercise `with_defaults()` (60 / 30 rpm) which never triggers the divide-to-zero.

**Fix:**
```rust
fn per_min_to_governor(rpm: u32) -> (u64, u32) {
    // Cap at 60_000 rpm (one token per millisecond) — finer-grained limits are
    // not expressible via governor's u64-millisecond period. Reject rpm = 0 at
    // .finish(), but defend against rpm > 60_000 here so the error message in
    // CoordinatorConfig::validate() points at the right field.
    assert!(
        (1..=60_000).contains(&rpm),
        "rate_limit_*_per_min must be in 1..=60_000; got {rpm}. Configure via \
         BLINDJOIN__COORDINATOR__RATE_LIMIT_{{INFO,WRITES}}_PER_MIN."
    );
    let period_ms = 60_000u64 / rpm as u64;
    (period_ms, rpm)
}
```
Better still, hoist a `CoordinatorConfig::validate()` that runs once in `run.rs` before any subsystem reads the config, so misconfiguration produces a single structured error instead of a deep-stack panic.

### CR-02: `max_concurrent_connections = 0` deadlocks the Tor accept loop silently

**File:** `coordinator/src/network/tor.rs:87, 101-104`
**Issue:** `Semaphore::new(max_concurrent_connections as usize)` permits a zero-capacity semaphore. The very first `acquire_owned().await` (line 101-104) parks forever; the accept loop never advances; `stream_requests.next()` is never re-polled; every rendezvous request from Tor stalls. The coordinator's `/info` PKARR record still says "this coordinator is up", but no client can complete a TCP/HS handshake. The `tracing::info!(cap = max_concurrent_connections, "Connection cap configured on Tor accept loop")` at line 88-91 logs `cap = 0` once and then goes silent — no further warning, no failure, no health-check tripwire.

`max_concurrent_connections: u32` in `config.rs:46` has no bound, no validation, no documented minimum. `try_parsing(true)` in `config.rs:130` happily deserializes `BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS=0`. The result is an availability hazard that exactly matches the Phase 8 DoS-mitigation goal in reverse — an attacker who can influence the operator's env (or a typo) silently wedges the coordinator.

**Fix:** Validate in `serve_onion_service` (and centrally in `CoordinatorConfig::validate()`):
```rust
// In serve_onion_service, before Semaphore::new:
anyhow::ensure!(
    max_concurrent_connections >= 1,
    "max_concurrent_connections must be >= 1; got 0. \
     Set BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS to a positive value."
);
```
Additionally, document a sane minimum (e.g. 8) since 1 would serialize all rendezvous handshakes.

## Warnings

### WR-01: `_permit = permit` binding can be removed cleanly without losing the lifetime

**File:** `coordinator/src/network/tor.rs:121-133`
**Issue:** The pattern
```rust
tokio::spawn(async move {
    let _permit = permit;
    if let Err(e) = http1::Builder::new().serve_connection(io, svc).await { ... }
});
```
relies on a reader to understand that `let _permit = permit;` is load-bearing (it prevents `permit` from being dropped at the top of the async block). This is the *one* line the entire connection-cap guarantee turns on; a future refactor that "cleans up the unused binding" silently makes the cap unlimited. The defensive comment is good, but the construct itself is fragile.

Equivalent guarantees can be provided with a less-fragile idiom:
```rust
tokio::spawn(async move {
    // Bind permit to the future's drop scope explicitly; no `_` prefix to
    // tempt cleanup. The match arms force the borrow checker to keep `permit`
    // alive across the await.
    let result = http1::Builder::new().serve_connection(io, svc).await;
    drop(permit); // explicit, surfaces in grep/clippy
    if let Err(e) = result {
        tracing::debug!(error = %e, "HS connection closed");
    }
});
```
Or move the permit into a `struct ConnectionGuard(OwnedSemaphorePermit)` whose `Drop` impl is the contract.

### WR-02: Test 408 assertion can flake under CI scheduler pressure

**File:** `tests/integration/rate_limiting.rs:414, 477`
**Issue:** The 408 test sets `request_timeout_secs = 1` and pauses for `3` seconds mid-body. On a fast machine that is fine, but the test also depends on:
  1. `coordinator::run` reaching the `/info`-ready state inside `wait_http_ready` (10 s).
  2. `tower_http::timeout::TimeoutLayer` actually firing inside the JSON-extractor future (not the connection-accept future).

Point 2 is subtle: `tower_http::timeout::TimeoutLayer` wraps the *handler service*, not the body reader. Whether the timeout fires on a slow body depends on whether axum's `Json<T>` extractor awaits the full body inside the handler future or inside the request-extraction step (which can sit outside `TimeoutLayer`'s scope depending on `ServiceBuilder` composition). The test passing today is good evidence the wiring is right, but the test does not include an "oversize body" path or a `Content-Length` exceeding `RequestBodyLimitLayer::new(64 * 1024)` to distinguish 408 from 413 — so a regression that makes 413 fire first (because `RequestBodyLimitLayer` is now OUTERMOST after the reversal in `api/mod.rs:79-83`) would be invisible to this test.

Also: there is no upper-bound assertion on time-to-408. If the layer waits for the full `Content-Length: 200` before timing out (which would be a regression — it should fire on the deadline regardless of body completion), the test still passes as long as the response arrives within the 5 s outer read timeout.

**Fix:** Add a separate sub-case:
```rust
// Assert 408 fires within ~1.5s (request_timeout_secs=1 + 500ms slack),
// not at 3s when the slow-write finishes:
let start = tokio::time::Instant::now();
let resp = read_response(...).await;
let elapsed = start.elapsed();
assert!(elapsed < Duration::from_millis(1_500), "408 must fire near the deadline, not after body completes; took {:?}", elapsed);
```
And add a 413 test that submits `Content-Length: 70000` to prove the body-limit layer still fires (catches Pitfall 4 regressions).

### WR-03: `tonic` is pulled into the production binary unintentionally

**File:** `coordinator/Cargo.toml:37`
**Issue:** `tower_governor = "0.8"` is declared without `default-features = false`. Per `tower_governor-0.8.0/Cargo.toml` (cached in registry), the default feature set is `["axum", "tonic"]`. The coordinator ships gRPC/tonic transitive code (HTTP/2 stack, prost, etc.) it never instantiates. Beyond binary-size bloat this is an attack-surface contributor (more code paths, more CVE exposure) for a project whose CLAUDE.md explicitly minimises dependencies.

The `tracing` feature of `tower_governor` is *not* enabled (verified — feature gate at `lib.rs:129`), so the per-429 `tracing::info!` log does not fire. That happens to be the correct privacy posture, but it is incidental, not declared. If a future bump pulls in `tracing` by default, every 429 will log under tower_governor's namespace at info level — competing with the coordinator's own envelope.

**Fix:**
```toml
tower_governor = { version = "0.8", default-features = false, features = ["axum"] }
```
Add a regression-test comment noting that enabling `tracing` would emit per-request log lines that have not been audited for PII.

### WR-04: `coordinator::run` warns about `max_concurrent_connections` in clearnet mode but still accepts the value silently

**File:** `coordinator/src/run.rs:282-285`
**Issue:** The warn-log is the right call (operators reading logs see the cap is not enforced), but the structured field `max_concurrent_connections = cfg.coordinator.max_concurrent_connections` is logged at every startup even when the operator made no config change — adding noise. More importantly, an operator running a production node in clearnet mode (against the policy in the warn message) has *no programmatic enforcement* of that policy. A `--no-tor-i-know-what-im-doing` flag or a build-time `#[cfg(not(debug_assertions))]` refusal would make the boundary explicit. As written, "production deployments must use tor_mode = true" is a comment in a warn-log, not a constraint.

**Fix:** Either (a) refuse to start with `tor_mode = false` when `cfg!(not(debug_assertions))` unless `BLINDJOIN_ALLOW_CLEARNET=1` is set, or (b) demote the warn to debug and add a one-time startup `error!` that includes a "set BLINDJOIN_ALLOW_CLEARNET=1 to silence" hint, so the policy is grep-able from the binary.

### WR-05: `Failed to accept HS stream` warn log may include PII through the error's `Display` impl

**File:** `coordinator/src/network/tor.rs:111`
**Issue:** `tracing::warn!(error = %e, "Failed to accept HS stream")` formats `e: arti error` via `Display`. `arti`'s `Display` impls can include circuit identifiers, partial introduction-point keys, or onion-service-side stream metadata. The CLAUDE.md privacy contract forbids logging anything that could allow correlation of clients; an error message containing "circuit id 0x...." is exactly the kind of substring that a malicious operator (or a forensic observer of operator logs) can use to correlate accept failures with subsequent successful connections.

The same concern applies to `tracing::debug!(error = %e, "HS connection closed")` at line 131, though debug-level reduces the operational risk.

**Fix:** Audit each arti error variant the project actually expects, and either (a) match-and-redact via a wrapper that strips identifiers before logging, or (b) downgrade to a counter (`metrics::counter!("hs_accept_failures").increment(1)`) for production builds. At minimum, add a comment near the log site noting which arti error variants have been audited as PII-free.

### WR-06: `tests/integration/full_round.rs` writes hardcoded `ban_list.jsonl` into the cwd

**File:** `tests/integration/full_round.rs:68, 1301`
**Issue:** Both `spawn_coordinator` and `coordinator_info_endpoint_fields` set `ban_file_path: "ban_list.jsonl".into()`. This is a relative path resolved against the test runner's cwd. When integration tests run in parallel (Cargo's default), all three tests that use these helpers race on the same file. Symptoms: occasional `ban_list_persistence` test interference (since that test also touches the same default path), file growing unbounded across test runs, and on CI, the file ends up in `target/` or `/tmp` depending on runner config — making teardown unreliable.

The newer tests (`rate_limiting.rs:201`, `round_bootstrap.rs:104`) correctly use `tempfile::tempdir()` for `ban_file_path`. The Phase 8 work added that pattern but did not retrofit it to the older helpers.

**Fix:** Apply the `tempfile::tempdir()` pattern uniformly:
```rust
let tmp = tempfile::tempdir().expect("create temp dir");
let ban_file_path = tmp.path().join("ban_list.jsonl").to_string_lossy().into_owned();
// ...keep `tmp` alive for the test's duration via a returned guard
```

## Info

### IN-01: Rate-limit `_` arm in `rate_limit_to_json` is unreachable but produces a generic 500

**File:** `coordinator/src/api/middleware.rs:167-183`
**Issue:** The comment correctly notes that `GlobalKeyExtractor::extract()` is infallible, so `UnableToExtractKey` and `Other` are unreachable. But if a future contributor swaps in a different key extractor (or `tower_governor` adds a new variant), the catch-all branch silently returns 500 with `INTERNAL_ERROR` — losing the originating error variant. Worth a `tracing::error!` so the impossible-happened case is at least visible, and consider `#[deny(unreachable_patterns)]` plus `match` against all named variants to force a compile error on future enum changes.

**Fix:**
```rust
GovernorError::UnableToExtractKey | GovernorError::Other { .. } => {
    tracing::error!(?err, "rate-limit subsystem error: key extractor failed unexpectedly");
    // ... existing 500 response
}
```

### IN-02: `governor = "0.10"` direct dep is fragile across tower_governor patch bumps

**File:** `coordinator/Cargo.toml:42`
**Issue:** The comment correctly explains why this is pinned (to name `NoOpMiddleware` / `QuantaInstant` in the `RateLimitLayers` struct), but tower_governor 0.8 depends on `governor = "0.10.0"` — a future 0.8.1 could bump to `0.10.5` while the coordinator stays on `0.10` (semver-compatible), and `governor 0.11.0` would break both at once. Add a `# bump in lockstep with tower_governor` note, and consider using a type alias from `tower_governor::middleware` (if/when it re-exports) to avoid the direct dep.

### IN-03: `parse_outpoint` failure in input handler returns `UTXO_NOT_FOUND`

**File:** `coordinator/src/api/handlers.rs:90-93` (cross-referenced from this review)
**Issue:** A syntactically malformed `utxo_outpoint` returns the code `UTXO_NOT_FOUND` — but the UTXO was never *looked up*; the request was malformed. This conflates two distinct failure modes for clients (retry-with-different-UTXO vs fix-the-request). A `MALFORMED_OUTPOINT` code or `BAD_REQUEST` distinction is clearer. Out of this phase's scope, but surfaced because the rate-limit envelope at `middleware.rs:148-165` correctly uses `RATE_LIMITED` — the bias toward semantic codes is established by Phase 8 work and worth backfilling.

### IN-04: `_permit = permit` and `drop(permit)` duplicate the permit-drop intent

**File:** `coordinator/src/network/tor.rs:113`
**Issue:** Line 113 has `drop(permit)` in the `Err` branch, but the spawned task at line 126 *also* takes ownership via `let _permit = permit;`. Because the `match` block consumes `permit` in the `Ok(ds)` arm by binding `data_stream`, then the `tokio::spawn` move closure moves `permit` again — but `permit` was already moved into the spawn? Re-reading the source: on `Err(e) =>` the explicit `drop(permit); continue;` ensures release. On `Ok(ds) =>` `permit` is NOT yet moved — it survives the match and is then moved into `async move { let _permit = permit; ... }`. So the code is correct, but the explicit `drop(permit)` in the `Err` arm is actually unnecessary: simply `continue;` after the `Err` arm would drop `permit` at the end of the scope. The explicit `drop` is defensive and arguably clearer; this is a style note, not a defect.

## Structural Findings (fallow)

No `<structural_findings>` block was provided with this review request, so this section is intentionally empty. If a structural pre-pass is added later for Phase 8, fold its results in above WR-01.

---

_Reviewed: 2026-05-26_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
