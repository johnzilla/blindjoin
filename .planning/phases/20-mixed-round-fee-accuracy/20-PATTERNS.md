# Phase 20 — Pattern Map

**Mapped:** 2026-05-31
**Phase:** 20 - Mixed-Round Fee Accuracy
**Files analyzed:** 6 modified (1 plan, 10 tasks per CONTEXT D-127)
**Analogs found:** 6 / 6 (all in-tree, all in-file or sibling-file)
**Scope:** Pure data plumbing + a 6-cell lookup table. Every modification has a strong in-crate analog already exercised by the v1.3 `full_round` and v1.4 `mixed_script_e2e` invariant gates. The load-bearing precedents are (a) the existing `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES` consts at `tx.rs:11,13` (lift into a `const fn` per-script match), (b) the `script_pubkey` field on `RegisteredInput` at `state.rs:60-65` (mirror exactly for `script_type`), and (c) the existing single-canonical `estimate_fee_share` helper at `fee.rs:11-18` (preserve the WR-04 invariant while changing the formula).

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `coordinator/src/bitcoin/tx.rs` (modify: delete 2 consts, add 2 `const fn`, extend `ParticipantInput`, modify `build_coinjoin_psbt` signature + body, extend `make_inputs` helper, add 8 tests) | service / fee math + types | transform (pure) | self (`INPUT_WEIGHT_VBYTES`, `build_coinjoin_psbt`, inline `mod tests` block) | exact (in-file lift) |
| `coordinator/src/bitcoin/fee.rs` (modify: `estimate_fee_share` signature gains `&BipConfig`, body uses worst-case per-script formula) | service / fee math | transform (pure) | self (current 1-line vsize formula at line 16) + sibling `tx.rs::build_coinjoin_psbt` (vsize formula in production path) | exact (in-file refactor) |
| `coordinator/src/bitcoin/utxo.rs` (modify: extend `UtxoDetails` with `script_type`, return `derived` in `Ok(...)` at line 116) | service / validation | request-response | self (existing `UtxoDetails {value_sats, script_pubkey}` + `let derived = dispatch_ownership_proof(...)` at line 99) | exact (already-computed value plumbed through) |
| `coordinator/src/round/state.rs` (modify: add `script_type: ScriptType` field with `#[zeroize(skip)]` to `RegisteredInput`) | model / round state | persistent (in-memory only) | self (`pub script_pubkey: ScriptBuf` with `#[zeroize(skip)]` at lines 60-65) | exact (mirror sibling field) |
| `coordinator/src/round/input_reg.rs` + `coordinator/src/api/handlers.rs` + `coordinator/src/round/signing.rs` (modify: thread `script_type` through `register_input` signature + write site; update `ParticipantInput` construction at 2 call sites; update 2 `estimate_fee_share` callsites to pass `&state.config.bip`) | controller/service plumbing | request-response | self (existing `utxo_script_pubkey` / `utxo_value_sats` parameter threading pattern at `input_reg.rs:35-43` + `handlers.rs:256-263`) | exact (parallel field added next to existing fields) |
| `coordinator/src/config.rs` (modify: add `BipConfig::allowed_set` helper) | utility / config | request-response (in-memory query) | self (existing `BipConfig::allows` at lines 191-197 + `BipConfig::supported` at lines 205-217) | exact (sibling method) |

---

## Pattern Assignments

### File: `coordinator/src/bitcoin/tx.rs` (modified)

#### Add 1 — `script_input_vbytes` + `script_output_vbytes` const fns

**Insert location:** Replace the two existing const declarations at `tx.rs:11-13`. `TX_OVERHEAD_VBYTES = 10` at line 15 STAYS (script-independent overhead per RESEARCH §"Hardcoded constants").

**Analog (in-file, the deleted consts they replace):** `tx.rs:10-13`

```rust
// coordinator/src/bitcoin/tx.rs:10-13 — ANALOG (deleted by Plan 20-01)
/// Estimated weight per input (P2WPKH): 68 vbytes
const INPUT_WEIGHT_VBYTES: u64 = 68;
/// Estimated weight per output (P2WPKH): 31 vbytes
const OUTPUT_WEIGHT_VBYTES: u64 = 31;
```

**New code pattern (per CONTEXT D-124 + RESEARCH §"Code Examples" Example 4):**

```rust
// coordinator/src/bitcoin/tx.rs — REPLACEMENT (Plan 20-01 Task 1)
use shared::bip322::ScriptType;

/// Input vbytes per BIP-141 worst-case witness, conservative-rounded-UP
/// (raw value → ceil(witness/4) via integer-arithmetic `(w + 3) / 4`).
pub const fn script_input_vbytes(st: ScriptType) -> u64 {
    match st {
        // 41 non_witness (32 prev_txid + 4 vout + 1 script_sig_len(0) + 4 sequence)
        // + 108 witness (1 stack_count + 1 sig_len(72) + 72 DER+SIGHASH_ALL
        // + 1 pk_len(33) + 33 compressed pk) / 4 = 27
        // = 68 vB
        ScriptType::P2wpkh => 68,
        // 41 non_witness (same as P2WPKH)
        // + 66 witness (1 stack_count + 1 sig_len(64) + 64 Schnorr SIGHASH_DEFAULT)
        //   → ceil(66/4) = 17
        // = 58 vB. ROADMAP SC#1 cites 57 (floor of 57.5); STATE.md §v1.5 design
        // notes mandates UP-rounding so the coordinator never underpays fees.
        ScriptType::P2tr => 58,
        // 64 non_witness (32 prev_txid + 4 vout + 1 script_sig_len(23) + 23 redeem
        // wrapper + 4 sequence) + 108 witness (same as P2WPKH) / 4 = 27
        // = 91 vB
        ScriptType::P2shP2wpkh => 91,
    }
}

/// Output vbytes — exact bytes (outputs have no segwit discount, no rounding).
pub const fn script_output_vbytes(st: ScriptType) -> u64 {
    match st {
        // 8 value + 1 script_len(22) + 22 (OP_0 OP_PUSHBYTES_20 <20>) = 31
        ScriptType::P2wpkh => 31,
        // 8 value + 1 script_len(34) + 34 (OP_1 OP_PUSHBYTES_32 <32>) = 43
        ScriptType::P2tr => 43,
        // 8 value + 1 script_len(23) + 23 (OP_HASH160 OP_PUSHBYTES_20 <20> OP_EQUAL) = 32
        ScriptType::P2shP2wpkh => 32,
    }
}
```

**CD-40 resolution:** RESEARCH recommends fee.rs as the canonical home (the two functions now share the weight table). Plan-phase can locate them in either tx.rs (close to `build_coinjoin_psbt`) or fee.rs (close to `estimate_fee_share`). If placed in fee.rs, tx.rs imports via `use crate::bitcoin::fee::{script_input_vbytes, script_output_vbytes};`.

**CD-41 resolution:** Use `const fn` — `ScriptType` is `Copy + PartialEq + Eq`, match arms over enum variants are const-evaluable in stable Rust 2025.

---

#### Add 2 — Extend `ParticipantInput` with `script_type` field

**Modify location:** `tx.rs:17-23` — add field after `change_address`.

**Analog (in-file, the struct being extended):**

```rust
// coordinator/src/bitcoin/tx.rs:17-23 — ORIGINAL
#[derive(Debug, Clone)]
pub struct ParticipantInput {
    pub outpoint: OutPoint,
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,   // for PSBT input UTXO field
    pub change_address: ScriptBuf,  // their change output script
}
```

**New code pattern:**

```rust
// coordinator/src/bitcoin/tx.rs:17-24 — PHASE 20
#[derive(Debug, Clone)]
pub struct ParticipantInput {
    pub outpoint: OutPoint,
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,                  // for PSBT input UTXO field
    pub change_address: ScriptBuf,                 // their change output script
    pub script_type: shared::bip322::ScriptType,   // FEE-02: per-input vbyte selector
                                                   // (coordinator-derived from on-chain SPK at
                                                   // validate_utxo, never client-declared — CRIT-01)
}
```

**Provenance discipline:** The new field has NO `#[serde]` derive on the struct (already none — `ParticipantInput` does not cross the wire). RESEARCH §"`ParticipantInput` Shape" confirms zero wire-format impact.

---

#### Add 3 — `build_coinjoin_psbt` signature + body

**Modify location:** `tx.rs:53-70` — extend signature; replace the vsize formula at lines 66-68.

**Analog (in-file, current production body):**

```rust
// coordinator/src/bitcoin/tx.rs:53-70 — ORIGINAL
pub fn build_coinjoin_psbt(
    inputs: &[ParticipantInput],
    outputs: &[ParticipantOutput],
    denomination_sats: u64,
    fee_rate_sat_per_vbyte: u64,
) -> Result<Psbt, TxError> {
    if inputs.is_empty() {
        return Err(TxError::NoParticipants);
    }
    let n = inputs.len() as u64;

    // Estimate size assuming all participants have change outputs (upper bound)
    let num_change_outputs = n;
    let estimated_vsize = TX_OVERHEAD_VBYTES
        + n * INPUT_WEIGHT_VBYTES
        + (n + num_change_outputs) * OUTPUT_WEIGHT_VBYTES;
    let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
    let fee_share = total_fee / n;  // each participant pays fee_share
```

**New code pattern (per CONTEXT D-127 Task 5 + RESEARCH Example 3):**

```rust
// coordinator/src/bitcoin/tx.rs — PHASE 20 (Task 5)
pub fn build_coinjoin_psbt(
    inputs: &[ParticipantInput],
    outputs: &[ParticipantOutput],
    denomination_sats: u64,
    fee_rate_sat_per_vbyte: u64,
    output_script_type: ScriptType,   // NEW (single output type per round, Phase 16 D-37)
) -> Result<Psbt, TxError> {
    if inputs.is_empty() {
        return Err(TxError::NoParticipants);
    }
    let n = inputs.len() as u64;

    // Estimate size assuming all participants have change outputs (upper bound)
    let num_change_outputs = n;
    let total_input_vb: u64 = inputs.iter()
        .map(|inp| script_input_vbytes(inp.script_type))
        .sum();
    let output_vb = script_output_vbytes(output_script_type);
    let estimated_vsize = TX_OVERHEAD_VBYTES
        + total_input_vb
        + (n + num_change_outputs) * output_vb;
    let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
    let fee_share = total_fee / n;  // PRESERVE integer floor (D-125 byte-equality)
```

**RISK-1 hedge:** `let fee_share = total_fee / n;` MUST be preserved verbatim (integer floor). Refactoring to a helper function or ceil-divide breaks D-125's hardcoded `266` baseline.

---

#### Add 4 — Extend `make_inputs` test helper (Plan 20-01 Task 1+ setup)

**Modify location:** `tx.rs:150-157` — extend signature to accept per-input ScriptType, keeping a P2WPKH-default convenience wrapper for the existing 5 tests.

**Analog (in-file, current helper):**

```rust
// coordinator/src/bitcoin/tx.rs:150-157 — ORIGINAL
fn make_inputs(n: usize, value_sats: u64) -> Vec<ParticipantInput> {
    (0..n).map(|i| ParticipantInput {
        outpoint: dummy_outpoint(i as u8),
        value_sats,
        script_pubkey: p2wpkh_script(i as u8),
        change_address: p2wpkh_script((i + 100) as u8),
    }).collect()
}
```

**New code pattern (extend in place; existing tests pass `ScriptType::P2wpkh` implicitly via a default wrapper or explicitly):**

```rust
// coordinator/src/bitcoin/tx.rs — PHASE 20
fn make_inputs(n: usize, value_sats: u64) -> Vec<ParticipantInput> {
    make_inputs_typed(&vec![ScriptType::P2wpkh; n], value_sats)
}

fn make_inputs_typed(types: &[ScriptType], value_sats: u64) -> Vec<ParticipantInput> {
    types.iter().enumerate().map(|(i, &st)| ParticipantInput {
        outpoint: dummy_outpoint(i as u8),
        value_sats,
        script_pubkey: p2wpkh_script(i as u8),  // SPK shape irrelevant for fee math
        change_address: p2wpkh_script((i + 100) as u8),
        script_type: st,
    }).collect()
}
```

The existing 5 tests at `tx.rs:165-223` continue to call `make_inputs(n, value)` unchanged.

---

#### Add 5 — 6 vbyte-table unit tests (CONTEXT D-124c)

**Insert location:** Inline in `tx.rs::tests` block (after the existing 5 tests at line 223, before module close at 224).

**Analog (in-file test convention):** `coinjoin_psbt_n_denomination_outputs` at `tx.rs:165-176` — single `#[test] fn`, plain `assert_eq!`, no fixtures, no async. Phase 20 follows the same minimalist style.

```rust
// coordinator/src/bitcoin/tx.rs:165-176 — ANALOG (style precedent)
#[test]
fn coinjoin_psbt_n_denomination_outputs() {
    let n = 3;
    let denomination_sats = 1_000_000;
    let inputs = make_inputs(n, 1_100_000);
    let outputs = make_outputs(n);
    let psbt = build_coinjoin_psbt(&inputs, &outputs, denomination_sats, 2).unwrap();
    let denom_outputs: Vec<_> = psbt.unsigned_tx.output.iter()
        .filter(|o| o.value.to_sat() == denomination_sats)
        .collect();
    assert_eq!(denom_outputs.len(), n, "Must have exactly N denomination outputs");
}
```

**New code pattern (6 tests pinning the table):**

```rust
// coordinator/src/bitcoin/tx.rs — PHASE 20 (Task 1 — 6 tests per D-124c)
#[test]
fn script_input_vbytes_p2wpkh_is_68() {
    assert_eq!(script_input_vbytes(ScriptType::P2wpkh), 68);
}
#[test]
fn script_input_vbytes_p2tr_is_58_up_rounded() {
    // 41 + ceil(66/4) = 41 + 17 = 58. ROADMAP says 57 (floor); STATE.md §v1.5
    // design notes mandates UP-rounding — 58 is correct.
    assert_eq!(script_input_vbytes(ScriptType::P2tr), 58);
}
#[test]
fn script_input_vbytes_p2sh_p2wpkh_is_91() {
    assert_eq!(script_input_vbytes(ScriptType::P2shP2wpkh), 91);
}
#[test]
fn script_output_vbytes_p2wpkh_is_31() {
    assert_eq!(script_output_vbytes(ScriptType::P2wpkh), 31);
}
#[test]
fn script_output_vbytes_p2tr_is_43() {
    assert_eq!(script_output_vbytes(ScriptType::P2tr), 43);
}
#[test]
fn script_output_vbytes_p2sh_p2wpkh_is_32() {
    assert_eq!(script_output_vbytes(ScriptType::P2shP2wpkh), 32);
}
```

---

#### Add 6 — FEE-03(a) `fee_share_p2wpkh_only_matches_v14_baseline` (CONTEXT D-125)

**Insert location:** Inline in `tx.rs::tests` block after the 6 vbyte tests.

**Analog (in-file, byte-exact PSBT assertion style):** `coinjoin_psbt_dust_folded_into_fee` at `tx.rs:189-204` — computes totals via `psbt.unsigned_tx.output.iter().map(...).sum()`, asserts inequalities. Phase 20's new test is stricter (byte-exact equality) but follows the same PSBT-introspection pattern.

```rust
// coordinator/src/bitcoin/tx.rs:189-204 — ANALOG (PSBT-introspection style)
let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
assert!(total_out < total_in, "Fee must be deducted: total_out < total_in");
```

**New code pattern (per CONTEXT D-125 + RESEARCH §"v1.4 baseline math"):**

```rust
// coordinator/src/bitcoin/tx.rs — PHASE 20 (Task 8)
#[test]
fn fee_share_p2wpkh_only_matches_v14_baseline() {
    // v1.4 baseline (P2WPKH-only, n=3, fee_rate=2):
    // estimated_vsize = TX_OVERHEAD + n*68 + (n + n)*31
    //                 = 10 + 3*68 + 6*31
    //                 = 10 + 204 + 186
    //                 = 400 vbytes
    // total_fee  = 400 * 2 = 800 sats
    // fee_share  = 800 / 3 = 266 sats (integer floor; 2-sat remainder absorbed)
    let n = 3;
    let denomination_sats = 1_000_000;
    let inputs = make_inputs(n, 1_100_000);   // CD-44: reuse existing helper
    let outputs = make_outputs(n);
    let psbt = build_coinjoin_psbt(
        &inputs, &outputs, denomination_sats, 2,
        ScriptType::P2wpkh,                   // output_script_type
    ).unwrap();
    // Derive fee_share from PSBT: total_in - total_out should equal 800 (total_fee)
    // and per-participant burden derives as 800/3 = 266.
    let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
    let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
    let total_fee = total_in - total_out;
    let fee_share = total_fee / (n as u64);
    assert_eq!(fee_share, 266, "v1.4 P2WPKH-only baseline must be byte-exact 266");
}
```

---

#### Add 7 — FEE-03(b) `fee_share_mixed_script_differs_from_uniform_baseline` (CONTEXT D-126)

**Insert location:** Inline in `tx.rs::tests` block after Add 6.

**New code pattern:**

```rust
// coordinator/src/bitcoin/tx.rs — PHASE 20 (Task 9)
#[test]
fn fee_share_mixed_script_differs_from_uniform_baseline() {
    // mixed-script (n=3, fee_rate=2, output_type=P2WPKH):
    // estimated_vsize = 10 + (68 + 58 + 91) + 6*31 = 10 + 217 + 186 = 413 vB
    // total_fee = 413 * 2 = 826 sats
    // fee_share = 826 / 3 = 275 sats
    // diff per participant: 275 - 266 = 9 sats (well above the ≥1 sat requirement)
    let n = 3;
    let denomination_sats = 1_000_000;
    let types = [ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh];
    let inputs = make_inputs_typed(&types, 1_100_000);
    let outputs = make_outputs(n);
    let psbt = build_coinjoin_psbt(
        &inputs, &outputs, denomination_sats, 2,
        ScriptType::P2wpkh,                   // single output type per round (D-37)
    ).unwrap();
    let total_in: u64 = inputs.iter().map(|i| i.value_sats).sum();
    let total_out: u64 = psbt.unsigned_tx.output.iter().map(|o| o.value.to_sat()).sum();
    let fee_share = (total_in - total_out) / (n as u64);
    assert!(
        fee_share.saturating_sub(266) >= 1,
        "Mixed-script fee_share must exceed P2WPKH-only baseline by >=1 sat \
         (got {fee_share}; would be 266 if per-script branch silently reverted)"
    );
}
```

---

### File: `coordinator/src/bitcoin/fee.rs` (modified)

#### Modify — `estimate_fee_share` signature + body

**Modify location:** Replace the entire body at `fee.rs:11-18`.

**Analog (in-file, current canonical helper):**

```rust
// coordinator/src/bitcoin/fee.rs:11-18 — ORIGINAL (load-bearing WR-04 surface)
pub fn estimate_fee_share(n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 {
        return 0;
    }
    let estimated_vsize = 10 + n * 68 + n * 2 * 31;
    (estimated_vsize * fee_rate) / n
}
```

**New code pattern (per CONTEXT D-122 + RESEARCH Example 2):**

```rust
// coordinator/src/bitcoin/fee.rs — PHASE 20 (Task 6)
use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes};
use crate::config::BipConfig;

/// Worst-case pre-registration fee share estimate. Used at INPUT_REG time
/// before the coordinator knows which script types will actually register —
/// MUST overestimate so `build_coinjoin_psbt`'s real per-input weight cannot
/// exceed what `validate_utxo` already required from each participant.
///
/// **Privacy property:** using the max-across-allowed-set is uniform regardless
/// of participant registration order — never leaks which script types are
/// currently registered (see CONTEXT §specifics).
pub fn estimate_fee_share(bip_config: &BipConfig, n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 {
        return 0;
    }
    let worst_input_vb = bip_config.allowed_set()
        .map(script_input_vbytes)
        .max()
        .expect("BipConfig::validate ensures at least one allow_* flag is true");
    let output_vb = script_output_vbytes(bip_config.output_script_type);
    let estimated_vsize = 10 + worst_input_vb * n + output_vb * 2 * n;
    (estimated_vsize * fee_rate) / n
}
```

**WR-04 invariant:** Both call sites (`handlers.rs:165` and `handlers.rs:505`) MUST continue calling this single function. Do not inline the formula at any call site (Risk 3 in RESEARCH §Risks).

**RESEARCH Wave 0 Gap:** fee.rs currently has no `#[cfg(test)] mod tests`. Plan-phase MAY add a unit test (`worst_case_picks_max_allowed_input_vbyte` or similar) co-located with the new body to cover the formula at the unit tier. Optional but recommended for symmetry with `tx.rs::tests`.

---

### File: `coordinator/src/bitcoin/utxo.rs` (modified)

#### Modify 1 — Extend `UtxoDetails` with `script_type` field

**Modify location:** `utxo.rs:37-40`.

**Analog (in-file, current struct):**

```rust
// coordinator/src/bitcoin/utxo.rs:37-40 — ORIGINAL
pub struct UtxoDetails {
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,
}
```

**New code pattern:**

```rust
// coordinator/src/bitcoin/utxo.rs — PHASE 20 (Task 2)
pub struct UtxoDetails {
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,
    /// Coordinator-derived script type from `detect_script_type(script_pubkey)`,
    /// computed inside `dispatch_ownership_proof` (CRIT-01 invariant: NEVER
    /// client-declared). Threaded through to the fee path via `RegisteredInput`
    /// → `ParticipantInput`.
    pub script_type: ScriptType,
}
```

#### Modify 2 — Return `derived` from `validate_utxo`

**Modify location:** `utxo.rs:116` — extend the `Ok(...)` return.

**Analog (in-file, line 99-116):**

```rust
// coordinator/src/bitcoin/utxo.rs:99-116 — ORIGINAL
let derived = dispatch_ownership_proof(
    &script_pubkey,
    ownership_proof,
    network,
    bip_config,
    message.as_bytes(),
)
.map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;

// D-50: structured success log. Fields = round_id (Display) + script_type
// (Debug) ONLY. No outpoint, address, witness, or pubkey bytes (PRIV-02).
tracing::info!(
    round_id = %round_id,
    script_type = ?derived,
    "ownership proof verified"
);

Ok(UtxoDetails { value_sats, script_pubkey })
```

**New code pattern (single-line surgical change at line 116):**

```rust
// coordinator/src/bitcoin/utxo.rs:116 — PHASE 20
Ok(UtxoDetails { value_sats, script_pubkey, script_type: derived })
```

**Provenance discipline (CRIT-01):** `derived` is the value already produced by `dispatch_ownership_proof` at `utxo.rs:99`. No new call sites for `detect_script_type` are added. The same value already feeds the structured log at `utxo.rs:110-113`; Phase 20 just stops discarding it from the return shape.

---

### File: `coordinator/src/round/state.rs` (modified)

#### Modify — Add `script_type: ScriptType` field to `RegisteredInput` with `#[zeroize(skip)]`

**Modify location:** `state.rs:53-68` — add new field after `value_sats`.

**Analog (in-file, sibling public-chain-data field):**

```rust
// coordinator/src/round/state.rs:53-68 — ORIGINAL (existing #[zeroize(skip)] precedent)
#[derive(Debug, Clone, Zeroize)]
pub struct RegisteredInput {
    /// String representation of the UTXO outpoint ("txid:vout")
    pub utxo_str: String,
    pub change_address: String,
    /// SHA-256 of the blind signature token hash (for double-registration prevention)
    pub blind_sig_hash: [u8; 32],
    /// On-chain script_pubkey of this UTXO, as returned by Bitcoin Core gettxout
    /// during validate_utxo. Used by signing.rs to populate the correct witness_utxo
    /// in the PSBT — clients no longer need to overwrite it locally with unverified values.
    /// Public chain data, no privacy concern, skipped from zeroize.
    #[zeroize(skip)]
    pub script_pubkey: ScriptBuf,
    /// On-chain value of this UTXO in satoshis, as returned by gettxout.
    pub value_sats: u64,
}
```

**New code pattern (mirror exactly the `script_pubkey` annotation precedent):**

```rust
// coordinator/src/round/state.rs — PHASE 20 (Task 3)
#[derive(Debug, Clone, Zeroize)]
pub struct RegisteredInput {
    pub utxo_str: String,
    pub change_address: String,
    pub blind_sig_hash: [u8; 32],
    #[zeroize(skip)]
    pub script_pubkey: ScriptBuf,
    pub value_sats: u64,
    /// Coordinator-derived script type (FEE-02 plumbing). Mirrors `script_pubkey`
    /// in provenance and zeroize policy: public chain data, derivable from
    /// `script_pubkey`, no key material, no privacy concern.
    #[zeroize(skip)]
    pub script_type: shared::bip322::ScriptType,
}
```

**Required imports:** Add `use shared::bip322::ScriptType;` near the existing `use bitcoin::ScriptBuf;` at `state.rs:4`.

**Test impact (test fixture refresh):** Both `register_input.rs::tests` (lines 192-198) and `signing.rs::tests` (`make_signing_state` at line 335-341) construct `RegisteredInput` literals — both need the new field. Default to `ScriptType::P2wpkh` (matches the test SPK shape — `bitcoin::ScriptBuf::new()` is also acceptable since fee math doesn't run in those tests, but P2WPKH is the documented test convention).

---

### File: `coordinator/src/round/input_reg.rs` (modified)

#### Modify — Thread `script_type` through `register_input` signature + write site

**Modify location:** `input_reg.rs:35-43` (signature) + `input_reg.rs:82-88` (write site).

**Analog (in-file, existing parallel field threading):**

```rust
// coordinator/src/round/input_reg.rs:35-43 — ORIGINAL (existing utxo_script_pubkey/utxo_value_sats threading)
pub fn register_input(
    state: &mut RoundState,
    utxo: &OutPoint,
    blinded_token_bytes: &[u8],
    change_address: &str,
    utxo_script_pubkey: ScriptBuf,
    utxo_value_sats: u64,
    round_id_str: &str,
) -> Result<InputRegResult, ApiError> {
```

```rust
// coordinator/src/round/input_reg.rs:82-88 — ORIGINAL (RegisteredInput insertion site)
inner.registered_inputs.insert(utxo_str.clone(), RegisteredInput {
    utxo_str: utxo_str.clone(),
    change_address: change_address.to_string(),
    blind_sig_hash,
    script_pubkey: utxo_script_pubkey,
    value_sats: utxo_value_sats,
});
```

**New code pattern (add parallel `utxo_script_type` parameter + write into the new field):**

```rust
// coordinator/src/round/input_reg.rs — PHASE 20 (Task 3)
pub fn register_input(
    state: &mut RoundState,
    utxo: &OutPoint,
    blinded_token_bytes: &[u8],
    change_address: &str,
    utxo_script_pubkey: ScriptBuf,
    utxo_value_sats: u64,
    utxo_script_type: shared::bip322::ScriptType,   // NEW
    round_id_str: &str,
) -> Result<InputRegResult, ApiError> {
    // ... (rest unchanged)

    // Register the input
    inner.registered_inputs.insert(utxo_str.clone(), RegisteredInput {
        utxo_str: utxo_str.clone(),
        change_address: change_address.to_string(),
        blind_sig_hash,
        script_pubkey: utxo_script_pubkey,
        value_sats: utxo_value_sats,
        script_type: utxo_script_type,              // NEW
    });
```

**Test impact:** `register_input_is_sync_and_succeeds` (line 155) + `register_input_rejects_double_registration` (line 204) — both call `register_input(...)` directly. Both need the new arg — pass `ScriptType::P2wpkh` (default test type, matches `ScriptBuf::new()` placeholder).

---

### File: `coordinator/src/api/handlers.rs` (modified)

#### Modify 1 — POST /round/input: pass `utxo_details.script_type` to `register_input`

**Modify location:** `handlers.rs:256-263` — extend the `register_input(...)` call.

**Analog (in-file, current call):**

```rust
// coordinator/src/api/handlers.rs:256-263 — ORIGINAL
let result = register_input(
    &mut guard,
    &utxo,
    &blinded_token_bytes,
    &req.change_address,
    utxo_details.script_pubkey.clone(),
    utxo_details.value_sats,
    &round_id_str,
)
```

**New code pattern:**

```rust
// coordinator/src/api/handlers.rs — PHASE 20 (Task 3)
let result = register_input(
    &mut guard,
    &utxo,
    &blinded_token_bytes,
    &req.change_address,
    utxo_details.script_pubkey.clone(),
    utxo_details.value_sats,
    utxo_details.script_type,                       // NEW (Copy type, no clone needed)
    &round_id_str,
)
```

#### Modify 2 — `estimate_fee_share` callsites pass `&state.config.bip`

**Modify locations:** `handlers.rs:165` (`fee_share_pre_lock`) and `handlers.rs:505` (`fee_per_participant_sats`).

**Analog (in-file, current callsites):**

```rust
// coordinator/src/api/handlers.rs:165 — ORIGINAL
let fee_share_pre_lock = estimate_fee_share(max_participants_snap, fee_rate_snap);
```
```rust
// coordinator/src/api/handlers.rs:504-505 — ORIGINAL
let n = participant_inputs.len() as u32;
let fee_per_participant_sats = estimate_fee_share(n, fee_rate);
```

**New code pattern (both call sites mechanically updated):**

```rust
// coordinator/src/api/handlers.rs:165 — PHASE 20 (Task 6)
let fee_share_pre_lock = estimate_fee_share(&state.config.bip, max_participants_snap, fee_rate_snap);
```
```rust
// coordinator/src/api/handlers.rs:505 — PHASE 20 (Task 6)
let fee_per_participant_sats = estimate_fee_share(&state.config.bip, n, fee_rate);
```

**Sibling pattern precedent:** `bitcoin_network_for_validate = parse_bitcoin_network(&state.config.network.bitcoin_network)` at `handlers.rs:177` shows the established style for threading nested config sub-structs into helpers — Phase 20 follows the same shape.

#### Modify 3 — `build_coinjoin_psbt` call in `get_tx` passes `output_script_type`

**Modify location:** `handlers.rs:491-499` (`build_coinjoin_psbt(...)` in `get_tx`) — also pass `state.config.bip.output_script_type` and write `script_type: reg.script_type` into each `ParticipantInput`.

**Analog (in-file, current call):**

```rust
// coordinator/src/api/handlers.rs:475-481 — ORIGINAL (ParticipantInput construction site)
participant_inputs.push(ParticipantInput {
    outpoint,
    value_sats: reg.value_sats,
    script_pubkey: reg.script_pubkey.clone(),
    change_address: change_script,
});
```

**New code pattern:**

```rust
// coordinator/src/api/handlers.rs — PHASE 20 (Task 4)
participant_inputs.push(ParticipantInput {
    outpoint,
    value_sats: reg.value_sats,
    script_pubkey: reg.script_pubkey.clone(),
    change_address: change_script,
    script_type: reg.script_type,                   // NEW (Copy type)
});
// ... later in get_tx:
let psbt = build_coinjoin_psbt(
    &participant_inputs,
    &participant_outputs,
    denomination_sats,
    fee_rate,
    state.config.bip.output_script_type,            // NEW
).map_err(...)?;
```

---

### File: `coordinator/src/round/signing.rs` (modified)

#### Modify — `ParticipantInput` construction in `assemble_and_broadcast` + `build_coinjoin_psbt` call

**Modify location:** `signing.rs:124-129` (ParticipantInput) + `signing.rs:145-154` (build_coinjoin_psbt call).

**Analog (in-file, current construction):**

```rust
// coordinator/src/round/signing.rs:124-129 — ORIGINAL
participant_inputs.push(ParticipantInput {
    outpoint,
    value_sats: reg.value_sats,
    script_pubkey: reg.script_pubkey.clone(),
    change_address: change_script,
});
```

```rust
// coordinator/src/round/signing.rs:145-154 — ORIGINAL
let mut psbt = build_coinjoin_psbt(
    &participant_inputs,
    &participant_outputs,
    config.coordinator.denomination_sats,
    config.coordinator.fee_rate_sat_per_vbyte,
).map_err(|e| ApiError {
    code: ErrorCode::BroadcastRejected,
    message: format!("PSBT construction failed: {e}"),
    round_id: Some(round_id_str.to_string()),
})?;
```

**New code pattern:**

```rust
// coordinator/src/round/signing.rs — PHASE 20 (Task 4 + 5)
participant_inputs.push(ParticipantInput {
    outpoint,
    value_sats: reg.value_sats,
    script_pubkey: reg.script_pubkey.clone(),
    change_address: change_script,
    script_type: reg.script_type,                   // NEW (Copy type)
});

// ... later:
let mut psbt = build_coinjoin_psbt(
    &participant_inputs,
    &participant_outputs,
    config.coordinator.denomination_sats,
    config.coordinator.fee_rate_sat_per_vbyte,
    config.bip.output_script_type,                  // NEW (same source as get_tx — WR-04 byte-identical)
).map_err(|e| ApiError { ... })?;
```

**WR-04 invariant:** The `output_script_type` value MUST come from the SAME source (`config.bip.output_script_type` / `state.config.bip.output_script_type`) at both call sites. Risk 3 in RESEARCH §Risks names this as the load-bearing path — both PSBTs must be byte-identical.

---

### File: `coordinator/src/config.rs` (modified)

#### Add — `BipConfig::allowed_set()` helper (CONTEXT D-122a / CD-43)

**Insert location:** Inside `impl BipConfig { ... }` at `config.rs:189-254` — add after `supported` at line 217, before `validate` at line 230.

**Analog (in-file, sibling methods):**

```rust
// coordinator/src/config.rs:191-217 — ANALOGS
pub fn allows(&self, st: ScriptType) -> bool {
    match st {
        ScriptType::P2wpkh => self.allow_p2wpkh,
        ScriptType::P2tr => self.allow_p2tr,
        ScriptType::P2shP2wpkh => self.allow_p2sh_p2wpkh,
    }
}

pub fn supported(&self) -> Vec<ScriptType> {
    let mut v = Vec::new();
    if self.allow_p2sh_p2wpkh {
        v.push(ScriptType::P2shP2wpkh);
    }
    if self.allow_p2tr {
        v.push(ScriptType::P2tr);
    }
    if self.allow_p2wpkh {
        v.push(ScriptType::P2wpkh);
    }
    v
}
```

**New code pattern (per CONTEXT CD-43 default — `impl Iterator`):**

```rust
// coordinator/src/config.rs — PHASE 20 (Task 7)
/// Iterator over allowed input ScriptTypes. Order is unspecified — callers
/// MUST NOT depend on iteration order (use `supported()` for the alphabetical
/// canonical order needed by PKARR advertisement).
///
/// Used by `fee::estimate_fee_share` to compute worst-case-across-allowed-set
/// vbytes; iteration order is irrelevant because `max(...)` is commutative.
pub fn allowed_set(&self) -> impl Iterator<Item = ScriptType> + '_ {
    [
        (self.allow_p2wpkh, ScriptType::P2wpkh),
        (self.allow_p2tr, ScriptType::P2tr),
        (self.allow_p2sh_p2wpkh, ScriptType::P2shP2wpkh),
    ]
    .into_iter()
    .filter_map(|(allowed, st)| if allowed { Some(st) } else { None })
}
```

**RISK-5 note:** This method's iteration order differs from `supported()` (alphabetical). The `'static` lifetime on the array means `+ '_` lifetime in the return type is required for non-borrowing iteration, but the method takes `&self` and returns owned `ScriptType` values (which are `Copy`), so no actual borrow flows through.

**Wave 0 test gap (optional):** A unit test asserting `allowed_set()` yields the expected subset for various `BipConfig` configurations would mirror the existing `bip_config_supported_skips_disallowed` test at `config.rs:510-522`. Plan-phase decides whether to add this for parity.

---

## Shared Patterns

### Pattern 1 — CRIT-01 single-source-of-truth derivation

**Source:** `coordinator/src/bitcoin/utxo.rs:99` (`let derived = dispatch_ownership_proof(...)`)
**Apply to:** Every site that reads or stores `ScriptType` in the Phase 20 plumbing chain (UtxoDetails → RegisteredInput → ParticipantInput).

**Discipline:**
- `detect_script_type` is called EXACTLY ONCE per UTXO, at `utxo.rs:163` (v=1) or `utxo.rs:184` (v=2) inside `dispatch_ownership_proof`.
- The returned `derived` value flows through `UtxoDetails.script_type` (returned at `utxo.rs:116`) → `RegisteredInput.script_type` (written at `input_reg.rs:82-88`) → `ParticipantInput.script_type` (constructed at `handlers.rs:475` and `signing.rs:124`).
- No new call site for `detect_script_type` is added in Phase 20.
- The value never traverses the wire — no `Serialize`/`Deserialize` impact.

### Pattern 2 — `#[zeroize(skip)]` on public-chain-data fields

**Source:** `coordinator/src/round/state.rs:60-65` (`#[zeroize(skip)] pub script_pubkey: ScriptBuf`)
**Apply to:** New `RegisteredInput.script_type` field.

```rust
// coordinator/src/round/state.rs:60-65 — ANALOG
#[zeroize(skip)]
pub script_pubkey: ScriptBuf,
```

**Discipline:** ScriptType is derivable from the already-skipped `script_pubkey` (which is itself public chain data). Zeroing it on round-end provides zero privacy benefit; a blockchain explorer reveals the same bytes. Mirror the existing annotation exactly.

### Pattern 3 — WR-04 single canonical fee helper

**Source:** `coordinator/src/bitcoin/fee.rs:11-18` (`pub fn estimate_fee_share`)
**Apply to:** Every read of fee math; Phase 20 preserves the helper as the SOLE source of truth.

**Discipline:**
- Both `handlers.rs::get_tx` (display path) and `signing.rs::assemble_and_broadcast` (broadcast path) MUST call `estimate_fee_share(&state.config.bip, n, fee_rate)`.
- Do NOT inline the formula at either call site.
- Both `build_coinjoin_psbt` callers MUST pass the same `output_script_type` value (`config.bip.output_script_type`) — divergence breaks the sighash that clients sign against.
- The integer-floor `fee_share = total_fee / n` in `build_coinjoin_psbt` MUST be preserved verbatim (D-125 byte-equality).

### Pattern 4 — Const-fn lookup-table for spec-defined data

**Source:** `coordinator/src/bitcoin/tx.rs:10-15` (the three existing fee-overhead consts the Phase 20 const fns replace)
**Apply to:** New `script_input_vbytes` + `script_output_vbytes`.

**Discipline:**
- BIP-141/BIP-341 numbers are spec-stable (9 years for BIP-141, 4 years for BIP-341). Encode as `const fn` with match arms over `ScriptType`.
- Each return arm carries a 4-6 line BIP-141 derivation comment inline. These comments are the audit-charter artifact Phase 21 cites.
- The 6 unit tests at `tx.rs::tests` (added in Plan 20-01 Task 1) pin the table against its derivation — refactors that silently change a value break a test.
- Do NOT use the rust-bitcoin `Weight` API at runtime (CONTEXT D-124 rejects it: const lookup is the chosen design for audit prose value).

### Pattern 5 — Sibling-field plumbing in struct + signature + callsite trios

**Source:** The existing `script_pubkey` / `value_sats` plumbing added in a prior phase — visible at `utxo.rs:37-40` (struct), `input_reg.rs:35-43` (signature), `input_reg.rs:82-88` (write site), `handlers.rs:256-263` (call site), `signing.rs:124-129` (read site)
**Apply to:** Phase 20's new `script_type` field follows the EXACT same plumbing path. Every diff hunk has a one-line analog in the existing diff that added `script_pubkey`.

**Discipline:** When extending an internal struct with a new field that's coordinator-derived from on-chain data, replicate the precedent set by `script_pubkey`: same struct, same signature pass-through, same write-site naming convention (`utxo_script_type` mirrors `utxo_script_pubkey`).

---

## Cross-Phase Invariants (planner gate prose)

Phase 20 preserves four invariants. Each plan boundary MUST verify all four are green:

| Invariant | Verification Command | Expected |
|-----------|----------------------|----------|
| v1.3 P2WPKH (8 tests) | `cargo test --test integration full_round` | 8/8 green, ~42s |
| v1.4 mixed-script (1 test) | `cargo test --test integration mixed_script_e2e` | 1/1 green |
| WR-04 single canonical fee helper | Code review: `grep -rn "estimate_fee_share\|10 + n \* 68" coordinator/src/` returns ONLY the fee.rs definition + the two handlers.rs callsites | No inline math at callsites |
| CRIT-01 coordinator-derived ScriptType | Code review: `grep -rn "detect_script_type" coordinator/src/ shared/src/` returns only the existing utxo.rs:163,184 sites | No new call sites |

Per CONTEXT line 44: if any invariant goes red, REPAIR-01 lesson #4 applies — abandon the structured plan and pivot to `/gsd:debug`.

---

## No Analog Found

None. Every modified file has a strong in-tree analog (often in the same file or a sibling file in the same module). This is consistent with RESEARCH §Architecture Patterns: "No structural change" — Phase 20 is pure data plumbing + a 6-cell lookup table extending an established weight-math precedent.

---

## CD-45 Resolution (refresh hardcoded amount assertions in mixed_script_e2e.rs)

RESEARCH already resolved this via grep: `tests/integration/mixed_script_e2e.rs` contains ZERO fee_share / numeric-fee assertions. The test asserts only (a) broadcast txid appears in regtest mempool, (b) CoinJoin tx has exactly 3 denomination outputs of 100_000 sats, (c) input script type set-equality. None of these are affected by Phase 20's fee-math refinement. **No refresh needed.**

Verified by:
```bash
grep -n "fee_share\|fee_per_participant\|266\|275" tests/integration/mixed_script_e2e.rs
# (no output — confirms zero matches)
```

---

## Metadata

**Analog search scope:** `coordinator/src/bitcoin/`, `coordinator/src/round/`, `coordinator/src/api/`, `coordinator/src/config.rs`, `shared/src/bip322/mod.rs`, `tests/integration/{full_round,mixed_script_e2e}.rs`
**Files scanned (full reads):** `tx.rs` (224 LOC), `fee.rs` (18 LOC), `utxo.rs` (479 LOC), `state.rs` (325 LOC), `input_reg.rs` (233 LOC), `signing.rs` (400+ LOC), `handlers.rs` (key call site sections only), `config.rs` (647 LOC)
**Pattern extraction date:** 2026-05-31
