---
phase: 16-coordinator-integration-advertisement
plan: 03
subsystem: discovery
tags: [coordinator, pkarr, advertisement, dht, byte-budget, multi-script, bip-322]

# Dependency graph
requires:
  - phase: 16-coordinator-integration-advertisement
    provides: "Plan 16-01 — BipConfig::supported() + BipConfig::output_script_type fields on CoordinatorConfig.bip"
  - phase: 16-coordinator-integration-advertisement
    provides: "Plan 16-02 — validate_utxo multi-script dispatcher (no overlap with PKARR but boundary preserved; CRIT-01 anchor stable at 2)"
  - phase: 15
    provides: "shared::bip322::ScriptType enum with snake_case + kebab-case serde wire form"
provides:
  - "PKARR record schema v0.2.0 with compact-name field layout (v/ds/mp/st/n + sst + ost)"
  - "Two CI byte-budget regression gates (production .onion < 220 + dev-mode localhost < 200)"
  - "Producer-side ADVERT-02 closure — Phase 17 WALLET-03/04 inherits the compact-code wire shape"
affects: [phase-17-client-multi-script-wallet-discovery, phase-18-mixed-script-e2e]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "PKARR JSON byte-budget regression gate at two tiers (production .onion + dev localhost) per D-44 + D-55 + B3"
    - "Compact-field-name encoding for DNS-TXT-bounded structured records (v/ds/mp/st/n etc.) — preserves load-bearing names selectively (type for schema id, onion for backwards client compat)"
    - "Inline ScriptType -> &str wire-form match at call site (single source of truth for PKARR wire form; decouples call site from ScriptType's Serialize impl)"
    - "Heartbeat-task hoisting: static config-derived fields computed ONCE at spawn, dynamic round-state fields recomputed per tick"

key-files:
  created: []
  modified:
    - "coordinator/src/discovery/pkarr_pub.rs"
    - "coordinator/src/run.rs"
    - ".planning/phases/16-coordinator-integration-advertisement/deferred-items.md"

key-decisions:
  - "PKARR schema v0.2.0 ships with B3 compact-name rename in a single atomic commit (Task 1) — verbose names would breach 220-byte production budget by ~1 byte"
  - "type and onion field names deliberately preserved (type is already 4 bytes and schema-identifies; onion is load-bearing for v1.3 Partial { onion } resolver per RESEARCH §V1.4-MOD-02)"
  - "Heartbeat call site HOISTS supported_strs + output_st_owned out of the per-iteration loop into spawn-task outer scope (BipConfig is static, never changes at runtime)"
  - "v1.5 deferral note: when 4th+ script type lands, re-evaluate sst encoding (single-char codes / bitmask / hash-of-sorted-set) — current 220-byte budget has ~11 bytes headroom at production worst case"

patterns-established:
  - "Pattern: Byte-budget CI regression gate at TWO tiers (production worst case + dev headroom) for any DNS-TXT-bounded structured field"
  - "Pattern: W3 transient-stub atomic-commit discipline — Task 1 adds #[allow(unused_variables)] stubs to keep workspace buildable; Task 2 explicitly removes them in its first edit"

requirements-completed: [ADVERT-02]

# Metrics
duration: ~13 min
completed: 2026-05-30
---

# Phase 16 Plan 16-03: PKARR Record v0.2.0 Advertisement Summary

**PKARR DHT record bumped from `version=0.1.0` (verbose names) to `v=0.2.0` (compact names) with new `sst` (supported script types CSV) + `ost` (output script type) advertisement fields, gated by two inline CI byte-budget regression tests (production `.onion` < 220 bytes; dev-mode localhost < 200 bytes).**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-05-30T05:12:17Z (continuation from 16-02 close)
- **Completed:** 2026-05-30T05:25:42Z
- **Tasks:** 2 (auto, both tdd=true)
- **Files modified:** 3 (`coordinator/src/discovery/pkarr_pub.rs`, `coordinator/src/run.rs`, `.planning/phases/16-coordinator-integration-advertisement/deferred-items.md`)

## Accomplishments

- **ADVERT-02 fully closed.** The InfoResponse half landed in 16-01; this plan ships the PKARR advertisement half so clients can fail-fast at discovery time before opening a Tor circuit (Phase 17 WALLET-03 consumes; Phase 18 INTEG-02 liquidity bot consumes).
- **PKARR schema bumped to `"v": "0.2.0"`** with two new advertisement fields (`sst` + `ost`) per D-39..D-43.
- **B3 compact-name migration applied** in a single atomic commit — 5 verbose field names compacted (`version` → `v`, `denomination_sats` → `ds`, `min_participants` → `mp`, `status` → `st`, `network` → `n`) saving ~56 bytes of headroom; `type` and `onion` preserved (schema-identifier + v1.3 client compat respectively).
- **Two CI byte-budget regression gates established:**
  - `coordinator_packet_under_220_byte_budget_production_onion` — production worst case (62-byte Tor v3 `.onion` + all-3-allowed CSV) measured at **209 bytes** (11 bytes of headroom under the 220-byte DNS-TXT warn).
  - `coordinator_packet_under_200_byte_budget_dev_mode` — dev mode (14-byte `127.0.0.1:8080` + all-3-allowed CSV) measured at **161 bytes** (39 bytes of headroom).
- **v1.3 cross-phase invariant green:** `cargo test --test integration full_round` 8/8 pass at the plan boundary.
- **Phase 16-02 CRIT-01 invariant preserved:** `grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` returns 2 (this plan does not touch utxo.rs; the 16-02 CI grep gate still passes).

## Task Commits

Each task was committed atomically per CD-10:

1. **Task 1: Bump build_coordinator_packet signature + JSON schema (compact-name rename + sst/ost) + 7 new inline tests + transient run.rs stub** — `d1a1912` (feat, includes W3 transient stub in run.rs to preserve atomic-commit buildability)
2. **Task 2: Remove transient stub + wire both run.rs PKARR publish call sites to derive args from cfg.bip** — `146e7c3` (feat, removes the W3 stub introduced by Task 1 and wires `cfg.bip.supported()` + `cfg.bip.output_script_type`)

**Plan metadata commit:** Pending — produced after this SUMMARY is written.

## Files Created/Modified

- **`coordinator/src/discovery/pkarr_pub.rs`** (lines 52-96 schema; lines 156-432 tests)
  - Signature change: `build_coordinator_packet` gained two new args at end of arg list (`supported: &[&str]`, `output_script_type: &str`).
  - JSON literal swapped from 7 verbose fields to 9 compact + 2 advertisement fields.
  - 3 existing tests updated to call with new signature + assert compact field names.
  - 7 new tests added including the two budget regression gates.
- **`coordinator/src/run.rs`** (lines 327-367 initial publish; lines 369-425 heartbeat)
  - Both PKARR publish call sites derive new args from `cfg.bip.supported()` + `cfg.bip.output_script_type` via an inline `ScriptType -> &str` match.
  - Heartbeat call site HOISTS `supported_strs` + `output_st_owned` out of the per-tick loop (BipConfig is static).
  - W2 dynamic `&status` source preserved at the heartbeat call site.
- **`.planning/phases/16-coordinator-integration-advertisement/deferred-items.md`** — Appended re-confirmation that the 14 pre-existing shared/src/bip322/* clippy lints from 16-02 remain out of scope and persist verbatim through 16-03 (which touches zero shared/ files).

## Final `build_coordinator_packet` Signature

```rust
pub fn build_coordinator_packet(
    keypair: &Keypair,
    coordinator_addr: &str,
    denomination_sats: u64,
    min_participants: u32,
    status: &str,
    supported: &[&str],
    output_script_type: &str,
) -> Result<SignedPacket>
```

## Final JSON Literal (verbatim from `coordinator/src/discovery/pkarr_pub.rs`)

```rust
let record = serde_json::json!({
    "type": "blindjoin-coordinator",
    "v": "0.2.0",
    "onion": coordinator_addr,
    "n": "signet",
    "ds": denomination_sats,
    "mp": min_participants,
    "st": status,
    "sst": supported.join(","),
    "ost": output_script_type,
});
```

## Byte-Budget Regression Gate Bodies (verbatim, for Phase 17 / 18 traceability)

### `coordinator_packet_under_220_byte_budget_production_onion` (production tier)

```rust
#[test]
fn coordinator_packet_under_220_byte_budget_production_onion() {
    // 56 base32 chars + ".onion" = 62 bytes total (production Tor v3 length).
    // [Rule 1 — Bug]: the PLAN literal had only 54 x's (= 60 bytes total).
    // Real Tor v3 .onion is 56 base32 chars; using 60 bytes would under-
    // approximate the worst case and weaken the regression gate. Padded to
    // 56 x's so the assertion truly bounds the PROJECT-constraint worst case.
    let onion = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.onion";
    assert_eq!(onion.len(), 62, "fixture .onion must be 62 bytes (Tor v3 length)");
    let json_str = build_record_json(
        onion,
        1_000_000,
        3,
        "idle",
        &["p2sh-p2wpkh", "p2tr", "p2wpkh"],
        "p2wpkh",
    );
    let len = json_str.len();
    assert!(
        len < 220,
        "PKARR byte-budget regression gate: production .onion worst case payload \
         must stay under the 220-byte DNS-TXT warn threshold; \
         got {len} bytes. Reduce field-name length or descope a field. \
         Payload: {json_str}",
    );
}
```

### `coordinator_packet_under_200_byte_budget_dev_mode` (dev tier)

```rust
#[test]
fn coordinator_packet_under_200_byte_budget_dev_mode() {
    let localhost = "127.0.0.1:8080";
    assert_eq!(localhost.len(), 14, "dev-mode fixture must be 14 bytes");
    let json_str = build_record_json(
        localhost,
        1_000_000,
        3,
        "idle",
        &["p2sh-p2wpkh", "p2tr", "p2wpkh"],
        "p2wpkh",
    );
    let len = json_str.len();
    assert!(
        len < 200,
        "PKARR byte-budget regression gate: dev-mode headroom payload \
         must stay under 200 bytes; got {len} bytes. \
         A future field addition that breaches this AND \
         coordinator_packet_under_220_byte_budget_production_onion must \
         trigger an encoding-compaction ADR. Payload: {json_str}",
    );
}
```

## Measured Worst-Case Byte Counts (regression-gate baseline)

| Tier        | Address fixture                                                  | Supported    | Output | Actual JSON length |
| ----------- | ---------------------------------------------------------------- | ------------ | ------ | ------------------ |
| Production  | `xxx…xxx.onion` (62 bytes; 56 base32 + `.onion`)                | all-3 CSV   | p2wpkh | **209 bytes**     |
| Dev mode    | `127.0.0.1:8080` (14 bytes)                                      | all-3 CSV   | p2wpkh | **161 bytes**     |

**Headroom at the regression gates:**
- Production: 220 − 209 = **11 bytes** of headroom (~2 more avg kebab-case CSV components OR 2-3 more compact-name fields).
- Dev mode:   200 − 161 = **39 bytes** of headroom.

**Future-field-addition reasoning rule:** A new field that adds N bytes to the production payload AND keeps it under 220 is admissible without an ADR. Any addition that breaches 220 (production) OR 200 (dev) triggers the encoding-compaction ADR per the plan's `<deferred_ideas>` section (single-char codes, bitmask, or hash-of-sorted-set fetch).

## Chosen Call-Site Pattern in `run.rs`

**Initial publish (lines 327-367):** Per-call derivation. The initial-publish block is non-async (the spawn `tokio::spawn(async move { ... })` only wraps the `publish_record` call); `supported_strs` / `supported_refs` / `output_st` are computed once in the outer block.

**Heartbeat publish (lines 369-425):** HOISTED out of the per-tick loop into the `tokio::spawn(async move { ... })` task's outer-but-inner-scope (before the `let mut ticker = ...;` line). Rationale:

- `BipConfig` is static — `cfg.bip.supported()` and `cfg.bip.output_script_type` do not change at runtime; recomputing them every heartbeat would waste 3 String allocations every 5 minutes for zero behavioural benefit.
- Owned `Vec<String>` + owned `String` are moved INTO the `async move` closure; the `supported_refs` borrowed slice is rebuilt at the top of the async task body (cheap — a single `iter().map(.as_str()).collect()`).
- W2 invariant preserved: `status` continues to be derived dynamically from `round_clone` inside the loop on each tick — only the BIP-322 fields are hoisted.

## Decisions Made

- **B3 compact-name migration in the same atomic commit as the schema bump.** Pre-B3 verbose-name worst case projected at ~221 bytes, breaching the 220-byte warn. Splitting the rename into a separate commit would have made the first commit unbuildable (warn fires at runtime) and the test suite incomplete. CD-10 + D-53 atomic-commit discipline demanded both land together.
- **Preserve `type` and `onion` field names verbatim.** `type` rename to `t` saves only 3 bytes and reduces schema identifiability for any future PKARR consumer; `onion` rename breaks the v1.3 client `Partial { onion: Option<String> }` resolver (`client/src/discover.rs:75-80`) and is therefore load-bearing per RESEARCH §V1.4-MOD-02.
- **Heartbeat hoist over per-tick recompute.** Documented in the inline comment block; matches the per-field clone-out style used immediately above the spawn for `denom` / `min_p`. Capture surface remains minimal — no `Arc<CoordinatorConfig>` clone.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] PLAN production-onion fixture was 60 bytes; padded to 62 bytes (Tor v3 actual)**

- **Found during:** Task 1 (`coordinator_packet_under_220_byte_budget_production_onion` test run)
- **Issue:** The PLAN literal at `coordinator/src/discovery/pkarr_pub.rs:266` (verbatim from `<behavior>`) had 54 `x` characters + `.onion` = 60 bytes. A real Tor v3 onion address is **56** base32 characters + `.onion` = **62 bytes** (per the PROJECT-constraint and the inline doc-comment in pkarr_pub.rs which states "v3 = 62-byte onion"). The 60-byte fixture under-approximates the production worst case by 2 bytes — a future field addition that pushed the payload from 209 to 220 bytes could still pass this guard while failing in production with a real .onion.
- **Fix:** Padded the fixture to exactly 56 `x` characters so `onion.len() == 62`. Added a comment block explaining the fix.
- **Files modified:** `coordinator/src/discovery/pkarr_pub.rs` (lines 261-267, test body)
- **Verification:** `assert_eq!(onion.len(), 62, ...)` passes; the production payload measures at 209 bytes (vs ~207 with the wrong 60-byte fixture).
- **Committed in:** `d1a1912` (Task 1 commit)

**2. [Rule 3 — Blocker] Multi-line `cfg.bip.supported()` call hidden from plan's grep gate**

- **Found during:** Task 2 self-check (`grep -cE 'cfg\.bip\.supported\(\)' coordinator/src/run.rs` returned 0 instead of >= 1)
- **Issue:** Idiomatic Rust formatting placed `cfg`, `.bip`, `.supported()` on three separate lines so `grep -E 'cfg\.bip\.supported\(\)'` matched zero lines. The plan's done-block grep gate (`>= 1`) failed even though the code was semantically correct.
- **Fix:** Collapsed the chain prefix `cfg.bip.supported()` onto a single line at both occurrences (initial publish and heartbeat publish) so the literal string `cfg.bip.supported()` appears in `wc -l`-greppable form. Single-line method-chain head followed by formatted continuations is still idiomatic Rust.
- **Files modified:** `coordinator/src/run.rs` (lines ~342 and ~389)
- **Verification:** `grep -cE 'cfg\.bip\.supported\(\)' coordinator/src/run.rs` now returns 2.
- **Committed in:** `146e7c3` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 Rule 1 bug, 1 Rule 3 blocker)
**Impact on plan:** Both auto-fixes preserve the plan's intent — the .onion fixture fix strengthens the regression gate to its true production worst case; the formatting fix makes the plan's literal grep gate green without altering semantics. No scope creep.

## Issues Encountered

- **Strict workspace clippy gate fails on pre-existing shared/src/bip322/* lints.** `cargo clippy --workspace --all-targets -- -D warnings` exits non-zero on the 14 pre-existing lints documented in `deferred-items.md` (12x `clippy::result_large_err` + 2x `clippy::unnecessary_to_owned`). These persist at HEAD before this plan; this plan modifies ZERO shared/ files. Per SCOPE BOUNDARY rule, deferred. Re-confirmed in deferred-items.md.

## Phase 16-02 CRIT-01 Invariant (cumulative check)

`grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` returns **2** at the 16-03 commit boundary. This plan does not touch utxo.rs; the 16-02 CI grep gate (`.github/workflows/ci.yml::crit-01-grep-check`) still passes.

## v1.3 Cross-Phase Invariant

`cargo test --test integration full_round` exits 0 with 8/8 tests green at the 16-03 commit boundary:

```
test full_round::coordinator_info_endpoint_fields ... ok
test full_round::adversarial_tampered_psbt_rejected ... ok
test full_round::adversarial_replay_token ... ok
test full_round::adversarial_wrong_denomination ... ok
test full_round::full_round_three_clients ... ok
test full_round::adversarial_invalid_utxo ... ok
test full_round::blame_non_signer_timeout ... ok
test full_round::round_restart_and_completion_after_blame ... ok
```

## Deferred Ideas Note (v1.5 watch-list)

Per the plan's `<deferred_ideas>` section: when a 4th script type lands (e.g., bare-P2PK, P2SH-multisig) the `sst` CSV alone will breach the 220-byte budget. Re-evaluate encoding at that point:

- **Option A** — single-character script-type codes (e.g., `w`/`t`/`s`/+new).
- **Option B** — bitmask integer (3 bits today, 4+ tomorrow).
- **Option C** — drop the CSV and ship a hash of the sorted set; clients fetch the full set out-of-band on demand.

Current 220-byte budget at production worst case has **~11 bytes of headroom** (enough for ~2 more average-length kebab-case CSV components or 3-4 more compact-name fields).

## User Setup Required

None — no external service configuration required for this plan.

## Next Phase Readiness

- **Phase 16 COMPLETE.** All three plans landed:
  - 16-01: BipConfig + InfoResponse wire-form extension (closed 2026-05-30).
  - 16-02: validate_utxo multi-script dispatcher + CRIT-01 dispatcher arm + 9 D-54 tests + CI grep gate (closed 2026-05-30).
  - 16-03 (this plan): PKARR record v0.2.0 + B3 compact-name rename + sst/ost + two byte-budget regression gates (closed 2026-05-30).
- **Phase 17 WALLET-01..04 ready to plan.** Phase 17 client resolver inherits the B3 compact wire shape and reads `v`, `sst`, `ost` directly (the v1.3 `Partial { onion }` resolver remains compatible — only the `onion` field is load-bearing for it). Phase 17 WALLET-02 implements the bdk path for P2TR sign per ADR §`#decision-4`; WALLET-03 reads the PKARR `sst`/`ost` to fail-fast at discovery; WALLET-04 wires the multi-script wallet onto the new client surface.
- **ADVERT-02 fully closed.** Both InfoResponse (16-01) and PKARR (this plan) halves shipped.

## Self-Check: PASSED

- [x] `coordinator/src/discovery/pkarr_pub.rs` exists and contains the new signature + JSON literal.
- [x] `coordinator/src/run.rs` exists with cfg.bip-derived args at both call sites + W3 stubs removed.
- [x] Commit `d1a1912` (Task 1) present in `git log --oneline`.
- [x] Commit `146e7c3` (Task 2) present in `git log --oneline`.
- [x] `cargo test -p coordinator --lib discovery::pkarr_pub` exits 0 with 10/10 tests passing.
- [x] `cargo test --test integration full_round` exits 0 with 8/8 tests passing.
- [x] Both budget-test names appear verbatim in `pkarr_pub.rs`.
- [x] `grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` returns 2 (cumulative Phase 16 invariant).

---
*Phase: 16-coordinator-integration-advertisement*
*Completed: 2026-05-30*
