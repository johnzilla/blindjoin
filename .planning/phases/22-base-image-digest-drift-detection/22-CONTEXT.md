# Phase 22: Base-Image Digest Drift Detection - Context

**Gathered:** 2026-06-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 22 delivers automated base-image digest discipline: the canonical manifest (`docker/digests.txt`), the daily drift-check workflow that opens — never auto-merges — issues on upstream digest movement, and the release/docker workflows that consume the manifest. After this phase, no tagged release build can publish without being built from the human-reviewed digest list, and any upstream drift surfaces as a `[digest-drift]` issue within 24 hours.

What this phase does NOT do: image signing (Phase 23), tarball signing (Phase 24), reproducible-build recipe (Phase 25). It builds the digest discipline *under* those signing layers — every later phase assumes `docker/digests.txt` exists and is read by the release pipeline.

</domain>

<decisions>
## Implementation Decisions

### Glue placement (the load-bearing structural choice)

- **D-01: Composite action at `.github/actions/read-base-digests/action.yml`.** The "read `docker/digests.txt` → emit `--build-arg DEBIAN_REF=… --build-arg CARGO_CHEF_REF=…`" logic lives in a single composite action that mirrors the existing `.github/actions/install-bitcoind/` pattern. Both `release.yml` and `docker.yml` invoke `- uses: ./.github/actions/read-base-digests` and consume named outputs. Rationale: consistent project shape, single source of truth, reusable by `digest-drift-check.yml` for its parse side.

- **D-02: Named per-image outputs (`debian_ref`, `cargo_chef_ref`).** The composite action is hardcoded to the two images named in DRIFT-01 (`debian:bookworm-slim`, `lukemathwalker/cargo-chef:latest-rust-1`). Callers write `--build-arg DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}` directly — no `fromJSON()` indirection. If a future milestone adds a third base image, the action grows a third named output; the rename is intentional friction (Pitfall 11 — base-image changes deserve human scrutiny).

- **D-03: Fail-fast inside the composite action.** The action validates: file exists, exactly the 2 expected images present, each line matches `^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$`. ANY deviation → `exit 1` with an explicit error pointing at the offending line and at `docs/AUDIT-CHARTER.md`-style supply-chain policy. Release/docker workflows cannot accidentally publish a tag without a valid manifest. The supply-chain guarantee is load-bearing inside the action, not at each caller.

- **D-04: Resolve logic stays separate from parse logic.** The composite action only PARSES `docker/digests.txt`. The daily drift-check workflow (`digest-drift-check.yml`) does its own upstream resolution and diffs against the parsed canonical list. Splitting the two keeps the consumption path (release/docker) minimal and free of any network calls to a registry. Tool choice for resolution is planner discretion (PITFALLS.md Pitfall 9 already constrains the idempotency shape).

### Governance enforcement of "human-reviewed PR only" (DRIFT-01 acceptance)

- **D-05: Prose + CODEOWNERS, no extra workflow gate.** Policy documented in both `SECURITY.md` (operator-facing supply-chain status) and `CONTRIBUTING.md` (contributor-facing PR etiquette). Structural enforcement via `.github/CODEOWNERS` mapping `docker/digests.txt` (and `.github/actions/read-base-digests/**`) to the maintainer's GitHub handle. Branch protection on `main` requiring CODEOWNERS approval is the structural mechanism; auto-merge bots cannot bypass CODEOWNERS approval. A separate `digest-policy-check.yml` label-gate workflow was considered and rejected as belt-suspenders-zipper overkill for a solo-maintained repo.

### Claude's Discretion (planner figures these out, guided by research/PITFALLS.md)

- **Drift-tool choice** in `digest-drift-check.yml` resolve step — `docker buildx imagetools inspect <image>:<tag> --format '{{.Manifest.Digest}}'` is already on the GitHub-hosted runner; `crane digest` is cleaner output but adds an install step; `skopeo` is heavier. Planner picks; favor zero-install if shell parsing of `imagetools` is tractable.

- **Issue body shape** — title format is locked by ROADMAP Success Criteria #2 (`[digest-drift] <image>:<tag> moved to sha256:<HEX>`) and idempotency is locked by Pitfall 9 (skip-create when an open issue with the same title exists, matched via `gh issue list --search`). Body content (old digest, new digest, registry link, Pitfall 8 retag-vs-substantive triage hints), label name (`digest-drift`), and auto-assign behavior are planner discretion.

- **Cron schedule** — DRIFT-02 calls for "daily"; pick a UTC time outside the project's other scheduled work and `workflow_dispatch`-rehearse it before merging (Pitfall 12 fresh-machine UAT spirit).

- **Rehearsal path** — `workflow_dispatch` on `digest-drift-check.yml` so it can be tested against the canonical manifest on any branch before the first scheduled run, identical to the rehearsal pattern documented for `release.yml`/`docker.yml` in `.planning/quick/260531-ubf-*/SUMMARY.md`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase contract (locked WHAT)
- `.planning/REQUIREMENTS.md` §Category 4 — DRIFT-01, DRIFT-02, DRIFT-03 verbatim text. The "manifest format = one `image:tag@sha256:HEX` per line" and "open issue not PR" and "release.yml + docker.yml pass `--build-arg DEBIAN_REF=...` automatically" are non-negotiable.
- `.planning/ROADMAP.md` §Phase 22 — 4 numbered Success Criteria. Acceptance test for this phase.

### Threat-model + design context
- `.planning/research/PITFALLS.md` §Pitfall 8 — "Base-image floating tags retag without semantic change" → drift check opens an ISSUE, never blocks CI; issue body must distinguish docs-only retag from substantive change.
- `.planning/research/PITFALLS.md` §Pitfall 9 — "Digest-drift check auto-opens duplicate issues" → idempotent by `gh issue list --label digest-drift --state open --search "<digest-hex>"`; machine-parseable title format.
- `.planning/research/PITFALLS.md` §Pitfall 11 — "Auto-merging digest bumps undermines the whole supply chain" → opens ISSUE not PR; CODEOWNERS-enforced human review on the manifest file (D-05).
- `.planning/research/PITFALLS.md` §Pitfall 12 — "Fresh-machine UAT every documented command before shipping" → applies to the rehearsal path for digest-drift-check.yml.
- `.planning/research/SUMMARY.md` — phase mapping (digest discipline before signing layers on top), stack additions, and the operator-facing verification command shape.
- `.planning/research/ARCHITECTURE.md` — ordering rationale (Phase 22 first because it's the lowest-risk, no operator-facing change, foundation for Phase 23-25).

### Existing pin discipline (the v1.5 starting point this phase builds on)
- `docker/Dockerfile` (top-of-file comment) — documents the existing ARG-overridable `CARGO_CHEF_REF` and `DEBIAN_REF`. The Dockerfile side is already wired; Phase 22 only adds the manifest + workflow consumption.
- `.planning/quick/260531-thw-v1-5-release-readiness-p0s-security-md-c/260531-thw-SUMMARY.md` §P0-2/3 — describes the v1.5 Dockerfile-side ARG pin work and explicitly defers `docker/digests.txt` + drift check to v1.6 Phase 22.
- `.github/actions/install-bitcoind/action.yml` — composite-action pattern the new `read-base-digests` action mirrors. Read this before structuring the new action's `inputs:` / `outputs:` / `runs:` block.
- `.github/workflows/release.yml` — current build job, current `permissions:` scope, current rehearsal-via-`workflow_dispatch` pattern. This phase adds a `- uses: ./.github/actions/read-base-digests` step and threads its outputs into the existing `cargo build` / `docker build` invocations.
- `.github/workflows/docker.yml` — current matrix-style image build. Same integration shape as release.yml.

### Policy + operator-facing docs (D-05 lands here)
- `SECURITY.md` §Supply-chain status — operator-facing policy paragraph and reference to the new manifest + CODEOWNERS gate.
- `CONTRIBUTING.md` — contributor-facing PR etiquette for `docker/digests.txt` bumps.
- `docs/AUDIT-CHARTER.md` (v1.5 charter, 574 LOC) — supply-chain policy language style + cross-reference target.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`.github/actions/install-bitcoind/`** — exact composite-action precedent (`action.yml` + composite shell steps + outputs). New `.github/actions/read-base-digests/` follows the same file shape.
- **`workflow_dispatch` rehearsal harness** — already wired into `release.yml` and `docker.yml` (see env-block comments referencing `260531-ubf-*` SUMMARY). The new `digest-drift-check.yml` should mirror this so the daily workflow can be hand-fired pre-merge.
- **`gh issue list ... --search` idempotency pattern** — not yet used in any workflow, but `gh` is on every GitHub-hosted runner; no extra install needed for Pitfall 9 dedupe.

### Established Patterns
- **SHA-pinned third-party actions** — every existing `uses:` line in the workflows is SHA-pinned with a `# vX.Y.Z` trailing comment. Any new action (e.g., a `cosign-installer` in Phase 23, or a fresh action in Phase 22 itself) MUST follow this. The composite action `.github/actions/read-base-digests/` is local so it's SHA-implicit via repo checkout.
- **Two-tier gate: `check` job + tag-gated `build`/`docker` job** — both release.yml and docker.yml use this pattern (`if: startsWith(github.ref, 'refs/tags/')`). Phase 22 does not change this; it threads digest reads into the second-tier jobs only.
- **Comments-as-contract above `env:` / `permissions:` blocks** — both workflows have detailed prose comments above their `env:` and `permissions:` blocks explaining what each setting enforces. New workflow file `digest-drift-check.yml` should follow this style; the inline comment is part of the audit trail.

### Integration Points
- `release.yml` `build` job → insert `- uses: ./.github/actions/read-base-digests` step BEFORE `cargo build` (cargo build itself doesn't consume digests; this is preparation for any future digest-aware step in release.yml, and consistency-with-docker.yml).
- `docker.yml` `docker` matrix job → insert `- uses: ./.github/actions/read-base-digests` step BEFORE `docker buildx build`; thread outputs into the existing `--build-arg` lines.
- `.github/workflows/digest-drift-check.yml` (NEW) → schedule + workflow_dispatch; reads canonical manifest via the composite action; resolves upstream digest via planner-chosen tool; idempotent `gh issue create` per Pitfall 9.
- `.github/CODEOWNERS` (NEW) — maps `docker/digests.txt` + `.github/actions/read-base-digests/**` to maintainer handle (`@johnzilla` per existing `SECURITY.md` / cosign-identity references in `research/SUMMARY.md`).

</code_context>

<specifics>
## Specific Ideas

- **Composite-action naming.** The action directory is `.github/actions/read-base-digests/` (kebab-case, matches `install-bitcoind`). The action's `name:` field should read "Read base-image digests" for visual consistency with "Install pinned bitcoind".
- **Error-message wording inside the composite action.** When `docker/digests.txt` is missing or malformed, the failure message must explicitly say something like: "supply-chain: docker/digests.txt is the canonical base-image digest manifest. See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest." The wording is part of the auditor-facing surface — the v1.5 audit-charter prose style applies.
- **Idempotency `gh issue list` search.** The drift-check workflow MUST match Pitfall 9's "by digest-hex" pattern, NOT by image-tag — because the same `<image>:<tag>` can drift to a NEW digest while an old `[digest-drift]` issue is still open from a PREVIOUS drift. Two different drifts of the same tag are two different issues.
- **CODEOWNERS scope.** Map BOTH `docker/digests.txt` AND `.github/actions/read-base-digests/**` to the maintainer — changing the parser logic is functionally the same level of supply-chain risk as changing a digest.

</specifics>

<deferred>
## Deferred Ideas

- **CI lint on PRs touching docker/digests.txt** — considered as belt-and-suspenders during D-03 discussion; rejected for Phase 22 because fail-fast inside the composite action plus CODEOWNERS approval already covers it. Revisit if a future contributor pattern emerges (carry-forward candidate).
- **Generic key/value JSON output for the composite action** — considered for extensibility if more base images appear in Phase 25 reproducible-build work. Deferred per D-02 reasoning: name-based outputs make rename intentional friction. Re-open only if Phase 25 actually grows base-image count.
- **`digest-policy-check.yml` label-gate workflow** — considered during D-05; rejected as overkill for solo-maintained. Carry-forward candidate if the project gains additional maintainers.
- **Drift severity classification (Pitfall 8 retag-vs-substantive)** — REQUIREMENTS.md §Future explicitly defers this to v1.7+ unless v1.6 ships clean without it. Phase 22 may surface enough heuristic signal in the issue body that severity-classification becomes unnecessary; otherwise carry-forward.

</deferred>

---

*Phase: 22-base-image-digest-drift-detection*
*Context gathered: 2026-06-01*
</content>
</invoke>