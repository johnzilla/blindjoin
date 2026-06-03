# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor manual.

This file documents the post-CI portion of the release cycle: pre-flight verifying the CI-built artifacts before tagging, then rehearsing reproducibility at the v1.6.0-rc.0 cut to capture the expected sha256 hash.

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

## Reproducibility verification rehearsal (v1.6.0-rc.0 procedure)

Before the v1.6.0 cut, capture the expected sha256sum and runner ImageVersion that go into [docs/REPRODUCIBLE-BUILD.md](REPRODUCIBLE-BUILD.md) §Expected sha256sum and §Toolchain pins. The rehearsal runs the same verifier that monthly verification will use — this is the one-shot pre-tag dry run.

1. Trigger `.github/workflows/reproducible-verify.yml` via `workflow_dispatch` from the Actions tab on the branch you intend to tag from. The workflow has no inputs.

2. Wait for the run to complete. From the "Capture runner ImageVersion" step's logs, copy the `Verifier image: <VALUE>` line; from the "Compare sha256 + classify result" step, copy the `Actual (rebuilt locally): <SHA>` line. (Note: the first rehearsal run will go RED because the verifier compares against `<TBD-v1.6.0-cut>` placeholder, NOT a real sha256 — this is expected; capture the values, do not interpret the failure as a regression.)

3. **Substitute the captured values across all FOUR placeholder sites** (BLOCKER 2 fix: the verifier consumes `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` as a colon-delimited triple `<tag>:<sha256>:<image-version>` — substitute each placeholder against its dedicated sed pattern; do NOT use a global sed because the markdown table has TWO occurrences with DIFFERENT replacement values):

   (i) **`docs/REPRODUCIBLE-BUILD.expected-sha256.txt` — sha256 placeholder** (used by verifier for byte-equality check):
   ```bash
   sed -i 's/<TBD-v1.6.0-cut-sha256>/<captured-sha256>/' docs/REPRODUCIBLE-BUILD.expected-sha256.txt
   ```

   (ii) **`docs/REPRODUCIBLE-BUILD.expected-sha256.txt` — ImageVersion placeholder** (used by verifier for D-12 drift-vs-divergence classification):
   ```bash
   sed -i 's/<TBD-v1.6.0-cut-imageversion>/<captured-imageversion>/' docs/REPRODUCIBLE-BUILD.expected-sha256.txt
   ```

   (iii) **`docs/REPRODUCIBLE-BUILD.md` §Expected sha256sum table — sha256 placeholder** (human-facing):
   ```bash
   # Substitute by section/file context (do not global-sed — the markdown table has another <TBD-v1.6.0-cut> in the Toolchain pins table)
   # Use a context-aware sed (anchor on the row prefix `| v1.6.0 |`) or hand-edit the table cell.
   sed -i '/^| v1\.6\.0 |/ s/<TBD-v1\.6\.0-cut>/<captured-sha256>/' docs/REPRODUCIBLE-BUILD.md
   ```

   (iv) **`docs/REPRODUCIBLE-BUILD.md` §Toolchain pins table (ubuntu-24.04 row) — ImageVersion placeholder** (human-facing):
   ```bash
   sed -i '/^| ubuntu-24\.04 runner image |/ s/<TBD-v1\.6\.0-cut>/<captured-imageversion>/' docs/REPRODUCIBLE-BUILD.md
   ```

   **Verification after substitution:**
   ```bash
   # Confirm the verifier's awk lookup now returns both real values, not placeholders:
   awk -F: '$1 == "v1.6.0" {print $2 " " $3}' docs/REPRODUCIBLE-BUILD.expected-sha256.txt
   # Should print: <captured-sha256> <captured-imageversion>  (no `<TBD-` substrings)

   # Confirm no TBD placeholders remain in either file:
   ! grep -q '<TBD-v1.6.0-cut' docs/REPRODUCIBLE-BUILD.md docs/REPRODUCIBLE-BUILD.expected-sha256.txt
   ```

4. Commit the changes with message `docs(25): capture v1.6.0-rc.0 reproducibility baseline (ImageVersion=<X>, sha256=<Y>)` — single commit, both files together.

5. Tag v1.6.0 and push (`git tag -s v1.6.0 -m "v1.6.0"` + `git push --tags`). The first scheduled `reproducible-verify.yml` run on the 1st of the next month will now compare against the real value and go GREEN if reproducibility holds.

After the first green scheduled run, proceed to [Reproducible-builds.org registry submission](#reproducible-buildsorg-registry-submission) below.

## Reproducible-builds.org registry submission

After `.github/workflows/reproducible-verify.yml` has run green for ≥1 monthly cycle post-v1.6.0, blindjoin is eligible for [reproducible-builds.org](https://reproducible-builds.org) project registry submission. This is a maintainer-driven manual procedure — the registry accepts only human-reviewed PRs.

1. **Verify the green-monthly-cycle prerequisite.** Open the [reproducible-verify workflow Actions tab](../../../actions/workflows/reproducible-verify.yml) and confirm at least one scheduled (NOT workflow_dispatch) run after the v1.6.0 tag exited 0.

2. **Fork and add the entry.** Fork [github.com/reproducible-builds/reproducible-builds.org](https://github.com/reproducible-builds/reproducible-builds.org). Add an entry under `_data/projects/` per the project's existing YAML schema. Required fields: name (blindjoin), language (Rust), reproducibility-doc URL (link to `docs/REPRODUCIBLE-BUILD.md`), verify-command (copy the Recipe section from `docs/REPRODUCIBLE-BUILD.md`).

3. **Open the PR + respond to reviewer feedback.** The reproducible-builds.org maintainers will review and may request schema adjustments. Expect ~1-2 round trips before merge.

4. **After merge: link back from blindjoin.** Edit `docs/REPRODUCIBLE-BUILD.md` §Continuous verification: replace `Registry entry: <added after blindjoin's submission lands...>` with the actual registry URL. Edit `SECURITY.md` §Supply-chain status §Reproducibility: replace the same placeholder with the actual URL. Commit both edits together.

REPRO-04 is closed once the registry PR is merged AND both blindjoin-side cross-links are updated.
