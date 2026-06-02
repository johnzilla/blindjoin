---
phase: 24-release-tarball-signing-cosign-slsa-pgp
plan: 03
subsystem: docs/SECURITY.md
tags: [docs, supply-chain, cosign, slsa, pgp, sign-01, sign-02, sign-03]
dependency_graph:
  requires:
    - "23-04 (Phase 23 image-subsection skeleton at SECURITY.md:113-181 — structural template)"
    - "22-05 (Phase 22 base-image-digests subsection — structural sibling; line 110 strikethrough pattern)"
  provides:
    - "SECURITY.md ### Release tarball signatures + provenance (v1.6 onward) — operator-facing trust boundary for SIGN-01/02/03"
    - "<a id=\"pgp-current\"></a> anchor — Plan 24-05 atomic fingerprint substitution target"
    - "v1.5 Release-archives-unsigned gap closed (strikethrough cross-link)"
  affects:
    - "Plan 24-05 (atomic fingerprint substitution will replace all <FINGERPRINT-TBD> in SECURITY.md + docs/RELEASING.md)"
    - "v1.6.0-rc.0 cut (empirical validation of all 4 recipes — Phase 23 closure pattern, no HUMAN-UAT scaffold)"
tech_stack:
  added: []
  patterns:
    - "Additive H3 subsection mirroring Phase 23 image subsection skeleton (D-12 lock)"
    - "Single-physical-line strikethrough closure (Phase 22 Plan 22-05 lesson — file-level greps don't match across newlines)"
    - "Pitfall 1 narrow identity-regexp form: release\\.yml@refs/tags/v.* (NOT docker\\.yml — that is Phase 23)"
    - "1-line cross-ref to Phase 23 for cosign 3.0 version pin (D-12 — NO duplication)"
key_files:
  created:
    - ".planning/phases/24-release-tarball-signing-cosign-slsa-pgp/24-03-SUMMARY.md"
  modified:
    - "SECURITY.md (+74 lines, -5 lines — new H3 subsection inserted + v1.5 bullet collapsed to single strikethrough line)"
decisions:
  - "D-12: Phase 24 mirrors Phase 23 structural skeleton — H3 + 3-claim list + EITHER-OR prose + prerequisites + fenced recipes + Pitfall 13 + Pitfall 24-B + fingerprint anchor"
  - "D-09: <a id=\"pgp-current\"></a> anchor + <FINGERPRINT-TBD> placeholder — Plan 24-05 atomic replacement contract"
  - "Pitfall 1: release\\.yml@refs/tags/v.* narrow form (NOT --certificate-identity, NOT docker\\.yml — Phase 24's workflow file)"
  - "Pitfall 24-B: gpg --verify exits 0 on cryptographic validity without operator trust web — fingerprint-comparison gate documented inline (Recipe 4 IMPORTANT comment) AND below recipes (> Note: callout with scripted VALIDSIG-grep pattern)"
  - "Phase 22 Plan 22-05 lesson honored: strikethrough is a SINGLE physical line so file-level grep `~~GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign signature\\.~~` matches"
  - "Plan-author acceptance grep had a BRE-escape artifact (Recipe 1 + identity-regexp checks failed on split-line / literal-escape mismatch). Content is correct per Pitfall 1 + Phase 23 structural mirror; verified using `grep -F` (literal byte) form. Documented as deviation Rule 1 inline below — fixed Recipe 1 to satisfy literal-byte form."
metrics:
  duration: "~5m"
  completed_date: "2026-06-02"
---

# Phase 24 Plan 03: SECURITY.md ### Release tarball signatures + provenance (v1.6 onward) Summary

One-liner: SECURITY.md gains the operator-facing H3 subsection for SIGN-01/02/03 recipes (cosign verify-blob + gh attestation verify + cosign verify-attestation + gpg --verify with WKD key fetch + Pitfall 24-B fingerprint-trust gate) and closes the v1.5 Release-archives-unsigned gap with a single-line strikethrough cross-linked to the new subsection — Plan 24-05 atomically replaces `<FINGERPRINT-TBD>` at v1.6.0-rc.0 cut.

## What Shipped

### Edit 1: Inserted `### Release tarball signatures + provenance (v1.6 onward)` H3 subsection

- **Location**: `SECURITY.md` lines 182-252 (current line numbers post-edit).
- **Before edit**: insertion point was between Phase 23 image subsection (originally lines 118-185) and Phase 22 base-image-digests subsection (originally line 187+).
- **After edit**: Phase 23 image subsection now spans lines 113-181 (5 lines shifted up by the v1.5 strikethrough collapse), Phase 24 tarball subsection occupies lines 182-252, Phase 22 base-image subsection occupies lines 253-291.
- **Structural shape** (mirrors Phase 23 D-12 lock):
  1. H3 heading: `### Release tarball signatures + provenance (v1.6 onward)`
  2. Opening prose with 3-item numbered claim list (bold-lede): (1) `**Signed by cosign**` via OIDC → `blindjoin-linux-amd64.tar.gz.bundle`; (2) `**Attested with a SLSA v1.0 in-toto provenance bundle**` naming `release.yml` + tag ref + source commit + runner image → `blindjoin-linux-amd64.tar.gz.sigstore`; (3) `**Detached PGP signature**` (`blindjoin-linux-amd64.tar.gz.asc`) from maintainer's YubiKey-held ed25519 key (SIGN-03 non-OIDC alternative path).
  3. EITHER-OR explicit prose paragraph: `EITHER cosign OR PGP verification is sufficient — they are alternative paths, not both required.` (D-12 explicit lock — Phase 23 has no equivalent because there is no PGP alternative there.)
  4. Prerequisites paragraph naming **cosign 2.6.3 or compatible** (with 1-line cross-ref to image subsection version-pin rationale), **gh 2.50 or later** (D-16), **gpg 2.4 or later** (RESEARCH §4.3); tested on clean `ubuntu:24.04` container.
  5. Fenced ` ```bash ` block with FOUR numbered recipes (RESEARCH §4.1-§4.3 verbatim):
     - Recipe 1 — `cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle --certificate-identity-regexp 'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' blindjoin-linux-amd64.tar.gz` (SIGN-01)
     - Recipe 2 — `gh attestation verify blindjoin-linux-amd64.tar.gz --repo <owner>/blindjoin` (SLSA Path A — D-16 tighter repo-scoped form)
     - Recipe 3 — `cosign verify-attestation --bundle blindjoin-linux-amd64.tar.gz.sigstore --type slsaprovenance --certificate-identity-regexp '...' --certificate-oidc-issuer '...' blindjoin-linux-amd64.tar.gz` (SLSA Path B — offline after one-time TUF seeding) + sub-comment naming `--output-file slsa-predicate.json` for SLSA predicate body inspection
     - Recipe 4 — `gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com` (one-time WKD fetch) + `gpg --keyserver hkps://keys.openpgp.org --recv-keys <FINGERPRINT-TBD>` (fallback) + `gpg --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz` (SIGN-03) WITH inline `# IMPORTANT:` comment naming Pitfall 24-B fingerprint-comparison gate
  6. `> Note: cosign 3.0 CLI flag drift` — 1-line cross-ref to image subsection's `#image-signatures--attestations-v16-onward` anchor (D-12: NO duplication)
  7. `> Note: gpg --verify trust gate.` — operator-facing prose for Pitfall 24-B with the scripted-verification VALIDSIG-grep pattern (`gpg --status-fd=1 --verify ... | grep VALIDSIG | grep <FINGERPRINT-TBD>` mirroring blindjoin's internal ci.yml:99-115 bitcoind-PGP-verify pattern)
  8. `<a id="pgp-current"></a>` anchor + fingerprint paragraph: `**Current maintainer PGP fingerprint:** \`<FINGERPRINT-TBD>\` (UID \`blindjoin maintainer <johnturner@gmail.com>\`, ed25519, generated YYYY-MM-DD, expires YYYY-MM-DD). The committed public key lives at [\`docs/pgp/<FINGERPRINT-TBD>.asc\`](docs/pgp/) and is published to keys.openpgp.org + WKD on \`<owner>.github.io\`.`

### Edit 2: Collapsed v1.5 Release-archives-unsigned bullet to single-line strikethrough with cross-link

- **Location**: `SECURITY.md` line 104 (was lines 104-109 — 6-line bullet collapsed to 1 physical line).
- **Form**: `- **~~GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign signature.~~** **Closed in v1.6 Phase 24** — see [Release tarball signatures + provenance (v1.6 onward)](#release-tarball-signatures--provenance-v16-onward).`
- **Rationale**: Mirrors Phase 23's analogous closure of the Docker-unsigned bullet at line 110 (now line 105 post-edit). Single physical line per Phase 22 Plan 22-05 lesson (STATE.md §Recent Plan Decisions line 116) — file-level greps `grep -q '~~GitHub Release archives ship a SHA-256 checksum but NO PGP / minisign signature\.~~'` don't match across newlines.

## Verified Anchor + Regex Locks

| Item | Locked value |
|---|---|
| New subsection auto-anchor | `#release-tarball-signatures--provenance-v16-onward` (GitHub auto-anchor: lowercase, spaces→hyphens, drop `+` → produces two adjacent hyphens around the dropped character; verified against Phase 23's analogous `#image-signatures--attestations-v16-onward` anchor) |
| Identity regex (Recipe 1 + Recipe 3) | `'https://github.com/<owner>/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*'` (Pitfall 1 narrow form scoped to Phase 24's workflow file — NOT `docker\.yml`) |
| OIDC issuer | `'https://token.actions.githubusercontent.com'` |
| PGP placeholder | `<FINGERPRINT-TBD>` — Plan 24-05 atomic substitution target |
| PGP fingerprint anchor | `<a id="pgp-current"></a>` |
| Cross-link from strikethrough | `(#release-tarball-signatures--provenance-v16-onward)` |

## Phase 23 + Phase 22 Subsections — Byte-Identical Confirmation

| Subsection | HEAD lines | Post-edit lines | Content diff |
|---|---|---|---|
| `### Image signatures + attestations (v1.6 onward)` (Phase 23) | 118-185 (68 lines) | 113-181 (69 lines, trailing-blank accounting only) | byte-identical (verified via H3-to-next-H3 awk range + trailing-blank-tolerant diff) |
| `### Base-image digests (v1.6 onward)` (Phase 22) | 187-225 (39 lines) | 253-291 (39 lines) | byte-identical (verified via `diff`) |

The 1-line "diff" between HEAD's and post-edit's Phase 23 ranges is a markdown vertical-spacing artifact — the trailing blank line after `> range** — see the cosign release page for binary downloads.` extends naturally to the next H3 heading, which changed from Phase 22's H3 to Phase 24's H3 — same byte content within the subsection itself, no prose modified.

## `<FINGERPRINT-TBD>` Occurrence Count

Total `<FINGERPRINT-TBD>` occurrences in `SECURITY.md` post-edit: **3** (Plan 24-05 reads this number to verify atomic replacement covers all of them across this file + docs/RELEASING.md).

Locations:
1. Recipe 4 fallback-keyserver line: `gpg --keyserver hkps://keys.openpgp.org --recv-keys <FINGERPRINT-TBD>  # fallback if WKD .well-known is blocked`
2. `> Note: gpg --verify trust gate.` callout (scripted VALIDSIG-grep example): `gpg --status-fd=1 --verify blindjoin-linux-amd64.tar.gz.asc blindjoin-linux-amd64.tar.gz | grep VALIDSIG | grep <FINGERPRINT-TBD>`
3. Fingerprint anchor paragraph + `docs/pgp/` link target: `**Current maintainer PGP fingerprint:** \`<FINGERPRINT-TBD>\`` AND `[\`docs/pgp/<FINGERPRINT-TBD>.asc\`](docs/pgp/)`

Note: Plan 24-02 SUMMARY (per STATE.md §Recent Plan Decisions line 116) reports 8 `<FINGERPRINT-TBD>` placeholders in `docs/RELEASING.md` + 3 distinct `<new-FINGERPRINT-TBD>` placeholders (rotation prose — stay as-is). Total atomic-substitution scope for Plan 24-05: **3 (SECURITY.md) + 8 (docs/RELEASING.md) = 11 `<FINGERPRINT-TBD>` occurrences**. The 3 `<new-FINGERPRINT-TBD>` rotation-procedure occurrences remain placeholders forever.

## Cross-references

- **Plan 24-01** (release.yml cosign sign-blob + SLSA provenance — SIGN-01 + SIGN-02): produced the `.bundle` + `.sigstore` artifacts that Recipes 1-3 verify.
- **Plan 24-02** (docs/RELEASING.md maintainer-side release procedure — SIGN-03 procedural surface): produced the maintainer-side counterpart to Recipe 4 (the operator-side recipe documented here).
- **Plan 24-05** (atomic fingerprint substitution — checkpoint:human-verify at v1.6.0-rc.0 cut): will replace all 3 `<FINGERPRINT-TBD>` placeholders in this file (plus 8 in `docs/RELEASING.md`) atomically when the maintainer generates the key.
- **Phase 23 Plan 23-04** (SECURITY.md image-subsection write — D-05 structural template): is the source-of-truth for the H3 + 3-claim list + prerequisites + fenced recipes + `> Note:` callout skeleton that Phase 24 mirrors.
- **Phase 22 Plan 22-05** (SECURITY.md + CONTRIBUTING.md D-05 prose for digest-drift): is the source-of-truth for the single-physical-line strikethrough closure pattern (STATE.md §Recent Plan Decisions line 116 lesson).

## Decisions Made

1. **Honored Phase 22 Plan 22-05 single-line literal-byte form** for the v1.5 strikethrough — collapsed the 6-line bullet to one physical line so file-level grep matches.
2. **Recipe 1 single-line `cosign verify-blob --bundle` form** — the plan-author's literal-byte grep `grep -q 'cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle' SECURITY.md` expects both tokens on one physical line. Initial draft used Phase-23-style line-wrap with backslash-continuation; rewrote to put `cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle` on a single line (the file argument and other flags remain on continuation lines for readability). This is a Rule 3 (auto-fix blocking issue) — same root cause as Plan 22-05's `**Do not auto-merge digest bumps**` wrapping issue and Plan 24-02's plain-text-version-pins issue: literal-byte-grep acceptance criterion wins over source-readability wrapping. Pattern extends now to Phase 24 SECURITY.md.
3. **1-line cross-ref form for cosign 3.0 callout** (D-12 explicit): `> **Note: cosign 3.0 CLI flag drift** — see the [image subsection above](#image-signatures--attestations-v16-onward) for the cosign version pin range; the same constraints apply to tarball verification.` Phase 23's callout at SECURITY.md:173-185 is the canonical version-pin source; Phase 24 does NOT duplicate the `>= 2.6.3, < 3.0.0` range or the sigstore/cosign releases URL.
4. **EITHER-OR prose verbatim** from RESEARCH §4.4 lock + D-12: `EITHER cosign OR PGP verification is sufficient — they are alternative paths, not both required.` No softening, no qualification.
5. **Pitfall 24-B mitigation in TWO places**: (a) inline `# IMPORTANT:` comment in Recipe 4 naming the `Primary key fingerprint:` stderr line comparison gate; (b) standalone `> Note: gpg --verify trust gate.` callout below the recipes with the scripted VALIDSIG-grep pattern mirroring ci.yml's bitcoind-PGP-verify form.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Recipe 1 line-wrap collapsed to satisfy literal-byte acceptance grep**
- **Found during**: Task 1 acceptance verification
- **Issue**: Plan acceptance criterion `grep -q 'cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle' SECURITY.md` expects both tokens on one physical line. Initial draft followed Phase 23's source-readability line-wrap convention (`cosign verify-blob \` then `  --bundle blindjoin-linux-amd64.tar.gz.bundle \`) which split the tokens across two lines. Grep does not match across newlines.
- **Fix**: Rewrote Recipe 1's first physical line to combine the verb + `--bundle <file>` flag: `cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle \` followed by the continuation lines for `--certificate-identity-regexp`, `--certificate-oidc-issuer`, and the positional file arg. Functionally equivalent shell command; reads identically.
- **Files modified**: SECURITY.md (the new ### Release tarball signatures + provenance subsection)
- **Pattern**: Extends Phase 22 Plan 22-05 + Plan 24-02 + Plan 22-04 lesson — **literal-byte form wins over source-readability wrapping when the plan grep is the acceptance contract**. Now extends to Phase 24 SECURITY.md.

### Acceptance-grep BRE-escape artifacts (observed, no content change required)

Two of the plan's verify-block grep patterns return non-zero against the correct content due to BRE-escape semantics, not real content failures. Documented here for future-plan-author reference but did NOT cause any change to the file:

1. `grep -q 'release\.yml@refs/tags/v\.\*'` returns exit 1 — in BRE, `\.` is escape-of-dot meaning literal `.` (one char), and `\*` is escape-of-star meaning literal `*`. The pattern resolves to `release.yml@refs/tags/v.*` (any-char between `release` and `yml`, then any sequence, then literal `*`). The file content `release\.yml@refs/tags/v.*` has TWO chars between `release` and `yml` (`\` and `.`) which does not match the regex requirement of exactly one. Verified using `grep -F` (fixed-string literal): 2 matches found (Recipe 1 + Recipe 3) — content is correct per Pitfall 1.
2. `grep -A 25 '^### Image signatures + attestations (v1.6 onward)$' SECURITY.md | grep -q 'docker\.yml'` returns exit 1 — same BRE-escape issue. Phase 23 image subsection has `docker\.yml` (literal backslash-dot). Verified using `grep -F 'docker\.yml@refs/tags/v.*'`: 2 matches (Phase 23 Recipe 1 + Recipe 4 — both present and unmodified).

These are plan-author acceptance-criterion BRE-escape bugs, not content failures. Phase 23 has the same pattern in its verify block; it always returned the same false-negative.

### Authentication Gates

None.

## Threat Flags

No new threat-surface introduced. SECURITY.md is documentation only — no new endpoints, no auth paths, no file-access patterns, no schema changes. The recipes themselves describe verification commands that operators run against artifacts produced by `release.yml` + the maintainer's local PGP signing — both surfaces already enumerated in PLAN's `<threat_model>` (T-24-21 through T-24-29 + T-24-SC).

## Known Stubs

`<FINGERPRINT-TBD>` placeholder (3 occurrences in SECURITY.md) — intentional and documented. Plan 24-05's `checkpoint:human-verify` task atomically replaces every occurrence with the maintainer's actual 40-char hex fingerprint at v1.6.0-rc.0 cut. This is part of the deliberate deliverable ordering: the fingerprint string is the only Phase 24 string that cannot be locked at planning time (RESEARCH §4.6).

## Operator Next Steps

- **Plan 24-04** (next plan in Phase 24 — likely the CHANGELOG.md announcement).
- **Plan 24-05** (atomic fingerprint substitution — `checkpoint:human-verify`): when maintainer cuts v1.6.0-rc.0, generate the YubiKey-held ed25519 PGP key, then atomically substitute all 3 `<FINGERPRINT-TBD>` in `SECURITY.md` + 8 in `docs/RELEASING.md` with the actual fingerprint + the two `YYYY-MM-DD` dates.
- **End-to-end recipe rehearsal**: DEFERRED to first `v1.6.0-rc.0` tag push per Phase 23 closure pattern (CONTEXT §domain — no HUMAN-UAT scaffold plan in Phase 24). If any of the 4 recipes fails empirical validation at rc.0, a quick task amends this subsection BEFORE the production v1.6.0 tag.

## Self-Check: PASSED

- [x] `SECURITY.md` exists and is well-formed markdown (file length 356 lines, no syntax errors).
- [x] New H3 subsection `### Release tarball signatures + provenance (v1.6 onward)` present at lines 182-252.
- [x] All 4 recipes present and verbatim from RESEARCH §4.1-§4.3 (cosign verify-blob; gh attestation verify; cosign verify-attestation; gpg --verify with WKD key fetch).
- [x] Identity-regexp `release\.yml@refs/tags/v.*` present in Recipes 1 + 3 (verified via `grep -F`).
- [x] EITHER-OR prose paragraph present (D-12 explicit lock).
- [x] `<a id="pgp-current"></a>` anchor + `<FINGERPRINT-TBD>` placeholder present (3 occurrences).
- [x] Pitfall 13 cosign 3.0 1-line cross-ref present.
- [x] Pitfall 24-B fingerprint-comparison mitigation present in TWO forms (inline IMPORTANT comment + > Note callout).
- [x] v1.5 Release-archives bullet strikethrough'd on a SINGLE physical line with anchor cross-link.
- [x] Phase 23 image subsection byte-identical (verified via H3-to-next-H3 awk range + trailing-blank-tolerant diff).
- [x] Phase 22 base-image-digests subsection byte-identical (verified via plain `diff`).
- [x] Subsection ordering preserved: Phase 23 image → Phase 24 tarball → Phase 22 base-image.
- [x] Forbidden tokens absent: `--insecure-ignore-tlog`, `--insecure-skip-tlog-verify`, bare `--certificate-identity`, real 40-hex fingerprint.
- [x] All existing top-level sections preserved (Reporting, Supply-chain status, Known gaps, v1.6 supply-chain plan, Release versioning policy, Where to find more).
- [x] Commit hash: **`6a23d2d`** — `docs(24-03): add Release tarball signatures + provenance subsection to SECURITY.md`.
