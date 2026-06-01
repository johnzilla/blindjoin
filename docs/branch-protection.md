# Branch Protection on `main`

`main` is protected by two GitHub Rulesets. Both are configured in the GitHub
Settings UI under **Settings → Rules → Rulesets** — GitHub does not allow
programmatic ruleset configuration from within a workflow run.

## Active rulesets

### `main` (id `17136873`) — v1.6 CODEOWNERS gate

The Phase 22 supply-chain gate. Targets `~DEFAULT_BRANCH` only.

| Rule | Setting |
|---|---|
| `pull_request` | `require_code_owner_review: true`, `required_approving_review_count: 1`, `dismiss_stale_reviews_on_push: true`, `require_last_push_approval: true` |
| `deletion` | restrict deletion of `main` |
| `non_fast_forward` | block force pushes |
| Bypass actors | `RepositoryRole admin (id 5), bypass_mode: always` |

When an outside contributor opens a PR touching [`docker/digests.txt`](../docker/digests.txt)
or [`.github/actions/read-base-digests/`](../.github/actions/read-base-digests/),
the [`.github/CODEOWNERS`](../.github/CODEOWNERS) rule requires the maintainer to
approve before merge. There is no way for the contributor to self-approve.

The admin bypass (`bypass_mode: always`) means the solo maintainer can push
directly to `main` and self-merge PRs without waiting for code-owner approval.
This is a deliberate trade-off documented in
[SECURITY.md §Supply-chain status](../SECURITY.md#supply-chain-status):
the gate exists to defend against unreviewed outside contributions, not to
slow the solo maintainer down. If blindjoin gains a second active maintainer,
the bypass actor should be removed and the gate re-enforced for everyone.

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
