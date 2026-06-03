# Phase 25: Reproducible-Build Recipe + Scheduled Verifier + Registry - Pattern Map

**Mapped:** 2026-06-02
**Files analyzed:** 8 (3 new, 5 modified)
**Analogs found:** 8 / 8

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `rust-toolchain.toml` (NEW) | config (toolchain pin) | static-config | `Cargo.toml` (workspace `[workspace]` block + comment header) | role-match (no prior `[toolchain]` config; `Cargo.toml` is the closest sibling config-at-workspace-root) |
| `docs/REPRODUCIBLE-BUILD.md` (NEW) | doc (operator-facing recipe + per-tag hashes) | reference-doc | `SECURITY.md` §Supply-chain status (L113-180 image-attest subsection + L182-230 tarball-sig subsection) | exact (operator-facing supply-chain recipe with fenced bash blocks + multi-§ structure) |
| `.github/workflows/reproducible-verify.yml` (NEW) | workflow (scheduled verifier with issue-on-mismatch) | event-driven (cron + dispatch) → side-effect (issue create) | `.github/workflows/digest-drift-check.yml` | exact (scheduled cron+dispatch + issue-not-PR + label-auto-create + title-dedup) |
| `.github/workflows/release.yml` (MOD) | workflow (build job env + Compute step + Package step + softprops cleanup) | request-response (tag push → artifact build → upload) | self (modify in-place per Phase 22/23/24 pattern); structural reference is Phase 24's `build` job at L60-222 | exact (in-place modification of established build job) |
| `.github/workflows/ci.yml` (MOD) | workflow (add `rust-toolchain-pin-check` grep gate; 8 `with: toolchain:` updates) | request-response (PR → CI gate) | `bip322-pin-check` job at L214-236 (RESEARCH override; Pitfall A) | exact (grep-gate pattern lifted verbatim) |
| `Cargo.toml` (MOD) | config (add `[profile.release]`) | static-config | self (current `[workspace]` + `[workspace.dependencies]` blocks with header comment style) | exact |
| `docs/RELEASING.md` (MOD) | doc (append + restructure post-D-13) | reference-doc | self (existing Phase 24 procedure; same prose + fenced-bash shape) | exact |
| `SECURITY.md` (MOD) | doc (insert `### Reproducibility (v1.6 onward)` subsection) | reference-doc | self (sibling `### Release tarball signatures + provenance (v1.6 onward)` at L182-230) | exact (mirror sibling subsection structure verbatim) |

## Pattern Assignments

### `.github/workflows/reproducible-verify.yml` (NEW workflow, event-driven cron → side-effect)

**Analog:** `.github/workflows/digest-drift-check.yml` (Phase 22)

**Top-of-file `name:` + prose comment block pattern** (`digest-drift-check.yml:1-22`):
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
# Tool choice: `docker buildx imagetools inspect ...`  ← (paragraph)
#
# Rehearsal: workflow_dispatch is wired so the workflow can be fired manually
# from any branch before the first scheduled run...
```
**Apply to Phase 25:** Single-line `name: Reproducible build verifier`, then multi-paragraph `#` block covering (a) what it does (re-verifies cosign + sha256 byte-equality monthly on ubuntu-24.04), (b) Pitfall 11 issue-not-PR contract (mirror), (c) Phase 22 idempotency-by-title-match inheritance, (d) tool choice (re-uses Phase 23/24 cosign-installer SHA pin), (e) rehearsal via workflow_dispatch.

**`env:` block pattern** (`digest-drift-check.yml:23-24`):
```yaml
env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
```
**Apply to Phase 25:** Identical single env var (repo-wide boilerplate; required for the Node-20 → Node-24 deprecation handling until `actions/checkout` v6 upgrade).

**`on:` block with cron-rationale comment** (`digest-drift-check.yml:26-33`):
```yaml
on:
  schedule:
    # 09:00 UTC daily — outside the US-eastern business-hours Actions queue
    # peak and outside the maintainer's typical synchronous review hours.
    # Verified 2026-06-01 to not collide with any other scheduled workflow
    # (`grep -r 'schedule:' .github/workflows/` returns zero matches).
    - cron: '0 9 * * *'
  workflow_dispatch:
```
**Apply to Phase 25:** Use `0 7 1 * *` (07:00 UTC on the 1st of each month per D-11/RESEARCH Pattern 3); cron-rationale comment cites non-collision with `digest-drift-check.yml`'s `0 9 * * *` (different hour AND monthly vs daily cadence — verified 2026-06-02).

**`permissions:` block with deliberately-omitted-scopes comment** (`digest-drift-check.yml:35-45`):
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
**Apply to Phase 25:** Same shape; comment names the same two granted scopes; deliberately-omitted block paraphrases `id-token`, `attestations`, `packages`, `PR-write`, `pages`, `deployments` (RESEARCH Pattern 4). NOTE: Phase 25's verifier DOES install cosign (re-verify only) — comment must explain that cosign-verify is a read-only operation (it does NOT need `id-token: write` because it's not minting a new sig).

**Label auto-create + title-dedup pattern** (`digest-drift-check.yml:69-72` + `:96-109`):
```bash
# Ensure the digest-drift label exists (idempotent; first run creates it).
gh label create digest-drift \
  --description "Automated base-image digest drift report from digest-drift-check.yml" \
  --color "fbca04" 2>/dev/null || true

# ... later in the script:
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
```
**Apply to Phase 25:** Verbatim same shape. Label name `reproducibility-regression`. Two titles per D-12:
- `[reproducibility-regression] runner image drift: ImageVersion <OLD> → <NEW>` (low-severity)
- `[reproducibility-regression] sha256 mismatch on ImageVersion <V>` (HIGH-severity)
Search-key is the exact title (less aggressive than digest-drift's hex-substring; the title encodes the ImageVersion which is the discriminant).

**`gh issue create` with auto-assignee pattern** (`digest-drift-check.yml:182-186`):
```bash
gh issue create \
  --title "${TITLE}" \
  --body "${BODY}" \
  --label digest-drift \
  --assignee "${GITHUB_REPOSITORY_OWNER}"
```
**Apply to Phase 25:** Same shape; substitute label + body. NOTE Phase 25's RESEARCH Example 5 differs on exit-code semantics: digest-drift returns 0 when dedup-skip fires (idempotent green); Phase 25 exits 1 ALWAYS on mismatch (RESEARCH Open Question 3 — green monthly run is the precondition for D-14 registry submission).

**SHA-pinned actions reused verbatim** (`digest-drift-check.yml:52` + `release.yml:128`):
```yaml
- uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
- uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1
  with:
    cosign-release: 'v2.6.3'
- uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9  # stable
  with:
    toolchain: "1.95.0"   # Phase 25 D-21 OVERRIDE — match rust-toolchain.toml channel
```
**Apply to Phase 25:** All four SHA pins reused verbatim. Phase 23 `sigstore-pin-check` at `ci.yml:292-326` greps every `.github/workflows/*` — covers the new verifier's `sigstore/cosign-installer` use automatically (no new gate).

---

### `.github/workflows/release.yml` (MOD — build job env + Compute step + Package step + softprops cleanup)

**Analog:** Self (modify in-place); structural reference is the existing Phase 24 `build` job at `release.yml:60-222`.

**Comments-as-contract above `permissions:` block** (`release.yml:67-81`):
```yaml
# Phase 24 SIGN-01/SIGN-02: cosign keyless signing + actions/attest-build-provenance need:
#   - contents:     write — softprops/action-gh-release uploads Release assets. Without
#                   this, softprops fails with 403 Forbidden on the Releases API call.
#                   See Phase 24 D-02.
#   - id-token:     write — OIDC token for Fulcio cert exchange. Without this,
#                   cosign sign-blob fails with the opaque "fulcio: 400 Bad
#                   Request" error. See PITFALLS Pitfall 2 + Phase 23 D-02.
#   - attestations: write — persist the SLSA provenance attestation to GitHub's
#                   attestations API. Without this, actions/attest-build-provenance
#                   fails with 403 Forbidden on the API call. See Phase 23
#                   RESEARCH §2.1 + the matching docker.yml block at lines 67-70.
# Deliberately omitted (auditor-grepable per Plan 22-04): packages, PR-write,
# pages, issues, deployments. These tokens MUST NOT appear anywhere in this file.
# release.yml does NOT push to ghcr.io — the absence of a literal `packages:` token
# at any indentation is the file-level audit gate confirming the no-ghcr-push contract.
```
**Apply to Phase 25:** New `env:` block above the existing `steps:` list (between line 86 `permissions:` closing and line 87 `steps:`); use RESEARCH Pattern 1 comment shape (verbatim per CONTEXT D-20 / RESEARCH Example pattern). The new block names RUSTFLAGS / CARGO_INCREMENTAL and cross-refs the upcoming `Compute SOURCE_DATE_EPOCH` step.

**Existing Build + Package steps to MODIFY** (`release.yml:107-117`):
```yaml
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
```
**Apply to Phase 25:**
- `Build coordinator and client` — append `--locked` flag (D-05); add prose comment above citing REPRO-02 + the fail-fast-on-stale-Cargo.lock rationale.
- Insert `Compute SOURCE_DATE_EPOCH from tagged commit time` step BETWEEN `Read canonical base-image digests` (current L103-105) and `Build coordinator and client` (current L107). Use RESEARCH Example 3 shape.
- REPLACE `Package` step body with the deterministic tar+gzip pipeline per RESEARCH Example 4 + D-06.

**Existing `runs-on:` pattern to MODIFY** (`release.yml:63`):
```yaml
build:
  name: Build linux-amd64
  needs: check
  runs-on: ubuntu-latest
```
**Apply to Phase 25:** Change to `ubuntu-24.04` per D-08. **CRITICAL forbidden-token absence audit (RESEARCH Pattern 5):** the comment block above must NOT contain the literal `ubuntu-latest` token; paraphrase as "the rolling-release runner alias" or "the unpinned runner image" when explaining the pin rationale. The `check` job at L34 stays `ubuntu-latest` (CONTEXT explicit).

**Existing softprops step to MODIFY** (`release.yml:201-222`):
```yaml
# Phase 24 D-07: Release ships as draft until the maintainer uploads the
# .asc detached PGP signature (per docs/RELEASING.md procedure) and runs
# `gh release edit vX.Y.Z --draft=false`. Operators visiting the Releases
# page never see a release missing the PGP signature — consistent
# verification UX (every published release has all 5 assets, or it's a draft).
#
# Phase 24 D-15: files: list is in semantic order — artifact → integrity →
# signature → provenance. The .asc PGP detached signature is uploaded
# post-CI by the maintainer; it is NOT in this list. Per D-06 PGP signing
# is OUT of release.yml entirely (CI does not hold the PGP private key and
# MUST NOT — that defeats SIGN-03's non-OIDC alternative path rationale).
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
**Apply to Phase 25 D-13:** Delete `draft: true` line; rename step `Upload to GitHub Releases` (drop draft mention); rewrite the comment block to cite D-13 + the 2026-06-02 SIGN-03 deferral (RESEARCH Example 6 prose). Files list + SHA pin + env block unchanged. **Forbidden-token absence audit:** post-Phase-25 `grep -q 'draft: true' .github/workflows/release.yml` must return zero matches.

---

### `.github/workflows/ci.yml` (MOD — add `rust-toolchain-pin-check` grep gate; 8 `with: toolchain:` updates)

**Analog:** `bip322-pin-check` job at `ci.yml:214-236`.

**Grep-gate pattern** (verbatim copy template — `ci.yml:214-236`):
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
**Apply to Phase 25 (new `rust-toolchain-pin-check` job, per RESEARCH Pitfall A KEEP-with-pin-match):**
- Job name `rust-toolchain-pin-check` + `runs-on: ubuntu-latest` (CI jobs stay on latest per CONTEXT explicit).
- Comment block cites Pitfall A + the dual-source-of-truth risk between `rust-toolchain.toml` and `with: toolchain:` blocks; mirrors `bip322-pin-check`'s "one job per pinned dep" pattern.
- Script: extract `channel = "X.Y.Z"` from `rust-toolchain.toml` via `grep -oP` or `awk`; then grep all `.github/workflows/*.yml` for `toolchain:\s*"[^"]+"` values and fail if any value ≠ extracted channel.
- Comments-as-contract: include reproducibility-regression cross-link.

**8 existing `with: toolchain:` blocks to UPDATE** (per RESEARCH Pitfall A KEEP-with-pin-match override):

Found locations:
- `release.yml:39` — `toolchain: stable` (check job)
- `release.yml:92` — `toolchain: stable` (build job)
- `ci.yml:34` — `toolchain: stable` (test job)
- `ci.yml:142` — `toolchain: stable` (clippy job)
- `ci.yml:163` — `toolchain: stable` (coordinator-smoke job)
- `ci.yml:177` — `toolchain: stable` (audit job)

**Pattern at each location** (from `ci.yml:32-34` exemplar):
```yaml
- uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
  with:
    toolchain: stable
```
**Apply to Phase 25:** Change `toolchain: stable` → `toolchain: "1.95.0"` (or whatever value `rust-toolchain.toml` channel reads at plan-time). Add a single-line comment above the `with:` block referencing Pitfall A + the grep-gate. (Note: CONTEXT D-21's "remove the with: input" instruction is OVERRIDDEN by RESEARCH — verified the action requires the input; see RESEARCH Pitfall A.) Total: 6 `with:` blocks across the two files (not 8 as CONTEXT prose mentions — verified at `release.yml:37-39, 90-92` and `ci.yml:32-34, 140-143, 161-163, 175-177` 2026-06-02).

---

### `rust-toolchain.toml` (NEW config file at workspace root)

**Analog:** `Cargo.toml` (sibling workspace-root config) — closest structural match for a "header-comment + TOML block at workspace root" pattern.

**Cargo.toml header-comment + block pattern** (`Cargo.toml:1-11`):
```toml
# blindjoin workspace
#
# Release versioning policy: the canonical release identifier is the git
# tag (vX.Y.Z), NOT the per-crate `version =` field. The four workspace
# crates are unpublished, so their `version =` fields stay at 0.1.0 by
# policy. See SECURITY.md § Release versioning policy for the rationale
# and the revisit condition (if `--version` CLI flags are added later).

[workspace]
members = ["coordinator", "client", "shared", "liquidity-bot"]
resolver = "2"
```
**Apply to Phase 25 (per RESEARCH Example 1 + D-15 verified value):**
```toml
# Phase 25 REPRO-01 / D-01: pin the exact Rust stable toolchain across local
# dev, CI, and the reproducible-build verifier. Single source of truth for
# the toolchain version; the channel value below is grep-asserted to match
# the `with: toolchain:` values in .github/workflows/{release,ci}.yml by the
# rust-toolchain-pin-check CI job (see ci.yml).
#
# Verified at planning time (2026-06-02): rustc 1.95.0 (59807616e 2026-04-14).
[toolchain]
channel = "1.95.0"
profile = "minimal"
components = ["rustfmt", "clippy"]
```

---

### `Cargo.toml` (MOD — add `[profile.release]` block at end)

**Analog:** Self — existing `[workspace]` + `[workspace.dependencies]` blocks at `Cargo.toml:9-37`.

**Existing structure to preserve + append** (`Cargo.toml:9-37`):
```toml
[workspace]
members = ["coordinator", "client", "shared", "liquidity-bot"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1.51", features = ["full"] }
# ... 23 more dependency lines through line 36
proptest = "1"
```
**Apply to Phase 25 D-07 (per RESEARCH Example 2):**
- INSERT new block AFTER `[workspace.dependencies]` (after line 36 `proptest = "1"`).
- Multi-line `#` prose comment ABOVE the block per Pattern 1 (cite REPRO-01 + the belt-and-suspenders rationale alongside `--remap-path-prefix`).
- Block body: `[profile.release]` then `strip = "symbols"` on the next line.

---

### `docs/REPRODUCIBLE-BUILD.md` (NEW operator-facing reproducibility doc)

**Analog:** `SECURITY.md` §Supply-chain status (L113-180 image-attest subsection + L182-230 tarball-sig subsection).

**Prose-paragraph + numbered-list + fenced-bash pattern** (`SECURITY.md:113-165`):
```markdown
### Image signatures + attestations (v1.6 onward)

Every `ghcr.io/<owner>/blindjoin-{coordinator,client,liquidity-bot}:X.Y.Z`
image push from a `vX.Y.Z` tag is:

1. **Signed by cosign** via OIDC keyless flow (no maintainer key custody). The
   signature is stored in the registry under `sha256-<HEX>.sig` and includes
   the Fulcio-issued cert bound to the GitHub Actions OIDC identity + the
   Rekor transparency-log inclusion proof.
2. **Attested with a SLSA v1.0 in-toto provenance bundle** ...

Verification requires **cosign 2.6.3 or compatible** and the **GitHub CLI
(`gh`) 2.x or later**. The verify recipes below have been tested on a clean
`ubuntu:24.04` container.

\`\`\`bash
# 1. Cosign signature verification (ATTEST-01)
cosign verify \
  --certificate-identity-regexp '...' \
  ...
\`\`\`
```
**Apply to Phase 25 (per D-09 + D-17):**
- Multi-section structure mirrors SECURITY.md §Supply-chain status — H2 sections per D-09 (1. Why this exists / 2. Recipe / 3. Toolchain pins / 4. Environment / 5. Expected sha256sum / 6. Continuous verification / 7. Reporting a reproducibility regression).
- Recipe section (§2) is a single fenced bash block per RESEARCH Example (D-17 verbatim shape) — copy-pasteable on a fresh `ubuntu-24.04` shell.
- Toolchain pins section (§3) is a markdown table: `component | version | pin file` rows for rustc, cargo, ubuntu-24.04, dtolnay-rust-toolchain SHA, sigstore/cosign-installer SHA.
- Expected sha256sum section (§5) is a markdown table with `| v1.6.0 | <TBD-v1.6.0-cut> |` (D-10 placeholder).
- §6 cross-links to `.github/workflows/reproducible-verify.yml`.
- §7 documents the `[reproducibility-regression]` issue scheme (cross-ref D-12).

**Note-blockquote pattern for cross-version pinning** (`SECURITY.md:174-180`):
```markdown
> **Note: cosign 3.0 CLI flag drift** (Pitfall 13). The recipes above have
> been tested with **cosign `>= 2.6.3, < 3.0.0`**. cosign 3.0 (released
> 2026 ...) may change CLI flags; when blindjoin's pipeline upgrades to
> cosign 3.x, the project will publish an updated recipe ...
```
**Apply to Phase 25:** Use the same `> **Note: ...**` blockquote shape in §4 Environment for the rust-toolchain bump-policy (or note that policy is deferred per CONTEXT deferred-ideas if planner prefers).

---

### `docs/RELEASING.md` (MOD — D-13 cleanup + D-10 rehearsal section + D-14 registry section)

**Analog:** Self — existing 66-line Phase 24 procedure.

**Existing prose + numbered-step + fenced-bash pattern** (`docs/RELEASING.md:18-35`):
```markdown
## Per-release procedure

CI handles the signing + attestation; the maintainer drives the tag cut, the pre-flight verify, and the draft flip.

1. **Cut and push the tag.** ... The tag MUST use 3-part semver (`vX.Y.Z`, not `vX.Y`) per [CONTRIBUTING.md `## Tagging releases`](../CONTRIBUTING.md#tagging-releases).

   \`\`\`bash
   git tag -s vX.Y.Z -m "vX.Y.Z"
   git push --tags
   \`\`\`

2. **Watch `release.yml` until green.** ...

3. **Run the [Pre-flight check](#pre-flight-check-before-flipping-out-of-draft) below.** If any verify fails, delete the draft release and re-cut the tag after the fix — do NOT flip the release.

4. **Flip the release out of draft.** ...

   \`\`\`bash
   gh release edit vX.Y.Z --draft=false
   \`\`\`
```
**Apply to Phase 25 D-13 cleanup:**
- Line 5: remove "flipping the GitHub Release out of draft" prose; replace with "verifying the CI-built artifacts and rehearsing reproducibility before tagging."
- Line 10: remove "before flipping the release out of draft" parenthetical.
- Lines 29-35: replace step 3 + step 4 (the draft-flip flow) with a single step "Verify the published release. Once CI is green and pre-flight passes, the release is already published. Operators can `gh release download` immediately." (RESEARCH Example 7 verbatim.)
- Line 37 onward: rename §"Pre-flight check before flipping out of draft" to "Pre-flight check after CI completes." Remove all `--draft=false` references (currently lines 5, 10, 29, 31, 34 per RESEARCH/CONTEXT — re-grep at plan time).

**Apply to Phase 25 D-10 (Reproducibility verification rehearsal):** APPEND new H2 section near end of file:
```markdown
## Reproducibility verification rehearsal (v1.6.0-rc.0 procedure)

Before the v1.6.0 cut, capture the expected sha256sum that goes into
[docs/REPRODUCIBLE-BUILD.md](REPRODUCIBLE-BUILD.md) §Expected sha256sum.

1. Trigger `.github/workflows/reproducible-verify.yml` via workflow_dispatch ...
2. Wait for the run to complete; check the "Capture runner ImageVersion" step output ...
3. Edit `docs/REPRODUCIBLE-BUILD.md` §Expected sha256sum table: replace `<TBD-v1.6.0-cut>` with the captured sha256.
4. Commit the change with a message citing the rc.0 cut.
5. Tag v1.6.0 and push.
```

**Apply to Phase 25 D-14 (Registry submission):** APPEND second new H2 section:
```markdown
## Reproducible-builds.org registry submission

After the verifier has run green for ≥1 monthly cycle post-v1.6.0:

1. Verify `.github/workflows/reproducible-verify.yml` has at least one green
   monthly run after the v1.6.0 tag (Actions tab; filter by workflow name).
2. Fork `github.com/reproducible-builds/reproducible-builds.org`; add an entry
   under `_data/projects/` per the project's existing YAML schema ...
3. Open a PR; respond to reviewer feedback.
4. After merge, link the registry entry from
   `docs/REPRODUCIBLE-BUILD.md` §Continuous verification AND from
   `SECURITY.md` §Supply-chain status §Reproducibility.
```

---

### `SECURITY.md` (MOD — add `### Reproducibility (v1.6 onward)` subsection)

**Analog:** Sibling `### Release tarball signatures + provenance (v1.6 onward)` at `SECURITY.md:182-230`.

**Sibling subsection pattern** (`SECURITY.md:182-225`):
```markdown
### Release tarball signatures + provenance (v1.6 onward)

Every `blindjoin-linux-amd64.tar.gz` Release archive published from a `vX.Y.Z`
tag is:

1. **Signed by cosign** via OIDC keyless flow ...
2. **Attested with a SLSA v1.0 in-toto provenance bundle** ...

Verification requires **cosign 2.6.3 or compatible** ...

\`\`\`bash
# 1. Cosign blob signature verification (SIGN-01)
cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz
# Expected: "Verified OK" + JSON cert claims.
\`\`\`

> **Note: cosign 3.0 CLI flag drift** — see the [image subsection above](#image-signatures--attestations-v16-onward) ...
```
**Apply to Phase 25 D-19 (per RESEARCH §Architecture Patterns + Example 6 specifics):**
- Light touch: 1 paragraph + 1 fenced "how to verify yourself" command block (which mirrors the Recipe block from REPRODUCIBLE-BUILD.md).
- Insertion point per RESEARCH Discretion D-19 verified: BETWEEN L231 (end of tarball-sig subsection) and L232 (start of `### Base-image digests (v1.6 onward)`), OR after L269 (right before `### v1.6 supply-chain plan`). Planner picks; either keeps the "v1.6 onward" subsections clustered.
- Cross-links to `docs/REPRODUCIBLE-BUILD.md` + `.github/workflows/reproducible-verify.yml` + placeholder for reproducible-builds.org registry entry (filled in after D-14).
- Sibling-section "Known gaps at v1.5" bullet at L106-110 needs **strikethrough update** (mirrors L104-105 pattern for the closed gaps): `- **~~No reproducible-build pipeline.~~** **Closed in v1.6 Phase 25** — see [Reproducibility (v1.6 onward)](#reproducibility-v16-onward).`
- "v1.6 supply-chain plan" subsection at L271-289: third bullet at L281-283 ("Reproducible-build instructions") also needs strikethrough update for closed status.

---

## Shared Patterns

### Comments-as-contract above structural blocks (Phase 22 Plan 22-04 paraphrasing discipline)
**Source:** Plan 22-04 lesson; `release.yml:67-81` (Phase 24 permissions block); `digest-drift-check.yml:35-45` (Phase 22 permissions block).
**Apply to:** Every YAML edit in Phase 25 — `release.yml` env block (D-20), `release.yml` Compute SOURCE_DATE_EPOCH step (D-03 prose), `release.yml` Package step (D-06 prose), `reproducible-verify.yml` permissions block (D-11), `reproducible-verify.yml` cron schedule, `reproducible-verify.yml` softprops-replacement (none in P25), `Cargo.toml` `[profile.release]` block (D-07), `rust-toolchain.toml` (D-15).
**Rule:** Any token forbidden by file-level grep must NOT appear in the file even inside comments. Examples for Phase 25: `ubuntu-latest` (must not appear in `release.yml build:` stanza or in `reproducible-verify.yml`); `draft: true` (must not appear in `release.yml` post-D-13); `--draft=false` (must not appear in `docs/RELEASING.md` post-D-13).

### SHA-pin trailing-comment style `@<40-hex> # vX.Y.Z`
**Source:** `release.yml:36, 37, 41, 88, 90, 94, 128, 182, 213`; `digest-drift-check.yml:52`; `ci.yml:31, 32, 35, 48, 139, 140, ...`.
**Apply to:** All `uses:` lines in new `reproducible-verify.yml` MUST follow. Phase 23's `sigstore-pin-check` job at `ci.yml:292-326` enforces this at file level.

### Workflow-level vs job-level permissions (job-level for narrower scope)
**Source:** Phase 22/23/24 pattern; `release.yml:28-29` (workflow-level `contents: write`) + `release.yml:82-85` (job-level `id-token: write` + `attestations: write` added explicitly per job).
**Apply to:** New `reproducible-verify.yml` — keep workflow-level `permissions:` at the documented two-scope shape (or omit and use default for read; job-level can override). RESEARCH Example 5 shows top-level `permissions: { contents: read, issues: write }` shape — single job, single permissions set, no override needed.

### Issue title format `[<category>] <subject-with-key-encoded>` + label-auto-create + title-dedup
**Source:** `digest-drift-check.yml:69-72` (label create) + `:96-109` (dedup) + `:182-186` (issue create).
**Apply to:** D-12 issue creation logic in `reproducible-verify.yml`. Label name `reproducibility-regression`; color `fbca04` (same yellow as digest-drift). Two title formats per D-12 (low-severity drift vs HIGH-severity sha256-mismatch).

### `if: startsWith(github.ref, 'refs/tags/')` tag-gate (preserved)
**Source:** `release.yml:66`.
**Apply to:** Phase 25 preserves verbatim on the `release.yml` `build` job — determinism env vars + the new Compute step only matter on real tag pushes. NOT applied to `reproducible-verify.yml` (it's cron+dispatch only — no tag-gate; runs against `gh release view --json tagName` to discover the latest tag).

### `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"` env header
**Source:** `release.yml:3-8`, `ci.yml:9-14`, `digest-drift-check.yml:23-24`.
**Apply to:** New `reproducible-verify.yml` MUST include the same env var at workflow level — required until the `actions/checkout` v4→v6 upgrade ships (tracked in `ci.yml:3-7` TODO).

### Cron-stagger pattern
**Source:** `digest-drift-check.yml:26-32` (`0 9 * * *` daily) with prose comment naming the verification-of-non-collision check.
**Apply to:** New `reproducible-verify.yml` uses `0 7 1 * *` (07:00 UTC on the 1st of each month per D-11/D-16) — different hour AND monthly vs daily cadence; verified non-colliding 2026-06-02. Cron comment cites the verification.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| — | — | — | Every Phase 25 file has a strong analog in the codebase. The `rust-toolchain.toml` is the weakest match (no prior `[toolchain]` config — but the workspace-root header-comment + TOML shape pattern from `Cargo.toml` covers it). All other files modify existing files or duplicate established workflow/doc patterns. |

## Metadata

**Analog search scope:**
- `.github/workflows/*.yml` (release.yml, ci.yml, digest-drift-check.yml, docker.yml — all read)
- `Cargo.toml` (workspace root — read)
- `docs/RELEASING.md` (full file — read)
- `SECURITY.md` §Supply-chain status (L85-289 — read)
- `.planning/phases/22-base-image-digest-drift-detection/22-04-PLAN.md` (Phase 22 Plan 22-04 — partial read for comments-as-contract discipline)

**Files scanned:** 7 source files + 1 plan file.

**Pattern extraction date:** 2026-06-02.

**Verified line ranges (live 2026-06-02):**
- `release.yml` 1-223 (full file, 223 lines)
- `ci.yml` 1-327 (full file, including `bip322-pin-check` L214-236 and `sigstore-pin-check` L292-326)
- `digest-drift-check.yml` 1-196 (full file)
- `Cargo.toml` 1-37 (full file, no `[profile.*]` blocks)
- `docs/RELEASING.md` 1-66 (full file)
- `SECURITY.md` 85-289 (§Supply-chain status; D-19 insertion candidates verified at L231 and L269)

**Confidence:** HIGH — every line range cited above verified against the live file 2026-06-02. The D-21 OVERRIDE in RESEARCH (Pitfall A) is reflected in this map's `with: toolchain: "1.95.0"` recommendation, not CONTEXT's "remove the input" wording.
