# Architecture Patterns — v1.6 Supply-Chain Attestation Integration

**Domain:** Integrating cosign image attestations, cosign blob signing, reproducible-build verification, and automated digest drift detection into blindjoin v1.5's existing release pipeline.
**Researched:** 2026-06-01
**Confidence:** HIGH on integration points (codebase + workflows read in full); HIGH on cosign + slsa-github-generator wiring patterns (well-documented for GHA).

---

## Existing release architecture (read at v1.5 ship)

```
push tag v* ─┬─→ release.yml
             │     ├─ check job  (cargo test --all-targets under BLINDJOIN_REQUIRE_BITCOIND=1,
             │     │              via composite ./.github/actions/install-bitcoind)
             │     └─ build job  [if startsWith(github.ref, 'refs/tags/')]
             │         ├─ cargo build --release --bin coordinator --bin client --bin liquidity-bot
             │         ├─ tar + sha256sum
             │         └─ softprops/action-gh-release@<sha> uploads .tar.gz + .sha256
             │
             └─→ docker.yml
                   ├─ check job  (same as above)
                   └─ docker matrix job (coordinator | client | liquidity-bot)
                       [if startsWith(github.ref, 'refs/tags/')]
                       ├─ docker/login-action@<sha>
                       ├─ docker/setup-buildx-action@<sha>
                       ├─ docker/metadata-action@<sha>  (tags = type=semver,pattern={{version}})
                       └─ docker/build-push-action@<sha>  (context=., file=docker/Dockerfile, target=<image>)
                                                          → pushes ghcr.io/<owner>/blindjoin-<image>:X.Y.Z + :X.Y
```

Composite action `.github/actions/install-bitcoind/action.yml` is the project's existing supply-chain pattern: PGP-key-pinned to a SHA-locked guix.sigs commit; SHA256SUMS verified before tarball install. v1.6 extends this discipline to its own artifacts.

---

## New components

### `docker/digests.txt` (new file)

Canonical digest manifest. One `image@sha256:HEX` per line. Hand-maintained; bumped via PR with human review.

```
# Canonical digests for docker/Dockerfile base images.
# Bump only via a PR that has been reviewed by a human.
# See SECURITY.md § Supply-chain status > Base-image digests.
debian:bookworm-slim@sha256:<HEX>
lukemathwalker/cargo-chef:latest-rust-1@sha256:<HEX>
```

### `.github/workflows/digest-drift-check.yml` (new workflow)

Runs on `schedule: cron 'X 4 * * *'` (daily) + `workflow_dispatch`. Reads `docker/digests.txt`, resolves each tag fresh via `docker buildx imagetools inspect`, compares. On drift: opens an issue via `gh issue create`. Idempotent (does not re-open if a `[digest-drift]` issue already exists for the same image+digest pair).

### `.github/workflows/reproducible-verify.yml` (new workflow)

Runs on `schedule: cron 'X 5 1 * *'` (monthly) + `workflow_dispatch`. Re-runs the release build recipe on a fresh `ubuntu-latest` runner, downloads the latest release tarball via `gh release download`, compares `sha256sum`. On mismatch: opens an issue + posts in a release-notes thread.

### `docs/REPRODUCIBLE-BUILD.md` (new file)

Operator-facing recipe. Names: Rust toolchain version (matches `ci.yml` pin), `ubuntu-latest` runner image SHA at v1.6 ship (frozen), env vars (`SOURCE_DATE_EPOCH`, `RUSTFLAGS`), build invocation, expected `sha256sum`.

---

## Modified components

### `.github/workflows/release.yml`

**Permissions** (add to `permissions:` block):
```yaml
permissions:
  contents: write    # existing — upload to GH Releases
  id-token: write    # NEW — for cosign keyless OIDC
```

**Build job** (modifications):
1. After `dtolnay/rust-toolchain` setup, add a step to derive `SOURCE_DATE_EPOCH` from the tag's commit time:
   ```yaml
   - name: Set reproducible-build env
     run: |
       echo "SOURCE_DATE_EPOCH=$(git log -1 --format=%ct $GITHUB_SHA)" >> $GITHUB_ENV
       echo "RUSTFLAGS=--remap-path-prefix=$GITHUB_WORKSPACE=. --remap-path-prefix=$HOME=." >> $GITHUB_ENV
   ```
2. Change `cargo build --release ...` to `cargo build --release --locked ...` (explicit lock check).
3. After `tar czf ... sha256sum > .sha256`, install cosign and sign the blob:
   ```yaml
   - uses: sigstore/cosign-installer@<sha>  # v3
   - name: Sign release tarball
     run: |
       cosign sign-blob --yes --bundle blindjoin-linux-amd64.tar.gz.bundle blindjoin-linux-amd64.tar.gz
   ```
4. Extend `softprops/action-gh-release` `files:` list to include `.bundle`.

### `.github/workflows/docker.yml`

**Permissions** (add `id-token: write` to the `docker` job):
```yaml
docker:
  permissions:
    contents: read
    packages: write
    id-token: write    # NEW
```

**Docker matrix job** (additions after `docker/build-push-action`):
```yaml
- uses: sigstore/cosign-installer@<sha>  # v3
- name: Sign image
  run: |
    cosign sign --yes \
      ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}@${{ steps.build-push.outputs.digest }}
- name: Attest SBOM
  uses: actions/attest-build-provenance@<sha>  # uses slsa-github-generator under the hood
  with:
    subject-name: ghcr.io/${{ github.repository_owner }}/blindjoin-${{ matrix.image }}
    subject-digest: ${{ steps.build-push.outputs.digest }}
    push-to-registry: true
```

The `actions/attest-build-provenance` action (maintained by GitHub) wraps the SLSA v1.0 provenance flow; it's the recommended path for SLSA Level 3 on GHA-hosted runners as of mid-2024.

### `docker/Dockerfile`

No structural change. The existing `ARG CARGO_CHEF_REF` + `ARG DEBIAN_REF` scaffold already supports digest pinning. The change is: release.yml + docker.yml read `docker/digests.txt` and pass `--build-arg DEBIAN_REF=...` automatically. Add a release.yml step:

```yaml
- name: Load canonical digests
  id: digests
  run: |
    DEBIAN_DIGEST=$(grep '^debian:' docker/digests.txt | cut -d= -f2)
    CARGO_CHEF_DIGEST=$(grep '^lukemathwalker/' docker/digests.txt | cut -d= -f2)
    echo "debian=$DEBIAN_DIGEST" >> $GITHUB_OUTPUT
    echo "cargo_chef=$CARGO_CHEF_DIGEST" >> $GITHUB_OUTPUT
```

Then pass them as build args in `docker/build-push-action`.

### `SECURITY.md`

After v1.6 ships, the `## Supply-chain status > Known gaps at v1.5` section is rewritten to reference the v1.6 deliverables in the past tense; the `### v1.6 supply-chain plan` becomes `### Verification commands` with cosign verify-blob + cosign verify recipes.

---

## Data flow — OIDC token to cosign signature

```
GitHub Actions runner ─┐
                       ├─→ requests OIDC token from GH (id-token: write)
                       │   token claims: sub = workflow ref + repo + ref
                       │
GitHub OIDC issuer ────┤
(token.actions.        │
 githubusercontent.com)│
                       │
cosign on runner ──────┤
                       ├─→ exchanges OIDC token with Fulcio for short-lived cert
                       │   cert SAN: workflow_ref claim
Fulcio (sigstore) ─────┤
                       │
cosign on runner ──────┤
                       ├─→ signs artifact bytes; appends to Rekor transparency log
Rekor (sigstore) ──────┤
                       │
artifact + .sig + .crt ┴─→ pushed to ghcr.io (image case) or GH Releases (tarball case)
```

Verifier-side: `cosign verify` re-derives the expected SAN from `--certificate-identity*` flags + `--certificate-oidc-issuer`, fetches Rekor entry, validates inclusion proof, validates cert chain, validates signature against artifact digest. All offline once the Sigstore TUF root is local.

---

## Suggested build order (passed to gsd-roadmapper)

1. **Phase 22 — Digest drift detection** (Category 4)
   - Lowest risk (no operator-facing artifact change). Builds the operational muscle for digest discipline before atop-of-stack signing lands.
   - Output: `docker/digests.txt`, `.github/workflows/digest-drift-check.yml`, release.yml + docker.yml reading the manifest.

2. **Phase 23 — cosign image attestations + SLSA provenance** (Category 1)
   - Adds `id-token: write` + cosign-installer + `actions/attest-build-provenance` to docker.yml.
   - This is the load-bearing piece. Once it ships, every future image build is signed + attested.

3. **Phase 24 — cosign blob signing on release tarballs** (Category 2)
   - Mirrors Phase 23 patterns into release.yml; reuses cosign-installer setup.
   - SECURITY.md draft updates land here too (verification command).

4. **Phase 25 — Reproducible-build recipe + scheduled verifier** (Category 3)
   - `docs/REPRODUCIBLE-BUILD.md` + reproducibility-affecting release.yml env vars + scheduled `reproducible-verify.yml`.
   - Depends on Phase 24 because the verifier checks the sig as well as byte-equality.

Each phase is independently shippable — operators see incremental supply-chain assurance after each. Phase 22 + Phase 23 are the "minimum lovable supply chain" milestone; Phase 24 closes the release-tarball gap; Phase 25 is the highest-assurance polish.
