//! P2TR BIP-322 Simple verify + sign per Phase 15 CONTEXT D-04.
//!
//! Both `verify` and `sign` are `pub(crate)` per D-27. The production `sign`
//! body is `todo!()` per CD-6 — Phase 17 WALLET-02 wires the bdk_wallet sign
//! path per ADR Decision #4. The `#[cfg(test)] sign_for_tests` helper exists
//! so the per-script property tests in Plan 15-03 can construct valid 64-byte
//! Schnorr keypath witnesses without depending on `bdk_wallet` from `shared/`.

use bitcoin::secp256k1::SecretKey;
use bitcoin::{Network, Script, Witness};

/// Verify a P2TR BIP-322 Simple proof.
///
/// Arity pre-flight: P2TR Simple witnesses carry a single Schnorr signature
/// (1 item, either 64 bytes SIGHASH_DEFAULT or 65 bytes SIGHASH_ALL). The
/// `bip322 = "=0.0.10"` crate's `verify_full_p2tr` (`verify.rs:187-258`)
/// handles both byte lengths internally.
pub(crate) fn verify(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 1 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 1,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}

/// Production sign for P2TR — `todo!()` per CD-6.
///
/// Phase 17 WALLET-02 swaps this body for the bdk_wallet 2.3 sign path per
/// ADR Decision #4 (Sprint-0-B PASS). The signature contract is locked at
/// this plan boundary so Phase 16 and Phase 17 can wire against a stable API.
pub(crate) fn sign(
    _spk: &Script,
    _key: &SecretKey,
    _message: &[u8],
) -> Result<Witness, super::Bip322Error> {
    todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")
}

/// Test-only signer producing a valid 64-byte Schnorr keyspend witness.
///
/// Uses the 8-step BIP-341 sequence from Phase 14 Sprint-0-B (and RESEARCH
/// Pattern 4): build to_spend → build to_sign → Keypair::from_secret_key →
/// tap_tweak → taproot_key_spend_signature_hash → sign_schnorr_no_aux_rand →
/// push 64 bytes into Witness. This is the load-bearing test signer that
/// Plan 15-03 consumes for per-script positive-vector tests.
#[cfg(test)]
pub(crate) fn sign_for_tests(spk: &Script, key: &SecretKey, message: &[u8]) -> Witness {
    use bitcoin::hashes::Hash;
    use bitcoin::key::{Keypair, TapTweak};
    use bitcoin::secp256k1::{Message, Secp256k1};
    use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
    use bitcoin::{Amount, TxOut};

    let msg_hash = super::bip322_message_hash(message);
    let to_spend = super::build_bip322_to_spend(spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, key);
    let tweaked = keypair.tap_tweak(&secp, None).to_keypair();

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .taproot_key_spend_signature_hash(
            0,
            &Prevouts::All(&[TxOut {
                value: Amount::ZERO,
                script_pubkey: spk.to_owned(),
            }]),
            TapSighashType::Default,
        )
        .expect("sighash on well-formed to_sign");

    let sig = secp.sign_schnorr_no_aux_rand(
        &Message::from_digest(sighash.to_byte_array()),
        &tweaked,
    );

    let mut w = Witness::new();
    w.push(sig.as_ref().to_vec());
    w
}
