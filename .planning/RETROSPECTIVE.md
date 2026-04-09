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

## Cross-Milestone Trends

### Process Evolution

| Milestone | Timeline | Phases | Key Change |
|-----------|----------|--------|------------|
| v1.0 | 3 days | 5 | Initial milestone — established Prove-Then-Layer pattern |

### Cumulative Quality

| Milestone | Rust LOC | Plans | Requirements |
|-----------|----------|-------|-------------|
| v1.0 | 7,353 | 17 | 52 (51 checked) |

### Top Lessons (Verified Across Milestones)

1. Verify crate APIs exist before writing plans that depend on them
2. Sequential execution without worktrees is simpler and more reliable for solo builders
