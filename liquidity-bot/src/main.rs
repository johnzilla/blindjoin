//! Liquidity bot binary — single-shot runner.
//!
//! This file is a thin wrapper: it parses env vars into BotConfig and calls
//! `liquidity_bot::run(config).await`. The main loop lives in lib.rs so
//! integration tests can drive 3 sequential in-process runs without spawning
//! a process (Pitfall 4 mitigation per Phase 18 18-02).
//!
//! Env vars (Phase 18 multi-script + rotation):
//!   BLINDJOIN_BOT_SCRIPT_TYPES         — CSV (kebab-case); default "p2wpkh" (v1.3 compat)
//!   BLINDJOIN_BOT_COUNTER_FILE         — counter path; default /app/data/bot_round_counter
//!   BLINDJOIN_BOT_P2WPKH_UTXO          — txid:vout (default empty)
//!   BLINDJOIN_BOT_P2WPKH_WIF           — WIF (default empty)
//!   BLINDJOIN_BOT_P2TR_UTXO            — txid:vout (default empty)
//!   BLINDJOIN_BOT_P2TR_DESCRIPTOR      — descriptor string (default empty)
//!   BLINDJOIN_BOT_P2TR_UTXO_ADDRESS    — bech32m (default empty)
//!   BLINDJOIN_BOT_P2SH_P2WPKH_UTXO          — txid:vout (default empty)
//!   BLINDJOIN_BOT_P2SH_P2WPKH_DESCRIPTOR    — descriptor string (default empty)
//!   BLINDJOIN_BOT_P2SH_P2WPKH_UTXO_ADDRESS  — base58 (default empty)
//!   BLINDJOIN_UTXO + BLINDJOIN_UTXO_WIF — LEGACY (v1.3 fallthrough; D-98)
//!   BLINDJOIN_NETWORK           — must be "signet" (safety guard preserved from v1.3)
//!   BLINDJOIN_COORDINATOR_URL   — coordinator HTTP URL

use anyhow::{bail, Result};
use liquidity_bot::{run, BotConfig, DescriptorTuple, P2wpkhTuple};
use shared::bip322::ScriptType;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("liquidity_bot=info".parse().unwrap()),
        )
        .init();

    // --- Network safety guard (preserved verbatim from v1.3 main.rs:43-49) ---
    let network_str = std::env::var("BLINDJOIN_NETWORK")
        .unwrap_or_else(|_| "signet".to_string());
    if network_str != "signet" {
        bail!(
            "Liquidity bot refuses to start: BLINDJOIN_NETWORK='{}' is not 'signet'. \
             The bot is a signet-only testing tool.",
            network_str
        );
    }
    let network = bitcoin::Network::Signet;

    // --- CSV parse for BLINDJOIN_BOT_SCRIPT_TYPES (default "p2wpkh" for v1.3 compat) ---
    let script_types_csv = std::env::var("BLINDJOIN_BOT_SCRIPT_TYPES")
        .unwrap_or_else(|_| "p2wpkh".to_string());
    let enabled_types: Vec<ScriptType> = script_types_csv
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|token| {
            client::config::parse_script_type(token).map_err(|e| anyhow::anyhow!(
                "BLINDJOIN_BOT_SCRIPT_TYPES = '{}' has unparseable token '{}': {} \
                 (expected lowercase kebab-case: p2wpkh, p2tr, p2sh-p2wpkh)",
                script_types_csv, token, e
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    if enabled_types.is_empty() {
        bail!(
            "BLINDJOIN_BOT_SCRIPT_TYPES = '{}' parsed to zero entries (CSV empty)",
            script_types_csv
        );
    }
    // Reject duplicates — rotation must be over distinct types (D-96).
    let mut seen = std::collections::HashSet::new();
    for st in &enabled_types {
        if !seen.insert(format!("{:?}", st)) {
            bail!(
                "BLINDJOIN_BOT_SCRIPT_TYPES = '{}' contains duplicate '{:?}' \
                 (rotation must be deterministic across distinct types)",
                script_types_csv, st
            );
        }
    }

    // --- Counter file path (BLINDJOIN_BOT_COUNTER_FILE; default /app/data/bot_round_counter) ---
    let counter_file = PathBuf::from(
        std::env::var("BLINDJOIN_BOT_COUNTER_FILE")
            .unwrap_or_else(|_| "/app/data/bot_round_counter".to_string()),
    );

    // --- Per-type tuple loading ---
    let p2wpkh_tuple = build_p2wpkh_tuple_with_legacy_fallthrough()?;
    let p2tr_tuple = build_descriptor_tuple("P2TR")?;
    let p2sh_p2wpkh_tuple = build_descriptor_tuple("P2SH_P2WPKH")?;

    // --- Startup validation: each enabled type must have a populated tuple ---
    for st in &enabled_types {
        match st {
            ScriptType::P2wpkh => {
                if p2wpkh_tuple.is_none() {
                    bail!(
                        "BLINDJOIN_BOT_SCRIPT_TYPES enables p2wpkh but neither \
                         BLINDJOIN_BOT_P2WPKH_{{UTXO,WIF}} nor legacy \
                         BLINDJOIN_UTXO + BLINDJOIN_UTXO_WIF are set"
                    );
                }
            }
            ScriptType::P2tr => {
                if p2tr_tuple.is_none() {
                    bail!(
                        "BLINDJOIN_BOT_SCRIPT_TYPES enables p2tr but \
                         BLINDJOIN_BOT_P2TR_{{UTXO,DESCRIPTOR,UTXO_ADDRESS}} are not all set"
                    );
                }
            }
            ScriptType::P2shP2wpkh => {
                if p2sh_p2wpkh_tuple.is_none() {
                    bail!(
                        "BLINDJOIN_BOT_SCRIPT_TYPES enables p2sh-p2wpkh but \
                         BLINDJOIN_BOT_P2SH_P2WPKH_{{UTXO,DESCRIPTOR,UTXO_ADDRESS}} are not all set"
                    );
                }
            }
        }
    }

    let coordinator_url = std::env::var("BLINDJOIN_COORDINATOR_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let config = BotConfig {
        coordinator_url,
        network,
        enabled_types,
        counter_file,
        p2wpkh_tuple,
        p2tr_tuple,
        p2sh_p2wpkh_tuple,
    };

    run(config).await
}

/// Build a P2WPKH tuple from env vars, with v1.3 legacy fallthrough (D-98).
///
/// Priority:
///   1. BLINDJOIN_BOT_P2WPKH_UTXO + BLINDJOIN_BOT_P2WPKH_WIF (new-style)
///   2. BLINDJOIN_UTXO + BLINDJOIN_UTXO_WIF (legacy v1.3 env vars)
///   3. None (P2WPKH disabled or will fail startup validation)
fn build_p2wpkh_tuple_with_legacy_fallthrough() -> Result<Option<P2wpkhTuple>> {
    let new_utxo = std::env::var("BLINDJOIN_BOT_P2WPKH_UTXO")
        .ok()
        .filter(|s| !s.is_empty());
    let new_wif = std::env::var("BLINDJOIN_BOT_P2WPKH_WIF")
        .ok()
        .filter(|s| !s.is_empty());

    match (new_utxo, new_wif) {
        (Some(utxo), Some(wif)) => {
            tracing::info!("Using BLINDJOIN_BOT_P2WPKH_{{UTXO,WIF}} env vars for P2WPKH");
            Ok(Some(P2wpkhTuple { utxo, wif }))
        }
        (Some(_), None) | (None, Some(_)) => bail!(
            "BLINDJOIN_BOT_P2WPKH_UTXO and BLINDJOIN_BOT_P2WPKH_WIF must be set \
             together (only one was set)"
        ),
        (None, None) => {
            // Legacy fallthrough (D-98): try BLINDJOIN_UTXO + BLINDJOIN_UTXO_WIF.
            let legacy_utxo = std::env::var("BLINDJOIN_UTXO")
                .ok()
                .filter(|s| !s.is_empty());
            let legacy_wif = std::env::var("BLINDJOIN_UTXO_WIF")
                .ok()
                .filter(|s| !s.is_empty());
            match (legacy_utxo, legacy_wif) {
                (Some(utxo), Some(wif)) => {
                    tracing::info!(
                        "Using legacy BLINDJOIN_UTXO + BLINDJOIN_UTXO_WIF env vars for \
                         P2WPKH (D-98 fallthrough)"
                    );
                    Ok(Some(P2wpkhTuple { utxo, wif }))
                }
                _ => Ok(None),
            }
        }
    }
}

/// Build a descriptor-mode tuple for P2TR or P2SH-P2WPKH.
///
/// `prefix` must be "P2TR" or "P2SH_P2WPKH" (maps to BLINDJOIN_BOT_{prefix}_{UTXO,DESCRIPTOR,UTXO_ADDRESS}).
/// Returns None when all three vars are unset. Bails when only a partial set is present.
fn build_descriptor_tuple(prefix: &str) -> Result<Option<DescriptorTuple>> {
    let utxo = std::env::var(format!("BLINDJOIN_BOT_{}_UTXO", prefix))
        .ok()
        .filter(|s| !s.is_empty());
    let descriptor = std::env::var(format!("BLINDJOIN_BOT_{}_DESCRIPTOR", prefix))
        .ok()
        .filter(|s| !s.is_empty());
    let utxo_address = std::env::var(format!("BLINDJOIN_BOT_{}_UTXO_ADDRESS", prefix))
        .ok()
        .filter(|s| !s.is_empty());

    match (utxo, descriptor, utxo_address) {
        (Some(utxo), Some(descriptor), Some(utxo_address)) => {
            Ok(Some(DescriptorTuple { utxo, descriptor, utxo_address }))
        }
        (None, None, None) => Ok(None),
        _ => bail!(
            "BLINDJOIN_BOT_{prefix}_{{UTXO,DESCRIPTOR,UTXO_ADDRESS}} must all be set \
             together (partial set detected)"
        ),
    }
}
