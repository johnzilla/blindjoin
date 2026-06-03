---
phase: 22-base-image-digest-drift-detection
plan: 02
subsystem: infra
tags: [github-actions, composite-action, supply-chain, docker, digest-pinning, bash]

# Dependency graph
requires:
  - phase: 22-base-image-digest-drift-detection
    provides: docker/digests.txt canonical manifest (Plan 22-01)
provides:
  - "Composite action `.github/actions/read-base-digests/` that parses `docker/digests.txt` and emits named outputs `debian_ref` + `cargo_chef_ref` in `image:tag@sha256:HEX` form"
  - "Structural supply-chain gate: a tag push cannot publish artifacts unless the manifest is present and well-formed (5-gate D-03 fail-fast contract)"
  - "Single source-of-truth for parse semantics — release.yml, docker.yml, and digest-drift-check.yml all consume the same composite action"
affects:
  - "Plan 22-03 (release.yml + docker.yml integration — consumes both named outputs via `${{ steps.digests.outputs.debian_ref }}` / `cargo_chef_ref`)"
  - "Plan 22-04 (digest-drift-check.yml — uses this action to parse the canonical list, then does upstream resolution separately per D-04)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composite action with `inputs: {}` + named `outputs:` block whose `value:` references a single composite step's `id`"
    - "Auditor-grepable error trailer (`Refusing to build without a valid manifest.`) inlined per error path — `grep -c` over the file finds every failure mode"
    - "Parse-only composite action separates parse from upstream resolve (D-04) — keeps release/docker workflows free of network calls"

key-files:
  created:
    - .github/actions/read-base-digests/action.yml
  modified: []

key-decisions:
  - "Inlined the full `Refusing to build without a valid manifest.` sentence in each error `echo` (rather than using a `POLICY_REF` shell variable) — the auditor-grepable acceptance criterion `grep -c >= 4` counts matching LINES, not interpolated runtime expansions"
  - "Mirrored install-bitcoind's `name:` + folded-scalar `description:` shape verbatim, swapping the action-specific content; reused the `Composite source-of-truth for v1.6+:` closer phrase to keep the audit-trail style consistent across composite actions"
  - "Action emits outputs via `>> \"${GITHUB_OUTPUT}\"` (modern form), NOT the deprecated `::set-output` syntax"

patterns-established:
  - "Composite action error-path style: inline `|| { echo \"supply-chain: ...\" >&2; exit 1; }` with auditor-facing prefix + full policy-reference sentence per error (mirrors install-bitcoind's `|| { echo \"ERROR: ...\"; exit 1; }` shape, swapping the prefix)"
  - "Composite action description: folded-scalar `>` block with WHAT-it-does paragraph, numbered fail-fast contract, then `Composite source-of-truth for v1.6+:` closer naming every consuming workflow"
  - "Named outputs declared at top-level `outputs:` block; the single composite step has `id:` matching the `outputs.value: ${{ steps.<id>.outputs.<name> }}` references"

requirements-completed:
  - DRIFT-01

# Metrics
duration: ~5min
completed: 2026-06-01
---

# Phase 22 Plan 02: Read-Base-Digests Composite Action Summary

**Composite action at `.github/actions/read-base-digests/` parses `docker/digests.txt` with a 5-gate fail-fast contract and emits `debian_ref` + `cargo_chef_ref` named outputs for release/docker/drift-check workflow consumption.**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-06-01T21:20:34Z (Plan 22-01 close per STATE.md)
- **Completed:** 2026-06-01T21:24:09Z
- **Tasks:** 1
- **Files created:** 1

## Accomplishments

- Created the single source-of-truth parser action at `.github/actions/read-base-digests/action.yml` (109 lines) — three v1.6 workflows (release.yml, docker.yml, digest-drift-check.yml) will all `uses: ./.github/actions/read-base-digests` per D-01, so parse semantics cannot drift between them.
- Implemented all five D-03 supply-chain gates inside one composite shell step (file-exists, line-count==2, per-line regex `^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$`, `debian:` prefix, `lukemathwalker/cargo-chef:` prefix); each gate emits the auditor-grepable `Refusing to build without a valid manifest.` trailer on stderr before `exit 1`.
- Held D-04 parse-only invariant strictly: zero network-call tooling (no `curl`, `docker buildx`, `gh issue`, `gh label`, `wget`). Upstream resolution belongs in Plan 22-04's `digest-drift-check.yml`.
- Verified the action's shell logic accepts the canonical (sentinel-digest) manifest from Plan 22-01 and correctly rejects an empty manifest in negative-path smoke tests.

## Task Commits

1. **Task 1: Create `.github/actions/read-base-digests/action.yml` composite action** — `d184845` (feat)

_No plan-metadata commit yet — happens as the final commit after this SUMMARY + STATE/ROADMAP updates._

## Files Created/Modified

- `.github/actions/read-base-digests/action.yml` — composite action; mirrors `.github/actions/install-bitcoind/` structure with three additions: top-level `outputs:` block (install-bitcoind exports via `$GITHUB_ENV`), empty `inputs: {}` declaration, and the auditor-facing `supply-chain: ...` error prefix (vs install-bitcoind's bare `ERROR:`).

## Locked Interface (consumed by Plans 22-03 and 22-04)

| Output name      | Type   | Value shape                                                          | Source                                  |
|------------------|--------|----------------------------------------------------------------------|-----------------------------------------|
| `debian_ref`     | string | `debian:bookworm-slim@sha256:<64-hex>`                               | `docker/digests.txt` line matching `^debian:` |
| `cargo_chef_ref` | string | `lukemathwalker/cargo-chef:latest-rust-1@sha256:<64-hex>`            | `docker/digests.txt` line matching `^lukemathwalker/cargo-chef:` |

**Inputs:** none (`inputs: {}` — D-02: image list hardcoded; future third image is intentional friction).
**Network calls:** none (D-04 parse-only contract).
**Side effects:** reads `docker/digests.txt` from checkout root; writes to `$GITHUB_OUTPUT`; emits 7 audit-trail lines on stdout on success or 1 `supply-chain: ...` error line on stderr on each failure mode.

## Decisions Made

- **Inlined `Refusing to build without a valid manifest.` per error path** instead of using a `POLICY_REF` shell variable. RESEARCH.md §3 originally factored the trailing sentence through `${POLICY_REF}`, but the plan's acceptance criterion `[ "$(grep -c 'Refusing to build without a valid manifest' ...)" -ge 4 ]` counts matching FILE LINES (not runtime expansions of an interpolated var). Inlining the literal sentence in each of the 4 `echo` calls satisfies the auditor-grepable contract at the file level. This is a faithful implementation of RESEARCH.md §2.4's intent ("auditors grepping logs for `Refusing to build without a valid manifest` find every failure mode at one search hit") — the change is to put that property at the file level too, not just the runtime-log level. Final count: 7 matching lines (4 in error echos + 2 in the comment justifying the inlining + 1 in the `description:` block).
- **Followed the install-bitcoind structural mirror exactly** for everything else: `name:` is a short imperative phrase ("Read base-image digests" ↔ "Install pinned bitcoind"); `description:` is a `>` folded-scalar multi-paragraph block; the `Composite source-of-truth for v1.6+:` closer phrase is reused verbatim and names the three consuming workflows; the composite step uses `name:` + `id:` + `shell: bash` + `run: |`; `set -euo pipefail` is at the top of the `run:` block; each guard is inline `if [ ! ... ]; then echo ... >&2; exit 1; fi` style.

## Deviations from Plan

None - plan executed exactly as written.

(The "inlined error sentence vs `POLICY_REF` variable" choice above was an implementation decision required to satisfy the plan's `<acceptance_criteria>` literally — the plan's acceptance criterion explicitly required `grep -c 'Refusing to build without a valid manifest' >= 4` at the file level, which forced inlining over variable interpolation. This is a faithful execution of the plan's contract, not a deviation from it.)

## Issues Encountered

- **`grep -c` semantics on interpolated shell variables (caught at first acceptance-criterion run).** Initial draft used `POLICY_REF="See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest."` then `echo "supply-chain: ... ${POLICY_REF}" >&2` for each error. `grep -c` reported only 1 match (the `POLICY_REF=` line itself) because the four error `echo`s reference the variable by name, not by literal content. Resolved by inlining the full trailing sentence into each error `echo` — `grep -c` now reports 7 matches (well above the >=4 floor), and the runtime behavior is unchanged (the same string is still emitted on stderr at each failure mode).

## User Setup Required

None - no external service configuration required for this plan. The action is consumed by workflows (release.yml, docker.yml, digest-drift-check.yml) that are wired up in Plans 22-03 and 22-04; CODEOWNERS branch-protection setup is the only operator-side step, and it's Plan 22-05's HUMAN-UAT.

## Verification Results

All 11 acceptance criteria PASS:

1. File exists at `.github/actions/read-base-digests/action.yml`
2. Valid YAML (`python3 -c 'import yaml; yaml.safe_load(...)'` exit 0)
3. `name: Read base-image digests` matches
4. `using: composite` declared
5. Both `debian_ref:` and `cargo_chef_ref:` declared as outputs
6. Regex contract `[a-f0-9]{64}` present
7. Auditor-grepable error trailer count: 7 (>= 4 required)
8. `$GITHUB_OUTPUT` modern form used
9. Deprecated `::set-output` NOT used
10. Zero network-call tooling (`curl|docker buildx|gh issue|gh label`) — D-04 parse-only invariant holds
11. `Composite source-of-truth` closer phrase present

Smoke tests:

- **Positive path:** action shell logic against the canonical `docker/digests.txt` (with sentinel digests from Plan 22-01) → emits both `debian_ref=debian:bookworm-slim@sha256:000...000` and `cargo_chef_ref=lukemathwalker/cargo-chef:latest-rust-1@sha256:000...000` to `$GITHUB_OUTPUT` and exits 0.
- **Negative path (empty manifest):** wrong-line-count gate fires; exit 1.

## Next Phase Readiness

- Composite action is ready for `uses: ./.github/actions/read-base-digests` consumption in Plans 22-03 (release.yml + docker.yml) and 22-04 (digest-drift-check.yml).
- The locked output names (`debian_ref`, `cargo_chef_ref`) and value shapes (`image:tag@sha256:HEX`) are the interface contract — Plans 22-03 and 22-04 MUST consume them by name; renaming requires coordinated multi-plan changes.
- The sentinel-digest manifest from Plan 22-01 will continue to pass the regex contract until Plan 22-06's HUMAN-UAT rehearsal swaps in the canonical upstream-resolved digests. Plans 22-03 and 22-04 can be planned and executed against the sentinel without functional issues (they parse and propagate the values; only `docker buildx build` would actually fail on a non-existent digest at image-pull time, which is Plan 22-03's smoke-test boundary).

## Self-Check: PASSED

- `.github/actions/read-base-digests/action.yml` — FOUND
- Commit `d184845` (feat(22-02): add read-base-digests composite action with fail-fast validation (DRIFT-01)) — FOUND in `git log --all`

---
*Phase: 22-base-image-digest-drift-detection*
*Completed: 2026-06-01*
