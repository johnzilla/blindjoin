---
phase: 09-ci-integration-test-reliability
plan: 02
subsystem: testing
tags: [corepc-node, raii, drop-guard, macro_rules, regtest, integration-tests, tokio]

# Dependency graph
requires:
  - phase: 09-01-ci-bitcoind-install
    provides: BLINDJOIN_REQUIRE_BITCOIND=1 workflow-level env var (consumed by require_bitcoind_inner)
provides:
  - require_bitcoind_inner() and require_bitcoind!() macro for env-var-gated bitcoind discovery
  - BitcoindGuard RAII type replacing the leak-the-Node pattern (Drop calls n.stop() then relies on Node::Drop's process.kill())
  - RpcCreds struct for canonical RPC credentials handoff
  - bootstrap_regtest_bitcoind() async helper consolidating Node::with_conf + cookie extract + 101-block mining + view_stdout=false + -printtoconsole=0
affects: [09-03 (full_round callsite migration), 09-04 (rate_limiting + round_bootstrap callsite migration), 10 (REPAIR-01 RPC schema repair)]

# Tech tracking
tech-stack:
  added: []  # zero new dependencies; uses existing corepc-node 0.12 features=["30_2"]
  patterns:
    - RAII guard with explicit impl Drop (extends ConnectionGuard pattern from coordinator/src/network/tor.rs)
    - macro_rules! for return-from-caller skip semantic (test-runner-aware skip pattern)
    - Option<T>::take() drain-in-Drop idiom for owned resources requiring graceful shutdown

key-files:
  created: []
  modified:
    - tests/integration/mod.rs

key-decisions:
  - "Macro form (require_bitcoind!()) is load-bearing — a plain fn cannot return from the calling test scope, and std::process::exit would abort the whole binary"
  - "bootstrap_regtest_bitcoind() calls require_bitcoind_inner() directly (not the macro) because the macro's None=>return expansion would type-error inside a function returning (BitcoindGuard, RpcCreds); tests invoke the macro themselves before calling bootstrap"
  - "view_stdout=false is set explicitly even though it is the corepc-node 0.12 default — protects against a future default flip silently re-introducing the pipe-hang"
  - "-printtoconsole=0 is passed as defense-in-depth alongside view_stdout=false (D-15 amended)"
  - "BitcoindGuard's Drop calls n.stop() (RPC) best-effort then lets Node::Drop run process.kill() as belt-and-suspenders — no panic in drop"

patterns-established:
  - "Test-fixture macro pattern: #[macro_export] macro_rules! in tests/integration/mod.rs, reachable as $crate::macro_name!() from submodules via the [[test]] declaration in coordinator/Cargo.toml"
  - "BitcoindGuard RAII pattern extending ConnectionGuard with explicit impl Drop for resources requiring graceful shutdown RPC before SIGKILL fallback"
  - "BLINDJOIN_REQUIRE_BITCOIND env-var gate as the canonical env-var-gated skip-vs-fail policy for test infrastructure"

requirements-completed: [TEST-02, TEST-03, TEST-04]

# Metrics
duration: 6min
completed: 2026-05-27
---

# Phase 9 Plan 2: Shared bitcoind Test Fixtures Summary

**Added the require_bitcoind!() macro, BitcoindGuard RAII type, RpcCreds, and bootstrap_regtest_bitcoind() helper to tests/integration/mod.rs — the contract substrate that plans 09-03 and 09-04 consume in parallel to retire Box::leak and the scattered skip-block pattern.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-05-27T02:29:31Z
- **Completed:** 2026-05-27T02:35:03Z
- **Tasks:** 2
- **Files modified:** 1 (`tests/integration/mod.rs` — expanded from 4 lines to 286 lines)

## Accomplishments

- `pub fn require_bitcoind_inner() -> Option<String>` — env-var-gated bitcoind discovery; panics on miss when `BLINDJOIN_REQUIRE_BITCOIND=1` (CI mode), else returns `None` for graceful local-dev skip. Panic message names both `BLINDJOIN_REQUIRE_BITCOIND` and `BITCOIND_EXE` for one-glance CI triage.
- `#[macro_export] macro_rules! require_bitcoind { () => { match $crate::require_bitcoind_inner() { Some(p) => p, None => return, } }; }` — the load-bearing macro that lets tests use a one-line skip without aborting sibling tests in the binary.
- `pub struct RpcCreds { url, user, pass }` (Clone + Debug derives) — canonical RPC credentials handoff between the bootstrap helper and consuming tests. user/pass come from the bitcoind cookie file (`Node::params::get_cookie_values`), not from any configured credentials.
- `pub struct BitcoindGuard { node: Option<corepc_node::Node> }` with `pub fn new(Node) -> Self`, `pub fn node(&self) -> &Node`, and an **explicit** `impl Drop` that calls `node.stop()` (best-effort RPC) then lets `Node::Drop` run `process.kill()` as belt-and-suspenders. Doc comment captures the structural lineage from `coordinator/src/network/tor.rs::ConnectionGuard` and the divergence (ConnectionGuard relies on implicit Drop; BitcoindGuard needs explicit because `stop()` is an RPC call).
- `pub async fn bootstrap_regtest_bitcoind() -> (BitcoindGuard, RpcCreds)` — single locus of regtest bring-up (D-13 + D-14). `tokio::task::spawn_blocking`-bridged; sets `conf.view_stdout = false`; pushes `-printtoconsole=0` (D-15 amended); mines 101 blocks for `coordinator::run`'s `startup_health_check`; returns the (guard, creds) tuple.

## Task Commits

Each task was committed atomically:

1. **Task 1: require_bitcoind_inner() fn + require_bitcoind! macro** — `746152a` (feat)
2. **Task 2: BitcoindGuard + RpcCreds + bootstrap_regtest_bitcoind** — `112e71e` (feat)

**Plan metadata commit:** (this commit)

## Exact Public Signatures Added

```rust
pub fn require_bitcoind_inner() -> Option<String>;

#[macro_export]
macro_rules! require_bitcoind {
    () => {
        match $crate::require_bitcoind_inner() {
            Some(p) => p,
            None => return,
        }
    };
}

#[derive(Clone, Debug)]
pub struct RpcCreds {
    pub url: String,
    pub user: String,
    pub pass: String,
}

pub struct BitcoindGuard { /* node: Option<corepc_node::Node> (private) */ }
impl BitcoindGuard {
    pub fn new(node: corepc_node::Node) -> Self;
    pub fn node(&self) -> &corepc_node::Node;
}
impl Drop for BitcoindGuard {
    fn drop(&mut self) { /* take + n.stop() then Node::Drop runs process.kill() */ }
}

pub async fn bootstrap_regtest_bitcoind() -> (BitcoindGuard, RpcCreds);
```

## Files Created/Modified

- `tests/integration/mod.rs` — Expanded from 4 lines (just `mod X;` declarations) to 286 lines containing all fixtures above. The 4 original module declarations remain at the top, preceded by an inner `//!` doc comment block explaining the file's dual role.

## Decisions Made

- **Macro form over plain fn for `require_bitcoind`** — RESEARCH.md Pattern 2 documents the footgun: a `pub fn require_bitcoind() -> String` form would either need to `panic!` (breaks local-dev opt-in) or `std::process::exit(0)` (aborts the whole binary, masking sibling test failures). The macro is the only correct shape for "skip THIS test only".
- **Inner `//!` doc comment placed BEFORE the `mod X;` declarations** — Rust syntactic constraint: inner doc comments must appear before any items.
- **`view_stdout = false` set explicitly despite being the corepc-node 0.12 default** — Discoverable via grep, robust against a future Conf default flip.
- **`bootstrap_regtest_bitcoind` calls `require_bitcoind_inner()` directly (NOT the `require_bitcoind!()` macro)** — see Deviations below; the macro is type-incompatible with the function's tuple return type. Doc comment instructs tests to invoke the macro themselves before calling bootstrap.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] `require_bitcoind!()` macro is type-incompatible with `bootstrap_regtest_bitcoind`'s tuple return type**

- **Found during:** Task 2 (BitcoindGuard + bootstrap helper)
- **Issue:** The plan's `<action>` block prescribed `let exe = require_bitcoind!();` as the first line of `pub async fn bootstrap_regtest_bitcoind() -> (BitcoindGuard, RpcCreds)`. The macro expands to `match $crate::require_bitcoind_inner() { Some(p) => p, None => return, }`. The bare `return` is `return ();` — a type error inside a function whose declared return type is `(BitcoindGuard, RpcCreds)`. Rustc rejected with `E0069: 'return;' in a function whose return type is not '()'`. This is a genuine bug in the plan's interface contract — the macro cannot be used inside a non-unit-returning function.
- **Fix:** `bootstrap_regtest_bitcoind` calls `require_bitcoind_inner()` directly and uses `.unwrap_or_else(|| panic!(...))` to surface a triage-friendly panic if bitcoind is missing. The skip semantic is preserved at a higher level: tests that want graceful local-dev skip invoke `require_bitcoind!()` themselves (e.g., `let _exe = require_bitcoind!();`) before calling `bootstrap_regtest_bitcoind().await`. This pattern is documented in the helper's doc comment so plans 09-03/09-04 have a stable contract.
- **Files modified:** `tests/integration/mod.rs`
- **Verification:** `cargo test --test integration --no-run` exits 0; `cargo clippy --workspace --all-targets -- -D warnings` exits 0; the function signature `pub async fn bootstrap_regtest_bitcoind() -> (BitcoindGuard, RpcCreds)` is unchanged (still grep-matches the acceptance criterion); the `require_bitcoind!` token still appears multiple times in mod.rs (doc-comment references).
- **Committed in:** `112e71e` (Task 2 commit)
- **Downstream implication:** 09-03/09-04 callsite migrations should follow this pattern:
  ```rust
  #[tokio::test]
  async fn my_test() {
      let _exe = require_bitcoind!();              // skips on miss in local-dev
      let (guard, creds) = bootstrap_regtest_bitcoind().await;
      // ... use creds; hold guard for the test's full duration ...
  }
  ```

**2. [Rule 3 - Blocking issue] Doc comments containing the literal string `Box::leak` violated an acceptance criterion**

- **Found during:** Task 2 verification
- **Issue:** The plan's acceptance criterion `grep -c 'Box::leak' tests/integration/mod.rs` returns 0 is unconditional. Two doc-comment paragraphs initially referenced `Box::leak(node)` by name as the old pattern being replaced — a natural and informative reference, but it tripped the grep.
- **Fix:** Reworded both doc-comment paragraphs to describe the old pattern semantically ("the historical leak-the-Node-so-bitcoind-survives pattern") without using the literal string `Box::leak`. Information density preserved; only the search-token removed.
- **Files modified:** `tests/integration/mod.rs`
- **Verification:** `grep -c 'Box::leak' tests/integration/mod.rs` returns 0; cargo test compile still passes; clippy still clean.
- **Committed in:** `112e71e` (Task 2 commit; same as #1)

---

**Total deviations:** 2 auto-fixed (both Rule 3 — Blocking issue)
**Impact on plan:** Both fixes preserve every public-surface contract that downstream plans (09-03/09-04) depend on. Public signatures of `require_bitcoind_inner`, `require_bitcoind!`, `RpcCreds`, `BitcoindGuard`, and `bootstrap_regtest_bitcoind` are exactly as specified in the plan's `<interfaces>` block. Only the internal implementation of `bootstrap_regtest_bitcoind` (which the spec marked as "internal") changed: it calls `require_bitcoind_inner()` directly rather than the macro. No scope creep; no architectural change.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo test --test integration --no-run` | ✓ exits 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✓ exits 0 |
| `grep -c 'macro_rules! require_bitcoind' tests/integration/mod.rs` | 1 |
| `grep -c 'pub struct BitcoindGuard' tests/integration/mod.rs` | 1 |
| `grep -c 'node: Option<corepc_node::Node>' tests/integration/mod.rs` | 1 |
| `grep -c 'impl Drop for BitcoindGuard' tests/integration/mod.rs` | 1 |
| `grep -c 'pub struct RpcCreds' tests/integration/mod.rs` | 1 |
| `grep -c 'pub async fn bootstrap_regtest_bitcoind() -> (BitcoindGuard, RpcCreds)' tests/integration/mod.rs` | 1 |
| `grep -c 'view_stdout = false' tests/integration/mod.rs` | 2 (≥1) |
| `grep -c -- '-printtoconsole=0' tests/integration/mod.rs` | 2 (≥1) |
| `grep -c 'require_bitcoind!' tests/integration/mod.rs` | 13 (≥1) |
| `grep -c 'Box::leak' tests/integration/mod.rs` | 0 |
| Original 4 `mod X;` declarations preserved | ✓ (head -25 shows all 4) |
| No callsite migration in this plan | ✓ (`git diff --name-only tests/integration/ \| grep -v mod.rs \| wc -l` returns 0) |

## Issues Encountered

None beyond the two deviations documented above. Both were caught by the compiler (E0069) or acceptance-criteria greps and resolved deterministically.

## User Setup Required

None. This plan only adds shared test fixtures; no external service configuration, environment variable, or runtime change.

## Next Phase Readiness

**Plan 09-03 (full_round.rs callsite migration) and Plan 09-04 (rate_limiting.rs + round_bootstrap.rs callsite migration) can run in parallel against the now-stable contract.**

The interface declared in this plan's `<interfaces>` block matches the implementation verbatim, except for the one documented internal change (`bootstrap_regtest_bitcoind` body uses `require_bitcoind_inner()` directly). Downstream plans should use the pattern:

```rust
#[tokio::test]
async fn my_test() {
    let _exe = require_bitcoind!();              // local-dev skip path
    let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;
    // ... rest of test; hold bitcoind_guard for the test's full duration ...
}
```

No blockers for the wave-2 work.

## TDD Gate Compliance

This plan's tasks were marked `tdd="true"` in PLAN.md, but Phase 9 operates in non-TDD mode (`tdd_mode=false` in `.planning/config.json`). Per the TDD gate sequence, no separate `test(...)` commits were created; the implementation commits (`746152a` and `112e71e`) include both the fixture code and the doc-test snippets demonstrating expected usage. The fixtures are not testable in isolation without bitcoind on the build host — they are themselves test infrastructure. Functional verification of the substrate happens transitively in plans 09-03/09-04 when callsites that consume these fixtures execute against a real bitcoind.

## Self-Check: PASSED

- FOUND: `tests/integration/mod.rs`
- FOUND: `.planning/phases/09-ci-integration-test-reliability/09-02-SUMMARY.md`
- FOUND commit: `746152a` (Task 1)
- FOUND commit: `112e71e` (Task 2)

---
*Phase: 09-ci-integration-test-reliability*
*Completed: 2026-05-27*
