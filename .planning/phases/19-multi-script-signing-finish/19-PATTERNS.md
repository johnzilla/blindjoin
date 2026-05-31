# Phase 19 — Pattern Map

**Mapped:** 2026-05-30
**Phase:** 19 - Multi-Script Signing Finish
**Scope:** Per-file analog map for new/modified files. All analogs are in-tree (same crate/file when possible); the load-bearing pattern for both new production sign bodies is the existing `p2wpkh::sign` body at `shared/src/bip322/p2wpkh.rs:46-72`, which shipped Phase 15 and is exercised by the v1.3 + v1.4 invariant gates.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `shared/src/bip322/mod.rs` (modify: delete `sign_simple_test_only`, add helper, add tests) | module / dispatcher | request-response | self (`bip322_message_hash`, `sign_simple`, existing `tests` block) | exact |
| `shared/src/bip322/p2tr.rs` (modify: replace `todo!()` body) | crypto-signer (per-script) | request-response | `p2tr::sign_for_tests` (same file, lines 60-95) PLUS `p2wpkh::sign` (sibling, lines 46-72) | exact lift |
| `shared/src/bip322/p2sh_p2wpkh.rs` (modify: replace `todo!()` body) | crypto-signer (per-script) | request-response | `p2sh_p2wpkh::sign_for_tests` (same file, lines 68-108) PLUS `p2wpkh::sign` (sibling) | exact lift |
| `shared/src/bip322/p2wpkh.rs` (modify: delete unused alias) | crypto-signer (per-script) | request-response | n/a (deletion-only) | n/a |
| `shared/tests/per_script_vectors.rs` (modify: 1 import + 2 callsites + 2 comment blocks) | integration test | request-response | `test_p2wpkh_sign_verify_roundtrip_via_dispatcher` (same file, lines 215-250) — already calls `sign_simple` | exact |
| `tests/integration/multi_script_validate.rs` (modify: 1 import + 1 helper-fn body) | integration test | request-response | `sign_witness` helper itself (same file, lines 113-121) | exact (self) |
| `tests/integration/mod.rs` (modify: 2 doc-comment refreshes) | test fixture module | n/a | n/a (comment-only) | n/a |
| `client/tests/wallet_sign_roundtrip.rs` (add: 2 parity tests) | integration test | request-response | `p2tr_descriptor_sign_roundtrip_verifies` + `p2sh_p2wpkh_descriptor_sign_roundtrip_verifies` (same file, lines 74-122) | exact (self) |

---

## Pattern Assignments

### File: `shared/src/bip322/mod.rs` (modified)

#### Add 1 — `pub fn p2sh_p2wpkh_final_script_sig` helper (Plan 19-01)

**Insert location:** Sibling to `sign_simple` — after the `sign_simple` body ends at line 272, before the `sign_simple_test_only` block that begins at line 274 (the entire block 274-314 is deleted in Plan 19-02 but EXISTS at Plan 19-01 commit time, so insert at line 273 directly after the closing `}` of `sign_simple`). The deletion in Plan 19-02 lifts the surrounding context; the new helper stays put.

**Analog (in-file, script-neutral helper):** `bip322_message_hash` at `mod.rs:45-53` — same module, `pub fn`, infallible, derives output (`[u8; 32]`) from input (`&[u8]`) deterministically via rust-bitcoin primitives.

```rust
// shared/src/bip322/mod.rs:45-53 — ANALOG
pub fn bip322_message_hash(message: &[u8]) -> [u8; 32] {
    let tag = b"BIP0322-signed-message";
    let tag_hash = sha256::Hash::hash(tag);
    let mut engine = sha256::HashEngine::default();
    engine.input(tag_hash.as_ref());
    engine.input(tag_hash.as_ref());
    engine.input(message);
    sha256::Hash::from_engine(engine).to_byte_array()
}
```

**Analog (cross-file, script-pubkey builder convention):** `build_bip322_to_spend` at `mod.rs:60-64` shows the codebase's `bitcoin::blockdata::script::Builder::new()` convention (NOT `ScriptBuf::builder()`). Phase 19 follows the same convention per CD-34 + RESEARCH Q3.

```rust
// shared/src/bip322/mod.rs:61-64 — script Builder convention
let script_sig = bitcoin::blockdata::script::Builder::new()
    .push_opcode(bitcoin::opcodes::OP_0)
    .push_slice(msg_hash)             // msg_hash: &[u8; 32] -> AsRef<PushBytes> via from_array! macro
    .into_script();
```

**Body (lift verbatim from RESEARCH §Q3):**

```rust
pub fn p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf {
    let compressed = bitcoin::PublicKey::new(*pubkey);
    let wpkh = compressed
        .wpubkey_hash()
        .expect("compressed pubkey always has wpubkey_hash");
    let redeem = ScriptBuf::new_p2wpkh(&wpkh);
    bitcoin::blockdata::script::Builder::new()
        .push_slice(
            <&bitcoin::script::PushBytes>::try_from(redeem.as_bytes())
                .expect("22-byte redeem fits push limit (520 bytes)"),
        )
        .into_script()
}
```

**Adaptation notes vs `bip322_message_hash` analog:**
- Same shape: `pub fn`, infallible, derives output deterministically from input.
- Input type: `&bitcoin::secp256k1::PublicKey` (lowest-privilege input, no secret material — see D-109).
- Output type: `ScriptBuf` (vs `[u8; 32]`).
- Body uses the existing `bitcoin::blockdata::script::Builder::new()` convention (NOT `ScriptBuf::builder()` per RESEARCH Q3 codebase convention check).
- The `<&PushBytes>::try_from(...).expect(...)` path is mandatory because `redeem.as_bytes()` returns `&[u8]` (not a fixed-size array), and only `[u8; N]` implements `AsRef<PushBytes>` directly. The 22-byte redeem trivially fits the 520-byte push limit.

#### Add 2 — `#[test] fn p2sh_p2wpkh_final_script_sig_derives_correctly` (Plan 19-01, D-108)

**Insert location:** Inside the `#[cfg(test)] mod tests` block in `mod.rs`, after the existing PII-safety test at line 567. The block ends at line 567 (`}`); insert before the closing `}` of `mod tests`.

**Analog (in-file fixture-using unit test):** Existing tests that use `fixture_secret_key()` (`mod.rs:441-443`) — e.g., `detect_script_type_returns_p2wpkh_for_p2wpkh_spk` at `mod.rs:476-480`.

```rust
// shared/src/bip322/mod.rs:441-443 — FIXTURE (reuse verbatim)
fn fixture_secret_key() -> SecpSecretKey {
    SecpSecretKey::from_slice(&[0x42_u8; 32]).unwrap()
}

// shared/src/bip322/mod.rs:476-480 — ANALOG SHAPE
#[test]
fn detect_script_type_returns_p2wpkh_for_p2wpkh_spk() {
    let spk = fixture_p2wpkh_spk();
    assert_eq!(detect_script_type(&spk).unwrap(), ScriptType::P2wpkh);
}
```

**Body (per RESEARCH §Q3, with the byte-count correction):**

```rust
#[test]
fn p2sh_p2wpkh_final_script_sig_derives_correctly() {
    use bitcoin::secp256k1::{PublicKey, Secp256k1};

    let secp = Secp256k1::new();
    let sk = fixture_secret_key();
    let pk = PublicKey::from_secret_key(&secp, &sk);

    let script_sig = p2sh_p2wpkh_final_script_sig(&pk);
    let bytes = script_sig.as_bytes();

    // BIP-141: scriptSig = OP_PUSHBYTES_22 || redeem
    //         redeem  = OP_0 || OP_PUSHBYTES_20 || HASH160(pubkey)
    // Total = 1 (push opcode) + 22 (redeem) = 23 bytes.
    // [Rule 1 — Bug] CONTEXT D-110 says 24; the correct count is 23 (see RESEARCH §Q3).
    assert_eq!(bytes.len(), 23, "scriptSig must be 23 bytes (1-byte push opcode + 22-byte redeem)");
    assert_eq!(bytes[0], 0x16, "first byte must be OP_PUSHBYTES_22");
    assert_eq!(bytes[1], 0x00, "redeem byte 0 must be OP_0");
    assert_eq!(bytes[2], 0x14, "redeem byte 1 must be OP_PUSHBYTES_20");

    let compressed = bitcoin::PublicKey::new(pk);
    let expected_wpkh = compressed.wpubkey_hash().expect("compressed");
    assert_eq!(&bytes[3..23], expected_wpkh.as_ref(), "trailing 20 bytes = HASH160(pubkey)");
}
```

**Adaptation notes vs analog:** Same fixture pattern (`fixture_secret_key()` reused verbatim); same `#[test] fn` shape inside `mod tests`. Assertion swaps `detect_script_type` for the new helper and asserts byte shape directly per BIP-141. The byte-count constant (23) is the RESEARCH-corrected value — D-110's "24 bytes total" is off-by-one.

#### Add 3 — D-111 cross-check rejection unit tests (Plan 19-01, CD-37 default = yes)

**Insert location:** Same `mod tests` block, after the new `p2sh_p2wpkh_final_script_sig_derives_correctly` test.

**Analog (in-file `Bip322Error::ScriptTypeMismatch` exerciser):** The existing PII-safety test at `mod.rs:512-565` constructs a `ScriptTypeMismatch { declared, derived }` value directly:

```rust
// shared/src/bip322/mod.rs:523-529 — ANALOG for ScriptTypeMismatch construction
format!(
    "{}",
    Bip322Error::ScriptTypeMismatch {
        declared: ScriptType::P2wpkh,
        derived: ScriptType::P2tr,
    }
),
```

**Analog (in-file rejection-style test):** `detect_script_type_rejects_op_return_with_unsupported_script_type` at `mod.rs:495-499`:

```rust
// shared/src/bip322/mod.rs:495-499 — REJECTION SHAPE
#[test]
fn detect_script_type_rejects_op_return_with_unsupported_script_type() {
    let spk = ScriptBuf::new_op_return([0x01, 0x02, 0x03]);
    let err = detect_script_type(&spk).expect_err("OP_RETURN must reject");
    assert!(matches!(err, Bip322Error::UnsupportedScriptType));
}
```

**Body sketch (one test per per-script sign body; planner names per CD-35):**

```rust
#[test]
fn p2tr_sign_rejects_mismatched_p2wpkh_spk() {
    // P2WPKH spk + P2TR-keyed secret → derived = P2tr, declared = P2wpkh.
    let spk = fixture_p2wpkh_spk();
    let key = fixture_secret_key();
    let err = sign_simple(ScriptType::P2tr, &spk, &key, b"x")
        .expect_err("P2TR sign with P2WPKH spk must reject");
    assert!(matches!(
        err,
        Bip322Error::ScriptTypeMismatch {
            declared: ScriptType::P2wpkh,
            derived: ScriptType::P2tr,
        }
    ));
}

#[test]
fn p2sh_p2wpkh_sign_rejects_mismatched_p2tr_spk() {
    let spk = fixture_p2tr_spk();
    let key = fixture_secret_key();
    let err = sign_simple(ScriptType::P2shP2wpkh, &spk, &key, b"x")
        .expect_err("P2SH-P2WPKH sign with P2TR spk must reject");
    assert!(matches!(
        err,
        Bip322Error::ScriptTypeMismatch {
            declared: ScriptType::P2tr,
            derived: ScriptType::P2shP2wpkh,
        }
    ));
}
```

**Adaptation notes:** Same `expect_err` + `matches!(err, ...)` shape as the existing OP_RETURN rejection test. The fixture functions (`fixture_p2wpkh_spk`, `fixture_p2tr_spk`, `fixture_secret_key`) are already defined at `mod.rs:441-474` — reused verbatim. The `declared` field semantics align with D-113's "derived from spk via `detect_script_type`" rule.

#### Deletion — `sign_simple_test_only` (Plan 19-02)

**Lines:** `mod.rs:274-314` — entire block, including the 27-line explanatory comment header (274-300) and the `#[doc(hidden)] pub fn` itself (302-314). No leftover symbol; the public dispatcher (`sign_simple` at 261-272) is untouched.

#### Modification — `sign_simple` doc-comment refresh (Plan 19-01, optional via CD-38)

**Location:** `mod.rs:258-260` — the comment under `sign_simple` references `todo!()` for P2TR + P2SH-P2WPKH. Plan 19-01 updates this to "all three scripts ship production bodies in Phase 19 per CONTEXT D-116; the per-script `sign` bodies cross-check spk↔key per D-111."

---

### File: `shared/src/bip322/p2tr.rs` (modified)

#### Modification 1 — Production `sign` body lift (Plan 19-01)

**Lines to replace:** `p2tr.rs:38-44` — the entire `pub(crate) fn sign(...)` including the `todo!()` body and the 3-line doc comment above it.

**Analog (in-file, EXACT lift target):** `sign_for_tests` at `p2tr.rs:60-95` — the full 8-step BIP-341 keypath sign sequence (build to_spend → build to_sign → Keypair::from_secret_key → tap_tweak → taproot_key_spend_signature_hash → sign_schnorr_no_aux_rand → push 64 bytes into Witness).

```rust
// shared/src/bip322/p2tr.rs:60-95 — LIFT TARGET (production body steps)
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
    w.push(sig.as_ref());
    w
}
```

**Analog (cross-file, production sign body shape):** `p2wpkh::sign` at `p2wpkh.rs:46-72` is the reference for "what a SHIPPED production sign body looks like." Same crate, same module, parallel script type — already audited and exercised by `full_round` 8/8 + `mixed_script_e2e` 1/1.

```rust
// shared/src/bip322/p2wpkh.rs:46-72 — REFERENCE PROD SHAPE (sibling)
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
    w.push(pubkey.serialize());
    Ok(w)
}
```

**Adaptation notes (lift + transform):**

1. **Signature change:** `pub(crate) fn sign_for_tests(spk: &Script, key: &SecretKey, message: &[u8]) -> Witness` → `pub(crate) fn sign(spk: &Script, key: &SecretKey, message: &[u8]) -> Result<Witness, super::Bip322Error>`. Note: the existing `pub(crate) fn sign` at lines 38-44 ALREADY has the correct signature — Plan 19-01 just replaces its `todo!()` body. The `_spk`/`_key`/`_message` underscores at lines 39-41 are dropped (params become load-bearing).
2. **Add D-111 cross-check at the TOP** of the new body (before the existing 8-step sequence). Per D-113 algorithm: derive `keypair = Keypair::from_secret_key(secp, key)` → `tweaked = keypair.tap_tweak(&secp, None)` → `expected_spk = ScriptBuf::new_p2tr_tweaked(tweaked.to_keypair().x_only_public_key().0.dangerous_assume_tweaked())` → compare to `spk` byte-equal → on mismatch return `Err(Bip322Error::ScriptTypeMismatch { declared: detect_script_type(spk)?, derived: ScriptType::P2tr })`.
3. **Wrap the witness return** at line 92-94 in `Ok(...)`. The `.expect("sighash on well-formed to_sign")` at line 85 stays as-is (infallible after the cross-check confirms `spk` is a valid P2TR SPK — `taproot_key_spend_signature_hash` cannot fail on a well-formed `to_sign` Transaction whose Prevouts match the input arity).
4. **Delete the `sign_for_tests` fn** at lines 46-95 (Plan 19-02 task, not Plan 19-01 — at the Plan 19-01 boundary `sign_for_tests` is still load-bearing for the test-only mirror in `mod.rs::sign_simple_test_only`).
5. **Per CD-38 default:** Add an inline `// Plan 19-01 Task N — BIP322-05 production body, lifted from prior sign_for_tests + D-111 cross-check at top` comment at the top of the new body.

#### Modification 2 — `sign_for_tests` deletion (Plan 19-02)

**Lines:** `p2tr.rs:46-95` — entire fn including the 14-line doc comment.

---

### File: `shared/src/bip322/p2sh_p2wpkh.rs` (modified)

#### Modification 1 — Production `sign` body lift (Plan 19-01)

**Lines to replace:** `p2sh_p2wpkh.rs:39-50` — the entire `pub(crate) fn sign(...)` including the `todo!()` body.

**Analog (in-file, EXACT lift target):** `sign_for_tests` at `p2sh_p2wpkh.rs:68-108` — full BIP-143 sequence (derive unwrapped P2WPKH from compressed pubkey → sighash via `p2wpkh_signature_hash(0, &unwrapped_p2wpkh, ...)` → DER-encode + push SIGHASH_ALL byte → push pubkey).

```rust
// shared/src/bip322/p2sh_p2wpkh.rs:68-108 — LIFT TARGET (production body steps)
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
    // [D-117] The to_spend output's script_pubkey is the OUTER P2SH SPK; the
    // existing sign_for_tests REBUILDS it from the key (footgun: ignores _spk).
    // Production body USES the passed `spk` directly here (after the D-111
    // cross-check confirms it byte-equals the derived value).
    let p2sh_spk = ScriptBuf::new_p2sh(&unwrapped_p2wpkh.script_hash());
    let to_spend = super::build_bip322_to_spend(&p2sh_spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .p2wpkh_signature_hash(
            0,
            &unwrapped_p2wpkh,        // BIP-143 sighash is over the UNWRAPPED redeem, NOT the P2SH SPK
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
```

**Adaptation notes (lift + transform):**

1. **Signature change:** Existing `pub(crate) fn sign` signature at lines 44-48 is already correct (`Result<Witness, super::Bip322Error>`). The `_spk`/`_key`/`_message` underscore prefixes at lines 45-47 ARE dropped — per D-117 the `_spk` becomes `spk` (load-bearing after cross-check).
2. **Add D-111 cross-check at the TOP** of the new body. Per D-113 P2SH-P2WPKH algorithm: derive `compressed = PublicKey::new(key.public_key(secp))` → `redeem = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().expect("compressed"))` → `expected_spk = ScriptBuf::new_p2sh(&redeem.script_hash())` → compare to `spk` byte-equal → on mismatch return `Err(Bip322Error::ScriptTypeMismatch { declared: detect_script_type(spk)?, derived: ScriptType::P2shP2wpkh })`.
3. **D-117 spk-used-directly:** Replace lines 85-86 (`let p2sh_spk = ScriptBuf::new_p2sh(&unwrapped_p2wpkh.script_hash()); let to_spend = super::build_bip322_to_spend(&p2sh_spk, &msg_hash);`) with `let to_spend = super::build_bip322_to_spend(spk, &msg_hash);`. The `spk` param is now load-bearing (the rebuild-from-key footgun is removed); the cross-check above proves byte-equality.
4. **Wrap the witness return** at lines 104-107 in `Ok(...)`.
5. **Sighash still uses `unwrapped_p2wpkh`** (line 93) — this is STRUCTURAL per BIP-143; the sighash for a P2SH-P2WPKH spend is computed against the unwrapped redeem, NOT the outer P2SH SPK. Do NOT replace this with `spk`.
6. **Delete the `sign_for_tests` fn** at lines 52-108 (Plan 19-02 task).
7. **Per CD-38 default:** Add `// Plan 19-01 Task N — BIP322-06 production body, lifted from prior sign_for_tests + D-111 cross-check + D-117 spk-used-directly` comment at the top of the new body.

#### Modification 2 — `sign_for_tests` deletion (Plan 19-02)

**Lines:** `p2sh_p2wpkh.rs:52-108` — entire fn including the 16-line doc comment.

---

### File: `shared/src/bip322/p2wpkh.rs` (modified)

#### Modification — `sign_for_tests` deletion (Plan 19-02 ONLY)

**Lines:** `p2wpkh.rs:74-95` — the entire fn including the 14-line doc comment and the `#[allow(dead_code)]` attribute. The production `sign` body at lines 22-72 is UNCHANGED.

**No analog needed** — this is a pure deletion of an unused alias (verified by RESEARCH §Q5 C3: `#[allow(dead_code)]` confirms the unused state; deletion silences nothing because `dead_code` is already silenced).

**No D-111 cross-check added for P2WPKH** — per CONTEXT `<code_context>` line 213, P2WPKH `sign` uses `spk` for sighash directly via `p2wpkh_signature_hash(0, spk, ...)`. A mismatched `spk` silently produces a wrong-sighash signature that fails verify downstream — functionally equivalent to the cross-check rejecting it earlier. The asymmetry is deferred to v1.6+ (CONTEXT `<deferred>` line 257).

---

### File: `shared/tests/per_script_vectors.rs` (modified — Plan 19-02 ONLY)

#### Modification 1 — Import refresh (line 21)

```diff
-use shared::bip322::{sign_simple, sign_simple_test_only, verify_simple, Bip322Error, ScriptType};
+use shared::bip322::{sign_simple, verify_simple, Bip322Error, ScriptType};
```

**Analog (in-file, existing `sign_simple` import shape):** Line 21 already imports `sign_simple` — the diff just drops the now-deleted `sign_simple_test_only`. `Bip322Error` stays (used by the dead-code compile check at line 366).

#### Modification 2 — P2TR callsite migration (line 274)

**Analog (in-file, existing `sign_simple` callsite):** `test_p2wpkh_sign_verify_roundtrip_via_dispatcher` at `per_script_vectors.rs:215-250` already uses `sign_simple` (NOT the test-only mirror) — the migration target is structurally identical to this existing test.

```rust
// shared/tests/per_script_vectors.rs:228-229 — ANALOG (existing sign_simple callsite, .expect pattern)
// P2WPKH production sign_simple is fully implemented per CD-6.
let witness = sign_simple(ScriptType::P2wpkh, &spk, &key, message).expect("sign_simple p2wpkh");
```

**Diff (per RESEARCH §Q5 Site 2):**

```diff
-    // P2TR production sign_simple is todo!() per CD-6; test path uses
-    // sign_simple_test_only which routes to p2tr::sign_for_tests (the
-    // 8-step BIP-341 sequence from Sprint-0-B).
-    let witness = sign_simple_test_only(ScriptType::P2tr, &spk, &key, message)
-        .expect("sign_simple_test_only p2tr");
+    // P2TR production sign_simple ships in Phase 19 Plan 19-01 (D-116
+    // lifted sign_for_tests body verbatim into production sign + D-111
+    // cross-check at top).
+    let witness = sign_simple(ScriptType::P2tr, &spk, &key, message)
+        .expect("sign_simple p2tr");
```

#### Modification 3 — P2SH-P2WPKH callsite migration (line 311)

**Diff (per RESEARCH §Q5 Site 3) — same pattern as Modification 2.**

```diff
-    // P2SH-P2WPKH production sign_simple is todo!() per CD-6; test path uses
-    // sign_simple_test_only which routes to p2sh_p2wpkh::sign_for_tests
-    // (sighash over the unwrapped P2WPKH redeem).
-    let witness = sign_simple_test_only(ScriptType::P2shP2wpkh, &spk, &key, message)
-        .expect("sign_simple_test_only p2sh-p2wpkh");
+    // P2SH-P2WPKH production sign_simple ships in Phase 19 Plan 19-01 (D-116
+    // lifted sign_for_tests body verbatim into production sign + D-111
+    // cross-check + D-117 spk-used-directly).
+    let witness = sign_simple(ScriptType::P2shP2wpkh, &spk, &key, message)
+        .expect("sign_simple p2sh-p2wpkh");
```

**Assertion-shape safety (per RESEARCH §Q5):** Both migrated tests assert via `verify_simple(...).is_ok()` + structural `witness.len() == N` + `sig_bytes.len()` checks. Neither asserts hardcoded witness bytes — production sign witnesses interoperate transparently because D-116 lifts the body verbatim.

---

### File: `tests/integration/multi_script_validate.rs` (modified — Plan 19-02 ONLY)

#### Modification 1 — Import refresh (line 23)

```diff
-use shared::bip322::{sign_simple_test_only, Bip322Error, ScriptType};
+use shared::bip322::{sign_simple, Bip322Error, ScriptType};
```

#### Modification 2 — `sign_witness` helper-fn body (lines 113-121)

**Analog (in-file, self):** The helper `sign_witness` IS the analog — only the inner call swaps. The fn signature and surrounding shape stays identical.

```rust
// tests/integration/multi_script_validate.rs:113-121 — CURRENT (Plan 19-02 input)
fn sign_witness(handle: &TypedUtxoHandle, message: &[u8]) -> Witness {
    sign_simple_test_only(
        handle.script_type,
        handle.script_pubkey.as_script(),
        &handle.secret_key,
        message,
    )
    .expect("sign_simple_test_only should produce a valid witness")
}
```

**Diff (per RESEARCH §Q5 Site 5):**

```diff
 fn sign_witness(handle: &TypedUtxoHandle, message: &[u8]) -> Witness {
-    sign_simple_test_only(
+    sign_simple(
         handle.script_type,
         handle.script_pubkey.as_script(),
         &handle.secret_key,
         message,
     )
-    .expect("sign_simple_test_only should produce a valid witness")
+    .expect("sign_simple should produce a valid witness")
 }
```

**Assertion-shape safety:** All 9 D-54 test cases that consume `sign_witness` assert via `validate_ownership_proof_typed(...).is_ok()` / `matches!(...)` — no witness-byte assertions. Production sign witnesses are interoperable per the same reasoning as `per_script_vectors.rs`.

---

### File: `tests/integration/mod.rs` (modified — Plan 19-02 ONLY)

#### Modification — Doc-comment refresh at lines 707 + 723

**Line 707** (inside a module-level explanatory comment block):

```diff
-//      shared::bip322::sign_simple_test_only.
+//      shared::bip322::sign_simple.
```

**Line 721-723** (inside `TypedUtxoHandle::secret_key` field doc):

```diff
-    /// Secret key matching the address — used by the integration tests to
-    /// construct valid BIP-322 v=2 witnesses via
-    /// shared::bip322::sign_simple_test_only.
+    /// Secret key matching the address — used by the integration tests to
+    /// construct valid BIP-322 v=2 witnesses via shared::bip322::sign_simple.
```

**No analog needed** — pure doc-comment refresh; no runtime / compile effect.

---

### File: `client/tests/wallet_sign_roundtrip.rs` (modified — Plan 19-01 ADDS 2 tests)

#### Add 1 — `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` (D-118)

**Insert location:** After the last existing `#[tokio::test]` fn (after `signed_proof_script_type_matches_wallet_script_type` ending at line 179), before the `dummy_outpoint_is_well_formed` non-tokio test at line 184. Insertion preserves the file's logical ordering: tokio descriptor-wallet tests first, defensive sanity test last.

**Analog (in-file, existing descriptor-wallet test):** `p2tr_descriptor_sign_roundtrip_verifies` at `wallet_sign_roundtrip.rs:74-95` — the SAME wallet construction pattern (`BdkClientWallet::generate(...)`), the SAME `wallet.sign_bip322(TEST_MESSAGE)` call, the SAME `verify_simple(...)` roundtrip. The new parity test EXTENDS this shape by adding a second sign call via `shared::bip322::sign_simple(...)` + a byte-equality assertion.

```rust
// client/tests/wallet_sign_roundtrip.rs:74-95 — ANALOG (existing P2TR descriptor test)
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
```

**Existing reusable constants in the file:**

- `TEST_WIF` at line 40 (canonical Bitcoin Core regtest "Hello World" WIF — reused as the single-key seed for the WIF-descriptor parity test).
- `DUMMY_OUTPOINT` at lines 32-33 (placeholder; BIP-322 signs SPK, not outpoint).
- `TEST_MESSAGE` at line 35.
- `NET = Network::Signet` at line 46.

**Body (lift from RESEARCH §Q2 recommended test-shape, adapted to the file's existing style):**

```rust
// ---------------------------------------------------------------------------
// Phase 19 Plan 19-01 — BIP322-05 SC#1 byte-equality parity tests (D-118 + D-119).
//
// Asserts shared::bip322::sign_simple produces the SAME witness bytes as
// BdkClientWallet::sign_bip322 for the same (key, message). Safe per
// RESEARCH §Q1: bdk_wallet 2.3.0 uses sign_schnorr_no_aux_rand (deterministic)
// for P2TR; P2SH-P2WPKH uses sign_ecdsa (RFC 6979 deterministic).
// ---------------------------------------------------------------------------

const PARITY_TEST_MESSAGE: &str = "blindjoin:19-01:parity:byte-for-byte";

/// Recover the secp256k1 SecretKey from the file's TEST_WIF constant.
fn parity_secret_key() -> bitcoin::secp256k1::SecretKey {
    bitcoin::PrivateKey::from_wif(TEST_WIF)
        .expect("test WIF is valid")
        .inner
}

#[tokio::test]
async fn p2tr_shared_sign_matches_bdk_sign_byte_for_byte() {
    // Single-key WIF descriptor — bdk_wallet 2.3 accepts `tr(<WIF>)` directly
    // (RESEARCH §Q2). Both signing paths see the SAME SecretKey, so byte-equality
    // holds per RESEARCH §Q1 (sign_schnorr_no_aux_rand on both sides).
    let descriptor = format!("tr({TEST_WIF})");
    let sk = parity_secret_key();

    // Derive the on-chain P2TR address from the same key (Note A in RESEARCH §Q2).
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let keypair = bitcoin::secp256k1::Keypair::from_secret_key(&secp, &sk);
    let tweaked = bitcoin::key::TapTweak::tap_tweak(keypair, &secp, None);
    let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
    let utxo_address = bitcoin::Address::p2tr_tweaked(
        tweaked_xonly.dangerous_assume_tweaked(),
        Network::Regtest,
    );
    let _ = pk;

    let wallet = BdkClientWallet::from_descriptor(
        &descriptor,
        DUMMY_OUTPOINT,
        &utxo_address.to_string(),
        Network::Regtest,
        ScriptType::P2tr,
    ).expect("P2TR single-key WIF descriptor should construct");

    let spk = wallet.script_pubkey();
    let bdk_signed = wallet
        .sign_bip322(PARITY_TEST_MESSAGE)
        .expect("bdk sign_bip322 should succeed");

    let shared_witness = shared::bip322::sign_simple(
        ScriptType::P2tr,
        &spk,
        &sk,
        PARITY_TEST_MESSAGE.as_bytes(),
    )
    .expect("shared::bip322::sign_simple P2TR should succeed");

    // SC#1: byte-equality. Safe per RESEARCH §Q1.
    assert_eq!(
        bdk_signed.witness, shared_witness,
        "P2TR bdk vs shared::bip322 witnesses must be byte-equal (D-118)"
    );
}
```

#### Add 2 — `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` (D-119)

**Analog (in-file):** `p2sh_p2wpkh_descriptor_sign_roundtrip_verifies` at `wallet_sign_roundtrip.rs:98-122` — same shape as the P2TR analog, with the `final_script_sig.is_some()` Pitfall 7 assertion appended.

**Body (same pattern as Add 1, with P2SH-P2WPKH derivation per RESEARCH §Q2 Note A):**

```rust
#[tokio::test]
async fn p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte() {
    let descriptor = format!("sh(wpkh({TEST_WIF}))");
    let sk = parity_secret_key();

    // Derive the on-chain P2SH-P2WPKH address from the same key.
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
    let compressed = bitcoin::PublicKey::new(pk);
    let wpkh = compressed.wpubkey_hash().expect("compressed key");
    let redeem = bitcoin::ScriptBuf::new_p2wpkh(&wpkh);
    let utxo_address = bitcoin::Address::p2sh(&redeem, Network::Regtest)
        .expect("p2sh derivation");

    let wallet = BdkClientWallet::from_descriptor(
        &descriptor,
        DUMMY_OUTPOINT,
        &utxo_address.to_string(),
        Network::Regtest,
        ScriptType::P2shP2wpkh,
    ).expect("P2SH-P2WPKH single-key WIF descriptor should construct");

    let spk = wallet.script_pubkey();
    let bdk_signed = wallet
        .sign_bip322(PARITY_TEST_MESSAGE)
        .expect("bdk sign_bip322 should succeed");

    let shared_witness = shared::bip322::sign_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &sk,
        PARITY_TEST_MESSAGE.as_bytes(),
    )
    .expect("shared::bip322::sign_simple P2SH-P2WPKH should succeed");

    // ECDSA uses RFC 6979 deterministic nonce — byte-equality always holds.
    assert_eq!(
        bdk_signed.witness, shared_witness,
        "P2SH-P2WPKH bdk vs shared::bip322 witnesses must be byte-equal (D-119)"
    );
}
```

**Adaptation notes vs analog descriptor-roundtrip tests:**

- Uses `from_descriptor` (single-key WIF) instead of `generate` (random-mnemonic) — RESEARCH §Q2 Option A.
- Network swaps `NET` (Signet) → `Network::Regtest` because the WIF constant is the regtest "Hello World" WIF; bdk's WIF parsing requires the network to match.
- Adds a second sign call to `shared::bip322::sign_simple(...)` with the same key recovered from `TEST_WIF`.
- Asserts byte-equality on `bdk_signed.witness` vs `shared_witness` (Phase 19's SC#1 closure).
- Drops the `verify_simple(...)` + `witness_stack == witness.iter().collect()` + `final_script_sig.is_none/some()` assertions present in the analog roundtrip tests — those are covered by the existing 5 tests; parity adds the NEW assertion only.
- Test count: 5 existing + 2 new parity + 1 sanity = 8 total. CONTEXT D-120's "4 existing + 2 new = 6" is OFF — recount of existing fns: `p2wpkh_descriptor_sign_roundtrip_verifies` (49), `p2tr_descriptor_sign_roundtrip_verifies` (74), `p2sh_p2wpkh_descriptor_sign_roundtrip_verifies` (98), `p2wpkh_wif_sign_roundtrip_verifies` (124), `signed_proof_witness_stack_matches_witness_iter` (152), `signed_proof_script_type_matches_wallet_script_type` (163), plus `dummy_outpoint_is_well_formed` (184) = 7 existing. New total = 7 + 2 = 9. [Minor pin for planner — D-120's count is off by 3; the gate assertion is the +2 delta, not the absolute count.]

---

## Shared Patterns

### Pattern 1 — Production sign body shape (cross-cutting analog for Plan 19-01)

**Source:** `shared/src/bip322/p2wpkh.rs:46-72` (Phase 15-shipped, exercised by `full_round` 8/8 + `mixed_script_e2e` 1/1).

**Apply to:** Both `p2tr::sign` and `p2sh_p2wpkh::sign` new production bodies. The structural template is:

```
secp = Secp256k1::new()
pubkey = PublicKey::from_secret_key(&secp, key)
[D-111 spk↔key cross-check inserted HERE]                  ← Phase 19 NEW
msg_hash = bip322_message_hash(message)
to_spend = build_bip322_to_spend(spk, &msg_hash)
to_sign = build_bip322_to_sign(&to_spend)
cache = SighashCache::new(&to_sign)
sighash = cache.<per-script-sighash-call>(...)
sig = secp.<per-script-sign-call>(message, key|keypair)
witness = build per-script witness shape
Ok(witness)
```

### Pattern 2 — `pub(crate)` per-script + `pub fn` dispatcher (Phase 15 D-27)

**Source:** `shared/src/bip322/mod.rs:261-272` (dispatcher) + per-script `pub(crate) fn` signatures.

**Apply to:** Phase 19 production sign bodies stay `pub(crate)`; only `sign_simple` is `pub`. The new `p2sh_p2wpkh_final_script_sig` helper is `pub` (it's a script-specific helper, not a sign variant) — this does NOT widen the dispatcher surface per D-109.

### Pattern 3 — Test fixture seed `[0x42_u8; 32]`

**Source:** `shared/src/bip322/mod.rs:441-443` (`fixture_secret_key`).

**Apply to:** All Plan 19-01 inline unit tests in `mod.rs::tests` (the new `p2sh_p2wpkh_final_script_sig_derives_correctly` + the 2 CD-37 cross-check rejection tests). Reused verbatim — no new fixture fns needed.

### Pattern 4 — PII-safe `Bip322Error::ScriptTypeMismatch { declared, derived }`

**Source:** `shared/src/bip322/mod.rs:184-188` (variant) + `mod.rs:512-565` (PII-safety test).

**Apply to:** Both new D-111 cross-check error returns in `p2tr::sign` and `p2sh_p2wpkh::sign`. Per D-112, the existing variant is REUSED (semantic stretch: documents dual meaning); no new variant added → no PII-safety test extension needed. The existing PII test already covers `ScriptTypeMismatch` with the `declared: P2wpkh, derived: P2tr` case at lines 523-529.

### Pattern 5 — `.expect(...)` (NOT `?`) at integration test callsites

**Source:** `shared/tests/per_script_vectors.rs:229` (existing `sign_simple` callsite for P2WPKH — already migrated by Phase 15).

```rust
let witness = sign_simple(ScriptType::P2wpkh, &spk, &key, message).expect("sign_simple p2wpkh");
```

**Apply to:** All 3 Plan 19-02 migrated callsites (`per_script_vectors.rs:274,311`, `multi_script_validate.rs:113-121`). None of the surrounding test fns return `Result`, so `?` is unavailable — `.expect("sign_simple <script>")` is the load-bearing convention per RESEARCH §Q5.

### Pattern 6 — Inline plan-comment provenance (CD-38 default)

**Source:** Existing convention from Phase 17 D-65 / D-66 inline comments throughout `client/src/wallet.rs` (e.g., line 116 "D-61: from_wif is P2WPKH-only — hardcode here so the cross-phase invariant (tests/integration/full_round.rs) stays bit-exact.").

**Apply to:** Top of each new production sign body — `// Plan 19-01 Task N — BIP322-<X> production body, lifted from prior sign_for_tests per D-116 + D-111 cross-check + [D-117 spk-used-directly for P2SH-P2WPKH]`. Per CD-38 default (more discoverable than per-decision tracking in 19-01-PLAN.md only).

---

## No Analog Found

No new file types in this phase — every change extends or mirrors a same-file pattern. No "first-of-kind" code.

---

## Metadata

**Analog search scope:**
- `shared/src/bip322/` (mod.rs, p2tr.rs, p2sh_p2wpkh.rs, p2wpkh.rs) — primary in-file analogs
- `shared/tests/per_script_vectors.rs` — `sign_simple` callsite pattern reference
- `tests/integration/multi_script_validate.rs` — `sign_witness` helper-self analog
- `client/tests/wallet_sign_roundtrip.rs` — descriptor-wallet test pattern reference
- `client/src/wallet.rs:115-195, 490-603` — `from_descriptor` + `sign_bip322` integration points

**Files Read for pattern extraction:** 7 source files (all in-tree, no cross-crate references except the documented bdk_wallet 2.3.0 source verified by RESEARCH).

**Pattern extraction date:** 2026-05-30

**Key correction surfaced during mapping:**
- CONTEXT D-110 byte-count "24 bytes total" is off-by-one — actual scriptSig length is 23 bytes (1-byte push opcode + 22-byte redeem). Plan 19-01 unit test asserts `bytes.len() == 23`. Per RESEARCH §Q3, this is a doc-comment-only correction — implementation already produces 23 bytes by construction.
- CONTEXT D-120 test count "4 existing + 2 new = 6" undercounts the existing fns in `wallet_sign_roundtrip.rs`; actual count is 7 existing + 2 new + 1 sanity = 10 total. The +2 delta gate is the load-bearing assertion.
