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
/// `bip322 = "=0.0.10"` crate's `verify_full_p2wpkh` (`verify.rs:101-185`)
/// handles BIP-143 sighash + the HASH160 redeem cross-check internally.
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
    let sig = secp.sign_ecdsa(&secp_msg, key);
    let mut sig_bytes = sig.serialize_der().to_vec();
    sig_bytes.push(0x01); // SIGHASH_ALL

    let mut w = Witness::new();
    w.push(sig_bytes);
    w.push(pubkey.serialize().to_vec());
    Ok(w)
}

/// Test-only signer kept beside the production [`sign`] so the per-script
/// property-test harness (Plan 15-03) can construct positive witnesses without
/// depending on the workspace `bdk_wallet` crate. Body is identical to
/// production [`sign`] for P2WPKH (the production body is already fully
/// implemented per CD-6); the alias exists for symmetry with `p2tr` /
/// `p2sh_p2wpkh` where the production body is `todo!()` but the test signer
/// is load-bearing.
///
/// Plan 15-03 promotes this from `#[cfg(test)] pub(crate)` to `pub(crate)`
/// (no cfg gate) so the integration-test dispatcher mirror
/// `super::sign_simple_test_only` (a `#[doc(hidden)] pub fn` in `mod.rs`)
/// can reach it from external test crates at `shared/tests/*.rs`. Production
/// callers cannot invoke this fn directly because the per-script module is
/// `pub(crate)`-only per D-27.
#[allow(dead_code)]
pub(crate) fn sign_for_tests(
    spk: &Script,
    key: &SecretKey,
    message: &[u8],
) -> Witness {
    sign(spk, key, message).expect("p2wpkh sign cannot fail on well-formed inputs")
}
