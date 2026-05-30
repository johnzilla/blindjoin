# Phase 17: Client Multi-Script Wallet & Discovery — Pattern Map

**Mapped:** 2026-05-30
**Files analyzed:** 7 (6 client-side modifications/additions + 1 integration test)
**Analogs found:** 7 / 7 (all files have strong in-tree analogs)

---

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------------|------|-----------|----------------|---------------|
| `client/src/config.rs` (modify — add `--type` field on `ClientConfig`) | config / CLI | request-response (clap parse) | `client/src/config.rs::ClientConfig` (existing `#[arg(long, env = ...)]` fields, lines 7-65) | exact (same struct, same crate) |
| `client/src/wallet.rs::BdkClientWallet` (modify — add `script_type` field + new `sign_bip322` method + `script_type()` accessor) | wallet / signer | request-response (sign → witness) | `client/src/wallet.rs::sign_psbt_input` (lines 248-288 — the existing PSBT-sign pattern with `trust_witness_utxo: true` + dual-path witness extraction) | exact (same wallet type, same bdk PSBT path, same witness-extraction shape) |
| `client/src/wallet.rs::BdkClientWallet::generate/from_descriptor` (modify — per-type descriptor templates + construction-time mismatch check) | wallet / constructor | request-response (descriptor → Wallet) | `client/src/wallet.rs::from_descriptor` (lines 75-110) + `generate` (lines 117-214) | exact (extends existing constructors) |
| `client/src/discover.rs` (modify — add `CoordinatorCapabilities`, `DiscoveryError`, extend `discover_coordinator` signature, replace `parse_onion_from_rr` with `parse_blindjoin_record`) | discovery / resolver | request-response (PKARR query → typed result) | `client/src/discover.rs::discover_coordinator` + `parse_onion_from_rr` (lines 18-81) | exact (extends existing resolver in-place) |
| `client/src/round/input.rs::register_input` (modify — replace `generate_bip322_witness` call site; add v1/v2 envelope branch + `build_v2_psbt_input_b64` helper; DELETE `generate_bip322_witness`) | round handler | request-response (BIP-322 sign → POST /round/input) | `client/src/round/input.rs::register_input` (lines 18-108) + existing v=1 envelope construction (lines 69-75) | exact (extends in-place) |
| `client/src/main.rs` (modify — pass `cfg.script_type` into wallet + `discover_coordinator`; map `DiscoveryError` to anyhow; log WARN on legacy coordinator) | wiring / entry-point | request-response | `client/src/main.rs` (existing PKARR call site at line 58; existing wallet construction at lines 44-54) | exact (in-place edits at known anchor lines) |
| **NEW** `tests/integration/multi_script_client.rs` | integration test | end-to-end (bitcoind + sign-roundtrip + stubbed coordinator HTTP) | `tests/integration/multi_script_validate.rs` (full file — Phase 16-02 LANDED) + `tests/integration/full_round.rs` lines 1-100 (v1.3 invariant gate header pattern) | exact (multi_script_validate is the parallel sibling — coordinator-side, with the same fixture set + macro pattern + matches!() discipline) |
| **NEW** `client/tests/wallet_sign_roundtrip.rs` (new file — also requires creating `client/tests/` directory) | unit test (integration-style, no bitcoind) | request-response (in-memory sign → verify) | No direct analog in `client/tests/` (directory does not exist yet). Closest pattern: `coordinator/src/bitcoin/utxo.rs` `#[cfg(test)] mod tests` (lines 227+) which builds wallets in-memory and verifies via `shared::bip322::verify_simple`. | role-match (test of `wallet.sign_bip322` against `shared::bip322::verify_simple` — no bitcoind, no HTTP) |

---

## Pattern Assignments

### 1. `client/src/config.rs` — add `--type` flag (WALLET-01, D-57, CD-22)

**Analog:** `client/src/config.rs::ClientConfig` (whole file — extend in place)

**Existing CLI-arg pattern** (lines 7-12, 23-29, 56-58 — established convention):
```rust
#[arg(long, env = "BLINDJOIN_COORDINATOR_URL", default_value = "http://127.0.0.1:8080")]
pub coordinator_url: String,

#[arg(long, env = "BLINDJOIN_UTXO_WIF")]
pub utxo_wif: Option<String>,

#[arg(long, env = "BLINDJOIN_DESCRIPTOR")]
pub descriptor: Option<String>,

#[arg(long, env = "BLINDJOIN_PKARR_PUBKEY")]
pub pkarr_pubkey: Option<String>,
```

**Pattern to copy:** Single-underscore `BLINDJOIN_*` env-var naming (CD-22 — NOT the coordinator's double-underscore `BLINDJOIN__*`); `default_value` as a string literal; one field per flag with doc-comment above. clap derive macros at top of file: `use clap::Parser; #[derive(Parser, Debug, Clone)]`.

**New field shape (D-57 + value_parser per RESEARCH §"CLI flag parser"):**
```rust
/// Script type for wallet descriptor generation. Selects BIP-84 (p2wpkh),
/// BIP-86 (p2tr), or BIP-49 (p2sh-p2wpkh). Default p2wpkh for v1.3 backwards
/// compatibility — existing wallets continue working unchanged.
#[arg(long = "type", env = "BLINDJOIN_SCRIPT_TYPE", default_value = "p2wpkh", value_parser = parse_script_type)]
pub script_type: shared::bip322::ScriptType,
```

**`parse_script_type` helper** (RESEARCH §"CLI flag parser" lines 532-541 — drop into `client/src/config.rs` body):
```rust
fn parse_script_type(s: &str) -> Result<shared::bip322::ScriptType, String> {
    // Wrap string in JSON quotes so serde_json::from_str fires the enum's serde impl
    // (#[serde(rename_all = "snake_case")] + #[serde(rename = "p2sh-p2wpkh")] per Phase 15 D-Q3)
    let quoted = format!("\"{}\"", s);
    serde_json::from_str::<shared::bip322::ScriptType>(&quoted)
        .map_err(|e| format!("invalid --type value '{s}': expected p2wpkh, p2tr, or p2sh-p2wpkh ({e})"))
}
```

---

### 2. `client/src/wallet.rs` — `script_type` field + `sign_bip322` method (WALLET-01 + WALLET-02, D-62..D-66)

**Analog:** `client/src/wallet.rs::sign_psbt_input` (lines 248-288 — the existing PSBT-sign pattern is mirrored exactly for the new `sign_bip322` body)

**Struct shape** (lines 17-26 — extend with new field at construction-time):
```rust
pub struct BdkClientWallet {
    #[allow(dead_code)]
    pub network: Network,
    pub utxo_outpoint: OutPoint,
    /// The P2WPKH script_pubkey controlling the UTXO (needed for BIP-322 and PSBT signing).
    utxo_script_pubkey: ScriptBuf,
    /// The WIF key string, stored for secret_key_for_signing (WIF wallets only).
    wif_key: Option<String>,
    inner: Wallet,
}
```

**Pattern to copy:** `script_type: ScriptType` field is set ONCE in each constructor (`from_wif`: hardcoded `ScriptType::P2wpkh` per D-61; `from_descriptor`: derived from `--type` flag with mismatch check per D-63; `generate`: from `--type` flag per D-60). The accessor pattern matches existing `pub fn script_pubkey(&self) -> ScriptBuf` at lines 230-232:
```rust
pub fn script_type(&self) -> shared::bip322::ScriptType {
    self.script_type
}
```

**Existing `bdk_wallet::Wallet::create(...)` constructor pattern** (lines 90-93 — works UNCHANGED for `tr(...)` and `sh(wpkh(...))` descriptors per D-58):
```rust
let bdk_net = bdk_network(network);
let inner = Wallet::create(external_desc.to_string(), internal_desc)
    .network(bdk_net)
    .create_wallet_no_persist()
    .map_err(|e| anyhow!("Failed to create bdk wallet from descriptor: {e}"))?;
```

**Existing `generate` literal-descriptor template pattern** (lines 140-141 — extend with per-type branch per D-58; coin=0' across ALL networks per D-66 to preserve v1.3 byte-equivalence):
```rust
// v1.3 path — keep coin=0' literal across networks per D-66:
let external_desc = format!("wpkh({}/84'/0'/0'/0/*)", xprv);
let internal_desc = format!("wpkh({}/84'/0'/0'/1/*)", xprv);
```

**WARNING anti-pattern (RESEARCH Pitfall 2):** Do NOT switch to `bdk_wallet::template::Bip84/Bip86/Bip49` — those auto-select coin=1' on testnet/signet and break v1.3 byte-equivalence. Use literal `format!()` per D-58.

**Existing `sign_psbt_input` body (lines 248-288 — the load-bearing template for `sign_bip322`):**
```rust
pub fn sign_psbt_input(&self, psbt: &mut Psbt) -> Result<Vec<u8>> {
    let input_idx = psbt.unsigned_tx.input.iter()
        .position(|inp| inp.previous_output == self.utxo_outpoint)
        .ok_or_else(|| anyhow!("Our UTXO not found in PSBT"))?;

    // trust_witness_utxo: true is required because we sign over a segwit witness_utxo
    // without populating non_witness_utxo (Phase 12 lesson #1 — load-bearing).
    #[allow(deprecated)]
    self.inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..SignOptions::default() })
        .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;

    // Dual-path extraction: prefer final_script_witness, fall back to partial_sigs
    let input = &psbt.inputs[input_idx];
    if let Some(witness) = &input.final_script_witness {
        return Ok(bitcoin::consensus::serialize(witness));
    }
    if let Some((pk, sig)) = input.partial_sigs.iter().next() {
        let mut witness = bitcoin::Witness::new();
        witness.push(sig.to_vec());
        witness.push(pk.to_bytes());
        return Ok(bitcoin::consensus::serialize(&witness));
    }

    Err(anyhow!("bdk_wallet did not produce a witness for our input"))
}
```

**Pattern to copy for `sign_bip322`** (mirror exactly: `#[allow(deprecated)] self.inner.sign(&mut psbt, SignOptions { trust_witness_utxo: true, ..Default::default() })` + dual-path witness extraction — replace `partial_sigs` fallback with `tap_key_sig` for P2TR per Sprint-0-B finding; add `final_script_sig` extraction for P2SH-P2WPKH per D-65):

The full `Bip322SignedProof` struct + `sign_bip322` body lives at CONTEXT D-64 / D-65 + RESEARCH §"Pattern 1: BIP-322 PSBT-shaped Sign" lines 296-341. The structural mirror of `sign_psbt_input` (existing PSBT-sign pattern) provides the imports (`SignOptions`, `bitcoin::Psbt`), the deprecation marker (`#[allow(deprecated)]`), the trust_witness_utxo discipline, and the dual-path extraction idiom. The only NEW pieces are:
- The BIP-322 PSBT envelope (`Psbt::from_unsigned_tx(to_sign)` + populate `witness_utxo` with `Amount::ZERO` + the BIP-322 `to_spend` SPK — per `shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign}` already used at `client/src/round/input.rs` line 9).
- Per-script witness extraction (P2TR adds `tap_key_sig` fallback; P2SH-P2WPKH adds `final_script_sig.clone()` extraction per RESEARCH Pitfall 7).

---

### 3. `client/src/discover.rs` — extended resolver (WALLET-03 + WALLET-04, D-71..D-76)

**Analog:** `client/src/discover.rs` (whole file — extend in place; existing `parse_onion_from_rr` grows into `parse_blindjoin_record`)

**Existing struct + PKARR resolve pattern** (lines 5-60 — extend `CoordinatorInfo` with capabilities; keep the `Client::builder().build()` + `resolve_most_recent` + `packet.resource_records("_blindjoin")` chain):
```rust
#[derive(Debug)]
pub struct CoordinatorInfo {
    pub coordinator_url: String,
}

pub async fn discover_coordinator(pkarr_pubkey: &str) -> Result<CoordinatorInfo> {
    let public_key: PublicKey = pkarr_pubkey
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid PKARR public key '{pkarr_pubkey}': {e}"))?;

    let client = Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build PKARR client: {e}"))?;

    let packet = client
        .resolve_most_recent(&public_key)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!("Coordinator not found in DHT for key '{pkarr_pubkey}'")
        })?;

    let coordinator_addr = packet
        .resource_records("_blindjoin")
        .find_map(|rr| parse_onion_from_rr(rr))
        .ok_or_else(|| { ... })?;

    // ... URL construction ...
    Ok(CoordinatorInfo { coordinator_url })
}
```

**Existing PKARR TXT decode pattern** (lines 67-81 — the inline `Partial` struct grows into `BlindjoinRecord`):
```rust
fn parse_onion_from_rr(rr: &pkarr::dns::ResourceRecord<'_>) -> Option<String> {
    use pkarr::dns::rdata::RData;
    let txt = match &rr.rdata {
        RData::TXT(txt) => txt,
        _ => return None,
    };
    let s = String::try_from(txt.clone()).ok()?;
    #[derive(Deserialize)]
    struct Partial {
        onion: Option<String>,
    }
    serde_json::from_str::<Partial>(&s).ok()?.onion
}
```

**Pattern to copy:** The `RData::TXT(txt) => txt` match, the `String::try_from(txt.clone())` decode, the `serde_json::from_str::<...>(&s)` parse, and the inline `#[derive(Deserialize)]` struct. Phase 17 extends `Partial { onion }` → `BlindjoinRecord { v, onion, sst, ost }` with the `#[serde(rename = "v", default = "default_legacy_version")]` annotation per RESEARCH Pitfall 5 (Phase 16-03 B3 rename — the wire field is `v`, NOT `version`).

**Cross-source-of-truth (the wire shape Phase 17 decodes against):** `coordinator/src/discovery/pkarr_pub.rs::build_coordinator_packet` lines 89-108:
```rust
let record = serde_json::json!({
    "type": "blindjoin-coordinator",
    "v": "0.2.0",
    "onion": coordinator_addr,
    "n": "signet",
    "ds": denomination_sats,
    "mp": min_participants,
    "st": status,
    "sst": supported.join(","),     // alphabetical CSV: "p2sh-p2wpkh,p2tr,p2wpkh"
    "ost": output_script_type,      // kebab-case scalar: "p2wpkh"
});
```
Phase 17's `BlindjoinRecord` struct MUST mirror these field names verbatim (compact `v`/`sst`/`ost`).

**Existing typed-error pattern in client** (analog: `coordinator/src/bitcoin/utxo.rs::UtxoError` at lines 17-29 — same `thiserror` discipline):
```rust
#[derive(Debug, thiserror::Error)]
pub enum UtxoError {
    #[error("UTXO not found or already spent")]
    NotFound,
    #[error("UTXO already registered in this round")]
    AlreadyRegistered,
    #[error("Invalid BIP-322 ownership proof: {reason}")]
    InvalidProof { reason: String },
    ...
}
```

**Pattern to copy:** `#[derive(Debug, thiserror::Error)]` + per-variant `#[error("...")]` with named-struct payloads. Phase 17's `DiscoveryError` (D-72 + CD-23) follows this exact convention. The variants + format strings are LOCKED in RESEARCH §"Discovery layer extension" lines 572-594.

**Existing pre-Tor placement** (`client/src/main.rs` lines 57-69 — the structural ordering proof per D-74):
```rust
// CLI-01: If --pkarr-pubkey is provided, resolve coordinator URL from DHT.
let coordinator_url = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
    let info = discover::discover_coordinator(pkarr_key)        // line 58 — runs FIRST
        .await
        .map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))?;
    info.coordinator_url
} else {
    cfg.coordinator_url.clone()
};
// CLI-05: when --tor is set, use two isolated Tor circuits...
let client = if cfg.use_tor {
    let handle = tor::init_tor(coordinator_url.clone()).await   // line 68 — runs AFTER
        ...
};
```
The fail-fast in `discover_coordinator` returns `Err` BEFORE the `if cfg.use_tor` branch is even evaluated. Add an inline comment per RESEARCH Pitfall 4: `// WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural ordering, not a runtime hack.`

---

### 4. `client/src/round/input.rs` — v1/v2 envelope branch (WALLET-02 + WALLET-04 encoder, D-68..D-70 + CD-20)

**Analog:** `client/src/round/input.rs::register_input` (lines 18-108 — extend in place) + existing v=1 envelope construction at lines 69-75

**Existing register_input flow** (lines 53-75 — the swap site is the `generate_bip322_witness(wallet, &bip322_message)?` call at line 63 and the OwnershipProof construction at lines 69-75):
```rust
// 4. Generate BIP-322 ownership proof for the UTXO
let round_id_str = info.round_id
    .map(|id| id.to_string())
    .unwrap_or_default();
let bip322_message = format!(
    "blindjoin:round:{}:utxo:{}:{}",
    round_id_str,
    wallet.utxo_outpoint.txid,
    wallet.utxo_outpoint.vout,
);
let witness_stack = generate_bip322_witness(wallet, &bip322_message)?;
// v1.4 Phase 15 Plan 15-01: OwnershipProof evolved to the v2 four-field flat
// envelope (CONTEXT D-22). The v1 path stays bit-exact on the wire because
// to_json_hex_str's CD-7 branch emits the v1.3 array-of-hex form when
// (version == 1 && psbt_input_b64.is_none() && script_type.is_none()).
// Phase 17 WALLET-02 swaps this to descriptor-aware v2 construction.
let ownership_proof_obj = shared::protocol::OwnershipProof {
    version: 1,
    witness_stack,
    psbt_input_b64: None,
    script_type: None,
};
let ownership_proof = ownership_proof_obj.to_json_hex_str();
```

**Pattern to copy:** Replace `generate_bip322_witness(wallet, &bip322_message)?` (line 63) with `wallet.sign_bip322(&bip322_message)?` (returns `Bip322SignedProof`). The existing message format string (`"blindjoin:round:{}:utxo:{}:{}"`) is UNCHANGED (matches coordinator dispatcher at `coordinator/src/bitcoin/utxo.rs:96`). The OwnershipProof construction (lines 69-75) becomes the v1/v2 branch per CONTEXT D-68 + RESEARCH §"v1/v2 envelope branch" lines 610-642.

**WARNING — RESEARCH Pitfall 1:** CONTEXT D-69's "`bitcoin::consensus::serialize(&psbt::Input)`" is wire-shape WRONG. The encoder MUST mirror `tests/integration/multi_script_validate.rs::build_v2_psbt_input_b64` (lines 56-74) which builds a FULL PSBT (`Psbt::from_unsigned_tx(unsigned_tx)`) and emits `B64.encode(psbt.serialize())`. The coordinator's decoder at `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` (lines 212-225) reads `Psbt::deserialize` — a full PSBT, not a bare Input. The helper signature should be `fn build_v2_psbt_input_b64(witness: &Witness, final_script_sig: Option<&ScriptBuf>) -> Result<String>`, NOT `-> Result<bitcoin::psbt::Input>`.

**Canonical v=2 encoder shape (mirror verbatim from `tests/integration/multi_script_validate.rs:56-74`):**
```rust
fn build_v2_psbt_input_b64(
    witness: &Witness,
    final_script_sig: Option<&ScriptBuf>,  // Some for P2SH-P2WPKH; None for P2WPKH/P2TR
) -> anyhow::Result<String> {
    use bitcoin::psbt::Psbt;
    use bitcoin::{absolute, transaction, OutPoint, ScriptBuf, Sequence, Transaction, TxIn};

    let unsigned_tx = Transaction {
        version: transaction::Version(2),
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ZERO,
            witness: Witness::new(),
        }],
        output: vec![],
    };
    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)?;
    psbt.inputs[0].final_script_witness = Some(witness.clone());
    if let Some(sig) = final_script_sig {
        psbt.inputs[0].final_script_sig = Some(sig.clone());
    }
    Ok(B64.encode(psbt.serialize()))
}
```

**CRIT-01 client-side discipline pattern (D-80 — symmetric with coordinator-side at `coordinator/src/bitcoin/utxo.rs:182`):**
```rust
shared::protocol::OwnershipProof {
    version: 2,
    witness_stack: signed.witness_stack,   // populated for symmetry per D-70
    psbt_input_b64: Some(psbt_input_b64),
    // CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo
    script_type: Some(signed.script_type),
}
```
The coordinator-side parallel comment is at `coordinator/src/bitcoin/utxo.rs:161 / 182`:
```rust
// CRIT-01: script_type derived from on-chain script_pubkey, never from client field
let derived = detect_script_type(script_pubkey)?;
```

**DELETE per CD-20:** Remove `fn generate_bip322_witness(wallet: &ClientWallet, message: &str) -> Result<Vec<Vec<u8>>>` at lines 115-149. This is the v1.3 hand-rolled P2WPKH BIP-322 sign — superseded by `wallet.sign_bip322(...)` which dispatches per script type.

---

### 5. `client/src/main.rs` — wiring (D-74 + CD-21)

**Analog:** `client/src/main.rs` (whole file — in-place edits at known anchor lines)

**Existing wallet construction wire-through pattern** (lines 44-54):
```rust
let wallet = if let Some(descriptor) = cfg.descriptor.as_deref() {
    let utxo_address = cfg.utxo_address.as_deref()
        .ok_or_else(|| anyhow::anyhow!("--utxo-address is required when using --descriptor"))?;
    ClientWallet::from_descriptor(descriptor, utxo, utxo_address, network)?
} else {
    let wif = cfg.utxo_wif.as_deref()
        .ok_or_else(|| anyhow::anyhow!("--utxo-wif is required when not using --descriptor or --generate-wallet"))?;
    ClientWallet::from_wif(wif, utxo, network)?
};
```

**Pattern to copy:** Pass `cfg.script_type` as a new argument to `ClientWallet::generate(utxo, network, cfg.script_type)?` (line 33) and `ClientWallet::from_descriptor(descriptor, utxo, utxo_address, network, cfg.script_type)?` (line 48). `from_wif` per D-61 stays single-script — keep the existing call site unchanged; the constructor internally asserts `ScriptType::P2wpkh` and ignores `cfg.script_type`.

**Existing PKARR call-site pattern** (lines 57-64 — extend with `cfg.script_type` arg + error mapping):
```rust
let coordinator_url = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
    let info = discover::discover_coordinator(pkarr_key)
        .await
        .map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))?;
    info.coordinator_url
} else {
    cfg.coordinator_url.clone()
};
```

**Pattern to copy:** Same `if let Some(ref pkarr_key)` branch; add `cfg.script_type` as a second argument; map the typed `DiscoveryError` via `.map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))`. Add the WARN log per CD-21 immediately after the Ok branch:
```rust
let info = discover::discover_coordinator(pkarr_key, cfg.script_type)
    .await
    .map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))?;
if info.capabilities.is_legacy {
    tracing::warn!(
        coordinator_pubkey = %pkarr_key,
        record_version = %info.capabilities.record_version,
        "Detected legacy v1.3 coordinator — using v1 OwnershipProof shim (WALLET-04)"
    );
}
```

**Existing tracing-subscriber init pattern** (lines 17-20 — unchanged; carries `tracing::warn!` correctly):
```rust
tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("client=info".parse().unwrap()))
    .init();
```

---

### 6. NEW `tests/integration/multi_script_client.rs` — Phase 17 acceptance gate (D-78)

**Analog:** `tests/integration/multi_script_validate.rs` (Phase 16-02 LANDED, full file — the parallel coordinator-side test)

This is the closest analog because it (a) tests the same v=1/v=2 envelope shapes from the OTHER end of the wire, (b) uses the SAME `BitcoindGuard` + `require_bitcoind!()` + `fund_regtest_typed` fixtures, (c) follows the SAME `matches!(...)` discipline for error-variant assertions per Phase 15-03 D-34.

**Header / module attribute pattern** (lines 1-28):
```rust
//! v1.4 Phase 16 Plan 16-02 Task 3 — 9 D-54 verbatim test cases covering the
//! multi-script `validate_utxo` dispatcher + CRIT-01 cross-check + allowlist
//! gate + envelope-shape edge cases.
//!
//! Reuses the shared regtest fixtures from `tests/integration/mod.rs`:
//!   - `BitcoindGuard` (RAII bitcoind ownership)
//!   - `require_bitcoind!()` (graceful-skip without bitcoind)
//!   - `fund_regtest_typed(...)` + `TypedUtxoHandle` (per-script UTXOs)

#![allow(clippy::needless_borrows_for_generic_args)]

use base64::Engine;
use bitcoin::{Network, Witness};
use coordinator::bitcoin::utxo::validate_ownership_proof_typed;
use coordinator::config::BipConfig;
use shared::bip322::{sign_simple_test_only, Bip322Error, ScriptType};
use shared::protocol::OwnershipProof;

use crate::{fund_regtest_typed, require_bitcoind, TypedUtxoHandle};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
```

**Pattern to copy:**
- Module-level doc-comment naming the plan (Phase 17 17-02 / 17-03) and the test scope (D-78).
- `#![allow(clippy::needless_borrows_for_generic_args)]` if needed (carries from sibling).
- `use crate::{fund_regtest_typed, require_bitcoind, TypedUtxoHandle};` — the THREE fixtures that come for free from `tests/integration/mod.rs`.
- `const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;` (top-of-file convenience).

**Per-test fixture pattern** (lines 131-158 — exact mirror for Phase 17 bitcoind-backed tests):
```rust
#[tokio::test]
async fn validate_p2wpkh_utxo_with_v1_legacy_proof_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    let handle = &setup.utxos[0];

    let round_id = unique_round_id("v1-p2wpkh-ok");
    let message = dispatcher_message(&round_id, handle);
    let witness = sign_witness(handle, message.as_bytes());

    // v=1 envelope: array-of-witness-items, no psbt or script_type.
    let witness_items: Vec<Vec<u8>> = witness.iter().map(|s| s.to_vec()).collect();
    let proof = build_v1_proof(witness_items);

    let cfg = default_bip_config();
    let result = validate_ownership_proof_typed(
        handle.script_pubkey.as_script(),
        &proof,
        Network::Regtest,
        &cfg,
        message.as_bytes(),
    );
    assert!(result.is_ok(), "v=1 legacy P2WPKH must verify: {result:?}");
    assert_eq!(result.unwrap(), ScriptType::P2wpkh);
}
```

**Pattern to copy:**
- `#[tokio::test]` macro on every async test (NOT `#[test]`).
- `let exe = require_bitcoind!();` as the FIRST line — graceful-skip if bitcoind unavailable.
- `let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;` — `_guard` MUST be bound to a local (drops at end-of-scope → kills bitcoind; never use `_` lone underscore which drops immediately).
- One bitcoind per test fn (matches existing isolation pattern).
- `unique_round_id("test-tag")` (lines 98-104) — per-test unique message strings defensive against any future regtest-bitcoind reuse.

**Matches! discipline for error-variant assertions** (lines 268-276 — Phase 15-03 D-34):
```rust
assert!(
    matches!(
        err,
        Bip322Error::ScriptTypeMismatch {
            declared: ScriptType::P2tr,
            derived: ScriptType::P2wpkh,
        }
    ),
    "expected ScriptTypeMismatch {{ P2tr, P2wpkh }}, got: {err:?}"
);
```

**Pattern to copy:** `assert!(matches!(err, ErrEnum::Variant { ... }), "expected ..., got: {err:?}")` — never string-parsing the error message; assert structurally on the enum variant + payload. Phase 17 uses this for `DiscoveryError::UnsupportedScriptType { required, supported, ... }` matching.

**Phase 17-specific test names per D-78** (the 9 tests this file must contain — listed in CONTEXT §"D. F. Plan ordering" + RESEARCH §"Specific Ideas"):
1. `generate_p2wpkh_wallet_emits_bip84_descriptor`
2. `generate_p2tr_wallet_emits_bip86_descriptor`
3. `generate_p2sh_p2wpkh_wallet_emits_bip49_descriptor`
4. `p2wpkh_sign_roundtrip_verifies` (no bitcoind — can live in `client/tests/wallet_sign_roundtrip.rs`)
5. `p2tr_sign_roundtrip_verifies` (no bitcoind)
6. `p2sh_p2wpkh_sign_roundtrip_verifies` (no bitcoind — ALSO asserts `signed.final_script_sig.is_some()`)
7. `v13_pkarr_record_with_p2tr_wallet_rejects_before_tor`
8. `v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope`
9. `v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope`

**Cross-phase invariant:** `tests/integration/full_round.rs` (file lines 1-100) MUST remain UNCHANGED. Phase 17 does NOT modify it. The header comment shape at lines 1-12 (`//! Integration test: ...`, threat-model compliance notes, `require_bitcoind!()` usage) is the template for the new `multi_script_client.rs`.

---

### 7. NEW `client/tests/wallet_sign_roundtrip.rs` — unit-test-style sign↔verify roundtrips (D-77 / 17-02 test scope)

**Analog:** No direct in-tree `client/tests/*.rs` exists yet (`client/tests/` directory is absent — Phase 17 plan must create it). Closest pattern: `coordinator/src/bitcoin/utxo.rs::mod tests` (lines 227+) which builds wallets in-memory and verifies via `shared::bip322::verify_simple`. The `client/src/discover.rs::mod tests` block at lines 83-102 also shows the `#[tokio::test]` + `cfg(test)` shape currently in the client crate.

**Scaffold (mirror from `multi_script_validate.rs` header, MINUS the bitcoind fixtures, PLUS in-memory wallet construction):**
```rust
//! Phase 17 17-02 D-77 — per-script BIP-322 sign↔verify roundtrips.
//!
//! Pure-in-memory tests with NO bitcoind dependency. For each script type,
//! constructs a wallet via the production constructor (BdkClientWallet::generate
//! or ::from_descriptor), derives the wallet's UTXO script_pubkey via
//! peek_address(External, 0).script_pubkey(), calls wallet.sign_bip322("test-message"),
//! and feeds the resulting witness to shared::bip322::verify_simple. Asserts
//! Ok(()) for all 3 script types. P2SH-P2WPKH additionally asserts
//! signed.final_script_sig.is_some() (RESEARCH Pitfall 7).
//!
//! Reuses NO test fixtures — pure shared::bip322 + client::wallet API surface.

use bitcoin::Network;
use client::wallet::BdkClientWallet;  // assumes client crate exposes BdkClientWallet
use shared::bip322::{verify_simple, ScriptType};

#[tokio::test]
async fn p2wpkh_sign_roundtrip_verifies() {
    // 1. Generate a P2WPKH wallet
    // 2. Derive utxo_script_pubkey from peek_address(External, 0)
    // 3. wallet.sign_bip322("test-message")
    // 4. verify_simple(ScriptType::P2wpkh, &spk, &signed.witness, b"test-message", Network::Signet)
    // 5. Assert Ok(())
}
```

**Pattern to copy:** Per-script test fn naming (`<script>_sign_roundtrip_verifies`); `verify_simple` is the LOCKED Phase 15 API at `shared/src/bip322/mod.rs:242-254` — Phase 17 imports it directly (already in tree). The wallet construction goes through the PRODUCTION constructor (`BdkClientWallet::generate(utxo, network, script_type)` — note the new `script_type` arg per Phase 17 17-01). The placeholder `utxo` outpoint can be the zero-txid pattern at `client/src/main.rs:32`: `"0000000000000000000000000000000000000000000000000000000000000000:0"`.

**Crate exposure pattern:** `client/tests/*.rs` files run as EXTERNAL integration-test crates (per `cargo test` semantics for files in `client/tests/`), so `BdkClientWallet` must be `pub` in `client::wallet`. Already true at `client/src/wallet.rs:17`.

**Cargo wiring:** Since `client/tests/` doesn't exist yet, the plan's first action in 17-02 must `mkdir client/tests` then `cargo test -p client --test wallet_sign_roundtrip` will pick up the new file automatically (cargo convention).

---

## Shared Patterns (Cross-Cutting)

### A. `#[allow(deprecated)] inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..Default::default() })`

**Source:** `client/src/wallet.rs:268-270` (existing `sign_psbt_input`)
**Apply to:** `client::wallet::sign_bip322` body (the new P2TR + P2SH-P2WPKH + descriptor-P2WPKH bdk paths)

**Carries forward Phase 12 lesson #1** — when signing over a witness_utxo without `non_witness_utxo`, MUST set `trust_witness_utxo: true`. The BIP-322 to_spend tx has `value: Amount::ZERO` so the "malicious coordinator lies about value" reasoning at `client/src/wallet.rs:259-267` carries (BIP-322 is off-chain — no value to lie about). The `#[allow(deprecated)]` marker stays until v1.5+ PSBT-signer migration.

### B. `thiserror`-derived typed error enum

**Source:** `coordinator/src/bitcoin/utxo.rs::UtxoError` lines 17-29
**Apply to:** `client::discover::DiscoveryError` (Phase 17 17-03)

**Pattern:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("...{field}...")]
    Variant { field: Type },
    ...
}
```
At the `main.rs` boundary, convert via `.map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))` — the Display impl from `thiserror` becomes the user-facing message.

### C. PII-free error messages (PROJECT.md constraint)

**Source:** `coordinator/src/bitcoin/utxo.rs::UtxoError` (no IP, no key, no wallet identifier in any variant)
**Apply to:** ALL Phase 17 error variants (`DiscoveryError`, `Bip322SignedProof` error paths)

`DiscoveryError::UnsupportedScriptType` names the coordinator pubkey (z32 string, PUBLIC DHT data) + the missing `ScriptType` enum value ONLY. Never the user's wallet, IP, UTXO outpoint, or amount. Symmetric with `coordinator/src/bitcoin/utxo.rs::UtxoError` which never leaks per-input identifiers (PRIV-02).

### D. CRIT-01 in-line comment + grep CI gate

**Source:** `coordinator/src/bitcoin/utxo.rs:161 / 182` (the dispatcher's CRIT-01 comment above `detect_script_type(script_pubkey)`)
**Apply to:** `client/src/round/input.rs` v2 envelope construction (above `script_type: Some(signed.script_type)`)

**Pattern:**
```rust
// CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo
script_type: Some(signed.script_type),
```
CI gate: `grep -c "CRIT-01" client/src/round/input.rs >= 1` per D-80 (symmetric with Phase 16's coordinator-side gate).

### E. PKARR resolver → main.rs ordering (D-74)

**Source:** `client/src/main.rs` lines 57-69 (existing structural ordering)
**Apply to:** `client::main` Phase 17 wiring

The pre-Tor fail-fast is STRUCTURAL — `discover_coordinator` runs UNCONDITIONALLY at the top of main.rs's coordinator-resolution branch (line 58); `tor::init_tor` only runs inside `if cfg.use_tor` at line 68. Phase 17 inherits this — the only edit is adding `cfg.script_type` as a second arg + the WARN log per CD-21. Document with inline comment per RESEARCH Pitfall 4: `// WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural ordering, not a runtime hack.`

### F. `tracing::warn!` with structured fields, no PII

**Source:** `coordinator/src/discovery/pkarr_pub.rs:43-46` (existing structured-field log) + `coordinator/src/bitcoin/utxo.rs:109-113` (success log)
**Apply to:** `client::main` legacy-coordinator detection log (CD-21)

**Pattern:**
```rust
tracing::warn!(
    coordinator_pubkey = %pkarr_key,
    record_version = %info.capabilities.record_version,
    "Detected legacy v1.3 coordinator — using v1 OwnershipProof shim (WALLET-04)"
);
```
Use `tracing::warn!` (NOT `eprintln!`) so the message respects the existing `RUST_LOG` / `EnvFilter` discipline at `client/src/main.rs:17-20`.

### G. `tokio::test` async-test pattern

**Source:** `client/src/discover.rs:87-96` (the only existing `#[tokio::test]` in the client crate)
**Apply to:** All new tests in `tests/integration/multi_script_client.rs` and `client/tests/wallet_sign_roundtrip.rs`

Every Phase 17 test MUST use `#[tokio::test]` (NOT `#[test]`) because:
- `discover_coordinator` is async (PKARR resolution).
- `wallet.sign_bip322` is sync but tests that call HTTP-stubbed register_input via `client::http::CoordinatorClient` are async.
- Symmetric with `multi_script_validate.rs` which uses `#[tokio::test]` throughout.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | — | — | Every Phase 17 file has a strong in-tree analog. The closest "no analog" case is `client/tests/wallet_sign_roundtrip.rs` — the `client/tests/` directory does not yet exist, but the test-file shape is mechanically derivable from `tests/integration/multi_script_validate.rs` (minus bitcoind fixtures) + `client/src/discover.rs::mod tests` (for the `#[tokio::test]` pattern within the client crate). |

---

## Metadata

**Analog search scope:**
- `client/src/{config,discover,wallet,main}.rs` and `client/src/round/{input,sign}.rs`
- `coordinator/src/{config,bitcoin/utxo,discovery/pkarr_pub}.rs`
- `shared/src/{bip322/mod,protocol}.rs`
- `tests/integration/{full_round,multi_script_validate,mod}.rs`

**Files scanned:** 11 (all primary sources; vendored bdk_wallet 2.3.0 templates examined per RESEARCH Pitfall 2 but NOT used as analog — Phase 17 deliberately avoids bdk templates per D-58)

**Pattern extraction date:** 2026-05-30

**Cross-references for the planner:**
- CONTEXT D-57..D-80 are the LOCKED inputs that bind each pattern (e.g., D-65 binds §2's `sign_bip322` body; D-68 binds §4's v1/v2 branch; D-73 binds §3's `BlindjoinRecord` shape).
- RESEARCH Pitfall 1 (D-69 wire-shape correction) and Pitfall 5 (D-73 field-rename `version`→`v`) are LOAD-BEARING corrections to CONTEXT — the planner must override CONTEXT literals with RESEARCH's corrected forms per the file references at `coordinator/src/bitcoin/utxo.rs:212-225` and `coordinator/src/discovery/pkarr_pub.rs:98-108`.
- The CRIT-01 client-side discipline (D-80) parallels the coordinator-side CRIT-01 at `coordinator/src/bitcoin/utxo.rs:161 / 182` — Phase 17 17-02 plan MUST include both the inline comment AND the grep CI gate.
- The `tests/integration/multi_script_validate.rs:56-74` `build_v2_psbt_input_b64` helper is the CANONICAL wire-shape reference — Phase 17 17-02 may copy it verbatim (with the `final_script_sig` extension for P2SH-P2WPKH) into `client/src/round/input.rs` as a private helper.
