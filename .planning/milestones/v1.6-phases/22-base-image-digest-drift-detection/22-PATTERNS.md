# Phase 22: Base-Image Digest Drift Detection - Pattern Map

**Mapped:** 2026-06-01
**Files analyzed:** 9 (4 new, 5 modified — one of those is no-touch verify-only)
**Analogs found:** 8 / 9 (CODEOWNERS has no in-repo analog by design)

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `docker/digests.txt` (NEW) | config / manifest | file-I/O (read-only by CI) | `docker/Dockerfile` top-of-file ARG comments | role-match (manifest header style); also `.bitcoind-version` pin pattern |
| `.github/actions/read-base-digests/action.yml` (NEW) | composite action / utility | transform (file → named outputs) | `.github/actions/install-bitcoind/action.yml` | exact (CONTEXT.md D-01 names this as the mirror) |
| `.github/workflows/digest-drift-check.yml` (NEW) | scheduled workflow | event-driven (cron + dispatch) | `.github/workflows/docker.yml` (workflow scaffold + permissions+env+comments) | partial (no existing `schedule:` cron in repo — confirmed by RESEARCH.md §2.3) |
| `.github/CODEOWNERS` (NEW) | config | static-mapping | none in repo (reference-driven, see §No Analog Found) | n/a |
| `.github/workflows/release.yml` (MODIFY) | workflow | request-response (CI run) | self (insertion-point pattern from existing job) | exact (modify in place) |
| `.github/workflows/docker.yml` (MODIFY) | workflow | request-response (CI run) | self (insertion-point pattern from existing job) | exact (modify in place) |
| `SECURITY.md` (MODIFY) | docs / policy | n/a | self (existing §Supply-chain status prose) | exact (extend in place) |
| `CONTRIBUTING.md` (MODIFY) | docs / contributor-facing | n/a | self (existing §Tagging releases section style) | exact (insertion after §Tagging releases) |
| `docker/Dockerfile` (VERIFY ONLY) | build recipe | n/a | self — verify `ARG DEBIAN_REF` / `ARG CARGO_CHEF_REF` names match composite-action outputs | exact (no edits — names are already correct at lines 32–33) |

---

## Pattern Assignments

### `.github/actions/read-base-digests/action.yml` (composite action, transform)

**Analog:** `.github/actions/install-bitcoind/action.yml` (CONTEXT.md D-01 names this byte-for-byte structural mirror)

**Top-of-file `name:` + `description:` block** (`.github/actions/install-bitcoind/action.yml` lines 1–15):

```yaml
name: Install pinned bitcoind
description: >
  Installs the bitcoind binary version pinned in `.bitcoind-version`,
  PGP-fingerprint-verified against achow101's release-signer key fetched
  from a SHA-pinned bitcoin-core/guix.sigs commit, and SHA-256-verified
  against `SHA256SUMS`. Caches the binary at runner level keyed on
  runner.os + version. Exports `BITCOIND_EXE` for `corepc-node` to pick
  up via its env-var first-precedence path (verified at
  corepc-node-0.12.0 node/src/lib.rs:635).

  Composite source-of-truth for v1.6+: ci.yml, release.yml, and docker.yml
  all `uses: ./.github/actions/install-bitcoind` so the verification gate
  does not drift between workflows. See `.planning/quick/260531-thw-*/`
  for the extraction context (P0-5).
```

Pattern to copy:
- `name:` is a short imperative phrase ("Install pinned bitcoind" → "Read base-image digests")
- `description:` is a `>` folded-scalar block, multi-paragraph
- First paragraph describes WHAT the action does
- Final paragraph is the **"Composite source-of-truth for v1.6+:"** sentence naming every workflow that consumes the action — this exact phrase is part of the project's audit-trail style. Copy verbatim, swap callers.
- References the extraction-context planning directory (`.planning/quick/...`) — for Phase 22 the analog cite is `.planning/phases/22-base-image-digest-drift-detection/`.

**`runs:` + composite-step structure** (`.github/actions/install-bitcoind/action.yml` lines 16–22 + 19–22 specifically):

```yaml
runs:
  using: composite
  steps:
    - name: Read pinned bitcoind version
      id: bitcoind_version
      shell: bash
      run: echo "version=$(cat .bitcoind-version)" >> $GITHUB_OUTPUT
```

Pattern to copy:
- `using: composite` — exact
- Each step has `name:`, `id:`, `shell: bash`, then `run:` block
- Outputs emitted via `>> $GITHUB_OUTPUT` (NOT the older `::set-output` form)
- The `id:` is referenced from `outputs:` block via `${{ steps.<id>.outputs.<name> }}`

**Inputs / outputs block (NEW — install-bitcoind has no inputs/outputs declared at top because it only writes to `$GITHUB_ENV`)**: Phase 22 action declares `inputs: {}` and a top-level `outputs:` block per RESEARCH.md §3:

```yaml
inputs: {}

outputs:
  debian_ref:
    description: Full pinned reference for debian:bookworm-slim (image:tag@sha256:HEX).
    value: ${{ steps.parse.outputs.debian_ref }}
  cargo_chef_ref:
    description: Full pinned reference for lukemathwalker/cargo-chef:latest-rust-1 (image:tag@sha256:HEX).
    value: ${{ steps.parse.outputs.cargo_chef_ref }}
```

This shape is NOT in install-bitcoind (which exports via `$GITHUB_ENV`), but it IS the standard composite-action `outputs:` form per GitHub Actions docs. The `value:` references `steps.<id>.outputs.<name>` from the composite step.

**Inline error-handling pattern** (`.github/actions/install-bitcoind/action.yml` lines 60–63 + 91–93):

```bash
gpg --list-keys --with-colons | grep -q "${KEY_FP}" \
  || { echo "ERROR: expected fingerprint ${KEY_FP} not found"; exit 1; }
...
grep -q "^\[GNUPG:\] VALIDSIG ${KEY_FP} " /tmp/gpg-status.txt \
  || { echo "ERROR: SHA256SUMS.asc does not have a VALIDSIG from achow101 fingerprint ${KEY_FP}"; exit 1; }
```

Pattern to copy:
- `set -euo pipefail` at top of `run:` block
- Inline `|| { echo "ERROR: ..."; exit 1; }` for fail-fast
- For Phase 22 D-03, swap the bare `ERROR:` prefix for the auditor-facing `supply-chain:` prefix and trailing `Refusing to build without a valid manifest.` per RESEARCH.md §2.4 — but the **shape** (echo to stderr, exit 1, no rescue) is identical.

**Inline-comment-as-audit-trail pattern** (`.github/actions/install-bitcoind/action.yml` lines 37–48 — the prose comment block above the integrity-gate `run:` step):

```yaml
    - name: Install bitcoind (cache miss only)
      if: steps.cache-bitcoind.outputs.cache-hit != 'true'
      shell: bash
      # Integrity gate (per D-04):
      #   1. Fetch achow101's PGP key from a SHA-pinned guix.sigs commit
      #      (avoids public keyserver flake — RESEARCH.md Pitfall 5).
      #   2. Verify imported key fingerprint matches the KEY_FP value set
      #      below (catches a hostile guix.sigs commit substituting a
      #      different key).
      #   3. gpg-verify SHA256SUMS.asc against SHA256SUMS (signed by achow101).
      #   4. hash-check the tarball against the signed SHA256SUMS entry.
```

Pattern to copy: comment block ABOVE the `run:` block, numbered list of contract steps, each referencing the locked decision ID (here `D-04`; for Phase 22 use `D-03`).

---

### `.github/workflows/digest-drift-check.yml` (workflow, event-driven)

**Analog:** `.github/workflows/docker.yml` for the scaffold (env + permissions + comments-as-contract style); RESEARCH.md §4 for the body.

**Top-of-file `name:` + comment block** (`.github/workflows/docker.yml` lines 1–14):

```yaml
name: Docker

env:
  # Force GitHub Actions runner to execute Node 20 JS actions on Node 24,
  # silencing the deprecation warning ahead of the June 2026 hard cutover.
  # See: actions/checkout v6.0.2 still declares `using: node20` — upgrading
  # the action SHA is tracked separately (see TODO at top of ci.yml).
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
  # Release-smoke gate (P0-5, v1.5 release-readiness): a tag push cannot
  # publish Docker images to ghcr.io unless the integration suite has
  # executed against the pinned bitcoind. Same trade-off as release.yml:
  # ~30s cache hit, ~90s cache miss; acceptable for release-grade
  # confidence on Docker images that operators will pull and run.
  BLINDJOIN_REQUIRE_BITCOIND: "1"
```

Pattern to copy:
- Single-word `name:` ("Docker" → "Digest drift check")
- `env:` block has a multi-line `#` comment ABOVE each env var explaining what it enforces
- The `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` env var is **boilerplate inherited across all workflows in this repo** — copy verbatim into the new workflow

**`on:` trigger pattern with workflow_dispatch rehearsal** (`.github/workflows/docker.yml` lines 16–26):

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

Pattern to copy:
- `workflow_dispatch:` ALWAYS has a prose comment ABOVE it explaining the rehearsal purpose
- The comment ALWAYS references the rehearsal-context planning doc (here `.planning/quick/260531-ubf-*/SUMMARY.md`)
- For Phase 22, the trigger is `schedule:` + `workflow_dispatch:`, not `push:` — but the prose-comment-above style applies. RESEARCH.md §4 lines 343–349 already provides the exact cron comment.

**`permissions:` block with prose-comment-as-contract** (`.github/workflows/docker.yml` lines 28–29 + the matrix-job-level `permissions:` at lines 61–63):

```yaml
permissions:
  contents: read
```

And within the `docker:` job:

```yaml
    permissions:
      contents: read
      packages: write
```

Pattern to copy:
- Top-level `permissions:` is conservative (`contents: read` for read-only workflows)
- Job-level `permissions:` only declares the additional scopes needed for that specific job
- For Phase 22 `digest-drift-check.yml`, the workflow needs `contents: read` + `issues: write` — per RESEARCH.md §4 lines 352–358, the comment style is:
  ```yaml
  # Minimum privileges:
  #   - contents: read   — checkout + read docker/digests.txt
  #   - issues:   write  — `gh issue create` / `gh issue list` on drift
  # No `packages:` (we don't push anything). No `id-token:` (no cosign here).
  permissions:
    contents: read
    issues: write
  ```
  This shape — the "Minimum privileges:" comment block followed by `# No X. No Y.` justifications — is the project's `permissions:`-block-prose-comments-as-contract style.

**SHA-pin `# vX.Y.Z` comment style on `uses:` lines** (every `uses:` line across release.yml, docker.yml, ci.yml — verified via `grep -n "uses:.* # v"`):

```yaml
- uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
- uses: docker/login-action@4907a6ddec9925e35a0a9e82d7399ccc52663121 # v4.1.0
- uses: docker/setup-buildx-action@4d04d5d9486b7bd6fa91e7baf45bbb4f8b9deedd # v4.0.0
- uses: docker/metadata-action@c299e40c65443455700f0fdfc63efafe5b349051 # v5.10.0
- uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0
- uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
- uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2
- uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0
- uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
```

Pattern to copy:
- Format: `<owner>/<repo>@<40-char-sha> # v<X.Y.Z>` (or `# stable` for rustup toolchain, or `# v2` for minor-only pins like Swatinem/rust-cache)
- Two spaces between SHA and `#`
- For Phase 22 `digest-drift-check.yml`, the ONLY third-party action used is `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` — already SHA-pinned across the repo, copy that exact line.
- Local composite actions (e.g. `./.github/actions/read-base-digests`) are SHA-implicit via repo checkout — NO trailing comment needed (mirrors `- uses: ./.github/actions/install-bitcoind` in release.yml line 50 + docker.yml line 44).

---

### `.github/workflows/release.yml` (MODIFY — insert composite-action call in `build` job)

**Analog:** self. The `build:` job at lines 60–96 is the integration point. Insertion goes between `Swatinem/rust-cache` (line 75) and `Build coordinator and client` (line 77).

**Current `build:` job step ordering** (`.github/workflows/release.yml` lines 60–96):

```yaml
  build:
    name: Build linux-amd64
    needs: check
    runs-on: ubuntu-latest
    # Publish gate: only run on a real tag push. workflow_dispatch runs
    # check-only (the rehearsal path) and never uploads release artifacts.
    if: startsWith(github.ref, 'refs/tags/')

    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
        with:
          toolchain: stable

      - uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2

      - name: Build coordinator and client
        run: cargo build --release --bin coordinator --bin client --bin liquidity-bot

      - name: Package
        run: |
          mkdir -p dist
          ...

      - name: Upload to GitHub Releases
        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
```

Pattern to respect:
- Step ordering: `checkout` → `rust-toolchain` → `rust-cache` → **[insert `read-base-digests` here]** → `Build coordinator and client` → `Package` → `Upload to GitHub Releases`
- `if: startsWith(github.ref, 'refs/tags/')` is at the JOB level, not the step level — the composite-action call inherits the tag-gate from the job
- The `check` job at lines 32–58 already uses `- name: Install pinned bitcoind` / `uses: ./.github/actions/install-bitcoind` at line 49–50 — the new `Read canonical base-image digests` step in `build` mirrors that local-action invocation style exactly
- No new `permissions:` needed — top-level `permissions: contents: write` (line 28–29) covers `contents: read` for the composite action
- Insertion shape per RESEARCH.md §5.1 lines 562–571 (8-line prose comment ABOVE the step explaining its supply-chain-gate role)

---

### `.github/workflows/docker.yml` (MODIFY — insert composite-action call + thread outputs into build-args)

**Analog:** self. The `docker:` matrix job at lines 54–108 is the integration point. Insertion goes between `docker/metadata-action` (line 91) and `docker/build-push-action` (line 99), and the existing `docker/build-push-action` `with:` block (lines 100–108) gains a new `build-args:` field.

**Current `docker:` job step ordering** (`.github/workflows/docker.yml` lines 75–108):

```yaml
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - uses: docker/login-action@4907a6ddec9925e35a0a9e82d7399ccc52663121 # v4.1.0
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - uses: docker/setup-buildx-action@4d04d5d9486b7bd6fa91e7baf45bbb4f8b9deedd # v4.0.0

      # NOTE: `type=semver` requires strict 3-part tags (vX.Y.Z). Two-part tags
      # like `v1.3` produce zero image tags and fail this job at the
      # `docker buildx build --push` step with `tag is needed when pushing to
      # registry`. Always tag milestone releases as `vMAJOR.MINOR.PATCH` — see
      # CONTRIBUTING.md § "Tagging releases".
      - uses: docker/metadata-action@c299e40c65443455700f0fdfc63efafe5b349051 # v5.10.0
        id: meta
        with:
          images: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}

      - uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0
        with:
          context: .
          file: docker/Dockerfile
          target: ${{ matrix.target }}
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

Pattern to respect:
- Step ordering: `checkout` → `docker/login-action` → `docker/setup-buildx-action` → `docker/metadata-action` (id: meta) → **[insert `read-base-digests` here]** → `docker/build-push-action` (with: block gains `build-args:`)
- The `docker/build-push-action` `with:` block currently has 8 fields (context, file, target, push, tags, labels, cache-from, cache-to). The new `build-args:` field is added between `labels:` and `cache-from:` per RESEARCH.md §5.2 lines 651–667.
- `build-args:` uses pipe-multiline syntax (one `KEY=value` per line, matching the existing `tags: |` style on line 95–97):
  ```yaml
  build-args: |
    DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}
    CARGO_CHEF_REF=${{ steps.digests.outputs.cargo_chef_ref }}
  ```
- `${{ matrix.image }}` and `${{ matrix.target }}` interpolation is already used (lines 55, 94, 103) — the new `${{ steps.digests.outputs.* }}` references follow the same form
- No new `permissions:` needed — the job already has `contents: read` + `packages: write` (lines 61–63); composite action needs only `contents: read` (covered)
- Insertion-point comment shape per RESEARCH.md §5.2 lines 640–646 (7-line prose comment ABOVE the `Read canonical base-image digests` step explaining its supply-chain-gate role and crediting the v1.5 P0-2/3 Dockerfile ARG scaffold)

---

### `SECURITY.md` (MODIFY — update §Supply-chain status)

**Analog:** self. The existing `## Supply-chain status` section at lines 95–145 is the insertion target.

**Existing §Supply-chain status structure** (`SECURITY.md` lines 95–145):

```markdown
## Supply-chain status

blindjoin's release artifacts have **known supply-chain gaps** at v1.5.
They are documented here, not hidden. If you operate blindjoin in any
environment where supply-chain assurance matters, read this section
before pulling a binary or image.

### Known gaps at v1.5

- **GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign
  signature.** A user pulling
  `blindjoin-linux-amd64.tar.gz` from a GitHub Release verifies the
  archive is intact (the `.sha256` companion file), but cannot
  cryptographically attribute the archive to the maintainer. A compromised
  GitHub account could publish a replaced binary with a matching checksum.
- **Docker images on `ghcr.io` are unsigned.** No cosign attestation, no
  Notary v2 signature, no Sigstore witness. Pulling
  ...
- **Base image digest pins are manual.** The `docker/Dockerfile` pins
  `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`
  by digest as of v1.5, but bumping these digests requires a maintainer
  to verify the new digest against a clean `docker pull` on a clean
  runner. There is no automated drift check.

### v1.6 supply-chain plan

The next milestone is expected to close the unsigned-build gap:

- **cosign image attestations** on the Docker images pushed to `ghcr.io`,
  ...
- **Detached signatures** on GitHub Release archives — either cosign
  blob signatures or detached PGP signatures, depending on the audit
  feedback.
- **Reproducible-build instructions** for the release archive,
  ...
- **Automated base-image digest drift check** so a stale digest in
  `docker/Dockerfile` fails CI rather than silently sticking.

Until those land, **treat the SHA-256 checksum + the GitHub Release
provenance as the only assurance the archive came from this project**.
For higher assurance, build from source on a known-good toolchain and
verify against the committed `Cargo.lock`.
```

Pattern to respect — **audit-charter prose voice**:
- Section opens with a one-paragraph orientation, then breaks into `###` subsections
- `###` subsections are `Known gaps at v1.X` / `v1.X supply-chain plan` — additive (Phase 22 adds `### Base-image digests (v1.6 onward)` per RESEARCH.md §7.1)
- Each bulleted gap is **bold lede + sentence(s)** (e.g. `**Docker images on \`ghcr.io\` are unsigned.** No cosign attestation, ...`)
- Direct, declarative voice — "blindjoin's release artifacts have known supply-chain gaps" — not hedging ("may have")
- Cross-references use markdown links with relative paths: `[external audit charter](docs/AUDIT-CHARTER.md)`, `[`docker/digests.txt`](docker/digests.txt)`
- Strikethrough + "Closed in v1.6" annotation for items moving from "Known gaps" → "shipped" (RESEARCH.md §7.1 lines 748–754):
  ```markdown
  - **~~Base image digest pins are manual.~~** **Closed in v1.6** — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).
  ```
- The closing "Until those land, **treat ...**" paragraph (lines 142–145) is the section's standing operator-facing caveat — keep the form when adding new subsections.

The new `### Base-image digests (v1.6 onward)` subsection text is locked verbatim in RESEARCH.md §7.1 lines 707–746; the planner copies that block directly.

---

### `CONTRIBUTING.md` (MODIFY — add §Bumping base-image digests)

**Analog:** self. The existing `## Tagging releases` section at lines 69–94 is the closest stylistic match and the named insertion point (RESEARCH.md §7.2 line 758: "Insertion point: after the existing `## Tagging releases` section").

**Existing §Tagging releases structure** (`CONTRIBUTING.md` lines 69–94):

```markdown
## Tagging releases

Milestone tags must follow strict 3-part semver: `vMAJOR.MINOR.PATCH` (e.g. `v1.3.0`, not `v1.3`).

**Why:** [.github/workflows/docker.yml](.github/workflows/docker.yml) uses `docker/metadata-action` with `type=semver,pattern={{version}}`, which only matches `vX.Y.Z`. A two-part tag like `v1.3` produces zero image tags, and `docker buildx build --push` then fails with `tag is needed when pushing to registry`. The Docker workflow has silently failed on every two-part tag (`v1.0`, `v1.1`, `v1.3`) and only ever succeeded on `v1.0.0`.

**Before tagging:** add a new `## [X.Y.Z] — YYYY-MM-DD` section to [CHANGELOG.md](CHANGELOG.md) and move any unreleased bullets into it. The CHANGELOG is the user-facing release-notes surface; commit it as part of the milestone close, not after the tag.

**Crate versions in `Cargo.toml` stay at `0.1.0`** by policy — see [SECURITY.md § Release versioning policy](SECURITY.md#release-versioning-policy). The git tag is the canonical release identifier; the four workspace crates are unpublished, so bumping their `version =` lines would be churn with no consumer benefit.

**Tagging a milestone close:**

\`\`\`bash
git tag -a v1.X.0 -m "v1.X <Milestone name>

<one-line delivered summary>

Key accomplishments:
- ...

See .planning/MILESTONES.md for full details."

git push origin v1.X.0
\`\`\`

The milestone *name* in planning docs (e.g. `v1.3 Test Infrastructure & Operational Hardening`) is independent of the git tag — docs may stay `v1.X` for readability while the tag is `v1.X.0`.
```

Pattern to respect — **contributor-facing PR-etiquette voice**:
- `##` section header is imperative gerund: "Tagging releases" → for Phase 22, "Bumping base-image digests" (matches RESEARCH.md §7.2 line 762)
- Section opens with a **one-sentence statement of the rule** (here: "Milestone tags must follow strict 3-part semver...")
- Sub-callouts use bold ledes: `**Why:**`, `**Before tagging:**`, `**Tagging a milestone close:**`
- Fenced bash blocks for exact commands the contributor copies
- Cross-references use markdown links with relative paths
- Voice is second-person-imperative ("**Before tagging:** add a new ..."), NOT passive

The new `## Bumping base-image digests` section text is locked verbatim in RESEARCH.md §7.2 lines 762–809; the planner copies that block directly.

---

### `docker/digests.txt` (NEW — canonical manifest)

**Closest analog:** `docker/Dockerfile` top-of-file ARG comments (lines 1–33) for the project's manifest-header documentation style; secondarily `.bitcoind-version` (single-line pinned-version file).

**`docker/Dockerfile` header pattern** (lines 1–33):

```dockerfile
# docker/Dockerfile
# Single multi-stage build for all binaries using cargo-chef for layer caching.
#
# Supply-chain pinning (P0-2/3, v1.5 release-readiness):
#   The two base image references below are ARG-overridable so a release build
#   can pin to a digest without editing the Dockerfile. The defaults are
#   floating tags for developer ergonomics; tagged-release CI MUST override
#   both ARGs with a digest-form reference.
...
#   The v1.6 supply-chain milestone (see SECURITY.md § Supply-chain status)
#   will: (a) automate the digest refresh + drift check via CI; (b) emit
#   cosign attestations on the published image; (c) publish reproducible-
#   build instructions so anyone can verify the digest matches source.

ARG CARGO_CHEF_REF=lukemathwalker/cargo-chef:latest-rust-1
ARG DEBIAN_REF=debian:bookworm-slim
```

Pattern to copy for `docker/digests.txt`:
- File header is a `#`-commented prose block explaining (a) what the file is, (b) why it exists, (c) what enforces the format
- References load-bearing cross-files: `SECURITY.md §Supply-chain status` and `.github/actions/read-base-digests/action.yml`
- Data lines below the header have NO inline comment — the parser strips comments + blanks before validation (per RESEARCH.md §3 lines 242–244)
- Manifest format locked by composite-action regex: `^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$`

The exact file content is locked verbatim in RESEARCH.md §3 lines 288–297.

**Critical cross-file invariant (verify-only on `docker/Dockerfile`):** the ARG names `DEBIAN_REF` and `CARGO_CHEF_REF` on Dockerfile lines 32–33 MUST match the composite-action output names (`debian_ref` / `cargo_chef_ref`) and the `--build-arg KEY=value` strings docker.yml passes. Verified against current Dockerfile content — names already match. **Phase 22 does NOT modify the Dockerfile.**

---

### `.github/CODEOWNERS` (NEW — 2-line file)

**Closest analog: NONE in repo.** No existing CODEOWNERS file (verified via `ls .github/CODEOWNERS` → not found). The format is GitHub-product-locked, not project-locked, so it is reference-driven from the GitHub Docs (https://docs.github.com/en/repositories/managing-your-repositories-settings-and-security/customizing-your-repository/about-code-owners), and the file content is locked verbatim in RESEARCH.md §6 lines 680–689.

Pattern to apply (from RESEARCH.md §6):

```
# Phase 22 (v1.6) supply-chain gate. Per .planning/research/PITFALLS.md §11
# and SECURITY.md §Supply-chain status, the canonical base-image digest
# manifest and the parser that reads it both require maintainer approval
# on every PR that touches them. Branch protection on `main` enforcing
# CODEOWNERS approval is the structural mechanism that prevents
# auto-merge bots from bypassing human review.
docker/digests.txt                         @johnzilla
.github/actions/read-base-digests/**       @johnzilla
```

Pattern notes:
- File header is `#`-commented prose with the same audit-trail style as Dockerfile / digests.txt headers
- Lists the load-bearing cross-references (`PITFALLS.md §11`, `SECURITY.md §Supply-chain status`)
- Maintainer handle `@johnzilla` verified per RESEARCH.md §6 line 693 — corroborated by `.planning/research/SUMMARY.md` cosign-identity references and the SECURITY.md email-handle correspondence

---

## Shared Patterns

### Composite-action invocation in workflows (local action, SHA-implicit)
**Source:** `.github/workflows/release.yml` line 49–50 + `.github/workflows/docker.yml` line 43–44
**Apply to:** `release.yml` build job, `docker.yml` docker job, `digest-drift-check.yml` drift-check job

```yaml
      # Composite source-of-truth — see .github/actions/install-bitcoind/action.yml.
      # ...
      - name: Install pinned bitcoind
        uses: ./.github/actions/install-bitcoind
```

Pattern to copy:
- One-line prose comment ABOVE the step pointing to the action's `action.yml` as source-of-truth
- `- name:` is a short imperative phrase
- `uses: ./.github/actions/<name>` — relative path, NO SHA, NO `@` (local actions are SHA-implicit via the checkout)
- For Phase 22: `- name: Read canonical base-image digests` / `uses: ./.github/actions/read-base-digests` / `id: digests` (id needed because downstream steps consume `${{ steps.digests.outputs.* }}`)

### SHA-pinned third-party actions (`# vX.Y.Z` trailing comment)
**Source:** every `uses:` line across `.github/workflows/*.yml` (16+ instances verified)
**Apply to:** `digest-drift-check.yml` — only third-party action is `actions/checkout`

```yaml
- uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
```

Format: `<owner>/<repo>@<full-40-char-SHA> # v<version>` with two spaces before `#`. Reuse the existing checkout SHA — already pinned project-wide.

### Comments-as-contract above `env:` / `permissions:` blocks (audit-trail style)
**Source:** `.github/workflows/docker.yml` lines 1–14 (env), lines 28–29 (permissions); `.github/workflows/release.yml` lines 1–14 (env), lines 28–29 (permissions)
**Apply to:** `digest-drift-check.yml` env + permissions blocks

Pattern to copy:
- `env:` block: each env var has a `#`-commented multi-paragraph rationale block ABOVE it (NOT inline)
- `permissions:` block (per RESEARCH.md §4 lines 352–358): a "Minimum privileges:" header, list of scopes with rationale, and a "No X. No Y." negative-space list calling out what is deliberately omitted
- Both styles serve audit-grep: an auditor reviewing the workflow can read the comment block without leaving the file

### Fail-fast error wording (`set -euo pipefail` + `|| { echo ERR; exit 1; }`)
**Source:** `.github/actions/install-bitcoind/action.yml` lines 50, 62–63, 92–93
**Apply to:** `.github/actions/read-base-digests/action.yml` parse step

Pattern:
- `set -euo pipefail` at top of every `run: |` block
- Inline `|| { echo "ERROR: ..."; exit 1; }` for guard checks
- For Phase 22 D-03, swap `ERROR:` prefix for `supply-chain:` prefix; append `Refusing to build without a valid manifest.` per RESEARCH.md §2.4 — but the structural shape (one-liner inline guard, NO error-handler function, NO trap) is identical to install-bitcoind.

### Workflow-dispatch rehearsal pattern (always paired with primary trigger)
**Source:** `.github/workflows/release.yml` lines 16–26; `.github/workflows/docker.yml` lines 16–26
**Apply to:** `digest-drift-check.yml` `on:` block

Pattern: any workflow whose primary trigger is `push: tags` OR `schedule:` ALSO declares `workflow_dispatch:` so it can be hand-fired for rehearsal. A multi-line `#` comment ABOVE the `workflow_dispatch:` line explains the rehearsal purpose and cross-references `.planning/quick/260531-ubf-*/SUMMARY.md`.

---

## No Analog Found

Files with no close in-repo analog (planner should use RESEARCH.md content directly):

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `.github/CODEOWNERS` | config | static-mapping | No prior CODEOWNERS file in repo. Format is GitHub-product-locked per https://docs.github.com/en/repositories/managing-your-repositories-settings-and-security/customizing-your-repository/about-code-owners; exact content locked verbatim in RESEARCH.md §6 lines 680–689. |
| `.github/workflows/digest-drift-check.yml` `schedule:` cron trigger | event-driven trigger | event-driven | No existing `schedule:`-cron workflow in the repo (confirmed by RESEARCH.md §2.3 line 158: `grep -r "schedule:" .github/workflows/` returns zero matches). Cron-trigger syntax is GitHub-product-locked; the full workflow scaffold borrows from `docker.yml` for env/permissions/comments style but the `schedule: - cron: '0 9 * * *'` line itself has no in-repo precedent — content locked verbatim in RESEARCH.md §4 lines 343–349. |

---

## Metadata

**Analog search scope:** `.github/actions/**`, `.github/workflows/**`, `docker/**`, repo-root `*.md`
**Files scanned:** 8 (install-bitcoind/action.yml, release.yml, docker.yml, ci.yml, Dockerfile, SECURITY.md, CONTRIBUTING.md, plus directory listings)
**Pattern extraction date:** 2026-06-01
**Analog ranking principle applied:** same role + same data flow > same role alone > most-recently-modified. CONTEXT.md D-01 named install-bitcoind as the explicit byte-for-byte structural mirror, locking the highest-impact analog assignment.

---

## PATTERN MAPPING COMPLETE

**Phase:** 22 - base-image-digest-drift-detection
**Files classified:** 9
**Analogs found:** 8 / 9

### Coverage
- Files with exact analog: 6 (`action.yml` ↔ install-bitcoind; release.yml/docker.yml/SECURITY.md/CONTRIBUTING.md modify self; Dockerfile verify self)
- Files with role-match analog: 2 (`digest-drift-check.yml` ↔ docker.yml scaffold; `digests.txt` ↔ Dockerfile-header style)
- Files with no analog: 1 (`CODEOWNERS` — reference-driven, content locked by RESEARCH.md §6)

### Key Patterns Identified
- `.github/actions/install-bitcoind/action.yml` is the byte-for-byte structural mirror for the new composite action — same `name:` + folded-scalar `description:` + "Composite source-of-truth for v1.6+:" closer, same `runs.using: composite` step shape, same inline `|| { echo ...; exit 1; }` fail-fast pattern. The new action adds `inputs: {}` + `outputs:` blocks (install-bitcoind exports via `$GITHUB_ENV` instead).
- Every `uses:` line across all 3 workflows pins third-party actions via 40-char SHA + ` # v<version>` trailing comment (two spaces before `#`); local composite actions are SHA-implicit (no `@`, no trailing comment).
- Both release.yml and docker.yml use the same step-insertion pattern: the new `Read canonical base-image digests` step lands AFTER setup steps (checkout/toolchain/cache OR checkout/login/buildx/metadata) and BEFORE the consuming step (cargo build OR docker/build-push-action). The `if: startsWith(github.ref, 'refs/tags/')` gate stays at JOB level — the inserted step inherits it.
- `permissions:` blocks across the repo are written with prose-comment-as-contract style: a "Minimum privileges:" header naming each scope's rationale, followed by `# No X. No Y.` calling out deliberately-omitted scopes. The Phase 22 `digest-drift-check.yml` `permissions:` block is locked verbatim in RESEARCH.md §4.
- `SECURITY.md §Supply-chain status` voice is direct/declarative ("blindjoin's release artifacts have known supply-chain gaps"), uses bold-lede bullets, and tracks shipped items via `~~strikethrough~~` + "Closed in v1.X" annotations. The new `### Base-image digests (v1.6 onward)` subsection follows that voice exactly.
- `CONTRIBUTING.md §Tagging releases` is the structural template for the new `## Bumping base-image digests` section: imperative-gerund header, one-sentence rule statement, `**Why:**` / `**Before X:**` bolded sub-callouts, fenced bash blocks for exact commands.

### File Created
`.planning/phases/22-base-image-digest-drift-detection/22-PATTERNS.md`

### Ready for Planning
Pattern mapping complete. Planner can now reference analog patterns in PLAN.md files — every new file has a concrete in-repo (or RESEARCH-locked) excerpt to copy, every modified file has its insertion-point step ordering pinned to specific line numbers.
