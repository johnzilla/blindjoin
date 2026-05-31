# Requirements: v1.5 Audit-Readiness & Multi-Script Finish

**Defined:** 2026-05-31
**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

**Milestone goal:** Close the externally-visible v1.4 follow-throughs (production sign bodies for P2TR + P2SH-P2WPKH, accurate fees for mixed-script rounds) and ready the codebase for external security audit by publishing a scoped audit charter, refreshing `.cargo/audit.toml` rationales, and tightening the RSA SecretKey zeroization window so the charter can describe an explicitly-bounded mitigation rather than "best-effort".

## v1.5 Requirements

### Multi-Script Signing Finish (`BIP322-*`, continuing from v1.4)

- [ ] **BIP322-05**: `shared::bip322::p2tr::sign` ships a production body — BIP-341 Schnorr keypath sign over the canonical BIP-322 `to_sign` sighash via `bitcoin::secp256k1::Secp256k1::sign_schnorr_no_aux_rand` (or `sign_schnorr` if aux-rand is provably safe in this context); produces a 1-element witness with the 64-byte SIGHASH_DEFAULT signature; no `bdk_wallet` dependency in `shared/`; identical bytes to what `BdkClientWallet::sign_bip322` returns for the same key + message (round-trip cross-check test); replaces `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` at `shared/src/bip322/p2tr.rs`.

- [ ] **BIP322-06**: `shared::bip322::p2sh_p2wpkh::sign` ships a production body — BIP-143 ECDSA sign over the unwrapped P2WPKH redeem script's sighash; produces a 2-element witness `[der_sig+SIGHASH_ALL, compressed_pubkey]` AND the `final_script_sig = OP_PUSHBYTES_22 OP_0 <20-byte HASH160(pubkey)>` (the P2SH wrapper); cross-check: HASH160(redeemScript) matches the P2SH `script_pubkey` hash160; replaces `todo!` at `shared/src/bip322/p2sh_p2wpkh.rs`.

- [ ] **BIP322-07**: Remove `#[doc(hidden)] pub fn sign_simple_test_only` from `shared/src/bip322/mod.rs:302-314` and remove the per-script `pub(crate) fn sign_for_tests` helpers in `p2tr.rs` + `p2sh_p2wpkh.rs` (and the unused `p2wpkh.rs::sign_for_tests` mirror); all integration tests at `shared/tests/{per_script_vectors,bip322_cross_shape}.rs` call the real dispatcher `sign_simple`. Net effect: the `shared::bip322` public surface shrinks back to `verify_simple` + `sign_simple` + `detect_script_type` + `ScriptType` + `Bip322Error` (CRIT-01 dispatcher-only invariant strengthened — no test-only escape hatch).

### Mixed-Round Fee Accuracy (`FEE-*`)

- [ ] **FEE-01**: `coordinator/src/bitcoin/tx.rs` exposes `script_input_vbytes(ScriptType) -> u64` and `script_output_vbytes(ScriptType) -> u64` lookup functions (replacing the hardcoded `INPUT_WEIGHT_VBYTES = 68` and `OUTPUT_WEIGHT_VBYTES = 31`); values derived from BIP-141 worst-case (round UP, never DOWN, so coordinator does not underpay): P2WPKH input ~68 vB / output 31 vB; P2TR input ~57 vB / output 43 vB; P2SH-P2WPKH input ~91 vB / output 32 vB; values + derivation cited inline in the source.

- [ ] **FEE-02**: `ParticipantInput` carries `script_type: shared::bip322::ScriptType` (added field, CRIT-01-derived from `script_pubkey` at registration — not client-declared); `build_coinjoin_psbt` sums actual per-input weights via `script_input_vbytes(inp.script_type)`; output weight is derived from `bip_config.output_script_type` (passed through the call chain). Per-input fee_share variance is documented inline (currently uniform; if non-uniform fee_share is adopted later that's a separate REQ).

- [ ] **FEE-03**: Two regression tests in `coordinator/src/bitcoin/tx.rs::tests`:
  (a) `fee_share_p2wpkh_only_matches_v14_baseline` — a 3-participant uniform-P2WPKH round produces a `fee_share` value byte-equal to v1.4 (preserves the v1.3 cross-phase invariant from the fee-math angle);
  (b) `fee_share_mixed_script_differs_from_uniform_baseline` — a 3-participant round (1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH) produces a `fee_share` that differs from the uniform-P2WPKH baseline by ≥ 1 sat per participant (sanity check that the per-script branch actually fires, not just compiles).

### Audit Scope & Charter (`AUDIT-*`)

- [ ] **AUDIT-01**: Publish `docs/AUDIT-CHARTER.md` (committed in `main`, linked from `README.md` §Security Model). Structure: (1) in-scope modules with line/file references — `shared::bip322` dispatcher + per-script modules, `coordinator/src/bitcoin/utxo.rs::validate_utxo` (CRIT-01 cross-check), `coordinator/src/blind/rsa.rs` (RSA SecretKey lifecycle), `client/src/round/input.rs` (v2 OwnershipProof PSBT envelope construction); (2) threat models per module (V1.4-CRIT-01 spoofing, V1.4-CRIT-02 silent sighash regression, V1.4-MIN-02 uniform-script fingerprint, RSA Marvin Attack residual exposure); (3) cross-shape rejection properties (the 9 D-34 cases in `shared/tests/bip322_cross_shape.rs` enumerated explicitly); (4) v=2 `OwnershipProof` PSBT handling (the full-BIP-174 shape, Pitfall 1, `decode_psbt_input_witness` boundary); (5) RSA SecretKey zeroization window (post AUDIT-03 bounded form); (6) out-of-scope (Tor circuit-isolation correctness — relies on arti-client; PKARR DHT — relies on pkarr crate); (7) residual risks accepted with rationale; (8) glossary mapping CRIT-01, MIN-02, ADR Decision #N, etc. to plain audit language.

- [ ] **AUDIT-02**: Update `.cargo/audit.toml` ignore-rationale strings to (a) reference the relevant `docs/AUDIT-CHARTER.md` section anchor per ignored advisory; (b) the RUSTSEC-2023-0071 (rsa Marvin Attack) entry's rationale paragraph references the AUDIT-03 bounded-window mitigation (not "best-effort" anymore); (c) "Reviewed: 2026-05-26" header bumped to the v1.5-ship date; (d) any v1.4 transitive deps that have advisories opened since the v1.3 review get explicit ignore-or-fix decisions with rationale (no silent additions).

- [ ] **AUDIT-03**: Tighten RSA `BjSecretKey` zeroization window — wrap `BjSecretKey` in a coordinator-local `RoundSecretKey(BjSecretKey)` newtype with an explicit `Drop` impl that zeroes the in-process buffer at round end (or as soon as the last blind-sign call returns, whichever the state machine permits). Replaces the "best-effort" qualification in the D-07 comment at `coordinator/src/blind/rsa.rs:18-22` with an *explicitly bounded* window expressible as a Rust lifetime ("the secret key is live for the duration of `Round.state.signer: Option<RoundSecretKey>` and is dropped (and zeroed) on `Round.complete()` / `Round.abort()` / `Round.timeout()`"). Test: after `Round::complete`, the `RoundSecretKey` instance no longer exists and any in-process scan of the round-state allocation does not contain the original DER bytes (best-effort RAM scan test acceptable; the structural lifetime bound is the load-bearing claim).

## Future Requirements (v1.6+)

Deferred from v1.5 with reason, tracked for future scheduling.

### Carry-Forward from Earlier Milestones

- **CARRY-TOR-UAT**: Tor-mode verification harness — coordinator with `tor_mode=true` + test client opening ≥257 concurrent `.onion` streams. Closes Phase 8 HUMAN-UAT item 3 deferred from v1.2.
- **CARRY-REPAIR-01-PR**: REPAIR-01 PR observation closure. v1.3 shipped closed-local only; v1.4 did not include a public PR. Discharge expected at the next external PR moment (often a packaging or release-prep milestone).
- **B-03**: Dynamic fee estimation (mempool-aware polling + RBF strategy). Pre-mainnet requirement; orthogonal to v1.5's *accuracy* fixes (B-03 is about *responsiveness* to mempool changes, not per-script weight precision).
- **TEST-EXT-01/02/03**: Cross-implementation differential fixtures via `ACken2/bip322-js`; regtest on-chain anchor test (strongest available correctness gate against V1.4-CRIT-02); automated v1.3↔v1.4 backwards-compat integration matrix. v1.5 AUDIT-01 documents the gap; v1.6+ closes it.

### Out of v1.5 Scope but Not Anti-Features

- **P2WSH multisig BIP-322 support**: Multi-key sighash complexity; v1.4 stretch dropped for scope discipline; v1.5 deliberately keeps the per-script set at {P2WPKH, P2TR, P2SH-P2WPKH} to keep audit scope tight.
- **Mixed output script types per participant (Wasabi 2.0.3-style)**: Separate output-policy milestone; v1.5 outputs remain single-script-type per round (coordinator-configured via `bip.output_script_type`).
- **Per-input variable fee_share**: v1.5 keeps the uniform `fee_share = total_fee / N` formula even when input weights differ. Per-input fee_share is fairer but changes the wire protocol (clients must accept variable amounts in the PSBT) — separate milestone.

## Out of Scope

Explicitly excluded with reasoning to prevent re-adding later.

| Feature | Reason |
|---------|--------|
| Adding new input script types in v1.5 (P2PKH, bare P2SH, P2WSH multisig) | Scope discipline; v1.5 finishes what v1.4 started rather than expanding the set. P2PKH remains a privacy anti-pattern. |
| Custom RSA / blind-signature crypto | PROJECT.md constraint: no custom crypto. AUDIT-03 wraps the existing `blind-rsa-signatures` `SecretKey` in a Drop newtype — does NOT replace or fork the crate. |
| Replacing `bip322 = "=0.0.10"` with a fork or custom impl | Pinned per v1.4 ADR Decision #1; Phase 19 sign bodies use the same crate-adapter pattern as the verify path. |
| Modifying the v=2 OwnershipProof wire format | Locked at v1.4 (ADR Decision #3); Phase 19 sign body changes are byte-compatible with v1.4 wire output. |
| Per-input variable fee_share in v1.5 | Wire-protocol change; out of v1.5 scope. Coordinator's `fee_share = total_fee / N` formula remains uniform even though per-script weights differ. |
| Adding new advisory ignores to `.cargo/audit.toml` without a fix plan | AUDIT-02 requires every ignore to either (a) reference a charter section AND a planned remediation, or (b) be removed in favor of a dep upgrade. No silent ignores. |
| External penetration test inside v1.5 | v1.5 *prepares for* the audit (charter + bounded mitigations); the audit itself is a separate engagement, not a code deliverable. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BIP322-05 | Phase 19 | Not started |
| BIP322-06 | Phase 19 | Not started |
| BIP322-07 | Phase 19 | Not started |
| FEE-01 | Phase 20 | Not started |
| FEE-02 | Phase 20 | Not started |
| FEE-03 | Phase 20 | Not started |
| AUDIT-01 | Phase 21 | Not started |
| AUDIT-02 | Phase 21 | Not started |
| AUDIT-03 | Phase 21 | Not started |

**Coverage:**
- v1.5 requirements: 9 total
- Mapped to phases: 9 ✓
- Unmapped: 0

### Phase Coverage Summary

| Phase | Requirements Mapped | Notes |
|-------|---------------------|-------|
| Phase 19 — Multi-Script Signing Finish | BIP322-05, BIP322-06, BIP322-07 | Production sign bodies + removal of test-only dispatcher mirror; unblocks AUDIT-01 because the charter wants to describe production code |
| Phase 20 — Mixed-Round Fee Accuracy | FEE-01, FEE-02, FEE-03 | Per-script weight table + `ParticipantInput.script_type` plumbing + 2 regression tests (v1.3 invariant + mixed-script sanity) |
| Phase 21 — Audit Charter & Zeroization Tightening | AUDIT-01, AUDIT-02, AUDIT-03 | Depends on Phases 19+20 landing so the charter can describe production state; AUDIT-03 RSA newtype enables charter's "bounded window" prose |

---
*Requirements defined: 2026-05-31*
*Last updated: 2026-05-31 (v1.5 roadmap drafted: Phases 19-21 with full requirement traceability)*
