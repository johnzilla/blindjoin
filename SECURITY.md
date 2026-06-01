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

- **GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign
  signature.** A user pulling
  `blindjoin-linux-amd64.tar.gz` from a GitHub Release verifies the
  archive is intact (the `.sha256` companion file), but cannot
  cryptographically attribute the archive to the maintainer. A compromised
  GitHub account could publish a replaced binary with a matching checksum.
- **Docker images on `ghcr.io` are unsigned.** No cosign attestation, no
  Notary v2 signature, no Sigstore witness. Pulling
  `ghcr.io/<owner>/blindjoin-coordinator:1.5.0` verifies the image
  digest matches what the registry advertises, but does not bind the
  image to the maintainer's identity.
- **No reproducible-build pipeline.** The CI build is deterministic in
  the trivial sense (same inputs → same output on the same runner image),
  but blindjoin does not publish a reproducible-build recipe that an
  independent rebuilder can use to certify the official archive matches
  what the source tree produces.
- **Base image digest pins are manual.** The `docker/Dockerfile` pins
  `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`
  by digest as of v1.5, but bumping these digests requires a maintainer
  to verify the new digest against a clean `docker pull` on a clean
  runner. There is no automated drift check.

### v1.6 supply-chain plan

The next milestone is expected to close the unsigned-build gap:

- **cosign image attestations** on the Docker images pushed to `ghcr.io`,
  attached to the GitHub Actions OIDC identity (no maintainer key
  custody required).
- **Detached signatures** on GitHub Release archives — either cosign
  blob signatures or detached PGP signatures, depending on the audit
  feedback.
- **Reproducible-build instructions** for the release archive,
  documenting the exact toolchain version, target triple, and build
  command, so an independent rebuilder can compare bytes.
- **Automated base-image digest drift check** so a stale digest in
  `docker/Dockerfile` fails CI rather than silently sticking.

Until those land, **treat the SHA-256 checksum + the GitHub Release
provenance as the only assurance the archive came from this project**.
For higher assurance, build from source on a known-good toolchain and
verify against the committed `Cargo.lock`.

## Where to find more

- **Threat model and residual risks**: [`docs/AUDIT-CHARTER.md`](docs/AUDIT-CHARTER.md)
- **CI provenance and dependency pins**: [`.cargo/audit.toml`](.cargo/audit.toml),
  [`.github/workflows/`](.github/workflows/), and `.bitcoind-version`
- **Privacy considerations and operator policy**: [`README.md`](README.md)
  § Security Model and § Privacy Considerations
- **Release notes**: [`CHANGELOG.md`](CHANGELOG.md)
