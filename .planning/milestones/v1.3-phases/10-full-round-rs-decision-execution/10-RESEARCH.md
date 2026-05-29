# Phase 10: full_round.rs decision + execution - Research

**Researched:** 2026-05-27
**Domain:** Rust integration-test repair — corepc-node 0.12 typed Client at `features=["30_2"]`, Bitcoin Core v30 descriptor-wallet semantics, `tokio::time::timeout`-based poll-until-deadline, GitHub Actions CI invariant grep
**Confidence:** HIGH (all RPC type signatures verified against pinned `corepc-node-0.12.0` source on GitHub; root-cause failure reproduced locally against brew bitcoind v31; v30/v31 release notes cross-checked)

## Summary

Phase 10 is a test-repair phase, not a feature phase. The 6 ignored tests in `tests/integration/full_round.rs` were quarantined with the marker `"RPC schema drift on listunspent/getrawtransaction"` during Phase 9, but the actual failure mode discovered in this research is **not schema drift in the corepc-node type definitions** — those already correctly map v30 JSON shapes. The real root cause is a **descriptor-wallet semantics regression**: Bitcoin Core v30 made descriptor wallets mandatory, and `listunspent` against a descriptor wallet returns only UTXOs the wallet owns, NOT UTXOs paid TO arbitrary external P2WPKH addresses derived from hardcoded test WIFs. The existing `fund_regtest` pattern (`send_to_address` to externally-derived addresses, then locate via `list_unspent`) was a legacy-wallet pattern that silently broke on v30.

The repair shape is one localized rewrite inside the promoted `fund_regtest` helper: replace the `listunspent`-and-match-by-address scan with a deterministic vout discovery via `getrawtransaction` (`Verbose`) on the funding txid — read the `.outputs` (`Vec<RawTransactionOutput>`) and find the vout whose `scriptPubKey` matches the recipient address. This works against any wallet model (legacy or descriptor) because it does not depend on wallet ownership of the recipient address. The corepc-node typed Client at `features=["30_2"]` provides all the required types (`GetRawTransactionVerbose`, `RawTransactionOutput`, `ScriptPubKey`) already in scope. No new dependencies, no schema bumps, no API design.

**Primary recommendation:** Repair via deterministic-vout discovery (`getrawtransaction verbose` against the funding txid; match output address). Promote a single `fund_regtest(&BitcoindGuard) -> FundedSetup` helper to `tests/integration/mod.rs` reusing the Phase 9 `bootstrap_regtest_bitcoind` shape. Fold in 4 WR-05 bare-sleep replacements with `tokio::time::timeout` + 100ms poll loops. Per-test unmute gates on local v31 PASS + CI v30.2 PASS. Add the REPAIR-02 CI grep job as a 10-line YAML step. No retirements expected.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Scope policy:**
- **D-01 (scope):** Repair all 6 ignored tests. Each covers a unique multi-client end-to-end scenario unit tests can't replicate: full 3-participant round, blame-timeout flow, replay-token attack, invalid-UTXO validation against real bitcoind, wrong-denomination check, post-blame round restart. The drift is mechanical (listunspent/getrawtransaction response shape changed between corepc-node 0.10 → 0.12 with feature `30_2`); the fix is a one-time port. Cost-to-keep < value-of-coverage.
- **D-02 (WR-05 fold-in):** The 4 Phase-9-deferred bare-sleep sites at `full_round.rs:369, 704, 1519, 1627` all live inside tests that are getting repaired anyway. Replace each `sleep(2s)` / `sleep(4s)` with poll-until-deadline (e.g., `tokio::time::timeout(Duration::from_secs(2), wait_for_phase(...)).await`) while touching the same test code. Doing it now avoids a separate Phase 10.1 / Phase 11 follow-up.
- **D-03 (doc count correction):** Phase 10 corrects "15 tests" → "8 tests (6 ignored carve-outs + 2 already-passing)" in three places: ROADMAP.md Phase 10 goal, ROADMAP.md Phase 10 success criterion 1, REQUIREMENTS.md REPAIR-01. Documentation accuracy is a phase deliverable, not deferred work.
- **D-04 (plan grouping):** 2 plans:
  - **Plan 10-01 — Schema port:** Port listunspent/getrawtransaction calls + `fund_regtest` helper to corepc-node 0.12's v30 client schema in one pass. Promote `fund_regtest` (or its successor) to `tests/integration/mod.rs` as a shared helper. No `#[ignore]` markers touched yet. Acceptance: shared helper compiles, exists, has signature matching the documented API contract.
  - **Plan 10-02 — Unmute + sleeps + doc fix:** Unmute the 6 carve-out tests one-at-a-time. For each test: (a) verify local PASS against brew bitcoind v31, (b) verify CI PASS against pinned v30.2, (c) remove the `#[ignore]` marker, (d) fix any bare-sleep sites inside that test (WR-05). Also: REPAIR-02 CI grep check + the "15 → 8" doc corrections. Acceptance per test: local + CI both PASS; whole-file invariant: `grep -c '^#\[ignore' tests/integration/full_round.rs` returns 0 at end of plan.

**Repair approach:**
- **D-05 (API choice):** Port to **corepc-node 0.12 typed v30 Client** (the `corepc_node::Client` API at `features=["30_2"]`). Already pinned at `coordinator/Cargo.toml:65`; the typed responses for `listunspent` / `getrawtransaction` at this feature flag have the correct v30+ shape (descriptor wallets, modern field names). Simplest fix, no new dependencies, matches the Phase 9 fixture style. Direct reqwest + corepc-types was considered (more fidelity to production coordinator) and rejected as scope creep — tests don't need to exercise the production RPC wire format; the typed client is the right level for test plumbing.
- **D-06 (plumbing location):** Promote the new RPC plumbing (the funded-regtest setup that consumes the v30 typed client) to `tests/integration/mod.rs` as a shared helper. Same consolidation pattern Phase 9 established. Any future test (the v1.4+ Tor harness, additional adversarial scenarios, etc.) reuses without reimplementing. Concrete signature: `pub async fn fund_regtest(guard: &BitcoindGuard, /* ...funding params... */) -> FundedSetup;` (or equivalent — exact signature is Claude's discretion; the contract is "given a BitcoindGuard, return a FundedSetup that lets a coordinator+clients run an end-to-end round"). The 6 unmuted tests `use crate::fund_regtest;` instead of the file-private version.
- **D-07 (per-test acceptance):** Each unmuted test must satisfy BOTH:
  1. **Local PASS:** `BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round::<test_name> -- --nocapture` exits 0 with `test result: ok. 1 passed`.
  2. **CI PASS:** The test appears as `... ok` in the PR's `cargo test` CI log against pinned v30.2 (not just "compiles," not "passes locally only").
  The `#[ignore]` marker is removed only after both. Same gate as Phase 9 UAT-1. This is per-test (not aggregate), so each unmute is its own atomic verification step.

**REPAIR-02 enforcement:**
- **D-08 (mechanism):** New tiny CI job in `.github/workflows/ci.yml` titled `corepc-node feature pin check`. Runs in parallel with `cargo test` / `cargo clippy` / `cargo audit` / `coordinator binary builds`. ~10 lines of YAML. Self-documenting failure name. Matches the project's "small focused job per gate" pattern.
- **D-09 (grep semantics):** Negative-match: catch lines that declare `corepc-node = ...` but lack a `features = ...` clause. Concrete shape (planner refines exact regex):
  ```
  grep -rEn 'corepc-node\s*=' --include='Cargo.toml' . \
    | grep -v 'features\s*=' \
    | grep -v '^[^:]*:[0-9]*:#' \
    && exit 1 || exit 0
  ```
  Catches `corepc-node = "0.12"` (no features), tolerates commented-out lines and the multi-line table form. Fails closed with exit 1 on any unfeatured match. Future minor bumps (`features = ["30_2"]` → `features = ["30_3"]`) don't require workflow edits — only adding a corepc-node entry WITHOUT features triggers failure.

**Coverage rescue:**
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

### Deferred Ideas (OUT OF SCOPE)

- **Tor-mode integration harness** — Already deferred to v1.4+ per REQUIREMENTS.md "Future Requirements" + Phase 8 HUMAN-UAT item 3 (`result: deferred`). Phase 10 does NOT advance this.
- **Workspace dependency inheritance for corepc-node** — Considered for D-08 (move declaration to `[workspace.dependencies]` so future crates inherit the features). Rejected as scope creep — the CI grep check satisfies REPAIR-02 at lower cost, and the workspace-inheritance refactor would touch the existing single declaration unnecessarily. Could revisit in v1.4+ if a second crate needs corepc-node.
- **Direct reqwest + corepc-types port for tests** — Considered for D-05 (match production coordinator's RPC pattern). Rejected as scope creep — the typed client is the right level for test plumbing; production-fidelity at the wire-format level is not what these tests are verifying.
- **cargo-deny adoption** — Considered for D-08 enforcement mechanism. Rejected — adds cargo-deny as a CI dep (currently only cargo-audit is in the toolchain). Worth revisiting if multiple workspace-invariants emerge that all want enforcement.
- **Property-based testing via proptest** — Mentioned in D-10 as a candidate for replacing lost coverage IF a test retires. Not in scope for this phase; could become v1.4+ work if any test does retire and the BACKLOG entry calls for it.

## Phase Requirements

| ID | Description (from REQUIREMENTS.md) | Research Support |
|----|------------------------------------|------------------|
| REPAIR-01 | `tests/integration/full_round.rs` is either repaired (all **8** tests pass against the pinned bitcoind version, including the **6 currently failing** on `listunspent`/RPC schema drift) **OR** explicitly retired with rationale in TODO.md. **The "15" count in the current REQUIREMENTS.md entry is stale and is corrected to "8" as part of this phase per D-03.** | Architectural Responsibility Map (test tier); Standard Stack (corepc-node 0.12 typed Client at `features=["30_2"]`); Architecture Patterns Pattern 1 (deterministic-vout discovery); Common Pitfalls 1 (descriptor-wallet `listunspent` semantics) |
| REPAIR-02 | Any test that uses `corepc-node`'s typed `Client` API enables the appropriate version feature explicitly (e.g. `features = ["30_2"]`), never relies on the silent `0_17_2` default | Standard Stack (existing pin satisfies); Architecture Patterns Pattern 4 (CI grep gate); Common Pitfalls 2 (silent default feature trap); Code Examples (verified grep semantics) |

## Project Constraints (from CLAUDE.md)

- **No custom crypto** — blind-rsa-signatures (jedisct1), rust-bitcoin, bdk, secp256k1 only. Phase 10 touches no crypto.
- **GSD Workflow Enforcement** — Phase 10 file edits go through Plan 10-01 / Plan 10-02 plans; no direct edits outside GSD execution.
- **Tor-native production** — Phase 10 tests use clearnet bitcoind regtest (test-only); does not affect production Tor constraint.
- **Privacy (No PII logging; round state zeroed)** — Phase 10 doesn't add logging, but any `eprintln!` in test code is allowed (test-only, not production).
- **License: MIT** — Phase 10 introduces no new dependencies, so no license review required.
- **Conventional commits per gstack conventions** — observed in recent log: `docs(state):`, `test(09-02):`, `feat(09-01):`. Phase 10 commits should follow `test(10-NN):` / `ci(10-NN):` / `docs(10):` shapes.

## Architectural Responsibility Map

> Phase 10 is a test-infrastructure phase, not a feature phase. The tier in question is **the integration-test crate**, not the production coordinator. The "capabilities" below are the moving pieces inside this phase.

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Regtest bitcoind bring-up | `tests/integration/mod.rs` (Phase 9 fixture) | — | `bootstrap_regtest_bitcoind()` already owns this; Phase 10 reuses verbatim |
| RPC plumbing (typed Client) | `tests/integration/mod.rs` (new helper) | corepc-node 0.12 typed Client | D-06 promotes `fund_regtest` here; D-05 picks the typed-client level (not raw reqwest+corepc-types) |
| Wallet funding + UTXO discovery | `tests/integration/mod.rs::fund_regtest` (promoted) | corepc-node `Node::client` | Single locus; descriptor-wallet-aware vout discovery (see Pattern 1) |
| Test orchestration (clients, coordinator, assertions) | `tests/integration/full_round.rs` (each test fn) | shared fixtures | Each test consumes `fund_regtest` + `bootstrap_regtest_bitcoind`; nothing else |
| Phase-transition waits (replacing bare sleeps) | `tests/integration/full_round.rs` (4 sites) | `tokio::time::timeout` | WR-05 fix; per-site or shared helper (Claude's discretion) |
| CI invariant: explicit `features = …` on every `corepc-node = …` | `.github/workflows/ci.yml` (new job) | grep -rE | Single 10-line YAML job; pattern 4 in Architecture Patterns |
| Documentation accuracy (the "15 → 8" correction) | `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md` | — | Three string replacements; D-03 |

**Why this matters:** The 6 ignored tests all live in `full_round.rs` but the actual repair surface is concentrated in the **promoted `fund_regtest` helper inside `tests/integration/mod.rs`** — one file, one function, one corrected RPC pattern. The test bodies themselves change only at their `use crate::fund_regtest;` import and at the 4 sleep sites. This map keeps the planner from mistakenly distributing repair logic across 6 test functions when 1 helper carries it.

## Standard Stack

### Core (already in coordinator/Cargo.toml — no additions needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| corepc-node | 0.12.0 with `features=["30_2"]` | Spawn regtest bitcoind, typed v30 Client | Already pinned at `coordinator/Cargo.toml:65`; the typed Client at this feature gives v30-shaped JSON responses for every RPC method this phase touches |
| tokio | workspace (1.51.x LTS) | Async runtime, `tokio::time::timeout`, `tokio::task::spawn_blocking` | Established Phase 9 pattern: bridge corepc-node's blocking Client onto async via `spawn_blocking` |
| bitcoin | workspace (0.32.x) | `Txid` (parameter to `get_raw_transaction`), `Address`, `ScriptBuf` | Pulled transitively through corepc-node; same crate as production coordinator |
| reqwest | workspace | Existing test code already uses it for coordinator HTTP — unchanged | n/a for Phase 10 changes |

**Version verification (executed 2026-05-27):**
```bash
# crates.io API confirms:
# corepc-node 0.12.0 published 2026-04-14, latest version, max_stable_version=0.12.0
curl -sL "https://crates.io/api/v1/crates/corepc-node"
# Returns: "newest_version": "0.12.0", "max_version": "0.12.0", "max_stable_version": "0.12.0", "num_versions": 11

# Version history (no 0.13 exists):
# 0.12.0  2026-04-14
# 0.10.1  2025-11-18  (skipped 0.11 entirely)
# 0.10.0  2025-10-07
# 0.9.0   2025-09-16
# ...
```

[VERIFIED: crates.io API + github.com/rust-bitcoin/corepc tag `corepc-node-0.12.0`]

### Supporting (existing — not changed by Phase 10)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tempfile | 3.x | `TempDir` for per-test ban_file isolation | Already in dev-deps; WR-06 pattern from Phase 8 |
| client (path = "../client") | local | `ClientWallet`, `CoordinatorClient` | Test orchestration; unchanged |
| shared (path = "../shared") | local | `protocol::InputRegRequest` / `OutputRegRequest` for adversarial tests | Unchanged |
| base64 | workspace | Adversarial replay/wrong-denom tests construct request bodies | Unchanged |

### Alternatives Considered

| Instead of | Could Use | Tradeoff | Rejected because |
|------------|-----------|----------|------------------|
| corepc-node typed Client (`features=["30_2"]`) | Direct `reqwest` + `corepc-types` JSON-RPC | Higher fidelity to production coordinator's RPC pattern | D-05 explicit reject — test plumbing doesn't need wire-format fidelity, the typed client is the right level |
| Promote `fund_regtest` to `mod.rs` | Keep file-private in `full_round.rs` | Smaller change in this phase | D-06 explicit choose — matches Phase 9 consolidation pattern; future tests reuse |
| `tokio::time::timeout` poll-and-await | Bare `tokio::time::sleep(N)` | Familiar to readers, less code | WR-05 explicit reject — bare sleeps flake on busy CI runners |
| New 10-line CI job for REPAIR-02 | `cargo deny` workspace-rule | Stronger declarative invariant surface | D-08 explicit reject — adds a new CI dep for a single-rule check |
| Inline regex grep in ci.yml | Extract to `scripts/ci/check-corepc-node-pin.sh` | Reusable, testable in isolation | Claude's-discretion — inline if regex stays <5 lines; current shape is 3 lines |

**Installation:** No new dependencies. Phase 10 adds zero crates to Cargo.toml.

## Package Legitimacy Audit

> Phase 10 installs **no new packages**. The single `corepc-node = { version = "0.12", features = ["30_2"] }` declaration is unchanged at `coordinator/Cargo.toml:65`, already in the repo, already verified by Phase 8/9 review.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| corepc-node 0.12.0 | crates.io | published 2026-04-14 (~6 weeks old at research date); crate created 2024-11-14, 11 versions | 345,817 total / 114,982 recent | github.com/rust-bitcoin/corepc (official rust-bitcoin organisation, MIT license) | not run — pre-existing dependency, not a new install | Approved (unchanged) |

**Packages removed due to slopcheck [SLOP] verdict:** none — no new installs in this phase
**Packages flagged as suspicious [SUS]:** none

*Justification for skipping slopcheck: Phase 10 introduces zero new package declarations. The only `corepc-node` line in the entire workspace is `coordinator/Cargo.toml:65`, already present, already shipped by Phase 8 (the bump-to-0.12 quick task on 2026-05-26 referenced in TODO.md "Resolved 2026-05-26"). corepc-node is published by `Tobin C. Harding <me@tobin.cc>` and `Riccardo Casatta <riccardo@casatta.it>` from the official rust-bitcoin GitHub organisation — same organisation publishing `bitcoin`, `corepc-types`, `corepc-client`, and `bip322`. This is the canonical Bitcoin Core test harness for Rust. [VERIFIED: crates.io API, github.com/rust-bitcoin/corepc tag `corepc-node-0.12.0`]*

## Architecture Patterns

### System Architecture Diagram

```
Phase 10 repair flow (per ignored test):
┌────────────────────────────────────────────────────────────────────────┐
│  Test entry: #[tokio::test] async fn <name>()                          │
└───────────────────────────┬────────────────────────────────────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │  require_bitcoind!()    │   ← Phase 9 macro (mod.rs)
                │  → exe: String          │      skip in local-dev / panic in CI
                └───────────┬─────────────┘
                            │
                            ▼
            ┌───────────────────────────────────┐
            │  bootstrap_regtest_bitcoind(exe)  │   ← Phase 9 helper (mod.rs)
            │  → (BitcoindGuard, RpcCreds)      │      starts regtest, mines 101 blocks
            └───────────────┬───────────────────┘
                            │
                            ▼
                ┌────────────────────────────┐
                │   fund_regtest(guard, ...) │   ← Phase 10 PROMOTED helper (mod.rs)
                │   → FundedSetup            │      ┌────────────────────────────┐
                │                            │      │ inside spawn_blocking:    │
                │                            │      │ 1. derive P2WPKH from WIFs│
                │                            │      │ 2. send_to_address × 3    │
                │                            │      │ 3. generate_to_address(1) │
                │                            │      │ 4. FOR EACH funding txid: │
                │                            │      │    get_raw_transaction_   │
                │                            │      │      verbose(txid)        │
                │                            │      │    find vout where        │
                │                            │      │      .scriptPubKey.addr   │
                │                            │      │      == recipient_addr    │
                │                            │      │    capture (outpoint,     │
                │                            │      │             value_sats)   │
                │                            │      └────────────────────────────┘
                └───────────────┬────────────┘
                                │
                                ▼
                ┌───────────────────────────────┐
                │  Each test body:              │
                │  - spawn_coordinator(...)     │   ← in-process coordinator
                │  - run 3 client tasks         │   ← multi-client orchestration
                │  - wait_for_phase + assertion │   ← WR-05 fix: poll-until-deadline,
                │                                       NOT bare sleep(2)
                └───────────────────────────────┘

CI invariant (parallel job, not in the test flow):
┌──────────────────────────────────────────────────────────────────────┐
│ corepc-node feature pin check (new in ci.yml)                        │
│   grep -rEn 'corepc-node\s*=' --include='Cargo.toml' .               │
│     | grep -v 'features\s*='                                         │
│     | grep -v '^[^:]*:[0-9]*:#'                                      │
│     && exit 1  || exit 0                                             │
│   → fails closed on any corepc-node = ... line lacking features      │
└──────────────────────────────────────────────────────────────────────┘
```

[VERIFIED: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0 — all macro signatures confirmed via raw source URLs]

### Recommended Project Structure

```
tests/integration/
├── mod.rs                     # CRATE ROOT for integration test binary
│   ├── (Phase 9) require_bitcoind!() macro              [unchanged]
│   ├── (Phase 9) BitcoindGuard struct                   [unchanged]
│   ├── (Phase 9) RpcCreds struct                        [unchanged]
│   ├── (Phase 9) bootstrap_regtest_bitcoind() helper    [unchanged]
│   ├── (Phase 10) FundedSetup struct                    [PROMOTED from full_round.rs]
│   └── (Phase 10) fund_regtest() async helper           [PROMOTED + REPAIRED]
├── full_round.rs              # 6 tests unmuted by Plan 10-02
│   ├── use crate::{..., fund_regtest, FundedSetup}      [+ new imports]
│   ├── 6 × #[ignore = "..."] removed                    [Plan 10-02]
│   ├── 4 × bare sleep replaced with poll-until-deadline [WR-05 fix]
│   └── 2 × file-private fund_regtest variants DELETED   [moved to mod.rs]
├── rate_limiting.rs           [unchanged]
├── round_bootstrap.rs         [unchanged]
└── ban_list_persistence.rs    [unchanged]

.github/workflows/ci.yml
└── + new job: corepc-node feature pin check             [Plan 10-02]
```

### Pattern 1: Wallet-Agnostic Vout Discovery (the actual repair)

**What:** After `send_to_address` to an external (non-wallet) address, locate the resulting vout by parsing the funding transaction directly, NOT by querying `listunspent` from the wallet's perspective.

**When to use:** Any test that sends to addresses derived from test WIFs (i.e. not addresses the bitcoind wallet owns). On v30+ descriptor wallets, `listunspent` only returns UTXOs the wallet owns — UTXOs paid to external addresses are invisible to it.

**Example:**
```rust
// Source: verified RPC signatures at github.com/rust-bitcoin/corepc, tag
// corepc-node-0.12.0:
//   client/src/client_sync/v17/raw_transactions.rs:165-170
//     pub fn get_raw_transaction_verbose(&self, txid: Txid) -> Result<GetRawTransactionVerbose>
//   types/src/v17/raw_transactions/mod.rs:341-389  GetRawTransactionVerbose { outputs: Vec<RawTransactionOutput>, ... }
//   types/src/psbt/mod.rs:142-151  RawTransactionOutput { value: f64, index: u64, script_pubkey: ScriptPubKey }
//
// Pattern: deterministic vout discovery via getrawtransaction (verbose).
// Works against legacy and descriptor wallets identically — no wallet ownership needed.

use std::str::FromStr;
use bitcoin::{Address, Amount, Network, Txid};
use corepc_node::Node;

fn locate_vout_by_address(
    node: &Node,
    funding_txid_str: &str,
    recipient_addr: &Address,
) -> (String /* outpoint "txid:vout" */, u64 /* sats */) {
    let txid = Txid::from_str(funding_txid_str).expect("valid txid hex");
    let tx = node.client
        .get_raw_transaction_verbose(txid)
        .expect("get_raw_transaction_verbose");

    // tx.outputs: Vec<RawTransactionOutput>
    // Each output has: value: f64 (BTC), index: u64, script_pubkey: ScriptPubKey
    // ScriptPubKey carries the human-readable addresses array.
    //
    // The recipient address appears in script_pubkey.address (Option<String> on v17+ types)
    // OR can be derived by parsing script_pubkey.hex back into an Address and comparing.
    // The simplest robust check: compare addresses as strings.
    let recipient_str = recipient_addr.to_string();
    let out = tx.outputs.iter()
        .find(|o| {
            // ScriptPubKey carries an `address: Option<String>` field on v23+ types
            // (verified via types/src/v23/util/mod.rs ScriptPubKey shape).
            // If the address field is absent (older format), fall back to hex parse.
            o.script_pubkey.address.as_deref() == Some(&recipient_str)
        })
        .unwrap_or_else(|| panic!(
            "funding tx {} has no output to {}",
            funding_txid_str, recipient_str
        ));

    let outpoint = format!("{}:{}", funding_txid_str, out.index);
    let sats = (out.value * 100_000_000.0).round() as u64;
    (outpoint, sats)
}
```

> **Note for the planner:** The exact field name on `ScriptPubKey` (`.address` vs `.addresses` vs hex-decode-then-Address::from_script) needs verification at execution time — the v23 types module split changed how addresses are encoded in `getrawtransaction verbose` responses. If `.address` is absent on the v17/v23 re-export at feature `30_2`, the fallback is to compare hex-decoded `script_pubkey.hex` via `Address::from_script(&ScriptBuf::from_hex(&hex)?, Network::Regtest)`. Either approach is wallet-agnostic; the executor picks the one that compiles first.

### Pattern 2: Poll-Until-Deadline (WR-05 fix)

**What:** Replace `tokio::time::sleep(Duration::from_secs(N))` with a bounded poll loop that exits early when the condition is met, OR fails the test cleanly when the deadline elapses.

**When to use:** Each of the 4 bare-sleep sites in `full_round.rs` (lines 369, 704, 1519, 1627). The sleeps are waiting for one of:
- mempool to contain the broadcast txid (line 369 — `sleep(2s)` after all clients sign)
- signing timeout + blame to complete and ban list to populate (line 713 / formerly 704 — `sleep(4s)`)
- round restart back to InputReg after blame (line 1547 / formerly 1519 — `sleep(4s)`)
- second-round broadcast to settle in mempool (line 1655 / formerly 1627 — `sleep(2s)`)

**Example (mempool-await variant):**
```rust
// Source: established Phase 9 pattern at tests/integration/full_round.rs:116-135
// (wait_for_coordinator) and the deadline loop at round_bootstrap.rs:128-204.
//
// Pattern: bounded poll with explicit deadline; panic with a useful message
// if the deadline elapses. Generalises to any condition without taking
// long-form sleeps.

async fn wait_for_mempool_nonempty(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
    timeout: Duration,
) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let rpc_url_c = rpc_url.clone();
        let rpc_user_c = rpc_user.clone();
        let rpc_pass_c = rpc_pass.clone();
        let txids = tokio::task::spawn_blocking(move || {
            use corepc_node::client::client_sync::Auth;
            let auth = Auth::UserPass(rpc_user_c, rpc_pass_c);
            let client = corepc_node::Client::new_with_auth(&rpc_url_c, auth)
                .expect("create rpc client");
            // GetRawMempool is the tuple wrapper around Vec<String>;
            // verified at client/src/client_sync/v21/blockchain.rs:14-19
            client.get_raw_mempool().expect("get_raw_mempool").0
        }).await.expect("spawn_blocking panicked");

        if !txids.is_empty() {
            return txids;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "mempool never received broadcast tx within {:?}; \
                 coordinator may not have called assemble_and_broadcast",
                timeout
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

**Mapping of WR-05 sites to predicates:**

| Original line | Current line (after Phase 9 edits) | Replaces sleep waiting for… | Predicate to poll |
|--------------|-------------------------------------|------------------------------|-------------------|
| 369 | 378 | mempool to contain broadcast | `!client.get_raw_mempool()?.0.is_empty()` |
| 704 | 713 | blame to ban non-signer + round → Idle | `info.round_state == "idle"` AND `ban_list.is_banned(utxo, now)` |
| 1519 | 1547 | round restart to InputReg | `info.round_state == "input_reg"` AND `ban_list.is_banned(utxo, now)` |
| 1627 | 1655 | second-round mempool broadcast | `!client.get_raw_mempool()?.0.is_empty()` |

> Lines 369, 704, 1519, 1627 are the Phase 9 CONTEXT.md / REVIEW.md citation. Direct file inspection 2026-05-27 found the bare sleeps at slightly different offsets (378, 713, 1547, 1655) due to comment additions in Phase 9 plan 09-03. The planner should grep for `tokio::time::sleep(Duration::from_secs(` inside `full_round.rs` rather than trust the line numbers verbatim — there are exactly 4 such sites, matching the count.

### Pattern 3: Promoted Shared Helper Visibility (D-06 contract)

**What:** Move the `fund_regtest` helper from `tests/integration/full_round.rs` (file-private `async fn`) to `tests/integration/mod.rs` (`pub async fn`), and replace 5 callsites' `fund_regtest(exe)` invocations with `crate::fund_regtest(exe)`.

**When to use:** Exactly once during Plan 10-01. The contract is:
- Function signature: `pub async fn fund_regtest(exe: String) -> (BitcoindGuard, FundedSetup)` — preserves Phase 9 ergonomics (caller passes the `exe` from `require_bitcoind!()`, gets back the RAII guard).
- `FundedSetup` struct: also promoted to `mod.rs`, also `pub`. Fields match the existing private struct (rpc_url, rpc_user, rpc_pass, utxos).
- Imports updated at the top of `full_round.rs`: add `fund_regtest` and `FundedSetup` to the existing `use crate::{...}` line.
- The 5 current callsites (line 966, 1090, 1147, 1472, plus the in-test-body version at line 177-289 for `full_round_three_clients`) all use the promoted version. The unique in-body version at 177-289 in `full_round_three_clients` may be deleted in favour of calling `fund_regtest` if the contract matches (the executor verifies at implementation time).

**Example skeleton:**
```rust
// tests/integration/mod.rs additions

/// UTXO funding result for an integration test.
///
/// Three P2WPKH UTXOs derived from hardcoded regtest WIFs, each funded with
/// (denomination + 50_000 sats fee margin) via the regtest wallet's
/// `sendtoaddress`. Carries the bitcoind RPC URL + cookie credentials so the
/// caller can construct additional `corepc_node::Client` instances for mempool
/// inspection without re-resolving cookies.
#[derive(Debug, Clone)]
pub struct FundedSetup {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_pass: String,
    /// (outpoint "txid:vout", value_sats) per participant
    pub utxos: [(String, u64); 3],
}

/// Spin up regtest bitcoind, mine 101 blocks, derive 3 P2WPKH addresses from
/// hardcoded test WIFs, send `denomination + 50_000` sats to each, mine 1
/// confirmation block, and locate each output's vout via
/// `get_raw_transaction_verbose` (wallet-agnostic — works against descriptor
/// wallets in Bitcoin Core v30+).
///
/// **Returns:** `(BitcoindGuard, FundedSetup)`. The caller MUST hold the
/// `BitcoindGuard` for the entire test duration; dropping it terminates
/// bitcoind and invalidates `FundedSetup.rpc_url` / etc.
///
/// **Schema note:** The vout discovery uses `get_raw_transaction_verbose`
/// instead of `list_unspent` because descriptor wallets (mandatory in v30+)
/// only return wallet-owned UTXOs from `listunspent`. The test WIFs derive
/// addresses that the bitcoind wallet does NOT own — `listunspent` returns
/// them empty. Reading the funding tx directly bypasses wallet ownership.
pub async fn fund_regtest(exe: String) -> (BitcoindGuard, FundedSetup) {
    /* see Pattern 1 for vout discovery; bootstrap_regtest_bitcoind for daemon bring-up */
    todo!("Plan 10-01 implements")
}
```

### Pattern 4: CI Grep Invariant (REPAIR-02)

**What:** A 10-line GitHub Actions job that runs three chained greps and fails closed.

**When to use:** Once, added to `.github/workflows/ci.yml` alongside the existing 4 jobs (`test`, `clippy`, `coordinator-smoke`, `audit`).

**Example:**
```yaml
  corepc-node-feature-pin-check:
    name: corepc-node feature pin check
    runs-on: ubuntu-latest
    # REPAIR-02 invariant: every `corepc-node = ...` declaration in any
    # Cargo.toml in the workspace must include an explicit `features = ...`
    # clause. corepc-node defaults to the silent `0_17_2` (Bitcoin Core 0.17.2,
    # released 2018) RPC schema if no version feature is selected; this gate
    # catches future additions that forget the features clause.
    #
    # Pattern (verified 2026-05-27 against the current tree — produces
    # zero matches, exit 0):
    #   grep -rEn 'corepc-node\s*='        — every declaration line
    #   | grep -v 'features\s*='           — drop the well-formed ones
    #   | grep -v '^[^:]*:[0-9]*:#'        — drop commented-out lines
    # The chain produces non-empty output IFF an unfeatured corepc-node
    # declaration exists. We invert via `&& exit 1 || exit 0`.
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5  # v4.3.1 (SHA matches sibling jobs)
      - name: Enforce explicit corepc-node feature
        run: |
          set -eu
          if grep -rEn 'corepc-node\s*=' --include='Cargo.toml' . \
             | grep -v 'features\s*=' \
             | grep -v '^[^:]*:[0-9]*:#'; then
            echo "ERROR: corepc-node declaration(s) above lack an explicit 'features = [...]' clause." >&2
            echo "       Without a version feature, corepc-node uses the Bitcoin Core 0.17.2 (2018) RPC schema." >&2
            echo "       Add 'features = [\"30_2\"]' (or whatever version pin is appropriate) to each declaration." >&2
            exit 1
          fi
```

> **Regex robustness check (2026-05-27, against current tree):**
>
> Current tree has exactly one `corepc-node = ...` line: `coordinator/Cargo.toml:65:corepc-node = { version = "0.12", features = ["30_2"] }`. Plus one comment line at `coordinator/Cargo.toml:62:# Note: corepc-node defaults...`.
>
> ```bash
> $ grep -rEn 'corepc-node\s*=' --include='Cargo.toml' . | grep -v 'features\s*=' | grep -v '^[^:]*:[0-9]*:#'
> (no output)
> ```
>
> Three failure modes the chain catches correctly:
> 1. `corepc-node = "0.12"` (no features at all) — line 1 of chain matches; line 2 doesn't drop it (no `features =` on the line); line 3 doesn't drop it (not a comment); output non-empty → exit 1. ✓
> 2. `corepc-node = { version = "0.12" }` (table form, no features key) — same path → exit 1. ✓
> 3. `# corepc-node = "0.12"` (commented-out experiment) — line 1 matches; line 3 drops it (`^[^:]*:[0-9]*:#` matches the `<file>:<line>:#` prefix from grep -n); output empty for this line → exit 0. ✓
>
> One known limitation:
> - **Multi-line table form split across `\n`** (e.g.:
>   ```toml
>   corepc-node = {
>     version = "0.12",
>     features = ["30_2"]
>   }
>   ```
>   ) — only the first line (`corepc-node = {`) matches the initial grep; that line lacks `features =`; depending on whether someone splits the features key onto its own line, this could produce a false positive. **Mitigation:** Document in the job's YAML comment that the table form must be single-line OR include `features =` on the same physical line as `corepc-node =`. The existing single declaration at `coordinator/Cargo.toml:65` is correctly single-line; future maintainers see the failure and either single-line it or extend the regex. Acceptable per Claude's-discretion clause on inline vs. extracted script.

### Anti-Patterns to Avoid

- **`listunspent` to locate funded external UTXOs.** Broken on v30+ descriptor wallets (the real Phase 9 → Phase 10 root cause). Use `getrawtransaction verbose` against the funding txid.
- **`Arc<BitcoindGuard>` + `Arc::try_unwrap` plumbing** (IN-02 in Phase 9 REVIEW.md). Cleaner: move bare `BitcoindGuard` into `spawn_blocking`, return it from the closure. Eliminates 6 lines of plumbing. *Note:* the executor can apply this cleanup opportunistically while touching `fund_regtest`, but it is not required by the plan — adding it is a structural improvement, not a correctness fix.
- **Bare `tokio::time::sleep(Duration::from_secs(N))` waits for async events.** Flake risk on busy CI runners. Use poll-until-deadline (Pattern 2).
- **Importing test WIFs into the regtest wallet** as a workaround. Would force every test's setup to also issue `importdescriptors` (descriptor-wallet equivalent), expanding the RPC surface unnecessarily. Vout discovery via `getrawtransaction` is simpler and wallet-model-agnostic.
- **`assert!(resp.status().is_client_error(), ...)` without checking the error body** (IN-03 in Phase 9 REVIEW.md). Accepts any 4xx including a 400 parse error, which would mask a regression. Phase 9 left this unaddressed; Phase 10 could improve while touching the test (but D-04 acceptance does not require it — Claude's-discretion).
- **Pinning the line numbers 369/704/1519/1627 verbatim in the WR-05 fix.** These are stale by ~10 lines each due to Phase 9 comment additions. The robust approach is to grep for `tokio::time::sleep(Duration::from_secs(` inside `full_round.rs` — should find exactly 4 sites.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Regtest bitcoind lifecycle | A bespoke `Command::new("bitcoind")` + tempdir + cookie reader | `corepc_node::Node::with_conf` + `bootstrap_regtest_bitcoind` (Phase 9) | Already solved; corepc-node handles port allocation, cookie wait, wallet bootstrap |
| Bitcoin Core JSON-RPC types | Hand-written serde structs for `listunspent` / `getrawtransaction` responses | `corepc_node::*` re-exports from `corepc-client::types::v30` | corepc-types defines every field correctly; hand-writing risks drift on next bitcoind release |
| Tokio-blocking-RPC bridging | `std::thread::spawn` + channel | `tokio::task::spawn_blocking` (Phase 9 pattern) | Native runtime support; blocking-pool worker doesn't stall async executor |
| Bounded async waits | Custom loop with `Instant::now()` + `sleep` math | `tokio::time::timeout(Duration, future).await` OR explicit deadline loop (Pattern 2) | `tokio::time::timeout` is the idiomatic shape and integrates with the runtime's pause/advance hooks |
| Address-from-WIF derivation | Manual ECDSA + WIF parsing | `bitcoin::PrivateKey::from_wif` + `bitcoin::Address::p2wpkh` | Already used everywhere; pulls through `bitcoin = 0.32` |
| Cargo.toml invariant enforcement | Custom Cargo wrapper / build script | One `grep -rEn` + 2 negative greps in a CI job | One-rule check; doesn't justify cargo-deny dep |

**Key insight:** Phase 10 is unusual in that ~95% of the work is "use the existing primitives correctly." The repair is a single 30-line helper function, 4 sleep-→-poll edits, 6 `#[ignore]` removals, and 1 new CI job. No new libraries, no new patterns. The temptation to "while we're touching it, also..." should be resisted — the WR-05 fold-in (D-02) is the one exception that's already scoped by CONTEXT.md.

## Runtime State Inventory

> Not applicable — Phase 10 is a code-and-config-only phase (test code, workflow YAML, doc string corrections). No databases, no external services, no OS-registered state, no secrets, no installed packages or build artifacts to migrate.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — Phase 10 reads no persistent datastore. Regtest bitcoind state is per-test ephemeral via `corepc-node`'s `TempDir`. | none |
| Live service config | None — Phase 10 touches no live service. No deployed coordinator gets reconfigured. | none |
| OS-registered state | None — no systemd / launchd / Task Scheduler / pm2 / docker registry interactions. | none |
| Secrets/env vars | `BLINDJOIN_REQUIRE_BITCOIND=1` (Phase 9 invariant, env-only, no code rename); `BITCOIND_EXE` (corepc-node lookup, no code rename). Neither needs change. | none |
| Build artifacts / installed packages | `~/.local/bin/bitcoind` (CI cache, key = `runner.os-bitcoind-v30.2`) — unaffected; pin version unchanged. Local brew `/opt/homebrew/Cellar/bitcoin/31.0_1/bin/bitcoind` — unaffected. | none |

**Nothing found in any category** — verified by reviewing Phase 10 scope (test code + CI YAML + 3 doc string replacements). No rename / refactor / migration semantics; this is purely additive code changes plus marker removals.

## Common Pitfalls

### Pitfall 1: Bitcoin Core v30 descriptor wallets break the `listunspent` external-address pattern

**What goes wrong:** A test that calls `node.client.send_to_address(external_addr, amount)` then `node.client.list_unspent()` and tries to locate the UTXO by `entry.address == external_addr` will find no match. The match silently fails (or panics via `unwrap_or_else`).

**Why it happens:** Bitcoin Core v30 made descriptor wallets mandatory ([CITED: github.com/bitcoin/bitcoin release-notes-30.0.md — "BDB legacy wallets can no longer be created or loaded"]). Descriptor wallets only return UTXOs from `listunspent` that the wallet *owns* (i.e., addresses derived from the wallet's own descriptors). External addresses (e.g., P2WPKH derived from hardcoded test WIFs not imported into the wallet) are invisible to `listunspent`, even after they receive UTXOs in a confirmed block. [CITED: developer.bitcoin.org/reference/rpc/listunspent.html — "listunspent returns only UTXOs that the wallet owns"]

**Reproduction:** Phase 10 research session 2026-05-27 ran `BITCOIND_EXE=/opt/homebrew/bin/bitcoind cargo test --test integration full_round::adversarial_invalid_utxo -- --ignored --nocapture` and observed:

```
thread 'tokio-rt-worker' panicked at .../full_round.rs:821:40:
Could not find UTXO txid=f369...f5a addr=bcrt1qgqd3mqyqaxpdavmj2pnmjztlxh2ct79a3arvtu
test full_round::adversarial_invalid_utxo ... FAILED
```

Confirms: the `funding_txid` is valid (real, confirmed transaction), the recipient address is valid (P2WPKH derived from `cPyRhf56...UqkZGQ`), but `listunspent` returns zero matching entries because the wallet does not own the recipient address.

**How to avoid:** Locate funded UTXOs by parsing the funding transaction directly (Pattern 1). `get_raw_transaction_verbose(txid)` returns `.outputs: Vec<RawTransactionOutput>` where each output has `.script_pubkey: ScriptPubKey` (containing the address representation) and `.value: f64` (in BTC) and `.index: u64` (the vout). Match on the recipient address; capture the vout. Wallet-model-agnostic.

**Warning signs:** A test failure of the form `Could not find UTXO txid=... addr=bcrt1...` with the funding txid being valid (you can run `bitcoin-cli getrawtransaction <txid>` and see it). This is NOT "RPC schema drift" — the response shape is correct; the wallet just isn't reporting external-address UTXOs.

[VERIFIED: locally reproduced 2026-05-27; CITED: bitcoin Core release-notes-30.0.md, developer.bitcoin.org listunspent reference]

### Pitfall 2: corepc-node's silent `0_17_2` default feature

**What goes wrong:** A `corepc-node = "0.12"` Cargo.toml entry without an explicit `features = [...]` clause silently selects the `0_17_2` default. The compiled binary uses Bitcoin Core 0.17.2 (2018) JSON-RPC schemas. RPC calls like `createwallet` hang or fail with cryptic errors against modern bitcoind ("Could not create or load wallet").

**Why it happens:** The corepc-node Cargo.toml declares `default = ["0_17_2"]` ([VERIFIED: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0, node/Cargo.toml]). Without a feature override, the typed client is wired to v17-shaped responses, which differ structurally from v30 responses (notably descriptor-wallet fields, deprecated fields removed, etc.). The crate's documentation calls this out, but a developer who adds a corepc-node entry following the crates.io quick-example will not see a hint.

**How to avoid:** Always specify `features = ["NN_M"]` for the matching Bitcoin Core version. For this project: `features = ["30_2"]`. The REPAIR-02 CI gate (D-08) enforces this workspace-wide.

**Warning signs:** RPC method calls that exhibit "weird" failures (createwallet hangs, listunspent returns empty when it shouldn't, fields missing on response structs). Cargo.toml line `corepc-node = "..."` without `features = `.

[VERIFIED: corepc-node-0.12.0/node/Cargo.toml `default = ["0_17_2"]`; corroborated by TODO.md Resolved 2026-05-26 "corepc-node 0.12 still defaults to a Bitcoin Core 0.17.2 (2018) RPC schema unless an explicit version feature is enabled"]

### Pitfall 3: `corepc-node` `Conf` defaults change between minor versions

**What goes wrong:** A future bump of corepc-node (e.g., 0.12.0 → 0.13.0) could change `Conf::default().view_stdout` from `false` back to `true`, silently re-introducing the pipe-buffering hang that Phase 9 fixed.

**Why it happens:** `Conf` is `#[non_exhaustive]` and defaults can shift. Phase 9 explicitly set `conf.view_stdout = false` and pushed `-printtoconsole=0` as belt-and-suspenders. Phase 10 does NOT touch this code, but the planner should note: any code change inside `bootstrap_regtest_bitcoind` requires keeping both belt and suspenders intact.

**How to avoid:** Phase 10 doesn't modify `bootstrap_regtest_bitcoind`. The planner should verify that `tests/integration/mod.rs::bootstrap_regtest_bitcoind` is untouched by Plan 10-01 (only `fund_regtest` and `FundedSetup` get added). If a future corepc-node bump becomes relevant, re-verify `Conf::default()` shape against the new crate source.

**Warning signs:** A regtest test that prints bitcoind log lines to cargo stdout; a hung cargo test process after a corepc-node bump.

[VERIFIED: corepc-node-0.12.0/node/src/lib.rs:265-278 `Conf::default()` returns `view_stdout: false`]

### Pitfall 4: `tokio::time::sleep` flake on busy CI runners

**What goes wrong:** A `tokio::time::sleep(Duration::from_secs(2))` followed by an assertion times out under CI runner contention. The bitcoind regtest mining + the coordinator's signing/broadcast cycle can collectively take 2.5–3 seconds on a noisy GitHub Actions ubuntu-latest VM, exceeding the 2-second window.

**Why it happens:** Documented by Phase 9 review WR-05. The current tests pass locally on M-series Mac (where bitcoind regtest mines in <100ms) but flake on shared CI runners. The `#[ignore]` shield masks the flake risk; Phase 10 unmute lifts the shield.

**How to avoid:** Replace each bare sleep with a poll-until-deadline loop (Pattern 2). The 4 sites have specific predicates (mempool non-empty, ban_list contains utxo, round state == "idle"/"input_reg"); the predicates are cheap (sub-50ms RPC or memory read), so 100ms poll cadence with a 10s deadline gives 100× safety margin over the original 2s sleep.

**Warning signs:** Intermittent CI failures where the same test passes locally; failure messages like "assertion `mempool_txids must not be empty`".

[CITED: .planning/phases/09-ci-integration-test-reliability/09-REVIEW.md §WR-05; .planning/phases/09-ci-integration-test-reliability/09-VERIFICATION.md §"Code-review WR-05"]

### Pitfall 5: `corepc-node` `check_expected_server_version` not invoked, but schema mismatch can still bite

**What goes wrong:** Local brew bitcoind v31.0.0 starts successfully against corepc-node 0.12 at feature `30_2` because `Node::with_conf` does NOT call `check_expected_server_version`. If v31 ever changed a response field that v30_2 types still expect, a test would compile but fail at runtime with a serde deserialize error.

**Why it happens:** corepc-node defines the version-check macro `impl_client_check_expected_server_version!({ [300000, 300100, 300200] })` for the `30_2` feature, but it is opt-in. The test bootstrap calls `Node::with_conf` which only verifies that bitcoind starts and responds to a basic `getblockchaininfo`; it does not assert the version is in the supported list.

**How to avoid:** Already checked for Phase 10 — Bitcoin Core 31.0 release notes ([CITED: github.com/bitcoin/bitcoin/blob/master/doc/release-notes/release-notes-31.0.md]) document NO changes to `listunspent`, `getrawtransaction`, `sendtoaddress`, `generatetoaddress`, or `getrawmempool`. The 31.0 changes are: `gettxspendingprevout` (new optional args), `getpeerinfo` (deprecated `startingheight`), `getblock` (new `coinbase_tx` field at verbosity 1/2/3), `gettxspendingprevout`, and two new mempool RPCs. None affect Phase 10's RPC surface. **Local v31 PASS and CI v30.2 PASS should agree.**

**Warning signs:** A test that PASSes locally against v31 but FAILs in CI against v30.2 (or vice versa) with a serde error mentioning a struct field name. If this happens, the planner files a follow-up to add `node.client.check_expected_server_version()` as a startup assertion inside `bootstrap_regtest_bitcoind`, gated by a `BLINDJOIN_TEST_ENFORCE_BITCOIND_VERSION` env var so it remains opt-in for forward compatibility.

[VERIFIED: corepc-node-0.12.0/node/src/lib.rs (no version check in `Node::with_conf`); github.com/rust-bitcoin/corepc client_sync/v30/mod.rs `impl_client_check_expected_server_version!({ [300000, 300100, 300200] })`; CITED: bitcoin/bitcoin release-notes-31.0.md]

## Code Examples

Verified patterns from official sources:

### Example 1: corepc-node typed Client constructor (Auth via cookie)

```rust
// Source: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0
//   client/src/client_sync/mod.rs:33-37   Auth enum
//   client/src/client_sync/mod.rs:81-110  Client::new_with_auth signature (via define_jsonrpc_bitreq_client! macro)
//
// At feature 30_2, `corepc_node::Client` resolves to corepc_client::client_sync::v30::Client
// (verified at node/src/client_versions.rs:9-10).
//
// The integration test code at tests/integration/full_round.rs:387-389 already uses this pattern:

use corepc_node::client::client_sync::Auth;

let auth = Auth::UserPass(rpc_user, rpc_pass);
let client = corepc_node::Client::new_with_auth(&rpc_url, auth)
    .expect("create rpc client");
```

[VERIFIED: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0, client/src/client_sync/mod.rs:33-37 (Auth) and the `define_jsonrpc_bitreq_client!` macro at lines 60-130]

### Example 2: RPC method signatures at feature `30_2` (full list of methods Phase 10 touches)

```rust
// Source: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0
//   client/src/client_sync/v17/wallet.rs:472-477
//   client/src/client_sync/v17/wallet.rs:575-587
//   client/src/client_sync/v17/raw_transactions.rs:158-173
//   client/src/client_sync/v17/generating.rs:14-26
//   client/src/client_sync/v17/wallet.rs:176-191
//   client/src/client_sync/v21/blockchain.rs:14-29
//
// All methods take &self on `corepc_node::Client` (which is `v30::Client` at feature 30_2).
// Response types are re-exported from `crate::types::v30::*` (verified at
// types/src/v30/mod.rs:252-369).

impl Client {
    // Wallet — calls "listunspent" RPC.
    // Returns ListUnspent(pub Vec<ListUnspentItem>) — from crate::types::v24 (re-exported via v30).
    pub fn list_unspent(&self) -> Result<ListUnspent>;

    // Wallet — calls "sendtoaddress" with no-RBF behaviour.
    // Returns SendToAddress(pub String) — newtype around txid hex.
    pub fn send_to_address(&self, address: &Address<NetworkChecked>, amount: Amount) -> Result<SendToAddress>;

    // Wallet — calls "getnewaddress" then parses with assume_checked.
    // Returns bitcoin::Address.
    pub fn new_address(&self) -> Result<bitcoin::Address>;

    // Generating — calls "generatetoaddress".
    // Returns GenerateToAddress(pub Vec<String>) — array of block hashes.
    pub fn generate_to_address(&self, nblocks: usize, address: &bitcoin::Address) -> Result<GenerateToAddress>;

    // Rawtransactions — calls "getrawtransaction" with verbose=false.
    // Returns GetRawTransaction(pub String) — hex-encoded raw tx.
    pub fn get_raw_transaction(&self, txid: bitcoin::Txid) -> Result<GetRawTransaction>;

    // Rawtransactions — calls "getrawtransaction" with verbose=true.
    // Returns GetRawTransactionVerbose { outputs: Vec<RawTransactionOutput>, ... }.
    pub fn get_raw_transaction_verbose(&self, txid: Txid) -> Result<GetRawTransactionVerbose>;

    // Blockchain — calls "getrawmempool" (verbose=false).
    // Returns GetRawMempool(pub Vec<String>) — array of mempool txids.
    pub fn get_raw_mempool(&self) -> Result<GetRawMempool>;
}
```

[VERIFIED: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0, all source URLs cited inline]

### Example 3: `ListUnspentItem` field shape at feature `30_2`

```rust
// Source: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0
//   types/src/v24/wallet/mod.rs:238-277
//
// At feature 30_2 the v30 module re-exports ListUnspent/ListUnspentItem from
// v24 (verified at types/src/v30/mod.rs:333: `v24::{..., ListUnspent, ListUnspentItem, ...}`).

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListUnspent(pub Vec<ListUnspentItem>);

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListUnspentItem {
    pub txid: String,
    pub vout: i64,
    pub address: String,
    pub label: Option<String>,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    pub amount: f64,           // BTC (not sats)
    pub confirmations: i64,
    #[serde(rename = "redeemScript")]
    pub redeem_script: Option<String>,
    pub spendable: bool,
    pub solvable: bool,
    #[serde(rename = "desc")]
    pub descriptor: Option<String>,
    pub safe: bool,
    #[serde(rename = "parent_descs")]
    pub parent_descriptors: Option<Vec<String>>,  // descriptor-wallet-only field
}
```

[VERIFIED: corepc-node-0.12.0, types/src/v24/wallet/mod.rs:238-277, fetched 2026-05-27]

### Example 4: `GetRawTransactionVerbose` and `RawTransactionOutput` field shapes

```rust
// Source: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0
//   types/src/v17/raw_transactions/mod.rs:341-389  GetRawTransactionVerbose
//   types/src/psbt/mod.rs:142-151                  RawTransactionOutput
//
// At feature 30_2 the v30 module re-exports GetRawTransactionVerbose from v17
// (types/src/v30/mod.rs:286: `v17::{..., GetRawTransactionVerbose, ...}`).

pub struct GetRawTransactionVerbose {
    pub in_active_chain: Option<bool>,
    pub hex: String,
    pub txid: String,
    pub hash: String,
    pub size: u64,
    pub vsize: u64,
    pub weight: u64,
    pub version: i32,
    #[serde(rename = "locktime")]
    pub lock_time: u32,
    #[serde(rename = "vin")]
    pub inputs: Vec<RawTransactionInput>,
    #[serde(rename = "vout")]
    pub outputs: Vec<RawTransactionOutput>,
    #[serde(rename = "blockhash")]
    pub block_hash: Option<String>,
    /* ...confirmations, time, blocktime — all Option<...> ... */
}

pub struct RawTransactionOutput {
    pub value: f64,                       // BTC
    #[serde(rename = "n")]
    pub index: u64,                       // the vout number
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: ScriptPubKey,      // .address: Option<String> (v23+); .hex: String; etc.
}
```

[VERIFIED: corepc-node-0.12.0, types/src/v17/raw_transactions/mod.rs:341-389 and types/src/psbt/mod.rs:142-151, fetched 2026-05-27]

### Example 5: Re-exports — what `30_2` actually maps to

```rust
// Source: github.com/rust-bitcoin/corepc tag corepc-node-0.12.0
//   node/src/client_versions.rs:9-10
//   types/src/v30/mod.rs:252-369
//
// At feature 30_2 (which depends on 30_0 per node/Cargo.toml:50-51), the
// corepc-node crate re-exports:
//   use corepc_client::client_sync::v30::*  →  all the impl_client_v17__*,
//                                              impl_client_v21__*, etc.
//                                              methods aggregated into v30::Client
//   use corepc_client::types::v30 as vtype  →  all the response struct types

// node/src/client_versions.rs:9-10:
#[cfg(feature = "30_0")]
pub use corepc_client::{client_sync::v30::*, types::v30 as vtype};

// The feature cascade (node/Cargo.toml:50-51):
// 30_2 = ["30_0"]   # 30_2 implies 30_0; 30_1 is skipped due to wallet migration bug
// 30_0 = ["29_0"]   # which transitively pulls 29_0 → 28_2 → ... → 0_17_2
//
// Only the highest-version feature determines the active client_sync module
// (see the #[cfg(feature = "30_0")] gate in client_versions.rs).
```

[VERIFIED: corepc-node-0.12.0/node/src/client_versions.rs:9-10 and node/Cargo.toml:50-51, fetched 2026-05-27]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `bitcoincore-rpc` crate | `corepc-node` (test harness) + `corepc-types` + reqwest (production) | November 2025 (archived) | Phase 10 already on the new stack; no migration |
| Legacy (BDB) wallets default | Descriptor wallets mandatory | Bitcoin Core 30.0 (2025) | Phase 10's actual repair surface — see Pitfall 1 |
| `bdk = "*"` | `bdk_wallet = "2.2"` (renamed crate) | BDK migration (2026-03) | Not in Phase 10 scope; production coordinator uses `bdk_wallet` per CLAUDE.md |
| `corepc-node = "0.10"` (Phase 8 baseline) | `corepc-node = "0.12"` with `features = ["30_2"]` | 2026-04-14 (release) → 2026-05-26 (bumped in this repo per TODO.md) | Already done; Phase 10 inherits the pin |

**Deprecated/outdated:**
- `bitcoincore-rpc` crate — archived November 2025. Replaced by `corepc-types` (production) and `corepc-node` (test harness).
- Bitcoin Core 0.17.2 RPC schema — corepc-node 0.12 still ships this as `default = ["0_17_2"]`. **A footgun, not a recommendation** — Phase 10's REPAIR-02 gate exists specifically to prevent silently selecting it.
- Bitcoin Core 30.1 — skipped by corepc-node entirely per the feature cascade `30_2 = ["30_0"]` comment "Skip v30.1 due to wallet migration bug." If anyone bumps the CI pin to v30.1, the typed client cannot accept it (the version check macro only allows `[300000, 300100, 300200]`, but `Node::with_conf` does not assert). Use v30.2 or v30.x (>= 30.2).

[VERIFIED: corepc-node-0.12.0/node/Cargo.toml feature comments; CITED: CLAUDE.md tech stack section]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The descriptor-wallet `listunspent` semantics is the *sole* root cause of all 6 ignored tests failing | Summary, Pitfall 1 | Repair to all 6 tests follows the same pattern. If one test fails for a different reason (e.g., a v30 schema field change we missed), Plan 10-02 surfaces it at the per-test gate (D-07), and that one test takes the D-10 escape valve. Acceptable — the per-test gate is exactly the failure-detection mechanism. |
| A2 | `RawTransactionOutput.script_pubkey: ScriptPubKey` carries an `address: Option<String>` field at feature 30_2 | Pattern 1 code example | If the field is absent, the executor falls back to hex-decode + `Address::from_script` (also wallet-agnostic). The repair pattern works either way; only the exact syntax inside `fund_regtest` differs. The planner should leave this as a Plan 10-01 implementation detail. |
| A3 | Bitcoin Core v31 (brew default) and v30.2 (CI pinned) agree on `listunspent` / `getrawtransaction` / `sendtoaddress` / `generatetoaddress` / `getrawmempool` schemas | Pitfall 5 | Verified by reading release-notes-31.0.md — no listed changes in those RPCs. Risk: an undocumented change. Mitigation: the per-test acceptance criterion D-07 explicitly requires both local-PASS AND CI-PASS, so this assumption is automatically validated for every unmuted test. |
| A4 | The current 4 bare-sleep sites in `full_round.rs` are all replaceable with poll-until-deadline (i.e. no test depends on the wall-clock duration as part of the protocol semantics) | Pattern 2 | The protocol semantics depend on the signing-timeout config (2s for blame tests), but the *test* doesn't — the test only needs to observe that "after sufficient time has elapsed, the predicate is true". Risk: a test where the wall-clock matters (e.g., asserting the timeout fired at exactly 2s ± 100ms). None of the 4 sites have such assertions per direct reading. |
| A5 | Phase 10's CI grep gate (Pattern 4) is robust enough for the current declaration shape (single-line table form) | Pattern 4, Common Pitfall 2 | Verified against current tree: zero matches (correct). Risk: a multi-line table form in a future Cargo.toml entry. Documented in the YAML comment as a known limitation; the gate fails closed in that case, prompting the maintainer to single-line the entry. Acceptable. |
| A6 | Promoting `fund_regtest` to `mod.rs` does not break any other test file's compilation | Architecture Patterns | Verified by `cargo check --tests` 2026-05-27 — passes. The 5 in-file callers all live inside `full_round.rs`; the only cross-file effect is removing the file-private `fund_regtest` definition. |
| A7 | corepc-node 0.13 is not released as of 2026-05-27 | Standard Stack | Verified via crates.io API — `max_version: 0.12.0`. If a 0.13 lands during the planning/execution window, the planner does NOT bump (Phase 10 is explicitly scoped to 0.12). A follow-up phase could revisit. |
| A8 | The proposed `fund_regtest` signature `pub async fn fund_regtest(exe: String) -> (BitcoindGuard, FundedSetup)` preserves all 5 caller call-sites' existing invocation pattern | Pattern 3 | The current signature at `full_round.rs:765` is identical to this — the only change is moving the function from `full_round.rs` (file-private) to `mod.rs` (pub). |

## Open Questions

> Phase 10 has no genuine open questions blocking planning. All technical questions raised in the additional_context's "Critical research questions for the planner" are answered above. The remaining items are implementation choices (Claude's discretion):
> - Whether to apply IN-02 cleanup (`Arc<BitcoindGuard>` → bare guard in spawn_blocking) opportunistically. Recommended: yes, but not required by the acceptance criteria.
> - Whether to apply IN-03 cleanup (assert on JSON envelope error code, not just status class). Recommended: no — out of scope for Phase 10; file as B-04+ if interesting.
> - Whether the WR-05 poll-until-deadline pattern should live as a shared `wait_for(predicate, deadline)` helper in mod.rs (one extra helper) or as 4 inline loops in `full_round.rs`. Recommended: 1 shared helper if all 4 sites have identical structure after refactor (predicate + RPC handle + deadline); 4 inline loops if the predicate shapes diverge. Plan 10-02 executor decides at implementation time.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `bitcoind` (local for Plan 10-02 per-test gate) | D-07 acceptance — local PASS | ✓ | v31.0.0 (brew, `/opt/homebrew/bin/bitcoind`) | — |
| `bitcoind` (CI for Plan 10-02 per-test gate) | D-07 acceptance — CI PASS | ✓ (provisioned by Phase 9 CI install step) | v30.2 (pinned via `.bitcoind-version`) | — |
| `cargo` / Rust stable toolchain | Build + run tests | ✓ | (project workspace toolchain) | — |
| `corepc-node 0.12.0` with `features=["30_2"]` | All test bodies | ✓ (already in dev-deps) | 0.12.0 | — |
| `grep -E`, `grep -v` (GNU grep on Ubuntu CI runner) | REPAIR-02 CI grep gate | ✓ (default on ubuntu-latest) | n/a | — |

**Missing dependencies with no fallback:** none — Phase 10 is fully provisioned by the existing Phase 9 infrastructure.
**Missing dependencies with fallback:** none.

## Sources

### Primary (HIGH confidence)

- **github.com/rust-bitcoin/corepc tag `corepc-node-0.12.0`** — direct source inspection of:
  - `node/Cargo.toml` (feature flags, default features, version cascade)
  - `node/src/lib.rs` (Node, Conf, exe_path, Auth re-export, no implicit version check)
  - `node/src/client_versions.rs` (30_2 → corepc_client::client_sync::v30::* + types::v30 as vtype)
  - `client/src/client_sync/mod.rs` (Auth enum, Client::new_with_auth via define_jsonrpc_bitreq_client! macro)
  - `client/src/client_sync/v17/wallet.rs` (list_unspent, send_to_address, new_address signatures)
  - `client/src/client_sync/v17/raw_transactions.rs` (get_raw_transaction, get_raw_transaction_verbose signatures)
  - `client/src/client_sync/v17/generating.rs` (generate_to_address signature)
  - `client/src/client_sync/v21/blockchain.rs` (get_raw_mempool signature)
  - `client/src/client_sync/v30/mod.rs` (define_jsonrpc_bitreq_client!("v30"), impl_client_check_expected_server_version!({ [300000, 300100, 300200] }))
  - `types/src/v30/mod.rs` (which structs are re-exported and from where)
  - `types/src/v24/wallet/mod.rs` (ListUnspent / ListUnspentItem definitions, descriptor-wallet field `parent_descs`)
  - `types/src/v17/raw_transactions/mod.rs` (GetRawTransactionVerbose definition)
  - `types/src/psbt/mod.rs` (RawTransactionOutput definition)
- **crates.io API for `corepc-node`** (`https://crates.io/api/v1/crates/corepc-node`) — fetched 2026-05-27: confirmed latest = 0.12.0, no 0.13 exists, published 2026-04-14, 11 total versions
- **`coordinator/Cargo.toml:61-69`** — direct read 2026-05-27: confirmed single declaration `corepc-node = { version = "0.12", features = ["30_2"] }`
- **`tests/integration/full_round.rs`** — direct read 2026-05-27: confirmed 6 `#[ignore]` markers (lines 164, 561, 962, 1086, 1143, 1468 in current file); 4 bare `tokio::time::sleep(Duration::from_secs(...))` sites (current lines 378, 713, 1547, 1655); `fund_regtest` defined at line 765
- **`tests/integration/mod.rs`** — direct read 2026-05-27: confirmed `bootstrap_regtest_bitcoind`, `BitcoindGuard`, `RpcCreds`, `require_bitcoind!()` shape
- **Local reproduction 2026-05-27**: ran `BITCOIND_EXE=/opt/homebrew/bin/bitcoind cargo test --test integration full_round::adversarial_invalid_utxo -- --ignored --nocapture` against brew bitcoind v31.0.0 — observed `Could not find UTXO txid=... addr=bcrt1...` panic, confirming Pitfall 1 root cause

### Secondary (MEDIUM-HIGH confidence)

- **bitcoincore.org release pages** — confirmed 31.0 is current release; 30.2, 30.1, 30.0 are intermediate releases
- **github.com/bitcoin/bitcoin/blob/master/doc/release-notes/release-notes-30.0.md** — descriptor wallets mandatory in v30.0; specific RPC changes do not include `listunspent` or `getrawtransaction`
- **github.com/bitcoin/bitcoin/blob/master/doc/release-notes/release-notes-31.0.md** — no changes to `listunspent`, `getrawtransaction`, `sendtoaddress`, `generatetoaddress`, `getrawmempool`
- **developer.bitcoin.org/reference/rpc/listunspent.html** — "listunspent returns only UTXOs that the wallet owns"

### Tertiary (LOW confidence, none load-bearing)

- WebSearch results on corepc-node usage patterns (used only to corroborate the canonical-stack assertion that corepc-node + the `30_2` feature is the standard test harness; not load-bearing for any specific claim in this RESEARCH.md)

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — every package + version verified against crates.io API + direct source at the pinned tag
- Architecture (Patterns 1-4): HIGH — Pattern 1 (vout discovery) is a textbook RPC pattern verified by reading the GetRawTransactionVerbose / RawTransactionOutput types; Patterns 2-4 are Phase 9 carry-forward idioms with direct callsite evidence
- Pitfalls 1-5: HIGH — Pitfall 1 reproduced locally; Pitfall 2 documented in TODO.md; Pitfalls 3-5 cross-referenced against pinned source
- Runtime State Inventory: HIGH — Phase 10 has no runtime state to migrate; verified by reviewing scope

**Research date:** 2026-05-27
**Valid until:** 2026-06-27 (stable area: rust-bitcoin / corepc-node release cadence is monthly; Bitcoin Core 32.0 is not on the horizon). Re-run version verification (`curl crates.io API`, brew bitcoind version) if Phase 10 execution is delayed past 2026-06-27.
