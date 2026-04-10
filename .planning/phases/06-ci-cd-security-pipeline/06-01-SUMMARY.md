---
phase: 06-ci-cd-security-pipeline
plan: "01"
subsystem: ci-cd
tags: [ci, github-actions, cargo-audit, clippy, branch-protection]
dependency_graph:
  requires: []
  provides: [CI-gate, release-gate, docker-gate]
  affects: [.github/workflows/ci.yml, .github/workflows/release.yml, .github/workflows/docker.yml]
tech_stack:
  added: [cargo-audit, github-actions-ci]
  patterns: [PR-gate, needs-prerequisite-job, required-status-checks]
key_files:
  created:
    - .github/workflows/ci.yml
    - docs/branch-protection.md
  modified:
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
decisions:
  - "Audit denies only high/critical CVEs (--deny high --deny critical) — low/medium are informational per D-04/D-05"
  - "Three separate CI jobs (test, clippy, audit) so GitHub shows distinct required status checks"
  - "Release/docker check jobs combine test+clippy only — no audit overhead on tag push critical path"
  - "Branch protection documented as manual UI steps per D-06 — no gh CLI commands"
metrics:
  duration_minutes: 3
  completed_date: "2026-04-09"
  tasks_completed: 3
  tasks_total: 3
  files_created: 2
  files_modified: 2
---

# Phase 6 Plan 1: CI/CD Security Pipeline Summary

**One-liner:** PR-triggered CI gate with three independent jobs (cargo test, cargo clippy --deny warnings, cargo audit --deny high/critical) plus release/docker prerequisite checks via needs: check.

## What Was Built

### .github/workflows/ci.yml (created)

PR-triggered CI workflow on `pull_request` targeting `main`. Three independent jobs:

- `test` — `cargo test --workspace`
- `clippy` — `cargo clippy --workspace -- -D warnings`
- `audit` — `cargo install cargo-audit --locked` then `cargo audit --deny high --deny critical`

Audit uses `--locked` for reproducible installs (T-06-01 supply-chain mitigation). Low and medium CVEs produce warnings only; high and critical block the PR.

### .github/workflows/release.yml (modified)

Added `check` job (test + clippy) before the existing `build` job. Build job now has `needs: check`. Tag trigger and all release upload steps preserved unchanged.

### .github/workflows/docker.yml (modified)

Added `check` job (test + clippy) before the existing `docker` job. Docker job now has `needs: check`. Matrix strategy (coordinator, client, liquidity-bot), tag trigger, and all push steps preserved unchanged.

### docs/branch-protection.md (created)

Step-by-step GitHub UI instructions for requiring `cargo test`, `cargo clippy`, and `cargo audit` as status checks on the `main` branch. Explains why this is a one-time manual step and when status checks become searchable in the GitHub UI.

## Audit Policy In Effect

| Severity | Disposition |
|----------|-------------|
| Critical | Blocks CI — `--deny critical` |
| High     | Blocks CI — `--deny high` |
| Medium   | Warning only — informational |
| Low      | Warning only — informational |

## Status Check Names for Branch Protection

| Status Check | CI Job | Command |
|---|---|---|
| `cargo test` | `test` | `cargo test --workspace` |
| `cargo clippy` | `clippy` | `cargo clippy --workspace -- -D warnings` |
| `cargo audit` | `audit` | `cargo audit --deny high --deny critical` |

## Next Manual Step

Configure branch protection per `docs/branch-protection.md`:
- Go to Settings → Branches → Add rule for `main`
- Enable "Require status checks to pass before merging"
- Add: `cargo test`, `cargo clippy`, `cargo audit`

Open a draft PR first to trigger an initial CI run so the checks appear in the search box.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | a3d4a7e | feat(06-01): add CI workflow with test, clippy, and audit gates |
| 2 | a7b4982 | feat(06-01): add check prerequisite jobs to release and docker workflows |
| 3 | c305d95 | docs(06-01): add branch protection setup guide for required status checks |

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None. No new network endpoints, auth paths, or trust boundaries introduced. Threat model mitigations from plan applied:
- T-06-01: `--locked` flag used on `cargo install cargo-audit`
- T-06-03: ci.yml uses default (read) permissions; release.yml scoped to `contents: write`; docker.yml scoped to `packages: write`

## Self-Check: PASSED

- .github/workflows/ci.yml: exists
- .github/workflows/release.yml: contains `needs: check`
- .github/workflows/docker.yml: contains `needs: check`
- docs/branch-protection.md: exists
- All three YAML files: valid syntax
- Commits a3d4a7e, a7b4982, c305d95: present in git log
