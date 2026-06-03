# Phase 24: Release Tarball Signing (cosign + SLSA + PGP) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-02
**Phase:** 24-release-tarball-signing-cosign-slsa-pgp
**Areas discussed:** PGP signing trust model, PGP key custody + generation, SLSA provenance distribution, Cosign blob signature format

---

## Gray-area selection

| Option | Description | Selected |
|--------|-------------|----------|
| PGP signing trust model | Where signing happens (CI vs maintainer-local vs hybrid subkey) | ✓ |
| PGP key custody + generation | Where master private key lives + key scope | ✓ |
| SLSA provenance distribution for tarballs | GH Attestations API vs Release asset vs both | ✓ |
| Cosign blob signature format | `.bundle` vs discrete `.sig`+`.crt` | ✓ |

**User's choice:** All four areas selected.
**Notes:** No additional areas added at the end-of-discussion checkpoint.

---

## PGP signing trust model

### Q1: Where does the PGP signature get produced?

| Option | Description | Selected |
|--------|-------------|----------|
| Maintainer-local sign + upload (Recommended) | CI never sees private key; maintainer manually signs + uploads `.asc` post-CI | ✓ |
| CI-managed (GH Secret) | Encrypted private key + passphrase in GH Secrets; release.yml signs | |
| Hybrid signing-subkey in CI | Master offline; signing subkey in GH Secrets | |

**User's choice:** Maintainer-local sign + upload.
**Notes:** Highest security; the entire SIGN-03 "non-OIDC alternative" rationale is that operators who don't trust GitHub/Sigstore can still verify — putting key in GH Secrets defeats that. Trade-off: ~60s manual step per release; acceptable for solo project.

### Q2: Release lifecycle — when does `.asc` arrive relative to publish?

| Option | Description | Selected |
|--------|-------------|----------|
| Draft until signed (Recommended) | softprops `draft: true`; maintainer flips after `.asc` upload | ✓ |
| Publish immediately, .asc arrives later | Race window where Release has no PGP | |
| Pre-release until signed | softprops `prerelease: true`; flip after | |

**User's choice:** Draft until signed.
**Notes:** Operators never see an incomplete release. Cost is one extra `gh release edit --draft=false` after `.asc` upload.

### Q3: Where does maintainer-side signing procedure get documented?

| Option | Description | Selected |
|--------|-------------|----------|
| New docs/RELEASING.md (Recommended) | Canonical maintainer doc; SECURITY.md stays operator-focused | ✓ |
| Section in CONTRIBUTING.md | Append `## Release procedure`; mixes audiences | |
| Inline in SECURITY.md | Sign + verify next to each other; mixes maintainer/operator | |

**User's choice:** New docs/RELEASING.md.

---

## PGP key custody + generation

### Q1: How is the master PGP signing key stored?

| Option | Description | Selected |
|--------|-------------|----------|
| YubiKey / hardware token (Recommended) | Private key never leaves hardware; touch-confirm per sign | ✓ |
| Encrypted keyfile + password manager | Cheaper; software key custody | |
| Existing maintainer PGP key (reuse) | Fastest; entangles personal + project identity | |

**User's choice:** YubiKey / hardware token.

### Q2: Fresh key dedicated to blindjoin, or existing/general-purpose?

| Option | Description | Selected |
|--------|-------------|----------|
| Fresh project-scoped key (Recommended) | New ed25519, blindjoin-scoped User-ID | ✓ |
| Existing personal PGP key | Reuse personal key; entangles identity | |
| Fresh key, but no separate User-ID | Same UID as personal key; ambiguous on keyservers | |

**User's choice:** Fresh project-scoped key.

### Q3: Public key file location + naming convention in repo?

| Option | Description | Selected |
|--------|-------------|----------|
| docs/pgp/<FULL-FINGERPRINT>.asc (Recommended) | 40-char fingerprint = unambiguous identity; matches roadmap | ✓ |
| docs/pgp/maintainer.asc + fingerprint in SECURITY.md | Stable filename across rotations | |
| docs/pgp/<short-id-16>.asc | Shorter; collision-attackable | |

**User's choice:** docs/pgp/<FULL-FINGERPRINT>.asc.

### Q4: Public-key publishing channels (multiSelect)?

| Option | Description | Selected (initial) |
|--------|-------------|----------|
| keys.openpgp.org (Recommended; roadmap-required) | Modern keyserver; SC#3 mandate | |
| WKD (Web Key Directory) on github.io | `gpg --auto-key-locate wkd` resolves automatically | ✓ |
| keyserver.ubuntu.com (SKS-style) | Belt-and-suspenders; poisoning risk | |

**Initial selection:** WKD only — flagged conflict with roadmap SC#3.

**Clarification turn:** User challenged the premise ("openpgp.org has no keyserver"). Claude clarified that openpgp.org (the standard's homepage) and keys.openpgp.org (the modern free keyserver) are distinct sites despite the shared root domain; roadmap text says literally `keys.openpgp.org`.

### Q4 (reconciled): With clarification, how to handle publishing?

| Option | Description | Selected |
|--------|-------------|----------|
| Both: WKD + keys.openpgp.org (Recommended) | Honor roadmap + ship WKD as cleaner UX | ✓ |
| Replace: WKD only — update roadmap | Edit ROADMAP.md SC#3 + REQUIREMENTS SIGN-03 | |
| Replace: keys.openpgp.org only — drop WKD | Stay strictly with roadmap | |
| Verify keys.openpgp.org first | Pause + web-fetch | |

**User's choice:** Both — WKD + keys.openpgp.org.

---

## SLSA provenance distribution for tarballs

### Q1: Where does SLSA provenance live for operators?

| Option | Description | Selected |
|--------|-------------|----------|
| Both: GitHub Attestations API + .sigstore Release asset (Recommended) | Two verification UX paths; covers github.com + offline | ✓ |
| GitHub Attestations API only | `gh attestation verify`; air-gapped operators lose provenance | |
| .sigstore bundle as Release asset only | Skips GH API path | |

**User's choice:** Both API + Release asset.

### Q2: Filename for the SLSA `.sigstore` bundle Release asset?

| Option | Description | Selected |
|--------|-------------|----------|
| blindjoin-linux-amd64.tar.gz.sigstore (Recommended) | Mirrors existing `.sha256` sibling convention | ✓ |
| blindjoin-linux-amd64.tar.gz.intoto.jsonl | SLSA-traditional; less familiar | |
| blindjoin-linux-amd64.tar.gz.provenance.sigstore | Disambiguates from cosign `.bundle` | |

**User's choice:** `.tar.gz.sigstore` suffix.

---

## Cosign blob signature format

### Q1: Cosign blob signature distribution format for the tarball?

| Option | Description | Selected |
|--------|-------------|----------|
| .bundle (Recommended) | Single file with sig+cert+Rekor proof; mirrors Phase 23 image-side | ✓ |
| Discrete .sig + .crt | Two assets; works for older cosign CLIs | |
| Both .bundle AND discrete .sig + .crt | Three assets; clutter | |

**User's choice:** `.bundle` format.

---

## End-of-discussion checkpoint

| Option | Description | Selected |
|--------|-------------|----------|
| I'm ready for context | Write 24-CONTEXT.md and proceed to plan-phase | ✓ |
| Explore more gray areas | Identify 2-3 additional gray areas | |

**User's choice:** Ready for context.

---

## Claude's Discretion (per CONTEXT.md `<decisions>` D-13 through D-20)

- D-13: SHA pins for new `uses:` lines — reuse Phase 23's SHAs for sigstore actions
- D-14: `actions/attest-build-provenance` output wiring for `.sigstore` filename — confirm exact input name against SHA-pinned version
- D-15: `softprops/action-gh-release` files list ordering — semantic grouping
- D-16: `gh attestation verify` command shape in SECURITY.md — spot-check against current gh CLI
- D-17: Key-rotation cadence + procedure prose in docs/RELEASING.md — write the procedure, do not execute it
- D-18: softprops `draft: true` support at the pinned version + flip wording — confirm + pick UX
- D-19: WKD setup steps in docs/RELEASING.md — document directory tree + `gpg-wks-client` helper
- D-20: Cross-references between CONTRIBUTING.md and docs/RELEASING.md — one-line cross-ref, natural insertion point

## Deferred Ideas

(Full list in CONTEXT.md `<deferred>` section; summary here.)

- CI-managed PGP signing (rejected; reconsider only if co-maintainer onboards)
- Hybrid signing-subkey in CI (overkill for solo; revisit with co-maintainer)
- SKS-style keyserver upload (poisoning risk; low marginal value)
- PGP encryption subkey (signing-only project)
- Sigstore TUF root pre-seeding doc (1-liner OK; defer expansion to v1.7)
- Cosign 3.0 migration doc (single quick task when cosign 3.0 lands)
- PGP key rotation execution (Phase 24 documents; maintainer executes)
- Per-architecture tarballs (linux-amd64 only today; v1.7+ if demand surfaces)
- `reproducibility-regression` post-release verifier (Phase 25's seat)
- Web-of-Trust signatures on the maintainer's key (defer indefinitely)
