# Enabling Branch Protection for CI Gates

After merging the CI workflow (`.github/workflows/ci.yml`), configure GitHub branch
protection to make the status checks required. This is a one-time manual step —
GitHub does not allow programmatic branch protection from within a workflow run.

## Why This Is Necessary

The `ci.yml` workflow runs `cargo test`, `cargo clippy`, and `cargo audit` on every
pull request. Without branch protection rules, the checks run but do not block merging.
Required status checks make the CI gate mandatory.

## Setup Steps

1. Go to **Settings** → **Branches** in this repository on GitHub.
2. Under **Branch protection rules**, click **Add rule** (or edit the existing `main` rule).
3. In the **Branch name pattern** field, enter: `main`
4. Check **Require status checks to pass before merging**.
5. Check **Require branches to be up to date before merging**.
6. In the status check search box, add the following checks (they appear after at least one
   CI run has completed):
   - `cargo test`
   - `cargo clippy`
   - `cargo audit`
7. Optionally check **Require a pull request before merging** if you want reviews enforced too.
8. Click **Save changes**.

## Status Check Names

These names come from the `name:` field of each job in `.github/workflows/ci.yml`:

| Status Check | CI Job | What It Enforces |
|---|---|---|
| `cargo test` | `test` | `cargo test --workspace` must pass |
| `cargo clippy` | `clippy` | `cargo clippy --workspace -- -D warnings` must pass |
| `cargo audit` | `audit` | No high or critical CVEs in dependency tree |

## Notes

- Status checks only appear in the GitHub search box after the workflow has run at least once.
  Open a draft PR to trigger the first run before configuring protection.
- The `check` jobs in `release.yml` and `docker.yml` are separate from these PR checks
  and do not need to be added to branch protection rules.
