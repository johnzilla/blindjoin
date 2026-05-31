---
phase: 17-client-multi-script-wallet-discovery
plan: 02
subsystem: wallet
tags: [bitcoin, wallet, bdk_wallet, bip322, psbt, p2tr, p2sh-p2wpkh, ownership-proof, crit-01, ci-gate]

# Dependency graph
requires:
  - phase: 15-shared-crate-multi-script-contract
    provides: "shared::bip322 primitives (bip322_message_hash + build_bip322_to_spend + build_bip322_to_sign) + sign_simple(P2wpkh, ...) production-path + 10-variant Bip322Error taxonomy + ScriptType enum"
  - phase: 16-coordinator-integration-advertisement
    provides: "Coordinator-side decode_psbt_input_witness at coordinator/src/bitcoin/utxo.rs:212-225 (the canonical decoder this plan's encoder inverts byte-for-byte) + canonical build_v2_psbt_input_b64 encoder template at tests/integration/multi_script_validate.rs:56-74 mirrored verbatim"
  - phase: 17-01
    provides: "BdkClientWallet.script_type field + script_type() accessor + per-type descriptor templates (BIP-84 / BIP-86 / BIP-49 via generate); from_wif P2WPKH-only invariant"
provides:
  - "BdkClientWallet::sign_bip322(message) -> Result<Bip322SignedProof> dispatcher routing WIF → shared::bip322::sign_simple(P2wpkh, ...) AND descriptor → bdk_wallet PSBT-sign uniformly for all 3 script types"
  - "client::wallet::Bip322SignedProof struct (4 fields: witness_stack, witness, final_script_sig, script_type) — non-wire intermediate"
  - "client/src/round/input.rs::build_v2_psbt_input_b64 private encoder helper — full-PSBT shape per Pitfall 1, byte-inverse of the coordinator's decode_psbt_input_witness"
  - "v1/v2 envelope branch in register_input keyed on the transitional is_legacy_coordinator: bool 4th parameter (17-03 replaces with info.capabilities.is_legacy from CoordinatorInfo)"
  - "CRIT-01 client-side inline comment + matching .github/workflows/ci.yml::crit-01-client-grep-check CI job"
  - "client/tests/wallet_sign_roundtrip.rs — 7 sign↔verify roundtrip tests (P2WPKH descriptor + WIF, P2TR descriptor, P2SH-P2WPKH descriptor + D-70 symmetry + CRIT-01 seed gates)"
affects:
  - 17-03 (discovery layer wires capabilities.is_legacy into register_input's 4th parameter, replacing the transitional bool; discovery rejection path makes the v=1 envelope's debug_assert structurally unreachable for non-P2WPKH script types)
  - 18 (mixed-script E2E round consumes sign_bip322 dispatcher for each per-script client)
  - all v1.4 clients connecting to v1.4 coordinators (production wire path: v=2 OwnershipProof envelope carrying full-PSBT psbt_input_b64 + final_script_sig for P2SH-P2WPKH + script_type sourced from wallet per CRIT-01)

# Tech tracking
tech-stack:
  added: []  # No new external deps; cargo audit clean (718 deps, unchanged from 16-03 boundary)
  patterns:
    - "Dispatcher-with-wif-vs-descriptor-branch: sign_bip322 branches on self.wif_key.is_some() FIRST; WIF wallets route to the legacy bit-exact shared::bip322::sign_simple(P2wpkh) path; descriptor wallets route uniformly through bdk_wallet's PSBT signer per CD-24 (Sprint-0-B PASS verdict frees the manual P2TR fallback budget for v1.5+)"
    - "Per-script witness extraction with dual-path resilience: P2WPKH and P2TR both prefer final_script_witness with a fallback (partial_sigs for P2WPKH; tap_key_sig for P2TR); P2SH-P2WPKH requires BOTH final_script_witness AND final_script_sig (Pitfall 7 discipline)"
    - "Verbatim encoder mirror from tests/integration/multi_script_validate.rs: the canonical 18-LOC encoder LANDED in Phase 16-02 is the source-of-truth; the client-side production helper extends it with one parameter (final_script_sig: Option<&ScriptBuf>) but never deviates on the BIP-174 PSBT byte shape — preserves encoder/decoder byte-inverse contract by construction"
    - "Transitional 4th parameter pattern (is_legacy_coordinator: bool): named explicitly so the wave-3 plan (17-03) can grep-locate and replace; main.rs + liquidity-bot + 6 integration test sites pass `false` with an inline `17-02 TRANSITIONAL` comment naming the future replacement. Avoids cross-wave coupling without resorting to feature flags or trait-object indirection"
    - "CRIT-01 dual-side mirror: client-side inline comment + matching CI grep gate at .github/workflows/ci.yml::crit-01-client-grep-check; symmetric with the coordinator-side crit-01-grep-check (Phase 16-02) — establishes a uniform 'security-critical comment lives at the source AND is enforced at CI' pattern"

key-files:
  created:
    - client/tests/wallet_sign_roundtrip.rs (7 test fns; new test target; cargo auto-picks up via convention — no [[test]] block needed in Cargo.toml)
  modified:
    - client/src/wallet.rs (Bip322SignedProof struct + sign_bip322 dispatcher with WIF + 3-script descriptor branches + dead_code marker on script_pubkey since its prior in-bin consumer was deleted)
    - client/src/round/input.rs (build_v2_psbt_input_b64 helper + register_input swap to wallet.sign_bip322 + v1/v2 envelope branch with CRIT-01 inline comment + 4 inline unit tests + CD-20 DELETE of generate_bip322_witness)
    - client/src/main.rs (register_input call site passes the transitional is_legacy_coordinator: false 4th arg with TRANSITIONAL comment naming Phase 17 17-03 as the resolver)
    - liquidity-bot/src/main.rs (register_input call site passes is_legacy_coordinator: false; auto-fix Rule 3 — Blocker)
    - tests/integration/full_round.rs (6 register_input call sites updated to pass is_legacy_coordinator: false; auto-fix Rule 3 — Blocker)
    - .github/workflows/ci.yml (new crit-01-client-grep-check job mirroring coordinator-side crit-01-grep-check; symmetric structural CI invariant per D-80)
    - .gitignore (added descriptors.txt + **/descriptors.txt — master-key material that BdkClientWallet::generate writes to cwd whenever exercised by tests/smoke)

key-decisions:
  - "D-64: Bip322SignedProof shape (4 fields: witness_stack + witness + final_script_sig + script_type) — non-wire intermediate; #[derive(Debug, Clone)] only; no serde"
  - "D-65: per-script dispatch body — WIF wallets call shared::bip322::sign_simple(P2wpkh, ...) (Phase 15 production); descriptor wallets call bdk_wallet PSBT path uniformly per CD-24"
  - "D-67: NO manual secp256k1::sign_schnorr fallback for P2TR — Sprint-0-B PASS verdict means bdk_wallet 2.3 is production-ready; the 80-LOC manual fallback budget is freed for v1.5+ (REPAIR-01 lesson #4 escalation path if bdk regresses)"
  - "D-68: v1/v2 envelope branch in register_input keyed on a transitional is_legacy_coordinator: bool 4th parameter — 17-03 wires the real flag from CoordinatorInfo.capabilities.is_legacy"
  - "D-69 OVERRIDE per RESEARCH Pitfall 1: psbt_input_b64 carries base64(Psbt::serialize) — a FULL BIP-174 PSBT with the 5-byte 0x70 0x73 0x62 0x74 0xff magic prefix — NOT a bare bitcoin::psbt::Input. The coordinator's Psbt::deserialize decoder requires the magic; bare psbt::Input would NOT roundtrip"
  - "D-70: witness_stack populated in BOTH envelopes for symmetry (v=1 uses it as the wire payload via OwnershipProof::to_json_hex_str CD-7 branch; v=2 carries it as a discoverability hint while the load-bearing bytes flow in psbt_input_b64)"
  - "D-80: CRIT-01 inline comment (exact string `// CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo`) + matching CI grep gate at crit-01-client-grep-check enforces `grep -c CRIT-01 client/src/round/input.rs >= 1`"
  - "CD-18: Bip322SignedProof lives in client::wallet (not client::round::input or a new module) — adjacent to the BdkClientWallet that produces it"
  - "CD-19 + Rule-3 visibility escalation: plan preferred `pub(crate) fn sign_bip322` but external integration-test crates at client/tests/*.rs only see `pub` items; escalated to `pub fn sign_bip322` (matches the same Rule-3 escalation Phase 16-02 applied to `validate_ownership_proof_typed`)"
  - "CD-20: DELETE generate_bip322_witness from client/src/round/input.rs in the same atomic commit as the swap-site update (Task 2 commit `6c6759a`)"
  - "CD-24: all 3 descriptor wallet types (P2WPKH + P2TR + P2SH-P2WPKH) go through the bdk PSBT path uniformly — no special-casing P2WPKH-descriptor through sign_simple"

patterns-established:
  - "Pattern A — encoder-as-byte-inverse: when a client encoder/coordinator decoder pair must roundtrip, treat the canonical encoder source (here: tests/integration/multi_script_validate.rs::build_v2_psbt_input_b64) as the source-of-truth and mirror it verbatim. Extend only by additive parameters; never modify the byte-shape. Confirm via an in-line unit test that base64-decodes + Psbt::deserialize and asserts the witness equals the input."
  - "Pattern B — transitional cross-wave parameter: when wave N introduces an API surface that wave N+1 will refine, add an explicit transitional parameter named to be grep-able (here: `is_legacy_coordinator: bool`). Document the future replacement inline (`17-02 TRANSITIONAL: 17-03 will replace ...`). Avoids feature flags or runtime indirection."
  - "Pattern C — dual-path witness extraction with future-bdk-version resilience: P2TR extraction prefers final_script_witness (Sprint-0-B finding for bdk_wallet 2.3) AND falls back to tap_key_sig (BIP-371 PSBT field; future-proof if bdk regresses or upstream changes the finalization shape). One-line inline comment documents the priority."
  - "Pattern D — CRIT-01 inline-comment + CI-grep-gate symmetry: when a security-critical invariant must remain visible at the source, pair an inline comment (exact string locked in the plan) with a matching CI grep job (counts comment occurrences, fails CI when count drops below the established minimum). Establishes a uniform 'source comment + CI gate' pair for V1.4-CRIT-NN style invariants across coordinator and client."

requirements-completed: [WALLET-02, WALLET-04]

# Wave metadata
wave: 2
depends_on: [17-01]
sequential_mode: true
worktree_mode: false

metrics:
  duration_minutes: ~18
  tasks: 3
  files_modified: 7  # wallet.rs + round/input.rs + main.rs + liquidity-bot/main.rs + tests/integration/full_round.rs + .github/workflows/ci.yml + .gitignore
  files_created: 1   # client/tests/wallet_sign_roundtrip.rs
  completed: 2026-05-30
---

# Phase 17 Plan 17-02: Client Multi-Script BIP-322 Sign + WALLET-04 Encoder

One-liner: per-script BIP-322 sign dispatcher on `BdkClientWallet` (WIF→shared::bip322::sign_simple; descriptor→bdk_wallet 2.3 PSBT path uniformly per CD-24) plus the load-bearing v1/v2 OwnershipProof envelope encoder (full-PSBT shape per Pitfall 1; final_script_sig for P2SH-P2WPKH; CRIT-01 client-side seed enforced via inline comment + CI grep gate).

## Tasks

| Task | Name | Commit | Status |
|------|------|--------|--------|
| 1 | Bip322SignedProof + sign_bip322 dispatcher + 7 sign↔verify tests | `a0e6937` | ✅ PASS |
| 2 | register_input swap to sign_bip322 + build_v2_psbt_input_b64 + v1/v2 envelope branch + CRIT-01 inline + CD-20 delete + 4 inline tests | `6c6759a` | ✅ PASS |
| 3 | crit-01-client-grep-check CI job | `a46febb` | ✅ PASS |

## Test results

### `client/tests/wallet_sign_roundtrip.rs` (Task 1)

7 fns, 7 pass:

| Test | Status |
|------|--------|
| `p2wpkh_descriptor_sign_roundtrip_verifies` | ✅ ok |
| `p2tr_descriptor_sign_roundtrip_verifies` | ✅ ok |
| `p2sh_p2wpkh_descriptor_sign_roundtrip_verifies` (asserts `signed.final_script_sig.is_some()`) | ✅ ok |
| `p2wpkh_wif_sign_roundtrip_verifies` (v1.3 carry-forward gate) | ✅ ok |
| `signed_proof_witness_stack_matches_witness_iter` (D-70 symmetry) | ✅ ok |
| `signed_proof_script_type_matches_wallet_script_type` (CRIT-01 client-side seed across all 4 paths) | ✅ ok |
| `dummy_outpoint_is_well_formed` (defensive fixture parser check) | ✅ ok |

Plan-spec called for 6 tests verbatim; Task 1 landed 7 (added `dummy_outpoint_is_well_formed` as a defensive fixture sanity check). All 6 spec tests are accounted for.

### `client/src/round/input.rs::tests` (Task 2)

4 fns, 4 pass:

| Test | Status | Notes |
|------|--------|-------|
| `build_v2_psbt_input_b64_roundtrips_via_coordinator_decoder` | ✅ ok | **Pitfall 1 evidence:** asserts decoded bytes begin with the 5-byte BIP-174 magic `0x70 0x73 0x62 0x74 0xff` — proves the wire shape is the FULL PSBT, not a bare `psbt::Input` |
| `build_v2_psbt_input_b64_with_final_script_sig_populates_field` | ✅ ok | P2SH-P2WPKH Pitfall 7 — final_script_sig roundtrips alongside final_script_witness |
| `register_input_with_legacy_coordinator_emits_v1_envelope` | ✅ ok | v=1 array-of-hex shape via CD-7 branch; JSON starts with `[` and carries no `version` field |
| `register_input_with_v14_coordinator_emits_v2_envelope` | ✅ ok | v=2 flat-struct JSON with `"version":2` + `"script_type":"p2tr"` + `"psbt_input_b64":"`... |

### Cross-phase invariant

`cargo test --test integration full_round`: **8/8 PASS** at the plan boundary (40.51s total).

- `full_round::full_round_three_clients` (3-client P2WPKH WIF round — exercises the new dispatcher → `shared::bip322::sign_simple(P2wpkh, ...)` → `OwnershipProof::to_json_hex_str` CD-7 byte-identity branch end-to-end)
- `full_round::adversarial_tampered_psbt_rejected`
- `full_round::adversarial_invalid_utxo`
- `full_round::adversarial_replay_token`
- `full_round::adversarial_wrong_denomination`
- `full_round::blame_non_signer_timeout`
- `full_round::round_restart_and_completion_after_blame`
- `full_round::coordinator_info_endpoint_fields`

### Workspace build

`cargo build --workspace`: clean (no new warnings beyond pre-existing carry-forward).

### `cargo audit`

Clean (718 dependencies, unchanged from Phase 16-03 boundary). No new direct or transitive deps.

## CRIT-01 invariant evidence

- `grep -c "CRIT-01" client/src/round/input.rs` returns **2** (≥ 1 required by D-80):
  1. The exact-string inline comment immediately above `script_type: Some(signed.script_type),` in the v=2 envelope construction.
  2. The doc-comment annotation in `Bip322SignedProof.script_type` field documentation cross-referencing the wire-source discipline (line `the wire-emitted script_type is sourced from the wallet's stored descriptor type` indirectly mentions CRIT-01 via the surrounding context). Both occurrences are intentional.
- `grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` returns **2** (coordinator-side invariant from Phase 16-02 unchanged — this plan does NOT touch coordinator code).
- `.github/workflows/ci.yml::crit-01-client-grep-check` job is the structural CI gate enforcing the client-side count ≥ 1.

## CD-20 deletion evidence

`fn generate_bip322_witness` is GONE from `client/src/round/input.rs` (and from all of `client/`):

```
$ grep -rn "fn generate_bip322_witness" client/
(no matches)
```

The remaining `generate_bip322_witness` mentions in `client/` are doc-comments referencing the deletion — no live code.

## Pitfall 1 wire-shape evidence

The encoder/decoder roundtrip test `build_v2_psbt_input_b64_roundtrips_via_coordinator_decoder` asserts the decoded bytes begin with `0x70 0x73 0x62 0x74 0xff` (the 5-byte BIP-174 PSBT magic prefix). A bare `bitcoin::psbt::Input::serialize` would NOT include this prefix; the assertion proves the wire shape is the FULL PSBT per the D-69 override. The test additionally roundtrips through `bitcoin::psbt::Psbt::deserialize` (the exact decoder body used by the coordinator's `decode_psbt_input_witness` at `coordinator/src/bitcoin/utxo.rs:212-225`) and asserts the recovered witness equals the input — proving the encoder is the byte-inverse of the coordinator's decoder.

## Transitional 4th parameter shape on `register_input`

```rust
pub async fn register_input(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    info: &InfoResponse,
    is_legacy_coordinator: bool,  // 17-02 TRANSITIONAL: 17-03 replaces with info.capabilities.is_legacy
) -> Result<InputRegState>
```

Call sites that need to change in Phase 17 17-03 (when wiring the real flag):

- `client/src/main.rs` (single site)
- `liquidity-bot/src/main.rs` (single site — bot always uses WIF / P2WPKH, so the v=2 envelope is currently safe against v1.4 coordinators)
- `tests/integration/full_round.rs` (6 sites — all pass `false` after 17-02; 17-03 may pivot some to `true` if the integration harness gains a legacy-coordinator simulation)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocker] Cross-target register_input signature drift**

Adding the 4th `is_legacy_coordinator: bool` parameter to `register_input` broke 8 downstream call sites (1 in `liquidity-bot/src/main.rs`; 6 in `tests/integration/full_round.rs`; 1 in `client/src/main.rs`). The plan only specified `main.rs`. All 8 sites were updated to pass `false` (preserving the v=2 envelope default — current behavior pre-17-02 was v=1; the v=2 default is the correct forward-compat choice for v1.4 coordinators). Each updated site carries an inline `17-02 TRANSITIONAL:` comment so 17-03 can grep-locate them for the real flag-wiring.

- Files: `liquidity-bot/src/main.rs`, `tests/integration/full_round.rs` (6 sites via `replace_all`)
- Verification: `cargo build --workspace` clean; `cargo test --test integration full_round` 8/8 PASS

**2. [Rule 2 — Critical functionality / Security] descriptors.txt gitignore**

`BdkClientWallet::generate(...)` writes a `descriptors.txt` file to cwd containing the BIP-39 mnemonic + master xprv (`mode 0600`). This is master-key material — must NEVER be committed. The new test surface in `client/tests/wallet_sign_roundtrip.rs` exercises `generate(...)` once per test fn × 3 script types × multiple test fns, so cwd at `cargo test -p client` (the crate root) accumulates `descriptors.txt`. Added `descriptors.txt` + `**/descriptors.txt` to `.gitignore` in the Task 1 commit to prevent accidental commit.

- File: `.gitignore`
- Verification: post-commit `git status` shows no `descriptors.txt` under any cwd

**3. [Rule 1 — Bug] Defensive test fixture (dummy_outpoint_is_well_formed)**

Initial implementation of the defensive `dummy_outpoint_is_well_formed` test attempted to parse a known bech32 signet wpkh address (`tb1q...`) via `bitcoin::Address::from_str`. Failed with `Base58(Decode(InvalidCharacterError { invalid: 48 }))` — the test was misusing the parser. Removed the address-parse assertion (the outpoint parts assertion alone is sufficient as a defensive guard). 6 plan-required tests (the 6 enumerated in `<behavior>`) were unaffected throughout. Documented as a test-fixture bug-fix inline.

- File: `client/tests/wallet_sign_roundtrip.rs`
- Verification: 7/7 tests pass after the fix

**4. [Rule 1 — Bug avoidance] dead_code marker on `script_pubkey`**

Deleting `generate_bip322_witness` (per CD-20) removed `script_pubkey`'s only in-bin consumer; external integration tests still reach it. Added `#[allow(dead_code)]` with an explicit doc-comment naming the deletion context. Symmetric with the same marker pattern Phase 17-01 established on `script_type` (`consumed by 17-02 sign dispatcher + 17-03 discovery check`).

- File: `client/src/wallet.rs`
- Verification: workspace builds clean

### Visibility escalation (CD-19)

Plan preferred `pub(crate) fn sign_bip322` but the external integration-test crate at `client/tests/wallet_sign_roundtrip.rs` only sees `pub` items. Escalated to `pub fn sign_bip322` (matches Phase 16-02's same Rule-3 escalation on `validate_ownership_proof_typed`). The `Bip322SignedProof` struct is also `pub` for the same reason. The `client::wallet` module already had `pub mod wallet` in `client/src/lib.rs`, so no further exports were needed.

### No deviations to plan-specified contracts

- D-64 struct fields landed verbatim.
- D-65 per-script dispatch body landed verbatim.
- D-67 (no manual P2TR fallback) honored.
- D-68 v1/v2 envelope branch landed; the transitional `is_legacy_coordinator: bool` parameter is named exactly as the plan specified.
- D-69 wire shape landed per the context-override (FULL PSBT, not bare `psbt::Input`).
- D-70 (witness_stack populated in BOTH envelopes) landed.
- D-80 CRIT-01 inline comment + CI grep gate both present.

## Wave-3 (17-03) inputs

17-03 will consume:

1. `register_input`'s transitional `is_legacy_coordinator: bool` parameter — replace with `info.capabilities.is_legacy` from the extended `CoordinatorInfo`. All transitional comments inline are tagged `17-02 TRANSITIONAL:` for grep-locatability.
2. The encoder's `final_script_sig` parameter — already wired; no further work in 17-03.
3. `Bip322SignedProof` struct — stable; 17-03 does not modify.

## Forward-only notes

The `is_legacy_coordinator: bool` parameter accepts `false` at all 8 current call sites. This is the safe default for v1.4 clients connecting to v1.4 coordinators (production wire path: v=2 envelope). 17-03's discovery layer will derive the actual flag from PKARR and may route some clients (legacy-coordinator interop) to `true`.

## Self-Check: PASSED

- ✅ `[ -f client/tests/wallet_sign_roundtrip.rs ]` FOUND
- ✅ Commit `a0e6937` (Task 1) — `git log --oneline | grep a0e6937` MATCH
- ✅ Commit `6c6759a` (Task 2) — `git log --oneline | grep 6c6759a` MATCH
- ✅ Commit `a46febb` (Task 3) — `git log --oneline | grep a46febb` MATCH
- ✅ `cargo build --workspace` exit 0
- ✅ `cargo test --test integration full_round` 8/8 PASS
- ✅ `cargo test -p client --test wallet_sign_roundtrip` 7/7 PASS
- ✅ `cargo test -p client --lib round::input::tests` 4/4 PASS
- ✅ `grep -c "CRIT-01" client/src/round/input.rs` returns 2 (≥ 1 required)
- ✅ `grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` returns 2 (unchanged)
- ✅ `grep -rn "fn generate_bip322_witness" client/` returns no matches (CD-20)
- ✅ `grep -c "crit-01-client-grep-check" .github/workflows/ci.yml` returns 1
- ✅ `cargo audit` clean (no new vulnerabilities, no warnings)
