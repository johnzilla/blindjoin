# Phase 20: Mixed-Round Fee Accuracy - Context

**Gathered:** 2026-05-31
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 20 replaces the P2WPKH-only fee approximation in `coordinator/src/bitcoin/tx.rs` (and the parallel `coordinator/src/bitcoin/fee.rs::estimate_fee_share`) with a per-script weight table so a mixed-script CoinJoin round (heterogeneous P2WPKH + P2TR + P2SH-P2WPKH inputs) charges a `fee_share` that reflects actual per-input witness weights. The change is plumbed end-to-end:

1. **FEE-01** — `coordinator/src/bitcoin/tx.rs` gains `pub fn script_input_vbytes(ScriptType) -> u64` and `pub fn script_output_vbytes(ScriptType) -> u64`, replacing the hardcoded `INPUT_WEIGHT_VBYTES = 68` and `OUTPUT_WEIGHT_VBYTES = 31` at `tx.rs:11,13`. Each return arm carries a 4-6 line BIP-141 derivation comment (D-124 below). Conservative rounding UP (never DOWN) per STATE.md §v1.5 design notes.

2. **FEE-02** — `ParticipantInput` (tx.rs:18) gains `script_type: shared::bip322::ScriptType`. The value is **coordinator-derived** at `validate_utxo` time via `detect_script_type(txout.script_pubkey)` (V1.4-CRIT-01 invariant preserved into the fee path — never client-declared). `build_coinjoin_psbt` sums per-input weights via `script_input_vbytes(inp.script_type)` and uses `script_output_vbytes(bip_config.output_script_type)` for the denomination + change outputs (single-output-type per round per Phase 16 D-37). The `fee_share = total_fee / N` formula stays uniform — per-input variable fee is REQUIREMENTS.md `Future requirements` (separate milestone, changes the wire protocol).

3. **FEE-03** — Two regression tests in `coordinator/src/bitcoin/tx.rs::tests`:
   (a) `fee_share_p2wpkh_only_matches_v14_baseline` — 3-participant uniform-P2WPKH round, fee_rate=2 sat/vB, asserts `fee_share == 266` (the byte-exact v1.4 value); preserves the v1.3 cross-phase invariant from the fee-math angle.
   (b) `fee_share_mixed_script_differs_from_uniform_baseline` — 3-participant 1×P2WPKH + 1×P2TR + 1×P2SH-P2WPKH round (output_script_type = P2WPKH), fee_rate=2 sat/vB, asserts `fee_share - 266 >= 1` (sanity check that the per-script branch actually fires, not just compiles).

**Requirements mapped to this phase** (per `.planning/REQUIREMENTS.md` §Traceability): FEE-01, FEE-02, FEE-03.

**Boundary changes (Phase 20 modifies these files):**
- `coordinator/src/bitcoin/tx.rs` — add `script_input_vbytes` + `script_output_vbytes` pub fns; delete `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES` consts; ParticipantInput gains `script_type` field; build_coinjoin_psbt signature gains `output_script_type: ScriptType` param; vsize loop sums per-input weights; add 2 regression tests.
- `coordinator/src/bitcoin/fee.rs` — `estimate_fee_share` signature gains `bip_config: &BipConfig` (or the allowed `ScriptType` set + the configured `output_script_type`); body uses `max(script_input_vbytes across allowed set)` + `script_output_vbytes(output_script_type)` for true worst-case-upper-bound (single canonical worst-case definition, WR-04 invariant preserved).
- `coordinator/src/bitcoin/utxo.rs` — `UtxoDetails` (line 37) gains `script_type: ScriptType`; `validate_utxo` returns the `derived` ScriptType from `dispatch_ownership_proof` (already computed internally — currently thrown away) in the returned struct.
- `coordinator/src/round/state.rs` — `RegisteredInput` (line 54) gains `script_type: ScriptType` with `#[zeroize(skip)]` (public chain data, no privacy concern, mirrors `script_pubkey`).
- `coordinator/src/api/handlers.rs` — POST /round/input writes `script_type` into `RegisteredInput`; `get_tx` constructs `ParticipantInput` with `script_type: reg.script_type`; the `estimate_fee_share` call sites pass `&state.config.bip`.
- `coordinator/src/round/signing.rs` — `assemble_and_broadcast` constructs `ParticipantInput` with `script_type: reg.script_type`; the `estimate_fee_share` call site at line 505 passes `&state.config.bip`.

**Not in scope (defer / reject):**
- Per-input variable `fee_share` (REQUIREMENTS.md `Future requirements`: changes wire protocol, separate milestone). Phase 20 keeps `fee_share = total_fee / N` uniform.
- Mixed output script types per participant (Wasabi 2.0.3-style) — REQUIREMENTS.md `Out of v1.5 scope but not anti-features`. Outputs remain single-type per round, coordinator-configured via `bip.output_script_type`.
- B-03 dynamic fee estimation (mempool-aware polling + RBF). Phase 20 is about per-script weight *accuracy*, not mempool *responsiveness*. Carry-forward to v1.6+ per `.planning/STATE.md` §"Carry-Forward Items".
- Validating change_address script_type matches `bip.output_script_type` (currently coordinator accepts any valid address; the weight calc assumes output_script_type — slight inaccuracy if participants submit different-type change addresses, but uniform fee_share absorbs it). Defer to v1.6+ if audit charter flags.
- Any change to client-side fee math (client does not compute fee; it receives the PSBT and signs).
- AUDIT-CHARTER.md (Phase 21 — but Phase 21's charter prose will reference Phase 20's per-script weight table as the "multi-script verification + fee path" complete picture).
- RSA RoundSecretKey newtype (Phase 21 work).

**Cross-phase invariants (carry to every Phase 20 plan boundary):**
1. **v1.3 P2WPKH invariant:** `cargo test --test integration full_round` 8/8 green (~42s). Phase 20 makes NO changes to `full_round.rs`. The v1.4-parity regression test FEE-03(a) is the load-bearing assertion that this gate stays green.
2. **v1.4 multi-script invariant:** `cargo test --test integration mixed_script_e2e` 1/1 green (acceptance gate). The new fee math will produce slightly different output amounts in this test (the change deduction shifts) — verify the test still asserts on broadcast success (txid observable in mempool) rather than exact change values; if it asserts exact values, refresh those values per Phase 20's per-script weights. Plan-phase confirms via grep on `mixed_script_e2e.rs` for hardcoded amount assertions.
3. **WR-04 single canonical fee definition:** `get_tx` and `assemble_and_broadcast` MUST produce byte-identical PSBTs (else clients sign against a different sighash than what gets broadcast). The single canonical `estimate_fee_share(&BipConfig, n, fee_rate)` is the load-bearing helper. Phase 20 preserves this — only the formula inside changes.
4. **V1.4-CRIT-01 invariant:** `script_type` is coordinator-derived from on-chain `script_pubkey` via `detect_script_type`, NEVER client-declared. The Phase 20 plumbing path (UtxoDetails → RegisteredInput → ParticipantInput) propagates the already-derived value; there is no point in the path where a client-supplied script-type value is read or trusted.

If any of these invariants goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phases 14/15/16/17/18 + REQUIREMENTS.md (NOT re-asked)

LOCKED upstream. Plan-phase consumes verbatim — no re-litigation.

- **Uniform `fee_share = total_fee / N`** — REQUIREMENTS.md `Future requirements` §"Per-input variable fee_share" is explicitly a separate milestone (changes the wire protocol; clients must accept variable amounts in the PSBT). Phase 20 keeps the uniform formula.
- **Single-output-type per round** — Phase 16 D-37 + REQUIREMENTS.md `Out of v1.5 scope but not anti-features` §"Mixed output script types per participant" — outputs remain single-type per round, coordinator-configured via `bip.output_script_type`. `script_output_vbytes` takes one `ScriptType` (the configured output_script_type) and applies to all denomination + change outputs uniformly.
- **V1.4-CRIT-01 (coordinator-derived script_type)** — `ScriptType` is derived from the on-chain `script_pubkey` returned by Bitcoin Core's `gettxout`, NEVER from client-supplied wire data. `validate_utxo` already calls `detect_script_type(spk)` inside `dispatch_ownership_proof`. Phase 20 returns that value through `UtxoDetails` instead of discarding it.
- **Conservative rounding UP** — STATE.md §"v1.5 design notes" line 2: "Rounding policy needs to be conservative (round UP) so the coordinator doesn't underpay fees on a mixed round." Binding for D-124 below.
- **v1.3 + v1.4 cross-phase invariants** — `full_round` 8/8 + `mixed_script_e2e` 1/1 stay green at every plan boundary.

### A. Pre-registration fee_share estimate (fee.rs)

- **D-122:** **`fee.rs::estimate_fee_share` becomes `estimate_fee_share(bip_config: &BipConfig, n: u32, fee_rate: u64) -> u64`** and computes a true worst-case upper bound: `max(script_input_vbytes(t) for t in bip_config.allowed_set()) * n` for the input side, `script_output_vbytes(bip_config.output_script_type) * (2 * n)` for the output side, plus the fixed `TX_OVERHEAD_VBYTES = 10`. **Rationale:** at INPUT_REG time the coordinator doesn't know which script types will register — it must overestimate so that when `build_coinjoin_psbt` later computes the real per-input weight, `fee_share` cannot exceed what `validate_utxo` already required from each participant. Over-charges a P2WPKH input in a round where P2SH-P2WPKH (91 vB) is allowed but not yet registered — acceptable, because the real `fee_share` at build time is the load-bearing number a participant actually pays. **CRIT-01 implication:** none — `BipConfig` is operator config, not client data.
- **D-122a:** **`BipConfig` gains a helper `pub fn allowed_set(&self) -> impl Iterator<Item = ScriptType>`** that yields each ScriptType whose corresponding `allow_*` flag is true. Used by `estimate_fee_share` and potentially the discovery/advertisement path; small ergonomic helper, no new policy. Plan-phase confirms exact method name + return shape.
- **D-122b:** **Two call sites of `estimate_fee_share` updated to pass `&state.config.bip`:** `coordinator/src/api/handlers.rs:165` (`fee_share_pre_lock`) and `coordinator/src/api/handlers.rs:505` (`fee_per_participant_sats`). Both already have access to `state.config.bip`. No new threading.

### B. ScriptType path through state structs (FEE-02 plumbing)

- **D-123:** **Full plumbing: `UtxoDetails` → `RegisteredInput` → `ParticipantInput`.** The single source of truth is `detect_script_type` called once at `validate_utxo` time inside `dispatch_ownership_proof`. The derived value is returned from `validate_utxo` (via `UtxoDetails`), stored in `RegisteredInput` (round state), and copied into `ParticipantInput` at build time. **Rationale:** matches REQUIREMENTS FEE-02 verbatim ("set by the coordinator at validate_utxo time"); never re-runs `detect_script_type` (1 hash per UTXO saved); zero re-derive risk at later phases; CRIT-01 audit prose is direct ("the field is populated exactly once, at the same callsite that runs `detect_script_type` on the on-chain `script_pubkey`"). Cost: 3 struct touches (UtxoDetails, RegisteredInput, ParticipantInput), 2 call-site touches (handlers.rs:475, signing.rs:124).
- **D-123a:** **`RegisteredInput.script_type` is `#[zeroize(skip)]`** — mirrors the existing `script_pubkey` annotation. ScriptType is one of 3 enum values; carries no key material, no privacy concern, derivable from `script_pubkey` which is already public chain data.
- **D-123b:** **`UtxoDetails` (utxo.rs:37) is extended NOT replaced** — adding `script_type: ScriptType` to the existing struct (currently `{value_sats, script_pubkey}`). Callers (handlers.rs:178 `let utxo_details = validate_utxo(...)`) consume the new field via `utxo_details.script_type`. No breaking-API churn; struct is `pub` but only used within the coordinator crate.

### C. vbyte source — hand-derived with BIP-141 math inline

- **D-124:** **`script_input_vbytes(ScriptType) -> u64` and `script_output_vbytes(ScriptType) -> u64` are `const fn` (or plain `pub fn` if const-fn isn't ergonomic for the match arms) with each return arm carrying a 4-6 line comment showing the BIP-141 derivation.** **Rationale:** the roadmap's literal numbers (68/31, 57/43, 91/32) are derived from BIP-141 worst-case; pinning them as opaque magic numbers loses the audit-charter prose target. Hand-deriving inline lets Phase 21 cite the source comment directly. **Conservative rounding UP** per STATE.md §v1.5 design notes is enforced via integer-arithmetic ceil: `(witness_bytes + 3) / 4` (NOT `witness_bytes / 4` which floors).

- **D-124a (inputs, per BIP-141 vsize = non_witness + ceil(witness/4)):**

  | ScriptType | non_witness | witness | derived vbytes | roadmap SC says |
  |---|---|---|---|---|
  | P2WPKH | 41 (32 prev_txid + 4 vout + 1 script_sig_len=0 + 4 sequence) | 108 (1 stack_count + 1 sig_len + 72 DER + 1 pk_len + 33 pk) | 41 + 27 = **68** ✓ | 68 |
  | P2TR keypath | 41 (same) | 66 (1 stack_count + 1 sig_len + 64 Schnorr SIGHASH_DEFAULT) | 41 + ceil(66/4) = 41 + 17 = **58** | **57** (floor) |
  | P2SH-P2WPKH | 64 (32 + 4 + 1 script_sig_len=23 + 23 redeem-wrapper + 4 sequence) | 108 (same as P2WPKH) | 64 + 27 = **91** ✓ | 91 |

  **P2TR discrepancy:** STATE.md's "round UP" directive says **58** (ceil of 57.5). Roadmap SC#1's literal "57" is a floor. Plan-phase research task: verify the canonical P2TR keypath worst-case vbyte against (a) rust-bitcoin's `predict_weight`, (b) sipa/BIP-341 worksheet, (c) Bitcoin Core's `getmempoolinfo`/`testmempoolaccept`. Default if research confirms 57.5 — use **58** (round UP per STATE.md) and document the divergence from the roadmap's 57 inline with a one-line "rounded UP per STATE.md §v1.5 design notes (raw value 57.5)". The roadmap number was a planning approximation; STATE.md's rounding policy is the load-bearing rule.

- **D-124b (outputs, exact — no segwit discount, no rounding):**

  | ScriptType | bytes | derivation |
  |---|---|---|
  | P2WPKH | **31** | 8 value + 1 script_len(22) + 22 (OP_0 OP_PUSHBYTES_20 <20>) |
  | P2TR | **43** | 8 value + 1 script_len(34) + 34 (OP_1 OP_PUSHBYTES_32 <32>) |
  | P2SH-P2WPKH | **32** | 8 value + 1 script_len(23) + 23 (OP_HASH160 OP_PUSHBYTES_20 <20> OP_EQUAL) |

  Outputs match the roadmap exactly; no rounding ambiguity.

- **D-124c:** **`#[test]` blocks** inline in `tx.rs::tests` pin the per-script vbyte numbers against their derivation (one assert per (ScriptType, input/output) combo, six asserts total). These tests are the audit-charter pin point — Phase 21 can cite them as "the per-script weight table is verified at the unit-test layer." Lightweight; ~30 LOC.

### D. v1.4-parity baseline test (FEE-03(a))

- **D-125:** **`fee_share_p2wpkh_only_matches_v14_baseline` hardcodes the numeric baseline `266`** with a derivation comment block inside the test fn body. **Math:** for n=3 (3-participant uniform-P2WPKH round per ROADMAP SC#3), fee_rate=2 sat/vB:
  ```
  // v1.4 baseline (P2WPKH-only, n=3, fee_rate=2):
  // estimated_vsize = TX_OVERHEAD + n*INPUT_WEIGHT + n*2*OUTPUT_WEIGHT
  //                 = 10 + 3*68 + 3*2*31
  //                 = 10 + 204 + 186
  //                 = 400 vbytes
  // total_fee = estimated_vsize * fee_rate = 400 * 2 = 800 sats
  // fee_share = total_fee / n = 800 / 3 = 266 sats (integer division; 2 sat remainder absorbed)
  ```
  **Rationale:** the inline derivation is the v1.3-invariant-from-the-fee-math-angle artifact; if someone refactors `build_coinjoin_psbt`'s formula and accidentally changes the P2WPKH-uniform baseline (e.g., switches output count from `2*n` to `n + non_dust_change_count`), this test catches it byte-exactly. The hardcoded `266` + the derivation comment is more durable than a `v14_formula()` helper (which a future cleanup might delete).

- **D-126:** **`fee_share_mixed_script_differs_from_uniform_baseline` asserts `fee_share - 266 >= 1`** with the mixed-script derivation in a comment block. **Math:** for n=3 (1×P2WPKH + 1×P2TR + 1×P2SH-P2WPKH inputs; output_script_type=P2WPKH; 3 denomination + 3 change outputs all at 31 vB each), fee_rate=2 sat/vB:
  ```
  // mixed-script (n=3, fee_rate=2, output_type=P2WPKH):
  // estimated_vsize = 10 + (68 + 58 + 91) + 6*31 = 10 + 217 + 186 = 413 vbytes
  // total_fee = 413 * 2 = 826 sats
  // fee_share = 826 / 3 = 275 sats
  // diff per participant: 275 - 266 = 9 sats > 1 ✓ (ROADMAP SC#4 satisfied at fee_rate=2)
  ```
  At fee_rate=2 sat/vB the divergence is 9 sats per participant — well above the ≥1 sat requirement. Plan-phase can tune `fee_rate` if needed for clarity but no escalation required at the default test rate.

### E. Plan structure / sequencing

- **D-127:** **ONE plan: `20-01-PLAN.md`.** The Phase 20 scope is internally cohesive — FEE-01 (per-script weight table), FEE-02 (plumbing), and FEE-03 (regression tests) all touch the same call chain (`tx.rs` + the validate_utxo → state → tx.rs path) and can't be staged independently without either (a) introducing a transient state where `ParticipantInput.script_type` exists but isn't yet used (FEE-01 without FEE-02), or (b) leaving the regression tests for a follow-up commit that lands days later (FEE-03 split). **Task breakdown inside the single plan:**
  1. Add `script_input_vbytes` + `script_output_vbytes` + their inline unit tests (D-124c). Delete the two consts at tx.rs:11,13.
  2. Extend `UtxoDetails` with `script_type`; thread the `derived` value out of `validate_utxo` (already computed by dispatch_ownership_proof).
  3. Add `script_type: ScriptType` field to `RegisteredInput` (state.rs:54) with `#[zeroize(skip)]`; populate at handlers.rs's POST /round/input write site.
  4. Add `script_type: ScriptType` field to `ParticipantInput` (tx.rs:18); update both call sites (handlers.rs:475, signing.rs:124) to write `reg.script_type`.
  5. Update `build_coinjoin_psbt` signature: add `output_script_type: ScriptType` param; replace the `n * INPUT_WEIGHT_VBYTES + (n + num_change_outputs) * OUTPUT_WEIGHT_VBYTES` formula with the per-input sum + per-output multiply. Update the two call sites (handlers.rs's `get_tx` and signing.rs's `assemble_and_broadcast`) to pass `state.config.bip.output_script_type`.
  6. Update `estimate_fee_share` (fee.rs) signature: take `&BipConfig`; use `max(script_input_vbytes across allowed_set()) * n + script_output_vbytes(output_script_type) * 2 * n + 10`. Update both call sites (handlers.rs:165, handlers.rs:505).
  7. Add `BipConfig::allowed_set()` helper (D-122a).
  8. Add FEE-03(a) `fee_share_p2wpkh_only_matches_v14_baseline` regression test.
  9. Add FEE-03(b) `fee_share_mixed_script_differs_from_uniform_baseline` regression test.
  10. Verify cross-phase invariants: `cargo test --test integration full_round` 8/8 green; `cargo test --test integration mixed_script_e2e` 1/1 green (refresh hardcoded amount assertions if any — plan-phase greps `mixed_script_e2e.rs` for this); `cargo clippy --workspace --all-targets -- -D warnings` clean.

  **Rationale for single plan:** the changes are tightly coupled at the type level (every struct + every callsite must agree on the new field at the same commit), and the regression tests are the load-bearing verification — splitting them would create a green intermediate state that doesn't actually exercise the per-script branch.

### Claude's Discretion

- **CD-40:** **Exact location of `script_input_vbytes` / `script_output_vbytes`** — Plan-phase decides between (a) keeping them in `coordinator/src/bitcoin/tx.rs` next to `build_coinjoin_psbt` (close to the only caller, smaller surface), or (b) promoting them to `coordinator/src/bitcoin/fee.rs` next to `estimate_fee_share` (the two functions now share the weight table). Default: **(b)** — fee.rs is now the canonical weight-table-and-fee-math module; tx.rs imports the helpers via `use crate::bitcoin::fee::{script_input_vbytes, script_output_vbytes};`. If plan-phase finds the import dependency is awkward (e.g., fee.rs becomes a circular dep), fall back to (a) and re-export from fee.rs.
- **CD-41:** **`script_input_vbytes` const-fn vs plain fn** — Plan-phase decides based on whether the match arms over `ScriptType` are const-evaluable in stable Rust 2025 (they likely are; `ScriptType` is `#[derive(Clone, Copy)]`). Default: **const fn** (cheaper at all call sites, no runtime branch in the optimised binary). Fall back to plain fn if `const fn` requires nightly features.
- **CD-42:** **P2TR vbyte resolution** — Plan-phase research task (per D-124a): verify the canonical P2TR keypath worst-case vbyte against rust-bitcoin's `predict_weight` (or `tx.weight()` constructed via a fixture transaction). If 57.5 is confirmed, ship **58** (round UP per STATE.md, contradicts roadmap 57 — divergence documented inline). If a different number surfaces (54 from a more careful BIP-341 reading?), update D-124a's table and re-derive D-126's mixed-script math.
- **CD-43:** **`BipConfig::allowed_set` method name + return shape** — Default: `pub fn allowed_set(&self) -> impl Iterator<Item = ScriptType>`. Plan-phase MAY rename to `enabled_types()` or `allowed_input_types()` if that fits the existing BipConfig naming convention better. The load-bearing contract is "yields each enabled ScriptType exactly once".
- **CD-44:** **fee_share_p2wpkh_only test fixture amount** — Plan-phase decides input value (must be > denomination + 266 = 1_000_266; current test uses 1_100_000 which is fine). Default: reuse existing `make_inputs(3, 1_100_000)` from tx.rs::tests for consistency with sibling tests.
- **CD-45:** **mixed_script_e2e.rs amount-assertion refresh** — Plan-phase greps `mixed_script_e2e.rs` for hardcoded sats values and refreshes them per the new fee math. If the test asserts only "broadcast succeeded; txid observable in mempool" (no exact-amount assertions), no refresh needed.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing.**

### Project-level anchors

- `.planning/PROJECT.md` §"Constraints" — no custom crypto (Phase 20 touches no crypto primitives), no PII logging (script_type is an enum value, no per-participant PII; the existing structured logs already include `script_type = ?derived` at utxo.rs:111 — Phase 20 doesn't add new log call sites).
- `.planning/PROJECT.md` §"Current Milestone: v1.5 Audit-Readiness & Multi-Script Finish" — Phase 20 is the second v1.5 phase; closes the externally-visible mixed-script fee-accuracy gap; unblocks Phase 21's audit charter §"v=2 OwnershipProof PSBT handling" which wants to describe the *complete* multi-script verification + fee path.
- `.planning/REQUIREMENTS.md` §FEE-01 (line 60-ish) — Phase 20 Plan 20-01 closes verbatim (the lookup-function names, the literal vbyte numbers, the rounding direction).
- `.planning/REQUIREMENTS.md` §FEE-02 — Phase 20 Plan 20-01 closes verbatim (the plumbing path, the CRIT-01-derived field provenance, the uniform fee_share formula).
- `.planning/REQUIREMENTS.md` §FEE-03 — Phase 20 Plan 20-01 closes verbatim (the two regression test names + their assertion shapes).
- `.planning/REQUIREMENTS.md` §"Future requirements" — pins "per-input variable fee_share" + "mixed output script types per participant" as explicitly v1.6+ work; Phase 20 cannot implement either (would require wire-protocol changes outside v1.5 scope).
- `.planning/REQUIREMENTS.md` §Traceability — FEE-01/02/03 → Phase 20.
- `.planning/ROADMAP.md` §"Phase 20" — 5 success criteria. Phase 20 Plan 20-01 closes all 5.
- `.planning/STATE.md` §"v1.5 design notes" line 2 — "Phase 20's per-script weight table: use bitcoin::Weight::from_witness_data_size or hand-derived vbyte numbers from BIP-141 (P2WPKH input ~68 vB, P2TR input ~57.5 vB, P2SH-P2WPKH input ~91 vB; outputs P2WPKH 31 vB, P2TR 43 vB, P2SH-P2WPKH 32 vB). Rounding policy needs to be conservative (round UP) so the coordinator doesn't underpay fees on a mixed round." Binds D-124. NOTE: the "~57.5 vB" in STATE.md is the raw value before rounding; D-124a renders this as **58** (ceil) — divergence from ROADMAP SC#1's "57" (floor) documented inline per D-124a's research task.

### Phase 15/16 outputs (LOCKED inputs)

- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §"ScriptType / detect_script_type" — `detect_script_type(spk)` is the single canonical derivation function. Phase 20 uses the value `dispatch_ownership_proof` already computes internally; does NOT add a second call site for `detect_script_type`.
- `.planning/milestones/v1.4-phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §D-37 — output_script_type must be in the allowed set; configuration validation at boot enforces this. Binds D-122's worst-case formula (the configured output_script_type is the SOLE output type for the round).
- `.planning/decisions/v1.4-adr.md` §"Decision #4" — sign path split between bdk_wallet (descriptor) and shared::bip322 (WIF); orthogonal to Phase 20 (fee math is coordinator-side; client signing path unchanged).

### Phase 19 outputs (just shipped)

- `.planning/phases/19-multi-script-signing-finish/19-CONTEXT.md` §"Carried forward from Phase 14 ADR + Phases 15/16/17/18" — V1.4-CRIT-01 dispatcher-only invariant. Phase 20 preserves this on the fee-path side: ScriptType flows from the dispatcher's `detect_script_type` call (utxo.rs:152) into the fee math via the `UtxoDetails → RegisteredInput → ParticipantInput` plumbing; never client-touched.

### Specs / external references

- **BIP-141** (Segregated Witness, Consensus Layer) — vsize formula `vsize = (weight + 3) / 4` where `weight = 3 * non_witness_bytes + total_serialized_bytes` (equivalently `non_witness + witness/4` rounded up). Binds D-124a's input vbyte derivation and the conservative-rounding policy.
- **BIP-141** §"P2WPKH nested in BIP16 P2SH" — redeem script shape `OP_0 OP_PUSHBYTES_20 <HASH160(pubkey)>` (22 bytes); script_sig = `OP_PUSHBYTES_22 <redeem-script>` (23 bytes serialised with the length prefix). Binds D-124a's P2SH-P2WPKH non-witness math (64 bytes including the 23-byte script_sig).
- **BIP-340** §3.3 + BIP-341 §sign — P2TR keypath SIGHASH_DEFAULT signature is 64 bytes (NOT 65 — no sighash byte appended). Binds D-124a's P2TR witness math (66 bytes = 1 stack_count + 1 sig_len + 64 sig).
- **BIP-143** — DER ECDSA signature worst-case 72 bytes (71-byte signature + 1-byte sighash flag). Binds D-124a's P2WPKH + P2SH-P2WPKH witness math (108 bytes = 1 stack_count + 1 sig_len + 72 sig + 1 pk_len + 33 pubkey).
- **rust-bitcoin `bitcoin::Transaction::weight` / `bitcoin::Weight`** — for plan-phase verification of D-124a's hand-derived numbers against the library's authoritative computation (per CD-42).

### Code anchors (Phase 20 reads OR modifies)

- `coordinator/src/bitcoin/tx.rs:11,13` (`INPUT_WEIGHT_VBYTES = 68`, `OUTPUT_WEIGHT_VBYTES = 31` consts) — Plan 20-01 DELETES these.
- `coordinator/src/bitcoin/tx.rs:17-23` (`pub struct ParticipantInput`) — Plan 20-01 adds `pub script_type: shared::bip322::ScriptType` field.
- `coordinator/src/bitcoin/tx.rs:53-129` (`pub fn build_coinjoin_psbt`) — Plan 20-01 changes signature (adds `output_script_type: ScriptType` param), replaces the vsize formula with per-input weight sum + per-output multiply.
- `coordinator/src/bitcoin/tx.rs:131-224` (existing `#[cfg(test)] mod tests`) — Plan 20-01 adds inline unit tests for `script_input_vbytes`/`script_output_vbytes` (D-124c) + the 2 FEE-03 regression tests (D-125, D-126). Existing `make_inputs` helper extends to take a script_type param.
- `coordinator/src/bitcoin/fee.rs:11-18` (`pub fn estimate_fee_share`) — Plan 20-01 changes signature (adds `bip_config: &BipConfig` param) and body (worst-case formula per D-122).
- `coordinator/src/bitcoin/utxo.rs:37-40` (`pub struct UtxoDetails`) — Plan 20-01 adds `pub script_type: ScriptType` field.
- `coordinator/src/bitcoin/utxo.rs:62-118` (`pub async fn validate_utxo`) — Plan 20-01 captures the `derived` ScriptType from `dispatch_ownership_proof` (currently the value is used only in the tracing log at line 109-113) and returns it in `UtxoDetails`.
- `coordinator/src/round/state.rs:53-68` (`pub struct RegisteredInput`) — Plan 20-01 adds `pub script_type: ScriptType` field with `#[zeroize(skip)]`.
- `coordinator/src/api/handlers.rs:165` (`estimate_fee_share(max_participants_snap, fee_rate_snap)` call) — Plan 20-01 changes to `estimate_fee_share(&state.config.bip, max_participants_snap, fee_rate_snap)`.
- `coordinator/src/api/handlers.rs:475-481` (POST /round/input — `RegisteredInput` write site; needs to write `script_type: utxo_details.script_type`. Plan 20-01 modifies this insertion. NOTE: this specific block is the `ParticipantInput` construction inside `get_tx`; the actual `RegisteredInput` insertion happens elsewhere in the same file — plan-phase greps for `registered_inputs.insert(` or `RegisteredInput {`).
- `coordinator/src/api/handlers.rs:505` (`estimate_fee_share(n, fee_rate)` call inside `get_tx`) — Plan 20-01 changes to `estimate_fee_share(&state.config.bip, n, fee_rate)`.
- `coordinator/src/round/signing.rs:124-129` (`ParticipantInput` construction in `assemble_and_broadcast`) — Plan 20-01 writes `script_type: reg.script_type`.
- `coordinator/src/config.rs:158-187` (`pub struct BipConfig`) — Plan 20-01 reads (no struct change); adds `impl BipConfig { pub fn allowed_set(&self) -> impl Iterator<Item = ScriptType> }` helper per CD-43.

### Cross-phase invariant references

- `tests/integration/full_round.rs` (v1.3 invariant gate, ~1597 LOC) — Phase 20 makes NO changes. Run `cargo test --test integration full_round` after Plan 20-01 lands; expect 8/8 green, ~42s. The new fee math should produce IDENTICAL `fee_share` values for the uniform-P2WPKH tests in this file (which is the point of D-125's byte-exact baseline assertion).
- `tests/integration/mixed_script_e2e.rs` (v1.4 invariant gate) — Phase 20 makes MINIMAL changes (only if the test has hardcoded amount assertions — plan-phase greps per CD-45). The broadcast path still works (the new fee math doesn't change any sighash inputs; only the change-output amounts shift slightly).
- `shared/tests/bip322_cross_shape.rs` (9 cross-shape rejection tests, Phase 15) — Phase 20 makes NO changes (different module, different invariant).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`dispatch_ownership_proof` at `coordinator/src/bitcoin/utxo.rs:149+`** — already calls `detect_script_type(script_pubkey)` and returns the derived ScriptType in `Ok(derived)`. Phase 20 plumbs this same value through `UtxoDetails` instead of throwing it away — single-source-of-truth, zero re-derivation cost, exact CRIT-01 invariant preserved (the value comes from the same dispatcher branch that already vouched for the on-chain spk).

- **`shared::bip322::ScriptType` enum** at `shared/src/bip322/mod.rs:152` — `#[derive(Clone, Copy)]` 3-variant enum (P2wpkh, P2tr, P2shP2wpkh). Copy semantics — passing it through structs and matching on it is zero-cost. Already imported via `use shared::bip322::ScriptType` in `coordinator/src/config.rs:3` and elsewhere.

- **`shared::bip322::detect_script_type(spk)` at `shared/src/bip322/mod.rs:238-247`** — the canonical script-type derivation function. Returns `Result<ScriptType, Bip322Error>` (Err for unknown shapes, but `validate_utxo` only stores already-validated UTXOs, so the result is always Ok by the time it reaches Phase 20's path). Phase 20 does NOT add new call sites for this — uses the value `dispatch_ownership_proof` already computed.

- **`BipConfig::validate` at `coordinator/src/config.rs:223+`** — boot-time validation that `output_script_type` is in the allowed set (D-37). Binds the assumption in `script_output_vbytes(bip_config.output_script_type)` — the configured type is guaranteed to be one of the 3 known variants, no `Bip322Error::UnsupportedScriptType` arm reachable at runtime.

- **`#[zeroize(skip)]` on `script_pubkey` in RegisteredInput** at `coordinator/src/round/state.rs:64` — established pattern for public-chain-data fields. Phase 20's new `script_type` field follows the same annotation.

- **`tracing::info!(round_id = %round_id, script_type = ?derived, ...)` at utxo.rs:109-113** — already structured-logs the derived ScriptType post-validation. Phase 20 doesn't add new log call sites; the existing log already captures the value at the right moment.

### Established Patterns

- **WR-04 single canonical fee definition** — `fee.rs::estimate_fee_share` is the SOLE source of truth for both `get_tx` (display) and `assemble_and_broadcast` (PSBT). Phase 20 preserves this — only the formula inside changes. Plan-phase MUST keep both call sites consuming the same helper (no inline duplication of the worst-case math).

- **`pub fn` helper in `coordinator/src/bitcoin/` module** — the existing `estimate_fee_share` is a free `pub fn` (not a method on a struct). `script_input_vbytes` and `script_output_vbytes` follow the same style.

- **Inline derivation comments in const declarations** — the existing `INPUT_WEIGHT_VBYTES = 68` at tx.rs:11 has a `/// Estimated weight per input (P2WPKH): 68 vbytes` comment but no derivation. Phase 20's per-script return arms LIFT this pattern but ADD the BIP-141 math (D-124).

- **`#[cfg(test)] mod tests` inside the same file** — the existing tx.rs has 5 unit tests in the inline `mod tests` block. Phase 20's new tests follow the same pattern (inline in tx.rs::tests, not a separate integration test file). Plan-phase decides whether to split `script_input_vbytes` table tests into their own helper-test fn or inline them with the existing `coinjoin_psbt_*` tests.

- **`bitcoin_network` thread-through** — `parse_bitcoin_network(&state.config.network.bitcoin_network)` is the existing pattern for pulling sub-configs into handlers. Phase 20's `&state.config.bip` thread-through follows the same shape (no new infrastructure).

### Integration Points

- **Two call sites of `estimate_fee_share`** — `handlers.rs:165` (`fee_share_pre_lock`) and `handlers.rs:505` (`fee_per_participant_sats`). Both have `state.config.bip` in scope; the signature change is mechanical.

- **Two call sites of `build_coinjoin_psbt`** — `handlers.rs::get_tx` (around line 460-500, displays the assembled PSBT to clients) and `signing.rs::assemble_and_broadcast` (around line 100-140, builds the PSBT for actual broadcast). Both must pass the new `output_script_type: ScriptType` param. WR-04 requires byte-identical PSBTs from both call sites — the param value MUST come from the same source (`state.config.bip.output_script_type`).

- **`RegisteredInput` write site in POST /round/input handler** — Plan-phase greps for `registered_inputs.insert(` or `RegisteredInput {` in `coordinator/src/api/handlers.rs`. The write happens under the write lock after `validate_utxo` returns successfully; `utxo_details.script_type` is in scope by then.

- **Liquidity bot doesn't compute fees** — Phase 20 doesn't touch `liquidity-bot/` (the bot uses whatever fee_share the coordinator advertises). The CSV-rotated `BLINDJOIN_BOT_SCRIPT_TYPES` from Phase 18 INTEG-02 is the source of mixed-script rounds in tests; that machinery is unchanged.

</code_context>

<specifics>
## Specific Ideas

- **Phase 20 is a single tightly-coupled change** (per D-127) — the type-level coupling between `UtxoDetails.script_type`, `RegisteredInput.script_type`, and `ParticipantInput.script_type` means the plumbing must land as ONE commit. Splitting into "add the field" + "use the field" creates an intermediate state where the compiler accepts unused fields but the FEE-03 tests can't run. Plan-phase respects this — single plan, single executor pass, single atomic commit.

- **STATE.md "round UP" is the load-bearing rule for P2TR vbyte resolution** (per CD-42) — when the roadmap (57) and STATE.md (~57.5, round UP → 58) disagree, STATE.md wins because it's the milestone-level rounding *policy* and the roadmap was a planning *approximation*. The audit-charter prose at Phase 21 will cite STATE.md's policy, not the roadmap's number.

- **fee.rs::estimate_fee_share worst-case-from-BipConfig is the privacy-safe choice** — using anything narrower (e.g., "use the most-recently-registered script type") would leak ordering information across rounds; worst-case-across-allowed-set is uniform regardless of who registers first. This is a soft but real privacy property worth preserving even though Phase 20 doesn't explicitly call it out as a threat model item.

- **The mixed-script regression test's ≥1 sat divergence is the load-bearing assertion** — bytes match would catch typos; ≥1 sat divergence at fee_rate=2 catches "I forgot to use the per-script branch and just multiplied by INPUT_WEIGHT_VBYTES=68 for all 3 types". At the computed delta of 9 sats per participant, this test has comfortable headroom (any silent revert to P2WPKH-only would produce diff=0, immediately failing).

</specifics>

<deferred>
## Deferred Ideas

- **Validating `change_address` script_type matches `bip.output_script_type`** — currently the coordinator accepts any valid address for `change_address`; the weight calc assumes `output_script_type` uniformly. Slight inaccuracy if a participant submits a different-script-type change address (e.g., output_script_type=P2WPKH but change_address is P2TR — weight under-estimated by 12 vB per such change output). The uniform fee_share formula absorbs this for all other participants (over-pay for the off-type participant). Defer to v1.6+ if Phase 21 audit-charter review flags as a policy gap.

- **Per-input variable `fee_share`** — REQUIREMENTS.md `Future requirements` explicitly defers this. Changes the wire protocol (clients must accept variable amounts in the PSBT). Separate milestone in v1.6+.

- **Mixed output script types per participant** (Wasabi 2.0.3-style) — REQUIREMENTS.md `Out of v1.5 scope but not anti-features`. Separate output-policy milestone in v1.6+. Phase 20 keeps single-type-per-round output discipline.

- **B-03 dynamic fee estimation** (mempool-aware polling + RBF strategy) — pre-mainnet requirement, orthogonal to Phase 20's *accuracy* fixes (B-03 is about *responsiveness* to mempool changes). Carry-forward to v1.6+ per STATE.md.

- **Compute vbytes at coordinator startup via `bitcoin::Weight` + assert against pinned consts** — overkill for v1.5 (the const lookup table is the load-bearing artifact); higher audit value but high boot-complexity cost. v1.6+ if audit charter review wants stronger machine-verification of the table.

- **Promote `script_input_vbytes` / `script_output_vbytes` to `shared/` crate** — if the client ever needs per-script fee math (e.g., for change-address selection or pre-flight balance check), the table could live in `shared::bip322` next to `ScriptType`. Phase 20 keeps it coordinator-local (only the coordinator computes fees today). v1.6+ if client-side fee preview becomes a feature.

- **`BipConfig::allowed_set` could be a method or a derived field** — Phase 20 ships it as a method per CD-43. If multiple call sites need it (Phase 21 audit charter, future client-side advertising), promote to a cached `Vec<ScriptType>` field populated at boot. v1.6+ if usage patterns warrant.

</deferred>

---

*Phase: 20-mixed-round-fee-accuracy*
*Context gathered: 2026-05-31*
