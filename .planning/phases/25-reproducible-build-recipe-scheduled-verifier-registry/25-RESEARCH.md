# Phase 25: Reproducible-Build Recipe + Scheduled Verifier + Registry - Research

**Researched:** 2026-06-02
**Domain:** Rust binary determinism + scheduled GitHub Actions verifier + supply-chain documentation
**Confidence:** HIGH on environmental facts (verified via tool); MEDIUM on first-iteration reproducibility outcome (Pitfall 6 long tail is project-specific and only the verifier itself proves it)

## Summary

Phase 25 is the final v1.6 milestone phase. The implementation surface is small (1 new workflow + 1 new doc + 1 new toolchain pin + 1 Cargo.toml block + targeted release.yml edits + a SECURITY.md subsection + a docs/RELEASING.md append) but each change is load-bearing for a different requirement. The locked WHAT comes from CONTEXT D-01..D-21 — no exploration of alternatives is needed for any decision marked there.

One CONTEXT assumption is wrong and must be flagged for the planner: **D-21 assumes that removing `with: toolchain:` from `dtolnay/rust-toolchain@<SHA>` lets `rust-toolchain.toml` take precedence. The action's parse step at the project-pinned SHA (`3c5f7ea2…`) explicitly exits 1 with `'toolchain' is a required input` when the input is empty.** The maintainer (dtolnay) deliberately rejected `rust-toolchain.toml` support; a third-party action (`dsherret/rust-toolchain-file`) exists specifically because of this. The planner has three viable answers — KEEP the `with: toolchain:` input with an explicit-version pin matching the file, SWITCH actions, or DROP the action and let rustup auto-detect from the file on `cargo` invocation. Recommendation: KEEP the input (lowest-blast-radius change). Full details in §Common Pitfalls Pitfall A.

Every other CONTEXT assumption was verified or is correct as stated.

**Primary recommendation:** Implement D-01..D-20 verbatim. For D-21, override the CONTEXT assumption — keep `with: toolchain: "1.95.0"` in the 6 dtolnay/rust-toolchain `with:` blocks and add a single-source-of-truth comment that the value must match `rust-toolchain.toml`. Add a CI grep gate (mirrors `bip322-pin-check`) that fails if the version in any `with: toolchain:` block ≠ the channel in `rust-toolchain.toml` — single source of truth enforced at file level rather than at action level.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Binary determinism stack (release.yml `build` job)**

- **D-01:** Add `rust-toolchain.toml` at workspace root pinning the exact stable toolchain. Conventional Rust pattern; `dtolnay/rust-toolchain` respects `rust-toolchain.toml` when present. *[Research note: dtolnay/rust-toolchain does NOT respect it — see Pitfall A. The `rust-toolchain.toml` file itself is still the right answer for `cargo` invocations and for local-dev / verifier rebuilds; only the CI action precedence assumption changes.]*  The file shape: `[toolchain] channel = "1.95.0" profile = "minimal" components = ["rustfmt", "clippy"]`. Planner pins to whatever `rustc --version` reports at planning time (currently `1.95.0`, verified 2026-06-02).
- **D-02:** RUSTFLAGS = two `--remap-path-prefix` flags. Set in the `build` job's `env:` block.
- **D-03:** SOURCE_DATE_EPOCH derived from the tagged commit time: `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)`. Computed in a step BEFORE the `Build` step and exported to `$GITHUB_ENV`.
- **D-04:** `CARGO_INCREMENTAL=0` at job `env:` level.
- **D-05:** `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot`. Explicit `--locked` for fail-fast on stale `Cargo.lock`.
- **D-06:** Deterministic `tar`+`gzip` pipeline: `tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH}" -cf - -C dist . | gzip -n > blindjoin-linux-amd64.tar.gz`.
- **D-07:** `[profile.release] strip = "symbols"` in workspace `Cargo.toml`.
- **D-08:** `release.yml` `build` job `runs-on: ubuntu-24.04` (NOT `ubuntu-latest`).

**Documentation skeleton (docs/REPRODUCIBLE-BUILD.md)**

- **D-09:** `docs/REPRODUCIBLE-BUILD.md` H2 section list (Why this exists / Recipe / Toolchain pins / Environment / Expected sha256sum / Continuous verification / Reporting a reproducibility regression).
- **D-10:** Expected sha256 placeholder is `<TBD-v1.6.0-cut>`; two-stage bootstrap with v1.6.0-rc.0 rehearsal capturing the real hash.

**Scheduled verifier (.github/workflows/reproducible-verify.yml)**

- **D-11:** New file, scheduled monthly + `workflow_dispatch`, `runs-on: ubuntu-24.04`, 7 verification steps including ImageVersion capture, cosign re-verify, source rebuild via REPRO-01 recipe, sha256 compare.
- **D-12:** Two-title `[reproducibility-regression]` issue scheme with title-dedup (mirrors Phase 22 `[digest-drift]`).

**release.yml orphan-draft cleanup**

- **D-13:** Remove `draft: true` from `release.yml` softprops step + rewrite the surrounding comment block. Folded into the same `release.yml` PLAN that lands D-01..D-08.

**REPRO-04 registry submission procedure**

- **D-14:** Manual maintainer procedure in `docs/RELEASING.md` (≥1 green monthly cycle prerequisite, fork registry repo, add entry, open PR, link back from REPRODUCIBLE-BUILD.md + SECURITY.md after merge).

### Claude's Discretion

- **D-15:** Exact `rust-toolchain.toml` channel value. *[Verified at planning time: `1.95.0` per `rustc --version` on maintainer machine.]*
- **D-16:** Cron schedule for `reproducible-verify.yml`. *[Phase 22 uses `0 9 * * *` daily; planner picks a non-colliding monthly slot. See §Architecture Patterns Pattern 3 for verified-non-colliding recommendation.]*
- **D-17:** `docs/REPRODUCIBLE-BUILD.md` exact prose + Recipe section copy-paste shape.
- **D-18:** Verifier's expected-sha256 lookup mechanism (markdown table parse vs separate `.expected-sha256` file).
- **D-19:** SECURITY.md §Supply-chain status cross-link. *[Verified at planning time: section lives at lines 95-289; existing subsections are "Known gaps at v1.5" (L102), "Image signatures + attestations (v1.6 onward)" (L113), "Release tarball signatures + provenance (v1.6 onward)" (L182), "Base-image digests (v1.6 onward)" (L232), "v1.6 supply-chain plan" (L271). D-19's `### Reproducibility (v1.6 onward)` slots in BETWEEN L231 and L232 (before "Base-image digests") to keep the "what's new in v1.6" subsections clustered, OR after L269 (right before "v1.6 supply-chain plan"). Planner picks; either is fine.]*
- **D-20:** Comment-block style for new `env:` block in `release.yml` build job.
- **D-21:** `dtolnay/rust-toolchain` `with:` after `rust-toolchain.toml` lands. *[OVERRIDE: research found the CONTEXT assumption wrong — see Pitfall A. The planner's discretion area expands: pick KEEP-with-pin-match OR SWITCH-action OR DROP-action. KEEP-with-pin-match is the lowest-blast-radius answer.]*

### Deferred Ideas (OUT OF SCOPE)

- Per-architecture reproducibility (linux-arm64, darwin-amd64, etc) — v1.7+
- `diffoscope` integration on verifier mismatch — deferred to first real divergence
- Bump-policy prose for `rust-toolchain.toml` — deferred to first toolchain bump moment
- Reproducibility for GHCR images themselves — v1.7+
- `workflow_run` trigger on `release.yml` success (immediate-trigger verifier) — v1.7+
- `[reproducibility-success]` issue on green monthly runs — rejected as noise
- Severity subdivision on HIGH `[reproducibility-regression]` — defer until false-positive workload demands
- `reproducible-builds.org` SBOM-comparison submission — v1.7+

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REPRO-01 | `docs/REPRODUCIBLE-BUILD.md` published, naming: exact Rust toolchain version (matches `ci.yml` pin), `ubuntu-24.04` runner image version, exact `cargo build` invocation, exact env vars (`SOURCE_DATE_EPOCH`, `RUSTFLAGS=--remap-path-prefix=...`, `CARGO_INCREMENTAL=0`), and the expected `sha256sum` of the resulting tarball. | D-01 (toolchain pin), D-08 (runner image pin), D-02/D-03/D-04 (env vars), D-05 (cargo invocation), D-09 (doc structure), D-10 (expected sha256), D-17 (Recipe section) |
| REPRO-02 | `.github/workflows/release.yml` build job updated for binary determinism: `cargo build --release --locked` (explicit `--locked`), `SOURCE_DATE_EPOCH` derived from tag's commit time, `RUSTFLAGS` set per REPRO-01, `CARGO_INCREMENTAL=0` in env. | D-02 (RUSTFLAGS), D-03 (SOURCE_DATE_EPOCH derivation), D-04 (CARGO_INCREMENTAL=0), D-05 (explicit `--locked`), D-06 (deterministic tar/gzip — needed for byte-equality), D-07 (strip="symbols"), D-08 (runner pin) |
| REPRO-03 | `.github/workflows/reproducible-verify.yml` (new) scheduled monthly: pulls the latest release tarball, rebuilds via the REPRO-01 recipe on a fresh `ubuntu-24.04` runner, asserts `sha256sum` equality. On mismatch: opens a `[reproducibility-regression]` issue + tags the next release. | D-11 (workflow structure), D-12 (two-title issue scheme + dedup), D-16 (cron stagger), D-18 (expected-sha256 lookup) |
| REPRO-04 | blindjoin submitted to the reproducible-builds.org project registry once REPRO-01 + REPRO-03 have been green for ≥1 monthly cycle. Public registration entry links to `docs/REPRODUCIBLE-BUILD.md`. | D-14 (manual maintainer procedure in docs/RELEASING.md), D-19 (SECURITY.md cross-link) |

</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Toolchain pin (single source of truth) | Source repo (`rust-toolchain.toml`) | CI workflow (`with: toolchain:` keeps in sync via grep gate) | rustup auto-respects `rust-toolchain.toml` for any `cargo` invocation — including local dev, the verifier, and the recipe. CI action's `with: toolchain:` is a separate concern (Pitfall A). |
| Binary determinism env vars | CI workflow (`release.yml` build job `env:` block) | — | Only the build job needs these; CI `check` job does not produce the artifact. |
| SOURCE_DATE_EPOCH derivation | CI workflow (Compute step) | Local rebuilder (same `git log -1` command in Recipe) | Derived from `git log -1 --format=%ct $GITHUB_SHA` so an external rebuilder gets the same value from a fresh clone. |
| Release artifact build | CI workflow (`release.yml` build job) | — | Single canonical builder. |
| Continuous verification | CI workflow (`reproducible-verify.yml`, scheduled) | — | The verifier IS the perpetual fresh-machine UAT (Pitfall 12). |
| Reproducibility regression triage | GitHub Issues (`[reproducibility-regression]` labeled) | Maintainer human review (per Pitfall 11 no-auto-merge policy) | Two-title scheme distinguishes runner-image drift from real divergence. |
| Registry submission | Maintainer manual procedure (docs/RELEASING.md) | reproducible-builds.org PR review | Third-party human-reviewed PR; not automatable. |
| Operator verify recipe | docs/REPRODUCIBLE-BUILD.md (Recipe section) | SECURITY.md (cross-link in §Reproducibility subsection) | Single bash block runnable end-to-end on fresh ubuntu-24.04 shell. |

## Standard Stack

### Core (already in project, no new deps)

| Library / Tool | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rustc / cargo | 1.95.0 [VERIFIED: `rustc --version` on maintainer machine 2026-06-02] | Pinned via `rust-toolchain.toml` (NEW FILE) | Project's existing stable channel. `rust-toolchain.toml` is the conventional Rust pattern for pinning toolchain in source. |
| GNU `tar` | bundled with ubuntu-24.04 | Deterministic archiving (`--sort`, `--mtime`, `--owner=0`, `--group=0`, `--numeric-owner`) [VERIFIED: reproducible-builds.org archive guidance] | Standard answer for reproducible-builds.org-grade tarballs. All five flags load-bearing. |
| GNU `gzip` | bundled with ubuntu-24.04 | `-n` flag strips ORIGINAL_NAME + MTIME from gzip header [VERIFIED: Debian wiki + reproducible-builds.org docs; web search 2026-06-02] | Without `-n`, gzip embeds the input filename AND its mtime in the header. Most common "tar bytes match, gzip bytes don't" failure mode. |
| `sigstore/cosign-installer` | `@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` [VERIFIED: release.yml:128 — Phase 23/24 inheritance] | Re-verify cosign sig on downloaded tarball (verifier step 4) | Phase 23/24 pin verbatim; sigstore-pin-check gate at ci.yml:292-326 catches the new use automatically. |
| `cosign` CLI | `2.6.3` [VERIFIED: release.yml:130 + SECURITY.md `>= 2.6.3, < 3.0.0` range] | Re-verify cosign sig on downloaded tarball | Same version range as operator-facing recipes in SECURITY.md. |
| `gh` CLI | (preinstalled on ubuntu-24.04) | `gh release view`, `gh release download`, `gh issue list/create` in verifier | Standard GitHub CLI; preinstalled. |
| `actions/checkout` | `@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` [VERIFIED: existing release.yml/ci.yml usage] | Checkout source for rebuild step | Verbatim reuse of project SHA pin. |
| `dtolnay/rust-toolchain` | `@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable` [VERIFIED: existing release.yml:37+90, ci.yml:32+140+161+175] | Install Rust toolchain on verifier runner. **DOES NOT respect rust-toolchain.toml** — see Pitfall A. | Project's existing pinned action; reusing the same SHA in `reproducible-verify.yml` is the right pattern. |
| `Swatinem/rust-cache` | `@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2` [VERIFIED: existing usage] | Optional in verifier (planner discretion): cache target/ dir between monthly runs. Mild speedup; not load-bearing. | Project's existing pinned cache action. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none new — all pins reused from Phase 22/23/24) | — | — | — |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `dtolnay/rust-toolchain` (CONTEXT-pinned action) | `dsherret/rust-toolchain-file` or `mkroening/rust-toolchain-toml` [VERIFIED: GitHub search 2026-06-02] | Both purpose-built to read `rust-toolchain.toml` directly. Tradeoffs: (1) introduces a NEW unpinned SHA across 6 `with:` blocks — supply-chain churn; (2) Phase 22/23/24's sigstore-pin-check gate doesn't cover them — would need a new pin policy entry; (3) the project has SHA-pinned `dtolnay/rust-toolchain` since v1.x — breaking the pattern is meaningful. **Rejected** in favor of KEEP-with-pin-match approach. |
| `gzip -n` | `gzip -9 -n` (max compression + no-name) | gzip default compression level is already deterministic given identical input bytes; `-9` only changes ratio, not determinism. Adding `-9` is harmless but unnecessary noise in the literal-byte recipe. **Use plain `gzip -n`.** |
| Markdown-table parse for expected-sha256 (D-18) | Separate `.expected-sha256` file with `<tag>:<sha>` line format | Table parse via `awk` works but is brittle (markdown column-width changes break it). Separate file is more robust and pairs naturally with the v1.6.0-rc.0 rehearsal procedure (one line per release). **Planner recommendation: separate file (e.g., `docs/REPRODUCIBLE-BUILD.expected-sha256.txt`)** — but markdown-table is also fine per D-18 explicit discretion. |
| `tokei` / `wc -l` for line counts in plans | (just count) | N/A — comment for completeness; no tool needed. |

**Installation:** No new packages installed. All tools are either preinstalled on `ubuntu-24.04` or already SHA-pinned in the project's existing workflows.

**Version verification:** `rustc --version` confirmed `1.95.0 (59807616e 2026-04-14)` on maintainer machine 2026-06-02. The CONTEXT-suggested value matches; pin to `1.95.0` in `rust-toolchain.toml` and in the 6 `with: toolchain:` blocks. Cargo `--locked` semantics verified via [Cargo reference docs](https://doc.rust-lang.org/cargo/commands/cargo-build.html) — exits 101 on stale `Cargo.lock`, exit 0 otherwise; fail-fast guarantee holds.

## Package Legitimacy Audit

> **Skipped:** Phase 25 installs zero new packages. All third-party actions (`sigstore/cosign-installer`, `actions/checkout`, `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, `softprops/action-gh-release`) are already SHA-pinned in the project from Phases 22/23/24. The new `reproducible-verify.yml` workflow reuses existing pins verbatim — no new SHAs introduced. The new `rust-toolchain.toml` references the upstream Rust release (`1.95.0`), which is not a third-party package install.
>
> If the planner chooses the alternative D-21 path of SWITCHING actions to `dsherret/rust-toolchain-file` or `mkroening/rust-toolchain-toml`, slopcheck + npm/git-tag verification of those actions becomes mandatory. KEEP-with-pin-match avoids this.

## Architecture Patterns

### System Architecture Diagram

```
Tag push (vX.Y.Z)
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│ release.yml :: build job (runs-on: ubuntu-24.04)            │
│                                                              │
│  env: RUSTFLAGS=--remap-path-prefix=... + CARGO_INCREMENTAL=0│
│   │                                                          │
│   ├─► Compute SOURCE_DATE_EPOCH from git log -1 --format=%ct │
│   │                                                          │
│   ├─► cargo build --release --locked --bin coordinator …    │
│   │                                                          │
│   ├─► tar --sort=name --owner=0 --group=0 --numeric-owner \  │
│   │       --mtime="@${SOURCE_DATE_EPOCH}" \                  │
│   │     | gzip -n > blindjoin-linux-amd64.tar.gz             │
│   │                                                          │
│   ├─► cosign sign-blob --yes --bundle .bundle                │
│   ├─► actions/attest-build-provenance → .sigstore            │
│   │                                                          │
│   └─► softprops/action-gh-release (NO draft: true) ──► GH   │
│                                                       Release│
└──────────────────────────────────────────────────────────────┘
                                                          │
                                                          ▼
┌──────────────────────────────────────────────────────────────┐
│ docs/REPRODUCIBLE-BUILD.md (NEW)                            │
│   │                                                          │
│   ├─ §Recipe: bash block (external rebuilder copy-pastes)    │
│   ├─ §Expected sha256sum: v1.6.0: <TBD-v1.6.0-cut>           │
│   └─ §Continuous verification → reproducible-verify.yml      │
└──────────────────────────────────────────────────────────────┘
                                                          │
                                                          ▼
┌──────────────────────────────────────────────────────────────┐
│ reproducible-verify.yml (NEW, monthly cron + dispatch)      │
│   on: schedule: cron: '<NON-COLLIDING-WITH-09:00>'           │
│   runs-on: ubuntu-24.04                                      │
│   permissions: contents:read + issues:write                  │
│                                                              │
│   1. Capture ${ImageVersion} → $GITHUB_ENV                   │
│   2. LATEST_TAG=$(gh release view --json tagName --jq …)     │
│   3. gh release download "$LATEST_TAG" --pattern '*.tar.gz*' │
│   4. cosign verify-blob --bundle … blindjoin-linux-amd64.tar.gz │
│   5. checkout @ $LATEST_TAG; rebuild per Recipe              │
│   6. EXPECTED=sha256sum (downloaded); ACTUAL=sha256sum (rebuilt) │
│   7. if EXPECTED == ACTUAL → green                           │
│      elif ImageVersion != pinned → [reproducibility-regression] runner image drift │
│      else → [reproducibility-regression] sha256 mismatch on ImageVersion <V>  HIGH │
│                                                              │
│   Idempotency: gh issue list --search "in:title \"<TITLE>\" is:open" │
│                  → skip create if match (Phase 22 pattern)   │
└──────────────────────────────────────────────────────────────┘
                                                          │
                                                          ▼
                                          maintainer review →
                                          if real divergence: investigate (Pitfall 11)
                                          if runner drift: re-verify on new image, update doc
```

### Recommended Project Structure (new + modified files)

```
.
├── rust-toolchain.toml                              # NEW (D-01/D-15)
├── Cargo.toml                                       # MODIFIED (D-07: add [profile.release])
├── .github/
│   └── workflows/
│       ├── release.yml                              # MODIFIED (D-01..D-08 + D-13)
│       ├── reproducible-verify.yml                  # NEW (D-11/D-12/D-16)
│       └── ci.yml                                   # OPTIONALLY MODIFIED (Pitfall A grep gate; see §Validation)
├── docs/
│   ├── REPRODUCIBLE-BUILD.md                        # NEW (D-09/D-10/D-17)
│   ├── REPRODUCIBLE-BUILD.expected-sha256.txt       # NEW (D-18; alternative: inline table — planner picks)
│   └── RELEASING.md                                 # MODIFIED (D-14 + D-10 rehearsal + D-13 cleanup of --draft=false references)
└── SECURITY.md                                      # MODIFIED (D-19: ### Reproducibility (v1.6 onward) subsection)
```

### Pattern 1: env-block-above-step + comments-as-contract (inherited from Phase 22/23/24)

**What:** Every modification to a workflow file gets an auditor-grepable comment block above the structural element being changed. The comment paraphrases forbidden tokens so file-level greps stay green.

**When to use:** Every YAML edit in this phase — the `env:` block in `release.yml` (D-20), the `Compute SOURCE_DATE_EPOCH` step (D-03), the deterministic tar/gzip step (D-06), the `permissions:` block in `reproducible-verify.yml` (D-11), the deliberately-omitted-scopes paraphrasing in the new workflow.

**Example (RECOMMENDED — verbatim per CONTEXT specifics):**
```yaml
# Phase 25 REPRO-01/02: binary determinism env vars. The three flags below
# make `cargo build --release --locked` produce byte-equal output across
# rebuilds on the pinned ubuntu-24.04 runner image.
#
# RUSTFLAGS:           Two --remap-path-prefix flags strip embedded build-host
#                      paths from debug info and panic messages.
# SOURCE_DATE_EPOCH:   Computed per REPRO-02 in the Compute step below;
#                      env entry here is a no-op marker for auditor visibility.
# CARGO_INCREMENTAL:   0 disables incremental compilation entirely.
#
# Pitfall 6 (research): expect 1-2 iteration cycles after the first ship to
# surface project-specific nondeterminism.
env:
  RUSTFLAGS: "--remap-path-prefix=${{ github.workspace }}=/build --remap-path-prefix=/home/runner/.cargo=/cargo"
  CARGO_INCREMENTAL: "0"
```

**Forbidden-token paraphrase rule:** If `ci.yml`'s `sigstore-pin-check` greps for any pattern in `.github/workflows/*`, comments in `reproducible-verify.yml` MUST NOT contain literal forbidden tokens. Example: when documenting "the verifier does NOT push to the OCI registry," do NOT write `push-to-registry: false` even in a comment — say "the OCI-registry-push input is not set."

### Pattern 2: issue-title-format `[<category>] <subject-with-key-encoded>` + title-dedup

**What:** Phase 22's `[digest-drift] <image>:<tag> moved to sha256:<HEX>` format is the template. Phase 25 mirrors:
- `[reproducibility-regression] runner image drift: ImageVersion <OLD> → <NEW>` (low-severity)
- `[reproducibility-regression] sha256 mismatch on ImageVersion <V>` (HIGH-severity)

**Dedup:** `gh issue list --label reproducibility-regression --state open --search 'in:title "<exact-title>" is:open' --json number,title --jq '.[] | select(.title == "<TITLE>") | .number' | head -n1`. If non-empty, skip `gh issue create`. Exact verbatim of Phase 22 Plan 22-04's idempotency block — see `digest-drift-check.yml:96-109` for the literal pattern to copy.

**Label auto-create:** `gh label create reproducibility-regression --description "Automated reproducibility regression report from reproducible-verify.yml" --color "fbca04" 2>/dev/null || true` — same pattern as `digest-drift-check.yml:69-72`.

**When to use:** D-12 issue creation logic in `reproducible-verify.yml`.

### Pattern 3: cron-stagger (D-16)

**What:** Phase 22's `digest-drift-check.yml` cron is `'0 9 * * *'` (09:00 UTC daily). Phase 25 verifier runs MONTHLY (per D-11/REPRO-03), so collision risk is low — but staggering by time-of-day still helps separate audit-trail signals.

**RECOMMENDED:** `cron: '0 7 1 * *'` (07:00 UTC on the 1st of each month) per CONTEXT D-11 — verified non-colliding because (a) only Phase 22's `0 9 * * *` exists as a recurring schedule (other workflows are push/PR-triggered); (b) `7 != 9` so they don't share a daily slot even on the 1st of a month; (c) `01` is the 1st of the month, distinguishable from Phase 22's `*` (every day). Planner: accept D-11's suggestion as-is.

### Pattern 4: deliberately-omitted-scopes (Phase 22/23/24 inheritance)

**What:** The `permissions:` block lists ONLY scopes the workflow uses. A comment above the block enumerates scopes deliberately omitted with paraphrased token names (Plan 22-04 lesson).

**For `reproducible-verify.yml`:**
```yaml
# Phase 25 REPRO-03: verifier needs read-only access to the source +
# write access to open [reproducibility-regression] issues. NOTHING else.
# Deliberately omitted (auditor-grepable): id-token (verifier neither signs
# nor pushes attestations — it only re-verifies the cosign sig on the
# downloaded tarball, which is a read-only operation), attestations,
# packages, PR-write, pages, deployments. These tokens MUST NOT appear
# anywhere in this file at any indentation.
permissions:
  contents: read
  issues: write
```

Note: `pull-requests:` is paraphrased to `PR-write` per Plan 22-04 + Plan 24-01 lesson (verify `! grep -q 'pull-requests:'` is a likely audit gate). Similarly avoid literal `packages:`, `id-token:`, `attestations:` with colon. Test the literal-byte forms locally before commit — verify gates in Phase 22/23/24 are unforgiving.

### Pattern 5: forbidden-token absence audit (file-level grep)

**What:** From CONTEXT canonical_refs + Phase 22-04 Plan: any token forbidden by file-level grep (e.g., `ubuntu-latest`) must NOT appear in the file even inside comments.

**For Phase 25 — `release.yml` `build` job + `reproducible-verify.yml`:**
- `! grep -q 'ubuntu-latest'` for the `build` job stanza (D-08 lock — the comment explaining why `ubuntu-24.04` is pinned MUST NOT contain `ubuntu-latest` literal).
- `! grep -q 'draft: true'` for `release.yml` post-D-13.
- `! grep -q '--draft=false'` for `docs/RELEASING.md` post-D-13 cleanup (verified at planning time: 4 occurrences at lines 5, 10, 29, 31, 34 — all referenced in the "Pre-flight check before flipping out of draft" workflow that ceases to exist after D-13).

### Anti-Patterns to Avoid

- **Comment-block contains forbidden tokens (e.g., `ubuntu-latest` literal when explaining the `ubuntu-24.04` pin).** Use paraphrase: "rolling-release runner image" or "the unpinned runner alias" — anything that conveys meaning without the literal token. Anti-pattern auto-fixed in Plan 22-04 (Rule 1).
- **Assuming `dtolnay/rust-toolchain` reads `rust-toolchain.toml`.** It does NOT. See Pitfall A.
- **Tar with default flags.** `tar czf` embeds the runner's user identity (owner/group from the filesystem), file order from the OS filesystem walk (varies), and the file mtimes (varies). All five D-06 flags are load-bearing — none can be dropped.
- **Gzip without `-n`.** Gzip embeds the source filename AND mtime in the header by default. The MOST COMMON "tar bytes match, gzip bytes don't" failure mode in repro-build reports. Without `-n`, the recipe will fail byte-equality on the first verifier run.
- **Compare sha256 of the rebuilt tarball against a hash in git that's wrong because no rehearsal happened.** D-10 placeholder `<TBD-v1.6.0-cut>` is the bootstrap solution; the v1.6.0-rc.0 rehearsal is the moment the real hash gets committed.
- **Verifier opens an issue on EVERY run regardless of green status.** A green run is the absence of news; do NOT create `[reproducibility-success]` issues (CONTEXT deferred-ideas-rejected list).
- **Comment-on-existing-issue dedup.** Title-exact-match-skip is the Phase 22 pattern; comment-on-existing adds noise on chronic regressions.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Reading `rust-toolchain.toml` from a GH action | Custom shell parse + `rustup install` step | `rust-toolchain.toml` + rustup auto-detect on first `cargo` invocation (no action change needed for the file's effect on local dev / verifier rebuild) | rustup natively respects `rust-toolchain.toml` for any `cargo`/`rustc` invocation within the workspace. The CI action is a separate concern (Pitfall A). |
| Deterministic tar/gzip | Shell-glob + custom find + manual stat resets | GNU `tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$EPOCH"` piped to `gzip -n` | Five flags do exactly what's needed. Hand-rolling reproduces every known failure mode of `tar czf`. |
| SOURCE_DATE_EPOCH derivation | Heuristic from `date` or env-var injection | `git log -1 --format=%ct $GITHUB_SHA` — locked by REPRO-02 + reproducible by external rebuilder running `git log -1 --format=%ct <TAG>` | The git-derived value is reproducible from source; any other derivation breaks the external-rebuilder contract. |
| Issue dedup | Database/cache state | `gh issue list --search 'in:title "<TITLE>" is:open' --jq '...exact-match...'` | Phase 22 idempotency pattern works. No new state needed. |
| Expected-sha256 lookup | Build-time constant baked into the verifier YAML | Read from `docs/REPRODUCIBLE-BUILD.md` table OR a separate `.expected-sha256` file at runtime | Build-time constant requires the verifier to be edited on every release; runtime read is the maintainable answer. |
| Diff explanation on byte mismatch | Inline diffoscope install in verifier | Defer to maintainer's local one-shot `diffoscope` run (CONTEXT deferred — see §Deferred Ideas) | `diffoscope` is ~150MB + 2min runner cost; first divergence is rare; maintainer-local diff is cheaper. |
| Registry submission script | GHA workflow that opens a PR against reproducible-builds.org | Manual maintainer procedure in `docs/RELEASING.md` (D-14) | Registry PRs are human-reviewed; not automatable. |
| GitHub runner ImageVersion API call | `curl` to GH API | `${ImageVersion}` env var (set by runner; format `20260518.149.1`) [VERIFIED: actions/runner-images docs] | The runner sets this automatically — no API call needed. |

**Key insight:** Phase 25 is mostly "wire well-known reproducibility primitives in the right shape." Almost no novel logic. The TWO load-bearing additions are (1) the verifier's two-title issue scheme (D-12) — which is a thin variant of Phase 22's pattern — and (2) the SOURCE_DATE_EPOCH derivation step's ordering before the Build step. Everything else is documented configuration.

## Runtime State Inventory

> Phase 25 is greenfield + documentation + workflow YAML. No data migration, no live-service config change, no OS registration. The only stateful surface this phase touches is GitHub Issues (via the verifier's `gh issue create`) — which is by design (REPRO-03) and uses Phase 22's same pattern.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — phase introduces zero data writes outside (a) the source tree (NEW files + MODIFIED files committed to git) and (b) GitHub Issues created by the verifier (which is the explicit REPRO-03 contract). | None |
| Live service config | None — no external services to reconfigure. GitHub Actions runners are pulled fresh per run; the `reproducible-verify.yml` workflow IS the new live config but it's defined in source. | None |
| OS-registered state | None — no OS-level registrations. | None |
| Secrets/env vars | `SOURCE_DATE_EPOCH` is a new BUILD-TIME env var set in `$GITHUB_ENV` per run; not a secret. `RUSTFLAGS` and `CARGO_INCREMENTAL` are new BUILD-TIME env vars set in the `build` job's `env:` block; not secrets. No new repo secrets need to be added. `GITHUB_TOKEN` (existing) covers `gh release download` + `gh issue list/create` in the verifier. | None — verifier uses default `GITHUB_TOKEN` with the narrower `contents: read + issues: write` permissions block. |
| Build artifacts | The release tarball's BYTES will change after Phase 25 — but this is the WHOLE POINT of the phase. The first post-Phase-25 release (v1.6.0-rc.0) is the rehearsal that captures the new deterministic hash; the `<TBD-v1.6.0-cut>` placeholder in `docs/REPRODUCIBLE-BUILD.md` is replaced with the real hash at the rc.0 cut (D-10 procedure). No PRE-Phase-25 artifact needs to be deleted or migrated. | The v1.6.0-rc.0 rehearsal procedure (D-10/D-14 in `docs/RELEASING.md`) IS the artifact-state transition; document explicitly. |

**Nothing found in any category:** Verified explicitly. Phase 25 is structurally a "introduce new determinism + verifier + docs" phase — no migration surface.

## Common Pitfalls

### Pitfall A: `dtolnay/rust-toolchain` does NOT respect `rust-toolchain.toml` (CONTEXT D-01 + D-21 wrong assumption)

**What goes wrong:** CONTEXT D-01's prose claims `dtolnay/rust-toolchain` respects `rust-toolchain.toml` when present. D-21 builds on this — instructing the planner to remove `with: toolchain: stable` from 6 `with:` blocks in `release.yml` (lines 39, 92) and `ci.yml` (lines 34, 142, 163, 177) so the file takes precedence. **Both assumptions are false.**

**Verified evidence (HIGH confidence):**
1. The action.yml at the project's pinned SHA `3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9` (verified 2026-06-02 via `gh api /repos/dtolnay/rust-toolchain/contents/action.yml?ref=3c5f7ea2…`) contains an explicit parse-step check:
   ```bash
   if [[ -z $toolchain ]]; then
     # GitHub does not enforce `required: true` inputs itself.
     echo "'toolchain' is a required input" >&2
     exit 1
   fi
   ```
2. The action's `inputs.toolchain` is declared `required: true` (no default value).
3. The dtolnay/rust-toolchain README does not mention `rust-toolchain.toml` at all.
4. Two third-party actions exist specifically because dtolnay rejected this feature: `dsherret/rust-toolchain-file` and `mkroening/rust-toolchain-toml`. The README on the former notes "the original dtolnay/rust-toolchain did not want to support installing from a rust-toolchain.toml file."

**Why it happens:** Maintainer policy decision — dtolnay considers toolchain selection the workflow author's concern, not the action's.

**How to avoid (RECOMMENDED — KEEP-with-pin-match):**
- DO NOT remove `with: toolchain:` from the 6 `with:` blocks per D-21. Instead, CHANGE the value from `stable` to the explicit pinned version `"1.95.0"` so it matches `rust-toolchain.toml`'s `[toolchain] channel = "1.95.0"`.
- Add a CI grep gate (mirroring `bip322-pin-check` in ci.yml:214-236) that fails if any `with: toolchain:` value ≠ the `channel` value in `rust-toolchain.toml`. Single source of truth enforced at file level rather than at action behavior level.
- `rust-toolchain.toml` still lands per D-01 — it's load-bearing for (a) local-dev (rustup auto-installs the right version on first `cargo` invocation), (b) the verifier's rebuild step (which runs `cargo build` after `dtolnay/rust-toolchain` installs the explicitly-pinned version; if the workspace `rust-toolchain.toml` says a DIFFERENT version, rustup will auto-install THAT version on the `cargo build` invocation, and the build will use it — the explicit `with:` input thus needs to MATCH the file).

**Alternative paths (lower-recommended):**
- SWITCH action: replace `dtolnay/rust-toolchain` with `dsherret/rust-toolchain-file@<SHA>` in 6 places. Tradeoffs: introduces unpinned-by-precedent SHAs, breaks the project's Phase-22/23/24 action-pin discipline, requires verification that the new action installs `profile = "minimal" components = ["rustfmt", "clippy"]` correctly. Higher blast radius for marginal gain.
- DROP action: remove the `dtolnay/rust-toolchain` step entirely. Tradeoffs: rustup is preinstalled on `ubuntu-24.04` but the action also handles `--profile minimal` and component installation; dropping it means the first `cargo` invocation triggers auto-install per `rust-toolchain.toml`, which is slower (one-time on cache miss) and surfaces a download error mid-build instead of upfront. Not recommended.

**Warning signs (during planning + verification):**
- If the planner's task action removes `with: toolchain:` per CONTEXT D-21 verbatim, the CI run on the first PR will fail with `'toolchain' is a required input`. This is the early-detection mechanism — fail-fast.
- If the planner KEEPS `with: toolchain: stable` (does nothing), the rust-toolchain.toml file silently overrides on `cargo build` (rustup auto-installs the pinned version), but the explicit `with:` says `stable`. Two sources of truth, drift is invisible until a stable bump.

**Phase mapping:** Phase 25 D-01 + D-21 (load-bearing — affects how rust-toolchain.toml is integrated).

### Pitfall 6 (PITFALLS.md inherited): Rust reproducible-build long tail

**What goes wrong:** Even with `--remap-path-prefix` + `SOURCE_DATE_EPOCH` + `--locked`, Rust binaries can fail bit-for-bit reproducibility due to: `proc-macro` crates that use `Instant::now()` at compile time; `build.rs` scripts that consult `env::current_dir()` or `chrono::Local::now()`; LLVM optimizations on certain targets producing nondeterministic ordering; cargo's incremental compilation interfering; random hash for `dyn Trait` vtable inclusion order.

**Why it happens:** Rust's reproducibility story is "mostly works" not "guaranteed works"; the last 5% is project-specific and surfaces only on rebuild.

**How to avoid:**
- First reproducibility run on a clean runner: capture EVERY diff. Most are fixable.
- Use `diffoscope` (deferred per CONTEXT — maintainer-local run on first divergence).
- Accept the realistic Phase-25 target: bit-for-bit equality ON `ubuntu-24.04` (specific ImageVersion) with documented env. The two-title D-12 scheme exists for precisely this — runner-image drift is low-severity, real divergence is HIGH-severity.

**Warning signs:** The first v1.6.0-rc.0 rehearsal (D-10) is the moment of truth. If `sha256sum` is non-deterministic across two `workflow_dispatch` runs on the SAME `ubuntu-24.04` ImageVersion, Pitfall 6 is biting. Triage path: run `diffoscope` locally; the most likely culprits are (in descending probability) `build.rs` scripts in dependencies, embedded rand-seeded data, then proc-macro time issues.

**Phase mapping:** Phase 25 — the verifier IS the iteration mechanism.

### Pitfall 7 (PITFALLS.md inherited): Verifier false-positives on `ubuntu-latest` rotation

**What goes wrong:** A monthly verifier on `ubuntu-latest` would false-positive every ~month when GH rotates the runner image.

**Prevention:** Locked via D-08 + D-11 — both `release.yml` `build` job and `reproducible-verify.yml` pin `ubuntu-24.04`. The `${ImageVersion}` capture (D-11 step 1) + D-12 two-title scheme handles the case where `ubuntu-24.04` itself gets a new ImageVersion (low-severity issue).

**Phase mapping:** Phase 25.

### Pitfall B: `${ImageVersion}` env var lifecycle (low-risk, but planner should know)

**What goes wrong:** `${ImageVersion}` is set by the GitHub-hosted runner on Linux images (verified: format `20260518.149.1`). It is NOT a standard GitHub Actions context variable — it's a runner-set environment variable. If a maintainer ever switches to a self-hosted runner, `${ImageVersion}` won't be set; the verifier will silently fail or write an empty value.

**Prevention:** Add a guard in the verifier's step 1: `[[ -n "${ImageVersion:-}" ]] || { echo "ImageVersion env var not set — verifier requires a GitHub-hosted ubuntu-24.04 runner"; exit 1; }`. Cheap defensive check; surfaces the misconfiguration immediately.

**Phase mapping:** Phase 25 D-11 step 1.

### Pitfall C: `--locked` is not enough if `Cargo.lock` is missing from the verifier's checkout

**What goes wrong:** `cargo build --locked` exits 101 if `Cargo.lock` is missing or out of sync. The verifier MUST checkout a ref that includes `Cargo.lock` (it's committed to the repo; verified: `ls Cargo.lock` exists at workspace root).

**Prevention:** Verifier step 5 (checkout source at `$LATEST_TAG`) uses `actions/checkout@<SHA>` with default behavior — this checks out the full tree including `Cargo.lock`. No special config needed. Planner: do NOT add `sparse-checkout:` to the verifier's checkout step (would risk excluding `Cargo.lock`).

**Phase mapping:** Phase 25 D-11 step 5.

## Code Examples

### Example 1: `rust-toolchain.toml` (NEW FILE, D-01/D-15)

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

### Example 2: `Cargo.toml` `[profile.release]` addition (D-07)

```toml
# Phase 25 REPRO-01: strip symbol-table entries from release binaries.
# Belt-and-suspenders alongside --remap-path-prefix — symbols would otherwise
# embed compiler-version-specific bytes that vary across rebuilds. Workspace-
# level [profile.release] applies to all four crate binaries uniformly.
# strip = "symbols" is stable since Rust 1.59; explicit "symbols" strips more
# aggressively than the synonym `true` and is the reproducible-builds.org
# recommended setting.
[profile.release]
strip = "symbols"
```

(Insert AFTER `[workspace.dependencies]` block, which currently ends at Cargo.toml:37. Verified: no existing `[profile.*]` block in Cargo.toml.)

### Example 3: `release.yml` `Compute SOURCE_DATE_EPOCH` step (D-03)

```yaml
# Phase 25 REPRO-02: derive SOURCE_DATE_EPOCH from the tagged commit time.
# Locked to git's recorded committer-time on $GITHUB_SHA so the value is
# reproducible from source — an external rebuilder running `git log -1
# --format=%ct v1.6.0` on a fresh clone gets the same epoch, the same
# debug-info baseline, and the same tar entry mtimes (per D-06).
# The value is written to $GITHUB_ENV; per GH-docs the assignment propagates
# to all subsequent steps in the same job automatically (verified
# 2026-06-02 via docs.github.com/en/actions docs).
- name: Compute SOURCE_DATE_EPOCH from tagged commit time
  run: echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)" >> $GITHUB_ENV
```

(Insert in `release.yml` `build` job AFTER the "Read canonical base-image digests" step at line 105 and BEFORE the "Build coordinator and client" step at line 107.)

### Example 4: `release.yml` deterministic `Package` step (D-06)

```yaml
# Phase 25 REPRO-01: deterministic tar + gzip. Five flags load-bearing for
# byte-equality: --sort=name (deterministic file order across filesystem
# walk variance), --owner/--group=0 --numeric-owner (strip runner user
# identity), --mtime="@$SOURCE_DATE_EPOCH" (uniform timestamps from D-03),
# and `gzip -n` (strip ORIGINAL_NAME + MTIME from the gzip header — default
# gzip embeds the input filename AND its mtime, the most common
# "tar matches, gzip doesn't" failure mode in repro-build reports).
- name: Package (deterministic tar + gzip)
  run: |
    mkdir -p dist
    cp target/release/coordinator dist/
    cp target/release/client dist/
    cp target/release/liquidity-bot dist/
    tar --sort=name --owner=0 --group=0 --numeric-owner \
        --mtime="@${SOURCE_DATE_EPOCH}" \
        -cf - -C dist . \
      | gzip -n > blindjoin-linux-amd64.tar.gz
    sha256sum blindjoin-linux-amd64.tar.gz > blindjoin-linux-amd64.tar.gz.sha256
```

(REPLACES the existing `Package` step at `release.yml:110-117`.)

### Example 5: `reproducible-verify.yml` skeleton (D-11)

```yaml
name: Reproducible build verifier

# Phase 25 REPRO-03: scheduled monthly verifier that proves blindjoin-linux-amd64.tar.gz
# is byte-equal across rebuilds. On mismatch: opens a [reproducibility-regression]
# issue per D-12's two-title scheme (runner-image-drift low-severity vs sha256-mismatch
# HIGH-severity). Mirrors Phase 22's digest-drift-check.yml structural template.

env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"

on:
  schedule:
    # 07:00 UTC on the 1st of each month. Verified 2026-06-02 to not collide
    # with the daily 09:00 UTC schedule of digest-drift-check.yml (Phase 22)
    # — different hour AND monthly vs daily cadence.
    - cron: '0 7 1 * *'
  workflow_dispatch:

# Phase 25 REPRO-03: verifier needs read-only source access + write access to
# open [reproducibility-regression] issues. NOTHING else. Deliberately omitted
# (auditor-grepable): id-token (verifier neither signs nor pushes attestations
# — only re-verifies the cosign sig on the downloaded tarball, a read-only
# operation), attestations, packages, PR-write, pages, deployments. These tokens
# MUST NOT appear anywhere in this file at any indentation.
permissions:
  contents: read
  issues: write

jobs:
  verify:
    name: Re-verify reproducibility of latest release
    runs-on: ubuntu-24.04
    steps:
      - name: Capture runner ImageVersion
        run: |
          [[ -n "${ImageVersion:-}" ]] || { echo "ImageVersion env var not set — verifier requires a GitHub-hosted ubuntu-24.04 runner"; exit 1; }
          echo "VERIFIER_IMAGE_VERSION=${ImageVersion}" >> $GITHUB_ENV
          echo "Verifier image: ${ImageVersion}"

      - name: Resolve latest release tag
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          LATEST_TAG=$(gh release view --json tagName --jq .tagName)
          echo "LATEST_TAG=${LATEST_TAG}" >> $GITHUB_ENV
          echo "Latest release tag: ${LATEST_TAG}"

      - name: Download release tarball + cosign bundle
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          mkdir -p /tmp/rel
          gh release download "${LATEST_TAG}" --pattern 'blindjoin-linux-amd64.tar.gz*' --dir /tmp/rel
          ls -la /tmp/rel

      # Phase 24 SIGN-01 inheritance — cosign re-verify ties reproducibility
      # green-status to signed-supply-chain green-status; both must hold.
      - name: Install cosign
        uses: sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5  # v3.10.1
        with:
          cosign-release: 'v2.6.3'

      - name: Re-verify cosign blob signature on downloaded tarball
        run: |
          cd /tmp/rel
          cosign verify-blob \
            --bundle blindjoin-linux-amd64.tar.gz.bundle \
            --certificate-identity-regexp 'https://github.com/${{ github.repository_owner }}/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
            --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
            blindjoin-linux-amd64.tar.gz

      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5  # v4.3.1
        with:
          ref: ${{ env.LATEST_TAG }}

      # Pitfall A: dtolnay/rust-toolchain requires explicit toolchain: input.
      # Pin value matches rust-toolchain.toml channel via the grep gate in ci.yml.
      - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9  # stable
        with:
          toolchain: "1.95.0"

      - name: Rebuild per REPRO-01 recipe
        env:
          RUSTFLAGS: "--remap-path-prefix=${{ github.workspace }}=/build --remap-path-prefix=/home/runner/.cargo=/cargo"
          CARGO_INCREMENTAL: "0"
        run: |
          export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)
          cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot
          mkdir -p dist
          cp target/release/coordinator dist/
          cp target/release/client dist/
          cp target/release/liquidity-bot dist/
          tar --sort=name --owner=0 --group=0 --numeric-owner \
              --mtime="@${SOURCE_DATE_EPOCH}" \
              -cf - -C dist . \
            | gzip -n > blindjoin-linux-amd64.tar.gz

      - name: Compare sha256 + classify result + open issue on mismatch
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          set -euo pipefail
          EXPECTED=$(sha256sum /tmp/rel/blindjoin-linux-amd64.tar.gz | cut -d' ' -f1)
          ACTUAL=$(sha256sum blindjoin-linux-amd64.tar.gz | cut -d' ' -f1)
          echo "Expected (downloaded release): ${EXPECTED}"
          echo "Actual   (rebuilt locally):    ${ACTUAL}"

          # Auto-create label (idempotent).
          gh label create reproducibility-regression \
            --description "Automated reproducibility regression report from reproducible-verify.yml" \
            --color "fbca04" 2>/dev/null || true

          if [ "${EXPECTED}" = "${ACTUAL}" ]; then
            echo "✓ Reproducibility verified for ${LATEST_TAG} on ImageVersion ${VERIFIER_IMAGE_VERSION}"
            exit 0
          fi

          # Mismatch. Classify: runner-image drift vs real divergence.
          # PINNED_IMAGE_VERSION comes from docs/REPRODUCIBLE-BUILD.md §Toolchain pins.
          # Planner picks lookup strategy per D-18; example uses grep.
          PINNED_IMAGE_VERSION=$(grep -oP 'ImageVersion: \K[0-9.]+' docs/REPRODUCIBLE-BUILD.md || echo "UNKNOWN")

          if [ "${VERIFIER_IMAGE_VERSION}" != "${PINNED_IMAGE_VERSION}" ]; then
            TITLE="[reproducibility-regression] runner image drift: ImageVersion ${PINNED_IMAGE_VERSION} → ${VERIFIER_IMAGE_VERSION}"
            BODY=$'GitHub rotated the `ubuntu-24.04` runner image SHA.\n\nVerify reproducibility on the new image; update `docs/REPRODUCIBLE-BUILD.md` with the new ImageVersion if green; investigate if not. **Not a supply-chain signal** until investigated.'
          else
            TITLE="[reproducibility-regression] sha256 mismatch on ImageVersion ${VERIFIER_IMAGE_VERSION}"
            BODY=$'Rebuilt tarball diverges from published `'${LATEST_TAG}$'` on the SAME `ubuntu-24.04` ImageVersion `'${VERIFIER_IMAGE_VERSION}$'`.\n\nThis is a **real supply-chain signal** — published release does not reproduce from source. Compare diffoscope output; suspect tampering, compromised CI, or undocumented build-env drift.'
          fi

          # Dedup by exact title (Phase 22 pattern).
          EXISTING=$(gh issue list \
            --label reproducibility-regression \
            --state open \
            --search "\"${TITLE}\" in:title" \
            --json number,title \
            --jq '.[] | select(.title == "'"${TITLE}"'") | .number' \
            | head -n1)

          if [ -n "${EXISTING}" ]; then
            echo "→ existing issue #${EXISTING} already tracks this regression; skipping"
            exit 1   # still fail the run — green-only is the contract
          fi

          gh issue create \
            --title "${TITLE}" \
            --body "${BODY}" \
            --label reproducibility-regression \
            --assignee "${GITHUB_REPOSITORY_OWNER}"

          exit 1   # mismatch must fail the run
```

(NEW FILE at `.github/workflows/reproducible-verify.yml`. Skeleton — planner adjusts per D-17/D-18 final shape.)

### Example 6: D-13 `release.yml` softprops step post-cleanup

```yaml
# Phase 25 D-13: Phase 24 SIGN-03 (PGP path) was deferred indefinitely
# 2026-06-02 (commit f11d544). The release publishes directly — every tag
# push produces a non-draft GitHub Release with all 4 assets (tarball +
# sha256 + cosign bundle + SLSA provenance). The reproducible-verify.yml
# workflow can `gh release download` without --draft + GITHUB_TOKEN gymnastics
# since public releases require no auth.
#
# Phase 24 D-15: files: list is in semantic order — artifact → integrity →
# signature → provenance. No .asc PGP detached signature is uploaded
# (SIGN-03 deferred).
- name: Upload to GitHub Releases
  uses: softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1
  with:
    files: |
      blindjoin-linux-amd64.tar.gz
      blindjoin-linux-amd64.tar.gz.sha256
      blindjoin-linux-amd64.tar.gz.bundle
      blindjoin-linux-amd64.tar.gz.sigstore
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

(REPLACES `release.yml:201-222`. Key change: `draft: true` line removed from `with:` block; step name no longer mentions "draft"; comment block rewritten to cite D-13 + the 2026-06-02 SIGN-03 deferral.)

### Example 7: `docs/RELEASING.md` cleanup of `--draft=false` references (D-13 procedural side)

Current `docs/RELEASING.md` has 4 references to the draft-flip flow (verified at planning time: lines 5, 18, 29, 31, 34, 37, 39, 65). After D-13, the entire "Pre-flight check before flipping out of draft" section (currently lines 37-65) needs restructuring because the release no longer ships as draft. Planner: rename to "Pre-flight check after CI completes" and remove all `--draft=false` references. The cosign verify commands themselves stay; only the timing context changes.

Replacement step 4 prose (replacing the current draft-flip):
```markdown
4. **Verify the published release.** Once CI is green and pre-flight passes,
   the release is already published. Operators can `gh release download` immediately.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tar czf` shorthand | `tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$EPOCH" -cf - \| gzip -n` | Reproducible-builds.org convention since ~2018 [VERIFIED: reproducible-builds.org/docs/archives/] | Five flags load-bearing; CONTEXT D-06 locks the exact shape |
| `RUSTFLAGS="-C debuginfo=0"` (strip-by-removing) | `RUSTFLAGS=--remap-path-prefix=... + [profile.release] strip="symbols"` | Rust 1.59 (strip stable; February 2022) | Cleaner answer; debug info still present in unstripped builds for crash analysis |
| `cargo build --release` | `cargo build --release --locked` | de-facto reproducible-build standard since ~2020 | Fail-fast on stale `Cargo.lock`; mandatory per REPRO-02 |
| `ubuntu-latest` runner | `ubuntu-24.04` explicit pin | Reproducible-build standard | Pitfall 7 — `ubuntu-latest` rotates ~monthly, false-positives every cycle |
| Manual digest-drift PR | Auto-issue + human-PR (Phase 22 pattern) | Phase 22 (2026-06-01) | Phase 25 inherits the issue-not-PR pattern for `[reproducibility-regression]` |
| Hand-rolled SLSA provenance | `actions/attest-build-provenance` (Phase 23 choice) | Phase 23 (2026-06-02) | Phase 25 verifier re-uses the cosign+sigstore stack verbatim |

**Deprecated/outdated:**
- `bitcoincore-rpc` Rust crate (archived Nov 2025) — not Phase 25 relevant
- `RUSTFLAGS=-C debuginfo=0` (replaced by `strip = "symbols"` since Rust 1.59)
- `gzip` without `-n` (still default but considered nondeterministic by reproducible-builds.org since 2018)

## Project Constraints (from CLAUDE.md)

- **Cryptography:** No custom crypto — Phase 25 introduces no new crypto. The verifier's `cosign verify-blob` re-uses Phase 24's stack. ✓ honored
- **Network:** Tor-native in production. Phase 25's verifier runs on GitHub-hosted runners over clearnet — but this is the supply-chain audit pipeline, not production traffic. Acceptable per CLAUDE.md's "development/testing may use clearnet TCP" exception. ✓ honored
- **Scope:** Signet-first. Not Phase 25 relevant. ✓ honored
- **Privacy:** No PII logging. Verifier logs `${ImageVersion}` (an opaque public version string), the LATEST_TAG (public), and sha256 hashes (cryptographic digests). No PII. ✓ honored
- **License:** MIT. New files inherit the project license; no per-file LICENSE header needed (verified by existing file patterns).
- **GSD Workflow Enforcement:** Phase 25 work goes through `/gsd:execute-phase`; the planner respects this. ✓ honored
- **Recommended Stack (from CLAUDE.md):** No new Rust deps added in Phase 25 — `tokio`, `bitcoin`, `bdk_wallet`, `blind-rsa-signatures`, etc. all unchanged. The only Cargo.toml edit is `[profile.release]` which is build-config, not a dependency. ✓ honored

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `${ImageVersion}` is set by all GitHub-hosted Linux runners including ubuntu-24.04, format `YYYYMMDD.N.M` | Pattern 3 + Example 5 + Pitfall B | LOW — verified via web search + actions/runner-images repo; defensive guard in Example 5 catches misconfiguration immediately. If wrong, verifier step 1 fails early with clear message |
| A2 | The CONTEXT-suggested `0 7 1 * *` cron is the right monthly slot | Pattern 3 / D-16 | LOW — verified non-colliding with existing `0 9 * * *` daily Phase 22 schedule; planner can change with no downstream impact |
| A3 | `softprops/action-gh-release` with no `draft:` key defaults to non-draft release | Example 6 (D-13) | LOW — verified against the v1 documented defaults; if wrong (e.g., a future v2 changes defaults), CI rehearsal at first non-draft tag push catches it |
| A4 | The 4 `--draft=false` references in `docs/RELEASING.md` are all parts of the post-Phase-24-D-07 draft-flip flow and ALL should be removed/restructured by D-13 | Example 7 + D-13 | LOW-MEDIUM — visually verified by reading the doc; planner should re-read at planning time and confirm the cleanup scope. If a reference is missed, doc-grep CI step (if any) catches it |
| A5 | The verifier's expected-sha256 lookup mechanism (D-18) — recommendation of separate `.expected-sha256` file vs markdown-table parse | Standard Stack Alternatives | LOW — both work; planner's judgement call. CONTEXT D-18 explicitly leaves to discretion |
| A6 | Phase 22's `bip322-pin-check` CI grep pattern is the right template for a new `rust-toolchain-pin-check` job to enforce single-source-of-truth between `rust-toolchain.toml` and `with: toolchain:` values | Pitfall A (KEEP-with-pin-match) | LOW — verified pattern exists at ci.yml:214-236 and is well-established. Planner can mirror exactly |
| A7 | The `dtolnay/rust-toolchain` at SHA `3c5f7ea2…` will still require `toolchain:` input on the next CI run (no behavior change since the SHA is pinned) | Pitfall A | HIGH confidence; the action.yml at that SHA was inspected verbatim 2026-06-02 |

## Open Questions

1. **Should the `rust-toolchain-pin-check` grep gate be ADDED to ci.yml as part of Phase 25, or DEFERRED to a follow-on?**
   - What we know: Pitfall A surfaces the dual-source-of-truth risk between `rust-toolchain.toml` and `with: toolchain:` blocks. A grep gate prevents drift.
   - What's unclear: CONTEXT D-21 doesn't mention this gate (it assumes the file wins, so no gate needed). Adding a new ci.yml job expands Phase 25's CI surface beyond what D-21 implies.
   - Recommendation: ADD the gate in Phase 25 as part of the same PLAN that lands D-01/D-21. Mirrors `bip322-pin-check`'s ~10-line shape; near-zero marginal cost; closes the dual-source-of-truth window opened by KEEP-with-pin-match. If planner prefers, can be a separate small PLAN (D-21B style) — but should not be deferred to a future phase.

2. **What value to put for `ImageVersion:` in `docs/REPRODUCIBLE-BUILD.md` initially?**
   - What we know: D-09 §3 (Toolchain pins) and D-12 reference the ImageVersion as a pinned value to compare against. D-10 procedure captures the sha256 at v1.6.0-rc.0 rehearsal.
   - What's unclear: The ImageVersion at rc.0 cut isn't known yet. D-10 captures it post-hoc.
   - Recommendation: Initial value `<TBD-v1.6.0-cut>` (same placeholder as the sha256). The rc.0 rehearsal procedure (D-10 in docs/RELEASING.md) captures BOTH ImageVersion AND sha256 in one step, and the maintainer commits both replacements before tagging.

3. **Should the verifier exit-fail on `[reproducibility-regression]` mismatch even if an existing issue is open?**
   - What we know: Phase 22's `digest-drift-check.yml` returns 0 if an existing issue is open (idempotent — skip create, continue). Example 5 here exits 1 in both create-and-skip-create branches on mismatch.
   - What's unclear: Whether the planner wants "monthly run goes red until fixed" (exit 1 always on mismatch) or "monthly run goes green once issue is acknowledged" (exit 0 when existing issue open).
   - Recommendation: Exit 1 ALWAYS on mismatch (Example 5 shape). A green monthly run is the precondition for D-14 registry submission — silencing the red because an issue is open weakens that precondition. Trade-off accepted.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| rustc | Local-dev / verifier rebuild | ✓ | 1.95.0 (maintainer machine, verified 2026-06-02) | — |
| cargo | Local-dev / verifier rebuild | ✓ | 1.95.0 (maintainer machine) | — |
| GNU tar | Verifier + release.yml Package step | ✓ on ubuntu-24.04 | bundled | — |
| GNU gzip | Verifier + release.yml Package step | ✓ on ubuntu-24.04 | bundled | — |
| cosign | Verifier step 4 (re-verify sig) | ✓ via SHA-pinned cosign-installer | 2.6.3 | — |
| gh CLI | Verifier (release download, issue create) | ✓ preinstalled on ubuntu-24.04 | latest | — |
| git | Verifier (clone, log -1) | ✓ preinstalled | latest | — |
| sha256sum | Verifier + Package step | ✓ via GNU coreutils on ubuntu-24.04 | bundled | — |
| docker buildx | NOT NEEDED in Phase 25 | n/a | — | — |
| `dtolnay/rust-toolchain` action | release.yml + ci.yml + verifier | ✓ via SHA pin (project-existing) | `3c5f7ea2…` | KEEP `with: toolchain:` per Pitfall A |
| `sigstore/cosign-installer` action | release.yml + verifier | ✓ via SHA pin (Phase 23/24 inheritance) | `7e8b541…` v3.10.1 | — |
| `actions/checkout` action | release.yml + ci.yml + verifier | ✓ via SHA pin | `34e1148…` v4.3.1 | — |
| reproducible-builds.org registry | REPRO-04 (manual maintainer submission) | external; HTTP-accessible | n/a | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None — Phase 25 is fully serviced by ubuntu-24.04 runner image preinstalls + existing project SHA-pinned actions.

## Validation Architecture

> **Skipped per .planning/config.json `workflow.nyquist_validation: false` (verified 2026-06-02).** Phase 25 validation strategy is:
> - **The verifier IS the perpetual validation** (Pitfall 12 — fresh-machine UAT every month).
> - **The v1.6.0-rc.0 rehearsal (D-10) is the one-shot dry-run** that captures the expected sha256 + ImageVersion + proves the recipe works end-to-end.
> - **Standard CI** (existing ci.yml `test` + `clippy` + `audit` jobs) covers the Cargo.toml `[profile.release]` addition automatically — those jobs build the workspace with the new profile and would surface any compile-time issues. No new test files needed.
> - **Phase 23's sigstore-pin-check job at ci.yml:292-326** covers the new `reproducible-verify.yml`'s `sigstore/cosign-installer` step automatically (greps every workflow under `.github/workflows/`). No new CI gate needed for that.
> - **NEW grep gate `rust-toolchain-pin-check` in ci.yml** is the one validation surface added by Phase 25 — closes the dual-source-of-truth window between `rust-toolchain.toml` and `with: toolchain:` blocks (per Pitfall A). Mirrors `bip322-pin-check` shape (~10 lines).
> - **Forbidden-token absence audit** at PLAN-level acceptance: `! grep -q 'ubuntu-latest'` on the build job stanza of release.yml; `! grep -q 'draft: true'` on release.yml; `! grep -q '<TBD-v1.6.0-cut>'` on `docs/REPRODUCIBLE-BUILD.md` post-rehearsal (planner discretion whether to make this a CI gate per D-10 prose).

## Security Domain

> Phase 25 is supply-chain hardening for the existing release pipeline. ASVS categories that apply:

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | GitHub OIDC for cosign keyless flow (Phase 23/24 inheritance; verifier RE-VERIFIES, doesn't re-sign) |
| V3 Session Management | no | n/a — no user sessions in a CI verifier |
| V4 Access Control | yes | `permissions:` block on verifier job (`contents: read + issues: write` only); Phase 22 deliberately-omitted-scopes pattern |
| V5 Input Validation | yes (light) | `gh release view --json tagName --jq .tagName` is structured; LATEST_TAG sanity-check via `[[ -n "$LATEST_TAG" ]]` (already practiced in Phase 22) |
| V6 Cryptography | yes | Never hand-roll. Re-uses Phase 23/24 cosign stack verbatim; no new crypto primitives |
| V10 Malicious Code | yes | All new GHA actions are SHA-pinned (Pitfall 4 inheritance); sigstore-pin-check gate covers the new use |
| V14 Configuration | yes | `rust-toolchain.toml` + `[profile.release]` + workflow env blocks; auditor-grepable comments per Phase 22-04 pattern |

### Known Threat Patterns for Phase 25's surface

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Toolchain spoofing (CI installs different version than `rust-toolchain.toml` declares) | Tampering | NEW `rust-toolchain-pin-check` grep gate (Pitfall A KEEP-with-pin-match approach) |
| Verifier false-positive masks real divergence | Information Disclosure (operator confusion) | D-12 two-title scheme distinguishes low-severity (runner image drift) from HIGH-severity (sha256 mismatch on same image) |
| Issue spam from chronic regression | Repudiation (audit trail noise) | Phase 22 idempotency pattern — title-exact-match-skip |
| Maintainer skips registry submission, project appears unreproducible to external operators | Information Disclosure | D-14 procedure in `docs/RELEASING.md` + SECURITY.md cross-link (D-19) make the gap visible |
| External rebuilder follows out-of-date Recipe and gets a different hash | Information Disclosure | `docs/REPRODUCIBLE-BUILD.md` includes per-tag expected sha256 table; mismatch is immediately diagnosable |
| Cosign 3.0 CLI flag drift breaks verifier | Denial of Service (verifier breaks) | Verifier uses SHA-pinned cosign-installer (v2.6.3 release pin); only updated via maintainer-reviewed PR (Pitfall 13 inheritance) |

## Sources

### Primary (HIGH confidence)
- [dtolnay/rust-toolchain action.yml at pinned SHA](https://github.com/dtolnay/rust-toolchain/blob/3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9/action.yml) — Verified via `gh api` 2026-06-02; parse step requires `toolchain:` input (Pitfall A evidence)
- [Cargo `--locked` documentation](https://doc.rust-lang.org/cargo/commands/cargo-build.html) — Fails with exit 101 on stale `Cargo.lock`
- [GitHub Actions $GITHUB_ENV propagation](https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/store-information-in-variables) — Confirmed env var propagates to subsequent steps in same job
- [reproducible-builds.org archive metadata](https://reproducible-builds.org/docs/archives/) — tar + gzip determinism flags
- [Debian wiki: TimestampsInGzipHeaders](https://wiki.debian.org/ReproducibleBuilds/TimestampsInGzipHeaders) — `gzip -n` strips ORIGINAL_NAME + MTIME
- Project files verified 2026-06-02: `Cargo.toml` (no `[profile.release]` block); `Cargo.lock` exists; `rust-toolchain.toml` does NOT exist; `.github/workflows/release.yml` (107→222 lines from Phase 24); `.github/workflows/ci.yml` (327 lines with sigstore-pin-check at 292-326); `.github/workflows/digest-drift-check.yml` (cron `0 9 * * *`); `docs/RELEASING.md` (66 lines, 4 `--draft=false` references); `SECURITY.md` §Supply-chain status at L95-289
- `rustc --version` on maintainer machine: `1.95.0 (59807616e 2026-04-14)`

### Secondary (MEDIUM confidence)
- [actions/runner-images Ubuntu2404-Readme.md](https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md) — `ImageVersion` env var set on runners; format `YYYYMMDD.N.M`
- [dsherret/rust-toolchain-file README](https://github.com/dsherret/rust-toolchain-file) — Confirms dtolnay rejected `rust-toolchain.toml` support (third-party action exists specifically because of this)
- Phase 22/23/24 CONTEXT + SUMMARY files — pattern inheritance (issue-not-PR, deliberately-omitted-scopes, paraphrased forbidden tokens, SHA-pin discipline, cron-stagger, sigstore-pin-check inheritance)

### Tertiary (LOW confidence)
- None — every claim in this research was verified against at least one HIGH-confidence source or MEDIUM-confidence direct project inspection.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all packages already in project; versions verified
- Architecture: HIGH — fully mirrors Phase 22/23/24 established patterns
- Pitfalls: HIGH on inherited pitfalls (6, 7, 11, 12, 13 from PITFALLS.md); HIGH on Pitfall A (verified via action.yml inspection); MEDIUM on first-rebuild outcome (Pitfall 6 long tail is project-specific — only the verifier itself proves it)
- File-level facts: HIGH — every line number cited in this research was verified against the live file 2026-06-02

**Research date:** 2026-06-02
**Valid until:** 2026-07-02 (30 days for stable supply-chain stack); shorter (7 days) for the `rustc 1.95.0` claim if Rust 1.96 ships in the interim — planner should re-verify `rustc --version` at plan time
