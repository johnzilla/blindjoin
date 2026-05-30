# Requirements: v1.4 BIP-322 Multi-Script Support

**Defined:** 2026-05-29
**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

**Milestone goal:** Broaden CoinJoin participation to P2TR and P2SH-P2WPKH UTXOs, eliminating the P2WPKH-only registration gate and making the "forward-compatible with all address types" claim in `PROJECT.md` match reality.

## v1.4 Requirements

### Cryptography & Verifier (BIP-322 dispatch + per-script paths)

- [x] **BIP322-01**: `shared` crate exposes `ScriptType` enum `{P2WPKH, P2TR, P2SH_P2WPKH}` with `detect_script_type(scriptPubKey) -> Result<ScriptType, UnsupportedScriptType>` (no fallthrough default arm; unknown patterns explicitly error)
- [x] **BIP322-02**: `shared::bip322::verify_simple` dispatches to per-script-type verifier impls — P2WPKH (BIP-143 ECDSA), P2TR (BIP-341 Schnorr keypath, accepting both SIGHASH_DEFAULT 64-byte and SIGHASH_ALL 65-byte sig forms), P2SH-P2WPKH (BIP-143 sighash over the unwrapped P2WPKH redeem script, with HASH160(redeemScript) cross-check)
- [x] **BIP322-03**: `shared::bip322::sign_simple` symmetric to `verify_simple` — produces a correct witness stack (and `final_script_sig` for P2SH-P2WPKH) for each script type from a signing key + message
- [ ] **BIP322-04**: Per-script property tests against the official BIP-322 `basic-test-vectors.json` (commit-SHA pinned from bitcoin/bips repo) — each verifier independently passes every vector in its class; cross-shape rejection tests for all 9 (script_pubkey × witness-shape) combinations confirm V1.4-CRIT-01 mitigation

### Coordinator Configuration & Advertisement

- [ ] **ADVERT-01**: `BipConfig` section in `coordinator.toml` (plus `BLINDJOIN__COORDINATOR__BIP__*` env-var overrides) with `allow_p2wpkh`, `allow_p2tr`, `allow_p2sh_p2wpkh` flags (default all `true`); validated at startup via `CoordinatorConfig::validate()` — fail-fast at boot, never panic-at-first-request
- [ ] **ADVERT-02**: Coordinator advertises `supported_script_types` over PKARR (bump record version `0.1.0` → `0.2.0`; CSV-encoded in TXT JSON to respect the 255-byte DNS limit and stay under the 220-byte warn threshold at `coordinator/src/discovery/pkarr_pub.rs:76`) and over `/round/info` (proper JSON array, no byte budget); `#[serde(default)]` on both ends enables v1.3↔v1.4 bidirectional deserialization (missing field interpreted as `["p2wpkh"]`)
- [ ] **ADVERT-03**: Coordinator derives `ScriptType` from `txout.script_pubkey` at validate-utxo time and cross-checks against the client-declared `script_type` (if any) on the `OwnershipProof` — mismatch rejects with `UnsupportedScriptType`; CRIT-01 invariant "script type derived from chain, not declared by client" is non-negotiable and load-bearing
- [x] **ADVERT-04**: `OwnershipProof` wire format extended to carry P2SH-P2WPKH `final_script_sig` (discuss-phase decides B1 tagged-enum vs B2 PSBT-input shape per SUMMARY.md Open Decision #3); roundtrip serialization test in `shared/` ships BEFORE either coordinator or client uses the new shape (v1.3 REPAIR-01 lesson #1)

### Client Wallet, Signing & Discovery

- [ ] **WALLET-01**: Client wallet supports three BIP descriptor templates — BIP-84 `wpkh(.../84'/...)` (P2WPKH, existing), BIP-86 `tr(.../86'/...)` (P2TR), BIP-49 `sh(wpkh(.../49'/...))` (P2SH-P2WPKH) — selected by `--type {p2wpkh|p2tr|p2sh-p2wpkh}` CLI flag at `generate-wallet`; defaults to `p2wpkh` for backwards compatibility
- [ ] **WALLET-02**: Client signs BIP-322 ownership proofs for all 3 script types via `shared::bip322::sign_simple` (Sprint-0-B Phase 14 spike verifies bdk_wallet 2.3 PSBT-sign path produces correct Taproot keypath witnesses; fallback to manual `secp256k1::Secp256k1::sign_schnorr` over direct sighash construction if bdk path unsuitable for P2TR — bdk_wallet/issues/150 means client signer is ours regardless)
- [ ] **WALLET-03**: Client reads `supported_script_types` from `/round/info` BEFORE opening a Tor circuit for input registration; rejects coordinator at discovery time on script-type mismatch with a clear error naming both the coordinator and the missing script type (V1.4-MOD-03 fail-fast UX)
- [ ] **WALLET-04**: Client detects pre-`0.2.0` coordinator (legacy PKARR record `"0.1.0"` or missing `supported_script_types` field on `/round/info`) and falls back to witness-only `OwnershipProof` wire format for P2WPKH-only rounds — v1.4 client interoperates with v1.3 coordinators (one-direction compat shim, the v1.4→v1.3 cell of the compat matrix)

### Test Infrastructure & Liquidity

- [ ] **INTEG-01**: Mixed-script E2E integration test on regtest — full CoinJoin round with at least 1 P2WPKH + 1 P2TR + 1 P2SH-P2WPKH input completes through INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST; reuses `BitcoindGuard` + `require_bitcoind!()` macro from v1.3 unchanged; v1.3 P2WPKH-only `full_round::*` tests remain green at this phase boundary (rollback safety net)
- [ ] **INTEG-02**: Liquidity bot generates UTXOs across all enabled script types (new config field `script_types: ["p2wpkh", "p2tr", "p2sh-p2wpkh"]`); rotates type per round so bot's UTXOs aren't a uniform fingerprint (V1.4-MIN-02 mitigation); per-round keychain derivation continues to prevent output-address clustering

## Future Requirements (v1.5+)

Deferred from v1.4 with reason, tracked for future scheduling.

### Correctness & Backwards-Compat (Deferred from v1.4)

- **TEST-EXT-01**: Cross-implementation differential test fixtures generated by `ACken2/bip322-js` (JS reference impl), checked into `tests/fixtures/bip322/` as static JSON files — catches sighash silent failures that own-code property tests can't surface
- **TEST-EXT-02**: Regtest on-chain anchor test — sign BIP-322 message + equivalent real spend with the same key + sighash routine, broadcast both; bitcoind acceptance of the real spend proves the sighash math is correct (strongest available correctness gate against V1.4-CRIT-02)
- **TEST-EXT-03**: Automated backwards-compat integration matrix — full grid (v1.3 client ↔ v1.4 coordinator, v1.4 client ↔ v1.3 coordinator, mixed v1.3/v1.4 participant rounds where v1.4 supports). WALLET-04 covers v1.4→v1.3 informally but no automated grid in v1.4.

### Carry-Forward from Earlier Milestones

- **CARRY-TOR-UAT**: Tor-mode verification harness — coordinator with `tor_mode=true` + test client opening ≥257 concurrent `.onion` streams. Closes Phase 8 HUMAN-UAT item 3 deferred from v1.2.
- **CARRY-REPAIR-01-PR**: REPAIR-01 PR observation closure. v1.3 shipped closed-local only. The v1.4 cut PR is the natural moment to discharge this but is NOT a v1.4 deliverable.
- **B-03**: Dynamic fee estimation (mempool-aware polling + RBF strategy). Pre-mainnet requirement.

### Out of v1.4 Scope but Not Anti-Features

- **P2WSH multisig BIP-322 support**: Multi-key sighash complexity; v1.4 stretch dropped for scope discipline. Could be a v1.5 phase if demand materializes.
- **Mixed output script types (Wasabi 2.0.3-style per-participant output choice)**: Separate output-policy milestone. v1.4 outputs remain single-script-type per round.

## Out of Scope

Explicitly excluded with reasoning to prevent re-adding later.

| Feature | Reason |
|---------|--------|
| Legacy P2PKH BIP-322 support | Privacy anti-pattern, low-anon-set marker — would degrade not broaden anon set |
| Bare P2SH (raw multisig in P2SH wrapper, not P2SH-wrapped P2WPKH) | Only single-sig P2SH-wrapped P2WPKH is accepted under the P2SH umbrella; raw P2SH multisig is outside ownership-proof scope |
| P2TR script-path spending for ownership proofs | Requires a script interpreter; no demonstrated demand; only key-path is supported |
| Per-script-type ban tracking | Leaks correlation across rounds; ban list stays uniform on `OutPoint` |
| Per-script-type rate limits | Defeats Tor-safe `GlobalKeyExtractor` (v1.2 hardening) — per-peer throttling is impossible on Tor anyway |
| Per-script-type round denominations | Fragments anonymity sets; one denomination remains the design invariant |
| Publicly advertised script-type breakdown of registrants per round | Exact partition vector leaks correlation; internal aggregate counters are operator-facing only |
| Custom RSA / blind-signature crypto | PROJECT.md constraint: no custom crypto. BIP-322 verification reuses `secp256k1` + `bitcoin::sighash` primitives only. |
| Adopting `bip322` crate without exact pin | Crate stalled at 0.0.10 for ~9 months; pre-1.0 SemVer means any release can break us. Discuss-phase decides adopt vs extend; either way pin is mandatory. |
| Tor-mode UAT harness inside v1.4 | Carry-forward from v1.2 Phase 8 HUMAN-UAT; out of v1.4 scope per user direction (pure compatibility milestone) |
| REPAIR-01 PR observation closure inside v1.4 | Closed-local in v1.3; full closure is a process step at v1.4 cut PR, not a v1.4 code deliverable |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BIP322-01 | Phase 15 | Complete |
| BIP322-02 | Phase 15 | Complete |
| BIP322-03 | Phase 15 | Complete |
| BIP322-04 | Phase 15 | Pending |
| ADVERT-01 | Phase 16 | Pending |
| ADVERT-02 | Phase 16 | Pending |
| ADVERT-03 | Phase 16 | Pending |
| ADVERT-04 | Phase 15 | Complete |
| WALLET-01 | Phase 17 | Pending |
| WALLET-02 | Phase 17 | Pending |
| WALLET-03 | Phase 17 | Pending |
| WALLET-04 | Phase 17 | Pending |
| INTEG-01 | Phase 18 | Pending |
| INTEG-02 | Phase 18 | Pending |

**Coverage:**
- v1.4 requirements: 14 total
- Mapped to phases: 14 ✓
- Unmapped: 0
- Phase 14 (Sprint-0 + Discuss-Phase Decisions) maps zero requirements — it is a gating spike/decision phase that produces an ADR resolving Open Decisions #1, #2, #3, #4 before Phase 15 plan-phase can derive tasks. This is intentional, not an orphan.

### Phase Coverage Summary

| Phase | Requirements Mapped | Notes |
|-------|---------------------|-------|
| Phase 14 — Sprint-0 Spikes + Discuss-Phase Decisions | (none) | Gating ADR-producing phase; resolves Open Decisions #1-#4 |
| Phase 15 — Shared Crate Multi-Script Contract | BIP322-01, BIP322-02, BIP322-03, BIP322-04, ADVERT-04 | shared/ contract + wire-format roundtrip test (REPAIR-01 lesson #1) |
| Phase 16 — Coordinator Integration & Advertisement | ADVERT-01, ADVERT-02, ADVERT-03 | Allowlist + dispatcher + PKARR/`/round/info` advertisement + CRIT-01 cross-check |
| Phase 17 — Client Multi-Script Wallet & Discovery | WALLET-01, WALLET-02, WALLET-03, WALLET-04 | Wallet descriptors + signing + fail-fast discovery + v1.4→v1.3 compat shim |
| Phase 18 — Mixed-Script E2E + Liquidity Bot | INTEG-01, INTEG-02 | Acceptance gate (mixed-script regtest round) + liquidity bot multi-script |

---
*Requirements defined: 2026-05-29*
*Last updated: 2026-05-29 after v1.4 roadmap drafted (Phases 14-18 with full requirement traceability)*
