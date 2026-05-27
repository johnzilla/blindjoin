# TODO

## Resolved 2026-05-27

- [x] **v1.3 Phase 9: CI integration-test reliability — SHIPPED.** All five
  TEST-XX requirements closed. Closes the four "Integration test harness
  reliability" follow-up findings (originally raised below, 2026-05-26):

  1. **CI now actually runs the integration tests.** `.github/workflows/ci.yml`
     `test:` job provisions Bitcoin Core v30.2 via integrity-verified install
     (PGP+SHA256, achow101 release-signing fingerprint
     `152812300785C96444D3334D17565732E08E5E41` pinned via guix.sigs SHA pin),
     cached across runs via `actions/cache@v4.3.0` keyed on `.bitcoind-version`.
     Workflow-level `BLINDJOIN_REQUIRE_BITCOIND=1` env makes integration tests
     panic-on-miss in CI instead of silently graceful-skipping.

  2. **`Box::leak`-pipe-hang eliminated.** Replaced 4 `Box::leak(node)` callsites
     with a shared `BitcoindGuard` RAII type whose `Drop::drop` calls
     `node.stop()` via `tokio::spawn_blocking` (so the kill happens off the
     tokio runtime thread per code-review CR-01). Combined with
     `conf.view_stdout = false` + `-printtoconsole=0`, the child never holds
     cargo's stdout pipe. Whole-repo `Box::leak` count in `tests/integration/`
     is now 0.

  3. **`full_round.rs` RPC-drift quarantined.** Six known-broken tests carry
     `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction
     -- see TODO.md"]` markers; CI shows them in the `ignored` column without
     executing. The repair-or-retire decision is Phase 10 (REPAIR-01 + REPAIR-02).

  4. **`CONTRIBUTING.md` documents the canonical invocation pattern.** Section
     "Running integration tests" with a copy-pasteable command, log file
     location (`target/integration-test.log`), single-test invocation example,
     and a 4-row pass/fail/skip/ignored reference card. New contributors no
     longer need to rediscover the pipe-buffering pitfall.

  UAT closure (this session):
  - **UAT-1** (live CI PASS verdict): PR #7 CI run 26512029044, 9 passed / 0
    failed / 6 ignored in 3m49s. First two CI runs caught real PGP-verify bugs
    (multi-signer SHA256SUMS.asc + long-key-id vs full-fingerprint matching) —
    fixed in `ea16787` + `6d10d05`. The live-CI signal Phase 9 SC1 was designed
    for worked as advertised.
  - **UAT-2** (bounded panic exit): Injected `panic!("UAT-2")` into
    `round_bootstrap`; 8s wallclock to clean exit, `panicked at` line in log,
    no hang.
  - **UAT-3** (no orphan bitcoind): Before/after `pgrep` both empty after full
    suite ran (7.51s, 9 passed / 0 failed / 6 ignored).

  Code review found 1 critical + 5 warnings + 5 info; 5 fixed atomically, 1
  deferred to Phase 10 (WR-05 bare-sleep migration in 4 already-ignored tests).
  See `.planning/milestones/v1.2-phases/08-public-endpoint-hardening/` for v1.2
  archive; see `.planning/phases/09-ci-integration-test-reliability/` for v1.3
  Phase 9 artifacts.

## Resolved 2026-05-26

- [x] **Phase 8 HUMAN-UAT items 1 & 2 closed by local runtime proof.** Ran
  `cargo test --test integration rate_limiting:: -- --include-ignored` against
  `bitcoind v31.0.0` (Homebrew). Both tests passed:
  `info_endpoint_returns_429_when_flooded` and `request_timeout_returns_408`.
  Item 3 (Tor connection-cap) remains deferred per Plan 04 A4 — see follow-up
  below.

  Getting there required two unrelated fixes that landed atomically:

  1. **Production bugfix in `coordinator/src/bitcoin/rpc.rs`:** JSON-RPC envelope
     bumped from `"1.1"` → `"2.0"`. Bitcoin Core 31 returns `-32600 JSON-RPC
     version not supported` for 1.1 requests; Bitcoin Core 27 (docker image) and
     all releases since ~v22 accept 2.0. This was a latent bug — the coordinator
     would have failed at startup against any modern bitcoind, but our docker
     stack runs v27 which still accepted 1.1.

  2. **Test harness bump:** `corepc-node` 0.10 → 0.12 with `features = ["30_2"]`.
     Defaults are silly — corepc-node 0.12 still defaults to a Bitcoin Core
     0.17.2 (2018) RPC schema unless an explicit version feature is enabled.
     Without the feature, the test harness `createwallet` RPC against bitcoind
     v31 hangs with "Could not create or load wallet".

  3. **Test fix in `tests/integration/full_round.rs`:** the hardcoded test WIFs
     had invalid Base58 checksums and would `panic!` on `PrivateKey::from_wif`.
     Replaced with deterministic valid regtest WIFs generated via
     `sha256("blindjoin-test-key-{A,B,C}")` → WIF encode.

- [x] **Integration test harness reliability** — all four findings closed by
  v1.3 Phase 9 (shipped 2026-05-27, PR #7). See "Resolved 2026-05-27" entry above
  for the closure breakdown. The single remaining item — Tor connection-cap
  runtime test (Phase 8 HUMAN-UAT #3) — remains deferred to v1.4+ pending a
  dedicated Tor-mode integration harness (arti + flooding client, ≥257 concurrent
  `.onion` streams). Tracked in `.planning/REQUIREMENTS.md` "Future Requirements"
  section.

- [x] **CI hygiene: full RUSTSEC-2026-0097 closure + Node 24 opt-in (quick
  task 260526-d7m + follow-up).** Bumped all three transitively-resolved rand
  tracks to their patched versions: `rand` 0.8.5 → 0.8.6 (closed initial 3
  Dependabot alerts), then 0.9.2 → 0.9.3 (via arti-client; closed alert #7
  after Dependabot's database expanded) and 0.10.0 → 0.10.1 (via
  blind-rsa-signatures; closed alert #1). All three are lockfile-only patch
  bumps within existing `"0.8"` / `"0.9"` / `"0.10"` semver constraints — no
  Cargo.toml edits. With every rand instance on a patched version, the
  `RUSTSEC-2026-0097` ignore in `.cargo/audit.toml` is no longer load-bearing
  and was removed entirely; cargo audit exits 0 with zero warnings.

  Added a workflow-root `env: FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"`
  block to `.github/workflows/{ci,release,docker}.yml` so all JS actions
  (`actions/checkout@v6.0.2`, `dtolnay/rust-toolchain@stable`, etc.) execute
  on Node 24 ahead of GitHub's June 2026 forced flip — silences the
  deprecation annotation without bumping pinned action SHAs. CI annotation
  language shifted from "may not work as expected" to "being forced to run
  on Node.js 24" — confirms the opt-in works.

  Notable finding: Dependabot's `vulnerable_version_range` field was narrower
  than RustSec's actual advisory coverage on the initial scan — the database
  expanded later to surface 0.9.x / 0.10.x instances that cargo audit had
  flagged from the start. Lesson: cross-check `cargo audit` output even when
  the Dependabot UI says "all resolved."

- [x] **B-01: Public-endpoint hardening shipped (v1.2 Phase 8).** Per-route
  rate limits via `tower_governor` 0.8 (reads 60/min, writes 30/min by default)
  return HTTP 429 + `Retry-After` + a `RATE_LIMITED` JSON envelope; uniform
  `tower_http::timeout::TimeoutLayer` (default 30s) returns HTTP 408 on stall;
  Tor accept-loop bounded by a `tokio::sync::Semaphore` (default 256) wrapped
  in a load-bearing `ConnectionGuard` RAII type. Four operator-tunable knobs
  on `[coordinator]` (`rate_limit_info_per_min`, `rate_limit_writes_per_min`,
  `request_timeout_secs`, `max_concurrent_connections`), all validated at
  startup via `CoordinatorConfig::validate()`. Release builds refuse to start
  in clearnet mode unless `BLINDJOIN_ALLOW_CLEARNET=1` is explicitly set.
  Rate limiter uses `GlobalKeyExtractor` (Tor-safe — `PeerIpKeyExtractor`
  would have been a critical bug). Integration test at
  `tests/integration/rate_limiting.rs` exercises both 429 and 408 end-to-end
  when bitcoind is available. Code review (`.planning/phases/08-public-endpoint-hardening/08-REVIEW.md`)
  found 2 BLOCKER + 6 WARNING items, all fixed in 7 atomic `fix(08): ...`
  commits before the phase landed. Phase verification 11/11 must-haves
  verified statically; 3 runtime items deferred to `08-HUMAN-UAT.md` for CI
  sign-off with bitcoind on PATH.

- [x] **Coordinator now bootstraps a round in production.** v1.1 shipped with the
  Idle→InputReg transition only inside `#[cfg(test)]` blocks; in production the
  coordinator stayed in Idle forever and every API request returned WRONG_PHASE.
  Promoted `start_round()` out of test scaffolding into
  `coordinator/src/round/manager.rs`, extracted `main`'s body into
  `coordinator::run()` so the integration test can spawn the real startup path,
  and extended the phase monitor to re-invoke `start_round()` on every return
  to Idle (continuous-rounds policy). New integration test
  `tests/integration/round_bootstrap.rs` exercises the in-process `run()` path
  and asserts InputReg + non-null RSA pubkey. Removed the
  `build_input_reg_round_state` test backdoor that masked this gap from v1.1
  verification — `full_round.rs` now delegates to the real `start_round()`.

- [x] **Ban list persists across coordinator restart in Docker.** The relative
  default `ban_list.jsonl` path resolved to the container's ephemeral working
  dir; restart wiped the list. Added `coordinator-data:/app/data` volume to
  `docker/docker-compose.yml`, set `BLINDJOIN__COORDINATOR__BAN_FILE_PATH` env
  var, and `RUN mkdir -p /app/data` in the Dockerfile. New integration test
  `tests/integration/ban_list_persistence.rs` exercises the production write
  path (`append_ban_entry`) and read path (`load_unexpired_entries`).

- [x] **CI runs integration tests.** v1.1 CI ran `cargo test --workspace --lib`
  which excluded the entire `tests/` directory — directly enabling the
  round-bootstrap regression to ship undetected. Switched to `--all-targets`,
  added `push: branches: [main]` trigger, and added a `coordinator binary
  builds` smoke job.

- [x] **Verification template hardened.** Added the heuristic to
  `~/.claude/agents/gsd-verifier.md`: when test setup constructs a production
  type, the verifier must cite the production analog (function + file:line) or
  mark the must-have as FAILED. Project-side record at
  `.planning/workstreams/fix-verification-gap/VERIFICATION-HEURISTIC.md`.

- [x] **Backdoor inventory complete.** Audited ~150 files for test-only
  helpers that construct production state types. 0 HIGH, 2 MEDIUM, 7 LOW. See
  `.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md`.

## Resolved 2026-05-25

- [x] **Integration tests now compile.** Reconciled struct/API drift in
  `tests/integration/full_round.rs` against current `CoordinatorSection`
  (`tor_mode`), `CoordinatorConfig` (`discovery`), and `Client::poll_until_phase`
  (`Duration` arg) signatures. Also fixed a needless-borrow lint in
  `client/src/round/sign.rs` and applied related lints in the integration
  test. CI clippy step upgraded to `--all-targets` so future test-code drift
  surfaces as a CI failure rather than silent rot. Integration tests still
  require a live bitcoind to run; without one they skip gracefully.

- [x] **`cargo audit` CI step is now blocking** (was previously
  `continue-on-error: true`). Documented residual-risk advisories declared in
  `.cargo/audit.toml`, each with a written rationale. `rustls-webpki` bumped
  to 0.103.13 to clear three open advisories.

## Open

### Tech debt
- [ ] **Migrate `make_input_reg_state` and `make_signing_state` to use production
  state-machine transitions.** Both build `RoundStateInner` via struct literal
  in `#[cfg(test)]` blocks; `make_signing_state` additionally does direct
  `state.phase = Phase::Signing` assignment (bypasses validator) and uses a
  placeholder RSA key. Currently flagged with `TODO(fix-verification-gap)`
  comments. Detail in
  [`.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md`](.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md)
  findings #1 and #2. Estimated ~30 min refactor.

### Scoped features (see BACKLOG.md)
- [ ] **B-02:** BIP-322 multi-script support (P2TR, P2SH-P2WPKH)
- [ ] **B-03:** Dynamic fee estimation (mempool-aware, safety margin, RBF)

These are deferred features with full scoping in [`.planning/BACKLOG.md`](.planning/BACKLOG.md). **B-01 shipped 2026-05-26** as Phase 8 of the v1.2 Production Readiness milestone (see Resolved above). B-02 and B-03 remain candidates for future milestones.
