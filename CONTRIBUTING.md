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

### Running ignored (Phase-10) tests locally

Six tests in `tests/integration/full_round.rs` carry `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction — see TODO.md"]` markers (per Phase 9 decision D-10). These tests do **not** run in CI and do **not** run with the default canonical command above. Phase 10 will repair them. If you are iterating on those tests locally as part of Phase-10 work, add `--include-ignored`:

```bash
BLINDJOIN_REQUIRE_BITCOIND=1 \
  BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration -- --include-ignored 2>&1 \
  | tee target/integration-test.log
```

Most of the six carve-out tests will fail until Phase 10 lands the RPC-schema repairs — that is the expected state, not a regression.

## Interpreting output

Cargo's test output uses a small set of literal strings. The table below maps the strings you'll see in your terminal (or in `target/integration-test.log`) to a verdict and the next step.

| Output snippet | Verdict | Next step |
|---|---|---|
| `test result: ok. N passed; 0 failed; M ignored` | Green | The `M ignored` count is expected — those are Phase-10 carve-outs with `#[ignore = "TODO(Phase-10): ..."]` markers in `tests/integration/full_round.rs`. Nothing to do. |
| `test result: FAILED. N failed` | Red | Open `target/integration-test.log` and grep for `panicked at` or `FAILED` to find the first failure. Re-run the failing test in isolation with the single-test command above and `-- --nocapture`. |
| `panicked at 'bitcoind required but not found'` | Red | `BLINDJOIN_REQUIRE_BITCOIND=1` is set but `BITCOIND_EXE` points to a missing or non-executable binary. Run `ls -l $BITCOIND_EXE` and `$BITCOIND_EXE --version` to verify the path resolves and runs. |
| `bitcoind not found (...), skipping (local-dev mode; ...)` | Skipped | `BLINDJOIN_REQUIRE_BITCOIND` is unset and `corepc-node` could not locate `bitcoind` on its own. Tests skipped gracefully — not an error in local-dev mode. To match CI's behavior (panic instead of skip), set `BLINDJOIN_REQUIRE_BITCOIND=1`. |
