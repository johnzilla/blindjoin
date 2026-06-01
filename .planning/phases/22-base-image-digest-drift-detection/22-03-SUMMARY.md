---
phase: 22-base-image-digest-drift-detection
plan: 03
subsystem: infra
tags: [github-actions, supply-chain, docker, digest-pinning, release-pipeline, composite-action-consumer]

# Dependency graph
requires:
  - phase: 22-base-image-digest-drift-detection
    provides: docker/digests.txt canonical manifest (Plan 22-01)
  - phase: 22-base-image-digest-drift-detection
    provides: read-base-digests composite action with named outputs debian_ref + cargo_chef_ref (Plan 22-02)
provides:
  - "release.yml `build` job invokes `./.github/actions/read-base-digests` as a structural supply-chain gate between rust-cache and `Build coordinator and client` — a tag push cannot publish a release tarball without a valid `docker/digests.txt`"
  - "docker.yml `docker` matrix job invokes `./.github/actions/read-base-digests` between `docker/metadata-action` and `docker/build-push-action`, then threads `debian_ref` + `cargo_chef_ref` into `docker/build-push-action`'s `build-args:` pipe-multiline block — every ghcr.io image push uses the canonical pinned digests automatically (DRIFT-03 satisfied)"
  - "Cross-file invariant verified: `build-args:` keys `DEBIAN_REF` + `CARGO_CHEF_REF` match the `ARG` names on `docker/Dockerfile` lines 32-33 exactly; Dockerfile NOT modified"
affects:
  - "Plan 22-04 (digest-drift-check.yml) — independent consumer of the same composite action; this plan establishes the consumer-side pattern for `id: digests` + `uses: ./.github/actions/read-base-digests`"
  - "Plan 22-05 (SECURITY.md + CONTRIBUTING.md prose) — the prose half of D-05 can now describe DRIFT-03 as shipped: workflows physically cannot publish without reading the manifest"
  - "Plan 22-06 (Human-UAT) — fresh-machine rehearsal of ROADMAP SC#3 (`grep '@sha256:' docker/digests.txt` against the build logs confirms the canonical digest was used) is now exercisable"
  - "Phase 25 (REPRO-01 reproducibility recipe) — `${{ steps.digests.outputs.* }}` is available in release.yml's build job; a future Phase 25 step can read it without re-inserting the composite-action call"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composite-action consumer pattern: `- name: Read canonical base-image digests` + `id: digests` + `uses: ./.github/actions/read-base-digests` — no SHA pin, no `@`, no trailing `# v` (local actions are SHA-implicit via the checkout)"
    - "docker/build-push-action `build-args:` pipe-multiline block placed between `labels:` and `cache-from:` (mirrors the existing `tags: |` form on the same step)"
    - "Tag-gate inheritance — composite-action step inserted inside a job with `if: startsWith(github.ref, 'refs/tags/')` inherits the gate; no per-step `if:` needed"
    - "Prose-comment-as-contract above the inserted step cites the requirement ID (`Phase 22 DRIFT-03`) and cross-references the consumed composite action's `action.yml`"

key-files:
  created: []
  modified:
    - .github/workflows/release.yml
    - .github/workflows/docker.yml

key-decisions:
  - "Followed the locked shape in RESEARCH.md §5.1 (lines 562-571) verbatim for the release.yml insertion — 8-line prose comment + step header + `id: digests` + `uses:` line. No additional `Echo canonical digests` step added: the composite action already echoes the parsed digests to stdout (action.yml lines 107-109), so the SC#3 audit-observability requirement (`grep '@sha256:' docker/digests.txt` against the build logs) is satisfied by the composite action's own audit trail without a redundant echo step in the consuming workflow"
  - "Inserted `build-args:` block in docker/build-push-action between `labels:` and `cache-from:` (PATTERNS §3 line 317 insertion point). Used pipe-multiline YAML syntax (`build-args: |`) to mirror the existing `tags: |` form on the same step, keeping the with: block visually consistent"
  - "Preserved the job-level `if: startsWith(github.ref, 'refs/tags/')` gate on both `build:` and `docker:` jobs — the composite-action step inherits the gate from the job (PATTERNS §3 line 265). No per-step `if:` was added; lifting or duplicating the gate would weaken the publish-only-on-tag invariant"
  - "Dockerfile NOT modified — `ARG DEBIAN_REF` / `ARG CARGO_CHEF_REF` on lines 32-33 already match the build-args keys passed by docker.yml (cross-file invariant from PATTERNS §3 line 477). The Dockerfile was intentionally scaffolded in v1.5 P0-2/3 to receive these args; Plan 22-03 finally makes that scaffold load-bearing in production"

patterns-established:
  - "Composite-action consumer pattern in workflows: insertion goes AFTER setup steps (checkout/toolchain/cache for release.yml; checkout/login/buildx/metadata for docker.yml) and BEFORE the consuming step (cargo build for release.yml; docker/build-push-action for docker.yml). Each insertion has a 7-8 line prose comment above the step naming the requirement ID and cross-referencing the consumed action's `action.yml`"
  - "docker/build-push-action `build-args:` field placement: between `labels:` and `cache-from:` — keeps deterministic-build inputs (tags, labels, build-args) grouped before cache-handling fields"

requirements-completed:
  - DRIFT-03

# Metrics
duration: ~6min
completed: 2026-06-01
---

# Phase 22 Plan 03: release.yml + docker.yml Composite-Action Wiring Summary

**`release.yml` and `docker.yml` both invoke `./.github/actions/read-base-digests` on every tag-gated job, and `docker.yml` threads the parsed `debian_ref` + `cargo_chef_ref` outputs into `docker/build-push-action`'s `build-args:` — every tagged release build now reads the canonical digest manifest automatically, with no manual `--build-arg` invocation (DRIFT-03 satisfied).**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-06-01T21:25:40Z (Plan 22-02 close per STATE.md)
- **Completed:** 2026-06-01T21:32:00Z
- **Tasks:** 2
- **Files modified:** 2 (release.yml + docker.yml)
- **Files created:** 0
- **Lines added:** 29 (release.yml: +11; docker.yml: +18)
- **Lines removed:** 0
- **Dockerfile:** untouched (verify-only invariant)

## What Was Built

### Task 1 — `.github/workflows/release.yml` build job consumer wiring

Inserted ONE step in the `build:` job between the existing `Swatinem/rust-cache@e18b497...` step and the existing `Build coordinator and client` step:

```yaml
      # Phase 22 DRIFT-03: read the canonical base-image digest manifest.
      # This step is a supply-chain gate — a tag push cannot publish a
      # release tarball unless docker/digests.txt is present and well-formed
      # (the composite action exits 1 otherwise; see
      # .github/actions/read-base-digests/action.yml). Outputs are unused by
      # the cargo build directly but exported for downstream phases
      # (Phase 25 reproducibility recipe) and for log-audit observability.
      - name: Read canonical base-image digests
        id: digests
        uses: ./.github/actions/read-base-digests
```

- **Insertion ordering verified** (AST walk): `rust-cache (idx 2) < Read canonical base-image digests (idx 3) < Build coordinator and client (idx 4)`.
- **Tag-gate preserved**: the `build:` job's `if: startsWith(github.ref, 'refs/tags/')` (line 66) is unchanged; the inserted step inherits the gate.
- **No new third-party actions**: `./.github/actions/read-base-digests` is local and SHA-implicit (`git diff` filter confirmed 0 new third-party `uses:` lines).
- **Permissions unchanged**: top-level `contents: write` covers the composite action's `contents: read` need.
- **No `cargo build` modification**: v1.6 Phase 22 does not thread digest outputs into cargo's command line (RESEARCH.md §5.1 lines 540-545 defer that to Phase 25). The step's role here is supply-chain gate + log audit + future-proofing for Phase 25.

**Commit:** `4a0b59f` — `feat(22-03): wire release.yml to read canonical base-image digest manifest (DRIFT-03)`

### Task 2 — `.github/workflows/docker.yml` matrix job consumer + build-args wiring

Two coordinated edits in the `docker:` matrix job:

**Edit 1 — Inserted composite-action step** between `docker/metadata-action` (id: meta) and `docker/build-push-action`:

```yaml
      # Phase 22 DRIFT-03: read the canonical base-image digest manifest
      # and pass its values to docker buildx via --build-arg. This
      # eliminates manual --build-arg invocation per the v1.5 P0-2/3
      # Dockerfile-side ARG scaffold. A tag push cannot publish ghcr.io
      # images unless docker/digests.txt is present and well-formed
      # (the composite action exits 1 otherwise; see
      # .github/actions/read-base-digests/action.yml).
      - name: Read canonical base-image digests
        id: digests
        uses: ./.github/actions/read-base-digests
```

**Edit 2 — Added `build-args:` pipe-multiline block** to the `docker/build-push-action` step, between `labels:` and `cache-from:`. This is the exact block Plan 04 (drift-check) and Plan 06 (UAT) should reference per the plan's `<output>` directive:

```yaml
          # Phase 22 DRIFT-03: pinned base-image digests from
          # docker/digests.txt threaded as build args. The Dockerfile's
          # `ARG CARGO_CHEF_REF` / `ARG DEBIAN_REF` scaffold (v1.5 P0-2/3)
          # consumes these in the `FROM ${...}` lines.
          build-args: |
            DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}
            CARGO_CHEF_REF=${{ steps.digests.outputs.cargo_chef_ref }}
```

- **Insertion ordering verified** (AST walk): `metadata-action (idx 3) < Read canonical base-image digests (idx 4) < build-push-action (idx 5)`.
- **Tag-gate preserved**: the `docker:` job's `if: startsWith(github.ref, 'refs/tags/')` is unchanged.
- **SHA pins preserved**: `docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0` + `docker/metadata-action@c299e40c65443455700f0fdfc63efafe5b349051 # v5.10.0` unchanged.
- **build-push-action other fields unchanged**: `context`, `file`, `target`, `push`, `tags`, `labels`, `cache-from`, `cache-to` are identical to pre-edit shape.
- **Cross-file invariant verified**: `ARG DEBIAN_REF=` + `ARG CARGO_CHEF_REF=` present on `docker/Dockerfile` lines 32-33; Dockerfile diff = 0 lines.
- **Permissions unchanged**: job-level `contents: read` + `packages: write` already covers the composite action.
- **Matrix strategy untouched**: 3-entry matrix (coordinator, client, liquidity-bot) unchanged; each entry runs the same composite-action + build-args wiring via matrix expansion.

**Commit:** `7556c1b` — `feat(22-03): thread canonical digests into docker.yml build-args (DRIFT-03)`

## Verification Evidence

All plan-level acceptance criteria pass on disk after the two commits:

```text
BOTH_YAML_OK                                        # python3 yaml.safe_load on both files
.github/workflows/docker.yml                        # grep -l 'uses: ./.github/actions/read-base-digests'
.github/workflows/release.yml                       # grep -l 'uses: ./.github/actions/read-base-digests'
            DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}       # grep -F in docker.yml
            CARGO_CHEF_REF=${{ steps.digests.outputs.cargo_chef_ref }}  # grep -F in docker.yml
DOCKERFILE_DEBIAN_OK                                # ARG DEBIAN_REF= present in Dockerfile
DOCKERFILE_CHEF_OK                                  # ARG CARGO_CHEF_REF= present in Dockerfile
```

AST-walk confirmations:
- release.yml `build:` step ordering: `rust-cache(2) < digest(3) < build(4)` PASS
- docker.yml `docker:` step ordering: `metadata(3) < digest(4) < build-push(5)` PASS
- docker.yml build-push-action `with.build-args` contains both `DEBIAN_REF=` and `CARGO_CHEF_REF=` PASS

`git diff` third-party-action filter: 0 new third-party `uses:` lines in either commit. The only new `uses:` is `./.github/actions/read-base-digests` (local, SHA-implicit).

## How DRIFT-03 Is Now Satisfied

The REQUIREMENTS.md entry for DRIFT-03 reads:

> `release.yml` + `docker.yml` updated to read `docker/digests.txt` and pass `--build-arg DEBIAN_REF=...` + `--build-arg CARGO_CHEF_REF=...` automatically from the manifest values. Means every tagged release build is built from the canonical digest list with no manual `--build-arg` invocation required.

Mapping:

| DRIFT-03 clause | Where satisfied |
|---|---|
| "release.yml ... updated to read docker/digests.txt" | release.yml `build:` job now invokes `./.github/actions/read-base-digests` (commit `4a0b59f`); the composite action reads `docker/digests.txt` and emits parsed outputs. |
| "docker.yml updated to read docker/digests.txt" | docker.yml `docker:` job now invokes `./.github/actions/read-base-digests` (commit `7556c1b`). |
| "pass --build-arg DEBIAN_REF=... + --build-arg CARGO_CHEF_REF=... automatically from the manifest values" | docker.yml `docker/build-push-action` now has a `build-args:` pipe-multiline block threading both outputs (commit `7556c1b`). The pattern `--build-arg` is the docker-CLI shape; `docker/build-push-action`'s native `build-args:` block is the workflow-action shape (the standard pattern per docker/build-push-action docs). |
| "every tagged release build is built from the canonical digest list with no manual --build-arg invocation required" | Both workflows preserve the existing `if: startsWith(github.ref, 'refs/tags/')` job-level tag-gate. The composite-action call inherits the gate, so every tag push triggers the canonical-digest read; no human typing `--build-arg` is involved. |

This also feeds ROADMAP SC#3:

> A tagged release build (`release.yml` and `docker.yml`) succeeds without any manual `--build-arg DEBIAN_REF=...` argument because the workflows read `docker/digests.txt` and pass the digests automatically; `grep '@sha256:' docker/digests.txt` against the build logs confirms the canonical digest was used.

The grep-against-build-logs check will pass because the composite action's stdout audit-trail lines (action.yml lines 107-109) print `debian_ref: <image>@sha256:HEX` + `cargo_chef_ref: <image>@sha256:HEX` to the runner log; an auditor grepping `@sha256:` against the run log finds the canonical digests there.

## Deviations from Plan

**None — plan executed exactly as written.**

- No bugs found (Rule 1): both workflows parse as valid YAML; both already had the correct tag-gates and SHA pins in place.
- No missing critical functionality added (Rule 2): the threat-model entries T-22-08 through T-22-11 are all `mitigate` (Task 2 acceptance criterion verifies cross-file invariant) or `accept` (no action) — no new files needed.
- No blocking issues hit (Rule 3): both workflows existed in the expected shape; insertion points matched PATTERNS §3 exactly.
- No architectural changes (Rule 4): scope was tightly bounded — two YAML edits.
- No `npm install` / `pip install` / `cargo add` invocations (Rule 3 exclusion): pure YAML edits, no package operations.

The orchestrator's `<plan_specifics>` block mentioned an optional "Echo canonical digests for audit log" step in release.yml; this was NOT added because (a) PLAN.md's task action locks the shape verbatim to RESEARCH.md §5.1 lines 562-571 which does NOT include such a step, (b) PLAN.md's acceptance criteria do NOT grep for an "Echo canonical digests" step name, and (c) the composite action already echoes the parsed digests to stdout (action.yml lines 107-109), so ROADMAP SC#3's audit-observability requirement is satisfied without a redundant echo step. Documented here for traceability — this is a faithful reading of the locked PLAN.md, not a deviation.

## Authentication Gates

None. No auth was required to make these workflow-file edits; the workflows themselves will use `GITHUB_TOKEN` at run time, but that's a separate concern not exercised by Plan 22-03.

## Known Stubs

None. The two edits are fully wired (no placeholder/empty values, no TODO/FIXME, no mock data). The composite action's outputs are consumed end-to-end into `docker/build-push-action`'s `build-args:` and into release.yml's `${{ steps.digests.outputs.* }}` namespace for future-phase consumption.

## Threat Flags

None. No new security-relevant surface introduced. The composite-action invocation strengthens (not weakens) the supply-chain posture: every tagged release now structurally cannot publish without a valid manifest read. The threat-register entries T-22-08 through T-22-11 from PLAN.md remain accurately dispositioned; T-22-08 (workflow-file tampering carry-forward to Phase 23 CODEOWNERS extension) is the only residual and is already tracked for Phase 23.

## Self-Check: PASSED

- `.github/workflows/release.yml` exists and contains `uses: ./.github/actions/read-base-digests` (verified).
- `.github/workflows/docker.yml` exists and contains `uses: ./.github/actions/read-base-digests` (verified).
- `.github/workflows/docker.yml` `build-args:` block contains both `DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}` and `CARGO_CHEF_REF=${{ steps.digests.outputs.cargo_chef_ref }}` (verified via `grep -F`).
- Both workflows pass `python3 -c 'import yaml; yaml.safe_load(...)'` (verified).
- `docker/Dockerfile` retains `ARG DEBIAN_REF=` and `ARG CARGO_CHEF_REF=` lines unchanged; `git diff docker/Dockerfile` returns 0 lines (verified).
- Job-level `if: startsWith(github.ref, 'refs/tags/')` gates preserved on both `build:` (release.yml line 66) and `docker:` (docker.yml line 60) jobs (verified via grep).
- Commit `4a0b59f` (Task 1) found in `git log` (verified).
- Commit `7556c1b` (Task 2) found in `git log` (verified).
- No deletions in either commit (verified via `git diff --diff-filter=D HEAD~1 HEAD`).
