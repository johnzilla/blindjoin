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

/// Outcome of the under-lock signing step (`process_sign`). M5b: `process_sign`
/// does NO network I/O — it records the signature and, once the last one arrives,
/// assembles the final transaction and moves the round `Signing → Broadcast`. The
/// caller then runs [`finalize_broadcast`] OFF the lock to actually broadcast.
#[derive(Debug)]
pub enum SignOutcome {
    /// Partial signature recorded; waiting for more participants.
    Recorded,
    /// All signatures collected and the final tx assembled; the round is now in
    /// `Broadcast`. The caller MUST run [`finalize_broadcast`] with this payload
    /// (off the write lock) to broadcast and finalize the round.
    ReadyToBroadcast(PreparedBroadcast),
}

/// Everything [`finalize_broadcast`] needs to broadcast off the lock: the fully
/// assembled tx hex and a snapshot of the registered input outpoints (for the
/// failure-path re-validation, since round state is dropped on Blame→Idle).
#[derive(Debug)]
pub struct PreparedBroadcast {
    pub tx_hex: String,
    pub input_outpoints: Vec<String>,
}

/// Under-lock signing step (M5b). Called by the handler with the round write lock
/// held. Verifies the session token and partial signature, records it, and — when
/// the final signature completes the set — assembles the canonical transaction and
/// transitions `Signing → Broadcast` so the signing-timeout monitor stops watching
/// the round. Performs NO network I/O; the actual broadcast happens off the lock in
/// [`finalize_broadcast`]. Every error before the `Signing → Broadcast` transition
/// leaves the phase in `Signing`, so the signing timeout still governs a failed
/// assembly.
pub fn process_sign(
    state: &mut RoundState,
    config: &CoordinatorConfig,
    utxo: &OutPoint,
    partial_signature: &[u8],
    session_token_bytes: &[u8; 32],
    round_id_str: &str,
) -> Result<SignOutcome, ApiError> {
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
        // All signatures collected. Assemble the final tx (pure compute, no RPC) and
        // snapshot the input outpoints for the failure-path re-validation. Any error
        // here returns Err WITHOUT leaving Signing, so the signing timeout still
        // governs a failed assembly (M5b item 1).
        let inner_ref = state.inner.as_ref().ok_or_else(|| ApiError {
            code: ErrorCode::WrongPhase,
            message: "No round state".into(),
            round_id: Some(round_id_str.to_string()),
        })?;
        let tx_hex = assemble_final_tx_hex(inner_ref, config, round_id_str)?;
        let input_outpoints: Vec<String> = inner_ref.registered_inputs.keys().cloned().collect();

        // Only now, with a valid tx in hand, mark the round in-flight. This must be
        // the LAST thing under the lock: once in Broadcast the signing-timeout monitor
        // ignores the round, so `finalize_broadcast` (off the lock) is solely
        // responsible for ending it (or the run.rs Broadcast watchdog as backstop).
        state.transition_to(crate::round::state::Phase::Broadcast).map_err(|e| ApiError {
            code: ErrorCode::BroadcastRejected,
            message: format!("Signing→Broadcast transition failed: {e}"),
            round_id: Some(round_id_str.to_string()),
        })?;
        return Ok(SignOutcome::ReadyToBroadcast(PreparedBroadcast { tx_hex, input_outpoints }));
    }

    Ok(SignOutcome::Recorded)
}

/// H2 (M5b, off-lock half): re-validate registered inputs against the mempool-aware
/// UTXO set and return the outpoints that have since been SPENT. Runs with NO locks
/// held — the caller passes a snapshot of the outpoints taken under the lock. Banning
/// is gated on ACTUAL spentness, so a transient broadcast failure (RPC hiccup, fee
/// issue) yields an empty set (nobody banned); only a participant who spent their
/// registered coin out from under the round appears here.
///
/// Bounded against a hung bitcoind (M5a ~10s per call): two consecutive RPC errors
/// mean the node is unreachable, so stop early — nobody is banned on RPC uncertainty
/// anyway, and this caps the failure path at ~2×10s.
///
/// WATCHDOG MATH INVARIANT: this loop is `max_participants` SERIAL RPCs — it sets the
/// worst-case honest finalize that `CoordinatorConfig::broadcast_watchdog_secs` must
/// exceed. Adding RPCs here (or making each call costlier) MUST re-derive that
/// formula (see the invariant comment at `finalize_broadcast`).
async fn revalidate_spent_inputs(
    rpc: &BitcoinRpc,
    outpoints: &[String],
    round_id_str: &str,
) -> Vec<String> {
    const MAX_CONSECUTIVE_RPC_ERRORS: u32 = 2;
    let mut spent = Vec::new();
    let mut consecutive_errors = 0u32;

    for utxo_str in outpoints {
        let Some(outpoint) = parse_outpoint(utxo_str) else { continue };
        match rpc
            .is_output_unspent_including_mempool(&outpoint.txid, outpoint.vout)
            .await
        {
            Ok(true) => consecutive_errors = 0, // still spendable — not this participant's fault
            Ok(false) => {
                consecutive_errors = 0;
                spent.push(utxo_str.clone());
            }
            Err(e) => {
                // Can't determine spentness — do NOT ban on uncertainty.
                warn!(round_id = %round_id_str, "re-validation gettxout failed: {e}");
                consecutive_errors += 1;
                if consecutive_errors >= MAX_CONSECUTIVE_RPC_ERRORS {
                    warn!(
                        round_id = %round_id_str,
                        "two consecutive re-validation RPC errors — bitcoind likely \
                         unreachable; stopping re-validation early"
                    );
                    break;
                }
            }
        }
    }
    spent
}

/// H2/H3 (M5b, on-lock half): apply the broadcast-failure outcome under the
/// re-acquired locks. Bans the provably-spent inputs, runs the consecutive-blame
/// counter logic, and ends the round `Broadcast → Blame → Idle`.
///
/// PRESERVED VERBATIM through the M5b split (single locus for the blame counter):
///   - attributed (banned > 0): a griefer was banned and can't recur → reset the
///     counter, restart immediately.
///   - unattributed (banned == 0): the failure (e.g. static fee too low) will recur,
///     so count it toward `CONSECUTIVE_BLAME_CAP`; on reaching the cap, arm the
///     FullAbort backoff (H3) so the re-armer stops fast-churning, and reset.
///
/// These transitions MUST stay here in the finalize path — never in `process_sign`'s
/// prepare step — or a hung broadcast could corrupt the blame bookkeeping.
fn apply_broadcast_failure_blame(
    state: &mut RoundState,
    ban_list: &mut BanList,
    blame_round_count: &AtomicU32,
    round_paused_until: &AtomicU64,
    config: &CoordinatorConfig,
    banned_utxos: &[String],
    round_id_str: &str,
) {
    use crate::round::state::Phase;

    let ban_file_path = &config.coordinator.ban_file_path;
    let ban_duration_secs = config.coordinator.blame_ban_duration_secs;
    let now = now_unix_secs();
    let ban_duration = Duration::from_secs(ban_duration_secs);

    for utxo_str in banned_utxos {
        ban_list.ban(utxo_str, now, ban_duration);
        let entry = BanEntry { banned_at: now, expires_at: now + ban_duration_secs };
        if let Err(e) = append_ban_entry(ban_file_path, utxo_str, &entry) {
            warn!(ban_file = ban_file_path, "Failed to append ban entry: {e}");
        }
    }

    let banned = banned_utxos.len();
    if banned > 0 {
        blame_round_count.store(0, Ordering::Relaxed);
    } else {
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
        "broadcast failed — attributed blame for spent coins"
    );

    // End the round: Broadcast→Blame→Idle (zeroes sensitive state). The phase
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

/// Assemble the final CoinJoin transaction hex from registered inputs/outputs and
/// the collected partial signatures. PURE COMPUTE — no RPC, no state mutation (M5b).
/// Runs under the round write lock inside `process_sign`; the actual broadcast
/// happens off the lock in [`finalize_broadcast`].
fn assemble_final_tx_hex(
    inner: &RoundStateInner,
    config: &CoordinatorConfig,
    round_id_str: &str,
) -> Result<String, ApiError> {
    use bitcoin::consensus::encode::serialize_hex;

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
    Ok(serialize_hex(&final_tx))
}

/// Broadcast the assembled CoinJoin off the round write lock, then re-acquire it to
/// finalize the round (M5b). Runs as a DETACHED task spawned by the sign handler, so
/// it completes even if the handler's HTTP connection is dropped mid-broadcast; the
/// run.rs Broadcast watchdog is the backstop if this task ever dies.
///
/// Lock discipline: the `testmempoolaccept` + `sendrawtransaction` (and, on failure,
/// the input re-validation) run with NO locks held — the whole point of M5b. Only the
/// terminal transitions re-acquire, always in `round → ban_list` order. On entry the
/// round is already in `Broadcast` (moved under the lock by `process_sign`), so the
/// signing-timeout monitor is not watching it.
///
/// The `round_id` guard on every re-acquire ensures a slow finalize that lost a race
/// with the watchdog (which force-Idles and mints a new round_id) does not touch the
/// wrong round — the tx may still be out, which is benign (the round already reset).
///
/// WATCHDOG MATH INVARIANT: any change to the number of serial RPCs in the
/// broadcast / failure path MUST re-derive `CoordinatorConfig::broadcast_watchdog_secs`
/// (`10 + max_participants × per-input-RPC-time` is its floor). The derivation comment
/// in `config.rs::broadcast_watchdog_secs` names this path — update both together or
/// the watchdog can preempt a live finalize and skip attributing a proven griefer.
#[allow(clippy::too_many_arguments)]
pub async fn finalize_broadcast(
    round: std::sync::Arc<tokio::sync::RwLock<RoundState>>,
    rpc: std::sync::Arc<BitcoinRpc>,
    ban_list: std::sync::Arc<tokio::sync::RwLock<BanList>>,
    blame_round_count: std::sync::Arc<AtomicU32>,
    round_paused_until: std::sync::Arc<AtomicU64>,
    config: std::sync::Arc<CoordinatorConfig>,
    prepared: PreparedBroadcast,
    round_id: uuid::Uuid,
) -> Result<String, ApiError> {
    use crate::round::state::Phase;

    let round_id_str = round_id.to_string();
    let tx_hex = prepared.tx_hex;

    // ---- Network I/O: NO locks held (M5b core) ----
    let outcome: Result<String, String> = match rpc.testmempoolaccept(&[&tx_hex]).await {
        Err(e) => Err(format!("testmempoolaccept failed: {e}")),
        Ok(accept_result) => {
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
                Err(format!("TX rejected by mempool: {reject_reason}"))
            } else {
                match rpc.sendrawtransaction(&tx_hex).await {
                    Ok(txid) => Ok(txid.to_string()),
                    Err(e) => Err(format!("Broadcast failed: {e}")),
                }
            }
        }
    };

    match outcome {
        Ok(txid) => {
            // ALLOWED to log txid — it's public info (T-04-05)
            info!(txid = %txid, round_id = %round_id_str, "CoinJoin TX broadcast");
            // Re-acquire to finalize success. H3 counter reset stays here in finalize
            // (single locus for the blame counter — never in prepare).
            let mut guard = round.write().await;
            if guard.round_id == round_id && guard.phase == Phase::Broadcast {
                blame_round_count.store(0, Ordering::Relaxed);
                let _ = guard.transition_to(Phase::Idle);
            } else {
                warn!(
                    round_id = %round_id_str, phase = guard.phase.as_str(),
                    "broadcast succeeded but round already advanced (watchdog?) — tx is out, no-op"
                );
            }
            Ok(txid)
        }
        Err(msg) => {
            // Failure: re-validate inputs OFF the lock, then re-acquire to attribute
            // blame and end the round.
            let banned = revalidate_spent_inputs(&rpc, &prepared.input_outpoints, &round_id_str).await;
            let mut guard = round.write().await;
            if guard.round_id == round_id && guard.phase == Phase::Broadcast {
                let mut bl = ban_list.write().await; // order: round → ban_list
                apply_broadcast_failure_blame(
                    &mut guard, &mut bl, &blame_round_count, &round_paused_until,
                    &config, &banned, &round_id_str,
                );
            } else {
                warn!(
                    round_id = %round_id_str, phase = guard.phase.as_str(),
                    "broadcast failed but round already advanced — skipping blame"
                );
            }
            Err(ApiError {
                code: ErrorCode::BroadcastRejected,
                message: msg,
                round_id: Some(round_id_str),
            })
        }
    }
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
    #[test]
    fn test_process_sign_invalid_session_token() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, _secret) = make_signing_state(&utxo_str);

        let config = CoordinatorConfig::with_defaults();
        let wrong_token = [0x00u8; 32];

        let result = process_sign(
            &mut state, &config, &utxo, &[1, 2, 3], &wrong_token, "test-round",
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, shared::errors::ErrorCode::SessionInvalid);
    }

    /// TEST-06: Wrong outpoint rejected — token valid for utxo:0 but submitted as utxo:1
    #[test]
    fn test_process_sign_wrong_outpoint() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

        let config = CoordinatorConfig::with_defaults();

        // Correct token for utxo:0 but submit as utxo:1 (wrong outpoint)
        let wrong_utxo = OutPoint::new(txid, 1);
        let _token_for_correct_utxo = generate_session_token(&secret, &utxo); // token for :0

        // Token generated for utxo:1 but utxo:1 is not registered → token check fails
        // (token for utxo:1 is different from token for utxo:0, and utxo:1 is not registered)
        let token_for_wrong_utxo = generate_session_token(&secret, &wrong_utxo);

        let result = process_sign(
            &mut state, &config, &wrong_utxo, &[1, 2, 3], &token_for_wrong_utxo, "test-round",
        );

        // utxo:1 passes token check but is not registered — SessionInvalid
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, shared::errors::ErrorCode::SessionInvalid);
    }

    /// H3: a structurally-valid-looking but cryptographically invalid partial
    /// signature is rejected at submission (InvalidSignature) and NOT recorded, so
    /// the sender stays a bannable non-signer rather than silently poisoning the
    /// round until broadcast.
    #[test]
    fn test_process_sign_rejects_invalid_signature() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

        let config = CoordinatorConfig::with_defaults();
        let token = generate_session_token(&secret, &utxo);
        state.participant_count = 2;

        // A well-formed 2-element witness whose signature is garbage.
        let mut bogus = Witness::new();
        bogus.push(vec![0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01, 0x01]); // junk DER + ALL flag
        bogus.push(fixture_input_spk().1.to_bytes());
        let bogus_bytes = bitcoin::consensus::serialize(&bogus);

        let result = process_sign(
            &mut state, &config, &utxo, &bogus_bytes, &token, "test-round",
        );

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
    #[test]
    fn test_process_sign_records_valid_signature() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);

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

        let result = process_sign(
            &mut state, &config, &utxo, &witness_bytes, &token, "test-round",
        );

        assert!(matches!(result, Ok(SignOutcome::Recorded)), "got: {result:?}");
        assert!(state.inner.as_ref().unwrap().partial_sigs.contains_key(&utxo_str));
    }

    /// M5b race-safety invariant: when the FINAL signature completes the set,
    /// `process_sign` assembles the tx and moves the round `Signing → Broadcast`
    /// under the lock, returning `ReadyToBroadcast` — BEFORE any broadcast RPC. Once
    /// in Broadcast the signing-timeout monitor (`phase != Signing → no-op`) no longer
    /// watches the round, so the off-lock broadcast can never race a blame. This is
    /// the deterministic core of "no signing-timeout blame fires during a slow
    /// broadcast"; the full round converging is covered by the e2e tests.
    #[test]
    fn process_sign_last_signature_moves_round_to_broadcast() {
        let txid = test_txid();
        let utxo = OutPoint::new(txid, 0);
        let utxo_str = format!("{}:0", txid);
        let (mut state, secret) = make_signing_state(&utxo_str);
        let config = CoordinatorConfig::with_defaults();
        let token = generate_session_token(&secret, &utxo);
        // make_signing_state sets participant_count = 1 → this one sig completes the set.
        assert_eq!(state.participant_count, 1);

        let (input_spk, cpk) = fixture_input_spk();
        let canonical = build_canonical_psbt(state.inner.as_ref().unwrap(), &config, "r")
            .expect("canonical psbt");
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

        let result = process_sign(&mut state, &config, &utxo, &witness_bytes, &token, "r");
        assert!(
            matches!(result, Ok(SignOutcome::ReadyToBroadcast(_))),
            "final signature must return ReadyToBroadcast; got {result:?}",
        );
        assert_eq!(
            state.phase, Phase::Broadcast,
            "round must move Signing→Broadcast under the lock, before any broadcast RPC (M5b)",
        );
    }

    /// Move a fresh signing-state fixture into the in-flight `Broadcast` phase, the
    /// state `apply_broadcast_failure_blame` runs from in production (M5b).
    fn broadcasting_state() -> RoundState {
        let (mut state, _secret) = make_signing_state(&format!("{}:0", test_txid()));
        state.transition_to(Phase::Broadcast).expect("Signing→Broadcast");
        state
    }

    /// H2 follow-up 1: an unattributed broadcast failure (nobody provably spent —
    /// `banned == []`) counts toward the consecutive-blame cap but does NOT pause
    /// below it, and ends the round.
    #[test]
    fn unattributed_broadcast_failure_below_cap_increments_no_pause() {
        let mut state = broadcasting_state();
        let config = CoordinatorConfig::with_defaults();
        let blame_count = AtomicU32::new(0);
        let paused = AtomicU64::new(0);

        apply_broadcast_failure_blame(
            &mut state, &mut BanList::new(), &blame_count, &paused, &config, &[], "r",
        );

        assert_eq!(blame_count.load(Ordering::Relaxed), 1, "unattributed failure counts");
        assert_eq!(paused.load(Ordering::Relaxed), 0, "must not pause below the cap");
        assert_eq!(state.phase, Phase::Idle, "round still ends");
    }

    /// H2 follow-up 1: on the Nth consecutive unattributed failure (cap reached),
    /// the FullAbort backoff is armed (round_paused_until set) and the counter reset.
    #[test]
    fn unattributed_broadcast_failure_at_cap_arms_backoff() {
        use crate::round::blame::CONSECUTIVE_BLAME_CAP;

        let mut state = broadcasting_state();
        let mut config = CoordinatorConfig::with_defaults();
        config.coordinator.blame_full_abort_backoff_secs = 300;
        let blame_count = AtomicU32::new(CONSECUTIVE_BLAME_CAP - 1); // one short of the cap
        let paused = AtomicU64::new(0);

        apply_broadcast_failure_blame(
            &mut state, &mut BanList::new(), &blame_count, &paused, &config, &[], "r",
        );

        assert!(paused.load(Ordering::Relaxed) > 0, "backoff armed at the cap");
        assert_eq!(blame_count.load(Ordering::Relaxed), 0, "counter reset after arming pause");
    }

    /// H2 follow-up 1 (attributed): a broadcast failure that DID attribute a spent
    /// input (`banned` non-empty) resets the counter and restarts immediately.
    #[test]
    fn attributed_broadcast_failure_resets_counter() {
        let mut state = broadcasting_state();
        let config = CoordinatorConfig::with_defaults();
        let blame_count = AtomicU32::new(1); // a prior unattributed failure
        let paused = AtomicU64::new(0);
        let mut ban_list = BanList::new();

        apply_broadcast_failure_blame(
            &mut state, &mut ban_list, &blame_count, &paused, &config,
            &["deadbeef:0".to_string()], "r",
        );

        assert_eq!(blame_count.load(Ordering::Relaxed), 0, "attribution resets the counter");
        assert_eq!(paused.load(Ordering::Relaxed), 0, "attributed failure does not pause");
        assert!(ban_list.is_banned("deadbeef:0", now_unix_secs() + 5), "spent input banned");
        assert_eq!(state.phase, Phase::Idle);
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
                    script_pubkey: bitcoin::ScriptBuf::new(),
                    value_sats: 150_000,
                    script_type: shared::bip322::ScriptType::P2wpkh,
                });
                m
            },
            redeemed_tokens: HashSet::new(),
            registered_outputs: vec![],
            partial_sigs: HashMap::new(), // No partial sig for txabc:0 → non-signer
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
        };
        // 2 inputs, 1 output → missing output
        inner.registered_inputs.insert("tx1:0".to_string(), RegisteredInput {
            utxo_str: "tx1:0".to_string(), change_address: "a".into(),
            script_pubkey: bitcoin::ScriptBuf::new(), value_sats: 150_000,
            script_type: shared::bip322::ScriptType::P2wpkh,
        });
        inner.registered_inputs.insert("tx2:0".to_string(), RegisteredInput {
            utxo_str: "tx2:0".to_string(), change_address: "b".into(),
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
        });
        let mut ban_list = BanList::new();
        on_signing_timeout(&mut state, &mut ban_list, "/dev/null", 3600, 0);
        // After blame, round is Idle and ready for the next round
        assert_eq!(state.phase, Phase::Idle);
        assert!(state.inner.is_none());
    }
}
