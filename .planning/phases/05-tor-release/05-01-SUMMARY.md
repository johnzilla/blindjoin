---
phase: 05-tor-release
plan: 01
subsystem: infra
tags: [tor, arti-client, onion-service, axum, hyper, pkarr, rust]

# Dependency graph
requires:
  - phase: 04-discovery-deployment
    provides: PKARR publisher, build_router_with_ban_list, CoordinatorConfig, clearnet TcpListener+axum::serve pattern
provides:
  - coordinator/src/network/tor.rs: serve_onion_service() wrapping axum::Router over arti v3 onion service
  - tor_mode: bool config field in CoordinatorSection (default false, serde default)
  - main.rs branching: tor_mode → onion service path; clearnet → TcpListener path
  - PKARR publisher uses resolved public_addr (onion or clearnet) not hardcoded config field
affects:
  - 05-02 (client Tor integration — client side uses same arti-client version)
  - 05-03 (release CI — coordinator binary must link correctly with sqlite dep)

# Tech tracking
tech-stack:
  added:
    - arti-client 0.41 (features: onion-service-service, tokio, rustls)
    - tor-hsservice 0.41
    - tor-cell 0.41 (features: hs) — for Connected::new_empty() accept message
    - safelog 0.8 — for HsId::display_unredacted() (HsId doesn't impl Display)
    - hyper 1.x (features: http1, server) — HTTP/1.1 server for HS connections
    - hyper-util 0.1 (features: tokio, service) — TokioIo + TowerToHyperService adapter
    - futures-util 0.3 — StreamExt for RendRequest/StreamRequest stream processing
  patterns:
    - Onion service accept loop: handle_rend_requests → StreamRequest::accept(Connected::new_empty()) → TokioIo → http1::Builder::serve_connection(TowerToHyperService(app))
    - Oneshot channel pattern: tor.rs sends .onion addr to main.rs; main.rs awaits before PKARR publish
    - Tor/clearnet mutual exclusion: if cfg.coordinator.tor_mode { ... } else { ... } with no shared bind

key-files:
  created:
    - coordinator/src/network/mod.rs
    - coordinator/src/network/tor.rs
  modified:
    - coordinator/Cargo.toml
    - coordinator/src/config.rs
    - coordinator/src/lib.rs
    - coordinator/src/main.rs
    - .gitignore

key-decisions:
  - "HsId::display_unredacted() used for .onion string — HsId implements DisplayRedacted not Display"
  - "TowerToHyperService adapter required to bridge axum::Router (tower::Service) to hyper http1::Builder"
  - "tor-cell added as direct dep (not just transitive) for Connected::new_empty() in StreamRequest::accept"
  - "safelog 0.8 added as direct dep for HsId display_unredacted trait method"
  - "main task parked with std::future::pending() — both tor and clearnet servers run in spawned tasks"
  - "sqlite3 linker workaround: arti-client transitively pulls rusqlite; systems without sqlite-devel need SQLITE3_LIB_DIR symlink"

patterns-established:
  - "Arti onion service serve loop: handle_rend_requests + StreamRequest::accept + TowerToHyperService + http1::Builder"
  - "Oneshot channel for async address handoff between spawned service and main task"

requirements-completed: [PRIV-03]

# Metrics
duration: 61min
completed: 2026-04-09
---

# Phase 5 Plan 01: Tor Hidden Service Integration Summary

**Coordinator serves axum API over arti v3 onion service when tor_mode=true; TCP path unchanged for dev/test**

## Performance

- **Duration:** ~61 min
- **Started:** 2026-04-09T19:34:00Z
- **Completed:** 2026-04-09T20:34:58Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added arti-client 0.41 + tor-hsservice + supporting crates to coordinator
- Implemented `serve_onion_service()` in coordinator/src/network/tor.rs: bootstraps TorClient, launches onion service, accepts StreamRequests via hyper HTTP/1.1 server
- Wired tor_mode config field into main.rs: Tor path sends .onion addr via oneshot channel; clearnet path (Phase 4 compatible) unchanged
- PKARR publisher now uses the resolved `public_addr` for both initial publish and heartbeat — no more hardcoded `coordinator_public_addr` in tor_mode

## Task Commits

1. **Task 1: Add arti deps + tor_mode config + network/tor.rs** - `5327abf` (feat)
2. **Task 2: Wire tor_mode into main.rs** - `ce42674` (feat)

## Files Created/Modified

- `coordinator/src/network/mod.rs` — new: declares `pub mod tor`
- `coordinator/src/network/tor.rs` — new: `serve_onion_service(app, addr_tx)` implementation
- `coordinator/Cargo.toml` — arti-client 0.41, tor-hsservice, tor-cell, safelog, hyper 1.x, hyper-util added
- `coordinator/src/config.rs` — `tor_mode: bool` field with `#[serde(default)]` added to CoordinatorSection
- `coordinator/src/lib.rs` — `pub mod network` added
- `coordinator/src/main.rs` — tor_mode branch, public_addr resolution, PKARR publish updated, pending() park
- `.gitignore` — `.sqlite-lib/` added (temp sqlite symlink for builds without sqlite-devel)

## Decisions Made

- **HsId display:** `HsId::display_unredacted()` used instead of `.to_string()` — `HsId` implements `safelog::DisplayRedacted`, not `std::fmt::Display`. Required adding `safelog = "0.8"` as direct dep.
- **TowerToHyperService:** axum::Router cannot be passed directly to `hyper::server::conn::http1::Builder::serve_connection` — wrapped with `hyper_util::service::TowerToHyperService`.
- **tor-cell direct dep:** `Connected::new_empty()` is from `tor_cell::relaycell::msg::Connected`; tor-cell must be a direct dep since it is not re-exported by tor-hsservice.
- **main task parking:** Both server paths (Tor and clearnet) run in `tokio::spawn`. Main task parks via `std::future::pending()` rather than blocking on a single `await?` — keeps the startup flow clean.
- **sqlite linker workaround:** `arti-client` → `tor-dirmgr` → `rusqlite` requires `libsqlite3.so` at link time. On Fedora without `sqlite-devel` installed, a symlink `libsqlite3.so → libsqlite3.so.0` in `.sqlite-lib/` with `SQLITE3_NO_PKG_CONFIG=1 SQLITE3_LIB_DIR=.sqlite-lib` unblocks the build.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan's ConnectedFlags::new_empty() doesn't exist; actual type is Connected::new_empty()**
- **Found during:** Task 1 (network/tor.rs implementation)
- **Issue:** Plan specified `tor_hsservice::ConnectedFlags::new_empty()` but the actual tor-hsservice API uses `tor_cell::relaycell::msg::Connected` with constructor `Connected::new_empty()`
- **Fix:** Used `Connected::new_empty()` from `tor_cell::relaycell::msg` as shown by compiler error `note: consider using Connected::new_empty`
- **Files modified:** coordinator/src/network/tor.rs, coordinator/Cargo.toml (added tor-cell)
- **Verification:** cargo check passes clean
- **Committed in:** 5327abf (Task 1 commit)

**2. [Rule 1 - Bug] HsId doesn't implement Display; needs safelog::DisplayRedacted trait**
- **Found during:** Task 1 (network/tor.rs — getting .onion address as String)
- **Issue:** Plan used `.to_string()` on `HsId` return from `onion_address()` but HsId implements `safelog::DisplayRedacted` not `Display`
- **Fix:** Added `safelog = "0.8"` dep; used `hsid.display_unredacted().to_string()` pattern
- **Files modified:** coordinator/src/network/tor.rs, coordinator/Cargo.toml
- **Verification:** cargo check passes clean; onion address is full `.onion` domain
- **Committed in:** 5327abf (Task 1 commit)

**3. [Rule 1 - Bug] axum::Router not directly compatible with hyper http1::Builder; needs TowerToHyperService**
- **Found during:** Task 1 (network/tor.rs serve_connection call)
- **Issue:** E0277: `Router` doesn't implement `Service<Request<hyper::body::Incoming>>`
- **Fix:** Wrapped `app.clone()` with `TowerToHyperService::new()`; added `hyper` and `hyper-util service` feature as direct deps
- **Files modified:** coordinator/src/network/tor.rs, coordinator/Cargo.toml
- **Verification:** cargo check passes clean
- **Committed in:** 5327abf (Task 1 commit)

**4. [Rule 3 - Blocking] sqlite3 dev library not installed; arti-client transitively requires it**
- **Found during:** Task 2 (cargo build --release)
- **Issue:** `arti-client` → `tor-dirmgr` → `rusqlite` → `libsqlite3-sys` requires `libsqlite3.so` (unversioned symlink) which sqlite-devel package provides. Fedora system has `libsqlite3.so.0` but not `libsqlite3.so`.
- **Fix:** Created `.sqlite-lib/libsqlite3.so → /usr/lib64/libsqlite3.so.0` symlink; build with `SQLITE3_NO_PKG_CONFIG=1 SQLITE3_LIB_DIR=.sqlite-lib`. Added `.sqlite-lib/` to .gitignore.
- **Files modified:** .gitignore
- **Verification:** `cargo build --release -p coordinator` exits 0
- **Committed in:** ce42674 (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (3 API bugs from plan's pre-release arti API assumptions, 1 blocking env issue)
**Impact on plan:** All auto-fixes necessary for compilation correctness. No scope creep. The plan was written based on docs.rs API which matched arti 2.x blog posts, but the actual arti-client 0.41 API differs in type names.

## Issues Encountered

- `onion_name()` was deprecated in arti 0.41 — used `onion_address()` instead (compiler warning guided fix)
- `hyper` crate not transitively available as a name — required explicit direct dependency for `use hyper::server::conn::http1`
- `safelog` crate version is `0.8` not `0.41` — the arti sub-crates use independent versioning

## User Setup Required

None for this plan. Note: production deployment with `tor_mode = true` requires Tor network connectivity at coordinator startup. The `create_bootstrapped()` call will fail if the Tor network is unreachable. This is intentional — fail-fast rather than silently serving clearnet.

## Known Stubs

None. The implementation is fully wired: `serve_onion_service` is called in the tor_mode branch, onion address flows to PKARR publisher, and the accept loop correctly dispatches HTTP connections to the axum router.

## Next Phase Readiness

- Coordinator binary compiles and links with arti-client 0.41
- `tor_mode = false` default preserves all Phase 4 behavior (47 unit tests pass)
- `serve_onion_service` is ready to be exercised by Phase 5 Plan 2 (client Tor integration)
- The `.sqlite-lib` workaround should be documented in the Docker build for CI (Phase 5 Plan 3)

---
*Phase: 05-tor-release*
*Completed: 2026-04-09*
