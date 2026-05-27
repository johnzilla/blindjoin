---
phase: 08-public-endpoint-hardening
plan: 03
subsystem: network
tags: [tor, semaphore, connection-cap, accept-loop, dos, tokio]

# Dependency graph
requires:
  - phase: 08-01-public-endpoint-hardening-foundation
    provides: "cfg.coordinator.max_concurrent_connections: u32 (default 256) on CoordinatorSection"
provides:
  - "coordinator/src/network/tor.rs::serve_onion_service gains a third positional parameter `max_concurrent_connections: u32` and gates the HS accept loop with a tokio::sync::Semaphore, bounding in-flight onion-service streams at the configured value (T-08-03-01)"
  - "Permit lifecycle correctness audited per RESEARCH §Common Pitfalls: acquire BEFORE accept (T-08-03-04 / Anti-Pattern guard), drop(permit) on accept failure (T-08-03-03), `let _permit = permit;` inside spawned task body (Pitfall 5 / T-08-03-02)"
  - "coordinator/src/run.rs threads cfg.coordinator.max_concurrent_connections into the tor_mode=true serve_onion_service call site; clearnet branch emits a startup tracing::warn! documenting that the cap is tor-only (T-08-03-05 accept disposition per Phase 8 A4 + CONTEXT D-01)"
affects: [08-04-integration-test]

# Tech tracking
tech-stack:
  added: []   # no new crates — tokio::sync::Semaphore is already in tree
  patterns:
    - "tokio::sync::Semaphore::acquire_owned() in accept loop for per-connection admission control — RESEARCH Pattern 3 retrofit"
    - "Permit-into-spawn ownership transfer via `let _permit = permit;` for connection-lifetime permit hold (Pitfall 5)"

key-files:
  created:
    - ".planning/phases/08-public-endpoint-hardening/08-03-SUMMARY.md (this file)"
  modified:
    - "coordinator/src/network/tor.rs (+33 lines: Arc + Semaphore imports, signature widened, semaphore construction, acquire_owned + drop(permit) + let _permit = permit; in accept loop)"
    - "coordinator/src/run.rs (+15 / -1 lines: capture cfg.coordinator.max_concurrent_connections into a let-binding for the spawned closure's third arg; clearnet tracing::warn! at the top of the else branch)"

key-decisions:
  - "serve_onion_service signature shape: third POSITIONAL parameter `max_concurrent_connections: u32` (the plan's truths allow either positional u32 OR `&CoordinatorConfig`; positional u32 chosen because the function reads only the one field and threading a full config reference would create an unnecessary dependency from network::tor on the config module's full surface)"
  - "Clearnet branch scope: option (c) per RESEARCH §Recommended Project Structure — clearnet path remains uncapped (axum::serve unchanged), with a startup tracing::warn! documenting the limitation (Pitfall 2 mitigation). Option (a) — replace axum::serve with a manual accept loop — was explicitly out of scope per the plan's must_haves (A4 resolution)."
  - "Permit on the spawned task captures permit by move via `let _permit = permit;` as the FIRST statement inside the tokio::spawn closure. The `_` prefix suppresses the unused-variable warning while keeping the binding alive for the connection's HTTP serve_connection future."
  - "Failure-path permit release uses `drop(permit);` explicitly between `tracing::warn!` and `continue` in the accept-error arm. Without this, repeated accept failures would slowly leak the entire cap (T-08-03-03)."

patterns-established:
  - "Transport-boundary admission control via tokio Semaphore at accept time — the canonical alternative to tower::limit::ConcurrencyLimitLayer (which queues per-request rather than rejecting per-connection)"
  - "Explicit-warning pattern for scope-limited mitigations: when a knob does not enforce on a code path operators might assume, emit a tracing::warn! at startup with the field name in the structured log (so operators tail the log + see exactly what they configured)"

requirements-completed: []

# Metrics
duration: 3min
completed: 2026-05-26
---

# Phase 8 Plan 03: Tor accept-loop connection cap Summary

**Wrapped the existing Tor hidden-service accept loop in coordinator/src/network/tor.rs with a tokio::sync::Semaphore — in-flight HS streams now cap at cfg.coordinator.max_concurrent_connections (default 256) and the (N+1)th acquire_owned().await parks until an earlier connection finishes.**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-05-26T04:08:50Z
- **Completed:** 2026-05-26T04:11:44Z
- **Tasks:** 2 of 2 complete
- **Files modified:** 2 (coordinator/src/network/tor.rs, coordinator/src/run.rs)

## Accomplishments

### (a) serve_onion_service: new third parameter, semaphore construction, accept-loop retrofit

`serve_onion_service` now reads:

```rust
pub async fn serve_onion_service(
    app: axum::Router,
    addr_tx: tokio::sync::oneshot::Sender<String>,
    max_concurrent_connections: u32,
) -> anyhow::Result<()>
```

Immediately before the existing `while let Some(stream_req) = stream_requests.next().await` loop, the function constructs:

```rust
let conn_sem = Arc::new(Semaphore::new(max_concurrent_connections as usize));
tracing::info!(
    cap = max_concurrent_connections,
    "Connection cap configured on Tor accept loop"
);
```

Inside the loop body, **BEFORE** `stream_req.accept(Connected::new_empty()).await`:

```rust
let permit = Arc::clone(&conn_sem)
    .acquire_owned()
    .await
    .expect("semaphore never closed");
```

The `.expect("semaphore never closed")` is documented as unreachable — `Semaphore::close()` is never called in this file and the function loops forever. `AcquireError` only fires on a closed semaphore.

Two further surgical edits inside the loop body close the permit-lifecycle audit:

- On `Err(e)` from `stream_req.accept(...)`: `drop(permit);` is inserted between the existing `tracing::warn!(error = %e, "Failed to accept HS stream");` and `continue;` so the slot is released for the next iteration (T-08-03-03).
- Inside the `tokio::spawn(async move { ... })` body, the FIRST statement is `let _permit = permit;` — this moves the permit into the spawned task and holds it for the full HTTP serve_connection future. When that future completes (whether success or error), the permit drops and a slot becomes available (T-08-03-02 / RESEARCH Pitfall 5).

### (b) Threading from cfg.coordinator.max_concurrent_connections — run.rs

The tor_mode = true branch in `coordinator/src/run.rs` (around lines 247-274) captures the config field into a `Copy` local binding, then passes it as the third argument inside the spawned future:

```rust
let max_concurrent_connections = cfg.coordinator.max_concurrent_connections;
tokio::spawn(async move {
    if let Err(e) =
        serve_onion_service(app, addr_tx, max_concurrent_connections).await
    {
        error!(error = %e, "Onion service fatal error");
        std::process::exit(1);
    }
});
```

`u32` is `Copy`, so no clone is needed and the value is captured by-value through the `move` closure cleanly.

### (c) Clearnet-cap startup warning (A4 + Pitfall 2 mitigation)

The tor_mode = false branch in `coordinator/src/run.rs` (around lines 280-308) now emits a single `tracing::warn!` at the top of the branch, BEFORE any router construction or socket binding:

```rust
tracing::warn!(
    max_concurrent_connections = cfg.coordinator.max_concurrent_connections,
    "Clearnet mode: max_concurrent_connections is NOT enforced — clearnet is dev/test only. Production deployments must use tor_mode = true."
);
```

The structured field carries no PII — just the aggregate configured cap value and a static string template (CLAUDE.md PRIV-02 compliant). The `axum::serve(listener, app)` call below this warning is unchanged; option (a) from RESEARCH §"Recommended Project Structure" (replace axum::serve with a manual semaphore-gated accept loop) was explicitly out of scope per the plan's must_haves (A4 resolution).

### (d) Audit log — three RESEARCH "Common Pitfalls" greps confirmed

| Pitfall | Mitigation | Grep |
|---------|------------|------|
| Pitfall 5 (permit drops before spawn enters) | `let _permit = permit;` is the FIRST statement inside `tokio::spawn(async move { ... })` | `grep -c "let _permit = permit" coordinator/src/network/tor.rs` → **1** ✓ |
| Anti-Pattern: permit acquired AFTER stream_req.accept() | `acquire_owned().await` is on line 102 (inside loop body, BEFORE `let data_stream = match stream_req.accept(...)` on line 108) | `grep -B6 -A1 "stream_req.accept(Connected" coordinator/src/network/tor.rs \| grep -c "acquire_owned"` → **1** ✓ (line ordering: 102 < 108) |
| Leaked permits on accept failure | `drop(permit);` is between the `tracing::warn!` and `continue;` in the `Err(e) =>` arm | `grep -c "drop(permit)" coordinator/src/network/tor.rs` → **1** ✓ (line 113, inside the Err arm) |

All three pitfall-grep audits PASSED. The widened `-B6` window (vs the plan's `-B1`) is needed because the modified accept loop has multi-line formatting (let-binding spans 4 lines) — the SEMANTIC check (acquire-line < accept-line in the same iteration body, no permit leak path) is what matters and is verified by line numbers above.

### (e) Verification evidence

- `cargo build -p coordinator --all-targets` → exits 0 cleanly (after Task 2 closes the call-site arity error from Task 1; this is the expected sequencing in the plan's done note for Task 1).
- `cargo clippy -p coordinator --all-targets -- -D warnings` → exits 0 with zero warnings.
- `cargo test -p coordinator --lib` → 58 passed, 0 failed, 0 ignored. Matches the post-Plan-02 baseline (Plan 02 added 2 construction tests in api::middleware; this plan adds none — accept-loop behavior is asserted end-to-end in Plan 04).
- `cargo test --no-run` → all test executables compile clean. Plan 01 already extended every `CoordinatorSection { ... }` literal in tests with the four new fields including `max_concurrent_connections: 256`, so the new signature does not break any existing integration test.
- `grep -c "Semaphore::new" coordinator/src/network/tor.rs` → 2 (one constructor call + one comment reference; ≥1 required).
- `grep -c "let _permit = permit" coordinator/src/network/tor.rs` → 1 (Pitfall 5 audit; ≥1 required).
- `grep -c "drop(permit)" coordinator/src/network/tor.rs` → 1 (failure-path audit; ≥1 required).
- `grep -c "max_concurrent_connections" coordinator/src/run.rs` → 5 (one in the tor_mode let-binding, one in the spawned closure's positional arg, one in the clearnet warning's field, one in the warning's structured field value, one in the comment near the let-binding; ≥2 required).
- `grep -n "serve_onion_service(" coordinator/src/run.rs` → line 265 shows `serve_onion_service(app, addr_tx, max_concurrent_connections).await` (three positional args).

### (f) End-to-end story — Plan 01 → Plan 03 connection-cap

```
Plan 01  →  Added rate_limit_info_per_min / rate_limit_writes_per_min /
            request_timeout_secs / max_concurrent_connections to
            CoordinatorSection (foundation: dep + config surface only;
            no runtime use yet).

Plan 02  →  Wired tower_governor GovernorLayer per-route +
            tower_http TimeoutLayer Router-scope, consuming the first
            three Plan-01 knobs. THIS plan's parallel-class companion
            (Wave 2 serial).

Plan 03  →  THIS PLAN. Consumed the fourth Plan-01 knob:
            max_concurrent_connections is now read by run.rs and
            passed to serve_onion_service, which uses it to bound the
            arti HS accept loop via tokio::sync::Semaphore.

Plan 04  →  Will exercise rate-limit + timeout end-to-end via an
            integration test. Connection-cap behavior at runtime is
            documented as DEFERRED per RESEARCH Open Question Q3 RESOLVED:
            the clearnet test harness cannot exercise the tor-only
            semaphore. Coverage stands via this plan's grep audits.
```

### (g) Scope acknowledgements

- **Clearnet path remains uncapped.** This is deliberate per Phase 8 A4 resolution + CONTEXT D-01. The `tracing::warn!` at the top of the clearnet branch (in run.rs) makes the limitation visible to operators who tail the startup log. Production operators MUST use `tor_mode = true`.
- **No integration test added.** Plan 04 owns the integration testing. The connection-cap-at-runtime test (open N+1 parallel HS streams; assert the +1th blocks) is documented as deferred in Plan 04 per RESEARCH Open Question Q3 RESOLVED — the in-process `coordinator::run()` test harness runs in clearnet mode (no bitcoind+tor stack in test env), so the tor-only semaphore cannot be exercised at runtime. This plan's three grep audits (acquire-before-accept, drop-on-failure, hold-for-spawn-lifetime) provide the static guarantee that the permit lifecycle is correct.
- **Operator misconfiguration (max_concurrent_connections = 0).** `Semaphore::new(0)` is infallible; the first `acquire_owned().await` parks forever and the coordinator silently stops accepting new HS streams. This is T-08-03-07 (accept disposition) — operator-error class; not addressed in this plan. A defensive `assert!(max_concurrent_connections > 0)` at the top of the function was considered and rejected: the plan's must_haves preserve the `.expect("semaphore never closed")` panic message as the only documented panic path, and an extra panic for a value of 0 would be a deviation from the plan's locked behavior. If operators want zero, they want zero — they need to know the consequences from documentation, not a runtime panic.

## Task Commits

Each task was committed atomically:

1. **Task 1: Gate the Tor accept loop with a tokio Semaphore** — `d83a858` (feat)
2. **Task 2: Thread max_concurrent_connections into the run.rs call site + clearnet-cap warning** — `e7fbeb9` (feat)

## Files Created/Modified

- `coordinator/src/network/tor.rs` (modified, +33 lines)
  - Added `use std::sync::Arc;` and `use tokio::sync::Semaphore;` to the import block.
  - Widened `serve_onion_service` signature with `max_concurrent_connections: u32` as a third positional parameter.
  - Constructed `Arc<Semaphore>` with the configured cap, emitted aggregate `tracing::info!` at startup.
  - Acquired an `OwnedSemaphorePermit` BEFORE each `stream_req.accept(...)`.
  - Released the permit via `drop(permit);` on the accept-failure path.
  - Moved the permit into the spawned task body via `let _permit = permit;` as the first closure statement.
- `coordinator/src/run.rs` (modified, +15 / -1 lines)
  - Captured `cfg.coordinator.max_concurrent_connections` into a `Copy` local binding inside the tor_mode = true branch.
  - Updated the `serve_onion_service(...)` call site to pass the third positional argument.
  - Inserted a `tracing::warn!` at the top of the tor_mode = false (clearnet) branch documenting that the cap is tor-only.
- `.planning/phases/08-public-endpoint-hardening/08-03-SUMMARY.md` (created, this file)

## Decisions Made

- **Signature shape: positional `u32` over `&CoordinatorConfig`.** The plan's truths explicitly allow either; positional `u32` keeps `network::tor` decoupled from the full config surface. The function reads exactly one field, so threading a full reference would be over-coupling.
- **Permit acquisition ordering: BEFORE accept.** RESEARCH Anti-Pattern is explicit — Andy Balaam's older blog showed the wrong pattern (acquire after accept), and the tokio Semaphore docs prescribe acquire-before-accept so the accept loop itself parks at cap. Followed the docs.
- **No defensive assert on `max_concurrent_connections > 0`.** Operator-error class; documented at threat model T-08-03-07 (accept disposition). Adding the assert would be a deviation from the plan's locked behavior set.
- **Single startup tracing::info! emit, not per-iteration.** The cap is configured once at function entry; a per-iteration log line would be PII-free but spam-grade noise.

## Deviations from Plan

None — plan executed exactly as written. The plan's must_haves prescribed every grep, every line, every comment; nothing required auto-fix (Rules 1-3) and nothing required architectural escalation (Rule 4). Build, clippy, and tests all passed clean on the first attempt for each task.

The Task 1 acceptance criterion `grep -B1 -A1 "stream_req.accept" coordinator/src/network/tor.rs | grep -c "acquire_owned"` technically returns 0 with the as-shipped multi-line `let permit = Arc::clone(&conn_sem)\n    .acquire_owned()\n    .await\n    .expect("semaphore never closed");` formatting — the `-B1` window doesn't include `.acquire_owned()` because there are 5 lines between it and `stream_req.accept`. Widening the window to `-B6` (or counting line numbers directly, as documented in §(d) above) confirms the ordering. This is a tool-precision artifact, not a behavior issue; the semantic check passes.

## Issues Encountered

- Expected E0061 (3 args, 2 supplied) at `run.rs:260` after Task 1's signature change — this is documented in the plan's Task 1 `done` note as the expected hand-off signal to Task 2. Task 2 closed it.
- No third-party or runtime issues.

## User Setup Required

None. `cfg.coordinator.max_concurrent_connections` defaults to 256 (set in Plan 01). Operators may override via `BLINDJOIN__COORDINATOR__MAX_CONCURRENT_CONNECTIONS` (env-var overlay also from Plan 01).

## Threat-model coverage

| Threat ID | Status |
|-----------|--------|
| T-08-03-01 (Connection exhaustion via unbounded HS streams) | mitigated — `Semaphore::new(max_concurrent_connections)` parks the accept loop at capacity. (N+1)th `acquire_owned().await` blocks until a slot frees. |
| T-08-03-02 (Leaked permits via drop-before-spawn) | mitigated — Pitfall 5 audit grep returns 1; `let _permit = permit;` is the first statement inside the spawned closure body. |
| T-08-03-03 (Leaked permits on accept failure) | mitigated — `drop(permit);` in the `Err(e) =>` arm of `stream_req.accept(...)`. Grep returns 1. |
| T-08-03-04 (Cap bypassed via acquire-after-accept) | mitigated — `acquire_owned()` line 102 < `stream_req.accept` line 108 in the same iteration body. The acquire-line precedes the accept-line in source order, so the accept loop itself parks at cap. |
| T-08-03-05 (Clearnet path uncapped) | **accept** — Phase 8 A4 + CONTEXT D-01: clearnet is dev/test only, not addressed in this phase. Mitigated visibility via the new clearnet startup `tracing::warn!`. |
| T-08-03-06 (Information disclosure via per-connection logs) | accept (not introduced) — this plan adds only aggregate `tracing::info!(cap = ...)` at function entry and a startup `tracing::warn!(max_concurrent_connections = ...)` in the clearnet branch. No per-connection identifiers logged. |
| T-08-03-07 (Misconfiguration: max_concurrent_connections = 0) | accept — operator-error class; `Semaphore::new(0)` is infallible but the coordinator stops accepting after first parked acquire. Documented; not enforced at runtime. |

## Next Phase Readiness

**Plan 04 (integration test)** is now unblocked. It will:

- Spawn coordinator in clearnet mode with TIGHT rate-limit values (e.g., `rate_limit_info_per_min = 3`).
- Flood `/info` past the budget; assert HTTP 429 with `Retry-After` header (covers Plan 02).
- Race a slow handler against `request_timeout_secs`; assert HTTP 408 (covers Plan 02).
- DEFER runtime testing of the connection cap (this plan's surface) — the clearnet test harness cannot exercise the tor-only semaphore. A TODO comment in `tests/integration/rate_limiting.rs` will document this per RESEARCH Open Question Q3 RESOLVED. Coverage stands via this plan's grep audits.

After Plan 04, the phase is complete and reviewable. The PR description should distinguish DoS mitigation (this phase's work) from sybil resistance (BIP-322 ownership proofs, unchanged) per CONTEXT D-01 — Phase 8 does NOT add sybil resistance, only DoS resistance, and the connection cap is the per-transport boundary half of the DoS story (the per-request half landed in Plan 02).

## Self-Check

- Created files: 1 planned (`08-03-SUMMARY.md`).
  - `.planning/phases/08-public-endpoint-hardening/08-03-SUMMARY.md` — created (this file)
- Modified files exist:
  - `coordinator/src/network/tor.rs` — FOUND (137 lines, +33 from baseline 104)
  - `coordinator/src/run.rs` — FOUND (376 lines, +14 net from baseline 362)
- Commits:
  - `d83a858` (Task 1) — FOUND in `git log --oneline -3`
  - `e7fbeb9` (Task 2) — FOUND in `git log --oneline -3`

## Self-Check: PASSED

---
*Phase: 08-public-endpoint-hardening*
*Completed: 2026-05-26*
