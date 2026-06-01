# Research Synthesis — v1.6 Supply-Chain Attestation

**Milestone:** v1.6 — Close the v1.5 unsigned-build supply-chain gap explicitly named in SECURITY.md § Supply-chain status.
**Synthesized:** 2026-06-01
**Inputs:** STACK.md, FEATURES.md, ARCHITECTURE.md, PITFALLS.md (all v1.6; v1.4 prior research archived to `.planning/milestones/v1.4-research-archive/`).
**Note:** The 4 parallel research agents hit API socket errors after ~22 min wallclock each without writing output; this synthesis was produced inline by the orchestrator drawing on the SECURITY.md design contract + ecosystem knowledge of sigstore/cosign/SLSA/Rust reproducibility, then fact-anchored against the actual workflow files in `.github/workflows/`.

---

## Stack additions

| Component | Pin | Purpose |
|---|---|---|
| **cosign** | 2.x (≥ 2.5) | Keyless OIDC signing of images + blobs; SLSA v1.0 attestations |
| **sigstore/cosign-installer** | `@v3` (SHA-pinned at adoption) | GHA wrapper to install cosign on the runner |
| **actions/attest-build-provenance** | latest (SHA-pinned at adoption) | GitHub-maintained wrapper that emits SLSA Level 3 provenance attestations — simpler than the slsa-github-generator reusable workflow |
| **`--remap-path-prefix` + `SOURCE_DATE_EPOCH` + `--locked`** | stable Rust | Strip the main sources of binary nondeterminism for reproducible release tarballs |

**NOT added:** Notation, slsa-github-generator reusable workflow (Pitfall 5), cargo-zigbuild (out of scope at single-target ship), Renovate (overkill for 2-image digest watch).

## Feature table stakes (per category)

**Category 1 — cosign image attestations:**
- Every ghcr.io image signed via OIDC keyless flow
- SLSA v1.0 build-level-3 provenance attached
- One documented `cosign verify` command in SECURITY.md

**Category 2 — Release tarball detached signatures:**
- `.bundle` (or `.sig` + `.crt`) companion files for every release tarball
- Sigstore Rekor inclusion (auditable transparency log)
- One documented `cosign verify-blob` command in SECURITY.md

**Category 3 — Reproducible-build recipe:**
- `docs/REPRODUCIBLE-BUILD.md` with exact toolchain + runner image + env vars + expected sha256
- release.yml updated with `--locked` + `--remap-path-prefix` + `SOURCE_DATE_EPOCH`
- Scheduled `reproducible-verify.yml` workflow that periodically asserts byte-equality from a clean runner

**Category 4 — Automated digest drift detection:**
- `docker/digests.txt` canonical digest manifest (human-bumped only)
- `.github/workflows/digest-drift-check.yml` scheduled workflow that opens an issue (not a PR) on drift
- release.yml + docker.yml read digests.txt and pass `--build-arg DEBIAN_REF=...` automatically

## Phase mapping (suggested)

| Phase | Name | Categories | Lift |
|---|---|---|---|
| 22 | Digest drift detection | 4 | LOW. No operator-facing change; builds digest discipline before signing layers on top. |
| 23 | Image attestations + SLSA provenance | 1 | MEDIUM. Adds `id-token: write` + cosign-installer + attest-build-provenance to docker.yml. SECURITY.md draft update. |
| 24 | Release tarball signing | 2 | LOW. Mirrors Phase 23 patterns into release.yml. Reuses cosign-installer setup. |
| 25 | Reproducible-build recipe + scheduled verifier | 3 | MEDIUM. Recipe write + iteration cycles to fix sources of binary nondeterminism (Pitfall 6). |

Continued numbering from v1.5 Phase 21 → v1.6 starts at Phase 22.

## Watch Out For (top pitfalls — full list in PITFALLS.md)

1. **`cosign verify` identity regex** — exact `--certificate-identity` per-release is brittle; use `--certificate-identity-regexp` bound to the workflow file + tag namespace. (Pitfall 1)
2. **`id-token: write` permission scope** — opaque failure mode if missed. Job-level scope, not workflow-level. (Pitfall 2)
3. **SHA-pin sigstore actions** at adoption — extends the project's existing GHA pin discipline. (Pitfall 4)
4. **Rust reproducibility long tail** — `--remap-path-prefix` + `SOURCE_DATE_EPOCH` are necessary but rarely sufficient; first reproducibility run will surface project-specific nondeterminism that needs case-by-case fixes. (Pitfall 6)
5. **Digest-drift auto-merge is the supply-chain risk we're closing** — drift check opens an ISSUE (human review), never an auto-mergeable PR. (Pitfall 11)
6. **GHCR UI "Unverified" badge** is unrelated to cosign verification — explicitly document this in SECURITY.md. (Pitfall 10)
7. **HUMAN-UAT every documented cosign verify command from a fresh machine** before shipping the doc update. (Pitfall 12)
8. **cosign 3.0 CLI flag drift** — pin operator-side cosign version in the SECURITY.md verification recipe. (Pitfall 13)

## Operator-facing verification commands (final shape at v1.6 ship)

```bash
# Image verification
cosign verify \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/docker\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  ghcr.io/johnzilla/blindjoin-coordinator:1.6.0

# Release tarball verification
cosign verify-blob \
  --bundle blindjoin-linux-amd64.tar.gz.bundle \
  --certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/release\.yml@refs/tags/v.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  blindjoin-linux-amd64.tar.gz

# Reproducibility (independent rebuilder)
# See docs/REPRODUCIBLE-BUILD.md — run on ubuntu-24.04 with documented env;
# expected sha256 matches blindjoin-linux-amd64.tar.gz.sha256 in the GH Release.
```
