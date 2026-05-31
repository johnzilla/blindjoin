# Phase 15: Shared Crate Multi-Script Contract - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 15-CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-30
**Phase:** 15-Shared Crate Multi-Script Contract
**Areas discussed:** OwnershipProof v2 wire shape, Module layout + dispatcher style, Error taxonomy + lib choice, Spec-vector fixture + rejection harness

---

## OwnershipProof v2 wire shape

### Q1: How should the v2 OwnershipProof envelope coexist with the v1 (witness-only) shape?

| Option | Description | Selected |
|--------|-------------|----------|
| Single flat struct, serde defaults | One `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }`. Coordinator branches on version. Smallest blast radius; aligns with D-12 verbatim. | ✓ |
| Tagged serde enum v1/v2 | `enum OwnershipProof { V1 { ... }, V2 { ... } }`. Re-introduces the B1 shape ADR Decision #3 explicitly rejected. | |
| Two separate structs + dispatcher fn | `OwnershipProofV1` + `OwnershipProofV2` + try-v2-then-v1 dispatcher. Doubles type surface + round-trip test matrix. | |

**User's choice:** Single flat struct, serde defaults
**Notes:** Matches D-12 verbatim. Coordinator runs `match proof.version` — clean.

### Q2: How should the v2 envelope ride through InputRegRequest.ownership_proof?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep `ownership_proof: String` | Stays a String containing JSON-serialized OwnershipProof. Zero wire change for outer request; preserves T-01-05. | ✓ |
| Switch to `ownership_proof: OwnershipProof` | Typed serde field. Cleaner Rust API but breaks v1.3 wire shape (adds nesting). | |

**User's choice:** Keep `ownership_proof: String`
**Notes:** Preserves T-01-05 "never pass raw bytes". v1.3 helpers become thin serde_json wrappers.

### Q3: Where does script_type live in v2: inside the PSBT or as a sibling envelope field?

| Option | Description | Selected |
|--------|-------------|----------|
| Sibling field on OwnershipProof | `script_type: Option<ScriptType>` on envelope. Coordinator cross-checks against `detect_script_type(on_chain_spk)` per D-10 / CRIT-01. Test #3 becomes a literal compare. | ✓ |
| Derived from psbt_input contents | Inferred from `psbt.witness_utxo.script_pubkey`. Weakens cross-check (checks PSBT vs chain, not DECLARATION vs chain). | |
| Both | Belt-and-suspenders; adds a third reject branch. | |

**User's choice:** Sibling field on OwnershipProof
**Notes:** Defensible at the wire surface; CRIT-01 cross-check is a literal field comparison.

### Q4: Should the version field default via `#[serde(default)]` when missing from incoming JSON?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — default = 1 | v1.3 clients compatible at wire level. D-12 says this. | ✓ |
| No — require version explicitly | Stricter; kills WALLET-04's compat guarantee at shared/ boundary. | |

**User's choice:** Yes — default = 1
**Notes:** WALLET-04 compat shim in Phase 17 just doesn't set the field for legacy mode.

---

## Module layout + dispatcher style

### Q1: Where does the bip322-crate verification adapter (the 26-LOC sketch from sprint-0-A) live?

| Option | Description | Selected |
|--------|-------------|----------|
| Inside mod.rs alongside dispatcher | Private fn in mod.rs called by `verify_simple`. Matches "mod.rs = public API + dispatcher" from 14-CONTEXT.md D-04. | ✓ |
| Dedicated `shared/src/bip322/adapter.rs` | Matches Sprint-0-A's sketch path verbatim. One more file but 1:1 with spike record. | |
| Inlined per-script | Each per-script file calls `bip322::verify_simple` directly. Duplicates Address+error mapping 3×. | |

**User's choice:** Inside mod.rs alongside dispatcher
**Notes:** Sprint-0-A's `adapter.rs` path was analytical sketch, not binding location. Dispatcher and adapter are the same conceptual layer.

### Q2: What's the public-API shape exposed from shared::bip322?

| Option | Description | Selected |
|--------|-------------|----------|
| Dispatcher fns only | `verify_simple`, `sign_simple`, `detect_script_type` + ScriptType + Bip322Error. Per-script files are crate-private. Eliminates wrong-arity-for-script + CRIT-01 spoofing at the API surface. | ✓ |
| Per-script fns + dispatcher | Both dispatcher AND `verify_p2wpkh / verify_p2tr / ...` pub. More flexible but invites coordinator to call wrong-type fn. | |
| Only ScriptType + per-script fns (no dispatcher) | Coordinator does `match script_type { ... }` at call site. Inverts API; concentrates dispatch in coordinator. | |

**User's choice:** Dispatcher fns only
**Notes:** Caller cannot bypass dispatch — no `verify_p2wpkh` to misuse. CRIT-01 closed at the API layer.

### Q3: How should the v2 OwnershipProof type live relative to shared::bip322?

| Option | Description | Selected |
|--------|-------------|----------|
| Stays in shared::protocol | OwnershipProof in `protocol.rs` alongside InputRegRequest. shared::bip322 owns verifiers + ScriptType only. ScriptType re-exported for serde derive. | ✓ |
| Moves to shared::bip322 | Co-locates wire type with verifier dispatch. But splits the v1.0 protocol-types convention. | |

**User's choice:** Stays in shared::protocol
**Notes:** Preserves v1.0 convention; no module cycle (`protocol` imports from `bip322`, never reverse).

---

## Error taxonomy + lib choice

### Q1: Where does the unified Bip322Error type live?

| Option | Description | Selected |
|--------|-------------|----------|
| `shared/src/bip322/mod.rs` | Single enum `shared::bip322::Bip322Error`. Coordinator-local at `utxo.rs:99` deleted. Single source of truth; matches "shared/ is the contract". | ✓ |
| `shared/src/errors.rs` alongside ApiError | Mixes API-layer (ErrorCode SCREAMING_SNAKE_CASE) with protocol-internal errors. | |
| `shared/src/bip322/errors.rs` (sub-module) | Dedicated errors.rs inside the bip322 module. Keeps mod.rs thin; one more file. | |

**User's choice:** `shared/src/bip322/mod.rs`
**Notes:** Delete the coordinator-local `Bip322Error` in Phase 15.

### Q2: Which error library?

| Option | Description | Selected |
|--------|-------------|----------|
| thiserror | Ecosystem default; zero runtime cost; matches existing patterns. Sprint-0-A's adapter sketch already used this syntax. | ✓ |
| snafu | Enters dep graph transitively via bip322 crate anyway. But context-based ergonomics differ from existing patterns. | |
| Hand-rolled Display + Error impls | No proc-macro deps. Cheapest deps; doesn't scale from 3 to ~10 variants. | |

**User's choice:** thiserror
**Notes:** snafu wraps via `#[source]` per Sprint-0-A's sketch; we present a thiserror-derived enum at the shared/ boundary.

### Q3: How granular should the variant taxonomy be?

| Option | Description | Selected |
|--------|-------------|----------|
| Wire + dispatch + per-script (~10 variants) | UnsupportedProofVersion, WireFormatMismatch, DecodeError, UnrecognisedScriptPubkey, UnsupportedScriptType, ScriptTypeMismatch, InvalidWitnessLength, CrateVerifyFailed, NetworkMismatch, ScriptMismatch (legacy). Pattern-match-able for blame. | ✓ |
| Coarse 3-variant | WireFormat / Unsupported / Verification with String reason. Loses pattern-matchability. | |
| Newtype-per-script-type sub-errors | P2wpkhError + P2trError + P2shP2wpkhError wrapped in top-level. Strongest separation; Phase 16 doesn't need that yet (ban list uniform per D-08). | |

**User's choice:** Wire + dispatch + per-script
**Notes:** Each variant maps 1:1 to a D-13 test case or 9-rejection scenario.

### Q4: How does Bip322Error map to the existing wire ErrorCode (ApiError)?

| Option | Description | Selected |
|--------|-------------|----------|
| All map to `ErrorCode::InvalidOwnershipProof` | Single wire code preserves v1.3 contract; internal pattern-match preserved for logging. Zero contract change for /round/input clients. | ✓ |
| Add new ErrorCode variants (UnsupportedScriptType, ScriptTypeMismatch, ...) | Per-error-code mapping enables WALLET-04 reactions. But REQUIREMENTS.md Out-of-Scope warns publicly advertised per-script-type breakdown is anti-feature — same leak shape. | |
| Deferred to Phase 16 | Phase 15 ships typed error only. Less context for Phase 16 to decide. | |

**User's choice:** All map to `ErrorCode::InvalidOwnershipProof`
**Notes:** Preserves v1.3 contract + avoids per-script-type wire leak.

---

## Spec-vector fixture + rejection harness

### Q1: How is the BIP-322 basic-test-vectors.json pinned and accessed at test time?

| Option | Description | Selected |
|--------|-------------|----------|
| Vendored snapshot at commit SHA | `tests/fixtures/bip322/basic-test-vectors.json` with header comment recording bitcoin/bips commit SHA + capture date. `include_str!` at compile time. Zero CI network. | ✓ |
| Git submodule | Always traceable to upstream but adds submodule init + MBs to fresh clones; v1.3 REPAIR-02 forensics warn against extra CI network. | |
| build.rs fetches at compile time | Smallest footprint; worst CI story (network at every build). | |

**User's choice:** Vendored snapshot at commit SHA
**Notes:** Bumping SHA = explicit code change. Reviewable in diff.

### Q2: How are the 9 (script_pubkey × witness-shape) cross-shape rejections expressed?

| Option | Description | Selected |
|--------|-------------|----------|
| Enumerated 9 #[test] cases | Nine explicit functions (`reject_p2wpkh_spk_with_p2tr_witness`, ...). 1:1 to CRIT-01 scenarios; failures localize to one fn. | ✓ |
| Proptest matrix generator | Single proptest fn over (ScriptType, WitnessShape). Less code; less direct failure output; non-determinism complicates the "exact 9-combination" contract. | |
| Table-driven (data + one runner) | One #[test] iterates `[(ScriptType, WitnessShape, ExpectedError); 9]`. Middle ground; failures still report "iteration k". | |

**User's choice:** Enumerated 9 #[test] cases
**Notes:** Diagonal entries are the positive sign↔verify property tests against `basic-test-vectors.json` (not part of this matrix). Each rejection test asserts a specific Bip322Error variant.

---

## Claude's Discretion

User accepted Claude's defaults on the following:
- **CD-6:** `sign_simple` ships as a fully-implemented P2WPKH path + `#[cfg(test)]`-only manual P2TR/P2SH-P2WPKH paths in Phase 15; Phase 17 swaps the production sign body to bdk at the client call site.
- **CD-7:** v1 legacy decoder tries `Vec<String>` (v1.3 array-of-hex) first, falls back to `OwnershipProof` flat-struct deserialization.
- **CD-8:** `bitcoin::Network` enum directly as `verify_simple` parameter; coordinator reads from `coordinator.toml`.
- **CD-9:** `bip322 = "=0.0.10"` default features unless Sprint-0-A flagged otherwise; plan-phase verifies via `cargo tree --no-default-features`.
- **CD-10:** Three-plan ordering — wire-format roundtrip test FIRST as its own commit (per D-13 / REPAIR-01 lesson #1), then module split + dispatcher, then per-script property tests + 9-rejection matrix.

## Deferred Ideas

- Removal of `coordinator/src/bitcoin/utxo.rs::verify_bip322_simple` + `is_p2wpkh()` gate → Phase 16
- Wire `ErrorCode` expansion per-script-type → anti-feature; not v1.5 either
- TEST-EXT-01 cross-impl differential fixtures (`ACken2/bip322-js`) → v1.5
- TEST-EXT-02 regtest on-chain anchor test → v1.5
- TEST-EXT-03 automated backwards-compat matrix → v1.5
- `sign_simple` production body for P2TR/P2SH-P2WPKH inside shared/ → Phase 17 implements via bdk in client/
- `bip322 = "=0.0.10"` → 1.0 reconsider trigger → v1.5 watch
- `#[non_exhaustive]` on Bip322Error → plan-phase / v1.5 hygiene decision
- `Bip322Error: Send + Sync + 'static` thread-boundary check → plan-phase verifies thiserror produces these by default
- DECISIONS-INDEX.md rolling summary → v1.5 candidate
