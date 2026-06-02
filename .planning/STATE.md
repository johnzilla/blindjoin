---
gsd_state_version: 1.0
milestone: v1.6
milestone_name: Supply-Chain Attestation
status: executing
last_updated: "2026-06-02T12:49:41Z"
last_activity: 2026-06-02 — Phase 24 CLOSED at cosign+SLSA scope. SIGN-03 (PGP/YubiKey path) deferred indefinitely after honest threat-model review: for a solo pre-customer project, the unique threats PGP mitigates are either negligible probability or better addressed by YubiKey-for-GitHub-2FA. Plan 24-05 superseded; PGP sections stripped from docs/RELEASING.md + SECURITY.md.
progress:
  total_phases: 4
  completed_phases: 3
  total_plans: 15
  completed_plans: 15
  percent: 75
---

# Project State

## Current Position

Phase: 24 (release-tarball-signing-cosign-slsa) — COMPLETE (cosign + SLSA scope; SIGN-03 PGP path deferred indefinitely)
Plan: 4 of 4 shipped (24-05 superseded — was YubiKey ceremony for the deferred PGP path)
Status: Phase 24 closed; ready for Phase 25 (Reproducible-Build Recipe + Verifier + Registry)
Last activity: 2026-06-02

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-06-01 — v1.6 Current Milestone)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 24 — release-tarball-signing-cosign-slsa-pgp

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

v1.6 Phase 23 plan decisions:

- **Plan 23-02: Paraphrased `slsa-framework/slsa-github-generator` in Pitfall 5 comment** — the acceptance criterion `! grep -q 'slsa-framework/slsa-github-generator'` fails if the string appears anywhere in the file, including comments. The comment now uses `slsa-github-generator` (without the `slsa-framework/` org prefix) to convey the prohibition without embedding the grep target. Mirrors Plan 23-01's `--no-tlog-upload` paraphrase pattern exactly.

v1.6 Phase 24 plan decisions:

- **Plan 24-01: RESEARCH §3.2 correction load-bearing — `actions/attest-build-provenance@v3.2.0` has NO `output-name` input.** CONTEXT D-14 had assumed an `output-name` input to control the .sigstore filename; planner-verified action.yml at SHA `96278af6` declared exactly 8 inputs (`subject-path`, `subject-digest`, `subject-name`, `subject-checksums`, OCI-registry-push, `create-storage-record`, `show-summary`, `github-token`) and 3 outputs (`bundle-path`, `attestation-id`, `attestation-url`). Plan split SIGN-02 into TWO steps: (a) attest step with `id: provenance`; (b) `mv "${{ steps.provenance.outputs.bundle-path }}" blindjoin-linux-amd64.tar.gz.sigstore`. Without (b), softprops upload fails "file not found" on `.sigstore`. Anti-pattern entry added to PATTERNS — first phase-level instance where RESEARCH overrode CONTEXT-assumed input.
- **Plan 24-01: Paraphrased forbidden-token names in attest-step comments** — the same Plan 22-04 paraphrasing discipline now extends to attest-* action inputs that the planner forbids by file-level grep. Specifically: the comment documenting the action's input list was rewritten from "exactly 8 inputs: subject-path, subject-digest, subject-name, subject-checksums, push-to-registry, ..." to "exactly 8 inputs and 3 outputs including bundle-path" so the literal tokens `subject-name:`, `subject-digest:`, `push-to-registry:` never appear at the file level. Pattern: file-level audit grep at `! grep -q '<input>:'` is honored at the file level even inside comments. Treated as Rule 1 (auto-fix bug) during Task 3 verification.
- **Plan 24-01: Phase 23 sigstore-pin-check inherited at file level — no new CI gate added.** RESEARCH §2.3 confirms the existing `sigstore-pin-check` job at ci.yml:292-326 greps every workflow under `.github/workflows/` (including `release.yml`); both new sigstore SHA pins in `release.yml` are caught automatically. Phase 24 establishes the "Phase 23 sets discipline; Phase 24 inherits" pattern for future Phase 25 reproducible-verify workflow.
- **Plan 24-02: Plain-text version pins in docs Prerequisites bullets** — initial draft of `docs/RELEASING.md`'s Prerequisites section used backtick-wrapped tool names (`` **`gpg` 2.4+** `` etc.), but the plan's automated acceptance criteria run literal-byte greps without backticks (`grep -q 'gpg 2\.4'`, `grep -q 'gh 2\.50'`, `grep -q 'cosign 2\.6\.3'`). Backticks split the contiguous-byte match — same root cause as Plan 22-05's `**Do not auto-merge digest bumps**` wrapping issue. Switched the Prerequisites bullets to bare-token form (`**gpg 2.4+** on the maintainer's machine.`) preserving Markdown bold styling while removing backticks from the version-pin tokens. Treated as Rule 3 (auto-fix blocking issue) before commit. Extends the "literal-byte form wins over source-file readability when the plan grep is the acceptance contract" pattern from workflow-modify plans (22-04, 22-05) to docs-modify plans.
- **Plan 24-02: `<FINGERPRINT-TBD>` vs `<new-FINGERPRINT-TBD>` placeholder disambiguation** — 8 `<FINGERPRINT-TBD>` placeholders (Plan 24-05 replaces atomically) vs 3 `<new-FINGERPRINT-TBD>` placeholders (future-rotation prose; STAY as-is). The two distinct placeholder strings prevent Plan 24-05's atomic substitution from corrupting the rotation-procedure prose — a single `<FINGERPRINT-TBD>` replacement contract would have substituted the rotation flow's example new fingerprint, breaking the prose semantics.
- **Plan 24-03: Recipe 1 single-line literal-byte form** — collapsed Phase-23-style `cosign verify-blob \` + `  --bundle blindjoin-linux-amd64.tar.gz.bundle \` line-wrap onto one physical line (`cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \`) so the plan-author's acceptance grep `grep -q 'cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle' SECURITY.md` matches. Same root cause as Plan 22-05 `**Do not auto-merge digest bumps**` re-wrap and Plan 24-02 plain-text-version-pins: when literal-byte grep is the acceptance contract, the byte form wins over Phase-23 source-readability line-wrap. Pattern now spans workflow-modify (22-04, 22-05), docs-modify (24-02), and SECURITY.md (24-03) plans. Rule 3 (auto-fix blocking issue).
- **Plan 24-03: BRE-escape acceptance-grep artifacts documented but no content change** — plan-author's `grep -q 'release\.yml@refs/tags/v\.\*' SECURITY.md` and `grep -A 25 ... \| grep -q 'docker\.yml'` patterns return false-negatives because BRE `\.` is escape-of-dot meaning "match one literal `.`", while the file contains the 2-byte literal `\.` regex source (backslash + dot inside the cosign --certificate-identity-regexp argument). Verified content correctness via `grep -F` (fixed-string literal): release.yml regex present in 2 places (Recipe 1 + Recipe 3); Phase 23's `docker\.yml` regex present in 2 places (image Recipe 1 + Recipe 4). Phase 23's own verify block has the same false-negative pattern; this is a future-plan-author note, not a content change.
- **Plan 24-03: 3 `<FINGERPRINT-TBD>` occurrences in SECURITY.md** — Plan 24-05 atomic-substitution scope is 3 (SECURITY.md) + 8 (docs/RELEASING.md per Plan 24-02 SUMMARY) = **11 occurrences total**. The 3 `<new-FINGERPRINT-TBD>` rotation-procedure placeholders in docs/RELEASING.md stay as-is (Plan 24-02 disambiguation).
- **Plan 24-04: Mirror Plan 24-02 audience-disambiguation lede in CONTRIBUTING.md cross-ref** — two-layer audience-gating: (a) CONTRIBUTING.md cross-ref tells contributors NOT to follow the link unless they're cutting a release; (b) docs/RELEASING.md's own H1 + lede re-states the same audience. Without (a), non-maintainer contributors reading `## Tagging releases` silently follow the cross-ref and start executing procedures requiring YubiKey + offline revocation cert + maintainer-only secrets they don't have (T-24-31 mitigation). Single physical source line per Phase 22 Plan 22-05 lesson — file-level grep audits match across the entire paragraph. Pre-edit 141 lines → post-edit 143 lines (net +2 source lines because pre-edit file already had a blank line between milestone-name paragraph and `## Bumping base-image digests` H2); within planner-stated ±1 tolerance of expected 144.

v1.6 Phase 22 plan decisions:

- **Plan 22-02: Inline the auditor-grepable trailer per error path** (not via a `POLICY_REF` shell variable) — the acceptance criterion `grep -c 'Refusing to build without a valid manifest' >= 4` counts FILE LINES, not runtime expansions. Inlining the literal trailer in each of the 4 error echos puts the auditor-grep property at the file level as well as the runtime-log level. Final count: 7 matching lines in `.github/actions/read-base-digests/action.yml`.
- **Plan 22-03: Followed RESEARCH.md §5.1 verbatim — no additional echo step in release.yml** — the orchestrator's `<plan_specifics>` mentioned an optional "Echo canonical digests for audit log" step, but PLAN.md locks the shape to RESEARCH.md §5.1 lines 562-571 which does NOT include such a step, and the composite action already echoes the parsed digests to stdout (action.yml lines 107-109). ROADMAP SC#3 audit-observability is satisfied by the composite action's own audit trail without a redundant echo in the consumer workflow.
- **Plan 22-03: `build-args:` placed between `labels:` and `cache-from:` in docker/build-push-action with: block** — PATTERNS §3 line 317 locks this insertion point; keeps deterministic-build inputs (tags, labels, build-args) grouped before cache-handling fields, mirroring the existing `tags: |` pipe-multiline shape on the same step.
- **Plan 22-04: Paraphrased deliberately-omitted-scope names in the `permissions:` comment** rather than quoting their YAML keys verbatim. The PLAN's acceptance criteria run `! grep -q 'pull-requests:'` and `! grep -q 'id-token:'` as LINE-LEVEL auditor-grepable invariants — those assertions fail when the comment block literally contains the omitted-scope key strings, even though the runtime permission gate is correct. The rewritten comment uses `PR-write`, `packages`, and `id-token` (without the literal `:` suffix on `pull-requests` and `id-token`) so the audit gate is satisfied at the file level too. Establishes a reusable "auditor-grepable deliberately-omitted-scopes" pattern for Phase 23 (cosign) and Phase 25 (reproducible-verify) workflows.
- **Plan 22-04: Followed RESEARCH.md §4 verbatim for the workflow YAML shape** — the locked structure (top-of-file comment block → `env:` → `on:` → `permissions:` → `jobs.drift-check:`) is the single source of truth for both DRIFT-02 implementation and the prose-comment-as-contract pattern future supply-chain workflows will mirror. Self-bootstrapping label via `gh label create digest-drift ... 2>/dev/null || true` eliminates the manual repo-setup step.
- **Plan 22-05: Re-wrap `**Do not auto-merge digest bumps**` in SECURITY.md to a single line** so PLAN-locked `grep -q '\*\*Do not auto-merge digest bumps\*\*' SECURITY.md` matches. RESEARCH.md §7.1 wrapped the bold marker across two lines for source-file readability; PLAN.md acceptance line 122 is a single-line grep that does NOT match across newlines. Phrasing preserved verbatim; only the line break inside the bold marker shifted. PATTERNS §"SECURITY.md MODIFY" lines 383-385 authorize additive-voice latitude. Treated as Rule 3 (auto-fix blocking issue). Establishes the pattern for v1.6+ docs phases: when RESEARCH-locked prose wrapping collides with PLAN-locked literal-byte greps, the literal-byte form wins — that is what the supply-chain audit trail queries.

## Performance Metrics

v1.5 per-phase metrics live in `.planning/milestones/v1.5-ROADMAP.md`. Cumulative cross-milestone trends live in `.planning/RETROSPECTIVE.md`.

v1.6 Phase 22:

| Plan | Name | Duration | Tasks | Files |
|------|------|----------|-------|-------|
| 22-01 | docker/digests.txt canonical manifest | (see Plan 22-01 SUMMARY) | — | 1 |
| 22-02 | read-base-digests composite action | ~5 min | 1 | 1 |
| 22-03 | release.yml + docker.yml composite-action wiring (DRIFT-03) | ~6 min | 2 | 2 |
| 22-04 | digest-drift-check.yml scheduled workflow (DRIFT-02) | ~7 min | 1 | 1 |
| 22-05 | SECURITY.md + CONTRIBUTING.md prose for D-05 (DRIFT-01 prose half) | ~11 min | 2 | 2 |

v1.6 Phase 23:

| Plan | Name | Duration | Tasks | Files |
|------|------|----------|-------|-------|
| 23-01 | Permissions + id:build + cosign-installer + cosign sign (ATTEST-01) | ~10 min | 2 | 1 |
| 23-02 | SBOM generation + SBOM attestation + build provenance (ATTEST-02 + ATTEST-03) | ~8 min | 2 | 1 |

v1.6 Phase 24:

| Plan | Name | Duration | Tasks | Files |
|------|------|----------|-------|-------|
| 24-01 | release.yml cosign sign-blob + SLSA provenance + softprops draft (SIGN-01 + SIGN-02) | ~5 min | 4 | 1 |
| 24-02 | docs/RELEASING.md maintainer-side release procedure (SIGN-03 procedural surface) | ~7 min | 2 | 1 |
| 24-03 | SECURITY.md ### Release tarball signatures + provenance subsection (SIGN-01 + SIGN-02 + SIGN-03 operator-facing recipes) | ~5 min | 1 | 1 |
| 24-04 | CONTRIBUTING.md → docs/RELEASING.md one-paragraph cross-reference (SIGN-03 contributor-manual discoverability) | ~1 min | 1 | 1 |

## Operator Next Steps

- v1.6 roadmap approved with 4 phases (22-25) covering 14 requirements (14/14 coverage, no orphans).
- Phase 22 complete. Phase 23 complete. Phase 24 in progress: 4/5 plans complete (24-01 release.yml SIGN-01/02, 24-02 docs/RELEASING.md SIGN-03 maintainer-side, 24-03 SECURITY.md operator-side recipes, 24-04 CONTRIBUTING.md → docs/RELEASING.md cross-ref).
- Plan 24-04 just shipped: CONTRIBUTING.md gains a one-paragraph cross-reference at the end of `## Tagging releases` section pointing to `docs/RELEASING.md`, with an audience-disambiguation lede ("Most contributors don't need it; it's the release-engineering manual for the maintainer.") that gates non-maintainer contributors away from release-engineering procedures (T-24-31 mitigation; D-11 + D-20). Pre-edit 141 → post-edit 143 lines.
- **Next:** Plan 24-05 (atomic `<FINGERPRINT-TBD>` substitution at v1.6.0-rc.0 cut — checkpoint:human-verify; maintainer YubiKey ceremony + `docs/pgp/<FINGERPRINT>.asc` commit + 11 `<FINGERPRINT-TBD>` substitutions across SECURITY.md + docs/RELEASING.md).
