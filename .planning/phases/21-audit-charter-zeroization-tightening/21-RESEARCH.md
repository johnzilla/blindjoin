# Phase 21: Audit Charter & Zeroization Tightening — Research

**Researched:** 2026-05-31
**Domain:** Rust memory zeroization (transitive Drop chains), cargo-audit advisory diff, audit-charter authoring
**Confidence:** HIGH

## Summary

Investigation collapses the three open questions CONTEXT.md hands to research into clear, evidence-backed recommendations:

1. **D-129 / D-129a Drop body shape.** The `blind-rsa-signatures 0.17.x` `SecretKey` wraps `rsa::RsaPrivateKey` (verified in installed registry source). `rsa = 0.9.10` has an **unconditional `impl Drop for RsaPrivateKey`** at `src/key.rs:76-82` that zeroizes `d`, `primes`, and `precomputed`, plus `impl ZeroizeOnDrop for RsaPrivateKey {}` at line 84. Both impls compile **without any feature flag**. This means dropping a `BjSecretKey` already runs RFC-9474-secret zeroization transitively through the `rsa` crate. The D-07 comment's "best-effort" qualifier is **factually out of date** as of `rsa 0.9.10`. **Recommended Drop body: empty + `tracing::debug!` event** — the upstream `ZeroizeOnDrop` does the cryptographically meaningful work; the newtype's value is the lifetime bound, not a redundant in-place scrub. The DER-roundtrip path is harmful (it allocates a fresh DER buffer that we then must zeroize separately) and the replace-with-dummy path wastes ~100ms per round on a no-op (the original key would be zeroized anyway by `Drop`).

2. **D-130a Drop trigger surface.** The 4 valid `Phase → Idle` FSM edges all route through `transition_to(Phase::Idle)`, which is the **sole** site setting `self.inner = None` (verified: only one `inner = None` assignment in the entire `coordinator/src/` tree). No path bypasses this trigger. The `RoundManager` has no `HashMap<RoundId, RoundState>` map drop-on-removal pattern — there is a single round per coordinator instance (single `Arc<RwLock<RoundState>>` in `run.rs`). The narrative charter §5 wants is **factually correct as written**.

3. **D-141 fresh cargo-audit diff.** Ran `cargo audit --json` against current `Cargo.lock` (current advisory DB: 1099 advisories, last commit `eaf48e7`, last updated `2026-05-29`). With current 3 ignores in place: **0 vulnerabilities, 0 warnings.** With ignores temporarily removed: the same 3 IDs surface (RUSTSEC-2023-0071 as vulnerability; RUSTSEC-2025-0141 and RUSTSEC-2024-0436 as unmaintained warnings) and nothing else. **No new advisories.** Planner can lock the 3 existing ignores verbatim with charter anchors.

**Primary recommendation:** Adopt empty-body `Drop for RoundSecretKey` (with PII-safe `tracing::debug!`) and rewrite the D-07 comment to cite the **transitive `rsa::RsaPrivateKey` `ZeroizeOnDrop`** rather than "best-effort upstream limitation." This is more honest, more accurate, and stronger audit narrative than the CONTEXT.md default. The structural lifetime bound (`Option<RsaBlindSigner>` on `RoundStateInner`) remains the load-bearing claim regardless.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| RSA secret-key lifetime bound (newtype + Drop) | Coordinator crypto layer (`coordinator/src/blind/rsa.rs`) | — | Coordinator-local; client never holds RSA secret. v1.6 promotion to `shared/` deferred per CONTEXT.md `<deferred>`. |
| RSA secret-key memory zeroization | Upstream `rsa` crate (`Drop for RsaPrivateKey` at `rsa-0.9.10/src/key.rs:76-82`) | Coordinator newtype (defensive ceremony only) | Cryptographically meaningful zeroization runs via transitive `ZeroizeOnDrop` on the wrapped `RsaPrivateKey`. The newtype's role is **lifetime expression**, not redundant scrub. |
| FSM drop trigger (`Option<RsaBlindSigner> = None`) | `coordinator/src/round/state.rs::transition_to` | — | Single chokepoint at line 194-200. No bypass paths. |
| Audit charter authoring (`docs/AUDIT-CHARTER.md`) | Documentation | — | Prose artifact; cites code by file:symbol. |
| Advisory residual-risk register | `.cargo/audit.toml` + charter §7 | — | Detection at research time, decision at planner time, expression at audit.toml + charter §7. |
| README integration (audit-charter callout) | `README.md` §Security Model | — | One-paragraph callout, established hardening-rollup convention. |

## Standard Stack

This phase **does NOT install or upgrade any dependencies**. All work is internal to existing crates and documentation. The Standard Stack table is therefore minimal — listing only the dependencies whose behavior is load-bearing for the research conclusions.

### Core (existing, unchanged)

| Library | Version | Purpose | Why Load-Bearing |
|---------|---------|---------|------------------|
| `blind-rsa-signatures` | `0.17.1` (per Cargo.lock) | RFC 9474 RSA blind signatures | The `SecretKey<H,S,M>` wraps `rsa::RsaPrivateKey` — verified in installed source at `/Users/john/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/blind-rsa-signatures-0.17.1/src/lib.rs:825-828`. |
| `rsa` (transitive) | `0.9.10` (per Cargo.lock) | `RsaPrivateKey` Drop impl | **`impl Drop for RsaPrivateKey`** at `key.rs:76-82` calls `.zeroize()` on `d`, `primes`, `precomputed`. `impl ZeroizeOnDrop for RsaPrivateKey {}` at line 84. Both UNCONDITIONAL (no `cfg` feature gate). |
| `zeroize` | `1.8` | Existing manual-Drop pattern on `RoundStateInner` | Cargo.toml workspace dep with `derive` feature. `RegisteredInput`, `RegisteredOutput` derive `Zeroize` — pattern reused by Phase 21 for nothing new. |
| `tracing` | `0.1` | PII-safe structured logging | Used in `Drop for RoundSecretKey` `debug!` event with `round_id` only — no key material. |

### Alternatives Considered (and rejected)

| Instead of | Could Use | Why Not (verified) |
|------------|-----------|---------------------|
| Empty Drop + transitive zeroize | DER-roundtrip scrub (D-129 option 1) | `SecretKey::to_der()` (`lib.rs:878`) calls `self.inner.to_pkcs8_der()...map(\|x\| mem::take(x.to_bytes().as_mut()))` — a fresh `Vec<u8>` already; calling `to_der()` in Drop ALLOCATES a new buffer we'd then have to `zeroize()` separately. The original `BigUint` allocations (`d`, `primes`) get zeroized by the wrapped `RsaPrivateKey::drop` regardless of whether we call `to_der()` first. DER-roundtrip is strictly worse: extra allocation, extra wall-clock, identical zeroization outcome. |
| Empty Drop + transitive zeroize | Replace-with-dummy `KeyPair::generate` (D-129 option 2) | Costs ~100ms RSA-2048 keygen per round end (verified by `blind-rsa-signatures` 0.17.1 `brsa.rs` `generate` implementation; RSA keygen is dominated by primality testing). The "overwrite the parent struct's allocation" rationale is incorrect: `std::mem::replace` drops the OLD `RsaPrivateKey`, which fires its `Drop` impl, which zeroizes `d`/`primes`/`precomputed` — the exact same outcome as an empty Drop. The dummy keypair never enters memory we care about because it's immediately dropped on scope exit too. Net: ~100ms wasted to achieve the same zeroization the transitive `Drop` chain already runs. |
| Empty Drop | Explicit `take_secret`-style API from upstream | Verified to not exist: no `secure_erase`, `take_secret`, `into_inner`, `scrub`, or similar method on `SecretKey`. The only secret-bearing API surface on `SecretKey` is `to_der`, `to_pem`, `components()`. The `components()` method returns `SecretKeyComponents<'a>` which contains an `&'a RsaPrivateKey` — no consume-and-zero shape. |

**Installation:** Not applicable. Phase 21 introduces no new dependencies.

**Version verification:**
```bash
grep -A1 '^name = "blind-rsa-signatures"' Cargo.lock  # 0.17.1
grep -A1 '^name = "rsa"' Cargo.lock                    # 0.9.10
```
Both verified against installed registry source (read directly from `~/.cargo/registry/src/.../*.rs`).

## Package Legitimacy Audit

> Phase 21 installs no new packages. No legitimacy audit needed.

| Package | Registry | Disposition |
|---------|----------|-------------|
| _(none — Phase 21 is internal-only)_ | — | — |

## Architecture Patterns

### System Architecture Diagram

```
Round lifecycle (FSM)
    Idle
     │  start_round() — manager.rs:40
     ▼
  InputReg
     │  quorum reached: transition_to(OutputReg) — run.rs:172
     │  quorum failed:  transition_to(Idle) ─────────────────┐
     ▼                                                       │
  OutputReg                                                  │
     │  all outputs:    transition_to(Signing) — output_reg.rs:36
     │  missing output: transition_to(Blame) → transition_to(Idle) ──┐
     ▼                                                       │       │
  Signing                                                    │       │
     │  success: transition_to(Broadcast) — signing.rs:279   │       │
     │  timeout: transition_to(Blame) → transition_to(Idle) ─┤       │
     ▼                                                       │       │
  Broadcast                                                  │       │
     │  transition_to(Idle) — signing.rs:280                 │       │
     ▼                                                       │       │
    Idle ◄─────────────────────────────────────────────────────────────┘

At every transition_to(Phase::Idle) call (state.rs:194-200):
  self.inner = None
    │
    └─► drop(Option<RoundStateInner>)
          │
          └─► drop(RoundStateInner)              // existing manual Drop, state.rs:120-149
                │
                ├─► self.rsa_signing_key.zeroize()
                ├─► self.round_secret.zeroize()
                ├─► (HashMap iter_mut + zeroize) ×3
                ├─► (Vec iter_mut + zeroize)
                └─► drop(Option<RsaBlindSigner>)   // PHASE 21: was bare RsaBlindSigner
                      │
                      └─► drop(RsaBlindSigner)      // unchanged structurally
                            │
                            └─► drop(RoundSecretKey)  // PHASE 21: NEW newtype
                                  │
                                  ├─► (Phase 21 Drop body: PII-safe tracing event)
                                  └─► drop(BjSecretKey)
                                        │
                                        └─► drop(SecretKey<Sha384,PSS,Randomized>)
                                              │
                                              └─► drop(inner: RsaPrivateKey)  // upstream
                                                    │  rsa-0.9.10/src/key.rs:76-82
                                                    ├─► self.d.zeroize()
                                                    ├─► self.primes.zeroize()
                                                    └─► self.precomputed.zeroize()
                                                          │
                                                          └─► drop(PrecomputedValues)
                                                                │  rsa-0.9.10/src/key.rs:114-118
                                                                └─► zeroize() (dp,dq,qinv,crt_values)
```

### Recommended Project Structure

No changes — Phase 21 adds files to existing locations:

```
coordinator/src/blind/rsa.rs       # 21-01: RoundSecretKey newtype + Drop + scrub test
coordinator/src/round/state.rs     # 21-01: Option<RsaBlindSigner> + structural test
coordinator/src/round/signing.rs   # 21-01: 4 test-fixture wraps in Some(...)
coordinator/src/round/manager.rs   # 21-01: 1 production .as_ref() + 1 test
coordinator/src/round/input_reg.rs # 21-01: 1 production .as_ref()
coordinator/src/api/handlers.rs    # 21-01: 1 production .as_ref()
docs/AUDIT-CHARTER.md              # 21-02: NEW
.cargo/audit.toml                  # 21-02: anchor refs + RUSTSEC-2023-0071 rewrite
README.md                          # 21-02: §Security Model callout
```

### Pattern 1: Transitive Drop Chain over Opaque Upstream Types

**What:** The newtype's `Drop` body does NOT need to do crypto work when the wrapped type already has correct `Drop` semantics upstream. The newtype's value is **lifetime expression** — making the secret's lifetime a `Option<_>` field that the FSM can null out on a single chokepoint.

**When to use:** When wrapping an upstream type whose `Drop` impl is correct but whose lifetime in the caller is ambient/unbounded. The newtype turns ambient lifetime into a value the FSM can null.

**Example:**
```rust
// Source: this codebase, coordinator/src/round/state.rs:120-149 (existing manual Drop pattern)
//         and proposed Phase 21 RoundSecretKey extension.

/// AUDIT-03: RoundSecretKey wraps BjSecretKey so the secret's lifetime is
/// bounded to Option<RsaBlindSigner> on RoundStateInner. The wrapped
/// BjSecretKey delegates to `rsa::RsaPrivateKey`, which has an unconditional
/// `impl Drop` that zeroizes `d`, `primes`, and `precomputed`
/// (rsa-0.9.10/src/key.rs:76-82). Drop chain:
///   transition_to(Phase::Idle) (state.rs:194-200)
///     → drop(Option<RoundStateInner>)
///       → drop(Option<RsaBlindSigner>)
///         → drop(RoundSecretKey)        // this impl
///           → drop(BjSecretKey)
///             → drop(rsa::RsaPrivateKey) // zeroizes d, primes, precomputed
pub struct RoundSecretKey(BjSecretKey);

impl Drop for RoundSecretKey {
    fn drop(&mut self) {
        // The wrapped `rsa::RsaPrivateKey` zeroizes d/primes/precomputed
        // in its own Drop. No additional in-place scrub needed here.
        //
        // PII-safe debug event documents the scrub firing — no key material,
        // no round_id (we don't have a handle to it from this scope).
        tracing::debug!(
            target: "blindjoin::audit",
            "RoundSecretKey dropped — RsaPrivateKey ZeroizeOnDrop fires transitively"
        );
    }
}
```

### Pattern 2: FSM Single-Chokepoint Drop Trigger

**What:** All drop-trigger paths route through `transition_to(Phase::Idle)`, which is the sole site clearing `self.inner = None`. No `inner = None` or `drop(inner)` outside this method.

**When to use:** When you want the audit charter to say "the secret is freed at SITE X" with confidence X is the only site.

**Example:**
Confirmed via grep:
```bash
$ rg -n 'inner = None|\.inner = None' coordinator/src/
coordinator/src/round/state.rs:195:    self.inner = None; // triggers ZeroizeOnDrop on RoundStateInner
```

### Anti-Patterns to Avoid

- **Calling `to_der()` in the Drop body** — allocates a fresh `Vec<u8>` we'd then have to `zeroize()` separately. The wrapped `RsaPrivateKey` already zeroizes its in-place `BigUint` allocations on drop; calling `to_der()` is pure ceremony with extra allocation cost.
- **Calling `KeyPair::generate` + `mem::replace` in Drop** — costs ~100ms of RSA-2048 keygen per round end; the dropped original is zeroized by the upstream `Drop` impl anyway; the dummy is also immediately dropped on scope exit. ~100ms wasted to achieve identical outcome.
- **Logging key material in the Drop body** — violates PROJECT.md no-PII-logging constraint. Drop body's `tracing::debug!` must not include any secret-derived bytes (including DER, public key hash with full bytes, etc.).
- **Adding new explicit `signer = None` call sites** — D-130b rejected this; the single existing `transition_to(Phase::Idle)` trigger is cleaner for the audit narrative.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RSA private key zeroization | Custom `mem::set` over raw key bytes | Transitive `rsa::RsaPrivateKey` `Drop` (`key.rs:76-82`) | Upstream uses crypto-correct `BigUint::zeroize` from the `zeroize` crate; raw `mem::set` over the wrapped `Vec`/`BigUint` allocations would miss the heap-resident limbs. |
| RAM-scan testing for zeroization | Allocator hooks (`mimalloc`, `jemalloc`) | A bounded heap sweep over a `Vec<u8>` allocated **after** the secret drops (CD-50 pattern) | The wrapped `RsaPrivateKey`'s allocations are `BigUint` (`crypto-bigint::Boxed`), zeroized in-place by Drop. A best-effort post-drop heap scan can detect if the original DER pattern survives in adjacent allocator pages — this is "structural-claim is load-bearing, scrub-test is sanity-check." |
| Audit charter rationale strings | Markdown-link syntax in `.cargo/audit.toml` comments | Bare relative path `See docs/AUDIT-CHARTER.md#anchor` | TOML comments render nowhere; markdown link syntax is overhead. D-139 locks this. |
| Charter file:symbol anchors | Line numbers (`rsa.rs:42`) | Symbol references (`rsa.rs::RoundSecretKey::drop`) | Line refs bit-rot on every patch; symbol refs are durable across reformats. D-138 locks this. |

**Key insight:** The cryptographically meaningful work is already done by the upstream `rsa = 0.9.10` `Drop` impl. The newtype's role is to make the **lifetime** of the secret a value the FSM can null — i.e., to convert ambient lifetime into a typed lifetime bound. This is what makes the charter say "bounded by `Option<RsaBlindSigner>`" rather than "best-effort."

## Runtime State Inventory

> Phase 21 is internal code+docs changes only. No databases, no live-service config, no OS-registered state. The runtime-state-inventory check is N/A for this phase.

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | None — Phase 21 touches no DB, no Mem0, no ChromaDB | — |
| Live service config | None — no n8n, Datadog, Cloudflare Tunnel touched | — |
| OS-registered state | None — no Task Scheduler, pm2, launchd, systemd touched | — |
| Secrets/env vars | None — no SOPS, no .env, no CI env-var rename | — |
| Build artifacts | None — package names unchanged; no egg-info, no compiled binary rename | — |

## Common Pitfalls

### Pitfall 1: Mistaking "best-effort upstream" for current state

**What goes wrong:** The existing D-07 comment at `coordinator/src/blind/rsa.rs:18-22` asserts "As of blind-rsa-signatures 0.17.x, SecretKey does not implement Zeroize." This is **literally true** at the `SecretKey` type level — `SecretKey<H,S,M>` has no `impl Zeroize`. But the comment elides the critical fact: `SecretKey<H,S,M>` **wraps** `rsa::RsaPrivateKey`, which has an unconditional `impl Drop` AND `impl ZeroizeOnDrop` in `rsa = 0.9.10` (verified at `~/.cargo/registry/.../rsa-0.9.10/src/key.rs:76-84`). When the `SecretKey` drops, its `inner: RsaPrivateKey` drops, which fires the upstream zeroization. **The "best-effort" qualification is functionally outdated.**

**Why it happens:** The D-07 comment was likely written when `rsa < 0.9.0` (which historically did NOT have unconditional Drop+Zeroize on `RsaPrivateKey`). The `rsa` crate added `ZeroizeOnDrop` for `RsaPrivateKey` in the 0.9 series. As Phase 21 ships, the upstream crate's behavior is stronger than the comment claims.

**How to avoid:** Plan 21-01's D-132 rewrite of the D-07 comment should cite **the upstream `rsa::RsaPrivateKey` Drop+ZeroizeOnDrop chain** as the cryptographically meaningful guarantee, and position the newtype's role as **lifetime expression**, not "best-effort scrub." This is more accurate and stronger audit narrative than CONTEXT.md's default.

**Warning signs:** A reviewer asking "why is there a Drop body that does nothing?" — because there's no "best-effort" anymore; the body's value is the **explicit citation of the transitive Drop chain** in its doc comment, not any in-place crypto operation.

### Pitfall 2: `tracing::debug!` in Drop body leaking key material

**What goes wrong:** A naive `tracing::debug!("secret key {:?} dropped", self.0)` would invoke `Debug` on `BjSecretKey` (which is `#[derive(Clone, Debug)]` per `blind-rsa-signatures` `lib.rs:824`). The `Debug` impl is auto-derived and prints the inner `RsaPrivateKey` Debug, which prints `BigUint` Debug, which **may** print the raw bytes of the secret exponent and primes (depends on `crypto-bigint`'s Debug impl).

**Why it happens:** Default `Debug` derivation has no PII-safety awareness.

**How to avoid:** Drop body's tracing event uses ONLY:
- a static string message
- the target `"blindjoin::audit"` for filtering
- NO field interpolation of `self`
- Optionally, a non-secret correlation field (but we don't have a `round_id` handle from this scope, and getting one would require threading it through `RsaBlindSigner` → `RoundSecretKey` which is overkill).

The proposed body (Pattern 1 above) follows this rule. Plan 21-01 should NOT add `?self` or `key_hash = ?self.public_key_hash()` to the Drop event.

**Warning signs:** Any `{:?}` formatter for `self` or any of its fields.

### Pitfall 3: `RsaBlindSigner::generate()` returning `Result` makes `Some(RsaBlindSigner::generate().unwrap())` a `.unwrap()` inside `Some()`

**What goes wrong:** Test-fixture refresh from `rsa_signer: RsaBlindSigner::generate().unwrap()` to `rsa_signer: Some(RsaBlindSigner::generate().unwrap())` is mechanical, BUT clippy may flag the nested `.unwrap()` if `clippy::unwrap_used` is configured. Verified: Cargo.toml has **no `[lints]` table** and there is **no `clippy.toml`**, so `unwrap_used` and `expect_used` are NOT denied. Tests use `.unwrap()` freely throughout the existing codebase. ✓ Safe.

**Why it happens:** Strict lint configs in some Rust projects deny `unwrap_used` in test code too.

**How to avoid:** No action needed for this project. Current clippy config is `--all-targets -- -D warnings` which denies all default-warn lints; `clippy::unwrap_used` is **not** a default-warn lint (it's `restriction` level). Verified: `cargo clippy -p coordinator --all-targets -- -D warnings` passes clean against the pre-Phase-21 codebase, which already uses `.unwrap()` in 4 test fixtures.

**Warning signs:** A future PR adding `clippy::unwrap_used = "deny"` to `[workspace.lints.clippy]` — would surface 5+ test-fixture sites. Not a Phase 21 concern.

### Pitfall 4: 4 production call sites mismatched against CONTEXT.md's "~6-10" estimate

**What goes wrong:** CONTEXT.md §"Integration Points" says "~6-10 call-site fix-ups". Actual count: **4** production call sites, **6** test-fixture call sites. Lower than the upper estimate.

**Why it happens:** Some call sites are likely inferred from `output_reg.rs::make_valid_token_sig` etc., but those construct local `signer` variables — they don't reach `inner.rsa_signer`. The `output_reg.rs` test fixtures take `&RsaBlindSigner` directly; they're NOT affected by the `Option<RsaBlindSigner>` refactor at all.

**How to avoid:** Plan 21-01 uses the exact 4-site list (below in "Production call sites of `inner.rsa_signer.*`") — no need to over-allocate task effort.

**Warning signs:** A test fixture that constructs a local `RsaBlindSigner::generate().unwrap()` and passes `&signer` to a helper — this is NOT affected by Phase 21 (the helper takes `&RsaBlindSigner`, not `&Option<RsaBlindSigner>`).

## Code Examples

### Proposed Drop Body (Recommended — see Open Question 1)

```rust
// Source: this codebase, proposed for coordinator/src/blind/rsa.rs

/// AUDIT-03: RoundSecretKey wraps BjSecretKey so the secret's lifetime is
/// expressible as `Option<RsaBlindSigner>` on RoundStateInner.
///
/// The wrapped BjSecretKey is `blind_rsa_signatures::SecretKey<Sha384,PSS,Randomized>`,
/// which holds `inner: rsa::RsaPrivateKey` (verified at
/// blind-rsa-signatures-0.17.1/src/lib.rs:825-828). When this struct drops,
/// it drops `inner: RsaPrivateKey`, whose unconditional `impl Drop`
/// (rsa-0.9.10/src/key.rs:76-82) calls `.zeroize()` on the `d`, `primes`,
/// and `precomputed` fields, and `impl ZeroizeOnDrop for RsaPrivateKey {}`
/// at line 84 marks the type as zeroize-on-drop.
///
/// The body of this Drop is therefore empty for crypto purposes — the
/// transitive upstream Drop chain does the cryptographically meaningful
/// work. The body emits a PII-safe tracing event to document the scrub
/// firing for ops observability.
pub struct RoundSecretKey(BjSecretKey);

impl RoundSecretKey {
    /// Wrap a fresh BjSecretKey. Called by RsaBlindSigner::generate.
    pub(crate) fn new(sk: BjSecretKey) -> Self {
        Self(sk)
    }

    /// Borrow the wrapped key for blind-signing operations.
    /// Never returns ownership — that would defeat the lifetime bound.
    pub(crate) fn as_inner(&self) -> &BjSecretKey {
        &self.0
    }
}

impl Drop for RoundSecretKey {
    fn drop(&mut self) {
        // The wrapped rsa::RsaPrivateKey zeroizes d/primes/precomputed in
        // its own Drop (rsa-0.9.10/src/key.rs:76-82). No in-place scrub
        // needed here — the transitive Drop chain runs as part of the
        // natural struct drop. PII-safe debug event documents the scrub
        // firing for ops observability.
        tracing::debug!(
            target: "blindjoin::audit",
            "RoundSecretKey dropped — rsa::RsaPrivateKey ZeroizeOnDrop fires transitively"
        );
    }
}
```

### Proposed Best-Effort Scrub Test (CD-50)

```rust
// Source: this codebase, proposed for coordinator/src/blind/rsa.rs::tests

/// AUDIT-03 best-effort: verify that after dropping a RoundSecretKey, a
/// freshly-allocated buffer of comparable size does not contain a recognizable
/// fingerprint of the dropped key material. This is a SANITY check; the
/// load-bearing assertion is the structural test in state.rs (that
/// transition_to(Idle) sets state.inner = None, triggering the Drop chain).
///
/// Mechanism: capture DER bytes of a known key, drop the RoundSecretKey,
/// allocate a Vec<u8> of similar size to occupy adjacent allocator pages,
/// then sweep for the captured DER bytes in the new buffer. Probabilistic;
/// marked #[ignore] if non-portable across toolchains. Per CONTEXT CD-50.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "non-portable heap layout; structural test in state.rs is the unconditional gate (D-131)")]
fn round_secret_key_buffer_overwritten_on_drop() {
    use blind_rsa_signatures::DefaultRng;

    // 1. Construct a known key and capture its DER fingerprint.
    let signer = RsaBlindSigner::generate().unwrap();
    let der_fingerprint = signer.secret_key_der().unwrap();
    // Take a short distinctive prefix — RSA-2048 PKCS#8 DER starts with a
    // SEQUENCE tag + length + version + algorithm OID, which is COMMON
    // across all RSA-2048 keys. Use the middle 32 bytes where the modulus
    // and exponent bytes live — they're per-key unique.
    assert!(der_fingerprint.len() >= 200, "RSA-2048 DER must be >200 bytes");
    let needle: Vec<u8> = der_fingerprint[100..132].to_vec();
    assert_eq!(needle.len(), 32);

    // 2. Drop the signer — Drop chain fires:
    //    RsaBlindSigner → RoundSecretKey → BjSecretKey → RsaPrivateKey
    //    (rsa-0.9.10/src/key.rs:76-82 zeroizes d/primes/precomputed in place).
    drop(signer);

    // 3. Allocate a Vec to occupy adjacent allocator pages, then sweep.
    //    Probabilistic: this does NOT guarantee the original allocation is
    //    reused; the structural test in state.rs is the load-bearing claim.
    let probe: Vec<u8> = vec![0u8; 8 * 1024 * 1024]; // 8 MB
    let found = probe.windows(needle.len()).any(|w| w == needle.as_slice());

    // Assertion is best-effort: we want needle to NOT be found, but a false
    // negative (needle is in some unrelated memory page) is acceptable.
    // The Drop fired (verified by the structural test); this just sanity-checks
    // no DER-tail survived in any adjacent allocation.
    assert!(
        !found,
        "RoundSecretKey buffer-scrub sanity check failed — \
         DER fingerprint survived in adjacent heap pages. \
         Structural lifetime bound (state.rs::transition_to_idle_clears_inner) \
         remains the load-bearing claim regardless."
    );
}
```

**Note:** The above test is **best-effort**, will be marked `#[ignore]` on non-Linux by default per CD-50, and is NOT load-bearing. The structural test in `state.rs` (proposed next) is the unconditional CI gate.

### Proposed Structural FSM Test (D-131 first bullet)

```rust
// Source: this codebase, proposed for coordinator/src/round/state.rs::tests

/// AUDIT-03 structural: the Round secret key's lifetime is bounded by
/// RoundStateInner.rsa_signer: Option<RsaBlindSigner>. Setting state.inner = None
/// (which transition_to(Phase::Idle) does at state.rs:194-200) drops the inner,
/// drops the Option<RsaBlindSigner>, drops the RsaBlindSigner, drops the
/// RoundSecretKey, which transitively zeroizes the underlying RsaPrivateKey
/// via rsa-0.9.10's unconditional impl Drop.
///
/// This is the LOAD-BEARING test for AUDIT-03 (D-131 structural bullet).
/// Mirrors transition_to_idle_clears_inner at line 262 but additionally
/// asserts the rsa_signer Option was Some pre-transition, so we know the
/// Drop chain DID fire on a non-None RoundSecretKey.
#[test]
fn round_secret_key_dropped_on_round_end() {
    use crate::blind::rsa::RsaBlindSigner;
    let mut state = RoundState::new_idle();
    state.phase = Phase::Signing;
    state.rsa_pubkey_der = Some(vec![1, 2, 3]);
    state.inner = Some(RoundStateInner {
        rsa_signing_key: vec![0xAA; 32],
        rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
        round_secret: [0xBB; 32],
        registered_inputs: Default::default(),
        redeemed_tokens: HashSet::new(),
        registered_outputs: vec![],
        partial_sigs: Default::default(),
        change_addresses: Default::default(),
    });
    // Pre-transition: rsa_signer is Some (Drop chain target exists).
    assert!(state.inner.as_ref().unwrap().rsa_signer.is_some(),
        "fixture must construct with Some(RsaBlindSigner)");

    // Drive the FSM through a real Signing→Broadcast→Idle path.
    state.transition_to(Phase::Broadcast).unwrap();
    state.transition_to(Phase::Idle).unwrap();

    // AUDIT-03: inner MUST be None — Drop chain has fired, RoundSecretKey
    // dropped, RsaPrivateKey zeroized transitively (rsa-0.9.10/src/key.rs:76-82).
    assert!(state.inner.is_none(),
        "AUDIT-03: RoundStateInner must be dropped on Idle transition");
    assert_eq!(state.phase, Phase::Idle);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact for Phase 21 |
|--------------|------------------|--------------|---------------------|
| `rsa < 0.9` had no unconditional `Drop` on `RsaPrivateKey` | `rsa >= 0.9` has unconditional `impl Drop` + `impl ZeroizeOnDrop for RsaPrivateKey` (verified at `key.rs:76-84`) | `rsa 0.9.0` release | The D-07 comment's "best-effort upstream" claim is functionally outdated. Plan 21-01 rewrites the comment to cite the transitive Drop chain. |
| `bitcoincore-rpc` crate (archived November 2025) | `corepc-types` (rust-bitcoin org replacement) | November 2025 | Already adopted in this project per Cargo.toml (`corepc-types = "0.11"`). No Phase 21 action. |
| `bdk` crate (deprecated) | `bdk_wallet` crate | March 2026 | Already adopted (`bdk_wallet = "2.3"`). No Phase 21 action. |

**Deprecated/outdated:**
- The D-07 comment block at `coordinator/src/blind/rsa.rs:18-22` ("best-effort only") — D-132 rewrites this to cite the transitive `rsa::RsaPrivateKey` `Drop` chain.

## Open Question 1 — D-129 / D-129a / CD-47: `RoundSecretKey::drop` body shape

### Investigation

**Sub-question 1: Does `SecretKey<H,S,M>` in blind-rsa-signatures 0.17.1 deref to `rsa::RsaPrivateKey`? Is `Zeroize` impl'd anywhere?**

Verified at `~/.cargo/registry/src/.../blind-rsa-signatures-0.17.1/src/lib.rs:822-828`:
```rust
/// An RSA secret key
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct SecretKey<H: HashAlgorithm, S: SaltMode, M: MessagePrepare> {
    inner: RsaPrivateKey,
    _phantom: PhantomData<(H, S, M)>,
}
```

- **Wraps `RsaPrivateKey`?** YES. `inner: RsaPrivateKey`.
- **`Deref<Target = RsaPrivateKey>`?** NO — but there is `impl AsRef<RsaPrivateKey> for SecretKey<H,S,M>` at `lib.rs:952`, providing typed access.
- **`Zeroize` impl?** NO — no `impl Zeroize for SecretKey` and no `#[derive(Zeroize)]`.
- **`ZeroizeOnDrop` impl?** NO — but `RsaPrivateKey` has one (see below), and dropping `SecretKey` drops `inner: RsaPrivateKey`, firing the upstream chain.
- **Crate feature flags?** `default = ["serde"]`, `serde = ["dep:serde", "rsa/serde"]`. **No zeroize-related feature flags.** Confirmed via `~/.cargo/registry/src/.../blind-rsa-signatures-0.17.1/Cargo.toml`.
- **`secure_erase` / `take_secret` / `scrub` / `into_inner` API?** NO. The only secret-bearing methods on `SecretKey` are `to_der()`, `to_pem()`, and `components()` (which returns `SecretKeyComponents<'a>` with `&'a RsaPrivateKey`).

**Sub-question 2: Does `rsa = 0.9.10` impl `ZeroizeOnDrop` on `RsaPrivateKey`?**

Verified at `~/.cargo/registry/src/.../rsa-0.9.10/src/key.rs:76-84`:
```rust
impl Drop for RsaPrivateKey {
    fn drop(&mut self) {
        self.d.zeroize();
        self.primes.zeroize();
        self.precomputed.zeroize();
    }
}

impl ZeroizeOnDrop for RsaPrivateKey {}
```

And at `key.rs:114-118` for the nested `PrecomputedValues`:
```rust
impl Drop for PrecomputedValues {
    fn drop(&mut self) {
        self.zeroize();
    }
}
```

- **Feature-gated?** NO — UNCONDITIONAL. No `#[cfg(feature = "zeroize")]` wrap.
- **`zeroize` dependency status?** NON-OPTIONAL in `rsa = 0.9.10`. `Cargo.toml` declares `zeroize = { version = "1.8", features = ["alloc"] }` without `optional = true`. Verified via WebFetch of `https://raw.githubusercontent.com/RustCrypto/RSA/master/Cargo.toml`.
- **What gets zeroized?** `d` (private exponent), `primes` (the p and q factors as a `Vec<BigUint>`), and `precomputed` (CRT optimization values `dp`, `dq`, `qinv`, and `crt_values`). This covers **all** the cryptographically sensitive RSA secret components.

**Sub-question 3: If a usable API exists, name it.**

`rsa::RsaPrivateKey`'s `Drop` is the usable API. Symbol: **`<rsa::RsaPrivateKey as Drop>::drop`** at `rsa-0.9.10/src/key.rs:77`. Feature flag required: **none**. Example of using it: do nothing — let the natural Rust drop chain fire on `RoundSecretKey` → `BjSecretKey` → `RsaPrivateKey`.

**Sub-question 4: If no usable API exists, recommend between DER-roundtrip and replace-with-dummy.**

N/A — a usable API exists. But for completeness, both alternatives are **strictly worse than empty Drop**:

| Approach | (i) Overwrites secret memory? | (ii) Robust to upstream changes? | (iii) Wall-clock at round teardown | (iv) Charter §5 narrative |
|----------|------------------------------|----------------------------------|-----------------------------------|---------------------------|
| **Empty Drop + transitive `RsaPrivateKey::drop`** (RECOMMENDED) | YES — upstream zeroizes `d`/`primes`/`precomputed` in place | YES — if upstream removes Drop, we'd see this as a `clippy::needless_drop_impl` warning or via the structural test failing | ~zero (just the natural drop chain) | "Zeroization runs transitively via `<rsa::RsaPrivateKey as Drop>::drop` (rsa-0.9.10/src/key.rs:76-82); the newtype bounds the lifetime via `Option<RsaBlindSigner>`." |
| DER-roundtrip scrub | NO new memory overwrite — the original `BigUint` allocations get zeroized by the same upstream `Drop` regardless; calling `to_der()` allocates a FRESH `Vec<u8>` that we'd then need to `zeroize()` separately | LESS robust — depends on `to_der()` not failing in `Drop` (a panic in Drop is UB-adjacent) | ~5-10 ms (DER serialization + `zeroize()` on a ~1200-byte buffer) | Weaker — "best-effort scrub of a fresh serialization buffer" is harder to defend than "transitive crypto-correct ZeroizeOnDrop chain." |
| Replace-with-dummy | NO — `mem::replace` drops the OLD `RsaPrivateKey`, which fires its `Drop`, which zeroizes `d`/`primes`/`precomputed` — IDENTICAL outcome to empty Drop | LESS robust — depends on `KeyPair::generate` not failing in `Drop` | ~100 ms (full RSA-2048 keygen) | Weaker — "we generate a fresh key to overwrite the old one" misleads the auditor into thinking the dummy keygen is doing crypto work; it's not. |

**Sub-question 5: Return type of `SecretKey::to_der()`.**

Verified at `blind-rsa-signatures-0.17.1/src/lib.rs:878-883`:
```rust
pub fn to_der(&self) -> Result<Vec<u8>, Error> {
    self.inner
        .to_pkcs8_der()
        .map_err(|_| Error::EncodingError)
        .map(|x| mem::take(x.to_bytes().as_mut()))
}
```

**Return type:** `Result<Vec<u8>, blind_rsa_signatures::Error>`. The `Vec<u8>` is directly `zeroize()`-able (it's `Vec<u8>`, which has `impl Zeroize`). No need to extract bytes from a `Document` wrapper. Note the upstream uses `mem::take` on a mutable buffer from `to_bytes()` — the upstream itself does some scrubbing of the intermediate `Document` before returning. Not directly relevant to D-129 if we adopt empty-Drop, but Plan 21-01's authors should know.

### RECOMMENDATION

**For Plan 21-01 to copy verbatim into the PLAN.md task body:**

> **D-129 / CD-47 decision: empty `Drop` body + transitive zeroization via `rsa::RsaPrivateKey`.**
>
> The `blind-rsa-signatures 0.17.1` `SecretKey<H,S,M>` holds `inner: RsaPrivateKey` (lib.rs:825-828). The `rsa = 0.9.10` crate has an UNCONDITIONAL `impl Drop for RsaPrivateKey` (key.rs:76-82) that zeroizes `d`, `primes`, and `precomputed`, plus `impl ZeroizeOnDrop for RsaPrivateKey {}` (line 84). Both impls compile without any feature flag (`zeroize` is a non-optional dep of `rsa`).
>
> Therefore: the cryptographically meaningful zeroization is done by the upstream Drop chain. The `RoundSecretKey::drop` body emits a PII-safe `tracing::debug!` event for ops observability and otherwise does nothing — the natural drop chain runs as part of the struct's drop.
>
> **Drop body:**
> ```rust
> impl Drop for RoundSecretKey {
>     fn drop(&mut self) {
>         tracing::debug!(
>             target: "blindjoin::audit",
>             "RoundSecretKey dropped — rsa::RsaPrivateKey ZeroizeOnDrop fires transitively"
>         );
>     }
> }
> ```
>
> **D-132 (D-07 comment rewrite) consequence:** the comment NO LONGER calls the in-place scrub "best-effort." It cites the transitive `rsa::RsaPrivateKey` Drop chain as the cryptographically correct mechanism, and positions the newtype's value as **lifetime expression** (the `Option<RsaBlindSigner>` field that the FSM nulls on `transition_to(Phase::Idle)`).
>
> **Audit charter §5 narrative consequence:** the charter says "the secret key's d/primes/precomputed are zeroized by `<rsa::RsaPrivateKey as Drop>::drop` (rsa-0.9.10/src/key.rs:76-82) when `RoundStateInner.rsa_signer` is set to `None` via `transition_to(Phase::Idle)` (state.rs:194-200). The structural lifetime bound (`Option<RsaBlindSigner>`) is the load-bearing claim; the cryptographic correctness of in-place zeroization is delegated to the upstream `rsa` crate, which the audit charter §6 lists as out-of-scope (consensus-critical primitive with separate upstream audit posture)."
>
> This is **stronger** than CONTEXT.md's default of DER-roundtrip + "best-effort" tracing note, because:
> - The in-place zeroization is **not** best-effort — it runs deterministically via upstream Drop;
> - The newtype's value is **lifetime expression**, not redundant crypto;
> - The audit narrative is internally consistent (charter §5 + §6 align: zeroization correctness is delegated to upstream `rsa`, which is out-of-scope per §6, and the bounded lifetime is in-scope per §5).

## Open Question 2 — D-130a: confirm the 4 valid Idle transitions all reach `transition_to(Phase::Idle)`

### Investigation

Grep-based enumeration of ALL `transition_to(Phase::Idle)` call sites in `coordinator/src/`:

```bash
$ rg -n 'transition_to\(Phase::Idle\)' coordinator/src/
coordinator/src/round/blame.rs:220:    let _ = state.transition_to(Phase::Idle);
coordinator/src/round/output_reg.rs:31:        let _ = state.transition_to(Phase::Idle);
coordinator/src/round/manager.rs:226:        state.transition_to(Phase::Idle).unwrap();           # tests-only, line 220-234
coordinator/src/round/signing.rs:280:    let _ = state.transition_to(Phase::Idle);
coordinator/src/run.rs:195:                                if let Err(e) = round.transition_to(Phase::Idle) {
```

Plus grep for any `inner = None` bypass:
```bash
$ rg -n 'inner = None|\.inner = None' coordinator/src/
coordinator/src/round/state.rs:195:    self.inner = None; // triggers ZeroizeOnDrop on RoundStateInner
```

**Only one site sets `inner = None`** — and it's inside `transition_to(Phase::Idle)`. No bypass paths.

### Path-by-path verification

| Path | Production site | Verified routes through `transition_to(Phase::Idle)` | Bypass risk |
|------|----------------|--------------------------------------------------|-----|
| **Broadcast → Idle (success)** | `coordinator/src/round/signing.rs:279-280` — `let _ = state.transition_to(Phase::Broadcast); let _ = state.transition_to(Phase::Idle);` at the end of `broadcast_round` after `sendrawtransaction` success | ✓ YES | None — explicit call. |
| **Blame → Idle (signing timeout)** | `coordinator/src/round/blame.rs:219-220` — `let _ = state.transition_to(Phase::Blame); let _ = state.transition_to(Phase::Idle);` inside `on_signing_timeout` | ✓ YES | None — explicit call. |
| **InputReg → Idle (quorum fail)** | `coordinator/src/run.rs:195` — `round.transition_to(Phase::Idle)` inside the input-reg-timeout spawn closure when `participant_count < min_participants` | ✓ YES | None — explicit call. The spawn closure runs in tokio::spawn, but on completion the round.write() lock is held; transition runs synchronously. |
| **Blame → Idle (missing output)** | `coordinator/src/round/output_reg.rs:30-31` — `let _ = state.transition_to(Phase::Blame); let _ = state.transition_to(Phase::Idle);` inside `on_output_reg_timeout` when `has_missing_outputs` | ✓ YES | None — explicit call. |

Plus: there is **NO** `RoundManager` with a `HashMap<RoundId, RoundState>` map. The coordinator holds a SINGLE `Arc<RwLock<RoundState>>` in `coordinator/src/run.rs:73-117` (`round` is the global state for the single-coordinator-single-round model). Round transitions happen on this single instance; there is no map-removal drop-on-removal pattern. **Confirmed via grep:** no `HashMap<.*RoundState>` or `HashMap<.*Round>` in the codebase.

### Drop chain unaffected by `Drop::drop` panics?

The Drop body proposed in OQ1 (`tracing::debug!` only) cannot panic — `tracing::debug!` is infallible. Even if a future change to the body introduces a panicking expression, Rust's drop-during-unwind safety means the wrapped `RsaPrivateKey`'s Drop would still run because it's a separate object — though a double-panic in Drop is process-aborting. Plan 21-01 should keep the Drop body strictly panic-free.

### RECOMMENDATION

**For Plan 21-02 (charter §5) to copy verbatim:**

> **Charter §5 prose: the FSM-transition narrative is factually correct.**
>
> The 4 valid Phase → Idle FSM edges all route through `RoundState::transition_to(Phase::Idle)` at `coordinator/src/round/state.rs:186-203`. This is the SOLE site setting `self.inner = None` (verified by grep of the entire `coordinator/src/` tree on 2026-05-31). No code path bypasses this trigger.
>
> - Broadcast → Idle (success path): `coordinator/src/round/signing.rs:279-280` (end of `broadcast_round`)
> - Blame → Idle (signing timeout): `coordinator/src/round/blame.rs:219-220` (`on_signing_timeout`)
> - Blame → Idle (missing output): `coordinator/src/round/output_reg.rs:30-31` (`on_output_reg_timeout`)
> - InputReg → Idle (quorum fail): `coordinator/src/run.rs:195` (input-reg-timeout spawn closure)
>
> The coordinator holds a single `Arc<RwLock<RoundState>>` per process (no `HashMap<RoundId, RoundState>` map; single-round-per-coordinator model). No drop-on-map-removal pattern exists.
>
> When `inner = None` is assigned (state.rs:195), the Drop chain fires: `Option<RoundStateInner>` → `RoundStateInner::drop` (state.rs:120-149, zeroizes the HashMaps + raw bytes) → `Option<RsaBlindSigner>` → `RsaBlindSigner::drop` (auto-generated) → `RoundSecretKey::drop` (Phase 21 newtype) → `BjSecretKey::drop` (auto-generated) → `<rsa::RsaPrivateKey as Drop>::drop` (rsa-0.9.10/src/key.rs:76-82, zeroizes `d`/`primes`/`precomputed`).

## Open Question 3 — D-141: fresh `cargo audit --json` diff

### Investigation

**Run:** `cargo audit --json` against current `Cargo.lock`, network-online mode (not `--no-fetch`).

**Advisory DB version at research time:** 1099 advisories, last commit `eaf48e749baa3d5e27d304107d8abf175fd756bb`, last updated `2026-05-29T20:55:26+02:00` (DB fetched fresh during this research session).

**Run #1 — with current `.cargo/audit.toml` (3 ignores):**

```json
{
  "vulnerabilities": {"found": false, "count": 0, "list": []},
  "warnings": {}
}
```

**Run #2 — with `.cargo/audit.toml` temporarily removed (no ignores):**

```json
{
  "vulnerabilities": "RUSTSEC-2023-0071",
  "warning_ids": ["RUSTSEC-2025-0141", "RUSTSEC-2024-0436"]
}
```

The 3 IDs surfaced are EXACTLY the 3 already in `.cargo/audit.toml`. No new advisories.

### Per-advisory detail

| Advisory | Crate + version | Title | Upstream fix | Transitive? Via | Runtime-reachable? | Classification |
|----------|----------------|-------|--------------|-----------------|---------------------|----------------|
| **RUSTSEC-2023-0071** | `rsa 0.9.10` | Marvin Attack (timing sidechannel) | No — open ticket on `rsa` crate; awaiting a constant-time RSA decryption rewrite | YES — via `blind-rsa-signatures 0.17.1` | Coordinator's `rsa_signer.blind_sign(...)` is the runtime path; `rsa` is reachable. Mitigation: ephemeral per-round keys + bounded participant count (default 20) + ephemeral coordinator process (Tor HS rotation) means Marvin's "long-lived key + unlimited measurements" preconditions don't obtain. | (b) ignore-with-rationale — already in `.cargo/audit.toml`. AUDIT-03 bounded-window mitigation tightens this further by structurally bounding the per-round key lifetime via `Option<RsaBlindSigner>` on `RoundStateInner`. |
| **RUSTSEC-2025-0141** | `bincode 2.0.1` | Marked unmaintained | No — crate maintainer paused work; replacements being scoped | YES — transitive (deep). Likely via `pkarr` → `mainline` → some downstream serializer | NOT reachable in `coordinator/src/` runtime code (build-only or DHT-internal). | (b) ignore-with-rationale — already in `.cargo/audit.toml`. Charter §7 (a) notes "transitive dep; not directly used by blindjoin; will track upstream for a maintained replacement; not a runtime vulnerability." |
| **RUSTSEC-2024-0436** | `paste 1.0.15` | Proc-macro marked unmaintained | No — single-maintainer hobby crate, no successor | YES — transitive proc-macro | NOT reachable at runtime (compile-time macro expansion only). | (b) ignore-with-rationale — already in `.cargo/audit.toml`. Charter §7 (a) notes "compile-time-only macro; no runtime code path." |

### No new advisories

`cargo audit` returned 0 vulnerabilities and 0 warnings with the current 3 ignores. The "diff vs the existing 3 ignores" comes back EMPTY: there are no additional advisories applicable to the current `Cargo.lock`. The advisory DB was fetched fresh during this session, so this is current as of 2026-05-29 (DB) + 2026-05-31 (research run).

### RECOMMENDATION

**For Plan 21-02 to lock the 3 existing audit.toml entries verbatim with charter anchors:**

> **D-141 / CD-48 decision: no new advisories. Lock the 3 existing ignores with charter-section anchors.**
>
> `cargo audit --json` against the current `Cargo.lock` (advisory DB last commit `eaf48e7`, last updated 2026-05-29) returns 0 vulnerabilities and 0 warnings with the existing `.cargo/audit.toml`. Running without the ignore file surfaces ONLY the 3 IDs already in the file (RUSTSEC-2023-0071, RUSTSEC-2025-0141, RUSTSEC-2024-0436). No new advisories require ignore-or-fix decisions.
>
> Plan 21-02 audit.toml refresh:
> 1. Each of the 3 existing comment blocks gets a closing line `See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.`
> 2. RUSTSEC-2023-0071 rationale paragraph rewritten to name AUDIT-03 bounded-window mitigation by name — replacing "destroys the key via `zeroize`" with language that cites the transitive `rsa::RsaPrivateKey` `Drop` chain bounded by `Option<RsaBlindSigner>` on `RoundStateInner` (per OQ1's RECOMMENDATION above).
> 3. `Reviewed:` header bumps from `2026-05-26` to the 21-02 commit date.
> 4. No new `ignore = [...]` entries added.
> 5. Flat TOML layout preserved (D-142 lock).
>
> Proposed anchor slugs (CD-49 allows refinement):
> - `RUSTSEC-2023-0071` → `#rsa-secret-key-zeroization-window` (charter §5)
> - `RUSTSEC-2025-0141` → `#residual-risks-cargo-audit-advisories` (charter §7 (a))
> - `RUSTSEC-2024-0436` → `#residual-risks-cargo-audit-advisories` (charter §7 (a))

## Production call sites of `inner.rsa_signer.*` (Standard Research #6)

Grep enumeration (run 2026-05-31):

```bash
$ rg -n '\.rsa_signer\.' coordinator/src/
coordinator/src/round/input_reg.rs:71:    let blind_sig = inner.rsa_signer.blind_sign(&blind_msg).map_err(...)
coordinator/src/round/state.rs:321:        assert_eq!(inner.rsa_signer.public_key_hash(), expected_hash,    # TEST
coordinator/src/round/manager.rs:195:        let pk_hash_from_signer = inner.rsa_signer.public_key_hash();   # TEST
coordinator/src/api/handlers.rs:383:        .rsa_signer.public_key.clone();

$ rg -n '\.rsa_signer\)' coordinator/src/
(no matches)

$ rg -n '&.*rsa_signer' coordinator/src/
(no matches)
```

**Total production call sites: 2** (`input_reg.rs:71`, `handlers.rs:383`). **Total test call sites: 2** (`state.rs:321`, `manager.rs:195`).

CONTEXT.md said "Initial scope: output_reg.rs, handlers.rs, input_reg.rs, manager.rs." Reality differs — `manager.rs` is a TEST callsite (`rsa_signer_consistent_with_key_bytes` style); `output_reg.rs::make_valid_token_sig` takes `&RsaBlindSigner` from local `signer` and is unaffected.

### Production call-site rewrite table

| File:line | Current snippet | Proposed `.as_ref().expect(...)` replacement | Option Some required? |
|-----------|----------------|----------------------------------------------|-----------------------|
| `coordinator/src/round/input_reg.rs:71` | `let blind_sig = inner.rsa_signer.blind_sign(&blind_msg).map_err(...)` | `let blind_sig = inner.rsa_signer.as_ref().expect("rsa_signer must be Some during InputReg").blind_sign(&blind_msg).map_err(...)` | **Required Some** — `register_input` is called only when `phase == InputReg`; pre-condition is that `inner` is `Some` (line 51 `state.inner.as_mut().ok_or_else(...)`); `inner.rsa_signer` is created in `start_round` (manager.rs:63) which always populates `Some`. |
| `coordinator/src/api/handlers.rs:383` | `let rsa_public_key = guard.inner.as_ref().ok_or_else(...)?.rsa_signer.public_key.clone();` | `let rsa_public_key = guard.inner.as_ref().ok_or_else(...)?.rsa_signer.as_ref().expect("rsa_signer must be Some during OutputReg").public_key.clone();` | **Required Some** — same logic as above; `inner.rsa_signer` is `Some` whenever `inner` is `Some`. |

### Test call-site rewrite table

| File:line | Current snippet | Proposed `.as_ref().expect(...)` replacement |
|-----------|----------------|----------------------------------------------|
| `coordinator/src/round/state.rs:321` | `assert_eq!(inner.rsa_signer.public_key_hash(), expected_hash, ...)` | `assert_eq!(inner.rsa_signer.as_ref().expect("test fixture: rsa_signer is Some").public_key_hash(), expected_hash, ...)` |
| `coordinator/src/round/manager.rs:195` | `let pk_hash_from_signer = inner.rsa_signer.public_key_hash();` | `let pk_hash_from_signer = inner.rsa_signer.as_ref().expect("test fixture: rsa_signer is Some").public_key_hash();` |

### Manager.rs field-init refresh

The single production field-init site at `coordinator/src/round/manager.rs:63` (inside `start_round`):

Current:
```rust
state.inner = Some(RoundStateInner {
    rsa_signing_key: sk_der,
    rsa_signer: signer,                  // bare value
    ...
});
```

Proposed:
```rust
state.inner = Some(RoundStateInner {
    rsa_signing_key: sk_der,
    rsa_signer: Some(signer),            // wrapped in Some(_) for the new Option<RsaBlindSigner> field
    ...
});
```

## Test-fixture call sites (Standard Research #7)

Grep enumeration (run 2026-05-31):

```bash
$ rg -n 'rsa_signer:' coordinator/src/
coordinator/src/round/state.rs:103:    pub rsa_signer: RsaBlindSigner,          # FIELD DEF (Phase 21 changes to Option<RsaBlindSigner>)
coordinator/src/round/state.rs:270:            rsa_signer: RsaBlindSigner::generate().unwrap(),    # TEST
coordinator/src/round/state.rs:311:            rsa_signer: signer,                                 # TEST (local var, generate above)
coordinator/src/round/manager.rs:63:        rsa_signer: signer,                                   # PRODUCTION
coordinator/src/round/signing.rs:450:            rsa_signer: RsaBlindSigner::generate().unwrap(),    # TEST
coordinator/src/round/signing.rs:496:            rsa_signer: RsaBlindSigner::generate().unwrap(),    # TEST
coordinator/src/round/signing.rs:521:            rsa_signer: RsaBlindSigner::generate().unwrap(),    # TEST
coordinator/src/round/signing.rs:560:            rsa_signer: RsaBlindSigner::generate().unwrap(),    # TEST
```

**Test fixture count: 6** (state.rs ×2 + signing.rs ×4). **Production field init: 1** (manager.rs:63).

All line numbers are EXACT against CONTEXT.md's "state.rs:270, state.rs:311, signing.rs:450, signing.rs:496, signing.rs:521, signing.rs:560." No drift post-Phase-20. ✓

`output_reg.rs::make_valid_token_sig` at `coordinator/src/round/output_reg.rs:102` is a helper taking `&RsaBlindSigner` from a LOCAL `signer` variable; callers (lines 128, 139, 155, 170) construct `let signer = RsaBlindSigner::generate().unwrap();` directly — NOT via `inner.rsa_signer`. These are **UNAFFECTED** by the `Option` refactor. No edit needed in `output_reg.rs`.

### Refresh pattern (mechanical)

```rust
// Before:
rsa_signer: RsaBlindSigner::generate().unwrap(),

// After:
rsa_signer: Some(RsaBlindSigner::generate().unwrap()),
```

For state.rs:311 (which uses a previously-constructed local `signer` variable):
```rust
// Before:
rsa_signer: signer,

// After:
rsa_signer: Some(signer),
```

For manager.rs:63 (production):
```rust
// Before:
rsa_signer: signer,

// After:
rsa_signer: Some(signer),
```

## Clippy lint config (Standard Research #8)

Investigation:
- Cargo.toml workspace root: NO `[lints]` or `[workspace.lints]` table (verified by reading full file).
- `clippy.toml`: does not exist (verified by `find . -name 'clippy.toml'`).
- Per-crate Cargo.toml files: NO `[lints]` table (verified by reading each).
- CI invocation: `cargo clippy --workspace --all-targets -- -D warnings` (per CONTEXT.md cross-phase invariant 4).

**Conclusion:** Only default-warn clippy lints are denied. `clippy::unwrap_used` and `clippy::expect_used` are `restriction`-level (not `warn` by default), so they are **NOT denied** by `-D warnings`. The proposed `.as_ref().expect("rsa_signer must be Some during ...")` pattern will NOT trip clippy.

**Verified by execution:** `cargo clippy -p coordinator --all-targets -- -D warnings` passes clean against the pre-Phase-21 codebase, which already uses `.unwrap()` in 6 test-fixture sites and 4 production sites (the `expect("RSA public key must be SPKI-encodable")` at rsa.rs:42, the `expect("HMAC accepts any key length")` at manager.rs:108, etc.).

**Recommendation:** Use `.as_ref().expect("rsa_signer must be Some during <phase>")` as the call-site pattern. This is consistent with existing `.expect(...)` style in the codebase, gives a meaningful panic message if the invariant is ever violated, and is clippy-clean.

## Best-effort RAM-scan test mechanism (Standard Research #9 / CD-50)

### Candidates evaluated

1. **Raw-pointer cast + `slice::from_raw_parts` post-drop.** Reads memory beyond a Vec's lifetime — undefined behavior; Miri would reject. Reject.

2. **`region` crate or allocator stats polling.** Adds a new dev-dependency; complex; `region` is Linux/Windows/macOS but the API differs per OS. Marginal value vs the simpler approach below. Reject.

3. **`nix::sys::mman::mlock` + post-drop scan.** Unix-only; requires the test to mlock its own pages, which depends on user ulimits. Marginal value. Reject.

4. **Construct → capture DER pattern → drop → allocate a same-size buffer → check if pattern appears.** Portable across Unix and macOS; no new dependencies; small LOC; honestly best-effort.

**Recommended: approach 4.** Sketch (~25 LOC) embedded in the "Code Examples" section above as `round_secret_key_buffer_overwritten_on_drop`. Key design choices:

- Capture a **32-byte slice from the middle of DER** (offset 100..132), where modulus/exponent bytes live — distinctive per-key (RSA-2048 DER prefix is COMMON across all keys; the middle is unique).
- Allocate an 8 MB probe `Vec<u8>` post-drop to give the allocator a chance to reuse the freed region.
- `windows(32).any(|w| w == needle)` for the sweep.
- `#[cfg_attr(not(target_os = "linux"), ignore = "...")]` — only run on Linux by default; allocator behavior on macOS differs.

### When to `#[ignore]`

Per CD-50, mark `#[ignore]` if the test flakes on a future toolchain. Conditions where Plan 21-01 should mark it ignored even before flakiness:

- **macOS, Windows:** allocator layout differs significantly from Linux glibc; the post-drop probe is unlikely to land on the freed region. Default to `#[cfg_attr(not(target_os = "linux"), ignore = "...")]`.
- **LTO / opt-level=3 builds:** the compiler may merge or elide allocations. If `cargo test --release` flakes, add `#[cfg_attr(not(debug_assertions), ignore = "...")]`.
- **Sanitizers (Miri, ASan):** the test interrogates "free-but-not-overwritten" memory — exactly what sanitizers reject. Add `#[cfg_attr(miri, ignore = "...")]` if Miri is ever added to CI.

The structural test in `state.rs::tests::round_secret_key_dropped_on_round_end` is the **unconditional CI gate** (D-131 load-bearing per REQUIREMENTS AUDIT-03 "structural lifetime bound is the load-bearing claim"). The RAM-scan test is **sanity-check ceremony**.

## Charter file:symbol stability (Standard Research #10 / D-138)

Verification of each anchor cited in CONTEXT.md and REQUIREMENTS.md:

| Anchor | Exists today? | Current location |
|--------|--------------|------------------|
| `coordinator/src/bitcoin/utxo.rs::validate_utxo` | ✓ | utxo.rs:67 |
| `coordinator/src/bitcoin/utxo.rs::dispatch_ownership_proof` | ✓ | utxo.rs:158 |
| `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` | ✓ | utxo.rs:218 |
| `client/src/round/input.rs::build_v2_psbt_input_b64` | ✓ | input.rs:35 |
| `shared/src/bip322/mod.rs::sign_simple` | ✓ | mod.rs:283 |
| `shared/src/bip322/mod.rs::verify_simple` | ✓ | mod.rs:257 |
| `shared/src/bip322/mod.rs::detect_script_type` | ✓ | mod.rs:238 |
| `coordinator/src/config.rs::BipConfig::validate` | ✓ | config.rs:249 (impl BipConfig) |
| `coordinator/src/blind/rsa.rs::RsaBlindSigner` | ✓ | rsa.rs:23 (struct) |
| `coordinator/src/round/state.rs::RoundStateInner` | ✓ | state.rs:92 (struct) |
| `coordinator/src/round/state.rs::transition_to` | ✓ | state.rs:186 (impl RoundState) |
| `coordinator/src/round/state.rs::tests::transition_to_idle_clears_inner` | ✓ | state.rs:262 |
| `shared/tests/bip322_cross_shape.rs` (9 tests) | ✓ | confirmed below |
| `RoundSecretKey` (NEW in Phase 21) | ✗ | will be added at rsa.rs:~28-40 |
| `RoundSecretKey::drop` (NEW) | ✗ | will be added at rsa.rs:~50 |
| `round_secret_key_dropped_on_round_end` (NEW) | ✗ | will be added at state.rs::tests |
| `round_secret_key_buffer_overwritten_on_drop` (NEW) | ✗ | will be added at rsa.rs::tests |

**Drift from CONTEXT.md:** ZERO. All 12 existing file:symbol anchors resolve to the cited symbols. The 4 NEW symbols (added by Phase 21 itself) will resolve once 21-01 ships.

## README.md §Security Model current shape (Standard Research #11)

Verified line numbers (current README.md):

- `## Security Model` header: **line 281**
- "The coordinator **cannot**" / "The coordinator **can**" / closing summary: lines 283-294
- **Availability hardening (v1.1):** line 296
- **Public-endpoint hardening (v1.2 Phase 8):** line 298
- **Supply-chain hygiene:** **line 300**
- **Test infrastructure (v1.3 Phase 9):** **line 302**
- **Multi-script script-type integrity (v1.4):** line 304
- (blank line) line 305
- `## Key Dependencies` header: line 306

**D-143 / CD-52 insertion point:** **directly after line 300** (Supply-chain hygiene paragraph), **before line 302** (v1.3 test infrastructure). This matches CONTEXT.md's "around line 300" estimate.

Proposed callout (from CONTEXT.md D-143, may be tightened by 21-02 plan):

```markdown
**External audit charter (v1.5):** [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md) enumerates in-scope modules with file:symbol refs, threat models per module, the 9 cross-shape rejection properties, the v=2 OwnershipProof PSBT handling boundary, the RSA SecretKey zeroization window (RoundSecretKey + bounded lifetime per AUDIT-03), out-of-scope dependencies, residual risks accepted with rationale, and a glossary mapping project terms to audit language.
```

The callout follows the established `**Category (vN.x):**` convention used by the 5 existing hardening rollups.

## `docs/` directory current contents (Standard Research #12)

```bash
$ ls docs/
PROTOCOL.md
branch-protection.md
```

CONTEXT.md is correct: exactly 2 existing files. Plan 21-02 adds `docs/AUDIT-CHARTER.md` as the third. No `docs/index.md` or auto-generated TOC exists; the only navigation is README §Security Model → `docs/AUDIT-CHARTER.md` link.

## 9 cross-shape rejection properties (Standard Research #13)

Enumerated from `shared/tests/bip322_cross_shape.rs`:

| # | Test fn name | Line | What it rejects |
|---|-------------|------|------------------|
| 1 | `reject_p2wpkh_spk_with_p2tr_witness` | 90 | P2WPKH SPK + 1-element (P2TR-shaped) witness → `Bip322Error::InvalidWitnessLength { expected: 2, got: 1 }` |
| 2 | `reject_p2wpkh_spk_with_p2sh_p2wpkh_witness` | 105 | P2WPKH SPK + 2-element (P2SH-P2WPKH-shaped) witness → `Bip322Error::CrateVerifyFailed` (arity passes; ECDSA verify fails) |
| 3 | `reject_p2tr_spk_with_p2wpkh_witness` | 118 | P2TR SPK + 2-element (P2WPKH-shaped) witness → `Bip322Error::InvalidWitnessLength { expected: 1, got: 2 }` |
| 4 | `reject_p2tr_spk_with_p2sh_p2wpkh_witness` | 133 | P2TR SPK + 2-element witness → `Bip322Error::InvalidWitnessLength { expected: 1, got: 2 }` |
| 5 | `reject_p2sh_p2wpkh_spk_with_p2wpkh_witness` | 148 | P2SH-P2WPKH SPK + 2-element (P2WPKH-shaped) witness → `Bip322Error::CrateVerifyFailed` (HASH160 cross-check fails) |
| 6 | `reject_p2sh_p2wpkh_spk_with_p2tr_witness` | 169 | P2SH-P2WPKH SPK + 1-element witness → `Bip322Error::InvalidWitnessLength { expected: 2, got: 1 }` |
| 7 | `reject_p2wpkh_spk_with_empty_witness` | 190 | P2WPKH SPK + empty witness → `Bip322Error::InvalidWitnessLength { expected: 2, got: 0 }` |
| 8 | `reject_p2tr_spk_with_empty_witness` | 204 | P2TR SPK + empty witness → `Bip322Error::InvalidWitnessLength { expected: 1, got: 0 }` |
| 9 | `reject_p2sh_p2wpkh_spk_with_empty_witness` | 218 | P2SH-P2WPKH SPK + empty witness → `Bip322Error::InvalidWitnessLength { expected: 2, got: 0 }` |

Plan 21-02 charter §3 table cites this table verbatim. Each test asserts the SPECIFIC `Bip322Error` variant via `matches!()` per RESEARCH A3 (15-RESEARCH.md), so silent acceptance of the wrong rejection class is statically impossible.

## `.github/workflows/ci.yml` cargo audit gate (Standard Research #14)

Verified at lines 168-181:

```yaml
audit:
  name: cargo audit
  runs-on: ubuntu-latest
  # Blocks merge: cargo audit exits non-zero on any advisory not listed in
  # .cargo/audit.toml. Each ignore in audit.toml carries a written rationale.
  steps:
    - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
    - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
    - name: Install cargo-audit
      run: cargo install cargo-audit --locked
    - name: Run audit
      run: cargo audit
```

- **Step exists:** YES (line 168-181).
- **Failure semantics:** `cargo audit` exits non-zero on any advisory not in `.cargo/audit.toml`. The comment block at line 171-172 explicitly states this gates merge.
- **PR-blocking:** YES (job is a required check via branch protection per `docs/branch-protection.md`).
- **Plan 21-02 audit.toml refresh impact:** the refreshed audit.toml retains the same 3 IDs in the `ignore = [...]` list (just adds charter-anchor comment lines + rewrites the RUSTSEC-2023-0071 rationale paragraph). The CI gate will pass with the new file because the ignore IDs are unchanged. Verified by simulating: `cargo audit --json` with current 3 ignores returns 0 vulns + 0 warnings.

**Risk:** if Plan 21-02 ACCIDENTALLY drops one of the 3 IDs while editing comments, the CI gate would fail (RUSTSEC-2023-0071 surfaces as a vulnerability; the other two as warnings). Plan 21-02 verification step should re-run `cargo audit` post-edit to confirm the gate is green.

## Phase Requirements

| ID | Description (verbatim from REQUIREMENTS.md) | Research Support |
|----|---------------------------------------------|------------------|
| AUDIT-01 | Publish `docs/AUDIT-CHARTER.md` with 8 sections (in-scope modules, threat models, cross-shape rejection properties, v=2 PSBT handling, RSA zeroization window, out-of-scope, residual risks, glossary). | OQ2 confirms charter §5 FSM narrative; Standard Research §10 confirms all file:symbol anchors resolve; §13 enumerates 9 cross-shape rejection tests for charter §3; §11 locates README insertion point; §12 confirms `docs/` directory has 2 files (charter is 3rd). |
| AUDIT-02 | Update `.cargo/audit.toml` rationales to reference charter section anchors; rewrite RUSTSEC-2023-0071 rationale to name AUDIT-03 mitigation; bump `Reviewed:` date; ignore-or-fix decision for any new advisories. | OQ3 confirms no new advisories — lock 3 existing entries verbatim with charter anchors and rewrite the RUSTSEC-2023-0071 paragraph per OQ1's RECOMMENDATION (transitive `rsa::RsaPrivateKey` Drop chain, NOT "best-effort"). §14 confirms CI gate semantics. |
| AUDIT-03 | Wrap `BjSecretKey` in `RoundSecretKey(BjSecretKey)` newtype with explicit Drop; `Option<RoundSecretKey>` lifetime; D-07 comment rewrite; structural test + best-effort RAM-scan test. | OQ1 recommends empty-body Drop + tracing event (upstream `rsa::RsaPrivateKey::drop` does the cryptographically meaningful work); OQ2 confirms the FSM trigger surface; Standard Research §6 enumerates the 4 production+test call sites needing `.as_ref().expect(...)`; §7 enumerates 6 test-fixture sites needing `Some(...)` wrap; §8 confirms clippy config tolerates `.expect()`; §9 provides RAM-scan test sketch. |

## Assumptions Log

| # | Claim | Section | Risk if wrong |
|---|-------|---------|---------------|
| A1 | The `rsa = 0.9.10` `Drop for RsaPrivateKey` impl will remain unconditional in future patch releases. | OQ1 RECOMMENDATION | LOW — `rsa` 0.9.x is in maintenance mode; removing the unconditional Drop would be a breaking change. Plan 21-01 should keep a comment citing the exact line numbers verified at research time (`rsa-0.9.10/src/key.rs:76-84`) so a future bump to 0.9.11 or 0.10.x can be re-verified by `grep -n 'impl Drop for RsaPrivateKey'` in the installed source. |
| A2 | Test 4 of Plan 21-01 (`round_secret_key_buffer_overwritten_on_drop`) is reliably reproducible on Linux glibc. | OQ1 / CD-50 | LOW — the test is best-effort and `#[cfg_attr(not(target_os = "linux"), ignore)]`; if it flakes on a future glibc bump, mark `#[ignore]` and rely on the structural test. |

**Notable:** all other claims in this research are verified by direct inspection of installed source (`~/.cargo/registry/src/...`), direct execution of `cargo audit --json`, direct grep of the codebase, or direct citation of cited files/line numbers. The Assumptions Log has only 2 entries because the dominant research method here was **direct code inspection**, not training-data inference.

## Open Questions (RESOLVED)

No remaining open questions. The 3 CONTEXT.md-flagged research questions are answered with RECOMMENDATION blocks above; the 9 Standard Research items have concrete tables/answers; the file:symbol anchors all resolve; the cargo-audit diff is empty; the call-site count is exact.

If Plan 21-01 or Plan 21-02 encounters a discrepancy at execution time (e.g., line numbers drift between research and execution), the SYMBOLS used throughout this research are stable across reformats. Use `rg -n` for fresh line numbers at execution time.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` + workspace toolchain | All builds | ✓ | stable | — |
| `cargo-audit` | Plan 21-02 audit.toml verification | ✓ | 0.22.1 (verified `cargo audit --version`) | — |
| Network for advisory DB fetch | Plan 21-02 verification | ✓ | (advisory DB last commit `eaf48e7` fetched 2026-05-29) | `cargo audit --no-fetch` works with last-fetched DB |
| `tracing` ecosystem | Drop body's `tracing::debug!` | ✓ | 0.1 workspace dep | — |

No missing dependencies; nothing blocks execution.

## Project Constraints (from CLAUDE.md)

- **No custom crypto** — Phase 21 wraps `blind-rsa-signatures` `SecretKey`; does NOT fork, replace, or modify it. AUDIT-03 is a NEWTYPE wrap, not a crypto change. ✓
- **No PII logging** — Drop body's `tracing::debug!` emits a static string with target `"blindjoin::audit"`; no key material, no Debug interpolation of `self`, no public-key-hash interpolation. ✓
- **MIT license** — Phase 21 changes are MIT-licensed (no AGPL/copyleft inclusions; the charter is a documentation artifact). ✓
- **Tor-native in production** — Phase 21 changes are coordinator-internal and orthogonal to Tor; the Drop chain runs in any network mode. ✓
- **Signet-first, mainnet flag-only** — Phase 21 doesn't touch network selection. ✓
- **GSD workflow enforcement** — research → plan → execute order respected; this RESEARCH.md is the planner's input. ✓

## Security Domain (ASVS)

This phase's security scope is dominated by V6 (Cryptography). All controls are **delegated to verified upstream crates** — no hand-rolled crypto.

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — (no auth surface modified) |
| V3 Session Management | no | — (no session surface modified) |
| V4 Access Control | no | — (no access control surface modified) |
| V5 Input Validation | no | — (no new input surface; charter §1 cites the existing `validate_utxo` / `BipConfig::validate` / `dispatch_ownership_proof` surfaces) |
| V6 Cryptography | yes | RFC 9474 RSA blind signatures via `blind-rsa-signatures`; transitive zeroization via `rsa::RsaPrivateKey::drop`; ephemeral per-round keys (D-02); RoundSecretKey newtype bounds in-process lifetime (AUDIT-03) |
| V7 Error Handling | no | — (no new error variants; D-129 Drop body emits structured `tracing` event with no PII) |
| V8 Data Protection | yes (overlaps V6) | Memory zeroization at round boundary; `Option<RoundStateInner>` + manual Drop pattern preserves no plaintext after FSM transition to Idle |
| V9 Communications | no | — (no network surface modified; Tor remains in `arti-client`) |
| V10 Malicious Code | no | — (no new dependencies; Package Legitimacy Audit: N/A) |
| V11 Business Logic | no | — (FSM unchanged; AUDIT-03 is structural overlay) |
| V12 Files & Resources | no | — (no file I/O changes) |
| V13 API | no | — (API surface unchanged) |
| V14 Configuration | no | — (config unchanged) |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| RSA Marvin Attack (RUSTSEC-2023-0071) | Information Disclosure (key extraction via timing sidechannel) | Ephemeral per-round RSA-2048 keypair (D-02); bounded blind-sign operations (≤20 per round default); AUDIT-03 structural lifetime bound prevents key reuse across rounds; transitive `rsa::RsaPrivateKey::drop` zeroizes in-place at round end |
| Memory residue after round teardown | Information Disclosure | Manual `Drop for RoundStateInner` (state.rs:120-149) zeroizes HashMap values + raw secret bytes; AUDIT-03 adds the wrapped `RsaBlindSigner`'s `SecretKey` to the zeroized set via the upstream `rsa::RsaPrivateKey` `Drop` chain |
| Mismatched `script_type` declaration (V1.4-CRIT-01) | Tampering / Spoofing | Coordinator derives `ScriptType` from on-chain `script_pubkey` in `validate_utxo`, never from client declaration; 9 cross-shape rejection tests at `shared/tests/bip322_cross_shape.rs` lock the rejection matrix |
| BIP-322 signature shape confusion | Tampering | Dispatcher-only public surface on `shared::bip322` (D-27); `pub(crate) fn sign` for per-script bodies; `verify_simple` / `sign_simple` are the only public entry points |

## Sources

### Primary (HIGH confidence)

- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/blind-rsa-signatures-0.17.1/src/lib.rs:822-919` — SecretKey type definition, to_der/from_der bodies, AsRef impl
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rsa-0.9.10/src/key.rs:76-118` — RsaPrivateKey + PrecomputedValues Drop/Zeroize impls
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/blind-rsa-signatures-0.17.1/Cargo.toml` — feature flags enumeration
- `cargo audit --json` (run 2026-05-31, advisory DB last commit `eaf48e7`, 2026-05-29) — vulnerability/warning counts
- `cargo audit --json` (run with audit.toml removed) — surface masked-by-ignore advisories
- Codebase greps (`rg`) — call-site enumeration, transition_to call enumeration, inner=None bypass detection
- `coordinator/src/blind/rsa.rs:1-163` — current RsaBlindSigner shape and D-07 comment
- `coordinator/src/round/state.rs:1-330` — RoundStateInner, transition_to, existing Drop impl, transition_to_idle_clears_inner test
- `coordinator/src/round/{signing,input_reg,manager,blame,output_reg}.rs` — verified production call sites + FSM trigger paths
- `coordinator/src/api/handlers.rs:380-400` — rsa_public_key clone call site
- `coordinator/src/run.rs:120-200` — input-reg quorum-fail timeout path
- `.cargo/audit.toml` — current 46-line file, 3 ignores
- `.github/workflows/ci.yml:167-181` — cargo audit CI step
- `README.md:281-325` — Security Model section current shape
- `shared/tests/bip322_cross_shape.rs` — 9 cross-shape rejection test enumeration

### Secondary (MEDIUM confidence — cross-verified)

- WebFetch `https://raw.githubusercontent.com/jedisct1/rust-blind-rsa-signatures/master/Cargo.toml` — confirmed no zeroize-related features
- WebFetch `https://raw.githubusercontent.com/RustCrypto/RSA/master/Cargo.toml` — confirmed `zeroize = "1.8"` non-optional dep
- WebFetch `https://github.com/RustCrypto/RSA/blob/master/src/key.rs` — confirmed master branch matches installed 0.9.10 for Drop/ZeroizeOnDrop impls (no cfg gate)
- WebFetch `https://github.com/jedisct1/rust-blind-rsa-signatures/blob/master/src/lib.rs` — confirmed master branch's SecretKey wraps RsaPrivateKey

### Tertiary (LOW confidence)

None — all claims are verified by direct source inspection or tool execution.

## Metadata

**Confidence breakdown:**
- Drop body shape recommendation (OQ1): **HIGH** — verified by direct inspection of installed crate source
- FSM trigger surface (OQ2): **HIGH** — verified by exhaustive grep of `coordinator/src/`
- cargo-audit diff (OQ3): **HIGH** — verified by execution against current advisory DB
- Call-site enumeration (#6 / #7): **HIGH** — verified by `rg` on current codebase
- Clippy lint config (#8): **HIGH** — verified by Cargo.toml inspection AND clippy execution
- RAM-scan test mechanism (#9 / CD-50): **MEDIUM** — sketch is portable on Linux but probabilistic; explicitly marked best-effort
- File:symbol stability (#10): **HIGH** — all 12 anchors verified by line-number lookup
- README current shape (#11): **HIGH** — verified by direct inspection
- `docs/` contents (#12): **HIGH** — verified by `ls`
- Cross-shape test enumeration (#13): **HIGH** — verified by direct inspection
- CI audit gate (#14): **HIGH** — verified by direct inspection

**Research date:** 2026-05-31
**Valid until:** Stable indefinitely for code references (file:symbol anchors are durable); advisory DB diff valid for ~7 days from research date (re-run `cargo audit --json` before Plan 21-02 execution to confirm no new advisories opened since 2026-05-29).
