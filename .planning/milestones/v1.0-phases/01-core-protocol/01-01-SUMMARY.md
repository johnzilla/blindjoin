---
phase: 01-core-protocol
plan: "01"
subsystem: api
tags: [rust, cargo, serde, bitcoin, sha2, uuid, hex, blind-signatures]

# Dependency graph
requires: []
provides:
  - "Cargo workspace with coordinator, client, shared crates"
  - "shared::errors — ErrorCode enum (SCREAMING_SNAKE_CASE) + ApiError struct"
  - "shared::types — RoundId type alias, Denomination newtype"
  - "shared::protocol — 7 wire message structs + OwnershipProof with canonical hex helpers"
  - "shared::token — compute_blind_token_message(script, amount) -> [u8; 32]"
affects:
  - "02-blind-rsa (uses compute_blind_token_message from shared::token)"
  - "03-utxo (uses InputRegRequest, OwnershipProof from shared::protocol)"
  - "04-api-handlers (uses all protocol types, ApiError)"
  - "05-client (uses all protocol types, compute_blind_token_message)"
  - "06-state-machine (uses RoundId, shared error types)"

# Tech tracking
tech-stack:
  added:
    - "bitcoin 0.32 (rust-bitcoin primitives)"
    - "sha2 0.11 (SHA-256 digest)"
    - "serde + serde_json 1.x (JSON serialization)"
    - "uuid 1.x with v4+serde features"
    - "hex 0.4 (witness stack encoding)"
  patterns:
    - "Workspace Cargo.toml with [workspace.dependencies] for version pinning"
    - "No deny_unknown_fields on wire types (forward compat D-06)"
    - "Domain separator SHA-256(blindjoin-v1 || script_bytes || amount_le64)"
    - "OwnershipProof canonical wire type — JSON array of hex strings"
    - "ErrorCode SCREAMING_SNAKE_CASE + ApiError wrapped in {error:...} at axum layer"

key-files:
  created:
    - "Cargo.toml (workspace root)"
    - "shared/Cargo.toml"
    - "shared/src/lib.rs"
    - "shared/src/errors.rs"
    - "shared/src/types.rs"
    - "shared/src/protocol.rs"
    - "shared/src/token.rs"
    - "coordinator/Cargo.toml"
    - "coordinator/src/main.rs"
    - "client/Cargo.toml"
    - "client/src/main.rs"
  modified: []

key-decisions:
  - "ErrorCode uses #[serde(rename_all = SCREAMING_SNAKE_CASE)] — UTXO_SPENT not UtxoSpent on wire"
  - "ApiError.round_id uses skip_serializing_if = Option::is_none to omit absent round IDs"
  - "OwnershipProof not derived Serialize/Deserialize — it is a helper type, not a wire struct"
  - "compute_blind_token_message uses as_bytes() for raw script bytes without CompactSize prefix"
  - "SignRequest uses utxo_outpoint (not input_index) per design doc correction"
  - "InfoResponse has both rsa_pubkey_hash (hex) and rsa_pubkey_der_b64 (base64 DER) — None when Idle"

patterns-established:
  - "Pattern 1: All wire structs derive Debug, Clone, Serialize, Deserialize with no deny_unknown_fields"
  - "Pattern 2: Threat mitigations documented inline with T-01-XX references in code comments"
  - "Pattern 3: Canonical encoding helpers (from_json_hex_str/to_json_hex_str) live in shared/ to prevent divergence"

requirements-completed: [PROTO-03, PROTO-05, PROTO-06, PROTO-07, TEST-08]

# Metrics
duration: 3min
completed: 2026-04-08
---

# Phase 01 Plan 01: Cargo Workspace + Shared Crate Summary

**Cargo workspace with shared crate providing all wire types, domain-separated blind token hasher (SHA-256 blindjoin-v1 domain separator), serde forward-compatible message structs, and canonical OwnershipProof wire type**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-08T15:54:29Z
- **Completed:** 2026-04-08T15:57:30Z
- **Tasks:** 2
- **Files modified:** 13 created

## Accomplishments

- Cargo workspace compiles cleanly: `cargo build --workspace` exits 0
- 7 shared unit tests pass covering error serialization, hash determinism, forward compat, and OwnershipProof round-trip
- All 8 protocol types defined (7 message structs + OwnershipProof) with correct field names and no deny_unknown_fields
- compute_blind_token_message implements SHA-256("blindjoin-v1" || script_bytes || amount_le64) canonical form
- OwnershipProof::from_json_hex_str and to_json_hex_str helpers enforce canonical BIP-322 witness wire format

## Task Commits

Each task was committed atomically:

1. **Task 1: Cargo workspace + shared crate scaffold** - `06adc5f` (feat)
2. **Task 2: Protocol message types + blind token hasher + OwnershipProof** - `9b30764` (feat)

**Plan metadata:** (see final commit)

_Note: TDD tasks included RED→GREEN cycle: tests written before implementation, compilation failure confirmed before implementation added._

## Files Created/Modified

- `Cargo.toml` - Workspace root with all [workspace.dependencies] pinned
- `shared/Cargo.toml` - shared crate manifest (bitcoin, sha2, serde, uuid, hex)
- `shared/src/lib.rs` - Module declarations (errors, protocol, token, types)
- `shared/src/errors.rs` - ErrorCode enum (14 variants, SCREAMING_SNAKE_CASE) + ApiError struct
- `shared/src/types.rs` - RoundId type alias + Denomination newtype
- `shared/src/protocol.rs` - All 7 wire message structs + OwnershipProof with canonical helpers
- `shared/src/token.rs` - compute_blind_token_message + test suite
- `coordinator/Cargo.toml` - Empty binary stub manifest
- `coordinator/src/main.rs` - Empty fn main()
- `client/Cargo.toml` - Empty binary stub manifest
- `client/src/main.rs` - Empty fn main()
- `Cargo.lock` - Dependency lockfile
- `.gitignore` - Excludes /target/

## Decisions Made

- **ErrorCode SCREAMING_SNAKE_CASE**: `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` on ErrorCode enum produces UTXO_SPENT on wire; coordinator and client both deserialize from same variant names
- **ApiError skip_serializing_if**: round_id omitted from JSON when None to avoid null fields in error responses
- **OwnershipProof not a wire struct**: Only used as a helper type with explicit encode/decode methods — not itself serialized with serde to prevent accidental use of wrong format
- **compute_blind_token_message uses as_bytes()**: Raw script bytes without CompactSize length prefix per D-03/PROTO-05 specification
- **SignRequest uses utxo_outpoint**: Design doc correction — NOT input_index, uses "txid:vout" string format
- **InfoResponse has rsa_pubkey_der_b64**: Option<String> — clients MUST verify SHA-256(decode(der_b64)) == rsa_pubkey_hash before blinding (D-02)

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- The verification command `grep -r "deny_unknown_fields" shared/src/ | wc -l` returns 1 because the prohibition is documented in a code comment (`// NO #[serde(deny_unknown_fields)]`). The attribute is not applied to any struct — this is correct and intentional documentation.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- All shared types ready for consumption by plans 02-06
- compute_blind_token_message ready for coordinator blind RSA signing (plan 02)
- InputRegRequest + OwnershipProof ready for UTXO validation (plan 03)
- ApiError + all response types ready for axum handler layer (plan 04)
- No blockers

---
*Phase: 01-core-protocol*
*Completed: 2026-04-08*

## Self-Check: PASSED

- All 13 files created: FOUND
- Commits 06adc5f and 9b30764: FOUND
- cargo test -p shared: 7 passed, 0 failed
