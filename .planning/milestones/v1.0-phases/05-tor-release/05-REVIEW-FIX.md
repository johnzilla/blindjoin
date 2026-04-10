---
phase: 05-tor-release
fixed_at: 2026-04-09T21:06:57Z
review_path: .planning/phases/05-tor-release/05-REVIEW.md
iteration: 1
findings_in_scope: 7
fixed: 7
skipped: 0
status: all_fixed
---

# Phase 05: Code Review Fix Report

**Fixed at:** 2026-04-09T21:06:57Z
**Source review:** .planning/phases/05-tor-release/05-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 7 (CR-01, CR-02, WR-01 through WR-05)
- Fixed: 7
- Skipped: 0

Info findings (IN-01, IN-02, IN-03) were excluded per fix_scope: critical_warning.

---

## Fixed Issues

### CR-01: SOCKS5 proxy leak — listener orphaned when `TorHandle` is dropped

**Files modified:** `client/src/tor.rs`, `client/src/main.rs`
**Commit:** e31ab1f
**Applied fix:** Restructured `TorHandle` to start both SOCKS5 proxy tasks at construction time (`TorHandle::new`) and store the returned `JoinHandle`s as `_alice_task` / `_bob_task` fields. Added a `Drop` impl that calls `.abort()` on both handles, ensuring OS port allocations and `TorClient` handles are released when the `TorHandle` goes out of scope. Changed `launch_socks5_proxy` to return `(u16, JoinHandle<()>)` instead of just `u16`. Updated `alice_proxy_url` / `bob_proxy_url` from async methods (which spawned a new proxy each call) to synchronous `&str` getters. Updated call sites in `client/src/main.rs` accordingly.

---

### CR-02: `addr_tx.send()` return value silently discarded — onion address may never reach PKARR publisher

**Files modified:** `coordinator/src/network/tor.rs`
**Commit:** e29eca1
**Applied fix:** Replaced `let _ = addr_tx.send(onion_addr);` with a propagating call using `.map_err(|_| anyhow::anyhow!(...))?`. If the receiver was dropped before the send, the function now returns an error immediately rather than entering the accept loop in a zombie state (serving connections but with no PKARR record published).

---

### WR-01: `poll_until_phase` has no timeout — client hangs indefinitely

**Files modified:** `client/src/http.rs`, `client/src/main.rs`
**Commit:** f46b81a
**Applied fix:** Added a `max_wait: tokio::time::Duration` parameter to `poll_until_phase` and wrapped the poll loop in `tokio::time::timeout`. Returns `anyhow::anyhow!("Timed out waiting for phase: {expected_phase}")` on expiry. Updated all three call sites in `client/src/main.rs` to pass a 10-minute ceiling (`Duration::from_secs(600)`).

---

### WR-02: Signing timeout timer fires once and is never restarted — blame logic broken across rounds

**Files modified:** `coordinator/src/main.rs`
**Commit:** 877007c
**Applied fix:** Replaced the two one-shot `tokio::spawn` timer blocks (which only fired for the first round) with a single phase-monitor task that polls the round state every 500ms. The monitor tracks the last `round_id` for which it armed an output_reg timer and a signing timer respectively. When it sees a new `round_id` in `OutputReg` or `Signing` phase, it spawns a fresh timeout task for that specific round. The spawned timer also guards against stale fires by checking `round.round_id == round_id` before executing blame logic.

**Status:** fixed: requires human verification — the polling interval (500ms) means there is up to a 500ms delay before a new round's timer is armed after phase entry. This is acceptable given phase durations of 30-60s, but should be confirmed acceptable for production.

---

### WR-03: SOCKS5 handshake does not validate SOCKS version byte or CMD byte

**Files modified:** `client/src/tor.rs`
**Commit:** 1c6f438
**Applied fix:** Added an explicit check `if version != 0x05` after reading the greeting header, bailing with a descriptive error on mismatch. Added an explicit check `if cmd != 0x01` after reading the request header; on mismatch, sends the RFC 1928 "command not supported" response (`0x05 0x07 ...`) before bailing with a clear error message.

---

### WR-04: IPv6 SOCKS5 target formatted with brackets but arti `connect()` may not accept bracketed form

**Files modified:** `client/src/tor.rs`
**Commit:** 65736f8
**Applied fix:** Changed the IPv6 (`ATYP 0x04`) arm to use `std::net::Ipv6Addr::from(ipv6).to_string()` instead of `format!("[{addr}]")`. This produces a plain IPv6 address string (e.g. `::1`) without URI brackets, which is the form arti's address parser expects. The subsequent `format!("{target_host}:{port}")` then produces `::1:port` which is unambiguous for arti's internal parser.

---

### WR-05: Docker workflow pushes images on every `main` branch push, including non-release commits

**Files modified:** `.github/workflows/docker.yml`
**Commit:** 3f6df5d
**Applied fix:** Removed the `branches: [main]` trigger from the `on.push` block. Docker images are now only published when a versioned tag (`v*`) is pushed. Added a comment explaining the rationale and how to rebuild `:latest` manually if needed.

---

_Fixed: 2026-04-09T21:06:57Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
