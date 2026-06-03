# Phase 25 Discussion Log

**Phase:** 25 — Reproducible-Build Recipe + Scheduled Verifier + Registry
**Discussed:** 2026-06-02
**Mode:** discuss (default)

## Context loaded

- `.planning/PROJECT.md` — v1.6 Supply-Chain Attestation milestone, blindjoin core value, constraints (no custom crypto, Tor-native, MIT)
- `.planning/STATE.md` — Phase 24 closed 2026-06-02 at cosign+SLSA scope; SIGN-03 (PGP path) deferred indefinitely; v1.6 3 of 4 phases shipped
- `.planning/REQUIREMENTS.md` — REPRO-01..04 (the locked phase contract)
- `.planning/ROADMAP.md` §Phase 25 — 4 Success Criteria (verbatim mappings of REPRO-01..04)
- `.planning/research/SUMMARY.md` — v1.6 phase mapping; Phase 25 is the final phase in milestone
- `.planning/research/PITFALLS.md` §Pitfall 6 (Rust reproducibility long tail), §Pitfall 7 (ubuntu-latest rotation), §Pitfall 11 (issue-not-PR), §Pitfall 12 (fresh-machine UAT), §Pitfall 13 (cosign 3.0 CLI drift)
- `.planning/phases/24-*/24-CONTEXT.md` — predecessor patterns (inline-in-existing-job, comments-as-contract, sigstore-pin reuse, SIGN-03 deferral context)
- `.planning/phases/22-*/22-CONTEXT.md` — issue-not-PR pattern, `[digest-drift]` title-dedup pattern that `[reproducibility-regression]` mirrors
- `.github/workflows/release.yml` — current `build` job state (after Phase 24 ship): cosign + SLSA steps present, `draft: true` orphaned after SIGN-03 deferral, `runs-on: ubuntu-latest` at line 63 (needs `ubuntu-24.04` per Pitfall 7)
- `Cargo.toml` — no `[profile.release]` block present; no `rust-toolchain.toml` at workspace root
- `rustc --version` output: `rustc 1.95.0 (59807616e 2026-04-14)` / `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` — captured for D-15

## Gray areas presented

Four gray areas surfaced after loading context. Phase 25 has a tightly-specified roadmap (REPRO-01..04 verbatim acceptance criteria); the discussion focused on implementation choices the roadmap deliberately left open.

### 1. Binary determinism stack

**Options presented:**
- Full reproducibility kit (Recommended) — `rust-toolchain.toml` + dual `--remap-path-prefix` + deterministic tar flags + `gzip -n` + `strip = "symbols"`
- Workflow-pinned, no rust-toolchain.toml — bump inline workflow pin only
- Minimal kit (REPRO-01 letter only) — single `--remap-path-prefix`, no strip, no tar flags, iterate on verifier failures

**User selected:** Full reproducibility kit
**→ Decisions:** D-01, D-02, D-06, D-07

### 2. Expected sha256sum chicken-and-egg

**Options presented:**
- Two-stage: placeholder in doc, v1.6.0-rc.0 cut fills it (Recommended)
- Compute-and-anchor: dispatch verifier produces hash, commit it, then tag
- Defer hash entirely: doc names recipe only, verifier compares against published hash

**User selected:** Two-stage with placeholder
**→ Decisions:** D-10, D-09 §Expected sha256sum table, docs/RELEASING.md verifier-rehearsal procedure (D-10 + D-14 surface)

### 3. Scheduled verifier mismatch triage logic

**Options presented:**
- Capture $ImageVersion env + dedup by title (Recommended) — two distinct issue titles, skip-on-existing-title-match dedup
- Always-open-issue, body explains classification — single title, body distinguishes drift vs real
- Two-issue-title scheme + comment dedup — same two titles, but COMMENT on existing instead of skip

**User selected:** Two-title scheme + skip-dedup (mirror Phase 22 `[digest-drift]` pattern)
**→ Decisions:** D-11 step 7, D-12

### 4. Orphan `draft: true` from Phase 24

**Options presented:**
- In-scope cleanup: remove `draft: true` + revert D-07 comments (Recommended) — fold into Phase 25's release.yml plan
- Out of scope: verifier uses `gh release download --draft` flag — leave Phase 24 state intact
- Split: cleanup as separate Phase 25 plan — same code change, but isolated PLAN file

**User selected:** In-scope cleanup, folded into same plan as determinism env vars
**→ Decisions:** D-13

## Deferred ideas captured

(See `25-CONTEXT.md` `<deferred>` section for full list.)

Phase-25-relevant deferrals:
- Per-architecture reproducibility (multi-arch tarball matrix) → v1.7+
- `diffoscope` integration on mismatch → after first real divergence
- `rust-toolchain.toml` bump-policy prose → after first real toolchain bump
- Reproducibility for GHCR images → v1.7+ or beyond
- `workflow_run`-trigger verifier (immediate, not monthly) → quick task at v1.7
- `[reproducibility-success]` issue on green runs → rejected as noise
- Severity subdivision on `[reproducibility-regression]` → after first false-positive workload signal
- `reproducible-builds.org` SBOM-comparison registration → v1.7+

## Scope-creep redirects

None — all 4 gray areas were scope-internal (implementation choices within REPRO-01..04, not new capabilities). The orphan-`draft:true` cleanup (D-13) is technically Phase 24's leftover but is folded in here because it tightly couples to the verifier's `gh release download` path (the verifier would need artificial auth coupling if `draft: true` stayed).

## Claude's Discretion items

Captured in `25-CONTEXT.md` as D-15 through D-21 — planner-decided details with stated guidance:
- D-15: exact rust-toolchain.toml channel value
- D-16: cron schedule pick (stagger from Phase 22 digest-drift)
- D-17: docs/REPRODUCIBLE-BUILD.md exact prose + Recipe block
- D-18: verifier's expected-sha256 lookup mechanism (table parse vs sidecar file)
- D-19: SECURITY.md §Reproducibility cross-link prose
- D-20: comment-block style for new env: block (auditor-grepable per Plan 22-04)
- D-21: removal of `with: toolchain: stable` from dtolnay/rust-toolchain steps after D-01 lands

---

*Phase: 25-reproducible-build-recipe-scheduled-verifier-registry*
*Discussion: 2026-06-02*
</content>
</invoke>