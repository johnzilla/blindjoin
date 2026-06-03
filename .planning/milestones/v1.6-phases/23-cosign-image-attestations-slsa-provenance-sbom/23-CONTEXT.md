# Phase 23: cosign Image Attestations + SLSA Provenance + SBOM - Context

**Gathered:** 2026-06-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 23 turns every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z` image push into a cryptographically verifiable artifact: a cosign keyless OIDC signature, a SLSA v1.0 in-toto provenance attestation (via `actions/attest-build-provenance`), an SPDX SBOM attestation (via `actions/attest-sbom`, Syft under the hood), and a downloadable cosign `.bundle` (sig + cert + Rekor inclusion proof) usable for offline verification once the Sigstore TUF root is cached locally. On the operator side, `SECURITY.md` `## Supply-chain status` is rewritten to remove the v1.5 "Docker images on ghcr.io are unsigned" gap and replace it with the canonical `cosign verify` recipe + Pitfall 10 (GHCR UI badge) + Pitfall 13 (cosign 3.0 CLI drift) callouts.

What this phase does NOT do: tarball signing (Phase 24 — reuses the cosign-installer SHA pin + `--certificate-identity-regexp` shape established here), reproducible-build recipe + verifier (Phase 25), any change to `release.yml` (Phase 24's seat). It also does NOT touch the Phase 22 base-image digest manifest or the `read-base-digests` composite action — those continue to feed `--build-arg DEBIAN_REF/CARGO_CHEF_REF` exactly as they do today; the new sign/attest steps run AFTER `docker/build-push-action` consumes them.

</domain>

<decisions>
## Implementation Decisions

### Sign + attest workflow placement

- **D-01: Inline in the existing `docker` matrix leg.** All three new steps (`cosign sign`, `actions/attest-build-provenance`, `actions/attest-sbom`) land in `docker.yml`'s `docker` job AFTER the `docker/build-push-action` step, inside the same matrix leg. The build-push action's `outputs.digest` is consumed in-context with no job-output plumbing. With `strategy.fail-fast: false` already set ([docker.yml:65](.github/workflows/docker.yml#L65)), a sign failure on one image does NOT block the other two. Trade-off accepted: a sign-step failure means re-pushing the image on retry (cosign is idempotent at the registry — re-signing the same digest is safe). Rationale: minimal YAML diff, no new job, mirrors the simplicity preference established when Phase 22 put `read-base-digests` inline rather than fanning out.

- **D-02: `id-token: write` added at JOB level on the `docker` job, NOT workflow-level.** Per PITFALLS Pitfall 2: narrower scope is strictly better. The workflow-level `permissions:` block stays at `contents: read`. The `docker` job's existing `permissions: { contents: read, packages: write }` block grows to `{ contents: read, packages: write, id-token: write }`. The new permission gets a dedicated comment line above the `permissions:` block explaining "id-token: cosign OIDC keyless signing — PITFALLS Pitfall 2" — comments-as-contract pattern from Phase 22's [22-CONTEXT.md `<code_context>`](.planning/phases/22-base-image-digest-drift-detection/22-CONTEXT.md). The `check` job's existing `permissions: contents: read` (implicit from workflow default) stays untouched. The auditor-grepable "deliberately-omitted-scopes" pattern established by Phase 22 Plan 22-04 applies: the comment block names `pull-requests` and `pages` as deliberately omitted, written as `PR-write` / `pages` (without the literal `:` suffix) so any future `! grep -q 'pull-requests:'` audit gate is satisfied at the file level too.

### SBOM emission path

- **D-03: `actions/attest-sbom` (GitHub-maintained, SHA-pinned at adoption).** Mirrors the already-chosen `actions/attest-build-provenance` shape (Pitfall 5 locked at roadmap level). Both actions follow the same input pattern (`subject-name`, `subject-digest`), both push attestations to the same registry alongside the image, both use the same OIDC subject claim. The action invokes Syft internally and emits SPDX format (REQUIREMENTS ATTEST-03 specifies SPDX-via-Syft; `actions/attest-sbom` defaults to Syft + SPDX, no `format` override needed). Rejected alternative: `anchore/sbom-action` + manual `cosign attest --predicate <file> --type spdx` would give per-invocation Syft control (scope, exclusions) at the cost of three more steps and a second action to SHA-pin. The Syft default scope (full image filesystem) is the right scope for ATTEST-03's "operator can grep for a CVE-identified package without pulling the image" acceptance test.

### SHA-pin enforcement

- **D-04: Narrow sigstore-pin grep gate (mirrors `bip322-pin-check`).** New CI script `.github/scripts/sigstore-pin-check.sh` (or inline in `ci.yml`) fails if any of `sigstore/cosign-installer` / `actions/attest-build-provenance` / `actions/attest-sbom` lack a `@<40-hex>` SHA pin in any workflow under `.github/workflows/`. Pattern mirrors `bip322-pin-check` (v1.4) and the v1.5 `crit-01-grep-check` family — narrow, audit-grepable, named after what it enforces. Rejected alternative: a broad `every uses: must be @<40-hex>` gate would surface pre-existing pre-pinned uses (e.g., `dtolnay/rust-toolchain@stable` at [docker.yml:37](.github/workflows/docker.yml#L37) and [docker.yml:38](.github/workflows/docker.yml#L38)) and require simultaneous triage — out of scope for Phase 23's signing focus. The broad gate is a v1.7 carry-forward candidate. Rejected alternative: hand-pin + defer the gate (Phase 22's stance) would leave the FIRST sigstore action additions ungoverned at exactly the moment governance matters most. The sigstore-pin gate is the natural fit because (a) it scopes to the new attack surface this phase introduces, (b) its grep target list is a stable, named set of 3 actions, (c) Phase 24 reuses the same action set (no gate maintenance) and Phase 25 adds zero new sigstore actions.

### SECURITY.md restructure

- **D-05: Full `## Supply-chain status` rewrite using a "prose intro + recipes block + callouts block" skeleton.** The v1.5 section reads as four loose status paragraphs (Docker images unsigned, release tarballs sha256-only, no reproducible build, base-image pins). Phase 23 rewrites the whole section into a v1.6-coherent shape that Phase 24 and Phase 25 can append to additively:
  1. **Prose intro** — one short paragraph naming what's signed (images NOW, tarballs in Phase 24), what attestations exist (provenance, SBOM), and how to verify (one-liner: "run the `cosign verify` recipes below from a clean machine").
  2. **Fenced `bash` recipes block** — the operator-facing `cosign verify` command, with the locked `--certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*'` + `--certificate-oidc-issuer 'https://token.actions.githubusercontent.com'`. One generic command (operator substitutes `<image>:<tag>` — coordinator/client/liquidity-bot all verify identically). Followed by `cosign download attestation --predicate-type https://slsa.dev/provenance/v1 ...` and `cosign download attestation --predicate-type https://spdx.dev/Document ...` for provenance + SBOM retrieval. The `.bundle` retrieval recipe (`cosign download signature --bundle blindjoin.bundle ...`) is in this block too (ATTEST-04 acceptance — "downloadable cosign `.bundle` asset").
  3. **`> Note` callouts block** — Pitfall 10 (GHCR UI "Unverified" badge is unrelated to cosign verification) and Pitfall 13 (tested with cosign 2.5.x; cosign 3.0 may change CLI flags — pin the operator-side version range). Operator-side cosign version specificity is planner discretion (D-08), bounded to one of three pin shapes named in `<specifics>`.
  - Phase 24 appends another fenced `bash` recipe block for `cosign verify-blob` + the PGP `gpg --verify` recipe. Phase 25 appends a reproducibility section. The Phase 22 v1.5 P0-1 paragraph (`### Base-image digests (v1.6 onward)`) stays untouched as a subsection — the rewrite is of the OVERVIEW prose + recipes, not of the per-topic subsections Phase 22 already shipped.
  - Rejected: status table + per-artifact subsections (heavier anchor structure, more diff this phase, downstream phases pay the same restructure cost). Rejected: AUDIT-CHARTER-style numbered sections (matches audit-charter prose style but the supply-chain section is operator-facing — recipe-first reads more naturally than `§1 / §2 / §3`).

### HUMAN-UAT shape (two-stage rehearsal — Pitfall 12 defensible)

- **D-06: Pre-merge workflow_dispatch verifies the pipeline; post-tag fresh-machine verify runs against `v1.6.0-rc.0`.** The chicken-and-egg: `workflow_dispatch` from a branch produces an OIDC subject `ref:refs/heads/<branch>`, which does NOT match the locked `--certificate-identity-regexp 'refs/tags/v.*'`. So pre-merge rehearsal can prove the pipeline runs end-to-end (cosign-installer, sign, attest-build-provenance, attest-sbom all succeed against Fulcio/Rekor) but CANNOT prove the operator-facing verify recipe is correct.
  - **Stage 1 (pre-merge):** `gh workflow run docker.yml --ref <feature-branch>` from a feature branch. Confirms YAML compiles, `id-token: write` resolves, cosign-installer succeeds, sign + attest steps reach Fulcio. The `docker` matrix job is gated on `if: startsWith(github.ref, 'refs/tags/')` ([docker.yml:60](.github/workflows/docker.yml#L60)) — for Stage 1, this gate must be temporarily loosened OR the gate is preserved and Stage 1 only proves the `check` job + cosign-installer install step on a synthetic dispatch path. Planner decides whether to add a `workflow_dispatch.inputs.dry_run` bypass or to accept that Stage 1 only covers check-job rehearsal (with sign-step dry-run on Stage 2's RC tag).
  - **Stage 2 (post-merge, pre-1.6.0):** Cut a `v1.6.0-rc.0` pre-release tag. The OIDC subject is now `ref:refs/tags/v1.6.0-rc.0`, which matches the locked regex `refs/tags/v.*` (the regex deliberately spans pre-release tags). From a fresh `docker run --rm -it ubuntu:24.04` container with no project caches: install cosign 2.5.x → run the documented `cosign verify` recipe against the rc.0 images → confirm exit 0 + parseable JSON. Then run the `cosign download attestation` recipes for provenance + SBOM. Then run the `.bundle` recipe and `cosign verify-blob --bundle` it offline. If all green: re-tag `v1.6.0` and proceed. If any fails: fix the doc/recipe in a quick task before the production tag.
  - Quick-task scaffold for the rehearsal log: same shape as v1.5 `260531-thw-*` / `260531-ubf-*` (SUMMARY.md, PASS/FAIL per recipe, fresh-machine evidence). The `--certificate-identity-regexp` spans pre-release tags by design — this is the same property that lets v1.6.1, v1.7.0, etc. all verify against one regex (Pitfall 1 rationale).
  - Rejected: pre-merge dispatch only + verify on actual v1.6.0 (Pitfall 12 violation if the recipe is wrong — fixing post-release is doc rot in the wild). Rejected: two-pass rehearsal recipe in the doc (documenting a "rehearsal regex" that loosens to `refs/(heads|tags)/.*` invites operator confusion and creates a second cosign-verify path that has to be maintained — costs doc clarity for marginal rehearsal benefit).

### Claude's Discretion (planner figures these out, guided by research/PITFALLS.md)

- **D-07: `.bundle` distribution mechanism.** ATTEST-04 says "downloadable cosign `.bundle` asset per image". The cosign default puts the sig at `<image-digest>.sig` in the registry, and the `.bundle` format (sig + cert + Rekor inclusion proof) is reachable via `cosign download signature --bundle <output> <image>`. Recommended interpretation: registry-attached only + document the `cosign download signature --bundle` recipe in SECURITY.md (under the D-05 fenced bash block). Rationale: Phase 23 doesn't publish a GitHub Release (that's Phase 24's seat for tarballs); shipping a bundle artifact via a separate channel would invent a v1.6-specific distribution mechanism with no clear operator benefit. Planner: confirm by reading cosign 2.5.x docs and produce the exact `cosign download signature --bundle` invocation; if the cosign CLI shape diverges, reopen as a discussion item.

- **D-08: Operator-side cosign version pin shape in SECURITY.md.** Pitfall 13 calls for a documented pin. Three shapes:
  - `cosign 2.5.x` (tightest, most doc-rot — every 2.x minor needs a quick task).
  - `>= 2.5, < 3.0` (range — handles 2.x minors automatically, requires action only at cosign 3.0 ship). RECOMMENDED — matches Pitfall 13's spirit, lowest maintenance overhead.
  - `>= 2.5` (loosest — requires a Pitfall 13 quick task at the moment cosign 3.0 lands).
  Planner picks; if pick is anything other than the recommended range, justify in PLAN.md.

- **D-09: Drift-detection grep gate location.** The new `sigstore-pin-check` gate (D-04) can live as:
  - A new job in `ci.yml` (mirrors the existing `bip322-pin-check` / `crit-01-grep-check` job placement).
  - A new step in an existing `ci.yml` job (cheaper but less audit-grepable).
  - A new dedicated workflow `sigstore-pin-check.yml` (heaviest, most isolated).
  Recommended: new job in `ci.yml` for symmetry with the existing pin-check family. Planner picks; if dedicated workflow, justify (likely overkill for a 3-action grep target).

- **D-10: `actions/attest-sbom` and `actions/attest-build-provenance` SHA pins.** Both pinned at adoption to the latest stable SHA with a `# v<X.Y.Z>` trailing comment (existing project pattern at every `uses:` line in `docker.yml` / `release.yml` / `ci.yml`). Planner resolves the exact SHA at planning time (avoids encoding a stale SHA here). `sigstore/cosign-installer` pinned the same way; planner picks the installer version (likely the latest 3.X that installs cosign 2.5+).

- **D-11: Cosign installer's installed cosign version.** `sigstore/cosign-installer` accepts a `cosign-release: v2.X.Y` input. Pin to the same version range the operator-side documentation names (D-08). RECOMMENDED: pin to the highest `v2.X.Y` available at adoption (e.g., `v2.5.0` or whatever current stable is at planning time). Planner resolves.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase contract (locked WHAT)
- `.planning/REQUIREMENTS.md` §Category 1 — ATTEST-01, ATTEST-02, ATTEST-03, ATTEST-04 verbatim text. "OIDC keyless flow", "SLSA v1.0 build-level-3 in-toto provenance via `actions/attest-build-provenance`", "SBOM attestation SPDX format generated by Syft", "downloadable cosign `.bundle` asset" are non-negotiable.
- `.planning/ROADMAP.md` §Phase 23 — 5 numbered Success Criteria. Acceptance test for this phase. SC#1 names the exact identity-regexp pattern; SC#5 names the SECURITY.md rewrite explicitly.

### Threat-model + design context (this phase's pitfalls)
- `.planning/research/PITFALLS.md` §Pitfall 1 — `--certificate-identity-regexp` vs `--certificate-identity`. Identity regex locked to `https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*`. Don't go too wide.
- `.planning/research/PITFALLS.md` §Pitfall 2 — `id-token: write` at JOB level (D-02), not workflow-level. Opaque failure mode if missed (Fulcio 400).
- `.planning/research/PITFALLS.md` §Pitfall 3 — Rekor transparency log mandatory by default; don't set `--no-tlog-upload`.
- `.planning/research/PITFALLS.md` §Pitfall 4 — SHA-pin discipline on sigstore actions. Source for D-04's narrow grep gate.
- `.planning/research/PITFALLS.md` §Pitfall 5 — `actions/attest-build-provenance` (locked) over `slsa-framework/slsa-github-generator` (rejected). Source-of-truth for the action choice.
- `.planning/research/PITFALLS.md` §Pitfall 10 — GHCR UI "Unverified" badge confusion. Source for D-05's callouts block.
- `.planning/research/PITFALLS.md` §Pitfall 12 — Fresh-machine UAT every documented command. Source for D-06's Stage 2.
- `.planning/research/PITFALLS.md` §Pitfall 13 — cosign 3.0 CLI flag drift. Source for D-05's callouts block + D-08 version-pin shape.
- `.planning/research/SUMMARY.md` — phase mapping (digest discipline → image signing → tarball signing → reproducibility) + the operator-facing verify command at lines 67-72.
- `.planning/research/ARCHITECTURE.md` — ordering rationale (Phase 23 introduces `id-token: write` and the sigstore action SHA-pin discipline; Phase 24 reuses both verbatim).
- `.planning/research/STACK.md` — cosign version range, `sigstore/cosign-installer` version, `actions/attest-build-provenance` + `actions/attest-sbom` named as the stack additions.
- `.planning/research/FEATURES.md` — feature table-stakes for Category 1 (image attestations).

### Predecessor phase patterns (this phase MUST mirror these)
- `.planning/phases/22-base-image-digest-drift-detection/22-CONTEXT.md` — D-05 (CODEOWNERS-as-gate, prose+structural enforcement); D-03 (fail-fast inside composite action); D-04 (parse vs resolve split). The "auditor-grepable deliberately-omitted-scopes" pattern Phase 22 Plan 22-04 established applies to D-02's permission block.
- `.planning/phases/22-base-image-digest-drift-detection/22-PATTERNS.md` — composite-action shape, `build-args:` insertion point in `docker/build-push-action with:` block, comments-as-contract style.
- `.planning/phases/22-base-image-digest-drift-detection/22-RESEARCH.md` — the layered workflow structure (env → on → permissions → jobs) that the new `id-token: write` addition must respect.

### Existing pin discipline + integration surface
- `.github/workflows/docker.yml` — 3-image matrix `docker` job ([docker.yml:54](.github/workflows/docker.yml#L54)), `if: startsWith(github.ref, 'refs/tags/')` gate ([docker.yml:60](.github/workflows/docker.yml#L60)), `permissions: { contents: read, packages: write }` block ([docker.yml:61](.github/workflows/docker.yml#L61)), `docker/build-push-action@bcafcacb...` outputs.digest source ([docker.yml:110](.github/workflows/docker.yml#L110)), `workflow_dispatch` rehearsal harness ([docker.yml:26](.github/workflows/docker.yml#L26)). All sign/attest steps land AFTER the build-push step inside each matrix leg.
- `.github/workflows/ci.yml` — `bip322-pin-check` / `crit-01-grep-check` job placement is the model for D-04's `sigstore-pin-check` job.
- `.github/actions/install-bitcoind/action.yml` + `.github/actions/read-base-digests/action.yml` — composite-action precedent; Phase 23 does NOT add a new composite action (all sign/attest logic is third-party action calls, no project-local action needed).

### Policy + operator-facing docs (D-05 lands here)
- `SECURITY.md` `## Supply-chain status` — full rewrite target. The existing `### Base-image digests (v1.6 onward)` subsection (Phase 22 P0-1) STAYS untouched; the rewrite is of the OVERVIEW prose + per-artifact recipes.
- `docs/AUDIT-CHARTER.md` (v1.5 charter, 574 LOC) — supply-chain policy language style. Cross-reference target if the rewritten section needs to cite the charter for the broader threat model.
- `CONTRIBUTING.md` — no change expected for Phase 23 (Phase 22's CODEOWNERS bumping etiquette already shipped); add a single line cross-referencing the rewritten `SECURITY.md` section if natural.
- `.planning/quick/260531-thw-v1-5-release-readiness-p0s-security-md-c/260531-thw-SUMMARY.md` — the v1.5 P0-1 SECURITY.md "Supply-chain status" diff that this phase rewrites. Reading it shows the starting point.
- `.planning/quick/260531-ubf-post-release-readiness-polish-remove-amp/260531-ubf-SUMMARY.md` — the v1.5 release-smoke rehearsal pattern that Stage 1 of D-06 mirrors.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`docker.yml` matrix shape** ([docker.yml:64-73](.github/workflows/docker.yml#L64)) — `strategy.fail-fast: false` with 3-leg matrix. Sign+attest inline per D-01 means each leg is independent; a coordinator-image sign failure does not block client/liquidity-bot.
- **`docker/build-push-action` outputs.digest** — the `bcafcacb...` SHA pin at [docker.yml:110](.github/workflows/docker.yml#L110) emits `outputs.digest` (full `sha256:...` form). Cosign sign + both `actions/attest-*` consume `${{ steps.<build-id>.outputs.digest }}` directly. No job-output plumbing needed.
- **`workflow_dispatch` rehearsal harness** ([docker.yml:19-26](.github/workflows/docker.yml#L19)) — already wired with the rehearsal-path comment. D-06's Stage 1 reuses this harness; the planner decides whether to add a `workflow_dispatch.inputs.dry_run` bypass to the tag gate or to accept that Stage 1 only covers check-job + cosign-installer rehearsal.
- **SHA-pin trailing-comment style** — every `uses:` line in `docker.yml` is `@<40-hex> # vX.Y.Z`. New `sigstore/cosign-installer` + `actions/attest-build-provenance` + `actions/attest-sbom` lines MUST follow this. The D-04 grep gate enforces.
- **`bip322-pin-check` / `crit-01-grep-check` job pattern** — established in `ci.yml`. D-04's `sigstore-pin-check` is the next iteration of this pattern; reuse the same shell-script-in-CI shape.

### Established Patterns
- **Two-tier `check` + `<publish>` gate** — `docker.yml` and `release.yml` both have a `check` job (test+clippy+audit) feeding a tag-gated `<publish>` job via `needs: check`. Phase 23 does NOT add a new job; the sign/attest steps land in the existing `docker` job between `docker/build-push-action` and end-of-job. The `check` job is untouched.
- **`if: startsWith(github.ref, 'refs/tags/')`** — production-only gate at [docker.yml:60](.github/workflows/docker.yml#L60). All sign/attest output goes to ghcr.io only on real tag pushes. Pre-merge `workflow_dispatch` runs the `check` job and stops short. D-06 Stage 1 either (a) accepts this scope, or (b) adds a `dry_run` bypass that allows sign-step rehearsal against a synthetic test image without pushing.
- **Comments-as-contract above `env:` / `permissions:` / `on:` blocks** — every workflow file has detailed prose comments above the structural blocks (see [docker.yml:9-14](.github/workflows/docker.yml#L9), [docker.yml:19-26](.github/workflows/docker.yml#L19)). New comment lines above the `id-token: write` permission addition (D-02) and above the new sign/attest steps must follow this style. The auditor-grepable "deliberately-omitted-scopes" pattern from Phase 22 Plan 22-04 applies.
- **Composite action precedent** — `.github/actions/install-bitcoind/` and `.github/actions/read-base-digests/` set the project's composite-action shape. Phase 23 introduces ZERO new composite actions; all sign/attest logic is third-party action calls. If a composite-action wrapper around `cosign sign` + `actions/attest-*` is tempting for DRY across the matrix legs, the answer is "no" — the 3-leg matrix already deduplicates; wrapping in a composite would add indirection without saving lines.

### Integration Points
- **`docker.yml` `docker` job permissions block** ([docker.yml:61](.github/workflows/docker.yml#L61)) — append `id-token: write` line; add Pitfall-2-citing comment above.
- **`docker.yml` `docker` job steps** (after [docker.yml:110](.github/workflows/docker.yml#L110) `docker/build-push-action` step) — append in order: (a) `sigstore/cosign-installer@<sha>` setup step, (b) `cosign sign --yes <image>@${{ steps.build.outputs.digest }}` step, (c) `actions/attest-build-provenance@<sha>` step with `subject-name: ghcr.io/.../blindjoin-${{ matrix.image }}` + `subject-digest: ${{ steps.build.outputs.digest }}` + `push-to-registry: true`, (d) `actions/attest-sbom@<sha>` step with the same subject-name/subject-digest + `push-to-registry: true`. Build-push step needs an `id:` if it doesn't have one (the [docker.yml:110](.github/workflows/docker.yml#L110) step has no id today — planner adds one, e.g., `id: build`).
- **`ci.yml`** — new `sigstore-pin-check` job (D-04 + D-09 recommendation). Mirrors `bip322-pin-check`; greps under `.github/workflows/` for `sigstore/cosign-installer` / `actions/attest-build-provenance` / `actions/attest-sbom` and fails if any match lacks `@<40-hex>`.
- **`SECURITY.md`** — full `## Supply-chain status` overview-prose + recipes-block + callouts-block rewrite per D-05. The existing `### Base-image digests (v1.6 onward)` subsection (Phase 22 P0-1) stays as a subsection underneath the rewritten overview.
- **`CONTRIBUTING.md`** — likely a one-line cross-reference addition to the rewritten SECURITY.md section. Planner decides if more is needed (e.g., a `### Tagging releases — image verify pre-flight` subsection mirroring the existing v1.4 tagging guidance).

</code_context>

<specifics>
## Specific Ideas

- **Operator-side cosign version pin in SECURITY.md (D-08 default).** RECOMMENDED phrasing: "Tested with cosign ≥ 2.5.0, < 3.0.0. cosign 3.0 may change CLI flags — when it ships, see the project release notes for the updated recipe." Range form, not point form. Planner can deviate with justification.
- **`.bundle` retrieval recipe (D-07).** RECOMMENDED phrasing in the SECURITY.md recipes block: `cosign download signature --bundle blindjoin-<image>.bundle ghcr.io/<owner>/blindjoin-<image>:<tag>` followed by `cosign verify-blob --bundle blindjoin-<image>.bundle ...` to demonstrate offline verification.
- **Cosign verify recipe — generic-by-image-name shape.** ONE documented command with `<image>` as a placeholder, NOT three separate commands per image. The verifier UX is "substitute coordinator|client|liquidity-bot for <image>". The locked `--certificate-identity-regexp` is `.github/workflows/docker\.yml@refs/tags/v.*` — the same workflow file produces all three images, so the regex applies to all three identically.
- **`id-token: write` comment wording.** Suggested: `# id-token: write — cosign OIDC keyless signing requires a Fulcio-issued cert; without this, sign fails with opaque "fulcio: 400 Bad Request". See PITFALLS Pitfall 2.` Audit-grepable, links cause to effect to source.
- **`sigstore-pin-check` job naming.** Suggested CI job id: `sigstore-pin-check` (kebab-case mirrors `bip322-pin-check`). Suggested script path if external: `.github/scripts/sigstore-pin-check.sh`. Both auditor-grepable.
- **Stage 2 fresh-machine container.** RECOMMENDED: `docker run --rm -it ubuntu:24.04` (matches the Phase 25 `ubuntu-24.04` reproducibility runner pin — Pitfall 7 spirit). Avoids `ubuntu:latest` rotation surprises. Inside the container: `apt-get update && apt-get install -y curl && curl -sLo cosign https://github.com/sigstore/cosign/releases/download/v2.5.X/cosign-linux-amd64 && chmod +x cosign` — the cosign install path documented in SECURITY.md must match what an operator following the doc would actually run.

</specifics>

<deferred>
## Deferred Ideas

- **Broad `every uses: must be @<40-hex>` CI grep gate** — D-04 narrowed to sigstore-only. The broad gate would surface pre-existing pre-pinned `dtolnay/rust-toolchain@stable` / `Swatinem/rust-cache@<sha>` triage work that's out of scope for Phase 23's signing focus. Carry-forward candidate for v1.7 quick task.
- **Composite-action wrapper for sign + attest** — considered for DRY across the 3 matrix legs; rejected because the matrix already deduplicates and a wrapper would add indirection. Revisit only if Phase 24 or Phase 25 surfaces a third caller (currently no — Phase 24 signs tarballs, Phase 25 only verifies).
- **`workflow_dispatch.inputs.dry_run` bypass for the tag-gate in `docker.yml`** — D-06 Stage 1 may want this to enable end-to-end sign-step rehearsal on a feature branch without pushing to ghcr.io. Listed as planner discretion. If declined, Stage 1 only covers `check` job + cosign-installer install rehearsal; the sign step itself is first exercised on the rc.0 tag.
- **`### Image verification` anchor subsection in SECURITY.md** — D-05 picked prose+recipes+callouts over per-artifact anchored subsections. If external docs (e.g., the AUDIT-CHARTER or a future blog post) need a stable anchor target, a quick task can add an `<a id="image-verification"></a>` invisible anchor inside the prose without restructuring. Carry-forward candidate.
- **Migrating off `@v3` floating tags on existing pre-Phase-23 actions** (`dtolnay/rust-toolchain@stable`, etc.) — surfaced while scoping D-04. Pre-existing scope, deferred per the narrow-gate decision. v1.7 quick task.
- **Cosign 3.0 migration doc** — Pitfall 13 anticipates this; planner is NOT writing it now. When cosign 3.0 lands, the project opens a quick task to update SECURITY.md's pinned range and add a migration note. v1.7+ depending on cosign release calendar.
- **Severity tagging on `[image-attestation-broken]` issues** (analog to Phase 22's drift-severity carry-forward) — no equivalent automated issue-opener exists for Phase 23 (cosign verify is operator-run, not CI-scheduled). Out of scope; revisit only if a "cosign verify the latest published image every N hours" CI job is later added.

</deferred>

---

*Phase: 23-cosign-image-attestations-slsa-provenance-sbom*
*Context gathered: 2026-06-01*
