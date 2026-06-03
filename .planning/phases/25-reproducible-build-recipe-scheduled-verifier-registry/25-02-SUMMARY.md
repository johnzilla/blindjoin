---
phase: 25-reproducible-build-recipe-scheduled-verifier-registry
plan: 02
subsystem: release-pipeline
tags: [release-yml, determinism, source-date-epoch, rustflags, deterministic-tar, draft-cleanup, repro-02]
requires:
  - 25-01  # rust-toolchain.toml + Cargo.toml [profile.release] strip="symbols" + rust-toolchain-pin-check
provides:
  - release.yml build job pinned to ubuntu-24.04 (D-08)
  - release.yml env: block with RUSTFLAGS (--remap-path-prefix x2) + CARGO_INCREMENTAL=0 (D-02 + D-04)
  - release.yml Compute SOURCE_DATE_EPOCH step deriving from `git log -1 --format=%ct $GITHUB_SHA` (D-03)
  - release.yml Build step uses explicit `--locked` (D-05; fail-fast on stale Cargo.lock)
  - release.yml Package step deterministic tar+gzip pipeline (D-06; 5 flags + `gzip -n`)
  - release.yml softprops step post-D-13 (no draft: true, clean step name, comment cites SIGN-03 deferral)
affects:
  - 25-03  # docs/REPRODUCIBLE-BUILD.md Recipe section reproduces RUSTFLAGS / SOURCE_DATE_EPOCH / tar pipeline
  - 25-04  # reproducible-verify.yml rebuild step + non-draft `gh release download` precondition
tech-stack:
  added: []   # no new dependencies; only YAML structural edits + env vars
  patterns:
    - comments-as-contract above structural blocks (Plan 22-04 paraphrasing discipline)
    - forbidden-token absence audit (file-level grep) — ubuntu-latest / draft: true / flips out of draft / --draft=false
    - SHA-pin trailing-comment style (`@<40-hex> # vX.Y.Z`) preserved on all `uses:` lines
key-files:
  created: []
  modified:
    - .github/workflows/release.yml
decisions:
  - "D-02 RUSTFLAGS literal verbatim per CONTEXT/RESEARCH: two --remap-path-prefix flags strip ${{ github.workspace }} + /home/runner/.cargo from embedded debug info. CARGO_INCREMENTAL=0 in same env: map (D-04). SOURCE_DATE_EPOCH NOT in env: map — derived at runtime via $GITHUB_ENV in the dedicated Compute step (D-03 spec)."
  - "D-08 runner pin: build job → ubuntu-24.04; check job stays on the rolling-release runner alias (CONTEXT explicit — tests have no byte-equal requirement)."
  - "D-13 softprops cleanup: removed `draft: true` line, renamed step to `Upload to GitHub Releases` (dropped parenthetical), rewrote comment block to cite D-13 + 2026-06-02 SIGN-03 deferral (commit f11d544). Files list + SHA pin + env block unchanged."
  - "Pre-mod 226 lines → post-mod 291 lines (+65 net). Increase driven primarily by comment blocks above the runs-on pin, env: block, Compute step, Build step, and Package step (each ~5-25 lines of auditor-grepable prose per Pattern 1 + Plan 22-04 discipline)."
metrics:
  duration: ~4min
  completed_date: 2026-06-02
---

# Phase 25 Plan 02: release.yml Determinism + Draft Cleanup Summary

REPRO-02 wired in a single coherent diff: build job pinned to `ubuntu-24.04`, env block introduces RUSTFLAGS + CARGO_INCREMENTAL=0, Compute SOURCE_DATE_EPOCH step inserted before Build, `--locked` added to cargo build, Package step rewritten as the deterministic tar+gzip pipeline, and the orphan `draft: true` from Phase 24 SIGN-03 (deferred indefinitely 2026-06-02) is removed so Plan 25-04's verifier can `gh release download` without auth gymnastics.

## What Shipped

Single file modified: `.github/workflows/release.yml` (pre-mod 226 lines → post-mod 291 lines, +65 net).

| Element | Post-mod line range | Change |
|---|---|---|
| `build` job `runs-on:` | L73 | `ubuntu-latest` → `ubuntu-24.04`; new 8-line comment block above citing REPRO-03 + Pitfall 7 (paraphrases the forbidden token as "rolling-release runner alias" / "unpinned runner image"). |
| `build` job `env:` block | L120-122 | NEW. RUSTFLAGS (two `--remap-path-prefix` flags) + CARGO_INCREMENTAL: "0". 25-line comment block above naming all three determinism vars + REPRO-01/02 + Pitfall 6 long-tail expectation + the SOURCE_DATE_EPOCH-not-in-env-map rationale. |
| `Compute SOURCE_DATE_EPOCH from tagged commit time` step | L154-155 | NEW. Inserted between `Read canonical base-image digests` (L142-144) and `Build coordinator and client` (L161-162). 8-line comment block cites REPRO-02 + the `git log -1 --format=%ct $GITHUB_SHA` derivation + the $GITHUB_ENV propagation note. |
| `Build coordinator and client` step | L161-162 | `cargo build --release …` → `cargo build --release --locked …`. 4-line comment block above cites REPRO-02 + the fail-fast-on-stale-Cargo.lock rationale. `--bin` ordering preserved (coordinator → client → liquidity-bot). |
| `Package` step | L178-188 | RENAMED to `Package (deterministic tar + gzip)`. Body replaced with the 5-flag tar invocation piped to `gzip -n`. 13-line comment block above enumerates all five tar flags + gzip -n + the "tar matches but gzip doesn't" failure-mode rationale (Debian wiki cited). |
| `Upload to GitHub Releases` (softprops) step | L282-291 | `draft: true` REMOVED. Step name dropped the parenthetical (`(draft — maintainer flips out of draft after PGP upload)` → `Upload to GitHub Releases`). 10-line comment block rewritten: cites D-13 + 2026-06-02 SIGN-03 deferral (commit f11d544) + the verifier `gh release download` precondition. Files list (4 entries), SHA pin (`@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`), and `env: GITHUB_TOKEN:` block all unchanged. |

## How It Works (Data Flow)

1. **Tag push** triggers `release.yml`; `check` job runs on the rolling runner alias as before.
2. **`build` job** (now on pinned `ubuntu-24.04`) executes top-down:
   - Checkout → toolchain install → rust-cache → digest manifest read (Phase 22).
   - **Compute SOURCE_DATE_EPOCH** writes `SOURCE_DATE_EPOCH=<tagged commit's committer time>` to `$GITHUB_ENV`. All subsequent steps in the job inherit it (verified via docs.github.com/en/actions).
   - **Build** runs `cargo build --release --locked` with `RUSTFLAGS` from the job-level `env:` map applied. `--locked` fails the run (exit 101) if `Cargo.lock` is out of sync; `RUSTFLAGS` strips `${{ github.workspace }}` and `/home/runner/.cargo` from embedded debug info; `CARGO_INCREMENTAL=0` prevents incremental compilation from leaking host-specific paths into metadata.
   - **Package (deterministic tar + gzip)** copies binaries to `dist/` then runs `tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH}" -cf - -C dist . | gzip -n > blindjoin-linux-amd64.tar.gz`. Five tar flags + `gzip -n` collectively strip every known nondeterminism source per reproducible-builds.org archive guidance.
   - **cosign sign-blob** + **attest-build-provenance** + **Rename to .sigstore** (Phase 24, preserved verbatim).
   - **Upload to GitHub Releases** uploads all 4 assets directly as a published (non-draft) release.
3. **External rebuilder** runs the same commands on a fresh `ubuntu-24.04` shell with `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct v1.6.0)` and gets a byte-equal tarball (REPRO-01 contract; verified continuously by `reproducible-verify.yml` from Plan 25-04).

## Deviations from Plan

None — plan executed exactly as written. RESEARCH §Code Examples 3, 4, 6 and CONTEXT §Specific Ideas D-02/D-04/D-06 blocks reproduced verbatim with the auditor-grepable comment shapes specified in §D-20.

The forbidden-token paraphrasing discipline (Plan 22-04 lesson) was applied to all new comments: the runner-pin comment block uses "rolling-release runner alias" and "unpinned runner image" rather than the literal `ubuntu-latest` token; the softprops comment block uses "without auth gymnastics" / "publishes directly" rather than `flips out of draft` / `--draft=false` / `flip the release`. All four forbidden-token absence audits pass at file level.

## Verification

All Task 1 verifications passed (10 grep assertions + YAML parse).
All Task 2 verifications passed (10 grep assertions + 4-entry files-list count + YAML parse).
All overall verifications (`<verification>` block in PLAN.md) passed:

| # | Check | Result |
|---|---|---|
| 1 | YAML parses | OK |
| 2 | build job `runs-on: ubuntu-24.04`; check job retains `runs-on: ubuntu-latest` | OK (L73 + L34) |
| 3 | env: block with RUSTFLAGS + CARGO_INCREMENTAL=0 + multi-paragraph comment | OK (L120-122) |
| 4 | Compute SOURCE_DATE_EPOCH step between digest-read and Build | OK (L154-155 between L142-144 and L161-162) |
| 5 | Build step uses `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot` | OK (L162) |
| 6 | Package step uses deterministic `tar ... \| gzip -n` pipeline with all 5 flags | OK (L178-188) |
| 7 | softprops step has no `draft: true`, clean step name, comment cites D-13 + SIGN-03 deferral | OK (L282-291) |
| 8 | Forbidden-token absence: `draft: true`, `flips out of draft`, `--draft=false`, `ubuntu-latest` (in build stanza) | OK — all 4 absent |
| 9 | Phase 22 read-base-digests composite preserved | OK (L144) |
| 10 | Phase 24 cosign + SLSA + rename steps preserved | OK (cosign L221, SLSA L253, rename L269) |

## Commits

| Hash | Task | Message |
|---|---|---|
| c3cd193 | 1 | feat(25-02): pin build runner + add determinism env + Compute SOURCE_DATE_EPOCH + --locked |
| 7cf68f4 | 2 | feat(25-02): deterministic tar+gzip Package step + D-13 softprops draft cleanup |

## Known Stubs

None. The plan delivers production YAML configuration; no placeholder values, no TODO comments, no unused steps. The `<TBD-v1.6.0-cut>` placeholder mentioned in the CONTEXT D-10 plan lives in `docs/REPRODUCIBLE-BUILD.md` (owned by Plan 25-03), not in `release.yml`.

## Threat Flags

None. Plan 25-02 modifies an existing trust boundary (CI → release tarball bytes) in the direction the phase's threat model prescribes — every change is a `mitigate` disposition from the PLAN's `<threat_model>` register (T-25-02-01 through T-25-02-05 are all addressed; T-25-02-06 and T-25-02-07 are accepted by design). No new security surface introduced.

## Self-Check: PASSED

- `[ -f .github/workflows/release.yml ]` → FOUND
- `git log --all --oneline | grep -q c3cd193` → FOUND (Task 1)
- `git log --all --oneline | grep -q 7cf68f4` → FOUND (Task 2)
- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` → parses cleanly
- All 4 forbidden-token absence audits → absent
- All 10 Task 1 + 10 Task 2 + 10 overall PLAN verifications → pass
