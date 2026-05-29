# Contributing to blindjoin

blindjoin is MIT-licensed open infrastructure — anyone is welcome to run a coordinator, build a client, or contribute upstream. There are no fees, no terms of service, no company.

This document is narrow on purpose: it covers the local-dev prerequisites and the integration-test loop. For protocol-level changes, see the relevant phase plan under `.planning/phases/`. The project README at the repo root is the marketing surface and operator quickstart; this file is the local-dev manual.

## Local prerequisites

- **Rust toolchain.** Use the stable channel that matches CI. Verify with `rustup show active-toolchain`, or read `.github/workflows/ci.yml` for the toolchain action's pin.
- **Bitcoin Core v30.2.** The version is pinned in `.bitcoind-version` at the repo root (currently `30.2`); CI reads the same file so local and CI behavior stay aligned. Install via `brew install bitcoin` on macOS, or download the release tarball from <https://bitcoincore.org/bin/bitcoin-core-30.2/>. Bumping the pin is a single-line PR.

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

Milestone tags must follow strict 3-part semver: `vMAJOR.MINOR.PATCH` (e.g. `v1.3.0`, not `v1.3`).

**Why:** [.github/workflows/docker.yml](.github/workflows/docker.yml) uses `docker/metadata-action` with `type=semver,pattern={{version}}`, which only matches `vX.Y.Z`. A two-part tag like `v1.3` produces zero image tags, and `docker buildx build --push` then fails with `tag is needed when pushing to registry`. The Docker workflow has silently failed on every two-part tag (`v1.0`, `v1.1`, `v1.3`) and only ever succeeded on `v1.0.0`.

**Tagging a milestone close:**

```bash
git tag -a v1.X.0 -m "v1.X <Milestone name>

<one-line delivered summary>

Key accomplishments:
- ...

See .planning/MILESTONES.md for full details."

git push origin v1.X.0
```

The milestone *name* in planning docs (e.g. `v1.3 Test Infrastructure & Operational Hardening`) is independent of the git tag — docs may stay `v1.X` for readability while the tag is `v1.X.0`.
