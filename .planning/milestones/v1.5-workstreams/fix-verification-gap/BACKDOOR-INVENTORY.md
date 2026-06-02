---
workstream: fix-verification-gap
deliverable: 2-of-3
audit_date: 2026-05-26
auditor: Explore (read-only investigation) + orchestrator synthesis
scope: tests/ tree + #[cfg(test)] blocks in coordinator/src/, shared/src/, client/src/, liquidity-bot/src/
---

# Backdoor Inventory

Audit of test-only helpers that construct production state types in the blindjoin codebase. Conducted in response to the v1.1 round-bootstrap regression, which shipped masked by a `build_input_reg_round_state` helper that constructed `RoundStateInner` without going through any production code path.

## Executive summary

| Severity | Count | Status |
|---|---|---|
| HIGH | **0** | ✓ |
| MEDIUM | **2** | TODO comments added; cleanup recommended as backlog item |
| LOW | **7** | Acceptable fixtures — no action needed |

**The original `build_input_reg_round_state` backdoor was the only HIGH-class case in the codebase.** It was deleted in commit `d342359` (`refactor(test): replace build_input_reg_round_state backdoor with start_round`). Two MEDIUM cases remain — both inside `#[cfg(test)]` modules in `coordinator/src/round/`. Neither blocks production correctness today, but both should be migrated to use the state machine to prevent the same anti-pattern from spreading.

## Methodology

Searched ~150 files. Patterns:
- `grep -rn 'fn build_\|fn make_\|fn create_\|fn new_test\|fn fake_\|fn mock_\|fn stub_\|fn dummy_' tests/`
- Same grep applied to `#[cfg(test)]` modules in `coordinator/src/`, `shared/src/`, `client/src/`, `liquidity-bot/src/`.
- Manual inspection of any function in test code returning a coordinator/round/state type.

**Classification rubric** (applied uniformly):
- **HIGH:** Constructs production state that has NO production code path to reach the same state. Same class as the round-bootstrap bug.
- **MEDIUM:** Wraps production code but bypasses a step production always performs (validation, ordering invariant, side effect).
- **LOW:** Fixture/utility that wraps production builders with sensible test defaults. Production-equivalent.

## Findings table

| # | File:line | Function | Constructs | Production analog | Risk | Rationale |
|---|---|---|---|---|---|---|
| 1 | [coordinator/src/round/input_reg.rs:111](coordinator/src/round/input_reg.rs:111) | `make_input_reg_state` | `RoundState` + `RoundStateInner` (struct literal) | `start_round` ([coordinator/src/round/manager.rs](coordinator/src/round/manager.rs)) | **MEDIUM** | Builds `RoundStateInner` via struct literal, then calls real `transition_to(Phase::InputReg)`. Uses real `RsaBlindSigner::generate()`, so RSA key is valid. The only divergence from production is the inner-state construction. |
| 2 | [coordinator/src/round/signing.rs:279](coordinator/src/round/signing.rs:279) | `make_signing_state` | `RoundState` + `RoundStateInner` (struct literal) | `start_round` + state-machine transitions to Signing | **MEDIUM** | Directly assigns `state.phase = Phase::Signing` (line 281) — bypasses `transition_to` validator entirely. Uses `vec![0u8; 1]` placeholder for `rsa_signing_key`. Worst of the MEDIUM pair. Used by 4 sign-handler tests. |
| 3 | [coordinator/src/round/output_reg.rs:102](coordinator/src/round/output_reg.rs:102) | `make_valid_token_sig` | RSA blind signature (production flow) | Implicit (same code clients use) | LOW | Wraps production `blind_sign()` + `finalize()`. No bypass. |
| 4 | [coordinator/src/bitcoin/tx.rs:136](coordinator/src/bitcoin/tx.rs:136) | `dummy_outpoint` | `OutPoint` test fixture | n/a — production never needs a "dummy" OutPoint | LOW | Pure fixture for PSBT tests. No state machine. |
| 5 | [coordinator/src/bitcoin/tx.rs:150](coordinator/src/bitcoin/tx.rs:150) | `make_inputs` | `Vec<ParticipantInput>` | n/a — production constructs these from API request bodies | LOW | PSBT test setup fixture. |
| 6 | [coordinator/src/bitcoin/tx.rs:159](coordinator/src/bitcoin/tx.rs:159) | `make_outputs` | `Vec<ParticipantOutput>` | n/a — production constructs these from API request bodies | LOW | PSBT test setup fixture. |
| 7 | [coordinator/src/bitcoin/utxo.rs:189](coordinator/src/bitcoin/utxo.rs:189) | `make_p2wpkh_and_witness` | BIP-322 witness | `build_bip322_to_spend` + `build_bip322_to_sign` ([shared/src/bip322.rs](shared/src/bip322.rs)) | LOW | Calls production builders to simulate client signing. |
| 8 | [client/src/round/sign.rs:94](client/src/round/sign.rs:94) | `make_state` | Client-side `InputRegState` | n/a — client-side struct, not coordinator state | LOW | CLI sign-test fixture. No server-side risk. |
| 9 | [liquidity-bot/src/strategy.rs:44](liquidity-bot/src/strategy.rs:44) | `make_info` | `InfoResponse` DTO | n/a — DTO wraps a deserialized API response | LOW | Pure fixture; no round state, no coordinator logic. |

## MEDIUM details

### Finding #1 — `make_input_reg_state` (input_reg.rs:111)

**Test call sites:**
- [coordinator/src/round/input_reg.rs:144](coordinator/src/round/input_reg.rs:144) (`register_input_is_sync_and_succeeds`)

**Why MEDIUM, not HIGH:** It calls `RsaBlindSigner::generate()` (production) and `transition_to(Phase::InputReg)` (production validator). Only the `RoundStateInner` construction is direct.

**Recommended fix:** Replace the struct-literal block (lines 122-131) with a call to `crate::round::manager::start_round(&mut state)?`. The post-fix `start_round` (commit `9fac638`) does exactly what this helper does, with the same RSA generation step. Net delta: ~12 lines removed from the test helper, one new line calling production code.

### Finding #2 — `make_signing_state` (signing.rs:279)

**Test call sites:**
- [coordinator/src/round/signing.rs:312](coordinator/src/round/signing.rs:312) (`test_process_sign_invalid_session_token`)
- [coordinator/src/round/signing.rs:336](coordinator/src/round/signing.rs:336) (`test_process_sign_valid`)
- [coordinator/src/round/signing.rs:374](coordinator/src/round/signing.rs:374) (`test_process_sign_already_signed`)
- [coordinator/src/round/signing.rs:418](coordinator/src/round/signing.rs:418) (`test_process_sign_coordinator_error`)

**Why MEDIUM** (not HIGH): production CAN reach `phase = Signing` via the state machine; this helper just gets there by a shorter route. But two red flags:
1. **Direct phase mutation** (`state.phase = Phase::Signing`, line 281) bypasses the `transition_to` validator. Any invariant added to that validator in the future will be silently bypassed by these 4 tests.
2. **Placeholder RSA signing key** (`vec![0u8; 1]`, line 284) — if the sign handler ever starts validating the key, these tests would not catch the regression.

**Recommended fix:** Build state through `start_round` → simulate input registration → call `transition_to(Phase::OutputReg)` → call `transition_to(Phase::Signing)`. Longer setup, but exercises the full production sequence. Or extract a `pub(crate) fn advance_to_signing_for_test(&mut state)` helper in `coordinator::round::manager` that does this and is itself tested.

## Follow-on work

Suggested backlog item (does not need its own workstream — fold into a future "test hygiene" pass):

> **chore(tests): migrate `make_input_reg_state` and `make_signing_state` to use production state-machine transitions.** Both currently construct `RoundStateInner` via struct literal — see [`.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md`](.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md). Estimate: ~30 min refactor + re-run the 5 affected tests.

## Patterns to avoid in future tests

1. **Never construct `RoundStateInner` via struct literal.** Use `start_round` to bootstrap Idle→InputReg; use `transition_to` for subsequent phases. Inline field assignment hides production invariants.
2. **Never assign `state.phase` directly.** Always go through `transition_to`. Direct assignment bypasses the validator.
3. **Avoid placeholder values for cryptographic material in state.** If a test needs Signing-phase state, generate a real RSA key (cheap with `RsaBlindSigner::generate()`). Placeholders mask future regressions where the key is actually validated.
4. **Wrap production functions, don't reimplement them.** `make_valid_token_sig` is a good example — it calls production `blind_sign` + `finalize`. `make_signing_state` is the anti-example.
5. **For integration tests, invoke the production startup path** (`coordinator::run`, `start_round`). Don't pre-populate state — that's exactly how the v1.1 bootstrap regression hid.

## Verification-template heuristic (Deliverable #3 input)

The findings here support a concrete heuristic for the verification-template work (Deliverable #3 of this workstream):

> **Rule:** When verifying a phase, if you encounter test setup code that constructs a production type, you MUST either:
> - Cite the equivalent production code path that constructs the same state, or
> - Flag the test setup as NOT VERIFIED and route to a phase that adds the missing production path.
>
> "Accepted because it's test-only" is not a valid disposition.

Two of nine candidates in this audit fail that rule today — both for the same root cause (legacy unit-test helpers predating `start_round`). The fact that even one MEDIUM slipped through suggests the heuristic would have value as a tooling/checklist constraint.
