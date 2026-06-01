# Phase 22: Base-Image Digest Drift Detection — Research

**Researched:** 2026-06-01
**Domain:** GitHub Actions composite actions + scheduled workflows + supply-chain digest manifests
**Confidence:** HIGH (every claim backed by either a file in this repo, official Docker docs, or a locked CONTEXT.md decision)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01: Composite action at `.github/actions/read-base-digests/action.yml`.** The "read `docker/digests.txt` → emit named digest outputs" logic lives in a single composite action that mirrors the existing `.github/actions/install-bitcoind/` pattern. Both `release.yml` and `docker.yml` invoke `- uses: ./.github/actions/read-base-digests` and consume named outputs.
- **D-02: Named per-image outputs (`debian_ref`, `cargo_chef_ref`).** Hardcoded to the two images named in DRIFT-01 (`debian:bookworm-slim`, `lukemathwalker/cargo-chef:latest-rust-1`). Callers write `--build-arg DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}` directly — no `fromJSON()` indirection. A future third base image grows a third named output; the rename is intentional friction.
- **D-03: Fail-fast inside the composite action.** Validates: file exists, exactly the 2 expected images present, each line matches `^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$`. Any deviation → `exit 1` with an explicit auditor-facing error pointing at SECURITY.md §Supply-chain status.
- **D-04: Resolve logic stays separate from parse logic.** The composite action only PARSES `docker/digests.txt`. The drift-check workflow does its own upstream resolution and diffs against the parsed canonical list. Composition path (release/docker) is free of network calls to a registry.
- **D-05: Prose + CODEOWNERS, no extra label-gate workflow.** Policy documented in both `SECURITY.md` (operator-facing) and `CONTRIBUTING.md` (contributor-facing). Structural enforcement via `.github/CODEOWNERS` mapping `docker/digests.txt` AND `.github/actions/read-base-digests/**` to the maintainer's GitHub handle. A separate `digest-policy-check.yml` label-gate workflow is REJECTED as belt-suspenders-zipper overkill.

### Claude's Discretion (resolved in §2 below)

- **Drift-tool choice** in `digest-drift-check.yml` resolve step — `docker buildx imagetools inspect` vs `crane digest` vs `skopeo`.
- **Issue body shape** — title format LOCKED by Success Criterion #2; idempotency LOCKED by Pitfall 9. Body content, label name, and auto-assign are open.
- **Cron schedule** — DRIFT-02 says "daily"; pick a UTC time outside other scheduled work.
- **Composite-action error wording** — match the auditor-facing prose style of `docs/AUDIT-CHARTER.md`.

### Deferred Ideas (OUT OF SCOPE for Phase 22)

- **CI lint on PRs touching docker/digests.txt** — fail-fast inside the composite action + CODEOWNERS approval already covers it. Revisit if a future contributor pattern emerges.
- **Generic key/value JSON output for the composite action** — name-based outputs make rename intentional friction. Re-open only if Phase 25 actually grows base-image count.
- **`digest-policy-check.yml` label-gate workflow** — overkill for solo-maintained. Carry-forward candidate if maintainership grows.
- **Drift severity classification (Pitfall 8 retag-vs-substantive)** — REQUIREMENTS.md §Future explicitly defers to v1.7+ unless v1.6 ships clean without it.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **DRIFT-01** | `docker/digests.txt` (new file) committed as the canonical digest manifest for `debian:bookworm-slim` + `lukemathwalker/cargo-chef:latest-rust-1`. Format: one `image:tag@sha256:HEX` per line. Bumped only via human-reviewed PR (NOT auto-merged). | §3 (manifest file contents), §6 (CODEOWNERS), §7 (policy copy for SECURITY.md + CONTRIBUTING.md). |
| **DRIFT-02** | `.github/workflows/digest-drift-check.yml` scheduled workflow runs daily (and on workflow_dispatch). Reads `docker/digests.txt`, resolves the same tags fresh, opens an issue titled `[digest-drift] <image>:<tag> moved to sha256:<HEX>` on drift. Idempotent: skips issue creation if an open issue with the same digest-hex already exists. | §2 (tool choice + issue body + cron), §4 (workflow YAML skeleton), §8 (validation + rehearsal). |
| **DRIFT-03** | `release.yml` + `docker.yml` updated to read `docker/digests.txt` and pass `--build-arg DEBIAN_REF=...` + `--build-arg CARGO_CHEF_REF=...` automatically from the manifest values. Means every tagged release build is built from the canonical digest list with no manual `--build-arg` invocation required. | §3 (composite action shape), §5 (integration into release.yml + docker.yml). |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **No protocol code touched** in this phase (CI / docs / Docker only — matches the v1.5 P0-2/3 + P0-5 quick-task shape).
- **MIT, public good** — supply-chain hardening for operators, not for a vendor sales motion.
- **Tor-native + signet-first** — Phase 22 does not regress either invariant (workflow changes only).
- **`/gsd` workflow enforcement** — Phase 22 work goes through `/gsd:execute-phase 22` after planning completes; no direct repo edits outside the GSD workflow.
- **Project skills** — `.claude/skills/` and `.agents/skills/` do not exist in this repo (verified). No skill rules to honor beyond CLAUDE.md text.

---

## 1. Phase Goal Recap

From ROADMAP §Phase 22: *"An operator's release build is always built from the canonical, human-reviewed list of base-image digests, and any upstream drift surfaces as a `[digest-drift]` issue for human review within 24 hours."*

This phase delivers: (a) the canonical digest manifest, (b) the composite parser that release.yml/docker.yml use, (c) the daily drift-check workflow that opens (never auto-merges) issues, and (d) the prose + CODEOWNERS gate that makes "auto-merge a digest bump" structurally impossible.

---

## 2. Discretion Resolutions

### 2.1 Drift-tool choice — `docker buildx imagetools inspect` [VERIFIED: docs.docker.com]

**Pick:** `docker buildx imagetools inspect <image>:<tag> --format '{{.Manifest.Digest}}'`.

| Tool | Install step | Output shape | Verdict |
|------|--------------|--------------|---------|
| `docker buildx imagetools inspect` | **none** (preinstalled on ubuntu-24.04) | Single line: `sha256:HEX` (verified via official docs) | **Chosen.** Zero install. Output is directly comparable to `cut -d@ -f2` of a digests.txt line. |
| `crane digest` | One step (`go-containerregistry/cmd/crane` or `imjasonh/setup-crane@<sha>` action) | Single line: `sha256:HEX` (cleanest output, identical to imagetools for this use case) | Rejected — adds a SHA-pinned install step (Pitfall 4) for a marginal UX improvement. |
| `skopeo inspect --no-tags` | Heavier install (`apt-get install -y skopeo`), bigger blast radius | JSON; needs `jq` to extract digest | Rejected — heaviest path, no upside for our 2-image manifest. |

**Confirmation of output shape** [CITED: https://docs.docker.com/reference/cli/docker/buildx/imagetools/inspect/]: With `--format "{{.Manifest.Digest}}"`, the command returns a single line like `sha256:21a3deaa0d32a8057914f36584b5288d2e5ecc984380bc0118285c70fa8c9300`. For multi-platform images (both of ours), this is the digest of the manifest **list** itself — which is exactly what `docker pull <image>:<tag>` records under `RepoDigests`, and exactly the value that goes into `docker/digests.txt`.

**Exact resolve command** for the drift workflow (per-image):

```bash
# Inputs: $IMAGE_TAG (e.g. "debian:bookworm-slim"), $CANONICAL_DIGEST (e.g. "sha256:abc...")
UPSTREAM_DIGEST=$(docker buildx imagetools inspect "${IMAGE_TAG}" --format '{{.Manifest.Digest}}')
if [ "${UPSTREAM_DIGEST}" != "${CANONICAL_DIGEST}" ]; then
  # drift — proceed to idempotent issue-open path
  ...
fi
```

The composite action's parsed outputs (`debian_ref` = `debian:bookworm-slim@sha256:HEX`) are split inside the drift workflow into `IMAGE_TAG` + `CANONICAL_DIGEST` via `cut -d@`. No re-parse of `docker/digests.txt` — the workflow `uses:` the composite action for parsing too (D-04: the action only parses; the workflow does upstream resolve).

### 2.2 Issue body shape — concrete copy

**Title (LOCKED by Success Criterion #2):** `[digest-drift] <image>:<tag> moved to sha256:<HEX>`
**Label (planner discretion → chosen):** `digest-drift` (new — created on first issue via `gh label create digest-drift --description "Automated base-image digest drift report from digest-drift-check.yml" --color "fbca04" || true` inside the workflow).
**Auto-assign (chosen):** `--assignee ${{ github.repository_owner }}` — solo-maintained repo; the maintainer is the only person who can act.

**Body (markdown copy — interpolated by the workflow):**

```markdown
The canonical base-image digest for `${IMAGE_TAG}` recorded in
[`docker/digests.txt`](https://github.com/${{ github.repository }}/blob/main/docker/digests.txt)
no longer matches the upstream registry.

| | Digest |
|---|---|
| **Canonical (this repo)** | `${CANONICAL_DIGEST}` |
| **Upstream now** | `${UPSTREAM_DIGEST}` |
| **Registry** | https://hub.docker.com/r/${IMAGE_NAMESPACE}/tags?name=${IMAGE_TAG_PART} |

This is an automated report from
[`.github/workflows/digest-drift-check.yml`](https://github.com/${{ github.repository }}/blob/main/.github/workflows/digest-drift-check.yml)
(workflow run: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}).

### Triage (per .planning/research/PITFALLS.md §8)

Before bumping `docker/digests.txt`, classify the drift:

- **Low-severity (docs/metadata retag).** A `debian:bookworm-slim` re-tag whose
  diff is confined to `/usr/share/doc/`, `/var/lib/dpkg/`, or a CHANGELOG update
  is a routine Debian security-backport republish. Bump the manifest in a
  same-day PR.
- **Substantive (library or binary change).** A diff touching `libc6`,
  `openssl`, `ca-certificates`, the Rust toolchain version in
  `lukemathwalker/cargo-chef`, or any `usr/bin/*` is supply-chain-significant.
  Investigate the upstream release notes before bumping.

Quick diff command (run locally on a clean machine):

\`\`\`bash
docker pull ${CANONICAL_DIGEST_REF}
docker pull ${UPSTREAM_DIGEST_REF}
docker run --rm -v /tmp:/out ${CANONICAL_DIGEST_REF} sh -c 'dpkg -l > /out/old.txt 2>/dev/null || apt list --installed > /out/old.txt'
docker run --rm -v /tmp:/out ${UPSTREAM_DIGEST_REF} sh -c 'dpkg -l > /out/new.txt 2>/dev/null || apt list --installed > /out/new.txt'
diff /tmp/old.txt /tmp/new.txt
\`\`\`

### Resolving this issue

1. Decide whether to accept the new digest (per triage above).
2. **Open a PR** that updates the single line in `docker/digests.txt` for
   `${IMAGE_TAG}` to `${IMAGE_TAG}@${UPSTREAM_DIGEST}`.
   CODEOWNERS approval on the file path is required by branch protection.
   **Do not auto-merge** (see SECURITY.md §Supply-chain status and
   .planning/research/PITFALLS.md §11).
3. Close this issue from the merged PR via `Closes #<this-issue-number>`.

---

*This issue will not be re-opened by tomorrow's scheduled drift check —
idempotency keyed on the digest-hex (\`${UPSTREAM_DIGEST_HEX_BARE}\`),
per .planning/research/PITFALLS.md §9.*
```

The body length is intentional — the auditor + a future fresh-eyes maintainer should be able to act on the issue without re-reading PITFALLS.md.

### 2.3 Cron schedule — `0 9 * * *` UTC (daily at 09:00 UTC)

**Audit of existing schedules:** `grep -r "schedule:" .github/workflows/` returns zero matches (verified 2026-06-01). No collision risk; the slot is open.

**Chosen time:** `0 9 * * *` (daily at 09:00 UTC = 02:00 America/Los_Angeles, 05:00 America/New_York, 10:00 Europe/London, 18:00 Asia/Tokyo). Rationale:

- **Outside GitHub Actions runner peak load.** The Actions queue is heaviest in the US-eastern business-hours window (13:00–22:00 UTC). 09:00 UTC is in a low-contention window.
- **Outside the maintainer's typical issue-triage hours.** A drift issue opened at 09:00 UTC is visible in the maintainer's morning (US Pacific) without competing with same-morning push notifications.
- **Predictable for `gh run list --workflow=digest-drift-check.yml` audit queries.**

**Mandatory rehearsal-before-merge:** the workflow MUST be runnable via `workflow_dispatch` so the maintainer can fire it manually on the feature branch before merging, per the precedent in `.planning/quick/260531-ubf-*/SUMMARY.md` Task D. Both `on:` triggers are declared (see §4).

### 2.4 Composite-action error wording

The auditor-facing audit-charter prose style is *direct, declarative, and refers the reader to a load-bearing policy document*. Each error message has three parts: (1) what happened, (2) the supply-chain reason it's a hard failure, (3) the canonical reference document.

The exact strings (used in §3 below):

| Condition | Error string (emitted via `>&2` then `exit 1`) |
|-----------|------|
| File missing | `supply-chain: docker/digests.txt is the canonical base-image digest manifest. See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest.` |
| Wrong line count | `supply-chain: docker/digests.txt must contain exactly 2 image lines (debian + cargo-chef); found <N>. See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest.` |
| Malformed line | `supply-chain: docker/digests.txt line <LINENO> does not match the required image:tag@sha256:HEX shape: <LINE>. See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest.` |
| Expected image missing | `supply-chain: docker/digests.txt is missing the required entry for <IMAGE>. Expected one line starting "<IMAGE>:". See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest.` |

All four share the same trailing sentence — auditors grepping logs for `Refusing to build without a valid manifest` find every failure mode at one search hit. Comment-only lines (starting with `#`) and blank lines are stripped before any count or shape check.

---

## 3. Composite Action Shape

### File: `.github/actions/read-base-digests/action.yml`

```yaml
name: Read base-image digests
description: >
  Parses docker/digests.txt — the canonical base-image digest manifest — and
  emits one named output per pinned image (debian_ref, cargo_chef_ref). The
  outputs are full image references in `image:tag@sha256:HEX` form, ready to
  be passed as `--build-arg DEBIAN_REF=...` / `--build-arg CARGO_CHEF_REF=...`
  to a docker build step.

  Fail-fast contract (per Phase 22 D-03):
    1. docker/digests.txt MUST exist at repo root.
    2. After stripping comments and blank lines, the file MUST contain
       exactly 2 lines.
    3. Each line MUST match ^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$.
    4. One line MUST start with "debian:" and one with "lukemathwalker/cargo-chef:".

  Any failure exits 1 with an auditor-facing message referencing
  SECURITY.md §Supply-chain status. The supply-chain guarantee is structural
  inside this action — release.yml / docker.yml cannot accidentally publish
  a tag against an invalid or missing manifest.

  Composite source-of-truth for v1.6+: release.yml, docker.yml, and
  digest-drift-check.yml all `uses: ./.github/actions/read-base-digests` so
  the parse semantics do not drift between workflows. Mirrors the precedent
  set by .github/actions/install-bitcoind/ (v1.5 quick-task 260531-thw P0-5).

inputs: {}

outputs:
  debian_ref:
    description: Full pinned reference for debian:bookworm-slim (image:tag@sha256:HEX).
    value: ${{ steps.parse.outputs.debian_ref }}
  cargo_chef_ref:
    description: Full pinned reference for lukemathwalker/cargo-chef:latest-rust-1 (image:tag@sha256:HEX).
    value: ${{ steps.parse.outputs.cargo_chef_ref }}

runs:
  using: composite
  steps:
    - name: Parse and validate docker/digests.txt
      id: parse
      shell: bash
      run: |
        set -euo pipefail

        MANIFEST="docker/digests.txt"
        POLICY_REF="See SECURITY.md §Supply-chain status. Refusing to build without a valid manifest."

        # 1. File MUST exist.
        if [ ! -f "${MANIFEST}" ]; then
          echo "supply-chain: ${MANIFEST} is the canonical base-image digest manifest. ${POLICY_REF}" >&2
          exit 1
        fi

        # 2. Strip comments + blank lines.
        DATA="$(grep -vE '^[[:space:]]*(#|$)' "${MANIFEST}" || true)"

        # 3. Exact line count.
        LINE_COUNT="$(printf '%s\n' "${DATA}" | grep -c '^' || true)"
        if [ "${LINE_COUNT}" != "2" ]; then
          echo "supply-chain: ${MANIFEST} must contain exactly 2 image lines (debian + cargo-chef); found ${LINE_COUNT}. ${POLICY_REF}" >&2
          exit 1
        fi

        # 4. Per-line regex shape — fail-fast on any deviation.
        SHAPE_RE='^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$'
        LINENO=0
        while IFS= read -r LINE; do
          LINENO=$((LINENO + 1))
          if ! [[ "${LINE}" =~ ${SHAPE_RE} ]]; then
            echo "supply-chain: ${MANIFEST} line ${LINENO} does not match the required image:tag@sha256:HEX shape: ${LINE}. ${POLICY_REF}" >&2
            exit 1
          fi
        done <<< "${DATA}"

        # 5. Expected images present (one per named output).
        DEBIAN_REF="$(printf '%s\n' "${DATA}" | grep -E '^debian:' || true)"
        if [ -z "${DEBIAN_REF}" ]; then
          echo "supply-chain: ${MANIFEST} is missing the required entry for debian:bookworm-slim. Expected one line starting \"debian:\". ${POLICY_REF}" >&2
          exit 1
        fi

        CARGO_CHEF_REF="$(printf '%s\n' "${DATA}" | grep -E '^lukemathwalker/cargo-chef:' || true)"
        if [ -z "${CARGO_CHEF_REF}" ]; then
          echo "supply-chain: ${MANIFEST} is missing the required entry for lukemathwalker/cargo-chef:latest-rust-1. Expected one line starting \"lukemathwalker/cargo-chef:\". ${POLICY_REF}" >&2
          exit 1
        fi

        # 6. Emit named outputs.
        echo "debian_ref=${DEBIAN_REF}" >> "${GITHUB_OUTPUT}"
        echo "cargo_chef_ref=${CARGO_CHEF_REF}" >> "${GITHUB_OUTPUT}"

        # 7. Audit trail in the runner log.
        echo "✓ Parsed canonical base-image digests from ${MANIFEST}:"
        echo "    debian_ref:     ${DEBIAN_REF}"
        echo "    cargo_chef_ref: ${CARGO_CHEF_REF}"
```

### File: `docker/digests.txt` (new)

```
# Canonical base-image digest manifest for docker/Dockerfile.
# Bump only via a PR that has been reviewed by a human (CODEOWNERS-gated).
# See SECURITY.md §Supply-chain status > Base-image digests.
# Parsed by .github/actions/read-base-digests/action.yml — exact line shape
# is contract: <image>:<tag>@sha256:<HEX>, one per line, comments + blanks
# allowed.
debian:bookworm-slim@sha256:REPLACE_WITH_REAL_DIGEST_AT_PLAN_EXECUTION
lukemathwalker/cargo-chef:latest-rust-1@sha256:REPLACE_WITH_REAL_DIGEST_AT_PLAN_EXECUTION
```

**Note for planner:** The actual digest values for the two images must be resolved on the runner (or maintainer's machine) at plan-execution time via the same `docker buildx imagetools inspect ... --format '{{.Manifest.Digest}}'` command the drift workflow uses. Per the v1.5 P0-2/3 quick task, hardcoding the digests in research is brittle — the maintainer fills them in on a clean runner during plan execution. The composite action's regex contract catches any drift in shape.

### Regex enforcement summary

| Constraint | Regex / check |
|------------|---------------|
| Line shape | `^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$` |
| Comment / blank strip | `grep -vE '^[[:space:]]*(#|$)'` |
| Exact 2 data lines | `wc -l` → must equal 2 |
| Required prefixes | `^debian:` AND `^lukemathwalker/cargo-chef:` |

---

## 4. `digest-drift-check.yml` Shape

### File: `.github/workflows/digest-drift-check.yml` (new)

```yaml
name: Digest drift check

# Daily drift check of docker/digests.txt against the upstream registry digests.
# Opens an issue (NOT a PR) on drift, per .planning/research/PITFALLS.md §11:
# auto-merging digest bumps would defeat the entire supply-chain assurance this
# milestone is closing. Human review is the whole point.
#
# Idempotency (per .planning/research/PITFALLS.md §9): before opening an issue
# the workflow greps existing open `digest-drift`-labeled issues for the
# upstream digest hex. Match → skip. This prevents daily-run issue spam when
# drift persists across multiple runs before the maintainer cuts a PR.
#
# Tool choice: `docker buildx imagetools inspect --format '{{.Manifest.Digest}}'`
# is preinstalled on ubuntu-24.04, returns a single-line `sha256:HEX`, and
# matches the digest shape recorded in docker/digests.txt. No install step
# needed. `crane` and `skopeo` considered; both rejected (one adds a
# SHA-pinned action install for marginal UX gain, the other is heavyweight).
#
# Rehearsal: workflow_dispatch is wired so the workflow can be fired manually
# from any branch before the first scheduled run, per the precedent in
# .planning/quick/260531-ubf-*/SUMMARY.md Task D.

env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"

on:
  schedule:
    # 09:00 UTC daily — outside the US-eastern business-hours Actions queue
    # peak and outside the maintainer's typical synchronous review hours.
    # Verified 2026-06-01 to not collide with any other scheduled workflow
    # (`grep -r 'schedule:' .github/workflows/` returns zero matches).
    - cron: '0 9 * * *'
  workflow_dispatch:

# Minimum privileges:
#   - contents: read   — checkout + read docker/digests.txt
#   - issues:   write  — `gh issue create` / `gh issue list` on drift
# No `packages:` (we don't push anything). No `id-token:` (no cosign here).
permissions:
  contents: read
  issues: write

jobs:
  drift-check:
    name: Resolve upstream digests and diff
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      # Parse docker/digests.txt via the single source-of-truth composite
      # action. This is the same parser release.yml + docker.yml use, so
      # "shape valid for build" == "shape valid for drift check".
      - name: Read canonical digests
        id: digests
        uses: ./.github/actions/read-base-digests

      - name: Resolve upstream digests and open issue on drift
        env:
          DEBIAN_REF:     ${{ steps.digests.outputs.debian_ref }}
          CARGO_CHEF_REF: ${{ steps.digests.outputs.cargo_chef_ref }}
          GH_TOKEN:       ${{ secrets.GITHUB_TOKEN }}
        run: |
          set -euo pipefail

          # Ensure the digest-drift label exists (idempotent; first run creates it).
          gh label create digest-drift \
            --description "Automated base-image digest drift report from digest-drift-check.yml" \
            --color "fbca04" 2>/dev/null || true

          check_one() {
            local CANONICAL_REF="$1"     # e.g. "debian:bookworm-slim@sha256:abc..."
            local IMAGE_TAG="${CANONICAL_REF%@*}"               # "debian:bookworm-slim"
            local CANONICAL_DIGEST="${CANONICAL_REF#*@}"        # "sha256:abc..."
            local CANONICAL_HEX="${CANONICAL_DIGEST#sha256:}"

            echo "Checking ${IMAGE_TAG}…"
            local UPSTREAM_DIGEST
            UPSTREAM_DIGEST=$(docker buildx imagetools inspect "${IMAGE_TAG}" \
              --format '{{.Manifest.Digest}}')
            local UPSTREAM_HEX="${UPSTREAM_DIGEST#sha256:}"

            if [ "${UPSTREAM_DIGEST}" = "${CANONICAL_DIGEST}" ]; then
              echo "  ✓ no drift (digest still ${CANONICAL_DIGEST})"
              return 0
            fi

            echo "  ✗ DRIFT: canonical=${CANONICAL_DIGEST} upstream=${UPSTREAM_DIGEST}"

            # Idempotency gate (PITFALLS.md §9): search by upstream digest hex,
            # NOT by image-tag. Two different drifts of the same tag are two
            # different issues.
            local TITLE="[digest-drift] ${IMAGE_TAG} moved to ${UPSTREAM_DIGEST}"
            local EXISTING
            EXISTING=$(gh issue list \
              --label digest-drift \
              --state open \
              --search "${UPSTREAM_HEX} in:title" \
              --json number,title \
              --jq '.[] | select(.title == "'"${TITLE}"'") | .number' \
              | head -n1)

            if [ -n "${EXISTING}" ]; then
              echo "  → existing issue #${EXISTING} already tracks this drift; skipping (idempotent per PITFALLS.md §9)"
              return 0
            fi

            # Compose body. ${IMAGE_NAMESPACE} / ${IMAGE_TAG_PART} support
            # the Docker Hub registry link in the body table.
            local IMAGE_NAMESPACE IMAGE_TAG_PART
            IMAGE_NAMESPACE="${IMAGE_TAG%%:*}"
            IMAGE_TAG_PART="${IMAGE_TAG#*:}"
            # Strip any path prefix for the Docker Hub library namespace
            # (debian → library/debian on Hub URL).
            case "${IMAGE_NAMESPACE}" in
              */*) HUB_PATH="${IMAGE_NAMESPACE}" ;;
              *)   HUB_PATH="library/${IMAGE_NAMESPACE}" ;;
            esac

            BODY=$(cat <<EOF
          The canonical base-image digest for \`${IMAGE_TAG}\` recorded in
          [\`docker/digests.txt\`](https://github.com/${GITHUB_REPOSITORY}/blob/main/docker/digests.txt)
          no longer matches the upstream registry.

          | | Digest |
          |---|---|
          | **Canonical (this repo)** | \`${CANONICAL_DIGEST}\` |
          | **Upstream now** | \`${UPSTREAM_DIGEST}\` |
          | **Registry** | https://hub.docker.com/r/${HUB_PATH}/tags?name=${IMAGE_TAG_PART} |

          This is an automated report from
          [\`.github/workflows/digest-drift-check.yml\`](https://github.com/${GITHUB_REPOSITORY}/blob/main/.github/workflows/digest-drift-check.yml)
          (workflow run: ${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}).

          ### Triage (per .planning/research/PITFALLS.md §8)

          Before bumping \`docker/digests.txt\`, classify the drift:

          - **Low-severity (docs/metadata retag).** A re-tag whose diff is
            confined to \`/usr/share/doc/\`, \`/var/lib/dpkg/\`, or a CHANGELOG
            update is a routine security-backport republish. Bump the manifest
            in a same-day PR.
          - **Substantive (library or binary change).** A diff touching
            \`libc6\`, \`openssl\`, \`ca-certificates\`, the Rust toolchain
            version in \`lukemathwalker/cargo-chef\`, or any \`usr/bin/*\` is
            supply-chain-significant. Investigate the upstream release notes
            before bumping.

          Quick diff command (run locally on a clean machine):

          \`\`\`bash
          docker pull ${IMAGE_TAG}@${CANONICAL_DIGEST}
          docker pull ${IMAGE_TAG}@${UPSTREAM_DIGEST}
          docker run --rm -v /tmp:/out ${IMAGE_TAG}@${CANONICAL_DIGEST} \\
            sh -c 'dpkg -l > /out/old.txt 2>/dev/null || apt list --installed > /out/old.txt'
          docker run --rm -v /tmp:/out ${IMAGE_TAG}@${UPSTREAM_DIGEST} \\
            sh -c 'dpkg -l > /out/new.txt 2>/dev/null || apt list --installed > /out/new.txt'
          diff /tmp/old.txt /tmp/new.txt
          \`\`\`

          ### Resolving this issue

          1. Decide whether to accept the new digest (per triage above).
          2. **Open a PR** that updates the single line in \`docker/digests.txt\`
             for \`${IMAGE_TAG}\` to \`${IMAGE_TAG}@${UPSTREAM_DIGEST}\`.
             CODEOWNERS approval on the file path is required by branch
             protection. **Do not auto-merge** (see SECURITY.md §Supply-chain
             status and .planning/research/PITFALLS.md §11).
          3. Close this issue from the merged PR via \`Closes #<this-issue-number>\`.

          ---

          *This issue will not be re-opened by tomorrow's scheduled drift
          check — idempotency keyed on the digest-hex (\`${UPSTREAM_HEX}\`),
          per .planning/research/PITFALLS.md §9.*
          EOF
          )

            gh issue create \
              --title "${TITLE}" \
              --body "${BODY}" \
              --label digest-drift \
              --assignee "${GITHUB_REPOSITORY_OWNER}"

            echo "  → opened new issue: ${TITLE}"
          }

          check_one "${DEBIAN_REF}"
          check_one "${CARGO_CHEF_REF}"

          echo
          echo "Drift check complete."
```

**Idempotency call-out (load-bearing per PITFALLS.md §9):**

```bash
gh issue list \
  --label digest-drift \
  --state open \
  --search "${UPSTREAM_HEX} in:title" \
  --json number,title \
  --jq '.[] | select(.title == "'"${TITLE}"'") | .number' \
  | head -n1
```

The search is keyed on the **upstream digest hex** (`UPSTREAM_HEX`), not the image-tag — because the same `<image>:<tag>` can drift to a NEW digest while an old `[digest-drift]` issue is still open from a PREVIOUS drift. Two different drifts of the same tag are two different issues. The `--jq` post-filter is belt-and-suspenders: the search-in-title narrows the candidate set, then we require an exact title match before treating the existing issue as a duplicate.

### Permissions audit

| Scope | Why | Could be tighter? |
|-------|-----|-------------------|
| `contents: read` | `actions/checkout` + read `docker/digests.txt` | No |
| `issues: write` | `gh issue create` + `gh label create` | No |
| (everything else) | unused — implicit `none` under the existing `permissions:` block | — |

No `id-token:`, no `packages:`, no `pull-requests:` — keep the surface minimal. This workflow can never push code or images.

---

## 5. `release.yml` + `docker.yml` Integration

### 5.1 `release.yml` — `build` job modifications

**Insertion point:** after `Swatinem/rust-cache` step, before `Build coordinator and client`. The cargo build itself doesn't consume the digests in v1.6 Phase 22 (no Rust crate is digest-pinned via the manifest), but the step runs on every tag-gated `build` invocation so that:

1. A future Phase 25 step that DOES consume the digests can read `${{ steps.digests.outputs.* }}` without re-inserting the composite action call.
2. The supply-chain gate is uniform with `docker.yml` — release.yml refuses to publish a release tarball if `docker/digests.txt` is missing or malformed.
3. Audit observability is consistent — the `Read canonical digests` step appears in every tagged-release run log.

```yaml
  build:
    name: Build linux-amd64
    needs: check
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')

    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
        with:
          toolchain: stable

      - uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2

      # Phase 22 DRIFT-03: read the canonical base-image digest manifest.
      # This step is a supply-chain gate — a tag push cannot publish a
      # release tarball unless docker/digests.txt is present and well-formed
      # (the composite action exits 1 otherwise; see
      # .github/actions/read-base-digests/action.yml). Outputs are unused by
      # the cargo build directly but exported for downstream phases
      # (Phase 25 reproducibility recipe) and for log-audit observability.
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

      - name: Upload to GitHub Releases
        uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
        with:
          files: |
            blindjoin-linux-amd64.tar.gz
            blindjoin-linux-amd64.tar.gz.sha256
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

No permission changes — release.yml already has `contents: write`. The composite action requires only `contents: read` (covered).

### 5.2 `docker.yml` — `docker` matrix job modifications

**Insertion point:** after `docker/metadata-action` (which sets `${{ steps.meta.outputs.tags }}`), before `docker/build-push-action`. The `id: digests` outputs thread into `docker/build-push-action` via a new `build-args:` block. The existing `cache-from`/`cache-to` lines are unchanged.

```yaml
  docker:
    name: Docker ${{ matrix.image }}
    needs: check
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/')
    permissions:
      contents: read
      packages: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - image: coordinator
            target: coordinator
          - image: client
            target: client
          - image: liquidity-bot
            target: liquidity-bot

    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1

      - uses: docker/login-action@4907a6ddec9925e35a0a9e82d7399ccc52663121 # v4.1.0
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - uses: docker/setup-buildx-action@4d04d5d9486b7bd6fa91e7baf45bbb4f8b9deedd # v4.0.0

      - uses: docker/metadata-action@c299e40c65443455700f0fdfc63efafe5b349051 # v5.10.0
        id: meta
        with:
          images: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}

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

      - uses: docker/build-push-action@bcafcacb16a39f128d818304e6c9c0c18556b85f # v7.1.0
        with:
          context: .
          file: docker/Dockerfile
          target: ${{ matrix.target }}
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          # Phase 22 DRIFT-03: pinned base-image digests from
          # docker/digests.txt threaded as build args. The Dockerfile's
          # `ARG CARGO_CHEF_REF` / `ARG DEBIAN_REF` scaffold (v1.5 P0-2/3)
          # consumes these in the `FROM ${...}` lines.
          build-args: |
            DEBIAN_REF=${{ steps.digests.outputs.debian_ref }}
            CARGO_CHEF_REF=${{ steps.digests.outputs.cargo_chef_ref }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

**Verification of Dockerfile ARG names** [VERIFIED: `docker/Dockerfile` lines 32-33 + 35 + 48]: `ARG CARGO_CHEF_REF=lukemathwalker/cargo-chef:latest-rust-1` and `ARG DEBIAN_REF=debian:bookworm-slim` are consumed in `FROM ${CARGO_CHEF_REF} AS chef` and `FROM ${DEBIAN_REF} AS runtime-base`. The build-args names in docker.yml MUST match these exactly — `DEBIAN_REF` and `CARGO_CHEF_REF`. Threaded correctly above.

**No permission changes** — docker.yml's `docker` job already has `contents: read` + `packages: write`. The composite action requires only `contents: read` (covered).

---

## 6. CODEOWNERS Content

### File: `.github/CODEOWNERS` (new)

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

**Why both paths and not just `docker/digests.txt`** (D-05 + CONTEXT §specifics): changing the parser logic in `.github/actions/read-base-digests/action.yml` is functionally the same level of supply-chain risk as changing a digest. A PR that loosens the regex from `[a-f0-9]{64}` to `.*` would let an attacker land a manifest line with a non-digest reference. Coverage of both paths is load-bearing.

**Owner verification** [VERIFIED: `docker.yml:94` uses `${{ github.repository_owner }}`, `.planning/research/SUMMARY.md:70-77` references `johnzilla` in cosign identity templates, `SECURITY.md` email is `johnturner@gmail.com` (account `@johnzilla`)]: the GitHub handle is `@johnzilla`. The maintainer is solo.

**Branch protection prerequisite (manual step, planner-flagged):** CODEOWNERS only fires when branch protection on `main` is configured to require CODEOWNERS approval. The planner MUST include a manual UAT item: "Maintainer verifies `Settings → Branches → main → Require review from Code Owners` is checked on github.com after Phase 22 merges." This is the one piece of the gate that lives outside the repo.

---

## 7. Policy Copy

### 7.1 SECURITY.md §Supply-chain status — updated paragraph

**What changes:** the existing `### Known gaps at v1.5` bullet about base-image digest pins is rewritten in the past tense, a new `### Base-image digests (v1.6)` subsection is added, and the existing `### v1.6 supply-chain plan` bullet for digest drift is moved to a "shipped" annotation.

**Proposed copy** (drops in under `## Supply-chain status`):

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
images each release was built from.** A release tagged `vX.Y.Z` was built
against the digests recorded at the same commit. An auditor reproducing
the build SHOULD use the same digest values.

**The manifest is bumped only by human-reviewed PR.** Both
`docker/digests.txt` and the parser action are listed in
[`.github/CODEOWNERS`](.github/CODEOWNERS); branch protection on `main`
requires maintainer approval on any PR touching either path. **Do not
auto-merge digest bumps** — auto-merging is the threat model this gate
exists to close. A compromised upstream base image accepted via
auto-merge would leak into the next release.

**Drift detection.** A scheduled workflow
([`.github/workflows/digest-drift-check.yml`](.github/workflows/digest-drift-check.yml))
runs daily at 09:00 UTC, resolves each pinned tag against the upstream
registry via `docker buildx imagetools inspect`, and opens a
`[digest-drift]`-labeled issue if the upstream digest has moved. The
workflow **opens issues, not PRs**, by design. The issue body includes a
retag-vs-substantive triage hint and the exact diff command an operator
can run locally before deciding to accept the new digest.

The workflow is idempotent — re-running it while a drift issue is open
does not create a duplicate (the search is keyed on the upstream digest
hex, so two different drifts of the same tag produce two different
issues). The workflow can be fired manually via `workflow_dispatch` from
the Actions tab; this is the recommended rehearsal path before pulling
any digest bump.
```

The existing `### Known gaps at v1.5 > Base image digest pins are manual.` bullet should be edited to:

> - **~~Base image digest pins are manual.~~** **Closed in v1.6** — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).

The existing `### v1.6 supply-chain plan > Automated base-image digest drift check` bullet should be edited to:

> - **~~Automated base-image digest drift check~~** ✓ Shipped in Phase 22 — see [Base-image digests (v1.6 onward)](#base-image-digests-v16-onward).

### 7.2 CONTRIBUTING.md — new "Bumping base-image digests" section

**Insertion point:** after the existing `## Tagging releases` section.

**Proposed copy:**

```markdown
## Bumping base-image digests

`docker/Dockerfile` derives from two upstream base images:
`debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. Both
are pinned by digest in [`docker/digests.txt`](docker/digests.txt) — the
canonical manifest — and consumed by the release pipeline via the composite
action at [`.github/actions/read-base-digests/`](.github/actions/read-base-digests/).

**When you'd bump:** a `[digest-drift]` issue has been opened by the
scheduled drift-check workflow, OR you've decided to proactively refresh a
base image (e.g. picking up a Debian security backport before the daily
check runs).

**How to bump (per PR):**

1. On a clean machine (fresh Docker daemon cache or no project-local
   config), resolve the new upstream digest for ONE image:
   ```bash
   docker buildx imagetools inspect debian:bookworm-slim \
     --format '{{.Manifest.Digest}}'
   # → sha256:<HEX>
   ```
2. Update the matching line in `docker/digests.txt` to
   `<image>:<tag>@sha256:<HEX>`. **Only that one line.** A bump-both-at-once
   PR is harder to review; one image per PR.
3. Open a PR. The PR body MUST link the originating `[digest-drift]`
   issue (if any), AND classify the change as docs/metadata-only or
   substantive per the triage guidance in the issue body (see
   [`.planning/research/PITFALLS.md`](.planning/research/PITFALLS.md) §8).
4. Wait for CODEOWNERS approval. **Do not auto-merge** — see
   [SECURITY.md §Supply-chain status](SECURITY.md#supply-chain-status).
   The CODEOWNERS gate exists specifically to prevent unreviewed digest
   bumps from leaking into releases.
5. Once merged, close the originating `[digest-drift]` issue via the
   `Closes #<N>` clause in the PR.

**Why not auto-merge?** A compromised upstream base image (xz utils, 2024;
event-stream, 2018) gets pulled in if the project accepts upstream digest
changes without human review. The drift check exists to surface bumps;
the CODEOWNERS gate exists to make sure a human looks at each one.

**What if the regex check fails?** The composite action validates each
line of `docker/digests.txt` against
`^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$` before any build
step runs. A malformed line fails the gate with an explicit message
referencing this section. Fix the line shape and push again.
```

---

## 8. Validation Architecture

> Nyquist validation is OFF in `.planning/config.json` (`workflow.nyquist_validation: false`), so the requirement→test mapping below is advisory rather than load-bearing. Included anyway because the planner benefits from concrete verification commands.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | GitHub Actions (no Rust test code in this phase — purely YAML + bash) |
| Config file | `.github/workflows/digest-drift-check.yml`, `.github/actions/read-base-digests/action.yml` |
| Quick local check | `bash -n <script>` to syntax-check; `python3 -c 'import yaml; yaml.safe_load(open("<file>"))'` to YAML-validate (per the v1.5 P0 quick task) |
| Rehearsal path | `gh workflow run digest-drift-check.yml --ref <branch>` (workflow_dispatch) |

### Phase Requirements → Success Criteria → Verification

| REQ | Success Criterion | Verification |
|-----|-------------------|--------------|
| DRIFT-01 | SC#1: `docker/digests.txt` exists; non-human-reviewed PR cannot auto-merge it. | (a) `test -f docker/digests.txt && head -1 docker/digests.txt | grep -q '^#'` confirms file + comment header. (b) `cat .github/CODEOWNERS | grep -q 'docker/digests.txt.*@johnzilla'` confirms ownership. (c) On github.com: Settings → Branches → `main` → "Require review from Code Owners" is checked. (d) On github.com: open a draft PR that touches `docker/digests.txt` from a non-CODEOWNERS account → confirm merge button is blocked. |
| DRIFT-02 | SC#2: `gh workflow run digest-drift-check.yml` against a tag whose registry digest has moved opens a new `[digest-drift]` issue; running it a second time with the same drift does NOT open a duplicate. | (a) On a feature branch, edit `docker/digests.txt` to a deliberately stale digest (e.g. change the last hex char). (b) `gh workflow run digest-drift-check.yml --ref <branch>`. (c) `gh run watch` → confirm "opened new issue" log line. (d) `gh issue list --label digest-drift --state open` → confirm exactly 1 issue. (e) `gh workflow run digest-drift-check.yml --ref <branch>` again. (f) `gh issue list --label digest-drift --state open` → still exactly 1 issue (idempotency). (g) Restore the canonical digest, close the test issue. |
| DRIFT-02 | SC#4: `digest-drift-check.yml` runs on the daily `schedule` cron AND on `workflow_dispatch`; absence of an open `[digest-drift]` issue after a successful run is observable evidence "no drift today". | (a) `grep -E 'schedule:|workflow_dispatch:' .github/workflows/digest-drift-check.yml` returns both triggers. (b) After a green workflow run with no drift: `gh issue list --label digest-drift --state open` returns empty → "no drift today" is observable. (c) After 24h: `gh run list --workflow=digest-drift-check.yml --limit 2` returns the scheduled + most-recent dispatched runs. |
| DRIFT-03 | SC#3: A tagged release build succeeds without manual `--build-arg DEBIAN_REF=...`; `grep '@sha256:' docker/digests.txt` against the build logs confirms the canonical digest was used. | (a) `gh workflow run release.yml --ref <branch>` (workflow_dispatch — rehearsal, runs check only). (b) `gh workflow run docker.yml --ref <branch>` (workflow_dispatch — rehearsal, runs check only). (c) On a real tag push: confirm both workflows' `Read canonical base-image digests` step runs and prints `debian_ref: debian:bookworm-slim@sha256:...` to the log. (d) `gh run view --log <docker-run-id> | grep -E 'DEBIAN_REF=debian:bookworm-slim@sha256:[a-f0-9]{64}'` — matches the canonical digest from `docker/digests.txt`. |

### Composite-action testability — workflow_dispatch rehearsal of release.yml + docker.yml

Both `release.yml` and `docker.yml` already have `workflow_dispatch:` triggers wired (per the v1.5 260531-ubf quick task). After Phase 22 adds the composite action, the rehearsal path is identical:

1. Push the Phase 22 changes on a feature branch.
2. Actions tab → **Release** → Run workflow → pick the feature branch → Run.
3. Actions tab → **Docker** → Run workflow → pick the feature branch → Run.
4. Expected: both `check` jobs pass, both `build`/`docker` jobs are skipped (the `if: startsWith(github.ref, 'refs/tags/')` gate).
5. Critical observation: this rehearsal does NOT exercise the composite action (because `build`/`docker` are skipped). To exercise the composite action without cutting a tag, temporarily comment out the `if:` gate on a throwaway branch and dispatch again — confirm the `Read canonical base-image digests` step runs and outputs the parsed digests.

### Drift-workflow end-to-end test (deliberately stale digest)

On a throwaway feature branch:

```bash
# 1. Stale the manifest by flipping the last hex char of debian's digest.
sed -i 's/\(debian:bookworm-slim@sha256:[a-f0-9]\{63\}\)\(.\)/\1z/' docker/digests.txt
# Wait — z isn't valid hex; better:
sed -i.bak '/^debian:/s/[0-9]$/0/; /^debian:/s/[a-f]$/0/' docker/digests.txt

# 2. Push to remote.
git checkout -b test/digest-drift-e2e
git commit -am "test: deliberately stale debian digest for drift rehearsal"
git push -u origin test/digest-drift-e2e

# 3. Fire the drift workflow against the stale branch.
gh workflow run digest-drift-check.yml --ref test/digest-drift-e2e

# 4. Watch — expect: digest mismatch, new issue opened, label applied.
gh run watch

# 5. Verify idempotency by re-running.
gh workflow run digest-drift-check.yml --ref test/digest-drift-e2e
gh run watch
# Expect: "existing issue #N already tracks this drift; skipping"

# 6. Verify exactly 1 open issue.
gh issue list --label digest-drift --state open

# 7. Clean up — close test issue, restore manifest, delete branch.
gh issue close <N> --comment "Closing — drift was test rehearsal"
git checkout main
git push origin --delete test/digest-drift-e2e
```

### Fresh-machine UAT command list (per PITFALLS.md §12)

The maintainer runs these on a fresh `ubuntu-24.04` runner (or fresh Docker container) before merging the Phase 22 PR — verifies the documented operator-facing commands actually work end-to-end:

```bash
# 1. Manifest file shape (one line per image, ends with @sha256:HEX).
grep -c '@sha256:[a-f0-9]\{64\}$' docker/digests.txt
# Expect: 2

# 2. CODEOWNERS recognized by GitHub (after push).
gh api "/repos/${OWNER}/blindjoin/contents/.github/CODEOWNERS" --jq '.path'
# Expect: ".github/CODEOWNERS"

# 3. Drift workflow can be hand-fired.
gh workflow run digest-drift-check.yml --ref main
gh run list --workflow=digest-drift-check.yml --limit 1

# 4. Composite action contract — exact regex shape, on a clean machine.
#    Verifies the manifest file passes the composite action's gate.
docker run --rm -v "$PWD:/r" -w /r ubuntu:24.04 bash -c '
  apt-get update -qq && apt-get install -y -qq grep
  MANIFEST=docker/digests.txt
  DATA=$(grep -vE "^[[:space:]]*(#|$)" "$MANIFEST")
  COUNT=$(printf "%s\n" "$DATA" | grep -c "^")
  [ "$COUNT" = "2" ] || { echo "FAIL: line count $COUNT, expected 2"; exit 1; }
  printf "%s\n" "$DATA" | grep -qE "^[a-zA-Z0-9._/-]+:[a-zA-Z0-9._-]+@sha256:[a-f0-9]{64}$" \
    && echo "PASS: manifest shape valid"
'

# 5. Resolve-one-tag tool sanity (zero-install on ubuntu-24.04).
docker buildx imagetools inspect debian:bookworm-slim --format '{{.Manifest.Digest}}'
# Expect: sha256:HEX (single line)

# 6. End-to-end: drift workflow opens issue, idempotency holds.
#    (See "Drift-workflow end-to-end test" above.)
```

### Wave 0 Gaps

- [ ] `.github/CODEOWNERS` (new file) — does not exist yet.
- [ ] `docker/digests.txt` (new file) — does not exist yet; the actual digest values must be resolved at plan-execution time on a clean runner.
- [ ] `.github/actions/read-base-digests/action.yml` (new file) — does not exist yet.
- [ ] `.github/workflows/digest-drift-check.yml` (new file) — does not exist yet.
- [ ] Branch protection on `main` requiring CODEOWNERS approval — manual GitHub UI step the maintainer takes after Phase 22 merges (planner MUST include this as a HUMAN-UAT item).
- [ ] `digest-drift` GitHub label — auto-created on first workflow run via `gh label create … || true`; no Wave 0 step needed.

---

## 9. Threat-Model Notes (what bypasses the gate)

The Phase 22 gate is **defense-in-depth via prose + CODEOWNERS**. The following bypasses exist; the planner MUST be aware of them when writing the PLAN.md.

| Bypass | Mitigation in Phase 22 | Residual + carry-forward |
|--------|------------------------|--------------------------|
| **Direct push to `main` bypassing CODEOWNERS** — `git push origin main` with admin override skips required reviews. | Requires: Branch protection on `main` enabled AND "Allow administrators to bypass required reviews" UNCHECKED in the GitHub branch-protection UI. Planner MUST include a HUMAN-UAT step for the maintainer to verify this setting after Phase 22 ships. | Strictly outside this repo — lives in github.com Settings UI. Document the required setting in SECURITY.md §Supply-chain status under "Base-image digests (v1.6 onward)". |
| **Action source modified in the same PR as a digest bump.** A malicious PR edits `.github/actions/read-base-digests/action.yml` to weaken the regex AND bumps `docker/digests.txt` to a non-digest reference in the same commit. | CODEOWNERS covers BOTH `docker/digests.txt` AND `.github/actions/read-base-digests/**` (D-05 + §6 above). A PR touching either requires maintainer approval. | None for Phase 22. Phase 23 cosign signing layers protection on top — even if a malicious release ships, the cosign identity-regexp pins to a specific workflow, so an attacker would need to compromise the workflow file too. |
| **Third-party SHA-pinned action being rotated.** The actions used inside `digest-drift-check.yml` (currently only `actions/checkout`) are SHA-pinned, but the upstream owner could push a new commit to the same SHA via force-push (vanishingly unlikely on a popular action; possible on a less-vetted one). | All third-party actions in Phase 22 are SHA-pinned with `# vX.Y.Z` trailing comments per the project's existing discipline. `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` is the only third-party action — already reused across the codebase. | No new third-party actions introduced in Phase 22. The composite action is local. |
| **Manifest file race during scheduled run.** The drift workflow reads `docker/digests.txt` from `main`'s tip at the time of the scheduled cron fire; if a maintainer-approved PR lands between the digest resolution and the issue-create, the workflow may open an issue against a digest that's already been bumped. | The drift workflow's idempotency is per-digest-hex (not per-image-tag); a subsequent run with the bumped manifest will find no drift and not re-open. The stale issue is closed manually. | None — accepted residual. Daily cadence makes the race window small. |
| **DNS / registry MITM.** `docker buildx imagetools inspect` resolves to Docker Hub over TLS; a network MITM could feed a fake digest. | Standard TLS — verified by the runner's CA store. `ubuntu-24.04` is hardened against this in the GitHub-hosted runner pool. | None — accepted residual. A successful MITM that drops a fake `sha256` would cause a false-positive drift issue (the maintainer would notice the digest doesn't match what `docker pull` resolves locally on triage). |
| **Compromised maintainer account.** Maintainer's GitHub account is compromised; attacker approves their own malicious digest bump PR. | Out of scope for Phase 22 (and v1.6 overall). The project's account-security posture (2FA, hardware key, etc.) is in `SECURITY.md` §Reporting / out-of-band. | Carry-forward to v1.7+ if the project gains additional maintainers — multi-maintainer review would mitigate. v1.6 SECURITY.md already names "solo-maintained" as the operating assumption. |

**Important note on Pitfall 8 (false-positive desensitization):** the issue body's retag-vs-substantive triage hint (per §2.2) is the v1.6 mitigation. If the maintainer sees that low-severity Debian security backports produce more than ~2 issues per month, severity classification becomes a v1.7+ phase (already deferred in REQUIREMENTS.md §Future).

**Important note on Pitfall 9 (duplicate-issue spam):** the idempotency search is keyed on `${UPSTREAM_HEX} in:title` — exactly the form recommended by Pitfall 9. The search is belt-and-suspenders-confirmed by a `--jq` post-filter requiring exact title match before treating an issue as a duplicate (handles the edge case where two different drifts produce searches that happen to overlap on a hex prefix).

---

## 10. Open Questions for Planner

**None** — research is complete. Every Claude's-Discretion item in CONTEXT.md is resolved with concrete commands, copy, or YAML above. The planner can pull all four sections (composite action, drift workflow, release/docker integration, policy copy) directly into PLAN.md tasks without making further design decisions.

The one operational item the planner MUST include as a HUMAN-UAT step (not a code task) is the branch-protection toggle on github.com:

> **Settings → Branches → `main` → Require review from Code Owners** must be checked. **"Allow administrators to bypass"** must be unchecked. Verified by the maintainer immediately after Phase 22 merges; this is the structural mechanism that prevents the CODEOWNERS file from being decorative.

---

## Sources

### Primary (HIGH confidence)
- `docker/Dockerfile` — confirms `ARG CARGO_CHEF_REF` / `ARG DEBIAN_REF` scaffold from v1.5 P0-2/3 (lines 32-33, 35, 48)
- `.github/actions/install-bitcoind/action.yml` — composite-action precedent (file structure, `runs.using: composite`, named outputs, `shell: bash`)
- `.github/workflows/release.yml` — current build job structure, `workflow_dispatch` rehearsal pattern, `if: startsWith(github.ref, 'refs/tags/')` gate
- `.github/workflows/docker.yml` — current matrix-style job, `docker/build-push-action` invocation, `${{ github.repository_owner }}` usage
- `.planning/REQUIREMENTS.md` §Category 4 — DRIFT-01/02/03 verbatim text (locked WHAT)
- `.planning/ROADMAP.md` §Phase 22 — 4 numbered Success Criteria (locked acceptance)
- `.planning/research/PITFALLS.md` §§8, 9, 11, 12 — load-bearing threat-model constraints
- `.planning/phases/22-base-image-digest-drift-detection/22-CONTEXT.md` — locked D-01..D-05 + Claude's Discretion items
- [docs.docker.com/reference/cli/docker/buildx/imagetools/inspect/](https://docs.docker.com/reference/cli/docker/buildx/imagetools/inspect/) — confirms `--format '{{.Manifest.Digest}}'` output shape is single-line `sha256:HEX` for both single-platform and multi-platform images

### Secondary (MEDIUM confidence)
- `.planning/research/SUMMARY.md` — confirms `johnzilla` maintainer handle via cosign identity templates (lines 70-77)
- `.planning/quick/260531-thw-*/SUMMARY.md` P0-2/3 — confirms the Dockerfile ARG scaffold was explicitly written to be consumed by Phase 22 digest manifest work
- `.planning/quick/260531-ubf-*/SUMMARY.md` Task D — confirms the `workflow_dispatch` rehearsal pattern Phase 22 must mirror

### Tertiary (LOW confidence)
- None — all claims in this research are either confirmed by repo files or by official Docker documentation.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Both Docker Hub images (`debian:bookworm-slim`, `lukemathwalker/cargo-chef:latest-rust-1`) publish multi-platform manifests; `--format '{{.Manifest.Digest}}'` returns the manifest-list digest (matching what `docker pull` stores in `RepoDigests`). | §2.1 | If one image is single-platform, the digest returned is still well-defined (single manifest, not a list); both shapes produce a stable `sha256:HEX` line. Risk: zero. |
| A2 | `gh issue list --search "<hex> in:title"` matches against the visible title of an open issue. (GitHub Search API documented behavior.) | §4 | If `in:title` is misnamed, the fallback `--jq` post-filter still requires exact title equality before counting an issue as a duplicate — duplicate-detection still works, just on a larger candidate set. Risk: minimal (small inefficiency, not a correctness break). |
| A3 | Maintainer (`@johnzilla`) is the only CODEOWNERS reviewer required at v1.6. | §6 | If additional maintainers are added between research and execution, CODEOWNERS gains entries. Phase 22 still ships correctly with one entry. Risk: zero — additive change. |
| A4 | `ubuntu-24.04` runner ships with Docker (and therefore `docker buildx imagetools inspect`) preinstalled. (Confirmed by GitHub Actions runner image documentation as of 2026.) | §2.1, §4 | If a future runner image drops Docker, the drift workflow fails fast with `docker: command not found` — the failure is observable and the workflow can be fixed in a same-day quick task. Risk: very low. |
| A5 | The 09:00 UTC cron slot remains uncontended across v1.6. (Verified at research time: no other workflows have `schedule:` triggers.) | §2.3 | If Phase 25's `reproducible-verify.yml` chooses an overlapping slot, the daily drift check still runs (Actions tolerates concurrent scheduled workflows); the only risk is queue contention during a runner shortage. Phase 25 should pick a different slot — note for the Phase 25 plan. | 

**The 5 assumptions above are low-risk and do not require user confirmation before plan execution.** All are operational rather than design-affecting.

---

## Metadata

**Confidence breakdown:**
- Composite action shape: **HIGH** — exact YAML mirroring an in-repo precedent (`install-bitcoind`), all error strings auditor-grepable, regex constraint matches D-03 verbatim.
- Drift workflow YAML: **HIGH** — tool choice confirmed against Docker official docs, idempotency matches PITFALLS.md §9 verbatim, issue body copy concrete and ready to drop in.
- Integration into release.yml + docker.yml: **HIGH** — Dockerfile ARG names verified in repo, `build-args` syntax matches `docker/build-push-action` v7.1.0, insertion points unambiguous.
- CODEOWNERS + policy copy: **HIGH** — handle confirmed against existing repo references, policy prose mirrors `docs/AUDIT-CHARTER.md` style.
- Validation architecture: **HIGH** — fresh-machine UAT command list is runnable, test-rehearsal procedure has explicit branch-cleanup steps.
- Threat model: **HIGH** — bypasses enumerated against PITFALLS.md, residuals named with carry-forward dispositions.

**Research date:** 2026-06-01
**Valid until:** 2026-07-01 (30 days for a stable CI / docs domain; re-verify the `imagetools inspect` flag if Docker buildx publishes a major version in that window).

---

## RESEARCH COMPLETE
