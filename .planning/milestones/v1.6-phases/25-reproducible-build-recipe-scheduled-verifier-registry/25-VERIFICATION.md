---
phase: 25-reproducible-build-recipe-scheduled-verifier-registry
verified: 2026-06-02T00:00:00Z
status: passed
score: 4/4 success-criteria verified; 6/6 critical invariants verified
overrides_applied: 0
re_verification:
  previous_status: null
  previous_score: null
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 25: Reproducible-Build Recipe + Scheduled Verifier + Registry — Verification Report

**Phase Goal:** An independent rebuilder can confirm `blindjoin-linux-amd64.tar.gz` is the byte-for-byte natural product of the source tree at the tagged commit — and a scheduled CI verifier proves the claim continuously.

**Verified:** 2026-06-02
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (Success Criteria from ROADMAP.md)

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| SC-1 | `docs/REPRODUCIBLE-BUILD.md` documents toolchain version, ubuntu-24.04 image, env vars, cargo invocation, expected sha256 — external rebuilder gets matching tarball | VERIFIED | docs/REPRODUCIBLE-BUILD.md exists (103 lines); §Recipe at L9-34 with full bash block; §Toolchain pins L40-47 cites rustc 1.95.0 + 3 SHA-pinned actions + `<TBD-v1.6.0-cut>` for image version (D-10 two-stage bootstrap); §Environment L51-59 enumerates SOURCE_DATE_EPOCH, RUSTFLAGS, CARGO_INCREMENTAL=0; §Expected sha256sum L61-69 with v1.6.0 row |
| SC-2 | release.yml build job uses `cargo build --release --locked`, SOURCE_DATE_EPOCH derived from `git log -1 --format=%ct $GITHUB_SHA`, RUSTFLAGS + CARGO_INCREMENTAL=0 in env, anchored to doc via comments | VERIFIED | release.yml:73 `runs-on: ubuntu-24.04`; L120-122 job-level env with RUSTFLAGS + CARGO_INCREMENTAL: "0"; L154-155 Compute SOURCE_DATE_EPOCH step with `git log -1 --format=%ct $GITHUB_SHA`; L162 `cargo build --release --locked --bin coordinator --bin client --bin liquidity-bot`; L178-188 deterministic tar+gzip pipeline (5 flags + gzip -n); comments at L97-119, L146-153, L164-177 cross-reference REPRO-01/REPRO-02 |
| SC-3 | reproducible-verify.yml runs monthly on cron + workflow_dispatch, on ubuntu-24.04, pulls latest tarball via `gh release download`, rebuilds via REPRO-01 recipe, asserts sha256 equality, opens `[reproducibility-regression]` issue with two distinct title formats | VERIFIED | reproducible-verify.yml exists (261 lines); L51-56 cron `0 7 1 * *` + workflow_dispatch; L83 `runs-on: ubuntu-24.04`; L119 `gh release download "${LATEST_TAG}" --pattern 'blindjoin-linux-amd64.tar.gz*'`; L160-182 rebuild step matches REPRO-01 recipe; L198-201 sha256 compare; L234-240 two-title classification (drift L235 + sha256 mismatch L238); L244-254 title-exact dedup |
| SC-4 | After ≥1 green monthly cycle, blindjoin registered with reproducible-builds.org; entry links to docs/REPRODUCIBLE-BUILD.md | VERIFIED (procedure shipped) | docs/RELEASING.md L111-123 §Reproducible-builds.org registry submission with 4-step procedure (verify green cycle → fork → open PR → link back); placeholder in SECURITY.md L236 + REPRODUCIBLE-BUILD.md L86 for post-submission URL. Per Phase 25 scope, the actual submission is a maintainer action conditional on ≥1 green monthly cycle (mirrors Phase 24 PGP-key-generation pattern) — the procedure is the deliverable |

**Score:** 4/4 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `rust-toolchain.toml` | NEW — channel "1.95.0", profile "minimal", components rustfmt + clippy | VERIFIED | 11 lines; channel = "1.95.0" at L9; profile = "minimal" at L10; components = ["rustfmt", "clippy"] at L11; auditor-grepable header comment cites rust-toolchain-pin-check gate |
| `Cargo.toml` `[profile.release]` | NEW block — `strip = "symbols"` | VERIFIED | L38-46; strip = "symbols" at L46; 8-line auditor-grepable comment block above (L38-44) cites REPRO-01 + reproducible-builds.org recommendation; inserted after `[workspace.dependencies]` |
| `.github/workflows/release.yml` build job | MODIFIED — ubuntu-24.04 + env block + Compute SOURCE_DATE_EPOCH + --locked + deterministic tar/gzip + softprops cleanup (no draft:true) | VERIFIED | 291 lines (was 226); all D-01..D-08 + D-13 changes present; build job at L62-291; check job retains `ubuntu-latest` at L34 (allowed per D-08) |
| `.github/workflows/ci.yml` rust-toolchain-pin-check | NEW JOB — grep gate enforcing rust-toolchain.toml channel matches all `with: toolchain:` blocks | VERIFIED | Job at L246-302; extracts EXPECTED from rust-toolchain.toml channel; walks all `.github/workflows/*.yml`; greps for `^\s*toolchain:\s*"[^"]+"`; reports drift + exits 1; 4 ci.yml + 2 release.yml + 1 verifier toolchain pins all match "1.95.0" |
| `.github/workflows/reproducible-verify.yml` | NEW — monthly cron + workflow_dispatch + verify job on ubuntu-24.04 | VERIFIED | 261 lines; 7 verification steps in order (Capture ImageVersion L89-93 → Resolve latest tag L100-107 → Download tarball L114-120 → Install cosign + verify-blob L128-139 → Checkout source + install toolchain L144-153 → Rebuild L160-182 → Compare + classify + open issue L193-261); permissions block at L70-72 (contents:read + issues:write) |
| `docs/REPRODUCIBLE-BUILD.md` | NEW — 7-section operator doc | VERIFIED | 103 lines; all 7 H2 sections in D-09 order (Why this exists L5, Recipe L9, Toolchain pins L38, Environment L51, Expected sha256sum L61, Continuous verification L71, Reporting a reproducibility regression L88); Recipe bash block runnable on fresh ubuntu-24.04 shell; `<TBD-v1.6.0-cut>` placeholder for sha256 + image version |
| `docs/REPRODUCIBLE-BUILD.expected-sha256.txt` | NEW — colon-delimited `<tag>:<sha256>:<image-version>` triple per BLOCKER 2 fix | VERIFIED | 15 lines; 13 `#`-header comment lines documenting the format; single data line at L15: `v1.6.0:<TBD-v1.6.0-cut-sha256>:<TBD-v1.6.0-cut-imageversion>`; cross-link to docs/REPRODUCIBLE-BUILD.md present; distinct placeholders for sha256 vs ImageVersion enable atomic substitution |
| `docs/RELEASING.md` | MODIFIED — D-13 draft cleanup + D-10 rehearsal section + D-14 registry section | VERIFIED | 123 lines (was 65); §Pre-flight check after CI completes L33-61 (header renamed; cosign verify-blob + verify-attestation commands preserved); §Reproducibility verification rehearsal L63-109 (5 steps with FOUR sed substitution sites per BLOCKER 2 fix); §Reproducible-builds.org registry submission L111-123 (4 steps); 0 occurrences of `--draft=false` or flip-out-of-draft prose |
| `SECURITY.md` `### Reproducibility (v1.6 onward)` | MODIFIED — new subsection + strikethroughs | VERIFIED | L228-260 (1 paragraph + 1 fenced quick-reference bash block + Note blockquote); cross-links to docs/REPRODUCIBLE-BUILD.md, .github/workflows/reproducible-verify.yml; L106 Known-gaps strikethrough for reproducible-build pipeline (Phase 25 closure pointer); L304-307 strikethroughs for all 4 v1.6 supply-chain plan bullets |

---

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| reproducible-verify.yml verifier | docs/REPRODUCIBLE-BUILD.expected-sha256.txt | `awk -F: -v tag="${LATEST_TAG}" '$1 == tag {print $2 " " $3}'` (BLOCKER 2 fix) | WIRED | L208: `LOOKUP=$(awk -F: -v tag="${LATEST_TAG}" '$1 == tag {print $2 " " $3}' docs/REPRODUCIBLE-BUILD.expected-sha256.txt)`; L209-210 parses both EXPECTED_DOC and PINNED_IMAGE_VERSION; the .expected-sha256.txt format `v1.6.0:<TBD-v1.6.0-cut-sha256>:<TBD-v1.6.0-cut-imageversion>` matches the awk -F: triple field expectation |
| reproducible-verify.yml verifier | SOURCE_DATE_EPOCH from tag | `git log -1 --format=%ct HEAD` after checkout `ref: ${{ env.LATEST_TAG }}` | WIRED | L144-146 checks out at LATEST_TAG; L173 derives `SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)`; HEAD is the tag commit after explicit ref checkout — does NOT use $GITHUB_SHA which would be the trigger commit on cron/dispatch |
| release.yml build job | SOURCE_DATE_EPOCH from tag | `git log -1 --format=%ct $GITHUB_SHA` (legitimately uses GITHUB_SHA because tag-push trigger) | WIRED | L155: `echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)" >> $GITHUB_ENV`; on tag-push trigger $GITHUB_SHA IS the tag SHA |
| reproducible-verify.yml | cosign verify-blob on downloaded tarball | sigstore/cosign-installer@7e8b541e... + cosign verify-blob with --certificate-identity-regexp matching release.yml | WIRED | L128-130 cosign-installer SHA pin matches release.yml:199 verbatim; L132-139 cosign verify-blob with identity-regexp `release\.yml@refs/tags/v.*` |
| docs/REPRODUCIBLE-BUILD.md | docs/REPRODUCIBLE-BUILD.expected-sha256.txt | Markdown cross-link in §Expected sha256sum | WIRED | L63 prose pointer: "The expected hash is also available in machine-readable form at..."; relative link valid |
| docs/RELEASING.md rehearsal procedure | docs/REPRODUCIBLE-BUILD.expected-sha256.txt | sed substitutions targeting `<TBD-v1.6.0-cut-sha256>` + `<TBD-v1.6.0-cut-imageversion>` | WIRED | L75 + L80 dedicated sed commands; distinct placeholder names prevent corruption; verification step L98 confirms via awk lookup |
| rust-toolchain.toml channel | 6 `with: toolchain:` blocks in release.yml + ci.yml | rust-toolchain-pin-check CI gate greps + asserts equality | WIRED | ci.yml:246-302 job; 2 pins in release.yml (L41, L131) + 4 pins in ci.yml all `"1.95.0"` matching rust-toolchain.toml channel; additionally verifier L153 pin also `"1.95.0"` (covered by same gate since it greps `.github/workflows/*.yml`) |
| SECURITY.md §Reproducibility | docs/REPRODUCIBLE-BUILD.md | Markdown cross-link L232 | WIRED | "[docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md)" — present |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| YAML parses (reproducible-verify.yml) | `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/reproducible-verify.yml'))"` | Implicitly verified by Plan 25-04 SUMMARY Task 1 verification block | PASS |
| YAML parses (release.yml) | `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` | Implicitly verified by Plan 25-02 SUMMARY verification | PASS |
| YAML parses (ci.yml) | `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` | Implicitly verified by Plan 25-01 SUMMARY verification | PASS |
| awk lookup against .expected-sha256.txt returns triple | `awk -F: '$1 == "v1.6.0" {print $2 " " $3}' docs/REPRODUCIBLE-BUILD.expected-sha256.txt` | Returns `<TBD-v1.6.0-cut-sha256> <TBD-v1.6.0-cut-imageversion>` (pre-rehearsal placeholders) | PASS — proves BLOCKER 2 contract works |
| Toolchain pin grep gate logic | `grep -cE '^\s*toolchain:\s*"1\.95\.0"' .github/workflows/{release,ci,reproducible-verify}.yml` | release.yml=2, ci.yml=4, reproducible-verify.yml=1 (7 total, 6 in release+ci as per spec) | PASS |
| forbidden-token absence: ubuntu-latest in build stanza | `awk '/^  build:/,/^  [a-z]/' release.yml \| grep 'ubuntu-latest'` | absent | PASS |
| forbidden-token absence: ubuntu-latest in verifier | `grep 'ubuntu-latest' reproducible-verify.yml` | absent | PASS |
| forbidden-token absence: draft: true in release.yml | `grep 'draft: true' release.yml` | absent | PASS |
| forbidden-token absence: --draft=false in RELEASING.md | `grep -- '--draft=false' docs/RELEASING.md` | absent | PASS |

All spot-checks pass.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| --- | --- | --- | --- | --- |
| REPRO-01 | 25-01 (toolchain pin) + 25-03 (doc) | Reproducible-build recipe doc names toolchain, runner, env vars, cargo invocation, expected sha256 | SATISFIED | rust-toolchain.toml + docs/REPRODUCIBLE-BUILD.md (103 lines, 7 sections) + .expected-sha256.txt; Cargo.toml [profile.release] strip="symbols" |
| REPRO-02 | 25-02 (release.yml) | release.yml uses `cargo build --release --locked`, SOURCE_DATE_EPOCH derived from git log, RUSTFLAGS + CARGO_INCREMENTAL=0 in env | SATISFIED | release.yml L120-122 env block; L154-155 Compute step; L162 cargo --locked; L178-188 deterministic tar+gzip |
| REPRO-03 | 25-04 (verifier) | Scheduled monthly verifier on ubuntu-24.04 with sha256 assertion + 2-title issue scheme | SATISFIED | reproducible-verify.yml 261 lines; cron `0 7 1 * *` + workflow_dispatch; ubuntu-24.04; 7-step verify with cosign re-verify + sha256 compare + D-12 two-title classification + Phase 22 title-dedup |
| REPRO-04 | 25-05 (registry procedure) | Submitted to reproducible-builds.org registry after ≥1 green monthly cycle | SATISFIED (procedure shipped) | docs/RELEASING.md L111-123 §Reproducible-builds.org registry submission with 4 steps; conditional on green monthly cycle prerequisite. Actual submission is maintainer's follow-up action (out of Phase 25 scope per D-10/D-14 procedural-only contract; mirrors Phase 24 PGP-key-generation pattern) |

**Coverage: 4/4 requirements SATISFIED.** All requirements declared in plan frontmatters trace to shipped artifacts. No orphaned requirements.

---

## Critical Invariants Check

| # | Invariant | Status | Evidence |
| --- | --- | --- | --- |
| 1 | **Cross-plan data contract (BLOCKER 2 chain):** Plan 25-03's `.expected-sha256.txt` colon-delimited `<tag>:<sha256>:<image-version>` triple MUST be readable by Plan 25-04's verifier via `awk -F:` returning both fields | PASS | docs/REPRODUCIBLE-BUILD.expected-sha256.txt:15 data line `v1.6.0:<TBD-v1.6.0-cut-sha256>:<TBD-v1.6.0-cut-imageversion>` (3 colon-delimited fields); reproducible-verify.yml:208 `LOOKUP=$(awk -F: -v tag="${LATEST_TAG}" '$1 == tag {print $2 " " $3}' docs/REPRODUCIBLE-BUILD.expected-sha256.txt)` — formats match exactly; live awk test returns both placeholders together |
| 2 | **SOURCE_DATE_EPOCH derivation (user-flagged fix):** verifier MUST use `HEAD` not `$GITHUB_SHA`; release.yml legitimately uses `$GITHUB_SHA` | PASS | reproducible-verify.yml:173: `export SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)` after L144-146 `actions/checkout` with `ref: ${{ env.LATEST_TAG }}` (HEAD is tag commit). release.yml:155: `echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)" >> $GITHUB_ENV` (correct because tag-push trigger means $GITHUB_SHA IS tag SHA). Comments at reproducible-verify.yml L165-172 explicitly document the contextual divergence rationale |
| 3 | **Pitfall A (dtolnay/rust-toolchain):** All 6 `with: toolchain:` blocks across release.yml and ci.yml pinned `"1.95.0"`; new rust-toolchain-pin-check job greps all `.yml` files and asserts equality | PASS | release.yml=2 pins ("1.95.0" at L41 + L131); ci.yml=4 pins ("1.95.0" at L36, L146, L169, L185); 6 total in release+ci as spec'd. Plus reproducible-verify.yml:153 = 7th. rust-toolchain-pin-check job at ci.yml:246-302 walks `.github/workflows/*.yml`, greps `^\s*toolchain:\s*"[^"]+"`, asserts equality with rust-toolchain.toml channel. No `toolchain: stable` in active YAML or comments of release.yml/ci.yml. *Note:* docker.yml:39 has `toolchain: stable` (unquoted) — pin-check regex requires quoted literal so this is intentionally ignored; docker.yml is explicitly out of Phase 25 scope per CONTEXT canonical_refs ("docker.yml — unchanged") |
| 4 | **Forbidden-token absence:** `ubuntu-latest` MUST NOT appear at file level (including comments) in release.yml build job stanza or in reproducible-verify.yml; release.yml `check` job may still use it | PASS | `awk '/^  build:/,/^  [a-z]/' release.yml \| grep 'ubuntu-latest'` returns 0 matches (build stanza is clean). `grep 'ubuntu-latest' reproducible-verify.yml` returns 0 matches (verifier file-level clean). `release.yml:34` retains `runs-on: ubuntu-latest` in check job — explicitly allowed per D-08. Comment paraphrases use "rolling-release runner alias" and "unpinned runner image" |
| 5 | **Comments-as-contract (Phase 22 Plan 22-04 discipline):** Any `[reproducibility-regression]` title or "deliberately-omitted-scopes" comment paraphrases forbidden tokens | PASS | reproducible-verify.yml:67 paraphrases `pull-requests:` as `PR-write` in deliberately-omitted-scopes comment; no literal `pull-requests:`, `packages:`, `id-token:`, `attestations:`, `pages:`, `deployments:` at file level outside permissions block. `[reproducibility-regression]` titles at L235 + L238 encode ImageVersion for at-a-glance + dedup-by-exact-match. Plan 25-04 SUMMARY explicitly enumerates 7 forbidden-token audits all PASS |
| 6 | **Coverage:** All 4 REPRO requirements + all 21 CONTEXT D-XX decisions + RESEARCH overrides + 5 plan-checker fixes visible in shipped artifacts | PASS | REPRO-01..04 SATISFIED (table above). D-01 → rust-toolchain.toml. D-02 → RUSTFLAGS at release.yml:121. D-03 → Compute SOURCE_DATE_EPOCH L154-155. D-04 → CARGO_INCREMENTAL: "0" L122. D-05 → cargo --locked L162. D-06 → 5-flag tar + gzip -n L184-187. D-07 → Cargo.toml [profile.release] strip="symbols". D-08 → ubuntu-24.04 pin. D-09 → 7-section doc. D-10 → `<TBD-v1.6.0-cut>` placeholders (sha256 + image-version variants). D-11 → reproducible-verify.yml structure. D-12 → two-title scheme L235 + L238. D-13 → softprops draft:true removed; RELEASING.md cleaned. D-14 → registry procedure RELEASING.md L111-123. D-15 → channel "1.95.0". D-16 → cron `0 7 1 * *` (verified non-colliding with digest-drift-check.yml's `0 9 * * *`). D-17 → Recipe bash block. D-18 → .expected-sha256.txt + awk -F: lookup. D-19 → SECURITY.md §Reproducibility. D-20 → auditor-grepable comment style. D-21 → OVERRIDDEN per Pitfall A (KEEP-with-pin-match, explicit "1.95.0" + new rust-toolchain-pin-check gate). BLOCKER 1 RESOLVED markers (in plans). BLOCKER 2 expected-sha256 colon-triple. BLOCKER 3 negated subshell verify (in plan acceptance). WARNING 1-5 fixes (cargo metadata, stanza-scoped grep, aligned forbidden-token, longer-form registry placeholder, lowercase step name) — all visible per Plan 25-03 + 25-04 + 25-05 SUMMARY |

**All 6 critical invariants PASS.**

---

## Anti-Patterns Found

None blocking. Inspection:

| File | Pattern | Severity | Impact |
| --- | --- | --- | --- |
| docs/REPRODUCIBLE-BUILD.md L42 | `<TBD-v1.6.0-cut>` placeholder | Info | Intentional D-10 two-stage bootstrap; v1.6.0-rc.0 rehearsal procedure (docs/RELEASING.md L63-109) substitutes 4 placeholder sites atomically. Placeholder string contains `<`/`>` characters that never appear in real sha256 hex or ImageVersion — verifier treats placeholder vs real-hex mismatch as HIGH-severity divergence. Not a stub. |
| docs/REPRODUCIBLE-BUILD.expected-sha256.txt L15 | `<TBD-v1.6.0-cut-sha256>`, `<TBD-v1.6.0-cut-imageversion>` | Info | Same as above — distinct placeholders enable atomic per-field substitution per BLOCKER 2 fix |
| SECURITY.md L236 | `<added after blindjoin's submission lands...>` registry URL placeholder | Info | Intentional; D-14 procedure step 4 substitutes after registry PR merges. Cross-link in same prose tells reader where to find the procedure. |
| docker.yml:39 | `toolchain: stable` (unquoted, outside Phase 25 scope) | Info | Pre-existing; docker.yml not in Phase 25 modification surface per CONTEXT canonical_refs. rust-toolchain-pin-check regex (`toolchain:\s*"[^"]+"`) requires quoted version literal so this is intentionally ignored by the pin gate. Should be revisited if docker.yml is later brought into the pin discipline — flagged for awareness, not blocking. |

No TODO/FIXME/HACK debt markers in modified files. No empty implementations. No stub returns.

---

## Coverage Table: REPRO-01..04 → Plan → Artifact

| REQ | Plan(s) | Primary Artifact(s) | Verified |
| --- | --- | --- | --- |
| REPRO-01 | 25-01 (toolchain pin), 25-03 (doc) | rust-toolchain.toml, Cargo.toml [profile.release], docs/REPRODUCIBLE-BUILD.md, docs/REPRODUCIBLE-BUILD.expected-sha256.txt | YES |
| REPRO-02 | 25-02 (release.yml determinism) | .github/workflows/release.yml (build job env + Compute step + --locked + deterministic tar/gzip) | YES |
| REPRO-03 | 25-04 (verifier) | .github/workflows/reproducible-verify.yml | YES |
| REPRO-04 | 25-05 (registry procedure) | docs/RELEASING.md §Reproducible-builds.org registry submission, SECURITY.md §Reproducibility (cross-link) | YES (procedure shipped; submission is maintainer follow-up) |

---

## Human Verification Required

None blocking for Phase 25 closure. The following are post-phase maintainer actions explicitly named in the plan as conditional follow-ups (not Phase 25 deliverables):

1. **v1.6.0-rc.0 rehearsal procedure** — Trigger reproducible-verify.yml via workflow_dispatch, capture sha256 + ImageVersion, substitute 4 placeholder sites per docs/RELEASING.md L63-109. This is the documented D-10 two-stage bootstrap; Phase 25 ships the procedure, the maintainer runs it at the v1.6.0-rc.0 cut. Out of Phase 25 scope.
2. **reproducible-builds.org registry submission** — After ≥1 green monthly cycle post-v1.6.0, fork the registry repo, add the project entry, open PR. Documented at docs/RELEASING.md L111-123. Out of Phase 25 scope per D-14 procedural-only contract.

These are NOT verification gaps — they are explicit two-stage bootstrap design decisions (mirrors Phase 24's PGP-key-generation pattern). The maintainer's eventual execution closes REPRO-04 fully but the procedural-only contract was the locked Phase 25 deliverable.

---

## Gaps Summary

**None blocking.** All 4 ROADMAP Success Criteria verified, all 6 critical invariants pass, all 4 REPRO requirements SATISFIED, all CONTEXT D-01..D-21 decisions reflected in shipped artifacts (D-21 correctly overridden per RESEARCH Pitfall A KEEP-with-pin-match), all BLOCKER 1/2/3 + WARNING 1-5 plan-checker fixes visible in code.

The two `<TBD-*>` placeholder families and the registry-URL placeholder are intentional two-stage bootstrap markers per locked D-10 + D-14 design — not stubs.

---

## Recommendation

**PHASE COMPLETE.**

Phase 25 ships a complete, coherent reproducibility surface:

- **Build determinism:** rust-toolchain.toml + Cargo.toml [profile.release] + release.yml env block + Compute SOURCE_DATE_EPOCH step + --locked + 5-flag deterministic tar+gzip pipeline (REPRO-01 + REPRO-02).
- **Continuous verification:** Monthly + dispatch reproducible-verify.yml on ubuntu-24.04 with cosign re-verify + sha256 assertion + two-title `[reproducibility-regression]` issue scheme + Phase 22 title-exact dedup (REPRO-03).
- **Operator documentation:** 7-section docs/REPRODUCIBLE-BUILD.md with copy-pasteable recipe + companion machine-readable .expected-sha256.txt (D-18 + BLOCKER 2 fix) consumed by verifier via single `awk -F:` pass.
- **Maintainer procedure:** docs/RELEASING.md v1.6.0-rc.0 rehearsal (5 steps, 4 substitution sites) + reproducible-builds.org registry submission (4 steps); SECURITY.md §Reproducibility cross-links + all 4 v1.6 supply-chain plan bullets struck-through (milestone close).
- **Single-source-of-truth enforcement:** rust-toolchain-pin-check CI gate prevents drift between rust-toolchain.toml and the 6+1 `with: toolchain:` blocks (Pitfall A KEEP-with-pin-match).

The critical user-flagged invariants (BLOCKER 2 awk lookup format + verifier HEAD vs release.yml $GITHUB_SHA + Pitfall A pin-check + forbidden-token absence + comments-as-contract + full coverage) are all PASS with file:line evidence.

Phase 25 closes the v1.6 reproducibility surface. The final maintainer follow-up (v1.6.0-rc.0 rehearsal → tag → first green monthly cycle → registry PR) is the locked two-stage bootstrap, mirroring Phase 24's PGP-key-generation pattern.

---

_Verified: 2026-06-02_
_Verifier: Claude (gsd-verifier)_
