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
use shared::bip322::{verify_simple, ScriptType};
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

// Defensive: ensure the deterministic outpoint placeholder is itself parseable
// (catches accidental DUMMY_OUTPOINT corruption during refactors).
#[test]
fn dummy_outpoint_is_well_formed() {
    let parts: Vec<&str> = DUMMY_OUTPOINT.split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 64);
    assert!(u32::from_str(parts[1]).is_ok());
}
