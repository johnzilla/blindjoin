use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::RwLock;
use axum::{Router, routing::{get, post}};
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
    pub blame_round_count: Arc<AtomicU32>,
}

pub fn build_router(
    round: Arc<RwLock<RoundState>>,
    rpc: Arc<BitcoinRpc>,
    config: Arc<CoordinatorConfig>,
) -> Router {
    build_router_with_ban_list(round, rpc, config, Arc::new(RwLock::new(BanList::new())))
}

/// Build router with a pre-populated ban list (used at startup after loading ban file).
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
