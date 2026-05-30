---
phase: 17-client-multi-script-wallet-discovery
plan: 03
subsystem: discovery
tags: [bitcoin, pkarr, discovery, fail-fast, tor, compat-shim, wallet-03, wallet-04, crit-01, thiserror, pitfall-5]

# Dependency graph
requires:
  - phase: 15-shared-crate-multi-script-contract
    provides: "shared::bip322::ScriptType with snake_case + kebab-case serde rename (consumed by DiscoveryError variants and BlindjoinRecord CSV decode)"
  - phase: 16-coordinator-integration-advertisement
    provides: "PKARR record wire shape (`v`/`sst`/`ost` compact field names via Phase 16-03 commit d1a1912) — the schema this plan's BlindjoinRecord decoder is the byte-inverse of"
  - phase: 17-01
    provides: "BdkClientWallet.script_type() accessor — the source of `required_script_type` arg passed into discover_coordinator in main.rs"
  - phase: 17-02
    provides: "register_input with transitional `is_legacy_coordinator: bool` 4th parameter (this plan REPLACES with &CoordinatorInfo) + build_v2_psbt_input_b64 encoder template + OwnershipProof CD-7 byte-identity branch"
provides:
  - "client::discover::CoordinatorCapabilities struct (4 fields: record_version, is_legacy, supported_script_types, output_script_type) — public per CD-21 so main.rs reads info.capabilities.is_legacy directly for the WARN log"
  - "client::discover::DiscoveryError typed enum (6 variants: InvalidPubkey, NotFound, MissingOnion, MalformedRecord, UnsupportedScriptType, UnsupportedOutputScriptType) — PII-free per PROJECT.md; symmetric with coordinator::bitcoin::utxo::UtxoError thiserror discipline"
  - "client::discover::BlindjoinRecord decoder with #[serde(rename = \"v\", default = \"default_legacy_version\")] — Pitfall 5 LOAD-BEARING correction against Phase 16-03 compact-field rename"
  - "client::discover::capabilities_from_record_v public (#[doc(hidden)]) helper — Rule-3 visibility escalation for cross-crate test access, mirrors Phase 16-02 escalation on validate_ownership_proof_typed"
  - "client::discover::discover_coordinator(pkarr_pubkey, required_script_type) -> Result<CoordinatorInfo, DiscoveryError> — WALLET-03 fail-fast at the resolver boundary BEFORE any Tor circuit opens (structural pre-Tor ordering per D-74 + RESEARCH Pitfall 4)"
  - "client::round::input::register_input(.., &CoordinatorInfo) — transitional bool 4th param REPLACED with the real CoordinatorInfo; v1/v2 envelope branch reads coordinator_info.capabilities.is_legacy"
  - "main.rs CD-21 WARN log on legacy-coordinator detection + inline WALLET-03 ordering proof comment + synthetic CoordinatorInfo on the non-PKARR --coordinator-url direct path"
  - "tests/integration/multi_script_client.rs — Phase 17 acceptance gate with 9 D-78 named tests (Tests 1-6 #[ignore]'d cross-references to 17-01/17-02 equivalents; Tests 7-9 LIVE — WALLET-03 reject + WALLET-04 v=1 happy path + v1.4 v=2 positive control)"
affects:
  - 18 (mixed-script E2E round consumes the fully-wired discovery + envelope path: PKARR record decode → CoordinatorInfo with capabilities → register_input branches on is_legacy → v1/v2 envelope; INTEG-01 will use a real v1.3 binary per D-79 deferral)
  - all v1.4 clients connecting via PKARR (production wire path: discover_coordinator validates script-type allowlist + output type pre-Tor, then routes to the v1/v2 encoder)
  - operator-side troubleshooting (DiscoveryError typed Display impls name pubkey + ScriptType enum values + structural reasons — PII-free, mirrors coordinator UtxoError discipline)

# Tech tracking
tech-stack:
  added:
    - "thiserror = { workspace = true } on client/Cargo.toml (workspace pin already present at root Cargo.toml line 17; this plan added the direct dep declaration so client::discover::DiscoveryError can derive thiserror::Error)"
  patterns:
    - "Pitfall 5 load-bearing rename: `#[serde(rename = \"v\", default = \"default_legacy_version\")]` on BlindjoinRecord.version. Without the rename every v1.4 coordinator would silently appear legacy on every connection — breaking WALLET-04 in the wrong direction. Phase 16-03 commit d1a1912 compactified the PKARR wire field name; this plan's decoder is the byte-inverse"
    - "Structural pre-Tor ordering: discover_coordinator runs UNCONDITIONALLY at main.rs ~line 60 BEFORE the `if cfg.use_tor` branch. The pre-Tor fail-fast is a structural (file-position) invariant, NOT a runtime ordering hack. Documented inline at the call site per RESEARCH Pitfall 4 so a future refactor cannot move the discover call inside the Tor branch without breaking the comment-invariant"
    - "Synthetic CoordinatorInfo for non-PKARR paths: when the operator passes --coordinator-url directly (bypassing discovery), construct a synthetic CoordinatorInfo defaulting `is_legacy: false` + all 3 script types supported + output_script_type matching the wallet. The graceful UX downgrade is: v1.4 client points at v1.3 coordinator → emits v=2 envelope → v1.3 coordinator rejects with a clear error message. Documented inline as T-17-03-05 (accept disposition)"
    - "Visibility escalation for cross-crate test reach: `capabilities_from_record_v` is `pub` with `#[doc(hidden)]` so tests/integration/multi_script_client.rs can exercise the legacy/v0.2.0 capability derivation branches without a live DHT roundtrip. Mirrors Phase 16-02's escalation on `validate_ownership_proof_typed` and Phase 15-03's on `sign_simple_test_only`"
    - "Encoder-inlining for test reach: rather than escalate `client::round::input::build_v2_psbt_input_b64` to `pub(crate)` for the new integration test, the 17-LOC helper was re-implemented verbatim inline in multi_script_client.rs. Production encoder stays module-private; test file mirrors the same byte-shape contract. Verbatim shape carried through 3 sites (production, 17-02 inline test, 17-03 new integration test) — all converge on the same Phase 16-02 canonical reference at tests/integration/multi_script_validate.rs:56-74"

key-files:
  created:
    - tests/integration/multi_script_client.rs (374 LOC; 9 D-78 named tests — Tests 1-6 ignored stubs with cross-references; Tests 7-9 live WALLET-03/04 + v1.4 envelope assertions)
  modified:
    - client/src/discover.rs (REPLACED — 102 LOC → 405 LOC: CoordinatorCapabilities + DiscoveryError + BlindjoinRecord decoder with Pitfall 5 rename + capabilities_from_record_v + extended discover_coordinator signature + 8 unit tests)
    - client/src/round/input.rs (transitional `is_legacy_coordinator: bool` 4th param REPLACED with `&CoordinatorInfo`; v1/v2 envelope branch reads coordinator_info.capabilities.is_legacy; CRIT-01 inline comment preserved at count=2)
    - client/src/main.rs (PKARR call site passes wallet.script_type() into discover_coordinator; CD-21 WARN log on legacy detection; WALLET-03 inline ordering proof comment; synthetic CoordinatorInfo on non-PKARR path; register_input call site updated to pass &coordinator_info)
    - client/Cargo.toml (thiserror = { workspace = true } added as direct dep)
    - Cargo.lock (regenerated for thiserror direct-dep declaration; net new transitives: 0 — already pulled by shared and coordinator)
    - liquidity-bot/src/main.rs (register_input call site updated to pass synthetic CoordinatorInfo; auto-fix Rule 3 — Blocker)
    - tests/integration/full_round.rs (6 register_input call sites updated via replace_all to use new v14_p2wpkh_coordinator_info() helper; helper added at module level)
    - tests/integration/mod.rs (added `mod multi_script_client;` per cargo-convention wiring)

key-decisions:
  - "D-71: CoordinatorCapabilities struct shape — 4 public fields (record_version, is_legacy, supported_script_types, output_script_type). Public per CD-21 so main.rs reads info.capabilities.is_legacy directly for the WARN log; encapsulation buys nothing"
  - "D-72: DiscoveryError 6-variant taxonomy via thiserror — InvalidPubkey, NotFound, MissingOnion, MalformedRecord, UnsupportedScriptType (carries the ROADMAP SC#3 literal 'does not support' wording), UnsupportedOutputScriptType. PII-free per PROJECT.md"
  - "D-73 OVERRIDDEN by Pitfall 5: BlindjoinRecord.version uses `#[serde(rename = \"v\", default = \"default_legacy_version\")]` per Phase 16-03 commit d1a1912's compactification. Documented load-bearing inline doc-comment cross-references coordinator/src/discovery/pkarr_pub.rs:89-108 as source of truth"
  - "D-74: pre-Tor fail-fast is STRUCTURAL (file-position invariant), not runtime. discover_coordinator runs UNCONDITIONALLY at main.rs before the `if cfg.use_tor` branch. Inline comment at the call site (`// WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural ordering, not a runtime hack.`) per RESEARCH Pitfall 4 prevents future refactor from silently breaking the ordering"
  - "D-75: NO double-check at /round/info for script types — PKARR sst is the canonical fail-fast signal; /round/info is informational only post-discovery"
  - "D-76 + CD-23: output_script_type mismatch ALSO fails at discovery, with split UnsupportedOutputScriptType variant for user-facing actionability (different fix path than input-mismatch)"
  - "D-78: 9 named tests in tests/integration/multi_script_client.rs. Plan-phase delegation: Tests 1-6 are #[ignore]'d stubs with cross-references to 17-01/17-02 LIVE equivalents (preserves the D-78 named-test contract + provides a single grep-target for Phase 17 acceptance audit); Tests 7-9 are LIVE WALLET-03/04 + v1.4 v=2 positive control"
  - "D-79: v1.3-binary integration test deferred to Phase 18 INTEG-01 — this plan covers the structural acceptance gate via stubbed PKARR records; INTEG-01 covers the real-binary acceptance gate"
  - "CD-21: CoordinatorCapabilities is a public struct; main.rs reads info.capabilities.is_legacy DIRECTLY and emits tracing::warn! with structured fields (coordinator_pubkey + record_version) — PII-free, both fields are public data"
  - "CD-23: split UnsupportedScriptType (input allowlist miss) vs UnsupportedOutputScriptType (output type mismatch) variants — different user-facing fix paths warrant separate error types"
  - "Synthetic CoordinatorInfo on --coordinator-url direct path (auto-fixed inline + tested via the full_round.rs helper): defaults is_legacy=false + all 3 script types + wallet.script_type() output. T-17-03-05 accept disposition; documented inline"

patterns-established:
  - "Pattern A — Pitfall 5 LOAD-BEARING rename discipline: when a wire-format produced by an upstream module compactifies a field name for byte-budget reasons, every downstream decoder MUST use `#[serde(rename = \"compact\", default = \"...\")]` to mirror the wire shape AND preserve backwards-compat with the verbose-named legacy form. Without the rename the decoder silently falls back to defaults, producing the OPPOSITE of the intended branch decision. Inline doc-comment cross-referencing the upstream commit + line range is the audit trail"
  - "Pattern B — structural pre-{network-side-effect} fail-fast: a fail-fast that must happen BEFORE a side-effecting operation (Tor circuit open, RPC call, disk write, network publish) MUST be enforced STRUCTURALLY (by code position in main.rs / call-graph topology), with an inline comment naming the invariant. NEVER rely on runtime ordering hacks. The comment is the proof; a future refactor that moves the fail-fast below the side effect breaks the comment-invariant"
  - "Pattern C — synthetic capability struct for non-discovery paths: when discovery is bypassed (operator direct config, --coordinator-url, env-var override), construct a synthetic capability/info struct defaulting to the most-recent supported wire shape (here: is_legacy=false, all 3 types). The graceful UX downgrade is: client emits modern shape → legacy coordinator rejects with clear error → user pivots to discovery or different coordinator. Documented inline as the threat-register accept disposition"
  - "Pattern D — encoder-inlining (vs visibility escalation) for cross-crate tests: when a small (<25 LOC) module-private helper needs to be exercised by an external integration test, prefer RE-IMPLEMENTATION verbatim in the test file over escalating the helper's visibility. Keeps the production module's surface minimal; the byte-shape contract is enforced by 3-site verbatim shape convergence (production + 17-02 inline test + 17-03 integration test) — all converging on the same canonical Phase 16-02 reference at tests/integration/multi_script_validate.rs:56-74"

requirements-completed: [WALLET-03, WALLET-04]

# Wave metadata
wave: 3
depends_on: [17-01, 17-02]
sequential_mode: true
worktree_mode: false

metrics:
  duration_minutes: ~11
  tasks: 3
  files_modified: 7  # discover.rs (replace) + round/input.rs + main.rs + Cargo.toml + Cargo.lock + liquidity-bot/main.rs + tests/integration/full_round.rs + tests/integration/mod.rs
  files_created: 1   # tests/integration/multi_script_client.rs
  completed: 2026-05-30
---

# Phase 17 Plan 17-03: WALLET-03 + WALLET-04 Discovery — Phase 17 Acceptance Gate

**Pre-Tor PKARR fail-fast resolver with typed `DiscoveryError`, `CoordinatorCapabilities` capability flags, and the Pitfall 5 LOAD-BEARING `#[serde(rename = "v")]` correction against Phase 16-03's compact-field rename — closing the WALLET-03 + WALLET-04 (discovery side) requirements and shipping the 9-test Phase 17 acceptance gate at `tests/integration/multi_script_client.rs`.**

This plan is the FINAL plan of Phase 17. With Tasks 1-3 landed, WALLET-01..04 are all closed; Phase 17 is COMPLETE.

## Tasks

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Extended PKARR resolver with CoordinatorCapabilities + DiscoveryError + Pitfall 5 rename + 8 unit tests | `1993436` | ✅ PASS |
| 2 | Wire discover→register_input with real CoordinatorInfo + WALLET-03 fail-fast + CD-21 WARN | `b677223` | ✅ PASS |
| 3 | Phase 17 acceptance gate — 9 D-78 multi-script client tests | `460aa4d` | ✅ PASS |

## Test Results

### `client::discover::tests` (Task 1) — 8/8 GREEN

| Test | Status | Notes |
|------|--------|-------|
| `parse_blindjoin_record_decodes_v0_2_0_record_with_sst_and_ost` | ✅ ok | Pitfall 5: compact `v` field name decoded correctly |
| `parse_blindjoin_record_decodes_legacy_v0_1_0_record_via_default` | ✅ ok | `default_legacy_version` fires when `v` absent |
| `parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field` | ✅ ok | Explicit regression: `v` wins over `version` per Pitfall 5 |
| `capabilities_is_legacy_true_for_v0_1_0` | ✅ ok | Legacy → P2WPKH-only defaults |
| `capabilities_is_legacy_false_for_v0_2_0_with_sst` | ✅ ok | v0.2.0 → parsed types preserved in declared order |
| `capabilities_returns_malformed_record_for_invalid_sst_token` | ✅ ok | Bad CSV token → MalformedRecord with reason |
| `discover_coordinator_rejects_invalid_pubkey` | ✅ ok | InvalidPubkey variant matches!() check |
| `unsupported_script_type_error_message_names_pubkey_and_required_and_supported` | ✅ ok | ROADMAP SC#3 "does not support" wording present |

### `client/src/round/input::tests` (Task 2) — 4/4 GREEN (unchanged from 17-02)

The 4 inline tests from 17-02 (`build_v2_psbt_input_b64_roundtrips_via_coordinator_decoder`, `build_v2_psbt_input_b64_with_final_script_sig_populates_field`, `register_input_with_legacy_coordinator_emits_v1_envelope`, `register_input_with_v14_coordinator_emits_v2_envelope`) remain GREEN after the parameter swap — they test envelope shapes at the JSON layer and don't depend on the transitional parameter.

### `tests/integration/multi_script_client.rs` (Task 3) — 9 tests, 3 ok + 6 ignored

```
test multi_script_client::generate_p2sh_p2wpkh_wallet_emits_bip49_descriptor ... ignored, covered by client/src/wallet::tests::generate_p2sh_p2wpkh_produces_bip49_descriptor
test multi_script_client::generate_p2tr_wallet_emits_bip86_descriptor ... ignored, covered by client/src/wallet::tests::generate_p2tr_produces_bip86_descriptor
test multi_script_client::generate_p2wpkh_wallet_emits_bip84_descriptor ... ignored, covered by client/src/wallet::tests::generate_p2wpkh_produces_bip84_descriptor
test multi_script_client::p2sh_p2wpkh_sign_roundtrip_verifies ... ignored, covered by client/tests/wallet_sign_roundtrip::p2sh_p2wpkh_descriptor_sign_roundtrip_verifies
test multi_script_client::p2tr_sign_roundtrip_verifies ... ignored, covered by client/tests/wallet_sign_roundtrip::p2tr_descriptor_sign_roundtrip_verifies
test multi_script_client::p2wpkh_sign_roundtrip_verifies ... ignored, covered by client/tests/wallet_sign_roundtrip::p2wpkh_descriptor_sign_roundtrip_verifies
test multi_script_client::v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope ... ok
test multi_script_client::v13_pkarr_record_with_p2tr_wallet_rejects_before_tor ... ok
test multi_script_client::v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope ... ok

test result: ok. 3 passed; 0 failed; 6 ignored; 0 measured
```

Tests 1-6 stubs preserve the D-78 named-test contract while delegating to the live equivalents at `client/src/wallet::tests::*` (17-01) and `client/tests/wallet_sign_roundtrip.rs` (17-02). Tests 7-9 are the load-bearing live gates.

### Cross-Phase Invariant — `cargo test --test integration full_round` 8/8 GREEN

```
test full_round::coordinator_info_endpoint_fields ... ok
test full_round::adversarial_tampered_psbt_rejected ... ok
test full_round::adversarial_invalid_utxo ... ok
test full_round::full_round_three_clients ... ok
test full_round::adversarial_wrong_denomination ... ok
test full_round::adversarial_replay_token ... ok
test full_round::blame_non_signer_timeout ... ok
test full_round::round_restart_and_completion_after_blame ... ok

test result: ok. 8 passed; 0 failed; finished in 42.12s
```

The v1.3 P2WPKH WIF path → bdk_wallet from_wif → ScriptType::P2wpkh → `shared::bip322::sign_simple(P2wpkh, ...)` → CD-7 byte-identity branch is bit-exact preserved. The `v14_p2wpkh_coordinator_info()` helper in `tests/integration/full_round.rs` constructs a synthetic CoordinatorInfo defaulting `is_legacy: false` (v=2 envelope) which routes through the same P2WPKH sighash math.

### Workspace Build — `cargo build --workspace` CLEAN

No new warnings beyond pre-existing carry-forward.

### `cargo audit` — CLEAN (exit 0, 718 deps)

No new vulnerabilities, no warnings. The `thiserror` direct-dep declaration in client/Cargo.toml pulls in no new transitives — the crate was already pulled by `shared` and `coordinator`.

## Verifiable Acceptance Criteria

### Source-Level Gates (all PASS via grep)

```
grep -q 'pub struct CoordinatorCapabilities' client/src/discover.rs         # PASS
grep -q 'pub enum DiscoveryError'           client/src/discover.rs         # PASS
grep -q 'UnsupportedScriptType'             client/src/discover.rs         # PASS (CD-23)
grep -q 'UnsupportedOutputScriptType'       client/src/discover.rs         # PASS (CD-23)
grep -q 'rename = "v"'                      client/src/discover.rs         # PASS (Pitfall 5)
grep -q 'fn default_legacy_version'         client/src/discover.rs         # PASS
grep -q 'fn parse_blindjoin_record'         client/src/discover.rs         # PASS
grep -q 'required_script_type: ScriptType'  client/src/discover.rs         # PASS (new sig)
grep -q 'does not support'                  client/src/discover.rs         # PASS (SC#3 wording)
! grep -q 'fn parse_onion_from_rr'          client/src/discover.rs         # PASS (old helper deleted)
! grep -q 'is_legacy_coordinator: bool'     client/src/round/input.rs      # PASS (transitional removed)
grep -q 'coordinator_info.capabilities.is_legacy' client/src/round/input.rs # PASS
grep -q 'discover_coordinator(pkarr_key, wallet.script_type())' client/src/main.rs # PASS
grep -q 'Detected legacy v1.3 coordinator' client/src/main.rs              # PASS (CD-21)
grep -q 'WALLET-03: fail-fast runs here'   client/src/main.rs              # PASS (Pitfall 4)
grep -q 'register_input(&client, &wallet, &info, &coordinator_info)' client/src/main.rs # PASS
grep -q 'CoordinatorCapabilities {'        client/src/main.rs              # PASS (synthetic)
grep -c 'CRIT-01'                          client/src/round/input.rs       # PASS (returns 2 ≥ 1)
grep -c 'CRIT-01'                          coordinator/src/bitcoin/utxo.rs # PASS (returns 2, unchanged)
```

### Test-Level Gates

- 8 `client::discover::tests` GREEN
- 4 `client::round::input::tests` GREEN (unchanged from 17-02)
- 3 LIVE multi_script_client tests GREEN; 6 ignored with cross-reference
- 8 `full_round::*` cross-phase invariant tests GREEN
- `cargo build --workspace` CLEAN
- `cargo audit` CLEAN (exit 0)

## DiscoveryError 6-Variant PII-Safety Audit

Per PROJECT.md constraint "No PII logging; round state zeroed after broadcast" + symmetric with `coordinator/src/bitcoin/utxo.rs::UtxoError`:

| Variant | Format String | PII-Safety |
|---------|---------------|------------|
| `InvalidPubkey(String)` | `"Invalid PKARR public key: {0}"` | Carries the bad pubkey z32 string (public DHT data; user-supplied input). Symmetric with UtxoError carrying "outpoint hex" — but no actual UTXO leaked. SAFE |
| `NotFound { pubkey }` | `"Coordinator not found in DHT for key '{pubkey}'"` | pubkey is z32 (public DHT data). SAFE |
| `MissingOnion { pubkey }` | `"No 'onion' field found in PKARR record for key '{pubkey}'"` | pubkey is z32. SAFE |
| `MalformedRecord { reason }` | `"Malformed PKARR record: {reason}"` | reason is a structural string (e.g. "invalid sst/ost token 'invalid-token'") — coordinator-side bug indicator. SAFE |
| `UnsupportedScriptType { pubkey, required, supported }` | `"coordinator {pubkey} does not support {required:?} ownership proofs (supports: {supported:?})"` | All public protocol values. SAFE. **Carries the ROADMAP SC#3 literal "does not support" wording** |
| `UnsupportedOutputScriptType { pubkey, advertised, wanted }` | `"coordinator {pubkey} CoinJoin output is {advertised:?} but wallet requires {wanted:?}"` | All public protocol values. SAFE |

**Never leaked:** IP, UTXO outpoint, wallet identifier, key bytes, amounts, BIP-322 witness contents.

## Pitfall 5 Evidence (LOAD-BEARING)

```
$ grep -A2 '#\[serde(rename = "v"' client/src/discover.rs
    #[serde(rename = "v", default = "default_legacy_version")]
    version: String,
    onion: String,

$ cargo test -p client --lib parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field
running 1 test
test discover::tests::parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field ... ok
```

The regression test `parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field` proves the `rename = "v"` discipline by feeding `{"version":"BOGUS","v":"0.2.0","onion":"z.onion"}` and asserting `parsed.version == "0.2.0"` (the compact `v` field wins). Without the rename annotation the decoder would parse `"BOGUS"` and the v1.4 coordinator would look like a legacy one — silently breaking WALLET-04 in the wrong direction.

## Pitfall 4 / D-74 Structural Pre-Tor Ordering Proof

```rust
// client/src/main.rs ~ lines 59-67
    //
    // WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural
    // ordering, not a runtime hack — the `discover::discover_coordinator`
    // call site runs UNCONDITIONALLY at this line, before the
    // `if cfg.use_tor` branch below at the `tor::init_tor` call site. Per
    // RESEARCH Pitfall 4 + D-74 a future refactor that moves the discover
    // call inside the Tor branch would silently break WALLET-03; this
    // comment is the in-source proof of the structural invariant.
    let coordinator_info = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
        let info = discover::discover_coordinator(pkarr_key, wallet.script_type())
            ...
```

The discover_coordinator call returns `Err` BEFORE the `if cfg.use_tor` branch is even evaluated; `tor::init_tor` is structurally unreachable on a rejected coordinator.

## Transitional Parameter Replacement (17-02 → 17-03)

| Site | 17-02 | 17-03 |
|------|-------|-------|
| `client::round::input::register_input` signature | `..., is_legacy_coordinator: bool` | `..., coordinator_info: &CoordinatorInfo` |
| `client/src/main.rs` call site | `register_input(&client, &wallet, &info, false)` | `register_input(&client, &wallet, &info, &coordinator_info)` |
| `liquidity-bot/src/main.rs` call site | `register_input(http, wallet, info, false)` | `register_input(http, wallet, info, &synthetic_info)` (synthetic CoordinatorInfo) |
| `tests/integration/full_round.rs` (6 sites) | `register_input(..., /* is_legacy_coordinator: */ false)` | `register_input(..., &v14_p2wpkh_coordinator_info())` (new helper) |

All 9 call sites (1 client + 1 liquidity-bot + 6 full_round + 1 main.rs already counted) updated consistently; workspace build clean; cross-phase invariant GREEN.

## ROADMAP Phase 17 Success Criteria 1-5 Mapping

| SC | Wording | Satisfied By |
|----|---------|--------------|
| #1 | `client --generate-wallet --type {p2wpkh|p2tr|p2sh-p2wpkh}` produces matching descriptor | 17-01 wallet::tests::generate_*_produces_bip*_descriptor (3 tests) + 17-03 ignored stubs cross-reference |
| #2 | sign roundtrip for each of 3 types | 17-02 client/tests/wallet_sign_roundtrip.rs (3 tests) + 17-03 ignored stubs cross-reference |
| #3 | rejects coordinator at discovery time BEFORE opening Tor circuit, error names BOTH pubkey AND missing script type | 17-03 multi_script_client::v13_pkarr_record_with_p2tr_wallet_rejects_before_tor LIVE + WALLET-03 structural ordering at main.rs:60 |
| #4 | v1.4 client with P2WPKH wallet against v1.3 coordinator emits v=1 OwnershipProof envelope | 17-03 multi_script_client::v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope LIVE + WIF wallet invariant gate |
| #5 | v1.3 P2WPKH-only full_round::* integration tests remain green | `cargo test --test integration full_round` 8/8 GREEN at every plan boundary in this phase |

## Deviations from Plan

### No Auto-fixes Required (no deviations from plan-spec contracts)

All Plan 17-03 contracts landed verbatim per CONTEXT D-71..D-80 + CD-17..CD-24 + the `<context_override>` block (Pitfall 5 correction). Three minor execution micro-decisions:

1. **`thiserror` direct dep declaration in client/Cargo.toml**: The plan implied this via the DiscoveryError thiserror derive. Workspace pin was already present at root Cargo.toml line 17; added the direct-dep declaration to client/Cargo.toml. Zero new transitives in Cargo.lock (already pulled by shared + coordinator).

2. **Encoder-inlining in test file (vs visibility escalation)**: Plan-spec said "either re-implement the 19-line helper inline in the test (acceptable for tests; the helper is verbatim from tests/integration/multi_script_validate.rs), or escalate the helper to `pub(crate)` in input.rs and import — plan-phase prefers RE-IMPLEMENT to keep input.rs's surface minimal." Followed the recommended re-implement path; the 17-LOC helper sits inline in multi_script_client.rs.

3. **Visibility escalation on `capabilities_from_record_v`**: The plan considered "if Rule 3 applied to `capabilities_from_record`, for cross-crate test access" — applied per the same Phase 16-02 escalation pattern on `validate_ownership_proof_typed`. Function is `pub` with `#[doc(hidden)]`. Documented inline in the function's doc-comment.

4. **`v14_p2wpkh_coordinator_info()` helper in full_round.rs**: The plan didn't pre-specify a helper for the 6 call-site updates; the cleanest mechanical replacement was to add a single-purpose helper function rather than inline 6 copies of the synthetic-CoordinatorInfo constructor. The helper documents the v1.3 invariant rationale inline.

## Self-Check

Verifying claims before proceeding.

**Files exist:**
- `client/src/discover.rs` — FOUND (405 LOC, replaced from 102 LOC)
- `client/src/round/input.rs` — FOUND (transitional param replaced)
- `client/src/main.rs` — FOUND (WALLET-03 comment + CD-21 WARN + synthetic CoordinatorInfo)
- `client/Cargo.toml` — FOUND (thiserror added)
- `liquidity-bot/src/main.rs` — FOUND (synthetic CoordinatorInfo)
- `tests/integration/full_round.rs` — FOUND (helper + 6 call sites updated)
- `tests/integration/mod.rs` — FOUND (`mod multi_script_client;` added)
- `tests/integration/multi_script_client.rs` — FOUND (NEW 374 LOC)

**Commits exist:**
- `1993436` (Task 1) — FOUND on main
- `b677223` (Task 2) — FOUND on main
- `460aa4d` (Task 3) — FOUND on main

**Verification commands GREEN:**
- `cargo build --workspace` — PASS (clean, no warnings)
- `cargo test -p client --lib discover::tests` — PASS (8/8)
- `cargo test -p client --lib round::input::tests` — PASS (4/4)
- `cargo test --test integration multi_script_client` — PASS (3/3 LIVE + 6 ignored)
- `cargo test --test integration full_round` — PASS (8/8 — cross-phase invariant)
- `cargo audit` — PASS (exit 0, 718 deps, no vulnerabilities)
- All source-level grep gates PASS
- All 9 D-78 test names present in multi_script_client.rs

## Phase 17 — COMPLETE

WALLET-01 (17-01) + WALLET-02 (17-02) + WALLET-04 encoder (17-02) + WALLET-03 (17-03) + WALLET-04 discovery (17-03) all closed. The 9-test acceptance gate at `tests/integration/multi_script_client.rs` is in place. Ready for `/gsd:verify-phase 17` + `/gsd:plan-phase 18` (INTEG-01 mixed-script E2E + INTEG-02 liquidity-bot multi-script keychain — per D-79 the v1.3-binary integration test belongs to Phase 18).

## Self-Check: PASSED
