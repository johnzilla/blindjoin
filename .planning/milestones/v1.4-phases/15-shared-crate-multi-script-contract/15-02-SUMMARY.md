---
phase: 15
plan: 02
subsystem: shared::bip322 dispatcher + 26-LOC crate adapter + coordinator error swap + CI grep gate
tags: [bip322, dispatcher, module-split, thiserror, ci-pin, adapter, V1.4-CRIT-01-mitigation]
requires:
  - v1.4-adr.md#decision-1
  - 14-CONTEXT.md (carry-forward constraint #3 — exact-pin every dep + CI grep gate)
  - 15-CONTEXT.md#D-04
  - 15-CONTEXT.md#D-26
  - 15-CONTEXT.md#D-27
  - 15-CONTEXT.md#D-29
  - 15-CONTEXT.md#D-31
  - 15-CONTEXT.md#D-32
  - 15-CONTEXT.md#CD-6
  - 15-CONTEXT.md#CD-9
  - sprint-0-A.md:145-175 (26-LOC adapter sketch verbatim)
  - 15-01-SUMMARY.md (ScriptType + OwnershipProof v2 envelope locked)
provides:
  - "shared::bip322 directory module per D-04: mod.rs + p2wpkh.rs + p2tr.rs + p2sh_p2wpkh.rs (flat shared/src/bip322.rs deleted)"
  - "pub fn verify_simple(ScriptType, &Script, &Witness, &[u8], Network) -> Result<(), Bip322Error> dispatcher"
  - "pub fn sign_simple(ScriptType, &Script, &SecretKey, &[u8]) -> Result<Witness, Bip322Error> dispatcher (P2WPKH full, P2TR + P2SH-P2WPKH todo!() per CD-6)"
  - "pub fn detect_script_type(&Script) -> Result<ScriptType, Bip322Error> with no fallthrough default arm"
  - "pub enum Bip322Error: 10-variant taxonomy per D-31 verbatim (thiserror-derived; PII-safe by construction)"
  - "pub(crate) fn verify_via_bip322_crate: the 26-LOC bip322 = '=0.0.10' adapter from Sprint-0-A:145-175 verbatim per D-26"
  - "coordinator/src/bitcoin/utxo.rs: local Bip322Error enum deleted; shared::bip322::Bip322Error imported per D-29"
  - "CI grep gate `bip322-pin-check` at .github/workflows/ci.yml mirroring corepc-node-feature-pin-check pattern"
affects:
  - shared/src/bip322.rs (DELETED — replaced by the directory module per D-04)
  - shared/src/bip322/mod.rs (CREATED — dispatcher + adapter + Bip322Error + ScriptType + primitives)
  - shared/src/bip322/p2wpkh.rs (CREATED — pub(crate) verify + full sign + #[cfg(test)] sign_for_tests)
  - shared/src/bip322/p2tr.rs (CREATED — pub(crate) verify + todo!() sign + #[cfg(test)] sign_for_tests using 8-step BIP-341 sequence)
  - shared/src/bip322/p2sh_p2wpkh.rs (CREATED — pub(crate) verify + todo!() sign + #[cfg(test)] sign_for_tests)
  - shared/Cargo.toml (added bip322 = "=0.0.10" exact-equals pin + thiserror = { workspace = true })
  - Cargo.lock (added bip322 v0.0.10, snafu v0.8.9, snafu-derive v0.8.9 — exactly the 3 transitives baselined by Sprint-0-A)
  - coordinator/src/bitcoin/utxo.rs (deleted local Bip322Error enum; remapped 6 Err(...) returns; updated matches!() to struct-shape InvalidWitnessLength)
  - .github/workflows/ci.yml (added bip322-pin-check job mirroring corepc-node-feature-pin-check)
tech-stack:
  added:
    - "bip322 = \"=0.0.10\" (exact-equals pin per Phase 14 carry-forward constraint #3; CC0-1.0 license; rust-bitcoin org)"
    - "snafu v0.8.9 (transitive via bip322; production-safe error lib used by bip322 internally)"
    - "snafu-derive v0.8.9 (transitive proc-macro; build-only)"
    - "thiserror = { workspace = true } (workspace-managed; resolves to v1.x per workspace pin)"
  patterns:
    - "RESEARCH Pattern 1 dispatcher-only public API: per-script verify+sign are pub(crate) inside mod.rs's mod declarations; V1.4-CRIT-01 spoofing vector is statically unreachable at the type level (no `pub fn verify_p2wpkh` exists for a caller to invoke against the wrong SPK)"
    - "RESEARCH Pattern 2 26-LOC adapter: pub(crate) fn verify_via_bip322_crate wraps bip322::verify_simple(&Address, msg, Witness) into our (spk, witness, msg, network) wire shape; preserves underlying bip322::Error via #[source] (no string collapse); witness.clone() is intentional per crate API (Witness by value)"
    - "RESEARCH Pattern 4 #[cfg(test)] sign_for_tests in each per-script file: P2WPKH sign_for_tests delegates to the production sign (already fully implemented per CD-6); P2TR sign_for_tests uses the 8-step BIP-341 Keypair::from_secret_key → tap_tweak → taproot_key_spend_signature_hash → sign_schnorr_no_aux_rand sequence verified in Sprint-0-B; P2SH-P2WPKH sign_for_tests builds the [sig, pubkey] witness with sighash over the UNWRAPPED P2WPKH SPK derived from the pubkey"
    - "Thiserror #[source] error chain on UnrecognisedScriptPubkey (bitcoin::address::FromScriptError) and CrateVerifyFailed (bip322::Error) — Display impls do NOT chain through #[source] by default (Pitfall 4); the chain is accessible via error.source() for server-side logging; wire collapses to ErrorCode::InvalidOwnershipProof per D-32 anyway"
    - "RESEARCH Pattern: bip322 crate's Error type lives at the CRATE ROOT (bip322::Error), NOT bip322::error::Error — the `error` module itself is private; the Error enum is re-exported at lib.rs:29 via `pub use {error::Error, sign::*, util::*, verify::*}`. Plan reading flagged this; runtime build error caught the typo and CONTEXT D-31 line 118 has bip322::error::Error — see deviation R-3 below"
key-files:
  created:
    - shared/src/bip322/mod.rs
    - shared/src/bip322/p2wpkh.rs
    - shared/src/bip322/p2tr.rs
    - shared/src/bip322/p2sh_p2wpkh.rs
    - .planning/phases/15-shared-crate-multi-script-contract/15-02-SUMMARY.md
  modified:
    - shared/Cargo.toml
    - coordinator/src/bitcoin/utxo.rs
    - .github/workflows/ci.yml
    - Cargo.lock
  deleted:
    - shared/src/bip322.rs (replaced by the directory module per D-04 — INTENTIONAL deletion, central to the plan)
decisions:
  - "Plan 15-02 ships as 3 atomic commits per CD-10 sequential ordering — Task 1 (module split + adapter + deps), Task 2 (coordinator-local Bip322Error swap), Task 3 (CI grep gate)"
  - "Bip322Error::CrateVerifyFailed's #[source] type is bip322::Error (root re-export), not bip322::error::Error — the `error` submodule itself is private. Per Rule 3 auto-fix the planner's literal type path in CONTEXT D-31 line 118 (`bip322::error::Error`) was adjusted to use the public re-export at the crate root; the runtime semantics are identical (it's literally the same type by re-export). No D-31 invariant violation; the variant identifier and the #[source] attribute are preserved verbatim."
  - "shared/src/bip322/mod.rs's tests block has 8 new sanity tests on top of the 6 lifted from the flat file: 3× detect_script_type positive cases (P2WPKH, P2TR, P2SH-P2WPKH), 1× detect_script_type OP_RETURN reject, 1× Bip322Error Display non-empty, 1× Bip322Error PII-safe-substring grep (8 of the 10 variants checked — UnrecognisedScriptPubkey and CrateVerifyFailed have non-Default-constructible #[source] payloads so they're skipped by the inline check; the static text of those variants' #[error(\"...\")] strings is PII-safe by inspection: 'script_pubkey is not a recognised single-key address ...' and 'BIP-322 crate verification failed')"
  - "Bip322Error's UnrecognisedScriptPubkey Display string contains the phrase 'single-key address (P2WPKH / P2TR / P2SH-P2WPKH)' which mentions the word 'address' — the PII-safe grep allows this because it only flags the substring 'address:' (with colon, indicating an interpolated value), not the generic English-language usage. PROJECT.md no-PII-logging invariant preserved by construction."
  - "Per-script files (p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs) declare both `verify` AND `sign` as pub(crate) only. The dispatchers verify_simple + sign_simple are the ONLY public entry points. V1.4-CRIT-01 spoofing vector (caller bypassing dispatch to call the wrong per-script verifier) is statically unreachable at the type level — there is no `pub fn verify_p2wpkh` for a caller to hand-construct a mismatched call against."
  - "Coordinator verify_bip322_simple body STAYS in place at Phase 15 with its is_p2wpkh() gate at line 117 preserved per D-29 + Deferred Ideas — Phase 16 ADVERT-03 swaps the call site to the new dispatcher and removes the whole legacy body wholesale. Phase 15's scope ends at the typed-error swap."
  - "The 6 Err(...) returns in coordinator/src/bitcoin/utxo.rs::verify_bip322_simple were remapped per the D-31 variant mapping table in 15-PATTERNS.md lines 580-587: UnsupportedScriptType→identical; InvalidWitnessLength(usize)→struct InvalidWitnessLength { expected, got }; SigParseError + PubkeyParseError + VerificationFailed→folded into DecodeError(format!(\"<class>: {e}\")); ScriptMismatch→identical. CrateVerifyFailed is NOT used by the legacy body (which does manual ECDSA, not crate-backed verify) — Phase 16's call-site swap is where CrateVerifyFailed becomes load-bearing."
  - "bdk_wallet caret-pin retighten (RESEARCH A7) is OUT OF SCOPE for Phase 15 per the plan-specific constraint — bdk_wallet stays at the workspace's `\"2.3\"` (caret-style) pin; Phase 17 or v1.5 owns the retighten. Lockfile pins the exact resolved version and cargo audit runs on each PR per the existing audit job."
  - "Three new transitives accepted into Cargo.lock per Sprint-0-A baseline: bip322 v0.0.10, snafu v0.8.9, snafu-derive v0.8.9. Lockfile dependency count rose from 707 (pre-Task-1) to 710 (post-Task-1) — exactly the +3 Sprint-0-A predicted. No drift; no checkpoint:human-verify trigger."
metrics:
  duration: "~10 minutes"
  tasks_completed: 3
  files_modified: 4
  files_created: 4
  files_deleted: 1
  tests_added: 8
  tests_passing: "27 shared lib + 6 shared integration + 3 coordinator utxo + 8 integration full_round = 44 across the cross-cut surface"
  cargo_audit_status: "clean — 0 vulnerabilities, 0 warnings, 710 deps (Sprint-0-A baseline matches: 707 → 710 = exactly +3 transitives as predicted)"
  completed_date: "2026-05-30"
---

# Phase 15 Plan 02: Shared::bip322 Dispatcher + 26-LOC Adapter + Coordinator Error Swap + CI Grep Gate Summary

Splits `shared/src/bip322.rs` into the four-file directory module per D-04, lands the dispatcher-only public API per D-27 (V1.4-CRIT-01 spoofing vector statically unreachable), ports the 26-LOC `bip322 = "=0.0.10"` crate adapter from `sprint-0-A.md:145-175` verbatim per D-26, deletes the coordinator-local `Bip322Error` enum per D-29, and adds the `bip322-pin-check` CI grep gate per Phase 14 carry-forward constraint #3.

## Tasks Executed

### Task 1 — Directory module split + 26-LOC adapter + 10-variant Bip322Error + bip322/thiserror deps

**Commit:** `c873db1` — `feat(15-02): split shared::bip322 into dispatcher module per D-04 + D-27`

- Deleted `shared/src/bip322.rs` (flat file); created `shared/src/bip322/{mod.rs, p2wpkh.rs, p2tr.rs, p2sh_p2wpkh.rs}`.
- Lifted the three script-NEUTRAL primitives (`bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`) and the v1.4 `ScriptType` enum verbatim into `mod.rs` so the public path `shared::bip322::*` stays byte-stable for the coordinator + client + 15-01 roundtrip suite.
- Added the dispatcher-only public surface per D-27: `ScriptType`, `Bip322Error` (10 variants per D-31 verbatim), `detect_script_type` (no fallthrough; unknown → `UnsupportedScriptType`), `verify_simple` (matches on `ScriptType` → per-script verify), `sign_simple` (matches on `ScriptType` → per-script sign). Per-script files declare `verify` + `sign` as `pub(crate)` only — V1.4-CRIT-01 spoofing vector statically unreachable.
- Ported the 26-LOC `bip322` crate adapter from `sprint-0-A.md:145-175` verbatim as `pub(crate) fn verify_via_bip322_crate` in `mod.rs` per D-26. Preserves the underlying `bip322::Error` via `#[source]` (no string collapse).
- P2WPKH sign body fully implemented per CD-6 (lifted from the prior `make_bip322_witness` test helper); P2TR + P2SH-P2WPKH production sign bodies are `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` per CD-6. Each per-script file ships a `#[cfg(test)] sign_for_tests` helper for Plan 15-03's positive-vector tests.
- `shared/Cargo.toml`: added `bip322 = "=0.0.10"` (exact-equals pin per Phase 14 carry-forward constraint #3) and `thiserror = { workspace = true }`.
- 8 new sanity tests in `mod.rs` (`detect_script_type` cases for all 4 SPK shapes, `Bip322Error::Display` non-empty + PII-safe).

### Task 2 — Coordinator-local Bip322Error deletion + variant remap + cross-phase invariant preservation

**Commit:** `777eaf6` — `refactor(15-02): delete coordinator-local Bip322Error; import shared per D-29`

- Deleted the local `Bip322Error` enum at `coordinator/src/bitcoin/utxo.rs` (was lines 87-101); extended the existing `use shared::bip322::{...}` import to include `Bip322Error` per D-29.
- Remapped the 6 `Err(...)` returns in `verify_bip322_simple` per the D-31 variant mapping table (see Coordinator Variant Remapping Table below).
- Updated `bip322_wrong_witness_length`'s `matches!()` from tuple shape `InvalidWitnessLength(1)` to struct shape `InvalidWitnessLength { expected: 2, got: 1 }`.
- `is_p2wpkh()` hard gate at line 117 STAYS (Phase 16 ADVERT-03 removes it).
- PROJECT.md no-PII-logging constraint preserved — `format!()` strings interpolate only the underlying bitcoin error's `Display`.

### Task 3 — `bip322-pin-check` CI grep gate

**Commit:** `cfea17c` — `ci(15-02): add bip322 exact-version pin check per Phase 14 carry-forward #3`

- Added a new `bip322-pin-check` job to `.github/workflows/ci.yml` immediately after the existing `corepc-node-feature-pin-check` job, mirroring its structure verbatim.
- Greps every workspace `Cargo.toml` for `bip322\s*=` declarations and asserts each match also matches `=\s*"=0\.0\.10"` (exact-equals pin operator). Uses the IDENTICAL `actions/checkout@34e114876b...` SHA-pinned reference as the analog job.
- Local sanity grep against the current tree produces ZERO drift matches — the only `bip322 = ...` line in any workspace `Cargo.toml` is exactly `bip322 = "=0.0.10"` in `shared/Cargo.toml`.
- NO `thiserror-pin-check` job (workspace-managed per RESEARCH A12); NO `bdk_wallet-pin-check` job (RESEARCH A7 defers to v1.5).

## Final `Bip322Error` Variant List (D-31 Verbatim)

| Variant | Payload | Display string | Wire bucket (D-32) |
|---|---|---|---|
| `UnsupportedProofVersion(u8)` | u8 | `"unsupported OwnershipProof version: <n>"` | `InvalidOwnershipProof` |
| `WireFormatMismatch(String)` | owned string | `"wire-format mismatch: <s>"` | `InvalidOwnershipProof` |
| `DecodeError(String)` | owned string | `"PSBT/base64 decode error: <s>"` | `InvalidOwnershipProof` |
| `UnrecognisedScriptPubkey { #[source]: bitcoin::address::FromScriptError }` | typed source | `"script_pubkey is not a recognised single-key address (P2WPKH / P2TR / P2SH-P2WPKH)"` | `InvalidOwnershipProof` |
| `UnsupportedScriptType` | unit | `"unsupported script type"` | `InvalidOwnershipProof` |
| `ScriptTypeMismatch { declared, derived }` | two ScriptType | `"declared script_type <D> does not match on-chain <D>"` | `InvalidOwnershipProof` |
| `InvalidWitnessLength { expected, got }` | two usize | `"invalid witness length: expected <n>, got <n>"` | `InvalidOwnershipProof` |
| `CrateVerifyFailed { #[source]: bip322::Error }` | typed source | `"BIP-322 crate verification failed"` | `InvalidOwnershipProof` |
| `NetworkMismatch { decoded, configured }` | two Network | `"network mismatch: address decoded for <N>, configured for <N>"` | `InvalidOwnershipProof` |
| `ScriptMismatch` | unit | `"legacy v1 script mismatch"` | `InvalidOwnershipProof` |

All 10 variants are present in `shared/src/bip322/mod.rs` and verified by `grep "\bV\b"` against the file. **PII-safe by construction:** every variant's `#[error("...")]` template interpolates ONLY enum-payload metadata (u8 / usize / `ScriptType` / `Network`) — no outpoint, address bytes, key bytes, or amount appears in any variant's `Display`.

## Coordinator Variant Remapping Table (Task 2 — D-29 + D-31 + D-32)

For 15-03 and Phase 16 to reason about wire-shape continuity, this is the exact remap applied to `coordinator/src/bitcoin/utxo.rs::verify_bip322_simple`:

| Old (local enum) | New (shared::bip322::Bip322Error) | Notes |
|---|---|---|
| `Err(Bip322Error::UnsupportedScriptType)` | `Err(Bip322Error::UnsupportedScriptType)` | Variant name identical per D-31. |
| `Err(Bip322Error::InvalidWitnessLength(witness_stack.len()))` | `Err(Bip322Error::InvalidWitnessLength { expected: 2, got: witness_stack.len() })` | Tuple → struct; arity-aware. |
| `.map_err(\|_\| Bip322Error::VerificationFailed)` (sighash compute) | `.map_err(\|e\| Bip322Error::DecodeError(format!("p2wpkh sighash: {e}")))` | Class label preserved; underlying Display only. |
| `.map_err(\|_\| Bip322Error::SigParseError)` | `.map_err(\|e\| Bip322Error::DecodeError(format!("ecdsa signature parse: {e}")))` | Folded into DecodeError. |
| `.map_err(\|_\| Bip322Error::PubkeyParseError)` (pubkey parse) | `.map_err(\|e\| Bip322Error::DecodeError(format!("pubkey parse: {e}")))` | Folded into DecodeError. |
| `.map_err(\|_\| Bip322Error::VerificationFailed)` (verify_ecdsa) | `.map_err(\|e\| Bip322Error::DecodeError(format!("ecdsa verify: {e}")))` | Folded into DecodeError. |
| `.map_err(\|_\| Bip322Error::PubkeyParseError)` (wpubkey_hash) | `.map_err(\|e\| Bip322Error::DecodeError(format!("wpubkey_hash: {e}")))` | Folded into DecodeError. |
| `return Err(Bip322Error::ScriptMismatch);` | `return Err(Bip322Error::ScriptMismatch);` | Variant name identical per D-31 (v1 path parity). |

**Wire shape unchanged per D-32**: every variant continues to map to `ErrorCode::InvalidOwnershipProof` at `coordinator/src/api/handlers.rs:136-137`. The `.map_err(\|e\| UtxoError::InvalidProof { reason: e.to_string() })` at `validate_utxo` line 74-75 stays unmodified.

## CI `bip322-pin-check` Job Spec

- **Job key:** `bip322-pin-check` (`.github/workflows/ci.yml:214`).
- **Job name:** `"bip322 exact-version pin check"`.
- **Trigger:** runs under the existing workflow jobs block — same triggers as `corepc-node-feature-pin-check`.
- **Checkout SHA:** `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5` (v4.3.1) — identical SHA-pinned reference as `corepc-node-feature-pin-check` for hash-pinning consistency.
- **Grep pattern:** `bip322\s*=` (line declarations).
- **Allow pattern:** `=\s*"=0\.0\.10"` (exact-equals pin operator).
- **Error message:** `"ERROR: bip322 declaration(s) above lack the exact-version pin '=0.0.10'."` + `"The bip322 crate is pre-1.0; minor changes can break the adapter at shared/src/bip322/mod.rs."`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] CONTEXT D-31's literal `bip322::error::Error` type path is private; correct public path is `bip322::Error` (root re-export)**

- **Found during:** Task 1 first `cargo build -p shared --tests` invocation. The compiler errored with `E0603: module 'error' is private` at `shared/src/bip322/mod.rs:158:25` where `Bip322Error::CrateVerifyFailed { #[source]: bip322::error::Error }` mirrored the literal path from CONTEXT D-31 line 118.
- **Issue:** `bip322` crate's `lib.rs:29` declares `pub use {error::Error, sign::*, util::*, verify::*}` — the `error` module itself is private; the `Error` enum is re-exported at the crate root. The literal source path `bip322::error::Error` quoted in CONTEXT D-31 is the underlying location, not the public path. The runtime type is identical (it's literally the same type via re-export), but the visible path used in the `#[source]` attribute must be the public one.
- **Fix:** Changed `source: bip322::error::Error` → `source: bip322::Error` in the `CrateVerifyFailed` variant. D-31's variant name and the `#[source]` attribute are preserved verbatim; only the type-path syntax was adjusted to the public re-export. No D-31 invariant violation.
- **Files modified:** `shared/src/bip322/mod.rs` (Bip322Error variant declaration only).
- **Commit:** `c873db1`.

**2. [Rule 3 — Blocking] `bitcoin::hashes::Hash` trait must be in scope where `to_byte_array()` is called**

- **Found during:** Task 1 first `cargo build -p shared --tests` invocation. The compiler errored with `E0599: no method named 'to_byte_array'` at three call sites: `shared/src/bip322/p2wpkh.rs:92` (`sighash.to_byte_array()` in production `sign`), `shared/src/bip322/p2tr.rs:81` (sighash conversion in `sign_for_tests`), and the test helper in `mod.rs` (`*sighash.as_byte_array()` resolved fine because the dereference path is different).
- **Issue:** The `Hash` trait that provides `to_byte_array()` is `bitcoin::hashes::Hash`. The existing `coordinator/src/bitcoin/utxo.rs` head imports it directly (`use bitcoin::{..., hashes::Hash}`), but my newly-created per-script files did not. The `bitcoin::hashes` glob in `mod.rs` only brings types into scope, not the trait.
- **Fix:** Added `use bitcoin::hashes::Hash;` at the top of `shared/src/bip322/p2wpkh.rs` and inside the `#[cfg(test)] sign_for_tests` function body of `p2tr.rs` and `p2sh_p2wpkh.rs` (the production `sign` bodies in those two files are `todo!()` and don't need the trait yet — Phase 17 will add the import then).
- **Files modified:** `shared/src/bip322/p2wpkh.rs`, `shared/src/bip322/p2tr.rs`, `shared/src/bip322/p2sh_p2wpkh.rs`.
- **Commit:** `c873db1` (rolled up into Task 1's commit because the fix happened during Task 1's RED/GREEN cycle, not after).

**3. [Rule 1 — Minor] Removed unused `use bitcoin::hashes::Hash as _` from `mod.rs` tests block**

- **Found during:** Task 1 build after fixing the `to_byte_array` errors. The compiler warned `unused import` for `bitcoin::hashes::Hash as _` in `mod.rs`'s `#[cfg(test)]` block — the trait was imported for the test helper `make_p2wpkh_script_and_witness` but the call now uses `*sighash.as_byte_array()` (deref of slice) which doesn't need the trait.
- **Fix:** Removed the line.
- **Files modified:** `shared/src/bip322/mod.rs` (tests block).
- **Commit:** `c873db1`.

### Other notes (no deviation)

- The 3 `sign_for_tests` functions in the per-script files generate `dead_code` warnings under `cargo build -p shared --tests`. This is expected — they are consumed by Plan 15-03's per-script property tests which have not yet landed. The warnings will resolve when 15-03 commits its `shared/tests/bip322_*.rs` test files. **Not** suppressed via `#[allow(dead_code)]` because that would mask future regressions (e.g., 15-03's tests linking the production `sign_simple` instead of `sign_for_tests`).
- The `_spk` parameter in `shared/src/bip322/p2sh_p2wpkh.rs::sign_for_tests` is intentionally prefixed `_` — the caller passes the outer P2SH SPK for API symmetry with the other per-script signers, but the sighash uses the UNWRAPPED P2WPKH SPK derived from the pubkey (per BIP-143). The crate's `verify_full_p2wpkh(is_p2sh=true)` at `verify.rs:167-169` mirrors this.

## Authentication Gates

None — all work is local (no network calls, no auth required).

## Cross-Phase Invariant Verification

- `cargo test --test integration -- full_round`: **8/8 PASS** — v1.3 P2WPKH happy-path + adversarial + blame + restart suite all green at this plan boundary.
- `cargo build --workspace`: **PASS** — coordinator, client, shared, liquidity-bot all compile.
- `cargo test -p shared`: **33/33 PASS** (27 lib + 6 ownership_proof_roundtrip integration).
- `cargo test -p coordinator --lib bitcoin::utxo`: **3/3 PASS** (the existing `bip322_valid_p2wpkh`, `bip322_wrong_witness_length`, `bip322_wrong_message_fails` survive the variant rename with the updated struct-shape `matches!()`).
- `cargo audit`: **CLEAN** — 710 deps, 0 vulnerabilities, 0 warnings (advisory db `eaf48e7`, matches Sprint-0-A baseline of 707 + exactly 3 new transitives `bip322 v0.0.10`, `snafu v0.8.9`, `snafu-derive v0.8.9`).

## v1.3 → v1.4 Continuity Confirmation

- `verify_bip322_simple` at `coordinator/src/bitcoin/utxo.rs` still exists, still gated by `is_p2wpkh()` at line 117, still called from `validate_utxo` line 73. The wire shape of UTXO registration is unchanged; the local error enum was replaced with the shared one without any caller-visible behavioural difference. Phase 16 ADVERT-03 owns the call-site swap.
- The new `bip322-pin-check` CI job runs alongside `corepc-node-feature-pin-check` — both gate every PR. No change to existing CI triggers or `needs:` graph.
- The lockfile dependency-count delta (707 → 710 = +3 transitives) matches Sprint-0-A's prediction exactly. No unexpected transitive drift triggered the resume-time `## CHECKPOINT REACHED lockfile-transitive-drift` halt.

## Known Stubs

- P2TR `sign` production body: `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` — INTENTIONAL stub per CD-6; Phase 17 owns the wire-up.
- P2SH-P2WPKH `sign` production body: `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` — INTENTIONAL stub per CD-6; Phase 17 owns the wire-up.

Both stubs are unreachable in Phase 15's execution paths because nobody (coordinator nor client) yet calls `sign_simple`. The `#[cfg(test)] sign_for_tests` helpers exist for Plan 15-03's per-script property-test surface and are NOT marked stubs — they're production-quality test signers.

## Threat Flags

None new. The plan's `<threat_model>` lists T-15-02-V1.4-CRIT-01, -02, -03, -PII, -D-32, -DOS-clone, -T-01-04, -SHA-stub — all are mitigated or accepted-by-design per the plan and the artifacts in this commit set do not introduce surface beyond what the threat model already enumerates.

## Self-Check: PASSED

- `[ -f shared/src/bip322/mod.rs ]` → FOUND
- `[ -f shared/src/bip322/p2wpkh.rs ]` → FOUND
- `[ -f shared/src/bip322/p2tr.rs ]` → FOUND
- `[ -f shared/src/bip322/p2sh_p2wpkh.rs ]` → FOUND
- `! [ -f shared/src/bip322.rs ]` → FOUND (deleted per D-04)
- `git log | grep c873db1` → FOUND
- `git log | grep 777eaf6` → FOUND
- `git log | grep cfea17c` → FOUND
- All success criteria gates (cargo build --workspace, cargo test -p shared, cargo test --test integration -- full_round, cargo audit, all grep gates) → PASSED
