---
phase: 01-core-protocol
plan: "03"
subsystem: bitcoin
tags: [rust, bitcoin, psbt, bip322, rpc, corepc-types, reqwest, thiserror]

# Dependency graph
requires:
  - phase: 01-01
    provides: "shared::protocol::OwnershipProof canonical BIP-322 wire type"
provides:
  - "coordinator::bitcoin::rpc::BitcoinRpc — 5-method async Bitcoin Core RPC client"
  - "coordinator::bitcoin::utxo::validate_utxo — UTXO existence, value, double-reg, BIP-322 checks"
  - "coordinator::bitcoin::utxo::verify_bip322_simple — P2WPKH BIP-322 Section 4+5 verification"
  - "coordinator::bitcoin::tx::build_coinjoin_psbt — CoinJoin PSBT with fee splitting and dust folding"
affects:
  - "02-blind-rsa (parallel wave, no dependency)"
  - "04-api-handlers (uses BitcoinRpc for startup health check and round broadcast)"
  - "06-state-machine (uses build_coinjoin_psbt to assemble PSBT in signing phase)"

# Tech tracking
tech-stack:
  added:
    - "reqwest 0.13 (async HTTP for Bitcoin Core JSON-RPC)"
    - "corepc-types 0.11 (type-safe GetTxOut deserialization)"
    - "thiserror 1 (RpcError, UtxoError, Bip322Error, TxError derive macros)"
    - "hex 0.4 (script_pubkey hex decode in utxo.rs)"
  patterns:
    - "Thin reqwest RPC client: json!() body + basic_auth + .json::<RpcResponse>() deserialization"
    - "BIP-322 Simple: to_spend (nVersion=0, scriptSig=OP_0 <tagged_hash>) + to_sign (spends to_spend, OP_RETURN output)"
    - "Dust folding: change < 294 sats omitted from outputs, silently absorbed into fee"
    - "PSBT witness_utxo populated for all inputs (required for hardware wallet SegWit signing)"

key-files:
  created:
    - "coordinator/src/bitcoin/mod.rs"
    - "coordinator/src/bitcoin/rpc.rs"
    - "coordinator/src/bitcoin/utxo.rs"
    - "coordinator/src/bitcoin/tx.rs"
  modified:
    - "Cargo.toml (added thiserror, hex to workspace.dependencies)"
    - "coordinator/Cargo.toml (added reqwest, corepc-types, thiserror, hex, serde, serde_json, bitcoin)"
    - "coordinator/src/main.rs (added pub mod bitcoin)"

key-decisions:
  - "corepc_types::v26::GetTxOut re-exports from v17 — value is f64 BTC, not Amount; convert via (value * 1e8).round() as u64"
  - "Version::NON_STANDARD does not exist in bitcoin 0.32 — used Version(0) for BIP-322 to_spend tx per spec"
  - "OP_0 is at bitcoin::opcodes::OP_0 not bitcoin::opcodes::all::OP_0 in rust-bitcoin 0.32"
  - "Txid construction in tests uses Txid::from_raw_hash(sha256d::Hash::from_byte_array(bytes)) with Hash trait in scope"
  - "DUST_THRESHOLD_SATS = 294 (P2WPKH standard relay dust limit per bitcoin 0.32 policy)"

patterns-established:
  - "Pattern 4: RpcError variants cover Http, Rpc, Parse, BroadcastRejected, Unreachable — propagates via From<RpcError> for UtxoError"
  - "Pattern 5: BIP-322 message format: 'blindjoin:round:{round_id}:utxo:{txid}:{vout}'"
  - "Pattern 6: Coordinator bitcoin module uses serde_json::Value as intermediate for RPC responses, then deserializes with serde_json::from_value"

requirements-completed: [UTXO-01, UTXO-02, UTXO-03, UTXO-04, UTXO-05, TX-01, TX-02, TX-03, TX-04, TX-05, TX-06, TX-07, TX-08, TEST-03, TEST-05, TEST-06]

# Metrics
duration: 15min
completed: 2026-04-07
---

# Phase 01 Plan 03: Bitcoin Subsystem Summary

**Thin Bitcoin Core RPC client (5 methods), UTXO validation with BIP-322 Simple P2WPKH proof verification, and CoinJoin PSBT construction with per-participant fee splitting and sub-294-sat dust folding**

## Performance

- **Duration:** 15 min
- **Started:** 2026-04-07T00:00:00Z
- **Completed:** 2026-04-07T00:15:00Z
- **Tasks:** 2
- **Files modified:** 7 (4 created, 3 modified)

## Accomplishments

- 8 tests pass: 3 BIP-322 tests (valid P2WPKH, wrong witness length, wrong message) + 5 PSBT tests (N denomination outputs, valid PSBT round-trip, dust folded, insufficient funds, witness_utxo set)
- BitcoinRpc async client handles spent UTXOs (gettxout returns null → Ok(None)), broadcast rejection (-25/-26/-27 codes), and unreachability as distinct error variants
- BIP-322 Simple implemented directly per spec (~50 lines): tagged hash "BIP0322-signed-message", to_spend nVersion=0, to_sign spends to_spend output, P2WPKH sighash verified then pubkey hash160-checked against scriptPubKey
- CoinJoin PSBT: denomination outputs + change outputs (dust folded), witness_utxo populated on all inputs, Psbt::deserialize(psbt.serialize()) round-trip passes

## Task Commits

Each task was committed atomically:

1. **Task 1: Bitcoin RPC client + UTXO validation + BIP-322** - `289cff0` (feat)
2. **Task 2: CoinJoin PSBT construction** - `fc226a3` (feat)

_Note: TDD tasks included test-and-implement cycle. All tests written inline with implementation._

## Files Created/Modified

- `coordinator/src/bitcoin/mod.rs` — module declarations (rpc, tx, utxo)
- `coordinator/src/bitcoin/rpc.rs` — BitcoinRpc struct + 5 async methods + RpcError enum
- `coordinator/src/bitcoin/utxo.rs` — validate_utxo, verify_bip322_simple, BIP-322 helpers, UtxoError, Bip322Error
- `coordinator/src/bitcoin/tx.rs` — build_coinjoin_psbt, ParticipantInput/Output, TxError, 5 tests
- `Cargo.toml` — added thiserror = "1", hex = "0.4" to [workspace.dependencies]
- `coordinator/Cargo.toml` — added reqwest, corepc-types, thiserror, hex, serde, serde_json, bitcoin as workspace deps
- `coordinator/src/main.rs` — added `pub mod bitcoin`

## Decisions Made

- **corepc-types GetTxOut API**: The v26 module re-exports from v17. `GetTxOut.value` is `f64` BTC (not `bitcoin::Amount`). Converted via `(value * 100_000_000.0).round() as u64`. `GetTxOut.script_pubkey` is `ScriptPubkey { hex: String, ... }` — hex-decoded to produce `ScriptBuf`.
- **BIP-322 to_spend version**: `bitcoin::transaction::Version::NON_STANDARD` does not exist in rust-bitcoin 0.32. BIP-322 Section 4 specifies nVersion=0 — used `Version(0)` directly.
- **OP_0 opcode path**: In rust-bitcoin 0.32, `OP_0` lives at `bitcoin::opcodes::OP_0`, not `bitcoin::opcodes::all::OP_0`.
- **Txid construction in tests**: `Txid::from_byte_array()` requires `bitcoin::hashes::Hash` trait in scope. Used `Txid::from_raw_hash(sha256d::Hash::from_byte_array(bytes))` pattern.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Three rust-bitcoin 0.32 API mismatches in plan code**
- **Found during:** Task 1 compilation
- **Issue 1:** `bitcoin::opcodes::all::OP_0` does not exist — correct path is `bitcoin::opcodes::OP_0`
- **Issue 2:** `bitcoin::transaction::Version::NON_STANDARD` does not exist — BIP-322 uses nVersion=0, so `Version(0)` is correct
- **Issue 3:** `Txid::from_byte_array()` in tests requires Hash trait in scope — used `Txid::from_raw_hash(sha256d::Hash::from_byte_array(...))` pattern
- **Fix:** Applied all three corrections in utxo.rs and tx.rs
- **Files modified:** `coordinator/src/bitcoin/utxo.rs`, `coordinator/src/bitcoin/tx.rs`
- **Verification:** `cargo test -p coordinator bitcoin` — 8 passed, 0 failed
- **Committed in:** `289cff0` and `fc226a3` (part of task commits)

---

**Total deviations:** 1 auto-fixed (Rule 1: 3 API mismatches in plan-provided code snippets)
**Impact on plan:** All fixes necessary for correctness with rust-bitcoin 0.32. No scope creep. BIP-322 behavior unchanged — Version(0) is what the spec requires.

## Issues Encountered

- corepc-types was not pre-downloaded in cargo registry; `cargo fetch` + `cargo build` downloaded it before implementation
- The `GetTxOut.value` field is raw `f64` BTC in corepc-types v17/v26 (not `bitcoin::Amount`), unlike what plan comments implied — documented in decisions

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- `BitcoinRpc` ready for plan 04 startup health check (`getblockcount`) and broadcast (`sendrawtransaction`)
- `validate_utxo` ready for plan 04 input registration handler
- `build_coinjoin_psbt` ready for plan 06 signing phase PSBT assembly
- No blockers

---
*Phase: 01-core-protocol*
*Completed: 2026-04-07*

## Self-Check: PASSED

- coordinator/src/bitcoin/mod.rs: FOUND
- coordinator/src/bitcoin/rpc.rs: FOUND
- coordinator/src/bitcoin/utxo.rs: FOUND
- coordinator/src/bitcoin/tx.rs: FOUND
- Commit 289cff0: FOUND (feat(01-03): Bitcoin RPC client, UTXO validation, BIP-322 Simple)
- Commit fc226a3: FOUND (feat(01-03): CoinJoin PSBT construction with fee splitting and dust folding)
- cargo test -p coordinator bitcoin: 8 passed, 0 failed
