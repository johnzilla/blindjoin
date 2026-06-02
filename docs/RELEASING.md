# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor manual.

This file documents the post-CI portion of the release cycle: pre-flight verifying the CI-built artifacts, then flipping the GitHub Release out of draft.

## Prerequisites

- **gh 2.50+** on the maintainer's machine. Install via [cli.github.com](https://cli.github.com).
- **cosign 2.6.3+** for the pre-flight verify gate before flipping the release out of draft (see [Pre-flight check](#pre-flight-check-before-flipping-out-of-draft)). Install:
  ```bash
  brew install cosign          # macOS
  # or download from https://github.com/sigstore/cosign/releases
  ```

## Per-release procedure

CI handles the signing + attestation; the maintainer drives the tag cut, the pre-flight verify, and the draft flip.

1. **Cut and push the tag.** Use signed tags so the maintainer-side commit identity is bound to the release. The tag MUST use 3-part semver (`vX.Y.Z`, not `vX.Y`) per [CONTRIBUTING.md `## Tagging releases`](../CONTRIBUTING.md#tagging-releases).

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push --tags
   ```

2. **Watch `release.yml` until green.** Open [`release.yml`](../.github/workflows/release.yml) in the Actions tab. CI creates a DRAFT GitHub Release with 4 assets: `blindjoin-linux-amd64.tar.gz`, `blindjoin-linux-amd64.tar.gz.sha256`, `blindjoin-linux-amd64.tar.gz.bundle` (cosign signature), and `blindjoin-linux-amd64.tar.gz.sigstore` (SLSA provenance). If the workflow fails, fix the underlying issue and re-cut the tag — do NOT proceed.

3. **Run the [Pre-flight check](#pre-flight-check-before-flipping-out-of-draft) below.** If any verify fails, delete the draft release and re-cut the tag after the fix — do NOT flip the release.

4. **Flip the release out of draft.** Once pre-flight passes, the draft flip is the publication event — operators see the release immediately after.

   ```bash
   gh release edit vX.Y.Z --draft=false
   ```

## Pre-flight check before flipping out of draft

Before running step 4's `--draft=false`, cosign-verify the CI-produced assets against the documented identity-regexp. This catches a misconfigured workflow, a missing permission, or a stale CI image BEFORE the release becomes operator-visible.

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

Both `cosign verify-blob` and `cosign verify-attestation` MUST return "Verified OK" (exit 0). If either fails, **DO NOT** flip the release out of draft. Recovery: `gh release delete vX.Y.Z`, fix the underlying issue, re-push the tag.
