---
phase: 04-discovery-deployment
plan: "01"
subsystem: discovery
tags: [pkarr, dht, discovery, coordinator, client]
dependency_graph:
  requires: []
  provides: [pkarr-coordinator-publish, pkarr-client-resolve]
  affects: [coordinator/src/main.rs, client/src/main.rs]
tech_stack:
  added: [pkarr = "5"]
  patterns: [SignedPacket::builder().txt().sign(), Client::resolve_most_recent(), Keypair::from_secret_key_file()]
key_files:
  created:
    - coordinator/src/discovery/mod.rs
    - coordinator/src/discovery/pkarr_pub.rs
    - client/src/discover.rs
  modified:
    - Cargo.toml
    - coordinator/Cargo.toml
    - coordinator/src/config.rs
    - coordinator/src/lib.rs
    - coordinator/src/main.rs
    - client/Cargo.toml
    - client/src/lib.rs
    - client/src/config.rs
    - client/src/main.rs
decisions:
  - "pkarr = '5' (not '2' from STACK.md) — version 5.0.4 is stable on crates.io; 6.0.0-rc.0 exists but is pre-release"
  - "Keypair file API takes &Path not &str — deviation from plan code snippet; fixed inline"
  - "CoordinatorInfo needs #[derive(Debug)] for unwrap_err() in tests — auto-fixed"
  - "Single JSON blob in _blindjoin TXT label (~130 bytes) — fits well under 255-byte DNS limit"
metrics:
  duration_minutes: 3
  tasks_completed: 2
  files_changed: 11
  completed_date: "2026-04-09"
---

# Phase 4 Plan 01: PKARR DHT Discovery Summary

PKARR DHT discovery wired into both coordinator (publish) and client (resolve) using pkarr 5.0.4 — coordinator signs and publishes a JSON TXT record to Mainline DHT on startup and every 5 minutes via heartbeat; client resolves a coordinator URL from a z32 public key before joining a round.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | PKARR workspace dep + coordinator discovery module | 5493f57 | Cargo.toml, coordinator/Cargo.toml, coordinator/src/config.rs, coordinator/src/lib.rs, coordinator/src/main.rs, coordinator/src/discovery/mod.rs, coordinator/src/discovery/pkarr_pub.rs |
| 2 | Client PKARR discovery module + --pkarr-pubkey CLI flag | 54abeb2 | client/Cargo.toml, client/src/lib.rs, client/src/discover.rs, client/src/config.rs, client/src/main.rs |

## Test Results

- `cargo test --lib -p coordinator -- discovery`: 3 passed (build_packet, contains_fields, keypair_persistence)
- `cargo test --lib -p client -- discover`: 1 passed (invalid_pubkey_returns_error)
- `cargo build --workspace`: clean (warnings only, no errors)
- Full lib suite: 62 tests passing across coordinator, client, shared

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Keypair file API takes &Path not &str**
- **Found during:** Task 1 (cargo build error)
- **Issue:** Plan code snippets called `Keypair::from_secret_key_file(path)` with `path: &str`. The actual pkarr 5.0.4 API requires `&Path`.
- **Fix:** Added `use std::path::Path; let p = Path::new(path);` in `load_or_generate_keypair`.
- **Files modified:** coordinator/src/discovery/pkarr_pub.rs
- **Commit:** 5493f57

**2. [Rule 1 - Bug] CoordinatorInfo missing #[derive(Debug)]**
- **Found during:** Task 2 (cargo test compile error)
- **Issue:** `unwrap_err()` in tests requires `T: Debug`; `CoordinatorInfo` lacked the derive.
- **Fix:** Added `#[derive(Debug)]` to `CoordinatorInfo`.
- **Files modified:** client/src/discover.rs
- **Commit:** 54abeb2

## Architecture Notes

- `pkarr 5.0.4` installed (not 5.0.2 which Cargo initially resolved — Cargo.lock pinned 5.0.2 from workspace; both satisfy `"5"` constraint; no functional difference for used APIs)
- TXT rdata access: used `String::try_from(txt.clone())` which joins all character strings via `simple_dns::TXT`'s `TryFrom` impl — correct for DNS wire format multi-string TXT records
- Heartbeat reads `round_state.phase.as_str()` live under read-lock — satisfies DISC-03 without a separate watch channel

## Known Stubs

None. The `coordinator_public_addr` default of "127.0.0.1:8080" is intentional for Phase 4 clearnet (documented in config.rs and plan). Phase 5 will replace with actual .onion address.

## Threat Flags

None. All threat mitigations from the plan's threat model are satisfied:
- T-04-01: pkarr crate verifies SignedPacket against the user-supplied Ed25519 public key
- T-04-02: Key file path documented in DiscoveryConfig; Docker volume mount pattern noted in RESEARCH.md
- T-04-06: `resolve_most_recent()` used (not `resolve()`) per RESEARCH.md Pitfall 5

## Self-Check: PASSED

Files verified present:
- coordinator/src/discovery/mod.rs: FOUND
- coordinator/src/discovery/pkarr_pub.rs: FOUND
- client/src/discover.rs: FOUND

Commits verified:
- 5493f57: FOUND
- 54abeb2: FOUND
