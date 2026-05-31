# Phase 17: Client Multi-Script Wallet & Discovery - Research

**Researched:** 2026-05-30
**Domain:** Rust client wallet (bdk_wallet 2.3) — BIP-322 multi-script signing, BIP-84/86/49 descriptor templates, PKARR resolver shim, v1.4→v1.3 backwards-compatibility shim
**Confidence:** HIGH (every load-bearing claim verified against in-tree source or vendored bdk_wallet 2.3.0)

## Summary

Phase 17 wires the v1.4 multi-script BIP-322 contract — settled in Phases 14 (ADR), 15 (shared crate API), and 16 (coordinator dispatcher + PKARR producer) — into the client half of the protocol. The locked decisions (CONTEXT D-57..D-80) constrain almost every implementation choice; this research surfaces the **few remaining unknowns** the planner needs to resolve and **five critical pitfalls** that would otherwise sink the plan.

The most consequential finding is that **CONTEXT D-69's wire-shape language is wrong by one envelope layer**: the coordinator's v=2 decoder at `coordinator/src/bitcoin/utxo.rs:212-225` calls `bitcoin::psbt::Psbt::deserialize` against a **full BIP-174 PSBT** (one input, zero outputs), not against a bare `bitcoin::psbt::Input` byte stream. `bitcoin::consensus::serialize(&psbt::Input)` is the wrong serializer and would not roundtrip. The encoder must build a `Psbt::from_unsigned_tx(...)` envelope, populate `psbt.inputs[0].final_script_witness` (and `final_script_sig` for P2SH-P2WPKH), and `B64.encode(psbt.serialize())` it — exactly mirroring the test helper at `tests/integration/multi_script_validate.rs:56-74`. This is the single most important correction the planner must adopt.

Second-most consequential: **bdk_wallet's `Bip84`/`Bip86`/`Bip49` descriptor templates auto-select coin type per network** (coin=1' for testnet/signet, coin=0' for mainnet) per `bdk_wallet-2.3.0/src/descriptor/template.rs:319,397,476`. The v1.3 client at `client/src/wallet.rs:140-141` hard-codes `84'/0'/0'` regardless of network. Phase 17's choice between `format!("wpkh({xprv}/84'/0'/0'/0/*)", ...)` (D-58's literal-template path, preserves v1.3 byte-equivalence) and `bdk_wallet::template::Bip84(xprv, KeychainKind::External)` (cleaner, but coin=1' on signet — breaks v1.3 wallet round-trip on the cross-phase invariant) is forced by D-66's "preserve byte-exact v1.3 wallet addresses" requirement. **Verdict: keep the literal-template path D-58 specifies; do NOT switch to bdk_wallet templates.**

**Primary recommendation:** Plan three sequential atomic plans per D-77 ordering (17-01 descriptors + script_type field; 17-02 sign dispatcher + v1/v2 envelope encoder; 17-03 discovery fail-fast + compat shim). The single PSBT-construction path for both P2TR and P2SH-P2WPKH (CD-24, uniform descriptor-wallet bdk path) drops the per-script complexity. The PKARR resolver extension is structurally trivial — the existing `_blindjoin` TXT JSON parser at `client/src/discover.rs:67` already handles the JSON shape; Phase 17 adds 3 fields and a typed error enum.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CLI argument parsing (`--type`) | Client / config layer | — | Existing `ClientConfig` struct at `client/src/config.rs`; clap's `value_parser` handles the ScriptType serde shape |
| Wallet descriptor construction (BIP-84/86/49) | Client / wallet layer | — | `BdkClientWallet::{generate, from_descriptor}`; descriptor type is wallet's intrinsic property |
| Script-type detection from descriptor | Client / wallet layer | — | Wallet KNOWS its descriptor type explicitly; storing `script_type: ScriptType` field on `BdkClientWallet` is cleaner than redetecting from SPK |
| BIP-322 sign dispatch | Client / wallet layer | shared::bip322 (P2WPKH only) | Wallet owns "sign for my own UTXO" responsibility; routes through bdk PSBT-sign for descriptor wallets, shared::bip322::sign_simple for legacy WIF wallets |
| BIP-322 witness verification (sign-roundtrip tests only) | shared::bip322 (LOCKED) | — | Phase 15 contract; Phase 17 only CONSUMES `verify_simple` for roundtrip assertions |
| v1/v2 OwnershipProof envelope construction | Client / round::input layer | — | round/input.rs knows what wire shape the coordinator expects (per coordinator capabilities); wallet only produces the signed proof |
| PKARR record decoding + capability flags | Client / discover layer | — | Resolver layer; extends `BlindjoinRecord` struct with v0.2.0-specific fields |
| Discovery-time fail-fast (pre-Tor) | Client / discover layer | — | Must happen BEFORE `tor::init_tor`; structurally enforced by the existing `main.rs:60` PKARR call preceding `main.rs:68` tor call |
| v1.4→v1.3 compat shim trigger | Client / discover layer (detect) + round/input layer (emit) | — | Two coupled locations: detect legacy at discover.rs (set `is_legacy=true`), branch envelope at round/input.rs (emit v1 OwnershipProof) |
| Tor circuit management | Client / tor layer | — | Unchanged from v1.3; Phase 17 does NOT touch this layer |

## User Constraints (from CONTEXT.md)

> Phase 17 CONTEXT.md was populated via `--auto` (autonomous decisions per recommended defaults). All decisions D-57..D-80 are LOCKED inputs from the planner's perspective; CD-17..CD-24 are areas where the plan-phase has bounded discretion. Discuss-phase did NOT challenge any default, so the full set is treated as user-confirmed.

### Locked Decisions (D-57..D-80)

- **D-57:** `--type` CLI flag with env-var `BLINDJOIN_SCRIPT_TYPE`; default `p2wpkh`; lowercase kebab-case wire form matching `ScriptType`'s serde rename.
- **D-58:** Per-type descriptor templates — `wpkh({xprv}/84'/0'/0'/0/*)` (P2WPKH/BIP-84), `tr({xprv}/86'/0'/0'/0/*)` (P2TR/BIP-86), `sh(wpkh({xprv}/49'/0'/0'/0/*))` (P2SH-P2WPKH/BIP-49). **Coin = 0' across ALL networks** (preserves v1.3 byte-exact wallet addresses).
- **D-59:** Network parameter handling unchanged — bdk_wallet's `Wallet::create(...).network(...)` handles signet/testnet/mainnet across all 3 descriptor types.
- **D-60:** `generate` per-type prominent fund-address output via existing `peek_address(External, 0)` pattern; new banner line `Script type: p2tr (BIP-86)`.
- **D-61:** `from_wif` stays P2WPKH-only legacy path; do NOT extend with script_type parameter.
- **D-62:** `script_type: ScriptType` field on `BdkClientWallet`, set at construction; single source of truth.
- **D-63:** Construction-time descriptor-vs-`--type` mismatch check in `from_descriptor`; fail-fast.
- **D-64:** New method `wallet.sign_bip322(message: &str) -> Result<Bip322SignedProof>`; `Bip322SignedProof` is a small client-internal struct (NOT a wire type), 4 fields: `witness_stack`, `witness`, `final_script_sig: Option<ScriptBuf>`, `script_type`.
- **D-65:** Per-script sign dispatch body — P2WPKH (WIF wallet only) via `shared::bip322::sign_simple(P2wpkh, ...)`; ALL descriptor wallets (incl. P2WPKH descriptor) route through bdk_wallet PSBT-sign with `SignOptions { trust_witness_utxo: true }` (CD-24); P2TR witness extraction prefers `final_script_witness` over `tap_key_sig` (Sprint-0-B finding); P2SH-P2WPKH extracts BOTH `final_script_witness` AND `final_script_sig`.
- **D-66:** Network parameter sourced from `wallet.network` (already cached at `client/src/wallet.rs:19`). No new field.
- **D-67:** No manual P2TR fallback — D-15's 80-LOC budget retired per ADR Decision #4 / Sprint-0-B PASS.
- **D-68:** v1/v2 envelope branch lives in `round::input::register_input`; inputs are `coordinator_info.capabilities.is_legacy`, `wallet.script_type()`, signed proof.
- **D-69:** `build_v2_psbt_input(signed) -> Result<bitcoin::psbt::Input>` private helper in `client::round::input`. **CORRECTION (this research):** the helper must return a full `bitcoin::psbt::Psbt` (one input, zero outputs), NOT a bare `psbt::Input` — see Common Pitfall 1.
- **D-70:** `witness_stack` populated in BOTH v1 and v2 envelopes for symmetry (Phase 15 D-22).
- **D-71..D-72:** Extended `CoordinatorInfo` with `capabilities: CoordinatorCapabilities`; `discover_coordinator(pubkey, required_script_type) -> Result<CoordinatorInfo, DiscoveryError>` signature change. Typed `DiscoveryError` enum with `InvalidPubkey`, `NotFound`, `MissingOnion`, `UnsupportedScriptType`, `UnsupportedOutputScriptType`, `MalformedRecord` variants.
- **D-73:** Richer `parse_blindjoin_record(rr) -> Option<BlindjoinRecord>` replacing `parse_onion_from_rr`. `BlindjoinRecord { version, onion, sst: Option<String>, ost: Option<String> }`. **Note:** Phase 16 compactified field names from `version` → `v` etc. (see Open Question 1).
- **D-74:** Pre-Tor placement verified by code location at `client/src/main.rs:57-69`; resolver returns before `tor::init_tor`.
- **D-75:** No double-check at `/round/info`; PKARR `sst` is the load-bearing pre-Tor signal.
- **D-76:** `output_script_type` mismatch ALSO fails at discovery (WALLET-03 sibling check).
- **D-77:** 3-plan ordering, sequential `17-01 → 17-02 → 17-03`. 17-01 = WALLET-01 (descriptors); 17-02 = WALLET-02 + WALLET-04 (encoder); 17-03 = WALLET-03 + WALLET-04 (discovery).
- **D-78:** NEW `tests/integration/multi_script_client.rs` with 9 named tests + `client/tests/wallet_sign_roundtrip.rs` for sign↔verify roundtrips without bitcoind.
- **D-79:** Liquidity-bot real-binary v1.3-vs-v1.4 integration test deferred to Phase 18.
- **D-80:** CRIT-01 client-side grep gate — `grep -c "CRIT-01" client/src/round/input.rs` ≥ 1.

### Claude's Discretion (CD-17..CD-24)

- **CD-17:** Lowercase kebab-case only for `--type`; no case-insensitive aliasing unless plan-phase argues for ergonomics.
- **CD-18:** `Bip322SignedProof` lives in `client::wallet` (producer module).
- **CD-19:** `wallet.sign_bip322` is `pub(crate)` (round/input is sole consumer).
- **CD-20:** DELETE old `generate_bip322_witness` helper at `client/src/round/input.rs:115-149` inside 17-02 atomic commit.
- **CD-21:** `CoordinatorCapabilities` is a public struct on `CoordinatorInfo`; `main.rs` reads `info.capabilities.is_legacy` directly.
- **CD-22:** Single-underscore `BLINDJOIN_SCRIPT_TYPE` env var (matches existing client convention).
- **CD-23:** Split `UnsupportedScriptType` vs `UnsupportedOutputScriptType` error variants.
- **CD-24:** UNIFORM bdk PSBT-sign path for ALL descriptor wallets (P2WPKH descriptor wallets ALSO route through bdk PSBT-sign, not through `shared::bip322::sign_simple`). The WIF wallet stays on the `secret_key_for_signing` + `shared::bip322::sign_simple` path.

### Deferred Ideas (OUT OF SCOPE)

- Manual P2TR sign fallback (`shared/src/bip322/p2tr.rs::sign_p2tr_keypath`, 80-LOC) — RETIRED for v1.4 per ADR #4. v1.5 swap target if bdk regresses.
- P2WSH multisig BIP-322 client support — v1.5+ per REQUIREMENTS Future Requirements.
- Cross-implementation differential test fixtures (TEST-EXT-01) — v1.5+ per REQUIREMENTS.
- Regtest on-chain anchor test (TEST-EXT-02) — v1.5+.
- Automated backwards-compat integration matrix (TEST-EXT-03) — v1.5+; Phase 17 covers WALLET-04 informally via stubs; Phase 18 INTEG-01 covers real-binary.
- BIP-44-correct testnet coin-type indexing (`m/84'/1'/...`) — v1.5+ migration; Phase 17 keeps `0'` per D-66.
- `--type` short form (`-t`) — plan-phase discretion per CD-17; defer if naming collides.
- `bdk_wallet = "=2.3.x"` exact-pin tightening — v1.5+ candidate.
- CSV-vs-array PKARR record format reconsideration — v1.5 problem; Phase 17 inherits CSV decoder unchanged.
- Per-coordinator output-type selection UX in `--generate-wallet` — v1.5 ergonomic polish.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WALLET-01 | Client wallet supports BIP-84 / BIP-86 / BIP-49 descriptor templates via `--type` flag | Standard Stack §"Descriptor Templates"; D-58 verbatim template strings; bdk_wallet 2.3 supports `wpkh`/`tr`/`sh(wpkh(...))` via `Wallet::create(...)` (verified at `bdk_wallet-2.3.0/src/descriptor/template.rs:319,397,476`) |
| WALLET-02 | Client signs BIP-322 proofs for all 3 script types via dispatcher | Sprint-0-B PoC (`.planning/research/sprint-0-B.md`); D-65 dispatch body; Code Examples §"BIP-322 Sign for P2TR" (verbatim Sprint-0-B 8-step sequence); coordinator's v=2 decoder at `coordinator/src/bitcoin/utxo.rs:212-225` defines the wire shape encoder must match |
| WALLET-03 | Client reads `sst` from PKARR record BEFORE opening Tor circuit; rejects on mismatch with typed error | D-72/D-74 fail-fast at resolver; structural ordering verified at `client/src/main.rs:57-69` (PKARR discover at line 58 returns BEFORE `tor::init_tor` at line 68); existing `parse_onion_from_rr` pattern at `client/src/discover.rs:67` extends to `parse_blindjoin_record` |
| WALLET-04 | Client detects pre-`0.2.0` PKARR + emits legacy v1 OwnershipProof for P2WPKH-vs-v1.3 cell of compat matrix | D-73 `BlindjoinRecord` parser with `#[serde(default)]` on `sst`/`ost`; D-68 v1 envelope branch in `register_input`; CD-7 byte-identity branch at `shared/src/protocol.rs:239` already emits v1.3 array-of-hex form when `version == 1 && psbt_input_b64.is_none() && script_type.is_none()` |

## Standard Stack

### Core (already in tree — no new deps)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `bdk_wallet` | =2.3.0 | PSBT signing for P2TR + P2SH-P2WPKH (and P2WPKH descriptor wallets via CD-24); BIP-32 key derivation | [VERIFIED: in tree, exact-pinned per Phase 12 carry-forward] |
| `bitcoin` | 0.32.8 | `Psbt::from_unsigned_tx`, `Psbt::serialize/deserialize`, `Witness`, `Address` primitives | [VERIFIED: workspace pin per sprint-0-A.md:199] |
| `shared::bip322` | (workspace internal) | `sign_simple(P2wpkh, ...)`, `verify_simple(...)`, `ScriptType`, `Bip322Error`, `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign` | [VERIFIED: Phase 15 LOCKED API at `shared/src/bip322/mod.rs`] |
| `shared::protocol` | (workspace internal) | `OwnershipProof` flat struct with `to_json_hex_str` CD-7 branch | [VERIFIED: Phase 15 LOCKED at `shared/src/protocol.rs`] |
| `base64` | 0.22 | b64 encode/decode for `psbt_input_b64` | [VERIFIED: in tree per Phase 15 Plan 15-01] |
| `pkarr` | latest (pinned in workspace) | DHT `resolve_most_recent`; existing dep at `client/src/discover.rs:1` | [VERIFIED: in tree] |
| `clap` | 4.x | `--type` flag parsing with `value_parser` | [VERIFIED: in tree at `client/src/config.rs:1`] |
| `serde` / `serde_json` | 1.x | `BlindjoinRecord` JSON decode; `ScriptType` derives serde | [VERIFIED: in tree] |
| `thiserror` | (in tree) | `DiscoveryError` typed enum (matches Phase 15/16 `Bip322Error` pattern) | [VERIFIED: existing convention] |
| `tracing` | 0.1 | WARN log for legacy-coordinator detection (CD-21) | [VERIFIED: in tree] |

### Supporting (test infrastructure only)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `corepc-node` | 0.12 + 30_2 feature | regtest bitcoind for `tests/integration/multi_script_client.rs` | Per-script-type funding via existing `fund_regtest_typed` helper at `tests/integration/mod.rs:616` (Phase 16 Plan 16-02 LANDED) |
| `tokio::test` | (via tokio) | async integration + unit tests | All Phase 17 tests follow `#[tokio::test]` pattern |

### Alternatives Considered (REJECTED — DO NOT use these in Phase 17)
| Instead of | Could Use | Why Rejected |
|------------|-----------|--------------|
| Literal descriptor `format!("wpkh({xprv}/84'/0'/0'/0/*)", ...)` | `bdk_wallet::template::Bip84(xprv, External)` | bdk template auto-selects coin=1' on testnet/signet (`bdk_wallet-2.3.0/src/descriptor/template.rs:397`); BREAKS v1.3 byte-equivalence required by D-66 |
| `bitcoin::consensus::serialize(&psbt::Input)` | `psbt::Input::serialize` (a BIP-174 key-value wire fn) | NEITHER will roundtrip through coordinator's `Psbt::deserialize`; the wire shape is a FULL PSBT, not a bare Input (see Common Pitfall 1) |
| Custom `ScriptType` re-decl in client | Import `shared::bip322::ScriptType` | Single source of truth; CRIT-01 invariant |
| New shared dep for P2TR sign | Use bdk_wallet PSBT-sign | Sprint-0-B PASS; ADR Decision #4 LOCKED |
| Manual `secp256k1::Secp256k1::sign_schnorr` for P2TR | bdk_wallet PSBT path | D-67 — manual fallback RETIRED for v1.4 |

**Installation:** No new dependencies needed. All required crates are already pinned in `Cargo.toml` workspace.

**Version verification:** Per the project's exact-pin discipline (v1.3 REPAIR-02 carry-forward #3 + Phase 14 ADR), Phase 17 adds ZERO new direct dependencies. Lockfile drift checks via `cargo audit` already gate this in CI. Skipped registry `npm view` / `pip index versions` calls — this is a Rust project consuming existing workspace pins.

## Package Legitimacy Audit

> Phase 17 introduces NO new external packages. All consumed crates are already pinned in the workspace `Cargo.toml` and were audited in earlier phases (sprint-0-A for `bip322`/`snafu`/`snafu-derive`; Phase 12 for `bdk_wallet 2.3`; v1.0+ for `bitcoin`/`tokio`/`clap`/`serde`/`base64`/`pkarr`/`tracing`/`thiserror`).

| Package | Registry | Source | Disposition |
|---------|----------|--------|-------------|
| (none added) | — | — | NO new package installs in this phase |

**Packages removed:** none (no installs to remove)
**Packages flagged:** none

*slopcheck was not run because Phase 17 installs zero new packages. The legitimacy gate is satisfied by absence of new package surface area.*

## Architecture Patterns

### System Architecture Diagram

```
                          ┌─────────────────────────────────────┐
                          │  CLI invocation: blindjoin-client   │
                          │  --type {p2wpkh|p2tr|p2sh-p2wpkh}   │
                          │  --pkarr-pubkey pk:... [--use-tor]  │
                          └────────────┬────────────────────────┘
                                       │
                                       ▼
        ┌───────────────────────────────────────────────────────────┐
        │  client::config::ClientConfig — clap parse                │
        │    + value_parser for ScriptType                          │
        │  → cfg.script_type: ScriptType                            │
        └────────────┬──────────────────────────────────────────────┘
                     │
        ┌────────────┴───────────────┐
        │                            │
        ▼                            ▼
   --generate-wallet?          --descriptor / --utxo-wif
        │                            │
        ▼                            ▼
   ClientWallet::generate(    ClientWallet::{from_descriptor|from_wif}
   utxo, network, script_type) ── (cfg.script_type → wallet.script_type)
        │                            │     ▲
        │                            │     │ D-63: construction-time mismatch check
        │                            │     │       (descriptor outer wrapper vs --type)
        ▼                            │
   print descriptors                  │
   exit(0)                            │
                                      ▼
              ┌─────────────────────────────────────────────┐
              │  client::discover::discover_coordinator(    │
              │    pkarr_pubkey, required_script_type=      │
              │      wallet.script_type()                   │
              │  ) — pre-Tor fail-fast (D-74)               │
              └─────┬───────────────────────────────────────┘
                    │
                    ▼  reads PKARR `_blindjoin` TXT record
              ┌─────────────────────────────────────────────┐
              │ PKARR resolver layer                        │
              │  pkarr::Client::resolve_most_recent(...)    │
              │   → BlindjoinRecord { v, onion, sst, ost }  │
              │   → CoordinatorCapabilities {               │
              │       record_version, is_legacy,            │
              │       supported_script_types,               │
              │       output_script_type }                  │
              └─────┬───────────────────────────────────────┘
                    │
              ┌─────┴────────────────────────────────────┐
              │  Capability check (D-72/D-76):           │
              │    if required NOT IN supported →        │
              │      Err(UnsupportedScriptType {...})    │
              │    if wallet_output_type ≠ ost →         │
              │      Err(UnsupportedOutputScriptType)    │
              └─────┬────────────────────────────────────┘
                    │ Ok(CoordinatorInfo { url, capabilities })
                    │
       (rejected) ◄─┘ ─────────────────► exits without opening Tor circuit
                    │
                    ▼  Ok path
              ┌─────────────────────────────────────────────┐
              │  client::tor::init_tor (only if --use-tor)  │
              │  → CoordinatorClient (clearnet OR Tor)      │
              └─────┬───────────────────────────────────────┘
                    │
                    ▼
              ┌─────────────────────────────────────────────┐
              │  poll_until_phase("input_reg") → InfoResponse│
              └─────┬───────────────────────────────────────┘
                    │
                    ▼
              ┌─────────────────────────────────────────────┐
              │  round::input::register_input(              │
              │    client, wallet, info, capabilities)      │
              │                                             │
              │  1. RSA pubkey hash verify                  │
              │  2. compute_blind_token_message             │
              │  3. blind                                   │
              │  4. signed = wallet.sign_bip322(message)    │
              │       │                                     │
              │       └─► dispatches by wallet.script_type: │
              │           ┌──────────────┬──────────────┐   │
              │           ▼              ▼              ▼   │
              │      WIF + P2WPKH  Descriptor wallet      │
              │           │       (ALL: P2WPKH/P2TR/      │
              │           │        P2SH-P2WPKH per CD-24) │
              │           │              │                │
              │           ▼              ▼                │
              │      shared::bip322::  bdk_wallet PSBT-   │
              │      sign_simple(     sign over BIP-322-  │
              │      P2wpkh, ...)    shaped PSBT          │
              │           │              │                │
              │           └─────┬────────┘                │
              │                 ▼                         │
              │           Bip322SignedProof {             │
              │             witness_stack, witness,       │
              │             final_script_sig: Option,     │
              │             script_type }                 │
              │                 │                         │
              │  5. if capabilities.is_legacy:           │
              │       OwnershipProof v=1 (witness_stack) │
              │     else:                                │
              │       OwnershipProof v=2 (               │
              │         psbt_input_b64 = B64.encode(     │
              │           Psbt::serialize) ◄── KEY!      │
              │         script_type = signed.script_type │
              │       )                                   │
              │       // CRIT-01: script_type from       │
              │       //   wallet (descriptor type)      │
              │  6. POST /round/input                     │
              │  7. unblind blind_sig                     │
              └─────┬───────────────────────────────────────┘
                    │
                    ▼
              (existing v1.3 flow continues: OUTPUT_REG → SIGNING → BROADCAST)
```

### Recommended Project Structure (DELTA from v1.3)

```
client/
├── src/
│   ├── config.rs              # +script_type: ScriptType field (D-57)
│   ├── discover.rs            # EXTEND: CoordinatorInfo + capabilities,
│   │                          #   DiscoveryError enum, parse_blindjoin_record
│   ├── wallet.rs              # +script_type field, +sign_bip322 method,
│   │                          #   +script_type() accessor, +Bip322SignedProof struct
│   ├── round/
│   │   └── input.rs           # REPLACE generate_bip322_witness call site,
│   │                          #   ADD v1/v2 envelope branch (D-68),
│   │                          #   ADD build_v2_psbt_input helper (D-69 corrected),
│   │                          #   DELETE generate_bip322_witness (CD-20)
│   └── main.rs                # pass cfg.script_type into wallet+discover,
│                              #   log WARN on legacy coordinator (CD-21)
└── tests/
    └── wallet_sign_roundtrip.rs   # NEW unit-test-style sign↔verify roundtrips
                                   #   for all 3 script types, no bitcoind needed

tests/integration/
├── full_round.rs              # UNCHANGED — v1.3 invariant gate
├── mod.rs                     # No changes (fund_regtest_typed already exists from 16-02)
└── multi_script_client.rs     # NEW — 9 D-78 named tests
```

### Pattern 1: BIP-322 PSBT-shaped Sign (P2TR + P2SH-P2WPKH + P2WPKH descriptor)

**What:** Build a BIP-322 to_spend/to_sign pair, wrap to_sign in a PSBT, populate `witness_utxo` with the canonical zero-value to_spend output, sign via bdk_wallet, extract from `final_script_witness` (with `tap_key_sig` fallback for P2TR / `partial_sigs` fallback for P2WPKH).

**When to use:** Every descriptor wallet sign path (CD-24 makes this uniform). WIF wallet stays on the `shared::bip322::sign_simple` path.

**Example (P2TR — verbatim from Sprint-0-B PASS PoC):**
```rust
// Source: .planning/research/sprint-0-B.md lines 86-285 (PASS verdict)
use bdk_wallet::signer::SignOptions;
use bitcoin::{Amount, TxOut, psbt::Psbt};
use shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign};

fn sign_bip322_p2tr_via_bdk(
    wallet: &mut bdk_wallet::Wallet,
    utxo_spk: &bitcoin::ScriptBuf,
    message: &[u8],
) -> anyhow::Result<bitcoin::Witness> {
    // 1-2: BIP-322 to_spend/to_sign via shared::bip322 helpers (V1.4-MOD-07 single source)
    let msg_hash = bip322_message_hash(message);
    let to_spend = build_bip322_to_spend(utxo_spk, &msg_hash);
    let to_sign = build_bip322_to_sign(&to_spend);

    // 3: wrap to_sign in PSBT
    let mut psbt = Psbt::from_unsigned_tx(to_sign)
        .map_err(|e| anyhow::anyhow!("from_unsigned_tx: {e}"))?;

    // 4: populate witness_utxo with canonical BIP-322 to_spend output (value = 0)
    psbt.inputs[0].witness_utxo = Some(TxOut {
        value: Amount::ZERO,
        script_pubkey: utxo_spk.clone(),
    });

    // 5: bdk sign with trust_witness_utxo (LOAD-BEARING per v1.3 Phase 12 lesson #1)
    #[allow(deprecated)]
    wallet.sign(&mut psbt, SignOptions {
        trust_witness_utxo: true,
        ..SignOptions::default()
    }).map_err(|e| anyhow::anyhow!("bdk sign: {e}"))?;

    // 6: extract witness — prefer final_script_witness (Sprint-0-B verdict),
    //    fall back to tap_key_sig if a future bdk version moves it back
    if let Some(w) = &psbt.inputs[0].final_script_witness {
        return Ok(w.clone());
    }
    if let Some(sig) = psbt.inputs[0].tap_key_sig {
        let mut w = bitcoin::Witness::new();
        w.push(sig.signature.serialize());
        return Ok(w);
    }
    Err(anyhow::anyhow!("bdk_wallet did not produce a P2TR witness"))
}
```

### Pattern 2: v=2 OwnershipProof Envelope — Full PSBT (NOT bare Input)

**What:** Build a full BIP-174 PSBT with one input + zero outputs; populate `final_script_witness` (and `final_script_sig` for P2SH-P2WPKH); base64-encode `Psbt::serialize()`.

**When to use:** Every v=2 OwnershipProof construction in `round::input::register_input`.

**Example (verbatim shape from `tests/integration/multi_script_validate.rs:56-74` which the coordinator dispatcher PROVES roundtrips):**
```rust
// Source: tests/integration/multi_script_validate.rs lines 56-74 (Phase 16-02 LANDED)
// Mirrors coordinator's decoder at coordinator/src/bitcoin/utxo.rs:212-225
use bitcoin::psbt::Psbt;
use bitcoin::{absolute, transaction, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, Witness};

fn build_v2_psbt_input_b64(
    witness: &Witness,
    final_script_sig: Option<&ScriptBuf>,  // Some for P2SH-P2WPKH only
) -> anyhow::Result<String> {
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
    Ok(base64::engine::general_purpose::STANDARD.encode(psbt.serialize()))
}
```

### Pattern 3: PKARR Resolver Capability Decode

**What:** Reuse the existing `parse_onion_from_rr` pattern at `client/src/discover.rs:67`; extend `Partial` struct with `v` (compact-renamed from `version` per Phase 16 B3 migration), `sst`, `ost` fields with `#[serde(default)]` defaults.

**Example:**
```rust
// Source: client/src/discover.rs:67-81 PATTERN extended for Phase 17
// CRITICAL: Phase 16 B3 migration renamed `version` -> `v` etc.
// See coordinator/src/discovery/pkarr_pub.rs:98-108 for canonical wire keys.
#[derive(Deserialize)]
struct BlindjoinRecord {
    #[serde(rename = "v", default = "default_legacy_version")]
    version: String,
    onion: String,
    #[serde(default)]
    sst: Option<String>,    // CSV: "p2sh-p2wpkh,p2tr,p2wpkh" (v0.2.0) | None (v0.1.0)
    #[serde(default)]
    ost: Option<String>,    // "p2wpkh" | "p2tr" | "p2sh-p2wpkh" (v0.2.0) | None (v0.1.0)
}

fn default_legacy_version() -> String { "0.1.0".into() }

fn parse_blindjoin_record(rr: &pkarr::dns::ResourceRecord<'_>) -> Option<BlindjoinRecord> {
    use pkarr::dns::rdata::RData;
    let RData::TXT(txt) = &rr.rdata else { return None };
    let s = String::try_from(txt.clone()).ok()?;
    serde_json::from_str::<BlindjoinRecord>(&s).ok()
}
```

### Anti-Patterns to Avoid

- **`bitcoin::consensus::serialize(&psbt::Input)`** for the v=2 envelope — this is what CONTEXT D-69 literally says, but the coordinator's decoder uses `Psbt::deserialize` against a full PSBT. Would not roundtrip. See Common Pitfall 1.
- **Using `bdk_wallet::template::Bip84` / `Bip86` / `Bip49`** — auto-selects coin=1' for testnet/signet, breaking v1.3 byte-equivalence. See Common Pitfall 2.
- **Reading `supported_script_types` from `/round/info` as the fail-fast signal** — `/round/info` is served over Tor (or whatever coordinator transport is in use); the fail-fast must happen at the PKARR resolver layer per D-75.
- **Re-declaring `ScriptType` in `client`** — break the single source of truth invariant. Import `shared::bip322::ScriptType`.
- **Populating `script_type` on the v2 envelope from the CLI flag directly** — would break CRIT-01 (the wallet's stored `script_type` is the wire source, set from descriptor at construction). See Common Pitfall 3.
- **Adding a redundant check at `/round/info` for script types** — per D-75, the PKARR `sst` is canonical; the `/round/info` field is informational only.
- **Re-introducing `tap_key_sig` extraction WITHOUT `final_script_witness` priority** — Sprint-0-B finding: bdk_wallet 2.3 puts the keyspend sig in `final_script_witness[0]` (cleared `tap_key_sig` at finalisation).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BIP-322 to_spend / to_sign construction | Custom tx-builder | `shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign}` | V1.4-MOD-07 single source of truth; Phase 15 [Rule 1 Bug] fixes (Version(0) + bare OP_RETURN) are LOAD-BEARING for correctness |
| P2WPKH BIP-322 sighash + ECDSA sign (WIF path only) | Manual `Secp256k1::sign_ecdsa` | `shared::bip322::sign_simple(ScriptType::P2wpkh, ...)` | Phase 15 production-ready; bit-exact with carried-forward path |
| P2TR keypath Schnorr sign | Manual `Secp256k1::sign_schnorr` over BIP-341 sighash | `bdk_wallet::Wallet::sign` with `SignOptions { trust_witness_utxo: true }` over BIP-322-shaped PSBT | ADR Decision #4 / Sprint-0-B PASS; D-67 RETIRED manual fallback for v1.4 |
| P2SH-P2WPKH BIP-143 sighash + redeem script construction | Manual P2SH unwrap + BIP-143 sighash | `bdk_wallet::Wallet::sign` (same as P2TR — bdk handles the P2SH wrap + redeem script population from the `sh(wpkh(...))` descriptor) | bdk_wallet 2.3 finalises sh(wpkh(...)) by populating BOTH `final_script_sig` (HASH160(redeem) push) AND `final_script_witness` (P2WPKH stack); verified via `bdk_wallet-2.3.0/src/descriptor/template.rs:320` |
| BIP-32 key derivation | Manual derivation walk | `bdk_wallet::Wallet::create(external_desc, internal_desc).network(...).create_wallet_no_persist()` | Existing v1.3 path at `client/src/wallet.rs:90-93` extends unchanged to `tr(...)` and `sh(wpkh(...))` |
| PSBT envelope wire shape | Custom binary framing | `bitcoin::psbt::Psbt::serialize` / `::deserialize` | Coordinator's decoder uses these EXACT functions; encoder MUST match (see Common Pitfall 1) |
| OwnershipProof JSON envelope | Custom JSON | `shared::protocol::OwnershipProof::to_json_hex_str` with the CD-7 byte-identity branch at line 239 | Single source of truth; emits bit-exact v1.3 array-of-hex when `version=1 && psbt_input_b64=None && script_type=None` |
| Legacy PKARR record parser | Custom serde struct in client | Phase 16 wire shape at `coordinator/src/discovery/pkarr_pub.rs:98-108` (compact-renamed: `v`, `onion`, `sst`, `ost`, plus `n`, `ds`, `mp`, `st`, `type`) | Coordinator is the wire-shape authority; client decoder mirrors it |
| Coordinator URL discovery | Manual DHT query | `pkarr::Client::resolve_most_recent` already used at `client/src/discover.rs:28` | Existing pattern extends; only the JSON decode body changes |
| WIF wallet sign path | New implementation | EXISTING `generate_bip322_witness` body lifted to `shared::bip322::p2wpkh::sign` (already done in Phase 15) | Phase 15-02 SUMMARY confirmed bit-exact |

**Key insight:** Phase 17 is overwhelmingly a CONSUMER of locked APIs from Phases 15 + 16. The only new production code is (a) a thin per-script sign dispatcher (~80-120 LOC), (b) a v1/v2 envelope branch (~15 LOC), (c) a PKARR record extension (~30 LOC), and (d) the CLI flag plumbing (~15 LOC). Custom solutions would re-introduce drift between encoder/decoder.

## Runtime State Inventory

> Phase 17 modifies CLIENT code only. No coordinator state, no databases, no on-chain registrations.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — Phase 17 client is single-use CLI; no persistent state. The Phase 13 wallet `descriptors.txt` is operator-managed, not auto-rotated; the generate-wallet flow writes new content per invocation. | No migration |
| Live service config | None — no service config changes. The coordinator's PKARR record producer (`sst`/`ost` fields) was added in Phase 16-03 and is already published; Phase 17 client consumes existing records. | None |
| OS-registered state | None — client is CLI invocation, not a registered service. | None |
| Secrets/env vars | NEW env var `BLINDJOIN_SCRIPT_TYPE` (D-57). Backwards-compat: missing env var defaults to `p2wpkh` per D-57 + CD-22; existing scripts that set no `--type` flag keep working unchanged. | Document in README (operator-facing); no migration |
| Build artifacts | None — pure source change. `cargo build -p client` produces an updated binary; no stale .egg-info / compiled artifacts to clean. | None |

**The canonical question:** *After every file in the repo is updated, what runtime systems still have the old string cached, stored, or registered?*

**Answer:** Nothing. Phase 17 is pure source code change; the new binary supersedes the old one. No data migration, no service re-registration, no scheduled-task updates.

## Common Pitfalls

### Pitfall 1: CONTEXT D-69's wire-shape language is wrong by one envelope layer
**What goes wrong:** D-69 says `build_v2_psbt_input(signed) -> Result<bitcoin::psbt::Input>` and shows `B64.encode(bitcoin::consensus::serialize(&psbt_input))`. Calling `bitcoin::consensus::serialize(&psbt::Input)` produces a different byte stream than what the coordinator's `bitcoin::psbt::Psbt::deserialize` reads — `psbt::Input` is a BIP-174 key-value substructure of a full PSBT, NOT a standalone consensus-serializable type. Even if `psbt::Input::serialize` (the BIP-174 key-value form) is used, it lacks the PSBT global header (magic bytes `0x70 0x73 0x62 0x74 0xff`, separator, etc.) that `Psbt::deserialize` requires.
**Why it happens:** The CONTEXT author conflated "encode a PSBT input" (the conceptual operation) with "use bare `psbt::Input` as the wire envelope" (an implementation detail). The coordinator-side reference at `coordinator/src/bitcoin/utxo.rs:212-225` is the source of truth and uses `bitcoin::psbt::Psbt::deserialize(&bytes)`, expecting a full PSBT.
**How to avoid:** Encoder MUST mirror `tests/integration/multi_script_validate.rs:56-74` (verbatim in Pattern 2 above): build a `bitcoin::psbt::Psbt::from_unsigned_tx(unsigned_tx)` with a placeholder tx (`version: 2`, `lock_time: ZERO`, `input: vec![TxIn { previous_output: OutPoint::null(), ... }]`, `output: vec![]`), populate `psbt.inputs[0].final_script_witness` (and `final_script_sig` for P2SH-P2WPKH), then `B64.encode(psbt.serialize())`. The helper return type should be `bitcoin::psbt::Psbt`, not `bitcoin::psbt::Input`.
**Warning signs:** The first integration test (D-78 `v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope`) will fail at the coordinator-side decode with `Bip322Error::DecodeError("psbt: ...")` if D-69's literal shape is implemented. Run the coordinator's decoder against the encoder's output in a sign-roundtrip unit test BEFORE writing integration tests.

### Pitfall 2: bdk_wallet's `Bip84`/`Bip86`/`Bip49` templates auto-select coin type per network
**What goes wrong:** Using `bdk_wallet::template::Bip84(xprv, External)` produces `wpkh(.../84'/1'/0'/0/*)` on testnet/signet (coin=1') and `wpkh(.../84'/0'/0'/0/*)` on mainnet (coin=0'). The v1.3 client at `client/src/wallet.rs:140-141` uses literal `wpkh({xprv}/84'/0'/0'/0/*)` regardless of network — coin=0' always. A v1.3 wallet generated on signet has a different first-external address than a Phase-17-via-Bip84-template wallet on signet. The `full_round.rs` cross-phase invariant test uses an existing WIF wallet so it would not surface this regression, BUT any user who funded a v1.3-generated signet wallet at the v1.3 address and upgrades to v1.4 would find their wallet's first address has SILENTLY CHANGED.
**Why it happens:** bdk_wallet templates implement BIP-44-strict coin-type indexing (BIP-44 §"Coin type" says coin=1' for testnet/signet). v1.3 deliberately chose BIP-44-non-strict (coin=0' always) per a project-level decision that's now load-bearing for byte-equivalence.
**How to avoid:** Use literal `format!()` descriptor strings per D-58 verbatim — do NOT switch to `bdk_wallet::template::Bip*` ergonomic helpers. The literal form makes the coin-type policy auditable in-source; the template form hides it.
**Warning signs:** A generated wallet's first external address differs from the v1.3 `client generate-wallet` output for the same seed. Add a sign-roundtrip test that asserts the generated address starts with `tb1q` / `tb1p` / `2N` (or whatever the network expects) AND matches the v1.3 hand-derived value when fed the same seed.
**Verified at:** `bdk_wallet-2.3.0/src/descriptor/template.rs:319,397,476` — each `impl DescriptorTemplate` calls `make_bipxx_private(BIP_NUMBER, key, keychain, network)`, where the helper at `segwit_v0::make_bipxx_private` derives `m/{bip}'/{coin}'/0'` with `coin = if network == Mainnet { 0 } else { 1 }`.

### Pitfall 3: CRIT-01 client-side discipline — `script_type` MUST come from the wallet, not the CLI flag
**What goes wrong:** Tempting shortcut: `OwnershipProof { script_type: Some(cfg.script_type), ... }` directly from the CLI flag at register_input call site. A user passing `--type p2tr` with a `wpkh(...)` descriptor would emit a v=2 envelope declaring `script_type: p2tr` over an on-chain P2WPKH SPK. The coordinator's CRIT-01 cross-check at `coordinator/src/bitcoin/utxo.rs:184` would reject with `ScriptTypeMismatch { declared: P2tr, derived: P2wpkh }` — but the user-facing error would be opaque, the wallet was misconfigured at construction time, and the wire shape doesn't reveal which side was wrong.
**Why it happens:** CLI flag is the most-recently-touched data; obvious source for the wire field. But the wallet's stored `script_type` field (D-62, set from descriptor at construction) is the CORRECT source — it has gone through the D-63 mismatch check.
**How to avoid:** ALWAYS write `OwnershipProof { script_type: Some(wallet.script_type()), ... }`. Add the inline `// CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo` comment above the assignment per D-80. The CI grep gate (`grep -c "CRIT-01" client/src/round/input.rs >= 1`) catches comment removal.
**Warning signs:** The `Bip322SignedProof` struct from `wallet.sign_bip322(...)` already carries `script_type` (D-64); the v2 envelope construction should mechanically use `signed.script_type` — not reach back to `cfg` or `wallet` for it. Pattern: `OwnershipProof { script_type: Some(signed.script_type), ... }`.

### Pitfall 4: Tor circuit ordering — main.rs has TWO branches and only ONE goes through tor::init_tor
**What goes wrong:** A reviewer or refactor might assume `tor::init_tor` always runs after `discover_coordinator`. In fact, `main.rs:67-75` shows `tor::init_tor` only runs when `cfg.use_tor` is set; otherwise the `CoordinatorClient::new(coordinator_url)` clearnet path is used. WALLET-03's fail-fast is "before any Tor circuit opens" — this is structurally satisfied by `discover_coordinator` running at line 58 (before the `if cfg.use_tor` branch at line 67), but the language of D-74 ("PKARR resolution already runs before Tor init at `main.rs:60`") under-specifies the ordering proof.
**Why it happens:** v1.3 client supports both clearnet and Tor modes; the Tor circuit is opened lazily in the `use_tor=true` branch only.
**How to avoid:** The proof is: `discover_coordinator` runs unconditionally at `main.rs:58` (line numbers may shift), and `tor::init_tor` is only reachable from the `if cfg.use_tor` branch at line 67. Both branches flow through `discover_coordinator` first. The fail-fast returns `Err(DiscoveryError::UnsupportedScriptType)` from `discover_coordinator` BEFORE the conditional Tor branch is even evaluated. Document this with an inline comment at `main.rs` above the discover call: `// WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural ordering, not a runtime hack.`
**Warning signs:** Any refactor that moves the discover call below the `if cfg.use_tor` branch breaks the structural ordering. Test in D-78 (`v13_pkarr_record_with_p2tr_wallet_rejects_before_tor`) should instrument with a test-only assertion that `tor::init_tor` was NOT called.

### Pitfall 5: Phase 16 B3 compact-name migration changed the PKARR field key from `version` to `v`
**What goes wrong:** CONTEXT D-73 shows `BlindjoinRecord { version: String, ... }` and `default_legacy_version() -> "0.1.0".into()`. But Phase 16-03 (commit `d1a1912`) compactified the PKARR field name from `version` to `v` for byte-budget reasons. A `#[derive(Deserialize)] struct BlindjoinRecord { version: String, ... }` against a Phase 16-produced record will MISS the field (no `version` key on the wire — only `v`), and the `#[serde(default)]` will produce `"0.1.0"` — making EVERY v1.4 coordinator look like a v1.3 legacy coordinator. The compat shim would fire on every connection.
**Why it happens:** CONTEXT D-73 was written before Phase 16-03 landed the B3 rename (or the rename was missed during context-gathering).
**How to avoid:** Use `#[serde(rename = "v", default = "default_legacy_version")] version: String` on the `BlindjoinRecord` struct. Verify by reading `coordinator/src/discovery/pkarr_pub.rs:98-108` — the production record emits `"v": "0.2.0"`, NOT `"version": "0.2.0"`. The same applies to OTHER compact-renamed fields if Phase 17 ever needs them (`ds`, `mp`, `st`, `n`); Phase 17 only needs `v`, `onion`, `sst`, `ost`, but the rename precedent is load-bearing.
**Warning signs:** The PKARR resolver test for "v0.2.0 record with `sst="p2tr,p2wpkh"`" produces `is_legacy = true` instead of `false`. The roundtrip would be: serialize the record via `coordinator::discovery::pkarr_pub::build_coordinator_packet`, decode via the new resolver, assert `is_legacy = false` AND `supported_script_types contains P2tr`. If the test fails, the rename is the most likely cause.

### Pitfall 6: WIF wallet path is the ONLY caller of `shared::bip322::sign_simple(P2wpkh, ...)` — descriptor wallets MUST NOT call it (it's `todo!()` for other types)
**What goes wrong:** A naïve dispatcher implementation might route ALL P2WPKH wallets (WIF + descriptor) through `shared::bip322::sign_simple(P2wpkh, ...)`. This works for P2WPKH descriptor wallets (the P2WPKH body in `shared::bip322::p2wpkh::sign` doesn't care whether the key came from a WIF or descriptor derivation). BUT the implementation would then fall over for P2TR descriptor wallets at `shared::bip322::sign_simple(P2tr, ...)` because `shared/src/bip322/mod.rs:269` routes to `p2tr::sign` which is `todo!()` in production per Phase 15 CD-6 (production `sign_simple` for P2TR/P2SH-P2WPKH is `todo!()`).
**Why it happens:** `sign_simple` has a uniform API across script types; the production-vs-test asymmetry is encoded in body bodies, not in the dispatcher.
**How to avoid:** Per D-65 and CD-24: WIF wallets (`from_wif` path) call `shared::bip322::sign_simple(P2wpkh, &spk, &secret_key, message)`. DESCRIPTOR wallets (`from_descriptor` + `generate` paths) — REGARDLESS of script type — call the bdk PSBT path. The dispatcher in `wallet.sign_bip322` should branch on `self.wif_key.is_some()` first (WIF vs descriptor), THEN on `self.script_type` only for descriptor wallets. WIF wallets are ALWAYS P2WPKH per D-61.
**Warning signs:** A descriptor wallet attempting to sign produces a `todo!()` panic. The `wallet_sign_roundtrip.rs` unit tests catch this immediately if they cover both wallet construction modes per script type.

### Pitfall 7: P2SH-P2WPKH `final_script_sig` extraction — bdk_wallet writes BOTH fields, the dispatcher must extract both
**What goes wrong:** A copy-paste of the P2TR extraction (which only checks `final_script_witness`) for P2SH-P2WPKH would emit a v=2 envelope with the P2WPKH witness present but `final_script_sig: None`. The coordinator's decoder at `coordinator/src/bitcoin/utxo.rs:212-225` only extracts `final_script_witness` — but the `shared::bip322::verify_simple(P2shP2wpkh, ...)` path (via the bip322 crate) needs the redeem script unwrap, which the BIP-322 verifier reconstructs from the witness + the on-chain SPK's HASH160 cross-check. So the COORDINATOR might still verify successfully without `final_script_sig` — making this a silent regression hazard rather than a hard failure.
**Why it happens:** bdk_wallet 2.3 finalises `sh(wpkh(...))` by populating BOTH `final_script_sig` (the `OP_PUSH <HASH160(redeem)>` push) AND `final_script_witness` (the P2WPKH stack). Skipping the `final_script_sig` extraction loses one piece of the spendability proof.
**How to avoid:** Per D-65 P2SH-P2WPKH dispatch body step 3: extract `psbt.inputs[0].final_script_sig.clone()` after `wallet.sign(...)` for P2SH-P2WPKH; assert `final_script_sig.is_some()` (fail loudly if bdk regresses). Pass both into the `Bip322SignedProof` struct; the v2 envelope encoder populates `psbt.inputs[0].final_script_sig = Some(...)` per Pattern 2 above.
**Warning signs:** The P2SH-P2WPKH `p2sh_p2wpkh_sign_roundtrip_verifies` test (D-78) should ALSO assert `signed.final_script_sig.is_some()` as a discipline gate, and the v=2 envelope b64 roundtrip test should assert the decoded PSBT input has BOTH fields populated.

### Pitfall 8: Sprint-0-B PoC file `client/examples/spike-p2tr.rs` is ONLY on `origin/spike/14-B-bdk-p2tr-poc`, NOT on main
**What goes wrong:** Phase 17 plans might say "executor: copy `client/examples/spike-p2tr.rs` verbatim." But this file does not exist on `main`. The spike branch was deliberately NOT merged per D-19 / D-21 (Phase 14 invariant: zero production-code commits in Phase 14). A plan that points executors at `client/examples/spike-p2tr.rs` will fail with "file not found".
**Why it happens:** Sprint-0-B was a throwaway PoC branch by design; the canonical record is the prose in `.planning/research/sprint-0-B.md`.
**How to avoid:** Plans should point executors at the verbatim code block in `sprint-0-B.md:22-285` as the canonical reference, OR copy the relevant 8-step sequence into the plan's `<actions>` section. The `cargo run -p client --example spike-p2tr` invocation is only reproducible via `git checkout spike/14-B-bdk-p2tr-poc`. Phase 17's analogous test path is `client/tests/wallet_sign_roundtrip.rs` (a new file in the `cargo test -p client` test target, NOT an example).
**Warning signs:** `ls client/examples/` returns empty (verified). Plans referencing `client/examples/spike-p2tr.rs` are pointing at a non-existent file.

## Code Examples

### Constructing per-type descriptors (D-58 verbatim — literal templates, NOT bdk templates)

```rust
// Source: D-58 verbatim; mirrors client/src/wallet.rs:140-141 v1.3 pattern
// CRITICAL: coin=0' across ALL networks (preserves v1.3 byte-equivalence per D-66)
fn build_descriptor_pair(script_type: ScriptType, xprv: &Xpriv) -> (String, String) {
    match script_type {
        ScriptType::P2wpkh => (
            format!("wpkh({}/84'/0'/0'/0/*)", xprv),
            format!("wpkh({}/84'/0'/0'/1/*)", xprv),
        ),
        ScriptType::P2tr => (
            format!("tr({}/86'/0'/0'/0/*)", xprv),
            format!("tr({}/86'/0'/0'/1/*)", xprv),
        ),
        ScriptType::P2shP2wpkh => (
            format!("sh(wpkh({}/49'/0'/0'/0/*))", xprv),
            format!("sh(wpkh({}/49'/0'/0'/1/*))", xprv),
        ),
    }
}
```

### CLI flag parser (D-57 + CD-17 lowercase kebab-case only)

```rust
// Source: D-57 verbatim
// Uses ScriptType's existing serde impl per Phase 15 D-Q3 RESOLVED
// (#[serde(rename_all = "snake_case")] + #[serde(rename = "p2sh-p2wpkh")])
fn parse_script_type(s: &str) -> Result<ScriptType, String> {
    // Wrap string in JSON quotes so serde_json::from_str fires the enum's serde impl
    let quoted = format!("\"{}\"", s);
    serde_json::from_str::<ScriptType>(&quoted)
        .map_err(|e| format!("invalid --type value '{s}': expected p2wpkh, p2tr, or p2sh-p2wpkh ({e})"))
}

// In ClientConfig:
#[arg(long = "type", env = "BLINDJOIN_SCRIPT_TYPE",
      default_value = "p2wpkh", value_parser = parse_script_type)]
pub script_type: ScriptType,
```

*Alternative: `clap::ValueEnum` derive on `ScriptType` would require modifying `shared::bip322::ScriptType` to add `#[derive(clap::ValueEnum)]`, which crosses the shared/client crate boundary. The `value_parser` approach above keeps clap-awareness in the client crate only.*

### Discovery layer extension (D-71/D-72/D-73)

```rust
// Source: D-71/D-72/D-73 + Phase 16 wire shape at
// coordinator/src/discovery/pkarr_pub.rs:98-108
use shared::bip322::ScriptType;

#[derive(Debug, Clone)]
pub struct CoordinatorInfo {
    pub coordinator_url: String,
    pub capabilities: CoordinatorCapabilities,
}

#[derive(Debug, Clone)]
pub struct CoordinatorCapabilities {
    pub record_version: String,
    pub is_legacy: bool,
    pub supported_script_types: Vec<ScriptType>,
    pub output_script_type: ScriptType,
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("invalid PKARR public key: {0}")]
    InvalidPubkey(String),
    #[error("coordinator not found in DHT for key '{pubkey}'")]
    NotFound { pubkey: String },
    #[error("no 'onion' field in PKARR record for key '{pubkey}'")]
    MissingOnion { pubkey: String },
    #[error("malformed PKARR record: {reason}")]
    MalformedRecord { reason: String },
    #[error("coordinator {pubkey} does not support {required:?} ownership proofs (supports: {supported:?})")]
    UnsupportedScriptType {
        pubkey: String,
        required: ScriptType,
        supported: Vec<ScriptType>,
    },
    #[error("coordinator {pubkey} produces {advertised:?} outputs but wallet expects {wanted:?}")]
    UnsupportedOutputScriptType {
        pubkey: String,
        advertised: ScriptType,
        wanted: ScriptType,
    },
}

pub async fn discover_coordinator(
    pkarr_pubkey: &str,
    required_input_script_type: ScriptType,
    required_output_script_type: ScriptType,
) -> Result<CoordinatorInfo, DiscoveryError> {
    // ... existing PKARR resolve_most_recent ...
    // ... parse BlindjoinRecord per Pattern 3 above ...
    // ... derive CoordinatorCapabilities from record ...
    // ... check capabilities and return typed error ...
}
```

### v1/v2 envelope branch (D-68 — corrected to use full PSBT per Pitfall 1)

```rust
// Source: D-68 + Pattern 2 wire-shape correction
let signed: Bip322SignedProof = wallet.sign_bip322(&bip322_message)?;

let proof = if coordinator_info.capabilities.is_legacy {
    // WALLET-04 compat shim — v1.3 coordinator path
    debug_assert_eq!(
        wallet.script_type(),
        ScriptType::P2wpkh,
        "unreachable: discovery layer rejected non-P2wpkh against legacy coordinator"
    );
    shared::protocol::OwnershipProof {
        version: 1,
        witness_stack: signed.witness_stack,
        psbt_input_b64: None,
        script_type: None,
    }
} else {
    // v1.4 coordinator path — always v2, regardless of wallet script type
    let psbt_input_b64 = build_v2_psbt_input_b64(
        &signed.witness,
        signed.final_script_sig.as_ref(),
    )?;
    shared::protocol::OwnershipProof {
        version: 2,
        witness_stack: signed.witness_stack,   // populated for symmetry (D-70)
        psbt_input_b64: Some(psbt_input_b64),
        // CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo
        script_type: Some(signed.script_type),
    }
};
let ownership_proof = proof.to_json_hex_str();
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Coordinator-local `verify_bip322_simple` hand-rolled P2WPKH-only | `shared::bip322::verify_simple` via dispatcher + bip322 crate | Phase 15-02 (2026-05-30) | All script types supported; Phase 17 reuses for sign-roundtrip tests |
| `OwnershipProof { script_pubkey, witness, message }` 3-field witness-only | `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` 4-field envelope | Phase 15-01 (2026-05-30) | Phase 17 client emits v=2 envelope by default; v=1 only for legacy compat shim |
| Manual P2TR sign via `secp256k1::sign_schnorr` (D-15, 80-LOC budget) | bdk_wallet 2.3 PSBT sign with `trust_witness_utxo: true` | Phase 14 ADR Decision #4 (Sprint-0-B PASS, 2026-05-29) | Phase 17 has zero manual crypto for P2TR/P2SH-P2WPKH |
| PKARR record `"version": "0.1.0"` with verbose field names | PKARR record `"v": "0.2.0"` with compact field names + `sst`/`ost` | Phase 16-03 (2026-05-30) | Phase 17 resolver uses compact-name keys; legacy decoder pattern preserved via `#[serde(default)]` |
| Coordinator-side `is_p2wpkh()` hard gate at validate_utxo | Allowlist + dispatcher (`bip_config.allows(derived)`) | Phase 16-02 (2026-05-30) | Client can register P2TR/P2SH-P2WPKH inputs once Phase 17 ships the sign path |
| Client `parse_onion_from_rr` extracting only `onion` field | Client `parse_blindjoin_record` extracting `v`, `onion`, `sst`, `ost` | Phase 17 (this phase) | WALLET-03 / WALLET-04 fail-fast at resolver |
| Client wallet implicit P2WPKH | Wallet stores `script_type: ScriptType` field | Phase 17 (this phase) | Single source of truth for CRIT-01 client-side discipline |

**Deprecated/outdated:**
- `client::round::input::generate_bip322_witness` (lines 115-149) — will be DELETED in 17-02 per CD-20 (replaced by `wallet.sign_bip322(...)` dispatch).
- `bdk_wallet::Wallet::sign` deprecation warning — still gated behind `#[allow(deprecated)]` per Phase 12 pattern at `client/src/wallet.rs:268`; PSBT signing migration to `bitcoin::psbt` is a v1.5+ candidate (Sprint-0-B notes this is not wired in bdk 2.3 yet).

## Project Constraints (from CLAUDE.md)

- **No custom crypto:** Phase 17 uses blind-rsa-signatures, rust-bitcoin, bdk_wallet, secp256k1 only. The bdk_wallet PSBT sign path satisfies this; calls `secp256k1` via bdk's internal signer. NO new direct `secp256k1::sign_schnorr` or `Secp256k1::sign_ecdsa` calls (the existing one at `client/src/round/input.rs:140` will be DELETED with `generate_bip322_witness` per CD-20).
- **Tor-native in production; dev/test may use clearnet TCP:** Phase 17 WALLET-03 fail-fast happens BEFORE any Tor circuit opens — pure resolver-layer logic. The clearnet vs Tor branch at `main.rs:67` is unchanged.
- **Signet-first; mainnet is a config flag:** Phase 17 honors this — `wallet.network` flows through the bdk path identically across networks.
- **No PII logging; round state zeroed after broadcast:** Phase 17 `DiscoveryError` messages name the coordinator pubkey (z32, public) + script type (enum value) ONLY. No IP, no wallet identifier, no UTXO outpoint. The CD-21 WARN log (legacy coordinator detection) also carries only `coordinator_pubkey` + `record_version` — public DHT data.
- **MIT licensed; public good, not a business:** N/A for Phase 17 implementation; relevant to README/docs unchanged.
- **GSD workflow enforcement (`./CLAUDE.md`):** Phase 17 plans must use GSD-managed edits via `/gsd-execute-phase`. The `/browse` skill is the canonical web-browsing route if executors need to look up bdk_wallet docs.
- **Skill routing rules:** N/A for this research; relevant only when user requests skill-managed work.

## Assumptions Log

> No `[ASSUMED]` claims in this research — every load-bearing fact was verified against in-tree source (`client/src/*.rs`, `coordinator/src/*.rs`, `shared/src/*.rs`, `tests/integration/*.rs`), the vendored bdk_wallet 2.3.0 source at `~/.cargo/registry/src/...`, or the canonical research artifact at `.planning/research/sprint-0-B.md`. The only "assumed" element is the planner's choice between CD-17/CD-18/CD-19/CD-22 alternatives within their bounded discretion — those are NOT claims, they are policies.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| (empty) | — | — | — |

## Open Questions

1. **CONTEXT D-73's `BlindjoinRecord.version` field key — `v` or `version`?**
   - What we know: Phase 16-03 (commit `d1a1912`) compactified the PKARR field name from `version` to `v` for byte-budget reasons (`coordinator/src/discovery/pkarr_pub.rs:98-108` emits `"v": "0.2.0"`).
   - What's unclear: D-73 literally says `version: String`. If executor copies the D-73 struct verbatim, the decoder misses every Phase 16 record and triggers the legacy shim on every connection.
   - Recommendation: Plan-phase MUST specify `#[serde(rename = "v", default = "default_legacy_version")] version: String` on the `BlindjoinRecord` struct. Update D-73 inline in the 17-03 PLAN as a CONTEXT correction.

2. **D-69's `bitcoin::psbt::Input` shape — bare Input or full PSBT?**
   - What we know: Coordinator decoder at `coordinator/src/bitcoin/utxo.rs:212-225` calls `bitcoin::psbt::Psbt::deserialize`, expecting a FULL PSBT. Existing helper at `tests/integration/multi_script_validate.rs:56-74` builds a full PSBT and roundtrips successfully.
   - What's unclear: D-69 literally says `build_v2_psbt_input(signed) -> Result<bitcoin::psbt::Input>` with `B64.encode(bitcoin::consensus::serialize(&psbt_input))`. This is wrong by one envelope layer.
   - Recommendation: Plan-phase MUST replace D-69's body with the Pattern 2 helper signature `fn build_v2_psbt_input_b64(witness: &Witness, final_script_sig: Option<&ScriptBuf>) -> Result<String>` that returns the base64-encoded full PSBT, per the test helper. Update D-69 inline in the 17-02 PLAN as a CONTEXT correction.

3. **`required_output_script_type` parameter — does the resolver need it AT discover-time, or after?**
   - What we know: D-76 says output script type mismatch ALSO fails at discovery. CD-23 splits this into a separate `UnsupportedOutputScriptType` variant.
   - What's unclear: The current `discover_coordinator(pubkey, required_input_script_type)` signature in D-72 is single-arg. Adding a second `required_output_script_type` arg is the simplest implementation, but the wallet's output script type is derived from its descriptor (same as input). Should the resolver call site at `main.rs:60` pass `(wallet.script_type(), wallet.script_type())` (both args identical because v1.4 has no mixed-output rounds per D-07), or should the signature be `discover_coordinator(pubkey, required: ScriptType)` (single arg, used for both checks)?
   - Recommendation: Single-arg signature `discover_coordinator(pubkey, wallet_script_type)` — checks both `wallet_script_type ∈ supported_script_types` AND `wallet_script_type == output_script_type`. Simpler API, matches the fact that a v1.4 client's input and output script types are the same (both derived from the wallet descriptor). Plan-phase decides.

4. **WIF wallet WIF-path P2WPKH — also through bdk PSBT path or stay on `shared::bip322::sign_simple`?**
   - What we know: CD-24 says "descriptor wallets ALL go through bdk_wallet's PSBT signer ... The WIF wallet stays on the secret_key_for_signing + shared::bip322::sign_simple path (legacy)." D-65 confirms.
   - What's unclear: A unified path (everything through bdk) would be cleaner. But the WIF wallet uses `Wallet::create_single` (no keychain; bdk-internal signer has only the single key), and the BIP-322 PSBT shape might not match what bdk's single-key signer expects.
   - Recommendation: Keep CD-24's split — WIF on shared::bip322::sign_simple (battle-tested for P2WPKH; bit-exact with v1.3 inline `generate_bip322_witness`); descriptor on bdk. The unified-path refactor is a v1.5+ cleanup, not Phase 17 scope.

5. **`bitcoin::psbt::Input::default()` field defaults — does an "empty Input with only `final_script_witness` and `final_script_sig` populated" roundtrip cleanly?**
   - What we know: `psbt::Input::default()` produces an empty struct with all `Option` fields = `None` and all `Vec`/`BTreeMap` fields empty. The test helper at `tests/integration/multi_script_validate.rs:56-74` uses this exact pattern (default tx + populate `final_script_witness`); the coordinator's `Psbt::deserialize` + `psbt.inputs[0].final_script_witness.clone()` successfully extracts the witness.
   - What's unclear: ADR Decision #3 Consequences predicted ~+100 bytes per proof for the v2 envelope. With only `final_script_witness` populated, the actual overhead may be lower (~60-80 bytes); if `final_script_sig` is also populated for P2SH-P2WPKH, the overhead grows slightly.
   - Recommendation: Not a correctness question — the wire shape works. Plan-phase may add a unit test `assert!(psbt_input_b64.len() < 300)` as a regression gate on PSBT envelope size growth, but not load-bearing.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` | All workspace builds | ✓ | 1.95.0 per sprint-0-B reproducibility line | — |
| `rustc` | All workspace builds | ✓ | 1.95.0 per sprint-0-B reproducibility line | — |
| `bitcoind` regtest | `tests/integration/multi_script_client.rs` per D-78 | ✓ (per Phase 16-02 successful run on this machine; `BLINDJOIN_REQUIRE_BITCOIND=1` for CI mode) | v30.2 pinned via `corepc-node` 0.12 feature flag `30_2` | Graceful skip via `require_bitcoind!()` macro — Phase 17 tests skip cleanly on dev machines without bitcoind |
| Bitcoin testnet/signet node | Manual operator testing (not in Phase 17 scope) | N/A | — | Not needed for Phase 17 acceptance |
| Tor / arti-client | Phase 17 fail-fast happens BEFORE Tor — Tor only needed for `--use-tor` runtime, NOT for any Phase 17 test | ✓ (already in tree, unchanged by Phase 17) | per workspace pin | N/A |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:** `bitcoind` is graceful-skip on dev machines via `require_bitcoind!()` — Phase 17 unit tests in `client/tests/wallet_sign_roundtrip.rs` are designed to need NO bitcoind (sign↔verify roundtrips against `shared::bip322::verify_simple`).

## Security Domain

> `security_enforcement` is not explicitly set in `.planning/config.json`, so treated as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase 17 uses BIP-322 signature-based authentication of UTXO ownership; that's a domain-specific protocol, not a session-auth flow. The session_token from `/round/input` is established AFTER ownership proof, by the existing v1.3 path. |
| V3 Session Management | no | Session tokens are existing v1.3 HMAC tokens; Phase 17 does not touch session management. |
| V4 Access Control | yes | The coordinator's allowlist (`BipConfig.allows`) is the access control surface for input script types; Phase 17 client respects this via WALLET-03 fail-fast (does not attempt to register an input the coordinator does not accept). |
| V5 Input Validation | yes | PKARR record decoding (`BlindjoinRecord` struct) MUST handle malformed JSON, missing fields, unknown script types gracefully (`DiscoveryError::MalformedRecord`); CLI `--type` flag validation rejects invalid script-type strings at parse time. CRIT-01 client-side discipline (D-80) prevents the client from emitting a self-inconsistent OwnershipProof envelope. |
| V6 Cryptography | yes | Phase 17 uses LIBRARY-only crypto — `bdk_wallet::Wallet::sign` for descriptor wallets, `shared::bip322::sign_simple` (which wraps `secp256k1::sign_ecdsa`) for WIF wallets. NO new direct calls to `secp256k1::sign_schnorr` or hand-rolled sighash construction (existing one at `client/src/round/input.rs:140` is DELETED per CD-20). |

### Known Threat Patterns for the v1.4 client stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Client-declared script type mismatches on-chain UTXO (CRIT-01) | Spoofing / Tampering | Wallet's stored `script_type` is the wire source (D-62, D-80); descriptor mismatch caught at `from_descriptor` construction (D-63); coordinator's CRIT-01 cross-check at `coordinator/src/bitcoin/utxo.rs:184` is the load-bearing rejection |
| Malicious PKARR record advertises capabilities it doesn't have | Spoofing | Phase 17 client trusts the PKARR `sst`/`ost` claim AT discovery time (fail-fast); coordinator-side enforcement is the actual gate. A malicious coordinator that claims `sst=p2tr` but rejects P2TR at runtime fails at the round, not at discovery. v1.5+ candidate: cross-check `/round/info.supported_script_types` against PKARR `sst` post-handshake — but per D-75, NOT in Phase 17 scope. |
| Coordinator advertises a script type unique to one operator (de-anonymization) | Information Disclosure | OUT OF SCOPE per REQUIREMENTS — coordinator advertises SUPPORTED SET, not per-round per-script-type registration counts (D-09); known limitation documented in privacy considerations |
| Tor circuit opened against a coordinator that rejects the client's script type (Tor metadata leak via mid-round abort) | Information Disclosure | WALLET-03 fail-fast at the PKARR resolver layer ensures NO Tor circuit opens for a rejected coordinator. Verified structurally at `client/src/main.rs:57-69` (resolver returns Err before the Tor branch). |
| BIP-322 sighash regression silently produces invalid signatures | Tampering | Phase 15 `[Rule 1 — Bug]` fixes (Version(0) + bare OP_RETURN) at `shared/src/bip322/mod.rs:105-138` are LOAD-BEARING; Phase 17 must NOT bypass these (use the `build_bip322_to_spend`/`build_bip322_to_sign` helpers, never reconstruct manually). |
| WIF private key disclosure via error logs | Information Disclosure | `Bip322SignedProof` and `DiscoveryError` Display impls NEVER interpolate key material; Phase 15 PII-leak grep test (`shared/src/bip322/mod.rs:511-565`) is the existing discipline; Phase 17 inherits. |
| Descriptors.txt file world-readable | Information Disclosure | Existing `0600` permission set at `client/src/wallet.rs:200`; Phase 17 generate path inherits unchanged. |

## Sources

### Primary (HIGH confidence — in-tree source verified)
- `client/src/wallet.rs` (whole file) — current BdkClientWallet shape, BIP-84 descriptor literal at lines 140-141, sign_psbt_input pattern at lines 248-288, witness extraction fallback at lines 277-285
- `client/src/round/input.rs` (whole file) — current register_input flow, generate_bip322_witness at lines 115-149 (to be DELETED per CD-20), v2 envelope construction site (currently emits v=1 hardcoded at lines 69-75)
- `client/src/discover.rs` (whole file) — current parse_onion_from_rr at lines 67-81, CoordinatorInfo shape at lines 5-8
- `client/src/config.rs` (whole file) — existing `#[arg]` patterns Phase 17 extends
- `client/src/main.rs` (whole file) — ordering proof: discover at line 58 BEFORE tor::init_tor at line 68
- `client/src/round/sign.rs` lines 1-60 — confirms `Psbt::deserialize(&b64.decode(...))` pattern is what client uses elsewhere
- `coordinator/src/bitcoin/utxo.rs` (whole file) — dispatcher at lines 152-195, decode_psbt_input_witness at lines 212-225 (LOAD-BEARING: uses `Psbt::deserialize`, NOT bare `psbt::Input`)
- `coordinator/src/discovery/pkarr_pub.rs` (whole file) — PKARR wire shape at lines 89-132: `v`/`onion`/`n`/`ds`/`mp`/`st`/`sst`/`ost`/`type` compact-renamed fields
- `coordinator/src/config.rs` lines 100-300 — BipConfig shape, validate(), supported() canonical alphabetical order
- `shared/src/bip322/mod.rs` (whole file) — locked API: ScriptType, Bip322Error, detect_script_type, verify_simple, sign_simple (P2TR/P2SH-P2WPKH bodies `todo!()` in production per CD-6); sign_simple_test_only at lines 302-314
- `shared/src/protocol.rs` (whole file) — OwnershipProof flat struct at lines 175-245, CD-7 byte-identity branch at line 239
- `tests/integration/multi_script_validate.rs` lines 1-200 — CANONICAL v=2 PSBT envelope encoder at lines 56-74 (proves Pattern 2 above)
- `tests/integration/mod.rs` lines 560-822 — fund_regtest_typed, TypedUtxoHandle, FundedTypedSetup helpers (Phase 16-02 LANDED; Phase 17 consumes unchanged)
- `.planning/research/sprint-0-B.md` — full 8-step P2TR sign PoC; verdict PASS at line 315; extraction-priority finding at lines 317-319
- `.planning/decisions/v1.4-adr.md` §`#decision-3` (lines 88-141) and §`#decision-4` (lines 143-186) — wire envelope + bdk path locked
- `.planning/phases/17-client-multi-script-wallet-discovery/17-CONTEXT.md` (whole file) — D-57..D-80 + CD-17..CD-24 + canonical refs
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` lines 1-100 — Phase 16 wire shape (cross-referenced for D-73 correction)
- `.planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` (via 17-CONTEXT inheritance) — D-22..D-32 wire envelope; CD-6 `sign_simple` todo!() in production
- `/Users/john/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bdk_wallet-2.3.0/src/descriptor/template.rs` lines 316-518 — Bip49/Bip84/Bip86 template internals; coin-type auto-selection at `segwit_v0::make_bipxx_private(BIP, key, keychain, network)` call sites at lines 320, 399, 478
- `.planning/REQUIREMENTS.md` — WALLET-01..04 definitions; Out-of-Scope matrix; Future Requirements
- `.planning/ROADMAP.md` Phase 17 §Success Criteria — 5 criteria binding cross-phase invariant

### Secondary (MEDIUM confidence — derived from primary)
- BIP-84 / BIP-86 / BIP-49 wire-form descriptors (project README + bdk_wallet doc examples cross-confirm)
- BIP-322 to_spend / to_sign canonical shape (Phase 15-03 [Rule 1] fixes + bip322 crate verify-side cross-check)
- v1.3 client wallet byte-equivalence requirement (D-66 + v1.3 wallet.rs:140-141 literal coin=0' verification)

### Tertiary (none — all claims verified from primary sources)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every dep already in tree; versions exact-pinned; no new package surface
- Architecture: HIGH — every code anchor verified; ordering proof structural; capability-flag flow mechanically derivable
- Pitfalls: HIGH — Pitfall 1 (D-69 wire shape) and Pitfall 2 (bdk template coin type) both verified against in-tree source + vendored bdk_wallet 2.3.0; Pitfall 5 (D-73 field rename) verified against `coordinator/src/discovery/pkarr_pub.rs:98-108`

**Research date:** 2026-05-30
**Valid until:** 2026-06-29 (30 days — Phase 17 plans should consume this RESEARCH before bdk_wallet 2.4 lands or before any phase that bumps dependency versions)
