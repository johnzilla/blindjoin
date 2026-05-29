---
status: issues_found
files_reviewed: 5
files_reviewed_list:
  - .github/workflows/ci.yml
  - tests/integration/mod.rs
  - tests/integration/full_round.rs
  - tests/integration/rate_limiting.rs
  - tests/integration/round_bootstrap.rs
depth: standard
counts:
  critical: 1
  warning: 5
  info: 5
total: 11
phase: 09-ci-integration-test-reliability
reviewed: 2026-05-26T00:00:00Z
---

# Phase 9: Code Review Report

**Reviewed:** 2026-05-26
**Depth:** standard
**Files Reviewed:** 5
**Status:** issues_found

## Summary

Phase 9 ships the bitcoind-pinning CI block, a shared `require_bitcoind!()` /
`BitcoindGuard` / `bootstrap_regtest_bitcoind` fixture set in
`tests/integration/mod.rs`, the six `#[ignore]` markers on bitcoind-coupled
tests in `full_round.rs`, and migration of `rate_limiting.rs` /
`round_bootstrap.rs` onto the shared fixtures. The mechanical migration is
clean: all six `#[ignore]` markers use the agreed string verbatim
(`"TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"`),
no `Box::leak` / `std::mem::forget` leftovers remain, the file-private
`bootstrap_regtest_bitcoind` is gone from `rate_limiting.rs`, all twelve
`uses:` lines in `ci.yml` carry 40-char SHAs with `# vX.Y.Z` or `# stable`
comments, and the `BLINDJOIN_REQUIRE_BITCOIND` env-var idiom is wire-compatible
with the canonical form at `coordinator/src/run.rs:296-298`.

The substantive finding is one Critical and several Warning items, almost all
clustered around the `BitcoindGuard::drop` blocking-in-async hazard and
synchronization gaps that *will* surface as flake on a busy CI runner.

## Critical Issues

### CR-01: `BitcoindGuard::drop` calls a synchronously-blocking `node.stop()` from a tokio runtime thread

**File:** `tests/integration/mod.rs:171-189`
**Issue:** `BitcoindGuard::drop` calls `n.stop()`, which (per
`corepc-node-0.12.0/src/lib.rs:548-551`) executes `self.client.stop()?` (sync
blocking JSON-RPC POST) followed by `self.process.wait()?` — a blocking
`std::process::Child::wait` that suspends the calling OS thread until bitcoind
exits. When a `#[tokio::test]` test ends, `Drop::drop` runs on the tokio
runtime worker thread (the same thread the test's `async fn` body was driven
on). A blocking `process.wait()` there parks the *executor* thread, not just
the task. For `#[tokio::test]` (current-thread runtime by default) this means
**the entire runtime is stalled** for as long as bitcoind takes to shut down.

The blocking is bounded (bitcoind exits within a few seconds on `stop`) so
the symptom is "test wall-clock includes shutdown time," not deadlock — but
the pattern is exactly the "do not block in async" anti-pattern. Worse: if
`client.stop()` returns Err because bitcoind already died (e.g. SIGKILLed by
an OOM), `process.wait()` still runs and is bounded only by OS reaper
behavior. On a slow runner that compounds the 5-test bootstrap cost.

This also means the doc claim at lines 174-179 — "Best-effort graceful
shutdown … swallow any error — the fallback is `corepc_node::Node`'s own
`Drop`" — is only partially correct. The fallback `Node::Drop` at
`corepc-node-0.12.0/src/lib.rs:575-582` ALSO calls `let _ = self.stop();` for
`Persistent` data dirs (which corepc-node's default tempdir-backed Conf is
NOT, so the second `stop()` is skipped in practice — fine), then
`self.process.kill()`. So the belt-and-suspenders claim is structurally
correct, but the synchronous wait in `n.stop()` runs first and already pays
the blocking cost.

**Fix:** Move the blocking shutdown off the runtime by handing the owned
`Node` to `tokio::task::spawn_blocking` and discarding the join handle. The
runtime worker is freed immediately; the blocking pool reaps bitcoind:

```rust
impl Drop for BitcoindGuard {
    fn drop(&mut self) {
        if let Some(mut n) = self.node.take() {
            // Offload the synchronously-blocking n.stop() (which calls
            // process.wait()) onto the blocking pool. We must not block a
            // tokio runtime worker — that stalls the executor for the
            // duration of bitcoind shutdown.
            //
            // We cannot .await the join handle here (drop is sync), so
            // detach. On test teardown the runtime shutdown will wait for
            // blocking-pool tasks to finish before the process exits, which
            // is the desired behavior.
            //
            // Outside a tokio runtime context (e.g. a sync test), fall back
            // to a direct blocking stop().
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    handle.spawn_blocking(move || {
                        let _ = n.stop();
                        // n drops here on the blocking pool; Node::Drop runs
                        // process.kill() as belt-and-suspenders.
                    });
                }
                Err(_) => {
                    let _ = n.stop();
                }
            }
        }
    }
}
```

If you accept the current blocking-in-drop behavior as a known cost, the
documentation at lines 174-189 must say so explicitly — the current comment
implies the pattern is fine, which it is not by tokio's own guidance.

## Warnings

### WR-01: `node.stop()` error is silently swallowed; no triage signal on shutdown failure

**File:** `tests/integration/mod.rs:184`
**Issue:** `let _ = n.stop();` swallows `client.stop` HTTP errors and
`process.wait` I/O errors with no log. Comment at lines 180-183 correctly
forbids panicking-in-drop, but the alternative — a single
`tracing::warn!`/`eprintln!` on Err — would be triage gold for a
shutdown-hang flake (Phase 9's whole reason for existing). The fallback to
`Node::Drop`'s `process.kill()` masks zombie / wedged-stop bugs in CI
indefinitely.

**Fix:**
```rust
if let Err(e) = n.stop() {
    eprintln!("BitcoindGuard: graceful stop failed ({e}); \
               relying on Node::Drop SIGKILL fallback");
}
```

### WR-02: PGP verify in CI does not assert the imported key has any signatures attached, only that the fingerprint is present

**File:** `.github/workflows/ci.yml:78-89`
**Issue:** The integrity gate fetches `achow101.gpg` from a SHA-pinned
guix.sigs commit (good), confirms the imported fingerprint matches
`KEY_FP=152812300785C96444D3334D17565732E08E5E41` (good), then runs
`gpg --verify SHA256SUMS.asc SHA256SUMS`. `gpg --verify` returns nonzero on
**bad** signature but ALSO returns 0 with a stderr warning when the signing
key has no trust path ("WARNING: This key is not certified with a trusted
signature!"). Under `set -euo pipefail` the trust warning is non-fatal — the
script proceeds. The script's verification is therefore "this SHA256SUMS was
signed by the key bound to `KEY_FP`, which we got from a pinned guix.sigs
commit" — which IS the intended security property, but only because we trust
the pinned guix.sigs commit. If the commit pin (`GUIX_SIGS_SHA`) is ever
bumped without re-reviewing the new commit, an attacker who's compromised
guix.sigs main HEAD could swap `achow101.gpg` for an attacker-controlled key
whose fingerprint matches `KEY_FP` (impossible — fingerprint is a
cryptographic hash of the key material) OR could swap a DIFFERENT key in but
also patch `KEY_FP` in the workflow in the same PR. The fingerprint check
defends against the first; PR review must defend against the second. Worth
adding a short rationale comment to that effect at lines 66-69 — the comment
currently says "catches a hostile guix.sigs commit substituting a different
key" without spelling out that PR review of the workflow itself is the
remaining trust root.

Also: `gpg --verify` is unhardened. Use `--status-fd` for machine-parseable
output and check for `GOODSIG` explicitly, which is the standard hardened
form:
```bash
gpg --status-fd=1 --verify SHA256SUMS.asc SHA256SUMS \
  | grep -E '^\[GNUPG:\] GOODSIG ' \
  || { echo "ERROR: gpg verify did not produce a GOODSIG line"; exit 1; }
```

**Fix:** Add the `GOODSIG` assert and amend the threat-model comment so
reviewers of future pins know that PR review IS part of the trust chain.

### WR-03: `bootstrap_regtest_bitcoind` panics on `Err` from `exe_path` even when called after `require_bitcoind!()` succeeded — failure mode is unlikely but documented incorrectly

**File:** `tests/integration/mod.rs:238-244`
**Issue:** The doc at lines 206-219 instructs callers to invoke
`require_bitcoind!()` first, which would either skip-via-return (local-dev,
graceful) or have already panicked (CI). If the macro returned `Some(p)`,
then by the time control reaches `bootstrap_regtest_bitcoind`, `exe_path()`
will return `Ok` again — corepc-node memoizes nothing, but reading the
`BITCOIND_EXE` env var is deterministic. So the `unwrap_or_else(panic!)`
branch is in practice dead. That's mostly fine, but the comment on lines
237-243 says "the macro's `None => return` expansion only works in a function
returning `()`" — which is true, but then justifies opening a SECOND code
path that re-resolves the exe and can independently panic. A simpler shape
would accept the `exe: String` as a parameter, removing the second
resolution entirely and removing the dead-panic branch:

```rust
pub async fn bootstrap_regtest_bitcoind(exe: String) -> (BitcoindGuard, RpcCreds) { ... }
// callers:
let exe = require_bitcoind!();
let (guard, creds) = bootstrap_regtest_bitcoind(exe).await;
```

This collapses three pieces of evidence (macro succeeded, helper resolved,
spawn_blocking re-resolved) into one. As-is, the divergence between
"required" path (macro panics with `BITCOIND_EXE` triage) and "helper" path
(helper panics with its own message naming both vars) is two places where
the operator-facing panic message can drift.

**Fix:** Either accept `exe: String` as a parameter (preferred), OR delete
the second `require_bitcoind_inner` call and document a hard precondition
"caller MUST have invoked `require_bitcoind!()`". Current shape is the
worst of both options — it re-resolves *and* duplicates the message.

### WR-04: `coordinator_info_endpoint_fields` spawns coordinator with non-routable bitcoind RPC URL — startup will silently log errors and never converge

**File:** `tests/integration/full_round.rs:1311-1419`
**Issue:** This test passes `bitcoin_rpc_url: "http://127.0.0.1:18443"` and
`bitcoin_rpc_user: "test"` to `BitcoinRpc::new`, but uses `build_router`
directly (not `coordinator::run`), so `startup_health_check` is *not* invoked
and the test passes. That's fine for /info smoke. But the `BitcoinRpc` Arc
will be used by any route that calls RPC — if the future `/info` handler
gains an RPC dependency (e.g., reporting current block height), this test
silently starts touching a non-existent bitcoind on port 18443, which on a CI
runner with an unrelated process bound to that port could become a flake or
worse. Smoke test should either explicitly document "in-process router only;
no RPC handler is reachable" OR use a clearly-unbindable URL like
`http://127.0.0.1:1/` to short-circuit any accidental connection.

**Fix:** Either add a clarifying assertion in the doc comment, OR change the
URL to `http://127.0.0.1:1/` and the credentials to `""`/`""` so any
accidental RPC use fails fast and obviously.

### WR-05: `tokio::time::sleep(Duration::from_secs(2))` / `(4)` flake risk on shared-runner CI

**File:** `tests/integration/full_round.rs:369, 704, 1519, 1627`
**Issue:** Multiple tests use bare `sleep` to await asynchronous events
(broadcast settling, signing-timeout firing, round restart). On a noisy CI
runner the 2s/4s windows can be exceeded under contention, especially with
bitcoind taking 1-3s to confirm a generate_to_address call. The current
`#[ignore]` shield masks this for the migration PR, but the bare-sleep
pattern will resurface as flake when Phase 10 lifts ignores. The right
shape is a poll-with-deadline (mirroring `wait_for_coordinator` at
`full_round.rs:116-135` and the explicit deadline in
`round_bootstrap.rs:128-204`).

**Fix:** Replace each `sleep` + assert pair with a polling loop that
deadlines out:
```rust
let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
while tokio::time::Instant::now() < deadline {
    if /* condition */ { return; /* or break to assert */ }
    tokio::time::sleep(Duration::from_millis(100)).await;
}
panic!("condition never satisfied within deadline");
```
Track as a Phase 10 line item alongside the RPC-schema unignore.

## Info

### IN-01: `Conf::default` "fallbackfee already included" comment is misleading

**File:** `tests/integration/mod.rs:265-270`
**Issue:** Comment claims `Conf::default()` "already includes
`-fallbackfee=0.0001`" then conditionally pushes if absent. If the comment
is correct, the check is dead and should be removed (with the comment as
the only evidence). If the comment might be wrong on a future corepc-node
bump, the comment should be downgraded to "guard against a corepc-node
default change." Either fix removes the small documentation drift risk.

**Fix:** Either delete the `if !` guard (trusting `Conf::default`) or
rephrase the comment to "guard against future Conf::default flip."

### IN-02: `Arc::try_unwrap` in `fund_regtest` is a structural smell

**File:** `tests/integration/full_round.rs:829-834`
**Issue:** `fund_regtest` wraps the guard in `Arc`, clones into a
`spawn_blocking` closure (which consumes its clone on return), then
`Arc::try_unwrap`s back to a bare `BitcoindGuard` to hand to the caller.
The `Arc` is purely to give `spawn_blocking` a `Send`-compatible borrow,
which is fine — but a cleaner shape moves the bare `BitcoindGuard` directly
into `spawn_blocking` and returns it from the closure:

```rust
let (guard, setup) = tokio::task::spawn_blocking(move || {
    // ... use guard.node() ...
    (guard, setup_data)
}).await.expect("...");
```

This removes 6 lines of `Arc::clone` / `Arc::try_unwrap` plumbing and the
"this is a bug" panic message in the unreachable `try_unwrap` failure
branch. The current shape is correct, just over-engineered. (Note:
`BitcoindGuard` already is `Send` — `corepc_node::Node` is `Send` per
RESEARCH.md A1, and `Option<Node>` is `Send`.)

**Fix:** Move bare guard into closure, return it from closure. Eliminates
`Arc` entirely.

### IN-03: Replay-token regression test depends on coordinator emitting client_error not server_error — but doesn't validate the specific error code

**File:** `tests/integration/full_round.rs:1058-1062`
**Issue:** `assert!(resp.status().is_client_error(), ...)` accepts ANY 4xx,
including 400 (parse error). A regression that changes the replay-token
handler to *crash on second invocation with a 400 due to deserialization*
would pass this assertion when the intent is to assert "coordinator
recognized the replay and returned a structured TOKEN_REPLAYED 4xx." Better
to assert on the JSON envelope's `error.code` field.

**Fix:**
```rust
let body: serde_json::Value = resp.json().await.expect("json");
let code = body.get("error").and_then(|e| e.get("code")).and_then(|c| c.as_str());
assert_eq!(code, Some("TOKEN_REPLAYED"), "replay must return TOKEN_REPLAYED");
```
Same applies to `adversarial_invalid_utxo` (line 1115) and
`adversarial_wrong_denomination` (line 1220). All three currently accept
"any 4xx" which is too loose.

### IN-04: `audit` job does not use `Swatinem/rust-cache` — `cargo install cargo-audit` re-downloads/recompiles every run

**File:** `.github/workflows/ci.yml:143-156`
**Issue:** The other three jobs use `Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2`
to cache the target/registry. The audit job re-installs cargo-audit from
source on every run (compiles ~80 crates). Adding `rust-cache` would cut
audit job wall-clock substantially; using `taiki-e/install-action` with
`tool: cargo-audit` would skip the compile entirely (binary download). Not
correctness; CI-quality.

**Fix:** Either add `Swatinem/rust-cache@<SHA> # v2` between the toolchain
and `cargo install` steps, or swap to `taiki-e/install-action@<SHA>` with
`tool: cargo-audit`. Both keep SHA-pin hygiene.

### IN-05: Env-var idiom doc claim "equivalent" is correct but uses a non-canonical spelling

**File:** `tests/integration/mod.rs:44-47, 52`
**Issue:** The doc comment cites `coordinator/src/run.rs:296-298` as the
canonical form (`.map(|v| v == "1").unwrap_or(false)`) and notes
`as_deref() == Ok("1")` is equivalent. Both are semantically identical, but
the codebase canonical form is the former. For consistency the helper
should match. Minor — would only matter if someone greps for the canonical
idiom and misses this file.

**Fix:**
```rust
let require = std::env::var("BLINDJOIN_REQUIRE_BITCOIND")
    .map(|v| v == "1")
    .unwrap_or(false);
if require { panic!(...) }
```

## Structural Findings (fallow)

No `<structural_findings>` block was supplied; this section is intentionally
omitted. Cross-module structural pre-pass was not performed for Phase 9.

---

_Reviewed: 2026-05-26_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
