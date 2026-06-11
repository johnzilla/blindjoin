//! Per-input partial-signature verification (H3).
//!
//! At signing time each participant submits a consensus-serialized witness for
//! their input. Before this module existed the coordinator stored those bytes
//! unchecked and only discovered an invalid signature in aggregate at
//! `testmempoolaccept` — at which point every participant had "signed", so the
//! blame path attributed the failure to nobody and the round aborted with no
//! ban. A single participant could therefore destroy every round at zero cost
//! and escape blame entirely.
//!
//! [`verify_input_signature`] reconstructs the BIP-143 / BIP-341 sighash for the
//! submitting input against the CANONICAL CoinJoin transaction (the same PSBT the
//! coordinator will broadcast) and verifies the witness against it. An invalid
//! submission is rejected at the door, so the sender stays unsigned and becomes a
//! bannable non-signer at the signing deadline.
//!
//! PII discipline: every error carries only the failure *kind* — never key
//! material, outpoints, amounts, or signatures.

use bitcoin::hashes::Hash;
use bitcoin::psbt::Psbt;
use bitcoin::secp256k1::{ecdsa, schnorr, Message, Secp256k1, XOnlyPublicKey};
use bitcoin::sighash::{EcdsaSighashType, Prevouts, SighashCache, TapSighashType};
use bitcoin::{Amount, CompressedPublicKey, Script, ScriptBuf, TxOut, Witness};
use shared::bip322::ScriptType;

/// A partial signature failed verification. The message names the failure kind
/// only (PII-safe).
#[derive(Debug, Clone)]
pub struct SigVerifyError(pub String);

impl std::fmt::Display for SigVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn err(kind: &str) -> SigVerifyError {
    SigVerifyError(kind.to_string())
}

/// Verify the witness submitted for `input_index` of the canonical CoinJoin tx
/// held in `psbt`. `psbt` MUST be the same transaction that will be broadcast and
/// MUST carry `witness_utxo` for every input (true for `build_coinjoin_psbt`
/// output) — taproot key-spend sighashes commit to all prevouts.
pub fn verify_input_signature(
    psbt: &Psbt,
    input_index: usize,
    script_type: ScriptType,
    witness_bytes: &[u8],
) -> Result<(), SigVerifyError> {
    let witness: Witness = bitcoin::consensus::deserialize(witness_bytes)
        .map_err(|_| err("witness is not consensus-deserializable"))?;

    let witness_utxo = psbt
        .inputs
        .get(input_index)
        .and_then(|i| i.witness_utxo.as_ref())
        .ok_or_else(|| err("input has no witness_utxo for sighash"))?;
    let value = witness_utxo.value;
    let spk = &witness_utxo.script_pubkey;

    match script_type {
        ScriptType::P2wpkh => verify_p2wpkh(psbt, input_index, &witness, spk, value),
        ScriptType::P2shP2wpkh => verify_p2sh_p2wpkh(psbt, input_index, &witness, spk, value),
        ScriptType::P2tr => verify_p2tr(psbt, input_index, &witness),
    }
}

/// Split a DER+hashtype witness signature element, enforcing SIGHASH_ALL. A
/// non-ALL flag (SINGLE / NONE / ANYONECANPAY) on a CoinJoin input could let a
/// participant disclaim commitment to other outputs, so it is refused.
fn parse_ecdsa_sig(sig_elem: &[u8]) -> Result<ecdsa::Signature, SigVerifyError> {
    let (flag, der) = sig_elem.split_last().ok_or_else(|| err("empty signature element"))?;
    if *flag != EcdsaSighashType::All as u8 {
        return Err(err("signature is not SIGHASH_ALL"));
    }
    ecdsa::Signature::from_der(der).map_err(|_| err("malformed DER signature"))
}

fn verify_p2wpkh(
    psbt: &Psbt,
    input_index: usize,
    witness: &Witness,
    spk: &Script,
    value: Amount,
) -> Result<(), SigVerifyError> {
    if witness.len() != 2 {
        return Err(err("p2wpkh witness must have exactly 2 elements"));
    }
    let sig = parse_ecdsa_sig(witness.nth(0).unwrap())?;
    let pubkey_bytes = witness.nth(1).unwrap();
    let pubkey = CompressedPublicKey::from_slice(pubkey_bytes)
        .map_err(|_| err("witness pubkey is not a valid compressed key"))?;

    // The pubkey must hash to the input's P2WPKH program, else a participant
    // could present an unrelated key whose signature verifies over the sighash
    // but does not control the UTXO.
    let derived = ScriptBuf::new_p2wpkh(&pubkey.wpubkey_hash());
    if derived.as_script() != spk {
        return Err(err("witness pubkey does not match input script"));
    }

    let sighash = SighashCache::new(&psbt.unsigned_tx)
        .p2wpkh_signature_hash(input_index, spk, value, EcdsaSighashType::All)
        .map_err(|_| err("p2wpkh sighash computation failed"))?;
    verify_ecdsa(&sighash.to_byte_array(), &sig, &pubkey.0)
}

fn verify_p2sh_p2wpkh(
    psbt: &Psbt,
    input_index: usize,
    witness: &Witness,
    spk: &Script,
    value: Amount,
) -> Result<(), SigVerifyError> {
    if witness.len() != 2 {
        return Err(err("p2sh-p2wpkh witness must have exactly 2 elements"));
    }
    let sig = parse_ecdsa_sig(witness.nth(0).unwrap())?;
    let pubkey_bytes = witness.nth(1).unwrap();
    let pubkey = CompressedPublicKey::from_slice(pubkey_bytes)
        .map_err(|_| err("witness pubkey is not a valid compressed key"))?;

    // The redeem script is the unwrapped P2WPKH program; the input's P2SH script
    // must commit to it. The redeem script is ALSO the BIP-143 sighash spk.
    let redeem = ScriptBuf::new_p2wpkh(&pubkey.wpubkey_hash());
    let derived_p2sh = ScriptBuf::new_p2sh(&redeem.script_hash());
    if derived_p2sh.as_script() != spk {
        return Err(err("witness pubkey does not match input p2sh script"));
    }

    let sighash = SighashCache::new(&psbt.unsigned_tx)
        .p2wpkh_signature_hash(input_index, redeem.as_script(), value, EcdsaSighashType::All)
        .map_err(|_| err("p2sh-p2wpkh sighash computation failed"))?;
    verify_ecdsa(&sighash.to_byte_array(), &sig, &pubkey.0)
}

fn verify_p2tr(psbt: &Psbt, input_index: usize, witness: &Witness) -> Result<(), SigVerifyError> {
    if witness.len() != 1 {
        return Err(err("p2tr key-path witness must have exactly 1 element"));
    }
    let sig_elem = witness.nth(0).unwrap();
    // BIP-341: a 64-byte signature implies SIGHASH_DEFAULT; a 65-byte signature
    // carries an explicit sighash type byte. Only DEFAULT/ALL are accepted.
    let (sig_bytes, sighash_type) = match sig_elem.len() {
        64 => (sig_elem, TapSighashType::Default),
        65 => {
            if sig_elem[64] != TapSighashType::All as u8 {
                return Err(err("p2tr explicit sighash type is not ALL"));
            }
            (&sig_elem[..64], TapSighashType::All)
        }
        _ => return Err(err("p2tr signature must be 64 or 65 bytes")),
    };
    let sig = schnorr::Signature::from_slice(sig_bytes)
        .map_err(|_| err("malformed schnorr signature"))?;

    // Taproot key-spend sighash commits to every prevout.
    let prevouts: Vec<TxOut> = psbt
        .inputs
        .iter()
        .map(|i| i.witness_utxo.clone())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| err("a round input is missing its witness_utxo"))?;

    let spk = &prevouts[input_index].script_pubkey;
    if !spk.is_p2tr() {
        return Err(err("input script is not p2tr"));
    }
    // P2TR spk = OP_1 <32-byte x-only key>; the program starts at byte 2.
    let xonly = XOnlyPublicKey::from_slice(&spk.as_bytes()[2..34])
        .map_err(|_| err("input p2tr program is not a valid x-only key"))?;

    let sighash = SighashCache::new(&psbt.unsigned_tx)
        .taproot_key_spend_signature_hash(input_index, &Prevouts::All(&prevouts), sighash_type)
        .map_err(|_| err("p2tr sighash computation failed"))?;
    let msg = Message::from_digest(sighash.to_byte_array());
    Secp256k1::verification_only()
        .verify_schnorr(&sig, &msg, &xonly)
        .map_err(|_| err("schnorr signature verification failed"))
}

fn verify_ecdsa(
    sighash: &[u8; 32],
    sig: &ecdsa::Signature,
    pubkey: &bitcoin::secp256k1::PublicKey,
) -> Result<(), SigVerifyError> {
    let msg = Message::from_digest(*sighash);
    Secp256k1::verification_only()
        .verify_ecdsa(&msg, sig, pubkey)
        .map_err(|_| err("ecdsa signature verification failed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::absolute::LockTime;
    use bitcoin::hashes::Hash;
    use bitcoin::key::{Keypair, TapTweak};
    use bitcoin::secp256k1::{PublicKey, SecretKey};
    use bitcoin::transaction::Version;
    use bitcoin::{OutPoint, Transaction, TxIn, Txid};

    fn secp() -> Secp256k1<bitcoin::secp256k1::All> {
        Secp256k1::new()
    }

    fn sk(fill: u8) -> SecretKey {
        SecretKey::from_slice(&[fill; 32]).expect("valid secret key")
    }

    fn p2wpkh_spk(s: &Secp256k1<bitcoin::secp256k1::All>, k: &SecretKey) -> (ScriptBuf, CompressedPublicKey) {
        let cpk = CompressedPublicKey(PublicKey::from_secret_key(s, k));
        (ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash()), cpk)
    }

    /// Build a canonical-style unsigned PSBT: one input per (spk, value), each
    /// with its witness_utxo populated, plus a single dummy output.
    fn build_psbt(inputs: &[(ScriptBuf, u64)]) -> Psbt {
        let input: Vec<TxIn> = (0..inputs.len())
            .map(|i| TxIn {
                previous_output: OutPoint::new(Txid::from_byte_array([(i as u8) + 1; 32]), 0),
                ..Default::default()
            })
            .collect();
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input,
            output: vec![TxOut { value: Amount::from_sat(40_000), script_pubkey: ScriptBuf::new() }],
        };
        let mut psbt = Psbt::from_unsigned_tx(tx).expect("valid psbt");
        for (i, (spk, value)) in inputs.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(TxOut {
                value: Amount::from_sat(*value),
                script_pubkey: spk.clone(),
            });
        }
        psbt
    }

    #[allow(clippy::too_many_arguments)] // test helper; explicit params keep call sites readable
    fn ecdsa_witness(
        s: &Secp256k1<bitcoin::secp256k1::All>,
        psbt: &Psbt,
        idx: usize,
        sighash_spk: &Script,
        value: u64,
        k: &SecretKey,
        cpk: &CompressedPublicKey,
        sighash_flag: u8,
    ) -> Witness {
        let sighash = SighashCache::new(&psbt.unsigned_tx)
            .p2wpkh_signature_hash(idx, sighash_spk, Amount::from_sat(value), EcdsaSighashType::All)
            .unwrap();
        let sig = s.sign_ecdsa(&Message::from_digest(sighash.to_byte_array()), k);
        let mut sig_ser = sig.serialize_der().to_vec();
        sig_ser.push(sighash_flag);
        let mut w = Witness::new();
        w.push(sig_ser);
        w.push(cpk.to_bytes());
        w
    }

    fn ser(w: &Witness) -> Vec<u8> {
        bitcoin::consensus::serialize(w)
    }

    #[test]
    fn p2wpkh_valid_sig_verifies() {
        let s = secp();
        let k = sk(0x11);
        let (spk, cpk) = p2wpkh_spk(&s, &k);
        let psbt = build_psbt(&[(spk.clone(), 100_000), (p2wpkh_spk(&s, &sk(0x12)).0, 90_000)]);
        let w = ecdsa_witness(&s, &psbt, 0, &spk, 100_000, &k, &cpk, EcdsaSighashType::All as u8);
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2wpkh, &ser(&w)).is_ok());
    }

    #[test]
    fn p2wpkh_tampered_sig_rejected() {
        let s = secp();
        let k = sk(0x11);
        let (spk, cpk) = p2wpkh_spk(&s, &k);
        let psbt = build_psbt(&[(spk.clone(), 100_000)]);
        let mut w = ecdsa_witness(&s, &psbt, 0, &spk, 100_000, &k, &cpk, EcdsaSighashType::All as u8);
        // Corrupt the first witness element (the signature).
        let mut bad: Vec<u8> = w.nth(0).unwrap().to_vec();
        bad[6] ^= 0xff;
        w = {
            let mut nw = Witness::new();
            nw.push(bad);
            nw.push(cpk.to_bytes());
            nw
        };
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2wpkh, &ser(&w)).is_err());
    }

    #[test]
    fn p2wpkh_non_sighash_all_rejected() {
        let s = secp();
        let k = sk(0x11);
        let (spk, cpk) = p2wpkh_spk(&s, &k);
        let psbt = build_psbt(&[(spk.clone(), 100_000)]);
        // Valid signature bytes but flagged SIGHASH_NONE (0x02).
        let w = ecdsa_witness(&s, &psbt, 0, &spk, 100_000, &k, &cpk, 0x02);
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2wpkh, &ser(&w)).is_err());
    }

    #[test]
    fn p2wpkh_pubkey_not_matching_script_rejected() {
        let s = secp();
        let k = sk(0x11);
        let (spk, _cpk) = p2wpkh_spk(&s, &k);
        // Sign with a DIFFERENT key and present its pubkey — sig may verify over
        // the sighash for that key, but the key does not control this UTXO.
        let other = sk(0x99);
        let other_cpk = CompressedPublicKey(PublicKey::from_secret_key(&s, &other));
        let psbt = build_psbt(&[(spk.clone(), 100_000)]);
        let w = ecdsa_witness(&s, &psbt, 0, &spk, 100_000, &other, &other_cpk, EcdsaSighashType::All as u8);
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2wpkh, &ser(&w)).is_err());
    }

    #[test]
    fn p2sh_p2wpkh_valid_sig_verifies_and_tamper_rejected() {
        let s = secp();
        let k = sk(0x21);
        let cpk = CompressedPublicKey(PublicKey::from_secret_key(&s, &k));
        let redeem = ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash());
        let spk = ScriptBuf::new_p2sh(&redeem.script_hash());
        let psbt = build_psbt(&[(spk.clone(), 120_000)]);
        // P2SH-P2WPKH sighash uses the UNWRAPPED p2wpkh (redeem) script.
        let w = ecdsa_witness(&s, &psbt, 0, redeem.as_script(), 120_000, &k, &cpk, EcdsaSighashType::All as u8);
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2shP2wpkh, &ser(&w)).is_ok());

        let mut bad: Vec<u8> = w.nth(0).unwrap().to_vec();
        bad[7] ^= 0xff;
        let mut w2 = Witness::new();
        w2.push(bad);
        w2.push(cpk.to_bytes());
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2shP2wpkh, &ser(&w2)).is_err());
    }

    #[test]
    fn p2tr_valid_keyspend_verifies_and_tamper_rejected() {
        let s = secp();
        let k = sk(0x31);
        let keypair = Keypair::from_secret_key(&s, &k);
        let tweaked = keypair.tap_tweak(&s, None);
        let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
        let spk = ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked());
        // Second (p2wpkh) input so Prevouts::All carries more than one prevout.
        let psbt = build_psbt(&[(spk.clone(), 100_000), (p2wpkh_spk(&s, &sk(0x32)).0, 70_000)]);
        let prevouts: Vec<TxOut> = psbt.inputs.iter().map(|i| i.witness_utxo.clone().unwrap()).collect();
        let sighash = SighashCache::new(&psbt.unsigned_tx)
            .taproot_key_spend_signature_hash(0, &Prevouts::All(&prevouts), TapSighashType::Default)
            .unwrap();
        let sig = s.sign_schnorr_no_aux_rand(&Message::from_digest(sighash.to_byte_array()), &tweaked.to_keypair());

        let mut w = Witness::new();
        w.push(sig.serialize()); // 64 bytes → SIGHASH_DEFAULT
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2tr, &ser(&w)).is_ok());

        let mut bad = sig.serialize().to_vec();
        bad[12] ^= 0xff;
        let mut w2 = Witness::new();
        w2.push(bad);
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2tr, &ser(&w2)).is_err());
    }

    #[test]
    fn wrong_script_type_dispatch_rejected() {
        // A valid P2WPKH witness verified as P2TR must fail (witness arity 2 != 1).
        let s = secp();
        let k = sk(0x41);
        let (spk, cpk) = p2wpkh_spk(&s, &k);
        let psbt = build_psbt(&[(spk.clone(), 100_000)]);
        let w = ecdsa_witness(&s, &psbt, 0, &spk, 100_000, &k, &cpk, EcdsaSighashType::All as u8);
        assert!(verify_input_signature(&psbt, 0, ScriptType::P2tr, &ser(&w)).is_err());
    }
}
