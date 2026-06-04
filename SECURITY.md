# Security Policy

blindjoin is a CoinJoin coordinator and client for Bitcoin signet. Signet-first; mainnet is a config flag, with additional residual risks documented in [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md#residual-risks-operational).

## Reporting a vulnerability

Email **<johnturner@gmail.com>** with subject `[blindjoin security]`. Include a description, affected module(s), and a repro if you have one. Public GitHub issues are fine for non-security bugs; use email for anything affecting cross-shape rejection, script-type spoofing (CRIT-01), sighash regression (CRIT-02), the RSA zeroization window, or any unlinkability break.

Fixes land when they land — solo maintainer, no SLA. Reporters get CHANGELOG credit unless they ask to stay anonymous.

## Audit-readiness

v1.5 shipped the [external audit charter](docs/AUDIT-CHARTER.md) — in-scope modules, threat models, the 9 cross-shape rejection properties, the v=2 OwnershipProof boundary, the RSA zeroization window, and explicitly accepted residual risks. The charter is the source of truth; this file is a pointer.

## Supply-chain status

### Build provenance

Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z` image and every `blindjoin-linux-amd64.tar.gz` Release tarball from a `vX.Y.Z` tag carries a SLSA v1.0 build provenance attestation — keyless OIDC via Sigstore (no maintainer key custody), bound to the workflow identity that produced the artifact. Verify with the `gh` CLI; no extra install required:

```bash
# Image
gh attestation verify oci://ghcr.io/<owner>/blindjoin-<image>:<tag> --owner <owner>

# Tarball
gh release download vX.Y.Z --pattern blindjoin-linux-amd64.tar.gz
gh attestation verify blindjoin-linux-amd64.tar.gz --owner <owner>
```

Sigstore reachability is required at verify time. The "Unverified" badge on the GHCR web UI does not consult Rekor — trust the CLI output, not the badge. No PGP detached signature; no separate cosign signature; no SBOM attestation.

### Base-image digests

`docker/Dockerfile` pins both base images by sha256 digest in its `FROM` lines: `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. To check for upstream drift before a release, run `docker buildx imagetools inspect <image> --format '{{.Manifest.Digest}}'` against each (see [CONTRIBUTING.md §Bumping base-image digests](CONTRIBUTING.md#bumping-base-image-digests)); update the digest in the matching `FROM` line via a one-line PR.

For the highest assurance, build from source — `Cargo.lock`, `rust-toolchain.toml`, and `docker/Dockerfile` are all committed; see [README § Build from Source](README.md#build-from-source).

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
