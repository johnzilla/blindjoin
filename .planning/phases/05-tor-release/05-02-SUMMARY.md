---
phase: "05-tor-release"
plan: "02"
subsystem: "client"
tags: ["tor", "circuit-isolation", "cli-05", "reqwest", "arti-client"]
dependency_graph:
  requires: ["05-01 (coordinator Tor HS — arti-client 0.41 API patterns)"]
  provides: ["--tor flag on client binary", "per-phase Tor circuit isolation (CLI-05)"]
  affects: ["client/src/tor.rs", "client/src/http.rs", "client/src/config.rs", "client/src/main.rs"]
tech_stack:
  added:
    - "arti-client 0.41 (onion-service-client + tokio + rustls) — client Tor bootstrapping and isolated_client()"
    - "tor-rtcompat 0.41 — PreferredRuntime type for explicit TorClient generic parameter"
    - "reqwest socks feature — SOCKS5 proxy support via Proxy::all(socks5h://...)"
  patterns:
    - "TorClient::isolated_client() for guaranteed circuit isolation between alice and bob phases"
    - "In-process SOCKS5 proxy (tokio::net::TcpListener + raw SOCKS5 protocol + TorClient::connect()) bridging arti and reqwest"
    - "CoordinatorClient::new_tor() with per-phase reqwest::Client instances configured via Proxy::all()"
key_files:
  created:
    - "client/src/tor.rs — TorHandle (alice/bob isolated clients), in-process SOCKS5 proxy, init_tor()"
  modified:
    - "client/src/http.rs — CoordinatorClient extended with new_tor(); post_output() uses bob circuit"
    - "client/src/config.rs — use_tor: bool field added (--tor / BLINDJOIN_USE_TOR)"
    - "client/src/main.rs — mod tor added; cfg.use_tor branch calling init_tor() + new_tor()"
    - "client/Cargo.toml — arti-client 0.41 + tor-rtcompat 0.41 added"
    - "Cargo.toml — reqwest socks feature added to workspace dep"
decisions:
  - "In-process SOCKS5 proxy using raw tokio::net::TcpListener + TorClient::connect() — arti-client 0.41 has no launch_socks5_listener(); implemented minimal RFC 1928 server (~80 lines) bridging arti to reqwest"
  - "tor-rtcompat added as direct dep — PreferredRuntime not re-exported by arti-client 0.41; explicit generic on struct fields requires direct import"
  - "reqwest socks feature added to workspace — Proxy::all(socks5h://...) requires this feature; coordinator unchanged since it uses raw hyper/arti streams"
metrics:
  duration_seconds: 298
  completed_date: "2026-04-09"
  tasks_completed: 2
  files_modified: 6
---

# Phase 05 Plan 02: Client Tor Circuit Isolation Summary

**One-liner:** Per-phase Tor circuit isolation via arti-client 0.41 `isolated_client()` + in-process SOCKS5 proxy bridging to reqwest, satisfying CLI-05.

## What Was Built

Added `--tor` flag to the blindjoin client binary. When set, input registration and output registration flow through two cryptographically isolated Tor circuits, preventing a network adversary from linking Alice (input reg) to Bob (output reg) by observing Tor exit traffic.

### Architecture

```
main.rs --tor branch
  → tor::init_tor(coordinator_url)
      → TorClient::create_bootstrapped()
      → base.isolated_client() → alice (TorClient)
      → base.isolated_client() → bob  (TorClient)
  → handle.alice_proxy_url()
      → TcpListener::bind(127.0.0.1:0)
      → tokio::spawn(socks5 accept loop using alice.connect())
      → "socks5h://127.0.0.1:{port}"
  → handle.bob_proxy_url()  (same, using bob.connect())
  → CoordinatorClient::new_tor(url, alice_proxy, bob_proxy)
      → reqwest::Client::builder().proxy(Proxy::all(alice_proxy))  → alice_client
      → reqwest::Client::builder().proxy(Proxy::all(bob_proxy))    → bob_client

CoordinatorClient routing:
  get_info()       → alice_client  (poll)
  post_input()     → alice_client  (input registration — Alice circuit)
  post_output()    → bob()         (output registration — Bob circuit, isolated)
  post_sign()      → alice_client
  get_tx()         → alice_client
  poll_until_phase → alice_client
```

### Isolation Guarantee

`TorClient::isolated_client()` creates a handle with a fresh `IsolationToken`. The arti circuit manager guarantees these handles share no guard nodes or circuits. A network adversary observing both circuits cannot correlate them to the same participant.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug / API unavailability] in-process SOCKS5 proxy instead of launch_socks5_listener()**
- **Found during:** Task 1
- **Issue:** `TorClient::launch_socks5_listener()` does not exist in arti-client 0.41. The plan noted this as a risk: "If the SOCKS5 approach is not available in 0.41, fall back to the IsolationToken + StreamPrefs approach."
- **Fix:** Implemented an in-process SOCKS5 proxy (RFC 1928 subset) using `tokio::net::TcpListener` + raw SOCKS5 protocol bytes + `TorClient::connect()` as the relay. This achieves the same reqwest integration (Proxy::all socks5h://...) without requiring a missing API. The isolation guarantee is preserved: each proxy instance holds a distinct `TorClient` handle obtained via `isolated_client()`.
- **Files modified:** `client/src/tor.rs` (launch_socks5_proxy + handle_socks5 functions)
- **Commits:** 52fb6e9

**2. [Rule 3 - Blocking] tor-rtcompat added as direct dep**
- **Found during:** Task 1 (cargo check)
- **Issue:** `PreferredRuntime` is not re-exported from `arti-client`; struct fields with explicit `TorClient<PreferredRuntime>` generic required `tor_rtcompat::PreferredRuntime` import, which needs a direct dep.
- **Fix:** Added `tor-rtcompat = "0.41"` to client/Cargo.toml. The crate is already pulled transitively by arti-client; adding it directly makes the import explicit and version-pinned.
- **Files modified:** `client/Cargo.toml`
- **Commits:** 52fb6e9

**3. [Rule 3 - Blocking] SQLITE3_LIB_DIR required for release build**
- **Found during:** Task 2 (cargo build --release)
- **Issue:** arti-client pulls rusqlite → libsqlite3-sys → linker needs `-lsqlite3`; the system has `libsqlite3.so.0` but no unversioned `libsqlite3.so` symlink. Same as Phase 05 Plan 01 (documented in STATE.md).
- **Fix:** Created `/tmp/sqlite3-lib/libsqlite3.so` symlink pointing to `/usr/lib64/libsqlite3.so.0`. Build runs with `SQLITE3_LIB_DIR=/tmp/sqlite3-lib`. This is a dev environment quirk; Docker images have the full `-dev` package.
- **Commit:** fd0cee4

## Verification Results

```
cargo build --release -p client     → Finished (with SQLITE3_LIB_DIR)
cargo build --release -p coordinator → Finished (no regressions)
cargo test -p client --lib           → 5 passed; 0 failed

grep bob_client|new_tor|post_output client/src/http.rs  → all present
grep use_tor|init_tor client/src/main.rs                → branch present
grep isolated_client|alice|bob client/src/tor.rs        → two isolated handles
```

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 | 52fb6e9 | feat(05-02): add arti-client dep and TorHandle in client/src/tor.rs |
| 2 | fd0cee4 | feat(05-02): wire TorHandle into http.rs new_tor constructor and main.rs --tor flag |

## Known Stubs

None. The `--tor` flag is fully wired end-to-end. When `--tor` is absent, the clearnet path is unchanged. The Tor path bootstraps arti, creates isolated circuits, and routes HTTP requests through per-phase proxies.

## Threat Flags

No new threat surface beyond what is in the plan's threat model. The SOCKS5 listeners bind to `127.0.0.1:0` (loopback only, ephemeral port) — T-05-08 mitigation confirmed implemented.

## Self-Check: PASSED

All files exist and both commits verified in git log.
