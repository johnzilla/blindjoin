//! Library form of the liquidity bot — Pitfall 4 extraction (Phase 18 18-02).
//!
//! `main.rs` becomes a thin wrapper that parses env vars into `BotConfig` and
//! calls `run(config).await`. This allows integration tests to call the bot's
//! main loop directly without spawning a process.

pub mod strategy;

use anyhow::Result;
use std::path::PathBuf;

/// Configuration for a single bot invocation.
///
/// Each field maps to one or more env vars; see `liquidity-bot/src/main.rs`
/// for the env-var → BotConfig parsing logic.
pub struct BotConfig {
    pub coordinator_url: String,
    pub network: bitcoin::Network,
    pub enabled_types: Vec<shared::bip322::ScriptType>,
    pub counter_file: PathBuf,
    pub p2wpkh_tuple: Option<P2wpkhTuple>,
    pub p2tr_tuple: Option<DescriptorTuple>,
    pub p2sh_p2wpkh_tuple: Option<DescriptorTuple>,
}

/// Env-var credentials for the P2WPKH wallet path (from_wif).
pub struct P2wpkhTuple {
    pub utxo: String,    // txid:vout
    pub wif: String,
}

/// Env-var credentials for descriptor-based wallet paths (P2TR, P2SH-P2WPKH).
pub struct DescriptorTuple {
    pub utxo: String,           // txid:vout
    pub descriptor: String,     // tr(xprv/86'/...) or sh(wpkh(xprv/49'/...))
    pub utxo_address: String,   // bech32m or base58
}

/// Run one single-shot bot iteration: pick the rotated-to script type, build
/// the per-type wallet, drive register_input → register_output → verify_and_sign,
/// and on success bump the rotation counter.
///
/// Pitfall 4 mitigation: this lifts the previous `main.rs` body so that
/// `tests/integration/bot_rotation.rs` can drive 3 sequential in-process
/// runs without spawning the bot binary.
///
/// Task 1 stubs this as `Ok(())` — Task 2 replaces with the full body.
pub async fn run(config: BotConfig) -> Result<()> {
    use tracing::info;
    use shared::bip322::ScriptType;
    use client::wallet::BdkClientWallet;

    // Build RotationState from BotConfig.
    let rotation = strategy::RotationState::new(config.counter_file.clone(), config.enabled_types.clone())?;

    // Pick script type for this run (does NOT bump until success).
    let script_type = rotation.pick_script_type().await?;
    info!(
        script_type = ?script_type,
        counter_file = %config.counter_file.display(),
        "Bot starting single-shot round"
    );

    // Per-type wallet construction.
    let wallet = match script_type {
        ScriptType::P2wpkh => {
            let tuple = config.p2wpkh_tuple.as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "BLINDJOIN_BOT_P2WPKH_UTXO + BLINDJOIN_BOT_P2WPKH_WIF required when \
                     p2wpkh is in BLINDJOIN_BOT_SCRIPT_TYPES (or set legacy BLINDJOIN_UTXO \
                     + BLINDJOIN_UTXO_WIF)"
                ))?;
            BdkClientWallet::from_wif(&tuple.wif, &tuple.utxo, config.network)?
        }
        ScriptType::P2tr => {
            let tuple = config.p2tr_tuple.as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "BLINDJOIN_BOT_P2TR_UTXO + BLINDJOIN_BOT_P2TR_DESCRIPTOR + \
                     BLINDJOIN_BOT_P2TR_UTXO_ADDRESS required when p2tr is enabled"
                ))?;
            BdkClientWallet::from_descriptor(
                &tuple.descriptor,
                &tuple.utxo,
                &tuple.utxo_address,
                config.network,
                ScriptType::P2tr,
            )?
        }
        ScriptType::P2shP2wpkh => {
            let tuple = config.p2sh_p2wpkh_tuple.as_ref()
                .ok_or_else(|| anyhow::anyhow!(
                    "BLINDJOIN_BOT_P2SH_P2WPKH_UTXO + BLINDJOIN_BOT_P2SH_P2WPKH_DESCRIPTOR \
                     + BLINDJOIN_BOT_P2SH_P2WPKH_UTXO_ADDRESS required when p2sh-p2wpkh is enabled"
                ))?;
            BdkClientWallet::from_descriptor(
                &tuple.descriptor,
                &tuple.utxo,
                &tuple.utxo_address,
                config.network,
                ScriptType::P2shP2wpkh,
            )?
        }
    };

    // Synthetic CoordinatorInfo — narrows supported_script_types to one-element
    // vec matching the wallet's type (D-85 parallel; satisfies Phase 17 D-72 + D-76).
    let synthetic_info = client::discover::CoordinatorInfo {
        coordinator_url: String::new(), // not consumed by register_input
        capabilities: client::discover::CoordinatorCapabilities {
            record_version: "manual".to_string(),
            is_legacy: false,
            supported_script_types: vec![wallet.script_type()],
            output_script_type: wallet.script_type(),
        },
    };

    // Drive the round.
    let http = client::http::CoordinatorClient::new(config.coordinator_url.clone());
    let info_response = http
        .poll_until_phase("input_reg", 100, std::time::Duration::from_secs(600))
        .await?;

    info!("Input registration phase detected");
    let state = client::round::input::register_input(&http, &wallet, &info_response, &synthetic_info)
        .await
        .map_err(|e| anyhow::anyhow!("Input registration failed: {e}"))?;
    info!("Input registered");

    http.poll_until_phase("output_reg", 1000, tokio::time::Duration::from_secs(600))
        .await
        .map_err(|e| anyhow::anyhow!("Timeout waiting for output_reg phase: {e}"))?;
    info!("OUTPUT_REG phase detected");

    client::round::output::register_output(&http, &wallet, &state, &info_response)
        .await
        .map_err(|e| anyhow::anyhow!("Output registration failed: {e}"))?;
    info!("Output registered");

    http.poll_until_phase("signing", 1000, tokio::time::Duration::from_secs(600))
        .await
        .map_err(|e| anyhow::anyhow!("Timeout waiting for signing phase: {e}"))?;
    info!("SIGNING phase detected");

    // Floor of 1 preserves the bot's existing join/complete behavior; the
    // load-bearing protection for the bot is the C1 output-value + fee-theft
    // check (max_fee None → denomination/10 backstop). A bot-specific anonymity
    // floor + "don't join near-empty rounds" lower bound is separate hardening.
    client::round::sign::verify_and_sign(&http, &wallet, &state, 1, None)
        .await
        .map_err(|e| anyhow::anyhow!("Sign phase failed: {e}"))?;
    info!("Signed successfully");

    // Bump counter ONLY on successful round completion (D-94).
    rotation.bump_counter().await?;
    info!(
        script_type = ?script_type,
        "Bot round complete; counter advanced to next type"
    );

    Ok(())
}
