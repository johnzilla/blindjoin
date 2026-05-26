# Phase 8: Public-endpoint hardening - Pattern Map

**Mapped:** 2026-05-26
**Files analyzed:** 5 (4 modified + 1 new)
**Analogs found:** 5 / 5

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `coordinator/src/api/middleware.rs` (MODIFIED) | middleware factory | request-response | `coordinator/src/api/mod.rs:5,51` (RequestBodyLimitLayer construction) + `coordinator/src/api/handlers.rs:30-43` (`api_error` factory) | exact (factory functions returning composable axum layers) |
| `coordinator/src/api/mod.rs` (MODIFIED) | router wiring | request-response | `coordinator/src/api/mod.rs:45-52` (itself — same file is its own pattern) | self-reference |
| `coordinator/src/config.rs` (MODIFIED) | config | n/a (declarative) | `coordinator/src/config.rs:13-36` (`CoordinatorSection` + `default_ban_file_path`) | exact (same struct, same default-fn pattern) |
| `coordinator/src/network/tor.rs` (MODIFIED) | transport / accept-loop | streaming (HS streams) | `coordinator/src/network/tor.rs:75-101` (itself — current accept loop is the surface to retrofit) | self-reference (semaphore wraps the existing loop body) |
| `tests/integration/rate_limiting.rs` (NEW) | integration test | request-response | `tests/integration/round_bootstrap.rs` (full file) | exact (in-process `coordinator::run` spawn + bitcoind graceful-skip + reqwest poll) |

## Pattern Assignments

### `coordinator/src/api/middleware.rs` (factory functions for tower layers)

**Status:** Currently a 2-line comment stub (`coordinator/src/api/middleware.rs:1-2`). Becomes the home for `build_rate_limit_layers` and `build_timeout_layer`.

**Analog A — RequestBodyLimitLayer construction & application** (`coordinator/src/api/mod.rs:5, 45-52`):

Imports pattern (line 5):
```rust
use tower_http::limit::RequestBodyLimitLayer;
```

Router-scope `.layer()` application (lines 45-52):
```rust
Router::new()
    .route("/info", get(handlers::get_info))
    .route("/round/input", post(handlers::post_input))
    .route("/round/output", post(handlers::post_output))
    .route("/round/sign", post(handlers::post_sign))
    .route("/round/tx", get(handlers::get_tx))
    .layer(RequestBodyLimitLayer::new(64 * 1024)) // 64KB max request body (T-04-02)
    .with_state(AppState { round, rpc, config, ban_list, blame_round_count })
```

Pattern to copy: tower layer constructed with a `Layer::new(args)` constructor, then attached at the Router level via `.layer(...)`. For Phase 8, the factory functions in `middleware.rs` produce `GovernorLayer` (per-route, attached via `MethodRouter::layer`) and `TimeoutLayer` (Router-scope, attached via `Router::layer` inside `ServiceBuilder`).

**Analog B — JSON error envelope factory** (`coordinator/src/api/handlers.rs:29-43`):

```rust
/// Build a standard API error response.
fn api_error(
    status: StatusCode,
    code: &str,
    message: impl ToString,
    round_id: Option<&str>,
) -> (StatusCode, Json<Value>) {
    (status, Json(json!({
        "error": {
            "code": code,
            "message": message.to_string(),
            "round_id": round_id,
        }
    })))
}
```

Pattern to copy: when implementing the custom 429 body (CONTEXT D-06 discretion, RESEARCH Code Example 1), match this envelope shape exactly: `{"error":{"code":"RATE_LIMITED","message":"...","round_id":null}}`. The `tower_governor` `.error_handler()` closure should construct a `Response<Body>` whose JSON body mirrors this shape.

**Imports the new module needs (derived from RESEARCH Standard Stack):**
```rust
use std::sync::Arc;
use std::time::Duration;
use http::StatusCode;
use tower_governor::{
    governor::GovernorConfigBuilder,
    key_extractor::GlobalKeyExtractor,
    GovernorLayer,
};
use tower_http::timeout::TimeoutLayer;
use crate::config::CoordinatorConfig;
```

---

### `coordinator/src/api/mod.rs` (per-route layer wiring at line 51)

**Analog — self-reference, current Router builder** (lines 38-53):
```rust
pub fn build_router_with_ban_list(
    round: Arc<RwLock<RoundState>>,
    rpc: Arc<BitcoinRpc>,
    config: Arc<CoordinatorConfig>,
    ban_list: Arc<RwLock<BanList>>,
) -> Router {
    let blame_round_count = Arc::new(AtomicU32::new(0));
    Router::new()
        .route("/info", get(handlers::get_info))
        .route("/round/input", post(handlers::post_input))
        .route("/round/output", post(handlers::post_output))
        .route("/round/sign", post(handlers::post_sign))
        .route("/round/tx", get(handlers::get_tx))
        .layer(RequestBodyLimitLayer::new(64 * 1024)) // 64KB max request body (T-04-02)
        .with_state(AppState { round, rpc, config, ban_list, blame_round_count })
}
```

Pattern modification:
- Attach `GovernorLayer` per-route via `MethodRouter::layer`: `get(handlers::get_info).layer(limits.info_layer.clone())`
- Replace the bare `.layer(RequestBodyLimitLayer::new(...))` with a `ServiceBuilder::new().layer(TimeoutLayer::...).layer(RequestBodyLimitLayer::new(...))` per RESEARCH Pitfall 3 (ServiceBuilder is top-to-bottom, bare chained `.layer()` is bottom-to-top — never mix).
- `&config` (or a snapshot of the 4 new fields) must be threaded into `build_router_with_ban_list` so the factory can read the operator-tuned values. Current signature takes `Arc<CoordinatorConfig>`; the factory can read from it directly.

**Concrete shape** (per RESEARCH §"Wire-in at `coordinator/src/api/mod.rs:45-52`"):
```rust
let limits = middleware::build_rate_limit_layers(&config);

Router::new()
    .route("/info",          get(handlers::get_info).layer(limits.info_layer.clone()))
    .route("/round/input",   post(handlers::post_input).layer(limits.writes_layer.clone()))
    .route("/round/output",  post(handlers::post_output).layer(limits.writes_layer.clone()))
    .route("/round/sign",    post(handlers::post_sign).layer(limits.writes_layer.clone()))
    .route("/round/tx",      get(handlers::get_tx).layer(limits.tx_layer.clone()))
    .layer(
        ServiceBuilder::new()
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(config.coordinator.request_timeout_secs),
            ))
            .layer(RequestBodyLimitLayer::new(64 * 1024)) // existing — keep
    )
    .with_state(AppState { round, rpc, config, ban_list, blame_round_count })
```

---

### `coordinator/src/config.rs` (4 new fields in CoordinatorSection)

**Analog — existing field with default-fn pattern** (`coordinator/src/config.rs:13-36`):

Struct field with `#[serde(default = "fn_name")]` (lines 24-26):
```rust
/// Path to the append-only ban file. Defaults to "ban_list.jsonl".
/// BLAME-05: persists ban records across coordinator restarts.
#[serde(default = "default_ban_file_path")]
pub ban_file_path: String,
```

Default function (lines 34-36):
```rust
fn default_ban_file_path() -> String {
    "ban_list.jsonl".to_string()
}
```

**Pattern to copy for the 4 new fields (CONTEXT D-04):** add fields immediately before `tor_mode` (or after, at the end of the struct), each with a `#[serde(default = "default_xxx")]` annotation and a corresponding `fn default_xxx() -> T { N }` below the struct. The numeric defaults are u32/u64, so the default fns are trivial:

```rust
// In CoordinatorSection (add four fields):
#[serde(default = "default_rate_limit_info_per_min")]
pub rate_limit_info_per_min: u32,
#[serde(default = "default_rate_limit_writes_per_min")]
pub rate_limit_writes_per_min: u32,
#[serde(default = "default_request_timeout_secs")]
pub request_timeout_secs: u64,
#[serde(default = "default_max_concurrent_connections")]
pub max_concurrent_connections: u32,

// Below the struct (mirror lines 34-36):
fn default_rate_limit_info_per_min() -> u32 { 60 }
fn default_rate_limit_writes_per_min() -> u32 { 30 }
fn default_request_timeout_secs() -> u64 { 30 }
fn default_max_concurrent_connections() -> u32 { 256 }
```

**Analog — `with_defaults()` literal** (`coordinator/src/config.rs:101-124`):

Existing pattern shows every field is explicitly written in `with_defaults()`:
```rust
coordinator: CoordinatorSection {
    denomination_sats: 1_000_000,
    min_participants: 3,
    max_participants: 20,
    round_timeout_input_reg_secs: 60,
    // ... etc
    ban_file_path: "ban_list.jsonl".into(),
    tor_mode: false,
},
```

**Pattern to copy:** add the 4 new fields to the `with_defaults()` literal with the same default values used in the `default_xxx` helper fns (60 / 30 / 30 / 256). Existing tests construct `CoordinatorSection { ... }` directly (see `tests/integration/round_bootstrap.rs:117-129`), so all such constructions also need the 4 new fields added — otherwise the test stops compiling.

**Env-var convention** (`coordinator/src/config.rs:84-95`):
```rust
pub fn load() -> Result<Self, config::ConfigError> {
    Config::builder()
        .add_source(File::with_name("blindjoin").required(false))
        .add_source(
            Environment::with_prefix("BLINDJOIN")
                .separator("__")
                .try_parsing(true),
        )
        .build()?
        .try_deserialize()
}
```

**Pattern to copy:** zero changes needed in `load()`. The four new fields automatically inherit `BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN` (etc.) via the prefix + double-underscore separator. The `try_parsing(true)` already coerces env strings into u32/u64.

---

### `coordinator/src/network/tor.rs` (semaphore around the accept loop)

**Analog — self-reference, current accept loop** (`coordinator/src/network/tor.rs:75-101`):
```rust
while let Some(stream_req) = stream_requests.next().await {
    // T-05-01: Only accept BEGIN (HTTP) streams; non-BEGIN variants are rejected
    // by handle_rend_requests filter before reaching here — all items are StreamRequests
    // which already correspond to accepted BEGIN messages.

    // Accept the stream, sending a CONNECTED cell to the client.
    // Connected::new_empty() sends a CONNECTED cell with no address hint — correct for HS.
    let data_stream = match stream_req.accept(Connected::new_empty()).await {
        Ok(ds) => ds,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to accept HS stream");
            continue;
        }
    };

    let io = TokioIo::new(data_stream);
    // Wrap axum::Router (tower::Service) into a hyper-compatible service.
    let svc = TowerToHyperService::new(app.clone());
    tokio::spawn(async move {
        if let Err(e) = http1::Builder::new()
            .serve_connection(io, svc)
            .await
        {
            tracing::debug!(error = %e, "HS connection closed");
        }
    });
}
```

**Pattern to retrofit (per RESEARCH Pattern 3):**

1. Before the `while let` loop, construct the semaphore using the new config field:
   ```rust
   use std::sync::Arc;
   use tokio::sync::Semaphore;
   let conn_sem = Arc::new(Semaphore::new(max_concurrent_connections as usize));
   ```
   The `max_concurrent_connections` value must arrive via a new parameter to `serve_onion_service` (current signature: `pub async fn serve_onion_service(app, addr_tx) -> anyhow::Result<()>`). Threading it through requires a small change in `run.rs:259-264` (the `tokio::spawn(serve_onion_service(...))` call site).

2. Inside the loop, **before** `stream_req.accept(...).await`, acquire a permit (RESEARCH Anti-Pattern: "Acquiring the semaphore permit *after* `stream_req.accept()`"):
   ```rust
   let permit = Arc::clone(&conn_sem).acquire_owned().await
       .expect("semaphore never closed");
   ```
   Per the existing `while let` structure: the permit acquisition replaces the loop's implicit "always accept" — it parks here when at cap.

3. On accept failure (existing `Err(e) => { tracing::warn!; continue }` branch), explicitly `drop(permit)` before `continue` — otherwise the permit lingers until the next iteration's reassignment.

4. Inside the `tokio::spawn` body, move the permit by holding it for the connection lifetime (RESEARCH Pitfall 5):
   ```rust
   tokio::spawn(async move {
       let _permit = permit;   // hold for connection lifetime
       if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
           tracing::debug!(error = %e, "HS connection closed");
       }
   });
   ```

**Imports to add at top of `network/tor.rs`:**
```rust
use std::sync::Arc;
use tokio::sync::Semaphore;
```
(Neither is currently imported here — verified by reading `coordinator/src/network/tor.rs:1-19`.)

---

### `tests/integration/rate_limiting.rs` (NEW integration test)

**Analog — `tests/integration/round_bootstrap.rs` (full file is the scaffolding template).**

**Key copy points:**

1. **Module registration** (`tests/integration/mod.rs:1-3`):
   ```rust
   mod ban_list_persistence;
   mod full_round;
   mod round_bootstrap;
   ```
   Add a fourth line: `mod rate_limiting;` — otherwise the new file does not compile.

2. **Graceful-skip-if-no-bitcoind** (`round_bootstrap.rs:45-54`):
   ```rust
   let exe = match corepc_node::exe_path() {
       Ok(p) => p,
       Err(e) => {
           eprintln!(
               "bitcoind not found ({}), skipping run_bootstraps_round_into_input_reg",
               e
           );
           return;
       }
   };
   ```
   **Why required:** RESEARCH §"Test scaffolding" confirms `coordinator::run()` calls `startup_health_check` which requires reachable bitcoind. Rate-limit decision happens *before* the handler's phase check, but the coordinator won't start without bitcoind.

3. **Regtest bootstrap** (`round_bootstrap.rs:56-89`) — copy verbatim:
   ```rust
   let (rpc_url, rpc_user, rpc_pass) = tokio::task::spawn_blocking(move || {
       use bitcoin::Address;
       use corepc_node::{Conf, Node};

       let mut conf = Conf::default();
       conf.network = "regtest";

       let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");
       let cookie = node.params.get_cookie_values()
           .expect("read cookie file")
           .expect("parse cookie values");
       let rpc_url = node.rpc_url();
       let rpc_user = cookie.user.clone();
       let rpc_pass = cookie.password.clone();

       let mine_addr: Address = node.client.new_address().expect("get new address");
       node.client.generate_to_address(101, &mine_addr).expect("generate 101 blocks");

       let node_box = Box::new(node);
       Box::leak(node_box);

       (rpc_url, rpc_user, rpc_pass)
   })
   .await
   .expect("regtest bootstrap spawn_blocking panicked");
   ```

4. **`reserve_free_port` helper** (`round_bootstrap.rs:26-33`):
   ```rust
   async fn reserve_free_port() -> u16 {
       let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
           .await
           .expect("bind 127.0.0.1:0");
       let port = listener.local_addr().expect("local_addr").port();
       drop(listener);
       port
   }
   ```
   Duplicate this helper inside `rate_limiting.rs` (or factor out — but `round_bootstrap.rs` keeps it local, so follow that style for now).

5. **Config construction** (`round_bootstrap.rs:110-138`) — copy and override the 4 new fields with TIGHT limits per RESEARCH Example 2:
   ```rust
   coordinator: CoordinatorSection {
       denomination_sats: 100_000,
       min_participants: 3,
       max_participants: 3,
       round_timeout_input_reg_secs: 60,
       round_timeout_output_reg_secs: 60,
       round_timeout_signing_secs: 30,
       blame_ban_duration_secs: 3600,
       fee_rate_sat_per_vbyte: 1,
       listen_addr,
       ban_file_path,
       tor_mode: false,
       // NEW fields — TIGHT for fast breach:
       rate_limit_info_per_min: 3,
       rate_limit_writes_per_min: 3,
       request_timeout_secs: 30,
       max_concurrent_connections: 256,
   },
   ```

6. **Spawn pattern** (`round_bootstrap.rs:141-145`):
   ```rust
   let run_handle = tokio::spawn(async move {
       if let Err(e) = coordinator::run(cfg).await {
           eprintln!("coordinator::run returned Err: {e}");
       }
   });
   ```

7. **HTTP-ready poll** (`round_bootstrap.rs:148-170`):
   ```rust
   let http_client = reqwest::Client::new();
   let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
   loop {
       if tokio::time::Instant::now() > deadline {
           run_handle.abort();
           panic!("HTTP never came up within 10s");
       }
       tokio::time::sleep(Duration::from_millis(100)).await;
       let resp = match http_client.get(format!("{coordinator_url}/info")).send().await {
           Ok(r) if r.status().is_success() => r,
           _ => continue,
       };
       // ... server is up — proceed to flood loop
       break;
   }
   ```

8. **Test-end cleanup** (`round_bootstrap.rs:221`):
   ```rust
   run_handle.abort();
   ```
   Always abort the run_handle before returning from the test, otherwise the spawned coordinator + bitcoind lingers across tests.

**New code unique to this test (not from analog):** the flood loop and 429 assertion — see RESEARCH Code Example 2 (the `for _ in 0..20 { ... if status == 429 && contains "retry-after" }` block).

---

## Shared Patterns

### Tower layer construction
**Source:** `coordinator/src/api/mod.rs:5, 51`
**Apply to:** all new `middleware.rs` factories
```rust
use tower_http::limit::RequestBodyLimitLayer;
// ...
.layer(RequestBodyLimitLayer::new(64 * 1024))
```
Pattern: every layer is `LayerType::new(args)` (or builder-style); wired via `Router::layer` or `MethodRouter::layer`. New layers (`GovernorLayer`, `TimeoutLayer`) follow identical shape.

### JSON error envelope
**Source:** `coordinator/src/api/handlers.rs:30-43`
**Apply to:** custom 429 body if planner chooses JSON-envelope mode (CONTEXT D-06 discretion)
```rust
{"error": {"code": "RATE_LIMITED", "message": "...", "round_id": null}}
```
The `code` is always SCREAMING_SNAKE_CASE; `round_id` is `Option<String>` and `null` for non-round-scoped errors like rate-limit rejections.

### Config field + default-fn pairing
**Source:** `coordinator/src/config.rs:24-26 + 34-36`
**Apply to:** every new field in `CoordinatorSection`
```rust
#[serde(default = "default_FIELD")]
pub FIELD: T,
// ...
fn default_FIELD() -> T { LITERAL }
```
Plus: update `with_defaults()` literal at `coordinator/src/config.rs:109-121` with the matching literal value.

### Env-var overlay
**Source:** `coordinator/src/config.rs:84-95`
**Apply to:** automatic — no code change. New fields named `rate_limit_info_per_min` are automatically reachable as `BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN` through the existing `Environment::with_prefix("BLINDJOIN").separator("__")` source. Document the four new env-var names in the PR description / operator-facing docs only.

### Integration-test bootstrap
**Source:** `tests/integration/round_bootstrap.rs` (full file)
**Apply to:** any new test under `tests/integration/` that needs an in-process coordinator
- Graceful-skip on missing bitcoind (`corepc_node::exe_path()` → return)
- `tokio::task::spawn_blocking` for regtest `Node` start + 101-block mine + `Box::leak(node_box)`
- `reserve_free_port()` async helper (private to each test file)
- `tokio::spawn(coordinator::run(cfg))` + `run_handle.abort()` at the end
- Construct `CoordinatorConfig` literally (NOT via `CoordinatorConfig::with_defaults()` — the tests need per-field overrides)
- Remember to register the new file in `tests/integration/mod.rs`

### Tower-http feature flag bump
**Source:** `coordinator/Cargo.toml:36`
**Apply to:** the Cargo.toml edit for Phase 8
```toml
# BEFORE:
tower-http = { version = "0.6", features = ["limit"] }
# AFTER:
tower-http = { version = "0.6", features = ["limit", "timeout"] }
tower_governor = "0.8"
```

## No Analog Found

None. Every file in Phase 8 has a clean analog in the existing codebase.

## Metadata

**Analog search scope:**
- `coordinator/src/api/**` (mod.rs, middleware.rs, handlers.rs)
- `coordinator/src/config.rs`
- `coordinator/src/network/tor.rs`
- `coordinator/src/run.rs`
- `tests/integration/**` (all 4 files)
- `coordinator/Cargo.toml` (dependency surface)

**Files read (no re-reads):**
1. `coordinator/src/api/mod.rs` (full, 53 lines)
2. `coordinator/src/api/middleware.rs` (full, 2 lines)
3. `coordinator/src/api/handlers.rs` (lines 1-80 only — the `api_error` factory is on lines 29-43)
4. `coordinator/src/config.rs` (full, 125 lines)
5. `coordinator/src/network/tor.rs` (full, 104 lines)
6. `coordinator/src/run.rs` (full, 362 lines)
7. `tests/integration/round_bootstrap.rs` (full, 225 lines)
8. `tests/integration/mod.rs` (full, 3 lines)
9. `coordinator/Cargo.toml` (full, 57 lines)

**Pattern extraction date:** 2026-05-26
