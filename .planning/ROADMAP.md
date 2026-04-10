# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- **v1.1 Security & Availability Hardening** — Phases 6-7

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-04-09</summary>

- [x] Phase 1: Core Protocol (6/6 plans) — completed 2026-04-09
- [x] Phase 2: Blame & Hardening (3/3 plans) — completed 2026-04-09
- [x] Phase 3: Client CLI (2/2 plans) — completed 2026-04-09
- [x] Phase 4: Discovery & Deployment (3/3 plans) — completed 2026-04-09
- [x] Phase 5: Tor & Release (3/3 plans) — completed 2026-04-09

</details>

**v1.1 Security & Availability Hardening**

- [x] **Phase 6: CI/CD Security Pipeline** - Cargo test, audit, and clippy gates run on every pull request (completed 2026-04-10)
- [ ] **Phase 7: Coordinator DoS Hardening** - Async RPC and key parse moved outside the write lock

## Phase Details

### Phase 6: CI/CD Security Pipeline
**Goal**: Every pull request is blocked from merging if tests fail, a known CVE is present in dependencies, or clippy warnings exist
**Depends on**: Nothing (CI configuration; no runtime dependencies)
**Requirements**: CICD-01, CICD-02, CICD-03, CICD-04
**Success Criteria** (what must be TRUE):
  1. Opening or updating a pull request automatically triggers a CI run with test, audit, and clippy jobs
  2. A PR with a failing `cargo test --workspace` cannot be merged — CI status is required
  3. A PR with a `cargo audit`-detected CVE in the dependency tree fails CI and cannot be merged
  4. A PR with any `cargo clippy --workspace -- -D warnings` warning fails CI and cannot be merged
**Plans**: 1 plan

Plans:
- [x] 06-01-PLAN.md — Create ci.yml PR gate, add check prereqs to release.yml and docker.yml, document branch protection setup

### Phase 7: Coordinator DoS Hardening
**Goal**: Input registration handlers cannot serialize concurrent participants behind each other's RPC latency or key deserialization cost
**Depends on**: Phase 6 (all coordinator changes go through CI gate)
**Requirements**: AVAIL-01, AVAIL-02
**Success Criteria** (what must be TRUE):
  1. The bitcoind RPC call in `post_input` completes before the `RoundState` write lock is acquired
  2. A slow or hung bitcoind response does not block other participants from registering inputs concurrently
  3. `RsaBlindSigner` is constructed once when a round is created and reused by all subsequent handler calls
  4. RSA key deserialization does not appear in the per-request hot path
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Protocol | v1.0 | 6/6 | Complete | 2026-04-09 |
| 2. Blame & Hardening | v1.0 | 3/3 | Complete | 2026-04-09 |
| 3. Client CLI | v1.0 | 2/2 | Complete | 2026-04-09 |
| 4. Discovery & Deployment | v1.0 | 3/3 | Complete | 2026-04-09 |
| 5. Tor & Release | v1.0 | 3/3 | Complete | 2026-04-09 |
| 6. CI/CD Security Pipeline | v1.1 | 1/1 | Complete   | 2026-04-10 |
| 7. Coordinator DoS Hardening | v1.1 | 0/? | Not started | - |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
