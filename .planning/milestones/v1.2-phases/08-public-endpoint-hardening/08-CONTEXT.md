# Phase 8: Public-endpoint hardening - Context

**Gathered:** 2026-05-26
**Status:** Ready for planning
**Source:** Promoted from BACKLOG.md B-01 on 2026-05-26

<domain>
## Phase Boundary

The coordinator HTTP API resists volume-based denial-of-service when exposed publicly:

- `/round/register_input` and `/round/sign` cannot be flooded faster than global per-route rate limits permit (returns HTTP 429).
- Slow clients cannot tie up request slots indefinitely (per-route timeouts).
- Concurrent connection counts at the listener are bounded.
- All limits are operator-tunable via `coordinator.toml`.

**Explicitly NOT in scope:**
- **Per-peer throttling on Tor.** Tor hidden-service streams have no client identity by design (see [code_context](#code_context)). Any "per-peer" rate limit would be impossible to enforce without breaking anonymity. Drop.
- **Sybil prevention.** Rate limiting doesn't prevent sybil attacks — adversaries trivially create new identities. The project's sybil resistance is **BIP-322 ownership proofs**: every input registration requires proving control of a real UTXO, which is the per-input cost an attacker must pay. Phase 8 does not add new sybil tech; this distinction must be explicit in the PR description so the project doesn't overclaim.
- **Per-IP throttling for clearnet mode.** Clearnet mode is dev/test only (production is `tor_mode = true` per project config). Marginal value, deferred.

</domain>

<decisions>
## Implementation Decisions

### Threat framing (D-01)
- **D-01:** Phase 8 is **DoS mitigation only**. Success criteria must explicitly state: "Coordinator resists volume-based DoS via global per-route rate limits, connection caps, and timeouts. Per-peer throttling is impossible on Tor by design; sybil resistance is BIP-322 ownership proofs (unchanged), not rate limits." The PR description must repeat this framing so reviewers don't conflate "DoS mitigation" with "sybil prevention."

### Rate-limit budget (D-02)
- **D-02:** Tight on writes, generous on reads. Starting defaults to encode in `CoordinatorConfig::with_defaults()`:
  - `/info`: **60 req/min** global (clients poll for round phase)
  - `/round/register_input`: **30 req/min** global
  - `/round/sign`: **30 req/min** global
  - `/round/output`: **30 req/min** global
  - `/round/tx`: **60 req/min** global
- These are starting points; the planner should justify in PLAN.md but doesn't need new research. All numbers are operator-tunable per D-04.

### Rejection mode (D-03)
- **D-03:** Return **HTTP 429 with `Retry-After` header**. Standard HTTP semantics, `tower-governor` default, clients can implement exponential backoff. The 429 is not a deanonymization vector — every Tor client gets the same error shape.

### Operator-tunable knobs (D-04)
- **D-04:** Add `[coordinator]` config fields:
  - `rate_limit_info_per_min: u32` (default 60)
  - `rate_limit_writes_per_min: u32` (default 30) — applies to all write endpoints uniformly
  - `request_timeout_secs: u64` (default 30)
  - `max_concurrent_connections: u32` (default 256)
- All knobs follow the existing `BLINDJOIN__COORDINATOR__*` env-var override pattern.

### Crate selection (D-05 — Claude's discretion)
- **D-05:** Use **`tower-governor`** for per-route rate limiting (tokio/tower-native, axum-compatible). Use **`tower::timeout::TimeoutLayer`** for per-route timeouts. Use **`tower::limit::ConcurrencyLimitLayer`** for connection caps. Stay within the existing tower ecosystem; no new framework introductions.

### Test approach (D-06 — Claude's discretion)
- **D-06:** Integration test under `tests/integration/rate_limiting.rs` using the in-process `coordinator::run()` path (landed in commit `4a5c2b3`). Spawn coordinator, hammer one write endpoint past the configured limit, assert 429 + Retry-After. No bitcoind needed for the limit-breach itself; may need to stub or skip the round-state checks the handlers perform first. Planner to decide test structure.

### Claude's Discretion
- Exact crate version pins (planner picks current stable).
- Whether to refactor `middleware.rs` from stub to a `pub fn build_middleware_stack(cfg: &CoordinatorConfig) -> tower::ServiceBuilder<...>` factory, or apply layers directly in `mod.rs:51`. Either is acceptable; factory is more testable.
- 429 response body shape (just a header, or include a JSON `{"error":...}` body matching the project's existing error response convention).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Source-of-truth scoping
- [`.planning/BACKLOG.md`](../../BACKLOG.md) §B-01 — Original deferral, full scope and rationale.
- This file (`08-CONTEXT.md`) — locked decisions for this phase.

### Code to modify
- [`coordinator/src/api/middleware.rs`](../../../coordinator/src/api/middleware.rs) — Currently a 2-line stub. Becomes the home for rate-limit factory functions.
- [`coordinator/src/api/mod.rs:51`](../../../coordinator/src/api/mod.rs:51) — Router setup; per-route layer wiring goes here.
- [`coordinator/src/config.rs:13-32`](../../../coordinator/src/config.rs:13) — `CoordinatorSection` struct; D-04 adds 4 new fields.

### Code to read (not modify)
- [`coordinator/src/network/tor.rs`](../../../coordinator/src/network/tor.rs) — Tor hidden-service integration. Critical for understanding why per-peer throttling is impossible (lines 75-101: streams have no client identity, just `DataStream` from arti).
- [`coordinator/src/api/handlers.rs`](../../../coordinator/src/api/handlers.rs) — Existing route handlers; rate-limit layers wrap these.
- [`coordinator/src/run.rs`](../../../coordinator/src/run.rs) — Integration-test entry point; rate-limit test spawns this.
- [`tests/integration/round_bootstrap.rs`](../../../tests/integration/round_bootstrap.rs) — Reference for how to spawn `run()` in an integration test.

### Crate documentation (planner consults during research)
- `tower-governor` — per-route rate limiting (https://docs.rs/tower-governor).
- `tower::timeout::TimeoutLayer` — per-route timeouts (https://docs.rs/tower/latest/tower/timeout/).
- `tower::limit::ConcurrencyLimitLayer` — connection caps (https://docs.rs/tower/latest/tower/limit/).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`RequestBodyLimitLayer`** already applied at [`coordinator/src/api/mod.rs:51`](../../../coordinator/src/api/mod.rs:51) (64KB body cap). Pattern for stacking tower layers is established.
- **`coordinator::run()`** ([`coordinator/src/run.rs`](../../../coordinator/src/run.rs)) — production startup path callable from tests, landed today. Rate-limit integration test uses this.
- **Existing `CoordinatorSection` config pattern** ([`coordinator/src/config.rs:13`](../../../coordinator/src/config.rs:13)) — `#[serde(default = "fn_name")]` for optional fields with defaults. D-04 follows this pattern.
- **`BLINDJOIN__COORDINATOR__*` env-var convention** ([`coordinator/src/config.rs:84-95`](../../../coordinator/src/config.rs:84)) — double-underscore separator. New rate-limit env vars follow automatically once fields are added.

### Established Patterns
- **Tower layer stacking** — Already in use via `RequestBodyLimitLayer`. New layers compose the same way.
- **Per-route layers in axum 0.8** — `Router::route("...", handler.layer(...))` or `.route_layer()` at the router level. Planner picks the appropriate scope.
- **JSON error envelope** — Existing error responses use `{"error": {"code": "...", "message": "...", "round_id": "..."}}` ([`coordinator/src/api/handlers.rs`](../../../coordinator/src/api/handlers.rs)). 429 body should match this shape if a body is included.

### Integration Points
- **Router setup at [`coordinator/src/api/mod.rs:51`](../../../coordinator/src/api/mod.rs:51)** — Single place where new layers wire in.
- **Config struct at [`coordinator/src/config.rs:13`](../../../coordinator/src/config.rs:13)** — Single place where new fields go; default factory functions live below the struct definition.

### Critical constraint on Tor
- **[`coordinator/src/network/tor.rs:75-101`](../../../coordinator/src/network/tor.rs:75)** — Hidden-service streams are wrapped in `TokioIo<DataStream>` and served via `hyper::http1::serve_connection`. The `DataStream` carries no peer identity. This is **the architectural reason per-peer throttling is out of scope** — not a project decision, but a property of Tor itself. Any "identity-based throttling" goal from the original BACKLOG entry must be reframed as "global per-route."

</code_context>

<specifics>
## Specific Ideas

- Reviewer's recent escalation: "Highest remaining operational risk" — phase ships with the understanding that mainnet/public deployment is now unblocked once this lands.
- Reviewer language explicitly conflates DoS and sybil ("perform sybil attacks against the anonymity set"). The PR description for Phase 8 must distinguish these clearly so future reviewers don't repeat the conflation.

</specifics>

<deferred>
## Deferred Ideas

- **Per-IP throttling for clearnet mode.** ~30 min of additional `tower-governor` config to use the per-IP extractor when `tor_mode = false`. Marginal value — clearnet is dev/test only. Could fold in as a small follow-up if the planner finds it costs only a few lines.
- **Connection-level rate limits on Tor.** Counting requests per Tor stream/connection. Doesn't help (adversaries open more streams) but mentioned in the gray-area discussion; preserved here so it doesn't resurface.
- **Adaptive limits.** Limits that adjust based on round state (e.g., loosen during InputReg, tighten during Signing). Worth considering for a future phase if traffic profiling shows it matters.
- **Updating PROJECT.md line 87** to soften "Forward compatible with all address types" — that's B-02's job (BIP-322 multi-script), not Phase 8. Don't touch in this phase.

</deferred>

---

*Phase: 8-public-endpoint-hardening*
*Context gathered: 2026-05-26*
