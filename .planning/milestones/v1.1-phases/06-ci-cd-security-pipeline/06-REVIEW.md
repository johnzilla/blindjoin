---
phase: 06-ci-cd-security-pipeline
reviewed: 2026-04-09T00:00:00Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - .github/workflows/ci.yml
  - .github/workflows/docker.yml
  - .github/workflows/release.yml
  - docs/branch-protection.md
findings:
  critical: 1
  warning: 3
  info: 2
  total: 6
status: issues_found
---

# Phase 06: Code Review Report

**Reviewed:** 2026-04-09
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Three GitHub Actions workflow files and one setup guide were reviewed. The workflows provide CI gating on PRs (test, clippy, audit), Docker image builds on tag push, and binary release builds on tag push. The overall structure is sound and follows reasonable security practices with scoped permissions. However, there is one critical issue — third-party actions are pinned to mutable floating version tags rather than immutable commit SHAs, creating a supply-chain attack vector. Three additional warnings cover logic gaps in the release workflow. Two info items flag minor quality points.

## Critical Issues

### CR-01: Third-party actions pinned to mutable version tags, not commit SHAs

**File:** `.github/workflows/ci.yml:14`, `.github/workflows/docker.yml:44,50,52,60`, `.github/workflows/release.yml:49`

**Issue:** All third-party GitHub Actions are referenced by floating version tags (e.g., `@v4`, `@v2`, `@v5`, `@v7`, `@v1`). A tag can be moved by the action author at any time to point to different — potentially malicious — code. Since these workflows run with `GITHUB_TOKEN` that has `packages: write` and `contents: write` permissions, a compromised action could exfiltrate the token, push malicious images to GHCR, or tamper with release artifacts. This is a supply-chain attack surface.

Affected action references:
- `actions/checkout@v4` (ci.yml:12, docker.yml:14,43, release.yml:15,32)
- `dtolnay/rust-toolchain@stable` (ci.yml:13, docker.yml:16, release.yml:17,34)
- `Swatinem/rust-cache@v2` (ci.yml:14, docker.yml:19, release.yml:20,35)
- `docker/login-action@v4` (docker.yml:44)
- `docker/setup-buildx-action@v4` (docker.yml:50)
- `docker/metadata-action@v5` (docker.yml:52)
- `docker/build-push-action@v7` (docker.yml:60)
- `softprops/action-gh-release@v1` (release.yml:49)

**Fix:** Pin every third-party action to an immutable commit SHA. Use a tool like `pin-github-actions` or `actionlint` with SHA pinning to automate this. Example for the most sensitive job (release):

```yaml
# Before
- uses: softprops/action-gh-release@v1

# After (fetch the current SHA with: gh api /repos/softprops/action-gh-release/git/ref/tags/v1)
- uses: softprops/action-gh-release@da2d9d0fffeb2faab4fd33620c17c6a3ff9cd190 # v1.0.0
```

Priority order for pinning (highest risk first, due to permission scope):
1. `softprops/action-gh-release` — runs with `contents: write`
2. `docker/*` actions — run with `packages: write`
3. `actions/checkout`, `dtolnay/rust-toolchain`, `Swatinem/rust-cache` — lower risk but still should be pinned

## Warnings

### WR-01: `cargo audit` not run before Docker push or Release binary upload

**File:** `.github/workflows/docker.yml:26-27`, `.github/workflows/release.yml:25-26`

**Issue:** Both `docker.yml` and `release.yml` have a `check` job that runs `cargo test` and `cargo clippy`, but neither runs `cargo audit`. This means a tag push can produce and publish Docker images or release binaries that contain dependencies with known high/critical CVEs — even though `cargo audit` is enforced on PRs. A vulnerability introduced between a PR merge and a tag push, or discovered after merging, would slip through.

**Fix:** Add a `cargo audit` step to the `check` job in both `docker.yml` and `release.yml`:

```yaml
- name: Install cargo-audit
  run: cargo install cargo-audit --locked
- name: Run audit (deny high and critical)
  run: cargo audit --deny high --deny critical
```

### WR-02: Release artifacts uploaded without checksums or signatures

**File:** `.github/workflows/release.yml:48-53`

**Issue:** The `blindjoin-linux-amd64.tar.gz` archive is uploaded to GitHub Releases with no accompanying SHA-256 checksum file or detached signature. Users cannot verify download integrity without trusting the GitHub CDN end-to-end. For a security-sensitive tool (Bitcoin CoinJoin coordinator), artifact integrity verification is important.

**Fix:** Generate and upload a checksum file alongside the archive:

```yaml
- name: Package
  run: |
    mkdir -p dist
    cp target/release/coordinator dist/
    cp target/release/client dist/
    cp target/release/liquidity-bot dist/
    tar czf blindjoin-linux-amd64.tar.gz -C dist .
    sha256sum blindjoin-linux-amd64.tar.gz > blindjoin-linux-amd64.tar.gz.sha256

- name: Upload to GitHub Releases
  uses: softprops/action-gh-release@v1
  with:
    files: |
      blindjoin-linux-amd64.tar.gz
      blindjoin-linux-amd64.tar.gz.sha256
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### WR-03: `docker.yml` has no `permissions` block at the job level — `check` job inherits overly broad `packages: write`

**File:** `.github/workflows/docker.yml:7-9,11-24`

**Issue:** The top-level `permissions` block grants `packages: write` for the entire workflow. The `check` job (which only runs tests and clippy) does not need registry write access. If the `check` job were compromised via a malicious action, it would hold unnecessary credentials. GitHub Actions supports per-job permission scoping.

**Fix:** Move the elevated permission to only the `docker` job:

```yaml
# At workflow level — minimal default
permissions:
  contents: read

jobs:
  check:
    # No additional permissions needed
    ...

  docker:
    permissions:
      contents: read
      packages: write
    ...
```

## Info

### IN-01: `dtolnay/rust-toolchain@stable` uses a moving toolchain channel

**File:** `.github/workflows/ci.yml:13`, `.github/workflows/docker.yml:17`, `.github/workflows/release.yml:17,34`

**Issue:** The `stable` channel resolves to whatever Rust stable is on the day the job runs. This means CI behavior can silently change when a new Rust stable is released — new lints enabled by `-D warnings`, new clippy lints, or behavior changes. This is not a bug today but can cause unexpected CI failures or mask issues that only appear on a specific toolchain.

**Fix:** Pin to a specific toolchain version for reproducibility, or add `rust-toolchain.toml` to the repo root:

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.86.0"  # or current stable at project creation time
```

This also self-documents the minimum supported Rust version (MSRV).

### IN-02: `branch-protection.md` status check names may not match job `name:` fields

**File:** `docs/branch-protection.md:31-37`

**Issue:** The branch protection guide instructs adding status checks named `cargo test`, `cargo clippy`, and `cargo audit`. However, GitHub's required status check names are derived from the job `name:` field in the workflow, which are `cargo test`, `cargo clippy`, and `cargo audit` (lines 9, 20, 31 of `ci.yml`) — these do match. This is informational only: if the job names in `ci.yml` are ever renamed, the branch protection rules will silently stop blocking merges without any warning until someone notices checks are no longer required.

**Fix:** Add a note in the doc reminding maintainers to update branch protection rules if CI job names change, and consider adding a comment in `ci.yml` cross-referencing the branch protection dependency:

```yaml
  test:
    name: cargo test  # NOTE: this name is referenced in branch protection rules — do not rename without updating
```

---

_Reviewed: 2026-04-09_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
