---
phase: 22-base-image-digest-drift-detection
plan: 04
subsystem: infra
tags: [github-actions, supply-chain, docker, digest-drift, scheduled-workflow, idempotency, gh-cli]

# Dependency graph
requires:
  - phase: 22-base-image-digest-drift-detection
    provides: docker/digests.txt canonical manifest (Plan 22-01)
  - phase: 22-base-image-digest-drift-detection
    provides: read-base-digests composite action with named outputs debian_ref + cargo_chef_ref (Plan 22-02)
provides:
  - ".github/workflows/digest-drift-check.yml — scheduled workflow that runs daily at `0 9 * * *` UTC and on `workflow_dispatch`, resolves upstream digests via `docker buildx imagetools inspect --format '{{.Manifest.Digest}}'`, and opens `[digest-drift] <image>:<tag> moved to sha256:<HEX>` issues on drift"
  - "Idempotency gate keyed on upstream digest hex (Pitfall 9): `gh issue list --label digest-drift --state open --search '<UPSTREAM_HEX> in:title' --json number,title --jq '.[] | select(.title == \"<TITLE>\") | .number' | head -n1` — two different drifts of the same image:tag yield two different issues, but a repeated drift of the same hex is a no-op"
  - "Self-bootstrapping `digest-drift` GitHub label: `gh label create digest-drift --color fbca04 ... 2>/dev/null || true` runs on every workflow invocation; the label exists after the first run without manual repo setup"
  - "Issue-only output (Pitfall 11): workflow physically cannot open a PR — permissions block declares only `contents: read` + `issues: write`; auditor-grepable invariant verified `grep -q 'pull-requests:' .github/workflows/digest-drift-check.yml` returns exit 1"
  - "DRIFT-02 requirement satisfied (ROADMAP SC#2 + SC#4): scheduled-cron drift surfaces drift within 24 hours; manual rehearsal trigger usable from any branch before the first scheduled run"
affects:
  - "Plan 22-05 (SECURITY.md + CONTRIBUTING.md prose) — DRIFT-02 + DRIFT-03 are both now shipped; the prose half of D-05 can describe drift-check.yml as the daily DRIFT-02 surface (`[digest-drift]` issue title format, idempotency keying, triage steps per Pitfall 8)"
  - "Plan 22-06 (Human-UAT) — fresh-machine rehearsal of ROADMAP SC#2 part 1 (`gh workflow run digest-drift-check.yml --ref <branch>` against a deliberately-stale `docker/digests.txt` → expects 2 `[digest-drift]` issues opened) and ROADMAP SC#2 part 2 (re-run → expects no new issues opened, both rounds reference the same existing issues by number) is now exercisable end-to-end"
  - "Phase 23 (cosign + SLSA + SBOM) — the prose-comment-as-contract style for `permissions:` blocks (explicit `# Deliberately omitted scopes ...` enumeration) is reusable for the cosign workflow's `id-token: write` boundary documentation"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Scheduled workflow with `workflow_dispatch:` rehearsal trigger: `on:` block declares BOTH `schedule: - cron: '...'` AND `workflow_dispatch:` so the schedule can be rehearsed before the first cron fires (precedent: .planning/quick/260531-ubf-*/SUMMARY.md Task D)"
    - "Auditor-grepable deliberately-omitted-scopes comment style: `permissions:` block above-comment enumerates BOTH granted scopes AND deliberately-omitted scopes in prose, but uses paraphrase (`PR-write`, `packages`, `id-token`) rather than the literal YAML keys (`pull-requests:`, `packages:`, `id-token:`) so a `grep -q 'pull-requests:'` audit assertion returns exit 1 on the comment block too"
    - "GitHub label self-bootstrapping in workflow: `gh label create <label> --color <hex> 2>/dev/null || true` ensures the label exists on first run without requiring repo setup; subsequent runs are no-ops (the `2>/dev/null || true` masks the 'label already exists' error)"
    - "Idempotency keyed on upstream payload (not consumer-visible identifier): `gh issue list --search '<HEX> in:title'` + `--jq` exact-title post-filter — `gh search` is substring-based so the `--jq` belt-and-suspenders filter narrows to exact-title matches before treating an existing issue as a duplicate"
    - "Issue body composition via heredoc with explicit `cat <<EOF ... EOF` inside a bash function — keeps multi-line markdown with backticks, code fences, and table syntax legible while still inheriting shell-variable interpolation"

key-files:
  created:
    - .github/workflows/digest-drift-check.yml
  modified: []

key-decisions:
  - "Followed RESEARCH.md §4 lines 316-508 verbatim for the workflow shape. The locked structure (top-of-file comment block → env → on → permissions → jobs.drift-check) is the single source of truth for both DRIFT-02 implementation and the prose-comment-as-contract pattern that future workflows (cosign, reproducible-verify) will mirror"
  - "Rewrote the `permissions:` block comment to paraphrase deliberately-omitted scopes (`PR-write`, `packages`, `id-token`) rather than quote them as YAML keys (`pull-requests:`, `packages:`, `id-token:`). The PLAN's acceptance criteria run `! grep -q 'pull-requests:'` and `! grep -q 'id-token:'` as auditor-grepable invariants — those assertions are LINE-LEVEL not BLOCK-LEVEL, so even a comment that mentions the literal token would fail the invariant. Paraphrase preserves the auditor-readable intent (these scopes are deliberately omitted) while satisfying the grep gate"
  - "Used the inline title-format string `[digest-drift] ${IMAGE_TAG} moved to ${UPSTREAM_DIGEST}` verbatim from ROADMAP SC#2 lock + RESEARCH.md §2.2 line 92. The runtime expansion produces e.g. `[digest-drift] debian:bookworm-slim moved to sha256:abc...` which an auditor can grep against `[digest-drift]` in both the workflow file and any actual issue"
  - "Pinned `runs-on: ubuntu-24.04` (not `ubuntu-latest`). RESEARCH §Assumptions A4 declares `docker buildx imagetools` preinstalled on ubuntu-24.04; using `ubuntu-latest` would silently break the no-install-step invariant when GitHub rotates the latest tag (cf. Pitfall 7 for the reproducibility verifier in Phase 25)"
  - "Cron-collision check was clean (`grep -r 'schedule:' .github/workflows/` returned zero matches). Cron `0 9 * * *` locked at RESEARCH §2.3 with rationale (outside US-eastern business-hours Actions queue peak + outside maintainer review hours)"

patterns-established:
  - "Auditor-grepable deliberately-omitted-scopes pattern for `permissions:` blocks: paraphrase the omitted scope names in the comment so a literal `grep -q '<scope>:'` invariant assertion still returns exit 1. Reusable for Phase 23's `id-token: write` workflow (the cosign workflow will use this pattern in reverse — granting id-token: write but explicitly omitting pull-requests: and contents: write)"
  - "Self-bootstrapping GitHub label via `gh label create ... 2>/dev/null || true` inside a scheduled workflow — eliminates the manual repo setup step where an operator would otherwise have to run `gh label create digest-drift` once before the first drift event. Reusable for Phase 23 (`signing-anomaly` label?) and Phase 25 (`reproducibility-regression` label)"
  - "Idempotency via upstream-payload-keyed search + exact-title `--jq` post-filter — keys idempotency on the NEW payload (here: upstream digest hex) rather than the consumer-visible identifier (image tag) so the same identifier can have multiple in-flight issues for distinct drift events"

requirements-completed:
  - DRIFT-02

# Metrics
duration: ~7min
completed: 2026-06-01
---

# Phase 22 Plan 04: `digest-drift-check.yml` Scheduled Workflow Summary

**Daily scheduled GitHub Actions workflow that resolves upstream Docker registry digests via `docker buildx imagetools inspect`, diffs them against `docker/digests.txt`, and opens `[digest-drift] <image>:<tag> moved to sha256:<HEX>` issues (NOT PRs) with a Pitfall 9 idempotency gate keyed on the upstream digest hex — DRIFT-02 satisfied.**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-06-01T21:32:00Z (approx)
- **Completed:** 2026-06-01T21:39:00Z (approx)
- **Tasks:** 1
- **Files modified:** 1 (created)

## Accomplishments

- Created `.github/workflows/digest-drift-check.yml` (195 lines) — the locked-shape scheduled workflow per RESEARCH.md §4 lines 316-508.
- Triggers: `schedule: cron '0 9 * * *'` UTC (daily) + `workflow_dispatch:` (rehearsal from any branch).
- Reuses `./.github/actions/read-base-digests` from Plan 22-02 for parse (D-04 invariant — no separate parse code path).
- Upstream digest resolution via `docker buildx imagetools inspect "${IMAGE_TAG}" --format '{{.Manifest.Digest}}'` (zero-install on ubuntu-24.04).
- Pitfall 9 idempotency gate: `gh issue list --search '<UPSTREAM_HEX> in:title' --json number,title --jq '...'` keyed on upstream digest hex, not image tag.
- Pitfall 11 issue-only output: workflow physically cannot open a PR — `permissions:` block declares only `contents: read` + `issues: write`.
- Self-bootstrapping `digest-drift` label via `gh label create ... 2>/dev/null || true` on every run.
- Issue body composed per RESEARCH §2.2 lines 96-151: canonical vs upstream digest table, Docker Hub registry link, Pitfall 8 triage hint (low-severity retag vs substantive library/binary change), local diff command, resolution steps citing SECURITY.md §Supply-chain status, idempotency call-out.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create `.github/workflows/digest-drift-check.yml` scheduled workflow** — `fae501f` (feat)

**Plan metadata:** (this commit — `docs(22-04): complete digest-drift-check.yml scheduled workflow plan`)

## Files Created/Modified

- `.github/workflows/digest-drift-check.yml` — Scheduled drift-check workflow. Reuses the composite action for parse, resolves upstream digests via `docker buildx imagetools inspect`, opens `[digest-drift]` issues with Pitfall 9 idempotency. Minimum-privilege permissions (`contents: read` + `issues: write` only).

## Locked Idempotency Command (for Plan 22-06 UAT rehearsal)

Plan 22-06 (Human-UAT) will rehearse the idempotency contract end-to-end. The exact `gh issue list` command shape used by the workflow at line 99-105 is:

```bash
gh issue list \
  --label digest-drift \
  --state open \
  --search "${UPSTREAM_HEX} in:title" \
  --json number,title \
  --jq '.[] | select(.title == "'"${TITLE}"'") | .number' \
  | head -n1
```

Where:

- `UPSTREAM_HEX` is the 64-hex `<HEX>` portion of the new upstream `sha256:<HEX>` (the `${UPSTREAM_DIGEST#sha256:}` parameter expansion).
- `TITLE` is `[digest-drift] ${IMAGE_TAG} moved to ${UPSTREAM_DIGEST}` (e.g. `[digest-drift] debian:bookworm-slim moved to sha256:abc...`).

UAT rehearsal procedure (deliberately-stale-digest test, RESEARCH.md §8 lines 845-878, full procedure in Plan 22-06):

1. On a feature branch, edit `docker/digests.txt` to set the `debian:bookworm-slim` line to a known-stale digest (e.g. an older Docker Hub tag's hex).
2. `gh workflow run digest-drift-check.yml --ref <branch>` → expects 1 new `[digest-drift] debian:bookworm-slim moved to sha256:<CURRENT_HEX>` issue opened.
3. `gh workflow run digest-drift-check.yml --ref <branch>` (second run, no other changes) → expects 0 new issues (the existing one is found by the `--search "${UPSTREAM_HEX} in:title"` gate and skipped).
4. Restore `docker/digests.txt` to the canonical pinned values.

## Locked Title Format (ROADMAP SC#2 lock — for Plan 22-06 UAT rehearsal)

```
[digest-drift] <image>:<tag> moved to sha256:<HEX>
```

Example (debian drift):

```
[digest-drift] debian:bookworm-slim moved to sha256:4f7c8e9d6b3a2c5e8f1d9b6c4a7e2f9d8b3c6e5a8f1d4b9c7e2a6f3d8c5b9e4
```

Example (cargo-chef drift):

```
[digest-drift] lukemathwalker/cargo-chef:latest-rust-1 moved to sha256:9b6c4a7e2f9d8b3c6e5a8f1d4b9c7e2a6f3d8c5b9e44f7c8e9d6b3a2c5e8f1d9
```

This format is LOCKED at ROADMAP SC#2 (`docs: define milestone v1.6 requirements` commit `897a7d4`) + RESEARCH.md §2.2 line 92. Plan 06 UAT rehearses against this exact shape.

## Decisions Made

See frontmatter `key-decisions` for the full list. Headline decisions:

1. **Followed RESEARCH.md §4 verbatim** — the locked YAML shape is the single source of truth for both this workflow and the prose-comment-as-contract pattern future supply-chain workflows will mirror.
2. **Paraphrased deliberately-omitted-scope names in the `permissions:` comment** (`PR-write`, `packages`, `id-token`) rather than quoting their YAML keys — so `grep -q 'pull-requests:'` and `grep -q 'id-token:'` audit assertions return exit 1 at the file level too, not just the runtime permission gate.
3. **Pinned `runs-on: ubuntu-24.04`** (not `ubuntu-latest`) — locks RESEARCH §Assumptions A4 (`docker buildx imagetools` preinstalled).
4. **Cron-collision check was clean** — `grep -r 'schedule:' .github/workflows/` returns zero matches, so `0 9 * * *` UTC daily is uncontested.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Auditor-grepable invariant for deliberately-omitted permission scopes**

- **Found during:** Task 1 (workflow file creation)
- **Issue:** The first draft of the `permissions:` block comment contained the literal strings ``No `packages:` (we don't push anything). No `id-token:` (no cosign here).`` and ``No `pull-requests:` (PITFALLS.md §11 — issues only, never PRs).`` Those comment lines literally contain the YAML keys `id-token:` and `pull-requests:`. The PLAN's acceptance criteria run `! grep -q 'pull-requests:' .github/workflows/digest-drift-check.yml` and `! grep -q 'id-token:' .github/workflows/digest-drift-check.yml` as line-level auditor-grepable invariants — those assertions fail when the comment quotes the literal key, even though the runtime permission gate is fine. This is an auditor-correctness gap: a future audit script that greps for these tokens to detect over-privileged workflows would false-positive on this workflow's comment block.
- **Fix:** Rewrote the comment to paraphrase the deliberately-omitted scopes (`PR-write`, `packages`, `id-token` without the literal `:` suffix on `pull-requests` and `id-token`). The intent (these scopes are deliberately omitted) is preserved for human readers; the auditor-grepable invariant `! grep -q 'pull-requests:'` + `! grep -q 'id-token:'` is satisfied at the line level. `packages` is fine to mention because the line-level check is `! grep -E '^[[:space:]]+packages:'` (rejects only indented `packages:` keys, not bare comments).
- **Files modified:** `.github/workflows/digest-drift-check.yml` (comment block above `permissions:`, lines 39-44)
- **Verification:** `grep -q 'pull-requests:'` exits 1 ✓; `grep -q 'id-token:'` exits 1 ✓; `grep -E '^[[:space:]]+packages:'` exits 1 ✓; semantic intent preserved in the rewritten comment.
- **Committed in:** `fae501f` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 Rule 2 — missing critical correctness)
**Impact on plan:** Necessary for auditor-grepable correctness; no scope creep. The plan's negative-assertion gates would have rejected the first draft; the fix aligns the comment block with the same line-level invariants the rest of the verification suite checks.

## Issues Encountered

None — the locked YAML shape in RESEARCH.md §4 was complete and unambiguous. The single auto-fix above was a correctness alignment between the comment block and the auditor-grepable invariants, not an issue with the workflow's runtime behavior.

## Threat-Model Compliance

All STRIDE entries from the plan threat model satisfied:

- **T-22-12** (Spoofing — workflow tampering to add `gh pr create`): Top-level `permissions:` declares only `contents: read` + `issues: write`. Even if shell logic were modified to invoke `gh pr create`, the platform-level permission gate blocks the call. Residual carry-forward to Phase 23: extend CODEOWNERS to cover `.github/workflows/digest-drift-check.yml`.
- **T-22-13** (Tampering — `GH_TOKEN` misuse outside declared scope): Same minimum-privilege gate. Token cannot perform `gh pr create`, `gh release create`, or any code push.
- **T-22-14** (Info Disclosure — secret leakage in issue body): Issue body contains only public information (image tags, digest hexes, registry URLs, workflow run URLs). Accepted as zero-risk.
- **T-22-15** (DoS — issue spam from manifest-race): Idempotency keyed on upstream hex closes the duplicate-issue spam vector. Accepted (race window = time between manifest merge and next-day cron).
- **T-22-16** (DNS/registry MITM → false-positive issue): Standard TLS via runner's CA store; false positive observable on triage. Accepted.
- **T-22-17** (Docker Hub rate-limit on inspect): 2 calls/day from a GitHub Actions IP, far below the anonymous-pull limit. Accepted-with-action (add Docker Hub auth if it ever becomes a problem; not a Phase 22 concern).
- **T-22-SC** (npm/pip/cargo installs): N/A — no package installs in this workflow. Uses only `gh` (preinstalled), `docker buildx imagetools` (preinstalled on ubuntu-24.04), and `bash`. SHA-pinned `actions/checkout` reused at the project's existing pin.

## User Setup Required

None — no external service configuration required. The `digest-drift` GitHub label is auto-created by the workflow itself on first run; no manual repo setup needed.

## Next Phase Readiness

- **DRIFT-02 satisfied.** ROADMAP SC#2 (`[digest-drift]` issue opens on drift; second run skips via idempotency) and SC#4 (`schedule:` + `workflow_dispatch:` triggers present) are both implementable end-to-end as of this commit.
- **Ready for Plan 22-05** (SECURITY.md + CONTRIBUTING.md prose) — DRIFT-02 + DRIFT-03 are both shipped, so the prose half of D-05 can describe the drift-check workflow as the daily DRIFT-02 surface with the locked title format and idempotency contract as documented invariants.
- **Ready for Plan 22-06** (Human-UAT) — the deliberately-stale-digest rehearsal procedure from RESEARCH.md §8 lines 845-878 is now exercisable. The two-rounds-no-duplicate test rehearses ROADMAP SC#2 part 1 + part 2 end-to-end.
- **No blockers.** Phase 22 has one plan remaining (22-05) before HUMAN-UAT (22-06).

## Self-Check: PASSED

- `.github/workflows/digest-drift-check.yml` exists (verified `[ -f ... ]`).
- `.planning/phases/22-base-image-digest-drift-detection/22-04-SUMMARY.md` exists (this file).
- Commit `fae501f` exists in `git log --oneline --all`.
- YAML validity: `python3 -c 'import yaml; yaml.safe_load(open(...))'` exit 0.
- All 13+ positive acceptance assertions from PLAN pass.
- All 4 negative acceptance assertions from PLAN pass (no `pull-requests:`, no `id-token:`, no indented `packages:`, no `gh pr create`).

---
*Phase: 22-base-image-digest-drift-detection*
*Completed: 2026-06-01*
