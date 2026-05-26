# TODO

## Resolved 2026-05-26

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
