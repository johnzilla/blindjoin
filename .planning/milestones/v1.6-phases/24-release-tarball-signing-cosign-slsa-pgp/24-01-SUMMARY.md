---
phase: 24-release-tarball-signing-cosign-slsa-pgp
plan: 01
subsystem: ci-supply-chain
tags: [cosign, slsa, sigstore, release, attestation, signing, oidc]
requires:
  - Phase 23 sigstore-pin-check CI gate at ci.yml:292-326 (Plan 23-03)
  - sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 (Phase 23 docker.yml:151)
  - actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f (Phase 23 docker.yml:272)
  - softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 (existing release.yml:101 pre-modification)
provides:
  - blindjoin-linux-amd64.tar.gz.bundle (cosign blob signature, .bundle format) — SIGN-01
  - blindjoin-linux-amd64.tar.gz.sigstore (SLSA v1.0 in-toto provenance bundle) — SIGN-02
  - GitHub Attestations API record (gh attestation verify path) — SIGN-02
  - draft GitHub Releases (operators never see a Release missing the PGP .asc) — D-07
affects:
  - .github/workflows/release.yml (modified; +116 net lines)
tech-stack:
  added: []
  patterns:
    - sigstore keyless OIDC blob signing via cosign sign-blob --bundle
    - GitHub Attestations API push via actions/attest-build-provenance with subject-path
    - RESEARCH §3.2 bundle-path → mv → .sigstore deterministic-filename infrastructure pattern
    - Job-level permissions block with auditor-grepable paraphrased deliberately-omitted-scopes (Plan 22-04 lesson)
key-files:
  created: []
  modified:
    - .github/workflows/release.yml
decisions:
  - "Verbatim Phase 23 SHA reuse — single source of truth across release.yml + docker.yml (D-13)"
  - "RESEARCH §3.2 correction load-bearing — no output-name input on attest-build-provenance@v3.2.0; bundle-path output + mv step is the only viable wiring"
  - "Paraphrased 'OCI-registry-push' / 'output-name' tokens in comments to satisfy plan's literal-token-absence verify gates (Rule 1 auto-fix during execution)"
  - "Step name 'Upload to GitHub Releases (draft — maintainer flips out of draft after PGP upload)' makes operator-visible reason for draft state explicit at workflow-log level"
metrics:
  duration_minutes: 5
  duration_seconds: 300
  tasks_completed: 4
  files_modified: 1
  completed: 2026-06-02
---

# Phase 24 Plan 01: Wire release.yml for cosign + SLSA tarball signing — Summary

Wired `.github/workflows/release.yml`'s `build` job for SIGN-01 (cosign blob signature) and SIGN-02 (SLSA v1.0 provenance attestation) via four structural changes: explicit job-level permissions block, cosign-installer + sign-blob steps, attest-build-provenance + rename steps, and a softprops upload-step modification (draft mode + 4-file files list). Total diff: +116 net lines on a single workflow file. All work atomic across 4 commits; Phase 23's sigstore-pin-check CI gate at ci.yml:292-326 catches both new sigstore SHA pins automatically without any new gate.

## What Got Built

### `.github/workflows/release.yml` (modified) — final shape

Pre-modification: 107 lines. Post-modification: 222 lines. Net additions: 115 lines (comments + step bodies).

Step-by-step line ranges in the final file:

| Range | Element | Phase 24 Origin |
|-------|---------|------------------|
| 67-85 | Job-level `permissions:` block + auditor-grepable comment header | Task 1 |
| 119-130 | Install cosign step (`sigstore/cosign-installer@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` + `cosign-release: 'v2.6.3'`) | Task 2 |
| 132-150 | Sign tarball with cosign — SIGN-01 step (`cosign sign-blob --yes --bundle ...`) | Task 2 |
| 152-184 | Attest tarball build provenance — SIGN-02 step (`actions/attest-build-provenance@96278af6caaf10aea03fd8d33a09a777ca52d62f # v3.2.0` + `id: provenance` + `subject-path:`) | Task 3 |
| 186-199 | Rename provenance bundle to .sigstore step (`mv "${{ steps.provenance.outputs.bundle-path }}" blindjoin-linux-amd64.tar.gz.sigstore`) | Task 3 |
| 201-222 | Modified `Upload to GitHub Releases` step (`draft: true` + 4-file `files:` list in D-15 semantic order) | Task 4 |

The verbatim 3-scope permissions block + comment header (Plan 24-03's SECURITY.md will cross-reference this for the operator identity-regexp):

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
    permissions:
      contents: write
      id-token: write
      attestations: write
```

### File-level audit gate confirmations

All Phase 22 Plan 22-04 + Phase 24 audit invariants hold at file level post-edit:

```bash
$ python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/release.yml"))' && echo OK
OK
$ ! grep -q 'pull-requests:' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q '^[[:space:]]*packages:' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'no-tlog-upload' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'output-name:' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'push-to-registry:' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'slsa-framework/slsa-github-generator' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'blindjoin-linux-amd64.tar.gz.asc' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'prerelease: true' .github/workflows/release.yml && echo PASS
PASS
$ ! grep -q 'fail_on_unmatched_files' .github/workflows/release.yml && echo PASS
PASS
```

### Phase 23 sigstore-pin-check inheritance confirmed

Phase 23 Plan 23-03's `sigstore-pin-check` CI gate at `.github/workflows/ci.yml:292-326` greps every workflow under `.github/workflows/` (including `release.yml`) for the four target actions used without a 40-hex SHA pin. Both new sigstore `uses:` lines in `release.yml` are covered automatically:

```bash
$ PATTERN='uses:\s*(sigstore/cosign-installer|actions/attest-build-provenance|actions/attest-sbom|anchore/sbom-action)@(?![a-f0-9]{40})'
$ ! grep -PnE "$PATTERN" .github/workflows/release.yml && echo PASS
PASS
```

No new CI gate added; no changes to `ci.yml`. RESEARCH §2.3 inheritance pattern: Phase 23 establishes discipline; Phase 24 inherits.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Add job-level `permissions:` block + auditor-grepable comment header | `3a2bb21` | `.github/workflows/release.yml` |
| 2 | Insert cosign-installer + cosign sign-blob steps (SIGN-01) | `0215894` | `.github/workflows/release.yml` |
| 3 | Insert attest-build-provenance + bundle rename steps (SIGN-02) | `4eb5b3a` | `.github/workflows/release.yml` |
| 4 | Modify softprops/action-gh-release — `draft: true` + 4-file `files:` list (D-07 + D-15) | `38ea819` | `.github/workflows/release.yml` |

Total commits: 4. Total files modified: 1. Total auto-fix attempts: 1 (Task 3 — see Deviations).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Comment text contained literal forbidden tokens that tripped plan's verify gates**

- **Found during:** Task 3 verification
- **Issue:** The initial Task 3 comment body contained the literal strings `push-to-registry:` (the action input I was documenting as "not set") and a comment phrasing that mirrored the action.yml's full input list (which also contained `subject-name`, `subject-digest`, `push-to-registry`). The plan's `<verify>` block enforces `! grep -q 'push-to-registry:'` as a literal-token-absence gate — these literal tokens in COMMENTS still tripped it.
- **Fix:** Paraphrased the comment text so the forbidden tokens-with-colon never appear at the file level. Specifically: rewrote the input-list paragraph to say "exactly 8 inputs and 3 outputs including bundle-path" (instead of enumerating all 8 by name); rewrote the "push-to-registry NOT set" paragraph to use the phrase "OCI-registry-push input is NOT set" (with an explicit auditor-grepable note citing Plan 22-04 lesson). Tokens-without-colon (`output-name`, `bundle-name`) are still in the comments where load-bearing for the RESEARCH §3.2 correction story but do not trip the verify gate (which targets `output-name:` with colon).
- **Files modified:** `.github/workflows/release.yml`
- **Commit:** `4eb5b3a` (included with Task 3 — the fix was made before Task 3 was committed)
- **Pattern continuity:** Phase 22 Plan 22-04 established this paraphrasing-in-comments discipline for the deliberately-omitted-scopes pattern; Phase 24 extends it to attest-* action inputs that the planner forbids by file-level grep.

### Authentication gates

None — Task work was pure file editing against a local repository. No CI run was executed in this plan; first end-to-end CI rehearsal against the produced .bundle + .sigstore assets is deferred to Plan 24-05 (`checkpoint:human-verify` at first `v1.6.0-rc.0` tag push per Phase 23 closure pattern — CONTEXT §domain).

## Verification

All 11 plan-level `<verification>` gates pass post-Task-4 (1=YAML parse, 2=job-level permissions block scoped correctly, 3=file-level audit gates from Plan 22-04 + Phase 24 no-ghcr-push contract, 4=both sigstore SHA pins verbatim with two-space-before-# comment style, 5=cosign sign-blob command verbatim, 6=attest step has id: provenance + single subject-path input, 7=mv rename step references ${{ steps.provenance.outputs.bundle-path }}, 8=softprops draft: true + .bundle + .sigstore entries in files: list, 9=forbidden tokens absent at file level, 10=six-step ordering Package → Install cosign → Sign tarball → Attest → Rename → Upload, 11=sigstore-pin-check pattern accepts the file).

Plan-level success criteria from `<success_criteria>` block: all satisfied. The `softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844 # v1` SHA pin and `env: GITHUB_TOKEN:` wiring are untouched as required. The workflow-level `permissions: { contents: write }` block at lines 28-29, the `check` job (lines 32-58), and the tag gate `if: startsWith(github.ref, 'refs/tags/')` at line 66 are all untouched as required.

## Cross-Plan Coupling

| Coupling | This plan provides | Consumer plan |
|----------|---------------------|---------------|
| Operator identity-regexp `release\.yml@refs/tags/v.*` | Embedded as comment in cosign sign-blob step (release.yml:138) | Plan 24-03 (SECURITY.md operator verify recipes block) |
| `.bundle` Release asset (SIGN-01 output) | Produced by `cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle` (release.yml:150); uploaded by softprops step (release.yml:219) | Plan 24-03 (`cosign verify-blob --bundle` recipe); Plan 24-05 (checkpoint:human-verify at first v1.6.0-rc.0 — empirical cosign verify-blob from a fresh machine) |
| `.sigstore` Release asset (SIGN-02 output) | Produced by `actions/attest-build-provenance` then renamed via `mv "${{ steps.provenance.outputs.bundle-path }}"` (release.yml:199); uploaded by softprops step (release.yml:220) | Plan 24-03 (`cosign verify-attestation --bundle ...sigstore --type slsaprovenance` recipe); Plan 24-05 (checkpoint:human-verify — empirical attestation verify) |
| `draft: true` state on every Release | softprops step (release.yml:215) | Plan 24-04 (`docs/RELEASING.md` maintainer-side `gh release edit v1.6.0 --draft=false` flip after `.asc` upload); Plan 24-02 (docs/RELEASING.md prerequisite) |
| Phase 23 sigstore-pin-check covers both new pins | Verified post-edit via the same regex Plan 23-03 baked into ci.yml | No new CI work in Phase 24 (no Plan 24-XX touches ci.yml) |

## Plan 24-05 Cross-Reference

Plan 24-05 (`checkpoint:human-verify`) is the empirical validation of this plan's CI deliverables — at the first `v1.6.0-rc.0` tag push, the maintainer runs the `<verification>`-block commands against the just-produced GitHub Release assets from a fresh `ubuntu:24.04` container: `cosign verify-blob --bundle blindjoin-linux-amd64.tar.gz.bundle ...`, `cosign verify-attestation --bundle blindjoin-linux-amd64.tar.gz.sigstore ...`, `gh attestation verify blindjoin-linux-amd64.tar.gz --repo <owner>/blindjoin`. The YubiKey ceremony (Plan 24-02 / docs/RELEASING.md PGP key generation) happens at the same rc.0 cut. Plan 24-05 also replaces the `<FULL-40-CHAR-FINGERPRINT>` placeholders in `docs/pgp/` and `SECURITY.md` with the real fingerprint generated at that ceremony (Option C from RESEARCH §6).

## Self-Check: PASSED

Created files verified to exist on disk and commits verified to exist in `git log --oneline --all`:

```
FOUND: .github/workflows/release.yml (modified)
FOUND: 3a2bb21 (Task 1 commit)
FOUND: 0215894 (Task 2 commit)
FOUND: 4eb5b3a (Task 3 commit)
FOUND: 38ea819 (Task 4 commit)
```

SUMMARY itself will be committed in the final metadata commit alongside STATE.md + ROADMAP.md updates.
