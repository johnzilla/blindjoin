---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: Supply-Chain Attestation
status: planning
last_updated: "2026-06-01T03:30:01.811Z"
last_activity: 2026-06-01
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-06-01 — Milestone v1.6 started

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-06-01 — v1.6 Current Milestone)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** v1.6 Supply-Chain Attestation — closing the unsigned-build gap (cosign on ghcr images, detached signatures on release tarballs, reproducible-build recipe, automated digest drift check). Defining requirements.

## Milestone Map

v1.5 shipped 2026-06-01. Phase artifacts archived to `.planning/milestones/v1.5-phases/`. Full per-phase details in `.planning/milestones/v1.5-ROADMAP.md` and `.planning/milestones/v1.5-REQUIREMENTS.md`. Per-milestone summary in `.planning/MILESTONES.md`.

v1.6 in scoping. Requirements + roadmap being defined; phase artifacts will land under `.planning/phases/` as they're planned.

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

### Load-bearing invariants (shipped, must not regress)

- **V1.4-CRIT-01** — `shared::bip322` dispatcher-only public surface (9 symbols exactly). After v1.5 Phase 19 closed the test-only `sign_simple_test_only` escape hatch, this is load-bearing at the type level with no holes.
- **CRIT-01 cross-check** in `coordinator::validate_utxo` — derives `ScriptType` from on-chain `script_pubkey` (NEVER from client-declared field). Preserved into the fee path by v1.5 Phase 20 (`ParticipantInput.script_type` plumbed through `dispatch_ownership_proof → UtxoDetails → RegisteredInput`).
- **CD-7 two-phase try-parse** on `OwnershipProof` — v1.3↔v1.4 wire compat preserved byte-exactly.
- **`bip322 = "=0.0.10"` exact pin** + `bip322-pin-check` CI gate.
- **AUDIT-03 structurally-bounded RSA SecretKey lifetime** — `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` with sole FSM chokepoint at `state.rs:202` (`transition_to(Phase::Idle)`). Verified by structural FSM test + grep gate. Charter §5 in `docs/AUDIT-CHARTER.md` is the auditor-facing description.

Full per-milestone invariant detail in `.planning/milestones/v1.5-ROADMAP.md` and earlier milestone archives.

## Recent Plan Decisions

v1.5 plan decisions archived to `.planning/milestones/v1.5-phases/{19,20,21}-*/`. Cumulative trends live in `.planning/RETROSPECTIVE.md`. Will repopulate when v1.6 work starts.

## Performance Metrics

v1.5 per-phase metrics live in `.planning/milestones/v1.5-ROADMAP.md`. Cumulative cross-milestone trends live in `.planning/RETROSPECTIVE.md`. Will repopulate when v1.6 work starts.

## Operator Next Steps

- v1.6 requirements + roadmap being defined via this `/gsd:new-milestone` cycle.
- After roadmap approval: `/gsd:discuss-phase 22` (first v1.6 phase, continued numbering from v1.5 Phase 21).
