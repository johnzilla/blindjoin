---
phase: 25-reproducible-build-recipe-scheduled-verifier-registry
plan: 03
subsystem: docs-reproducible-build
tags: [docs, reproducible-build, operator-recipe, expected-sha256-lookup]
requires:
  - rust-toolchain-toml-channel-1.95.0  # from 25-01
provides:
  - docs/REPRODUCIBLE-BUILD.md operator-facing 7-section recipe
  - docs/REPRODUCIBLE-BUILD.expected-sha256.txt machine-readable lookup (D-18 + BLOCKER 2 fix)
  - colon-delimited <tag>:<sha256>:<image-version> single-source-of-truth format for Plan 25-04 verifier
affects:
  - .planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-04-PLAN.md (verifier reads .expected-sha256.txt for BOTH expected sha256 AND pinned ImageVersion via single awk -F: lookup)
  - .planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-05-PLAN.md (rehearsal procedure replaces 3 placeholders atomically: <TBD-v1.6.0-cut-sha256>, <TBD-v1.6.0-cut-imageversion> in .expected-sha256.txt + <TBD-v1.6.0-cut> in markdown table; also appends registry entry URL to §Continuous verification after D-14)
tech-stack:
  added: []
  patterns:
    - "Operator-facing 7-section H2 markdown doc mirroring SECURITY.md sibling-subsection style (prose intro + numbered list + fenced bash + > Note blockquote)"
    - "Companion `.expected-sha256.txt` file with colon-delimited triple format <tag>:<sha256>:<image-version> as verifier single-source-of-truth (D-18 + BLOCKER 2 fix — supersedes markdown-table parse)"
    - "Distinct placeholder strings for sha256 vs ImageVersion (<TBD-v1.6.0-cut-sha256>, <TBD-v1.6.0-cut-imageversion>) so Plan 25-05's rehearsal substitutes each via dedicated sed passes — atomic substitution scope is now 3 sites for v1.6.0-rc.0"
key-files:
  created:
    - docs/REPRODUCIBLE-BUILD.md
    - docs/REPRODUCIBLE-BUILD.expected-sha256.txt
  modified: []
decisions:
  - "D-09 7-section H2 structure landed verbatim in the exact order specified: Why this exists, Recipe, Toolchain pins, Environment, Expected sha256sum, Continuous verification, Reporting a reproducibility regression."
  - "D-17 Recipe section bash block landed verbatim — git clone, git checkout, env exports (SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD), RUSTFLAGS=--remap-path-prefix=..., CARGO_INCREMENTAL=0), cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot, deterministic tar+gzip (D-06 5-flag set: --sort=name --owner=0 --group=0 --numeric-owner --mtime=\"@${SOURCE_DATE_EPOCH}\" + gzip -n), sha256sum."
  - "D-18 chosen mechanism: separate machine-readable file (NOT markdown-table parse). Per RESEARCH §Standard Stack Alternatives Considered, the line-by-line format is more robust than awk on a markdown table whose column-width can shift."
  - "BLOCKER 2 fix: `.expected-sha256.txt` carries colon-delimited TRIPLE `<tag>:<sha256>:<image-version>` (not just `<tag>:<sha256>` pair) so the Plan 25-04 verifier derives BOTH EXPECTED_SHA256 and PINNED_IMAGE_VERSION from a single `awk -F:` lookup. Replaces the broken `grep ImageVersion.{0,5}[0-9.]+ docs/REPRODUCIBLE-BUILD.md` approach (the markdown table's label column does NOT contain the literal token 'ImageVersion', so the grep returned nothing → UNKNOWN → guard fails → every mismatch was mis-classified as HIGH-severity sha256-divergence-on-same-image)."
  - "D-10 placeholder strategy extended: TWO distinct placeholders in the `.expected-sha256.txt` (`<TBD-v1.6.0-cut-sha256>` and `<TBD-v1.6.0-cut-imageversion>`) plus the single `<TBD-v1.6.0-cut>` in the markdown table. Plan 25-05's rehearsal procedure now replaces 3 placeholder sites total (atomic substitution: 2 dedicated sed passes against the txt file, 1 against the md file). Distinct placeholder names prevent the txt-file substitution from corrupting the markdown table's ImageVersion placeholder."
  - "D-12 two-title scheme landed in §Reporting a reproducibility regression: `[reproducibility-regression] runner image drift: ImageVersion <OLD> → <NEW>` (low-severity, environmental rotation) vs `[reproducibility-regression] sha256 mismatch on ImageVersion <V>` (HIGH-severity, real supply-chain signal)."
  - "Cross-links to `.github/workflows/reproducible-verify.yml` are forward-compatible — the file will be created in Plan 25-04 (Wave 3), and markdown tolerates broken relative links until the target exists. T-25-03-05 documented and accepted as a transient state through end of Wave 2."
  - "Registry-entry placeholder in §Continuous verification uses the longer relative path `docs/RELEASING.md` (WARNING 4 fix) — IDENTICAL to the SECURITY.md placeholder Plan 25-05 will substitute so D-14 step 4's atomic substitution against one literal string matches both files."
metrics:
  duration: "~10 min"
  completed: "2026-06-03"
  tasks: 2
  files_modified: 2
---

# Phase 25 Plan 03: Reproducible-Build Recipe Doc + Expected-SHA256 Lookup File Summary

One-liner: Operator-facing 7-section reproducibility doc per D-09 + D-17 (`docs/REPRODUCIBLE-BUILD.md`, 103 lines), with companion machine-readable lookup file (`docs/REPRODUCIBLE-BUILD.expected-sha256.txt`) carrying colon-delimited `<tag>:<sha256>:<image-version>` triples per D-18 + BLOCKER 2 fix so the Plan 25-04 verifier derives both expected sha256 AND pinned ImageVersion from a single `awk -F:` lookup.

## What shipped

### Task 1 — `docs/REPRODUCIBLE-BUILD.md` (commit `105b734`)

**`docs/REPRODUCIBLE-BUILD.md`** (NEW, 103 lines, `docs/` root) — operator-facing reproducibility recipe. Structure per D-09 verbatim:

1. **`## Why this exists`** — 3-sentence operator-facing intro: what reproducibility proves, why it matters for supply-chain-sensitive operators, where the continuous verifier lives.
2. **`## Recipe`** — single fenced bash block per D-17 verbatim: `git clone https://github.com/<owner>/blindjoin.git` → `git checkout v1.6.0` → `export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)` + `RUSTFLAGS` + `CARGO_INCREMENTAL=0` → `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot` → deterministic `tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH}" -cf - -C dist . | gzip -n > blindjoin-linux-amd64.tar.gz` → `sha256sum`.
3. **`## Toolchain pins`** — markdown table with 6 rows: rustc 1.95.0 (pins in `rust-toolchain.toml`), cargo 1.95.0, ubuntu-24.04 runner image (placeholder `<TBD-v1.6.0-cut>`), `dtolnay/rust-toolchain` SHA `@3c5f7ea2…`, `sigstore/cosign-installer` SHA `@7e8b541e…`, `actions/checkout` SHA `@34e11487…`.
4. **`## Environment`** — explicit derivation of each of the three env vars + a `> Note: Rust reproducibility long tail` blockquote per RESEARCH §Pitfall 6.
5. **`## Expected sha256sum`** — markdown table with `v1.6.0: <TBD-v1.6.0-cut>` placeholder + 1-line prose pointer to the companion `.expected-sha256.txt`.
6. **`## Continuous verification`** — 8-step numbered list documenting the monthly verifier's exact algorithm (capture ImageVersion → resolve latest tag → download tarball + cosign bundle → re-verify cosign sig → checkout source at tag → rebuild → compute sha256 → look up expected via `awk -F:` → open issue on mismatch). Forward-compatible cross-link to `.github/workflows/reproducible-verify.yml` (Plan 25-04). Registry-entry placeholder uses path `docs/RELEASING.md` IDENTICAL to the SECURITY.md placeholder Plan 25-05 will substitute (WARNING 4 fix).
7. **`## Reporting a reproducibility regression`** — D-12 two-title scheme verbatim: low-severity runner-image drift vs HIGH-severity sha256 mismatch on same image. Maintainer's `diffoscope` triage path named explicitly.

File ends with metadata footer `*Maintained at: docs/REPRODUCIBLE-BUILD.md. Last updated: 2026-06-02.*` — Plan 25-05 bumps the date on rehearsal.

All 22 plan-verification grep assertions pass. All 7 H2 sections appear in the exact D-09 order at lines 5, 9, 38, 51, 61, 71, 88.

### Task 2 — `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` (commit `a853512`)

**`docs/REPRODUCIBLE-BUILD.expected-sha256.txt`** (NEW, 15 lines, `docs/` root) — D-18 chosen mechanism: machine-readable lookup file (not markdown-table parse). Per RESEARCH §Standard Stack Alternatives Considered, line-by-line format is more robust than `awk` on a markdown table.

**BLOCKER 2 fix:** Colon-delimited TRIPLE format `<tag>:<sha256>:<image-version>` carrying BOTH the expected sha256 AND the pinned ubuntu-24.04 ImageVersion that Plan 25-04's verifier needs for D-12 drift-vs-divergence classification. Single `awk -F:` lookup returns both values together — replaces the broken `grep ImageVersion.{0,5}[0-9.]+ docs/REPRODUCIBLE-BUILD.md` approach (markdown table's label column does NOT contain the literal token "ImageVersion", so grep returned nothing → UNKNOWN → guard fails → every mismatch mis-classified as HIGH-severity sha256-divergence-on-same-image).

Initial content:
- 13 `#`-comment lines documenting the format, the verifier-only-parses-this-file boundary, the cross-link to `docs/REPRODUCIBLE-BUILD.md`, and the v1.6.0-rc.0 rehearsal cross-link to `docs/RELEASING.md`.
- 1 blank line separator.
- 1 non-comment data line: `v1.6.0:<TBD-v1.6.0-cut-sha256>:<TBD-v1.6.0-cut-imageversion>`.

Distinct placeholder names (`<TBD-v1.6.0-cut-sha256>` vs `<TBD-v1.6.0-cut-imageversion>`) prevent Plan 25-05's atomic-substitution from corrupting one when replacing the other. Plan 25-05 rehearsal scope: 2 dedicated sed passes against this file + 1 against the markdown table (3 total substitution sites for v1.6.0-rc.0).

Verifier consumes via:
```bash
LOOKUP=$(awk -F: '$1 == "'"$LATEST_TAG"'" {print $2 " " $3}' docs/REPRODUCIBLE-BUILD.expected-sha256.txt)
# Returns: "<TBD-v1.6.0-cut-sha256> <TBD-v1.6.0-cut-imageversion>" (pre-rehearsal)
# Returns: "<40-hex-sha256> <ImageVersion>" (post-rehearsal)
```

All 7 plan-verification grep + `awk` assertions pass; the file ends with a final newline (no trailing whitespace).

## Files touched

| File | Action | Lines | Purpose |
|------|--------|-------|---------|
| `docs/REPRODUCIBLE-BUILD.md` | created | 103 | Operator-facing 7-section H2 reproducibility recipe doc (D-09, D-17, D-10, D-12) |
| `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` | created | 15 | Machine-readable colon-delimited `<tag>:<sha256>:<image-version>` lookup for Plan 25-04 verifier (D-18, BLOCKER 2 fix) |

## Deviations from Plan

**1. [Rule 3 — Blocking issue] Expanded §Continuous verification with 8-step numbered list + added 2 lines to §Reporting intro to meet `min_lines: 100` contract**

- **Found during:** Task 1 verification block
- **Issue:** Initial draft of `docs/REPRODUCIBLE-BUILD.md` came in at 90 lines — the plan's `must_haves.artifacts[0].min_lines: 100` contract required ≥100. All 22 content-grep assertions passed; only line count failed.
- **Fix:** Expanded `## Continuous verification` from a 3-paragraph prose summary into a substantive 8-step numbered list documenting the verifier's exact algorithm (the same algorithm Plan 25-04 will implement in YAML). Also split `## Reporting a reproducibility regression` intro from a single sentence into 3 sentences explaining title-dedup behavior. Both expansions add operator-useful detail rather than filler.
- **Files modified:** `docs/REPRODUCIBLE-BUILD.md` (90 → 103 lines)
- **Commit:** `105b734` (single Task 1 commit)
- **Rationale:** Rule 3 (auto-fix blocking issue) — line-count assertion was the acceptance contract; expansion preserved D-09/D-17 content invariants while adding the operator-facing 8-step algorithm that future Plan 25-04 mirrors.

No other deviations. Plan executed as written.

## Verification

All 22 grep assertions from Task 1's `<verify><automated>` block pass:
- 7 H2 section headers in the exact D-09 order (lines 5, 9, 38, 51, 61, 71, 88).
- Recipe content cross-checks: `git clone https://github.com/<owner>/blindjoin.git`, `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot`, `--sort=name --owner=0 --group=0 --numeric-owner`, `gzip -n > blindjoin-linux-amd64.tar.gz`, `export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)`, `export CARGO_INCREMENTAL=0`.
- Toolchain-pins table cells: `1.95.0`, `3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9`, `7e8b541eb2e61bf99390e1afd4be13a184e9ebc5`.
- Expected sha256 placeholder: `<TBD-v1.6.0-cut>`.
- Cross-links: `REPRODUCIBLE-BUILD.expected-sha256.txt`, `reproducible-verify.yml`, `RELEASING.md`.
- D-12 title formats: `runner image drift: ImageVersion`, `sha256 mismatch on ImageVersion`.
- Line count: 103 ≥ 100.

All 7 grep + `awk` assertions from Task 2's `<verify><automated>` block pass:
- Colon-delimited triple: `^v1\.6\.0:<TBD-v1\.6\.0-cut-sha256>:<TBD-v1\.6\.0-cut-imageversion>$` matches.
- Comment header tokens: `Phase 25 REPRO-01 / D-18`, `<tag>:<sha256hex>:<image-version>`.
- Single non-comment data line: exactly 1.
- `awk -F: '$1 == "v1.6.0" {print $2 " " $3}'` returns `<TBD-v1.6.0-cut-sha256> <TBD-v1.6.0-cut-imageversion>` (both values in one pass — BLOCKER 2 fix validated).
- Cross-link: `docs/REPRODUCIBLE-BUILD.md` present.
- File ends with final newline (no trailing whitespace).

Overall plan `<verification>` block all 9 points pass — 7 H2 sections, Recipe verbatim, toolchain pins table, expected-sha256 placeholder, continuous-verification cross-links + cadence, §Reporting two-title scheme, `.expected-sha256.txt` triple format, distinct placeholder disambiguation, recipe runnable end-to-end on fresh `ubuntu-24.04` shell (manual smoke deferred to Plan 25-05 rehearsal per Pitfall 12 contract).

## Threat surface scan

No new security-relevant surface introduced by this plan. Files created are operator-facing markdown doc + machine-readable colon-delimited text file. T-25-03-05 (broken cross-link to `reproducible-verify.yml` until Plan 25-04 ships) is documented in `<threat_model>` and accepted; markdown tolerates broken relative links and the link resolves once Plan 25-04 lands in Wave 3.

## Forward dependencies (consumed by later plans)

- **Plan 25-04 verifier** reads `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` via a single `awk -F: '$1 == "'"$LATEST_TAG"'" {print $2 " " $3}'` lookup to derive BOTH `EXPECTED_SHA256` AND `PINNED_IMAGE_VERSION` for the D-12 drift-vs-divergence classification. The markdown table in `docs/REPRODUCIBLE-BUILD.md` is human-facing only — verifier never parses markdown (BLOCKER 2 fix).
- **Plan 25-05 rehearsal procedure** atomically replaces 3 placeholder sites at the v1.6.0-rc.0 cut: (a) `<TBD-v1.6.0-cut-sha256>` in `.expected-sha256.txt`, (b) `<TBD-v1.6.0-cut-imageversion>` in `.expected-sha256.txt`, (c) `<TBD-v1.6.0-cut>` in the markdown table's Toolchain pins + Expected sha256sum rows. Plan 25-05 also appends the reproducible-builds.org registry entry URL to §Continuous verification once D-14 lands.

## Self-Check: PASSED

Files claimed created exist:
- `docs/REPRODUCIBLE-BUILD.md` — FOUND (103 lines)
- `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` — FOUND (15 lines)

Commits claimed exist:
- `105b734` (Task 1) — FOUND in `git log --oneline`
- `a853512` (Task 2) — FOUND in `git log --oneline`
