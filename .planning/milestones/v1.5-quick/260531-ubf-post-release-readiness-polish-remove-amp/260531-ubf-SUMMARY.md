---
quick_id: 260531-ubf
status: complete
description: Post-release-readiness polish (amp cleanup, doc cross-links, version policy, smoke-rehearsal trigger)
date: 2026-06-01
commits:
  - ed2f5a4 — docs(quick-260531-ubf): cross-link SECURITY.md + CHANGELOG.md from README + CONTRIBUTING
  - 8259dd5 — docs(quick-260531-ubf): document crate-version policy in SECURITY.md (keep 0.1.0)
  - ceca7b4 — ci(quick-260531-ubf): enable release-smoke rehearsal via workflow_dispatch
files_changed:
  added: []
  modified:
    - README.md
    - CONTRIBUTING.md
    - SECURITY.md
    - Cargo.toml
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
  removed:
    - amp (untracked, filesystem-only)
---

# Quick task 260531-ubf — Summary

Four post-release-readiness items, three atomic code-changing commits.
No source changed; cargo check / clippy / audit still pass.

## Task A — Remove stray amp file ✓

`amp` was a 0-byte untracked file at repo root, dated 2026-05-29
(predates 260531-thw). Removed via `rm amp`. Was never tracked by git
so there's no commit for the removal — the file is simply gone from the
filesystem.

Verified: `ls amp` returns "No such file"; `git status` clean.

## Task B — Cross-link SECURITY.md + CHANGELOG.md ✓

- `README.md` § Documentation: prepended `**[Security policy](SECURITY.md)**`
  and `**[Changelog](CHANGELOG.md)**` to the list. Security disclosure
  outranks the FAQ for someone evaluating risk; placement reflects that.
- `README.md` § Security Model: opens with a "Reporting a vulnerability:
  see [SECURITY.md]" paragraph so the disclosure surface is not buried
  below the capability list.
- `CONTRIBUTING.md` § Tagging releases: adds a "Before tagging" note
  pointing at CHANGELOG.md as the user-facing release-notes surface and
  forward-references the SECURITY.md crate-version policy (committed in
  Task C).

Verification:
- `grep -c SECURITY.md README.md` → 3
- `grep -c CHANGELOG.md README.md` → 1
- `grep -c "CHANGELOG.md\|SECURITY.md" CONTRIBUTING.md` → 2

Commit: **ed2f5a4**.

## Task C — Document crate-version policy ✓

**Decision: keep all four workspace crates at `0.1.0`.** Recorded in
SECURITY.md as a new `## Release versioning policy` section. Rationale
(also captured in the section):

- None of `coordinator`, `client`, `liquidity-bot`, `shared` are
  published to crates.io; the `version` field is purely internal to the
  Cargo dependency graph.
- The canonical release identifier is the git tag (`v1.5.0`) + GitHub
  Release; that's what operators consume.
- The binaries expose no `--version` CLI flag (per the
  `coordinator-smoke` job comment in `ci.yml`), so `CARGO_PKG_VERSION`
  is never user-visible.
- Bumping the four `version =` lines at every milestone close would be
  churn with zero downstream consumer.

Revisit condition: if `--version` CLI flags are added later, the
binaries should derive the version from the git tag at build time
(e.g., `GIT_DESCRIBE` via a build script) — not the static
`Cargo.toml` value. This keeps the displayed version honest across all
build paths (including local ad-hoc builds with no tag).

Adds a top-of-Cargo.toml comment block pointing future contributors at
the SECURITY.md policy + revisit condition before they edit any
`version =` line.

Commit: **8259dd5**.

## Task D — Release-smoke rehearsal via workflow_dispatch ✓

Adds `workflow_dispatch:` alongside `push: tags: ['v*']` on both
release.yml and docker.yml. Gates the publish steps:

- `release.yml`: the `build` job (cross-compile + tarball + sha256 +
  upload to GitHub Releases) gates on
  `if: startsWith(github.ref, 'refs/tags/')`.
- `docker.yml`: the `docker` matrix job (buildx + push to ghcr.io)
  gates on the same condition.

The `check` jobs run on both event types, so a dispatch exercises the
full release-smoke gate (BLINDJOIN_REQUIRE_BITCOIND=1 via the composite
install-bitcoind action, then `cargo test --workspace --all-targets`,
then clippy and audit) without publishing anything.

### Rehearsal procedure (manual user action)

The infrastructure is in place; running it is a user action.

1. Open the Actions tab on github.com/johnzilla/blindjoin.
2. Pick **Release** (or **Docker**) from the workflow list.
3. Click **Run workflow**. Pick any branch (`main` is fine for the
   happy-path rehearsal).
4. Click the green **Run workflow** button.
5. Watch the run. Expected outcome on a healthy `main`:
   - **check** job: passes, including the composite
     install-bitcoind step + the `cargo test --workspace --all-targets`
     step. This is the gate.
   - **build** job: skipped — "This check was skipped". This is the
     gate working: no tag means no publish.
6. To prove the gate fails closed: push a branch with a deliberately
   broken integration test (e.g., assert `false` inside
   `tests/integration/full_round.rs`), dispatch the same workflow
   against that branch, and confirm the check job fails on the
   broken test.

Total wall-clock for a happy-path dispatch is roughly the same as the
existing CI run: cache-hit bitcoind (~0s setup) + cargo test
(~3-4 min on first run, ~1-2 min cached) + clippy + audit ≈ 5-10
minutes.

Commit: **ceca7b4**.

## Out of scope (deliberately not done)

- I did NOT actually run the workflow_dispatch rehearsal — that's a
  GitHub UI action that requires the user. The commit log + workflow
  YAML changes are everything I can contribute from the local checkout.
- I did NOT push a "deliberately broken integration test" branch.
  That's a destructive-ish action the user should decide whether to do.
- ci.yml still inlines its bitcoind setup steps — refactoring it to
  use the composite action is no-behavior-change and stays out of
  scope per the 260531-thw plan.

## Verification

- `cargo check --workspace` exits 0 (cargo.toml comment block parsed
  cleanly).
- All YAML files parse via `python3 -c 'import yaml; ...'`.
- `git log --oneline -3` shows 3 atomic commits: ed2f5a4, 8259dd5,
  ceca7b4.
- `grep -c workflow_dispatch .github/workflows/release.yml .github/workflows/docker.yml`
  shows the trigger landed in both files.

## Next

User runs the workflow_dispatch rehearsal (steps above) to close item #4
end-to-end. For v1.6 scoping, the carry-forward backlog is
`.planning/PROJECT.md` § Carry-Forward Items + the SECURITY.md § v1.6
supply-chain plan section.
