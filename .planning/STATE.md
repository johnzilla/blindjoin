---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 02-blame-hardening/02-01-PLAN.md
last_updated: "2026-04-09T13:31:49.384Z"
last_activity: 2026-04-09
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 9
  completed_plans: 7
  percent: 78
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-07)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 2 — Blame & Hardening

## Current Position

Phase: 2 (Blame & Hardening) — EXECUTING
Plan: 2 of 3
Status: Ready to execute
Last activity: 2026-04-09

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 6
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 6 | - | - |

**Recent Trend:**

- Last 5 plans: —
- Trend: —

*Updated after each plan completion*
| Phase 01-core-protocol P06 | 32172 | 2 tasks | 7 files |
| Phase 02-blame-hardening P01 | 123 | 2 tasks | 5 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Roadmap: Approach B (Prove-Then-Layer) — clearnet first, Tor in Phase 5
- Roadmap: Enum FSM (not typestate) for round state machine
- Roadmap: reqwest + thin RPC client for Bitcoin Core (not archived bitcoincore-rpc crate)
- Roadmap: bdk_wallet 1.0 (not deprecated bdk)
- Roadmap: BIP-322 Simple implemented directly (~50 lines rust-bitcoin, not bip322 crate)
- Roadmap: Liquidity bot deferred to Phase 4 (Sprint 4)
- Roadmap: Per-round ephemeral RSA-2048 keys with pre-commitment hash
- Roadmap: zeroize crate + ZeroizeOnDrop on all round-state structs
- Roadmap: Append-only ban file for ban list persistence across restarts
- Roadmap: HMAC-based session tokens for signing phase reconnection
- Roadmap: Polling at 5s intervals (Tor-safe, no persistent connections)
- Roadmap: Domain separator SHA-256("blindjoin-v1" || scriptPubKey || amount_le64)
- [Phase 01-core-protocol]: Integration test placed under coordinator crate: virtual workspaces cannot have [[test]] targets
- [Phase 01-core-protocol]: parse_address_to_script extended to include Regtest network in handlers.rs and signing.rs
- [Phase 01-core-protocol]: Coordinator pre-initialized in InputReg state for integration tests — no admin HTTP endpoint needed
- [Phase 02-blame-hardening]: Ban check placed at handler layer (not input_reg.rs logic layer) — consistent with how phase checks work in post_input
- [Phase 02-blame-hardening]: BanList stored in AppState not RoundStateInner — must survive round transitions and state zeroing

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-04-09T13:31:49.381Z
Stopped at: Completed 02-blame-hardening/02-01-PLAN.md
Resume file: None
