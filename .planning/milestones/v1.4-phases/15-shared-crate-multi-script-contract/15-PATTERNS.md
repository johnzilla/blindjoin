# Phase 15: Shared Crate Multi-Script Contract - Pattern Map

**Mapped:** 2026-05-29
**Files analyzed:** 14 (4 new src files, 1 refactored src file, 1 evolved type, 1 Cargo.toml, 5 new test/fixture files, 1 deleted enum, 1 modified CI workflow)
**Analogs found:** 13 / 14 (the only file with no analog is `shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json` — derived from upstream `bip322` crate's `lib.rs:46-48` constants per RESEARCH Pitfall 6)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `shared/src/bip322.rs` (DELETE flat file) | refactor-of-existing | n/a (replaced by directory module) | self (the file being replaced) | exact |
| `shared/src/bip322/mod.rs` (NEW) | module-root + dispatcher + adapter + error enum | dispatcher / wire-format / error-mapping | `shared/src/bip322.rs` (primitives) + `coordinator/src/bitcoin/utxo.rs:87-101` (error taxonomy seed) | role-match (synthetic) |
| `shared/src/bip322/p2wpkh.rs` (NEW) | per-script inner module | verify-path + sign-path (BIP-143 ECDSA) | `shared/src/bip322.rs:78-108` (`make_bip322_witness` helper) + `coordinator/src/bitcoin/utxo.rs:114-177` (`verify_bip322_simple`) | exact (sign body) / role-match (verify wrapper) |
| `shared/src/bip322/p2tr.rs` (NEW) | per-script inner module | verify-path + sign-path (BIP-341 Schnorr keypath) | `shared/src/bip322.rs:86-108` (test-only signer pattern) + Sprint-0-B PoC `sprint-0-B.md:130-270` | role-match (no in-repo P2TR signer exists) |
| `shared/src/bip322/p2sh_p2wpkh.rs` (NEW) | per-script inner module | verify-path + sign-path (BIP-143 over unwrapped redeem) | `shared/src/bip322.rs:86-108` (witness shape parity) + bip322 crate `verify.rs:87-94` (HASH160 cross-check internal) | role-match (no in-repo P2SH-P2WPKH signer exists) |
| `shared/src/protocol.rs:105-139` (EVOLVED `OwnershipProof`) | wire-type model | wire-format (versioned envelope) | `shared/src/protocol.rs:30-72` (`InputRegRequest` + `OutputRegRequest` `#[serde(skip_serializing_if)]` pattern) | exact (serde patterns) |
| `shared/Cargo.toml` (MODIFIED) | config (deps) | dependency-pin | `Cargo.toml` workspace root (lines 17-18, 28) + `coordinator/src/bitcoin/utxo.rs:8` (existing thiserror derive site) | exact |
| `shared/tests/ownership_proof_roundtrip.rs` (NEW) | integration test (wire roundtrip) | wire-format roundtrip | `shared/src/bip322.rs:78-132` (`#[cfg(test)] mod tests`) + `shared/src/errors.rs:41-77` (serde roundtrip assertions) | role-match (integration test dir is new) |
| `shared/tests/bip322_cross_shape.rs` (NEW) | integration test (rejection matrix) | verify-path negative | `coordinator/src/bitcoin/utxo.rs:213-236` (rejection `#[test]` fns; `bip322_wrong_witness_length` is the structural twin) | exact (matches!-on-error pattern) |
| `shared/tests/per_script_vectors.rs` (NEW) | integration test (property tests against vendored fixture) | verify-path positive | `coordinator/src/bitcoin/utxo.rs:213-218` (`bip322_valid_p2wpkh` positive test) | role-match |
| `shared/tests/fixtures/bip322/basic-test-vectors.json` (NEW vendored) | test fixture | n/a (data, vendored snapshot) | (none in repo; supply-chain pattern lifted from v1.3 REPAIR-02 `corepc-node` feature pin) | no analog |
| `shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json` (NEW) | test fixture (supplement) | n/a (data, supplement) | (none in repo; lifted from `bip322` crate `lib.rs:46-48` per RESEARCH Pitfall 6) | no analog |
| `coordinator/src/bitcoin/utxo.rs:87-101` (DELETE local `Bip322Error`) | refactor-of-existing (delete + re-import) | error-mapping | self (Phase 15 deletes this 14-LOC enum) | exact |
| `.github/workflows/ci.yml` (NEW grep-gate job for bip322 pin) | config (CI) | dependency-pin | `.github/workflows/ci.yml:183-213` (`corepc-node-feature-pin-check` job) | exact |

---

## Pattern Assignments

### `shared/src/bip322/mod.rs` (NEW — public API + dispatcher + 26-LOC adapter + `Bip322Error`)

**Analog 1:** `shared/src/bip322.rs` (133 LOC, ENTIRELY replaced — script-NEUTRAL primitives carried over verbatim per V1.4-MOD-07)
**Analog 2:** `coordinator/src/bitcoin/utxo.rs:87-101` (existing local `Bip322Error`, expanded from 6 variants to ~10 per CONTEXT D-31)

#### Imports pattern (carry over from `shared/src/bip322.rs:12-13`)

```rust
// shared/src/bip322.rs lines 12-13 — KEEP these in shared/src/bip322/mod.rs verbatim
use bitcoin::{OutPoint, Script, ScriptBuf, Sequence, Witness, Amount, Transaction, TxIn, TxOut};
use bitcoin::hashes::{sha256, HashEngine, Hash};
```

Phase 15 adds these for the dispatcher + adapter (per RESEARCH Pattern 2):

```rust
use bitcoin::{Address, Network};
use bitcoin::secp256k1::SecretKey;
```

#### Script-NEUTRAL primitives (carry over verbatim from `shared/src/bip322.rs:19-76`)

These three functions are **V1.4-MOD-07 single source of truth** — copy verbatim into `mod.rs` (or keep at module root and re-export). Phase 15 must NOT re-implement.

```rust
// shared/src/bip322.rs lines 19-27 — VERBATIM CARRY-OVER
pub fn bip322_message_hash(message: &[u8]) -> [u8; 32] {
    let tag = b"BIP0322-signed-message";
    let tag_hash = sha256::Hash::hash(tag);
    let mut engine = sha256::HashEngine::default();
    engine.input(tag_hash.as_ref());
    engine.input(tag_hash.as_ref());
    engine.input(message);
    sha256::Hash::from_engine(engine).to_byte_array()
}

// shared/src/bip322.rs lines 34-53 — VERBATIM CARRY-OVER
pub fn build_bip322_to_spend(script_pubkey: &Script, msg_hash: &[u8; 32]) -> Transaction { /* ... */ }

// shared/src/bip322.rs lines 60-76 — VERBATIM CARRY-OVER
pub fn build_bip322_to_sign(to_spend: &Transaction) -> Transaction { /* ... */ }
```

#### Error taxonomy pattern (analog: `coordinator/src/bitcoin/utxo.rs:87-101`)

The existing 6-variant local enum is the **shape template**. Phase 15 expands to the ~10 variants in CONTEXT D-31. Critical patterns to preserve:

```rust
// coordinator/src/bitcoin/utxo.rs lines 87-101 — TAXONOMY SEED
#[derive(Debug, thiserror::Error)]
pub enum Bip322Error {
    #[error("Unsupported script type")]
    UnsupportedScriptType,
    #[error("Invalid witness stack length: expected 2, got {0}")]
    InvalidWitnessLength(usize),
    #[error("ECDSA signature parse error")]
    SigParseError,
    #[error("Public key parse error")]
    PubkeyParseError,
    #[error("Signature verification failed")]
    VerificationFailed,
    #[error("Script mismatch: pubkey does not match script_pubkey")]
    ScriptMismatch,
}
```

Phase 15 evolves to D-31's 10 variants (executor reads CONTEXT D-31 verbatim). Critical evolution points the executor MUST honour:
- `InvalidWitnessLength` becomes struct-style `{ expected: usize, got: usize }` (analog uses tuple `(usize)` — break this for per-script clarity, since arity differs per script).
- Add `#[source]` chain for `UnrecognisedScriptPubkey { source: bitcoin::address::FromScriptError }` and `CrateVerifyFailed { source: bip322::error::Error }` per Sprint-0-A:145-175.
- `ScriptMismatch` (preserved from analog) maps to D-31's identical variant for v1 path parity.

#### Dispatcher pattern (no in-repo analog; lift from RESEARCH Pattern 1 + Sprint-0-A:145-175)

The dispatcher fn-signatures are the **public contract** Phase 16 / Phase 17 link against. Executor lifts the shape verbatim from RESEARCH `## Architecture Patterns / Pattern 1: Dispatcher with hidden inner modules` (lines 343-429 of 15-RESEARCH.md):

```rust
pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error> { /* per RESEARCH Pattern 1 */ }
pub fn verify_simple(script_type: ScriptType, spk: &Script, witness: &Witness, message: &[u8], network: Network) -> Result<(), Bip322Error> { /* dispatcher */ }
pub fn sign_simple(script_type: ScriptType, spk: &Script, key: &SecretKey, message: &[u8]) -> Result<Witness, Bip322Error> { /* dispatcher; P2TR/P2SH-P2WPKH bodies = todo!() per CD-6 */ }
```

#### 26-LOC adapter (no in-repo analog; lift from Sprint-0-A:145-175 verbatim)

Per CONTEXT D-26 the adapter lives as a `pub(crate)` fn inside `mod.rs`. Executor copies RESEARCH `## Architecture Patterns / Pattern 2` (lines 431-462 of 15-RESEARCH.md):

```rust
pub(crate) fn verify_via_bip322_crate(
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), Bip322Error> {
    let address = Address::from_script(spk, network)
        .map_err(|source| Bip322Error::UnrecognisedScriptPubkey { source })?;
    bip322::verify_simple(&address, message, witness.clone())
        .map_err(|source| Bip322Error::CrateVerifyFailed { source })
}
```

**Faithfulness note** (per RESEARCH Pattern 2): `bip322::verify_simple` takes `Witness` BY VALUE; `witness.clone()` is required (cheap; `bitcoin::Witness` is `derive(Clone)`).

---

### `shared/src/bip322/p2wpkh.rs` (NEW — `pub(crate)` inner module)

**Analog 1 (sign body):** `shared/src/bip322.rs:86-108` (`make_bip322_witness` test helper) — direct lift, generalised from test helper to `pub(crate)` fn.
**Analog 2 (verify body):** `coordinator/src/bitcoin/utxo.rs:114-177` (`verify_bip322_simple`) — structural reference; Phase 15's `verify` only does arity-check + adapter delegate, NOT a full re-implementation (the crate handles it).

#### Verify pattern (lift from RESEARCH Example 1, lines 722-746 of 15-RESEARCH.md)

```rust
pub(crate) fn verify(
    spk: &bitcoin::Script,
    witness: &bitcoin::Witness,
    message: &[u8],
    network: bitcoin::Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 2 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 2,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}
```

The arity check (`witness.len() != 2`) is the **load-bearing pre-flight** that satisfies the 9-rejection matrix's `reject_p2wpkh_spk_with_empty_witness` assertion (D-34). Per RESEARCH Assumption A10, this is technically redundant with the bip322 crate's internal check, but our pre-flight converts "`CrateVerifyFailed`" into the more-precise "`InvalidWitnessLength`" the matrix asserts.

#### Sign pattern (lift verbatim from `shared/src/bip322.rs:86-108` — the existing `make_bip322_witness` test helper)

```rust
// shared/src/bip322.rs lines 86-108 — LIFT this into shared/src/bip322/p2wpkh.rs::sign
// Generalise: replace SecretKey::from_slice(&[0x01_u8; 32]) with the `key: &SecretKey` parameter.
pub fn make_bip322_witness(message: &str) -> (ScriptBuf, Vec<Vec<u8>>) {
    let secp = Secp256k1::new();
    let secret_key = SecpSecretKey::from_slice(&[0x01_u8; 32]).unwrap();    // ← REPLACE with key param
    let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let compressed = PublicKey::new(pubkey);
    let script_pubkey = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash().unwrap());

    let msg_hash = bip322_message_hash(message.as_bytes());
    let to_spend = build_bip322_to_spend(&script_pubkey, &msg_hash);
    let to_sign = build_bip322_to_sign(&to_spend);

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache
        .p2wpkh_signature_hash(0, &script_pubkey, Amount::ZERO, EcdsaSighashType::All)
        .unwrap();
    let secp_msg = bitcoin::secp256k1::Message::from_digest(*sighash.as_byte_array());
    let sig = secp.sign_ecdsa(&secp_msg, &secret_key);
    let mut sig_bytes = sig.serialize_der().to_vec();
    sig_bytes.push(0x01); // SIGHASH_ALL

    let witness_stack = vec![sig_bytes, pubkey.serialize().to_vec()];
    (script_pubkey, witness_stack)
}
```

Production sign body in `p2wpkh.rs::sign` (per RESEARCH Example 4, lines 802-840):
- Takes `key: &SecretKey` parameter (no hardcoded `[0x01_u8; 32]`).
- Returns `Result<Witness, Bip322Error>` not `(ScriptBuf, Vec<Vec<u8>>)` — pushes to `bitcoin::Witness::new()` instead.
- Replaces `.unwrap()` with `.map_err(|e| Bip322Error::DecodeError(format!("p2wpkh sighash: {e}")))` per D-31.

---

### `shared/src/bip322/p2tr.rs` (NEW — `pub(crate)` inner module)

**Analog 1 (verify):** None in repo; lift from RESEARCH Example 2 (lines 750-771 of 15-RESEARCH.md). Body is essentially `if witness.len() != 1 { Err(...) }; super::verify_via_bip322_crate(...)` — the bip322 crate handles BIP-341 keypath sighash + 64/65-byte branching internally.
**Analog 2 (#[cfg(test)] sign helper):** `shared/src/bip322.rs:86-108` (`make_bip322_witness` shape; substitute `taproot_key_spend_signature_hash` + `tap_tweak` + `sign_schnorr_no_aux_rand` for the ECDSA path).
**Analog 3 (sign sequence):** Sprint-0-B `.planning/research/sprint-0-B.md:130-270` — 8-step BIP-341 sign PoC; lift verbatim into `#[cfg(test)] fn sign_for_tests`.

#### Verify pattern (lift from RESEARCH Example 2)

```rust
pub(crate) fn verify(
    spk: &bitcoin::Script,
    witness: &bitcoin::Witness,
    message: &[u8],
    network: bitcoin::Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 1 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 1,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}
```

Per RESEARCH §"Don't Hand-Roll" row 1: do NOT implement a custom `taproot_key_spend_signature_hash` + `verify_schnorr` chain. The crate covers SIGHASH_DEFAULT (64-byte) AND SIGHASH_ALL (65-byte) at `verify.rs:214-231`.

#### Production sign (CD-6 default: `todo!()`)

```rust
pub(crate) fn sign(
    _spk: &bitcoin::Script,
    _key: &bitcoin::secp256k1::SecretKey,
    _message: &[u8],
) -> Result<bitcoin::Witness, super::Bip322Error> {
    todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")
}
```

#### `#[cfg(test)] sign_for_tests` (lift from RESEARCH Pattern 4 verbatim, lines 537-600 of 15-RESEARCH.md)

This is the load-bearing test-only signer that enables per-script property tests to run end-to-end inside `shared/`. The 8-step sequence is verified against Sprint-0-B and the bip322 crate's `sign.rs:155-216`.

---

### `shared/src/bip322/p2sh_p2wpkh.rs` (NEW — `pub(crate)` inner module)

**Analog 1 (verify):** None in repo; lift from RESEARCH Example 3 (lines 775-799 of 15-RESEARCH.md). The HASH160 cross-check is implicit inside the bip322 crate's `verify_full_p2wpkh(is_p2sh=true)` at `verify.rs:167-169` per RESEARCH §"Don't Hand-Roll" row 2.
**Analog 2 (witness shape):** `shared/src/bip322.rs:86-108` (`make_bip322_witness`) — same `[sig, pubkey]` 2-item witness shape as P2WPKH. The difference is at the SPK byte layer: P2SH-P2WPKH = `OP_HASH160 <20-byte-hash> OP_EQUAL`, with the unwrapped P2WPKH `final_script_sig` sitting outside the witness.

#### Verify pattern (lift from RESEARCH Example 3 verbatim)

```rust
pub(crate) fn verify(
    spk: &bitcoin::Script,
    witness: &bitcoin::Witness,
    message: &[u8],
    network: bitcoin::Network,
) -> Result<(), super::Bip322Error> {
    if witness.len() != 2 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 2,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}
```

#### Production sign (CD-6 default: `todo!()`)

Same shape as `p2tr.rs::sign` — body is `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")`.

#### `#[cfg(test)] sign_for_tests`

Generalisation of `shared/src/bip322.rs::make_bip322_witness` (lines 86-108) with the addition of `final_script_sig = OP_PUSH <wpkh_script>`. Per CONTEXT `<specifics>`: "P2SH-P2WPKH uses the same shape as P2WPKH but with `final_script_sig = OP_HASH160 <redeem-script-hash>` and witness = `[sig, pubkey]`".

---

### `shared/src/protocol.rs:105-139` — EVOLVED `OwnershipProof`

**Analog 1 (serde patterns):** `shared/src/protocol.rs:30-72` (`InputRegRequest`, `OutputRegRequest`, `OutputRegResponse`). The `#[serde(skip_serializing_if = "Option::is_none")]` pattern on `msg_randomizer` (line 70) is the template for Phase 15's `psbt_input_b64: Option<String>` and `script_type: Option<ScriptType>` fields.
**Analog 2 (helpers):** `shared/src/protocol.rs:121-138` (existing `from_json_hex_str` / `to_json_hex_str`) — Phase 15 evolves these to two-phase try-parse per CD-7. The legacy v1.3 array-of-hex shape (existing `hex::decode` loop at lines 126-128) becomes phase 1 of the new try-parse.

#### Imports pattern (from `shared/src/protocol.rs:1`)

```rust
// shared/src/protocol.rs line 1 — KEEP verbatim
use serde::{Deserialize, Serialize};
```

#### `#[serde(skip_serializing_if = "Option::is_none")]` pattern (from `shared/src/protocol.rs:69-71`)

```rust
// shared/src/protocol.rs lines 69-72 — TEMPLATE for psbt_input_b64 and script_type
#[serde(skip_serializing_if = "Option::is_none")]
pub msg_randomizer: Option<String>,
```

Apply to:
- `psbt_input_b64: Option<String>` (CONTEXT D-22)
- `script_type: Option<ScriptType>` (CONTEXT D-22)

#### Existing struct shape to evolve (`shared/src/protocol.rs:105-139` verbatim)

```rust
// shared/src/protocol.rs lines 113-116 — REPLACE with the D-22 4-field struct
pub struct OwnershipProof {
    /// The raw witness stack items (decoded from hex on receive, encoded to hex on send)
    pub witness_stack: Vec<Vec<u8>>,
}
```

becomes (per CONTEXT D-22 verbatim + CD-7 two-phase try-parse from RESEARCH Pattern 3 lines 471-527):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipProof {
    #[serde(default = "default_proof_version")] pub version: u8,
    #[serde(default)] pub witness_stack: Vec<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")] pub psbt_input_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub script_type: Option<crate::bip322::ScriptType>,
}

fn default_proof_version() -> u8 { 1 }
```

#### Existing `from_json_hex_str` (lines 121-131 verbatim) — KEEP signature, EVOLVE body

```rust
// shared/src/protocol.rs lines 121-131 — PRESERVE signature Result<Self, String> per RESEARCH Pitfall 7
// EVOLVE body to CD-7 two-phase try-parse:
pub fn from_json_hex_str(s: &str) -> Result<Self, String> {
    let items: Vec<String> = serde_json::from_str(s)
        .map_err(|e| format!("OwnershipProof: JSON parse error: {e}"))?;
    let witness_stack = items
        .iter()
        .map(|h| {
            hex::decode(h).map_err(|e| format!("OwnershipProof: hex decode error: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Self { witness_stack })
}
```

Per RESEARCH Pitfall 7: **return type stays `Result<Self, String>`** — typing it as `Result<Self, Bip322Error>` would force a module cycle (`protocol.rs` imports from `bip322/` while `bip322/` already imports `ScriptType` via re-export). Keep untyped here.

#### v1 backwards-compat encode rule (CD-7 default)

Per RESEARCH Pattern 3 lines 514-525: `to_json_hex_str` emits the v1.3 array-of-hex shape when `version == 1 && psbt_input_b64.is_none() && script_type.is_none()`, otherwise flat-struct JSON. This preserves bit-exact wire compatibility with v1.3 coordinators that haven't read the flat-struct shape.

#### Cross-cutting forward-compat constraint (`shared/src/protocol.rs:3-5` verbatim)

```rust
// shared/src/protocol.rs lines 3-5 — INVARIANT: do NOT add deny_unknown_fields
// NO #[serde(deny_unknown_fields)] on any struct — forward compat per D-06 / T-01-04.
// All structs silently drop unknown fields, allowing protocol evolution without breaking
// older clients or coordinators.
```

The new `OwnershipProof` MUST preserve this — RESEARCH §Anti-Patterns confirms.

---

### `shared/Cargo.toml` — MODIFIED (add 3 lines + 1 dev-dep section)

**Analog 1 (workspace re-export pattern):** `Cargo.toml` workspace root lines 17 (`thiserror = "1"`), 28 (`proptest = "1"`).
**Analog 2 (existing thiserror derive site):** `coordinator/src/bitcoin/utxo.rs:8` (`#[derive(Debug, thiserror::Error)]`) — proves `thiserror = { workspace = true }` works in the workspace.

#### Existing baseline (`shared/Cargo.toml` lines 1-13 verbatim)

```toml
[package]
name = "shared"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
bitcoin = { workspace = true }
sha2 = { workspace = true }
uuid = { workspace = true }
hex = "0.4"
```

#### Phase 15 additions (per RESEARCH "Installation" lines 158-172)

```toml
[dependencies]
# ... existing ...
# NEW in Phase 15:
bip322 = "=0.0.10"
thiserror = { workspace = true }

[dev-dependencies]
# NEW in Phase 15:
proptest = { workspace = true }
```

**A8 open question (RESEARCH §Sources Tertiary, line 1070):** whether `base64 = "0.22"` needs to be added as a 4th direct dep. Default per RESEARCH Pattern 3: try `bitcoin::base64` re-export first. Planner adds `base64 = "0.22"` only if `bitcoin::base64::Engine` is not publicly re-exported. Verify at plan time via `cargo doc -p bitcoin`.

**Workspace pin precedence (RESEARCH A7 line 960):** workspace pins `thiserror = "1"` (caret, NOT exact). The CI grep gate's pin enforcement is on the lockfile, not the Cargo.toml string — see CI section below.

---

### `shared/tests/ownership_proof_roundtrip.rs` (NEW — 15-01-PLAN.md atomic commit)

**Analog 1:** `shared/src/bip322.rs:78-132` (existing `#[cfg(test)] mod tests`). Phase 15's `shared/tests/` directory is NEW (no existing analog in shared/), but the **test-fn shape** mirrors the existing inline test patterns.
**Analog 2 (serde roundtrip assertions):** `shared/src/errors.rs:46-76` — three `#[test]` fns that assert serde roundtrip. Direct pattern:

```rust
// shared/src/errors.rs lines 46-51 — TEMPLATE for assert-roundtrip pattern
#[test]
fn error_code_serializes_screaming_snake_case() {
    assert_eq!(
        serde_json::to_string(&ErrorCode::UtxoSpent).unwrap(),
        "\"UTXO_SPENT\""
    );
}
```

#### Imports pattern (Phase 15 file is in `shared/tests/`, NOT `shared/src/`)

```rust
// shared/tests/ownership_proof_roundtrip.rs — integration test imports public API only
use shared::bip322::ScriptType;
use shared::protocol::OwnershipProof;
```

(No `use super::*` — integration tests in `tests/` are external crates per Cargo convention.)

#### Test pattern (lift from RESEARCH Example 5 verbatim, lines 845-908 of 15-RESEARCH.md)

The 5 D-13 test cases are spelled out in RESEARCH Example 5:
1. `v2_roundtrip_p2wpkh` — flat-struct encode → decode → field-by-field assert
2. `v2_roundtrip_p2tr` — same, P2TR variant
3. `v2_roundtrip_p2sh_p2wpkh` — same, P2SH-P2WPKH variant
4. `v1_legacy_decode_array_of_hex` — bit-exact v1.3 input decodes as `version: 1`
5. `unknown_version_rejects_on_verify_dispatch` — `version: 3` decodes permissively but downstream verify rejects
6. (bonus, ships in same file) `corrupted_base64_in_psbt_input_rejects_on_decode` — non-base64 string in `psbt_input_b64`

Each `#[test]` fn follows the `serde_json::from_str`/`to_string`/assert_eq! pattern from `shared/src/errors.rs:46-76`.

#### Atomic commit constraint (per CONTEXT CD-10 + REPAIR-01 lesson #1)

This file MUST ship in 15-01-PLAN.md as its OWN atomic commit, BEFORE the bip322 module split (15-02). Per CONTEXT `<canonical_refs>` v1.3 carry-forward: lesson #1 (wire-format roundtrip test ships FIRST) is non-negotiable.

---

### `shared/tests/bip322_cross_shape.rs` (NEW — 15-03-PLAN.md)

**Analog 1 (matches!-on-error pattern):** `coordinator/src/bitcoin/utxo.rs:220-226` — direct structural twin:

```rust
// coordinator/src/bitcoin/utxo.rs lines 220-226 — TEMPLATE for D-34 rejection harness
#[test]
fn bip322_wrong_witness_length() {
    let msg = "blindjoin:round:test:utxo:abc:0";
    let (script, _witness) = make_p2wpkh_and_witness(msg);
    let result = verify_bip322_simple(&script, &[vec![0x01]], msg);
    assert!(matches!(result, Err(Bip322Error::InvalidWitnessLength(1))));
}
```

The 9-rejection matrix per D-34 follows this exact pattern, with each `#[test]` fn:
1. Constructs a known SPK of one type
2. Constructs a known witness of a DIFFERENT type (or empty)
3. Calls `verify_simple(declared_type, spk, witness, msg, Network::Regtest)`
4. Asserts a specific `Bip322Error` variant via `matches!`

#### Test pattern (lift from RESEARCH Example 6 verbatim, lines 912-933 of 15-RESEARCH.md)

```rust
use shared::bip322::{verify_simple, ScriptType, Bip322Error};
use bitcoin::{Network, ScriptBuf, Witness};

#[test]
fn reject_p2wpkh_spk_with_p2tr_witness() {
    let p2wpkh_spk = make_known_p2wpkh_spk();
    let p2tr_witness = make_p2tr_keypath_witness(); // 1 element, 64 bytes
    let message = b"test";
    let result = verify_simple(ScriptType::P2wpkh, &p2wpkh_spk, &p2tr_witness, message, Network::Regtest);
    assert!(matches!(
        result,
        Err(Bip322Error::InvalidWitnessLength { expected: 2, got: 1 })
    ));
}
```

#### Nine enumerated `#[test]` fn names (CONTEXT D-34 verbatim)

```
reject_p2wpkh_spk_with_p2tr_witness
reject_p2wpkh_spk_with_p2sh_p2wpkh_witness
reject_p2tr_spk_with_p2wpkh_witness
reject_p2tr_spk_with_p2sh_p2wpkh_witness
reject_p2sh_p2wpkh_spk_with_p2wpkh_witness
reject_p2sh_p2wpkh_spk_with_p2tr_witness
reject_p2wpkh_spk_with_empty_witness         // arity edge case
reject_p2tr_spk_with_empty_witness            // arity edge case
reject_p2sh_p2wpkh_spk_with_empty_witness     // arity edge case
```

---

### `shared/tests/per_script_vectors.rs` (NEW — 15-03-PLAN.md)

**Analog (positive-test pattern):** `coordinator/src/bitcoin/utxo.rs:213-218`:

```rust
// coordinator/src/bitcoin/utxo.rs lines 213-218 — TEMPLATE for positive vector tests
#[test]
fn bip322_valid_p2wpkh() {
    let msg = "blindjoin:round:test-round-id:utxo:abc:0";
    let (script, witness) = make_p2wpkh_and_witness(msg);
    assert!(verify_bip322_simple(&script, &witness, msg).is_ok());
}
```

Phase 15 generalises to per-script positive vectors driven by the vendored fixture. Per RESEARCH Assumption A4 (the 2-vector supplement gap for P2SH-P2WPKH), the test harness iterates BOTH `basic-test-vectors.json` AND `p2sh_p2wpkh_supplement.json` with a uniform shape (`message`, `private_keys`, `address`, `type`, `bip322_signatures`).

#### Compile-time fixture loading (D-33 invariant)

```rust
// Per CONTEXT D-33: include_str!("fixtures/bip322/basic-test-vectors.json")
const BASIC_VECTORS: &str = include_str!("fixtures/bip322/basic-test-vectors.json");
const P2SH_P2WPKH_SUPPLEMENT: &str = include_str!("fixtures/bip322/p2sh_p2wpkh_supplement.json");
```

---

### `shared/tests/fixtures/bip322/basic-test-vectors.json` (NEW — vendored)

**No in-repo analog.** Supply-chain pattern is lifted from v1.3 REPAIR-02 `corepc-node` feature pin (vendoring as defence against upstream drift).

Per CONTEXT D-33:
- Header comment line records `# source: bitcoin/bips@<commit-sha>; captured 2026-05-XX`.
- Sibling `README.md` records commit SHA + capture date + `curl` command used.
- File is `include_str!`'d at compile time per the per_script_vectors.rs pattern above.

---

### `shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json` (NEW)

**No in-repo analog.** Lifted from upstream `bip322` crate `lib.rs:46-48` per RESEARCH Pitfall 6 (line 698-706). Format matches upstream `simple` array entry shape so per_script_vectors.rs can iterate both files uniformly:

```json
{
  "message": "...",
  "private_keys": "...",
  "address": "3HSVzEhCFuH9Z3wvoWTexy7BMVVp3PjS6f",
  "type": "p2sh-p2wpkh",
  "witness_script": "",
  "bip322_signatures": ["..."]
}
```

Includes a header comment explaining the supplement: `# Supplement for upstream basic-test-vectors.json which has 0 P2SH-P2WPKH cases. Lifted from bip322 v0.0.10 lib.rs:46-48 + lib.rs:299-321 test constants.`

---

### `coordinator/src/bitcoin/utxo.rs:87-101` — DELETE local `Bip322Error`

**Analog: self.** The 14-LOC local enum at `coordinator/src/bitcoin/utxo.rs:87-101` is deleted in Phase 15 per CONTEXT D-29. The replacement is an import at the top of the file:

```rust
// coordinator/src/bitcoin/utxo.rs — Phase 15 changes lines 4-5 to add the import:
use shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign, Bip322Error};
//                                                                                     ^^^^^^^^^^^ ADD
use shared::protocol::OwnershipProof;
```

Per CONTEXT `<canonical_refs>` code anchors and RESEARCH §"State of the Art":
- `verify_bip322_simple` STAYS in coordinator (lines 114-177) — Phase 16 swaps the call site to the new dispatcher.
- The `is_p2wpkh()` hard gate at line 119 STAYS — Phase 16 removes it.
- ONLY the local `Bip322Error` enum (lines 87-101) is deleted in Phase 15.

#### Cross-check that the existing variants survive the migration

The existing 6 local variants need to be mapped to D-31's 10-variant `shared::bip322::Bip322Error`. Per CONTEXT D-31 and the existing analog:

| Existing local variant (`utxo.rs:87-101`) | Maps to `shared::bip322::Bip322Error` (D-31) |
|------|------|
| `UnsupportedScriptType` | `UnsupportedScriptType` (identical) |
| `InvalidWitnessLength(usize)` | `InvalidWitnessLength { expected, got }` (struct-style; arity-aware) |
| `SigParseError` | Folded into `CrateVerifyFailed { source: bip322::error::Error }` |
| `PubkeyParseError` | Folded into `CrateVerifyFailed { source: bip322::error::Error }` |
| `VerificationFailed` | Folded into `CrateVerifyFailed { source: bip322::error::Error }` |
| `ScriptMismatch` | `ScriptMismatch` (preserved per D-31 for v1 path parity) |

Phase 15's `verify_bip322_simple` function body (lines 114-177) will need its `Err(...)` returns updated to use the new variant names from `shared::bip322::Bip322Error`. The call-site at line 74-75 already does `.map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })` so the wire-shape is preserved.

---

### `.github/workflows/ci.yml` — ADD new grep-gate job (15-02 or 15-03)

**Analog:** `.github/workflows/ci.yml:183-213` (`corepc-node-feature-pin-check` job). Exact copy-paste-modify template per RESEARCH Open Question #2 (lines 976-979) and Assumption A12 (line 965).

#### Existing job verbatim (CI lines 183-213)

```yaml
  corepc-node-feature-pin-check:
    name: corepc-node feature pin check
    runs-on: ubuntu-latest
    # REPAIR-02 invariant: every `corepc-node = ...` declaration in any
    # Cargo.toml in the workspace must include an explicit `features = ...`
    # clause. corepc-node defaults to the silent `0_17_2` (Bitcoin Core 0.17.2,
    # released 2018) RPC schema if no version feature is selected; this gate
    # catches future additions that forget the features clause.
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - name: Enforce explicit corepc-node feature
        run: |
          set -eu
          if grep -rEn 'corepc-node\s*=' --include='Cargo.toml' . \
             | grep -v 'features\s*=' \
             | grep -v '^[^:]*:[0-9]*:#'; then
            echo "ERROR: corepc-node declaration(s) above lack an explicit 'features = [...]' clause." >&2
            echo "       Without a version feature, corepc-node uses the Bitcoin Core 0.17.2 (2018) RPC schema." >&2
            echo "       Add 'features = [\"30_2\"]' (or whatever version pin is appropriate) to each declaration." >&2
            exit 1
          fi
```

#### Phase 15 NEW job (apply this template)

The new job asserts `bip322\s*=\s*"=0\.0\.10"` (exact-equals pin). Per RESEARCH Open Question #2 recommendation: add as a SEPARATE job (one job per invariant) for clearer PR check log output.

```yaml
  bip322-pin-check:
    name: bip322 exact-version pin check
    runs-on: ubuntu-latest
    # v1.4 ADR Decision #1 invariant: bip322 is pre-1.0; the API can change
    # between patch releases. Pin must be EXACTLY =0.0.10 (note the `=` operator).
    # The 26-LOC adapter at shared/src/bip322/mod.rs is verified against this version
    # only; any drift requires the adapter to be re-verified.
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - name: Enforce exact bip322 pin
        run: |
          set -eu
          if grep -rEn 'bip322\s*=' --include='Cargo.toml' . \
             | grep -v '=\s*"=0\.0\.10"' \
             | grep -v '^[^:]*:[0-9]*:#'; then
            echo "ERROR: bip322 declaration(s) above lack the exact-version pin '=0.0.10'." >&2
            echo "       The bip322 crate is pre-1.0; minor changes can break the adapter at shared/src/bip322/mod.rs." >&2
            exit 1
          fi
```

(RESEARCH §Assumptions A7 notes the workspace `thiserror` is workspace-managed, so a separate grep-gate for `thiserror` is NOT required at the per-crate Cargo.toml layer — workspace inheritance handles it.)

---

## Shared Patterns

### Shared Pattern 1: `#[derive(Debug, thiserror::Error)]` for typed errors

**Source:** `coordinator/src/bitcoin/utxo.rs:8-20` (existing `UtxoError`) + `:87-101` (existing `Bip322Error`).
**Apply to:** `shared/src/bip322/mod.rs` `Bip322Error` (the new ~10-variant taxonomy).

```rust
// coordinator/src/bitcoin/utxo.rs lines 8-20 — REFERENCE TEMPLATE for thiserror derive
#[derive(Debug, thiserror::Error)]
pub enum UtxoError {
    #[error("UTXO not found or already spent")]
    NotFound,
    #[error("UTXO already registered in this round")]
    AlreadyRegistered,
    #[error("UTXO value {value} sats insufficient (need {required} sats)")]
    InsufficientValue { value: u64, required: u64 },
    // ...
}
```

Per RESEARCH Pitfall 4: `#[error("...")]` controls Display; `#[source]` controls cause chain. The wire (single-bucket `InvalidOwnershipProof` per D-32) is opaque by design; the cause chain is preserved for server-side `tracing::warn!(error = ?e, error.source = ?e.source(), ...)`.

### Shared Pattern 2: serde forward-compat (no `deny_unknown_fields`)

**Source:** `shared/src/protocol.rs:3-5` (verbatim comment-as-invariant).
**Apply to:** The new `OwnershipProof` v2 struct — preserve the invariant.

```rust
// shared/src/protocol.rs lines 3-5 — INVARIANT to preserve across Phase 15
// NO #[serde(deny_unknown_fields)] on any struct — forward compat per D-06 / T-01-04.
// All structs silently drop unknown fields, allowing protocol evolution without breaking
// older clients or coordinators.
```

Per RESEARCH §"Anti-Patterns to Avoid": `#[serde(deny_unknown_fields)]` on the new `OwnershipProof` would violate T-01-04.

### Shared Pattern 3: `#[serde(default = "...")]` for backwards-compat defaults

**Source:** `shared/src/protocol.rs:70` (`#[serde(skip_serializing_if = "Option::is_none")]`) + general serde patterns.
**Apply to:** `OwnershipProof.version: u8` with `#[serde(default = "default_proof_version")]` returning `1` per CONTEXT D-25.

```rust
// PATTERN (per CONTEXT D-25 + CD-7):
#[serde(default = "default_proof_version")]
pub version: u8,
// ...
fn default_proof_version() -> u8 { 1 }
```

### Shared Pattern 4: workspace dep inheritance

**Source:** `shared/Cargo.toml:7-11` (existing `{ workspace = true }` declarations) + workspace root `Cargo.toml:5-29`.
**Apply to:** Phase 15's new `thiserror = { workspace = true }` and `proptest = { workspace = true }` (the exception is `bip322 = "=0.0.10"` which is EXACT-PINNED at the shared/ Cargo.toml because the workspace doesn't yet pin bip322).

```toml
# shared/Cargo.toml lines 7-11 — TEMPLATE
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
bitcoin = { workspace = true }
sha2 = { workspace = true }
uuid = { workspace = true }
```

### Shared Pattern 5: `pub(crate)` for inner mechanics + `pub` for dispatcher only

**Source:** No direct in-repo analog (Phase 15 establishes this). RESEARCH Pattern 1 documents the rationale.
**Apply to:** Every per-script file (`p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`) declares `pub(crate) fn verify` / `pub(crate) fn sign` ONLY. The dispatcher in `mod.rs` is the only `pub` entry point. This is the V1.4-CRIT-01 mitigation at the type level — coordinator/client cannot accidentally bypass dispatch because no per-script `pub fn` exists.

### Shared Pattern 6: Test fn naming as documentation

**Source:** `coordinator/src/bitcoin/utxo.rs:213-236` (`bip322_valid_p2wpkh`, `bip322_wrong_witness_length`, `bip322_wrong_message_fails`).
**Apply to:** The 9 enumerated `reject_<spk>_<witness>` test fns in `shared/tests/bip322_cross_shape.rs`. Each test name spells out exactly which V1.4-CRIT-01 spoofing vector it closes.

### Shared Pattern 7: `serde_json::from_str` + assert roundtrip

**Source:** `shared/src/errors.rs:46-76` (3 existing roundtrip tests).
**Apply to:** Every #[test] in `shared/tests/ownership_proof_roundtrip.rs`.

```rust
// shared/src/errors.rs lines 46-51 — REFERENCE PATTERN
#[test]
fn error_code_serializes_screaming_snake_case() {
    assert_eq!(
        serde_json::to_string(&ErrorCode::UtxoSpent).unwrap(),
        "\"UTXO_SPENT\""
    );
}
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `shared/tests/fixtures/bip322/basic-test-vectors.json` | test fixture | n/a (vendored data) | No vendored-fixture pattern exists in the repo yet. The supply-chain hardening template (v1.3 REPAIR-02 corepc-node feature pin) is conceptually parallel but architecturally distinct (one is a Cargo.toml feature pin; the other is an `include_str!`'d JSON file). The CI grep-gate analog above covers the supply-chain enforcement; the fixture file itself is new ground for the repo. |
| `shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json` | test fixture (supplement) | n/a (data lifted from external crate) | RESEARCH Pitfall 6 documents the upstream `basic-test-vectors.json` gap (0 P2SH-P2WPKH cases). The supplement is derived from `bip322 v0.0.10 lib.rs:46-48 + lib.rs:284-335` constants per RESEARCH §"Pitfall 6 / How to avoid (recommended)". Format matches the upstream `simple` array entry shape so the test harness can iterate both files uniformly. |
| Architecture: `shared::bip322::sign_simple` dispatcher (with P2TR/P2SH-P2WPKH `todo!()`) | dispatcher | sign-path | The repo has no precedent for an API surface where production bodies are `todo!()` and `#[cfg(test)]` bodies enable property tests. RESEARCH Pattern 4 (lines 530-602) lifts this from CD-6 + Sprint-0-B; planner copies that pattern wholesale. |

---

## Metadata

**Analog search scope:**
- `shared/src/` — all 5 files (bip322.rs, errors.rs, lib.rs, protocol.rs, token.rs, types.rs)
- `coordinator/src/bitcoin/utxo.rs` (the existing BIP-322 verify call site + local error enum)
- `coordinator/src/api/handlers.rs:130-180` (the existing `from_json_hex_str` call site)
- `client/src/round/input.rs:55-80` (the existing OwnershipProof construction site)
- `Cargo.toml` workspace root (workspace pin patterns)
- `shared/Cargo.toml` (existing dep declarations)
- `.github/workflows/ci.yml:183-213` (CI grep-gate template)

**Files scanned:** ~12 source files + 1 workspace Cargo + 1 CI workflow + 5 planning docs (CONTEXT, RESEARCH, ROADMAP, STATE, plus the v1.4 ADR for cross-reference)

**Pattern extraction date:** 2026-05-29

**Key insight:** Phase 15's "no in-repo P2TR / P2SH-P2WPKH analog" is by design — the entire point of the phase is to introduce these script types to `shared/`. The planner's job is to:
1. Carry over `shared/src/bip322.rs` script-NEUTRAL primitives verbatim (V1.4-MOD-07 single source of truth).
2. Generalise the `make_bip322_witness` test helper into `p2wpkh.rs::sign` + `p2wpkh.rs::sign_for_tests` (P2WPKH path, full production-ready).
3. Lift the Sprint-0-A 26-LOC adapter sketch + Sprint-0-B 8-step P2TR sign sequence into `mod.rs` + `p2tr.rs` (research artefacts ARE the analog).
4. Apply the existing serde patterns from `shared/src/protocol.rs:30-72` to the new `OwnershipProof` v2 envelope.
5. Apply the existing `thiserror::Error` derive pattern from `coordinator/src/bitcoin/utxo.rs:8` to the new ~10-variant `Bip322Error`.
6. Apply the existing CI grep-gate template from `.github/workflows/ci.yml:183-213` to enforce `bip322 = "=0.0.10"`.
7. Apply the existing serde roundtrip test pattern from `shared/src/errors.rs:46-76` to the 5 D-13 cases + 9-rejection matrix.

Every Phase 15 file has either (a) a direct structural analog in the codebase, or (b) a verbatim sketch in the locked research artefacts (Sprint-0-A:145-175, Sprint-0-B:130-270). The planner's task is mechanical lifting + CONTEXT D-22..D-34 application, not original architecture.
