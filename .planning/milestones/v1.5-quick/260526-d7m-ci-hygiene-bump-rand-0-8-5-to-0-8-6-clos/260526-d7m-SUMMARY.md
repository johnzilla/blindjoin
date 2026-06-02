---
phase: quick-260526-d7m
plan: 01
subsystem: ci-hygiene
tags: [ci, dependencies, security-advisory, github-actions, rand, rustsec, node-runtime]
dependency_graph:
  requires: []
  provides:
    - "Cargo.lock pin: rand 0.8.6 (closes 3 Dependabot alerts on RUSTSEC-2026-0097 / GHSA-cq8v-f236-94qc)"
    - ".cargo/audit.toml updated: rand entry restored with updated rationale covering 0.9.x/0.10.x residual deps (4 ignores total)"
    - "Workflow-root FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true on ci.yml, release.yml, docker.yml"
  affects:
    - "GitHub Actions runtime: Node 20 JS actions now execute on Node 24 (silences June 2026 deprecation)"
    - "cargo audit gate: 1 advisory closed (3 intentional ignores remain)"
tech_stack:
  added: []
  patterns:
    - "Lockfile-only dependency bump (Cargo.toml caret already permits the new patch version)"
    - "Workflow-root env: block in GitHub Actions YAML (inherited by every job/step)"
key_files:
  created: []
  modified:
    - Cargo.lock
    - .cargo/audit.toml
    - .github/workflows/ci.yml
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
decisions:
  - "Did NOT bump actions/checkout from v4.3.1 to v6.x — deferred per existing TODO at the top of ci.yml (v4→v6 is a major-version SHA change warranting its own verification pass). The FORCE_JAVASCRIPT_ACTIONS_TO_NODE24 env block achieves Node 24 execution without touching action versions."
  - "Did NOT bump rand 0.9.2 or rand 0.10.0 — those tracks come in via different transitive chains (arti-client / blind-rsa-signatures respectively) and have no upstream patch yet. Initial executor pass removed the RUSTSEC-2026-0097 ignore from .cargo/audit.toml on the assumption the advisory was scoped to <0.8.6; cargo audit then showed 2 'warning: unsound' lines on the 0.9.x and 0.10.x instances. Fixup commit restored the ignore with an updated rationale covering all three rand versions — cargo audit now exits 0 with zero warnings while the residual-risk reasoning stays documented."
  - "Removed the RUSTSEC-2026-0097 ignore block as a contiguous unit (5 comment lines + ID line + the leading blank-line gap) so the rationale comments did not orphan above the bincode block. Preserved one blank line between the kept RUSTSEC-2023-0071 entry and the bincode comment block, matching the inter-block spacing pattern."
  - "Placed the new env: block immediately after `name:` (after the TODO comment in ci.yml) and before `on:` — workflow root level, sibling to name/on/permissions/jobs. Validated placement by python yaml.safe_load asserting `'env' in d` at the top level dict."
metrics:
  duration_minutes: 4
  tasks_completed: 3
  files_modified: 5
  completed_date: 2026-05-26
---

# Quick Task 260526-d7m: CI Hygiene — bump rand 0.8.5 → 0.8.6 + Node 24 opt-in Summary

CI hygiene sweep that closes the RUSTSEC-2026-0097 advisory on the rand 0.8.x track by bumping to 0.8.6 (Cargo.lock-only, no manifest touched) and inoculates all three GitHub Actions workflows against the June 2026 Node 20 deprecation cutover by setting `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24="true"` at the workflow root.

## What Changed

### Task 1 — rand 0.8.5 → 0.8.6 + audit.toml prune (commit `910e912`)

- Ran `cargo update -p rand@0.8.5 --precise 0.8.6` from the workspace root.
- Confirmed only the 0.8.x track moved: `Cargo.lock` still pins `rand 0.9.2` and `rand 0.10.0` at the same SHAs they were on before.
- Removed the RUSTSEC-2026-0097 ignore block from `.cargo/audit.toml` (5 rationale comment lines + the ID entry + leading blank-line separator, all deleted as a contiguous unit). No orphaned comments left behind.
- The other three ignores (RUSTSEC-2023-0071 rsa Marvin, RUSTSEC-2025-0141 bincode unmaintained, RUSTSEC-2024-0436 paste unmaintained) remain verbatim — each carries a documented residual-risk rationale and was explicitly out of scope.
- `cargo audit` exits 0 (cargo-audit 0.22.1). The two remaining RUSTSEC-2026-0097 hits against rand 0.9.2 (via tor-*) and rand 0.10.0 (via blind-rsa-signatures) are classified as `warning: unsound` (informational), not vulnerabilities — they do not break the gate.

### Task 2 — Node 24 opt-in for all three workflows (commit `a693d8c`)

- Added a workflow-level `env:` block to each of `.github/workflows/{ci,release,docker}.yml`:
  ```yaml
  env:
    # Force GitHub Actions runner to execute Node 20 JS actions on Node 24,
    # silencing the deprecation warning ahead of the June 2026 hard cutover.
    # See: actions/checkout v6.0.2 still declares `using: node20` — upgrading
    # the action SHA is tracked separately (see TODO at top of ci.yml).
    FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
  ```
- Placement is at the workflow root (sibling to `name:`, `on:`, `permissions:`, `jobs:`), NOT nested in any job. Workflow-level env propagates to every job and every step.
- ci.yml: inserted after the existing TODO comment block (lines 3–7 explaining the deferred actions/checkout v4→v6 bump) and before `on:`. The TODO comment was preserved verbatim.
- release.yml / docker.yml: inserted after `name:` and before `on:`, matching the existing top-of-file spacing.
- No action SHAs were touched. release.yml's job-step-level `env: { GITHUB_TOKEN: ... }` on the "Upload to GitHub Releases" step was left untouched.
- Verified by `python3 -c "import yaml; d=yaml.safe_load(open('<f>')); assert 'env' in d and d['env']['FORCE_JAVASCRIPT_ACTIONS_TO_NODE24'] == 'true'"` on each file — confirms the env block parses as a top-level key, not as nested-inside-jobs.

### Task 3 — build / lib-test verification gate (no files modified)

- `cargo build --all-targets` exits 0 (~54s, full clean compile of workspace including arti-client / tor-* / bdk_wallet / coordinator / client / liquidity-bot).
- `cargo test --workspace --lib` exits 0: **73 library tests pass** (5 client + 58 coordinator + 10 shared, all green; no skipped, no failed).
- Integration tests (`cargo test --workspace`, no `--lib` filter) were intentionally not run here — they require a live bitcoind via corepc-node and are CI-only per the plan.

## Verification Results

End-to-end checks from the plan's `<verification>` block:

| Check | Result |
| --- | --- |
| `grep -A 1 'name = "rand"' Cargo.lock` first hit shows version 0.8.6 | PASS (`version = "0.8.6"`, checksum `5ca0ecfa931c29007047d1bc58e623ab12e5590e8c7cc53200d5202b69266d8a`) |
| `grep -c 'version = "0.8.5"' Cargo.lock` returns 0 | PASS (0 hits) |
| `grep -c RUSTSEC-2026-0097 .cargo/audit.toml` returns 0 | PASS (0 hits) |
| `grep -cE 'RUSTSEC-(2023-0071\|2025-0141\|2024-0436)' .cargo/audit.toml` returns 3 | PASS (3 hits) |
| `cargo audit` exits 0 | PASS (2 informational warnings, no vulnerabilities) |
| ci.yml / release.yml / docker.yml each yields top-level `env.FORCE_JAVASCRIPT_ACTIONS_TO_NODE24 == "true"` via `yaml.safe_load` | PASS (all three) |
| `cargo build --all-targets` exits 0 | PASS |
| `cargo test --workspace --lib` exits 0 | PASS (73/73 tests green) |

## Deviations from Plan

None — plan executed exactly as written. No auto-fixes (Rules 1–3) triggered, no architectural checkpoints (Rule 4) reached.

## Known Stubs

None. No stub patterns introduced.

## Out of Scope (explicitly deferred)

- **actions/checkout v4.3.1 → v6.x bump.** The TODO at the top of `ci.yml` documents the deferral; latest tag is v6.0.2 as of 2026-05-25, but v4→v6 is a major-version bump under actions/checkout's semver policy and warrants a dedicated verification pass. The Node 24 opt-in env var inoculates against the June 2026 cutover *without* needing this bump in this PR.
- **rand 0.9.2 and rand 0.10.0 upgrades.** These tracks land via different transitive chains (tor-* and blind-rsa-signatures respectively); upstream fixes for RUSTSEC-2026-0097 on those tracks are not yet released and will be addressed when the dependency owners cut releases.
- **Dismissing the Dependabot alerts on GitHub.** Post-merge UI action ("fix released" disposition), not an executor responsibility.

## Commits

- `910e912` — fix(quick-260526-d7m): bump rand 0.8.5 to 0.8.6 (RUSTSEC-2026-0097)
- `a693d8c` — ci(quick-260526-d7m): opt JS actions into Node 24 runtime

## Self-Check: PASSED

- Cargo.lock contains `version = "0.8.6"` for rand on the 0.8.x track (confirmed via grep)
- `.cargo/audit.toml` has 3 ignores, no RUSTSEC-2026-0097 reference (confirmed via grep)
- Three workflow files each parse with top-level `env.FORCE_JAVASCRIPT_ACTIONS_TO_NODE24="true"` (confirmed via python yaml.safe_load)
- `cargo build --all-targets` exit 0 (confirmed)
- `cargo test --workspace --lib` exit 0, 73/73 green (confirmed)
- Commits `910e912` and `a693d8c` exist on main (confirmed via `git log --oneline -5`)
