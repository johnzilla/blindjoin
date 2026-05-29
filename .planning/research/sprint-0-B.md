# Sprint-0-B: bdk_wallet 2.3 P2TR BIP-322 sign PoC

**Branch:** spike/14-B-bdk-p2tr-poc (pushed to origin; NOT merged to main per D-19)
**PoC location:** client/examples/spike-p2tr.rs (spike branch only; per CD-4)
**Invocation:** `cargo run -p client --example spike-p2tr` (the `-p client` selector is required — workspace root has no package; only member crates host examples)
**Date:** 2026-05-29
**Sprint cap:** 2 days (D-18)

## 8-Step Sequence (per CONTEXT.md Specifics #2)

1. Deterministic seed: `[0u8; 32]` (throwaway PoC, NOT production key material)
2. BIP-86 descriptor: `tr(.../86'/1'/0'/0/*)` (testnet/signet coin type 1') via `bdk_wallet::template::Bip86`
3. `to_spend` via `shared::bip322::build_bip322_to_spend(&p2tr_spk, &msg_hash)`
4. `to_sign` via `shared::bip322::build_bip322_to_sign(&to_spend)`
5. PSBT input populated with REAL on-chain `witness_utxo` = `TxOut { value: 0, script_pubkey: <P2TR SPK> }` (value = 0 is BIP-322 Section 4 canonical, NOT a v1.3 Phase 12 zero-placeholder strawman)
6. `wallet.sign(&mut psbt, SignOptions { trust_witness_utxo: true, ..Default::default() })` (load-bearing per v1.3 Phase 12)
7. Extract `psbt.inputs[0].tap_key_sig`. For finalizable single-key taproot inputs, bdk_wallet 2.3 moves the sig from `tap_key_sig` into `final_script_witness`; recover from whichever path bdk took.
8. Verify the 64-byte Schnorr sig via `secp256k1::verify_schnorr(&sig, &Message::from_digest(sighash), &xonly_pubkey)`

## PoC Source (verbatim from client/examples/spike-p2tr.rs)

```rust
//! Sprint-0-B PoC: bdk_wallet 2.3 BIP-322 sign for a P2TR (taproot keypath) descriptor.
//!
//! Resolves Open Decision #4 (per Phase 14 CONTEXT.md, Specifics #2): does
//! bdk_wallet's PSBT signer produce a 64-byte Schnorr keypath witness for a
//! BIP-322 `to_sign` PSBT whose `witness_utxo` carries a P2TR scriptPubKey, or
//! does the manual `secp256k1::Secp256k1::sign_schnorr` fallback (D-15) need to
//! fire?
//!
//! WARNING: throwaway PoC. The seed is hardcoded `[0u8; 32]` and is NOT
//! production key material; it exists ONLY to make this PoC byte-deterministic
//! across machines/reviewers so the verdict is reproducible.
//!
//! Invocation: `cargo run -p client --example spike-p2tr`
//! (The `-p client` selector is required because the workspace root has no
//! package; the workspace has multiple member crates so cargo cannot
//! disambiguate the example by name alone.)
//!
//! The 8-step sequence (verbatim from 14-CONTEXT.md Specifics #2):
//!   1. Deterministic seed `[0u8; 32]`
//!   2. BIP-86 (`tr(.../86'/1'/0'/0/*)`) descriptor via bdk_wallet's Bip86 template
//!   3. `shared::bip322::build_bip322_to_spend(&spk, &bip322_message_hash(msg))`
//!   4. `shared::bip322::build_bip322_to_sign(&to_spend)`
//!   5. Wrap to_sign in a Psbt; populate `inputs[0].witness_utxo` with the REAL
//!      on-chain witness_utxo — `TxOut { value: 0, script_pubkey: <P2TR SPK> }`
//!      (value = 0 is BIP-322-correct: it is the to_spend output amount per spec,
//!      NOT a v1.3 Phase 12 zero-placeholder strawman)
//!   6. `wallet.sign(&mut psbt, SignOptions { trust_witness_utxo: true, ..Default::default() })`
//!      (`trust_witness_utxo: true` is LOAD-BEARING per v1.3 Phase 12 — without
//!      it bdk demands non_witness_utxo and silently produces no signature on
//!      segwit; see client/src/wallet.rs:260 for the in-source rationale)
//!   7. Extract `psbt.inputs[0].tap_key_sig`. Some(_) = bdk produced a Schnorr
//!      keypath sig; None = bdk did not sign.
//!   8. Verify the 64-byte Schnorr signature against the expected sighash via
//!      `secp256k1::verify_schnorr`. PASS = Ok(()); FAIL = Err(_).
//!
//! Output is stdout-parseable (one key per line, for sprint-0-B.md embedding):
//!   STEP_6_BDK_SIGN: <Ok | Err: ...>
//!   STEP_7_EXTRACT_TAP_KEY_SIG: <Some(<hex>) | None>
//!   STEP_8_VERIFY_SCHNORR: <Ok | Err: ...>
//!   WITNESS_HEX: <128-char hex string on PASS, empty on FAIL>
//!   VERDICT: <PASS | FAIL>

// bdk_wallet 2.3 still ships SignOptions on the wallet sign path; the
// "PSBT signing was moved to bitcoin::psbt" deprecation marker is part of a
// future migration not yet wired up in 2.3. The production client wallet
// (client/src/wallet.rs:269) uses the same #[allow(deprecated)] pattern.
#![allow(deprecated)]

use bdk_wallet::signer::SignOptions;
use bdk_wallet::template::Bip86;
use bdk_wallet::{KeychainKind, Wallet};
use bitcoin::bip32::Xpriv;
use bitcoin::hashes::Hash;
use bitcoin::psbt::Psbt;
use bitcoin::secp256k1::{Message, Secp256k1, XOnlyPublicKey};
use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
use bitcoin::{Amount, Network, TxOut};

use shared::bip322::{bip322_message_hash, build_bip322_to_sign, build_bip322_to_spend};

const MESSAGE: &[u8] = b"blindjoin spike-0-B taproot test message";

fn main() {
    // ---------------------------------------------------------------------
    // Step 1: deterministic seed (throwaway; NOT production key material).
    // ---------------------------------------------------------------------
    let seed: [u8; 32] = [0u8; 32];

    // ---------------------------------------------------------------------
    // Step 2: BIP-86 descriptor (tr(.../86'/1'/0'/0/*) for testnet/signet).
    // Use bdk_wallet's Bip86 template so the descriptor string is generated
    // canonically (handles the hardened-derivation + checksum suffix).
    // The Bip86 template builds tr(xprv/86h/<coin>h/0h/<keychain>/*) — coin
    // type 1' for testnet/signet, 0' for mainnet — matching Phase 17
    // WALLET-01's BIP-86 spec.
    // ---------------------------------------------------------------------
    let xpriv = Xpriv::new_master(Network::Testnet, &seed)
        .expect("Xpriv::new_master must succeed for a 32-byte seed");

    let mut wallet = Wallet::create(
        Bip86(xpriv, KeychainKind::External),
        Bip86(xpriv, KeychainKind::Internal),
    )
    .network(Network::Testnet)
    .create_wallet_no_persist()
    .expect("Wallet::create must succeed for a well-formed BIP-86 template");

    // Reveal the first external address so the wallet's signer container
    // contains the derived private key for index 0 (peek_address alone does
    // not always trigger derivation-state population that the signer needs).
    let address_info = wallet.reveal_next_address(KeychainKind::External);
    let p2tr_spk = address_info.address.script_pubkey();
    assert!(
        p2tr_spk.is_p2tr(),
        "BIP-86 descriptor must produce a P2TR scriptPubKey"
    );

    // ---------------------------------------------------------------------
    // Step 3: BIP-322 to_spend virtual tx.
    // Reuses the script-type-NEUTRAL primitive from shared/src/bip322.rs
    // (V1.4-MOD-07: single source of truth for to_spend/to_sign/message_hash).
    // ---------------------------------------------------------------------
    let msg_hash = bip322_message_hash(MESSAGE);
    let to_spend = build_bip322_to_spend(&p2tr_spk, &msg_hash);

    // ---------------------------------------------------------------------
    // Step 4: BIP-322 to_sign virtual tx.
    // ---------------------------------------------------------------------
    let to_sign = build_bip322_to_sign(&to_spend);

    // ---------------------------------------------------------------------
    // Step 5: Wrap to_sign in a PSBT and populate witness_utxo with the REAL
    // on-chain witness_utxo. The to_spend output (TxOut { value: 0,
    // script_pubkey: p2tr_spk.clone() }) IS the real on-chain witness_utxo
    // for the synthetic to_sign tx — value = 0 is BIP-322-correct (Section 4:
    // "the to_spend output has value 0"). This is NOT a v1.3 Phase 12 zero
    // placeholder: the value is canonically zero per the BIP-322 spec, not a
    // sentinel-for-missing-data.
    // ---------------------------------------------------------------------
    let witness_utxo = TxOut {
        value: Amount::ZERO,
        script_pubkey: p2tr_spk.clone(),
    };
    let mut psbt =
        Psbt::from_unsigned_tx(to_sign.clone()).expect("to_sign must be a valid unsigned tx");
    psbt.inputs[0].witness_utxo = Some(witness_utxo.clone());

    // ---------------------------------------------------------------------
    // Step 6: ask bdk to sign with trust_witness_utxo: true.
    // trust_witness_utxo: true is LOAD-BEARING per v1.3 Phase 12 — without it
    // bdk demands non_witness_utxo and silently produces no signature for a
    // segwit/taproot input whose only prevout data is witness_utxo. See
    // client/src/wallet.rs:260 for the in-source rationale that this PoC
    // reproduces (and which Phase 17 WALLET-02 will inherit if the verdict
    // here is PASS).
    // ---------------------------------------------------------------------
    let sign_options = SignOptions {
        trust_witness_utxo: true,
        ..SignOptions::default()
    };
    let step6_result = wallet.sign(&mut psbt, sign_options);
    let step6_line = match &step6_result {
        Ok(finalized) => format!("Ok(finalized={finalized})"),
        Err(e) => format!("Err: {e}"),
    };
    println!("STEP_6_BDK_SIGN: {step6_line}");

    // ---------------------------------------------------------------------
    // Step 7: extract the 64-byte Schnorr sig that bdk produced.
    //
    // bdk_wallet 2.3 takes one of two code paths for a P2TR keyspend input
    // depending on whether the PSBT is "finalizable":
    //   (a) NOT finalizable yet → bdk populates psbt.inputs[0].tap_key_sig
    //       with the structured taproot::Signature.
    //   (b) Finalizable (single-key taproot is the canonical finalizable
    //       case) → bdk moves the sig into final_script_witness as the
    //       single-element witness stack `[<schnorr-sig-64-or-65-bytes>]`
    //       and clears the partial-sig / tap_key_sig fields.
    //
    // For Open Decision #4's question — "did bdk produce a valid 64-byte
    // Schnorr keypath witness?" — both paths are equally valid: the witness
    // is what the broadcast tx will carry. Phase 17 WALLET-02's job is to
    // extract that witness regardless of whether it sits in tap_key_sig or
    // final_script_witness. Look in both places and pin down which path bdk
    // 2.3 actually exercises.
    // ---------------------------------------------------------------------
    let tap_key_sig_opt = psbt.inputs[0].tap_key_sig;
    let final_witness_opt = psbt.inputs[0].final_script_witness.clone();

    let (witness_hex, step7_line) = if let Some(sig) = tap_key_sig_opt {
        let hex_str = hex::encode(sig.signature.serialize());
        (
            hex_str.clone(),
            format!("Some(tap_key_sig={hex_str})"),
        )
    } else if let Some(witness) = final_witness_opt.as_ref() {
        // Keyspend finalized witness: 1 element = 64-byte schnorr sig
        // (sighash_type Default) or 65 bytes (sig + non-default sighash byte).
        // Pull bytes 0..64 as the canonical Schnorr signature.
        let elems: Vec<&[u8]> = witness.iter().collect();
        if elems.len() == 1 && (elems[0].len() == 64 || elems[0].len() == 65) {
            let sig_bytes = &elems[0][..64];
            let hex_str = hex::encode(sig_bytes);
            (
                hex_str.clone(),
                format!(
                    "Some(final_script_witness elems={} first_len={} sig64={hex_str})",
                    elems.len(),
                    elems[0].len()
                ),
            )
        } else {
            (
                String::new(),
                format!(
                    "None (final_script_witness has unexpected shape: elems={} lens={:?})",
                    elems.len(),
                    elems.iter().map(|e| e.len()).collect::<Vec<_>>()
                ),
            )
        }
    } else {
        (String::new(), "None".to_string())
    };
    println!("STEP_7_EXTRACT_TAP_KEY_SIG: {step7_line}");

    // ---------------------------------------------------------------------
    // Step 8: verify the Schnorr sig against the expected sighash.
    //   sighash    = taproot_key_spend_signature_hash(0, Prevouts::All([witness_utxo]), Default)
    //   xonly_key  = bytes 2..34 of the P2TR scriptPubKey (skip OP_1 + push-32)
    //   verify     = Secp256k1::new().verify_schnorr(&sig, &sighash, &xonly_key)
    // PASS = Ok(()); FAIL = Err(_).
    // ---------------------------------------------------------------------
    let (verdict, step8_line) = if witness_hex.is_empty() {
        (
            "FAIL".to_string(),
            "Err: no Schnorr sig recovered from PSBT input".to_string(),
        )
    } else {
        let expected_sighash = SighashCache::new(&to_sign)
            .taproot_key_spend_signature_hash(
                0,
                &Prevouts::All(&[witness_utxo.clone()]),
                TapSighashType::Default,
            )
            .expect("sighash computation must not fail for a well-formed to_sign tx");

        // The P2TR scriptPubKey encoding is: OP_1 (0x51) <push-32> <x-only-pubkey-32-bytes>.
        // Skip the first 2 bytes to recover the 32-byte x-only pubkey.
        let spk_bytes = p2tr_spk.as_bytes();
        let xonly = XOnlyPublicKey::from_slice(&spk_bytes[2..34])
            .expect("P2TR scriptPubKey embeds a valid x-only pubkey at bytes 2..34");

        let sig_bytes = hex::decode(&witness_hex)
            .expect("witness_hex was produced by hex::encode and must round-trip");
        let sig = bitcoin::secp256k1::schnorr::Signature::from_slice(&sig_bytes)
            .expect("recovered Schnorr sig must be a valid 64-byte secp256k1 schnorr sig");

        let secp = Secp256k1::verification_only();
        let sighash_bytes = expected_sighash.to_byte_array();
        let msg = Message::from_digest(sighash_bytes);
        let res = secp.verify_schnorr(&sig, &msg, &xonly);
        match res {
            Ok(()) => ("PASS".to_string(), "Ok".to_string()),
            Err(e) => ("FAIL".to_string(), format!("Err: {e}")),
        }
    };
    println!("STEP_8_VERIFY_SCHNORR: {step8_line}");

    // ---------------------------------------------------------------------
    // Output the witness hex (empty on FAIL) and the column-0 verdict line.
    // ---------------------------------------------------------------------
    println!("WITNESS_HEX: {witness_hex}");
    println!("VERDICT: {verdict}");

    // Exit non-zero on FAIL so CI / scripts can detect it; success on PASS.
    // Note: a FAIL verdict (with a clean printed line) is a valid PoC
    // outcome per Sprint-0-B's binary-by-design verdict (D-14). A panic or
    // compile error is NOT.
    if verdict == "FAIL" {
        std::process::exit(1);
    }
}
```

## Captured Output (verbatim from `cargo run -p client --example spike-p2tr`)

```
   Compiling client v0.1.0 (/Users/john/Desktop/vault/projects/github.com/blindjoin/client)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.66s
     Running `target/debug/examples/spike-p2tr`
STEP_6_BDK_SIGN: Ok(finalized=true)
STEP_7_EXTRACT_TAP_KEY_SIG: Some(final_script_witness elems=1 first_len=64 sig64=295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069)
STEP_8_VERIFY_SCHNORR: Ok
WITNESS_HEX: 295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069
VERDICT: PASS
```

## Witness Hex (extracted)

`295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069`

64 bytes = 128 hex chars. Schnorr BIP-340 signature, sighash type Default (no trailing sighash byte). Recovered from `psbt.inputs[0].final_script_witness` (single witness element of exactly 64 bytes), not from `psbt.inputs[0].tap_key_sig` (which bdk cleared during finalization).

## verify_schnorr Result

`Ok(())`

`secp256k1::Secp256k1::verify_schnorr(&sig, &Message::from_digest(taproot_key_spend_signature_hash(0, Prevouts::All([witness_utxo]), TapSighashType::Default)), &xonly_pubkey)` returned `Ok(())` — the 64-byte Schnorr signature is a valid BIP-340 keyspend signature over the BIP-341 sighash for the BIP-322 `to_sign` tx with the P2TR `witness_utxo`.

## Overall verdict

PASS: bdk_wallet 2.3's PSBT signer produces a valid 64-byte Schnorr keypath witness for a BIP-322 `to_sign` PSBT under a BIP-86 (`tr(.../86'/1'/0'/0/*)`) descriptor when `SignOptions { trust_witness_utxo: true }` is set and `witness_utxo` is populated with the canonical BIP-322 to_spend output (value = 0, script_pubkey = P2TR SPK). Step 6 (bdk sign) returned `Ok(finalized=true)`, step 7 recovered the 64-byte sig from `final_script_witness`, and step 8 (`secp256k1::verify_schnorr`) returned `Ok(())`.

### Implementation note for Phase 17 WALLET-02

bdk_wallet 2.3 takes the **finalized** code path for single-key P2TR keyspend: the sig lands in `psbt.inputs[0].final_script_witness` (a single 64-byte witness element), and `psbt.inputs[0].tap_key_sig` is `None` after sign. Phase 17 WALLET-02's signing path must extract the witness from `final_script_witness` for the P2TR descriptor case (the existing client/src/wallet.rs P2WPKH path already does the analogous fallback at line 277-285). If bdk ever changes this behavior in a 2.x point release (it would need a regression-grade reason to do so for finalizable single-key inputs), the existing `final_script_witness` → `partial_sigs` / `tap_key_sig` two-step extraction in client/src/wallet.rs already handles both cases.

## Recommendation

bdk path

Rationale:
- PASS → Phase 17 WALLET-02 uses bdk_wallet's PSBT-sign for the P2TR descriptor; no new `shared/src/bip322/p2tr.rs::sign_p2tr_keypath` needed. D-05 asymmetry still applies (bdk has no BIP-322 sign API per se — we're using its PSBT signer for a BIP-322-shaped PSBT; verify path remains ours regardless).
- FAIL → Phase 17 WALLET-02 implements `shared/src/bip322/p2tr.rs::sign_p2tr_keypath` per D-15 (80 LOC budget, reuses `shared::bip322::bip322_message_hash`, symmetric with D-04's module split).
- INCONCLUSIVE → same as FAIL per D-18 timebox + D-15 default.

## Input contract for Plan 14-03 (ADR Decision #4)

Plan 14-03 sets ADR Decision #4 STATUS line to:

> **Status:** ACCEPTED (bdk path)

Plus the consequences section records:
- Positive: zero new LOC under v1.4 for the P2TR sign path; bdk-validated via existing dep
- Negative: tied to bdk_wallet 2.x — if v1.5 needs to migrate off bdk_wallet (e.g., Arti workspace constraints), the P2TR sign path becomes a swap target
- Neutral: D-05 asymmetry stands — verify path remains ours regardless of sign path choice

Plan 14-03's grep target `(bdk path|manual fallback per D-15)` matches `bdk path` in the Recommendation section above.

## Reproducibility

- Spike branch HEAD SHA: `9ff73cd286920d1e9fcac1e6506e7e3300b7abe7`
- PoC seed: `[0u8; 32]` (deterministic; results reproducible byte-for-byte across machines)
- Workspace `bitcoin` pin: `"0.32"` (resolved to `bitcoin v0.32.8` per sprint-0-A's cargo tree)
- Workspace `bdk_wallet` pin: `"2.3"` (resolved to `bdk_wallet v2.3.0` per local registry cache)
- Toolchain at PoC time: cargo 1.95.0 (f2d3ce0bd 2026-03-21), rustc 1.95.0 (59807616e 2026-04-14)
- Sprint elapsed: < 1 hour (well within D-18's 2-day cap)
- To reproduce locally:
  ```
  git fetch origin spike/14-B-bdk-p2tr-poc
  git checkout spike/14-B-bdk-p2tr-poc
  cargo run -p client --example spike-p2tr
  ```
  Expected last 5 lines of stdout (deterministic given the `[0u8; 32]` seed):
  ```
  STEP_6_BDK_SIGN: Ok(finalized=true)
  STEP_7_EXTRACT_TAP_KEY_SIG: Some(final_script_witness elems=1 first_len=64 sig64=295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069)
  STEP_8_VERIFY_SCHNORR: Ok
  WITNESS_HEX: 295d214353bd7fc07ef2345b99a89307740d102abcf59a5503c4139f3629d6dd758421d358baab75f909e6c7396b927a1060f648a8b8a0569ec4529f285ac069
  VERDICT: PASS
  ```
