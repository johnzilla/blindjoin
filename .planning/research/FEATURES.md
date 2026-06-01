# Feature Landscape — v1.6 Supply-Chain Attestation

**Domain:** Closing the v1.5 unsigned-build supply-chain gap explicitly committed to in `SECURITY.md § Supply-chain status > v1.6 supply-chain plan`.
**Researched:** 2026-06-01
**Confidence:** HIGH on what cosign / SLSA v1.0 verification flows look like in practice; HIGH on what's table-stakes vs differentiator for an OSS Rust project releasing through GHA in 2026.

---

## Category 1 — cosign image attestations on ghcr.io

**Why this matters:** ghcr.io images today (`blindjoin-coordinator`, `-client`, `-liquidity-bot`) carry only the registry-asserted image digest. An operator pulling `ghcr.io/johnzilla/blindjoin-coordinator:1.6.0` has no cryptographic binding from the image bytes back to the maintainer identity + the source tree they came from.

**Table stakes (every operator expects):**
- **Image signed with cosign** in keyless OIDC mode at push time → every ghcr.io image has a corresponding `sha256:...sig` reference in the registry
- **SLSA v1.0 build-level-3 provenance** attached as an in-toto attestation → tells the verifier which workflow (file + ref + SHA) built the image and from what source commit
- **Verification command documented** in SECURITY.md (and SECURITY.md updated post-v1.6 to remove the "unsigned" callout)

**Differentiators (set blindjoin apart):**
- **SBOM attestation** (Syft-generated SPDX SBOM attached as `--predicate-type spdx`) — operators can scan for CVE exposure without pulling the image
- **Cosign bundle format** (`.bundle` files in the registry) — single-file verification asset combining sig + cert + Rekor inclusion proof; works offline once downloaded

**Anti-features (tempting but NOT for v1.6):**
- **Key-based cosign signing** — requires maintainer key custody. Explicit non-goal in SECURITY.md.
- **Notation / CNCF Notary v2** — alternative trust model; would split the project's signing posture between two ecosystems with no operator benefit.
- **Self-hosted Fulcio/Rekor** — viable for an org with airgapped operators, not for a solo MIT project.

**Operator verification UX:**
```bash
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/johnzilla/blindjoin-coordinator:1.6.0
```
One copy-pasteable command per image; no maintainer-published keys to track.

**Complexity:** LOW-MEDIUM. Mostly YAML additions to `docker.yml` + permission scope changes. SHA-pin every new action. Reusable workflow from slsa-github-generator does most of the work.

**Dependencies:**
- `id-token: write` permission added to `docker.yml`'s `docker` job
- `sigstore/cosign-installer@v3` action SHA-pinned and adopted
- Existing docker/build-push-action step must emit the image digest as output (it already does — `steps.<id>.outputs.digest`)

---

## Category 2 — Detached signatures on GitHub Release tarballs

**Why this matters:** v1.5 ships `blindjoin-linux-amd64.tar.gz` + `.sha256` companion. The sha256 lets an operator verify the archive is intact; it does NOT bind the archive to the maintainer. A compromised GitHub account uploading a substituted binary with a matching checksum is undetected by sha256 alone.

**Table stakes:**
- **`.sig` and `.crt` companion files** uploaded alongside the tarball via `cosign sign-blob --bundle` (or the discrete sig + cert outputs)
- **Verification command in SECURITY.md** — single cosign invocation that an operator runs cold against a downloaded tarball
- **Sigstore Rekor inclusion** — signature is logged to the public transparency log, so a future attacker cannot retroactively replace the signature without leaving an audit trail

**Differentiators:**
- **`.bundle` file format** — single asset operators download alongside the tarball, instead of two files
- **SLSA provenance for the tarball** (via `slsa-framework/slsa-github-generator/.github/workflows/generator_generic_slsa3.yml@v2`) — same provenance machinery as the Docker images; consistent verifier UX

**Anti-features:**
- **Detached PGP signature instead of cosign** — adds a separate trust root (a maintainer-held PGP key) that operators have to fetch, fingerprint-verify, and trust. Inconsistent with cosign image signing. Only worth doing if the audit feedback explicitly requests PGP.
- **Signing every CI build (not just tagged releases)** — token spam, no operator value.

**Operator verification UX:**
```bash
# Download tarball + .sig + .crt from the GitHub Release
cosign verify-blob \
  --certificate blindjoin-linux-amd64.tar.gz.crt \
  --signature blindjoin-linux-amd64.tar.gz.sig \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
```

**Complexity:** LOW. YAML addition to `release.yml`'s build job + extending the `softprops/action-gh-release` upload list with the sig/cert assets.

**Dependencies:**
- `id-token: write` added to release.yml's `build` job
- Same cosign-installer + cosign-installer SHA pin as Category 1 (one action covers both)

---

## Category 3 — Reproducible-build recipe + independent verification

**Why this matters:** Operators who don't trust the GitHub Releases tarball binary can today read the source — but there's no documented way to confirm that the binary in the release matches what `cargo build --release` would produce on a clean machine. Without that, even a signed binary is only "the maintainer signed THIS bytes"; reproducibility upgrades it to "and these bytes are the natural product of the source tree".

**Table stakes:**
- **`docs/REPRODUCIBLE-BUILD.md`** — written recipe: exact Rust toolchain version (matches `ci.yml` pin), exact `ubuntu-latest` runner image SHA at v1.6 ship, exact `RUSTFLAGS`, exact `SOURCE_DATE_EPOCH` derivation, exact `cargo build` invocation, expected `sha256sum` of the resulting tarball.
- **`cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot`** in release.yml updated to include `--locked` (currently it's implicit; making it explicit is the document-vs-code anchor).
- **`RUSTFLAGS="--remap-path-prefix=$PWD=. --remap-path-prefix=$HOME=."` and `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)`** set in the build job env. These strip the two main sources of binary nondeterminism (absolute build paths in panic messages; mtimes in archive metadata).
- **Independent verification job** — a separate scheduled workflow that runs the recipe on a fresh runner monthly, downloads the published tarball, asserts byte-equality. Failure = supply-chain regression → opens an issue.

**Differentiators:**
- **`reproducible-builds.org` registration** — submit blindjoin to the upstream registry of reproducible projects, so external rebuilders find us.
- **Multi-rebuilder cross-check** — in addition to the scheduled internal verifier, document the recipe in a form that one outside party (e.g. a security auditor) can run on their machine and report match/mismatch.

**Anti-features:**
- **Cross-distro reproducibility (zigbuild, musl, etc.)** — out of scope for v1.6's single-target linux-amd64 ship. Track as v1.7+ follow-on if multi-arch lands.
- **Bit-for-bit reproducibility of Docker images** — images embed a base-layer rootfs that changes with debian security backports; reproducibility for images is a different (harder) problem than the binary tarball. Defer.

**Operator verification UX:**
```bash
# Follow docs/REPRODUCIBLE-BUILD.md
# Result: sha256sum matches blindjoin-linux-amd64.tar.gz.sha256 in the GH Release
```

**Complexity:** MEDIUM. The recipe itself is straightforward; the long tail is verifying the recipe actually reproduces on a clean runner. Expect 1-2 iteration cycles to find sources of nondeterminism the first pass missed.

**Dependencies:** Categories 1 & 2 don't block this; can run in parallel. The independent-verification scheduled workflow depends on Category 2 because it verifies the cosign sig as well as the byte-equality.

---

## Category 4 — Automated base-image digest drift check

**Why this matters:** `docker/Dockerfile` accepts `CARGO_CHEF_REF` and `DEBIAN_REF` build args defaulting to floating tags. Release builds pin to digests, but the digests are passed at build time — there's no committed canonical digest list, and nothing fails CI if `debian:bookworm-slim` retags out from under the pin.

**Table stakes:**
- **`docker/digests.txt`** — committed canonical digest list. One line per base image: `debian:bookworm-slim@sha256:<HEX>` and `lukemathwalker/cargo-chef:latest-rust-1@sha256:<HEX>`. Bumped via a PR with explicit human review.
- **`.github/workflows/digest-drift-check.yml`** — scheduled (daily or weekly) workflow that:
  1. Reads `docker/digests.txt`
  2. Pulls each named tag fresh from the registry
  3. Compares — if any current registry digest differs from the committed digest, opens an issue (NOT a PR) titled `[digest-drift] <image> moved to <new-digest>`
- **`release.yml` + `docker.yml`** updated to read `docker/digests.txt` and pass `--build-arg DEBIAN_REF=...` automatically, so release builds always use the canonical digest list.

**Differentiators:**
- **Drift severity classification** — distinguish security-backport retags (expected, low-priority) from substantive base-image changes via comparing the diff in OS package versions. Stretch — manual review usually catches this.
- **Coordinated rotation** — when bumping a digest in `docker/digests.txt`, the same PR re-runs the `cosign verify` flow against a freshly-built image to confirm the supply chain still verifies end-to-end.

**Anti-features:**
- **Auto-merging digest bumps** — directly antithetical to the supply-chain assurance v1.6 is closing. Always human-review.
- **Renovate config** — adds an external dependency for a watch list of 2 images. Custom GHA scheduled workflow is simpler.
- **Watching all Dockerfile ARG-based refs in the repo** — overkill for a 2-base-image surface; the digest-drift workflow can explicitly enumerate the two refs.

**Operator verification UX (not externally facing):** This category is internal — operators don't run anything. The benefit lands in the next image build using verified-current digests.

**Complexity:** LOW. New file + new workflow YAML. Hardest part is deciding what to do when drift IS detected (= issue text).

**Dependencies:** None. Can ship independently of Categories 1-3. Useful early-phase candidate since it has no integration risk with the operator-facing artifacts.

---

## Category dependencies summary

| Category | Depends on | Blocks |
|---|---|---|
| 1 — Image attestations | (nothing — can run first) | None |
| 2 — Release tarball signing | Cosign-installer pin from Category 1 (shared) | Independent-verification job in Category 3 needs Category 2 to have shipped sig assets |
| 3 — Reproducible builds | (independent from 1 & 2) — but the scheduled verifier wants Category 2's sig assets to verify | None |
| 4 — Digest drift check | (independent) — useful as a parallel early-phase | None |

Build order suggestion (passed to gsd-roadmapper):
1. **Phase 22: Digest drift check** — independent, low risk, builds confidence in the digest-pin workflow before adding cosign atop it.
2. **Phase 23: cosign image attestations + SLSA provenance** (Categories 1 + part of 3) — the load-bearing piece. Adds id-token: write and the new actions.
3. **Phase 24: cosign blob signing on release tarballs** (Category 2) — reuses Category 1's installer + permission scaffolding.
4. **Phase 25: Reproducible-build recipe + scheduled verifier** (Category 3) — depends on Category 2 having shipped sigs to verify.
