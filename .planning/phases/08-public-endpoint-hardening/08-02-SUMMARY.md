---
phase: 08-public-endpoint-hardening
plan: 02
subsystem: api
tags: [tower, tower_governor, tower-http, rate-limit, timeout, axum, middleware, dos-mitigation]

# Dependency graph
requires:
  - phase: 08-01-public-endpoint-hardening-foundation
    provides: "CoordinatorSection knobs rate_limit_info_per_min/rate_limit_writes_per_min/request_timeout_secs + tower_governor 0.8 dep + tower-http timeout feature"
provides:
  - "coordinator/src/api/middleware.rs factory module exports `build_rate_limit_layers(&CoordinatorConfig) -> RateLimitLayers` and `build_timeout_layer(&CoordinatorConfig) -> TimeoutLayer`"
  - "RateLimitLayers struct with `reads_layer` (60 rpm shared by `/info` + `/round/tx`) and `writes_layer` (30 rpm shared by `/round/input`, `/round/output`, `/round/sign`) — both GovernorLayer<GlobalKeyExtractor, NoOpMiddleware, axum::body::Body>"
  - "coordinator/src/api/mod.rs::build_router_with_ban_list wires per-route GovernorLayer onto each MethodRouter + Router-scope TimeoutLayer + RequestBodyLimitLayer composed via ServiceBuilder (Pitfall 3 safe)"
  - "Custom 429 response body matching project's JSON envelope shape: `{\"error\":{\"code\":\"RATE_LIMITED\",\"message\":\"...\",\"round_id\":null}}` (mirrors handlers::api_error)"
  - "408 timeout responses emitted by tower_http::timeout::TimeoutLayer when handler exceeds `request_timeout_secs`"
  - "GovernorLayer uses GlobalKeyExtractor (Tor-safe — Pitfall 1 mitigated); fail-fast `.expect(\"non-zero rpm and burst\")` on misconfigured rpm=0 (T-08-02-05)"
affects: [08-03-connection-cap, 08-04-integration-test]

# Tech tracking
tech-stack:
  added:
    - "governor = \"0.10\" (direct dep added — was already transitive via tower_governor 0.8; needed for naming NoOpMiddleware/QuantaInstant in RateLimitLayers struct fields)"
    - "http = \"1\" (direct dep added — was already transitive; needed for `http::StatusCode` import in middleware.rs)"
  patterns:
    - "Factory-function-per-layer-kind pattern in middleware.rs (testable via inline #[cfg(test)] construction proof)"
    - "Per-bucket Arc<GovernorConfig> isolation (one Arc per quota bucket, never reused — tower_governor README pitfall #1)"
    - "ServiceBuilder for Router-scope layer composition (>1 layer at Router scope mandates ServiceBuilder per RESEARCH Pitfall 3)"

key-files:
  created: []
  modified:
    - "coordinator/Cargo.toml — added `governor = \"0.10\"` and `http = \"1\"` as direct deps (both already transitive; no version churn)"
    - "coordinator/src/api/middleware.rs — replaced 2-line stub with ~165-line factory module (factory functions + JSON error handler + construction tests)"
    - "coordinator/src/api/mod.rs — added `use tower::ServiceBuilder;`, threaded RateLimitLayers per-route, reordered Router-scope ServiceBuilder composition"
    - "Cargo.lock — updated to record direct deps for governor 0.10.4 and http 1.4.0 (no new resolved versions)"

key-decisions:
  - "Custom GovernorLayer error_handler emits the project's JSON envelope shape on 429 responses (D-06 + A5) — operators and tools see one consistent error format across all HTTP failure modes"
  - "GlobalKeyExtractor is called FIRST in both bucket builders (RESEARCH Pitfall 1 mitigation); typestate of GovernorConfigBuilder<GlobalKeyExtractor, NoOpMiddleware> makes the requirement compiler-visible"
  - "burst_size = rpm + per_millisecond = 60_000/rpm (A1 resolution) — forgiving GCRA convention; client can briefly burn a minute's budget then waits"
  - "Two SEPARATE Arc<GovernorConfig> allocations (reads, writes) — never one builder reused; per tower_governor README pitfall #1, reusing the same config creates independent limiters"
  - "Router-scope layer order REVERSED from the plan: RequestBodyLimitLayer OUTERMOST, TimeoutLayer INNER. Plan proposed timeout-outer, but tower_http::timeout::Timeout::Service requires its inner service's response body to implement Default — `ResponseBody<axum::body::Body>` (from RequestBodyLimitLayer) does NOT implement Default, causing E0277. Reversed order compiles and remains functionally correct (Pitfall 4)."
  - "Custom error_handler attached to BOTH reads_layer and writes_layer via .error_handler(rate_limit_to_json) — confirms the JSON envelope on 429s regardless of which route is hit"

patterns-established:
  - "tower middleware factory in `coordinator/src/api/middleware.rs` is the canonical home for any new Layer construction; api/mod.rs consumes via `let limits = middleware::build_xxx(&config);` then attaches per-route or via ServiceBuilder"
  - "Inline #[cfg(test)] mod tests at the bottom of factory modules — assertion-free construction tests catch runtime panics from misconfigured config defaults (T-08-02-05 fail-fast complement)"

requirements-completed: []

# Metrics
duration: 5min
completed: 2026-05-26
---

# Phase 8 Plan 02: Rate-limit and request-timeout middleware Summary

**Per-route tower_governor GovernorLayer (with GlobalKeyExtractor for Tor-safety) + uniform tower_http::timeout::TimeoutLayer wrap the coordinator HTTP API — flooded routes now return HTTP 429 with Retry-After and a JSON envelope; stalled handlers return HTTP 408.**

## Performance

- **Duration:** ~5.5 min
- **Started:** 2026-05-26T03:58:13Z
- **Completed:** 2026-05-26T04:03:45Z
- **Tasks:** 3 of 3 complete (Task 1 was the human-verify checkpoint, completed by the prior agent with user response `approved`)
- **Files modified:** 4 (Cargo.toml, Cargo.lock, api/middleware.rs, api/mod.rs)

## Accomplishments

- `coordinator/src/api/middleware.rs` upgraded from 2-line stub to 165-line factory module. Exports `build_rate_limit_layers(&CoordinatorConfig) -> RateLimitLayers` and `build_timeout_layer(&CoordinatorConfig) -> TimeoutLayer`. RateLimitLayers carries two `GovernorLayer<GlobalKeyExtractor, NoOpMiddleware, axum::body::Body>` instances — one per quota bucket (reads, writes).
- Both bucket builders call `.key_extractor(GlobalKeyExtractor)` FIRST (RESEARCH Pitfall 1; Tor-safe — `DataStream` has no peer SocketAddr extension). The typestate transition `GovernorConfigBuilder<PeerIpKeyExtractor, _> -> GovernorConfigBuilder<GlobalKeyExtractor, _>` makes the requirement visible in the bound types.
- Custom `rate_limit_to_json` error handler shapes 429 responses to the existing `handlers::api_error` envelope (D-06 + A5):
  ```json
  { "error": { "code": "RATE_LIMITED", "message": "Too many requests; retry after 2s", "round_id": null } }
  ```
  with `retry-after: 2` header (duplicated from tower_governor's own header for self-describing responses).
- Uniform Router-scope timeout via `tower_http::timeout::TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(cfg.coordinator.request_timeout_secs))` — emits a 408 with empty body when any handler exceeds the deadline (default 30s).
- `coordinator/src/api/mod.rs::build_router_with_ban_list` wires per-route GovernorLayer via `MethodRouter::layer`:
  ```
  /info         → limits.reads_layer.clone()
  /round/input  → limits.writes_layer.clone()
  /round/output → limits.writes_layer.clone()
  /round/sign   → limits.writes_layer.clone()
  /round/tx     → limits.reads_layer.clone()
  ```
  Router-scope layers compose via `ServiceBuilder` (Pitfall 3 — never mix bare `.layer()` chaining with ServiceBuilder when there is >1 Router-scope layer).
- `cargo build --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p coordinator --lib api::middleware` all pass clean. The full 58 coordinator-lib unit tests pass (was 56 before; 2 new construction tests added).
- The 2 inline construction tests (`rate_limit_layers_construct_with_defaults`, `timeout_layer_constructs_with_defaults`) runtime-prove that the factory functions don't panic with `CoordinatorConfig::with_defaults()` — catches the typo class (`per_milisecond` with one `l`, `60_000 / 0`, etc.) that `cargo build`/`cargo clippy` cannot.

## (a) middleware.rs API surface

```rust
pub struct RateLimitLayers {
    pub reads_layer:  GovernorLayer<GlobalKeyExtractor, NoOpMw, axum::body::Body>,
    pub writes_layer: GovernorLayer<GlobalKeyExtractor, NoOpMw, axum::body::Body>,
}

pub fn build_rate_limit_layers(cfg: &CoordinatorConfig) -> RateLimitLayers;
pub fn build_timeout_layer(cfg: &CoordinatorConfig)     -> tower_http::timeout::TimeoutLayer;

fn rate_limit_to_json(err: GovernorError) -> axum::response::Response<axum::body::Body>; // private
```

Both `GovernorLayer` instances are constructed with `.error_handler(rate_limit_to_json)`. The `NoOpMw` type alias resolves to `::governor::middleware::NoOpMiddleware<::governor::clock::QuantaInstant>` (needs the direct `governor = "0.10"` dep added in this plan — see "Deviations" §1).

## (b) Per-route wiring in mod.rs

| Route             | Bucket  | rpm budget (default) | Source |
|-------------------|---------|----------------------|--------|
| `GET  /info`      | reads   | 60                   | `cfg.coordinator.rate_limit_info_per_min` |
| `POST /round/input`  | writes  | 30                | `cfg.coordinator.rate_limit_writes_per_min` |
| `POST /round/output` | writes  | 30                | `cfg.coordinator.rate_limit_writes_per_min` |
| `POST /round/sign`   | writes  | 30                | `cfg.coordinator.rate_limit_writes_per_min` |
| `GET  /round/tx`  | reads   | 60                   | `cfg.coordinator.rate_limit_info_per_min` |

The three write routes clone the SAME `GovernorLayer` (which wraps a single `Arc<GovernorConfig>`) — they share one global limiter at 30 rpm. The two read routes clone the SAME `GovernorLayer` for the reads bucket (60 rpm shared). A flooded write route consumes the bucket for all three writes; reads remain independent (D-02 + A2).

## (c) Pitfall 1 audit — GlobalKeyExtractor on BOTH buckets

```bash
$ grep -c "GlobalKeyExtractor" coordinator/src/api/middleware.rs
12   # imports, type signatures, two .key_extractor(GlobalKeyExtractor) calls, doc references
```

The two `.key_extractor(GlobalKeyExtractor)` calls (one per bucket builder) are the canonical mitigation. Without them, the default `PeerIpKeyExtractor` would `Err(GovernorError::UnableToExtractKey)` on every Tor request — surface as HTTP 500 to every client. Audit PASSED.

## (d) Pitfall 3 audit — ServiceBuilder for Router-scope composition

```bash
$ grep -nE "ServiceBuilder::new\(\)|build_timeout_layer\(" coordinator/src/api/mod.rs
80:            ServiceBuilder::new()
82:                .layer(middleware::build_timeout_layer(&config)),
```

Router-scope composition uses `ServiceBuilder::new().layer(RequestBodyLimitLayer::new(64*1024)).layer(build_timeout_layer(&config))`. Top-to-bottom = outside-in semantics — RequestBodyLimitLayer is OUTERMOST, TimeoutLayer is closer to the handler. No bare chained `.layer()` calls for Router-scope work. Per-route layers attach via single `MethodRouter::layer` calls (one layer per route, no ordering risk). Audit PASSED.

## (e) JSON 429 envelope shape (A5 resolution)

```json
{
  "error": {
    "code": "RATE_LIMITED",
    "message": "Too many requests; retry after {wait_time}s",
    "round_id": null
  }
}
```

Headers emitted by `rate_limit_to_json`:
- `content-type: application/json`
- `retry-after: {wait_time}` (echoed from tower_governor's internal computation — self-describing envelope)
- Plus whatever headers tower_governor's internal `negative.quota()` path attaches via the `extend(h)` call (typically `retry-after`, `x-ratelimit-after`).

Status code: `429 Too Many Requests`. Matches the existing `handlers::api_error` factory at `handlers.rs:30-43` so operators see one envelope shape across all error paths.

## (f) D-05 deviation rationale — `tower_http::timeout::TimeoutLayer` over `tower::timeout::TimeoutLayer`

The plan's must_haves explicitly preserve this deviation:

```
D-05 deviation (documented): timeout uses `tower_http::timeout::TimeoutLayer`, NOT
`tower::timeout::TimeoutLayer` — the latter returns `BoxError` requiring
`HandleErrorLayer` (RESEARCH §"Standard Stack — Alternatives Considered").
```

Verification:
```bash
$ grep -n "tower_http::timeout::TimeoutLayer" coordinator/src/api/middleware.rs
22://   - D-05 deviation: timeout uses `tower_http::timeout::TimeoutLayer`, NOT
41:use tower_http::timeout::TimeoutLayer;
123:/// Crate path matters: this is `tower_http::timeout::TimeoutLayer`, NOT
```

No `tower::timeout::TimeoutLayer` imports anywhere. The tower_http variant returns a clean empty-body Response with the configured status code (408 here) — no `HandleErrorLayer` wrapping ceremony. Matches Plan 03's parallel choice for `tower_http::limit::*` over `tower::limit::ConcurrencyLimitLayer`.

## (g) Inline #[cfg(test)] construction test outcome

```bash
$ cargo test -p coordinator --lib api::middleware
test api::middleware::tests::timeout_layer_constructs_with_defaults ... ok
test api::middleware::tests::rate_limit_layers_construct_with_defaults ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out
```

Both tests exercise the factories with `CoordinatorConfig::with_defaults()`. The point: `.finish().expect("non-zero rpm and burst")` and the `TimeoutLayer::with_status_code` constructor MUST NOT panic against the default rpm values (60, 30) and the default `request_timeout_secs` (30). Construction not panicking IS the assertion — catches Pitfall 1 typos (`per_milisecond`) and arithmetic errors (`60_000 / 0`) that `cargo build`/`cargo clippy` cannot detect.

## (h) Scope acknowledgement — Plan 03 + Plan 04 boundaries

- **Plan 03 (connection cap)** is the parallel Wave 2 plan. It adds a `tokio::sync::Semaphore` in `coordinator/src/network/tor.rs`'s accept loop to bound concurrent HS streams at `cfg.coordinator.max_concurrent_connections`. That work is OUT of scope for this plan; this plan only wires the rate-limit and request-timeout layers. Plan 03 makes the same "deviation" choice (`tower_http::limit::*` over `tower::limit::ConcurrencyLimitLayer`).
- **Plan 04 (integration test)** will spawn the coordinator with TIGHT limits (e.g. `rate_limit_info_per_min = 3`), flood `/info` past the budget, and assert at least one HTTP 429 with `retry-after` header. It will ALSO test a slow handler past `request_timeout_secs` and assert HTTP 408. That end-to-end proof is OUT of scope for this plan; this plan only guarantees that the layers construct cleanly and compile-time-correctly wrap the Router.

## (i) Task 1 human-verify checkpoint outcome

Task 1 was a `checkpoint:human-verify` blocking-human gate. The prior executor agent paused at Task 1 and surfaced the package-legitimacy verification checklist for `tower_governor 0.8`:
- crates.io page: latest 0.8.x, repository `github.com/benwis/tower-governor`, recent publish, MIT/Apache-2.0, uploader `benwis`.
- GitHub repo: public, README + commit history, `key_extractor::GlobalKeyExtractor` symbol present in `src/key_extractor.rs`.

The user responded `approved`. The continuation agent (this one) resumed from Task 2 with confidence that the `use tower_governor::*` imports in `middleware.rs` are not introducing a slopsquatted/hallucinated dependency.

## Task Commits

Each task was committed atomically:

1. **Task 1: Human-verify tower_governor crate before first runtime use** — no commit (verification-only checkpoint; user responded `approved`)
2. **Task 2: Implement middleware.rs factory functions for rate-limit and timeout layers** — `b455c14` (feat)
3. **Task 3: Wire middleware into the Router builder in api/mod.rs** — `5552206` (feat)

## Files Created/Modified

- `coordinator/Cargo.toml` — added two direct deps that were already transitive: `governor = "0.10"` (so middleware.rs can name `NoOpMiddleware`/`QuantaInstant` in struct fields — `tower_governor` doesn't `pub use` them) and `http = "1"` (so middleware.rs can `use http::StatusCode`). Cargo.lock untouched in terms of resolved versions.
- `Cargo.lock` — updated to record the new direct binding for `governor 0.10.4` and `http 1.4.0` (no new resolved versions, just lock-file accounting).
- `coordinator/src/api/middleware.rs` — replaced the 2-line stub with a 165-line factory module exporting `RateLimitLayers`, `build_rate_limit_layers`, `build_timeout_layer`, and the private `rate_limit_to_json` error handler; plus two #[cfg(test)] construction tests.
- `coordinator/src/api/mod.rs` — added `use tower::ServiceBuilder;`, threaded `let limits = middleware::build_rate_limit_layers(&config);` near the top of `build_router_with_ban_list`, attached per-route `GovernorLayer` clones to each `MethodRouter`, and replaced the bare `.layer(RequestBodyLimitLayer::new(64 * 1024))` with `.layer(ServiceBuilder::new().layer(RequestBodyLimitLayer::new(64 * 1024)).layer(middleware::build_timeout_layer(&config)))`.

## Decisions Made

- **Custom 429 envelope** (A5): wired `rate_limit_to_json` via `GovernorLayer::error_handler` on BOTH buckets so flooded clients see the project's standard JSON envelope shape, not the default plain-text `Too Many Requests! Wait for Ns`.
- **Two separate Arc<GovernorConfig> allocations**: one per quota bucket — never one builder reused (RESEARCH Pitfall #1; reusing creates independent limiters with split state).
- **GlobalKeyExtractor first in chain**: typestate transition makes the requirement compiler-visible in the function signature (`GovernorConfigBuilder<GlobalKeyExtractor, NoOpMiddleware>`).
- **Direct deps governor + http**: added to `coordinator/Cargo.toml` because `tower_governor 0.8` does NOT `pub use` `governor::middleware::*` or `governor::clock::*`, but the `RateLimitLayers` struct fields MUST name `NoOpMiddleware<QuantaInstant>` as type parameters in `GovernorLayer<K, M, RespBody>`. Both crates were already transitive deps — cargo tree shows no new resolved versions, only new direct-dep edges.
- **Layer order reversed** (vs plan): `RequestBodyLimitLayer` outermost, `TimeoutLayer` inner — see Deviations §2 below for full E0277 trait-bound analysis.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue: unresolved imports] Added `governor = "0.10"` and `http = "1"` direct deps to `coordinator/Cargo.toml`**

- **Found during:** Task 2 (middleware.rs first `cargo build`)
- **Issue:**
  - `use http::StatusCode;` failed with E0432 ("unresolved import `http`") — the `http` crate was transitive (via axum/tower) but not a direct dep.
  - `type NoOpMw = ::governor::middleware::NoOpMiddleware<::governor::clock::QuantaInstant>;` failed with E0433 ("cannot find `governor` in the crate root") for the same reason — `governor 0.10` was transitive via `tower_governor`, but `tower_governor` does NOT `pub use` `governor::middleware::*` or `governor::clock::*`, and naming type parameters in struct fields requires the path to be reachable.
  - The `RateLimitLayers` struct fields MUST name `NoOpMw` in `GovernorLayer<K, M, RespBody>` — third type parameter `M` is `RateLimitingMiddleware<QuantaInstant>` per the trait bound on `GovernorConfigBuilder`.
- **Fix:** Added `governor = "0.10"` and `http = "1"` to `coordinator/Cargo.toml` `[dependencies]`. Both crates were already transitive deps (`cargo tree | grep -E "governor v|http v"` showed `governor v0.10.4` and `http v1.4.0` prior to the addition); the change adds direct-dep edges only, no resolved-version changes.
- **Verification:** `cargo build --all-targets` exits 0 cleanly. `cargo tree` shows no new resolved versions added.
- **Files modified:** `coordinator/Cargo.toml`, `Cargo.lock`
- **Committed in:** `b455c14` (Task 2 commit)

**2. [Rule 1 — Bug / Rule 3 — Blocking: compile error] Reversed Router-scope layer order: RequestBodyLimitLayer outermost, TimeoutLayer inner**

- **Found during:** Task 3 (api/mod.rs first `cargo build`)
- **Issue:** The plan prescribed `ServiceBuilder::new().layer(build_timeout_layer(&config)).layer(RequestBodyLimitLayer::new(64*1024))` (timeout outermost). This fails to compile with E0277:
  ```
  the trait `std::default::Default` is not implemented for `ResponseBody<axum::body::Body>`
  required for `tower_http::timeout::Timeout<RequestBodyLimit<Route>>` to implement
  `tower::Service<http::Request<axum::body::Body>>`
  ```
  Root cause: `tower_http::timeout::Timeout::Service` has trait bound `ResBody: Default` on the inner service's response body type (it constructs an empty-body Response with the configured status code on elapsed deadline). When TimeoutLayer is outside RequestBodyLimitLayer, the inner service's response body is `tower_http::limit::ResponseBody<axum::body::Body>`, which does NOT implement `Default`.
- **Fix:** Reverse the layer order: `ServiceBuilder::new().layer(RequestBodyLimitLayer::new(64*1024)).layer(build_timeout_layer(&config))`. Now TimeoutLayer's inner service is the bare route handler stack with response body `axum::body::Body`, which DOES implement `Default`. Compiles cleanly.
- **Functional impact:** Per RESEARCH Pitfall 4: timeout still covers slow body reads because `RequestBodyLimitLayer` reads the body INSIDE the handler future TimeoutLayer wraps. The combined behavior is "bounded slot consumption for any oversize body, however framed". Sub-millisecond admission cost of body-limit-then-timeout is unchanged from timeout-then-body-limit.
- **Verification:** `cargo build -p coordinator --all-targets` exits 0; `cargo clippy -p coordinator --all-targets -- -D warnings` exits 0; `cargo test --no-run --test integration` compiles. The full Plan 04 integration test will prove the behavior end-to-end.
- **Files modified:** `coordinator/src/api/mod.rs` (inline comment documents the trait-bound reason for the reversal)
- **Committed in:** `5552206` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 Rule 3 blocking-import, 1 Rule 1+3 compile-error layer-order fix)
**Impact on plan:** Both were essential for correctness/compile-ability. Neither changes the threat model (T-08-02-* coverage is preserved — Pitfall 1, 3, 4 all still mitigated; T-08-02-03 slow-loris coverage retained because TimeoutLayer still wraps the handler future).

## Issues Encountered

- E0432/E0433 unresolved-import errors on the first build of `middleware.rs` (governor + http not direct deps) — resolved via Cargo.toml additions (Deviation §1).
- E0277 trait-bound error on the first build of `mod.rs` (ResponseBody<axum::body::Body> doesn't implement Default) — resolved via layer-order reversal (Deviation §2).
- No third-party / runtime issues. tower_governor 0.8.0, governor 0.10.4, tower_http 0.6.8 all resolve and link cleanly through the existing axum 0.8 + tower 0.5 graph.

## User Setup Required

None. The four operator-tunable knobs (`rate_limit_info_per_min`, `rate_limit_writes_per_min`, `request_timeout_secs`, `max_concurrent_connections`) landed in Plan 01 with production-safe defaults (60/30/30/256). Plan 02 only wires the layers — no operator action required for default behavior. Operators may override via `BLINDJOIN__COORDINATOR__*` env vars (inherited from Plan 01).

## Threat-model coverage

| Threat ID | Status |
|-----------|--------|
| T-08-02-01 (DoS volume on writes) | mitigated — `writes_layer` enforces `rate_limit_writes_per_min` shared across the three write routes (D-02 + A2). |
| T-08-02-02 (DoS volume on reads) | mitigated — `reads_layer` enforces `rate_limit_info_per_min` shared across `/info` and `/round/tx`. |
| T-08-02-03 (slow-loris) | mitigated — Router-scope `TimeoutLayer::with_status_code(REQUEST_TIMEOUT, request_timeout_secs)` cuts handlers past deadline (default 30s). Combined with RequestBodyLimitLayer per RESEARCH Pitfall 4: bounded slot consumption for any oversize body, however framed. |
| T-08-02-04 (info-disclosure via 429 body) | accepted — body matches `api_error` envelope, no PII. |
| T-08-02-05 (misconfiguration: rpm=0) | mitigated — `.finish().expect("non-zero rpm and burst")` fails the coordinator at startup with a clear error message. Inline construction tests catch the typo-class at `cargo test` time. |
| T-08-02-06 (PeerIpKeyExtractor would panic on Tor) | mitigated — `.key_extractor(GlobalKeyExtractor)` first in BOTH bucket builders; grep-verified (`GlobalKeyExtractor` count = 12 in middleware.rs). |
| T-08-02-07 (Layer ordering: ServiceBuilder vs bare .layer) | mitigated — Router-scope composition uses `ServiceBuilder::new().layer(...).layer(...)` exclusively; per-route layers use single bare `.layer()` (one layer per route, no ordering risk). |
| T-08-02-SC (first runtime use of tower_governor: slopcheck substitute) | mitigated — Task 1 human-verify checkpoint completed; user response `approved` before Task 2 ran the first `use tower_governor::*` line. |

## Verification evidence

- `cargo build -p coordinator --all-targets` exits 0 (clean — final build after Task 3).
- `cargo clippy -p coordinator --all-targets -- -D warnings` exits 0 (no warnings; doc-list-indent lint passed after one fixup during Task 2).
- `cargo test -p coordinator --lib` → 58 passed, 0 failed (was 56 prior to this plan; +2 from `api::middleware::tests::*`).
- `cargo test -p coordinator --lib api::middleware` → 2 passed, 0 failed.
- `cargo test --no-run --test integration` → compiles clean against the new Router shape (no behavior tested here — Plan 04's job).
- `cargo test --workspace --lib` → all crates green.
- `grep -c "GlobalKeyExtractor" coordinator/src/api/middleware.rs` → 12 (Pitfall 1 audit, ≥2 required).
- `grep -c "GovernorConfigBuilder::default()" coordinator/src/api/middleware.rs` → 2 (one per bucket).
- `grep -c "RATE_LIMITED" coordinator/src/api/middleware.rs` → 2 (A5 envelope code in production + comment).
- `grep -c "StatusCode::REQUEST_TIMEOUT" coordinator/src/api/middleware.rs` → 1 (408 emitted by TimeoutLayer).
- `grep -c "tower_http::timeout::TimeoutLayer" coordinator/src/api/middleware.rs` → 4 (D-05 deviation guard — confirms NOT `tower::timeout::TimeoutLayer`).
- `grep -c "limits.writes_layer.clone()" coordinator/src/api/mod.rs` → 3 (three write routes, ≥3 required).
- `grep -c "limits.reads_layer.clone()" coordinator/src/api/mod.rs` → 2 (two read routes, ≥2 required).
- `grep -c "ServiceBuilder::new" coordinator/src/api/mod.rs` → 1 (Pitfall-3-safe composition).
- `grep -c "use tower::ServiceBuilder" coordinator/src/api/mod.rs` → 1.

## Next Phase Readiness

**Plan 03 (connection cap)** is now unblocked at the same Wave 2 level — Plan 03 reads `cfg.coordinator.max_concurrent_connections` (already landed in Plan 01) and adds a semaphore to `coordinator/src/network/tor.rs`'s accept loop. Plan 03 does not touch the Router or middleware; the two plans are orthogonal.

**Plan 04 (integration test)** is now unblocked once Plan 03 completes — it will spawn the coordinator with TIGHT rate limits (e.g. `rate_limit_info_per_min = 3`) and verify end-to-end:
- Flooding `/info` past the budget produces at least one HTTP 429 with `Retry-After` header.
- A slow handler past `request_timeout_secs` produces HTTP 408.
- (Connection-cap behavior is exercised in Plan 03's grep audits and a `tokio::join!`-style test; documented as deferred for the Tor-only path per RESEARCH §"Open Questions RESOLVED" Q3.)

**Reminder for Plan 04 authors:** The 429 response body shape this plan ships is the project's standard JSON envelope (`{"error":{"code":"RATE_LIMITED","message":"...","round_id":null}}`). Plan 04's assertions should check for the `Retry-After` header and the 429 status code — and optionally also assert the JSON envelope shape for forward-compatibility.

## Self-Check

- Created files: none (this plan is modification-only).
- Modified files exist:
  - `coordinator/Cargo.toml` — FOUND
  - `coordinator/src/api/middleware.rs` — FOUND (165 lines)
  - `coordinator/src/api/mod.rs` — FOUND
  - `Cargo.lock` — FOUND
- Commits:
  - `b455c14` (Task 2) — FOUND in `git log --oneline -5`
  - `5552206` (Task 3) — FOUND in `git log --oneline -5`

## Self-Check: PASSED

---
*Phase: 08-public-endpoint-hardening*
*Completed: 2026-05-26*
