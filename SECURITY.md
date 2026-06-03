# Security Policy

blindjoin is a CoinJoin coordinator and client for Bitcoin signet. Signet-first; mainnet is a config flag, with additional residual risks documented in [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md#residual-risks-operational).

## Reporting a vulnerability

Email **<johnturner@gmail.com>** with subject `[blindjoin security]`. Include a description, affected module(s), and a repro if you have one. Public GitHub issues are fine for non-security bugs; use email for anything affecting cross-shape rejection, script-type spoofing (CRIT-01), sighash regression (CRIT-02), the RSA zeroization window, or any unlinkability break.

Fixes land when they land — solo maintainer, no SLA. Reporters get CHANGELOG credit unless they ask to stay anonymous.

## Audit-readiness

v1.5 shipped the [external audit charter](docs/AUDIT-CHARTER.md) — in-scope modules, threat models, the 9 cross-shape rejection properties, the v=2 OwnershipProof boundary, the RSA zeroization window, and explicitly accepted residual risks. The charter is the source of truth; this file is a pointer.

## Supply-chain status

### Image signatures + attestations

Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z` push from a `vX.Y.Z` tag is cosign-signed (keyless OIDC, no maintainer key custody), SLSA v1.0 provenance-attested, and SBOM-attested (SPDX via Syft). Verify against the workflow identity:

```bash
cosign verify \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/<owner>/blindjoin-<image>:<tag>
```

cosign pin range: `>= 2.6.3, < 3.0.0`. The "Unverified" badge on the GHCR web UI does not consult Rekor — trust the CLI output, not the badge.

### Release tarball signatures + provenance

Every `blindjoin-linux-amd64.tar.gz` Release archive is cosign-signed (bundle: `blindjoin-linux-amd64.tar.gz.bundle`) and SLSA v1.0 provenance-attested (bundle: `blindjoin-linux-amd64.tar.gz.sigstore`). Verify:

```bash
cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
```

No PGP detached signature; Sigstore reachability is required.

### Reproducibility

Release tarballs rebuild byte-for-byte from source on `ubuntu-24.04`. Recipe + expected sha256 per tag: [docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md).

### Base-image digests

`docker/Dockerfile` derives from `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. Both are digest-pinned in [`docker/digests.txt`](docker/digests.txt) and threaded into `docker buildx --build-arg` via [`.github/actions/read-base-digests/`](.github/actions/read-base-digests/). Bumps go through PR review — `docker/digests.txt` is CODEOWNERS-gated.

To check upstream drift before a release, run `docker buildx imagetools inspect <image> --format '{{.Manifest.Digest}}'` against each entry in `docker/digests.txt` (see `docs/RELEASING.md`).

For the highest assurance, build from source per [docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md) and compare your sha256sum against the per-tag table.

## Operating notes

- **Back up the coordinator's PKARR key** (`coordinator_pkarr.key`, or the `coordinator-keys` Docker volume). Losing it creates a new DHT identity; participants holding your old `pk:...` will no longer discover you. The `docker/docker-compose.yml` volume note flags the same.
- **Ban-list durability.** `append_ban_entry` (`coordinator/src/round/blame.rs`) calls `sync_all()` after each write so a ban that returned `Ok()` to the caller survives the power-loss / kernel-panic / OOM-kill window a solo-VPS operator actually hits. The ban file lives in the `coordinator-data` volume — back it up alongside the keys volume if you want bans to outlive the host.

## Release versioning

The canonical release identifier is the git tag (`vX.Y.Z`) plus the matching GitHub Release. Workspace crate `version` fields stay pinned at `0.1.0` — none of the crates publish to crates.io, so the field is private and bumping it would be churn. Binaries currently expose no `--version`; if added later, they should report the git tag (`GIT_DESCRIBE` at build time), not `CARGO_PKG_VERSION`.

Tags: `v1.X.0` per [`CONTRIBUTING.md` § Tagging releases](CONTRIBUTING.md#tagging-releases). Release notes in [`CHANGELOG.md`](CHANGELOG.md) before tagging.

## Where to find more

- Threat model + residual risks — [`docs/AUDIT-CHARTER.md`](docs/AUDIT-CHARTER.md)
- CI pins + workflows — [`.github/workflows/`](.github/workflows/), `.bitcoind-version`, `.cargo/audit.toml`
- Privacy + operator notes — [`README.md`](README.md)
- Release notes — [`CHANGELOG.md`](CHANGELOG.md)
