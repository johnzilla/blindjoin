---
phase: 21-audit-charter-zeroization-tightening
verified: 2026-05-31T23:55:00Z
resolved: 2026-06-01T00:30:00Z
status: passed
score: 5/5 must-haves verified; all 3 human verification items resolved in 21-HUMAN-UAT.md (CR-01 = WARNING accepted + documented in AUDIT-CHARTER.md §7; line-number drift = FIX-NOW applied at 5 citation sites; README link rendering = visually confirmed by user via grip preview)
overrides_applied: 0
human_verification_resolution: All 3 human_verification items below were dispositioned in 21-HUMAN-UAT.md (status: resolved, summary: 3 passed / 0 pending). See 21-HUMAN-UAT.md for the per-item resolution notes and the fix commits 7018fc8 (CR-01 + WR-01) and 86b2edd (README/FAQ/PROTOCOL.md propagation).
human_verification:
  - test: "Decide whether the REVIEW.md CR-01 finding (let _ = on FSM transitions at signing.rs:279-280, blame.rs:219-220, output_reg.rs:30-31) constitutes a real gap against SC#3's structurally-enforced mitigation claim, OR an acceptable defense-in-depth concern surfaced by the reviewer that the chain still holds today."
    expected: "User decides one of: (a) treat as BLOCKER — Phase 21 must add explicit transition-result handling + `state.inner = None` fallback at each of the 3 success-path FSM trigger sites before close; (b) accept as WARNING with rationale — the current write-lock semantics make `Signing → Broadcast` and `Blame/OutputReg → Blame` reachable-failures unreachable today, and the charter's load-bearing claim still holds; if (b), document this in a Residual Risk row in AUDIT-CHARTER.md §7 sub-bucket (b) Protocol-level so future auditors see the framing; (c) defer to a Phase 22 follow-up."
    why_human: "This is a design-judgment call: is 'silently discards a Result that is unreachable-failing under the current concurrency model' an AUDIT-03 hole or an acceptable trade-off? The structural test passes (verified). The Drop chain fires on the happy path (verified). But the charter §5 verification subsection does not acknowledge the silent-failure risk, and the reviewer flagged it as Critical. Codebase evidence alone cannot resolve whether to widen the scope of Phase 21."
  - test: "Decide whether the line-number drift across rsa.rs (state.rs:194-200 / state.rs:195 anchors), audit.toml (state.rs:194-200), and AUDIT-CHARTER.md (state.rs:194-200, transition_to at line 186) — actual location is state.rs:202 inside the block at 201-207, transition_to declared at line 193 — is a doc-only fix (WR-01) or a blocker."
    expected: "User decides: (a) accept as known doc-anchor drift — the file:symbol form is the durable anchor (charter §1 paragraph itself says so), and the parenthetical line numbers are orientation-only; OR (b) require the line-number citations to be corrected before marking phase complete; OR (c) replace every numeric line-anchor with file:symbol form (charter's own preferred convention)."
    why_human: "The charter explicitly states 'symbols survive line-number churn ... whereas a bare file:NN ref bit-rots' at lines 28-31, then proceeds to use bare file:NN anchors that ARE already wrong. This is structurally minor (audit narrative still resolves), but the charter's own preamble argues against the choice. Reviewer judgment is needed."
  - test: "Verify by sed inspection that the new README.md §Security Model `**External audit charter (v1.5):**` paragraph reads as intended in rendered Markdown, and that the link target opens AUDIT-CHARTER.md correctly when clicked on GitHub."
    expected: "Open README.md on github.com/johnzilla/blindjoin (or the local rendered preview); confirm: (a) the paragraph is between the Supply-chain hygiene paragraph and the Test infrastructure paragraph; (b) the `docs/AUDIT-CHARTER.md` link is blue + clickable; (c) clicking it loads AUDIT-CHARTER.md."
    why_human: "Visual rendering of a Markdown link target in GitHub's rendered view cannot be verified by grep — needs a human to load the page and click."
gaps: []
deferred: []
---

# Phase 21: Audit Charter & Zeroization Tightening Verification Report

**Phase Goal:** `docs/AUDIT-CHARTER.md` exists and an external auditor can read it cold, identify exactly which files and properties are in scope, and start reviewing without asking the project team for clarification; `.cargo/audit.toml` rationale strings reference the charter; the RSA SecretKey lifetime is *explicitly bounded* via a `RoundSecretKey` newtype so the charter can describe a structurally-enforced mitigation rather than "best-effort".

**Verified:** 2026-05-31T23:55:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | `docs/AUDIT-CHARTER.md` exists, committed in `main`, linked from `README.md` §Security Model, contains 8 H2 sections in AUDIT-01 mandated order | ✓ VERIFIED | File exists at `/Users/john/Desktop/vault/projects/github.com/blindjoin/docs/AUDIT-CHARTER.md` (574 lines). 8 H2 sections in order: §1 In-Scope Modules (line 23), §2 Threat Models per Module (line 59), §3 Cross-shape Rejection Properties (line 217), §4 v=2 OwnershipProof PSBT Handling (line 253), §5 RSA Secret Key Zeroization Window (line 307), §6 Out-of-Scope Components (line 409), §7 Residual Risks Accepted (line 429), §8 Glossary (line 529). Committed in commit `92ae533` (atomic landing per D-133a). Linked from README.md line 302: `[docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md)` between Supply-chain hygiene (line 300) and Test infrastructure (line 304). |
| 2 | `.cargo/audit.toml` updated — each `ignore` rationale references a `docs/AUDIT-CHARTER.md#section-anchor`; RUSTSEC-2023-0071 paragraph references AUDIT-03 bounded-window mitigation by name (no "best-effort"); Reviewed: bumped; no new transitive advisories | ✓ VERIFIED | 3 closing anchor lines present (lines 35, 42, 48). RUSTSEC-2023-0071 paragraph (lines 11-35) names AUDIT-03 explicitly: "AUDIT-03 RoundSecretKey + Option<RsaBlindSigner> bounded lifetime on RoundStateInner" (lines 20-21), cites `coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end` (line 26-27) as verification. The phrase "best-effort" is ABSENT from the file. `Reviewed: 2026-05-31` (line 7). 3 ignore IDs preserved verbatim: RUSTSEC-2023-0071, RUSTSEC-2025-0141, RUSTSEC-2024-0436. `cargo audit --no-fetch --json` returns 0 vulnerabilities + 0 warnings. |
| 3 | `coordinator/src/blind/rsa.rs` introduces `RoundSecretKey(BjSecretKey)` newtype with explicit `Drop`; round state holds `Option<RoundSecretKey>`-equivalent (`Option<RsaBlindSigner>`) and sets to `None` on Round end; D-07 comment rewritten as bounded statement | ✓ VERIFIED (with structural caveat — see WARNING below) | `pub struct RoundSecretKey(BjSecretKey);` exists at rsa.rs:33. `impl Drop for RoundSecretKey` at rsa.rs:52-69 (empty-crypto body, PII-safe `tracing::debug!` with target `blindjoin::audit`). `RsaBlindSigner.secret_key: RoundSecretKey` at rsa.rs:101. `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` at state.rs:110. Single `self.inner = None` assignment at state.rs:202 inside `transition_to(Phase::Idle)` block (state.rs:201-207). Rewritten D-07 doc-comment at rsa.rs:76-98 cites the transitive `rsa::RsaPrivateKey` Drop chain, names `Option<RsaBlindSigner>` as the lifetime bound, names `transition_to(Phase::Idle)` as the trigger, ends with `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` anchor (line 97). The substring "best-effort only" is ABSENT. **Structural caveat:** 3 production sites (signing.rs:279-280, blame.rs:219-220, output_reg.rs:30-31) use `let _ = state.transition_to(...)` which discards the FSM transition `Result`. Today this is safe (write-lock semantics make first-call failure unreachable), but the charter does NOT document the silent-failure path. See WARNING below. |
| 4 | Test exists that constructs a `RoundSecretKey`, drops it, and asserts the underlying buffer no longer matches original DER bytes (best-effort RAM scan acceptable — structural lifetime bound is load-bearing) | ✓ VERIFIED | `round_secret_key_buffer_overwritten_on_drop` exists at rsa.rs:258-297 (best-effort scrub test, gated `#[cfg_attr(not(target_os = "linux"), ignore = ...)]` per CD-50). Load-bearing structural FSM test `round_secret_key_dropped_on_round_end` exists at state.rs:307-340 — drives Signing → Broadcast → Idle FSM and asserts `state.inner.is_none()` post-transition with pre-transition assertion that `rsa_signer.is_some()`. Test result: `test round::state::tests::round_secret_key_dropped_on_round_end ... ok` (verified by `cargo test -p coordinator --lib round::state::tests::round_secret_key_dropped_on_round_end -- --exact`). |
| 5 | v1.3 `full_round::*` 8/8 + v1.4 `mixed_script_e2e_three_clients_broadcast` + Phase 20 fee tests all green; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo audit` 0 vulnerabilities | ✓ VERIFIED | `cargo test --test integration full_round`: 8 passed, 0 failed, 0 ignored (41.00s). `cargo test --test integration mixed_script_e2e`: 1 passed, 0 failed (2.91s — `mixed_script_e2e::mixed_script_e2e_three_clients_broadcast`). `cargo test -p coordinator --lib fee_share`: 2 passed (`fee_share_p2wpkh_only_matches_v14_baseline`, `fee_share_mixed_script_differs_from_uniform_baseline`). `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, no warnings. `cargo audit --no-fetch --json`: 0 vulnerabilities, 0 warnings. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `coordinator/src/blind/rsa.rs` | RoundSecretKey newtype + Drop + rewritten D-07 + scrub test | ✓ VERIFIED | `pub struct RoundSecretKey(BjSecretKey);` at line 33; `impl Drop for RoundSecretKey` at lines 52-69; rewritten D-07 at lines 76-98 with charter anchor; scrub test at lines 258-297. Wired: `RsaBlindSigner.secret_key: RoundSecretKey` (line 101) consumed by `generate()` (line 110), `blind_sign()` (line 124 via `.as_inner()`), `from_der_secret_key()` (line 132), `secret_key_der()` (line 137). Data flows: 3 production call sites consume the Option<RsaBlindSigner> chain at handlers.rs:383, input_reg.rs:71, manager.rs:63. |
| `coordinator/src/round/state.rs` | rsa_signer: Option<RsaBlindSigner> field + structural test | ✓ VERIFIED | `pub rsa_signer: Option<RsaBlindSigner>` at line 110. Structural FSM test `round_secret_key_dropped_on_round_end` at lines 307-340. The previous bare-field form is absent (grep confirms). |
| `docs/AUDIT-CHARTER.md` | 8 H2 sections in AUDIT-01 order | ✓ VERIFIED | 574 lines, 8 H2 sections in mandated order (verified by `grep -nE "^## "`). All 9 cross-shape test names enumerated in §3 (grep confirms 9 occurrences). All 12 file:symbol anchors in §1/§2/§4/§5 resolve in codebase (verified by spot-grep). §5 H2 slug `rsa-secret-key-zeroization-window` matches the anchor referenced from rsa.rs and audit.toml. |
| `.cargo/audit.toml` | 3 charter-anchor closing lines + RUSTSEC-2023-0071 rewrite + Reviewed: bump | ✓ VERIFIED | 3 `See docs/AUDIT-CHARTER.md#...` closing lines at lines 35, 42, 48. RUSTSEC-2023-0071 rewrite at lines 11-35 names AUDIT-03 + Option<RsaBlindSigner> + `rsa-0.9.10/src/key.rs:76-82` + `round_secret_key_dropped_on_round_end`. `Reviewed: 2026-05-31` at line 7. Flat TOML layout preserved (no sub-tables). |
| `README.md` | §Security Model audit-charter callout | ✓ VERIFIED | `**External audit charter (v1.5):**` paragraph at line 302, between Supply-chain hygiene (line 300) and Test infrastructure v1.3 Phase 9 (line 304). Markdown link `[docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md)` present. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `coordinator/src/round/state.rs::transition_to(Phase::Idle)` | `coordinator/src/blind/rsa.rs::RoundSecretKey::drop` | `self.inner = None` at state.rs:202 → Drop chain | ✓ WIRED (with caveat) | Drop chain fires when `inner = None` is set. The chokepoint is the SOLE site assigning `inner = None` (verified by grep). Structural test passes. **Caveat:** the 3 production trigger sites discard the transition `Result` (`let _ =`), so a future regression that makes the first transition reachable-failing could silently bypass the chokepoint. See WARNING below. |
| `coordinator/src/round/input_reg.rs::register_input` | `coordinator/src/blind/rsa.rs::RsaBlindSigner::blind_sign` | `.as_ref().expect("rsa_signer must be Some during InputReg")` at input_reg.rs:71 | ✓ WIRED | Production call site traverses the Option via `.as_ref().expect(...)` with phase-specific panic message. |
| `coordinator/src/api/handlers.rs::register_output` | `coordinator/src/blind/rsa.rs::RsaBlindSigner::public_key` | `.rsa_signer.as_ref().expect("rsa_signer must be Some during OutputReg").public_key.clone()` at handlers.rs:383 | ✓ WIRED | Production call site traverses the Option via `.as_ref().expect(...)` with phase-specific panic message. |
| `coordinator/src/round/manager.rs::start_round` | `coordinator/src/round/state.rs::RoundStateInner.rsa_signer` | `rsa_signer: Some(signer)` at manager.rs:63 | ✓ WIRED | Production field-init wraps signer in `Some(...)`. |
| `.cargo/audit.toml::RUSTSEC-2023-0071` | `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` | `# See docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window for the full rationale.` at audit.toml:35 | ✓ WIRED | Anchor slug resolves to §5 H2 heading "RSA Secret Key Zeroization Window" (GitHub auto-slug). |
| `.cargo/audit.toml::RUSTSEC-2025-0141/0436` | `docs/AUDIT-CHARTER.md#residual-risks-cargo-audit-advisories` | Closing lines at audit.toml:42, 48 | ✓ WIRED | Anchor slug resolves to §7 H3 heading "Residual Risks: cargo-audit Advisories" (line 438). |
| `coordinator/src/blind/rsa.rs::D-07 comment` | `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` | Anchor reference at rsa.rs:97 | ✓ WIRED | The rewritten D-07 doc-comment ends with the charter anchor reference. |
| `README.md::§Security Model callout` | `docs/AUDIT-CHARTER.md` | `[docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md)` at README.md:302 | ✓ WIRED | Relative markdown link; file exists. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` | wrapped `RoundSecretKey(BjSecretKey)` | `RsaBlindSigner::generate()` → `BjKeyPair::generate(&mut DefaultRng, 2048)` at rsa.rs:109 | Yes — fresh RSA-2048 keypair per round | ✓ FLOWING |
| `RoundSecretKey::drop` event | `tracing::debug!` PII-safe static-string event | Direct emission in drop body at rsa.rs:64-67 | Yes — tracing event under target `blindjoin::audit` | ✓ FLOWING |
| `cargo audit` ignore-list verdict | 3 RUSTSEC IDs | `.cargo/audit.toml::ignore = [...]` lines 36, 43, 49 | Yes — `cargo audit --no-fetch --json` returns 0 vulns + 0 warnings | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Structural FSM Drop chain | `cargo test -p coordinator --lib round::state::tests::round_secret_key_dropped_on_round_end -- --exact` | 1 passed; 0 failed | ✓ PASS |
| Best-effort scrub (macOS gated ignored) | `cargo test -p coordinator --lib blind::rsa::tests` | 5 passed; 0 failed; 1 ignored (scrub test on macOS as expected per CD-50) | ✓ PASS |
| v1.3 P2WPKH invariant | `cargo test --test integration full_round` | 8 passed; 0 failed; 0 ignored (41.00s) | ✓ PASS |
| v1.4 multi-script invariant | `cargo test --test integration mixed_script_e2e` | 1 passed; 0 failed (2.91s) | ✓ PASS |
| Phase 20 FEE-03 fee accuracy | `cargo test -p coordinator --lib fee_share` | 2 passed; 0 failed | ✓ PASS |
| Clippy `-D warnings` clean | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0; 0 warnings | ✓ PASS |
| cargo audit 0/0 with refreshed audit.toml | `cargo audit --no-fetch --json` | 0 vulnerabilities, 0 warnings | ✓ PASS |
| 8 H2 sections in charter in mandated order | `grep -nE "^## " docs/AUDIT-CHARTER.md` | 8 sections in correct order: In-Scope Modules, Threat Models per Module, Cross-shape Rejection Properties, v=2 OwnershipProof PSBT Handling, RSA Secret Key Zeroization Window, Out-of-Scope Components, Residual Risks Accepted, Glossary | ✓ PASS |

### Probe Execution

Not applicable — Phase 21 ships no project-convention probes (no `scripts/*/tests/probe-*.sh`). The probe-equivalent is the structural FSM test + the integration suite, both verified above.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| AUDIT-01 | 21-02-PLAN.md | Publish `docs/AUDIT-CHARTER.md`, linked from README §Security Model, 8 mandated sections | ✓ SATISFIED | Charter exists at 574 lines with all 8 H2 sections in REQUIREMENTS order. README §Security Model callout at line 302. Atomic commit `92ae533`. |
| AUDIT-02 | 21-02-PLAN.md | Update `.cargo/audit.toml` ignore rationales (charter anchors, RUSTSEC-2023-0071 rewrite, Reviewed: bump, no silent additions) | ✓ SATISFIED | 3 closing anchor lines, RUSTSEC-2023-0071 paragraph names AUDIT-03 bounded-window mitigation, `Reviewed: 2026-05-31`, 3 IDs preserved verbatim, no new ignores added. |
| AUDIT-03 | 21-01-PLAN.md | `RoundSecretKey(BjSecretKey)` newtype + explicit Drop + `Option<RoundSecretKey>`-equivalent in round state + D-07 rewrite + scrub test | ✓ SATISFIED (with structural caveat — see WARNING) | `RoundSecretKey` at rsa.rs:33. `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` at state.rs:110 (transitively owns RoundSecretKey via RsaBlindSigner). D-07 rewritten. Structural FSM test green; scrub test ignored on macOS as designed. **See WARNING below: the 3 production trigger sites discard the FSM `Result`, leaving a defense-in-depth gap that the charter does not currently document.** |

No orphaned requirements detected (REQUIREMENTS.md AUDIT-01, AUDIT-02, AUDIT-03 all map to Phase 21 plans and are claimed in the plans' `requirements:` fields).

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `coordinator/src/round/signing.rs` | 279-280 | `let _ = state.transition_to(Phase::Broadcast); let _ = state.transition_to(Phase::Idle);` — discards FSM transition Result | ⚠️ Warning | Success-path Drop trigger. Today safe (write-lock makes Signing → Broadcast unreachable-failing), but the charter §5 does not document this. If a future regression makes the first transition reachable-failing, `Signing → Idle` is NOT a valid FSM edge — so the second call returns `Err` too, `inner` stays live, and the AUDIT-03 lifetime bound is silently broken. CR-01 in REVIEW.md. |
| `coordinator/src/round/blame.rs` | 219-220 | `let _ = state.transition_to(Phase::Blame); let _ = state.transition_to(Phase::Idle);` | ⚠️ Warning | Same failure mode as above on the blame path. |
| `coordinator/src/round/output_reg.rs` | 30-31 | `let _ = state.transition_to(Phase::Blame); let _ = state.transition_to(Phase::Idle);` | ⚠️ Warning | Same failure mode as above on the missing-output blame path. |
| `coordinator/src/api/handlers.rs` | 286, 427 | `let _ = guard.transition_to(...)` on InputReg→OutputReg + OutputReg→Signing | ℹ️ Info | Not AUDIT-03 trigger sites; happy-path advancement under write lock. Diagnostic-loss only. |
| `docs/AUDIT-CHARTER.md` and `.cargo/audit.toml` and `coordinator/src/blind/rsa.rs` | charter:39/194/333-336; audit.toml:22-23; rsa.rs:20/32/82-83 | Line-number drift: cite `state.rs:194-200` / `state.rs:195` but actual chokepoint is at `state.rs:202` (block 201-207). `transition_to` declared at line 193. | ℹ️ Info | The file:symbol form (`state.rs::transition_to`) is the durable anchor per the charter's own preamble. Line numbers are orientation-only. WR-01 in REVIEW.md. |
| `coordinator/src/blind/rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop` | doc-comment lines 240-255, 274-277 | Mechanism description claims to detect zeroize-chain regression but the needle is DER bytes (separate live allocation held in `der_fingerprint` until end of test) — does not match what's actually tested | ℹ️ Info | Test is correctly labeled as "sanity ceremony"; structural test remains load-bearing. WR-02 in REVIEW.md. |
| `coordinator/src/api/handlers.rs` | 130-142 | Blinded-token size check accepts both 256 and 512 bytes (RSA-2048 and RSA-4096) but coordinator hardcodes RSA-2048 at rsa.rs:109 | ℹ️ Info | Wastes work on RSA-4096-sized inputs (256 bytes extra decode + lock + TOCTOU + blind-sign attempt). Pre-existing per WR-03 in REVIEW.md; not Phase 21-introduced. |

### Human Verification Required

#### 1. CR-01 disposition — silent FSM transition failure on success path

**Test:** Decide whether the REVIEW.md CR-01 finding constitutes a real gap against SC#3's structurally-enforced mitigation claim, OR an acceptable defense-in-depth concern that the charter should document under §7 Residual Risks.

**Current evidence:**
- The 3 production sites at signing.rs:279-280, blame.rs:219-220, output_reg.rs:30-31 all use `let _ = state.transition_to(...)`.
- The FSM definition (state.rs:32-50) does NOT allow `Signing → Idle` directly — only `Signing → Broadcast → Idle` or `Signing → Blame → Idle`.
- The chokepoint at state.rs:202 (the SOLE `inner = None` assignment) is gated by `if next == Phase::Idle` AND `can_transition_to` succeeding.
- If `transition_to(Phase::Broadcast)` ever returns `Err` (e.g., concurrent state mutation, future FSM tightening), the state stays in Signing → the second call attempts illegal Signing → Idle → returns `Err` → `inner` stays live with the per-round RSA key.
- TODAY this is safe: `assemble_and_broadcast` is reached only under a write lock that observes `Phase::Signing`; the write lock prevents concurrent phase change; `Signing → Broadcast` is therefore unreachable-failing.
- The structural test `round_secret_key_dropped_on_round_end` passes because it controls the FSM in a context where neither call can fail — it does not exercise the silent-failure path.
- The charter §5 verification subsection cites the structural test as the load-bearing CI gate and does not mention the silent-failure risk.
- AUDIT-CHARTER.md §7 sub-bucket (b) Protocol-level does not include this as a documented residual risk.

**Expected:** User decides one of:
- (a) **BLOCKER** — fix the 3 trigger sites to handle the Result + force `state.inner = None` on rejection (per CR-01's fix block) BEFORE marking Phase 21 complete. This preserves the structural claim cleanly.
- (b) **WARNING with documentation** — accept that under current concurrency semantics the failure mode is unreachable, but add a Residual Risk row to AUDIT-CHARTER.md §7 sub-bucket (b) Protocol-level acknowledging "FSM trigger silent-failure path is unreachable under v1.5 write-lock semantics; future FSM tightening or concurrency model change must re-verify". This makes the structural claim honest about its caveat.
- (c) **DEFER to Phase 22 follow-up** — leave the code as-is, mark this as a known issue in `STATE.md`, address in a follow-up phase.

**Why human:** This is a design-judgment call about the meaning of "structurally enforced". The codebase supports either reading; the reviewer flagged it as Critical because the entire AUDIT-03 mitigation exists to prevent this exact failure mode being silent. But the current concurrency model makes the failure mode unreachable, so accepting it as defense-in-depth is also defensible. Goal-backward verification cannot resolve this without project-team intent.

#### 2. Line-number drift in line citations across rsa.rs / audit.toml / AUDIT-CHARTER.md

**Test:** Decide whether to (a) accept the drift, (b) correct the line citations to `state.rs:201-207` (block) and `state.rs:202` (chokepoint), or (c) replace numeric anchors with file:symbol form throughout (charter's own preferred convention per its preamble at lines 28-31).

**Current evidence:** Multiple citations name `state.rs:194-200` / `state.rs:195` as the chokepoint location. Actual location: `transition_to` declared at line 193; the `if !can_transition_to { return Err(...); }` early-return block is at lines 194-200; the `if next == Phase::Idle { self.inner = None; ... }` block is at lines 201-207, with `self.inner = None` at line 202. Examples:
- `coordinator/src/blind/rsa.rs:20`: `transition_to(Phase::Idle) (state.rs:194-200)`
- `coordinator/src/blind/rsa.rs:32`: `the FSM nulls at one chokepoint (state.rs:195)`
- `coordinator/src/blind/rsa.rs:82-83`: `sets self.inner = None (coordinator/src/round/state.rs:194-200)`
- `.cargo/audit.toml:22-23`: `transition_to(Phase::Idle) (coordinator/src/round/state.rs:194-200)`
- `docs/AUDIT-CHARTER.md:39`: `transition_to ... ~line 186` (actual: line 193)
- `docs/AUDIT-CHARTER.md:194`: `coordinator/src/round/state.rs:194-200`
- `docs/AUDIT-CHARTER.md:333-334`: `the SOLE site setting inner = None ... line 194-200`

**Expected:** User picks disposition. (Option c is the most defensible per the charter's own anchor-stability convention.)

**Why human:** Doc-only issue; structural verification still passes (the file:symbol anchors all resolve). But the charter explicitly argues against bare `file:NN` anchors in its preamble, then uses them; the inconsistency is judgement-needing.

#### 3. README markdown rendering verification

**Test:** Open `README.md` on github.com/johnzilla/blindjoin (or render locally); confirm the new `**External audit charter (v1.5):**` paragraph displays correctly between the Supply-chain hygiene paragraph and the Test infrastructure paragraph; click the `docs/AUDIT-CHARTER.md` link and confirm it loads.

**Expected:** Visual confirmation that the GitHub-rendered Markdown shows the paragraph in the correct place with a clickable link.

**Why human:** Visual rendering and GitHub anchor resolution behavior cannot be verified by grep.

### Gaps Summary

**No blocker gaps.** All 5 ROADMAP Success Criteria are verified by codebase evidence:
- SC#1 (charter exists, 8 sections, README link) ✓
- SC#2 (audit.toml refresh) ✓
- SC#3 (RoundSecretKey newtype + Option + D-07 rewrite) ✓ — with the structural caveat detailed in WARNING below.
- SC#4 (scrub test + structural test — both green) ✓
- SC#5 (full_round 8/8 + mixed_script 1/1 + Phase 20 FEE-03 2/2 + clippy clean + cargo audit 0/0) ✓

**Warnings raised by the code reviewer (REVIEW.md CR-01 + WR-01/02/03) that bear on the audit narrative:**

1. **CR-01 (Critical per reviewer):** The 3 production FSM trigger sites use `let _ = state.transition_to(...)`, discarding the transition `Result`. The structural test passes because it drives the FSM in a controlled context. Today's write-lock semantics make `Signing → Broadcast` unreachable-failing, so the chain holds in practice. But the AUDIT-03 narrative claims the lifetime bound is "structurally enforced" — and `let _ =` makes the bound fragile against future FSM tightening or concurrency-model changes. The charter §5 does not acknowledge this risk. Goal-backward verdict: the codebase evidence supports the structural claim under v1.5 semantics; the reviewer's concern is about robustness across future changes. This is a judgement call between (a) tightening the code (per CR-01's fix), (b) documenting as Residual Risk in charter §7, or (c) accepting as-is and re-visiting in Phase 22+.

2. **WR-01 (Warning):** Line-number citations in rsa.rs, audit.toml, and AUDIT-CHARTER.md cite `state.rs:194-200` / `state.rs:195` but the actual chokepoint is at `state.rs:202`. The file:symbol anchors all resolve; this is a doc-only inconsistency that the charter's own preamble (lines 28-31) explicitly argues against. Cosmetic but the charter contradicts itself.

3. **WR-02 (Warning):** The scrub test's doc-comment claims to detect zeroize-chain regressions, but the needle is extracted from a DER `Vec<u8>` (held by `der_fingerprint` until end of test) — not from the BigUint limbs that the upstream chain actually zeroizes. The test does not prove what its comment claims. Test is correctly labeled "sanity ceremony" so the AUDIT-03 narrative is not damaged, but the description is misleading.

4. **WR-03 (Warning):** Pre-existing — blinded-token size check at handlers.rs:130-142 accepts RSA-4096-sized inputs but coordinator hardcodes RSA-2048. Phase 21 did not introduce this; not a Phase 21 gap.

5. **WR-04/IN-01/IN-02/IN-03/IN-04:** Quality/clarity concerns not blocking AUDIT-* requirements.

**Recommendation:** Phase 21's automated verification surface is green. Whether CR-01 + WR-01 + WR-02 warrant a Phase 21 closure-gate fix vs. a Phase 22 follow-up is a project-team decision. The verifier surfaces all three to the user for explicit disposition (see Human Verification Required above).

---

_Verified: 2026-05-31T23:55:00Z_
_Verifier: Claude (gsd-verifier)_
