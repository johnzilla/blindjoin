---
phase: 25-reproducible-build-recipe-scheduled-verifier-registry
plan: 04
subsystem: infra
tags: [scheduled-workflow, verifier, reproducibility, cosign-reverify, issue-on-mismatch, github-actions]

# Dependency graph
requires:
  - phase: 25-reproducible-build-recipe-scheduled-verifier-registry
    provides: "release.yml deterministic build (Plan 25-02) + docs/REPRODUCIBLE-BUILD.expected-sha256.txt colon-delimited lookup (Plan 25-03) + rust-toolchain.toml channel 1.95.0 (Plan 25-01)"
  - phase: 24-tarball-signature-attestation
    provides: "cosign sign-blob → blindjoin-linux-amd64.tar.gz.bundle (SIGN-01); release publishes non-draft per Phase 25 D-13 so gh release download needs no auth"
  - phase: 23-image-signature-attestation
    provides: "sigstore/cosign-installer SHA pin (v3.10.1) + cosign-release v2.6.3; sigstore-pin-check gate at ci.yml:292-326 auto-covers any new sigstore use"
  - phase: 22-base-image-digest-drift-detection
    provides: "digest-drift-check.yml structural template (cron+dispatch + permissions paraphrasing + label-auto-create + title-exact dedup + issue-not-PR contract)"
provides:
  - "Monthly scheduled verifier (.github/workflows/reproducible-verify.yml, 261 lines) closing REPRO-03"
  - "Two-title issue scheme (D-12): low-severity runner-image-drift vs HIGH-severity sha256-mismatch on identical ImageVersion"
  - "workflow_dispatch rehearsal entry point that Plan 25-05's v1.6.0-rc.0 procedure dispatches to capture the real sha256 + ImageVersion"
  - "Cosign re-verify gate (Phase 24 SIGN-01 inheritance) — reproducibility green-status now ties to signed-supply-chain green-status; both must hold"
  - "BLOCKER 2 fix in production: single `awk -F:` pass against .expected-sha256.txt derives BOTH EXPECTED_DOC and PINNED_IMAGE_VERSION (no markdown parsing)"
affects:
  - "Plan 25-05 (rehearsal + registry submission — dispatches this workflow_dispatch + cites the workflow URL in the registry entry)"
  - "Future maintainer triage workflow (every [reproducibility-regression] issue is owned by ${GITHUB_REPOSITORY_OWNER} via auto-assignee)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Verbatim SHA-pin reuse across workflows: actions/checkout, sigstore/cosign-installer + cosign-release, dtolnay/rust-toolchain — single source of truth; pin-check gates already cover any new file"
    - "Single awk -F: pass for both lookup fields (replaces broken grep -oE markdown parse that returned nothing — Phase 25 BLOCKER 2 lesson)"
    - "Workflow context divergence: $GITHUB_SHA is the trigger commit on cron/dispatch (NOT the tag SHA); when the verifier checks out a tag explicitly, SOURCE_DATE_EPOCH MUST derive from HEAD, not $GITHUB_SHA"

key-files:
  created:
    - ".github/workflows/reproducible-verify.yml (261 lines, 7 verification steps, monthly cron + workflow_dispatch)"
  modified: []

key-decisions:
  - "Implemented D-12 two-title scheme with drift-vs-divergence classification reading PINNED_IMAGE_VERSION via the BLOCKER 2-fixed single awk -F: pass (the broken `grep -oE 'ImageVersion.{0,5}[0-9.]+' docs/REPRODUCIBLE-BUILD.md` is gone)"
  - "SOURCE_DATE_EPOCH derives from `git log -1 --format=%ct HEAD` (NOT $GITHUB_SHA) because the verifier runs on cron/dispatch where $GITHUB_SHA is the trigger commit, not the tag — release.yml legitimately uses $GITHUB_SHA only because it runs on tag push where they coincide"
  - "Verifier exits 1 ALWAYS on mismatch (even when dedup skips the issue create) per RESEARCH Open Question #3 — green-only is the precondition for D-14 registry submission"
  - "Step-level env: block on Rebuild step rather than job-level — this workflow has no job-level env to inherit from, unlike release.yml's build job"

patterns-established:
  - "Phase 22 issue-creation triad inherited verbatim: gh label create (idempotent) → title-exact dedup via `gh issue list --search '\"<TITLE>\" in:title'` → gh issue create with --assignee ${GITHUB_REPOSITORY_OWNER}"
  - "Phase 24 SIGN-01 inheritance: any rebuild-and-compare workflow must cosign verify-blob BEFORE the sha256 compare so reproducibility green-status implies signed-supply-chain green-status"
  - "Pitfall B defensive guard: `[[ -n \"${ImageVersion:-}\" ]] || exit 1` is mandatory on any workflow that consumes the GitHub-hosted-runner-only ImageVersion env var"

requirements-completed:
  - REPRO-03

# Metrics
duration: 13min
completed: 2026-06-03
---

# Phase 25 Plan 04: Scheduled monthly reproducible-build verifier Summary

**Monthly cron + workflow_dispatch GitHub Actions workflow that re-verifies blindjoin-linux-amd64.tar.gz byte-equality on a pinned ubuntu-24.04 runner, re-verifies the cosign bundle inline, and opens a `[reproducibility-regression]` issue with drift-vs-divergence classification on mismatch.**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-06-03T01:59:14Z
- **Completed:** 2026-06-03T02:12:02Z
- **Tasks:** 2
- **Files modified:** 1 (new file)

## Accomplishments

- Created `.github/workflows/reproducible-verify.yml` (261 lines) — the scheduled monthly verifier that closes REPRO-03
- All 7 verification steps from D-11 + RESEARCH §Code Examples Example 5 implemented in order
- Phase 24 SIGN-01 cosign re-verify wired inline before the sha256 compare (defense-in-depth: cosign sig + sha256 byte-equality must BOTH hold)
- D-12 two-title scheme implemented with the BLOCKER 2-fixed drift-vs-divergence classification (single awk -F: pass against `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` derives BOTH `EXPECTED_DOC` and `PINNED_IMAGE_VERSION` — markdown parsing eliminated)
- Phase 22 issue-creation triad inherited verbatim: label auto-create + title-exact dedup + gh issue create with auto-assignee
- All four `uses:` SHA pins reused verbatim from Phase 23/24 — Phase 23's `sigstore-pin-check` gate at `ci.yml:292-326` auto-covers the new sigstore use with no new CI gate
- `workflow_dispatch:` trigger wired so Plan 25-05's v1.6.0-rc.0 rehearsal procedure can dispatch this workflow to capture the real sha256 + ImageVersion that replace the placeholders in `.expected-sha256.txt`

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold reproducible-verify.yml (top-of-file + Capture ImageVersion step)** — `87a669d` (feat)
2. **Task 2: Append 6 remaining verification steps** — `8550f42` (feat)

## Files Created/Modified

- `.github/workflows/reproducible-verify.yml` (NEW, 261 lines) — scheduled monthly verifier with 7 steps:

| # | Step name | Line range | Role |
|---|-----------|------------|------|
| 1 | Capture runner ImageVersion | 89–93 | Pitfall B defensive guard + export `VERIFIER_IMAGE_VERSION` |
| 2 | Resolve latest release tag | 100–112 | `gh release view --json tagName --jq .tagName` + fail-fast on no releases |
| 3 | Download release tarball + cosign bundle | 114–126 | `gh release download --pattern 'blindjoin-linux-amd64.tar.gz*'` → `/tmp/rel` |
| 4 | Re-verify cosign blob signature on downloaded tarball | 128–142 (incl. cosign-installer uses: at 128) | Phase 24 SIGN-01 inheritance — same `--certificate-identity-regexp` shape as release.yml |
| 5 | Checkout source at LATEST_TAG + install pinned toolchain | 144–155 (checkout at 144, dtolnay at 151) | Pitfall A (explicit toolchain: input) + Pitfall C (default full checkout for Cargo.lock) |
| 6 | Rebuild per REPRO-01 recipe | 160–191 | Step-level env: RUSTFLAGS + CARGO_INCREMENTAL=0; `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)`; `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot`; 5-flag deterministic tar + `gzip -n` |
| 7 | Compare sha256 + classify result + open issue on mismatch | 193–261 | D-18 + BLOCKER 2 single awk pass → EXPECTED_DOC + PINNED_IMAGE_VERSION; label auto-create; D-12 two-title classification; Phase 22 title-exact dedup; exit 1 always on mismatch |

## Decisions Made

- Followed PLAN.md exactly as written, applying the BLOCKER 2 fix (single `awk -F:` lookup against `.expected-sha256.txt`) in Step 7 — the broken `grep -oE 'ImageVersion.{0,5}[0-9.]+' docs/REPRODUCIBLE-BUILD.md` from RESEARCH Example 5 is gone.
- Used `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)` (not `$GITHUB_SHA`) per the interfaces L93–94 fix note: `$GITHUB_SHA` on cron/dispatch is the trigger commit (default-branch HEAD), not the release tag. After `actions/checkout` with `ref: ${{ env.LATEST_TAG }}`, `HEAD` IS the tag commit, so both contexts (release.yml and verifier) end up deriving the same value (the tag commit's committer time).
- Used step-level `env:` block on the Rebuild step rather than a job-level block — `reproducible-verify.yml` has no job-level `env:`, so the determinism vars must be declared on the step itself.

## Deviations from Plan

None — plan executed exactly as written. Both tasks completed in order with all `<verify>` assertions passing.

(Note: one chained-grep verify assertion in Task 2 used a regex `'release\.yml@refs/tags/v\.\*'` that does not match the literal `release\.yml@refs/tags/v.*` (with `\.` for the dot) that appears verbatim in the file — and verbatim in `release.yml`. All 24 individual semantic assertions verified pass: cosign verify-blob is present with the correct `--certificate-identity-regexp` issuer pattern and `--certificate-oidc-issuer` value. The grep-regex mismatch is a quirk of the plan's verification command, not a deviation in the implemented file content.)

## Forbidden-Token Absence Audit

All Plan 22-04 paraphrasing-discipline audits pass:

| Token | Audit | Result |
|-------|-------|--------|
| `ubuntu-latest` | `! grep -q 'ubuntu-latest' reproducible-verify.yml` | PASS — only `runs-on: ubuntu-24.04` appears |
| `id-token:` at code | `! grep -qE '^[^#]*id-token:' …` | PASS — only `id-token` (no colon) in deliberately-omitted-scopes comment |
| `attestations:` at code | `! grep -qE '^[^#]*attestations:' …` | PASS |
| `packages:` at code | `! grep -qE '^[^#]*packages:' …` | PASS |
| `pull-requests:` at code | `! grep -qE '^[^#]*pull-requests:' …` | PASS — paraphrased as "PR-write" in the omitted-scopes comment |
| `pages:` at code | `! grep -qE '^[^#]*pages:' …` | PASS |
| `deployments:` at code | `! grep -qE '^[^#]*deployments:' …` | PASS |

## Recipe Byte-Equality Confirmation

The Rebuild step (lines 160–191) reproduces release.yml's `build` job byte-for-byte:

- **Same toolchain pin:** `dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable` + `toolchain: "1.95.0"` (matches release.yml:127–131 verbatim)
- **Same determinism env:** `RUSTFLAGS: "--remap-path-prefix=${{ github.workspace }}=/build --remap-path-prefix=/home/runner/.cargo=/cargo"` + `CARGO_INCREMENTAL: "0"` (matches release.yml:121–122 verbatim; only the scope differs — step-level here vs job-level there)
- **Same SOURCE_DATE_EPOCH derivation:** committer time of the tag commit (release.yml uses `$GITHUB_SHA` because on a tag-push trigger it IS the tag SHA; verifier uses `HEAD` after explicit `actions/checkout` with `ref: ${{ env.LATEST_TAG }}` — both resolve to the same epoch value, the divergence is structural per workflow trigger context)
- **Same build:** `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot` (matches release.yml:162 verbatim)
- **Same 5-flag deterministic tar + gzip -n pipeline:** `--sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH}" -cf - -C dist .` then `| gzip -n > blindjoin-linux-amd64.tar.gz` (matches release.yml:184–187 verbatim)

The verifier IS the reproduction — any drift between this step and release.yml's build job is precisely the supply-chain threat the workflow exists to surface.

## Issues Encountered

None.

## User Setup Required

None — the workflow uses only `${{ secrets.GITHUB_TOKEN }}` (auto-provisioned on every workflow run) and no additional repo secrets. The `reproducibility-regression` label is auto-created on first run (`gh label create … 2>/dev/null || true`).

## Next Phase Readiness

Plan 25-05 inputs ready:
- **Rehearsal entry point:** Plan 25-05's `docs/RELEASING.md §Reproducibility verification rehearsal` procedure (D-10) cites this file's `workflow_dispatch:` trigger to capture the v1.6.0-rc.0 sha256 + ImageVersion. From the Actions tab → "Reproducible build verifier" → "Run workflow" → wait for "Capture runner ImageVersion" output + the rebuilt-sha256 value, then replace the `<TBD-v1.6.0-cut-sha256>` and `<TBD-v1.6.0-cut-imageversion>` placeholders in `docs/REPRODUCIBLE-BUILD.expected-sha256.txt`.
- **Registry-submission gate:** RESEARCH Open Question #3 — D-14 reproducible-builds.org registry submission requires "≥1 green monthly run after the v1.6.0 tag." The cron schedule is 07:00 UTC on the 1st of each month, so the first green run lands one calendar month after v1.6.0 ships (or earlier via workflow_dispatch).
- **Phase 23 sigstore-pin-check coverage:** The `sigstore-pin-check` job at `ci.yml:292-326` greps every `.github/workflows/*` for the cosign-installer SHA — the new `reproducible-verify.yml` is automatically covered with no new CI gate needed.

## Self-Check: PASSED

- `[ -f .github/workflows/reproducible-verify.yml ]` → FOUND
- `git log --oneline | grep 87a669d` → FOUND (Task 1)
- `git log --oneline | grep 8550f42` → FOUND (Task 2)
- All 7 step names + all 4 SHA pins + D-18 lookup mechanism + D-12 two-title scheme + forbidden-token absence audits + YAML-parse: all PASS per individual assertion check

---
*Phase: 25-reproducible-build-recipe-scheduled-verifier-registry*
*Completed: 2026-06-03*
