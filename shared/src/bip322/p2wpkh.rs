//! P2WPKH BIP-322 Simple verify + sign per Phase 15 CONTEXT D-04.
//!
//! Both `verify` and `sign` are `pub(crate)` per D-27 — callers reach them
//! only via the dispatchers in [`super`]. This keeps the V1.4-CRIT-01 spoofing
//! vector statically unreachable.
//!
//! `sign` ships a FULL production body in Phase 15 per CD-6 (carried over
//! from the v1.3 path at the prior `shared/src/bip322.rs::tests::make_bip322_witness`,
//! generalised to take a [`SecretKey`] parameter and return
//! `Result<Witness, Bip322Error>`).

use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::{Amount, Network, Script, Witness};

/// Verify a P2WPKH BIP-322 Simple proof.
///
/// Arity pre-flight: P2WPKH witnesses are `[sig, pubkey]` (2 items). The
/// `bip322 = "=0.0.11"` crate's `verify_full_p2wpkh` handles the BIP-143 sighash
/// and signature check, and (since 0.0.11) also verifies the witness pubkey hashes
/// to the address's HASH160. blindjoin still enforces that key-to-address binding
/// independently via the guard in [`super::verify_via_bip322_crate`] (rejecting an
/// unrelated key with [`super::Bip322Error::WitnessKeyMismatch`]) as defense-in-depth
/// — it was the soundness gap in 0.0.6–0.0.10. Without that guard a signature by
/// any key would verify against any P2WPKH address.
pub(crate) fn verify(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 2 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 2,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}

/// Sign a P2WPKH BIP-322 Simple proof — full production body per CD-6.
///
/// Lifted from the prior `shared/src/bip322.rs::tests::make_bip322_witness`
/// (lines 86-108 in the pre-15-02 file) and generalised:
/// - takes the secret key as a parameter (was a hardcoded `[0x01; 32]` test fixture);
/// - returns `Result<Witness, Bip322Error>` (was infallible `(ScriptBuf, Vec<Vec<u8>>)`);
/// - maps the `p2wpkh_signature_hash` failure to [`super::Bip322Error::DecodeError`]
///   (PII-safe: only the bitcoin sighash error's Display is interpolated; no
///   key, address, or outpoint appears in the error message).
pub(crate) fn sign(
    spk: &Script,
    key: &SecretKey,
    message: &[u8],
) -> Result<Witness, super::Bip322Error> {
    let secp = Secp256k1::new();
    let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, key);

    let msg_hash = super::bip322_message_hash(message);
    let to_spend = super::build_bip322_to_spend(spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .p2wpkh_signature_hash(0, spk, Amount::ZERO, EcdsaSighashType::All)
        .map_err(|e| super::Bip322Error::DecodeError(format!("p2wpkh sighash: {e}")))?;

    let secp_msg = Message::from_digest(sighash.to_byte_array());
    let sig = super::sign_ecdsa_compat_bip322_length(&secp, &secp_msg, key);
    let mut sig_bytes = sig.serialize_der().to_vec();
    sig_bytes.push(0x01); // SIGHASH_ALL

    let mut w = Witness::new();
    w.push(sig_bytes);
    w.push(pubkey.serialize());
    Ok(w)
}

