# Phase 15: Shared Crate Multi-Script Contract - Context

**Gathered:** 2026-05-30
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 15 turns `shared/` into the single source of truth for **BIP-322 multi-script verification + the v1.4 wire types**, so coordinator and client compile against one contract and produce byte-identical to_spend / to_sign per script type. Concretely:

1. **`shared/src/bip322/`** replaces the flat `shared/src/bip322.rs` per the D-04 module split:
   ```
   shared/src/bip322/
     mod.rs           # ScriptType enum, public dispatcher API, crate-verify adapter, Bip322Error
     p2wpkh.rs        # P2WPKH BIP-143 ECDSA inner mechanics (carried over)
     p2tr.rs          # P2TR BIP-341 Schnorr keypath inner mechanics (new; accepts 64-byte SIGHASH_DEFAULT + 65-byte SIGHASH_ALL)
     p2sh_p2wpkh.rs   # P2SH-P2WPKH BIP-143 over unwrapped redeem script + HASH160 cross-check (new)
   ```
   The script-type-NEUTRAL primitives (`bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`) stay as **V1.4-MOD-07 single source of truth** — wrapped, never replaced (per ADR Decision #1 Consequences/Neutral).

2. **`shared::bip322` public API** (the contract both coordinator and client compile against):
   - `pub enum ScriptType { P2WPKH, P2TR, P2SH_P2WPKH }` (BIP322-01)
   - `pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error>` — no fallthrough default arm; unknown patterns error (BIP322-01)
   - `pub fn verify_simple(script_type, spk, witness, message, network) -> Result<(), Bip322Error>` — dispatcher; internally calls the wrapped `bip322 = "=0.0.10"` crate (BIP322-02)
   - `pub fn sign_simple(script_type, spk, key, message) -> Result<Witness, Bip322Error>` — symmetric dispatcher (BIP322-03) — note: actual P2TR signing in Phase 17 uses bdk path per ADR Decision #4; this surface exists in shared/ as the contract Phase 17 implements against
   - `pub enum Bip322Error` (thiserror-derived) — ~10 variants; see Decisions

3. **`shared::protocol::OwnershipProof` v2 envelope** (ADVERT-04, D-12 verbatim):
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct OwnershipProof {
       #[serde(default = "default_proof_version")] pub version: u8,        // default 1 (v1.3 compat)
       #[serde(default)] pub witness_stack: Vec<Vec<u8>>,                 // v1 path (witness-only)
       #[serde(skip_serializing_if = "Option::is_none")] pub psbt_input_b64: Option<String>,  // v2 path
       #[serde(skip_serializing_if = "Option::is_none")] pub script_type: Option<ScriptType>, // v2 declared type
   }
   ```
   Coordinator branches on `match proof.version { 1 => v1_witness_path(...), 2 => v2_psbt_path(...), _ => Err(UnsupportedProofVersion) }`. Single flat struct; no tagged enum (ADR Decision #3 explicitly rejected B1).

4. **Wire-format roundtrip test ships FIRST in shared/** (D-13, REPAIR-01 lesson #1, non-negotiable phase boundary). 5 D-13 cases + 9 cross-shape rejections + per-script property tests against the BIP-322 `basic-test-vectors.json` (commit-SHA pinned snapshot, `include_str!`'d at compile time).

**Net effect:** Phase 16 (coordinator) and Phase 17 (client) link the same `shared::bip322` dispatcher API + `shared::protocol::OwnershipProof` type, with zero ambiguity about the wire shape or who owns the verify dispatch. The 9-combination rejection matrix is statically provable inside `shared/` — coordinator never gets a chance to silently accept a spoofed script_type.

**Requirements mapped to this phase (per `.planning/REQUIREMENTS.md` traceability):** BIP322-01, BIP322-02, BIP322-03, BIP322-04, ADVERT-04.

**Not in scope:**
- Coordinator allowlist config (`[bip] allow_p2wpkh/allow_p2tr/allow_p2sh_p2wpkh`) — Phase 16 (ADVERT-01).
- Coordinator PKARR + `/round/info` advertisement of `supported_script_types` — Phase 16 (ADVERT-02).
- Coordinator CRIT-01 cross-check at validate-utxo time — Phase 16 (ADVERT-03 / D-10). Phase 15 ships the `detect_script_type` primitive Phase 16 calls; the wiring itself is Phase 16's.
- Removal of the `is_p2wpkh()` hard gate at `coordinator/src/bitcoin/utxo.rs:119` — Phase 16. Phase 15 leaves coordinator's old `verify_bip322_simple` in place; Phase 16 replaces the call site with the new dispatcher.
- Client wallet descriptor selection (BIP-84 / BIP-86 / BIP-49) — Phase 17 (WALLET-01).
- Client BIP-322 signing via bdk_wallet — Phase 17 (WALLET-02). Phase 15 exposes the `sign_simple` API surface; Phase 17 calls bdk inside it per ADR Decision #4.
- Client discovery-time fail-fast (`WALLET-03`) and v1.4→v1.3 compat shim (`WALLET-04`) — Phase 17.
- Mixed-script E2E integration test + liquidity-bot multi-script — Phase 18 (INTEG-01, INTEG-02).

**Cross-phase invariant (carries to every v1.4 phase boundary):** v1.3 P2WPKH-only `tests/integration/full_round.rs` tests MUST remain green at this phase boundary. The Phase 15 changes to `shared/src/bip322/` are additive on the v1 path — existing P2WPKH witness-only verification is preserved bit-exact by routing v1 (`version = 1`) through the same `bip322 = "=0.0.10"` crate adapter with a `witness_stack`→`Witness` conversion. If the v1.3 test goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phase 14 ADR (NOT re-asked)

These are LOCKED upstream. Plan-phase consumes them verbatim — no re-litigation. Anchors are to `.planning/decisions/v1.4-adr.md`.

- **ADR #1 (#decision-1):** ADOPT `bip322 = "=0.0.10"`. 26-LOC adapter from `.planning/research/sprint-0-A.md` lines 145-175 is the implementation template. Three new transitive crates accepted: `bip322 v0.0.10`, `snafu v0.8.9`, `snafu-derive v0.8.9`. Script-type-NEUTRAL primitives stay in shared/ as V1.4-MOD-07 single source of truth.
- **ADR #3 (#decision-3):** B2 PSBT-input wire shape; explicit `version: u8` envelope; `version = 1` = v1.3 witness-only path, `version = 2` = v1.4 PSBT path. 5 D-13 roundtrip test cases ship FIRST per REPAIR-01 lesson #1.
- **ADR #4 (#decision-4):** bdk path for P2TR sign — implementation in Phase 17 WALLET-02; Phase 15 only ships the `sign_simple` API surface. D-15 manual `secp256k1::sign_schnorr` fallback retired for v1.4; D-15/D-16 stay on the books as a v1.5 swap target if bdk regresses.
- **REQUIREMENTS BIP322-02:** P2TR accepts BOTH SIGHASH_DEFAULT 64-byte AND SIGHASH_ALL 65-byte sig forms (the crate adapter handles this; we test both).
- **REQUIREMENTS BIP322-02:** P2SH-P2WPKH dispatch performs BIP-143 sighash over the unwrapped P2WPKH redeem script WITH a `HASH160(redeemScript) == script_pubkey.p2sh_hash` cross-check.
- **Phase 14 D-04:** Module split = `mod.rs / p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`. Each per-type file owns its sighash + signature primitive + arity check in isolation; Phase 15 spec-vector failures localise to one file.
- **Phase 14 carry-forward constraint #3:** Exact-pin every new dependency. `bip322 = "=0.0.10"` and `thiserror` (any v1.x); both enforced via CI grep gate alongside the existing `bdk_wallet = "=2.3.x"` and `corepc-node` feature pins (v1.3 REPAIR-02 pattern).

### OwnershipProof v2 wire shape

- **D-22 (v1↔v2 coexistence):** Single flat `OwnershipProof` struct with `#[serde(default)]` on `version` (default = 1), `witness_stack` (default = empty), `psbt_input_b64` (Option), `script_type` (Option). Coordinator branches with `match proof.version`. NO tagged serde enum (ADR Decision #3 rejected B1). NO two-struct dispatcher (doubles the type surface). Matches D-12 verbatim.
- **D-23 (envelope transport):** `InputRegRequest.ownership_proof` stays `String` containing JSON-serialized `OwnershipProof`. Coordinator parses with `serde_json::from_str`. Zero wire-format change for the outer `InputRegRequest`; preserves T-01-05 "never pass raw bytes" pattern. The existing v1.3 `from_json_hex_str` / `to_json_hex_str` helpers become thin convenience wrappers around `serde_json` (v1 backwards-compat: when `version = 1` and `psbt_input_b64.is_none()`, the encoder emits the v1.3 array-of-hex form; the decoder accepts both legacy array-of-hex AND new flat-struct JSON).
- **D-24 (script_type placement):** `script_type: Option<ScriptType>` is a sibling envelope field, NOT inferred from PSBT contents. Coordinator (Phase 16) reads it explicitly, then cross-checks against `detect_script_type(on_chain_spk)` per D-10 / CRIT-01. D-13 test case #3 (mismatched declared vs PSBT contents) becomes a literal comparison: `proof.script_type != detect_script_type_from_psbt(psbt_input)`. Decoupled from PSBT internals; defensible at the wire surface.
- **D-25 (version default):** `#[serde(default = "default_proof_version")]` returning `1`. v1.3 clients that omit the field deserialize as `version = 1` — wire-compatible until WALLET-04 in Phase 17 makes the compatibility shim explicit at the client side. Field absence is NOT an error; only an unknown version (e.g., `3+`) errors with `UnsupportedProofVersion`.

### Module layout + dispatcher style

- **D-26 (adapter location):** The 26-LOC `bip322 = "=0.0.10"` crate adapter (sketched in `sprint-0-A.md:145-175` as `shared/src/bip322/adapter.rs`) lives as a **crate-private fn in `shared/src/bip322/mod.rs`** alongside the `verify_simple` dispatcher. Sprint-0-A's `adapter.rs` path was an analytical sketch, not a binding location; the dispatcher and adapter are the same conceptual layer ("crate-backed verify, per-script entry"). Matches 14-CONTEXT.md D-04's "mod.rs = public API + dispatcher".
- **D-27 (public API surface):** Public from `shared::bip322` is dispatcher-only:
  - `pub enum ScriptType`
  - `pub enum Bip322Error`
  - `pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error>`
  - `pub fn verify_simple(script_type: ScriptType, spk: &Script, witness: &Witness, message: &[u8], network: Network) -> Result<(), Bip322Error>`
  - `pub fn sign_simple(script_type: ScriptType, spk: &Script, key: &SecretKey, message: &[u8]) -> Result<Witness, Bip322Error>` *(Phase 17 fills in the bdk-backed body per ADR Decision #4; Phase 15 ships the signature + a `todo!()`-or-stub-with-test marker so the contract is locked)*
  - `pub fn` script-NEUTRAL helpers (`bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`) — re-exported from the module root.
  - Per-script files (`p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`) are `pub(crate)` inner mechanics only. **No type-specific `pub` fns at the boundary** — coordinator and client cannot bypass dispatch, eliminating the V1.4-CRIT-01 spoofing vector at the API layer (a caller can't accidentally call `verify_p2wpkh` for a P2TR UTXO because no such pub fn exists).
- **D-28 (OwnershipProof home):** `OwnershipProof` stays in `shared/src/protocol.rs` alongside `InputRegRequest` / `InputRegResponse`. `shared::bip322` owns verifiers + `ScriptType` only. `ScriptType` is `pub use shared::bip322::ScriptType` re-exported from `protocol.rs` so the wire struct's `script_type: Option<ScriptType>` derive works without a module cycle (`protocol` imports from `bip322`; the reverse never happens).

### Error taxonomy + lib choice

- **D-29 (Bip322Error home):** Single unified `Bip322Error` enum in `shared/src/bip322/mod.rs`, exported as `pub`. The existing coordinator-local `Bip322Error` at `coordinator/src/bitcoin/utxo.rs:99` is **deleted in Phase 15**. Coordinator imports `shared::bip322::Bip322Error`; ApiError mapping happens at the handler layer. Matches "shared/ is the contract" invariant.
- **D-30 (error lib):** `thiserror` (any v1.x; exact-pinned in `shared/Cargo.toml` to current latest). Sprint-0-A's 26-LOC adapter sketch already uses thiserror syntax verbatim. snafu enters the dep graph transitively via the bip322 crate but is NOT used directly — we wrap `bip322::error::Error` via `#[source]` (per Sprint-0-A's adapter sketch) and present a thiserror-derived enum to the rest of `shared/`. Reasoning: thiserror is the ecosystem default for libraries with serde-shaped errors, has zero runtime cost, and matches existing patterns in `shared/`.
- **D-31 (variant taxonomy, ~10 variants):**
  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum Bip322Error {
      #[error("unsupported OwnershipProof version: {0}")]
      UnsupportedProofVersion(u8),
      #[error("wire-format mismatch: {0}")]
      WireFormatMismatch(String),
      #[error("PSBT/base64 decode error: {0}")]
      DecodeError(String),
      #[error("script_pubkey is not a recognised single-key address (P2WPKH / P2TR / P2SH-P2WPKH)")]
      UnrecognisedScriptPubkey { #[source] source: bitcoin::address::FromScriptError },
      #[error("unsupported script type")]
      UnsupportedScriptType,
      #[error("declared script_type {declared:?} does not match on-chain {derived:?}")]
      ScriptTypeMismatch { declared: ScriptType, derived: ScriptType },
      #[error("invalid witness length: expected {expected}, got {got}")]
      InvalidWitnessLength { expected: usize, got: usize },
      #[error("BIP-322 crate verification failed")]
      CrateVerifyFailed { #[source] source: bip322::error::Error },
      #[error("network mismatch: address decoded for {decoded:?}, configured for {configured:?}")]
      NetworkMismatch { decoded: bitcoin::Network, configured: bitcoin::Network },
      #[error("legacy v1 script mismatch")]
      ScriptMismatch,  // preserved from v1.3 for v1 path parity
  }
  ```
  Each variant maps 1:1 to a specific D-13 test case (UnsupportedProofVersion → #4; WireFormatMismatch → #3; DecodeError → #5; UnrecognisedScriptPubkey, UnsupportedScriptType, ScriptTypeMismatch → 9-combination cross-shape rejections). Pattern-match-able server-side for blame/logging without leaking to the wire.
- **D-32 (wire mapping):** ALL `Bip322Error` variants map to `ApiError { code: ErrorCode::InvalidOwnershipProof, message: e.to_string() }` at the coordinator handler layer. NO new `ErrorCode` variants added in v1.4. Reasoning: distinct error codes per script-type-related rejection would let an external observer fingerprint which script type the coordinator rejected — a REQUIREMENTS.md anti-feature ("publicly advertised script-type breakdown of registrants per round" is Out-of-Scope and the same leak shape applies to per-script-type error codes). Internal pattern-match-ability is preserved for server-side logging + future blame routing.

### Spec-vector fixture + rejection harness

- **D-33 (fixture pinning):** **Vendored snapshot** at `shared/tests/fixtures/bip322/basic-test-vectors.json` with a header line/comment recording: `# source: bitcoin/bips@<commit-sha>; captured 2026-05-XX`. `include_str!("fixtures/bip322/basic-test-vectors.json")` at compile time. Zero network in CI, fully reproducible, reviewable in PR diff. Bumping the SHA is an explicit code change (the file changes; the comment changes; CI re-runs). Rejected: git submodule (extra clone+CI init cost, fetches MBs of unrelated BIPs content), build.rs fetch (CI network dependency contradicts v1.3 REPAIR-02 supply-chain hardening).
- **D-34 (cross-shape rejection harness):** **Nine enumerated `#[test]` functions**, one per off-diagonal (script_pubkey, witness_shape) combination:
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
  Diagonal entries (p2wpkh × p2wpkh, p2tr × p2tr, p2sh_p2wpkh × p2sh_p2wpkh) are the positive sign↔verify property tests against `basic-test-vectors.json`, NOT in this matrix. Each rejection test asserts a specific `Bip322Error` variant (`UnrecognisedScriptPubkey | UnsupportedScriptType | InvalidWitnessLength | CrateVerifyFailed`) so silent acceptance is impossible at this layer. Failures localise to one function name; no proptest shrink output to interpret. 1:1 map to V1.4-CRIT-01 spoofing scenarios.

### Claude's Discretion

- **CD-6:** Whether `sign_simple` ships in Phase 15 as a `todo!("Phase 17 wires bdk")` marker, a stub that panics with a clear "not implemented in shared; phase 17 implements via bdk in client::wallet" message, OR a fully-working manual `secp256k1` implementation that gets deleted in Phase 17. Default: **`todo!()` marker with a `#[cfg(test)]`-only sign for the property-test path** — keeps the contract type-checked, lets Phase 15's per-script sign↔verify round-trip tests run end-to-end using a test-only signer in `shared/`, and Phase 17 swaps the production sign body to call into `bdk_wallet` from the client side without changing the `shared::bip322::sign_simple` signature. Plan-phase can override if the round-trip tests would be cleaner with the manual impl living in shared/ permanently.
- **CD-7:** Whether the `version = 1` legacy decoder accepts BOTH the v1.3 array-of-hex JSON shape (`["3045...", "02ab..."]`) AND the new flat-struct JSON shape (with `witness_stack` field), or only one. Default: **both** — `OwnershipProof::from_json_hex_str` tries the array-of-hex form first (matches v1.3 exactly), falls back to `serde_json::from_str::<OwnershipProof>`. Preserves bit-exact v1.3 client compatibility at this phase boundary (cross-phase invariant) without forcing WALLET-04 to ship until Phase 17.
- **CD-8:** Whether the `Network` parameter on `verify_simple` is the actual `bitcoin::Network` enum or a thin newtype. Default: **`bitcoin::Network` enum directly** — Sprint-0-A's adapter takes it as an explicit argument, and the coordinator can read it from `coordinator.toml` (`[bitcoin] network = "signet"`) at validate-utxo time. Plan-phase can override if a newtype would simplify Phase 16's config plumbing.
- **CD-9:** Exact `shared/Cargo.toml` dep declaration order and feature flags for `bip322 = "=0.0.10"` (default features vs `--no-default-features`). Default: **default features** unless Sprint-0-A flagged a default feature that pulls in undesired transitives. Plan-phase verifies via `cargo tree --no-default-features` whether trimming saves any deps.
- **CD-10:** Whether the wire-format roundtrip test ships as its own dedicated plan (e.g., `15-01-PLAN.md` = wire-format tests FIRST; `15-02-PLAN.md` = bip322 module split + dispatcher; `15-03-PLAN.md` = per-script tests + 9-rejection matrix) OR is integrated into the broader bip322 module split. Default: **dedicated plan first** per D-13 / REPAIR-01 lesson #1 — the wire-format test is non-negotiable and lands on its own atomic commit so `git bisect` can identify it cleanly if Phase 16/17 ever re-trigger a wire-format regression.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing Phase 15.**

### Phase 14 outputs (LOCKED inputs — read by ADR anchor)

- `.planning/decisions/v1.4-adr.md` §`#decision-1` — ADOPT `bip322 = "=0.0.10"` rationale + 26-LOC adapter contract; Phase 15 implements per this anchor.
- `.planning/decisions/v1.4-adr.md` §`#decision-3` — B2 PSBT-input shape + `version: u8` envelope + 5 D-13 roundtrip test cases. Phase 15's wire-format roundtrip test is gated on this anchor.
- `.planning/decisions/v1.4-adr.md` §`#decision-4` — bdk path for P2TR sign. Phase 15 ships the `sign_simple` API surface; actual bdk-backed body lands in Phase 17 WALLET-02.
- `.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` — Module split (D-04), conditional-flip rule (D-01..D-03), wire-format test discipline (D-13), spike branch hygiene (D-19), `Bip322Error` taxonomy seed.
- `.planning/research/sprint-0-A.md` lines 145-175 — The 26-LOC adapter code sketch verbatim (`Bip322Error` enum, `verify_from_wire` fn). Phase 15 lifts this into `shared/src/bip322/mod.rs` with the `Bip322Error` variants expanded per D-31.
- `.planning/research/sprint-0-B.md` lines 315-319 — bdk_wallet 2.3 P2TR PSBT-sign behaviour confirmation; witness extraction must check both `tap_key_sig` AND `final_script_witness`. Phase 17 implementation note carried via the `sign_simple` contract.
- `.planning/research/SUMMARY.md` §"Architecture Plan / Touchpoints" + §"Pitfalls Watchlist" + §"Open Decisions" — Researcher synthesis. V1.4-CRIT-01 (script-type spoofing) and V1.4-CRIT-02 (silent sighash failures) are the load-bearing pitfalls Phase 15's test design must address.
- `.planning/research/PITFALLS.md` §V1.4-CRIT-01 (lines 13-35) — Concrete property-test design for the 9 (spk × witness) cross-shape matrix.
- `.planning/research/PITFALLS.md` §V1.4-CRIT-02 (lines 36-60) — "Three distinct verifier functions, not one parameterized helper" — anchors D-04 module split + per-script-file isolation.
- `.planning/research/PITFALLS.md` §V1.4-MOD-01 (line 89) — OwnershipProof wire-format evolution constraints (versioning + roundtrip test before either side uses new shape).
- `.planning/research/PITFALLS.md` §V1.4-MOD-07 — `bip322_message_hash` as the single source of truth across all three verifiers + sign path.

### Project-level anchors

- `.planning/PROJECT.md` §"Current Milestone: v1.4 BIP-322 Multi-Script Support" — Milestone goal + target features; §"Constraints" (no custom crypto; Tor-native; signet-first; no PII logging; MIT) — bounds every Phase 15 implementation decision.
- `.planning/REQUIREMENTS.md` §"v1.4 Requirements" — BIP322-01..04 + ADVERT-04 mapped to Phase 15 in §"Traceability"; Out-of-Scope table (per-script-type ban tracking, per-script-type rate limits, per-script-type round denominations, publicly advertised script-type breakdown) bounds the error-code mapping decision (D-32).
- `.planning/ROADMAP.md` §"Phase 15: Shared Crate Multi-Script Contract" — Phase goal, 5 success criteria (per-script property tests, 9-combination cross-shape, roundtrip test FIRST, exact-pinned deps, v1.3 invariant), cross-phase invariant.
- `.planning/STATE.md` §"Accumulated Context" + §"Carry-forward constraints from v1.3 REPAIR-01 forensics" — 5 carry-forward rules; rule #1 (wire-format roundtrip test FIRST) is D-13 verbatim; rule #4 (pivot to `/gsd:debug` if 2-3 carry-forward plans appear) is the escape valve if Phase 15 goes sideways.

### v1.3 carry-forward (forensics + invariants)

- `.planning/milestones/v1.3-phases/13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle/13-CONTEXT.md` — REPAIR-01 closure context; lesson #1 (roundtrip test ships FIRST) enforced by D-13 and locked again in this CONTEXT.
- `.planning/milestones/v1.3-phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-CONTEXT.md` — `trust_witness_utxo: true` + real `witness_utxo` requirement; load-bearing for the eventual Phase 17 bdk-sign implementation that `sign_simple` will call into.
- `tests/integration/full_round.rs` — v1.3 P2WPKH-only integration test suite; remains green at this phase boundary (cross-phase invariant).

### Code anchors (Phase 15 modifies the first 3, references the others)

- `shared/src/bip322.rs` (133 LOC) — The custom v1.0 BIP-322 implementation. Phase 15 replaces this single file with the `shared/src/bip322/{mod.rs, p2wpkh.rs, p2tr.rs, p2sh_p2wpkh.rs}` module split per D-04. Existing tests inside the file (lines 78-132) move into the new per-script files or into `shared/tests/`.
- `shared/src/protocol.rs` lines 105-139 — `OwnershipProof` struct + manual `from_json_hex_str` / `to_json_hex_str`. Phase 15 evolves this to the v2 envelope per D-22..D-25; the existing helpers become thin convenience wrappers per CD-7.
- `shared/src/lib.rs` — Module declarations; Phase 15 changes `pub mod bip322;` from "file module" to "directory module" (compiler-handled; no source change needed in lib.rs because `bip322` becomes `bip322/mod.rs`).
- `shared/Cargo.toml` — Phase 15 adds `bip322 = "=0.0.10"`, `thiserror = "1"` (exact pin), `base64` (transitive via bitcoin or direct), and `proptest = "1"` to `[dev-dependencies]`.
- `shared/tests/` (currently empty/none) — Phase 15 creates `shared/tests/ownership_proof_roundtrip.rs` (D-13's 5 cases — ships in its own commit FIRST per CD-10) and `shared/tests/bip322_cross_shape.rs` (D-34's 9 enumerated rejection tests).
- `shared/tests/fixtures/bip322/basic-test-vectors.json` — Vendored snapshot from `bitcoin/bips` at commit SHA recorded in a header comment per D-33.
- `coordinator/src/bitcoin/utxo.rs:99-114` — Existing `Bip322Error` enum + `verify_bip322_simple`. Phase 15 deletes the local `Bip322Error` (replaced by `shared::bip322::Bip322Error` import); Phase 15 leaves `verify_bip322_simple` and the `is_p2wpkh()` gate at line 119 in place — Phase 16 swaps the call site to the new dispatcher.
- `coordinator/src/api/handlers.rs:136` — `OwnershipProof::from_json_hex_str` call site. Phase 15 leaves this call intact; the helper now accepts both v1.3 array-of-hex AND new flat-struct JSON per CD-7, so handlers.rs is bit-compatible at this phase boundary.
- `client/src/round/input.rs:64-65` — `OwnershipProof { witness_stack }` construction + `to_json_hex_str()`. Phase 15 leaves this code path intact (emits v1 shape); Phase 17 WALLET-02 extends it to construct v2 shapes per descriptor type.

### External specs (Phase 15 references; no code touches these directly)

- BIP-322 specification — `https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki`. Implementation reference. Pin commit SHA in D-33 fixture header.
- BIP-322 `basic-test-vectors.json` — `https://github.com/bitcoin/bips/blob/master/bip-0322/basic-test-vectors.json`. Vendored at D-33's pinned SHA.
- BIP-341 (Taproot) — keypath sighash semantics for p2tr.rs.
- BIP-143 (segwit v0 signature hashing) — sighash semantics for p2wpkh.rs AND p2sh_p2wpkh.rs (the latter applies BIP-143 over the unwrapped P2WPKH redeem script).
- BIP-86 / BIP-49 / BIP-84 — descriptor formats referenced from shared types but consumed by Phase 17 WALLET-01.

### Tools / commands relevant to Phase 15 execution

- `cargo tree -p bip322 --no-default-features` (optional verification per CD-9).
- `cargo test -p shared` — primary gate; runs the wire-format roundtrip suite + per-script property tests + 9-combination rejection matrix.
- `cargo audit` — must remain clean after `bip322 = "=0.0.10"` + `thiserror = "1"` + `snafu` (transitive) land in `Cargo.lock`.
- `cargo test --test full_round` (with pinned bitcoind) — cross-phase invariant gate; v1.3 P2WPKH-only suite stays green.
- CI grep gate addition: extend the existing v1.3 REPAIR-02 pattern (`corepc-node` feature pin grep) to also assert `bip322 = "=0.0.10"` and the exact `bdk_wallet = "=2.3.x"` pin live in `shared/Cargo.toml` / `client/Cargo.toml`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`shared/src/bip322.rs::bip322_message_hash` (lines 19-27), `build_bip322_to_spend` (lines 34-53), `build_bip322_to_sign` (lines 60-76)** — Script-type-NEUTRAL primitives. V1.4-MOD-07 single source of truth. Phase 15 moves them to `shared/src/bip322/mod.rs` (or keeps them at module root re-exported) and they are reused unchanged by all three per-script verifiers + the eventual sign paths. Existing tests at lines 110-132 demonstrate they're deterministic — re-validated in the new test layout.
- **`shared/src/bip322.rs::make_bip322_witness` test helper (lines 86-108)** — Reference implementation for generating a valid P2WPKH BIP-322 witness deterministically with `SecretKey::from_slice(&[0x01_u8; 32])`. Phase 15 generalises this into per-script `make_<type>_bip322_witness` test helpers (P2TR uses `secp256k1::sign_schnorr` against the keypath sighash; P2SH-P2WPKH uses the same shape as P2WPKH but with `final_script_sig = OP_HASH160 <redeem-script-hash>` and witness = `[sig, pubkey]`).
- **`bitcoin 0.32.x` primitives** (already in dep graph via workspace): `SighashCache::p2wpkh_signature_hash`, `SighashCache::taproot_key_spend_signature_hash`, `Script::is_p2wpkh / is_p2tr / is_p2sh`, `secp256k1::verify_schnorr`, `XOnlyPublicKey::from_slice`, `bitcoin::Witness`, `bitcoin::psbt::Input`. Every primitive Phase 15 needs is available; the `bip322 = "=0.0.10"` crate uses the same underlying types so adapter conversions are zero-copy.
- **`shared/src/protocol.rs` serde patterns** — `#[serde(skip_serializing_if = "Option::is_none")]` on `msg_randomizer` (line 70) is the pattern for Phase 15's `psbt_input_b64: Option<String>` and `script_type: Option<ScriptType>` fields. NO `#[serde(deny_unknown_fields)]` anywhere (line 3 comment locks this — forward compat per D-06 / T-01-04); Phase 15 preserves this invariant on the new `OwnershipProof` struct.
- **`coordinator/src/bitcoin/utxo.rs:99-114` existing `Bip322Error` enum** — Three variants (`UnsupportedScriptType`, `InvalidWitnessLength`, `ScriptMismatch`). Phase 15 deletes this local enum and replaces it with `shared::bip322::Bip322Error` (expanded to ~10 variants per D-31); the handler-layer wire mapping (`InvalidOwnershipProof`) is preserved per D-32.
- **`BitcoindGuard` + `require_bitcoind!()` macro** (v1.3 Phase 9) — Script-type-agnostic. Phase 15 does NOT use bitcoind directly (`shared/` is a pure crate); Phase 18 reuses these unchanged for the mixed-script E2E.
- **v1.3 REPAIR-02 CI grep gate pattern** — Existing CI step asserts `corepc-node` feature pin. Phase 15 extends this pattern to assert `bip322 = "=0.0.10"` + `thiserror = "1"` exact pins.

### Established Patterns

- **"shared crate is the contract"** (v1.0 pattern, preserved through v1.3) — Both coordinator and client compile against `shared/`. Phase 15 extends, never replaces. The dispatcher API in `shared::bip322` is the explicit one-and-only-way Phase 16/17 invoke BIP-322 verification.
- **Per-round RSA keypair + memory-only round state** (v1.0) — Not directly touched by Phase 15, but the eventual Phase 16 `RegisteredInput.script_type` field will be `#[zeroize(skip)]` per memory-only invariant. Phase 15's `ScriptType` enum derives `Copy + Clone + Debug + PartialEq + Eq + Serialize + Deserialize` so it's friction-free for Phase 16's struct embedding.
- **Exact-pin all dependencies + CI grep gate** (v1.3 REPAIR-02) — Carries to v1.4. Phase 15 adds `bip322 = "=0.0.10"` + `thiserror = "1"` (exact-pinned) and extends the grep gate.
- **Wire-format roundtrip test ships FIRST** (v1.3 REPAIR-01 lesson #1) — Locked in D-13 and again in CD-10. Phase 15's plan ordering: `15-01-PLAN.md` = wire-format roundtrip test (5 D-13 cases) as a standalone atomic commit; `15-02-PLAN.md` = bip322 module split + dispatcher API + Bip322Error; `15-03-PLAN.md` = per-script property tests + 9-combination rejection matrix.
- **NO `#[serde(deny_unknown_fields)]` on wire types** (T-01-04 / D-06 from v1.0) — Forward compat. Phase 15's new `OwnershipProof` MUST follow this convention.
- **`#[serde(default)]` for backwards compat** (existing pattern in `InfoResponse`, etc.) — D-25 applies this directly.
- **T-01-05 "never pass raw bytes; use typed wrappers"** — D-23 preserves this for `ownership_proof: String` containing JSON-serialized `OwnershipProof`.
- **bdk_wallet 2.3 SignOptions { trust_witness_utxo: true }** (v1.3 Phase 12 lesson) — Required for Phase 17's `sign_simple` bdk-backed body; Phase 15 documents this requirement in the `sign_simple` doc-comment so Phase 17 inherits it.

### Integration Points

- **Phase 15 → Phase 16:** `shared::bip322::verify_simple` + `detect_script_type` are the API Phase 16 calls from `coordinator/src/bitcoin/utxo.rs` (replacing the existing `verify_bip322_simple` + `is_p2wpkh()` gate). `shared::protocol::OwnershipProof` v2 fields are what Phase 16's handler decodes. `shared::bip322::Bip322Error` is the typed error Phase 16's handler maps to `ApiError::InvalidOwnershipProof` per D-32.
- **Phase 15 → Phase 17:** `shared::bip322::sign_simple` is the API Phase 17's `client/src/wallet.rs` implements against (the bdk-backed body lives in client/ per ADR Decision #4 — `shared/` exposes the signature, client/ provides the implementation via the trait/fn surface). `shared::protocol::OwnershipProof` v2 envelope is what Phase 17's `client/src/round/input.rs` constructs for `version = 2` proofs (per descriptor type). `shared::bip322::ScriptType` enum gates the `client generate-wallet --type` CLI flag's allowed values.
- **Phase 15 → Phase 18:** `shared/tests/fixtures/bip322/basic-test-vectors.json` (vendored snapshot per D-33) is reused by Phase 18's mixed-script E2E if cross-validation is desired. No direct API dependency.
- **Phase 15 closes the wire-format gate for Phases 16/17** — once the D-13 5-case + 9-rejection matrix is green, downstream phases can land additive code without re-litigating wire-format ambiguity (the REPAIR-01 forensic trace's root cause).

</code_context>

<specifics>
## Specific Ideas

- **`sign_simple` shape in Phase 15** (CD-6 default): The fn signature is `pub fn sign_simple(script_type: ScriptType, spk: &Script, key: &SecretKey, message: &[u8]) -> Result<Witness, Bip322Error>`. In Phase 15, the body for `P2WPKH` is fully implemented (already a known good path via the existing `make_bip322_witness` test helper generalised + manual `secp256k1::sign_ecdsa`); `P2TR` and `P2SH_P2WPKH` bodies are gated behind a `#[cfg(test)]`-only manual implementation so the per-script property tests can run end-to-end inside `shared/` without depending on bdk_wallet. Phase 17 WALLET-02 swaps the production sign call site (in `client/src/round/input.rs`) from `shared::bip322::sign_simple` to a bdk-backed path that produces the same `Witness` output per ADR Decision #4 — the `shared/` contract type-checks, and the actual key handling stays in `client/`.
- **`version = 1` legacy decoder fallthrough** (CD-7 default): `OwnershipProof::from_json_hex_str(s)` tries `serde_json::from_str::<Vec<String>>(s)` first (array-of-hex, v1.3 shape). On success, returns `OwnershipProof { version: 1, witness_stack: <decoded>, psbt_input_b64: None, script_type: None }`. On parse-error, tries `serde_json::from_str::<OwnershipProof>(s)` (flat-struct shape, both v1 and v2). Two-phase try-parse preserves bit-exact v1.3 compatibility AND accepts the new shape.
- **`Network` parameter source** (CD-8 default): `bitcoin::Network` enum, read from coordinator config in Phase 16 (`coordinator.toml` already has a `[bitcoin] network = "signet"` knob from v1.0). Phase 15 just exposes the parameter on `verify_simple`; Phase 17 client reads from its own config and threads through the same way.
- **Plan ordering** (CD-10 default): Three plans land in order — `15-01-PLAN.md` (wire-format roundtrip test FIRST, atomic commit, can't be missed by `git bisect`); `15-02-PLAN.md` (bip322 module split + crate adapter + dispatcher + Bip322Error); `15-03-PLAN.md` (per-script property tests against `basic-test-vectors.json` + 9-combination cross-shape rejection matrix + sign_simple shape). Each plan is atomic and individually re-runnable.
- **The 9-rejection matrix names** are concrete (D-34) and self-documenting. A code reviewer scanning `shared/tests/bip322_cross_shape.rs` sees nine `#[test]` functions whose names spell out exactly which CRIT-01 spoofing vector each closes.
- **Vendored fixture header comment** (D-33): The first line of `basic-test-vectors.json` is preserved as-is from upstream; a sibling `README.md` in `shared/tests/fixtures/bip322/` records the commit SHA + capture date + the exact `curl` command used to fetch (for future bump audits). Example: `# source: https://github.com/bitcoin/bips/blob/<SHA>/bip-0322/basic-test-vectors.json — captured 2026-05-XX via `curl -L`.

</specifics>

<deferred>
## Deferred Ideas

- **Removal of `coordinator/src/bitcoin/utxo.rs::verify_bip322_simple` + the `is_p2wpkh()` gate at line 119** — Phase 16 (ADVERT-03). Phase 15 leaves these in place; Phase 16 swaps the call site to `shared::bip322::verify_simple` + the allowlist dispatcher.
- **Wire ErrorCode expansion** (per-script-type rejection codes) — Anti-feature per REQUIREMENTS.md Out-of-Scope; D-32 locked this. NOT a v1.5 candidate either; the leak surface is the same.
- **TEST-EXT-01 cross-impl differential fixtures** (`ACken2/bip322-js`) — Already in REQUIREMENTS.md Future Requirements; v1.5 candidate. Vendored `basic-test-vectors.json` (D-33) is the v1.4 minimum gate.
- **TEST-EXT-02 regtest on-chain anchor test** — REQUIREMENTS.md Future Requirements; v1.5 candidate. Strongest correctness gate against V1.4-CRIT-02; Phase 15's per-script property tests are the v1.4 minimum.
- **TEST-EXT-03 automated backwards-compat matrix** — REQUIREMENTS.md Future Requirements; v1.5 candidate. Phase 15's D-13 case #2 (v1 shape deserializes correctly) is the v1.4 minimum at the wire-shape level; full grid is v1.5.
- **`sign_simple` production body for P2TR/P2SH-P2WPKH inside shared/** — Phase 17 implements in `client/` via bdk per ADR Decision #4. If bdk ever regresses (v1.5+), D-15's manual fallback budget (80 LOC in `shared/src/bip322/p2tr.rs::sign_p2tr_keypath`) becomes the swap target.
- **`bip322 = "=0.0.10"` → 1.0 reconsider trigger** — Carried over from 14-CONTEXT.md; if `bip322` crate ships 1.0 before v1.5, re-open Decision #1.
- **`#[non_exhaustive]` on `Bip322Error`** — Not addressed in this discussion. Default: NOT non-exhaustive in v1.4 (shared/ + coordinator are in the same workspace, so adding variants is a coordinated change). Plan-phase can decide if non-exhaustive is worth adding now for v1.5 hygiene.
- **`Bip322Error: Send + Sync + 'static`** — Plan-phase verifies thiserror produces these bounds by default (it does, via `std::error::Error` blanket impl); add explicit `+ Sync` constraint only if a tokio task boundary surfaces a `'static` issue at execute time.
- **DECISIONS-INDEX.md rolling summary** — `.planning/DECISIONS-INDEX.md` still doesn't exist; per discuss-phase workflow it's a bounded rolling summary that supersedes per-phase reads. v1.5 candidate per 14-CONTEXT.md.

</deferred>

---

*Phase: 15-Shared Crate Multi-Script Contract*
*Context gathered: 2026-05-30*
