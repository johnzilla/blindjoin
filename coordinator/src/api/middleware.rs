// Phase 8 Plan 02: per-route rate-limit + uniform request-timeout middleware factory.
//
// Provides:
//   - `build_rate_limit_layers(cfg)` → `RateLimitLayers { reads_layer, writes_layer }`
//   - `build_timeout_layer(cfg)`     → `tower_http::timeout::TimeoutLayer`
//
// Design notes (decisions locked in 08-02-PLAN.md):
//   - D-02 + A2: write endpoints (`/round/input`, `/round/output`, `/round/sign`)
//     share ONE `GovernorConfig` (cloned three times via Arc inside `GovernorLayer`);
//     read endpoints (`/info`, `/round/tx`) share ONE `GovernorConfig`. Two separate
//     `Arc<GovernorConfig>` allocations — never one builder reused (per tower_governor
//     README pitfall: building the same config twice creates independent limiters).
//   - D-05 + RESEARCH Pitfall 1: ALWAYS call `.key_extractor(GlobalKeyExtractor)`
//     FIRST. The default `PeerIpKeyExtractor` panics on Tor `DataStream` (no peer
//     SocketAddr extension). The typestate transition makes this visible in types.
//   - D-04 + A1: `burst_size = rpm`, `per_millisecond = 60_000 / rpm` — a client can
//     briefly burn their full minute's budget then must wait. Documented in
//     `per_min_to_governor` helper.
//   - T-08-02-05: `.finish()` returns `None` for zero rpm / zero period; the
//     `.expect("...")` is the fail-fast that refuses to start the coordinator with
//     a misconfigured (rpm = 0) value.
//   - D-05 deviation: timeout uses `tower_http::timeout::TimeoutLayer`, NOT
//     `tower::timeout::TimeoutLayer` (latter returns `BoxError` requiring an extra
//     `HandleErrorLayer` for HTTP-response shaping).
//   - D-06 + A5: 429 body matches the project's `handlers::api_error` envelope:
//     `{"error":{"code":"RATE_LIMITED","message":...,"round_id":null}}`.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::response::Response;
use http::StatusCode;
use serde_json::json;
use tower_governor::{
    errors::GovernorError,
    governor::{GovernorConfig, GovernorConfigBuilder},
    key_extractor::GlobalKeyExtractor,
    GovernorLayer,
};
use tower_http::timeout::TimeoutLayer;

use crate::config::CoordinatorConfig;

/// Inner middleware type for `GovernorConfig`. We use the default `NoOpMiddleware`
/// (no `x-ratelimit-limit`/`x-ratelimit-remaining` headers on the success path) —
/// only the `retry-after` header on 429 responses, which `tower_governor` emits
/// automatically.
type NoOpMw = ::governor::middleware::NoOpMiddleware<::governor::clock::QuantaInstant>;

/// Two `GovernorLayer` instances — one per quota bucket — ready to clone onto
/// individual `MethodRouter`s in `api/mod.rs`.
///
/// Per D-02 + A2:
///   - `reads_layer` is shared by `/info` and `/round/tx`.
///   - `writes_layer` is shared by `/round/input`, `/round/output`, `/round/sign`.
///
/// Each layer wraps a SINGLE `Arc<GovernorConfig>` — cloning a layer is cheap
/// (Arc bump) and all clones share the same rate-limiter state.
pub struct RateLimitLayers {
    pub reads_layer: GovernorLayer<GlobalKeyExtractor, NoOpMw, Body>,
    pub writes_layer: GovernorLayer<GlobalKeyExtractor, NoOpMw, Body>,
}

/// Convert "N requests per minute" → governor's (period_ms, burst_size).
///
/// `burst_size = rpm` (A1 resolution): a client can briefly burn their full
/// minute's budget then must wait. `per_millisecond = 60_000 / rpm`: one token
/// replenishes every (60s / rpm). Example: 30 rpm → 2_000 ms per token, burst 30.
///
/// Bounds: rpm MUST be in `1..=60_000`. Above 60_000, integer division yields
/// `period_ms = 0` and `GovernorConfigBuilder::finish()` returns `None`,
/// triggering an opaque panic in `build_rate_limit_layers`. Phase 8 CR-01 fix:
/// `CoordinatorConfig::validate()` (called from `run::run`) is the primary
/// fence — this `assert!` is defense in depth so the message blames the
/// correct field if validation is ever bypassed (e.g. a direct call from a
/// future test or a custom embedding of the router).
fn per_min_to_governor(rpm: u32) -> (u64, u32) {
    assert!(
        (1..=60_000).contains(&rpm),
        "rate_limit_*_per_min must be in 1..=60_000; got {rpm}. Configure via \
         BLINDJOIN__COORDINATOR__RATE_LIMIT_{{INFO,WRITES}}_PER_MIN, or call \
         CoordinatorConfig::validate() before constructing the router.",
    );
    let period_ms = 60_000u64 / rpm as u64;
    (period_ms, rpm)
}

/// Build both `GovernorLayer`s — one per quota bucket — from the operator-tuned
/// rpm values in `CoordinatorSection`.
///
/// Two SEPARATE `Arc<GovernorConfig>` allocations (one per bucket). DO NOT
/// collapse to one builder — per `tower_governor` README pitfall #1, constructing
/// the same config twice creates independent limiters.
pub fn build_rate_limit_layers(cfg: &CoordinatorConfig) -> RateLimitLayers {
    let (reads_period_ms, reads_burst) =
        per_min_to_governor(cfg.coordinator.rate_limit_info_per_min);
    let (writes_period_ms, writes_burst) =
        per_min_to_governor(cfg.coordinator.rate_limit_writes_per_min);

    // Reads bucket. `.key_extractor(GlobalKeyExtractor)` FIRST so the typestate
    // makes the requirement visible (RESEARCH Pitfall 1). It returns a fresh
    // builder of type `GovernorConfigBuilder<GlobalKeyExtractor, NoOpMiddleware>`.
    let reads_cfg: Arc<GovernorConfig<GlobalKeyExtractor, NoOpMw>> = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(GlobalKeyExtractor)
            .per_millisecond(reads_period_ms)
            .burst_size(reads_burst)
            .finish()
            .expect("non-zero rate_limit_info_per_min and burst — check coordinator config"),
    );

    // Writes bucket — SEPARATE Arc allocation (independent limiter state).
    let writes_cfg: Arc<GovernorConfig<GlobalKeyExtractor, NoOpMw>> = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(GlobalKeyExtractor)
            .per_millisecond(writes_period_ms)
            .burst_size(writes_burst)
            .finish()
            .expect("non-zero rate_limit_writes_per_min and burst — check coordinator config"),
    );

    RateLimitLayers {
        reads_layer: GovernorLayer::new(reads_cfg).error_handler(rate_limit_to_json),
        writes_layer: GovernorLayer::new(writes_cfg).error_handler(rate_limit_to_json),
    }
}

/// Build the uniform Router-scope request timeout layer (D-04 + A3 — single
/// timeout across all routes; per-route override deferred per RESEARCH §"Open
/// Questions RESOLVED" Q2).
///
/// Crate path matters: this is `tower_http::timeout::TimeoutLayer`, NOT
/// `tower::timeout::TimeoutLayer` (D-05 deviation — the tower-core variant
/// returns `BoxError` and would require wrapping in `HandleErrorLayer`).
/// On elapsed deadline this returns an empty-body Response with status 408.
pub fn build_timeout_layer(cfg: &CoordinatorConfig) -> TimeoutLayer {
    TimeoutLayer::with_status_code(
        StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(cfg.coordinator.request_timeout_secs),
    )
}

/// Shape `GovernorError::TooManyRequests` into the project's standard JSON error
/// envelope (matches `handlers::api_error` at handlers.rs:30-43 — D-06 + A5).
///
/// `TooManyRequests` is the only practically reachable variant under
/// `GlobalKeyExtractor` (its `extract()` is infallible — `Result::Ok(())` always).
/// `UnableToExtractKey` and `Other` are unreachable but exhaustiveness requires
/// the catch-all branch.
///
/// PRIVACY (CLAUDE.md): no PII in the response body or headers — only `wait_time`
/// (anonymized aggregate) and the static error code. tower_governor emits its
/// own `retry-after` header on the way out; we duplicate it in our envelope to
/// keep the response self-describing.
fn rate_limit_to_json(err: GovernorError) -> Response<Body> {
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
                .header("retry-after", wait_time.to_string())
                .body(Body::from(body.to_string()))
                .expect("static response builder inputs are valid");
            if let Some(h) = headers {
                resp.headers_mut().extend(h);
            }
            resp
        }
        // Unreachable in practice with `GlobalKeyExtractor` (it never fails to
        // extract), but `match` requires exhaustive handling.
        _ => {
            let body = json!({
                "error": {
                    "code": "INTERNAL_ERROR",
                    "message": "rate-limit subsystem error",
                    "round_id": null,
                }
            });
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("static response builder inputs are valid")
        }
    }
}

#[cfg(test)]
mod tests {
    //! Runtime construction proofs. The point of these tests is that
    //! `.finish().expect(...)` and the `TimeoutLayer` constructor do not panic
    //! against `CoordinatorConfig::with_defaults()`. Catches typos like
    //! `per_milisecond` (one `l`) and arithmetic like `60_000 / 0` that
    //! `grep`/`cargo build`/`cargo clippy` cannot detect.

    use super::*;
    use crate::config::CoordinatorConfig;

    #[test]
    fn rate_limit_layers_construct_with_defaults() {
        let _ = build_rate_limit_layers(&CoordinatorConfig::with_defaults());
    }

    #[test]
    fn timeout_layer_constructs_with_defaults() {
        let _ = build_timeout_layer(&CoordinatorConfig::with_defaults());
    }
}
