---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: planning
stopped_at: Phase 1 context gathered
last_updated: "2026-04-08T14:20:35.769Z"
last_activity: 2026-04-07 — Roadmap created, requirements mapped to 5 phases
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-07)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 1 — Core Protocol

## Current Position

Phase: 1 of 5 (Core Protocol)
Plan: 0 of TBD in current phase
Status: Ready to plan
Last activity: 2026-04-07 — Roadmap created, requirements mapped to 5 phases

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**

- Last 5 plans: —
- Trend: —

*Updated after each plan completion*

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

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-04-08T14:20:35.766Z
Stopped at: Phase 1 context gathered
Resume file: .planning/phases/01-core-protocol/01-CONTEXT.md
