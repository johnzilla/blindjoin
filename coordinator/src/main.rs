//! Coordinator binary — thin wrapper around `coordinator::run`.
//!
//! All startup logic lives in `coordinator::run::run` so integration tests can
//! exercise the same path as the production binary.

use tracing::error;

use coordinator::config::CoordinatorConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging — never log PII (PRIV-02)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("coordinator=info".parse().unwrap()),
        )
        .init();

    // Load configuration (D-11)
    let cfg = CoordinatorConfig::load().unwrap_or_else(|e| {
        error!(error = %e, "Config load failed — using defaults");
        CoordinatorConfig::with_defaults()
    });

    coordinator::run(cfg).await
}
