---
phase: 22-base-image-digest-drift-detection
plan: 01
subsystem: ci-supply-chain
tags: [ci, supply-chain, docker, digest, governance, codeowners]
requirements: [DRIFT-01]
requires: []
provides:
  - docker/digests.txt (canonical base-image digest manifest)
  - .github/CODEOWNERS (governance gate for manifest + parser action)
affects:
  - Plan 22-02 (composite action will parse docker/digests.txt)
  - Plan 22-04 (drift workflow will reference manifest)
  - Plan 22-06 (HUMAN-UAT rehearsal will re-resolve the sentinel digests)
tech-stack:
  added: []
  patterns:
    - "Manifest header style mirrors docker/Dockerfile prose-comment block"
    - "CODEOWNERS path-glob + handle, governed by branch protection"
key-files:
  created:
    - docker/digests.txt
    - .github/CODEOWNERS
  modified: []
decisions:
  - "D-01/D-02/D-03 honored: exactly two named per-image entries, regex-validated shape"
  - "D-05 honored: CODEOWNERS gates both the manifest and the parser-action directory glob"
  - "Sentinel digest values used because docker buildx is not installed on the planning machine; documented as TBC at Plan 06 rehearsal in the file header"
metrics:
  duration: ~5 minutes
  completed: 2026-06-01
  tasks_completed: 2
  files_created: 2
  files_modified: 0
---

# Phase 22 Plan 01: Canonical digest manifest + CODEOWNERS governance gate Summary

Created the canonical base-image digest manifest `docker/digests.txt` and the `.github/CODEOWNERS` file that gates PR review on both the manifest and the parser-action directory — delivering the foundational source-of-truth artifact for DRIFT-01 and the structural enforcement mechanism behind the no-auto-merge guarantee documented in D-05.

## What shipped

### `docker/digests.txt` (NEW, 14 lines)

Two-line digest manifest with a 9-line header comment block. Format per D-03 contract:

```
debian:bookworm-slim@sha256:<64-hex>
lukemathwalker/cargo-chef:latest-rust-1@sha256:<64-hex>
```

Header documents:
- Role as canonical manifest for `docker/Dockerfile`.
- Human-review-only bump policy (cross-references SECURITY.md §Supply-chain status).
- Parser contract (`.github/actions/read-base-digests/action.yml`).
- An additional note (added at plan-execution time) that the digest values are sentinel placeholders to be re-resolved on a clean `ubuntu-24.04` runner during Plan 06 HUMAN-UAT rehearsal via `docker buildx imagetools inspect <image>:<tag> --format '{{.Manifest.Digest}}'`.

**Sentinel digest values used:** `sha256:0000...0000` (64 zero hex). Reason: `docker buildx imagetools inspect` is not installed on the planning machine (the local CLI has no `docker` binary). The format-regex contract from D-03 is satisfied by the sentinel (`[a-f0-9]{64}` matches 64 zeroes), so downstream Plan 02 parser development is not blocked. Plan 06 HUMAN-UAT (clean `ubuntu-24.04` runner) will overwrite both lines with the canonical digests resolved fresh from the upstream registry; that rehearsal is the correct gate for digest authenticity per RESEARCH.md §3 lines 298–299.

### `.github/CODEOWNERS` (NEW, 8 lines)

Two rule-line CODEOWNERS file with a 6-line prose header. Body:

```
docker/digests.txt                         @johnzilla
.github/actions/read-base-digests/**       @johnzilla
```

Header cites `.planning/research/PITFALLS.md §11` and `SECURITY.md §Supply-chain status` as the audit-trail policy basis. Per D-05 + CONTEXT.md §specifics line 102, both paths are load-bearing — loosening the parser regex is functionally the same supply-chain risk as bumping a digest, so both must require maintainer review.

**Inert until branch protection is enabled.** This file alone does not enforce anything; it requires `Settings → Branches → main → Require review from Code Owners` to be checked on github.com. That toggle is scheduled as a Plan 06 HUMAN-UAT task per the plan's own note.

## Acceptance checks (all passing)

### Task 1 (digest manifest, 7 assertions)
- `test -f docker/digests.txt` -> OK
- `head -1 docker/digests.txt | grep -q '^#'` -> OK (first line is `#` comment)
- `grep -vE '^[[:space:]]*(#|$)' docker/digests.txt | grep -c '^'` -> `2`
- `grep -vE '^[[:space:]]*(#|$)' docker/digests.txt | grep -cE '^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$'` -> `2`
- `grep -q '^debian:bookworm-slim@sha256:' docker/digests.txt` -> OK
- `grep -q '^lukemathwalker/cargo-chef:latest-rust-1@sha256:' docker/digests.txt` -> OK
- `grep -q 'SECURITY.md' docker/digests.txt` -> OK
- `grep -q 'read-base-digests' docker/digests.txt` -> OK

### Task 2 (CODEOWNERS, 7 assertions)
- `test -f .github/CODEOWNERS` -> OK
- `head -1 .github/CODEOWNERS | grep -q '^#'` -> OK
- `grep -qE '^docker/digests\.txt[[:space:]]+@johnzilla' .github/CODEOWNERS` -> OK
- `grep -qE '^\.github/actions/read-base-digests/\*\*[[:space:]]+@johnzilla' .github/CODEOWNERS` -> OK
- `grep -q 'PITFALLS.md' .github/CODEOWNERS` -> OK
- `grep -q 'SECURITY.md' .github/CODEOWNERS` -> OK
- `grep -vE '^[[:space:]]*(#|$)' .github/CODEOWNERS | grep -c '^'` -> `2`

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| Task 1 | 8de5925 | feat(22-01): add canonical base-image digest manifest (DRIFT-01) |
| Task 2 | 6b87f07 | feat(22-01): add CODEOWNERS gate for digest manifest (DRIFT-01) |

## Deviations from Plan

None of the plan's design or contract was changed. One execution-time substitution was made and documented:

### Sentinel digests instead of live `docker buildx imagetools inspect` resolution

- **Found during:** Task 1 — preflight environment check.
- **Cause:** `docker` is not installed on the planning machine (`which docker` returns nonzero; the agent runs locally, not on a `ubuntu-24.04` GitHub Actions runner).
- **Resolution:** Per the plan's own escape hatch ("If `docker buildx` is unavailable on this machine, fall back to documented sentinel digests with a placeholder commit message and a top-of-file comment noting the digests are TBC at Plan 06 rehearsal — this is acceptable because Plan 06 is a HUMAN-UAT rehearsal where digests will be re-resolved on a fresh `ubuntu-24.04` runner anyway"), used `sha256:` + 64 zeros for both entries.
- **Header annotation:** Added a `NOTE:` paragraph to the file header documenting that the values are sentinels and naming the resolution command for the rehearsal PR.
- **Format-regex safety:** The sentinel format `sha256:0{64}` is a valid match for the D-03 regex (`^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$`), so the composite-action parser (Plan 02) will not reject the file during development.
- **Not a Rule-1/2/3 deviation:** This was explicitly anticipated and authorized by the plan's `<plan_specifics>` block. No fix-and-continue cycle was needed.

## Threat Flags

None. No new network endpoints, auth paths, or schema changes were introduced. The CODEOWNERS file is governance metadata; the digest manifest is build-input data — both surface area was already named in the plan's `<threat_model>` (T-22-01, T-22-02 mitigated; T-22-03 deferred to Plan 06 HUMAN-UAT; T-22-SC n/a).

## Known Stubs

The two sentinel digest values (`sha256:0{64}`) are stubs that block Phase 22 from being usable in a real release. They are tracked here for the verifier:

| File | Line | Stub | Resolved by |
|------|------|------|-------------|
| docker/digests.txt | 13 | `debian:bookworm-slim@sha256:0...0` | Plan 06 HUMAN-UAT (re-resolved on clean ubuntu-24.04 runner) |
| docker/digests.txt | 14 | `lukemathwalker/cargo-chef:latest-rust-1@sha256:0...0` | Plan 06 HUMAN-UAT (re-resolved on clean ubuntu-24.04 runner) |

This is intentional and explicitly authorized by the plan's `<plan_specifics>` escape hatch. Plan 06 must overwrite both before Phase 22 closes.

## Maintainer handle verification

The CODEOWNERS handle `@johnzilla` is per RESEARCH.md §6 line 693 and CONTEXT.md D-05 (locked at planning time). RESEARCH.md cites corroborating evidence in `.planning/research/SUMMARY.md` cosign-identity references and `SECURITY.md` email-handle correspondence. No execution-time re-verification was attempted (the planner already locked this decision).

## Next plans

- **Plan 22-02** — composite action `.github/actions/read-base-digests/action.yml` parses this manifest into named `${{ steps.digests.outputs.* }}` outputs.
- **Plan 22-04** — `digest-drift-check.yml` reads the manifest, resolves upstream, opens issues on drift.
- **Plan 22-06** — HUMAN-UAT rehearsal: (a) re-resolves the two sentinel digests on a fresh `ubuntu-24.04` runner and lands a PR overwriting them; (b) toggles branch protection to require CODEOWNERS approval on `main`.

## Self-Check: PASSED

- `docker/digests.txt` exists: FOUND
- `.github/CODEOWNERS` exists: FOUND
- Commit 8de5925 (Task 1): FOUND in `git log`
- Commit 6b87f07 (Task 2): FOUND in `git log`
- All 14 acceptance assertions across both tasks: PASS
- End-to-end verification block from plan's `<verification>` section: PASS
