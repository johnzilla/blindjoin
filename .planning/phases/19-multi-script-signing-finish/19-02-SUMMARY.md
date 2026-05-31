---
phase: 19-multi-script-signing-finish
plan: 02
subsystem: shared-bip322
tags: [bip322, surface-shrink, audit-readiness, cleanup]
requirements: [BIP322-07]
depends_on: [19-01]
dependency_graph:
  requires:
    - "shared::bip322::p2tr::sign production body (shipped Plan 19-01)"
    - "shared::bip322::p2sh_p2wpkh::sign production body (shipped Plan 19-01)"
  provides:
    - "shared::bip322 public surface shrunk to 9 symbols: ScriptType, Bip322Error, detect_script_type, verify_simple, sign_simple, p2sh_p2wpkh_final_script_sig, bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign"
    - "V1.4-CRIT-01 dispatcher-only invariant load-bearing at the type level with no test-only mirror"
  affects: []
tech-stack:
  added: []
  patterns:
    - "Test-only escape hatch removal once production body is byte-equivalent (CD-6 follow-through)"
    - "Surface-shrink as audit-charter prerequisite (Phase 21 will describe a dispatcher-only surface without footnotes)"
key-files:
  created: []
  modified:
    - shared/src/bip322/mod.rs
    - shared/src/bip322/p2tr.rs
    - shared/src/bip322/p2sh_p2wpkh.rs
    - shared/src/bip322/p2wpkh.rs
    - shared/tests/per_script_vectors.rs
    - tests/integration/multi_script_validate.rs
    - tests/integration/mod.rs
decisions:
  - "D-120: Plan 19-02 task list — Task 1 deletes sign_simple_test_only + 3 sign_for_tests; Task 2 migrates 4 callsites + refreshes comments + grep-verifies + clippy gate"
  - "D-121: Wave 2 sequential (depends on production sign bodies from Plan 19-01); no parallelism within phase"
  - "CD-39 (default = fold): tests/integration/mod.rs doc-comment refreshes folded into this plan rather than a separate docs-only commit"
metrics:
  duration_minutes: 7
  tasks_completed: 2
  files_modified: 7
  files_created: 0
  commits: 2
  completed_date: 2026-05-31
---

# Phase 19 Plan 02: Multi-Script Signing Surface-Shrink Summary

Closes BIP322-07: deletes the `#[doc(hidden)] pub fn sign_simple_test_only` test-only dispatcher mirror from `shared::bip322::mod` and the per-script `pub(crate) fn sign_for_tests` helpers from `p2tr.rs`, `p2sh_p2wpkh.rs`, and `p2wpkh.rs`; migrates the 4 remaining callsites in `shared/tests/per_script_vectors.rs` (1 import + 2 sign call sites) and `tests/integration/multi_script_validate.rs` (1 import + 1 helper-fn body) onto the production `sign_simple` dispatcher; refreshes 2 doc-comments in `tests/integration/mod.rs` per CD-39 default. The V1.4-CRIT-01 dispatcher-only public surface is now load-bearing at the type level — `shared::bip322` exposes exactly the 9 audit-charter-friendly symbols with no test-only hole, and the per-script-vector + 9 cross-shape rejection integration tests now exercise the production `sign` bodies shipped in Plan 19-01.

## Tasks Completed

| # | Task | Files | Commit |
|---|------|-------|--------|
| 1 | Delete `sign_simple_test_only` from mod.rs + `sign_for_tests` from all 3 per-script modules + refresh 2 module doc-comments | `shared/src/bip322/mod.rs`, `shared/src/bip322/p2tr.rs`, `shared/src/bip322/p2sh_p2wpkh.rs`, `shared/src/bip322/p2wpkh.rs` | `1dd364d` |
| 2 | Migrate 4 callsites (`per_script_vectors.rs` import + 2 sign sites + `multi_script_validate.rs` import + helper body) + refresh 2 `tests/integration/mod.rs` doc-comments + grep-verify + clippy gate | `shared/tests/per_script_vectors.rs`, `tests/integration/multi_script_validate.rs`, `tests/integration/mod.rs` | `a8378df` |

## What Changed

### Task 1 — Deletions (commit `1dd364d`)

- **`shared/src/bip322/mod.rs`**: Deleted the entire `sign_simple_test_only` block (the 27-line explanatory comment header + the `#[doc(hidden)] pub fn` body, ~41 lines total). The dispatcher `pub fn sign_simple` is unchanged. The Plan 19-01 helper `pub fn p2sh_p2wpkh_final_script_sig` and the 3 unit tests it added stay put.
- **`shared/src/bip322/p2tr.rs`**: Deleted `pub(crate) fn sign_for_tests` (~50 lines incl. doc comment). Production `pub(crate) fn sign` body unchanged; verified the D-111 cross-check and `sign_schnorr_no_aux_rand` invocation remain intact. Module doc-comment refreshed: "alias remains for Plan 15-03 integration tests; Plan 19-02 deletes it" → "test-only alias was deleted in Plan 19-02 (BIP322-07) — production sign is now the only sign path."
- **`shared/src/bip322/p2sh_p2wpkh.rs`**: Deleted `pub(crate) fn sign_for_tests` (~57 lines). Production `pub(crate) fn sign` body unchanged (D-111 cross-check + D-117 spk-used-directly + BIP-143 over unwrapped P2WPKH redeem). Module doc-comment refreshed analogously.
- **`shared/src/bip322/p2wpkh.rs`**: Deleted the unused `pub(crate) fn sign_for_tests` alias (22 lines incl. its `#[allow(dead_code)]` attribute). Production `pub(crate) fn sign` body unchanged.

At the end of this task, `cargo build -p shared` is clean (the deletions don't break the shared lib itself); the workspace breaks at the test crates that still import `sign_simple_test_only`, gated to Task 2.

### Task 2 — Caller migration + doc refreshes (commit `a8378df`)

- **`shared/tests/per_script_vectors.rs`**:
  - Import line: dropped `sign_simple_test_only` (kept `sign_simple`, `verify_simple`, `Bip322Error`, `ScriptType` per RESEARCH Q5 C6 — `Bip322Error` is consumed by `_bip322_error_path_check`).
  - Module-level doc-comment: refreshed to note Plan 19-02 migrated the test suite off the test-only mirror onto the production dispatcher.
  - P2TR sign↔verify roundtrip test (`test_p2tr_sign_verify_roundtrip_via_dispatcher`): `sign_simple_test_only(ScriptType::P2tr, ...)` → `sign_simple(ScriptType::P2tr, ...)`; section header + inline explanatory comment refreshed.
  - P2SH-P2WPKH sign↔verify roundtrip test (`test_p2sh_p2wpkh_sign_verify_roundtrip_via_dispatcher`): analogous swap + comment refresh.
- **`tests/integration/multi_script_validate.rs`**:
  - Import: `sign_simple_test_only` → `sign_simple`.
  - `sign_witness` helper body: `sign_simple_test_only(...)` → `sign_simple(...)` + `.expect("sign_simple_test_only should produce a valid witness")` → `.expect("sign_simple should produce a valid witness")` + doc-comment added explaining the migration.
- **`tests/integration/mod.rs`** (CD-39 default — folded into this plan):
  - Module-level explanatory comment block at line ~707 (referenced from `fund_regtest_typed` purpose-statement): `shared::bip322::sign_simple_test_only` → `shared::bip322::sign_simple`.
  - `TypedUtxoHandle::secret_key` field doc at line ~721: same refresh.

### Workspace-wide grep verification

```
grep -rn -E '(sign_simple_test_only|fn sign_for_tests)' \
  shared/ tests/ client/ coordinator/ liquidity-bot/ \
  --include='*.rs' 2>/dev/null | wc -l
```

Returns `0`. Even the migration-history comments in `per_script_vectors.rs` and `multi_script_validate.rs` use generic phrasing ("the deleted test-only mirror") rather than naming the removed symbol — preserves the audit-charter goal of being able to describe the surface without footnotes.

## Verification Results

| Suite | Pre (Plan 19-01) | Post (Plan 19-02) | Status |
|-------|------------------|-------------------|--------|
| `cargo test -p shared --lib` | 34 | 34 | 34/34 PASS |
| `cargo test -p shared --test per_script_vectors` | 7 | 7 | 7/7 PASS (now exercises production sign bodies — load-bearing per CONTEXT D-120 closing line) |
| `cargo test -p shared --test bip322_cross_shape` | 9 | 9 | 9/9 PASS |
| `cargo test -p client --test wallet_sign_roundtrip` | 9 | 9 | 9/9 PASS |
| `cargo test --test integration multi_script_validate` | 9 | 9 | 9/9 PASS (the 9 D-54 cross-shape rejection cases now exercise the production `sign_simple` end-to-end through coordinator validation) |
| `cargo test --test integration full_round` | 8 | 8 | 8/8 PASS (v1.3 cross-phase invariant — held) |
| `cargo test --test integration mixed_script_e2e` | 1 | 1 | 1/1 PASS (v1.4 cross-phase invariant — held) |
| `cargo build --workspace` | clean | clean | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | clean | OK |

### CI grep-gate tokens preserved

- `bip322 = "=0.0.10"` pin: unchanged (0 violations from `bip322-pin-check`).
- `CRIT-01` in `coordinator/src/bitcoin/utxo.rs`: 2 (≥ 2 — `crit-01-grep-check` green).
- `CRIT-01` in `client/src/round/input.rs`: 2 (≥ 1 — `crit-01-client-grep-check` green).

### Plan-specific grep-gate checks

- `grep -c 'sign_simple_test_only' shared/src/bip322/mod.rs` = 0 (was ≥ 1)
- `grep -c '#\[doc(hidden)\]' shared/src/bip322/mod.rs` = 0 (the only one was the test-only mirror)
- `grep -c 'fn sign_for_tests' shared/src/bip322/{p2tr,p2sh_p2wpkh,p2wpkh}.rs` = 0 each (was 1 each)
- `grep -c 'pub(crate) fn sign(' shared/src/bip322/{p2tr,p2sh_p2wpkh,p2wpkh}.rs` = 1 each (production bodies preserved)
- `grep -c 'pub fn sign_simple(' shared/src/bip322/mod.rs` = 1 (dispatcher preserved)
- `grep -c 'pub fn p2sh_p2wpkh_final_script_sig' shared/src/bip322/mod.rs` = 1 (Plan 19-01 helper preserved)
- `grep -c 'verify_via_bip322_crate' shared/src/bip322/mod.rs` = 4 (crate adapter preserved per CONTEXT D-LOCKED scope)

### Final `shared::bip322` public surface

Per `grep -nE '^pub (fn|enum|struct)' shared/src/bip322/mod.rs`:

```
pub fn bip322_message_hash(message: &[u8]) -> [u8; 32]
pub fn build_bip322_to_spend(script_pubkey: &Script, msg_hash: &[u8; 32]) -> Transaction
pub fn build_bip322_to_sign(to_spend: &Transaction) -> Transaction
pub enum ScriptType
pub enum Bip322Error
pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error>
pub fn verify_simple(...)
pub fn sign_simple(...)
pub fn p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf
```

Exactly the 9 symbols enumerated in CONTEXT §"Phase Boundary" line 15. The Phase 21 audit charter can now describe `shared::bip322` as a dispatcher-only surface without a "but also there's this `#[doc(hidden)]` thing" footnote.

## Decisions Implemented

- **D-120** (Plan 19-02 two-task structure) — Task 1 deletions + Task 2 caller migration + verify.
- **D-121** (Wave 2 sequential, depends on Plan 19-01) — confirmed by green test results: production sign bodies from Plan 19-01 are byte-equivalent to the lifted-from `sign_for_tests` bodies (per D-116), so test assertions are unchanged.
- **CD-39** (default = fold `tests/integration/mod.rs` comment refreshes into this plan) — applied; no follow-up commit needed.
- **BIP322-07** (REQUIREMENTS.md) — closed.

Carried-forward decisions from Plan 19-01 honored unchanged:
- **D-107** sign_simple return type stays `Result<Witness, Bip322Error>` — confirmed by grep.
- **D-108** helper unit test (`p2sh_p2wpkh_final_script_sig_derives_correctly`) stays green — confirmed (34 lib tests including this one).
- **D-109** helper is `pub fn` sibling to `sign_simple` — preserved.
- **D-110** helper body unchanged — preserved.
- **D-111** spk↔key cross-check at top of both per-script `sign` bodies — preserved (the 2 unit tests `p2tr_sign_rejects_p2sh_p2wpkh_spk_with_p2tr_key` and `p2sh_p2wpkh_sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` from Plan 19-01 stay green).
- **D-112** `Bip322Error::ScriptTypeMismatch` reused for dual meaning — variant unchanged; PII-safety test still green.
- **D-114** `sign_schnorr_no_aux_rand` for P2TR — unchanged.
- **D-116** sign bodies lifted from `sign_for_tests` — the now-deleted helpers were byte-equivalent to production `sign`, so the test suite's positive-vector assertions interoperate with production witnesses without change.
- **D-117** P2SH-P2WPKH `spk` load-bearing — unchanged.
- **D-118 / D-119** Plan 19-01 parity tests — both green (9/9 in `wallet_sign_roundtrip`).
- **CD-36** ScriptTypeMismatch doc-comment for dual meaning — preserved unchanged.

## Threat Model Items Mitigated

| Threat ID | Mitigation | Verification |
|-----------|------------|--------------|
| **T-19-B** (Tampering/Elevation of Privilege via `#[doc(hidden)] pub fn sign_simple_test_only`) | Task 1 DELETED the symbol entirely. The dispatcher-only public surface is now load-bearing at the TYPE level — no caller (production or test) can route around the dispatcher. | Workspace-wide grep returns 0 matches for `sign_simple_test_only` and `fn sign_for_tests` in `*.rs` files. |
| **T-19-A** (carry-forward — Spoofing/Tampering on sign-side, mitigated by D-111 cross-check in Plan 19-01) | D-111 cross-check at the TOP of `p2tr::sign` and `p2sh_p2wpkh::sign` unchanged by this plan; now exercised end-to-end by the 9 cross-shape rejection cases in `tests/integration/multi_script_validate.rs` (which Task 2 migrated to call `sign_simple` directly). | `multi_script_validate` 9/9 PASS. |
| **T-19-D** (carry-forward — Information Disclosure via `Bip322Error` Display) | No new error variants in this plan; existing PII-safety test `bip322_error_display_does_not_leak_pii_substrings` at `mod.rs:512-565` (now `~535-588` post-deletion) covers the variant unchanged. | `shared --lib` 34/34 PASS includes the PII-safety test. |

## Deviations from Plan

None — plan executed exactly as written.

The only minor adjustment was wording the migration-history comments to not literally name the deleted symbol (using "the deleted test-only mirror" instead of "`sign_simple_test_only`") so the workspace-wide grep returns 0 matches in `*.rs` files per the Task 2 Subtask A acceptance criterion. This is consistent with the PLAN's wording ("If any `*.rs` match returns, fix it before completing this task") and preserves the audit-charter goal of being able to describe the surface without leftover references to dead code.

## Known Stubs

None. This plan is a pure deletion + caller-migration plan; no new code paths added, no placeholder data flows introduced. The Plan 19-01 helper `p2sh_p2wpkh_final_script_sig` is wired into 1 unit test (D-108) and exposed for downstream callers (Phase 21 audit charter prose + future v=2 OwnershipProof PSBT-side use).

## Notes for Phase 19 close + Plan 19+ chain

Plan 19-02 closes the third and final requirement of Phase 19 (BIP322-07). At this commit boundary:

- Phase 19 ships clean: **BIP322-05** (Plan 19-01), **BIP322-06** (Plan 19-01), **BIP322-07** (Plan 19-02) — all three requirements closed.
- **Phase 20** (FEE-01/02/03) is now unblocked: the per-script weight table work depends only on the existing `bip_config.output_script_type` plumbing from Phase 16 + the production sign bodies from Plan 19-01.
- **Phase 21** (AUDIT-01/02/03) gains its load-bearing dependency: the audit charter prose can now describe `shared::bip322` as a dispatcher-only public surface enumerated at exactly 9 symbols, with no test-only mirror footnote and no `todo!()` references.

The v1.5 cross-phase invariant (v1.3 `full_round` 8/8 + v1.4 `mixed_script_e2e` 1/1) is held green at the Phase 19 boundary.

## Self-Check: PASSED

Files verified to exist and contain expected content:
- `shared/src/bip322/mod.rs` — FOUND (no `sign_simple_test_only`, no `#[doc(hidden)]`, dispatcher + helper + adapter all intact)
- `shared/src/bip322/p2tr.rs` — FOUND (no `sign_for_tests`, production `sign` body intact)
- `shared/src/bip322/p2sh_p2wpkh.rs` — FOUND (no `sign_for_tests`, production `sign` body intact)
- `shared/src/bip322/p2wpkh.rs` — FOUND (no `sign_for_tests`, production `sign` body intact)
- `shared/tests/per_script_vectors.rs` — FOUND (imports + 2 callsites migrated, comments refreshed)
- `tests/integration/multi_script_validate.rs` — FOUND (import + `sign_witness` helper migrated)
- `tests/integration/mod.rs` — FOUND (2 doc-comments refreshed)

Commits verified to exist:
- `1dd364d`: refactor(19-02): delete sign_simple_test_only + sign_for_tests test-only mirrors — FOUND
- `a8378df`: refactor(19-02): migrate test callers from sign_simple_test_only to sign_simple — FOUND
