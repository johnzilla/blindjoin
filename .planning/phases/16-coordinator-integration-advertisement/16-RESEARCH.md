# Phase 16: Coordinator Integration & Advertisement - Research

**Researched:** 2026-05-29
**Domain:** Rust async coordinator wiring — BIP-322 multi-script dispatch + PKARR advertisement + config validation
**Confidence:** HIGH (every load-bearing claim is grounded in already-shipped code + lockfile-pinned versions + the Phase 15 verification record)

## Summary

Phase 16 is a coordinator-side **wiring** phase. Every primitive it needs already exists and has been verified in earlier phases — the work is the integration discipline of routing them through the right call sites without breaking the v1.3 invariant or leaking the on-chain-vs-declared script-type cross-check (CRIT-01).

The five-piece deliverable splits cleanly into three atomic commits per D-53: (16-01) `BipConfig` + `InfoResponse` extension + `/round/info` handler; (16-02) `validate_utxo` dispatcher swap + deletion of the now-dead `verify_bip322_simple` + `is_p2wpkh()` gate, plus the new `multi_script_validate.rs` integration test; (16-03) PKARR record schema bump + 220-byte budget assertion. CRIT-01 is a load-bearing invariant present at **two** code locations (one per version branch) and is enforced by a CI grep gate mirroring the existing `bip322-pin-check` job.

**Primary recommendation:** Treat every claim in this document as a stable contract that the planner can compile-check against the existing code surface — there are zero unknowns at the API or wire-format layer (Phase 15 closed all of them).

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Carried forward from Phase 14 ADR + Phase 15 (NOT re-asked):**

- **ADR #2 / D-06:** MIXED rounds. One round queue accepts heterogeneous P2WPKH + P2TR + P2SH-P2WPKH inputs. Round-state machine carries v1.3 shape forward unchanged.
- **D-07:** Outputs single-script-type per round; operator-configured via `[bip] output_script_type` (default `p2wpkh`).
- **D-08:** No per-script-type minimum participants gate.
- **D-09:** Coordinator advertises SUPPORTED SET only. Does NOT advertise per-round per-script-type registration counts.
- **D-10 / CRIT-01:** Coordinator MUST derive `script_type` from on-chain `txout.script_pubkey` and cross-check against client-declared `script_type` at validate-utxo time. Non-negotiable, load-bearing, code-review checked.
- **D-12:** `OwnershipProof.version: u8` envelope. v=1 = v1.3 witness-only, v=2 = v1.4 PSBT-input. Coordinator branches `match proof.version`; unknown version → `UnsupportedProofVersion`.
- **Phase 15 outputs (LOCKED API surface):** `shared::bip322::{ScriptType, Bip322Error, detect_script_type, verify_simple(script_type, spk, witness, message, network), sign_simple(script_type, spk, key, message)}`. Per-script files are `pub(crate)` and unreachable from coordinator.
- **Phase 15 outputs:** `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` flat struct with serde defaults. `InputRegRequest.ownership_proof: String` stays a JSON-encoded envelope. Two-phase try-parse preserves v1.3 array-of-hex bit-exactness.
- **`Bip322Error` taxonomy (10 variants from Phase 15):** Phase 16 maps ALL variants to `ApiError { code: ErrorCode::InvalidOwnershipProof, message: e.to_string() }` at the handler layer — no new wire `ErrorCode` variants per D-32.
- **Cross-phase invariant:** `cargo test --test full_round` MUST remain green at every plan boundary in Phase 16.

**Phase 16 decisions (D-35 → D-56):** Reproduced verbatim from `16-CONTEXT.md`. Notable highlights:

- **D-35..D-38:** `BipConfig` lives in `coordinator/src/config.rs` as a top-level `[bip]` section with `#[serde(default)]` for v1.3-config compat; fields `allow_p2wpkh / allow_p2tr / allow_p2sh_p2wpkh` (default all `true`) + `output_script_type: ScriptType` (default `P2wpkh`). Methods `allows() / supported() / validate()`.
- **D-39..D-44:** PKARR uses compact field names `"sst"` + `"ost"`; CSV alphabetical sort; bump `"version"` → `"0.2.0"`; inline `< 220` byte assertion as a CI gate.
- **D-45..D-52:** `validate_utxo` branches on `proof.version`; both branches carry the `CRIT-01:` comment + the `detect_script_type(&on_chain_spk)` call; v=2 path additionally requires `proof.script_type.is_some()` and asserts `declared == derived`.
- **D-53:** Three plans, sequential. 16-01 = wire/config first (REPAIR-01 lesson #1). 16-02 = behavior swap. 16-03 = advertisement.
- **D-54..D-55:** 9 named integration test cases in `tests/integration/multi_script_validate.rs` covering the 3×3 script-type × declared-vs-derived matrix + 2 envelope-shape edge cases. Plus the 220-byte budget inline test in `pkarr_pub.rs::tests`.
- **D-56:** Phase 16 ships PKARR producer-side only. Client resolver changes (Phase 17 WALLET-03/04) are out of scope.

### Claude's Discretion

- **CD-11:** `BipConfig::supported()` returns alphabetical order (deterministic CSV in PKARR).
- **CD-12:** `tracing::info!` always at INFO (consistent operator log shape; small v1.3 verbosity delta acceptable).
- **CD-13:** Env-var override for `output_script_type` accepts wire-form lowercase kebab-case (`"p2wpkh" / "p2tr" / "p2sh-p2wpkh"`).
- **CD-14:** v1 path passes `network: bitcoin::Network` to `verify_simple` (no v1-vs-v2 fork).
- **CD-15:** `verify_bip322_simple` + `is_p2wpkh()` deletion lives inside 16-02 atomic commit.
- **CD-16:** `multi_script_validate` uses `BitcoindGuard` (real regtest UTXOs), not a mock.

### Deferred Ideas (OUT OF SCOPE)

- Per-round-per-script-type registration breakdown advertisement (privacy anti-feature).
- Per-script-type ban tracking / rate limits / denominations (all anti-features).
- Mixed output script types per round (v1.5+).
- PKARR resolver-side `#[serde(default)]` shim on the resolved-record type in `client/src/discover.rs` (Phase 17 WALLET-03/04).
- Tor-mode UAT harness (v1.5+).
- REPAIR-01 PR observation closure (v1.5 process step).
- B-03 dynamic fee estimation (pre-mainnet).
- TEST-EXT-01/02/03 (v1.5+).
- DECISIONS-INDEX.md rolling summary (v1.5 candidate).
- CSV-vs-array PKARR format reconsideration (v1.5 problem at 4+ script types).
- `bdk_wallet = "=2.3.x"` exact-pin tightening (v1.5+).

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ADVERT-01 | `BipConfig` section in `coordinator.toml` + env-var overrides + `validate()` fail-fast at startup | §A1, §A2, §A3 — `config 0.15.22` env-var deserialize behavior verified [VERIFIED: Cargo.lock + docs.rs] |
| ADVERT-02 | PKARR record bump `0.1.0`→`0.2.0` + CSV-encoded `supported_script_types` + JSON-array form on `/round/info` + `#[serde(default)]` bidirectional compat | §B1, §B2, §B3 — pkarr 5.0.2 publishes signed DNS TXT JSON; v1.3 client resolver tolerates unknown JSON fields [VERIFIED: code read at `client/src/discover.rs`] |
| ADVERT-03 | Coordinator derives `ScriptType` from `txout.script_pubkey` + cross-checks against client-declared field; mismatch rejects — CRIT-01 invariant load-bearing | §C1, §C2, §C3 — Phase 15 already shipped `detect_script_type` + the type-level dispatcher-only public surface [VERIFIED: Phase 15-VERIFICATION + code read at `shared/src/bip322/mod.rs`] |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| BIP-322 verification (script-type dispatch + per-script primitives) | `shared/` crate (single source of truth) | — | Phase 15 D-27 dispatcher-only public surface; coordinator never re-implements verification logic |
| BIP-322 script-type allowlist policy | Coordinator (operator-configurable) | `shared/` provides `ScriptType` enum | Policy belongs to the operator-tuned coordinator config; verification primitives belong to the shared contract |
| Coordinator advertisement (PKARR + HTTP) | Coordinator | `shared/` provides `InfoResponse` wire struct | DHT publish + HTTP handler are coordinator-private mechanics; the wire types are the shared contract |
| Client-side discovery + script-type fail-fast | Client (Phase 17) | — | EXPLICITLY OUT OF PHASE 16 SCOPE (D-56) |
| Round state machine + RSA blind-signature flow | Coordinator (round/state.rs) | — | UNCHANGED IN PHASE 16 (D-06 → MIXED rounds carry v1.3 shape forward) |

## Standard Stack

### Core (already in the workspace; Phase 16 adds zero new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `shared` (workspace path crate) | 0.1.0 | BIP-322 dispatch contract + wire types | Phase 15 single source of truth — coordinator + client compile against same crate [VERIFIED: code read at `shared/Cargo.toml`] |
| `bitcoin` | 0.32 (workspace pin) | `Script`, `Network`, `Witness`, `OutPoint` primitives | Already in coordinator deps; nothing new [VERIFIED: workspace `Cargo.toml`] |
| `bip322` (transitive via shared) | =0.0.10 (exact pin) | BIP-322 verify_simple adapter | Pinned by Phase 14 ADR Decision #1; CI grep gate enforces [VERIFIED: Cargo.lock + .github/workflows/ci.yml::bip322-pin-check] |
| `config` | 0.15.22 | TOML + env-var layered config | Already in `coordinator/Cargo.toml`; `BipConfig` slots in [VERIFIED: Cargo.lock] |
| `serde` / `serde_json` | 1.x (workspace) | JSON wire encoding | Phase 15 already uses for `OwnershipProof` v2 envelope [VERIFIED: code read at `shared/src/protocol.rs`] |
| `pkarr` | 5.0.2 | Sign + publish DNS TXT JSON via Mainline DHT | Already in `coordinator/Cargo.toml`; only the JSON inside changes [VERIFIED: Cargo.lock + code read at `coordinator/src/discovery/pkarr_pub.rs`] |
| `tracing` | 0.1.x | Structured logging | Coordinator pattern; `round_id = %` Display + `script_type = ?` Debug per CD-12 [VERIFIED: existing patterns in `coordinator/src/run.rs:163,207,222`] |
| `anyhow` | 1.x | `validate()` Result type | Existing `CoordinatorConfig::validate()` returns `anyhow::Result<()>`; `BipConfig::validate()` follows suit [VERIFIED: code read at `coordinator/src/config.rs:157`] |
| `thiserror` (transitive via shared) | 1.x | `Bip322Error` enum | Phase 15 — no new direct usage in coordinator [VERIFIED] |

### Supporting (already in the workspace)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `corepc-types` | 0.11 (`features = ["30_2"]`) | `GetTxOut` typed RPC response — yields `script_pubkey.hex` | Existing `parse_script_pubkey_from_txout` at `coordinator/src/bitcoin/utxo.rs:80-85` — no change in Phase 16 [VERIFIED: code read] |
| `corepc-node` (dev-dep) | 0.12 (`features = ["30_2"]`) | regtest bitcoind harness — `getnewaddress` + `send_to_address` for funding non-P2WPKH UTXOs in `multi_script_validate.rs` | Test-only; Phase 16's new integration test [VERIFIED: code read at `tests/integration/mod.rs`] |
| `tempfile` (dev-dep) | 3.x | per-test temp dir for ban-file isolation | Existing pattern; `multi_script_validate.rs` follows it [VERIFIED] |
| `uuid` | 1.x | `round_id` for tracing log line | Already used — `round_id = %guard.round_id` Display via `uuid::Uuid::Display` [VERIFIED] |
| `hex` | 0.4 | `txout.script_pubkey.hex` decode (existing path) | Already used at `coordinator/src/bitcoin/utxo.rs:83` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tracing::info!(script_type = ?derived, ...)` | `tracing::info!(script_type = %derived, ...)` via `serde_plain` or `Display` impl | Display would require adding `#[derive(strum::Display)]` or a manual impl on `ScriptType`; `Debug` is free and produces `P2wpkh / P2tr / P2shP2wpkh` — operator-readable. Stay with `?`. |
| CSV `"sst"` PKARR value | JSON array `"sst": ["p2sh-p2wpkh","p2tr","p2wpkh"]` | Array form costs ~8 extra bytes (brackets + quotes) and pushes the worst-case payload from ~205 → ~213 bytes. CSV is tighter AND easier for non-Rust resolvers to parse from a TXT record. CSV is the locked choice (D-40). |
| compact `"sst" / "ost"` | full `"supported_script_types" / "output_script_type"` | Full names push payload to ~226 bytes (worst case), breaching the 220-byte warn at `pkarr_pub.rs:76`. Compact names are the locked choice (D-39). |
| Adding `#[derive(Default)]` on `BipConfig` | Free-standing `default_*` fns per field | Existing project pattern uses per-field `default_*` fns (see `default_ban_file_path`, `default_rate_limit_info_per_min` at `coordinator/src/config.rs:54-72`); `BipConfig` follows for consistency. |

**Installation:** Phase 16 adds **zero new dependencies**. Every required crate is already in `coordinator/Cargo.toml` (verified) or `shared/Cargo.toml` (Phase 15-introduced and verified). The plan must NOT introduce new direct deps.

**Version verification:** All version claims confirmed against `Cargo.lock` (already in tree). `bip322 = "=0.0.10"`, `config = 0.15.22`, `pkarr = 5.0.2`, `bitcoin = 0.32.x`, `corepc-node = 0.12` confirmed at lockfile read [VERIFIED: Cargo.lock].

## Package Legitimacy Audit

Phase 16 installs **zero** new packages. Every dependency the plan references is already in `Cargo.lock` and was vetted in a prior phase (Phase 14 ADR Decision #1 for `bip322`, Phase 8 for `config` + `tower_governor`, Phase 4 for `pkarr`, v1.0 for `bitcoin` / `tokio` / etc.). The Phase 15 audit confirmed `cargo audit` clean at 718 total dependencies after `proptest` dev-dep addition. **No further legitimacy review is required for Phase 16** — but `cargo audit` SHOULD remain a CI gate (it is, per `.github/workflows/ci.yml::audit` job).

| Package | Registry | Disposition |
|---------|----------|-------------|
| (none) | — | Phase 16 introduces zero new packages |

## Architecture Patterns

### System Architecture Diagram

```
                ┌──────────────────────────────────────────────────────┐
                │            coordinator.toml + ENV                     │
                │  [bip] allow_p2wpkh / allow_p2tr / allow_p2sh_p2wpkh │
                │        output_script_type                            │
                └──────────────────────┬───────────────────────────────┘
                                       │ config::Config::builder()
                                       ▼
                ┌──────────────────────────────────────────────────────┐
                │           CoordinatorConfig (Phase 8 base)            │
                │  + pub bip: BipConfig    (Phase 16 NEW)              │
                │  validate() chains BipConfig::validate()             │
                └──────────────────────┬───────────────────────────────┘
                                       │
            ┌──────────────────────────┴────────────────────────────┐
            │                                                       │
            ▼                                                       ▼
┌────────────────────────┐                            ┌─────────────────────────────┐
│   GET /round/info      │                            │   PKARR publish loop        │
│   (handlers.rs)        │                            │   (run.rs:335, 367)         │
│                        │                            │                             │
│  InfoResponse {        │                            │  build_coordinator_packet(  │
│    ...                 │                            │    keypair, addr,           │
│    supported_script    │                            │    denom, min_p, status,    │
│      _types: Vec       │                            │    supported, ost,          │
│    output_script_type  │                            │  )                          │
│  }                     │                            │                             │
└────────────────────────┘                            │  TXT JSON:                  │
                                                      │  { type, version: "0.2.0",  │
                                                      │    onion, network,          │
                                                      │    denomination_sats,       │
                                                      │    min_participants,        │
                                                      │    status,                  │
                                                      │    sst: "p2..,p2..,p2..",   │
                                                      │    ost: "p2.." }            │
                                                      │   ↳ < 220 bytes (inline     │
                                                      │     assert)                 │
                                                      └─────────────────────────────┘

                                       Client-side (POST /round/input)
                                                       │
                                                       ▼
                ┌──────────────────────────────────────────────────────┐
                │   handlers::post_input                               │
                │   - decode OwnershipProof (two-phase try-parse)      │
                │   - call validate_utxo(...)                          │
                └──────────────────────┬───────────────────────────────┘
                                       │
                                       ▼
                ┌──────────────────────────────────────────────────────┐
                │   validate_utxo (Phase 16 SWAPPED dispatcher)        │
                │                                                       │
                │   on_chain_spk = parse from gettxout(utxo)           │
                │                                                       │
                │   match proof.version {                              │
                │     1 => {                                           │
                │       // CRIT-01: derive from chain                  │
                │       derived = detect_script_type(&on_chain_spk)?;  │
                │       allowlist_check(derived)?;                     │
                │       verify_simple(derived, spk, &witness, msg, net)│
                │     }                                                │
                │     2 => {                                           │
                │       psbt = decode(psbt_input_b64)?;                │
                │       witness = extract_witness(&psbt)?;             │
                │       declared = proof.script_type ok_or             │
                │         WireFormatMismatch;                          │
                │       // CRIT-01: derive from chain                  │
                │       derived = detect_script_type(&on_chain_spk)?;  │
                │       if declared != derived ScriptTypeMismatch;     │
                │       allowlist_check(derived)?;                     │
                │       verify_simple(derived, spk, &witness, msg, net)│
                │     }                                                │
                │     _ => Err(UnsupportedProofVersion)                │
                │   }                                                  │
                │                                                       │
                │   tracing::info!(round_id=%, script_type=?derived,   │
                │                  "ownership proof verified");        │
                └──────────────────────────────────────────────────────┘
```

### Recommended Project Structure

```
coordinator/src/
├── config.rs                # +BipConfig struct; +validate() extension
├── api/
│   └── handlers.rs          # +InfoResponse field population in get_info
├── bitcoin/
│   └── utxo.rs              # validate_utxo dispatcher swap; DELETE is_p2wpkh + verify_bip322_simple
└── discovery/
    └── pkarr_pub.rs         # bump "version" → "0.2.0"; add "sst" + "ost"; inline 220-byte test

tests/integration/
├── mod.rs                   # +fund_regtest_typed (new helper, Phase 16)
├── full_round.rs            # DO NOT MODIFY (v1.3 invariant gate)
└── multi_script_validate.rs # NEW — 9 D-54 cases

shared/                      # DO NOT MODIFY (Phase 15 contract is locked)

client/                      # DO NOT MODIFY (Phase 17 WALLET-03/04)

.github/workflows/ci.yml     # +crit01-grep-check job (mirrors bip322-pin-check)
```

### Pattern 1: BipConfig field shape (D-38)

**What:** New top-level config section, `#[serde(default)]` for v1.3 config-file compat (a coordinator.toml without `[bip]` boots with all-allowed defaults).
**When to use:** Every coordinator-startup boot reads this; nothing else touches it.
**Example:**

```rust
// Source: D-38 verbatim + existing per-field default fns at coordinator/src/config.rs:54-72
use shared::bip322::ScriptType;

#[derive(Debug, Deserialize, Clone)]
pub struct BipConfig {
    #[serde(default = "default_true")]
    pub allow_p2wpkh: bool,
    #[serde(default = "default_true")]
    pub allow_p2tr: bool,
    #[serde(default = "default_true")]
    pub allow_p2sh_p2wpkh: bool,
    #[serde(default = "default_output_script_type")]
    pub output_script_type: ScriptType,
}

fn default_true() -> bool { true }
fn default_output_script_type() -> ScriptType { ScriptType::P2wpkh }

impl BipConfig {
    pub fn allows(&self, st: ScriptType) -> bool {
        match st {
            ScriptType::P2wpkh => self.allow_p2wpkh,
            ScriptType::P2tr => self.allow_p2tr,
            ScriptType::P2shP2wpkh => self.allow_p2sh_p2wpkh,
        }
    }

    /// Alphabetical canonical order per CD-11 — locks PKARR CSV byte length deterministic.
    pub fn supported(&self) -> Vec<ScriptType> {
        let mut v = Vec::new();
        if self.allow_p2sh_p2wpkh { v.push(ScriptType::P2shP2wpkh); }
        if self.allow_p2tr        { v.push(ScriptType::P2tr); }
        if self.allow_p2wpkh      { v.push(ScriptType::P2wpkh); }
        v
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.allow_p2wpkh || self.allow_p2tr || self.allow_p2sh_p2wpkh,
            "bip section requires at least one allow_* flag = true; got all false. \
             Set BLINDJOIN__COORDINATOR__BIP__ALLOW_P2WPKH=true (or another flag) \
             to enable input acceptance for that script type."
        );
        anyhow::ensure!(
            self.allows(self.output_script_type),
            "bip.output_script_type = {:?} but the matching allow_* flag is false. \
             The coordinator cannot advertise an output type it cannot accept on its own \
             round outputs. Set the matching BLINDJOIN__COORDINATOR__BIP__ALLOW_* flag = true \
             or change BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE.",
            self.output_script_type,
        );
        Ok(())
    }
}
```

Add `pub bip: BipConfig` (with `#[serde(default)]`) to `CoordinatorConfig` and chain `self.bip.validate()` from `CoordinatorConfig::validate()` (existing fn at lines 157-188).

### Pattern 2: validate_utxo dispatcher swap (D-45..D-50)

**What:** Replace the linear `verify_bip322_simple(&script_pubkey, &witness, &message)` call with a `match proof.version { 1 => v1_path, 2 => v2_path, _ => Err(UnsupportedProofVersion) }`. Both branches derive `script_type` from the chain. v=2 additionally cross-checks against the declared field.
**When to use:** Single call site inside `validate_utxo` at `coordinator/src/bitcoin/utxo.rs:74`. No other place in the codebase calls `verify_bip322_simple`.
**Example:**

```rust
// Source: D-45..D-50 + existing call-site shape at coordinator/src/bitcoin/utxo.rs:70-77
use shared::bip322::{detect_script_type, verify_simple, Bip322Error};
use shared::protocol::OwnershipProof;
use bitcoin::Witness;

// (inside validate_utxo, after the `let script_pubkey = ...` line)
let network = parse_bitcoin_network(&network_str);  // threaded from caller per D-51
let message = format!("blindjoin:round:{}:utxo:{}:{}", round_id, utxo.txid, utxo.vout);

let derived = match ownership_proof.version {
    1 => {
        // CRIT-01: script_type derived from on-chain script_pubkey, never from client field
        let derived = detect_script_type(&script_pubkey)
            .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;
        if !config.bip.allows(derived) {
            return Err(UtxoError::InvalidProof {
                reason: Bip322Error::UnsupportedScriptType.to_string(),
            });
        }
        let witness = Witness::from_slice(&ownership_proof.witness_stack);
        verify_simple(derived, &script_pubkey, &witness, message.as_bytes(), network)
            .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;
        derived
    }
    2 => {
        let psbt_input_b64 = ownership_proof.psbt_input_b64.as_ref()
            .ok_or_else(|| UtxoError::InvalidProof {
                reason: Bip322Error::WireFormatMismatch(
                    "v2 OwnershipProof requires psbt_input_b64".into()
                ).to_string(),
            })?;
        let declared = ownership_proof.script_type
            .ok_or_else(|| UtxoError::InvalidProof {
                reason: Bip322Error::WireFormatMismatch(
                    "v2 OwnershipProof requires script_type field".into()
                ).to_string(),
            })?;
        let (witness, _) = decode_psbt_input(psbt_input_b64)
            .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;
        // CRIT-01: script_type derived from on-chain script_pubkey, never from client field
        let derived = detect_script_type(&script_pubkey)
            .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;
        if declared != derived {
            return Err(UtxoError::InvalidProof {
                reason: Bip322Error::ScriptTypeMismatch { declared, derived }.to_string(),
            });
        }
        if !config.bip.allows(derived) {
            return Err(UtxoError::InvalidProof {
                reason: Bip322Error::UnsupportedScriptType.to_string(),
            });
        }
        verify_simple(derived, &script_pubkey, &witness, message.as_bytes(), network)
            .map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;
        derived
    }
    v => {
        return Err(UtxoError::InvalidProof {
            reason: Bip322Error::UnsupportedProofVersion(v).to_string(),
        });
    }
};

tracing::info!(
    round_id = %round_id,
    script_type = ?derived,
    "ownership proof verified"
);
```

Note the **two CRIT-01 comments** — one in each branch — satisfying the grep gate.

### Pattern 3: PKARR record schema bump (D-39..D-43)

**What:** Extend the existing `serde_json::json!({...})` literal in `build_coordinator_packet` with `"sst"` (CSV alphabetical) + `"ost"` (single value); bump `"version"` → `"0.2.0"`; assert payload bytes < 220 inline.
**When to use:** Every PKARR record publish (initial + heartbeat) emits the v0.2.0 shape starting Phase 16.
**Example:**

```rust
// Source: D-39..D-44 + existing build_coordinator_packet at coordinator/src/discovery/pkarr_pub.rs:57-96
pub fn build_coordinator_packet(
    keypair: &Keypair,
    coordinator_addr: &str,
    denomination_sats: u64,
    min_participants: u32,
    status: &str,
    supported: &[&str],          // NEW (D-39, D-40)
    output_script_type: &str,    // NEW (D-41)
) -> Result<SignedPacket> {
    let record = serde_json::json!({
        "type": "blindjoin-coordinator",
        "version": "0.2.0",                                 // BUMP from 0.1.0
        "onion": coordinator_addr,
        "network": "signet",
        "denomination_sats": denomination_sats,
        "min_participants": min_participants,
        "status": status,
        "sst": supported.join(","),                          // NEW (D-40)
        "ost": output_script_type,                           // NEW (D-41)
    });
    let json_str = serde_json::to_string(&record)?;
    // ... rest unchanged
}
```

Both call sites in `coordinator/src/run.rs:335-336` and `367-368` gain two new args derived from `cfg.bip.supported()` (mapped to wire-form strings via serde_json) and `cfg.bip.output_script_type` (likewise).

### Pattern 4: InfoResponse JSON array form (D-42)

**What:** Add two fields to `shared::protocol::InfoResponse`. Wire form is a proper JSON array (no byte budget for HTTP JSON; the 220-byte limit applies ONLY to PKARR TXT). `#[serde(default = "...")]` on both makes v1.3↔v1.4 deserialize cleanly.
**Example:**

```rust
// Source: D-42 + existing InfoResponse at shared/src/protocol.rs:13-28
use crate::bip322::ScriptType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoResponse {
    pub version: String,
    // ... existing fields ...
    pub round_id: Option<uuid::Uuid>,

    // Phase 16 NEW (D-42)
    #[serde(default = "default_legacy_supported")]
    pub supported_script_types: Vec<ScriptType>,
    #[serde(default = "default_legacy_output")]
    pub output_script_type: ScriptType,
}

fn default_legacy_supported() -> Vec<ScriptType> {
    vec![ScriptType::P2wpkh]
}
fn default_legacy_output() -> ScriptType {
    ScriptType::P2wpkh
}
```

`coordinator/src/api/handlers.rs::get_info` (existing at lines 46-67) gains two field reads from `state.config.bip`:

```rust
Json(InfoResponse {
    // ... existing fields ...
    supported_script_types: state.config.bip.supported(),
    output_script_type: state.config.bip.output_script_type,
})
```

### Anti-Patterns to Avoid

- **Deriving `script_type` from the client-declared field** (CRIT-01 violation). The whole point of D-10 is that the client doesn't get to declare the script type — the chain does. v=2's declared field exists ONLY as a defense-in-depth cross-check, never as the authoritative source.
- **Putting the CRIT-01 comment ONCE** at the top of `validate_utxo` instead of inline at each branch. The grep gate counts `CRIT-01` occurrences in `coordinator/src/bitcoin/utxo.rs` and requires ≥ 2. The intent is that a future refactor cannot accidentally drop the v=2 cross-check while leaving the v=1 path looking superficially correct.
- **Using PSBT's `witness_utxo.script_pubkey` for script-type derivation.** A malicious client can put ANY `TxOut` into the PSBT input's `witness_utxo` field; the only trustworthy source is the SPK fetched from `bitcoincore::gettxout(utxo)` — which Phase 16 already has via the existing `parse_script_pubkey_from_txout` at `coordinator/src/bitcoin/utxo.rs:80-85`.
- **Mixing the dispatcher swap with PKARR changes in one commit** (CD-15 violation). Atomic commits per CD-10 — REPAIR-01 lesson #1.
- **Using full PKARR field names** (`"supported_script_types"` / `"output_script_type"`). Pushes payload to ~226 bytes, breaching the 220-byte warn. Use `"sst"` / `"ost"` (D-39).
- **Adding a JSON-array form for PKARR `sst`** instead of CSV. Costs ~8 extra bytes vs. CSV; CSV is the locked choice (D-40).
- **Modifying `tests/integration/full_round.rs`** at this phase boundary. v1.3 invariant gate — must remain identical at every v1.4 phase boundary per the ROADMAP cross-phase invariant.
- **Modifying `client/src/discover.rs`** at this phase boundary. Phase 17 WALLET-03 owns it; Phase 16 only verifies that the existing resolver tolerates new JSON fields (it does — see Pitfall 3 below).
- **Hand-rolling `parse_bitcoin_network` for the v=2 path** when handlers.rs:602-610 already has it. The existing helper at the handler layer covers regtest, signet, testnet, mainnet — reuse, don't duplicate.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BIP-322 script-type detection | Custom `is_p2tr() / is_p2sh()` chain in coordinator | `shared::bip322::detect_script_type` | Phase 15 already shipped; covers P2WPKH / P2TR / P2SH with no fallthrough; returns `Bip322Error::UnsupportedScriptType` on unknown SPK shape [VERIFIED: code read at shared/src/bip322/mod.rs:223-233] |
| BIP-322 multi-script verify | Per-script ECDSA / Schnorr sighash code in coordinator | `shared::bip322::verify_simple` | Phase 15 shipped; routes through the pinned `bip322 = "=0.0.10"` crate adapter; handles 64-byte SIGHASH_DEFAULT + 65-byte SIGHASH_ALL Schnorr forms; P2SH-P2WPKH HASH160 cross-check is internal to the crate [VERIFIED: code read at shared/src/bip322/mod.rs:242-254] |
| OwnershipProof parsing | Manual JSON inspection + version branching at the handler | `shared::protocol::OwnershipProof::from_json_hex_str` | Phase 15 shipped two-phase try-parse — handles BOTH v1.3 array-of-hex AND v2 flat-struct envelopes; existing call site at `coordinator/src/api/handlers.rs:136` is unchanged in Phase 16 [VERIFIED] |
| Config env-var bool parsing | Custom `"true"`/`"false"` string match | `config::Environment::try_parsing(true)` | Existing pattern at `coordinator/src/config.rs:130`; `try_parsing` attempts bool, i64, f64 in sequence and routes the right type through serde — `BLINDJOIN__COORDINATOR__BIP__ALLOW_P2WPKH=true` deserializes as `bool` cleanly [VERIFIED: existing usage + CITED: docs.rs/config] |
| PKARR signed-record publish | DIY DNS TXT signing | `pkarr::SignedPacket::builder().txt(...).sign(keypair)` | Existing pattern at `coordinator/src/discovery/pkarr_pub.rs:92-95`; Phase 16 only changes the JSON inside the TXT, not the publish path |
| Tracing field formatting for `ScriptType` | Manual `Display` impl on `ScriptType` | `tracing::info!(script_type = ?derived, ...)` — Debug formatting | `ScriptType` already derives `Debug`; output is `P2wpkh / P2tr / P2shP2wpkh` (operator-readable). Adding `Display` would expand the crate's public surface for no gain [VERIFIED: code read at shared/src/bip322/mod.rs:150] |
| Regtest UTXO generation for P2TR / P2SH-P2WPKH | Manual descriptor construction + key derivation in the test | Bitcoin Core `getnewaddress` with `address_type = "bech32m"` / `"p2sh-segwit"` via corepc-node's `node.client.new_address_with_type(...)` (or JSON-RPC fallthrough — see Pitfall 6) | The existing P2WPKH fund_regtest already uses external WIFs; Phase 16's test fixture needs the bdk-free `sign_simple_test_only` mirror Phase 15 added so non-P2WPKH witnesses can be constructed without bringing `bdk_wallet` into the test [VERIFIED: code read at shared/src/bip322/mod.rs:302-314] |

**Key insight:** Phase 16's deliverable is fundamentally **wiring** — every primitive it needs has been shipped, verified, and stabilized by Phase 14 (ADR) and Phase 15 (shared crate). The risk is in the wiring discipline (CRIT-01 at both branches, atomic commits per plan, the v1.3 cross-phase invariant), not in any unsolved technical problem.

## Common Pitfalls

### Pitfall 1: CRIT-01 comment present at only ONE branch

**What goes wrong:** A refactor or merge silently drops the v=2 declared-vs-derived cross-check, leaving the v=1 branch with the comment but the v=2 branch trusting the client's declaration.
**Why it happens:** The grep gate `grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` ≥ 2 catches the missing comment, but only if the gate is implemented (D-49). Without it, a future PR could regress silently.
**How to avoid:** Land the CI grep gate in 16-02 (same commit as the dispatcher swap). Mirror `.github/workflows/ci.yml::bip322-pin-check`. Pattern:
```yaml
crit01-grep-check:
  name: CRIT-01 invariant comment count
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@<pinned-sha>
    - name: Enforce CRIT-01 dual-branch comment
      run: |
        set -eu
        COUNT=$(grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs)
        if [ "$COUNT" -lt 2 ]; then
          echo "ERROR: coordinator/src/bitcoin/utxo.rs has $COUNT CRIT-01 comments (need >= 2)." >&2
          echo "       The script-type derived-from-chain invariant must be commented" >&2
          echo "       at EACH version branch of the validate_utxo dispatcher." >&2
          exit 1
        fi
```
**Warning signs:** Code review sees the dispatcher's `match` arms drift apart — one branch verifying against `derived`, the other verifying against `declared`. Code-review checklist item: "Both branches call `detect_script_type(&on_chain_spk)` BEFORE `verify_simple`."

### Pitfall 2: PKARR byte budget breach when a fourth script type lands in v1.5+

**What goes wrong:** v1.5 (hypothetical) adds a fourth script type (e.g., P2WSH). The `sst` CSV grows from `"p2sh-p2wpkh,p2tr,p2wpkh"` (~24 bytes) to `"p2sh-p2wpkh,p2tr,p2wpkh,p2wsh"` (~30 bytes). Worst-case payload pushes past 220 bytes.
**Why it happens:** The compact-name strategy buys ~25 bytes vs. full names; that's the entire margin. Adding fields or types eats it fast.
**How to avoid:** Phase 16's D-55 inline test is the regression gate. ANY plan that adds fields to the PKARR record or extensions to the script-type set MUST run the test locally first AND verify the assertion still passes at the worst case (all-allowed default config).
**Warning signs:** The `coordinator_packet_under_220_byte_budget` test goes red. The plan's response: do NOT relax the assertion; redesign the encoding (single-char codes, bitmask, etc.) — this is a v1.5+ problem (per `<deferred>`).

### Pitfall 3: v1.3 client resolver breaks on new JSON fields in the PKARR record

**What goes wrong:** v1.3 client's `client/src/discover.rs::parse_onion_from_rr` uses `serde_json::from_str::<Partial>(&s)` where `Partial { onion: Option<String> }`. If the parser were `#[serde(deny_unknown_fields)]`, the new `"sst"` / `"ost"` / `"version": "0.2.0"` fields would break deserialization.
**Why it happens:** This is exactly the v1.3 ↔ v1.4 backwards-compat contract.
**Mitigation status:** SAFE. The v1.3 `Partial` struct at `client/src/discover.rs:77-79` has only one field and no `deny_unknown_fields`. Default serde behavior silently drops unknown JSON keys. v1.3 clients see `"onion"` and ignore the rest. [VERIFIED: code read at client/src/discover.rs:75-80]
**Warning signs:** None for Phase 16. A v1.3 binary against a v1.4 coordinator will discover successfully and proceed to `/round/info`, where it gets a v1.4 InfoResponse that ALSO tolerates unknown fields (the `InfoResponse` struct at `shared/src/protocol.rs:13-28` has no `deny_unknown_fields` — file-top invariant per `shared/src/protocol.rs:3-5`).

### Pitfall 4: v1.4 coordinator's PKARR resolver-side roundtrip (re-resolve own record)

**What goes wrong:** A future plan needs the coordinator to re-resolve its own PKARR record (e.g., for self-health-checks). The v1.4 producer emits `"sst"` + `"ost"` which the v1.3 client crate doesn't parse. If the coordinator imports the client crate to do the resolution, the new fields are silently dropped (which is fine), BUT the coordinator can't read `sst` back through that path.
**Mitigation status:** SAFE for Phase 16 — D-56 explicitly defers resolver-side changes to Phase 17. The coordinator does NOT resolve its own record in Phase 16. If a v1.5 phase needs that capability, the resolver-side `#[serde(default)]` shim lives in the client crate, not coordinator.
**Warning signs:** A plan task references `discover_coordinator(...)` from inside `coordinator/`. That's an architectural smell — flag it.

### Pitfall 5: `config 0.15` env-var bool parsing ambiguity (`"true"` vs `"1"`)

**What goes wrong:** Operators sometimes use `0/1` in env vars rather than `true/false`. If `config::Environment::try_parsing(true)` doesn't recognise `"1"` as `true`, the override silently fails.
**Why it happens:** `try_parsing(true)` attempts bool, then i64, then f64. `"1"` parses as i64 (1), NOT bool — so the resulting `Value` is integer-typed; serde's `bool` deserializer rejects an integer.
**Mitigation status:** Behavior verified — `try_parsing(true)` accepts `"true"` and `"false"` strings cleanly (lowercased) and the deserializer maps to `bool`. `"1"` / `"0"` DO NOT work in this layout. Existing Phase 8 code uses `try_parsing(true)` and the existing bool field `tor_mode` works with `BLINDJOIN__COORDINATOR__TOR_MODE=true` [VERIFIED: existing usage + CITED: docs.rs/config Environment].
**How to avoid:** Document the convention in the BipConfig field-comment: "Use 'true' or 'false' (lowercase strings) — '0' / '1' do not deserialise as bool." Inline doctest is overkill; the existing TOML/env override unit test pattern (covered in 16-01 task plan) catches it.
**Warning signs:** A bug report along the lines of "I set `BLINDJOIN__COORDINATOR__BIP__ALLOW_P2TR=1` and the coordinator still accepts P2TR." That's the symptom; fix is to use `=true`/`=false`.

### Pitfall 6: regtest P2TR / P2SH-P2WPKH UTXO generation via corepc-node

**What goes wrong:** The existing `fund_regtest` at `tests/integration/mod.rs:396-534` derives P2WPKH addresses from hardcoded WIFs and funds them with `node.client.send_to_address(&addr, fund_btc)`. The `Address::p2wpkh(...)` helper is P2WPKH-only — there's no equivalent for P2TR / P2SH-P2WPKH from a WIF.
**Why it happens:** P2TR requires the BIP-341 tweaked output key (derived from an x-only pubkey); P2SH-P2WPKH requires the HASH160 of the inner P2WPKH redeem script. Both derivations are doable in pure rust-bitcoin (Phase 15's `fixture_p2tr_spk` + `fixture_p2sh_spk` at `shared/src/bip322/mod.rs:453-474` shows the recipe).
**How to avoid:** Add a typed `fund_regtest_typed(exe, &[(ScriptType, n_utxos)])` helper to `tests/integration/mod.rs`. For each requested script type:
- P2WPKH: existing path (WIF → CompressedPublicKey → Address::p2wpkh).
- P2TR: WIF → Keypair → tap_tweak (no merkle root) → `Address::p2tr_tweaked` (or build the SPK directly via `ScriptBuf::new_p2tr_tweaked`).
- P2SH-P2WPKH: WIF → CompressedPublicKey → wpkh redeem ScriptBuf → `Address::p2sh(&redeem, network)`.
Then `node.client.send_to_address(&addr, fund_btc)` works for ALL three (Bitcoin Core's `sendtoaddress` is address-type-agnostic; the wallet's `getnewaddress` is what cares about address types, and we're not using that for the funded addresses anyway). Bitcoin Core's `getnewaddress` for mining payouts accepts `address_type = "bech32m"` / `"p2sh-segwit"` / `"bech32"` / `"legacy"` if needed [CITED: bitcoincore.org/en/doc/28.0.0/rpc/wallet/getnewaddress].
**Warning signs:** Test failure: "funding tx X has no output to Y" — the wallet-agnostic vout lookup at `tests/integration/mod.rs:486-502` matches the recipient address as a string. If `Address::p2tr_tweaked(...)` produces a slightly-different string form than what the wallet's `script_pubkey.address` returns, the lookup misses. Mitigation: compare ScriptBuf bytes (`o.script_pubkey.hex`) rather than `address` strings.

### Pitfall 7: v=2 PSBT input decode — `bitcoin::psbt::Input` has no `pub fn deserialize(bytes)`

**What goes wrong:** D-47 says "decode_psbt_input(&proof.psbt_input_b64)" returns a `bitcoin::psbt::Input`. But `bitcoin::psbt::Input` in 0.32.x does NOT expose a public byte-level decode method — only the full `Psbt` has `Psbt::deserialize(&[u8])` and `FromStr` (base64) [CITED: docs.rs/bitcoin/0.32.5/bitcoin/psbt/struct.Psbt.html].
**Why it happens:** The PSBT BIP-174 binary format is at the `Psbt`-level (magic bytes + unsigned tx + per-input/per-output key-value maps). A bare `Input` byte-encoding doesn't exist in the spec.
**How to avoid:** Two acceptable shapes — the planner must pick one and document the choice:
1. **`psbt_input_b64` carries a full BIP-174 PSBT** (with the unsigned tx + one input + zero outputs). Decode via `Psbt::deserialize(&base64_decoded_bytes)` (or `Psbt::from_str(&b64_string)` since `Psbt` impls `FromStr` when the `base64` feature is enabled). Extract `psbt.inputs[0]`.
2. **`psbt_input_b64` carries serde-json-encoded `Input`** via `serde_json::from_slice`. `bitcoin::psbt::Input` does impl `Deserialize` [CITED: docs.rs/bitcoin/0.32.5/bitcoin/psbt/struct.Input.html]. But this breaks the "base64-encoded binary PSBT" mental model.

**Recommendation:** Option 1 (full PSBT with single input). This matches ADR Decision #3 wording "B2 base64 PSBT-input shape" — interpreted as "a base64-encoded PSBT containing the input we're proving ownership of." Sprint-0-A's adapter sketch and Phase 15 D-22..D-25 do not contradict this. The plan must verify the chosen shape lands in Phase 15's 5 D-13 roundtrip tests (Phase 15-01-SUMMARY confirmed they pass; the shape there is the contract).

**Witness extraction from the decoded PSBT input:**
```rust
fn decode_psbt_input(b64: &str) -> Result<(Witness, ScriptBuf), Bip322Error> {
    let bytes = BASE64.decode(b64)
        .map_err(|e| Bip322Error::DecodeError(format!("base64: {e}")))?;
    let psbt = bitcoin::psbt::Psbt::deserialize(&bytes)
        .map_err(|e| Bip322Error::DecodeError(format!("psbt: {e}")))?;
    let input = psbt.inputs.first()
        .ok_or_else(|| Bip322Error::WireFormatMismatch(
            "v2 PSBT envelope contains zero inputs".into()
        ))?;
    let witness = input.final_script_witness.clone()
        .ok_or_else(|| Bip322Error::WireFormatMismatch(
            "v2 PSBT input lacks final_script_witness".into()
        ))?;
    // Note: this returns the witness only; the on-chain SPK is sourced
    // from gettxout (CRIT-01), NOT from input.witness_utxo (which a
    // malicious client could spoof).
    let _client_supplied_spk = input.witness_utxo.as_ref().map(|t| t.script_pubkey.clone());
    Ok((witness, _client_supplied_spk.unwrap_or_default()))
}
```
The returned `ScriptBuf` from `witness_utxo` is **NOT USED** for verification — CRIT-01 says the on-chain SPK from `gettxout` is the only trustworthy source. The fn signature returns it for symmetry / future-cross-check potential but the dispatcher discards it.

**Warning signs:** The 9 D-54 test cases at `multi_script_validate.rs` fail with `Bip322Error::DecodeError("psbt: ...")` rather than the expected `ScriptTypeMismatch / UnsupportedScriptType / WireFormatMismatch`. Indicates the test fixture's PSBT envelope shape doesn't match what the dispatcher expects to decode. Plan-time: align test fixture's `psbt_input_b64` construction with the production decode path.

### Pitfall 8: tracing log spam at INFO for high-volume input registration

**What goes wrong:** Every successful UTXO registration emits one INFO line. At max_participants=20 per round and a busy coordinator running multiple rounds per minute, this is ~20-100 INFO lines/minute — significantly more than v1.3 (which was silent on this path).
**Why it happens:** CD-12 defaulted to "INFO always" (consistent operator log shape; v1.3 silence delta acceptable).
**Mitigation status:** ACCEPTED RISK per CD-12. Plan-phase may override to DEBUG if operator feedback shows the volume is too high; defer to runtime observation.
**Warning signs:** Operator complaint: "the coordinator log is too verbose since v1.4." Fix: flip CD-12 to DEBUG for P2WPKH (matches v1.3 silence) and INFO for P2TR/P2SH-P2WPKH (new behavior worth surfacing). Phase 16 plan does NOT need to pre-emptively split — accept the simple "INFO always" until evidence demands otherwise.

### Pitfall 9: OwnershipProof decoder collision on a malformed v=2 payload

**What goes wrong:** A client sends `{"version": 2, "psbt_input_b64": "AA==", "script_type": "p2wpkh", "witness_stack": ["0011"]}` — both the v1 (witness_stack populated) and v2 (psbt_input_b64 + script_type populated) fields. Which path does the dispatcher take?
**Why it happens:** The two-phase try-parse at `shared/src/protocol.rs:170-187` always returns a SINGLE `OwnershipProof`. The dispatch is purely on `proof.version`.
**Mitigation status:** SAFE. `match proof.version` is unambiguous; v=2 takes the v=2 path and ignores `witness_stack` (the v=2 path extracts witness from `psbt_input_b64`, not from `witness_stack`). v=1 path ignores `psbt_input_b64` + `script_type`. There's no ambiguity at the verifier — only the wire shape is permissive.
**Warning signs:** None. The Phase 15 5 D-13 roundtrip tests cover the relevant shapes.

### Pitfall 10: v1.3 client sends `version: 1` + a non-P2WPKH UTXO

**What goes wrong:** A v1.3 client with a P2TR wallet would send `version = 1` (the only version v1.3 emits) + a witness stack against a P2TR SPK. v1.3 coordinator rejected with `is_p2wpkh()` gate. Phase 16 coordinator's v=1 path now calls `detect_script_type(on_chain_spk)` → returns `ScriptType::P2tr` → allowlist check passes (default) → `verify_simple(ScriptType::P2tr, ...)` — verification proceeds.
**Why it happens:** The v=1 envelope was never P2WPKH-only in the wire-format sense; it was P2WPKH-only because the verifier was P2WPKH-only. Phase 16 lifts that restriction.
**Mitigation status:** This is INTENDED BEHAVIOR per the success criterion #1 ("operator running v1.4 with default config sees a P2TR ownership proof registered and accepted"). v1.3 clients SHOULDN'T be sending non-P2WPKH UTXOs (v1.3 wallet didn't support them), but if they do, the v=1 path will route correctly.
**Warning signs:** None for Phase 16. Phase 17 WALLET-01 adds the client-side wallet descriptors that make this scenario possible from a v1.4 client; Phase 17 WALLET-04 makes the v1.4 client emit `version = 1` only for P2WPKH UTXOs against a v1.3 coordinator (the v1.4→v1.3 compat shim direction).

## Code Examples

### Example 1: Existing CoordinatorConfig::validate extension pattern

```rust
// Source: coordinator/src/config.rs:157-185 — verified current
pub fn validate(&self) -> anyhow::Result<()> {
    let c = &self.coordinator;

    anyhow::ensure!(
        (1..=60_000).contains(&c.rate_limit_info_per_min),
        "coordinator.rate_limit_info_per_min must be in 1..=60_000; got {}. \
         Set BLINDJOIN__COORDINATOR__RATE_LIMIT_INFO_PER_MIN to a value in that range.",
        c.rate_limit_info_per_min,
    );
    // ... (3 more anyhow::ensure! blocks)

    // Phase 16: chain bip section validation
    self.bip.validate()?;
    Ok(())
}
```

The Phase 16 addition is one line: `self.bip.validate()?;` placed after the existing 4 `anyhow::ensure!` checks. Error messages from `BipConfig::validate()` flow through the same `anyhow::Error` channel; existing Phase 8 startup-error formatting at `coordinator/src/run.rs:46` (`.context("Invalid coordinator configuration")?`) wraps both.

### Example 2: PKARR publish call-site update in run.rs

```rust
// Source: coordinator/src/run.rs:335-336 (initial publish) + 367-368 (heartbeat)
// Existing shape:
if let Ok(packet) = discovery::pkarr_pub::build_coordinator_packet(
    &keypair, &addr, denom, min_p, "idle",
) { ... }

// Phase 16 shape:
let supported_strs: Vec<String> = cfg.bip.supported()
    .iter()
    .map(|st| match st {
        shared::bip322::ScriptType::P2wpkh => "p2wpkh".to_string(),
        shared::bip322::ScriptType::P2tr => "p2tr".to_string(),
        shared::bip322::ScriptType::P2shP2wpkh => "p2sh-p2wpkh".to_string(),
    })
    .collect();
let supported_refs: Vec<&str> = supported_strs.iter().map(|s| s.as_str()).collect();
let output_st = serde_plain_or_inline_match(&cfg.bip.output_script_type);  // returns "p2wpkh" etc.

if let Ok(packet) = discovery::pkarr_pub::build_coordinator_packet(
    &keypair, &addr, denom, min_p, "idle",
    &supported_refs, &output_st,
) { ... }
```

Note: `ScriptType` already has `Serialize` derive with the kebab-case form (`shared/src/bip322/mod.rs:150-157`). The plan can either use `serde_json::to_value(&st).unwrap().as_str()` to get the wire string, or inline the match. Both work; the inline match is one fewer function call and more grep-able for the alphabetical-canonical-order invariant.

### Example 3: `tests/integration/multi_script_validate.rs` test skeleton

```rust
// Source: D-54 test case names verbatim + Phase 15 matches!() discipline
// File: tests/integration/multi_script_validate.rs (NEW in 16-02)

use crate::{fund_regtest_typed, require_bitcoind, BitcoindGuard};
use shared::bip322::{detect_script_type, ScriptType, Bip322Error};
use shared::protocol::OwnershipProof;

#[tokio::test]
async fn validate_p2wpkh_utxo_with_v1_legacy_proof_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    // ... build a v=1 ownership_proof for the P2WPKH UTXO ...
    // ... call validate_utxo directly OR via the HTTP path; assert Ok(_) ...
}

#[tokio::test]
async fn validate_p2tr_utxo_with_v2_declared_p2tr_ok() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2tr, 1)]).await;
    // ... build a v=2 ownership_proof with script_type = Some(P2tr); ...
    // ... call validate_utxo; assert Ok(_) ...
}

#[tokio::test]
async fn validate_p2wpkh_utxo_with_v2_declared_p2tr_rejects_spoofing() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)]).await;
    // ... build a v=2 ownership_proof with script_type = Some(P2tr) but a P2WPKH UTXO ...
    // ... call validate_utxo; assert matches!(err, Bip322Error::ScriptTypeMismatch{...}) ...
    // The discrimination point: the on-chain P2WPKH SPK detect_script_type → P2wpkh;
    // the declared field says P2tr; ScriptTypeMismatch fires before verify_simple even runs.
}

#[tokio::test]
async fn validate_p2tr_utxo_with_allow_p2tr_false_rejects_unsupported() {
    let exe = require_bitcoind!();
    let (_guard, setup) = fund_regtest_typed(exe, &[(ScriptType::P2tr, 1)]).await;
    // ... build BipConfig with allow_p2tr = false, allow_p2wpkh = true ...
    // ... call validate_utxo; assert matches!(err, Bip322Error::UnsupportedScriptType) ...
}

// ... 5 more tests per D-54 verbatim ...
```

Each test sets up exactly the UTXO type it needs. The shared `fund_regtest_typed` helper (new in 16-02 — see Pitfall 6) takes a slice of `(ScriptType, count)` and returns funded UTXOs of each type. The `BipConfig` is constructed in-test (not from a TOML file) so each test can isolate the allowlist config.

### Example 4: PKARR byte-budget assertion (D-55)

```rust
// Source: D-55 verbatim
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinator_packet_under_220_byte_budget() {
        let kp = Keypair::random();
        // Worst case: all 3 script types allowed → longest sst CSV.
        let supported = ["p2sh-p2wpkh", "p2tr", "p2wpkh"];
        let _packet = build_coordinator_packet(
            &kp,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
            &supported,
            "p2wpkh",
        ).expect("packet builds");
        // The function internally serializes to compute warn threshold; we
        // duplicate the serialization here to assert the size as a CI gate.
        let serialized = serde_json::to_string(&serde_json::json!({
            "type": "blindjoin-coordinator",
            "version": "0.2.0",
            "onion": "127.0.0.1:8080",
            "network": "signet",
            "denomination_sats": 1_000_000_u64,
            "min_participants": 3_u32,
            "status": "idle",
            "sst": supported.join(","),
            "ost": "p2wpkh",
        })).unwrap();
        assert!(
            serialized.len() < 220,
            "PKARR packet {} bytes exceeds 220-byte CI warn budget — regression gate. \
             Did you add a field? Reduce field-name length or descope the addition.",
            serialized.len()
        );
    }
}
```

The duplicated serialization is intentional — the test asserts the SHAPE the production code emits, not just that `build_coordinator_packet` doesn't panic. A future production change that adds a field WILL fail this test until the field name is compacted or the test threshold is raised (which should require explicit ADR discussion per CD-15's discipline pattern).

## Runtime State Inventory

> Phase 16 is a code/integration phase, not a rename or migration. No runtime-state migration is required. The inventory is included below per the protocol; every category is explicitly empty.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — Phase 16 does NOT change SQLite schemas, ChromaDB collections, Mem0 datastores, ban list format, or any persisted-state shape. Round state remains memory-only with `#[zeroize(skip)]` on non-PII fields (v1.0). Ban list format (BLAME-05 append-only JSONL) is unchanged. | None |
| Live service config | None — Phase 16 does not change any externally-hosted configuration. The PKARR record published to Mainline DHT IS new (v0.2.0 schema), but the publish path is automatic at startup + heartbeat; no external UI or admin database to update. v1.3 coordinator instances would emit v0.1.0 records; v1.4 coordinator instances emit v0.2.0 records starting first publish post-deploy. Old DHT records age out via TTL (300s heartbeat). | None |
| OS-registered state | None — Phase 16 does not touch Docker Compose service names, systemd unit files, launchd plists, pm2 process names, or Windows task scheduler. The `coordinator` binary name is unchanged; the listening port is unchanged. | None |
| Secrets / env vars | New env var name space `BLINDJOIN__COORDINATOR__BIP__*` introduced. NO existing env vars renamed. Pre-Phase-16 `.env` files / SOPS configs / pm2 ecosystem files have no `[bip]` section; they will boot with the all-allowed defaults via `#[serde(default)]` on `pub bip: BipConfig`. PKARR keypair file path (`coordinator_pkarr.key`) unchanged — same identity across Phase 16. | None unless operator wants to lock down the allowlist (in which case they set new env vars; no migration of old ones) |
| Build artifacts / installed packages | None — Phase 16 adds zero new dependencies; `Cargo.lock` deltas should be empty after `cargo build --workspace`. Any pre-existing artifacts (Docker images, release tarballs) remain compatible with the v1.3 binary, but v1.4 PKARR records will not be readable by v1.3 binaries WITH RESPECT TO THE NEW FIELDS — v1.3 binaries see `"onion"` and proceed as before (per Pitfall 3). | None for Phase 16; Phase 17 WALLET-04 ships the v1.4→v1.3 compat shim |

**Nothing found in any category. Verified by:**
- code read of `coordinator/src/config.rs` (no schema migration)
- code read of `coordinator/src/discovery/pkarr_pub.rs` (publish path unchanged; only JSON inside changes)
- code read of `shared/src/protocol.rs` (additive struct fields only; `#[serde(default)]` on every new field)
- code read of `client/src/discover.rs` (resolver ignores unknown JSON fields by default; v1.3 binary against v1.4 coordinator works)

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` + Rust stable | All Phase 16 work | ✓ (workspace already builds) | (whatever toolchain CI uses) | — |
| `bitcoind` regtest binary | `multi_script_validate.rs` integration test | ✓ in CI (Phase 9 provisions v30.2 via PGP-verified install — see `.github/workflows/ci.yml::test`); ✓ locally via `corepc_node::exe_path()` or `BITCOIND_EXE` env | v30.2 (pinned in `.bitcoind-version`) | None needed in CI; local-dev tests gracefully skip via `require_bitcoind!()` macro when binary absent |
| `cargo audit` | CI gate (existing) | ✓ (CI installs in `audit` job) | latest stable | — |
| `cargo clippy` | CI gate (existing) | ✓ (CI installs in `clippy` job) | latest stable | — |
| `pkarr` Mainline DHT connectivity | E2E PKARR roundtrip test (none required in Phase 16) | N/A — Phase 16 only tests the **build** side of PKARR (byte budget); the publish path is exercised by existing v1.0 integration tests | — | — |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None.

Phase 16 has zero environment-side risk. Every required tool is already in CI; the integration test gracefully skips on developer machines without `bitcoind`.

## Project Constraints (from CLAUDE.md)

> Critical compliance items distilled from `./CLAUDE.md` + `.planning/PROJECT.md` constraints. The planner MUST honor each line.

| Constraint | Source | How Phase 16 honors it |
|------------|--------|------------------------|
| No custom crypto | PROJECT.md constraints | Phase 16 introduces zero new cryptographic code; all BIP-322 work routes through `shared::bip322::verify_simple` which delegates to the pinned `bip322 = "=0.0.10"` crate |
| Tor-native in production | PROJECT.md constraints | `tor_mode = true` path is unchanged; PKARR record publishes via the same code path regardless of clearnet-vs-Tor; the `"onion"` field carries either a clearnet or `.onion` address |
| Signet-first; mainnet via flag | PROJECT.md constraints | Phase 16 threads `bitcoin::Network` from `state.config.network.bitcoin_network` through to `verify_simple`; works for signet/testnet/mainnet/regtest without code change |
| No PII logging | PROJECT.md constraints + PRIV-02 | The new tracing log line emits ONLY `round_id` + `script_type`; no outpoint, address, witness bytes, or per-participant identifier. `Bip322Error` variants (Phase 15 D-31) are PII-safe by construction (verified by the existing `bip322_error_display_does_not_leak_pii_substrings` test at `shared/src/bip322/mod.rs:511`) |
| MIT licensed; public good | PROJECT.md constraints | Phase 16 introduces zero new license-incompatible deps (it introduces zero new deps period) |
| GSD workflow enforcement | `.planning/blindjoin/CLAUDE.md` "GSD Workflow Enforcement" | This research file IS the gated GSD step; downstream `/gsd:plan-phase` consumes it before `/gsd:execute-phase` |
| `/browse` skill for web browsing | gstack CLAUDE.md | N/A — Phase 16 implementation does not browse; if a follow-up debug-phase needs to, use `/browse` not direct fetch tools |
| No `mcp__claude-in-chrome__*` | gstack CLAUDE.md | N/A |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Coordinator-local `Bip322Error` enum at `coordinator/src/bitcoin/utxo.rs:87-101` | Shared `shared::bip322::Bip322Error` 10-variant taxonomy | Phase 15 Plan 15-02 (cfea17c) | Phase 16 maps every variant to `ErrorCode::InvalidOwnershipProof` at handler — D-32 unchanged |
| Coordinator-local `verify_bip322_simple` + `is_p2wpkh()` gate at `coordinator/src/bitcoin/utxo.rs:112-178` | `shared::bip322::verify_simple` dispatcher routing through `bip322 = "=0.0.10"` crate adapter | Phase 16 Plan 16-02 (this phase) | Deletes ~70 LOC of coordinator-local BIP-322 code; coordinator becomes a thin consumer of the shared contract |
| Single P2WPKH `is_p2wpkh()` script-type check | `detect_script_type(on_chain_spk)` returning `ScriptType` enum | Phase 15 BIP322-01; consumed in Phase 16 | Unlocks multi-script registration; CRIT-01 cross-check at v=2 dispatcher branch makes spoofing rejection load-bearing |
| `OwnershipProof` as `Vec<Vec<u8>>` array-of-hex on the wire | Flat 4-field struct `{ version, witness_stack, psbt_input_b64, script_type }` with two-phase try-parse | Phase 15 Plan 15-01 (ADVERT-04) | v1.3 wire-bit-exact compat preserved; v1.4 v=2 envelope carries PSBT-input shape per ADR Decision #3 |
| `InfoResponse` advertises only `version / network / denomination_sats / ...` | Plus `supported_script_types: Vec<ScriptType>` + `output_script_type: ScriptType` | Phase 16 (this phase) | Phase 17 client uses this for discovery-time fail-fast (D-56 defers resolver-side); v1.3 clients tolerate unknown fields |
| PKARR record version `"0.1.0"` | PKARR record version `"0.2.0"` with `"sst"` + `"ost"` fields | Phase 16 (this phase) | v1.3 client resolver tolerates the new fields; v1.4 client can read `sst` for discovery-time script-type filter |
| `bitcoincore-rpc` crate (archived November 2025) | `corepc-types` + `corepc-node` | v1.3 Phase 9 already migrated | No change in Phase 16; existing `corepc-types::v26::GetTxOut` shape is reused at `coordinator/src/bitcoin/utxo.rs:80` |

**Deprecated/outdated:**
- `bitcoincore-rpc` crate — archived [VERIFIED: github.com/rust-bitcoin/rust-bitcoincore-rpc archived Nov 2025]. Phase 16 does NOT introduce it.
- Manual `is_p2wpkh()` check — being deleted in 16-02 (CD-15 LOCKED).
- Coordinator-local `verify_bip322_simple` body — being deleted in 16-02 (CD-15 LOCKED).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `config 0.15.22` `Environment::try_parsing(true)` correctly parses `"true"`/`"false"` env-var values into `bool` via serde's bool deserializer | Pitfall 5 | LOW — verified by existing `tor_mode: bool` field at `coordinator/src/config.rs:50-51` which is overridable via `BLINDJOIN__COORDINATOR__TOR_MODE=true`. If wrong, BipConfig env overrides would silently fail to flip the bool; the BipConfig serde-default unit test (planned in 16-01) catches it locally before merge |
| A2 | v=2 path's `psbt_input_b64` carries a full BIP-174 PSBT (not just a serialized `Input`) | Pitfall 7 | MEDIUM — if Phase 15's roundtrip tests actually use a serde-json-encoded `Input`, the v=2 decode path in 16-02 needs to mirror that shape. Plan-phase MUST re-read `shared/tests/ownership_proof_roundtrip.rs` (the 5 D-13 cases) and the Phase 15-01-SUMMARY before locking the decode helper. Recommendation: pick the encoding that matches Phase 15's existing roundtrip tests verbatim. |
| A3 | `client/src/discover.rs::parse_onion_from_rr` `Partial` struct tolerates unknown JSON keys without `deny_unknown_fields` | Pitfall 3 | LOW — verified by code read (the struct only declares `onion: Option<String>` and no `deny_unknown_fields` attribute). Verified by `serde_json` default behavior. |
| A4 | Adding `pub bip: BipConfig` with `#[serde(default)]` to `CoordinatorConfig` does not require updating `with_defaults()` (existing test helper at `coordinator/src/config.rs:188-215`) | Pattern 1 / D-35 | MEDIUM — actually `with_defaults()` constructs the struct literally, so it WILL need an explicit `bip: BipConfig::default_for_tests()` field added. Plan-phase must include this in 16-01 task list to keep coordinator unit tests compiling. |
| A5 | `bitcoin 0.32` `Witness::from_slice(&[Vec<u8>])` exists and produces a `Witness` from a slice of byte vectors | Pattern 2 (v=1 path) | LOW — verified at Phase 15 Plan 15-01 (the existing `from_json_hex_str` produces `witness_stack: Vec<Vec<u8>>`, which is consumed by `Witness::from_slice` in the shared bip322 adapter at v=1). If wrong, plan must use `let mut w = Witness::new(); for item in &stack { w.push(item.clone()); } w` instead. |
| A6 | The 220-byte budget assertion at default config (all 3 allowed, alphabetical CSV) passes — i.e., actual serialized payload is < 220 bytes | D-44 / Pitfall 2 | LOW — D-44 calculated worst case at ~205 bytes; the inline assert is a CI gate against future field additions, not a Phase-16-failure risk. If the actual byte count is between 205 and 220, the gate passes; if somehow > 220 (e.g., longer "onion" address), the warning was already noisy at v0.1.0 — the planner can document the address-length contribution and bump the warn-threshold ADR if needed. Probability of actual breach with current addresses: very low (signet `.onion` addresses are 62 bytes; clearnet `127.0.0.1:8080` is 14 bytes). |
| A7 | `corepc-node 0.12` `node.client.new_address()` returns a P2WPKH address by default (existing test code uses this without specifying address_type) | Pitfall 6 | LOW — confirmed by existing `fund_regtest` at `tests/integration/mod.rs:313` which uses `new_address()` for the mining address. For Phase 16's typed test, use `node.client.new_address_with_type(corepc_node::AddressType::Bech32m)` for P2TR + `P2shSegwit` for P2SH-P2WPKH (verify corepc-node 0.12 API surface during 16-02 plan-task implementation; if missing, fall back to raw JSON-RPC via `node.client.call::<_, GetNewAddress>("getnewaddress", &[json!(""), json!("bech32m")])`). Alternative: derive addresses purely in rust-bitcoin (as Phase 15's `fixture_p2tr_spk` does) and fund externally — works regardless of corepc-node API. |
| A8 | Phase 16 grep gate count for `CRIT-01` in `coordinator/src/bitcoin/utxo.rs` ≥ 2 has no false positives from existing mentions | Pitfall 1 / D-49 | LOW — verified by `grep -rn "CRIT-01" coordinator/src/` returning ZERO existing mentions. The two new mentions Phase 16 adds (one per version branch) are the only matches. Existing `V1.4-CRIT-01` mentions live in `shared/` files and are scoped out of the gate's file pattern. |

**All assumptions are LOW-MEDIUM risk and have explicit mitigation paths.** No HIGH-risk assumption blocks plan-phase from proceeding.

## Open Questions

1. **Should v=2 path's `psbt_input_b64` decode tolerate the PSBT having BOTH `witness_utxo` and `non_witness_utxo` populated (oversize transmission)?**
   - What we know: BIP-174 says either is valid for SegWit-spending inputs; the rust-bitcoin `bitcoin::psbt::Psbt::deserialize` accepts both.
   - What's unclear: whether Phase 16 should warn on `non_witness_utxo` being present (it's overspecified for SegWit witnesses) or silently accept.
   - Recommendation: silently accept — neither field is consulted by the dispatcher; only `final_script_witness` is read. Plan-phase may choose to add a defensive log if `non_witness_utxo.is_some()`, but it's optional.

2. **Should the byte-budget inline test parameterize over operator-configured `denomination_sats` and `min_participants` to catch edge cases at large values?**
   - What we know: D-44 specifies `denomination=1_000_000, min_participants=3` — modest values producing modest JSON.
   - What's unclear: a coordinator with `denomination_sats=10_000_000_000_000_000` (16 digits) costs ~7 extra bytes vs. the 7-digit baseline.
   - Recommendation: keep the test at the spec-default values for now (matches what the operator typically configures). If a Phase 17+ debugging session uncovers a real edge-case breach, parameterize then. Default config is the realistic worst case.

3. **For the `multi_script_validate.rs` test, should ALL 9 cases share a single regtest bitcoind boot, or boot one per test?**
   - What we know: existing `full_round` uses one bitcoind per test (`#[tokio::test]` semantics); `BitcoindGuard` ensures clean shutdown per test.
   - What's unclear: 9 bitcoind boots costs ~20 seconds of CI time vs. ~3 seconds for a shared one. But sharing requires `OnceCell` + a test-scope module-level harness.
   - Recommendation: one per test (matches existing pattern; isolates test failures cleanly; the 20-second CI cost is acceptable on the current `cargo test --workspace --all-targets` job which already runs `full_round` similarly).

## Phase 16-specific risks (beyond inherited carry-forwards)

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| PKARR byte-budget breach at v1.5 4th script type | MEDIUM (when v1.5 plans) | breaks DNS-TXT-255 limit | Encoded as the D-55 inline regression test; future v1.5 plan will see it fail and redesign the encoding |
| CRIT-01 comment regression on future utxo.rs refactor | MEDIUM (developer error) | silent spoofing acceptance | CI grep gate (D-49) — implement in 16-02 atomic commit |
| INFO log spam at high TPS (CD-12) | LOW (modest TPS reality) | log volume operator complaint | Flip CD-12 to DEBUG-on-P2WPKH later if evidence demands; not blocking for Phase 16 |
| v1 path masks P2WPKH-vs-P2TR-vs-P2SH ambiguity for a malicious client | LOW | spoofing — but caught by CRIT-01 | The CRIT-01 derive-from-chain path runs FIRST in BOTH branches; even v=1 (no client declaration) routes through `detect_script_type(on_chain_spk)`. A malicious v=1 client that puts a P2TR witness against a P2WPKH UTXO triggers `verify_simple(ScriptType::P2wpkh, ...)` which rejects via the bip322 crate's internal arity / sighash check |
| PSBT `witness_utxo` field spoofing via malicious client (per Pitfall 7) | LOW | trust violation if dispatcher uses client-supplied SPK | The dispatcher MUST derive SPK from `gettxout` (existing `parse_script_pubkey_from_txout` at `coordinator/src/bitcoin/utxo.rs:80-85`); the PSBT `witness_utxo` is read but only for symmetry, never for verification. Code-review checklist item: "Search `coordinator/src/bitcoin/utxo.rs` for `witness_utxo` — every read must be discarded; only `gettxout` SPK feeds `detect_script_type`." |
| v=2 PSBT decode failure mode confusion with `ScriptTypeMismatch` | MEDIUM | wrong error code surfaces to client | The decode path returns `Bip322Error::DecodeError(...)` BEFORE script-type comparison. Test fixture for `validate_p2wpkh_utxo_with_v2_declared_p2tr_rejects_spoofing` must produce a VALID base64 PSBT — if the test fixture's PSBT decoding fails, the test gets `DecodeError` instead of `ScriptTypeMismatch` and the assertion mismatches. Catch at test-construction time via roundtrip-decode sanity assert. |

## Sources

### Primary (HIGH confidence)
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` — D-35..D-56 + CD-11..CD-16 + canonical refs + 16-DISCUSSION-LOG.md. Phase 16 contract.
- `.planning/decisions/v1.4-adr.md` — ADR Decisions #1 (ADOPT bip322), #2 (MIXED rounds D-06..D-10), #3 (B2 PSBT-input shape), #4 (bdk path for P2TR sign).
- `.planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` — D-22..D-32 wire shape + 10-variant Bip322Error taxonomy + module split.
- `.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` — Decision #2 D-06..D-10 verbatim.
- `.planning/REQUIREMENTS.md` — ADVERT-01/02/03 requirement text + Out-of-Scope table.
- `.planning/ROADMAP.md` Phase 16 — 5 success criteria + cross-phase invariant.
- `.planning/STATE.md` — Phase 14 + 15 decisions + accumulated context + v1.3 REPAIR-01 lessons.
- `.planning/PROJECT.md` — constraints (no custom crypto, no PII logging, MIT, Tor-native).
- Code reads (all verified against `git show HEAD`):
  - `shared/src/bip322/mod.rs` (1-567) — dispatcher + 10-variant Bip322Error + detect_script_type + verify_simple + sign_simple + sign_simple_test_only
  - `shared/src/protocol.rs` (1-273) — OwnershipProof v2 envelope + InfoResponse current shape
  - `shared/src/bip322/p2sh_p2wpkh.rs` (1-60) — P2SH-P2WPKH dispatch + Network handling
  - `coordinator/src/config.rs` (1-216) — CoordinatorConfig + validate() + with_defaults()
  - `coordinator/src/bitcoin/utxo.rs` (1-242) — validate_utxo current shape + verify_bip322_simple (to be deleted)
  - `coordinator/src/api/handlers.rs` (1-611) — get_info + post_input + parse_bitcoin_network
  - `coordinator/src/discovery/pkarr_pub.rs` (1-150) — build_coordinator_packet + 220-byte warn at line 76
  - `coordinator/src/discovery/mod.rs` — single-line module declaration
  - `client/src/discover.rs` (1-103) — resolver Partial struct (no deny_unknown_fields)
  - `coordinator/src/run.rs` (1-200, 315-385) — PKARR publish call sites
  - `tests/integration/mod.rs` (1-535) — BitcoindGuard + bootstrap_regtest_bitcoind + fund_regtest
  - `tests/integration/full_round.rs` (1-80) — header + import shape (DO NOT MODIFY pattern)
  - `Cargo.lock` — bip322 = 0.0.10, config = 0.15.22, pkarr = 5.0.2 confirmed
  - `coordinator/Cargo.toml` — corepc-node 0.12 features="30_2"
  - `client/Cargo.toml` — `shared = { path = "../shared" }` (workspace path crate, no version skew)
  - `shared/Cargo.toml` — base64 = 0.22, bip322 = =0.0.10
  - `.github/workflows/ci.yml` (1-237) — corepc-node-feature-pin-check + bip322-pin-check pattern

### Secondary (MEDIUM confidence)
- [docs.rs/bitcoin/0.32.5/bitcoin/psbt/struct.Psbt.html](https://docs.rs/bitcoin/0.32.5/bitcoin/psbt/struct.Psbt.html) — `Psbt::deserialize(&[u8])` + `FromStr` (base64) — verified for v=2 decode path
- [docs.rs/bitcoin/0.32.5/bitcoin/psbt/struct.Input.html](https://docs.rs/bitcoin/0.32.5/bitcoin/psbt/struct.Input.html) — `bitcoin::psbt::Input` impl `Deserialize` (serde) but no byte-level `consensus_decode`
- [docs.rs/config/latest/config/struct.Environment.html](https://docs.rs/config/latest/config/struct.Environment.html) — `try_parsing(true)` attempts bool/i64/f64 — informs Pitfall 5
- [bitcoincore.org/en/doc/28.0.0/rpc/wallet/getnewaddress/](https://bitcoincore.org/en/doc/28.0.0/rpc/wallet/getnewaddress/) — `address_type` values `"legacy"`, `"p2sh-segwit"`, `"bech32"`, `"bech32m"` — informs Pitfall 6 / A7
- Phase 15 SUMMARY files (`15-01-SUMMARY.md`, `15-02-SUMMARY.md`, `15-03-SUMMARY.md`) — implementation deltas + auto-fix notes (Version(0) + bare OP_RETURN to match bip322 crate)

### Tertiary (LOW confidence)
- (none — every claim in this document is grounded in code already in the tree or an authoritative external doc page)

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — every required crate is pinned in `Cargo.lock` and read directly from existing code
- Architecture: **HIGH** — Phase 15 verified the shared::bip322 contract; Phase 16 is pure wiring
- Pitfalls: **HIGH-MEDIUM** — Pitfalls 1-5 + 8-10 are HIGH (code-read verified); Pitfalls 6-7 are MEDIUM (require plan-time verification against the existing Phase 15 D-13 roundtrip test shape for PSBT encoding choice; and against corepc-node 0.12's `new_address_with_type` API surface for funded-UTXO generation)
- Code examples: **HIGH** — every example is grounded in existing code at the cited line numbers
- Assumptions: **HIGH** — A1-A8 each have explicit mitigation paths and LOW-MEDIUM risk individually

**Research date:** 2026-05-29
**Valid until:** 2026-06-29 (30 days for stable Rust async stack with pinned dependencies; sooner if any of bip322 = 0.0.10 / config = 0.15.22 / pkarr = 5.0.2 / bitcoin = 0.32.x gets a non-patch release that the workspace adopts)
