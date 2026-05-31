//! P2SH-P2WPKH BIP-322 Simple verify + sign per Phase 15 CONTEXT D-04.
//!
//! Both `verify` and `sign` are `pub(crate)` per D-27. The production `sign`
//! body is `todo!()` per CD-6 — Phase 17 WALLET-02 wires the bdk_wallet sign
//! path per ADR Decision #4. The `#[cfg(test)] sign_for_tests` helper builds
//! a valid `[sig, pubkey]` 2-item witness mirroring the P2WPKH shape (per
//! CONTEXT `<specifics>`: "P2SH-P2WPKH uses the same shape as P2WPKH but
//! with final_script_sig = OP_HASH160 <redeem-script-hash> and witness =
//! [sig, pubkey]"), so Plan 15-03's per-script property tests can construct
//! positive vectors without depending on `bdk_wallet` from `shared/`.

use bitcoin::secp256k1::SecretKey;
use bitcoin::{Network, Script, Witness};

/// Verify a P2SH-P2WPKH BIP-322 Simple proof.
///
/// Arity pre-flight: P2SH-P2WPKH witnesses are `[sig, pubkey]` (2 items),
/// identical shape to P2WPKH because the unwrapped redeem IS a P2WPKH SPK.
/// The `bip322 = "=0.0.10"` crate's `verify_simple` (`verify.rs:62-99` ->
/// `verify_full_p2wpkh(is_p2sh=true)` at `verify.rs:167-169`) reconstructs
/// the unwrapped P2WPKH from `witness[1].wpubkey_hash()` and HASH160-cross-
/// checks against the P2SH SPK, so non-P2WPKH-wrapped P2SH scripts reject
/// at verify time.
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

/// Production sign for P2SH-P2WPKH — `todo!()` per CD-6.
///
/// Phase 17 WALLET-02 swaps this body for the bdk_wallet 2.3 sign path per
/// ADR Decision #4. The signature contract is locked at this plan boundary
/// so Phase 16 and Phase 17 can wire against a stable API.
pub(crate) fn sign(
    _spk: &Script,
    _key: &SecretKey,
    _message: &[u8],
) -> Result<Witness, super::Bip322Error> {
    todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")
}

/// Test-only signer producing the `[sig, pubkey]` witness for a P2SH-P2WPKH
/// SPK. The sighash uses the UNWRAPPED P2WPKH script derived from the pubkey
/// (per BIP-143 over the redeem), NOT the outer P2SH SPK — this matches the
/// bip322 crate's internal `verify_full_p2wpkh(is_p2sh=true)` shape at
/// `verify.rs:167-169`.
///
/// `_spk` is the OUTER P2SH SPK (kept in the signature for symmetry with the
/// other per-script signers); we don't use it directly because the sighash
/// uses the inner P2WPKH SPK derived from the pubkey.
///
/// Plan 15-03 promotes this from `#[cfg(test)] pub(crate)` to `pub(crate)`
/// (no cfg gate) so the integration-test dispatcher mirror
/// `super::sign_simple_test_only` (a `#[doc(hidden)] pub fn` in `mod.rs`)
/// can reach it from external test crates at `shared/tests/*.rs`. Production
/// callers cannot invoke this fn directly because the per-script module is
/// `pub(crate)`-only per D-27.
pub(crate) fn sign_for_tests(_spk: &Script, key: &SecretKey, message: &[u8]) -> Witness {
    use bitcoin::hashes::Hash;
    use bitcoin::secp256k1::{Message, Secp256k1};
    use bitcoin::sighash::{EcdsaSighashType, SighashCache};
    use bitcoin::{Amount, PublicKey, ScriptBuf};

    let secp = Secp256k1::new();
    let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, key);
    let compressed = PublicKey::new(pubkey);
    // Derive the UNWRAPPED P2WPKH SPK from the pubkey (this is the sighash SPK).
    let unwrapped_p2wpkh =
        ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed key"));

    let msg_hash = super::bip322_message_hash(message);
    // The to_spend output's script_pubkey is the OUTER P2SH SPK; we derive it
    // here so callers can pass `_spk` for symmetry but we actually use the
    // unwrapped P2WPKH for sighash per BIP-143.
    let p2sh_spk = ScriptBuf::new_p2sh(&unwrapped_p2wpkh.script_hash());
    let to_spend = super::build_bip322_to_spend(&p2sh_spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .p2wpkh_signature_hash(
            0,
            &unwrapped_p2wpkh,
            Amount::ZERO,
            EcdsaSighashType::All,
        )
        .expect("sighash on well-formed to_sign");

    let secp_msg = Message::from_digest(sighash.to_byte_array());
    let sig = secp.sign_ecdsa(&secp_msg, key);
    let mut sig_bytes = sig.serialize_der().to_vec();
    sig_bytes.push(0x01); // SIGHASH_ALL

    let mut w = Witness::new();
    w.push(sig_bytes);
    w.push(pubkey.serialize());
    w
}
