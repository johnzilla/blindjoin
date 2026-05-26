# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- ✅ **v1.1 Security & Availability Hardening** — Phases 6-7 (shipped 2026-04-10)
- **v1.2 Production Readiness** — Phase 8 (in progress)

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

**v1.2 Production Readiness** (in progress)

- [ ] **Phase 8: Public-endpoint hardening** — Rate limiting, timeouts, connection caps, and identity-aware throttling on the coordinator HTTP API (promoted from BACKLOG.md B-01)

**Goal:** The coordinator HTTP API resists volume-based denial-of-service when exposed publicly: `/round/input` and `/round/sign` cannot be flooded past global per-route rate limits (HTTP 429 + Retry-After); slow clients cannot tie up request slots indefinitely (per-route timeouts, HTTP 408); concurrent connection counts at the Tor listener are bounded; all limits are operator-tunable via `coordinator.toml`. Per-peer throttling is impossible on Tor by design (see CONTEXT D-01); sybil resistance is BIP-322 ownership proofs (unchanged), not rate limits.

**Plans:** 4 plans

Plans:
- [ ] 08-01-PLAN.md — Config foundation: add 4 new CoordinatorSection fields (D-04) with serde defaults; bump Cargo.toml (tower_governor 0.8 + tower-http "timeout" feature); update existing test literals (wave 1)
- [ ] 08-02-PLAN.md — Middleware factory + per-route wiring: implement middleware.rs build_rate_limit_layers (GlobalKeyExtractor, JSON envelope) + build_timeout_layer; wire into api/mod.rs via ServiceBuilder (wave 2, depends on 08-01)
- [ ] 08-03-PLAN.md — Connection cap on Tor accept loop: tokio::sync::Semaphore in network/tor.rs:75-101; run.rs call-site update + clearnet uncapped warning (wave 2, depends on 08-01)
- [ ] 08-04-PLAN.md — Integration test: tests/integration/rate_limiting.rs proving 429 + Retry-After + JSON envelope end-to-end via coordinator::run (wave 3, depends on 08-02 + 08-03)

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
| 8. Public-endpoint hardening | v1.2 | 0/4 | Ready to execute | — |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
Full v1.1 details: `.planning/milestones/v1.1-ROADMAP.md`
