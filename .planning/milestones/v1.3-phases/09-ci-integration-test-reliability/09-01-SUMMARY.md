---
phase: 09-ci-integration-test-reliability
plan: 01
subsystem: infra
tags: [ci, github-actions, bitcoind, corepc-node, pgp, actions-cache, supply-chain]

requires:
  - phase: 08-public-endpoint-hardening
    provides: "SHA-pin discipline for GitHub Actions and workflow-level env: block (FORCE_JAVASCRIPT_ACTIONS_TO_NODE24 precedent)"
provides:
  - ".bitcoind-version: single-source-of-truth pin for Bitcoin Core release used by integration tests (30.2)"
  - "CI bitcoind install pipeline (read pin -> cache -> PGP+SHA256-verified install on miss -> export BITCOIND_EXE) wired into the test: job in .github/workflows/ci.yml"
  - "Workflow-level BLINDJOIN_REQUIRE_BITCOIND=1 contract — the env-var gate Plans 09-02..09-04 require to make integration tests panic-on-miss in CI"
affects:
  - "09-02 (require_bitcoind helper) — reads BLINDJOIN_REQUIRE_BITCOIND set here"
  - "09-03 (BitcoindGuard + bootstrap_regtest_bitcoind) — depends on BITCOIND_EXE being exported"
  - "09-04 (#[ignore] carve-outs in full_round.rs) — relies on CI invocation NOT passing --include-ignored"
  - "09-05 (CONTRIBUTING.md) — documents the same .bitcoind-version + BITCOIND_EXE / BLINDJOIN_REQUIRE_BITCOIND contract for local dev"
  - "Phase 10 (REPAIR-01/02) — its success criterion 'all tests pass against pinned bitcoind' becomes observable only with this CI substrate"

tech-stack:
  added:
    - "actions/cache@v4.3.0 (SHA: 0057852bfaa89a56745cba8c7296529d2fc39830) — first cache action in the repo for an external binary"
  patterns:
    - "Pin manifest file (.bitcoind-version, plain version string, no comments) read by CI via $(cat) and referenced by docs"
    - "Content-addressed key fetch from a SHA-pinned bitcoin-core/guix.sigs commit (defeats keyserver flake and a hostile main HEAD)"
    - "Imported-key fingerprint assertion BEFORE trusting any signature (gpg --list-keys --with-colons | grep -q ${KEY_FP})"
    - "Cache key includes ${{ runner.os }} + version segment so OS-variant rotation and version bumps invalidate the cache correctly"

key-files:
  created:
    - ".bitcoind-version"
  modified:
    - ".github/workflows/ci.yml"

key-decisions:
  - "Re-verified actions/cache v4 SHA at execution time matches the value documented in CONTEXT.md/RESEARCH.md (0057852bfaa89a56745cba8c7296529d2fc39830 — no drift since 2026-05-27 research)"
  - "Pinned bitcoin-core/guix.sigs commit 893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59 (main HEAD on 2026-05-26) for the achow101 key fetch; documented inline so the next .bitcoind-version bump can re-pin in one place"
  - "Removed --include-ignored from the Run tests command (kept the existing cargo test --workspace --all-targets verbatim) per amended D-10: the 6 Phase-10 carve-out tests will list as ignored in cargo output without executing"
  - "Workflow-level BLINDJOIN_REQUIRE_BITCOIND=1 added now (Plan 09-01) even though the require_bitcoind helper that consumes it doesn't land until 09-02 — the env var is harmless to all jobs (clippy/coordinator-smoke/audit don't read it) and front-loading the contract surfaces the dependency in git history"

patterns-established:
  - "Pin manifest pattern: plain-text version file at repo root, no metadata, $(cat) substitutes cleanly into URLs and download steps"
  - "Cache-then-verify-on-miss: actions/cache restores the binary directly when warm; on cache miss the install step runs the full PGP+SHA256 integrity gate before populating the cache. A poisoned cache slot is recoverable by bumping .bitcoind-version (forces cache miss and full re-verify)"
  - "SHA-pin discipline extended to actions/cache@v4: mirrors the existing actions/checkout / dtolnay/rust-toolchain / Swatinem/rust-cache pins in this same file with the same ' # <human-tag>' comment style"

requirements-completed:
  - "TEST-01"
  - "TEST-02"

duration: 20min
completed: 2026-05-27
---

# Phase 9 Plan 1: CI bitcoind install substrate Summary

**Pinned Bitcoin Core v30.2 onto the GitHub Actions runner via an actions/cache + PGP+SHA256-verified tarball install, exported BITCOIND_EXE for corepc-node, and added workflow-level BLINDJOIN_REQUIRE_BITCOIND=1 — the CI substrate Plans 09-02..09-04 build on to make integration tests panic-on-miss instead of silently graceful-skip.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-05-27T02:04:46Z
- **Completed:** 2026-05-27T02:25Z (approximate; per /tmp epoch delta)
- **Tasks:** 3 / 3
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments

- `.bitcoind-version` at repo root contains the bare string `30.2` — the single source of truth that CI, Plan 09-05's CONTRIBUTING.md, and future-version-bump PRs all read.
- `.github/workflows/ci.yml` workflow-level `env:` block carries `BLINDJOIN_REQUIRE_BITCOIND: "1"` alongside the existing `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` — the env-var gate the per-test `require_bitcoind` helper (Plan 09-02) reads at runtime.
- The `test:` job in `.github/workflows/ci.yml` now installs `bitcoind` v30.2 between `Swatinem/rust-cache` and `Run tests` via four new steps: read pin, restore cache, install on miss (achow101 PGP key fetched from a SHA-pinned guix.sigs commit + fingerprint asserted + SHA256SUMS gpg-verified + tarball hash-checked), export `BITCOIND_EXE`. `Run tests` is unchanged at `cargo test --workspace --all-targets` (no `--include-ignored`).
- Other jobs (`clippy`, `coordinator-smoke`, `audit`) are untouched — they don't need bitcoind, and the workflow-level env var is inert to them.

## Task Commits

Each task was committed atomically:

1. **Task 1: Pin Bitcoin Core version in .bitcoind-version** — `326cbf9` (chore)
2. **Task 2: Add BLINDJOIN_REQUIRE_BITCOIND to workflow-level env block** — `24f2bc6` (feat)
3. **Task 3: Add bitcoind install + cache + BITCOIND_EXE export steps to test job** — `b254023` (feat)

**Plan metadata commit:** _(pending — to follow this SUMMARY)_

## Files Created/Modified

- `.bitcoind-version` (created) — Plain-text version pin (`30.2`). 1 line. Read by `$(cat .bitcoind-version)` in `ci.yml` step output `bitcoind_version.version`. Future bumps are a one-line PR.
- `.github/workflows/ci.yml` (modified) — Two distinct additions: (a) workflow-level env-var `BLINDJOIN_REQUIRE_BITCOIND: "1"` with a 2-line Phase-9 comment; (b) 4 new steps in the `test:` job that read the pin, cache the binary, install + verify it on cache miss, and export `BITCOIND_EXE`. Total: +74 lines, 0 deletions.

## Decisions Made

- **actions/cache@v4 SHA re-verification at execution time.** Ran `curl -sL https://api.github.com/repos/actions/cache/git/refs/tags/v4 | jq -r .object.sha` → `0057852bfaa89a56745cba8c7296529d2fc39830`. Matches CONTEXT.md / RESEARCH.md exactly — no drift since 2026-05-27 research. Used the same SHA + `# v4.3.0` comment style established for the existing pins in this file (actions/checkout, dtolnay/rust-toolchain, Swatinem/rust-cache).
- **guix.sigs main-HEAD SHA pin.** Ran `curl -sL https://api.github.com/repos/bitcoin-core/guix.sigs/commits/main | jq -r .sha` → `893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59`. Pinned this in the install step as `GUIX_SIGS_SHA=893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59  # pinned 2026-05-26`. Sanity-checked the achow101.gpg blob is reachable at that SHA (HTTP 200). The next maintenance touch will be a one-line change when achow101's key rotates (years out, per RESEARCH.md "Open Questions §4").
- **Removed the fingerprint duplicate from comments after first verification round.** Initial draft repeated `152812300785C96444D3334D17565732E08E5E41` in both the comment block and the `KEY_FP=` shell assignment. Task 3's acceptance criterion specifies `grep -c '<fingerprint>' returns 1`. Cleaned the comment to reference `KEY_FP` symbolically while keeping the fingerprint authoritatively in the shell assignment.
- **Removed the literal string `sha256sum -c` from comments for the same reason.** Acceptance criterion specifies `grep -c 'sha256sum -c' returns 1`. Comment now says "hash-check the tarball against the signed SHA256SUMS entry"; the actual `sha256sum -c` invocation remains the single in-code occurrence.

## Deviations from Plan

None — plan executed exactly as written.

Two minor adjustments worth noting (not deviations from the plan's _intent_, only from the first draft of the edit):

1. After Task 3's first edit, `grep -c '152812300785C96444D3334D17565732E08E5E41'` returned 2 (once in a documentation comment, once in the `KEY_FP=` assignment). The plan's acceptance criterion required exactly 1. Cleaned the comment to reference `KEY_FP` symbolically. Same fix applied for `sha256sum -c`. Both are documentation-style adjustments to satisfy strict literal-grep counts; functionally equivalent.

2. No package install was needed (no Rule 3 package-install case). No architectural change was needed (no Rule 4 case). No bug or missing critical functionality was discovered (no Rule 1 / Rule 2 case). The plan as written was sufficient for clean execution.

## Issues Encountered

- None substantive. Two acceptance-criteria precision issues (fingerprint count = 2, `sha256sum -c` count = 2) caught by the verify step on Task 3 and fixed in the same edit cycle before commit.

## Threat Flags

No new threat surface introduced outside the plan's `<threat_model>`. All threats enumerated in the plan (T-09-01 tarball tampering, T-09-02 key-blob tampering, T-09-03 cache-poisoning, T-09-04 repudiation, T-09-SC actions/cache supply chain) are mitigated per the plan's disposition column:

- T-09-01: SHA256SUMS verified against achow101 PGP signature **before** the tarball hash check.
- T-09-02: guix.sigs SHA-pinned at commit `893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59`; imported-key fingerprint asserted against `KEY_FP=152812300785C96444D3334D17565732E08E5E41`.
- T-09-03: Cache key is `${{ runner.os }}-bitcoind-${{ version }}` — both cross-OS and cross-version collision-safe; cache miss path re-runs full verify.
- T-09-SC: `actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0` pinned per Phase 6 supply-chain discipline.

## User Setup Required

None — no external service configuration required. The plan modifies CI infrastructure only; no operator-side actions are required to merge it. The first PR after merge will trigger a cache miss path (proving the install works); subsequent runs will hit the cache.

## Next Phase Readiness

**Ready for Plan 09-02 (require_bitcoind helper).** The contract Plan 09-02 depends on is now in place:
- `BLINDJOIN_REQUIRE_BITCOIND=1` is set workflow-wide in CI.
- `BITCOIND_EXE` points at `$HOME/.local/bin/bitcoind` in every step after the export.
- The pinned version is observable in `cargo test` stdout via the `bitcoind --version` line if any test wants to assert it (Plan 09-02 may or may not).

**Ready for Plans 09-03 / 09-04** (BitcoindGuard + bootstrap helper, `#[ignore]` carve-outs). The CI invocation is `cargo test --workspace --all-targets` — without `--include-ignored` — so `#[ignore]`-marked carve-out tests will appear as `ignored` lines in output without executing. This matches D-10's amended intent.

**Runtime verification deferred to first CI run.** The acceptance criteria for this plan are all source-text and YAML-validity checks (`grep`, `python3 -c "import yaml"`). The runtime behaviour (cache miss exercises full install path; cache hit skips install; `bitcoind --version` reports `Bitcoin Core version v30.2.0`; `BITCOIND_EXE` is honored by `corepc_node::exe_path()`) can only be observed when a PR actually runs CI. Document expected runtime behaviour:

- First PR after this merges: cache miss → full install path runs (PGP key fetch, fingerprint assert, SHA256SUMS verify, tarball hash check, extract). Expected duration: 30–60s. Look for `Verified achow101's signature on SHA256SUMS` in step output.
- Second PR on the same runner OS / same version: cache hit → install step skipped (`if: steps.cache-bitcoind.outputs.cache-hit != 'true'` is false). `BITCOIND_EXE` still exported.
- Any PR that bumps `.bitcoind-version`: cache miss (the version segment of the cache key changes); full verify re-runs.

## Self-Check: PASSED

Verified before writing this SUMMARY:

- `[ -f .bitcoind-version ]` → exit 0; `cat .bitcoind-version` → `30.2`.
- `[ -f .github/workflows/ci.yml ]` → exit 0; `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` → exit 0.
- `git log --oneline -5` shows all three task commits (`326cbf9`, `24f2bc6`, `b254023`) plus the prior context-amendment commits — confirms commits were not silently dropped.
- All 12 source-level acceptance criteria across Tasks 1–3 (grep counts, YAML validity, no `--include-ignored`, no `<EXECUTOR_PICKS_CURRENT_MAIN_HEAD>` placeholder) verified passing immediately before commit.

---

*Phase: 09-ci-integration-test-reliability*
*Plan: 01*
*Completed: 2026-05-27*
