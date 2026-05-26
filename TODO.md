# TODO

## Resolved 2026-05-26

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
- [ ] **B-01:** Public-endpoint hardening (rate limiting, timeouts, connection caps)
- [ ] **B-02:** BIP-322 multi-script support (P2TR, P2SH-P2WPKH)
- [ ] **B-03:** Dynamic fee estimation (mempool-aware, safety margin, RBF)

These are deferred features with full scoping in [`.planning/BACKLOG.md`](.planning/BACKLOG.md). Schedule into a future milestone — naturally a "v1.2 Production Readiness" set.
