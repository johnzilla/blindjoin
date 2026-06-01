# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- ✅ **v1.1 Security & Availability Hardening** — Phases 6-7 (shipped 2026-04-10)
- ✅ **v1.2 Production Readiness** — Phase 8 (shipped 2026-05-26)
- ✅ **v1.3 Test Infrastructure & Operational Hardening** — Phases 9-13 (shipped 2026-05-29)
- ✅ **v1.4 BIP-322 Multi-Script Support** — Phases 14-18 (shipped 2026-05-31)
- ✅ **v1.5 Audit-Readiness & Multi-Script Finish** — Phases 19-21 (shipped 2026-06-01)
- 📋 **v1.6 Supply-Chain Attestation** — Phases 22-25 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-04-09</summary>

- [x] Phase 1: Core Protocol (6/6 plans) — completed 2026-04-09
- [x] Phase 2: Blame & Hardening (3/3 plans) — completed 2026-04-09
- [x] Phase 3: Client CLI (2/2 plans) — completed 2026-04-09
- [x] Phase 4: Discovery & Deployment (3/3 plans) — completed 2026-04-09
- [x] Phase 5: Tor & Release (3/3 plans) — completed 2026-04-09

</details>

<details>
<summary>✅ v1.1 Security & Availability Hardening (Phases 6-7) — SHIPPED 2026-04-10</summary>

- [x] Phase 6: CI/CD Security Pipeline (1/1 plans) — completed 2026-04-10
- [x] Phase 7: Coordinator DoS Hardening (3/3 plans) — completed 2026-04-10

</details>

<details>
<summary>✅ v1.2 Production Readiness (Phase 8) — SHIPPED 2026-05-26</summary>

- [x] Phase 8: Public-endpoint hardening (4/4 plans) — completed 2026-05-26

</details>

<details>
<summary>✅ v1.3 Test Infrastructure & Operational Hardening (Phases 9-13) — SHIPPED 2026-05-29</summary>

- [x] Phase 9: CI integration-test reliability (5/5 plans) — completed 2026-05-27
- [x] Phase 10: full_round.rs decision + execution (2/2 plans; Task 3 carry-forward) — completed 2026-05-28
- [x] Phase 11: RSA SPKI handshake + unmute (carry-forward from 10) — closed via direct commits 2026-05-28
- [x] Phase 12: bdk_wallet 2.3 trust_witness_utxo (carry-forward from 11) — closed via direct commits 2026-05-28
- [x] Phase 13: Wire-format Witness encoding + unmute (carry-forward from 12) — closed via direct commits 2026-05-29

</details>

<details>
<summary>✅ v1.4 BIP-322 Multi-Script Support (Phases 14-18) — SHIPPED 2026-05-31</summary>

- [x] Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions (3/3 plans) — completed 2026-05-29
- [x] Phase 15: Shared Crate Multi-Script Contract (3/3 plans) — completed 2026-05-30
- [x] Phase 16: Coordinator Integration & Advertisement (3/3 plans) — completed 2026-05-30
- [x] Phase 17: Client Multi-Script Wallet & Discovery (3/3 plans) — completed 2026-05-30
- [x] Phase 18: Mixed-Script E2E + Liquidity Bot (3/3 plans) — completed 2026-05-31

</details>

<details>
<summary>✅ v1.5 Audit-Readiness & Multi-Script Finish (Phases 19-21) — SHIPPED 2026-06-01</summary>

- [x] Phase 19: Multi-Script Signing Finish (2/2 plans) — completed 2026-05-31
- [x] Phase 20: Mixed-Round Fee Accuracy (1/1 plan) — completed 2026-05-31
- [x] Phase 21: Audit Charter & Zeroization Tightening (2/2 plans) — completed 2026-05-31

</details>

### 📋 v1.6 Supply-Chain Attestation (Phases 22-25) — IN PROGRESS

- [x] **Phase 22: Base-Image Digest Drift Detection** — Commit a canonical digest manifest and a scheduled drift-check workflow that opens issues (not PRs) on drift, with release builds reading digests from the manifest. (completed 2026-06-01)
- [ ] **Phase 23: cosign Image Attestations + SLSA Provenance + SBOM** — Every ghcr.io image is signed via OIDC keyless flow with SLSA v1.0 provenance, an SPDX SBOM attestation, and a downloadable cosign `.bundle`.
- [ ] **Phase 24: Release Tarball Signing (cosign + SLSA + PGP)** — Every release tarball ships a cosign signature, SLSA provenance, and a detached PGP signature as a non-OIDC alternative verification path.
- [ ] **Phase 25: Reproducible-Build Recipe + Scheduled Verifier + Registry** — Publish a reproducible-build recipe, run an `ubuntu-24.04`-pinned monthly verifier that asserts byte-equality and opens issues on drift, and register with reproducible-builds.org.

## Phase Details

### Phase 22: Base-Image Digest Drift Detection
**Goal**: An operator's release build is always built from the canonical, human-reviewed list of base-image digests, and any upstream drift surfaces as a `[digest-drift]` issue for human review within 24 hours.
**Depends on**: Nothing (independent — first v1.6 phase per ARCHITECTURE.md ordering; lowest risk, builds digest discipline before signing layers on top).
**Requirements**: DRIFT-01, DRIFT-02, DRIFT-03
**Success Criteria** (what must be TRUE):
  1. `docker/digests.txt` exists as the canonical manifest (one `image:tag@sha256:HEX` per line for `debian:bookworm-slim` + `lukemathwalker/cargo-chef:latest-rust-1`), and a non-human-reviewed PR that attempts to bump it cannot be auto-merged (policy documented in SECURITY.md + CONTRIBUTING.md per Pitfall 11).
  2. A maintainer running `gh workflow run digest-drift-check.yml` against a tag whose registry digest has moved sees a new issue titled `[digest-drift] <image>:<tag> moved to sha256:<HEX>` appear within the workflow run; running it a second time with the same drift does NOT open a duplicate (idempotency, Pitfall 9).
  3. A tagged release build (`release.yml` and `docker.yml`) succeeds without any manual `--build-arg DEBIAN_REF=...` argument because the workflows read `docker/digests.txt` and pass the digests automatically; `grep '@sha256:' docker/digests.txt` against the build logs confirms the canonical digest was used.
  4. `digest-drift-check.yml` runs on the daily `schedule` cron AND on `workflow_dispatch`; absence of an open `[digest-drift]` issue after a successful run is observable evidence "no drift today".
**Plans**:
- [x] 22-01-PLAN.md — Create canonical `docker/digests.txt` manifest + `.github/CODEOWNERS` governance gate (DRIFT-01)
- [x] 22-02-PLAN.md — Create `.github/actions/read-base-digests/` composite action with fail-fast regex validation (DRIFT-01)
- [x] 22-03-PLAN.md — Wire `release.yml` + `docker.yml` to consume the composite action; thread `DEBIAN_REF` + `CARGO_CHEF_REF` build-args from manifest (DRIFT-03)
- [x] 22-04-PLAN.md — Create `.github/workflows/digest-drift-check.yml` scheduled workflow with Pitfall 9 idempotency + `docker buildx imagetools inspect` resolution (DRIFT-02)
- [x] 22-05-PLAN.md — Update `SECURITY.md` §Supply-chain status + add `CONTRIBUTING.md` §Bumping base-image digests (D-05 prose half of the gate)
- [x] 22-06-PLAN.md — Human-UAT: branch-protection toggle + fresh-machine rehearsal of ROADMAP SC#1-4

### Phase 23: cosign Image Attestations + SLSA Provenance + SBOM
**Goal**: Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z` image carries a cryptographically verifiable binding to the maintainer's GitHub Actions OIDC identity, the source commit it was built from, and a machine-readable SBOM — all reachable in the registry without maintainer key custody.
**Depends on**: Phase 22 (canonical digest list informs which base layers the attestation covers; first phase to need `id-token: write` and the sigstore action pin discipline).
**Requirements**: ATTEST-01, ATTEST-02, ATTEST-03, ATTEST-04
**Success Criteria** (what must be TRUE):
  1. An operator pulling `ghcr.io/<owner>/blindjoin-coordinator:1.6.0` and running `cosign verify --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' <image>` from a fresh machine returns exit code 0 with a parseable JSON verification report (Pitfall 1 identity-regexp shape, Pitfall 12 fresh-machine UAT).
  2. `cosign download attestation --predicate-type https://slsa.dev/provenance/v1 ghcr.io/<owner>/blindjoin-coordinator:1.6.0` returns a SLSA v1.0 in-toto provenance bundle naming `docker.yml` as the builder workflow, the `refs/tags/v1.6.0` ref, and the source commit SHA — emitted by `actions/attest-build-provenance` (NOT `slsa-github-generator`, per Pitfall 5 single-path choice).
  3. `cosign download attestation --predicate-type https://spdx.dev/Document ghcr.io/<owner>/blindjoin-coordinator:1.6.0` returns an SPDX-format SBOM (generated by Syft) listing the OS-package and Rust crate inventory; an operator can `grep` it for a CVE-identified package without pulling the image.
  4. A `.bundle` file (Sigstore bundle format: signature + cert + Rekor inclusion proof) is downloadable per image and re-verifies offline against the locally-cached Sigstore TUF root once seeded.
  5. SECURITY.md `## Supply-chain status` is updated to remove the "Docker images on ghcr.io are unsigned" gap and replace it with the canonical `cosign verify` command (including the explicit callout that the GHCR UI "Unverified" badge is unrelated to cosign verification per Pitfall 10).
**Plans**: TBD

### Phase 24: Release Tarball Signing (cosign + SLSA + PGP)
**Goal**: Every `blindjoin-linux-amd64.tar.gz` published as a GitHub Release asset can be cryptographically attributed to the maintainer via TWO independent paths — the OIDC-keyless cosign path (consistent with image signing) AND a maintainer-held PGP key path for operators who cannot reach Sigstore Fulcio/Rekor at verification time.
**Depends on**: Phase 23 (reuses the SHA-pinned `sigstore/cosign-installer` action and the `--certificate-identity-regexp` shape; SECURITY.md verification-commands template).
**Requirements**: SIGN-01, SIGN-02, SIGN-03
**Success Criteria** (what must be TRUE):
  1. Every GitHub Release at v1.6.0+ includes the tarball plus a companion cosign `.bundle` (or discrete `.sig` + `.crt`) asset; running `cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' blindjoin-linux-amd64.tar.gz` from a fresh machine returns exit code 0 (Pitfall 12 fresh-machine UAT).
  2. A SLSA v1.0 provenance attestation is downloadable for the tarball via the same `actions/attest-build-provenance` machinery as Phase 23, naming `release.yml` as the builder; the cosign verify recipe in SECURITY.md is structurally identical for image and tarball artifacts (consistent verifier UX).
  3. A maintainer-held PGP public key is committed at `docs/pgp/<fingerprint>.asc` AND uploaded to `keys.openpgp.org`; the fingerprint is documented in SECURITY.md; every release tarball ships a `.asc` detached signature that an operator verifies via `gpg --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz` without touching Sigstore.
  4. SECURITY.md `## Supply-chain status` carries BOTH verification recipes (cosign + PGP) side by side, with an explicit note that EITHER path is sufficient — and pins a documented operator-side cosign version range (Pitfall 13 cosign 3.0 CLI drift mitigation).
**Plans**: TBD

### Phase 25: Reproducible-Build Recipe + Scheduled Verifier + Registry
**Goal**: An independent rebuilder can confirm the `blindjoin-linux-amd64.tar.gz` GitHub Release artifact is the byte-for-byte natural product of the source tree at the tagged commit — and a scheduled CI verifier proves blindjoin's reproducibility claim continuously rather than just at ship time.
**Depends on**: Phase 24 (the scheduled verifier also checks the cosign signature alongside byte-equality; assumes the canonical digest path from Phase 22 is already loading `ubuntu-24.04` base layers identically).
**Requirements**: REPRO-01, REPRO-02, REPRO-03, REPRO-04
**Success Criteria** (what must be TRUE):
  1. `docs/REPRODUCIBLE-BUILD.md` documents the exact Rust toolchain version (matches `ci.yml` pin), the exact `ubuntu-24.04` runner image version (NOT `ubuntu-latest` per Pitfall 7), all required env vars (`SOURCE_DATE_EPOCH`, `RUSTFLAGS=--remap-path-prefix=...`, `CARGO_INCREMENTAL=0`), the exact `cargo build --release --locked` invocation, and the expected `sha256sum` for the v1.6.0 release tarball — an external rebuilder running the documented recipe on a fresh `ubuntu-24.04` runner produces a tarball whose sha256 matches the committed expected value.
  2. `.github/workflows/release.yml`'s build job uses `cargo build --release --locked` explicitly (not implicit), `SOURCE_DATE_EPOCH` derived from `$(git log -1 --format=%ct $GITHUB_SHA)`, `RUSTFLAGS` set per REPRO-01, and `CARGO_INCREMENTAL=0` in env — the recipe in `REPRODUCIBLE-BUILD.md` and the workflow are anchored to each other via comment cross-reference.
  3. `.github/workflows/reproducible-verify.yml` runs monthly on `schedule: cron` AND on `workflow_dispatch`, pulls the latest release tarball via `gh release download`, rebuilds on a `runs-on: ubuntu-24.04` (pinned, NOT `ubuntu-latest`, per Pitfall 7) runner per the REPRO-01 recipe, and asserts `sha256sum` equality; on mismatch it opens a `[reproducibility-regression]` issue whose message distinguishes "runner image drift" from "actual sha256 mismatch on identical env" (Pitfall 7).
  4. After REPRO-01 + REPRO-03 have been continuously green for ≥1 monthly verification cycle, blindjoin is registered with [reproducible-builds.org](https://reproducible-builds.org) project registry; the public registry entry links to `docs/REPRODUCIBLE-BUILD.md`, observable by visiting the registry page.
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Protocol | v1.0 | 6/6 | Complete | 2026-04-09 |
| 2. Blame & Hardening | v1.0 | 3/3 | Complete | 2026-04-09 |
| 3. Client CLI | v1.0 | 2/2 | Complete | 2026-04-09 |
| 4. Discovery & Deployment | v1.0 | 3/3 | Complete | 2026-04-09 |
| 5. Tor & Release | v1.0 | 3/3 | Complete | 2026-04-09 |
| 6. CI/CD Security Pipeline | v1.1 | 1/1 | Complete | 2026-04-10 |
| 7. Coordinator DoS Hardening | v1.1 | 3/3 | Complete | 2026-04-10 |
| 8. Public-endpoint hardening | v1.2 | 4/4 | Complete | 2026-05-26 |
| 9. CI integration-test reliability | v1.3 | 5/5 | Complete | 2026-05-27 |
| 10. full_round.rs decision + execution | v1.3 | 2/2 | Complete | 2026-05-28 |
| 11-13. REPAIR-01 carry-forward (shipped as direct commits) | v1.3 | n/a | Closed-local | 2026-05-29 |
| 14. Sprint-0 Spikes + Discuss-Phase Decisions | v1.4 | 3/3 | Complete | 2026-05-29 |
| 15. Shared Crate Multi-Script Contract | v1.4 | 3/3 | Complete | 2026-05-30 |
| 16. Coordinator Integration & Advertisement | v1.4 | 3/3 | Complete | 2026-05-30 |
| 17. Client Multi-Script Wallet & Discovery | v1.4 | 3/3 | Complete | 2026-05-30 |
| 18. Mixed-Script E2E + Liquidity Bot | v1.4 | 3/3 | Complete | 2026-05-31 |
| 19. Multi-Script Signing Finish | v1.5 | 2/2 | Complete | 2026-05-31 |
| 20. Mixed-Round Fee Accuracy | v1.5 | 1/1 | Complete | 2026-05-31 |
| 21. Audit Charter & Zeroization Tightening | v1.5 | 2/2 | Complete | 2026-05-31 |
| 22. Base-Image Digest Drift Detection | v1.6 | 6/6 | Complete    | 2026-06-01 |
| 23. cosign Image Attestations + SLSA + SBOM | v1.6 | 0/? | Not started | — |
| 24. Release Tarball Signing (cosign + SLSA + PGP) | v1.6 | 0/? | Not started | — |
| 25. Reproducible-Build Recipe + Verifier + Registry | v1.6 | 0/? | Not started | — |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
Full v1.1 details: `.planning/milestones/v1.1-ROADMAP.md`
Full v1.2 details: `.planning/milestones/v1.2-ROADMAP.md`
Full v1.3 details: `.planning/milestones/v1.3-ROADMAP.md`
Full v1.4 details: `.planning/milestones/v1.4-ROADMAP.md`
Full v1.5 details: `.planning/milestones/v1.5-ROADMAP.md`
