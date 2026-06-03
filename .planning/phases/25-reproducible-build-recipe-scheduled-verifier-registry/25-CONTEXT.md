# Phase 25: Reproducible-Build Recipe + Scheduled Verifier + Registry - Context

**Gathered:** 2026-06-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 25 makes `blindjoin-linux-amd64.tar.gz` byte-equal across rebuilds and proves it continuously. Three deliverable surfaces: (1) `docs/REPRODUCIBLE-BUILD.md` documenting the exact toolchain pin, `ubuntu-24.04` runner image, env vars (`SOURCE_DATE_EPOCH`, `RUSTFLAGS`, `CARGO_INCREMENTAL=0`), build invocation, and expected `sha256sum`; (2) `.github/workflows/release.yml` `build` job hardened for determinism (env block + `--locked` + deterministic `tar`/`gzip` + `runs-on: ubuntu-24.04`); (3) a new `.github/workflows/reproducible-verify.yml` that runs monthly on a fresh `ubuntu-24.04` runner, downloads the latest release tarball, rebuilds via the REPRO-01 recipe, asserts `sha256sum` equality, and on mismatch opens a `[reproducibility-regression]` issue distinguishing runner-image drift from real divergence. Plus the maintainer-side procedure (verifier rehearsal at v1.6.0 cut to capture the expected hash; registry submission after one green monthly cycle) lives in `docs/RELEASING.md`. Phase 25 also folds in a tightly-coupled cleanup: the orphan `draft: true` step from Phase 24 (PGP path deferred indefinitely 2026-06-02) is removed in the same `release.yml` change so the verifier can `gh release download` without auth coupling.

What this phase does NOT do: image-side reproducibility (Phase 22 + Phase 23 already cover the image supply-chain via base-image digest pinning + cosign + SLSA — image bytes are not subject to byte-equal verification because runtime image layers include immutable metadata that varies harmlessly between builds; tarball-only is the v1.6 reproducibility scope). It does NOT add a new sigstore action — the Phase 23 `sigstore-pin-check` job already greps every `.github/workflows/*` and will cover the new verifier workflow automatically (no new gate, no new pinned action). It does NOT change `release.yml`'s `check` job (`ubuntu-latest` stays — `check` runs tests, not the byte-equal artifact build; no determinism requirement). It does NOT cross-architecture: linux-amd64 stays the only target (per-architecture tarballs is a v1.7+ scope expansion if operator demand surfaces). The actual `reproducible-builds.org` registry submission (REPRO-04) is the maintainer's manual action AFTER the verifier has run green for ≥1 monthly cycle — Phase 25 ships the documented procedure, not the executed submission (mirrors Phase 24's PGP key generation pattern).

</domain>

<decisions>
## Implementation Decisions

### Binary determinism stack (release.yml `build` job)

- **D-01: Add `rust-toolchain.toml` at workspace root pinning the exact stable toolchain.** Conventional Rust pattern; `dtolnay/rust-toolchain` respects `rust-toolchain.toml` when present. Current resolved version at the time of this writing is `1.95.0` (`rustc 1.95.0 59807616e 2026-04-14` / `cargo 1.95.0`). The file gets a `[toolchain] channel = "1.95.0" profile = "minimal" components = ["rustfmt", "clippy"]` shape so dev + CI + verifier all resolve to the same toolchain. Planner: pin to whatever `rustc --version` reports at planning time on the maintainer's machine (use the value above unless planner finds a newer stable). The `dtolnay/rust-toolchain` step in `release.yml`/`ci.yml` keeps its existing SHA pin but its `with: toolchain:` input becomes `with:` (empty) — letting `rust-toolchain.toml` drive the version. Rejected: bare inline workflow pin without `rust-toolchain.toml` (loses single source of truth; contributors building locally would silently use a different toolchain). Rejected: `nightly-YYYY-MM-DD` (stable is sufficient for binary determinism; nightly volatility introduces unnecessary churn).

- **D-02: RUSTFLAGS = two `--remap-path-prefix` flags.** Set in the `build` job's `env:` block:
  ```yaml
  RUSTFLAGS: "--remap-path-prefix=${{ github.workspace }}=/build --remap-path-prefix=/home/runner/.cargo=/cargo"
  ```
  First remap strips the workspace path from the embedded debug info (`/home/runner/work/blindjoin/blindjoin` → `/build`). Second remap strips `CARGO_HOME` paths so dependency source paths embedded in panic messages and debug info don't vary by runner home (`/home/runner/.cargo/registry/src/...` → `/cargo/registry/src/...`). Rejected: single `--remap-path-prefix=$HOME=/h` (too coarse — also remaps `/home/runner/work`, ambiguous; multiple targeted remaps are clearer). Rejected: just stripping debug info entirely via `RUSTFLAGS=-C debuginfo=0` (loses debug info from the binary, useful for crash reports; D-07 `strip="symbols"` is the cleaner answer to the same concern).

- **D-03: SOURCE_DATE_EPOCH derived from the tagged commit time.** REPRO-02 locks the derivation: `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)`. Computed in a step BEFORE the `Build` step and exported to `$GITHUB_ENV` so subsequent steps inherit it. The same value is used for tar's `--mtime` flag in the `Package` step (D-06) so tarball entries inherit the commit time, not the runner wall-clock. The verifier (D-11) re-derives the same value from the tarball's source commit and gets byte-equal mtime entries.

- **D-04: `CARGO_INCREMENTAL=0` at job `env:` level.** Prevents incremental compilation from embedding host-specific intermediate paths in metadata. Set alongside `RUSTFLAGS` and `SOURCE_DATE_EPOCH` in the `build` job's `env:` block (NOT step-level — multiple steps need the same env). Auditor-grepable comment block above the `env:` block names all three determinism env vars + a 1-line citation of REPRO-01/02 + the Pitfall 6 long-tail expectation.

- **D-05: `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot`.** Adds explicit `--locked` (REPRO-02 requirement). `--locked` makes `cargo` refuse to update `Cargo.lock` during the build — fails the run if the lockfile is out of sync rather than silently regenerating. `--bins` order preserved from the current invocation. Comment cites REPRO-02 + the "fails-fast on stale Cargo.lock" rationale.

- **D-06: Deterministic `tar`+`gzip`.** Replace current `tar czf blindjoin-linux-amd64.tar.gz -C dist .` with:
  ```bash
  tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH}" \
      -cf - -C dist . \
    | gzip -n > blindjoin-linux-amd64.tar.gz
  ```
  Five flags load-bearing for byte-equality: `--sort=name` (deterministic file order across filesystem walk variance), `--owner=0 --group=0 --numeric-owner` (strip runner user identity), `--mtime="@${SOURCE_DATE_EPOCH}"` (uniform entry timestamps), and `gzip -n` (`-n` strips the original filename and mtime from the gzip header — the gzip default INCLUDES these and is the most common source of "tar bytes match, gzip bytes don't" puzzles in repro-build reports). Rejected: `tar czf` shorthand with separate `gzip` invocation — Bash pipeline is the conventional shape for `gzip -n`. Rejected: switching to `xz` or `zstd` — release operators have `tar xzf` muscle memory; gz stays.

- **D-07: `[profile.release] strip = "symbols"` in workspace `Cargo.toml`.** No `[profile.release]` block exists today. Adding `strip = "symbols"` (Rust 1.59+ feature, stable for years) removes debug symbol entries that vary harmlessly between builds and shrinks the binary ~30%. Belt-and-suspenders alongside `--remap-path-prefix` — strip removes most of what remap would have remapped. Workspace-level `[profile.release]` applies to all four crate binaries (coordinator, client, liquidity-bot, shared) uniformly. Rejected: per-crate `[profile.release]` (workspace-level is the conventional pattern for shared release profiles). Rejected: `strip = true` (synonym for `"debuginfo"`; explicit `"symbols"` strips more aggressively and is the documented reproducible-builds recommendation).

- **D-08: `release.yml` `build` job `runs-on: ubuntu-24.04` (NOT `ubuntu-latest`).** Roadmap SC#3 + Pitfall 7 lock this. The `check` job at [release.yml:34](.github/workflows/release.yml#L34) stays on `ubuntu-latest` (tests/clippy/audit have no byte-equal requirement; runner-image rotation is acceptable). The new `reproducible-verify.yml` also pins `ubuntu-24.04` (D-11). Comment cites Pitfall 7 + the "monthly verifier on a moving runner would false-positive every ~month" rationale.

### Documentation skeleton (docs/REPRODUCIBLE-BUILD.md)

- **D-09: `docs/REPRODUCIBLE-BUILD.md` H2 section list:**
  1. `## Why this exists` (3-sentence operator-facing intro: what reproducibility proves, why it matters, where the continuous verifier lives)
  2. `## Recipe` (the exact commands an external rebuilder runs)
  3. `## Toolchain pins` (table: component → exact version → file pin lives in)
  4. `## Environment` (`SOURCE_DATE_EPOCH` derivation, `RUSTFLAGS` literal, `CARGO_INCREMENTAL`)
  5. `## Expected sha256sum` (per-tag table — `v1.6.0: <TBD-v1.6.0-cut>` initially)
  6. `## Continuous verification` (cross-link to `reproducible-verify.yml` + the registry entry once D-14 lands)
  7. `## Reporting a reproducibility regression` (operators who hit a divergence file an issue; auto-detected divergences open `[reproducibility-regression]` automatically)
  Planner: write each section as prose + fenced bash recipes where applicable. The Recipe section is the load-bearing one — an external rebuilder copy-pastes it into a fresh `ubuntu-24.04` shell and gets a byte-equal tarball.

- **D-10: Expected sha256 placeholder is `<TBD-v1.6.0-cut>`.** Two-stage bootstrap: Phase 25 ships `docs/REPRODUCIBLE-BUILD.md` with `v1.6.0: <TBD-v1.6.0-cut>` in the table; the v1.6.0-rc.0 cut procedure (D-13) runs the verifier in `workflow_dispatch` mode against a freshly-rebuilt tarball, captures the resulting sha256, and the maintainer commits the replacement (`v1.6.0: <40-hex sha256>`) BEFORE flipping the release out of draft (if any draft remains — see D-14). The placeholder string is unambiguous (no chance of being mistaken for a real hash); a literal-byte grep gate in CI (optional, planner-discretion) ensures no release ships with a `<TBD-*>` token still in the doc. Rejected: deferring the hash entirely (bends REPRO-01 wording — roadmap mandates the doc names the sha256). Rejected: dispatch-then-commit-then-tag (more procedural steps; the placeholder approach is simpler and the doc is correctable in a follow-up commit if the rehearsal hash diverges from the tagged hash).

### Scheduled verifier (.github/workflows/reproducible-verify.yml)

- **D-11: New `.github/workflows/reproducible-verify.yml`, scheduled monthly + `workflow_dispatch`.**
  ```yaml
  on:
    schedule:
      - cron: '0 7 1 * *'   # 07:00 UTC on the 1st of each month
    workflow_dispatch:
  ```
  Single job `verify` on `runs-on: ubuntu-24.04` (NEVER `ubuntu-latest` — D-08). Steps:
  1. Capture runner image: `echo "VERIFIER_IMAGE_VERSION=${ImageVersion}" >> $GITHUB_ENV` (the GH-runner-provided env var; format like `20260520.1.0`).
  2. Resolve latest release tag: `LATEST_TAG=$(gh release view --json tagName --jq .tagName)`.
  3. Download release tarball + cosign bundle: `gh release download "$LATEST_TAG" --pattern 'blindjoin-linux-amd64.tar.gz*' --dir /tmp/rel`.
  4. Re-verify cosign signature on the downloaded tarball (Phase 24 SIGN-01 inheritance — the verifier proves cosign + byte-equality together; both must hold for a green run).
  5. Checkout the source at `$LATEST_TAG`, rebuild per the REPRO-01 recipe (`rust-toolchain.toml` resolves toolchain; `SOURCE_DATE_EPOCH` derived from tagged commit; `RUSTFLAGS` per D-02; tar/gzip per D-06).
  6. `EXPECTED=$(sha256sum /tmp/rel/blindjoin-linux-amd64.tar.gz | cut -d' ' -f1)` + `ACTUAL=$(sha256sum blindjoin-linux-amd64.tar.gz | cut -d' ' -f1)`.
  7. Compare against the value in `docs/REPRODUCIBLE-BUILD.md` for the tagged release (parse the expected-sha256 table). On mismatch: open an issue per D-12.
  Permissions block: `contents: read`, `issues: write` (for the regression-issue open). Auditor-grepable comment cites the deliberately-omitted scopes (`id-token`, `attestations`, `packages`, `pull-requests`, `pages` — the verifier neither signs nor pushes).

- **D-12: Two-title `[reproducibility-regression]` issue scheme + title-dedup.** Two failure-mode classes, two distinct titles, dedup by exact title-match (same pattern as Phase 22 `[digest-drift]`):
  - **Runner-image drift (low-severity):** If `${ImageVersion}` differs from the value pinned in `docs/REPRODUCIBLE-BUILD.md` (D-09 §Toolchain pins): open issue titled `[reproducibility-regression] runner image drift: ImageVersion <OLD> → <NEW>`. Body: "GitHub rotated the `ubuntu-24.04` runner image SHA. Verify reproducibility on the new image; update `docs/REPRODUCIBLE-BUILD.md` with the new ImageVersion if green; investigate if not. Not a supply-chain signal until investigated."
  - **Actual divergence (HIGH-severity):** If `${ImageVersion}` matches but `sha256sum` differs: open issue titled `[reproducibility-regression] sha256 mismatch on ImageVersion <V>`. Body: "Rebuilt tarball diverges from published `<LATEST_TAG>` on the SAME `ubuntu-24.04` ImageVersion `<V>`. This is a real supply-chain signal — published release does not reproduce from source. Compare diffoscope output; suspect tampering, compromised CI, or undocumented build-env drift."
  Dedup is title-exact: `gh issue list --search 'in:title "<exact-title>" is:open'`; skip create if a matching open issue exists. Pattern lifted verbatim from Phase 22 Plan 22-02 digest-drift idempotency. Rejected: comment-on-existing-issue dedup (adds noise on chronic regressions; title-skip is cleaner). Rejected: single-title-with-body-classification (loses at-a-glance severity in the issue list; auditors scanning issue titles get less signal).

### release.yml orphan-draft cleanup (folded into determinism plan)

- **D-13: Remove `draft: true` from `release.yml` softprops step + rewrite the surrounding comment block.** Phase 24 D-07 added `draft: true` so the maintainer could upload `.asc` post-CI. SIGN-03 (PGP path) was deferred indefinitely on 2026-06-02 (per recent commits f11d544 and f1f3a50). The `draft: true` is now functionally orphaned — no PGP upload procedure executes, no maintainer-flip happens, and the verifier's `gh release download` would need `--draft` + `GITHUB_TOKEN` indirection that would be artificial if the release just published directly. Phase 25 folds this cleanup into the same `release.yml` PLAN that lands D-01..D-08 (single coherent diff; ~10 lines). The comment block above the softprops step is rewritten: drop the "until the maintainer uploads the .asc" prose; reference D-13 + the 2026-06-02 SIGN-03 deferral instead. The `--draft=false` flip step in `docs/RELEASING.md` (if still present) is also removed in the same plan. After this, every tag push produces a published release with cosign + SLSA + sha256 assets; the verifier downloads via plain `gh release download` (no auth needed for public releases). Rejected: split into its own PLAN (Option C from discussion) — chose Option A: cleanup is tightly coupled to the verifier's download path and to the determinism env vars added in the same job, so single PLAN keeps the diff coherent. Rejected: leave as-is and use `--draft` flag in the verifier (Option B) — adds artificial auth coupling for a stale Phase 24 artifact; the cleanup IS the right answer.

### REPRO-04 registry submission procedure

- **D-14: Manual maintainer procedure in `docs/RELEASING.md`.** Mirror Phase 24's PGP-key-generation pattern: Phase 25 ships the documented procedure, not the executed submission. New section in `docs/RELEASING.md` titled `## Reproducible-builds.org registry submission` covers: (a) prerequisite — at least 1 green monthly `reproducible-verify.yml` run after a tagged release (verifiable via the workflow's Actions tab); (b) registry submission steps — fork [reproducible-builds.org/projects](https://reproducible-builds.org/projects/), add an entry per the project's existing schema (name, language, reproducibility tooling, verification command link), open a PR, wait for review; (c) after merge, link the registry entry from `docs/REPRODUCIBLE-BUILD.md` §Continuous verification + from `SECURITY.md` §Supply-chain status (D-19). The actual submission is the maintainer's action after observing one green cycle (≥30 days post v1.6.0 release). Rejected: scripted/CI-driven submission (registry submissions are human-reviewed PRs by a third-party project; not automatable). Rejected: separate doc file (`docs/REGISTRY-SUBMISSION.md`) — single ~15-line procedure fits naturally into `docs/RELEASING.md` alongside other maintainer-side procedures.

### Claude's Discretion (planner figures these out, guided by research + this CONTEXT)

- **D-15: Exact `rust-toolchain.toml` channel value.** Pin to whatever `rustc --version` reports at planning time on the maintainer's machine (currently `1.95.0`). If a newer stable has shipped between this CONTEXT and the PLAN, use the newer value. Document the pinning decision in `docs/REPRODUCIBLE-BUILD.md` §Toolchain pins with a 1-liner on the bump policy (planner may also choose to defer bump-policy prose to v1.7+ when the first toolchain bump happens — Phase 25 doesn't need to predict that).

- **D-16: Cron schedule for `reproducible-verify.yml`.** D-11 suggests `0 7 1 * *` (07:00 UTC, 1st of month). Planner: pick a slot that doesn't collide with the daily `digest-drift-check.yml` schedule (Phase 22) — staggering the schedules avoids same-day GitHub-runner-quota contention. Phase 22's `digest-drift-check.yml` runs at... planner confirms via grep at planning time.

- **D-17: `docs/REPRODUCIBLE-BUILD.md` exact prose + Recipe section copy-paste shape.** Planner writes the Recipe section as a single fenced bash block that an external rebuilder can copy verbatim. The Recipe MUST include: `git clone https://github.com/<owner>/blindjoin.git`, `git checkout v1.6.0`, exact env exports (`SOURCE_DATE_EPOCH`, `RUSTFLAGS`, `CARGO_INCREMENTAL`), exact `cargo build` invocation, exact `tar`/`gzip` pipeline (D-06), exact `sha256sum` comparison. Planner: ensure the bash block runs end-to-end on a fresh `ubuntu-24.04` shell with no project-specific prior state.

- **D-18: Verifier's expected-sha256 lookup mechanism.** D-09 §5 puts the expected hash in a markdown table. Planner picks the parse strategy: `awk '/^\| v1\.6\.0/ {print $4}' docs/REPRODUCIBLE-BUILD.md` or equivalent. Brittle if the table format changes — planner may prefer a separate `.expected-sha256` file alongside or under `docs/` that the verifier reads directly (line-by-line `tag:sha256` pairs). Either shape is fine; planner picks whichever is cleaner. Whatever shape is chosen, the maintainer's v1.6.0-rc.0 procedure (D-10) updates the same source.

- **D-19: SECURITY.md §Supply-chain status cross-link.** Phase 23 + Phase 24 established a `## Supply-chain status` section with image and tarball signing subsections. Phase 25 adds a third short subsection `### Reproducibility (v1.6 onward)` that names: (a) reproducible build recipe at `docs/REPRODUCIBLE-BUILD.md`; (b) continuous monthly verification via `.github/workflows/reproducible-verify.yml`; (c) registry entry at `reproducible-builds.org/projects/blindjoin` (filled in after D-14 lands). Light touch — 1 paragraph + 1 fenced "how to verify yourself" command block (which is the same Recipe block as REPRODUCIBLE-BUILD.md). Planner: confirm the existing section structure at planning time; if Phase 24's PGP-deletion has left the section structurally different than this CONTEXT assumes, adjust.

- **D-20: Comment-block style for new `env:` block in `release.yml` build job.** Follow the Phase 22 / Phase 23 / Phase 24 auditor-grepable pattern: prose comment block ABOVE the `env:` block naming the three determinism vars + a 1-line citation of REPRO-01/02 + a 1-line citation of Pitfall 6 (Rust reproducibility long tail) + a 1-line citation of D-03 (SOURCE_DATE_EPOCH derivation). The new `Compute SOURCE_DATE_EPOCH` step's comment block cites REPRO-02 verbatim.

- **D-21: `dtolnay/rust-toolchain` `with:` after `rust-toolchain.toml` lands.** D-01 says `rust-toolchain.toml` drives the version; the existing `with: toolchain: stable` in `release.yml` lines 39 + 90-91 and in `ci.yml` (8 jobs) should be removed so the file takes precedence. Planner: do the removal in the same plan that lands D-01, NOT a separate plan — single coherent diff per file. If a job needs a different toolchain than the workspace default (none currently does), it would still set `with: toolchain:` explicitly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase contract (locked WHAT)
- `.planning/REQUIREMENTS.md` §Category 3 — REPRO-01, REPRO-02, REPRO-03, REPRO-04 verbatim text. REPRO-01 names the `docs/REPRODUCIBLE-BUILD.md` contents (toolchain version + runner image + cargo invocation + env vars + expected sha256sum). REPRO-02 locks `cargo build --release --locked` + `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)` + `RUSTFLAGS=--remap-path-prefix=...` + `CARGO_INCREMENTAL=0`. REPRO-03 mandates `runs-on: ubuntu-24.04` (NOT `ubuntu-latest`) + monthly cron + workflow_dispatch + sha256 assertion + `[reproducibility-regression]` issue on mismatch + drift-vs-real-divergence distinction. REPRO-04 conditions on REPRO-01 + REPRO-03 being green for ≥1 monthly cycle before reproducible-builds.org submission.
- `.planning/ROADMAP.md` §Phase 25 — 4 numbered Success Criteria, all verbatim mappings of REPRO-01..04. SC#3 explicitly names `[reproducibility-regression]` issue title + the drift-vs-real-divergence message distinction.

### Threat-model + design context (Pitfalls Phase 25 inherits)
- `.planning/research/PITFALLS.md` §Pitfall 6 — Rust reproducibility long tail. `--remap-path-prefix` + `SOURCE_DATE_EPOCH` are necessary but rarely sufficient on the first try; expect 1-2 iteration cycles after Phase 25's initial ship to surface project-specific nondeterminism (e.g., `incremental` artifacts, env-leaked locale/timezone, `rand` seeding in tests that touch `RUSTFLAGS`). The verifier IS the iteration mechanism — its first failures inform the next prevention cycle.
- `.planning/research/PITFALLS.md` §Pitfall 7 — Verifier false-positives on `ubuntu-latest` rotation. Locked: `ubuntu-24.04` pin in both `release.yml` `build` job (D-08) and `reproducible-verify.yml` (D-11). D-12 names the two-title scheme that surfaces the GitHub-side image rotation distinctly from a real supply-chain signal.
- `.planning/research/PITFALLS.md` §Pitfall 11 — Auto-merging supply-chain bumps. The `[reproducibility-regression]` issue → human-investigation → human-PR path mirrors the Phase 22 `[digest-drift]` policy. D-12's two-title dedup-by-title-match lifts the Phase 22 Plan 22-02 idempotency pattern.
- `.planning/research/PITFALLS.md` §Pitfall 12 — Fresh-machine UAT every documented command. The Recipe section in `docs/REPRODUCIBLE-BUILD.md` (D-17) MUST run end-to-end on a clean `ubuntu-24.04` shell. The verifier IS the perpetual fresh-machine UAT; the v1.6.0-rc.0 maintainer rehearsal (D-10) is the one-time fresh-machine UAT for the recipe doc itself.
- `.planning/research/PITFALLS.md` §Pitfall 13 — cosign 3.0 CLI flag drift. The verifier re-runs `cosign verify-blob` (D-11 step 4); Phase 23's SECURITY.md callout on cosign version range covers the operator-facing recipe. The verifier itself uses `sigstore/cosign-installer` (D-11) pinned to the same SHA as Phase 23/24 — single source of truth.
- `.planning/research/SUMMARY.md` — phase mapping (Phase 22 → Phase 23 → Phase 24 → Phase 25); Phase 25 closes the v1.6 reproducibility surface and is the final phase in the milestone.
- `.planning/research/STACK.md` — cosign version range, sigstore action versions, `actions/attest-build-provenance` version. All inherited from Phase 23/24.

### Predecessor phase patterns (this phase MUST mirror these)
- `.planning/phases/22-base-image-digest-drift-detection/22-CONTEXT.md` — issue-not-PR scheduled-workflow pattern; `[digest-drift]` title format that D-12's `[reproducibility-regression]` mirrors verbatim; dedup-by-title-match (`gh issue list --search` skip-create); auditor-grepable "deliberately-omitted scopes" comment style for the verifier's permissions block.
- `.planning/phases/22-base-image-digest-drift-detection/22-04-PLAN.md` — comments-as-contract paraphrasing discipline. Phase 25 follows: any token forbidden by file-level grep (e.g., `ubuntu-latest`) must NOT appear in the file even inside comments.
- `.planning/phases/23-cosign-image-attestations-slsa-provenance-sbom/23-CONTEXT.md` — sigstore SHA-pin reuse pattern (D-13/D-21 inheritance); `sigstore-pin-check` job at `ci.yml:292-326` greps every `.github/workflows/*` — covers the new `reproducible-verify.yml` automatically. Phase 25 adds zero new sigstore actions; reuses Phase 23 pins verbatim.
- `.planning/phases/24-release-tarball-signing-cosign-slsa-pgp/24-CONTEXT.md` — D-01 (inline-in-existing-build-job) pattern that D-08+D-13 follow; D-15 files-list ordering convention; the SIGN-03 PGP-deferral context that motivates D-13's orphan-`draft:true` cleanup.
- `.planning/phases/24-release-tarball-signing-cosign-slsa-pgp/24-01-SUMMARY.md` — the as-shipped `release.yml` `build` job structure that Phase 25 modifies. Names the exact comment-block style for `id-token: write` + `attestations: write` that Phase 25's `env:` block mirrors.

### Existing pin discipline + integration surface
- `.github/workflows/release.yml` — `build` job at [release.yml:60-223](.github/workflows/release.yml#L60). `if: startsWith(github.ref, 'refs/tags/')` gate at [release.yml:66](.github/workflows/release.yml#L66) preserved. Existing permissions block at [release.yml:82-85](.github/workflows/release.yml#L82) (contents/id-token/attestations) preserved. Phase 22 `read-base-digests` composite at [release.yml:103-105](.github/workflows/release.yml#L103) preserved. Phase 24 cosign + SLSA steps at [release.yml:127-199](.github/workflows/release.yml#L127) preserved. Phase 25 modifies: (a) `runs-on: ubuntu-latest` → `ubuntu-24.04` at line 63 (D-08); (b) adds `env:` block to the `build` job with `RUSTFLAGS` + `CARGO_INCREMENTAL=0` (D-02/D-04); (c) adds a `Compute SOURCE_DATE_EPOCH` step before `Build` (D-03); (d) modifies the `Build` step's `cargo build` invocation to add `--locked` (D-05); (e) replaces the `Package` step's `tar czf` with the deterministic pipeline (D-06); (f) removes `draft: true` from the softprops step at [release.yml:215](.github/workflows/release.yml#L215) (D-13); (g) rewrites the comment block above the softprops step to drop the PGP-flip prose.
- `.github/workflows/ci.yml` — `sigstore-pin-check` job already greps every workflow file; covers `reproducible-verify.yml` for free. No new CI gate needed. The 8 `runs-on: ubuntu-latest` jobs in `ci.yml` are NOT changed (CI doesn't produce the byte-equal artifact).
- `.github/workflows/docker.yml` — unchanged.
- `.github/workflows/digest-drift-check.yml` — Phase 22's scheduled workflow; structural template for D-11's scheduled verifier (cron + workflow_dispatch + single job + issue-creation steps). Planner reads this file to mirror the cron-stagger choice (D-16) and the `gh issue list --search` dedup logic (D-12).
- `Cargo.toml` (workspace root) — `[workspace.dependencies]` block at lines 13-37. Phase 25 ADDS a new `[profile.release]` block (D-07: `strip = "symbols"`). No existing `[profile.release]` block to merge into.

### Policy + operator-facing docs (D-09 + D-14 + D-19 land here)
- `docs/REPRODUCIBLE-BUILD.md` — NEW FILE; H2 skeleton per D-09. Operator-facing recipe; updates v1.6.0-rc.0+ with the expected sha256 per D-10.
- `docs/RELEASING.md` — exists from Phase 24. Phase 25 ADDS a `## Reproducibility verification rehearsal` subsection (the v1.6.0-rc.0 procedure for capturing the expected sha256 per D-10) + a `## Reproducible-builds.org registry submission` subsection (D-14 procedure). If `docs/RELEASING.md` still references the `--draft=false` flip from Phase 24 D-07, that reference is removed in the same change per D-13.
- `SECURITY.md` `## Supply-chain status` — adds a third subsection `### Reproducibility (v1.6 onward)` per D-19. Light touch: 1 paragraph + 1 fenced verify-yourself command block.
- `.github/workflows/reproducible-verify.yml` — NEW FILE; D-11 structure.
- `rust-toolchain.toml` — NEW FILE at workspace root; D-01 / D-15.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`release.yml` build job structure** ([release.yml:60-223](.github/workflows/release.yml#L60)) — single-job artifact builder, already tag-gated, already has cosign + SLSA from Phase 24. Phase 25 modifies env + steps; structure (one job, sequential steps) preserved. The job-level `permissions:` block (id-token/attestations from Phase 24) stays; no new permissions needed for determinism (the verifier in the new workflow gets its own narrower permissions: `contents: read` + `issues: write`).
- **Phase 22 `read-base-digests` composite action** ([release.yml:103-105](.github/workflows/release.yml#L103)) — already wired; reads `docker/digests.txt`. Phase 25 doesn't touch the composite but inherits its presence as evidence that supply-chain manifests live in source. The `[reproducibility-regression]` issue body in D-12 can cross-link to `docker/digests.txt` as one of the "things that didn't drift" when the rebuild diverges.
- **Phase 22 `digest-drift-check.yml`** (existing scheduled workflow) — template for D-11. Cron schedule, single-job-on-ubuntu-latest (Phase 22 chose `ubuntu-latest` because docker manifests don't need byte-equality; Phase 25 reverses for `ubuntu-24.04` per D-08). Issue-open-with-dedup pattern is the direct inheritance.
- **Phase 23 `sigstore-pin-check` job** (in `ci.yml`) — already greps all of `.github/workflows/*` for sigstore actions. The new `reproducible-verify.yml`'s `sigstore/cosign-installer` step inherits this gate automatically.
- **Phase 24 cosign-installer SHA pin + cosign-release version** ([release.yml:127-130](.github/workflows/release.yml#L127)) — `sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` + `cosign-release: 'v2.6.3'`. D-11 step 4 (re-verify cosign on downloaded tarball) reuses these exact pins verbatim; single source of truth for the sigstore vocabulary.
- **Phase 24 softprops/action-gh-release SHA pin** ([release.yml:213](.github/workflows/release.yml#L213)) — `softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1`. Already SHA-pinned. D-13 removes `draft: true` from the `with:` block; the pin stays.

### Established Patterns
- **Comments-as-contract above structural blocks** — every workflow file has prose comments above `env:` / `on:` / `permissions:` / `jobs:` / new steps. Auditor-grepable per Plan 22-04 — paraphrase forbidden tokens so file-level greps stay green. D-08 + D-20 inherit; the comment block above the new `env:` block in `release.yml` must NOT contain the literal `ubuntu-latest` token even when explaining why `ubuntu-24.04` is locked.
- **SHA-pin trailing-comment style** — `@<40-hex> # vX.Y.Z`. New uses in `reproducible-verify.yml` (cosign-installer, gh-action equivalents) MUST follow. Phase 23 `sigstore-pin-check` enforces at file level.
- **`if: startsWith(github.ref, 'refs/tags/')` tag-gate** — load-bearing for `release.yml` `build` job. Phase 25 preserves; the determinism env vars only matter on real tag pushes (where the released tarball gets the locked hash).
- **Workflow-level vs job-level permissions** — Pitfall 2 + Phase 23 D-02 + Phase 24 D-02: ALWAYS job-level for narrower scope. New `reproducible-verify.yml` follows: workflow-level `permissions:` block stays default (read-all); the single `verify` job grows an explicit `permissions: { contents: read, issues: write }` block + auditor-grepable "deliberately-omitted-scopes" comment.
- **Issue title format `[<category>] <subject>`** — Phase 22's `[digest-drift] <image>:<tag> moved to sha256:<HEX>`. Phase 25's `[reproducibility-regression] runner image drift: ImageVersion <OLD> → <NEW>` (low-severity) + `[reproducibility-regression] sha256 mismatch on ImageVersion <V>` (HIGH-severity) per D-12.

### Integration Points
- **`Cargo.toml` workspace root** — add `[profile.release]` block (D-07). Currently has `[workspace]` + `[workspace.dependencies]` only; no `[profile.*]` blocks. Insert after `[workspace.dependencies]`.
- **`release.yml` `build` job** — line-level changes per D-01..D-08 + D-13. Single coherent diff. Plan: one PLAN.md for `release.yml` modifications + `Cargo.toml` + `rust-toolchain.toml`.
- **NEW `.github/workflows/reproducible-verify.yml`** — single-file PLAN for D-11 + D-12.
- **NEW `docs/REPRODUCIBLE-BUILD.md`** — single-file PLAN for D-09 + D-10 (skeleton + placeholder).
- **`docs/RELEASING.md`** — single PLAN for D-14 (registry submission procedure) + D-10 (verifier rehearsal procedure). Existing file from Phase 24; APPEND new sections, don't restructure.
- **`SECURITY.md` `## Supply-chain status`** — single tiny PLAN (or fold into the REPRODUCIBLE-BUILD.md plan) for D-19 cross-link. Light touch.
- **`ci.yml`** — NOT modified (the 8 `ubuntu-latest` jobs stay; determinism is build-only, not test-only). Verify at planning time that no `with: toolchain:` overrides need adjustment after D-01.

</code_context>

<specifics>
## Specific Ideas

- **`rust-toolchain.toml` content (D-01).** RECOMMENDED:
  ```toml
  [toolchain]
  channel = "1.95.0"
  profile = "minimal"
  components = ["rustfmt", "clippy"]
  ```
  Profile `minimal` keeps the install lean on the runner (no docs/examples). `rustfmt` + `clippy` required because `ci.yml` clippy job + dev `cargo fmt` workflow need them. Planner: bump `1.95.0` to current stable at planning time if a newer version has shipped.

- **`release.yml` `build` job `env:` block (D-02 + D-04 + comment per D-20).** RECOMMENDED:
  ```yaml
  # Phase 25 REPRO-01/02: binary determinism env vars. The three flags below
  # make `cargo build --release --locked` produce byte-equal output across
  # rebuilds on the pinned ubuntu-24.04 runner image.
  #
  # RUSTFLAGS:           Two --remap-path-prefix flags strip embedded build-host
  #                      paths from debug info and panic messages. Without
  #                      these, rebuilds on a different runner produce diverging
  #                      bytes even on identical toolchain. See REPRO-01.
  # SOURCE_DATE_EPOCH:   Computed per REPRO-02 in the Compute step below;
  #                      env entry here is a no-op marker for auditor visibility.
  # CARGO_INCREMENTAL:   0 disables incremental compilation entirely, which
  #                      otherwise embeds host-specific intermediate paths in
  #                      metadata. See REPRO-01.
  #
  # Pitfall 6 (research): expect 1-2 iteration cycles after the first ship to
  # surface project-specific nondeterminism. The reproducible-verify.yml
  # workflow is the iteration mechanism — failures inform the next prevention.
  env:
    RUSTFLAGS: "--remap-path-prefix=${{ github.workspace }}=/build --remap-path-prefix=/home/runner/.cargo=/cargo"
    CARGO_INCREMENTAL: "0"
  ```

- **`Compute SOURCE_DATE_EPOCH` step (D-03).** RECOMMENDED — runs after `Read canonical base-image digests`, before `Build`:
  ```yaml
  # Phase 25 REPRO-02: derive SOURCE_DATE_EPOCH from the tagged commit time.
  # Locked to git's recorded committer-time on $GITHUB_SHA so the value is
  # reproducible from source — an external rebuilder running `git log -1
  # --format=%ct v1.6.0` on a fresh clone gets the same epoch, the same
  # debug-info baseline, and the same tar entry mtimes (per D-06).
  - name: Compute SOURCE_DATE_EPOCH from tagged commit time
    run: echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)" >> $GITHUB_ENV
  ```

- **`Package` step (D-06) — deterministic tar/gzip pipeline.** RECOMMENDED:
  ```yaml
  # Phase 25 REPRO-01: deterministic tar + gzip. Five flags load-bearing for
  # byte-equality: --sort=name (deterministic file order), --owner/--group=0
  # --numeric-owner (strip runner user identity), --mtime="@$SOURCE_DATE_EPOCH"
  # (uniform timestamps from D-03), and `gzip -n` (strip filename + mtime from
  # the gzip header — default gzip embeds wall-clock and original path, which
  # is the most common "tar matches, gzip doesn't" failure mode).
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

- **`reproducible-verify.yml` `permissions:` block.** RECOMMENDED:
  ```yaml
  # Phase 25 REPRO-03: verifier needs read-only access to the source +
  # write access to open [reproducibility-regression] issues. NOTHING else.
  # Deliberately omitted (auditor-grepable): id-token (verifier neither signs
  # nor pushes attestations — it only re-verifies the cosign sig on the
  # downloaded tarball, which is a read-only operation), attestations,
  # packages, pull-requests, pages, deployments. These tokens MUST NOT appear
  # anywhere in this file at any indentation.
  permissions:
    contents: read
    issues: write
  ```

- **`docs/REPRODUCIBLE-BUILD.md` Recipe section (D-17).** RECOMMENDED — fenced bash block an external rebuilder copy-pastes verbatim:
  ```bash
  # On a fresh ubuntu-24.04 runner image (or VM):
  git clone https://github.com/<owner>/blindjoin.git
  cd blindjoin
  git checkout v1.6.0   # or whatever tag you're verifying
  # Toolchain pin resolved by rust-toolchain.toml at repo root.
  # Determinism env vars per REPRO-01:
  export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)
  export RUSTFLAGS="--remap-path-prefix=$(pwd)=/build --remap-path-prefix=$HOME/.cargo=/cargo"
  export CARGO_INCREMENTAL=0
  # Build:
  cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot
  # Package (deterministic):
  mkdir -p dist
  cp target/release/coordinator target/release/client target/release/liquidity-bot dist/
  tar --sort=name --owner=0 --group=0 --numeric-owner \
      --mtime="@${SOURCE_DATE_EPOCH}" \
      -cf - -C dist . \
    | gzip -n > blindjoin-linux-amd64.tar.gz
  # Compare against the expected hash in this doc:
  sha256sum blindjoin-linux-amd64.tar.gz
  ```

- **`[reproducibility-regression]` issue title formats (D-12).** RECOMMENDED:
  - Low-severity: `[reproducibility-regression] runner image drift: ImageVersion 20260520.1.0 → 20260620.2.1`
  - HIGH-severity: `[reproducibility-regression] sha256 mismatch on ImageVersion 20260520.1.0`
  Both titles encode the ImageVersion so dedup is exact-title-match and the issue list reads at a glance.

- **`docs/RELEASING.md` REPRO-04 procedure skeleton (D-14).** RECOMMENDED 4-step:
  1. Verify `.github/workflows/reproducible-verify.yml` has at least one green monthly run after the v1.6.0 tag (Actions tab; filter by workflow name).
  2. Fork `github.com/reproducible-builds/reproducible-builds.org`; add an entry under `_data/projects/` per the project's existing YAML schema (name: blindjoin; language: Rust; reproducibility doc URL: link to `docs/REPRODUCIBLE-BUILD.md`; verify command: copy from the Recipe section).
  3. Open a PR; respond to reviewer feedback.
  4. After merge, link the registry entry from `docs/REPRODUCIBLE-BUILD.md` §Continuous verification AND from `SECURITY.md` §Supply-chain status §Reproducibility.

- **`SECURITY.md` §Reproducibility subsection (D-19).** RECOMMENDED 1-paragraph + 1-recipe shape:
  ```markdown
  ### Reproducibility (v1.6 onward)

  Every tagged release tarball is reproducible byte-for-byte from source on the
  pinned `ubuntu-24.04` runner image. The full recipe, expected `sha256sum` per
  release, and toolchain pins live in [docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md).
  Continuous verification runs monthly via
  [.github/workflows/reproducible-verify.yml](.github/workflows/reproducible-verify.yml);
  a failure opens a `[reproducibility-regression]` issue. blindjoin is registered
  with the reproducible-builds.org project registry: <link added after D-14 lands>.

  To verify a release yourself, follow the Recipe section of REPRODUCIBLE-BUILD.md.
  ```

</specifics>

<deferred>
## Deferred Ideas

- **Per-architecture reproducibility (linux-arm64, darwin-amd64, etc).** Current release is `linux-amd64` only. Multi-arch would multiply the recipe + verifier matrix; out of scope for Phase 25. Tracked as a v1.7+ scope expansion if operator demand surfaces.
- **`diffoscope` integration on verifier mismatch.** When the verifier opens a `sha256 mismatch on ImageVersion <V>` (HIGH-severity) issue, attaching a `diffoscope` summary of the two tarballs would accelerate triage. Deferred: `diffoscope` installation alone is ~150MB and adds ~2min to the runner. Defer until the first real divergence — at that point, a one-shot manual `diffoscope` run from the maintainer's machine on the two tarballs is sufficient; if a second divergence happens, automate.
- **Bump-policy prose for `rust-toolchain.toml`.** D-15 leaves the bump policy unwritten ("future-planner predicts that"). When the first stable Rust bump happens after v1.6.0 ships and the verifier flags a real divergence due to toolchain rotation, document the bump → verify → re-pin loop in `docs/REPRODUCIBLE-BUILD.md` §Toolchain pins. Until that real moment, prose would be speculative.
- **Reproducibility for the GHCR images themselves.** Phase 22 + Phase 23 cover image supply-chain via base-image digest pinning + cosign + SLSA. True byte-equal reproducibility of OCI layers (deterministic-tar + `--build-arg SOURCE_DATE_EPOCH` propagation through `cargo-chef` + buildkit) is harder than tarball reproducibility because image layers embed buildkit metadata that varies by runner state. Defer to v1.7+ or beyond — image attestations already give operators the supply-chain signal they need; byte-equal image layers is a stretch goal.
- **`workflow_run` trigger on `release.yml` success.** Alternative to monthly cron: trigger the verifier IMMEDIATELY after a release.yml run completes successfully. Deferred: monthly cron is the contract REPRO-03 names; immediate-trigger adds value (catches the regression within hours instead of weeks) but adds complexity (`workflow_run` requires careful permissions handling; the verifier would need to dedupe against the cron run). Quick task for v1.7 if the maintainer wants tighter feedback.
- **A `[reproducibility-success]` issue on green monthly runs.** Could provide ongoing audit trail. Rejected as noise: a green run is the absence of news; the workflow's Actions tab already provides the audit trail; an issue per success would clutter the issue tracker. Registry submission (D-14) is the canonical "we are reproducible" signal.
- **Severity classification on `[reproducibility-regression]` (subdivide HIGH).** D-12 has two titles (low/HIGH). Could subdivide HIGH into "deterministic divergence" vs "non-deterministic divergence" (running the verifier twice on the same runner with the same source). Defer: requires running the verifier twice per scheduled cycle (2x cost); single-run HIGH severity is sufficient for the first cycle of operation; subdivide only if false-positive triage workload demands it.
- **`reproducible-builds.org` SBOM-comparison submission.** The registry accepts richer integrations than the basic "verify command + reproducibility doc" submission D-14 describes (e.g., SBOM-equivalence checks across builds). Deferred to v1.7+; Phase 25's basic registration is the milestone goal.

</deferred>

---

*Phase: 25-reproducible-build-recipe-scheduled-verifier-registry*
*Context gathered: 2026-06-02*
</content>
</invoke>