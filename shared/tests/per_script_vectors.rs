//! BIP322-04 per-script positive vector tests.
//!
//! Plan 15-03 Task 2 — runs sign↔verify positive tests against the vendored
//! BIP-322 `basic-test-vectors.json` + the `p2sh_p2wpkh_supplement.json` so
//! every script type (P2WPKH, P2TR, P2SH-P2WPKH) has ≥1 passing positive
//! vector through the public dispatcher (`shared::bip322::verify_simple` +
//! `sign_simple`). Compile-time fixture loading per D-33 (`include_str!`);
//! no network in CI.
//!
//! Phase 19 Plan 19-02 (BIP322-07) migrated the P2TR + P2SH-P2WPKH
//! sign↔verify roundtrip tests off the deleted test-only mirror onto
//! the production `sign_simple` dispatcher — the positive-vector suite
//! now exercises the production `sign` bodies shipped in Plan 19-01.
//!
//! Each #[test] fn asserts that it iterated ≥1 vector before declaring
//! success — a future fixture-bump that drops a script type is caught at
//! CI time. The dispatcher API is the ONLY shared::bip322 entry point used
//! (no direct `p2wpkh::`, `p2tr::`, `p2sh_p2wpkh::` module access — those
//! are pub(crate) per D-27).

use base64::Engine;
use bitcoin::consensus::Decodable;
use bitcoin::secp256k1::SecretKey;
use bitcoin::{Address, Network, ScriptBuf, Witness};
use serde_json::Value;
use shared::bip322::{sign_simple, verify_simple, Bip322Error, ScriptType};
use std::str::FromStr;

// --- Compile-time fixture loading (D-33) ---

const BASIC_VECTORS: &str = include_str!("fixtures/bip322/basic-test-vectors.json");
const P2SH_P2WPKH_SUPPLEMENT: &str = include_str!("fixtures/bip322/p2sh_p2wpkh_supplement.json");

// --- Helpers (free fns at module scope) ---

/// Classify a `simple` entry into a [`ScriptType`] using the explicit `type`
/// field if present, falling back to the address prefix. Returns `None` for
/// out-of-scope shapes (e.g., `p2wsh-multisig-3of3` from upstream
/// `basic-test-vectors.json`).
fn classify(addr: &str, explicit_type: Option<&str>) -> Option<ScriptType> {
    if let Some(t) = explicit_type {
        match t {
            "p2wpkh" => return Some(ScriptType::P2wpkh),
            "p2tr" => return Some(ScriptType::P2tr),
            "p2sh-p2wpkh" => return Some(ScriptType::P2shP2wpkh),
            _ => return None, // out of scope (e.g., p2wsh-multisig-3of3)
        }
    }
    // Fallback by address prefix
    if addr.starts_with("bc1q") || addr.starts_with("tb1q") {
        Some(ScriptType::P2wpkh)
    } else if addr.starts_with("bc1p") || addr.starts_with("tb1p") {
        Some(ScriptType::P2tr)
    } else if addr.starts_with('3') || addr.starts_with('2') {
        Some(ScriptType::P2shP2wpkh)
    } else {
        None
    }
}

/// Recover the on-chain script_pubkey from an address string. Tries
/// mainnet first (bc1*, 3...) then signet/testnet (tb1*, 2...).
fn address_to_spk(addr: &str) -> Option<ScriptBuf> {
    for net in [Network::Bitcoin, Network::Signet, Network::Testnet, Network::Regtest] {
        if let Ok(parsed) = Address::from_str(addr) {
            if let Ok(checked) = parsed.require_network(net) {
                return Some(checked.script_pubkey());
            }
        }
    }
    None
}

/// Decode a base64 `bip322_signatures[i]` entry into a canonical
/// `bitcoin::Witness`. Returns `None` on any decode failure (including the
/// May 2026 upstream malformed P2WPKH prefix — RESEARCH note in
/// `shared/tests/fixtures/bip322/README.md`).
fn base64_to_witness(b64: &str) -> Option<Witness> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let mut cursor = bitcoin::io::Cursor::new(&bytes[..]);
    Witness::consensus_decode_from_finite_reader(&mut cursor).ok()
}

/// Iterate all entries in a serde_json::Value::Array. Returns the count of
/// vectors that were classifiable AND had a decodable witness AND verified
/// via the dispatcher for the target script type. Used by both the
/// upstream-vector path and the supplement path.
///
/// `target` filters which script type to count — if `Some(P2wpkh)`, only
/// P2WPKH entries are processed. If `None`, all classifiable entries are
/// processed regardless of type.
///
/// Each entry classified as `target` MUST verify (an `assert!` panics on
/// failure); entries that don't match `target` are skipped silently.
fn run_positive_vectors(entries: &[Value], target: ScriptType) -> usize {
    let mut count = 0_usize;
    for (idx, entry) in entries.iter().enumerate() {
        let address = entry.get("address").and_then(Value::as_str).unwrap_or("");
        let explicit_type = entry.get("type").and_then(Value::as_str);
        let message = entry.get("message").and_then(Value::as_str).unwrap_or("");
        let signatures = entry
            .get("bip322_signatures")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let script_type = match classify(address, explicit_type) {
            Some(st) => st,
            None => continue, // out of scope
        };
        if script_type != target {
            continue;
        }

        let spk = match address_to_spk(address) {
            Some(spk) => spk,
            None => continue,
        };

        for (sig_idx, sig_value) in signatures.iter().enumerate() {
            let b64 = match sig_value.as_str() {
                Some(s) => s,
                None => continue,
            };
            let witness = match base64_to_witness(b64) {
                Some(w) => w,
                None => {
                    // Defensive skip for malformed upstream encodings — see
                    // shared/tests/fixtures/bip322/README.md "May 2026 upstream
                    // encoding change" note. Clean coverage comes from the
                    // supplement; this skip preserves the verbatim vendoring.
                    eprintln!(
                        "skipped malformed signature: entry {idx}, sig {sig_idx} ({} bytes b64)",
                        b64.len(),
                    );
                    continue;
                }
            };
            let result = verify_simple(
                script_type,
                &spk,
                &witness,
                message.as_bytes(),
                Network::Bitcoin,
            );
            assert!(
                result.is_ok(),
                "verify_simple failed for {:?} entry {idx} sig {sig_idx}: {result:?}",
                target,
            );
            count += 1;
        }
    }
    count
}

// --- Test 1: P2WPKH upstream vectors via dispatcher ---

#[test]
fn test_p2wpkh_vectors_verify_via_dispatcher() {
    let basic: Value = serde_json::from_str(BASIC_VECTORS).expect("parse basic-test-vectors.json");
    let basic_simple: Vec<Value> = basic
        .get("simple")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let basic_count = run_positive_vectors(&basic_simple, ScriptType::P2wpkh);

    let supplement: Vec<Value> =
        serde_json::from_str(P2SH_P2WPKH_SUPPLEMENT).expect("parse supplement");
    let supplement_count = run_positive_vectors(&supplement, ScriptType::P2wpkh);

    let total = basic_count + supplement_count;
    eprintln!(
        "P2WPKH positives verified: basic={basic_count}, supplement={supplement_count}, total={total}",
    );
    // RESEARCH A3: ≥1 P2WPKH positive vector must run. Supplement provides
    // canonical encoding; upstream P2WPKH at d77863fb9e may all skip due to
    // malformed encoding (README note). Total across both files must be ≥1.
    assert!(
        total >= 1,
        "BIP322-04 gate: at least 1 P2WPKH positive vector must verify (basic={basic_count}, supplement={supplement_count})",
    );
}

// --- Test 2: P2TR upstream vector via dispatcher ---

#[test]
fn test_p2tr_vectors_verify_via_dispatcher() {
    let basic: Value = serde_json::from_str(BASIC_VECTORS).expect("parse basic-test-vectors.json");
    let basic_simple: Vec<Value> = basic
        .get("simple")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let count = run_positive_vectors(&basic_simple, ScriptType::P2tr);
    eprintln!("P2TR positives verified (basic-test-vectors.json): {count}");
    assert!(
        count >= 1,
        "BIP322-04 gate: at least 1 P2TR positive vector must verify",
    );
}

// --- Test 3: P2SH-P2WPKH supplement via dispatcher ---

#[test]
fn test_p2sh_p2wpkh_supplement_verify_via_dispatcher() {
    let supplement: Vec<Value> =
        serde_json::from_str(P2SH_P2WPKH_SUPPLEMENT).expect("parse supplement");
    let count = run_positive_vectors(&supplement, ScriptType::P2shP2wpkh);
    eprintln!("P2SH-P2WPKH positives verified (supplement): {count}");
    assert!(
        count >= 1,
        "BIP322-04 gate: at least 1 P2SH-P2WPKH positive vector must verify",
    );
}

// --- Test 4: P2WPKH sign↔verify roundtrip via production dispatcher ---

#[test]
fn test_p2wpkh_sign_verify_roundtrip_via_dispatcher() {
    use bitcoin::secp256k1::{PublicKey, Secp256k1};
    use bitcoin::PublicKey as BtcPubkey;

    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[0x05; 32]).expect("deterministic key");
    let pk = PublicKey::from_secret_key(&secp, &key);
    let compressed = BtcPubkey::new(pk);
    let spk = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed key"));

    let message = b"blindjoin:15-03:per-script-vector-test:p2wpkh";

    // P2WPKH production sign_simple is fully implemented per CD-6.
    let witness = sign_simple(ScriptType::P2wpkh, &spk, &key, message).expect("sign_simple p2wpkh");

    // Network::Regtest is acceptable because the bip322 crate's verify is
    // network-agnostic at the SPK byte layer (RESEARCH Pitfall 5).
    let result = verify_simple(
        ScriptType::P2wpkh,
        &spk,
        &witness,
        message,
        Network::Regtest,
    );
    assert!(
        result.is_ok(),
        "P2WPKH sign↔verify roundtrip failed: {result:?}",
    );
    // Witness count assertion: P2WPKH must produce exactly 2 stack items.
    assert_eq!(
        witness.len(),
        2,
        "P2WPKH witness must be [sig, pubkey] (len=2)",
    );
}

// --- Test 5: P2TR sign↔verify roundtrip via production dispatcher ---

#[test]
fn test_p2tr_sign_verify_roundtrip_via_dispatcher() {
    use bitcoin::key::TapTweak;
    use bitcoin::secp256k1::{Keypair, Secp256k1, XOnlyPublicKey};

    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[0x06; 32]).expect("deterministic key");
    let keypair = Keypair::from_secret_key(&secp, &key);
    let (xonly, _parity) = XOnlyPublicKey::from_keypair(&keypair);
    // Derive the tap-tweaked output key (BIP-341 keypath-only).
    let tweaked = keypair.tap_tweak(&secp, None);
    let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
    let _ = xonly;
    let spk = ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked());

    let message = b"blindjoin:15-03:per-script-vector-test:p2tr";

    // P2TR production sign_simple ships in Phase 19 Plan 19-01 (D-116 lifted
    // the prior test-only sign body verbatim into production sign + D-111
    // cross-check at top). Plan 19-02 migrated this callsite off the deleted
    // test-only mirror.
    let witness = sign_simple(ScriptType::P2tr, &spk, &key, message)
        .expect("sign_simple p2tr");
    let result = verify_simple(ScriptType::P2tr, &spk, &witness, message, Network::Regtest);
    assert!(
        result.is_ok(),
        "P2TR sign↔verify roundtrip failed: {result:?}",
    );
    assert_eq!(witness.len(), 1, "P2TR witness must be 1 element (Schnorr keypath)");
    let sig_bytes = witness.iter().next().expect("first witness element");
    assert!(
        sig_bytes.len() == 64 || sig_bytes.len() == 65,
        "P2TR sig must be 64 (SIGHASH_DEFAULT) or 65 (SIGHASH_ALL) bytes; got {}",
        sig_bytes.len(),
    );
}

// --- Test 6: P2SH-P2WPKH sign↔verify roundtrip via production dispatcher ---

#[test]
fn test_p2sh_p2wpkh_sign_verify_roundtrip_via_dispatcher() {
    use bitcoin::secp256k1::{PublicKey, Secp256k1};
    use bitcoin::PublicKey as BtcPubkey;

    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&[0x07; 32]).expect("deterministic key");
    let pk = PublicKey::from_secret_key(&secp, &key);
    let compressed = BtcPubkey::new(pk);
    // P2SH-P2WPKH SPK = OP_HASH160 <HASH160(P2WPKH redeem)> OP_EQUAL.
    let inner_p2wpkh =
        ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed key"));
    let spk = ScriptBuf::new_p2sh(&inner_p2wpkh.script_hash());

    let message = b"blindjoin:15-03:per-script-vector-test:p2sh-p2wpkh";

    // P2SH-P2WPKH production sign_simple ships in Phase 19 Plan 19-01 (D-116
    // lifted the prior test-only sign body verbatim + D-111 cross-check +
    // D-117 spk-used-directly). Plan 19-02 migrated this callsite off the
    // deleted test-only mirror.
    let witness = sign_simple(ScriptType::P2shP2wpkh, &spk, &key, message)
        .expect("sign_simple p2sh-p2wpkh");
    let result = verify_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &witness,
        message,
        Network::Regtest,
    );
    assert!(
        result.is_ok(),
        "P2SH-P2WPKH sign↔verify roundtrip failed: {result:?}",
    );
    assert_eq!(
        witness.len(),
        2,
        "P2SH-P2WPKH witness must be [sig, pubkey] (len=2)",
    );
}

// --- Optional defensive helper test: classify works as expected ---

#[test]
fn test_classify_handles_all_script_types_and_skips_unsupported() {
    // P2WPKH bc1q (mainnet)
    assert_eq!(
        classify("bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l", Some("p2wpkh")),
        Some(ScriptType::P2wpkh),
    );
    // P2TR bc1p (mainnet)
    assert_eq!(
        classify(
            "bc1pss0zhytly75awhm6x2hhvd5lnzv3vssgrf9axfheq8ldyzn88ges79fler",
            Some("p2tr"),
        ),
        Some(ScriptType::P2tr),
    );
    // P2SH-P2WPKH 3... (mainnet)
    assert_eq!(
        classify("3HSVzEhCFuH9Z3wvoWTexy7BMVVp3PjS6f", Some("p2sh-p2wpkh")),
        Some(ScriptType::P2shP2wpkh),
    );
    // P2WSH-multisig is out of scope — None.
    assert_eq!(
        classify(
            "bc1qp0ahvfh83088w49k405szqgg4f3pptr7p2g06tdxfjcd40z4lh4q95lsz9",
            Some("p2wsh-multisig-3of3"),
        ),
        None,
    );
}

// Compile-time sanity: the Bip322Error type is reachable through the public
// API surface. (Use it explicitly so an accidental removal trips the build.)
#[allow(dead_code)]
fn _bip322_error_path_check(_: Bip322Error) {}
