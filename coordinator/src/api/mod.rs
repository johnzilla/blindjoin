use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{Router, routing::{get, post}};
use tower_http::limit::RequestBodyLimitLayer;
use crate::round::state::RoundState;
use crate::bitcoin::rpc::BitcoinRpc;
use crate::config::CoordinatorConfig;

pub mod handlers;
pub mod middleware;

#[derive(Clone)]
pub struct AppState {
    pub round: Arc<RwLock<RoundState>>,
    pub rpc: Arc<BitcoinRpc>,
    pub config: Arc<CoordinatorConfig>,
}

pub fn build_router(
    round: Arc<RwLock<RoundState>>,
    rpc: Arc<BitcoinRpc>,
    config: Arc<CoordinatorConfig>,
) -> Router {
    Router::new()
        .route("/info", get(handlers::get_info))
        .route("/round/input", post(handlers::post_input))
        .route("/round/output", post(handlers::post_output))
        .route("/round/sign", post(handlers::post_sign))
        .route("/round/tx", get(handlers::get_tx))
        .layer(RequestBodyLimitLayer::new(64 * 1024)) // 64KB max request body (T-04-02)
        .with_state(AppState { round, rpc, config })
}
