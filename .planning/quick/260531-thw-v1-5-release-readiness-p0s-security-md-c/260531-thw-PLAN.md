---
quick_id: 260531-thw
status: in_progress
description: v1.5 release-readiness P0s (SECURITY.md + CHANGELOG, BACKLOG prune, CI integration, Dockerfile pins)
must_haves:
  truths:
    - SECURITY.md exists at repo root with disclosure policy + contact email
    - CHANGELOG.md exists at repo root with v1.0-v1.5 entries seeded from MILESTONES.md + RETROSPECTIVE.md
    - BACKLOG.md B-01 / B-02 marked Shipped (with milestone link); B-03 keeps "Deferred" with cross-ref to PROJECT.md Carry-Forward; BACKLOG header points at .planning/workstreams/ + PROJECT.md
    - .cargo/audit.toml "Reviewed:" date bumped to 2026-06-01 (v1.5 release-readiness re-review)
    - .github/workflows/release.yml and docker.yml check jobs run integration tests with BLINDJOIN_REQUIRE_BITCOIND=1 (composite action extracted from ci.yml)
    - docker/Dockerfile pins debian:bookworm-slim and lukemathwalker/cargo-chef base images by digest
    - docker/docker-compose.yml bumps bitcoind image from bitcoin/bitcoin:27 to a 30.x signed release
    - SECURITY.md contains a "Supply-chain status" callout naming unsigned-build gap + cosign/gpg plan for v1.6
  artifacts:
    - SECURITY.md
    - CHANGELOG.md
    - .planning/BACKLOG.md
    - .cargo/audit.toml
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
    - .github/actions/install-bitcoind/action.yml
    - docker/Dockerfile
    - docker/docker-compose.yml
  key_links:
    - docs/AUDIT-CHARTER.md (charter §Residual Risks; author + contact email)
    - .planning/RETROSPECTIVE.md (v1.5 milestone notes for CHANGELOG seeding)
    - .planning/MILESTONES.md (per-milestone "What was built" for CHANGELOG entries)
    - .planning/PROJECT.md (Carry-Forward Items, validated requirements per milestone)
---

# Quick task 260531-thw — v1.5 release-readiness P0s

Bundled punch-list before cutting the v1.5 GitHub release. Five mechanical
deliverables that don't change protocol code; all live in docs / CI / Docker.

## Task 1 — SECURITY.md + CHANGELOG.md (P0-1)

**files:** `SECURITY.md`, `CHANGELOG.md`

**action:**
- Write `SECURITY.md` covering supported versions, responsible-disclosure policy,
  contact email (johnturner@gmail.com, from AUDIT-CHARTER.md frontmatter), explicit
  v1.5 audit-readiness status, and a `## Supply-chain status` section that names
  the unsigned-build gap and points at the v1.6 cosign/gpg plan + the
  reproducible-build follow-up.
- Write `CHANGELOG.md` in Keep-a-Changelog format with one section per shipped
  milestone (v1.0 → v1.5). Seed each entry's bullets from the matching
  `## What Was Built` section in `.planning/RETROSPECTIVE.md` plus the milestone
  summary in `.planning/MILESTONES.md`. No exhaustive code-level enumeration —
  one bullet per requirement-class.

**verify:** Both files exist at repo root; `SECURITY.md` contains `johnturner@gmail.com`
and the supply-chain callout; `CHANGELOG.md` contains a `## [1.5.0]` heading and
the `## Supported versions` shape exists in `SECURITY.md`.

**done:** Both files committed; no link points to anchors that don't exist
in the linked docs.

## Task 2 — Prune BACKLOG + bump audit.toml date (P0-4)

**files:** `.planning/BACKLOG.md`, `.cargo/audit.toml`

**action:**
- `BACKLOG.md`: Mark B-01 (public-endpoint hardening) and B-02 (BIP-322
  multi-script) as `Status: ✓ Shipped` with the milestone link
  (v1.2 Phase 8 for B-01; v1.4 Phases 15-18 for B-02). Leave bodies in
  place as forensic record. B-03 (dynamic fee estimation) keeps its
  Deferred status and adds a cross-ref note pointing at
  `.planning/PROJECT.md` §Carry-Forward Items (where v1.6+ candidates
  live alongside CARRY-TOR-UAT, TEST-EXT-01/02/03, etc.). Update the
  preamble to point readers at `.planning/workstreams/` (where in-flight
  workstream context lives) and PROJECT.md (where the active carry-forward
  list lives) as the authoritative discovery surfaces.
- `.cargo/audit.toml`: Bump `# Reviewed:` from `2026-05-31` to `2026-06-01`
  (v1.5 ship date, marks fresh review pass as part of release readiness).

**verify:** B-01 / B-02 both show `Shipped` markers with milestone links;
B-03 still says `Deferred` and points at PROJECT.md; audit.toml `Reviewed:`
line says `2026-06-01`.

**done:** Files committed; `cargo audit` still exits 0.

## Task 3 — CI release-smoke with BLINDJOIN_REQUIRE_BITCOIND=1 (P0-5)

**files:** `.github/actions/install-bitcoind/action.yml` (new),
`.github/workflows/release.yml`, `.github/workflows/docker.yml`

**action:**
- Extract the `Read pinned bitcoind version` + `Cache bitcoind binary` +
  `Install bitcoind (cache miss only)` + `Export BITCOIND_EXE` steps from
  `ci.yml` into a composite action at `.github/actions/install-bitcoind`.
  Keep the PGP+SHA256 verification logic exactly as-is so the integrity
  gate is identical across all three workflows.
- `release.yml` check job: switch `cargo test --workspace --lib` to
  `cargo test --workspace --all-targets` under
  `BLINDJOIN_REQUIRE_BITCOIND=1` after invoking the new composite action.
- `docker.yml` check job: same treatment.
- Document the trade-off in a comment block on each workflow: bitcoind
  install adds ~30s on cache hit, ~90s on cache miss (one tag bump).
  Acceptable cost for release confidence that integration tests pass
  before a tag pushes binaries / images.
- `ci.yml` keeps its inlined steps — refactoring it to use the composite
  action is a follow-on (no behavior change, just DRY).

**verify:** Composite action file parses (`yamllint -d relaxed` if
available, otherwise inspect indentation manually); both release.yml and
docker.yml reference the composite via `uses: ./.github/actions/install-bitcoind`;
both run `cargo test --workspace --all-targets`; both set
`BLINDJOIN_REQUIRE_BITCOIND: "1"` at env-level.

**done:** Files committed; workflows still trigger on `push: tags ['v*']`.

## Task 4 — Dockerfile digest pins + supply-chain callout (P0-2/3)

**files:** `docker/Dockerfile`, `docker/docker-compose.yml`, `SECURITY.md`

**action:**
- `docker/Dockerfile`: Pin `lukemathwalker/cargo-chef:latest-rust-1` and
  `debian:bookworm-slim` to current digests. Add a NOTE comment block
  explaining the manual-bump cadence (per release; verified via
  `docker pull --quiet` + `docker inspect --format='{{index .RepoDigests 0}}'`
  on a clean runner). Digest pins below are the current production digests
  as of 2026-06-01.
- `docker/docker-compose.yml`: Bump `bitcoin/bitcoin:27` to `bitcoin/bitcoin:30`
  (matches `.bitcoind-version` = 30.2 used in CI), and pin by digest.
  Add a comment block explaining the version pin + signed-release source.
- `SECURITY.md` already added in Task 1 contains the unsigned-build callout
  and v1.6 cosign/gpg plan — confirm the language references "Docker images
  on ghcr.io are unsigned" + "binaries on GitHub Releases ship sha256 only,
  no PGP signature" + "v1.6 plan: cosign attestations on Docker images,
  detached PGP signatures on release archives".

**verify:**
- `grep -E '@sha256:' docker/Dockerfile` returns ≥ 2 lines (cargo-chef +
  debian).
- `grep -E '@sha256:' docker/docker-compose.yml` returns ≥ 1 line (bitcoind).
- `SECURITY.md` contains "unsigned" + "cosign" tokens in the supply-chain
  section.

**done:** All three files committed; `docker compose -f docker/docker-compose.yml config` (if docker available) still validates.

## Notes

- Single-task batches are intentional: each task is a leaf deliverable, so a
  failure in one doesn't block the others.
- No new dependencies added; no rust source touched.
- `git commit` cadence is per-task (4 atomic commits expected), matching the
  GSD quick-task convention.
