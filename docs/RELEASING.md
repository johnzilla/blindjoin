# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor manual.

This file documents the post-CI portion of the release cycle: downloading the CI-built tarball, signing it with PGP on a YubiKey, uploading the detached signature, and flipping the GitHub Release out of draft. It also documents the one-time PGP key generation ceremony, the 2-year key rotation procedure, the emergency revocation procedure, and the publication procedures for the two key-distribution channels (`keys.openpgp.org` and WKD on `<owner>.github.io`).

Every fingerprint reference in this file is the literal placeholder string `<FINGERPRINT-TBD>`. A later plan replaces every `<FINGERPRINT-TBD>` occurrence atomically when the maintainer generates the key on the YubiKey.

## Prerequisites

- **YubiKey 5 (firmware ≥ 5.2.3 for ed25519 support)** with the blindjoin maintainer PGP key loaded. The YubiKey OpenPGP applet has supported ed25519 since firmware 5.2.3 (Yubico docs). One-time key generation lives in [PGP key generation](#pgp-key-generation-one-time-not-a-release-cut-step).
- **gpg 2.4+** on the maintainer's machine. Install:
  ```bash
  brew install gnupg           # macOS
  apt install gnupg            # Debian / Ubuntu
  ```
- **gh 2.50+** on the maintainer's machine. Install via [cli.github.com](https://cli.github.com).
- **cosign 2.6.3+** for the pre-flight verify gate before flipping the release out of draft (see [Pre-flight check](#pre-flight-check-before-flipping-out-of-draft)). Install:
  ```bash
  brew install cosign          # macOS
  # or download from https://github.com/sigstore/cosign/releases
  ```
- **`<owner>.github.io` repo exists with WKD published.** This is a one-time setup; the procedure lives in [Publishing the key to WKD](#publishing-the-key-to-wkd).

## Per-release procedure (5 steps)

The maintainer cuts a release in 5 steps. CI handles steps 1-2; the maintainer drives steps 3-5 from a local machine with the YubiKey plugged in.

1. **Cut and push the tag.** Use signed tags so the maintainer-side commit identity is bound to the release. The tag MUST use 3-part semver (`vX.Y.Z`, not `vX.Y`) per [CONTRIBUTING.md `## Tagging releases`](../CONTRIBUTING.md#tagging-releases).

   ```bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push --tags
   ```

2. **Watch `release.yml` until green.** Open [`release.yml`](../.github/workflows/release.yml) in the Actions tab. CI creates a DRAFT GitHub Release with 4 assets: `blindjoin-linux-amd64.tar.gz`, `blindjoin-linux-amd64.tar.gz.sha256`, `blindjoin-linux-amd64.tar.gz.bundle` (cosign signature), and `blindjoin-linux-amd64.tar.gz.sigstore` (SLSA provenance). If the workflow fails, fix the underlying issue and re-cut the tag — do NOT proceed.

3. **Download the CI-built tarball** to a fresh local directory.

   ```bash
   gh release download vX.Y.Z -p 'blindjoin-linux-amd64.tar.gz' --dir /tmp/blindjoin-release
   ```

4. **Sign with PGP on the YubiKey.** The YubiKey will prompt for physical touch — confirm the signing request on the hardware token. The signing key never leaves the YubiKey.

   ```bash
   cd /tmp/blindjoin-release
   gpg --detach-sign --armor --local-user <FINGERPRINT-TBD> blindjoin-linux-amd64.tar.gz
   ```

   This produces `blindjoin-linux-amd64.tar.gz.asc` in the same directory.

5. **Upload `.asc` and flip the release out of draft.** Run the [Pre-flight check](#pre-flight-check-before-flipping-out-of-draft) below BEFORE this step. The draft flip is the publication event — operators see the release immediately after.

   ```bash
   gh release upload vX.Y.Z blindjoin-linux-amd64.tar.gz.asc
   gh release edit vX.Y.Z --draft=false
   ```

## Pre-flight check before flipping out of draft

Before running step 5's `--draft=false`, cosign-verify the 4 CI-produced assets against the documented identity-regexp. This catches a misconfigured workflow, a missing permission, or a stale CI image BEFORE the release becomes operator-visible.

```bash
# Download all 4 CI-produced assets to a fresh directory.
cd /tmp/blindjoin-release
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

Both `cosign verify-blob` and `cosign verify-attestation` MUST return "Verified OK" (exit 0). If either fails, **DO NOT** flip the release out of draft. Recovery path:

```bash
# Delete the broken draft release and re-cut the tag after the fix.
gh release delete vX.Y.Z
# Investigate the workflow failure (almost certainly a release.yml change, a
# permissions misconfiguration, or a sigstore action SHA rotation). Commit the
# fix, merge, then re-run from step 1.
```

Re-pushing the same tag is the recovery — softprops is idempotent at the release level, and the deleted draft frees the tag for re-publication.
