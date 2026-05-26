---
phase: 08-public-endpoint-hardening
verified: 2026-05-26T00:00:00Z
status: human_needed
score: 11/11 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Runtime proof of HTTP 429 + Retry-After + RATE_LIMITED JSON envelope on /info under flood"
    expected: "With bitcoind in PATH, `cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture --include-ignored` runs end-to-end and emits `info_endpoint_returns_429_when_flooded PASSED: 429 + retry-after + JSON envelope (code=RATE_LIMITED) observed`. In this verification environment bitcoind is absent and the test graceful-skipped — the assertion code is wired correctly but the runtime behavior was not exercised here."
    why_human: "Requires bitcoind binary on PATH or via $BITCOIND_EXE. Verifier confirmed compile, registration, and graceful-skip path. End-to-end execution must occur in a CI/local environment with bitcoind installed."
  - test: "Runtime proof of HTTP 408 REQUEST_TIMEOUT on slow-body request"
    expected: "With bitcoind in PATH, `cargo test --test integration rate_limiting::request_timeout_returns_408 -- --nocapture --include-ignored` runs end-to-end and emits `request_timeout_returns_408 PASSED: HTTP 408 REQUEST_TIMEOUT observed within 5s of a request that paused mid-body for 3s against request_timeout_secs=1`. WR-02 fix means the test also asserts time-to-first-byte < 1750 ms (proves the layer fires near the deadline, not after the body completes)."
    why_human: "Requires bitcoind binary on PATH or via $BITCOIND_EXE. Verifier confirmed compile, registration, and graceful-skip path. End-to-end execution must occur in a CI/local environment with bitcoind installed."
  - test: "Tor connection-cap runtime behavior (N+1 streams park beyond max_concurrent_connections)"
    expected: "An attacker opening 257 simultaneous .onion streams sees only 256 served; the 257th parks until an earlier connection finishes. Plan 04 explicitly defers this assertion to a future-phase Tor-mode harness (TODO(Phase-8 Q3, A4) in tests/integration/rate_limiting.rs:70-74)."
    why_human: "Clearnet test infra cannot drive the Tor-only semaphore. Coverage stands via Plan 03 grep audits and the in-source ConnectionGuard contract. Real end-to-end proof requires a future Tor-mode integration harness."
---

# Phase 08: public-endpoint-hardening Verification Report

**Phase Goal:** The coordinator HTTP API resists volume-based denial-of-service when exposed publicly: `/round/input` and `/round/sign` cannot be flooded past global per-route rate limits (HTTP 429 + Retry-After); slow clients cannot tie up request slots indefinitely (per-route timeouts, HTTP 408); concurrent connection counts at the Tor listener are bounded; all limits are operator-tunable via `coordinator.toml`. Per-peer throttling is impossible on Tor by design (see CONTEXT D-01); sybil resistance is BIP-322 ownership proofs (unchanged), not rate limits.

**Verified:** 2026-05-26
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Must-Haves)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `/round/input` + `/round/sign` cannot be flooded past per-route rate limits (HTTP 429 + Retry-After) | VERIFIED (static) | `coordinator/src/api/mod.rs:54,56` attaches `limits.writes_layer.clone()` to POST `/round/input` and POST `/round/sign`; `coordinator/src/api/middleware.rs:114-121` constructs the writes-bucket `GovernorConfig` at `rate_limit_writes_per_min` rpm; `coordinator/src/api/middleware.rs:166-169` emits 429 + `retry-after` header. Runtime proof requires bitcoind (graceful-skipped here). |
| 2 | Slow clients cut with per-route timeouts (HTTP 408) | VERIFIED (static) | `coordinator/src/api/middleware.rs:138-141` returns `TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(request_timeout_secs))`; `coordinator/src/api/mod.rs:79-83` wraps it via `ServiceBuilder` at Router scope so every route inherits the deadline. Runtime proof in `request_timeout_returns_408` requires bitcoind (graceful-skipped here). |
| 3 | Concurrent connection counts at Tor listener bounded | VERIFIED (static) | `coordinator/src/network/tor.rs:144` `Semaphore::new(max_concurrent_connections as usize)`; line 158-161 acquires permit BEFORE `stream_req.accept(...)` (Pitfall: acquire-after-accept defeats cap); line 180 `drop(permit)` on accept failure; line 197 `ConnectionGuard::new(permit)` and line 206 explicit `drop(guard)` after `serve_connection`. WR-01 ConnectionGuard wrapper protects against accidental refactor that would silently unbind the permit. |
| 4 | All limits operator-tunable via `coordinator.toml` | VERIFIED | `coordinator/src/config.rs:25-46` defines all four knobs (`rate_limit_info_per_min`, `rate_limit_writes_per_min`, `request_timeout_secs`, `max_concurrent_connections`) with `#[serde(default = ...)]`; `coordinator/src/config.rs:124-134` `load()` includes both `File::with_name("blindjoin")` and `Environment::with_prefix("BLINDJOIN").separator("__")` so TOML file values AND `BLINDJOIN__COORDINATOR__*` env-var overrides both reach the fields. |
| 5 | Per-peer throttling impossible on Tor — uses GlobalKeyExtractor | VERIFIED | `coordinator/src/api/middleware.rs:106,116` both bucket builders call `.key_extractor(GlobalKeyExtractor)` FIRST in the typestate chain. `grep -rn PeerIpKeyExtractor coordinator/src/` returns only a comment in middleware.rs:14 explaining why it must NOT be used. The Tor-safe extractor is mandatory: `PeerIpKeyExtractor` would `Err(UnableToExtractKey)` on every Tor request and surface as HTTP 500. |
| 6 | Rate-limit bodies match project JSON envelope `{"error":{"code":"RATE_LIMITED",...}}` | VERIFIED (static) | `coordinator/src/api/middleware.rs:156-194` `rate_limit_to_json` shapes 429 bodies with `code: "RATE_LIMITED"`, `message`, `round_id: null` — matches `handlers::api_error` envelope at handlers.rs:30-43. Attached to both buckets via `.error_handler(rate_limit_to_json)` at middleware.rs:124,125. Test asserts JSON envelope code at tests/integration/rate_limiting.rs:283-300 (runtime-validated only with bitcoind). |
| 7 | Read split: `/info` + `/round/tx` on reads bucket (60 rpm default) | VERIFIED | `coordinator/src/api/mod.rs:53,57` — both GET `/info` and GET `/round/tx` have `limits.reads_layer.clone()`. Writes bucket separately attached to 3 write routes (lines 54,55,56). |
| 8 | Config validation: rpm bounded 1..=60_000, max_concurrent_connections >= 1, request_timeout_secs >= 1 | VERIFIED | `coordinator/src/config.rs:157-185` `validate()` enforces all four constraints with `anyhow::ensure!` and actionable error messages (env-var names included). `coordinator/src/run.rs:46` calls `cfg.validate().context("Invalid coordinator configuration")?` ONCE at startup before any subsystem reads the config. This is the CR-01 + CR-02 fix that prevented the divide-by-zero panic and the silent semaphore deadlock. |
| 9 | ConnectionGuard pattern protects permit lifecycle from accidental refactor | VERIFIED | `coordinator/src/network/tor.rs:58-66` defines `struct ConnectionGuard { _permit: OwnedSemaphorePermit }` with a doc-block explaining the WR-01 contract; line 197 constructs the guard before `tokio::spawn`; line 206 explicit `drop(guard)` after `serve_connection`. A future "clean up unused variable" pass cannot strip this without also removing the struct field — load-bearing intent is now grep-able. |
| 10 | Sanitized Tor accept/close error logs (no PII leakage via Display chain) | VERIFIED | `coordinator/src/network/tor.rs:34-42` `client_error_kind(err) -> &'static str` maps each `ClientError` variant to a stable static tag; line 178 logs `error_kind = kind` instead of `error = %e`. Line 215 `tracing::debug!("HS connection closed with error")` carries no error payload. WR-05 fix — addresses CLAUDE.md PRIV-02 (no circuit/peer-side metadata in logs). |
| 11 | WR-03: tonic dropped from tower_governor default feature set | VERIFIED | `coordinator/Cargo.toml:43` `tower_governor = { version = "0.8", default-features = false, features = ["axum"] }`; `cargo tree -p coordinator | grep -i tonic` returns empty. The coordinator no longer ships unused gRPC code. |

**Score:** 11/11 truths verified (8 statically; 2 require bitcoind for end-to-end runtime confirmation — graceful-skipped in this environment; 1 deferred to future tor-mode harness per A4)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `coordinator/src/api/middleware.rs` | Factory module + GlobalKeyExtractor + RATE_LIMITED envelope + TimeoutLayer | VERIFIED | 217 lines; exports `RateLimitLayers`, `build_rate_limit_layers`, `build_timeout_layer`, private `rate_limit_to_json`; 2 `#[cfg(test)]` construction tests pass. |
| `coordinator/src/api/mod.rs` | Per-route GovernorLayer + ServiceBuilder Router-scope composition | VERIFIED | 86 lines; `let limits = middleware::build_rate_limit_layers(&config)` at line 51; 3× writes_layer + 2× reads_layer attached to routes 53-57; `ServiceBuilder::new().layer(RequestBodyLimitLayer).layer(TimeoutLayer)` at 79-83. |
| `coordinator/src/network/tor.rs` | Semaphore-gated accept loop + ConnectionGuard | VERIFIED | 221 lines; `serve_onion_service` has third param `max_concurrent_connections: u32` (line 71); `Semaphore::new` at 144; permit acquire BEFORE accept at 158-161; `drop(permit)` on accept failure at 180; `ConnectionGuard::new(permit)` at 197; explicit `drop(guard)` at 206. |
| `coordinator/src/config.rs` | 4 new knobs + validate() + env-var overlay | VERIFIED | 217 lines; CoordinatorSection has all 4 new fields with `#[serde(default = ...)]` attrs (25-46); 4 default-fns (58-72); `validate()` at 157-185 covers all 4 bounds; `load()` env-var overlay at 128-130 unchanged. |
| `coordinator/src/run.rs` | `cfg.validate()` call site + max_concurrent_connections threading + clearnet refusal | VERIFIED | 401 lines; `cfg.validate().context(..)?` at line 46; `max_concurrent_connections` captured at 266 and passed to `serve_onion_service` at 272; clearnet refusal (WR-04) at 296-305 (release-build bail unless `BLINDJOIN_ALLOW_CLEARNET=1`); clearnet warn at 306-310. |
| `tests/integration/rate_limiting.rs` | 429 + 408 runtime regression guard | VERIFIED (static + graceful-skip) | 583 lines; two `#[tokio::test]` fns (`info_endpoint_returns_429_when_flooded`, `request_timeout_returns_408`); both compile, both graceful-skip when bitcoind absent. WR-02 fix at 487-545 adds time-to-first-byte upper-bound assertion. |
| `tests/integration/mod.rs` | rate_limiting module registered | VERIFIED | 4 lines; `mod rate_limiting;` between `mod full_round;` and `mod round_bootstrap;`. |
| `coordinator/Cargo.toml` | tower_governor 0.8 (no tonic) + tower-http timeout feature | VERIFIED | Lines 36 (`tower-http` features `["limit", "timeout"]`), 43 (`tower_governor` with `default-features = false`, `features = ["axum"]`), 48-49 (direct deps for `governor`/`http` so type aliases are nameable). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `coordinator/src/api/mod.rs` (build_router_with_ban_list) | `coordinator/src/api/middleware.rs` (build_rate_limit_layers, build_timeout_layer) | factory call passing `&config` | WIRED | mod.rs:51 calls `build_rate_limit_layers(&config)`; mod.rs:82 calls `build_timeout_layer(&config)`. Both factories consumed. |
| `GovernorLayer` (both buckets) | `GlobalKeyExtractor` | `.key_extractor(GlobalKeyExtractor)` | WIRED | middleware.rs:106 (reads bucket), 116 (writes bucket). Typestate makes the requirement compiler-visible. |
| `coordinator/src/run.rs` (tor_mode branch) | `coordinator/src/network/tor.rs::serve_onion_service` | `tokio::spawn(serve_onion_service(app, addr_tx, max_concurrent_connections))` | WIRED | run.rs:272 — third positional arg present. |
| `coordinator/src/run.rs` (startup) | `coordinator/src/config.rs::CoordinatorConfig::validate` | `cfg.validate().context(..)?` | WIRED | run.rs:46. Validates rpm bounds, max_concurrent_connections >= 1, request_timeout_secs >= 1 BEFORE any subsystem reads the config. |
| `GovernorLayer` (both buckets) | `rate_limit_to_json` | `.error_handler(rate_limit_to_json)` | WIRED | middleware.rs:124,125 — both layers carry the custom handler so 429 bodies always match the project envelope. |
| `Tor accept loop` | `tokio::sync::Semaphore` | `Arc::clone(&conn_sem).acquire_owned().await` | WIRED | tor.rs:158-161; cap reads from function param threaded from run.rs which reads from `cfg.coordinator.max_concurrent_connections`. |
| `tests/integration/rate_limiting.rs` | `coordinator::run(cfg)` | `tokio::spawn(coordinator::run(cfg))` | WIRED | rate_limiting.rs:244,426 — both tests spawn the production run path (D-06 / T-06-02: no test-only backdoors). |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `build_rate_limit_layers` | `cfg.coordinator.rate_limit_info_per_min` / `rate_limit_writes_per_min` | `CoordinatorConfig::load` (TOML + BLINDJOIN__* env vars) | Yes — defaults 60/30; validate() enforces 1..=60_000 | FLOWING |
| `build_timeout_layer` | `cfg.coordinator.request_timeout_secs` | `CoordinatorConfig::load` | Yes — default 30; validate() enforces >= 1 | FLOWING |
| `serve_onion_service` | `max_concurrent_connections: u32` | run.rs:266 captures `cfg.coordinator.max_concurrent_connections` and passes it positionally | Yes — default 256; validate() enforces >= 1 | FLOWING |
| `rate_limit_to_json` | `wait_time: u64`, `headers: Option<HeaderMap>` | `GovernorError::TooManyRequests` payload populated by tower_governor when bucket is exhausted | Yes — body and `retry-after` header use the real wait_time computed by the governor | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Coordinator workspace builds clean | `cargo build -p coordinator --all-targets` | exit 0 in 0.78s | PASS |
| Clippy passes with -D warnings | `cargo clippy --all-targets -- -D warnings` | exit 0 (no warnings) | PASS |
| Coordinator lib tests pass | `cargo test -p coordinator --lib` | 58 passed, 0 failed (includes 2 `api::middleware::tests` construction proofs) | PASS |
| Rate-limit + timeout integration tests compile and are discoverable | `cargo test --test integration rate_limiting:: -- --include-ignored` | 2 tests discovered; both graceful-skipped because bitcoind is absent (`bitcoind not found`); exit 0 | PASS (graceful-skip — runtime proof requires bitcoind) |
| Integration test crate compiles | `cargo test --test integration --no-run` | exit 0 | PASS |
| tonic dropped from coordinator dependency graph (WR-03) | `cargo tree -p coordinator \| grep -i tonic` | empty | PASS |
| Middleware unit tests (factory construction proofs) | `cargo test -p coordinator --lib api::middleware` | 2 passed: `rate_limit_layers_construct_with_defaults`, `timeout_layer_constructs_with_defaults` | PASS |

### Probe Execution

No `scripts/*/tests/probe-*.sh` exist in this project. The phase's runtime probes are the `tests/integration/rate_limiting.rs` tests, which are exercised under Behavioral Spot-Checks above. Status: N/A (no probes declared by the phase).

### Requirements Coverage

Phase plans declare `requirements: []` (no formal requirement IDs). `.planning/REQUIREMENTS.md` does not exist. No requirement coverage to verify.

### Test Setup Audit (Step 7d)

| Helper | Constructs | Production analog | Risk | Disposition |
|--------|-----------|--------------------|------|-------------|
| `bootstrap_regtest_bitcoind` (tests/integration/rate_limiting.rs:96) | live regtest Bitcoin Core node via `corepc_node` | N/A — test infrastructure, not a production type | LOW | accepted fixture (mirrors `round_bootstrap.rs:59-89` verbatim) |
| `reserve_free_port` (tests/integration/rate_limiting.rs:83) | OS-assigned port via bind-port-0 + drop | N/A — test infrastructure | LOW | accepted fixture |
| `wait_http_ready` (tests/integration/rate_limiting.rs:136) | HTTP polling utility | N/A — test infrastructure | LOW | accepted fixture |
| `CoordinatorConfig` literal in 429 test (rate_limiting.rs:207-241) | Production `CoordinatorConfig` — same type production uses | `CoordinatorConfig::load()` at coordinator/src/config.rs:124 — operator-supplied TOML + env | LOW | accepted fixture — test constructs the same type production constructs; the four Phase 8 knobs are TIGHT (rate_limit_*_per_min: 3) but every other field uses production-realistic values. The production code path under test (`coordinator::run`) accepts this config exactly as it accepts an operator-supplied one. |

No HIGH-risk test setup. Tests exercise the production type via the production entry point (`coordinator::run`).

### Anti-Patterns Found

Grep audit on the 5 phase-modified files (`coordinator/src/api/middleware.rs`, `coordinator/src/api/mod.rs`, `coordinator/src/network/tor.rs`, `coordinator/src/config.rs`, `coordinator/src/run.rs`) and the new test file (`tests/integration/rate_limiting.rs`):

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| tests/integration/rate_limiting.rs | 39, 70 | `TODO(Phase-8 Q3, A4)` | Info | References formal follow-up work (A4 resolution + RESEARCH Q3 deferral). Tor-mode integration harness deferred — clearnet test infra cannot exercise the tor-only semaphore. Acceptable per debt-marker gate (formal follow-up reference present). |

Production files (`coordinator/src/`): zero `TODO|FIXME|XXX|TBD` markers introduced by Phase 8. Empty-data / hollow-prop / placeholder patterns: none detected.

### Human Verification Required

Three items require human verification (these do NOT invalidate the static + compile + graceful-skip evidence; they confirm runtime behavior under conditions only reproducible outside this verification environment):

#### 1. Runtime proof of HTTP 429 + Retry-After + RATE_LIMITED JSON envelope on /info under flood

**Test:** With bitcoind in PATH (or `BITCOIND_EXE` set), run:
`cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture --include-ignored`

**Expected:** Output includes the line `info_endpoint_returns_429_when_flooded PASSED: 429 + retry-after + JSON envelope (code=RATE_LIMITED) observed; Plan 02 D-02/D-03/A5 runtime proof complete.`

**Why human:** Verifier confirmed the test compiles, registers, and graceful-skips when bitcoind is absent. The end-to-end runtime proof (the actual 429 response, the `retry-after` header, the JSON body containing `code: "RATE_LIMITED"`) only fires when bitcoind is available — and bitcoind is absent in this verification environment. CI must run this test with bitcoind available before Phase 8 can be considered runtime-proven for D-02/D-03/A5.

#### 2. Runtime proof of HTTP 408 REQUEST_TIMEOUT on slow-body request

**Test:** With bitcoind in PATH (or `BITCOIND_EXE` set), run:
`cargo test --test integration rate_limiting::request_timeout_returns_408 -- --nocapture --include-ignored`

**Expected:** Output includes the line `request_timeout_returns_408 PASSED: HTTP 408 REQUEST_TIMEOUT observed within 5s of a request that paused mid-body for 3s against request_timeout_secs=1; Plan 02 D-04 runtime proof complete.` The WR-02 fix at rate_limiting.rs:487-545 additionally asserts time-to-first-byte is under 1750 ms (catches a regression where the layer would wait for full body completion).

**Why human:** Verifier confirmed the test compiles, registers, and graceful-skips when bitcoind is absent. The 408 emission, the timing within ~1s of the deadline (not 3s when the body pause ends), and the `Request Timeout` reason phrase only fire with bitcoind available. CI must run this test with bitcoind available before D-04 is runtime-proven.

#### 3. Tor connection-cap runtime behavior (deferred to future-phase Tor-mode harness)

**Test:** Open `max_concurrent_connections + 1 = 257` simultaneous `.onion` streams to a coordinator instance with default `max_concurrent_connections = 256`.

**Expected:** Exactly 256 streams are served concurrently; the 257th `acquire_owned().await` parks until an earlier connection finishes (the accept loop itself blocks at cap, so the Tor client sees no BEGIN ack for the 257th stream).

**Why human:** Per Plan 04's explicit deferral (TODO(Phase-8 Q3, A4) at tests/integration/rate_limiting.rs:70-74), clearnet test infrastructure cannot drive the Tor-only semaphore — Plan 03's cap attaches inside `serve_onion_service`'s arti accept loop, not inside `axum::serve`'s clearnet loop. Static coverage stands via Plan 03 grep audits (acquire-BEFORE-accept ordering, drop-on-failure release path, ConnectionGuard-mediated permit hold for connection lifetime, all verified in this report under Truth #3 and Key Link Verification). End-to-end runtime proof requires a Tor-mode integration harness deferred to a future phase.

### Gaps Summary

No gaps blocking the phase goal. All 11 must-haves are verified at the static / compile / unit-test level, including:

- The 5 ROADMAP goal clauses (rate limit on writes + 429+Retry-After; timeout + 408; bounded connection cap; operator-tunable knobs; Tor-safe GlobalKeyExtractor)
- The 6 plan-level must-haves (read/write bucket split; JSON RATE_LIMITED envelope; config validation; ConnectionGuard pattern; sanitized error logs; tonic dropped)

The code review (08-REVIEW.md) flagged 2 critical (CR-01 + CR-02) and 6 warning-level issues (WR-01..WR-06). All 8 items are marked `fixed` in the REVIEW frontmatter and confirmed in this verification:

- **CR-01 (rate_limit_*_per_min > 60_000 panic):** `coordinator/src/config.rs:160-171` validate() enforces 1..=60_000; coordinator/src/api/middleware.rs:78-87 keeps the assert as defense-in-depth.
- **CR-02 (max_concurrent_connections = 0 deadlock):** `coordinator/src/config.rs:172-177` validate() enforces >= 1; coordinator/src/network/tor.rs:79-84 keeps `anyhow::ensure!` as defense-in-depth.
- **WR-01 (load-bearing `_permit` binding):** Replaced with `ConnectionGuard` RAII struct at coordinator/src/network/tor.rs:58-66; explicit `drop(guard)` at 206.
- **WR-02 (408 test timing assumption):** Added upper-bound `time_to_first_byte < 1750ms` assertion at tests/integration/rate_limiting.rs:487-545.
- **WR-03 (tonic dependency):** `coordinator/Cargo.toml:43` `default-features = false, features = ["axum"]`; `cargo tree | grep tonic` empty.
- **WR-04 (clearnet warn-only policy):** `coordinator/src/run.rs:296-305` bails on release builds unless `BLINDJOIN_ALLOW_CLEARNET=1`.
- **WR-05 (PII via arti error Display):** `coordinator/src/network/tor.rs:34-42` `client_error_kind` maps each variant to a stable static tag; logs use `error_kind` field, not the error chain.
- **WR-06 (hardcoded ban_list.jsonl in tests):** `tests/integration/full_round.rs:58,1304` uses `tempfile::tempdir()`.

The remaining unmet items are **runtime proofs that require external infrastructure** (bitcoind for the integration tests; a Tor-mode harness for the connection cap). These are listed under "Human Verification Required" — they are not code gaps, but environment limitations of this verifier instance.

---

_Verified: 2026-05-26_
_Verifier: Claude (gsd-verifier)_
