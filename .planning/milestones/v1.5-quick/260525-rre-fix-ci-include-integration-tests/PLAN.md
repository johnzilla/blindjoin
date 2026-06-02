---
quick_id: 260525-rre
slug: fix-ci-include-integration-tests
workstream: fix-verification-gap
created: 2026-05-25
status: in-progress
---

# Fix CI to actually exercise integration tests

## Why
v1.1's CI only ran `cargo test --workspace --lib` — library tests only. This excluded the entire `tests/` directory. As a direct consequence, the round-bootstrap regression (workstream `fix-round-bootstrap`) shipped to v1.1 unnoticed because the integration test that would have caught it could not exist in the current CI command. Even after writing `tests/integration/round_bootstrap.rs` in the fix, it still won't run on CI until this change lands.

CI also only triggers on `pull_request` and `workflow_dispatch` — not push-to-main. The round-bootstrap fix pushed 4 commits to main and did NOT trigger CI; required manual `gh workflow run`.

## Scope (this task only — backdoor inventory and verification-template heuristic are separate follow-on phases)

### Change 1: trigger surface + test command
Edit `.github/workflows/ci.yml`:
- Add `push: branches: [main]` to the `on:` triggers (keep existing `pull_request` and `workflow_dispatch`).
- Change the `cargo test` step command from `cargo test --workspace --lib` to `cargo test --workspace --all-targets`.

### Change 2: coordinator-binary smoke job
Add a new job to `ci.yml` that proves the production binary actually boots without panicking:
- Job name: `coordinator-smoke` (or similar — match existing style).
- Steps: checkout → install Rust toolchain → cache → `cargo run --bin coordinator -- --help`.
- **Binary name confirmed:** `coordinator` (not `blindjoin-coordinator`). From `coordinator/Cargo.toml`.
- Do NOT attempt to bring up bitcoind in CI in this task — that's a much bigger change and belongs in a follow-on phase. The `--help` smoke is the minimal "binary builds + boots far enough to parse args" check.

### Change 3 (bonus, defer if non-trivial): actions/checkout SHA bump
Current: `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5` (v4.3.1, Node 20, deprecated June 2026).
- If a newer SHA can be confirmed quickly (e.g. via `gh api repos/actions/checkout/releases/latest`), bump. Otherwise leave a TODO and defer.

## Commits (atomic)
1. `ci: include integration tests and main-branch pushes in CI`
2. `ci: add coordinator binary smoke job`
3. (optional) `ci: bump actions/checkout to current v4.x SHA`

## Validation
- Push to feature branch `fix-ci-include-integration-tests` (NOT main).
- Open PR to main. PR-trigger will execute the new CI.
- Confirm on the PR's CI run:
  - The new `round_bootstrap` integration test executes and passes (or surfaces a clean failure that tells us bitcoind is needed — that's useful signal too).
  - The `coordinator-smoke` job passes.
  - Existing tests, clippy, audit still pass.
- Do not merge until CI is green on the PR.

## Constraints
- Don't touch `.planning/` git history.
- Atomic commits.
- No backdoor-inventory work or verification-template work in this task.

## Expected output
- PR URL.
- Confirmation that `round_bootstrap` integration test ran on PR CI (pass or specific blocker).
- Note on whether SHA bump landed or deferred.
