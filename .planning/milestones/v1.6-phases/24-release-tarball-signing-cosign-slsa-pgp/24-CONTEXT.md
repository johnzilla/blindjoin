# Phase 24: Release Tarball Signing (cosign + SLSA + PGP) - Context

**Gathered:** 2026-06-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 24 turns every `blindjoin-linux-amd64.tar.gz` GitHub Release asset into a cryptographically verifiable artifact via TWO independent paths: (1) a cosign OIDC keyless blob signature + SLSA v1.0 provenance via the same `actions/attest-build-provenance` machinery as Phase 23, scoped to `release.yml@refs/tags/v.*`; and (2) a detached PGP signature produced locally by the maintainer on a YubiKey-held ed25519 key, uploaded after the workflow finishes and the operator flips the GitHub Release out of draft. SECURITY.md grows a second fenced bash recipe block for tarball verification (`cosign verify-blob --bundle` + `cosign verify-attestation --bundle ... --type slsaprovenance` + `gpg --verify`), appended additively under the Phase 23 D-05 skeleton. The maintainer-side procedure (cut tag → wait for CI → download artifact → `gpg --detach-sign` → `gh release upload` → flip out of draft) lives in a new `docs/RELEASING.md`.

What this phase does NOT do: image signing (Phase 23 already shipped — Phase 24 reuses the cosign-installer SHA pin + identity-regexp shape + `id-token: write` Pitfall 2 discipline), reproducible-build recipe / monthly verifier (Phase 25 — Phase 24 just signs whatever bytes `cargo build --release` produces today; Phase 25 is what makes those bytes byte-equal across rebuilds). It also does NOT introduce a new sigstore-pin grep gate — the Phase 23 `sigstore-pin-check` job (D-04) already covers `sigstore/cosign-installer` + `actions/attest-build-provenance` + `actions/attest-sbom`; Phase 24 adds zero new sigstore actions (provenance reuses the Phase 23 action). It also does NOT add a `.sbom` Release asset for tarballs — SBOM is image-side per ATTEST-03, not in scope for SIGN-0*.

</domain>

<decisions>
## Implementation Decisions

### Cosign blob signing in `release.yml`

- **D-01: Inline in the existing `build` job AFTER `Package` step.** Per Phase 23 D-01 placement pattern (sign/attest steps land inside the same publish job that produced the artifact). The new steps go between [release.yml:91-98 `Package`](.github/workflows/release.yml#L91) and [release.yml:100-107 `Upload to GitHub Releases`](.github/workflows/release.yml#L100): (a) `sigstore/cosign-installer@<sha>` setup, (b) `cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz`, (c) `actions/attest-build-provenance@<sha>` with `subject-path: blindjoin-linux-amd64.tar.gz` + capture the bundle output path. The existing `softprops/action-gh-release` `files:` list ([release.yml:103-105](.github/workflows/release.yml#L103)) grows from 2 to 4 entries (tarball, .sha256, .bundle, .sigstore). Trade-off accepted: a sign-step failure aborts the upload (acceptable — re-pushing the tag is the recovery, and softprops idempotently uploads on retry). Rationale: minimal YAML diff, no new job, mirrors Phase 23's simplicity preference and Phase 22's inline-composite-action style.

- **D-02: `id-token: write` added at JOB level on `release.yml`'s `build` job, NOT workflow-level.** Per PITFALLS Pitfall 2 + Phase 23 D-02: narrower scope strictly better. The workflow-level `permissions:` block at [release.yml:28-29](.github/workflows/release.yml#L28) stays at `contents: write` (needed for `gh release upload`). The `build` job grows an explicit `permissions: { contents: write, id-token: write, attestations: write }` block — `attestations: write` is required by `actions/attest-build-provenance` to push to the GitHub Attestations API. The new permission additions get the auditor-grepable comment style established by Phase 22 + Phase 23: a comment block above naming the additions and listing deliberately-omitted scopes (`packages` / `pull-requests` / `pages`) so future `! grep -q '<scope>:'` audits are satisfied at the file level. The `check` job's implicit workflow-default permission stays untouched.

- **D-03: Cosign signature distributed as `.bundle` (single file).** ATTEST-04 / SIGN-01 roadmap allows either `.bundle` or discrete `.sig` + `.crt`. Picked `.bundle` to mirror Phase 23's image-side distribution: operators learn ONE verification recipe shape and apply it to both image and tarball artifacts (just swap `cosign verify --bundle` for `cosign verify-blob --bundle`). Rejected: discrete `.sig` + `.crt` (two assets per release, fiddlier operator command, no benefit at the operator-side pin range `≥ 2.5, < 3.0` which fully supports `--bundle`). Rejected: both formats (clutter; same security; deferred to v1.7 only if an operator who can't use `--bundle` materializes).

### SLSA provenance distribution

- **D-04: Both GitHub Attestations API AND `.sigstore` bundle as Release asset.** `actions/attest-build-provenance` invoked with `subject-path: blindjoin-linux-amd64.tar.gz` pushes the in-toto provenance to GitHub's Attestations API (verifiable via `gh attestation verify blindjoin-linux-amd64.tar.gz --owner <owner>`) AND emits a `bundle-path` output pointing to a `.sigstore` bundle file on disk. That file gets piped into the same `softprops/action-gh-release` step's `files:` list. Operators get TWO verification paths: (a) `gh attestation verify <tarball>` for github.com-reachable environments (no separate download), (b) `cosign verify-attestation --bundle blindjoin-linux-amd64.tar.gz.sigstore --type slsaprovenance --certificate-identity-regexp '...release.yml@refs/tags/v.*' --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' <tarball>` for fully-offline verification after one-time TUF root seeding. Both paths are documented in SECURITY.md (D-08). Rejected: API-only (air-gapped operators can verify cosign sig + PGP sig but not provenance — gap defeats the "tarball has the same attestation surface as images" goal). Rejected: bundle-only (loses the cleanest operator UX path Phase 24 can offer that Phase 23 cannot — images go through registries, not GitHub Releases).

- **D-05: `.sigstore` bundle filename = `blindjoin-linux-amd64.tar.gz.sigstore`.** Mirrors the existing `.sha256` sibling-suffix convention at [release.yml:98](.github/workflows/release.yml#L98). Self-documenting; `cosign verify-attestation --bundle blindjoin-linux-amd64.tar.gz.sigstore ...` reads naturally. Disambiguates from the cosign signature `.bundle` (different suffix → no operator confusion). Planner: confirm `actions/attest-build-provenance` `bundle-path` output is renameable / writable to a chosen path (the action documents an `output-name` input — wire it to this filename so the upload step finds the file deterministically).

### Maintainer-held PGP path

- **D-06: Maintainer-local sign + upload.** PGP signing is OUT of `release.yml` entirely. CI builds the tarball, signs it with cosign, attests provenance, uploads the Release as `draft: true`, and STOPS. The maintainer (post-tag, post-CI-green) runs the documented procedure: `gh release download v1.6.0 -p '*.tar.gz' --dir /tmp/blindjoin-release` → `gpg --detach-sign --armor --local-user <fingerprint> /tmp/blindjoin-release/blindjoin-linux-amd64.tar.gz` → `gh release upload v1.6.0 /tmp/blindjoin-release/blindjoin-linux-amd64.tar.gz.asc` → `gh release edit v1.6.0 --draft=false`. Rationale: the entire SIGN-03 "non-OIDC alternative path" rationale is that operators who don't trust Sigstore/GitHub can still verify. Putting the PGP private key in GitHub Secrets re-introduces GitHub as a trusted party for the PGP path — defeats the rationale. Trade-off accepted: a ~60-second manual step per release, in a solo-maintainer project where tagging is already a manual decision. Rejected: CI-managed (GH Secret holds passphrase + private key — defeats SIGN-03 rationale; one platform compromise breaks both paths). Rejected: hybrid signing-subkey in CI (best long-term security but adds subkey lifecycle, revocation procedure, monitoring — overkill for solo project at v1.6.0 maturity; revisit at v1.8+ if a co-maintainer onboards).

- **D-07: GitHub Releases ship as `draft: true` until the maintainer flips them after `.asc` upload.** `softprops/action-gh-release` step grows `draft: true`. Operators who hit the Releases page never see a release without the PGP signature attached (consistent verification UX — every published release has all 5 assets, or it's a draft). The `gh release edit v1.6.0 --draft=false` flip is the last step of the maintainer-side procedure in `docs/RELEASING.md`. Rejected: publish-immediately + `.asc` arrives within minutes (races with operator who pulls instantly; visible "no PGP" window). Rejected: `prerelease: true` until signed (operators see a "this is staged" badge they don't need; SemVer pre-releases like `-rc.0` already use prerelease, conflating signals).

### PGP key custody + generation

- **D-08: Fresh ed25519 key generated on / transferred to a YubiKey 5 OpenPGP applet.** Private key material never resides on a general-purpose computer that runs untrusted code. `gpg --detach-sign` invocations prompt for YubiKey touch. Algorithm: ed25519 for the signing key (modern, small, fast verify; ed25519 is the cosign / sigstore default and matches the project's secp256k1/ed25519 cryptographic vocabulary). User-ID: `blindjoin maintainer <johnturner@gmail.com>` (project-scoped UID — when blindjoin's key needs revocation, no personal-key consumer is affected; if/when a co-maintainer joins, key handover is a fresh project key, not a personal key handover). The key has NO encryption subkey (signing-only; PGP is not used for encryption in this project). Expiry: 2 years from creation, with documented renewal procedure in `docs/RELEASING.md`. Revocation certificate generated at key creation, stored offline (USB drive + paper backup), procedure documented. Rejected: encrypted keyfile in password manager (good but YubiKey is strictly better; the project is supply-chain-signing for a privacy tool, the operational-security gap from software key custody is the wrong corner to cut). Rejected: existing personal PGP key (entangles personal identity with project signing identity; revocation cascades; blast radius wrong).

- **D-09: Public key committed at `docs/pgp/<FULL-40-CHAR-FINGERPRINT>.asc`.** Filename is the unambiguous identity. Operators verify the file's identity by computing `gpg --with-colons --import-options show-only --import docs/pgp/<...>.asc` and comparing the printed fingerprint to the filename — no SECURITY.md prose required to anchor the binding. When the key rotates (every 2 years per D-08), a new file is added beside the old one — old fingerprints stay in the repo as historical record (operators verifying old releases can still locate the right key). SECURITY.md's prose names the CURRENT fingerprint with a stable anchor (`<a id="pgp-current"></a>`). Rejected: `docs/pgp/maintainer.asc` stable filename (key rotation invisible in git diff of the asc file; loses historical-key story). Rejected: short-ID (16-char) filename (collision-attackable for keyserver lookups; full fingerprint is the modern standard).

- **D-10: Public key published to BOTH WKD (on `<owner>.github.io`) AND `keys.openpgp.org`.** Roadmap SC#3 explicitly mandates `keys.openpgp.org` upload — kept verbatim. WKD added on top because it's the cleanest operator UX (`gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com` resolves the key automatically without keyserver flags). Two channels reference the same public key file in `docs/pgp/`; publishing to both costs one `gpg --send-keys --keyserver hkps://keys.openpgp.org <fingerprint>` plus one push to a `<owner>.github.io` repo populating `.well-known/openpgpkey/openpgpkey-mailbox.example.org/hu/...`. The WKD setup is one-time and lives in `docs/RELEASING.md`. Rejected: keys.openpgp.org only (modern operators expect WKD-by-email; offering WKD is low-cost). Rejected: SKS-style keyservers (`keyserver.ubuntu.com` etc — signature poisoning risk, low marginal value).

### Documentation split

- **D-11: New `docs/RELEASING.md` owns the maintainer-side release procedure.** No such file exists today. It births in Phase 24 with the full procedure: tag-cutting, `gh workflow run` rehearsal, post-CI download + PGP sign + upload + draft-flip, key rotation cadence + procedure, revocation procedure, WKD publish refresh, `keys.openpgp.org` refresh. SECURITY.md stays operator-facing (verify recipes only). `CONTRIBUTING.md` gets a one-line cross-reference to `docs/RELEASING.md` for any contributor who needs to understand the release cycle. Rejected: appending the procedure to `CONTRIBUTING.md` (mixes maintainer ops with contributor onboarding). Rejected: appending to `SECURITY.md` (mixes audiences — operators want verify, maintainers want sign).

- **D-12: SECURITY.md `## Supply-chain status` grows a second fenced bash recipes block for tarball verification.** Follows Phase 23 D-05's additive skeleton — the existing `### Image signatures + attestations (v1.6 onward)` subsection stays untouched; a new `### Release tarball signatures + provenance (v1.6 onward)` subsection appends below it with: (a) prose intro (3 sentences: what's signed, what's attested, what the PGP alternative path is for); (b) fenced bash block with TWO `cosign verify-blob --bundle` invocations (one with the locked `--certificate-identity-regexp '...release.yml@refs/tags/v.*'` — different workflow file from Phase 23's `docker.yml`), `cosign verify-attestation --bundle ...sigstore --type slsaprovenance ...`, `gh attestation verify ...`, and `gpg --verify ...asc ...tar.gz`; (c) `> Note` callout block citing Pitfall 13 (cosign 3.0 CLI drift — already in Phase 23's section, but a 1-liner cross-ref keeps tarball section self-contained) + a new callout naming the maintainer fingerprint with an HTML anchor (D-09 anchor). Note explicit prose: "EITHER cosign OR PGP verification is sufficient — they're alternative paths, not both required."

### Claude's Discretion (planner figures these out, guided by research + this CONTEXT)

- **D-13: SHA pins for new `uses:` lines.** The new `sigstore/cosign-installer@<sha>` step in `release.yml` reuses the exact SHA Phase 23 pinned in `docker.yml`. Same for `actions/attest-build-provenance` (same Phase 23 SHA — single source of truth). Planner: cross-reference `.github/workflows/docker.yml` for the canonical pins at planning time; if Phase 23 pinned a SHA that has since had a security advisory, planner reopens the choice as a discussion item. Comment style: `@<40-hex> # vX.Y.Z` trailing comment (project pattern at every `uses:`).

- **D-14: `actions/attest-build-provenance` output wiring for the `.sigstore` filename.** The action accepts an `output-name` (or equivalent) input to control the bundle file path. Planner: confirm exact input name against the SHA-pinned action version's `action.yml`; wire it to `blindjoin-linux-amd64.tar.gz.sigstore` so the downstream `softprops/action-gh-release` step finds the file at a deterministic path. If the input doesn't exist, fall back to reading the `bundle-path` output and `mv`-ing into place — but the cleaner solution is the input.

- **D-15: `release.yml` `softprops/action-gh-release` files list ordering.** Five assets per release: the tarball, `.sha256`, `.bundle`, `.sigstore`, `.asc` (the `.asc` arrives post-CI; CI uploads only the first four). Order in the `files:` list: tarball, .sha256, .bundle, .sigstore — semantic grouping (artifact, integrity, signature, provenance). Planner: minor; pick what reads cleanest.

- **D-16: `gh attestation verify` command shape for SECURITY.md.** Confirm against current `gh` CLI version (likely `2.50+`) — `gh attestation verify <file> --owner <owner>` is the modern shape; older `gh` versions may use `--repo`. Document the modern shape with a footnote on the `gh` version requirement. Planner: spot-check at planning time.

- **D-17: Key-rotation cadence + procedure prose in `docs/RELEASING.md`.** D-08 sets 2-year expiry. The renewal procedure (generate new key on YubiKey, sign it with old key, commit new `.asc`, update SECURITY.md anchor, publish to WKD + keys.openpgp.org, keep old key file in repo as historical) lives in `docs/RELEASING.md`. Planner: write this as a 5-step numbered procedure with `gpg` command examples; do NOT execute it now (that's the maintainer's actual key-rotation action, not a Phase 24 deliverable).

- **D-18: `release.yml` upload step's draft mode and the `--draft=false` flip CLI in `docs/RELEASING.md`.** `softprops/action-gh-release` gets `draft: true`. The maintainer-side flip is `gh release edit v1.6.0 --draft=false`. Planner: confirm softprops supports `draft: true` at the SHA-pinned version (`@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`) — current versions do, but verify. Also planner-discretion: whether `docs/RELEASING.md` documents `--draft=false` flip in the SAME step as `.asc` upload or as a separate paragraph (small UX choice).

- **D-19: WKD setup steps in `docs/RELEASING.md`.** WKD requires a `.well-known/openpgpkey/...` directory tree under `<owner>.github.io`. Planner: document the directory structure, the `gpg-wks-client --print-wkd-hash` helper for the email-mailbox-hash filename, and whether the `<owner>.github.io` repo already exists or needs to be created. If `<owner>.github.io` doesn't exist, this is a one-time maintainer setup task documented (with `gh repo create` instructions) but the actual repo creation is the maintainer's action, not a Phase 24 commit.

- **D-20: Cross-references between CONTRIBUTING.md and docs/RELEASING.md.** A one-line cross-ref in CONTRIBUTING.md is enough. Planner: pick the natural insertion point (probably under the v1.4 tagging guidance section that already exists).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase contract (locked WHAT)
- `.planning/REQUIREMENTS.md` §Category 1 — SIGN-01, SIGN-02, SIGN-03 verbatim text. "cosign blob signature uploaded to the same GitHub Release (`.bundle` format, or discrete `.sig` + `.crt`)" + "SLSA v1.0 provenance attestation via the same `actions/attest-build-provenance` machinery as ATTEST-02" + "detached PGP signature alternative path; maintainer-held PGP key; exported public key committed to the repo + uploaded to keys.openpgp.org; signing key fingerprint documented in `SECURITY.md`" are non-negotiable.
- `.planning/ROADMAP.md` §Phase 24 — 4 numbered Success Criteria. SC#1 names `release.yml@refs/tags/v.*` identity-regexp shape; SC#3 names `docs/pgp/<fingerprint>.asc` + `keys.openpgp.org` upload + `gpg --verify` recipe; SC#4 names the side-by-side cosign + PGP recipe layout in SECURITY.md.

### Threat-model + design context (Pitfalls Phase 24 inherits)
- `.planning/research/PITFALLS.md` §Pitfall 1 — `--certificate-identity-regexp` shape. Phase 24's regex `https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*` — `release.yml` substitution from Phase 23's `docker.yml`. Same rationale (spans pre-release tags, future minor/major bumps).
- `.planning/research/PITFALLS.md` §Pitfall 2 — `id-token: write` at JOB level (D-02), not workflow-level. Opaque Fulcio 400 failure mode if missed.
- `.planning/research/PITFALLS.md` §Pitfall 3 — Rekor transparency log mandatory by default; no `--no-tlog-upload` in the `cosign sign-blob` invocation.
- `.planning/research/PITFALLS.md` §Pitfall 4 — SHA-pin discipline. Phase 23's `sigstore-pin-check` job covers `release.yml` too (it greps under `.github/workflows/`); no new gate needed.
- `.planning/research/PITFALLS.md` §Pitfall 5 — `actions/attest-build-provenance` chosen, not `slsa-framework/slsa-github-generator`. Phase 24 reuses Phase 23's choice.
- `.planning/research/PITFALLS.md` §Pitfall 12 — Fresh-machine UAT every documented command. For Phase 24: deferred to the first `v1.6.0-rc.0` tag push, matching the Phase 23 closure pattern (no HUMAN-UAT scaffold plan).
- `.planning/research/PITFALLS.md` §Pitfall 13 — cosign 3.0 CLI flag drift. Phase 24 inherits Phase 23's SECURITY.md callout; tarball recipes block adds a 1-line cross-reference rather than duplicating.
- `.planning/research/SUMMARY.md` — phase mapping (Phase 23 → Phase 24 → Phase 25) + the operator-facing verify command shape.
- `.planning/research/ARCHITECTURE.md` — ordering rationale (Phase 24 reuses Phase 23's `id-token: write` discipline; introduces nothing structurally new beyond `actions/attest-build-provenance` with `subject-path` for blob artifacts).
- `.planning/research/STACK.md` — cosign version range (`≥ 2.5, < 3.0`), `sigstore/cosign-installer` version, `actions/attest-build-provenance` version. All inherited from Phase 23.

### Predecessor phase patterns (this phase MUST mirror these)
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-CONTEXT.md` — D-01 (inline-in-existing-job), D-02 (job-level `id-token: write` + comments-as-contract), D-04 (sigstore-pin-check gate already covers `release.yml`), D-05 (SECURITY.md prose+recipes+callouts skeleton — Phase 24 APPENDS), D-08 (cosign version pin `≥ 2.5, < 3.0` — operator-side doc already says this), D-10 (SHA pin reuse for sigstore actions).
- `.planning/phases/22-base-image-digest-drift-detection/22-CONTEXT.md` — comments-as-contract style; auditor-grepable "deliberately-omitted-scopes" pattern that applies to Phase 24's permissions block (omit `packages`, `pull-requests`, `pages`).

### Existing pin discipline + integration surface
- `.github/workflows/release.yml` — `build` job ([release.yml:60](.github/workflows/release.yml#L60)), `if: startsWith(github.ref, 'refs/tags/')` gate ([release.yml:66](.github/workflows/release.yml#L66)), `Package` step ([release.yml:91-98](.github/workflows/release.yml#L91)), `Upload to GitHub Releases` step ([release.yml:100-107](.github/workflows/release.yml#L100)). Workflow-level `permissions: { contents: write }` at [release.yml:28-29](.github/workflows/release.yml#L28); `build` job will grow an explicit `permissions:` block. `softprops/action-gh-release@de2c0eb...` already SHA-pinned at [release.yml:101](.github/workflows/release.yml#L101).
- `.github/workflows/docker.yml` — Phase 23's `docker` job is the structural template for the new steps in `release.yml`'s `build` job. The cosign-installer step, the `cosign sign` step shape, the `actions/attest-build-provenance` step shape (modulo `subject-path` for blob vs `subject-name + subject-digest` for image) are all transplantable.
- `.github/workflows/ci.yml` — `sigstore-pin-check` job from Phase 23 D-04 already greps `release.yml` for sigstore actions; no new gate needed.

### Policy + operator-facing docs (D-11 + D-12 land here)
- `SECURITY.md` `## Supply-chain status` — append a new `### Release tarball signatures + provenance (v1.6 onward)` subsection BELOW the existing `### Image signatures + attestations (v1.6 onward)` subsection. The Phase 23 D-05 skeleton (prose intro + fenced bash recipes + `> Note` callouts) is the pattern; do not restructure the existing image subsection.
- `docs/RELEASING.md` — NEW FILE born in Phase 24. Maintainer-side procedure for cutting tags, post-CI PGP signing, draft-flip, key rotation, key revocation, WKD/keys.openpgp.org publish refresh. Documents D-06 through D-10's procedural surface.
- `docs/pgp/<FULL-FINGERPRINT>.asc` — NEW FILE (or files, on rotation). The maintainer's blindjoin-scoped ed25519 public key, exported with `gpg --export --armor <fingerprint> > docs/pgp/<fingerprint>.asc`. Generated on YubiKey per D-08.
- `CONTRIBUTING.md` — one-line cross-reference to `docs/RELEASING.md` (D-11). Planner picks insertion point near the existing v1.4 tagging guidance.
- `docs/AUDIT-CHARTER.md` (v1.5 charter) — supply-chain policy language style. Cross-reference target if the new SECURITY.md subsection needs to cite the charter for threat-model context.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`release.yml` `build` job** ([release.yml:60-107](.github/workflows/release.yml#L60)) — single-job artifact builder. Already tag-gated, already has `actions/checkout` + `dtolnay/rust-toolchain` + `Swatinem/rust-cache` + Phase 22 `read-base-digests` + `softprops/action-gh-release`. New sign + attest steps land BEFORE the upload step; the upload step's `files:` list grows from 2 entries to 4 (CI-uploaded) entries.
- **`softprops/action-gh-release@de2c0eb...`** ([release.yml:101](.github/workflows/release.yml#L101)) — already SHA-pinned. Adding `draft: true` per D-07 is a one-line `with:` addition. The `files:` list grows; the existing `GITHUB_TOKEN` env wiring stays.
- **Phase 23 sigstore SHA pins (canonical source)** — `docker.yml` is the source-of-truth for `sigstore/cosign-installer@<sha>` and `actions/attest-build-provenance@<sha>` versions; `release.yml` adopts the same SHAs. Single source of truth; if either action rotates, both workflows update together. The Phase 23 `sigstore-pin-check` job already covers both files.
- **`workflow_dispatch` rehearsal harness** ([release.yml:26](.github/workflows/release.yml#L26)) — existing dispatch path; runs `check` job and stops short of `build` (tag-gate). Phase 24 doesn't change this. A pre-merge `gh workflow run release.yml --ref <branch>` confirms YAML compiles and `id-token: write` resolves without trying to push attestations from a non-tag ref (which would fail the identity-regexp at verify time anyway).

### Established Patterns
- **Two-tier `check` + `build` gate** — `release.yml` mirrors `docker.yml`'s shape. Phase 24 does NOT add a new job; sign + attest land in the existing `build` job. The `check` job is untouched.
- **`if: startsWith(github.ref, 'refs/tags/')` + workflow-default permissions** — production-only gate at [release.yml:66](.github/workflows/release.yml#L66). All sign/attest emission happens only on real tag pushes. Pre-merge `workflow_dispatch` covers the `check` job only.
- **Comments-as-contract above structural blocks** — every workflow file has prose comments above `env:` / `on:` / `permissions:` / `jobs:` (see [release.yml:3-15](.github/workflows/release.yml#L3), [release.yml:19-26](.github/workflows/release.yml#L19), [release.yml:64-66](.github/workflows/release.yml#L64)). New comment lines above the `id-token: write` permission addition (D-02) and above the new sign/attest steps must follow this style. Auditor-grepable "deliberately-omitted-scopes" pattern from Phase 22 Plan 22-04 applies.
- **SHA-pin trailing-comment style** — `@<40-hex> # vX.Y.Z`. New `sigstore/cosign-installer` + `actions/attest-build-provenance` lines MUST follow. Phase 23 sigstore-pin-check enforces.

### Integration Points
- **`release.yml` `build` job permissions block** — explicit block to add: `permissions: { contents: write, id-token: write, attestations: write }`. Pitfall-2 citing comment block above.
- **`release.yml` `build` job steps** (between [release.yml:98](.github/workflows/release.yml#L98) `Package` and [release.yml:100](.github/workflows/release.yml#L100) `Upload to GitHub Releases`) — append in order:
  1. `sigstore/cosign-installer@<sha>` (cosign-release: same version Phase 23 pinned, likely `v2.5.X`).
  2. `cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz` (produces the `.bundle` Release asset per D-03).
  3. `actions/attest-build-provenance@<sha>` with `subject-path: blindjoin-linux-amd64.tar.gz` + (planner-discretion D-14) wire bundle output filename to `blindjoin-linux-amd64.tar.gz.sigstore`.
- **`release.yml` `Upload to GitHub Releases` step** — grows: (a) `draft: true` per D-07, (b) `files:` list expands to 4 entries: `blindjoin-linux-amd64.tar.gz`, `blindjoin-linux-amd64.tar.gz.sha256`, `blindjoin-linux-amd64.tar.gz.bundle`, `blindjoin-linux-amd64.tar.gz.sigstore`. The `.asc` is NOT in this list — maintainer uploads it post-CI.
- **`SECURITY.md`** — append `### Release tarball signatures + provenance (v1.6 onward)` subsection under `## Supply-chain status`. Existing `### Image signatures + attestations (v1.6 onward)` subsection (Phase 23) stays untouched.
- **`docs/RELEASING.md`** — NEW FILE; full maintainer-side procedure.
- **`docs/pgp/<fingerprint>.asc`** — NEW FILE; armored ed25519 public key.
- **`CONTRIBUTING.md`** — one-line cross-reference addition to `docs/RELEASING.md`.

</code_context>

<specifics>
## Specific Ideas

- **`cosign sign-blob` invocation shape (D-01 step).** RECOMMENDED:
  ```bash
  cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz
  ```
  `--yes` skips the interactive confirmation (required in CI). `--bundle <output>` writes the sig+cert+Rekor-proof bundle to a file (rather than printing sig+cert to stdout). No `--no-tlog-upload` (Pitfall 3 — Rekor is mandatory).
- **`actions/attest-build-provenance` invocation shape (D-01 step).** RECOMMENDED:
  ```yaml
  - uses: actions/attest-build-provenance@<sha> # v<version>
    with:
      subject-path: blindjoin-linux-amd64.tar.gz
      # planner: confirm output-name / bundle-path input against pinned version
  ```
- **Operator-facing cosign verify-blob recipe shape (D-12 SECURITY.md block).** RECOMMENDED:
  ```bash
  cosign verify-blob \
    --bundle blindjoin-linux-amd64.tar.gz.bundle \
    --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
    --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
    blindjoin-linux-amd64.tar.gz
  ```
  Identity-regexp is `release\.yml` (NOT `docker\.yml` like Phase 23). Issuer is identical. `--bundle` file extension is `.bundle`.
- **Operator-facing SLSA verify recipe shape (D-12 SECURITY.md block).** TWO commands documented side-by-side:
  ```bash
  # Path A: GitHub Attestations API (requires github.com reachable)
  gh attestation verify blindjoin-linux-amd64.tar.gz --owner johnzilla
  # Path B: offline cosign-based verify
  cosign verify-attestation \
    --bundle blindjoin-linux-amd64.tar.gz.sigstore \
    --type slsaprovenance \
    --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
    --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
    blindjoin-linux-amd64.tar.gz
  ```
- **Operator-facing PGP verify recipe shape (D-12 SECURITY.md block).** RECOMMENDED:
  ```bash
  # One-time key fetch via WKD (or paste from keys.openpgp.org)
  gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com
  # Verify
  gpg --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz
  ```
  Fallback fetch via `gpg --keyserver hkps://keys.openpgp.org --recv-keys <fingerprint>`.
- **Maintainer-facing release procedure prose (D-11 docs/RELEASING.md).** RECOMMENDED 5-step skeleton:
  1. Run `git tag -s v1.6.0 -m "v1.6.0"` then `git push --tags`.
  2. Watch `release.yml` in Actions tab until green; CI creates the draft Release with 4 assets.
  3. `gh release download v1.6.0 -p 'blindjoin-linux-amd64.tar.gz' --dir /tmp/blindjoin-release`.
  4. `cd /tmp/blindjoin-release && gpg --detach-sign --armor --local-user <fingerprint> blindjoin-linux-amd64.tar.gz` (YubiKey touch).
  5. `gh release upload v1.6.0 blindjoin-linux-amd64.tar.gz.asc && gh release edit v1.6.0 --draft=false`.
  Plus pre-flight checklist (cosign verify the freshly-published artifacts BEFORE flipping draft → published; if cosign-verify fails, do NOT flip; delete the release + re-tag).
- **`id-token: write` comment wording.** Suggested: `# id-token: write — cosign-blob OIDC keyless signing requires a Fulcio-issued cert; without this, sign-blob fails with opaque "fulcio: 400 Bad Request". See PITFALLS Pitfall 2 + Phase 23 D-02.`
- **`attestations: write` comment wording.** Suggested: `# attestations: write — actions/attest-build-provenance pushes the in-toto provenance to the GitHub Attestations API; gh attestation verify reads from that API.`
- **PGP key User-ID format.** Suggested: `blindjoin maintainer <johnturner@gmail.com>` — UID names the role + scope + contact mailbox. Fingerprint binds to identity; UID binds to project role.

</specifics>

<deferred>
## Deferred Ideas

- **CI-managed PGP signing (D-06 alternative)** — rejected because it defeats the SIGN-03 "non-OIDC alternative path" rationale (GitHub becomes a trusted party for the PGP path). Reconsider only if a co-maintainer onboards and the manual-sign procedure becomes a bottleneck. v1.8+ at earliest.
- **Hybrid signing-subkey in CI (D-06 third option)** — best long-term security but adds subkey lifecycle, revocation procedure, monitoring. Overkill for solo-maintainer at v1.6.0. Revisit only if the project gains a co-maintainer.
- **SKS-style keyserver upload (`keyserver.ubuntu.com` etc)** — signature-poisoning risk + low marginal value. WKD + keys.openpgp.org cover the modern + roadmap-required publishing paths.
- **PGP encryption subkey** — out of scope; PGP is signing-only in this project. SECURITY.md already documents an `age`-based reporting channel for confidential disclosure (separate from release signing).
- **Sigstore TUF root pre-seeding doc** — operators wanting fully-offline verification need a one-time `cosign initialize` to fetch the TUF root. Phase 24's SECURITY.md may add a 1-liner ("first-time setup: run cosign initialize"). If it grows beyond a 1-liner, defer to a v1.7 quick task.
- **Cosign 3.0 migration doc** — Pitfall 13 anticipates this; Phase 23's SECURITY.md callout already names the cosign version range. When cosign 3.0 lands, both image-side AND tarball-side recipes need a touch. Single quick task at that time, not Phase 24's seat.
- **PGP key rotation execution (not documentation)** — Phase 24 documents the procedure; it does NOT execute it. The maintainer's actual key generation + first publish to WKD + keys.openpgp.org is the maintainer's action when they ship v1.6.0. If the maintainer wants this driven from CI via documented procedure, that's a quick task at key-rotation time.
- **Per-architecture tarballs (linux-arm64, darwin-amd64, etc)** — current release is linux-amd64 only. Adding other arches would multiply the sign/attest matrix. Out of scope for Phase 24; tracked as a v1.7+ scope expansion if operator demand surfaces.
- **`reproducibility-regression` post-release verifier** — Phase 25's seat. Phase 24 signs whatever bytes `cargo build --release` produces today; Phase 25 makes those bytes byte-equal.
- **Web-of-Trust signatures on the maintainer's PGP key** — defer indefinitely. Modern keys.openpgp.org doesn't propagate third-party signatures; WKD doesn't either. WoT is a niche-operator concern. If a specific operator requests it, signature can be added off-band.

</deferred>

---

*Phase: 24-release-tarball-signing-cosign-slsa-pgp*
*Context gathered: 2026-06-02*
