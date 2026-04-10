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
#[derive(Debug)]
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

    let bitcoin_network = parse_bitcoin_network(&config.network.bitcoin_network);

    // Build participant inputs for PSBT construction
    let mut participant_inputs: Vec<ParticipantInput> = Vec::new();
    for reg in inner.registered_inputs.values() {
        let outpoint = parse_outpoint(&reg.utxo_str).ok_or_else(|| ApiError {
            code: ErrorCode::BroadcastRejected,
            message: format!("Invalid outpoint string: {}", reg.utxo_str),
            round_id: Some(round_id_str.to_string()),
        })?;
        let change_script = parse_address_to_script(&reg.change_address, bitcoin_network)
            .map_err(|e| ApiError {
                code: ErrorCode::BroadcastRejected,
                message: format!("Invalid change address: {e}"),
                round_id: Some(round_id_str.to_string()),
            })?;
        participant_inputs.push(ParticipantInput {
            outpoint,
            value_sats: config.coordinator.denomination_sats + estimate_fee_share_per_participant(
                inner.registered_inputs.len() as u32,
                config.coordinator.fee_rate_sat_per_vbyte,
            ),
            script_pubkey: change_script.clone(),
            change_address: change_script,
        });
    }

    // Build participant outputs
    let mut participant_outputs: Vec<ParticipantOutput> = Vec::new();
    for out in inner.registered_outputs.iter() {
        let script = parse_address_to_script(&out.address, bitcoin_network)
            .map_err(|e| ApiError {
                code: ErrorCode::BroadcastRejected,
                message: format!("Invalid output address: {e}"),
                round_id: Some(round_id_str.to_string()),
            })?;
        participant_outputs.push(ParticipantOutput { script_pubkey: script });
    }

    // Build PSBT
    let mut psbt = build_coinjoin_psbt(
        &participant_inputs,
        &participant_outputs,
        config.coordinator.denomination_sats,
        config.coordinator.fee_rate_sat_per_vbyte,
    ).map_err(|e| ApiError {
        code: ErrorCode::BroadcastRejected,
        message: format!("PSBT construction failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;

    // Apply partial signatures as witness data to each input.
    // Each participant submitted their signature + pubkey as serialized witness bytes.
    // We decode these and set them as the witness for the corresponding input.
    for (i, input) in psbt.unsigned_tx.input.iter().enumerate() {
        let outpoint_str = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
        if let Some(sig_bytes) = inner.partial_sigs.get(&outpoint_str) {
            // Deserialize the witness from the raw bytes the client sent
            match bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes) {
                Ok(witness) => {
                    psbt.inputs[i].final_script_witness = Some(witness);
                }
                Err(_) => {
                    return Err(ApiError {
                        code: ErrorCode::BroadcastRejected,
                        message: format!("Invalid witness data for input {}", i),
                        round_id: Some(round_id_str.to_string()),
                    });
                }
            }
        } else {
            return Err(ApiError {
                code: ErrorCode::BroadcastRejected,
                message: format!("Missing signature for input {}", i),
                round_id: Some(round_id_str.to_string()),
            });
        }
    }

    // Extract the finalized transaction from the PSBT
    let final_tx = psbt.extract_tx().map_err(|e| ApiError {
        code: ErrorCode::BroadcastRejected,
        message: format!("PSBT extraction failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })?;
    let tx_hex = serialize_hex(&final_tx);

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

/// Parse an address string to ScriptBuf, validating against the expected network.
///
/// Returns an error if the address is invalid or belongs to a different network.
/// This prevents cross-network confusion attacks (e.g., a signet address accepted
/// by a mainnet coordinator) and surfaces bad addresses before PSBT construction.
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
        _ => bitcoin::Network::Signet, // safe default
    }
}

fn estimate_fee_share_per_participant(n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 { return 0; }
    let estimated_vsize = 10 + n * 68 + n * 2 * 31;
    (estimated_vsize * fee_rate) / n
}

// TEST-06: Signing unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::state::{Phase, RoundState, RoundStateInner, RegisteredInput};
    use crate::round::manager::generate_session_token;
    use crate::bitcoin::rpc::BitcoinRpc;
    use crate::config::CoordinatorConfig;
    use crate::blind::rsa::RsaBlindSigner;
    use bitcoin::{OutPoint, Txid};
    use std::collections::{HashMap, HashSet};
    use std::str::FromStr;

    fn test_txid() -> Txid {
        Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ).unwrap()
    }

    fn make_signing_state(utxo_str: &str) -> (RoundState, [u8; 32]) {
        let mut state = RoundState::new_idle();
        state.phase = Phase::Signing;
        let round_secret = [0xab_u8; 32];
        let inner = RoundStateInner {
            rsa_signing_key: vec![0u8; 1], // placeholder
            rsa_signer: RsaBlindSigner::generate().unwrap(),
            round_secret,
            registered_inputs: {
                let mut m = HashMap::new();
                m.insert(utxo_str.to_string(), RegisteredInput {
                    utxo_str: utxo_str.to_string(),
                    change_address: "tb1qtest".to_string(),
                    blind_sig_hash: [0u8; 32],
                });
                m
            },
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(),
            change_addresses: HashMap::new(),
        };
        state.participant_count = 1;
        state.inner = Some(inner);
        (state, round_secret)
    }

    /// TEST-06: Invalid session token rejected — wrong token bytes → SessionInvalid
    #[tokio::test]
    async fn test_process_sign_invalid_session_token() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, _secret) = make_signing_state(&utxo_str);

        let rpc = BitcoinRpc::new(
            "http://127.0.0.1:38332".into(), "user".into(), "pass".into()
        );
        let config = CoordinatorConfig::with_defaults();
        let wrong_token = [0x00u8; 32];

        let result = process_sign(
            &mut state, &rpc, &config, &utxo,
            &[1, 2, 3], &wrong_token, "test-round"
        ).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, shared::errors::ErrorCode::SessionInvalid);
    }

    /// TEST-06: Wrong outpoint rejected — token valid for utxo:0 but submitted as utxo:1
    #[tokio::test]
    async fn test_process_sign_wrong_outpoint() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

        let rpc = BitcoinRpc::new(
            "http://127.0.0.1:38332".into(), "user".into(), "pass".into()
        );
        let config = CoordinatorConfig::with_defaults();

        // Correct token for utxo:0 but submit as utxo:1 (wrong outpoint)
        let wrong_utxo = OutPoint::new(txid, 1);
        let _token_for_correct_utxo = generate_session_token(&secret, &utxo); // token for :0

        // Token generated for utxo:1 but utxo:1 is not registered → token check fails
        // (token for utxo:1 is different from token for utxo:0, and utxo:1 is not registered)
        let token_for_wrong_utxo = generate_session_token(&secret, &wrong_utxo);

        let result = process_sign(
            &mut state, &rpc, &config, &wrong_utxo,
            &[1, 2, 3], &token_for_wrong_utxo, "test-round"
        ).await;

        // utxo:1 passes token check but is not registered — SessionInvalid
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, shared::errors::ErrorCode::SessionInvalid);
    }

    /// TEST-06: Valid partial sig recorded — participant_count=2, 1 sig → SignResult::Recorded
    #[tokio::test]
    async fn test_process_sign_records_partial_sig() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

        let rpc = BitcoinRpc::new(
            "http://127.0.0.1:38332".into(), "user".into(), "pass".into()
        );
        let config = CoordinatorConfig::with_defaults();
        let token = generate_session_token(&secret, &utxo);

        // With participant_count=2, submitting 1 sig just records — no broadcast attempt
        state.participant_count = 2;

        let result = process_sign(
            &mut state, &rpc, &config, &utxo,
            &[0xde, 0xad, 0xbe, 0xef], &token, "test-round"
        ).await;

        // participant_count=2 but only 1 sig collected → Recorded
        assert!(matches!(result, Ok(SignResult::Recorded)));
        let partial_sigs = &state.inner.as_ref().unwrap().partial_sigs;
        assert!(partial_sigs.contains_key(&utxo_str));
    }

    // TEST-07 blame unit tests — on_signing_timeout and BlameOutcome from crate::round::blame

    /// TEST-07: Non-signer detected and banned after on_signing_timeout
    #[test]
    fn test_blame_non_signer_banned() {
        use crate::round::blame::{BanList, on_signing_timeout, BlameOutcome, now_unix_secs};

        let mut state = RoundState::new_idle();
        state.phase = Phase::Signing;
        let inner = RoundStateInner {
            rsa_signing_key: vec![],
            rsa_signer: RsaBlindSigner::generate().unwrap(),
            round_secret: [0u8; 32],
            registered_inputs: {
                let mut m = HashMap::new();
                m.insert("txabc:0".to_string(), RegisteredInput {
                    utxo_str: "txabc:0".to_string(),
                    change_address: "addr".to_string(),
                    blind_sig_hash: [0u8; 32],
                });
                m
            },
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(), // No partial sig for txabc:0 → non-signer
            change_addresses: HashMap::new(),
        };
        state.participant_count = 1;
        state.inner = Some(inner);

        let mut ban_list = BanList::new();
        let now = now_unix_secs();

        let outcome = on_signing_timeout(
            &mut state, &mut ban_list,
            "/dev/null", // ban file — writes will fail silently
            3600, 0,    // ban_duration_secs=3600, blame_round_count=0
        );

        assert!(ban_list.is_banned("txabc:0", now + 10), "non-signer must be banned");
        assert!(matches!(outcome, BlameOutcome::RestartWithout { .. }));
        assert_eq!(state.phase, Phase::Idle, "state must be Idle after blame");
        assert!(state.inner.is_none(), "inner must be dropped");
    }

    /// TEST-07: blame_round_count cap (>=2) triggers FullAbort regardless of non-signers
    #[test]
    fn test_blame_cap_triggers_full_abort() {
        use crate::round::blame::{BanList, on_signing_timeout, BlameOutcome};

        let mut state = RoundState::new_idle();
        state.phase = Phase::Signing;
        state.inner = Some(RoundStateInner {
            rsa_signing_key: vec![],
            rsa_signer: RsaBlindSigner::generate().unwrap(),
            round_secret: [0u8; 32],
            registered_inputs: HashMap::new(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(),
            change_addresses: HashMap::new(),
        });
        let mut ban_list = BanList::new();

        // blame_round_count = 2 → should trigger FullAbort regardless
        let outcome = on_signing_timeout(&mut state, &mut ban_list, "/dev/null", 3600, 2);
        assert!(matches!(outcome, BlameOutcome::FullAbort));
    }

    /// TEST-07: 2 inputs / 1 output → on_output_reg_timeout returns BlameRestart, state=Idle
    #[test]
    fn test_missing_output_triggers_blame() {
        use crate::round::output_reg::{on_output_reg_timeout, OutputRegOutcome};
        use crate::round::state::RegisteredOutput;

        let mut state = RoundState::new_idle();
        state.phase = Phase::OutputReg;
        let mut inner = RoundStateInner {
            rsa_signing_key: vec![],
            rsa_signer: RsaBlindSigner::generate().unwrap(),
            round_secret: [0u8; 32],
            registered_inputs: HashMap::new(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(),
            change_addresses: HashMap::new(),
        };
        // 2 inputs, 1 output → missing output
        inner.registered_inputs.insert("tx1:0".to_string(), RegisteredInput {
            utxo_str: "tx1:0".to_string(), change_address: "a".into(), blind_sig_hash: [0u8; 32],
        });
        inner.registered_inputs.insert("tx2:0".to_string(), RegisteredInput {
            utxo_str: "tx2:0".to_string(), change_address: "b".into(), blind_sig_hash: [0u8; 32],
        });
        inner.registered_outputs.push(RegisteredOutput {
            address: "out1".into(), amount_sats: 100_000,
        });
        state.participant_count = 2;
        state.inner = Some(inner);

        let outcome = on_output_reg_timeout(&mut state);
        assert!(matches!(outcome, OutputRegOutcome::BlameRestart));
        assert_eq!(state.phase, Phase::Idle);
    }

    /// TEST-07: After blame, round is in Idle phase and inner is None
    #[test]
    fn test_round_restart_after_blame_state_is_idle() {
        use crate::round::blame::{BanList, on_signing_timeout};

        let mut state = RoundState::new_idle();
        state.phase = Phase::Signing;
        state.inner = Some(RoundStateInner {
            rsa_signing_key: vec![],
            rsa_signer: RsaBlindSigner::generate().unwrap(),
            round_secret: [0u8; 32],
            registered_inputs: HashMap::new(),
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(),
            change_addresses: HashMap::new(),
        });
        let mut ban_list = BanList::new();
        on_signing_timeout(&mut state, &mut ban_list, "/dev/null", 3600, 0);
        // After blame, round is Idle and ready for the next round
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.inner.is_none());
    }
}
