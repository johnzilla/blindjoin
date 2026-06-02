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

## PGP key generation (one-time, NOT a release-cut step)

Generate the blindjoin maintainer signing key directly on a YubiKey. Private key material never resides on a host filesystem. This procedure runs ONCE at first key creation (and again every 2 years on rotation — see [PGP key rotation](#pgp-key-rotation-every-2-years)).

1. **Generate primary signing key directly on YubiKey.** Generation happens on-card; private key material never touches the host filesystem.

   ```bash
   gpg --card-edit
   # > admin
   # > generate
   # Select: (1) ed25519, signing-only, 2-year expiry
   # User-ID: blindjoin maintainer <johnturner@gmail.com>
   ```

   The YubiKey ed25519 OpenPGP applet has been supported since YubiKey firmware 5.2.3 (Yubico docs). The UID is project-scoped — `blindjoin maintainer <johnturner@gmail.com>` — so a future revocation/rotation does not entangle a personal-key consumer.

2. **Generate the revocation certificate; store offline.** The revocation cert is the ONLY recovery path if the YubiKey is lost or compromised. Keep it on a USB drive AND on a paper backup; never on a host filesystem.

   ```bash
   gpg --output revoke.asc --gen-revoke <FINGERPRINT-TBD>
   # Move revoke.asc to USB + paper, then remove from disk:
   shred -u revoke.asc 2>/dev/null || rm -P revoke.asc 2>/dev/null
   ```

3. **Export the public key to the repo.** Filename IS the full 40-char fingerprint — operators verify the binding by comparing the file's `fpr:` line to the filename, no additional prose required.

   ```bash
   gpg --export --armor <FINGERPRINT-TBD> > docs/pgp/<FINGERPRINT-TBD>.asc
   ```

4. **Publish the public key to keys.openpgp.org.** See [Publishing the key to keys.openpgp.org](#publishing-the-key-to-keysopenpgporg) for the full procedure.

5. **Publish the public key to WKD.** See [Publishing the key to WKD](#publishing-the-key-to-wkd) for the full procedure.

**Operator-side verification of the public-key binding.** Any operator can verify that the committed `.asc` file matches its filename by running:

```bash
gpg --with-colons --import-options show-only --import docs/pgp/<FINGERPRINT-TBD>.asc | head -2
# The fpr: line's 10th field (the fingerprint) equals the filename.
```

This is self-verifying — no SECURITY.md prose needs to anchor the binding.

## PGP key rotation (every 2 years)

The maintainer's PGP key expires 2 years from generation (D-08). Rotate BEFORE expiry, NOT after — operators with the old key cached should see a verifiable signature from the new key while the old one is still valid for cross-signing.

1. **6 months before expiry**, generate a new ed25519 key on the same YubiKey (or a new YubiKey for stronger key isolation). Use the same `gpg --card-edit` ceremony as the initial generation in [PGP key generation](#pgp-key-generation-one-time-not-a-release-cut-step) step 1.

2. **Cross-sign the new key with the old key** while the old key is still valid. This provides a verifiable provenance chain for operators using WoT-aware paths (rare; modern keys.openpgp.org strips third-party signatures, but the on-disk signature in the committed `.asc` file still carries the cross-signature for offline verification).

   ```bash
   gpg --sign-key <new-FINGERPRINT-TBD>
   ```

3. **Commit the new public key** alongside the old one. The OLD key file STAYS in the repo as a historical record — operators verifying old releases can still locate the right key.

   ```bash
   gpg --export --armor <new-FINGERPRINT-TBD> > docs/pgp/<new-FINGERPRINT-TBD>.asc
   git add docs/pgp/<new-FINGERPRINT-TBD>.asc
   ```

4. **Update `SECURITY.md`'s `<a id="pgp-current"></a>` anchor** to name the new fingerprint. The old fingerprint stays in CHANGELOG.md (transparency — operators see the rotation event in the release notes).

5. **Publish the new key to keys.openpgp.org and WKD.** Wait 24h for keyserver propagation before the first release signed with the new key.

6. **Cut the next release with the new key.** The first release signed with the new key SHOULD be accompanied by a CHANGELOG entry naming the rotation event (date, old fingerprint, new fingerprint).

## PGP key revocation (emergency — YubiKey lost or compromised)

Rotation and revocation are different procedures. Rotation happens on a planned 2-year cadence with cross-signing. Revocation is what happens if the YubiKey is lost or compromised — the old key is INVALIDATED, not rotated, and a new key replaces it without the cross-signature chain.

1. **Recover the offline revocation cert** from the USB drive + paper backup generated at [PGP key generation](#pgp-key-generation-one-time-not-a-release-cut-step) step 2.

2. **Import the revocation cert** to invalidate the compromised key locally:

   ```bash
   gpg --import revoke.asc
   ```

3. **Publish the revocation** to keys.openpgp.org and WKD so operators see the key is no longer valid. Operators with the old key cached will see the revocation status on their next refresh.

   ```bash
   gpg --send-keys --keyserver hkps://keys.openpgp.org <FINGERPRINT-TBD>
   # Also re-publish the now-revoked key to WKD — see Publishing the key to WKD.
   ```

4. **Run the full PGP key generation procedure** for the new key. Skip the cross-sign step from rotation: the old key is REVOKED and cannot sign anything trustworthy.

5. **Document the revocation event in CHANGELOG.md** with the date and a brief reason (compromise / loss / other). This is the transparency surface — operators tracing back through releases can see when the trust root rotated under duress versus planned cadence.

## Publishing the key to keys.openpgp.org

The single-command publish path. `keys.openpgp.org` is the modern, abuse-resistant keyserver — it strips third-party signatures and requires email confirmation before serving the key publicly (defeats keyserver spam).

```bash
gpg --send-keys --keyserver hkps://keys.openpgp.org <FINGERPRINT-TBD>
```

After running this command, `keys.openpgp.org` sends an email confirmation link to the UID's mailbox (`johnturner@gmail.com`). Click the link to complete the publication. **Without the email confirmation, keys.openpgp.org does NOT serve the key publicly.** Operators querying for the key will receive a "not found" response until the confirmation is processed.

## Publishing the key to WKD

WKD (Web Key Directory) is the modern operator-UX path for key discovery: `gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com` resolves the key automatically without keyserver flags. blindjoin publishes via the WKD **direct method** on `<owner>.github.io` (GitHub Pages). The subdomain method (`openpgpkey.<domain>`) is NOT viable on `*.github.io` — GitHub Pages does NOT support subdomain wildcarding, so the direct method is the only path that works on the maintainer's GitHub Pages site.

1. **Verify the `<owner>.github.io` repo exists.** If this is the first time the maintainer is publishing a key under this UID, the repo may not exist yet. One-time setup:

   ```bash
   gh repo create <owner>.github.io --public --description "GitHub Pages site for <owner> — hosts WKD .well-known/openpgpkey for blindjoin maintainer key"
   ```

   This is a one-time maintainer-side step at first key generation. The actual repo creation is the maintainer's action; Phase 24 documents the procedure but does not execute it.

2. **Compute the WKD hash for the UID mailbox.** The hash is `SHA-1(lowercase(local-part)) → z-base32`; using `gpg-wks-client` ensures the canonical computation (do not hand-roll the lowercase + z-base32 conversion).

   ```bash
   WKD_HASH=$(gpg-wks-client --print-wkd-hash johnturner@gmail.com | awk '{print $1}')
   ```

3. **Export the public key in WKD's binary keyring format.** WKD expects the binary keyring format, NOT the ASCII-armored form that the repo `docs/pgp/<FINGERPRINT-TBD>.asc` uses.

   ```bash
   gpg --no-armor --export johnturner@gmail.com > "${WKD_HASH}"
   ```

4. **Commit the binary keyring to `<owner>.github.io`** at the direct-method path:

   ```bash
   cd path/to/<owner>.github.io
   mkdir -p .well-known/openpgpkey/hu
   mv /path/to/${WKD_HASH} .well-known/openpgpkey/hu/${WKD_HASH}
   git add .well-known/openpgpkey/hu/${WKD_HASH}
   git commit -m "wkd: publish blindjoin maintainer key for johnturner@gmail.com"
   git push
   ```

5. **Test WKD resolution from a fresh machine** (or from a fresh `gnupg` profile to avoid keyring caching).

   ```bash
   gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com
   # Should print the imported key and its fingerprint.
   ```

**Refresh cadence.** The WKD-published key needs re-uploading ONLY on rotation (every 2 years per D-08) or on emergency revocation. There is no daily/weekly refresh required.

**Direct method vs subdomain method.** The GnuPG wiki notes the subdomain method (`openpgpkey.<domain>`) is "preferred" by the spec, but GitHub Pages does NOT support subdomain wildcarding on `*.github.io` — the direct method (`<owner>.github.io/.well-known/openpgpkey/...`) is the only viable path here. If the maintainer later switches to a custom domain (`blindjoin.example.com`), the subdomain method becomes available as an upgrade.
