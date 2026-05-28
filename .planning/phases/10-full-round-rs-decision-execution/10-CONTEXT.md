# Phase 10: full_round.rs decision + execution - Context

**Gathered:** 2026-05-27
**Status:** Ready for planning

<domain>
## Phase Boundary

With Phase 9's CI substrate now actually executing the integration test suite end-to-end on every PR, Phase 10 closes the remaining v1.3 test-infrastructure gap: the 6 `#[ignore]`'d "Phase-10 carve-out" tests in `tests/integration/full_round.rs` get **repaired** (not retired) against the pinned Bitcoin Core v30.2 / corepc-node 0.12 `features=["30_2"]` schema. Phase 9's deferred WR-05 bare-sleep migrations get folded into the same plans that touch the affected test code. A workspace-wide CI invariant locks in the explicit `corepc-node` feature-pin requirement so future Cargo.toml additions can't silently regress to the corepc-node 0.17.2 default.

**Concretely in scope:**
- Port the listunspent/getrawtransaction call sites in `tests/integration/full_round.rs::fund_regtest` (and any other drift-affected code paths) to corepc-node 0.12's typed v30 Client. The shared `fund_regtest` plumbing gets promoted to `tests/integration/mod.rs` as a reusable helper, matching the consolidation pattern Phase 9 established for `bootstrap_regtest_bitcoind` / `BitcoindGuard` / `require_bitcoind!()`.
- Unmute the 6 carve-out tests one-at-a-time, removing the `#[ignore = "TODO(Phase-10)..."]` marker only after each test PASSes locally against brew bitcoind v31 AND PASSes in CI against the pinned v30.2.
- Fix the 4 deferred WR-05 bare-sleep sites at `full_round.rs:369, 704, 1519, 1627` while in the same test code — replace `sleep(2s)` / `sleep(4s)` with poll-until-deadline (e.g., wait for round phase to advance with a bounded budget).
- Add a new tiny CI job (`corepc-node feature pin check`) that fails closed on any `corepc-node = ...` Cargo.toml line lacking an explicit `features = ...` clause. Future Cargo.toml additions can't silently regress.
- Correct the stale "15 tests" language in ROADMAP.md Phase 10 entry + success criterion 1 + REQUIREMENTS.md REPAIR-01 entry to the actual count of 8 (6 ignored carve-outs + 2 already-passing).

**Explicitly NOT in scope:**
- Tor-mode integration harness (still deferred to v1.4+ per REQUIREMENTS.md "Future Requirements" — Phase 8 HUMAN-UAT item 3 remains `result: deferred`).
- Mainnet enablement / protocol changes / new features (this milestone is test infrastructure only).
- Migrating any production code off corepc-node — the production coordinator does NOT use corepc-node; it uses direct reqwest + corepc-types JSON-RPC. corepc-node is dev-only (test harness).

**Per-test escape valve (fallback only):** If a specific test cannot be cleanly repaired within Plan 10-02's scope, that ONE test alone may retire — delete the test function, document rationale in TODO.md "Resolved" section (what was tried, why it didn't work, what coverage gap remains), and file a `B-04+` entry in BACKLOG.md naming the protocol scenario lost + a sketch of how a future test could cover it differently (property-based via proptest, isolated state-machine test without bitcoind, etc.). The other 5 still get repaired. Phase 10 still ships green CI. This escape valve is for genuinely-stuck repairs, NOT a planning shortcut.

</domain>

<decisions>
## Implementation Decisions

### Scope policy

- **D-01 (scope):** Repair all 6 ignored tests. Each covers a unique multi-client end-to-end scenario unit tests can't replicate: full 3-participant round, blame-timeout flow, replay-token attack, invalid-UTXO validation against real bitcoind, wrong-denomination check, post-blame round restart. The drift is mechanical (listunspent/getrawtransaction response shape changed between corepc-node 0.10 → 0.12 with feature `30_2`); the fix is a one-time port. Cost-to-keep < value-of-coverage.

- **D-02 (WR-05 fold-in):** The 4 Phase-9-deferred bare-sleep sites at `full_round.rs:369, 704, 1519, 1627` all live inside tests that are getting repaired anyway. Replace each `sleep(2s)` / `sleep(4s)` with poll-until-deadline (e.g., `tokio::time::timeout(Duration::from_secs(2), wait_for_phase(...)).await`) while touching the same test code. Doing it now avoids a separate Phase 10.1 / Phase 11 follow-up.

- **D-03 (doc count correction):** Phase 10 corrects "15 tests" → "8 tests (6 ignored carve-outs + 2 already-passing)" in three places: ROADMAP.md Phase 10 goal, ROADMAP.md Phase 10 success criterion 1, REQUIREMENTS.md REPAIR-01. Documentation accuracy is a phase deliverable, not deferred work.

- **D-04 (plan grouping):** 2 plans:
  - **Plan 10-01 — Schema port:** Port listunspent/getrawtransaction calls + `fund_regtest` helper to corepc-node 0.12's v30 client schema in one pass. Promote `fund_regtest` (or its successor) to `tests/integration/mod.rs` as a shared helper. No `#[ignore]` markers touched yet. Acceptance: shared helper compiles, exists, has signature matching the documented API contract.
  - **Plan 10-02 — Unmute + sleeps + doc fix:** Unmute the 6 carve-out tests one-at-a-time. For each test: (a) verify local PASS against brew bitcoind v31, (b) verify CI PASS against pinned v30.2, (c) remove the `#[ignore]` marker, (d) fix any bare-sleep sites inside that test (WR-05). Also: REPAIR-02 CI grep check + the "15 → 8" doc corrections. Acceptance per test: local + CI both PASS; whole-file invariant: `grep -c '^#\[ignore' tests/integration/full_round.rs` returns 0 at end of plan.

### Repair approach

- **D-05 (API choice):** Port to **corepc-node 0.12 typed v30 Client** (the `corepc_node::Client` API at `features=["30_2"]`). Already pinned at `coordinator/Cargo.toml:65`; the typed responses for `listunspent` / `getrawtransaction` at this feature flag have the correct v30+ shape (descriptor wallets, modern field names). Simplest fix, no new dependencies, matches the Phase 9 fixture style. Direct reqwest + corepc-types was considered (more fidelity to production coordinator) and rejected as scope creep — tests don't need to exercise the production RPC wire format; the typed client is the right level for test plumbing.

- **D-06 (plumbing location):** Promote the new RPC plumbing (the funded-regtest setup that consumes the v30 typed client) to `tests/integration/mod.rs` as a shared helper. Same consolidation pattern Phase 9 established. Any future test (the v1.4+ Tor harness, additional adversarial scenarios, etc.) reuses without reimplementing. Concrete signature: `pub async fn fund_regtest(guard: &BitcoindGuard, /* ...funding params... */) -> FundedSetup;` (or equivalent — exact signature is Claude's discretion; the contract is "given a BitcoindGuard, return a FundedSetup that lets a coordinator+clients run an end-to-end round"). The 6 unmuted tests `use crate::fund_regtest;` instead of the file-private version.

- **D-07 (per-test acceptance):** Each unmuted test must satisfy BOTH:
  1. **Local PASS:** `BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round::<test_name> -- --nocapture` exits 0 with `test result: ok. 1 passed`.
  2. **CI PASS:** The test appears as `... ok` in the PR's `cargo test` CI log against pinned v30.2 (not just "compiles," not "passes locally only").
  The `#[ignore]` marker is removed only after both. Same gate as Phase 9 UAT-1. This is per-test (not aggregate), so each unmute is its own atomic verification step.

### REPAIR-02 enforcement

- **D-08 (mechanism):** New tiny CI job in `.github/workflows/ci.yml` titled `corepc-node feature pin check`. Runs in parallel with `cargo test` / `cargo clippy` / `cargo audit` / `coordinator binary builds`. ~10 lines of YAML. Self-documenting failure name. Matches the project's "small focused job per gate" pattern.

- **D-09 (grep semantics):** Negative-match: catch lines that declare `corepc-node = ...` but lack a `features = ...` clause. Concrete shape (planner refines exact regex):
  ```
  grep -rEn 'corepc-node\s*=' --include='Cargo.toml' . \
    | grep -v 'features\s*=' \
    | grep -v '^[^:]*:[0-9]*:#' \
    && exit 1 || exit 0
  ```
  Catches `corepc-node = "0.12"` (no features), tolerates commented-out lines and the multi-line table form. Fails closed with exit 1 on any unfeatured match. Future minor bumps (`features = ["30_2"]` → `features = ["30_3"]`) don't require workflow edits — only adding a corepc-node entry WITHOUT features triggers failure.

### Coverage rescue

- **D-10 (escape valve):** If — and only if — a specific test cannot be repaired within Plan 10-02's scope (e.g., a v30 schema change is deeper than expected for one test's specific RPC flow), that ONE test alone may retire. Procedure:
  1. Delete the test function from `full_round.rs`.
  2. Add a paragraph (1–3 sentences) under TODO.md "Resolved 2026-05-27" describing: what was tried, the specific blocker, what coverage gap remains, what alternative coverage would close the gap.
  3. File a `B-04+` (next free B-number) entry in `.planning/BACKLOG.md` matching the Phase 8 B-01/B-02/B-03 format: protocol scenario lost, code reference of what was deleted, sketch of how a future test could cover it differently (property-based via proptest? isolated state-machine test without bitcoind? Tor-mode harness?). This is NOT a planning shortcut — repair is the default; retirement is a fallback for genuinely-stuck repairs.

- **D-11 (default expectation):** Plan 10-02 must aim for 0 retirements. If the executor finds itself reaching for the escape valve on more than 1 test, that's a signal to stop, surface the blocker to the user, and discuss whether the scope decision (D-01) needs revisiting in a follow-up checkpoint rather than ploughing through.

### Claude's Discretion

- Exact poll-until-deadline implementation for WR-05 fixes (D-02). Reasonable choices: `tokio::time::timeout` wrapping `wait_for_phase`; explicit poll loop with 100ms tick + 2s deadline; or whatever pattern reads cleanest at each site.
- Exact signature of the promoted `fund_regtest` helper (D-06). The contract is "given a BitcoindGuard, return a FundedSetup."
- Whether the CI grep check (D-08/D-09) lives as a `script:` block inline in ci.yml or extracts to `scripts/ci/check-corepc-node-pin.sh`. Inline is simpler; script is reusable. Default to inline unless the regex grows past ~5 lines.
- Whether to additionally add a `tests/integration/mod.rs` doc-comment block above the new `fund_regtest` helper that summarizes the v30 schema gotchas the planner / executor learned during the port. Useful for future maintainers; small extra cost.
- Whether Plan 10-02 commits per-test (6 commits like `test(10-02): unmute full_round_three_clients`) or in one batch commit (`test(10-02): unmute 6 carve-out tests`). Per-test gives finer-grained git bisect / revert; batch is fewer commits. Both acceptable.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase goal + requirements
- [`.planning/ROADMAP.md`](../../ROADMAP.md) §Phase 10 — Goal, 3 success criteria, dependency on Phase 9. **Note: count "15 tests" is stale; actual is 8 (6 ignored + 2 passing). Phase 10 corrects this language per D-03.**
- [`.planning/REQUIREMENTS.md`](../../REQUIREMENTS.md) §"Test Repair (REPAIR)" — REPAIR-01, REPAIR-02. **Same "15" → "8" correction applies to REPAIR-01.**
- [`.planning/PROJECT.md`](../../PROJECT.md) §"Current Milestone: v1.3" — milestone goal framing.

### Phase 9 carry-forward
- [`.planning/phases/09-ci-integration-test-reliability/09-CONTEXT.md`](../09-ci-integration-test-reliability/09-CONTEXT.md) §"D-10 amended" — documents the `#[ignore]` carve-out this phase removes. The marker string Phase 10 removes is exactly: `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]`.
- [`.planning/phases/09-ci-integration-test-reliability/09-REVIEW.md`](../09-ci-integration-test-reliability/09-REVIEW.md) §WR-05 — documents the 4 bare-sleep sites Phase 10 fixes (D-02). Sites are at `full_round.rs:369, 704, 1519, 1627`.
- [`.planning/phases/09-ci-integration-test-reliability/09-REVIEW-FIX.md`](../09-ci-integration-test-reliability/09-REVIEW-FIX.md) — documents which Phase 9 fixes landed and which deferred (WR-05 deferred to Phase 10).

### Code to modify
- [`tests/integration/full_round.rs`](../../../tests/integration/full_round.rs) — The 6 ignored tests at lines 163, 560, 961, 1085, 1142, 1467 (marker on the line above each `#[tokio::test]`). The `fund_regtest` helper at line 765. The 4 bare-sleep sites at 369, 704, 1519, 1627.
- [`tests/integration/mod.rs`](../../../tests/integration/mod.rs) — Destination for the promoted `fund_regtest` shared helper (D-06). Already hosts the Phase 9 `BitcoindGuard` / `bootstrap_regtest_bitcoind` / `require_bitcoind!()` fixtures.
- [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) — Add new `corepc-node feature pin check` job (D-08, D-09). Existing jobs: `cargo test`, `cargo clippy`, `cargo audit`, `coordinator binary builds`.
- [`.planning/ROADMAP.md`](../../ROADMAP.md) — Correct Phase 10 goal text + success criterion 1 ("15 tests" → "8 tests, 6 carve-outs to repair") per D-03.
- [`.planning/REQUIREMENTS.md`](../../REQUIREMENTS.md) — Correct REPAIR-01 entry ("all 15 tests pass" → "all 8 tests pass") per D-03.

### Code to read (not modify)
- [`coordinator/Cargo.toml`](../../../coordinator/Cargo.toml) §lines 61–69 — `corepc-node = { version = "0.12", features = ["30_2"] }`. The schema target. Only Cargo.toml line in the workspace that uses corepc-node today.
- [`coordinator/src/bitcoin/rpc.rs`](../../../coordinator/src/bitcoin/rpc.rs) — Production RPC pattern (reqwest + corepc-types) for reference; explicitly NOT what Phase 10 uses (D-05 picks typed client).
- [`tests/integration/rate_limiting.rs`](../../../tests/integration/rate_limiting.rs), [`round_bootstrap.rs`](../../../tests/integration/round_bootstrap.rs) — Phase 9 examples of tests consuming the shared `mod.rs` fixtures; pattern to match for the unmuted full_round tests.

### Files to create / modify
- New CI job in `.github/workflows/ci.yml` per D-08.
- Optional: `tests/integration/mod.rs` doc-comment block above `fund_regtest` (Claude's discretion).

### Operator-side / project context
- [`TODO.md`](../../../TODO.md) §"Resolved 2026-05-27" — Phase 9 closure entry; Phase 10 retirements (if any) go in a new "Resolved YYYY-MM-DD" entry per D-10.
- [`.planning/BACKLOG.md`](../../BACKLOG.md) — `B-04+` entries for any retired test scenarios per D-10 / D-11.

### Crate / external doc references
- `corepc-node` 0.12 with `features=["30_2"]` — typed Client API for Bitcoin Core v30+ schema. Docs: https://docs.rs/corepc-node/0.12. Particularly the `listunspent`, `getrawtransaction`, `send_to_address`, `generate_to_address` types at the `30_2` feature flag.
- Bitcoin Core v30 release notes (relevant for listunspent/getrawtransaction shape changes): https://github.com/bitcoin/bitcoin/blob/v30.x/doc/release-notes.md (planner consults).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`tests/integration/mod.rs` shared fixtures** (Phase 9) — `BitcoindGuard` RAII, `bootstrap_regtest_bitcoind()`, `require_bitcoind!()` macro, `RpcCreds` struct. Plan 10-01's promoted `fund_regtest` consumes `BitcoindGuard` and probably constructs a `corepc_node::Client` from `RpcCreds.url + .user + .pass` for the funding RPC calls.
- **`coordinator/src/bitcoin/rpc.rs`** — Production reqwest-based JSON-RPC pattern. Not what Phase 10 uses, but reference for what "real" RPC plumbing looks like vs the typed client.
- **CI job pattern** ([`.github/workflows/ci.yml:107–148`](../../../.github/workflows/ci.yml)) — `cargo clippy`, `cargo audit`, `coordinator binary builds` are all small focused jobs. New `corepc-node feature pin check` (D-08) follows the same shape: `runs-on: ubuntu-latest`, checkout step (no toolchain needed for grep), one `Run …` step.

### Established Patterns
- **`#[ignore = "…"]` markers** — Phase 9 established the form `#[ignore = "TODO(Phase-N): <reason>"]`. Phase 10 REMOVES these (not adds); future phases that need to defer tests should follow the same form.
- **`BLINDJOIN_REQUIRE_BITCOIND=1` env-var gate** — Phase 9 invariant; the 6 unmuted tests inherit this automatically via `require_bitcoind!()`.
- **SHA-pinned GitHub Actions** — Phase 6 baseline. The new CI job's `actions/checkout@…` uses the SAME 40-char SHA already pinned in the other jobs (don't pick a different one); no new actions are introduced.
- **`tokio::time::timeout` for bounded waits** — Phase 9 established this idiom (see `BitcoindGuard::drop` using `spawn_blocking + timeout`); WR-05 sleep fixes can follow the same shape.

### Integration Points
- **`tests/integration/mod.rs`** — single locus for shared test plumbing. `fund_regtest` joins `BitcoindGuard`, `bootstrap_regtest_bitcoind`, etc.
- **`.github/workflows/ci.yml`** — new top-level job entry alongside existing four. No env-block changes needed.
- **`coordinator/Cargo.toml:65`** — the existing corepc-node declaration ALREADY satisfies the REPAIR-02 invariant. Phase 10's CI check is exclusively future-proofing; no Cargo.toml edits are required for compliance today.

### Critical constraint on RPC drift
- The drift surfaced when corepc-node bumped from 0.10 to 0.12 with `features=["30_2"]` (Phase 8 closeout, per TODO.md). corepc-node 0.12 still defaults to a Bitcoin Core 0.17.2 (2018!) RPC schema unless an explicit version feature is enabled — that's why `listunspent` / `getrawtransaction` shapes diverged. The fix: use the typed Client at `features=["30_2"]` which has v30-shaped responses. The drift will recur if any future Cargo.toml entry forgets the features clause — exactly what the REPAIR-02 CI gate (D-08) prevents.

</code_context>

<specifics>
## Specific Ideas

- The user's bias is toward **repair, not retire**. The 6 ignored tests are protocol-critical multi-client e2e coverage; unit tests don't reach this layer. The escape valve (D-10) is genuinely a fallback for stuck repairs, not a planning shortcut — Plan 10-02 should expect 0 retirements and surface anything else as a checkpoint.
- The "15 tests" stale count in ROADMAP/REQ caught during this discussion is a small but real doc-debt signal — fix as a phase deliverable (D-03), don't defer.
- WR-05 fold-in (D-02) is a "while you're already touching the code" optimization. Phase 9's deferred-to-Phase-10 note explicitly anticipated this; Plan 10-02 closes that loop.
- The REPAIR-02 CI gate (D-08) is essentially zero-work-today (only one Cargo.toml entry exists and it's already correct), but locks in the invariant for the v1.4+ Tor harness work which will likely add new test-only Cargo.toml entries.

</specifics>

<deferred>
## Deferred Ideas

- **Tor-mode integration harness** — Already deferred to v1.4+ per REQUIREMENTS.md "Future Requirements" + Phase 8 HUMAN-UAT item 3 (`result: deferred`). Phase 10 does NOT advance this.
- **Workspace dependency inheritance for corepc-node** — Considered for D-08 (move declaration to `[workspace.dependencies]` so future crates inherit the features). Rejected as scope creep — the CI grep check satisfies REPAIR-02 at lower cost, and the workspace-inheritance refactor would touch the existing single declaration unnecessarily. Could revisit in v1.4+ if a second crate needs corepc-node.
- **Direct reqwest + corepc-types port for tests** — Considered for D-05 (match production coordinator's RPC pattern). Rejected as scope creep — the typed client is the right level for test plumbing; production-fidelity at the wire-format level is not what these tests are verifying.
- **cargo-deny adoption** — Considered for D-08 enforcement mechanism. Rejected — adds cargo-deny as a CI dep (currently only cargo-audit is in the toolchain). Worth revisiting if multiple workspace-invariants emerge that all want enforcement.
- **Property-based testing via proptest** — Mentioned in D-10 as a candidate for replacing lost coverage IF a test retires. Not in scope for this phase; could become v1.4+ work if any test does retire and the BACKLOG entry calls for it.

</deferred>

---

*Phase: 10-full-round-rs-decision-execution*
*Context gathered: 2026-05-27*
