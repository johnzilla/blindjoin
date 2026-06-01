---
quick_id: 260531-thw
status: complete
description: v1.5 release-readiness P0s (SECURITY.md + CHANGELOG, BACKLOG prune, CI integration, Dockerfile pins)
date: 2026-06-01
commits:
  - adc2aa6 — docs(quick-260531-thw): add SECURITY.md + CHANGELOG.md (P0-1)
  - 870ff71 — docs(quick-260531-thw): mark B-01/B-02 shipped + bump audit.toml date (P0-4)
  - 6fea538 — ci(quick-260531-thw): release-smoke runs integration suite (P0-5)
  - 578a903 — build(quick-260531-thw): pin Dockerfile bases via ARG + bump bitcoind 27→30 (P0-2/3)
files_changed:
  added:
    - SECURITY.md
    - CHANGELOG.md
    - .github/actions/install-bitcoind/action.yml
  modified:
    - .planning/BACKLOG.md
    - .cargo/audit.toml
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
    - docker/Dockerfile
    - docker/docker-compose.yml
---

# Quick task 260531-thw — Summary

Bundled four P0 items from the v1.5 release-readiness punch-list. All are
docs / CI / Docker; no protocol code touched. Four atomic commits, one per
P0 item.

## P0-1 — SECURITY.md + CHANGELOG.md ✓

- `SECURITY.md` at repo root: supported-versions table, responsible-disclosure
  flow with `johnturner@gmail.com` contact (sourced from
  `docs/AUDIT-CHARTER.md` frontmatter), audit-readiness status pointing at
  the charter, and a `## Supply-chain status` section that names the
  known v1.5 gaps (unsigned ghcr.io images, sha256-only release archives,
  no reproducible-build pipeline, manual base-image digest pins) and the
  v1.6 closure plan (cosign attestations, detached signatures, reproducible
  build instructions, automated digest drift check).
- `CHANGELOG.md` at repo root: Keep-a-Changelog format, one section per
  milestone v1.0 → v1.5. Entries seeded from
  `.planning/RETROSPECTIVE.md` § What Was Built and
  `.planning/MILESTONES.md` summaries — one bullet per requirement-class,
  not exhaustive code-level enumeration.

Commit: **adc2aa6**.

## P0-4 — BACKLOG.md prune + audit.toml date bump ✓

- `BACKLOG.md` preamble points readers at
  `.planning/PROJECT.md` § Carry-Forward Items and `.planning/workstreams/`
  as the active discovery surfaces; flags BACKLOG.md as forensic.
- **B-01 (Public-endpoint hardening)** marked `✓ Shipped in v1.2 Phase 8`
  with milestone link; original deferral rationale retained below as
  forensic record.
- **B-02 (BIP-322 multi-script)** marked `✓ Shipped across v1.4 Phases
  15-18 + v1.5 Phase 19` with milestone link; original deferral rationale
  retained.
- **B-03 (Dynamic fee estimation)** stays `Deferred` and adds an
  explicit cross-ref to `.planning/PROJECT.md` § Carry-Forward Items and
  to `docs/AUDIT-CHARTER.md` § Residual Risks: Operational where it is
  flagged **REQUIRED before mainnet flip**.
- "When to schedule" section updated to note the historical ordering
  is now historical (B-01 + B-02 shipped, B-03 open).
- `.cargo/audit.toml` `# Reviewed:` line bumped `2026-05-31 → 2026-06-01`
  (v1.5 ship date; marks fresh review pass as part of release readiness).
- `cargo audit` still exits 0 (verified locally).

Commit: **870ff71**.

## P0-5 — CI release-smoke runs integration suite ✓

- New composite action at `.github/actions/install-bitcoind/action.yml`
  containing the four bitcoind install steps lifted verbatim from
  `ci.yml`: `Read pinned bitcoind version`, `Cache bitcoind binary`,
  `Install bitcoind (cache miss only)` (PGP+SHA256 integrity gate), and
  `Export BITCOIND_EXE`. The gate's logic — fingerprint-pin against
  achow101's release-signer key fetched from a SHA-pinned guix.sigs
  commit, then SHA256SUMS hash check — is identical to the inline ci.yml
  version so no verification semantics drift between workflows.
- `.github/workflows/release.yml` check job:
  - adds `BLINDJOIN_REQUIRE_BITCOIND: "1"` at env level;
  - replaces inline bitcoind setup with `uses: ./.github/actions/install-bitcoind`;
  - swaps `cargo test --workspace --lib` → `cargo test --workspace --all-targets`;
  - bumps clippy to `--all-targets`.
- `.github/workflows/docker.yml` check job: same treatment.
- Trade-off documented inline in both workflows: ~30s on cache hit,
  ~90s on cache miss (first job after a `.bitcoind-version` bump).
  Acceptable for release-grade confidence.
- `ci.yml` left as-is for now — refactoring its inline steps to call the
  composite is a no-behavior-change follow-on, deferred to keep this
  quick task's blast radius small.

Verified all three YAML files parse via `python3 -c 'import yaml; ...'`.

Commit: **6fea538**.

## P0-2/3 — Dockerfile digest pins + bitcoind image bump ✓

- `docker/Dockerfile`:
  - Adds `ARG CARGO_CHEF_REF=lukemathwalker/cargo-chef:latest-rust-1`
    and `ARG DEBIAN_REF=debian:bookworm-slim` so release builds can
    pin to digest-form refs without editing the Dockerfile:
    ```
    docker build \
      --build-arg DEBIAN_REF=debian@sha256:<HEX> \
      --build-arg CARGO_CHEF_REF=lukemathwalker/cargo-chef@sha256:<HEX> \
      -f docker/Dockerfile -t blindjoin-coordinator:1.5.0 .
    ```
  - Top-of-file comment documents the digest-resolve-and-verify
    procedure (`docker pull` + `docker inspect --format='{{index .RepoDigests 0}}'`)
    and the v1.6 cosign/reproducible-build plan that supersedes the
    manual procedure.
- `docker/docker-compose.yml`:
  - bumps `bitcoin/bitcoin:27` → `bitcoin/bitcoin:30` to track the
    same major line as `.bitcoind-version` (currently 30.2) used in
    the CI integration tests.
  - inline comment documents the digest-pin procedure and the
    signing gap on the Docker Hub `bitcoin/bitcoin` image (the official
    guix.sigs PGP trust root is the path to independently verify).
- `SECURITY.md` § Supply-chain status already lists these gaps + the
  v1.6 closure plan; no additional callout needed.

Commit: **578a903**.

## Out of scope (deliberately not done)

- Actual digest values for `debian:bookworm-slim` and
  `lukemathwalker/cargo-chef:latest-rust-1` are NOT hardcoded. Pinning
  by digest in a Dockerfile is brittle without a clean-runner verify
  step; the ARG pattern lets the maintainer fill them in at release
  time. The v1.6 supply-chain milestone is the right place to automate
  the digest refresh + drift check.
- `ci.yml` was not refactored to use the composite action. That's a
  no-behavior-change follow-on; keeping it out of this quick task
  reduces blast radius.
- `cosign sign` / `cosign attest` not added to docker.yml. v1.6
  supply-chain milestone covers it per SECURITY.md plan.

## Verification

- `cargo audit` exits 0 (P0-4 sanity check).
- All YAML files parse (`python3 -c 'import yaml; ...'`).
- `grep ARG docker/Dockerfile` returns the 2 expected build args.
- `git log --oneline -5` shows 4 atomic commits, one per P0.
- `SECURITY.md` and `CHANGELOG.md` exist at repo root.

## Next

User can cut the v1.5 GitHub Release tag (e.g. `v1.5.0`) once they want
to publish. The release workflow will now block on the integration suite
under `BLINDJOIN_REQUIRE_BITCOIND=1`; a tag push that fails the gate
fails the release rather than silently skipping the integration tests.

For v1.6 scoping, the SECURITY.md § Supply-chain status > v1.6 plan
list is the concrete supply-chain workstream; the broader v1.6 candidate
list lives in `.planning/PROJECT.md` § Carry-Forward Items.
