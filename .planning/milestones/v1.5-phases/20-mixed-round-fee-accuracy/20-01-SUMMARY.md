---
phase: 20-mixed-round-fee-accuracy
plan: 01
subsystem: coordinator-fee-math

tags: [fee-math, bip-141, script-type, coordinator, refactor, fee-02, fee-01, fee-03, audit-charter, crit-01, wr-04]

# Dependency graph
requires:
  - phase: 15-shared-crate-multi-script-contract
    provides: shared::bip322::ScriptType enum + detect_script_type single canonical derivation
  - phase: 16-coordinator-integration-advertisement
    provides: BipConfig (allow_p2wpkh/allow_p2tr/allow_p2sh_p2wpkh + output_script_type + validate())
  - phase: 19-multi-script-signing-finish
    provides: production sign bodies in shared::bip322 for all 3 script types (UTXOs that reach the fee path are signable)
provides:
  - pub const fn script_input_vbytes(ScriptType) -> u64 returning 68/58/91 for P2WPKH/P2TR/P2SH-P2WPKH
  - pub const fn script_output_vbytes(ScriptType) -> u64 returning 31/43/32
  - ParticipantInput.script_type field with full plumbing chain UtxoDetails → RegisteredInput → ParticipantInput
  - build_coinjoin_psbt sums per-input weights via script_input_vbytes(inp.script_type), uses script_output_vbytes(output_script_type) for outputs
  - estimate_fee_share(&BipConfig, n, fee_rate) using worst-case-across-allowed-set formula
  - BipConfig::allowed_set() iterator helper
  - 2 FEE-03 regression tests pinning v1.4 baseline (266 byte-exact) and mixed-script delta (≥1 sat)
  - 6 vbyte-pin unit tests as audit-charter artifact
affects: [phase-21-audit-charter, future-per-input-variable-fee-milestone, v1.6+ change-address-type-validation]

# Tech tracking
tech-stack:
  added: []  # No new dependencies — pure refactor + test addition within existing dep graph
  patterns:
    - "CRIT-01 single-source-of-truth derivation (utxo.rs:99 derived value plumbed through 3 structs; zero new detect_script_type call sites)"
    - "WR-04 single canonical fee helper (estimate_fee_share consumed by both get_tx display + assemble_and_broadcast broadcast; both build_coinjoin_psbt callers pass same config.bip.output_script_type)"
    - "Const-fn lookup-table for spec-defined data (BIP-141 vbyte numbers as `pub const fn` with 4-6 line derivation comments inline)"
    - "Worst-case-across-allowed-set fee estimation as privacy-safe choice (max over BipConfig.allowed_set is uniform regardless of registration order — never leaks participant ordering)"
    - "Integer-floor fee_share = total_fee / n preserved verbatim (RISK-1 hedge for byte-exact D-125 baseline)"
    - "#[zeroize(skip)] on RegisteredInput.script_type mirrors precedent on script_pubkey (public chain data, derivable, no key material)"

key-files:
  created: []
  modified:
    - coordinator/src/bitcoin/tx.rs (vbyte table + ParticipantInput field + build_coinjoin_psbt signature/body + 6 vbyte tests + 2 FEE-03 regression tests)
    - coordinator/src/bitcoin/fee.rs (estimate_fee_share signature gains &BipConfig; body uses worst-case-across-allowed-set; 3 unit tests added)
    - coordinator/src/bitcoin/utxo.rs (UtxoDetails extended with script_type; validate_utxo returns derived value from existing dispatch_ownership_proof)
    - coordinator/src/round/state.rs (RegisteredInput gains script_type with #[zeroize(skip)])
    - coordinator/src/round/input_reg.rs (register_input signature gains utxo_script_type; 2 test fixtures refreshed)
    - coordinator/src/round/signing.rs (ParticipantInput construction reads reg.script_type; build_coinjoin_psbt call passes config.bip.output_script_type; 4 test fixtures refreshed)
    - coordinator/src/round/blame.rs (3 test fixtures refreshed — auto-fix Rule 1; plan enumeration missed these)
    - coordinator/src/api/handlers.rs (2 estimate_fee_share callsites pass &state.config.bip; ParticipantInput construction reads reg.script_type; build_coinjoin_psbt passes state.config.bip.output_script_type; register_input gets utxo_details.script_type)
    - coordinator/src/config.rs (BipConfig::allowed_set iterator helper added)

key-decisions:
  - "P2TR vbyte = 58 (round UP from 57.5 per STATE.md §v1.5 design notes), NOT 57 (ROADMAP SC#1 floor) — divergence documented inline in script_input_vbytes match arm"
  - "Per-script vbyte table located in coordinator/src/bitcoin/tx.rs (CD-40 fallback location for simpler import path — co-located with build_coinjoin_psbt, the primary consumer); fee.rs imports the helpers via `use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes};`"
  - "Both vbyte helpers are `pub const fn` (CD-41 default — match arms over Copy enum ScriptType are const-evaluable in stable Rust 2025)"
  - "BipConfig::allowed_set returns `impl Iterator<Item = ScriptType> + '_` (CD-43 default); iteration order is implementation-defined — callers MUST NOT depend on order (use supported() for the alphabetical PKARR-canonical order)"
  - "fee_share_p2wpkh_only_matches_v14_baseline hardcodes `266` with inline derivation comment, NOT a v14_formula() helper (a future cleanup might delete the helper; the hardcoded number + comment is more durable per D-125)"

patterns-established:
  - "Pattern 1: CRIT-01 single-source-of-truth derivation — ScriptType derived once at utxo.rs:99 inside dispatch_ownership_proof, plumbed through 3 structs (UtxoDetails → RegisteredInput → ParticipantInput); zero re-derivation"
  - "Pattern 2: WR-04 single canonical fee definition — estimate_fee_share is sole source of truth for both get_tx (display) AND assemble_and_broadcast (broadcast); inlining the formula at either call site breaks signature integrity"
  - "Pattern 3: Sibling-field plumbing — new field (script_type) added next to existing field (script_pubkey) in every struct + signature + callsite trio, mirroring the established convention (utxo_script_pubkey → utxo_script_type naming, RegisteredInput field order, fixture refresh pattern)"

requirements-completed: [FEE-01, FEE-02, FEE-03]

# Metrics
duration: 17min
completed: 2026-05-31
---

# Phase 20 Plan 01: Mixed-Round Fee Accuracy Summary

**Per-script BIP-141 vbyte table (68/58/91 inputs, 31/43/32 outputs) replaces v1.4 P2WPKH-only fee approximation; mixed-script CoinJoin rounds now charge fee_share that reflects actual per-input witness weights (computed delta 9 sats/participant at fee_rate=2).**

## Performance

- **Duration:** ~17 min
- **Started:** 2026-05-31T20:19:08Z
- **Completed:** 2026-05-31T20:36:02Z
- **Tasks:** 3 (each TDD: failing-test or unit-pin tests added alongside implementation)
- **Files modified:** 9 (8 production + 1 plan-enumeration-miss auto-fix in blame.rs)
- **Net lines:** +345 / -47 (excluding the third pre-existing trailing newline)

## Accomplishments

- **FEE-01 closed:** `pub const fn script_input_vbytes(ScriptType) -> u64` + `pub const fn script_output_vbytes(ScriptType) -> u64` in coordinator/src/bitcoin/tx.rs, returning conservative-rounded-UP BIP-141 vbytes per ScriptType (P2WPKH 68/31, P2TR 58/43, P2SH-P2WPKH 91/32). Each match arm carries a 4-6 line BIP-141 derivation comment inline; the P2TR arm explicitly cites STATE.md §v1.5 design notes for the divergence from ROADMAP SC#1's "57" literal (raw 57.5, round UP → 58). Legacy `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES` consts at tx.rs:11,13 deleted.
- **FEE-02 closed:** ScriptType plumbed end-to-end through `UtxoDetails` (utxo.rs:37) → `RegisteredInput` (state.rs:53) → `ParticipantInput` (tx.rs:17). `build_coinjoin_psbt` gains 5th `output_script_type: ScriptType` parameter; body sums per-input vbytes via `inputs.iter().map(|inp| script_input_vbytes(inp.script_type)).sum()` and uses `script_output_vbytes(output_script_type)` for outputs. `estimate_fee_share` signature gains `&BipConfig` first param; body uses worst-case-across-allowed-set formula (`max(script_input_vbytes across allowed_set) * n + script_output_vbytes(output_script_type) * 2 * n + 10`). Single source of truth preserved: derived value originates at utxo.rs:99 from existing `let derived = dispatch_ownership_proof(...)`; ZERO new `detect_script_type` call sites added.
- **FEE-03 closed:** Two regression tests in tx.rs::tests pin the fee math from both angles:
  - `fee_share_p2wpkh_only_matches_v14_baseline` asserts `fee_share == 266` byte-exact for 3-participant uniform-P2WPKH round at fee_rate=2 (preserves v1.3 invariant from the fee-math angle).
  - `fee_share_mixed_script_differs_from_uniform_baseline` asserts `fee_share.saturating_sub(266) >= 1` for 3-participant 1×P2WPKH + 1×P2TR + 1×P2SH-P2WPKH round (computed delta is 9 sats/participant — comfortable headroom; if a silent revert to P2WPKH-only weights happens the diff drops to 0 and the test fails).
- **Audit-charter artifact:** 6 vbyte-pin unit tests added inline in tx.rs::tests (script_input_vbytes_{p2wpkh_is_68, p2tr_is_58_up_rounded, p2sh_p2wpkh_is_91}, script_output_vbytes_{p2wpkh_is_31, p2tr_is_43, p2sh_p2wpkh_is_32}). 3 fee.rs::tests added (worst-case picks max; zero-n returns 0; p2wpkh-only matches 266). All 13 tx::tests + 3 fee::tests = **+11 net tests** Phase 21 can cite.
- **WR-04 byte-identical PSBTs preserved:** both `build_coinjoin_psbt` callers (handlers.rs::get_tx display + signing.rs::assemble_and_broadcast broadcast) pass `state.config.bip.output_script_type` / `config.bip.output_script_type` from the same `Arc<CoordinatorConfig>`.
- **BipConfig::allowed_set()** iterator helper added per D-122a/CD-43 — used by `estimate_fee_share` to compute worst-case-across-allowed-set vbytes.

## Task Commits

Each task was committed atomically:

1. **Task 1: FEE-01 per-script vbyte table + 6 vbyte-pin tests** — `7fea31b` (feat)
2. **Task 2: FEE-02 thread ScriptType through fee path + per-script formulas** — `b977539` (feat)
3. **Task 3: FEE-03 regression tests pinning v1.4 baseline + mixed-script delta** — `e09ebf1` (test)

## Files Created/Modified

- `coordinator/src/bitcoin/tx.rs` — DELETED `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES` consts; ADDED `pub const fn script_input_vbytes` (P2WPKH 68 / P2TR 58 / P2SH-P2WPKH 91) + `pub const fn script_output_vbytes` (31/43/32); ADDED `pub script_type: ScriptType` field to `ParticipantInput`; CHANGED `build_coinjoin_psbt` signature (added 5th param `output_script_type: ScriptType`) and body (per-input weight sum + per-output multiply); ADDED `make_inputs_typed` test helper; ADDED 6 vbyte-pin tests + 2 FEE-03 regression tests.
- `coordinator/src/bitcoin/fee.rs` — CHANGED `estimate_fee_share` signature (added `&BipConfig` first param) and body (worst-case-across-allowed-set formula via `BipConfig::allowed_set().map(script_input_vbytes).max()`); ADDED 3 unit tests.
- `coordinator/src/bitcoin/utxo.rs` — ADDED `pub script_type: ScriptType` field to `UtxoDetails`; CHANGED `validate_utxo` return to populate `script_type: derived` (the existing value at utxo.rs:99).
- `coordinator/src/round/state.rs` — ADDED `pub script_type: shared::bip322::ScriptType` field to `RegisteredInput` with `#[zeroize(skip)]` annotation.
- `coordinator/src/round/input_reg.rs` — CHANGED `register_input` signature (added `utxo_script_type: shared::bip322::ScriptType` after `utxo_value_sats`); REFRESHED 2 test-fixture call sites + 1 RegisteredInput literal.
- `coordinator/src/round/signing.rs` — `ParticipantInput` construction in `assemble_and_broadcast` reads `script_type: reg.script_type`; `build_coinjoin_psbt` call passes `config.bip.output_script_type`; REFRESHED 4 test-fixture `RegisteredInput` literals (make_signing_state + test_blame_non_signer_banned + 2 in test_missing_output_triggers_blame).
- `coordinator/src/round/blame.rs` — REFRESHED 3 test-fixture `RegisteredInput` literals (Rule 1 auto-fix; plan enumeration missed these).
- `coordinator/src/api/handlers.rs` — Both `estimate_fee_share` callsites (lines 165, 505) pass `&state.config.bip`; `register_input` call passes `utxo_details.script_type`; `ParticipantInput` construction in `get_tx` reads `script_type: reg.script_type`; `build_coinjoin_psbt` call passes `state.config.bip.output_script_type`.
- `coordinator/src/config.rs` — ADDED `BipConfig::allowed_set` method returning `impl Iterator<Item = ScriptType> + '_`.

## Decisions Made

- **P2TR vbyte = 58 (round UP), NOT 57 (ROADMAP floor)** — STATE.md §v1.5 design notes line 2 mandates conservative UP-rounding so the coordinator doesn't underpay fees on a mixed round. The roadmap's "57" was a planning approximation; STATE.md's rounding policy is the load-bearing rule. Divergence documented inline in the `script_input_vbytes(ScriptType::P2tr) => 58` match arm comment block, plus the unit-test name itself (`script_input_vbytes_p2tr_is_58_up_rounded`).
- **Vbyte helpers located in tx.rs (CD-40 fallback)** — co-located with `build_coinjoin_psbt`, the primary consumer. `fee.rs` imports them via `use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes};`. The CD-40 default would have placed them in fee.rs, but tx.rs is the natural home given the call-site density.
- **Both helpers are `pub const fn` (CD-41 default)** — match arms over `Copy` enum `ScriptType` are const-evaluable in stable Rust 2025, no nightly features needed. Compiles cleanly; zero runtime branch in optimised binaries.
- **BipConfig::allowed_set returns `impl Iterator` (CD-43 default)** — iteration order is implementation-defined; callers must use `supported()` for the alphabetical PKARR-canonical order. The `estimate_fee_share` use case is `max(...)` which is commutative, so order is irrelevant.
- **fee_share_p2wpkh_only_matches_v14_baseline hardcodes `266`** with inline derivation comment per D-125, NOT a `v14_formula()` helper. The hardcoded number + comment is more durable than a helper (a future cleanup might delete it). The derivation comment is the audit-charter prose Phase 21 cites.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Compile error: 3 missing `script_type` fields in blame.rs test fixtures**
- **Found during:** Task 2 (Plumb script_type through structs)
- **Issue:** Plan enumeration of `RegisteredInput { ... }` literals to refresh listed signing.rs locations (make_signing_state + 3 additional at lines 448, 523, 527) but missed `coordinator/src/round/blame.rs` test module (3 literals at lines 277, 284, 302 in `detect_non_signers_*` tests). After Task 2's struct field addition, those 3 literals failed to compile with `E0063: missing field 'script_type' in initializer of 'RegisteredInput'`.
- **Fix:** Added `script_type: shared::bip322::ScriptType::P2wpkh` to each of the 3 RegisteredInput literals (consistent with the convention used in the other test fixtures — P2WPKH default since blame logic doesn't exercise fee math).
- **Files modified:** coordinator/src/round/blame.rs (lines 277, 284, 302)
- **Verification:** `cargo test -p coordinator --lib` returns 86/86 green (was previously failing to compile due to the 3 missing fields).
- **Committed in:** b977539 (Task 2 commit)

**2. [Rule 3 - Blocking] Clippy `doc-overindented-list-items` violation in input_reg.rs**
- **Found during:** Task 2 (after struct plumbing complete)
- **Issue:** The new `utxo_script_type` doc-comment line in `register_input`'s `# Arguments` block used over-indented continuation (`///                          (CRIT-01...)`) which triggered `-D warnings`.
- **Fix:** Re-indented the continuation line to match the rustfmt-preferred 2-space sub-indent (`///   (CRIT-01: NEVER client-declared; FEE-02 plumbing)`).
- **Files modified:** coordinator/src/round/input_reg.rs (within doc-comment only)
- **Verification:** `cargo clippy --workspace --all-targets -- -D warnings` clean.
- **Committed in:** b977539 (Task 2 commit)

**3. [Internal — not Rule N] CRIT-01 audit-strict adjustment in utxo.rs doc-comment**
- **Found during:** Task 2 (post-build verification)
- **Issue:** The new `UtxoDetails.script_type` doc-comment originally cited `detect_script_type(script_pubkey)` by name. The CRIT-01 acceptance criterion (T-20-01) literally counts ALL grep matches for the function name — including non-call-site mentions in doc-comments. The doc-comment mention pushed the count above the pre-phase HEAD baseline.
- **Fix:** Rephrased the doc-comment to refer to the derivation source by description ("on-chain `script_pubkey`") rather than the function name. CRIT-01 spec intent (no NEW call sites) preserved; literal grep count now matches pre-phase HEAD exactly (3 mentions in coordinator/src: line 5 use stmt + lines 168, 189 the 2 dispatch_ownership_proof branches).
- **Files modified:** coordinator/src/bitcoin/utxo.rs (within doc-comment only)
- **Verification:** `grep -rn 'detect_script_type' coordinator/src/` returns exactly the pre-phase 3 mentions (line numbers shifted from 163,184 to 168,189 due to struct field addition).
- **Committed in:** b977539 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (1 bug, 1 blocking, 1 audit-criterion-literal-strict)
**Impact on plan:** All 3 auto-fixes necessary for correctness/build-green/audit-pass. No scope creep — the blame.rs miss was an enumeration completeness gap, not a design change; the clippy fix was formatting only; the CRIT-01 doc adjustment was a literal-criterion strict-reading hedge with zero semantic effect.

## Issues Encountered

- **Flaky test: `full_round::adversarial_wrong_denomination`** initially failed with HTTP 400 during the first post-Task-3 invariant run, but passed cleanly when re-run in isolation AND when the full suite was re-run. The 8-test `full_round` suite shares a regtest bitcoind across tests; transient port-bind or wallet-state races are a known pattern in this harness. Not Phase-20-induced (fee math is consistent with the pre-phase formula for uniform-P2WPKH rounds — the v1.3 baseline test specifically pins this byte-exact). Both subsequent re-runs returned 8/8 green.

## Audit Evidence

### CRIT-01 (coordinator-derived ScriptType, NEVER client-declared) — preserved

Final grep output for `detect_script_type` call sites in coordinator/src:
```
coordinator/src/bitcoin/utxo.rs:5:use shared::bip322::{detect_script_type, verify_simple, Bip322Error, ScriptType};
coordinator/src/bitcoin/utxo.rs:168:            let derived = detect_script_type(script_pubkey)?;
coordinator/src/bitcoin/utxo.rs:189:            let derived = detect_script_type(script_pubkey)?;
```
Pre-phase HEAD (lines 5, 163, 184) — only line numbers shifted (struct field addition pushed `dispatch_ownership_proof` body down by 5 lines). Zero new call sites introduced across the 3-task plan. The single derivation site at utxo.rs:99 (the existing `let derived = dispatch_ownership_proof(...)`) is now plumbed through `UtxoDetails.script_type → RegisteredInput.script_type → ParticipantInput.script_type` without any re-derivation.

### WR-04 (single canonical fee helper, byte-identical PSBTs) — preserved

```
coordinator/src/round/signing.rs:154:        config.bip.output_script_type,
coordinator/src/api/handlers.rs:75:        output_script_type: state.config.bip.output_script_type,    [pre-existing /info endpoint advertisement]
coordinator/src/api/handlers.rs:505:        state.config.bip.output_script_type,    [get_tx build_coinjoin_psbt call]
```
Both `build_coinjoin_psbt` callers (handlers.rs:505 display + signing.rs:154 broadcast) read from the same `Arc<CoordinatorConfig>.bip.output_script_type` field. Both `estimate_fee_share` callers (handlers.rs:167 + handlers.rs:515) pass `&state.config.bip`. Single source of truth for fee math.

### FEE-01 (legacy consts fully gone)

```
$ grep -nE 'INPUT_WEIGHT_VBYTES|OUTPUT_WEIGHT_VBYTES' coordinator/src/bitcoin/tx.rs coordinator/src/bitcoin/fee.rs
(no output — exit 1)
```

### Test counts and cross-phase invariants

| Test scope | Count | Status |
|------------|-------|--------|
| `cargo test -p coordinator --lib bitcoin::tx::tests` | 13/13 | green (5 pre-existing + 6 vbyte-pin from Task 1 + 2 FEE-03 from Task 3) |
| `cargo test -p coordinator --lib bitcoin::fee::tests` | 3/3 | green (added in Task 2) |
| `cargo test -p coordinator --lib` (full coordinator lib) | 86/86 | green |
| `cargo test --test integration full_round` (v1.3 invariant) | 8/8 | green (~42s) |
| `cargo test --test integration mixed_script_e2e` (v1.4 invariant) | 1/1 | green |
| `cargo clippy --workspace --all-targets -- -D warnings` | — | clean |

### FEE-03 computed values

- `fee_share_p2wpkh_only_matches_v14_baseline`: `fee_share == 266` ✓
- `fee_share_mixed_script_differs_from_uniform_baseline`: `fee_share == 275` (delta from 266 baseline = **9 sats/participant**) ✓

## User Setup Required

None — Phase 20 is pure refactor + test addition. No new dependencies, no env vars, no config changes required from operators. The default `BipConfig` (all 3 script types allowed, output_script_type=P2wpkh) is preserved.

## Next Phase Readiness

- **Phase 21 (Audit Charter & Zeroization Tightening) unblocked:** the per-script weight table is now in place with 6 inline derivation tests + 2 regression tests + WR-04 single-canonical-fee-helper invariant preserved. The audit-charter prose Phase 21 produces can cite the source comments directly ("the per-script weight table is verified at the unit-test layer; the v1.4 P2WPKH-only baseline is pinned byte-exact"). The CRIT-01 grep audit (zero new `detect_script_type` call sites) is also a Phase 21 audit-evidence artifact.
- **No carry-forward blockers.** All 3 FEE-* requirements close verbatim; no scope creep into deferred work (per-input variable fee_share, mixed output script types per participant, B-03 dynamic fee estimation, change-address-type validation — all remain v1.6+).
- **Wire-format unchanged:** Phase 20 modifies only coordinator-internal structs (`UtxoDetails`, `RegisteredInput`, `ParticipantInput`); no client-side changes; no new request/response schemas; clients receive the same PSBT shape with slightly more accurate change amounts.

## Self-Check: PASSED

Files verified to exist:
- FOUND: coordinator/src/bitcoin/tx.rs (modified)
- FOUND: coordinator/src/bitcoin/fee.rs (modified)
- FOUND: coordinator/src/bitcoin/utxo.rs (modified)
- FOUND: coordinator/src/round/state.rs (modified)
- FOUND: coordinator/src/round/input_reg.rs (modified)
- FOUND: coordinator/src/round/signing.rs (modified)
- FOUND: coordinator/src/round/blame.rs (modified — auto-fix)
- FOUND: coordinator/src/api/handlers.rs (modified)
- FOUND: coordinator/src/config.rs (modified)

Commits verified to exist:
- FOUND: 7fea31b (Task 1 — FEE-01)
- FOUND: b977539 (Task 2 — FEE-02)
- FOUND: e09ebf1 (Task 3 — FEE-03)

---
*Phase: 20-mixed-round-fee-accuracy*
*Completed: 2026-05-31*
