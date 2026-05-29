---
phase: 09
fixed_at: 2026-05-27T00:00:00Z
review_path: .planning/phases/09-ci-integration-test-reliability/09-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 5
skipped: 1
status: partial
---

# Phase 9: Code Review Fix Report

**Fixed at:** 2026-05-27
**Source review:** `.planning/phases/09-ci-integration-test-reliability/09-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope (Critical + Warning): 6
- Fixed: 5
- Skipped: 1 (deferred to Phase 10 per review guidance)

`cargo build --workspace --all-targets` and
`cargo clippy --workspace --all-targets -- -D warnings` both exit 0
after every commit on this branch.

## Fixed Issues

### CR-01: `BitcoindGuard::drop` calls a synchronously-blocking `node.stop()` from a tokio runtime thread

**Files modified:** `tests/integration/mod.rs`
**Commit:** `8d41e89`
**Applied fix:** Replaced the inline `let _ = n.stop();` in `Drop::drop`
with `tokio::runtime::Handle::try_current()` dispatch — when a runtime
handle is available (the normal `#[tokio::test]` case), the owned
`Node` is handed to `handle.spawn_blocking(...)` and the join handle is
detached. The runtime's shutdown waits for blocking-pool tasks to drain
before the process exits, so bitcoind is still reaped deterministically
but the runtime worker thread is freed immediately rather than parked
for the full shutdown duration. Falls back to inline blocking
`n.stop()` outside a runtime context.

### WR-01: `node.stop()` error is silently swallowed; no triage signal on shutdown failure

**Files modified:** `tests/integration/mod.rs`
**Commit:** `6fc3756`
**Applied fix:** In both branches of the `Handle::try_current()` match
introduced by CR-01, replaced `let _ = n.stop();` with
`if let Err(e) = n.stop() { eprintln!(...) }`. The eprintln names the
guard, the underlying error, and explicitly notes that `Node::Drop`'s
SIGKILL still runs as fallback — so a future shutdown-hang flake
leaves triage signal in the test log instead of being silently
suppressed. No `panic!` in drop.

### WR-02: PGP verify in CI does not assert the imported key has any signatures attached

**Files modified:** `.github/workflows/ci.yml`
**Commit:** `12cdb12`
**Applied fix:** Kept the existing `gpg --verify SHA256SUMS.asc SHA256SUMS`
as a belt-and-suspenders sanity check (its non-zero exit under
`set -euo pipefail` still fails on a bad signature) and added an
explicit `--status-fd=1` parse asserting the
`^\[GNUPG:\] GOODSIG ` line is present. This defends against the "key
not certified with a trusted signature" case where gpg can exit 0 with
a stderr warning. Comment block explains that PR review of
`GUIX_SIGS_SHA` / `KEY_FP` bumps is the residual trust root.

### WR-03: `bootstrap_regtest_bitcoind` panics on `Err` from `exe_path` even after `require_bitcoind!()` succeeded

**Files modified:** `tests/integration/mod.rs`,
`tests/integration/full_round.rs`,
`tests/integration/rate_limiting.rs`,
`tests/integration/round_bootstrap.rs`
**Commit:** `873f0b2`
**Applied fix:** Refactored `bootstrap_regtest_bitcoind` to accept
`exe: String` as a parameter; deleted its internal
`require_bitcoind_inner().unwrap_or_else(panic!)` block. Applied the
same change to the wrapper `fund_regtest(exe: String)` in
`full_round.rs`. Updated all 6 call sites (2 in `full_round.rs` direct,
4 via `fund_regtest`, 2 in `rate_limiting.rs`, 1 in `round_bootstrap.rs`)
to bind the macro's return value as `let exe = require_bitcoind!();`
and forward it. The macro is now the single panic gate per test
invocation; helper has no independent panic path.

### WR-04: `coordinator_info_endpoint_fields` uses non-routable bitcoind RPC URL

**Files modified:** `tests/integration/full_round.rs`
**Commit:** `ab14799`
**Applied fix:** Replaced `http://127.0.0.1:18443` (which on a CI
runner could collide with a co-tenant bitcoind) with the obviously
unbindable sentinel `http://invalid-rpc-not-running.localhost:1` and
zeroed the credentials. Bound the sentinel URL to a local variable
used by both the `CoordinatorConfig.network.bitcoin_rpc_url` field and
the `BitcoinRpc::new` constructor so they cannot drift. Added a
docstring on the test describing the deliberate non-routable target.

## Skipped Issues

### WR-05: `tokio::time::sleep(Duration::from_secs(2))` / `(4)` flake risk on shared-runner CI

**File:** `tests/integration/full_round.rs:369, 704, 1519, 1627`
**Reason:** `skipped: deferred to Phase 10` (per orchestrator
guidance). The four sites are pre-existing test-flake debt, not
introduced by Phase 9, and they currently sit behind `#[ignore]` so
the flake risk is masked until Phase 10 lifts the ignores in lockstep
with the RPC-schema unignore. A TODO marker was added at line 369
(commit `db65a6e`) naming all four locations and pointing to the
existing `wait_for_coordinator` poll-until-deadline pattern that
should replace bare sleep when Phase 10 picks this up. The TODO marker
is discoverability work, not the actual fix; the actual fix is
deferred.

**Original issue:** Multiple tests use bare `sleep` to await
asynchronous events (broadcast settling, signing-timeout firing, round
restart). On a noisy CI runner the 2s/4s windows can be exceeded under
contention, especially with bitcoind taking 1-3s to confirm a
generate_to_address call. The right shape is a poll-with-deadline
mirroring `wait_for_coordinator` at full_round.rs:116-135 and the
explicit deadline in round_bootstrap.rs:128-204.

## Out-of-scope Findings (Info-tier)

Per `fix_scope: critical_warning`, Info findings IN-01 through IN-05
were not addressed in this fix pass. They remain in REVIEW.md for a
future iteration or manual cleanup:

- IN-01: `Conf::default` fallbackfee comment is misleading
- IN-02: `Arc::try_unwrap` in `fund_regtest` is a structural smell
- IN-03: Replay-token regression test doesn't validate specific error code
- IN-04: `audit` job does not use `Swatinem/rust-cache`
- IN-05: Env-var idiom uses non-canonical spelling

---

_Fixed: 2026-05-27_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
