# Branch Protection on `main`

`main` is protected by two GitHub Rulesets, configured in the GitHub Settings
UI under **Settings → Rules → Rulesets** (GitHub does not allow programmatic
ruleset configuration from within a workflow run).

## Active rulesets

### `main` (id `28125482`)

Targets `~DEFAULT_BRANCH`. **`bypass_actors: []`** — there is no bypass, so
every rule below applies to everyone, the solo maintainer included.

| Rule | Setting |
|---|---|
| `required_status_checks` | **strict** (require branches to be up to date before merging). Required contexts: `cargo test`, `cargo clippy`, `cargo audit`, `coordinator binary builds`, `corepc-node feature pin check`, `bip322 exact-version pin check` |
| `deletion` | block deletion of `main` |
| `non_fast_forward` | block force pushes |

Effect: a PR into `main` cannot merge until **all six checks pass AND the branch
is up to date** with `main`. If `main` moves while a PR is open, GitHub shows an
**Update branch** button and re-runs CI before the PR can merge — this is what
prevents a stale-base merge (a PR green on an older base landing a broken tree).
No one, the owner included, can force-push or delete `main`.

### `main-default` (id `15456374`) — baseline

Targets `~DEFAULT_BRANCH`. Rules: `deletion` and `non_fast_forward`. Empty
bypass list. Its two rules overlap `main`'s, so it is effectively redundant —
kept as a harmless baseline guard.

## No outside-PR review gate — deliberate

The ruleset intentionally has **no `pull_request` rule** (no
`required_approving_review_count`). This is a recorded decision for a
solo-maintainer repo, not an oversight:

- The human gate is the maintainer's **merge click**; the automated gate is the
  **six required checks + up-to-date** requirement above.
- A required 1-approval rule would be counterproductive here: with
  `bypass_actors` empty, GitHub blocks the sole maintainer from merging their
  own PRs (you cannot approve your own PR), and adding an `admin` bypass to work
  around that would make the rule a no-op for the only person who merges.
- **Revisit if blindjoin gains a second active maintainer** and starts taking
  outside / fork PRs: add a `pull_request` rule
  (`required_approving_review_count: 1`, `dismiss_stale_reviews_on_push: true`,
  `require_last_push_approval: true`) and decide on a bypass actor at that point.

There is no `CODEOWNERS` file and no `require_code_owner_review` rule; an earlier
code-owner gate on base-image digests became moot once digests moved into
`docker/Dockerfile`'s `FROM` lines directly.

> Historical note: an earlier `main` ruleset (since deleted, id `17136873`)
> carried a 1-approval `pull_request` rule plus an `admin … always` bypass and
> did **not** require CI checks. The current ruleset inverts that: CI checks are
> required and enforced for everyone, and the review rule is dropped per the
> decision above.

## CI status checks (required)

`.github/workflows/ci.yml` runs the six jobs listed in the `main` ruleset table;
all six are required contexts, with strict up-to-date enforcement. Status-check
names come from each job's `name:` field in `ci.yml` and only appear in the
ruleset picker after at least one CI run has reported them (add each as a
**separate** check — pasting a comma-joined string registers one phantom check
that never reports and blocks all merges). The `check` jobs in `release.yml`
and `docker.yml`, and the scheduled `cargo audit` in `audit-cron.yml`, are
separate from the PR checks and are not required for merge — `audit-cron.yml`
is a between-releases tripwire (weekly) that re-runs the gating audit against the
committed `Cargo.lock`, not a merge gate.

## Verifying the configuration

```bash
gh api repos/johnzilla/blindjoin/rulesets \
  --jq '.[] | {id, name, enforcement, target}'

gh api repos/johnzilla/blindjoin/rulesets/28125482 \
  --jq '{name, enforcement, bypass_actors, rules: [.rules[] | {type, parameters}]}'
```
