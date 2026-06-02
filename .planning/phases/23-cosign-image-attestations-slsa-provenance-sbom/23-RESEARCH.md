# Phase 23: cosign Image Attestations + SLSA Provenance + SBOM — Research

**Researched:** 2026-06-01
**Domain:** Sigstore cosign keyless OIDC signing + GitHub Artifact Attestations (SLSA provenance + SPDX SBOM) + GitHub Actions YAML integration
**Confidence:** HIGH on the corrected sign-vs-attest separation (verified against actions/attest README + sigstore docs + cosign 2.6.x release notes); HIGH on permission scopes (verified against multiple sources); MEDIUM on the exact `.bundle` distribution shape (CONTEXT.md D-07 assumption corrected — see §3.4); HIGH on cosign-installer / attest-* SHA pins (resolved against GitHub releases pages at research time).

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01: Inline in the existing `docker` matrix leg.** All new sign/attest steps land in `docker.yml`'s `docker` job AFTER `docker/build-push-action` inside the same matrix leg. `outputs.digest` consumed in-context. `strategy.fail-fast: false` means a sign failure on one image does not block the others. Trade-off accepted: a sign-step failure means re-pushing the image (cosign is idempotent at the registry; re-signing the same digest is safe).
- **D-02: `id-token: write` added at JOB level on the `docker` job, NOT workflow-level** — per PITFALLS Pitfall 2. The new permission gets a dedicated comment line above the `permissions:` block citing Pitfall 2. The Phase 22 Plan 22-04 "auditor-grepable deliberately-omitted-scopes" pattern applies. **⚠️ RESEARCH CORRECTION (§2.1):** the plan must ALSO add `attestations: write` — this is undocumented in CONTEXT.md but required by `actions/attest-build-provenance` and `actions/attest-sbom`. Without it, the attest steps fail with `403 Forbidden`.
- **D-03: `actions/attest-sbom` (GitHub-maintained, SHA-pinned at adoption).** Mirrors `actions/attest-build-provenance` shape. Pushes attestations to the same registry alongside the image. Same OIDC subject claim. **⚠️ RESEARCH CORRECTION (§2.2):** CONTEXT.md asserts `actions/attest-sbom` "invokes Syft internally and emits SPDX format" — **this is INCORRECT**. As of `actions/attest-sbom@v2.4.0` and the v4 wrapper that supersedes it, the action consumes a pre-generated `sbom-path: <file>.spdx.json` file but does NOT generate it. The plan MUST add a separate `anchore/sbom-action` step (Syft under the hood, emits SPDX-JSON by default) BEFORE the `actions/attest-sbom` step. This adds one action to the SHA-pin set (D-04).
- **D-04: Narrow sigstore-pin grep gate (mirrors `bip322-pin-check`).** New CI script/job fails if `sigstore/cosign-installer` / `actions/attest-build-provenance` / `actions/attest-sbom` lack a `@<40-hex>` pin under `.github/workflows/`. **⚠️ EXTENSION:** add `anchore/sbom-action` to the grep target list (per the D-03 correction above). The grep target is now a 4-action stable set.
- **D-05: Full `## Supply-chain status` rewrite using "prose intro + recipes block + callouts block" skeleton.** Phase 22's `### Base-image digests (v1.6 onward)` subsection STAYS untouched as a subsection. **⚠️ RESEARCH CORRECTION (§3.4):** the recipes block must show TWO verify paths: (a) `cosign verify --certificate-identity-regexp ...` for the classic cosign `<digest>.sig` signature, and (b) the `cosign download attestation --predicate-type ...` recipes for retrieving SLSA + SPDX attestations. There is NO single `cosign download signature --bundle` command (the bundle flag exists on `cosign sign`, not on `cosign download signature` — see §3.4). For ATTEST-04's `.bundle` requirement, the recipe is `cosign save` (saves image + sigs + attestations to a local directory) OR `cosign sign --bundle FILE --upload=false ...` at signing time. See §3.4 for the corrected recipe shape.
- **D-06: Pre-merge `workflow_dispatch` verifies the pipeline; post-tag fresh-machine verify runs against `v1.6.0-rc.0`.** Stage 1 proves the pipeline runs end-to-end; Stage 2 proves the operator-facing verify recipe is correct. The `--certificate-identity-regexp` locked at `'refs/tags/v.*'` spans pre-release tags.
- **D-07: `.bundle` distribution mechanism.** Recommended: registry-attached only + document `cosign download` recipe in SECURITY.md. **⚠️ RESEARCH CORRECTION (§3.4):** the CLI shape CONTEXT.md proposed (`cosign download signature --bundle blindjoin.bundle ...`) does NOT exist — `cosign download signature` has no `--bundle` flag. Two valid alternatives: (i) `cosign save --dir blindjoin-image/ ghcr.io/.../blindjoin-<image>:<tag>` produces an offline-verifiable directory containing image + sigs + attestations; (ii) at SIGN time, run `cosign sign --output-signature sig.txt --output-certificate cert.pem ... <image>@<digest>` to produce discrete files. Recommended: (i) `cosign save` — single command, single artifact, no extra workflow plumbing.
- **D-08: Operator-side cosign version pin shape in SECURITY.md.** Recommended `>= 2.5, < 3.0` range.
- **D-09: Drift-detection grep gate location.** Recommended: new job in `ci.yml`.
- **D-10: `actions/attest-sbom` / `actions/attest-build-provenance` / `sigstore/cosign-installer` SHA pins.** Resolved in §2.3 below at planning time.
- **D-11: Cosign installer's installed cosign version.** Recommended: pin to the highest `v2.X.Y` available. Resolved in §2.3 below.

### Claude's Discretion (resolved in §2-§6 below)

- Exact SHA pins for cosign-installer + attest-build-provenance + attest-sbom + anchore/sbom-action — §2.3
- `cosign-release:` input value — §2.3 (recommended: `v2.5.3`, the last cosign 2.5.x; or `v2.6.3` if the operator-side range permits 2.6 — see §2.3 trade-off)
- Exact `.bundle` retrieval CLI shape — §3.4 (RESEARCH CORRECTION — `cosign save` recipe)
- Permission scopes — §2.1 (RESEARCH CORRECTION — must include `attestations: write`)
- SBOM generation step (separate from attest-sbom) — §2.2 (RESEARCH CORRECTION — anchore/sbom-action required)
- `sigstore-pin-check` job/script shape — §5
- HUMAN-UAT recipe specifics (cosign install in fresh container) — §6
- Build-push step `id:` addition — §3.1 (already needed for `${{ steps.build.outputs.digest }}` propagation)

### Deferred Ideas (OUT OF SCOPE for Phase 23)

- Broad `every uses: must be @<40-hex>` CI grep gate — v1.7 carry-forward
- Composite-action wrapper for sign + attest across matrix legs — rejected (matrix already deduplicates)
- `workflow_dispatch.inputs.dry_run` bypass for tag-gate in `docker.yml` — planner discretion in D-06 Stage 1
- `### Image verification` anchor subsection in SECURITY.md — D-05 picked prose+recipes+callouts; anchor is a v1.7 quick task
- Migrating off `@v3`/`@stable` floating tags on pre-Phase-23 actions — v1.7 carry-forward
- Cosign 3.0 migration doc — written when cosign 3.0 lands and the project upgrades
- Severity tagging on `[image-attestation-broken]` issues — no automated issue-opener exists for Phase 23
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **ATTEST-01** | Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z` image push is signed by cosign via OIDC keyless flow. Signature reachable at `<image-digest>.sig` in the registry. | §3.1 — `cosign sign` step shape; §3.4 — `<digest>.sig` storage convention (verified against [augmentedmind.de](https://www.augmentedmind.de/2025/03/02/docker-image-signing-with-cosign/) — cosign 2.x stores at `sha256-<HEX>.sig` registry tag). |
| **ATTEST-02** | Every signed image carries a SLSA v1.0 build-level-3 in-toto provenance attestation emitted by `actions/attest-build-provenance`, naming the workflow file + ref + source commit + build environment. | §3.2 — `actions/attest-build-provenance` step shape with `subject-name` + `subject-digest` + `push-to-registry: true`; §3.4 — `cosign download attestation --predicate-type https://slsa.dev/provenance/v1` retrieval recipe. |
| **ATTEST-03** | Every signed image carries an SBOM attestation (SPDX format, generated by Syft) so operators can scan for CVE exposure without pulling the image. | §3.3 — TWO-STEP pattern: `anchore/sbom-action` generates `sbom.spdx.json` (Syft default = SPDX-JSON), then `actions/attest-sbom` consumes via `sbom-path:`; §3.4 — `cosign download attestation --predicate-type https://spdx.dev/Document` retrieval. |
| **ATTEST-04** | Every signed image has a downloadable cosign `.bundle` asset (sig + cert + Rekor inclusion proof) usable for offline verification once cached. | §3.4 — **CORRECTED RECIPE.** `cosign save --dir ./blindjoin-image ghcr.io/.../blindjoin-<image>:<tag>` is the canonical offline-export path (cosign 2.x). Saves image + sig + cert + attestations + Rekor inclusion proof to a local directory tree that `cosign verify --local-image ./blindjoin-image ...` can verify offline. The original CONTEXT.md `cosign download signature --bundle ...` CLI shape does NOT exist — verified at https://github.com/sigstore/cosign/blob/main/doc/cosign_download_signature.md. |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **No protocol code touched** — pure CI/CD/docs (mirrors Phase 22 scope discipline).
- **MIT, public good** — supply-chain hardening for operators, not a vendor sales motion.
- **Tor-native + signet-first** — neither invariant regresses (workflow changes only).
- **`/gsd` workflow enforcement** — Phase 23 work goes through `/gsd:execute-phase 23` after planning completes; no direct repo edits outside the GSD workflow.
- **Project skills** — `.claude/skills/` and `.agents/skills/` do not exist (verified). No project-specific skill rules to honor beyond CLAUDE.md.

---

## 1. Phase Goal Recap

From ROADMAP §Phase 23 + the phase description: *"Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z` image carries a cryptographically verifiable binding to the maintainer's GitHub Actions OIDC identity, the source commit it was built from, and a machine-readable SBOM — all reachable in the registry without maintainer key custody."*

This phase delivers, INSIDE `docker.yml`'s existing `docker` matrix job and inheriting the existing `if: startsWith(github.ref, 'refs/tags/')` gate:

1. A `sigstore/cosign-installer` setup step (installs cosign 2.5+ on the runner).
2. A `cosign sign --yes <image>@<digest>` step (ATTEST-01 — produces the `<digest>.sig` artifact).
3. An `anchore/sbom-action` step that scans the just-pushed image and produces `sbom.spdx.json` (Syft + SPDX-JSON default) — **NEW per RESEARCH CORRECTION §2.2; not in CONTEXT.md**.
4. An `actions/attest-build-provenance@<sha>` step (ATTEST-02 — produces the SLSA v1.0 in-toto attestation).
5. An `actions/attest-sbom@<sha>` step consuming the SBOM file from step 3 (ATTEST-03).
6. Optionally — and recommended for ATTEST-04's `.bundle` requirement — a final `cosign save` step that exports image + sigs + attestations to a directory operators can verify offline. **OR** the operator follows the documented `cosign save` recipe themselves (off-CI). See §3.4 trade-off discussion.

Additionally, in `ci.yml`: a new `sigstore-pin-check` job (D-04 + D-09) that greps for the four new sigstore-ecosystem actions and fails if any lacks a 40-hex SHA pin. Pattern mirrors the in-repo `bip322-pin-check` / `crit-01-grep-check` family.

Finally, in `SECURITY.md`: a full `## Supply-chain status` overview-prose + recipes-block + callouts-block rewrite (D-05) that adds operator-facing cosign verify recipes + downloads + Pitfall 10 / Pitfall 13 callouts.

---

## 2. Research Corrections (CONTEXT.md assertions that need updating)

### 2.1 `attestations: write` permission is required (D-02 supplement) [VERIFIED: docs.github.com + github.blog]

**CONTEXT.md D-02 names ONLY `id-token: write` as the new permission.** This is incomplete. Both `actions/attest-build-provenance` and `actions/attest-sbom` require THREE permissions to function:

| Permission | Why | Source |
|-----------|-----|--------|
| `id-token: write` | OIDC token for Fulcio cert exchange | PITFALLS Pitfall 2 + verified at [docs.github.com](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds) |
| `attestations: write` | Persist the attestation to GitHub's attestations API | [docs.github.com (same page)](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds) + verified at [github.blog Supply-chain post](https://github.blog/security/supply-chain-security/configure-github-artifact-attestations-for-secure-cloud-native-delivery/) — **"`attestations: write` permission is necessary to persist the attestation"** |
| `packages: write` | Push image to ghcr.io + push attestation to registry | Already present on the `docker` job at [docker.yml:63](.github/workflows/docker.yml#L63) |

**Action for the planner:** the `docker` job's `permissions:` block grows from:

```yaml
    permissions:
      contents: read
      packages: write
```

to:

```yaml
    # Phase 23 ATTEST-01/02/03: cosign keyless signing + attest-* actions need:
    #   - id-token:      OIDC token for Fulcio cert exchange. Without this,
    #                    cosign sign fails with the opaque "fulcio: 400 Bad
    #                    Request" error. See PITFALLS Pitfall 2.
    #   - attestations:  persist the attestation to GitHub's attestations API.
    #                    Without this, actions/attest-build-provenance fails
    #                    with 403 Forbidden on the API call.
    #   - packages:      already present — push image + attestation to ghcr.io.
    # Deliberately omitted (auditor-grepable per Plan 22-04): PR-write, pages.
    permissions:
      contents: read
      packages: write
      id-token: write
      attestations: write
```

The "deliberately-omitted" list uses `PR-write` and `pages` (paraphrased, not literal `pull-requests:` / `pages:`) so the Phase 22 Plan 22-04 audit-gate pattern (`! grep -q 'pull-requests:'` at file level) continues to hold.

### 2.2 `actions/attest-sbom` does NOT generate the SBOM — an external generator is required (D-03 correction) [VERIFIED: actions/attest-sbom v2.4.0 README]

**CONTEXT.md D-03 asserts:** *"The action invokes Syft internally and emits SPDX format (REQUIREMENTS ATTEST-03 specifies SPDX-via-Syft; `actions/attest-sbom` defaults to Syft + SPDX, no `format` override needed)."*

**Reality (verified at [actions/attest-sbom v2.4.0 README](https://github.com/actions/attest-sbom/blob/v2.4.0/README.md)):** *"The action itself does not generate SBOMs. The examples show using external tools: the documentation states it 'accepts SBOMs which have been generated by external tools.' Examples use Anchore's sbom-action."*

The action takes a `sbom-path: <file>.spdx.json` input. It accepts SPDX-JSON or CycloneDX-JSON. It does NOT scan filesystems or images; it just signs and attests a pre-existing JSON file.

**Action for the planner:** the plan MUST add an `anchore/sbom-action@<sha>` step BEFORE the `actions/attest-sbom` step. The SBOM scope is the just-built container image (passed via `image:` input). Syft is the default scanner under `anchore/sbom-action`; SPDX-JSON is the default output format. This satisfies REQUIREMENTS ATTEST-03's "SPDX format, generated by Syft" verbatim — just via two steps instead of one.

Verified YAML shape (from [anchore/sbom-action README](https://github.com/anchore/sbom-action), latest v0.24.0):

```yaml
- name: Generate SPDX SBOM (Syft) for ATTEST-03
  uses: anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0.24.0
  with:
    image: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}@${{ steps.build.outputs.digest }}
    format: spdx-json
    output-file: sbom.spdx.json
    # Do not upload to GH release artifacts — this SBOM exists to feed the
    # attest-sbom step. The attestation IS the operator-facing artifact.
    upload-artifact: false
    upload-release-assets: false
```

Then the `actions/attest-sbom` step consumes `sbom.spdx.json` via its `sbom-path:` input. See §3.3 for the integrated shape.

### 2.3 SHA pins resolved at planning time (D-10 + D-11) [VERIFIED: GitHub Releases pages, 2026-06-01]

All four sigstore-ecosystem actions Phase 23 introduces, pinned to their current latest stable release at research time:

| Action | Version | Commit SHA | Released | Notes |
|--------|---------|------------|----------|-------|
| `sigstore/cosign-installer` | **v3.10.1** | `7e8b541eb2e61bf99390e1afd4be13a184e9ebc5` | 2023-10-16 | **Pinned to v3.X (not v4.X) for cosign 2.x compatibility.** Verified at [v3.10.1 release notes](https://github.com/sigstore/cosign-installer/releases/tag/v3.10.1): "cosign-installer v3.x cannot install Cosign v3.x — users must upgrade to cosign-installer v4 for that capability." Since CONTEXT D-08 + D-11 lock to cosign 2.x for operator-side consistency, the installer MUST stay on v3.X. Default cosign version installed by v3.10.1 is v2.6.1. |
| `sigstore/cosign-installer` `cosign-release:` input | **`v2.5.3`** (recommended) OR **`v2.6.3`** (recommended-alternative) | n/a (release tag) | 2024-07-17 (v2.5.3) / 2026-04-06 (v2.6.3) | **Trade-off.** CONTEXT.md D-08 recommended pin `>= 2.5, < 3.0`. The operator-side SECURITY.md docs should match what CI uses. `v2.5.3` is the last `2.5.x` and matches the D-08 minimum-of-2.5 literal; `v2.6.3` is the latest 2.x stable. Recommended: pin to **`v2.6.3`** in CI AND update SECURITY.md to "Tested with cosign `>= 2.6.3, < 3.0.0`" — gives operators the latest bug fixes (notably the v2.5.3 → v2.6.x CLI is stable, no breaking changes verified at [v2.5.3 release notes](https://github.com/sigstore/cosign/releases/tag/v2.5.3) through current). |
| `actions/attest-build-provenance` | **v3.2.0** | `96278af6caaf10aea03fd8d33a09a777ca52d62f` | 2026-01-26 | **Pinned to v3.X (not v4.X).** Verified at [actions/attest-build-provenance releases](https://github.com/actions/attest-build-provenance/releases): "As of version 4, `actions/attest-build-provenance` is simply a wrapper on top of `actions/attest`. Existing applications may continue to use the `attest-build-provenance` action, but new implementations should use `actions/attest` instead." Migrating to `actions/attest@v4` directly is the v1.7 carry-forward; v3.2.0 is the last "real" attest-build-provenance and the path that matches all existing community examples + CONTEXT D-03's locked action name. |
| `actions/attest-sbom` | **v2.4.0** | `bd218ad0dbcb3e146bd073d1d9c6d78e08aa8a0b` | 2023-06-11 | **Pinned to v2.X (not v3.X or v4.X).** Same wrapper-deprecation rationale as attest-build-provenance — v3.0 and v4.0 are increasingly thin wrappers around `actions/attest`. v2.4.0 is the last release with full standalone documentation. Latest "real" version that does what CONTEXT.md D-03 names. |
| `anchore/sbom-action` | **v0.24.0** | `e22c389904149dbc22b58101806040fa8d37a610` | 2025-03-20 | **NEW per RESEARCH CORRECTION §2.2.** This action does NOT appear in CONTEXT.md but is REQUIRED for ATTEST-03's "SPDX-via-Syft" path (since attest-sbom does not generate SBOMs internally). Syft is bundled internally; SPDX-JSON is the default format. |

**Composite SHA-pin line shape** (matches the existing project pattern at every `uses:` line in docker.yml — verified at [docker.yml:36](.github/workflows/docker.yml#L36), [docker.yml:78](.github/workflows/docker.yml#L78), [docker.yml:84](.github/workflows/docker.yml#L84), [docker.yml:91](.github/workflows/docker.yml#L91), [docker.yml:110](.github/workflows/docker.yml#L110)):

```yaml
- uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1
  with:
    cosign-release: 'v2.6.3'
- uses: anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0.24.0
- uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0
- uses: actions/attest-sbom@bd218ad0dbcb3e146bd073d1d9c6d78e08aa8a0b # v2.4.0
```

All four lines satisfy the D-04 grep gate (full 40-hex SHA + `# v<X.Y.Z>` trailing comment, two spaces before `#`).

**Planner option (carry-forward):** if the planner wants to future-proof for the v4 migration path, the recommendation is to pin to v3.2.0/v2.4.0 NOW (matches CONTEXT.md D-03 locked action names) and file a v1.7 carry-forward note to migrate to `actions/attest@v4` as a single consolidated step in a future phase. The v3/v2 wrappers continue to function correctly per the upstream README.

### 2.4 `actions/attest-build-provenance` storage format differs from cosign sign (D-07 supplement) [VERIFIED: actions/attest-build-provenance docs + augmentedmind.de + ianlewis.org]

**Background fact CONTEXT.md does not flag clearly:** `actions/attest-build-provenance` and `cosign sign` are NOT the same thing. They produce different artifacts, store them under different OCI tag conventions, and are verified by different commands. The phase needs BOTH:

| Producer | Artifact | OCI storage tag | Verification command |
|----------|----------|-----------------|----------------------|
| `cosign sign --yes <image>@<digest>` | Cosign-format signature (sig + cert + Rekor proof) | `<image>:sha256-<HEX>.sig` (cosign 2.x default; OCI 1.1 referrer in cosign 2.6+) | `cosign verify <image>@<digest> --certificate-identity-regexp ... --certificate-oidc-issuer ...` |
| `actions/attest-build-provenance` (push-to-registry: true) | SLSA v1.0 in-toto provenance attestation bundle | `<image>:sha256-<HEX>` referrer manifest (GitHub attestation API + registry referrer) | `gh attestation verify oci://<image>@<digest> --repo <owner>/<repo>` (recommended) OR `cosign verify-blob-attestation --bundle <downloaded.json> --new-bundle-format ...` |
| `actions/attest-sbom` (push-to-registry: true) | SLSA in-toto SBOM attestation bundle (SPDX predicate) | Same `<image>:sha256-<HEX>` referrer (separate attestation in the same referrer manifest) | Same as above (`gh attestation verify` is the primary path; cosign verify-blob-attestation is the secondary path) |

**Verified at [augmentedmind.de Docker Image attestation post](https://www.augmentedmind.de/2025/03/09/docker-image-attestations-github/):** *"Yes, push-to-registry: true pushes attestations directly to the OCI registry. However, the storage mechanism differs from traditional Cosign signatures. The article explains: GitHub creates 'a new _tag_ named `sha256-<digest>`' containing a manifest that emulates the _referrers API_. Rather than using the `.sig` naming convention, attestations are stored as separate OCI artifacts referenced through manifest annotations with the predicate type `https://slsa.dev/provenance/v1`."*

**Verified at [actions/attest-build-provenance issue #162](https://github.com/actions/attest-build-provenance/issues/162):** the issue (still open as of research time) asks how to verify attest-build-provenance output with `cosign verify`/`cosign download attestation`. The verified status: *"The only successful verification method demonstrated in the issue is using the GitHub CLI: `gh attestation verify` worked correctly."* The standalone `cosign download attestation` path returns 404 on these artifacts because they use referrer-API semantics, not the legacy `.att` tag convention.

**What this means for ATTEST-01 specifically:** ATTEST-01 requires the signature to be reachable at `<image-digest>.sig`. **`actions/attest-build-provenance` alone does NOT satisfy ATTEST-01** — it stores at `sha256-<HEX>` referrer tag, not `<digest>.sig`. The plan MUST include `cosign sign` as a separate step to satisfy ATTEST-01's literal text.

**What this means for the operator verify path in SECURITY.md (D-05):** the recipes block must show TWO verify flows:
1. **Image signature** (ATTEST-01) → `cosign verify <image>@<digest> --certificate-identity-regexp ...`
2. **SLSA + SBOM attestations** (ATTEST-02 + ATTEST-03) → `gh attestation verify oci://<image>@<digest> --repo <owner>/<repo>` (primary), with `cosign download attestation` mentioned as a known-limitation alternative that may 404 against GitHub attestation artifacts.

### Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Keyless OIDC signing (ATTEST-01) | CI runner (GHA `docker` job) | Sigstore Fulcio (cert issuance) + Rekor (transparency log) | The signing identity IS the GHA OIDC token; Fulcio binds it to a cert that's only valid for ~10 min. No local key material on disk. |
| SLSA in-toto provenance (ATTEST-02) | CI runner (`actions/attest-build-provenance`) | GitHub attestations API + OCI referrer manifest in ghcr.io | The provenance is derived from the workflow context (job spec, runner image, commit SHA) — only the runner has all of that. |
| SBOM generation (ATTEST-03 generator step) | CI runner (`anchore/sbom-action` via Syft) | n/a | Syft scans the pushed image; the SBOM must be regenerated for each release because Cargo dep changes drop in via the build cache. |
| SBOM attestation signing (ATTEST-03 attest step) | CI runner (`actions/attest-sbom`) | GitHub attestations API + OCI referrer | Same identity binding as ATTEST-02; SPDX predicate inside the same referrer manifest. |
| Offline-verifiable bundle (ATTEST-04) | Operator workstation (post-`cosign save`) | OCI registry (source) | `cosign save` is operator-run, not CI-run. CI's job is to make the registry-side artifacts complete (sig + cert + Rekor proof + attestations all reachable); the operator pulls them once and verifies offline thereafter. |
| Pin enforcement (D-04 grep gate) | CI runner (`ci.yml` job) | n/a | Grep-only; runs on every PR. |
| Operator-facing verify recipes (D-05) | `SECURITY.md` (operator-facing docs) | n/a | Documentation tier; consumed off-CI. |

---

## 3. Integration Shape — `docker.yml` step-by-step

### 3.1 Build-push step needs an `id:` added (no other changes to existing step)

[docker.yml:110](.github/workflows/docker.yml#L110) currently has NO `id:` on the `docker/build-push-action` step. The new sign/attest steps all consume `${{ steps.<build-id>.outputs.digest }}`. The planner adds `id: build` to the existing step:

```yaml
      - uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0
        id: build  # ← Phase 23: needed by cosign sign + attest-* downstream
        with:
          context: .
          ...
```

This is the ONLY change to the existing build-push step. The `with:` block (lines 111-126) is untouched — no new inputs like `provenance:` or `sbom:` get added there. **Why not use `docker/build-push-action`'s built-in `provenance: mode=max` + `sbom: true`?** Per CONTEXT.md D-03 the locked path is `actions/attest-build-provenance` + `actions/attest-sbom` (Pitfall 5 — pick ONE path; the GitHub-maintained actions are the chosen one). Setting `provenance:` / `sbom:` on the build-push step would compete with the attest-* actions and produce two attestations per image (one BuildKit-native, one GitHub-native) — exactly the "two competing attestations" failure mode PITFALLS Pitfall 5 names.

### 3.2 New step: `sigstore/cosign-installer` (setup) + `cosign sign` (ATTEST-01)

**Insertion point:** immediately after the modified `docker/build-push-action` step (so `${{ steps.build.outputs.digest }}` is available).

```yaml
      # Phase 23 ATTEST-01: cosign keyless OIDC signing.
      # Installer pinned to v3.10.1 (last v3.X — see RESEARCH §2.3). v4.X
      # of cosign-installer mandates cosign 3.x which is operator-incompatible
      # with the documented `>= 2.5, < 3.0` range in SECURITY.md. Pin
      # cosign-release to v2.6.3 (latest 2.x stable; no CLI-breaking changes
      # from 2.5.x verified at sigstore/cosign release notes).
      - name: Install cosign (keyless signing toolchain)
        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1
        with:
          cosign-release: 'v2.6.3'

      # Phase 23 ATTEST-01: produces ghcr.io/.../blindjoin-<image>:sha256-<HEX>.sig
      # — a Fulcio-issued cert bound to the GHA OIDC subject claim, plus the
      # Rekor inclusion proof. Operator-side `cosign verify` recipe in
      # SECURITY.md verifies against the locked --certificate-identity-regexp.
      #
      # --yes: non-interactive; required in CI (would otherwise prompt for
      # transparency-log consent).
      # Re-running this step on the same digest is idempotent at the registry
      # (cosign 2.x stores at content-addressed `sha256-<HEX>.sig`; re-sign
      # produces a new Rekor entry but the registry tag is overwritten safely).
      # See PITFALLS Pitfall 3: do NOT add `--no-tlog-upload` — Rekor is the
      # operator-facing transparency guarantee.
      - name: Sign image with cosign (keyless OIDC)
        env:
          IMAGE: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          DIGEST: ${{ steps.build.outputs.digest }}
        run: cosign sign --yes "${IMAGE}@${DIGEST}"
```

### 3.3 New steps: SBOM generation (anchore/sbom-action) + SBOM attestation (actions/attest-sbom) [ATTEST-03]

**Insertion point:** after the `cosign sign` step. Two steps:

```yaml
      # Phase 23 ATTEST-03 (generator half): scan the just-pushed image with
      # Syft (bundled inside anchore/sbom-action) and write the SBOM to
      # sbom.spdx.json. SPDX-JSON is the action's default format; explicit
      # for auditor clarity. Scope: full image filesystem (Syft default).
      #
      # The image reference uses the build-push-action's outputs.digest so
      # we scan the EXACT bytes that were pushed — avoids the latent race
      # where a Syft scan of the local-tagged image could lag behind the
      # registry-pushed image.
      #
      # upload-artifact: false / upload-release-assets: false — the operator-
      # facing artifact is the ATTESTATION (next step), not the SBOM file
      # itself. Skipping the upload also keeps the workflow_dispatch rehearsal
      # path (D-06 Stage 1) free of stale workflow-artifact spam.
      - name: Generate SPDX SBOM with Syft (ATTEST-03 generator)
        uses: anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0.24.0
        with:
          image: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}@${{ steps.build.outputs.digest }}
          format: spdx-json
          output-file: sbom.spdx.json
          upload-artifact: false
          upload-release-assets: false

      # Phase 23 ATTEST-03 (attest half): sign sbom.spdx.json as a SLSA
      # in-toto SBOM attestation and push to the OCI registry alongside
      # the image (referrer manifest at ghcr.io/.../blindjoin-<image>:sha256-<HEX>).
      #
      # attest-sbom does NOT generate the SBOM — it consumes the file written
      # by the previous step. CONTEXT.md D-03 originally asserted this action
      # "invokes Syft internally"; that's incorrect as of v2.4.0 — see
      # RESEARCH §2.2 for the correction.
      #
      # Predicate type emitted: https://spdx.dev/Document (auto-derived from
      # the sbom-path file format).
      - name: Attest SBOM (ATTEST-03)
        uses: actions/attest-sbom@bd218ad0dbcb3e146bd073d1d9c6d78e08aa8a0b # v2.4.0
        with:
          subject-name: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          subject-digest: ${{ steps.build.outputs.digest }}
          sbom-path: 'sbom.spdx.json'
          push-to-registry: true
```

### 3.4 New step: SLSA provenance attestation (actions/attest-build-provenance) [ATTEST-02]

**Insertion point:** after the SBOM steps (order between attest-sbom and attest-build-provenance is not contentful; alphabetical SBOM-then-provenance is a reasonable convention).

```yaml
      # Phase 23 ATTEST-02: SLSA v1.0 in-toto build provenance attestation.
      # GitHub-maintained action; emits predicate-type https://slsa.dev/provenance/v1
      # naming the workflow file (docker.yml), the tag ref, the source commit,
      # and the runner image — all derived from the workflow context, so the
      # `with:` block only carries the subject identification.
      #
      # push-to-registry: true puts the attestation in the OCI registry as a
      # referrer of ghcr.io/.../blindjoin-<image>@sha256:<HEX>. Operator can
      # retrieve it via `gh attestation verify oci://...` (primary) — see
      # SECURITY.md recipes block (D-05) and RESEARCH §3.5.
      #
      # Pinned to v3.2.0 (NOT v4.X) — v4 of attest-build-provenance is a
      # wrapper on actions/attest. v3.2.0 is the last release that does what
      # CONTEXT.md D-03 names. v1.7 carry-forward: migrate to
      # `actions/attest@v4` as a single step. See RESEARCH §2.3.
      - name: Attest build provenance (ATTEST-02)
        uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0
        with:
          subject-name: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          subject-digest: ${{ steps.build.outputs.digest }}
          push-to-registry: true
```

### 3.5 ATTEST-04 (.bundle): RESEARCH CORRECTION — no CI step needed; recipe lives in SECURITY.md

**CONTEXT.md D-07 proposed:** *"`cosign download signature --bundle blindjoin-<image>.bundle ghcr.io/<owner>/blindjoin-<image>:<tag>` followed by `cosign verify-blob --bundle blindjoin-<image>.bundle ...` to demonstrate offline verification."*

**Reality (verified at [sigstore/cosign download_signature docs](https://github.com/sigstore/cosign/blob/main/doc/cosign_download_signature.md)):** the `cosign download signature` command has NO `--bundle` flag. Its available flags are registry-auth (`--registry-cacert`, `--registry-token`, etc.) and the inherited `--output-file`. There is no way to produce a "bundle" file with that command.

**Three valid alternatives that satisfy ATTEST-04's "downloadable cosign `.bundle` asset usable for offline verification" wording:**

| Path | CLI shape | Trade-off |
|------|-----------|-----------|
| **(i) `cosign save` (RECOMMENDED — operator-side, off-CI)** | `cosign save --dir ./blindjoin-image ghcr.io/<owner>/blindjoin-<image>:<tag>` produces a directory containing the image manifest, layers, sig manifest, cert, Rekor proof, and attestations. `cosign verify --local-image ./blindjoin-image --certificate-identity-regexp ...` verifies entirely offline thereafter. | Single command. Documented in SECURITY.md. No CI plumbing change. Single directory artifact is "the bundle" in a broad sense — the user has everything needed to verify without network. **Most idiomatic in cosign 2.x.** |
| **(ii) `cosign sign --bundle <FILE>` at SIGN time** | Add `--bundle blindjoin-<image>-sig.bundle --upload=false` to the `cosign sign` step in §3.2, AND a follow-up `oras attach` step to push the bundle as an OCI artifact alongside the image. | Adds two flags + one new step + introduces `--upload=false` which means we then need a separate step to actually push the signature. **Rejected — much more complex than (i) and the registry-stored sig is already enough for ATTEST-01.** |
| **(iii) `cosign sign --output-signature sig.txt --output-certificate cert.pem` + upload as GH Release assets** | Sign as in §3.2 but additionally write the discrete sig + cert files, then upload as workflow artifacts (not GH Release — Phase 23 doesn't touch release.yml). | Discrete files, not a bundle. Operator must reassemble. **Rejected — discrete sig/cert/Rekor handling is the failure mode the bundle format exists to prevent.** |

**Recommended planner choice:** **(i) `cosign save` documented in SECURITY.md, no CI step change**. The `.bundle` requirement of ATTEST-04 is satisfied because the directory output of `cosign save` contains everything an operator needs for offline verification, and `cosign verify --local-image ./<dir>` does exactly that. The CONTEXT.md `.bundle` literal-string wording is preserved in the SECURITY.md recipe text (we can call the output directory "the offline verification bundle" — same semantic content, different filesystem shape).

**SECURITY.md recipe for §4 (D-05) — the operator-facing `cosign save` block:**

```bash
# Offline-verifiable bundle (ATTEST-04): save image + sig + cert + Rekor proof
# + attestations to a local directory. After this command runs once with
# network access, all subsequent `cosign verify --local-image ./<dir> ...`
# invocations verify entirely offline.
cosign save --dir ./blindjoin-coordinator-1.6.0 \
  ghcr.io/<owner>/blindjoin-coordinator:1.6.0

cosign verify --local-image ./blindjoin-coordinator-1.6.0 \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
```

---

## 4. Operator-facing verify recipes (SECURITY.md D-05 block)

The full corrected recipes block for SECURITY.md `## Supply-chain status` (replaces the v1.5 paragraphs about Docker images being unsigned). Locked text below is what the planner copies into the SECURITY.md task.

````markdown
### Image signatures + attestations (v1.6 onward)

Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z`
image push from a `vX.Y.Z` tag is:

1. **Signed by cosign** via OIDC keyless flow (no maintainer key custody). The
   signature is stored in the registry under `sha256-<HEX>.sig` and includes
   the Fulcio-issued cert bound to the GitHub Actions OIDC identity + the
   Rekor transparency-log inclusion proof.
2. **Attested with a SLSA v1.0 in-toto provenance bundle** (predicate type
   `https://slsa.dev/provenance/v1`), naming the workflow file + tag ref +
   source commit + runner image. Stored as an OCI referrer of the image.
3. **Attested with a SPDX SBOM** (predicate type `https://spdx.dev/Document`),
   generated by Syft against the full image filesystem. Stored as a sibling
   referrer of the SLSA attestation.

Verification requires **cosign 2.6.3 or compatible** and the **GitHub CLI
(`gh`) 2.x or later**. The verify recipes below have been tested on a clean
`ubuntu:24.04` container.

```bash
# 1. Cosign signature verification (ATTEST-01)
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/<owner>/blindjoin-<image>:<tag>
# Substitute <image> = coordinator | client | liquidity-bot
# Expected: "Verification for ghcr.io/.../blindjoin-<image>:<tag> --" + JSON
#           output of the verified cert claims.

# 2. SLSA build provenance attestation (ATTEST-02)
gh attestation verify oci://ghcr.io/<owner>/blindjoin-<image>:<tag> \
  --repo <owner>/blindjoin \
  --predicate-type https://slsa.dev/provenance/v1
# Expected: "Loaded N attestation(s) ... ✓ Verified provenance attestation."

# 3. SBOM attestation (ATTEST-03)
gh attestation verify oci://ghcr.io/<owner>/blindjoin-<image>:<tag> \
  --repo <owner>/blindjoin \
  --predicate-type https://spdx.dev/Document
# Expected: same shape as (2), with the SPDX SBOM payload.

# 4. Offline-verifiable bundle directory (ATTEST-04)
cosign save --dir ./blindjoin-<image>-<tag> \
  ghcr.io/<owner>/blindjoin-<image>:<tag>
cosign verify --local-image ./blindjoin-<image>-<tag> \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
# After step 1's `cosign save` completes (one-time network), all subsequent
# `cosign verify --local-image` invocations against this directory verify
# offline. Recipes 1–3 require network access to Fulcio + Rekor + the OCI
# registry; recipe 4's verify step does not.
```

> **Note: GHCR UI "Unverified" badge** is unrelated to cosign verification
> (Pitfall 10). GHCR's web view does not consult Rekor by default. **The
> `cosign verify` CLI output is the source of truth for signature status.**
> Operators should not interpret a "Verified" / "Unverified" badge on the
> GHCR web UI as a substitute for running the verify recipes above. GitHub
> may add cosign-aware UI in a future GHCR release.

> **Note: cosign 3.0 CLI flag drift** (Pitfall 13). The recipes above have
> been tested with **cosign `>= 2.6.3, < 3.0.0`**. cosign 3.0 (released
> 2026 — see [sigstore/cosign releases](https://github.com/sigstore/cosign/releases))
> may change CLI flags; when blindjoin's pipeline upgrades to cosign 3.x,
> the project will publish an updated recipe and a migration note in the
> release notes. **Until then, install cosign in the documented version
> range** — see the cosign release page for binary downloads.
````

**Important:** the existing `### Base-image digests (v1.6 onward)` subsection (Phase 22 P0-1, [SECURITY.md:122-159](SECURITY.md#L122)) stays UNTOUCHED below this new subsection. The Phase 22 strikethrough on "Base image digest pins are manual" continues to apply. Two additional strikethrough lines get added to the "Known gaps at v1.5" list:

```markdown
- **~~GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign signature.~~**
  Closed in v1.6 Phase 24 — see [Release tarball signatures (v1.6 onward)](#release-tarball-signatures-v16-onward).
  <!-- Phase 24 adds this subsection; Phase 23 leaves the strikethrough as a forward-reference -->
- **~~Docker images on `ghcr.io` are unsigned.~~** **Closed in v1.6 Phase 23**
  — see [Image signatures + attestations (v1.6 onward)](#image-signatures--attestations-v16-onward).
```

The Phase 24 line is added forward-looking (the strikethrough is true once Phase 24 ships; if Phase 23 lands first, Phase 24's plan adds its own anchor and the comment goes away). This is consistent with the Phase 22 pattern for cross-phase status tracking.

---

## 5. `sigstore-pin-check` CI gate (D-04 + D-09 + RESEARCH §2.2 extension)

**Location:** new job in `ci.yml`, alphabetically after `crit-01-client-grep-check` (end of file, [ci.yml:265](.github/workflows/ci.yml#L265)) and before any future grep-check jobs. Mirrors the exact YAML shape of `bip322-pin-check` ([ci.yml:214-236](.github/workflows/ci.yml#L214)).

**Grep target list (FOUR actions, per RESEARCH §2.2 extension):**

| Action | Why included |
|--------|-------------|
| `sigstore/cosign-installer` | D-04 — original target. |
| `actions/attest-build-provenance` | D-04 — original target. |
| `actions/attest-sbom` | D-04 — original target. |
| `anchore/sbom-action` | **RESEARCH §2.2 extension.** Without Syft, attest-sbom has nothing to sign — this action is load-bearing for ATTEST-03 and supply-chain-equivalent to the attest-* actions. |

**Job shape (drop-in for `ci.yml`):**

```yaml
  sigstore-pin-check:
    name: sigstore + sbom action SHA-pin check
    runs-on: ubuntu-latest
    # v1.6 Phase 23 ATTEST-01/02/03 invariant: the four GitHub Actions that
    # produce the supply-chain attestations (sigstore/cosign-installer,
    # actions/attest-build-provenance, actions/attest-sbom, anchore/sbom-action)
    # MUST be pinned to a 40-hex commit SHA in every workflow under
    # .github/workflows/. Floating tags like @v3 expose the project to silent
    # action substitution — exactly the attack surface this milestone is closing.
    # Pattern mirrors bip322-pin-check (v1.4) and the v1.5 crit-01-grep-check
    # family — narrow, audit-grepable, named after what it enforces.
    # RESEARCH.md §2.2 adds anchore/sbom-action to the target list because Syft
    # is the ATTEST-03 SBOM generator (attest-sbom does NOT generate SBOMs
    # internally as of v2.4.0).
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - name: Enforce SHA pin on sigstore + sbom actions
        run: |
          set -eu
          # Greps for any of the four target actions used WITHOUT a 40-hex SHA
          # ref. Match form: `uses: <owner>/<repo>@<not-40-hex>` (e.g. @v3, @main).
          # Exits 1 with an auditor-facing message naming the offending file:line.
          PATTERN='uses:\s*(sigstore/cosign-installer|actions/attest-build-provenance|actions/attest-sbom|anchore/sbom-action)@(?![a-f0-9]{40})'
          if grep -rnPE "${PATTERN}" .github/workflows/; then
            echo "ERROR: One or more sigstore-ecosystem / sbom-action uses above lacks a 40-hex SHA pin." >&2
            echo "       v1.6 Phase 23 supply-chain invariant: ALL of" >&2
            echo "         - sigstore/cosign-installer" >&2
            echo "         - actions/attest-build-provenance" >&2
            echo "         - actions/attest-sbom" >&2
            echo "         - anchore/sbom-action" >&2
            echo "       MUST be pinned by 40-hex commit SHA in every workflow." >&2
            echo "       See SECURITY.md § Supply-chain status > Image signatures and" >&2
            echo "       .planning/research/PITFALLS.md §4 for rationale." >&2
            exit 1
          fi
```

**Notes for the planner:**

- The `grep -P` (Perl-compatible regex) is available on `ubuntu-latest` via system `grep` (GNU grep with `-P`). Verified by the existing `crit-01-grep-check` job using the same `-rEn` family of flags ([ci.yml:255](.github/workflows/ci.yml#L255)).
- The `(?![a-f0-9]{40})` negative lookahead is the load-bearing piece — it matches `uses: <action>@<anything-NOT-40-hex>` (so `@v3`, `@main`, `@stable` all match and fail the gate).
- The match output `file:line:line-content` lets the maintainer fix the offending file directly from the CI log.
- Per Phase 22 Plan 22-04's lesson: do NOT use a `POLICY_REF` shell variable for the error trailer. Inline the literal `See SECURITY.md § Supply-chain status` strings in the echoes so any future `grep -c 'Supply-chain'` audit gate counts them at the file level too.

**Alternative location considered: `.github/scripts/sigstore-pin-check.sh` (D-09 alternative).** Rejected for symmetry with the existing `bip322-pin-check` / `crit-01-grep-check` family which live inline in `ci.yml` with no separate script file. If the grep pattern grows complex enough to warrant a dedicated script, that's a future split — not a v1.6 task.

---

## 6. HUMAN-UAT specifics (D-06 Stage 2 — fresh-machine rehearsal recipe)

**The container:** `docker run --rm -it ubuntu:24.04` (matches the Phase 25 reproducibility-verifier pin per Pitfall 7 spirit; avoids `ubuntu:latest` rotation surprises).

**The install steps the operator runs inside the container** — these MUST match verbatim what an external operator following SECURITY.md would run. The SECURITY.md recipe block does NOT show the install steps (assumed prerequisite), so this list is HUMAN-UAT-only; if any step in the install fails, the SECURITY.md recipe must be amended to include an install-prerequisite section.

```bash
# Stage 2 fresh-machine UAT — runs inside `docker run --rm -it ubuntu:24.04`
# Confirms the operator-facing recipes in SECURITY.md are runnable from zero.

# 1. Install prerequisites (curl, gh — both apt-installable on 24.04).
apt-get update -qq
apt-get install -y -qq curl ca-certificates

# 2. Install cosign v2.6.3 (matches what CI installs via cosign-installer).
# The exact URL is the sigstore/cosign release asset for linux-amd64. The
# checksum is verified against the GitHub-published SHA256 file at the same URL.
COSIGN_VERSION="v2.6.3"
COSIGN_URL="https://github.com/sigstore/cosign/releases/download/${COSIGN_VERSION}/cosign-linux-amd64"
COSIGN_SHA_URL="${COSIGN_URL}-keyless.sig"  # cosign self-signs releases — see Pitfall 4 + sigstore release process
curl -sLo /usr/local/bin/cosign "${COSIGN_URL}"
chmod +x /usr/local/bin/cosign
cosign version | grep -q "${COSIGN_VERSION#v}"  # confirm install
# Optional: verify the cosign binary self-signature (skipped in HUMAN-UAT for
# brevity; recommended for security-sensitive operators — `cosign verify-blob
# --bundle ${COSIGN_URL}.bundle /usr/local/bin/cosign`).

# 3. Install gh CLI 2.x for the SLSA + SBOM attestation verifies.
# Per official gh install docs: https://github.com/cli/cli/blob/trunk/docs/install_linux.md
mkdir -p -m 755 /etc/apt/keyrings
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
  | dd of=/etc/apt/keyrings/githubcli-archive-keyring.gpg
chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
  > /etc/apt/sources.list.d/github-cli.list
apt-get update -qq
apt-get install -y -qq gh
gh --version  # confirm install

# 4. Authenticate gh with a no-scope token (read-only — sufficient for
#    `gh attestation verify`). Token from https://github.com/settings/tokens
#    with NO scopes selected.
echo "$GITHUB_PAT" | gh auth login --with-token

# 5. Run the four recipes from SECURITY.md verbatim.
#    Recipe 1 — cosign sig verify on coordinator image (PASS expected)
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/johnzilla/blindjoin-coordinator:1.6.0-rc.0

#    Recipe 2 — SLSA provenance attestation (PASS expected)
gh attestation verify oci://ghcr.io/johnzilla/blindjoin-coordinator:1.6.0-rc.0 \
  --repo johnzilla/blindjoin \
  --predicate-type https://slsa.dev/provenance/v1

#    Recipe 3 — SBOM attestation (PASS expected)
gh attestation verify oci://ghcr.io/johnzilla/blindjoin-coordinator:1.6.0-rc.0 \
  --repo johnzilla/blindjoin \
  --predicate-type https://spdx.dev/Document

#    Recipe 4 — offline bundle save + verify (PASS expected on second verify
#    without network)
cosign save --dir ./blindjoin-coordinator-rc \
  ghcr.io/johnzilla/blindjoin-coordinator:1.6.0-rc.0
# Disconnect network here in a real rehearsal — `iptables -A OUTPUT -j DROP`
# inside the container, or run on an air-gapped machine.
cosign verify --local-image ./blindjoin-coordinator-rc \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'

#    Repeat recipes 1-4 substituting `client` and `liquidity-bot` for
#    `coordinator` in the image name. The --certificate-identity-regexp
#    is identical across all three (same workflow file).

# 6. Negative-test: substitute an image NOT signed by blindjoin and confirm
#    the verify fails. This is the test that proves the regex isn't too wide.
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/distroless/static:latest 2>&1 | grep -q "no matching signatures" \
  && echo "PASS: negative test confirms regex doesn't over-match"
```

**Stage 2 PASS/FAIL log shape** (matches the v1.5 `260531-thw-*` / `260531-ubf-*` SUMMARY.md format):

```markdown
## Stage 2 — Fresh-Machine UAT (cosign verify + gh attestation verify)

**Container:** `docker run --rm -it ubuntu:24.04` (clean — no project state).
**Tag verified:** `v1.6.0-rc.0`
**Date:** YYYY-MM-DD

| Recipe | Image | Result | Notes |
|--------|-------|--------|-------|
| 1. cosign verify | coordinator | PASS / FAIL | (paste JSON exit) |
| 1. cosign verify | client | PASS / FAIL | |
| 1. cosign verify | liquidity-bot | PASS / FAIL | |
| 2. SLSA provenance | coordinator | PASS / FAIL | |
| 2. SLSA provenance | client | PASS / FAIL | |
| 2. SLSA provenance | liquidity-bot | PASS / FAIL | |
| 3. SPDX SBOM | coordinator | PASS / FAIL | |
| 3. SPDX SBOM | client | PASS / FAIL | |
| 3. SPDX SBOM | liquidity-bot | PASS / FAIL | |
| 4. cosign save + offline verify | coordinator | PASS / FAIL | |
| 4. cosign save + offline verify | client | PASS / FAIL | |
| 4. cosign save + offline verify | liquidity-bot | PASS / FAIL | |
| Negative test (distroless/static) | n/a | PASS / FAIL | proves regex not over-wide |

**Verdict:** GO for v1.6.0 / NO-GO (with reason).
```

If ANY row is FAIL, the planner opens a quick task to amend the failing recipe + re-rehearses BEFORE re-tagging `v1.6.0`.

**Stage 1 (pre-merge, workflow_dispatch) — RESEARCH-recommended shape:**

Per CONTEXT.md D-06 Stage 1, the planner chooses between (a) adding a `workflow_dispatch.inputs.dry_run` bypass to the `if: startsWith(github.ref, 'refs/tags/')` gate or (b) accepting that Stage 1 only covers check-job + cosign-installer install rehearsal. **RESEARCH-recommended: (b)** — the marginal benefit of running the sign step against a synthetic test image on a feature branch does NOT justify the security debt of a documented bypass on the production tag gate. Stage 1's value is *"confirms cosign-installer pins resolve and the YAML compiles"*; Stage 2 covers *"the sign + attest steps actually work end-to-end and the verify recipe is correct"*. Both stages have distinct value; trying to make Stage 1 do Stage 2's job is the trade-off CONTEXT.md D-06 names but ultimately rejects.

---

## 7. New pitfalls surfaced during research (not in PITFALLS §1-13)

### 7.1 NEW Pitfall — `attest-sbom` is not a one-action solution (RESEARCH §2.2)

**Why it bites:** the action's name suggests "attest the SBOM" but it actually only attests an EXISTING SBOM. CONTEXT.md misread the README and assumed Syft was bundled. If the planner copies CONTEXT.md verbatim into PLAN.md, the workflow will run with an empty `sbom-path:` and fail.

**Prevention:** always pair `actions/attest-sbom` with `anchore/sbom-action` (Syft) or another SBOM generator. The composition pattern is locked in §3.3 above.

**Lasting risk:** when blindjoin migrates to `actions/attest@v4` (v1.7 carry-forward), the same generator+attest split applies — `actions/attest` does NOT generate SBOMs either. The Syft step remains.

### 7.2 NEW Pitfall — `attestations: write` is undocumented in CONTEXT.md (RESEARCH §2.1)

**Why it bites:** CONTEXT.md D-02 only names `id-token: write`. A planner following D-02 verbatim writes a workflow that PASSES the YAML lint and the cosign-installer install, but fails at the first `actions/attest-build-provenance` step with `403 Forbidden` on the attestations API. The failure message is clearer than the Pitfall-2 Fulcio 400, but it still costs one CI cycle.

**Prevention:** the `permissions:` block always carries `id-token: write` AND `attestations: write` together when any `actions/attest-*` step is present. Comment them as a pair (per §2.1 block).

### 7.3 NEW Pitfall — `cosign sign` and `actions/attest-build-provenance` are NOT interchangeable (RESEARCH §2.4)

**Why it bites:** the two produce different OCI artifacts under different tag conventions. A planner who sees "we already have attest-build-provenance" might skip `cosign sign` thinking it's redundant. But ATTEST-01 specifically requires the `<digest>.sig` registry-tag form, which only `cosign sign` produces. Conversely, `cosign sign` does NOT produce SLSA provenance — that's attest-build-provenance's job.

**Prevention:** the §3.1-§3.4 step list explicitly includes BOTH. The PLAN.md task descriptions should name BOTH as separate steps with their distinct REQ-IDs (ATTEST-01 for cosign sign; ATTEST-02 for attest-build-provenance).

### 7.4 NEW Pitfall — v4 of attest-build-provenance / attest-sbom is a deprecation wrapper (RESEARCH §2.3)

**Why it bites:** GitHub bumped both actions to v4 in early 2025 as wrappers around `actions/attest`. If a planner blindly pins to the latest `@v4` they get the wrapper; the wrapper documentation just says "see actions/attest". Inputs MAY have shifted (e.g., `predicate-type` is now required for non-default attestation types). The v3.X line is the last release with self-contained docs matching CONTEXT.md D-03's wording.

**Prevention:** pin to v3.2.0 (attest-build-provenance) / v2.4.0 (attest-sbom) per §2.3. File a v1.7 carry-forward to migrate to `actions/attest@v4` as a single consolidated step.

### 7.5 NEW Pitfall — `cosign download signature --bundle` does not exist (RESEARCH §3.4)

**Why it bites:** CONTEXT.md D-07 mints a CLI shape that isn't in cosign 2.x. If the planner writes the SECURITY.md recipe verbatim from CONTEXT.md, the operator copy-pastes it and gets `Error: unknown flag: --bundle`. Same class of "documented command doesn't exist" failure that Pitfall 12 (fresh-machine UAT) is designed to catch — but for it to catch this, Stage 2 of the UAT has to actually run the recipe verbatim. The §6 HUMAN-UAT recipe above does exactly that.

**Prevention:** use `cosign save` for the offline-bundle recipe (§3.4 path (i)). Document the alternative `cosign save` flow in SECURITY.md (§4 recipe block, item 4).

### 7.6 NEW Pitfall — Matrix-leg context affects OIDC subject claim, but the locked regex DOES handle it

**Why it bites:** the OIDC subject claim is `https://github.com/<owner>/<repo>/.github/workflows/<workflow>.yml@<ref>`. It does NOT include the matrix leg (`coordinator` vs `client` vs `liquidity-bot`) — the matrix is a runtime expansion, but the workflow file ref is the same for all three legs.

**Verified at [github.blog (Sigstore Cosign Keyless Signing with GitHub Actions OIDC)](https://www.qcecuring.com/blog/sigstore-cosign-keyless-github-actions):** *"the OIDC token includes the repository owner (myorg/myrepo), not just the workflow path."* The matrix leg is a job-level variable; the subject claim is workflow-level.

**Implication for the locked regex `'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*'`:** this matches correctly for ALL THREE matrix legs since the workflow file (`.github/workflows/docker.yml`) and tag ref (`refs/tags/v...`) are identical across them. No regex tweak needed. **CONTEXT.md's locked regex is correct as-stated.**

**Prevention:** keep the regex as locked. The HUMAN-UAT in §6 verifies this empirically by running Recipe 1 against all three images.

### 7.7 NEW Pitfall — `provenance: mode=max` / `sbom: true` on `docker/build-push-action` competes with attest-* actions (RESEARCH §3.1)

**Why it bites:** `docker/build-push-action@v7` has built-in `provenance:` and `sbom:` inputs that produce BuildKit-native attestations stored in the OCI image manifest as `application/vnd.in-toto+json` layers. If a planner adds these AND uses `actions/attest-build-provenance`/`actions/attest-sbom`, the image carries TWO competing attestations — a BuildKit one and a GitHub one. Verifiers may pick the wrong one. This is the exact failure mode PITFALLS Pitfall 5 names ("two paths to SLSA provenance; mixing produces two competing attestations").

**Prevention:** the build-push step's `with:` block does NOT gain `provenance:` or `sbom:` inputs in Phase 23 (or 24 or 25). The locked path is the GitHub-actions one (CONTEXT D-03 + PITFALLS Pitfall 5). §3.1 explicitly says "no changes to `with:` block" — preserve this in PLAN.md.

---

## 8. Open Questions for Planner

**Mostly resolved.** The five surprises documented in §7 (Pitfalls 7.1-7.7) are the load-bearing ones; this section captures the remaining handful that fall short of "blocking research" but warrant a planner decision.

1. **Cosign version pin: v2.5.3 or v2.6.3?** (RESEARCH §2.3 sub-question.)
   - What we know: v2.6.3 is the latest 2.x stable; v2.5.3 is the last 2.5.x; CONTEXT.md D-08 names ">= 2.5, < 3.0" range.
   - What's unclear: whether the planner prefers maximally-current (v2.6.3) vs minimum-of-range (v2.5.3) for operator-side install hand-holding.
   - Recommendation: **v2.6.3 + SECURITY.md range `>= 2.6.3, < 3.0.0`**. Reasoning: bug fixes between v2.5.3 and v2.6.3 are operator-relevant (cert verification fixes per v2.5.3 release notes); the upgrade path is binary-swap; CI side and operator side stay aligned.

2. **`sigstore-pin-check` job placement: end of `ci.yml` (after crit-01-client-grep-check) or grouped with other pin-check family?**
   - What we know: existing `bip322-pin-check` is at [ci.yml:214](.github/workflows/ci.yml#L214); existing `crit-01-grep-check` is at [ci.yml:238](.github/workflows/ci.yml#L238); `crit-01-client-grep-check` is at [ci.yml:265](.github/workflows/ci.yml#L265). The file currently ends at line 290.
   - Recommendation: append at the end of file (after `crit-01-client-grep-check`). Maintains chronological-introduction order; future Phase-25 grep-check jobs append after.

3. **Forward strikethrough in SECURITY.md for Phase 24's release tarball signature line — write now or in Phase 24?**
   - What we know: §4 above proposes writing both strikethroughs (Phase 23's Docker image one + a forward-looking Phase 24 tarball one) in Phase 23's commit, with a comment that Phase 24 will add the anchor.
   - Recommendation: **write only the Phase 23 strikethrough now**. Phase 24 owns its own anchor + strikethrough. Cross-phase forward-references add risk of stale anchors if Phase 24 changes shape. Mirrors Phase 22's actual ship discipline.

4. **`upload-artifact: false` on `anchore/sbom-action` — confirm this is the right default?**
   - What we know: `upload-artifact` defaults to TRUE on `anchore/sbom-action@v0.24.0` — meaning the SBOM file would be uploaded as a workflow artifact even when we just want it for the next step.
   - Recommendation: **explicitly set `false`**. The attestation IS the operator-facing artifact (registry-stored). A workflow-artifact SBOM is short-lived (90 days default) and creates the "which SBOM is canonical?" question the attestation is supposed to answer. The §3.3 block already includes `upload-artifact: false`.

5. **Concurrency / job-output plumbing across the matrix?**
   - What we know: D-01 locks "no job-output plumbing — `outputs.digest` consumed in-context within each matrix leg". Each leg writes its own `sbom.spdx.json` to its own runner FS; no cross-leg sharing needed.
   - Recommendation: confirmed — no plumbing change. The three matrix legs are fully independent; `fail-fast: false` already prevents cascade failure.

---

## 9. Environment Availability

| Dependency | Required By | Available on `ubuntu-24.04` runner | Version | Fallback |
|------------|------------|------------|---------|----------|
| `cosign` binary | sign step (§3.2), HUMAN-UAT Stage 2 | ✗ (not preinstalled) | n/a | Installed by `sigstore/cosign-installer@v3.10.1` (§3.2 + §2.3). Operator side: documented `curl` install in §6. |
| `syft` binary | SBOM generation (§3.3) | ✗ (not preinstalled) | n/a | Installed by `anchore/sbom-action@v0.24.0` (bundled internally). |
| `docker buildx` | build-push step (already used by Phase 22) | ✓ | bundled with runner Docker | — |
| `gh` CLI | HUMAN-UAT Stage 2 attestation-verify recipes | ✓ (preinstalled on `ubuntu-24.04`) | 2.x | apt-installable on Ubuntu 24.04 client side (§6 step 3). |
| `curl` | HUMAN-UAT Stage 2 cosign download | ✓ on runner; apt-installable on `ubuntu:24.04` container | — | — |
| OIDC token (`id-token: write`) | cosign sign, attest-build-provenance, attest-sbom | ✓ (provided by GitHub when permission granted) | n/a | None — gate failure is hard-stop (Pitfall 2). |
| Fulcio + Rekor reachability | cosign sign, cosign verify (online verify) | ✓ (public good services; GitHub runners reach them) | n/a | `cosign verify --insecure-ignore-tlog` for airgapped operators — documented as opt-out in SECURITY.md callouts (Pitfall 3 spirit). |
| ghcr.io reachability + `packages: write` | push-to-registry on sign + attest-* steps | ✓ (already used by Phase 22) | n/a | — |

**Missing dependencies with no fallback:** None. All required tools are either preinstalled, action-installable, or apt-installable on the documented platforms.

**Missing dependencies with fallback:** All four installable tools (cosign, syft, gh, curl) have documented install paths above.

---

## 10. Files to be Modified / Created in Phase 23

| File | Action | Role | Closest analog |
|------|--------|------|----------------|
| `.github/workflows/docker.yml` | MODIFY | workflow — sign + attest steps + permission additions | self (Phase 22 added `read-base-digests` step; Phase 23 adds 5 new steps + 2 permission lines) |
| `.github/workflows/ci.yml` | MODIFY | workflow — new `sigstore-pin-check` job | self ([ci.yml:214 bip322-pin-check](.github/workflows/ci.yml#L214) — exact structural mirror) |
| `SECURITY.md` | MODIFY | docs — full `## Supply-chain status` overview + recipes + callouts rewrite per D-05 | self (Phase 22 added `### Base-image digests (v1.6 onward)` subsection — Phase 23 rewrites the OVERVIEW prose + adds a new `### Image signatures + attestations (v1.6 onward)` subsection ABOVE the Phase 22 subsection) |
| `CONTRIBUTING.md` | OPTIONAL MODIFY | docs — one-line cross-reference to the rewritten SECURITY.md section | self ([CONTRIBUTING.md `## Tagging releases`](CONTRIBUTING.md#tagging-releases) precedent — planner decides if a `### Verifying releases` subsection is warranted; recommended NO — SECURITY.md is the operator-facing surface) |
| `.github/CODEOWNERS` | NO CHANGE | config | Phase 22 created the file; Phase 23 has no new path that requires CODEOWNERS coverage (sigstore action SHA pins live INSIDE `docker.yml` which is already in scope for maintainer review via branch protection) |

**No new files.** Phase 23 deliberately introduces ZERO new composite actions, ZERO new workflows. The 3-leg matrix already deduplicates the sign+attest steps; a composite wrapper would add indirection without saving lines. The `sigstore-pin-check` job lives inside `ci.yml` per D-09 recommendation. The HUMAN-UAT rehearsal log goes into `.planning/quick/<timestamp>-<slug>/` per the v1.5 `260531-thw-*` precedent — that directory is the rehearsal log, not a checked-in code artifact.

---

## 11. Threat-Model Notes (what bypasses the Phase 23 gates)

The Phase 23 sign+attest gates are **defense-in-depth on top of Phase 22's digest gate**. The following bypasses exist; the planner MUST be aware when writing PLAN.md.

| Bypass | Mitigation in Phase 23 | Residual + carry-forward |
|--------|------------------------|--------------------------|
| **Maintainer's GitHub account compromised; attacker triggers a release.** The OIDC subject would match the locked regex (the attacker IS in the `johnzilla/blindjoin` repo's workflow context). | Out of scope for v1.6 — same account-compromise residual SECURITY.md already documents at v1.5 (solo-maintained). Sigstore's transparency log makes the attack PUBLICLY VISIBLE after the fact, which is what Rekor exists for. | Carry-forward to v1.7+ if blindjoin gains additional maintainers — multi-maintainer review on tag pushes would mitigate. |
| **Attacker pushes a forked branch named `refs/tags/v999.0.0` that matches the regex.** The regex spans `refs/tags/v.*`. A malicious fork CANNOT push tags to the canonical `johnzilla/blindjoin` repo (only the maintainer can), so this requires the previous bypass first. | The regex is narrow on the workflow file path (`docker.yml`) AND tag namespace (`refs/tags/v.*`) — attacker would need to control BOTH. | None — accepted residual. The forked-repo subject claim ALSO includes the repo owner; the regex's `https://github.com/johnzilla/blindjoin/.*` prefix means a fork's OIDC subject wouldn't match (per RESEARCH §7.6 + ianlewis.org verification). |
| **SHA-pinned action repo is compromised; pin force-pushed.** Theoretical: an attacker takes over `actions/attest-build-provenance` and force-pushes the SHA we pinned to point to malicious code. | Vanishingly unlikely on a popular action; GitHub itself disallows force-push on action releases. All four sigstore-ecosystem actions are SHA-pinned (D-04) with the `sigstore-pin-check` gate catching future regressions. | None — accepted residual. Re-verification at the SHA level is the cosign installer's job. |
| **Fulcio root key compromise.** Sigstore's trust root is rotated periodically; cosign 2.x ships with a TUF-distributed trust root. If the Fulcio root key is compromised, all keyless signatures everywhere become forgeable. | Out of scope — Sigstore community problem, not blindjoin's. cosign's TUF root distribution handles rotation. | None — accepted residual. The HUMAN-UAT recipe's negative test (§6 step 6) would still detect this if the operator's local cosign TUF cache is up-to-date. |
| **Build-time supply-chain compromise (e.g., compromised Cargo dep).** The Syft SBOM scans the FINAL image filesystem; a compromise that affects the binary but not the manifest WOULD be visible. A compromise that strips its trace from the filesystem (rare; requires fs-level reflection-attack) would not. | The SLSA provenance binds the build to the source commit; if the commit is in git, an auditor can rebuild and diff. Combined with Phase 25 reproducible-build verifier (future), this closes more. | Phase 25 carry-forward — reproducibility verifier catches build-time injection. |
| **The `>` redirect operator in the HUMAN-UAT recipe writes to `/usr/local/bin/cosign` without checksum verification.** The §6 recipe step 2 downloads cosign + chmod + run — does NOT verify the binary against its self-signed Sigstore release. | Pitfall 4 spirit applies; the HUMAN-UAT recipe could be tightened with a `cosign verify-blob --bundle <bundle> /usr/local/bin/cosign` step BEFORE first use. | Recommended carry-forward: add this verification to §6 step 2 in a quick task once the bootstrap-cosign chicken-and-egg is solved (it's not — verifying cosign requires cosign). Acceptable residual for v1.6 ship. |

---

## 12. Sources

### Primary (HIGH confidence — referenced inline above)

- **In-repo** files read in full at research time:
  - `.github/workflows/docker.yml` (full — 127 lines, post-Phase 22)
  - `.github/workflows/release.yml` (full — 108 lines, post-Phase 22)
  - `.github/workflows/ci.yml` (full — 290 lines)
  - `docker/Dockerfile` (full — 72 lines)
  - `docker/digests.txt` (full — 8 lines)
  - `SECURITY.md` (full — 226 lines)
  - `.planning/phases/22-base-image-digest-drift-detection/22-RESEARCH.md` (1010 lines)
  - `.planning/phases/22-base-image-digest-drift-detection/22-PATTERNS.md` (604 lines)
  - `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-CONTEXT.md` (167 lines)
  - `.planning/REQUIREMENTS.md` (79 lines)
  - `.planning/STATE.md` (134 lines)
  - `.planning/research/PITFALLS.md` (188 lines)
  - `.planning/research/SUMMARY.md` (85 lines)
  - `.planning/research/STACK.md` (127 lines)
  - `.planning/research/ARCHITECTURE.md` (188 lines)
  - `.planning/config.json` (confirms `nyquist_validation: false`)

- **External (verified at research time, 2026-06-01):**
  - [sigstore/cosign-installer v3.10.1 release notes](https://github.com/sigstore/cosign-installer/releases/tag/v3.10.1) — SHA `7e8b541eb2e61bf99390e1afd4be13a184e9ebc5`, October 2023, default cosign v2.6.1, v3.X cannot install cosign v3.X
  - [actions/attest-build-provenance releases](https://github.com/actions/attest-build-provenance/releases) — v3.2.0 SHA `96278af6caaf10aea03fd8d33a09a777ca52d62f`, January 2026
  - [actions/attest-sbom v2.4.0 README](https://github.com/actions/attest-sbom/blob/v2.4.0/README.md) — confirms attest-sbom does NOT generate SBOMs (research correction §2.2); v2.4.0 SHA `bd218ad0dbcb3e146bd073d1d9c6d78e08aa8a0b`
  - [anchore/sbom-action releases](https://github.com/anchore/sbom-action/releases) — v0.24.0 SHA `e22c389904149dbc22b58101806040fa8d37a610`, March 2025; bundles Syft; SPDX-JSON default
  - [sigstore/cosign releases](https://github.com/sigstore/cosign/releases) — confirms v2.6.3 (April 2026 latest 2.x), v3.0.6 (April 2026 latest 3.x); cosign 3.0 has shipped
  - [sigstore/cosign download_signature CLI docs](https://github.com/sigstore/cosign/blob/main/doc/cosign_download_signature.md) — confirms NO `--bundle` flag exists (research correction §3.4)
  - [sigstore/cosign sign CLI docs](https://github.com/sigstore/cosign/blob/main/doc/cosign_sign.md) — confirms `--bundle FILE` flag exists on `cosign sign` for local-file output (alternative path in §3.4)
  - [docs.github.com — Using artifact attestations](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds) — confirms `attestations: write` permission requirement (research correction §2.1)
  - [github.blog Supply-chain Artifact Attestations post](https://github.blog/security/supply-chain-security/configure-github-artifact-attestations-for-secure-cloud-native-delivery/) — confirms `attestations: write` rationale

### Secondary (MEDIUM confidence — community/blog sources cross-referenced with above)

- [augmentedmind.de Docker image attestations with GitHub attestations](https://www.augmentedmind.de/2025/03/09/docker-image-attestations-github/) — full canonical YAML example with `attestations: write` + `subject-name`/`subject-digest`/`push-to-registry` pattern; confirms attest-build-provenance stores under `sha256-<digest>` referrer (research correction §2.4)
- [augmentedmind.de Docker image signing with cosign](https://www.augmentedmind.de/2025/03/02/docker-image-signing-with-cosign/) — confirms cosign 2.x sig storage at `sha256-<HEX>.sig` registry tag
- [blog.sigstore.dev cosign verify bundles](https://blog.sigstore.dev/cosign-verify-bundles/) — confirms `cosign verify-blob-attestation --new-bundle-format` is the cosign-side path for verifying GitHub attestations (secondary, behind `gh attestation verify`)
- [some-natalie.dev Verifying Cosign signatures offline](https://some-natalie.dev/blog/cosign-disconnected/) — confirms `cosign save` is the canonical offline-verification path (research correction §3.4 path (i))
- [actions/attest-build-provenance issue #162](https://github.com/actions/attest-build-provenance/issues/162) — confirms `gh attestation verify` is the primary verify path for these attestations; `cosign download attestation` returns 404 against them

### Tertiary (LOW confidence — not relied on for any load-bearing claim)

- None. All load-bearing claims are anchored to either an in-repo file, an official sigstore/github docs source, or a community source cross-verified against an official source.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `actions/attest-sbom@v2.4.0` accepts `subject-name` + `subject-digest` + `sbom-path` + `push-to-registry: true` inputs as documented in its README at the v2.4.0 tag. | §3.3 | If the input names changed in v2.4.0 (unlikely — README in the v2.4.0 tag confirms them), the YAML lints but fails at step run with "unknown input". Fix is a 1-line edit. Risk: very low. |
| A2 | `anchore/sbom-action@v0.24.0`'s `image:` input accepts a digest-pinned reference (`<image>@sha256:<HEX>`), not just a tag (`<image>:<tag>`). | §3.3 | If only tag form is supported, the SBOM scope could drift between Syft scan and cosign sign (we'd be scanning a tag that points to a different digest than what we just pushed). README says "image will be fetched using the Docker daemon if available, which will use any authentication available to the daemon, and if the Docker daemon is not available, the action will retrieve the image directly from the container registry" — implies any pullable form works. Risk: low. |
| A3 | `cosign save` outputs everything needed for `cosign verify --local-image` to verify offline (sig + cert + Rekor inclusion proof + attestations). | §3.4, §6 | If `cosign save` skips the attestations directory (i.e., only saves the image + sig), the offline verify of recipes 2-3 in §6 would fail. Some-natalie blog confirms image+sigs but doesn't explicitly mention attestations. **HUMAN-UAT Stage 2 step 4 explicitly tests this.** If it fails, the operator-facing recipe shifts to "download attestations separately via `gh attestation download`". Risk: medium-low; the rehearsal catches it. |
| A4 | The cosign 2.5.3 → 2.6.3 CLI is backward-compatible (no breaking flag changes between minor versions in the 2.x line). | §2.3, §4 | If a flag we use (`--certificate-identity-regexp`, `--certificate-oidc-issuer`, `--local-image`) shifted between 2.5.x and 2.6.x, the SECURITY.md range would need narrowing. Verified at v2.5.3 release notes that changes are additive features + bug fixes; no rename/remove of the above flags in the changelog. Risk: low. |
| A5 | The `--certificate-identity-regexp` locked in CONTEXT.md correctly matches the OIDC subject claim format that GitHub emits for matrix-job runs against tag refs. | §7.6 | If the matrix-leg context affected the subject claim (it doesn't, per RESEARCH §7.6 verified against multiple sources), the regex would over-match or under-match. **HUMAN-UAT Stage 2 step 6 (negative test) catches over-match; steps 1 across all three images catch under-match.** Risk: low. |
| A6 | `attestations: write` permission is available at the JOB level (not workflow-only). | §2.1 | If GitHub only honors `attestations: write` at workflow level, the per-job grant would silently no-op. The github.blog source's "basic example" shows it at job level. Risk: very low. |
| A7 | `gh attestation verify oci://...` is the primary verify path for GitHub-attested images (not cosign-side). | §4, §6 | If `gh attestation verify` doesn't exist in `gh` 2.x (it was added in gh 2.42 per release notes), the SECURITY.md recipe would point operators at a missing command. `gh --version` check in §6 step 3 catches this. Risk: very low. |

**The 7 assumptions above are low-risk and do not require user confirmation before plan execution.** All are operational rather than design-affecting. A3 is the most likely to surface in HUMAN-UAT; the rehearsal stage is designed to catch it.

---

## Metadata

**Confidence breakdown:**
- Sign + attest YAML shape: **HIGH** — anchored to multiple canonical community examples (augmentedmind.de, actions/attest README, github.blog).
- SHA pins: **HIGH** — resolved against GitHub Releases pages at research time; all four actions have stable release histories with semver-tagged commits.
- Permission scopes (incl. `attestations: write` correction): **HIGH** — verified against docs.github.com primary source plus github.blog secondary.
- SBOM generation correction (anchore/sbom-action required): **HIGH** — verified against actions/attest-sbom v2.4.0 README directly.
- `.bundle` distribution correction (cosign save vs nonexistent `cosign download signature --bundle`): **HIGH** — verified against sigstore/cosign source docs at github.com/sigstore/cosign/blob/main/doc/cosign_download_signature.md.
- `--certificate-identity-regexp` matrix-leg correctness: **HIGH** — verified against ianlewis.org + qcecuring.com + augmentedmind.de cross-references.
- Operator-side cosign install recipe (§6): **MEDIUM** — install URL is correct per sigstore release pattern, but the negative test (step 6) and the cosign-verify-cosign self-bootstrapping note are best-effort; HUMAN-UAT Stage 2 is the empirical confirmation.
- Threat-model bypasses: **HIGH** — enumerated against PITFALLS, ianlewis.org, and the Sigstore bundle/trust-root architecture.

**Research date:** 2026-06-01
**Valid until:** 2026-07-01 (30 days for the sigstore ecosystem; re-verify SHA pins + cosign-installer / attest-* major versions if any of those four upstream repos ship a new major version in that window — particularly relevant given attest-build-provenance v3 → v4 is a deprecation transition already in flight).

---

## RESEARCH COMPLETE
