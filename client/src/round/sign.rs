use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bitcoin::psbt::Psbt;
use bitcoin::{OutPoint, ScriptBuf};
use shared::protocol::SignRequest;
use crate::http::CoordinatorClient;
use crate::wallet::ClientWallet;
use super::InputRegState;

/// Fetch the assembled PSBT, validate OUR economic outcome against it, sign our
/// input, and submit.
///
/// The coordinator is untrusted. Every check below derives from the PSBT itself —
/// never from a coordinator-reported number (fee, participant count). Refuses to
/// sign unless ALL hold:
///   - C1: our coinjoin output is present and valued at exactly the denomination
///     (a shorted output routes the difference to the coordinator).
///   - H1: the PSBT contains >= `min_anonymity_set` distinct denomination outputs
///     (our anonymity floor — a coordinator running 1 victim + sybils reports any
///     count it likes, so the count is enforced from the PSBT, not from /info).
///   - C1: the fee WE pay (our input − our denomination output − our change output,
///     all read from the PSBT) does not exceed `max_fee_sats` (catches a coordinator
///     that drops/shrinks our change and pockets it, or shifts fees onto us).
pub async fn verify_and_sign(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    state: &InputRegState,
    min_anonymity_set: u32,
    max_fee_sats: Option<u64>,
) -> Result<()> {
    // 1. Get the assembled PSBT
    let tx_resp = client.get_tx().await?;
    let psbt_bytes = B64.decode(&tx_resp.psbt)?;
    let mut psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| anyhow!("PSBT parse error: {e}"))?;

    // 2. C1: our coinjoin output is present AND valued at exactly the denomination.
    verify_own_denomination_output(&psbt, &state.output_script, state.denomination_sats)?;

    // 3. H1: client-side anonymity floor, counted from the PSBT (not coordinator-reported).
    verify_anonymity_floor(&psbt, state.denomination_sats, min_anonymity_set)?;

    // 4. C1: bound the fee WE pay, derived entirely from the PSBT.
    //    our_input value comes from our own input's witness_utxo: our signature
    //    (BIP-143 / BIP-341) commits to it, so a coordinator that lies about it
    //    produces a signature bitcoind rejects on broadcast (DoS, not theft) — safe
    //    to trust here for the fee computation.
    let our_input_value = own_input_value(&psbt, &wallet.utxo_outpoint)?;
    let our_change_value = our_change_value(&psbt, &wallet.change_address().script_pubkey());
    let fee_cap = max_fee_sats.unwrap_or(state.denomination_sats / 10);
    verify_fee_within_cap(our_input_value, state.denomination_sats, our_change_value, fee_cap)?;

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

/// C1: our coinjoin output must be present in the PSBT AND valued at exactly the
/// denomination. A coordinator that shorts our output (e.g. to dust) and routes the
/// difference to its own output would otherwise have us sign over the theft, since
/// SIGHASH_ALL commits to whatever outputs the PSBT carries and bitcoind validates
/// the result as a perfectly valid transaction.
pub fn verify_own_denomination_output(psbt: &Psbt, our_script: &ScriptBuf, denomination_sats: u64) -> Result<()> {
    let our_output = psbt.unsigned_tx.output.iter()
        .find(|o| &o.script_pubkey == our_script)
        .ok_or_else(|| anyhow!("Our output not found in PSBT — refusing to sign"))?;
    let value = our_output.value.to_sat();
    if value != denomination_sats {
        return Err(anyhow!(
            "Our output is {value} sats but the denomination is {denomination_sats} — \
             refusing to sign (coordinator shorted our output)"
        ));
    }
    Ok(())
}

/// H1: refuse to sign unless the PSBT carries at least `min_anonymity_set` distinct
/// denomination outputs (our own included). Counted from the PSBT, never from the
/// coordinator-reported participant count — that number is attacker-controlled when
/// the coordinator itself is the sybil.
pub fn verify_anonymity_floor(psbt: &Psbt, denomination_sats: u64, min_anonymity_set: u32) -> Result<()> {
    let denom_output_count = psbt.unsigned_tx.output.iter()
        .filter(|o| o.value.to_sat() == denomination_sats)
        .count() as u32;
    if denom_output_count < min_anonymity_set {
        return Err(anyhow!(
            "PSBT has {denom_output_count} denomination outputs but our anonymity floor is \
             {min_anonymity_set} — refusing to sign"
        ));
    }
    Ok(())
}

/// C1: bound the fee WE pay = our_input − our_denomination_output − our_change_output.
/// All three are read from the PSBT, so the bound holds even against a coordinator
/// that lies in its `fee_per_participant_sats` field.
fn verify_fee_within_cap(
    our_input_value: u64,
    denomination_sats: u64,
    our_change_value: u64,
    max_fee_sats: u64,
) -> Result<()> {
    let returned_to_us = denomination_sats.saturating_add(our_change_value);
    if returned_to_us > our_input_value {
        return Err(anyhow!(
            "Our outputs ({returned_to_us} sats) exceed our input ({our_input_value} sats) — \
             refusing to sign"
        ));
    }
    let our_fee = our_input_value - returned_to_us;
    if our_fee > max_fee_sats {
        return Err(anyhow!(
            "Our fee share is {our_fee} sats but the cap is {max_fee_sats} — refusing to sign \
             (possible change theft or fee shifting)"
        ));
    }
    Ok(())
}

/// Read our own input's value from its `witness_utxo` in the PSBT. Safe to trust:
/// our signature commits to this amount, so a lie yields an on-chain rejection.
fn own_input_value(psbt: &Psbt, our_outpoint: &OutPoint) -> Result<u64> {
    let idx = psbt.unsigned_tx.input.iter()
        .position(|i| &i.previous_output == our_outpoint)
        .ok_or_else(|| anyhow!("Our input not found in PSBT — refusing to sign"))?;
    let witness_utxo = psbt.inputs.get(idx)
        .and_then(|i| i.witness_utxo.as_ref())
        .ok_or_else(|| anyhow!("Our input has no witness_utxo in PSBT — refusing to sign"))?;
    Ok(witness_utxo.value.to_sat())
}

/// Sum the values of every PSBT output paying to our change script (0 if the
/// coordinator dropped our change entirely — which the fee bound then catches).
fn our_change_value(psbt: &Psbt, our_change_script: &ScriptBuf) -> u64 {
    psbt.unsigned_tx.output.iter()
        .filter(|o| &o.script_pubkey == our_change_script)
        .map(|o| o.value.to_sat())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        absolute::LockTime, transaction::Version, Amount, OutPoint, ScriptBuf,
        Transaction, TxIn, TxOut, Txid,
    };
    use std::str::FromStr;

    const DENOM: u64 = 100_000;

    /// Distinct dummy script_pubkey keyed by a tag byte (so outputs don't collide).
    fn script(tag: u8) -> ScriptBuf {
        ScriptBuf::from_bytes(vec![tag; 22])
    }

    fn outpoint(vout: u32) -> OutPoint {
        let txid = Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001",
        ).unwrap();
        OutPoint::new(txid, vout)
    }

    /// Build an unsigned PSBT with the given outputs and one input at `in_op`,
    /// optionally carrying a witness_utxo of `in_value` sats.
    fn make_psbt(outputs: Vec<(ScriptBuf, u64)>, in_op: OutPoint, in_value: Option<u64>) -> Psbt {
        let output = outputs.into_iter()
            .map(|(script_pubkey, v)| TxOut { value: Amount::from_sat(v), script_pubkey })
            .collect();
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn { previous_output: in_op, ..Default::default() }],
            output,
        };
        let mut psbt = Psbt::from_unsigned_tx(tx).expect("valid PSBT");
        if let Some(v) = in_value {
            psbt.inputs[0].witness_utxo = Some(TxOut {
                value: Amount::from_sat(v),
                script_pubkey: script(0xff),
            });
        }
        psbt
    }

    // ---- C1: own denomination output ----

    #[test]
    fn own_output_present_at_denomination_passes() {
        let psbt = make_psbt(vec![(script(1), DENOM), (script(2), DENOM)], outpoint(0), None);
        assert!(verify_own_denomination_output(&psbt, &script(1), DENOM).is_ok());
    }

    #[test]
    fn own_output_shorted_is_rejected() {
        // Coordinator gives us dust and routes the difference elsewhere.
        let psbt = make_psbt(vec![(script(1), 1_000), (script(2), DENOM)], outpoint(0), None);
        let err = verify_own_denomination_output(&psbt, &script(1), DENOM).unwrap_err().to_string();
        assert!(err.contains("shorted"), "got: {err}");
    }

    #[test]
    fn own_output_absent_is_rejected() {
        let psbt = make_psbt(vec![(script(2), DENOM)], outpoint(0), None);
        let err = verify_own_denomination_output(&psbt, &script(1), DENOM).unwrap_err().to_string();
        assert!(err.contains("not found"), "got: {err}");
    }

    // ---- H1: anonymity floor (counted from the PSBT) ----

    #[test]
    fn anonymity_floor_met_passes() {
        let psbt = make_psbt(
            vec![(script(1), DENOM), (script(2), DENOM), (script(3), DENOM)],
            outpoint(0), None,
        );
        assert!(verify_anonymity_floor(&psbt, DENOM, 3).is_ok());
    }

    #[test]
    fn anonymity_floor_unmet_is_rejected() {
        // Two denom outputs, floor of 3 — the coordinator-as-sybil case.
        let psbt = make_psbt(
            vec![(script(1), DENOM), (script(2), DENOM), (script(3), DENOM - 1)],
            outpoint(0), None,
        );
        let err = verify_anonymity_floor(&psbt, DENOM, 3).unwrap_err().to_string();
        assert!(err.contains("anonymity floor"), "got: {err}");
    }

    // ---- C1: fee bound (derived from the PSBT, not coordinator-reported) ----

    #[test]
    fn fee_within_cap_passes() {
        // input 102_000 − denom 100_000 − change 1_500 = 500 fee, cap 1_000.
        assert!(verify_fee_within_cap(102_000, DENOM, 1_500, 1_000).is_ok());
    }

    #[test]
    fn fee_change_theft_is_rejected() {
        // Coordinator shrinks our change to dust and pockets the rest:
        // input 1_000_000 − denom 100_000 − change 1_000 = 899_000 fee, cap 10_000.
        let err = verify_fee_within_cap(1_000_000, DENOM, 1_000, 10_000).unwrap_err().to_string();
        assert!(err.contains("change theft") || err.contains("fee"), "got: {err}");
    }

    #[test]
    fn fee_outputs_exceeding_input_is_rejected() {
        // denom + change > input — impossible in an honest round.
        let err = verify_fee_within_cap(100_000, DENOM, 50_000, 10_000).unwrap_err().to_string();
        assert!(err.contains("exceed"), "got: {err}");
    }

    // ---- PSBT readers feeding the fee bound ----

    #[test]
    fn own_input_value_reads_witness_utxo() {
        let psbt = make_psbt(vec![(script(1), DENOM)], outpoint(7), Some(123_456));
        assert_eq!(own_input_value(&psbt, &outpoint(7)).unwrap(), 123_456);
    }

    #[test]
    fn own_input_value_errors_without_witness_utxo() {
        let psbt = make_psbt(vec![(script(1), DENOM)], outpoint(7), None);
        assert!(own_input_value(&psbt, &outpoint(7)).is_err());
    }

    #[test]
    fn own_input_value_errors_when_input_absent() {
        let psbt = make_psbt(vec![(script(1), DENOM)], outpoint(7), Some(1));
        assert!(own_input_value(&psbt, &outpoint(9)).is_err());
    }

    #[test]
    fn change_value_sums_matching_outputs_else_zero() {
        let change = script(5);
        let psbt = make_psbt(
            vec![(script(1), DENOM), (change.clone(), 2_000), (change.clone(), 300)],
            outpoint(0), None,
        );
        assert_eq!(our_change_value(&psbt, &change), 2_300);
        assert_eq!(our_change_value(&psbt, &script(8)), 0, "no change output → 0");
    }
}
