---
phase: 20-mixed-round-fee-accuracy
reviewed: 2026-05-31T20:44:44Z
depth: standard
files_reviewed: 9
files_reviewed_list:
  - coordinator/src/api/handlers.rs
  - coordinator/src/bitcoin/fee.rs
  - coordinator/src/bitcoin/tx.rs
  - coordinator/src/bitcoin/utxo.rs
  - coordinator/src/config.rs
  - coordinator/src/round/blame.rs
  - coordinator/src/round/input_reg.rs
  - coordinator/src/round/signing.rs
  - coordinator/src/round/state.rs
findings:
  critical: 0
  warning: 5
  info: 4
  total: 9
status: issues_found
---

# Phase 20: Code Review Report

**Reviewed:** 2026-05-31T20:44:44Z
**Depth:** standard
**Files Reviewed:** 9
**Status:** issues_found

## Summary

Phase 20's per-script fee table is mechanically sound: the new `script_input_vbytes` / `script_output_vbytes` const fns match BIP-141 derivations (pinned by 6 vbyte tests), `ScriptType` is correctly threaded `UtxoDetails → RegisteredInput → ParticipantInput` with no new client-trust hole, and the `output_script_type` source-of-truth contract is honoured at both `get_tx` and `assemble_and_broadcast` call sites (WR-04). The integer-floor `fee_share = total_fee / n` is preserved verbatim, the v1.4 P2WPKH-only baseline (266 sats @ n=3, fee_rate=2) is byte-exact, and the FEE-03(b) mixed-script test (275 sats) proves the per-script branch fires. CRIT-01 is intact — both `detect_script_type` call sites live inside `dispatch_ownership_proof`.

However, **the API response field `fee_per_participant_sats` returned by `GET /round/tx` now silently diverges from the actual fee a participant pays in the PSBT** for any round whose allowed-set contains a heavier script type than the inputs that actually registered. The handler uses the worst-case `estimate_fee_share` (which over-charges by ≥46 sats per participant in a P2WPKH-only round with mixed-allowed config) while `build_coinjoin_psbt` charges the real per-input value. This is the headline finding (WR-01). Additional concerns: the new `tracing::info!` in `validate_utxo` logs `script_type` per-participant, which is per-participant data outside the PRIV-02 allowlist (WR-02); the magic `10` in `fee.rs` duplicates `TX_OVERHEAD_VBYTES` instead of re-using the const (WR-03); the inline P2TR-vbyte comment block is misplaced relative to its arm (WR-04); and `UtxoError::InvalidProof` is misused for what is actually an RPC-payload parse failure (WR-05).

## Warnings

### WR-01: `GET /round/tx` response advertises a fee that does not match the PSBT

**File:** `coordinator/src/api/handlers.rs:514-523`
**Issue:** The handler builds the PSBT via `build_coinjoin_psbt(..)`, which sums the *real* per-input `script_input_vbytes(inp.script_type)` over registered inputs. Then it computes the response field independently via `estimate_fee_share(&state.config.bip, n, fee_rate)`, which uses `max(script_input_vbytes across allowed_set())` for every input. The two formulas are not equivalent and diverge whenever the actually-registered inputs are not all the worst-case allowed script type.

Concrete example with the default `[bip]` (all 3 allowed) at `n=3`, `fee_rate=2`, all three participants register P2WPKH:
- PSBT real `fee_share` (from `build_coinjoin_psbt`): `(10 + 68*3 + 31*6) * 2 / 3 = 400 * 2 / 3 = 266` sats
- Response `fee_per_participant_sats` (from `estimate_fee_share`): `(10 + 91*3 + 31*6) * 2 / 3 = 469 * 2 / 3 = 312` sats
- Response `fee_total_sats = 312 * 3 = 936` sats

The participant decodes the PSBT and sees they are charged 266 sats; the JSON envelope tells them they are charged 312 sats. The PSBT itself is correct (clients can recompute), but the API field becomes a lie whenever the registered set is narrower than the allowed set — which is the common case for a P2WPKH-only round on a fully-allowed coordinator. The phase context names this exact location as a WR-04 byte-equality site, but the response metadata was overlooked.

**Fix:** Derive `fee_per_participant_sats` from the PSBT the handler just built (single source of truth), e.g.:
```rust
let total_in: u64 = participant_inputs.iter().map(|i| i.value_sats).sum();
let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
let fee_total_sats = total_in - total_out;
let fee_per_participant_sats = fee_total_sats / (n as u64);
```
Alternatively, expose `build_coinjoin_psbt`'s `fee_share` as a return value (e.g., return `(Psbt, FeeReport)`) so both the PSBT and the response field are produced from the same computation. The current `estimate_fee_share` call site is correct for the *pre-lock value check* in `post_input` (`fee_share_pre_lock`, line 167) — keep it there.

### WR-02: `tracing::info!` in `validate_utxo` logs per-participant `script_type`

**File:** `coordinator/src/bitcoin/utxo.rs:115-119`
**Issue:** Per the phase context and the PRIV-02 logging allowlist at the top of `handlers.rs` ("ALLOWED in logs: round_id, phase, participant_count, txid (after broadcast), addr (listen addr), block_count"), `script_type` is per-participant data that is not on the allowed list. The current log line emits one structured event per input registration, producing a per-round sequence such as `[p2wpkh, p2tr, p2wpkh, p2sh-p2wpkh]` in chronological order. An adversary with log access (operator-internal or via log shipping) can correlate this sequence with input-registration request timing and, in combination with network observation, narrow the candidate set for individual participants. The 3-value cardinality is small but the *ordering* leak is meaningful for small `n`.

**Fix:** Either drop the line, or aggregate per round (e.g., emit a single structured event at `Signing→Broadcast` transition with counts only — `{"p2wpkh": 2, "p2tr": 1}` — never the registration sequence). If the line is kept for debug, downgrade to `tracing::debug!` and gate behind a non-default subscriber level so production never emits it. Example:
```rust
// Aggregate at broadcast time, not per-registration:
tracing::info!(round_id = %round_id, "ownership proof verified");
// (no script_type field)
```

### WR-03: `fee.rs::estimate_fee_share` hard-codes `10` instead of reusing `TX_OVERHEAD_VBYTES`

**File:** `coordinator/src/bitcoin/fee.rs:40`
**Issue:** `let estimated_vsize = 10 + worst_input_vb * n + output_vb * 2 * n;` — the `10` is the same TX overhead that `tx.rs` defines as `const TX_OVERHEAD_VBYTES: u64 = 10` (line 15). Duplicating the magic number creates a silent regression hazard: a future BIP-141 overhead update (or any move to TX version 3 / annex-aware accounting) would have to be made in two places, and divergence between the pre-lock estimate and the build-time formula would cause `validate_utxo`'s `required = denomination_sats + fee_share` check to under- or over-estimate.

**Fix:** Re-export `TX_OVERHEAD_VBYTES` from `tx.rs` (e.g., `pub const TX_OVERHEAD_VBYTES: u64 = 10;` is already `const` — just change visibility) and import it in `fee.rs`:
```rust
use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes, TX_OVERHEAD_VBYTES};
...
let estimated_vsize = TX_OVERHEAD_VBYTES + worst_input_vb * n + output_vb * 2 * n;
```

### WR-04: P2TR derivation comment is misplaced relative to its match arm

**File:** `coordinator/src/bitcoin/tx.rs:32-44`
**Issue:** Inside `script_input_vbytes`, the comment block that documents the P2TR `58 vB` value (including the load-bearing STATE.md round-UP justification — "ROADMAP SC#1 cites 57 (floor of 57.5); STATE.md §v1.5 design notes mandates UP-rounding") sits *after* the `ScriptType::P2tr => 58,` arm and immediately *before* `ScriptType::P2shP2wpkh => 91,`. A reader scanning the match expects the derivation comment to *precede* each arm — the current layout makes the P2TR justification look like part of the P2SH-P2WPKH derivation. The phase context explicitly calls this out: "the const fn match arm for `ScriptType::P2tr` should have an inline comment citing STATE.md to document the intentional divergence." The comment exists but is in the wrong place.

**Fix:** Move the P2TR derivation block above the `P2tr` arm:
```rust
pub const fn script_input_vbytes(st: ScriptType) -> u64 {
    match st {
        // P2WPKH: 41 non_witness + ceil(108/4) = 41 + 27 = 68 vB
        ScriptType::P2wpkh => 68,
        // P2TR: 41 non_witness + ceil(66/4) = 41 + 17 = 58 vB.
        // ROADMAP SC#1 cites 57 (floor of 57.5); STATE.md §v1.5 design notes
        // mandates UP-rounding so the coordinator never underpays on a mixed
        // round — 58 is the load-bearing value (raw 57.5, round UP).
        ScriptType::P2tr => 58,
        // P2SH-P2WPKH: 64 non_witness (23-byte redeem wrapper inside scriptSig)
        // + ceil(108/4) = 64 + 27 = 91 vB
        ScriptType::P2shP2wpkh => 91,
    }
}
```

### WR-05: `UtxoError::InvalidProof` is reused for BTC-amount parse failure

**File:** `coordinator/src/bitcoin/utxo.rs:91-93`
**Issue:** `bitcoin::Amount::from_btc(txout.value)` failing means Bitcoin Core returned a BTC value the client crate cannot represent (e.g., negative, NaN, or above the 21 M ceiling) — this is an RPC-payload integrity failure, NOT a BIP-322 ownership-proof failure. Returning `UtxoError::InvalidProof { reason: format!("BTC amount parse: {e}") }` maps to HTTP `400 BAD_REQUEST / INVALID_PROOF` in `post_input`, leading the client to believe their ownership proof was malformed when in fact Bitcoin Core's RPC response was wrong. Operators debugging from logs will chase a phantom proof bug. This was added/changed in phase 19 (commit 5843691) and is now reached more often because mixed-script rounds exercise the path more aggressively, but the error-code mismatch slipped through phase 20 review of the file.

**Fix:** Add a dedicated variant (or reuse `RpcUnavailable` with a clear message):
```rust
#[derive(Debug, thiserror::Error)]
pub enum UtxoError {
    ...
    #[error("Bitcoin Core returned an unparseable BTC amount: {0}")]
    RpcResponseInvalid(String),
}

let value_sats = bitcoin::Amount::from_btc(txout.value)
    .map_err(|e| UtxoError::RpcResponseInvalid(format!("BTC amount parse: {e}")))?
    .to_sat();
```
And map to `503 SERVICE_UNAVAILABLE / RPC_UNAVAILABLE` (or a new `RPC_RESPONSE_INVALID`) in `handlers.rs:191-207`. As a minimum-touch alternative, change to `UtxoError::RpcUnavailable(format!("gettxout returned unparseable BTC amount: {e}"))` so at least the HTTP status is correct.

## Info

### IN-01: No test pins WR-04 byte-equality between `get_tx` and `assemble_and_broadcast`

**File:** `coordinator/src/round/signing.rs` / `coordinator/src/api/handlers.rs`
**Issue:** The WR-04 invariant — "the broadcast PSBT is byte-identical to the one clients signed against in display path" — is documented in inline comments at both call sites but has no executable test. The two code paths happen to call `build_coinjoin_psbt` with arguments sourced from the same `Arc<CoordinatorConfig>`, so byte-equality holds today; nothing prevents a future refactor (e.g., adding a second `fee_rate` override knob, or passing a per-call `output_script_type`) from silently breaking it. The 6 vbyte-pin tests + 2 FEE-03 tests cover the fee math but not the cross-handler equality.
**Fix:** Add an integration test that drives `post_input → post_output → get_tx → process_sign` against a fixture round, compares `get_tx.psbt` to the PSBT extracted from `assemble_and_broadcast` (before `extract_tx`), and asserts they serialize byte-equal modulo the participant witnesses. A unit-tier alternative: factor out a `build_round_psbt(state, config) -> Psbt` helper and have both handlers call it (already effectively the case — just enforce by hoisting the call to a shared function).

### IN-02: `estimate_fee_share`'s `expect()` panics if a `BipConfig` is constructed with all flags false outside the validated config path

**File:** `coordinator/src/bitcoin/fee.rs:34-38`
**Issue:** `.allowed_set().map(...).max().expect("BipConfig::validate ensures at least one allow_* flag is true")` assumes every `BipConfig` reaching this function has been through `CoordinatorConfig::validate()`. A test (or future code path) that constructs `BipConfig { allow_p2wpkh: false, allow_p2tr: false, allow_p2sh_p2wpkh: false, ... }` directly will panic instead of returning a useful error. The `fee::tests::make_bip_config(false, false, false, ...)` is reachable in principle.
**Fix:** Return `Result<u64, FeeError>` (or saturate to a safe default with a `tracing::error!`) rather than panicking:
```rust
let worst_input_vb = bip_config.allowed_set().map(script_input_vbytes).max()
    .ok_or(FeeError::EmptyAllowedSet)?;
```
At minimum, document the invariant on the function signature with a `# Panics` section so callers know.

### IN-03: No overflow guards on `estimated_vsize * fee_rate`

**File:** `coordinator/src/bitcoin/fee.rs:40-41` and `coordinator/src/bitcoin/tx.rs:130`
**Issue:** Both `(estimated_vsize * fee_rate) / n` (fee.rs) and `estimated_vsize * fee_rate_sat_per_vbyte` (tx.rs) use plain `*` on `u64`. With unbounded `fee_rate_sat_per_vbyte` config (no upper bound in `CoordinatorConfig::validate`) and arbitrarily large `n`, the multiplication can overflow — panic in debug, wraparound in release. Realistic config values (`fee_rate ≤ 1000` sat/vB, `n ≤ 100`) make this practically unreachable, but a malicious or fat-fingered operator setting `fee_rate_sat_per_vbyte = u64::MAX` would crash the coordinator on the first PSBT build.
**Fix:** Use `checked_mul` and return `TxError::PsbtBuildFailed` (or analogous) on overflow, or add a `fee_rate_sat_per_vbyte` upper bound to `CoordinatorConfig::validate` (e.g., reject anything above 10_000 sat/vB as obviously misconfigured).

### IN-04: `parse_bitcoin_network` silently defaults unknown strings to Signet

**File:** `coordinator/src/api/handlers.rs:628-636` and `coordinator/src/round/signing.rs:299-307`
**Issue:** Both handler and signing modules contain identical local copies of `parse_bitcoin_network` that fall through to `_ => bitcoin::Network::Signet`. A typo in `coordinator.toml` (e.g., `bitcoin_network = "testnet5"`) silently boots the coordinator as Signet rather than failing fast. Less severe than it sounds because `CoordinatorConfig::validate` runs once at startup and the bitcoin-network string is presumably re-validated elsewhere, but the silent-default pattern + code duplication (two copies that must stay in sync) is fragile. Not phase-20-introduced but the file is in scope.
**Fix:** Hoist `parse_bitcoin_network` to a shared module (e.g., `coordinator::config::parse_bitcoin_network`), make it return `Result<bitcoin::Network, ConfigError>` instead of silently defaulting, and validate at config-load time.

---

_Reviewed: 2026-05-31T20:44:44Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
