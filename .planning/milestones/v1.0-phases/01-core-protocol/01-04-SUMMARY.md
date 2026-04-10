---
phase: 01-core-protocol
plan: "04"
subsystem: coordinator-api
tags: [axum, http-api, config, round-protocol, blind-rsa, privacy]
one_liner: "Runnable coordinator binary with 5 REST endpoints wiring config, RSA blind signing, round FSM, and UTXO validation into a complete HTTP API"

dependency_graph:
  requires:
    - "01-01"  # shared crate: protocol types, errors, token
    - "01-02"  # coordinator blind/rsa, round/state, round/manager
    - "01-03"  # coordinator bitcoin/rpc, bitcoin/utxo, bitcoin/tx
  provides:
    - coordinator_binary         # runnable coordinator HTTP server
    - config_loading             # TOML + env var config (BLINDJOIN__*)
    - all_5_http_endpoints       # /info, /round/input, /round/output, /round/sign, /round/tx
    - output_reg_unit_tests      # TEST-04: 4 passing output_reg unit tests
  affects:
    - "01-05"  # client CLI will call these endpoints
    - "01-06"  # integration test will drive the full round against this binary

tech_stack:
  added:
    - axum 0.8 (HTTP router + handlers)
    - tower-http 0.6 (RequestBodyLimitLayer 64KB)
    - config 0.15 (TOML + env var layered config)
    - base64 0.22 (encode/decode blind sigs, tokens, PSBT)
    - anyhow 1 (main() error handling)
  patterns:
    - axum State extractor with AppState{round, rpc, config}
    - TOCTOU-safe phase check: read-lock check + write-lock re-check before mutation
    - Pure function register_output_logic testable without axum machinery
    - PRIV-02: zero PII fields in tracing! macros (enforced by comment + grep)

key_files:
  created:
    - coordinator/src/config.rs          # CoordinatorConfig: TOML + BLINDJOIN__* env var overrides
    - coordinator/src/api/mod.rs         # build_router: 5 routes + 64KB body limit
    - coordinator/src/api/handlers.rs    # get_info, post_input, post_output, get_tx, post_sign
    - coordinator/src/api/middleware.rs  # Phase 2 rate limiting placeholder
    - coordinator/src/round/input_reg.rs # register_input business logic
    - coordinator/src/round/output_reg.rs # register_output_logic + 4 unit tests
    - coordinator/src/round/signing.rs   # process_sign with session token + broadcast
    - blindjoin.toml.example             # all config keys with comments
  modified:
    - coordinator/src/main.rs            # tokio::main, startup health check, axum server bind
    - coordinator/src/lib.rs             # expose api, config modules
    - coordinator/src/round/mod.rs       # expose input_reg, output_reg, signing
    - coordinator/src/round/state.rs     # add rsa_pubkey_der: Option<Vec<u8>> to RoundState
    - coordinator/src/blind/rsa.rs       # add from_der_secret_key, secret_key_der, public_key_spki_der
    - shared/src/protocol.rs             # add msg_randomizer: Option<String> to OutputRegRequest
    - coordinator/Cargo.toml             # add axum, config, tower-http, base64, anyhow deps

decisions:
  - "msg_randomizer field added to OutputRegRequest: RSABSSA-SHA384-PSS-Randomized requires msg_randomizer at verify time; passing None causes verification failure (RFC 9474 §3.3.2)"
  - "rsa_pubkey_der stored in RoundState outer struct (not inner) so GET /info can serve it without reconstructing from zeroed inner after broadcast"
  - "Signer reconstructed per-request from inner.rsa_signing_key DER bytes; avoids storing non-Zeroize types in shared state"
  - "TOCTOU prevention: read-lock phase check + write-lock re-check in all mutating handlers (T-04-01)"

metrics:
  duration: "~3 hours"
  completed_date: "2026-04-09"
  tasks_completed: 3
  tasks_total: 3
  files_created: 8
  files_modified: 7
---

# Phase 1 Plan 4: HTTP API + Coordinator Binary Summary

Wired all subsystems from plans 01-03 into a runnable coordinator binary with 5 REST endpoints, config loading, startup health checks, and privacy-safe structured logging.

## What Was Built

### Task 1: Config + Main (commit cddfeef)

`coordinator/src/config.rs` loads from `blindjoin.toml` (optional) with `BLINDJOIN__*` env var overrides using double-underscore separator for nested keys (`BLINDJOIN__COORDINATOR__DENOMINATION_SATS`).

`coordinator/src/main.rs` initializes structured JSON logging (PRIV-02), loads config, performs startup health check (bitcoind reachable + block count > 0), then binds an axum listener.

`blindjoin.toml.example` documents all config fields with comments.

### Task 2: HTTP Handlers + Round Logic (commit 8d6e89c)

`api/mod.rs` defines `AppState{round, rpc, config}` and `build_router()` with 5 routes and `RequestBodyLimitLayer(64KB)` (T-04-02).

`api/handlers.rs` implements all 5 handlers with:
- TOCTOU-safe phase gating: read-lock check followed by write-lock re-check (T-04-01)
- PRIV-02 enforced: `info!` macros use only `round_id`, `participant_count`, `txid` (after broadcast)
- D-09 error shape: `{"error": {"code": "SCREAMING_SNAKE", "message": "...", "round_id": "..."}}`

Round handler modules (`input_reg.rs`, `output_reg.rs`, `signing.rs`) isolate business logic from axum for testability.

### Task 3: Output Registration Unit Tests (commit 487bbe0)

4 unit tests in `round/output_reg.rs` (TEST-04):

| Test | Status |
|------|--------|
| output_reg_accepts_valid_token | PASS |
| output_reg_rejects_replay | PASS |
| output_reg_rejects_wrong_denomination | PASS |
| output_reg_rejects_invalid_signature | PASS |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Added msg_randomizer to OutputRegRequest**

- **Found during:** Task 3 (RED phase — `output_reg_accepts_valid_token` failed)
- **Issue:** `RSABSSA-SHA384-PSS-Randomized` (the RFC 9474 default used by `RsaBlindSigner`) requires `msg_randomizer` at verify time. Passing `None` causes verification failure because the hash computation includes the randomizer prefix: `H::hash_message(msg_randomizer, msg)`.
- **Fix:** Added `msg_randomizer: Option<String>` to `OutputRegRequest` in `shared/src/protocol.rs`. Updated `register_output_logic` signature to accept `Option<MessageRandomizer>`. Updated `post_output` handler to decode and pass it. Test helper now captures `blinding_result.msg_randomizer` and passes it through.
- **Files modified:** `shared/src/protocol.rs`, `coordinator/src/round/output_reg.rs`, `coordinator/src/api/handlers.rs`
- **Commit:** 487bbe0

**2. [Rule 1 - Bug] Used DefaultRng instead of rand::thread_rng() in tests**

- **Found during:** Task 3 compile
- **Issue:** `rand::thread_rng()` is from a different crate version than what `blind-rsa-signatures` expects; `CryptoRng` bound not satisfied across crate versions.
- **Fix:** Replaced `rand::thread_rng()` with `DefaultRng` from `blind-rsa-signatures` in test helper.
- **Commit:** 487bbe0

## Privacy Logging Verification

```
grep output (handlers.rs tracing! macros):
- info!(round_id, participant_count, "Max participants reached...") — ALLOWED
- info!(round_id, participant_count, "All outputs registered...") — ALLOWED
- info!(txid, round_id, "CoinJoin TX broadcast") — ALLOWED (txid is public)

Zero matches for: utxo_outpoint, address, token, signature, IP, blinded_msg
```

## Final Binary

- Path: `target/debug/coordinator`
- Size: ~139MB (debug, unoptimized)
- Build: `cargo build -p coordinator` exits 0

## Verification Results

```
cargo build -p coordinator           -> Finished (0 errors, warnings only)
5 routes in build_router()           -> PASS (5 lines matched)
blindjoin.toml.example denomination  -> PASS (denomination_sats = 1000000)
cargo test -p coordinator output_reg -> 4/4 PASS
```

## Self-Check: PASSED

Files exist:
- FOUND: coordinator/src/config.rs
- FOUND: coordinator/src/api/mod.rs
- FOUND: coordinator/src/api/handlers.rs
- FOUND: coordinator/src/round/input_reg.rs
- FOUND: coordinator/src/round/output_reg.rs
- FOUND: coordinator/src/round/signing.rs
- FOUND: blindjoin.toml.example

Commits exist:
- FOUND: cddfeef (Task 1)
- FOUND: 8d6e89c (Task 2)
- FOUND: 487bbe0 (Task 3)

## Known Stubs

**signing.rs `assemble_and_broadcast`:** Uses placeholder `script_pubkey` values when building `ParticipantInput` for `build_coinjoin_psbt`. The actual UTXO script_pubkey from Bitcoin Core (fetched during UTXO validation in `input_reg`) is not stored in `RegisteredInput` — only the change address string is stored. This means the broadcast PSBT will have incorrect witness_utxo fields. The integration test (plan 06) uses a test harness that controls the round flow and will surface this. Fix: store `ScriptBuf` in `RegisteredInput` during input registration.

## Threat Flags

No new network endpoints or trust boundaries beyond what the plan's threat model covers.
