# Phase 22: Base-Image Digest Drift Detection - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-01
**Phase:** 22-base-image-digest-drift-detection
**Areas discussed:** Glue placement

---

## Gray-Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Glue placement | Where the 'read digests.txt → emit `--build-arg`' logic lives | ✓ |
| Drift tool & resolve path | `docker buildx imagetools inspect` vs `crane` vs `skopeo` | |
| Issue body shape | Body content + label + Pitfall 8 triage hints | |
| Manifest-missing fail mode | Hard fail vs fall back vs CI lint on PRs | |

**User's choice:** Glue placement only. Other three deferred to planner discretion (constrained by research/PITFALLS.md and the structural locks decided inside Glue placement).
**Notes:** Manifest-missing fail mode was partly absorbed into D-03 (fail-fast inside the composite action).

---

## Glue placement — Q1: Where does the logic live?

| Option | Description | Selected |
|--------|-------------|----------|
| Composite action | New `.github/actions/read-base-digests/action.yml`, mirrors `install-bitcoind` | ✓ |
| Inline bash step | Per-workflow shell that greps digests.txt and sets GITHUB_OUTPUT | |
| Shared `scripts/*.sh` | Locally invokable; breaks from composite-action precedent | |

**User's choice:** Composite action.
**Notes:** Locks `D-01`. Consistent project shape — `.github/actions/install-bitcoind/` is the precedent. Reusable by `digest-drift-check.yml` for its parse side. Phase 23/24 will likely reuse the pattern when wiring cosign installers.

---

## Glue placement — Q2: Composite action surface

| Option | Description | Selected |
|--------|-------------|----------|
| Named per-image outputs | `outputs.debian_ref`, `outputs.cargo_chef_ref` — hardcoded to the 2 named images | ✓ |
| Generic key/value JSON output | Single `digests_json` map, callers `fromJSON()` | |
| Parse + Resolve in one action | Composite action does BOTH parse and upstream re-resolve | |

**User's choice:** Named per-image outputs.
**Notes:** Locks `D-02` and `D-04`. Simplest call-site shape. Renaming/adding a base image becomes intentional friction (Pitfall 11 — base-image changes deserve human scrutiny). Resolve logic explicitly stays out of the composite action — lives in `digest-drift-check.yml` only.

---

## Glue placement — Q3: Strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Fail-fast inside the action | Validates file + line shape + image set; `exit 1` on any deviation | ✓ |
| Best-effort + caller asserts | Returns empty outputs on missing/malformed; each caller decides | |
| Fail-fast + CI lint on PRs | Same as option 1 PLUS ci.yml lint job on every PR | |

**User's choice:** Fail-fast inside the action.
**Notes:** Locks `D-03`. Supply-chain guarantee is structurally load-bearing inside the action — release/docker workflows cannot accidentally publish a tag without a valid manifest. Extra PR-time lint considered and deferred (overkill given CODEOWNERS gate from Q4).

---

## Glue placement — Q4: Enforcement of "human-reviewed PR only" (DRIFT-01 acceptance)

| Option | Description | Selected |
|--------|-------------|----------|
| Prose only | Document policy in SECURITY.md + CONTRIBUTING.md, no structural gate | |
| Prose + CODEOWNERS | Same docs PLUS `.github/CODEOWNERS` mapping `docker/digests.txt` to maintainer | ✓ |
| Prose + CODEOWNERS + label-block rule | All of the above PLUS digest-policy-check.yml label-gate workflow | |

**User's choice:** Prose + CODEOWNERS.
**Notes:** Locks `D-05`. CODEOWNERS approval is the structural mechanism that auto-merge bots cannot bypass. Label-gate workflow rejected as belt-suspenders-zipper overkill for a solo-maintained repo. CODEOWNERS scope intentionally covers BOTH `docker/digests.txt` AND `.github/actions/read-base-digests/**` — changing the parser logic is functionally the same supply-chain risk as changing a digest.

---

## Glue placement — Q5: Wrap or continue?

| Option | Description | Selected |
|--------|-------------|----------|
| Wrap to CONTEXT.md | Locked decisions sufficient for researcher + planner | ✓ |
| Drift tool & resolve path | Pick `docker buildx imagetools inspect` vs `crane` vs `skopeo` | |
| Issue body shape | Define body + label + Pitfall 8 hints | |

**User's choice:** Wrap to CONTEXT.md.
**Notes:** Remaining areas are tight enough to defer to planner discretion. Research/PITFALLS.md §§ 8, 9, 11, 12 and Success Criteria #2 already constrain them.

---

## Claude's Discretion

These areas were intentionally NOT pinned — planner picks during PLAN.md, constrained by research/PITFALLS.md and ROADMAP success criteria:

- **Drift-tool choice** (`docker buildx imagetools inspect` vs `crane digest` vs `skopeo inspect`) for the resolve step in `digest-drift-check.yml`. Favor zero-install if shell parsing is tractable.
- **Issue body shape** — title format and idempotency are locked; body content, label name (`digest-drift`), Pitfall 8 retag-vs-substantive triage hints, auto-assign behavior are planner discretion.
- **Cron schedule** — DRIFT-02 calls for "daily"; pick a UTC time outside other scheduled work and rehearse via `workflow_dispatch` before merging.
- **Rehearsal documentation** — follow the precedent in `.planning/quick/260531-ubf-*/SUMMARY.md`; document the `workflow_dispatch` test path so the daily workflow can be hand-fired pre-merge.

## Deferred Ideas

- **CI lint on PRs touching `docker/digests.txt`** — covered by composite-action fail-fast + CODEOWNERS approval; carry-forward candidate if contributor patterns change.
- **Generic key/value JSON output** for the composite action — Phase 22 doesn't need extensibility; revisit only if Phase 25 grows base-image count.
- **`digest-policy-check.yml` label-gate workflow** — overkill for solo-maintained; carry-forward candidate if additional maintainers join.
- **Drift severity classification (Pitfall 8 retag-vs-substantive)** — REQUIREMENTS.md §Future defers to v1.7+. Phase 22 may surface enough signal in the issue body to make this unnecessary; otherwise carry-forward.
</content>
</invoke>