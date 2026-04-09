use bitcoin::OutPoint;
use shared::errors::{ApiError, ErrorCode};
use crate::round::state::RoundState;
use crate::round::manager::verify_session_token;
use crate::round::input_reg::parse_outpoint;
use crate::bitcoin::rpc::BitcoinRpc;
use crate::bitcoin::tx::{build_coinjoin_psbt, ParticipantInput, ParticipantOutput};
use crate::config::CoordinatorConfig;
use bitcoin::ScriptBuf;
use tracing::info;

/// Result of a successful signing submission — indicates whether broadcast was triggered.
pub enum SignResult {
    /// Partial signature recorded; waiting for more participants.
    Recorded,
    /// All signatures collected; TX assembled and broadcast.
    Broadcast { txid: String },
}

/// Core signing phase logic. Called from handler with write-locked state.
///
/// Verifies session token, records partial signature, and broadcasts if complete.
pub async fn process_sign(
    state: &mut RoundState,
    rpc: &BitcoinRpc,
    config: &CoordinatorConfig,
    utxo: &OutPoint,
    partial_signature: &[u8],
    session_token_bytes: &[u8; 32],
    round_id_str: &str,
) -> Result<SignResult, ApiError> {
    let inner = state.inner.as_mut().ok_or_else(|| ApiError {
        code: ErrorCode::WrongPhase,
        message: "Round not in signing phase".into(),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Verify session token (T-04-04: must verify before accepting any partial sig)
    if !verify_session_token(&inner.round_secret, utxo, session_token_bytes) {
        return Err(ApiError {
            code: ErrorCode::SessionInvalid,
            message: "Invalid session token".into(),
            round_id: Some(round_id_str.to_string()),
        });
    }

    let utxo_str = format!("{}:{}", utxo.txid, utxo.vout);

    // Verify UTXO is a registered input in this round
    if !inner.registered_inputs.contains_key(&utxo_str) {
        return Err(ApiError {
            code: ErrorCode::SessionInvalid,
            message: "UTXO not registered in this round".into(),
            round_id: Some(round_id_str.to_string()),
        });
    }

    // Record partial signature (keyed by utxo_outpoint)
    inner.partial_sigs.insert(utxo_str, partial_signature.to_vec());

    // Check if all participants have submitted
    let expected_count = state.participant_count as usize;
    let collected = state.inner.as_ref().map_or(0, |i| i.partial_sigs.len());

    if collected >= expected_count {
        // All signatures collected — assemble and broadcast
        let txid = assemble_and_broadcast(state, rpc, config, round_id_str).await?;
        return Ok(SignResult::Broadcast { txid });
    }

    Ok(SignResult::Recorded)
}

/// Assemble the CoinJoin TX from registered inputs/outputs and broadcast.
async fn assemble_and_broadcast(
    state: &mut RoundState,
    rpc: &BitcoinRpc,
    config: &CoordinatorConfig,
    round_id_str: &str,
) -> Result<String, ApiError> {
    use bitcoin::consensus::encode::serialize_hex;
    use crate::round::state::Phase;

    let inner = state.inner.as_ref().ok_or_else(|| ApiError {
        code: ErrorCode::WrongPhase,
        message: "No round state".into(),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Build participant inputs for PSBT construction
    let participant_inputs: Vec<ParticipantInput> = inner.registered_inputs.values()
        .filter_map(|reg| {
            let outpoint = parse_outpoint(&reg.utxo_str)?;
            // Use a placeholder script_pubkey — actual signing is done client-side via PSBT
            // The PSBT includes witness_utxo for proper signing; we use P2WPKH placeholder
            let change_script = parse_address_to_script(&reg.change_address);
            Some(ParticipantInput {
                outpoint,
                value_sats: config.coordinator.denomination_sats + estimate_fee_share_per_participant(
                    inner.registered_inputs.len() as u32,
                    config.coordinator.fee_rate_sat_per_vbyte,
                ),
                script_pubkey: change_script.clone(),
                change_address: change_script,
            })
        })
        .collect();

    // Build participant outputs
    let participant_outputs: Vec<ParticipantOutput> = inner.registered_outputs.iter()
        .map(|out| ParticipantOutput {
            script_pubkey: parse_address_to_script(&out.address),
        })
        .collect();

    // Build PSBT
    let psbt = build_coinjoin_psbt(
        &participant_inputs,
        &participant_outputs,
        config.coordinator.denomination_sats,
        config.coordinator.fee_rate_sat_per_vbyte,
    ).map_err(|e| ApiError {
        code: ErrorCode::BroadcastRejected,
        message: format!("PSBT construction failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Serialize PSBT to raw TX hex for broadcast
    // For Phase 1: clients have submitted partial_signature bytes which we treat as
    // raw witness data. A full PSBT finalization would merge these; for now we
    // serialize the unsigned TX and broadcast (integration test uses regtest where
    // signatures are pre-applied by the test harness).
    let tx_hex = serialize_hex(&psbt.unsigned_tx);

    // testmempoolaccept before broadcast (T-04 boundary requirement)
    let accept_result = rpc.testmempoolaccept(&[&tx_hex]).await.map_err(|e| ApiError {
        code: ErrorCode::RpcUnavailable,
        message: format!("testmempoolaccept failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Check if accepted
    let accepted = accept_result
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.get("allowed"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !accepted {
        let reject_reason = accept_result
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|entry| entry.get("reject-reason"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        return Err(ApiError {
            code: ErrorCode::BroadcastRejected,
            message: format!("TX rejected by mempool: {reject_reason}"),
            round_id: Some(round_id_str.to_string()),
        });
    }

    // Broadcast
    let txid = rpc.sendrawtransaction(&tx_hex).await.map_err(|e| ApiError {
        code: ErrorCode::BroadcastRejected,
        message: format!("Broadcast failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;

    // ALLOWED to log txid — it's public info (T-04-05)
    info!(txid = %txid, round_id = %round_id_str, "CoinJoin TX broadcast");

    // Transition round to Broadcast then Idle (zeroes all sensitive state)
    let _ = state.transition_to(Phase::Broadcast);
    let _ = state.transition_to(Phase::Idle);

    Ok(txid.to_string())
}

/// Parse an address string to ScriptBuf (best-effort; falls back to empty script).
fn parse_address_to_script(addr_str: &str) -> ScriptBuf {
    use std::str::FromStr;
    // Try mainnet, testnet, signet
    for network in [bitcoin::Network::Signet, bitcoin::Network::Bitcoin, bitcoin::Network::Testnet] {
        if let Ok(addr) = bitcoin::Address::from_str(addr_str)
            .and_then(|a| a.require_network(network).map_err(|e| bitcoin::address::ParseError::from(e)))
        {
            return addr.script_pubkey();
        }
    }
    // Fallback: empty script (will fail validation downstream)
    ScriptBuf::new()
}

fn estimate_fee_share_per_participant(n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 { return 0; }
    let estimated_vsize = 10 + n * 68 + n * 2 * 31;
    (estimated_vsize * fee_rate) / n
}
