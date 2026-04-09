---
phase: 05-tor-release
plan: "03"
subsystem: infra
tags: [github-actions, docker, ghcr, cross-rs, cargo-chef, multi-arch, cicd, release]

# Dependency graph
requires:
  - phase: 05-tor-release
    provides: "Tor hidden service + PKARR coordinator (05-01, 05-02) — the binaries and images being released"
provides:
  - GitHub Actions release workflow: matrix builds for linux-amd64, linux-arm64, macos-amd64, macos-arm64
  - GitHub Actions Docker workflow: multi-arch images (linux/amd64 + linux/arm64) pushed to ghcr.io
  - docker/Dockerfile.client: cargo-chef multi-stage build for client binary
affects: [release, distribution, operators]

# Tech tracking
tech-stack:
  added:
    - softprops/action-gh-release@v1 (GitHub Releases upload action)
    - dtolnay/rust-toolchain@stable (Rust toolchain action)
    - Swatinem/rust-cache@v2 (Cargo build cache action)
    - cross-rs (aarch64 cross-compilation)
    - docker/login-action@v4 (GHCR authentication)
    - docker/setup-qemu-action@v4 (ARM64 emulation for Docker Buildx)
    - docker/setup-buildx-action@v4 (multi-arch Docker builds)
    - docker/metadata-action@v5 (semver image tagging)
    - docker/build-push-action@v7 (multi-arch image push)
  patterns:
    - cross-rs for Linux aarch64; native cargo for x86_64 and macOS runners
    - cargo-chef multi-stage Dockerfile pattern extended to client binary
    - docker/metadata-action@v5 for semver + latest tagging from git tags

key-files:
  created:
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
    - docker/Dockerfile.client
  modified: []

key-decisions:
  - "cross-rs via cargo install --git (HEAD) used for aarch64 Linux cross-compilation — no Cross.toml needed since arti-client uses rustls feature (no openssl-sys)"
  - "RESEARCH.md Pattern 5 had duplicate push: YAML keys — corrected to single push: with branches and tags as siblings"
  - "liquidity-bot excluded from release.yml (binary tarball) — Docker-only release for the bot"
  - "docker/metadata-action@v5 used for tags over hardcoded :latest — enables semver versioning automatically"

patterns-established:
  - "Pattern: Two GHA workflows — release.yml for binary tarballs (v* tags only), docker.yml for images (v* tags + main)"
  - "Pattern: fail-fast: false on all matrix strategies — partial failures don't abort the whole release"

requirements-completed: [DEPL-03, DEPL-04]

# Metrics
duration: 1min
completed: 2026-04-09
---

# Phase 5 Plan 03: GitHub Actions CI/CD Release Workflows Summary

**Matrix binary release (4 targets, cross-rs for ARM64) + multi-arch Docker image push to ghcr.io via cargo-chef Dockerfiles**

## Performance

- **Duration:** ~1 min
- **Started:** 2026-04-09T20:47:16Z
- **Completed:** 2026-04-09T20:48:25Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Release workflow builds coordinator + client for linux-amd64, linux-arm64, macos-amd64, macos-arm64 on v* tag push; uploads blindjoin-{name}.tar.gz to GitHub Releases
- Docker workflow builds coordinator, client, and liquidity-bot multi-arch images (linux/amd64 + linux/arm64) and pushes to ghcr.io on v* tags and main branch
- Dockerfile.client created using the same cargo-chef multi-stage pattern as Dockerfile.coordinator and Dockerfile.bot

## Task Commits

1. **Task 1: GitHub Actions release workflow for Linux + macOS binaries (DEPL-03)** - `1a721f4` (feat)
2. **Task 2: Docker multi-arch GHCR workflow + Dockerfile.client (DEPL-04)** - `71b5caa` (feat)

## Files Created/Modified

- `.github/workflows/release.yml` - Matrix CI (4 targets x 2 binaries), triggered on v* tag, cross-rs for aarch64, softprops/action-gh-release@v1 upload
- `.github/workflows/docker.yml` - Multi-arch Docker build (linux/amd64, linux/arm64) for 3 images, triggered on v* tags and main branch
- `docker/Dockerfile.client` - cargo-chef multi-stage Dockerfile producing client binary in debian:bookworm-slim runtime

## Decisions Made

- cross-rs installed from GitHub HEAD (`cargo install cross --git https://github.com/cross-rs/cross`) — no Cross.toml or OPENSSL_* env vars needed since arti-client uses `features = ["rustls"]` (no openssl-sys cross-compile issue)
- RESEARCH.md Pattern 5 had a YAML bug with duplicate `push:` keys under `on:` — corrected in docker.yml to use a single `push:` with `tags` and `branches` as sibling keys
- liquidity-bot excluded from release.yml binary tarballs — the bot is Docker-only (no standalone binary download needed)
- `docker/metadata-action@v5` used for image tagging, providing semver tags (vX.Y.Z, vX.Y, latest) automatically from git tags

## Deviations from Plan

None - plan executed exactly as written. The YAML duplicate-key note in Task 2 was already called out in the plan action ("NOTE on `on.push` with two keys"), so the correction was part of the spec.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required. The workflows use `GITHUB_TOKEN` (automatically provided by GitHub Actions) with minimum required permissions (`contents: write` for releases, `packages: write` for GHCR).

## Next Phase Readiness

- All three CI/CD artifacts are in place: release.yml, docker.yml, Dockerfile.client
- Pushing a `v0.1.0` tag will trigger both workflows simultaneously
- Phase 5 (tor-release) is complete — all 3 plans executed

---
*Phase: 05-tor-release*
*Completed: 2026-04-09*
