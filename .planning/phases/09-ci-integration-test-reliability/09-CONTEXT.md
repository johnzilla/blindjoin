# Phase 9: CI integration-test reliability - Context

**Gathered:** 2026-05-27
**Status:** Ready for planning

<domain>
## Phase Boundary

CI on every PR actually executes the bitcoind-dependent integration suite end-to-end — no silent graceful-skips, no leaked child processes blocking `cargo test`'s stdout pipe, no orphan `bitcoind` processes after the suite exits. A new `CONTRIBUTING.md` documents the canonical invocation pattern.

**Concretely in scope:**
- A pinned `bitcoind` binary is provisioned on the GitHub runner via an integrity-verified tarball install, cached between CI runs, and discoverable to `corepc-node` via `BITCOIND_EXE`.
- The graceful-skip pattern in `tests/integration/{full_round,rate_limiting,round_bootstrap}.rs` is replaced with a shared `require_bitcoind()` helper that **panics under `BLINDJOIN_REQUIRE_BITCOIND=1`** (set workflow-wide in CI) and falls back to today's skip behaviour locally.
- The `Box::leak(node)` lifecycle pattern at `full_round.rs:269,616,800`, `rate_limiting.rs:122`, and `round_bootstrap.rs:84` is replaced with a `BitcoindGuard` RAII type whose `Drop::drop` runs `node.stop()` + brief wait + SIGKILL fallback. Bootstrap logic is consolidated into a shared `bootstrap_regtest_bitcoind()` helper in `tests/integration/mod.rs`.
- `bitcoind` is launched with `-printtoconsole=0` and its child stdio is redirected to a per-test temp log file, so even if shutdown is slow the daemon never holds cargo's stdout pipe.
- A new `CONTRIBUTING.md` at repo root contains a "Running integration tests" section with the canonical command, log-file location, single-test invocation example, and a 3-line table mapping output strings to pass/fail/skip verdicts.

**Explicitly NOT in scope (belongs to Phase 10):**
- Repairing the 6 RPC-schema-drift-broken tests in `full_round.rs` (REPAIR-01).
- Auditing every workspace `corepc-node` dependency declaration for explicit version features (REPAIR-02 — incidentally already correct in `coordinator/Cargo.toml:65`, but a workspace-wide audit is Phase 10's job).
- These 6 broken tests will carry `#[ignore]` markers with `// TODO(Phase-10): RPC schema drift` comments so Phase 9 ships a green CI. Phase 10 removes the markers as it repairs them.

**Deferred to v1.4+:**
- Tor-mode integration harness (per REQUIREMENTS.md "Future Requirements" + STATE.md decisions); Phase 8 HUMAN-UAT item 3 remains a `deferred` item, not a Phase 9 deliverable.

</domain>

<decisions>
## Implementation Decisions

### bitcoind install in CI

- **D-01 (install method):** Tarball + `actions/cache`. Download Bitcoin Core's prebuilt `linux-x86_64` tarball from `bitcoincore.org`, verify integrity, extract to `~/.local/bin`, cache by version string. Cache hit is the steady-state path (fast, no network). Build-from-source, third-party action, and Docker-service-container alternatives were considered and rejected (slow / extra supply-chain surface / incompatible with corepc-node's spawn-the-child pattern).

- **D-02 (pin location):** A new `.bitcoind-version` plain-text file at repo root holds the version string (e.g. `30.0`). Single source of truth — CI reads it, `CONTRIBUTING.md` references it, future bumps are a single-line PR.

- **D-03 (version target):** `30.2` (Bitcoin Core 30.2, released 2026-01-10). **Amended from initial `30.0` per research.** v30.0 was withdrawn from `bitcoincore.org` (HTTP 404) over a wallet-migration data-loss bug; v30.2 is the rollback fix. The feature-name `30_2` in `corepc-node = { version = "0.12", features = ["30_2"] }` (`coordinator/Cargo.toml:65`) literally matches v30.2 better than v30.0. RPC-compatible with brew's `bitcoind v31.0.0` (verified during Phase 8 close, per TODO.md). Pin lives in `.bitcoind-version` as `30.2`.

- **D-04 (integrity verification):** `SHA256SUMS` + signed manifest. Workflow downloads `SHA256SUMS` and `SHA256SUMS.asc` alongside the tarball, verifies the `.asc` signature against a pinned Bitcoin Core release-signer PGP key (Andrew Chow's guix-signer key is the conventional choice — planner picks the exact fingerprint), then checks the tarball's hash against the verified `SHA256SUMS`. Matches the Phase 6 supply-chain bar (SHA-pin everything, no untrusted blobs).

- **D-05 (CI step placement):** Inline step in the existing `test:` job in `.github/workflows/ci.yml`, ordered before `Run tests`. No composite action / no shell-script extraction — simple to read in the workflow file. If a future Tor-mode harness job needs the same install, the planner can decide at that point whether to extract.

- **D-06 (runtime discovery):** `BITCOIND_EXE=$HOME/.local/bin/bitcoind` env var exported by the install step. `corepc-node::exe_path()` honors this env var first. Same mechanism documented for local dev in CONTRIBUTING.md (point at brew's bitcoind). Decoupled from `$PATH` ordering.

### Skip-vs-fail policy

- **D-07 (gate mechanism):** Env-var gate `BLINDJOIN_REQUIRE_BITCOIND=1`. Tests `panic!("bitcoind required but not found")` when the env var is set and `corepc-node::exe_path()` errors; tests fall back to today's `eprintln! + return` skip behaviour when the env var is unset. CI sets it, local dev does not — preserves the "I can run `cargo test` without bitcoind installed" UX while satisfying TEST-02 (no silent skips in CI).

- **D-08 (implementation locus):** Shared `pub fn require_bitcoind() -> String` helper in `tests/integration/mod.rs`. Every test calls `let exe = require_bitcoind();` instead of the local `match` block. Single point of policy across all 7 callsites — the next policy change touches one function.

- **D-09 (CI env-var placement):** Workflow-level `env:` block in `.github/workflows/ci.yml`, alongside the existing `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"`. Applies uniformly to every job; future jobs that run the same tests inherit it.

- **D-10 (Phase-10 carve-out):** The 6 currently-failing tests in `full_round.rs` (`full_round_three_clients`, `round_restart_and_completion_after_blame`, `adversarial_invalid_utxo`, `adversarial_replay_token`, `adversarial_wrong_denomination`, `blame_non_signer_timeout` — see TODO.md "Resolved 2026-05-26 / Integration test harness reliability" follow-up) carry `#[ignore]` markers with `// TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction` comments. Phase 9 CI runs `cargo test --test integration` **without `--include-ignored`** (**amended per research** — the flag would *execute* the broken tests and red the build; default cargo already emits a one-line `ignored` entry per `#[ignore]` test in its output, satisfying the visibility intent without execution). Phase 10 removes the markers as it repairs them, and its repairs are exercised by re-running with the flag locally.

### Box::leak replacement

- **D-11 (lifecycle pattern):** RAII drop guard returned from `spawn_blocking`. `spawn_blocking` returns `(FundedSetup, BitcoindGuard)` where `BitcoindGuard` owns the `corepc_node::Node` and its `Drop::drop` impl runs `node.stop()` + brief wait + SIGKILL fallback. Test holds the guard for its full duration; guard drops naturally on panic via standard Rust stack unwinding. No statics, no `ctor::dtor` registry, no `catch_unwind` heroics.

- **D-12 (shutdown sequence):** `node.stop()` RPC, wait up to ~3 seconds for graceful exit, fall back to SIGKILL via the child handle if still alive. Matches Bitcoin Core's intended shutdown path and is safe under regtest (disposable state).

- **D-13 (helper location):** Both `require_bitcoind()` (D-08), `BitcoindGuard`, and `bootstrap_regtest_bitcoind()` live in `tests/integration/mod.rs` — the existing shared module. No new `tests/common/` directory; tests import via the existing `mod.rs` path.

- **D-14 (bootstrap consolidation):** `bootstrap_regtest_bitcoind()` becomes a shared helper returning `(BitcoindGuard, RpcCreds { url, user, pass })`. Today's near-duplicate `Node::with_conf` + mine-101-blocks + cookie-extraction logic in `full_round.rs`, `rate_limiting.rs`, and `round_bootstrap.rs` collapses to a single implementation. Tests still own their post-bootstrap funding/key-derivation logic; only the daemon-bring-up is consolidated.

- **D-15 (stdio handling):** Set `Conf::view_stdout = false` (child stdio → `Stdio::null()`) **and** pass `-printtoconsole=0` via `Conf::args`. **Amended from initial "per-test temp log" per research:** `corepc-node 0.12::Conf` only exposes `view_stdout: bool` (inherit vs null) — no `Stdio::from(File)` path is available without bypassing the corepc-node spawn helper. `view_stdout=false` (Stdio::null) achieves D-15's actual goal — the child never holds cargo's stdout pipe, eliminating the pipe-hang root cause. Postmortem context remains available via bitcoind's on-disk `debug.log` inside its data-dir (`Node` exposes the datadir path). Belt-and-suspenders: even if shutdown is slow, /dev/null can't block.

### CONTRIBUTING.md invocation

- **D-16 (canonical command):** Plain `cargo test` with explicit redirect — no wrapper script, no cargo-nextest (**amended per D-10 research:** drop `--include-ignored`):
  ```
  BLINDJOIN_REQUIRE_BITCOIND=1 \
    BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
    cargo test --test integration 2>&1 \
    | tee target/integration-test.log
  ```
  Self-contained one-liner. The pipe is now safe because BitcoindGuard (D-11) kills bitcoind on drop and child stdio is routed to /dev/null (D-15). CONTRIBUTING.md additionally documents `cargo test --test integration -- --include-ignored` as the **local-only** invocation Phase-10 contributors use when iterating on the 6 carve-out tests.

- **D-17 (file scope):** Narrow — CONTRIBUTING.md as a new file scoped to integration testing and local-dev prerequisites only. Sections: "Local prerequisites" (brew install bitcoin, rust toolchain), "Running integration tests" (the canonical command + log location + single-test example + pass/fail/skip table), "Interpreting output". Anything broader (PR style, commit conventions) is scope creep for this phase.

- **D-18 (pitfall callout):** Brief — one sentence explaining the redirect (`bitcoind inherits cargo's stdout pipe; piping to | tail can hang`). No full leak-saga / corepc-node version history — that lives in TODO.md and git log.

- **D-19 (single-test example):** Yes — include `cargo test --test integration rate_limiting::info_endpoint_returns_429_when_flooded -- --include-ignored --nocapture` as an example of running one test in isolation, since the full suite is slow.

- **D-20 (log location):** `target/integration-test.log`. Lives under cargo's already-gitignored build dir; auto-cleaned by `cargo clean`; no new `.gitignore` entry needed.

- **D-21 (pass/fail/skip table):** Reference card mapping output strings to verdicts (**amended per D-10 research** — adds an `ignored` row explaining the Phase-10 carve-out lines):
  - `test result: ok. N passed; 0 failed; M ignored` → green. The `M ignored` count is expected: those are the Phase-10 carve-out tests with `#[ignore]` markers.
  - `test result: FAILED. N failed` → red.
  - `panicked at 'bitcoind required but not found'` → `BLINDJOIN_REQUIRE_BITCOIND` set but `BITCOIND_EXE` missing/wrong; check local install.

### Claude's Discretion

- Exact PGP fingerprint to pin for SHA256SUMS.asc verification (D-04). Planner picks from the current Bitcoin Core release-signer set; document the fingerprint in `.bitcoind-version` or a comment in the workflow.
- Exact filename pattern for the tarball (`bitcoin-30.0-x86_64-linux-gnu.tar.gz` vs `bitcoin-30.0-linux-amd64.tar.gz` — bitcoincore.org's actual naming controls this).
- Whether the per-test temp log path is parameterised (e.g., `$BLINDJOIN_TEST_LOG_DIR` override) or hardcoded to `$TMPDIR`. Hardcoded is simpler and matches the rest of the test fixtures.
- The exact wording / phrasing of the CONTRIBUTING.md sections. The decisions above specify *what* must be present; tone and prose are Claude's call.
- Whether to emit a single consolidated `target/integration-test.log` or one log per test binary. Single file matches the canonical command's `tee` target.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase goal + requirements
- [`.planning/ROADMAP.md`](../../ROADMAP.md) §Phase 9 — Goal, dependencies (Phase 8), 4 success criteria.
- [`.planning/REQUIREMENTS.md`](../../REQUIREMENTS.md) §"Test Infrastructure (TEST)" — TEST-01 through TEST-05 with traceability table.
- [`.planning/PROJECT.md`](../../PROJECT.md) §"Current Milestone: v1.3" — milestone goal framing.
- [`.planning/STATE.md`](../../STATE.md) §"Decisions" — v1.3 phase-shape rationale (why Phase 9 bundles all 5 TEST-* requirements).

### Code to modify
- [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) — Add bitcoind install step to `test:` job, add workflow-level `BLINDJOIN_REQUIRE_BITCOIND=1` env var. Test invocation stays `cargo test --test integration` (CI does **not** pass `--include-ignored` — per amended D-10).
- [`tests/integration/mod.rs`](../../../tests/integration/mod.rs) — Home for new `require_bitcoind()`, `BitcoindGuard`, `bootstrap_regtest_bitcoind()` helpers.
- [`tests/integration/full_round.rs`](../../../tests/integration/full_round.rs) — Lines 156–163, 540–550, 920–930, 1050–1060, 1110–1120, 1420–1430 (skip blocks → `require_bitcoind()`); lines 167–279, 561–620, 745–805 (bootstrap inlines → `bootstrap_regtest_bitcoind()` calls); lines 268–269, 615–616, 799–800 (`Box::leak` → guard return). Add `#[ignore]` + `// TODO(Phase-10)` to the 6 known-broken tests per D-10.
- [`tests/integration/rate_limiting.rs`](../../../tests/integration/rate_limiting.rs) — Lines 92–125 bootstrap; line 122 `Box::leak`; lines 175–185 skip block.
- [`tests/integration/round_bootstrap.rs`](../../../tests/integration/round_bootstrap.rs) — Bootstrap (~lines 70–90); line 84 `Box::leak`.

### Code to read (not modify)
- [`tests/integration/ban_list_persistence.rs`](../../../tests/integration/ban_list_persistence.rs) — Pure persistence test; no bitcoind. Reference for what a "no-fixture" integration test looks like; not touched by Phase 9.
- [`coordinator/Cargo.toml`](../../../coordinator/Cargo.toml) §lines 61–69 — `corepc-node = { version = "0.12", features = ["30_2"] }` already pins the RPC-schema feature flag. Phase 9 does not change this; Phase 10's REPAIR-02 will audit the workspace.
- [`coordinator/src/run.rs`](../../../coordinator/src/run.rs) — Integration-test entrypoint introduced in Phase 8's bootstrap fix; not modified here.

### Files to create
- `.bitcoind-version` (repo root) — Plain text: `30.2`. Single source of truth per D-02 (**version amended from 30.0 per research**).
- `CONTRIBUTING.md` (repo root) — Per D-17 / D-18 / D-19 / D-20 / D-21.

### Operator-side / project context
- [`TODO.md`](../../../TODO.md) §"Integration test harness reliability (FOLLOW-UP)" (lines ~28–75) — Documents the four root-cause findings Phase 9 addresses. Each finding maps to one TEST-* requirement.
- [`.planning/BACKLOG.md`](../../BACKLOG.md) — No Phase-9 entry; the work was pulled directly from Phase 8's HUMAN-UAT close.

### Crate / external doc references (consulted during research)
- `09-RESEARCH.md` — Phase 9 research findings, including the 3 amendments above and external sources (PGP fingerprint, corepc-node 0.12 source review, actions/cache v4 SHA pin).
- `corepc-node` 0.12 — `Node` (stop/Drop), `Conf` (view_stdout, args), `exe_path()`. (https://docs.rs/corepc-node/0.12)
- `actions/cache` v4.3.0 — Pin to release-SHA per Phase 6 supply-chain rule.
- Bitcoin Core 30.2 release page — `https://bitcoincore.org/bin/bitcoin-core-30.2/` for tarball + `SHA256SUMS` + `SHA256SUMS.asc` filenames.
- Bitcoin Core release-signer (achow101) PGP fingerprint: `152812300785C96444D3334D17565732E08E5E41` — verified present in v30.2 attestation directory; key blob lives at `https://raw.githubusercontent.com/bitcoin-core/guix.sigs/main/builder-keys/achow101.gpg` (planner picks a SHA-pinned commit of `bitcoin-core/guix.sigs` rather than `main` per supply-chain rule).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`tests/integration/mod.rs`** — already serves as the integration suite's shared module. Adding `require_bitcoind()`, `BitcoindGuard`, and `bootstrap_regtest_bitcoind()` here matches established structure; no new `tests/common/` directory needed.
- **`coordinator::run()`** ([`coordinator/src/run.rs`](../../../coordinator/src/run.rs)) — production startup callable from integration tests (landed during Phase 8). All Phase-9 test changes happen above this layer; `run()` itself is untouched.
- **`Swatinem/rust-cache@v2`** ([`.github/workflows/ci.yml:35`](../../../.github/workflows/ci.yml:35)) — cargo cache action already SHA-pinned. The new `actions/cache@v4` step for the bitcoind tarball follows the same SHA-pin discipline (Phase 6 standard).
- **Workflow-level `env:` block** ([`.github/workflows/ci.yml:10–17`](../../../.github/workflows/ci.yml:10)) — already established as the place for cross-job env vars (`FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`). `BLINDJOIN_REQUIRE_BITCOIND: "1"` slots in alongside it.

### Established Patterns
- **`BLINDJOIN_*` env-var naming** — `BLINDJOIN__COORDINATOR__*` for config overrides, `BLINDJOIN_ALLOW_CLEARNET=1` for release-mode gates (Phase 8). `BLINDJOIN_REQUIRE_BITCOIND=1` follows the second pattern: explicit, project-prefixed, boolean-by-presence.
- **`#[ignore]` markers with TODO comments** — TODO.md and `08-04` plan establish the precedent of `#[ignore]` on tests that depend on infrastructure not yet ready, paired with a tracking TODO line. Phase 9's 6 carve-outs (D-10) follow this pattern.
- **RAII guards for system resources** — Phase 8's `ConnectionGuard` (Tor accept-loop semaphore permit RAII) is the precedent. `BitcoindGuard` extends the same pattern to test-process bitcoind ownership.
- **Tower-style ServiceBuilder layering** — irrelevant here, listed only to note Phase 9 does *not* touch the coordinator's middleware stack.

### Integration Points
- **`tests/integration/mod.rs`** — single locus for shared helpers; tests import via `super::` or `crate::integration::`.
- **`.github/workflows/ci.yml` `test:` job** — single locus for the bitcoind install + the `cargo test` invocation. Other jobs (clippy, audit, coordinator-smoke) do not need bitcoind.
- **`coordinator/Cargo.toml:65`** — the `features = ["30_2"]` declaration is the contract between our test code and corepc-node. Phase 9 honors it (D-03 pins bitcoind v30.x); Phase 10 audits it (REPAIR-02).

### Critical constraint on corepc-node
- **`corepc_node::Node` is `!Send` in some configurations** ([`tests/integration/full_round.rs:166–167`](../../../tests/integration/full_round.rs:166) comment) — that's the reason today's tests do all synchronous bitcoind work inside a single `spawn_blocking` and `Box::leak` the node before returning. The Phase-9 fix preserves the `spawn_blocking` boundary but returns the `Node` (wrapped in `BitcoindGuard`) instead of leaking it. The guard itself is `Send` (it just holds the `Node`); test code holds it in the outer `tokio::test` scope.

</code_context>

<specifics>
## Specific Ideas

- The user explicitly wants to preserve local-dev UX where running `cargo test` without bitcoind doesn't fail. CI enforces; local dev is opt-in. This shape is captured in D-07 (gate via env var, not unconditional fail).
- "Bottom of stack" framing: D-15 (stdio redirect) is defense in depth in case D-11 (RAII guard) doesn't fully terminate bitcoind. The user accepted both rather than picking one — the pipe-hang has burned us once, belt-and-suspenders is the right posture.
- The Phase-10 carve-out (D-10) is explicit: Phase 9 ships with `#[ignore]` markers visible in CI output so the carve-out is *advertised*, not hidden. Phase 10's first task is removing those markers as it repairs the underlying tests.
- The "Phase 9 must ship green CI" constraint shaped multiple decisions: D-10 (carve out broken tests), D-15 (defense-in-depth stdio), D-21 (clear failure-mode reference card so a misconfigured local run isn't mistaken for a code bug).

### Research-driven amendments (2026-05-27, post-discuss)

- **D-03 `30.0` → `30.2`** — Bitcoin Core v30.0 was withdrawn from `bitcoincore.org` (HTTP 404 verified) over a wallet-migration data-loss bug; v30.2 is the rollback fix and also matches the `30_2` feature-name in `corepc-node` better than v30.0 did. User confirmed substitution.
- **D-15 `per-test temp log` → `view_stdout: false` (Stdio::null)`** — `corepc-node 0.12::Conf` only exposes a boolean `view_stdout` (inherit vs null); no `Stdio::from(File)` is available without bypassing the spawn helper. /dev/null still achieves the load-bearing goal (child doesn't hold cargo's stdout pipe). Postmortem context remains available via bitcoind's on-disk `debug.log` inside its data-dir. User confirmed substitution.
- **D-10 + D-16 + D-21 `--include-ignored` dropped** — `cargo test ... -- --include-ignored` *runs* ignored tests; the 6 carve-out tests would have failed CI. Default `cargo test --test integration` already emits a per-test `ignored` line, satisfying the "carve-out is advertised" intent. User confirmed dropping the flag.

</specifics>

<deferred>
## Deferred Ideas

- **Composite GitHub Action for bitcoind install** — D-05 keeps the install inline. If a future Tor-mode harness job (v1.4+) needs the same install, extract then.
- **cargo-nextest adoption** — Considered for D-16; rejected as out of scope. Could revisit if `cargo test`'s output-control limitations bite again.
- **scripts/test-integration.sh wrapper** — Considered for D-16; rejected as out of scope. Could revisit if the canonical command grows past ~3 env vars.
- **Tor-mode integration harness** — Already deferred to v1.4+ per REQUIREMENTS.md "Future Requirements". Phase 8 HUMAN-UAT item 3 remains `result: deferred`; Phase 9 does not advance it.
- **Workspace-wide audit of every `corepc-node` declaration for explicit version features (REPAIR-02)** — Phase 10's job; `coordinator/Cargo.toml:65` is already correct.
- **Repair of the 6 RPC-schema-drift `full_round.rs` tests (REPAIR-01)** — Phase 10. Phase 9 only adds the `#[ignore]` markers that Phase 10 will then remove one-by-one.

</deferred>

---

*Phase: 9-CI integration-test reliability*
*Context gathered: 2026-05-27*
