# Phase 19: Multi-Script Signing Finish - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-30
**Phase:** 19-multi-script-signing-finish
**Areas discussed:** P2SH-P2WPKH sign API surface, Defense-in-depth on spk arg, P2TR Schnorr nonce strategy, Plan structure / sequencing

---

## P2SH-P2WPKH sign API surface

| Option | Description | Selected |
|--------|-------------|----------|
| Witness-only + helper | Keep `sign_simple(...) -> Result<Witness, Bip322Error>` unchanged; add `pub fn p2sh_p2wpkh_final_script_sig(pubkey) -> ScriptBuf` sibling helper. Treat SC#2's scriptSig clause as conceptual (BIP-322 verify only consumes Witness; client::wallet gets final_script_sig from bdk PSBT for descriptor path). | ✓ |
| Expand return type | Change `sign_simple` to return `Bip322ProofPieces { witness, script_sig: Option<ScriptBuf> }`. Mechanically cleaner type-level guarantee; touches client::wallet:512 + every test caller. | |
| Witness only, no helper | Keep current return type; callers derive scriptSig from pubkey themselves (5 lines). Loosest API; relies on documentation. | |

**User's choice:** Witness-only + helper
**Notes:** Aligns with verify_simple's consumption surface (witness-only) and avoids cascading the return-type change through client::wallet + all test callers. The helper covers the scriptSig clause of ROADMAP SC#2 at the BIP-141 derivation level.

### Follow-up: Helper site

| Option | Description | Selected |
|--------|-------------|----------|
| In `shared::bip322` | Sibling to `sign_simple`. Visibility `pub fn`. Name: `p2sh_p2wpkh_final_script_sig(pubkey)`. Keeps BIP-322 primitives in one place. | ✓ |
| Inside dispatcher | Add `pub fn final_script_sig(script_type, pubkey) -> Option<ScriptBuf>` returning None for P2WPKH/P2TR. Leaky abstraction. | |
| In client::wallet only | shared/ stays witness-only; client wallet (with bdk_wallet) is the only production caller that needs scriptSig. | |

**User's choice:** In `shared::bip322`

### Follow-up: Helper signature

| Option | Description | Selected |
|--------|-------------|----------|
| Take PublicKey | `fn p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf`. Lowest-privilege input (no secret material). | ✓ |
| Take SecretKey + derive | Tighter coupling with sign_simple call site, but passes secret material into a fn that doesn't need it — surface for accidental misuse. | |

**User's choice:** Take PublicKey

---

## Defense-in-depth on spk arg

| Option | Description | Selected |
|--------|-------------|----------|
| Cross-check, fail fast | Defense-in-depth. P2TR: derive tap-tweaked output key, compare to spk. P2SH-P2WPKH: derive HASH160(P2WPKH(pubkey)), compare to spk's script_hash. Return ScriptTypeMismatch on miss. Phase 21 audit charter can cite as structural mitigation. | ✓ |
| Silent trust | Current sign_for_tests for P2SH-P2WPKH ignores _spk and rebuilds from key. Silent if mismatched — verify will reject witness anyway. Lowest LOC. | |
| Cross-check P2SH-P2WPKH only | P2SH-P2WPKH alone has the silent-rebuild footgun; P2TR's spk mismatch is self-rejecting at verify time. | |

**User's choice:** Cross-check, fail fast
**Notes:** Sets up Phase 21 to describe the cross-check as structural, not "best-effort".

### Follow-up: Mismatch error variant

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse `ScriptTypeMismatch` | Existing variant. Semantic stretch (mismatch is key↔spk not declared↔derived script type) but no new variant means no test/match-arm churn at downstream callers. Document dual meaning inline. | ✓ |
| Add `KeyScriptMismatch` variant | New variant. Cleaner semantics; audit charter describes two distinct mismatch shapes. Adds 1 variant + Display + PII-safety test case. | |
| Reuse `UnrecognisedScriptPubkey` | Stretches existing variant the other way (about non-single-key SPKs). | |

**User's choice:** Reuse `ScriptTypeMismatch`

---

## P2TR Schnorr nonce strategy

| Option | Description | Selected |
|--------|-------------|----------|
| sign_schnorr_no_aux_rand | Deterministic (RFC 6979-style nonce). Required by BIP322-05 SC#1's byte-equality parity test. Matches current sign_for_tests. Matches BIP-340 §3.3. | ✓ |
| sign_schnorr (with aux-rand) | Aux-rand strengthens nonce against fault-attack adversary models. Breaks parity-test invariant (each call produces different bytes for same input). Would force parity test to assert on verify roundtrip rather than byte equality. | |

**User's choice:** sign_schnorr_no_aux_rand
**Notes:** Researcher must confirm bdk_wallet 2.3's BIP-322 sign path also uses deterministic Schnorr — if bdk emits aux-rand, parity test downgrades to verify-roundtrip assertion.

---

## Plan structure / sequencing

| Option | Description | Selected |
|--------|-------------|----------|
| 2 plans | P1 = production sign bodies (BIP322-05 + BIP322-06) + helper + cross-check + parity tests. P2 = remove sign_simple_test_only + sign_for_tests + migrate callers. P1's tests prove production sign works; P2 is cleanup against green baseline. | ✓ |
| 1 plan | Single plan: bodies + cleanup + tests. Atomic but larger diff at one commit boundary. | |
| 3 plans | P1 = p2tr::sign + parity. P2 = p2sh_p2wpkh::sign + helper. P3 = BIP322-07 cleanup. Most granular blast-radius but extra overhead. | |
| Defer to plan-phase | Plan-phase decides count. | |

**User's choice:** 2 plans

### Follow-up: Parity test location

| Option | Description | Selected |
|--------|-------------|----------|
| client/tests/ | `client/tests/wallet_sign_roundtrip.rs` already exists and exercises BdkClientWallet::sign_bip322 across all 3 script types. Avoids forcing shared/ to take bdk_wallet dev-dep (would conflict with no-bdk-in-shared CD-6). | ✓ |
| tests/integration/ | Workspace integration tests have both crates. But Phase 19 changes don't need bitcoind — adds bitcoind-skip surface for no benefit. | |
| shared/tests/ with frozen byte vector | Pinned `expected_bdk_witness.hex` fixture. Decouples test from bdk_wallet, but fixture is hand-generated and drifts if bdk-wallet's signing changes. | |

**User's choice:** client/tests/

---

## Claude's Discretion

Plan-phase has discretion on (per CONTEXT.md §"Claude's Discretion"):
- CD-34: Exact rust-bitcoin builder API for the `p2sh_p2wpkh_final_script_sig` helper body (`Builder::new()` vs `ScriptBuf::builder()`).
- CD-35: Exact names of the parity test functions; whether to consolidate into a parameterised test fn.
- CD-36: Doc-comment update for the reused `Bip322Error::ScriptTypeMismatch` variant vs inline sign-site notes.
- CD-37: Whether to add `#[test] fn sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` unit tests pinning the D-111 cross-check behavior. Default: yes.
- CD-38: Inline `// Plan 19-01 Task N` comments at the per-script `sign` body summary vs tracking in 19-01-PLAN.md only.
- CD-39: Whether Plan 19-02 includes the `tests/integration/mod.rs:707,723` comment refreshes. Default: yes.

## Deferred Ideas

- P2WPKH spk↔key cross-check for symmetry with P2TR + P2SH-P2WPKH (v1.6+ if audit-charter review flags asymmetry).
- Adding a dedicated `Bip322Error::KeyScriptMismatch` variant instead of reusing `ScriptTypeMismatch` (v1.6+ if the dual-meaning reuse is flagged as a documentation smell).
- Parameterised parity test consolidating both script types (v1.6+ — current per-fn style of wallet_sign_roundtrip.rs wins on consistency).
- `p2sh_p2wpkh_final_script_sig` returning `Result<ScriptBuf, Bip322Error>` (v1.6+ if a future caller passes uncompressed pubkeys; currently infallible because we take `&secp256k1::PublicKey` which is always 33-byte compressed).
- Shared `derive_p2tr_spk(key)` / `derive_p2sh_p2wpkh_spk(key)` helpers exposed at module level (v1.6+ if other callers need the derivation independently).
