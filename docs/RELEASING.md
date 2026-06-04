# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor manual.

This file documents the post-CI portion of the release cycle: pre-flight verifying the CI-built artifacts.

## Prerequisites

- **gh 2.50+** on the maintainer's machine. Install via [cli.github.com](https://cli.github.com). The `gh attestation verify` pre-flight subcommand requires gh 2.49+; 2.50+ is the safe floor.

## Per-release procedure

CI handles the signing + attestation; the maintainer drives the tag cut and the pre-flight verify. After Phase 25 (v1.6.0+), every tag push produces a non-draft GitHub Release immediately — no manual draft-flip step.

1. **Cut and push the tag.** Use signed tags so the maintainer-side commit identity is bound to the release. The tag MUST use 3-part semver (`vX.Y.Z`, not `vX.Y`) per [CONTRIBUTING.md `## Tagging releases`](../CONTRIBUTING.md#tagging-releases).

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push --tags
   ```

2. **Watch `release.yml` until green.** Open [`release.yml`](../.github/workflows/release.yml) in the Actions tab. CI creates a published GitHub Release with 3 assets: `blindjoin-linux-amd64.tar.gz`, `blindjoin-linux-amd64.tar.gz.sha256`, and `blindjoin-linux-amd64.tar.gz.sigstore` (SLSA provenance bundle). If the workflow fails, fix the underlying issue and re-cut the tag — do NOT proceed.

3. **Run the [Pre-flight check](#pre-flight-check-after-ci-completes) below.** If any verify fails, recover via `gh release delete vX.Y.Z` and re-cut the tag after the fix.

4. **Verify the published release.** Once CI is green and pre-flight passes, the release is already published — operators can `gh release download vX.Y.Z` immediately.

## Pre-flight check after CI completes

```bash
mkdir -p /tmp/blindjoin-release && cd /tmp/blindjoin-release
gh release download vX.Y.Z --dir . --pattern blindjoin-linux-amd64.tar.gz
gh attestation verify blindjoin-linux-amd64.tar.gz --owner <owner>
```

Must return a green checkmark / `Loaded ... attestations` + `verified` line. If not, `gh release delete vX.Y.Z`, fix, re-tag.

## Base-image digest check (before a release)

Before tagging, confirm the digests pinned in `docker/Dockerfile`'s two `FROM` lines still match upstream:

```bash
docker buildx imagetools inspect debian:bookworm-slim                    --format '{{.Manifest.Digest}}'
docker buildx imagetools inspect lukemathwalker/cargo-chef:latest-rust-1 --format '{{.Manifest.Digest}}'
```

If either differs from the matching `@sha256:...` in `docker/Dockerfile`, decide whether to bump (security backport → yes; arbitrary metadata churn → no), then update the digest portion of the relevant `FROM` line in a one-line PR.

