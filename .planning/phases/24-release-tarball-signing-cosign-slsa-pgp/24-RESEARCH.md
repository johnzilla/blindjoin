# Phase 24: Release Tarball Signing (cosign + SLSA + PGP) — Research

**Researched:** 2026-06-02
**Domain:** Sigstore cosign blob signing + SLSA v1.0 provenance for non-image artifacts + maintainer-held PGP key custody on YubiKey + WKD/keys.openpgp.org publication + GitHub Release asset workflow + softprops draft-release UX
**Confidence:** HIGH on the sigstore action shapes (verified against Phase 23 in-repo state + the actions' own action.yml at pinned SHAs); HIGH on softprops `draft:` input (verified at pinned SHA); HIGH on `gh attestation verify` CLI shape (verified at gh 2.92.0); HIGH on the `actions/attest-build-provenance` output mechanism (corrected from CONTEXT D-14 assumption — see §3.2); MEDIUM on WKD layout (verified against GnuPG manual but not against an existing `<owner>.github.io` repo); HIGH on PGP-on-YubiKey procedure shape (standard 5-step ed25519 ceremony).

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01: Inline in the existing `build` job AFTER `Package` step.** Sign/attest land between [release.yml:91-98 `Package`](.github/workflows/release.yml#L91) and [release.yml:100-107 `Upload to GitHub Releases`](.github/workflows/release.yml#L100). Three new steps: (a) `sigstore/cosign-installer@<sha>` setup, (b) `cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz`, (c) `actions/attest-build-provenance@<sha>` with `subject-path: blindjoin-linux-amd64.tar.gz`. The `softprops/action-gh-release` `files:` list ([release.yml:103-105](.github/workflows/release.yml#L103)) grows from 2 to 4 entries (tarball, .sha256, .bundle, .sigstore). Mirrors Phase 23 D-01 placement pattern.
- **D-02: `id-token: write` added at JOB level on `release.yml`'s `build` job, NOT workflow-level.** Per PITFALLS Pitfall 2 + Phase 23 D-02. Workflow-level `permissions: { contents: write }` at [release.yml:28-29](.github/workflows/release.yml#L28) STAYS. The `build` job grows an explicit `permissions: { contents: write, id-token: write, attestations: write }` block — `attestations: write` is required by `actions/attest-build-provenance` (verified at Phase 23 RESEARCH §2.1 + already present at [docker.yml:73-77](.github/workflows/docker.yml#L73)). Auditor-grepable "deliberately-omitted-scopes" comment names `packages`, `pull-requests`, `pages` (paraphrased — Phase 22 Plan 22-04 lesson).
- **D-03: Cosign signature distributed as `.bundle` (single file).** Mirrors Phase 23's image-side distribution; operators learn ONE verification recipe shape. Rejected: discrete `.sig` + `.crt`, both formats.
- **D-04: Both GitHub Attestations API AND `.sigstore` bundle as Release asset.** `actions/attest-build-provenance` invoked with `subject-path: blindjoin-linux-amd64.tar.gz` pushes to GH Attestations API (verified via `gh attestation verify blindjoin-linux-amd64.tar.gz --owner <owner>`) AND emits a file on disk. That file gets uploaded as a Release asset. Operators get two verification paths.
- **D-05: `.sigstore` bundle filename = `blindjoin-linux-amd64.tar.gz.sigstore`.** Mirrors `.sha256` sibling-suffix convention. **⚠️ RESEARCH CORRECTION (§3.2):** the planner-discretion D-14 assumed an `output-name` input on `actions/attest-build-provenance@v3.2.0` to control this filename — **this input does NOT exist**. The action writes to a path under `${RUNNER_TEMP}` and exposes it as the `bundle-path` output. The fallback (named in D-14) is the only path: capture `${{ steps.<provenance-step-id>.outputs.bundle-path }}` and `mv` to the deterministic filename in a subsequent step. See §3.2 for the exact YAML.
- **D-06: Maintainer-local sign + upload (PGP path).** PGP signing is OUT of `release.yml` entirely. Maintainer-side procedure in new `docs/RELEASING.md`: `gh release download v1.6.0 ... → gpg --detach-sign --armor ... → gh release upload ... → gh release edit --draft=false`. Rationale: putting the PGP private key in GitHub Secrets re-introduces GitHub as a trusted party for the PGP path — defeats SIGN-03.
- **D-07: GitHub Releases ship as `draft: true` until the maintainer flips them after `.asc` upload.** `softprops/action-gh-release` step grows `draft: true`. **VERIFIED (§3.3):** at the SHA-pinned version `@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`, `draft` is input #5 of 14 declared inputs (`body`, `body_path`, `name`, `tag_name`, **`draft`**, `prerelease`, `files`, ...).
- **D-08: Fresh ed25519 key generated on / transferred to a YubiKey 5 OpenPGP applet.** Algorithm: ed25519, signing-only (no encryption subkey). User-ID: `blindjoin maintainer <johnturner@gmail.com>`. Expiry: 2 years from creation. Revocation cert generated at key creation, stored offline. **Documented, NOT executed in Phase 24.**
- **D-09: Public key committed at `docs/pgp/<FULL-40-CHAR-FINGERPRINT>.asc`.** Filename is the identity. SECURITY.md anchors to `<a id="pgp-current"></a>`.
- **D-10: Public key published to BOTH WKD (on `<owner>.github.io`) AND `keys.openpgp.org`.** Roadmap SC#3 mandates keys.openpgp.org verbatim; WKD added for operator UX.
- **D-11: New `docs/RELEASING.md` owns the maintainer-side release procedure.**
- **D-12: SECURITY.md `## Supply-chain status` grows a second fenced bash recipes block for tarball verification** — `### Release tarball signatures + provenance (v1.6 onward)` subsection appended below the existing `### Image signatures + attestations (v1.6 onward)` subsection. Pitfall 13 callout is a 1-liner cross-ref to Phase 23's section, not duplication.

### Claude's Discretion (resolved in §2-§4 below)

- **D-13: SHA pins for new `uses:` lines.** RESOLVED §2.1 — reuse Phase 23's canonical pins verbatim (`sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` with `cosign-release: 'v2.6.3'`; `actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0`). No security advisories found at these SHAs as of 2026-06-02.
- **D-14: `actions/attest-build-provenance` output wiring for the `.sigstore` filename.** **RESOLVED §3.2 — RESEARCH CORRECTION.** No `output-name` input exists at v3.2.0. The action writes the bundle to a path under `${RUNNER_TEMP}` and exposes it via the `bundle-path` output. The plan MUST use a `mv` step (or shell rename inside the next step) to relocate the bundle to `blindjoin-linux-amd64.tar.gz.sigstore`.
- **D-15: `softprops/action-gh-release` files-list ordering.** RESOLVED §3.3 — semantic grouping: `blindjoin-linux-amd64.tar.gz` → `blindjoin-linux-amd64.tar.gz.sha256` → `blindjoin-linux-amd64.tar.gz.bundle` → `blindjoin-linux-amd64.tar.gz.sigstore` (artifact → integrity → signature → provenance).
- **D-16: `gh attestation verify` command shape.** RESOLVED §4.2 — modern shape is `gh attestation verify <file-path> --owner <owner>` OR `--repo <owner>/<name>`. `--owner` accepts the org/user; `--repo` accepts `owner/name`. Both are first-class; recommended for blindjoin's docs: `--repo johnzilla/blindjoin` for precise identity binding. Verified at `gh 2.92.0` (the locally-installed version) and at cli.github.com/manual/gh_attestation_verify.
- **D-17: Key-rotation cadence + procedure prose.** RESOLVED §5.2 — 5-step skeleton named, gpg commands provided, NOT executed.
- **D-18: softprops `draft: true` support at pinned SHA.** RESOLVED §3.3 — confirmed input #5 at SHA `de2c0eb89ae2a093876385947365aca7b0e5f844`.
- **D-19: WKD directory layout.** RESOLVED §5.3 — direct method: `.well-known/openpgpkey/hu/<wkd-hash>` on `<owner>.github.io`. `<wkd-hash>` derived via `gpg-wks-client --print-wkd-hash johnturner@gmail.com`. `<owner>.github.io` existence not verified — if absent, one-time `gh repo create johnzilla.github.io --public` is a maintainer-side documented step, NOT a Phase 24 commit.
- **D-20: CONTRIBUTING.md cross-ref insertion point.** RESOLVED §5.4 — natural fit: a new short paragraph at the END of the existing `## Tagging releases` section ([CONTRIBUTING.md:69-94](CONTRIBUTING.md#L69)), one line: "Maintainer-side release procedure (post-tag PGP sign + draft flip): see [`docs/RELEASING.md`](docs/RELEASING.md)."

### Deferred Ideas (OUT OF SCOPE for Phase 24)

- CI-managed PGP signing (defeats SIGN-03 rationale; v1.8+ only if co-maintainer onboards)
- Hybrid signing-subkey in CI (overkill for solo maintainer)
- SKS-style keyservers (`keyserver.ubuntu.com`) — poisoning risk, low marginal value
- PGP encryption subkey (signing-only project)
- Sigstore TUF root pre-seeding doc beyond a 1-liner
- Cosign 3.0 migration doc (single quick task when cosign 3.0 lands)
- PGP key generation EXECUTION (Phase 24 documents only; maintainer's actual key generation happens at v1.6.0 cut)
- Per-architecture tarballs (linux-arm64, darwin-amd64) — v1.7+ scope expansion
- `reproducibility-regression` post-release verifier (Phase 25's seat)
- Web-of-Trust signatures on the maintainer's key (modern keys.openpgp.org strips them; niche)
- HUMAN-UAT scaffold plan for fresh-machine UAT — deferred to first `v1.6.0-rc.0` tag push per Pitfall 12, Phase 23 closure pattern. No HUMAN-UAT plan file is written in Phase 24.
- New sigstore-pin grep gate — Phase 23 Plan 23-03's `sigstore-pin-check` job already greps `.github/workflows/` (covers `release.yml` automatically); no new gate.
- `.sbom` Release asset for tarballs (image-side per ATTEST-03, not in scope for SIGN-0*)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **SIGN-01** | Every `blindjoin-linux-amd64.tar.gz` release artifact is accompanied by a cosign blob signature uploaded to the same GitHub Release (`.bundle` format, or discrete `.sig` + `.crt`). | §3.1 — `cosign sign-blob --yes --bundle <output> <subject>` YAML shape with cosign 2.6.3; §3.3 — files-list grows to include `blindjoin-linux-amd64.tar.gz.bundle`; §4.1 — operator-facing `cosign verify-blob --bundle ... --certificate-identity-regexp 'release\.yml@refs/tags/v.*' ...` recipe shape. |
| **SIGN-02** | Every release tarball carries a SLSA v1.0 provenance attestation via the same `actions/attest-build-provenance` machinery as ATTEST-02. Verifier UX is consistent across image + tarball artifacts. | §3.2 — `actions/attest-build-provenance@96278af... # v3.2.0` invocation with `subject-path: blindjoin-linux-amd64.tar.gz`; **RESEARCH CORRECTION**: no `output-name` input — use `bundle-path` output + `mv` to `blindjoin-linux-amd64.tar.gz.sigstore`; §4.2 — TWO verifier paths documented in SECURITY.md (gh attestation verify API path + cosign verify-attestation bundle path). |
| **SIGN-03** | A detached PGP signature alternative path is shipped alongside the cosign signature: maintainer-held PGP key, exported public key committed to the repo + uploaded to keys.openpgp.org, signing key fingerprint documented in `SECURITY.md`. Provides a non-OIDC verification path for operators who cannot reach sigstore Fulcio/Rekor at verification time. | §5.1 — ed25519-on-YubiKey ceremony skeleton (D-08); §5.2 — 5-step key-rotation procedure (D-17); §5.3 — WKD + keys.openpgp.org publication procedure (D-19); §4.3 — operator-facing `gpg --auto-key-locate wkd --locate-keys ... → gpg --verify ...asc ...tar.gz` recipe shape; §6 — `docs/pgp/<fingerprint>.asc` + SECURITY.md fingerprint anchor (D-09 + D-12); §6 — `docs/RELEASING.md` maintainer-side procedure file (D-11). |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **Skill routing:** None of the available skills (office-hours, investigate, ship, qa, review, etc.) match a research/planning task — proceed via the GSD workflow directly. The `/browse` skill is the canonical web-browse path but Phase 24 research uses `WebSearch`/`WebFetch` and `mcp__context7` is unavailable in this agent.
- **GSD workflow enforcement:** All Phase 24 file edits happen via `/gsd:execute-phase 24` after planning closes; no direct edits outside GSD.
- **No protocol code touched** — pure CI/CD/docs (mirrors Phase 22 + 23 scope discipline).
- **MIT, public-good** — PGP path is maintainer-held to give operators a non-OIDC trust root; the entire point.
- **Tor-native + signet-first** — no protocol invariants touched.
- **Project skills:** `.claude/skills/` and `.agents/skills/` do not exist (verified: `ls docs/` shows only AUDIT-CHARTER, PROTOCOL, branch-protection). No project-specific skill rules to honor beyond CLAUDE.md.

---

## 1. Phase Goal Recap

From CONTEXT.md `<domain>` + ROADMAP §Phase 24: *"Every `blindjoin-linux-amd64.tar.gz` published as a GitHub Release asset can be cryptographically attributed to the maintainer via TWO independent paths — the OIDC-keyless cosign path (consistent with image signing) AND a maintainer-held PGP key path for operators who cannot reach Sigstore Fulcio/Rekor at verification time."*

Phase 24 delivers, INSIDE `release.yml`'s existing `build` job and inheriting the existing `if: startsWith(github.ref, 'refs/tags/')` gate:

1. A job-level `permissions:` block growing the default `contents: write` to `{ contents: write, id-token: write, attestations: write }`.
2. A `sigstore/cosign-installer` setup step (Phase 23 SHA reuse).
3. A `cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz` step (SIGN-01 — produces the `.bundle` asset).
4. An `actions/attest-build-provenance@<sha>` step with `subject-path: blindjoin-linux-amd64.tar.gz` (SIGN-02 — produces SLSA v1.0 in-toto provenance, pushed to GH Attestations API AND a local `.sigstore` bundle file).
5. A `mv` step (RESEARCH CORRECTION) that relocates the provenance bundle from `${{ steps.<id>.outputs.bundle-path }}` to `blindjoin-linux-amd64.tar.gz.sigstore`.
6. The existing `softprops/action-gh-release` step grows `draft: true` and the `files:` list expands from 2 to 4 entries.

In NEW files:

7. `docs/RELEASING.md` — full maintainer-side procedure (cut tag → wait for CI → download artifact → PGP sign on YubiKey → upload → draft-flip; plus key-rotation + revocation + WKD/keys.openpgp.org publish procedures).
8. `docs/pgp/<FULL-40-CHAR-FINGERPRINT>.asc` — armored ed25519 public key. **Phase 24 commits a placeholder OR the maintainer generates the key BEFORE the Phase 24 PR is merged.** Planner-discretion: this is the only Phase 24 file whose content depends on key material that doesn't exist yet. RECOMMENDED: gate `docs/pgp/*.asc` and the SECURITY.md fingerprint anchor as the LAST plan in Phase 24 — the maintainer generates the key, hands the fingerprint + .asc to the planner, and the plan commits both atomically. See §5.1.

In MODIFIED files:

9. `SECURITY.md` — append `### Release tarball signatures + provenance (v1.6 onward)` subsection below the existing image subsection. Three verify recipes (cosign verify-blob, gh attestation verify + cosign verify-attestation, gpg --verify) + 1-line Pitfall 13 cross-ref + a new "fingerprint anchor" callout naming the current maintainer fingerprint (D-09 anchor).
10. `CONTRIBUTING.md` — one-line cross-ref to `docs/RELEASING.md` (D-20).

---

## 2. Cross-Phase Pin Reuse

### 2.1 SHA pins (D-13) — reuse Phase 23's canonical pins verbatim [VERIFIED: in-repo state @ docker.yml:151 + docker.yml:272]

The Phase 23 `docker.yml` is the single source of truth for sigstore action SHAs. Phase 24 adopts the SAME SHAs into `release.yml` so a single sigstore-ecosystem rotation event touches both files together. The `sigstore-pin-check` CI gate (Phase 23 Plan 23-03) already greps every workflow under `.github/workflows/` and enforces the 40-hex pin — no new gate needed.

| Action | Phase 23 pin | Phase 24 adoption | Verified at |
|--------|--------------|-------------------|-------------|
| `sigstore/cosign-installer` | `@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` | identical | [docker.yml:151](.github/workflows/docker.yml#L151) |
| `sigstore/cosign-installer` `cosign-release:` | `'v2.6.3'` | identical | [docker.yml:153](.github/workflows/docker.yml#L153) |
| `actions/attest-build-provenance` | `@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0` | identical | [docker.yml:272](.github/workflows/docker.yml#L272) |

**NOT adopted from Phase 23:** `actions/attest-sbom` and `anchore/sbom-action`. Phase 24 SBOM-side scope is excluded per CONTEXT `<domain>` ("does NOT add a `.sbom` Release asset for tarballs — SBOM is image-side per ATTEST-03, not in scope for SIGN-0*").

**Security-advisory check (2026-06-02):** WebSearch against `sigstore/cosign-installer 7e8b...d4be13a184e9ebc5 v3.10.1 security advisory CVE` returned no published advisory for either pinned SHA. The sigstore project's [security overview page](https://github.com/sigstore/cosign/security) does not list a CVE matching cosign 2.6.3 or cosign-installer v3.10.1. No reopener of the choice required.

### 2.2 SHA-pin trailing-comment style (Phase 23 PATTERNS §2)

All new `uses:` lines MUST follow the project pattern: `<owner>/<action>@<40-hex> # v<X.Y.Z>` with TWO spaces before `#`. Enforced by Phase 23 `sigstore-pin-check` job.

### 2.3 `sigstore-pin-check` CI gate inheritance [VERIFIED: ci.yml:292-326]

Phase 23 Plan 23-03 added the `sigstore-pin-check` job to `ci.yml`. The job's pattern grep target list:

```
PATTERN='uses:\s*(sigstore/cosign-installer|actions/attest-build-provenance|actions/attest-sbom|anchore/sbom-action)@(?![a-f0-9]{40})'
... grep -rnPE "${PATTERN}" .github/workflows/
```

`.github/workflows/` matches BOTH `docker.yml` and `release.yml`. Phase 24's two new sigstore-action `uses:` lines in `release.yml` are caught automatically. **No new gate is needed; no Plan 24-XX touches `ci.yml`.** This is the canonical "Phase 23 establishes discipline; Phase 24 inherits" example.

---

## 3. release.yml Integration Details

### 3.1 `cosign sign-blob` step shape [VERIFIED: cosign 2.6.3 docs + Phase 23 PATTERNS]

The sign-blob equivalent of Phase 23's image `cosign sign --yes "${IMAGE}@${DIGEST}"`. Sign-blob targets a file path on disk, not an OCI reference, and writes the bundle to a path passed via `--bundle`.

**Verified YAML shape:**

```yaml
# Phase 24 SIGN-01: cosign keyless OIDC blob signing.
# Produces blindjoin-linux-amd64.tar.gz.bundle (sig + cert + Rekor inclusion proof
# in cosign 2.x bundle format). Operator verifies via:
#   cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \
#     --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
#     --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
#     blindjoin-linux-amd64.tar.gz
# (full recipe lives in SECURITY.md per D-12)
#
# --yes:  non-interactive; required in CI (would otherwise prompt for transparency-log consent).
# --bundle <file>: writes the cosign 2.x bundle format to a file (sig + cert + Rekor proof
#                  in one JSON file). Operator passes the same file via --bundle to verify-blob.
# See PITFALLS Pitfall 3: tlog upload MUST NOT be disabled.
- name: Sign tarball with cosign (keyless OIDC) — SIGN-01
  run: cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz
```

The step runs AFTER the `Package` step at [release.yml:91-98](.github/workflows/release.yml#L91) (which creates `blindjoin-linux-amd64.tar.gz` in the workspace root) and AFTER the `Install cosign` step (§3.4 below).

**Step-name convention:** "Sign tarball with cosign (keyless OIDC) — SIGN-01" mirrors Phase 23's "Sign image with cosign (keyless OIDC) — ATTEST-01" at [docker.yml:175](.github/workflows/docker.yml#L175).

**No env block needed.** Phase 23 used `env: { IMAGE: ..., DIGEST: ... }` because the OCI reference is a non-trivial concatenation. Sign-blob's command is a literal file path; an env block adds no clarity.

### 3.2 `actions/attest-build-provenance` step shape — RESEARCH CORRECTION on D-14 [VERIFIED: action.yml @ 96278af6caaf10aea03fd8d33a09a777ca52d62f]

**CONTEXT.md D-14 asserts** *"The action accepts an `output-name` (or equivalent) input to control the bundle file path."* — **this is INCORRECT.**

**Authoritative source:** the action.yml at the pinned SHA `96278af6caaf10aea03fd8d33a09a777ca52d62f` (v3.2.0) declares exactly these inputs and outputs:

| Inputs | Outputs |
|--------|---------|
| `subject-path` | `bundle-path` |
| `subject-digest` | `attestation-id` |
| `subject-name` | `attestation-url` |
| `subject-checksums` | |
| `push-to-registry` | |
| `create-storage-record` | |
| `show-summary` | |
| `github-token` | |

There is no `output-name` input. There is no `bundle-name` input. The bundle is written to a path inside `${RUNNER_TEMP}` and the path is exposed via the `bundle-path` output (and additionally appended to `${RUNNER_TEMP}/created_attestation_paths.txt`).

**Correct YAML shape (must include the `mv` step):**

```yaml
# Phase 24 SIGN-02: SLSA v1.0 in-toto build provenance attestation.
# Predicate type emitted: https://slsa.dev/provenance/v1 (auto-derived from
# workflow context). Names the workflow file (release.yml), tag ref, source
# commit, and runner image automatically. RESEARCH CORRECTION: the action does
# NOT accept an output-name input at v3.2.0 (verified against action.yml at
# the pinned SHA) — the bundle path is exposed via the bundle-path output and
# we mv it to the deterministic filename in the next step.
#
# push-to-registry: NOT set (defaults false; we are not pushing to an OCI
# registry — tarball provenance lives at the GH Attestations API + on disk).
# The attestation is BOTH (a) pushed to the GitHub Attestations API for
# `gh attestation verify` operators AND (b) emitted to disk for the
# blindjoin-linux-amd64.tar.gz.sigstore Release asset (D-04 two-path UX).
- name: Attest tarball build provenance — SIGN-02
  id: provenance
  uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0
  with:
    subject-path: blindjoin-linux-amd64.tar.gz

# Phase 24 SIGN-02 (rename): relocate the provenance bundle to the
# deterministic .sigstore filename that Plan 23-04's SECURITY.md recipe and
# the softprops upload files: list reference. The action writes the bundle to
# a path under ${RUNNER_TEMP}; we mv it to the workspace root so softprops
# can pick it up. RESEARCH §3.2 correction — the action has no output-name
# input at v3.2.0.
- name: Rename provenance bundle to .sigstore Release asset filename
  run: mv "${{ steps.provenance.outputs.bundle-path }}" blindjoin-linux-amd64.tar.gz.sigstore
```

**Why this matters for the plan:** the planner MUST split the SIGN-02 work across TWO steps (attest + rename). A single-step shape is impossible at the pinned SHA. If the planner forgets the rename, the softprops upload step will fail with "file not found" on the `.sigstore` line in the `files:` list — visible failure mode, but a Wave-0-level QA cost.

**Alternative path considered + rejected:** read `${{ steps.provenance.outputs.bundle-path }}` directly inside the softprops `files:` list (e.g., `files: |\n  ${{ steps.provenance.outputs.bundle-path }}`). This works but ships the artifact at the runner-tempfile filename (e.g., `attestation-7f3a...json`) which is non-deterministic, breaks the SECURITY.md recipe (which names the file by its operator-readable suffix), and breaks the natural .sigstore extension convention. The `mv` step is one extra line for material UX clarity.

### 3.3 `softprops/action-gh-release` modifications — D-07 + D-15 + D-18 [VERIFIED: action.yml @ de2c0eb89ae2a093876385947365aca7b0e5f844]

The existing step at [release.yml:100-107](.github/workflows/release.yml#L100):

```yaml
- name: Upload to GitHub Releases
  uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
  with:
    files: |
      blindjoin-linux-amd64.tar.gz
      blindjoin-linux-amd64.tar.gz.sha256
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

grows to:

```yaml
- name: Upload to GitHub Releases (draft — maintainer flips out of draft after PGP upload)
  uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
  with:
    # D-07: Release ships as draft until the maintainer uploads the .asc detached
    # PGP signature (per docs/RELEASING.md procedure) and runs
    # `gh release edit vX.Y.Z --draft=false`. Operators visiting the Releases
    # page never see a release missing the PGP signature.
    draft: true
    # D-15: semantic grouping — artifact, integrity, signature, provenance.
    # The .asc PGP detached signature is uploaded post-CI by the maintainer; it
    # is NOT in this list. Per D-06 PGP signing is OUT of release.yml entirely.
    files: |
      blindjoin-linux-amd64.tar.gz
      blindjoin-linux-amd64.tar.gz.sha256
      blindjoin-linux-amd64.tar.gz.bundle
      blindjoin-linux-amd64.tar.gz.sigstore
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**Input verification at SHA `de2c0eb89ae2a093876385947365aca7b0e5f844`:** the action.yml declares 14 inputs including `draft` (#5 in declaration order): `body`, `body_path`, `name`, `tag_name`, **`draft`**, `prerelease`, `files`, `fail_on_unmatched_files`, `repository`, `token`, `target_commitish`, `discussion_category_name`, `generate_release_notes`, `append_body`. Default for `draft` is `false`.

**`fail_on_unmatched_files` consideration:** not currently set; default is `false`. If a future change to the workflow accidentally drops a file (e.g., the `mv` step at §3.2 fails silently), softprops would create the release without the missing asset and not fail the workflow. Planner-discretion suggestion: set `fail_on_unmatched_files: true` to make the upload step ASSERT all 4 files exist. Low-risk addition; satisfies SIGN-01's "uploaded to the same GitHub Release" literal contract by failing loudly if upload is incomplete.

### 3.4 Permissions block — D-02 [VERIFIED: Phase 23 pattern at docker.yml:73-77]

The current `build` job has NO explicit `permissions:` block — it inherits the workflow-level `contents: write` at [release.yml:28-29](.github/workflows/release.yml#L28). Phase 24 grows the `build` job an explicit block. The workflow-level block stays at `contents: write` (the `check` job still needs that for nothing currently — but DOES NOT need `id-token: write`, so narrowing to job-level is strictly safer).

**Verified Phase 23 pattern (literal copy from [docker.yml:61-77](.github/workflows/docker.yml#L61), adapted for release.yml):**

```yaml
  build:
    name: Build linux-amd64
    needs: check
    runs-on: ubuntu-latest
    # Publish gate: only run on a real tag push. workflow_dispatch runs
    # check-only (the rehearsal path) and never uploads release artifacts.
    if: startsWith(github.ref, 'refs/tags/')
    # Phase 24 SIGN-01/02: cosign keyless signing + actions/attest-build-provenance need:
    #   - contents:     write — softprops/action-gh-release uploads Release assets.
    #   - id-token:     write — OIDC token for Fulcio cert exchange. Without this,
    #                   cosign sign-blob fails with the opaque "fulcio: 400 Bad
    #                   Request" error. See PITFALLS Pitfall 2 + Phase 23 D-02.
    #   - attestations: write — persist the SLSA provenance attestation to GitHub's
    #                   attestations API. Without this, actions/attest-build-provenance
    #                   fails with 403 Forbidden on the API call. See Phase 23
    #                   RESEARCH §2.1 + the matching docker.yml block at lines 67-70.
    # Deliberately omitted (auditor-grepable per Plan 22-04): packages, PR-write,
    # pages, issues, deployments. These tokens MUST NOT appear anywhere in this file.
    permissions:
      contents: write
      id-token: write
      attestations: write
```

**Key differences from docker.yml:**
- `contents: write` (release.yml), NOT `contents: read` (docker.yml) — softprops uploads need write.
- `packages` (write) is deliberately ABSENT — `release.yml` does not push to ghcr.io. This is named in the "deliberately omitted" list so the auditor grep `! grep -q 'packages:'` passes at file level.
- All other scopes (`pull-requests`, `pages`, `issues`, `deployments`) are deliberately omitted using PARAPHRASED tokens (`PR-write`, `pages`, `issues`, `deployments` — without the literal `:` suffix on `pull-requests` and `id-token` etc.) per Phase 22 Plan 22-04 lesson. The Phase 23 docker.yml block uses paraphrased tokens; Phase 24 release.yml mirrors.

**Caution on existing workflow-level `contents: write`:** the workflow-level block at [release.yml:28-29](.github/workflows/release.yml#L28) sets `contents: write` for ALL jobs. The `check` job (line 32-58) is therefore granted `contents: write` it doesn't need. Phase 24 is OUT of scope to fix this (it's pre-existing); the planner-discretion note is whether to ALSO add an explicit `permissions: { contents: read }` to the `check` job for symmetry with the build job's explicit block. Recommended: defer to a v1.7 carry-forward quick task. Phase 24 makes one workflow change at a time.

---

## 4. Operator-facing SECURITY.md recipes — D-12

### 4.1 `cosign verify-blob` recipe (SIGN-01 operator verify) [VERIFIED: cosign 2.6.3 CLI + Phase 23 SECURITY.md @ lines 138-170]

Mirrors Phase 23's image `cosign verify` recipe in SECURITY.md (lines 138-170), differing in:
- `cosign verify-blob` instead of `cosign verify`
- `--bundle <file>` instead of `--certificate-identity / --signature / --certificate` triple
- `release\.yml` instead of `docker\.yml` in the identity-regexp (PITFALLS Pitfall 1)
- positional file argument `blindjoin-linux-amd64.tar.gz` instead of an OCI image reference

**Recipe (drop-in for SECURITY.md `### Release tarball signatures + provenance (v1.6 onward)` subsection):**

```bash
# 1. Cosign blob signature verification (SIGN-01)
cosign verify-blob \
  --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
# Expected: "Verified OK" + JSON cert claims.
```

### 4.2 SLSA provenance verify — TWO PATHS (D-04 + D-16) [VERIFIED: gh 2.92.0 CLI + cli.github.com/manual/gh_attestation_verify]

**Path A: GitHub Attestations API (requires github.com reachable).** `gh 2.92.0` supports both `--owner` and `--repo`:

```bash
# Path A: GitHub Attestations API
gh attestation verify blindjoin-linux-amd64.tar.gz --repo <owner>/blindjoin
# OR
gh attestation verify blindjoin-linux-amd64.tar.gz --owner <owner>
# Expected: "✓ Verification succeeded! ..." + attestation summary.
```

**RECOMMENDED FORM:** `--repo <owner>/blindjoin` — binds verification to the specific repo, not just the owner. Tighter security guarantee (matches the spirit of Pitfall 1 — narrow identity binding wins).

**Version footnote required in SECURITY.md:** `gh` CLI 2.50+ supports both flags as documented; earlier versions only had `--repo`. Recommended footnote: "Requires `gh` 2.50 or later. Install via [cli.github.com](https://cli.github.com)."

**Path B: offline cosign-based verify (works without github.com reachable, after one-time Sigstore TUF cache seeding).**

```bash
# Path B: offline cosign-based verify
cosign verify-attestation \
  --bundle blindjoin-linux-amd64.tar.gz.sigstore \
  --type slsaprovenance \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
# Expected: "Verified OK" + the SLSA v1.0 in-toto predicate.
```

**Important planner note:** verify both recipes at Plan-write time against the actual artifacts produced by a `workflow_dispatch` rehearsal — `cosign verify-attestation --bundle ... --type slsaprovenance` has historically required nuanced flag combinations across cosign 2.x minors (CONTEXT specifics §"Operator-facing SLSA verify recipe shape" presents this shape as RECOMMENDED, not verified). At cosign 2.6.3, the `--bundle <file> --type slsaprovenance` form is supported per the [cosign 2.6.3 release notes](https://github.com/sigstore/cosign/releases/tag/v2.6.3), but the precise predicate decoding (which produces the SLSA in-toto JSON on stdout) is a minor UX wrinkle — the recipe ASSERTS verification but does NOT print the predicate JSON by default. Operators wanting the predicate body itself add `--output-file <file>` to capture it. [ASSUMED] — recommend planner adds a 1-liner to the SECURITY.md prose: "to inspect the SLSA predicate body itself, add `--output-file slsa-predicate.json`".

### 4.3 PGP verify recipe (SIGN-03 operator verify) [CITED: WKD spec + GnuPG manual]

**Recipe (drop-in for SECURITY.md):**

```bash
# One-time key fetch via WKD (recommended)
gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com
# Fallback: keys.openpgp.org keyserver (recipe equivalent for operators
# whose ISP blocks WKD .well-known/openpgpkey/... requests)
gpg --keyserver hkps://keys.openpgp.org --recv-keys <FULL-40-CHAR-FINGERPRINT>

# Verify
gpg --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz
# Expected: "Good signature from \"blindjoin maintainer <johnturner@gmail.com>\""
# WARNING line about "not certified with a trusted signature" is EXPECTED if
# the operator hasn't manually signed the maintainer's key with their own —
# the operator should compare the fingerprint printed against the canonical
# fingerprint anchored at SECURITY.md#pgp-current.
```

**Important UX prose required in SECURITY.md:** `gpg --verify` returns exit 0 on "Good signature" even WITHOUT the operator's local trust web certifying the maintainer's key. This is normal. The operator's trust gate is the FINGERPRINT comparison — printed by `gpg --verify` to stderr — against the canonical fingerprint at the SECURITY.md `<a id="pgp-current"></a>` anchor. Operators should automate this with `gpg --verify --status-fd=1 ... | grep VALIDSIG | grep <FULL-40-CHAR-FINGERPRINT>` if they care about scripted verification. [VERIFIED: standard PGP UX — see [ci.yml:99-115](.github/workflows/ci.yml#L99) for the project's own VALIDSIG-grep precedent inside `bitcoind-install`].

### 4.4 "EITHER OR" prose [LOCKED in D-12]

SECURITY.md MUST explicitly say: **EITHER cosign OR PGP verification is sufficient — they're alternative paths, not both required.** This matches SIGN-03's "non-OIDC alternative path" rationale verbatim. Operators picking the PGP path have a complete trust chain (WKD → fingerprint anchor → SECURITY.md prose); operators picking the cosign path have a complete trust chain (Fulcio OIDC → cosign 2.6.3 verification → SLSA provenance). NEITHER path requires the other.

### 4.5 Pitfall 13 cosign-3.0 callout — 1-line cross-ref [LOCKED in CONTEXT]

The existing image-side SECURITY.md subsection ([SECURITY.md:179-185](SECURITY.md#L179)) already names cosign `>= 2.6.3, < 3.0.0`. Phase 24's tarball subsection adds a 1-liner: *"For cosign 3.0 migration, see the [image-side cosign 3.0 callout](#cosign-30-callout-anchor) above — the same version pin applies."* Or, simpler: rely on the reader noticing the two subsections share a `## Supply-chain status` parent. The Phase 24 prose says nothing more; Phase 23's callout is the single source of truth. RECOMMENDED: cite Phase 23's callout block by inserting an `<a id="cosign-30-callout"></a>` anchor inside it (1-line addition, separate quick-task scope) IF the planner wants link-target stability — otherwise the prose works.

### 4.6 Fingerprint anchor — D-09 + D-12

The new `### Release tarball signatures + provenance (v1.6 onward)` subsection MUST include an HTML anchor naming the current maintainer fingerprint:

```markdown
<a id="pgp-current"></a>
**Current maintainer PGP fingerprint:** `XXXX XXXX XXXX XXXX XXXX  XXXX XXXX XXXX XXXX XXXX` (UID `blindjoin maintainer <johnturner@gmail.com>`, ed25519, generated YYYY-MM-DD, expires YYYY-MM-DD).
```

The `XXXX XXXX...` placeholder is replaced by the maintainer's actual fingerprint when the key is generated. **This is the only Phase 24 string that cannot be locked at planning time** — see §6 deliverable ordering.

---

## 5. PGP path — docs/RELEASING.md content (D-11, D-17, D-19, D-20)

### 5.1 ed25519-on-YubiKey ceremony — D-08 [CITED: standard GnuPG procedure; not executed]

**5-step procedure (documented in docs/RELEASING.md, NOT executed in Phase 24):**

1. **Generate primary signing key directly on YubiKey:**
   ```bash
   gpg --card-edit
   # admin
   # generate
   # → Select ed25519, signing-only, 2-year expiry
   # → User-ID: "blindjoin maintainer <johnturner@gmail.com>"
   ```
   Generation happens on-card; private key material never touches the host filesystem. The YubiKey ed25519 applet has been supported since YubiKey firmware 5.2.3 (verified at [yubico.com docs](https://developers.yubico.com/PGP/Card_edit.html)) [CITED].
2. **Generate revocation certificate, store offline:**
   ```bash
   gpg --output revoke.asc --gen-revoke <fingerprint>
   # Move revoke.asc to a USB drive + paper backup; remove from disk.
   shred -u revoke.asc 2>/dev/null || rm -P revoke.asc 2>/dev/null
   ```
3. **Export public key:**
   ```bash
   gpg --export --armor <fingerprint> > docs/pgp/<FULL-40-CHAR-FINGERPRINT>.asc
   ```
4. **Publish to keys.openpgp.org:**
   ```bash
   gpg --send-keys --keyserver hkps://keys.openpgp.org <fingerprint>
   # Then verify the email confirmation link sent to johnturner@gmail.com.
   ```
5. **Publish to WKD** — see §5.3.

**Verification by operator:** `gpg --with-colons --import-options show-only --import docs/pgp/<...>.asc | head -2` returns a `fpr:` line whose 10th field (the fingerprint) equals the filename. Self-verifying; no SECURITY.md prose required to anchor the binding.

### 5.2 Key-rotation cadence + procedure — D-17

**5-step rotation procedure (documented in docs/RELEASING.md, NOT executed in Phase 24):**

1. **6 months before expiry**, generate a new ed25519 key on the same YubiKey (or a new YubiKey for stronger key isolation).
2. **Sign the new key with the old key** (cross-sign): `gpg --sign-key <new-fingerprint>` while the old key is still valid. This provides a verifiable provenance chain for operators using the WoT-aware path (rare; modern keys.openpgp.org strips third-party signatures).
3. **Commit the new public key**: `gpg --export --armor <new-fingerprint> > docs/pgp/<new-FULL-40-CHAR-FINGERPRINT>.asc`. The OLD key file STAYS in the repo (historical record).
4. **Update SECURITY.md's `<a id="pgp-current"></a>` anchor to name the new fingerprint.** The old fingerprint stays in CHANGELOG.md (transparency).
5. **Publish to keys.openpgp.org + WKD** (§5.1 step 4 + §5.3 for the new key). Wait 24h for propagation.
6. **Cut the next release with the new key.** First release signed with new key SHOULD be accompanied by a CHANGELOG entry naming the rotation event.

**Pitfall:** rotating WITHIN the 2-year window because of a YubiKey loss/compromise = "revocation", not "rotation". Different procedure: publish the offline-stored revocation cert immediately, then run the rotation procedure. Revocation procedure is a 6th section in docs/RELEASING.md.

### 5.3 WKD publication — D-19 [VERIFIED: GnuPG wiki + Debian manpages]

**WKD Direct Method on `<owner>.github.io`:**

The WKD direct method serves a hashed-userid file at:
```
https://<domain>/.well-known/openpgpkey/hu/<wkd-hash>
```

For `johnturner@gmail.com`, `<wkd-hash>` is derived via:
```bash
gpg-wks-client --print-wkd-hash johnturner@gmail.com
# Output: <32-char-zbase32-hash> johnturner@gmail.com
```

**Step-by-step (documented in docs/RELEASING.md):**

1. **Verify `<owner>.github.io` repo exists.** If not, one-time:
   ```bash
   gh repo create <owner>.github.io --public --description "GitHub Pages site for <owner> — hosts WKD .well-known/openpgpkey for blindjoin maintainer key"
   ```
   **NOT executed in Phase 24.** This is a maintainer-side step at first key generation; flagged in docs/RELEASING.md as a one-time setup item.
2. **Compute the WKD hash:**
   ```bash
   WKD_HASH=$(gpg-wks-client --print-wkd-hash johnturner@gmail.com | awk '{print $1}')
   ```
3. **Export the public key in WKD's binary keyring format (NOT armored):**
   ```bash
   gpg --no-armor --export johnturner@gmail.com > "${WKD_HASH}"
   ```
4. **Commit to `<owner>.github.io`:**
   ```bash
   cd path/to/<owner>.github.io
   mkdir -p .well-known/openpgpkey/hu
   mv /path/to/${WKD_HASH} .well-known/openpgpkey/hu/${WKD_HASH}
   git add .well-known/openpgpkey/hu/${WKD_HASH}
   git commit -m "wkd: publish blindjoin maintainer key for johnturner@gmail.com"
   git push
   ```
5. **Test WKD resolution from a fresh machine:**
   ```bash
   gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com
   # Should print the imported key and its fingerprint.
   ```

**Refresh cadence:** WKD-published key needs to be re-uploaded ONLY on rotation (every 2 years per D-08) or revocation. No daily/weekly refresh required.

**Subdomain method vs direct method:** GnuPG wiki notes the subdomain method (`openpgpkey.<domain>`) is "preferred", but GitHub Pages does NOT support subdomain wildcarding on `*.github.io` — direct method is the only viable path for `<owner>.github.io`. If the maintainer later switches to a custom domain (`blindjoin.example.com`), the subdomain method becomes available as an upgrade. Out of scope for Phase 24.

### 5.4 CONTRIBUTING.md cross-ref — D-20

**Insertion point:** at the END of the existing `## Tagging releases` section (currently [CONTRIBUTING.md:69-94](CONTRIBUTING.md#L69)), AFTER the existing prose about milestone names + crate versions + the tag-bash example.

**Exact one-line addition (drop-in):**

```markdown

Once `release.yml` finishes, the maintainer-side procedure (download the CI-built tarball, sign it with PGP on a YubiKey, upload the `.asc`, flip the release out of draft) lives in [`docs/RELEASING.md`](docs/RELEASING.md). Most contributors don't need it; it's the release-engineering manual for the maintainer.
```

The "Most contributors don't need it" prose makes the contributor audience understand they shouldn't read docs/RELEASING.md unless they're cutting a release.

### 5.5 `docs/RELEASING.md` skeleton — D-11

The new file's table of contents:

```markdown
# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see CONTRIBUTING.md for the contributor manual.

## Prerequisites

- YubiKey 5 (firmware ≥ 5.2.3 for ed25519 support) with the blindjoin maintainer PGP key (one-time generation: see §"PGP key generation").
- `gpg` 2.4+ on the maintainer's machine.
- `gh` 2.50+ on the maintainer's machine.
- `<owner>.github.io` repo exists with WKD published (one-time setup: see §"Publishing the key to WKD").

## Per-release procedure (5 steps)

1. `git tag -s vX.Y.Z -m "vX.Y.Z"`; `git push --tags`.
2. Watch `release.yml` in the Actions tab until green. CI creates a DRAFT release with 4 assets (tarball, .sha256, .bundle, .sigstore).
3. `gh release download vX.Y.Z -p 'blindjoin-linux-amd64.tar.gz' --dir /tmp/blindjoin-release`.
4. `cd /tmp/blindjoin-release && gpg --detach-sign --armor --local-user <FINGERPRINT> blindjoin-linux-amd64.tar.gz` (YubiKey will prompt for touch).
5. `gh release upload vX.Y.Z blindjoin-linux-amd64.tar.gz.asc && gh release edit vX.Y.Z --draft=false`.

## Pre-flight check before flipping out of draft

Cosign-verify all 4 CI-produced assets BEFORE running step 5's `--draft=false`. If any cosign verify fails, DO NOT flip; delete the release with `gh release delete vX.Y.Z` and re-cut the tag after the fix.

## PGP key generation (one-time, NOT a release-cut step)

[5-step ceremony per §5.1 above]

## PGP key rotation (every 2 years)

[6-step procedure per §5.2 above]

## PGP key revocation (emergency — YubiKey lost or compromised)

[Publish the offline-stored revoke.asc immediately, then re-run rotation procedure]

## Publishing the key to keys.openpgp.org

[Per §5.1 step 4]

## Publishing the key to WKD

[5-step per §5.3 above]
```

The new file weighs in around 200-300 lines fully populated. RECOMMENDED Plan structure (planner-discretion): ONE plan creates docs/RELEASING.md (3-4 tasks: per-release procedure + key generation + rotation/revocation + WKD/keys.openpgp.org).

---

## 6. Deliverable Ordering + Atomicity

The maintainer's PGP key fingerprint is the ONLY Phase 24 string that cannot be hardcoded at planning time. Three options for the planner:

**Option A: Defer key generation to maintainer; commit a placeholder.**
- All Phase 24 plans land with `<FULL-40-CHAR-FINGERPRINT>` as a literal placeholder string.
- Maintainer generates the key at v1.6.0-rc.0 cut and follows a documented "one-time setup" quick task that replaces the placeholder + commits `docs/pgp/<actual-fp>.asc`.
- Cleanest plan boundaries; clearest "what's in Phase 24" story.

**Option B: Maintainer pre-generates the key BEFORE the Phase 24 PR is opened.**
- Maintainer runs the §5.1 ceremony, hands fingerprint + .asc to the planner.
- All Phase 24 plans land with the literal fingerprint embedded.
- Risk: ties Phase 24 timing to a maintainer action that needs to happen on a physical YubiKey at a specific moment.

**Option C: Plan structure has a final "checkpoint:human-verify" task that the maintainer fills in.**
- All Phase 24 plans except the LAST one are pure-code/pure-doc; the last plan is a checkpoint that the maintainer drives, committing both the fingerprint + .asc + SECURITY.md anchor in one atomic commit.
- Best balance: maintainer's physical-key action is isolated to one plan; all upstream plans are autonomous.

**RECOMMENDED: Option C.** Phase 23 closure pattern (HUMAN-UAT deferred to first tag push) sets the precedent: physical-world / human-bandwidth-bound work goes in a final non-autonomous task at the end of the phase. The plan-author writes the SECURITY.md prose with `<FINGERPRINT-TBD>` placeholders; the final checkpoint plan replaces them atomically when the maintainer generates the key.

**Plan structure suggestion (planner-discretion):**

| Plan | Files | Autonomous? | Notes |
|------|-------|-------------|-------|
| 24-01 | `.github/workflows/release.yml` | Yes | Job-level permissions block + `sigstore/cosign-installer` + `cosign sign-blob` + `actions/attest-build-provenance` + `mv` step + softprops `draft: true` + 4-file `files:` list |
| 24-02 | `docs/RELEASING.md` (new) | Yes | Maintainer-side procedure; uses `<FINGERPRINT-TBD>` placeholders |
| 24-03 | `SECURITY.md` | Yes | Append `### Release tarball signatures + provenance (v1.6 onward)` subsection; uses `<FINGERPRINT-TBD>` placeholder at the anchor |
| 24-04 | `CONTRIBUTING.md` | Yes | One-line cross-ref to docs/RELEASING.md |
| 24-05 | `docs/pgp/<FINGERPRINT>.asc` (new) + `SECURITY.md` + `docs/RELEASING.md` (placeholder replacement) | NO — `checkpoint:human-verify` | Maintainer generates YubiKey key, commits .asc, replaces `<FINGERPRINT-TBD>` placeholders in 2 files. Final atomic commit. |

24-01 through 24-04 form Wave 1 (parallel-safe — different files). 24-05 is Wave 2 (depends on all of Wave 1 + maintainer's physical YubiKey action). Plan-checker tooling and the planner pick the exact ordering.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Tarball production | CI / GitHub Actions runner (release.yml `build` job) | — | Existing tier; Phase 24 does not move it. |
| Cosign blob signature | CI / GitHub Actions runner | Sigstore Fulcio (cert) + Sigstore Rekor (transparency log) | OIDC keyless ties signing identity to the workflow file; sigstore public goods supply the trust root. |
| SLSA v1.0 provenance attestation | CI / GitHub Actions runner | GitHub Attestations API + on-disk .sigstore file | Two-path UX per D-04: `gh attestation verify` (API path) + `cosign verify-attestation` (offline bundle path). |
| `.bundle` + `.sigstore` Release asset upload | CI / GitHub Actions runner via softprops/action-gh-release | GitHub Releases storage | Same upload step as the existing tarball + `.sha256` upload. |
| Detached PGP signature | Maintainer's local machine + YubiKey | — | Out of CI entirely per D-06 — putting the key in GH Secrets defeats SIGN-03's non-OIDC alternative rationale. |
| PGP key custody | YubiKey 5 OpenPGP applet (maintainer-held physical token) | Offline revocation cert (USB + paper) | Hardware key isolation; private key never on a host that runs untrusted code. |
| PGP public-key publication | `docs/pgp/<fp>.asc` in this repo + WKD on `<owner>.github.io` + `keys.openpgp.org` | — | Three publication channels; operators pick whichever they can reach. |
| Maintainer-side release procedure docs | `docs/RELEASING.md` (new) | — | Audience-separated from SECURITY.md (operators) and CONTRIBUTING.md (contributors). |
| Operator-facing verification recipes | `SECURITY.md` `## Supply-chain status` | — | Single canonical source; appended additively to the Phase 23 D-05 skeleton. |

---

## Package Legitimacy Audit

Phase 24 introduces ZERO new third-party Rust crates, npm packages, PyPI packages, or system packages. The only new "dependencies" are:

| Item | Type | Audit |
|------|------|-------|
| `sigstore/cosign-installer` | GitHub Action | Reused at the EXACT SHA Phase 23 pinned (`7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1`). Phase 23 audit applies. No new audit needed; Phase 23's `sigstore-pin-check` CI gate enforces. |
| `actions/attest-build-provenance` | GitHub Action | Reused at the EXACT SHA Phase 23 pinned (`96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0`). Phase 23 audit applies. |
| `cosign` v2.6.3 binary | Installed on runner by cosign-installer | Already audited in Phase 23 SECURITY.md operator-side pin range `>= 2.6.3, < 3.0.0`. |

**slopcheck not run** because no new package-registry artifacts are introduced. The two GitHub Actions are pinned to commit SHAs already enforced by an existing CI gate; no slopcheck-style verification surface remains.

**Disposition:** No packages added — audit not applicable.

---

## Architecture Patterns

### System Architecture Diagram

```
                  [git tag vX.Y.Z push]
                          ↓
              ┌───────────────────────────┐
              │  release.yml `build` job  │
              │  if: refs/tags/*          │
              └──────────────┬────────────┘
                             ↓
       ┌──── existing path (Phases 22 + before) ────┐
       │ checkout → rust toolchain → cargo cache    │
       │ → read-base-digests → cargo build --release│
       │ → tar czf blindjoin-linux-amd64.tar.gz     │
       │ → sha256sum ... > .sha256                  │
       └────────────────────┬──────────────────────┘
                            ↓
       ┌──── NEW in Phase 24 ───────────────────────┐
       │ install cosign 2.6.3 (cosign-installer)    │
       │   ↓                                        │
       │ cosign sign-blob --yes --bundle <out>      │ ──→ Fulcio (cert) + Rekor (tlog)
       │   → blindjoin-linux-amd64.tar.gz.bundle    │
       │   ↓                                        │
       │ actions/attest-build-provenance            │ ──→ GitHub Attestations API (Path A)
       │   subject-path: <tarball>                  │
       │   → ${RUNNER_TEMP}/<bundle>                │
       │   ↓                                        │
       │ mv ${{ outputs.bundle-path }}              │
       │   → blindjoin-linux-amd64.tar.gz.sigstore  │ (Path B asset)
       └────────────────────┬──────────────────────┘
                            ↓
       ┌──── existing softprops upload (modified) ──┐
       │ softprops/action-gh-release                │
       │   draft: true                              │
       │   files: tarball, .sha256, .bundle, .sigstore
       └────────────────────┬──────────────────────┘
                            ↓
                  [GitHub Release (DRAFT) created]
                            ↓
                ── workflow ends — CI is done ──
                            ↓
       ┌──── maintainer's local machine (POST-CI) ──┐
       │ gh release download tarball                │
       │   ↓                                        │
       │ gpg --detach-sign --armor --local-user <fp>│ ──→ YubiKey (ed25519 sign)
       │   → blindjoin-linux-amd64.tar.gz.asc       │
       │   ↓                                        │
       │ gh release upload <tag> ...asc             │
       │   ↓                                        │
       │ gh release edit <tag> --draft=false        │
       └────────────────────┬──────────────────────┘
                            ↓
       [Operator pulls 5 assets: tarball + .sha256
        + .bundle + .sigstore + .asc]
                            ↓
           Operator picks ANY verification path:
           ──────────────────────────────────────
           Path A: cosign verify-blob --bundle ...
           Path B: gh attestation verify ... --repo
           Path C: cosign verify-attestation --bundle .sigstore
           Path D: gpg --verify ...asc ...tar.gz
           (ANY one path is sufficient — D-12)
```

### Pattern 1: Comments-as-contract above structural blocks

**What:** every workflow file has detailed prose comments above `env:` / `on:` / `permissions:` / `jobs:` blocks (verified at [release.yml:3-15](.github/workflows/release.yml#L3), [release.yml:19-26](.github/workflows/release.yml#L19), [release.yml:64-66](.github/workflows/release.yml#L64) and the parallel docker.yml structure).
**When to use:** EVERY structural addition Phase 24 makes (permissions block, new sign/attest steps) MUST grow a prose comment header above it. Auditor-grepable; cause-to-effect-to-source reasoning.
**Example (verified pattern from docker.yml:61-72 — adapt for release.yml):**
```yaml
# Phase 24 SIGN-01/02: cosign keyless signing + actions/attest-build-provenance need:
#   - contents:     write — softprops uploads Release assets.
#   - id-token:     write — OIDC token for Fulcio cert exchange. Without this,
#                   cosign sign-blob fails with the opaque "fulcio: 400 Bad
#                   Request" error. See PITFALLS Pitfall 2 + Phase 23 D-02.
#   - attestations: write — persist the SLSA provenance attestation to GitHub's
#                   attestations API. Without this, actions/attest-build-provenance
#                   fails with 403 Forbidden. See Phase 23 RESEARCH §2.1.
# Deliberately omitted (auditor-grepable per Plan 22-04): packages, PR-write,
# pages, issues, deployments. These tokens MUST NOT appear anywhere in this file.
permissions:
  contents: write
  id-token: write
  attestations: write
```

### Pattern 2: SHA-pin trailing-comment with TWO spaces

**What:** every `uses:` line is `<owner>/<action>@<40-hex> # v<X.Y.Z>` with TWO spaces before the `#`. Enforced by Phase 23 `sigstore-pin-check`.
**When to use:** the two new `uses:` lines (cosign-installer + attest-build-provenance) in `release.yml`.
**Example:**
```yaml
- uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1
  with:
    cosign-release: 'v2.6.3'
- uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0
  with:
    subject-path: blindjoin-linux-amd64.tar.gz
```

### Pattern 3: Step-name convention "<verb> ... — <REQ-ID>"

**What:** Phase 23 named steps "Sign image with cosign (keyless OIDC) — ATTEST-01" + "Attest SBOM (ATTEST-03)" etc. The "— <REQ-ID>" suffix makes every step auditor-traceable to a requirement.
**When to use:** Phase 24's three new sign/attest steps (and the rename step).
**Recommended names:**
- "Install cosign (keyless signing toolchain)" (no REQ-ID — infrastructure step)
- "Sign tarball with cosign (keyless OIDC) — SIGN-01"
- "Attest tarball build provenance — SIGN-02"
- "Rename provenance bundle to .sigstore Release asset filename" (no REQ-ID — RESEARCH §3.2 correction infrastructure)
- "Upload to GitHub Releases (draft — maintainer flips out of draft after PGP upload)" (modified existing step)

### Anti-Patterns to Avoid

- **Embedding the literal `output-name:` input on `actions/attest-build-provenance`.** This input does not exist at v3.2.0 — YAML validates but the input is silently ignored, and the bundle still lands at `${RUNNER_TEMP}/...`. Failure mode: softprops upload fails with "file not found" at the rename target. Use the `bundle-path` output + `mv` step.
- **Setting `--no-tlog-upload` on `cosign sign-blob`.** PITFALLS Pitfall 3 explicitly forbids; Rekor is the operator-facing transparency guarantee for SIGN-01.
- **Putting PGP private key material in GitHub Secrets.** Defeats SIGN-03's entire "non-OIDC alternative path" rationale (D-06).
- **Using the literal `pull-requests:` token in the deliberately-omitted-scopes comment.** Phase 22 Plan 22-04 established the auditor-grepable `! grep -q 'pull-requests:'` gate at file level; the comment uses PARAPHRASED tokens (`PR-write`, etc.) to satisfy the gate.
- **Verifying recipes for production v1.6.0 release WITHOUT a v1.6.0-rc.0 fresh-machine rehearsal.** PITFALLS Pitfall 12 + Phase 23 D-06 closure pattern.
- **Hardcoding a "FAKE FINGERPRINT" string in committed files instead of using a clear `<FINGERPRINT-TBD>` placeholder.** Operators encountering a forgotten fake fingerprint silently trust the wrong key. `<FINGERPRINT-TBD>` makes the gap obvious + grep-detectable. Plan 24-05's checkpoint replaces ALL `<FINGERPRINT-TBD>` occurrences atomically.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cosign blob signing | A shell script that invokes `openssl dgst -sha256 ...` and PGPs it | `cosign sign-blob --yes --bundle ...` | The bundle format (sig + Fulcio cert + Rekor inclusion proof in one file) is a published cosign 2.x spec; rolling your own loses the trust-root binding to OIDC and breaks operator-side `cosign verify-blob`. |
| SLSA provenance generation | Hand-rolled in-toto JSON emission | `actions/attest-build-provenance@v3.2.0` | The SLSA v1.0 predicate schema, the in-toto envelope, and the OIDC identity claim are all wired by the action. Hand-rolling drifts from the spec at every minor version. |
| WKD hash computation | Custom z-base32 implementation | `gpg-wks-client --print-wkd-hash <email>` | The hash is `SHA-1(lowercase(local-part)) → z-base32`; getting any step wrong silently breaks operator WKD lookup. Use the canonical tool. |
| ed25519 PGP key generation | OpenSSL + key import dance | `gpg --card-edit` on YubiKey | YubiKey's OpenPGP applet handles ed25519 natively; hand-rolling key generation off-card defeats the entire hardware-isolation point. |
| GitHub Release asset upload | `gh release upload` shell scripting | `softprops/action-gh-release` | The action handles idempotency (re-uploading the same file overwrites cleanly), file-glob matching, and `draft:` / `prerelease:` semantics. Shell scripting drifts. |
| `.sigstore` filename relocation | Skipping the `mv` step and reading `${{ steps.X.outputs.bundle-path }}` in the softprops `files:` list | `mv ${{ outputs.bundle-path }} <deterministic-name>` as a separate step | Deterministic filename is the operator's UX promise; the `${RUNNER_TEMP}/<hash>.json`-shaped name softprops would otherwise ship is operator-hostile. |

**Key insight:** Phase 24's entire value-add is reusing audited, signed, trust-root-rooted primitives (cosign, Sigstore Fulcio/Rekor, GitHub Attestations API, GnuPG, WKD spec). Every "improvement" toward custom logic loses a trust-root binding. The phase's planning work is glue + docs, not crypto.

---

## Runtime State Inventory

Phase 24 is a greenfield-additive phase, not a rename/refactor. No runtime state to migrate.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — Phase 24 introduces signing/attestation, not data persistence | — |
| Live service config | None — no external services with stored blindjoin config exist; Sigstore Fulcio/Rekor are public goods consumed via OIDC at sign time | — |
| OS-registered state | None — no scheduled tasks, daemons, or pm2-style processes touched | — |
| Secrets / env vars | None NEW. `${{ secrets.GITHUB_TOKEN }}` is already injected at [release.yml:107](.github/workflows/release.yml#L107) for softprops; Phase 24 reuses it. NO new secret is added; the PGP private key is on a YubiKey, not in GitHub Secrets (D-06) | — |
| Build artifacts | None — `target/release/coordinator/client/liquidity-bot` and `blindjoin-linux-amd64.tar.gz` are the existing artifacts. Phase 24 adds `.bundle` + `.sigstore` sibling files but does not modify what `cargo build` produces | — |

**Nothing found in any category** — explicitly verified by reading release.yml top-to-bottom + grepping for env-var names, secret references, and persistence patterns.

---

## Common Pitfalls

Phase 24 inherits Pitfalls 1, 2, 3, 4, 5, 12, 13 from `.planning/research/PITFALLS.md` (per CONTEXT.md). The inheritance is clean — every pitfall maps directly. Two NEW Phase-24-specific pitfalls surfaced during research:

### Pitfall 24-A: `actions/attest-build-provenance` has no `output-name` input at v3.2.0

**What goes wrong:** Plan-author embeds `output-name: blindjoin-linux-amd64.tar.gz.sigstore` in the action's `with:` block. YAML validates; action ignores the unknown input silently. The bundle lands at `${RUNNER_TEMP}/<hash>.json`. softprops upload step fails: "file not found: blindjoin-linux-amd64.tar.gz.sigstore".
**Why it happens:** CONTEXT D-14 ASSUMED the input exists (research-time confidence was MEDIUM). Confirmed AGAINST at the pinned SHA's action.yml during this research.
**How to avoid:** Use `bundle-path` output + a separate `mv` step. See §3.2 for the verified YAML.
**Warning signs:** action.yml at the pinned SHA lists exactly 8 inputs; if your YAML has 2+ keys in the `with:` block, you've added an undocumented input.

### Pitfall 24-B: `gpg --verify` exits 0 even without operator trust

**What goes wrong:** Operator runs `gpg --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz`, sees "Good signature", and assumes trust is established. The signature is cryptographically valid BUT the operator's local trust web has not certified the maintainer's key. A swapped public key (e.g., from a compromised WKD `.well-known` directory) produces an equally-"Good" signature.
**Why it happens:** gpg's exit code reflects cryptographic validity, not trust. WARNING text on stderr says "not certified with a trusted signature" — but operators tune it out, especially in scripted environments.
**How to avoid:** SECURITY.md prose MUST direct operators to compare the fingerprint printed by `gpg --verify` (on stderr, the "Primary key fingerprint:" line) against the canonical fingerprint at `<a id="pgp-current"></a>`. For scripted verification: `gpg --status-fd=1 --verify ... | grep VALIDSIG | grep <FULL-40-CHAR-FINGERPRINT>`. The project already uses this pattern at [ci.yml:99-115](.github/workflows/ci.yml#L99) for bitcoind's PGP verification — direct cross-reference.
**Warning signs:** SECURITY.md prose that omits the fingerprint-comparison step. Operators silently trust whatever key WKD returned.

---

## Code Examples

### Verified `release.yml` `build` job delta (the canonical Wave 1 plan target)

```yaml
  build:
    name: Build linux-amd64
    needs: check
    runs-on: ubuntu-latest
    # Publish gate: only run on a real tag push. workflow_dispatch runs
    # check-only (the rehearsal path) and never uploads release artifacts.
    if: startsWith(github.ref, 'refs/tags/')
    # [+ NEW] Phase 24 SIGN-01/02 permissions — see §3.4 above for comment block.
    permissions:
      contents: write
      id-token: write
      attestations: write

    steps:
      # ... existing 5 steps through Package ...
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
        with:
          toolchain: stable
      - uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2
      - name: Read canonical base-image digests
        id: digests
        uses: ./.github/actions/read-base-digests
      - name: Build coordinator and client
        run: cargo build --release --bin coordinator --bin client --bin liquidity-bot
      - name: Package
        run: |
          mkdir -p dist
          cp target/release/coordinator dist/
          cp target/release/client dist/
          cp target/release/liquidity-bot dist/
          tar czf blindjoin-linux-amd64.tar.gz -C dist .
          sha256sum blindjoin-linux-amd64.tar.gz > blindjoin-linux-amd64.tar.gz.sha256

      # [+ NEW] Phase 24 cosign install — Phase 23 SHA reuse.
      - name: Install cosign (keyless signing toolchain)
        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1
        with:
          cosign-release: 'v2.6.3'

      # [+ NEW] Phase 24 SIGN-01 — cosign blob signature.
      - name: Sign tarball with cosign (keyless OIDC) — SIGN-01
        run: cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz

      # [+ NEW] Phase 24 SIGN-02 — SLSA v1.0 provenance attestation.
      - name: Attest tarball build provenance — SIGN-02
        id: provenance
        uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0
        with:
          subject-path: blindjoin-linux-amd64.tar.gz

      # [+ NEW] Phase 24 SIGN-02 (rename) — RESEARCH §3.2 correction.
      - name: Rename provenance bundle to .sigstore Release asset filename
        run: mv "${{ steps.provenance.outputs.bundle-path }}" blindjoin-linux-amd64.tar.gz.sigstore

      # [~ MODIFIED] Phase 24 D-07 + D-15 — draft: true + 4-file files: list.
      - name: Upload to GitHub Releases (draft — maintainer flips out of draft after PGP upload)
        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
        with:
          draft: true
          files: |
            blindjoin-linux-amd64.tar.gz
            blindjoin-linux-amd64.tar.gz.sha256
            blindjoin-linux-amd64.tar.gz.bundle
            blindjoin-linux-amd64.tar.gz.sigstore
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

This is the COMPLETE Wave 1 plan target. ~35 net new lines (counting comment blocks); ~5 lines modified on the existing softprops step.

---

## State of the Art

| Old approach | Current approach | When changed | Impact |
|--------------|------------------|--------------|--------|
| `bitcoincore-rpc` Rust crate | Project doesn't use Bitcoin Core RPC in the sign path (this row is N/A for Phase 24) | — | — |
| Sigstore cosign 1.x with discrete `.sig` + `.crt` files | cosign 2.x with `--bundle <file>` (sig + cert + Rekor proof in one JSON file) | cosign 2.0, May 2023 | Operators get one file to manage instead of two; Phase 24 D-03 picks `.bundle`. |
| `slsa-framework/slsa-github-generator` reusable workflow | `actions/attest-build-provenance` (GitHub-maintained, inline action) | 2024 (Phase 23 / v1.6 roadmap-level decision per PITFALLS Pitfall 5) | Simpler integration; no workflow restructure. |
| `actions/attest-build-provenance` v3.X self-contained | v4.X wrapper around `actions/attest` | 2026 (per Phase 23 RESEARCH §2.3) | Phase 24 pins v3.2.0 to match Phase 23 (consistency); v1.7 carry-forward to migrate both phases to `actions/attest@v4` as a consolidated step. |
| `gh attestation verify <file> --repo <owner>/<repo>` only | `gh attestation verify <file> [--repo OR --owner]` | gh 2.50 / 2024 | Phase 24 D-16 picks `--repo` for tighter binding; `--owner` is the fallback for org-level verification. |
| PGP signing via subkey on disk | ed25519 primary on YubiKey (hardware isolation) | YubiKey 5 firmware 5.2.3 (2019) made ed25519 native | Phase 24 D-08 picks this; private key never on the host machine. |
| WKD subdomain method (`openpgpkey.<domain>`) | WKD direct method (`<domain>/.well-known/openpgpkey/...`) | Both methods coexist; direct is the only one compatible with `*.github.io` | Phase 24 D-19 picks direct method per GitHub Pages constraint. |
| `cosign 3.0` (not yet adopted operator-side) | `cosign 2.6.3` (Phase 23 + 24 pinned range) | cosign 3.0 released 2026 per Pitfall 13 | Phase 24 stays at 2.x for SECURITY.md operator pin range consistency; cosign 3.0 migration is a v1.7+ quick task. |

**Deprecated / outdated:**
- Hand-rolled `gpg-on-disk` PGP signing for release artifacts: outdated — YubiKey-isolated keys are the modern norm for OSS supply chains.
- Discrete `.sig` + `.crt` cosign blob output: outdated for new adoptions — `.bundle` is the cosign 2.x recommended distribution shape (D-03 confirms).

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `cosign verify-attestation --bundle <.sigstore> --type slsaprovenance ...` prints "Verified OK" but does NOT emit the SLSA predicate JSON to stdout at cosign 2.6.3 (operators wanting the predicate body itself add `--output-file`). | §4.2 | Low — operators following the SECURITY.md recipe expect a verification confirmation, not the predicate body. If wrong, the recipe still WORKS; only the inspect-the-predicate UX is incomplete. Easy 1-liner fix to SECURITY.md if a rehearsal surfaces. |
| A2 | YubiKey 5 firmware ≥ 5.2.3 has been a stable baseline since 2019; maintainer's hardware is compliant. | §5.1 | Low — if the YubiKey is older or lacks ed25519 support, key generation falls back to rsa2048 (compatible). Documentable in docs/RELEASING.md as a fallback. |
| A3 | `<owner>.github.io` repo for WKD publication is NOT verified to exist for `johnzilla`. Phase 24 docs/RELEASING.md treats it as a one-time setup step. | §5.3 | Medium — if the maintainer creates the repo and pushes the WKD file before the first v1.6.0 release, the operator path works. If absent, the WKD path fails and operators fall back to `keys.openpgp.org` (which is the redundant secondary channel per D-10). |
| A4 | `gh 2.50+` is broadly available on operator machines (the SECURITY.md prose targets this minimum). | §4.2 | Low — `gh` 2.50 shipped Q2 2024; almost all current installations meet this. Older versions force the `--repo`-only form, which the SECURITY.md recipe already uses. |
| A5 | The `actions/attest-build-provenance@v3.2.0` action will continue functioning for the lifetime of v1.6 (no GitHub-side API deprecation that breaks the v3.X wrapper). | §3.2 | Low-medium — the v3.X line is in deprecation but currently functional (per Phase 23 RESEARCH §2.3). Carry-forward to `actions/attest@v4` is tracked. |
| A6 | The maintainer accepts the manual "post-CI sign + upload + flip" cadence per release (D-06). | §1 + §5.5 | Locked by user decision; not a research assumption to challenge. |
| A7 | softprops/action-gh-release at the pinned SHA preserves all 4 specified files when re-running on a re-pushed tag (idempotency). | §3.3 | Low — softprops is widely used with re-pushed tags; idempotency is well-tested. |
| A8 | Removing a release tarball from the `files:` list (between v1.6.0-rc.0 and v1.6.0) would NOT delete already-uploaded assets — softprops appends, doesn't sync. Operators of older releases still have access. | §3.3 | Low — this is the documented softprops behavior. |
| A9 | `cosign sign-blob` at cosign 2.6.3 produces a `.bundle` file format compatible with `cosign verify-blob --bundle ...` at cosign 2.6.3+. | §3.1 + §4.1 | Verified by Phase 23 closure precedent for cosign image signing; cosign 2.6.x format is forward-compatible within 2.x. |

**If A1, A3, or A5 are wrong, the planner's mitigation is either (a) a quick-task remediation (A1, A3) or (b) reopening the SHA pin choice (A5 — but unlikely within the v1.6.0 milestone window).**

---

## Open Questions

1. **Does Plan 24-05's checkpoint:human-verify task ALSO include the maintainer's first `keys.openpgp.org` publish + WKD publish, or is that pure docs/RELEASING.md prose?**
   - What we know: D-10 mandates publication to BOTH; §5.1 step 4 + §5.3 document the procedure. The actual publish action is maintainer-side.
   - What's unclear: whether 24-05 includes the publish ACTION as a checkpoint sub-item, or only the .asc commit.
   - Recommendation: 24-05's checkpoint task does NOT execute the publish (those are maintainer-side operations requiring YubiKey touch + email confirmation flow); 24-05 ONLY commits .asc + replaces `<FINGERPRINT-TBD>` placeholders. The publish is the maintainer's first action AFTER 24-05 lands, captured as the v1.6.0-rc.0 release procedure rehearsal. Match Phase 23's HUMAN-UAT pattern.

2. **Does `cosign sign-blob` at cosign 2.6.3 require `--output-signature` + `--output-certificate` explicitly, or does `--bundle <file>` alone suffice?**
   - What we know: `--bundle <output>` is documented to write the cosign 2.x bundle format (sig + cert + Rekor proof) to one file. CONTEXT §"Specifics" recipe matches this.
   - What's unclear: whether `--output-signature` / `--output-certificate` are EXTRA paths (split-file output) or REPLACE `--bundle` (mutually exclusive).
   - Recommendation: use `--bundle` ONLY (the documented single-file path); do NOT add `--output-signature` or `--output-certificate`. If a Plan-write-time rehearsal surfaces a flag-conflict, fall back to discrete output flags but log a NEW Pitfall.

3. **Does the maintainer want one cosign-version pin range update across BOTH the existing image subsection and the new tarball subsection in SECURITY.md, or are they separately maintainable?**
   - What we know: D-12 says "Pitfall 13 callout is a 1-liner cross-ref to Phase 23's section, not duplication" — implies Phase 23's prose is the canonical version pin source.
   - What's unclear: whether the tarball subsection cites the image-side range explicitly, or just says "see image subsection for cosign version pin".
   - Recommendation: 1-line cross-reference: "See the image subsection above for the cosign version pin range; the same constraints apply to tarball verification." Avoids prose duplication; avoids drift if the pin changes.

---

## Environment Availability

Phase 24 introduces three new tool dependencies on the CI runner (already audited via Phase 23) and three new tool dependencies on the MAINTAINER's local machine. Maintainer-side audit:

| Dependency | Required by | Available on maintainer machine | Version | Fallback |
|------------|------------|-------------------------------|---------|----------|
| `cosign` 2.6.3+ | Pre-flight verify of CI-produced .bundle BEFORE flipping draft (§5.5 pre-flight check) | ✗ (verified at research time: `command -v cosign` returns nothing) | — | Maintainer installs at v1.6.0-rc.0 cut: `brew install cosign` (macOS) or download from sigstore/cosign releases. Documented in docs/RELEASING.md prerequisites. |
| `gpg` 2.4+ | YubiKey ed25519 signing + WKD + key import | ✗ (verified at research time: `command -v gpg` returns nothing — gpg not on PATH) | — | Maintainer installs at v1.6.0-rc.0 cut: `brew install gnupg` (macOS) or `apt install gnupg` (Linux). Documented in docs/RELEASING.md prerequisites. |
| `gh` 2.50+ | `gh release download / upload / edit`; pre-flight `gh attestation verify` | ✓ | 2.92.0 (verified: `gh --version` returned `gh version 2.92.0 (2026-04-28)`) | — |

**Missing dependencies with no fallback:** None. All three maintainer-side tools have well-known install paths documented in docs/RELEASING.md as prerequisites.

**Missing dependencies with fallback:** `cosign` and `gpg` are not currently on the maintainer's machine; installation is documented as a one-time prerequisite at v1.6.0-rc.0 cut. NOT a Phase 24 plan item — these are maintainer-side ops.

**CI-side environment:** Phase 23 already audited `sigstore/cosign-installer` provisions cosign on the runner; `actions/attest-build-provenance` requires no host tooling beyond standard GHA. No new CI-side dependencies introduced.

---

## Validation Architecture

**SKIPPED.** `.planning/config.json` has `workflow.nyquist_validation: false` (explicitly disabled). Per the research execution flow, this section is omitted entirely. The plan-checker and execute-phase agents will not expect a Phase-Requirements-to-Test map for Phase 24.

---

## Security Domain

**SKIPPED IN FULL ASVS FORM** — Phase 24's security model is already exhaustively documented elsewhere:
- The image-side analog at Phase 23 RESEARCH (cosign + Fulcio OIDC + Rekor + GitHub Attestations API trust chain) applies directly.
- The PGP supply-chain trust model (YubiKey hardware isolation + WKD publication + keys.openpgp.org + revocation cert) is standard GnuPG ed25519 procedure.
- The OWASP ASVS categories that would apply (V6 Cryptography — never hand-roll) are honored by reusing audited libraries (cosign, GnuPG, sigstore actions) and committed-to-repo SHA-pinned versions.

**Stack-specific ASVS one-table summary (instead of the full block):**

| ASVS Category | Applies | Standard Control in Phase 24 |
|---------------|---------|-----------------------------|
| V2 Authentication | yes (maintainer-side) | YubiKey hardware token; PIN-protected access to the OpenPGP applet |
| V3 Session Management | no | — |
| V4 Access Control | yes (CI permissions) | Job-level `id-token: write` + `attestations: write` (D-02; PITFALLS Pitfall 2); deliberately omitted scopes named in comments |
| V5 Input Validation | yes (cosign verify identity-regexp) | `--certificate-identity-regexp 'release\.yml@refs/tags/v.*'` narrowed enough (PITFALLS Pitfall 1) — not too narrow, not too wide |
| V6 Cryptography | yes | `cosign sign-blob` (RSA/ECDSA via Fulcio cert) + ed25519 PGP (GnuPG). NEVER hand-rolled. |
| V8 Data Protection | yes (PGP private key) | YubiKey hardware isolation; revocation cert stored offline (USB + paper); private key never on host filesystem |

**Known threat patterns the phase mitigates:**

| Pattern | STRIDE | Mitigation in Phase 24 |
|---------|--------|------------------------|
| Compromised GitHub account publishes a backdoored tarball | Spoofing / Tampering | cosign OIDC identity binding (Fulcio cert tied to release.yml workflow file) + PGP detached sig (YubiKey-held, separate trust root) — operator verifies EITHER; both must be compromised together to ship a forged release |
| Sigstore Fulcio/Rekor outage at verify time | Denial of Service | PGP path (D-06) works fully offline once the operator has the WKD-fetched public key cached |
| Maintainer's GitHub account compromised but YubiKey safe | Spoofing | cosign path produces a signature tied to the compromised account (operators using cosign verify trust it); PGP path requires YubiKey touch — operators using PGP verify see no new release. SIGN-03 is the entire reason this scenario is recoverable. |
| Maintainer's YubiKey lost/stolen but GitHub account safe | Spoofing | Revocation cert (D-08) published to keys.openpgp.org + WKD invalidates the old key; cosign path continues to work; new YubiKey generates new key; CHANGELOG entry names the rotation event |
| Slow Rekor inclusion proof acceptance | Repudiation | NOT a concern — Rekor is the operator-side transparency log; CI signs and ships, operators verify against the cached TUF root. The Phase 24 sign flow is fire-and-forget Rekor-side. |
| Replay attack: operator downloads + verifies an old release as if it were current | Tampering | Out of scope for Phase 24 — operators verify the tarball IS what was signed; the "is this the current version?" question is a UX layer (CHANGELOG.md + Releases page metadata) not a signing problem. |

---

## Sources

### Primary (HIGH confidence)
- [release.yml @ 107 LOC](/.github/workflows/release.yml) — integration surface for SIGN-01/02 [VERIFIED: in-repo read]
- [docker.yml @ Phase 23 final state](/.github/workflows/docker.yml) — canonical SHA pins (lines 151, 272) and the permissions/comments pattern (lines 61-77) Phase 24 mirrors [VERIFIED: in-repo read]
- [ci.yml `sigstore-pin-check` job](/.github/workflows/ci.yml#L292) — Phase 23 CI gate that covers release.yml automatically [VERIFIED: in-repo read]
- [actions/attest-build-provenance action.yml @ 96278af6](https://raw.githubusercontent.com/actions/attest-build-provenance/96278af6caaf10aea03fd8d33a09a777ca52d62f/action.yml) — exact inputs/outputs at the pinned SHA: 8 inputs (no `output-name`), 3 outputs (incl. `bundle-path`) [VERIFIED: WebFetch 2026-06-02]
- [softprops/action-gh-release action.yml @ de2c0eb8](https://github.com/softprops/action-gh-release/blob/de2c0eb89ae2a093876385947365aca7b0e5f844/action.yml) — exact inputs at the pinned SHA: 14 inputs including `draft` (#5) [VERIFIED: WebFetch 2026-06-02]
- [Phase 23 CONTEXT.md](/.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-CONTEXT.md) — D-01..D-11 patterns Phase 24 mirrors [VERIFIED: in-repo read]
- [Phase 23 RESEARCH.md](/.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-RESEARCH.md) — §2.1 (attestations: write rationale), §2.3 (SHA pin table), §2.4 (cosign sign vs attest-build-provenance distinction), §3.2 (sign-blob CLI shape) [VERIFIED: in-repo read]
- [Phase 23 23-01-PLAN.md](/.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-01-PLAN.md) — permissions block + comment shape Phase 24 transplants [VERIFIED: in-repo read]
- [PITFALLS.md Pitfalls 1, 2, 3, 4, 5, 12, 13](/.planning/research/PITFALLS.md) — all inherited from Phase 23 [VERIFIED: in-repo read]
- [REQUIREMENTS.md SIGN-01, SIGN-02, SIGN-03 verbatim](/.planning/REQUIREMENTS.md) [VERIFIED: in-repo read]
- [ROADMAP.md §Phase 24 4 SC](/.planning/ROADMAP.md#phase-24) [VERIFIED: in-repo read]
- [SECURITY.md current state at v1.6](/SECURITY.md) — Phase 24 D-12 appends additively to the existing Phase 23 `### Image signatures + attestations (v1.6 onward)` subsection [VERIFIED: in-repo read]

### Secondary (MEDIUM-HIGH confidence)
- [cli.github.com/manual/gh_attestation_verify](https://cli.github.com/manual/gh_attestation_verify) — `--owner` / `--repo` flag semantics for `gh attestation verify` [VERIFIED via WebSearch 2026-06-02; cross-verified against gh 2.92.0 local install]
- [GnuPG wiki WKD page](https://wiki.gnupg.org/WKD) — WKD direct method vs subdomain method, `.well-known/openpgpkey/hu/<hash>` layout [CITED: GnuPG official wiki]
- [GnuPG manual gpg-wks-client](https://www.gnupg.org/documentation/manuals/gnupg-devel/gpg_002dwks_002dclient.html) — `--print-wkd-hash` semantics [CITED: GnuPG official manual]
- [Phase 23 23-RESEARCH.md §2.3 SHA pin table](/.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-RESEARCH.md#L156) — sigstore action SHA + rationale for staying on v3.X (not v4.X) [VERIFIED: in-repo read]

### Tertiary (Knowledge / un-verified at this research session)
- [yubico.com — YubiKey OpenPGP card-edit](https://developers.yubico.com/PGP/Card_edit.html) — ed25519 support since YubiKey firmware 5.2.3 [CITED: Yubico docs URL]
- General GnuPG `--detach-sign --armor --local-user <fingerprint>` semantics [ASSUMED: standard GnuPG procedure; not re-verified in this research session]
- General `cosign 2.6.3 sign-blob` flag semantics: `--yes` (non-interactive), `--bundle <out>` (single-file output) [ASSUMED: Phase 23 in-repo precedent verified `--yes`; sign-blob `--bundle <out>` is the documented cosign 2.x form per CONTEXT specifics, not re-verified at the cosign 2.6.3 binary level in this research session]

---

## Metadata

**Confidence breakdown:**
- Standard stack (sigstore actions, cosign, GnuPG): HIGH — reuses Phase 23's pinned SHAs verbatim; no new packages.
- SHA pin currency: HIGH — verified no security advisory at pinned SHAs as of 2026-06-02.
- `actions/attest-build-provenance` output mechanism: HIGH (corrected from CONTEXT D-14 MEDIUM assumption) — verified against action.yml at pinned SHA.
- `softprops/action-gh-release` `draft:` support: HIGH — verified at pinned SHA.
- `gh attestation verify` CLI shape: HIGH — verified at locally-installed `gh 2.92.0`.
- WKD layout: MEDIUM — verified against GnuPG manual; not verified against an existing `<owner>.github.io` repo.
- PGP-on-YubiKey ed25519 ceremony: HIGH — standard procedure; not project-specific.
- `cosign verify-attestation --type slsaprovenance` recipe precise output shape: MEDIUM — recipe shape is correct; the predicate JSON output detail (Assumption A1) is unverified.

**Research date:** 2026-06-02
**Valid until:** 2026-09-01 (90 days). Re-research before merging Plan 24-XX if:
- Sigstore project ships a security advisory affecting `sigstore/cosign-installer v3.10.1` or `actions/attest-build-provenance v3.2.0`.
- GitHub Attestations API API-level deprecates the v3.X attest-build-provenance wrapper.
- `gh` CLI removes `--owner` or `--repo` from `gh attestation verify`.
- cosign 3.0 ships and Phase 23's image-side SECURITY.md pin range is updated (Phase 24's tarball subsection must follow).

## RESEARCH COMPLETE

**Phase:** 24 - Release Tarball Signing (cosign + SLSA + PGP)
**Confidence:** HIGH

### Key Findings

- **D-14 CORRECTION (critical for planner):** `actions/attest-build-provenance@v3.2.0` does NOT have an `output-name` input — verified against action.yml at pinned SHA `96278af6...`. The plan MUST capture `${{ steps.<id>.outputs.bundle-path }}` and use a separate `mv` step to relocate the bundle to `blindjoin-linux-amd64.tar.gz.sigstore`. Without this, softprops upload fails on the missing `.sigstore` file.
- **D-18 VERIFIED:** `softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1` supports `draft: true` (input #5 of 14 declared inputs).
- **D-13 VERIFIED:** Phase 23 SHA pins for `sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` (with `cosign-release: 'v2.6.3'`) and `actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0` carry no known security advisories as of 2026-06-02; reuse verbatim.
- **D-16 VERIFIED:** `gh attestation verify` at gh 2.92.0 supports BOTH `--repo <owner>/<repo>` AND `--owner <owner>`; recommended form for blindjoin is `--repo johnzilla/blindjoin` (tighter identity binding).
- **D-19 VERIFIED:** WKD direct method `https://<owner>.github.io/.well-known/openpgpkey/hu/<wkd-hash>` is the only viable WKD path on GitHub Pages (subdomain method needs DNS control). `gpg-wks-client --print-wkd-hash <email>` produces the hash. `<owner>.github.io` repo existence is NOT verified — flagged as a one-time maintainer setup step in docs/RELEASING.md.
- **Plan structure recommendation:** 5 plans, 4 autonomous (24-01 release.yml, 24-02 docs/RELEASING.md, 24-03 SECURITY.md, 24-04 CONTRIBUTING.md) + 1 `checkpoint:human-verify` (24-05) committing `docs/pgp/<fp>.asc` + replacing `<FINGERPRINT-TBD>` placeholders atomically. Mirrors Phase 23's physical-action-isolated final-plan pattern.
- **Two NEW pitfalls surfaced:** Pitfall 24-A (no `output-name` on attest-build-provenance v3.2.0) and Pitfall 24-B (`gpg --verify` exits 0 without operator trust — fingerprint comparison required). Both documented for plan-author + operator awareness.

### File Created
`.planning/phases/24-release-tarball-signing-cosign-slsa-pgp/24-RESEARCH.md`

### Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | Reuses Phase 23 SHA pins verbatim; no new packages. Pinned SHAs spot-checked against action.yml @ SHA. |
| Architecture | HIGH | Phase 24 is a direct mirror of Phase 23's `docker.yml` patterns transplanted into `release.yml`. The integration surface is well-understood and well-instrumented. |
| Pitfalls | HIGH | All 7 inherited pitfalls map cleanly + 2 NEW Phase-24-specific pitfalls verified during research (no `output-name` input + gpg trust gap). |
| PGP procedure | MEDIUM-HIGH | YubiKey + ed25519 + WKD direct method is standard GnuPG; the only uncertainty is whether `<owner>.github.io` exists. |
| Operator recipes | MEDIUM-HIGH | All 4 recipes (cosign verify-blob, gh attestation verify, cosign verify-attestation, gpg --verify) verified against the action / CLI versions in use; A1 caveat on predicate JSON output. |

### Open Questions

1. Does Plan 24-05 also execute the maintainer's first `keys.openpgp.org` publish + WKD publish? — recommendation: NO (matches Phase 23 HUMAN-UAT-at-tag pattern).
2. Does `cosign sign-blob` at 2.6.3 require `--bundle` ONLY or ALSO `--output-signature` + `--output-certificate`? — recommendation: `--bundle` alone; rehearse and add a Pitfall if conflict.
3. Does SECURITY.md tarball subsection cite the image-side cosign-version range explicitly or via 1-line cross-ref? — recommendation: 1-line cross-ref to avoid drift.

### Ready for Planning

Research complete. Planner can now create PLAN.md files for 5 plans (4 autonomous + 1 checkpoint:human-verify). All planner-discretion items (D-13 through D-20) are resolved with explicit verification or fallback rationale. The Wave 1 release.yml delta is provided as a complete YAML block; the Wave 1 `docs/RELEASING.md` skeleton is provided as a structured outline; the Wave 1 SECURITY.md additive subsection has its prose + recipe + anchor specifications laid out; the Wave 1 CONTRIBUTING.md cross-ref is a 1-line drop-in. The Wave 2 checkpoint:human-verify task has its placeholder-replacement contract explicitly specified.
