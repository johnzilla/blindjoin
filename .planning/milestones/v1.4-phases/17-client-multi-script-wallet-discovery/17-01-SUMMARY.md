---
phase: 17-client-multi-script-wallet-discovery
plan: 01
subsystem: wallet
tags: [bitcoin, wallet, bdk_wallet, bip322, bip84, bip86, bip49, cli, descriptor, clap, serde]

# Dependency graph
requires:
  - phase: 15-shared-crate-multi-script-contract
    provides: shared::bip322::ScriptType enum with snake_case + kebab-case serde wire form (the LOCKED type imported as the --type CLI flag value + the BdkClientWallet::script_type field)
  - phase: 16-coordinator-integration-advertisement
    provides: PKARR record schema (v=0.2.0 + sst + ost) and InfoResponse extension consumed by 17-03 discovery layer (not used in 17-01 itself; this plan is wave 1)
provides:
  - "`--type {p2wpkh|p2tr|p2sh-p2wpkh}` CLI flag on ClientConfig + BLINDJOIN_SCRIPT_TYPE env var + parse_script_type value_parser"
  - "`script_type: ScriptType` field on BdkClientWallet (set at construction; single source of truth for downstream consumers)"
  - "`pub fn script_type(&self) -> ScriptType` accessor on BdkClientWallet"
  - "Per-type literal descriptor templates in BdkClientWallet::generate (BIP-84 / BIP-86 / BIP-49, coin=0' across all networks per D-66)"
  - "Construction-time --type vs descriptor wrapper mismatch check in BdkClientWallet::from_descriptor (D-63 fail-fast naming BOTH values)"
  - "Per-type FUND-THIS-ADDRESS banner line `Script type: {kebab} (BIP-{n})` (D-60)"
  - "from_wif P2WPKH-only invariant preserved per D-61 (no script_type param on the signature; cross-phase invariant safe)"
affects:
  - 17-02 (sign dispatcher reads wallet.script_type() to route P2WPKH vs P2TR vs P2SH-P2WPKH; CRIT-01 wire source for v=2 OwnershipProof)
  - 17-03 (discovery layer takes the wallet's script_type as `required_script_type` arg; main.rs threads cfg.script_type into discover_coordinator)
  - 18 (mixed-script E2E test reuses BdkClientWallet::generate with each ScriptType)

# Tech tracking
tech-stack:
  added: []  # No new external dependencies — shared::bip322::ScriptType + bdk_wallet 2.3 already in tree
  patterns:
    - "clap value_parser routing through serde wire form: parse_script_type wraps input in JSON quotes + calls serde_json::from_str::<ScriptType> so the ScriptType serde rename is the single source of truth for accepted CLI tokens"
    - "Literal descriptor templates with coin=0' across all networks (D-66 anti-pattern guard: bdk_wallet::template::Bip84/86/49 helpers explicitly NOT used because they auto-select coin=1' on testnet/signet, breaking v1.3 byte-equivalence — documented inline)"
    - "Construction-time string-prefix script-type detection with sh(wpkh( ordered BEFORE wpkh( (longest-match-first to avoid sh(wpkh matching wpkh)"
    - "Test-only #[cfg(test)] external_desc_str() accessor + struct field so unit tests deterministically assert descriptor prefix shape without driving bdk_wallet's internal descriptor formatter"
    - "Forward-only #[allow(dead_code)] on script_type field + accessor with explicit consumer comment (`consumed by 17-02 sign dispatcher + 17-03 discovery check`) — bridges the wave-1→wave-2 cycle without warning suppression in committed code beyond this plan"

key-files:
  created: []
  modified:
    - client/src/config.rs (parse_script_type helper + --type/BLINDJOIN_SCRIPT_TYPE flag on ClientConfig + 6 unit tests)
    - client/src/wallet.rs (use shared::bip322::ScriptType; script_type field + Test-only external_desc_str field + from_wif hardcodes P2WPKH per D-61 + from_descriptor extended with script_type param + D-63 mismatch check + generate extended with script_type param + per-type templates per D-58 + D-60 banner update + script_type() accessor + #[cfg(test)] mod tests with 6 unit tests)
    - client/src/main.rs (generate call site forwards cfg.script_type; from_descriptor call site forwards cfg.script_type; from_wif call site UNCHANGED per D-61)

key-decisions:
  - "D-57 + CD-22: single-underscore env var `BLINDJOIN_SCRIPT_TYPE` (matches existing client convention; the coordinator-side double-underscore `BLINDJOIN__COORDINATOR__*` is config-crate-driven and not in use here)"
  - "D-58 + D-66: LITERAL format!() descriptor templates with coin=0' across all networks; bdk_wallet::template::Bip84/86/49 helpers explicitly forbidden via inline doc comment (RESEARCH Pitfall 2, load-bearing for v1.3 byte-equivalence and cross-phase invariant)"
  - "D-61: from_wif takes NO script_type parameter; hardcodes ScriptType::P2wpkh on the returned wallet so the v1.3 cross-phase invariant (tests/integration/full_round.rs) stays bit-exact unchanged"
  - "D-62: script_type stored as a field on BdkClientWallet (set ONCE at construction) rather than re-detected via shared::bip322::detect_script_type at use-time; the wallet KNOWS its descriptor type explicitly so the field is the single source of truth for downstream consumers"
  - "D-63: construction-time outer-wrapper mismatch check by string-prefix match (sh(wpkh( first, then wpkh(, then tr(); error message names BOTH the declared --type and the detected wrapper for user-actionable diagnostics"
  - "D-60: per-type banner line `Script type: {kebab} (BIP-{n})` inserted between the FUND-THIS-ADDRESS line and the derivation-path note; the existing path-note line is now dynamically interpolated with the BIP number"
  - "CD-17: lowercase kebab-case-only --type values (P2TR is rejected) — serde wire form is the single source of truth; no separate case-folding step"
  - "Plan-instruction interpretation re Test 5: use `match` on the Result rather than `expect_err()` because BdkClientWallet does NOT derive Debug; the assertion still verifies the error names BOTH 'p2tr' AND 'wpkh' per the D-63 contract"

patterns-established:
  - "Pattern A — serde-driven clap value_parser: every CLI flag that maps to a serde-renamed enum routes through `serde_json::from_str` so the enum's wire form is the single source of truth. Reusable for future flags like --network if the bitcoin::Network parser ever drifts from clap's default"
  - "Pattern B — wallet-as-source-of-truth: BdkClientWallet stores its descriptor's script_type at construction. Downstream code (round/input.rs, discover.rs) reads from wallet.script_type() rather than re-detecting from the script_pubkey. Mirrors the coordinator-side CRIT-01 discipline at the inverse boundary"
  - "Pattern C — D-66 literal templates with coin=0': any new BIP descriptor template added in v1.5+ MUST use the literal format!() shape with coin=0', NOT a bdk_wallet template helper, to preserve byte-exact carry-forward semantics"
  - "Pattern D — #[cfg(test)] external_desc_str() mirror: test-only accessor + field for asserting descriptor strings deterministically without driving the internal bdk formatter. Reusable for any future wallet that needs descriptor-shape regression tests"

requirements-completed: [WALLET-01]

# Metrics
duration: ~10 min
completed: 2026-05-30
---

# Phase 17 Plan 01: Client Multi-Script Wallet & Discovery (WALLET-01) Summary

**Per-type BIP-84 / BIP-86 / BIP-49 descriptor generation in BdkClientWallet driven by a new `--type` CLI flag, with construction-time wrapper-vs-flag mismatch fail-fast and a wallet-stored ScriptType field as the single source of truth for downstream consumers.**

## Performance

- **Duration:** ~10 min (Started: 2026-05-30T14:04:13Z; Completed: 2026-05-30T14:13:57Z)
- **Tasks:** 3
- **Files modified:** 3 (client/src/config.rs, client/src/wallet.rs, client/src/main.rs)
- **Unit tests added:** 12 (6 config::tests::* + 6 wallet::tests::*)

## Accomplishments

- `--type {p2wpkh|p2tr|p2sh-p2wpkh}` CLI flag + `BLINDJOIN_SCRIPT_TYPE` env var added to ClientConfig with default `p2wpkh` for v1.3 backwards compatibility. The `parse_script_type` value_parser routes through `serde_json::from_str::<shared::bip322::ScriptType>` so the LOCKED Phase 15 serde wire form is the single source of truth for accepted tokens.
- BdkClientWallet now stores `script_type: ScriptType` as a struct field set at construction (D-62). The `script_type()` accessor exposes it to downstream consumers (17-02 sign dispatcher + 17-03 discovery check + the v=2 OwnershipProof CRIT-01 wire source).
- `generate(...)` now takes a `script_type` parameter and emits the matching literal descriptor template:
  - P2WPKH (BIP-84): `wpkh({xprv}/84'/0'/0'/0/*)` — UNCHANGED from v1.3, byte-equivalent.
  - P2TR (BIP-86): `tr({xprv}/86'/0'/0'/0/*)`.
  - P2SH-P2WPKH (BIP-49): `sh(wpkh({xprv}/49'/0'/0'/0/*))`.
  All branches use coin=0' across all networks per D-66 (bdk_wallet template helpers deliberately NOT used per RESEARCH Pitfall 2; documented inline).
- `from_descriptor(...)` now takes a `script_type` parameter and performs a D-63 construction-time outer-wrapper mismatch check: detects the wrapper by string prefix (sh(wpkh( ordered first, then wpkh(, then tr() and returns an error naming BOTH the declared --type and the detected wrapper. Catches "--type p2tr --descriptor wpkh(...)" at the user-facing seam instead of as an opaque coordinator-side `ScriptTypeMismatch` later.
- `from_wif(...)` deliberately UNCHANGED per D-61 — no script_type parameter; the wallet's script_type is hardcoded to `ScriptType::P2wpkh`. This preserves the v1.3 cross-phase invariant (`tests/integration/full_round.rs` uses the WIF path).
- FUND-THIS-ADDRESS banner gets a new line per D-60: `Script type: {kebab} (BIP-{n})`. The existing derivation-path note is now interpolated dynamically: `(BIP-{bip} path: m/{bip}'/0'/0'/0/0)`.
- main.rs threads `cfg.script_type` into `ClientWallet::generate` and `ClientWallet::from_descriptor`; the `from_wif` call site is intentionally untouched. The PKARR call site is also untouched — that is 17-03's scope.

## Verification Evidence

### config::tests (6/6 GREEN)

```
test config::tests::parse_script_type_accepts_p2wpkh ... ok
test config::tests::parse_script_type_accepts_p2tr ... ok
test config::tests::parse_script_type_accepts_p2sh_p2wpkh ... ok
test config::tests::parse_script_type_rejects_uppercase ... ok
test config::tests::parse_script_type_rejects_unknown ... ok
test config::tests::client_config_defaults_to_p2wpkh ... ok
```

### wallet::tests (6/6 GREEN)

```
test wallet::tests::generate_p2wpkh_produces_bip84_descriptor ... ok
test wallet::tests::generate_p2tr_produces_bip86_descriptor ... ok
test wallet::tests::generate_p2sh_p2wpkh_produces_bip49_descriptor ... ok
test wallet::tests::script_type_accessor_matches_construction ... ok
test wallet::tests::from_descriptor_rejects_p2tr_flag_with_wpkh_descriptor ... ok
test wallet::tests::from_wif_asserts_p2wpkh ... ok
```

### Cross-phase invariant — `cargo test --test integration full_round` (8/8 GREEN)

```
test full_round::adversarial_tampered_psbt_rejected ... ok
test full_round::coordinator_info_endpoint_fields ... ok
test full_round::adversarial_wrong_denomination ... ok
test full_round::adversarial_replay_token ... ok
test full_round::adversarial_invalid_utxo ... ok
test full_round::full_round_three_clients ... ok
test full_round::blame_non_signer_timeout ... ok
test full_round::round_restart_and_completion_after_blame ... ok
```

(First run saw `full_round_three_clients` flake with HTTP 400 then cascading 429s — second run was 8/8 GREEN. This is the v1.3 carry-forward flake from REPAIR-01 forensics, unrelated to this plan's changes which only touch wallet construction signatures, not the actual signing path.)

### End-to-end CLI smoke tests — actual printed descriptors

The full `client --generate-wallet --type {each}` smoke chain confirms config → main → wallet → generate works end-to-end:

```text
# DEFAULT (no --type)
External descriptor (receiving addresses):
  wpkh(tprv8ZgxMBicQKsPdPZxv6U7CMNt4STjL3N2rW13ZacAakfdCFD5Jtup7KCng39xT1AZP27dBAjTzxyypLTdZpcwRB2S1PZvNLeT5gdveQdV7bF/84'/0'/0'/0/*)

# --type p2tr
External descriptor (receiving addresses):
  tr(tprv8ZgxMBicQKsPeqfYh9JcFLBQCeU7sB1yWXoYt8Q7uE7Ck2BvgXfofyfeeRgzbFQh4zhb18nvuhq9gBnHA6Kvu6Nd6BXWdVJeMX35NJH1nZX/86'/0'/0'/0/*)

# --type p2sh-p2wpkh
External descriptor (receiving addresses):
  sh(wpkh(tprv8ZgxMBicQKsPeZ2bBQpESmfmzV6jhSrxkftPbzwxG2xpNEU5Ae8swqbvzNVDp7ZGJohMjLJGnFGNvCMx5Lz3wcQ67VggiVpjRxB9YVvV5CP/49'/0'/0'/0/*))

# D-60 banner output (--type p2tr)
  Script type: p2tr (BIP-86)
  (BIP-86 path: m/86'/0'/0'/0/0)
```

All three variants produce the correct BIP-84/86/49 descriptor shape; coin=0' literal preserved across networks per D-66; banner lines emitted per D-60. Default (no --type) path is BYTE-EQUIVALENT to v1.3.

### Workspace build

`cargo build --workspace` — clean, zero warnings.

## Task Commits

Each task was committed atomically:

1. **Task 1: --type CLI flag + BLINDJOIN_SCRIPT_TYPE env + parse_script_type helper** — `f2af5e8` (feat)
2. **Task 2: script_type field + per-type templates + mismatch check + accessor on BdkClientWallet** — `4c36d50` (feat)
3. **Task 3: Wire cfg.script_type into wallet constructors at main.rs call sites** — `43b9b41` (feat)

## Files Created/Modified

- `client/src/config.rs` — Added `parse_script_type` value_parser helper (routes through serde wire form), added `pub script_type: shared::bip322::ScriptType` field on ClientConfig with `--type` flag + `BLINDJOIN_SCRIPT_TYPE` env + default `p2wpkh`, added 6 unit tests covering the 3 LOCKED tokens + uppercase rejection + unknown-token rejection + default value.
- `client/src/wallet.rs` — Added `use shared::bip322::ScriptType;` import, added `script_type: ScriptType` field on BdkClientWallet (with `#[allow(dead_code)]` until 17-02/17-03 consume), added Test-only `external_desc_str` field + `external_desc_str()` accessor, extended `from_descriptor` signature with `script_type: ScriptType` param + D-63 mismatch check, extended `generate` signature with `script_type: ScriptType` param + per-type literal templates per D-58 + D-60 banner update, hardcoded `ScriptType::P2wpkh` in `from_wif` per D-61, added `pub fn script_type(&self) -> ScriptType` accessor, added 6 wallet::tests::* unit tests.
- `client/src/main.rs` — Forwarded `cfg.script_type` into `ClientWallet::generate(utxo, network, cfg.script_type)` and `ClientWallet::from_descriptor(descriptor, utxo, utxo_address, network, cfg.script_type)`; left `ClientWallet::from_wif(wif, utxo, network)` UNCHANGED per D-61; left the PKARR discover_coordinator call site UNCHANGED (17-03 scope).

## Decisions Made

All major decisions were pre-locked in 17-CONTEXT.md (D-57..D-66, CD-17, CD-22) and applied verbatim. The only execution-time micro-decisions:

- **Use a `match` block instead of `.expect_err()` in the mismatch test:** BdkClientWallet does not derive Debug, so `Result<Self, _>::expect_err` does not compile. Switched to an explicit `match` that still asserts the error names BOTH 'p2tr' AND 'wpkh'. Functionally identical; preserves the D-63 contract verification.
- **`#[allow(dead_code)]` on the new `script_type` field + accessor:** Forward-only — they are consumed by 17-02's sign dispatcher and 17-03's discovery check. Annotated with explicit consumer comment (`consumed by 17-02 sign dispatcher + 17-03 discovery check`) so the suppression is unambiguous + auditable. Removed in 17-02 (the sign dispatcher reads `wallet.script_type()`).
- **`sh(wpkh(` ordered BEFORE `wpkh(` in the D-63 wrapper detection:** Longest-match-first. `sh(wpkh(` is a strict superset of `wpkh(` after the leading `sh(` (which it isn't, actually — but the prefix `wpkh(` would match the inside of `sh(wpkh(` if we ever stripped the `sh(` first). Using longest-prefix ordering is defensive and obviously correct.

## Deviations from Plan

None. Plan executed exactly as written per locked D-57..D-66, CD-17, CD-22.

The plan body explicitly enumerated each EDIT (A through E) and each test name; all were applied verbatim. Two micro-execution decisions documented in "Decisions Made" above are within plan author intent (Test 5's `match` block is the only way to assert without Debug; the `#[allow(dead_code)]` is a forward-compat bridge documented in the plan's `<read_first>` for 17-02 to consume).

## Issues Encountered

- **`Result::expect_err` requires `T: Debug`** — BdkClientWallet does not derive Debug. Resolved by switching the mismatch test to a `match` block. (Non-deviation; the assertion still verifies D-63's "BOTH names in the error" contract.)
- **Dead-code warnings on `script_type` field + accessor after Task 2 lib-build** — Annotated with `#[allow(dead_code)]` and the explicit consumer comment in Task 3. Will be removed when 17-02 lands the sign dispatcher.
- **`full_round_three_clients` flake on first invariant run** — HTTP 400 then cascading 429s. Pre-existing carry-forward from REPAIR-01 forensics, unrelated to this plan (this plan does not touch the sign path or the rate-limited /info endpoint). Second run was 8/8 GREEN.

## Next Phase Readiness

17-02 (WALLET-02 + WALLET-04 encoder side) is unblocked:
- `wallet.script_type()` accessor is the LOCKED source of truth for the per-script sign dispatcher.
- `BdkClientWallet { script_type, ... }` is set at construction for ALL three paths (from_wif → P2WPKH; from_descriptor → declared with mismatch fail-fast; generate → declared per --type).
- The `cfg.script_type` plumbing in main.rs is symmetric with what 17-03 will consume for the discovery call site.

17-03 (WALLET-03 + WALLET-04 discovery side) inherits the same `cfg.script_type` source-of-truth path; the field on ClientConfig is in place.

Cross-phase invariant (`tests/integration/full_round.rs`) GREEN at the plan boundary — D-61's P2WPKH-only from_wif discipline preserves the v1.3 sign path bit-exact.

## Self-Check

Verifying claims before proceeding.

**Files exist:**
- `client/src/config.rs` — FOUND (75 LOC added)
- `client/src/wallet.rs` — FOUND (237 LOC added/modified)
- `client/src/main.rs` — FOUND (9 LOC modified)

**Commits exist:**
- `f2af5e8` (Task 1) — FOUND on main
- `4c36d50` (Task 2) — FOUND on main
- `43b9b41` (Task 3) — FOUND on main

**Verification commands GREEN:**
- `cargo build --workspace` — PASS (zero warnings)
- `cargo test -p client --lib` — PASS (17/17 including 12 new unit tests)
- `cargo test -p client --lib config::tests` — PASS (6/6)
- `cargo test -p client --lib wallet::tests` — PASS (6/6)
- `cargo test --test integration full_round` — PASS (8/8 on retry; first run hit unrelated v1.3 carry-forward flake)
- CLI smoke tests — PASS (all 3 --type variants produce correct BIP-84/86/49 descriptor shape; banner emits per D-60)

## Self-Check: PASSED

---
*Phase: 17-client-multi-script-wallet-discovery*
*Completed: 2026-05-30*
