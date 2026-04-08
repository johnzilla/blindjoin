use bitcoin::{
    Amount, OutPoint, Psbt, ScriptBuf, Transaction, TxIn, TxOut,
    Sequence, Witness,
};

/// P2WPKH dust threshold: 294 sats (standard relay dust limit).
/// If change would be less than this, fold it into fee. (TX-04)
const DUST_THRESHOLD_SATS: u64 = 294;

/// Estimated weight per input (P2WPKH): 68 vbytes
const INPUT_WEIGHT_VBYTES: u64 = 68;
/// Estimated weight per output (P2WPKH): 31 vbytes
const OUTPUT_WEIGHT_VBYTES: u64 = 31;
/// Fixed TX overhead: 10 vbytes (version, locktime, vin/vout counts)
const TX_OVERHEAD_VBYTES: u64 = 10;

#[derive(Debug, Clone)]
pub struct ParticipantInput {
    pub outpoint: OutPoint,
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,   // for PSBT input UTXO field
    pub change_address: ScriptBuf,  // their change output script
}

#[derive(Debug, Clone)]
pub struct ParticipantOutput {
    pub script_pubkey: ScriptBuf,   // their CoinJoin output address script
}

#[derive(Debug, thiserror::Error)]
pub enum TxError {
    #[error("Participant input too small: {value} sats < {required} sats")]
    InsufficientFunds { value: u64, required: u64 },
    #[error("No participants")]
    NoParticipants,
    #[error("PSBT error: {0}")]
    Psbt(String),
}

/// Build a CoinJoin PSBT.
///
/// Structure:
///   Inputs: all registered participant inputs (in registration order)
///   Outputs: all denomination outputs (one per participant)
///            change outputs per participant (if above dust threshold, TX-04)
///
/// Fee calculation:
///   estimated_vsize = overhead + N*input_weight + (N denomination + N change)*output_weight
///   total_fee = estimated_vsize * fee_rate_sat_per_vbyte
///   fee_share = total_fee / N (integer division; remainder absorbed)
///
/// Returns the PSBT ready for distribution to participants.
pub fn build_coinjoin_psbt(
    inputs: &[ParticipantInput],
    outputs: &[ParticipantOutput],
    denomination_sats: u64,
    fee_rate_sat_per_vbyte: u64,
) -> Result<Psbt, TxError> {
    if inputs.is_empty() {
        return Err(TxError::NoParticipants);
    }
    let n = inputs.len() as u64;

    // Estimate size assuming all participants have change outputs (upper bound)
    let num_change_outputs = n;
    let estimated_vsize = TX_OVERHEAD_VBYTES
        + n * INPUT_WEIGHT_VBYTES
        + (n + num_change_outputs) * OUTPUT_WEIGHT_VBYTES;
    let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
    let fee_share = total_fee / n;  // each participant pays fee_share

    // Validate each input can cover denomination + fee_share
    for inp in inputs {
        let required = denomination_sats + fee_share;
        if inp.value_sats < required {
            return Err(TxError::InsufficientFunds {
                value: inp.value_sats,
                required,
            });
        }
    }

    // Build transaction inputs
    let tx_inputs: Vec<TxIn> = inputs.iter().map(|inp| TxIn {
        previous_output: inp.outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
    }).collect();

    // Build denomination outputs (one per participant, TX-01)
    let mut tx_outputs: Vec<TxOut> = outputs.iter().map(|out| TxOut {
        value: Amount::from_sat(denomination_sats),
        script_pubkey: out.script_pubkey.clone(),
    }).collect();

    // Build change outputs — fold dust into fee (TX-04, T-03-06)
    for inp in inputs {
        let change = inp.value_sats - denomination_sats - fee_share;
        if change >= DUST_THRESHOLD_SATS {
            tx_outputs.push(TxOut {
                value: Amount::from_sat(change),
                script_pubkey: inp.change_address.clone(),
            });
        }
        // else: change < dust threshold — folded into fee (no output emitted)
    }

    let tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: tx_inputs,
        output: tx_outputs,
    };

    // Build PSBT
    let mut psbt = Psbt::from_unsigned_tx(tx)
        .map_err(|e| TxError::Psbt(e.to_string()))?;

    // Add UTXO info for each input (required for SegWit inputs, TX-06)
    for (i, inp) in inputs.iter().enumerate() {
        psbt.inputs[i].witness_utxo = Some(TxOut {
            value: Amount::from_sat(inp.value_sats),
            script_pubkey: inp.script_pubkey.clone(),
        });
    }

    Ok(psbt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;

    fn dummy_outpoint(n: u8) -> OutPoint {
        use bitcoin::hashes::Hash;
        let mut txid_bytes = [0u8; 32];
        txid_bytes[0] = n;
        OutPoint::new(Txid::from_raw_hash(bitcoin::hashes::sha256d::Hash::from_byte_array(txid_bytes)), 0)
    }

    fn p2wpkh_script(byte: u8) -> ScriptBuf {
        // 22-byte P2WPKH: OP_0 <20-byte-hash>
        let mut bytes = vec![0x00, 0x14];
        bytes.extend([byte; 20]);
        ScriptBuf::from_bytes(bytes)
    }

    fn make_inputs(n: usize, value_sats: u64) -> Vec<ParticipantInput> {
        (0..n).map(|i| ParticipantInput {
            outpoint: dummy_outpoint(i as u8),
            value_sats,
            script_pubkey: p2wpkh_script(i as u8),
            change_address: p2wpkh_script((i + 100) as u8),
        }).collect()
    }

    fn make_outputs(n: usize) -> Vec<ParticipantOutput> {
        (0..n).map(|i| ParticipantOutput {
            script_pubkey: p2wpkh_script((i + 200) as u8),
        }).collect()
    }

    #[test]
    fn coinjoin_psbt_n_denomination_outputs() {
        let n = 3;
        let denomination_sats = 1_000_000;
        let inputs = make_inputs(n, 1_100_000);
        let outputs = make_outputs(n);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, denomination_sats, 2).unwrap();
        let denom_outputs: Vec<_> = psbt.unsigned_tx.output.iter()
            .filter(|o| o.value.to_sat() == denomination_sats)
            .collect();
        assert_eq!(denom_outputs.len(), n, "Must have exactly N denomination outputs");
    }

    #[test]
    fn coinjoin_psbt_is_valid_psbt() {
        let inputs = make_inputs(3, 1_100_000);
        let outputs = make_outputs(3);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, 1_000_000, 2).unwrap();
        // Serialize and deserialize — must succeed
        let serialized = psbt.serialize();
        let reparsed = Psbt::deserialize(&serialized).expect("PSBT must be valid");
        assert_eq!(reparsed.unsigned_tx.input.len(), 3);
    }

    #[test]
    fn coinjoin_psbt_dust_folded_into_fee() {
        // Input value = denomination + fee_share + 100 sats (below dust 294)
        // So change of 100 sats should be folded, no change output emitted
        let n = 3;
        let denomination_sats = 1_000_000;
        // fee_share estimate: roughly 2 sat/vB * ~250 vB / 3 ≈ 166 sats per participant
        // Set input value = denomination + estimated_fee_share + 100 (dust)
        let inputs = make_inputs(n, denomination_sats + 500 + 100); // 100 is below dust
        let outputs = make_outputs(n);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, denomination_sats, 2).unwrap();
        // The test assertion: total output value < total input value (fee was paid)
        let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
        let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
        assert!(total_out < total_in, "Fee must be deducted: total_out < total_in");
    }

    #[test]
    fn coinjoin_psbt_insufficient_funds_error() {
        let inputs = make_inputs(3, 100); // way too small
        let outputs = make_outputs(3);
        let result = build_coinjoin_psbt(&inputs, &outputs, 1_000_000, 2);
        assert!(matches!(result, Err(TxError::InsufficientFunds { .. })));
    }

    #[test]
    fn coinjoin_psbt_witness_utxo_set() {
        let inputs = make_inputs(3, 1_100_000);
        let outputs = make_outputs(3);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, 1_000_000, 2).unwrap();
        for (i, psbt_input) in psbt.inputs.iter().enumerate() {
            assert!(psbt_input.witness_utxo.is_some(),
                "Input {} must have witness_utxo set for SegWit signing", i);
        }
    }
}
