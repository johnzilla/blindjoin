---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Completed 04-discovery-deployment/04-01-PLAN.md
last_updated: "2026-04-09T18:32:47.967Z"
last_activity: 2026-04-09
progress:
  total_phases: 5
  completed_phases: 3
  total_plans: 14
  completed_plans: 12
  percent: 86
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-07)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Phase 4 — Discovery & Deployment

## Current Position

Phase: 4 (Discovery & Deployment) — EXECUTING
Plan: 2 of 3
Status: Ready to execute
Last activity: 2026-04-09

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**

- Total plans completed: 11
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1 | 6 | - | - |
| 2 | 3 | - | - |
| 3 | 2 | - | - |

**Recent Trend:**

- Last 5 plans: —
- Trend: —

*Updated after each plan completion*
| Phase 01-core-protocol P06 | 32172 | 2 tasks | 7 files |
| Phase 02-blame-hardening P01 | 123 | 2 tasks | 5 files |
| Phase 02-blame-hardening P02 | 4 | 2 tasks | 7 files |
| Phase 02-blame-hardening P03 | 4 | 2 tasks | 3 files |
| Phase 03-client-cli P01 | 35 | 2 tasks | 8 files |
| Phase 03-client-cli P02 | 7 | 2 tasks | 2 files |
| Phase 04-discovery-deployment P01 | 3 | 2 tasks | 11 files |

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
- [Phase 02-blame-hardening]: on_signing_timeout and BlameOutcome placed in blame.rs not signing.rs — avoids import cycle, keeps blame logic co-located
- [Phase 02-blame-hardening]: build_router_with_ban_list() added alongside build_router() — integration tests unchanged, startup uses pre-loaded ban list
- [Phase 02-blame-hardening]: blame_round_count stored as Arc<AtomicU32> in AppState — shared between timer tasks without additional lock contention
- [Phase 02-blame-hardening]: Blame unit tests (TEST-07) placed in signing.rs test block using crate::round::blame imports — avoids import cycle, keeps blame logic co-located
- [Phase 02-blame-hardening]: OutputReg→Blame FSM transition added to can_transition_to — on_output_reg_timeout was silently failing without this edge
- [Phase 02-blame-hardening]: Integration blame test uses shared Arc<RwLock<BanList>> for direct assertion — faster than HTTP retry, tests same production ban path
- [Phase 03-client-cli]: peek_address(0) over next_unused_address in BdkClientWallet — single-use CLI wallet has no address reuse concern; avoids &mut self requirement on callers
- [Phase 03-client-cli]: check_psbt_denomination_outputs extracted as public fn — testable independently of async HTTP; CLI-04 anti-censorship check before signing
- [Phase 03-client-cli]: wif_key: Option<String> stored on BdkClientWallet — avoids fragile descriptor-string parsing to recover signing key for BIP-322 in input.rs
- [Phase 03-client-cli]: fund_regtest() helper extracted: shared bitcoind setup reduces copy-paste; spawn_coordinator_with_blame_and_restart resets round state via Arc<RwLock> after RestartWithout; adversarial_tampered_psbt_rejected pure in-memory so CLI-04 always runs in CI without bitcoind
- [Phase 04-discovery-deployment]: pkarr = '5' used (not '2' from STACK.md); version 5.0.4 stable on crates.io
- [Phase 04-discovery-deployment]: Single JSON blob in _blindjoin TXT label (~130 bytes) — fits under 255-byte DNS limit
- [Phase 04-discovery-deployment]: Heartbeat reads round_state.phase live under read-lock — satisfies DISC-03 without watch channel

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-04-09T18:32:47.964Z
Stopped at: Completed 04-discovery-deployment/04-01-PLAN.md
Resume file: None
