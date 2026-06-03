---
phase: 25-reproducible-build-recipe-scheduled-verifier-registry
plan: 01
subsystem: build-pipeline
tags: [rust-toolchain, profile-release, reproducibility, ci-pin-gate]
requires: []
provides:
  - rust-toolchain.toml-channel-1.95.0
  - profile-release-strip-symbols
  - rust-toolchain-pin-check-ci-gate
affects:
  - .planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-02-PLAN.md (release.yml determinism env inherits the pin)
  - .planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-03-PLAN.md (doc Recipe section names 1.95.0)
  - .planning/phases/25-reproducible-build-recipe-scheduled-verifier-registry/25-04-PLAN.md (verifier rebuild step uses the pin)
tech-stack:
  added: []
  patterns:
    - "rust-toolchain.toml at workspace root as the canonical Rust pin (Pitfall A KEEP-with-pin-match)"
    - "workspace-level [profile.release] strip=\"symbols\" applies to all four crate binaries uniformly"
    - "rust-toolchain-pin-check CI gate mirrors bip322-pin-check shape (one job per pinned dep)"
key-files:
  created:
    - rust-toolchain.toml
  modified:
    - Cargo.toml
    - .github/workflows/release.yml
    - .github/workflows/ci.yml
decisions:
  - "Followed RESEARCH Pitfall A KEEP-with-pin-match (overriding CONTEXT D-21 which had instructed removing the with: toolchain: input). The dtolnay/rust-toolchain action.yml at the project-pinned SHA `3c5f7ea2…` explicitly exits 1 with `'toolchain' is a required input` when the input is empty; verified at planning time 2026-06-02."
  - "Paraphrased the action name as 'the toolchain action' in the new rust-toolchain-pin-check job's comment block and in the 6 single-line per-block comments — applies the Plan 22-04 / Plan 24-01 forbidden-token-paraphrasing discipline preemptively (the literal dtolnay/rust-toolchain token already appears in `uses:` lines and is not a file-level forbidden token here, but discipline is established)."
  - "Paraphrased the prior unpinned channel value ('stable') everywhere in the new comment text: per the Plan 22-04 paraphrasing discipline, `toolchain: stable` does NOT appear in any active YAML or comment line in release.yml or ci.yml after this plan."
metrics:
  duration: "~6 min"
  completed: "2026-06-03"
  tasks: 2
  files_modified: 4
---

# Phase 25 Plan 01: Toolchain Pin Foundation Summary

One-liner: Land the Rust toolchain pin foundation that every subsequent Phase 25 plan depends on — `rust-toolchain.toml` at workspace root (channel 1.95.0), `[profile.release] strip="symbols"` in Cargo.toml, all 6 dtolnay/rust-toolchain `with: toolchain:` blocks aligned to `"1.95.0"`, and a new `rust-toolchain-pin-check` CI gate that fails the PR on any drift.

## What shipped

### Task 1 — rust-toolchain.toml + Cargo.toml [profile.release] (commit `b9da7b5`)

**rust-toolchain.toml** (NEW, 11 lines, workspace root) per RESEARCH §Code Examples Example 1 verbatim:

```toml
[toolchain]
channel = "1.95.0"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

Header `#` comment block cross-references the `rust-toolchain-pin-check` CI gate added in Task 2. `rustup` natively respects the file for any `cargo`/`rustc` invocation within the workspace — local dev + the future Plan 25-04 verifier rebuild both auto-resolve to 1.95.0 from a fresh clone.

**Cargo.toml** (MODIFIED, +12 lines net) — appended `[profile.release]` block AFTER `[workspace.dependencies]` at the prior line 36 (`proptest = "1"`). Insertion lands at **L38-46** (header comment L38-44, block header L45, body L46). Block body is single-directive `strip = "symbols"` only — no `lto`, no `codegen-units`, no `opt-level` (D-07 scoped strictly to strip).

Verification: `cargo metadata` resolved successfully on maintainer machine (proves rustup auto-resolved 1.95.0 from the new file without spending 30-60s on a full release build).

### Task 2 — 6 with: toolchain: blocks + new pin-check gate (commit `7ef4631`)

**Six `with: toolchain:` blocks aligned to `"1.95.0"`** with a single-line Pitfall-A comment above each `with:`:

| File | Block | Pre-edit line | Post-edit `toolchain:` line | Job |
|------|-------|---------------|------------------------------|-----|
| `.github/workflows/release.yml` | 1 | L39 (`toolchain: stable`) | **L41** (`toolchain: "1.95.0"`) | `check` |
| `.github/workflows/release.yml` | 2 | L92 (`toolchain: stable`) | **L96** (`toolchain: "1.95.0"`) | `build` |
| `.github/workflows/ci.yml` | 3 | L34 (`toolchain: stable`) | **L36** (`toolchain: "1.95.0"`) | `test` |
| `.github/workflows/ci.yml` | 4 | L142 (`toolchain: stable`) | **L146** (`toolchain: "1.95.0"`) | `clippy` |
| `.github/workflows/ci.yml` | 5 | L163 (`toolchain: stable`) | **L169** (`toolchain: "1.95.0"`) | `coordinator-smoke` |
| `.github/workflows/ci.yml` | 6 | L177 (`toolchain: stable`) | **L185** (`toolchain: "1.95.0"`) | `audit` |

Each comment block (1 file line each) cites `Pitfall A: the toolchain action requires an explicit toolchain: input. Pin value matches rust-toolchain.toml channel via the rust-toolchain-pin-check gate in ci.yml.`

**New `rust-toolchain-pin-check` job in `.github/workflows/ci.yml`** at **L246-302** (job header L246, gate run-step L268-302). Inserted between `bip322-pin-check` (ends at L244, prior L236) and `crit-01-grep-check` (now at L304, prior L238). Job structure mirrors `bip322-pin-check` exactly:

- `runs-on: ubuntu-latest` (CI jobs stay on ubuntu-latest per CONTEXT D-08 explicit — only the release.yml `build` job and the future Plan 25-04 reproducible-verify.yml pin `ubuntu-24.04`).
- Multi-line `#` comment block (L249-265) citing RESEARCH Pitfall A, the dual-source-of-truth risk, and the bip322-pin-check shape inheritance.
- Steps: `actions/checkout@34e114876b…  # v4.3.1` (verbatim reuse of project SHA pin — covered automatically by Phase 23 `sigstore-pin-check` at ci.yml:357-391); then `Enforce rust-toolchain.toml single source of truth` run-step that:
  1. Extracts the channel value: `EXPECTED=$(grep -oE 'channel = "[^"]+"' rust-toolchain.toml | grep -oE '"[^"]+"' | tr -d '"')`
  2. Walks every `.github/workflows/*.yml`, greps for `^\s*toolchain:\s*"[^"]+"` lines (skips comments naturally — the regex requires `toolchain:` after only leading whitespace)
  3. Compares each found value against `$EXPECTED`; emits `ERROR: $FILE:$LINE_NUM — toolchain pin '$ACTUAL' != rust-toolchain.toml channel '$EXPECTED'` to stderr + counts drift
  4. Exits 1 if any drift found; otherwise echoes `OK: 6 with: toolchain: block(s) match rust-toolchain.toml channel '1.95.0'`

Self-run locally: extracted `EXPECTED=1.95.0`, found 6 matching blocks, drift count 0. Gate would PASS on the post-Task-2 tree.

## Verification (overall plan `<verification>` block)

| # | Check | Result |
|---|-------|--------|
| 1 | `rust-toolchain.toml` exists with channel/profile/components | PASS |
| 2 | `Cargo.toml` has `[profile.release]` + `strip = "symbols"` | PASS |
| 3 | `release.yml` has exactly 2 `toolchain: "1.95.0"` occurrences | PASS (2) |
| 4 | `ci.yml` has exactly 4 `toolchain: "1.95.0"` occurrences | PASS (4) |
| 5 | `rust-toolchain-pin-check` job lives between `bip322-pin-check` and `crit-01-grep-check` | PASS (L246, between L222 and L304) |
| 6 | No `toolchain: stable` literal in active YAML or comments | PASS |
| 7 | Both YAML files parse with `python3 -c "import yaml; yaml.safe_load(...)"` | PASS |
| 8 | `cargo metadata` resolves successfully on a clean local checkout | PASS |
| 9 | New pin-check gate script runs locally without error (extracted channel == "1.95.0") | PASS |

Existing CI gates verified unaffected post-edit:

- `bip322-pin-check` — still present
- `sigstore-pin-check` — still present (covers the new gate's `actions/checkout` SHA pin automatically per Phase 23)
- `crit-01-grep-check` — still present
- `crit-01-client-grep-check` — still present
- `corepc-node-feature-pin-check` — still present

## Deviations from Plan

### Auto-fixed Issues

None — plan executed as written with no Rule 1/2/3 deviations.

### Notes for downstream plans

**1. Paraphrasing discipline applied preemptively (Plan 22-04 / Plan 24-01 pattern).**
- In the new `rust-toolchain-pin-check` job's comment block, the dtolnay action is referred to as "the toolchain action" rather than the literal `dtolnay/rust-toolchain`. (The literal is not file-level-forbidden in workflow YAMLs — it appears in many `uses:` lines — but the discipline keeps the comment text portable if a future audit gate ever introduces the literal as forbidden.)
- In the 6 per-block comments, the phrasing is "the toolchain action requires an explicit toolchain: input" rather than naming `dtolnay/rust-toolchain` directly. Same rationale.
- `toolchain: stable` (the prior unpinned channel alias) does NOT appear in any line of `release.yml` or `ci.yml` after this plan — neither active YAML nor any comment.

**2. The new `rust-toolchain-pin-check` job is `runs-on: ubuntu-latest`, NOT `ubuntu-24.04`.** CI grep gates have no byte-equality requirement; runner-image rotation is acceptable. Only the future Plan 25-02 release.yml `build` job and the future Plan 25-04 reproducible-verify.yml pin `ubuntu-24.04` (per CONTEXT D-08 + D-11 explicit).

**3. Inserted-job location guaranteed downstream-plan-stable.** The new job is between `bip322-pin-check` (ends now at L244) and `crit-01-grep-check` (now at L304). Subsequent Phase 25 plans that touch ci.yml (none currently planned — the verifier is a new workflow file in Plan 25-04) inherit the same boundary.

## Authentication gates

None.

## Known Stubs

None — both files modified in this plan are workflow YAML / TOML config; no UI rendering or data sources involved.

## Self-Check: PASSED

- [x] `rust-toolchain.toml` exists at workspace root (FOUND)
- [x] `Cargo.toml` modified — `[profile.release]` block present (FOUND at L45)
- [x] `.github/workflows/release.yml` modified — 2 `toolchain: "1.95.0"` blocks (FOUND at L41, L96)
- [x] `.github/workflows/ci.yml` modified — 4 `toolchain: "1.95.0"` blocks + new pin-check job (FOUND at L36, L146, L169, L185; job at L246)
- [x] Task 1 commit `b9da7b5` (FOUND via `git log --oneline -3`)
- [x] Task 2 commit `7ef4631` (FOUND via `git log --oneline -3`)
