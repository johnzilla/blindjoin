# Phase 23: cosign Image Attestations + SLSA Provenance + SBOM - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-01
**Phase:** 23-cosign-image-attestations-slsa-provenance-sbom
**Areas discussed:** Sign placement, SBOM action, Pin gate, SECURITY.md scope, SECURITY.md skeleton, HUMAN-UAT shape

---

## Sign placement

| Option | Description | Selected |
|--------|-------------|----------|
| Inline in matrix leg | Add sign+attest steps after build-push-action in the same matrix leg (per-image). Reuses outputs.digest in-context, no job-output plumbing. A sign failure on coordinator does NOT block client/liquidity-bot (matrix fail-fast: false already set). | ✓ |
| Separate attest job, fan-out matrix | New `attest` job with the same 3-leg matrix, needs: docker. Image digest passes via job outputs. Cleaner failure isolation (sign retryable without re-pushing image) but doubles checkout+cosign-install cost. | |
| Separate attest job, single leg with loop | Single `attest` job, needs: docker, that loops over the 3 images. Cheapest CI minutes but one sign failure blocks all 3. | |

**User's choice:** Inline in matrix leg
**Notes:** Matches Phase 22's simplicity preference; cosign re-sign is idempotent at the registry, so retry cost is low. Captured as D-01 in CONTEXT.md.

---

## SBOM action

| Option | Description | Selected |
|--------|-------------|----------|
| actions/attest-sbom (GitHub-maintained) | Mirrors the already-chosen `actions/attest-build-provenance` shape. GitHub-maintained, SHA-pin discipline applies identically, attestation auto-pushed to registry alongside the image. Syft is invoked internally; SPDX is the supported format. | ✓ |
| anchore/sbom-action then cosign attest | Syft maintainer's official GHA wrapper. Produces SPDX file artifact, then `cosign attest --predicate <file> --type spdx` attaches it. More moving parts but you control the Syft invocation directly (scope, exclusions). | |
| Manual: install syft + cosign attest | Most flexible, most YAML. Only worth it if (1) or (2) hit a sharp edge. Defer to planner. | |

**User's choice:** actions/attest-sbom (GitHub-maintained)
**Notes:** Consistency with the already-locked `actions/attest-build-provenance` choice (Pitfall 5). Default Syft scope (full image fs) is correct for REQUIREMENTS ATTEST-03 "operator can grep for a CVE-identified package without pulling the image". Captured as D-03.

---

## Pin gate

| Option | Description | Selected |
|--------|-------------|----------|
| Narrow sigstore-pin grep gate | Mirrors `bip322-pin-check`: a CI script that fails if `sigstore/cosign-installer` or `actions/attest-build-provenance` or `actions/attest-sbom` aren't on their pinned SHA. Cheap to add, audit-grepable. Doesn't touch existing actions. | ✓ |
| Broad GHA pin gate (all actions @<40-hex>) | Pitfall 4's stronger suggestion: a regex check that EVERY `uses:` line in `.github/` is `@<40-hex>`. Catches future regressions on any action. May surface pre-existing floating-tag uses (e.g., `dtolnay/rust-toolchain@stable`) — requires triage. | |
| Hand-pin only, defer the gate | Phase 22 took this stance. Manual SHA-pin in YAML + trust the existing review discipline. Pitfall 4 grep gate carried to a v1.7 quick task. | |

**User's choice:** Narrow sigstore-pin grep gate
**Notes:** Scoped to the new attack surface Phase 23 introduces. Stable 3-action target list (same across Phase 24, zero new in Phase 25 — no gate maintenance). Broad-gate triage of pre-existing floating tags deferred as v1.7 carry-forward. Captured as D-04.

---

## SECURITY.md scope

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal in-place edit | Replace the existing "Docker images on ghcr.io are unsigned" line with the cosign verify recipe + inline Pitfall 10 (GHCR UI) + Pitfall 13 (cosign version) callouts. Keeps the prose-paragraph shape Phase 22 already extended. Phase 24 adds its tarball recipe as another minor edit. | |
| New ### Verification subsection | Add `### Image verification` under `## Supply-chain status` with the recipe + callouts. Phase 24 adds `### Tarball verification` parallel to it. Cleaner anchor-link targets for external docs (e.g., the AUDIT-CHARTER cross-ref). | |
| Full ## Supply-chain status rewrite | Restructure the whole section for v1.6 coherence: status table + verification recipes side-by-side. Larger diff this phase, but Phase 24's diff becomes additive-only and Phase 25's reproducibility recipe slots in cleanly. | ✓ |

**User's choice:** Full ## Supply-chain status rewrite
**Notes:** Captured as D-05. Trades a larger Phase 23 diff for additive-only Phase 24/25 diffs. Existing `### Base-image digests (v1.6 onward)` subsection (Phase 22 P0-1) stays as a subsection under the rewritten overview.

---

## SECURITY.md skeleton

| Option | Description | Selected |
|--------|-------------|----------|
| Status table + per-artifact subsections | Open with a small `\| Artifact \| Signing \| Provenance \| SBOM \|` table summarizing coverage, then `### Image verification` (this phase) followed by `### Tarball verification` placeholder (Phase 24) and `### Reproducible builds` placeholder (Phase 25). Anchor-link-friendly. Forward-references future phases by name so the doc is self-evidently in-progress. | |
| Prose intro + recipes block + callouts block | One short prose paragraph stating what's signed and how to verify, followed by a fenced `bash` block with the cosign verify recipe, followed by a `> Note` block with the GHCR-UI + cosign-version callouts. Phase 24 appends another recipe block; Phase 25 appends a reproducibility section. Less anchor structure but reads more naturally top-to-bottom. | ✓ |
| AUDIT-CHARTER-style numbered sections | Mirror the AUDIT-CHARTER.md `§1 / §2 / §3` shape: `§1 Status table`, `§2 Image verification`, `§3 Verifier prerequisites`. Heaviest structure, matches the project's audit-grade prose style, costs the most diff. | |

**User's choice:** Prose intro + recipes block + callouts block
**Notes:** Recipe-first reads more naturally for the operator audience than per-artifact anchors or numbered sections. Anchor-stability concerns (e.g., external links to a `#image-verification` target) listed as a deferred-idea carry-forward in CONTEXT.md.

---

## HUMAN-UAT shape

| Option | Description | Selected |
|--------|-------------|----------|
| Pre-merge: dispatch verifies pipeline runs; post-tag: fresh-machine verify on v1.6.0-rc.0 | Pre-merge workflow_dispatch run confirms cosign-installer + sign + attest-build-provenance + attest-sbom steps all succeed against Fulcio/Rekor (the pipeline works). After merge, cut a `v1.6.0-rc.0` pre-release tag, run the full operator verify recipe from a clean Docker container (fresh machine spirit per Pitfall 12), then re-tag `v1.6.0` once green. Two-stage rehearsal, defensible against Pitfall 12. | ✓ |
| Pre-merge dispatch only; verify end-to-end on the actual v1.6.0 tag | Pre-merge workflow_dispatch verifies the workflow YAML compiles + cosign install/sign step succeeds. End-to-end `cosign verify` from fresh machine happens on the real `v1.6.0` tag. Risk: if the regex or recipe is wrong, the doc ships broken and a quick task fixes it post-release. Lighter rehearsal, accepts a small doc-rot risk. | |
| Two-pass: rehearsal recipe + production recipe in the doc | SECURITY.md documents BOTH a rehearsal regex (`refs/(heads\|tags)/.*`) for pre-tag verification AND the production regex (`refs/tags/v.*`). HUMAN-UAT runs the rehearsal recipe pre-merge from a fresh machine, proving the end-to-end pipeline + verify path before the first tag. Costs doc clarity but eliminates the chicken-and-egg. | |

**User's choice:** Pre-merge: dispatch verifies pipeline runs; post-tag: fresh-machine verify on v1.6.0-rc.0
**Notes:** Captured as D-06. The `--certificate-identity-regexp 'refs/tags/v.*'` deliberately spans pre-release tags (same property that lets v1.6.1, v1.7.0 all verify with one regex per Pitfall 1). RC tag rehearsal turns the chicken-and-egg into a defensible two-stage Pitfall 12 procedure without polluting the operator-facing doc with a "rehearsal regex" that has to be maintained.

---

## Continuation check

| Option | Description | Selected |
|--------|-------------|----------|
| Ready for CONTEXT.md | Write CONTEXT.md now. Planner handles: (a) operator-side cosign version pin specificity, (b) `.bundle` distribution doc wording. Both have clear defaults in cosign docs + PITFALLS Pitfall 13. | ✓ |
| Discuss the cosign version pin | Specifically nail down whether SECURITY.md names `cosign 2.5.x`, `>= 2.5, < 3.0`, or `>= 2.5`. | |
| Discuss `.bundle` mechanics | Specifically settle whether ATTEST-04's `.bundle` deliverable is just the registry-attached cosign default or a separately-uploaded artifact. | |

**User's choice:** Ready for CONTEXT.md
**Notes:** Both deferred items captured as Claude's-Discretion D-07 (`.bundle`) and D-08 (cosign version pin) in CONTEXT.md with recommended defaults the planner can deviate from with justification.

---

## Claude's Discretion

Captured in CONTEXT.md `<decisions>` §Claude's Discretion as D-07 through D-11:

- **D-07:** `.bundle` distribution mechanism — registry-attached only + document the `cosign download signature --bundle` recipe in SECURITY.md. Planner confirms via cosign 2.5.x docs.
- **D-08:** Operator-side cosign version pin shape in SECURITY.md — recommended `>= 2.5, < 3.0` range form. Planner can deviate with justification.
- **D-09:** `sigstore-pin-check` location — recommended new job in `ci.yml` for symmetry with existing pin-check family.
- **D-10:** `actions/attest-sbom` + `actions/attest-build-provenance` + `sigstore/cosign-installer` SHA pins — resolved at planning time (avoids stale SHAs in this doc).
- **D-11:** `sigstore/cosign-installer` `cosign-release:` input version — pin to highest `v2.X.Y` stable at planning time, matching D-08's operator-side range.

## Deferred Ideas

Captured in CONTEXT.md `<deferred>`. Headline carry-forward candidates:

- Broad `every uses: must be @<40-hex>` CI grep gate (v1.7 quick task)
- Composite-action wrapper for sign+attest (only if a third caller surfaces)
- `workflow_dispatch.inputs.dry_run` bypass for tag-gate (planner discretion in Phase 23)
- `<a id="image-verification"></a>` anchor in SECURITY.md (if external docs need a stable target)
- Migrating off `@v3` floating tags on pre-Phase-23 actions (v1.7 quick task)
- Cosign 3.0 migration doc (v1.7+ depending on cosign release calendar)
- Severity tagging for hypothetical `[image-attestation-broken]` issues (only if CI cosign-verify job is later added)
