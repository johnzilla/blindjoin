# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor manual.

This file documents the post-CI portion of the release cycle: pre-flight verifying the CI-built artifacts, and capturing the reproducibility baseline before the first tag in a new toolchain/runner generation.

## Prerequisites

- **gh 2.50+** on the maintainer's machine. Install via [cli.github.com](https://cli.github.com).
- **cosign 2.6.3+** for the pre-flight verify gate (see [Pre-flight check](#pre-flight-check-after-ci-completes)). Install:
  ```bash
  brew install cosign          # macOS
  # or download from https://github.com/sigstore/cosign/releases
  ```

## Per-release procedure

CI handles the signing + attestation; the maintainer drives the tag cut and the pre-flight verify. After Phase 25 (v1.6.0+), every tag push produces a non-draft GitHub Release immediately — no manual draft-flip step.

1. **Cut and push the tag.** Use signed tags so the maintainer-side commit identity is bound to the release. The tag MUST use 3-part semver (`vX.Y.Z`, not `vX.Y`) per [CONTRIBUTING.md `## Tagging releases`](../CONTRIBUTING.md#tagging-releases).

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push --tags
   ```

2. **Watch `release.yml` until green.** Open [`release.yml`](../.github/workflows/release.yml) in the Actions tab. CI creates a published GitHub Release with 4 assets: `blindjoin-linux-amd64.tar.gz`, `blindjoin-linux-amd64.tar.gz.sha256`, `blindjoin-linux-amd64.tar.gz.bundle` (cosign signature), and `blindjoin-linux-amd64.tar.gz.sigstore` (SLSA provenance). If the workflow fails, fix the underlying issue and re-cut the tag — do NOT proceed.

3. **Run the [Pre-flight check](#pre-flight-check-after-ci-completes) below.** If any verify fails, recover via `gh release delete vX.Y.Z` and re-cut the tag after the fix.

4. **Verify the published release.** Once CI is green and pre-flight passes, the release is already published — operators can `gh release download vX.Y.Z` immediately.

## Pre-flight check after CI completes

After CI completes, cosign-verify the CI-produced assets against the documented identity-regexp. This catches a misconfigured workflow, a missing permission, or a stale CI image BEFORE operators pull the release.

```bash
# Download all 4 CI-produced assets to a fresh directory.
mkdir -p /tmp/blindjoin-release && cd /tmp/blindjoin-release
gh release download vX.Y.Z --dir .

# 1. Cosign blob signature verifies against the release.yml identity-regexp.
cosign verify-blob \
  --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz

# 2. SLSA provenance attestation verifies against the same identity-regexp.
cosign verify-attestation \
  --bundle blindjoin-linux-amd64.tar.gz.sigstore \
  --type slsaprovenance \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz

# 3. Optional: GitHub Attestations API path (requires github.com reachable).
gh attestation verify blindjoin-linux-amd64.tar.gz --repo <owner>/blindjoin
```

Both `cosign verify-blob` and `cosign verify-attestation` MUST return "Verified OK" (exit 0). If either fails, recover via `gh release delete vX.Y.Z`, fix the underlying issue, and re-push the tag.

## Reproducibility baseline (first release after a Rust/runner bump)

Before tagging, dispatch `.github/workflows/reproducible-verify.yml` from the Actions tab. The first run will fail because the workflow's `EXPECTED_SHA256` env still holds `<TBD-v1.6.0-cut>`. Copy the `Rebuilt locally:` sha256 from the log into:

1. The `EXPECTED_SHA256:` env in `.github/workflows/reproducible-verify.yml`.
2. The `v1.6.0` row in `docs/REPRODUCIBLE-BUILD.md` §Expected sha256sum.

Commit, then tag.
