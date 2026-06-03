---
phase: 24-release-tarball-signing-cosign-slsa-pgp
plan: 02
post_execution_scope_reduction:
  date: 2026-06-02
  reason: |
    SIGN-03 (PGP path) deferred indefinitely at Phase 24 closeout. The PGP key
    generation, rotation, revocation, keys.openpgp.org publish, and WKD publish
    sections of docs/RELEASING.md were stripped — file went from ~245 lines to
    ~60 lines. Per-release procedure tightened: PGP detach-sign step removed,
    .asc upload removed, .asc upload + draft-flip merged into a single step.
  remaining_scope: |
    docs/RELEASING.md now documents the cosign+SLSA-only release procedure:
    prerequisites (gh, cosign), per-release procedure (tag → watch CI →
    pre-flight verify → flip draft), and pre-flight cosign verify recipes.
subsystem: ci-supply-chain
tags: [docs, releasing, cosign, slsa, maintainer-procedure]
requires:
  - CONTRIBUTING.md (existing — relative link target for audience-disambiguation lede)
  - .github/workflows/release.yml (existing — Phase 24 Plan 24-01 final state; per-release step 2 link target)
  - Phase 24 Plan 24-05 atomic placeholder-replacement contract for <FINGERPRINT-TBD>
provides:
  - docs/RELEASING.md — maintainer-side release procedure (5-step per-release flow)
  - docs/RELEASING.md — PGP key generation 5-step YubiKey ceremony (RESEARCH §5.1)
  - docs/RELEASING.md — PGP key rotation 6-step procedure (RESEARCH §5.2)
  - docs/RELEASING.md — PGP key revocation emergency procedure (RESEARCH §5.2 pitfall)
  - docs/RELEASING.md — keys.openpgp.org publication procedure (RESEARCH §5.1 step 4)
  - docs/RELEASING.md — WKD direct-method publication procedure (RESEARCH §5.3)
  - docs/RELEASING.md — pre-flight cosign verify gate before draft flip
  - 8 <FINGERPRINT-TBD> placeholders + 3 <new-FINGERPRINT-TBD> placeholders (Plan 24-05 replaces atomically)
affects:
  - docs/RELEASING.md (created; 245 lines)
tech-stack:
  added: []
  patterns:
    - Long-form docs/<UPPERCASE>.md policy file shape (mirrors AUDIT-CHARTER.md, PROTOCOL.md precedent)
    - Audience-disambiguation lede gating contributors away from maintenance procedures (D-11)
    - <FINGERPRINT-TBD> placeholder convention for delayed atomic substitution (Plan 24-05 contract)
    - Fenced ```bash command blocks with inline literal commands (operator copy-paste UX)
    - Cross-section anchors inside docs/RELEASING.md (#publishing-the-key-to-wkd, #publishing-the-key-to-keysopenpgporg, #pgp-key-generation-one-time-not-a-release-cut-step)
    - Plain-text version pins ("gpg 2.4+", not "`gpg` 2.4+") to satisfy literal-byte plan grep contracts
key-files:
  created:
    - docs/RELEASING.md
  modified: []
decisions:
  - "Filename uses UPPERCASE convention matching docs/AUDIT-CHARTER.md + docs/PROTOCOL.md precedent (PATTERNS §9)"
  - "Audience-disambiguation lede gates contributors away from maintenance procedures (D-11) — load-bearing for plan must_haves"
  - "Subdomain method WKD documented as NOT viable on *.github.io (RESEARCH §5.3) — direct method only"
  - "PGP key revocation is a separate H2 from rotation (RESEARCH §5.2 pitfall — different procedures)"
  - "Operator-side public-key binding verification (gpg --with-colons --import-options show-only) is self-verifying — no SECURITY.md prose required to anchor the binding (D-09)"
  - "Plain-text version pins in Prerequisites bullets to satisfy plan's literal-byte grep contract (Rule 3 auto-fix — backtick-wrapped 'gpg' / 'gh' / 'cosign' broke `grep -q 'gpg 2\\.4'` style checks; switched to bare-token form)"
metrics:
  duration_minutes: 7
  duration_seconds: 420
  tasks_completed: 2
  files_modified: 1
  completed: 2026-06-02
---

# Phase 24 Plan 02: Create docs/RELEASING.md maintainer-side release procedure — Summary

Created `docs/RELEASING.md` (NEW file, 245 lines) documenting the maintainer-side release procedure with H1 + audience-disambiguation lede + 8 H2 sections in the RESEARCH §5.5 locked ordering. Documents the 5-step per-release procedure (tag → wait for CI → download → PGP sign on YubiKey → upload .asc + flip draft), the pre-flight cosign verify gate before the draft flip, the one-time PGP key generation YubiKey ed25519 ceremony, the 2-year key rotation procedure with cross-signing, the emergency key revocation procedure, and the publication procedures for both `keys.openpgp.org` and WKD direct-method on `<owner>.github.io`. All fingerprint references use the `<FINGERPRINT-TBD>` placeholder (8 occurrences) and the `<new-FINGERPRINT-TBD>` placeholder for future-rotation prose (3 occurrences) — Plan 24-05 replaces all `<FINGERPRINT-TBD>` atomically when the maintainer generates the key.

Phase 24 documents these procedures; it does NOT execute them. The maintainer's first execution is the v1.6.0-rc.0 cut.

## What Got Built

### `docs/RELEASING.md` (created) — final shape

Section-by-section line ranges in the final file:

| Range | Element | Origin |
|-------|---------|--------|
| 1-7 | H1 (`# Releasing blindjoin`) + audience-disambiguation lede + `<FINGERPRINT-TBD>` placeholder-convention note | Task 1 |
| 9-23 | `## Prerequisites` — 5-item bullet list: YubiKey 5 (≥ 5.2.3 fw), gpg 2.4+, gh 2.50+, cosign 2.6.3+, `<owner>.github.io` repo | Task 1 |
| 25-58 | `## Per-release procedure (5 steps)` — numbered procedure with literal `git tag -s` / `gh release download` / `gpg --detach-sign --armor --local-user <FINGERPRINT-TBD>` / `gh release upload .asc` / `gh release edit --draft=false` commands | Task 1 |
| 60-98 | `## Pre-flight check before flipping out of draft` — cosign verify-blob + verify-attestation recipes + `gh release delete` recovery path | Task 1 |
| 100-141 | `## PGP key generation (one-time, NOT a release-cut step)` — 5-step YubiKey ed25519 ceremony per RESEARCH §5.1 (gpg --card-edit → admin → generate; revoke.asc + shred; export to docs/pgp/; cross-refs to keys.openpgp.org + WKD sections) + operator-side public-key binding verification sub-paragraph | Task 2 |
| 143-166 | `## PGP key rotation (every 2 years)` — 6-step procedure per RESEARCH §5.2 (6-months-before generation, cross-sign with old key via `gpg --sign-key <new-FINGERPRINT-TBD>`, commit new .asc alongside old, update SECURITY.md `<a id="pgp-current">` anchor, publish to keys.openpgp.org + WKD, CHANGELOG entry on rotation) | Task 2 |
| 168-189 | `## PGP key revocation (emergency — YubiKey lost or compromised)` — recover offline revoke.asc, `gpg --import revoke.asc`, publish revocation to keys.openpgp.org + WKD, run full key generation for new key (skip cross-sign — old key is revoked), document in CHANGELOG | Task 2 |
| 191-199 | `## Publishing the key to keys.openpgp.org` — `gpg --send-keys --keyserver hkps://keys.openpgp.org <FINGERPRINT-TBD>` + email confirmation flow gate prose | Task 2 |
| 201-245 | `## Publishing the key to WKD` — 5-step direct-method procedure per RESEARCH §5.3: verify `<owner>.github.io` repo (with `gh repo create` fallback), compute `gpg-wks-client --print-wkd-hash johnturner@gmail.com`, binary keyring export, `.well-known/openpgpkey/hu/<WKD_HASH>` commit, WKD resolution test + closing prose on refresh cadence + direct-vs-subdomain method (subdomain NOT viable on `*.github.io`) | Task 2 |

### `<FINGERPRINT-TBD>` placeholder occurrence count

Plan 24-05 reads this number to verify atomic replacement covers every site:

| Placeholder | Occurrences | Notes |
|-------------|-------------|-------|
| `<FINGERPRINT-TBD>` | 8 | Replaced atomically when maintainer generates key (Plan 24-05 contract) |
| `<new-FINGERPRINT-TBD>` | 3 | Future-rotation prose; NOT replaced at v1.6.0-rc.0 cut (this is documentation of the rotation flow, not an active rotation) |

Plan 24-05's atomicity contract MUST replace all 8 `<FINGERPRINT-TBD>` occurrences with the maintainer's actual 40-char hex fingerprint in one commit. The 3 `<new-FINGERPRINT-TBD>` occurrences STAY as-is — they are part of the rotation procedure prose and will only be replaced at the next 2-year rotation cycle (when the maintainer chooses a fresh placeholder set if desired).

### Cross-references made from `docs/RELEASING.md`

| Target | Status | Notes |
|--------|--------|-------|
| `../CONTRIBUTING.md` | EXISTS — Phase 24 Plan 24-04 will INSERT a one-line cross-ref back from CONTRIBUTING.md to `docs/RELEASING.md` (the reverse link) | Phase 24 audience-disambiguation lede; mirrors Plan 24-04 D-20 |
| `../.github/workflows/release.yml` | EXISTS (Phase 24 Plan 24-01 final state) | Per-release step 2 link target — "Watch [`release.yml`](../.github/workflows/release.yml) in the Actions tab" |
| `docs/pgp/<FINGERPRINT-TBD>.asc` | PENDING — Plan 24-05 creates this file with the maintainer's actual fingerprint at the v1.6.0-rc.0 cut | Self-verifying filename = full 40-char fingerprint (D-09) |
| `SECURITY.md#pgp-current` | PENDING — Plan 24-03 inserts the `<a id="pgp-current"></a>` anchor + current-fingerprint prose | Cross-referenced in `## PGP key rotation` step 4 (update the anchor to name new fingerprint) |
| `../CONTRIBUTING.md#tagging-releases` | EXISTS — CONTRIBUTING.md `## Tagging releases` section at lines 69-94 | Per-release procedure step 1 cites the 3-part semver gate |

All cross-references resolve to files that EITHER already exist in-tree OR are created by Plans 24-01 / 24-03 / 24-05 per the Phase 24 plan structure. Plan 24-02 introduces no broken cross-references; the only unresolved targets are documented as PENDING above and have explicit creator-plan assignments.

### No real fingerprint hex strings

Verified empty: `grep -qE '\b[A-F0-9]{40}\b' docs/RELEASING.md` returns no match. Every fingerprint reference uses `<FINGERPRINT-TBD>` or `<new-FINGERPRINT-TBD>` placeholders. Plan 24-05's acceptance criterion `! grep -q '<FINGERPRINT-TBD>' docs/RELEASING.md` (after replacement) will tell the maintainer whether atomic replacement succeeded.

### Cross-reference to Plan 24-04 (CONTRIBUTING.md insertion)

Plan 24-04 inserts a one-line cross-ref into CONTRIBUTING.md `## Tagging releases` pointing back to `docs/RELEASING.md`. Plan 24-02's audience-disambiguation lede is the forward direction; Plan 24-04's CONTRIBUTING.md addition is the reverse direction. Both directions together close the audience-disambiguation loop documented in CONTEXT D-11 + D-20.

### Cross-reference to Plan 24-05 (atomic placeholder replacement at v1.6.0-rc.0 cut)

Plan 24-05 is the `checkpoint:human-verify` task that the maintainer drives. Its atomic contract:

1. Maintainer generates the ed25519 PGP key on YubiKey per `docs/RELEASING.md ## PGP key generation` (Plan 24-02 documents the procedure).
2. Maintainer exports the public key to `docs/pgp/<actual-fingerprint>.asc` (Plan 24-05 creates the file).
3. Plan 24-05 replaces all 8 `<FINGERPRINT-TBD>` occurrences in `docs/RELEASING.md` AND all `<FINGERPRINT-TBD>` occurrences in `SECURITY.md` (Plan 24-03's anchor + fingerprint prose) with the maintainer's actual 40-char hex fingerprint in one commit.
4. Plan 24-05 commits all three changes (`.asc` creation + 2 file rewrites) atomically.

After Plan 24-05 lands, the maintainer's first action (NOT a Phase 24 commit; the v1.6.0-rc.0 release procedure rehearsal) is to publish the key to keys.openpgp.org + WKD per the procedures Plan 24-02 documents.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking issue] Replaced backtick-wrapped Prerequisites version pins with plain-text form**

- **Found during:** Task 1 verification
- **Issue:** Initial draft of the Prerequisites bullets used backtick-wrapped tool names: `` `gpg` 2.4+ ``, `` `gh` 2.50+ ``, `` `cosign` 2.6.3+ ``. The plan's automated acceptance criteria use literal-byte greps without backticks: `grep -q 'gpg 2\.4' docs/RELEASING.md`, `grep -q 'gh 2\.50' docs/RELEASING.md`, `grep -q 'cosign 2\.6\.3' docs/RELEASING.md`. These all failed because the backtick character broke the contiguous-byte match.
- **Fix:** Switched the Prerequisites bullets to bare-token form: `**YubiKey 5 ...**`, `**gpg 2.4+** on the maintainer's machine.`, `**gh 2.50+** on the maintainer's machine.`, `**cosign 2.6.3+** for the pre-flight verify gate ...`. Markdown bold styling preserved; backticks removed only from the version-pin tokens that the plan's grep contract reads.
- **Files modified:** docs/RELEASING.md (3 lines edited)
- **Commit:** Folded into Task 1 commit `af11734` before commit

This mirrors Plan 22-04's "auditor-grepable" pattern — when literal-byte greps form the acceptance contract, the source-file representation MUST match the contract at the byte level even when human-readability would prefer backtick-styled identifiers. PATTERNS-style precedent extends to docs-modify plans, not just workflow-modify plans.

### Authentication gates

None encountered. This plan creates one Markdown file via Write + Edit; no external services, no installs, no commits requiring credentials beyond the standard git commit gate.

## Threat Surface Scan

No new security-relevant surface introduced. The file documents existing trust boundaries (YubiKey ↔ host, host ↔ keys.openpgp.org, host ↔ WKD on `<owner>.github.io`) per the plan's `<threat_model>` STRIDE register; the file itself is pure documentation and introduces no new endpoints, auth paths, or schema changes.

No `threat_flag:` entries to add.

## Known Stubs

None. The `<FINGERPRINT-TBD>` placeholder is NOT a stub — it is a literal placeholder string with an explicit replacement contract (Plan 24-05). The plan intentionally leaves the fingerprint un-filled because the key has not been generated yet (per CONTEXT D-08 + RESEARCH §6 deliverable-ordering recommendation).

## Self-Check: PASSED

- File `docs/RELEASING.md` exists at the canonical path with UPPERCASE filename matching `AUDIT-CHARTER.md` + `PROTOCOL.md` precedent: `test -f docs/RELEASING.md` → found.
- All 8 H2 sections present in the locked ordering (Prerequisites → Per-release procedure → Pre-flight check → PGP key generation → PGP key rotation → PGP key revocation → Publishing the key to keys.openpgp.org → Publishing the key to WKD): `awk` H2-ordering check returns exit 0.
- All literal commands from RESEARCH §5.5 verbatim: `git tag -s vX.Y.Z`, `gh release download vX.Y.Z`, `gpg --detach-sign --armor --local-user <FINGERPRINT-TBD>`, `gh release upload vX.Y.Z blindjoin-linux-amd64.tar.gz.asc`, `gh release edit vX.Y.Z --draft=false`, `gpg --card-edit`, `gpg --output revoke.asc --gen-revoke`, `shred -u revoke.asc`, `gpg --export --armor <FINGERPRINT-TBD> > docs/pgp/<FINGERPRINT-TBD>.asc`, `gpg --sign-key <new-FINGERPRINT-TBD>`, `gpg --import revoke.asc`, `gpg --send-keys --keyserver hkps://keys.openpgp.org <FINGERPRINT-TBD>`, `gpg-wks-client --print-wkd-hash johnturner@gmail.com`, `gpg --auto-key-locate wkd --locate-keys johnturner@gmail.com`, `gh repo create <owner>.github.io` — all verified by `grep -q`.
- No real 40-char hex fingerprint strings: `! grep -qE '\b[A-F0-9]{40}\b' docs/RELEASING.md` → exit 0 (no match).
- No SKS-style keyservers recommended: `! grep -q 'keyserver.ubuntu.com' docs/RELEASING.md` → exit 0.
- Subdomain method WKD correctly noted as non-viable on `*.github.io`: documented explicitly in both the WKD section's opening paragraph and the closing "Direct method vs subdomain method" sub-paragraph.
- `<FINGERPRINT-TBD>` occurrence count: 8 (≥ 5 plan minimum).
- Line count: 245 (≥ 150 plan minimum; within RESEARCH §5.5 estimate of 200-300 lines).
- All cross-references resolve: `../CONTRIBUTING.md` exists, `../.github/workflows/release.yml` exists, `SECURITY.md` exists (anchor pending Plan 24-03), `docs/pgp/<FINGERPRINT-TBD>.asc` pending Plan 24-05.
- Both commits exist: `af11734` (Task 1) + `9df8bc6` (Task 2) verified in `git log --oneline`.

## Commits

| Task | Commit | Subject |
|------|--------|---------|
| 1 | `af11734` | docs(24-02): create docs/RELEASING.md skeleton with per-release procedure |
| 2 | `9df8bc6` | docs(24-02): append PGP key lifecycle + WKD/keys.openpgp.org publication sections |
