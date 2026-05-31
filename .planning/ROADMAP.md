# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- ✅ **v1.1 Security & Availability Hardening** — Phases 6-7 (shipped 2026-04-10)
- ✅ **v1.2 Production Readiness** — Phase 8 (shipped 2026-05-26)
- ✅ **v1.3 Test Infrastructure & Operational Hardening** — Phases 9-13 (shipped 2026-05-29)
- ✅ **v1.4 BIP-322 Multi-Script Support** — Phases 14-18 (shipped 2026-05-31)
- 🚧 **v1.5 Audit-Readiness & Multi-Script Finish** — Phases 19-21 (planning)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-04-09</summary>

- [x] Phase 1: Core Protocol (6/6 plans) — completed 2026-04-09
- [x] Phase 2: Blame & Hardening (3/3 plans) — completed 2026-04-09
- [x] Phase 3: Client CLI (2/2 plans) — completed 2026-04-09
- [x] Phase 4: Discovery & Deployment (3/3 plans) — completed 2026-04-09
- [x] Phase 5: Tor & Release (3/3 plans) — completed 2026-04-09

</details>

<details>
<summary>✅ v1.1 Security & Availability Hardening (Phases 6-7) — SHIPPED 2026-04-10</summary>

- [x] Phase 6: CI/CD Security Pipeline (1/1 plans) — completed 2026-04-10
- [x] Phase 7: Coordinator DoS Hardening (3/3 plans) — completed 2026-04-10

</details>

<details>
<summary>✅ v1.2 Production Readiness (Phase 8) — SHIPPED 2026-05-26</summary>

- [x] Phase 8: Public-endpoint hardening (4/4 plans) — completed 2026-05-26

</details>

<details>
<summary>✅ v1.3 Test Infrastructure & Operational Hardening (Phases 9-13) — SHIPPED 2026-05-29</summary>

- [x] Phase 9: CI integration-test reliability (5/5 plans) — completed 2026-05-27
- [x] Phase 10: full_round.rs decision + execution (2/2 plans; Task 3 carry-forward) — completed 2026-05-28
- [x] Phase 11: RSA SPKI handshake + unmute (carry-forward from 10) — closed via direct commits 2026-05-28
- [x] Phase 12: bdk_wallet 2.3 trust_witness_utxo (carry-forward from 11) — closed via direct commits 2026-05-28
- [x] Phase 13: Wire-format Witness encoding + unmute (carry-forward from 12) — closed via direct commits 2026-05-29

</details>

<details>
<summary>✅ v1.4 BIP-322 Multi-Script Support (Phases 14-18) — SHIPPED 2026-05-31</summary>

- [x] Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions (3/3 plans) — completed 2026-05-29
- [x] Phase 15: Shared Crate Multi-Script Contract (3/3 plans) — completed 2026-05-30
- [x] Phase 16: Coordinator Integration & Advertisement (3/3 plans) — completed 2026-05-30
- [x] Phase 17: Client Multi-Script Wallet & Discovery (3/3 plans) — completed 2026-05-30
- [x] Phase 18: Mixed-Script E2E + Liquidity Bot (3/3 plans) — completed 2026-05-31

</details>

### 🚧 v1.5 Audit-Readiness & Multi-Script Finish (Phases 19-21)

- [x] **Phase 19: Multi-Script Signing Finish** — Ship production `sign` bodies for `shared::bip322::{p2tr,p2sh_p2wpkh}::sign` (replacing the `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")`) and remove the `#[doc(hidden)] sign_simple_test_only` mirror + per-script `sign_for_tests` helpers, strengthening the V1.4-CRIT-01 dispatcher-only public surface. (planning) (completed 2026-05-31)
- [x] **Phase 20: Mixed-Round Fee Accuracy** — Replace the hardcoded `INPUT_WEIGHT_VBYTES = 68` / `OUTPUT_WEIGHT_VBYTES = 31` in `coordinator/src/bitcoin/tx.rs` with a per-script weight table; thread `ScriptType` through `ParticipantInput`; coordinator passes `bip_config.output_script_type` to fee math; add v1.4-fee-byte-equality test + mixed-script fee-divergence sanity test. (planning) (completed 2026-05-31)
- [ ] **Phase 21: Audit Charter & Zeroization Tightening** — Publish `docs/AUDIT-CHARTER.md` scoping external audit (BIP-322 dispatcher + per-script modules, 9 cross-shape rejection properties, v=2 `OwnershipProof` PSBT handling, RSA SecretKey zeroization window); refresh `.cargo/audit.toml` rationale strings to reference charter sections; wrap `BjSecretKey` in a `RoundSecretKey` newtype with explicit `Drop` so the zeroization window is *explicitly bounded* via Rust lifetime, not "best-effort". Depends on Phases 19+20 landing so the charter can describe production state. (planning)

## Phase Details

### Phase 19: Multi-Script Signing Finish

**Goal**: `shared::bip322` ships production `sign` bodies for all 3 script types via the `pub(crate) fn sign` surface, and the test-only escape hatches (`sign_simple_test_only` + per-script `sign_for_tests` helpers) are gone — V1.4-CRIT-01 dispatcher-only invariant is now load-bearing at the type level with no holes.
**Depends on**: v1.4 ship (Phase 18 closed; `bip322 = "=0.0.10"` adapter at `shared/src/bip322/mod.rs::verify_via_bip322_crate` is the reference pattern for the symmetric sign path).
**Requirements**: BIP322-05, BIP322-06, BIP322-07
**Plans:** 2/2 plans complete
Plans:
**Wave 1**

- [x] 19-01-PLAN.md — Ship production p2tr::sign + p2sh_p2wpkh::sign bodies, D-111 spk↔key cross-check, p2sh_p2wpkh_final_script_sig helper + 3 unit tests, 2 byte-equality parity tests in client/tests/wallet_sign_roundtrip.rs (closes BIP322-05 + BIP322-06) — **shipped 2026-05-31** (4 commits: `0b64e41`, `ffcfb9d`, `2d8c7f6`, `d1425fd`)

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 19-02-PLAN.md — Delete sign_simple_test_only + per-script sign_for_tests helpers, migrate 6 callsites to sign_simple, refresh doc-comments, grep-verify zero residual references (closes BIP322-07)

**Success Criteria** (what must be TRUE):

  1. `shared/src/bip322/p2tr.rs::sign` returns a 1-element witness with a valid 64-byte BIP-341 Schnorr SIGHASH_DEFAULT signature over the canonical BIP-322 `to_sign` sighash; `secp256k1::verify_schnorr(sig, sighash, x_only_pubkey)` returns `Ok(())` on the output of `sign(spk, key, message)`; identical witness bytes to what `BdkClientWallet::sign_bip322` produces for the same `(key, message)` (parity test).
  2. `shared/src/bip322/p2sh_p2wpkh.rs::sign` returns a 2-element witness `[der_sig+SIGHASH_ALL, compressed_pubkey]` AND a `final_script_sig = OP_PUSHBYTES_22 OP_0 <20-byte HASH160(pubkey)>`; HASH160(redeemScript) equals the P2SH SPK hash160; `shared::bip322::verify_simple(ScriptType::P2shP2wpkh, spk, witness, message, network)` returns `Ok(())` on the output of `sign(spk, key, message)`.
  3. `#[doc(hidden)] pub fn sign_simple_test_only` at `shared/src/bip322/mod.rs:302-314` is **deleted** AND no `pub(crate) fn sign_for_tests` remains in `p2wpkh.rs` / `p2tr.rs` / `p2sh_p2wpkh.rs`; integration tests at `shared/tests/{per_script_vectors,bip322_cross_shape}.rs` call only `verify_simple` / `sign_simple` / `detect_script_type` (the real dispatcher).
  4. `cargo test -p shared` 31/31 + the 7 + 9 + 6 integration tests stay green at the plan boundary; `cargo test --test integration full_round` 8/8 green (v1.3 cross-phase invariant unchanged — Phase 19 is shared/-internal).
  5. `cargo clippy --workspace --all-targets -- -D warnings` clean; `bip322-pin-check` + `crit-01-grep-check` + `crit-01-client-grep-check` CI jobs all green.

### Phase 20: Mixed-Round Fee Accuracy

**Goal**: A mixed-script CoinJoin round (heterogeneous P2WPKH + P2TR + P2SH-P2WPKH inputs) charges a `fee_share` that reflects actual per-input witness weights rather than the v1.4 P2WPKH-only approximation; v1.4 P2WPKH-only round fee math is byte-preserved.
**Depends on**: v1.4 Phase 16 (`BipConfig.output_script_type` exists on `CoordinatorConfig`) and Phase 18 (mixed-script E2E test exists at `tests/integration/mixed_script_e2e.rs`).
**Requirements**: FEE-01, FEE-02, FEE-03
**Plans:** 1/1 plans complete
Plans:
**Wave 1**

- [x] 20-01-PLAN.md — Per-script vbyte table (`script_input_vbytes` / `script_output_vbytes` const fns in `coordinator/src/bitcoin/tx.rs`, deletes the legacy `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES` consts; P2TR = 58 per STATE.md round-UP, divergence from ROADMAP "57" documented inline); plumb `script_type` through `UtxoDetails → RegisteredInput → ParticipantInput` (single derivation point at `utxo.rs:99`, CRIT-01 preserved with zero new `detect_script_type` call sites); rewrite `build_coinjoin_psbt` to per-input weight sum + `output_script_type` param (WR-04 byte-identical PSBTs from both call sites); refactor `estimate_fee_share(&BipConfig, n, fee_rate)` to worst-case-across-allowed-set + add `BipConfig::allowed_set()` iterator helper; 6 vbyte-pin unit tests + 2 FEE-03 regression tests (P2WPKH-only baseline = 266 byte-exact, mixed-script delta = 9 sats ≥ 1) (closes FEE-01 + FEE-02 + FEE-03).

**Success Criteria** (what must be TRUE):

  1. `coordinator/src/bitcoin/tx.rs` exposes `pub fn script_input_vbytes(ScriptType) -> u64` and `pub fn script_output_vbytes(ScriptType) -> u64` returning conservative-rounded-UP BIP-141 weights (P2WPKH in/out 68/31; P2TR 57/43; P2SH-P2WPKH 91/32); the hardcoded `INPUT_WEIGHT_VBYTES = 68` and `OUTPUT_WEIGHT_VBYTES = 31` consts at `tx.rs:11,13` are gone.
  2. `ParticipantInput` gains a `script_type: shared::bip322::ScriptType` field; `build_coinjoin_psbt` sums per-input weights via `script_input_vbytes(inp.script_type)` and uses `script_output_vbytes(output_script_type)` for the denomination + change outputs; the `script_type` value is set by the **coordinator** at `validate_utxo` time via `detect_script_type(txout.script_pubkey)` (CRIT-01 invariant preserved into the fee path — never client-declared).
  3. Regression test `fee_share_p2wpkh_only_matches_v14_baseline` in `coordinator/src/bitcoin/tx.rs::tests` constructs a 3-participant uniform-P2WPKH round and asserts `fee_share` matches the pre-Phase-20 value byte-equal (the v1.3 cross-phase invariant from the fee-math angle).
  4. Regression test `fee_share_mixed_script_differs_from_uniform_baseline` constructs a 3-participant 1×P2WPKH + 1×P2TR + 1×P2SH-P2WPKH round and asserts `fee_share` ≠ the uniform-P2WPKH baseline by ≥ 1 sat per participant (sanity check that the per-script branch fires, not just compiles).
  5. v1.3 `full_round::*` 8/8 green AND the v1.4 `mixed_script_e2e_three_clients_broadcast` test in `tests/integration/mixed_script_e2e.rs` still passes with the new fee math — broadcast txid still observable in mempool, output amounts adjust to the new (more accurate) fee deduction.

### Phase 21: Audit Charter & Zeroization Tightening

**Goal**: `docs/AUDIT-CHARTER.md` exists and an external auditor can read it cold, identify exactly which files and properties are in scope, and start reviewing without asking the project team for clarification; `.cargo/audit.toml` rationale strings reference the charter; the RSA SecretKey lifetime is *explicitly bounded* via a `RoundSecretKey` newtype so the charter can describe a structurally-enforced mitigation rather than "best-effort".
**Depends on**: Phase 19 (production sign bodies — charter §"BIP-322 dispatcher + per-script modules" wants to point at production code, not `todo!()`) and Phase 20 (`tx.rs` per-script weight table — charter §"v=2 OwnershipProof PSBT handling" wants to describe the *complete* multi-script verification + fee path).
**Requirements**: AUDIT-01, AUDIT-02, AUDIT-03
**Success Criteria** (what must be TRUE):

  1. `docs/AUDIT-CHARTER.md` exists, committed in `main`, linked from `README.md` §Security Model; contains 8 sections (in-scope modules with line/file references, threat models per module, cross-shape rejection properties enumerated, v=2 PSBT handling boundary, RSA zeroization window in its bounded form, out-of-scope with rationale, residual risks accepted with rationale, glossary of project terms → audit language).
  2. `.cargo/audit.toml` updated: each `ignore = [...]` entry's rationale string references a `docs/AUDIT-CHARTER.md#section-anchor`; the RUSTSEC-2023-0071 (rsa Marvin Attack) entry's rationale paragraph references the AUDIT-03 bounded-window mitigation by name (not "best-effort" anymore); header "Reviewed: YYYY-MM-DD" bumped to v1.5 ship date; any new transitive advisories opened since v1.3 review get explicit ignore-or-fix decisions with rationale.
  3. `coordinator/src/blind/rsa.rs` introduces a `RoundSecretKey(BjSecretKey)` newtype wrapper with an explicit `Drop` impl that zeroes the in-process buffer; the round state struct holds `Option<RoundSecretKey>` and explicitly sets it to `None` (triggering Drop) on `Round.complete()` / `Round.abort()` / `Round.timeout()`; the "best-effort" qualification in the D-07 comment at `rsa.rs:18-22` is rewritten as a bounded statement ("zeroed on round end via `RoundSecretKey::drop`; lifetime bound to `Round.state.signer: Option<RoundSecretKey>`").
  4. Test `round_secret_key_zeroes_on_drop` in `coordinator/src/blind/rsa.rs::tests` constructs a `RoundSecretKey`, drops it, and asserts the underlying buffer no longer matches the original DER bytes (best-effort RAM scan acceptable per the existing `blind-rsa-signatures 0.17.x` limitation — the structural lifetime bound is the load-bearing claim).
  5. v1.3 `full_round::*` 8/8 + v1.4 `mixed_script_e2e_three_clients_broadcast` + Phase 20 fee tests all green at the plan boundary; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo audit` returns 0 vulnerabilities with the refreshed `audit.toml`.

**Plans:** 1/2 plans executed

Plans:
**Wave 1**

- [x] 21-01-PLAN.md — RoundSecretKey newtype + Option<RsaBlindSigner> refactor + 2 tests (structural FSM + best-effort RAM scan) + D-07 comment rewrite + 4 call-site fix-ups (closes AUDIT-03)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 21-02-PLAN.md — docs/AUDIT-CHARTER.md (8 sections) + .cargo/audit.toml charter-anchor refresh + RUSTSEC-2023-0071 rewrite + README §Security Model callout — single atomic commit per D-133a (closes AUDIT-01 + AUDIT-02)


## Cross-Phase Invariant (v1.5)

> **At every v1.5 phase boundary, the v1.3 P2WPKH-only `full_round::*` integration tests MUST remain green AND the v1.4 `mixed_script_e2e_three_clients_broadcast` test MUST remain green.** This extends the v1.4 rollback safety net to include the v1.4 multi-script E2E gate — v1.5 is finishing v1.4, so v1.4's acceptance gate becomes v1.5's invariant. Per the v1.4 lesson: if a phase breaks either suite, abandon the structured plan and pivot to `/gsd:debug` (REPAIR-01 lesson #4).

## Carry-Forward (explicitly NOT v1.5)

These items appear in `REQUIREMENTS.md` Future Requirements and are NOT mapped to any v1.5 phase. They are tracked for v1.6+ scheduling:

- **CARRY-TOR-UAT**: Tor-mode verification harness (Phase 8 HUMAN-UAT item 3).
- **CARRY-REPAIR-01-PR**: v1.3 REPAIR-01 PR observation closure (discharged at the next external PR moment).
- **B-03**: Dynamic fee estimation (mempool-aware polling + RBF) — orthogonal to v1.5's per-script *accuracy* fixes.
- **TEST-EXT-01/02/03**: Cross-implementation differential fixtures, on-chain anchor test, automated backwards-compat matrix.
- **P2WSH multisig BIP-322 support**.
- **Mixed output script types** (Wasabi 2.0.3-style per-participant output choice).
- **Per-input variable fee_share** (uniform `fee_share = total_fee / N` preserved in v1.5; per-input variance would change the wire protocol).

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Protocol | v1.0 | 6/6 | Complete | 2026-04-09 |
| 2. Blame & Hardening | v1.0 | 3/3 | Complete | 2026-04-09 |
| 3. Client CLI | v1.0 | 2/2 | Complete | 2026-04-09 |
| 4. Discovery & Deployment | v1.0 | 3/3 | Complete | 2026-04-09 |
| 5. Tor & Release | v1.0 | 3/3 | Complete | 2026-04-09 |
| 6. CI/CD Security Pipeline | v1.1 | 1/1 | Complete | 2026-04-10 |
| 7. Coordinator DoS Hardening | v1.1 | 3/3 | Complete | 2026-04-10 |
| 8. Public-endpoint hardening | v1.2 | 4/4 | Complete | 2026-05-26 |
| 9. CI integration-test reliability | v1.3 | 5/5 | Complete | 2026-05-27 |
| 10. full_round.rs decision + execution | v1.3 | 2/2 | Complete | 2026-05-28 |
| 11-13. REPAIR-01 carry-forward (shipped as direct commits) | v1.3 | n/a | Closed-local | 2026-05-29 |
| 14. Sprint-0 Spikes + Discuss-Phase Decisions | v1.4 | 3/3 | Complete | 2026-05-29 |
| 15. Shared Crate Multi-Script Contract | v1.4 | 3/3 | Complete | 2026-05-30 |
| 16. Coordinator Integration & Advertisement | v1.4 | 3/3 | Complete | 2026-05-30 |
| 17. Client Multi-Script Wallet & Discovery | v1.4 | 3/3 | Complete | 2026-05-30 |
| 18. Mixed-Script E2E + Liquidity Bot | v1.4 | 3/3 | Complete | 2026-05-31 |
| 19. Multi-Script Signing Finish | v1.5 | 2/2 | Complete    | 2026-05-31 |
| 20. Mixed-Round Fee Accuracy | v1.5 | 1/1 | Complete    | 2026-05-31 |
| 21. Audit Charter & Zeroization Tightening | v1.5 | 1/2 | In Progress|  |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
Full v1.1 details: `.planning/milestones/v1.1-ROADMAP.md`
Full v1.2 details: `.planning/milestones/v1.2-ROADMAP.md`
Full v1.3 details: `.planning/milestones/v1.3-ROADMAP.md`
Full v1.4 details: `.planning/milestones/v1.4-ROADMAP.md`
