# Phase 21: Audit Charter & Zeroization Tightening — Pattern Map

**Mapped:** 2026-05-31
**Files analyzed:** 9 (5 Rust source + 1 audit.toml + 1 README + 1 new docs file + 1 indirect — handlers.rs)
**Analogs found:** 8 / 9 (1 file — the best-effort RAM-scan test — has no codebase analog; novel pattern)

## File Classification

| File | Status | Role | Data Flow | Closest Analog | Match Quality |
|------|--------|------|-----------|----------------|---------------|
| `coordinator/src/blind/rsa.rs` | MODIFY | crypto module (newtype + Drop impl + doc-comment rewrite + test) | event-driven (Drop trigger), pure data construction | `coordinator/src/round/state.rs::Drop for RoundStateInner` (lines 120-149) + `state.rs::RoundStateInner` doc-comment (lines 82-91) | exact (manual-Drop on a sensitive newtype) |
| `coordinator/src/round/state.rs` (`RoundStateInner.rsa_signer` field) | MODIFY | state field type change | — | `rsa_pubkey_hash: Option<[u8; 32]>` and `rsa_pubkey_der: Option<Vec<u8>>` on `RoundState` (state.rs:157-160) | exact (Option-wrapped sensitive field, None when Idle) |
| `coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end` | ADD test | structural FSM test | event-driven | `state.rs::tests::transition_to_idle_clears_inner` (lines 261-286) | exact |
| `coordinator/src/blind/rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop` | ADD test | best-effort RAM-scan | request-response (allocate + sweep) | **none — novel pattern** | no analog (see "No Analog Found" below) |
| `coordinator/src/round/signing.rs` (4 fixtures at 450, 496, 521, 560) | MODIFY | test fixture wrap in Some(...) | — | `signing.rs:450` (current shape, struct-literal `RoundStateInner { rsa_signer: RsaBlindSigner::generate().unwrap(), … }`) | exact (mechanical wrap) |
| `coordinator/src/round/state.rs` (2 fixtures at 270, 311) | MODIFY | test fixture wrap in Some(...) | — | same as above | exact |
| `coordinator/src/round/output_reg.rs` (`make_valid_token_sig` helper) | MODIFY | test helper analysis | — | local `signer: &RsaBlindSigner` parameter at output_reg.rs:102 — NOT affected by Option refactor | no change required at helper level (see Pattern Assignments) |
| `coordinator/src/round/input_reg.rs:71`, `coordinator/src/api/handlers.rs:383` | MODIFY | production callers of `inner.rsa_signer.*` | request-response | existing call sites at exact lines (production) | exact (mechanical `.as_ref().expect("...")` wrap) |
| `docs/AUDIT-CHARTER.md` | CREATE | documentation (new charter) | — | `docs/PROTOCOL.md` (top-of-file table + heading hierarchy + cross-reference style) | exact (only existing long-form docs file in `docs/`) |
| `.cargo/audit.toml` | MODIFY | config file comment append + RUSTSEC-2023-0071 rewrite | — | itself (current 3 ignore comment blocks — established prose-comment-per-ignore pattern) | exact (own-file pattern) |
| `README.md` §Security Model | MODIFY | doc callout insertion | — | "**Supply-chain hygiene:**" and "**Multi-script script-type integrity (v1.4):**" paragraphs at README.md:300 and 304 | exact |

## Pattern Assignments

---

### `coordinator/src/blind/rsa.rs` — newtype + Drop impl (21-01)

#### Analog 1 (primary): `coordinator/src/round/state.rs::Drop for RoundStateInner` lines 120-149

Manual-Drop-on-sensitive-struct pattern, in-file in the coordinator crate. The new `Drop for RoundSecretKey` mirrors this structure but with an EMPTY crypto body per 21-RESEARCH OQ1 (rsa 0.9.10 `ZeroizeOnDrop` does the work transitively).

**Doc-comment style** (lines 82-91 of state.rs):
```rust
/// Sensitive round material — zeroed on drop.
/// Phase enum and metadata are stored separately (cannot derive Zeroize on enums).
///
/// D-07: Manual Drop zeroes all sensitive cryptographic material (key bytes, secrets,
/// participant data) when the round completes or is aborted.
///
/// NOTE: HashMap does not implement Zeroize (upstream limitation). We implement Drop
/// manually to zeroize the fields that support it (Vec<u8>, [u8;32]) and then clear
/// HashMaps so their heap allocations are freed. The map keys/values that are Strings
/// or Vecs are individually zeroized before clearing.
pub struct RoundStateInner {
```

**Style features the D-132 rewrite (rsa.rs:18-22) must adopt:**
- Open with a one-line summary.
- Blank-line separator.
- Cite the design-decision ID (`D-07:` here; new comment cites `D-07 + AUDIT-03:`).
- Optional `NOTE:` block explaining the upstream limitation in plain prose.
- End with a charter-anchor reference (NEW for D-132 — see CD-49).

**Drop-impl body shape** (lines 120-149 of state.rs):
```rust
impl Drop for RoundStateInner {
    fn drop(&mut self) {
        // Zeroize the RSA key bytes and round secret first
        self.rsa_signing_key.zeroize();
        self.round_secret.zeroize();
        // Zeroize registered input sensitive data
        for (_k, v) in self.registered_inputs.iter_mut() {
            v.zeroize();
        }
        self.registered_inputs.clear();
        // ... (continues with redeemed_tokens, registered_outputs, partial_sigs, change_addresses)
    }
}
```

**Per 21-RESEARCH OQ1 recommendation, the new `Drop for RoundSecretKey` body is EMPTY of crypto work** — only emits a PII-safe `tracing::debug!` event. The doc-comment ABOVE the impl is where the work happens (citing the transitive `<rsa::RsaPrivateKey as Drop>::drop` chain).

#### Analog 2: Existing D-07 doc-comment to be rewritten (rsa.rs:18-22)

**Current text** (the prose D-132 replaces):
```rust
/// NOTE on memory zeroing (D-07): As of blind-rsa-signatures 0.17.x, SecretKey does not
/// implement Zeroize. The RSA private key bytes held in this struct are not explicitly
/// zeroed on drop — this is a known upstream limitation. RoundStateInner (round/state.rs)
/// stores the serialized key under ZeroizeOnDrop; that serialized copy IS zeroed on round
/// completion. The in-process copy in SecretKey here is best-effort only.
```

**Doc-comment style features to preserve in D-132 rewrite:**
- `NOTE on <topic> (D-XX):` opening (D-132 uses `NOTE on memory zeroing (D-07 + AUDIT-03):`).
- Names the upstream crate + version explicitly.
- Names the load-bearing claim in its own paragraph.
- Per 21-RESEARCH OQ1, the new comment INVERTS the "best-effort" framing — names the transitive `rsa::RsaPrivateKey` Drop chain as cryptographically correct, and positions the newtype's value as **lifetime expression**.

#### Analog 3: Test-fixture struct-literal shape (state.rs:268-277)

The 21-01 mechanical refresh wraps `RsaBlindSigner::generate().unwrap()` in `Some(...)`. The surrounding struct-literal for orientation:
```rust
state.inner = Some(RoundStateInner {
    rsa_signing_key: vec![0xAA; 32],
    rsa_signer: RsaBlindSigner::generate().unwrap(),   // ← becomes Some(RsaBlindSigner::generate().unwrap())
    round_secret: [0xBB; 32],
    registered_inputs: Default::default(),
    redeemed_tokens: HashSet::new(),
    registered_outputs: vec![],
    partial_sigs: Default::default(),
    change_addresses: Default::default(),
});
```

**All 6 mechanical sites use this struct-literal shape** (state.rs:270 + 311; signing.rs:450 + 496 + 521 + 560). No other surrounding-code change required — the wrap is purely the rsa_signer field-value expression.

---

### `coordinator/src/round/state.rs` — `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` field (21-01)

#### Analog: existing `Option<_>` fields on the same struct (state.rs:153-164)

```rust
/// The full round state. Outer struct holds non-sensitive metadata.
/// Inner state is dropped (and zeroed) on transition to Idle.
pub struct RoundState {
    pub phase: Phase,
    pub round_id: Uuid,
    /// SHA-256 of the DER-encoded RSA public key — None when Idle.
    pub rsa_pubkey_hash: Option<[u8; 32]>,
    /// DER-encoded SubjectPublicKeyInfo bytes of the RSA public key — None when Idle.
    /// Published in GET /info so clients can verify and use for blinding (D-02).
    pub rsa_pubkey_der: Option<Vec<u8>>,
    pub participant_count: u32,
    /// Sensitive material — None when Idle (dropped and zeroed after round completion).
    pub inner: Option<RoundStateInner>,
}
```

**Style features the 21-01 refactor adopts:**
- Doc-comment ends with "— None when Idle" or equivalent precise statement of when the Option is empty.
- For the rsa_signer field at state.rs:97-103, the doc-comment also names the construction path (`round::manager::start_round`). New comment: `/// Parsed RSA blind signer — Some(_) during an active round, None when Idle.` + reference to AUDIT-03.

#### Field doc-comment in current shape (state.rs:97-103) — replaced wholesale:
```rust
/// Parsed RSA blind signer — cached once at round creation (D-04, D-05, AVAIL-02).
/// Not zeroized on drop (upstream SecretKey limitation; raw bytes above ARE zeroed).
///
/// Constructed in production by `round::manager::start_round`. Tests may construct
/// directly via the public field; they should prefer `start_round` where possible
/// to keep production and test bootstrap aligned.
pub rsa_signer: RsaBlindSigner,
```

**Plan-21-01 rewrite:** drop the "Not zeroized on drop" sentence (inverted per OQ1); change type to `Option<RsaBlindSigner>`; add reference to AUDIT-03 lifetime bound. The "constructed in production by ..." sentence stays.

---

### `coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end` — ADD test (21-01)

#### Analog: `state.rs::tests::transition_to_idle_clears_inner` lines 258-286

**EXACT pattern** (per CONTEXT.md and 21-RESEARCH OQ2 — D-131 first bullet mirrors this verbatim with one extra pre-condition assertion).

Full excerpt for the planner to copy/adapt:
```rust
/// PRIV-01: Verify inner is None (dropped + zeroed) after Broadcast→Idle transition.
/// Drop impl on RoundStateInner calls .zeroize() on all sensitive fields before clearing.
/// This is the correctness assertion confirming memory zeroing runs when round completes.
#[test]
fn transition_to_idle_clears_inner() {
    use crate::blind::rsa::RsaBlindSigner;
    let mut state = RoundState::new_idle();
    // Simulate having entered a round
    state.phase = Phase::Signing;
    state.rsa_pubkey_der = Some(vec![1, 2, 3]); // simulate active round
    state.inner = Some(RoundStateInner {
        rsa_signing_key: vec![0xAA; 32],
        rsa_signer: RsaBlindSigner::generate().unwrap(),
        round_secret: [0xBB; 32],
        registered_inputs: Default::default(),
        redeemed_tokens: HashSet::new(),
        registered_outputs: vec![],
        partial_sigs: Default::default(),
        change_addresses: Default::default(),
    });
    // Transition to Broadcast then to Idle
    state.transition_to(Phase::Broadcast).unwrap();
    state.transition_to(Phase::Idle).unwrap();
    // PRIV-01: inner MUST be None after Idle transition (Drop was called → zeroize ran)
    assert!(state.inner.is_none(), "PRIV-01: RoundStateInner must be dropped on Idle transition");
    assert_eq!(state.phase, Phase::Idle);
    assert!(state.rsa_pubkey_hash.is_none());
    assert!(state.rsa_pubkey_der.is_none());
}
```

**Style features the new test adopts:**
- Doc-comment opens with the design-ID (the new test uses `AUDIT-03:`).
- Doc-comment names the load-bearing claim.
- Test body: `RoundState::new_idle()` → set phase + simulate active round → `state.inner = Some(RoundStateInner { … })` struct-literal → drive `transition_to` through the relevant FSM edge → assert `state.inner.is_none()`.
- The new test additionally asserts `state.inner.as_ref().unwrap().rsa_signer.is_some()` BEFORE transition (per CONTEXT D-131: "additionally asserts that the rsa_signer Option held a Some"). Note: this assertion runs against the locally-constructed `inner` BEFORE the call to `transition_to(Broadcast)`, since after the Idle transition `inner` is None.

---

### `coordinator/src/round/signing.rs`, `state.rs`, `output_reg.rs` — test-fixture refreshes (21-01)

#### Analog: existing fixture at signing.rs:448-468 (one full example)

```rust
let mut state = RoundState::new_idle();
state.phase = Phase::Signing;
let inner = RoundStateInner {
    rsa_signing_key: vec![],
    rsa_signer: RsaBlindSigner::generate().unwrap(),   // ← becomes Some(RsaBlindSigner::generate().unwrap())
    round_secret: [0u8; 32],
    registered_inputs: {
        let mut m = HashMap::new();
        m.insert("txabc:0".to_string(), RegisteredInput {
            utxo_str: "txabc:0".to_string(),
            change_address: "addr".to_string(),
            blind_sig_hash: [0u8; 32],
            script_pubkey: bitcoin::ScriptBuf::new(),
            value_sats: 150_000,
            script_type: shared::bip322::ScriptType::P2wpkh,
        });
        m
    },
    redeemed_tokens: HashSet::new(),
    registered_outputs: vec![],
    partial_sigs: HashMap::new(),
    change_addresses: HashMap::new(),
};
```

**Mechanical rule:** every `rsa_signer: RsaBlindSigner::generate().unwrap(),` line inside a `RoundStateInner { ... }` struct literal becomes `rsa_signer: Some(RsaBlindSigner::generate().unwrap()),`. No other field changes. Per 21-RESEARCH Pitfall 3, no clippy lint will surface.

#### Note on `output_reg.rs::make_valid_token_sig` (lines 100-114):
```rust
fn make_valid_token_sig(signer: &RsaBlindSigner, amount: u64) -> ([u8; 32], Vec<u8>, Option<MessageRandomizer>) {
```
This helper takes `&RsaBlindSigner` directly (NOT `&Option<RsaBlindSigner>`). Callers construct a local `let signer = RsaBlindSigner::generate().unwrap();` and pass `&signer` — NOT `&inner.rsa_signer`. Per 21-RESEARCH Pitfall 4, this helper is **NOT** affected by the Option refactor.

---

### `coordinator/src/round/input_reg.rs:71` + `coordinator/src/api/handlers.rs:383` — production call-site refreshes (21-01)

#### Analog: existing call sites (own-file pattern; refresh is mechanical)

**`coordinator/src/round/input_reg.rs:71` (production call site #1):**
```rust
// Blind-sign the blinded message using the cached signer (AVAIL-02: no per-request key deserialization)
let blind_msg = BlindMessage(blinded_token_bytes.to_vec());
let blind_sig = inner.rsa_signer.blind_sign(&blind_msg).map_err(|e| ApiError {
    code: ErrorCode::InvalidToken,
    message: format!("Blind signing failed: {e}"),
    round_id: Some(round_id_str.to_string()),
})?;
```

**Refresh:** `inner.rsa_signer.blind_sign(...)` → `inner.rsa_signer.as_ref().expect("rsa_signer must be Some during active round").blind_sign(...)`.

**`coordinator/src/api/handlers.rs:380-383` (production call site #2):**
```rust
let rsa_public_key = guard.inner.as_ref()
    .ok_or_else(|| api_error(StatusCode::CONFLICT, "WRONG_PHASE",
        "Round inner state not initialized", Some(&round_id_str)))?
    .rsa_signer.public_key.clone();
```

**Refresh:** the `.rsa_signer.public_key.clone()` becomes `.rsa_signer.as_ref().expect("rsa_signer must be Some during active round").public_key.clone()`. Note: this site already does the outer `.inner.as_ref().ok_or_else(...)` correctly — the new wrap is only on the inner Option.

**Test-only production-shaped call site (`coordinator/src/round/manager.rs:195`):**
```rust
let pk_hash_from_signer = inner.rsa_signer.public_key_hash();
```
This is inside a `#[cfg(test)]` block (the AVAIL-02 consistency test at state.rs:301-328 calls `inner.rsa_signer.public_key_hash()` directly). Refresh: `inner.rsa_signer.as_ref().unwrap().public_key_hash()`.

**Test-only construction at `manager.rs:63`:**
```rust
rsa_signer: signer,
```
inside `start_round`'s struct-literal — becomes `rsa_signer: Some(signer),`.

**Per 21-RESEARCH OQ2 Pitfall 4: total count = 4 production-shaped call sites (input_reg.rs:71, handlers.rs:383, manager.rs:63 construction, manager.rs:195 test-helper read) + 6 test-fixture struct-literal sites (state.rs:270, state.rs:311, signing.rs:450, signing.rs:496, signing.rs:521, signing.rs:560).** CONTEXT.md "~6-10" estimate was high — the actual is 4 + 6 = 10 sites total but only 2 are runtime-path production calls.

---

### `docs/AUDIT-CHARTER.md` — NEW file (21-02)

#### Analog: `docs/PROTOCOL.md` (top of file)

Only long-form docs file in `docs/`; this is the convention reference.

**Top-of-file frontmatter table** (PROTOCOL.md:1-12):
```markdown
# blindjoin Protocol Specification

| Field | Value |
|---|---|
| **Title** | blindjoin: A Blind-Signed CoinJoin Coordination Protocol with DHT-Based Discovery |
| **Authors** | John Turner (`<johnturner@gmail.com>`) |
| **Status** | Draft |
| **Layer** | Applications |
| **License** | MIT (this specification and the reference implementation) |
| **Created** | 2026-05 |
| **Implementation** | https://github.com/johnzilla/blindjoin |

> **Status — Draft.** This document is the in-progress normative specification of
> the blindjoin coordinator-client wire protocol. [...] Comments and review issues
> welcome via the project issue tracker.

---

## Abstract
[...]
```

**Style features the new charter adopts:**
- H1 file title with the doc's own name.
- Frontmatter `| Field | Value |` table (Title, Authors, Status, License, Created, Implementation).
- One blockquote status callout (`> **Status — ...**`).
- `---` horizontal rule before the first H2.
- H2 sections with optional H3 sub-sections (e.g., `### Round lifecycle`).
- Mid-doc tables for enumerated facts (e.g., `Roles` table at PROTOCOL.md:74-78, `Cryptographic primitives` table at 82-89).
- Cross-references use `[`label`](relative-path-from-this-file)` form (e.g., `[`OwnershipProof`](../shared/src/protocol.rs)` at PROTOCOL.md:127).
- No auto-generated TOC.

**Plan-21-02 mapping:** the 8 mandated charter sections (per D-134) become 8 H2 headings; tables for §1/§3/§6/§8 (per D-134); narrative for §2/§4/§5/§7. Frontmatter table includes `Status: Draft` (or `Status: v1.5 audit-readiness`), `Created: 2026-05`, `License: MIT`.

---

### `.cargo/audit.toml` — refresh (21-02)

#### Analog: own file (current 3-entry pattern, 41 LOC)

Full file (audit.toml:1-40) shows the established prose-comment-per-ignore convention. Excerpt of ONE entry (RUSTSEC-2023-0071, the one D-139 + D-140 rewrites):

```toml
[advisories]
ignore = [
    # rsa crate (transitive via blind-rsa-signatures) — Marvin Attack timing
    # sidechannel. No upstream fix available.
    #
    # Why accepted as residual risk in blindjoin:
    # The Marvin Attack model assumes the attacker can submit many chosen
    # ciphertexts against the same long-lived private key and measure
    # decryption timing precisely. blindjoin's coordinator generates a fresh
    # ephemeral RSA keypair per round, performs at most `max_participants`
    # blind-sign operations against it (default cap: 20 per round), and
    # destroys the key via `zeroize` after the round broadcasts. The attack
    # model's preconditions (long-lived key, unlimited measurements) do not
    # obtain.
    #
    # This residual-risk acceptance is explicitly in scope for the planned
    # external security audit. If the audit finds the ephemeral-key mitigation
    # insufficient (e.g. cross-round timing oracles via shared TLS state),
    # this entry will be removed and a remediation plan published.
    "RUSTSEC-2023-0071",
    [... bincode entry ...]
    [... paste entry ...]
]
```

**Style features the 21-02 refresh adopts:**
- Top-of-file header: `# cargo-audit configuration for blindjoin.` + brief explanation + `# Reviewed: YYYY-MM-DD.` line (D-140 bumps this).
- Per-ignore prose block: `# <crate name + version> — <one-line summary>.` opening; blank-comment-line separator; `# Why accepted as residual risk in blindjoin:` paragraph; optional `# This residual-risk acceptance is explicitly in scope for the planned external security audit. ...` closing.
- The literal advisory ID string follows the comment block.
- D-139 appends a NEW closing line BEFORE the literal ID: `# See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.` — bare path, no markdown link syntax (TOML comments render nowhere).
- D-140 bumps `# Reviewed: 2026-05-26.` to the 21-02 commit date.
- D-139 also REWRITES the RUSTSEC-2023-0071 paragraph: replace "destroys the key via `zeroize` after the round broadcasts" with language naming AUDIT-03 bounded-window mitigation explicitly. Per 21-RESEARCH OQ1, the new prose cites the transitive `rsa::RsaPrivateKey` `Drop` chain bounded by `Option<RsaBlindSigner>` on `RoundStateInner`.

---

### `README.md` §Security Model audit-charter callout — INSERT (21-02)

#### Analog: existing hardening-rollup paragraphs at README.md:296-304

Two existing paragraphs use the exact `**Category (vN.x):** prose` pattern D-143 requires. Excerpts:

**"Supply-chain hygiene" paragraph (README.md:300):**
```markdown
**Supply-chain hygiene:** TLS is pure-Rust [rustls](https://github.com/rustls/rustls) across the entire dependency tree; the openssl crate chain is not pulled in. The `cargo audit` CI step blocks merge on any advisory not declared in [`.cargo/audit.toml`](.cargo/audit.toml), where each accepted residual risk carries a written rationale. The `cargo clippy --all-targets` CI step blocks merge on any lint, including in integration-test code. As of v1.3 Phase 9, CI's `bitcoind` install verifies the Bitcoin Core tarball against achow101's PGP signature (key fingerprint `152812300785C96444D3334D17565732E08E5E41`, pulled from a SHA-pinned `guix.sigs` commit) before extracting it — the install will fail closed on a substituted binary or a stale key.
```

**"Multi-script script-type integrity (v1.4)" paragraph (README.md:304):**
```markdown
**Multi-script script-type integrity (v1.4):** The coordinator's `validate_utxo` derives the `ScriptType` of every input from the on-chain `script_pubkey` (not from the client-declared field on the wire) and cross-checks against the declaration; mismatch returns `Bip322Error::ScriptTypeMismatch` **before** the per-script verifier ever runs. [...] 9 cross-shape rejection tests in `shared/tests/bip322_cross_shape.rs` lock the V1.4-CRIT-01 spoofing-vector closure at the `shared/` crate boundary.
```

**Style features the 21-02 insertion adopts:**
- Opens with `**External audit charter (v1.5):**` (matches the established `**Category (vN.x):**` form).
- One paragraph, no sub-bullets.
- Uses markdown link to the relative docs path: `[docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md)`.
- Lists what the charter enumerates (in-scope modules, threat models, 9 rejection properties, v=2 OwnershipProof boundary, RSA zeroization window, out-of-scope, residual risks, glossary).
- **Placement** (per CD-52 + D-143): after the "Supply-chain hygiene" paragraph (line 300), before the v1.3 "Test infrastructure" paragraph (line 302). Plan-21-02 inserts ONE new paragraph between them.

---

## Shared Patterns

### `// AUDIT-03:` design-decision header comments
**Source:** existing inline header comments throughout coordinator/ (`// D-07:`, `// AVAIL-02:`, `// PRIV-01:`, `// CRIT-01:`, etc. — see Established Patterns in CONTEXT.md line 302).
**Apply to:** every new code site introduced by 21-01 (the `RoundSecretKey` newtype, the `Drop for RoundSecretKey` impl, the field type change at state.rs:103, the two new tests).

Example:
```rust
// AUDIT-03: RoundSecretKey wraps BjSecretKey; bounded lifetime via
// Option<RsaBlindSigner> on RoundStateInner; Drop runs on transition_to(Phase::Idle).
```

### PII-safe `tracing::debug!` / `tracing::info!` structured-field logging
**Source:** `coordinator/src/run.rs:163`, `coordinator/src/round/signing.rs:276`, `coordinator/src/api/handlers.rs:282`.
**Apply to:** the new `Drop for RoundSecretKey` body (1 `tracing::debug!` line).

Excerpts of the established style:

```rust
// coordinator/src/run.rs:163
tracing::debug!(%round_id, "Arming input_reg timeout timer");
```

```rust
// coordinator/src/round/signing.rs:276
info!(txid = %txid, round_id = %round_id_str, "CoinJoin TX broadcast");
```

```rust
// coordinator/src/api/handlers.rs:281-285
info!(
    round_id = %round_id_str,
    participant_count = guard.participant_count,
    "Max participants reached — advancing to output_reg"
);
```

**Style features for the new Drop body:**
- Use `tracing::debug!` (not `info!` — Drop event is ops-observability, not normal flow).
- Use a static-string message; do NOT interpolate `self` or any field of `self` (per 21-RESEARCH Pitfall 2: Default `Debug` derivation on `BjSecretKey` would print key material via the wrapped `RsaPrivateKey`'s auto-derived `Debug`).
- Per 21-RESEARCH OQ1 recommendation, use `target: "blindjoin::audit"` for filterability.
- No `round_id` field — the Drop scope does not have a handle to it, and threading one in would force a bigger refactor than the value of the log line justifies.

Reference body (per OQ1):
```rust
tracing::debug!(
    target: "blindjoin::audit",
    "RoundSecretKey dropped — rsa::RsaPrivateKey ZeroizeOnDrop fires transitively"
);
```

### `#[cfg(test)] mod tests` inline-in-the-same-file convention
**Source:** both `coordinator/src/blind/rsa.rs` (lines 70-163) and `coordinator/src/round/state.rs` (lines 206-329).
**Apply to:** both new tests (one in each file's existing tests module — `round_secret_key_buffer_overwritten_on_drop` in rsa.rs, `round_secret_key_dropped_on_round_end` in state.rs). No new file or external `tests/` integration test.

### `#[ignore = "<reason>"]` annotation style
**Source:** `tests/integration/v13_binary_compat.rs:124` and `tests/integration/multi_script_client.rs:81-116`.
**Apply to:** the best-effort RAM-scan test (CD-50), if it proves non-portable.

Examples of the established convention:
```rust
// tests/integration/v13_binary_compat.rs:124
#[ignore = "v1.3 binary BIP-322 to_sign format incompatible with v1.4 coordinator bip322 crate — see 18-VERIFICATION.md §Success Criterion #5 for D-87 UAT path"]
```

```rust
// tests/integration/multi_script_client.rs:81
#[ignore = "covered by client/src/wallet::tests::generate_p2wpkh_produces_bip84_descriptor"]
```

**Style features the new test adopts** (if marked ignored):
- `#[ignore = "<reason>"]` form (NOT bare `#[ignore]`).
- Reason names WHY the test is ignored AND points to the sibling that carries the load-bearing claim.
- Per 21-RESEARCH OQ1 example: `#[cfg_attr(not(target_os = "linux"), ignore = "non-portable heap layout; structural test in state.rs is the unconditional gate (D-131)")]`.

### `Option<_>` field doc-comment convention
**Source:** state.rs:155-164 (rsa_pubkey_hash, rsa_pubkey_der, inner).
**Apply to:** the rsa_signer field doc-comment after refactor to `Option<RsaBlindSigner>`.

Style: doc-comment names what the field holds, then a sentence ending with "— None when Idle" or "— Some(_) during an active round, None when Idle".

---

## No Analog Found

### `coordinator/src/blind/rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop` (CD-50)

**Role:** best-effort RAM-scan test (allocate a buffer post-drop, sweep for DER fingerprint).
**Data flow:** request-response (synchronous in-process scan).
**Reason no analog exists:** no existing test in the codebase scans heap memory after dropping a sensitive value. The closest existing tests in `coordinator/src/round/state.rs::tests` are structural FSM tests (they assert `state.inner.is_none()` AFTER a transition); they do NOT touch memory contents. `shared/` and `client/` similarly have no precedent.

**Closest reference is the `#[ignore = "..."]` convention** (covered in Shared Patterns above). The planner should treat this test as a NOVEL pattern and follow the 21-RESEARCH `Code Examples` block (`Proposed Best-Effort Scrub Test (CD-50)`) verbatim — that block IS the planner's reference template, since no in-codebase analog exists.

---

## Metadata

**Analog search scope:** `coordinator/src/` (full), `shared/src/` (selective), `client/src/` (verified empty for this pattern set), `tests/` (selective for `#[ignore]` examples), `docs/` (full — only 2 files), `.cargo/` (full — only audit.toml), `README.md` (§Security Model only).

**Files scanned:**
- `coordinator/src/blind/rsa.rs` (163 lines, full)
- `coordinator/src/round/state.rs` (329 lines, full)
- `coordinator/src/round/signing.rs` (lines 270-285 + 440-475, targeted)
- `coordinator/src/round/output_reg.rs` (lines 85-115, targeted)
- `coordinator/src/round/input_reg.rs` (lines 60-90, targeted)
- `coordinator/src/round/manager.rs` (lines 55-70 + 180-200, targeted)
- `coordinator/src/api/handlers.rs` (lines 270-290 + 375-390, targeted)
- `coordinator/src/run.rs` (lines 160-200, targeted)
- `.cargo/audit.toml` (41 lines, full)
- `docs/PROTOCOL.md` (lines 1-160, primary)
- `docs/branch-protection.md` (50 lines, full — secondary)
- `README.md` (lines 270-325, targeted)
- `tests/integration/v13_binary_compat.rs` + `tests/integration/multi_script_client.rs` (ignored-test annotation examples)

**Pattern extraction date:** 2026-05-31
