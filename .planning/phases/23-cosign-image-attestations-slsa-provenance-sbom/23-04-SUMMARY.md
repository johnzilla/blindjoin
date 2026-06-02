---
phase: 23-cosign-image-attestations-slsa-provenance-sbom
plan: 04
subsystem: infra
tags: [cosign, sigstore, slsa, spdx, sbom, security, supply-chain, documentation]

requires:
  - phase: 23-cosign-image-attestations-slsa-provenance-sbom
    provides: locked identity regex and cosign/gh CLI recipes (Plans 23-01/02/03 produce the artifacts these recipes verify)

provides:
  - SECURITY.md ### Image signatures + attestations (v1.6 onward) subsection with 4 operator-facing verify recipes
  - Strikethrough closure of v1.5 Docker-unsigned gap with cross-link to new subsection
  - Pitfall 10 (GHCR UI badge) and Pitfall 13 (cosign 3.0 CLI drift) callouts
  - Operator-side cosign version pin >= 2.6.3, < 3.0.0

affects:
  - 23-05 (Plan 23-05 Stage 2 HUMAN-UAT runs every recipe in this section empirically against v1.6.0-rc.0)
  - 24 (Phase 24 appends tarball-signing recipe block in the same SECURITY.md section structure)

tech-stack:
  added: []
  patterns:
    - "RESEARCH-locked recipes block copied verbatim into SECURITY.md (recipe correctness invariant)"
    - "Single-line strikethrough bold marker pattern for v1.5 gap closure (Phase 22 established, Phase 23 repeats)"
    - "cosign save --dir for offline-verifiable bundle (ATTEST-04 — corrects CONTEXT D-07's nonexistent cosign download signature --bundle)"

key-files:
  created: []
  modified:
    - SECURITY.md

key-decisions:
  - "ATTEST-04 recipe uses cosign save --dir (not cosign download signature --bundle) — per RESEARCH §3.4 the --bundle flag on cosign download signature does not exist; cosign save produces a directory with all sig/cert/Rekor artifacts suitable for cosign verify --local-image"
  - "Operator-side cosign version pin is >= 2.6.3, < 3.0.0 (D-08 range form) matching CI side Plan 23-01 cosign-release: v2.6.3"
  - "Forward-strikethrough on tarball-signature bullet NOT added — Phase 24 owns that anchor per RESEARCH §8 Q3"
  - "Single-line strikethrough for Docker-unsigned bullet — Phase 22 Plan 22-05 lesson: literal-byte grep form wins over multi-line prose wrapping"

patterns-established:
  - "Operator-facing supply-chain subsection shape: H3 (v1.6 onward) + 3-item numbered list + prerequisite tooling paragraph + fenced bash recipes block + > Note: callouts"

requirements-completed: [ATTEST-01, ATTEST-02, ATTEST-03, ATTEST-04]

duration: ~15min
completed: 2026-06-02
---

# Phase 23 Plan 04: SECURITY.md Image Signatures + Attestations Subsection Summary

**Operator-facing cosign verify + gh attestation verify + cosign save recipes (ATTEST-01/02/03/04) added to SECURITY.md with locked docker.yml@refs/tags/v.* identity regex, cosign >= 2.6.3 version pin, and v1.5 Docker-unsigned gap strikethrough.**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-06-02T02:25:00Z
- **Completed:** 2026-06-02T02:40:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added `### Image signatures + attestations (v1.6 onward)` subsection to `SECURITY.md` between `### Known gaps at v1.5` and `### Base-image digests (v1.6 onward)` (lines 118–185)
- Subsection contains 4 recipes: cosign verify (ATTEST-01), gh attestation verify SLSA (ATTEST-02), gh attestation verify SPDX (ATTEST-03), cosign save + cosign verify --local-image (ATTEST-04)
- Locked identity regex `'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*'` (Pitfall 1 narrow form — covers all three matrix images per RESEARCH §7.6)
- Two `> Note:` callouts: Pitfall 10 (GHCR UI badge unrelated to cosign verification) and Pitfall 13 (cosign >= 2.6.3, < 3.0.0 version pin)
- Strikethrough'd the multi-line v1.5 Docker-unsigned bullet to a single-line cross-link form
- Phase 22 `### Base-image digests (v1.6 onward)` subsection preserved byte-identical

## Task Commits

1. **Task 1: Insert Image signatures + attestations subsection + strikethrough v1.5 Docker bullet** — `6e6c776` (docs)

**Plan metadata:** (included in final docs commit below)

## Files Created/Modified

- `SECURITY.md` — New `### Image signatures + attestations (v1.6 onward)` subsection at lines 118–185 (70 insertions, 5 deletions; file grew from 225 to 290 lines)

## New Subsection Line Numbers

- **Before edit:** new subsection did not exist; Docker-unsigned bullet was lines 110–114 (multi-line)
- **After edit:** new subsection at lines 118–185; Docker-unsigned bullet collapsed to single line 110; `### Base-image digests (v1.6 onward)` now at line 187 (was line 122)

## Literal Regex String (for Plan 23-05 Stage 2 UAT cross-reference)

```
'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*'
```

This is the OIDC subject identity regexp used in both Recipe 1 (cosign verify) and Recipe 4 (cosign verify --local-image). Plan 23-05 Stage 2 UAT must run this exact string against the v1.6.0-rc.0 tag from a fresh ubuntu:24.04 container and confirm exit 0.

## Verified Anchor

The strikethrough cross-link uses `#image-signatures--attestations-v16-onward`, which is the auto-generated GitHub anchor for `### Image signatures + attestations (v1.6 onward)`:
- GitHub anchor algorithm: lowercase, spaces → hyphens, `+` dropped (leaving two hyphens around the dropped character)
- `Image signatures` → `image-signatures`
- ` + ` → `--` (space-hyphen for space, plus dropped, space-hyphen for space)
- `attestations` → `attestations`
- ` (v1.6 onward)` → `-v16-onward`
- Result: `#image-signatures--attestations-v16-onward`

## Phase 22 Base-image-digests Subsection Preservation

The `### Base-image digests (v1.6 onward)` subsection (previously lines 122–159, now lines 187–224) was not touched. All content from `blindjoin's \`docker/Dockerfile\` derives from two upstream base images` through the `digest-drift-check.yml` idempotency paragraph is byte-identical to pre-edit state. Verified by: `grep -A 35 '^### Base-image digests (v1.6 onward)$' SECURITY.md | grep -q 'digest-drift-check.yml'` — PASS.

## Cross-reference to Plan 23-05

Plan 23-05 Stage 2 (HUMAN-UAT) empirically validates every recipe in the new subsection from a fresh `ubuntu:24.04` container against `v1.6.0-rc.0`. If Stage 2 reveals a recipe failure, a quick task amends the SECURITY.md section BEFORE the production `v1.6.0` tag (Pitfall 12 gate). The locked regex `'docker\.yml@refs/tags/v.*'` deliberately spans pre-release tags (v1.6.0-rc.0 matches), so Stage 2 provides a full end-to-end recipe rehearsal.

## Deviations from Plan

None — plan executed exactly as written. The RESEARCH §3.4 correction (cosign save --dir instead of cosign download signature --bundle) was already incorporated into the plan's task action; no deviation handling was needed.

## Issues Encountered

None.

## User Setup Required

None — documentation-only change.

## Next Phase Readiness

- SECURITY.md operator-facing recipes are complete; ROADMAP SC#5 satisfied
- Plan 23-05 Stage 2 UAT will empirically run each recipe against v1.6.0-rc.0
- Phase 24 can append a tarball-signing recipe block in the same `## Supply-chain status` structure without restructuring

---
*Phase: 23-cosign-image-attestations-slsa-provenance-sbom*
*Completed: 2026-06-02*
