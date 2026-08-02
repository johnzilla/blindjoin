use bitcoin::OutPoint;
use bitcoin::psbt::Psbt;
use shared::errors::{ApiError, ErrorCode};
use crate::round::state::{RoundState, RoundStateInner};
use crate::round::manager::verify_session_token;
use crate::round::input_reg::parse_outpoint;
use crate::bitcoin::rpc::BitcoinRpc;
use crate::bitcoin::sig_verify::verify_input_signature;
use crate::bitcoin::tx::{build_coinjoin_psbt, ParticipantInput, ParticipantOutput};
use crate::config::CoordinatorConfig;
use crate::round::blame::{BanList, BanEntry, append_ban_entry, now_unix_secs, CONSECUTIVE_BLAME_CAP};
use bitcoin::ScriptBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;
use tracing::{info, warn};

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
#[allow(clippy::too_many_arguments)]
pub async fn process_sign(
    state: &mut RoundState,
    rpc: &BitcoinRpc,
    config: &CoordinatorConfig,
    ban_list: &mut BanList,
    blame_round_count: &AtomicU32,
    round_paused_until: &AtomicU64,
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

    // Reject duplicate submission — prevent a participant from replacing their valid
    // signature with malformed data and forcing a blame round (WR-01).
    if inner.partial_sigs.contains_key(&utxo_str) {
        return Err(ApiError {
            code: ErrorCode::SessionInvalid,
            message: "Partial signature already submitted for this input".into(),
            round_id: Some(round_id_str.to_string()),
        });
    }

    // H3: cryptographically verify the partial signature against the canonical
    // CoinJoin transaction's sighash BEFORE recording it. Previously the bytes
    // were stored unchecked and only an aggregate testmempoolaccept caught a bad
    // signature — by which point everyone had "signed", so the blame path banned
    // nobody and the round aborted. A single participant could thus destroy every
    // round at zero cost and escape blame. Rejecting here (without recording)
    // leaves the sender unsigned, so the signing-deadline blame treats them as a
    // bannable non-signer.
    //
    // The PSBT built here is the SAME one assemble_and_broadcast will broadcast
    // (same build_canonical_psbt, same `inner`, registration closed) — so the
    // sighash verified is exactly the sighash that will be spent.
    let script_type = inner
        .registered_inputs
        .get(&utxo_str)
        .map(|reg| reg.script_type)
        .ok_or_else(|| ApiError {
            code: ErrorCode::SessionInvalid,
            message: "UTXO not registered in this round".into(),
            round_id: Some(round_id_str.to_string()),
        })?;
    let canonical = build_canonical_psbt(inner, config, round_id_str)?;
    let input_index = canonical
        .unsigned_tx
        .input
        .iter()
        .position(|i| i.previous_output == *utxo)
        .ok_or_else(|| ApiError {
            code: ErrorCode::BroadcastRejected,
            message: "Registered input absent from canonical transaction".into(),
            round_id: Some(round_id_str.to_string()),
        })?;
    verify_input_signature(&canonical, input_index, script_type, partial_signature).map_err(|e| {
        ApiError {
            code: ErrorCode::InvalidSignature,
            message: format!("Partial signature rejected: {e}"),
            round_id: Some(round_id_str.to_string()),
        }
    })?;

    // Record partial signature (keyed by utxo_outpoint)
    inner.partial_sigs.insert(utxo_str, partial_signature.to_vec());

    // Check if all participants have submitted
    let expected_count = state.participant_count as usize;
    let collected = state.inner.as_ref().map_or(0, |i| i.partial_sigs.len());

    if collected >= expected_count {
        // All signatures collected — assemble and broadcast
        let txid = assemble_and_broadcast(
            state, rpc, config, ban_list, blame_round_count, round_paused_until, round_id_str,
        ).await?;
        return Ok(SignResult::Broadcast { txid });
    }

    Ok(SignResult::Recorded)
}

/// H2: on broadcast failure, re-validate every registered input against the
/// mempool-aware UTXO set, ban the ones that have since been spent, and end the
/// round via Blame→Idle instead of leaving it wedged in Signing until the signing
/// timeout (where every participant has "signed" → zero non-signers → the round
/// dies with nobody attributed and the griefer escapes).
///
/// Banning is gated on ACTUAL on-chain/mempool spentness, so a genuinely transient
/// broadcast failure (RPC hiccup, fee issue) bans nobody — it just restarts the
/// round. Only a participant who spent their registered coin out from under the
/// round is banned, which is correct attribution.
///
/// H2 follow-up 1 (fast-churn guard): an *unattributed* failure (banned == 0, e.g.
/// the static fee rate is too low for the current mempool) would otherwise recur
/// every round forever, N RPC calls per cycle, because Blame→Idle lets the monitor
/// re-arm instantly into the identical failure. So when nothing was attributed we
/// count the round toward the consecutive-blame cap and, on reaching it, arm the
/// same FullAbort backoff the signing-timeout path uses (H3) — pausing the
/// re-armer. When a griefer IS attributed (banned > 0) the round made progress
/// (that coin is now banned and can't recur), so we reset the counter and let the
/// next round start immediately.
async fn blame_broadcast_failure(
    state: &mut RoundState,
    rpc: &BitcoinRpc,
    ban_list: &mut BanList,
    blame_round_count: &AtomicU32,
    round_paused_until: &AtomicU64,
    config: &CoordinatorConfig,
    round_id_str: &str,
) {
    use crate::round::state::Phase;

    let ban_file_path = &config.coordinator.ban_file_path;
    let ban_duration_secs = config.coordinator.blame_ban_duration_secs;

    // Snapshot the outpoints to re-check (release the borrow before mutating state).
    let outpoints: Vec<String> = state
        .inner
        .as_ref()
        .map(|inner| inner.registered_inputs.keys().cloned().collect())
        .unwrap_or_default();

    let now = now_unix_secs();
    let ban_duration = Duration::from_secs(ban_duration_secs);
    let mut banned = 0usize;

    for utxo_str in &outpoints {
        let Some(outpoint) = parse_outpoint(utxo_str) else { continue };
        match rpc
            .is_output_unspent_including_mempool(&outpoint.txid, outpoint.vout)
            .await
        {
            Ok(true) => {} // still spendable — not this participant's fault
            Ok(false) => {
                // Spent (incl. mempool) since registration → double-spend griefing.
                ban_list.ban(utxo_str, now, ban_duration);
                let entry = BanEntry { banned_at: now, expires_at: now + ban_duration_secs };
                if let Err(e) = append_ban_entry(ban_file_path, utxo_str, &entry) {
                    warn!(ban_file = ban_file_path, "Failed to append ban entry: {e}");
                }
                banned += 1;
            }
            Err(e) => {
                // Can't determine spentness — do NOT ban on uncertainty (never ban
                // an honest participant for a coordinator-side RPC failure).
                warn!(round_id = %round_id_str, "re-validation gettxout failed: {e}");
            }
        }
    }

    // H2 follow-up 1: attributed vs unattributed churn control.
    if banned > 0 {
        // Progress made — a griefer was banned and can't recur. Restart immediately.
        blame_round_count.store(0, Ordering::Relaxed);
    } else {
        // Nothing attributed: this failure will recur next round. Count it, and once
        // the cap is hit arm the FullAbort backoff so the re-armer stops fast-churning.
        let n = blame_round_count.fetch_add(1, Ordering::Relaxed) + 1;
        let backoff = config.coordinator.blame_full_abort_backoff_secs;
        if n >= CONSECUTIVE_BLAME_CAP && backoff > 0 {
            round_paused_until.store(now + backoff, Ordering::Relaxed);
            blame_round_count.store(0, Ordering::Relaxed);
            warn!(
                round_id = %round_id_str, backoff_secs = backoff,
                "unattributed broadcast failures hit the blame cap — pausing round re-armer"
            );
        }
    }

    warn!(
        round_id = %round_id_str,
        banned,
        checked = outpoints.len(),
        "broadcast failed — re-validated inputs and attributed blame for spent coins"
    );

    // End the wedged round: Signing→Blame→Idle (zeroes sensitive state). The phase
    // monitor re-arms a fresh round on its next tick (unless paused above).
    let _ = state.transition_to(Phase::Blame);
    let _ = state.transition_to(Phase::Idle);
}

/// Build the canonical CoinJoin PSBT from the round's registered inputs/outputs.
///
/// This is the SINGLE source of the transaction that gets both (a) verified
/// against at partial-signature submission (H3) and (b) broadcast once all
/// signatures are collected. Both call sites MUST use this function on the same
/// `inner` so the sighash a participant signs is exactly the sighash that is
/// spent — a divergence would make signature verification meaningless. Inputs
/// carry `witness_utxo` (value + on-chain script_pubkey from validate_utxo's
/// gettxout), required for sighash computation.
fn build_canonical_psbt(
    inner: &RoundStateInner,
    config: &CoordinatorConfig,
    round_id_str: &str,
) -> Result<Psbt, ApiError> {
    // M1: config was validated at startup, so the network string is known-good here.
    let bitcoin_network = crate::config::parse_bitcoin_network(&config.network.bitcoin_network)
        .expect("bitcoin_network validated in CoordinatorConfig::validate");

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
            value_sats: reg.value_sats,
            script_pubkey: reg.script_pubkey.clone(),
            change_address: change_script,
            script_type: reg.script_type,
        });
    }

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

    // WR-04 invariant: `output_script_type` MUST come from the SAME source as the
    // get_tx call site (`config.bip.output_script_type`) so the broadcast PSBT is
    // byte-identical to the one clients signed against in the display path.
    build_coinjoin_psbt(
        &participant_inputs,
        &participant_outputs,
        config.coordinator.denomination_sats,
        config.coordinator.fee_rate_sat_per_vbyte,
        config.bip.output_script_type,
    ).map_err(|e| ApiError {
        code: ErrorCode::BroadcastRejected,
        message: format!("PSBT construction failed: {e}"),
        round_id: Some(round_id_str.to_string()),
    })
}

/// Assemble the CoinJoin TX from registered inputs/outputs and broadcast.
#[allow(clippy::too_many_arguments)]
async fn assemble_and_broadcast(
    state: &mut RoundState,
    rpc: &BitcoinRpc,
    config: &CoordinatorConfig,
    ban_list: &mut BanList,
    blame_round_count: &AtomicU32,
    round_paused_until: &AtomicU64,
    round_id_str: &str,
) -> Result<String, ApiError> {
    use bitcoin::consensus::encode::serialize_hex;
    use crate::round::state::Phase;

    let inner = state.inner.as_ref().ok_or_else(|| ApiError {
        code: ErrorCode::WrongPhase,
        message: "No round state".into(),
        round_id: Some(round_id_str.to_string()),
    })?;

    let mut psbt = build_canonical_psbt(inner, config, round_id_str)?;

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

                    // Rule 2 (missing critical functionality): P2SH-P2WPKH inputs require
                    // BOTH a witness (sig + pubkey) AND a scriptSig (the redeem script push).
                    // The client submits only the witness; the coordinator reconstructs the
                    // scriptSig from the witness pubkey element and the P2SH outer script.
                    //
                    // Detection: psbt.inputs[i].witness_utxo.script_pubkey.is_p2sh() AND
                    // the witness has 2 items (ECDSA sig, compressed pubkey — standard
                    // P2SH-P2WPKH structure).
                    //
                    // Reconstruction: compressed_pubkey → hash160 → OP_0 <hash160> is the
                    // inner P2WPKH redeem script. script_sig = push(redeem_script).
                    // This is deterministic from the pubkey alone; no key material needed.
                    if let Some(ref witness_utxo) = psbt.inputs[i].witness_utxo {
                        let spk = &witness_utxo.script_pubkey;
                        let witness_ref = psbt.inputs[i].final_script_witness.as_ref().unwrap();
                        if spk.is_p2sh() && witness_ref.len() == 2 {
                            // witness[1] is the compressed pubkey (33 bytes).
                            let pubkey_bytes = witness_ref.nth(1);
                            if let Some(pk_bytes) = pubkey_bytes {
                                if pk_bytes.len() == 33 {
                                    use bitcoin::hashes::{hash160, Hash};
                                    let wpkh = hash160::Hash::hash(pk_bytes);
                                    // Redeem script: OP_0 OP_PUSHBYTES_20 <20-byte-hash>
                                    let mut redeem = vec![0x00u8, 0x14];
                                    redeem.extend_from_slice(wpkh.as_byte_array());
                                    // scriptSig: push of the 22-byte redeem script
                                    // OP_PUSHBYTES_22 = 0x16
                                    let mut script_sig_bytes = vec![0x16u8];
                                    script_sig_bytes.extend_from_slice(&redeem);
                                    psbt.inputs[i].final_script_sig =
                                        Some(bitcoin::ScriptBuf::from_bytes(script_sig_bytes));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let preview: String = sig_bytes.iter().take(16)
                        .map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("");
                    return Err(ApiError {
                        code: ErrorCode::BroadcastRejected,
                        message: format!(
                            "Invalid witness data for input {} (len={}, prefix={}, err={})",
                            i, sig_bytes.len(), preview, e
                        ),
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
        // H2: a rejected CoinJoin is the double-spend griefer's signature. Attribute
        // blame to whoever spent their registered input, ban them, and end the round.
        blame_broadcast_failure(
            state, rpc, ban_list, blame_round_count, round_paused_until, config, round_id_str,
        ).await;
        return Err(ApiError {
            code: ErrorCode::BroadcastRejected,
            message: format!("TX rejected by mempool: {reject_reason}"),
            round_id: Some(round_id_str.to_string()),
        });
    }

    // Broadcast
    let txid = match rpc.sendrawtransaction(&tx_hex).await {
        Ok(txid) => txid,
        Err(e) => {
            // H2: same treatment as a testmempoolaccept rejection — a coin was spent
            // between the accept check and broadcast, or the node rejected it.
            blame_broadcast_failure(
                state, rpc, ban_list, blame_round_count, round_paused_until, config, round_id_str,
            ).await;
            return Err(ApiError {
                code: ErrorCode::BroadcastRejected,
                message: format!("Broadcast failed: {e}"),
                round_id: Some(round_id_str.to_string()),
            });
        }
    };

    // ALLOWED to log txid — it's public info (T-04-05)
    info!(txid = %txid, round_id = %round_id_str, "CoinJoin TX broadcast");

    // H3: a successful broadcast is the only thing that makes the blame rounds truly
    // "consecutive" — reset the counter so an earlier blamed round doesn't carry
    // over toward the FullAbort cap across a success.
    blame_round_count.store(0, Ordering::Relaxed);

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
        .and_then(|a| a.require_network(expected_network))
        .map(|a| a.script_pubkey())
        .map_err(|e| format!("Invalid address '{}': {}", addr_str, e))
}

// TEST-06: Signing unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::state::{Phase, RoundState, RoundStateInner, RegisteredInput, RegisteredOutput};
    use crate::round::manager::generate_session_token;
    use crate::bitcoin::rpc::BitcoinRpc;
    use crate::config::CoordinatorConfig;
    use crate::blind::rsa::RsaBlindSigner;
    use bitcoin::secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
    use bitcoin::sighash::{EcdsaSighashType, SighashCache};
    use bitcoin::hashes::Hash;
    use bitcoin::{Address, Amount, CompressedPublicKey, Network, OutPoint, Txid, Witness};
    use std::collections::{HashMap, HashSet};
    use std::str::FromStr;

    fn test_txid() -> Txid {
        Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001"
        ).unwrap()
    }

    /// Fixed fixture key controlling the registered P2WPKH input. Used both to
    /// build the input's script_pubkey/address in `make_signing_state` and to
    /// produce a valid signature in the happy-path test.
    fn fixture_sk() -> SecretKey {
        SecretKey::from_slice(&[0x77u8; 32]).expect("valid key")
    }

    fn fixture_input_spk() -> (ScriptBuf, CompressedPublicKey) {
        let secp = Secp256k1::new();
        let cpk = CompressedPublicKey(PublicKey::from_secret_key(&secp, &fixture_sk()));
        (ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash()), cpk)
    }

    /// A valid signet address string derived from an arbitrary key (for change /
    /// output fields that `build_canonical_psbt` must parse).
    fn signet_addr(fill: u8) -> String {
        let secp = Secp256k1::new();
        let cpk = CompressedPublicKey(PublicKey::from_secret_key(
            &secp,
            &SecretKey::from_slice(&[fill; 32]).unwrap(),
        ));
        let spk = ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash());
        Address::from_script(&spk, Network::Signet).unwrap().to_string()
    }

    /// Build a state in Signing phase via production transitions:
    /// Idle → InputReg (via start_round) → OutputReg → Signing. Inserts one
    /// RegisteredInput so process_sign's UTXO-registered check passes. The
    /// returned round_secret is copied from inner so the caller can mint
    /// matching session tokens.
    fn make_signing_state(utxo_str: &str) -> (RoundState, [u8; 32]) {
        let mut state = RoundState::new_idle();
        crate::round::manager::start_round(&mut state).expect("start_round");
        // Insert one registered input + one registered output so the fixture is
        // rich enough for build_canonical_psbt to succeed (H3 verification runs
        // against that canonical tx). Real on-chain script_pubkey from the fixture
        // key; value > denomination (1_000_000) + fee so the PSBT builds.
        let (input_spk, _cpk) = fixture_input_spk();
        {
            let inner = state.inner.as_mut().expect("inner populated by start_round");
            inner.registered_inputs.insert(utxo_str.to_string(), RegisteredInput {
                utxo_str: utxo_str.to_string(),
                change_address: signet_addr(0x78),
                blind_sig_hash: [0u8; 32],
                script_pubkey: input_spk,
                value_sats: 1_100_000,
                script_type: shared::bip322::ScriptType::P2wpkh,
            });
            inner.registered_outputs.push(RegisteredOutput {
                address: signet_addr(0x79),
                amount_sats: 1_000_000,
            });
        }
        state.participant_count = 1;
        state.transition_to(Phase::OutputReg).expect("InputReg→OutputReg");
        state.transition_to(Phase::Signing).expect("OutputReg→Signing");
        let round_secret = state.inner.as_ref().expect("inner").round_secret;
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
        let mut ban_list = BanList::new();

        let result = process_sign(
            &mut state, &rpc, &config, &mut ban_list,
            &AtomicU32::new(0), &AtomicU64::new(0), &utxo,
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
        let mut ban_list = BanList::new();

        let result = process_sign(
            &mut state, &rpc, &config, &mut ban_list,
            &AtomicU32::new(0), &AtomicU64::new(0), &wrong_utxo,
            &[1, 2, 3], &token_for_wrong_utxo, "test-round"
        ).await;

        // utxo:1 passes token check but is not registered — SessionInvalid
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, shared::errors::ErrorCode::SessionInvalid);
    }

    /// H3: a structurally-valid-looking but cryptographically invalid partial
    /// signature is rejected at submission (InvalidSignature) and NOT recorded, so
    /// the sender stays a bannable non-signer rather than silently poisoning the
    /// round until broadcast.
    #[tokio::test]
    async fn test_process_sign_rejects_invalid_signature() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

        let rpc = BitcoinRpc::new(
            "http://127.0.0.1:38332".into(), "user".into(), "pass".into()
        );
        let config = CoordinatorConfig::with_defaults();
        let token = generate_session_token(&secret, &utxo);
        state.participant_count = 2;

        // A well-formed 2-element witness whose signature is garbage.
        let mut bogus = Witness::new();
        bogus.push(vec![0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x01]); // junk DER + ALL flag
        bogus.push(fixture_input_spk().1.to_bytes());
        let bogus_bytes = bitcoin::consensus::serialize(&bogus);
        let mut ban_list = BanList::new();

        let result = process_sign(
            &mut state, &rpc, &config, &mut ban_list,
            &AtomicU32::new(0), &AtomicU64::new(0), &utxo, &bogus_bytes, &token, "test-round",
        ).await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, shared::errors::ErrorCode::InvalidSignature);
        assert!(
            !state.inner.as_ref().unwrap().partial_sigs.contains_key(&utxo_str),
            "rejected signature must NOT be recorded",
        );
    }

    /// H3 happy path: a valid signature over the canonical CoinJoin tx is accepted
    /// and recorded. participant_count=2 with 1 sig collected → Recorded (no
    /// broadcast attempt, so no bitcoind needed).
    #[tokio::test]
    async fn test_process_sign_records_valid_signature() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

        let rpc = BitcoinRpc::new(
            "http://127.0.0.1:38332".into(), "user".into(), "pass".into()
        );
        let config = CoordinatorConfig::with_defaults();
        let token = generate_session_token(&secret, &utxo);
        state.participant_count = 2;

        // Sign input 0 of the SAME canonical tx process_sign will rebuild.
        let (input_spk, cpk) = fixture_input_spk();
        let canonical = build_canonical_psbt(
            state.inner.as_ref().unwrap(), &config, "test-round",
        ).expect("canonical psbt");
        let idx = canonical.unsigned_tx.input.iter()
            .position(|i| i.previous_output == utxo).expect("our input present");
        let secp = Secp256k1::new();
        let sighash = SighashCache::new(&canonical.unsigned_tx)
            .p2wpkh_signature_hash(idx, &input_spk, Amount::from_sat(1_100_000), EcdsaSighashType::All)
            .unwrap();
        let sig = secp.sign_ecdsa(&Message::from_digest(sighash.to_byte_array()), &fixture_sk());
        let mut sig_ser = sig.serialize_der().to_vec();
        sig_ser.push(EcdsaSighashType::All as u8);
        let mut witness = Witness::new();
        witness.push(sig_ser);
        witness.push(cpk.to_bytes());
        let witness_bytes = bitcoin::consensus::serialize(&witness);
        let mut ban_list = BanList::new();

        let result = process_sign(
            &mut state, &rpc, &config, &mut ban_list,
            &AtomicU32::new(0), &AtomicU64::new(0), &utxo, &witness_bytes, &token, "test-round",
        ).await;

        assert!(matches!(result, Ok(SignResult::Recorded)), "got: {result:?}");
        assert!(state.inner.as_ref().unwrap().partial_sigs.contains_key(&utxo_str));
    }

    /// H2 follow-up 1: an unattributed broadcast failure (nothing provably spent —
    /// here forced by an unreachable RPC, so every re-validation returns Err → no
    /// ban) counts toward the consecutive-blame cap but does NOT pause below it.
    #[tokio::test]
    async fn unattributed_broadcast_failure_below_cap_increments_no_pause() {
        let utxo_str = format!("{}:0", test_txid());
        let (mut state, _secret) = make_signing_state(&utxo_str);
        // Unreachable RPC → is_output_unspent_including_mempool returns Err → banned=0.
        let rpc = BitcoinRpc::new("http://127.0.0.1:1".into(), "u".into(), "p".into());
        let config = CoordinatorConfig::with_defaults();
        let blame_count = AtomicU32::new(0);
        let paused = AtomicU64::new(0);

        blame_broadcast_failure(
            &mut state, &rpc, &mut BanList::new(), &blame_count, &paused, &config, "r",
        ).await;

        assert_eq!(blame_count.load(Ordering::Relaxed), 1, "unattributed failure counts");
        assert_eq!(paused.load(Ordering::Relaxed), 0, "must not pause below the cap");
        assert_eq!(state.phase, Phase::Idle, "round still ends");
    }

    /// H2 follow-up 1: on the Nth consecutive unattributed failure (cap reached),
    /// the FullAbort backoff is armed (round_paused_until set) and the counter reset,
    /// so the re-armer stops fast-churning into the same static failure.
    #[tokio::test]
    async fn unattributed_broadcast_failure_at_cap_arms_backoff() {
        use crate::round::blame::CONSECUTIVE_BLAME_CAP;

        let utxo_str = format!("{}:0", test_txid());
        let (mut state, _secret) = make_signing_state(&utxo_str);
        let rpc = BitcoinRpc::new("http://127.0.0.1:1".into(), "u".into(), "p".into());
        let mut config = CoordinatorConfig::with_defaults();
        config.coordinator.blame_full_abort_backoff_secs = 300;
        let blame_count = AtomicU32::new(CONSECUTIVE_BLAME_CAP - 1); // one short of the cap
        let paused = AtomicU64::new(0);

        blame_broadcast_failure(
            &mut state, &rpc, &mut BanList::new(), &blame_count, &paused, &config, "r",
        ).await;

        assert!(paused.load(Ordering::Relaxed) > 0, "backoff armed at the cap");
        assert_eq!(blame_count.load(Ordering::Relaxed), 0, "counter reset after arming pause");
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
            rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
            round_secret: [0u8; 32],
            registered_inputs: {
                let mut m = HashMap::new();
                m.insert("txabc:0".to_string(), RegisteredInput {
                    utxo_str: "txabc:0".to_string(),
                    change_address: "addr".to_string(),
                    blind_sig_hash: [0u8; 32],
                    script_pubkey: bitcoin::ScriptBuf::new(),
                    value_sats: 150_000,
                    script_type: shared::bip322::ScriptType::P2wpkh,
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
            rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
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
            rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
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
            script_pubkey: bitcoin::ScriptBuf::new(), value_sats: 150_000,
            script_type: shared::bip322::ScriptType::P2wpkh,
        });
        inner.registered_inputs.insert("tx2:0".to_string(), RegisteredInput {
            utxo_str: "tx2:0".to_string(), change_address: "b".into(), blind_sig_hash: [0u8; 32],
            script_pubkey: bitcoin::ScriptBuf::new(), value_sats: 150_000,
            script_type: shared::bip322::ScriptType::P2wpkh,
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
            rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
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
