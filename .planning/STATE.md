---
gsd_state_version: 1.0
milestone: v1.5
milestone_name: Audit-Readiness & Multi-Script Finish
status: milestone_complete
last_updated: 2026-05-31T23:51:33.134Z
last_activity: 2026-05-31
progress:
  total_phases: 3
  completed_phases: 3
  total_plans: 5
  completed_plans: 5
  percent: 100
stopped_at: Milestone complete (Phase 21 was final phase)
---

# Project State

## Current Position

Phase: 21
Plan: Not started
Status: Milestone complete
Last activity: 2026-05-31

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-31 — v1.5 scoped)

**Core value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.
**Current focus:** Milestone complete

## Milestone Map

- ✅ **Phase 19** — Multi-Script Signing Finish (BIP322-05, BIP322-06, BIP322-07) — execution complete, awaiting verification
- ✅ **Phase 20** — Mixed-Round Fee Accuracy (FEE-01, FEE-02, FEE-03) — execution complete, awaiting verification
- ✅ **Phase 21** — Audit Charter & Zeroization Tightening (AUDIT-01, AUDIT-02, AUDIT-03) — execution complete, awaiting verification

## Blockers

None at the roadmap level. Phase 19 unblocks Phase 21 (audit charter wants to describe production code, not `todo!()`).

## Deferred Items

Items acknowledged and deferred at v1.5 milestone close on 2026-05-31:

| Category | Item | Status | Note |
|----------|------|--------|------|
| uat_gap_scanner_false_positive | 21-HUMAN-UAT.md | resolved | All 3 items dispositioned (3 passed / 0 pending). Scanner over-reports resolved UAT files; no action needed. |
| quick_task_scanner_false_positive | 260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos | shipped 2026-05-26 | SUMMARY.md exists at .planning/quick/. Scanner flagged `missing` because frontmatter lacks a `status:` field; this is a quick-task template issue, not a real gap. |

## Carry-Forward Items (deferred from v1.4 → v1.6+ candidates)

- **CARRY-TOR-UAT** — Tor-mode verification harness (Phase 8 HUMAN-UAT item 3)
- **CARRY-REPAIR-01-PR** — v1.3 REPAIR-01 PR observation closure (v1.4 cut PR is the natural moment)
- **B-03** — Dynamic fee estimation (mempool-aware polling + RBF strategy; pre-mainnet requirement)
- **TEST-EXT-01/02/03** — Cross-implementation differential fixtures, on-chain anchor test, automated v1.3↔v1.4 backwards-compat matrix
- **P2WSH multisig BIP-322 support**
- **Mixed output script types** (Wasabi 2.0.3-style per-participant output choice)

## Accumulated Context (carried from v1.4)

### v1.4 cross-phase invariant (preserved in v1.5)

The v1.3 P2WPKH-only `full_round::*` integration tests (8 tests) MUST remain green at every v1.5 phase boundary. This is the rollback safety net inherited from v1.3 REPAIR-01 forensics — load-bearing for every phase's success criteria.

### Load-bearing v1.4 invariants v1.5 must preserve

- **V1.4-CRIT-01** dispatcher-only public surface on `shared::bip322` — production sign bodies in Phase 19 land at `pub(crate) fn sign` so callers still cannot reach `p2tr::sign` from outside the crate; only `verify_simple` and `sign_simple` are `pub`. Removing `sign_simple_test_only` in BIP322-07 strengthens this.
- **CRIT-01 cross-check** in `coordinator::validate_utxo` — derives `ScriptType` from on-chain `script_pubkey`, not from client declaration. Phase 20 fee estimator must use the same chain-derived `ScriptType` (not the client-declared one), preserving CRIT-01 invariant in the fee path.
- **CD-7 two-phase try-parse** on `OwnershipProof` — v1.3↔v1.4 wire compat preserved byte-exactly. Phase 19 changes to sign bodies must not alter wire output.
- **`bip322 = "=0.0.10"` exact pin** + `bip322-pin-check` CI gate — Phase 19 sign bodies route through the same crate-adapter pattern as the verify path.

### v1.5 design notes

- Phase 19 sign bodies SHOULD reuse the existing `#[cfg(test)] sign_for_tests` implementations almost verbatim — those helpers are already correct (they produce the witnesses the existing tests verify against the bip322 crate); the change is mostly "make them production, remove the test-only escape hatch."
- Phase 20's per-script weight table: use `bitcoin::Weight::from_witness_data_size` or hand-derived vbyte numbers from BIP-141 (P2WPKH input ~68 vB, P2TR input ~57.5 vB, P2SH-P2WPKH input ~91 vB; outputs P2WPKH 31 vB, P2TR 43 vB, P2SH-P2WPKH 32 vB). Rounding policy needs to be conservative (round UP) so the coordinator doesn't underpay fees on a mixed round.
- Phase 21's AUDIT-CHARTER.md should be structured as: in-scope modules (with line/file references), out-of-scope explicitly listed, threat models per module, residual risks accepted, and a glossary mapping audit terminology to project terms (CRIT-01, V1.4-MIN-02, etc.).

## Recent Plan Decisions

- **Plan 19-01** (2026-05-31): Production `sign` bodies shipped for P2TR (BIP-341 Schnorr keypath via `sign_schnorr_no_aux_rand`) and P2SH-P2WPKH (BIP-143 ECDSA over unwrapped P2WPKH redeem) in `shared::bip322`. D-111 spk↔key cross-check at the TOP of each new body — defense-in-depth for the Phase 21 audit charter's T-19-A mitigation claim.
- **Plan 19-01** (2026-05-31): Added `pub fn p2sh_p2wpkh_final_script_sig(pubkey) -> ScriptBuf` helper (D-109) — sibling to `sign_simple`, produces 23-byte BIP-141 nested-SegWit scriptSig. RESEARCH §Q3 corrected CONTEXT D-110 byte-count off-by-one (23, not 24).
- **Plan 19-01** (2026-05-31): Byte-equality parity with bdk_wallet 2.3 PROVEN for both P2TR (`sign_schnorr_no_aux_rand` on both sides) and P2SH-P2WPKH (ECDSA RFC 6979 deterministic). T-19-C empirical mitigation in `client/tests/wallet_sign_roundtrip.rs`.
- **Plan 19-01** (2026-05-31, Rule 3): `BdkClientWallet::from_descriptor` now supports single-key non-derivation descriptors via `Wallet::create_single` branch (gated on `/0/*)` template marker). RESEARCH §Q2 named the descriptor shape; the wrapper change to support it surfaced during parity-test execution.
- **Plan 19-02** (2026-05-31): BIP322-07 closed — `#[doc(hidden)] pub fn sign_simple_test_only` deleted from `shared::bip322::mod`; per-script `pub(crate) fn sign_for_tests` helpers deleted from `p2tr.rs`, `p2sh_p2wpkh.rs`, `p2wpkh.rs`. 4 test callsites (per_script_vectors import + 2 sign sites + multi_script_validate import + sign_witness helper) migrated to production `sign_simple` dispatcher; 2 `tests/integration/mod.rs` doc-comments refreshed (CD-39 default). The 9 cross-shape rejection cases now exercise production sign bodies.
- **Plan 19-02** (2026-05-31): `shared::bip322` public surface now shrunk to exactly 9 symbols (`ScriptType`, `Bip322Error`, `detect_script_type`, `verify_simple`, `sign_simple`, `p2sh_p2wpkh_final_script_sig`, `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`) — V1.4-CRIT-01 dispatcher-only invariant load-bearing at the type level with no test-only mirror. Phase 21 audit charter can now describe the surface without a "but also there's this `#[doc(hidden)]` thing" footnote.
- **Plan 20-01** (2026-05-31): P2TR vbyte fixed at **58** (round UP from raw 57.5 per STATE.md §v1.5 design notes), NOT 57 (ROADMAP SC#1 floor). Divergence documented inline in the `script_input_vbytes(ScriptType::P2tr) => 58` match-arm comment block AND in the unit-test name (`script_input_vbytes_p2tr_is_58_up_rounded`). STATE.md's rounding policy supersedes ROADMAP's planning approximation.
- **Plan 20-01** (2026-05-31): `script_input_vbytes` / `script_output_vbytes` located in `coordinator/src/bitcoin/tx.rs` (CD-40 fallback) co-located with `build_coinjoin_psbt`; `fee.rs` imports them via `use crate::bitcoin::tx::{script_input_vbytes, script_output_vbytes};`. Both are `pub const fn` over the `ScriptType` Copy enum (CD-41 default).
- **Plan 20-01** (2026-05-31): `BipConfig::allowed_set` returns `impl Iterator<Item = ScriptType> + '_` (CD-43 default); iteration order is implementation-defined — callers MUST NOT depend on order (use `supported()` for the alphabetical PKARR-canonical order). The `estimate_fee_share` use case is `max(...)` which is commutative.
- **Plan 20-01** (2026-05-31): FEE-03 baseline test hardcodes `266` with inline derivation comment per D-125 (NOT a `v14_formula()` helper — hardcoded number + comment is more durable for the audit-charter artifact Phase 21 cites).
- **Plan 20-01** (2026-05-31, Rule 1): `coordinator/src/round/blame.rs` test fixtures (3 `RegisteredInput` literals at lines 277/284/302) required `script_type` field refresh — plan enumeration listed signing.rs sites but missed blame.rs `detect_non_signers_*` tests. Auto-fixed; pattern is the same (P2WPKH default for tests that don't exercise fee math).
- **Plan 21-01** (2026-05-31): `RoundSecretKey(BjSecretKey)` newtype shipped in `coordinator/src/blind/rsa.rs` with empty-crypto `Drop` body emitting a PII-safe `tracing::debug!` event under target `blindjoin::audit`. Per 21-RESEARCH OQ1 the wrapped `rsa::RsaPrivateKey` already implements UNCONDITIONAL `Drop` + `ZeroizeOnDrop` (`rsa-0.9.10/src/key.rs:76-84`, no feature flag); DER-roundtrip and replace-with-dummy approaches are strictly worse (extra allocation / ~100ms keygen for identical outcome). The newtype's value is **lifetime expression**, not redundant in-place scrub.
- **Plan 21-01** (2026-05-31): `RoundStateInner.rsa_signer` refactored from bare `RsaBlindSigner` to `Option<RsaBlindSigner>` (D-128). The bounded-lifetime claim is now expressible as a Rust type signature at `state.rs:110` — load-bearing AUDIT-03 mitigation per REQUIREMENTS. New structural FSM test `round_secret_key_dropped_on_round_end` is the unconditional CI gate; sibling best-effort scrub `round_secret_key_buffer_overwritten_on_drop` is sanity-only and ignored on non-Linux per CD-50.
- **Plan 21-01** (2026-05-31): D-07 comment at `coordinator/src/blind/rsa.rs:18-22` rewritten per D-132 — old "best-effort only" qualifier removed (upstream `rsa 0.9.10` Drop chain runs deterministically). New comment cites the transitive `rsa::RsaPrivateKey` Drop chain by file:line, names `Option<RsaBlindSigner>` as the lifetime bound, names `transition_to(Phase::Idle)` as the trigger, and ends with charter anchor `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` — which Plan 21-02 materializes (the forward reference is intentional per the cross-phase invariant boundary).
- **Plan 21-02** (2026-05-31): `docs/AUDIT-CHARTER.md` shipped at 574 LOC with 8 H2 sections in REQUIREMENTS AUDIT-01 mandated order; hybrid voice (tables §1/§3/§6/§8, narrative §2/§4/§5/§7) per D-134. Closes AUDIT-01.
- **Plan 21-02** (2026-05-31): `.cargo/audit.toml` refreshed — RUSTSEC-2023-0071 rationale paragraph rewritten per D-139 + 21-RESEARCH OQ1 to name AUDIT-03 bounded-window mitigation explicitly (drops "destroys the key via `zeroize`" phrasing in favor of citing the transitive `<rsa::RsaPrivateKey as Drop>::drop` chain at `rsa-0.9.10/src/key.rs:76-82` bounded by `Option<RsaBlindSigner>` on `RoundStateInner`, verified by `round_secret_key_dropped_on_round_end`); 3 closing anchor lines appended; `Reviewed:` date bumped to 2026-05-31 (D-140). 3 ignore IDs preserved verbatim per OQ3. Closes AUDIT-02.
- **Plan 21-02** (2026-05-31, D-133a): all 3 artifacts (charter + audit.toml + README §Security Model callout) shipped in commit 92ae533 as a single atomic landing — prevents the anchor-drift window where audit.toml references charter headings that don't yet exist on the branch. The 4-way navigation loop (code ↔ charter ↔ audit.toml ↔ README) is intact at every commit boundary.
- **Plan 21-02** (2026-05-31, CD-49 slug-refinement): the §5 H2 heading is `## RSA Secret Key Zeroization Window` (with space between "Secret" and "Key") so GitHub's markdown auto-slugger produces `rsa-secret-key-zeroization-window` — matching the anchor cited by `rsa.rs` D-07 comment + `audit.toml` RUSTSEC-2023-0071 closing line. The §7 H3 sub-headings use colon form (`### Residual Risks: cargo-audit Advisories` etc.) NOT em-dash form so the slug is clean `residual-risks-cargo-audit-advisories` without a double-hyphen. GitHub's slugger does not split CamelCase nor collapse repeated hyphens; CD-49 grants this discretion explicitly.

## Performance Metrics

| Phase | Plan | Duration | Tasks | Files | Date |
|-------|------|----------|-------|-------|------|
| 19    | 01   | 11 min   | 4     | 5     | 2026-05-31 |
| 19    | 02   | 7 min    | 2     | 7     | 2026-05-31 |
| 20    | 01   | 17 min   | 3     | 9     | 2026-05-31 |
| 21    | 01   | 7 min    | 3     | 6     | 2026-05-31 |
| 21    | 02   | 11 min   | 3     | 3     | 2026-05-31 |

(v1.5 metrics will accumulate per-phase. Cumulative trends live in `RETROSPECTIVE.md`. v1.4 milestone-scoped metrics live in `milestones/v1.4-*` archives.)
