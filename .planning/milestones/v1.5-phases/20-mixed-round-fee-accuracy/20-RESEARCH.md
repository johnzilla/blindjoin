# Phase 20: Mixed-Round Fee Accuracy - Research

**Researched:** 2026-05-31
**Domain:** Bitcoin fee estimation (BIP-141 per-script witness weights) + coordinator state-struct plumbing
**Confidence:** HIGH (all factual claims verified against source code at HEAD; only the P2TR rounding policy is operator judgment)

## Summary

Phase 20 replaces a 2-line hardcoded P2WPKH-only fee approximation with a 3-script weight table and threads `ScriptType` through three struct boundaries (`UtxoDetails` → `RegisteredInput` → `ParticipantInput`). The work is tightly bounded: 2 source files contain every hardcoded fee constant (`coordinator/src/bitcoin/{tx.rs,fee.rs}`), 3 structs gain one field each, 2 call sites construct `ParticipantInput`, 2 call sites invoke `estimate_fee_share`, and 1 call site already derives `ScriptType` from on-chain `script_pubkey` but throws it away (`coordinator/src/bitcoin/utxo.rs:99,116` — the value comes from `dispatch_ownership_proof` and never escapes `validate_utxo`'s return shape).

The CRIT-01 invariant is preserved by construction: the new `ParticipantInput.script_type` value originates exactly once at `utxo.rs:99` (the existing `let derived = dispatch_ownership_proof(...)`), flows through `UtxoDetails.script_type` (returned at `utxo.rs:116`), is stored in `RegisteredInput.script_type` (written at `input_reg.rs:82`), and is copied into `ParticipantInput.script_type` at the two construction sites (`handlers.rs:475`, `signing.rs:124`). There is no point in this path where a client-supplied field is read or trusted; the field doesn't even need to traverse the wire.

The mixed_script_e2e test asserts only on broadcast-success and denomination-output-count (`100_000` sats each) — there is NO `fee_share` numeric assertion in `tests/integration/mixed_script_e2e.rs`, so CONTEXT CD-45's "refresh hardcoded amounts if any" task resolves to "no refresh needed." Similarly, `tests/integration/full_round.rs` has zero `fee_share` numeric assertions — the only load-bearing fee-value test will be the new `fee_share_p2wpkh_only_matches_v14_baseline` regression test that Phase 20 introduces.

**Primary recommendation:** Execute CONTEXT D-127's 10-task single plan as-written. The CONTEXT file is unusually complete (line numbers verified, math derived, divergences documented inline). Research surfaces zero new ambiguities; the only judgment call is CD-42 (P2TR vbyte = 57 vs 58), and STATE.md's binding "round UP" policy makes the answer **58** with the divergence-from-roadmap noted in the source comment.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| BIP-141 vbyte lookup (`script_input_vbytes` / `script_output_vbytes`) | Coordinator (fee math) | — | Pure data table; coordinator is the only fee-computing party |
| `ScriptType` derivation from on-chain SPK | Coordinator (`validate_utxo`) | — | CRIT-01: must NOT be client-declared; chain-derived only |
| `ScriptType` storage across round state | Coordinator (`RegisteredInput`) | — | In-memory round state, not wire-serialized |
| `ParticipantInput` construction | Coordinator (`get_tx` + `assemble_and_broadcast`) | — | Both must produce byte-identical PSBTs (WR-04) |
| Pre-registration fee estimate (worst-case) | Coordinator (`fee.rs::estimate_fee_share`) | — | Operator-config-driven, applies before any input is known |
| Per-round PSBT fee math | Coordinator (`tx.rs::build_coinjoin_psbt`) | — | Reads actual per-input ScriptType from the registered set |

No tier shifts in this phase — every change lives in the coordinator. The client is uninvolved: the PSBT it receives carries the new (more accurate) `change` amount but the witness/sighash semantics are unchanged.

## User Constraints (from CONTEXT.md)

### Locked Decisions

These flow from REQUIREMENTS.md / STATE.md / Phase 16-18 outputs and are NOT re-litigated:

- **Uniform `fee_share = total_fee / N`** — Per-input variable fee_share is REQUIREMENTS.md `Future requirements` (changes wire protocol, separate v1.6+ milestone).
- **Single-output-type per round** — Phase 16 D-37 + REQUIREMENTS.md `Out of v1.5 scope but not anti-features`. `script_output_vbytes` takes ONE `ScriptType` (the configured `output_script_type`) and applies to all denomination + change outputs uniformly.
- **V1.4-CRIT-01 (coordinator-derived script_type)** — `ScriptType` is derived from the on-chain `script_pubkey` returned by Bitcoin Core's `gettxout`, NEVER from client-supplied wire data. `validate_utxo` already calls `detect_script_type(spk)` inside `dispatch_ownership_proof` (utxo.rs:163,184); Phase 20 returns that value through `UtxoDetails` instead of discarding it.
- **Conservative rounding UP** — STATE.md §v1.5 design notes line 2: "Rounding policy needs to be conservative (round UP) so the coordinator doesn't underpay fees on a mixed round." Binds D-124 (P2TR = 58, not 57).
- **v1.3 + v1.4 cross-phase invariants** — `full_round` 8/8 + `mixed_script_e2e` 1/1 stay green at every plan boundary.

CONTEXT also locks 5 implementation decisions (D-122 through D-127):

- **D-122/D-122a/D-122b**: `fee.rs::estimate_fee_share` signature becomes `(bip_config: &BipConfig, n: u32, fee_rate: u64) -> u64`; body uses `max(script_input_vbytes across allowed_set()) * n + script_output_vbytes(output_script_type) * 2 * n + 10`. `BipConfig::allowed_set()` helper added. Two call sites updated (`handlers.rs:165`, `handlers.rs:505`).
- **D-123/D-123a/D-123b**: Full plumbing `UtxoDetails → RegisteredInput → ParticipantInput`. `RegisteredInput.script_type` is `#[zeroize(skip)]`. `UtxoDetails` extended (NOT replaced).
- **D-124/D-124a/D-124b/D-124c**: Per-script vbyte table as `const fn`/`pub fn` with 4-6 line BIP-141 derivation comments inline. Inputs: P2WPKH 68 / P2TR 58 (UP from 57.5) / P2SH-P2WPKH 91. Outputs: P2WPKH 31 / P2TR 43 / P2SH-P2WPKH 32. Six unit tests pin the table.
- **D-125**: `fee_share_p2wpkh_only_matches_v14_baseline` hardcodes baseline `266` with inline derivation comment. Math: `(10 + 3*68 + 6*31) * 2 / 3 = 800/3 = 266`.
- **D-126**: `fee_share_mixed_script_differs_from_uniform_baseline` asserts `fee_share - 266 >= 1`. At fee_rate=2 sat/vB the derived delta is **9 sats/participant** (mixed-script vsize = `10 + (68+58+91) + 6*31 = 413`; total_fee = 826; fee_share = 275; 275 - 266 = 9).
- **D-127**: ONE plan (`20-01-PLAN.md`) with 10 sequenced tasks (see CONTEXT line 125-135).

### Claude's Discretion

- **CD-40**: Location of `script_input_vbytes` / `script_output_vbytes` — default fee.rs; fallback tx.rs.
- **CD-41**: `const fn` vs plain `pub fn` — default `const fn`.
- **CD-42**: P2TR vbyte (57 vs 58) — STATE.md policy is "round UP", so **58** is correct; document the divergence from ROADMAP SC#1's literal "57" inline.
- **CD-43**: `BipConfig::allowed_set` name/return shape — default `pub fn allowed_set(&self) -> impl Iterator<Item = ScriptType>`.
- **CD-44**: Test fixture amount for FEE-03(a) — default `make_inputs(3, 1_100_000)` (existing helper).
- **CD-45**: `mixed_script_e2e.rs` amount-refresh — **RESOLVED by this research**: no hardcoded fee_share values exist in that file; no refresh needed.

### Deferred Ideas (OUT OF SCOPE)

- Validating `change_address` script_type matches `bip.output_script_type` — v1.6+ if Phase 21 audit charter flags.
- Per-input variable `fee_share` — REQUIREMENTS.md `Future requirements`, separate v1.6+ milestone.
- Mixed output script types per participant (Wasabi 2.0.3-style) — REQUIREMENTS.md `Out of v1.5 scope`.
- **B-03 dynamic fee estimation** (mempool-aware polling + RBF) — orthogonal v1.6+ work.
- Compute vbytes at startup via `bitcoin::Weight` + assert against pinned consts — overkill for v1.5.
- Promote `script_*_vbytes` to `shared/` crate — v1.6+ if client needs fee preview.
- `BipConfig::allowed_set` cached `Vec` instead of method — v1.6+ if usage patterns warrant.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FEE-01 | `script_input_vbytes(ScriptType) -> u64` + `script_output_vbytes(ScriptType) -> u64` in `coordinator/src/bitcoin/tx.rs` (replacing `INPUT_WEIGHT_VBYTES = 68` and `OUTPUT_WEIGHT_VBYTES = 31` at tx.rs:11,13); conservative-rounded-UP BIP-141 values; derivation inline | §Current State of `tx.rs`, §BIP-141 vbyte references, §D-124 table |
| FEE-02 | `ParticipantInput.script_type: ScriptType` added; `build_coinjoin_psbt` sums per-input weights and uses `script_output_vbytes(output_script_type)`; CRIT-01 preserved (coordinator-derived from on-chain SPK) | §`ScriptType` provenance, §`ParticipantInput` shape, §`validate_utxo` wiring, §Plumbing path verified end-to-end |
| FEE-03 | Two regression tests in `tx.rs::tests`: (a) `fee_share_p2wpkh_only_matches_v14_baseline` byte-equal to pre-Phase-20; (b) `fee_share_mixed_script_differs_from_uniform_baseline` differs by ≥1 sat/participant | §Existing tx.rs test module structure, §v1.4 baseline math (266), §mixed-script math (275 → diff = 9 sats) |

## Standard Stack

Phase 20 adds no new dependencies. All work uses crates already in `coordinator/Cargo.toml`:

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| bitcoin | 0.32 (workspace pin) | `bitcoin::Witness`, `bitcoin::Script` (only via existing `ScriptType` enum) | Already pinned at workspace root; no new bitcoin API used |
| shared (path dep) | local | `shared::bip322::ScriptType` enum | Already imported in `coordinator/src/config.rs:3` and `coordinator/src/bitcoin/utxo.rs:5` |
| serde / thiserror | (transitive) | No new use | n/a |

**Verification (Bash):**
```bash
grep -E "^bitcoin\b" Cargo.toml
# Output: bitcoin = { version = "0.32", features = ["serde"] }  [VERIFIED: source]
```

**No new packages → no Package Legitimacy Audit needed.** Phase 20 is a pure refactor + test addition within the existing dep graph.

## Architecture Patterns

### System Architecture Diagram (Phase 20 data flow for `ScriptType`)

```
Client → POST /round/input → handlers.rs::post_input
                              │
                              ▼
                       validate_utxo (utxo.rs:62)
                              │
                              ├─→ rpc.gettxout(...)               [returns txout.script_pubkey.hex]
                              ├─→ parse_script_pubkey_from_txout  [→ ScriptBuf]
                              ├─→ dispatch_ownership_proof        [→ ScriptType derived via detect_script_type]
                              │                                   ↑ CRIT-01 invariant: derivation happens here, once
                              ▼
                       UtxoDetails { value_sats, script_pubkey, script_type }   ← Phase 20 adds script_type
                              │
                              ▼ (back in handlers.rs, write-locked)
                       register_input (input_reg.rs:35)
                              │
                              ▼
                       RegisteredInput { utxo_str, change_address, blind_sig_hash,
                                         script_pubkey, value_sats, script_type }   ← Phase 20 adds script_type
                              │
                              ▼ stored in inner.registered_inputs HashMap
                              │
        ┌─────────────────────┴─────────────────────┐
        │                                           │
        ▼ (display path)                            ▼ (broadcast path)
   handlers.rs::get_tx                       signing.rs::assemble_and_broadcast
        │                                           │
        ▼                                           ▼
   ParticipantInput { ...,                   ParticipantInput { ...,
                      script_type }                              script_type }   ← Phase 20 adds at both sites
        │                                           │
        └─────────────┬─────────────────────────────┘
                      ▼
              build_coinjoin_psbt(inputs, outputs, denomination_sats,
                                  fee_rate, output_script_type)   ← Phase 20 adds output_script_type
                      │
                      ▼ vsize = TX_OVERHEAD + Σ script_input_vbytes(inp.script_type)
                                            + (n_denom + n_change) * script_output_vbytes(output_script_type)
                      ▼
                  Psbt (byte-identical from both call sites — WR-04 invariant)
```

**Single source of truth for `ScriptType` derivation:** `utxo.rs:99` (`let derived = dispatch_ownership_proof(...)`). Phase 20 adds zero new `detect_script_type` call sites.

### Recommended Project Structure

No structural change. Files modified:

```
coordinator/src/
├── bitcoin/
│   ├── tx.rs           # FEE-01 (script_*_vbytes), FEE-02 (ParticipantInput.script_type,
│   │                   # build_coinjoin_psbt signature + body), FEE-03 (2 regression tests)
│   ├── fee.rs          # estimate_fee_share signature + body (worst-case formula)
│   └── utxo.rs         # UtxoDetails.script_type field; validate_utxo returns derived
├── round/
│   ├── state.rs        # RegisteredInput.script_type (#[zeroize(skip)])
│   ├── input_reg.rs    # register_input: write script_type into RegisteredInput
│   └── signing.rs      # assemble_and_broadcast: read reg.script_type → ParticipantInput
├── api/
│   └── handlers.rs     # 2 fee_share callsites pass &state.config.bip; 1 ParticipantInput
│                       # construction site reads reg.script_type; register_input gets script_type
└── config.rs           # BipConfig::allowed_set() helper added (no struct change)
```

### Pattern 1: Single-source-of-truth derivation
**What:** `ScriptType` is computed once at `utxo.rs:99` and propagated by reference/copy through three structs. No re-derivation.
**When to use:** Anywhere a value crosses multiple data structures but has one canonical computation site.
**Example:**
```rust
// Source: coordinator/src/bitcoin/utxo.rs:99 (existing) [VERIFIED: read at HEAD]
let derived = dispatch_ownership_proof(
    &script_pubkey, ownership_proof, network, bip_config, message.as_bytes(),
).map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;

// Phase 20 change: return `derived` in UtxoDetails (currently discarded after log line)
Ok(UtxoDetails { value_sats, script_pubkey, script_type: derived })
```

### Pattern 2: WR-04 single-canonical fee helper
**What:** `estimate_fee_share` is the ONLY fee-math source. Both `get_tx` (display) and `assemble_and_broadcast` (PSBT) call it.
**When to use:** Whenever two code paths must produce byte-identical fee values (signature integrity depends on it).
**Anti-pattern caught by this rule:** Inlining `estimate_fee_share`'s math in either call site would create a divergence where the displayed fee differs from the broadcast fee — clients sign against a different sighash than what hits the mempool. Phase 20 preserves this; only the formula inside the single helper changes.

### Pattern 3: `#[zeroize(skip)]` on public-chain-data fields
**What:** Fields derivable from on-chain data (or that ARE on-chain data) carry `#[zeroize(skip)]` because zeroing them on round-end provides zero privacy benefit (a blockchain explorer reveals the same bytes).
**Source pattern (existing):**
```rust
// Source: coordinator/src/round/state.rs:64 [VERIFIED: read at HEAD]
#[zeroize(skip)]
pub script_pubkey: ScriptBuf,
```
**Phase 20 application:** `RegisteredInput.script_type` follows the same annotation — derivable from `script_pubkey`, so equivalently public.

### Anti-Patterns to Avoid

- **Re-calling `detect_script_type(script_pubkey)` at the fee-path site** — duplicates the derivation, opens the door to a future bug where the two derivations disagree (e.g., one is called before the SPK is fully validated). The CONTEXT D-123 plumbing path explicitly avoids this.
- **Adding `script_type` to the wire-format `RegisterInputRequest`** — would allow a client to declare ScriptType, then a future careless refactor might trust the client value. Don't open that door. The field lives in coordinator-internal structs only.
- **Inlining the vsize formula at one of the call sites** — breaks WR-04. The `estimate_fee_share` helper must remain the single source of truth.
- **Floor-rounding the P2TR vbyte** — STATE.md mandates UP-rounding. The roadmap's "57" is a planning approximation; **58** is the load-bearing value (raw P2TR = 41 + ceil(66/4) = 41 + 17 = 58).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-script vbyte computation | Custom transaction-size simulator | Hand-derived const fn with BIP-141 derivation in comments | The 6 numbers are spec-defined; a runtime simulator adds complexity for zero accuracy gain. CONTEXT D-124 explicitly chose the const-fn approach for audit-charter prose value. |
| `ScriptType` storage | New enum / parallel `HashMap<OutPoint, ScriptType>` | Add field to existing `RegisteredInput` | Mirrors `script_pubkey` field already present; same lifecycle, same zeroize policy. |
| Worst-case fee_share estimate | Per-script-type cap with leakage protection | `max(script_input_vbytes across allowed_set())` per CONTEXT D-122 | Uniform-across-allowed-set is the privacy-safe choice (no participant-ordering leak). |

**Key insight:** Phase 20 has no "complex problem to solve" — it's data plumbing + a 6-cell lookup table. The CONTEXT correctly resists every temptation to over-engineer (no Weight-API runtime computation, no Wire-protocol changes, no per-input variable fee, no separate state-machine abstraction).

## Current State of `coordinator/src/bitcoin/tx.rs`

Verified by reading the file at HEAD (224 LOC; 2026-05-31). [VERIFIED: source read]

### Hardcoded constants (lines 11, 13)
```rust
/// Estimated weight per input (P2WPKH): 68 vbytes
const INPUT_WEIGHT_VBYTES: u64 = 68;
/// Estimated weight per output (P2WPKH): 31 vbytes
const OUTPUT_WEIGHT_VBYTES: u64 = 31;
/// Fixed TX overhead: 10 vbytes (version, locktime, vin/vout counts)
const TX_OVERHEAD_VBYTES: u64 = 10;
```

**`TX_OVERHEAD_VBYTES = 10` is NOT in scope for deletion** — it's the script-independent transaction overhead (4 version + 4 locktime + 1 vin count varint + 1 vout count varint), correct regardless of script types. Phase 20 keeps it.

### Call sites of the constants

- `tx.rs:67` — `+ n * INPUT_WEIGHT_VBYTES` in `build_coinjoin_psbt`
- `tx.rs:68` — `+ (n + num_change_outputs) * OUTPUT_WEIGHT_VBYTES` in `build_coinjoin_psbt`
- `fee.rs:16` — `let estimated_vsize = 10 + n * 68 + n * 2 * 31;` (inline, not via const — same hardcoded values)

Both files contain the entire hardcoded fee surface. Workspace-wide grep confirms no other crate has 68/31 fee-weight assumptions baked in (verified above with `grep -rn ... | grep -iE "vbyte|weight|fee"`).

### Current `build_coinjoin_psbt` signature (tx.rs:53-58)
```rust
pub fn build_coinjoin_psbt(
    inputs: &[ParticipantInput],
    outputs: &[ParticipantOutput],
    denomination_sats: u64,
    fee_rate_sat_per_vbyte: u64,
) -> Result<Psbt, TxError>
```

**Phase 20 adds:** `output_script_type: ScriptType` (5th param). The fn body changes the vsize formula at tx.rs:66-68 from `n * INPUT_WEIGHT_VBYTES + (n + num_change_outputs) * OUTPUT_WEIGHT_VBYTES` to per-input weight sum + per-output multiply.

### Current `fee_share` computation (tx.rs:69-70)
```rust
let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
let fee_share = total_fee / n;  // each participant pays fee_share
```

**Integer-division behavior:** `total_fee / n` floors the per-participant share. For n=3, fee_rate=2, the v1.4 math is `(400 * 2) / 3 = 800 / 3 = 266` (the 2-sat remainder is absorbed by the coordinator and not redistributed). Phase 20 preserves this floor-on-divide; only `estimated_vsize` changes. This is load-bearing for D-125's byte-exact assertion.

### Current `ParticipantInput` shape (tx.rs:17-23)
```rust
#[derive(Debug, Clone)]
pub struct ParticipantInput {
    pub outpoint: OutPoint,
    pub value_sats: u64,
    pub script_pubkey: ScriptBuf,   // for PSBT input UTXO field
    pub change_address: ScriptBuf,  // their change output script
}
```

**Phase 20 adds:** `pub script_type: shared::bip322::ScriptType`. The struct is `Clone` (already required) and `Debug` (already required); `ScriptType` is `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]` (shared/src/bip322/mod.rs:150-152), so the derives stay valid.

**Wire-crossing:** `ParticipantInput` does NOT cross the wire (no `Serialize`/`Deserialize` derives) — it's a coordinator-internal struct built at PSBT-construction time. Adding `script_type` requires zero wire-protocol changes.

**Client constructs it:** Never. `ParticipantInput` is built only in `handlers.rs::get_tx` (line 475) and `signing.rs::assemble_and_broadcast` (line 124), both coordinator-side, both reading from `RegisteredInput`.

### Existing tx.rs test module structure (tx.rs:131-224)

`#[cfg(test)] mod tests` is inline (NOT a separate file). It contains 5 tests, plus 4 helpers:

| Helper | Lines | Purpose |
|--------|-------|---------|
| `dummy_outpoint(n: u8) -> OutPoint` | 136-141 | Synthesize unique OutPoint per n |
| `p2wpkh_script(byte: u8) -> ScriptBuf` | 143-148 | 22-byte P2WPKH SPK |
| `make_inputs(n: usize, value_sats: u64) -> Vec<ParticipantInput>` | 150-157 | Build n participant inputs (all P2WPKH currently) |
| `make_outputs(n: usize) -> Vec<ParticipantOutput>` | 159-163 | Build n participant outputs |

| Test | Lines | What it asserts |
|------|-------|-----------------|
| `coinjoin_psbt_n_denomination_outputs` | 165-176 | N denom outputs created |
| `coinjoin_psbt_is_valid_psbt` | 178-187 | PSBT round-trips serialize/deserialize |
| `coinjoin_psbt_dust_folded_into_fee` | 189-204 | Sub-dust change folded into fee |
| `coinjoin_psbt_insufficient_funds_error` | 206-212 | Returns InsufficientFunds error |
| `coinjoin_psbt_witness_utxo_set` | 214-223 | witness_utxo populated for SegWit |

**Phase 20 needs to extend `make_inputs`** to take a per-input `script_type` (or add a parallel `make_inputs_mixed(types: &[ScriptType], value_sats: u64)` helper). Existing tests should continue calling the P2WPKH-default variant unchanged — the existing assertions don't depend on weight math.

The two new FEE-03 tests (`fee_share_p2wpkh_only_matches_v14_baseline`, `fee_share_mixed_script_differs_from_uniform_baseline`) follow the same inline pattern.

Phase 20 should also add 6 vbyte-table unit tests per CONTEXT D-124c — one assert per (ScriptType, input|output) combo, pinning the table against its derivation.

## `ScriptType` Enum + `detect_script_type` Provenance

### Enum variants (shared/src/bip322/mod.rs:150-157)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptType {
    P2wpkh,
    P2tr,
    #[serde(rename = "p2sh-p2wpkh")]
    P2shP2wpkh,
}
```

3-variant enum, exactly the 3 script types Phase 20 needs. **No surprise variants.** Wire forms: `"p2wpkh"`, `"p2tr"`, `"p2sh-p2wpkh"` (kebab-case explicit rename on the third). `Copy` semantics → cheap to pass through structs.

### `detect_script_type` (shared/src/bip322/mod.rs:238-248)
```rust
pub fn detect_script_type(spk: &Script) -> Result<ScriptType, Bip322Error> {
    if spk.is_p2wpkh() {
        Ok(ScriptType::P2wpkh)
    } else if spk.is_p2tr() {
        Ok(ScriptType::P2tr)
    } else if spk.is_p2sh() {
        Ok(ScriptType::P2shP2wpkh)
    } else {
        Err(Bip322Error::UnsupportedScriptType)
    }
}
```

**P2SH-disambiguation note (from doc-comment lines 232-237):** `is_p2sh()` alone can't tell P2SH-P2WPKH from raw P2SH-multisig. `detect_script_type` optimistically returns `ScriptType::P2shP2wpkh` for ANY P2SH SPK; the per-script verifier in `p2sh_p2wpkh.rs` delegates to the bip322 crate which performs HASH160 cross-check internally. **For Phase 20's fee path:** by the time `ScriptType` reaches `RegisteredInput`, the bip322 crate's verifier has already vouched for the P2SH-P2WPKH shape (otherwise `validate_utxo` would have errored at `dispatch_ownership_proof`). So the `script_type = P2shP2wpkh` value in `ParticipantInput` is correct by construction.

### Current call sites of `detect_script_type`
```bash
$ grep -rn "detect_script_type" coordinator/src/ shared/src/
```

[VERIFIED: source read] Call sites:
- `coordinator/src/bitcoin/utxo.rs:163` — v=1 branch of dispatcher
- `coordinator/src/bitcoin/utxo.rs:184` — v=2 branch of dispatcher (with cross-check)

Both inside `dispatch_ownership_proof` (utxo.rs:153-196). Both return the derived value as the `Ok(...)` of the dispatcher; both are gated by `bip_config.allows(derived)` (line 164 / 188), so an unsupported-but-syntactically-valid script type rejects before propagating.

**Phase 20 adds ZERO new call sites for `detect_script_type`.** CONTEXT D-123 is explicit: the single source of truth is the existing call at `utxo.rs:99` (the `let derived = dispatch_ownership_proof(...)` site). Phase 20 plumbs the value through `UtxoDetails`, not re-derives it.

### CRIT-01 historical anchor

From `.planning/STATE.md` §"Load-bearing v1.4 invariants v1.5 must preserve":
> **CRIT-01 cross-check** in `coordinator::validate_utxo` — derives `ScriptType` from on-chain `script_pubkey`, not from client declaration. Phase 20 fee estimator must use the same chain-derived `ScriptType` (not the client-declared one), preserving CRIT-01 invariant in the fee path.

From `.planning/phases/19-multi-script-signing-finish/19-CONTEXT.md` §"V1.4-CRIT-01 dispatcher-only invariant" (carried into Phase 19): "Production sign bodies land at `pub(crate) fn sign` so callers still cannot reach `p2tr::sign` from outside the crate; only `verify_simple` and `sign_simple` are `pub`." Phase 20's fee path strengthens this on the verify-side: the `ScriptType` value that drives `script_input_vbytes` originates strictly from the existing `dispatch_ownership_proof` derivation, so no new "trust client" surface is added.

## BIP-141 vbyte References

Cited values verified against multiple sources. Conservative-rounded-UP per STATE.md's binding policy. [CITED + VERIFIED across 3 sources]

### Inputs

| ScriptType | non_witness bytes | witness bytes | vbytes raw | vbytes (UP) | ROADMAP says |
|------------|-------------------|---------------|------------|-------------|--------------|
| P2WPKH | 41 = 32 prev_txid + 4 vout + 1 script_sig_len(0) + 4 sequence | 108 = 1 stack_count + 1 sig_len(72) + 72 DER+SIGHASH_ALL + 1 pk_len(33) + 33 compressed pk | 41 + 27 = **68** | 68 | 68 ✓ |
| P2TR keypath SIGHASH_DEFAULT | 41 (same as P2WPKH) | 66 = 1 stack_count + 1 sig_len(64) + 64 Schnorr | 41 + ceil(66/4) = 41 + 17 = **58** | **58** | **57** (floor — diverges) |
| P2SH-P2WPKH | 64 = 32 + 4 + 1 script_sig_len(23) + 23 redeem-wrapper + 4 sequence | 108 (same as P2WPKH) | 64 + 27 = **91** | 91 | 91 ✓ |

**P2TR resolution (CD-42):** The raw value 57.5 (= 41 + 66/4) is widely cited at 57.5 vB in third-party fee references ([Spark transaction size reference](https://www.spark.money/tools/bitcoin-transaction-size-reference): "P2TR inputs are cheaper to spend at 57.5 vB compared to 68 vB for P2WPKH"). The integer-arithmetic round-up via `(witness + 3) / 4` yields **17** (= ceil(66/4)), so **58 vB** total. STATE.md §v1.5 design notes line 2 binds this: "Rounding policy needs to be conservative (round UP) so the coordinator doesn't underpay fees on a mixed round."

**ROADMAP's "57" is a planning floor approximation.** STATE.md's UP-rounding policy supersedes it (CONTEXT explicitly delegates this resolution to plan-phase via CD-42 with STATE.md as the load-bearing rule). Document the divergence inline in the source comment.

### Outputs (exact — no segwit discount, no rounding)

| ScriptType | bytes | derivation |
|------------|-------|------------|
| P2WPKH | **31** | 8 value + 1 script_len(22) + 22 (OP_0 OP_PUSHBYTES_20 \<20\>) |
| P2TR | **43** | 8 value + 1 script_len(34) + 34 (OP_1 OP_PUSHBYTES_32 \<32\>) |
| P2SH-P2WPKH | **32** | 8 value + 1 script_len(23) + 23 (OP_HASH160 OP_PUSHBYTES_20 \<20\> OP_EQUAL) |

Outputs match the roadmap exactly; no rounding ambiguity.

### Alternative: `rust-bitcoin` Weight API (per CD-42 verification recommendation)

`bitcoin::Weight` and `bitcoin::Transaction::weight()` exist and could compute these at runtime ([rust-bitcoin#1636 — Weight prediction by Kixunil](https://github.com/rust-bitcoin/rust-bitcoin/pull/1636)). However:

- CONTEXT D-124 explicitly chose hand-derived `const fn` over the Weight API for audit-charter prose value ("the per-script weight table is verified at the unit-test layer"; "Phase 21 can cite the source comment directly").
- The 6 constants are spec-defined; a runtime simulator adds boot complexity for zero accuracy benefit.
- The Weight API could be used in a unit test to assert the hand-derived numbers — defer to plan-phase whether to include such a cross-check (CONTEXT D-124c's 6 inline derivation tests are the minimal load-bearing pin point).

## `ParticipantInput` Shape + Serialization Surface

| Property | Value |
|----------|-------|
| File | `coordinator/src/bitcoin/tx.rs:17-23` |
| Visibility | `pub struct` (within coordinator crate) |
| Derives | `Debug, Clone` |
| Wire-crossing? | **NO** — no `Serialize` / `Deserialize` derives |
| Construction sites | `coordinator/src/api/handlers.rs:475` (get_tx), `coordinator/src/round/signing.rs:124` (assemble_and_broadcast) — both coordinator-side, both reading from `RegisteredInput` |
| Mirror struct? | None — `ParticipantInput` has no client-side equivalent. The client sends `RegisterInputRequest` (a wire type), which the coordinator validates and stores in `RegisteredInput`; `ParticipantInput` is built fresh at PSBT-construction time from `RegisteredInput` |

Adding `pub script_type: shared::bip322::ScriptType` is safe:
- Derives stay valid (`ScriptType` is `Clone + Copy + Debug`).
- Zero wire-format impact.
- Both construction sites read `reg.script_type` (which Phase 20 adds to `RegisteredInput`).
- No tests outside `tx.rs::tests` construct `ParticipantInput` directly (verified: `grep -rn "ParticipantInput {" coordinator/ shared/ client/ tests/`).

## `validate_utxo` Wiring

| Property | Value |
|----------|-------|
| File | `coordinator/src/bitcoin/utxo.rs:62-117` |
| `txout.script_pubkey` source | `rpc.gettxout(&utxo.txid, utxo.vout).await?` (line 79) — corepc_types `GetTxOut`'s `script_pubkey.hex` field, parsed by `parse_script_pubkey_from_txout(&txout)` at line 95 |
| `dispatch_ownership_proof` call | Line 99-106 |
| `detect_script_type` call sites | Inside `dispatch_ownership_proof` body at lines 163, 184 (v=1 and v=2 branches) — both return the derived value as the function's `Ok(...)` |
| Current `UtxoDetails` shape | `pub struct UtxoDetails { pub value_sats: u64, pub script_pubkey: ScriptBuf }` (utxo.rs:37-40) |
| Current return | `Ok(UtxoDetails { value_sats, script_pubkey })` at line 116 — discards `derived` |
| Phase 20 insertion point | Extend `UtxoDetails` (add `pub script_type: ScriptType`); return `Ok(UtxoDetails { value_sats, script_pubkey, script_type: derived })` at line 116 |

### Error-path defensiveness

| Scenario | Current behavior | Reachable in fee path? |
|----------|------------------|------------------------|
| `detect_script_type` returns `Bip322Error::UnsupportedScriptType` (e.g., bare P2SH-multisig, OP_RETURN, P2PK) | `dispatch_ownership_proof` propagates the error → `validate_utxo` returns `UtxoError::InvalidProof` → handler returns 400 | **No.** Only validated UTXOs reach `RegisteredInput`, which is the only source feeding `ParticipantInput.script_type`. |
| `bip_config.allows(derived)` is false | `dispatch_ownership_proof` returns `Bip322Error::UnsupportedScriptType` (utxo.rs:165, 189) | **No** — same as above. |
| `validate_utxo` short-circuits before `dispatch_ownership_proof` (UTXO not found, value too low, etc.) | Returns `UtxoError::{NotFound, AlreadyRegistered, InsufficientValue, RpcUnavailable}` | **No** — `RegisteredInput` is never written. |

`validate_utxo` is the chokepoint. The fee path does NOT need to be defensive about `ScriptType` provenance — by the time `script_type` reaches `build_coinjoin_psbt`, it has already passed `dispatch_ownership_proof` AND the bip322 crate's verifier. `script_input_vbytes(st)` can safely `match st { P2wpkh | P2tr | P2shP2wpkh }` exhaustively without a fallback arm. `BipConfig::validate` (config.rs:230+) also enforces at boot that `bip.output_script_type` is in the allowed set, so `script_output_vbytes(bip.output_script_type)` is similarly safe.

## Existing Fee Tests + The v1.4 Baseline

### v1.3 `full_round` tests

| Property | Value |
|----------|-------|
| File | `tests/integration/full_round.rs` (1461 LOC, 8 test fns) |
| Hardcoded fee assertions? | **None.** `grep -n "fee_share\|fee_per_participant\|fee_total\|266\|800\|400" tests/integration/full_round.rs` returns zero matches. |
| What it asserts | End-to-end protocol behavior: round setup, input registration, output registration, signing, broadcast — no numeric fee assertions |
| Phase 20 risk | **Low.** New per-script fee math is byte-identical for the all-P2WPKH case (D-125 baseline = 266), which is what `full_round` exercises. Run as a regression gate, not refresh. |

### v1.4 `mixed_script_e2e` test

| Property | Value |
|----------|-------|
| File | `tests/integration/mixed_script_e2e.rs` (521 LOC, 1 test fn: `mixed_script_e2e_three_clients_broadcast`) |
| What it asserts (line-verified) | (1) Broadcast txid appears in regtest mempool within 10s (line 372 — `!mempool_txids.is_empty()`); (2) CoinJoin tx has exactly 3 denomination outputs of 100_000 sats (line 414 — `assert_eq!(..., "CoinJoin tx must have exactly 3 denomination outputs of {} sats; got {}")`); (3) Input script type set-equality per D-104 (line 510) |
| Numeric fee assertions? | **None.** `grep -n "fee_share\|fee_per_participant\|fee_total" tests/integration/mixed_script_e2e.rs` returns zero matches. |
| Phase 20 risk | **Low.** Output amounts shift slightly (more accurate fee deduction), but the test only checks denomination count (100_000 sats each) and broadcast success — both unaffected by the fee-math refinement. CONTEXT CD-45's "refresh hardcoded amounts" task resolves to "no refresh needed." |

### Pre-existing fee-math unit test

| Property | Value |
|----------|-------|
| Location searched | `coordinator/src/bitcoin/tx.rs::tests`, `coordinator/src/bitcoin/fee.rs` (no tests module), workspace-wide |
| Found? | **NO** pre-existing test asserts a specific `fee_share` numeric value. The existing tx.rs tests (n_denomination_outputs, is_valid_psbt, dust_folded_into_fee, insufficient_funds_error, witness_utxo_set) all assert structural properties, not fee numbers. |
| Implication for D-125 | The new `fee_share_p2wpkh_only_matches_v14_baseline` test introduces the **first** numeric fee assertion in the codebase. Phase 20 must compute the baseline (266) from the pre-Phase-20 formula and bake it in as the regression expectation. CONTEXT D-125's inline derivation comment is the audit-charter artifact. |

### v1.4 baseline math (verifies D-125's `266`)

For n=3 uniform-P2WPKH, fee_rate=2 sat/vB, per the pre-Phase-20 formula in tx.rs:66-70:

```
estimated_vsize = TX_OVERHEAD + n*INPUT_WEIGHT + (n + num_change_outputs)*OUTPUT_WEIGHT
                = 10 + 3*68 + (3 + 3)*31           # num_change_outputs = n (upper bound, tx.rs:65)
                = 10 + 204 + 186
                = 400 vbytes
total_fee = 400 * 2 = 800 sats
fee_share = 800 / 3 = 266 sats  (integer floor; 2-sat remainder absorbed)
```

This matches D-125's hardcoded `266` exactly.

### Mixed-script math (verifies D-126's "≥1 sat")

For n=3, 1×P2WPKH (68) + 1×P2TR (58) + 1×P2SH-P2WPKH (91), output_script_type=P2WPKH (31), fee_rate=2:

```
estimated_vsize = 10 + (68 + 58 + 91) + 6*31      # 6 outputs = 3 denom + 3 change
                = 10 + 217 + 186
                = 413 vbytes
total_fee = 413 * 2 = 826 sats
fee_share = 826 / 3 = 275 sats
diff = 275 - 266 = 9 sats per participant   (well above the ≥1 sat threshold)
```

D-126's `>= 1` assertion has 9-sat headroom at fee_rate=2. If a future change reverts to P2WPKH-only weights for all inputs, the diff drops to 0 and the test fails — exactly the "per-script branch fires, not just compiles" sanity gate. **If P2TR=57 were used instead of 58 (floor not ceil):** vsize = 412, fee_share = 824/3 = 274, diff = 8 — still passes ≥1. Either choice satisfies D-126, but only 58 satisfies STATE.md's rounding policy.

## Risks / Landmines

### Risk 1: Floor-rounding remainder shifts byte-equality (D-125 risk)

**What goes wrong:** If Phase 20's refactor accidentally rounds differently (e.g., switches to `(total_fee + n - 1) / n` ceil-divide), the uniform-P2WPKH baseline shifts from 266 to 267 — D-125 fails byte-equality.

**Why it happens:** Refactoring `build_coinjoin_psbt` from `total_fee / n` to a separate `compute_per_participant_fee()` helper could introduce a different rounding direction.

**How to avoid:** Preserve `let fee_share = total_fee / n;` at tx.rs:70 verbatim. The vsize formula changes; the divide does not.

**Warning signs:** D-125 regression test fails with `assertion `left == right` failed: left: 266, right: 267`.

### Risk 2: Mixed-script test diverges by exactly 0 sats (silent revert)

**What goes wrong:** If `build_coinjoin_psbt` is refactored to always sum `script_input_vbytes(ScriptType::P2wpkh)` (e.g., via a hardcoded constant lookup), the mixed-script case still produces 800 sats / 266 share — D-126's `>= 1` assertion fails.

**Why it happens:** Copy-paste from the uniform-P2WPKH case, or a stub helper returning a fixed value.

**How to avoid:** Sum per-input weights via `inputs.iter().map(|inp| script_input_vbytes(inp.script_type)).sum::<u64>()`. The pattern is unambiguous about reading the per-input variant.

**Warning signs:** D-126 regression test fails with `assertion `(left - 266) >= 1` failed: left: 266`.

### Risk 3: PSBT signing path bytes shift (WR-04 violation)

**What goes wrong:** Adding `script_type` to `ParticipantInput` doesn't change PSBT bytes IF the field is purely metadata. But if `build_coinjoin_psbt` accidentally writes `script_type` into a PSBT proprietary key, the sighash for clients changes — clients sign against a different sighash than what gets broadcast.

**Why it happens:** PSBT extension via `psbt.inputs[i].proprietary` is a common "store extra info" pattern that affects the serialized PSBT bytes.

**How to avoid:** `script_type` is consumed ONLY by the in-memory vsize loop. It must NOT be written to `psbt.inputs[i]`. The output PSBT bytes from `build_coinjoin_psbt` MUST be identical (modulo fee/change amounts) whether or not the new field exists. Verify by manually inspecting the PSBT serialization in `coinjoin_psbt_is_valid_psbt` test.

**Warning signs:** v1.3 `full_round` tests fail at the signing/broadcast phase with sighash mismatches.

### Risk 4: P2TR ROADMAP/STATE.md numeric disagreement

**What goes wrong:** Plan-phase or executor picks 57 (matching ROADMAP SC#1 literal) instead of 58 (STATE.md round-UP policy). Audit-charter prose then references the WRONG number, and any future "underpay fees on mixed round" claim is technically false (57 underpays by 1 vbyte per P2TR input × fee_rate).

**Why it happens:** Two sources, two numbers, no inline reconciliation note.

**How to avoid:** Add inline comment in `script_input_vbytes(ScriptType::P2tr)`: `// 41 + ceil(66/4) = 58 vB. ROADMAP SC#1 cites 57 (floor of 57.5); STATE.md §v1.5 design notes mandates UP-rounding, so 58 is correct.` D-126's mixed-script math is recomputed with 58 (already done above).

**Warning signs:** Audit charter (Phase 21) cites 57 while source ships 58 — internal contradiction.

### Risk 5: `BipConfig::allowed_set` ordering leaks ScriptType enumeration order

**What goes wrong:** If `allowed_set()` yields `ScriptType`s in `match`-arm order (P2wpkh, P2tr, P2shP2wpkh) but Phase 16-03's PKARR advertisement uses alphabetical order (p2sh-p2wpkh, p2tr, p2wpkh — see `BipConfig::supported()` at config.rs:205-217), the two orderings disagree and downstream callers that iterate may produce different outputs.

**Why it happens:** Two helpers with subtly different ordering semantics on the same struct.

**How to avoid:** CONTEXT CD-43 leaves the method name/return shape open. Plan-phase should either (a) name the new helper `allowed_set` and document "iteration order is implementation-defined, callers must not depend on order" OR (b) reuse the existing `supported() -> Vec<ScriptType>` (alphabetical) if iteration order is needed for determinism. For `estimate_fee_share`'s `max(script_input_vbytes(t))` use case, ORDER DOES NOT MATTER (max is associative/commutative) — so either approach works.

**Warning signs:** None functional; this is a "future-proofing" risk.

### Risk 6: Confusing `output_script_type` parameter with per-output `script_type`

**What goes wrong:** Plan-phase or executor reads CONTEXT D-127 task 5 ("build_coinjoin_psbt signature: add `output_script_type: ScriptType` param") and assumes ALL outputs use that single type — but a participant's `change_address` might decode to a different script type (Wasabi-style, deferred to v1.6+).

**Why it happens:** The current `build_coinjoin_psbt` accepts arbitrary `change_address: ScriptBuf` per participant. The new `output_script_type` is a fee-math assumption, NOT a validation gate.

**How to avoid:** The fee-math change is correct under the v1.5 invariant "single output script type per round" — `bip.output_script_type` is the configured type and participants are expected to submit matching change addresses. The slight inaccuracy if they don't is accepted (CONTEXT "Not in scope" line 33: "uniform fee_share absorbs it"). Plan-phase should NOT add validation that `change_address.script_type() == bip.output_script_type` — that's a deferred v1.6+ feature.

**Warning signs:** Plan-phase adds a change-address-type validation that's not in the CONTEXT decision set.

## Code Examples

Verified patterns from the source at HEAD. [VERIFIED: source read]

### Example 1: Threading `script_type` through `UtxoDetails`

```rust
// Source: coordinator/src/bitcoin/utxo.rs:99-116 (existing — Phase 20 adds script_type to return)
let derived = dispatch_ownership_proof(
    &script_pubkey, ownership_proof, network, bip_config, message.as_bytes(),
).map_err(|e| UtxoError::InvalidProof { reason: e.to_string() })?;

// existing structured log (utxo.rs:110-114) already captures `derived`
tracing::info!(
    round_id = %round_id,
    script_type = ?derived,
    "ownership proof verified"
);

// Phase 20 change: include script_type in returned UtxoDetails
Ok(UtxoDetails { value_sats, script_pubkey, script_type: derived })
```

### Example 2: Worst-case fee_share estimate using `BipConfig`

```rust
// Source: derived from CONTEXT D-122 + existing fee.rs body
pub fn estimate_fee_share(bip_config: &BipConfig, n: u32, fee_rate: u64) -> u64 {
    let n = n as u64;
    if n == 0 { return 0; }
    let worst_input_vb = bip_config.allowed_set()
        .map(script_input_vbytes)
        .max()
        .expect("BipConfig::validate ensures at least one allow_* flag is true");
    let output_vb = script_output_vbytes(bip_config.output_script_type);
    let estimated_vsize = 10 + worst_input_vb * n + output_vb * 2 * n;
    (estimated_vsize * fee_rate) / n
}
```

### Example 3: Per-input weight sum in `build_coinjoin_psbt`

```rust
// Source: derived from CONTEXT D-127 task 5 + existing tx.rs:66-70 body
let num_change_outputs = n;
let total_input_vb: u64 = inputs.iter()
    .map(|inp| script_input_vbytes(inp.script_type))
    .sum();
let output_vb = script_output_vbytes(output_script_type);
let estimated_vsize = TX_OVERHEAD_VBYTES
    + total_input_vb
    + (n + num_change_outputs) * output_vb;
let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
let fee_share = total_fee / n;  // PRESERVE: integer floor (D-125 byte-equality)
```

### Example 4: Per-script vbyte table (CONTEXT D-124)

```rust
// Source: derived from CONTEXT D-124a + D-124b + STATE.md round-UP policy
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

## State of the Art

No state-of-the-art shift. The BIP-141 weight discount is 9 years stable (activated SegWit August 2017). BIP-341 P2TR weights are 4 years stable (Taproot activated November 2021). The 6 vbyte numbers in §BIP-141 vbyte references are unchanged from spec-publication date.

The only thing "current" here is the rust-bitcoin Weight API (PR#1636) which Phase 20 explicitly DOES NOT use per CONTEXT D-124 (const-fn table is the chosen approach for audit-charter prose value).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Cargo test + `#[tokio::test]` for async paths (Tokio 1.51 LTS); proptest available but not required for Phase 20 |
| Config file | `Cargo.toml` workspace (no separate test config); test binaries at `tests/integration/*.rs` |
| Quick run command | `cargo test -p coordinator --lib bitcoin::tx::tests` (~< 5s for the 5 existing + 2 new + 6 vbyte-pin tests) |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| FEE-01 | `script_input_vbytes` + `script_output_vbytes` return correct vbytes per ScriptType | unit | `cargo test -p coordinator --lib bitcoin::tx::tests::script_input_vbytes` (and `script_output_vbytes`; CONTEXT D-124c — 6 tests) | ❌ Wave 0: new tests in existing inline `mod tests` block |
| FEE-02 | `ParticipantInput.script_type` propagates UtxoDetails → RegisteredInput → ParticipantInput; build_coinjoin_psbt consumes it; CRIT-01 preserved | unit + integration | `cargo test -p coordinator --lib` + `cargo test --test integration mixed_script_e2e` | Partially — existing tests cover plumbing if code compiles |
| FEE-03(a) | Uniform-P2WPKH baseline byte-equal to 266 | unit | `cargo test -p coordinator --lib bitcoin::tx::tests::fee_share_p2wpkh_only_matches_v14_baseline` | ❌ Wave 0: new test |
| FEE-03(b) | Mixed-script differs by ≥1 sat/participant | unit | `cargo test -p coordinator --lib bitcoin::tx::tests::fee_share_mixed_script_differs_from_uniform_baseline` | ❌ Wave 0: new test |
| Cross-phase invariant 1 | v1.3 P2WPKH-only `full_round` 8/8 green | integration | `cargo test --test integration full_round` (~42s) | ✅ |
| Cross-phase invariant 2 | v1.4 `mixed_script_e2e_three_clients_broadcast` 1/1 green | integration | `cargo test --test integration mixed_script_e2e` | ✅ |
| Clippy gate | No new lints | static | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ existing CI |

### Sampling Rate
- **Per task commit:** `cargo test -p coordinator --lib bitcoin::tx::tests bitcoin::fee` + `cargo clippy --workspace --all-targets -- -D warnings`
- **Per wave merge:** `cargo test --workspace` (excludes signet-anchor tests; ~2 min)
- **Phase gate:** Full suite green + both `full_round` (8/8) AND `mixed_script_e2e` (1/1) green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `coordinator/src/bitcoin/tx.rs::tests` — extend `make_inputs` helper to accept per-input `ScriptType`; add 8 new tests (6 vbyte-table pins per D-124c + 2 FEE-03 regression tests per D-125/D-126)
- [ ] `coordinator/src/bitcoin/fee.rs` — no existing tests module; Phase 20 should add at least 1 unit test for `estimate_fee_share` (e.g., `worst_case_picks_max_allowed_input_vbyte`) so the worst-case formula is covered at the unit tier

*(No new framework install; all tooling already in place.)*

## Security Domain

`security_enforcement` is enabled (no project config indicates otherwise; PROJECT.md constraints apply).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | no | Phase 20 changes no auth surface |
| V3 Session Management | no | Phase 20 changes no session surface |
| V4 Access Control | no | Phase 20 changes no access-control surface; `bip_config.allows()` already gates at `dispatch_ownership_proof` |
| V5 Input Validation | yes | `ScriptType` derives from on-chain SPK (chain-validated), not client wire data; CRIT-01 invariant |
| V6 Cryptography | no | Phase 20 touches no crypto primitives (no signing, no hashing, no key material) |
| V7 Error Handling & Logging | yes | Existing PII-safe `tracing::info!(round_id, script_type = ?derived, ...)` log at utxo.rs:110-114; Phase 20 adds no new log call sites |

### Known Threat Patterns for blindjoin coordinator (Phase 20 scope)

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Client-declared `ScriptType` spoofing | Spoofing (S) / Tampering (T) | **CRIT-01 invariant:** `ScriptType` derived from on-chain `script_pubkey` via `detect_script_type`; never read from client wire data. Phase 20 plumbing path (UtxoDetails → RegisteredInput → ParticipantInput) propagates the already-derived value; there is NO point in the path where a client-supplied script-type value is read or trusted |
| Fee underpayment in mixed-script round | Repudiation (R) / DoS (D) | Conservative round-UP per STATE.md §v1.5 design notes: P2TR 58 (not 57); pre-registration estimate uses worst-case across allowed_set (D-122); coordinator never underestimates |
| Sighash mismatch between display/broadcast (WR-04 violation) | Tampering (T) | Single canonical `estimate_fee_share` helper consumed by both `get_tx` and `assemble_and_broadcast`; Phase 20 preserves this invariant — only the formula inside the helper changes |
| Privacy leak via fee_share fingerprinting | Information Disclosure (I) | Worst-case-across-allowed-set in pre-reg estimate is uniform regardless of registration order — does NOT leak which participants registered which script types. (Privacy property called out in CONTEXT §specifics.) |
| Cross-shape SPK acceptance (e.g., raw P2SH-multisig wrapped in P2SH-P2WPKH script_type) | Tampering (T) | Out of Phase 20 scope. `detect_script_type` optimistically returns P2shP2wpkh for any P2SH SPK; the bip322 crate's verifier (already in production) performs HASH160 cross-check at verify time. Phase 20 trusts the already-vouched-for ScriptType from `dispatch_ownership_proof` — does not weaken this gate |

**No new attack surface introduced.** Phase 20's only new code path is reading a value the dispatcher already produces and using it in arithmetic that the operator already controls. The CRIT-01 invariant is preserved by construction (no new `detect_script_type` call sites; no new client-wire fields).

## Runtime State Inventory

**Phase 20 is NOT a rename/refactor/migration.** It is a code-only feature addition. Skip the full inventory.

Brief sanity check (rather than leaving blank):

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — `RegisteredInput` lives in `RoundStateInner` (in-memory only, zeroed on round-end via Drop) | None |
| Live service config | None — no external config service stores fee constants | None |
| OS-registered state | None — Phase 20 changes no Docker, systemd, scheduler, or process-manager artifacts | None |
| Secrets/env vars | None — `bip.output_script_type` and `bip.allow_*` env vars already exist and are read; Phase 20 adds no new env vars | None |
| Build artifacts | None — Rust compilation; no egg-info / lockfiles / generated stubs at risk | None |

## Environment Availability

Phase 20 is code/config-only. No external dependencies introduced. The existing stack (Rust 1.x, cargo, bitcoin 0.32, regtest Bitcoin Core via corepc-node for integration tests) is unchanged. **Section skipped.**

## Project Constraints (from CLAUDE.md)

| Constraint | Phase 20 Compliance |
|------------|---------------------|
| No custom crypto — blind-rsa-signatures, rust-bitcoin, bdk, secp256k1 only | ✅ Phase 20 touches no crypto. Only structural changes + arithmetic. |
| Tor-native in production; dev/test may use clearnet TCP | ✅ Phase 20 changes no transport |
| Signet-first; mainnet is a config flag | ✅ Phase 20 changes no network parameters |
| No PII logging; round state zeroed after broadcast | ✅ Existing PII-safe log at utxo.rs:110-114 reused (script_type is enum value, not PII); new `script_type` field on `RegisteredInput` carries `#[zeroize(skip)]` per CONTEXT D-123a (mirrors existing `script_pubkey` annotation; ScriptType is public chain data) |
| MIT licensed — public good, not a business | ✅ Phase 20 adds no external service integration |
| Use `bitcoin = 0.32`, `bdk_wallet = 2.2`, `tokio = 1.51 LTS`, `axum = 0.8`, `arti-client = 2.x` | ✅ Phase 20 adds no new dependencies; uses only already-imported `shared::bip322::ScriptType` |
| `gsd-debug` pivot if invariants go red (REPAIR-01 lesson #4) | ✅ Documented in CONTEXT §Cross-phase invariants line 44; both v1.3 `full_round` and v1.4 `mixed_script_e2e` are the load-bearing gates |
| Skill routing: When user request matches an available skill, ALWAYS invoke via Skill tool first | n/a — Phase 20 is GSD-workflow internal (planner runs after this research) |
| GSD Workflow Enforcement: file-changing tools through a GSD command | ✅ This research is part of `/gsd:plan-phase`; the planner consumes RESEARCH.md next |

No constraints conflict with Phase 20's decisions. All CONTEXT decisions (D-122..D-127) honor CLAUDE.md.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Existing `BipConfig::supported()` (config.rs:205-217) returns alphabetical order (`P2shP2wpkh, P2tr, P2wpkh`) — useful if plan-phase reuses it instead of adding `allowed_set()` | §Risks #5 + §Decision CD-43 | Low — `max(script_input_vbytes(...))` is order-independent; this is a future-proofing observation only |
| A2 | `bitcoin::Weight::predict_weight` exists in `bitcoin = 0.32.x` (the workspace pin) — cited as a verification option in CD-42 | §BIP-141 vbyte references §Alternative | Low — CONTEXT D-124 explicitly does NOT use the Weight API; the hand-derived const-fn table is the chosen path. If `predict_weight` does not exist in 0.32, the only loss is the optional cross-check test, which is not required |
| A3 | Coordinator `BipConfig::validate` at boot guarantees `output_script_type ∈ allowed_set` — so `script_output_vbytes(bip.output_script_type)` is always reachable | §`validate_utxo` error-path defensiveness | Very low — verified at config.rs:230-249 (D-37 enforcement); `BipConfig::validate` is called in production startup paths |

All other claims are tagged `[VERIFIED: source read]` or `[CITED: ...]` inline. The Assumptions Log is short by design — CONTEXT is unusually well-grounded.

## Open Questions

**None blocking plan-phase.** All P0/P1 questions are resolved in CONTEXT decisions D-122..D-127 + CD-40..CD-45.

Optional follow-ups (do NOT block Phase 20):

1. **Should plan-phase add a Weight-API cross-check test?**
   - What we know: `bitcoin::Weight` exists; could construct a real P2WPKH TX with one input and assert `tx.weight() / WITNESS_SCALE_FACTOR` rounds to 68 vB.
   - What's unclear: Whether it adds audit-charter value beyond the inline derivation comments (CONTEXT D-124's reason for hand-derived).
   - Recommendation: Defer to plan-phase judgment. Default = skip (CONTEXT D-124's intent); add only if it costs ≤ 10 LOC.

2. **Should `BipConfig::allowed_set` reuse `supported()` (already exists at config.rs:205) or introduce a new method?**
   - What we know: `supported() -> Vec<ScriptType>` already exists with alphabetical ordering (load-bearing for Phase 16-03 PKARR byte budget per the comment at config.rs:199-204).
   - What's unclear: Whether adding a parallel `allowed_set()` causes naming confusion.
   - Recommendation: Plan-phase decides — either reuse `supported()` (simpler, but iterator-vs-Vec impedance) or add `allowed_set() -> impl Iterator<Item = ScriptType>` (CONTEXT CD-43 default; cleaner for the `.map().max()` chain).

## Sources

### Primary (HIGH confidence) — source code at HEAD, 2026-05-31

- `coordinator/src/bitcoin/tx.rs` — hardcoded constants (lines 11, 13), `build_coinjoin_psbt` signature (53-58), `ParticipantInput` shape (17-23), `fee_share` floor-divide (70), inline tests module (131-224) [VERIFIED: source read]
- `coordinator/src/bitcoin/fee.rs` — `estimate_fee_share` signature + hardcoded vsize (lines 11-18) [VERIFIED: source read]
- `coordinator/src/bitcoin/utxo.rs` — `UtxoDetails` shape (37-40), `validate_utxo` flow (62-117), `dispatch_ownership_proof` body with `detect_script_type` calls (153-196) [VERIFIED: source read]
- `coordinator/src/round/state.rs` — `RegisteredInput` shape with `#[zeroize(skip)]` precedent (53-68), `RoundStateInner` Drop semantics (115-144) [VERIFIED: source read]
- `coordinator/src/round/input_reg.rs` — `register_input` body with `RegisteredInput` insertion at line 82 [VERIFIED: source read]
- `coordinator/src/round/signing.rs` — `ParticipantInput` construction in `assemble_and_broadcast` at lines 124-129 [VERIFIED: source read]
- `coordinator/src/api/handlers.rs` — `estimate_fee_share` call sites (165, 505), `ParticipantInput` construction in `get_tx` (475-481), `validate_utxo` call (178-188) [VERIFIED: source read]
- `coordinator/src/config.rs` — `BipConfig` struct (158-187), `allows()` method (191-197), `supported()` alphabetical order (205-217), `validate()` D-37 enforcement (230-249) [VERIFIED: source read]
- `shared/src/bip322/mod.rs` — `ScriptType` enum (150-157), `detect_script_type` (238-248), `sign_simple` / `verify_simple` dispatchers (257-294) [VERIFIED: source read]
- `tests/integration/full_round.rs` — confirmed zero `fee_share`/numeric-fee assertions [VERIFIED: grep at HEAD]
- `tests/integration/mixed_script_e2e.rs` — confirmed asserts on broadcast txid + denomination count only [VERIFIED: source read + grep at HEAD]
- `Cargo.toml` (workspace) — `bitcoin = 0.32` pin [VERIFIED: source read]
- `.planning/REQUIREMENTS.md` — FEE-01/02/03 verbatim spec, Future Requirements deferrals, Out-of-Scope rationale [VERIFIED: file read]
- `.planning/STATE.md` — v1.5 design notes line 2 (UP-rounding policy), CRIT-01 invariant carryover [VERIFIED: file read]
- `.planning/ROADMAP.md` — Phase 20 5 success criteria, depends-on Phase 16+18, cross-phase invariant [VERIFIED: file read]
- `.planning/phases/20-mixed-round-fee-accuracy/20-CONTEXT.md` — 6 LOCKED decisions (D-122..D-127), 6 CD discretion items (CD-40..CD-45), 8 code anchors with line numbers [VERIFIED: file read]

### Secondary (MEDIUM confidence) — external corroboration

- [Spark: Bitcoin Transaction Size Reference](https://www.spark.money/tools/bitcoin-transaction-size-reference) — "P2TR inputs are cheaper to spend at 57.5 vB compared to 68 vB for P2WPKH" — confirms P2TR raw vbyte is 57.5 (corroborates STATE.md round-UP → 58)
- [Spark: Bitcoin Address Formats Explained](https://www.spark.money/tools/bitcoin-address-format-guide) — P2TR keypath signature properties (64-byte Schnorr, no pubkey in witness)
- [Bitcoin Optech: Preparing for Taproot](https://bitcoinops.org/en/preparing-for-taproot/) — BIP-341 keyspend witness composition

### Tertiary (LOW confidence) — referenced but not load-bearing

- [rust-bitcoin#1636: Weight prediction by Kixunil](https://github.com/rust-bitcoin/rust-bitcoin/pull/1636) — `predict_weight` API; cited as the "alternative the planner can consider" per CD-42, but explicitly NOT used per CONTEXT D-124

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; existing crates already pinned in `Cargo.toml` and `coordinator/Cargo.toml`
- Architecture: HIGH — full source-code verification of every line number cited in CONTEXT; data flow traced end-to-end (UtxoDetails → RegisteredInput → ParticipantInput) and confirmed no wire-protocol changes needed
- BIP-141 numbers: HIGH for outputs (exact spec values); HIGH for P2WPKH input (68 = roadmap + spec agreement); HIGH for P2SH-P2WPKH input (91 = roadmap + spec agreement); HIGH for P2TR input choosing 58 over 57 (STATE.md UP-rounding policy is binding, raw value 57.5 cross-verified via 1 external source)
- v1.4 baseline 266: HIGH — derived from existing tx.rs formula at lines 66-70, math reproduces D-125's hardcoded value
- Mixed-script diff 9 sats: HIGH — derived using the chosen P2TR=58; even with P2TR=57 the diff is 8 sats (both pass `≥1`)
- Risk/landmine analysis: HIGH for risks 1-4 (direct source/test reads); MEDIUM for risk 5 (future-proofing speculation); HIGH for risk 6 (CONTEXT explicit on what's NOT in scope)
- CRIT-01 preservation: HIGH — single derivation point at utxo.rs:99, propagation path has no client-trust hole, verified by reading every struct mutation site

**Research date:** 2026-05-31
**Valid until:** ~2026-06-30 (Phase 20 should be executed within ~30 days; BIP-141/BIP-341 numbers are spec-stable; the only volatile item is the rust-bitcoin API version, and Phase 20 doesn't depend on it)
