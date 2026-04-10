# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MVP

**Shipped:** 2026-04-09
**Phases:** 5 | **Plans:** 17 | **Timeline:** 3 days (2026-04-07 → 2026-04-09)

### What Was Built
- Full CoinJoin coordinator with RSA blind signatures (RFC 9474) ensuring cryptographic input-output unlinkability
- Client CLI with bdk_wallet, per-phase Tor circuit isolation (alice/bob), and anti-censorship PSBT verification
- Blame protocol with non-signer detection, UTXO banning with persistence, and automatic round restart
- PKARR DHT discovery — coordinators publish .onion addresses, clients resolve without hardcoded addresses
- Tor v3 hidden service via arti-client — no clearnet endpoint in production
- Docker Compose stack (bitcoind + coordinator + liquidity bot) for zero-to-CoinJoin in 5 minutes
- GitHub Actions CI: cross-compiled binaries (4 targets) and multi-arch Docker images to ghcr.io

### What Worked
- **Prove-Then-Layer build order**: Protocol bugs and network bugs never entangled. Each phase was independently verifiable.
- **Coarse 5-phase roadmap**: Kept scope tight. No phase sprawl. Each phase had a clear, testable goal.
- **Sequential execution (no worktrees)**: Avoided file conflicts and dangling commits. Simpler mental model.
- **Code review + auto-fix pipeline**: Caught real issues (SOCKS5 listener leak, silent oneshot failure) and fixed them automatically.

### What Was Inefficient
- **SUMMARY.md one-liner extraction**: Many summaries had malformed one-liners (literal "One-liner:" text). The summary template or agent needs stronger enforcement.
- **DEPL-01 tracking artifact**: Docker Compose was delivered in Phase 4 but the REQUIREMENTS.md checkbox wasn't checked, creating a false "incomplete" signal at milestone close.
- **arti-client API discovery**: Plans assumed APIs (`launch_socks5_listener`, `ConnectedFlags::new_empty`) that don't exist in arti-client 0.41. Required runtime deviation handling in every Tor-related plan.

### Patterns Established
- Thin reqwest RPC client over corepc-types (not the archived bitcoincore-rpc crate)
- In-process SOCKS5 proxy pattern for bridging arti TorClient to reqwest
- cargo-chef multi-stage Dockerfiles for all workspace binaries
- Domain-separated blind tokens: SHA-256("blindjoin-v1" || scriptPubKey || amount_sats_le64)

### Key Lessons
1. **Pin arti-client to exact version** — the API surface changes significantly between minor releases. Plan tasks should reference specific method signatures, not assumed APIs.
2. **Check crate APIs at research time** — Phase 5 research didn't verify `launch_socks5_listener` existence. A 5-minute `cargo doc` check would have avoided the deviation.
3. **Requirements checkboxes need a completion gate** — the executor agent should verify REQUIREMENTS.md checkboxes match SUMMARY.md claims at phase completion time.

### Cost Observations
- Model mix: ~20% opus (planning, verification), ~80% sonnet (execution, code review)
- Notable: Code review + fix pass added ~10% overhead but caught 2 critical bugs (listener leak, silent send failure)

---

## Milestone: v1.1 — Security & Availability Hardening

**Shipped:** 2026-04-10
**Phases:** 2 | **Plans:** 4 | **Timeline:** 1 day (2026-04-09 → 2026-04-10)

### What Was Built
- CI/CD security pipeline: PR-triggered test/clippy/audit gates, release and Docker workflows gated on check prerequisites
- Supply-chain hardening: all GitHub Actions SHA-pinned, SHA-256 checksums on release archives, per-job permission scoping
- Coordinator DoS hardening: validate_utxo RPC moved before write lock (AVAIL-01), RsaBlindSigner cached per-round (AVAIL-02)
- Input validation: blinded token size bounds, address pre-validation, duplicate partial-sig guard, fee formula consolidation

### What Worked
- **Targeted discuss-phase**: Only 3 gray areas per phase — kept discussion fast and focused for well-defined hardening work.
- **Gap closure cycle**: Verification caught that 07-02 executor re-introduced the RSA deserialization bug. The gap closure plan (07-03) fixed it cleanly in one task.
- **Code review + fix pipeline**: Caught and auto-fixed 8 findings across both phases (SHA pinning, audit in releases, checksums, permission scoping, dup-sig, token bounds, address validation, fee duplication).
- **Inline execution for gap closure**: Small fix executed inline without subagent overhead — faster and cheaper.

### What Was Inefficient
- **Borrow checker deviations**: Both 07-01 and 07-02 executors hit Rust borrow conflicts not anticipated in the plan. The plan assumed a signer parameter pattern that conflicted with &mut state. Plan-time should verify borrow patterns with cargo check.
- **SUMMARY.md one-liners still broken**: The one_liner extraction from summaries still returns "One-liner:" literal text. Agents aren't populating the frontmatter field correctly.

### Patterns Established
- Validate-then-lock pattern for coordinator handlers with async I/O
- Cached parsed crypto objects in state structs (keep raw bytes for zeroize, parsed object for hot path)
- CI workflow structure: separate ci.yml for PRs + check prereqs in release/docker workflows

### Key Lessons
1. **Verify Rust borrow patterns at plan time** — if a plan passes &signer and &mut state to the same function, check that the borrow checker accepts it. A quick `cargo check` during planning saves a gap closure cycle.
2. **Gap closure works well** — the verify → plan-gaps → execute-gaps → re-verify cycle closed AVAIL-02 cleanly. Don't fear gaps; the system handles them.
3. **Code review fixes are high-value** — the 8 auto-fixes improved supply-chain security, input validation, and code quality with minimal effort.

### Cost Observations
- Model mix: ~15% opus (planning), ~85% sonnet (execution, review, verification)
- Notable: Gap closure (07-03) added 1 extra plan but caught a real regression — worth the cost

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Timeline | Phases | Key Change |
|-----------|----------|--------|------------|
| v1.0 | 3 days | 5 | Initial milestone — established Prove-Then-Layer pattern |
| v1.1 | 1 day | 2 | Hardening milestone — established gap closure cycle and code review fix pipeline |

### Cumulative Quality

| Milestone | Rust LOC | Plans | Requirements |
|-----------|----------|-------|-------------|
| v1.0 | 7,353 | 17 | 52 (51 checked) |
| v1.1 | 5,918 | 4 | 6 (6 checked) |

### Top Lessons (Verified Across Milestones)

1. Verify crate APIs exist before writing plans that depend on them
2. Sequential execution without worktrees is simpler and more reliable for solo builders
3. Verify Rust borrow patterns at plan time — cargo check during planning prevents gap closure cycles
4. Code review + auto-fix pipeline catches real issues with minimal overhead (~10%)
