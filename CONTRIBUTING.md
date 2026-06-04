# Contributing to blindjoin

blindjoin is MIT-licensed open infrastructure — anyone is welcome to run a coordinator, build a client, or contribute upstream. There are no fees, no terms of service, no company.

This document is narrow on purpose: it covers the local-dev prerequisites and the integration-test loop. For protocol-level changes, see the relevant phase plan under `.planning/phases/`. The project README at the repo root is the marketing surface and operator quickstart; this file is the local-dev manual.

## Local prerequisites

- **Rust toolchain.** Use the stable channel that matches CI. Verify with `rustup show active-toolchain`, or read `.github/workflows/ci.yml` for the toolchain action's pin.
- **Bitcoin Core v30.2.** The version is pinned in `.bitcoind-version` at the repo root (currently `30.2`); CI reads the same file so local and CI behavior stay aligned. Install via `brew install bitcoin` on macOS, or download the release tarball from <https://bitcoincore.org/bin/bitcoin-core-30.2/>. Bumping the pin is a single-line PR.

## Pre-push hook (optional but recommended)

A tracked pre-push hook lives at `.githooks/pre-push` and mirrors the CI checks in `.github/workflows/ci.yml` (`cargo check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --lib`, `cargo audit`). It catches the common case where local clippy passes without `--all-targets` but CI fails on lints in `tests/` files.

Enable it once per clone:

```bash
git config core.hooksPath .githooks
```

The hook runs `cargo test --workspace --lib` (fast unit tests only) by default. To mirror CI exactly and run integration tests too — slow, needs `bitcoind` on `$PATH` or `$BITCOIND_EXE` — set `BLINDJOIN_HOOK_FULL_TEST=1`:

```bash
BLINDJOIN_HOOK_FULL_TEST=1 git push
```

Skip the hook for a single push (e.g. a WIP branch): `git push --no-verify`.

## Running integration tests

The integration tests under `tests/integration/` exercise the production startup path of the coordinator (in-process `coordinator::run`) against a real regtest `bitcoind` and a real HTTP client. They are slow — 60+ seconds per test, sometimes longer — and they require a running `bitcoind` binary discoverable via `BITCOIND_EXE`.

The canonical local invocation:

```bash
BLINDJOIN_REQUIRE_BITCOIND=1 \
  BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration 2>&1 \
  | tee target/integration-test.log
```

The `2>&1 | tee target/integration-test.log` pattern routes test output to both your terminal and a log file you can grep after the suite finishes. Do **not** pipe `cargo test` through `| tail` or similar truncating filters — Phase 9 fixed the root cause where `bitcoind` inherited cargo's stdout pipe and hung the suite, but `| tee` to a file remains the right pattern for postmortem inspection. The log lives at `target/integration-test.log`, which is under cargo's already-gitignored `target/` directory and is auto-cleaned by `cargo clean`.

### Running a single test

The full suite is slow; iterating on one test is much faster while you're debugging a specific behavior. Use the test's module-qualified path:

```bash
BLINDJOIN_REQUIRE_BITCOIND=1 \
  BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture
```

The `-- --nocapture` flag lets you see `eprintln!` output and `tracing` events inline. Drop it once the test is passing for cleaner output.

## Interpreting output

Cargo's test output uses a small set of literal strings. The table below maps the strings you'll see in your terminal (or in `target/integration-test.log`) to a verdict and the next step.

| Output snippet | Verdict | Next step |
|---|---|---|
| `test result: ok. N passed; 0 failed; 0 ignored` | Green | All tests passed. |
| `test result: ok. N passed; 0 failed; M ignored` (M > 0) | Investigate | Some tests were skipped via `#[ignore]`. Grep `tests/` for `#[ignore` to see which and why. The repo currently ships with no ignored tests; an `M ignored` count means a future ignore-marker was added and may need attention. |
| `test result: FAILED. N failed` | Red | Open `target/integration-test.log` and grep for `panicked at` or `FAILED` to find the first failure. Re-run the failing test in isolation with the single-test command above and `-- --nocapture`. |
| `panicked at 'bitcoind required but not found'` | Red | `BLINDJOIN_REQUIRE_BITCOIND=1` is set but `BITCOIND_EXE` points to a missing or non-executable binary. Run `ls -l $BITCOIND_EXE` and `$BITCOIND_EXE --version` to verify the path resolves and runs. |
| `bitcoind not found (...), skipping (local-dev mode; ...)` | Skipped | `BLINDJOIN_REQUIRE_BITCOIND` is unset and `corepc-node` could not locate `bitcoind` on its own. Tests skipped gracefully — not an error in local-dev mode. To match CI's behavior (panic instead of skip), set `BLINDJOIN_REQUIRE_BITCOIND=1`. |

## Tagging releases

Milestone tags must follow strict 3-part semver: `vMAJOR.MINOR.PATCH` (e.g. `v1.3.0`, not `v1.3`). The `release-gate` job in [release.yml](.github/workflows/release.yml) and [docker.yml](.github/workflows/docker.yml) hard-fails any tag that doesn't match — the build never runs. (History: `v1.0`, `v1.1`, `v1.3` all silently produced zero image tags via `docker/metadata-action` before the gate existed.)

**Pre-tag checklist:**

1. Move all bullets from `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) into a new `## [X.Y.Z] — YYYY-MM-DD` section. `release-gate` hard-fails if the matching section is missing.
2. (Optional) Run the base-image digest check (see [§Bumping base-image digests](#bumping-base-image-digests)). Bump in a separate one-line PR before the release tag if upstream has drifted and the changelog warrants the rotation.
3. **Crate versions in `Cargo.toml` stay at `0.1.0`** — see [SECURITY.md § Release versioning policy](SECURITY.md#release-versioning-policy). The git tag is the canonical release identifier; the four workspace crates are unpublished.

**Cut the tag:**

```bash
git tag -s v1.X.0 -m "v1.X <Milestone name>

<one-line delivered summary>

Key accomplishments:
- ...

See .planning/MILESTONES.md for full details."

git push origin v1.X.0
```

The milestone *name* in planning docs (e.g. `v1.3 Test Infrastructure & Operational Hardening`) is independent of the git tag — docs may stay `v1.X` for readability while the tag is `v1.X.0`.

After `release.yml` and `docker.yml` complete green in the Actions tab, the release is published. CI's `actions/attest-build-provenance` step IS the supply-chain claim — no local pre-flight verify needed. If something goes wrong after tag push, `gh release delete vX.Y.Z` and re-cut after the fix.

## Bumping base-image digests

`docker/Dockerfile` pins both base images (`debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`) by sha256 digest directly in the `FROM` lines. To bump one:

```bash
docker buildx imagetools inspect debian:bookworm-slim --format '{{.Manifest.Digest}}'
# → sha256:<HEX>
```

Replace the digest portion of the matching `FROM` line in `docker/Dockerfile` with the new value. One image per PR; don't combine with unrelated changes. A compromised upstream base image (xz utils, 2024; event-stream, 2018) would otherwise leak into releases — review the upstream changelog before bumping.
