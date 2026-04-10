---
phase: 06-ci-cd-security-pipeline
fixed_at: 2026-04-09T00:00:00Z
review_path: .planning/phases/06-ci-cd-security-pipeline/06-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 06: Code Review Fix Report

**Fixed at:** 2026-04-09
**Source review:** .planning/phases/06-ci-cd-security-pipeline/06-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4
- Fixed: 4
- Skipped: 0

## Fixed Issues

### CR-01: Third-party actions pinned to mutable version tags, not commit SHAs

**Files modified:** `.github/workflows/ci.yml`, `.github/workflows/docker.yml`, `.github/workflows/release.yml`
**Commit:** 5193605
**Applied fix:** Replaced all floating version tag references with immutable commit SHAs plus version comments. Full mapping applied:
- `actions/checkout@v4` → `@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`
- `dtolnay/rust-toolchain@stable` → `@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable`
- `Swatinem/rust-cache@v2` → `@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2`
- `docker/login-action@v4` → `@4907a6ddec9925e35a0a9e82d7399ccc52663121 # v4.1.0`
- `docker/setup-buildx-action@v4` → `@4d04d5d9486b7bd6fa91e7baf45bbb4f8b9deedd # v4.0.0`
- `docker/metadata-action@v5` → `@c299e40c65443455700f0fdfc63efafe5b349051 # v5.10.0`
- `docker/build-push-action@v7` → `@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0`
- `softprops/action-gh-release@v1` → `@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`

All SHAs were resolved live via `gh api` at fix time. Annotated tag objects were dereferenced to their underlying commit SHAs.

### WR-01: `cargo audit` not run before Docker push or Release binary upload

**Files modified:** `.github/workflows/docker.yml`, `.github/workflows/release.yml`
**Commit:** bff2a70
**Applied fix:** Added two steps to the `check` job in both docker.yml and release.yml — `cargo install cargo-audit --locked` followed by `cargo audit --deny high --deny critical`. The `check` job is a required gate before either the `docker` or `build` job runs, so audit failure now blocks any tag-push publish.

### WR-02: Release artifacts uploaded without checksums or signatures

**Files modified:** `.github/workflows/release.yml`
**Commit:** bf78a1e
**Applied fix:** Added `sha256sum blindjoin-linux-amd64.tar.gz > blindjoin-linux-amd64.tar.gz.sha256` to the Package step, and extended the `softprops/action-gh-release` `files:` block to upload both the archive and the `.sha256` file as release assets.

### WR-03: `docker.yml` check job inherits overly broad `packages: write`

**Files modified:** `.github/workflows/docker.yml`
**Commit:** 88a2b94
**Applied fix:** Changed the workflow-level `permissions` block to `contents: read` only (removing `packages: write`). Added a job-level `permissions` block to the `docker` job granting `contents: read` and `packages: write`. The `check` job now runs with only `contents: read`, eliminating the unnecessary registry credential exposure.

---

_Fixed: 2026-04-09_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
