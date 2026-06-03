# Phase 23: cosign Image Attestations + SLSA Provenance + SBOM — Pattern Map

**Mapped:** 2026-06-01
**Files analyzed:** 5 (3 modified, 2 reference-only — HUMAN-UAT artifacts under `.planning/quick/`; no new checked-in code files)
**Analogs found:** 5 / 5 (all in-repo; HUMAN-UAT shape mirrors v1.5 quick-task pair)

> **Read-this-first:** RESEARCH.md materially corrects three CONTEXT.md
> assertions that this pattern map must therefore split into more steps than
> CONTEXT.md anticipates:
> 1. `actions/attest-sbom` does NOT generate the SBOM — a separate
>    `anchore/sbom-action` step (Syft inside) must run first to write
>    `sbom.spdx.json`. See **Pattern Assignment #4 (Generate SBOM)** and
>    **#5 (Attest SBOM)** below — TWO analogs, two distinct excerpts.
> 2. `cosign sign` and `actions/attest-build-provenance` are NOT
>    interchangeable — they produce different OCI artifacts under different
>    tag conventions (`<digest>.sig` vs `sha256-<HEX>` referrer manifest).
>    Phase 23 MUST emit BOTH. See **Pattern Assignment #3 (Sign image)** and
>    **#6 (Attest provenance)** below — separate steps, separate REQ-IDs
>    (ATTEST-01 vs ATTEST-02).
> 3. `attestations: write` is NOT mentioned in CONTEXT.md D-02 but is
>    REQUIRED — without it, the `actions/attest-*` steps fail with
>    `403 Forbidden`. See **Shared Pattern: Job-level permissions block**.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `.github/workflows/docker.yml` (MODIFY) | workflow — add 5 new steps + 2 permission lines + 1 `id:` on existing step | request-response (tag push → publish + sign + attest) | self (Phase 22 inserted `read-base-digests` step here; Phase 23 extends the same `docker` matrix leg downward) | exact (modify in place) |
| `.github/workflows/ci.yml` (MODIFY) | workflow — add new `sigstore-pin-check` job | event-driven (push + PR grep gate) | self ([ci.yml:214-236 `bip322-pin-check`](.github/workflows/ci.yml#L214)) | exact (CONTEXT D-04 names it as the structural mirror) |
| `SECURITY.md` (MODIFY) | docs / policy — full `## Supply-chain status` rewrite (intro prose + recipes block + callouts block) + add new `### Image signatures + attestations (v1.6 onward)` subsection | n/a | self (Phase 22 P0-1 prose at [SECURITY.md:95-159](SECURITY.md#L95) is the starting point; Phase 22's `### Base-image digests (v1.6 onward)` subsection stays untouched as a sibling) | exact (extend + rewrite in place) |
| `CONTRIBUTING.md` (OPTIONAL MODIFY) | docs — one-line cross-reference to the rewritten SECURITY.md subsection | n/a | self (existing `## Tagging releases` section style) | optional — planner discretion, recommended NO change per CONTEXT D-05 |
| `.planning/quick/YYMMDD-<slug>-...-SUMMARY.md` (NEW HUMAN-UAT artifact) | planning / rehearsal log | n/a | `.planning/milestones/v1.5-quick/260531-thw-*/260531-thw-SUMMARY.md` + `260531-ubf-SUMMARY.md` | exact (CONTEXT D-06 names them as the shape mirror) |

**No new checked-in code files.** Phase 23 deliberately introduces ZERO new composite actions and ZERO new workflows. The 3-leg matrix in `docker.yml` already deduplicates the sign+attest steps; the `sigstore-pin-check` job lives inline in `ci.yml` per D-09 recommendation; HUMAN-UAT rehearsal logs go under `.planning/quick/` per v1.5 precedent.

---

## Pattern Assignments

Order mirrors the step ordering in `docker.yml`'s `docker` job after the new edits land:

| # | New step | REQ-ID | Action | Section |
|---|----------|--------|--------|---------|
| 1 | Add `id: build` to existing build-push step | (enables ATTEST-01/02/03 digest plumbing) | none — modify existing step | §1 |
| 2 | Install cosign | (toolchain) | `sigstore/cosign-installer@...v3.10.1` | §2 |
| 3 | **Sign image with cosign** | ATTEST-01 | `cosign sign` (CLI via `run:`) | §3 |
| 4 | **Generate SPDX SBOM with Syft** | ATTEST-03 (generator half — NOT in CONTEXT.md, RESEARCH §2.2 correction) | `anchore/sbom-action@...v0.24.0` | §4 |
| 5 | **Attest SBOM** | ATTEST-03 (attest half) | `actions/attest-sbom@...v2.4.0` | §5 |
| 6 | **Attest build provenance** | ATTEST-02 | `actions/attest-build-provenance@...v3.2.0` | §6 |
| 7 | New `sigstore-pin-check` job in `ci.yml` | (D-04 invariant) | inline grep gate | §7 |
| 8 | `SECURITY.md` rewrite (overview + recipes + callouts + new subsection) | ATTEST-01/02/03/04 documentation | docs | §8 |

ATTEST-04 (downloadable `.bundle` for offline verify) is satisfied by the **operator-side `cosign save` recipe** documented in SECURITY.md (§8) — **no CI step**, per RESEARCH §3.4 correction.

---

### §1. Existing build-push step — add `id: build` (only change to existing `with:` block)

**Analog:** the existing `docker/build-push-action` step in this same file ([docker.yml:110-126](.github/workflows/docker.yml#L110)).

**Source-of-truth excerpt** ([docker.yml:106-126](.github/workflows/docker.yml#L106) — the previous `read-base-digests` step shows the project's `id: <name>` + `uses:` step shape that downstream steps' `${{ steps.<id>.outputs.<name> }}` reads consume):

```yaml
      - name: Read canonical base-image digests
        id: digests
        uses: ./.github/actions/read-base-digests

      - uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0
        with:
          context: .
          file: docker/Dockerfile
          target: ${{ matrix.target }}
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          build-args: |
            DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}
            CARGO_CHEF_REF=${{ steps.digests.outputs.cargo_chef_ref }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

**Pattern to copy** (Phase 23 modification): add a single line `id: build` between the existing `uses:` line and the existing `with:` line on the build-push step. Do NOT add `provenance:` or `sbom:` inputs to the `with:` block (RESEARCH §3.1 + PITFALLS Pitfall 5 — competing attestations are the failure mode). The downstream sign + attest steps then read `${{ steps.build.outputs.digest }}` exactly the way the existing build-push step at [docker.yml:122-123](.github/workflows/docker.yml#L122) reads `${{ steps.digests.outputs.debian_ref }}` — same project convention.

---

### §2. Install cosign — `sigstore/cosign-installer`

**Analog:** ANY existing `uses:` line in `docker.yml` — the SHA-pin trailing-comment style is uniform across the file.

**Source-of-truth excerpts** for the SHA-pin trailing-comment style (every `uses:` in `docker.yml` follows this shape):

- [docker.yml:36](.github/workflows/docker.yml#L36) — `      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`
- [docker.yml:78](.github/workflows/docker.yml#L78) — `      - uses: docker/login-action@4907a6ddec9925e35a0a9e82d7399ccc52663121 # v4.1.0`
- [docker.yml:84](.github/workflows/docker.yml#L84) — `      - uses: docker/setup-buildx-action@4d04d5d9486b7bd6fa91e7baf45bbb4f8b9deedd # v4.0.0`
- [docker.yml:110](.github/workflows/docker.yml#L110) — `      - uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0`
- [release.yml:101](.github/workflows/release.yml#L101) — `        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`

**Pattern to copy:**
- Form: `<owner>/<action>@<40-hex-SHA> # v<X.Y.Z>` — full 40-hex SHA, TWO spaces before `#`, version comment after.
- For setup actions that need configuration, the `with:` block follows on the next indented line; for plain `uses:` lines, no `with:` block at all.

**Destination shape for Phase 23** (from RESEARCH.md §3.2, planner copies verbatim into `docker.yml` immediately after the modified `docker/build-push-action` step):

```yaml
      # Phase 23 ATTEST-01: cosign keyless OIDC signing toolchain.
      # Installer pinned to v3.10.1 (last v3.X — see RESEARCH §2.3). v4.X
      # of cosign-installer mandates cosign 3.x which is operator-incompatible
      # with the documented `>= 2.6.3, < 3.0.0` range in SECURITY.md. Pin
      # cosign-release to v2.6.3 (latest 2.x stable).
      - name: Install cosign (keyless signing toolchain)
        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1
        with:
          cosign-release: 'v2.6.3'
```

---

### §3. Sign image with cosign — ATTEST-01

**Analog:** the existing inline shell step on an env-passed digest in this same file (none exists — closest is the `run: cargo audit` style in [ci.yml:181](.github/workflows/ci.yml#L181)). The new step uses the `env:` + `run:` pattern from [ci.yml:48-54](.github/workflows/ci.yml#L48) (the cache-key step) which exports values via `env:` to the shell.

**Source-of-truth excerpt** for the `env:` + `run:` shape ([ci.yml:62-118](.github/workflows/ci.yml#L62) — the install-bitcoind step uses `set -euo pipefail` + env-var-referenced `${VARNAME}` form):

```yaml
      - name: Install bitcoind (cache miss only)
        if: steps.cache-bitcoind.outputs.cache-hit != 'true'
        run: |
          set -euo pipefail
          VERSION="${{ steps.bitcoind_version.outputs.version }}"
          TARBALL="bitcoin-${VERSION}-x86_64-linux-gnu.tar.gz"
          BASE="https://bitcoincore.org/bin/bitcoin-core-${VERSION}"
          ...
```

**Pattern to copy:**
- `env:` block under the step for any value that comes from another step's outputs — keeps the `run:` body free of `${{ … }}` interpolation noise.
- One-line `run:` works when there's a single command; multi-line `run: |` + `set -euo pipefail` for anything with multiple statements.

**Destination shape** (from RESEARCH.md §3.2, planner copies after the cosign-installer step):

```yaml
      # Phase 23 ATTEST-01: produces ghcr.io/.../blindjoin-<image>:sha256-<HEX>.sig
      # — a Fulcio-issued cert bound to the GHA OIDC subject claim, plus the
      # Rekor inclusion proof. Operator-side `cosign verify` recipe in
      # SECURITY.md verifies against the locked --certificate-identity-regexp.
      #
      # --yes: non-interactive; required in CI (would otherwise prompt for
      # transparency-log consent).
      # See PITFALLS Pitfall 3: do NOT add `--no-tlog-upload` — Rekor is the
      # operator-facing transparency guarantee.
      - name: Sign image with cosign (keyless OIDC) — ATTEST-01
        env:
          IMAGE: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          DIGEST: ${{ steps.build.outputs.digest }}
        run: cosign sign --yes "${IMAGE}@${DIGEST}"
```

---

### §4. Generate SPDX SBOM with Syft — ATTEST-03 (generator half — RESEARCH §2.2 correction)

**Analog:** the existing `read-base-digests` step at [docker.yml:106-108](.github/workflows/docker.yml#L106) — same `name:` + `id:` + `uses:` + `with:` shape that the planner mirrors for every new third-party-action step (cosign-installer in §2, sbom-action here, attest-sbom in §5, attest-build-provenance in §6).

**Source-of-truth excerpt** ([docker.yml:99-108](.github/workflows/docker.yml#L99)):

```yaml
      # Phase 22 DRIFT-03: read the canonical base-image digest manifest
      # and pass its values to docker buildx via --build-arg. This
      # eliminates manual --build-arg invocation per the v1.5 P0-2/3
      # Dockerfile-side ARG scaffold. A tag push cannot publish ghcr.io
      # images unless docker/digests.txt is present and well-formed
      # (the composite action exits 1 otherwise; see
      # .github/actions/read-base-digests/action.yml).
      - name: Read canonical base-image digests
        id: digests
        uses: ./.github/actions/read-base-digests
```

**Pattern to copy:**
- Multi-line `#` comment block IMMEDIATELY ABOVE the step, citing the REQ-ID (`Phase 22 DRIFT-03:`) — Phase 23's mirror is `Phase 23 ATTEST-03 (generator):`.
- `name:` is a human-readable phrase ending in a noun.
- `id:` is single-word, used by downstream steps via `steps.<id>.outputs.*`.
- `uses:` line carries the full 40-hex SHA + trailing `# v<X.Y.Z>` comment.

**Destination shape** (from RESEARCH.md §3.3 — note `upload-artifact: false` per RESEARCH §8 Q4):

```yaml
      # Phase 23 ATTEST-03 (generator): scan the just-pushed image with Syft
      # (bundled inside anchore/sbom-action) and write the SBOM to
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
      #
      # RESEARCH CORRECTION: CONTEXT.md D-03 originally asserted attest-sbom
      # "invokes Syft internally"; that's incorrect as of v2.4.0 — this step
      # is the load-bearing generator that attest-sbom consumes.
      - name: Generate SPDX SBOM with Syft (ATTEST-03 generator)
        uses: anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0.24.0
        with:
          image: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}@${{ steps.build.outputs.digest }}
          format: spdx-json
          output-file: sbom.spdx.json
          upload-artifact: false
          upload-release-assets: false
```

---

### §5. Attest SBOM — ATTEST-03 (attest half)

**Analog:** same as §4 — `read-base-digests` step shape ([docker.yml:106-108](.github/workflows/docker.yml#L106)). The `with:` block carries inputs the action consumes; no `env:` shell-variable layer needed because attest-sbom is a JS action that reads its inputs from `with:` directly.

**Pattern to copy:** identical shape to §4 (name + uses + with), distinct `with:` keys per the action's documented inputs.

**Destination shape** (from RESEARCH.md §3.3):

```yaml
      # Phase 23 ATTEST-03 (attest): sign sbom.spdx.json as a SLSA in-toto
      # SBOM attestation and push to the OCI registry alongside the image
      # (referrer manifest at ghcr.io/.../blindjoin-<image>:sha256-<HEX>).
      #
      # attest-sbom does NOT generate the SBOM — it consumes the file written
      # by the previous step (RESEARCH §2.2 correction).
      #
      # Predicate type emitted: https://spdx.dev/Document (auto-derived from
      # the sbom-path file format).
      #
      # Pinned to v2.4.0 (NOT v3.X or v4.X) — v3/v4 are wrappers on
      # actions/attest; v2.4.0 is the last release with self-contained docs
      # matching CONTEXT.md D-03 wording. v1.7 carry-forward: migrate to
      # actions/attest@v4 as a single consolidated step.
      - name: Attest SBOM (ATTEST-03)
        uses: actions/attest-sbom@bd218ad0dbcb3e146bd073d1d9c6d78e08aa8a0b # v2.4.0
        with:
          subject-name: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          subject-digest: ${{ steps.build.outputs.digest }}
          sbom-path: 'sbom.spdx.json'
          push-to-registry: true
```

---

### §6. Attest build provenance — ATTEST-02

**Analog:** same as §4/§5 — `read-base-digests` step shape ([docker.yml:106-108](.github/workflows/docker.yml#L106)).

**Destination shape** (from RESEARCH.md §3.4):

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
      # SECURITY.md recipes block (D-05).
      #
      # RESEARCH §2.4: cosign sign (§3 above) and attest-build-provenance are
      # NOT interchangeable — they produce different OCI artifacts under
      # different tag conventions (<digest>.sig vs sha256-<HEX> referrer).
      # ATTEST-01 needs the former; ATTEST-02 needs the latter. Both required.
      #
      # Pinned to v3.2.0 (NOT v4.X) — v4 is a wrapper on actions/attest.
      # v3.2.0 is the last release that does what CONTEXT.md D-03 names.
      - name: Attest build provenance (ATTEST-02)
        uses: actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0
        with:
          subject-name: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          subject-digest: ${{ steps.build.outputs.digest }}
          push-to-registry: true
```

---

### §7. `sigstore-pin-check` job in `ci.yml` — D-04 + D-09 + RESEARCH §2.2 extension

**Analog:** `bip322-pin-check` job at [ci.yml:214-236](.github/workflows/ci.yml#L214). CONTEXT D-04 explicitly names this as the structural mirror. The new job extends the grep target list to FOUR actions (per RESEARCH §2.2 — `anchore/sbom-action` is load-bearing for ATTEST-03 and belongs in the gate).

**Source-of-truth excerpt** ([ci.yml:214-236](.github/workflows/ci.yml#L214)):

```yaml
  bip322-pin-check:
    name: bip322 exact-version pin check
    runs-on: ubuntu-latest
    # v1.4 ADR Decision #1 invariant: bip322 is pre-1.0; the API can change
    # between patch releases. Pin must be EXACTLY =0.0.10 (note the `=` operator).
    # The 26-LOC adapter at shared/src/bip322/mod.rs is verified against this
    # version only; any drift requires the adapter to be re-verified per Phase 14
    # carry-forward constraint #3 (exact-pin every dependency referenced in
    # test fixtures; CI-enforce). Mirrors the corepc-node-feature-pin-check
    # pattern above per RESEARCH Open Question #2 recommendation (one job per
    # pinned dep for clearer PR check log output).
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - name: Enforce exact bip322 pin
        run: |
          set -eu
          if grep -rEn 'bip322\s*=' --include='Cargo.toml' . \
             | grep -v '=\s*"=0\.0\.10"' \
             | grep -v '^[^:]*:[0-9]*:#'; then
            echo "ERROR: bip322 declaration(s) above lack the exact-version pin '=0.0.10'." >&2
            echo "       The bip322 crate is pre-1.0; minor changes can break the adapter at shared/src/bip322/mod.rs." >&2
            exit 1
          fi
```

**Cross-reference excerpt** ([ci.yml:238-263](.github/workflows/ci.yml#L238)) — the `crit-01-grep-check` job is a second example of the same "narrow, audit-grepable, named-after-what-it-enforces" shape:

```yaml
  crit-01-grep-check:
    name: CRIT-01 dual-branch invariant grep gate
    runs-on: ubuntu-latest
    # ... [comment block citing v1.4 Phase 16 Plan 16-02] ...
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - name: Enforce CRIT-01 dual-branch comment
        run: |
          set -eu
          COUNT=$(grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs || true)
          if [ "$COUNT" -lt 2 ]; then
            echo "ERROR: coordinator/src/bitcoin/utxo.rs has $COUNT CRIT-01 occurrences (need >= 2)." >&2
            echo "       The script-type derived-from-chain invariant must be commented" >&2
            echo "       at EACH version branch of the validate_utxo dispatcher." >&2
            ...
            exit 1
          fi
```

**Pattern to copy:**
- Job ID kebab-case, named after what it enforces (`bip322-pin-check` → `sigstore-pin-check`).
- `name:` field is human-readable, ≤ 50 chars (`bip322 exact-version pin check` → `sigstore + sbom action SHA-pin check`).
- Comment block above `steps:` cites the invariant source (`v1.4 ADR Decision #1` → `v1.6 Phase 23 ATTEST-01/02/03`) and explains the threat model.
- First step is always `actions/checkout@34e1148...` (the canonical project pin).
- Second step is the grep gate: `run: |` + `set -eu` + a single `if grep ...; then echo ERROR; exit 1; fi` block.
- Error message: multi-line `echo ... >&2` block, names the invariant, names the policy document (SECURITY.md / PITFALLS.md), exits 1.
- **Inline error strings** (no `POLICY_REF` shell variable — Phase 22 Plan 22-04 lesson; the file-level grep `grep -c 'Supply-chain'` on the workflow file MUST count the citation literals).

**Destination shape** (from RESEARCH.md §5, planner appends at end of `ci.yml` after `crit-01-client-grep-check`):

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

---

### §8. `SECURITY.md` rewrite — D-05 (full `## Supply-chain status` rewrite + new `### Image signatures + attestations (v1.6 onward)` subsection)

**Analog:** Phase 22 P0-1 prose at [SECURITY.md:95-159](SECURITY.md#L95). The existing `## Supply-chain status` overview prose (lines 97-101) + the "Known gaps at v1.5" bullets (lines 102-120) are the rewrite targets. The Phase 22 `### Base-image digests (v1.6 onward)` subsection (lines 122-159) STAYS UNTOUCHED as a sibling subsection below the new `### Image signatures + attestations (v1.6 onward)` Phase 23 adds.

**Source-of-truth excerpts:**

**[SECURITY.md:95-120](SECURITY.md#L95)** — the overview + known-gaps bullets that get rewritten:

```markdown
## Supply-chain status

blindjoin's release artifacts have **known supply-chain gaps** at v1.5.
They are documented here, not hidden. If you operate blindjoin in any
environment where supply-chain assurance matters, read this section
before pulling a binary or image.

### Known gaps at v1.5

- **GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign
  signature.** ...
- **Docker images on `ghcr.io` are unsigned.** No cosign attestation, no
  Notary v2 signature, no Sigstore witness. ...
- **No reproducible-build pipeline.** ...
- **~~Base image digest pins are manual.~~** **Closed in v1.6** — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).
```

**[SECURITY.md:122-159](SECURITY.md#L122)** — the Phase 22 subsection style Phase 23 mirrors (intro paragraph + bold lead-in claim + linked workflow path + workflow-policy paragraph + idempotency paragraph). Phase 23's new `### Image signatures + attestations (v1.6 onward)` subsection follows the SAME structural shape: opens with a claim ("Every X is Y"), enumerated artifacts (numbered list of what's signed/attested), prerequisite tooling paragraph (`cosign 2.6.3 or compatible + gh 2.x`), fenced bash recipes block, callout blocks (`> Note: ...`).

```markdown
### Base-image digests (v1.6 onward)

blindjoin's `docker/Dockerfile` derives from two upstream base images:
`debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. As
of v1.6 (Phase 22), both are pinned by digest in
[`docker/digests.txt`](docker/digests.txt) — the canonical manifest — and
every tagged release build passes those digests to `docker buildx build
--build-arg DEBIAN_REF=… --build-arg CARGO_CHEF_REF=…` automatically via
[`.github/actions/read-base-digests/`](.github/actions/read-base-digests/).

**`docker/digests.txt` is the canonical record of which upstream base
images each release was built from.** ...

**The manifest is bumped only by human-reviewed PR.** ...

**Drift detection.** A scheduled workflow ...
```

**Pattern to copy** for the new `### Image signatures + attestations (v1.6 onward)` subsection (Phase 23's analog):

- H3 heading named `### <topic> (v1.6 onward)` — exact Phase 22 mirror.
- Opening paragraph: short prose stating WHAT is signed/attested + cross-referencing the workflow file path.
- Bold lead-in claims (`**`) for each artifact class — Phase 22 has three (`canonical record`, `bumped only by`, `Drift detection`); Phase 23 has three (`Signed by cosign`, `Attested with SLSA`, `Attested with SPDX SBOM`).
- Embedded inline links to the workflow file in `.github/workflows/...` form, exactly as the Phase 22 `[`.github/workflows/digest-drift-check.yml`](.github/workflows/digest-drift-check.yml)` form.
- Strikethrough on the "Known gaps at v1.5" Docker bullet at [SECURITY.md:110-114](SECURITY.md#L110), mirroring the Phase 22 strikethrough at [SECURITY.md:120](SECURITY.md#L120).

**Destination shape** (from RESEARCH.md §4 — the planner copies the full block verbatim into SECURITY.md):

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

[... fenced bash recipes block — 4 recipes; see RESEARCH §4 for the full text ...]

> **Note: GHCR UI "Unverified" badge** is unrelated to cosign verification
> (Pitfall 10). ...

> **Note: cosign 3.0 CLI flag drift** (Pitfall 13). The recipes above have
> been tested with **cosign `>= 2.6.3, < 3.0.0`**. ...
```

**Strikethrough on the "Known gaps at v1.5" Docker-images bullet** ([SECURITY.md:110-114](SECURITY.md#L110)):

```markdown
- **~~Docker images on `ghcr.io` are unsigned.~~** **Closed in v1.6 Phase 23**
  — see [Image signatures + attestations (v1.6 onward)](#image-signatures--attestations-v16-onward).
```

Same `~~strikethrough~~ + **Closed in v1.6 (Phase X)** — see [...](...)` shape Phase 22 already shipped at [SECURITY.md:120](SECURITY.md#L120). Per RESEARCH §8 Q3, do NOT forward-strikethrough the tarball bullet at [SECURITY.md:104-109](SECURITY.md#L104) — Phase 24 owns that anchor.

---

## Shared Patterns

### Shared #1: Job-level `permissions:` block — `id-token: write` + `attestations: write` (NEW; auditor-grepable deliberately-omitted-scopes comment block)

**Source-of-truth analogs:**

**(a) Workflow-level minimum-permissions** ([docker.yml:28-29](.github/workflows/docker.yml#L28)):

```yaml
permissions:
  contents: read
```

**(b) Job-level scope-additive permissions** ([docker.yml:61-63](.github/workflows/docker.yml#L61)):

```yaml
    permissions:
      contents: read
      packages: write
```

**(c) Auditor-grepable deliberately-omitted-scopes comment block** ([digest-drift-check.yml:35-46](.github/workflows/digest-drift-check.yml#L35) — the Phase 22 Plan 22-04 pattern this phase mirrors):

```yaml
# Minimum privileges:
#   - contents: read   — checkout + read docker/digests.txt
#   - issues:   write  — `gh issue create` / `gh issue list` on drift
# Deliberately omitted scopes (auditor-grepable: these tokens MUST NOT appear
# anywhere in this file): packages (we don't push anything), id-token (no
# cosign here — that's Phase 23), and PR-write (PITFALLS.md §11 — drift
# opens issues only, never PRs; auto-merging digest bumps would defeat the
# entire supply-chain assurance this milestone is closing).
permissions:
  contents: read
  issues: write
```

**Pattern to copy** — the key audit-grepable property is the literal `PR-write` token (NOT `pull-requests:` with the colon) in the deliberately-omitted list. This way a future file-level audit gate `! grep -q 'pull-requests:'` on this workflow continues to pass even after the comment is added. Phase 22's literal lesson per CONTEXT.md D-02.

**Destination shape** (CONTEXT D-02 + RESEARCH §2.1 correction — Phase 23 modifies the `docker` job's permissions block at [docker.yml:61-63](.github/workflows/docker.yml#L61)):

```yaml
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

**Apply to:** the `docker` job in `docker.yml` (NOT the `check` job at [docker.yml:32](.github/workflows/docker.yml#L32); NOT the workflow-level block at [docker.yml:28-29](.github/workflows/docker.yml#L28)).

---

### Shared #2: SHA-pin trailing-comment style (every new `uses:` line)

**Source-of-truth excerpts** (representative lines from `docker.yml` / `release.yml` / `ci.yml`):

- [docker.yml:36](.github/workflows/docker.yml#L36) — `      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`
- [docker.yml:91](.github/workflows/docker.yml#L91) — `      - uses: docker/metadata-action@c299e40c65443455700f0fdfc63efafe5b349051 # v5.10.0`
- [docker.yml:110](.github/workflows/docker.yml#L110) — `      - uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0`
- [release.yml:101](.github/workflows/release.yml#L101) — `        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`
- [ci.yml:48](.github/workflows/ci.yml#L48) — `        uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0`

**Pattern to copy:**
- Form: `uses: <owner>/<action>@<40-hex-SHA> # v<X.Y.Z>` (or `# stable` for `dtolnay/rust-toolchain`, but new sigstore actions never use the `# stable` form — always `# v<X.Y.Z>`).
- TWO spaces before the `#`.
- Full 40 hex chars (the `D-04 sigstore-pin-check` gate enforces this).

**Apply to:** every new `uses:` line in `docker.yml` (4 lines: cosign-installer + sbom-action + attest-sbom + attest-build-provenance) AND the new `actions/checkout` line in `ci.yml`'s `sigstore-pin-check` job. The new pins resolved in RESEARCH §2.3:

| Action | Pin |
|--------|-----|
| `sigstore/cosign-installer` | `@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` |
| `anchore/sbom-action` | `@e22c389904149dbc22b58101806040fa8d37a610 # v0.24.0` |
| `actions/attest-build-provenance` | `@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0` |
| `actions/attest-sbom` | `@bd218ad0dbcb3e146bd073d1d9c6d78e08aa8a0b # v2.4.0` |

---

### Shared #3: Comments-as-contract above every new structural block

**Source-of-truth analogs:**

**(a) Above the workflow-level `env:` block** ([docker.yml:3-14](.github/workflows/docker.yml#L3)):

```yaml
env:
  # Force GitHub Actions runner to execute Node 20 JS actions on Node 24,
  # silencing the deprecation warning ahead of the June 2026 hard cutover.
  # See: actions/checkout v6.0.2 still declares `using: node20` — upgrading
  # the action SHA is tracked separately (see TODO at top of ci.yml).
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
  # Release-smoke gate (P0-5, v1.5 release-readiness): a tag push cannot
  # publish Docker images to ghcr.io unless the integration suite has
  # executed against the pinned bitcoind. ...
  BLINDJOIN_REQUIRE_BITCOIND: "1"
```

**(b) Above the `on:` block** ([docker.yml:19-26](.github/workflows/docker.yml#L19)):

```yaml
on:
  push:
    tags: ['v*']
  # workflow_dispatch enables rehearsal of the check job (release-smoke
  # gate under BLINDJOIN_REQUIRE_BITCOIND=1 via the composite
  # install-bitcoind action) without cutting a tag or pushing images.
  # Trigger from the Actions tab on any branch; the `docker` matrix job
  # below is gated on `refs/tags/*` so a dispatch run runs check only
  # and stops short of pushing to ghcr.io. See
  # .planning/quick/260531-ubf-*/SUMMARY.md for the rehearsal procedure.
  workflow_dispatch:
```

**Pattern to copy:**
- Every new env var, every new permission line, every new step gets a multi-line `#` comment block IMMEDIATELY ABOVE the structural element.
- Comment cites: (a) the REQ-ID (`Phase 22 DRIFT-03`, `P0-5`, `v1.4 ADR Decision #1`), (b) the WHY (the failure mode this avoids), (c) cross-references to related files (`.planning/quick/...`, `PITFALLS.md §N`, `SECURITY.md § Supply-chain status`).
- Cause-to-effect-to-source structure: `<failure if missing> — <how it manifests> — <where the design is recorded>`.

**Apply to:** all new edits in `docker.yml` (§1–§6 above), the new job in `ci.yml` (§7), and the rewritten subsections in `SECURITY.md` (§8). The CONTEXT.md `<specifics>` block names the exact wording for the `id-token: write` comment as auditor-grepable.

---

## HUMAN-UAT artifact shape (D-06 Stage 1 + Stage 2 rehearsal log)

**Analog:** `.planning/milestones/v1.5-quick/260531-thw-v1-5-release-readiness-p0s-security-md-c/260531-thw-SUMMARY.md` + `.planning/milestones/v1.5-quick/260531-ubf-post-release-readiness-polish-remove-amp/260531-ubf-SUMMARY.md`. CONTEXT D-06 explicitly names both as the shape mirror.

**Source-of-truth excerpts:**

**(a) YAML frontmatter shape** (260531-thw-SUMMARY.md lines 1-23):

```markdown
---
quick_id: 260531-thw
status: complete
description: v1.5 release-readiness P0s (SECURITY.md + CHANGELOG, BACKLOG prune, CI integration, Dockerfile pins)
date: 2026-06-01
commits:
  - adc2aa6 — docs(quick-260531-thw): add SECURITY.md + CHANGELOG.md (P0-1)
  - 870ff71 — docs(quick-260531-thw): mark B-01/B-02 shipped + bump audit.toml date (P0-4)
  - 6fea538 — ci(quick-260531-thw): release-smoke runs integration suite (P0-5)
  - 578a903 — build(quick-260531-thw): pin Dockerfile bases via ARG + bump bitcoind 27→30 (P0-2/3)
files_changed:
  added:
    - SECURITY.md
    - CHANGELOG.md
    - .github/actions/install-bitcoind/action.yml
  modified:
    - .planning/BACKLOG.md
    - .cargo/audit.toml
    - .github/workflows/release.yml
    - .github/workflows/docker.yml
    - docker/Dockerfile
    - docker/docker-compose.yml
---
```

**(b) Task-level PASS/FAIL log shape** (260531-thw-SUMMARY.md `## P0-1 — ... ✓` heading style + bullet-list verification subsection — and 260531-ubf-SUMMARY.md `## Task D — Release-smoke rehearsal via workflow_dispatch ✓` paragraph + step-by-step procedure).

**Pattern to copy:**
- File path: `.planning/quick/YYMMDD-<3-letter-slug>-<kebab-descriptive-name>/YYMMDD-<slug>-SUMMARY.md`.
- YAML frontmatter: `quick_id`, `status`, `description`, `date`, `commits` (list of `<sha7> — <subject>`), `files_changed` (split `added` / `modified` / `removed`).
- Body: `# Quick task YYMMDD-<slug> — Summary` H1.
- One `## Task <letter> — <name> ✓` (or `## Stage <number> — <name>` for Phase 23's two-stage rehearsal per RESEARCH §6) per discrete unit.
- Each task subsection: prose + verification commands + commit SHA.
- For Phase 23 Stage 2: an additional table with one row per recipe×image (per RESEARCH §6 — 13 rows: 3 images × 4 recipes + 1 negative test), each scored `PASS / FAIL` + notes column.

**Destination shape** for Phase 23 Stage 2 table (verbatim from RESEARCH §6, planner copies into the SUMMARY.md):

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
| ...
| 4. cosign save + offline verify | liquidity-bot | PASS / FAIL | |
| Negative test (distroless/static) | n/a | PASS / FAIL | proves regex not over-wide |

**Verdict:** GO for v1.6.0 / NO-GO (with reason).
```

---

## No Analog Found

| File / asset | Role | Data flow | Reason |
|--------------|------|-----------|--------|
| `gh attestation verify oci://...` recipe | docs CLI snippet | n/a | `gh` is preinstalled on `ubuntu-24.04` runners and is not currently used by any in-repo workflow; the cosign/gh combination is novel to Phase 23 and SECURITY.md. No in-repo `gh attestation verify` analog exists. RESEARCH §6 provides the canonical verified invocation. |
| `cosign save --dir` offline-bundle recipe | docs CLI snippet | n/a | No in-repo `cosign save` usage. RESEARCH §3.4 corrected the CONTEXT D-07 nonexistent `cosign download signature --bundle` CLI to this. RESEARCH §4 supplies the canonical recipe text for SECURITY.md. |

Both novel patterns are documentation-only and live exclusively in `SECURITY.md`'s new `### Image signatures + attestations (v1.6 onward)` subsection. The planner copies them VERBATIM from RESEARCH §4.

---

## Metadata

**Analog search scope:**
- `.github/workflows/docker.yml` (full — 127 lines)
- `.github/workflows/ci.yml` (full — 290 lines)
- `.github/workflows/release.yml` (`uses:` lines + SHA-pin style)
- `.github/workflows/digest-drift-check.yml` (auditor-grepable deliberately-omitted-scopes pattern)
- `SECURITY.md` (full — 226 lines)
- `.planning/milestones/v1.5-quick/260531-thw-.../260531-thw-SUMMARY.md` (HUMAN-UAT frontmatter + task shape)
- `.planning/milestones/v1.5-quick/260531-ubf-.../260531-ubf-SUMMARY.md` (HUMAN-UAT task + procedure shape)
- `.planning/phases/22-base-image-digest-drift-detection/22-PATTERNS.md` (predecessor pattern-map style)
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-CONTEXT.md` (decisions + code_context)
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-RESEARCH.md` (corrected facts + concrete YAML)
- `.planning/REQUIREMENTS.md` (ATTEST-01 through ATTEST-04 verbatim)

**Files scanned:** 11 (all in-repo)

**Pattern extraction date:** 2026-06-01
