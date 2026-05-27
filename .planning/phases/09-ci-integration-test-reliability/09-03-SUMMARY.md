---
phase: 09-ci-integration-test-reliability
plan: 03
subsystem: testing
tags: [integration-tests, raii, drop-guard, bitcoind, regtest, ignore-marker, callsite-migration]

# Dependency graph
requires:
  - phase: 09-02-shared-bitcoind-fixtures
    provides: require_bitcoind!() macro, BitcoindGuard, RpcCreds, bootstrap_regtest_bitcoind()
provides:
  - tests/integration/full_round.rs migrated to shared fixtures (zero Box::leak, zero inline skip blocks)
  - fund_regtest helper now returns (BitcoindGuard, FundedSetup) — RAII-managed across 5 callers
  - 6 RPC-drift Phase-10 carve-out tests carry the canonical #[ignore = "TODO(Phase-10)..."] marker for grep discovery
affects: [09-04 (rate_limiting.rs + round_bootstrap.rs callsite migration — parallel sibling), 10 (REPAIR-01 — removes ignore markers as it repairs each test)]

# Tech tracking
tech-stack:
  added: []  # zero new dependencies; all helpers were added by 09-02
  patterns:
    - Arc<BitcoindGuard> for shared ownership across the spawn_blocking boundary (one Arc keeps bitcoind alive while a clone moves into the closure to drive RPC via guard.node())
    - RpcCreds destructure in async scope so creds can be moved into spawn_blocking that constructs FundedSetup
    - `let _bitcoind_guard = bitcoind_guard;` clippy-safe binding-extension idiom (D-11 invariant: guard must outlive test body)
    - `Arc::try_unwrap` on a function-local Arc inside an async helper (fund_regtest) to return a bare BitcoindGuard to the caller after spawn_blocking has dropped its clone

key-files:
  created: []
  modified:
    - tests/integration/full_round.rs

key-decisions:
  - "Arc<BitcoindGuard> pattern (vs. reconstruct-RPC-client-from-creds) — chosen because corepc-node's Node-borrowed methods (Node::client::new_address, list_unspent, generate_to_address, send_to_address) are the most direct way to drive funding RPC, and Arc<BitcoindGuard> is Send+Sync; the spawn_blocking closure receives an Arc clone, and the outer Arc binding holds bitcoind alive for the rest of the test"
  - "fund_regtest() returns the BitcoindGuard via Arc::try_unwrap — safe because we always create a fresh Arc and the spawn_blocking has fully returned (its clone is dropped) before try_unwrap runs"
  - "Caller migration was lifted from Task 2 into Task 1 (Rule 3 auto-fix) — Task 1's compile acceptance criterion cannot be satisfied without updating the 5 fund_regtest callers in tandem with the signature change; Task 2 retains its narrow scope of metadata-only #[ignore] markers"
  - "Em-dash in the ignore reason string replaced with ASCII `--` per the plan's own action(c) note — deterministic grep, matches the canonical_refs string verbatim, intent (em-dash) documented inline"
  - "coordinator_info_endpoint_fields left alone — confirmed by reading L1292-1400 that it uses a hardcoded http://127.0.0.1:18443 RPC URL inside CoordinatorConfig but never actually invokes bitcoind RPC; it tests the /info endpoint's response shape against an idle coordinator. No bitcoind, no #[ignore] needed."

patterns-established:
  - "Test pattern: `let _exe = require_bitcoind!(); let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;` as the canonical opening of a bitcoind-dependent integration test (sets up local-dev skip + RAII daemon ownership in 2 lines)"
  - "Arc<BitcoindGuard> for sharing the guard between async-scope holder and spawn_blocking funding closure — the closure receives `Arc::clone(&bitcoind_guard)` and dereferences via `.node()`; the outer Arc binding stays alive for the test"
  - "fund_regtest() -> (BitcoindGuard, FundedSetup) — the canonical multi-caller fixture form; callers destructure as `let (bitcoind_guard, setup) = fund_regtest().await;` and hold `_bitcoind_guard` to end-of-test"
  - "ASCII `--` (two hyphens) as the deterministic stand-in for em-dash in grep-searchable ignore-reason strings; documented inline so future maintainers know the intent"

requirements-completed: [TEST-03, TEST-04]

# Metrics
duration: 6min
completed: 2026-05-27
---

# Phase 9 Plan 3: full_round.rs Callsite Migration Summary

**Migrated tests/integration/full_round.rs to the shared 09-02 fixtures — 3 Box::leak callsites and 6 inline skip blocks eliminated, fund_regtest helper returns a BitcoindGuard, and the 6 RPC-drift Phase-10 carve-out tests carry the canonical #[ignore] marker so CI lists them as ignored without executing them.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-05-27T02:39:49Z
- **Completed:** 2026-05-27T02:45:41Z
- **Tasks:** 2
- **Files modified:** 1 (`tests/integration/full_round.rs` — 1665 → 1672 lines net; net +124/-114 in Task 1 and +6/-0 in Task 2)

## Accomplishments

- `tests/integration/full_round.rs` now imports `bootstrap_regtest_bitcoind`, `BitcoindGuard`, `RpcCreds`, and `require_bitcoind` from `crate::` and uses them throughout — zero `Box::leak` calls remain, zero inline `match corepc_node::exe_path()` skip blocks remain in non-comment lines.
- 3 internal-bootstrap test functions migrated to shared bootstrap: `full_round_three_clients`, `blame_non_signer_timeout`, and the `fund_regtest` helper. Each calls `bootstrap_regtest_bitcoind().await` and holds an `Arc<BitcoindGuard>` whose clone is moved into the funding `spawn_blocking` and whose outer binding is held to end-of-test.
- `fund_regtest` signature changed from `async fn fund_regtest(exe: String) -> FundedSetup` to `async fn fund_regtest() -> (BitcoindGuard, FundedSetup)` — its 5 callers (the 4 adversarial tests + `round_restart_and_completion_after_blame`) updated to the tuple-destructure form `let (bitcoind_guard, setup) = fund_regtest().await;` with a trailing `let _bitcoind_guard = bitcoind_guard;` to keep the daemon alive for the test body.
- 6 RPC-schema-drift-broken tests now carry the canonical attribute `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]`:
  1. `full_round_three_clients`
  2. `blame_non_signer_timeout`
  3. `adversarial_replay_token`
  4. `adversarial_invalid_utxo`
  5. `adversarial_wrong_denomination`
  6. `round_restart_and_completion_after_blame`
- The 2 currently-passing tests (`adversarial_tampered_psbt_rejected`, `coordinator_info_endpoint_fields`) remain non-ignored — CI's PASS verdict is observable (TEST-02).
- Runtime verification: locally with `BITCOIND_EXE=/opt/homebrew/bin/bitcoind`, `cargo test --test integration full_round::` reports `test result: ok. 2 passed; 0 failed; 6 ignored`, and the 6 ignored tests print their canonical reason string at list time.

## Task Commits

Each task was committed atomically:

1. **Task 1: migrate full_round.rs callsites to shared fixtures (+ caller updates)** — `caf4bc1` (refactor)
2. **Task 2: add #[ignore] markers to 6 Phase-10 carve-out tests** — `8cdae5b` (test)

**Plan metadata commit:** (this commit)

## Exact Patterns Used

### (a) fund_regtest call-pattern: Arc<BitcoindGuard> (option ii of the plan)

Chose the **Arc<BitcoindGuard>** option (plan action(b) sub-option ii) over reconstruct-client-from-creds (option i). The Arc clone moves into `tokio::task::spawn_blocking`, while the outer Arc binding keeps bitcoind alive for the rest of the test. This is the most surgical change — funding logic continues to drive `Node::client` directly (same RPC methods as the pre-migration code) and there are no new corepc-node Client constructions:

```rust
#[tokio::test]
async fn full_round_three_clients() {
    let _exe = require_bitcoind!();
    let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;
    let bitcoind_guard = Arc::new(bitcoind_guard);

    let guard_for_setup = Arc::clone(&bitcoind_guard);
    let RpcCreds { url: rpc_url, user: rpc_user, pass: rpc_pass } = creds;
    let setup: FundedSetup = tokio::task::spawn_blocking(move || {
        let node = guard_for_setup.node();
        // ... drive funding via node.client.* ...
        FundedSetup { rpc_url, rpc_user, rpc_pass, utxos: [...] }
    }).await.expect("setup spawn_blocking panicked");

    let _bitcoind_guard = bitcoind_guard;  // hold to end-of-test
    // ...
}
```

`fund_regtest` follows the same shape but takes the additional step of unwrapping the Arc back to a bare `BitcoindGuard` before returning, so the caller does not have to deal with `Arc<BitcoindGuard>`:

```rust
let bitcoind_guard = Arc::try_unwrap(bitcoind_guard).unwrap_or_else(|_| {
    panic!("fund_regtest: BitcoindGuard Arc still has outstanding clones — \
            this is a bug; the spawn_blocking closure should have dropped its clone")
});
(bitcoind_guard, setup)
```

`Arc::try_unwrap` is safe here because we always create the Arc fresh inside `fund_regtest` (no caller can have a clone), the spawn_blocking closure has already returned (its `guard_for_setup` clone is dropped), and the `unwrap_or_else` panic message is a debugging aid that should never fire — the assertion is documented but not load-bearing.

### (b) coordinator_info_endpoint_fields: left alone

Read L1292-1400 to confirm. The test:
- Constructs a `CoordinatorConfig` with `bitcoin_rpc_url: "http://127.0.0.1:18443"` and dummy user/pass `"test"/"test"` — no real bitcoind is required.
- Constructs a `BitcoinRpc::new(...)` and an idle `RoundState::new_idle()` (no Phase::InputReg transition that would require bitcoind).
- Spawns the axum router and tests the `/info` endpoint's response shape: `round_state == "idle"`, `denomination_sats`, `min_participants`, `network`, `rsa_pubkey_hash.is_none()`, etc.

The test never invokes any RPC method on `BitcoinRpc`, so it does not depend on a live bitcoind. **No migration applied; no #[ignore] added.** This matches the plan's expectation in `<acceptance_criteria>` ("the count from `grep -c 'fund_regtest()'` is at least 6 (1 definition + 5 callers; if coordinator_info_endpoint_fields uses bitcoind, count is 7)") and PATTERNS.md L253 ("`coordinator_info_endpoint_fields` (line 1293) — verify with planner").

### (c) Exact ignore marker string

Used the ASCII `--` (two hyphens, single space each side) as the deterministic on-disk form of the em-dash documented in CONTEXT.md / PATTERNS.md. Verbatim:

```
#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
```

Confirmed via `grep -c 'RPC schema drift on listunspent/getrawtransaction -- see TODO.md' tests/integration/full_round.rs` returning exactly 6. The em-dash is the documented intent; the `--` is the on-disk form for grep determinism. Phase 10 can find every marker via either `grep TODO\(Phase-10\)` (6 hits) or `grep '#\[ignore = "TODO(Phase-10)'` (6 hits).

### (d) Grep counts (acceptance criteria evidence)

All taken via `grep -c` on `tests/integration/full_round.rs` at end of Task 2:

| Acceptance criterion | Result | Required |
|----------------------|--------|----------|
| `Box::leak` | 0 | == 0 |
| `use crate::{bootstrap_regtest_bitcoind` (imports) | 1 | == 1 |
| `bootstrap_regtest_bitcoind` (all occurrences) | 9 | ≥ 3 |
| `bootstrap_regtest_bitcoind().await` | 3 | ≥ 3 (full_round_three_clients, blame_non_signer_timeout, fund_regtest body) |
| `fn fund_regtest()` (new signature) | 1 | == 1 |
| `fn fund_regtest(exe: String)` (old signature) | 0 | == 0 |
| `fund_regtest(exe` (callsite with exe) | 0 | == 0 |
| `match corepc_node::exe_path` (non-comment) | 0 | == 0 |
| `#[tokio::test]` | 8 | == 8 |
| `#[ignore` (total) | 6 | == 6 |
| `TODO(Phase-10)` | 6 | == 6 |
| `#[ignore = "TODO(Phase-10)` | 6 | == 6 |
| `RPC schema drift on listunspent/getrawtransaction -- see TODO.md` | 6 | == 6 |
| `require_bitcoind!()` (invocations) | 8 | ≥ 1 per non-ignored bitcoind test |

Per-test ignore confirmation (each `grep -B1 'async fn X' \| grep -c '#\[ignore'`):

| Function | Has #[ignore]? | Required |
|----------|---------------|----------|
| full_round_three_clients | 1 | 1 |
| blame_non_signer_timeout | 1 | 1 |
| adversarial_replay_token | 1 | 1 |
| adversarial_invalid_utxo | 1 | 1 |
| adversarial_wrong_denomination | 1 | 1 |
| round_restart_and_completion_after_blame | 1 | 1 |
| adversarial_tampered_psbt_rejected | 0 | 0 |
| coordinator_info_endpoint_fields | 0 | 0 |

### (e) Local-bitcoind PASS check

Ran locally — bitcoind is available at `/opt/homebrew/bin/bitcoind` (matches the project's brew install):

```
BITCOIND_EXE=/opt/homebrew/bin/bitcoind \
    cargo test --test integration adversarial_tampered_psbt_rejected -- --nocapture
```

Result: `test result: ok. 1 passed; 0 failed; 0 ignored`. The test's `eprintln!` output confirms: `adversarial_tampered_psbt_rejected PASSED: tampered PSBT rejected with: PSBT has 2 denomination outputs but 3 participants registered — refusing to sign (possible output censorship)`.

Additional check — running all 8 full_round tests locally:

```
$ cargo test --test integration full_round::
running 8 tests
test full_round::adversarial_invalid_utxo ... ignored, TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md
test full_round::adversarial_replay_token ... ignored, TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md
test full_round::adversarial_wrong_denomination ... ignored, TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md
test full_round::blame_non_signer_timeout ... ignored, TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md
test full_round::full_round_three_clients ... ignored, TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md
test full_round::round_restart_and_completion_after_blame ... ignored, TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md
test full_round::coordinator_info_endpoint_fields ... ok
test full_round::adversarial_tampered_psbt_rejected ... ok

test result: ok. 2 passed; 0 failed; 6 ignored; 0 measured; 7 filtered out
```

This is the exact verdict shape D-21 documents for CI.

## Decisions Made

- **Arc<BitcoindGuard> over reconstruct-RPC-client** — Cleanest preservation of the pre-migration funding code (every RPC method continues to be invoked on `Node::client::*`); the Arc clone moves into spawn_blocking and the outer binding keeps bitcoind alive for the rest of the test. Verified that `Node` is Send (per 09-02's RESEARCH.md Pitfall 4 + Assumption A1), so `Arc<BitcoindGuard>` crosses the `.await` boundary cleanly.
- **`Arc::try_unwrap` in fund_regtest body** — Safe because the Arc is created fresh inside the helper, the spawn_blocking has fully returned (its clone is dropped) by the time try_unwrap runs, and no caller can have a clone. The `unwrap_or_else` panic is defensive — it should never fire — but documents the invariant.
- **`let _bitcoind_guard = bitcoind_guard;` (named binding, leading underscore)** — Clippy-safe form for "hold to end-of-scope, name documents intent". Pure `let _ = bitcoind_guard;` would drop immediately (bug — would terminate bitcoind mid-test); `let _bitcoind_guard = bitcoind_guard;` extends the binding to end-of-scope, which is what D-11 requires.
- **ASCII `--` instead of em-dash in ignore reason** — Per the plan's own action(c) note: the prose in CONTEXT.md/PATTERNS.md uses U+2014 em-dash, but for deterministic grep we use the `--` form. Verified via `grep -c 'RPC schema drift on listunspent/getrawtransaction -- see TODO.md'` returning exactly 6.
- **coordinator_info_endpoint_fields left alone** — Read the body (L1292-1400) and confirmed it uses dummy RPC creds and never invokes a live bitcoind. No #[ignore], no migration.

## Files Created/Modified

- `tests/integration/full_round.rs` — single file modified across 2 commits:
  - Commit `caf4bc1` (Task 1): +124 / -114 lines. Imports, full_round_three_clients, blame_non_signer_timeout, fund_regtest helper migration + 5 caller updates.
  - Commit `8cdae5b` (Task 2): +6 / -0 lines. Exactly 6 `#[ignore = "..."]` attribute lines, one per RPC-drift-broken test.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking issue] Task 1 cannot satisfy compile acceptance criterion without also updating the 5 fund_regtest callers**

- **Found during:** Task 1 (after migrating fund_regtest signature)
- **Issue:** The plan's `<action>` step (d) says "the 4 adversarial tests + round_restart_and_completion_after_blame; addressed in Task 2" — explicitly deferring caller updates to Task 2. But Task 1's `<acceptance_criteria>` requires `cargo test --test integration --no-run` to exit 0 AND `grep -c 'fund_regtest(exe' tests/integration/full_round.rs` to return 0. Both are unsatisfiable if the 5 callers still pass `exe` (compile fails with E0061/E0609; grep returns 5). The plan is internally inconsistent: the *signature change* and *caller updates* must land together to compile.
- **Fix:** Lifted the 5 caller updates from Task 2 into Task 1. Each caller now uses `let (bitcoind_guard, setup) = fund_regtest().await;` followed by `let _bitcoind_guard = bitcoind_guard;` after the coordinator setup. Task 2's scope shrinks to metadata-only — adding the 6 `#[ignore]` attributes — which is what the plan intended for that task at the structural level (a "pure, easily-revertible commit").
- **Files modified:** `tests/integration/full_round.rs` (within Task 1's commit `caf4bc1`)
- **Verification:** Task 1 compile (`cargo test --test integration --no-run` exits 0) + all Task 1 grep counts pass + clippy clean.
- **Committed in:** `caf4bc1` (Task 1)

**2. [Rule 3 - Blocking issue] `require_bitcoind!()` macro not auto-imported by `#[macro_export]` reaching crate root**

- **Found during:** Task 1 first compile attempt (after writing the migration but before adjusting imports)
- **Issue:** 09-02 documented that the `#[macro_export]` on `macro_rules! require_bitcoind` in `tests/integration/mod.rs` should make it reachable as bare `require_bitcoind!()` from submodules ("`#[macro_export]`-ed macros defined here are therefore reachable as `crate::require_bitcoind!()` from each `mod X;` submodule"). In practice, the `mod.rs` doc comment is accurate that the macro can be invoked as `crate::require_bitcoind!()` or `$crate::require_bitcoind!()` (the expansion is `$crate::require_bitcoind_inner()`), but the **bare unqualified form** `require_bitcoind!()` requires an explicit `use crate::require_bitcoind;` import in the consuming submodule. Rustc's compile error confirmed: `cannot find macro 'require_bitcoind' in this scope ... consider importing this macro through its public re-export`.
- **Fix:** Added `require_bitcoind` to the existing `use crate::{...}` import line: `use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};`. This brings the macro into scope so the bare `require_bitcoind!()` form compiles in each test body.
- **Files modified:** `tests/integration/full_round.rs` (within Task 1's commit `caf4bc1`)
- **Verification:** `cargo test --test integration --no-run` exits 0; `cargo clippy` clean. All 8 invocations of `require_bitcoind!()` in the file compile.
- **Committed in:** `caf4bc1` (Task 1)
- **Downstream implication:** Plan 09-04 (the parallel sibling) will need the same import line in `rate_limiting.rs` and `round_bootstrap.rs`. The plan's `<interfaces>` block says "the require_bitcoind macro is `#[macro_export]`-ed at the crate root ... and may be invokable as bare `require_bitcoind!()` thanks to macro_export reaching crate scope; if rustc complains, fall back to `crate::require_bitcoind!()` at each callsite" — this deviation confirms rustc *does* complain in the bare form and the canonical fix is the import, not the fully-qualified form (the import is more readable and a one-liner). Plan 09-04 should add `use crate::require_bitcoind;` (or merge with the bootstrap import line) before its first invocation.

---

**Total deviations:** 2 auto-fixed (both Rule 3 — Blocking issue)
**Impact on plan:** Neither deviation changes the deliverable. (1) is a scope re-balancing between Tasks 1 and 2 that the plan's own acceptance criteria forced; (2) is a 1-token import addition that satisfies a Rust scoping requirement the plan documented as a possible fallback. All success criteria — including TEST-03 and TEST-04 partial closure — are met.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo test --test integration --no-run` | exits 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exits 0 |
| `grep -c 'Box::leak' tests/integration/full_round.rs` | 0 |
| `grep -c 'use crate::{bootstrap_regtest_bitcoind' tests/integration/full_round.rs` | 1 |
| `grep -c 'bootstrap_regtest_bitcoind().await' tests/integration/full_round.rs` | 3 |
| `grep -c 'fn fund_regtest()' tests/integration/full_round.rs` | 1 |
| `grep -c 'fn fund_regtest(exe: String)' tests/integration/full_round.rs` | 0 |
| `grep -c 'fund_regtest(exe' tests/integration/full_round.rs` | 0 |
| `grep -v '^[[:space:]]*//' tests/integration/full_round.rs \| grep -c 'match corepc_node::exe_path'` | 0 |
| `grep -c '#\[tokio::test\]' tests/integration/full_round.rs` | 8 |
| `grep -c '#\[ignore' tests/integration/full_round.rs` | 6 |
| `grep -c 'TODO(Phase-10)' tests/integration/full_round.rs` | 6 |
| `grep -c '#\[ignore = "TODO(Phase-10)' tests/integration/full_round.rs` | 6 |
| `grep -c 'RPC schema drift on listunspent/getrawtransaction -- see TODO.md' tests/integration/full_round.rs` | 6 |
| `grep -c 'require_bitcoind!()' tests/integration/full_round.rs` | 8 |
| Per-test #[ignore] presence (6 broken tests) | each 1 |
| Per-test #[ignore] absence (adversarial_tampered_psbt_rejected, coordinator_info_endpoint_fields) | each 0 |
| Local bitcoind run of adversarial_tampered_psbt_rejected | PASSED |
| `cargo test --test integration full_round::` (locally with bitcoind) | `2 passed; 0 failed; 6 ignored` |

## Issues Encountered

The 2 deviations documented above. Both were caught immediately by rustc (E0061/E0609 + macro-not-found) and resolved deterministically.

## User Setup Required

None. This plan only modifies test code; no environment variables, no external service configuration. CI's `BLINDJOIN_REQUIRE_BITCOIND=1` was set by Plan 09-01 and the workflow-level env var is consumed by `require_bitcoind_inner()` (defined in Plan 09-02), now wired through `require_bitcoind!()` at each of the 8 invocation sites in this file.

## Next Phase Readiness

**Plan 09-04 (rate_limiting.rs + round_bootstrap.rs callsite migration) can proceed in parallel.** It uses the identical pattern documented here:

```rust
#[tokio::test]
async fn my_test() {
    let _exe = require_bitcoind!();
    let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;
    // ... use creds; hold bitcoind_guard via `let _bitcoind_guard = bitcoind_guard;` to end-of-test ...
}
```

For tests that need to drive the Node directly (mining additional blocks, custom funding), wrap as `Arc<BitcoindGuard>` and pass a clone into spawn_blocking — same pattern as `full_round_three_clients` in this plan. Plan 09-04's callsites are smaller (1 Box::leak in `rate_limiting.rs`, 1 in `round_bootstrap.rs`) and `round_bootstrap.rs` is the cleanest analog (no funding step) — see PATTERNS.md.

After Plan 09-04 lands, the entire `tests/integration/` tree will be Box::leak-free and skip-block-free, and the success criteria of TEST-03 + TEST-04 will be fully closed.

## TDD Gate Compliance

This plan's tasks were marked `tdd="true"` in PLAN.md, but Phase 9 operates in non-TDD mode (`tdd_mode=false` in `.planning/config.json`). Per the same convention as Plan 09-02, no separate `test(...)` commits were created — the refactor commits (`caf4bc1`, `8cdae5b`) include the migrated tests and acceptance-criteria evidence. The tests themselves *are* the behavior; the migration preserves their pre-migration assertion logic verbatim and only changes the daemon bring-up boilerplate.

## Self-Check: PASSED

- FOUND: `tests/integration/full_round.rs`
- FOUND: `.planning/phases/09-ci-integration-test-reliability/09-03-SUMMARY.md`
- FOUND commit: `caf4bc1` (Task 1 — refactor)
- FOUND commit: `8cdae5b` (Task 2 — test)

---
*Phase: 09-ci-integration-test-reliability*
*Completed: 2026-05-27*
