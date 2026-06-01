---
phase: 19-multi-script-signing-finish
plan: 01
subsystem: shared-bip322
tags: [bip322, signing, schnorr, ecdsa, multi-script, audit-readiness, p2tr, p2sh-p2wpkh]
requirements: [BIP322-05, BIP322-06]
depends_on: []
dependency_graph:
  requires: []
  provides:
    - "shared::bip322::p2tr::sign — production Schnorr keypath body (no todo!())"
    - "shared::bip322::p2sh_p2wpkh::sign — production BIP-143 body (no todo!())"
    - "shared::bip322::p2sh_p2wpkh_final_script_sig — pub helper (BIP-141 scriptSig)"
    - "client::wallet::BdkClientWallet::from_descriptor — single-key WIF descriptor support"
  affects:
    - "client/src/wallet.rs — from_descriptor branches on /0/*) marker (Rule 3 fix)"
tech-stack:
  added: []
  patterns:
    - "Lift-from-sign_for_tests production-body promotion (D-116)"
    - "Defense-in-depth spk↔key cross-check at top of sign body (D-111)"
    - "Variant doc-comment reuse for dual-meaning (D-112 + CD-36)"
    - "Inline plan-comment provenance in production body (CD-38)"
    - "Single-key WIF descriptor support via Wallet::create_single (Rule 3)"
key-files:
  created: []
  modified:
    - shared/src/bip322/p2tr.rs
    - shared/src/bip322/p2sh_p2wpkh.rs
    - shared/src/bip322/mod.rs
    - client/tests/wallet_sign_roundtrip.rs
    - client/src/wallet.rs
decisions:
  - "D-107: sign_simple Result<Witness, Bip322Error> return type unchanged (no Bip322ProofPieces)"
  - "D-108: p2sh_p2wpkh_final_script_sig_derives_correctly unit test pins BIP-141 wire shape"
  - "D-109: Helper is `pub fn` SIBLING to sign_simple in mod.rs (NOT in p2sh_p2wpkh.rs)"
  - "D-110 (corrected): Helper produces 23-byte ScriptBuf (NOT 24; RESEARCH §Q3 corrects byte count)"
  - "D-111: spk↔key cross-check in BOTH per-script sign bodies (P2TR + P2SH-P2WPKH)"
  - "D-112: Bip322Error::ScriptTypeMismatch variant reused for dual meaning (no new variant)"
  - "D-113: Cross-check algorithm — P2TR via tap_tweak + new_p2tr_tweaked; P2SH-P2WPKH via wpubkey_hash + new_p2sh"
  - "D-114: P2TR sign uses sign_schnorr_no_aux_rand (BIP-340 §3.3 — deterministic)"
  - "D-115: RESEARCH Q1 confirmed bdk_wallet 2.3 uses sign_schnorr_no_aux_rand — byte-equality assertion safe (no downgrade)"
  - "D-116: sign_for_tests bodies LIFTED near-verbatim into production sign"
  - "D-117: P2SH-P2WPKH _spk → spk load-bearing after cross-check (rebuild-from-key footgun removed)"
  - "D-118: p2tr_shared_sign_matches_bdk_sign_byte_for_byte parity test (T-19-C mitigation)"
  - "D-119: p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte parity test (RFC 6979 deterministic)"
  - "CD-34: Builder::push_slice with <&PushBytes>::try_from per RESEARCH Q3"
  - "CD-35: Per-script parity test names — kept separate (no consolidation)"
  - "CD-36: ScriptTypeMismatch variant doc-comment updated for dual meaning (default = yes)"
  - "CD-37: Cross-check rejection unit tests added — 2 tests pinning D-111 behavior (default = yes)"
  - "CD-38: Inline `// Plan 19-01 Task N` provenance comments in sign bodies (default = yes)"
metrics:
  duration_minutes: 11
  tasks_completed: 4
  files_modified: 5
  files_created: 0
  commits: 4
  completed_date: 2026-05-31
---

# Phase 19 Plan 01: Multi-Script Signing Production Bodies + Parity Tests Summary

Replaces the Phase 17 WALLET-02 `todo!()` placeholders in `shared::bip322::p2tr::sign` and `shared::bip322::p2sh_p2wpkh::sign` with production BIP-341 Schnorr keypath / BIP-143 ECDSA bodies (lifted near-verbatim from the existing `sign_for_tests` helpers per D-116), adds D-111 defense-in-depth spk↔key cross-checks at the top of each new body, exposes a `pub fn p2sh_p2wpkh_final_script_sig(pubkey) -> ScriptBuf` helper sibling to `sign_simple` (BIP-141 nested-SegWit scriptSig derivation), and ships 2 byte-equality parity tests proving `shared::bip322::sign_simple` produces witnesses byte-equal to `BdkClientWallet::sign_bip322` for both P2TR and P2SH-P2WPKH.

## Tasks Completed

| # | Task | Files | Commit |
|---|------|-------|--------|
| 1 | Ship `p2tr::sign` production body + D-111 cross-check | `shared/src/bip322/p2tr.rs` | `0b64e41` |
| 2 | Ship `p2sh_p2wpkh::sign` production body + D-111 + D-117 | `shared/src/bip322/p2sh_p2wpkh.rs` | `ffcfb9d` |
| 3 | Add `p2sh_p2wpkh_final_script_sig` helper + 3 unit tests | `shared/src/bip322/mod.rs` | `2d8c7f6` |
| 4 | Add 2 bdk-vs-shared byte-equality parity tests (+ Rule 3 wallet.rs fix) | `client/tests/wallet_sign_roundtrip.rs`, `client/src/wallet.rs` | `d1425fd` |

## What Changed

### `shared/src/bip322/p2tr.rs::sign` (Task 1)

- Removed `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")`.
- Body lifted from `sign_for_tests` (D-116): build `to_spend` → `to_sign` → `Keypair::from_secret_key` → `tap_tweak(None)` → `taproot_key_spend_signature_hash` (`SIGHASH_DEFAULT`) → `sign_schnorr_no_aux_rand` → push 64 bytes into `Witness`.
- D-111 cross-check inserted at the TOP (before any sighash work): derives `expected_spk = ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked())` and rejects with `Bip322Error::ScriptTypeMismatch { declared: detect_script_type(spk)?, derived: ScriptType::P2tr }` on mismatch.
- Determinism via `sign_schnorr_no_aux_rand` (D-114) — RESEARCH §Q1 confirms bdk_wallet 2.3 uses the same call, so byte-equality is the load-bearing closure for SC#1.
- Module doc-comment refreshed to reflect production-shipped state (no `todo!()` reference). Inline `// Plan 19-01 Task 1` provenance comment per CD-38.
- `sign_for_tests` REMAINS at this plan boundary (Plan 19-02 deletes).

### `shared/src/bip322/p2sh_p2wpkh.rs::sign` (Task 2)

- Removed `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")`.
- Body lifted from `sign_for_tests` (D-116): derive unwrapped P2WPKH SPK from compressed pubkey → BIP-143 sighash via `p2wpkh_signature_hash(0, &unwrapped_p2wpkh, Amount::ZERO, EcdsaSighashType::All)` → `sign_ecdsa` → DER + SIGHASH_ALL byte → push 33-byte compressed pubkey → 2-item witness.
- D-111 cross-check inserted at the TOP: derives `expected_spk = ScriptBuf::new_p2sh(&unwrapped_p2wpkh.script_hash())` and rejects with `Bip322Error::ScriptTypeMismatch { declared, derived: ScriptType::P2shP2wpkh }` on mismatch.
- **D-117**: `spk` is now load-bearing — `build_bip322_to_spend(spk, ...)` consumes the caller-supplied parameter directly (the cross-check above proves byte-equality). The prior `sign_for_tests` rebuilt the outer P2SH SPK from the key, silently ignoring its `_spk` argument. The footgun is gone in the production body.
- Sighash still uses the UNWRAPPED `unwrapped_p2wpkh` (BIP-143 structural — matches the bip322 crate's internal `verify_full_p2wpkh(is_p2sh=true)`).
- ECDSA via `sign_ecdsa` is RFC 6979 deterministic — byte-equality with bdk_wallet always holds (no aux-rand caveat).
- Module doc-comment refreshed. Inline `// Plan 19-01 Task 2` provenance comment per CD-38.
- `sign_for_tests` REMAINS (Plan 19-02 deletes).

### `shared/src/bip322/mod.rs` (Task 3)

**New helper** — `pub fn p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf`:

- Inserted as a sibling to `sign_simple` (between `sign_simple` and `sign_simple_test_only`).
- Per BIP-141 nested-SegWit: `scriptSig = OP_PUSHBYTES_22 <redeem>` where `redeem = OP_0 OP_PUSHBYTES_20 <HASH160(pubkey)>` (22 bytes); total scriptSig = 23 bytes.
- Infallible — takes a 33-byte compressed `secp256k1::PublicKey` so `bitcoin::PublicKey::new(_).wpubkey_hash()` is always `Some(_)`.
- Uses the codebase's `bitcoin::blockdata::script::Builder::new()` convention with `<&bitcoin::script::PushBytes>::try_from(redeem.as_bytes()).expect(...)` per CD-34 + RESEARCH §Q3 (the `&[u8]` → `&PushBytes` conversion is mandatory because variable-length slices don't auto-coerce; only fixed-size arrays do via the `from_array!` macro).

**Variant doc-comment update** (CD-36 default):

- `Bip322Error::ScriptTypeMismatch` doc-comment expanded to note the dual meaning per D-112 — original Phase 15 verify-side use AND Phase 19 sign-side reuse where `declared` = script type derived from `script_pubkey` arg, `derived` = script type derived from `SecretKey` arg. PII safety preserved (Display unchanged).

**Dispatcher doc-comment update** (CD-38 default):

- `sign_simple` doc-comment refreshed to remove the `todo!()` reference; all three per-script sign bodies now ship production code.

**3 new unit tests** in `tests` block:

1. `p2sh_p2wpkh_final_script_sig_derives_correctly` (D-108) — asserts `bytes.len() == 23` per RESEARCH §Q3 byte-count correction (CONTEXT D-110 said 24; correct count is 23). Asserts `bytes[0] == 0x16` (OP_PUSHBYTES_22), `bytes[1] == 0x00` (OP_0), `bytes[2] == 0x14` (OP_PUSHBYTES_20), `bytes[3..23] == HASH160(pubkey)`.
2. `p2tr_sign_rejects_p2sh_p2wpkh_spk_with_p2tr_key` (CD-37) — exercises D-111 P2TR cross-check; expects `ScriptTypeMismatch { declared: P2shP2wpkh, derived: P2tr }`.
3. `p2sh_p2wpkh_sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` (CD-37) — exercises D-111 P2SH-P2WPKH cross-check; expects `ScriptTypeMismatch { declared: P2tr, derived: P2shP2wpkh }`.

### `client/tests/wallet_sign_roundtrip.rs` (Task 4)

**2 new `#[tokio::test] async fn`s** added between `signed_proof_script_type_matches_wallet_script_type` and the non-tokio `dummy_outpoint_is_well_formed`:

1. `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` (D-118; T-19-C mitigation):
   - Builds a `tr({TEST_WIF})` single-key descriptor wallet on `Network::Regtest`.
   - Derives utxo_address inline from the same key (Note A in RESEARCH §Q2): tap_tweak with no merkle root → `Address::p2tr_tweaked`.
   - Defensive sanity: asserts `wallet.script_pubkey() == utxo_address.script_pubkey()` (catches bdk descriptor-parsing regressions).
   - Calls `wallet.sign_bip322(PARITY_TEST_MESSAGE)` → bdk_signed.
   - Calls `sign_simple(ScriptType::P2tr, &spk, &sk, msg)` → shared_witness.
   - **Asserts `bdk_signed.witness == shared_witness`** (byte-equality). Safe per RESEARCH §Q1 (both sides use `sign_schnorr_no_aux_rand`).
   - Belt-and-suspenders: asserts shared witness verifies under `verify_simple`.
2. `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` (D-119):
   - Analogous shape with `sh(wpkh({TEST_WIF}))` descriptor + `Address::p2sh(&redeem, Network::Regtest)` derivation.
   - ECDSA via RFC 6979 — byte-equality always holds.

Helper constants/functions added: `PARITY_TEST_MESSAGE`, `parity_secret_key()`, `parity_p2tr_address()`, `parity_p2sh_p2wpkh_address()`.

### `client/src/wallet.rs::from_descriptor` (Task 4 — Rule 3 deviation)

- Branched the bdk Wallet construction on the `/0/*)` derivation-template marker. Multi-key BIP-84/86/49 descriptors continue to use `Wallet::create(external, internal)` with `/1/*)` change-key derivation. **Single-key non-derivation descriptors** (e.g., `tr(<WIF>)`, `sh(wpkh(<WIF>))`) now route through `Wallet::create_single` — bdk_wallet 2.3 rejects `create(d, d)` with `"External and internal descriptors are the same"` for keychain-less descriptors, and `create_single` is the purpose-built API for this case (already used by `from_wif`).

## Verification Results

All v1.5 / v1.4 / v1.3 invariants green at the Plan 19-01 boundary:

| Suite | Pre | Post | Status |
|-------|-----|------|--------|
| `cargo test -p shared --lib bip322` | 14 | 17 | 17/17 (3 new from Task 3) |
| `cargo test -p shared --test per_script_vectors` | 7 | 7 | 7/7 (exercises production via test-only mirror) |
| `cargo test -p shared --test bip322_cross_shape` | 9 | 9 | 9/9 |
| `cargo test -p client --test wallet_sign_roundtrip` | 7 | 9 | 9/9 (2 new parity from Task 4) |
| `cargo test --test integration full_round` | 8 | 8 | 8/8 (v1.3 invariant) |
| `cargo test --test integration mixed_script_e2e` | 1 | 1 | 1/1 (v1.4 invariant) |
| `cargo test --test integration multi_script_validate` | 9 | 9 | 9/9 |
| `cargo build --workspace` | clean | clean | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean | clean | OK |

CI grep-gate tokens preserved:
- `CRIT-01` in `coordinator/src/bitcoin/utxo.rs`: 2 (unchanged)
- `CRIT-01` in `client/src/round/input.rs`: 2 (unchanged)
- `bip322 = "=0.0.10"` pin: unchanged (no Cargo.toml edits)

Plan-specific grep-gate checks:
- `grep -c 'todo!' shared/src/bip322/p2tr.rs` = 0 (was 2)
- `grep -c 'todo!' shared/src/bip322/p2sh_p2wpkh.rs` = 0 (was 2)
- `grep -c 'sign_schnorr_no_aux_rand' shared/src/bip322/p2tr.rs` = 4 (≥ 2 — body + sign_for_tests + 2 inline comments)
- `grep -c 'p2wpkh_signature_hash' shared/src/bip322/p2sh_p2wpkh.rs` = 2 (= sign + sign_for_tests)
- `grep -c 'ScriptTypeMismatch' shared/src/bip322/p2tr.rs` = 2 (D-111 + doc-link)
- `grep -c 'ScriptTypeMismatch' shared/src/bip322/p2sh_p2wpkh.rs` = 2 (D-111 + doc-link)
- `grep -c 'pub fn p2sh_p2wpkh_final_script_sig' shared/src/bip322/mod.rs` = 1
- `grep -c 'fn sign_for_tests' shared/src/bip322/{p2tr,p2sh_p2wpkh,p2wpkh}.rs` = 3 (all 3 still load-bearing at this boundary)
- `grep -c 'sign_simple_test_only' shared/src/bip322/mod.rs` ≥ 1 (still exists; Plan 19-02 deletes)
- `grep -c 'dbg!' shared/src/bip322/mod.rs` = 0 (smoke-check `dbg!` removed before commit per RESEARCH §Q3 caveat)

## Decisions Implemented

D-107, D-108, D-109, D-110 (with RESEARCH §Q3 byte-count correction noted), D-111, D-112, D-113, D-114, D-115 (RESEARCH §Q1 confirmed bdk_wallet 2.3 uses `sign_schnorr_no_aux_rand` — no downgrade), D-116, D-117, D-118, D-119, CD-34, CD-35, CD-36, CD-37, CD-38.

## Threat Model Items Mitigated

| Threat ID | Mitigation | Verification |
|-----------|------------|--------------|
| **T-19-A** (Spoofing/Tampering on sign-side) | D-111 spk-vs-key cross-check at TOP of `p2tr::sign` + `p2sh_p2wpkh::sign` — returns `Bip322Error::ScriptTypeMismatch` BEFORE any sighash work | Task 3 added 2 unit tests `p2tr_sign_rejects_p2sh_p2wpkh_spk_with_p2tr_key` + `p2sh_p2wpkh_sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` |
| **T-19-C** (Silent divergence bdk vs shared) | Task 4 added 2 byte-equality parity tests asserting `bdk_signed.witness == shared_witness` for same `(TEST_WIF-derived SecretKey, message)` | Both tests pass; safe per RESEARCH §Q1 (sign_schnorr_no_aux_rand) + ECDSA RFC 6979 determinism |
| **T-19-D** (PII leakage via error Display) | Per D-112 — reused existing PII-safe `Bip322Error::ScriptTypeMismatch` variant unchanged; Display interpolates only `ScriptType` enum values | Existing `bip322_error_display_does_not_leak_pii_substrings` test still green; CD-36 doc-comment update does not affect Display impl |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `BdkClientWallet::from_descriptor` fails on single-key WIF descriptors**

- **Found during:** Task 4 (first execution of `p2tr_shared_sign_matches_bdk_sign_byte_for_byte`).
- **Issue:** Plan + RESEARCH §Q2 specified `tr({TEST_WIF})` and `sh(wpkh({TEST_WIF}))` single-key WIF descriptors for the parity tests. RESEARCH verified bdk_wallet 2.3 accepts these descriptors AS DESCRIPTORS — but did not verify our `from_descriptor` wrapper. The wrapper calls `Wallet::create(external_desc, internal_desc)` where for single-key descriptors `internal == external` (no `/0/*` template path to swap to `/1/*)`). bdk_wallet 2.3 rejects this with `"External and internal descriptors are the same"` and the test panicked at the `expect(...)` on `from_descriptor`.
- **Fix:** Branched on the `/0/*)` derivation-template marker in `from_descriptor`. Multi-key BIP-84/86/49 descriptors continue to use `Wallet::create(external, internal)`. Single-key non-derivation descriptors now route through `Wallet::create_single` (the API `from_wif` already uses for `wpkh(WIF)`). This is a strict superset of prior behavior — every prior caller path goes through the `/0/*)` branch and behaves identically.
- **Files modified:** `client/src/wallet.rs` (inside `from_descriptor`).
- **Commit:** `d1425fd` (same commit as the parity tests it unblocks).
- **Scope:** within Task 4's purview — directly caused by the new test requiring a path the existing wrapper doesn't support. Did NOT touch from_wif or generate.
- **Verification:** v1.3 `full_round` 8/8 still green; v1.4 `mixed_script_e2e` 1/1 still green (those paths use multi-key descriptors with `/0/*)`, the unchanged code path).

### Task 1 Type Annotation Fix (mid-task, not separately committed)

During Task 3 the smoke test for `p2sh_p2wpkh_final_script_sig_derives_correctly` initially failed compilation with `E0283` on `expected_wpkh.as_ref()` — `WPubkeyHash` impls `AsRef<[u8]>`, `AsRef<[u8; 20]>`, AND `AsRef<PushBytes>`, so the compiler couldn't infer the target type. Fixed inline with `<WPubkeyHash as AsRef<[u8]>>::as_ref(&expected_wpkh)`. Not tracked as a separate deviation — pure mechanical typing.

## Notes for Plan 19-02

Carry-forward items at this plan boundary (PRESENT and load-bearing; Plan 19-02 deletes/migrates):

- `shared/src/bip322/mod.rs::sign_simple_test_only` (lines ~352-368) — `#[doc(hidden)] pub fn` test-only dispatcher mirror. Still consumed by `shared/tests/per_script_vectors.rs:274,311` and `tests/integration/multi_script_validate.rs:114-120`.
- `shared/src/bip322/p2tr.rs::sign_for_tests` (lines 60-95) — load-bearing for `sign_simple_test_only` routing.
- `shared/src/bip322/p2sh_p2wpkh.rs::sign_for_tests` (lines 68-108) — same.
- `shared/src/bip322/p2wpkh.rs::sign_for_tests` (lines 88-95) — unused alias with `#[allow(dead_code)]`; can be deleted directly.

Plan 19-02 will:
1. Delete `sign_simple_test_only` from `mod.rs`.
2. Delete `sign_for_tests` from `p2tr.rs`, `p2sh_p2wpkh.rs`, `p2wpkh.rs`.
3. Migrate `per_script_vectors.rs:274,311` callsites from `sign_simple_test_only` to `sign_simple` (now backed by production bodies shipped here).
4. Migrate `multi_script_validate.rs:23,114,120` callsites.
5. Refresh comments at `tests/integration/mod.rs:707,723`.

After Plan 19-02, the `shared::bip322` public surface shrinks to its final v1.5 shape with the V1.4-CRIT-01 dispatcher-only invariant load-bearing at the type level with NO test-only mirror.

## Self-Check: PASSED

Files verified to exist and contain expected content:
- `shared/src/bip322/p2tr.rs` — FOUND (production `sign` body; `todo!()` count = 0)
- `shared/src/bip322/p2sh_p2wpkh.rs` — FOUND (production `sign` body; `todo!()` count = 0)
- `shared/src/bip322/mod.rs` — FOUND (helper + 3 new tests; ScriptTypeMismatch doc-comment expanded)
- `client/tests/wallet_sign_roundtrip.rs` — FOUND (2 new parity tests; 9/9 pass)
- `client/src/wallet.rs` — FOUND (from_descriptor branches on `/0/*)`)

Commits verified to exist:
- `0b64e41`: feat(19-01): ship p2tr::sign production body with D-111 cross-check — FOUND
- `ffcfb9d`: feat(19-01): ship p2sh_p2wpkh::sign production body with D-111 + D-117 — FOUND
- `2d8c7f6`: feat(19-01): add p2sh_p2wpkh_final_script_sig helper + 3 unit tests — FOUND
- `d1425fd`: test(19-01): add bdk-vs-shared byte-equality parity tests (T-19-C) — FOUND
