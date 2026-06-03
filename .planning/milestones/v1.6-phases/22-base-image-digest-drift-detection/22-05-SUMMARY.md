---
phase: 22-base-image-digest-drift-detection
plan: 05
subsystem: infra
tags: [docs, security, supply-chain, codeowners, contributing, security-md]

# Dependency graph
requires:
  - phase: 22-base-image-digest-drift-detection
    provides: "docker/digests.txt manifest (22-01), .github/CODEOWNERS gate (22-01), read-base-digests composite action (22-02), release.yml+docker.yml wiring (22-03), digest-drift-check.yml scheduled workflow (22-04)"
provides:
  - "SECURITY.md §Base-image digests (v1.6 onward) — operator-facing policy documenting docker/digests.txt as the canonical manifest, the CODEOWNERS-gated human-review-only bump policy, the daily drift-check workflow with workflow_dispatch rehearsal, and the Pitfall-9 upstream-hex idempotency guarantee"
  - "SECURITY.md §Known gaps at v1.5 — strikethrough closure of the v1.5 'Base image digest pins are manual' bullet with **Closed in v1.6** cross-link"
  - "SECURITY.md §v1.6 supply-chain plan — strikethrough closure of the 'Automated base-image digest drift check' bullet with 'Shipped in Phase 22' cross-link"
  - "CONTRIBUTING.md §Bumping base-image digests — contributor-facing 5-step PR workflow with fenced docker buildx imagetools inspect command, one-image-per-PR policy, no-auto-merge mandate, xz utils / event-stream threat-model anchors, and regex-check failure recovery path"
affects: [22-06-HUMAN-UAT, phase-23-cosign-image-attestations, phase-24-release-tarball-signing, phase-25-reproducible-build]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Strikethrough + cross-link closure pattern: ~~bullet text~~ **Closed in v1.X** — see [Section anchor](#anchor) (mirrors v1.4→v1.5 audit-charter shipped-item moves)"
    - "Contributor-facing imperative-gerund header + bold-lede sub-callouts (**When you'd bump:** / **How to bump (per PR):** / **Why not auto-merge?** / **What if the regex check fails?**) — mirrors §Tagging releases PR-etiquette voice"
    - "Threat-model anchoring by named incident (xz utils 2024, event-stream 2018) — concrete supply-chain attacks the gate exists to close, not abstract risk language"

key-files:
  created:
    - ".planning/phases/22-base-image-digest-drift-detection/22-05-SUMMARY.md"
  modified:
    - "SECURITY.md (+34 lines / -7 lines — new §Base-image digests (v1.6 onward) subsection inserted between §Known gaps at v1.5 and §v1.6 supply-chain plan; two existing bullets now strikethrough with cross-links to the new subsection)"
    - "CONTRIBUTING.md (+47 lines — new ## Bumping base-image digests section inserted after ## Tagging releases)"

key-decisions:
  - "Re-wrap the **Do not auto-merge digest bumps** paragraph in SECURITY.md so the bolded directive lives on a single line. The RESEARCH.md §7.1-locked copy at line 727 wrapped the bold marker across two lines (`**Do not\\nauto-merge digest bumps**`); PLAN.md acceptance criterion line 122 runs `grep -q '\\*\\*Do not auto-merge digest bumps\\*\\*' SECURITY.md` which is a single-line grep that does NOT match across newlines. The phrasing is preserved verbatim — only the line break inside the bold marker moved. Treated as Rule 3 (auto-fix blocking issue) because the acceptance grep is load-bearing for D-05 verifiability."

patterns-established:
  - "Strikethrough closure with cross-link to new subsection: bullets moving from 'Known gaps at vN' to 'shipped in v(N+1)' use `~~strikethrough~~` lede + bold annotation + relative anchor link (`#base-image-digests-v16-onward`). Future v1.6 phases (23 cosign, 24 PGP, 25 reproducible-build) will use the same pattern when their gaps close."
  - "Contributor §Bumping base-image digests structural template: §Tagging releases is now the in-repo precedent for any future contributor-facing release-pipeline section — imperative-gerund header, one-sentence rule statement, **Why:** / **Before X:** / **How to X:** bolded sub-callouts, fenced bash blocks for exact commands, cross-references to SECURITY.md and PITFALLS.md as policy anchors."

requirements-completed: [DRIFT-01]

# Metrics
duration: 11min
completed: 2026-06-01
---

# Phase 22 Plan 05: SECURITY.md + CONTRIBUTING.md prose for base-image digest policy (D-05) Summary

**Operator-facing SECURITY.md and contributor-facing CONTRIBUTING.md now document the v1.6 base-image digest discipline — canonical `docker/digests.txt` manifest, CODEOWNERS-gated bumps, daily `digest-drift-check.yml` with `workflow_dispatch` rehearsal, and the 5-step PR workflow anchored in named supply-chain incidents (xz utils 2024, event-stream 2018).**

## Performance

- **Duration:** ~11 min
- **Started:** 2026-06-01T21:39:00Z (approx, post-22-04 commit)
- **Completed:** 2026-06-01T21:50:00Z (approx)
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- **SECURITY.md §Base-image digests (v1.6 onward) inserted** with five paragraphs (lead orientation, canonical-manifest assertion, no-auto-merge directive, drift-detection description, idempotency + rehearsal guarantees) and three load-bearing markdown links: `[`docker/digests.txt`](docker/digests.txt)`, `[`.github/CODEOWNERS`](.github/CODEOWNERS)`, `[.github/workflows/digest-drift-check.yml](.github/workflows/digest-drift-check.yml)`. The composite action at `[`.github/actions/read-base-digests/`](.github/actions/read-base-digests/)` is also cross-linked.
- **Two strikethrough closures** land in SECURITY.md tracking the v1.5→v1.6 supply-chain milestone: §Known gaps at v1.5's `~~Base image digest pins are manual.~~` with **Closed in v1.6** cross-link, and §v1.6 supply-chain plan's `~~Automated base-image digest drift check~~` with ✓ Shipped in Phase 22 cross-link. Both anchor at `#base-image-digests-v16-onward`.
- **CONTRIBUTING.md §Bumping base-image digests inserted** after §Tagging releases with the canonical `docker buildx imagetools inspect debian:bookworm-slim --format '{{.Manifest.Digest}}'` fenced command, the 5-step PR workflow, the "one image per PR" policy, the **Do not auto-merge** mandate with cross-link to `SECURITY.md#supply-chain-status`, the xz utils / event-stream threat anchors, and the `^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$` regex contract documentation for the recovery path.
- **D-05 complete** — prose half of the structural-plus-prose supply-chain gate is now landed. Combined with the v1.5-shipped `.github/CODEOWNERS` (22-01) and the Phase-22-shipped `read-base-digests` action (22-02) + drift-check workflow (22-04), DRIFT-01's "bumped only via human-reviewed PR (NOT auto-merged)" requirement is satisfied at three reinforcing layers: prose (this plan), structural (CODEOWNERS), and detection (daily workflow).
- **Standing operator-facing caveat preserved** — the "Until those land, **treat the SHA-256 checksum + the GitHub Release provenance as the only assurance the archive came from this project**" closing paragraph at lines 175-178 of SECURITY.md is unchanged. The other v1.5 known-gap bullets (cosign image attestations, GitHub Release archive signatures, reproducible-build instructions) are also unchanged — they close in Phases 23, 24, 25 respectively.

## Task Commits

Each task was committed atomically:

1. **Task 1: Update SECURITY.md §Supply-chain status with v1.6 base-image-digest policy** — `2de5de4` (docs)
2. **Task 2: Add ## Bumping base-image digests section to CONTRIBUTING.md** — `4e42d4c` (docs)

## Files Created/Modified

- `SECURITY.md` — New §Base-image digests (v1.6 onward) subsection inserted between §Known gaps at v1.5 and §v1.6 supply-chain plan. Two existing bullets strikethrough with cross-link annotations to the new subsection anchor `#base-image-digests-v16-onward`.
- `CONTRIBUTING.md` — New ## Bumping base-image digests section inserted immediately after ## Tagging releases. Documents the 5-step bump-PR workflow, the no-auto-merge mandate (with link to `SECURITY.md#supply-chain-status`), threat-model anchors, and the action's regex-validation recovery path.
- `.planning/phases/22-base-image-digest-drift-detection/22-05-SUMMARY.md` (this file).

## Decisions Made

- **Re-wrap the `**Do not auto-merge digest bumps**` paragraph in SECURITY.md so the bolded directive lives on a single line.** RESEARCH.md §7.1 line 727 wrapped the bold marker across two lines (`**Do not\nauto-merge digest bumps**`). PLAN.md acceptance criterion line 122 (`grep -q '\*\*Do not auto-merge digest bumps\*\*' SECURITY.md`) is a single-line grep that does NOT match across newlines. Wording is preserved verbatim — only the line break inside the bold marker moved by one position. The rest of the paragraph still hard-wraps at ~65 chars per the surrounding voice; only the `**Do not auto-merge digest bumps**` directive is constrained to one line. See PATTERNS §"SECURITY.md MODIFY" lines 383-385 for the additive-voice latitude that authorizes this minor re-flow.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Re-wrap `**Do not auto-merge digest bumps**` so PLAN-locked grep matches**

- **Found during:** Task 1 (SECURITY.md update) — initial verification run
- **Issue:** The RESEARCH.md §7.1-locked verbatim copy (line 727) breaks the bold marker across a line: `**Do not\nauto-merge digest bumps**`. The PLAN's acceptance criterion at line 122 runs `grep -q '\*\*Do not auto-merge digest bumps\*\*' SECURITY.md` — a single-line grep that does NOT match across line breaks. Dropping the RESEARCH-locked copy in verbatim would have failed the PLAN's own load-bearing verifiability check, even though the rendered markdown reads correctly on github.com.
- **Fix:** Re-wrapped the paragraph so `**Do not auto-merge digest bumps**` lives on a single line. The phrasing is preserved verbatim — only the line break inside the bold marker shifted one position. PATTERNS §"SECURITY.md MODIFY" lines 383-385 authorize additive-voice latitude for the new subsection; this minor whitespace adjustment is within that latitude and is required to satisfy the PLAN's load-bearing D-05 verifiability gate.
- **Files modified:** SECURITY.md (one paragraph reflow inside the §Base-image digests (v1.6 onward) subsection)
- **Verification:** Re-ran `grep -q '\*\*Do not auto-merge digest bumps\*\*' SECURITY.md` → PASS. Then re-ran all 14 SECURITY.md acceptance assertions → all PASS. Then re-ran the markdown-fence balance check → PASS.
- **Committed in:** `2de5de4` (Task 1 commit — the reflow happened pre-commit during the verification loop, so the committed file already has the single-line form)

---

**Total deviations:** 1 auto-fixed (1 Rule 3 blocking — whitespace inside a load-bearing grep target)
**Impact on plan:** No scope creep. The deviation preserves the locked phrasing verbatim and is required to satisfy the PLAN's own acceptance criterion. The PATTERNS rules call this kind of additive prose change explicitly in-scope for SECURITY.md modifications. RESEARCH.md §7.1 wraps prose at ~65 chars for source-file readability; the PLAN's acceptance grep operates at the literal-byte level. When those two contracts collide on a bolded directive, the literal-byte form wins because it is what the supply-chain audit trail actually queries.

## Issues Encountered

None beyond the deviation documented above. Both tasks executed on the first edit attempt, both verification rounds passed cleanly after the Task 1 reflow, both commits landed without pre-commit hook complaints (the `.githooks/pre-push` only fires on push and the docs commit didn't trigger Rust toolchain checks).

## User Setup Required

None — no external service configuration required. The branch-protection toggle on `main` that enforces CODEOWNERS approval is a separate human-UAT step that lands in Plan 22-06.

## Next Phase Readiness

**Plan 22-06 (HUMAN-UAT) is unblocked.** Plan 22-06 will:

- Verify the new SECURITY.md subsection anchor (`#base-image-digests-v16-onward`) renders correctly on github.com (the renderer normalizes punctuation: `Base-image digests (v1.6 onward)` → `base-image-digests-v16-onward` — verified during planning, not yet verified on the live render).
- Verify the new CONTRIBUTING.md §Bumping base-image digests cross-link (`SECURITY.md#supply-chain-status`) resolves correctly on github.com.
- Toggle on branch protection for `main` requiring CODEOWNERS approval (the structural enforcement layer that makes D-05's prose load-bearing).
- Run the fresh-machine rehearsal of ROADMAP Phase 22 Success Criteria #1-4 per Pitfall 12.

**Phase 23 dependencies are now fully documented.** Phase 23 (cosign image attestations + SLSA + SBOM) needs to update SECURITY.md again — the prose-update pattern established here (strikethrough closure with cross-link to a new subsection) is the template Phase 23 will follow when it closes the v1.5 "Docker images on `ghcr.io` are unsigned" known-gap bullet.

**No blockers.**

## Self-Check: PASSED

Files verified to exist on disk:

- `SECURITY.md` (modified) → FOUND
- `CONTRIBUTING.md` (modified) → FOUND
- `.planning/phases/22-base-image-digest-drift-detection/22-05-SUMMARY.md` → FOUND (this file)

Commits verified in `git log`:

- `2de5de4` (Task 1 — SECURITY.md) → FOUND
- `4e42d4c` (Task 2 — CONTRIBUTING.md) → FOUND

Cross-reference targets verified to exist on disk (PLAN verification block):

- `docker/digests.txt` → FOUND
- `.github/CODEOWNERS` → FOUND
- `.github/actions/read-base-digests/action.yml` → FOUND
- `.github/workflows/digest-drift-check.yml` → FOUND
- `.planning/research/PITFALLS.md` → FOUND (cross-referenced by CONTRIBUTING.md §8)

Acceptance grep assertions (all 15 of Task 1's + all 15 of Task 2's):

- All PASS after the Rule 3 reflow on Task 1.

Markdown integrity:

- SECURITY.md fence count: balanced (even).
- CONTRIBUTING.md fence count: balanced (even).

GitHub-anchor rendering note (verified at planning time, re-verify at 22-06 UAT):

- SECURITY.md §Base-image digests (v1.6 onward) → anchor `#base-image-digests-v16-onward` (lowercased, punctuation stripped except hyphens, dots removed: `v1.6` → `v16`).
- SECURITY.md §Supply-chain status → anchor `#supply-chain-status` (already in use, cross-linked from CONTRIBUTING.md).
- CONTRIBUTING.md §Bumping base-image digests → anchor `#bumping-base-image-digests` (not yet linked from anywhere in this plan, but Plan 22-06 may want to cross-link from SECURITY.md eventually).

---

*Phase: 22-base-image-digest-drift-detection*
*Completed: 2026-06-01*
