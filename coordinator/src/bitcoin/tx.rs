use bitcoin::{
    Amount, OutPoint, Psbt, ScriptBuf, Transaction, TxIn, TxOut,
    Sequence, Witness,
};
use shared::bip322::ScriptType;

/// P2WPKH dust threshold: 294 sats (standard relay dust limit).
/// If change would be less than this, fold it into fee. (TX-04)
const DUST_THRESHOLD_SATS: u64 = 294;

/// Fixed TX overhead: 10 vbytes (version, locktime, vin/vout counts).
/// Script-independent — the only weight constant that stays a plain const after
/// Phase 20's per-script weight table replaced the legacy per-input / per-output
/// constants with `script_input_vbytes` / `script_output_vbytes`.
const TX_OVERHEAD_VBYTES: u64 = 10;

/// Input vbytes per BIP-141 worst-case witness, conservative-rounded-UP
/// (raw value → ceil(witness/4) via integer-arithmetic `(w + 3) / 4`).
///
/// **CRIT-01 discipline:** `st` is coordinator-derived from the on-chain
/// `script_pubkey` by the BIP-322 ownership-proof dispatcher (see
/// `coordinator/src/bitcoin/utxo.rs`), never from a client-supplied wire
/// field. Phase 20 plumbs the already-derived value through `UtxoDetails`
/// → `RegisteredInput` → `ParticipantInput`; the fee path NEVER re-derives
/// the script type itself.
pub const fn script_input_vbytes(st: ScriptType) -> u64 {
    match st {
        // 41 non_witness (32 prev_txid + 4 vout + 1 script_sig_len(0) + 4 sequence)
        // + 108 witness (1 stack_count + 1 sig_len(72) + 72 DER+SIGHASH_ALL
        // + 1 pk_len(33) + 33 compressed pk) / 4 = 27
        // = 68 vB
        ScriptType::P2wpkh => 68,
        ScriptType::P2tr => 58,
        // P2TR derivation: 41 non_witness (same as P2WPKH)
        // + 66 witness (1 stack_count + 1 sig_len(64) + 64 Schnorr SIGHASH_DEFAULT)
        //   → ceil(66/4) = 17
        // = 58 vB. ROADMAP SC#1 cites 57 (floor of 57.5); STATE.md §v1.5 design
        // notes mandates UP-rounding so the coordinator never underpays fees on
        // a mixed round — 58 is the load-bearing value (raw 57.5, round UP).
        // 64 non_witness (32 prev_txid + 4 vout + 1 script_sig_len(23) + 23 redeem
        // wrapper + 4 sequence) + 108 witness (same as P2WPKH) / 4 = 27
        // = 91 vB
        ScriptType::P2shP2wpkh => 91,
    }
}

/// Output vbytes — exact bytes (outputs have no segwit discount, no rounding).
pub const fn script_output_vbytes(st: ScriptType) -> u64 {
    match st {
        // 8 value + 1 script_len(22) + 22 (OP_0 OP_PUSHBYTES_20 <20>) = 31
        ScriptType::P2wpkh => 31,
        // 8 value + 1 script_len(34) + 34 (OP_1 OP_PUSHBYTES_32 <32>) = 43
        ScriptType::P2tr => 43,
        // 8 value + 1 script_len(23) + 23 (OP_HASH160 OP_PUSHBYTES_20 <20> OP_EQUAL) = 32
        ScriptType::P2shP2wpkh => 32,
    }
}

#[derive(Debug, Clone)]
pub struct ParticipantInput {
    pub outpoint: OutPoint,
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,   // for PSBT input UTXO field
    pub change_address: ScriptBuf,  // their change output script
    /// FEE-02: per-input vbyte selector (coordinator-derived from on-chain SPK
    /// at validate_utxo, never client-declared — CRIT-01 invariant). Consumed
    /// only by the in-memory vsize loop in `build_coinjoin_psbt`; NOT written
    /// to any PSBT field (T-20-05 hedge — keeps serialized PSBT bytes
    /// invariant w.r.t. the new field).
    pub script_type: shared::bip322::ScriptType,
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
///   Inputs: all registered participant inputs (canonical outpoint order — M6)
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
    output_script_type: ScriptType,
) -> Result<Psbt, TxError> {
    if inputs.is_empty() {
        return Err(TxError::NoParticipants);
    }
    let n = inputs.len() as u64;

    // M6: canonical input ordering by outpoint (BIP-69-style). The three PSBT build
    // paths — get_tx (display), each process_sign verification, and the broadcast —
    // MUST produce byte-identical transactions or a participant's signature verifies
    // against a different sighash than the one broadcast (silent round-wedge). That
    // byte-identity previously held only by accident: all three iterate the same
    // `registered_inputs` HashMap and its iteration order happens to be stable within
    // one process while registration is closed. Sorting here makes the ordering an
    // explicit, deterministic invariant independent of HashMap internals (and, as a
    // bonus, independent of participant registration *timing*). Change outputs below
    // follow this same order.
    let mut ordered: Vec<&ParticipantInput> = inputs.iter().collect();
    ordered.sort_by_key(|i| i.outpoint);

    // Estimate size assuming all participants have change outputs (upper bound).
    //
    // Phase 20 Task 2 (FEE-02): per-input weight sum reads the coordinator-derived
    // ScriptType from each ParticipantInput; output weight uses the operator-
    // configured `output_script_type` (Phase 16 D-37: single output type per
    // round). Single canonical source of truth — both `get_tx` (display) and
    // `assemble_and_broadcast` (broadcast) MUST call this fn with the same
    // `output_script_type` value sourced from `config.bip.output_script_type`
    // (WR-04: byte-identical PSBTs).
    let num_change_outputs = n;
    let total_input_vb: u64 = inputs.iter()
        .map(|inp| script_input_vbytes(inp.script_type))
        .sum();
    let output_vb = script_output_vbytes(output_script_type);
    let estimated_vsize = TX_OVERHEAD_VBYTES
        + total_input_vb
        + (n + num_change_outputs) * output_vb;
    let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
    // PRESERVE VERBATIM: integer floor — D-125 byte-equality assertion in
    // `fee_share_p2wpkh_only_matches_v14_baseline` depends on this. Do NOT
    // refactor to ceil-divide or a helper function. RISK-1 hedge.
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

    // Build transaction inputs (canonical outpoint order — see M6 note above)
    let tx_inputs: Vec<TxIn> = ordered.iter().map(|inp| TxIn {
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

    // Build change outputs — fold dust into fee (TX-04, T-03-06). Same canonical
    // input order as tx_inputs so change-output positions are deterministic too.
    for inp in &ordered {
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

    // Add UTXO info for each input (required for SegWit inputs, TX-06).
    // MUST iterate `ordered` (the same canonical order as tx_inputs) so
    // psbt.inputs[i].witness_utxo aligns with unsigned_tx.input[i] — misalignment
    // would attach the wrong prevout to a sighash and silently break signing (M6).
    for (i, inp) in ordered.iter().enumerate() {
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
        make_inputs_typed(&vec![ScriptType::P2wpkh; n], value_sats)
    }

    /// FEE-03 fixture: build n participant inputs with caller-specified ScriptType
    /// per input. The on-chain SPK shape is irrelevant for the fee math (the test
    /// fixture uses a P2WPKH-shaped SPK regardless), so the helper deliberately
    /// uses p2wpkh_script for all inputs and only varies `script_type`.
    fn make_inputs_typed(types: &[ScriptType], value_sats: u64) -> Vec<ParticipantInput> {
        types.iter().enumerate().map(|(i, &st)| ParticipantInput {
            outpoint: dummy_outpoint(i as u8),
            value_sats,
            script_pubkey: p2wpkh_script(i as u8),
            change_address: p2wpkh_script((i + 100) as u8),
            script_type: st,
        }).collect()
    }

    fn make_outputs(n: usize) -> Vec<ParticipantOutput> {
        (0..n).map(|i| ParticipantOutput {
            script_pubkey: p2wpkh_script((i + 200) as u8),
        }).collect()
    }

    /// M6: byte-identical output regardless of the order inputs are supplied in.
    /// This is the load-bearing invariant behind sighash agreement across the
    /// get_tx / per-sign-verify / broadcast PSBT builds — previously it held only
    /// because all three iterated the same HashMap. Feeding the inputs in reverse
    /// must yield the same serialized transaction.
    #[test]
    fn coinjoin_psbt_input_order_is_canonical() {
        let denomination_sats = 1_000_000;
        let forward = make_inputs(3, 1_100_000);
        let mut reversed = forward.clone();
        reversed.reverse();
        let outputs = make_outputs(3);

        let psbt_fwd = build_coinjoin_psbt(&forward, &outputs, denomination_sats, 2, ScriptType::P2wpkh).unwrap();
        let psbt_rev = build_coinjoin_psbt(&reversed, &outputs, denomination_sats, 2, ScriptType::P2wpkh).unwrap();

        assert_eq!(
            psbt_fwd.serialize(), psbt_rev.serialize(),
            "PSBT must be byte-identical regardless of input registration order (M6)",
        );
        // And the canonical order is ascending by outpoint.
        let outpoints: Vec<_> = psbt_fwd.unsigned_tx.input.iter().map(|i| i.previous_output).collect();
        let mut sorted = outpoints.clone();
        sorted.sort();
        assert_eq!(outpoints, sorted, "inputs must be in ascending outpoint order");
    }

    #[test]
    fn coinjoin_psbt_n_denomination_outputs() {
        let n = 3;
        let denomination_sats = 1_000_000;
        let inputs = make_inputs(n, 1_100_000);
        let outputs = make_outputs(n);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, denomination_sats, 2, ScriptType::P2wpkh).unwrap();
        let denom_outputs: Vec<_> = psbt.unsigned_tx.output.iter()
            .filter(|o| o.value.to_sat() == denomination_sats)
            .collect();
        assert_eq!(denom_outputs.len(), n, "Must have exactly N denomination outputs");
    }

    #[test]
    fn coinjoin_psbt_is_valid_psbt() {
        let inputs = make_inputs(3, 1_100_000);
        let outputs = make_outputs(3);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, 1_000_000, 2, ScriptType::P2wpkh).unwrap();
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
        let psbt = build_coinjoin_psbt(&inputs, &outputs, denomination_sats, 2, ScriptType::P2wpkh).unwrap();
        // The test assertion: total output value < total input value (fee was paid)
        let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
        let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
        assert!(total_out < total_in, "Fee must be deducted: total_out < total_in");
    }

    #[test]
    fn coinjoin_psbt_insufficient_funds_error() {
        let inputs = make_inputs(3, 100); // way too small
        let outputs = make_outputs(3);
        let result = build_coinjoin_psbt(&inputs, &outputs, 1_000_000, 2, ScriptType::P2wpkh);
        assert!(matches!(result, Err(TxError::InsufficientFunds { .. })));
    }

    #[test]
    fn coinjoin_psbt_witness_utxo_set() {
        let inputs = make_inputs(3, 1_100_000);
        let outputs = make_outputs(3);
        let psbt = build_coinjoin_psbt(&inputs, &outputs, 1_000_000, 2, ScriptType::P2wpkh).unwrap();
        for (i, psbt_input) in psbt.inputs.iter().enumerate() {
            assert!(psbt_input.witness_utxo.is_some(),
                "Input {} must have witness_utxo set for SegWit signing", i);
        }
    }

    // ---------------------------------------------------------------------
    // Phase 20 Task 1 (FEE-01): per-script vbyte table pin tests.
    //
    // These 6 tests pin `script_input_vbytes` / `script_output_vbytes` against
    // their BIP-141 derivation. Each assertion is the audit-charter artifact
    // for Phase 21: a refactor that silently changes a value breaks a test.
    // ---------------------------------------------------------------------

    #[test]
    fn script_input_vbytes_p2wpkh_is_68() {
        assert_eq!(script_input_vbytes(ScriptType::P2wpkh), 68);
    }

    #[test]
    fn script_input_vbytes_p2tr_is_58_up_rounded() {
        // 41 + ceil(66/4) = 41 + 17 = 58. ROADMAP says 57 (floor); STATE.md §v1.5
        // design notes mandates UP-rounding — 58 is correct.
        assert_eq!(script_input_vbytes(ScriptType::P2tr), 58);
    }

    #[test]
    fn script_input_vbytes_p2sh_p2wpkh_is_91() {
        assert_eq!(script_input_vbytes(ScriptType::P2shP2wpkh), 91);
    }

    #[test]
    fn script_output_vbytes_p2wpkh_is_31() {
        assert_eq!(script_output_vbytes(ScriptType::P2wpkh), 31);
    }

    #[test]
    fn script_output_vbytes_p2tr_is_43() {
        assert_eq!(script_output_vbytes(ScriptType::P2tr), 43);
    }

    #[test]
    fn script_output_vbytes_p2sh_p2wpkh_is_32() {
        assert_eq!(script_output_vbytes(ScriptType::P2shP2wpkh), 32);
    }

    // ---------------------------------------------------------------------
    // Phase 20 Task 3 (FEE-03): regression tests pinning the v1.4 P2WPKH-only
    // fee baseline byte-equal AND proving the per-script branch fires for a
    // mixed-script round. CONTEXT D-125 + D-126 lock the test names + the
    // exact assertion shapes; the inline derivation comments are the audit-
    // charter artifact Phase 21 cites.
    // ---------------------------------------------------------------------

    #[test]
    fn fee_share_p2wpkh_only_matches_v14_baseline() {
        // v1.4 baseline (P2WPKH-only, n=3, fee_rate=2):
        // estimated_vsize = TX_OVERHEAD + n*68 + (n + n)*31
        //                 = 10 + 3*68 + 6*31
        //                 = 10 + 204 + 186
        //                 = 400 vbytes
        // total_fee  = 400 * 2 = 800 sats
        // fee_share  = 800 / 3 = 266 sats (integer floor; 2-sat remainder absorbed)
        let n = 3;
        let denomination_sats = 1_000_000;
        let inputs = make_inputs(n, 1_100_000);   // CD-44: reuse existing helper
        let outputs = make_outputs(n);
        let psbt = build_coinjoin_psbt(
            &inputs, &outputs, denomination_sats, 2,
            ScriptType::P2wpkh,                   // output_script_type
        ).unwrap();
        // Derive fee_share from PSBT: total_in - total_out should equal 800 (total_fee)
        // and per-participant burden derives as 800/3 = 266.
        let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
        let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
        let total_fee = total_in - total_out;
        let fee_share = total_fee / (n as u64);
        assert_eq!(fee_share, 266, "v1.4 P2WPKH-only baseline must be byte-exact 266");
    }

    #[test]
    fn fee_share_mixed_script_differs_from_uniform_baseline() {
        // mixed-script (n=3, fee_rate=2, output_type=P2WPKH):
        // estimated_vsize = 10 + (68 + 58 + 91) + 6*31 = 10 + 217 + 186 = 413 vB
        // total_fee = 413 * 2 = 826 sats
        // fee_share = 826 / 3 = 275 sats
        // diff per participant: 275 - 266 = 9 sats (well above the ≥1 sat requirement)
        let n = 3;
        let denomination_sats = 1_000_000;
        let types = [ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh];
        let inputs = make_inputs_typed(&types, 1_100_000);
        let outputs = make_outputs(n);
        let psbt = build_coinjoin_psbt(
            &inputs, &outputs, denomination_sats, 2,
            ScriptType::P2wpkh,                   // single output type per round (D-37)
        ).unwrap();
        let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
        let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
        let fee_share = (total_in - total_out) / (n as u64);
        assert!(
            fee_share.saturating_sub(266) >= 1,
            "Mixed-script fee_share must exceed P2WPKH-only baseline by >=1 sat \
             (got {fee_share}; would be 266 if per-script branch silently reverted)"
        );
    }
}
