---
quick_id: 260531-ubf
status: in_progress
description: Post-release-readiness polish (amp cleanup, doc cross-links, version policy, smoke-rehearsal trigger)
must_haves:
  truths:
    - The 0-byte amp file at repo root is deleted (not gitignored — has no purpose)
    - README.md Documentation section links to SECURITY.md and CHANGELOG.md
    - README.md Security Model section opens with a pointer to SECURITY.md (the disclosure surface) rather than burying it
    - CONTRIBUTING.md "Tagging releases" section references CHANGELOG.md as the place to add release-note bullets
    - SECURITY.md contains a "Release versioning policy" section stating the canonical version is the git tag and explaining why Cargo.toml versions stay at 0.1.0
    - root Cargo.toml has a top-of-file comment pointing readers at the SECURITY.md versioning section
    - .github/workflows/release.yml + .github/workflows/docker.yml both accept workflow_dispatch and gate publish/build-push steps with `if: startsWith(github.ref, 'refs/tags/')`
    - SUMMARY.md documents the rehearsal procedure (which dispatch button to click, what to expect)
  artifacts:
    - README.md
    - CONTRIBUTING.md
    - SECURITY.md
    - Cargo.toml
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
  key_links:
    - SECURITY.md (already exists from 260531-thw)
    - CHANGELOG.md (already exists from 260531-thw)
    - .github/actions/install-bitcoind/action.yml (already exists from 260531-thw)
---

# Quick task 260531-ubf — Post-release-readiness polish

Four follow-up items surfaced after 260531-thw closed. All low-blast-radius.

## Task A — Remove stray amp file

**files:** `amp`

**action:** `git rm amp`. It's 0 bytes, dated May 29 (predates 260531-thw),
no purpose visible. Likely an accidental `> amp` shell redirect. Not
gitignored because gitignoring noise is worse than removing it.

**verify:** `git status` clean re: amp; `ls amp` returns "No such file".

## Task B — Cross-link SECURITY.md + CHANGELOG.md

**files:** `README.md`, `CONTRIBUTING.md`

**action:**
- `README.md` § Documentation: add two list entries pointing at
  `SECURITY.md` and `CHANGELOG.md`. Place them at the top of the list
  (security disclosure is more important than the FAQ for someone
  evaluating risk).
- `README.md` § Security Model: prepend a short sentence pointing readers
  at SECURITY.md for the disclosure policy and supply-chain status, since
  the section currently dives into capabilities without naming where to
  report a vulnerability.
- `CONTRIBUTING.md` § Tagging releases: add a note that contributors
  should add a `## [X.Y.Z]` entry to CHANGELOG.md before tagging.

**verify:**
- `grep "SECURITY.md" README.md` returns ≥ 2 lines (Documentation + Security Model).
- `grep "CHANGELOG.md" README.md` returns ≥ 1 line.
- `grep "CHANGELOG.md" CONTRIBUTING.md` returns ≥ 1 line.

## Task C — Document crate-version policy

**Decision (recorded here, not in a separate ADR):** Keep all four
workspace crates at `0.1.0`. Rationale:

- None of `coordinator`, `client`, `liquidity-bot`, `shared` are published
  to crates.io. The Cargo.toml `version` field is purely internal.
- The canonical release identifier is the **git tag** (`v1.5.0`) +
  GitHub Release; that's what operators consume.
- The binaries currently have no `--version` CLI flag (per the
  `coordinator-smoke` job comment in `ci.yml`), so `CARGO_PKG_VERSION`
  is never user-visible.
- Bumping the four Cargo.toml `version =` lines at every milestone close
  is mechanical churn with zero downstream consumer. Skipping it keeps
  milestone diffs focused on the actual delivered work.

If `--version` flags land in a future milestone, the policy is revisited:
the binaries should report the git tag (via build-time `CARGO_PKG_VERSION`
override or `GIT_DESCRIBE` env), not the static Cargo.toml value.

**files:** `SECURITY.md`, `Cargo.toml`

**action:**
- `SECURITY.md`: insert a `## Release versioning policy` section above
  "Where to find more" stating the policy above.
- `Cargo.toml` (workspace root): add a 3-line comment block at top
  pointing readers at the SECURITY.md section.

**verify:**
- `grep "Release versioning policy" SECURITY.md` returns 1 line.
- `grep -A1 "^# " Cargo.toml | head -5` shows the comment block.
- All four crate `version = "0.1.0"` lines unchanged.

## Task D — Enable release-smoke rehearsal via workflow_dispatch

**files:** `.github/workflows/release.yml`, `.github/workflows/docker.yml`

**action:**
- `release.yml`: add `workflow_dispatch:` alongside the `push: tags: ['v*']`
  trigger. Gate the `build` job with
  `if: startsWith(github.ref, 'refs/tags/')` so a dispatch run executes
  only the check job and stops short of uploading to GitHub Releases.
- `docker.yml`: same — workflow_dispatch trigger added; gate the
  `docker` job (the matrix that builds and pushes to ghcr.io) with
  `if: startsWith(github.ref, 'refs/tags/')` so a dispatch run executes
  only the check job and stops short of pushing images.
- Document inline at each trigger: "workflow_dispatch enables rehearsal
  of the check job (BLINDJOIN_REQUIRE_BITCOIND=1 + composite
  install-bitcoind action) without publishing artifacts. Trigger from
  the Actions tab on any branch."

**verify:**
- `grep "workflow_dispatch" .github/workflows/release.yml .github/workflows/docker.yml`
  returns 2 lines.
- `grep "startsWith(github.ref, 'refs/tags/')"` returns 2 lines (one per
  workflow's publish job).

**done:** Files committed. Actual rehearsal (clicking the dispatch button
+ confirming the check job runs the integration suite + confirming a
deliberately-broken test would fail it) is the user's action — the
infrastructure is in place.

## Notes

- Decision on Task C is recorded in the body of this file, not via
  AskUserQuestion. The user's brief said "Decide + document"; the
  rationale chain (no crates.io, no --version flag, churn vs zero
  consumer benefit) is concrete enough that asking would be ceremony.
- No code touched. Pre-push hook will still run cargo check / clippy /
  audit but no test should regress.
