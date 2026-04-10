// PRIVACY: never log UTXO outpoints, addresses, tokens, or signatures (PRIV-02)
// ALLOWED in logs: round_id, phase, participant_count, txid (after broadcast), addr (listen addr), block_count

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use base64::Engine;
use serde_json::{json, Value};
use shared::protocol::{
    InfoResponse, InputRegRequest, InputRegResponse,
    OutputRegRequest, OutputRegResponse, SignRequest, RoundTxResponse,
};
use tracing::info;

use crate::api::AppState;
use crate::round::state::Phase;
use crate::round::input_reg::{register_input, parse_outpoint};
use crate::round::output_reg::register_output_logic;
use crate::round::signing::{process_sign, SignResult};
use crate::bitcoin::tx::{build_coinjoin_psbt, ParticipantInput, ParticipantOutput};
use blind_rsa_signatures::MessageRandomizer;
use bitcoin::ScriptBuf;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

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

/// GET /info — coordinator status and round parameters.
pub async fn get_info(State(state): State<AppState>) -> Json<InfoResponse> {
    let guard = state.round.read().await;

    let rsa_pubkey_der_b64 = guard.rsa_pubkey_der
        .as_ref()
        .map(|der| B64.encode(der));

    let rsa_pubkey_hash = guard.rsa_pubkey_hash
        .map(hex::encode);

    Json(InfoResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        network: state.config.network.bitcoin_network.clone(),
        denomination_sats: state.config.coordinator.denomination_sats,
        min_participants: state.config.coordinator.min_participants,
        max_participants: state.config.coordinator.max_participants,
        round_state: guard.phase.as_str().to_string(),
        participants_registered: guard.participant_count,
        rsa_pubkey_hash,
        rsa_pubkey_der_b64,
        round_id: Some(guard.round_id),
    })
}

/// POST /round/input — register a UTXO for CoinJoin.
pub async fn post_input(
    State(state): State<AppState>,
    Json(req): Json<InputRegRequest>,
) -> Result<Json<InputRegResponse>, (StatusCode, Json<Value>)> {
    // Phase check — read lock first
    {
        let guard = state.round.read().await;
        if guard.phase != Phase::InputReg {
            let round_id = guard.round_id.to_string();
            return Err(api_error(
                StatusCode::CONFLICT,
                "WRONG_PHASE",
                format!("Expected input_reg, got {}", guard.phase.as_str()),
                Some(&round_id),
            ));
        }
    }

    // Parse UTXO outpoint
    let utxo = parse_outpoint(&req.utxo_outpoint).ok_or_else(|| {
        api_error(StatusCode::BAD_REQUEST, "UTXO_NOT_FOUND",
            "Invalid utxo_outpoint format (expected txid:vout)", None)
    })?;

    // Decode base64 blinded token
    let blinded_token_bytes = B64.decode(&req.blinded_token).map_err(|_| {
        api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
            "blinded_token is not valid base64", None)
    })?;

    // Ban check — fast rejection before acquiring round write lock (T-02-01).
    // BanList::is_banned hashes the outpoint internally; pass the raw string.
    // No UTXO identifiers are logged here (PRIV-02).
    {
        let ban_guard = state.ban_list.read().await;
        let now = crate::round::blame::now_unix_secs();
        if ban_guard.is_banned(&req.utxo_outpoint, now) {
            return Err(api_error(
                StatusCode::FORBIDDEN,
                "UTXO_BANNED",
                "UTXO is temporarily banned",
                None,
            ));
        }
    }

    // Write lock for state mutation
    let mut guard = state.round.write().await;

    // Re-check phase under write lock (TOCTOU prevention, T-04-01)
    if guard.phase != Phase::InputReg {
        let round_id = guard.round_id.to_string();
        return Err(api_error(
            StatusCode::CONFLICT,
            "WRONG_PHASE",
            format!("Expected input_reg, got {}", guard.phase.as_str()),
            Some(&round_id),
        ));
    }

    let round_id = guard.round_id;
    let round_id_str = round_id.to_string();
    let denomination_sats = state.config.coordinator.denomination_sats;
    let fee_rate = state.config.coordinator.fee_rate_sat_per_vbyte;
    let max_participants = state.config.coordinator.max_participants;

    // Check round not full
    if guard.participant_count >= max_participants {
        return Err(api_error(
            StatusCode::CONFLICT,
            "ROUND_FULL",
            "Round is full",
            Some(&round_id_str),
        ));
    }

    // Ensure round inner state is initialized (should be if in InputReg)
    if guard.inner.is_none() {
        return Err(api_error(
            StatusCode::CONFLICT,
            "WRONG_PHASE",
            "Round inner state not initialized",
            Some(&round_id_str),
        ));
    }

    let result = register_input(
        &mut guard,
        &state.rpc,
        &utxo,
        &req.ownership_proof,
        &blinded_token_bytes,
        &req.change_address,
        denomination_sats,
        fee_rate,
        max_participants,
        &round_id_str,
    ).await.map_err(|e| {
        let status = match e.code {
            shared::errors::ErrorCode::WrongPhase => StatusCode::CONFLICT,
            shared::errors::ErrorCode::RpcUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_REQUEST,
        };
        let code_str = serde_json::to_string(&e.code)
            .unwrap_or_else(|_| "\"INTERNAL_ERROR\"".into())
            .trim_matches('"').to_string();
        api_error(status, &code_str, e.message, Some(&round_id_str))
    })?;

    // If max participants reached, advance to OutputReg and initialize signer pubkey
    if guard.participant_count >= max_participants {
        info!(
            round_id = %round_id_str,
            participant_count = guard.participant_count,
            "Max participants reached — advancing to output_reg"
        );
        let _ = guard.transition_to(Phase::OutputReg);
    }

    Ok(Json(InputRegResponse {
        blind_signature: result.blind_signature_b64,
        round_id,
        session_token: result.session_token_b64,
    }))
}

/// POST /round/output — register CoinJoin output using unblinded token.
pub async fn post_output(
    State(state): State<AppState>,
    Json(req): Json<OutputRegRequest>,
) -> Result<Json<OutputRegResponse>, (StatusCode, Json<Value>)> {
    // Phase check
    {
        let guard = state.round.read().await;
        if guard.phase != Phase::OutputReg {
            let round_id = guard.round_id.to_string();
            return Err(api_error(
                StatusCode::CONFLICT,
                "WRONG_PHASE",
                format!("Expected output_reg, got {}", guard.phase.as_str()),
                Some(&round_id),
            ));
        }
    }

    // Decode token and signature
    let token_bytes = B64.decode(&req.unblinded_token).map_err(|_| {
        api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
            "unblinded_token is not valid base64", None)
    })?;
    if token_bytes.len() != 32 {
        return Err(api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
            "unblinded_token must be 32 bytes", None));
    }
    let mut token_msg = [0u8; 32];
    token_msg.copy_from_slice(&token_bytes);

    let sig_bytes = B64.decode(&req.signature).map_err(|_| {
        api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
            "signature is not valid base64", None)
    })?;

    // Decode msg_randomizer (required for RSABSSA-SHA384-PSS-Randomized verification)
    let msg_randomizer = match &req.msg_randomizer {
        Some(b64_str) => {
            let bytes = B64.decode(b64_str).map_err(|_| {
                api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
                    "msg_randomizer is not valid base64", None)
            })?;
            if bytes.len() != 32 {
                return Err(api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
                    "msg_randomizer must be 32 bytes", None));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Some(MessageRandomizer(arr))
        }
        None => None,
    };

    // Write lock
    let mut guard = state.round.write().await;

    // Re-check phase under write lock (T-04-01)
    if guard.phase != Phase::OutputReg {
        let round_id = guard.round_id.to_string();
        return Err(api_error(
            StatusCode::CONFLICT,
            "WRONG_PHASE",
            format!("Expected output_reg, got {}", guard.phase.as_str()),
            Some(&round_id),
        ));
    }

    let round_id = guard.round_id;
    let round_id_str = round_id.to_string();
    let denomination_sats = state.config.coordinator.denomination_sats;

    // Use cached RSA public key for token verification (signer parsed once at creation — AVAIL-02).
    // Clone the public key here to release the immutable borrow before the mutable borrow below.
    // BjPublicKey implements Clone (blind-rsa-signatures 0.17.1, lib.rs:601).
    let rsa_public_key = guard.inner.as_ref()
        .ok_or_else(|| api_error(StatusCode::CONFLICT, "WRONG_PHASE",
            "Round inner state not initialized", Some(&round_id_str)))?
        .rsa_signer.public_key.clone();

    // Run pure output reg logic
    {
        let inner = guard.inner.as_mut().ok_or_else(|| {
            api_error(StatusCode::CONFLICT, "WRONG_PHASE",
                "Round inner state not initialized", Some(&round_id_str))
        })?;

        register_output_logic(
            &rsa_public_key,
            &mut inner.redeemed_tokens,
            &token_msg,
            &sig_bytes,
            msg_randomizer,
            denomination_sats,
            req.amount_sats,
        ).map_err(|e| {
            let status = match e.code {
                shared::errors::ErrorCode::TokenAlreadyUsed => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            let code_str = serde_json::to_string(&e.code)
                .unwrap_or_else(|_| "\"INTERNAL_ERROR\"".into())
                .trim_matches('"').to_string();
            api_error(status, &code_str, e.message, Some(&round_id_str))
        })?;

        // Parse and store output address
        inner.registered_outputs.push(crate::round::state::RegisteredOutput {
            address: req.output_address.clone(),
            amount_sats: req.amount_sats,
        });
    }

    // Check if all participants have registered outputs
    let outputs_count = guard.inner.as_ref().map_or(0, |i| i.registered_outputs.len());
    let expected = guard.participant_count as usize;
    if outputs_count >= expected && expected > 0 {
        info!(
            round_id = %round_id_str,
            participant_count = guard.participant_count,
            "All outputs registered — advancing to signing"
        );
        let _ = guard.transition_to(Phase::Signing);
    }

    Ok(Json(OutputRegResponse {
        accepted: true,
        round_id,
    }))
}

/// GET /round/tx — return the assembled PSBT for participant signing.
pub async fn get_tx(
    State(state): State<AppState>,
) -> Result<Json<RoundTxResponse>, (StatusCode, Json<Value>)> {
    let guard = state.round.read().await;

    if guard.phase != Phase::Signing {
        let round_id = guard.round_id.to_string();
        return Err(api_error(
            StatusCode::CONFLICT,
            "WRONG_PHASE",
            format!("Expected signing, got {}", guard.phase.as_str()),
            Some(&round_id),
        ));
    }

    let round_id = guard.round_id;
    let round_id_str = round_id.to_string();
    let denomination_sats = state.config.coordinator.denomination_sats;
    let fee_rate = state.config.coordinator.fee_rate_sat_per_vbyte;

    let inner = guard.inner.as_ref().ok_or_else(|| {
        api_error(StatusCode::CONFLICT, "WRONG_PHASE",
            "Round inner state not initialized", Some(&round_id_str))
    })?;

    let bitcoin_network = parse_bitcoin_network(&state.config.network.bitcoin_network);

    // Build participant inputs and outputs for PSBT
    let mut participant_inputs: Vec<ParticipantInput> = Vec::new();
    for reg in inner.registered_inputs.values() {
        let outpoint = parse_outpoint(&reg.utxo_str).ok_or_else(|| {
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR",
                format!("Invalid outpoint: {}", reg.utxo_str), Some(&round_id_str))
        })?;
        let n = inner.registered_inputs.len() as u32;
        let fee_share = estimate_fee_share(n, fee_rate);
        let change_script = parse_address_to_script(&reg.change_address, bitcoin_network)
            .map_err(|e| api_error(StatusCode::BAD_REQUEST, "INVALID_ADDRESS",
                e, Some(&round_id_str)))?;
        participant_inputs.push(ParticipantInput {
            outpoint,
            value_sats: denomination_sats + fee_share,
            script_pubkey: change_script.clone(),
            change_address: change_script,
        });
    }

    let mut participant_outputs: Vec<ParticipantOutput> = Vec::new();
    for out in inner.registered_outputs.iter() {
        let script = parse_address_to_script(&out.address, bitcoin_network)
            .map_err(|e| api_error(StatusCode::BAD_REQUEST, "INVALID_ADDRESS",
                e, Some(&round_id_str)))?;
        participant_outputs.push(ParticipantOutput { script_pubkey: script });
    }

    let psbt = build_coinjoin_psbt(
        &participant_inputs,
        &participant_outputs,
        denomination_sats,
        fee_rate,
    ).map_err(|e| {
        api_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR",
            format!("PSBT build failed: {e}"), Some(&round_id_str))
    })?;

    let psbt_bytes = psbt.serialize();
    let psbt_b64 = B64.encode(&psbt_bytes);

    let n = participant_inputs.len() as u64;
    let estimated_vsize = 10 + n * 68 + n * 2 * 31;
    let fee_total_sats = estimated_vsize * fee_rate;
    let fee_per_participant_sats = if n > 0 { fee_total_sats / n } else { 0 };

    Ok(Json(RoundTxResponse {
        round_id,
        psbt: psbt_b64,
        fee_total_sats,
        fee_per_participant_sats,
    }))
}

/// POST /round/sign — submit a partial PSBT signature.
pub async fn post_sign(
    State(state): State<AppState>,
    Json(req): Json<SignRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Phase check
    {
        let guard = state.round.read().await;
        if guard.phase != Phase::Signing {
            let round_id = guard.round_id.to_string();
            return Err(api_error(
                StatusCode::CONFLICT,
                "WRONG_PHASE",
                format!("Expected signing, got {}", guard.phase.as_str()),
                Some(&round_id),
            ));
        }
    }

    // Parse UTXO outpoint
    let utxo = parse_outpoint(&req.utxo_outpoint).ok_or_else(|| {
        api_error(StatusCode::BAD_REQUEST, "SESSION_INVALID",
            "Invalid utxo_outpoint format", None)
    })?;

    // Decode partial signature
    let partial_sig_bytes = B64.decode(&req.partial_signature).map_err(|_| {
        api_error(StatusCode::BAD_REQUEST, "SESSION_INVALID",
            "partial_signature is not valid base64", None)
    })?;

    // Decode session token
    let session_token_bytes = B64.decode(&req.session_token).map_err(|_| {
        api_error(StatusCode::BAD_REQUEST, "SESSION_INVALID",
            "session_token is not valid base64", None)
    })?;
    if session_token_bytes.len() != 32 {
        return Err(api_error(StatusCode::BAD_REQUEST, "SESSION_INVALID",
            "session_token must be 32 bytes", None));
    }
    let mut session_token = [0u8; 32];
    session_token.copy_from_slice(&session_token_bytes);

    // Write lock
    let mut guard = state.round.write().await;

    // Re-check phase under write lock (T-04-01)
    if guard.phase != Phase::Signing {
        let round_id = guard.round_id.to_string();
        return Err(api_error(
            StatusCode::CONFLICT,
            "WRONG_PHASE",
            format!("Expected signing, got {}", guard.phase.as_str()),
            Some(&round_id),
        ));
    }

    let round_id_str = guard.round_id.to_string();

    let sign_result = process_sign(
        &mut guard,
        &state.rpc,
        &state.config,
        &utxo,
        &partial_sig_bytes,
        &session_token,
        &round_id_str,
    ).await.map_err(|e| {
        let status = match e.code {
            shared::errors::ErrorCode::SessionInvalid => StatusCode::UNAUTHORIZED,
            shared::errors::ErrorCode::WrongPhase => StatusCode::CONFLICT,
            shared::errors::ErrorCode::RpcUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_REQUEST,
        };
        let code_str = serde_json::to_string(&e.code)
            .unwrap_or_else(|_| "\"INTERNAL_ERROR\"".into())
            .trim_matches('"').to_string();
        api_error(status, &code_str, e.message, Some(&round_id_str))
    })?;

    match sign_result {
        SignResult::Recorded => {
            Ok(Json(json!({ "status": "recorded" })))
        }
        SignResult::Broadcast { txid } => {
            Ok(Json(json!({ "status": "broadcast", "txid": txid })))
        }
    }
}

/// Parse an address string to ScriptBuf, validating against the expected network.
///
/// Returns an error if the address is invalid or belongs to a different network.
fn parse_address_to_script(addr_str: &str, expected_network: bitcoin::Network) -> Result<ScriptBuf, String> {
    use std::str::FromStr;
    bitcoin::Address::from_str(addr_str)
        .and_then(|a| a.require_network(expected_network).map_err(bitcoin::address::ParseError::from))
        .map(|a| a.script_pubkey())
        .map_err(|e| format!("Invalid address '{}': {}", addr_str, e))
}

/// Parse a bitcoin network name string into bitcoin::Network.
fn parse_bitcoin_network(network_str: &str) -> bitcoin::Network {
    match network_str {
        "mainnet" | "bitcoin" => bitcoin::Network::Bitcoin,
        "testnet" | "testnet4" => bitcoin::Network::Testnet,
        "signet" => bitcoin::Network::Signet,
        "regtest" => bitcoin::Network::Regtest,
        _ => bitcoin::Network::Signet,
    }
}

fn estimate_fee_share(n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 { return 0; }
    let estimated_vsize = 10 + n * 68 + n * 2 * 31;
    (estimated_vsize * fee_rate) / n
}
