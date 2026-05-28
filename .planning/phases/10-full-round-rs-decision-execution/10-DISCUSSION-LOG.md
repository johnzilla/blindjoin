# Phase 10: full_round.rs decision + execution - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `10-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-05-27
**Phase:** 10-full-round-rs-decision-execution
**Areas discussed:** Repair-vs-retire scope, Repair approach, REPAIR-02 enforcement, Coverage rescue

---

## Repair-vs-retire scope policy

### Q1: Scope policy for the 6 ignored tests?

| Option | Description | Selected |
|--------|-------------|----------|
| Repair all 6 | Each covers unique multi-client e2e scenario unit tests can't replicate. Fix is mechanical. | ✓ |
| Tiered: repair 4, retire 2 | Retire the 2 most-redundant (replay_token, wrong_denomination — partial unit coverage). | |
| Retire all 6 | Delete full_round.rs entirely. Lowest cost but creates coverage gap. | |
| Per-test decision | Defer to plan-phase, 6 sub-questions. | |

**User's choice:** Repair all 6

### Q2: Fold WR-05 (4 bare-sleep sites) into repair plans?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — fold in | Sleep sites live inside tests getting repaired; replace with poll-until-deadline while touching the code. | ✓ |
| No — separate Phase 11 item | Cleaner scope separation but two passes over same code. | |

**User's choice:** Yes — fold in

### Q3: Correct ROADMAP/REQ "15 tests" → "8 tests"?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — phase deliverable | Fix while in the codebase. | ✓ |
| No — separate docs-only quick task | Cleaner separation but might never happen. | |

**User's choice:** Yes — phase deliverable

### Q4: Plan grouping?

| Option | Description | Selected |
|--------|-------------|----------|
| One plan per test (6 plans) | Max isolation. | |
| Grouped by drift type (2 plans) | 10-01 schema port, 10-02 unmute+sleeps. Narrative-driven. | ✓ |
| All-in-one (1 plan) | Largest commit, hardest to review. | |
| Functional groups (3 plans) | 10-01 schema port, 10-02 adversarial trio, 10-03 round-flow trio. | |

**User's choice:** Grouped by drift type (2 plans)

---

## Repair approach

### Q5: Which API approach for RPC-drift-affected calls?

| Option | Description | Selected |
|--------|-------------|----------|
| corepc-node 0.12 typed v30 Client | features=["30_2"] already pinned. Typed responses have v30+ shape. | ✓ |
| Direct reqwest + corepc-types | Matches production coordinator pattern. More code per site. | |
| Hybrid (typed for simple, reqwest for drifted) | Two RPC styles in one file. | |

**User's choice:** corepc-node 0.12 typed v30 Client

### Q6: Where does the new RPC plumbing live?

| Option | Description | Selected |
|--------|-------------|----------|
| Promote to mod.rs as shared helper | Matches Phase 9 consolidation pattern. Reusable by future tests. | ✓ |
| Keep in full_round.rs | Smallest diff. | |

**User's choice:** Promote to mod.rs

### Q7: Per-test acceptance bar?

| Option | Description | Selected |
|--------|-------------|----------|
| Local PASS + CI PASS (both required) | Same gate as Phase 9 UAT-1. Per-test atomic verification. | ✓ |
| Local PASS only | Faster iteration; CI is final gate at PR merge. | |
| Aggregate PASS (all 6 together at end) | Compounds risk; harder to localize failures. | |

**User's choice:** Local PASS + CI PASS

---

## REPAIR-02 enforcement mechanism

### Q8: Which enforcement mechanism?

| Option | Description | Selected |
|--------|-------------|----------|
| CI grep check in ci.yml | Tiny YAML diff. Matches Phase 9 gate-via-CI pattern. | ✓ |
| cargo-deny rule (deny.toml) | Standard tool but adds cargo-deny dep. | |
| Workspace-inherited dep declaration | Best long-term but requires refactoring existing decl. | |
| Docs-only (CONTRIBUTING.md note) | No enforcement; weakest. | |

**User's choice:** CI grep check

### Q9: Where in ci.yml does the grep check live?

| Option | Description | Selected |
|--------|-------------|----------|
| New tiny job ("corepc-node feature pin check") | Self-documenting failure name. Matches existing focused-job pattern. | ✓ |
| Inline step in cargo test job | Shares setup overhead but less distinct failure naming. | |
| Inline step in cargo clippy job | Mixes Rust-lint with TOML-grep. | |

**User's choice:** New tiny job

### Q10: Grep / failure semantics?

| Option | Description | Selected |
|--------|-------------|----------|
| Negative match: lines without features= | Tolerates minor bumps; catches only the actual violation. | ✓ |
| Explicit list: require features=["30_2"] | Stricter but bumps cost more. | |
| cargo metadata JSON instead of grep | Most robust to formatting variation; needs jq. | |

**User's choice:** Negative match

---

## Coverage rescue strategy

### Q11: Fallback if a specific repair attempt fails?

| Option | Description | Selected |
|--------|-------------|----------|
| Per-test escape valve: retire individually with TODO.md rationale | Phase ships green; documents the gap; preserves other 5 repairs. | ✓ |
| Phase blocks until all 6 pass | Strictest; risks block-on-one-test. | |
| Mark stuck test back to #[ignore] with new TODO marker | Preserves code for future retry; permanent reminder in tree. | |

**User's choice:** Per-test escape valve

### Q12: If any test retires, does a BACKLOG.md entry get filed?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — file a B-04+ entry per retired test | Matches Phase 8 BACKLOG B-01/B-02/B-03 pattern. Lost coverage tracked, not just dropped. | ✓ |
| No — TODO.md rationale is enough | Lighter-weight but easier to forget. | |

**User's choice:** Yes — B-04+ per retirement

---

## Claude's Discretion

- Exact poll-until-deadline implementation for WR-05 fixes (tokio::time::timeout vs explicit poll loop).
- Exact signature of the promoted `fund_regtest` helper (contract documented in D-06).
- Whether CI grep lives inline in ci.yml or extracts to `scripts/ci/check-corepc-node-pin.sh`.
- Whether to add a `tests/integration/mod.rs` doc-comment block above `fund_regtest` summarizing v30 schema gotchas.
- Per-test vs batch commit style for Plan 10-02 unmute commits.

## Deferred Ideas

- Tor-mode integration harness — v1.4+ (Phase 8 HUMAN-UAT item 3, deferred).
- Workspace dependency inheritance for corepc-node — defer until a second crate needs it.
- Direct reqwest + corepc-types port for tests — rejected as scope creep.
- cargo-deny adoption — revisit if multiple workspace-invariants emerge needing enforcement.
- Property-based testing via proptest — out of scope for Phase 10; candidate for BACKLOG if any test retires.
