# Phase 10: full_round.rs decision + execution - Pattern Map

**Mapped:** 2026-05-27
**Files analyzed:** 7 (3 code, 4 docs)
**Analogs found:** 7 / 7 (100 % — all analogs are in-repo and well-established Phase 9 patterns)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `tests/integration/mod.rs` (modify: add `fund_regtest` + `FundedSetup`) | test fixture | request-response (RPC + spawn_blocking bridge) | `tests/integration/mod.rs::bootstrap_regtest_bitcoind` (same file, sibling helper) | **exact** |
| `tests/integration/full_round.rs` (modify: vout repair, unmute 6, sleep→poll ×4) | integration test | event-driven (multi-client orchestration); request-response (RPC + HTTP) | `tests/integration/full_round.rs::wait_for_coordinator` (lines 116-135) for poll-until-deadline; `tests/integration/round_bootstrap.rs:127-204` for the explicit deadline-loop variant | **exact** |
| `.github/workflows/ci.yml` (modify: add `corepc-node feature pin check` job) | CI config | batch (one grep + exit) | `audit` job at lines 168-181 in same file (smallest, no toolchain-cache shape needed) | **exact** |
| `.planning/ROADMAP.md` (modify: "15 tests" → "8 tests") | docs (markdown) | n/a | self-edit; no analog needed | n/a |
| `.planning/REQUIREMENTS.md` (modify: REPAIR-01 "15" → "8") | docs (markdown) | n/a | self-edit; no analog needed | n/a |
| `TODO.md` (conditional, only on D-10 retirement) | docs (markdown) | n/a | `TODO.md:3-53` "Resolved 2026-05-27" Phase 9 closure entry | **exact** |
| `.planning/BACKLOG.md` (conditional, only on D-10 retirement) | docs (markdown) | n/a | `BACKLOG.md:11-32` B-01 entry | **exact** |

## Pattern Assignments

### `tests/integration/mod.rs` — ADD `fund_regtest` + `FundedSetup`

**Role:** test fixture (async helper exposed at crate root of the integration test binary)
**Data flow:** request-response (corepc-node sync RPC bridged onto tokio via `spawn_blocking`)

**Analog:** `tests/integration/mod.rs::bootstrap_regtest_bitcoind` (lines 271-322) — the sibling helper. This is the canonical Phase 9 shape for promoting a shared regtest fixture: doc block first, then `pub async fn` taking the resolved `exe` path, then the entire blocking RPC body inside `tokio::task::spawn_blocking(move || { ... }).await.expect(...)`.

**Imports pattern** (from the sibling helper, lines 271-279):
```rust
pub async fn bootstrap_regtest_bitcoind(exe: String) -> (BitcoindGuard, RpcCreds) {
    tokio::task::spawn_blocking(move || {
        use bitcoin::Address;
        use corepc_node::{Conf, Node};
        // ... function-local `use` declarations rather than top-of-file ...
```

Apply identically to `fund_regtest`: function-local `use` declarations (`bitcoin::{Amount, Address, PrivateKey, CompressedPublicKey, Network, secp256k1::Secp256k1}`, the corepc-node types needed for `get_raw_transaction_verbose`). Keep them local to the `spawn_blocking` closure, mirroring the existing style.

**Doc-block pattern** (from the sibling helper, lines 229-270):
```rust
/// Spin up a regtest `bitcoind`, mine 101 blocks, and return an
/// [`RpcCreds`] handle bound to a [`BitcoindGuard`] that owns the daemon
/// process for the caller's full scope.
///
/// **Single locus** for regtest bring-up across all bitcoind-dependent
/// integration tests (D-13 + D-14). [...]
///
/// **Caller contract:** Hold the returned [`BitcoindGuard`] for the
/// test's full duration. Dropping it earlier kills bitcoind mid-test,
/// breaking subsequent RPC calls. [...]
///
/// Canonical caller shape:
/// ```ignore
/// #[tokio::test]
/// async fn my_test() {
///     let exe = require_bitcoind!();                       // skip if missing
///     let (guard, creds) = bootstrap_regtest_bitcoind(exe).await;
///     // ... use creds; hold guard for the test's duration ...
/// }
/// ```
```

Apply the same shape to `fund_regtest`: state the **single locus** justification (D-06 promotion), the **caller contract** (hold the guard), a **schema note** (the v30 descriptor-wallet gotcha — Pitfall 1 from RESEARCH.md) explaining *why* this helper exists at all, and a canonical caller skeleton. Use `///` doc syntax + `[`type`]` link references, never bare `//` comments — matches the sibling helper.

**Async-blocking-bridge pattern** (from the sibling helper, lines 277-321):
```rust
pub async fn bootstrap_regtest_bitcoind(exe: String) -> (BitcoindGuard, RpcCreds) {
    tokio::task::spawn_blocking(move || {
        // ... all sync RPC + Node setup work here ...
        let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");
        let cookie = node.params.get_cookie_values()
            .expect("read cookie file").expect("parse cookie values");
        // ... mine 101 blocks ...
        let mine_addr: Address = node.client.new_address().expect("get new address");
        node.client.generate_to_address(101, &mine_addr).expect("generate 101 blocks");
        (BitcoindGuard::new(node), creds)
    })
    .await
    .expect("regtest bootstrap spawn_blocking panicked")
}
```

For `fund_regtest`, do NOT re-spawn its own bitcoind — **call `bootstrap_regtest_bitcoind(exe).await` first** (composition, not duplication). Then do the funding work inside ONE additional `spawn_blocking` block. This mirrors the existing `full_round.rs:765-849` file-private `fund_regtest`. The cleanup opportunity (Anti-Pattern from RESEARCH.md / IN-02): drop the `Arc<BitcoindGuard>` plumbing — move the bare `BitcoindGuard` into the closure, return it from the closure with the `FundedSetup`, no `Arc::try_unwrap` needed at the end.

**Struct-promotion pattern** (from `mod.rs::RpcCreds`, lines 108-120):
```rust
/// Bitcoind RPC credentials extracted from the regtest cookie file.
///
/// Canonical handoff struct between [`bootstrap_regtest_bitcoind`] and
/// consuming tests. `user` and `pass` come from the bitcoind cookie file
/// (via `Node::params::get_cookie_values`), NOT from any configured
/// credentials — corepc-node provisions a per-run cookie inside the
/// node's tempdir-backed datadir.
#[derive(Clone, Debug)]
pub struct RpcCreds {
    pub url: String,
    pub user: String,
    pub pass: String,
}
```

Apply identically to `FundedSetup`: short doc block stating the handoff contract, `#[derive(Clone, Debug)]`, all fields `pub` (since the type crosses the `mod.rs` ↔ `full_round.rs` module boundary). Preserve the existing 4-field shape from the current file-private struct (`tests/integration/full_round.rs:140-146`):

```rust
struct FundedSetup {
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
    /// (outpoint "txid:vout", value_sats) for each participant
    utxos: [(String, u64); 3],
}
```

The promoted version differs only in visibility: `pub struct FundedSetup { pub rpc_url: String, ... }`.

**Vout-discovery pattern** (NEW for this phase — RESEARCH.md Pattern 1; replaces the broken `list_unspent` scan at `full_round.rs:813-825`):
```rust
// Wallet-agnostic UTXO discovery — works on v30 descriptor wallets where
// list_unspent does NOT return UTXOs paid to addresses the wallet does
// not own. The 3 test WIFs derive addresses that are external to the
// bitcoind wallet, so we must parse the funding tx directly.
//
// get_raw_transaction_verbose returns GetRawTransactionVerbose whose
// .outputs is Vec<RawTransactionOutput>. Each output carries
// .script_pubkey: ScriptPubKey (with .address: Option<String> on v23+
// types — re-exported through v30 at feature 30_2) and .value: f64 (BTC)
// and .index: u64 (the vout number). See RESEARCH.md Example 4.

use std::str::FromStr;
use bitcoin::Txid;

let recipient_str = recipient_addr.to_string();
let txid = Txid::from_str(&funding_txid_str).expect("valid txid hex");
let tx = node.client.get_raw_transaction_verbose(txid)
    .expect("get_raw_transaction_verbose");
let out = tx.outputs.iter()
    .find(|o| o.script_pubkey.address.as_deref() == Some(&recipient_str))
    .unwrap_or_else(|| panic!(
        "funding tx {} has no output to {}",
        funding_txid_str, recipient_str
    ));
let outpoint = format!("{}:{}", funding_txid_str, out.index);
let value_sats = (out.value * 100_000_000.0).round() as u64;
```

**Note on the field-name fallback:** if `script_pubkey.address` is absent at the `30_2` re-export of the v17 type, fall back to hex-decode + `Address::from_script` (also wallet-agnostic). The executor picks whichever compiles first; both are acceptable. RESEARCH.md Assumption A2 documents this.

**Conventions to preserve:**
- Visibility: `pub` on the function and on `FundedSetup` (crosses module boundary).
- Error handling: `.expect(...)` on every RPC call with a message naming the specific failure (matches sibling helper). NOT `?` — these are tests; loud panics are the contract.
- Async style: `pub async fn`; body is `spawn_blocking(move || { ... }).await.expect(...)`.
- Naming: snake_case helper name matches the existing file-private `fund_regtest`; struct name unchanged.
- `#[derive(Clone, Debug)]` on `FundedSetup` (matches `RpcCreds`).

---

### `tests/integration/full_round.rs` — repair callsites + unmute + sleep→poll

**Role:** integration test (multi-client end-to-end orchestration)
**Data flow:** request-response (HTTP to coordinator) + event-driven (waiting on async phase transitions)

**Analog:** in-file (`wait_for_coordinator`, lines 116-135) for the poll-until-deadline shape, and `tests/integration/round_bootstrap.rs:127-204` for the explicit deadline-loop with side-effect-bearing predicate. Both established Phase 9 patterns.

**Import-update pattern** (current line 24):
```rust
use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};
```

After Plan 10-01: add `fund_regtest` and `FundedSetup`:
```rust
use crate::{
    bootstrap_regtest_bitcoind, fund_regtest, require_bitcoind, BitcoindGuard,
    FundedSetup, RpcCreds,
};
```

**Ignore-marker removal pattern** (current lines 163-165, repeated at 560-562, 961-963, 1085-1087, 1142-1144, 1467-1469):
```rust
#[tokio::test]
#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
async fn full_round_three_clients() {
```

After Plan 10-02 per-test unmute: delete the `#[ignore = ...]` line **only after** local v31 PASS + CI v30.2 PASS gate (D-07):
```rust
#[tokio::test]
async fn full_round_three_clients() {
```

The whole-file invariant at end of Plan 10-02 is `grep -c '^#\[ignore' tests/integration/full_round.rs` → `0`.

**Poll-until-deadline pattern — in-file analog** (`wait_for_coordinator`, lines 116-135):
```rust
async fn wait_for_coordinator(coordinator_url: &str) {
    let http_client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let ok = http_client
            .get(format!("{}/info", coordinator_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Coordinator did not start within 5 seconds"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
```

**Poll-until-deadline pattern — cross-file analog** (`round_bootstrap.rs:127-145`, illustrating an explicit-deadline loop whose predicate is computed each iteration):
```rust
let http_client = reqwest::Client::new();
let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
let mut last_info: Option<shared::protocol::InfoResponse> = None;
loop {
    if tokio::time::Instant::now() > deadline {
        run_handle.abort();
        panic!(
            "Coordinator never left Idle within 10s. Last /info: {:?}",
            last_info
        );
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    // ... compute predicate; `continue` if not ready, `return`/`break` if done ...
}
```

This is the load-bearing shape for the 4 WR-05 sleep→poll fixes. Each of the 4 sites becomes a loop with: a 100ms poll cadence, an explicit `deadline` Instant 5-10× the original sleep budget, a per-iteration predicate evaluated via `spawn_blocking` (for the corepc-node RPC sites — `get_raw_mempool`) or a plain `await` (for the HTTP /info sites — see `wait_for_coordinator`), and a `panic!` with a useful diagnostic on deadline elapse.

**WR-05 site mapping** (4 bare sleeps to replace; line numbers drift, so grep for `tokio::time::sleep(Duration::from_secs(` not the literal offsets):

| Current line | Sleep duration | Predicate to poll for | Suggested deadline |
|--------------|----------------|------------------------|---------------------|
| 378 (full_round_three_clients) | 2s | mempool non-empty: `!client.get_raw_mempool()?.0.is_empty()` | 10s |
| 713 (blame test) | 4s | `info.round_state == "idle"` AND `ban_list.is_banned(utxo, now)` | 10s |
| 1547 (round_restart) | 4s | `info.round_state == "input_reg"` AND `ban_list.is_banned(utxo, now)` | 10s |
| 1655 (round 2 broadcast) | 2s | mempool non-empty (same predicate as line 378) | 10s |

**Mempool-poll RPC body pattern** (existing site at lines 384-395, executed once after the bare sleep at 378):
```rust
let mempool_txids: Vec<String> = tokio::task::spawn_blocking(move || {
    use corepc_node::client::client_sync::Auth;
    let auth = Auth::UserPass(rpc_user, rpc_pass);
    let client = corepc_node::Client::new_with_auth(&rpc_url, auth)
        .expect("create rpc client for mempool check");
    client.get_raw_mempool().expect("get_raw_mempool").0
}).await.expect("mempool check spawn_blocking");
```

Inside a poll loop, the same `spawn_blocking` body is invoked per-iteration. The clone-into-spawn-blocking string handling (`let rpc_url_c = rpc_url.clone();` per iteration) is the canonical Phase 9 idiom — see RESEARCH.md Pattern 2 example. Whether to extract a shared `wait_for(predicate, deadline)` helper into `mod.rs` is Claude's-discretion per CONTEXT.md (D-02, Open Questions §3): if all 4 predicates collapse to the same shape, one helper; otherwise 4 inline loops.

**Conventions to preserve:**
- Loop cadence: `tokio::time::sleep(Duration::from_millis(100))` between checks (matches `round_bootstrap.rs:141`).
- Deadline check: `assert!(tokio::time::Instant::now() < deadline, "...")` OR an explicit `if Instant::now() > deadline { panic!(...) }` block — both forms appear in the existing analogs. Either is acceptable.
- Diagnostic on timeout: include the failing predicate AND the most recent observation (e.g., `last_info` field at `round_bootstrap.rs:135-137`).
- Caller contract on RPC handles: clone the `String` URL/user/pass into each `spawn_blocking` closure; do NOT smuggle `&str` borrows across the `.await`.

**Local fund_regtest deletion** (current lines 140-146 + 749-849):
- Delete the file-private `struct FundedSetup { ... }` declaration at lines 140-146 (replaced by `crate::FundedSetup`).
- Delete the file-private `async fn fund_regtest(exe: String) -> (BitcoindGuard, FundedSetup)` at lines 749-849.
- Replace all 5 `fund_regtest(exe).await` callsites (at lines 966, 1090, 1147, 1472, and the inline body at 193-284 in `full_round_three_clients`) with `crate::fund_regtest(exe).await`. The `full_round_three_clients` inline body (lines 193-284) can be collapsed to the shared call now that the contract matches — confirmed at RESEARCH.md Assumption A8.

---

### `.github/workflows/ci.yml` — ADD `corepc-node feature pin check` job

**Role:** CI config (workflow job)
**Data flow:** batch (single `grep | grep -v | grep -v` chain + exit code)

**Analog:** the `audit` job at lines 168-181 in the same file — the smallest existing job (no toolchain cache, no bitcoind install, no test cache, no runtime work). A single `Run …` step with `set -euo pipefail`-style failure semantics.

**Job-skeleton pattern** (full `audit` job, lines 168-181):
```yaml
  audit:
    name: cargo audit
    runs-on: ubuntu-latest
    # Blocks merge: cargo audit exits non-zero on any advisory not listed in
    # .cargo/audit.toml. Each ignore in audit.toml carries a written rationale.
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
      - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
        with:
          toolchain: stable
      - name: Install cargo-audit
        run: cargo install cargo-audit --locked
      - name: Run audit
        run: cargo audit
```

Apply the same shape, but stripped of the toolchain steps (grep needs no Rust toolchain). REPAIR-02 gate body comes from RESEARCH.md Pattern 4:

```yaml
  corepc-node-feature-pin-check:
    name: corepc-node feature pin check
    runs-on: ubuntu-latest
    # REPAIR-02 invariant: every `corepc-node = ...` declaration in any
    # Cargo.toml in the workspace must include an explicit `features = ...`
    # clause. corepc-node defaults to the silent `0_17_2` (Bitcoin Core 0.17.2,
    # released 2018) RPC schema if no version feature is selected; this gate
    # catches future additions that forget the features clause.
    steps:
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
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

**Conventions to preserve:**
- **`actions/checkout` SHA pin:** use **exactly** `34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1` — the identical SHA already pinned by all 4 existing jobs (`test:31`, `clippy:139`, `coordinator-smoke:160`, `audit:174`). Do **not** introduce a different SHA; this would split the audit surface. Phase 6 baseline + Phase 9 Established Patterns from CONTEXT.md.
- **Job-name kebab-case** (top-level key) + **human-readable `name:`** field (matches all 4 existing jobs).
- **`runs-on: ubuntu-latest`** (matches all 4 existing jobs).
- **Top-of-job comment block** explaining *what* the gate is and *why* it exists (matches the `audit` job's 2-line rationale block and the `coordinator-smoke` block's longer rationale).
- **Failure semantics:** non-zero exit code on regression (matches `cargo audit`'s exit behavior). RESEARCH.md Pattern 4 documents the `&& exit 1 || exit 0` inversion is robust against the negative-grep chain.
- **No new dependencies / no `cargo install` step** — this gate uses only `grep` which is preinstalled on `ubuntu-latest`.
- **No reliance on `env:` block** at the workflow level — the gate is self-contained, does not need `BLINDJOIN_REQUIRE_BITCOIND`, does not need `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` (the env-block at lines 9-17 applies globally but the gate is indifferent).

**Position in file:** alphabetical-ish by name is the existing convention (`test`, `clippy`, `coordinator-smoke`, `audit` — roughly alphabetical with `test` as the anchor). Plan 10-02 adds the new job at the end of `jobs:`, after `audit`, which preserves the rough ordering.

---

### `.planning/ROADMAP.md` — text correction

**Role:** docs (markdown — Phase 10 entry)
**Data flow:** n/a

**Analog:** self-edit; no analog needed. Per CONTEXT.md D-03, two surgical replacements:

- **Line 80 (Phase 10 success criterion 1):**
  ```
  1. `cargo test --test integration full_round::` either runs all 15 tests to completion against the pinned bitcoind [...]
  ```
  becomes
  ```
  1. `cargo test --test integration full_round::` either runs all 8 tests (6 carve-outs to repair + 2 already-passing) to completion against the pinned bitcoind [...]
  ```

- **Phase 10 goal (line 75, optional touch-up if the phrasing still implies a count):** the current goal reads "the `full_round.rs` integration file is either fully green against the pinned bitcoind or explicitly retired" — no numerical count; safe to leave as-is. The D-03 correction is exclusively the success-criterion line.

**Conventions to preserve:**
- Surround literal commands with backticks (matches the existing entry).
- Keep the "OR retired" branch wording verbatim — D-03 is a number-only correction.
- No new line, no reformatting of the criterion list.

---

### `.planning/REQUIREMENTS.md` — text correction

**Role:** docs (markdown — REPAIR-01 entry)
**Data flow:** n/a

**Analog:** self-edit. Per CONTEXT.md D-03 + RESEARCH.md Phase Requirements table:

- **Line 20 (REPAIR-01 entry):**
  ```
  - [ ] **REPAIR-01**: `tests/integration/full_round.rs` is either repaired (all 15 tests pass against the pinned bitcoind version, including the 6 currently failing on `listunspent`/RPC schema drift) **OR** explicitly retired [...]
  ```
  becomes
  ```
  - [ ] **REPAIR-01**: `tests/integration/full_round.rs` is either repaired (all 8 tests pass against the pinned bitcoind version, including the 6 currently failing on `listunspent`/RPC schema drift) **OR** explicitly retired [...]
  ```

**Conventions to preserve:** literal `**REPAIR-01**:` prefix unchanged; checkbox state `[ ]` unchanged (this is the live tracker — Phase 10 deliverables include marking it `[x]` when complete, but that is a separate operation).

---

### `TODO.md` — conditional (D-10 escape valve fires)

**Role:** docs (markdown — "Resolved YYYY-MM-DD" section)
**Data flow:** n/a

**Analog:** existing "Resolved 2026-05-27" entry at `TODO.md:3-53` (the Phase 9 closure entry). Apply only if a single test retires under D-10 (CONTEXT.md), which the default expectation D-11 explicitly aims for **zero of**.

**Entry-shape pattern** (extracted from `TODO.md:3-53`'s structural skeleton):
```markdown
## Resolved 2026-05-27

- [x] **<Title> shipped (<milestone> Phase N).** <1-3 sentences naming
  what was tried, the specific blocker, what coverage gap remains, and
  what alternative coverage would close the gap.> See backlog entry
  B-NN. [optional pointer to PR / commit].
```

The Phase 9 entry follows this with multi-paragraph "what / why / cross-refs" content; for a single retired test, 1-3 sentences is the contract from D-10.

**Conventions to preserve:**
- Section header `## Resolved 2026-05-27` (today's date; the existing entry is already dated 2026-05-27 so a Phase 10 retirement appends a bullet under the same section rather than creating a new one — unless the entry needs an isolated header for readability, Claude's discretion).
- `[x]` checkbox prefix (matches existing resolved entries).
- Bold lead-in identifier (`**B-04:**` or `**<test name> retired:**`).
- Cross-reference the corresponding `B-NN` BACKLOG.md entry (matches the existing Phase 9 entry's `B-01` references).

---

### `.planning/BACKLOG.md` — conditional (D-10 escape valve fires)

**Role:** docs (markdown — `B-NN` deferred-item entry)
**Data flow:** n/a

**Analog:** existing `B-01 — Public-endpoint hardening` at `BACKLOG.md:11-32`. The format is well-defined and reused for B-02 (lines 36-55) and B-03 (lines 61-83).

**Entry template pattern** (extracted from `BACKLOG.md:11-32`):
```markdown
## B-NN — <short title>

**Status:** Deferred from <source phase>. <one-line reason, ideally with code-reference link>.

**Why deferred:** <1-2 sentences explaining the original scope-boundary>.

**Why it matters:** <1-3 sentences naming the privacy / correctness / coverage gap left open>.

**Scope:**
- <bullet 1>
- <bullet 2>
- <bullet 3>

**Dependencies:** <"None blocking" or named blockers>.

**Estimated complexity:** <Small / Medium / Large> phase. ~<N> plans.

**Recommended entry:** `/gsd-discuss-phase` (or skip-to `/gsd-plan-phase` for clear-scoped items).

**Source:** Surfaced by <phase NN> / <link to CONTEXT.md or REVIEW.md>.

---
```

For Phase 10 retirements specifically: D-10 calls out *protocol scenario lost, code reference of what was deleted, sketch of how a future test could cover it differently (property-based via proptest? isolated state-machine test without bitcoind? Tor-mode harness?)*. Map that to the `**Scope:**` bullets (alternative-coverage sketches) and `**Source:**` (link to Phase 10's `10-CONTEXT.md` and the deleted test's code reference).

**Conventions to preserve:**
- Heading `## B-NN — <title>` (em-dash, not hyphen — matches B-01/B-02/B-03).
- `**Bold:**` field prefixes (matches existing entries).
- Inline code-reference link form: `[coordinator/src/...](coordinator/src/...)` with the path repeated as the link target (matches B-01's `[coordinator/src/api/middleware.rs:1-2](coordinator/src/api/middleware.rs:1)`).
- Closing `---` separator between entries.
- Next free number: existing entries are B-01, B-02, B-03; first Phase 10 retirement uses **B-04**, second (if it happens) uses **B-05**. D-11 expects zero retirements; D-10 budgets at most one before the executor stops to discuss with the user.

---

## Shared Patterns

### `spawn_blocking` async-blocking bridge

**Source:** `tests/integration/mod.rs:271-321` (`bootstrap_regtest_bitcoind`); also `tests/integration/full_round.rs:193-284` (existing inline funding setup).

**Apply to:** the new `fund_regtest` helper body in `mod.rs`; the per-iteration mempool RPC inside each WR-05 poll loop in `full_round.rs`.

```rust
tokio::task::spawn_blocking(move || {
    // ... all sync RPC + bitcoin::* crate work here; corepc-node's Client
    // is not async-aware, so we run it on the tokio blocking pool ...
    (return_value_owned_by_closure)
})
.await
.expect("<context>-bound spawn_blocking panicked")
```

**Key conventions:**
- The closure is `move ||` — all captures owned (`String`s cloned from the outer scope, the `BitcoindGuard` itself moved in, etc.).
- The return type is owned (no borrows escape the closure).
- `.await.expect("<context>")` — single-line expect with a message that identifies *which* spawn_blocking this is (so a backtrace pinpoints the failing helper).
- For the BitcoindGuard cleanup in `fund_regtest`: move the bare guard into the closure, return it from the closure (eliminates the `Arc<BitcoindGuard>` plumbing — anti-pattern called out in RESEARCH.md).

### corepc-node `Client::new_with_auth` instantiation

**Source:** existing pattern at `tests/integration/full_round.rs:384-395` (mempool check) and `tests/integration/full_round.rs:1662-1668` (round-2 mempool check). Both identical.

**Apply to:** every spawn_blocking that needs a fresh corepc-node Client (the 4 WR-05 sites that need to poll the mempool; any new ad-hoc RPC inside `fund_regtest` that operates without holding the `BitcoindGuard.node()` reference).

```rust
use corepc_node::client::client_sync::Auth;
let auth = Auth::UserPass(rpc_user, rpc_pass);  // both Strings, moved in
let client = corepc_node::Client::new_with_auth(&rpc_url, auth)
    .expect("create rpc client for <purpose>");
client.<method>().expect("<method> failed")
```

**Conventions:**
- `Auth::UserPass(String, String)` — both fields are `String`, not `&str`. Move from outer-scope `let rpc_user = rpc_user.clone(); let rpc_pass = rpc_pass.clone();` ahead of the spawn_blocking. The `rpc_url` is also a `String` and is passed by reference (`&rpc_url`) into `new_with_auth`.
- Always `.expect(...)` with a context-bearing message (no `unwrap()` without diagnostics; no `?` — these are tests).

### `require_bitcoind!()` macro skip-or-panic gate

**Source:** `tests/integration/mod.rs:99-106`.

**Apply to:** every unmuted test that needs bitcoind (i.e., all 6 currently-ignored tests). They already use this pattern — no change required, just preserved verbatim after the `#[ignore]` removal:

```rust
#[tokio::test]
async fn <name>() {
    let exe = require_bitcoind!();                       // skip in local-dev, panic in CI
    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    // ... hold bitcoind_guard for the test's full duration ...
}
```

### SHA-pinned `actions/checkout`

**Source:** repeated 4 times in `.github/workflows/ci.yml` at lines 31, 139, 160, 174 — exactly `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`.

**Apply to:** the new `corepc-node feature pin check` job's checkout step. Use the **identical SHA** — Phase 6 baseline + Phase 9 Established Patterns. Do not introduce a new pin.

```yaml
      - uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
```

(The TODO at the top of `ci.yml` lines 3-7 tracks the action-wide bump to v6.x as a separate concern; Phase 10 does not advance that.)

### Doc-correction surgical edit

**Source:** the surgical edits made to `ROADMAP.md` and `REQUIREMENTS.md` are *string-replacement* operations — no analog file needed; the analog is the doc-string conventions already present in the same files.

**Apply to:** ROADMAP.md (success criterion 1) and REQUIREMENTS.md (REPAIR-01). The minimal-diff principle (only the count changes; rationale prose unchanged) preserves the surrounding doc style.

## No Analog Found

None — every modified/created file in Phase 10 has a strong in-repo analog. Phase 10 is unusual in that the entire repair surface is "use existing Phase 9 fixtures + Phase 9 idioms correctly," with one new wrinkle (the v30 descriptor-wallet vout-discovery pattern from RESEARCH.md Pattern 1) that does not yet exist in-repo but is fully specified by `corepc-node-0.12.0`'s pinned type signatures (see `Read` of `10-RESEARCH.md` Examples 1-5).

## Metadata

**Analog search scope:**
- `tests/integration/mod.rs` (the Phase 9 fixture file — single locus for shared test plumbing)
- `tests/integration/full_round.rs` (in-file analog for poll-until-deadline; current file-private `fund_regtest`)
- `tests/integration/round_bootstrap.rs` (cross-file consumer; explicit-deadline-loop variant of poll-until-deadline)
- `tests/integration/rate_limiting.rs` (cross-file consumer; same import-shape as `round_bootstrap.rs`)
- `.github/workflows/ci.yml` (sibling-job analogs: `audit`, `clippy`, `coordinator-smoke`)
- `coordinator/Cargo.toml` (the only `corepc-node = ...` declaration in the workspace; locked at lines 61-69)
- `.planning/BACKLOG.md` (existing B-01/B-02/B-03 entries — the canonical entry shape for D-10 retirement entries)
- `TODO.md` (existing "Resolved 2026-05-27" / "Resolved 2026-05-26" / "Resolved 2026-05-25" sections — the canonical entry shape for D-10 retirement narrative)

**Files scanned:** 8 (read fully or via targeted Grep + Read)
**Pattern extraction date:** 2026-05-27
