---
phase: 19-multi-script-signing-finish
reviewed: 2026-05-31T13:30:00Z
depth: standard
files_reviewed: 9
files_reviewed_list:
  - client/src/wallet.rs
  - client/tests/wallet_sign_roundtrip.rs
  - shared/src/bip322/mod.rs
  - shared/src/bip322/p2sh_p2wpkh.rs
  - shared/src/bip322/p2tr.rs
  - shared/src/bip322/p2wpkh.rs
  - shared/tests/per_script_vectors.rs
  - tests/integration/mod.rs
  - tests/integration/multi_script_validate.rs
findings:
  critical: 1
  warning: 5
  info: 4
  total: 10
status: issues_found
---

# Phase 19: Code Review Report

**Reviewed:** 2026-05-31T13:30:00Z
**Depth:** standard
**Files Reviewed:** 9
**Status:** issues_found

## Summary

Phase 19 ships the production `sign` bodies for P2TR (BIP-341 Schnorr keypath, SIGHASH_DEFAULT) and P2SH-P2WPKH (BIP-143 ECDSA over the unwrapped P2WPKH redeem) in `shared/src/bip322/{p2tr,p2sh_p2wpkh}.rs`, replacing the prior `todo!()` placeholders. The cryptographic core is **correct**: sighashes are computed over the right script context per spec, the witness-arity preflights match the bip322 crate's expectations, and the D-111 spk↔key cross-check is structurally sound. The byte-equality parity tests in `client/tests/wallet_sign_roundtrip.rs` empirically pin bdk-vs-shared agreement.

That said, the review surfaces:

1. **One CRITICAL PII/key-leak bug** in `client/src/wallet.rs::from_descriptor` (CR-01): a fail-fast `anyhow!()` error embeds the **full descriptor with `{:?}` Debug formatting**, which for `tr(<WIF>)` or `wpkh(xprv...)` will print the raw private key material into the user-visible error stream. This contradicts CLAUDE.md's "No PII logging" project constraint, applies to BOTH the new single-key-WIF descriptor path AND the existing xprv path, and is reachable by any user with a typo'd descriptor prefix.

2. **Several WARNINGs** around the cross-check semantics (the `detect_script_type(spk)?` early-return semantics can produce a misleading error variant for SPK shapes outside the P2WPKH/P2TR/P2SH trio), the fragile string-based descriptor-template detection (`contains("/0/*)")` would silently misclassify a checksummed descriptor), the floating-point BTC→sats conversion in `mod.rs::fund_regtest{,_typed}`, and a couple of bdk_wallet error pass-throughs that may also leak descriptor strings.

3. **INFO items** on stylistic / discoverability points (unused public helper, magic `0.5 BTC fee margin`, etc.).

## Structural Findings (fallow)

_No `<structural_findings>` block provided in the review prompt. Narrative findings below stand on their own._

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: PII leak — full descriptor (with private key material) printed in unrecognised-wrapper error

**File:** `client/src/wallet.rs:152-156`
**Issue:**
```rust
} else {
    return Err(anyhow!(
        "descriptor wrapper not recognised: expected `wpkh(...)`, `tr(...)`, or `sh(wpkh(...))` (got: {external_desc:?})"
    ));
};
```

The error message interpolates `external_desc` via `{:?}` Debug formatting. For any of the supported descriptor inputs this string contains **master private key material**: a WIF (e.g. `tr(cVt4o7BG...)`) or an xprv (e.g. `wpkh(tprv8ZgxM.../84'/0'/0'/0/*)`). When a user supplies a typo'd or unknown wrapper (e.g. `tap(...)`, ` wpkh(...)` with leading whitespace, `wsh(...)`), this error propagates up the CLI through `anyhow`'s default `Display` → stderr, leaking the entire xprv or WIF to:

- The terminal scrollback
- Any shell-recording tool (script, asciinema)
- Any log-aggregation tool the user has hooked into stderr
- The `RUST_BACKTRACE=1` panic trail if a caller chooses to `unwrap()`

This directly violates the CLAUDE.md project constraint **"No PII logging; round state zeroed after broadcast"** and the broader "Privacy and PII safety are paramount" stance — a leaked xprv is unrecoverable theft if the user ever funds the derivation.

The bug is **also reachable** through bdk_wallet's parse-error pass-through on lines 176 and 181 (`anyhow!("Failed to create bdk wallet from descriptor: {e}")`) when bdk's underlying miniscript parser includes the descriptor body in its error `Display`. That's a second leak vector for the same data.

**Fix:**

Print only the prefix and length, never the body. The prefix is sufficient to diagnose a typo:

```rust
} else {
    // CR-01: do NOT interpolate the full descriptor — it carries master
    // private key material (xprv / WIF). Print only the leading non-key
    // tokens so a user-side typo is diagnosable without leaking key bytes.
    let prefix: String = external_desc
        .chars()
        .take(10)
        .collect();
    return Err(anyhow!(
        "descriptor wrapper not recognised: expected `wpkh(...)`, `tr(...)`, or `sh(wpkh(...))` (prefix: {prefix:?}, len: {})",
        external_desc.len()
    ));
};
```

Apply the same redaction discipline to the bdk_wallet error pass-throughs on lines 176 and 181 — wrap the error so the descriptor body cannot escape through the error chain. One option:

```rust
.map_err(|_e| anyhow!("Failed to create bdk wallet from descriptor (descriptor body redacted to avoid leaking key material)"))?
```

A complementary defensive step: add a `#[test]` that asserts the error `Display` for a known WIF-bearing descriptor does NOT contain the WIF's characteristic alphabet substring — this gates regressions at CI time.

---

## Warnings

### WR-01: `detect_script_type(spk)?` in cross-check can mask the real failure with `UnsupportedScriptType`

**File:** `shared/src/bip322/p2tr.rs:75-79` and `shared/src/bip322/p2sh_p2wpkh.rs:93-97`
**Issue:**

The D-111 cross-check uses `super::detect_script_type(spk)?` to populate the `declared` field of `ScriptTypeMismatch`. If the caller passes an SPK that is **none of** P2WPKH/P2TR/P2SH (e.g. P2WSH, OP_RETURN, bare multisig), `detect_script_type` returns `UnsupportedScriptType` and the `?` propagates that variant — the caller sees `UnsupportedScriptType`, NOT `ScriptTypeMismatch`. The user expectation set by the dispatcher's doc comment is "a mismatch returns `Bip322Error::ScriptTypeMismatch` BEFORE any sighash work", which isn't quite true: it returns `UnsupportedScriptType` for unrecognised SPK shapes.

This isn't a correctness defect (signing against an OP_RETURN SPK *is* unsupported, and the verifier would reject anyway), but it's a debuggability defect — a caller troubleshooting "why did sign_simple(P2tr, ...) reject my SPK" will see two different error variants depending on whether their SPK is a P2WPKH/P2SH (→ `ScriptTypeMismatch`) or a P2WSH (→ `UnsupportedScriptType`). The doc comment promises the former.

**Fix:** Either (a) update the doc comment to enumerate both outcomes explicitly, or (b) collapse the unsupported-SPK case into `ScriptTypeMismatch` by giving `detect_script_type` an infallible-on-best-effort alternative used only for the `declared` field:

```rust
// Best-effort classifier for the cross-check's `declared` field. Falls
// back to the variant the caller invoked (here: P2TR) so a non-standard
// SPK still produces a ScriptTypeMismatch with semantically sensible
// fields, not a surprising UnsupportedScriptType.
let declared = super::detect_script_type(spk).unwrap_or(super::ScriptType::P2tr);
if expected_spk.as_script() != spk {
    return Err(super::Bip322Error::ScriptTypeMismatch {
        declared,
        derived: super::ScriptType::P2tr,
    });
}
```

Pick whichever option matches the team's design intent — but pick one. The current behaviour is inconsistent with the doc comment.

---

### WR-02: bdk_wallet descriptor parse-error pass-through may leak key material

**File:** `client/src/wallet.rs:173-181`
**Issue:**

```rust
Wallet::create(external_desc.to_string(), internal_desc)
    .network(bdk_net)
    .create_wallet_no_persist()
    .map_err(|e| anyhow!("Failed to create bdk wallet from descriptor: {e}"))?
```

`bdk_wallet`'s underlying `miniscript::Error` and `descriptor::Error` Display implementations have historically been verbose and have, in some upstream versions, included parts of the offending descriptor string in their messages (the descriptor body is what's being parsed and reported on). When that body contains an xprv or WIF, the propagated `{e}` interpolation leaks it through the same channel as CR-01.

This is a softer concern than CR-01 because (a) it only triggers on a `bdk_wallet::create_wallet_no_persist` failure (a rare path on the happy flow), and (b) the exact leak surface depends on bdk_wallet's error formatting at the pinned version (2.3.x per CLAUDE.md). But the codebase has no upper-bound test today on what those errors say, and a bdk-version bump could silently widen the leak.

**Fix:**

Wrap the error so the descriptor body cannot escape:

```rust
.map_err(|_| anyhow!(
    "Failed to create bdk wallet from descriptor \
     (parse error suppressed to avoid leaking key material; \
     check that the descriptor has a valid wrapper and key)"
))?
```

Pair this with a `#[test]` that supplies an intentionally malformed `tr(<WIF>` (missing close paren) and asserts the error `Display` does not contain the leading 4 characters of the WIF — this is a cheap CI gate.

---

### WR-03: Fragile descriptor-template detection via `contains("/0/*)")`

**File:** `client/src/wallet.rs:171`
**Issue:**

```rust
let inner = if external_desc.contains("/0/*)") {
    let internal_desc = external_desc.replacen("/0/*)", "/1/*)", 1);
    Wallet::create(external_desc.to_string(), internal_desc)...
} else {
    Wallet::create_single(external_desc.to_string())...
};
```

The `contains("/0/*)")` heuristic is brittle:

1. **Descriptors with checksums fail silently.** A descriptor of the form `wpkh(xprv.../84'/0'/0'/0/*)#abc12345` *does* contain `/0/*)` (followed by `#…`), and `replacen` will produce `wpkh(xprv.../84'/0'/0'/1/*)#abc12345`. But the checksum is now wrong for the new keychain, so bdk_wallet will reject the internal descriptor with a checksum-mismatch error — and the user sees a confusing "Failed to create bdk wallet from descriptor" rather than a "checksummed descriptors not supported" message.

2. **Non-standard derivation paths are missed.** A descriptor like `wpkh(xprv.../84'/0'/0'/0/0)` (single fixed child instead of `/0/*`) lacks the literal `/0/*)` substring, so it falls into the single-key branch and is constructed via `Wallet::create_single`. That's probably fine on bdk's side but is silently inconsistent with what the user wrote.

3. **Multi-path BIP-389 descriptors collide.** A descriptor like `wpkh(xprv.../<0;1>/*)` (combined external+internal) is a real BIP-389 form that bdk supports; it doesn't match `/0/*)` so it would route to `Wallet::create_single`, dropping the change keychain semantics.

**Fix:**

Replace the string-match with a structural check. Two options:

a) Make the path explicit at the API boundary: accept an `Option<&str> internal_desc` parameter in `from_descriptor` (CLI passes it; tests pass `None`). This is the canonical bdk_wallet idiom.

b) If you must auto-derive, use a tighter regex that anchors to a known-good template (`(/0/\*\)|/0/\*\))` and explicitly reject descriptors with checksums (`#`), multi-path (`<…>`), or non-`/0/*` keychain templates — fail fast with a clear message rather than producing a silently-wrong internal descriptor.

At minimum, document the constraint in the `from_descriptor` rustdoc explicitly: "descriptors must end with `/0/*)` or `/0/*))` (the latter for `sh(wpkh)`); checksums and multi-path descriptors are NOT supported".

---

### WR-04: Floating-point BTC→sats conversion can lose precision

**File:** `tests/integration/mod.rs:655` and `tests/integration/mod.rs:943`
**Issue:**

```rust
let value_sats = (out.value * 100_000_000.0).round() as u64;
```

Multiplying an `f64` BTC value by `100_000_000.0` is fine for the small denominations used in these tests (max ~150_000 sats), but the pattern is a footgun for future test additions:

- `f64` can exactly represent BTC values whose sat count fits in 53 bits, which covers all amounts up to ~90,071,992 BTC — so for any realistic test value the cast is safe.
- BUT the deserialized `out.value: f64` comes from JSON parsing of `corepc-types`'s `bitcoin_json::ResolvedTransactionOutput::value`. If a future Bitcoin Core release returns the value with a trailing precision artifact (e.g. `0.00150000000000001`), the `.round()` will save you, but `.round() as u64` of a value just below `u64::MAX` would overflow silently.

This is integration-test-only code and the inputs are coordinator-controlled, so the actual risk is low. But the `(f64 * 100_000_000.0).round() as u64` pattern is a code smell when the source library (`corepc-types`/`bitcoincore-rpc`) usually offers a `Amount`-typed accessor that bypasses the float entirely.

**Fix:**

Prefer `corepc-types`'s typed sat accessor if available, or use `bitcoin::Amount::from_btc(out.value).expect("...").to_sat()` (the rust-bitcoin idiom — it does the multiplication with overflow checks). If neither is available in this version, at least add a saturating cast:

```rust
let value_sats = (out.value * 100_000_000.0).round();
assert!(value_sats >= 0.0 && value_sats <= u64::MAX as f64,
    "BTC value out of u64 sat range: {}", out.value);
let value_sats = value_sats as u64;
```

---

### WR-05: WIF stored as `String` for the wallet's lifetime — no zeroize on drop

**File:** `client/src/wallet.rs:57, 115, 345-352`
**Issue:**

```rust
pub struct BdkClientWallet {
    ...
    /// The WIF key string, stored for secret_key_for_signing (WIF wallets only).
    wif_key: Option<String>,
    ...
}
```

The WIF is stored as a plain `String` for the wallet's entire lifetime, and is also handed back as a fresh `SecretKey` on every call to `secret_key_for_signing()` (which re-parses the WIF, leaving the original `String` heap allocation in place). On drop, the `String`'s heap allocation is freed without being zeroed — so the WIF lives in freed-but-uncleared heap pages until the allocator reuses them. A subsequent allocation in the same process can read those pages (the classic "heap reuse" leak).

This is a sharp edge given the project's "Privacy and PII safety are paramount" stance from CLAUDE.md and the user-facing `WARNING: MASTER PRIVATE KEY MATERIAL` banner on line 276. The mnemonic, xprv, and WIF should ideally all live in a `zeroize::Zeroizing<String>` or equivalent.

This is a long-standing issue (not new in Phase 19) but is in scope for review because Phase 19's `secret_key_for_signing` keeps the same `wif_key.as_deref()` access pattern; the new parity tests in `wallet_sign_roundtrip.rs::parity_secret_key()` also do `PrivateKey::from_wif(TEST_WIF).inner` and discard the `PrivateKey` without zeroing.

**Fix:**

Three options, in increasing order of effort:

a) **Minimal** — drop the WIF storage entirely. The `SecretKey` is recoverable from bdk_wallet's keymap, and `from_wif` could store the `SecretKey` directly (or a `Zeroizing<[u8; 32]>` of its bytes) instead of the WIF `String`. This is a localised refactor.

b) **Defensive** — wrap `wif_key` as `Option<zeroize::Zeroizing<String>>`. Add `zeroize = "1"` to `client/Cargo.toml`. This is one struct-field change plus a couple of `Zeroizing::new(...)` call-site changes.

c) **Most thorough** — also `impl Drop for BdkClientWallet` that explicitly zeros the `wif_key` buffer. Belt-and-suspenders on top of (b).

The integration tests' use of WIFs (`TEST_WIF` constants) is a separate, smaller concern — those are public-domain regtest WIFs with no monetary value, so the priority there is documentation, not zeroize.

---

## Info

### IN-01: `p2sh_p2wpkh_final_script_sig` has no production callers

**File:** `shared/src/bip322/mod.rs:309-321`
**Issue:**

The new pub helper `p2sh_p2wpkh_final_script_sig` is exported from the dispatcher module but is only used by its own unit test (`p2sh_p2wpkh_final_script_sig_derives_correctly`) — `grep -rn` finds no production callers in `client/`, `coordinator/`, or `tests/integration/`. The Phase 19 D-109 doc on lines 297-308 names it a "sibling to `sign_simple`", but the actual P2SH-P2WPKH sign path in `client/src/wallet.rs::sign_bip322` extracts `final_script_sig` from bdk_wallet's PSBT finalization (line 596-598), not via this helper.

If the helper is intended to ship for future production use (e.g. for the upcoming v=2 wire envelope's `final_script_sig` carry), the doc should say so. If it's vestigial from a design that was superseded by the bdk_wallet PSBT-finalize route, it should either be removed or downgraded to `pub(crate)` so it can't drift into production via an unintended caller.

**Fix:**

Either name the future production caller in the rustdoc (gives the reviewer confidence the function is load-bearing), or downgrade visibility to `pub(crate)` until a real caller materializes. Removing it outright is also fine since the BIP-141 nested-SegWit recipe is one-liner inline code at any future callsite.

---

### IN-02: Magic number `50_000` fee margin appears in two helpers

**File:** `tests/integration/mod.rs:577` and `tests/integration/mod.rs:797`
**Issue:**

```rust
let fund_sats: u64 = denomination + 50_000; // covers denomination + fee margin
```

The `50_000` sat fee margin is duplicated between `fund_regtest` and `fund_regtest_typed` (both helpers). A future tweak to the denomination/fee balance would need to edit both sites in lockstep; the second site copies the first verbatim. This is minor since the helpers are clearly siblings, but the `denomination`+`fund_sats` pair would benefit from a single module-level `const FUND_PER_UTXO_MARGIN: u64 = 50_000;`.

**Fix:**

Promote to a module-level constant:

```rust
/// Fee margin added to each funded test UTXO so the round's per-input
/// fee_share fits inside the input value without bumping the funded amount
/// up to a round-number BTC denomination.
const FUND_PER_UTXO_FEE_MARGIN_SATS: u64 = 50_000;
```

…then both helpers reference it. Cosmetic but improves a small refactor surface.

---

### IN-03: Commented-out / under-justified `dangerous_assume_tweaked` calls

**File:** `client/tests/wallet_sign_roundtrip.rs:219-222`, `shared/src/bip322/mod.rs:469`, and `shared/src/bip322/p2tr.rs:73`
**Issue:**

`dangerous_assume_tweaked` is a rust-bitcoin API surface specifically named "dangerous" because the caller is asserting that the x-only key it's wrapping has already been BIP-341-tweaked. In the Phase 19 codebase, all callers DO indeed apply the BIP-341 tweak via `keypair.tap_tweak(&secp, None)` immediately before, so the use is correct. But the doc comments at the use sites don't repeat the "I have applied tap_tweak in the line above" justification, which makes a future cleanup pass (e.g. dropping the `tap_tweak` for some refactor reason) more likely to leave behind a silent untweaked-key bug.

**Fix:**

One-line comment at each `dangerous_assume_tweaked` site:

```rust
// Safe: `tweaked_xonly` is the output of keypair.tap_tweak(&secp, None)
// on the line(s) above; the BIP-341 tweak has been applied.
let spk = ScriptBuf::new_p2tr_tweaked(tweaked_xonly.dangerous_assume_tweaked());
```

This is the kind of safety-comment discipline that catches itself the moment someone tries to delete the `tap_tweak`.

---

### IN-04: `XOnlyPublicKey::from_keypair` result discarded with `_ = xonly;`

**File:** `shared/src/bip322/mod.rs:464-468`, `shared/tests/per_script_vectors.rs:267, 271`, and `tests/integration/mod.rs:859`
**Issue:**

```rust
let (xonly, _parity) = XOnlyPublicKey::from_keypair(&keypair);
let tweaked = keypair.tap_tweak(&secp, None);
let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
let _ = xonly; // suppress unused (sanity: untweaked key derivable too)
```

`xonly` is computed and then deliberately discarded via `let _ = xonly`. The comment "sanity: untweaked key derivable too" is doing meaningful documentation work, but the line itself is dead code — the variable is never read. A future cleanup pass is likely to delete it.

If the goal is to assert that the untweaked key derivation also succeeds (i.e. a smoke gate), promote it to a real assertion. Otherwise drop the destructure and the `_ = xonly` line:

```rust
// Drop the untweaked-key derivation entirely; only the tweaked output key
// is needed for the BIP-341 SPK.
let tweaked = Keypair::from_secret_key(&secp, &sk).tap_tweak(&secp, None);
let tweaked_xonly = tweaked.to_keypair().x_only_public_key().0;
```

Same pattern repeats in `per_script_vectors.rs:267-271` and `tests/integration/mod.rs:859`. If you're keeping the assertion in spirit, make it a real `let _: XOnlyPublicKey = xonly;` type-check (one line, no runtime cost) or a `debug_assert!(xonly.serialize().len() == 32)`.

---

_Reviewed: 2026-05-31T13:30:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
