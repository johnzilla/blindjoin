# Phase 8 Discussion Log

**Discussed:** 2026-05-26
**Mode:** Single-pass batched (4 questions in one AskUserQuestion call)
**Source:** Phase scaffolded from BACKLOG.md B-01; pre-identified 5 gray areas in scaffold CONTEXT.md

## Pre-discussion investigation (Claude)

**Question 2 from scaffold ("arti-axum peer-identity exposure") resolved by code reading, not by asking user:**

Read [`coordinator/src/network/tor.rs`](../../../coordinator/src/network/tor.rs) end-to-end. Key findings:
- Project does NOT use the community `arti-axum` crate — it manually bridges `arti::DataStream` → `hyper` via `TokioIo` and `TowerToHyperService`.
- More importantly: Tor hidden-service streams carry no client identity. The `DataStream` from `stream_req.accept()` has no peer address, no `.onion` identifier, no client public key. Clients are anonymous by design — that's the whole point of Tor.
- Conclusion: **per-peer throttling on Tor is fundamentally impossible**, not just hard. This collapses 5 questions to 4 and reframes the phase from "identity-aware throttling" to "global rate limiting."

## Questions asked

### Q1: Threat framing — given per-peer throttling is impossible, how to frame success criteria?

**Options presented:**
1. DoS mitigation only — explicit (Recommended)
2. DoS + clearnet per-IP (if tor_mode=false)
3. Add stream-level per-connection limits on Tor

**User selected:** Option 1 (DoS mitigation only — explicit)

**Notes:** Honest framing wins. Sybil resistance is BIP-322 ownership proofs, not rate limits. PR description must make this distinction explicit so reviewers don't conflate the two. Clearnet per-IP becomes a deferred idea.

### Q2: Per-route rate limit budget

**Options presented:**
1. Tight on writes, generous on reads (Recommended) — /info 60, writes 30
2. Uniform tight limits — 10 req/min global on everything
3. Defer numbers to plan-phase

**User selected:** Option 1 (Tight on writes, generous on reads)

**Notes:** /info polling is high-frequency by design; writes are bounded by realistic round participation (~3-20 participants/round). Numbers are starting defaults; operator can tune via D-04.

### Q3: Rejection mode for excess requests

**Options presented:**
1. HTTP 429 with Retry-After header (Recommended)
2. Silent timing-stable drop
3. 503 Service Unavailable

**User selected:** Option 1 (HTTP 429 with Retry-After)

**Notes:** Standard HTTP semantics, tower-governor default. The 429 is not a deanonymization vector on Tor. Silent drops would hurt legitimate clients with no actionable error.

### Q4: Operator-tunable knobs

**Options presented:**
1. All limits + timeouts (Recommended)
2. Just the rate limits (timeouts hardcoded)
3. Nothing tunable — ship with hardcoded defaults

**User selected:** Option 1 (All limits + timeouts)

**Notes:** Aligns with "disposable coordinator" design ethos — operators can tune for their traffic. 4 new config fields, all with `BLINDJOIN__COORDINATOR__*` env-var override per existing convention.

## Decisions locked

See [08-CONTEXT.md](08-CONTEXT.md) `<decisions>` block:
- D-01: DoS mitigation framing
- D-02: Per-route limits (60/30 split)
- D-03: HTTP 429 + Retry-After
- D-04: All 4 knobs operator-tunable
- D-05 (Claude's discretion): `tower-governor` + `tower::timeout` + `tower::limit::ConcurrencyLimitLayer`
- D-06 (Claude's discretion): Integration test via `coordinator::run()`

## Deferred (captured, not acted on)

- Per-IP throttling for clearnet mode
- Connection-level rate limits on Tor
- Adaptive limits based on round phase
- PROJECT.md line 87 update (belongs to B-02, not this phase)

## Scope creep avoided

None this session — the reviewer's "sybil attacks against the anonymity set" framing could have pulled scope into sybil prevention work, but Q1 explicitly resolved that as out-of-scope (sybil resistance is BIP-322, not rate limits).

---

*Phase: 8-public-endpoint-hardening*
*Logged: 2026-05-26*
