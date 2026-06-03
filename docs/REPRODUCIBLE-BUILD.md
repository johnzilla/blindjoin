# Reproducible builds

blindjoin's `blindjoin-linux-amd64.tar.gz` Release archive is reproducible byte-for-byte from source on the pinned `ubuntu-24.04` runner image. Anyone — operator, auditor, or independent reviewer — can rebuild from a fresh checkout and verify the bytes match what the maintainer published. The recipe below is the single canonical procedure; the same recipe is exercised continuously by `.github/workflows/reproducible-verify.yml` (see [§Continuous verification](#continuous-verification)).

## Why this exists

Reproducibility proves that the published tarball is the natural product of the source at the tagged commit — not a tampered binary, not an arbitrary build, not the maintainer's local-machine artifact uploaded by hand. For operators who deploy blindjoin in environments where binary provenance matters (audit-track supply chains, regulated environments, air-gapped review), reproducibility is the verifiable bridge between "the maintainer says this is the v1.6.0 binary" and "I can prove it on my own machine, from source, without trusting the maintainer's build host." The continuous verifier at [`.github/workflows/reproducible-verify.yml`](../.github/workflows/reproducible-verify.yml) (created in Phase 25 Plan 25-04) runs the same recipe monthly on a fresh `ubuntu-24.04` runner and opens a `[reproducibility-regression]` issue on mismatch — so a drift between source and published bytes surfaces within one monthly cycle rather than waiting for an external reviewer to notice.

## Recipe

Run on a fresh `ubuntu-24.04` shell (Docker, VM, or GitHub Actions runner).

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

The output `sha256sum` MUST match the value in [§Expected sha256sum](#expected-sha256sum) for the tag you checked out. If it does not, see [§Reporting a reproducibility regression](#reporting-a-reproducibility-regression).

## Toolchain pins

| Component | Version | Pin lives in |
| --- | --- | --- |
| rustc | 1.95.0 | [`rust-toolchain.toml`](../rust-toolchain.toml) — workspace root |
| cargo | 1.95.0 (bundled with rustc) | same as above |
| ubuntu-24.04 runner image | `<TBD-v1.6.0-cut>` (ImageVersion captured at v1.6.0-rc.0 cut per [docs/RELEASING.md](docs/RELEASING.md) §Reproducibility verification rehearsal) | `.github/workflows/release.yml` + `.github/workflows/reproducible-verify.yml` |
| `dtolnay/rust-toolchain` action | `@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable` | `.github/workflows/{release,ci}.yml` (6 `with:` blocks; pinned at SHA + value via `rust-toolchain-pin-check` CI gate) |
| `sigstore/cosign-installer` action | `@7e8b541eb2e61bf99390e1afd4be13a184e9ebc5 # v3.10.1` (cosign-release `v2.6.3`) | `.github/workflows/{release,reproducible-verify}.yml` |
| `actions/checkout` action | `@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` | every `.github/workflows/*.yml` |

The rustc 1.95.0 pin is the single source of truth; the `with: toolchain:` inputs across `release.yml` + `ci.yml` are grep-asserted to match by the `rust-toolchain-pin-check` CI job. The runner-image ImageVersion placeholder will be captured at the v1.6.0-rc.0 cut and substituted atomically alongside the [§Expected sha256sum](#expected-sha256sum) placeholder (see [docs/RELEASING.md](docs/RELEASING.md) §Reproducibility verification rehearsal).

## Environment

Three build-time environment variables drive byte-equality. All three are set in the `release.yml` `build` job and reproduced verbatim in the [Recipe](#recipe) above so external rebuilders get the same values on their own machines.

- **`SOURCE_DATE_EPOCH`** — derived from `git log -1 --format=%ct $GITHUB_SHA` per REPRO-02; runtime-set in [`.github/workflows/release.yml`](../.github/workflows/release.yml) `Compute SOURCE_DATE_EPOCH from tagged commit time` step. External rebuilders use `git log -1 --format=%ct HEAD` after checking out the tag (per the Recipe). The value is the committer-time of the tagged commit, expressed as a Unix epoch — reproducible from source because git records it immutably with the commit object.
- **`RUSTFLAGS`** — exact literal: `--remap-path-prefix=<workspace>=/build --remap-path-prefix=<cargo-home>=/cargo`. Two `--remap-path-prefix` flags strip embedded build-host paths from debug info and panic messages. Without these, rebuilds on different runners produce diverging bytes even on identical toolchain because `rustc` embeds the absolute paths of the workspace and `$CARGO_HOME` into debug-info sections and into panic-message format strings.
- **`CARGO_INCREMENTAL`** — exact literal: `0`. Disables incremental compilation, which otherwise embeds host-specific intermediate paths in metadata. Incremental compilation is a dev-mode optimization that trades reproducibility for rebuild speed; release builds disable it explicitly.

> **Note: Rust reproducibility long tail.** The three env vars above handle the known mechanical sources of nondeterminism, but Rust binary determinism can still surface project-specific surprises — `proc-macro` crates that call `Instant::now()` at compile time, `build.rs` scripts that consult `chrono::Local::now()` or `env::current_dir()`, LLVM ordering nondeterminism on certain targets, or random hash seeds for `dyn Trait` vtable layout. The continuous verifier (see [§Continuous verification](#continuous-verification)) is the iteration mechanism for surfacing and fixing these on each monthly cycle; the maintainer's local `diffoscope` run on the first divergence is the manual triage path.

## Expected sha256sum

The expected hash is also available in machine-readable form at [`docs/REPRODUCIBLE-BUILD.expected-sha256.txt`](REPRODUCIBLE-BUILD.expected-sha256.txt) — this is what the scheduled verifier (see [§Continuous verification](#continuous-verification)) reads.

| Release tag | Expected `sha256sum blindjoin-linux-amd64.tar.gz` |
| --- | --- |
| v1.6.0 | `<TBD-v1.6.0-cut>` |

The `<TBD-v1.6.0-cut>` placeholder is replaced at the v1.6.0-rc.0 cut per the rehearsal procedure in [docs/RELEASING.md](docs/RELEASING.md) §Reproducibility verification rehearsal. The placeholder string contains `<` and `>` characters that never appear in a real sha256 hex value; the verifier treats a placeholder-vs-real-hex mismatch as a HIGH-severity divergence per [§Reporting a reproducibility regression](#reporting-a-reproducibility-regression).

## Continuous verification

The scheduled verifier at [`.github/workflows/reproducible-verify.yml`](../.github/workflows/reproducible-verify.yml) (created in Phase 25 Plan 25-04) runs monthly on the 1st of each month at 07:00 UTC on a fresh `ubuntu-24.04` runner. Each run executes the following steps:

1. **Capture the runner ImageVersion** — read the `${ImageVersion}` env var (format `20260518.149.1`) that GitHub-hosted runners expose, and compare against the pinned value in [`docs/REPRODUCIBLE-BUILD.expected-sha256.txt`](REPRODUCIBLE-BUILD.expected-sha256.txt).
2. **Resolve the latest release tag** — `LATEST_TAG=$(gh release view --json tagName --jq .tagName)`.
3. **Download the release tarball + cosign bundle** — `gh release download "$LATEST_TAG" --pattern 'blindjoin-linux-amd64.tar.gz*'`.
4. **Re-verify the cosign signature** on the downloaded tarball (Phase 24 SIGN-01 inheritance — the verifier proves cosign + byte-equality together; both must hold for a green run).
5. **Checkout the source at `$LATEST_TAG`** and rebuild via the [Recipe](#recipe) above.
6. **Compute the rebuilt sha256** — `ACTUAL=$(sha256sum blindjoin-linux-amd64.tar.gz | cut -d' ' -f1)`.
7. **Look up the expected sha256 + pinned ImageVersion** from [`docs/REPRODUCIBLE-BUILD.expected-sha256.txt`](REPRODUCIBLE-BUILD.expected-sha256.txt) via `awk -F: '$1 == "'"$LATEST_TAG"'" {print $2 " " $3}'` — a single colon-delimited lookup returning both values together. Classify the result: green if both match; runner-image drift (low-severity) if the ImageVersion differs; sha256 mismatch (HIGH-severity) if the ImageVersion matches but the hash diverges.
8. **Open a `[reproducibility-regression]` issue on mismatch** per [§Reporting a reproducibility regression](#reporting-a-reproducibility-regression) — title-deduplicated against any open issue so chronic regressions don't spam the issue tracker.

The verifier can also be triggered on-demand via `workflow_dispatch` from the Actions tab — used at the v1.6.0-rc.0 cut to capture the initial expected hash and at any subsequent moment when a maintainer wants to re-prove reproducibility outside the monthly cadence.

Registry entry: `<added after blindjoin's submission lands; see [docs/RELEASING.md](docs/RELEASING.md) §Reproducible-builds.org registry submission>`. After REPRO-01 + the verifier have been green for ≥1 monthly cycle, the maintainer submits blindjoin to the [reproducible-builds.org project registry](https://reproducible-builds.org/projects/) per Phase 25 Plan 25-05's documented procedure; this section gets updated with the public registry-entry URL at that time.

## Reporting a reproducibility regression

The verifier opens `[reproducibility-regression]`-labeled issues automatically on mismatch.

Operators who rebuild via the [Recipe](#recipe) above and find a divergence can file the same issue manually using the same title scheme. Use the exact title format below so the verifier's dedup-by-title-match logic recognizes a pre-existing operator-filed report and avoids opening a duplicate on its next scheduled run.

There are two title formats with distinct severities:

- **Runner-image drift (low-severity):** `[reproducibility-regression] runner image drift: ImageVersion <OLD> → <NEW>` — GitHub rotated the `ubuntu-24.04` runner image SHA. Verify reproducibility on the new image; update [`docs/REPRODUCIBLE-BUILD.expected-sha256.txt`](REPRODUCIBLE-BUILD.expected-sha256.txt) (and the table in [§Toolchain pins](#toolchain-pins)) with the new ImageVersion if the rebuild is green; investigate if not. This is not a supply-chain signal until investigated — it reflects an environmental rotation outside the project's control.
- **sha256 mismatch (HIGH-severity):** `[reproducibility-regression] sha256 mismatch on ImageVersion <V>` — the rebuilt tarball diverges from the published release on the SAME `ubuntu-24.04` ImageVersion `<V>`. This is a real supply-chain signal — the published release does not reproduce from source. Suspect tampering, compromised CI, or undocumented build-env drift. The maintainer's triage path: run `diffoscope` on the two tarballs locally; the most likely culprits (in descending probability) are `build.rs` scripts in dependencies, embedded random-seeded data, then proc-macro time issues.

The distinction matters because the GitHub runner-image rotation is environmentally normal and not necessarily a regression. Encoding the ImageVersion in the title surfaces this at-a-glance in the issue list and lets dedup-by-title-match remain exact.

---

*Maintained at: docs/REPRODUCIBLE-BUILD.md. Last updated: 2026-06-02.*
