# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- ✅ **v1.1 Security & Availability Hardening** — Phases 6-7 (shipped 2026-04-10)
- ✅ **v1.2 Production Readiness** — Phase 8 (shipped 2026-05-26)
- **v1.3 Test Infrastructure & Operational Hardening** — Phases 9-10 (in progress, started 2026-05-26)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-04-09</summary>

- [x] Phase 1: Core Protocol (6/6 plans) — completed 2026-04-09
- [x] Phase 2: Blame & Hardening (3/3 plans) — completed 2026-04-09
- [x] Phase 3: Client CLI (2/2 plans) — completed 2026-04-09
- [x] Phase 4: Discovery & Deployment (3/3 plans) — completed 2026-04-09
- [x] Phase 5: Tor & Release (3/3 plans) — completed 2026-04-09

</details>

<details>
<summary>✅ v1.1 Security & Availability Hardening (Phases 6-7) — SHIPPED 2026-04-10</summary>

- [x] Phase 6: CI/CD Security Pipeline (1/1 plans) — completed 2026-04-10
- [x] Phase 7: Coordinator DoS Hardening (3/3 plans) — completed 2026-04-10

</details>

<details>
<summary>✅ v1.2 Production Readiness (Phase 8) — SHIPPED 2026-05-26</summary>

- [x] Phase 8: Public-endpoint hardening (4/4 plans) — completed 2026-05-26

</details>

**v1.3 Test Infrastructure & Operational Hardening** (in progress)

- [x] **Phase 9: CI integration-test reliability** — Pin bitcoind in CI, eliminate the leaked-process stdout-hang, document the canonical invocation pattern so the integration suite actually runs end-to-end on every PR (completed 2026-05-27)
- [ ] **Phase 10: full_round.rs decision + execution** — Repair-or-retire the 8-test full_round suite (6 carve-outs to repair + 2 already-passing) against the pinned bitcoind, with explicit corepc-node version pinning everywhere a typed Client is used

## Phase Details

### Phase 9: CI integration-test reliability

**Goal**: Integration tests that depend on bitcoind run end-to-end in CI on every PR — no silent graceful-skips, no leaked child processes blocking stdout, and a documented invocation pattern future contributors can copy-paste
**Depends on**: Phase 8 (v1.2 shipped — the public-endpoint hardening that surfaced this rot is in production)
**Requirements**: TEST-01, TEST-02, TEST-03, TEST-04, TEST-05
**Success Criteria** (what must be TRUE):

  1. A fresh PR's CI log shows at least one bitcoind-dependent integration test executing with a PASS verdict (not a SKIPPED line) — pinned bitcoind is available on the runner via a cached install
  2. Running `cargo test --test integration` locally writes test output to a log file and the suite process exits within a bounded time even when an individual test panics — no leaked bitcoind blocks the cargo pipe
  3. When the integration suite completes (pass, fail, or panic), no orphan `bitcoind` processes remain in the process tree — `corepc-node` fixtures release their spawned daemons on test end
  4. `CONTRIBUTING.md` contains a section titled "Running integration tests" with a copy-pasteable command, an explanation of where output lands, and how to interpret pass/fail/skip — a new contributor can run the suite without rediscovering the pipe-buffering pitfall

**Plans**: 5 plans
Plans:
**Wave 1**

- [x] 09-01-PLAN.md — Provision pinned bitcoind v30.2 in CI (BLINDJOIN_REQUIRE_BITCOIND env, .bitcoind-version pin, actions/cache + PGP-verified install, BITCOIND_EXE export)
- [x] 09-02-PLAN.md — Add shared test fixtures to tests/integration/mod.rs (require_bitcoind! macro, BitcoindGuard RAII, RpcCreds, bootstrap_regtest_bitcoind helper)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 09-03-PLAN.md — Migrate tests/integration/full_round.rs to shared helpers (3 Box::leak removed, 6 skip blocks removed, 6 #[ignore = TODO(Phase-10)] markers added)
- [x] 09-04-PLAN.md — Migrate tests/integration/rate_limiting.rs + round_bootstrap.rs to shared helpers (1 Box::leak removed, file-private bootstrap deleted, 3 skip blocks removed)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 09-05-PLAN.md — Create CONTRIBUTING.md (Running integration tests + Local prerequisites + Interpreting output reference card per D-17..D-21)

### Phase 10: full_round.rs decision + execution

**Goal**: With the CI suite now actually running, the `full_round.rs` integration file is either fully green against the pinned bitcoind or explicitly retired with rationale in TODO.md — and any test in the workspace that touches corepc-node's typed Client pins an explicit version feature
**Depends on**: Phase 9 (CI must actually run the suite before "passes in CI" is a meaningful success bar; explicit version pinning is naturally exercised by whatever repair path is taken)
**Requirements**: REPAIR-01, REPAIR-02
**Success Criteria** (what must be TRUE):

  1. `cargo test --test integration full_round::` either runs all 8 tests (6 carve-outs to repair + 2 already-passing) to completion against the pinned bitcoind with a PASS verdict on every one, OR `tests/integration/full_round.rs` is deleted from the repo and the TODO.md "Resolved" section references the retirement decision with rationale
  2. `grep -r "corepc-node" --include='Cargo.toml'` shows every dependency declaration with an explicit `features = ["NN_M"]` entry — no test in the workspace silently depends on the corepc-node `0_17_2` (Bitcoin Core 0.17.2) default feature
  3. CI's integration-test job remains green on a PR that touches `tests/integration/full_round.rs` (or, if retired, on a PR that proves the retirement landed cleanly with no orphan references to the deleted module)

**Plans**: 2 plans

**Wave 1**

- [x] 10-01-PLAN.md — Promote fund_regtest + FundedSetup to tests/integration/mod.rs with wallet-agnostic vout discovery via get_raw_transaction_verbose (D-04 Plan 10-01, D-05, D-06)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 10-02-PLAN.md — Unmute 6 carve-out tests (D-07 per-test gate), replace 4 WR-05 bare sleeps with poll-until-deadline (D-02), add corepc-node feature pin check CI job (D-08/D-09), correct "15 tests" → "8 tests" in ROADMAP/REQUIREMENTS (D-03)

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Protocol | v1.0 | 6/6 | Complete | 2026-04-09 |
| 2. Blame & Hardening | v1.0 | 3/3 | Complete | 2026-04-09 |
| 3. Client CLI | v1.0 | 2/2 | Complete | 2026-04-09 |
| 4. Discovery & Deployment | v1.0 | 3/3 | Complete | 2026-04-09 |
| 5. Tor & Release | v1.0 | 3/3 | Complete | 2026-04-09 |
| 6. CI/CD Security Pipeline | v1.1 | 1/1 | Complete | 2026-04-10 |
| 7. Coordinator DoS Hardening | v1.1 | 3/3 | Complete | 2026-04-10 |
| 8. Public-endpoint hardening | v1.2 | 4/4 | Complete | 2026-05-26 |
| 9. CI integration-test reliability | v1.3 | 5/5 | Complete   | 2026-05-27 |
| 10. full_round.rs decision + execution | v1.3 | 1/2 (10-02 partial: Tasks 1+2 of 3 — Task 3 blocked) | In Progress (blocked) |  |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
Full v1.1 details: `.planning/milestones/v1.1-ROADMAP.md`

### Phase 11: coordinator RSA pubkey encoding + full_round.rs unmute completion

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 10
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd-plan-phase 11 to break down)
