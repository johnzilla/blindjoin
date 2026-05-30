---
phase: 17-client-multi-script-wallet-discovery
verified: 2026-05-30T21:30:00Z
status: passed
score: 16/16 must-haves verified
overrides_applied: 0
---

# Phase 17: Client Multi-Script Wallet & Discovery — Verification Report

**Phase Goal:** A user with a v1.4 client can generate a wallet of any of three script types, sign BIP-322 ownership proofs for that type, and reject mismatched coordinators before any Tor circuit opens.

**Verified:** 2026-05-30T21:30:00Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### ROADMAP Success Criteria (5 / 5 satisfied)

| # | Success Criterion | Status | Evidence |
|---|---|---|---|
| SC#1 | Per-type descriptor generation (`tr(.../86'/...)`, `sh(wpkh(.../49'/...))`, `wpkh(.../84'/...)`) | ✓ VERIFIED | CLI smoke (all three) printed correct descriptors with literal `coin=0'` across networks per D-66; unit tests `generate_p2{wpkh,tr,sh_p2wpkh}_produces_bip{84,86,49}_descriptor` GREEN |
| SC#2 | Sign roundtrip for all 3 script types via `wallet.sign_bip322` → `shared::bip322::verify_simple` | ✓ VERIFIED | `cargo test -p client --test wallet_sign_roundtrip` 7/7 GREEN (P2WPKH descriptor + P2WPKH WIF + P2TR descriptor + P2SH-P2WPKH descriptor sign↔verify roundtrips; D-70 symmetry + CRIT-01 seed assertions) |
| SC#3 | P2TR wallet vs v1.3 coordinator rejected BEFORE Tor with clear error naming both coordinator + missing script type | ✓ VERIFIED | `v13_pkarr_record_with_p2tr_wallet_rejects_before_tor` GREEN — error Display includes literal `does not support`, `P2tr`, `P2wpkh`, and the pubkey; main.rs structural pre-Tor ordering at line 68 (discover) BEFORE line 111 (`if cfg.use_tor`); WALLET-03 inline comment at line 61 documents the invariant |
| SC#4 | P2WPKH wallet vs v1.3 coordinator → v=1 OwnershipProof shim | ✓ VERIFIED | `v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope` GREEN — emits v1.3 array-of-hex form via OwnershipProof::to_json_hex_str CD-7 branch; WIF wallet path bit-exact preserved |
| SC#5 | v1.3 `full_round::*` integration tests remain green | ✓ VERIFIED | `cargo test --test integration full_round` 8/8 GREEN (42.23s) — cross-phase invariant preserved at phase boundary |

**Score:** 5/5 ROADMAP Success Criteria verified

### Observable Truths (from PLAN frontmatter must_haves)

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | `client --generate-wallet --type p2tr` prints `tr(.../86'/0'/0'/0/*)` descriptor | ✓ VERIFIED | CLI smoke: `tr(tprv.../86'/0'/0'/0/*)` printed |
| 2 | `--type p2sh-p2wpkh` prints `sh(wpkh(.../49'/0'/0'/0/*))` | ✓ VERIFIED | CLI smoke: `sh(wpkh(tprv.../49'/0'/0'/0/*))` printed |
| 3 | `--type p2wpkh` (or omitted) prints `wpkh(.../84'/0'/0'/0/*)` byte-equivalent v1.3 | ✓ VERIFIED | CLI smoke: `wpkh(tprv.../84'/0'/0'/0/*)` printed; coin=0' literal preserved |
| 4 | v1.4 client with `--type p2tr` + wpkh descriptor fails at construction naming both | ✓ VERIFIED | `from_descriptor_rejects_p2tr_flag_with_wpkh_descriptor` GREEN; error: `"descriptor wrapper P2wpkh does not match --type P2tr"` |
| 5 | `wallet.script_type()` returns descriptor's outer wrapper for every construction path | ✓ VERIFIED | `script_type_accessor_matches_construction` + `from_wif_asserts_p2wpkh` both GREEN |
| 6 | `wallet.sign_bip322(message)` returns Bip322SignedProof for all 3 descriptor wallet types via bdk PSBT path | ✓ VERIFIED | `p2{wpkh,tr,sh_p2wpkh}_descriptor_sign_roundtrip_verifies` (3 tests) GREEN |
| 7 | `wallet.sign_bip322(message)` returns Bip322SignedProof for P2WPKH WIF via `sign_simple` (v1.3 bit-exact) | ✓ VERIFIED | `p2wpkh_wif_sign_roundtrip_verifies` GREEN; WIF branch routes through `shared::bip322::sign_simple(P2wpkh, ...)` |
| 8 | `register_input` emits v=2 envelope with full-PSBT psbt_input_b64 when coordinator non-legacy | ✓ VERIFIED | `register_input_with_v14_coordinator_emits_v2_envelope` + `build_v2_psbt_input_b64_roundtrips_via_coordinator_decoder` both GREEN; encoder/decoder byte-inverse contract (Pitfall 1 fix) confirmed via BIP-174 magic `0x70 0x73 0x62 0x74 0xff` prefix assertion |
| 9 | `register_input` emits v=1 envelope when coordinator is legacy (byte-identical to v1.3) | ✓ VERIFIED | `register_input_with_legacy_coordinator_emits_v1_envelope` GREEN; CD-7 branch fires array-of-hex form |
| 10 | CRIT-01 inline comment present at v=2 envelope script_type assignment | ✓ VERIFIED | `grep -c "CRIT-01" client/src/round/input.rs` returns 2 (≥ 1 required); exact-string comment at line 152 |
| 11 | `crit-01-client-grep-check` CI job enforces grep gate | ✓ VERIFIED | `.github/workflows/ci.yml` lines 265-290 — symmetric with coordinator-side `crit-01-grep-check` |
| 12 | `generate_bip322_witness` DELETED per CD-20 | ✓ VERIFIED | `grep -c "fn generate_bip322_witness"` returns 0 across all of `client/` |
| 13 | Transitional `is_legacy_coordinator: bool` param REMOVED from register_input | ✓ VERIFIED | `grep -rn "is_legacy_coordinator"` returns only comment references documenting the prior transitional state in main.rs:129 + full_round.rs:41 — NO live param uses; new signature takes `&CoordinatorInfo` |
| 14 | BlindjoinRecord uses `#[serde(rename = "v", default = "default_legacy_version")]` per Pitfall 5 | ✓ VERIFIED | discover.rs line 123; `parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field` GREEN proves the rename is load-bearing |
| 15 | WALLET-03 structural pre-Tor ordering proof | ✓ VERIFIED | main.rs lines 61-67 `WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural ordering, not a runtime hack` — discover call at line 68 < tor::init_tor at line 112 |
| 16 | v1.3 cross-phase invariant remains GREEN | ✓ VERIFIED | `cargo test --test integration full_round` 8/8 PASS |

**Score:** 16/16 must-haves verified

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| WALLET-01 | 17-01 | Client wallet supports BIP-84/86/49 descriptor templates via `--type` flag | ✓ SATISFIED | CLI smoke + 12 unit tests (6 config + 6 wallet) GREEN; per-type descriptor templates land in `generate` + `from_descriptor` with construction-time mismatch check per D-63 |
| WALLET-02 | 17-02 | Client signs BIP-322 ownership proofs for all 3 script types via `shared::bip322::sign_simple` + bdk PSBT path | ✓ SATISFIED | 7 sign roundtrip tests + 4 round::input::tests GREEN; WIF→sign_simple(P2wpkh); descriptor→bdk PSBT uniform per CD-24 |
| WALLET-03 | 17-03 | Client reads supported_script_types from PKARR BEFORE Tor; rejects mismatched coordinator | ✓ SATISFIED | `v13_pkarr_record_with_p2tr_wallet_rejects_before_tor` LIVE GREEN; main.rs structural pre-Tor ordering + WALLET-03 inline comment + ROADMAP SC#3 wording check (`does not support`) |
| WALLET-04 | 17-02 (encoder) + 17-03 (discovery) | Pre-0.2.0 coordinator detection → v=1 OwnershipProof shim | ✓ SATISFIED | Discovery side: `capabilities.is_legacy` derived from `record.version != "0.2.0" \|\| record.sst.is_none()`; encoder side: v=1 envelope branch emits CD-7 byte-identity form; `v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope` LIVE GREEN |

**No orphaned requirements.** All 4 phase requirement IDs accounted for via 3 plans (17-01, 17-02, 17-03).

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `client/src/config.rs` | --type flag + BLINDJOIN_SCRIPT_TYPE env var + parse_script_type helper | ✓ VERIFIED | All elements present; 6 config::tests GREEN |
| `client/src/wallet.rs` | script_type field + per-type descriptor templates + mismatch check + Bip322SignedProof + sign_bip322 dispatcher | ✓ VERIFIED | All elements present; 6 wallet::tests GREEN + 7 wallet_sign_roundtrip tests GREEN |
| `client/src/discover.rs` | CoordinatorCapabilities + DiscoveryError 6 variants + extended discover_coordinator + Pitfall 5 rename | ✓ VERIFIED | All elements present; 8 discover::tests GREEN |
| `client/src/round/input.rs` | build_v2_psbt_input_b64 + v1/v2 envelope branch + CRIT-01 comment + capabilities consumption | ✓ VERIFIED | All elements present; 4 round::input::tests GREEN; generate_bip322_witness DELETED |
| `client/src/main.rs` | Wire cfg.script_type + discover_coordinator + CD-21 WARN log + WALLET-03 inline comment | ✓ VERIFIED | All elements present; CLI smoke confirms end-to-end |
| `client/tests/wallet_sign_roundtrip.rs` | 7 sign↔verify roundtrip tests | ✓ VERIFIED | 7/7 tests GREEN |
| `tests/integration/multi_script_client.rs` | 9 D-78 named tests | ✓ VERIFIED | 3 LIVE + 6 ignored cross-references; all 9 test fn names present |
| `.github/workflows/ci.yml` | crit-01-client-grep-check CI job | ✓ VERIFIED | Lines 265-290 present, symmetric with coordinator-side gate |

---

## Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `client/src/config.rs` | `shared::bip322::ScriptType` | value_parser via serde wire-form roundtrip | ✓ WIRED | `parse_script_type` routes through `serde_json::from_str::<ScriptType>` |
| `client/src/wallet.rs::generate` | BIP-84/86/49 literal descriptor templates | per-script `format!()` branch | ✓ WIRED | Lines 237-246 — coin=0' literal across networks per D-66 (Pitfall 2 guard) |
| `client/src/wallet.rs::from_descriptor` | Construction-time mismatch error | outer-wrapper detection vs `script_type` | ✓ WIRED | Lines 146-161 — longest-match ordering (sh(wpkh first) |
| `client/src/round/input.rs::build_v2_psbt_input_b64` | `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` | base64(Psbt::serialize) ↔ Psbt::deserialize byte-inverse | ✓ WIRED | Roundtrip test asserts BIP-174 magic prefix + Psbt::deserialize round-trip (Pitfall 1 fix) |
| `client/src/round/input.rs (v=2 envelope)` | `wallet.script_type()` | `signed.script_type` (NEVER cfg.script_type) | ✓ WIRED | CRIT-01 inline comment at input.rs:152 + grep gate enforces |
| `client/src/discover.rs::discover_coordinator` | `client/src/main.rs` PKARR call site | Pre-Tor structural ordering | ✓ WIRED | discover_coordinator at main.rs:68 < tor::init_tor at main.rs:112 |
| `wallet.script_type()` (17-01) | `discover_coordinator(pubkey, wallet.script_type())` | Single-arg passthrough | ✓ WIRED | main.rs:69 passes `wallet.script_type()` directly |
| `register_input` (17-02) | `coordinator_info.capabilities.is_legacy` (17-03) | New &CoordinatorInfo param (transitional bool removed) | ✓ WIRED | round/input.rs:70 takes `&CoordinatorInfo`; line 123 reads `is_legacy` |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `BdkClientWallet.script_type` | script_type field | Set at construction in from_wif/from_descriptor/generate | Yes — descriptor parsed or CLI-supplied | ✓ FLOWING |
| `Bip322SignedProof.witness` | witness bytes | bdk_wallet PSBT sign (descriptor) OR shared::bip322::sign_simple (WIF) | Yes — verified via verify_simple roundtrip in 7 tests | ✓ FLOWING |
| `CoordinatorCapabilities.is_legacy` | is_legacy flag | Derived from PKARR record version + sst presence | Yes — `record.version != "0.2.0" \|\| record.sst.is_none()` | ✓ FLOWING |
| `OwnershipProof.script_type` (v=2 wire) | script_type field | `signed.script_type` (which is `self.script_type` from wallet) | Yes — CRIT-01 client-side seed verified in tests + comment + CI gate | ✓ FLOWING |

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| CLI generates P2TR descriptor | `client --generate-wallet --type p2tr` | Prints `tr(tprv.../86'/0'/0'/0/*)` + matching change descriptor | ✓ PASS |
| CLI generates P2SH-P2WPKH descriptor | `client --generate-wallet --type p2sh-p2wpkh` | Prints `sh(wpkh(tprv.../49'/0'/0'/0/*))` | ✓ PASS |
| CLI generates default P2WPKH descriptor | `client --generate-wallet` | Prints `wpkh(tprv.../84'/0'/0'/0/*)` (coin=0' v1.3 byte-equivalent) | ✓ PASS |
| Sign roundtrip all 3 types | `cargo test -p client --test wallet_sign_roundtrip` | 7/7 PASS in 0.28s | ✓ PASS |
| Discovery rejects mismatched script type before Tor | `cargo test --test integration multi_script_client v13_pkarr_record_with_p2tr_wallet_rejects_before_tor` | PASS | ✓ PASS |
| v=1 envelope shim for legacy coordinator | `cargo test --test integration multi_script_client v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope` | PASS | ✓ PASS |
| v=2 envelope happy path | `cargo test --test integration multi_script_client v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope` | PASS — includes encoder/decoder roundtrip via Psbt::deserialize | ✓ PASS |
| Cross-phase invariant | `cargo test --test integration full_round` | 8/8 PASS in 42.23s | ✓ PASS |
| Workspace builds clean | `cargo build --workspace` | Finished `dev` profile in 0.29s; no warnings | ✓ PASS |
| Cargo audit clean | `cargo audit --quiet` | EXIT=0 | ✓ PASS |

---

## Anti-Patterns Found

**None.**

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| — | — | No `TBD`, `FIXME`, `XXX` markers in any modified file | — | — |
| — | — | No `TODO`, `HACK`, `PLACEHOLDER` markers in production code | — | — |
| — | — | No `bdk_wallet::template::Bip*` helpers used (Pitfall 2 guard) | — | — |
| — | — | No manual P2TR sign fallback (D-15 RETIRED) | — | — |
| — | — | No PII in user-facing logs/errors (PROJECT.md discipline) | — | — |

---

## Critical Correctness Checks (from verification request)

| Check | Status | Evidence |
|---|---|---|
| D-69 OVERRIDE: `build_v2_psbt_input_b64` uses `Psbt::serialize` (full PSBT) not `psbt::Input::serialize` | ✓ VERIFIED | `client/src/round/input.rs:35-57` uses `Psbt::from_unsigned_tx` + `psbt.serialize()`; roundtrip test asserts BIP-174 magic `0x70 0x73 0x62 0x74 0xff` prefix |
| D-73 OVERRIDE: `#[serde(rename = "v"` in client/src/discover.rs | ✓ VERIFIED | Line 123 — load-bearing rename per Pitfall 5; regression test `parse_blindjoin_record_decodes_v0_2_0_compact_form_uses_v_field` proves `v` wins over `version` |
| CRIT-01 client-side discipline | ✓ VERIFIED | `grep -c CRIT-01 client/src/round/input.rs` = 2 (≥ 1 required); `crit-01-client-grep-check` CI job present at ci.yml:265 |
| `generate_bip322_witness` deletion | ✓ VERIFIED | No matches in client/ (`grep -c "fn generate_bip322_witness" client/src/...` = 0) |
| Transitional `is_legacy_coordinator: bool` removed | ✓ VERIFIED | Only comment-references remain in main.rs:129 + full_round.rs:41 (both documenting prior transitional state for grep traceability) — no live param |
| Discoverable types preserved (Phase 15 LOCKED API) | ✓ VERIFIED | client imports `shared::bip322::{ScriptType, verify_simple, sign_simple}`; no client-local BIP-322 primitives leaked in |
| WALLET-04 supported intersection only (encoder NEVER silently emits v=1 for non-P2WPKH) | ✓ VERIFIED | Discovery rejects non-P2WPKH against v1.3 upstream (`v13_pkarr_record_with_p2tr_wallet_rejects_before_tor`); encoder's v=1 branch has `debug_assert_eq!(signed.script_type, P2wpkh)` guard |
| Threat model: no PII in logs/errors | ✓ VERIFIED | DiscoveryError variants name only pubkey z32 (public DHT data) + ScriptType enum values + structural reasons; CD-21 WARN log carries `coordinator_pubkey` + `record_version` only |
| Out-of-scope adherence | ✓ VERIFIED | No P2WSH multisig, no per-script ban tracking/rate limits/denominations, no mixed output script types, no manual P2TR fallback — all absent from grep |

---

## Test Setup Audit (Step 7d)

| Helper | Constructs | Production analog | Risk | Disposition |
|---|---|---|---|---|
| `BdkClientWallet::generate(DUMMY_OUTPOINT, Network, ScriptType)` | BdkClientWallet (production type) | `cfg.script_type` → main.rs:33 → `ClientWallet::generate(utxo, network, cfg.script_type)` | LOW | Acceptable fixture — production reaches same constructor via CLI |
| `BdkClientWallet::from_wif(TEST_WIF, DUMMY_OUTPOINT, Network)` | BdkClientWallet (production type) | `cfg.utxo_wif` → main.rs:56 → `ClientWallet::from_wif(wif, utxo, network)` | LOW | Acceptable fixture — production reaches same constructor via CLI |
| `capabilities_from_record_v(version, sst, ost)` | CoordinatorCapabilities | `discover_coordinator` (production) calls same function at discover.rs:265 | LOW | Acceptable — pub(doc(hidden)) test escalation; production path uses same helper |
| `build_v2_psbt_input_b64(witness, sig)` (inlined in tests) | Wire bytes | Same helper at `client/src/round/input.rs:35` (called from `register_input`) | LOW | Acceptable — test re-implements the 19-LOC helper inline to avoid visibility escalation; shape contract verified by 3-site verbatim convergence |
| `OwnershipProof { version: 2, ... }` (inlined construction) | Wire envelope | Same struct constructed at input.rs:147 in register_input | LOW | Acceptable — direct struct literal mirrors production branch |

**No HIGH-risk test setup helpers found.** Tests exercise production-reachable code paths.

---

## ROADMAP Success Criteria Mapping

| SC# | Wording | Satisfied By | Evidence Type |
|---|---|---|---|
| 1 | `client generate-wallet --type {each}` produces matching descriptor | 17-01 wallet::tests + 17-03 multi_script_client stubs cross-reference | Unit tests + CLI smoke |
| 2 | sign roundtrip for each of 3 types | 17-02 client/tests/wallet_sign_roundtrip.rs (7 tests) | Integration test |
| 3 | Rejects coordinator at discovery BEFORE Tor, error names both | 17-03 multi_script_client::v13_pkarr_record_with_p2tr_wallet_rejects_before_tor LIVE + WALLET-03 structural ordering | Integration test + structural ordering proof |
| 4 | v1.4 client + P2WPKH wallet + v1.3 coordinator emits v=1 envelope | 17-03 multi_script_client::v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope LIVE | Integration test (WIF wallet invariant gate) |
| 5 | v1.3 full_round::* tests still pass | `cargo test --test integration full_round` 8/8 GREEN | Cross-phase invariant |

---

## Verification Commands & Outputs

```
$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

$ cargo test -p client --lib
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s

$ cargo test -p client --test wallet_sign_roundtrip
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s

$ cargo test --test integration multi_script_client
test result: ok. 3 passed; 0 failed; 6 ignored; 0 measured; 30 filtered out; finished in 0.13s

$ cargo test --test integration full_round
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 31 filtered out; finished in 42.23s

$ cargo audit --quiet; echo "EXIT=$?"
EXIT=0

$ grep -c "CRIT-01" client/src/round/input.rs
2

$ grep -c "fn generate_bip322_witness" client/src/round/input.rs client/src/wallet.rs client/src/main.rs
0 (all files)

$ grep -rn "is_legacy_coordinator" client/ liquidity-bot/ tests/
client/src/main.rs:129:    // the 17-02 transitional `is_legacy_coordinator: bool` 4th arg). The   [comment only]
tests/integration/full_round.rs:41:/// the 17-02 transitional `is_legacy_coordinator: bool` 4th arg). The  [comment only]

$ grep -nE "^[^/]*[^=]rename\s*=\s*\"v\"" client/src/discover.rs
123:    #[serde(rename = "v", default = "default_legacy_version")]

$ /tmp/blindjoin client --generate-wallet --type p2tr | grep "External descriptor" -A1
External descriptor (receiving addresses):
  tr(tprv8ZgxMBicQKsPep1By4FNyTyWLQbae7DVdoWcHv1qh4n7soeJQVgAVSFnjqXHnds1QjrCmnb4ARUpLmstqLV3Hse15cg2ThQtrMaAUw6NTVD/86'/0'/0'/0/*)

$ /tmp/blindjoin client --generate-wallet --type p2sh-p2wpkh | grep "External descriptor" -A1
External descriptor (receiving addresses):
  sh(wpkh(tprv8ZgxMBicQKsPedoVUb3trxoo6Pv1J5XJFkKmd3MdZBRUP5YdJXDVGYnfmTNsrT8sud8erkMww9qCiSqgGdpKKaPLraTKEbFQjUWSLd4mynS/49'/0'/0'/0/*))

$ /tmp/blindjoin client --generate-wallet | grep "External descriptor" -A1
External descriptor (receiving addresses):
  wpkh(tprv8ZgxMBicQKsPd94Su6QxwU1vodwaovnyBcJPQbs3dVufDfYX7VVddLNZFbQc8CWG316aZt3YJiqFGTefkgeoFNKAi2XemwBd3xzvegygjK4/84'/0'/0'/0/*)
```

---

## Summary

Phase 17 has fully achieved its goal. All 5 ROADMAP Success Criteria + all 16 derived must-have truths are observably satisfied in the codebase:

1. **WALLET-01 (per-type descriptors)**: `--type` flag drives BIP-84/86/49 literal templates with `coin=0'` preserved across networks per D-66; construction-time mismatch check fires per D-63.
2. **WALLET-02 (per-type BIP-322 sign)**: `wallet.sign_bip322` dispatches WIF→`shared::bip322::sign_simple(P2wpkh)` and descriptor→bdk PSBT path uniformly per CD-24; all 3 script types roundtrip via `verify_simple`.
3. **WALLET-03 (pre-Tor fail-fast)**: `discover_coordinator` rejects mismatched script types at the resolver boundary; structural pre-Tor ordering at main.rs:68 < line 112 (Tor init); WALLET-03 inline comment is the in-source invariant proof.
4. **WALLET-04 (compat shim)**: Discovery side detects legacy coordinator via `version != "0.2.0" \|\| sst.is_none()`; encoder side routes through OwnershipProof CD-7 byte-identity branch.

**Load-bearing corrections verified:**
- Pitfall 1 (D-69 wire shape override): `build_v2_psbt_input_b64` produces full BIP-174 PSBT, not bare `psbt::Input` — confirmed via the `0x70 0x73 0x62 0x74 0xff` magic prefix assertion.
- Pitfall 5 (D-73 field rename override): BlindjoinRecord uses `#[serde(rename = "v")]` — confirmed via regression test that proves `v` wins over `version`.

**Cross-phase invariant preserved**: `cargo test --test integration full_round` 8/8 GREEN (42.23s) — v1.3 P2WPKH WIF path bit-exact unchanged.

**CRIT-01 client-side discipline**: inline comment present + CI grep gate enforces; `grep -c CRIT-01 client/src/round/input.rs` returns 2.

**CD-20 deletion completed**: `generate_bip322_witness` no longer exists in client/.

**Transitional removed**: `is_legacy_coordinator: bool` parameter from 17-02 replaced by `&CoordinatorInfo` in 17-03; only comment-references remain for traceability.

**Out-of-scope adherence**: No P2WSH multisig, no per-script ban tracking, no per-script rate limits, no mixed output script types, no manual P2TR sign fallback, no BIP-44-correct testnet coin-type indexing — all correctly absent.

**No anti-patterns**: Zero debt markers (TBD/FIXME/XXX/TODO/HACK/PLACEHOLDER) in modified files; no PII in user-facing errors or logs.

Phase 17 is complete and ready for Phase 18 (Mixed-Script E2E + Liquidity Bot).

---

_Verified: 2026-05-30T21:30:00Z_
_Verifier: Claude (gsd-verifier)_
