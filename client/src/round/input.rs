use anyhow::{anyhow, Result};
use bitcoin::psbt::Psbt;
use bitcoin::{absolute, transaction, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, Witness};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use blind_rsa_signatures::{
    BlindSignature, DefaultRng,
    Sha384, PSS, Randomized,
};
use shared::token::compute_blind_token_message;
use shared::bip322::ScriptType;
use shared::protocol::{InputRegRequest, InfoResponse, OwnershipProof};
use crate::wallet::ClientWallet;
use crate::http::CoordinatorClient;
use super::InputRegState;

/// Type alias matching the coordinator's parameter set (SHA-384, PSS, Randomized).
type BjPublicKey = blind_rsa_signatures::PublicKey<Sha384, PSS, Randomized>;

/// Build the v=2 `psbt_input_b64` wire-shape from a signed BIP-322 witness +
/// an optional `final_script_sig` (P2SH-P2WPKH only).
///
/// Phase 17 17-02 D-69 (OVERRIDDEN by RESEARCH Pitfall 1 — see plan
/// `<context_override>`): the wire carries a FULL BIP-174 PSBT, NOT a bare
/// `psbt::Input`. The coordinator's decoder at
/// `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` calls
/// `bitcoin::psbt::Psbt::deserialize` and reads `psbt.inputs[0].final_script_witness`
/// (and `final_script_sig` for P2SH-P2WPKH). This helper is the byte-inverse
/// of that decoder; the encoder/decoder roundtrip test at the bottom of this
/// file asserts the contract.
///
/// Mirrors verbatim the canonical encoder at
/// `tests/integration/multi_script_validate.rs::build_v2_psbt_input_b64`
/// (LANDED in Phase 16-02), with the addition of the `final_script_sig`
/// parameter for P2SH-P2WPKH (Pitfall 7).
fn build_v2_psbt_input_b64(
    witness: &Witness,
    final_script_sig: Option<&ScriptBuf>,
) -> Result<String> {
    let unsigned_tx = Transaction {
        version: transaction::Version(2),
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![],
    };
    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)
        .map_err(|e| anyhow!("Psbt::from_unsigned_tx (v=2 envelope): {e}"))?;
    psbt.inputs[0].final_script_witness = Some(witness.clone());
    if let Some(sig) = final_script_sig {
        psbt.inputs[0].final_script_sig = Some(sig.clone());
    }
    Ok(B64.encode(psbt.serialize()))
}

pub async fn register_input(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    info: &InfoResponse,
    // 17-02 TRANSITIONAL: 17-03 will replace this 4th parameter with
    // `info.capabilities.is_legacy` read off the extended CoordinatorInfo
    // (PKARR discovery layer). For 17-02 main.rs passes `false` so a v1.4
    // client posts a v=2 OwnershipProof envelope to v1.4 coordinators by
    // default — matching the cross-impl interop assumption. When 17-03 lands
    // the discovery rejection path for legacy coordinators against non-P2WPKH
    // wallets executes BEFORE this fn is reached, so the legacy arm below
    // remains structurally unreachable for non-P2WPKH script types.
    is_legacy_coordinator: bool,
) -> Result<InputRegState> {
    // 1. Decode and verify coordinator RSA public key (T-05-01 mitigation: D-02)
    let pk_der_b64 = info.rsa_pubkey_der_b64.as_ref()
        .ok_or_else(|| anyhow!("Coordinator did not provide RSA public key in /info"))?;
    let pk_der = B64.decode(pk_der_b64)?;

    // Verify SHA-256(pk_der) == announced rsa_pubkey_hash
    let pk_hash_actual: [u8; 32] = {
        use sha2::{Sha256, Digest};
        Sha256::digest(&pk_der).into()
    };
    let announced_hash = info.rsa_pubkey_hash.as_ref()
        .ok_or_else(|| anyhow!("Coordinator did not announce RSA key hash"))?;
    let announced_bytes = hex::decode(announced_hash)?;
    if announced_bytes != pk_hash_actual {
        return Err(anyhow!("RSA public key hash mismatch — coordinator key commitment violated"));
    }

    let pk = BjPublicKey::from_spki(&pk_der)
        .map_err(|e| anyhow!("Failed to parse coordinator RSA public key: {e}"))?;

    // 2. Compute blind token message M = compute_blind_token_message(output_script, denomination)
    let output_address = wallet.coinjoin_output_address();
    let output_script = output_address.script_pubkey();
    let denomination = info.denomination_sats;
    let message_bytes = compute_blind_token_message(&output_script, denomination);

    // 3. Blind the message (D-03) using DefaultRng from blind-rsa-signatures
    let blinding_result = pk.blind(&mut DefaultRng, message_bytes)
        .map_err(|e| anyhow!("Blinding failed: {e}"))?;

    // 4. Generate BIP-322 ownership proof for the UTXO via the wallet's
    //    sign_bip322 dispatcher (Phase 17 17-02 Task 2 — replaces the prior
    //    P2WPKH-only generate_bip322_witness deleted in this commit per CD-20).
    let round_id_str = info.round_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let bip322_message = format!(
        "blindjoin:round:{}:utxo:{}:{}",
        round_id_str,
        wallet.utxo_outpoint.txid,
        wallet.utxo_outpoint.vout,
    );
    let signed: crate::wallet::Bip322SignedProof = wallet.sign_bip322(&bip322_message)?;

    // Phase 17 17-02 D-68: envelope branch on the (transitional) is_legacy
    // flag. v1 (legacy) emits byte-identical-to-v1.3 via OwnershipProof's
    // CD-7 two-phase serializer; v2 (default for v1.4 coordinators) carries
    // the corrected full-PSBT shape per Pitfall 1 + final_script_sig for
    // P2SH-P2WPKH per Pitfall 7 + script_type from wallet per CRIT-01.
    let ownership_proof_obj = if is_legacy_coordinator {
        // Legacy coordinator: pin v=1. 17-03's discovery layer rejects
        // non-P2WPKH wallets against legacy coordinators BEFORE register_input
        // is reached, so this branch is structurally unreachable for
        // non-P2WPKH; the debug_assert documents the precondition and traps
        // a regression in test builds.
        debug_assert_eq!(
            signed.script_type,
            ScriptType::P2wpkh,
            "unreachable: discovery layer must reject non-P2wpkh against legacy coordinator (17-03)"
        );
        OwnershipProof {
            version: 1,
            witness_stack: signed.witness_stack,
            psbt_input_b64: None,
            script_type: None,
        }
    } else {
        // v1.4 coordinator: emit v=2 with the full-PSBT psbt_input_b64.
        let psbt_input_b64 = build_v2_psbt_input_b64(
            &signed.witness,
            signed.final_script_sig.as_ref(),
        )?;
        OwnershipProof {
            version: 2,
            // D-70: witness_stack populated in both envelopes for symmetry.
            witness_stack: signed.witness_stack,
            psbt_input_b64: Some(psbt_input_b64),
            // CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo
            script_type: Some(signed.script_type),
        }
    };
    let ownership_proof = ownership_proof_obj.to_json_hex_str();

    // 5. POST /round/input
    let req = InputRegRequest {
        utxo_outpoint: format!("{}:{}", wallet.utxo_outpoint.txid, wallet.utxo_outpoint.vout),
        ownership_proof,
        blinded_token: B64.encode::<&[u8]>(blinding_result.blind_message.as_ref()),
        change_address: wallet.change_address().to_string(),
    };
    let resp = client.post_input(req).await?;

    // 6. Decode and verify blind signature — unblind immediately to catch bad sigs early
    let blind_sig_bytes = B64.decode(&resp.blind_signature)?;
    let blind_sig = BlindSignature(blind_sig_bytes);

    // finalize() also internally verifies the unblinded signature
    let sig = pk.finalize(&blind_sig, &blinding_result, message_bytes)
        .map_err(|e| anyhow!("Unblinding/finalization failed — blind signature invalid: {e}"))?;

    let session_token = B64.decode(&resp.session_token)?;

    Ok(InputRegState {
        round_id: resp.round_id,
        session_token,
        blinding_secret: blinding_result.secret,
        msg_randomizer: blinding_result.msg_randomizer,
        message_bytes,
        output_script,
        unblinded_sig: sig,
        pk_hash_at_registration: pk_hash_actual,
        participants_registered: info.participants_registered,
        denomination_sats: info.denomination_sats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1: encoder/decoder roundtrip via the coordinator's exact decode
    /// path — confirms `build_v2_psbt_input_b64` is the byte-inverse of
    /// `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness`.
    ///
    /// Reproduces the decoder body in-line (cannot import the
    /// coordinator-private `decode_psbt_input_witness`); the bytes flow must
    /// match exactly.
    #[test]
    fn build_v2_psbt_input_b64_roundtrips_via_coordinator_decoder() {
        let mut witness = Witness::new();
        witness.push(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        witness.push(vec![0x01, 0x02, 0x03]);

        let b64 = build_v2_psbt_input_b64(&witness, None)
            .expect("build_v2_psbt_input_b64 should succeed");

        // Pitfall 1 evidence — full-PSBT shape carries the 5-byte
        // BIP-174 magic prefix `0x70 0x73 0x62 0x74 0xff`.
        let bytes = B64.decode(&b64).expect("base64 decode");
        assert!(
            bytes.len() >= 5,
            "PSBT envelope must include BIP-174 magic prefix (full-PSBT shape per Pitfall 1)"
        );
        assert_eq!(&bytes[..5], &[0x70, 0x73, 0x62, 0x74, 0xff],
                   "wire shape must be FULL BIP-174 PSBT (Pitfall 1 fix)");

        // Coordinator decoder roundtrip
        let psbt = Psbt::deserialize(&bytes).expect("Psbt::deserialize must accept our encoding");
        assert_eq!(psbt.inputs.len(), 1, "v=2 PSBT envelope has exactly one input");
        let recovered = psbt.inputs[0]
            .final_script_witness
            .clone()
            .expect("final_script_witness present");
        assert_eq!(recovered, witness, "decoded witness must equal encoded witness");
    }

    /// Test 2: P2SH-P2WPKH path populates `final_script_sig` in the wire
    /// envelope (RESEARCH Pitfall 7).
    #[test]
    fn build_v2_psbt_input_b64_with_final_script_sig_populates_field() {
        let mut witness = Witness::new();
        witness.push(vec![0x42; 71]);
        witness.push(vec![0x99; 33]);

        // Build a representative P2SH-P2WPKH redeem-script-sig: just the
        // 22-byte witnessv0 push (the redeem-script wrapped scriptSig form).
        let sig_bytes: Vec<u8> = vec![0x16, 0x00, 0x14]
            .into_iter()
            .chain(std::iter::repeat(0xCD).take(20))
            .collect();
        let final_sig = ScriptBuf::from_bytes(sig_bytes);

        let b64 = build_v2_psbt_input_b64(&witness, Some(&final_sig))
            .expect("build_v2_psbt_input_b64 (with final_script_sig) should succeed");
        let bytes = B64.decode(&b64).expect("base64 decode");
        let psbt = Psbt::deserialize(&bytes).expect("Psbt::deserialize");
        assert_eq!(
            psbt.inputs[0].final_script_sig.as_ref(),
            Some(&final_sig),
            "P2SH-P2WPKH final_script_sig must roundtrip"
        );
        assert_eq!(
            psbt.inputs[0].final_script_witness.as_ref(),
            Some(&witness),
            "final_script_witness must roundtrip alongside final_script_sig"
        );
    }

    /// Test 3: legacy-coordinator branch emits a v=1 array-of-hex envelope
    /// (byte-identical to v1.3 via OwnershipProof::to_json_hex_str's CD-7
    /// two-phase serializer). Asserts on the JSON shape directly because
    /// register_input itself needs an HTTP client + coordinator stub (those
    /// flow tests live under 17-03's `tests/integration/multi_script_client.rs`).
    #[test]
    fn register_input_with_legacy_coordinator_emits_v1_envelope() {
        let proof = OwnershipProof {
            version: 1,
            witness_stack: vec![vec![0x30, 0x44], vec![0x02, 0x21]],
            psbt_input_b64: None,
            script_type: None,
        };
        let json = proof.to_json_hex_str();
        // CD-7 v1.3-byte-identity branch — array-of-hex form.
        assert!(
            json.starts_with('['),
            "v1.3 array-of-hex form expected (CD-7 branch); got: {json}"
        );
        assert!(
            !json.contains("\"version\""),
            "v=1 envelope must NOT carry a version field on the wire (CD-7 branch)"
        );
    }

    /// Test 4: v1.4-coordinator branch emits a v=2 flat-struct envelope with
    /// version + psbt_input_b64 + script_type fields.
    #[test]
    fn register_input_with_v14_coordinator_emits_v2_envelope() {
        let mut witness = Witness::new();
        witness.push(vec![0x64; 64]);
        let psbt_b64 = build_v2_psbt_input_b64(&witness, None)
            .expect("encoder should succeed");
        let proof = OwnershipProof {
            version: 2,
            witness_stack: vec![witness.iter().next().unwrap().to_vec()],
            psbt_input_b64: Some(psbt_b64),
            script_type: Some(ScriptType::P2tr),
        };
        let json = proof.to_json_hex_str();
        assert!(json.starts_with('{'), "v=2 envelope must be flat-struct JSON, got: {json}");
        assert!(json.contains("\"version\":2"), "v=2 envelope carries version=2, got: {json}");
        assert!(json.contains("\"script_type\":\"p2tr\""),
                "v=2 envelope carries kebab-case script_type, got: {json}");
        assert!(json.contains("\"psbt_input_b64\":\""),
                "v=2 envelope carries psbt_input_b64, got: {json}");
    }
}
