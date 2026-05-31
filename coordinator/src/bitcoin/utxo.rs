use std::collections::HashSet;

use base64::Engine;
use bitcoin::{Network, OutPoint, Script, ScriptBuf, Witness};
use shared::bip322::{detect_script_type, verify_simple, Bip322Error, ScriptType};
use shared::protocol::OwnershipProof;

use crate::bitcoin::rpc::{BitcoinRpc, RpcError};
use crate::config::BipConfig;

// Phase 16: per-script verify lives in shared::bip322; the dispatcher in
// validate_utxo below is the only entry point. Per the v1.4 ADR, the script
// type is derived from the on-chain SPK, never from a client-supplied field.

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Debug, thiserror::Error)]
pub enum UtxoError {
    #[error("UTXO not found or already spent")]
    NotFound,
    #[error("UTXO already registered in this round")]
    AlreadyRegistered,
    #[error("UTXO value {value} sats insufficient (need {required} sats)")]
    InsufficientValue { value: u64, required: u64 },
    #[error("Invalid BIP-322 ownership proof: {reason}")]
    InvalidProof { reason: String },
    #[error("Bitcoin Core unreachable: {0}")]
    RpcUnavailable(String),
}

impl From<RpcError> for UtxoError {
    fn from(e: RpcError) -> Self {
        UtxoError::RpcUnavailable(e.to_string())
    }
}

pub struct UtxoDetails {
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,
    /// Coordinator-derived script type from the on-chain `script_pubkey`,
    /// computed inside `dispatch_ownership_proof` (CRIT-01 invariant: NEVER
    /// client-declared). Threaded through to the fee path via `RegisteredInput`
    /// → `ParticipantInput` (FEE-02 plumbing).
    pub script_type: ScriptType,
}

/// Validate a UTXO registration request.
///
/// Checks in order:
/// 1. Not already registered in this round (double-registration prevention, UTXO-03)
/// 2. Exists and is unspent (via Bitcoin Core RPC, UTXO-01)
/// 3. Value >= denomination + fee_share_sats (UTXO-02)
/// 4. BIP-322 ownership proof valid via the multi-script dispatcher (UTXO-04).
///    The dispatcher routes `OwnershipProof.version` to a v=1 (legacy
///    witness-only) or v=2 (PSBT-input) branch. BOTH branches derive
///    `ScriptType` from the on-chain script_pubkey (load-bearing security
///    invariant — see inline comments in each version arm of the match) and
///    check the `BipConfig` allowlist before calling
///    `shared::bip322::verify_simple`. v=2 additionally cross-checks
///    `declared == derived` BEFORE verify.
///
/// `bip_config` carries the operator-configured allowlist (D-51 / CD-14).
/// `network` is threaded from `CoordinatorConfig::network::bitcoin_network`
/// once at startup (D-51); the per-script verifier passes it to the bip322
/// crate's `Address::from_script` step regardless of script type.
#[allow(clippy::too_many_arguments)]
pub async fn validate_utxo(
    rpc: &BitcoinRpc,
    utxo: &OutPoint,
    registered_inputs: &HashSet<OutPoint>,
    denomination_sats: u64,
    fee_share_sats: u64,
    ownership_proof: &OwnershipProof,
    bip_config: &BipConfig,
    network: Network,
    round_id: &str,
) -> Result<UtxoDetails, UtxoError> {
    // 1. Double-registration check (T-03-02: prevents double-spend of same UTXO)
    if registered_inputs.contains(utxo) {
        return Err(UtxoError::AlreadyRegistered);
    }

    // 2. Existence + unspent check (UTXO-01)
    let txout = rpc.gettxout(&utxo.txid, utxo.vout).await?;
    let txout = txout.ok_or(UtxoError::NotFound)?;

    // 3. Value check (UTXO-02)
    // corepc_types GetTxOut (v17/v26) has value as f64 BTC; convert to sats
    // Use bitcoin::Amount::from_btc for correct decimal-to-satoshi conversion
    // without floating-point truncation errors.
    let value_sats = bitcoin::Amount::from_btc(txout.value)
        .map_err(|e| UtxoError::InvalidProof { reason: format!("BTC amount parse: {e}") })?
        .to_sat();
    let required = denomination_sats + fee_share_sats;
    if value_sats < required {
        return Err(UtxoError::InsufficientValue { value: value_sats, required });
    }

    // 4. BIP-322 ownership proof — multi-script dispatcher (UTXO-04, T-03-01).
    let script_pubkey = parse_script_pubkey_from_txout(&txout)
        .map_err(|e| UtxoError::InvalidProof { reason: e })?;
    let message = format!("blindjoin:round:{}:utxo:{}:{}", round_id, utxo.txid, utxo.vout);

    let derived = dispatch_ownership_proof(
        &script_pubkey,
        ownership_proof,
        network,
        bip_config,
        message.as_bytes(),
    )
    .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;

    // D-50: structured success log. Fields = round_id (Display) + script_type
    // (Debug) ONLY. No outpoint, address, witness, or pubkey bytes (PRIV-02).
    tracing::info!(
        round_id = %round_id,
        script_type = ?derived,
        "ownership proof verified"
    );

    Ok(UtxoDetails { value_sats, script_pubkey, script_type: derived })
}

/// Dispatcher core — pure function (no RPC / I/O) that takes the on-chain
/// SPK and the wire envelope and returns the derived ScriptType on success
/// or a typed `Bip322Error` on rejection. The dual-branch invariant comments
/// live in the private body (`dispatch_ownership_proof`); see those for the
/// load-bearing security note.
///
/// Extracted so unit tests (Tests 1-5 in plan 16-02 Task 1) AND integration
/// tests (`tests/integration/multi_script_validate.rs`, Plan 16-02 Task 3)
/// can assert on specific `Bip322Error` variants without (a) spinning up a
/// BitcoinRpc, or (b) parsing the `UtxoError::InvalidProof { reason }` string
/// — Phase 15-03 D-34 discipline.
///
/// Visibility is plain `pub` because the integration test binary at
/// `tests/integration/multi_script_validate.rs` lives in a separate compilation
/// unit (external-crate test target) and cannot see `#[cfg(test)]` items from
/// the coordinator lib. `#[doc(hidden)]` keeps it out of public `cargo doc`
/// output, and the name carries the `_typed` suffix to signal that production
/// callers (HTTP handlers) MUST use `validate_utxo` instead — which performs
/// the RPC + value checks and emits the success log line.
#[doc(hidden)]
pub fn validate_ownership_proof_typed(
    script_pubkey: &Script,
    ownership_proof: &OwnershipProof,
    network: Network,
    bip_config: &BipConfig,
    message: &[u8],
) -> Result<ScriptType, Bip322Error> {
    dispatch_ownership_proof(script_pubkey, ownership_proof, network, bip_config, message)
}

/// Private dispatcher body — called from `validate_utxo` (production path) and
/// from `validate_ownership_proof_typed` (test path). Centralising the body
/// guarantees the production code is bit-exact with the path the tests assert
/// on.
fn dispatch_ownership_proof(
    script_pubkey: &Script,
    ownership_proof: &OwnershipProof,
    network: Network,
    bip_config: &BipConfig,
    message: &[u8],
) -> Result<ScriptType, Bip322Error> {
    match ownership_proof.version {
        1 => {
            // CRIT-01: script_type derived from on-chain script_pubkey, never from client field
            let derived = detect_script_type(script_pubkey)?;
            if !bip_config.allows(derived) {
                return Err(Bip322Error::UnsupportedScriptType);
            }
            let witness = Witness::from_slice(&ownership_proof.witness_stack);
            verify_simple(derived, script_pubkey, &witness, message, network)?;
            Ok(derived)
        }
        2 => {
            let psbt_input_b64 = ownership_proof.psbt_input_b64.as_ref().ok_or_else(|| {
                Bip322Error::WireFormatMismatch(
                    "v2 OwnershipProof requires psbt_input_b64".into(),
                )
            })?;
            let declared = ownership_proof.script_type.ok_or_else(|| {
                Bip322Error::WireFormatMismatch(
                    "v2 OwnershipProof requires script_type field".into(),
                )
            })?;
            let witness = decode_psbt_input_witness(psbt_input_b64)?;
            // CRIT-01: script_type derived from on-chain script_pubkey, never from client field
            let derived = detect_script_type(script_pubkey)?;
            if declared != derived {
                return Err(Bip322Error::ScriptTypeMismatch { declared, derived });
            }
            if !bip_config.allows(derived) {
                return Err(Bip322Error::UnsupportedScriptType);
            }
            verify_simple(derived, script_pubkey, &witness, message, network)?;
            Ok(derived)
        }
        v => Err(Bip322Error::UnsupportedProofVersion(v)),
    }
}

fn parse_script_pubkey_from_txout(txout: &corepc_types::v26::GetTxOut) -> Result<ScriptBuf, String> {
    // corepc_types v17/v26 GetTxOut has script_pubkey.hex field
    let hex_str = &txout.script_pubkey.hex;
    let bytes = hex::decode(hex_str).map_err(|e| format!("hex decode: {e}"))?;
    Ok(ScriptBuf::from_bytes(bytes))
}

/// Decode the v=2 PSBT-input envelope and extract the final witness.
///
/// Wire shape (Phase 16 RESEARCH Pitfall 7 Option 1): the base64 carries a
/// full BIP-174 PSBT containing one input and zero outputs. We extract
/// `psbt.inputs[0].final_script_witness`. The PSBT's
/// `witness_utxo.script_pubkey` is **IGNORED** — the on-chain SPK from
/// `gettxout` is the only trusted source. The PSBT here is a transport for
/// the witness bytes, not an authority for the script type.
fn decode_psbt_input_witness(b64: &str) -> Result<Witness, Bip322Error> {
    let bytes = B64
        .decode(b64)
        .map_err(|e| Bip322Error::DecodeError(format!("base64: {e}")))?;
    let psbt = bitcoin::psbt::Psbt::deserialize(&bytes)
        .map_err(|e| Bip322Error::DecodeError(format!("psbt: {e}")))?;
    let input = psbt.inputs.first().ok_or_else(|| {
        Bip322Error::WireFormatMismatch("v2 PSBT envelope contains zero inputs".into())
    })?;
    let witness = input.final_script_witness.clone().ok_or_else(|| {
        Bip322Error::WireFormatMismatch("v2 PSBT input lacks final_script_witness".into())
    })?;
    Ok(witness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;
    use bitcoin::key::TapTweak;
    use bitcoin::secp256k1::{Keypair, Message as SecpMessage, Secp256k1, SecretKey as SecpSecretKey, XOnlyPublicKey};
    use bitcoin::sighash::{EcdsaSighashType, SighashCache};
    use bitcoin::{Amount, PublicKey};
    use shared::bip322::{
        bip322_message_hash, build_bip322_to_sign, build_bip322_to_spend,
    };

    fn fixture_secret_key() -> SecpSecretKey {
        SecpSecretKey::from_slice(&[0x42_u8; 32]).unwrap()
    }

    // Recipes mirror shared::bip322::tests::fixture_p2wpkh_spk / fixture_p2tr_spk
    // (shared/src/bip322/mod.rs:445-474). Reconstructed verbatim here because the
    // Phase 15 helpers are `#[cfg(test)] mod tests` private and not reachable from
    // an external crate's test code. Keeping the per-script SPK construction
    // co-located with the dispatcher tests makes B4 self-contained.
    fn fixture_p2wpkh_spk() -> ScriptBuf {
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let compressed = PublicKey::new(pk);
        ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().unwrap())
    }

    fn fixture_p2tr_spk() -> ScriptBuf {
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let keypair = Keypair::from_secret_key(&secp, &sk);
        let (_xonly, _parity) = XOnlyPublicKey::from_keypair(&keypair);
        let tweaked = keypair.tap_tweak(&secp, None);
        let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
        ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked())
    }

    /// Build a valid P2WPKH witness stack for the given (spk, message) pair.
    /// Mirrors the inner sign body in shared::bip322::p2wpkh::sign; reproduced
    /// inline because that fn is `pub(crate)` to the shared crate.
    fn build_p2wpkh_witness_stack(spk: &Script, message: &[u8]) -> Vec<Vec<u8>> {
        let secp = Secp256k1::new();
        let sk = fixture_secret_key();
        let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let msg_hash = bip322_message_hash(message);
        let to_spend = build_bip322_to_spend(spk, &msg_hash);
        let to_sign = build_bip322_to_sign(&to_spend);
        let mut cache = SighashCache::new(&to_sign);
        let sighash = cache
            .p2wpkh_signature_hash(0, spk, Amount::ZERO, EcdsaSighashType::All)
            .unwrap();
        let secp_msg = SecpMessage::from_digest(*sighash.as_byte_array());
        let sig = secp.sign_ecdsa(&secp_msg, &sk);
        let mut sig_bytes = sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL
        vec![sig_bytes, pk.serialize().to_vec()]
    }

    /// Default-allowlist BipConfig (all 3 script types allowed). Production
    /// default per Phase 16 Plan 16-01 D-38.
    fn default_bip_config() -> BipConfig {
        BipConfig::default()
    }

    #[test]
    fn dispatcher_v1_legacy_p2wpkh_routes_to_verify_simple() {
        // Cross-phase invariant test at unit-tier: v=1 path's verify_simple(P2wpkh, ...)
        // is bit-exact with the deleted verify_bip322_simple per Phase 15-02 SUMMARY.
        let spk = fixture_p2wpkh_spk();
        let round_id = "test-round-id";
        let utxo_id = "abcdef:0";
        let message = format!("blindjoin:round:{}:utxo:{}", round_id, utxo_id);
        let witness_stack = build_p2wpkh_witness_stack(&spk, message.as_bytes());

        let proof = OwnershipProof {
            version: 1,
            witness_stack,
            psbt_input_b64: None,
            script_type: None,
        };
        let cfg = default_bip_config();

        let result = validate_ownership_proof_typed(
            &spk,
            &proof,
            Network::Regtest,
            &cfg,
            message.as_bytes(),
        );
        assert!(result.is_ok(), "v=1 legacy P2WPKH must verify: {result:?}");
        assert_eq!(result.unwrap(), ScriptType::P2wpkh);
    }

    #[test]
    fn dispatcher_unknown_version_3_rejects_unsupported_proof_version() {
        let spk = fixture_p2wpkh_spk();
        let proof = OwnershipProof {
            version: 3,
            witness_stack: vec![],
            psbt_input_b64: None,
            script_type: None,
        };
        let cfg = default_bip_config();
        let err = validate_ownership_proof_typed(
            &spk,
            &proof,
            Network::Regtest,
            &cfg,
            b"msg",
        )
        .expect_err("v=3 must reject");
        assert!(
            matches!(err, Bip322Error::UnsupportedProofVersion(3)),
            "expected UnsupportedProofVersion(3), got: {err:?}"
        );
    }

    #[test]
    fn dispatcher_v2_proof_without_script_type_rejects_wireformat_mismatch() {
        let spk = fixture_p2wpkh_spk();
        let proof = OwnershipProof {
            version: 2,
            witness_stack: vec![],
            psbt_input_b64: Some("AA==".to_string()),
            script_type: None,
        };
        let cfg = default_bip_config();
        let err = validate_ownership_proof_typed(
            &spk,
            &proof,
            Network::Regtest,
            &cfg,
            b"msg",
        )
        .expect_err("v=2 without script_type must reject");
        match err {
            Bip322Error::WireFormatMismatch(ref msg) => {
                assert!(
                    msg.contains("script_type"),
                    "WireFormatMismatch should mention 'script_type': {msg}"
                );
            }
            other => panic!("expected WireFormatMismatch, got: {other:?}"),
        }
    }

    // ---------------------------------------------------------------------
    // Spoofing-rejection at fast-CI tier (Plan 16-02 B4 closure).
    // ---------------------------------------------------------------------
    //
    // Constructs a v=2 OwnershipProof whose `script_type` field declares
    // ONE script type, against an on-chain SPK derived from ANOTHER script
    // type. The dispatcher's declared-vs-derived cross-check MUST fire
    // BEFORE verify_simple inspects the witness — so the witness can be
    // arbitrary bytes (we use the smallest valid PSBT envelope) and the
    // test still exercises the correct rejection path.
    //
    // A future refactor that drops `if declared != derived` from the v=2
    // arm fails these two tests at fast-CI tier (no bitcoind required).
    // ---------------------------------------------------------------------

    /// Build a minimal v=2 PSBT envelope b64 carrying a single-input,
    /// zero-output PSBT with a `final_script_witness` of arbitrary bytes.
    /// The witness bytes are NOT inspected — the declared-vs-derived
    /// cross-check fires strictly before verify_simple.
    fn minimal_v2_psbt_b64_with_arbitrary_witness() -> String {
        use bitcoin::psbt::Psbt;
        use bitcoin::{absolute, transaction, OutPoint, Sequence, Transaction, TxIn};

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
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).expect("unsigned tx -> psbt");
        // Push an arbitrary-bytes witness; the dispatcher's declared-vs-derived cross-check
        // fires BEFORE the witness is consumed, so contents are irrelevant.
        let mut w = Witness::new();
        w.push([0u8; 64]);
        psbt.inputs[0].final_script_witness = Some(w);
        B64.encode(psbt.serialize())
    }

    #[test]
    fn dispatcher_v2_p2wpkh_chain_p2tr_declared_rejects_spoofing() {
        let spk_on_chain = fixture_p2wpkh_spk();
        let proof = OwnershipProof {
            version: 2,
            witness_stack: vec![],
            psbt_input_b64: Some(minimal_v2_psbt_b64_with_arbitrary_witness()),
            script_type: Some(ScriptType::P2tr), // CLIENT-DECLARED P2TR
        };
        let cfg = default_bip_config();
        let err = validate_ownership_proof_typed(
            &spk_on_chain,
            &proof,
            Network::Regtest,
            &cfg,
            b"msg",
        )
        .expect_err("declared p2tr against on-chain p2wpkh must reject");
        assert!(
            matches!(
                err,
                Bip322Error::ScriptTypeMismatch {
                    declared: ScriptType::P2tr,
                    derived: ScriptType::P2wpkh,
                }
            ),
            "expected ScriptTypeMismatch {{ declared: P2tr, derived: P2wpkh }}, got: {err:?}"
        );
    }

    #[test]
    fn dispatcher_v2_p2tr_chain_p2wpkh_declared_rejects_spoofing() {
        let spk_on_chain = fixture_p2tr_spk();
        let proof = OwnershipProof {
            version: 2,
            witness_stack: vec![],
            psbt_input_b64: Some(minimal_v2_psbt_b64_with_arbitrary_witness()),
            script_type: Some(ScriptType::P2wpkh), // CLIENT-DECLARED P2WPKH
        };
        let cfg = default_bip_config();
        let err = validate_ownership_proof_typed(
            &spk_on_chain,
            &proof,
            Network::Regtest,
            &cfg,
            b"msg",
        )
        .expect_err("declared p2wpkh against on-chain p2tr must reject");
        assert!(
            matches!(
                err,
                Bip322Error::ScriptTypeMismatch {
                    declared: ScriptType::P2wpkh,
                    derived: ScriptType::P2tr,
                }
            ),
            "expected ScriptTypeMismatch {{ declared: P2wpkh, derived: P2tr }}, got: {err:?}"
        );
    }
}
