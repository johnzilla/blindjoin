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

### v1.6 — supply-chain attestation (in progress)

- **Phase 22 — base-image digest drift detection (DRIFT-01/02/03).**
  - [`docker/digests.txt`](docker/digests.txt) ships as the canonical pin manifest
    for `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`
    (one `image:tag@sha256:HEX` per line, regex-validated).
  - New composite action [`.github/actions/read-base-digests/`](.github/actions/read-base-digests/)
    parses the manifest with a fail-fast contract (4+ auditor-grepable
    `Refusing to build without a valid manifest.` error trailers); emits
    named `debian_ref` / `cargo_chef_ref` outputs.
  - [`release.yml`](.github/workflows/release.yml) and
    [`docker.yml`](.github/workflows/docker.yml) now read the manifest
    automatically — tagged builds no longer need manual `--build-arg DEBIAN_REF=…`.
  - New scheduled workflow
    [`digest-drift-check.yml`](.github/workflows/digest-drift-check.yml) runs daily
    (`0 9 * * *` UTC) plus `workflow_dispatch`; resolves upstream digests via
    `docker buildx imagetools inspect` and opens a `[digest-drift]` GitHub
    issue when the canonical and upstream digests diverge. Idempotent by
    upstream digest hex — a second run with the same drift does not open a
    duplicate.
  - Governance: new [`.github/CODEOWNERS`](.github/CODEOWNERS) maps both the
    manifest and the parser action to the maintainer. GitHub Ruleset on `main`
    (`require_code_owner_review: true`) blocks unreviewed digest bumps for
    outside contributors. See [docs/branch-protection.md](docs/branch-protection.md)
    for the full ruleset config and the documented admin-bypass trade-off.
  - Documented in [SECURITY.md §Supply-chain status](SECURITY.md#supply-chain-status)
    and [CONTRIBUTING.md §Bumping base-image digests](CONTRIBUTING.md#bumping-base-image-digests).

- Remaining v1.6 phases (cosign image attestations + SLSA + SBOM,
  release-tarball signing, reproducible-build recipe) are not yet started.
  See [`.planning/REQUIREMENTS.md`](.planning/REQUIREMENTS.md) for the
  full milestone scope.

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
- **PKARR record v0.2.0** with `sst`/`ost` compact field names
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

- **CoinJoin coordinator with RSA blind signatures** (RFC 9474) ensuring
  cryptographic input-output unlinkability.
- **Round state machine**: `IDLE → INPUT_REG → OUTPUT_REG → SIGNING →
  BROADCAST | BLAME → IDLE`.
- **Blame protocol**: non-signer detection, UTXO banning with
  persistence, automatic round restart with remaining participants.
- **Client CLI** with `bdk_wallet` key management, per-phase Tor circuit
  isolation (Alice / Bob), anti-censorship PSBT verification.
- **UTXO ownership proof via BIP-322** (P2WPKH at v1.0; multi-script
  added at v1.4).
- **PKARR DHT discovery**: coordinators publish `.onion` addresses, round
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
