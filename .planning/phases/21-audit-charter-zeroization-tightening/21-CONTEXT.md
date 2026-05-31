# Phase 21: Audit Charter & Zeroization Tightening - Context

**Gathered:** 2026-05-31
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 21 is the v1.5 audit-readiness deliverable. It ships three artifacts so an external auditor can read the codebase cold and start work without project-team clarification, and so the RSA SecretKey lifetime is a *structurally-bounded* mitigation the charter can describe in lifetime terms rather than "best-effort":

1. **AUDIT-03** — `coordinator/src/blind/rsa.rs` gains a `RoundSecretKey(BjSecretKey)` newtype. The newtype lives INSIDE `RsaBlindSigner` (replacing the bare `secret_key: BjSecretKey` field at `rsa.rs:25`); `RsaBlindSigner` externals (`blind_sign`, `public_key_hash`, `from_der_secret_key`, `secret_key_der`, `public_key_spki_der`, `generate`) are unchanged. `RoundStateInner.rsa_signer` (`state.rs:103`) is restructured to `Option<RsaBlindSigner>`. `RoundSecretKey` impls `Drop` with a best-effort secure-erase body (best-effort because `blind-rsa-signatures 0.17.x` does NOT impl Zeroize on its `SecretKey` — the structural lifetime bound is the load-bearing claim per REQUIREMENTS). The D-07 "best-effort" qualification at `rsa.rs:18-22` is rewritten as a bounded statement citing the new lifetime path.

2. **AUDIT-01** — `docs/AUDIT-CHARTER.md` exists, committed in `main`, linked from `README.md` §Security Model. 8 sections (mandated by REQUIREMENTS AUDIT-01 verbatim): (1) in-scope modules with file:symbol refs, (2) threat models per module, (3) 9 cross-shape rejection properties enumerated, (4) v=2 OwnershipProof PSBT handling boundary, (5) RSA zeroization window in its post-AUDIT-03 bounded form, (6) out-of-scope with rationale (extended beyond REQUIREMENTS Tor+PKARR to all 3rd-party crypto crates), (7) residual risks accepted with rationale (extended beyond cargo-audit advisories to non-advisory protocol + operational residuals), (8) glossary mapping active v1.4/v1.5 identifiers to plain audit language. Hybrid voice: tables for §1/§3/§6/§8, short narrative for §2/§4/§5/§7.

3. **AUDIT-02** — `.cargo/audit.toml` ignore-rationale prose updated: each entry's comment block gets a closing line `See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.` (bare path + anchor, no markdown link syntax since TOML comments are plain text). RUSTSEC-2023-0071 (rsa Marvin Attack) rationale paragraph is rewritten to name the AUDIT-03 bounded-window mitigation specifically (no more "best-effort"). `Reviewed:` header bumps to the actual 21-02 commit date. Any NEW advisories opened against v1.4 transitive deps since 2026-05-26 get explicit ignore-or-fix decisions with rationale — detection happens in 21-RESEARCH.md via fresh `cargo audit --json` diff against the existing 3 ignores.

**Requirements mapped to this phase** (per `.planning/REQUIREMENTS.md` §Traceability): AUDIT-01, AUDIT-02, AUDIT-03.

**Boundary changes (Phase 21 modifies these files):**

*21-01 (AUDIT-03) wave 1:*
- `coordinator/src/blind/rsa.rs` — add `RoundSecretKey(BjSecretKey)` newtype + `Drop` impl with best-effort secure-erase body; replace `RsaBlindSigner.secret_key: BjSecretKey` with `secret_key: RoundSecretKey`; rewrite D-07 comment at lines 18-22 to cite the bounded lifetime path; ADD unit test `round_secret_key_buffer_overwritten_on_drop` (best-effort RAM scan, may be `#[ignore]`-able if it flakes on a future toolchain).
- `coordinator/src/round/state.rs` — change `rsa_signer: RsaBlindSigner` (line 103) to `rsa_signer: Option<RsaBlindSigner>`; update existing `Drop for RoundStateInner` body (lines 120-149) to note the new `Option` shape lets the inner Drop chain run on `inner = None`; ADD test `round_secret_key_dropped_on_round_end` (structural: construct full RoundStateInner with `Some(signer)`, run a full FSM transition through Broadcast→Idle, assert `state.inner.is_none()` and that the rsa_signer Option held a Some). This test mirrors `transition_to_idle_clears_inner` at `state.rs:262`.
- `coordinator/src/round/signing.rs` — 4 test fixtures at lines 450, 496, 521, 560 construct `rsa_signer: RsaBlindSigner::generate().unwrap()`; refresh to `rsa_signer: Some(RsaBlindSigner::generate().unwrap())`.
- `coordinator/src/round/state.rs` — 2 test fixtures at lines 270 + 311 construct `rsa_signer`; refresh similarly.
- `coordinator/src/round/output_reg.rs` — test fixtures using `RsaBlindSigner` (the `make_valid_token_sig` helper at line 102) take a `&RsaBlindSigner` reference; consumers of `RoundStateInner.rsa_signer` need to unwrap the Option. Plan-phase greps for `.rsa_signer.` and `.rsa_signer)` to enumerate the exact call sites that need `.as_ref().expect(...)` or `.as_ref().unwrap()`.
- `coordinator/src/api/handlers.rs`, `coordinator/src/round/manager.rs`, `coordinator/src/round/input_reg.rs` — production callers of `inner.rsa_signer.blind_sign(...)` and `inner.rsa_signer.public_key_hash()` need an `.as_ref().expect("rsa_signer must be present during round")` or similar. Plan-phase identifies the exact set during 21-RESEARCH.

*21-02 (AUDIT-01 + AUDIT-02) wave 2 (single commit):*
- `docs/AUDIT-CHARTER.md` — NEW file. 8 sections per AUDIT-01 spec. ~600-1000 LOC of prose+tables.
- `.cargo/audit.toml` — append `See docs/AUDIT-CHARTER.md#<anchor>` to each of the 3 existing comment blocks; rewrite RUSTSEC-2023-0071 rationale paragraph to name AUDIT-03; bump `Reviewed:` to commit date; potentially ADD new ignore entries with rationale for any cargo-audit advisories opened since 2026-05-26 (researched in 21-RESEARCH).
- `README.md` — §Security Model gains a one-paragraph callout: "External audit charter at [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md) enumerates in-scope modules, threat models, cross-shape rejection properties, and residual risks accepted." Placement near the existing supply-chain hygiene paragraph.

**Not in scope (defer / reject):**

- Per-input variable `fee_share` (REQUIREMENTS `Future requirements` — wire-protocol change, v1.6+).
- Mixed output script types per participant (REQUIREMENTS `Out of v1.5 scope but not anti-features` — separate output-policy milestone).
- TEST-EXT-01/02/03 cross-implementation differential fixtures (REQUIREMENTS `Future requirements` — Phase 21 charter §7 DOCUMENTS the gap as a residual risk; closure is v1.6+).
- P2WSH multisig BIP-322 (REQUIREMENTS `Out of v1.5 scope` — keeps audit scope tight at {P2WPKH, P2TR, P2SH-P2WPKH}).
- B-03 dynamic fee estimation (REQUIREMENTS `Future requirements` — pre-mainnet, orthogonal to v1.5 accuracy fixes).
- External penetration test (REQUIREMENTS `Out of Scope` — v1.5 *prepares for* audit, doesn't *perform* one).
- Replacing `bip322 = "=0.0.10"` with a fork or custom impl (REQUIREMENTS `Out of Scope` — pinned per v1.4 ADR Decision #1).
- Modifying v=2 OwnershipProof wire format (REQUIREMENTS `Out of Scope` — locked at v1.4 ADR Decision #3).
- Custom RSA / blind-signature crypto (PROJECT.md constraint + REQUIREMENTS `Out of Scope` — AUDIT-03 wraps the existing crate's SecretKey, does NOT fork).
- Adding new advisory ignores without a fix plan (REQUIREMENTS `Out of Scope` — AUDIT-02 requires either charter-anchor + remediation, or removal in favor of dep upgrade).
- Per-script weight table changes (Phase 20 work — landed, untouched by Phase 21).
- BIP-322 sign body changes (Phase 19 work — landed, untouched by Phase 21).

**Cross-phase invariants (carry to every Phase 21 plan boundary):**

1. **v1.3 P2WPKH invariant:** `cargo test --test integration full_round` 8/8 green (~42s). Phase 21 makes NO changes to `full_round.rs`. The 21-01 refactor of `rsa_signer` to `Option<RsaBlindSigner>` MUST keep the v1.3 round-execution path intact — every production call site that today does `inner.rsa_signer.blind_sign(...)` becomes `inner.rsa_signer.as_ref().expect("...").blind_sign(...)`, semantically identical at runtime when `Some` is present.

2. **v1.4 multi-script invariant:** `cargo test --test integration mixed_script_e2e` 1/1 green. Phase 21 makes NO changes to `mixed_script_e2e.rs` or any of the mixed-script path. The audit-charter prose CITES this test as the acceptance gate for multi-script verification + fee path.

3. **v1.5 fee-accuracy invariant:** Phase 20's two FEE-03 regression tests (`fee_share_p2wpkh_only_matches_v14_baseline` and `fee_share_mixed_script_differs_from_uniform_baseline`) stay green. Phase 21 makes NO changes to `coordinator/src/bitcoin/tx.rs` or `coordinator/src/bitcoin/fee.rs`. The charter §"v=2 OwnershipProof PSBT handling" describes the *complete* multi-script verification + fee path that Phase 20 landed.

4. **`cargo clippy --workspace --all-targets -- -D warnings` clean** at every plan boundary. The Option<RsaBlindSigner> refactor introduces `.as_ref().expect(...)` at call sites; clippy may flag `unwrap_used` or `expect_used` depending on configured lints — plan-phase confirms current clippy config and the expected lint set.

5. **`cargo audit` returns 0 vulnerabilities** with the refreshed `.cargo/audit.toml`. After 21-02 lands, the cargo-audit CI gate at `.github/workflows/ci.yml` MUST pass with the new ignore set; if 21-RESEARCH surfaces NEW advisories, the plan-phase decision is upgrade-vs-ignore per advisory.

6. **V1.4-CRIT-01 invariant** (dispatcher-only `shared::bip322` public surface): UNTOUCHED. Phase 21 does NOT modify `shared/`. The charter §"in-scope modules" CITES the surface enumeration from the Phase 19-02 close (9 public symbols).

If any invariant goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phases 14/15/16/17/18/19/20 + REQUIREMENTS.md (NOT re-asked)

LOCKED upstream. Plan-phase consumes verbatim — no re-litigation.

- **Newtype name = `RoundSecretKey(BjSecretKey)`** — REQUIREMENTS AUDIT-03 verbatim.
- **`Option<RoundSecretKey>` lifetime bound on round state** — REQUIREMENTS AUDIT-03 verbatim ("the secret key is live for the duration of `Round.state.signer: Option<RoundSecretKey>` and is dropped (and zeroed) on `Round.complete()` / `Round.abort()` / `Round.timeout()`"). Note: D-128 below pins that the existing `transition_to(Phase::Idle)` path subsumes complete/abort/timeout in this codebase — no new explicit `signer = None` calls.
- **Best-effort RAM scan acceptable for the test** — REQUIREMENTS AUDIT-03 verbatim ("best-effort RAM scan test acceptable; the structural lifetime bound is the load-bearing claim").
- **Charter has 8 mandated sections** — REQUIREMENTS AUDIT-01 verbatim. Phase 21 cannot drop or merge sections; section ORDER follows REQUIREMENTS enumeration order for auditor predictability.
- **Charter linked from `README.md` §Security Model** — REQUIREMENTS AUDIT-01 verbatim.
- **audit.toml each ignore references a charter section anchor** — REQUIREMENTS AUDIT-02(a) verbatim.
- **RUSTSEC-2023-0071 rationale names AUDIT-03 bounded mitigation by name** — REQUIREMENTS AUDIT-02(b) verbatim ("not 'best-effort' anymore").
- **No silent additions to audit.toml** — REQUIREMENTS AUDIT-02(d) + `Out of Scope` table verbatim ("AUDIT-02 requires every ignore to either (a) reference a charter section AND a planned remediation, or (b) be removed in favor of a dep upgrade").
- **No custom crypto in AUDIT-03** — PROJECT.md constraint + REQUIREMENTS `Out of Scope`. AUDIT-03 wraps the existing `blind-rsa-signatures` crate's `SecretKey`; does NOT replace, fork, or modify it.
- **v1.3 / v1.4 / v1.5-Phase-20 invariants stay green at every Phase 21 plan boundary** — STATE.md §"Cross-Phase Invariant (v1.5)" + ROADMAP §"Phase 21" SC#5.

### A. RoundSecretKey newtype shape (AUDIT-03)

- **D-128:** **Newtype lives INSIDE RsaBlindSigner.** Replace the existing `secret_key: BjSecretKey` field at `coordinator/src/blind/rsa.rs:25` with `secret_key: RoundSecretKey`. `RoundStateInner.rsa_signer` at `state.rs:103` becomes `Option<RsaBlindSigner>`. **Rationale:** REQUIREMENTS verbatim uses two slightly different framings — `RoundSecretKey(BjSecretKey)` (the wrapping) and `Round.state.signer: Option<RoundSecretKey>` (the lifetime). The cleanest reconciliation is: newtype wraps `BjSecretKey` inside `RsaBlindSigner`; round state holds `Option<RsaBlindSigner>` (which transitively holds `Option<RoundSecretKey>`). Externals of `RsaBlindSigner` (the 6 public methods) stay unchanged — `blind_sign(&self, blinded_msg)` still delegates to `self.secret_key.0.blind_sign(blinded_msg)`. Charter prose: "BjSecretKey is wrapped in RoundSecretKey, owned by RsaBlindSigner, owned by Option<RsaBlindSigner> on RoundStateInner; setting the Option to None on round end triggers the Drop chain that runs RoundSecretKey::drop." Cost: 1 type change in `rsa.rs`, 1 type change in `state.rs`, ~6-10 call-site fix-ups (`.as_ref().expect(...)`) in production code, ~5-6 test-fixture fix-ups.
- **D-128a:** **`RsaBlindSigner.public_key: BjPublicKey` stays a bare field**, NOT wrapped. RSA public keys are not secret; no Drop semantics needed; no need to add `RoundPublicKey` symmetry. The audit charter §"RSA SecretKey zeroization window" prose explicitly notes "only the SecretKey is wrapped; the PublicKey is published in PKARR records and `/round/info` and has no zeroization requirement."

### B. Drop body (AUDIT-03)

- **D-129:** **Best-effort secure-erase + drop.** `RoundSecretKey::drop` attempts to overwrite the secret-key buffer in place. **Body strategy (plan-phase research confirms exact mechanism — see D-129a):** the `blind-rsa-signatures 0.17.x` `SecretKey` is `rsa::RsaPrivateKey` under the hood; rust-rsa's `RsaPrivateKey` exposes no public scrub API. Two approaches:
   1. **DER-roundtrip scrub:** call `self.0.to_der()` to get the byte vec, then `bytes.zeroize()` and let the Vec drop. This zeroes the DER serialization but NOT the parsed `RsaPrivateKey`'s internal `BigUint` allocations.
   2. **Replace-with-dummy:** `let dummy = blind_rsa_signatures::KeyPair::generate(&mut DefaultRng, 2048).unwrap(); std::mem::replace(&mut self.0, dummy.sk);` — overwrites the inner storage with a fresh keygen's allocation. Cost: 1 RSA-2048 keygen per round end (≈100ms on commodity CPU); acceptable for audit-grade scrub but adds wall-clock to round teardown.
   Plan-phase decides between these in 21-RESEARCH.md. **Default: DER-roundtrip + a tracing::debug! noting "best-effort scrub; structural lifetime bound is load-bearing" so reviewers see the limitation explicitly.** D-07 comment rewrite (D-132) explains the upstream limitation and cites this Drop body.

- **D-129a:** **21-RESEARCH.md researches `blind-rsa-signatures 0.17.x` SecretKey internals.** Questions for the researcher: (1) does `SecretKey` deref to `rsa::RsaPrivateKey`? (2) is `Zeroize` impl'd anywhere upstream we missed (check feature flags `zeroize`)? (3) does the crate offer a scrub API (e.g., `secure_erase`, `take_secret`)? (4) does `rsa = "0.9.x"` (the transitive RSA crate) impl Zeroize on `RsaPrivateKey` under any feature flag? If (2) or (4) returns a usable API, D-129's Drop body uses it; otherwise default to DER-roundtrip + tracing note.

### C. Drop trigger surface (AUDIT-03)

- **D-130:** **Keep existing trigger: `transition_to(Phase::Idle)` is the SOLE drop path.** The `RoundState::transition_to` body at `state.rs:194-200` already sets `self.inner = None` whenever `next == Phase::Idle`. The FSM enumerates 4 valid Idle transitions (`Broadcast → Idle`, `Blame → Idle`, `InputReg → Idle` quorum-fail, and the abort/timeout paths that reach one of these). After D-128, dropping `inner` cascades through `Drop for RoundStateInner` → drops `Option<RsaBlindSigner>` → drops `RoundSecretKey` → runs `RoundSecretKey::drop`. **Charter prose:** "the secret key is live for the duration of `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` (which transitively owns RoundSecretKey); RoundStateInner is dropped when `RoundState.inner` is set to None at any transition to Phase::Idle (the 4 valid edges in the FSM)."
- **D-130a:** **Plan-phase audit task:** enumerate the 4 valid Idle transitions and verify each end-state code path reaches `transition_to(Phase::Idle)`. Specifically: round abort on InputReg quorum-fail (manager.rs path), round complete on Broadcast success (signing.rs path), round blame on signing-timeout (signing.rs blame path), round timeout (timeout-handler in manager.rs or input_reg.rs). If any path bypasses `transition_to(Phase::Idle)` (e.g., a panic-unwind that drops `RoundState` directly), the Drop still runs because `RoundStateInner` impls Drop — but the charter prose needs the FSM-transition narrative, so plan-phase confirms there are no surprises.
- **D-130b:** **No new explicit `signer = None` helper added.** Considered (option 2 of the discussion) and rejected: would add a redundant call before transition_to(Idle) that obscures the actual drop trigger. Charter prose cites the single existing trigger; auditor reads ONE state-transition function, ONE Drop chain.

### D. Test pattern (AUDIT-03)

- **D-131:** **TWO tests, split by ownership concern:**
   - **Structural test** (`coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end`): construct a `RoundState` with `Some(RoundStateInner)` containing `Some(RsaBlindSigner)`, transition `Idle → InputReg → OutputReg → Signing → Broadcast → Idle`, assert `state.inner.is_none()`. Mirrors `transition_to_idle_clears_inner` at `state.rs:262`. **This is the load-bearing assertion per REQUIREMENTS** ("the structural lifetime bound is the load-bearing claim"). Cannot flake — it asserts on the FSM, not on memory.
   - **Best-effort scrub test** (`coordinator/src/blind/rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop`): construct a `RoundSecretKey` from a known DER blob, capture the post-construction value via `secret_key_der()`, drop the RoundSecretKey, then attempt to find the original byte pattern in a heap region. **Acceptable to mark `#[ignore]` if it flakes on a future toolchain** — the structural test (D-131 first bullet) is the unconditional CI gate. Plan-phase confirms a non-flaky implementation; if no robust approach exists, mark `#[ignore = "best-effort RAM scan flakes under LTO; structural bound is enforced by sibling test"]` and document in 21-VERIFICATION.md.

   Charter §5 prose CITES both tests by name and explains the split: "the structural lifetime bound is verified by `round_secret_key_dropped_on_round_end`; the best-effort buffer scrub is verified by `round_secret_key_buffer_overwritten_on_drop` (may be ignored if non-portable; structural test is the unconditional gate)."

### E. D-07 comment rewrite (AUDIT-03)

- **D-132:** **Rewrite `rsa.rs:18-22` from "best-effort only" to the bounded statement.** Concrete new prose (plan-phase may tighten):
   ```
   /// NOTE on memory zeroing (D-07 + AUDIT-03):
   ///
   /// The RSA secret key is wrapped in RoundSecretKey which impls Drop with a
   /// best-effort secure-erase body. The key's lifetime is bounded by
   /// RoundStateInner.rsa_signer: Option<RsaBlindSigner> (which transitively owns
   /// RoundSecretKey). On any transition to Phase::Idle, RoundStateInner is dropped
   /// (state.rs:194-200), which drops Option<RsaBlindSigner>, which drops
   /// RoundSecretKey, which runs the secure-erase Drop body.
   ///
   /// The structural lifetime bound is the load-bearing claim; the in-place buffer
   /// scrub is best-effort because blind-rsa-signatures 0.17.x does not expose its
   /// SecretKey internals for guaranteed zeroization.
   ///
   /// See docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window for the full
   /// threat-model treatment.
   ```
   **Rationale:** the audit charter §5 citation in the comment closes the loop both directions — code points to charter, charter points to code by file:symbol.

### F. Plan split (AUDIT-01 + AUDIT-02 + AUDIT-03)

- **D-133:** **TWO plans:**
   - **21-01-PLAN.md = AUDIT-03** (newtype + Drop + 2 tests + D-07 comment rewrite + Option<RsaBlindSigner> refactor + ~6-10 call-site fix-ups). At this plan boundary all cross-phase invariants must be green: `cargo test --test integration full_round` 8/8; `cargo test --test integration mixed_script_e2e` 1/1; Phase 20 FEE-03 tests; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo audit` 0-vulns (still uses pre-Phase-21 audit.toml — refreshed in 21-02).
   - **21-02-PLAN.md = AUDIT-01 + AUDIT-02** (charter prose + audit.toml refresh + README link). Single commit (D-133a below). At this plan boundary: cargo audit 0-vulns with the refreshed audit.toml; all charter file:symbol refs resolve to real symbols in the codebase; README link resolves to `docs/AUDIT-CHARTER.md`; `cargo test` and `cargo clippy` unchanged from 21-01 boundary (no code changes in 21-02).
   - **Wave structure:** sequential. 21-01 wave 1 → 21-02 wave 2. Charter prose cites `RoundSecretKey` and the bounded-window mitigation that 21-01 ships; writing 21-02 against speculative line numbers/symbols is unnecessary risk.
- **D-133a:** **21-02 ships in a single commit** (charter + audit.toml + README in one atomic landing). **Rationale:** the charter section anchors are referenced by audit.toml rationale strings; landing them in separate commits creates a window where audit.toml points at anchors that don't exist yet. Atomic landing prevents anchor drift between the two artifacts.

### G. Charter authoring (AUDIT-01)

- **D-134:** **Hybrid voice — tables for facts, narrative for threats.** Per-section authoring style:
   - §1 (in-scope modules) — TABLE (file:symbol | description | line at v1.5 tag for orientation).
   - §2 (threat models per module) — SHORT NARRATIVE (3-6 paragraphs per module). Cites: V1.4-CRIT-01 spoofing, V1.4-CRIT-02 silent sighash regression, V1.4-MIN-02 uniform-script fingerprint, RSA Marvin Attack residual exposure (per REQUIREMENTS AUDIT-01).
   - §3 (9 cross-shape rejection properties) — TABLE (property name | source test file:fn | what it rejects | mitigation citation). Source: `shared/tests/bip322_cross_shape.rs` enumerated explicitly.
   - §4 (v=2 OwnershipProof PSBT handling) — SHORT NARRATIVE (4-8 paragraphs). Covers full-BIP-174 shape, RESEARCH Pitfall 1, `decode_psbt_input_witness` boundary.
   - §5 (RSA SecretKey zeroization window) — SHORT NARRATIVE (3-5 paragraphs). Cites RoundSecretKey + bounded-window + best-effort scrub limitation + structural-test load-bearing claim.
   - §6 (out-of-scope) — TABLE (component | rationale). Extended to all 3rd-party crypto crates (per D-135).
   - §7 (residual risks accepted with rationale) — SHORT NARRATIVE per risk class (per D-136).
   - §8 (glossary) — TABLE (project term | plain audit language) (per D-137).
- **D-135:** **§6 out-of-scope extends beyond REQUIREMENTS Tor+PKARR to all 3rd-party crypto crates.** Enumeration:
   - `arti-client` (Tor circuit isolation + hidden-service hosting) — out-of-scope; relies on upstream Tor Project Arti 0.41 audit posture.
   - `pkarr` (Mainline DHT discovery layer) — out-of-scope; relies on Pubky project audit posture.
   - `blind-rsa-signatures` (jedisct1) — INTERNALS out-of-scope (RFC 9474 RSA blind sign primitive); OUR USAGE shape including AUDIT-03 RoundSecretKey IS in-scope.
   - `bip322 = "=0.0.10"` (rust-bitcoin org) — CRATE INTERNALS out-of-scope (BIP-322 verify path); OUR 26-LOC adapter at `shared/src/bip322/mod.rs::verify_via_bip322_crate` IS in-scope.
   - `rust-bitcoin` (Bitcoin primitives + PSBT) and `secp256k1` (curve primitives) — out-of-scope; consensus-critical primitives audited by upstream rust-bitcoin team.
   - `bdk_wallet` (client-side descriptor wallet) — CLIENT-SIDE out-of-scope; coordinator never runs bdk_wallet. CLIENT's USAGE shape (`client/src/wallet.rs::sign_bip322`) IS in-scope insofar as it constructs v=2 OwnershipProof per ADR Decision #3 / #4.
   Per-line rationale ("relies on X for Y, where X = upstream audit posture / well-known consensus primitive / standard wallet library").
- **D-136:** **§7 residual risks split into 3 sub-buckets:**
   - (a) `cargo-audit`-flagged advisories — 3 entries mirroring `.cargo/audit.toml` rationale paragraphs (RUSTSEC-2023-0071 + 2025-0141 + 2024-0436), plus any new advisories surfaced by 21-RESEARCH per D-141.
   - (b) Protocol-level residuals — heterogeneous-input chain-analysis tradeoff (per v1.4 ADR Decision #2; documented in README §Privacy Considerations); V1.4-MIN-02 uniform-script fingerprint partially mitigated by liquidity-bot rotation (per Phase 18 INTEG-02) but operator-set bot ratios cannot eliminate; TEST-EXT-01/02/03 differential-fixture gap per REQUIREMENTS `Future Requirements` (closure deferred to v1.6+).
   - (c) Operational residuals — single-coordinator-per-round trust model (DHT discovery makes coordinators replaceable but not byzantine-fault-tolerant); sybil dilution depends on operator-set min-participant cap (no cryptographic sybil resistance beyond BIP-322 proof of UTXO ownership); PKARR replay window (records are versioned but a stale record may resolve momentarily).
   Each sub-bucket gets 1-2 narrative paragraphs; auditor sees the unified residual-risk register in one place rather than chasing README / TODO / REQUIREMENTS.
- **D-137:** **§8 glossary scope = active v1.4/v1.5 identifiers only.** ~25-30 entries. Covers: V1.4-CRIT-01, V1.4-CRIT-02, V1.4-MIN-02, V1.4-MOD-03, AVAIL-01, AVAIL-02, CR-01, CR-02, WR-01, WR-04, D-07, D-27, D-31, D-34, D-37, D-111, D-122, D-124, D-128 (new this phase), D-130 (new this phase), ADR Decision #1-4 (v1.4), AUDIT-01/02/03 (v1.5). Closing line: "Retired pre-v1.4 identifiers live in `.planning/milestones/v1.0-1.3-*` archives." Pointer-based hygiene; auditor can chase if needed but glossary stays scannable.
- **D-138:** **Anchor style = file:symbol refs.** `coordinator/src/blind/rsa.rs::RoundSecretKey` / `coordinator/src/blind/rsa.rs::RoundSecretKey::drop` / `shared/src/bip322/mod.rs::sign_simple` etc. **Rationale:** symbol-based refs are stable across reformats; `:NN` line refs bit-rot with every patch. Plan-phase MAY include parenthetical `(approx. line 30 at v1.5 ship)` for orientation but the symbol is the durable anchor. Charter §1 table has 3 columns: `file:symbol | description | (orientation line at v1.5 tag)`.

### H. audit.toml refresh (AUDIT-02)

- **D-139:** **Bare relative path + anchor style.** Each existing comment block gets a closing line: `See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.` No markdown link syntax (`[]()`)— TOML comments are plain text, render nowhere. Anchor slugs match the markdown auto-generated form (lowercase, hyphenated; e.g., `#rsa-secret-key-zeroization-window`). **Rationale:** easy to grep, easy to update, no rendering ambiguity.
- **D-140:** **Bump `Reviewed:` header to the actual 21-02 commit date.** NOT to today (2026-05-31 — pre-write), NOT to a TBD-v1.5-milestone-close placeholder. Charter ships in 21-02 commit; audit.toml refresh ships in the same commit; date is the commit's date. **Rationale:** honest — the review happened when Phase 21 landed, not before, not later. v1.5 'ship' as a milestone is bookkeeping at /gsd-complete-milestone; the substantive ship is Phase 21.
- **D-141:** **21-RESEARCH.md detects + classifies NEW advisories.** Tasks for the researcher:
   1. Run `cargo audit --json` against current `Cargo.lock`.
   2. Diff result against the existing 3 ignores (RUSTSEC-2023-0071, RUSTSEC-2025-0141, RUSTSEC-2024-0436).
   3. For each NEW advisory: classify as (a) upstream fix available → propose dep upgrade in 21-02, (b) no fix available → propose new ignore entry with rationale referencing charter section (and a planned remediation per REQUIREMENTS `Out of Scope` table).
   4. Report findings to planner; planner decides upgrade-vs-ignore per advisory and adds tasks to 21-02-PLAN.md.
   **Rationale:** detection happens BEFORE 21-02 planning; planner has the full advisory picture; no surprise advisory failures during 21-02 execution.
- **D-142:** **Flat TOML layout preserved.** Just `[advisories]\nignore = ["RUSTSEC-...", "RUSTSEC-..."]` with prose comments. NO new TOML table structures (`[advisories.ignore."RUSTSEC-..."]` with rationale + charter_anchor + review_date sub-keys) — the documented cargo-audit schema is the flat list; changing layout requires upstream verification we don't need to take on. Per-ignore prose comment is the audit trail.

### I. README integration (AUDIT-01)

- **D-143:** **One-paragraph callout in README.md §Security Model.** Placement: directly after the existing "Supply-chain hygiene" paragraph (around line 300), before the v1.3 test-infrastructure paragraph. Suggested prose: "**External audit charter (v1.5):** [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md) enumerates in-scope modules with file:symbol refs, threat models per module, the 9 cross-shape rejection properties, the v=2 OwnershipProof PSBT handling boundary, the RSA SecretKey zeroization window (RoundSecretKey + bounded lifetime per AUDIT-03), out-of-scope dependencies, residual risks accepted with rationale, and a glossary mapping project terms to audit language." **Rationale:** signal that v1.5 reached audit-readiness; one paragraph is enough — the charter itself is the depth.

### Claude's Discretion

- **CD-46:** **Exact prose wording of the D-07 comment rewrite (D-132).** Plan-phase tightens / rephrases per the established `coordinator/src/blind/rsa.rs` doc-comment style. Load-bearing contract: the new comment names `RoundSecretKey`, names the bounded lifetime via `Option<RsaBlindSigner>` on RoundStateInner, names the existing trigger (`transition_to(Phase::Idle)`), explicitly cites the upstream limitation as the reason for "best-effort" in-place scrub, and points to the charter section anchor.
- **CD-47:** **DER-roundtrip vs replace-with-dummy scrub choice (D-129).** Plan-phase decides based on 21-RESEARCH findings on blind-rsa-signatures internals. Default: DER-roundtrip + tracing::debug! note. If 21-RESEARCH finds a usable upstream scrub API (D-129a), use that; document the choice in 21-01-PLAN.md so future audits see why this particular Drop body shape was selected.
- **CD-48:** **Exact ignore-or-fix decisions for any new advisories surfaced by 21-RESEARCH (D-141).** Plan-phase reads researcher's report and decides per advisory. Default heuristic: upstream fix available (even minor bump) → bump; no fix and dep is transitive crypto-adjacent → add ignore with charter section; no fix and dep is unmaintained build-only → add ignore citing "build-time only, no runtime path".
- **CD-49:** **Charter section anchor naming.** Plan-phase MAY refine the anchor slugs from D-139's defaults. Load-bearing contract: each `.cargo/audit.toml` ignore comment ends with a `See docs/AUDIT-CHARTER.md#<anchor>` line; the anchor exists in the charter (markdown render of the section heading). Whether `#rsa-secret-key-zeroization-window` or `#rsa-zeroization-window` is fine — atomic landing in one commit prevents drift.
- **CD-50:** **Best-effort RAM-scan test implementation (D-131 second bullet).** Plan-phase decides exact mechanism. Default: capture a copy of the DER bytes pre-drop, drop the RoundSecretKey, then scan the heap allocator region adjacent to where the RoundSecretKey lived for the original byte pattern (use `std::alloc::System` heap stats or `mimalloc` hooks if pre-existing). If no portable approach is found, mark `#[ignore = "best-effort RAM scan flakes under LTO; structural bound is enforced by sibling test"]` and add a `Phase 21 D-131` inline comment in the test body explaining why the assertion is intentionally weakened.
- **CD-51:** **Charter §4 (v=2 OwnershipProof PSBT handling) depth.** Plan-phase decides paragraph count (3-8). Load-bearing contract: covers `OwnershipProof.psbt_input_b64` full-BIP-174 shape, cites RESEARCH Pitfall 1 ("encoded as a 1-input PSBT, not bare psbt::Input"), cites `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` as the verification boundary, cites `client/src/round/input.rs::build_v2_psbt_input_b64` as the construction boundary.
- **CD-52:** **README callout placement (D-143).** Plan-phase confirms exact line after re-reading README.md current state. Default: after "Supply-chain hygiene" paragraph (currently line 300-ish), before the v1.3 test-infrastructure paragraph.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing.**

### Project-level anchors

- `.planning/PROJECT.md` §"Constraints" — no custom crypto (AUDIT-03 wraps the existing `blind-rsa-signatures` SecretKey; does NOT fork); no PII logging (RoundSecretKey carries no PII; Drop body's `tracing::debug!` notes the scrub event without dumping bytes); MIT licensed (charter prose may reference dependency licenses but doesn't itself become license-encumbered).
- `.planning/PROJECT.md` §"Current Milestone: v1.5 Audit-Readiness & Multi-Script Finish" — Phase 21 is the FINAL v1.5 phase; closes audit-readiness; charter cites Phases 19 + 20 as production state.
- `.planning/PROJECT.md` §"Key Decisions" — table of locked decisions for the §8 glossary cross-reference (ADR Decision #1-4, RSA blind sigs, Approach B, arti-client, BIP-322, corepc-types, bdk_wallet, PKARR, etc.).
- `.planning/REQUIREMENTS.md` §AUDIT-01 (line 30) — Phase 21 Plan 21-02 closes verbatim (the 8 mandated sections, the README link, the in-scope file/symbol list).
- `.planning/REQUIREMENTS.md` §AUDIT-02 (line 32) — Phase 21 Plan 21-02 closes verbatim (charter-anchor refs, RUSTSEC-2023-0071 rewrite, Reviewed date bump, no silent additions).
- `.planning/REQUIREMENTS.md` §AUDIT-03 (line 34) — Phase 21 Plan 21-01 closes verbatim (RoundSecretKey newtype, Option<RoundSecretKey> on round state, explicit Drop, best-effort RAM scan test acceptable, structural bound is load-bearing).
- `.planning/REQUIREMENTS.md` §"Out of Scope" table — pins anti-features (custom RSA crypto, replacing bip322 pin, modifying v=2 OwnershipProof wire format, silent audit.toml additions, external pen-test in-scope). Plan-phase consults to confirm Phase 21 stays inside the scope.
- `.planning/REQUIREMENTS.md` §"Future Requirements" — TEST-EXT-01/02/03, CARRY-*, B-03 — these names appear in charter §7 residual-risks as v1.6+ deferred items.
- `.planning/REQUIREMENTS.md` §Traceability — AUDIT-01/02/03 → Phase 21.
- `.planning/ROADMAP.md` §"Phase 21" (line 112-123) — 5 success criteria. Plan 21-01 closes SC#3, SC#4, contributes to SC#5; Plan 21-02 closes SC#1, SC#2, contributes to SC#5.
- `.planning/STATE.md` §"Carry-Forward Items" — CARRY-TOR-UAT, CARRY-REPAIR-01-PR, B-03, TEST-EXT-*, P2WSH multisig, Mixed output script types. These names appear in charter §7 (b) residual risks.
- `.planning/STATE.md` §"v1.5 design notes" line 3 — "Phase 21's AUDIT-CHARTER.md should be structured as: in-scope modules (with line/file references), out-of-scope explicitly listed, threat models per module, residual risks accepted, and a glossary mapping audit terminology to project terms (CRIT-01, V1.4-MIN-02, etc.)." Binds D-134's section structure (mostly aligned with REQUIREMENTS AUDIT-01).

### Phase 14/15/16/17/18/19/20 outputs (LOCKED inputs)

- `.planning/decisions/v1.4-adr.md` (full file) — 4 ADR Decisions; charter §8 glossary cites all 4 (RSA blind sigs over WabiSabi, mixed-rounds, B2 base64 PSBT wire, bdk path for P2TR sign). Charter §2 threat model for V1.4-CRIT-01 references the dispatcher-only public surface decision (Decision #1).
- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-27 (dispatcher-only public surface) — charter §1 in-scope modules cites `shared::bip322::sign_simple` / `shared::bip322::verify_simple` as the load-bearing surface; charter §2 cites D-27 as the static-typing mitigation for V1.4-CRIT-01.
- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-31 (10-variant Bip322Error taxonomy + PII safety) — charter §1 cites the PII-safety test at `shared/src/bip322/mod.rs:512-565` as the PII-bound proof.
- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-34 — 9 cross-shape rejection properties enumerated. Charter §3 cites by name + test file (`shared/tests/bip322_cross_shape.rs`).
- `.planning/milestones/v1.4-phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §D-37 — output_script_type validation at boot. Charter §1 cites `coordinator/src/config.rs::BipConfig::validate`.
- `.planning/phases/19-multi-script-signing-finish/19-CONTEXT.md` §"Carried forward" — V1.4-CRIT-01 dispatcher-only invariant. Charter §2 V1.4-CRIT-01 prose cites the Phase 19 close as "production sign bodies preserve the dispatcher-only invariant; the test-only escape hatch was deleted in 19-02."
- `.planning/phases/19-multi-script-signing-finish/19-CONTEXT.md` §D-111 (spk↔key cross-check at top of each sign body) — charter §2 V1.4-CRIT-01 cites as defense-in-depth structural mitigation.
- `.planning/phases/20-mixed-round-fee-accuracy/20-CONTEXT.md` §D-124a (per-script vbyte table) — charter §4 v=2 OwnershipProof prose cites the per-script weight table as the closure of the "complete multi-script verification + fee path" claim.

### Specs / external references

- **RFC 9474** (RSA Blind Signatures) — charter §5 RSA SecretKey zeroization window cites RFC 9474 as the protocol primitive; AUDIT-03 RoundSecretKey is OUR usage shape, not a fork of the primitive.
- **RUSTSEC-2023-0071** (rsa Marvin Attack) — `.cargo/audit.toml` ignore; charter §7 (a) residual risks paragraph; rationale references AUDIT-03 bounded-window mitigation by name. https://rustsec.org/advisories/RUSTSEC-2023-0071.html (informational; researcher confirms current upstream-fix status).
- **RUSTSEC-2025-0141** (bincode 2.0.1 unmaintained) — `.cargo/audit.toml` ignore; charter §7 (a) residual risks. Transitive dep, no runtime path.
- **RUSTSEC-2024-0436** (paste 1.0.15 unmaintained) — `.cargo/audit.toml` ignore; charter §7 (a) residual risks. Compile-time macro, no runtime path.
- **BIP-322** (Generic Signed Message Format) — charter §3 (cross-shape rejection) + §4 (v=2 OwnershipProof) cite BIP-322 §4-5 as the protocol primitive.
- **BIP-174** (PSBT) — charter §4 v=2 OwnershipProof PSBT handling cites BIP-174 §global-types as the wire-shape spec; RESEARCH Pitfall 1 is the "full PSBT, not bare psbt::Input" insight.

### Code anchors (Phase 21 reads OR modifies)

*Phase 21 modifies (21-01):*
- `coordinator/src/blind/rsa.rs::RsaBlindSigner` (currently lines 23-26) — Plan 21-01 changes the `secret_key` field type from `BjSecretKey` to `RoundSecretKey`.
- `coordinator/src/blind/rsa.rs` — Plan 21-01 ADDS `pub struct RoundSecretKey(BjSecretKey);` newtype + `Drop` impl + the 2-test split's scrub test.
- `coordinator/src/blind/rsa.rs` D-07 comment (currently lines 18-22) — Plan 21-01 REWRITES per D-132.
- `coordinator/src/round/state.rs::RoundStateInner.rsa_signer` (line 103) — Plan 21-01 changes type from `RsaBlindSigner` to `Option<RsaBlindSigner>`.
- `coordinator/src/round/state.rs::Drop for RoundStateInner` (lines 120-149) — Plan 21-01 refreshes inline comments to cite the new Option<RsaBlindSigner> shape; the Drop body itself may stay structurally identical (the Option's inner is dropped as part of the struct's natural drop).
- `coordinator/src/round/state.rs::tests` — Plan 21-01 ADDS `round_secret_key_dropped_on_round_end` (structural FSM test mirroring `transition_to_idle_clears_inner` at line 262).
- Test-fixture call sites at `state.rs:270`, `state.rs:311`, `signing.rs:450`, `signing.rs:496`, `signing.rs:521`, `signing.rs:560` — Plan 21-01 refreshes `rsa_signer: RsaBlindSigner::generate().unwrap()` to `rsa_signer: Some(RsaBlindSigner::generate().unwrap())`.
- Production call sites of `inner.rsa_signer.*` — Plan 21-01 enumerates via grep `\\.rsa_signer\\.` and `\\.rsa_signer)` in coordinator/src/; refreshes each to `.as_ref().expect("rsa_signer must be Some during active round")` or equivalent. Initial scope: `coordinator/src/round/output_reg.rs`, `coordinator/src/api/handlers.rs`, `coordinator/src/round/input_reg.rs`, `coordinator/src/round/manager.rs`. 21-RESEARCH confirms exact count.

*Phase 21 modifies (21-02):*
- `docs/AUDIT-CHARTER.md` — NEW file. Plan 21-02 creates with 8 sections per D-134.
- `.cargo/audit.toml` (full file, currently 46 lines) — Plan 21-02 appends `See docs/AUDIT-CHARTER.md#<anchor>` to each of the 3 existing comment blocks; rewrites RUSTSEC-2023-0071 rationale paragraph to name AUDIT-03 (D-139, D-140); potentially adds new entries per 21-RESEARCH D-141 findings; bumps `Reviewed:` header (D-140).
- `README.md` §Security Model (around line 300) — Plan 21-02 inserts one-paragraph audit-charter callout per D-143.

*Phase 21 reads (NOT modifies):*
- `coordinator/src/blind/rsa.rs` (full file, 163 lines) — context for 21-RESEARCH on blind-rsa-signatures SecretKey internals (D-129a).
- `coordinator/src/round/state.rs::transition_to` (lines 184-203) — Plan 21-01 verifies the existing transition logic suffices; Plan 21-02 charter §5 cites this method by name.
- `coordinator/src/round/state.rs::tests::transition_to_idle_clears_inner` (line 262) — pattern reference for the new `round_secret_key_dropped_on_round_end` test.
- `coordinator/src/round/manager.rs`, `coordinator/src/round/input_reg.rs`, `coordinator/src/round/signing.rs` — Plan 21-01 reads to enumerate end-state code paths (Broadcast→Idle, Blame→Idle, InputReg→Idle, timeout→Idle) per D-130a.
- `coordinator/src/bitcoin/utxo.rs::validate_utxo` + `coordinator/src/bitcoin/utxo.rs::dispatch_ownership_proof` (currently ~line 67-118 + 154+) — charter §1 in-scope cites `validate_utxo` + the v=1/v=2 dispatcher arms (CRIT-01 cross-check) at file:symbol level.
- `shared/src/bip322/mod.rs` (659 lines) — charter §1 in-scope cites the 9 public symbols (dispatcher-only public surface).
- `shared/tests/bip322_cross_shape.rs` (full file) — charter §3 enumerates the 9 cross-shape rejection properties from this file; cites by test fn name.
- `client/src/round/input.rs::build_v2_psbt_input_b64` (line 35+) — charter §4 v=2 OwnershipProof PSBT construction boundary.
- `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` (cited by `client/src/round/input.rs:25` comment) — charter §4 v=2 OwnershipProof verification boundary.
- `.cargo/audit.toml` — current 3 ignores (RUSTSEC-2023-0071, RUSTSEC-2025-0141, RUSTSEC-2024-0436).
- `README.md` §Security Model (lines 281-305) — current shape; Plan 21-02 D-143 placement reference.
- `.planning/decisions/v1.4-adr.md` (full file) — ADR Decisions #1-4 for charter §8 glossary.

### Cross-phase invariant references

- `tests/integration/full_round.rs` (1597 LOC, v1.3 invariant gate) — Phase 21 makes NO changes. Run `cargo test --test integration full_round` after each Phase 21 plan; expect 8/8 green, ~42s. The 21-01 refactor to `Option<RsaBlindSigner>` MUST not break this gate.
- `tests/integration/mixed_script_e2e.rs` (v1.4 invariant gate) — Phase 21 makes NO changes. Run `cargo test --test integration mixed_script_e2e` after each Phase 21 plan; expect 1/1 green.
- `coordinator/src/bitcoin/tx.rs::tests::fee_share_p2wpkh_only_matches_v14_baseline` + `fee_share_mixed_script_differs_from_uniform_baseline` (Phase 20 FEE-03 regression tests) — Phase 21 makes NO changes to `tx.rs` / `fee.rs`. Run after each plan; expect green.
- `shared/tests/bip322_cross_shape.rs` (9 cross-shape rejection tests) — Phase 21 makes NO changes; charter §3 CITES this file by line/fn name.
- `.github/workflows/ci.yml` `cargo audit` step — Phase 21 21-02 refreshes `.cargo/audit.toml`; the CI gate MUST pass with 0 vulnerabilities after the refresh.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`Drop for RoundStateInner` at `coordinator/src/round/state.rs:120-149`** — already implements the manual-Drop pattern for HashMap-bound zeroize (HashMap doesn't impl Zeroize, so each map's values are iter_mut'd + zeroize'd before clearing). Plan 21-01's Option<RsaBlindSigner> refactor preserves this body's structure; the new RoundSecretKey::drop fires as part of the Option's natural drop chain, no edit needed to this body's logic (only inline comment refresh).

- **`transition_to(Phase::Idle)` body at `state.rs:194-200`** — already sets `self.inner = None` (triggering full RoundStateInner drop) and resets `rsa_pubkey_hash`, `rsa_pubkey_der`, `participant_count`, `round_id`. The 21-01 refactor adds nothing here — the existing trigger subsumes the AUDIT-03 lifetime-bound prose.

- **`transition_to_idle_clears_inner` test at `state.rs:262`** — the EXACT pattern the new `round_secret_key_dropped_on_round_end` follows. Construct a `RoundState` with `Some(inner)`, transition through a valid Idle edge, assert `state.inner.is_none()`. The new test additionally asserts that the rsa_signer Option was `Some` before the transition (so the drop chain DID fire on a non-None RoundSecretKey).

- **`Drop for RoundStateInner` inline comment block (lines 82-91)** — already documents the "HashMap doesn't impl Zeroize; manual Drop + clear" pattern. Plan 21-01 follows the SAME convention for the new RoundSecretKey Drop body: structured doc-comment naming the load-bearing claim + the upstream limitation.

- **3-entry `.cargo/audit.toml` (current 46 LOC)** — established prose-comment-per-ignore pattern. Plan 21-02 follows verbatim (just appends the closing charter-anchor line + rewrites RUSTSEC-2023-0071 paragraph).

- **README.md §Security Model paragraph structure (lines 281-305)** — established "**Category (vN.x):** prose" structure for hardening rollups (Availability v1.1, Public-endpoint v1.2 Phase 8, Supply-chain hygiene, Test infrastructure v1.3 Phase 9, Multi-script script-type integrity v1.4). Plan 21-02 D-143 follows this convention for the v1.5 audit-charter callout.

- **`tracing::info!` / `tracing::debug!` PII-safe structured logging** — established pattern across coordinator/. Plan 21-01's optional Drop-body debug log uses `tracing::debug!(round_id = ?_round_id, "RoundSecretKey scrubbed at round end")` — no key material in the log; round_id is already a public correlation key.

### Established Patterns

- **`Option<_>` for round-lifetime sensitive material on RoundStateInner** — `rsa_pubkey_hash: Option<[u8; 32]>` and `rsa_pubkey_der: Option<Vec<u8>>` at state.rs:157-160 already follow this pattern. The 21-01 refactor of `rsa_signer` to `Option<RsaBlindSigner>` is consistent with the existing public-key fields.

- **Per-section header comments naming the constraint** (`// D-07: ...`, `// AVAIL-02: ...`, `// PRIV-01: ...`) — used throughout coordinator/ to label code with its design-decision reference. Plan 21-01 follows: `// AUDIT-03: RoundSecretKey wraps BjSecretKey; bounded lifetime via Option<RsaBlindSigner> on RoundStateInner; Drop runs on transition_to(Phase::Idle)`.

- **`#[cfg(test)] mod tests` inline in the same file** — both `rsa.rs` and `state.rs` follow this convention. Plan 21-01's two new tests live in their respective file's existing tests modules.

- **Doc-comment paragraphs cite design decisions by ID + line ref** — example: rsa.rs:18-22's D-07 comment cites the upstream limitation; state.rs:82-91's RoundStateInner doc-comment cites D-07 + the HashMap-zeroize-limitation. Plan 21-01's D-132 rewrite follows: doc-comment NAMES `RoundSecretKey`, NAMES the trigger (`transition_to(Phase::Idle)`), NAMES the upstream limitation, ENDS with a charter-anchor reference closing the audit loop.

### Integration Points

- **All production call sites of `inner.rsa_signer.*`** — Plan 21-01 grep enumeration:
  - `coordinator/src/round/output_reg.rs:96-102` (`make_valid_token_sig` helper takes `&RsaBlindSigner`; callers pass `&inner.rsa_signer`).
  - `coordinator/src/api/handlers.rs` (POST /round/info uses rsa_signer.public_key_hash(); POST /round/output uses rsa_signer.blind_sign()).
  - `coordinator/src/round/input_reg.rs`, `coordinator/src/round/manager.rs` (round bootstrap reads rsa_signer.public_key for /info response).
  - Each call becomes `.as_ref().expect("rsa_signer must be Some during active round")` or `.as_ref().unwrap()`. The Option is None only when round is Idle (no inner) OR when the signer has been explicitly dropped post-Signing (NOT used in this codebase per D-130b). Plan-phase confirms each call site is on a code path that REQUIRES the signer to be Some — if any call site can legitimately reach with Some=None, that's a real bug not a refactor concern.

- **All test-fixture call sites of `RsaBlindSigner::generate().unwrap()` constructing RoundStateInner** — already enumerated: state.rs:270, state.rs:311, signing.rs:450, signing.rs:496, signing.rs:521, signing.rs:560. Plus output_reg.rs::tests. 21-01 refresh is mechanical: wrap in `Some(...)`.

- **`docs/` directory currently has 2 files** (PROTOCOL.md, branch-protection.md). Plan 21-02 adds `docs/AUDIT-CHARTER.md` as the third. No `docs/index.md` or similar — README §Security Model is the only entry point.

- **README.md §Security Model anchor (line 281)** — Plan 21-02 D-143 inserts the audit-charter callout. The README has no auto-generated TOC; the callout is reachable via the existing `## Security Model` heading.

</code_context>

<specifics>
## Specific Ideas

- **The audit-readiness storyline is "structurally bounded, not best-effort"** — every Phase 21 artifact reinforces this single claim. AUDIT-03 makes the RSA SecretKey lifetime expressible as a Rust type (`Option<RsaBlindSigner>`); AUDIT-01 charter §5 prose cites the type signature directly; AUDIT-02 audit.toml RUSTSEC-2023-0071 rationale names the new mitigation by name. An auditor reading any one of the three artifacts is one click from the other two. This coherence is the load-bearing audit deliverable; bit-rotting any one link breaks the story.

- **Phase 21 is the v1.5 milestone-close marker** — the planned charter, audit.toml refresh, and bounded-window mitigation are the difference between "v1.4 multi-script shipped" and "v1.5 audit-ready". Plan-phase respects: no scope creep beyond REQUIREMENTS AUDIT-01/02/03; no new feature, no external pen-test (out of scope), no v1.6+ residual-risk closure (only documentation of the gaps in charter §7).

- **`Option<RsaBlindSigner>` is the lifetime-bound type** (not `Option<RoundSecretKey>`) — REQUIREMENTS uses both framings, the cleaner reconciliation (per D-128) is that the newtype wraps `BjSecretKey` inside `RsaBlindSigner` and the round-state field is `Option<RsaBlindSigner>`. Charter prose handles this transparently: "Option<RsaBlindSigner> on RoundStateInner transitively owns RoundSecretKey; setting to None triggers the Drop chain."

- **Best-effort scrub is honest, not weak** — the upstream `blind-rsa-signatures 0.17.x` does not expose its SecretKey internals; the Marvin Attack mitigation depends on ephemeral keys + bounded chosen-ciphertext counts, NOT on guaranteed in-memory erasure. The charter says this explicitly: "the structural lifetime bound is the load-bearing mitigation against the Marvin Attack model preconditions (long-lived key + unlimited measurements); in-place buffer scrub is best-effort and audit-charter §5 documents the upstream limitation."

- **File:symbol anchors beat line numbers for audit longevity** — line refs in the charter would bit-rot within weeks; `coordinator/src/blind/rsa.rs::RoundSecretKey::drop` is stable across reformats. Charter table includes an orientation line ("approx. line NN at v1.5 ship tag") but the durable anchor is the symbol.

- **21-RESEARCH.md is non-trivial** — three research questions to ground D-129 (Drop body shape), D-129a (blind-rsa-signatures internals), and D-141 (new advisories diff). Plan-phase budgets ~20-40 min for the researcher; planner consumes the report to lock D-129 / D-141 decisions.

</specifics>

<deferred>
## Deferred Ideas

- **`Zeroize` impl upstream on `blind-rsa-signatures::SecretKey`** — would close AUDIT-03's "best-effort" gap. Upstream contribution candidate; out of scope for v1.5. Charter §7 (a) RUSTSEC-2023-0071 paragraph notes "future v1.6+ contribution: propose upstream Zeroize impl on blind-rsa-signatures SecretKey; closure of the residual-risk best-effort qualifier."

- **`KeyScriptMismatch` Bip322Error variant** (carried from Phase 19 §Deferred) — cleaner semantics than reusing `ScriptTypeMismatch` for the spk↔key cross-check. Adds 1 variant + 1 Display impl + 1 PII-safety test case. v1.6+ if external audit flags the dual-meaning reuse as a documentation smell.

- **Per-input variable `fee_share`** — REQUIREMENTS `Future requirements`. v1.6+; changes wire protocol.

- **TEST-EXT-01/02/03** — cross-implementation differential fixtures, regtest on-chain anchor test, automated v1.3↔v1.4 backwards-compat matrix. Charter §7 (b) DOCUMENTS the gap; v1.6+ closes it.

- **CARRY-TOR-UAT** — Tor-mode verification harness with ≥257 concurrent .onion streams. Phase 8 v1.2 carry-forward; v1.6+ closure.

- **CARRY-REPAIR-01-PR** — REPAIR-01 PR observation closure. v1.6+ on next external PR moment.

- **B-03 dynamic fee estimation** — pre-mainnet requirement. Orthogonal to v1.5 audit-readiness; charter §7 (c) cites in operational-residuals as "pre-mainnet requirement to be addressed before mainnet flip."

- **External penetration test execution** — REQUIREMENTS `Out of Scope`. Phase 21 ships the charter that EN ABLES an external pen-test engagement; the engagement itself is a separate milestone after v1.5 ships.

- **`docs/AUDIT-CHARTER.md` versioning** — when v1.6+ ships and the charter needs a refresh, plan-phase decides between in-place edit (rolling charter) vs. versioned charters (`docs/AUDIT-CHARTER-v1.5.md`, `docs/AUDIT-CHARTER-v1.6.md`). For now (single-charter v1.5), no versioning. v1.6+ planning revisits.

- **Promote `RoundSecretKey` to `shared/` crate** — if client ever needs round-secret semantics (it doesn't today; client doesn't hold the RSA secret), the newtype could live in shared. Phase 21 keeps it coordinator-local. v1.6+ if client-side ephemeral keys become a feature.

- **`Reviewed:` log structure on audit.toml** — append-only review history (per the alternative discussed in audit.toml area). Phase 21 keeps the single `Reviewed:` line per D-140. v1.6+ if cumulative review history becomes a compliance requirement.

- **Per-ignore TOML table layout** (`[advisories.ignore."RUSTSEC-..."]`) — REJECTED for v1.5 (D-142); cargo-audit schema is the flat list. v1.6+ if upstream cargo-audit supports the structured form and we want machine-extractable rationale.

</deferred>

---

*Phase: 21-audit-charter-zeroization-tightening*
*Context gathered: 2026-05-31*
