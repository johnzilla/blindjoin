---
phase: 21-audit-charter-zeroization-tightening
reviewed: 2026-05-31T00:00:00Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - .cargo/audit.toml
  - coordinator/src/api/handlers.rs
  - coordinator/src/blind/rsa.rs
  - coordinator/src/round/input_reg.rs
  - coordinator/src/round/manager.rs
  - coordinator/src/round/signing.rs
  - coordinator/src/round/state.rs
  - docs/AUDIT-CHARTER.md
findings:
  critical: 1
  warning: 5
  info: 4
  total: 10
status: issues_found
---

# Phase 21: Code Review Report

**Reviewed:** 2026-05-31
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Phase 21 delivered two related changes: (a) an `Option<RsaBlindSigner>` lifetime-bounded
`RoundSecretKey` newtype (Plan 21-01) and (b) a new `docs/AUDIT-CHARTER.md` + refreshed
`.cargo/audit.toml` (Plan 21-02). The core mechanism — wrapping `BjSecretKey` in a newtype
that participates in the transitive `rsa::RsaPrivateKey` `ZeroizeOnDrop` chain — is
structurally sound. The newtype `RoundSecretKey`, its empty-crypto-body Drop impl, the
production `.as_ref().expect(...)` call sites at handlers.rs:383 and input_reg.rs:71, and
the new structural FSM test `round_secret_key_dropped_on_round_end` all match the design
intent. The audit charter is well-organized into the required 8 H2 sections and the README
callout is in place.

However, the AUDIT-03 mitigation is **structurally undermined by a pre-existing pattern
that this phase did not address**: every FSM trigger that fires the Drop chain uses
`let _ = state.transition_to(...)` (signing.rs:279-280, blame.rs:219-220, output_reg.rs:30-31).
On the success-broadcast path, a failed first transition silently masks a no-op second
transition (Signing → Idle is NOT a valid FSM edge), leaving `inner` (and the RSA secret
key) live in memory after a successful CoinJoin broadcast. This is the inverse of the
load-bearing AUDIT-03 claim. **CR-01 below.**

Additionally: the line numbers cited in `rsa.rs`, `.cargo/audit.toml`, and
`AUDIT-CHARTER.md` for the `self.inner = None` chokepoint are wrong (`state.rs:194-200`
/ `state.rs:195` in citations; actual location is line **202** inside the block at
201-207). The buffer-scrub sanity test in `rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop`
contains a mechanism description that misrepresents what the test actually proves. The
blinded-token size check accepts RSA-4096-sized inputs even though the coordinator only
generates RSA-2048 keys.

## Critical Issues

### CR-01: Success-broadcast Drop trigger can silently fail to fire — RSA secret key remains live in memory

**File:** `coordinator/src/round/signing.rs:278-280`
**Issue:** The success path `assemble_and_broadcast` fires the AUDIT-03 zeroization
trigger via:

```rust
// Transition round to Broadcast then Idle (zeroes all sensitive state)
let _ = state.transition_to(Phase::Broadcast);
let _ = state.transition_to(Phase::Idle);
```

The `let _ =` discards both transition results. Per the FSM definition in
`state.rs::can_transition_to`, **`Signing → Idle` is NOT a valid edge** (the valid edges
from `Signing` are `Signing → Broadcast` and `Signing → Blame` only). If the first call
ever returns `Err` (e.g., a concurrent state mutation has already advanced the FSM out
of `Signing`, or a future patch tightens a precondition), the state stays in `Signing`,
the second call then attempts the illegal `Signing → Idle` edge, that ALSO returns
`Err`, the `let _` discards it, and `self.inner` is **never set to `None`** — meaning
the `RoundStateInner` (containing `Option<RsaBlindSigner>` containing the per-round
`rsa::RsaPrivateKey` limbs) **survives the round** and remains live in memory.

This directly contradicts the load-bearing AUDIT-03 claim documented at
`docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` ("The SOLE FSM trigger that
nulls this Option is `transition_to(Phase::Idle)` … The full chain runs"). The
structural test `round_secret_key_dropped_on_round_end` passes only because it drives
the FSM in a controlled context where the first transition cannot fail — it does not
exercise the silent-failure path.

The same pattern exists at `blame.rs:219-220` (Signing/OutputReg → Blame → Idle) and
`output_reg.rs:30-31` (OutputReg → Blame → Idle), with the same risk. The audit
charter section §5 does not mention this risk at all.

**Severity rationale:** Marked Critical (not Warning) because this is the failure mode
that the entire AUDIT-03 mitigation is designed to prevent. The Marvin Attack residual
risk in `.cargo/audit.toml` and the charter explicitly cite the bounded lifetime as the
mitigation; if the bound can be silently broken on the success path, the rationale for
accepting RUSTSEC-2023-0071 weakens. While in the current codebase Signing → Broadcast
is reachable in practice on the happy path, the AUDIT-03 chain should be robust against
any future FSM tightening, and the `let _` makes that impossible to verify from the
type signature alone.

**Fix:**
```rust
// Transition Signing → Broadcast → Idle. Both edges MUST succeed for the AUDIT-03
// Drop chain to fire and zeroize the per-round RSA secret key
// (docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window). Log-and-explicit-zero
// on failure so the secret never outlives the round even if the FSM rejects the edge.
if let Err(e) = state.transition_to(Phase::Broadcast) {
    tracing::error!(round_id = %round_id_str, error = %e,
        "AUDIT-03: Signing → Broadcast transition rejected — forcing inner drop");
    state.inner = None; // last-resort scrub to preserve the AUDIT-03 lifetime bound
}
if let Err(e) = state.transition_to(Phase::Idle) {
    tracing::error!(round_id = %round_id_str, error = %e,
        "AUDIT-03: Broadcast → Idle transition rejected — forcing inner drop");
    state.inner = None;
}
```
Apply the equivalent pattern at `blame.rs:219-220` and `output_reg.rs:30-31`. Optionally:
add an FSM edge `Signing → Idle` (currently disallowed) so the success path can survive
a Signing → Broadcast failure cleanly without bypassing `transition_to`. Either way, the
AUDIT-03 invariant should be expressed so that **the secret cannot outlive `inner = None`
on any control-flow path**, including FSM-rejection paths.

## Warnings

### WR-01: Audit-charter line-number citations are wrong throughout

**File:** `docs/AUDIT-CHARTER.md:39, 194, 333-336`, `.cargo/audit.toml:22-23`,
`coordinator/src/blind/rsa.rs:20, 32, 82-83`
**Issue:** Multiple production-anchored citations name `coordinator/src/round/state.rs:194-200`
(or `state.rs:195`) as the location of `self.inner = None` inside
`transition_to(Phase::Idle)`. The **actual** location is:

- `pub fn transition_to(...)` starts at line **193**
- Lines **194-200** are the early-return error block (`if !can_transition_to { return Err(...); }`)
- The `if next == Phase::Idle` block is at lines **201-207**
- `self.inner = None;` is at line **202**

Verified by reading `state.rs` directly. So every citation of "state.rs:194-200" points
at the wrong code (error early-return) and "state.rs:195" points at the middle of an
error variant constructor — not at the load-bearing line. Examples:
- `rsa.rs:20`: ``transition_to(Phase::Idle) (`state.rs:194-200`)``
- `rsa.rs:32`: ``the FSM nulls at one chokepoint (`state.rs:195`)``
- `rsa.rs:82-83`: ``sets self.inner = None (coordinator/src/round/state.rs:194-200)``
- `audit.toml:22-23`: ``transition_to(Phase::Idle) (coordinator/src/round/state.rs:194-200)``
- `AUDIT-CHARTER.md:39`: ``transition_to ... ~line 186`` (actual: line 193)
- `AUDIT-CHARTER.md:194`: ``coordinator/src/round/state.rs:194-200``
- `AUDIT-CHARTER.md:333-334`: ``the SOLE site setting inner = None (verified by grep ...) ... line 194-200``

The charter's own preamble (§1, lines 28-31) acknowledges this risk: "The durable anchor
is the `file:symbol` form (per the project's anchor-stability convention): symbols
survive line-number churn ... whereas a bare `file:NN` ref bit-rots". The charter then
proceeds to violate its own convention by anchoring on numeric line ranges that are
**already** wrong at the v1.5 ship.

**Fix:** Replace every `state.rs:194-200` / `state.rs:195` citation with either:
- the symbol-form anchor `state.rs::transition_to` (charter's own preferred form), OR
- the correct line range `state.rs:201-207` with `self.inner = None` at `state.rs:202`.

Search/replace targets:
```
state.rs:194-200  →  state.rs::transition_to  (or state.rs:201-207)
state.rs:195      →  state.rs::transition_to  (or state.rs:202)
```
Cross-check after the fix that `AUDIT-CHARTER.md` §5 table at line 39 (currently
"~line 186") matches the actual `transition_to` decl line (193).

### WR-02: Buffer-scrub sanity test mechanism description does not match what the test proves

**File:** `coordinator/src/blind/rsa.rs:240-297` (`round_secret_key_buffer_overwritten_on_drop`)
**Issue:** The test doc-comment (lines 240-255) and inline comment (lines 274-277) claim:
> "drop the signer to fire the transitive `<rsa::RsaPrivateKey as Drop>::drop` chain
> (`rsa-0.9.10/src/key.rs:76-82`), then sweep an 8 MB probe buffer for the captured
> fingerprint. ... a hit means the DER bytes survived in an adjacent allocation, which
> would indicate the upstream zeroize chain did not run."

This is misleading on two grounds:

1. **The needle is DER bytes, but the upstream zeroize chain zeroizes the in-memory
   RSA limb arrays (`d`, `primes`, `precomputed`), NOT the DER serialization.** The DER
   form is produced on demand by `secret_key_der()` → `to_der()`, which allocates a
   fresh `Vec<u8>`. That allocation is held by `der_fingerprint` for the test's full
   lifetime (`der_fingerprint` is not dropped until end of test). The dropped `signer`
   never contained the DER bytes that the needle is extracted from.

2. **A "found" outcome therefore proves nothing about the zeroize chain.** It proves
   only that the 8 MB probe allocation happened to overlap with whatever heap region
   the (still-live) `der_fingerprint` occupies, OR with a deserialization scratch buffer
   that the BigUint→DER encoder used and freed. Neither is the property the comment
   claims to detect. A "not found" outcome (the success case) is similarly weak: it
   says nothing about whether the limbs were zeroized; it says only that the probe did
   not collide with the still-live `der_fingerprint`.

The structural FSM test (`state.rs::round_secret_key_dropped_on_round_end`) is correctly
labeled as the load-bearing assertion, so the test's overall framing as a "SANITY CHECK"
is fine — but the mechanism explanation is wrong and would mislead any future engineer
reading it to believe the test detects scrub-chain regressions.

A further confusion in the doc comment (line 250): "Probabilistic — false negatives are
acceptable". The terminology is inverted relative to the assertion (`assert!(!found)`):
the test's failure mode is the needle being **found**, so a "false negative" (in the
detection sense: scrub fails but test doesn't catch it) is a true-negative of the
assertion. The doc and assertion talk past each other.

**Fix:** Either (a) delete the test (the structural test is load-bearing; the
sanity ceremony adds review burden without proving the claimed property), or
(b) rewrite the doc-comment to accurately describe what it tests:

```rust
/// AUDIT-03 sanity ceremony (D-131 second bullet, CD-50): a heap-collision
/// smoke test. This test does NOT detect zeroize-chain regressions — the
/// upstream `rsa` crate zeroizes BigUint limb arrays (in-memory `d`, `primes`,
/// `precomputed`), not the DER serialization. The needle is extracted from
/// a separate DER allocation that survives the `drop(signer)` call.
///
/// The structural FSM test
/// `coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end`
/// is the load-bearing assertion. This test exists only as a build-time smoke
/// check that nothing about the heap layout has changed dramatically.
```
Also: drop `der_fingerprint` BEFORE the probe allocation so the test at least
plausibly probes freed memory:
```rust
let needle: Vec<u8> = der_fingerprint[100..132].to_vec();
drop(der_fingerprint);  // release the live DER allocation before probing
drop(signer);
let probe: Vec<u8> = vec![0u8; 8 * 1024 * 1024];
```
Otherwise the test is provably weak. Either fix is acceptable; the current state is not.

### WR-03: Blinded-token size check accepts RSA-4096 inputs but coordinator only generates RSA-2048

**File:** `coordinator/src/api/handlers.rs:130-142`
**Issue:** The pre-lock size validation accepts blinded tokens of either 256 bytes
(RSA-2048 modulus) OR 512 bytes (RSA-4096 modulus):

```rust
const RSA_2048_BYTES: usize = 256;
const RSA_4096_BYTES: usize = 512;
if blinded_token_bytes.len() != RSA_2048_BYTES && blinded_token_bytes.len() != RSA_4096_BYTES {
    return Err(api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN", ...));
}
```

However, `RsaBlindSigner::generate()` at `rsa.rs:109` hardcodes RSA-2048:
```rust
let kp = BjKeyPair::generate(&mut DefaultRng, 2048)?;
```

A 512-byte blinded token will pass this check, traverse `register_input` to the
`blind_sign(&blind_msg)` call at `input_reg.rs:71`, where the underlying
`blind_rsa_signatures::SecretKey::blind_sign` will reject the size mismatch (returning
the error to the client as `INVALID_TOKEN`). The effect is: an attacker can send
2× the work (256 bytes of decoded base64, plus a write-lock acquisition, plus
TOCTOU re-check, plus an attempted blind-sign) before being rejected. Not a
DoS-grade issue but it is wasted work that the size check is supposed to prevent.

The relaxation is also confusing: why advertise RSA-4096 support in the validator if
the coordinator never generates RSA-4096 keys? If RSA-4096 is a planned future config
flag, it should be tied to a `signer_modulus_bits` config value (single source of
truth); if it is not planned, the RSA_4096_BYTES alternative is dead code.

**Fix:** Tie the size check to the actual signer modulus. The modulus length is
queryable from `rsa_signer.public_key.0.size()` or via a new accessor. Simplest patch:
```rust
// Snapshot the actual modulus size from the live signer.
let expected_modulus_bytes = {
    let guard = state.round.read().await;
    guard.inner.as_ref()
        .and_then(|i| i.rsa_signer.as_ref())
        .map(|s| s.public_key.0.size())
        .unwrap_or(256)
};
if blinded_token_bytes.len() != expected_modulus_bytes {
    return Err(api_error(StatusCode::BAD_REQUEST, "INVALID_TOKEN",
        format!("blinded_token must be {} bytes (RSA modulus size), got {}",
            expected_modulus_bytes, blinded_token_bytes.len()), None));
}
```
If RSA-4096 is intentionally not supported, simplify to just RSA_2048_BYTES (256).

### WR-04: `let _ =` on FSM transitions is a project-wide pattern that drops error information

**File:** `coordinator/src/round/signing.rs:279-280`, `coordinator/src/round/blame.rs:219-220`,
`coordinator/src/round/output_reg.rs:30-31, 36`, `coordinator/src/api/handlers.rs:286, 427`
**Issue:** Even setting aside the AUDIT-03 implications (covered in CR-01), the
`let _ = guard.transition_to(...)` / `let _ = state.transition_to(...)` pattern
silently discards a `Result<(), TransitionError>` whose `Err` variant carries
diagnostic information (`InvalidTransition { from, to }`) that would be valuable for
debugging FSM races. The seven call sites differ in their guarantees:

- `handlers.rs:286, 427`: Idle → InputReg / OutputReg → Signing both run under a write
  lock that re-checked the phase, so failure is unreachable today.
- `signing.rs:279-280`, `blame.rs:219-220`, `output_reg.rs:30-31`: As analyzed in
  CR-01, failure leaves `inner` live.
- `output_reg.rs:36`: OutputReg → Signing happy path. Same as handlers.rs:427.

Discarding the result conflates "currently unreachable" with "intentionally ignored",
and the pattern provides zero protection against a future regression that makes one
of these transitions reachable-failing.

**Fix:** At each call site, replace `let _ =` with explicit logging:
```rust
if let Err(e) = state.transition_to(Phase::OutputReg) {
    tracing::warn!(round_id = %round_id_str, error = %e,
        "FSM transition rejected — this should be unreachable from this code path");
}
```
For the signing.rs / blame.rs / output_reg.rs sites, additionally apply the CR-01 fix
(force `inner = None` on failure to preserve AUDIT-03).

### WR-05: Audit charter §5 verification claim does not address the silent-failure risk

**File:** `docs/AUDIT-CHARTER.md:377-398` (§5 Verification subsection)
**Issue:** The charter §5 cites two verification artifacts:
1. The structural test `round_secret_key_dropped_on_round_end`
2. The best-effort buffer scrub test

Both run the FSM transition in a controlled context where neither `transition_to`
call can fail. Neither test exercises the production failure mode (the silent-failure
analyzed in CR-01). An auditor reading §5 would conclude the verification surface is
complete; the structural test pattern matches the production code only because the
production code uses `let _ =` to suppress exactly the failure mode the test would
catch.

The charter also asserts at lines 333-336:
> "The SOLE site setting `inner = None` (verified by grep of the entire
> `coordinator/src/` tree) ... All 4 routes through `transition_to(Phase::Idle)`;
> none bypass the chokepoint."

This is true as a static-grep observation but is misleading without noting that the
chokepoint itself can no-op silently. The charter should either:
- Acknowledge the silent-failure risk as a documented gap (Residual Risk), OR
- Defer §5's "load-bearing claim" framing until CR-01 is fixed and `transition_to`
  is wrapped in code that guarantees `inner = None` regardless of FSM verdict.

**Fix:** After applying CR-01, update §5 paragraph "The trigger" to add:
> "Each call site that fires the trigger checks the transition result and forces
> `state.inner = None` on rejection (preserving the AUDIT-03 lifetime bound even if
> a future FSM tightening makes the transition unreachable from a given phase)."

If CR-01 is deferred, add a Residual Risk entry to §7 acknowledging that the
zeroization trigger can be silently skipped if a future FSM tightening or
concurrent-state race causes the first `let _ = transition_to(...)` call to fail.

## Info

### IN-01: D-07 doc comment in rsa.rs uses lossy "may leak DER bytes" wording

**File:** `coordinator/src/blind/rsa.rs:62-63`
**Issue:** The Drop impl comment says: "a naive `?self.0` would invoke `BjSecretKey`'s
derived `Debug`, which prints the inner `RsaPrivateKey` and may leak DER bytes". The
"may leak" framing understates the certainty — `Debug` on `RsaPrivateKey` deterministically
prints (or redacts, depending on rsa crate behavior) field contents; whether it leaks DER
specifically depends on the upstream impl. Better to say "may format the private exponent
`d`, the prime factors, or other secret limbs" — accurate to the limb representation
rather than the DER form.

**Fix:**
```rust
// PII-safe: static-string message only, target `blindjoin::audit` for
// filterability. No `{:?}` formatter on `self` or any field — `BjSecretKey`'s
// derived `Debug` forwards to `rsa::RsaPrivateKey::fmt`, which (depending on
// upstream version) may format the private exponent `d`, the prime factors,
// or other secret limbs.
```

### IN-02: `let _ = guard.transition_to(...)` in handlers.rs:286 swallows result twice

**File:** `coordinator/src/api/handlers.rs:286, 427`
**Issue:** Both happy-path transitions (`InputReg → OutputReg` when max participants
reached; `OutputReg → Signing` when all outputs registered) discard the result. These
are not AUDIT-03 trigger sites so the safety argument is weaker, but the same
diagnostic-loss concern applies. Identical fix to WR-04.

**Fix:** See WR-04 fix.

### IN-03: Hardcoded RSA modulus byte constants duplicated between handler and rsa.rs

**File:** `coordinator/src/api/handlers.rs:130-131` vs `coordinator/src/blind/rsa.rs:109`
**Issue:** The numeric literal `2048` (bits) appears in `rsa.rs:109` (`generate(&mut DefaultRng, 2048)`),
and the derived byte constant `256` appears in `handlers.rs:130` as `RSA_2048_BYTES: usize = 256`.
Two-source-of-truth for the RSA modulus. If the project ever supports configurable modulus
size, both need synchronized updates. Related to WR-03.

**Fix:** Make modulus size a single constant in `rsa.rs`:
```rust
pub const RSA_KEY_BITS: usize = 2048;
pub const RSA_KEY_BYTES: usize = RSA_KEY_BITS / 8; // 256
```
And reference these from both call sites.

### IN-04: `RoundSecretKey::as_inner` could be marked `#[inline]`

**File:** `coordinator/src/blind/rsa.rs:47-49`
**Issue:** `as_inner` is a trivial accessor called in hot paths (`blind_sign`, DER export);
the optimizer will likely inline it across crate boundaries since it's `pub(crate)`, but an
explicit `#[inline]` documents the intent and stabilizes behavior under future codegen
changes.

**Fix:**
```rust
#[inline]
pub(crate) fn as_inner(&self) -> &BjSecretKey {
    &self.0
}
```
Low priority — current behavior is likely fine; this is documentation more than performance.

---

## Out-of-scope observations (not flagged as findings)

Two observations are out of scope for this phase but worth noting for the project:

1. **`assemble_and_broadcast` performs Bitcoin RPC (`testmempoolaccept`, `sendrawtransaction`)
   under the write lock** (`signing.rs:239-273`, reached via `process_sign` at the write lock
   at `handlers.rs:570`). This appears to conflict with the AVAIL-01 principle ("Async RPC
   calls execute before the write lock") cited elsewhere in the codebase. This is pre-existing
   and not introduced by Phase 21, so it is not flagged.

2. **`shared/` and `client/` were not in scope for review per the context block** and were
   not modified by this phase. The V1.4-CRIT-01 dispatcher-only public surface invariant
   is structurally preserved by the absence of changes to `shared/`. No findings.

---

_Reviewed: 2026-05-31_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
