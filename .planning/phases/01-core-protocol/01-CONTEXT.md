# Phase 1: Core Protocol - Context

**Gathered:** 2026-04-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Coordinator completes a real CoinJoin round on Bitcoin signet with clearnet TCP transport. This phase delivers: the round state machine, RSA blind signature engine, UTXO validation, transaction construction, coordinator HTTP API, shared protocol types, and unit tests. The output is a confirmed CoinJoin txid on signet.

No Tor, no PKARR discovery, no Docker, no liquidity bot in this phase.

</domain>

<decisions>
## Implementation Decisions

### Protocol Architecture (from eng review)
- **D-01:** Enum FSM for round state machine (not typestate). 6 states, <10 transitions. Arc<RwLock<RoundState>> for shared state in axum handlers.
- **D-02:** Per-round ephemeral RSA-2048 keys with pre-commitment. Coordinator publishes key hash in GET /info before accepting registrations. Clients verify key matches hash before blinding.
- **D-03:** Domain-separated blind token format: SHA-256("blindjoin-v1" || scriptPubKey_bytes || amount_sats_le64). Security-critical canonical serialization.
- **D-04:** BIP-322 Simple verification implemented directly (~50 lines using rust-bitcoin primitives). No bip322 crate dependency.
- **D-05:** HMAC-based session tokens for signing phase reconnection: HMAC(coordinator_round_secret, UTXO_outpoint). Deterministic, no storage needed.
- **D-06:** Shared protocol message types use serde with default behavior (allow unknown fields) for forward compatibility between coordinator/client versions.
- **D-07:** zeroize crate with ZeroizeOnDrop on all round-state structs. Memory zeroing is a design principle, not a post-hoc audit.

### Wire Protocol
- **D-08:** REST-style HTTP API (spec as-is): GET /info, POST /round/input, POST /round/output, POST /round/sign, GET /round/tx. Standard HTTP verbs + paths. Debuggable with curl.
- **D-09:** Structured JSON error responses: {"error": {"code": "UTXO_SPENT", "message": "...", "round_id": "..."}}. Machine-parseable error codes enable programmatic client retry/failover.
- **D-10:** Polling GET /info for phase transition detection. 5s intervals when Tor is added (Sprint 3); 1s acceptable for clearnet Sprint 1.

### Coordinator Operations
- **D-11:** Configuration via TOML file (blindjoin.toml) with BLINDJOIN_* environment variable overrides for any field. Standard for both Docker and bare-metal.
- **D-12:** Fail-fast startup health checks: verify bitcoind reachable, correct network (signet/testnet4/mainnet), synced (not IBD). Exit with clear error if any check fails.
- **D-13:** Thin reqwest-based Bitcoin RPC client (~100 lines, 5 methods: getrawtransaction, gettxout, sendrawtransaction, getblockcount, testmempoolaccept). Use corepc-types for type-safe request/response structs.

### Client Wallet
- **D-14:** bdk_wallet 1.0+ for client wallet operations. Generate new descriptor wallet by default (BIP-84 derivation), accept --descriptor flag for importing existing wallet.
- **D-15:** Manual signet faucet for first coins. No built-in faucet integration in Sprint 1.

### Round Parameters
- **D-16:** Use spec defaults: denomination 1,000,000 sats (0.01 BTC), min 3 participants, max 20, input reg timeout 60s, output reg timeout 60s, signing timeout 30s, ban duration 1 hour, fee rate 2 sat/vB. All configurable via blindjoin.toml.

### Claude's Discretion
- Error code taxonomy (specific error codes for each rejection type)
- Axum middleware configuration (rate limiting, request size limits)
- Internal data structures for round state (participant tracking, token registry)
- Logging format and verbosity levels (tracing crate configuration)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `blindjoin-technical-spec.md` — Full protocol spec: round phases, API endpoints, architecture, testing strategy. NOTE: some API fields are superseded by the design doc decisions (change_address in /round/input, utxo_outpoint in /round/sign).

### Design & Architecture
- `~/.gstack/projects/johnzilla-blindjoin/john-main-design-20260407-220513.md` — Approved design doc with Protocol Decisions section resolving API gaps. Status: APPROVED.
- `~/.gstack/projects/johnzilla-blindjoin/john-main-eng-review-test-plan-20260407-232603.md` — Test plan with all 34 codepaths identified.

### Research
- `.planning/research/STACK.md` — Technology recommendations with specific versions and rationale
- `.planning/research/FEATURES.md` — Feature landscape: table stakes vs differentiators
- `.planning/research/ARCHITECTURE.md` — Component boundaries and build order
- `.planning/research/PITFALLS.md` — 14 domain-specific pitfalls with prevention strategies
- `.planning/research/SUMMARY.md` — Synthesized findings

### External Standards
- RFC 9474 (RSA Blind Signatures) — the core cryptographic protocol
- BIP-322 (Generic Message Signing) — UTXO ownership proof format

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project. No existing code.

### Established Patterns
- Cargo workspace with coordinator/, client/, shared/ crates (from spec)
- axum + tokio for async HTTP (from stack research)
- serde + serde_json for all serialization (from stack research)

### Integration Points
- bitcoind (signet) via JSON-RPC over reqwest
- blind-rsa-signatures crate for RSA blind signing
- bdk_wallet for client PSBT signing

</code_context>

<specifics>
## Specific Ideas

- The coordinator's RSA public key hash should be returned in GET /info response alongside round state, denomination, and participant count
- The session token (HMAC) should be returned to the client during input registration in the response body, alongside the blind signature
- Change address is provided during input registration (linkable to input, documented and expected)
- Signing phase identifies by UTXO outpoint (not input index) to prevent cross-participant signature injection

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-core-protocol*
*Context gathered: 2026-04-08*
