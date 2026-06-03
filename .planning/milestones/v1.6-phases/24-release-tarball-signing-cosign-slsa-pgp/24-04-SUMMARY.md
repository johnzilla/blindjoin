---
phase: 24-release-tarball-signing-cosign-slsa-pgp
plan: 04
subsystem: docs-supply-chain
tags: [docs, contributing, cross-reference, releasing, audience-disambiguation, sign-03-procedural]
requires:
  - CONTRIBUTING.md (existing — pre-edit 141 lines, `## Tagging releases` section at lines 69-94)
  - docs/RELEASING.md (created by Plan 24-02, commit 1eea5af — the cross-ref target)
provides:
  - CONTRIBUTING.md one-paragraph cross-reference at the end of `## Tagging releases` section (D-11 + D-20)
  - Audience-disambiguation lede gating non-maintainer contributors away from release-engineering procedures
  - Contributor-manual entry point into docs/RELEASING.md (without it, the maintainer-side procedure is undiscoverable from CONTRIBUTING.md)
affects:
  - CONTRIBUTING.md (modified; +2 lines net: blank + one-paragraph; 141 → 143 lines)
tech-stack:
  added: []
  patterns:
    - Audience-disambiguation lede convention reused from docs/RELEASING.md (Plan 24-02) — "Most contributors don't need it; it's the release-engineering manual for the maintainer."
    - Single-physical-source-line cross-ref paragraph (Phase 22 Plan 22-05 single-line-literal-for-grep lesson) — paragraph fits on one source line in CONTRIBUTING.md so file-level grep audits match across the entire paragraph
    - Relative markdown link form `[`docs/RELEASING.md`](docs/RELEASING.md)` (no leading slash, no dot-slash) mirrors CONTRIBUTING.md existing link conventions at lines 73 + 75
    - Location-anchored cross-ref at the end of `## Tagging releases` (natural insertion point per D-20)
key-files:
  created: []
  modified:
    - CONTRIBUTING.md
decisions:
  - "Insertion point at end of `## Tagging releases` section (between existing milestone-name-vs-tag paragraph and `## Bumping base-image digests` H2) — natural location-anchor for D-20 cross-ref placement"
  - "Single physical source line for the cross-ref paragraph — Phase 22 Plan 22-05 lesson preserved: file-level grep audit gates need the full paragraph on one line to match the literal-byte regex"
  - "Audience-disambiguation lede `Most contributors don't need it; it's the release-engineering manual for the maintainer.` is LOAD-BEARING (D-11) — without it, non-maintainer contributors silently follow the cross-ref and start executing procedures requiring YubiKey + offline revocation cert + maintainer-only secrets they don't have"
  - "Relative-link form `docs/RELEASING.md` (no leading slash, no `./`) mirrors CONTRIBUTING.md's existing link conventions at lines 73 (`[.github/workflows/docker.yml](.github/workflows/docker.yml)`) and 75 (`[CHANGELOG.md](CHANGELOG.md)`)"
  - "SIGN-03 stays unchecked in REQUIREMENTS.md — this plan addresses the procedural-surface DISCOVERABILITY piece of SIGN-03 (D-11 + D-20); the load-bearing PGP key artifact piece is Plan 24-05 (maintainer YubiKey ceremony + `docs/pgp/<FINGERPRINT>.asc` commit + `<FINGERPRINT-TBD>` atomic substitution)"
metrics:
  duration_minutes: 1
  duration_seconds: 73
  tasks_completed: 1
  files_modified: 1
  completed: 2026-06-02
---

# Phase 24 Plan 04: CONTRIBUTING.md → docs/RELEASING.md cross-reference Summary

Added a one-paragraph cross-reference from `CONTRIBUTING.md` to `docs/RELEASING.md` at the end of the existing `## Tagging releases` section. The new paragraph names the post-tag maintainer-side procedure (PGP sign on YubiKey, .asc upload, draft flip) and carries an audience-disambiguation lede (`Most contributors don't need it; it's the release-engineering manual for the maintainer.`) that gates non-maintainer contributors away from the cross-ref so they don't follow it into release-engineering procedures they shouldn't execute. This is the contributor-manual entry point into the maintainer-side procedure that Plan 24-02 created.

## What Got Built

### `CONTRIBUTING.md` (modified) — new paragraph at line 96

The new paragraph (verbatim, single physical source line per Phase 22 Plan 22-05 lesson):

> Once `release.yml` finishes, the maintainer-side procedure (download the CI-built tarball, sign it with PGP on a YubiKey, upload the `.asc`, flip the release out of draft) lives in [`docs/RELEASING.md`](docs/RELEASING.md). Most contributors don't need it; it's the release-engineering manual for the maintainer.

**File location in the post-edit file:**
- Line 94 — existing milestone-name-vs-tag paragraph (`The milestone *name* in planning docs (e.g. \`v1.3 Test Infrastructure & Operational Hardening\`) is independent of the git tag — docs may stay \`v1.X\` for readability while the tag is \`v1.X.0\`.`)
- Line 95 — blank
- **Line 96 — NEW cross-reference paragraph** (single physical source line)
- Line 97 — blank
- Line 98 — existing `## Bumping base-image digests` H2

**Pre-edit → post-edit line count:** 141 → 143 (net +2 source lines: blank + paragraph; the trailing blank already existed between the old line 94 and the old line 96 H2 in the pre-edit file). Plan acceptance criterion allowed `144 ±1`; actual is 143, within tolerance.

**Relative-link form used:** `` [`docs/RELEASING.md`](docs/RELEASING.md) `` — no leading slash, no dot-slash. Matches CONTRIBUTING.md's existing link conventions at:
- Line 73 — `` [.github/workflows/docker.yml](.github/workflows/docker.yml) ``
- Line 75 — `[CHANGELOG.md](CHANGELOG.md)`

**Cross-coherence with Plan 24-02:** The audience-disambiguation lede (`Most contributors don't need it; it's the release-engineering manual for the maintainer.`) deliberately mirrors the audience-disambiguation lede that Plan 24-02 wrote into `docs/RELEASING.md` itself (the destination file's own H1 + lede states it's for the maintainer). Two-layer audience message: (a) CONTRIBUTING.md's cross-ref tells contributors NOT to follow the link unless they're cutting a release; (b) docs/RELEASING.md's own opening prose re-states the same audience.

## What Got Verified

All plan-locked acceptance criteria executed and passed:

| Gate | Check | Result |
|------|-------|--------|
| 1 | `test -f CONTRIBUTING.md` | PASS |
| 2 | `grep -q 'Once \`release.yml\` finishes' CONTRIBUTING.md` | PASS |
| 3 | `grep -q '\[\`docs/RELEASING.md\`\](docs/RELEASING.md)' CONTRIBUTING.md` | PASS |
| 4a | `grep -q "Most contributors don't need it" CONTRIBUTING.md` | PASS |
| 4b | `grep -q 'release-engineering manual for the maintainer' CONTRIBUTING.md` | PASS |
| 5a | `grep -q '^## Tagging releases$' CONTRIBUTING.md` | PASS (preserved) |
| 5b | `grep -q '^## Bumping base-image digests$' CONTRIBUTING.md` | PASS (preserved) |
| 6 | `awk` insertion-order gate (Tagging releases NR < cross-ref NR < Bumping base-image digests NR) | PASS |
| 7 | `! grep -q '\[docs/RELEASING\.md\](/docs/RELEASING\.md)'` (no abs-path form) | PASS |
| 8 | `! grep -q '\[docs/RELEASING\.md\](\./docs/RELEASING\.md)'` (no dot-slash form) | PASS |
| 9 | `grep -E 'Once \`release\.yml\` finishes.*maintainer-side procedure.*docs/RELEASING\.md.*Most contributors don.t need it.*release-engineering manual'` (single-physical-line property) | PASS |
| 10 | File length grew by 3 (±1) source lines | PASS (141 → 143; within ±1 tolerance) |
| 11a | `grep -A 14 '^## Tagging releases$' CONTRIBUTING.md \| grep -q 'git tag -a v1.X.0'` (bash example preserved) | PASS |
| 11b | `grep -A 25 '^## Tagging releases$' CONTRIBUTING.md \| grep -q 'See .planning/MILESTONES.md'` (bash example tail preserved) | PASS |
| 12 | `grep -q 'The milestone \*name\* in planning docs' CONTRIBUTING.md` (milestone-name-vs-tag paragraph preserved) | PASS |
| 13 | `## Running integration tests` + `## Interpreting output` + `## Tagging releases` + `## Bumping base-image digests` all present (no other section displaced) | PASS |

All gates green; no Rule 1/2/3 auto-fixes needed; no checkpoints triggered.

## Deviations from Plan

None — plan executed exactly as written. The single-paragraph insertion at the planner-specified location matched the file structure precisely; no edits to other parts of CONTRIBUTING.md.

(Note: the plan's `wc -l` acceptance criterion expected 144 with `±1` tolerance acknowledging trailing-newline editor convention differences. Actual post-edit count is 143 — the pre-edit file already had a blank line between the milestone-name paragraph (line 94) and the `## Bumping base-image digests` H2 (line 96), so the Edit added: blank-line-already-present + new paragraph + new blank line + H2-already-present = net +2 visible source lines. The 143 is within the planner's stated ±1 tolerance.)

## Known Stubs

None — this plan inserts a complete, self-contained one-paragraph cross-reference. The link target (`docs/RELEASING.md`) is already present in the repo (created by Plan 24-02, commit 1eea5af); no broken-link risk.

## Threat Surface

This plan modifies one documentation file (CONTRIBUTING.md) — no new network endpoints, auth paths, file-access patterns, or schema changes at trust boundaries. The audience-disambiguation lede (`Most contributors don't need it; it's the release-engineering manual for the maintainer.`) is the trust gate addressed by T-24-31 in the plan's threat model — present as required.

## Commits

| Commit | Subject |
|--------|---------|
| `27f4ca8` | docs(24-04): add cross-ref from CONTRIBUTING.md to docs/RELEASING.md |

## Coherence with Phase 24 Plan Set

- **Plan 24-01** (commits aa5af3f + 13ecde9 + 7e0f97c) wired `release.yml` to produce the cosign `.bundle` + SLSA `.sigstore` artifacts + softprops draft. This plan's cross-ref paragraph references `release.yml` by name and the post-tag flow it kicks off.
- **Plan 24-02** (commit 1eea5af) created `docs/RELEASING.md` — the destination of this plan's cross-reference link.
- **Plan 24-03** (commit a91d4a1) added the operator-facing SECURITY.md `### Release tarball signatures + provenance (v1.6 onward)` subsection — the OTHER audience-disambiguation surface (operators verifying releases) vs. this plan's contributor surface (maintainers cutting releases).
- **Plan 24-05** (pending) — maintainer YubiKey ceremony + `<FINGERPRINT-TBD>` atomic substitution in SECURITY.md + docs/RELEASING.md (no edit to CONTRIBUTING.md required; this plan's cross-ref paragraph does not reference the fingerprint).

## SIGN-03 Coverage Status

This plan addresses the **procedural-surface discoverability** piece of SIGN-03 (D-11 + D-20). Plan 24-02 (docs/RELEASING.md procedural manual) + Plan 24-03 (SECURITY.md operator-facing recipes) + Plan 24-04 (this plan — CONTRIBUTING.md contributor-facing entry point) are the documentation half of SIGN-03; Plan 24-05 (maintainer YubiKey ceremony + `docs/pgp/<FINGERPRINT>.asc` commit) is the load-bearing artifact half. SIGN-03 stays unchecked in REQUIREMENTS.md until Plan 24-05 completes — completing the documentation half without the PGP key artifact would be premature.

## Self-Check: PASSED

- **File exists:** `test -f CONTRIBUTING.md` → FOUND
- **Commit exists:** `git log --oneline | grep -q 27f4ca8` → FOUND
- **Cross-ref paragraph present at line 96 with verbatim text:** verified via Read tool on lines 90-104 (paragraph at line 96, between existing milestone-name paragraph at line 94 and `## Bumping base-image digests` H2 at line 98)
- **All 16 plan-locked acceptance gates passed** (see "What Got Verified" table)
- **No file deletions** in commit `27f4ca8` (verified `git diff --diff-filter=D --name-only HEAD~1 HEAD`)
- **No untracked files** after edit (verified `git status --short`)
