# Security Policy

blindjoin is a CoinJoin coordinator and client for Bitcoin signet. This
document covers how to report a vulnerability, which versions are supported,
the project's current audit-readiness status, and the known supply-chain
gaps you should be aware of before running it.

This file is the front door. The technical threat model and the cross-shape
rejection properties live in [`docs/AUDIT-CHARTER.md`](docs/AUDIT-CHARTER.md);
the v1.5 ship retrospective lives in [`.planning/RETROSPECTIVE.md`](.planning/RETROSPECTIVE.md).

## Supported versions

Only the most recent minor release is supported with security fixes.
Older minor releases are archived and will not receive backports.

| Version | Supported | Status |
|---------|-----------|--------|
| 1.5.x   | ✅        | Current — audit-ready (charter shipped at v1.5) |
| 1.4.x   | ❌        | Superseded by 1.5; multi-script complete in 1.4 but `sign` bodies finalized in 1.5 |
| 1.3.x   | ❌        | Superseded |
| 1.2.x   | ❌        | Superseded |
| 1.1.x   | ❌        | Superseded |
| 1.0.x   | ❌        | Superseded |
| `main`  | best-effort | Pre-release; fixes land here first |

blindjoin is signet-first. Mainnet operation is a config flag, not a
code change — but mainnet has additional residual risks (see
[Operational residual risks](docs/AUDIT-CHARTER.md#residual-risks-operational))
that are **explicitly not closed for v1.5**.

## Reporting a vulnerability

Email **<johnturner@gmail.com>** with the subject line `[blindjoin security]`.

Please include:

- A description of the issue and the affected module(s) (component file
  paths help — e.g. `coordinator/src/bitcoin/utxo.rs::dispatch_ownership_proof`).
- A proof-of-concept or reproduction recipe if available.
- Your assessment of impact (e.g. coordinator DoS, client signing key
  exposure, unlinkability break, byte-equality regression vs v1.4).
- Whether you intend to publicly disclose, and on what timeline.

If you prefer an end-to-end encrypted channel, request a PGP key in your
first message and the maintainer will reply with a fingerprint signed from
the same address.

**Do not file public GitHub issues for security reports.** Issues affecting
the v1.4 acceptance gate (cross-shape rejection properties, V1.4-CRIT-01
script-type spoofing, AUDIT-03 zeroization window) qualify as security
reports — file them privately first.

## Disclosure policy

- **Acknowledgement**: within 72 hours of report.
- **Initial triage**: within 7 days. Triage decides whether the report
  qualifies as a security issue or is reclassified as a regular bug.
- **Fix window**: as fast as the issue allows; no fixed SLA. blindjoin
  is a public-good project maintained by one person — pace will reflect
  that.
- **Coordinated disclosure**: maintainer and reporter agree on a public
  disclosure window. Default is 90 days from report or earlier if the fix
  ships first. The reporter gets credit in the CHANGELOG entry unless
  they request anonymity.
- **Embargoed fixes** land on `main` only after the public disclosure
  window opens or after the reporter agrees the fix can ship.

## Audit-readiness status

v1.5 shipped the [external audit charter](docs/AUDIT-CHARTER.md), which
enumerates:

- in-scope modules (file:symbol references — durable across line-number churn);
- threat models per module (V1.4-CRIT-01 script_type spoofing, V1.4-CRIT-02
  silent sighash regression, V1.4-MIN-02 uniform-script fingerprint, the
  `rsa` Marvin Attack residual);
- the 9 cross-shape rejection properties locked at v1.4;
- the v=2 OwnershipProof PSBT handling boundary;
- the RSA secret key zeroization window (post AUDIT-03 bounded form: the
  per-round RSA secret key is an `Option<RsaBlindSigner>` on
  `RoundStateInner`, nulled at the SOLE FSM chokepoint
  `transition_to(Phase::Idle)` — verified by grep);
- out-of-scope components and dependencies;
- residual risks accepted with explicit dispositions and rationale.

An external auditor's starting points are §1 (in-scope modules) and §5
(RSA zeroization window). The `must_haves` truths in each v1.4/v1.5 phase
plan map one-to-one onto the auditable invariants.

The charter is the source of truth for what is and is not in scope for
external review. This SECURITY.md file is a pointer to it, not a
substitute for it.

## Supply-chain status

blindjoin's release artifacts have **known supply-chain gaps** at v1.5.
They are documented here, not hidden. If you operate blindjoin in any
environment where supply-chain assurance matters, read this section
before pulling a binary or image.

### Known gaps at v1.5

- **~~GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign signature.~~** **Closed in v1.6 Phase 24** — see [Release tarball signatures + provenance (v1.6 onward)](#release-tarball-signatures--provenance-v16-onward).
- **~~Docker images on `ghcr.io` are unsigned.~~** **Closed in v1.6 Phase 23** — see [Image signatures + attestations (v1.6 onward)](#image-signatures--attestations-v16-onward).
- **~~No reproducible-build pipeline.~~** **Closed in v1.6 Phase 25** — see [Reproducibility (v1.6 onward)](#reproducibility-v16-onward).
- **~~Base image digest pins are manual.~~** **Closed in v1.6** — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).

### Image signatures + attestations (v1.6 onward)

Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z`
image push from a `vX.Y.Z` tag is:

1. **Signed by cosign** via OIDC keyless flow (no maintainer key custody). The
   signature is stored in the registry under `sha256-<HEX>.sig` and includes
   the Fulcio-issued cert bound to the GitHub Actions OIDC identity + the
   Rekor transparency-log inclusion proof.
2. **Attested with a SLSA v1.0 in-toto provenance bundle** (predicate type
   `https://slsa.dev/provenance/v1`), naming the workflow file + tag ref +
   source commit + runner image. Stored as an OCI referrer of the image.
3. **Attested with a SPDX SBOM** (predicate type `https://spdx.dev/Document`),
   generated by Syft against the full image filesystem. Stored as a sibling
   referrer of the SLSA attestation.

Verification requires **cosign 2.6.3 or compatible** and the **GitHub CLI
(`gh`) 2.x or later**. The verify recipes below have been tested on a clean
`ubuntu:24.04` container.

```bash
# 1. Cosign signature verification (ATTEST-01)
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/<owner>/blindjoin-<image>:<tag>
# Substitute <image> = coordinator | client | liquidity-bot
# Expected: "Verification for ghcr.io/.../blindjoin-<image>:<tag> --" + JSON
#           output of the verified cert claims.

# 2. SLSA build provenance attestation (ATTEST-02)
gh attestation verify oci://ghcr.io/<owner>/blindjoin-<image>:<tag> \
  --repo <owner>/blindjoin \
  --predicate-type https://slsa.dev/provenance/v1
# Expected: "Loaded N attestation(s) ... ✓ Verified provenance attestation."

# 3. SBOM attestation (ATTEST-03)
gh attestation verify oci://ghcr.io/<owner>/blindjoin-<image>:<tag> \
  --repo <owner>/blindjoin \
  --predicate-type https://spdx.dev/Document
# Expected: same shape as (2), with the SPDX SBOM payload.

# 4. Offline-verifiable bundle directory (ATTEST-04)
cosign save --dir ./blindjoin-<image>-<tag> \
  ghcr.io/<owner>/blindjoin-<image>:<tag>
cosign verify --local-image ./blindjoin-<image>-<tag> \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
# After step 1's `cosign save` completes (one-time network), all subsequent
# `cosign verify --local-image` invocations against this directory verify
# offline. Recipes 1–3 require network access to Fulcio + Rekor + the OCI
# registry; recipe 4's verify step does not.
```

> **Note: GHCR UI "Unverified" badge** is unrelated to cosign verification
> (Pitfall 10). GHCR's web view does not consult Rekor by default. **The
> `cosign verify` CLI output is the source of truth for signature status.**
> Operators should not interpret a "Verified" / "Unverified" badge on the
> GHCR web UI as a substitute for running the verify recipes above. GitHub
> may add cosign-aware UI in a future GHCR release.

> **Note: cosign 3.0 CLI flag drift** (Pitfall 13). The recipes above have
> been tested with **cosign `>= 2.6.3, < 3.0.0`**. cosign 3.0 (released
> 2026 — see [sigstore/cosign releases](https://github.com/sigstore/cosign/releases))
> may change CLI flags; when blindjoin's pipeline upgrades to cosign 3.x,
> the project will publish an updated recipe and a migration note in the
> release notes. **Until then, install cosign in the documented version
> range** — see the cosign release page for binary downloads.

### Release tarball signatures + provenance (v1.6 onward)

Every `blindjoin-linux-amd64.tar.gz` Release archive published from a `vX.Y.Z`
tag is:

1. **Signed by cosign** via OIDC keyless flow (no maintainer key custody). The
   signature is distributed as `blindjoin-linux-amd64.tar.gz.bundle` — a single
   JSON file containing the signature, Fulcio-issued cert, and Rekor
   transparency-log inclusion proof.
2. **Attested with a SLSA v1.0 in-toto provenance bundle** (predicate type
   `https://slsa.dev/provenance/v1`), naming the workflow file (`release.yml`)
   + tag ref + source commit + runner image. The attestation is pushed to the
   GitHub Attestations API AND distributed as
   `blindjoin-linux-amd64.tar.gz.sigstore`.

Verification requires **cosign 2.6.3 or compatible** (see the image subsection
above for the cosign version pin rationale) and the **GitHub CLI (`gh`) 2.50 or
later**. The verify recipes below have been tested on a clean `ubuntu:24.04`
container.

```bash
# 1. Cosign blob signature verification (SIGN-01)
cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
# Expected: "Verified OK" + JSON cert claims.

# 2. SLSA provenance — Path A (GitHub Attestations API; requires github.com reachable)
gh attestation verify blindjoin-linux-amd64.tar.gz --repo <owner>/blindjoin
# Expected: "Verification succeeded!" + attestation summary.

# 3. SLSA provenance — Path B (offline cosign verify; works after one-time TUF cache seeding)
cosign verify-attestation \
  --bundle blindjoin-linux-amd64.tar.gz.sigstore \
  --type slsaprovenance \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
# Expected: "Verified OK" + the SLSA v1.0 in-toto predicate.
# To inspect the SLSA predicate body itself, add --output-file slsa-predicate.json.
```

> **Note: cosign 3.0 CLI flag drift** — see the [image subsection above](#image-signatures--attestations-v16-onward) for the cosign version pin range; the same constraints apply to tarball verification.

> **Note: no detached PGP signature path.** An air-gapped operator who cannot
> reach Sigstore Fulcio/Rekor today must wait for that operator-side issue to
> be filed before blindjoin adds a maintainer-held PGP path. The cosign +
> SLSA paths above are sufficient for github.com-reachable verification.

### Reproducibility (v1.6 onward)

Every tagged release tarball is reproducible byte-for-byte from source on the
pinned `ubuntu-24.04` runner image. The full recipe, expected `sha256sum` per
release, and toolchain pins live in [docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md).
Continuous verification runs monthly via
[.github/workflows/reproducible-verify.yml](.github/workflows/reproducible-verify.yml);
a failure opens a `[reproducibility-regression]` issue. blindjoin is registered
with the reproducible-builds.org project registry: <added after blindjoin's submission lands; see [docs/RELEASING.md](docs/RELEASING.md) §Reproducible-builds.org registry submission>.

To verify a release yourself, follow the Recipe section of REPRODUCIBLE-BUILD.md.
Quick reference (single command from a clean checkout of the tag):

```bash
export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)
export RUSTFLAGS="--remap-path-prefix=$(pwd)=/build --remap-path-prefix=$HOME/.cargo=/cargo"
export CARGO_INCREMENTAL=0
cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot
mkdir -p dist
cp target/release/coordinator target/release/client target/release/liquidity-bot dist/
tar --sort=name --owner=0 --group=0 --numeric-owner \
    --mtime="@${SOURCE_DATE_EPOCH}" \
    -cf - -C dist . \
  | gzip -n > blindjoin-linux-amd64.tar.gz
sha256sum blindjoin-linux-amd64.tar.gz
# Compare against the expected hash in docs/REPRODUCIBLE-BUILD.md §Expected sha256sum.
```

> **Note: Rust reproducibility long tail** — bit-for-bit equality is verified on
> `ubuntu-24.04` only. Rebuilding on a different distribution or runner image
> may produce divergent bytes due to system-tool differences. The monthly
> verifier (linked above) catches `ubuntu-24.04` ImageVersion drift distinctly
> from real divergence per Phase 25 D-12.

### Base-image digests (v1.6 onward)

blindjoin's `docker/Dockerfile` derives from two upstream base images:
`debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. As
of v1.6 (Phase 22), both are pinned by digest in
[`docker/digests.txt`](docker/digests.txt) — the canonical manifest — and
every tagged release build passes those digests to `docker buildx build
--build-arg DEBIAN_REF=… --build-arg CARGO_CHEF_REF=…` automatically via
[`.github/actions/read-base-digests/`](.github/actions/read-base-digests/).

**`docker/digests.txt` is the canonical record of which upstream base
images each release was built from.** A release tagged `vX.Y.Z` was built
against the digests recorded at the same commit. An auditor reproducing
the build SHOULD use the same digest values.

**The manifest is bumped only by human-reviewed PR.** Both
`docker/digests.txt` and the parser action are listed in
[`.github/CODEOWNERS`](.github/CODEOWNERS); branch protection on `main`
requires maintainer approval on any PR touching either path.
**Do not auto-merge digest bumps** — auto-merging is the threat model
this gate exists to close. A compromised upstream base image accepted
via auto-merge would leak into the next release.

**Drift detection.** A scheduled workflow
([`.github/workflows/digest-drift-check.yml`](.github/workflows/digest-drift-check.yml))
runs daily at 09:00 UTC, resolves each pinned tag against the upstream
registry via `docker buildx imagetools inspect`, and opens a
`[digest-drift]`-labeled issue if the upstream digest has moved. The
workflow **opens issues, not PRs**, by design. The issue body includes a
retag-vs-substantive triage hint and the exact diff command an operator
can run locally before deciding to accept the new digest.

The workflow is idempotent — re-running it while a drift issue is open
does not create a duplicate (the search is keyed on the upstream digest
hex, so two different drifts of the same tag produce two different
issues). The workflow can be fired manually via `workflow_dispatch` from
the Actions tab; this is the recommended rehearsal path before pulling
any digest bump.

### v1.6 supply-chain plan

The v1.6 milestone has closed all four planned supply-chain items:

- **~~cosign image attestations~~** ✓ Shipped in Phase 23 — see [Image signatures + attestations (v1.6 onward)](#image-signatures--attestations-v16-onward).
- **~~Detached signatures on GitHub Release archives~~** ✓ Shipped in Phase 24 (cosign blob signatures + SLSA provenance; PGP path deferred indefinitely 2026-06-02) — see [Release tarball signatures + provenance (v1.6 onward)](#release-tarball-signatures--provenance-v16-onward).
- **~~Reproducible-build instructions~~** ✓ Shipped in Phase 25 — see [Reproducibility (v1.6 onward)](#reproducibility-v16-onward).
- **~~Automated base-image digest drift check~~** ✓ Shipped in Phase 22 — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).

For the highest assurance, build from source on a known-good toolchain and
verify against the committed `Cargo.lock` using the recipe in
[docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md) — then compare your
local sha256sum against the per-tag expected hash table in that same doc.

## Release versioning policy

**The canonical release identifier is the git tag** (e.g. `v1.5.0`)
plus the corresponding GitHub Release. That tag drives the
[`release.yml`](.github/workflows/release.yml) workflow — which
produces `blindjoin-linux-amd64.tar.gz` + `.sha256` companion — and
the [`docker.yml`](.github/workflows/docker.yml) workflow, which
produces `ghcr.io/<owner>/blindjoin-coordinator:X.Y.Z` images. When
talking about "what version is running", that's what to look at.

**Workspace crate `version` fields are intentionally pinned at `0.1.0`**
and do not track milestone releases. There are four workspace crates —
`coordinator`, `client`, `liquidity-bot`, `shared` — and none of them
are published to crates.io. The `version` field in their `Cargo.toml`
is effectively private and serves only as the
[Cargo dependency graph identifier](https://doc.rust-lang.org/cargo/reference/manifest.html#the-version-field).
There is no consumer who reads `0.1.0` and acts on it; bumping it at
every milestone close would be churn with zero downstream benefit.

The binaries currently expose no `--version` flag (see the
`coordinator-smoke` job comment in [`.github/workflows/ci.yml`](.github/workflows/ci.yml)),
so `CARGO_PKG_VERSION` is never user-visible. If a future milestone
adds `--version` flags, the policy is revisited: the binaries should
report the **git tag**, derived at build time via `GIT_DESCRIBE` or an
equivalent build script — not the static `Cargo.toml` value. This
keeps the displayed version honest under all build paths (including
ad-hoc local builds where there's no tag at all).

**TL;DR for contributors:**

- Tag releases as `v1.X.0` per [`CONTRIBUTING.md` § Tagging releases](CONTRIBUTING.md#tagging-releases).
- Add release notes to [`CHANGELOG.md`](CHANGELOG.md) before tagging.
- **Do not** bump the four workspace `version =` lines as part of a milestone close.
- If you do need to bump a crate `version` (e.g., preparing one for
  crates.io publication), that's a separate decision that gets its own
  discussion.

## Where to find more

- **Threat model and residual risks**: [`docs/AUDIT-CHARTER.md`](docs/AUDIT-CHARTER.md)
- **CI provenance and dependency pins**: [`.cargo/audit.toml`](.cargo/audit.toml),
  [`.github/workflows/`](.github/workflows/), and `.bitcoind-version`
- **Privacy considerations and operator policy**: [`README.md`](README.md)
  § Security Model and § Privacy Considerations
- **Release notes**: [`CHANGELOG.md`](CHANGELOG.md)
