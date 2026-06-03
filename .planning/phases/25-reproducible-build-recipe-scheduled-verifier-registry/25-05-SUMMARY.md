---
phase: 25-reproducible-build-recipe-scheduled-verifier-registry
plan: 05
subsystem: docs+security
tags: [docs, releasing, security-md, registry-submission, rehearsal-procedure, draft-cleanup, milestone-close]
requires:
  - .github/workflows/reproducible-verify.yml (Plan 25-04 — workflow_dispatch entry referenced by rehearsal)
  - docs/REPRODUCIBLE-BUILD.md (Plan 25-03 — Recipe + tables substituted by rehearsal)
  - docs/REPRODUCIBLE-BUILD.expected-sha256.txt (Plan 25-03 — placeholders cited verbatim per BLOCKER 2 fix)
  - .github/workflows/release.yml (Plan 25-02 — draft: true removal made --draft=false docs cleanup safe)
provides:
  - "docs/RELEASING.md ## Reproducibility verification rehearsal (v1.6.0-rc.0 procedure)"
  - "docs/RELEASING.md ## Reproducible-builds.org registry submission"
  - "SECURITY.md ### Reproducibility (v1.6 onward) subsection"
  - "SECURITY.md Known-gaps strikethrough for No reproducible-build pipeline (Phase 25 closure pointer)"
  - "SECURITY.md v1.6 supply-chain plan strikethroughs for cosign image attestations (Phase 23), Detached signatures (Phase 24), Reproducible-build instructions (Phase 25)"
  - "D-13 docs-side cleanup: 0 occurrences of --draft=false; 0 occurrences of flip-out-of-draft prose; Pre-flight header renamed"
affects:
  - "REPRO-04 — procedurally closed; actual reproducible-builds.org submission is a maintainer action conditional on ≥1 green monthly cycle post-v1.6.0 (out of Phase 25 scope per D-10/D-14 procedural-only contract)"
  - "v1.6 milestone — all 4 supply-chain plan items now strikethrough; SECURITY.md presents a coherent post-milestone picture"
tech-stack:
  added: []
  patterns:
    - "sibling-subsection insertion in SECURITY.md (mirrors L182-230 Release tarball signatures subsection — 1 paragraph + 1 fenced quick-reference block + Note blockquote)"
    - "strikethrough closure pointer for closed gaps + shipped items (mirrors existing L104-105 + L284 patterns)"
    - "context-aware sed for placeholder substitution (anchor on row prefix to avoid global-sed accidents — BLOCKER 2 fix)"
key-files:
  created:
    - .planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-05-SUMMARY.md
  modified:
    - docs/RELEASING.md (65 → 123 lines; +71/-13)
    - SECURITY.md (335 → 359 lines; +44/-20)
decisions:
  - "Restructured docs/RELEASING.md as a single coherent write rather than per-line surgical edits — the D-13 cleanup touches lines 5, 10, 18, 27, 29-35, 37, 39, 65 and the appended sections add ~58 lines; a full rewrite was cleaner than 8+ Edit calls"
  - "Updated all three v1.6-supply-chain-plan bullets (not just the third) to strikethrough — the Known-gaps section above already credits Phase 23 closure of Docker image signing and Phase 24 closure of release tarball signing; leaving the plan-section bullets unstruck would have contradicted the Known-gaps strikethroughs in the same file (Rule 2 in-scope consistency cleanup; the plan's success criterion #4 explicitly anticipates this)"
  - "Updated the v1.6-supply-chain-plan section lede + trailing paragraph to reflect milestone close — leaving 'The next milestone is expected to close the unsigned-build gap:' as the lede above 4 strikethrough bullets would have been internally contradictory (Rule 2)"
metrics:
  duration: "~25 minutes"
  completed: "2026-06-02"
---

# Phase 25 Plan 05: Documentation surface for D-10 rehearsal + D-13 cleanup + D-14 registry + D-19 SECURITY.md cross-link Summary

Closes the v1.6 reproducibility milestone with the docs-and-procedure surface
that REPRO-04 names. Removes the orphan `--draft=false` flow from `docs/RELEASING.md`
(D-13 — the YAML side landed in Plan 25-02); adds the v1.6.0-rc.0 rehearsal
procedure that the maintainer runs to capture the expected sha256 + ImageVersion
(D-10 — 5 steps, FOUR placeholder-substitution sites per BLOCKER 2 fix); adds
the manual reproducible-builds.org registry-submission procedure (D-14 — 4 steps,
conditional on ≥1 green monthly cycle post-v1.6.0); inserts the
`### Reproducibility (v1.6 onward)` subsection into `SECURITY.md` §Supply-chain
status (D-19); and updates the Known-gaps + v1.6 supply-chain plan bullets to
strikethrough closure / shipped pointers.

## Tasks

### Task 1: Restructure docs/RELEASING.md for D-13 cleanup + append D-10 rehearsal + D-14 registry
- **Commit:** `f63ee43`
- **Status:** ✅ Done
- **File:** `docs/RELEASING.md` (65 → 123 lines)
- **Verification:** All 20 grep assertions pass (no `--draft=false`, no flip-out-of-draft prose, both new H2 sections present with required citations to `workflow_dispatch`, `<TBD-*>` placeholders, `awk -F:`, `reproducible-builds.org`, `_data/projects/`, cosign verify-blob/verify-attestation preserved, line count ≥100)

### Task 2: SECURITY.md ### Reproducibility (v1.6 onward) subsection + strikethrough closure pointers
- **Commit:** `46ba81e`
- **Status:** ✅ Done
- **File:** `SECURITY.md` (335 → 359 lines)
- **Verification:** All 15 grep assertions pass (new H3 subsection present + cross-links + bash block contract + Note blockquote, both strikethrough pointers present, no `ubuntu-latest` anywhere)

## Files Changed

| File | Pre | Post | Δ | Notes |
| --- | --- | --- | --- | --- |
| `docs/RELEASING.md` | 65 | 123 | +58 | D-13 cleanup of 4 `--draft=false` + flip-out-of-draft prose; new H2 sections at L63 (rehearsal) and L111 (registry submission); Pre-flight header renamed at L33 |
| `SECURITY.md` | 335 | 359 | +24 | New `### Reproducibility (v1.6 onward)` subsection at L228-260; strikethrough at L106 (No reproducible-build pipeline → Phase 25 closure); strikethrough at L305-307 (all three remaining v1.6 plan bullets → Phase 23/24/25 shipped pointers); lede + trailing paragraph at L301-313 updated to reflect milestone close |

## Line Ranges of New Sections

**docs/RELEASING.md new H2 sections:**
- **L63-109:** `## Reproducibility verification rehearsal (v1.6.0-rc.0 procedure)` — 5 numbered steps with all FOUR per-site sed substitutions documented (BLOCKER 2 fix); cites `<TBD-v1.6.0-cut-sha256>`, `<TBD-v1.6.0-cut-imageversion>`, and `<TBD-v1.6.0-cut>` (markdown-table placeholder) explicitly; documents the `awk -F:` verifier-lookup mechanism; closing cross-link to the registry-submission section
- **L111-123:** `## Reproducible-builds.org registry submission` — 4 numbered steps (verify green-monthly-cycle, fork-and-add-entry, open-PR, link-back-from-blindjoin); closing line "REPRO-04 is closed once the registry PR is merged AND both blindjoin-side cross-links are updated"

**SECURITY.md new H3 subsection:**
- **L228-260:** `### Reproducibility (v1.6 onward)` — 1 paragraph + 1 fenced quick-reference bash block + 1 `> **Note: Rust reproducibility long tail**` blockquote; cross-links to `docs/REPRODUCIBLE-BUILD.md`, `.github/workflows/reproducible-verify.yml`, and placeholder for the reproducible-builds.org registry URL (to be filled in after D-14 lands)

## D-13 Cleanup Completeness

- ✅ `! grep -- '--draft=false' docs/RELEASING.md` returns zero matches (was 4 matches at lines 5, 10, 29, 31, 34)
- ✅ `! grep -qi 'flipping the github release out of draft' docs/RELEASING.md` returns zero matches
- ✅ `! grep -qi 'flip the release out of draft' docs/RELEASING.md` returns zero matches
- ✅ Pre-flight section header renamed: `## Pre-flight check before flipping out of draft` → `## Pre-flight check after CI completes`
- ✅ Old step 4 (`gh release edit vX.Y.Z --draft=false` fenced block) replaced with single-step "Verify the published release" prose per RESEARCH Example 7
- ✅ Cosign verify-blob and verify-attestation commands in Pre-flight section preserved verbatim
- ✅ Closing sentence (L65) reworded: "DO NOT flip the release out of draft. Recovery: …" → "recover via `gh release delete vX.Y.Z`, fix the underlying issue, and re-push the tag."

## Strikethrough Pointer Lines

**SECURITY.md Known-gaps bullet (replaces 5-line bullet at original L106-110):**
```markdown
- **~~No reproducible-build pipeline.~~** **Closed in v1.6 Phase 25** — see [Reproducibility (v1.6 onward)](#reproducibility-v16-onward).
```

**SECURITY.md v1.6 supply-chain plan bullets (replaces L275-283 originals):**
```markdown
- **~~cosign image attestations~~** ✓ Shipped in Phase 23 — see [Image signatures + attestations (v1.6 onward)](#image-signatures--attestations-v16-onward).
- **~~Detached signatures on GitHub Release archives~~** ✓ Shipped in Phase 24 (cosign blob signatures + SLSA provenance; PGP path deferred indefinitely 2026-06-02) — see [Release tarball signatures + provenance (v1.6 onward)](#release-tarball-signatures--provenance-v16-onward).
- **~~Reproducible-build instructions~~** ✓ Shipped in Phase 25 — see [Reproducibility (v1.6 onward)](#reproducibility-v16-onward).
- **~~Automated base-image digest drift check~~** ✓ Shipped in Phase 22 — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).
```

All four v1.6 supply-chain plan items now show strikethrough + shipped-status pointers. (Plan 25-05's success criterion #4 explicitly anticipated this: "if Phase 23 cosign image attestations are also marked shipped if not already — re-read the current state and update consistently to avoid inconsistency". Pre-edit state had 1/4 struck; post-edit state has 4/4 struck.)

## Deviations from Plan

### Rule 2 - Section consistency cleanup (in scope)

**1. [Rule 2 - Missing consistency] Updated v1.6 supply-chain plan lede + trailing paragraph to reflect milestone close**
- **Found during:** Task 2, while applying the third-bullet strikethrough
- **Issue:** The plan's instruction was to strikethrough only the third bullet (Reproducible-build instructions). I extended that to bullets 1 and 2 per the plan's explicit success criterion #4 anticipation. With all four bullets struck, the section's lede ("The next milestone is expected to close the unsigned-build gap:") and trailing paragraph ("Until those land, **treat the SHA-256 checksum…**") became internally contradictory.
- **Fix:** Replaced the lede with "The v1.6 milestone has closed all four planned supply-chain items:" and replaced the trailing paragraph with a positive-framed pointer to the new reproducibility recipe + per-tag expected hash table.
- **Files modified:** `SECURITY.md` L301-313 (within the same commit as Task 2)
- **Commit:** `46ba81e`

### Auth Gates
None encountered.

### Architectural Changes (Rule 4)
None required.

## Known Stubs

| Stub | File | Line | Reason |
| --- | --- | --- | --- |
| `<added after blindjoin's submission lands; see [docs/RELEASING.md](docs/RELEASING.md) §Reproducible-buildsorg registry submission>` | SECURITY.md | ~L237 | D-14 procedure step 4 substitutes this placeholder after the registry PR merges. Tracked: the SECURITY.md prose explicitly cites this as the slot to fill — operator confusion risk is zero (cross-link points to the procedure that resolves it). |
| `<TBD-v1.6.0-cut-sha256>`, `<TBD-v1.6.0-cut-imageversion>` | docs/REPRODUCIBLE-BUILD.expected-sha256.txt | L15 | Pre-existing from Plan 25-03. The rehearsal procedure in docs/RELEASING.md L63-109 IS the documented mechanism for substituting these. Two-stage bootstrap was the locked D-10 design decision. |
| `<TBD-v1.6.0-cut>` (×2) | docs/REPRODUCIBLE-BUILD.md | §Expected sha256sum table, §Toolchain pins ubuntu-24.04 row | Same as above — pre-existing placeholders from Plan 25-03; resolved by the same rehearsal procedure (placeholder sites iii and iv). |

These are not implementation stubs but documented two-stage bootstrap placeholders per D-10. The bootstrap is the locked design — Phase 25 ships the documentation; the maintainer's rehearsal at v1.6.0-rc.0 cut fills them.

## Milestone Close Notes

**REPRO-04 is procedurally complete.** Phase 25 ships the documented maintainer
procedure for reproducible-builds.org submission (docs/RELEASING.md L111-123).
The actual submission is a maintainer action conditional on ≥1 green monthly
`reproducible-verify.yml` cycle post-v1.6.0 — outside Phase 25 scope per the
D-10/D-14 procedural-only contract (mirrors Phase 24's PGP-key-generation pattern).

**Next maintainer action (out of Phase 25 scope) at v1.6.0-rc.0 cut:**
1. Trigger `.github/workflows/reproducible-verify.yml` via `workflow_dispatch` from the Actions tab
2. Capture `Verifier image: <V>` from "Capture runner ImageVersion" step logs
3. Capture `Actual (rebuilt locally): <SHA>` from "Compare sha256 + classify result" step logs (the first rehearsal goes RED against `<TBD-*>`; this is expected)
4. Substitute FOUR placeholder sites per BLOCKER 2 fix:
   - `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` `<TBD-v1.6.0-cut-sha256>` → captured sha256
   - `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` `<TBD-v1.6.0-cut-imageversion>` → captured ImageVersion
   - `docs/REPRODUCIBLE-BUILD.md` §Expected sha256sum table row → captured sha256 (context-anchored sed)
   - `docs/REPRODUCIBLE-BUILD.md` §Toolchain pins ubuntu-24.04 row → captured ImageVersion (context-anchored sed)
5. Commit both files together with message `docs(25): capture v1.6.0-rc.0 reproducibility baseline (ImageVersion=<X>, sha256=<Y>)`
6. Tag v1.6.0 + push; first scheduled `0 7 1 * *` run on the next month will go GREEN

**Phase 25 surface complete.** All 4 REPRO requirements have plan coverage:
- REPRO-01: docs/REPRODUCIBLE-BUILD.md shipped (Plan 25-03)
- REPRO-02: release.yml + rust-toolchain.toml + Cargo.toml shipped (Plan 25-01)
- REPRO-03: .github/workflows/reproducible-verify.yml shipped (Plan 25-04)
- REPRO-04: maintainer procedure shipped (Plan 25-05) — registry PR is the maintainer's next action

## Self-Check: PASSED

Files verified to exist:
- ✅ `docs/RELEASING.md` (123 lines)
- ✅ `SECURITY.md` (359 lines)
- ✅ `.planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-05-SUMMARY.md` (this file)

Commits verified in git log:
- ✅ `f63ee43` — docs(25-05): clean up RELEASING.md for D-13 + add rehearsal + registry sections
- ✅ `46ba81e` — docs(25-05): add SECURITY.md ### Reproducibility (v1.6 onward) subsection
