# Phase 9: CI integration-test reliability - Pattern Map

**Mapped:** 2026-05-27
**Files analyzed:** 7 (2 create + 5 modify)
**Analogs found:** 7 / 7

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `.github/workflows/ci.yml` (M) | CI workflow | event-driven (PR/push) | `.github/workflows/ci.yml` (existing `test:` job) | self-extend |
| `tests/integration/mod.rs` (M) | test fixture module | n/a (declares submodules + new shared helpers) | `tests/integration/mod.rs` (current 4-line module) + `coordinator/src/network/tor.rs` (ConnectionGuard RAII) | role-match (new helpers); structural-match (RAII) |
| `tests/integration/full_round.rs` (M) | integration test | request-response (HTTP) + RPC | `tests/integration/round_bootstrap.rs` | exact (same harness shape) |
| `tests/integration/rate_limiting.rs` (M) | integration test | request-response (HTTP) + RPC | `tests/integration/round_bootstrap.rs` | exact |
| `tests/integration/round_bootstrap.rs` (M) | integration test | request-response (HTTP) + RPC | self (its `bootstrap_regtest_bitcoind` helper at L96-128 is the analog being lifted into `mod.rs`) | exact |
| `.bitcoind-version` (C) | config file (pin manifest) | static | (none — first pin manifest in repo) | no analog |
| `CONTRIBUTING.md` (C) | project doc | static | `README.md` (existing repo-root doc) | role-match (doc tone/structure) |

## Pattern Assignments

---

### `tests/integration/mod.rs` (test fixture module — new shared helpers)

**Current state** (full content, 4 lines):
```rust
mod ban_list_persistence;
mod full_round;
mod rate_limiting;
mod round_bootstrap;
```

**Analog 1 (RAII guard shape):** `coordinator/src/network/tor.rs:44-66`

**Doc-comment + struct + impl + implicit Drop pattern** (lines 44-66):
```rust
/// RAII guard for the per-connection semaphore permit (Phase 8 WR-01 fix).
///
/// The original code held the permit alive via `let _permit = permit;` inside
/// the per-connection spawned task. That pattern is load-bearing — drop the
/// binding and the connection cap silently becomes unlimited — but a future
/// refactor "cleaning up an unused variable" can strip it without warning.
///
/// Wrapping the permit in a named struct makes the contract explicit:
///   - The struct's only purpose is to own the permit for the connection's
///     lifetime; clippy will not flag it as unused because it has methods
///     (the `Drop` impl is implicit; the field is private).
///   - Grep for `ConnectionGuard` to find every site that holds a permit.
///   - Removing the binding now requires removing the struct field too,
///     which is no longer a "clean up unused variable" change.
struct ConnectionGuard {
    _permit: OwnedSemaphorePermit,
}

impl ConnectionGuard {
    fn new(permit: OwnedSemaphorePermit) -> Self {
        Self { _permit: permit }
    }
}
```

**Copy guidance for `BitcoindGuard`:**
- Mirror the rationale-heavy doc comment shape (problem → invariant → why a named struct).
- `ConnectionGuard` relies on the implicit `Drop` (just dropping `OwnedSemaphorePermit` is enough). `BitcoindGuard` is **different**: it needs an **explicit `impl Drop`** because `node.stop()` is an RPC call we must invoke before `Node::Drop` runs `process.kill()`. Document this divergence inline.
- Use `Option<Node>` so `drop(&mut self)` can `.take()` the node (the standard "drain in Drop" idiom).
- Field is `node: Option<Node>` (not `_node`) because the field is **read** by `Drop::drop`, unlike `ConnectionGuard::_permit` which is only existence-checked.

**Analog 2 (env-var gate shape):** `coordinator/src/run.rs:296-305`

**`BLINDJOIN_*` env-var gate pattern** (lines 296-305):
```rust
let allow_clearnet = std::env::var("BLINDJOIN_ALLOW_CLEARNET")
    .map(|v| v == "1")
    .unwrap_or(false);
if !cfg!(debug_assertions) && !allow_clearnet {
    anyhow::bail!(
        "tor_mode = false in a release build, but BLINDJOIN_ALLOW_CLEARNET is not set. \
         Clearnet mode is dev/test only — set tor_mode = true (recommended) or set \
         BLINDJOIN_ALLOW_CLEARNET=1 to explicitly acknowledge the risk.",
    );
}
```

**Copy guidance for `require_bitcoind()`:**
- Match the `std::env::var("BLINDJOIN_*").map(|v| v == "1").unwrap_or(false)` shape exactly — this is the project's canonical boolean-by-presence env-var idiom.
- The error string structure (what is set / what is missing / how to fix) maps 1:1 onto Phase 9's `panic!("bitcoind required but not found ({e}). BLINDJOIN_REQUIRE_BITCOIND=1 — check that BITCOIND_EXE points to a valid binary.")`.
- One semantic divergence: `run.rs` uses `anyhow::bail!`; `require_bitcoind` uses `panic!` (test binaries don't return `Result`). RESEARCH.md Pattern 2 captures this — use the macro form `require_bitcoind!()` so `return` exits the calling `#[tokio::test]` not the test binary.

**Analog 3 (bootstrap consolidation source):** `tests/integration/rate_limiting.rs:92-128`

**Existing `bootstrap_regtest_bitcoind(exe: String)` to lift into `mod.rs`** (lines 92-128):
```rust
/// Spin up a regtest bitcoind via corepc_node, mine 101 blocks, and return its
/// RPC URL + cookie credentials. Leaks the `Node` so bitcoind stays alive for
/// the test's duration (OS reaps at process exit). Mirrors
/// `round_bootstrap.rs:59-89` verbatim.
async fn bootstrap_regtest_bitcoind(exe: String) -> (String, String, String) {
    tokio::task::spawn_blocking(move || {
        use bitcoin::Address;
        use corepc_node::{Conf, Node};

        let mut conf = Conf::default();
        conf.network = "regtest";

        let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");
        let cookie = node
            .params
            .get_cookie_values()
            .expect("read cookie file")
            .expect("parse cookie values");
        let rpc_url = node.rpc_url();
        let rpc_user = cookie.user.clone();
        let rpc_pass = cookie.password.clone();

        // Mine 101 blocks so the health check's block_count > 0 assertion holds.
        let mine_addr: Address = node.client.new_address().expect("get new address");
        node.client
            .generate_to_address(101, &mine_addr)
            .expect("generate 101 blocks");

        // Leak the node — OS reaps it at test exit.
        let node_box = Box::new(node);
        Box::leak(node_box);

        (rpc_url, rpc_user, rpc_pass)
    })
    .await
    .expect("regtest bootstrap spawn_blocking panicked")
}
```

**Copy guidance for the new shared `bootstrap_regtest_bitcoind()` in `mod.rs`:**
- This function is already in `rate_limiting.rs:92-128` — lift it verbatim, then:
  1. Change return type to `(BitcoindGuard, RpcCreds)` (RESEARCH.md A1 verifies `Node` is `Send`, so the guard crosses `.await` cleanly).
  2. Replace `Box::leak(node_box)` (lines 120-122) with `BitcoindGuard::new(node)`.
  3. Add `conf.view_stdout = false;` and `conf.args = vec!["-printtoconsole=0", "-fallbackfee=0.0001"];` before `Node::with_conf` (D-15 amended; defense-in-depth — `view_stdout=false` routes child stdio to `Stdio::null()`).
  4. Take no `exe` argument — call `require_bitcoind!()` internally (D-08 + D-13: single locus of policy).
- Keep the "mine 101 blocks" inline; D-14 only consolidates daemon-bring-up, not test-specific funding.
- Mirror `round_bootstrap.rs:59-89` rather than `rate_limiting.rs:92-128` for the doc-comment style — `round_bootstrap.rs` is the older, better-commented reference; `rate_limiting.rs`'s comment literally says "Mirrors round_bootstrap.rs:59-89 verbatim".

**`RpcCreds` struct location:** define inside `tests/integration/mod.rs` alongside `BitcoindGuard` and `bootstrap_regtest_bitcoind()`. No precedent — Phase 9 introduces it. Fields per RESEARCH.md L222-226: `url: String, user: String, pass: String`.

---

### `tests/integration/full_round.rs` (integration test — 4 Box::leak callsites + 7 skip blocks + 6 `#[ignore]` markers)

**Analog:** `tests/integration/round_bootstrap.rs` (post-Phase-9, after `mod.rs` shared helpers land)

**Existing skip block (the 7 callsites to replace)** — canonical instance at lines 156-163:
```rust
// ----- Step 1: skip if bitcoind not available -----
let exe = match corepc_node::exe_path() {
    Ok(p) => p,
    Err(e) => {
        eprintln!("bitcoind not found ({}), skipping full_round_three_clients", e);
        return;
    }
};
```

**Identical shape repeats at:** L156-163 (`full_round_three_clients`), L545-551 (`blame_non_signer_timeout`), L923 (`adversarial_replay_token`), L1050 (`adversarial_invalid_utxo`), L1110 (`adversarial_wrong_denomination`), L1422 (`round_restart_and_completion_after_blame`), and a 7th if the test count grows. Each is 7-9 lines.

**Replacement pattern (one line per callsite):**
```rust
let exe = require_bitcoind!();
```

Or — if the planner chooses the `pub fn require_bitcoind() -> String` form per RESEARCH.md Pattern 2 — the helper itself panics on CI miss and the local-dev skip path is handled by the macro. Either way, the 7-9-line `match` block collapses to one line.

**Existing Box::leak block (the 4 callsites to replace)** — canonical instance at lines 165-279 of `full_round_three_clients`:
```rust
// ----- Steps 2-4: all synchronous bitcoind work in one spawn_blocking -----
// corepc-node's Client is not Clone, so we do all sync work here and
// then leak the node to keep bitcoind alive for the coordinator's RPC calls.
let setup: FundedSetup = tokio::task::spawn_blocking(move || {
    use bitcoin::{
        secp256k1::Secp256k1, Address, Amount, CompressedPublicKey, Network, PrivateKey,
    };
    use corepc_node::{Conf, Node};

    let mut conf = Conf::default();
    conf.network = "regtest";

    let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");
    // ... (cookie extract, fund 3 UTXOs, mine confirmation block) ...

    // Leak the node so bitcoind stays alive for the coordinator's RPC calls.
    // This is acceptable in a test — OS reaps the process at test exit.
    let node_box = Box::new(node);
    Box::leak(node_box);

    FundedSetup {
        rpc_url,
        rpc_user,
        rpc_pass,
        utxos: [utxos_vec[0].clone(), utxos_vec[1].clone(), utxos_vec[2].clone()],
    }
})
.await
.expect("setup spawn_blocking panicked");
```

**Identical shape repeats at:** L268-269 (`full_round_three_clients`), L615-616 (`blame_non_signer_timeout`), L799-800 (`fund_regtest` helper, called by `adversarial_*` + `round_restart_*`). 4 Box::leak callsites total when counted with `rate_limiting.rs:122`.

**Replacement pattern:**
```rust
// Bootstrap shared daemon (lift from mod.rs).
let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;

// Funding is test-specific — keep inline, but receive the node by reference
// through bitcoind_guard.node() and Stop using the guard pattern.
let setup: FundedSetup = tokio::task::spawn_blocking({
    let url = creds.url.clone();
    let user = creds.user.clone();
    let pass = creds.pass.clone();
    move || {
        // ... existing funding logic (lines 189-265 in full_round.rs), but
        //     access the node via a fresh corepc_node::Client built from creds,
        //     OR (cleaner) pass `bitcoind_guard.node()` reference into the
        //     spawn_blocking via Arc<BitcoindGuard> if Node::client allows shared borrow.
        // ...
        FundedSetup { rpc_url: url, rpc_user: user, rpc_pass: pass, utxos: [...] }
    }
}).await.expect("setup spawn_blocking panicked");

// `bitcoind_guard` must remain in scope for the rest of the test;
// drops at end-of-scope → Drop::drop runs node.stop() then Node::Drop runs process.kill().
```

**6 tests requiring `#[ignore]` markers (per D-10):**

From TODO.md L57-69 + CONTEXT.md D-10, the broken-by-RPC-schema-drift tests are:
1. `full_round_three_clients` (line 155)
2. `blame_non_signer_timeout` (line 543)
3. `adversarial_replay_token` (line 922)
4. `adversarial_invalid_utxo` (line 1049)
5. `adversarial_wrong_denomination` (line 1109)
6. `round_restart_and_completion_after_blame` (line 1421)

**Marker pattern (Phase 9 introduces this — no precedent in the codebase):**
```rust
#[tokio::test]
#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction — see TODO.md"]
async fn full_round_three_clients() {
    // ...
}
```

Reference text comes verbatim from TODO.md L58-69 ("Almost certainly because corepc-node 0.12's `listunspent` / `getrawtransaction` response shapes differ from the v28 shapes the test was written against").

**Tests in `full_round.rs` that should NOT be ignored** (still passing, do not mark):
- `adversarial_tampered_psbt_rejected` (line 1217) — verify with planner
- `coordinator_info_endpoint_fields` (line 1293) — verify with planner

---

### `tests/integration/rate_limiting.rs` (integration test — 1 Box::leak + 2 skip blocks + 1 local bootstrap)

**Analog:** itself (lines 92-128 are the bootstrap to lift; lines 176-185 + 361-370 are the skip blocks to replace).

**Skip block** (lines 176-185 — same exact shape as `full_round.rs:156-163`):
```rust
// ----- Skip gracefully if bitcoind is unavailable -----
let exe = match corepc_node::exe_path() {
    Ok(p) => p,
    Err(e) => {
        eprintln!(
            "bitcoind not found ({}), skipping info_endpoint_returns_429_when_flooded",
            e
        );
        return;
    }
};
```

**Replacement:** `let _ = require_bitcoind!();` (or remove entirely, since `bootstrap_regtest_bitcoind()` calls it internally — clean up by deleting the skip block and calling bootstrap directly).

**Identical block at L361-370** for `request_timeout_returns_408` — same replacement.

**Box::leak block** (lines 96-128) — see Pattern Assignments → `tests/integration/mod.rs` → Analog 3 (this is the source the planner lifts into the shared helper). The replacement here is just deletion + call to the now-shared `bootstrap_regtest_bitcoind()`.

**Existing call site** (line 188) already matches the new shape:
```rust
// ----- Spin up regtest bitcoind so coordinator::run's startup_health_check passes -----
let (rpc_url, rpc_user, rpc_pass) = bootstrap_regtest_bitcoind(exe).await;
```
After Phase 9, becomes:
```rust
let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;
let (rpc_url, rpc_user, rpc_pass) = (creds.url, creds.user, creds.pass);
// hold bitcoind_guard for the test's full duration
```

---

### `tests/integration/round_bootstrap.rs` (integration test — 1 Box::leak + 1 skip block + 1 local bootstrap inline)

**Analog:** itself + the new shared `bootstrap_regtest_bitcoind()` in `mod.rs`.

**Existing inline bootstrap** (lines 45-89) — same shape as Pattern Assignments → mod.rs → Analog 3. Replace the entire 45-89 block with:
```rust
let (bitcoind_guard, creds) = bootstrap_regtest_bitcoind().await;
let rpc_url = creds.url;
let rpc_user = creds.user;
let rpc_pass = creds.pass;
// `bitcoind_guard` must outlive the test body — keep binding to the end.
```

This is the cleanest test to migrate (one call site, no funding step, no derived addresses).

---

### `.github/workflows/ci.yml` (CI workflow — add bitcoind install step + workflow-level env var)

**Analog:** `.github/workflows/ci.yml` itself.

**SHA-pin discipline** (existing pattern, line 28-32):
```yaml
- uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1
- uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # stable
  with:
    toolchain: stable
- uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2
```

**Copy guidance for the new `actions/cache@<sha>` step:**
- Format exactly: `uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0` (SHA from RESEARCH.md, verified via `api.github.com/repos/actions/cache/git/refs/tags/v4`).
- Comment after `#` is the human-readable version tag (matches existing convention).

**Workflow-level `env:` block** (existing pattern, lines 9-14):
```yaml
env:
  # Force GitHub Actions runner to execute Node 20 JS actions on Node 24,
  # silencing the deprecation warning ahead of the June 2026 hard cutover.
  # See: actions/checkout v6.0.2 still declares `using: node20` — upgrading
  # the action SHA is tracked separately (see TODO at top of ci.yml).
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
```

**Copy guidance:** Add `BLINDJOIN_REQUIRE_BITCOIND: "1"` to this same block with a 2-line comment explaining its purpose (CI demands the daemon; local dev opts out by not setting it). The new block should look like:
```yaml
env:
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"
  # Phase 9: when set, tests that fail to find bitcoind PANIC instead of
  # graceful-skipping. CI sets this; local-dev does not.
  BLINDJOIN_REQUIRE_BITCOIND: "1"
```

**Step ordering in `test:` job** (existing pattern, lines 24-34):
```yaml
test:
  name: cargo test
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@... # v4.3.1
    - uses: dtolnay/rust-toolchain@... # stable
      with:
        toolchain: stable
    - uses: Swatinem/rust-cache@... # v2
    - name: Run tests
      run: cargo test --workspace --all-targets
```

**Copy guidance:** New step ordering after Phase 9 (insert bitcoind install between rust-cache and Run tests):
```yaml
test:
  name: cargo test
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@... # v4.3.1
    - uses: dtolnay/rust-toolchain@... # stable
    - uses: Swatinem/rust-cache@... # v2
    - name: Read pinned bitcoind version
      id: bitcoind_version
      run: echo "version=$(cat .bitcoind-version)" >> $GITHUB_OUTPUT
    - name: Cache bitcoind binary
      id: cache-bitcoind
      uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830 # v4.3.0
      with:
        path: ~/.local/bin/bitcoind
        key: ${{ runner.os }}-bitcoind-${{ steps.bitcoind_version.outputs.version }}
    - name: Install bitcoind (cache miss only)
      if: steps.cache-bitcoind.outputs.cache-hit != 'true'
      run: |
        # SHA256 + PGP-verified tarball install — see RESEARCH.md Code Examples
        ...
    - name: Export BITCOIND_EXE
      run: echo "BITCOIND_EXE=$HOME/.local/bin/bitcoind" >> $GITHUB_ENV
    - name: Run tests
      run: cargo test --workspace --all-targets    # CI does NOT pass --include-ignored (amended D-10)
```

**Install-step body (full script):** See RESEARCH.md L460-512 "Code Examples → actions/cache@v4 step for bitcoind tarball" — copy verbatim, substituting Pitfall-2-corrected `30.2` for `30.0` and pinning the `GUIX_SIGS_SHA` to the current `bitcoin-core/guix.sigs` HEAD commit (planner picks at implementation time).

**No-touch jobs:** `clippy`, `coordinator-smoke`, `audit` (lines 36-82) do NOT need bitcoind. Leave untouched. The workflow-level `BLINDJOIN_REQUIRE_BITCOIND=1` is harmless to them (no test invocations consume it).

---

### `.bitcoind-version` (config file — new, repo root)

**Analog:** None — first pin manifest in the repo.

**Content** (single line, no trailing newline conventions to mirror; standard text file):
```
30.2
```

**Copy guidance:** Plain text, one line, version string only. CI reads via `cat .bitcoind-version`; CONTRIBUTING.md can reference it as the source of truth.

Optionally append a single-line comment with the achow101 PGP fingerprint, if the planner wants a self-documenting manifest:
```
30.2
# Verified against achow101 PGP fingerprint 152812300785C96444D3334D17565732E08E5E41
```
(but `cat .bitcoind-version` would then pick up the comment line — better to keep the file pure-version and document the fingerprint in `ci.yml` as a comment near the install step.)

---

### `CONTRIBUTING.md` (project doc — new, repo root)

**Analog:** `README.md` (existing repo-root doc)

**Tone/structure to mirror** — README.md sections:
- Top-of-file title and one-paragraph project summary (line 1-5)
- Numbered functional bullet list (lines 7-14)
- H2-headed feature sections (`## Quick Start (Docker)`, `## Build from Source`, `## Run the Coordinator`)
- Fenced code blocks with `bash` language tag for shell snippets (lines 28-37, 47-50, 56-67)
- Three-column markdown tables (lines 78-93) for reference data

**Specific README excerpts to mirror tone-of:**

*Code-block invocation style* (README.md:47-50):
```bash
cargo build --workspace
cargo test --workspace --all-targets   # unit + integration tests (integration tests skip gracefully without bitcoind)
```

*Inline env-var-flagged variant* (README.md:62-68):
```bash
# Start coordinator (clearnet, for development)
cargo run -p coordinator

# Start coordinator (Tor hidden service, for production)
BLINDJOIN_COORDINATOR_TOR_MODE=true cargo run -p coordinator
```

**Copy guidance for CONTRIBUTING.md (per D-17/18/19/20/21):**

Recommended section structure:
```markdown
# Contributing to blindjoin

[2-3 sentence intro — MIT, no fees, infrastructure not product. Mirror
README.md's opening sentence cadence.]

## Local prerequisites

- Rust 1.89+ (matches README.md line 45's floor — same toolchain)
- A `bitcoind` binary. Recommended: `brew install bitcoin` on macOS, or
  download v30.2 from `https://bitcoincore.org/bin/bitcoin-core-30.2/`
  (matches the pin in `.bitcoind-version`).

## Running integration tests

The integration suite under `tests/integration/` exercises a full CoinJoin
round against a real regtest `bitcoind`. Canonical invocation:

\`\`\`bash
BLINDJOIN_REQUIRE_BITCOIND=1 \
  BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration 2>&1 \
  | tee target/integration-test.log
\`\`\`

[One-sentence pitfall callout: bitcoind inherits cargo's stdout pipe; piping
to `| tail` can hang the suite.]

### Running a single test

[D-19 single-test example block.]

\`\`\`bash
BLINDJOIN_REQUIRE_BITCOIND=1 \
  BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --nocapture
\`\`\`

### Running ignored (Phase-10) tests locally

\`\`\`bash
cargo test --test integration -- --include-ignored
\`\`\`

[One-sentence explanation: these are the 6 RPC-schema-drift tests Phase 10
will repair; CI does not run them.]

## Interpreting output

| Output | Verdict |
|--------|---------|
| `test result: ok. N passed; 0 failed; M ignored` | Green — `M ignored` is expected (Phase-10 carve-outs) |
| `test result: FAILED. N failed` | Red |
| `panicked at 'bitcoind required but not found'` | `BLINDJOIN_REQUIRE_BITCOIND` set but `BITCOIND_EXE` missing/wrong |
```

**Scope discipline (D-17):** Do NOT add PR style, commit conventions, code style sections — those are scope creep for Phase 9. CONTRIBUTING.md is narrow: prerequisites + integration tests + output reference card.

---

## Shared Patterns

### RAII Drop Guard for OS Resources

**Source:** `coordinator/src/network/tor.rs:44-66` (Phase 8 `ConnectionGuard`)
**Apply to:** `BitcoindGuard` in `tests/integration/mod.rs`

```rust
// Pattern: rationale-heavy doc comment → struct (single private field) →
// inherent impl with `new` → implicit (or explicit) Drop.
//
// Divergence for BitcoindGuard: needs explicit impl Drop because
// node.stop() is an RPC call (not just a destructor side-effect).
// Use Option<Node> + .take() in drop so we can call methods on the
// owned Node before letting it drop.
```

### `BLINDJOIN_*` Env-Var Gate

**Source:** `coordinator/src/run.rs:296-298`
**Apply to:** `require_bitcoind()` / `require_bitcoind!()` in `tests/integration/mod.rs`

```rust
let allow_clearnet = std::env::var("BLINDJOIN_ALLOW_CLEARNET")
    .map(|v| v == "1")
    .unwrap_or(false);
```

Phase 9 analog:
```rust
let require = std::env::var("BLINDJOIN_REQUIRE_BITCOIND")
    .map(|v| v == "1")
    .unwrap_or(false);
```

### GitHub Actions SHA-Pinning

**Source:** `.github/workflows/ci.yml:28,29,32`
**Apply to:** New `actions/cache@<sha>` step

Format: `uses: <org>/<action>@<40-char-sha> # <human-readable-version>`. The comment after `#` is mandatory in this repo's discipline — RESEARCH.md L470 example matches.

### `tokio::task::spawn_blocking` for Sync RPC Work

**Source:** Every existing bitcoind test (full_round.rs:168, 366, 396, 554, 739, 1618, 1637; rate_limiting.rs:97; round_bootstrap.rs:59)
**Apply to:** `bootstrap_regtest_bitcoind()` in `tests/integration/mod.rs`

Idiom:
```rust
tokio::task::spawn_blocking(move || { /* sync corepc-node work */ })
    .await
    .expect("<descriptive panic message> spawn_blocking panicked")
```

The `.expect("... panicked")` message format is repo-canonical (verified across 8 callsites).

### `#[ignore]` with TODO Comment

**Source:** None in codebase yet — Phase 9 introduces this pattern.
**Apply to:** 6 `#[tokio::test]` functions in `tests/integration/full_round.rs` (per D-10).

**New canonical format** (Phase 9 establishes this — Phase 10 will follow):
```rust
#[tokio::test]
#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction — see TODO.md"]
async fn full_round_three_clients() { ... }
```

The reason string is human-readable AND surfaces in `cargo test` output (each ignored test prints `... ignored, <reason>`). The `// TODO(Phase-10):` token in the reason string is grep-friendly so Phase 10 can `grep -rn "TODO(Phase-10)" tests/` to find every marker.

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `.bitcoind-version` | config / pin manifest | static | First pin-manifest file in the repo. No existing single-purpose plain-text version file. Closest cousin is `rust-toolchain.toml` (not present) or `Cargo.lock` (managed by cargo). Just write the version string. |

`CONTRIBUTING.md` has a role-match analog (README.md) and is therefore listed under Pattern Assignments, not here.

`#[ignore]` markers have no current analog in the codebase but the pattern is straightforward enough that "no analog" is not blocking — Phase 9 establishes the canonical form.

## Metadata

**Analog search scope:**
- `tests/integration/` (5 files)
- `coordinator/src/` (full tree — looking for Drop impls + env-var gates)
- `.github/workflows/` (1 file)
- Repo root (README.md, TODO.md, CLAUDE.md)

**Files scanned:** 14 (3 read in full; 11 grepped + targeted reads)

**Files referenced for excerpts:**
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/tests/integration/mod.rs` (4 lines, full)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/tests/integration/full_round.rs` (1665 lines — targeted reads at 140-300, 540-624, 735-810)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/tests/integration/rate_limiting.rs` (583 lines — targeted reads at 85-220, 355-370)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/tests/integration/round_bootstrap.rs` (229 lines, full)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/.github/workflows/ci.yml` (82 lines, full)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/coordinator/src/network/tor.rs` (lines 40-130)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/coordinator/src/run.rs` (lines 285-325)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/README.md` (249 lines, full)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/TODO.md` (lines 50-75)

**Pattern extraction date:** 2026-05-27
