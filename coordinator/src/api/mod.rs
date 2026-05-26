use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::RwLock;
use axum::{Router, routing::{get, post}};
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;
use crate::round::state::RoundState;
use crate::round::blame::BanList;
use crate::bitcoin::rpc::BitcoinRpc;
use crate::config::CoordinatorConfig;

pub mod handlers;
pub mod middleware;

#[derive(Clone)]
pub struct AppState {
    pub round: Arc<RwLock<RoundState>>,
    pub rpc: Arc<BitcoinRpc>,
    pub config: Arc<CoordinatorConfig>,
    /// In-memory ban list. Survives across rounds. Checked before UTXO validation.
    pub ban_list: Arc<RwLock<BanList>>,
    /// Count of consecutive blame rounds for the current original round.
    /// Capped at 2 (BLAME-04, Pitfall 3): after cap, full abort without restart.
    /// AtomicU32 prevents TOCTOU on concurrent timer reads (T-02-10).
    #[allow(dead_code)]
    pub blame_round_count: Arc<AtomicU32>,
}

#[allow(dead_code)]
pub fn build_router(
    round: Arc<RwLock<RoundState>>,
    rpc: Arc<BitcoinRpc>,
    config: Arc<CoordinatorConfig>,
) -> Router {
    build_router_with_ban_list(round, rpc, config, Arc::new(RwLock::new(BanList::new())))
}

/// Build router with a pre-populated ban list (used at startup after loading ban file).
///
/// Phase 8 Plan 02: per-route `GovernorLayer` (rate-limit, D-02/D-03/D-05) attaches
/// to each `MethodRouter`; uniform `TimeoutLayer` (D-04) + existing `RequestBodyLimitLayer`
/// compose at Router scope via `ServiceBuilder` (Pitfall 3 — never mix bare `.layer()`
/// chaining with `ServiceBuilder` when there is >1 Router-scope layer).
pub fn build_router_with_ban_list(
    round: Arc<RwLock<RoundState>>,
    rpc: Arc<BitcoinRpc>,
    config: Arc<CoordinatorConfig>,
    ban_list: Arc<RwLock<BanList>>,
) -> Router {
    let blame_round_count = Arc::new(AtomicU32::new(0));
    let limits = middleware::build_rate_limit_layers(&config);
    Router::new()
        .route("/info", get(handlers::get_info).layer(limits.reads_layer.clone()))
        .route("/round/input", post(handlers::post_input).layer(limits.writes_layer.clone()))
        .route("/round/output", post(handlers::post_output).layer(limits.writes_layer.clone()))
        .route("/round/sign", post(handlers::post_sign).layer(limits.writes_layer.clone()))
        .route("/round/tx", get(handlers::get_tx).layer(limits.reads_layer.clone()))
        // Router-scope composition (Pitfall 3: ServiceBuilder is top-to-bottom = outside-in).
        //   1. RequestBodyLimitLayer (OUTERMOST) — existing 64 KB cap (T-04-02);
        //      short-circuits oversized Content-Length headers before any further
        //      processing.
        //   2. TimeoutLayer (INNER) — uniform request deadline (D-04 + A3;
        //      T-08-02-03 slow-loris mitigation).
        //
        // Order rationale (deviating from RESEARCH §"Order rationale" which proposed
        // TimeoutLayer outermost): `tower_http::timeout::Timeout::Service` requires
        // its INNER service's response body to implement `Default` (it constructs
        // an empty-body Response on elapsed deadline). `RequestBodyLimitLayer`'s
        // output type `ResponseBody<axum::body::Body>` does NOT implement `Default`,
        // so wrapping RequestBodyLimitLayer with TimeoutLayer fails to compile.
        // Reversing the order yields TimeoutLayer wrapping the route handlers whose
        // response body is `axum::body::Body` — which DOES implement `Default`.
        //
        // Functional impact: timeout still covers slow body reads because
        // `RequestBodyLimitLayer` reads the body INSIDE the handler future the
        // TimeoutLayer is wrapping (RESEARCH Pitfall 4: combined behavior is
        // "bounded slot consumption for any oversize body, however framed").
        // Sub-millisecond TimeoutLayer admission cost is unchanged.
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(64 * 1024))
                .layer(middleware::build_timeout_layer(&config)),
        )
        .with_state(AppState { round, rpc, config, ban_list, blame_round_count })
}
