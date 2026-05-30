# Phase 15: Shared Crate Multi-Script Contract - Research

**Researched:** 2026-05-29
**Domain:** Rust crate API design — `shared/` becomes the BIP-322 multi-script verification + v1.4 wire-type contract for coordinator and client
**Confidence:** HIGH

## Summary

Phase 15 is a **surgical refactor of the `shared/` crate** into the single source of truth for BIP-322 multi-script verification. The 133-LOC flat `shared/src/bip322.rs` becomes a four-file module (`mod.rs / p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`). All three load-bearing decisions are LOCKED upstream by `.planning/decisions/v1.4-adr.md` and CONTEXT D-22..D-34: ADOPT `bip322 = "=0.0.10"` as a private crate-backed verifier (26-LOC adapter from Sprint-0-A:145-175), evolve `OwnershipProof` to a flat serde struct with `version: u8` envelope, and ship the wire-format roundtrip test FIRST in its own commit (REPAIR-01 lesson #1). The phase has zero coordinator/ or client/ code touches beyond deleting a single 14-LOC local error enum (`coordinator/src/bitcoin/utxo.rs:87-101`) and rewriting its import path. `verify_simple` and `is_p2wpkh()` gate **stay in place** — Phase 16 removes them.

Verification of the bip322 crate's actual source confirmed at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bip322-0.0.10/src/`: `verify_simple(&Address, message: impl AsRef<[u8]>, signature: Witness) -> Result<()>` matches Sprint-0-A's sketch byte-for-byte. The crate's `Error` enum is snafu-derived with 19 variants — wrapping via `#[source]` (the Sprint-0-A pattern) is the correct integration shape; the planner does NOT need to enumerate the upstream variants. Critical upstream finding: the official `basic-test-vectors.json` at `github.com/bitcoin/bips/bip-0322/` contains **no P2SH-P2WPKH vectors** — only P2WPKH, P2TR, and a P2WSH-multisig case (out of v1.4 scope). The bip322 crate self-tests P2SH-P2WPKH against hardcoded constants in `lib.rs:46-48` (mainnet `3HSV...` address). Phase 15 must either supplement the upstream vectors with crate-internal vectors OR self-generate P2SH-P2WPKH fixtures via the test-only signer.

**Primary recommendation:** Lift the Sprint-0-A 26-LOC adapter verbatim into `shared/src/bip322/mod.rs`, expand `Bip322Error` to the 10 variants enumerated in CONTEXT D-31, vendor `basic-test-vectors.json` from `bitcoin/bips@<commit-sha>` plus a supplementary P2SH-P2WPKH vector lifted from the bip322 crate's `lib.rs:46-48`, and structure plans exactly as CD-10 suggests: `15-01` wire-format roundtrip → `15-02` module split + dispatcher → `15-03` per-script tests + 9-rejection matrix + `sign_simple`. The CI grep gate at `.github/workflows/ci.yml:183-213` is an existing template — extend it for `bip322 = "=0.0.10"` and `thiserror = "1"` (workspace already pins these correctly).

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Carried forward from Phase 14 ADR (NOT re-asked):**

- **ADR #1 (#decision-1):** ADOPT `bip322 = "=0.0.10"`. 26-LOC adapter from `.planning/research/sprint-0-A.md:145-175` is the implementation template. Three new transitive crates accepted: `bip322 v0.0.10`, `snafu v0.8.9`, `snafu-derive v0.8.9`. Script-type-NEUTRAL primitives stay in shared/ as V1.4-MOD-07 single source of truth.
- **ADR #3 (#decision-3):** B2 PSBT-input wire shape; explicit `version: u8` envelope; `version = 1` = v1.3 witness-only path, `version = 2` = v1.4 PSBT path. 5 D-13 roundtrip test cases ship FIRST per REPAIR-01 lesson #1.
- **ADR #4 (#decision-4):** bdk path for P2TR sign — implementation in Phase 17 WALLET-02; Phase 15 only ships the `sign_simple` API surface. D-15 manual `secp256k1::sign_schnorr` fallback retired for v1.4.
- **REQUIREMENTS BIP322-02:** P2TR accepts BOTH SIGHASH_DEFAULT 64-byte AND SIGHASH_ALL 65-byte sig forms; P2SH-P2WPKH dispatch performs BIP-143 sighash over the unwrapped P2WPKH redeem script WITH a `HASH160(redeemScript) == script_pubkey.p2sh_hash` cross-check.
- **Phase 14 D-04:** Module split = `mod.rs / p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`.
- **Phase 14 carry-forward constraint #3:** Exact-pin every new dependency. `bip322 = "=0.0.10"` and `thiserror` (any v1.x); both enforced via CI grep gate alongside the existing `bdk_wallet = "=2.3.x"` and `corepc-node` feature pins.

**OwnershipProof v2 wire shape:**

- **D-22 (v1↔v2 coexistence):** Single flat `OwnershipProof` struct with `#[serde(default)]` on `version` (default = 1), `witness_stack` (default = empty), `psbt_input_b64` (Option), `script_type` (Option). Coordinator branches with `match proof.version`. NO tagged serde enum. NO two-struct dispatcher.
- **D-23 (envelope transport):** `InputRegRequest.ownership_proof` stays `String` containing JSON-serialized `OwnershipProof`. The existing v1.3 `from_json_hex_str` / `to_json_hex_str` helpers become thin convenience wrappers around `serde_json`.
- **D-24 (script_type placement):** `script_type: Option<ScriptType>` is a sibling envelope field, NOT inferred from PSBT contents.
- **D-25 (version default):** `#[serde(default = "default_proof_version")]` returning `1`.

**Module layout + dispatcher style:**

- **D-26 (adapter location):** The 26-LOC `bip322 = "=0.0.10"` crate adapter lives as a **crate-private fn in `shared/src/bip322/mod.rs`** alongside the `verify_simple` dispatcher.
- **D-27 (public API surface):** Public from `shared::bip322` is dispatcher-only:
  - `pub enum ScriptType`
  - `pub enum Bip322Error`
  - `pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error>`
  - `pub fn verify_simple(script_type: ScriptType, spk: &Script, witness: &Witness, message: &[u8], network: Network) -> Result<(), Bip322Error>`
  - `pub fn sign_simple(script_type: ScriptType, spk: &Script, key: &SecretKey, message: &[u8]) -> Result<Witness, Bip322Error>` (Phase 17 fills in the bdk-backed body)
  - Script-NEUTRAL helpers (`bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`) — re-exported from the module root.
  - Per-script files are `pub(crate)` inner mechanics only.
- **D-28 (OwnershipProof home):** `OwnershipProof` stays in `shared/src/protocol.rs`. `shared::bip322` owns verifiers + `ScriptType` only. `ScriptType` is `pub use shared::bip322::ScriptType` re-exported.

**Error taxonomy + lib choice:**

- **D-29 (Bip322Error home):** Single unified `Bip322Error` enum in `shared/src/bip322/mod.rs`, exported as `pub`. The existing coordinator-local `Bip322Error` at `coordinator/src/bitcoin/utxo.rs:87-101` is **deleted in Phase 15**.
- **D-30 (error lib):** `thiserror` (any v1.x). snafu enters transitively via the bip322 crate but is NOT used directly.
- **D-31 (variant taxonomy, ~10 variants):** Verbatim from CONTEXT D-31 — `UnsupportedProofVersion(u8)`, `WireFormatMismatch(String)`, `DecodeError(String)`, `UnrecognisedScriptPubkey { #[source] }`, `UnsupportedScriptType`, `ScriptTypeMismatch { declared, derived }`, `InvalidWitnessLength { expected, got }`, `CrateVerifyFailed { #[source] bip322::error::Error }`, `NetworkMismatch { decoded, configured }`, `ScriptMismatch`.
- **D-32 (wire mapping):** ALL `Bip322Error` variants map to `ApiError { code: ErrorCode::InvalidOwnershipProof, ... }` at the coordinator handler layer. NO new `ErrorCode` variants.

**Spec-vector fixture + rejection harness:**

- **D-33 (fixture pinning):** Vendored snapshot at `shared/tests/fixtures/bip322/basic-test-vectors.json` with a header recording `# source: bitcoin/bips@<commit-sha>; captured 2026-05-XX`. `include_str!` at compile time. Zero network in CI.
- **D-34 (cross-shape rejection harness):** 9 enumerated `#[test]` functions per off-diagonal (spk × witness) combination — see CONTEXT D-34 for the list. Diagonal entries (p2wpkh × p2wpkh, etc.) are positive sign↔verify property tests against `basic-test-vectors.json`, NOT in this matrix.

### Claude's Discretion

- **CD-6:** `sign_simple` shape — Default: **`todo!()` marker for P2TR/P2SH-P2WPKH with a `#[cfg(test)]`-only sign for the property-test path**. P2WPKH body fully implemented in Phase 15 (already known good via the existing `make_bip322_witness` test helper).
- **CD-7:** `version = 1` legacy decoder accepts BOTH v1.3 array-of-hex JSON AND new flat-struct JSON. Default: **both** — `OwnershipProof::from_json_hex_str` tries `serde_json::from_str::<Vec<String>>` first, falls back to `serde_json::from_str::<OwnershipProof>`.
- **CD-8:** `Network` parameter on `verify_simple` is `bitcoin::Network` enum directly (Sprint-0-A's adapter shape).
- **CD-9:** `shared/Cargo.toml` dep declaration uses **default features** for `bip322 = "=0.0.10"`. **VERIFIED via the crate's `Cargo.toml`:** `default = []` (no default features exist). No `--no-default-features` flag needed; the `cargo tree` shows `bip322 v0.0.10 default` because the empty default IS the default. Confirmed.
- **CD-10:** Wire-format roundtrip test ships as its own dedicated plan (15-01-PLAN.md = wire-format tests FIRST; 15-02-PLAN.md = bip322 module split + dispatcher; 15-03-PLAN.md = per-script tests + 9-rejection matrix).

### Deferred Ideas (OUT OF SCOPE)

- Removal of `coordinator/src/bitcoin/utxo.rs::verify_bip322_simple` + the `is_p2wpkh()` gate at line 119 — Phase 16 (ADVERT-03).
- Wire ErrorCode expansion (per-script-type rejection codes) — Anti-feature per REQUIREMENTS.md Out-of-Scope; D-32 locked.
- TEST-EXT-01/02/03 (cross-impl differential fixtures, on-chain anchor test, automated compat matrix) — v1.5 candidates.
- `sign_simple` production body for P2TR/P2SH-P2WPKH inside `shared/` — Phase 17 implements in `client/` via bdk per ADR Decision #4.
- `bip322 = "=0.0.10"` → 1.0 reconsider trigger — v1.5+ if the crate ships 1.0.
- `#[non_exhaustive]` on `Bip322Error` — Default: NOT non-exhaustive in v1.4.
- DECISIONS-INDEX.md rolling summary — v1.5 candidate.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BIP322-01 | `ScriptType` enum + `detect_script_type` with no fallthrough default arm | `shared::bip322::ScriptType` + `detect_script_type` per D-27; backed by `bitcoin 0.32.x` `Script::is_p2wpkh / is_p2tr / is_p2sh` (already in dep graph). Sprint-0-A:145-175 covers the dispatcher shape. |
| BIP322-02 | `verify_simple` dispatches to P2WPKH (BIP-143), P2TR (BIP-341, accept 64-byte + 65-byte), P2SH-P2WPKH (BIP-143 over unwrapped redeem + HASH160 cross-check) | `bip322 = "=0.0.10"` crate's `verify_simple` already routes to `verify_full_p2wpkh(is_p2sh: bool)` and `verify_full_p2tr` per `verify.rs:62-99`. P2TR sighash length branching is at `verify.rs:214-231` (handles 64 + 65 bytes). P2SH path passes `is_p2sh: true` and reconstructs the unwrapped P2WPKH SPK at `verify.rs:167-169`. All BIP322-02 semantics are crate-provided; our adapter just wraps. |
| BIP322-03 | `sign_simple` symmetric to `verify_simple` — produces witness stack per script type | Phase 15 ships the signature surface; CONTEXT CD-6 defaults the production body to `todo!()` for P2TR/P2SH-P2WPKH with `#[cfg(test)]` test signers. P2WPKH body uses existing `make_bip322_witness` helper. Phase 17 WALLET-02 fills bdk-backed bodies per ADR #4. |
| BIP322-04 | Per-script property tests against official BIP-322 `basic-test-vectors.json` + 9 (spk × witness-shape) cross-shape rejections | Vendored fixture at `shared/tests/fixtures/bip322/basic-test-vectors.json` per D-33. **Critical: upstream has no P2SH-P2WPKH vectors** — supplement with the bip322 crate's `lib.rs:46-48` test constants for P2SH-P2WPKH coverage. 9-rejection matrix per D-34. |
| ADVERT-04 | `OwnershipProof` wire format extended for P2SH-P2WPKH `final_script_sig`; roundtrip test ships BEFORE coordinator/client uses new shape | D-22..D-25 + CD-7 fully specify the flat-struct envelope. Roundtrip test ships FIRST as `15-01-PLAN.md` (CD-10). PSBT-input shape (`bitcoin::psbt::Input` base64) natively carries `final_script_sig` per ADR Decision #3. |

## Project Constraints (from CLAUDE.md)

CLAUDE.md sets these project-level guardrails Phase 15 MUST honour:

- **No custom crypto** — wrap `bip322 = "=0.0.10"` for verify, reuse `bitcoin 0.32.x` primitives (`SighashCache`, `secp256k1`, `XOnlyPublicKey`) for sign. Do NOT introduce `ring`, `openssl`, or a third-party Schnorr-sign helper.
- **MIT licence** — `bip322 v0.0.10` is licensed CC0-1.0 (per its `Cargo.toml`); `thiserror` is MIT/Apache-2.0; `snafu` is MIT/Apache-2.0. All compatible. No licence-cleanliness gate triggered.
- **Tor-native in production; clearnet OK in dev/test** — Phase 15 is a pure-crate phase; no network code touched.
- **Signet-first** — Phase 15's `Network` parameter (CD-8) accepts the full `bitcoin::Network` enum; test code uses `Network::Regtest`. No special signet-versus-mainnet branching needed in `shared/`.
- **No PII logging** — Phase 15 does not emit log statements; `Bip322Error`'s `Display` impls reveal only script-type metadata + spec error codes, not user-identifying fields. Confirmed safe.
- **GSD workflow enforcement** — Phase 15 is invoked via `/gsd-execute-phase` per CLAUDE.md routing.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| BIP-322 sighash computation (sign + verify) | `shared/` crate | `bip322 = "=0.0.10"` (private dep) | Both coordinator and client compile against `shared::bip322` — the "shared crate is the contract" invariant from v1.0. The bip322 crate handles inner math; we wrap. |
| `OwnershipProof` wire envelope | `shared/src/protocol.rs` | `shared/src/bip322/mod.rs` (re-exports `ScriptType` only) | `protocol.rs` owns wire types per existing v1.0 pattern. `bip322/mod.rs` imports `ScriptType` from `shared::bip322` via `pub use` re-export to break the cycle. |
| Script-type detection (`detect_script_type`) | `shared/src/bip322/mod.rs` | `bitcoin::Script::is_p2wpkh/is_p2tr/is_p2sh` (transitive) | Coordinator (Phase 16) will call this from `coordinator/src/bitcoin/utxo.rs` at validate-utxo time per CRIT-01. Logic lives in `shared/` so client + coordinator agree on classification. |
| Per-script-type verifier mechanics | `shared/src/bip322/{p2wpkh,p2tr,p2sh_p2wpkh}.rs` (`pub(crate)`) | `bip322 = "=0.0.10"` | One file per script type per D-04 / V1.4-CRIT-02 — failure localises to one file. No public per-script entry points (D-27); only the dispatcher is `pub`. |
| Error mapping to wire `ErrorCode::InvalidOwnershipProof` | `coordinator/src/api/handlers.rs` (Phase 16 work) | `shared::bip322::Bip322Error` (Phase 15 surfaces the typed enum) | Phase 15 ships the typed enum; handler-layer wire mapping is Phase 16. D-32 locks the single-bucket mapping. |
| Production `sign_simple` for P2TR/P2SH-P2WPKH | `client/src/wallet.rs` (Phase 17 via `bdk_wallet`) | `shared::bip322::sign_simple` signature surface (Phase 15) | ADR Decision #4: bdk path lives in `client/`; `shared/` exposes the contract. Phase 15 ships `todo!()` for P2TR/P2SH-P2WPKH with `#[cfg(test)]` signers for round-trip tests. |
| Test fixtures (vendored BIP-322 vectors + crate's internal P2SH-P2WPKH constants) | `shared/tests/fixtures/bip322/` | `include_str!` at compile time | Zero network in CI per D-33. The vendored snapshot is the supply-chain hardening pattern carried forward from v1.3 REPAIR-02. |

## Standard Stack

### Core (already in workspace — Phase 15 does NOT add at workspace level)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| bitcoin | 0.32.x (workspace `^2.3.0` → resolves 0.32.8 per Sprint-0-A) | Primitives: `Script`, `Witness`, `Address`, `Network`, `SighashCache`, `secp256k1` re-exports | The canonical rust-bitcoin crate. The `bip322` crate depends on `bitcoin 0.32.5`, our pin resolves to `0.32.8` — single version in the tree per Sprint-0-A gate 1. [VERIFIED: `cargo tree` in sprint-0-A.md] |
| serde + serde_json | 1.x (workspace) | `OwnershipProof` flat-struct derive | Universal. `#[serde(default)]` + `#[serde(skip_serializing_if = "Option::is_none")]` patterns already in `shared/src/protocol.rs:70`. [VERIFIED: existing code at `shared/src/protocol.rs:1-3`] |
| hex | 0.4 (already in `shared/Cargo.toml`) | Legacy v1.3 array-of-hex compat decoder | Existing dep; CD-7's two-phase try-parse uses `hex::decode` for the v1 path. [VERIFIED: `shared/Cargo.toml:12`] |
| uuid | workspace | (Indirect via `OwnershipProof` neighbouring types) | No change. |

### New Direct Dependencies (Phase 15 ADDS these to `shared/Cargo.toml`)

| Library | Exact Pin | Purpose | Why Standard |
|---------|-----------|---------|--------------|
| bip322 | `=0.0.10` | Wrap as private adapter inside `mod.rs` per Sprint-0-A:145-175 | ADR Decision #1 ACCEPTED-ADOPT. CC0-1.0 licensed, rust-bitcoin org maintained. Last published ~Sep 2025; v1.4 carry-forward triggers a re-evaluation if 1.0 ships before v1.5. [VERIFIED: crates.io via `cargo search bip322`; slopcheck [OK]; local source at `~/.cargo/registry/src/.../bip322-0.0.10/`] |
| thiserror | `1` (workspace re-export) | Derive `Bip322Error` enum per D-30 | Workspace already pins `thiserror = "1"` at root `Cargo.toml:18`. Phase 15 just adds `thiserror = { workspace = true }` to `shared/Cargo.toml` (currently absent from `shared/`). [VERIFIED: workspace Cargo.toml line 18; slopcheck [OK]] |

### New Dev-Dependencies (Phase 15 ADDS to `shared/Cargo.toml [dev-dependencies]`)

| Library | Exact Pin | Purpose | Why Standard |
|---------|-----------|---------|--------------|
| proptest | `1` (workspace re-export) | Property tests for sign↔verify round-trip per script type; cross-shape rejection matrix | Workspace already pins `proptest = "1"` at root `Cargo.toml:28`. Phase 15 just adds `proptest = { workspace = true }` to `shared/Cargo.toml [dev-dependencies]`. [VERIFIED: workspace Cargo.toml line 28; slopcheck [OK]] |

### Transitive (NOT direct deps; arrive via `bip322 = "=0.0.10"`)

Per Sprint-0-A's cargo tree:
- `snafu v0.8.9` — used internally by the bip322 crate for its error type. We do NOT use snafu directly (D-30); we wrap `bip322::error::Error` via `thiserror`'s `#[source]`.
- `snafu-derive v0.8.9` — proc-macro, compile-time only. Zero runtime attack surface.
- `base64 v0.22.1` — used by the bip322 crate. Already in our tree via the `bitcoin` crate (which carries `base64 v0.21.7`); two `base64` versions coexist in the lockfile per Sprint-0-A. **Phase 15 does NOT add `base64` as a direct dep** — for the `psbt_input_b64` field, use `bitcoin::base64::Engine` re-export OR the workspace `base64` via `client/`'s existing dep declaration. Re-check at plan time which engine is most ergonomic. [VERIFIED: cargo tree in sprint-0-A.md]

### Alternatives Considered (all rejected upstream by ADR or CONTEXT)

| Instead of | Could Use | Why Rejected |
|------------|-----------|--------------|
| `bip322 = "=0.0.10"` wrap | Hand-roll P2TR + P2SH-P2WPKH verifiers in `shared/` (~205 LOC) | ADR Decision #1 ACCEPTED-ADOPT per Sprint-0-A GO verdict. EXTEND would have added ~70 LOC and triple-maintenance vs the crate; ADOPT is 26 LOC. |
| `thiserror` | snafu directly | D-30 locks thiserror — ecosystem default for libraries; existing pattern in `coordinator/src/bitcoin/utxo.rs:8`. Snafu enters transitively via bip322 but is not idiomatic in our error declarations. |
| `thiserror v1.x` | `thiserror v2.x` (latest 2.0.18) | Workspace pins `thiserror = "1"` at line 18 of `Cargo.toml`. Changing to v2 would force `coordinator/src/bitcoin/utxo.rs:8`'s existing `#[derive(thiserror::Error)]` to be re-validated. Phase 15 stays on v1. |
| Vendored `basic-test-vectors.json` | Git submodule pointing to `bitcoin/bips` | D-33 explicit reject: submodule fetches MBs of unrelated BIPs content, slows CI checkout, and ties CI to GitHub availability. Vendoring is supply-chain hardening (v1.3 REPAIR-02 pattern). |
| `OwnershipProof` flat struct (D-22) | Tagged serde enum (`#[serde(tag = "version")]`) | ADR Decision #3 rejected B1. Tagged enum couples wire shape to in-memory dispatch and forces serde re-derive per added script type. |
| `psbt_input_b64: String` (B2) | `final_script_sig: Option<Vec<u8>>` add-on field (B3) | ADR Decision #3 rejected B3. Implicit "if both present, you're P2SH-P2WPKH" re-introduces wire ambiguity REPAIR-01 closed. |

### Installation

Phase 15 adds these to `shared/Cargo.toml` (additions only — existing deps unchanged):

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
bitcoin = { workspace = true }
sha2 = { workspace = true }
uuid = { workspace = true }
hex = "0.4"
# NEW in Phase 15:
bip322 = "=0.0.10"
thiserror = { workspace = true }

[dev-dependencies]
# NEW in Phase 15:
proptest = { workspace = true }
```

**Version verification (executed during research):**

```bash
$ cargo search bip322 --limit 3
bip322 = "0.0.10"       # Implements BIP322 generic message signing and verification
```
[VERIFIED: crates.io, 2026-05-29 — version 0.0.10 confirmed]

```bash
$ cargo search thiserror --limit 1
thiserror = "2.0.18"    # derive(Error)
```
[VERIFIED: crates.io, 2026-05-29 — latest is 2.0.18 but workspace pins v1.x; Phase 15 uses workspace re-export, no change.]

```bash
$ cargo search proptest --limit 1
proptest = "1.11.0"
```
[VERIFIED: crates.io, 2026-05-29 — current 1.11.0; workspace pin `proptest = "1"` accepts this transparently.]

## Package Legitimacy Audit

Verification executed 2026-05-29.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `bip322` | crates.io | ~9 months at 0.0.10 (published Sep 2025); crate exists since 2024 | High (rust-bitcoin org; used by `ord`, `bdk_bip322`) | `github.com/rust-bitcoin/bip322` (canonical, embedded in `.cargo_vcs_info.json`: sha1 `142acfb0...`) | [OK] | Approved — already validated by Sprint-0-A (GO verdict on all 3 D-02 gates: clean `cargo tree`, clean `cargo audit`, 26-LOC adapter under 50-LOC budget) |
| `thiserror` | crates.io | Years; latest v2.0.18 | Top-1000 crate; ecosystem standard | `github.com/dtolnay/thiserror` | [OK] | Approved — workspace already pins `thiserror = "1"` at `Cargo.toml:18` |
| `proptest` | crates.io | Years; latest 1.11.0 | Top-1000 crate | `github.com/proptest-rs/proptest` | [OK] | Approved — workspace already pins `proptest = "1"` at `Cargo.toml:28` |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

**Postinstall script audit:** N/A — Cargo crates have no equivalent of npm `postinstall` scripts. Build scripts (`build.rs`) are the analogous concern; none of these crates ship a `build.rs` that does network calls. `snafu-derive` is a proc-macro (compile-time only); the bip322 crate's `Cargo.toml` (read at `~/.cargo/registry/src/.../bip322-0.0.10/Cargo.toml`) declares `build = false`.

**Note on bip322 crate maturity:** The crate is at `0.0.10` (pre-1.0). ADR Decision #1's Consequences/Negative explicitly accepts this risk, mitigated by exact-pinning (`=0.0.10`) and CI grep gate enforcement. Re-evaluation trigger: if `bip322` ships 1.0 before v1.5 starts.

## Architecture Patterns

### System Architecture Diagram

```
                    ┌─────────────────────────────────────────────┐
                    │   client::round::input::register_input       │
                    │   (v1.3 sign + send path; UNCHANGED in 15)   │
                    └────────────────┬────────────────────────────┘
                                     │ constructs OwnershipProof
                                     │ via to_json_hex_str()
                                     ▼
                    ┌─────────────────────────────────────────────┐
                    │   shared::protocol::OwnershipProof           │
                    │   - version: u8 (default 1)  ← NEW           │
                    │   - witness_stack: Vec<Vec<u8>>              │
                    │   - psbt_input_b64: Option<String>  ← NEW    │
                    │   - script_type: Option<ScriptType>  ← NEW   │
                    │   + from_json_hex_str: 2-phase try-parse     │
                    │     (v1 array-of-hex OR v2 flat-struct)      │
                    └────────────────┬────────────────────────────┘
                                     │ JSON wire envelope
                                     │ over InputRegRequest.ownership_proof
                                     ▼
                    ┌─────────────────────────────────────────────┐
                    │   coordinator::api::handlers::post_input     │
                    │   (calls from_json_hex_str — UNCHANGED in 15)│
                    └────────────────┬────────────────────────────┘
                                     │ ownership_proof: &OwnershipProof
                                     ▼
                    ┌─────────────────────────────────────────────┐
                    │   coordinator::bitcoin::utxo::validate_utxo  │
                    │   (calls verify_bip322_simple — UNCHANGED 15)│
                    │   (deletes local Bip322Error import — phase15│
                    └────────────────┬────────────────────────────┘
                                     │ Phase 16 replaces this
                                     │ call with shared::bip322
                                     ▼ ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
                    ┌──────────────────────────────────────────────┐
                    │   shared::bip322 (NEW MODULE STRUCTURE)      │
                    │   ┌─────────────────────────────────────┐    │
                    │   │ mod.rs                              │    │
                    │   │  - pub enum ScriptType              │    │
                    │   │  - pub enum Bip322Error             │    │
                    │   │  - pub fn detect_script_type        │    │
                    │   │  - pub fn verify_simple (dispatcher)│    │
                    │   │  - pub fn sign_simple (dispatcher)  │    │
                    │   │  - pub re-exports: bip322_message_  │    │
                    │   │    hash, build_bip322_to_spend/sign │    │
                    │   │  - crate-private verify_via_bip322_ │    │
                    │   │    crate(spk, witness, msg, network)│    │
                    │   │    [Sprint-0-A 26-LOC adapter]      │    │
                    │   └──────────────┬──────────────────────┘    │
                    │                  │                            │
                    │   ┌──────────────┴────┬──────────────────┐   │
                    │   ▼                   ▼                  ▼   │
                    │ p2wpkh.rs          p2tr.rs        p2sh_p2wpkh│
                    │ pub(crate) inner   pub(crate)    pub(crate)  │
                    │ BIP-143 ECDSA      BIP-341       BIP-143 over│
                    │ + arity check      Schnorr       unwrapped   │
                    │                   (64/65 byte)  P2WPKH + H160│
                    └────────────────────┬──────────────────────────┘
                                         │ private adapter call
                                         ▼
                    ┌──────────────────────────────────────────────┐
                    │   bip322 = "=0.0.10" (crate, wrapped private) │
                    │   - pub fn verify_simple(&Address, msg, Witns)│
                    │   - returns Result<(), bip322::error::Error>  │
                    │   - routes P2WPKH/P2TR/P2SH internally        │
                    │   [VERIFIED at ~/.cargo/.../bip322-0.0.10/]   │
                    └──────────────────────────────────────────────┘

╔══════════════════════════════════════════════════════════════════╗
║  TEST HARNESS (Phase 15 ships THREE plans, in order)             ║
╠══════════════════════════════════════════════════════════════════╣
║  15-01: shared/tests/ownership_proof_roundtrip.rs                ║
║    - 5 D-13 cases: v2 self-roundtrip × 3 script types, v1 legacy ║
║      decode, declared-vs-PSBT mismatch, unknown version reject,  ║
║      corrupted base64/PSBT reject                                ║
║    SHIPS FIRST as own atomic commit (REPAIR-01 lesson #1)        ║
║                                                                    ║
║  15-02: shared/src/bip322/{mod,p2wpkh,p2tr,p2sh_p2wpkh}.rs       ║
║    - Module split, dispatcher, adapter, Bip322Error             ║
║                                                                    ║
║  15-03: shared/tests/bip322_cross_shape.rs                       ║
║    - 9 enumerated #[test] off-diagonal rejections (D-34)         ║
║    + shared/tests/bip322_per_script.rs                           ║
║    - per-script positive vectors against vendored fixture        ║
║    - sign_simple shape with #[cfg(test)] body for round-trip     ║
║  shared/tests/fixtures/bip322/basic-test-vectors.json (vendored) ║
║  shared/tests/fixtures/bip322/README.md (commit SHA + curl cmd)  ║
╚══════════════════════════════════════════════════════════════════╝
```

### Recommended Project Structure

```
shared/
├── Cargo.toml                                   # +bip322=0.0.10, +thiserror, +proptest(dev)
├── src/
│   ├── lib.rs                                   # UNCHANGED — pub mod bip322 still resolves to bip322/
│   ├── bip322/                                  # NEW directory (replaces flat bip322.rs)
│   │   ├── mod.rs                               # Public API + dispatcher + adapter + Bip322Error
│   │   ├── p2wpkh.rs                            # pub(crate) — carried over from existing impl
│   │   ├── p2tr.rs                              # pub(crate) — new, BIP-341 keypath
│   │   └── p2sh_p2wpkh.rs                       # pub(crate) — new, BIP-143 over unwrapped + H160
│   ├── errors.rs                                # UNCHANGED (ErrorCode::InvalidOwnershipProof exists)
│   ├── protocol.rs                              # OwnershipProof evolved to flat struct + helpers
│   ├── token.rs                                 # UNCHANGED
│   └── types.rs                                 # UNCHANGED
└── tests/                                       # NEW dir (currently absent)
    ├── ownership_proof_roundtrip.rs             # 15-01 atomic commit; 5 D-13 cases
    ├── bip322_per_script.rs                     # 15-03; positive vectors per script
    ├── bip322_cross_shape.rs                    # 15-03; 9-rejection matrix
    └── fixtures/
        └── bip322/
            ├── basic-test-vectors.json          # vendored from bitcoin/bips@<SHA>
            ├── p2sh_p2wpkh_supplement.json      # supplement — upstream has none
            └── README.md                        # commit SHA + curl cmd per D-33

coordinator/src/bitcoin/utxo.rs                   # Phase 15: ONLY DELETE lines 87-101 (local Bip322Error)
                                                  #          and replace import to shared::bip322::Bip322Error
                                                  # Phase 16 will replace verify_bip322_simple call site

client/                                           # Phase 15: ZERO changes
                                                  # Phase 17 will extend wallet.rs + round/input.rs

tests/integration/full_round.rs                   # UNCHANGED — must stay green (cross-phase invariant)
```

### Pattern 1: Dispatcher with hidden inner modules

**What:** Public API exposes ONE dispatcher fn per operation (verify, sign); inner per-script modules are `pub(crate)` only. Caller cannot bypass dispatch.

**When to use:** Whenever there's a typed enum that fans out to N implementations and you want compile-time enforcement that nobody can call the wrong impl.

**Example:**

```rust
// shared/src/bip322/mod.rs
// Source: synthesized from Sprint-0-A:145-175 + CONTEXT D-27 + bip322 crate's lib.rs:29

use bitcoin::{Address, Network, Script, Witness};
use bitcoin::secp256k1::SecretKey;

mod p2wpkh;
mod p2tr;
mod p2sh_p2wpkh;

// Script-type-NEUTRAL primitives kept public for V1.4-MOD-07 single-source-of-truth.
// These are carried over from the existing shared/src/bip322.rs verbatim.
pub use self::primitives::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    P2wpkh,
    P2tr,
    #[serde(rename = "p2sh-p2wpkh")]
    P2shP2wpkh,
}

#[derive(Debug, thiserror::Error)]
pub enum Bip322Error {
    #[error("unsupported OwnershipProof version: {0}")]
    UnsupportedProofVersion(u8),
    #[error("wire-format mismatch: {0}")]
    WireFormatMismatch(String),
    // ... (10 variants total per D-31; see Bip322Error taxonomy below)
}

pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error> {
    if spk.is_p2wpkh() {
        Ok(ScriptType::P2wpkh)
    } else if spk.is_p2tr() {
        Ok(ScriptType::P2tr)
    } else if spk.is_p2sh() {
        // NOTE: is_p2sh() alone cannot distinguish P2SH-P2WPKH from raw P2SH-multisig.
        // The on-chain SPK is only the HASH160; the wire MUST carry the redeem script
        // (PSBT-input shape per D-23) for the dispatcher to confirm it's P2WPKH-wrapped.
        // This caller-supplied disambiguation happens in verify_simple via the witness
        // shape check at p2sh_p2wpkh.rs.
        Ok(ScriptType::P2shP2wpkh)
    } else {
        Err(Bip322Error::UnsupportedScriptType)
    }
}

pub fn verify_simple(
    script_type: ScriptType,
    spk: &Script,
    witness: &Witness,
    message: &[u8],
    network: Network,
) -> Result<(), Bip322Error> {
    // D-28 invariant: per-script files are pub(crate) only.
    // The dispatcher routes; no caller can reach a per-script verifier directly.
    match script_type {
        ScriptType::P2wpkh => p2wpkh::verify(spk, witness, message, network),
        ScriptType::P2tr => p2tr::verify(spk, witness, message, network),
        ScriptType::P2shP2wpkh => p2sh_p2wpkh::verify(spk, witness, message, network),
    }
}

pub fn sign_simple(
    script_type: ScriptType,
    spk: &Script,
    key: &SecretKey,
    message: &[u8],
) -> Result<Witness, Bip322Error> {
    match script_type {
        ScriptType::P2wpkh => p2wpkh::sign(spk, key, message),
        // CD-6 default: P2TR and P2SH-P2WPKH production paths are todo!() in Phase 15;
        // Phase 17 WALLET-02 swaps these for bdk-backed signing in client/.
        ScriptType::P2tr => todo!("Phase 17 WALLET-02: bdk_wallet sign path per ADR #4"),
        ScriptType::P2shP2wpkh => todo!("Phase 17 WALLET-02: bdk_wallet sign path per ADR #4"),
    }
}
```

### Pattern 2: 26-LOC crate adapter (Sprint-0-A verbatim)

**What:** Wrap `bip322 = "=0.0.10"` `verify_simple(&Address, msg, Witness)` into our wire shape `(spk, witness, msg, network)`. Verified faithful to crate at `~/.cargo/.../bip322-0.0.10/src/verify.rs:46-58`.

**When to use:** Inside each per-script verifier file (or as a crate-private helper in `mod.rs`), wrap the crate once. **Do NOT call the crate from per-script files directly more than once each** — the adapter is the single source of truth for crate-vs-our error mapping.

**Example:**

```rust
// shared/src/bip322/mod.rs (crate-private helper)
// Source: .planning/research/sprint-0-A.md lines 145-175 verbatim

use bitcoin::{Address, Network, Script, Witness};

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

**Faithfulness notes (verified against crate source):**
- `bip322::verify_simple` signature confirmed at `verify.rs:46-50`: `pub fn verify_simple(address: &Address, message: impl AsRef<[u8]>, signature: Witness) -> Result<(), Error>`. Takes `Witness` by **value**, not reference — `witness.clone()` is required (cheap; `bitcoin::Witness::clone` is `derive(Clone)`).
- `Address::from_script(spk, network)` returns `Result<Address, FromScriptError>` — matches D-31's `UnrecognisedScriptPubkey { #[source]: bitcoin::address::FromScriptError }`.
- The crate's `verify_simple` internally routes to P2WPKH (`verify_full_p2wpkh(is_p2sh=false)`), P2TR (`verify_full_p2tr`), and P2SH-P2WPKH (`verify_full_p2wpkh(is_p2sh=true)`) per `verify.rs:62-99` — covers all three v1.4 script types.

### Pattern 3: Two-phase try-parse for v1↔v2 wire coexistence

**What:** Legacy decoder accepts BOTH v1.3 array-of-hex JSON AND new flat-struct JSON. CD-7 default.

**When to use:** Any time a wire format evolves and the old shape MUST still decode for at least one transition milestone (here: v1.4 → v1.5).

**Example:**

```rust
// shared/src/protocol.rs (replaces the existing OwnershipProof impl block)
// Source: synthesized from CONTEXT D-22..D-25 + CD-7 default

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipProof {
    #[serde(default = "default_proof_version")]
    pub version: u8,
    #[serde(default)]
    pub witness_stack: Vec<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psbt_input_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_type: Option<crate::bip322::ScriptType>,
}

fn default_proof_version() -> u8 { 1 }

impl OwnershipProof {
    /// Two-phase try-parse per CD-7.
    pub fn from_json_hex_str(s: &str) -> Result<Self, String> {
        // Phase 1: try v1.3 array-of-hex shape (existing v1.3 client wire form)
        if let Ok(items) = serde_json::from_str::<Vec<String>>(s) {
            let witness_stack = items
                .iter()
                .map(|h| hex::decode(h).map_err(|e| format!("hex decode: {e}")))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Self {
                version: 1,
                witness_stack,
                psbt_input_b64: None,
                script_type: None,
            });
        }
        // Phase 2: fall back to the flat-struct shape (covers both v1 explicit
        // and v2 envelopes)
        serde_json::from_str::<Self>(s)
            .map_err(|e| format!("OwnershipProof parse: {e}"))
    }

    /// Backwards-compatible encoder per CD-7.
    /// version=1 + witness_stack-only → emit v1.3 array-of-hex (wire-compatible
    /// with v1.3 coordinators that have not yet read this struct's flat-struct shape).
    /// Otherwise emit the flat-struct JSON.
    pub fn to_json_hex_str(&self) -> String {
        if self.version == 1
            && self.psbt_input_b64.is_none()
            && self.script_type.is_none()
        {
            let hex_items: Vec<String> = self.witness_stack.iter().map(hex::encode).collect();
            return serde_json::to_string(&hex_items)
                .expect("Vec<String> always serializes");
        }
        serde_json::to_string(self).expect("OwnershipProof serializes")
    }
}
```

### Pattern 4: `#[cfg(test)]` body for `sign_simple` round-trip enablement

**What:** Phase 15's `sign_simple` has a `todo!()` for P2TR/P2SH-P2WPKH in production but a `#[cfg(test)]`-only manual implementation that enables `shared/tests/` to run end-to-end sign↔verify property tests. Phase 17 swaps the production body to bdk; the `#[cfg(test)]` helper stays for the property-test surface.

**When to use:** When an API contract must be type-checked across the crate AND tested in isolation, but the production implementation depends on a downstream crate's signing primitive (bdk_wallet here).

**Example:**

```rust
// shared/src/bip322/p2tr.rs (pub(crate) inner mechanics)
// Source: synthesized from CD-6 default + Sprint-0-B step 6-8 logic

pub(crate) fn verify(
    spk: &bitcoin::Script,
    witness: &bitcoin::Witness,
    message: &[u8],
    network: bitcoin::Network,
) -> Result<(), super::Bip322Error> {
    super::verify_via_bip322_crate(spk, witness, message, network)
}

pub(crate) fn sign(
    _spk: &bitcoin::Script,
    _key: &bitcoin::secp256k1::SecretKey,
    _message: &[u8],
) -> Result<bitcoin::Witness, super::Bip322Error> {
    todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")
}

#[cfg(test)]
pub(crate) fn sign_for_tests(
    spk: &bitcoin::Script,
    key: &bitcoin::secp256k1::SecretKey,
    message: &[u8],
) -> bitcoin::Witness {
    // Reuses Sprint-0-B's verified 8-step sequence (sprint-0-B.md:130-270),
    // condensed to: build to_spend, build to_sign, taproot_key_spend_signature_hash,
    // tap-tweak the keypair, sign_schnorr_no_aux_rand, return 64-byte witness.
    // [referenced from sprint-0-B.md:155-216 + bip322 crate's sign.rs:155-216]
    use bitcoin::secp256k1::Secp256k1;
    use bitcoin::sighash::{Prevouts, SighashCache, TapSighashType};
    use bitcoin::key::{Keypair, TapTweak};
    use bitcoin::{Amount, TxOut, Witness};

    let msg_hash = super::bip322_message_hash(message);
    let to_spend = super::build_bip322_to_spend(spk, &msg_hash);
    let to_sign = super::build_bip322_to_sign(&to_spend);

    let secp = Secp256k1::new();
    let keypair = Keypair::from_secret_key(&secp, key);
    let tweaked = keypair.tap_tweak(&secp, None).to_keypair();

    let mut cache = SighashCache::new(&to_sign);
    let sighash = cache.taproot_key_spend_signature_hash(
        0,
        &Prevouts::All(&[TxOut {
            value: Amount::ZERO,
            script_pubkey: spk.to_owned(),
        }]),
        TapSighashType::Default,
    ).expect("sighash on well-formed to_sign");

    let sig = secp.sign_schnorr_no_aux_rand(
        &bitcoin::secp256k1::Message::from_digest(sighash.to_byte_array()),
        &tweaked,
    );

    let mut w = Witness::new();
    w.push(sig.as_ref().to_vec());
    w
}
```

This pattern keeps `shared/tests/` self-contained (no `bdk_wallet` dep on the `shared/` crate; bdk stays in `client/`).

### Anti-Patterns to Avoid

- **`match ownership_proof.script_type` in coordinator** — V1.4-CRIT-01 spoofing vector. Phase 16 will derive `script_type` from on-chain SPK via `detect_script_type` and cross-check; Phase 15's API surface enforces this by making per-script verifiers `pub(crate)`.
- **A `pub fn verify_p2wpkh` / `verify_p2tr` / `verify_p2sh_p2wpkh` in `shared::bip322`** — defeats D-27's "dispatcher-only public" invariant. Caller could accidentally invoke the wrong verifier.
- **A shared sighash helper that switches on `script_type` internally** — concentrates exactly the bug class V1.4-CRIT-02 warns against. Three SEPARATE verifier functions in `p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`, even if their bodies are 80% similar.
- **Adding `bitcoin::base64::Engine` as a direct dependency** — `bitcoin 0.32.x` already re-exports a base64 engine. For `psbt_input_b64` encode/decode, prefer the re-export (no new direct dep).
- **`from_der` or `Signature::from_der` in the P2TR verifier path** — P2TR uses 64-byte Schnorr, not DER ECDSA. Code-review red flag if seen in `p2tr.rs`.
- **`#[serde(deny_unknown_fields)]` on `OwnershipProof`** — violates T-01-04 forward-compat invariant. Existing code at `shared/src/protocol.rs:3` documents this; Phase 15 preserves it.
- **`#[serde(tag = "version")]` on `OwnershipProof`** — would force a tagged-enum shape that ADR Decision #3 rejected. Use plain struct with `#[serde(default)]` on `version` per D-22.
- **A `git submodule` for `bitcoin/bips`** — D-33 explicitly rejects this; vendor the single JSON file with a header comment.
- **Leaving the coordinator's `Bip322Error` enum in place at `utxo.rs:87-101`** — D-29 mandates its deletion in Phase 15. Phase 15's planner must NOT skip this — the v1.3 P2WPKH path STAYS, but the local error enum is replaced by `shared::bip322::Bip322Error` import.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| BIP-322 P2TR Schnorr keypath verify | A custom `taproot_key_spend_signature_hash` + `verify_schnorr` chain | `bip322 = "=0.0.10"` `verify_simple` (via the 26-LOC adapter) | Crate covers SIGHASH_DEFAULT 64-byte AND SIGHASH_ALL 65-byte cases at `verify.rs:214-231`; correctly handles `XOnlyPublicKey::from_slice` from the SPK at `verify.rs:71`. Hand-rolling re-introduces V1.4-CRIT-02 risk. |
| BIP-322 P2SH-P2WPKH unwrap + BIP-143 over redeem | A custom `HASH160(redeem) == p2sh_hash` check + manual P2WPKH sighash | bip322 crate's `verify_full_p2wpkh(is_p2sh: true)` at `verify.rs:167-169` (via our adapter) | Crate correctly reconstructs the unwrapped P2WPKH SPK from `pub_key.wpubkey_hash()` and sighashes over it. Hand-rolling repeats the witness-program-extraction logic we already get from `bip322::verify_simple`. |
| Schnorr signature parsing (64 vs 65 byte) | A custom `match encoded_signature.len()` | bip322 crate's verify (handles both internally) | Same crate already branches at `verify.rs:214-231`. We never see the raw sighash type byte. |
| `OwnershipProof` versioned envelope encode/decode | A bespoke binary header + custom byte parser | serde flat-struct with `#[serde(default)]` + 2-phase try-parse | serde already gives us field-default semantics + forward compat (`!deny_unknown_fields`). The bespoke route re-introduces the v1.3 REPAIR-01 wire-format-mismatch class of bugs. |
| Tagged-hash for BIP-322 message | Re-implement `SHA256(tag) \|\| SHA256(tag) \|\| message` | Existing `shared::bip322::bip322_message_hash` (V1.4-MOD-07 single source of truth; carried over from v1.3 implementation; matches crate's `tagged_hash(BIP322_TAG, message)` per `util.rs:6-14` byte-for-byte) | V1.4-MOD-07 explicit: every verifier reuses this one fn. Reimplementing is the legacy/BIP-322 confusion vector. |
| Per-script error code mapping | New `ErrorCode::InvalidP2trProof / InvalidP2shProof / InvalidP2wpkhProof` variants in `shared::errors` | Existing `ErrorCode::InvalidOwnershipProof` (single bucket) | D-32 locks the single-bucket mapping. Per-script error codes leak script-type fingerprint (REQUIREMENTS.md Out-of-Scope anti-feature). |
| Vendored test vectors fetch at build time | A `build.rs` that downloads `basic-test-vectors.json` from GitHub | Manual one-time vendor + commit-SHA header per D-33 | v1.3 REPAIR-02 carry-forward: zero network in CI. `build.rs` fetch is a supply-chain hole. |
| `cfg` gating for `sign_simple` test body | A separate `signing_test_helpers` crate | `#[cfg(test)] pub(crate) fn sign_for_tests` inline in each per-script file | Keeps shared/ self-contained; matches the existing `shared/src/bip322.rs::tests::make_bip322_witness` pattern at lines 86-108 (already structurally identical). |
| Address-network ambiguity for P2SH-P2WPKH | A custom network-aware address-string-prefix matcher | `bitcoin::Address::from_script(spk, Network::Regtest)` (the SPK is network-agnostic at the script level — only the printable address differs) | P2SH-P2WPKH `script_pubkey` is `OP_HASH160 <20-byte-hash> OP_EQUAL` — byte-identical across mainnet/testnet/signet/regtest. The `Network` parameter at the adapter only affects address pretty-printing in error messages. Confirmed: bip322 crate's `Address::from_script` returns the right `Address` shape for any network. |

**Key insight:** The bip322 crate adoption (ADR Decision #1) is the single highest-leverage anti-hand-rolling decision in v1.4. Its `verify_full_p2wpkh(is_p2sh: bool)` covers TWO of the three v1.4 script types with one parameter; `verify_full_p2tr` covers the third. The crate's internal sighash math is identical to what a careful hand-roll would produce (verified by reading `verify.rs:101-258`), but the crate carries upstream maintenance and test coverage we'd otherwise own forever. Phase 15's owned LOC for verify drops from ~205 (extend path) to ~26 (the adapter).

## Runtime State Inventory

> Phase 15 is a code-only refactor of `shared/`. Phase 15 is NOT a rename or migration — there are no databases, services, OS-registered tasks, or environment variables whose semantics change. The Runtime State Inventory is therefore largely empty.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — `shared/` is a pure library crate; no SQLite, no on-disk state. The coordinator's `ban_list.db` (sqlx-managed) is keyed by `OutPoint` (script-type-agnostic by design per REQUIREMENTS.md Out-of-Scope "per-script-type ban tracking"); Phase 15 does NOT change that schema. | none |
| Live service config | None — Phase 15 does not change `coordinator.toml` or `client.toml` shape. Phase 16 will add `[bip] allow_*` keys; Phase 15 doesn't touch config. n8n / Datadog / Tailscale: N/A — blindjoin doesn't use them. | none |
| OS-registered state | None — no Task Scheduler / launchd / systemd / pm2. Coordinator runs under Docker per CLAUDE.md. | none |
| Secrets / env vars | None — Phase 15 does not introduce new env vars. The existing `BLINDJOIN__COORDINATOR__*` prefix is unchanged. RSA keypair handling unchanged. | none |
| Build artifacts / installed packages | `Cargo.lock` will gain `bip322 v0.0.10`, `snafu v0.8.9`, `snafu-derive v0.8.9` per Sprint-0-A's cargo tree probe. The lockfile dependency count rises from current to 710 (Sprint-0-A measured). | Lock file change is expected and reviewable in the Phase 15 PR. The CI grep gate addition will assert the exact `=0.0.10` pin survives any future `cargo update`. |

**Nothing found in category** is the correct answer for 4 of 5 categories; the build-artifact row is the only one with a non-empty action.

## Common Pitfalls

### Pitfall 1: Confusing the crate's `to_spend` / `to_sign` helpers with ours

**What goes wrong:** The bip322 crate exports `create_to_spend(&Address, msg) -> Result<Transaction>` and `create_to_sign(&Transaction, Option<Witness>) -> Result<Psbt>` at `lib.rs:29` via `pub use util::*`. These are NOT byte-identical to our `shared::bip322::build_bip322_to_spend(&Script, &msg_hash)` and `build_bip322_to_sign(&Transaction)`.

**Why it happens:** Both crates implement the same BIP-322 spec, so the resulting `Transaction` is identical when given equivalent inputs. But (a) the crate's helpers take `&Address` not `&Script` (network coupling); (b) the crate's `create_to_sign` returns a `Psbt`, not a `Transaction`; (c) the crate's `create_to_spend` returns `Result<Transaction>`, ours returns `Transaction` infallibly.

**How to avoid:** V1.4-MOD-07 invariant: our `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign` STAY as the single source of truth. The crate's `create_to_spend` / `create_to_sign` are wrapped INSIDE the crate's own `verify_simple` — we never call them from `shared/`. Our adapter only calls `bip322::verify_simple(&Address, msg, Witness)`.

**Warning signs:** A `use bip322::{create_to_spend, create_to_sign}` import in any `shared/src/bip322/*.rs` file is a code-review red flag.

### Pitfall 2: `Witness::clone()` cost per verification under load

**What goes wrong:** The crate's `verify_simple(address, message, witness: Witness)` takes the witness BY VALUE. The Sprint-0-A adapter clones at the call site. Under high coordinator throughput (e.g., 25 participants × 1 verification each per round, every 5 minutes), the clone cost adds up.

**Why it happens:** Rust's ownership model; the crate API was designed before allocation pressure mattered.

**How to avoid:** **Negligible at our throughput.** `bitcoin::Witness` is a `Vec<Vec<u8>>`-shaped container; typical BIP-322 witnesses are 2-3 items totalling < 200 bytes; clone is < 1 µs per call. At 25 verifications per 5-minute round, the budget is < 25 µs per round. Below measurement threshold.

**Warning signs:** If a future phase introduces per-request BIP-322 verification at hundreds of QPS (e.g., a public verify API), revisit this. For v1.4's CoinJoin round throughput, ignore.

### Pitfall 3: `Address::from_script` rejecting "valid" but unusual SPK shapes

**What goes wrong:** `bitcoin::Address::from_script(spk, network)` returns `Err(FromScriptError::UnrecognizedScript)` for SPK shapes that aren't standard single-key addresses (e.g., bare multisig, future witness versions). Our adapter maps this to `Bip322Error::UnrecognisedScriptPubkey`, which then maps to `ErrorCode::InvalidOwnershipProof` per D-32.

**Why it happens:** v1.4 explicitly out-of-scopes P2WSH multisig and P2TR script-path (REQUIREMENTS.md "Out of Scope" table). These SPK shapes legitimately reach the adapter and legitimately reject.

**How to avoid:** This is **desired behavior**, not a bug. The 9-rejection matrix in D-34 includes empty-witness cases that exercise this path. The error message `"script_pubkey is not a recognised single-key address (P2WPKH / P2TR / P2SH-P2WPKH)"` is operator-facing and accurate.

**Warning signs:** If a real participant's UTXO triggers `UnrecognisedScriptPubkey` on the legitimate path, that's a script-type they shouldn't be registering — the error UX (single-bucket `INVALID_OWNERSHIP_PROOF` per D-32) is the correct outcome.

### Pitfall 4: `thiserror` `#[source]` chain Display not propagating through `bitcoin::address::FromScriptError`

**What goes wrong:** `Bip322Error::UnrecognisedScriptPubkey { #[source]: bitcoin::address::FromScriptError }` may not produce a chained `Display` showing the inner reason. `bitcoin::address::FromScriptError` is `pub enum` with several variants and its own `Display` impl. The user-facing message may be just `"script_pubkey is not a recognised single-key address ..."` without the inner detail.

**Why it happens:** `thiserror`'s `#[error("...")]` attribute determines the top-level message; `#[source]` only populates the cause chain accessible via `error.source()`. Default `Display` does NOT include the chain.

**How to avoid:** **Acceptable for the wire** — per D-32, the only thing that crosses the wire is `ErrorCode::InvalidOwnershipProof` + a short `message` string. The full chain is preserved for **server-side logging** (where the coordinator can `tracing::warn!(error = ?e, error.source = ?e.source(), "ownership proof rejected")`) and pattern-matching on the typed enum. Internal observability is preserved; the wire is opaque by design.

**Warning signs:** If a Phase 16 / handlers.rs reviewer asks "why is the error message just one line?", point them at D-32 — the wire is single-bucket by design.

### Pitfall 5: P2SH-P2WPKH `Network` ambiguity (encoding is network-agnostic)

**What goes wrong:** P2SH-P2WPKH `script_pubkey` is `OP_HASH160 <20-byte-hash> OP_EQUAL` — byte-identical across mainnet / testnet / signet / regtest. The differentiator is only the printable address prefix (`3...` mainnet vs `2...` testnet). The Sprint-0-A adapter takes `Network` as an explicit argument; for P2SH SPKs, passing the wrong `Network` doesn't change verification correctness — it only affects `Address::from_script`'s printable form.

**Why it happens:** Bitcoin's address encoding has network-specific prefixes; the script bytes do not.

**How to avoid:** Pass the operator-configured `Network` from `coordinator.toml` (`[bitcoin] network = "signet"` already exists from v1.0) verbatim. The coordinator MUST know its own network anyway. The `Bip322Error::NetworkMismatch { decoded, configured }` variant exists to catch the case where an address string with the wrong network prefix slips in (e.g., a v1.4 client misconfigured for mainnet talks to a signet coordinator) — but for the SPK-byte path used in `verify_simple`, network mismatches don't cause verify-side false positives.

**Warning signs:** A planner proposing to "infer Network from the address string" — Sprint-0-A's adapter shape (Network as explicit argument) is the correct decoupling.

### Pitfall 6: The vendored fixture's lack of P2SH-P2WPKH vectors

**What goes wrong:** The upstream `bitcoin/bips/bip-0322/basic-test-vectors.json` contains P2WPKH, P2TR, and P2WSH-multisig vectors only — NO P2SH-P2WPKH. Phase 15's per-script property tests (BIP322-04) cannot exhaustively cover P2SH-P2WPKH from upstream alone.

**Why it happens:** The upstream test file was authored before P2SH-P2WPKH support became a common reference case; nobody added vectors.

**How to avoid (recommended):** Supplement the vendored upstream snapshot with the bip322 crate's own P2SH-P2WPKH test constants at `~/.cargo/registry/src/.../bip322-0.0.10/src/lib.rs:46-48` (`NESTED_SEGWIT_ADDRESS = "3HSVzEhCFuH9Z3wvoWTexy7BMVVp3PjS6f"`, WIF private key, message-signature pairs at lib.rs:284-335). Ship this as `shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json` with a header explaining the supplement and pointing at the crate version it was lifted from. Format the supplement to match the upstream `simple` array entry shape (`message`, `private_keys`, `address`, `type: "p2sh-p2wpkh"`, `witness_script: ""`, `bip322_signatures`) so the per-script harness can iterate both files uniformly.

**Warning signs:** A planner who says "we'll just skip P2SH-P2WPKH in the property test against `basic-test-vectors.json`" — that leaves BIP322-04 incomplete for one of the three script types and breaks the per-script symmetry. Supplement, don't skip.

### Pitfall 7: `from_json_hex_str` returning `Result<Self, String>` instead of `Result<Self, Bip322Error>`

**What goes wrong:** The existing v1.3 `OwnershipProof::from_json_hex_str` returns `Result<Self, String>` (untyped error). Phase 15's expansion is tempting to type as `Result<Self, Bip322Error>` for consistency with the new dispatcher API. But `OwnershipProof` lives in `shared/src/protocol.rs` (D-28) and `Bip322Error` lives in `shared/src/bip322/mod.rs` — typing the helper that way would force `protocol.rs` to import from `bip322/` (cycle).

**Why it happens:** Tempting refactor at the wrong abstraction level.

**How to avoid:** Keep `from_json_hex_str` returning `Result<Self, String>` (preserves v1.3 API). The handler-layer call site at `coordinator/src/api/handlers.rs:136-137` already maps the string error to `INVALID_PROOF`. Phase 15's typed `Bip322Error` is for the VERIFY path (which is downstream of the decode); the wire-decode error stays untyped to avoid the module cycle and preserve handler compatibility.

**Warning signs:** A planner inserting `use crate::bip322::Bip322Error` at the top of `protocol.rs` — that's the cycle. Reject.

## Code Examples

Verified patterns from existing code and the bip322 crate source:

### Example 1: P2WPKH verify via the adapter (Phase 15 minimum body)

```rust
// shared/src/bip322/p2wpkh.rs
// Source: synthesized from sprint-0-A.md:145-175 + bip322 crate verify.rs:101-185

pub(crate) fn verify(
    spk: &bitcoin::Script,
    witness: &bitcoin::Witness,
    message: &[u8],
    network: bitcoin::Network,
) -> Result<(), super::Bip322Error> {
    // Arity check moved to the dispatcher level OR kept here per CONTEXT — D-34's
    // `reject_p2wpkh_spk_with_empty_witness` test asserts the InvalidWitnessLength
    // path so the check MUST exist somewhere on the path. Putting it here keeps
    // each per-script file self-checking.
    if witness.len() != 2 {
        return Err(super::Bip322Error::InvalidWitnessLength {
            expected: 2,
            got: witness.len(),
        });
    }
    super::verify_via_bip322_crate(spk, witness, message, network)
}
```

### Example 2: P2TR verify (just delegates — the crate handles 64/65-byte branching internally)

```rust
// shared/src/bip322/p2tr.rs
// Source: synthesized from CONTEXT D-04 / D-27 + bip322 crate verify.rs:187-258

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
    // The crate's verify_full_p2tr handles both SIGHASH_DEFAULT (64-byte) and
    // SIGHASH_ALL (65-byte) sig forms at verify.rs:214-231 — REQUIREMENTS.md
    // BIP322-02 satisfied by the crate, not by us.
    super::verify_via_bip322_crate(spk, witness, message, network)
}
```

### Example 3: P2SH-P2WPKH verify with explicit HASH160 cross-check

```rust
// shared/src/bip322/p2sh_p2wpkh.rs
// Source: synthesized from REQUIREMENTS BIP322-02 + bip322 crate verify.rs:87-94

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
    // The crate's verify_simple internally:
    //   1. Address::from_script(spk, network) → AddressData::P2sh { script_hash }
    //   2. Builds the unwrapped P2WPKH redeem from witness[1] (the pubkey)
    //   3. HASH160 cross-check is implicit in verify_full_p2wpkh(is_p2sh=true)
    //      where the sighash uses ScriptBuf::new_p2wpkh(pub_key.wpubkey_hash())
    //      not the original SPK — if the pubkey doesn't match the script's hash,
    //      sighash verification fails.
    super::verify_via_bip322_crate(spk, witness, message, network)
}
```

### Example 4: P2WPKH sign (Phase 15 ships this fully — carries over the v1.3 path)

```rust
// shared/src/bip322/p2wpkh.rs (full sign body, Phase 15-deliverable)
// Source: lifted verbatim from existing shared/src/bip322.rs:86-108 (the make_bip322_witness
// test helper generalised to a pub(crate) sign fn for the dispatcher)

pub(crate) fn sign(
    spk: &bitcoin::Script,
    key: &bitcoin::secp256k1::SecretKey,
    message: &[u8],
) -> Result<bitcoin::Witness, super::Bip322Error> {
    use bitcoin::secp256k1::{Secp256k1, Message};
    use bitcoin::sighash::{SighashCache, EcdsaSighashType};
    use bitcoin::{Amount, Witness};

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
    w.push(pubkey.serialize().to_vec());
    Ok(w)
}
```

### Example 5: Wire-format roundtrip test (15-01-PLAN.md atomic commit)

```rust
// shared/tests/ownership_proof_roundtrip.rs
// Source: synthesized from D-13 cases + CD-7 default

use shared::bip322::ScriptType;
use shared::protocol::OwnershipProof;

#[test]
fn v2_roundtrip_p2wpkh() {
    let proof = OwnershipProof {
        version: 2,
        witness_stack: vec![],
        psbt_input_b64: Some("cHNidP8B...".into()), // realistic base64
        script_type: Some(ScriptType::P2wpkh),
    };
    let json = proof.to_json_hex_str();
    let parsed = OwnershipProof::from_json_hex_str(&json).expect("v2 roundtrip");
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.script_type, Some(ScriptType::P2wpkh));
}

#[test]
fn v2_roundtrip_p2tr() { /* same shape, ScriptType::P2tr */ }

#[test]
fn v2_roundtrip_p2sh_p2wpkh() { /* same shape, ScriptType::P2shP2wpkh */ }

#[test]
fn v1_legacy_decode_array_of_hex() {
    let v1_wire = r#"["3045022100abcd","02ab1234"]"#;
    let parsed = OwnershipProof::from_json_hex_str(v1_wire)
        .expect("v1 array-of-hex must decode");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.witness_stack.len(), 2);
    assert!(parsed.psbt_input_b64.is_none());
    assert!(parsed.script_type.is_none());
}

#[test]
fn unknown_version_rejects_on_verify_dispatch() {
    // version=3 deserialises (we don't reject at decode — D-25 default semantics)
    // but the coordinator's match-on-version downstream rejects with
    // UnsupportedProofVersion. We assert here by hand-constructing the wire form
    // and exercising the planned match arm.
    let wire = r#"{"version":3,"witness_stack":[]}"#;
    let parsed = OwnershipProof::from_json_hex_str(wire).expect("decode succeeds");
    assert_eq!(parsed.version, 3);
    // (The version=3 → UnsupportedProofVersion rejection is exercised at the
    // verify dispatch layer; this test confirms decode is permissive.)
}

#[test]
fn corrupted_base64_in_psbt_input_rejects_on_decode() {
    // version=2 + non-base64 psbt_input_b64 → must NOT panic; surfaces typed error
    // when downstream code attempts to base64::decode the payload.
    let wire = r#"{"version":2,"psbt_input_b64":"not-base64-!!!","script_type":"p2wpkh"}"#;
    let parsed = OwnershipProof::from_json_hex_str(wire).expect("JSON decode itself is OK");
    // The base64 decode failure will surface in the planned downstream
    // psbt_input decode step (Phase 16 work) as Bip322Error::DecodeError.
    // Phase 15's roundtrip test asserts the JSON layer is permissive and
    // doesn't panic.
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.psbt_input_b64.as_deref(), Some("not-base64-!!!"));
}
```

### Example 6: 9-rejection matrix entry (15-03-PLAN.md)

```rust
// shared/tests/bip322_cross_shape.rs
// Source: D-34 verbatim

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

// ... 8 more #[test] fns per the D-34 enumeration, each asserting a specific
// Bip322Error variant. Self-documenting names; no proptest shrinkage to interpret.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `bdk` crate (deprecated) | `bdk_wallet` crate | rename ~2024 | Already adopted in v1.0; not new in Phase 15. |
| `bitcoincore-rpc` crate (archived Nov 2025) | `corepc-types` + manual reqwest | v1.0 | Already adopted; not new in Phase 15. |
| Custom `shared/src/bip322.rs` (P2WPKH only, 133 LOC) | `bip322 = "=0.0.10"` crate wrap + module split (`bip322/{mod,p2wpkh,p2tr,p2sh_p2wpkh}.rs`) | Phase 15 (ADR Decision #1, 2026-05-29) | Verify surface drops to ~26 LOC adapter; sign surface stays in `shared/` per V1.4-MOD-07 with bdk-backed production bodies in Phase 17. |
| `OwnershipProof { witness_stack: Vec<Vec<u8>> }` (v1.3, 2-field struct) | Flat struct with `version: u8` + `witness_stack: Vec<Vec<u8>>` + `psbt_input_b64: Option<String>` + `script_type: Option<ScriptType>` | Phase 15 (ADR Decision #3, 2026-05-29) | Wire-compatible with v1.3 via `#[serde(default)]`. v1.4 clients construct v2; v1.4 coordinator accepts both. |
| `is_p2wpkh()` hard gate at `coordinator/src/bitcoin/utxo.rs:119` | (deferred — Phase 16 replaces with `detect_script_type` + allowlist dispatcher) | Phase 16 (NOT Phase 15) | Phase 15 leaves the gate in place; coordinator still single-script. |

**Deprecated/outdated in v1.4:**
- The flat `shared/src/bip322.rs` file (replaced by `shared/src/bip322/` directory).
- The local `Bip322Error` enum at `coordinator/src/bitcoin/utxo.rs:87-101` (replaced by `shared::bip322::Bip322Error` import).
- The single-fn signature `verify_bip322_simple(spk, witness, message)` (NOT removed in Phase 15; Phase 16 swaps the call site).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The bip322 crate's `verify_simple` signature `(&Address, message: impl AsRef<[u8]>, signature: Witness) -> Result<()>` is stable across `=0.0.10` patches | Pattern 2, Architecture Diagram | [VERIFIED at `~/.cargo/registry/.../bip322-0.0.10/src/verify.rs:46-50`] — exact-pin (`=0.0.10`) means there are no patches to drift. Risk transfers to the next pin bump (v1.5 candidate). |
| A2 | `bitcoin::Witness::clone()` is byte-exact (no item dropping) | Pattern 2, Pitfall 2 | [VERIFIED in sprint-0-A.md:178 + Rust's `derive(Clone)` semantics — `Witness` is `Vec<u8>` shaped; clone is mem-copy.] No risk. |
| A3 | The 9-rejection matrix's `reject_p2*_spk_with_p2*_witness` tests can all be expressed as `Bip322Error::InvalidWitnessLength` OR `Bip322Error::CrateVerifyFailed` rejections | Common Pitfalls, Example 6 | If the crate's `verify_simple` accepts a mismatched (spk, witness) shape silently in any cell of the matrix, that's V1.4-CRIT-01 unmitigated. **Mitigation:** Phase 15's 15-03-PLAN must actually run all 9 tests on the green path before declaring done. |
| A4 | Supplementing the upstream BIP-322 vectors with the bip322 crate's `lib.rs:46-48` `NESTED_SEGWIT_ADDRESS` constants gives sufficient P2SH-P2WPKH property-test coverage | Pitfall 6, Architecture Diagram | The supplement has 2 sign-verify test cases (`simple_sign_p2sh_p2wpkh` + `roundtrip_p2sh_p2wpkh_simple` at `lib.rs:299-321`). Light coverage but adequate for v1.4 minimum. v1.5 TEST-EXT-01 (cross-impl differential fixtures via `bip322-js`) is the gap-closer. [ASSUMED — planner should confirm with user that 2 vectors meet "per-script property test" gate intent.] |
| A5 | `bitcoin::Address::from_script(spk, network)` correctly identifies P2SH-P2WPKH (the SPK is just `OP_HASH160 <20-byte-hash> OP_EQUAL` — same byte shape as any other P2SH) | Pitfall 5, Pattern 3 | The bip322 crate uses this exact call internally at `verify.rs:67` and routes via `AddressData::P2sh { script_hash: _ }` arm at `verify.rs:87`. [VERIFIED via reading crate source.] No risk. |
| A6 | The current `shared/Cargo.toml`'s lack of `thiserror` as a direct dep is the correct baseline; Phase 15 ADDS it via workspace re-export, not as a new transitive | Standard Stack | [VERIFIED at `shared/Cargo.toml:6-12` — no `thiserror` line currently; workspace pins it at root `Cargo.toml:18`.] No risk. |
| A7 | Workspace `bdk_wallet = "2.3"` resolves to `bdk_wallet v2.3.x` deterministically given the workspace lockfile (NOT an exact pin syntactically) | Standard Stack, Don't Hand-Roll | The workspace declares `bdk_wallet = { version = "2.3", features = ... }`. This is a caret-prefixed pin (`^2.3.0`) per Cargo semantics, NOT `=2.3.x`. The lockfile pins the exact resolved version per build. The CI grep gate per Phase 14 carry-forward constraint #3 should assert the exact-version pattern; that may require tightening the workspace declaration to `bdk_wallet = "=2.3.x"`. [ASSUMED — needs planner clarification with user about whether the grep gate enforces lockfile state or Cargo.toml string. Sprint-0-A's "exact-pin" assertion at v1.3 REPAIR-02 is about the lockfile, not the Cargo.toml string.] |
| A8 | The base64 engine for `psbt_input_b64` encode/decode can be sourced from `bitcoin::base64` re-export or from the workspace dep declaration in `client/Cargo.toml` without adding a new direct dep to `shared/Cargo.toml` | Transitive deps | The `bitcoin v0.32.8` dep brings in `base64 v0.21.7`; the bip322 crate's transitive `base64 v0.22.1` coexists in the lockfile. Either is callable; the choice is ergonomic. If neither is accessible without adding a direct dep, Phase 15 adds `base64 = "0.22"` to `shared/Cargo.toml` (a 4th new dep). [ASSUMED — needs verification at plan time via `cargo doc` on `bitcoin::base64`.] |
| A9 | `Bip322Error: Send + Sync + 'static` comes for free from `thiserror::Error` derive | Constraint discussion in CONTEXT deferred | `thiserror`'s blanket `std::error::Error` impl requires `Display + Debug` only. `Send + Sync` come from the variant types: all enum payloads here are `Send + Sync + 'static` (`u8`, `String`, `bitcoin::address::FromScriptError`, `bip322::error::Error`, `bitcoin::Network`, `ScriptType`, `usize` — all bound-clean). [VERIFIED by enum-payload audit; ASSUMED until the crate compiles with the derived enum.] |
| A10 | The bip322 crate's `verify_simple` correctly enforces witness-shape invariants per script type, so our additional `InvalidWitnessLength` checks at the per-script file boundary are redundant for the positive path but useful for the 9-rejection matrix's error-precision requirement | Examples 1-3, D-34 | The crate's `verify_full_p2wpkh` rejects `witness.len() != 2` at `verify.rs:127-129`; `verify_full_p2tr` rejects len-not-1 implicitly via the signature-length branch at `verify.rs:214-231`. Our pre-flight arity check converts "crate rejection with CrateVerifyFailed" into "our rejection with InvalidWitnessLength" — preserves the precision the matrix needs. [VERIFIED via crate source read.] |
| A11 | Vendoring the BIP-322 `basic-test-vectors.json` from `bitcoin/bips@master` is acceptable supply-chain practice as long as the commit SHA is recorded in a header file | D-33 | [VERIFIED: D-33 explicitly mandates this pattern; v1.3 REPAIR-02 corepc-node feature pin is the analogous template.] No risk. |
| A12 | The CI grep gate for `bip322 = "=0.0.10"` can be added by copy-paste-modifying the existing `corepc-node-feature-pin-check` job at `.github/workflows/ci.yml:183-213` | Don't Hand-Roll, Common Pitfalls | The template is well-documented and pattern-uniform. Phase 15's planner can mechanically extend it. [VERIFIED via reading `.github/workflows/ci.yml:183-213`.] No risk. |

**If this table is empty:** N/A — 12 assumptions are listed, most VERIFIED via source-of-truth reads. A4 (P2SH-P2WPKH vector supplementation) and A7 (lockfile-vs-Cargo.toml pin semantics) and A8 (base64 engine sourcing) need user/planner confirmation. A3 carries an empirical risk that the 15-03-PLAN must close on the green path.

## Open Questions (RESOLVED)

All three open questions resolved at plan-phase boundary 2026-05-30; resolutions applied verbatim across `15-01-PLAN.md`, `15-02-PLAN.md`, and `15-03-PLAN.md`.

1. **Are the bip322 crate's 2 P2SH-P2WPKH test constants at `lib.rs:46-48` adequate for the BIP322-04 "per-script property test" gate, or do we need to GENERATE additional P2SH-P2WPKH vectors via the test-only signer?**
   - What we know: Upstream `basic-test-vectors.json` has 0 P2SH-P2WPKH cases. The crate's `lib.rs` has 2 cases (one sign, one roundtrip). REQUIREMENTS BIP322-04 calls for "per-script property tests against the official BIP-322 basic-test-vectors.json (commit-SHA pinned from bitcoin/bips)".
   - What's unclear: Whether REQUIREMENTS' "official" language strictly bars supplementing with crate-internal vectors, or whether "supplemented for v1.4 minimum, TEST-EXT-01 cross-impl differential closes the gap in v1.5" is acceptable per the upstream policy.
   - **RESOLVED:** Treat the crate's vectors as a fixture supplement (vendored as a separate file with a clear "supplement for missing upstream coverage" README), AND have the test-only signer in `#[cfg(test)] sign_for_tests` generate additional roundtrip vectors at test runtime. The combination gives strong P2SH-P2WPKH coverage without claiming "official vector" status for crate-internal data. Applied in `15-03-PLAN.md` Task 1 (`shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json`).

2. **Should the CI grep gate for the new `bip322 = "=0.0.10"` pin be a NEW workflow job, or an extension of the existing `corepc-node-feature-pin-check`?**
   - What we know: The corepc-node job at `.github/workflows/ci.yml:183-213` is well-documented and copy-pasteable. The pattern (`grep -rEn '<dep>\s*=' --include='Cargo.toml' . | grep -v '<expected pattern>' | grep -v '^[^:]*:[0-9]*:#'`) generalises to any dependency.
   - What's unclear: Whether reviewers prefer one job per pin (clearer log output, distinct failure messages) or a single job per pin-type-class (less workflow YAML to maintain).
   - **RESOLVED:** Add a NEW job `bip322-pin-check` per the existing job's template, with the grep pattern asserting `bip322\s*=\s*"=0\.0\.10"` (exact-equals pin). Easier to read in PR check status; matches the v1.3 REPAIR-02 precedent's "one job per invariant" shape. Applied in `15-02-PLAN.md` Task 3.

3. **Should `OwnershipProof.script_type: Option<ScriptType>` be serialised as `"p2wpkh"` / `"p2tr"` / `"p2sh-p2wpkh"` (kebab-style, matching REQUIREMENTS' ADVERT-02 wire format) or as `"P2wpkh"` / `"P2tr"` / `"P2shP2wpkh"` (default `serde(rename_all)` ascii)?**
   - What we know: ADVERT-02 already chose `["p2wpkh", "p2tr", "p2sh-p2wpkh"]` for the PKARR / `/round/info` wire form (Phase 16 work). v1.4 wants symmetric forms across the protocol where possible.
   - What's unclear: Whether `OwnershipProof.script_type`'s wire encoding should match ADVERT-02 verbatim (kebab-case for p2sh-p2wpkh) or use a different style (snake-case `p2sh_p2wpkh`).
   - **RESOLVED:** Use `#[serde(rename_all = "snake_case")]` with an explicit `#[serde(rename = "p2sh-p2wpkh")]` on the `P2shP2wpkh` variant — matches ADVERT-02 verbatim. Locks Phase 16's job to "just deserialize whatever Phase 15 set up", removing ambiguity at the wire. Applied in `15-01-PLAN.md` Task 1 + `15-02-PLAN.md` Task 1 (ScriptType enum derive).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (Rust toolchain) | Build + test | ✓ | 1.95.0 (verified) | — |
| `rustc` | Build | ✓ | 1.95.0 | — |
| `bitcoin v0.32.8` (transitive via workspace pin) | Build | ✓ | 0.32.8 (per Sprint-0-A cargo tree) | — |
| `bip322 v0.0.10` crate | Build (after Phase 15 dep addition) | ✓ | 0.0.10 (local cache + verified registry) | — |
| `thiserror v1.x` (workspace) | Build (after Phase 15 dep declaration) | ✓ | 1.x via workspace | — |
| `proptest v1.x` (workspace, dev) | Test runner | ✓ | 1.x via workspace | — |
| `cargo audit` | CI gate | ✓ (per Sprint-0-A) | 0.22.1 | — |
| `cargo tree` | Verification step | ✓ | builtin | — |
| `bitcoind v30.2` | NOT NEEDED for Phase 15 (`shared/` is a pure crate; no integration tests at this phase) | n/a | n/a | n/a |
| Network access to crates.io | First-time build only (Cargo cache fills) | ✓ | n/a | Vendored deps via `cargo vendor` if needed (not currently configured) |
| Network access to `github.com/bitcoin/bips` | One-time vendor of `basic-test-vectors.json` at Phase 15 commit time | ✓ | n/a | If GitHub is down at vendor time, retry later — fixture is a one-time commit. |

**Missing dependencies with no fallback:** none — Phase 15 has zero external runtime dependencies.

**Missing dependencies with fallback:** none.

## Security Domain

Per `.planning/config.json` and the project security posture, Phase 15 is in the cryptographic-verification path; security domain coverage is mandatory.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes (BIP-322 proof IS authentication of UTXO control) | `bip322 = "=0.0.10"` crate verify — DO NOT hand-roll signature verification |
| V3 Session Management | no | n/a (sessions are HMAC tokens via shared::token, untouched by Phase 15) |
| V4 Access Control | no | n/a (coordinator allowlist is Phase 16) |
| V5 Input Validation | yes | `OwnershipProof::from_json_hex_str` two-phase try-parse rejects malformed input; `Bip322Error::DecodeError` + `WireFormatMismatch` variants surface typed validation failures |
| V6 Cryptography | yes (Schnorr + ECDSA verification, BIP-143 + BIP-341 sighash) | `bip322 = "=0.0.10"` crate handles all crypto; we wrap. PROJECT.md "no custom crypto" enforced. |
| V11 Business Logic | yes (script-type-spoofing prevention is a business invariant) | D-27 dispatcher-only public API prevents per-script verifier bypass at the type level. V1.4-CRIT-01 mitigation. |
| V14 Configuration | partial | Phase 15 adds 2 dep declarations + CI grep gate per v1.3 REPAIR-02 carry-forward pattern. No config-file changes in Phase 15. |

### Known Threat Patterns for `shared/` BIP-322 verifier

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Script-type spoofing (V1.4-CRIT-01) | Spoofing, Elevation of Privilege | `detect_script_type(spk)` returns the on-chain truth; the dispatcher-only `pub` surface forbids bypass; Phase 16 cross-check at validate-utxo time per D-10. Phase 15 makes the spoof STATICALLY UNREACHABLE in `shared/` (per-script verifiers are `pub(crate)`). |
| Silent sighash failure across script types (V1.4-CRIT-02) | Tampering | Three SEPARATE per-script files (`p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`) per D-04. Per-script property tests against vendored `basic-test-vectors.json` per D-33. 9-rejection cross-shape matrix per D-34. The bip322 crate's correctness is the secondary defence; our property tests are the primary. |
| Wire-format ambiguity (V1.4-MOD-01, REPAIR-01 lesson #1) | Tampering | Wire-format roundtrip test ships FIRST as `15-01-PLAN.md` atomic commit per CD-10. Versioned `OwnershipProof` envelope per D-22..D-25. `#[serde(default)]` on `version` for v1→v2 coexistence. |
| Pre-1.0 dep API churn (V1.4-CRIT-03) | Repudiation (supply-chain) | Exact-pin `bip322 = "=0.0.10"`. CI grep gate per Phase 14 carry-forward constraint #3. Re-evaluation trigger if crate ships 1.0 in v1.5. |
| Pre-1.0 dep supply-chain (bip322, snafu, snafu-derive) | Repudiation | Sprint-0-A `cargo audit` returned 0 advisories + 0 warnings. Slopcheck [OK] on all 3 Phase-15-added direct deps. Three new transitive crates accepted; CI re-runs `cargo audit` on every PR per v1.1 hardening. |
| Per-script-type error code fingerprinting | Information Disclosure | D-32 single-bucket `ErrorCode::InvalidOwnershipProof` mapping. Internal typed enum preserved for server-side logging only. |
| Memory exhaustion via large `psbt_input_b64` | Denial of Service | The `bitcoin::psbt::Input` decoder has internal bounds via `consensus_decode_from_finite_reader`. Phase 15's roundtrip test case #5 (corrupted base64 / truncated PSBT) asserts non-panicking rejection. Tower-http per-route body-size limits at the coordinator layer (Phase 8) bound the input. |

**Out of Phase 15 scope (deferred to other phases or future milestones):**
- TLS / authentication of the coordinator API surface (covered by Tor hidden service per PROJECT.md constraint).
- Per-script-type ban list (anti-feature per REQUIREMENTS.md Out-of-Scope).
- Per-script-type rate limits (anti-feature per REQUIREMENTS.md Out-of-Scope).

## Sources

### Primary (HIGH confidence)

- **bip322 crate source (local cache):** `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bip322-0.0.10/` — verified `verify.rs:46-58` (verify_simple signature), `verify.rs:62-99` (script-type routing), `verify.rs:101-185` (P2WPKH/P2SH-P2WPKH branch), `verify.rs:187-258` (P2TR branch with 64/65-byte handling), `error.rs:1-69` (19-variant Error enum), `lib.rs:42-48` (test constants including `NESTED_SEGWIT_ADDRESS`), `sign.rs:106-217` (sign helpers), `util.rs:1-83` (create_to_spend/create_to_sign + `BIP322_TAG`), `Cargo.toml` (default-features = empty, bitcoin = "0.32.5" pin, snafu = "0.8.5" with rust_1_61+std features).
- **.planning/decisions/v1.4-adr.md** — Decisions #1, #3, #4 ratified 2026-05-29.
- **.planning/research/sprint-0-A.md** — Cargo tree verbatim, cargo audit verbatim, 26-LOC adapter sketch at lines 145-175.
- **.planning/research/sprint-0-B.md** — bdk_wallet 2.3 P2TR sign PoC, recovered witness hex, verify_schnorr Ok verdict, finalisation-path note at lines 317-319.
- **.planning/research/SUMMARY.md** — V1.4-MOD-07 single-source-of-truth invariant; V1.4-CRIT-01/02 mitigations.
- **.planning/research/PITFALLS.md** — V1.4-CRIT-01 (script-type spoofing), V1.4-CRIT-02 (silent sighash failures), V1.4-MOD-01 (OwnershipProof wire-format evolution), V1.4-MOD-07 (BIP-322 vs legacy `signmessage`).
- **.planning/REQUIREMENTS.md** — BIP322-01..04, ADVERT-04, Out-of-Scope table.
- **.planning/ROADMAP.md** — Phase 15 success criteria #1-#5, cross-phase invariant.
- **.planning/STATE.md** — Phase 14 close, 4 ADR decisions ratified.
- **shared/src/bip322.rs** — Existing 133-LOC implementation (file in tree, read in full).
- **shared/src/protocol.rs** — Existing `OwnershipProof` struct (lines 105-139), `InfoResponse`, `InputRegRequest`.
- **shared/src/errors.rs** — `ErrorCode::InvalidOwnershipProof` already exists at line 16 — no new variants needed per D-32.
- **shared/Cargo.toml** — Current direct deps; no `thiserror`, no `proptest`.
- **coordinator/src/bitcoin/utxo.rs:1-220** — `verify_bip322_simple` + local `Bip322Error` at lines 87-101 to be deleted.
- **coordinator/src/api/handlers.rs:130-180** — `OwnershipProof::from_json_hex_str` call site.
- **client/src/round/input.rs:1-120** — `generate_bip322_witness` + `OwnershipProof { witness_stack }` construction.
- **.github/workflows/ci.yml:183-213** — corepc-node CI grep gate template (Phase 15 extends).
- **Cargo.toml (workspace root):** `thiserror = "1"` at line 18, `proptest = "1"` at line 28, `bitcoin = { version = "0.32" }` at line 11.

### Secondary (MEDIUM confidence)

- **`https://raw.githubusercontent.com/bitcoin/bips/master/bip-0322/basic-test-vectors.json`** (fetched via WebFetch 2026-05-29) — Top-level object with `tx_hashes`, `simple`, `error` keys; covers P2WPKH and P2TR; **NO P2SH-P2WPKH vectors** confirmed (Pitfall 6 source).
- **crates.io search results:** `bip322 = "0.0.10"`, `thiserror = "2.0.18"` (workspace pins v1.x), `proptest = "1.11.0"`.
- **slopcheck audit:** all three Phase-15-added packages return [OK].

### Tertiary (LOW confidence — flagged for plan-time verification)

- **`bitcoin::base64::Engine` re-export availability** — claim that the `bitcoin v0.32.x` crate publicly re-exports a usable base64 engine is unverified at research time. Planner should run `cargo doc --open -p bitcoin && search for 'base64'` at plan time. Fallback: add `base64 = "0.22"` to `shared/Cargo.toml` as a 4th direct dep.
- **Workspace `bdk_wallet = "2.3"` interpreted as exact-pin** — the v1.3 REPAIR-02 CI grep gate likely enforces lockfile state, not Cargo.toml string. Planner should examine the existing gate's grep pattern at `.github/workflows/ci.yml:202-213` and decide whether Phase 15 extends with a parallel pattern for `bip322 = "=0.0.10"`. The CONTEXT's claim of "exact-pin every new dep" treats the Cargo.toml string as the gate target; this likely needs to be tightened.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — crate source verified locally; workspace deps inspected directly; slopcheck clean.
- Architecture: HIGH — all module locations specified in CONTEXT/ADR; bip322 crate's internal routing confirmed by reading `verify.rs`.
- Pitfalls: HIGH on cataloging (7 documented with concrete warning signs), MEDIUM on Pitfall 4's `thiserror` chain Display behaviour (acceptable per D-32 even if Display drops the chain; preserved via `.source()` for logging).
- Wire format: HIGH — D-22..D-25 verbatim; existing `shared/src/protocol.rs` serde patterns confirmed.
- Test strategy: MEDIUM-HIGH — upstream `basic-test-vectors.json` schema confirmed via WebFetch; supplement strategy for P2SH-P2WPKH is the only soft spot (A4 needs user confirmation).
- Security: HIGH — V1.4-CRIT-01 statically unreachable via D-27 dispatcher-only API; V1.4-CRIT-02 mitigated by per-script file separation; V1.4-CRIT-03 mitigated by exact-pin + CI grep gate.

**Research date:** 2026-05-29
**Valid until:** 2026-06-29 — bip322 crate's pre-1.0 status means any upstream change to `=0.0.10` invalidates the adapter sketch. The exact-pin protects us until we deliberately bump. The vendored `basic-test-vectors.json` is similarly time-frozen until we bump the SHA.

*Phase: 15 — Shared Crate Multi-Script Contract*
*Research synthesised from CONTEXT, ADR, Sprint-0-A/B, crate source, and codebase reads.*
