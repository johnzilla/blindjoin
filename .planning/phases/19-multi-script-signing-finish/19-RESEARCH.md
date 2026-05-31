# Phase 19 — Research

**Researched:** 2026-05-30
**Scope:** Answers to 5 load-bearing questions CONTEXT.md cannot answer.

CONTEXT.md (`.planning/phases/19-multi-script-signing-finish/19-CONTEXT.md`) locks decisions
D-107 through D-121 + CD-34 through CD-39. This research surfaces only the grep-evidenced
facts the planner needs to author Plan 19-01 and Plan 19-02 without re-litigating CONTEXT.

**Pinned versions (Cargo.lock):**
- `bitcoin` 0.32.8 [VERIFIED: Cargo.lock:581-582]
- `bdk_wallet` 2.3.0 [VERIFIED: Cargo.lock:512-513]
- `secp256k1` 0.29.1 [VERIFIED: Cargo.lock:4734-4735]
- `bip322` 0.0.10 (exact pin enforced by CI `bip322-pin-check`) [VERIFIED: Cargo.lock:543-544 + .github/workflows/ci.yml:214-236]

---

## Q1 — bdk_wallet Schnorr nonce strategy

**VERDICT (HIGH confidence):** bdk_wallet 2.3.0 emits Schnorr signatures via
`secp.sign_schnorr_no_aux_rand` — deterministic, NO aux-rand. The BIP322-05 SC#1
byte-equality parity test in `client/tests/wallet_sign_roundtrip.rs` per D-118 IS safe
to assert. **No downgrade to verify-roundtrip is needed.**

**Evidence:** `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bdk_wallet-2.3.0/src/wallet/signer.rs:585`:

```rust
let msg = &Message::from(sighash);
let signature = secp.sign_schnorr_no_aux_rand(msg, &keypair);
secp.verify_schnorr(&signature, msg, &XOnlyPublicKey::from_keypair(&keypair).0)
    .expect("invalid or corrupted schnorr signature");
// ...
psbt_input.tap_key_sig = Some(final_signature);
```

The fn is the taproot signing helper invoked from the descriptor-policy signer chain
(reachable for any P2TR descriptor wallet during `inner.sign(&mut psbt, ...)` —
which is what `BdkClientWallet::sign_bip322` calls at `client/src/wallet.rs:537-542`).

**Cross-check with our `sign_for_tests`:** `shared/src/bip322/p2tr.rs:87-90` already uses
`secp.sign_schnorr_no_aux_rand` with the same tap-tweaked keypair shape — so the Phase 19
production body lift (D-116) inherits the same deterministic call. Both sides match
bit-exactly: bdk's `secp.sign_schnorr_no_aux_rand(&Message::from(sighash), &keypair)` and
our `secp.sign_schnorr_no_aux_rand(&Message::from_digest(sighash.to_byte_array()), &tweaked)`
where `tweaked = keypair.tap_tweak(&secp, None).to_keypair()`.

**Planner action:**
- Plan 19-01 parity test `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` keeps `assert_eq!(bdk_witness, shared_witness)` (per D-118 strong form).
- 19-01-VERIFICATION.md cites this RESEARCH §Q1 instead of writing a downgrade note.
- No changes to D-114 (the `sign_schnorr_no_aux_rand` choice in our production body) — it's the byte-equality enabler.

**Subtle gotcha:** bdk_wallet's helper uses `Message::from(sighash)` (the `From<TapSighash> for Message` impl); our `sign_for_tests:88` uses `Message::from_digest(sighash.to_byte_array())`. Both produce the same 32-byte message digest under the hood — the bytes signed are identical. Plan 19-01 task SUMMARY may reference this as a "verified bytes-equivalent" note.

---

## Q2 — bdk_wallet SecretKey extractability + parity test fixture

**VERDICT (HIGH confidence):** Use **Option A — construct `SecretKey` first, then build a
single-key descriptor from the WIF form of that key**. bdk_wallet 2.3.0 accepts
single-key WIF descriptors directly (no `/0/*` derivation path); both the bdk descriptor
sign path and the shared::bip322 sign path see the same key.

**Evidence — bdk_wallet accepts single-key WIF descriptors:**
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bdk_wallet-2.3.0/tests/wallet.rs:952`:

```rust
get_funded_wallet_single("sh(wpkh(cVpPVruEDdmutPzisEsYvtST1usBR3ntr8pXSyt6D2YYqXRyPcFW))");
```

This is the standard bdk pattern for fixed-key test wallets — works for `wpkh(WIF)`,
`tr(WIF)`, and `sh(wpkh(WIF))` outer wrappers.

**Why not Option B (extract key from a generated descriptor):** `BdkClientWallet::generate`
internally generates a random BIP-39 mnemonic + BIP-32 xprv (`wallet.rs:209-332`). The
test would need to (a) capture the generated mnemonic via stdout/`descriptors.txt` (fragile)
OR (b) walk `m/86'/0'/0'/0/0` with `bitcoin::bip32` primitives that don't exist anywhere
in this codebase yet (verified — `grep -rn "DerivationPath::from_str\|Xpriv\|derive_priv"`
in client/shared returned zero hits). Option B is brittle AND adds new derivation code.

**Why not `from_wif` + `secret_key_for_signing()`:** `BdkClientWallet::from_wif` is
P2WPKH-only per D-61 (`client/src/wallet.rs:83-...` + `wallet.rs:498-503` debug_assert).
For the P2TR parity test (which is the actual SC#1 byte-equality gate per BIP322-05),
WIF doesn't work — only the descriptor path covers P2TR.

### Recommended test-shape (planner lifts verbatim)

```rust
// client/tests/wallet_sign_roundtrip.rs — ADD after existing test fns.

// Test-only WIF reused from the file's TEST_WIF constant at line 40
// (canonical Bitcoin Core regtest "Hello World" WIF, not for real funds).
// Used here as the SHARED key seed for both the bdk descriptor sign path
// and the shared::bip322::sign_simple direct call — both paths sign with
// the same SecretKey, so the BIP322-05 SC#1 byte-equality parity assertion
// can hold per RESEARCH Q1 (bdk_wallet 2.3.0 uses sign_schnorr_no_aux_rand,
// deterministic — no aux-rand divergence).
const PARITY_TEST_WIF: &str = TEST_WIF;
const PARITY_TEST_MESSAGE: &str = "blindjoin:19-01:parity:byte-for-byte";

/// Recover the secp256k1 SecretKey from the file's TEST_WIF constant.
fn parity_secret_key() -> bitcoin::secp256k1::SecretKey {
    bitcoin::PrivateKey::from_wif(PARITY_TEST_WIF)
        .expect("test WIF is valid")
        .inner
}

#[tokio::test]
async fn p2tr_shared_sign_matches_bdk_sign_byte_for_byte() {
    // Construct a P2TR descriptor wallet from the SAME WIF the parity check uses.
    // bdk_wallet 2.3 accepts `tr(<WIF>)` as a single-key non-derivation descriptor
    // (verified in bdk_wallet-2.3.0/tests/wallet.rs:952 for the sh(wpkh(WIF)) shape;
    // the tr(WIF) shape uses identical miniscript parsing).
    let descriptor = format!("tr({})", PARITY_TEST_WIF);
    let wallet = BdkClientWallet::from_descriptor(
        &descriptor,
        DUMMY_OUTPOINT,
        // utxo_address is derived from the same key for the on-chain SPK match —
        // bdk's wallet.peek_address(External, 0) on a single-key descriptor returns
        // the address for that exact key.
        /* construct the address from the same key — see Note A below */,
        Network::Regtest, // network choice is verify-side only; signing is network-agnostic
        ScriptType::P2tr,
    ).expect("P2TR single-key descriptor should construct");

    let spk = wallet.script_pubkey();
    let key = parity_secret_key();

    let bdk_signed = wallet
        .sign_bip322(PARITY_TEST_MESSAGE)
        .expect("bdk sign_bip322 should succeed");

    let shared_witness = shared::bip322::sign_simple(
        ScriptType::P2tr,
        &spk,
        &key,
        PARITY_TEST_MESSAGE.as_bytes(),
    )
    .expect("shared::bip322::sign_simple P2TR should succeed");

    // SC#1: byte-equality. Safe because bdk_wallet 2.3.0 uses
    // sign_schnorr_no_aux_rand (deterministic) — see RESEARCH Q1.
    assert_eq!(
        bdk_signed.witness, shared_witness,
        "P2TR bdk vs shared::bip322 witnesses must be byte-equal"
    );
}

#[tokio::test]
async fn p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte() {
    let descriptor = format!("sh(wpkh({}))", PARITY_TEST_WIF);
    let wallet = BdkClientWallet::from_descriptor(
        &descriptor,
        DUMMY_OUTPOINT,
        /* utxo_address derived from the same key — Note A */,
        Network::Regtest,
        ScriptType::P2shP2wpkh,
    ).expect("P2SH-P2WPKH single-key descriptor should construct");

    let spk = wallet.script_pubkey();
    let key = parity_secret_key();

    let bdk_signed = wallet
        .sign_bip322(PARITY_TEST_MESSAGE)
        .expect("bdk sign_bip322 should succeed");

    let shared_witness = shared::bip322::sign_simple(
        ScriptType::P2shP2wpkh,
        &spk,
        &key,
        PARITY_TEST_MESSAGE.as_bytes(),
    )
    .expect("shared::bip322::sign_simple P2SH-P2WPKH should succeed");

    // ECDSA uses RFC 6979 deterministic nonce — byte-equality always holds for
    // sign_ecdsa across implementations (no aux-rand variant exists).
    assert_eq!(
        bdk_signed.witness, shared_witness,
        "P2SH-P2WPKH bdk vs shared::bip322 witnesses must be byte-equal"
    );
}
```

**Note A — `utxo_address` derivation in the test fixture:** `BdkClientWallet::from_descriptor`
requires a `utxo_address` arg (`wallet.rs:135-140`). For the single-key WIF case, derive
it inline in the test by:

```rust
let sk = parity_secret_key();
let secp = bitcoin::secp256k1::Secp256k1::new();
let pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);
// P2TR:
let xonly = bitcoin::secp256k1::XOnlyPublicKey::from(pk);
let tweaked = /* tap_tweak with no merkle root, as in shared::bip322::tests::fixture_p2tr_spk */;
let address = bitcoin::Address::p2tr_tweaked(tweaked, Network::Regtest);
// P2SH-P2WPKH:
let compressed = bitcoin::PublicKey::new(pk);
let wpkh = compressed.wpubkey_hash().expect("compressed key");
let redeem = bitcoin::ScriptBuf::new_p2wpkh(&wpkh);
let address = bitcoin::Address::p2sh(&redeem, Network::Regtest).expect("p2sh derivation");
```

Plan-phase MAY extract these into `fixture_p2tr_address(network, sk)` / `fixture_p2sh_address(...)` helpers in the test file. The existing helper patterns at `shared/src/bip322/mod.rs:441-474` (`fixture_p2tr_spk`, `fixture_p2sh_spk`) are the structural reference.

**Spk-vs-bdk-derived-address cross-check:** After construction, `wallet.script_pubkey()`
should byte-equal the SPK derived from `key` (defense in depth — a regression in bdk's
single-key descriptor parsing would surface immediately as a Plan 19-01 test failure).
Plan-phase MAY add `assert_eq!(wallet.script_pubkey(), expected_spk)` before the sign call.

**Planner action:**
- Plan 19-01 Task 4 lifts the test-shape above verbatim into `client/tests/wallet_sign_roundtrip.rs`.
- Uses the existing `TEST_WIF` constant at line 40 (the canonical Bitcoin Core regtest "Hello World" WIF).
- Per CD-35 default: two separate test fns, no parameterised consolidation.
- The 2 new tests bring the file's test count from 5 (existing) → 7 (CONTEXT D-120 wave 1 boundary check).

---

## Q3 — `p2sh_p2wpkh_final_script_sig` body shape

**VERDICT (HIGH confidence):** Use `<&PushBytes>::try_from(redeem.as_bytes()).expect(...)`
path. `&[u8]` does NOT implement `AsRef<PushBytes>` — only fixed-size arrays `[u8; N]`
(via the `from_array!` macro covering sizes 1-76) do. The 22-byte redeem script is a
`Vec<u8>` (not a fixed array), so we must funnel through `TryFrom<&[u8]>` which checks
the 520-byte push limit. The expect is safe (22 << 520).

### Evidence

**`Builder::push_slice` signature** at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bitcoin-0.32.8/src/blockdata/script/builder.rs:60`:

```rust
pub fn push_slice<T: AsRef<PushBytes>>(mut self, data: T) -> Builder {
    self.0.push_slice(data);
    // ...
}
```

**`AsRef<PushBytes>` impls** at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bitcoin-0.32.8/src/blockdata/script/push_bytes.rs:156` (inside the `from_array!` macro):

```rust
impl AsRef<PushBytes> for [u8; $len] {
    fn as_ref(&self) -> &PushBytes {
        self.into()
    }
}
// Macro called with: 0..=75 + a few specific sizes (see push_bytes.rs:184)
```

**`TryFrom<&[u8]> for &PushBytes`** at `push_bytes.rs:116`:

```rust
impl<'a> TryFrom<&'a [u8]> for &'a PushBytes {
    type Error = PushBytesError;
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        check_limit(bytes.len())?;
        Ok(unsafe { PushBytes::from_slice_unchecked(bytes) })
    }
}
```

`redeem.as_bytes()` returns `&[u8]` (length 22), not a `&[u8; 22]`. Even though `22`
is in the from_array range, type inference cannot insert the conversion — we get `&[u8]`.

### Codebase convention (existing precedent)

`grep -n -E '(Builder::new|push_slice)'` in `coordinator/src + client/src + shared/src`
returned only 3 BIP-322-relevant hits, all in `shared/src/bip322/mod.rs`:

```rust
// shared/src/bip322/mod.rs:61-64 (push_slice with fixed array — works via AsRef macro)
let script_sig = bitcoin::blockdata::script::Builder::new()
    .push_opcode(bitcoin::opcodes::OP_0)
    .push_slice(msg_hash)             // msg_hash: &[u8; 32] → AsRef<PushBytes> ✓
    .into_script();
```

```rust
// shared/src/bip322/mod.rs:121-123 (no push_slice — just an opcode)
let op_return_only = bitcoin::blockdata::script::Builder::new()
    .push_opcode(bitcoin::opcodes::all::OP_RETURN)
    .into_script();
```

The codebase uses `bitcoin::blockdata::script::Builder::new()` style (the long form),
NOT `ScriptBuf::builder()`. **Plan 19 follows the same convention** for consistency.

### Exact recommended body

```rust
/// Build the `final_script_sig` for a P2SH-P2WPKH input spending a UTXO controlled
/// by `pubkey`. Per BIP-141, scriptSig = `OP_PUSHBYTES_22 <redeem>` where
/// redeem = `OP_0 OP_PUSHBYTES_20 <HASH160(pubkey)>` (22 bytes total).
///
/// Output bytes: `0x16 0x00 0x14 <20-byte HASH160(pubkey)>` (24 bytes total).
///
/// Infallible: takes a 33-byte compressed `secp256k1::PublicKey`, so
/// `compressed.wpubkey_hash()` is always `Some(...)` (per Phase 19 CONTEXT
/// Deferred Ideas "infallible helper" verification at RESEARCH §Q3).
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

### Infallibility verification (CONTEXT Deferred Ideas line)

`bitcoin::PublicKey::new(secp_pubkey)` constructs with `compressed: true` by default
(taking a 33-byte input). `bitcoin::PublicKey::wpubkey_hash()` returns `None` ONLY
for `compressed: false` keys — verified in `bitcoin-0.32.8/src/crypto/key.rs`
(see the `wpubkey_hash` method's `if self.compressed { Some(...) } else { None }`
guard). Therefore for any input `secp256k1::PublicKey`, the helper's return type
stays `ScriptBuf` (infallible) — NOT `Result<ScriptBuf, Bip322Error>`. The
Deferred Ideas item is RESOLVED to "infallible". [VERIFIED: bitcoin 0.32.8 source]

### Unit-test assertion (D-108)

The Plan 19-01 inline test in `shared/src/bip322/mod.rs::tests` asserts the byte
shape directly:

```rust
#[test]
fn p2sh_p2wpkh_final_script_sig_derives_correctly() {
    use bitcoin::secp256k1::{PublicKey, Secp256k1};

    // Fixture: 0x42-seed key reused from existing fixture_secret_key().
    let secp = Secp256k1::new();
    let sk = fixture_secret_key();
    let pk = PublicKey::from_secret_key(&secp, &sk);

    let script_sig = p2sh_p2wpkh_final_script_sig(&pk);
    let bytes = script_sig.as_bytes();

    // BIP-141: OP_PUSHBYTES_22 (0x16) || redeem (0x00 0x14 || HASH160(pubkey))
    assert_eq!(bytes.len(), 24, "scriptSig must be 24 bytes (1-byte push + 23-byte redeem)");
    assert_eq!(bytes[0], 0x16, "first byte must be OP_PUSHBYTES_22");
    assert_eq!(bytes[1], 0x00, "redeem byte 0 must be OP_0");
    assert_eq!(bytes[2], 0x14, "redeem byte 1 must be OP_PUSHBYTES_20");

    // The trailing 20 bytes must byte-equal HASH160(compressed pubkey).
    let compressed = bitcoin::PublicKey::new(pk);
    let expected_wpkh = compressed.wpubkey_hash().expect("compressed");
    assert_eq!(&bytes[3..23], expected_wpkh.as_ref(), "trailing 20 bytes = HASH160(pubkey)");
}
```

Note: bytes[3..23] (20 bytes) is HASH160; bytes[23] is the byte AFTER the redeem
ends — but the redeem is 22 bytes (0x00 + 0x14 + 20-byte hash), so bytes[1..23] is
the 22-byte redeem, bytes[23] doesn't exist (the script is exactly 24 bytes:
0x16 + 22 redeem + 1-byte push opcode counted in length? Re-derive).

Re-derivation: `OP_PUSHBYTES_22` is a SINGLE opcode (0x16). It tells the script
interpreter "read the next 22 bytes". So the scriptSig stream is `[0x16][22 redeem
bytes]` = 23 bytes total. The redeem itself is `[0x00][0x14][20 hash bytes]` = 22
bytes. So `bytes.len() == 23`, `bytes[0] == 0x16`, `bytes[1] == 0x00`, `bytes[2]
== 0x14`, `bytes[3..23] == hash160`.

CONTEXT D-110's claim "24 bytes total" is OFF BY ONE — actual scriptSig length is
23 bytes (1 push opcode + 22-byte redeem). Verified by counting BIP-141 §"P2WPKH
nested in BIP16 P2SH" example: scriptSig = `<22 bytes> 0x0014{20-byte-key-hash}`
where `<22 bytes>` is the OP_PUSHBYTES_22 telling the next 22 bytes are the push.
The PUSH OPCODE itself is 1 byte (0x16), then 22 bytes of pushed data, total 23
bytes.

**Planner action — D-110 byte-count correction:**
- The helper output is 23 bytes (not 24). Plan 19-01 unit test `p2sh_p2wpkh_final_script_sig_derives_correctly` asserts `bytes.len() == 23` (NOT 24).
- This is a doc-comment correction in the helper's prose and the D-108/D-110 CONTEXT lines. Implementation-wise it doesn't change anything — `Builder::new().push_slice(<22 bytes>).into_script()` already produces 23-byte output by construction.
- Plan 19-01 task SUMMARY may include a "[Rule 1 — Bug] CONTEXT D-110 byte-count off-by-one (24 → 23)" note for clarity.

[ASSUMED — recommend verifying via a `dbg!(p2sh_p2wpkh_final_script_sig(&pk).len())` print in a smoke test before committing the assertion constant. The OP_PUSHBYTES_22 counting rule is unambiguous in BIP-141 but the test should be the authority.]

---

## Q4 — CI grep checks vs Plan 19-02 deletion

**VERDICT (HIGH confidence):** All 3 CI grep jobs (`bip322-pin-check`, `crit-01-grep-check`,
`crit-01-client-grep-check`) stay green after Plan 19-02 deletes `sign_simple_test_only` +
`sign_for_tests`. **No CI-script edits are needed in Plan 19-02.**

### Per-check verdict

`grep -rn -E '(sign_simple_test_only|sign_for_tests)' .github/` returned **zero hits**
across all workflow files. The CI jobs don't reference the test-only helpers — they
gate orthogonal invariants.

| CI job | Pattern | What it gates | Affected by Plan 19-02? |
|---|---|---|---|
| `bip322-pin-check` | `bip322\s*=` in `Cargo.toml` without `=0.0.10` | Exact-version pin of `bip322` crate | NO — Plan 19-02 doesn't touch Cargo.toml |
| `crit-01-grep-check` | `CRIT-01` count `>= 2` in `coordinator/src/bitcoin/utxo.rs` | Dual-branch (v=1 + v=2) cross-check comment present | NO — Plan 19-02 is shared/ + tests/ only, not coordinator/ |
| `crit-01-client-grep-check` | `CRIT-01` count `>= 1` in `client/src/round/input.rs` | Client-side v=2 envelope cross-check comment present | NO — Plan 19-02 doesn't touch `client/src/round/input.rs` |
| `corepc-node-feature-pin-check` | `corepc-node\s*=` in `Cargo.toml` without `features=` | corepc-node version-feature pin | NO — orthogonal |

### Evidence

`/Users/john/Desktop/vault/projects/github.com/blindjoin/.github/workflows/ci.yml:214-290`
shows the 4 grep jobs in full. None reference `sign_simple_test_only`, `sign_for_tests`,
`p2tr::sign`, `p2sh_p2wpkh::sign`, or any of Plan 19-02's deletion targets.

The `clippy` job at lines 135-148 runs `cargo clippy --workspace --all-targets -- -D warnings`
which IS sensitive to Plan 19-02's deletion — but that's a Q5 concern (unused-import +
dead-code warnings after the migration), not a grep-check concern.

The `test` job at line 132-133 runs `cargo test --workspace --all-targets` which IS
sensitive — but the 3 callsite migrations Plan 19-02 performs (per_script_vectors.rs +
multi_script_validate.rs) keep all tests green per Q5.

**Planner action:**
- Plan 19-02 has NO CI-script edit task. The CI surface is unchanged.
- Plan 19-02 task ordering: deletions FIRST, callsite migrations SECOND, clippy/test green at the boundary. (Reverse would briefly break local-dev `cargo test`.)

---

## Q5 — Migration mechanics + clippy risk

**VERDICT (MEDIUM-HIGH confidence):** Each of the 3 callsites swaps cleanly to
`sign_simple` with `.expect(...)` (NOT `?` — the surrounding test bodies don't
`Result`-return). Assertions in all 3 sites are verify-roundtrips (NOT hardcoded
witness bytes), so the production-sign witness is interchangeable with the test-only
witness. Predicted clippy warnings: 2 unused-import lines + 1 unused-fn warning (if the
file scoping leaves any `sign_for_tests` reference behind transiently); all are
pre-emptively fixed by the migration tasks themselves.

### Per-callsite migration

**Site 1 — `shared/tests/per_script_vectors.rs:21` (import):**

```diff
-use shared::bip322::{sign_simple, sign_simple_test_only, verify_simple, Bip322Error, ScriptType};
+use shared::bip322::{sign_simple, verify_simple, Bip322Error, ScriptType};
```

**Site 2 — `shared/tests/per_script_vectors.rs:274` (P2TR test body):**

```diff
-    // P2TR production sign_simple is todo!() per CD-6; test path uses
-    // sign_simple_test_only which routes to p2tr::sign_for_tests (the
-    // 8-step BIP-341 sequence from Sprint-0-B).
-    let witness = sign_simple_test_only(ScriptType::P2tr, &spk, &key, message)
-        .expect("sign_simple_test_only p2tr");
+    // P2TR production sign_simple ships in Phase 19 Plan 19-01 (D-116
+    // lifted sign_for_tests body verbatim into production sign + cross-check).
+    let witness = sign_simple(ScriptType::P2tr, &spk, &key, message)
+        .expect("sign_simple p2tr");
```

**Site 3 — `shared/tests/per_script_vectors.rs:311` (P2SH-P2WPKH test body):**

```diff
-    // P2SH-P2WPKH production sign_simple is todo!() per CD-6; test path uses
-    // sign_simple_test_only which routes to p2sh_p2wpkh::sign_for_tests
-    // (sighash over the unwrapped P2WPKH redeem).
-    let witness = sign_simple_test_only(ScriptType::P2shP2wpkh, &spk, &key, message)
-        .expect("sign_simple_test_only p2sh-p2wpkh");
+    // P2SH-P2WPKH production sign_simple ships in Phase 19 Plan 19-01 (D-116
+    // lifted sign_for_tests body verbatim into production sign + D-111 cross-check
+    // + D-117 spk-used-directly after cross-check).
+    let witness = sign_simple(ScriptType::P2shP2wpkh, &spk, &key, message)
+        .expect("sign_simple p2sh-p2wpkh");
```

**Site 4 — `tests/integration/multi_script_validate.rs:23` (import):**

```diff
-use shared::bip322::{sign_simple_test_only, Bip322Error, ScriptType};
+use shared::bip322::{sign_simple, Bip322Error, ScriptType};
```

**Site 5 — `tests/integration/multi_script_validate.rs:114-120` (helper fn body):**

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

**Site 6 — `tests/integration/mod.rs:707,723` (comment refresh):**

```diff
-//      shared::bip322::sign_simple_test_only.
+//      shared::bip322::sign_simple.
```
```diff
-    /// Secret key matching the address — used by the integration tests to
-    /// construct valid BIP-322 v=2 witnesses via
-    /// shared::bip322::sign_simple_test_only.
+    /// Secret key matching the address — used by the integration tests to
+    /// construct valid BIP-322 v=2 witnesses via shared::bip322::sign_simple.
```

### Assertion-shape verification

For each migration site, I verified the test asserts a **verify-roundtrip**, NOT a
hardcoded witness byte equality:

| File:Lines | Assertion | Sensitive to witness byte change? |
|---|---|---|
| `per_script_vectors.rs:276-280` (P2TR) | `verify_simple(...).is_ok()` + `witness.len() == 1` + `sig.len() == 64 || 65` | NO — verify-roundtrip + structural |
| `per_script_vectors.rs:313-328` (P2SH-P2WPKH) | `verify_simple(...).is_ok()` + `witness.len() == 2` | NO — verify-roundtrip + structural |
| `multi_script_validate.rs:131-425` (all 9 D-54 cases) | `validate_ownership_proof_typed(...).is_ok()` / `expect_err matches!(...)` | NO — all check Result variants, not witness bytes |

CONTEXT D-116 ("lift sign_for_tests bodies near-verbatim into sign") guarantees the
witnesses are bit-identical to what the test-only mirror produced. Even if D-116
allowed micro-divergence, the assertions wouldn't catch it — they check only that
the witness verifies, not the exact bytes.

**Confidence note:** The 9 `multi_script_validate.rs` cases also exercise the
coordinator's `validate_ownership_proof_typed` dispatcher (with its CRIT-01 cross-check)
— the witnesses must verify against the on-chain SPK. As long as Plan 19-01's new
production sign body is round-trip correct (which the existing per-script-vector
tests assert), Plan 19-02 keeps these 9 tests green.

### Predicted clippy warnings + cleanup tasks

`cargo clippy --workspace --all-targets -- -D warnings` after Plan 19-02 deletions:

| # | Warning | Origin | Cleanup |
|---|---|---|---|
| C1 | `unused import: sign_simple_test_only` | `per_script_vectors.rs:21` | Site-1 migration removes it pre-emptively |
| C2 | `unused import: sign_simple_test_only` | `multi_script_validate.rs:23` | Site-4 migration removes it pre-emptively |
| C3 | `function sign_for_tests is never used` | `p2wpkh.rs:88-95` (already `#[allow(dead_code)]`) | Plan 19-02 task 2 deletes the fn outright; `#[allow(dead_code)]` goes with it. No leftover warning. |
| C4 | N/A — `_spk` rename to `spk` (D-117) | `p2sh_p2wpkh.rs::sign` | Happens IN Plan 19-01 production-body task; cross-check uses `spk` so the underscore prefix becomes wrong-naming. No clippy warning (Rust doesn't warn on leading-underscore params that ARE used — but it WOULD warn on a param named `spk` that's UNUSED. Since we use it after the cross-check, no warning either way.) |
| C5 | Doc-comment references to deleted helpers | `mod.rs` doc-comment at the dispatcher (line 258-260 references `sign_for_tests` indirectly via "P2TR and P2SH-P2WPKH bodies are `todo!()`") | Plan 19-01 task 1 updates the dispatcher's doc-comment when ship'ing prod bodies. Plan 19-02 verifies no stale references remain. |
| C6 | `Bip322Error` import on test-file may become unused | `per_script_vectors.rs:21` (Bip322Error stays — the file's `_bip322_error_path_check` at line 366 uses it) | NO cleanup needed — verified unused-check passes. |

**Worst case (newly-detected during plan-phase research):** The `tests/integration/mod.rs:707,723`
comment refresh per CD-39 default-folded into Plan 19-02 may leave a third file referencing
`sign_simple_test_only` in a doc-comment (`pub struct TypedUtxoHandle` field doc). Plan 19-02
task 5 handles this; no risk of compile-time or clippy warning (it's a `///` doc-comment, not
a path expression).

### Pre-empted cleanup tasks (insert into Plan 19-02)

**Task 6 — Verify no stale references remain:** After all deletions and migrations, run:

```bash
grep -rn -E '(sign_simple_test_only|sign_for_tests)' \
  shared/ tests/ client/ coordinator/ liquidity-bot/ \
  --include='*.rs' --include='*.md'
# Expected output: zero hits (silent success).
```

If any hit returns, that file needs editing before the Plan 19-02 commit.

**Task 7 — Local pre-commit clippy gate:**

```bash
cargo clippy --workspace --all-targets -- -D warnings
# Expected: green (no errors, no warnings).
```

Both tasks are 1-line additions to Plan 19-02's task list; they catch the predicted
clippy warnings before the CI pipeline does.

### Confidence summary

- Migration mechanics: **HIGH** — 6 callsites enumerated, exact diffs provided, all verified from source.
- Assertion shape (verify-roundtrip, not byte-equality): **HIGH** — re-read each test body line by line.
- Clippy warning prediction: **MEDIUM-HIGH** — C1/C2/C5 are mechanically certain; C3/C6 verified from source; C4 is a Plan 19-01 concern (not 19-02) and well-bounded.
- D-110 byte-count off-by-one observation: **MEDIUM** — needs runtime verification via `dbg!` in the Plan 19-01 unit test before committing the `assert_eq!(bytes.len(), 23)` constant.

---

## RESEARCH COMPLETE
