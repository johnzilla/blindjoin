---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: Supply-Chain Attestation
status: executing
last_updated: "2026-06-01T21:20:34.556Z"
last_activity: 2026-06-01
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 6
  completed_plans: 1
  percent: 0
---

# Project State

## Current Position

Phase: 22 (base-image-digest-drift-detection) — EXECUTING
Plan: 2 of 6
Status: Ready to execute
Last activity: 2026-06-01

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-06-01 — v1.6 Current Milestone)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 22 — base-image-digest-drift-detection

## Milestone Map

v1.5 shipped 2026-06-01. Phase artifacts archived to `.planning/milestones/v1.5-phases/`. Full per-phase details in `.planning/milestones/v1.5-ROADMAP.md` and `.planning/milestones/v1.5-REQUIREMENTS.md`. Per-milestone summary in `.planning/MILESTONES.md`.

v1.6 in progress. Roadmap approved 2026-06-01:

- **Phase 22 — Base-Image Digest Drift Detection** (DRIFT-01, DRIFT-02, DRIFT-03) — canonical `docker/digests.txt` + scheduled drift-check workflow that opens issues (not PRs) per Pitfall 11; release/docker workflows read the manifest automatically.
- **Phase 23 — cosign Image Attestations + SLSA Provenance + SBOM** (ATTEST-01, ATTEST-02, ATTEST-03, ATTEST-04) — every ghcr.io image signed via OIDC keyless flow with SLSA v1.0 provenance via `actions/attest-build-provenance` (Pitfall 5 choice), SPDX SBOM, and `.bundle` for offline verification.
- **Phase 24 — Release Tarball Signing (cosign + SLSA + PGP)** (SIGN-01, SIGN-02, SIGN-03) — release tarballs ship cosign blob signature + SLSA provenance + detached PGP signature as a non-OIDC alternative path.
- **Phase 25 — Reproducible-Build Recipe + Verifier + Registry** (REPRO-01, REPRO-02, REPRO-03, REPRO-04) — `docs/REPRODUCIBLE-BUILD.md` + release.yml determinism env + scheduled `reproducible-verify.yml` pinned to `ubuntu-24.04` (NOT `ubuntu-latest` per Pitfall 7) + reproducible-builds.org registration.

Phase artifacts will land under `.planning/phases/` as they're planned (start with `/gsd:discuss-phase 22`).

## Blockers

None.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260531-thw | v1.5 release-readiness P0s: SECURITY.md + CHANGELOG.md, BACKLOG prune, CI release-smoke, Dockerfile digest pins | 2026-06-01 | 578a903 | [260531-thw-v1-5-release-readiness-p0s-security-md-c](./quick/260531-thw-v1-5-release-readiness-p0s-security-md-c/) |
| 260531-ubf | Post-release polish: amp cleanup, README/CONTRIBUTING cross-links to SECURITY+CHANGELOG, crate-version policy, release-smoke rehearsal trigger | 2026-06-01 | ceca7b4 | [260531-ubf-post-release-readiness-polish-remove-amp](./quick/260531-ubf-post-release-readiness-polish-remove-amp/) |

## Deferred Items

Items acknowledged and deferred at v1.5 milestone close on 2026-05-31:

| Category | Item | Status | Note |
|----------|------|--------|------|
| uat_gap_scanner_false_positive | 21-HUMAN-UAT.md | resolved | All 3 items dispositioned (3 passed / 0 pending). Scanner over-reports resolved UAT files; no action needed. |
| quick_task_scanner_false_positive | 260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos | shipped 2026-05-26 | SUMMARY.md exists at .planning/quick/. Scanner flagged `missing` because frontmatter lacks a `status:` field; this is a quick-task template issue, not a real gap. |

## Carry-Forward Items (v1.6+ candidates)

Full list with rationale lives in `.planning/PROJECT.md` §Carry-Forward Items. Headline items:

- **CARRY-TOR-UAT**, **CARRY-REPAIR-01-PR**, **B-03** (dynamic fee estimation), **TEST-EXT-01/02/03** (differential + on-chain anchor + backwards-compat CI), **P2WSH multisig BIP-322**, **Mixed output script types**, **per-input variable `fee_share`**, **AUDIT-03 chokepoint `let _ =` closure** (REVIEW.md CR-01, accepted as defense-in-depth gap per AUDIT-CHARTER.md §7).

## Accumulated Context

### Standing cross-phase invariants (preserved across milestones)

The v1.3 P2WPKH-only `full_round::*` integration tests (8 tests) MUST remain green at every phase boundary. v1.4 `mixed_script_e2e_three_clients_broadcast` (1 test) MUST also remain green. These are the rollback safety nets that have held since v1.3 REPAIR-01 forensics and v1.4 INTEG-01 acceptance.

v1.6 adds a new category of cross-phase invariants — supply-chain release-pipeline gates — that get enforced in CI as each phase ships:

- Phase 22 onward: `digest-drift-check.yml` runs daily; a `[digest-drift]` issue is acknowledged within the maintainer review SLA before the next release tag.
- Phase 23 onward: every ghcr.io image push produces a cosign signature + SLSA provenance + SBOM; absence is a release blocker.
- Phase 24 onward: every release tarball has a cosign `.bundle` + PGP `.asc` companion; absence is a release blocker.
- Phase 25 onward: monthly `reproducible-verify.yml` runs are green; a `[reproducibility-regression]` issue is investigated within the same monthly cycle.

### Load-bearing invariants (shipped, must not regress)

- **V1.4-CRIT-01** — `shared::bip322` dispatcher-only public surface (9 symbols exactly). After v1.5 Phase 19 closed the test-only `sign_simple_test_only` escape hatch, this is load-bearing at the type level with no holes.
- **CRIT-01 cross-check** in `coordinator::validate_utxo` — derives `ScriptType` from on-chain `script_pubkey` (NEVER from client-declared field). Preserved into the fee path by v1.5 Phase 20 (`ParticipantInput.script_type` plumbed through `dispatch_ownership_proof → UtxoDetails → RegisteredInput`).
- **CD-7 two-phase try-parse** on `OwnershipProof` — v1.3↔v1.4 wire compat preserved byte-exactly.
- **`bip322 = "=0.0.10"` exact pin** + `bip322-pin-check` CI gate.
- **AUDIT-03 structurally-bounded RSA SecretKey lifetime** — `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` with sole FSM chokepoint at `state.rs:202` (`transition_to(Phase::Idle)`). Verified by structural FSM test + grep gate. Charter §5 in `docs/AUDIT-CHARTER.md` is the auditor-facing description.

Full per-milestone invariant detail in `.planning/milestones/v1.5-ROADMAP.md` and earlier milestone archives.

## Recent Plan Decisions

v1.5 plan decisions archived to `.planning/milestones/v1.5-phases/{19,20,21}-*/`. Cumulative trends live in `.planning/RETROSPECTIVE.md`.

v1.6 roadmap-level decisions (2026-06-01):

- **`actions/attest-build-provenance` over `slsa-framework/slsa-github-generator`** (Pitfall 5) — simpler matrix-style integration with existing `docker.yml`, no workflow restructure. Locked at roadmap level; Phase 23 PLAN.md must cite this choice.
- **Digest-drift opens issues, not PRs** (Pitfall 11) — auto-merging digest bumps is the supply-chain risk v1.6 is closing. Human review is the whole point.
- **Verifier pins `ubuntu-24.04`, NOT `ubuntu-latest`** (Pitfall 7) — `ubuntu-latest` rotation would produce false-positive reproducibility regressions every ~month. Explicit version pin makes the breaking event observable.
- **`--certificate-identity-regexp` over `--certificate-identity`** (Pitfall 1) — exact identity binding breaks on every new tag. Regex bound to the workflow file + tag namespace survives across releases.
- **PGP path alongside cosign (SIGN-03), not replacing it** — cosign is the primary path (consistent with image signing); PGP is the redundant non-OIDC alternative for operators who can't reach Sigstore Fulcio/Rekor at verification time.

## Performance Metrics

v1.5 per-phase metrics live in `.planning/milestones/v1.5-ROADMAP.md`. Cumulative cross-milestone trends live in `.planning/RETROSPECTIVE.md`. Will repopulate as v1.6 phases ship.

## Operator Next Steps

- v1.6 roadmap approved with 4 phases (22-25) covering 14 requirements (14/14 coverage, no orphans).
- **Next:** `/gsd:discuss-phase 22` (Base-Image Digest Drift Detection — lowest risk, builds digest discipline before signing layers on top).
- After Phase 22 ships: Phase 23 (cosign image attestations + SLSA + SBOM — the load-bearing piece).
