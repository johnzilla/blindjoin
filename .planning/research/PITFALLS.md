# PITFALLS — v1.6 Supply-Chain Attestation

**Domain:** Adding cosign image attestations, cosign blob signing, reproducible builds, and automated digest drift checks to blindjoin v1.5's release pipeline.
**Researched:** 2026-06-01
**Overall confidence:** HIGH on cosign/sigstore failure modes (well-documented community failures since 2022); MEDIUM on Rust reproducibility long tail (project-specific); HIGH on GHA OIDC quirks.

---

## Pitfall 1 — cosign verify identity regex too narrow (or too wide)

**Bites because:** `cosign verify --certificate-identity` exact-matches the SAN claim from the Fulcio cert. The SAN includes the workflow file path + ref. If you write the verification command in SECURITY.md with `--certificate-identity 'https://github.com/johnzilla/blindjoin/.github/workflows/docker.yml@refs/tags/v1.6.0'`, it works for v1.6.0 only. Every subsequent release breaks the documented command.

**Prevention strategy:** Use `--certificate-identity-regexp 'https://github.com/johnzilla/blindjoin/\.github/workflows/(docker|release)\.yml@refs/tags/v.*'` — narrow enough to bind to the specific workflows + tag namespace, wide enough to survive across releases. Document the regex in SECURITY.md, not a specific identity.

**Don't go too wide:** `--certificate-identity-regexp 'https://github.com/johnzilla/.*'` is exploitable — any workflow in any of your repos can produce a "valid" signature for blindjoin. Bind to the workflow file.

**Phase:** Phase 23 (cosign image attestations) — write the canonical regex, use it in both Phase 23 and Phase 24 docs.

---

## Pitfall 2 — Forgetting `id-token: write` permission scope

**Bites because:** cosign keyless signing requires a GitHub OIDC token, which requires `id-token: write` permission on the job. The default `permissions:` for a workflow run is `read-all`; cosign sign will fail with `signing keyless: getting signer: getting signer: fulcio: 400 Bad Request` — opaque error that doesn't immediately point at the permission.

**Prevention strategy:** Add `id-token: write` to the specific job (not workflow-wide — narrower scope is better). Add a comment line above the permissions block explaining what each scope is for. CI smoke-test on workflow_dispatch BEFORE the first tagged release uses it.

**Phase:** Phase 23 — first phase that needs OIDC. Phase 24 inherits the pattern.

---

## Pitfall 3 — Rekor transparency log is mandatory (and what to do when it's not available)

**Bites because:** cosign sign defaults to logging the signature to public Rekor. If the runner can't reach Rekor (network flake, sigstore outage, airgapped runner), `cosign sign` hangs or fails. Operators verifying ALSO consult Rekor by default — `cosign verify` without internet → fails.

**Prevention strategy:**
- Signer side: don't set `--no-tlog-upload` (default behavior is correct — log to Rekor).
- Verifier side: document the standard verify command in SECURITY.md, AND document `--insecure-ignore-tlog` for airgapped operators with an explicit risk callout ("Skipping Rekor verification means a future attacker who compromises your local Fulcio root could backdate a signature").
- For the project's release pipeline itself: if `cosign sign` fails on a release because Rekor is down, the right move is to RETRY, not to skip logging. Document this in the release runbook.

**Phase:** Phase 23 + Phase 24 (cosign signing surfaces).

---

## Pitfall 4 — SHA pin discipline on sigstore actions

**Bites because:** `sigstore/cosign-installer@v3` is a floating tag. The sigstore team releases frequently; a v3.1 → v3.2 transition can quietly change cosign install behavior. The project's existing GHA discipline (everything SHA-pinned) must extend to new actions.

**Prevention strategy:** At adoption, pin every new action by SHA with a `# v3.X.Y` comment. Add a CI grep gate (mirroring the existing `crit-01-grep-check` pattern) that fails if any action ref doesn't have the form `@<40-hex>` — catches future regressions.

Add the new sigstore actions to a tracked list in `SECURITY.md` so dependabot/renovate-style bumps are reviewed against the list.

**Phase:** Phase 23 — first sigstore action adoption. Document the pin discipline; reuse in Phase 24, Phase 25.

---

## Pitfall 5 — `actions/attest-build-provenance` vs `slsa-framework/slsa-github-generator` confusion

**Bites because:** Two paths to SLSA provenance on GHA:
- `actions/attest-build-provenance` — maintained by GitHub, simpler API, lives inside the existing job.
- `slsa-framework/slsa-github-generator` — original sigstore-community path, requires using a REUSABLE workflow (different YAML shape — `uses: slsa-framework/...` at workflow level, not action level).

Mixing them silently produces two competing attestations; verifiers may pick the wrong one.

**Prevention strategy:** Pick ONE. **Recommend `actions/attest-build-provenance`** for v1.6 — simpler integration with existing matrix-style `docker.yml`, no workflow restructure. Documentation in this phase's PLAN.md must name this choice + cite the rationale (SECURITY.md should explain what the verifier downloads + how to read it).

**Phase:** Phase 23 — locked at planning time.

---

## Pitfall 6 — Rust reproducible-build long tail

**Bites because:** Even with `--remap-path-prefix` + `SOURCE_DATE_EPOCH` + `--locked`, Rust binaries can fail bit-for-bit reproducibility due to:
- `proc-macro` crates that use `Instant::now()` at compile time (rare but real)
- `build.rs` scripts that consult `env::current_dir()` or `chrono::Local::now()`
- LLVM optimizations on certain targets producing nondeterministic ordering
- Cargo's incremental compilation interfering — must use a clean target dir
- Random hash for `dyn Trait` vtable inclusion order

**Prevention strategy:**
- First reproducibility run on a clean runner: capture EVERY diff between two builds. Most are fixable; a few are upstream-blocked.
- Add `CARGO_INCREMENTAL=0` to release.yml's env (already implicit on CI but make explicit).
- Use `diffoscope` to diagnose binary diffs (it's the standard reproducibility tool).
- Accept the realistic v1.6 target: bit-for-bit reproducibility ON `ubuntu-latest` (same image SHA) with documented env. Cross-distro reproducibility is v1.7+.
- Document KNOWN sources of nondeterminism in `docs/REPRODUCIBLE-BUILD.md` so a failed verifier rebuild can be triaged ("did one of these change?") instead of being treated as a supply-chain compromise.

**Phase:** Phase 25 — entirely.

---

## Pitfall 7 — Reproducibility verifier false-positives on time-of-build env drift

**Bites because:** The scheduled `reproducible-verify.yml` rebuilds the v1.6.0 tarball on a runner image SHA that may have drifted since the original ship. `ubuntu-latest` is a moving target — the runner image rotates roughly monthly. A monthly verifier run may rebuild on a NEW runner image and find a diff that's not a supply-chain issue, just a runner update.

**Prevention strategy:**
- Pin the runner image SHA in `docs/REPRODUCIBLE-BUILD.md` AND in `reproducible-verify.yml`'s `runs-on: ubuntu-24.04` (explicit version, not `ubuntu-latest`). 
- When `ubuntu-24.04` itself is upgraded by GH, that's a documented breaking event — the verifier should fail, the maintainer should re-confirm reproducibility on the new image, and `REPRODUCIBLE-BUILD.md` should be updated.
- The verifier's failure message must distinguish "runner image drift" from "actual sha256 mismatch on identical env".

**Phase:** Phase 25.

---

## Pitfall 8 — Digest-drift false positives from Debian security backports

**Bites because:** `debian:bookworm-slim` gets retagged when Debian releases security-backport patches (e.g. CVE in `apt`). The digest moves but the tag still points at "bookworm-slim". The drift check fires every time — operators tune it out → real supply-chain drift is also tuned out.

**Prevention strategy:**
- Drift check OPENS AN ISSUE — does not block CI. Issue title includes the previous + current digest + a link to the registry. Human reviews, decides whether to bump `docker/digests.txt` or investigate.
- For false-positive desensitization: classify drift severity — if the diff is only in `usr/share/doc/` or `/var/lib/dpkg/`, it's a docs/metadata-only retag (low-severity). If `libc6` or `openssl` versions changed, it's substantive (high-severity).
- Severity classification is OPTIONAL for v1.6; defer to a follow-on if Phase 22 ships and the maintainer sees too many low-severity issues.

**Phase:** Phase 22.

---

## Pitfall 9 — Digest-drift check auto-opens duplicate issues

**Bites because:** The workflow runs daily. If drift persists (and the maintainer hasn't bumped digests.txt yet), each daily run opens a new issue. Issue spam → maintainer mutes notifications → next real drift event missed.

**Prevention strategy:**
- Before opening an issue, `gh issue list --label digest-drift --state open --search "<digest-hex>"` — if an issue already exists for the same image + new-digest pair, skip.
- Issue title format that's machine-parseable: `[digest-drift] <image>:<tag> moved to sha256:<HEX>`. The check greps for this exact form.
- Document this in the workflow file with a comment block.

**Phase:** Phase 22.

---

## Pitfall 10 — GHCR "Unverified" UI badge confusion

**Bites because:** GHCR has its own image-signing UI notion separate from cosign. A cosign-signed image may still display as "Unverified" on the GHCR web UI because GHCR's verification doesn't consult Rekor by default. Operators see "Unverified" and conclude the supply chain is broken.

**Prevention strategy:**
- Document explicitly in SECURITY.md that the cosign verify CLI is the source of truth, NOT the GHCR UI badge.
- Operators should NOT rely on GHCR's UI for signature confirmation; they should run `cosign verify` directly.
- This is a UX-not-cryptographic gap; GitHub may add cosign-aware UI in the future, at which point this callout becomes obsolete.

**Phase:** Phase 23 — covered in the SECURITY.md draft update.

---

## Pitfall 11 — Auto-merging digest bumps undermines the whole supply chain

**Bites because:** When a digest drift issue lands, the easy fix is "bump the digest in docker/digests.txt and merge". If that bump is auto-merged via a bot (or a fast human review), the project just accepted a base-image change with zero scrutiny — exactly the threat model the supply chain is supposed to mitigate. Worst case: a compromised base image (e.g. xz utils incident, 2024) gets pulled in via auto-merged drift bump.

**Prevention strategy:**
- Digest-drift check opens an ISSUE, not a PR. Issue → human investigation → human-written PR → human-reviewed merge.
- If a renovate/dependabot config is added later for other deps, EXCLUDE Docker base images from auto-merge.
- Document the policy in SECURITY.md + CONTRIBUTING.md.

**Phase:** Phase 22 — sets the policy. SECURITY.md update in Phase 23 reinforces.

---

## Pitfall 12 — Releasing the cosign verify command before testing it from a clean machine

**Bites because:** "Works on my machine" is the classic. The SECURITY.md update with the cosign verify command needs to be tested by someone who hasn't been involved in the implementation, on a machine with no project-specific config (no `.sigstore/`, no project cosign cache).

**Prevention strategy:**
- Phase 23 + Phase 24 each have a HUMAN-UAT item: "operator rehearsal of the documented cosign verify command, on a fresh runner image / fresh Docker container". Don't ship without it.
- Include the exact cosign version operators should install (the project's published verify command should be runnable with cosign 2.5+ specifically — pinning the user-side version avoids breakage if cosign 3.0 changes the CLI).

**Phase:** Phase 23 (image verify) + Phase 24 (blob verify).

---

## Pitfall 13 — Cosign 3.0 CLI flag drift

**Bites because:** cosign 2.x → 3.0 may rename or repurpose flags. SECURITY.md cosign verify commands documented at v1.6 ship may be invalid by the time cosign 3.0 lands. Operators following the doc with cosign 3.0 see cryptic errors.

**Prevention strategy:**
- Pin a documented cosign version range in SECURITY.md: "tested with cosign 2.5.x; for cosign ≥ 3.0 see [link to cosign migration notes]".
- Reproducibility verifier + image-sign step in CI use a SHA-pinned `sigstore/cosign-installer` step (Pitfall 4) so the CI side is locked.
- When cosign 3.0 lands, a separate quick task updates SECURITY.md.

**Phase:** Phase 23, Phase 24.

---

## Cross-phase summary

| Phase | Pitfalls addressed |
|---|---|
| Phase 22 (digest drift) | 8, 9, 11 |
| Phase 23 (image attestations) | 1, 2, 3, 4, 5, 10, 12, 13 |
| Phase 24 (blob signing) | 1, 2, 3, 4, 12, 13 |
| Phase 25 (reproducibility) | 6, 7 |
