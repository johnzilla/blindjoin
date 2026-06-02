# Phase 24: Release Tarball Signing (cosign + SLSA + PGP) — Pattern Map

**Mapped:** 2026-06-02
**Files analyzed:** 5 (3 modify, 2 create)
**Analogs found:** 5 / 5 — all in-repo; Phase 24 is a direct mirror of Phase 23 patterns transplanted from `docker.yml` to `release.yml`

> **Read-this-first:** RESEARCH.md §3.2 materially corrects CONTEXT.md D-14.
> `actions/attest-build-provenance@v3.2.0` has NO `output-name` input. The
> bundle path is exposed via the `bundle-path` output and MUST be relocated
> with a separate `mv` step. This adds ONE step Phase 23's docker.yml block
> did not need — see **Pattern Assignment #4 (Rename provenance bundle)**.
>
> Phase 23 produced the canonical patterns (permissions block, SHA-pin
> comment style, comments-as-contract, recipes block shape). Phase 24's
> entire pattern map is "copy from `docker.yml`, swap `docker.yml` →
> `release.yml` in the identity-regexp, swap subject-name/digest plumbing
> for `subject-path: <tarball>`, add the `mv` step." No novel structural
> patterns are introduced beyond the `mv` step and the PGP key custody +
> WKD publication procedures (which are pure-docs and have no in-repo
> structural analog — they cite GnuPG canonical procedures).

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `.github/workflows/release.yml` (MODIFY) | workflow — add job-level `permissions:` block + 4 new steps + modify softprops step | request-response (tag push → build + sign + attest + upload) | `.github/workflows/docker.yml` (Phase 23 final state) — same shape; same sigstore actions at same SHA pins | exact (transplant cosign + attest-build-provenance pattern; only subject-shape differs: `subject-path` for blob vs `subject-name + subject-digest` for OCI) |
| `SECURITY.md` (MODIFY) | docs / policy — append `### Release tarball signatures + provenance (v1.6 onward)` subsection under `## Supply-chain status`, below Phase 23's `### Image signatures + attestations (v1.6 onward)` | n/a | `SECURITY.md` lines 118-185 (Phase 23 `### Image signatures + attestations (v1.6 onward)` subsection) | exact — Phase 24 mirrors the H3 header shape, opening prose, numbered claim list, prerequisites paragraph, fenced bash recipes block, `> Note:` callouts |
| `CONTRIBUTING.md` (MODIFY) | docs — one-line cross-reference to `docs/RELEASING.md` at end of `## Tagging releases` section | n/a | `CONTRIBUTING.md` lines 69-94 (existing `## Tagging releases` section style) | exact (insertion point inside existing section; one-line addition) |
| `docs/RELEASING.md` (CREATE) | docs — maintainer-side release procedure | n/a | no in-repo analog for maintainer-side release docs; closest by shape is `docs/AUDIT-CHARTER.md` (long-form `docs/` policy file) for H1 + H2 ToC + prose style | partial — borrows top-level shape from `docs/AUDIT-CHARTER.md`; content is novel (GnuPG canonical procedures) |
| `docs/pgp/<FULL-40-CHAR-FINGERPRINT>.asc` (CREATE) | armored ed25519 public key | n/a | no in-repo analog (first PGP key file in the project) | no analog — file is a `gpg --export --armor` output; format is the OpenPGP ASCII Armor spec |

**No new code files; no new composite actions.** Phase 24 mirrors Phase 23's "modify existing workflow + add docs" scope discipline. Phase 23's `sigstore-pin-check` job in `ci.yml` ALREADY covers `release.yml` (it greps `.github/workflows/`) — no new CI gate required.

---

## Pattern Assignments

Order mirrors the step ordering in `release.yml`'s `build` job AFTER Phase 24 edits land. Each `#` corresponds to a structural change; analogs from `docker.yml` (Phase 23 final state) or the existing `release.yml` are cited with file:line excerpts.

| # | New step / change | REQ-ID | Analog | Section |
|---|-------------------|--------|--------|---------|
| 1 | Job-level `permissions:` block on `build` (replaces inherited workflow-default) | (D-02 invariant) | `docker.yml:61-77` (Phase 23 docker job permissions) | §1 |
| 2 | Install cosign — `sigstore/cosign-installer@<sha>` step | (toolchain) | `docker.yml:143-153` (Phase 23 ATTEST-01 installer) | §2 |
| 3 | Sign tarball with cosign — `cosign sign-blob --yes --bundle ...` | SIGN-01 | `docker.yml:175-179` (Phase 23 `cosign sign` for image) | §3 |
| 4 | Attest tarball build provenance — `actions/attest-build-provenance@<sha>` (note: `subject-path` not `subject-name + subject-digest`) | SIGN-02 | `docker.yml:237-276` (Phase 23 ATTEST-02 image provenance) | §4 |
| 5 | NEW step (no Phase 23 analog): rename bundle to deterministic `.sigstore` filename via `mv ${{ steps.X.outputs.bundle-path }} ...` | (SIGN-02 RESEARCH §3.2 correction) | closest analog is the inline shell-in-step pattern at `ci.yml:62-118` (multi-line `run: |` with env-style variable expansion) | §5 |
| 6 | Modify existing softprops step: add `draft: true`, expand `files:` from 2 to 4 entries | SIGN-01 + SIGN-02 (delivery) | `release.yml:100-107` (existing softprops step — modify in place) | §6 |
| 7 | `SECURITY.md` append `### Release tarball signatures + provenance (v1.6 onward)` subsection + fingerprint anchor | SIGN-01/02/03 documentation | `SECURITY.md:118-185` (Phase 23 image subsection — exact mirror) | §7 |
| 8 | `CONTRIBUTING.md` one-line cross-ref to `docs/RELEASING.md` | (D-20) | `CONTRIBUTING.md:69-94` (existing `## Tagging releases` section — append at end) | §8 |
| 9 | NEW file `docs/RELEASING.md` — maintainer-side procedure | SIGN-03 (procedural) | `docs/AUDIT-CHARTER.md` (long-form docs/ file shape; `# Title` + H2 ToC) | §9 |
| 10 | NEW file `docs/pgp/<FP>.asc` — armored ed25519 public key | SIGN-03 (artifact) | no analog | §10 |

---

### §1. Job-level `permissions:` block — D-02 invariant

**Analog:** `docker.yml:61-77` (Phase 23 final state) — the canonical "job-level scope-additive permissions with auditor-grepable deliberately-omitted-scopes comment block" pattern.

**Source-of-truth excerpt** (`.github/workflows/docker.yml` lines 60-77):

```yaml
  docker:
    name: Docker ${{ matrix.image }}
    needs: check
    runs-on: ubuntu-latest
    # Publish gate: only run on a real tag push. workflow_dispatch runs
    # check-only (the rehearsal path) and never pushes images to ghcr.io.
    if: startsWith(github.ref, 'refs/tags/')
    # Phase 23 ATTEST-01/02/03: cosign keyless signing + attest-* actions need:
    #   - contents:     read — checkout the source tree.
    #   - packages:     write — push image + sig + attestation to ghcr.io.
    #   - id-token:     write — OIDC token for Fulcio cert exchange. Without this,
    #                   cosign sign fails with the opaque "fulcio: 400 Bad
    #                   Request" error. See PITFALLS Pitfall 2.
    #   - attestations: write — persist the attestation to GitHub's attestations
    #                   API. Without this, actions/attest-build-provenance fails
    #                   with 403 Forbidden on the API call. RESEARCH §2.1
    #                   correction — CONTEXT D-02 omitted this; it is required.
    # Deliberately omitted (auditor-grepable per Plan 22-04): PR-write, pages,
    # issues, deployments. These tokens MUST NOT appear anywhere in this file.
    permissions:
      contents: read
      packages: write
      id-token: write
      attestations: write
```

**Pattern to copy** (Phase 24 modification to `release.yml`):

- Multi-line `#` comment block IMMEDIATELY ABOVE the `permissions:` key, citing every scope and the failure mode if absent.
- Cause-to-effect-to-source structure: each scope gets one line naming WHAT it enables, then a sub-line naming HOW it fails if missing + the design-record cross-ref (`PITFALLS Pitfall 2`, `Phase 23 RESEARCH §2.1`).
- Deliberately-omitted-scopes line uses PARAPHRASED tokens (`PR-write`, `pages`, `issues`, `deployments`) — NOT the literal `pull-requests:` or `pages:` with colon. Phase 22 Plan 22-04 lesson: any future file-level audit gate `! grep -q 'pull-requests:'` continues to hold.

**Destination shape for Phase 24** (RESEARCH §3.4 — placement: between the existing `if: startsWith(github.ref, 'refs/tags/')` line at `release.yml:66` and the `steps:` key at `release.yml:68`):

```yaml
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

**Key differences from docker.yml's block:**

- `contents: write` (not `read`) — softprops needs write to upload Release assets.
- `packages` is in the deliberately-omitted list (Phase 24 does NOT push to ghcr.io; only Phase 23's `docker.yml` does).
- Workflow-level `contents: write` at `release.yml:28-29` STAYS untouched (still needed by the `check` job's implicit grant).

---

### §2. Install cosign — `sigstore/cosign-installer` step

**Analog:** `docker.yml:143-153` (Phase 23 ATTEST-01 cosign-installer step). Phase 24 reuses the EXACT SHA pin and `cosign-release:` version verbatim (RESEARCH §2.1 — single source of truth).

**Source-of-truth excerpt** (`.github/workflows/docker.yml` lines 143-153):

```yaml
      # Phase 23 ATTEST-01: cosign keyless OIDC signing toolchain.
      # Installer pinned to v3.10.1 (last v3.X — see RESEARCH §2.3). v4.X
      # of cosign-installer mandates cosign 3.x which is operator-incompatible
      # with the documented `>= 2.6.3, < 3.0.0` range in SECURITY.md (D-08).
      # cosign-release pinned to v2.6.3 (latest 2.x stable; no CLI-breaking
      # changes from 2.5.x through 2.6.x verified at sigstore/cosign release
      # notes). Pin contract enforced by Plan 23-03 sigstore-pin-check gate.
      - name: Install cosign (keyless signing toolchain)
        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1
        with:
          cosign-release: 'v2.6.3'
```

**Pattern to copy:**

- Multi-line `#` comment block above the step (Phase 24 inherits Phase 23's prose — same pin range rationale, same `sigstore-pin-check` gate enforcement).
- `name:` is the same human-readable phrase (`Install cosign (keyless signing toolchain)`) — Phase 23 already established this naming pattern.
- `uses: <owner>/<action>@<40-hex>  # v<X.Y.Z>` with TWO spaces before `#` (Phase 23 PATTERNS §"Shared #2", enforced by `sigstore-pin-check`).
- `cosign-release: 'v2.6.3'` quoted string (Phase 23 form verified at the SHA).

**Destination shape for Phase 24** (RESEARCH §3.4 — placement: AFTER the existing `Package` step at `release.yml:91-98`, BEFORE the new sign-blob step):

```yaml
      # Phase 24 SIGN-01: cosign keyless OIDC signing toolchain — pin reuses
      # Phase 23 docker.yml SHA verbatim (single source of truth; if either
      # sigstore action rotates, both workflows update together via the
      # sigstore-pin-check gate at ci.yml:292-326 which greps all of .github/workflows/).
      - name: Install cosign (keyless signing toolchain)
        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1
        with:
          cosign-release: 'v2.6.3'
```

---

### §3. Sign tarball with cosign — SIGN-01

**Analog:** `docker.yml:175-179` (Phase 23 ATTEST-01 `cosign sign` for the image). Phase 24's sign-blob differs in (a) `sign-blob` subcommand (not `sign`), (b) literal file path subject (not OCI `${IMAGE}@${DIGEST}`), (c) `--bundle <file>` flag to write the bundle to disk (the image-side path uses registry tag conventions instead), (d) no `env:` block needed (the single tarball filename doesn't warrant variable extraction).

**Source-of-truth excerpt** (`.github/workflows/docker.yml` lines 155-179):

```yaml
      # Phase 23 ATTEST-01: produces ghcr.io/.../blindjoin-<image>:sha256-<HEX>.sig
      # — a Fulcio-issued cert bound to the GHA OIDC subject claim, plus the
      # Rekor transparency-log inclusion proof. Operator-side `cosign verify`
      # recipe in SECURITY.md verifies against the locked
      # --certificate-identity-regexp (see Plan 23-04 SECURITY.md rewrite).
      #
      # NOTE: cosign sign and actions/attest-build-provenance are NOT
      # interchangeable — they produce different OCI artifacts under different
      # tag conventions (<digest>.sig vs sha256-<HEX> referrer manifest).
      # ATTEST-01 specifically requires the <digest>.sig form; Plan 23-02
      # delivers ATTEST-02 (provenance) + ATTEST-03 (SBOM) via attest-* actions.
      # See RESEARCH §2.4 (NEW Pitfall §7.3) for the distinction.
      #
      # --yes: non-interactive; required in CI (would otherwise prompt for
      # transparency-log consent).
      # Re-running this step on the same digest is idempotent at the registry
      # (cosign 2.x stores at content-addressed `sha256-<HEX>.sig`; re-sign
      # produces a new Rekor entry but the registry tag is overwritten safely).
      # See PITFALLS Pitfall 3: the tlog-upload flag MUST NOT be disabled —
      # Rekor is the operator-facing transparency guarantee for ATTEST-01.
      - name: Sign image with cosign (keyless OIDC) — ATTEST-01
        env:
          IMAGE: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          DIGEST: ${{ steps.build.outputs.digest }}
        run: cosign sign --yes "${IMAGE}@${DIGEST}"
```

**Pattern to copy:**

- Multi-line `#` comment block above the step naming the REQ-ID, the artifact produced, the operator-side verify pointer, and PITFALLS cross-refs (Pitfall 3 — Rekor mandatory).
- Step name follows `<verb> <object> with <tool> (<modifier>) — <REQ-ID>` convention (Phase 23 step-name pattern, also documented in `PATTERNS §3` of RESEARCH).
- `--yes` flag inline (non-interactive CI requirement).
- No `--no-tlog-upload` flag (PITFALLS Pitfall 3 — Rekor mandatory).

**Destination shape for Phase 24** (RESEARCH §3.1):

```yaml
      # Phase 24 SIGN-01: cosign keyless OIDC blob signing.
      # Produces blindjoin-linux-amd64.tar.gz.bundle (sig + cert + Rekor inclusion
      # proof in cosign 2.x bundle format). Operator verifies via:
      #   cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \
      #     --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
      #     --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
      #     blindjoin-linux-amd64.tar.gz
      # (full recipe lives in SECURITY.md per D-12)
      #
      # --yes:  non-interactive; required in CI (would otherwise prompt for
      #         transparency-log consent).
      # --bundle <file>: writes the cosign 2.x bundle format to a file (sig + cert
      #         + Rekor proof in one JSON file). Operator passes the same file
      #         via --bundle to verify-blob.
      # See PITFALLS Pitfall 3: tlog upload MUST NOT be disabled.
      - name: Sign tarball with cosign (keyless OIDC) — SIGN-01
        run: cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz
```

**Key differences from docker.yml's `cosign sign` step:**

- Subcommand: `sign-blob` (not `sign`).
- No `env:` block (single literal file path; no variable interpolation needed).
- `--bundle <output>` flag (writes the bundle to a file path on disk; image-side path stores at registry-tag convention instead).
- Identity regex in the verify recipe (referenced by the comment, not by the step) targets `release\.yml`, not `docker\.yml` (Phase 24's workflow file).

---

### §4. Attest tarball build provenance — SIGN-02

**Analog:** `docker.yml:237-276` (Phase 23 ATTEST-02 `actions/attest-build-provenance` for the image). Phase 24's differs in (a) `subject-path:` (single key) replaces image-side `subject-name:` + `subject-digest:` (two keys), (b) NO `push-to-registry:` input (Phase 24 isn't pushing to an OCI registry — the attestation lives at the GH Attestations API + the on-disk `.sigstore` bundle), (c) the action ALSO emits a bundle file path via `outputs.bundle-path` that the next step (§5) consumes.

**Source-of-truth excerpt** (`.github/workflows/docker.yml` lines 237-276):

```yaml
      # Phase 23 ATTEST-02: SLSA v1.0 in-toto build provenance attestation.
      # Predicate type emitted: https://slsa.dev/provenance/v1 (auto-derived
      # from the workflow context — no explicit predicate-type input needed).
      # The action names the builder workflow (docker.yml), the tag ref, the
      # source commit, and the runner image from the GHA context automatically.
      #
      # RESEARCH §2.4 + §7.3 — cosign sign (Plan 23-01) and attest-build-
      # provenance are NOT interchangeable. They produce DIFFERENT OCI artifacts
      # under different tag conventions: cosign sign produces <digest>.sig
      # (ATTEST-01 requirement), while attest-build-provenance produces a
      # sha256-<HEX> referrer manifest (ATTEST-02 requirement). BOTH steps are
      # required; they satisfy distinct REQ-IDs.
      #
      # [... shortened: retrieval prose, Pitfall 5 lock, v3.X-not-v4.X pin rationale ...]
      #
      # push-to-registry: true (D-01 inline registry-attached path). No extra
      # with: inputs beyond the three locked ones — predicate-type, workflow-
      # file, tag-ref, and source-commit are all auto-derived from workflow
      # context (RESEARCH §3.4).
      - name: Attest build provenance (ATTEST-02)
        uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0
        with:
          subject-name: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          subject-digest: ${{ steps.build.outputs.digest }}
          push-to-registry: true
```

**Pattern to copy:**

- Multi-line `#` comment block citing predicate type (auto-derived) + workflow-context auto-population + Pitfall 5 lock (don't add `slsa-framework/slsa-github-generator`).
- Step name follows `<verb> <object> — <REQ-ID>` convention.
- `uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0` — Phase 24 reuses Phase 23's exact SHA pin verbatim.
- TWO spaces before `#` in the SHA pin trailing comment.

**Destination shape for Phase 24** (RESEARCH §3.2 — note: NO `push-to-registry:` for blob attestation; subject is a file path, not an OCI ref):

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
```

**Key differences from docker.yml's image attest step:**

- `subject-path: <tarball>` (single key, points to a file on disk) replaces image-side `subject-name + subject-digest` (two keys naming the OCI reference + sha256 digest).
- NO `push-to-registry:` — Phase 24 has no OCI registry to push to; the attestation API + on-disk bundle suffice (D-04).
- `id: provenance` is REQUIRED (the next step consumes `${{ steps.provenance.outputs.bundle-path }}`).

---

### §5. Rename provenance bundle to `.sigstore` filename — NEW (no Phase 23 analog)

**Analog:** none directly — Phase 23's `docker.yml` uses `push-to-registry: true` which routes the artifact through the OCI registry under a deterministic referrer name; Phase 24's tarball-side has no registry-mediated rename path. The closest analog is the inline shell step pattern from `release.yml`'s `Package` step (already in-file) or `ci.yml`'s install-bitcoind step (multi-line `run: |` with environment-style variables).

**Source-of-truth excerpt for inline `mv` shell pattern** (`.github/workflows/release.yml` lines 91-98):

```yaml
      - name: Package
        run: |
          mkdir -p dist
          cp target/release/coordinator dist/
          cp target/release/client dist/
          cp target/release/liquidity-bot dist/
          tar czf blindjoin-linux-amd64.tar.gz -C dist .
          sha256sum blindjoin-linux-amd64.tar.gz > blindjoin-linux-amd64.tar.gz.sha256
```

**Pattern to copy:**

- Step `name:` describing the file operation in a single phrase.
- Single-line `run:` for one command (multi-line `run: |` only when there are multiple statements).
- Use `${{ steps.<id>.outputs.<name> }}` for cross-step value plumbing (project-wide pattern — see also `docker.yml:138-139` which reads `${{ steps.digests.outputs.debian_ref }}`).

**Destination shape for Phase 24** (RESEARCH §3.2):

```yaml
      # Phase 24 SIGN-02 (rename): relocate the provenance bundle to the
      # deterministic .sigstore filename that the SECURITY.md operator recipe and
      # the softprops upload files: list reference. The action writes the bundle to
      # a path under ${RUNNER_TEMP}; we mv it to the workspace root so softprops
      # can pick it up. RESEARCH §3.2 correction — the action has no output-name
      # input at v3.2.0.
      - name: Rename provenance bundle to .sigstore Release asset filename
        run: mv "${{ steps.provenance.outputs.bundle-path }}" blindjoin-linux-amd64.tar.gz.sigstore
```

**Why this is a NEW pattern (Pitfall 24-A in RESEARCH §"Common Pitfalls"):** the `output-name` input that CONTEXT D-14 ASSUMED exists is NOT in `action.yml` at `96278af6...`. Without this `mv` step, softprops's `files:` list references a nonexistent filename and fails the workflow at upload time with "file not found." The `mv` is one extra line for material UX clarity (operator sees `blindjoin-linux-amd64.tar.gz.sigstore`, not `${RUNNER_TEMP}/attestation-<hash>.json`).

---

### §6. Modify existing softprops step — D-07 + D-15

**Analog:** `release.yml:100-107` (the EXISTING softprops step — modify in place). The SHA pin and `GITHUB_TOKEN` env wiring stay; the `files:` list grows from 2 to 4 entries; a new `draft: true` key is added.

**Source-of-truth excerpt** (`.github/workflows/release.yml` lines 100-107, BEFORE Phase 24):

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

**Pattern to copy:**

- SHA pin format `@<40-hex> # v<X.Y.Z>` — UNCHANGED (Phase 24 reuses the pinned SHA; no version bump in scope).
- `with:` block uses the multi-line `files: |` literal-block scalar (one file per line, indented).
- `env: GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}` — UNCHANGED.

**Destination shape for Phase 24** (RESEARCH §3.3):

```yaml
      # Phase 24 D-07 + D-15: ship as draft + 4-file files: list.
      # The .asc PGP detached signature arrives post-CI via the maintainer's
      # documented procedure in docs/RELEASING.md.
      - name: Upload to GitHub Releases (draft — maintainer flips out of draft after PGP upload)
        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
        with:
          # D-07: Release ships as draft until the maintainer uploads the .asc
          # detached PGP signature and runs `gh release edit vX.Y.Z --draft=false`.
          # Operators visiting the Releases page never see a release missing the
          # PGP signature.
          draft: true
          # D-15: semantic grouping — artifact, integrity, signature, provenance.
          # The .asc PGP signature is uploaded post-CI by the maintainer.
          files: |
            blindjoin-linux-amd64.tar.gz
            blindjoin-linux-amd64.tar.gz.sha256
            blindjoin-linux-amd64.tar.gz.bundle
            blindjoin-linux-amd64.tar.gz.sigstore
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**Key differences from the existing step:**

- Step `name:` grows to explain the draft state (operator-readable).
- `with: draft: true` — RESEARCH §3.3 VERIFIED at the pinned SHA: `draft` is input #5 of 14 declared inputs.
- `files:` list grows from 2 to 4 entries (semantic ordering: artifact → integrity → signature → provenance).

---

### §7. `SECURITY.md` — append `### Release tarball signatures + provenance (v1.6 onward)` subsection

**Analog:** `SECURITY.md:118-185` (Phase 23's `### Image signatures + attestations (v1.6 onward)` subsection). Phase 24 mirrors the EXACT structural shape: H3 heading + opening prose + numbered claims + prerequisites paragraph + fenced bash recipes block + `> Note:` callout blocks.

**Source-of-truth excerpt** (`SECURITY.md` lines 118-185):

```markdown
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
# [... shortened ...]
```

> **Note: GHCR UI "Unverified" badge** is unrelated to cosign verification
> (Pitfall 10). [... shortened ...]

> **Note: cosign 3.0 CLI flag drift** (Pitfall 13). The recipes above have
> been tested with **cosign `>= 2.6.3, < 3.0.0`**. [... shortened ...]
```

**Pattern to copy:**

- H3 heading: `### <topic> (v1.6 onward)` — exact mirror of Phase 22/23 form.
- Opening paragraph: states WHAT is signed/attested + names the artifact + tag-trigger source.
- Numbered claim list with bold lede on each item (`**Signed by cosign** ...`, `**Attested with a SLSA v1.0 ...**`). Phase 24 follows the same number of items as Phase 23 (3 items, modulo SBOM scope — Phase 24 omits SBOM per `<domain>`).
- Prerequisites paragraph naming `cosign 2.6.3 or compatible` + `gh 2.x or later` versions.
- Fenced ` ```bash ` recipes block with numbered comments inline.
- `> Note:` callouts below the recipes (one per Pitfall referenced).

**Destination shape for Phase 24** (RESEARCH §4 — exact text reproduced in the next plan):

```markdown
### Release tarball signatures + provenance (v1.6 onward)

Every `blindjoin-linux-amd64.tar.gz` Release archive published from a `vX.Y.Z`
tag is:

1. **Signed by cosign** via OIDC keyless flow (no maintainer key custody). The
   signature is distributed as `blindjoin-linux-amd64.tar.gz.bundle` — a single
   JSON file containing the signature, Fulcio-issued cert, and Rekor
   transparency-log inclusion proof.
2. **Attested with a SLSA v1.0 in-toto provenance bundle** (predicate type
   `https://slsa.dev/provenance/v1`), naming the workflow file (`release.yml`)
   + tag ref + source commit + runner image. The attestation is pushed to the
   GitHub Attestations API AND distributed as `blindjoin-linux-amd64.tar.gz.sigstore`.
3. **Detached PGP signature** (`blindjoin-linux-amd64.tar.gz.asc`) from the
   maintainer's YubiKey-held ed25519 key, uploaded post-CI as an alternative
   trust path for operators who cannot reach Sigstore Fulcio/Rekor.

EITHER cosign OR PGP verification is sufficient — they are alternative paths,
not both required.

Verification requires **cosign 2.6.3 or compatible** (see the image subsection
above for the cosign version pin rationale), the **GitHub CLI (`gh`) 2.50 or
later**, and **gpg 2.4 or later**.

```bash
# 1. Cosign blob signature verification (SIGN-01)
cosign verify-blob \
  --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz

# 2. SLSA provenance — Path A (GitHub Attestations API; requires github.com reachable)
gh attestation verify blindjoin-linux-amd64.tar.gz --repo <owner>/blindjoin

# 2. SLSA provenance — Path B (offline cosign verify; works after one-time TUF cache seeding)
cosign verify-attestation \
  --bundle blindjoin-linux-amd64.tar.gz.sigstore \
  --type slsaprovenance \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz

# 3. Detached PGP signature (SIGN-03)
gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com  # one-time key fetch via WKD
gpg --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz
# IMPORTANT: gpg --verify returns exit 0 on cryptographic validity even when the
# operator's trust web has not certified the key. Compare the "Primary key
# fingerprint:" line printed to stderr against the canonical fingerprint below.
```

> **Note: cosign 3.0 CLI flag drift** — see the [image subsection above](#image-signatures--attestations-v16-onward) for the cosign version pin range; the same constraints apply to tarball verification.

<a id="pgp-current"></a>
**Current maintainer PGP fingerprint:** `<FINGERPRINT-TBD>` (UID `blindjoin maintainer <johnturner@gmail.com>`, ed25519, generated YYYY-MM-DD, expires YYYY-MM-DD). The committed public key lives at [`docs/pgp/<FINGERPRINT-TBD>.asc`](docs/pgp/) and is published to keys.openpgp.org + WKD on `<owner>.github.io`.
```

**Key differences from Phase 23's image subsection:**

- Three numbered claims, not three — same number, but SIGN-03 (PGP) replaces ATTEST-03 (SBOM).
- New "EITHER cosign OR PGP verification is sufficient" prose paragraph between numbered list and prerequisites (D-12 explicit lock — Phase 23 has no equivalent because there's no PGP alternative).
- Recipe identity-regexp targets `release\.yml`, not `docker\.yml`.
- `cosign verify-blob --bundle ...` (not `cosign verify ...` for OCI).
- `cosign verify-attestation --bundle ...sigstore --type slsaprovenance` (the Path B form).
- `gh attestation verify <file>` (file argument, not `oci://...`).
- New PGP recipe block (no Phase 23 analog — completely novel for the tarball subsection).
- New `<a id="pgp-current"></a>` anchor + fingerprint prose at the END of the subsection (D-09 + D-12).
- Pitfall 13 callout is a 1-LINE cross-ref to the image subsection's version-pin callout (D-12 explicit — avoids prose duplication).

---

### §8. `CONTRIBUTING.md` — one-line cross-ref to `docs/RELEASING.md`

**Analog:** `CONTRIBUTING.md:69-94` (existing `## Tagging releases` section). The insertion point is the END of this section, AFTER the existing tag-bash example.

**Source-of-truth excerpt** (`CONTRIBUTING.md` lines 69-94):

```markdown
## Tagging releases

Milestone tags must follow strict 3-part semver: `vMAJOR.MINOR.PATCH` (e.g. `v1.3.0`, not `v1.3`).

**Why:** [.github/workflows/docker.yml](.github/workflows/docker.yml) uses `docker/metadata-action` with `type=semver,pattern={{version}}`, which only matches `vX.Y.Z`. [... shortened ...]

**Before tagging:** add a new `## [X.Y.Z] — YYYY-MM-DD` section to [CHANGELOG.md](CHANGELOG.md) and move any unreleased bullets into it. [... shortened ...]

**Crate versions in `Cargo.toml` stay at `0.1.0`** by policy [... shortened ...]

**Tagging a milestone close:**

```bash
git tag -a v1.X.0 -m "v1.X <Milestone name>
[... shortened ...]
```

The milestone *name* in planning docs (e.g. `v1.3 Test Infrastructure & Operational Hardening`) is independent of the git tag — docs may stay `v1.X` for readability while the tag is `v1.X.0`.
```

**Pattern to copy:**

- Append a NEW paragraph at the END of the section (between the existing milestone-name paragraph at line 94 and the `## Bumping base-image digests` H2 at line 96).
- Prose style: short, conversational, uses markdown relative link `[`docs/RELEASING.md`](docs/RELEASING.md)`.
- Audience-disambiguation lede: "Most contributors don't need it" (so a contributor reading the section knows they shouldn't go down the rabbit hole unless they're cutting a release).

**Destination shape for Phase 24** (RESEARCH §5.4):

```markdown

Once `release.yml` finishes, the maintainer-side procedure (download the CI-built tarball, sign it with PGP on a YubiKey, upload the `.asc`, flip the release out of draft) lives in [`docs/RELEASING.md`](docs/RELEASING.md). Most contributors don't need it; it's the release-engineering manual for the maintainer.
```

Inserted as a new paragraph BETWEEN `CONTRIBUTING.md` lines 94 and 96 (after the milestone-name paragraph; before the `## Bumping base-image digests` H2).

---

### §9. `docs/RELEASING.md` — NEW maintainer-side procedure

**Analog:** no in-repo procedural-docs analog. Closest by shape is `docs/AUDIT-CHARTER.md` (long-form `docs/` policy file with H1 + H2 ToC). The content (GnuPG ceremony, WKD publication, key rotation/revocation) is governed by canonical GnuPG procedures cited in RESEARCH §5.

**Source-of-truth excerpt for project `docs/` file shape** (the `docs/AUDIT-CHARTER.md` H1 + table-of-contents pattern — confirmed via `ls docs/` showing `AUDIT-CHARTER.md`, `PROTOCOL.md`, `branch-protection/`).

**Pattern to copy:**

- Filename: lowercase-lowercase or UPPER-CASE-MULTI-WORD — `RELEASING.md` is UPPERCASE matching `AUDIT-CHARTER.md`, `PROTOCOL.md` precedent.
- H1: `# Releasing blindjoin` — concise title.
- Opening paragraph: audience disambiguation ("Maintainer-side procedure ... Contributors don't need this — see CONTRIBUTING.md ...").
- H2 sections: `## Prerequisites`, `## Per-release procedure (5 steps)`, `## Pre-flight check before flipping out of draft`, `## PGP key generation (one-time, NOT a release-cut step)`, `## PGP key rotation (every 2 years)`, `## PGP key revocation (emergency — YubiKey lost or compromised)`, `## Publishing the key to keys.openpgp.org`, `## Publishing the key to WKD`.
- Per-step numbered procedures with inline fenced ` ```bash ` blocks.
- `<FINGERPRINT-TBD>` placeholders throughout — replaced atomically by Plan 24-05's `checkpoint:human-verify` task.

**Destination shape for Phase 24** (RESEARCH §5.5 — the full skeleton):

```markdown
# Releasing blindjoin

Maintainer-side procedure for cutting a release. Contributors don't need this — see [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor manual.

## Prerequisites

- YubiKey 5 (firmware ≥ 5.2.3 for ed25519 support) with the blindjoin maintainer PGP key (one-time generation: see [PGP key generation](#pgp-key-generation-one-time-not-a-release-cut-step)).
- `gpg` 2.4+ on the maintainer's machine.
- `gh` 2.50+ on the maintainer's machine.
- `cosign` 2.6.3+ for pre-flight verify (see Pre-flight check).
- `<owner>.github.io` repo exists with WKD published (one-time setup: see [Publishing the key to WKD](#publishing-the-key-to-wkd)).

## Per-release procedure (5 steps)

1. `git tag -s vX.Y.Z -m "vX.Y.Z"`; `git push --tags`.
2. Watch [`release.yml`](../.github/workflows/release.yml) in the Actions tab until green. CI creates a DRAFT release with 4 assets (tarball, .sha256, .bundle, .sigstore).
3. `gh release download vX.Y.Z -p 'blindjoin-linux-amd64.tar.gz' --dir /tmp/blindjoin-release`.
4. `cd /tmp/blindjoin-release && gpg --detach-sign --armor --local-user <FINGERPRINT-TBD> blindjoin-linux-amd64.tar.gz` (YubiKey will prompt for touch).
5. `gh release upload vX.Y.Z blindjoin-linux-amd64.tar.gz.asc && gh release edit vX.Y.Z --draft=false`.

## Pre-flight check before flipping out of draft

[... cosign-verify all 4 CI-produced assets BEFORE step 5's --draft=false ...]

## PGP key generation (one-time, NOT a release-cut step)

[5-step ceremony per RESEARCH §5.1 — generate on YubiKey, revocation cert offline, export, publish to keys.openpgp.org, publish to WKD]

## PGP key rotation (every 2 years)

[6-step procedure per RESEARCH §5.2]

## PGP key revocation (emergency — YubiKey lost or compromised)

[Publish offline revoke.asc immediately; then rotation procedure]

## Publishing the key to keys.openpgp.org

[gpg --send-keys + email confirmation flow]

## Publishing the key to WKD

[5-step per RESEARCH §5.3 — gpg-wks-client --print-wkd-hash, .well-known/openpgpkey/hu/<hash>, gh repo create <owner>.github.io if needed]
```

---

### §10. `docs/pgp/<FULL-40-CHAR-FINGERPRINT>.asc` — NEW armored ed25519 public key

**Analog:** none in the repo. The file is a `gpg --export --armor <fingerprint> > docs/pgp/<fingerprint>.asc` output following the OpenPGP ASCII Armor specification (RFC 9580). The maintainer generates the key on YubiKey at the §10 procedure in `docs/RELEASING.md` and exports it; Phase 24 commits the resulting `.asc` file via Plan 24-05's `checkpoint:human-verify` task.

**Pattern to copy:** none — first PGP artifact in the project. Filename convention is the unambiguous full 40-char fingerprint per D-09. Content is the maintainer's ASCII-armored public key (begins `-----BEGIN PGP PUBLIC KEY BLOCK-----`).

**Note for the planner:** Plan 24-05 commits THREE atomic changes together (the .asc file + the SECURITY.md `<FINGERPRINT-TBD>` placeholder replacement + the `docs/RELEASING.md` `<FINGERPRINT-TBD>` placeholder replacement). This is a `checkpoint:human-verify` task — Plan-author writes the placeholder string `<FINGERPRINT-TBD>` in Plans 24-02 and 24-03; maintainer + Plan 24-05 execute the atomic replacement.

---

## Shared Patterns

### Shared #1: SHA-pin trailing-comment style — TWO spaces, full 40-hex, `# v<X.Y.Z>` suffix

**Source-of-truth excerpts** (every `uses:` line in the repo follows this shape):

- `docker.yml:151` — `        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1`
- `docker.yml:272` — `        uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0`
- `docker.yml:36` — `      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`
- `release.yml:101` — `        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`
- `release.yml:69` — `      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`

**Pattern to copy:**

- Form: `<owner>/<action>@<40-hex-SHA> # v<X.Y.Z>` (or `# stable` for `dtolnay/rust-toolchain` only; never for sigstore actions).
- TWO spaces before `#` (Phase 23 PATTERNS §"Shared #2", enforced by `sigstore-pin-check` at `ci.yml:292-326`).
- Full 40 hex chars (the gate's regex `(?![a-f0-9]{40})` rejects shorter forms).

**Apply to:** the two new `uses:` lines in `release.yml` (cosign-installer at §2 + attest-build-provenance at §4). Pin values resolved in RESEARCH §2.1 (verbatim reuse of Phase 23 pins).

| Action | Pin |
|--------|-----|
| `sigstore/cosign-installer` | `@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1` |
| `actions/attest-build-provenance` | `@96278af6caaf10aea03fd8d33a09a777ca52d62f  # v3.2.0` |

**Existing pin (unchanged in Phase 24):**

| Action | Pin |
|--------|-----|
| `softprops/action-gh-release` | `@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1` |

---

### Shared #2: Comments-as-contract above every new structural block

**Source-of-truth excerpts** (every structural block in `release.yml` / `docker.yml` / `ci.yml` is prefaced by a multi-line `#` comment block):

**(a) Above the workflow-level `env:` block** (`release.yml:3-15`):

```yaml
env:
  # Force GitHub Actions runner to execute Node 20 JS actions on Node 24,
  # silencing the deprecation warning ahead of the June 2026 hard cutover.
  # See: actions/checkout v6.0.2 still declares `using: node20` — upgrading
  # the action SHA is tracked separately (see TODO at top of ci.yml).
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
  # Release-smoke gate (P0-5, v1.5 release-readiness): integration tests
  # under tests/integration/* must FAIL on bitcoind-missing rather than
  # graceful-skip during a release. [... shortened ...]
  BLINDJOIN_REQUIRE_BITCOIND: "1"
```

**(b) Above the `if: startsWith(github.ref, 'refs/tags/')` job-gate** (`release.yml:64-66`):

```yaml
    # Publish gate: only run on a real tag push. workflow_dispatch runs
    # check-only (the rehearsal path) and never uploads release artifacts.
    if: startsWith(github.ref, 'refs/tags/')
```

**(c) Above the existing `Read canonical base-image digests` step** (`release.yml:77-86`):

```yaml
      # Phase 22 DRIFT-03: read the canonical base-image digest manifest.
      # This step is a supply-chain gate — a tag push cannot publish a
      # release tarball unless docker/digests.txt is present and well-formed
      # (the composite action exits 1 otherwise; see
      # .github/actions/read-base-digests/action.yml). [... shortened ...]
      - name: Read canonical base-image digests
        id: digests
        uses: ./.github/actions/read-base-digests
```

**Pattern to copy:**

- Every new structural element (permissions block at §1, install-cosign step at §2, sign-blob step at §3, attest step at §4, rename step at §5, modified softprops step at §6) gets a multi-line `#` comment block IMMEDIATELY ABOVE it.
- Comment cites: (a) the REQ-ID (`Phase 24 SIGN-01`, `Phase 24 SIGN-02`), (b) the WHY (what failure mode this avoids), (c) cross-references to related files (`PITFALLS Pitfall N`, `Phase 23 D-XX`, `RESEARCH §N.N`).
- Cause-to-effect-to-source structure: `<what this does> — <how it would fail without it> — <where the design is recorded>`.

**Apply to:** all 6 new structural changes in `release.yml` (§1-§6 above), the SECURITY.md prose (§7 inherits the prose-style discipline), the `docs/RELEASING.md` skeleton (§9 uses the same comments-rich style for the bash recipes).

---

### Shared #3: Step-name convention `<verb> ... — <REQ-ID>`

**Source-of-truth excerpts** (Phase 23 step names in `docker.yml`):

- `docker.yml:175` — `- name: Sign image with cosign (keyless OIDC) — ATTEST-01`
- `docker.yml:198` — `- name: Generate SPDX SBOM with Syft (ATTEST-03 generator)`
- `docker.yml:229` — `- name: Attest SBOM (ATTEST-03)`
- `docker.yml:271` — `- name: Attest build provenance (ATTEST-02)`

**Pattern to copy:**

- Step name format: `<verb> <object> [(<modifier>)] — <REQ-ID>` (em-dash, then REQ-ID).
- Verb is action-oriented (`Sign`, `Attest`, `Install`, `Rename`, `Upload`).
- REQ-ID matches the requirement being satisfied (`SIGN-01`, `SIGN-02`).
- Infrastructure steps (no REQ-ID) get a parenthetical descriptor instead (`Install cosign (keyless signing toolchain)`, `Rename provenance bundle to .sigstore Release asset filename`).

**Apply to:** all 4 new step names in `release.yml` per the Phase 24 final mapping:

| Step | Name |
|------|------|
| Install cosign | `Install cosign (keyless signing toolchain)` |
| Sign tarball | `Sign tarball with cosign (keyless OIDC) — SIGN-01` |
| Attest provenance | `Attest tarball build provenance — SIGN-02` |
| Rename bundle | `Rename provenance bundle to .sigstore Release asset filename` |
| Modified softprops | `Upload to GitHub Releases (draft — maintainer flips out of draft after PGP upload)` |

---

### Shared #4: Operator recipes block — fenced bash with numbered comments and expected output

**Source-of-truth excerpt** (`SECURITY.md:138-170` — Phase 23's verify recipes block):

```bash
# 1. Cosign signature verification (ATTEST-01)
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/<owner>/blindjoin-<image>:<tag>
# Substitute <image> = coordinator | client | liquidity-bot
# Expected: "Verification for ghcr.io/.../blindjoin-<image>:<tag> --" + JSON
#           output of the verified cert claims.
```

**Pattern to copy:**

- Single fenced ` ```bash ` block (NOT separate code blocks per recipe).
- Numbered comments (`# 1. <description>`) above each recipe.
- Multi-line bash commands use trailing `\` for line continuation.
- Indented `<owner>` / `<image>` / `<tag>` placeholders in angle brackets.
- `# Expected: ...` comment after each recipe naming the success-output shape (so operators know what success looks like).

**Apply to:** the SECURITY.md recipes block in §7 above. Phase 24's block contains 4 recipes (cosign verify-blob, gh attestation verify, cosign verify-attestation, gpg --verify) — same style.

---

## No Analog Found

| File / asset | Role | Data flow | Reason |
|--------------|------|-----------|--------|
| `docs/pgp/<FP>.asc` | armored ed25519 public key | n/a | First PGP key file in the project. Format is the OpenPGP ASCII Armor spec; content is the maintainer's `gpg --export --armor` output. No in-repo prior art. |
| `mv "${{ steps.X.outputs.bundle-path }}" ...` step (§5) | inline shell file-rename | request-response | Phase 23 uses `push-to-registry: true` on `actions/attest-build-provenance` to route the attestation through OCI; Phase 24 has no registry path so the bundle must be relocated on the runner filesystem. Closest in-repo analog is `release.yml:91-98`'s `Package` step (multi-statement shell), but the §5 step is single-statement. The pattern is "new but trivial" — `mv "<src>" <dst>` with project's standard `${{ steps.X.outputs.Y }}` plumbing. |
| WKD `.well-known/openpgpkey/hu/<hash>` layout | hosted public-key file | n/a | Documented procedure in `docs/RELEASING.md` per RESEARCH §5.3; the file lives in `<owner>.github.io`, NOT in this repo. The in-repo deliverable is only the prose procedure. |
| `gpg-wks-client --print-wkd-hash <email>` shell snippet | docs CLI snippet | n/a | Standard GnuPG canonical procedure; cited from GnuPG manual in RESEARCH §5.3. No in-repo analog. |

All four novel patterns are either documentation-only (and live in `docs/RELEASING.md` + `SECURITY.md`) or trivial single-statement shell steps in `release.yml`. The planner copies them VERBATIM from RESEARCH §3.2 / §4 / §5.

---

## Metadata

**Analog search scope:**

- `.github/workflows/release.yml` (full — 107 lines; the modification target)
- `.github/workflows/docker.yml` (full — 277 lines; the Phase 23 structural source-of-truth)
- `.github/workflows/ci.yml` (lines 285-326; the `sigstore-pin-check` job inherited from Phase 23)
- `SECURITY.md` (full — 291 lines; the Phase 23 image subsection that Phase 24's tarball subsection mirrors)
- `CONTRIBUTING.md` (full — 142 lines; the `## Tagging releases` section insertion point)
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-PATTERNS.md` (predecessor pattern-map for the comments-as-contract + SHA-pin + permissions block shapes Phase 24 inherits)
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-01-PLAN.md` (Plan frontmatter + must_haves structure that Phase 24's plans will mirror)
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-04-PLAN.md` (Plan structure for SECURITY.md docs modification)
- `.planning/phases/24-release-tarball-signing-cosign-slsa-pgp/24-CONTEXT.md` (D-01..D-20 user decisions)
- `.planning/phases/24-release-tarball-signing-cosign-slsa-pgp/24-RESEARCH.md` (verified pins + RESEARCH §3.2 D-14 correction + canonical recipe text)

**Files scanned:** 10 (all in-repo)

**Pattern extraction date:** 2026-06-02

## PATTERN MAPPING COMPLETE
