# Changelog

All notable changes to blindjoin are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/).

Each entry is a high-level summary of what shipped in the milestone — one
or two bullets per requirement-class. For the engineering detail behind
each milestone see [`.planning/MILESTONES.md`](.planning/MILESTONES.md)
and [`.planning/RETROSPECTIVE.md`](.planning/RETROSPECTIVE.md); for the
threat-model treatment of v1.4 / v1.5 invariants see
[`docs/AUDIT-CHARTER.md`](docs/AUDIT-CHARTER.md).

## [Unreleased]

### Removed

- `.github/actions/read-base-digests/` composite action, `docker/digests.txt`
  canonical manifest, `.github/CODEOWNERS` file. Base-image sha256 digests
  now live inline in `docker/Dockerfile`'s two `FROM` lines (one image per
  PR to bump). The composite action's strict regex parser, 7-step contract,
  "Refusing to build without a valid manifest" auditor-pose error strings,
  and dual-source-of-truth between digests.txt + the Dockerfile ARGs all
  went with it. Outside-PR review is still gated by the `main` ruleset's
  `required_approving_review_count: 1` (see docs/branch-protection.md);
  the now-empty `require_code_owner_review: true` setting can be toggled
  off in the GitHub UI at the maintainer's leisure.

- Byte-equal reproducible build pipeline (`reproducible-verify.yml`,
  `docs/REPRODUCIBLE-BUILD.md`, `EXPECTED_SHA256` env, `SOURCE_DATE_EPOCH`
  derivation, `RUSTFLAGS=--remap-path-prefix=...`, `CARGO_INCREMENTAL=0`,
  deterministic 5-flag tar + `gzip -n`, `ubuntu-24.04` build-job pin). For
  a solo MIT project the cosign + SLSA provenance + committed
  `Cargo.lock` / `rust-toolchain.toml` / `Dockerfile` already cover the
  "this binary came from the official tagged workflow" question; byte-
  equality only catches a Sigstore-compromise scenario that isn't worth
  the ongoing fragility cost (every base-image rotation, Rust bump, or
  runner change is a potential divergence to debug). Anyone who wants to
  rebuild can still do so from source — see README §Build from Source.
  `cargo build --release --locked` (catches a stale `Cargo.lock`),
  `[profile.release] strip = "symbols"` (binary size), and
  `rust-toolchain.toml` (dev convenience) all stay. release.yml build
  job switched from `ubuntu-24.04` → `ubuntu-latest`.

## [1.6.0] — 2026-06-03

### Supply-chain attestation

Closed the v1.5 unsigned-build supply-chain gap. Every release tarball and ghcr.io image is cosign-signed (keyless OIDC, no maintainer key custody), SLSA v1.0 provenance-attested, and SPDX SBOM-attested. Base-image digests pin in `docker/digests.txt`. Release tarballs rebuild byte-for-byte from source on `ubuntu-24.04`.

12 of 14 v1 requirements shipped. SIGN-03 (PGP YubiKey path) deferred indefinitely 2026-06-02 (cosign+SLSA covers the threat model; PGP would be ongoing key-rotation cost for negligible solo-project benefit). REPRO-04 (reproducible-builds.org registry submission) descoped 2026-06-03.

- **Base-image digest pinning (DRIFT-01/02/03):** `docker/digests.txt` is the canonical manifest for `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. `release.yml` + `docker.yml` thread the values into builds via the [`.github/actions/read-base-digests/`](.github/actions/read-base-digests/) composite action. [`digest-drift-check.yml`](.github/workflows/digest-drift-check.yml) runs on `workflow_dispatch`, exits non-zero on drift. `docker/digests.txt` is CODEOWNERS-gated.
- **Image attestations (ATTEST-01..04):** Every tagged ghcr.io push gets `cosign sign` (keyless OIDC), `actions/attest-build-provenance` (SLSA v1.0), `actions/attest-sbom` + `anchore/sbom-action` (SPDX SBOM via Syft). cosign-installer pinned at v3.10.1 / cosign 2.6.3. `sigstore-pin-check` CI job catches floating tags on the four sigstore/sbom actions.
- **Release tarball signatures (SIGN-01/02):** Every `blindjoin-linux-amd64.tar.gz` ships a cosign `.bundle` companion and a SLSA `.sigstore` provenance bundle via the same `actions/attest-build-provenance` machinery.
- **Reproducible builds (REPRO-01/02/03):** `rust-toolchain.toml` pins rustc 1.95.0; `Cargo.toml` adds `[profile.release] strip = "symbols"`; `release.yml` build job runs on the pinned `ubuntu-24.04` with deterministic `RUSTFLAGS` + `CARGO_INCREMENTAL=0` + `SOURCE_DATE_EPOCH` from the tagged commit + `cargo build --release --locked` + deterministic 5-flag `tar` piped through `gzip -n`. Recipe + per-tag expected sha256: [docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md). On-demand verifier: [`reproducible-verify.yml`](.github/workflows/reproducible-verify.yml).
- **SECURITY.md** rewritten with single-recipe verify commands for image and tarball signatures and a single-line pointer to the reproducibility recipe.

**Theater strip after Phase 25 close** (commits `bf102ab` + `54e5ea5`, ~1,400 lines deleted): removed scaffolded auto-issue creation systems with two-title schemes + label-create + dedup (both digest-drift and reproducibility verifiers), removed monthly/daily cron schedules in favor of `workflow_dispatch:` only, collapsed multi-path verify recipes to single commands, removed the colon-delimited expected-sha256 lookup file in favor of inline env values, removed the reproducible-builds.org registry submission procedure, removed `rust-toolchain-pin-check` and the two `crit-01-grep-check` CI gates (the latter enforced presence of a comment rather than behavior), removed SECURITY.md disclosure-policy ceremony and strikethrough bookkeeping, stripped planning cross-references and auditor-pose comments throughout workflow files. Real cryptographic verification and supply-chain pinning intact.

Tagged 2026-06-03. Reproducibility baseline captured post-tag by dispatching `reproducible-verify.yml` against the published v1.6.0 release and committing the rebuilt sha256 into `EXPECTED_SHA256:` + the markdown table.

## [1.5.0] — 2026-06-01

### Added

- **Production BIP-322 `sign` bodies for P2TR and P2SH-P2WPKH** in
  `shared::bip322`. P2TR uses BIP-341 Schnorr keypath via
  `sign_schnorr_no_aux_rand`; P2SH-P2WPKH uses BIP-143 ECDSA over the
  unwrapped P2WPKH redeem. Byte-equality with
  `BdkClientWallet::sign_bip322` proven empirically.
- **Per-script BIP-141 vbyte table** in `coordinator/src/bitcoin/tx.rs`
  (P2WPKH 68/31, P2TR 58/43 round-UP, P2SH-P2WPKH 91/32). Replaces the
  legacy P2WPKH-only constants. v1.4 P2WPKH-only `fee_share == 266`
  byte-equality preserved by regression test; mixed-script branch fires
  with a measurable per-participant delta.
- **External audit charter** at `docs/AUDIT-CHARTER.md` (574 LOC,
  8 H2 sections) covering in-scope modules with `file:symbol` references,
  threat models per module, the 9 cross-shape rejection properties, the
  v=2 PSBT handling boundary, the RSA SecretKey zeroization window, and
  residual risks with explicit dispositions.
- **`SECURITY.md`** at the repo root documenting the responsible
  disclosure policy, supported versions, audit-readiness status, and
  the known supply-chain gaps (unsigned Docker images, no reproducible-
  build pipeline at v1.5).
- **`CHANGELOG.md`** (this file).

### Changed

- **RSA SecretKey lifetime tightened from prose to type signature**:
  `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` is the bounded
  lifetime; SOLE FSM chokepoint is `transition_to(Phase::Idle)` (verified
  by grep — no other site sets `self.inner = None`). The Drop body on
  `RoundSecretKey` is empty-crypto (PII-safe `tracing::debug!` only);
  the transitive `rsa-0.9.10` `ZeroizeOnDrop` chain on `RsaPrivateKey`
  does the cryptographic work.
- **`.cargo/audit.toml`** refreshed: each of the three RUSTSEC ignores
  (RUSTSEC-2023-0071 Marvin Attack, RUSTSEC-2025-0141 bincode
  unmaintained, RUSTSEC-2024-0436 paste unmaintained) cites the charter
  anchor for its full rationale. The "best-effort" framing on
  RUSTSEC-2023-0071 is gone; the AUDIT-03 bounded-window mitigation is
  named explicitly.
- **README § Security Model** links into the charter.

### Removed

- `sign_simple_test_only` + per-script `sign_for_tests` helpers. All
  test callsites migrated to the production `sign_simple` dispatcher;
  the V1.4-CRIT-01 dispatcher-only invariant is now load-bearing at the
  type level with no test-only escape hatch.

### Security

- AUDIT-03 chokepoint result-discard pattern (`let _ =` on the 3
  success-path FSM trigger sites) accepted as a defense-in-depth gap
  and documented in `AUDIT-CHARTER.md` § Residual Risks: Protocol-level.
  Closure deferred to v1.6+; closure trigger = any future FSM
  concurrency-model or `can_transition_to`-edge change.

## [1.4.0] — 2026-05-31

### Added

- **Multi-script BIP-322 coordinator + client** accepting P2WPKH, P2TR,
  and P2SH-P2WPKH ownership proofs via a `match version` dispatcher
  with on-chain CRIT-01 cross-check (script_type derived from
  `script_pubkey`, never trusted from the client-declared field).
- **Upstream `bip322 = "=0.0.10"` crate** adopted via a 26-LOC zero-lossy
  adapter at `shared/src/bip322/mod.rs`, replacing the in-tree
  implementation. Pinned via the `bip322-pin-check` CI grep gate.
- **[PKARR](https://github.com/pubky/pkarr) record v0.2.0** with `sst`/`ost` compact field names
  advertising supported / output script types in 209 bytes (11-byte
  headroom under the 220-byte threshold). Clients reject mismatched
  coordinators with `DiscoveryError::UnsupportedScriptType` **before**
  opening any Tor circuit.
- **v1.3 ↔ v1.4 backwards-compat shim** — v1.4 client → v1.3 coordinator
  emits byte-identical v1.3 array-of-hex `OwnershipProof` via CD-7
  two-phase try-parse; v1.3 client → v1.4 coordinator verified inline
  against a pinned v1.3 binary SHA `05f21438`.
- **Liquidity bot script-type rotation** via `BLINDJOIN_BOT_SCRIPT_TYPES`
  CSV + per-round rotation counter file. Defeats V1.4-MIN-02
  uniform-script fingerprint.
- **Mixed-script end-to-end acceptance test**
  `mixed_script_e2e_three_clients_broadcast` (1× P2WPKH + 1× P2TR +
  1× P2SH-P2WPKH input through INPUT_REG → BROADCAST on regtest).

### Removed

- Coordinator's P2WPKH-only registration gate at
  `coordinator/src/bitcoin/utxo.rs`. Replaced by the multi-script
  dispatcher.

### Security

- **V1.4-CRIT-01 script_type spoofing** mitigated by making the
  dispatcher the only public verify entry on `shared::bip322` — direct
  per-script callers are now `pub(crate)` and statically unreachable
  from outside the crate.

## [1.3.0] — 2026-05-29

### Added

- **Pinned `bitcoind` v30.2 in CI**, PGP-key-fingerprint-verified +
  SHA-256-verified against `SHA256SUMS.asc`. Cached at runner level so
  cache hits cost ~0s; cache misses re-run the integrity gate.
- **`BitcoindGuard` RAII fixture** + **`require_bitcoind!()` macro**.
  Replaces `Box::leak`-based fixtures across all `tests/integration/`
  callsites; eliminates "passes locally, hangs in CI" class of bugs.
- **`CONTRIBUTING.md` canonical integration-test invocation pattern.**
- **CI grep gate for `corepc-node` feature pin** (REPAIR-02 invariant):
  every `corepc-node = ...` declaration in any `Cargo.toml` must carry
  an explicit `features = ...` clause, or the gate fails the build.

### Fixed

- **`full_round.rs` repaired** — all 8 tests pass against pinned
  `bitcoind` v31. Fixes spanned RSA SPKI handshake, bdk_wallet 2.3
  `trust_witness_utxo`, wire-format `Witness` consensus encoding, real
  on-chain `witness_utxo`, ban-check ordering, and error-body surfacing.

## [1.2.0] — 2026-05-26

### Added

- **Per-route rate limiting on the coordinator HTTP API** via
  `tower_governor`. Read split 60/min, write split 30/min. HTTP 429 +
  `Retry-After` + `RATE_LIMITED` JSON envelope on rejection.
- **Uniform request timeout** (HTTP 408) honoring `request_timeout_secs`
  from `coordinator.toml`.
- **Tor accept-loop connection cap** via `tokio::sync::Semaphore`
  (default 256) with a `ConnectionGuard` RAII pattern.
- **`GlobalKeyExtractor`** for rate limiting — Tor-safe.
  `PeerIpKeyExtractor` would break under Tor where peer-IP is uniform.
- **Operator-tunable knobs** via `coordinator.toml` and
  `BLINDJOIN__COORDINATOR__*` env vars, validated at startup.

### Security

- **Release clearnet refusal** — coordinator refuses to start a release
  build with a non-Tor listener configured (signet flag carves an
  explicit exception).

## [1.1.0] — 2026-04-10

### Added

- **CI/CD security pipeline**: `cargo test`, `cargo clippy`, `cargo audit`
  as gates on PRs and releases. All GitHub Actions SHA-pinned. SHA-256
  checksums on release archives. Per-job permission scoping.

### Fixed

- **Eliminated write-lock DoS**: moved async Bitcoin Core RPC call
  outside the `RoundState` write lock in `post_input` (AVAIL-01).
- **Eliminated key-deserialization DoS**: `RsaBlindSigner` parsed once
  at round creation and reused across requests (AVAIL-02).
- **Input validation hardening**: blinded-token size bounds, address
  pre-validation, duplicate partial-sig guard, fee-formula
  consolidation.

## [1.0.0] — 2026-04-09

### Added

- **CoinJoin coordinator with RSA blind signatures** ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) ensuring
  cryptographic input-output unlinkability.
- **Round state machine**: `IDLE → INPUT_REG → OUTPUT_REG → SIGNING →
  BROADCAST | BLAME → IDLE`.
- **Blame protocol**: non-signer detection, UTXO banning with
  persistence, automatic round restart with remaining participants.
- **Client CLI** with `bdk_wallet` key management, per-phase Tor circuit
  isolation (Alice / Bob), anti-censorship PSBT verification.
- **UTXO ownership proof via BIP-322** (P2WPKH at v1.0; multi-script
  added at v1.4).
- **[PKARR](https://github.com/pubky/pkarr) DHT discovery**: coordinators publish `.onion` addresses, round
  parameters, and status to Mainline. Clients resolve without
  hardcoded addresses.
- **Tor v3 hidden service** via `arti-client`. No clearnet endpoint in
  production.
- **Docker Compose stack** (`bitcoind` + coordinator + liquidity bot) —
  zero-to-CoinJoin in 5 minutes on signet.
- **Liquidity bot** auto-joins rounds for testing and cold-start.
- **Integration tests**: full round (3+ participants), blame protocol,
  adversarial scenarios on signet.
- **Pre-built binaries** via GitHub Releases; multi-arch Docker images
  via `ghcr.io`.

### Security

- All round state zeroed from memory after transaction broadcast.
- No logging of PII, IP addresses, or input-output mappings.
