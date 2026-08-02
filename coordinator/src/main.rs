//! Coordinator binary — thin wrapper around `coordinator::run`.
//!
//! All startup logic lives in `coordinator::run::run` so integration tests can
//! exercise the same path as the production binary.

use anyhow::Context;

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

    // Load configuration (D-11). M1: a config error is a FATAL startup failure —
    // never silently fall back to defaults (signet + hardcoded creds + clearnet),
    // which would boot a mainnet-intended daemon as something else entirely.
    let cfg = CoordinatorConfig::load()
        .context("Failed to load coordinator configuration (refusing to boot defaults)")?;

    coordinator::run(cfg).await
}
