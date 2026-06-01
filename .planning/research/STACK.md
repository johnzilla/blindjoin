# Technology Stack — v1.6 Supply-Chain Attestation

**Project:** blindjoin — Rust CoinJoin coordinator + client
**Milestone:** v1.6 (subsequent — closing the v1.5 unsigned-build gap)
**Researched:** 2026-06-01
**Confidence:** HIGH on sigstore/cosign maturity (cosign 2.x mainstream since 2023, OIDC keyless flow stable); HIGH on GitHub Actions OIDC token API; MEDIUM on Rust reproducible-build edge cases (the long tail is real and project-specific); HIGH on Docker Hub / ghcr.io digest resolution.

---

## Sigstore / cosign

| Component | Version | Purpose | Why |
|---|---|---|---|
| **cosign** | 2.x (≥ 2.2; 2.5+ as of mid-2026) | Sign & verify container images and blobs; produce in-toto attestations | Mature, OIDC-bound keyless flow eliminates maintainer key custody. Industry standard for OSS supply chain. Single binary, no daemon. |
| **sigstore/cosign-installer** | `@v3` (SHA-pinned at adoption) | GitHub Action that installs cosign on the runner | Maintained by sigstore, SHA-pinnable. Matches the project's existing SHA-pin discipline. |
| **Fulcio** | sigstore public good | Short-lived certificate authority that binds the GHA OIDC identity to a one-time signing cert | Default endpoint; works out of the box. Alternative: self-hosted Fulcio (not warranted for blindjoin). |
| **Rekor** | sigstore public good | Transparency log for signatures + attestations | Default endpoint. Operators verifying offline can use `--insecure-ignore-tlog` BUT that defeats the assurance — document the default + opt-out. |

**Verification UX** (what an operator runs):

```bash
# Image attestation
cosign verify \
  --certificate-identity 'https://github.com/johnzilla/blindjoin/.github/workflows/docker.yml@refs/tags/v1.6.0' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/johnzilla/blindjoin-coordinator:1.6.0

# Blob signature on a release tarball
cosign verify-blob \
  --certificate blindjoin-linux-amd64.tar.gz.crt \
  --signature blindjoin-linux-amd64.tar.gz.sig \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
```

Two assets ship per tarball: `.sig` (the signature) and `.crt` (the short-lived Fulcio cert bound to the OIDC identity). Optionally `.bundle` (cosign 2.x bundle format combining sig + cert + Rekor inclusion proof).

---

## SLSA provenance

| Component | Version | Purpose | Why |
|---|---|---|---|
| **slsa-framework/slsa-github-generator** | `@v2` (SHA-pinned at adoption) | Reusable GHA workflow that emits SLSA v1.0 provenance attestations for binaries + container images | Achieves **SLSA Build Level 3** for GitHub-hosted runners. The provenance is what an external verifier inspects to confirm the artifact came from the claimed source + workflow. |
| **SLSA v1.0 spec** | published Feb 2024 | Provenance schema (`builder.id`, `metadata`, `materials`, `buildType`) | The schema cosign emits as an in-toto attestation. v1.0 is the current stable; v0.2 is deprecated. |

Realistic v1.6 target: **SLSA Level 3** for both Docker images (via `slsa-github-generator/.github/workflows/generator_container_slsa3.yml@v2`) and the linux-amd64 tarball (via `generator_generic_slsa3.yml@v2`). Level 3 specifies non-falsifiable provenance + hardened build platform; GitHub-hosted runners + the reusable workflow give us Level 3 without self-hosted infra.

---

## Reproducible builds for Rust

| Component | Status | Purpose | Why |
|---|---|---|---|
| **Cargo `--remap-path-prefix`** | stable since Rust 1.31 | Strips absolute build paths from binary | Without this, `/home/runner/work/blindjoin/blindjoin/...` ends up in panic messages → not reproducible. |
| **`SOURCE_DATE_EPOCH`** | de-facto standard | Pins timestamps in build metadata | Set from tag commit time so two independent rebuilds at different wall-clocks produce identical metadata. |
| **`cargo --locked`** | stable | Forces resolved deps to exactly match `Cargo.lock` | Mandatory for reproducibility. We already use `--locked` implicitly via release.yml's `cargo build --release`; v1.6 should make it explicit. |
| **`cargo-zigbuild`** | optional | Cross-compiles via zig toolchain → identical binaries across Linux distros | Heavier setup; **NOT recommended for v1.6** since we ship only linux-amd64 and the GHA `ubuntu-latest` runner is consistent enough. Track as a follow-on for multi-arch. |
| **`reproducible-builds.org`** | reference | The community's hash-equivalence verification recipes | Project will publish a `REPRODUCIBLE-BUILD.md` recipe that names: Rust toolchain version, runner image SHA, build env vars (`SOURCE_DATE_EPOCH`, `RUSTFLAGS`), and the `sha256sum` an independent rebuilder should match. |

**Realistic v1.6 target:** bit-for-bit binary equality for `blindjoin-linux-amd64.tar.gz` when rebuilt on `ubuntu-latest` (same image SHA) with the documented env. NOT cross-runner-distro reproducibility (that's a v1.7+ stretch).

---

## Docker base-image digest drift detection

| Component | Version | Purpose | Why |
|---|---|---|---|
| **Renovate** OR **Dependabot for Docker** | both production-grade | Watches `FROM` lines in Dockerfile + raises PRs when a pinned digest moves | **Recommended: Dependabot** for blindjoin — already first-party with GHA, no extra config server. Renovate is more flexible but adds an external dependency. |
| **`docker buildx imagetools inspect`** | docker 23+ | Resolves a tag to its current digest from the registry — usable in a custom GHA cron job | Fallback if Dependabot can't see the ARG-based scaffold. v1.6 will likely need a small custom GHA cron because Dependabot watches literal `FROM image:tag` lines, not `ARG` defaults. |

**Likely approach:** a custom GHA scheduled workflow (`.github/workflows/digest-drift-check.yml`) running daily that:
1. Reads `docker/digests.txt` (new file — committed canonical digest list for `debian:bookworm-slim` + `lukemathwalker/cargo-chef:latest-rust-1`)
2. Resolves the same tags fresh against the registry
3. If drift → opens an issue (NOT a PR — auto-merging digest bumps without human review IS a supply-chain risk)

---

## Github Actions

| Component | Required scope | Purpose |
|---|---|---|
| `id-token: write` | NEW for v1.6 | Allows the workflow to request a GitHub OIDC token, which cosign exchanges with Fulcio for a short-lived cert |
| `contents: write` | already present in release.yml | Upload signed artifacts + .sig/.crt files to GitHub Releases |
| `packages: write` | already present in docker.yml | Push images + attestations to ghcr.io |
| `actions: read` | implicit | Required by slsa-github-generator to introspect the workflow context |

All new actions adopted in v1.6 (`sigstore/cosign-installer`, `slsa-framework/slsa-github-generator`) get SHA-pinned at adoption per the project's existing GHA-pin discipline. Pin SHAs go in a comment line next to each `@v3` ref.

---

## Alternatives considered

| Category | Recommended | Alternative | Why not |
|---|---|---|---|
| Image signing | cosign keyless via OIDC | Notation (CNCF) | Less established for OSS; weaker tool ecosystem. |
| Image signing | cosign keyless | cosign key-based | Requires maintainer key custody — explicit non-goal per SECURITY.md. |
| Blob signing | cosign blob sign | Detached PGP (gpg --detach-sign) | Requires GPG key management. Rust ecosystem hasn't strongly normed on either; cosign keeps the trust root consistent with image signing. |
| Provenance | SLSA v1.0 via slsa-github-generator | in-toto attestations hand-rolled | Reusable workflow is curated, audited, and tracks SLSA spec updates. Hand-rolling is unnecessary maintenance. |
| Reproducibility | `--remap-path-prefix` + `SOURCE_DATE_EPOCH` + `--locked` | cargo-zigbuild + multi-distro reproducibility | Out of scope for v1.6's single-target linux-amd64 ship. |
| Digest drift | Custom GHA cron + `digests.txt` | Renovate config | One less external dependency for a one-file watch. Renovate is appropriate if the project later watches many manifests. |
| Digest drift output | Open an issue | Open an auto-mergeable PR | Auto-merging digest bumps WITHOUT human review is itself a supply-chain risk (the very thing we're closing). Issue → human review → PR. |

---

## Version summary

| Tool | Pin |
|---|---|
| cosign | 2.x (latest stable at v1.6 ship) |
| sigstore/cosign-installer | v3 (+ SHA at adoption) |
| slsa-framework/slsa-github-generator | v2 (+ SHA at adoption) |
| Rust toolchain | stable (matches ci.yml) |
| Dockerfile bases (debian, cargo-chef) | digest-pinned via the existing ARG scaffold; canonical digests in `docker/digests.txt` |

---

## Sources

- [sigstore.dev — cosign documentation](https://docs.sigstore.dev/cosign/) [HIGH]
- [slsa.dev — SLSA v1.0 specification](https://slsa.dev/spec/v1.0/) [HIGH]
- [github.com/slsa-framework/slsa-github-generator](https://github.com/slsa-framework/slsa-github-generator) [HIGH]
- [docs.github.com — OIDC for GitHub Actions](https://docs.github.com/en/actions/deployment/security-hardening-your-deployments/about-security-hardening-with-openid-connect) [HIGH]
- [reproducible-builds.org/docs/](https://reproducible-builds.org/docs/) [MEDIUM]
- [doc.rust-lang.org — `--remap-path-prefix`](https://doc.rust-lang.org/rustc/command-line-arguments.html#--remap-path-prefix-remap-source-names-in-output) [HIGH]
