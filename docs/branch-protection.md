# Branch Protection on `main`

`main` is protected by two GitHub Rulesets. Both are configured in the GitHub
Settings UI under **Settings → Rules → Rulesets** — GitHub does not allow
programmatic ruleset configuration from within a workflow run.

## Active rulesets

### `main` (id `17136873`) — outside-PR approval gate

Targets `~DEFAULT_BRANCH` only.

| Rule | Setting |
|---|---|
| `pull_request` | `required_approving_review_count: 1`, `dismiss_stale_reviews_on_push: true`, `require_last_push_approval: true` |
| `deletion` | restrict deletion of `main` |
| `non_fast_forward` | block force pushes |
| Bypass actors | `RepositoryRole admin (id 5), bypass_mode: always` |

The `required_approving_review_count: 1` rule means an outside contributor (PR from a fork) cannot merge without the maintainer's approval. The admin bypass lets the solo maintainer push directly to `main` and self-merge without waiting on their own approval. If blindjoin ever gains a second active maintainer, drop the bypass and the gate enforces for everyone.

The previous `require_code_owner_review: true` setting + `CODEOWNERS` file gated specific paths (`docker/digests.txt` + the read-base-digests composite action). Both were removed when base-image digests moved into `docker/Dockerfile`'s `FROM` lines directly. The setting can be toggled off in the GitHub UI; with no `CODEOWNERS` file it has nothing to evaluate and is a no-op either way.

### `main-default` (id `15456374`) — baseline protection

Applies to all branches (`~ALL`). Two rules: `deletion` and `non_fast_forward`.
Empty bypass list. Just guards against accidental branch deletion and force
pushes anywhere in the repo.

## CI status checks

`ci.yml` runs `cargo test`, `cargo clippy`, and `cargo audit` on every PR.
These are **not** currently configured as required status checks in the
ruleset above — they're advisory. To make them required:

1. Open **Settings → Rules → Rulesets → main (id `17136873`)**
2. Add a `required_status_checks` rule with these contexts:
   - `cargo test`
   - `cargo clippy`
   - `cargo audit`
3. Status check names come from the `name:` field of each job in
   `.github/workflows/ci.yml` and only appear in the picker after at least one
   CI run has completed.

The `check` jobs in `release.yml` and `docker.yml` are separate from these
PR checks and do not need to be added.

## Verifying the configuration

```bash
gh api repos/johnzilla/blindjoin/rulesets \
  --jq '.[] | {id, name, enforcement, target}'

gh api repos/johnzilla/blindjoin/rulesets/17136873 \
  --jq '{name, enforcement, bypass_actors, rules: [.rules[] | {type, parameters}]}'
```
