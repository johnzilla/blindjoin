use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bitcoin::psbt::Psbt;
use shared::protocol::SignRequest;
use crate::http::CoordinatorClient;
use crate::wallet::ClientWallet;
use super::InputRegState;

/// Fetch the PSBT, verify our output is present, verify denomination output count,
/// sign our input, and submit.
///
/// Security: verifies own output before signing (T-05-02 mitigation).
/// Refuses to sign if:
///   - Our output_script is absent from the PSBT outputs
///   - Fee per participant exceeds 10% of denomination
///   - Denomination output count in PSBT < participants_registered (CLI-04, T-03-01)
pub async fn verify_and_sign(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    state: &InputRegState,
    _poll_interval_ms: u64,
) -> Result<()> {
    // 1. Get the assembled PSBT
    let tx_resp = client.get_tx().await?;
    let psbt_bytes = B64.decode(&tx_resp.psbt)?;
    let mut psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| anyhow::anyhow!("PSBT parse error: {e}"))?;

    // 2. Verify our output is present (T-05-02: refuse tampered PSBT)
    let our_script = &state.output_script;
    let our_output = psbt.unsigned_tx.output.iter()
        .find(|o| &o.script_pubkey == our_script)
        .ok_or_else(|| anyhow::anyhow!("Our output not found in PSBT — refusing to sign"))?;

    // 3. Verify fee is reasonable: fee_per_participant < 10% of our output value
    if tx_resp.fee_per_participant_sats > our_output.value.to_sat() / 10 {
        return Err(anyhow::anyhow!(
            "Fee per participant ({}) exceeds 10% of output value ({}) — refusing to sign",
            tx_resp.fee_per_participant_sats,
            our_output.value.to_sat(),
        ));
    }

    // 4. CLI-04: verify PSBT has expected denomination outputs (T-03-01 anti-tampering)
    check_psbt_denomination_outputs(&psbt, state)?;

    // 5. Sign our PSBT input (T-05-04: finds input by outpoint, not index)
    let partial_sig = wallet.sign_psbt_input(&mut psbt)?;

    // 6. Submit partial signature
    let utxo_outpoint = format!("{}:{}", wallet.utxo_outpoint.txid, wallet.utxo_outpoint.vout);
    let session_token_b64 = B64.encode(&state.session_token);
    let req = SignRequest {
        round_id: state.round_id,
        utxo_outpoint,
        partial_signature: B64.encode(&partial_sig),
        session_token: session_token_b64,
    };
    client.post_sign(req).await?;
    Ok(())
}

/// CLI-04: Count denomination outputs in the PSBT and refuse to sign if the count is
/// less than the number of participants registered at input registration time.
///
/// This catches a coordinator that assembles a PSBT with fewer outputs than participants —
/// a possible output censorship or output-drop attack (T-03-01).
pub fn check_psbt_denomination_outputs(psbt: &Psbt, state: &InputRegState) -> Result<()> {
    let denomination_sats = state.denomination_sats;
    let denom_output_count = psbt.unsigned_tx.output.iter()
        .filter(|o| o.value.to_sat() == denomination_sats)
        .count() as u32;

    if denom_output_count < state.participants_registered {
        return Err(anyhow::anyhow!(
            "PSBT has {} denomination outputs but {} participants registered — \
             refusing to sign (possible output censorship)",
            denom_output_count,
            state.participants_registered,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        absolute::LockTime, transaction::Version, Amount, OutPoint, ScriptBuf,
        Transaction, TxIn, TxOut,
    };

    /// Build a minimal InputRegState with the given participants_registered and denomination_sats.
    fn make_state(participants_registered: u32, denomination_sats: u64) -> InputRegState {
        use blind_rsa_signatures::{Sha384, PSS, Randomized, DefaultRng};
        type BjKeyPair = blind_rsa_signatures::KeyPair<Sha384, PSS, Randomized>;
        type BjPublicKey = blind_rsa_signatures::PublicKey<Sha384, PSS, Randomized>;
        type BjSecretKey = blind_rsa_signatures::SecretKey<Sha384, PSS, Randomized>;

        let kp = BjKeyPair::generate(&mut DefaultRng, 2048).expect("keygen");
        let pk = BjPublicKey::from_der(&kp.pk.to_der().unwrap()).unwrap();

        // Blind a dummy message to get a valid blinding result
        let message_bytes = [0u8; 32];
        let blinding_result = pk.blind(&mut DefaultRng, message_bytes).expect("blind");

        // Sign and finalize for a valid Signature
        let sk_der = kp.sk.to_der().unwrap();
        let sk = BjSecretKey::from_der(&sk_der).unwrap();
        let blind_sig = sk.blind_sign(&blinding_result.blind_message).unwrap();
        let sig = pk.finalize(&blind_sig, &blinding_result, message_bytes).unwrap();

        InputRegState {
            round_id: uuid::Uuid::new_v4(),
            session_token: vec![0u8; 32],
            blinding_secret: blinding_result.secret,
            msg_randomizer: blinding_result.msg_randomizer,
            message_bytes,
            output_script: ScriptBuf::new(),
            unblinded_sig: sig,
            pk_hash_at_registration: [0u8; 32],
            participants_registered,
            denomination_sats,
        }
    }

    /// Build a PSBT with the specified number of denomination outputs.
    fn make_psbt_with_denom_outputs(denom_sats: u64, denom_count: usize) -> Psbt {
        let mut outputs: Vec<TxOut> = (0..denom_count)
            .map(|_| TxOut {
                value: Amount::from_sat(denom_sats),
                script_pubkey: ScriptBuf::new(),
            })
            .collect();
        // Add a change output with a different amount
        outputs.push(TxOut {
            value: Amount::from_sat(denom_sats - 1000),
            script_pubkey: ScriptBuf::new(),
        });

        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                ..Default::default()
            }],
            output: outputs,
        };
        Psbt::from_unsigned_tx(tx).expect("valid PSBT")
    }

    /// CLI-04 PASS: PSBT has exactly N denomination outputs and N participants registered.
    #[test]
    fn test_output_count_check_passes() {
        let state = make_state(3, 100_000);
        let psbt = make_psbt_with_denom_outputs(100_000, 3);
        let result = check_psbt_denomination_outputs(&psbt, &state);
        assert!(result.is_ok(), "Expected Ok but got: {:?}", result);
    }

    /// CLI-04 PASS: PSBT has more denomination outputs than participants (e.g., coordinator added extra).
    #[test]
    fn test_output_count_check_passes_with_extra_outputs() {
        let state = make_state(3, 100_000);
        let psbt = make_psbt_with_denom_outputs(100_000, 5); // 5 > 3
        let result = check_psbt_denomination_outputs(&psbt, &state);
        assert!(result.is_ok(), "Expected Ok but got: {:?}", result);
    }

    /// CLI-04 REJECT: PSBT has fewer denomination outputs than participants registered.
    #[test]
    fn test_output_count_check_rejects() {
        let state = make_state(3, 100_000);
        let psbt = make_psbt_with_denom_outputs(100_000, 2); // 2 < 3
        let result = check_psbt_denomination_outputs(&psbt, &state);
        assert!(result.is_err(), "Expected Err but got Ok");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("output censorship"),
            "Error message should mention output censorship, got: {err_msg}"
        );
        assert!(
            err_msg.contains('2') && err_msg.contains('3'),
            "Error message should include counts, got: {err_msg}"
        );
    }

    /// CLI-04 REJECT: PSBT has zero denomination outputs.
    #[test]
    fn test_output_count_check_rejects_zero_outputs() {
        let state = make_state(3, 100_000);
        let psbt = make_psbt_with_denom_outputs(100_000, 0); // 0 < 3
        let result = check_psbt_denomination_outputs(&psbt, &state);
        assert!(result.is_err(), "Expected Err but got Ok");
    }
}
