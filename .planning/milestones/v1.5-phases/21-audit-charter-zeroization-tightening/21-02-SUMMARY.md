---
phase: 21-audit-charter-zeroization-tightening
plan: 02
subsystem: docs
tags: [audit, audit-charter, audit-toml, rustsec, marvin-attack, audit-readiness, anchor-stability]

# Dependency graph
requires:
  - phase: 21-audit-charter-zeroization-tightening
    provides: "Plan 21-01 — RoundSecretKey newtype + Option<RsaBlindSigner> bounded lifetime; the post-21-01 symbol set (RoundSecretKey, RsaBlindSigner.secret_key: RoundSecretKey, Option<RsaBlindSigner> on RoundStateInner, transition_to(Phase::Idle) trigger, structural FSM test round_secret_key_dropped_on_round_end, best-effort scrub test round_secret_key_buffer_overwritten_on_drop) is what the charter §1 in-scope table + §5 zeroization-window narrative cite by name"
  - phase: 19-multi-script-signing-finish
    provides: "Production sign bodies + dispatcher-only public surface on shared::bip322 — charter §2 V1.4-CRIT-01 narrative cites; charter §1 in-scope table cites sign_simple / verify_simple / detect_script_type"
  - phase: 20-mixed-round-fee-accuracy
    provides: "Per-script vbyte table + ScriptType plumbing through ParticipantInput — charter §4 v=2 OwnershipProof narrative cites as the orthogonal-but-shared-derivation-point closure"
provides:
  - "docs/AUDIT-CHARTER.md — 574-line external audit charter; 8 H2 sections in REQUIREMENTS AUDIT-01 mandated order; hybrid voice per D-134 (tables for §1/§3/§6/§8, narrative for §2/§4/§5/§7); ~30-row glossary; 7-row out-of-scope table; 9-row cross-shape rejection table; 15-row in-scope file:symbol table"
  - "Refreshed .cargo/audit.toml — 3 charter-anchor closing lines (D-139); RUSTSEC-2023-0071 rationale paragraph rewritten to name AUDIT-03 bounded-window mitigation (D-139 + 21-RESEARCH OQ1) — no more 'best-effort'; cites transitive rsa::RsaPrivateKey Drop chain at rsa-0.9.10/src/key.rs:76-82 bounded by Option<RsaBlindSigner> on RoundStateInner; verification cite of round_secret_key_dropped_on_round_end; Reviewed: date bumped to 2026-05-31 (D-140); 3 ignore IDs preserved verbatim (D-141 + 21-RESEARCH OQ3); flat TOML layout preserved (D-142)"
  - "README.md §Security Model audit-charter callout — one-paragraph '**External audit charter (v1.5):**' insertion between the Supply-chain hygiene paragraph (line 300) and the Test infrastructure (v1.3 Phase 9) paragraph (now line 304); follows the established '**Category (vN.x):**' rollup convention (D-143 + CD-52)"
  - "Closed-loop anchor stability — every docs/AUDIT-CHARTER.md#<anchor> ref in audit.toml + rsa.rs D-07 comment resolves to a real H2/H3 heading in the charter; every file:symbol anchor in the charter §1/§2/§4/§5 resolves to a real symbol in the codebase; every cross-shape test name in §3 resolves at shared/tests/bip322_cross_shape.rs; the 4-way artifact loop (code → charter → audit.toml → README → back to code) is one-click navigable"
affects:
  - "v1.5 milestone close (this is the FINAL v1.5 phase plan — both AUDIT-01 + AUDIT-02 close here; AUDIT-03 closed at 21-01)"
  - "Future external audit engagement (the charter is the deliverable an auditor reads cold to start work without project-team clarification)"
  - "v1.6+ Phase 22+ — any future code change near in-scope symbols should preserve the file:symbol anchors named in charter §1 (or update charter atomically with the rename per the same D-133a discipline this plan exercises)"

# Tech tracking
tech-stack:
  added: []  # No new dependencies; Phase 21-02 is internal docs + config only.
  patterns:
    - "Atomic-commit cross-artifact landing (D-133a): when 3 mutually-referencing artifacts (docs/AUDIT-CHARTER.md, .cargo/audit.toml, README.md) need to ship together — stage all 3 and commit in ONE git commit, not three. Prevents the anchor-drift window where audit.toml references charter headings that don't yet exist on the branch."
    - "GitHub markdown auto-slug compatibility for cross-artifact anchors: when audit.toml + source-code D-07 comments reference charter headings by markdown anchor (e.g., #rsa-secret-key-zeroization-window), the charter H2 heading text must use word-spacing that GitHub's slugger renders as the expected slug. Use 'RSA Secret Key Zeroization Window' (slugs to 'rsa-secret-key-zeroization-window'), NOT 'RSA SecretKey Zeroization Window' (slugs to 'rsa-secretkey-zeroization-window' — GitHub does not split CamelCase). Use colon-form ('Residual Risks: cargo-audit Advisories') NOT em-dash form ('Residual Risks — cargo-audit Advisories' slugs to 'residual-risks--cargo-audit-advisories' with a double-hyphen). CD-49 grants executor discretion to refine slug naming so the load-bearing contract — audit.toml ref resolves — is met."
    - "File:symbol anchor stability (D-138) as durable cross-artifact convention: the charter cites code by 'file:symbol' (e.g., coordinator/src/blind/rsa.rs::RoundSecretKey), with the parenthetical line number as orientation only. Symbol-based refs survive line-number churn across reformats. The grep-verify cross-check at Task 3 Edit C is the load-bearing gate."
    - "Bare-path comment closing-line convention for TOML rationale (D-139): per-ignore comment blocks in .cargo/audit.toml end with 'See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.' — bare path + anchor, no markdown link syntax. TOML comments render nowhere; markdown is overhead."

key-files:
  created:
    - "docs/AUDIT-CHARTER.md — NEW, 574 LOC, 8 H2 sections + 7 H3 sub-sections + 4 narrative + 4 tables (in-scope, cross-shape, out-of-scope, glossary)"
  modified:
    - ".cargo/audit.toml — 7 lines deleted (old prose paragraph for RUSTSEC-2023-0071) + ~24 lines added (new AUDIT-03-named rationale + 3 closing anchor lines + Reviewed: date bump). Net 53 LOC after vs 41 LOC before."
    - "README.md — 1 line deleted (no, +1 paragraph inserted between lines 300 and 302; net +1 paragraph + surrounding blank line)"

key-decisions:
  - "D-133a (atomic landing): 3 files (docs/AUDIT-CHARTER.md + .cargo/audit.toml + README.md) shipped in commit 92ae533 as a single atomic landing — prevents the anchor-drift window where audit.toml references charter headings that don't yet exist on the branch."
  - "D-134 (hybrid voice): §1/§3/§6/§8 are tables (in-scope modules, cross-shape rejection properties, out-of-scope components, glossary); §2/§4/§5/§7 are narrative (threat models per module, v=2 OwnershipProof PSBT handling, RSA Secret Key zeroization window, residual risks accepted). Tables for enumerable facts an auditor scans; narrative for threats and dispositions an auditor reads end-to-end."
  - "D-135 (§6 extended scope): out-of-scope table extends beyond REQUIREMENTS' Tor+PKARR baseline to all 3rd-party crypto crates — arti-client, pkarr, blind-rsa-signatures internals, bip322 = '=0.0.10' internals, rust-bitcoin + secp256k1, bdk_wallet, plus external pen-test execution. 7 rows. Each row names the rationale (upstream audit posture / consensus primitive / operational engagement)."
  - "D-136 (§7 3 sub-buckets): residual risks split into (a) cargo-audit advisories — 3 entries mirroring audit.toml; (b) protocol-level — heterogeneous-input tradeoff, V1.4-MIN-02 uniform-output fingerprint, TEST-EXT-01/02/03 gap; (c) operational — single-coordinator trust, sybil dilution, PKARR replay, B-03 dynamic fee. Each item has ACCEPTED-with-rationale or DOCUMENTED-GAP disposition."
  - "D-137 (§8 scope): glossary covers active v1.4/v1.5 identifiers (~30 entries). Retired pre-v1.4 IDs (REPAIR-01 v1.3 forensics, v1.0 PRIV-*, Phase 8 STREAM-*) point at .planning/milestones/v1.0-1.3-* archives."
  - "D-138 (file:symbol anchors): every code reference in the charter uses file::symbol form, with the line number as a parenthetical orientation aid only. Stable across reformats; bit-rot-resistant; greppable for the anchor-stability sweep at Task 3 Edit C."
  - "D-139 (bare-path anchor closing-lines): each of the 3 ignore comment blocks in .cargo/audit.toml gets a closing line 'See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.' — bare relative path + anchor, NO markdown link syntax. TOML comments render nowhere."
  - "D-140 (Reviewed: date bump): header line bumped from '# Reviewed: 2026-05-26.' to '# Reviewed: 2026-05-31.' — the actual 21-02 commit date (today). The review happened when Phase 21 landed, not before."
  - "D-141 (new-advisory detection): cargo audit --json against current Cargo.lock returns 0 vulnerabilities + 0 warnings with the existing 3 ignores. Per 21-RESEARCH OQ3 finding (advisory DB last commit eaf48e7, 2026-05-29) the diff is empty — 3 existing IDs locked verbatim; no new ignore-or-fix decisions needed."
  - "D-142 (flat TOML layout preserved): [advisories]\\nignore = ['RUSTSEC-...', ...] with prose comments — no [advisories.ignore.'RUSTSEC-...'] sub-tables. cargo-audit's documented schema is the flat list; sub-table extension is upstream-uncertain and out of scope."
  - "D-143 (README callout placement): the '**External audit charter (v1.5):**' paragraph inserted AFTER the Supply-chain hygiene paragraph and BEFORE the Test infrastructure (v1.3 Phase 9) paragraph in §Security Model — established '**Category (vN.x):** prose' convention used by 5 existing hardening rollups. One paragraph, no sub-bullets, markdown link to docs/AUDIT-CHARTER.md."
  - "CD-48 (no new advisories): no new ignore-or-fix decisions because 21-RESEARCH OQ3 + Task 2 Edit E re-confirmation both returned 0 vulnerabilities + 0 warnings. The 3 existing IDs are locked verbatim."
  - "CD-49 (anchor slug refinement): the §5 H2 heading text was finalized as 'RSA Secret Key Zeroization Window' (with a space between 'Secret' and 'Key') so GitHub's auto-slugger produces 'rsa-secret-key-zeroization-window' to match the anchor already cited by .cargo/audit.toml + coordinator/src/blind/rsa.rs D-07 comment. The §7 H3 headings use colon form ('Residual Risks: cargo-audit Advisories' etc.) NOT em-dash form so the slug is clean 'residual-risks-cargo-audit-advisories' without a double-hyphen. CD-49 explicitly grants this discretion to the executor."
  - "CD-51 (§4 paragraph count): 5 paragraphs (the construction boundary, the verification boundary, the V1.4-CRIT-01 cross-check ordering, the wire-byte lock via Phase 19 parity tests, the orthogonality with Phase 20 fee accuracy). Within the CONTEXT 4-8 range."
  - "CD-52 (README line confirmation): the live README.md still has Supply-chain hygiene at line 300 and Test infrastructure at (then-)line 302 at 21-02 execution time, matching 21-RESEARCH §11. Insertion landed cleanly between them — Test infrastructure now at line 304."

patterns-established:
  - "Cross-artifact anchor stability via atomic landing (D-133a) + GitHub-slugger-aware heading text (CD-49): when shipping mutually-referencing documentation + config files, ship them as one git commit and pick heading text that the markdown auto-slugger produces the same way the references spell the anchor."
  - "Bare-path closing-line convention for TOML rationale (D-139): per-ignore comment blocks end with 'See <relative path>#<anchor> for the full rationale.' — bare, no markdown link syntax."
  - "Hybrid voice charter authoring (D-134): tables for enumerable facts (in-scope modules, rejection properties, out-of-scope, glossary); narrative for threats + dispositions (threat models, PSBT handling, zeroization window, residual risks)."

requirements-completed: [AUDIT-01, AUDIT-02]

# Metrics
duration: 11min
completed: 2026-05-31
---

# Phase 21 Plan 02: Audit Charter & Zeroization Tightening — AUDIT-01 + AUDIT-02 Summary

**Ships docs/AUDIT-CHARTER.md (574-line external audit charter, 8 H2 sections per REQUIREMENTS), refreshes .cargo/audit.toml with charter anchors + RUSTSEC-2023-0071 rationale rewritten to name AUDIT-03 bounded-window mitigation (no more "best-effort"), and inserts README.md §Security Model callout — all 3 files landed in commit 92ae533 as ONE atomic commit per D-133a, closing the 4-way artifact loop (code ↔ charter ↔ audit.toml ↔ README ↔ back to code) with zero anchor drift.**

## Performance

- **Duration:** ~11 minutes
- **Started:** 2026-05-31T23:09:38Z (continuation from 21-01 STATE.md last_updated)
- **Completed:** 2026-05-31T23:20:26Z
- **Tasks:** 3
- **Files modified:** 3 (1 new, 2 modified)

## Accomplishments

- **AUDIT-01 closure:** `docs/AUDIT-CHARTER.md` shipped at 574 lines with the 8 H2 sections in the REQUIREMENTS-mandated order. Section structure:
  1. `## In-Scope Modules` (TABLE, 15 file:symbol rows)
  2. `## Threat Models per Module` (NARRATIVE, 4 H3 sub-sections: V1.4-CRIT-01, V1.4-CRIT-02, V1.4-MIN-02, RSA Marvin Attack)
  3. `## Cross-shape Rejection Properties` (TABLE, 9 rows enumerating each `reject_*` test from `shared/tests/bip322_cross_shape.rs` with the asserted `Bip322Error` variant)
  4. `## v=2 OwnershipProof PSBT Handling` (NARRATIVE, 5 paragraphs covering construction boundary at `client/src/round/input.rs::build_v2_psbt_input_b64`, verification boundary at `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness`, CRIT-01 cross-check ordering, Phase 19 byte-equality wire lock, Phase 20 fee path shared-derivation-point closure)
  5. `## RSA Secret Key Zeroization Window` (NARRATIVE, 5 paragraphs naming the bounded-lifetime claim via `Option<RsaBlindSigner>`, the SOLE FSM trigger `transition_to(Phase::Idle)` at state.rs:194-200, the transitive `<rsa::RsaPrivateKey as Drop>::drop` chain at `rsa-0.9.10/src/key.rs:76-82`, the newtype's value as **lifetime expression** not redundant scrub, and the 2-test split with structural FSM test as load-bearing CI gate + best-effort scrub as Linux-gated sanity)
  6. `## Out-of-Scope Components` (TABLE, 7 rows per D-135 — arti-client, pkarr, blind-rsa-signatures internals, bip322 internals, rust-bitcoin + secp256k1, bdk_wallet, external pen-test execution)
  7. `## Residual Risks Accepted` (NARRATIVE with 3 H3 sub-sections per D-136: cargo-audit advisories, protocol-level, operational)
  8. `## Glossary` (TABLE, 30 rows of active v1.4/v1.5 identifiers per D-137)
- **AUDIT-02 closure:** `.cargo/audit.toml` refreshed. The RUSTSEC-2023-0071 rationale paragraph dropped the previous "destroys the key via `zeroize` after the round broadcasts" phrasing and now names the AUDIT-03 bounded-window mitigation explicitly: cites `RoundSecretKey` + `Option<RsaBlindSigner>` lifetime bound, names the SOLE FSM trigger `transition_to(Phase::Idle)` at `state.rs:194-200`, cites the transitive `<rsa::RsaPrivateKey as Drop>::drop` chain at `rsa-0.9.10/src/key.rs:76-82` that zeroizes `d`, `primes`, `precomputed` unconditionally, and cites the verification test `round_secret_key_dropped_on_round_end`. The 3 existing ignore IDs (RUSTSEC-2023-0071, RUSTSEC-2025-0141, RUSTSEC-2024-0436) are preserved verbatim per 21-RESEARCH OQ3 finding. 3 new "See docs/AUDIT-CHARTER.md#<anchor> for the full rationale." closing lines added (D-139). `Reviewed:` date bumped from `2026-05-26` to `2026-05-31` (D-140). Flat TOML layout preserved (D-142).
- **AUDIT-01 README integration:** `README.md` §Security Model gained a new paragraph `**External audit charter (v1.5):**` between the existing Supply-chain hygiene paragraph (line 300) and the Test infrastructure (v1.3 Phase 9) paragraph (now line 304). Follows the established `**Category (vN.x):** prose` convention used by 5 existing hardening rollups in the same section.
- **Closed-loop anchor stability:** the 4-way artifact navigation is one-click in both directions:
  - `coordinator/src/blind/rsa.rs` D-07 comment → `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` (resolves to §5 H2 heading)
  - `.cargo/audit.toml` RUSTSEC-2023-0071 closing line → `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` (resolves to same §5 H2)
  - `.cargo/audit.toml` RUSTSEC-2025-0141 + RUSTSEC-2024-0436 closing lines → `docs/AUDIT-CHARTER.md#residual-risks-cargo-audit-advisories` (resolves to §7 H3 "Residual Risks: cargo-audit Advisories")
  - `README.md` §Security Model callout → `[docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md)` (resolves to the file)
  - Charter §1 in-scope table → 12+ file:symbol anchors all greppable in the codebase (verified by Task 3 Edit C)
  - Charter §3 cross-shape table → 9 test fn names all greppable in `shared/tests/bip322_cross_shape.rs` (verified)
- **D-133a atomic landing:** all 3 files landed in commit `92ae533` as ONE atomic git commit — prevented the anchor-drift window that would have existed if audit.toml had been committed before the charter.

## Anchor-stability sweep results (Task 3 Edit C)

Every cross-artifact anchor type resolved with 0 misses:

| Check | Items | Result |
| --- | --- | --- |
| audit.toml + rsa.rs `#anchor` refs → charter H2/H3 headings | 2 distinct anchors (`rsa-secret-key-zeroization-window`, `residual-risks-cargo-audit-advisories`) | ✅ both resolve via GitHub markdown auto-slug |
| Charter §1/§2/§4/§5 file:symbol anchors → codebase symbols | 12 named symbols | ✅ all 12 grep cleanly (incl. post-21-01 `RoundSecretKey`) |
| Charter §3 cross-shape test names → `shared/tests/bip322_cross_shape.rs` | 9 test fn names | ✅ all 9 grep cleanly (`fn reject_p2*_spk_with_*_witness`) |
| README markdown link → `docs/AUDIT-CHARTER.md` | 1 link | ✅ file exists, well-formed markdown syntax |
| Insertion placement: callout between Supply-chain and Test infrastructure | sed range check | ✅ callout falls inside the expected paragraph window |

## Cross-phase invariant matrix

| # | Invariant | Command | Result |
| - | --- | --- | --- |
| 1 | cargo audit | `cargo audit` | 0 vulnerabilities, 0 warnings (exit 0; advisory DB 1099 advisories) |
| 2 | v1.3 P2WPKH full_round | `cargo test --test integration full_round` | 8 passed, 0 failed, 0 ignored (~44.27s) |
| 3 | v1.4 multi-script | `cargo test --test integration mixed_script_e2e` | 1 passed, 0 failed (~2.75s) |
| 4 | Clippy `-D warnings` | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings (exit 0) |
| 5 | V1.4-CRIT-01 scope discipline | `git diff --name-only HEAD~1 HEAD -- coordinator/src/ shared/ client/` | empty (Phase 21 plan 02 modifies only `docs/` + `.cargo/` + `README.md`) |
| 6 | Atomic landing (D-133a) | `git show --stat 92ae533 -- docs/AUDIT-CHARTER.md .cargo/audit.toml README.md` | 3 files changed, 593 insertions, 7 deletions — single commit |

## Task Commits

This plan ships in ONE atomic commit per D-133a (charter + audit.toml + README together — prevents anchor-drift window). There are no per-task commits:

1. **Tasks 1 + 2 + 3 landed atomically:** `92ae533` (docs)
   - `docs/AUDIT-CHARTER.md` (Task 1) — NEW, 574 lines
   - `.cargo/audit.toml` (Task 2) — refreshed
   - `README.md` (Task 3) — callout inserted

**Plan metadata commit:** will follow this SUMMARY.

## Files Created/Modified

- **`docs/AUDIT-CHARTER.md` (NEW, 574 LOC)** — 8 H2 + 7 H3 sections + 4 tables + 4 narrative subsections. The §1 in-scope table cites 15 file:symbol anchors; §3 cross-shape table cites 9 test fn names; §6 out-of-scope table lists 7 components with rationale; §8 glossary maps 30 project terms to plain audit language.
- **`.cargo/audit.toml` (refreshed, 41 → 53 LOC)** — 3 closing `See docs/AUDIT-CHARTER.md#<anchor>` lines appended; RUSTSEC-2023-0071 rationale paragraph rewritten to name AUDIT-03 bounded-window mitigation by name (replaces the previous "destroys the key via `zeroize`" phrasing); `Reviewed:` date bumped to 2026-05-31; 3 ignore IDs preserved verbatim; flat TOML layout preserved.
- **`README.md` (1 paragraph inserted)** — `**External audit charter (v1.5):**` paragraph between the Supply-chain hygiene paragraph (line 300) and the Test infrastructure (v1.3 Phase 9) paragraph (now line 304).

## Decisions Made

- **D-133a (atomic landing):** 3 files shipped in a single commit `92ae533` — prevents the anchor-drift window where audit.toml references charter headings that don't yet exist on the branch.
- **D-134 (hybrid voice):** tables for §1/§3/§6/§8 (enumerable facts an auditor scans); narrative for §2/§4/§5/§7 (threats + dispositions an auditor reads end-to-end).
- **D-135 (§6 extended scope):** out-of-scope table extends beyond REQUIREMENTS' Tor+PKARR baseline to all 3rd-party crypto crates. 7 rows.
- **D-136 (§7 3 sub-buckets):** residual risks split into (a) cargo-audit advisories — 3 entries; (b) protocol-level — 3 items; (c) operational — 4 items.
- **D-137 (§8 scope):** glossary covers active v1.4/v1.5 identifiers (~30 entries); retired pre-v1.4 IDs point at archives.
- **D-138 (file:symbol anchors):** every code reference in the charter uses file::symbol form; line numbers are parenthetical orientation only.
- **D-139 (bare-path closing-lines):** 3 ignore comment blocks in .cargo/audit.toml end with bare-path anchor refs; no markdown link syntax (TOML comments render nowhere).
- **D-140 (Reviewed: bump):** header date bumped to `2026-05-31` (the actual 21-02 commit date).
- **D-141 (no new advisories):** cargo audit confirmed 0/0 with existing 3 ignores; no new ignore-or-fix decisions needed.
- **D-142 (flat TOML layout):** preserved verbatim; no `[advisories.ignore."..."]` sub-tables.
- **D-143 (README callout placement):** inserted between Supply-chain hygiene and Test infrastructure paragraphs in §Security Model.
- **CD-48 (no new ignores):** 3 existing IDs locked verbatim.
- **CD-49 (anchor slug refinement):** §5 H2 heading is `## RSA Secret Key Zeroization Window` (with space between Secret and Key) so GitHub auto-slugs to `rsa-secret-key-zeroization-window` — matching the anchor cited by rsa.rs + audit.toml. §7 H3 headings use colon form (`### Residual Risks: cargo-audit Advisories` etc.) so the slug is clean `residual-risks-cargo-audit-advisories` without a double-hyphen. CD-49 explicitly grants this discretion.
- **CD-51 (§4 length):** 5 paragraphs (within the CONTEXT 4-8 range).
- **CD-52 (README line confirmation):** Supply-chain hygiene paragraph still at line 300 in the live README, matching 21-RESEARCH §11; insertion landed cleanly between line 300 and (then-)line 302.

## Deviations from Plan

[Rule 1 — slug compatibility correction] The plan's Task 1 verify command and acceptance criteria both specified the heading `## RSA SecretKey Zeroization Window` AND the slug `#rsa-secret-key-zeroization-window`. GitHub's markdown auto-slugger does NOT split CamelCase (`SecretKey` → `secretkey`, not `secret-key`), so those two specs are internally inconsistent — the slug `#rsa-secret-key-zeroization-window` is generated by the heading `## RSA Secret Key Zeroization Window` (with a space). I changed the heading text to `## RSA Secret Key Zeroization Window` per CD-49's explicit grant of slug-refinement discretion. The substantive contract (audit.toml + rsa.rs anchor refs resolve to a real charter heading) is met. The §1 in-scope table column header text and §5 narrative use the same updated wording for consistency.

Similarly, the plan's `### Residual Risks — cargo-audit Advisories` heading uses em-dash spacing that GitHub slugs to `residual-risks--cargo-audit-advisories` (double-hyphen), not `residual-risks-cargo-audit-advisories` (single-hyphen) that audit.toml references. I used colon form `### Residual Risks: cargo-audit Advisories` so the slug is clean. Same CD-49 justification.

**Total deviations:** 1 (slug compatibility correction across 4 heading sites — §5 H2 + 3 §7 H3s — and the corresponding 2 in-narrative cross-references).
**Impact on plan:** None on semantic content; this is a markdown-renderer compatibility fix. All 4 acceptance-criteria source assertions for the §5 anchor + §7 anchor still pass (the audit.toml + rsa.rs references resolve to real charter headings, which is the load-bearing contract).

## Issues Encountered

None during planned work. The GitHub markdown auto-slugger inconsistency between CamelCase and hyphenated headings (described in the Deviation above) was caught by the executor at Task 1 verification time — the slug check would have failed silently if not addressed before the atomic commit landed. CD-49 anticipates this exact class of executor-discretion issue.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **Phase 21 v1.5 audit-readiness milestone is COMPLETE.** Both AUDIT-01 (charter) and AUDIT-02 (audit.toml refresh + charter anchor) close in this plan; AUDIT-03 (RoundSecretKey + bounded lifetime) closed at 21-01. The 3 requirements together close v1.5 Phase 21.
- **For the v1.5 milestone close** (`/gsd-complete-milestone` or equivalent): the SC#1 + SC#2 + SC#5 success criteria for Phase 21 are all green. SC#3 + SC#4 closed at 21-01.
- **For any future audit engagement:** the auditor reads `docs/AUDIT-CHARTER.md` cold and is one click from `.cargo/audit.toml` (via the 3 closing anchor lines), one click from the codebase (via 15 file:symbol anchors in §1), and one click from the residual-risk register (via §7 + audit.toml's per-ignore prose). The 4-way navigation loop is intact.
- **For v1.6+ Phase 22+:** any future code change near in-scope symbols should preserve the file:symbol anchors named in charter §1, or update the charter atomically with the rename (same D-133a discipline). The `coordinator/src/blind/rsa.rs::RoundSecretKey` + `coordinator/src/round/state.rs::RoundStateInner.rsa_signer: Option<RsaBlindSigner>` structural pair is load-bearing for the AUDIT-03 mitigation narrative and should be preserved.

## Self-Check: PASSED

- `docs/AUDIT-CHARTER.md` exists at 574 lines with all 8 H2 sections in the AUDIT-01 mandated order — verified.
- `.cargo/audit.toml` has 3 charter-anchor closing lines, AUDIT-03-named RUSTSEC-2023-0071 rationale, `Reviewed: 2026-05-31.`, and 3 preserved ignore IDs — verified.
- `README.md` §Security Model contains the new `**External audit charter (v1.5):**` paragraph between Supply-chain hygiene and Test infrastructure — verified by `sed -n '/Supply-chain hygiene/,/Test infrastructure (v1.3 Phase 9)/p' README.md | grep -q "External audit charter (v1.5)"`.
- Commit `92ae533` found in `git log` containing exactly the 3 files (`docs/AUDIT-CHARTER.md`, `.cargo/audit.toml`, `README.md`) — verified by `git show --stat 92ae533`.
- All 12 charter §1 file:symbol anchors resolve via grep in the codebase — verified.
- All 9 charter §3 cross-shape test names resolve in `shared/tests/bip322_cross_shape.rs` — verified.
- All cross-artifact `#anchor` refs (in audit.toml + rsa.rs D-07 comment) resolve to real charter H2/H3 headings via GitHub markdown auto-slug rules — verified.
- `.planning/phases/21-audit-charter-zeroization-tightening/21-02-SUMMARY.md` exists — verified.
- Cross-phase invariants green: cargo audit 0/0, full_round 8/8, mixed_script_e2e 1/1, clippy 0 warnings, V1.4-CRIT-01 scope discipline preserved — verified.

---
*Phase: 21-audit-charter-zeroization-tightening*
*Completed: 2026-05-31*
