---
phase: 23-cosign-image-attestations-slsa-provenance-sbom
plan: "03"
subsystem: infra
tags: [github-actions, ci, sigstore, cosign, sbom, supply-chain, grep-gate]

# Dependency graph
requires: []
provides:
  - "sigstore-pin-check CI job in ci.yml enforcing 40-hex SHA pins on 4 sigstore-ecosystem actions"
  - "Perl negative-lookahead grep gate: catches @v3/@main/@stable refs across all .github/workflows/"
  - "Inline SECURITY.md + PITFALLS.md citations in gate error message (Plan 22-04 audit-trail pattern)"
affects:
  - "23-01 (docker.yml cosign sign step — must use SHA-pinned sigstore/cosign-installer)"
  - "23-02 (docker.yml attest steps — must use SHA-pinned actions/attest-build-provenance, actions/attest-sbom, anchore/sbom-action)"
  - "24-xx (release.yml — inherits this gate automatically; no additional gate maintenance)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "sigstore-pin-check mirrors bip322-pin-check / crit-01-grep-check shape: independent job, ubuntu-latest, checkout + grep gate"
    - "grep -rnPE with Perl negative-lookahead (?![a-f0-9]{40}) for 40-hex SHA detection"
    - "4-action target list (RESEARCH §2.2 extension): sigstore/cosign-installer, actions/attest-build-provenance, actions/attest-sbom, anchore/sbom-action"
    - "Inline literal SECURITY.md + PITFALLS.md citations in error echo (no POLICY_REF shell var)"

key-files:
  created: []
  modified:
    - ".github/workflows/ci.yml"

key-decisions:
  - "D-04 narrow grep gate: scoped to 4 sigstore-ecosystem actions only, NOT a broad every-uses-must-be-@40-hex gate (would surface pre-existing dtolnay/rust-toolchain@stable etc.)"
  - "D-09 new job in ci.yml (not standalone workflow): mirrors bip322-pin-check family symmetry"
  - "RESEARCH §2.2 extension: anchore/sbom-action added as 4th target because attest-sbom does NOT generate SBOMs internally — Syft is the ATTEST-03 load-bearing generator"
  - "Plan 22-04 lesson upheld: SECURITY.md + PITFALLS.md citations are inline string literals in echo, not POLICY_REF shell variable"

patterns-established:
  - "grep -rnPE with (?![a-f0-9]{40}) negative-lookahead: canonical pattern for SHA-pin enforcement on named action sets"
  - "4-action sigstore target list: stable named set for Phase 24/25 reuse with no gate maintenance"

requirements-completed:
  - ATTEST-01
  - ATTEST-02
  - ATTEST-03

# Metrics
duration: 8min
completed: 2026-06-01
---

# Phase 23 Plan 03: sigstore-pin-check CI Gate Summary

**CI gate enforcing 40-hex SHA pins on 4 sigstore-ecosystem actions via Perl negative-lookahead grep, inlining SECURITY.md + PITFALLS.md citations in the error message per Plan 22-04 audit-trail lesson**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-06-01T00:00:00Z
- **Completed:** 2026-06-01T00:08:00Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Appended `sigstore-pin-check` job to end of `.github/workflows/ci.yml` after `crit-01-client-grep-check`
- Grep gate uses `grep -rnPE` with Perl negative-lookahead `(?![a-f0-9]{40})` to catch @v3/@main/@stable floating tags
- 4-action target list (RESEARCH §2.2 extension): `sigstore/cosign-installer`, `actions/attest-build-provenance`, `actions/attest-sbom`, `anchore/sbom-action`
- Error message names all 4 actions individually and cites `SECURITY.md § Supply-chain status > Image signatures` and `.planning/research/PITFALLS.md §4` as inline literals

## Job Block Appended

```yaml
  sigstore-pin-check:
    name: sigstore + sbom action SHA-pin check
    runs-on: ubuntu-latest
    # v1.6 Phase 23 ATTEST-01/02/03 invariant: the four GitHub Actions that
    # produce the supply-chain attestations (sigstore/cosign-installer,
    # actions/attest-build-provenance, actions/attest-sbom, anchore/sbom-action)
    # MUST be pinned to a 40-hex commit SHA in every workflow under
    # .github/workflows/. Floating tags like @v3 expose the project to silent
    # action substitution — exactly the attack surface this milestone is closing.
    # Pattern mirrors bip322-pin-check (v1.4) and the v1.5 crit-01-grep-check
    # family — narrow, audit-grepable, named after what it enforces.
    # RESEARCH.md §2.2 adds anchore/sbom-action to the target list because Syft
    # is the ATTEST-03 SBOM generator (attest-sbom does NOT generate SBOMs
    # internally as of v2.4.0).
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - name: Enforce SHA pin on sigstore + sbom actions
        run: |
          set -eu
          # Greps for any of the four target actions used WITHOUT a 40-hex SHA
          # ref. Match form: `uses: <owner>/<repo>@<not-40-hex>` (e.g. @v3, @main).
          # Exits 1 with an auditor-facing message naming the offending file:line.
          PATTERN='uses:\s*(sigstore/cosign-installer|actions/attest-build-provenance|actions/attest-sbom|anchore/sbom-action)@(?![a-f0-9]{40})'
          if grep -rnPE "${PATTERN}" .github/workflows/; then
            echo "ERROR: One or more sigstore-ecosystem / sbom-action uses above lacks a 40-hex SHA pin." >&2
            echo "       v1.6 Phase 23 supply-chain invariant: ALL of" >&2
            echo "         - sigstore/cosign-installer" >&2
            echo "         - actions/attest-build-provenance" >&2
            echo "         - actions/attest-sbom" >&2
            echo "         - anchore/sbom-action" >&2
            echo "       MUST be pinned by 40-hex commit SHA in every workflow." >&2
            echo "       See SECURITY.md § Supply-chain status > Image signatures and" >&2
            echo "       .planning/research/PITFALLS.md §4 for rationale." >&2
            exit 1
          fi
```

## Exact PATTERN Regex

```
PATTERN='uses:\s*(sigstore/cosign-installer|actions/attest-build-provenance|actions/attest-sbom|anchore/sbom-action)@(?![a-f0-9]{40})'
```

The negative lookahead `(?![a-f0-9]{40})` matches when the `@ref` is NOT exactly 40 lowercase hex characters. Any floating tag (`@v3`, `@main`, `@stable`, `@v2.4.0`, any short SHA) matches and fails the gate. A full 40-hex commit SHA does not match — gate passes.

## Self-Test Results

**Self-test against current repo state (before Plans 23-01/02 land):**
```
PASS: all SHA-pinned (vacuously)
```
None of the 4 target actions are present in any workflow yet — the gate is vacuously satisfied. After Plans 23-01 and 23-02 land with all 4 actions SHA-pinned, the gate will also pass (non-vacuously).

**Negative-test (synthetic regression — does the gate correctly fail on bad input?):**
```
.github/workflows/bad.yml:1:uses: sigstore/cosign-installer@v3
PASS: gate would correctly fail this input
```
The gate matches `@v3` on `sigstore/cosign-installer` and exits 1, as expected.

## Task Commits

1. **Task 1: Append sigstore-pin-check job to ci.yml** - `1bdef3c` (feat)

## Files Created/Modified

- `.github/workflows/ci.yml` — New `sigstore-pin-check` job appended after `crit-01-client-grep-check` (36 lines inserted)

## Decisions Made

- **D-04 narrow gate:** 4-action target list only, not a broad every-uses-must-be-pinned gate. Pre-existing `dtolnay/rust-toolchain@stable` and similar are out of scope — broad gate is a v1.7 carry-forward.
- **D-09 new job in ci.yml:** Mirrors `bip322-pin-check` / `crit-01-grep-check` job family. No standalone workflow (overkill for a 4-action grep target).
- **RESEARCH §2.2 extension:** `anchore/sbom-action` added as the 4th target because `actions/attest-sbom` v2.4.0 does NOT generate SBOMs internally — it only signs a pre-existing SBOM file. Syft (via `anchore/sbom-action`) is the ATTEST-03 load-bearing SBOM generator.
- **Plan 22-04 lesson applied:** SECURITY.md + PITFALLS.md citations are inline string literals in the `echo` block, not extracted into a `POLICY_REF` shell variable. This preserves file-level audit-grep count accuracy.

## Cross-Reference to Plan 23-04

The gate's error message cites `SECURITY.md § Supply-chain status > Image signatures`. That anchor is written by Plan 23-04 (SECURITY.md full rewrite of the `## Supply-chain status` section). When Plan 23-04 lands, the anchor will resolve. Before it lands, the citation is a forward-reference (the section exists but the "Image signatures" subsection does not yet — no functional impact on the gate itself).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- Plans 23-01 and 23-02 (docker.yml sign/attest steps) must land with SHA-pinned uses: lines for all 4 target actions. After those land, the gate passes non-vacuously.
- Plan 23-04 (SECURITY.md rewrite) creates the `Image signatures` anchor that the gate's error message cites.
- Phase 24 (release.yml tarball signing) reuses the same 4-action target list — the gate automatically inherits invariant coverage without any gate maintenance.

## Self-Check: PASSED

- `.github/workflows/ci.yml` found and contains `sigstore-pin-check` job
- Commit `1bdef3c` exists in git log
- All acceptance criteria verified: YAML valid, job ID present, job name present, runs-on ubuntu-latest, 4 target actions in pattern, negative-lookahead regex present, grep -rnPE flag, project-wide checkout pin, Phase 23 ATTEST citation, SECURITY.md cited, PITFALLS.md cited, Supply-chain literal present, POLICY_REF absent, exit 1 in if-block, no needs:, job placed after crit-01-client-grep-check

---
*Phase: 23-cosign-image-attestations-slsa-provenance-sbom*
*Completed: 2026-06-01*
