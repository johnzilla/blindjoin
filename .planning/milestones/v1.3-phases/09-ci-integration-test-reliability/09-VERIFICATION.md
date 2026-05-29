---
phase: 09-ci-integration-test-reliability
verified: 2026-05-26T00:00:00Z
status: passed
score: 20/20 must-haves verified (17 static + 3 human UAT closed 2026-05-27)
overrides_applied: 0
uat_closure_evidence: |
  UAT-1: PR #7 CI run https://github.com/johnzilla/blindjoin/actions/runs/26512029044
         cargo test 3m49s, 9 passed / 0 failed / 6 ignored. VALIDSIG matched.
         Required 2 fix commits (ea16787, 6d10d05) to harden multi-signer
         PGP verify — the exact kind of finding live-CI UAT was designed for.
  UAT-2: Local macOS arm64, brew bitcoind v31.0.0. Injected panic!("UAT-2")
         in round_bootstrap.rs:67 after _bitcoind_guard binding. cargo test
         exited in 8s wallclock (compile + 2.49s test run), log contained
         `panicked at` line, NO hang. Panic reverted post-test.
  UAT-3: Same host. Before: 0 bitcoind processes. Ran full suite (9 pass,
         0 fail, 6 ignored, 7.51s). After (5s settling): 0 bitcoind
         processes. BitcoindGuard::drop terminated all 4 spawned daemons.
human_verification:
  - test: "Fresh-PR CI log shows at least one bitcoind-dependent integration test executing with a PASS verdict (SC1)"
    expected: "Push a no-op PR to a branch off main. The `cargo test` job in `.github/workflows/ci.yml` runs `cargo test --workspace --all-targets` with `BLINDJOIN_REQUIRE_BITCOIND=1` and `BITCOIND_EXE=$HOME/.local/bin/bitcoind` exported. The log should contain a PASS line for at least one of: `rate_limiting::info_endpoint_returns_429_when_flooded`, `rate_limiting::request_timeout_returns_408`, or `round_bootstrap::run_bootstraps_round_into_input_reg` — and zero `bitcoind not found (...), skipping (local-dev mode; ...)` notices. Six `full_round.rs` carve-out tests should appear in the `ignored` column without executing."
    why_human: "SC1 is a runtime observation of the CI substrate created in Plan 09-01. The verifier can confirm the YAML is structurally correct (cache key, integrity gates, env exports, no `--include-ignored`), but cannot run actions/cache on a fresh runner. The first PR after Phase 9 lands is the canonical proof."
  - test: "Suite process exits within a bounded time when an individual test panics — no leaked bitcoind blocks the cargo pipe (SC2 runtime side)"
    expected: "Locally, force a panic in one bitcoind-using integration test (e.g., add a `panic!()` after `bootstrap_regtest_bitcoind` in `run_bootstraps_round_into_input_reg`) and run `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration 2>&1 | tee target/integration-test.log`. The suite process MUST exit within ~10 seconds of the panic, not hang waiting for bitcoind shutdown. `target/integration-test.log` should contain the `panicked at` line."
    why_human: "The static side of SC2 is verified (BitcoindGuard::drop, `view_stdout=false`, `-printtoconsole=0`, zero Box::leak). The runtime side — that an actual panic-unwind on a `#[tokio::test]` thread does terminate bitcoind before cargo finishes — needs a panic test against a live bitcoind. CR-01 (blocking-in-drop) means shutdown adds wall-clock; we want a contributor to confirm it stays bounded."
  - test: "No orphan bitcoind processes after the suite completes (SC3 runtime side)"
    expected: "On macOS or Linux, run `ps aux | grep bitcoind` BEFORE and AFTER `cargo test --test integration`. The two listings should match exactly — no bitcoind PID present after that wasn't present before, even when individual tests panic. If a PID lingers, `kill -9 <pid>` and investigate which test failed to drop its BitcoindGuard."
    why_human: "Static analysis confirms every callsite holds the guard in a let-binding for the test's full scope, and BitcoindGuard::drop calls `node.stop()` with `Node::Drop` SIGKILL fallback. But OS-level process-tree assertion after suite completion requires a live `ps`-equivalent check that's not reproducible from grep audits alone."
re_verification:
  previous_status: none
  previous_score: none
  gaps_closed: []
  gaps_remaining: []
  regressions: []
---

# Phase 9: CI integration-test reliability — Verification Report

**Phase Goal:** Integration tests that depend on bitcoind run end-to-end in CI on every PR — no silent graceful-skips, no leaked child processes blocking stdout, and a documented invocation pattern future contributors can copy-paste.

**Verified:** 2026-05-26
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP.md Success Criteria + plan must_haves merged)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Fresh PR's CI log shows a bitcoind-dependent integration test executing with PASS (SC1) | VERIFIED (static) — runtime needs CI | `.github/workflows/ci.yml` lines 31-108 wire: read `.bitcoind-version` (line 44) → cache (line 47-54) → PGP+SHA256-verified install (lines 57-99) → export `BITCOIND_EXE=$HOME/.local/bin/bitcoind` (line 105) → `cargo test --workspace --all-targets` (line 108). Workflow-level `BLINDJOIN_REQUIRE_BITCOIND: "1"` at line 17. The invocation does NOT pass `--include-ignored`, per amended D-10/D-16 — the 6 Phase-10 carve-outs in `full_round.rs` are skipped from execution. Three bitcoind-dependent, non-`#[ignore]` tests remain: `rate_limiting::info_endpoint_returns_429_when_flooded`, `rate_limiting::request_timeout_returns_408`, `round_bootstrap::run_bootstraps_round_into_input_reg`. Live CI run on a fresh PR is the runtime proof — see human verification item 1. |
| 2 | Pinned bitcoind binary is available on the runner via a cached install (SC1 substrate) | VERIFIED | `.bitcoind-version` exists at repo root with content `30.2`. `.github/workflows/ci.yml` lines 47-54 wire `actions/cache@<v4.3.0 SHA>` keyed on `${{ runner.os }}-bitcoind-${{ steps.bitcoind_version.outputs.version }}`. Lines 57-99 fetch `achow101.gpg` from `bitcoin-core/guix.sigs` commit `893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59`, assert fingerprint `152812300785C96444D3334D17565732E08E5E41`, gpg-verify SHA256SUMS, hash-check tarball, install to `~/.local/bin/bitcoind`. |
| 3 | Running `cargo test --test integration` writes test output to a log file (SC2 doc side) | VERIFIED | `CONTRIBUTING.md:19-23` documents the canonical command including `2>&1 \| tee target/integration-test.log`. Line 25 explains the pattern is the right shape for postmortem inspection and warns against `\| tail`. |
| 4 | Suite process exits within a bounded time even when a test panics — no leaked bitcoind blocks the cargo pipe (SC2 mechanism) | VERIFIED (static) — runtime needs panic test | `tests/integration/mod.rs:171-189` defines `impl Drop for BitcoindGuard` calling `node.stop()` with `Option::take()` to consume the owned Node before `Node::Drop` runs `process.kill()` (verified at corepc-node-0.12.0 lib.rs:575-582). Lines 260 (`conf.view_stdout = false`) + 264 (`conf.args.push("-printtoconsole=0")`) ensure bitcoind's child stdio is /dev/null — the cargo-stdout-pipe inheritance that caused the historical hang cannot occur. `grep -rn "Box::leak\|std::mem::forget" tests/integration/` returns ZERO matches across all 5 files. Runtime verification of panic-bounded shutdown is human verification item 2. |
| 5 | When the suite completes (pass, fail, panic), no orphan bitcoind remains in the process tree (SC3) | VERIFIED (static) — runtime needs ps check | Two-layer termination: (a) `BitcoindGuard::drop` (mod.rs:172-188) calls `n.stop()` which sends bitcoind's `stop` RPC and `wait()`s for the child; (b) when `n` falls out of scope, `corepc_node::Node::Drop` runs `process.kill()` unconditionally as belt-and-suspenders (per CONTEXT amendment + RESEARCH.md). Every bitcoind-using test holds the guard for its full scope: `full_round.rs:288, 630, 963, 1087, 1144, 1455` (all 6 ignored ones + `fund_regtest` callers); `rate_limiting.rs:152, 336`; `round_bootstrap.rs:65`. **Advisory:** CR-01 in 09-REVIEW.md flags that `node.stop()` blocks the tokio worker thread — kill still occurs, just stalls the executor for the shutdown duration. This is a quality issue documented but not a goal-blocker (runtime termination still happens). |
| 6 | `corepc-node` fixtures release their spawned bitcoind on test end (SC3 mechanism / TEST-04) | VERIFIED | RAII via `BitcoindGuard::drop`; zero `Box::leak` in `tests/integration/` (was 4 across `full_round.rs` (3) + `rate_limiting.rs` (1) per Plans 09-03/04 history). The destructor is no longer suppressed — corepc-node's own `Drop` runs and reaps the child. |
| 7 | CONTRIBUTING.md contains a "Running integration tests" section with copy-pasteable command (SC4) | VERIFIED | `CONTRIBUTING.md:12` heading `## Running integration tests`. Lines 18-23 contain the canonical bash command block without `--include-ignored`, with `BLINDJOIN_REQUIRE_BITCOIND=1`, `BITCOIND_EXE=$(brew --prefix)/bin/bitcoind`, and `2>&1 \| tee target/integration-test.log`. |
| 8 | CONTRIBUTING.md explains where output lands (SC4 log location) | VERIFIED | `CONTRIBUTING.md:25` documents `target/integration-test.log` location, gitignored-under-target rationale, and `cargo clean` auto-cleanup. |
| 9 | CONTRIBUTING.md explains how to interpret pass/fail/skip — 4-row reference card (SC4 / amended D-21) | VERIFIED | `CONTRIBUTING.md:56-61` has a 4-row table: `test result: ok ...` (Green), `test result: FAILED ...` (Red), `panicked at 'bitcoind required ...'` (Red), `bitcoind not found ... skipping (local-dev mode)` (Skipped). Each row maps the literal cargo string to verdict + next-step. |
| 10 | CONTRIBUTING.md provides a single-test invocation example (SC4 / D-19) | VERIFIED | `CONTRIBUTING.md:32-35` shows `cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture`, with the same env-var prefix as the canonical command. |
| 11 | Workflow-level `BLINDJOIN_REQUIRE_BITCOIND: "1"` env (Plan 09-01 must_have) | VERIFIED | `.github/workflows/ci.yml:9-17` sets `env:` at workflow scope; line 17 is `BLINDJOIN_REQUIRE_BITCOIND: "1"`. All three jobs (`test`, `clippy`, `coordinator-smoke`, `audit`) inherit it; the `test` job's `cargo test --workspace --all-targets` step at line 108 therefore runs with it set. |
| 12 | `require_bitcoind!()` macro + `BitcoindGuard` + `bootstrap_regtest_bitcoind()` fixtures defined in `tests/integration/mod.rs` (Plan 09-02 must_haves) | VERIFIED | `tests/integration/mod.rs:48-66` defines `require_bitcoind_inner`; lines 98-106 define the `require_bitcoind!()` `#[macro_export]`; lines 148-189 define `BitcoindGuard` + `impl Drop`; lines 231-296 define `bootstrap_regtest_bitcoind`. Module is the integration test crate root via `coordinator/Cargo.toml` `[[test]] name="integration" path="../tests/integration/mod.rs"`. |
| 13 | `full_round.rs` migrated to shared fixtures, zero `Box::leak`, 6 specific tests `#[ignore]`, 2 non-ignored (Plan 09-03 must_haves) | VERIFIED | `tests/integration/full_round.rs:24` `use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};`. Six `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]` markers at lines 164, 552, 950, 1074, 1131, 1440 — all with the agreed verbatim string. `grep "#\[tokio::test\]"` returns 8 across the file; 8 − 6 = 2 non-ignored: `adversarial_tampered_psbt_rejected` (line 1236) and `coordinator_info_endpoint_fields` (line 1312). Zero `Box::leak` in this file. |
| 14 | `rate_limiting.rs` + `round_bootstrap.rs` migrated to shared fixtures, file-private bootstrap deleted, zero `Box::leak` (Plan 09-04 must_haves) | VERIFIED | `tests/integration/rate_limiting.rs:78` and `tests/integration/round_bootstrap.rs:26` both `use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};`. No inline `bootstrap_regtest_bitcoind(exe: String)` helper survives (search returns only the shared mod.rs version). All `#[tokio::test]` functions invoke `bootstrap_regtest_bitcoind()` and bind the returned `BitcoindGuard` to a `let _bitcoind_guard = ...` for the test's full scope. Zero `Box::leak` in either file. |
| 15 | `CONTRIBUTING.md` documents `--include-ignored` ONLY for local Phase-10 work (Plan 09-05 amended D-10 / D-16) | VERIFIED | `CONTRIBUTING.md:39-50` adds a "Running ignored (Phase-10) tests locally" subsection; line 41 explicitly states the six carve-outs do not run in CI; line 50 acknowledges most will fail until Phase 10 lands the RPC-schema repairs. |
| 16 | bitcoind's child stdio routed to /dev/null — pipe-hang root cause closed (Plan 09-02 D-15 amended) | VERIFIED | `tests/integration/mod.rs:260` `conf.view_stdout = false` (corepc-node default; set explicitly per amended D-15 to defend against a future default flip); line 264 `conf.args.push("-printtoconsole=0")` as defense-in-depth so bitcoind itself suppresses console output even if `view_stdout` is bypassed. |
| 17 | bitcoind binary integrity gates (PGP key fingerprint pinned, SHA256SUMS signature-verified, tarball hash-checked) (Plan 09-01 D-04) | VERIFIED | `.github/workflows/ci.yml:75` pins `KEY_FP=152812300785C96444D3334D17565732E08E5E41` (achow101); line 76 pins `GUIX_SIGS_SHA=893b44f5fb1ed2abcdd79feb1c54723e3ccf5b59`; line 83-84 asserts imported fingerprint matches; line 89 `gpg --verify SHA256SUMS.asc SHA256SUMS`; line 93 `grep "  ${TARBALL}$" SHA256SUMS \| sha256sum -c`. Threat model per RESEARCH.md A6 + Pitfall 5. |

**Score:** 17/17 truths verified (15 statically + 2 with static-only side fully closed but live-runtime side routed to human verification for SC1 PR observation and SC2/SC3 panic-and-ps assertions).

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `.bitcoind-version` | Single-line `30.2` (per D-02) | VERIFIED | 2 lines including trailing newline; content `30.2`. |
| `.github/workflows/ci.yml` | `test:` job with bitcoind install, workflow-level `BLINDJOIN_REQUIRE_BITCOIND`, `BITCOIND_EXE` export, `cargo test --workspace --all-targets` (no `--include-ignored`) | VERIFIED | 157 lines; env block 9-17 (line 17 = `BLINDJOIN_REQUIRE_BITCOIND: "1"`); test job 26-108; pin/cache/install/export steps 42-105; cargo test invocation 107-108. |
| `tests/integration/mod.rs` | `require_bitcoind!()` macro + `BitcoindGuard` (with `impl Drop` calling `node.stop()`) + `RpcCreds` + `bootstrap_regtest_bitcoind()` | VERIFIED | 297 lines; macro at 98-106; `BitcoindGuard` struct at 148-150 with `impl Drop` at 171-189; `RpcCreds` at 115-120; `bootstrap_regtest_bitcoind` at 231-296 with `view_stdout=false` (260) + `-printtoconsole=0` (264). |
| `tests/integration/full_round.rs` | Migrated, 6 specific `#[ignore]` markers, zero `Box::leak`, 2 non-ignored tests preserved | VERIFIED | 1682 lines; 8 `#[tokio::test]` functions, 6 `#[ignore = "TODO(Phase-10): ..."]` markers (lines 164, 552, 950, 1074, 1131, 1440); non-ignored: `adversarial_tampered_psbt_rejected` (1236), `coordinator_info_endpoint_fields` (1312). Zero `Box::leak`. |
| `tests/integration/rate_limiting.rs` | Migrated, no inline bootstrap, zero `Box::leak` | VERIFIED | 547 lines; `use crate::{...}` at line 78; both `#[tokio::test]` fns (`info_endpoint_returns_429_when_flooded`, `request_timeout_returns_408`) call `bootstrap_regtest_bitcoind`. Zero `Box::leak`. No file-private bootstrap. |
| `tests/integration/round_bootstrap.rs` | Migrated, no inline bootstrap, zero `Box::leak` | VERIFIED | 206 lines; `use crate::{...}` at line 26; `run_bootstraps_round_into_input_reg` calls `bootstrap_regtest_bitcoind` at line 59. Zero `Box::leak`. |
| `CONTRIBUTING.md` | New file at repo root, sections: Local prerequisites + Running integration tests + Running a single test + Running ignored tests + Interpreting output (4-row card) | VERIFIED | 62 lines; intro 1-5; `## Local prerequisites` line 7; `## Running integration tests` line 12; `### Running a single test` line 27; `### Running ignored (Phase-10) tests locally` line 39; `## Interpreting output` line 52 with 4-row table 56-61. |
| `tests/integration/ban_list_persistence.rs` | UNCHANGED by Phase 9 (control sample, no bitcoind dependency) | VERIFIED | 179 lines; pure persistence-layer test; no `use crate::{...}` line, no `bootstrap_regtest_bitcoind` call, no bitcoind dependency. As expected. |
| `coordinator/Cargo.toml` corepc-node feature pin | UNCHANGED by Phase 9 — `corepc-node = { version = "0.12", features = ["30_2"] }` (REPAIR-02 is Phase 10) | VERIFIED | Line 65 confirmed: `corepc-node = { version = "0.12", features = ["30_2"] }`. Workspace audit for ALL Cargo.tomls is deferred to Phase 10 REPAIR-02. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `.github/workflows/ci.yml` | `.bitcoind-version` | `cat .bitcoind-version` inside the `Read pinned bitcoind version` step | WIRED | Line 44: `run: echo "version=$(cat .bitcoind-version) >> $GITHUB_OUTPUT"`; consumed at line 54 (`key: ${{ runner.os }}-bitcoind-${{ steps.bitcoind_version.outputs.version }}`) and line 72 (`VERSION="${{ steps.bitcoind_version.outputs.version }}"`). |
| `.github/workflows/ci.yml` | `$GITHUB_ENV` | `BITCOIND_EXE=$HOME/.local/bin/bitcoind` export step | WIRED | Lines 101-105: `Export BITCOIND_EXE` step runs unconditionally on both cache-hit and cache-miss; writes the env. |
| `.github/workflows/ci.yml workflow-level env` | `tests/integration/mod.rs require_bitcoind logic` | `BLINDJOIN_REQUIRE_BITCOIND=1` at runtime | WIRED | `tests/integration/mod.rs:52` `std::env::var("BLINDJOIN_REQUIRE_BITCOIND").as_deref() == Ok("1")` reads the same env var; CI sets it at line 17. |
| `tests/integration/mod.rs require_bitcoind!() macro` | `BLINDJOIN_REQUIRE_BITCOIND` env var | `std::env::var` call | WIRED | `tests/integration/mod.rs:48-66` `require_bitcoind_inner`; macro at 98-106 expands `match $crate::require_bitcoind_inner() { Some(p) => p, None => return }`. |
| `tests/integration/mod.rs BitcoindGuard::drop` | `corepc_node::Node::stop()` | `let _ = n.stop()` after `Option::take` | WIRED | Lines 172-188; `if let Some(mut n) = self.node.take() { let _ = n.stop(); }`. Followed by `n` going out of scope → corepc-node's `Node::Drop` runs `process.kill()`. |
| `tests/integration/mod.rs bootstrap_regtest_bitcoind()` | 3 consumer files | `let (guard, creds) = bootstrap_regtest_bitcoind().await` | WIRED | `full_round.rs:24`, `rate_limiting.rs:78`, `round_bootstrap.rs:26` all import; each callsite holds the returned `BitcoindGuard` in a local for the test's full duration. |
| `tests/integration/full_round.rs` | `tests/integration/mod.rs` | `use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};` | WIRED | Line 24. |
| `tests/integration/rate_limiting.rs` | `tests/integration/mod.rs` | `use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};` | WIRED | Line 78. |
| `tests/integration/round_bootstrap.rs` | `tests/integration/mod.rs` | `use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};` | WIRED | Line 26. |
| `CONTRIBUTING.md` | `.bitcoind-version` | Local prerequisites section references pinned `30.2` | WIRED | Line 10 quotes `30.2` and points readers to `.bitcoind-version` as the single source of truth. |
| `CONTRIBUTING.md` canonical command | `tests/integration/mod.rs require_bitcoind!()` | `BLINDJOIN_REQUIRE_BITCOIND=1` env in the command block | WIRED | Line 19 sets the env; the macro reads it at mod.rs:52. |
| `CONTRIBUTING.md` Interpreting output table | cargo test stdout format | Literal strings `test result: ok`, `test result: FAILED`, `panicked at 'bitcoind required ...'`, `bitcoind not found (...), skipping` | WIRED | Lines 58-61. The "bitcoind required but not found" string matches the panic at mod.rs:53-57; the "bitcoind not found (...), skipping (local-dev mode; ...)" string matches the eprintln at mod.rs:59-62. |

### Data-Flow Trace (Level 4)

For non-UI test/infrastructure phases, Level 4 reduces to verifying that data flows through the wiring. The key data pathway is `cargo test → bitcoind binary`:

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `corepc_node::exe_path()` call in `require_bitcoind_inner` | `path: String` | env `BITCOIND_EXE` (set by ci.yml line 105) OR PATH lookup | YES (real bitcoind binary path) | FLOWING — CI installs the binary at `$HOME/.local/bin/bitcoind` and exports the env; local-dev relies on `brew install bitcoin` per CONTRIBUTING.md line 10 |
| `bootstrap_regtest_bitcoind()` | `(BitcoindGuard, RpcCreds)` | `Node::with_conf(&exe, &conf)` (mod.rs:272) + `node.params.get_cookie_values()` (mod.rs:274) | YES (live regtest daemon with cookie auth, 101 blocks mined) | FLOWING — corepc-node's Node manages the OS subprocess; the cookie is read from the per-run tempdir-backed datadir |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Integration test crate compiles cleanly | `cargo check --tests --quiet` (run from repo root) | exit 0, no output | PASS |
| `.bitcoind-version` content is exactly `30.2` | `cat .bitcoind-version` | `30.2` (1 line + trailing newline) | PASS |
| Zero `Box::leak` or `std::mem::forget` in `tests/integration/` | `grep -rn "Box::leak\|std::mem::forget" tests/integration/` | 0 matches | PASS |
| `BLINDJOIN_REQUIRE_BITCOIND` env set at workflow scope | `grep -n BLINDJOIN_REQUIRE_BITCOIND .github/workflows/ci.yml` | match at line 17 | PASS |
| 6 `#[ignore]` markers in `full_round.rs` with verbatim string | `grep -c '#\[ignore' tests/integration/full_round.rs` | 6 | PASS |
| 8 `#[tokio::test]` functions in `full_round.rs` | `grep -c '#\[tokio::test\]' tests/integration/full_round.rs` | 8 | PASS |
| Live integration test run with real bitcoind producing a PASS line in CI | (cannot run locally without CI substrate) | — | SKIP — routed to human verification item 1 |

### Probe Execution

No probe scripts declared in plans or PHASE artifacts; no `scripts/*/tests/probe-*.sh` convention exists in this repo. **SKIPPED** — phase did not declare probes (Phase 9 is a CI substrate + RAII fixtures phase, not a migration phase).

### Test Setup Audit (Step 7d)

The verifier cites the integration tests as evidence for SC2/SC3 mechanism. Audit of test setup helpers:

| Helper | Constructs | Production analog | Risk | Disposition |
|---|---|---|---|---|
| `tests/integration/mod.rs:154` `BitcoindGuard::new(node)` | `BitcoindGuard { node: Some(node) }` wrapping a real `corepc_node::Node` | N/A — this IS the production helper; only used in tests, but it constructs and owns a real OS-level bitcoind child process, not a mock | LOW | Acceptable fixture |
| `tests/integration/mod.rs:231` `bootstrap_regtest_bitcoind()` | A live `corepc_node::Node` with `Node::with_conf` (line 272), then cookie extraction (line 274) and `generate_to_address(101)` (line 289) | Same `corepc_node` API a production CLI tool would use to bring up a regtest node | LOW | Acceptable fixture — exercises the same Node API as any production regtest-driven workflow |
| `tests/integration/full_round.rs:33` `build_input_reg_round_state()` | Calls production `coordinator::round::manager::start_round(&mut state)` directly — comment at line 30-32 explicitly cites "no hand-rolled RoundStateInner — eliminates the T-06-02 test-only backdoor" | `coordinator::run` invokes `start_round` at startup (per the comment + `coordinator::run` source) | LOW | Acceptable fixture — explicit comment confirms zero divergence from production path |

No HIGH-risk test setup helpers identified. No must-have FAILED on test-setup-audit grounds.

### Requirements Coverage

Cross-reference of all phase-claimed REQ-IDs against `.planning/REQUIREMENTS.md`:

| Requirement | Source Plan(s) | Description | Status | Evidence |
|---|---|---|---|---|
| TEST-01 | 09-01 | CI installs a pinned `bitcoind` binary (cached between runs) | SATISFIED | `.bitcoind-version` + `actions/cache` keyed on version + PGP-verified install (ci.yml 42-99) |
| TEST-02 | 09-01, 09-02 | Integration tests that require bitcoind actually execute in CI on every PR — no silent graceful-skips | SATISFIED (substrate VERIFIED; runtime per CI run) | `BLINDJOIN_REQUIRE_BITCOIND: "1"` workflow env (ci.yml:17) + `require_bitcoind!()` panic-on-miss when var is "1" (mod.rs:52-57); 3 non-ignored bitcoind-dependent tests will run on CI. Runtime confirmation per human verification item 1. |
| TEST-03 | 09-02, 09-03, 09-04 | `cargo test` produces output that streams to a log file (no buffering pipes) and the suite exits cleanly even if individual tests panic | SATISFIED (mechanism VERIFIED) | `view_stdout=false` (mod.rs:260) + `-printtoconsole=0` (mod.rs:264) prevent pipe inheritance; `BitcoindGuard::drop` (mod.rs:171-189) terminates bitcoind on panic-unwind; `2>&1 \| tee target/integration-test.log` documented (CONTRIBUTING.md:22). Runtime bounded-exit confirmation per human verification item 2. |
| TEST-04 | 09-02, 09-03, 09-04 | `corepc-node` test fixtures release their spawned `bitcoind` on test completion (no `Box::leak`) | SATISFIED | Zero `Box::leak` in `tests/integration/`; `BitcoindGuard::drop` calls `node.stop()` + `Node::Drop` SIGKILL fallback. Runtime no-orphan confirmation per human verification item 3. |
| TEST-05 | 09-05 | `CONTRIBUTING.md` documents the canonical integration-test invocation pattern | SATISFIED | `CONTRIBUTING.md:12-61` has Running integration tests + canonical command + log location + single-test pattern + 4-row reference card. |

**Status drift in REQUIREMENTS.md traceability table:** Lines 48-52 list TEST-01..05 as `active`, but lines 12-16 (the checklist authority) have all five marked `[x]`. Minor documentation drift but not a verifier blocker — the `[x]` is the source of truth and matches the implementation. Recommended cleanup: update lines 48-52 to `complete` for symmetry. (No phase task explicitly required this; flagging as info.)

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `.github/workflows/ci.yml` | 3-7, 13 | `TODO(fix-verification-gap)` comment referencing actions/checkout v6 upgrade | Info | Marker references explicit formal follow-up ("Latest tag is v6.0.2 as of 2026-05-25... deferred from this PR because v4→v6 is a major version bump"). Self-documented deferral, not unresolved debt. |
| `tests/integration/full_round.rs` | 164, 552, 950, 1074, 1131, 1440 | 6× `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]` | Info (intentional) | Per amended D-10/D-16: these are the carve-outs Phase 10 will repair via REPAIR-01. The phase plan EXPLICITLY required these markers. Phase 10 is the next scheduled phase in the same milestone (v1.3) and is declared in ROADMAP.md lines 41, 73-83. References formal follow-up work — acceptable per debt-marker gate. |
| `tests/integration/rate_limiting.rs` | 39, 70-74 | `TODO(Phase-8 Q3, A4)` connection-cap deferral | Info | Predates Phase 9 (was authored in Phase 8). Phase 9 did not modify this comment. Phase 8 verification accepted it as a deferred item. |
| `tests/integration/mod.rs` | 184 (CR-01) | `let _ = n.stop()` blocks tokio runtime worker during shutdown | Warning (advisory) | Code-review CR-01: `n.stop()` calls `process.wait()` synchronously on the runtime thread. Daemon still terminates (kill happens), but shutdown wall-clock blocks the executor. CR-01 proposed `tokio::task::spawn_blocking` offload. Phase did not adopt the fix; treated as quality issue not goal-blocker. **Recommend Phase 10 line item.** |
| `tests/integration/mod.rs` | 184 (WR-01) | Silent error swallow on `n.stop()` failure (`let _ = n.stop()`) | Warning (advisory) | Code-review WR-01: shutdown failures emit no triage signal. Recommended `eprintln!` on Err. Not adopted; flake-debugging cost. **Recommend Phase 10 line item.** |
| `tests/integration/full_round.rs` | 369, 704, 1519, 1627 | Bare `tokio::time::sleep(Duration::from_secs(2 or 4))` instead of poll-with-deadline | Warning (advisory) | Code-review WR-05: sleep-based async waits are flake risk on shared runners. Currently masked by `#[ignore]`, will resurface when Phase 10 unmutes. **Recommend Phase 10 line item.** |
| `.github/workflows/ci.yml` | 87-89 | `gpg --verify SHA256SUMS.asc SHA256SUMS` without `--status-fd` / GOODSIG assert | Warning (advisory) | Code-review WR-02: hardened form would assert `GOODSIG` on stdout. Current form catches BAD signatures (nonzero exit) but trust-warning is not surfaced. Mitigated by fingerprint pin + guix.sigs commit pin. **Recommend Phase 10 or follow-up PR.** |
| `tests/integration/full_round.rs` | 1311-1419 (WR-04) | `coordinator_info_endpoint_fields` uses fake bitcoind RPC URL `http://127.0.0.1:18443` | Warning (advisory) | Test uses `build_router` (not `coordinator::run`), so `startup_health_check` is skipped — the fake URL never gets contacted today. Future `/info` handler RPC use would silently flake. WR-04 recommended switching to `http://127.0.0.1:1/` or adding a doc assertion. Not goal-blocking. |
| `tests/integration/full_round.rs` | 829-834 (IN-02) | `Arc::try_unwrap` smell in `fund_regtest` | Info | Code-review IN-02: over-engineered Arc dance; could move bare guard directly into closure. Style, not correctness. |
| `tests/integration/mod.rs` | 265-270 (IN-01) | "fallbackfee already included" comment + conditional push (dead-code shape) | Info | Code-review IN-01: comment ambiguous about whether the conditional push is dead code. Style. |
| `.github/workflows/ci.yml` | 143-156 (IN-04) | `audit` job lacks `Swatinem/rust-cache` — recompiles cargo-audit every run | Info | CI quality, not correctness. |

**Debt-marker gate summary:** No `TBD`, `FIXME`, or `XXX` markers in Phase-9-modified files. All `TODO` markers reference formal follow-up work (Phase 10 ROADMAP slot for the 6 ignored tests; explicit "deferred from this PR" rationale for actions/checkout; Phase-8 carry-forward for rate_limiting.rs comment). No unresolved debt blockers.

### Human Verification Required

The following items require live testing that cannot be completed by grep / static analysis alone. They have been routed to the frontmatter `human_verification:` block and will materialize at `.planning/phases/09-ci-integration-test-reliability/09-HUMAN-UAT.md` via the standard execute-phase workflow.

#### 1. Fresh-PR CI log shows at least one bitcoind-dependent integration test executing with PASS

**Test:** Push a no-op PR to a branch off main. Wait for the `cargo test` job to complete.
**Expected:** The job log contains a PASS line for at least one of `rate_limiting::info_endpoint_returns_429_when_flooded`, `rate_limiting::request_timeout_returns_408`, or `round_bootstrap::run_bootstraps_round_into_input_reg`. Zero `bitcoind not found (...), skipping (local-dev mode; ...)` notices appear. The six `full_round.rs` carve-outs are listed as `ignored` without executing.
**Why human:** SC1 is a runtime observation of the CI substrate. The verifier confirmed the YAML structure (cache key, integrity gates, env exports, no `--include-ignored`) but cannot run `actions/cache` and `actions/checkout` on a real GitHub-hosted runner from local grep.

#### 2. Suite exits within bounded time when a test panics — no leaked bitcoind blocks the cargo pipe

**Test:** Locally, add `panic!("force exit");` after `let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;` in `tests/integration/round_bootstrap.rs:59`. Run `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration 2>&1 | tee target/integration-test.log`.
**Expected:** The suite process exits within ~10 seconds of the panic. `target/integration-test.log` contains a `panicked at` line. `ps aux | grep bitcoind` (run immediately after the suite exits) shows no orphan PIDs from the panicked test.
**Why human:** Static side of SC2 is verified (BitcoindGuard::drop, `view_stdout=false`, `-printtoconsole=0`, zero Box::leak). The panic-unwind side requires a live regtest bitcoind to confirm the runtime exit is bounded. CR-01 (blocking-in-drop) means shutdown adds wall-clock — a contributor needs to confirm it stays bounded.

#### 3. No orphan bitcoind processes remain in the process tree after suite completion

**Test:** On macOS or Linux:
```bash
ps aux | grep -i bitcoind | grep -v grep > /tmp/before.txt
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration 2>&1 | tee target/integration-test.log
ps aux | grep -i bitcoind | grep -v grep > /tmp/after.txt
diff /tmp/before.txt /tmp/after.txt
```
**Expected:** `diff` is empty (the two listings match exactly). No bitcoind PID survives that wasn't present before.
**Why human:** Static analysis confirms every callsite holds the guard in a let-binding and `BitcoindGuard::drop` calls `node.stop()` with `Node::Drop` SIGKILL fallback. OS-level process-tree assertion after the suite completes is a runtime observation not reproducible from grep audits.

### Gaps Summary

No gaps blocking goal achievement. All 17 must-haves verified at the source-level. Three human-verification items remain because Phase 9's goal is fundamentally about CI runtime behavior (SC1 = "a fresh PR's CI log shows ..." — a live observation by definition) and local panic / process-tree verification (SC2, SC3) that grep cannot exercise. The mechanism is fully wired; the live observation is the natural follow-up step performed during PR review and contributor acceptance.

**Advisories (not blockers, recommended for Phase 10 line items):**

- CR-01 (Critical in code review): `BitcoindGuard::drop` calls `node.stop()` synchronously on the tokio worker thread, blocking the executor for the bitcoind shutdown duration. Goal is achieved (daemon still terminates), but the pattern violates tokio guidance. Phase 10 should adopt the `spawn_blocking` offload proposed in 09-REVIEW.md.
- WR-01: silent error swallow on `n.stop()` failure makes shutdown flake debugging harder.
- WR-05: bare `sleep(Duration::from_secs(N))` in `full_round.rs` will flake on busy CI when Phase 10 unmutes the `#[ignore]` markers.
- WR-02: `gpg --verify` should assert `GOODSIG` via `--status-fd`.
- REQUIREMENTS.md traceability table (lines 48-52) still shows `active` while the checklist (lines 12-16) shows `[x]`. Recommended fix: bump table to `complete`.

---

_Verified: 2026-05-26T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
