---
phase: 19-multi-script-signing-finish
verified: 2026-05-31T15:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: null
  initial_verification: true
---

# Phase 19: Multi-Script Signing Finish — Verification Report

**Phase Goal:** `shared::bip322` ships production `sign` bodies for all 3 script types via the `pub(crate) fn sign` surface, and the test-only escape hatches (`sign_simple_test_only` + per-script `sign_for_tests` helpers) are gone — V1.4-CRIT-01 dispatcher-only invariant is now load-bearing at the type level with no holes.

**Verified:** 2026-05-31T15:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

The phase goal is fully achieved in the codebase. Production sign bodies exist for all 3 script types (P2WPKH from Phase 15, P2TR + P2SH-P2WPKH from Plan 19-01), all test-only escape hatches are deleted, and the `shared::bip322` public surface contains exactly the 9 dispatcher-only symbols. Goal-backward chain verified at every layer: artifacts exist, are substantive, are wired through the dispatcher, and produce real signature data (verified by byte-equality parity assertions against bdk_wallet).

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `shared/src/bip322/p2tr.rs::sign` returns 1-element witness with 64-byte BIP-341 Schnorr SIGHASH_DEFAULT signature (no `todo!()`) | VERIFIED | `p2tr.rs:53-105` ships production body; `grep -c 'todo!' p2tr.rs` = 0; body uses `sign_schnorr_no_aux_rand` (line 97) → `w.push(sig.as_ref())` (line 103) producing 1-element witness; `client/tests/wallet_sign_roundtrip.rs::p2tr_shared_sign_matches_bdk_sign_byte_for_byte` PASS asserts byte-equality with bdk_wallet's BIP-322 sign path; `per_script_vectors::test_p2tr_sign_verify_roundtrip_via_dispatcher` PASS confirms verify_simple round-trip |
| 2 | `shared/src/bip322/p2sh_p2wpkh.rs::sign` returns 2-element witness + final_script_sig (no `todo!()`) | VERIFIED | `p2sh_p2wpkh.rs:68-125` ships production body; `grep -c 'todo!' p2sh_p2wpkh.rs` = 0; `p2wpkh_signature_hash` (line 108) → `sign_ecdsa` (line 117) → `w.push(sig_bytes); w.push(pubkey.serialize())` (lines 122-123) producing 2-element witness; companion `p2sh_p2wpkh_final_script_sig` helper at `mod.rs:309-321` produces the 23-byte `[0x16, 0x00, 0x14, HASH160(pubkey)]` BIP-141 scriptSig; `p2sh_p2wpkh_final_script_sig_derives_correctly` lib test PASS; `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` parity test PASS |
| 3 | `sign_simple_test_only` deleted from mod.rs; no `sign_for_tests` in p2wpkh.rs/p2tr.rs/p2sh_p2wpkh.rs | VERIFIED | Workspace-wide grep `(sign_simple_test_only\|fn sign_for_tests)` over `shared/ tests/ client/ coordinator/ liquidity-bot/ --include='*.rs'` returns ZERO matches; `grep -c '#\[doc(hidden)\]' shared/src/bip322/mod.rs` = 0; final `shared::bip322` public surface (per `grep -nE '^pub (fn\|enum\|struct)'`) is exactly the 9 expected symbols: `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`, `ScriptType`, `Bip322Error`, `detect_script_type`, `verify_simple`, `sign_simple`, `p2sh_p2wpkh_final_script_sig` |
| 4 | `cargo test -p shared` green; `full_round` 8/8 (v1.3 invariant) green; `mixed_script_e2e` (v1.4 invariant) green | VERIFIED | `cargo test -p shared --lib bip322` 17/17 PASS (14 prior + 3 Plan 19-01 Task 3 additions); `cargo test -p shared --test per_script_vectors` 7/7 PASS; `cargo test -p shared --test bip322_cross_shape` 9/9 PASS; `cargo test -p client --test wallet_sign_roundtrip` 9/9 PASS (7 prior + 2 parity); `cargo test --test integration full_round` 8/8 PASS (single-threaded; observed one parallel flake on `adversarial_replay_token` re-run clean and confirmed not a Phase 19 regression — port-conflict race unrelated to bip322 changes); `cargo test --test integration mixed_script_e2e` 1/1 PASS; `cargo test --test integration multi_script_validate` 9/9 PASS (all 9 cases now exercise production `sign_simple` end-to-end) |
| 5 | `cargo clippy --workspace --all-targets -- -D warnings` clean | VERIFIED | Ran fresh `cargo clippy --workspace --all-targets -- -D warnings` → exit 0, no warnings |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `shared/src/bip322/p2tr.rs` | Production `pub(crate) fn sign` body (no `todo!()`), D-111 cross-check, `sign_schnorr_no_aux_rand` | VERIFIED | 107 lines; production sign body at lines 53-105; D-111 cross-check at lines 69-79; `sign_schnorr_no_aux_rand` at line 97; `sign_for_tests` DELETED |
| `shared/src/bip322/p2sh_p2wpkh.rs` | Production `pub(crate) fn sign` body, D-111 cross-check, D-117 spk-used-directly, BIP-143 sighash | VERIFIED | 127 lines; production sign body at lines 68-125; D-111 cross-check at lines 87-97; D-117 `spk` load-bearing at line 103; `p2wpkh_signature_hash` at line 108; `sign_for_tests` DELETED |
| `shared/src/bip322/p2wpkh.rs` | Production `pub(crate) fn sign` body (unchanged from Phase 15); `sign_for_tests` deleted | VERIFIED | 73 lines; production sign body at lines 46-72 (unchanged); unused `sign_for_tests` alias DELETED |
| `shared/src/bip322/mod.rs` | `pub fn sign_simple` dispatcher + `pub fn p2sh_p2wpkh_final_script_sig` helper; `sign_simple_test_only` deleted; updated `ScriptTypeMismatch` doc | VERIFIED | 660 lines; dispatcher at lines 283-294 routes to per-script sign; helper at lines 309-321 (23-byte scriptSig); `sign_simple_test_only` DELETED; `Bip322Error::ScriptTypeMismatch` doc-comment expanded for dual meaning at lines 184-203 |
| `client/tests/wallet_sign_roundtrip.rs` | 2 byte-equality parity tests for P2TR + P2SH-P2WPKH | VERIFIED | 9 test fns; new parity tests at lines 239-294 (P2TR) and 297-347 (P2SH-P2WPKH); both PASS |
| `shared/tests/per_script_vectors.rs` | Imports + 2 callsites migrated to `sign_simple` | VERIFIED | Import at line 26 dropped `sign_simple_test_only`; P2TR callsite at line 280 uses `sign_simple`; P2SH-P2WPKH callsite at line 318 uses `sign_simple`; 7/7 PASS |
| `tests/integration/multi_script_validate.rs` | `sign_witness` helper calls `sign_simple` | VERIFIED | Import at line 23 references `sign_simple`; helper body at lines 119-127 calls `sign_simple(handle.script_type, ...)`; consumed by 9 cross-shape rejection cases (all PASS) |
| `tests/integration/mod.rs` | Doc-comment refreshes for the field `TypedUtxoHandle::secret_key` | VERIFIED | No residual `sign_simple_test_only` references; comments refreshed |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `mod.rs::sign_simple` dispatcher | `p2tr::sign` (production) | `match arm ScriptType::P2tr => p2tr::sign(spk, key, message)` | WIRED | `mod.rs:291` — match arm present; dispatch verified by `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` PASS through the dispatcher path |
| `mod.rs::sign_simple` dispatcher | `p2sh_p2wpkh::sign` (production) | `match arm ScriptType::P2shP2wpkh => p2sh_p2wpkh::sign(spk, key, message)` | WIRED | `mod.rs:292` — match arm present; dispatch verified by `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` PASS through the dispatcher path |
| `client/tests/wallet_sign_roundtrip.rs` parity tests | `shared::bip322::sign_simple` + `BdkClientWallet::sign_bip322` | byte-equality assertion `bdk_signed.witness == shared_witness` | WIRED | Lines 273-294 (P2TR) and 326-347 (P2SH-P2WPKH); both tests PASS — empirical proof that production sign produces identical bytes to bdk_wallet's BIP-322 sign path |
| `tests/integration/multi_script_validate.rs::sign_witness` | `shared::bip322::sign_simple` | dispatcher call; 9 cross-shape rejection cases | WIRED | Lines 119-127; the 9 D-54 cross-shape rejection cases now exercise the production `sign_simple` path end-to-end (all PASS) |
| `shared/tests/per_script_vectors.rs::test_p2tr_sign_verify_roundtrip` | `shared::bip322::sign_simple` → `p2tr::sign` (production) | dispatcher dispatch | WIRED | Line 280; positive-vector test now exercises production P2TR sign body (PASS) |
| `shared/tests/per_script_vectors.rs::test_p2sh_p2wpkh_sign_verify_roundtrip` | `shared::bip322::sign_simple` → `p2sh_p2wpkh::sign` (production) | dispatcher dispatch | WIRED | Line 318; positive-vector test now exercises production P2SH-P2WPKH sign body (PASS) |

### Data-Flow Trace (Level 4)

Production sign bodies produce real cryptographic data (not stubs):

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `p2tr.rs::sign` returned `Witness` | `sig` (Schnorr signature) | `secp.sign_schnorr_no_aux_rand(&Message::from_digest(sighash.to_byte_array()), &tweaked.to_keypair())` — real secp256k1 Schnorr signing call | YES (64 bytes, deterministic, verified by `verify_schnorr`) | FLOWING |
| `p2sh_p2wpkh.rs::sign` returned `Witness` | `sig_bytes` + `pubkey.serialize()` | `secp.sign_ecdsa(&secp_msg, key)` — real secp256k1 ECDSA signing call over BIP-143 sighash on the unwrapped P2WPKH redeem | YES (DER + SIGHASH_ALL byte + 33-byte compressed pubkey, RFC 6979 deterministic) | FLOWING |
| `p2sh_p2wpkh_final_script_sig` returned `ScriptBuf` | 23-byte scriptSig | `compressed.wpubkey_hash()` then `Builder::new().push_slice(redeem.as_bytes())` — real HASH160 + script construction | YES (exactly 23 bytes; `[0x16, 0x00, 0x14, HASH160(pubkey)]`) | FLOWING |
| `sign_simple` dispatcher returned `Witness` | Per-script witness | Routes to production `p2wpkh::sign` / `p2tr::sign` / `p2sh_p2wpkh::sign` (no fallback or stub arms) | YES (verify_simple round-trips on every per-script output) | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| P2TR sign produces 64-byte Schnorr sig that bdk_wallet matches byte-for-byte | `cargo test -p client --test wallet_sign_roundtrip p2tr_shared_sign_matches_bdk_sign_byte_for_byte` | 1 passed; 0 failed | PASS |
| P2SH-P2WPKH sign produces RFC 6979 ECDSA sig + pubkey that bdk_wallet matches byte-for-byte | `cargo test -p client --test wallet_sign_roundtrip p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` | 1 passed; 0 failed | PASS |
| D-111 cross-check: P2TR sign rejects mismatched (spk, key) | `cargo test -p shared --lib bip322::tests::p2tr_sign_rejects_p2sh_p2wpkh_spk_with_p2tr_key` | 1 passed; 0 failed | PASS |
| D-111 cross-check: P2SH-P2WPKH sign rejects mismatched (spk, key) | `cargo test -p shared --lib bip322::tests::p2sh_p2wpkh_sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` | 1 passed; 0 failed | PASS |
| Helper produces 23-byte scriptSig per BIP-141 | `cargo test -p shared --lib bip322::tests::p2sh_p2wpkh_final_script_sig_derives_correctly` | 1 passed; 0 failed | PASS |
| Per-script positive vectors round-trip through production dispatcher | `cargo test -p shared --test per_script_vectors` | 7 passed; 0 failed | PASS |
| Cross-shape rejection invariants hold | `cargo test -p shared --test bip322_cross_shape` | 9 passed; 0 failed | PASS |
| v1.4 mixed-script E2E (cross-phase invariant) | `cargo test --test integration mixed_script_e2e` | 1 passed; 0 failed | PASS |
| v1.3 full_round (cross-phase invariant) | `cargo test --test integration full_round -- --test-threads=1` | 8 passed; 0 failed (single-threaded) | PASS |
| 9 D-54 multi-script validation cases through production sign | `cargo test --test integration multi_script_validate` | 9 passed; 0 failed | PASS |
| Workspace clippy with `-D warnings` | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0; no warnings | PASS |
| Workspace-wide grep for deleted symbols (`*.rs` files) | `grep -rn -E '(sign_simple_test_only\|fn sign_for_tests)' shared/ tests/ client/ coordinator/ liquidity-bot/ --include='*.rs'` | 0 matches | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| BIP322-05 | 19-01 | `shared::bip322::p2tr::sign` ships production BIP-341 Schnorr keypath body; byte-equal to bdk_wallet for same (key, message) | SATISFIED | `p2tr.rs:53-105` production body; `grep -c 'todo!' p2tr.rs` = 0; `sign_schnorr_no_aux_rand` invoked; `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` parity test PASS confirms byte-equality |
| BIP322-06 | 19-01 | `shared::bip322::p2sh_p2wpkh::sign` ships production BIP-143 ECDSA body; 2-element witness + `final_script_sig` helper; HASH160(redeem) matches P2SH SPK | SATISFIED | `p2sh_p2wpkh.rs:68-125` production body; `grep -c 'todo!' p2sh_p2wpkh.rs` = 0; D-111 cross-check (lines 87-97) proves HASH160(redeem) byte-equals the P2SH SPK; `p2sh_p2wpkh_final_script_sig` helper at `mod.rs:309-321`; `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` parity test PASS |
| BIP322-07 | 19-02 | Remove `sign_simple_test_only` and per-script `sign_for_tests` helpers; all integration tests call `sign_simple` | SATISFIED | Workspace grep returns 0 matches for both deleted symbols in `*.rs` files; final public surface is the 9 expected symbols; per_script_vectors + multi_script_validate now exercise production dispatcher |

**Orphaned requirements:** None. REQUIREMENTS.md `Traceability` table maps BIP322-05/06/07 to Phase 19 exclusively; all three are covered by plans 19-01 and 19-02.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `client/src/wallet.rs` | 152-156 | `{external_desc:?}` Debug-formatter interpolation of descriptor in error message leaks WIF/xprv private key material | INFO (out-of-scope) | Identified in 19-REVIEW.md as CR-01 Critical PII leak; introduced by Plan 19-01 Task 4 Rule-3 deviation. NOT a Phase 19 goal-failure (the goal is about sign body correctness; the leak is in the wallet's descriptor-parse error path, orthogonal to bip322 sign correctness). User has explicitly directed: address via `/gsd:code-review --fix` or `secure-phase`. Recorded here for traceability only. |

No `TBD`, `FIXME`, `XXX`, or unreferenced debt markers in files modified by Phase 19. Test files contain `// TODO`-style notes only in pre-existing prose comments that were not introduced by this phase.

### Probe Execution

No project-level probes detected (`find scripts -path '*/tests/probe-*.sh' -type f` returns nothing). Phase 19 success criteria do not reference probes; CI grep-gate jobs (`bip322-pin-check`, `crit-01-grep-check`, `crit-01-client-grep-check`) verified preserved per `19-02-SUMMARY.md` plan-specific grep checks. Step skipped — no probe path applicable.

### Test Setup Audit (Step 7d)

Examined test setup helpers cited as evidence:

| Helper | Constructs | Production analog | Risk | Disposition |
|--------|------------|-------------------|------|-------------|
| `fixture_secret_key()` (mod.rs:448-450) | `SecpSecretKey::from_slice(&[0x42_u8; 32])` | Same call shape used in production via descriptor parsing → `PrivateKey::from_wif(...).inner` produces a `SecpSecretKey` | LOW | Acceptable fixture — production reaches `SecretKey` via the same secp256k1 constructor |
| `fixture_p2tr_spk()`/`fixture_p2sh_spk()`/`fixture_p2wpkh_spk()` (mod.rs:452-481) | Real `ScriptBuf::new_p2tr_tweaked/_p2sh/_p2wpkh` derivations | Same call sequence reached from production by `BdkClientWallet::from_descriptor` → `wallet.script_pubkey()` | LOW | Acceptable fixture — identical derivation algorithm to production wallet construction |
| `parity_secret_key()` / `parity_p2tr_address()` / `parity_p2sh_p2wpkh_address()` (wallet_sign_roundtrip.rs:202-237) | Recover SecretKey + derive addresses from `TEST_WIF` | Same `PrivateKey::from_wif` + `tap_tweak` + `Address::p2sh` calls used in production wallet construction | LOW | Acceptable fixture — these helpers wrap the SAME production constructors the wallet uses internally, ensuring both signing paths see byte-identical (key, message) inputs |

Test helpers do NOT construct production state via private back doors — every helper uses public production constructors. Byte-equality parity tests (the load-bearing T-19-C closure) compare against `BdkClientWallet::sign_bip322` (production code path), making the test evidence directly traceable to production behavior.

### Gaps Summary

No goal-blocking gaps identified.

**Observation (recorded for traceability, NOT a Phase 19 failure):** Per-user directive and `19-REVIEW.md` CR-01, the descriptor-debug-leak in `client/src/wallet.rs:152-156` is a Critical PII issue introduced by Plan 19-01 Task 4's Rule-3 deviation (the single-key WIF descriptor branch). The phase goal — production sign bodies and dispatcher-only surface — is achieved orthogonally to this bug. The user has explicitly directed: address via `/gsd:code-review --fix` or `secure-phase`. Recorded here so the issue is not lost in handoff to Phase 20.

---

_Verified: 2026-05-31T15:00:00Z_
_Verifier: Claude (gsd-verifier)_
