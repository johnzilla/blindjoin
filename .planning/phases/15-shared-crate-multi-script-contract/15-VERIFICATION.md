---
phase: 15-shared-crate-multi-script-contract
verified: 2026-05-29T00:00:00Z
status: passed
score: 5/5 ROADMAP success criteria verified; 5/5 REQ-IDs satisfied
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: n/a
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 15: Shared Crate Multi-Script Contract — Verification Report

**Phase Goal:** `shared/` becomes the single source of truth for BIP-322 multi-script verification and the new wire types, so coordinator and client compile against one contract and produce byte-identical to_spend/to_sign transactions per script type.

**Verified:** 2026-05-29 (initial verification — no previous VERIFICATION.md present)
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### ROADMAP Success Criteria

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `cargo test -p shared` passes per-script sign↔verify roundtrip property tests for P2WPKH, P2TR, P2SH-P2WPKH against `basic-test-vectors.json` (commit-SHA pinned) | ✓ VERIFIED | `cargo test -p shared` → 49 tests pass (27 lib + 9 cross-shape + 7 per-script + 6 roundtrip). `test_p2wpkh_vectors_verify_via_dispatcher`, `test_p2tr_vectors_verify_via_dispatcher`, `test_p2sh_p2wpkh_supplement_verify_via_dispatcher` all green. Fixture pinned at upstream `d77863fb9e` per `shared/tests/fixtures/bip322/README.md:9-10`. |
| 2 | 9 (script_pubkey × witness-shape) cross-shape rejection combinations fail with expected `Bip322Error` variants (V1.4-CRIT-01 spoofing mitigation statically provable) | ✓ VERIFIED | `shared/tests/bip322_cross_shape.rs` contains exactly 9 `#[test]` fns (verified: `grep -c '^#\[test\]' = 9`); all 9 use `matches!(result, Err(Bip322Error::VARIANT))` patterns (no `is_err()` shortcuts). `cargo test -p shared --test bip322_cross_shape` → `test result: ok. 9 passed; 0 failed`. |
| 3 | OwnershipProof wire-format roundtrip test passes in `shared/` BEFORE coordinator/client consume the new shape (v1.3 REPAIR-01 lesson #1 phase boundary) | ✓ VERIFIED | `shared/tests/ownership_proof_roundtrip.rs` shipped as atomic commit `8a202bc` (Plan 15-01) FIRST per CD-10. 6 `#[test]` fns covering 5 D-13 cases + 1 corrupted-base64 sibling. All 6 pass. The coordinator-side dispatcher swap and client-side v2 construction are deferred to Phases 16 + 17. |
| 4 | `shared` crate compiles with exact-pinned dependency versions; no minor-version drift on `bdk_wallet`, `bitcoin`, or `bip322` | ✓ VERIFIED | `shared/Cargo.toml:20` → `bip322 = "=0.0.10"` (exact-equals pin). CI grep gate `.github/workflows/ci.yml:214-236` (`bip322-pin-check`) enforces this. `bdk_wallet` workspace-pinned at `"2.3"` (caret-style — RESEARCH A7 explicit deferral to Phase 17/v1.5; not regression). `bitcoin` workspace-pinned via root Cargo.toml. `cargo build --workspace` → exit 0. `cargo audit` → 0 vulnerabilities, 0 warnings. |
| 5 | v1.3 `full_round::*` integration tests still pass at phase boundary (additive; P2WPKH witness-only unchanged for v1.3-format inputs) | ✓ VERIFIED | `cargo test --test integration full_round` → 8/8 pass in 43.52s. Specific tests confirmed green: `coordinator_info_endpoint_fields`, `adversarial_tampered_psbt_rejected`, `adversarial_replay_token`, `adversarial_wrong_denomination`, `adversarial_invalid_utxo`, `full_round_three_clients`, `blame_non_signer_timeout`, `round_restart_and_completion_after_blame`. |

**Score:** 5/5 ROADMAP success criteria verified.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `shared/src/bip322/mod.rs` | Dispatcher + ScriptType + 10-variant Bip322Error + 26-LOC adapter + script-NEUTRAL primitives + sign_simple_test_only | ✓ VERIFIED | 568 LOC. `pub enum ScriptType` (line 152), `pub enum Bip322Error` (line 170) with all 10 D-31 variants present (UnsupportedProofVersion, WireFormatMismatch, DecodeError, UnrecognisedScriptPubkey, UnsupportedScriptType, ScriptTypeMismatch, InvalidWitnessLength, CrateVerifyFailed, NetworkMismatch, ScriptMismatch). `pub fn detect_script_type` (line 223) routes via is_p2wpkh/is_p2tr/is_p2sh with explicit UnsupportedScriptType for unknown. `pub fn verify_simple` (line 242), `pub fn sign_simple` (line 261). `pub(crate) fn verify_via_bip322_crate` (line 323) — 26-LOC adapter verbatim per Sprint-0-A:145-175. `#[doc(hidden)] pub fn sign_simple_test_only` (line 302-303) with `#[doc(hidden)]` directly above `pub fn`. |
| `shared/src/bip322/p2wpkh.rs` | pub(crate) verify + sign (full production body) + sign_for_tests | ✓ VERIFIED | 96 LOC. `pub(crate) fn verify` (line 22) with arity check `witness.len() != 2 → InvalidWitnessLength { expected: 2, got: w.len() }`. `pub(crate) fn sign` (line 46) — full production ECDSA body lifted from prior `make_bip322_witness`. `pub(crate) fn sign_for_tests` (line 89). No `pub fn` (confirmed via grep). No `todo!`. |
| `shared/src/bip322/p2tr.rs` | pub(crate) verify + sign (todo!) + sign_for_tests (8-step BIP-341 sequence) | ✓ VERIFIED | 96 LOC. `pub(crate) fn verify` (line 18) with arity check `witness.len() != 1 → InvalidWitnessLength { expected: 1, got: w.len() }`. `pub(crate) fn sign` (line 38) body is `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` at line 43 per CD-6. `pub(crate) fn sign_for_tests` (line 60) implements the 8-step BIP-341 sequence (Keypair → tap_tweak → taproot_key_spend_signature_hash → sign_schnorr_no_aux_rand → 64-byte witness). |
| `shared/src/bip322/p2sh_p2wpkh.rs` | pub(crate) verify + sign (todo!) + sign_for_tests | ✓ VERIFIED | 109 LOC. `pub(crate) fn verify` (line 24) with arity check `witness.len() != 2 → InvalidWitnessLength { expected: 2, got: w.len() }`. `pub(crate) fn sign` (line 44) body is `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` at line 49 per CD-6. `pub(crate) fn sign_for_tests` (line 68) builds [sig, pubkey] over unwrapped P2WPKH redeem-script sighash. |
| `shared/src/protocol.rs` | OwnershipProof v2 four-field flat envelope (version + witness_stack + psbt_input_b64 + script_type) | ✓ VERIFIED | `OwnershipProof` struct (line 132) declares all four fields verbatim per D-22. `#[serde(default = "default_proof_version")]` on `version` (line 135). `#[serde(default)]` on `witness_stack` (line 139). `#[serde(skip_serializing_if = "Option::is_none")]` on `psbt_input_b64` and `script_type` (lines 142, 147). `fn default_proof_version() -> u8 { 1 }` (line 152). NO `#[serde(deny_unknown_fields)]` anywhere (T-01-04 invariant preserved). |
| `shared/Cargo.toml` | Exact-pinned bip322, thiserror, base64, proptest (dev) | ✓ VERIFIED | Line 13: `base64 = "0.22"`. Line 20: `bip322 = "=0.0.10"` (exact-equals pin). Line 21: `thiserror = { workspace = true }`. Line 29: `proptest = { workspace = true }` (dev-deps). |
| `shared/tests/ownership_proof_roundtrip.rs` | 5 D-13 cases + corrupted-base64 sibling | ✓ VERIFIED | 6 `#[test]` fns: `v2_roundtrip_p2wpkh`, `v2_roundtrip_p2tr`, `v2_roundtrip_p2sh_p2wpkh`, `v1_legacy_decode_array_of_hex`, `unknown_version_permissive_decode`, `corrupted_base64_in_psbt_input_permissive_decode`. Defensive assertion in v2_roundtrip_p2sh_p2wpkh confirms wire form is kebab-case `"p2sh-p2wpkh"` AND NOT snake_case `"p2sh_p2wpkh"`. |
| `shared/tests/per_script_vectors.rs` | Positive sign↔verify tests via dispatcher for all 3 script types | ✓ VERIFIED | 7 `#[test]` fns (6 required + 1 classify helper). All assert ≥1 vector exercised per RESEARCH A3. Uses dispatcher API only (`shared::bip322::{verify_simple, sign_simple, sign_simple_test_only}`). All 7 pass. |
| `shared/tests/bip322_cross_shape.rs` | 9 enumerated #[test] fns per D-34 verbatim | ✓ VERIFIED | Exactly 9 `#[test]` fns with names matching D-34 verbatim (verified via grep). Each uses `matches!(result, Err(Bip322Error::VARIANT { .. }))` per RESEARCH A3 (no is_err shortcuts). All 9 pass. |
| `shared/tests/fixtures/bip322/basic-test-vectors.json` | Vendored upstream snapshot | ✓ VERIFIED | Exists, 6895 bytes; `jq '.simple \| length' = 4` (3 P2WPKH + 1 P2WSH-multisig + 1 P2TR per README). |
| `shared/tests/fixtures/bip322/p2sh_p2wpkh_supplement.json` | P2SH-P2WPKH supplement + P2WPKH recovery entries | ✓ VERIFIED | Exists, 1569 bytes; `jq 'length' = 3` (1 P2SH-P2WPKH + 2 P2WPKH recovery). Types verified: [0]=p2sh-p2wpkh, [1]=p2wpkh, [2]=p2wpkh. |
| `shared/tests/fixtures/bip322/README.md` | D-33 provenance: source URL, commit SHA, capture date, curl command + recovery rationale | ✓ VERIFIED | Documents BOTH SHAs: `d77863fb9e` (May 2026 — vendored verbatim per D-33) and `3ab70c98a7` (April 2026 — recovery from upstream P2WPKH encoding anomaly). Records May 2026 0xb2 0x6a 0x40 prefix anomaly in detail. Curl commands present. v1.5 TEST-EXT-01 promotion path documented. |
| `coordinator/src/bitcoin/utxo.rs` | Local Bip322Error deleted; shared::bip322 imported; is_p2wpkh() gate + verify_bip322_simple intact (Phase 16 territory) | ✓ VERIFIED | Local `pub enum Bip322Error` not present (grep exits non-zero). Line 4: `use shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign, Bip322Error};`. Line 117: `if !script_pubkey.is_p2wpkh()` (gate still present per Phase 15 boundary). Line 112: `pub fn verify_bip322_simple(` (function still present per Phase 15 boundary). |
| `.github/workflows/ci.yml` | bip322-pin-check job | ✓ VERIFIED | Line 214: `bip322-pin-check:` job key. Lines 215-236: full job spec mirroring `corepc-node-feature-pin-check` structurally. Grep pattern `bip322\s*=` + allow `=\s*"=0\.0\.10"`. Identical SHA-pinned actions/checkout (`34e114876b...`). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `shared::bip322::verify_simple` | per-script `verify` | match on ScriptType | ✓ WIRED | mod.rs:249-253 — explicit match arm routes ScriptType::P2wpkh → p2wpkh::verify, P2tr → p2tr::verify, P2shP2wpkh → p2sh_p2wpkh::verify. |
| per-script `verify` | `verify_via_bip322_crate` adapter | super::verify_via_bip322_crate | ✓ WIRED | p2wpkh.rs:34, p2tr.rs:30, p2sh_p2wpkh.rs:36 — all three delegate to super::verify_via_bip322_crate after arity pre-flight. |
| `verify_via_bip322_crate` | `bip322::verify_simple` (=0.0.10 crate) | bip322::verify_simple(&Address, msg, witness.clone()) | ✓ WIRED | mod.rs:331 — `bip322::verify_simple(&address, message, witness.clone())` per Sprint-0-A:145-175 26-LOC adapter; errors wrapped via `Bip322Error::CrateVerifyFailed { #[source] }`. |
| `coordinator::bitcoin::utxo` | `shared::bip322::Bip322Error` | use shared::bip322::{...Bip322Error} | ✓ WIRED | coordinator/src/bitcoin/utxo.rs:4 — explicit import. Local enum deleted. |
| `OwnershipProof.script_type` | `ScriptType` | crate::bip322::ScriptType (re-exported sibling field) | ✓ WIRED | shared/src/protocol.rs:148 — `Option<crate::bip322::ScriptType>` field type. No module cycle (protocol imports bip322; reverse not present). |
| `sign_simple_test_only` | per-script sign_for_tests | match on ScriptType (test-only dispatcher mirror) | ✓ WIRED | mod.rs:308-313 — P2wpkh→p2wpkh::sign (production), P2tr→p2tr::sign_for_tests, P2shP2wpkh→p2sh_p2wpkh::sign_for_tests. |
| `per_script_vectors.rs` test harness | `verify_simple` + `sign_simple_test_only` + fixtures | use shared::bip322::{...} + include_str! | ✓ WIRED | tests/per_script_vectors.rs:21 imports public API. Lines 26-27 include_str! both fixtures at compile time. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `shared/src/bip322/p2tr.rs` | 43 | `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` | ℹ️ Info | REQUIRED per CD-6 + ADR Decision #4 — not a debt marker. Explicitly scoped to Phase 17 WALLET-02. References formal follow-up phase identifier. |
| `shared/src/bip322/p2sh_p2wpkh.rs` | 49 | `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` | ℹ️ Info | REQUIRED per CD-6 + ADR Decision #4 — not a debt marker. References formal follow-up phase identifier. |

No TBD / FIXME / XXX / HACK / unreferenced TODO markers found in Phase 15-modified files.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Shared lib + integration test suite passes | `cargo test -p shared` | 49 tests pass (27 lib + 9 cross-shape + 7 per-script + 6 roundtrip) | ✓ PASS |
| Cross-shape rejection matrix passes | `cargo test -p shared --test bip322_cross_shape` | `test result: ok. 9 passed; 0 failed` | ✓ PASS |
| Per-script positive vectors pass | `cargo test -p shared --test per_script_vectors` | `test result: ok. 7 passed; 0 failed` | ✓ PASS |
| Ownership proof wire roundtrip passes | `cargo test -p shared --test ownership_proof_roundtrip` | `test result: ok. 6 passed; 0 failed` | ✓ PASS |
| v1.3 cross-phase invariant | `cargo test --test integration full_round` | 8/8 pass in 43.52s | ✓ PASS |
| Workspace builds | `cargo build --workspace` | exit 0 | ✓ PASS |
| Supply chain clean | `cargo audit --json` | vulnerabilities.count = 0, warnings = 0 | ✓ PASS |
| Exact pin on bip322 | `grep -E '^bip322\s*=\s*"=0\.0\.10"' shared/Cargo.toml` | match | ✓ PASS |
| thiserror workspace re-export | `grep -E '^thiserror\s*=' shared/Cargo.toml` | `thiserror = { workspace = true }` | ✓ PASS |
| base64 exact-pin | `grep -E '^base64\s*=\s*"0\.22"' shared/Cargo.toml` | match | ✓ PASS |
| proptest dev-dep | `grep -E '^proptest\s*=' shared/Cargo.toml` | `proptest = { workspace = true }` | ✓ PASS |
| No per-script pub fn leak | `grep -E 'pub fn (verify\|sign)_(p2wpkh\|p2tr\|p2sh_p2wpkh)' shared/src/bip322/p*.rs` | 0 matches (exit 1) | ✓ PASS |
| Coordinator local enum deleted | `grep -E 'pub enum Bip322Error' coordinator/src/bitcoin/utxo.rs` | exit 1 (not present) | ✓ PASS |
| Coordinator imports shared Bip322Error | `grep -E 'use shared::bip322::.*Bip322Error' coordinator/src/bitcoin/utxo.rs` | match (line 4) | ✓ PASS |
| is_p2wpkh() gate intact | `grep -E 'is_p2wpkh\(\)' coordinator/src/bitcoin/utxo.rs` | match (line 117) | ✓ PASS |
| verify_bip322_simple intact | `grep -E 'fn verify_bip322_simple' coordinator/src/bitcoin/utxo.rs` | match (line 112) | ✓ PASS |
| todo! markers in P2TR/P2SH-P2WPKH sign | `grep -E 'todo!' shared/src/bip322/p2tr.rs shared/src/bip322/p2sh_p2wpkh.rs` | 2+ matches each (Phase 17 WALLET-02) | ✓ PASS |
| 9 cross-shape tests by exact name | `grep -c '^#\[test\]' shared/tests/bip322_cross_shape.rs` | exactly 9 | ✓ PASS |
| bip322-pin-check CI job present | `grep -E 'bip322-pin-check' .github/workflows/ci.yml` | match (line 214) | ✓ PASS |

### Probe Execution

No conventional `scripts/*/tests/probe-*.sh` paths exist in the repo for Phase 15 — coverage relies on `cargo test` suites and the CI grep gate. Not applicable.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| BIP322-01 | 15-02 | `shared` exposes `ScriptType` enum + `detect_script_type(scriptPubKey) -> Result<ScriptType, UnsupportedScriptType>` (no fallthrough default arm) | ✓ SATISFIED | `pub enum ScriptType { P2wpkh, P2tr, P2shP2wpkh }` at mod.rs:152. `pub fn detect_script_type` at mod.rs:223 with explicit if/else-if/else routing via is_p2wpkh/is_p2tr/is_p2sh; unknown returns `Err(Bip322Error::UnsupportedScriptType)` at mod.rs:231. Unit test `detect_script_type_rejects_op_return_with_unsupported_script_type` exercises the explicit error path. |
| BIP322-02 | 15-02 | `verify_simple` dispatches to per-script — P2WPKH BIP-143 ECDSA, P2TR BIP-341 Schnorr (accepts 64-byte SIGHASH_DEFAULT + 65-byte SIGHASH_ALL), P2SH-P2WPKH BIP-143 over unwrapped redeem with HASH160 cross-check | ✓ SATISFIED | `pub fn verify_simple` (mod.rs:242) dispatches via match. Per-script files perform arity pre-flight then delegate to bip322 = "=0.0.10" crate's `verify_simple` (which handles BIP-143/BIP-341/64-65-byte branching + HASH160 cross-check internally per Sprint-0-A:145-175 + RESEARCH "Don't Hand-Roll" row 2). Per_script_vectors.rs exercises P2TR vector at simple[3] (64-byte SIGHASH_DEFAULT) successfully. |
| BIP322-03 | 15-02 | `sign_simple` symmetric to `verify_simple` — produces correct witness stack per script type | ✓ SATISFIED | `pub fn sign_simple` (mod.rs:261) dispatches via match. P2WPKH production body fully implemented (p2wpkh.rs:46-72). P2TR + P2SH-P2WPKH production bodies are `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` per ADR Decision #4 + CD-6 (this is the explicit phase boundary — Phase 17 swaps in bdk). The contract signature is locked. Test-only dispatcher mirror `sign_simple_test_only` produces correct witnesses for all three types; per_script_vectors roundtrip tests confirm sign↔verify cycle. |
| BIP322-04 | 15-03 | Per-script property tests against `basic-test-vectors.json` (commit-SHA pinned) + 9 cross-shape rejection combinations | ✓ SATISFIED | Per-script positive tests in `shared/tests/per_script_vectors.rs` (3 source-driven tests + 3 sign↔verify roundtrip tests). 9 enumerated cross-shape rejection tests in `shared/tests/bip322_cross_shape.rs` per D-34 verbatim. Fixture pinned at `bitcoin/bips@d77863fb9e` per README provenance. All 16 tests green. |
| ADVERT-04 | 15-01 | OwnershipProof wire format extended to carry P2SH-P2WPKH final_script_sig (PSBT-input shape per ADR Decision #3); roundtrip test ships in `shared/` BEFORE coordinator/client consume (v1.3 REPAIR-01 lesson #1) | ✓ SATISFIED | OwnershipProof v2 envelope at shared/src/protocol.rs:131-149 with `psbt_input_b64: Option<String>` + `script_type: Option<ScriptType>` sibling fields per D-22+D-24. `shared/tests/ownership_proof_roundtrip.rs` (commit `8a202bc`) shipped as atomic commit FIRST per CD-10 BEFORE the bip322 crate dep landed (15-02) and BEFORE coordinator/client consumed the new shape. 5 D-13 cases + 1 corrupted-base64 sibling = 6 passing tests. |

No ORPHANED requirements — all 5 Phase 15 REQ-IDs are present in at least one plan's frontmatter (`requirements:` field) and trace to verified implementation.

## Boundary Check Results (Scope Creep / Scope Shrink Detection)

| Check | Expected | Actual | Status |
|-------|----------|--------|--------|
| `coordinator/src/bitcoin/utxo.rs::is_p2wpkh()` gate at line ~119 still present (NOT removed — Phase 16's job) | Match `is_p2wpkh()` | Match found at line 117 | ✓ Boundary HOLDS (no scope creep) |
| `coordinator/src/bitcoin/utxo.rs::verify_bip322_simple` function still present (NOT removed) | Match `fn verify_bip322_simple` | Match found at line 112 | ✓ Boundary HOLDS (no scope creep) |
| NO touches to `client/` source (Phase 17 territory) | Zero modifications | 1 commit (`25a7dba`) touched `client/src/round/input.rs:63-71` | ⚠️ Auto-fix required (acceptable per Rule 3) |
| `sign_simple` production bodies for P2TR + P2SH-P2WPKH MUST be `todo!("Phase 17 WALLET-02")` per CD-6 (full bdk-backed bodies = SCOPE CREEP) | Match `todo!` in both files | `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` at p2tr.rs:43 and p2sh_p2wpkh.rs:49 | ✓ Boundary HOLDS (CD-6 honoured exactly) |

**On the `client/src/round/input.rs` boundary nuance:** Phase 15 Plan 15-01 commit `25a7dba` extended one struct literal in `client/src/round/input.rs` from `OwnershipProof { witness_stack }` (v1.3 two-field) to `OwnershipProof { version: 1, witness_stack, psbt_input_b64: None, script_type: None }` (v2 four-field with explicit v1 wire-shape defaults). This was a Rust compile-time requirement after the `OwnershipProof` struct evolved — without it, the workspace would not build. The auto-fix sets `version: 1` + both `Option` fields = `None`, which the CD-7 `to_json_hex_str` branch encodes as the v1.3 array-of-hex form bit-exactly. Verified by `cargo test --test integration full_round` → 8/8 pass (same wire shape as v1.3). This is acceptable per Rule 3 auto-fix policy and explicitly documented as a deviation in 15-01-SUMMARY.md "Auto-fixed Issues" section. The change is non-behavioural — Phase 17 WALLET-02 will replace it with descriptor-aware v2 construction. **Not a scope-creep gap.**

## Auto-fix Deviation Review

| Fix | Description | Verified? | Notes |
|-----|-------------|-----------|-------|
| Fix 1 | `build_bip322_to_sign` Version `TWO → 0` to match BIP-322 §5 + bip322 crate `util::create_to_sign` | ✓ Correct | mod.rs:66 + mod.rs:125 use `Version(0)`. Doc comment at mod.rs:81-104 records the rationale. Aligns with BIP. v1.3 path consistent because both sign+verify call the same helper (full_round 8/8 green). |
| Fix 2 | `build_bip322_to_sign` OP_RETURN script_pubkey bare 1-byte (was 2 bytes via `new_op_return([])`) | ✓ Correct | mod.rs:121-123 uses `Builder::new().push_opcode(opcodes::all::OP_RETURN).into_script()`. Doc comment at mod.rs:107-120 records the BIP-322 §5 reasoning. |
| Fix 3 | `#[cfg(test)] pub fn` → `#[doc(hidden)] pub fn` for `sign_simple_test_only`; per-script `sign_for_tests` promoted from `#[cfg(test)] pub(crate)` to `pub(crate)` | ✓ Correct + safe | mod.rs:302-303 — `#[doc(hidden)]` is on the line immediately preceding `pub fn sign_simple_test_only` (verified via grep). Symbol is NOT part of cargo doc output. The `_test_only` suffix + `#[doc(hidden)]` together signal production callers MUST NOT invoke. V1.4-CRIT-01 spoofing surface stays constrained — dispatcher routes by ScriptType only; no per-script `pub fn` exists. |
| Fix 4 | Upstream `basic-test-vectors.json` SHA `d77863fb9e` had malformed P2WPKH base64; recovered via earlier SHA `3ab70c98a7`; README documents both | ✓ Correct + documented | README.md:9-10 records vendor SHA `d77863fb9e` verbatim per D-33. README.md:30-45 documents the May 2026 0xb2 0x6a 0x40 prefix anomaly. README.md:53-58 documents the recovery via earlier SHA `3ab70c98a7` (entries 0-1 of supplement). v1.5 TEST-EXT-01 promotion path noted at README.md:43-45 + 85-90. Per-script test harness defensively skips malformed entries with `eprintln!` note (per_script_vectors.rs:122-132). Supplement provides ≥1 clean P2WPKH per RESEARCH A3. |

All four auto-fixes are correct, safe, and well-documented.

## Cross-Phase Invariant Status

| Invariant | Status | Evidence |
|-----------|--------|----------|
| v1.3 P2WPKH-only full_round::* tests stay green | ✓ HOLDS | `cargo test --test integration full_round` → 8/8 pass in 43.52s |
| `shared/Cargo.toml` has exact-pinned bip322 + workspace thiserror | ✓ HOLDS | Lines 20-21 verified |
| CI grep gate enforces bip322 pin | ✓ HOLDS | `.github/workflows/ci.yml:214-236` verified |
| `cargo audit` clean | ✓ HOLDS | 0 vulnerabilities, 0 warnings (718 deps) |
| No-PII-logging constraint | ✓ HOLDS | `bip322_error_display_does_not_leak_pii_substrings` test passes; coordinator error messages in `verify_bip322_simple` interpolate only underlying bitcoin error Display (no outpoints/addresses/keys) |

## Phase 16 / 17 Readiness

The Phase 15 closure produces a stable contract for Phases 16 and 17 to consume:

- **Phase 16 derives against:** `shared::bip322::verify_simple` + `detect_script_type` + 10-variant `Bip322Error` + `OwnershipProof.script_type` sibling field. Phase 16 swaps `validate_utxo`'s call site, adds the allowlist config, adds the PKARR `supported_script_types` field, and adds the CRIT-01 cross-check via `detect_script_type(on_chain_spk)`.
- **Phase 17 derives against:** the same dispatcher API for client-side sign. WALLET-02 replaces `sign_simple_test_only` call sites in production with `bdk_wallet`-backed paths; `sign_simple`'s P2TR + P2SH-P2WPKH branches flip from `todo!()` to bdk delegations.

## Human Verification Required

None. Phase 15 is a pure-crate change with no UI surface, no user-facing flow, no real-time behaviour, no external services — every truth is observable in code + test output and was verified above.

## Gaps Summary

**No gaps found.** All 5 ROADMAP success criteria verified. All 5 REQ-IDs satisfied. Both scope-creep checks (is_p2wpkh gate + verify_bip322_simple intact) and scope-shrink checks (`todo!` markers for P2TR/P2SH-P2WPKH sign) pass. All 4 auto-fixes verified correct + documented. CI grep gate added. `cargo audit` clean.

The minor `client/src/round/input.rs` modification is a Rust compile-time auto-fix that preserves bit-exact v1.3 wire encoding (confirmed by full_round 8/8 tests passing) and is explicitly documented in 15-01-SUMMARY.md.

---

*Verified: 2026-05-29*
*Verifier: Claude (gsd-verifier, goal-backward)*

## VERIFICATION PASSED
