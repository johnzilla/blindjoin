# Phase 8: Public-endpoint hardening — Research

**Researched:** 2026-05-25
**Domain:** axum 0.8 / tower middleware stacking for DoS-mitigation (rate-limit, timeout, connection cap) on a Tor-hosted HTTP coordinator
**Confidence:** HIGH (every recommended crate verified on crates.io within the last 9 months; tor-specific constraints confirmed by re-reading `coordinator/src/network/tor.rs`)

## Summary

Phase 8 adds three independent tower layers to the coordinator's HTTP API: per-route rate limiting (`tower_governor` 0.8.0), per-route timeouts (`tower_http::timeout::TimeoutLayer` 0.6.x — **not** `tower::timeout::TimeoutLayer`, which returns errors rather than HTTP responses), and a connection cap (a `tokio::sync::Semaphore` wrapped around the Tor accept loop in `coordinator/src/network/tor.rs` and around the clearnet accept path in `coordinator/src/run.rs` — **not** `tower::limit::ConcurrencyLimitLayer`, which queues rather than rejects and would not bound TCP/HS streams). All four `[coordinator]` config fields from D-04 fit cleanly into the existing `CoordinatorSection` and inherit the `BLINDJOIN__COORDINATOR__*` env-var pattern.

The single non-obvious decision: `tower_governor` defaults to `PeerIpKeyExtractor`, which reads a `SocketAddr` from request extensions inserted by `into_make_service_with_connect_info::<SocketAddr>()`. Tor `DataStream` carries no peer address (`coordinator/src/network/tor.rs:75-101`), so the rate limiter **must** be constructed with `.key_extractor(GlobalKeyExtractor)`. Building the config any other way will panic at the first request under tor_mode. D-05 already locks this in spirit ("global per-route rate limits"); this research confirms the crate-level mechanic.

**Primary recommendation:** Refactor `coordinator/src/api/middleware.rs` from a comment stub into `pub fn build_rate_limit_layers(cfg: &CoordinatorConfig) -> RateLimitLayers` returning a struct of three `GovernorLayer` instances (one per quota bucket: `info`, `writes`, `tx`). In `mod.rs:51`, wire each layer onto its specific `MethodRouter` via `.route("/path", post(handler).layer(layer))` — `axum 0.8` supports this. Stack `RequestBodyLimitLayer` and the new `TimeoutLayer` at the Router level via `ServiceBuilder` so they wrap every route, with rate-limit closest to the handler so a request rejected at the limiter consumes neither timeout slot nor body-buffer memory. For the connection cap, add a `tokio::sync::Semaphore` `acquire_owned()` **before** each `stream_requests.next()` accept in `tor.rs` and before each `listener.accept()` in `run.rs`'s clearnet branch.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Per-route rate limit (`/info`, `/round/input`, `/round/output`, `/round/sign`, `/round/tx`) | axum 0.8 Router (per-route layer) | — | tower_governor is a tower::Layer; per-route quotas attach to the MethodRouter, not to a connection. Read-vs-write differentiation (D-02) requires per-route configs. |
| Per-route request timeout | axum 0.8 Router (Router-level layer) | — | Uniform timeout per D-04 (`request_timeout_secs`, one value for all routes). Single `TimeoutLayer` at Router scope applies it cleanly. |
| Body size cap (existing, 64 KB) | axum 0.8 Router (Router-level layer) | — | Already established at `coordinator/src/api/mod.rs:51`. Stays at Router scope; new layers compose. |
| Max concurrent TCP/HS streams | Accept loop (transport boundary) | — | tower::ConcurrencyLimitLayer measures **in-flight requests**, not connections, and applies backpressure (queues) rather than refusing. A hard cap on accepted connections requires intervention at the accept loop — tokio Semaphore in `coordinator/src/network/tor.rs:75` and `coordinator/src/run.rs:279`. |
| 429 / 408 response shape | axum 0.8 (response IntoResponse) | — | Both layers return raw `Response<String>` bodies. To match the project's `{"error":{"code":..., "message":..., "round_id":...}}` JSON envelope (handlers.rs:30-43), use `GovernorLayer::error_handler` and a custom timeout response — both layers expose hooks for this. |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tower_governor` | `0.8.0` (Aug 14 2025) | Per-route rate limiting via the [`governor`](https://github.com/antifuchs/governor) crate (GCRA / token-bucket) | The de-facto tower-native rate limiter; depends explicitly on `axum = "0.8"` in its Cargo.toml. Wraps the same `governor` crate that powers actix-governor. Returns `429` + `retry-after` header by default. [VERIFIED: cargo search] |
| `tower-http` | `0.6.x` (already in tree, `0.6.11` is latest) | `TimeoutLayer` (per-route timeout that **returns an HTTP response**, not an error) + the existing `RequestBodyLimitLayer` | The project already depends on `tower-http = "0.6"`. `tower_http::timeout::TimeoutLayer` is the correct choice over `tower::timeout::TimeoutLayer` — the latter only wraps the inner service's error type with `BoxError`, requiring an additional `HandleErrorLayer` to convert to a response. The tower-http variant produces a clean `Response<empty body>` with `StatusCode::REQUEST_TIMEOUT` (408). [VERIFIED: existing Cargo.toml + tower-http docs] |
| `tokio` | `1.51` (already in workspace) | `tokio::sync::Semaphore` for connection-cap permit accounting | Standard concurrency primitive; `acquire_owned` returns an `OwnedSemaphorePermit` that's `Send`, suitable for moving into the spawned per-connection task. [VERIFIED: tokio docs] |

**Feature flags to enable on `tower-http`:** Add `"timeout"` to the existing `["limit"]` features list. The existing `RequestBodyLimitLayer` import already pulls `limit`; adding `timeout` enables `tower_http::timeout::TimeoutLayer`.

**`tower_governor` feature flag:** the `"axum"` feature is on by default; nothing extra to enable. Disable `"tracing"` only if you want to suppress its log output — leave default.

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tower::ServiceBuilder` | (already in tree via `tower = "0.5"`) | Compose `TimeoutLayer` + `RequestBodyLimitLayer` in declared-order top-to-bottom semantics | Use at the Router-scope `.layer()` call to make stacking order obvious. Per-route rate-limit layers stay attached to individual MethodRouters via `.route("/p", post(h).layer(rl))` because they have different configs. |
| `http::HeaderMap`, `http::StatusCode` | (transitive via axum) | Construct custom 429 / 408 JSON bodies if matching the project's error envelope | Only needed if D-decision "include JSON body matching project convention" is chosen (planner discretion per CONTEXT D-06). |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tower_governor` | `axum-governor` (the unrelated canmi21 crate, currently 2.0.1) | Newer (Aug 2025), independent implementation, but **not** the same project — and the rust ecosystem has consolidated around tower_governor (the benwis crate). The README of canmi21/axum-governor itself notes "it is not related to tower-governor." Pick the more-established option: tower_governor. |
| `tower_governor` | `axum_gcra` (Lantern-chat) | Also GCRA-based, per-route + per-key. Lower download counts. Less feature-overlap with what's already in the tower ecosystem. Reject. |
| `tower_http::timeout::TimeoutLayer` | `tower::timeout::TimeoutLayer` + `HandleErrorLayer` | The tower-core layer requires a `HandleErrorLayer` wrapping it to convert the `Elapsed` error into an HTTP response — extra code, easier to get wrong. tower-http's variant returns a response directly. Reject the tower-core layer. |
| `tokio::sync::Semaphore` (custom in accept loop) | `tower::limit::ConcurrencyLimitLayer` | `ConcurrencyLimitLayer` enforces in-flight **request** concurrency, not connection concurrency, and **queues** (returns `Poll::Pending`) rather than rejecting. For a max-concurrent-connections cap (D-04), this is the wrong primitive — Tor streams sit idle between HTTP requests and wouldn't be counted. The canonical pattern is a semaphore in the accept loop. |
| Custom in accept loop | `axum::serve::ListenerExt::limit_connections` | This is the *correct* primitive but is **unreleased** as of axum 0.8.9 (only landed in main on 2025-09-23; not yet on crates.io). Cannot pin to an unreleased version. Also: doesn't help the Tor path, which uses a custom accept loop, not `axum::serve`. Reject. |

**Installation (diff against current `coordinator/Cargo.toml`):**

```toml
[dependencies]
# existing entries unchanged ...
tower-http = { version = "0.6", features = ["limit", "timeout"] }   # add "timeout"
tower_governor = "0.8"                                              # new
```

(No new workspace-level entries needed — `tower`, `tokio`, `axum` are already in `[workspace.dependencies]`.)

**Version verification (executed during research):**

```bash
$ cargo search tower_governor --limit 3
tower_governor = "0.8.0"    # A rate-limiting middleware for Tower backed by the governor crate ...
```

axum 0.8.9 (latest, April 2026), tower-http 0.6.11 (latest, May 2026) — both verified on lib.rs. Current `coordinator/Cargo.toml` pins axum 0.8 and tower-http 0.6, so tower_governor 0.8.0 is the correct match (its own dep is `axum = "0.8"`).

## Package Legitimacy Audit

slopcheck was unavailable in the environment (pip install attempted and failed silently). Per protocol, packages are individually verified below by registry presence + author/source-repo + publication-date heuristics.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `tower_governor` | crates.io (cargo search confirmed) | ~9 mo (0.8.0 = Aug 2025; project history goes back to 2022) | Heavy (used throughout tower/axum ecosystem; lib.rs lists it as established) | `github.com/benwis/tower-governor` (active, public, MIT/Apache) | unavailable | Approved — well-established crate; author `benwis` is a recognized Rust author; the crate is the canonical port of `actix-governor`. |
| `tower-http` (already in tree) | crates.io | Years (workspace member of tower-rs) | Massive | `github.com/tower-rs/tower-http` | unavailable | Approved (already a dependency) — just enabling `"timeout"` feature. |
| `tokio` (already in tree) | crates.io | Years (LTS) | Massive | `github.com/tokio-rs/tokio` | unavailable | Approved (already a dependency). |

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none — but because slopcheck was unavailable, the planner SHOULD add a single `checkpoint:human-verify` task before the `cargo add tower_governor@0.8` step so the operator can eyeball the crate one more time. (Insignificant friction; consistent with CLAUDE.md's no-custom-crypto-and-no-magic-deps spirit.)

*If slopcheck becomes available before planning, the planner should re-run `slopcheck install tower_governor --json` and drop the checkpoint if `[OK]` returns.*

## Architecture Patterns

### System Architecture Diagram

```
                            ┌────────────────────────────────────────────────┐
   Tor client ──────HS────▶ │  arti onion service (network/tor.rs)           │
                            │    while let stream_req = next().await:        │
                            │      ▶ NEW: permit = sem.acquire_owned().await │
   clearnet client ──TCP──▶ │  TcpListener (run.rs clearnet branch)          │
                            │    loop { listener.accept().await:             │
                            │      ▶ NEW: permit = sem.acquire_owned().await │
                            └──────────────────┬─────────────────────────────┘
                                               │ DataStream / TcpStream
                                               ▼
                            ┌────────────────────────────────────────────────┐
                            │  hyper::http1::serve_connection(io, svc)       │
                            │    svc = TowerToHyperService(axum::Router)     │
                            │    Router layers (declared outside-in via      │
                            │    ServiceBuilder):                            │
                            │      1. TimeoutLayer       (tower_http)        │
                            │      2. RequestBodyLimit   (tower_http)        │
                            │      3. per-route GovernorLayer (tower_governor)│
                            │      4. handler (handlers.rs)                  │
                            └──────────────────┬─────────────────────────────┘
                                               │ on rate-limit:
                                               ▼  → 429 + Retry-After (governor)
                                               │ on timeout:
                                               ▼  → 408 (tower_http)
                                               │ on oversize body:
                                               ▼  → 413 (existing behavior)
                                               │ otherwise:
                                               ▼  → handler returns 2xx/4xx
```

**Order rationale (innermost first, request flows top-to-bottom on the way in):**

1. **`TimeoutLayer` outermost** — needs to start the timer the moment a request enters the stack, not after it's been admitted past rate-limit. (Note: this differs from my earlier suggestion that rate-limit should be outermost. After re-reading the tower-http source: `TimeoutLayer` only times the inner service's future, so wrapping it outside the rate-limit means the timer covers both the rate-limit decision time and the handler time. Since rate-limit decisions are sub-millisecond, this has no practical cost and the layer ordering is mostly about correctness of error mapping rather than efficiency.)
2. **`RequestBodyLimitLayer` next** — short-circuits oversized requests on Content-Length without invoking the handler.
3. **`GovernorLayer` per-route** — closest to handler; per-route quotas mean it can't be at Router scope unless all routes share one quota, which they don't (D-02 distinguishes reads from writes).
4. **Handler** — innermost.

**Wasted-work tradeoff:** because per-route layers attach to specific MethodRouters via `.route(p, h.layer(rl))`, the rate-limit is consulted only after axum's routing matches a path. For an attacker flooding a non-existent path, only the Router-level layers run — that's fine because 404 is cheap. For a real-route flood, the body-limit drops oversize bodies *before* the rate-limit runs, but a small malicious body (under 64 KB) will be parsed before rate-limit rejects. That's acceptable: the body-limit already keeps per-request cost bounded, and the rate-limit gates the *rate* of accepted requests, not the size.

### Recommended Project Structure (deltas only)

```
coordinator/src/
├── api/
│   ├── mod.rs           # MODIFIED: wire new layers; ~15 lines of insertion at line 51 area
│   ├── middleware.rs    # MODIFIED: stub becomes factory: pub fn build_rate_limit_layers + build_timeout_layer
│   └── handlers.rs      # UNCHANGED (handlers don't know about layers)
├── config.rs            # MODIFIED: +4 fields in CoordinatorSection + 4 defaults + 4 with_defaults entries
├── network/
│   └── tor.rs           # MODIFIED: wrap accept loop with Semaphore around lines 73-101
└── run.rs               # MODIFIED: wrap clearnet accept loop (currently uses axum::serve at line 283) with Semaphore — need to switch from axum::serve to a manual loop, OR use ListenerExt::tap_io creatively

tests/integration/
└── rate_limiting.rs     # NEW: integration test per D-06
```

**Note on `run.rs` clearnet branch:** the current code uses `axum::serve(listener, app)` at line 283 which has its own internal accept loop. To add per-connection semaphore admission control, either:
- (a) Replace `axum::serve` with a manual `loop { listener.accept() … hyper::serve_connection(…) }` mirroring the Tor path (consistent pattern, more code).
- (b) Use `axum::serve(listener.tap_io(|io| sem.try_acquire() …), app)` — but `tap_io` is a hook for *inspecting* `Io`, not for rejecting/queuing connections; this hack would leak permits.
- (c) Accept that the connection cap only enforces in the production-relevant `tor_mode = true` path (`coordinator/src/network/tor.rs`), and leave the clearnet path uncapped or only capped via documentation. This is acceptable per CONTEXT: "clearnet mode is dev/test only (production is `tor_mode = true`)."

**Recommendation for planner:** option (a) for consistency — clearnet and Tor both use the manual accept-loop + semaphore pattern. Or option (c) as a deliberate scope decision flagged in PLAN.md. Avoid option (b).

### Pattern 1: `tower_governor` per-route rate limit with `GlobalKeyExtractor`

**What:** Each route gets a `GovernorLayer` parameterized by `GlobalKeyExtractor` (no SocketAddr extension required, suitable for Tor where stream peer identity is absent).

**When to use:** All five routes in `coordinator/src/api/mod.rs`. Read endpoints (`/info`, `/round/tx`) share one quota; write endpoints (`/round/input`, `/round/output`, `/round/sign`) share another.

**Example (verified against tower_governor 0.8.0 source and README):**

```rust
// Source: https://github.com/benwis/tower-governor README + custom_key_bearer.rs example
use std::sync::Arc;
use std::time::Duration;
use tower_governor::{
    governor::GovernorConfigBuilder,
    key_extractor::GlobalKeyExtractor,
    GovernorLayer,
};

/// Convert "N requests per minute, burst = N" into governor's per_millisecond + burst_size.
/// Example: 30/min → replenish 1 token every 2_000 ms; burst_size = 30.
fn per_min_to_governor(rpm: u32) -> (u64, u32) {
    let period_ms = 60_000 / rpm as u64;   // 30 rpm → 2_000 ms per token
    (period_ms, rpm)                        // burst lets the round absorb its full minute of budget
}

pub struct RateLimitLayers {
    pub info_layer: GovernorLayer<GlobalKeyExtractor, governor::middleware::NoOpMiddleware>,
    pub writes_layer: GovernorLayer<GlobalKeyExtractor, governor::middleware::NoOpMiddleware>,
    pub tx_layer: GovernorLayer<GlobalKeyExtractor, governor::middleware::NoOpMiddleware>,
}

pub fn build_rate_limit_layers(cfg: &CoordinatorConfig) -> RateLimitLayers {
    let (info_period_ms, info_burst) = per_min_to_governor(cfg.coordinator.rate_limit_info_per_min);
    let (write_period_ms, write_burst) = per_min_to_governor(cfg.coordinator.rate_limit_writes_per_min);

    let info_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(GlobalKeyExtractor)
            .per_millisecond(info_period_ms)
            .burst_size(info_burst)
            .finish()
            .expect("non-zero period and burst — checked above"),
    );
    let writes_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(GlobalKeyExtractor)
            .per_millisecond(write_period_ms)
            .burst_size(write_burst)
            .finish()
            .expect("non-zero period and burst"),
    );
    // tx endpoint shares info's quota — both are reads (D-02: 60 rpm)
    let tx_conf = info_conf.clone();

    RateLimitLayers {
        info_layer: GovernorLayer::new(info_conf),
        writes_layer: GovernorLayer::new(writes_conf),
        tx_layer: GovernorLayer::new(tx_conf),
    }
}
```

**Wire-in at `coordinator/src/api/mod.rs:45-52` (verified against axum 0.8.9 MethodRouter::layer signature):**

```rust
let limits = middleware::build_rate_limit_layers(&config);

Router::new()
    .route("/info",          get(handlers::get_info).layer(limits.info_layer.clone()))
    .route("/round/input",   post(handlers::post_input).layer(limits.writes_layer.clone()))
    .route("/round/output",  post(handlers::post_output).layer(limits.writes_layer.clone()))
    .route("/round/sign",    post(handlers::post_sign).layer(limits.writes_layer.clone()))
    .route("/round/tx",      get(handlers::get_tx).layer(limits.tx_layer.clone()))
    // Router-scope layers wrap all routes (run from outside in, declared top-to-bottom):
    .layer(
        ServiceBuilder::new()
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(config.coordinator.request_timeout_secs),
            ))
            .layer(RequestBodyLimitLayer::new(64 * 1024))  // existing 64 KB cap
    )
    .with_state(AppState { round, rpc, config, ban_list, blame_round_count })
```

**Important pitfall (from tower_governor README "Common pitfalls" section 1):** *"Do not construct the same configuration multiple times, unless explicitly wanted! This will create an independent rate limiter for each configuration!"* — In the snippet above, `writes_layer` is `.clone()`d three times, sharing one `Arc<GovernorConfig>` for the three write endpoints. This is intentional per D-02: writes share a single quota. If the planner instead wants each write endpoint to have its own 30 rpm budget, build three separate configs.

### Pattern 2: `tower_http::timeout::TimeoutLayer` with custom status (Router-scope)

**What:** Returns a fast HTTP 408 response after `request_timeout_secs` elapses.

**When to use:** Once, at Router scope, wrapping all handlers.

**Example (verified against tower_http 0.6.11 docs):**

```rust
// Source: https://docs.rs/tower-http/0.6/tower_http/timeout/struct.TimeoutLayer.html
use tower_http::timeout::TimeoutLayer;
use http::StatusCode;
use std::time::Duration;

let timeout = TimeoutLayer::with_status_code(
    StatusCode::REQUEST_TIMEOUT,                                    // 408
    Duration::from_secs(cfg.coordinator.request_timeout_secs),      // default 30
);
```

**Body shape:** empty body (no JSON envelope). If the planner wants `{"error":{"code":"REQUEST_TIMEOUT", ...}}`, the tower-http TimeoutLayer does not expose a body customization — the planner would need to wrap it in a small custom layer or use a `from_fn` middleware that races a timeout future. Recommendation: ship with empty body for v1 (consistent with HTTP convention; clients can rely on status alone), upgrade later if needed.

### Pattern 3: Tokio semaphore for connection cap (Tor accept loop)

**What:** Bound the number of concurrent in-flight onion-service streams by acquiring a permit *before* `stream_requests.next().await` returns.

**When to use:** In `coordinator/src/network/tor.rs` around the `while let Some(stream_req) = stream_requests.next().await` loop at line 75. Also in the clearnet branch of `run.rs` if option (a) from the structure note above is chosen.

**Example (verified against tokio 1.51 docs and tokio Semaphore canonical example):**

```rust
// Source: https://docs.rs/tokio/1.51/tokio/sync/struct.Semaphore.html#method.acquire_owned
use std::sync::Arc;
use tokio::sync::Semaphore;

let conn_sem = Arc::new(Semaphore::new(cfg.coordinator.max_concurrent_connections as usize));

while let Some(stream_req) = stream_requests.next().await {
    // Acquire BEFORE accepting the next stream. acquire_owned() returns a
    // permit that's Send and can be moved into the spawned task; drop on
    // task completion releases the slot.
    let permit = Arc::clone(&conn_sem).acquire_owned().await
        .expect("semaphore never closed");

    let data_stream = match stream_req.accept(Connected::new_empty()).await {
        Ok(ds) => ds,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to accept HS stream");
            drop(permit);   // release on failed accept
            continue;
        }
    };

    let io = TokioIo::new(data_stream);
    let svc = TowerToHyperService::new(app.clone());
    tokio::spawn(async move {
        let _permit = permit;   // hold permit for connection lifetime
        if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
            tracing::debug!(error = %e, "HS connection closed");
        }
    });
}
```

**Behavior:** When the cap is reached, the `acquire_owned().await` parks the accept loop — Tor sees no new BEGIN cells accepted until a connection finishes. This is **the correct DoS-mitigation behavior**: refuse the load rather than fan out unbounded tasks. The `await` does not consume CPU.

**Trade-off:** when the cap is reached, no error is sent back to the would-be Tor client. The HS just doesn't accept the stream. This is exactly the same observable behavior a healthy coordinator under heavy load would show, which is desirable for DoS resistance.

### Anti-Patterns to Avoid

- **Using `PeerIpKeyExtractor` (the default) in Tor mode.** It calls `req.extensions().get::<ConnectInfo<SocketAddr>>()`, which is `None` for Tor `DataStream`. Result: every request fails with `GovernorError::UnableToExtractKey` → 500 Internal Server Error to every Tor client. Explicit `.key_extractor(GlobalKeyExtractor)` avoids this.
- **Using `tower::ConcurrencyLimitLayer` for max_concurrent_connections.** Queues rather than rejects (returns `Poll::Pending`); measures in-flight requests, not connections. Sounds right, isn't.
- **Using `tower::timeout::TimeoutLayer` (the tower-core one) without `HandleErrorLayer`.** Returns `BoxError` rather than a Response — the connection drops with no HTTP status sent. Tower-http's `TimeoutLayer` is the one with the Response semantics.
- **Constructing the same `GovernorConfig` multiple times.** Per README pitfall #1, each construction is a separate rate-limiter with independent state. Use one `Arc<GovernorConfig>` per quota bucket; clone the `Arc`, not the builder output.
- **Acquiring the semaphore permit *after* `stream_req.accept()`** (the [Andy Balaam blog](https://artificialworlds.net/blog/2021/01/08/limiting-the-number-of-open-sockets-in-a-tokio-based-tcp-listener/) shows the wrong pattern). Tokio's own Semaphore docs prescribe `acquire_owned().await` *before* accepting, so the accept loop itself parks when the cap is reached. Otherwise the loop accepts unbounded and the semaphore only limits per-connection *work* — not connection count.
- **Calling `.into_make_service_with_connect_info::<SocketAddr>()` in tor_mode.** Irrelevant — the Tor path doesn't use `axum::serve`. (No active risk; just noting that the standard tower_governor README example does this for clearnet and it doesn't apply here.)

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Token bucket / GCRA rate limiting | A custom HashMap + Instant timestamps + tokio::Mutex | `tower_governor` 0.8 wrapping `governor` 0.6 | GCRA correctness under burst behavior is non-obvious. The `governor` crate has been audited for fairness and overflow correctness for years. |
| Request timeout | A custom `tokio::select!` race between handler future and `tokio::time::sleep` in every handler | `tower_http::timeout::TimeoutLayer` | Layer composes once; per-handler `select!` repeats and is error-prone (the handler still continues running on the losing branch unless explicitly cancelled). |
| Connection cap | A custom AtomicUsize counter | `tokio::sync::Semaphore::acquire_owned` | Atomic counters race on the increment/check pair. Semaphore handles fair, async-safe permit accounting. |
| 429 Retry-After header | Hand-format the seconds value | tower_governor emits it automatically (`retry-after` is on by default, even without `.use_headers()`; confirmed in `src/governor.rs:214` of the crate) | Standard-compliant header math (computing `wait_time` from GCRA state) is non-trivial. |
| Per-route configurable limits | Hand-wire each limit in router setup | `build_rate_limit_layers(cfg)` factory in `middleware.rs` returning a struct | Single source of truth, testable in isolation, no router-setup divergence. |

**Key insight:** The entire Phase 8 surface is "stitch together three battle-tested tower middlewares with project-specific config plumbing." The work is in *configuration and integration*, not in implementing any new primitive. Per CLAUDE.md spirit ("No custom crypto"), the same applies to DoS mitigation: don't hand-roll, use the libraries.

## Common Pitfalls

### Pitfall 1: `GovernorConfigBuilder::default()` keeps `PeerIpKeyExtractor`

**What goes wrong:** Calling `.per_millisecond(2000).burst_size(30).finish()` without first calling `.key_extractor(GlobalKeyExtractor)` keeps the default `PeerIpKeyExtractor`, which panics on every Tor request because no `ConnectInfo<SocketAddr>` extension is set.

**Why it happens:** The builder pattern is fluent; it's easy to forget the one method that diverges from the README example.

**How to avoid:** In `build_rate_limit_layers`, **always** call `.key_extractor(GlobalKeyExtractor)` first, before any other config method, as the typestate transition (the method returns `GovernorConfigBuilder<GlobalKeyExtractor, M>`) makes the change visible in the function signature.

**Warning signs:** Integration test passes when using `axum::serve` directly (which auto-fills SocketAddr), then fails in production over Tor with `GovernorError::UnableToExtractKey` → HTTP 500. The integration test in `tests/integration/rate_limiting.rs` should explicitly exercise tor_mode = false **and verify** the layer construction wouldn't fail with global extractor — easiest is to just unit-test `build_rate_limit_layers()` returns valid configs and the integration test hits a real route until 429.

### Pitfall 2: `axum::serve` masks the connection cap on clearnet

**What goes wrong:** Adding a semaphore in `network/tor.rs` but leaving `axum::serve` on the clearnet path in `run.rs:283` means `max_concurrent_connections` only enforces in tor_mode. A reviewer or operator running clearnet tests against the config might assume the cap works there too.

**Why it happens:** The Tor path has a custom accept loop; the clearnet path doesn't. Different surgery sites.

**How to avoid:** Either (a) replace `axum::serve` with a manual loop mirroring the Tor pattern, or (b) document in `coordinator/CHANGELOG.md` / `PROJECT.md` that the connection cap is only enforced in `tor_mode = true` and explicitly state this is acceptable because clearnet is dev/test only.

**Warning signs:** None at runtime — the bug only surfaces under load testing. Catch it at planning time by explicitly addressing in PLAN.md which path the cap applies to.

### Pitfall 3: Layer ordering reversal between `ServiceBuilder` and bare `.layer()`

**What goes wrong:** `Router::new().layer(A).layer(B)` runs B-then-A (outside-in is bottom-to-top). `ServiceBuilder::new().layer(A).layer(B)` runs A-then-B (outside-in is top-to-bottom). The contradictory conventions cause real bugs.

**Why it happens:** Two different DSLs with opposite mental models.

**How to avoid:** Always use `ServiceBuilder` when composing more than one Router-scope layer, never chain bare `.layer()` calls. (`tower::ServiceBuilder` is already idiomatic in axum and is documented as the recommended way.) For per-route layers attached to a single MethodRouter, a single bare `.layer()` call is fine — only one layer there.

**Warning signs:** Test passes locally because the layer interaction is invisible (e.g., timeout never fires under low test load); production load flips the bug into view.

### Pitfall 4: `RequestBodyLimitLayer` does not block streaming bodies past the limit

**What goes wrong:** RequestBodyLimitLayer rejects requests whose `Content-Length` header exceeds the limit (returns 413 immediately). But for chunked / streaming bodies without Content-Length, it only enforces the limit **as the body is read**, allowing an attacker to start the request and waste a TCP slot before being rejected.

**Why it happens:** This is a tower-http design choice — it can't reject a body before reading any of it if Content-Length is missing.

**How to avoid:** Combine with `TimeoutLayer` (which the new phase adds anyway) — a slow streaming body trip-wires the timeout. The combined behavior is what we want: bounded slot consumption for any oversize body, however framed.

**Warning signs:** Profile-driven; not a correctness bug, but a DoS-resistance corner. Note in PLAN.md.

### Pitfall 5: `Semaphore` permit dropped before `tokio::spawn` enters the task

**What goes wrong:** Acquiring the permit, then `tokio::spawn(async move { ... })` without moving the permit into the spawned closure → permit drops immediately when the outer scope ends, and the cap is effectively `usize::MAX`.

**Why it happens:** Common Rust ownership trap when refactoring permit-acquisition logic.

**How to avoid:** Always move the `OwnedSemaphorePermit` into the spawned closure (`let _permit = permit;` at top of the spawn body) and let it drop naturally when the handler future completes.

**Warning signs:** No 429s observed under load; CPU at ceiling instead. Symptom is "the cap isn't doing anything." Easy to catch with a deliberate test that opens N+1 connections in parallel and asserts the N+1th blocks (the test scaffolding in `tests/integration/rate_limiting.rs` can do this with `reqwest` and `tokio::join!`).

## Code Examples

### Example 1: Custom 429 body matching the project's JSON envelope (optional, planner discretion per CONTEXT D-06)

```rust
// Source: pattern derived from tower_governor::governor::Governor::error_handler
// (https://docs.rs/tower_governor/0.8.0/tower_governor/governor/struct.Governor.html#method.error_handler)
// and matched to coordinator/src/api/handlers.rs:30-43 api_error() shape.

use http::{StatusCode, HeaderValue};
use serde_json::json;
use tower_governor::errors::GovernorError;

fn rate_limit_to_json(err: GovernorError) -> axum::response::Response {
    use axum::body::Body;
    use axum::response::Response;

    match err {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let body = json!({
                "error": {
                    "code": "RATE_LIMITED",
                    "message": format!("Too many requests; retry after {wait_time}s"),
                    "round_id": null,
                }
            });
            let mut resp = Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("content-type", "application/json")
                .header("retry-after", wait_time.to_string())   // duplicate of governor's default; harmless
                .body(Body::from(body.to_string()))
                .unwrap();
            if let Some(h) = headers {
                resp.headers_mut().extend(h);
            }
            resp
        }
        // Fallback for the other variants — shouldn't happen with GlobalKeyExtractor
        // since UnableToExtractKey can't fire (the extractor never fails).
        _ => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(json!({"error":{"code":"INTERNAL_ERROR","message":"rate-limit subsystem error","round_id":null}}).to_string()))
            .unwrap(),
    }
}

// Apply via .error_handler:
let writes_layer = GovernorLayer::new(writes_conf).error_handler(rate_limit_to_json);
```

### Example 2: Integration test skeleton for `tests/integration/rate_limiting.rs` (per D-06)

```rust
// Source: pattern derived from tests/integration/round_bootstrap.rs (in-process coordinator::run)
//
// Strategy:
//   1. Spawn coordinator::run() with VERY tight rate limits (e.g. 3 rpm) so the
//      test breach happens quickly without waiting for the default 30 rpm bucket.
//   2. Hammer /info (or any single route) faster than the limit.
//   3. Assert at least one 429 + Retry-After header in the response stream.
//   4. The rate-limit decision happens BEFORE round-state checks (because layers
//      wrap the handler), so this test does NOT need bitcoind for the limit-breach
//      itself — but `coordinator::run()` calls startup_health_check which DOES
//      need bitcoind. So the test still spins up corepc-node regtest (matching
//      round_bootstrap.rs's pattern). Graceful skip if bitcoind absent.

use std::time::Duration;

#[tokio::test]
async fn info_endpoint_returns_429_when_flooded() {
    use coordinator::config::{
        CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig,
    };

    // Graceful skip (matches round_bootstrap.rs pattern)
    let exe = match corepc_node::exe_path() {
        Ok(p) => p,
        Err(e) => { eprintln!("bitcoind not found ({e}), skip"); return; }
    };

    // ... (regtest bootstrap identical to round_bootstrap.rs lines 56-89) ...

    let port = reserve_free_port().await;  // reuse helper from round_bootstrap.rs
    let listen_addr = format!("127.0.0.1:{port}");

    let cfg = CoordinatorConfig {
        // ... (same as round_bootstrap.rs but with) ...
        coordinator: CoordinatorSection {
            // ... existing fields ...
            rate_limit_info_per_min: 3,        // TIGHT — test budget
            rate_limit_writes_per_min: 3,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            // ... rest unchanged ...
        },
        // ...
    };

    let _run = tokio::spawn(async move {
        let _ = coordinator::run(cfg).await;
    });

    // Wait for HTTP up (poll /info, succeed at least once before flooding)
    let http = reqwest::Client::new();
    let url = format!("http://{listen_addr}/info");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if tokio::time::Instant::now() > deadline { panic!("never came up"); }
        if http.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false) { break; }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Flood: 20 sequential requests; with burst=3, replenish every 20s,
    // we should see ~17 responses with status 429 + retry-after header.
    let mut saw_429_with_retry_after = false;
    for _ in 0..20 {
        let resp = http.get(&url).send().await.expect("send");
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if resp.headers().contains_key("retry-after") {
                saw_429_with_retry_after = true;
                break;
            }
        }
    }
    assert!(saw_429_with_retry_after, "expected at least one 429 with retry-after header");
}
```

**Note on test scaffolding:** the rate-limit test can use the in-process `coordinator::run()` path (consistent with `round_bootstrap.rs`). It does still need bitcoind because `run()` calls `startup_health_check`. There is no need to stub the round-state check — the rate-limit layer wraps the handler, so 429 is returned *before* the handler's phase check runs.

## Project Constraints (from CLAUDE.md)

The following CLAUDE.md directives apply to Phase 8 work; planner must honor:

- **No custom crypto / no custom security primitives.** Phase 8 satisfies this by selecting tower_governor + tower_http + tokio Semaphore — every primitive is library-provided, no hand-rolled rate-limit math, no hand-rolled timeout, no hand-rolled connection counter.
- **Tor-native in production.** D-05 already selects layers that work over the existing arti onion-service path. No bypass needed.
- **No PII logging.** Rate-limit rejections must not log Tor stream identifiers or any client-identifying info (in fact tor_mode has none to log). Counter / wait_time / route logging is fine — these are anonymized aggregates. Confirm during plan-check.
- **MIT licensed.** All recommended crates (tower_governor, tower-http, tokio) are MIT/Apache-2.0 dual-licensed. Compatible.
- **GSD workflow enforcement.** All edits must come through the GSD phase loop. Standard.
- **Stack stability (from CLAUDE.md ## Technology Stack):** axum 0.8.x, tower 0.5.x, tower-http 0.6.x are all approved in the recommended stack. tower_governor is a new addition consistent with "tower-native" guidance — no framework introduction, just one new tower middleware crate.

## Runtime State Inventory

This phase is **not a rename/refactor/migration** — it is a feature add. The Runtime State Inventory section is not applicable (per execution_flow Step 2.5 trigger criteria). For completeness:

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | None — no on-disk state introduced; rate-limit state is in-memory only (governor `RateLimiter`) and resets on coordinator restart, which is correct behavior for DoS mitigation | None |
| Live service config | None | None |
| OS-registered state | None | None |
| Secrets/env vars | New env vars `BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN`, `..._WRITES_PER_MIN`, `..._REQUEST_TIMEOUT_SECS`, `..._MAX_CONCURRENT_CONNECTIONS` are inherited automatically from CoordinatorSection field additions per the existing convention | Update `blindjoin.toml.example` (if exists; verify during planning) and document in operator-facing config table |
| Build artifacts | None | Standard `cargo build` after `Cargo.toml` edit |

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain (cargo, rustc) | Build | ✓ (project already builds) | n/a | — |
| crates.io network access | `cargo add tower_governor` | ✓ (assumed; project routinely fetches crates) | n/a | Vendor via `cargo vendor` if planner needs offline-capable build |
| `bitcoind` (regtest) | Integration test `rate_limiting.rs` startup_health_check | conditional (graceful skip pattern already established in `round_bootstrap.rs`) | corepc-node `0.10` (already in dev-deps) | Skip test gracefully — same pattern as existing tests |
| `tor` daemon | Not required (production uses arti embedded; test uses clearnet path) | n/a | n/a | n/a |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `bitcoind` for integration test, but pattern is already in place — `corepc_node::exe_path()` returns Err → `eprintln + return`. No new work for planner.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hand-rolled `tower::limit::RateLimitLayer` (rate-only, no per-key) for axum | `tower_governor` (per-key, GCRA-based) | tower_governor 0.1 → 0.8 (2022–2025); now the canonical pick | Use tower_governor; don't try to compose tower::limit primitives for this |
| `axum::middleware::from_fn` with custom timeout via `tokio::select!` | `tower_http::timeout::TimeoutLayer` (response-shaped, not error-shaped) | tower-http 0.4+ | Use the layer; from_fn cancellation semantics are subtler |
| `tower::limit::ConcurrencyLimitLayer` for connection caps | `tokio::sync::Semaphore` in accept loop (or unreleased `ListenerExt::limit_connections`) | Established pattern; axum maintainers explicit on discussion #2561: "Not currently" built-in | Use the semaphore until axum's official PR lands |

**Deprecated/outdated (in the stack but NOT for this phase):**
- `bitcoincore-rpc` crate (per CLAUDE.md) — not relevant to Phase 8.
- `tower::timeout::TimeoutLayer` for HTTP — works but requires HandleErrorLayer; tower-http variant supersedes it for HTTP use cases.

## Assumptions Log

> All claims tagged `[ASSUMED]` in this research. Discuss-phase / plan-checker should confirm these with the user before execution.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | "Burst size = rate_per_min" is the right semantic — i.e., a client can briefly consume their full minute's budget then must wait. Alternative: burst_size = 1 means strict per-2s spacing with no burst tolerance. | Standard Stack / Code Examples Pattern 1 | Misconfigured budget — too forgiving (DoS gets one extra full minute before blocking) or too strict (legitimate clients hit 429 from minor traffic bursts). Discuss-phase or planner should decide explicitly: "burst = N or burst = 1?". |
| A2 | Routes share quota buckets per D-02 (reads share one, writes share one). The CONTEXT decision lists each route's per-minute number separately, which could be read as either "one bucket per route" or "shared bucket per category." | Standard Stack / Code Examples Pattern 1 | If per-route is intended but shared is implemented, an attacker can flood `/round/input` to exhaust the writes bucket and lock out `/round/output` and `/round/sign` simultaneously. Recommend planner clarify with one line in PLAN.md. |
| A3 | Operator wants a *uniform* `request_timeout_secs` across all routes (D-04 declares only one field). Some endpoints (`/round/tx` PSBT construction) may legitimately take longer than `/info`. | Architectural Responsibility Map / Pattern 2 | A 30s default might be too tight for `/round/tx` under heavy participant counts. Mitigation: pick 30s as a safe default for the v1 of this phase; allow per-route override in a follow-up if profiling shows it matters. Note in PLAN.md. |
| A4 | The clearnet `axum::serve(listener, app)` path is acceptable to leave UNCAPPED for connections (option (c) in structure note), OR the planner converts it to manual accept loop (option (a)). CONTEXT notes clearnet is dev/test only, which supports (c). | Architecture Patterns / Recommended Project Structure | If operators run clearnet in production despite the warning, max_concurrent_connections silently fails to enforce. Recommend planner pick (a) for defense in depth, or (c) with a `tracing::warn!` at startup that the cap is tor-only. |
| A5 | JSON-envelope 429 body is desirable (matching project convention) — but plain-text `Too Many Requests! Wait for Ns` would also satisfy D-03 (which only requires 429 + Retry-After). CONTEXT D-06 explicitly leaves body shape to Claude's discretion. | Code Examples Example 1 | Planner choice. JSON envelope = ~30 lines additional code (custom `error_handler`); plain text = zero extra code. Recommend JSON for consistency. |

## Open Questions (RESOLVED)

1. **Connection cap target: connections or concurrent requests?**
   - What we know: D-04 says `max_concurrent_connections` (worded as connections). The natural fit is per-stream semaphore in the accept loop, which is what this research recommends.
   - What's unclear: If the operator actually wants "max concurrent in-flight HTTP requests across all connections" (looser bound — connections can be idle), a `tower::ConcurrencyLimitLayer` at the Router root would be the right tool, but with its queueing behavior, that's typically not what "max concurrent connections" reviewers mean.
   - Recommendation: ship as connection cap (semaphore in accept loop). The wording in D-04 supports this directly. If the planner wants both — bounded connections AND bounded concurrent requests — these are independent layers and can both be added.
   - **RESOLVED:** ship as connection cap (semaphore in accept loop) per Plan 03. CONTEXT D-04 wording `max_concurrent_connections` is taken at face value. No `ConcurrencyLimitLayer` added — operator can request a separate concurrent-request cap in a follow-up phase if profiling shows it matters.

2. **Per-route timeout vs uniform timeout?**
   - What we know: D-04 declares a single `request_timeout_secs`. Single TimeoutLayer at Router scope is correct under that declaration.
   - What's unclear: If `/round/tx` PSBT construction at max_participants = 20 sometimes exceeds 30 s (the default), this clamp causes spurious 408s.
   - Recommendation: ship single uniform timeout per D-04 strictly; add per-route override as a follow-up if profiling shows it matters. Note in PLAN.md as a deliberate "watch this" item.
   - **RESOLVED:** ship single uniform `tower_http::timeout::TimeoutLayer::with_status_code(REQUEST_TIMEOUT, Duration::from_secs(request_timeout_secs))` at Router scope per Plan 02 (A3). Per-route override deferred; PLAN.md / SUMMARY records the `/round/tx` watch-this concern.

3. **Should the integration test exercise the connection-cap?**
   - What we know: D-06 says "hammer one write endpoint past the configured limit; assert 429 + Retry-After." Connection-cap is a separate axis.
   - What's unclear: Whether the same integration-test file should also test the semaphore (open `max_concurrent_connections + 1` parallel reqwest connections, assert the +1th blocks).
   - Recommendation: yes — write the connection-cap test alongside the rate-limit test in the same file. Use `tokio::join!` of N+1 long-running connection futures and assert one of them is parked (or use `tokio::time::timeout` on the parked future). Minor scope add but high signal value. Planner discretion.
   - **RESOLVED:** 429+Retry-After test mandatory (Plan 04 Task 1 `info_endpoint_returns_429_when_flooded`); 408 timeout test mandatory (Plan 04 Task 1 `request_timeout_returns_408`, per the revision making the OR-skip clause non-optional); connection-cap runtime test DEFERRED per A4 — clearnet-only test infra cannot exercise the tor-only semaphore (Plan 03 attaches the cap inside the arti accept loop). Coverage stands via Plan 03 grep audits; a TODO comment inside `tests/integration/rate_limiting.rs` documents the deferral.

## Sources

### Primary (HIGH confidence)
- `coordinator/src/api/mod.rs`, `middleware.rs`, `handlers.rs`, `config.rs`, `network/tor.rs`, `run.rs`, `tests/integration/round_bootstrap.rs`, `tests/integration/full_round.rs`, `Cargo.toml`, `coordinator/Cargo.toml` — read in full from local checkout
- `coordinator/Cargo.toml` confirmed via `cat` — axum 0.8, tower 0.5, tower-http 0.6 already present
- `cargo search tower_governor --limit 3` — confirmed tower_governor 0.8.0 on registry
- `.planning/phases/08-public-endpoint-hardening/08-CONTEXT.md` — locked decisions D-01..D-06
- `.planning/BACKLOG.md` §B-01 — original scope and deferral rationale
- `https://github.com/benwis/tower-governor/blob/main/README.md` — fetched via `curl` (raw.githubusercontent.com), full API surface including common pitfalls
- `https://raw.githubusercontent.com/benwis/tower-governor/main/examples/src/basic.rs` and `custom_key_bearer.rs` — fetched verbatim
- `https://raw.githubusercontent.com/benwis/tower-governor/main/src/errors.rs` and `src/governor.rs` (lines 1-250) — verified retry-after default + 429 status mapping
- `https://docs.rs/tower_governor/0.8.0/tower_governor/governor/struct.GovernorConfigBuilder.html` — verified per_second / per_millisecond / burst_size / key_extractor signatures
- `https://docs.rs/tower_governor/0.8.0/tower_governor/key_extractor/struct.GlobalKeyExtractor.html` — confirmed `()` key type and no SocketAddr requirement
- `https://docs.rs/tower-http/latest/tower_http/timeout/struct.TimeoutLayer.html` — confirmed `with_status_code` API and default 408
- `https://docs.rs/axum/0.8.9/axum/routing/method_routing/struct.MethodRouter.html` — confirmed `.layer()` on `MethodRouter` is supported in 0.8

### Secondary (MEDIUM confidence)
- `https://github.com/tokio-rs/axum/discussions/2561` — community + maintainer consensus that axum lacks built-in connection limiting; recommends custom accept-loop pattern
- `https://github.com/tokio-rs/axum/pull/3489` — `ListenerExt::limit_connections` PR merged 2025-09-23 but **not yet released** as of axum 0.8.9 (verified via changelog read of `tokio-rs/axum/blob/main/axum/CHANGELOG.md`)
- `https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html` — canonical pattern: `acquire_owned().await` BEFORE accept; permit moves into spawned task
- `https://docs.rs/axum/0.8/axum/middleware/index.html` — confirmed bottom-to-top execution order for stacked bare `.layer()` calls; ServiceBuilder is top-to-bottom; recommends ServiceBuilder for clarity
- `lib.rs` listings for `axum` (0.8.9, April 2026), `tower-http` (0.6.11, May 2026), `tower_governor` (0.8.0, August 2025) — confirmed publication dates

### Tertiary (LOW confidence — for context only, not load-bearing)
- `https://artificialworlds.net/blog/2021/01/08/...` (Andy Balaam) — older blog post showing INCORRECT semaphore-acquire-after-spawn pattern; cited only to flag it as an anti-pattern
- `https://aarambhdevhub.medium.com/i-built-a-complete-axum-0-8-203061e27de5` — general axum 0.8 course material; corroborative only

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — every recommended crate verified by registry presence + source repo + recent publish date; tower_governor's `GlobalKeyExtractor` confirmed via doc + source-code reading; tower-http `TimeoutLayer` API verified.
- Architecture: **HIGH** — read the existing `tor.rs` accept loop in full; confirmed `DataStream` has no peer-address surface; confirmed semaphore is the canonical primitive.
- Pitfalls: **HIGH** — pitfalls 1, 4, 5 are general Rust/tower traps; pitfall 2 is project-specific (clearnet vs Tor path divergence) and is verified by reading both code paths; pitfall 3 is from official axum docs.
- Test scaffolding: **HIGH** — read both existing integration tests and confirmed the in-process `coordinator::run` pattern can be reused; rate-limit decision happens before round-state check (verified by tracing layer composition: rate-limit wraps handler).
- Assumptions log: **MEDIUM** — five items the planner / discuss-phase should confirm with the user (A1 burst semantics, A2 quota grouping, A3 uniform timeout, A4 clearnet cap scope, A5 JSON body shape).

**Research date:** 2026-05-25
**Valid until:** 2026-06-25 (30 days — stack is stable; tower-governor doesn't release often, axum 0.8 series is stable through April 2026)
