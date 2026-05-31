//! Phase 17 17-02 D-77 — per-script BIP-322 sign↔verify roundtrip tests for
//! `BdkClientWallet::sign_bip322` (the WALLET-02 acceptance gate).
//!
//! For each ScriptType supported by the wallet construction surface
//! (P2WPKH descriptor, P2TR descriptor, P2SH-P2WPKH descriptor, P2WPKH WIF):
//!  - construct a wallet at a deterministic outpoint placeholder;
//!  - call `wallet.sign_bip322("test-message")`;
//!  - feed the resulting `Witness` back through `shared::bip322::verify_simple`
//!    and assert `Ok(())`.
//!
//! Per-script gates additionally enforce:
//!  - P2SH-P2WPKH: `signed.final_script_sig.is_some()` per RESEARCH Pitfall 7
//!    (bdk_wallet finalises sh(wpkh(...)) by populating BOTH
//!    `final_script_witness` AND `final_script_sig`).
//!  - All paths: `signed.witness_stack == signed.witness.iter().map(...).collect()`
//!    per D-70 symmetry.
//!  - All paths: `signed.script_type == wallet.script_type()` — the CRIT-01
//!    client-side seed; the v=2 envelope's script_type field reads from
//!    `signed.script_type` (never `cfg.script_type` direct echo).
//!
//! No bitcoind dependency: BIP-322 sign + verify are pure-crypto pure-rust;
//! we use bdk_wallet's keychain-only path and never touch chain state.

use bitcoin::Network;
use client::wallet::BdkClientWallet;
use shared::bip322::{sign_simple, verify_simple, ScriptType};
use std::str::FromStr;

// Placeholder outpoint accepted by all three wallet constructors. BIP-322
// signs the message against the SPK, NOT against the outpoint, so any
// well-formed "txid:vout" suffices here.
const DUMMY_OUTPOINT: &str =
    "0000000000000000000000000000000000000000000000000000000000000000:0";

const TEST_MESSAGE: &str = "test-message";

/// Deterministic test-only WIF — NOT for real funds. The canonical Bitcoin
/// Core regtest "Hello World" WIF, also used by `client::wallet::tests` and
/// other integration fixtures in this workspace.
const TEST_WIF: &str = "cVt4o7BGAig1UXywgGSmARhxMdzP5qvQsxKkSsc1XEkw3tDTQFpy";

/// Network used for the descriptor-wallet tests. Signet matches the rest of
/// the workspace's test surface (config defaults, the 17-01 wallet tests).
/// BIP-322 itself is network-agnostic — `verify_simple` only consults the
/// network for address parsing of the SPK.
const NET: Network = Network::Signet;

#[tokio::test]
async fn p2wpkh_descriptor_sign_roundtrip_verifies() {
    let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, NET, ScriptType::P2wpkh)
        .expect("P2WPKH descriptor generate should succeed");
    let spk = wallet.script_pubkey();
    let signed = wallet
        .sign_bip322(TEST_MESSAGE)
        .expect("sign_bip322 (P2WPKH descriptor) should succeed");
    verify_simple(
        ScriptType::P2wpkh,
        &spk,
        &signed.witness,
        TEST_MESSAGE.as_bytes(),
        NET,
    )
    .expect("P2WPKH descriptor verify_simple should accept the produced witness");

    // D-70: witness_stack derived from witness.iter()
    let expected_stack: Vec<Vec<u8>> = signed.witness.iter().map(|s| s.to_vec()).collect();
    assert_eq!(signed.witness_stack, expected_stack);
    // CRIT-01 client-side seed: signed.script_type traces to wallet.script_type().
    assert_eq!(signed.script_type, wallet.script_type());
    // P2WPKH descriptor path: final_script_sig is always None.
    assert!(signed.final_script_sig.is_none(), "P2WPKH must have no final_script_sig");
}

#[tokio::test]
async fn p2tr_descriptor_sign_roundtrip_verifies() {
    let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, NET, ScriptType::P2tr)
        .expect("P2TR descriptor generate should succeed");
    let spk = wallet.script_pubkey();
    let signed = wallet
        .sign_bip322(TEST_MESSAGE)
        .expect("sign_bip322 (P2TR descriptor) should succeed");
    verify_simple(
        ScriptType::P2tr,
        &spk,
        &signed.witness,
        TEST_MESSAGE.as_bytes(),
        NET,
    )
    .expect("P2TR descriptor verify_simple should accept the produced witness");

    let expected_stack: Vec<Vec<u8>> = signed.witness.iter().map(|s| s.to_vec()).collect();
    assert_eq!(signed.witness_stack, expected_stack);
    assert_eq!(signed.script_type, wallet.script_type());
    assert!(signed.final_script_sig.is_none(), "P2TR must have no final_script_sig");
}

#[tokio::test]
async fn p2sh_p2wpkh_descriptor_sign_roundtrip_verifies() {
    let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, NET, ScriptType::P2shP2wpkh)
        .expect("P2SH-P2WPKH descriptor generate should succeed");
    let spk = wallet.script_pubkey();
    let signed = wallet
        .sign_bip322(TEST_MESSAGE)
        .expect("sign_bip322 (P2SH-P2WPKH descriptor) should succeed");
    verify_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &signed.witness,
        TEST_MESSAGE.as_bytes(),
        NET,
    )
    .expect("P2SH-P2WPKH descriptor verify_simple should accept the produced witness");

    let expected_stack: Vec<Vec<u8>> = signed.witness.iter().map(|s| s.to_vec()).collect();
    assert_eq!(signed.witness_stack, expected_stack);
    assert_eq!(signed.script_type, wallet.script_type());
    // RESEARCH Pitfall 7: P2SH-P2WPKH MUST carry the final_script_sig.
    assert!(
        signed.final_script_sig.is_some(),
        "P2SH-P2WPKH must populate final_script_sig (Pitfall 7)"
    );
}

#[tokio::test]
async fn p2wpkh_wif_sign_roundtrip_verifies() {
    // Preserves the v1.3 cross-phase invariant gate: the WIF path routes
    // internally through `shared::bip322::sign_simple(P2wpkh, ...)` which
    // Phase 15 confirmed bit-exact with the prior hand-rolled
    // generate_bip322_witness (deleted in Plan 17-02 Task 2 per CD-20).
    let wallet = BdkClientWallet::from_wif(TEST_WIF, DUMMY_OUTPOINT, Network::Regtest)
        .expect("from_wif should succeed for the test WIF");
    let spk = wallet.script_pubkey();
    let signed = wallet
        .sign_bip322(TEST_MESSAGE)
        .expect("sign_bip322 (WIF) should succeed");
    verify_simple(
        ScriptType::P2wpkh,
        &spk,
        &signed.witness,
        TEST_MESSAGE.as_bytes(),
        Network::Regtest,
    )
    .expect("WIF path verify_simple should accept the produced witness");

    let expected_stack: Vec<Vec<u8>> = signed.witness.iter().map(|s| s.to_vec()).collect();
    assert_eq!(signed.witness_stack, expected_stack);
    assert_eq!(signed.script_type, ScriptType::P2wpkh);
    assert!(signed.final_script_sig.is_none(), "WIF P2WPKH must have no final_script_sig");
}

#[tokio::test]
async fn signed_proof_witness_stack_matches_witness_iter() {
    // D-70 symmetry: witness_stack is always the flat-Vec<Vec<u8>> form of
    // witness.iter(). One canonical assertion across the dispatcher.
    let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, NET, ScriptType::P2tr)
        .expect("P2TR generate should succeed");
    let signed = wallet.sign_bip322(TEST_MESSAGE).expect("sign_bip322 should succeed");
    let expected: Vec<Vec<u8>> = signed.witness.iter().map(|s| s.to_vec()).collect();
    assert_eq!(signed.witness_stack, expected);
}

#[tokio::test]
async fn signed_proof_script_type_matches_wallet_script_type() {
    // CRIT-01 client-side seed: signed.script_type is sourced from
    // wallet.script_type() (descriptor outer-wrapper), NEVER from a CLI flag.
    // Exercise all three descriptor paths + the WIF path.
    for st in [ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh] {
        let w = BdkClientWallet::generate(DUMMY_OUTPOINT, NET, st)
            .expect("descriptor generate should succeed");
        let signed = w.sign_bip322(TEST_MESSAGE).expect("sign_bip322 should succeed");
        assert_eq!(signed.script_type, w.script_type());
        assert_eq!(signed.script_type, st);
    }
    let w = BdkClientWallet::from_wif(TEST_WIF, DUMMY_OUTPOINT, Network::Regtest)
        .expect("from_wif should succeed");
    let signed = w.sign_bip322(TEST_MESSAGE).expect("sign_bip322 should succeed");
    assert_eq!(signed.script_type, w.script_type());
    assert_eq!(signed.script_type, ScriptType::P2wpkh);
}

// ---------------------------------------------------------------------------
// Phase 19 Plan 19-01 — BIP322-05 SC#1 byte-equality parity tests
// (D-118 + D-119; T-19-C mitigation).
//
// Asserts that `shared::bip322::sign_simple` produces the SAME witness bytes
// as `BdkClientWallet::sign_bip322` for the same (key, message). Safe per
// Phase 19 RESEARCH §Q1: bdk_wallet 2.3 uses `sign_schnorr_no_aux_rand`
// (deterministic) for P2TR; P2SH-P2WPKH uses `sign_ecdsa` (RFC 6979
// deterministic). Both tests use single-key WIF descriptors (RESEARCH §Q2:
// bdk_wallet 2.3 accepts `tr(<WIF>)` and `sh(wpkh(<WIF>))` directly), and
// recover the SAME SecretKey from `TEST_WIF` so both signing paths see an
// identical (key, message) input.
//
// Network: Regtest (NOT the file's NET = Signet) — TEST_WIF is the canonical
// Bitcoin Core regtest "Hello World" WIF and bdk's WIF parser requires the
// network to match.
// ---------------------------------------------------------------------------

const PARITY_TEST_MESSAGE: &str = "blindjoin:19-01:parity:byte-for-byte";

/// Recover the secp256k1 SecretKey from the file's TEST_WIF constant.
fn parity_secret_key() -> bitcoin::secp256k1::SecretKey {
    bitcoin::PrivateKey::from_wif(TEST_WIF)
        .expect("test WIF is valid")
        .inner
}

/// Derive the on-chain P2TR address controlling the UTXO spent by the
/// parity test wallet (Note A in RESEARCH §Q2).
fn parity_p2tr_address() -> bitcoin::Address {
    use bitcoin::key::TapTweak;
    use bitcoin::secp256k1::{Keypair, Secp256k1};

    let secp = Secp256k1::new();
    let sk = parity_secret_key();
    let keypair = Keypair::from_secret_key(&secp, &sk);
    let tweaked = keypair.tap_tweak(&secp, None);
    let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
    bitcoin::Address::p2tr_tweaked(
        tweaked_xonly.dangerous_assume_tweaked(),
        Network::Regtest,
    )
}

/// Derive the on-chain P2SH-P2WPKH address controlling the UTXO spent by
/// the parity test wallet (Note A in RESEARCH §Q2).
fn parity_p2sh_p2wpkh_address() -> bitcoin::Address {
    use bitcoin::secp256k1::Secp256k1;

    let secp = Secp256k1::new();
    let sk = parity_secret_key();
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let compressed = bitcoin::PublicKey::new(pk);
    let wpkh = compressed.wpubkey_hash().expect("compressed key");
    let redeem = bitcoin::ScriptBuf::new_p2wpkh(&wpkh);
    bitcoin::Address::p2sh(&redeem, Network::Regtest).expect("p2sh derivation")
}

#[tokio::test]
async fn p2tr_shared_sign_matches_bdk_sign_byte_for_byte() {
    // D-118: SC#1 byte-equality closure (T-19-C mitigation). Safe per
    // RESEARCH §Q1 — bdk_wallet 2.3 uses sign_schnorr_no_aux_rand and our
    // shared::bip322::p2tr::sign body (Plan 19-01 Task 1) uses the SAME
    // call; both produce identical 64-byte BIP-341 SIGHASH_DEFAULT
    // signatures over the canonical BIP-322 to_sign sighash.
    let descriptor = format!("tr({TEST_WIF})");
    let utxo_address = parity_p2tr_address();
    let wallet = BdkClientWallet::from_descriptor(
        &descriptor,
        DUMMY_OUTPOINT,
        &utxo_address.to_string(),
        Network::Regtest,
        ScriptType::P2tr,
    )
    .expect("P2TR single-key WIF descriptor should construct");

    let spk = wallet.script_pubkey();
    let sk = parity_secret_key();

    // Defensive sanity: catch a regression in bdk's single-key descriptor
    // parsing BEFORE the sign call. The wallet's on-chain SPK must
    // byte-equal the SPK derived from the same SecretKey.
    let expected_spk: bitcoin::ScriptBuf = utxo_address.script_pubkey();
    assert_eq!(
        spk, expected_spk,
        "wallet.script_pubkey() must byte-equal the SPK derived from TEST_WIF"
    );

    let bdk_signed = wallet
        .sign_bip322(PARITY_TEST_MESSAGE)
        .expect("bdk sign_bip322 (P2TR descriptor) should succeed");

    let shared_witness = sign_simple(
        ScriptType::P2tr,
        &spk,
        &sk,
        PARITY_TEST_MESSAGE.as_bytes(),
    )
    .expect("shared::bip322::sign_simple P2TR should succeed");

    assert_eq!(
        bdk_signed.witness, shared_witness,
        "P2TR bdk vs shared::bip322 witnesses must be byte-equal (D-118)"
    );

    // Belt-and-suspenders: both witnesses must verify under verify_simple.
    verify_simple(
        ScriptType::P2tr,
        &spk,
        &shared_witness,
        PARITY_TEST_MESSAGE.as_bytes(),
        Network::Regtest,
    )
    .expect("P2TR parity witness must verify under verify_simple");
}

#[tokio::test]
async fn p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte() {
    // D-119: SC#2 byte-equality. ECDSA via sign_ecdsa is RFC 6979
    // deterministic on BOTH sides — byte-equality always holds with no
    // aux-rand caveat.
    let descriptor = format!("sh(wpkh({TEST_WIF}))");
    let utxo_address = parity_p2sh_p2wpkh_address();
    let wallet = BdkClientWallet::from_descriptor(
        &descriptor,
        DUMMY_OUTPOINT,
        &utxo_address.to_string(),
        Network::Regtest,
        ScriptType::P2shP2wpkh,
    )
    .expect("P2SH-P2WPKH single-key WIF descriptor should construct");

    let spk = wallet.script_pubkey();
    let sk = parity_secret_key();

    let expected_spk: bitcoin::ScriptBuf = utxo_address.script_pubkey();
    assert_eq!(
        spk, expected_spk,
        "wallet.script_pubkey() must byte-equal the SPK derived from TEST_WIF"
    );

    let bdk_signed = wallet
        .sign_bip322(PARITY_TEST_MESSAGE)
        .expect("bdk sign_bip322 (P2SH-P2WPKH descriptor) should succeed");

    let shared_witness = sign_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &sk,
        PARITY_TEST_MESSAGE.as_bytes(),
    )
    .expect("shared::bip322::sign_simple P2SH-P2WPKH should succeed");

    assert_eq!(
        bdk_signed.witness, shared_witness,
        "P2SH-P2WPKH bdk vs shared::bip322 witnesses must be byte-equal (D-119)"
    );

    verify_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &shared_witness,
        PARITY_TEST_MESSAGE.as_bytes(),
        Network::Regtest,
    )
    .expect("P2SH-P2WPKH parity witness must verify under verify_simple");
}

// Defensive: ensure the deterministic outpoint placeholder is itself parseable
// (catches accidental DUMMY_OUTPOINT corruption during refactors).
#[test]
fn dummy_outpoint_is_well_formed() {
    let parts: Vec<&str> = DUMMY_OUTPOINT.split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 64);
    assert!(u32::from_str(parts[1]).is_ok());
}
