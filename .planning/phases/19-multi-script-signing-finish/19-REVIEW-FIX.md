---
phase: 19-multi-script-signing-finish
fixed_at: 2026-05-31T13:50:00Z
review_path: .planning/phases/19-multi-script-signing-finish/19-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 6
skipped: 0
status: all_fixed
---

# Phase 19: Code Review Fix Report

**Fixed at:** 2026-05-31T13:50:00Z
**Source review:** `.planning/phases/19-multi-script-signing-finish/19-REVIEW.md`
**Iteration:** 1

**Summary:**

- Findings in scope: 6 (1 Critical + 5 Warning; Info findings out of scope per default `--fix` policy)
- Fixed: 6
- Skipped: 0

**Verification:**

- `cargo test --workspace --lib` — all 149 lib tests pass (28 client + 77 coordinator + 10 liquidity-bot + 34 shared)
- `cargo test -p client --tests` — all 65 tests pass (28 lib + 28 integration cfg + 9 wallet_sign_roundtrip)
- `cargo test -p shared --tests` — all 56 tests pass (34 lib + 9 cross_shape + 6 ownership_proof_roundtrip + 7 per_script_vectors)
- `cargo clippy --workspace --all-targets -- -D warnings` — clean (no warnings)

## Fixed Issues

### CR-01: PII leak — full descriptor (with private key material) printed in unrecognised-wrapper error

**Files modified:** `client/src/wallet.rs`
**Commit:** d9e74c6
**Applied fix:** Replaced the `{external_desc:?}` interpolation on the unrecognised-wrapper branch with a prefix-only message (10-char prefix + length). Also redacted both `bdk_wallet::Wallet::create*` parse-error pass-throughs (the `.map_err(|e| anyhow!("...: {e}"))` form) to swallow the inner error rather than risk leaking the descriptor body through bdk's miniscript Display chain. WR-02 (which the review explicitly noted overlapped with CR-01's lines 176/181) is covered by the same edit and listed separately below for traceability.

### WR-01: `detect_script_type(spk)?` in cross-check can mask the real failure with `UnsupportedScriptType`

**Files modified:** `shared/src/bip322/p2tr.rs`, `shared/src/bip322/p2sh_p2wpkh.rs`
**Commit:** 2e68d14
**Applied fix:** Changed the D-111 cross-check's `declared` field to use `detect_script_type(spk).unwrap_or(<caller-invoked variant>)` instead of the `?` propagation form. A caller-invoked sign with a non-standard SPK (P2WSH, OP_RETURN, bare multisig, etc.) now produces `Bip322Error::ScriptTypeMismatch` — matching the dispatcher rustdoc's promise — instead of leaking `UnsupportedScriptType`. Existing `p2tr_sign_rejects_p2sh_p2wpkh_spk_with_p2tr_key` and `p2sh_p2wpkh_sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` tests still pass unchanged (those exercise the in-trio detect path, which `unwrap_or` does not affect).

### WR-02: bdk_wallet descriptor parse-error pass-through may leak key material

**Files modified:** `client/src/wallet.rs`
**Commit:** d9e74c6 (bundled with CR-01 per review's explicit scoping)
**Applied fix:** Both `Wallet::create(...).create_wallet_no_persist()` and `Wallet::create_single(...).create_wallet_no_persist()` paths now use `.map_err(|_| anyhow!("...parse error suppressed to avoid leaking key material..."))?` — the inner `e` is discarded so bdk's underlying miniscript / descriptor `Display` output (which has historically embedded parts of the offending descriptor body) cannot escape through the error chain. The REVIEW.md explicitly named CR-01's lines 176/181 (the bdk pass-through sites) as a "second leak vector for the same data", so the redaction was applied as part of the CR-01 commit rather than a separate commit.

### WR-03: Fragile descriptor-template detection via `contains("/0/*)")`

**Files modified:** `client/src/wallet.rs`
**Commit:** b698190
**Applied fix:** Added two explicit fail-fast guards in `from_descriptor` before the `contains("/0/*)")` template-detection branch:

1. `external_desc.contains('#')` → reject with "descriptor checksums (`#…`) are not supported" — prevents the silent checksum-mismatch failure path where `replacen` produces an internal descriptor whose checksum no longer matches the body.
2. `external_desc.contains('<') && external_desc.contains('>')` → reject with "BIP-389 multi-path descriptors (`<a;b>`) are not supported" — prevents the silent fall-through to `Wallet::create_single` that would drop the user-encoded change keychain semantics.

Did NOT change the underlying `contains("/0/*)")` matcher itself, because the user's review-guidance offered three alternatives and option (b) — fail fast on the unsupported shapes — is the lowest-risk fix that closes the cited footguns without disturbing the working bdk_wallet single-wrapper vs `create_single` dispatch logic. Test `from_descriptor_rejects_p2tr_flag_with_wpkh_descriptor` still passes; the 9 `wallet_sign_roundtrip.rs` tests (which use WIF descriptors with no `#` or `<>`) still pass.

### WR-04: Floating-point BTC→sats conversion can lose precision

**Files modified:** `tests/integration/mod.rs`
**Commit:** 5843691
**Applied fix:** Replaced both `(out.value * 100_000_000.0).round() as u64` casts in `fund_regtest` (line 655) and `fund_regtest_typed` (line 943) with `bitcoin::Amount::from_btc(out.value).unwrap_or_else(|_| panic!(...)).to_sat()`. The rust-bitcoin helper performs the multiplication with explicit overflow / non-finite / out-of-range checks (the prior `as u64` would silently saturate above `u64::MAX` and produce 0 for NaN). `Amount` is already imported in both functions so no new use was required. Out-of-scope (not cited in REVIEW.md): the same pattern at `full_round.rs:225`, `full_round.rs:1444`, and `mixed_script_e2e.rs:406` — left for a follow-up cleanup pass to keep the commit scope tight.

### WR-05: WIF stored as `String` for the wallet's lifetime — no zeroize on drop

**Files modified:** `client/src/wallet.rs`
**Commit:** 6a7d3bf
**Applied fix:** Review's Option B — wrapped `BdkClientWallet::wif_key` as `Option<zeroize::Zeroizing<String>>`. The `zeroize` crate is already declared in `client/Cargo.toml` (workspace dep) so no Cargo.toml change was needed. `Zeroizing<String>` has a `Drop` impl that zeroes the heap buffer when dropped, so the WIF no longer survives in freed-but-uncleared heap pages where a later allocation in the same process could read it ("heap reuse" leak). Touched the `from_wif` constructor (`Some(Zeroizing::new(wif.to_string()))`) and `secret_key_for_signing` (`as_ref().map(|z| z.as_str())` because `.as_deref()` does not Deref-chain through the double wrap automatically). The two `wif_key: None` initialisers in `from_descriptor` and `generate` did not need changes — `None` is type-compatible with the new `Option<Zeroizing<String>>`.

---

_Fixed: 2026-05-31T13:50:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
