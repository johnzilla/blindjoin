# Roadmap: blindjoin

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-04-09)
- ✅ **v1.1 Security & Availability Hardening** — Phases 6-7 (shipped 2026-04-10)
- ✅ **v1.2 Production Readiness** — Phase 8 (shipped 2026-05-26)
- ✅ **v1.3 Test Infrastructure & Operational Hardening** — Phases 9-13 (shipped 2026-05-29)
- 🚧 **v1.4 BIP-322 Multi-Script Support** — Phases 14-18 (planning)

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

### 🚧 v1.4 BIP-322 Multi-Script Support (Phases 14-18)

- [x] **Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions** — Two timeboxed spikes (`bip322` crate pin verification, bdk_wallet 2.3 P2TR sign PoC) produced a GO/PASS ADR resolving Open Decisions #1 (ADOPT `bip322 = "=0.0.10"`), #2 (mixed rounds), #3 (B2 PSBT-input wire format), #4 (bdk path for P2TR sign). ✅ completed 2026-05-29
- [ ] **Phase 15: Shared Crate Multi-Script Contract** — `shared/` exposes `ScriptType` dispatch, per-type BIP-322 sign/verify, extended `OwnershipProof` + `InfoResponse` wire types, and per-script-type property tests against the official BIP-322 vectors.
- [ ] **Phase 16: Coordinator Integration & Advertisement** — Replace the `is_p2wpkh()` gate with a config-driven allowlist + dispatcher; advertise `supported_script_types` over PKARR and `/round/info`; cross-check declared script type against on-chain `scriptPubKey` (CRIT-01).
- [ ] **Phase 17: Client Multi-Script Wallet & Discovery** — Client wallet supports BIP-84 / BIP-86 / BIP-49 descriptors, signs ownership proofs for all three types, and rejects mismatched coordinators at discovery before opening a Tor circuit; ships v1.4→v1.3 compatibility shim.
- [ ] **Phase 18: Mixed-Script E2E + Liquidity Bot** — Liquidity bot generates UTXOs across all enabled script types and rotates per round; mixed-script regtest integration test (1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH) completes a full CoinJoin round through BROADCAST.

## Phase Details

### Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions
**Goal**: Resolve every load-bearing v1.4 decision before any production code is written, so downstream phases have unambiguous specifications.
**Depends on**: v1.3 ship (Phase 13 closed-local; baseline `full_round::*` tests green on pinned bitcoind v30.2).
**Requirements**: (none — gating spike/decision phase; produces an ADR artifact that unblocks Phases 15-18)
**Success Criteria** (what must be TRUE):
  1. `cargo tree -p bip322` output is checked into `.planning/research/sprint-0-A.md` and shows whether `bip322 0.0.10` pins to `bitcoin 0.32.x` (or earlier), and a GO/NO-GO call is recorded on Open Decision #1 (adopt crate vs extend custom).
  2. A throwaway bdk_wallet 2.3 P2TR descriptor + BIP-322 message signing PoC has been run, and the result (bdk path viable OR manual `secp256k1::sign_schnorr` fallback required) is recorded in `.planning/research/sprint-0-B.md`, resolving Open Decision #4.
  3. An ADR (architectural decision record) checked into `.planning/decisions/v1.4-adr.md` records the resolutions of Open Decisions #1 (crate adopt/extend), #2 (mixed vs segregated rounds), and #3 (P2SH-P2WPKH wire format: B1 tagged enum vs B2 PSBT-input shape), each with the chosen option and a one-paragraph rationale.
  4. v1.3 `full_round::*` integration tests still pass at this phase boundary (no production code touched by Phase 14 spikes; rollback safety net intact).
  5. Each spike was capped at 2 days of effort or explicitly escalated; spike branches are not merged into `main` (POC code lives in branches, not the trunk).
**Plans**: 3/3 complete
- [x] 14-01-PLAN.md — Sprint-0-A: bip322 0.0.10 cargo tree + cargo audit probe (resolved Open Decision #1 → ADOPT)
- [x] 14-02-PLAN.md — Sprint-0-B: bdk_wallet 2.3 P2TR BIP-322 sign PoC (resolved Open Decision #4 → bdk path)
- [x] 14-03-PLAN.md — v1.4 ADR ratification + Phase 14 closeout (recorded all 4 Open Decisions; structural D-21 gate verified empty)

### Phase 15: Shared Crate Multi-Script Contract
**Goal**: `shared/` becomes the single source of truth for BIP-322 multi-script verification and the new wire types, so coordinator and client compile against one contract and produce byte-identical to_spend/to_sign transactions per script type.
**Depends on**: Phase 14 (Open Decisions #1 + #3 resolved; ADR ratified).
**Requirements**: BIP322-01, BIP322-02, BIP322-03, BIP322-04, ADVERT-04
**Success Criteria** (what must be TRUE):
  1. A `cargo test -p shared` invocation passes per-script-type sign↔verify round-trip property tests for P2WPKH, P2TR, and P2SH-P2WPKH against the official BIP-322 `basic-test-vectors.json` pinned by commit SHA from `bitcoin/bips`.
  2. The 9 (script_pubkey × witness-shape) cross-shape rejection combinations all fail verification with the expected `UnsupportedScriptType` or sighash-mismatch error (V1.4-CRIT-01 spoofing mitigation, statically provable in the `shared/` crate).
  3. A roundtrip serialization test for the new `OwnershipProof` wire format (including the P2SH-P2WPKH `final_script_sig` field per ADR Open Decision #3) passes in `shared/` BEFORE any coordinator or client code consumes the new shape (v1.3 REPAIR-01 lesson #1 enforced as a phase boundary).
  4. `shared` crate compiles with exact-pinned dependency versions; `Cargo.lock` reflects no minor-version drift on `bdk_wallet`, `bitcoin`, or (if adopted) `bip322`.
  5. v1.3 `full_round::*` integration tests still pass at this phase boundary (`shared` changes are additive; existing P2WPKH witness-only path unchanged for v1.3-format inputs).
**Plans**: 3 plans
- [x] 14-01-PLAN.md — Sprint-0-A: bip322 0.0.10 cargo tree + cargo audit probe (resolves Open Decision #1)
- [x] 14-02-PLAN.md — Sprint-0-B: bdk_wallet 2.3 P2TR BIP-322 sign PoC (resolves Open Decision #4)
- [x] 14-03-PLAN.md — v1.4 ADR ratification + Phase 14 closeout (records all 4 Open Decisions; structural D-21 gate) (completed 2026-05-30)

### Phase 16: Coordinator Integration & Advertisement
**Goal**: Coordinator accepts P2WPKH + P2TR + P2SH-P2WPKH ownership proofs under an operator-configurable allowlist and advertises the supported set over PKARR + `/round/info` so clients can fail-fast before opening a Tor circuit.
**Depends on**: Phase 15 (shared crate contract stable).
**Requirements**: ADVERT-01, ADVERT-02, ADVERT-03
**Success Criteria** (what must be TRUE):
  1. An operator running the v1.4 coordinator binary with default config sees a P2TR ownership proof registered and accepted on regtest (the `is_p2wpkh()` gate at `coordinator/src/bitcoin/utxo.rs:119` is gone) — observable via `coordinator` log line "ownership proof verified script_type=p2tr".
  2. An operator who sets `[bip] allow_p2tr = false` in `coordinator.toml` (or `BLINDJOIN__COORDINATOR__BIP__ALLOW_P2TR=false`) sees the binary refuse to start if the config is malformed, and otherwise reject P2TR registrations at runtime with `UnsupportedScriptType` while still accepting P2WPKH — startup validation is fail-fast at boot, never panic-at-first-request.
  3. A client that resolves the coordinator's PKARR record observes `supported_script_types` as a CSV-encoded TXT field on a record bumped to `version: "0.2.0"`; the total payload remains under the 220-byte warn threshold at `coordinator/src/discovery/pkarr_pub.rs:76`. The `/round/info` response carries `supported_script_types` as a proper JSON array.
  4. A spoofing attempt — client declares `script_type: p2wpkh` for an on-chain P2TR UTXO — is rejected with `UnsupportedScriptType` at validate-utxo time because the coordinator derives `script_type` from `txout.script_pubkey` and cross-checks against the client declaration (CRIT-01 invariant, load-bearing, code-review checked).
  5. v1.3 `full_round::*` integration tests still pass at this phase boundary AND a v1.3 client successfully registers a P2WPKH UTXO against the v1.4 coordinator (one cell of the backwards-compat matrix verified inline).
**Plans**: 3 plans
- [x] 14-01-PLAN.md — Sprint-0-A: bip322 0.0.10 cargo tree + cargo audit probe (resolves Open Decision #1)
- [ ] 14-02-PLAN.md — Sprint-0-B: bdk_wallet 2.3 P2TR BIP-322 sign PoC (resolves Open Decision #4)
- [x] 14-03-PLAN.md — v1.4 ADR ratification + Phase 14 closeout (records all 4 Open Decisions; structural D-21 gate)

### Phase 17: Client Multi-Script Wallet & Discovery
**Goal**: A user with a v1.4 client can generate a wallet of any of three script types, sign BIP-322 ownership proofs for that type, and reject mismatched coordinators before any Tor circuit opens.
**Depends on**: Phase 15 (shared crate contract) and Phase 16 (coordinator advertisement format stable so the client can write code against it).
**Requirements**: WALLET-01, WALLET-02, WALLET-03, WALLET-04
**Success Criteria** (what must be TRUE):
  1. A user runs `client generate-wallet --type p2tr` and the resulting descriptor file holds a `tr(.../86'/...)` descriptor (BIP-86); `--type p2sh-p2wpkh` produces a `sh(wpkh(.../49'/...))` descriptor (BIP-49); `--type p2wpkh` (default for backwards compatibility) produces a `wpkh(.../84'/...)` descriptor.
  2. A v1.4 client successfully completes the BIP-322 ownership-proof signing step for all three script types against a v1.4 coordinator on regtest — observable as the round transitioning out of INPUT_REG with no `Bip322Error` for any of the three input variants.
  3. A v1.4 client with a P2TR wallet pointed at a v1.3 coordinator (or v1.4 coordinator with `allow_p2tr = false`) rejects the coordinator at discovery time BEFORE opening a Tor circuit, with a clear error naming both the coordinator and the missing script type (e.g. `coordinator <onion> does not support p2tr ownership proofs`).
  4. A v1.4 client with a P2WPKH wallet successfully completes a full CoinJoin round against an unmodified v1.3 coordinator (the WALLET-04 compatibility shim correctly detects pre-`0.2.0` PKARR / missing `/round/info` field and emits the legacy witness-only `OwnershipProof` wire format).
  5. v1.3 `full_round::*` integration tests still pass at this phase boundary (the client's existing P2WPKH path is preserved as a code path, not removed in favor of the new dispatcher).
**Plans**: 3 plans
- [x] 14-01-PLAN.md — Sprint-0-A: bip322 0.0.10 cargo tree + cargo audit probe (resolves Open Decision #1)
- [ ] 14-02-PLAN.md — Sprint-0-B: bdk_wallet 2.3 P2TR BIP-322 sign PoC (resolves Open Decision #4)
- [ ] 14-03-PLAN.md — v1.4 ADR ratification + Phase 14 closeout (records all 4 Open Decisions; structural D-21 gate)

### Phase 18: Mixed-Script E2E + Liquidity Bot
**Goal**: An operator running the v1.4 stack on signet sees the liquidity bot generate UTXOs across all enabled script types, and the v1.4 acceptance gate — a mixed-script CoinJoin round on regtest — completes and broadcasts a real txid.
**Depends on**: Phase 17 (client multi-script signing) and Phase 16 (coordinator multi-script verification).
**Requirements**: INTEG-01, INTEG-02
**Success Criteria** (what must be TRUE):
  1. A `cargo test -p coordinator --test full_round -- --include-ignored` invocation on a developer machine with pinned bitcoind reports a passing mixed-script E2E test where at least 1 P2WPKH + 1 P2TR + 1 P2SH-P2WPKH input register, complete OUTPUT_REG and SIGNING, and the resulting txid is observable in the regtest mempool (BROADCAST phase reached).
  2. The mixed-script test reuses `BitcoindGuard` + `require_bitcoind!()` unchanged from v1.3 — no new test-fixture machinery, no `Box::leak`, no inline skip blocks.
  3. The liquidity bot, started with `script_types = ["p2wpkh", "p2tr", "p2sh-p2wpkh"]` in its config, generates UTXOs across all three types over a 3-round signet window AND rotates the type it uses per round (so its registrations are not a uniform-script fingerprint that defeats V1.4-MIN-02).
  4. v1.3 `full_round::*` P2WPKH-only integration tests still pass alongside the new mixed-script test — both suites green in a single `cargo test` run, providing the rollback safety net at the milestone boundary.
  5. The v1.3-client ↔ v1.4-coordinator compatibility cell of the backwards-compat matrix is verified inline (a v1.3 client binary registers a P2WPKH UTXO against the v1.4 coordinator and the round completes), discharging the WALLET-04 compatibility shim against a real v1.3 build artifact.
**Plans**: 3 plans
- [ ] 14-01-PLAN.md — Sprint-0-A: bip322 0.0.10 cargo tree + cargo audit probe (resolves Open Decision #1)
- [ ] 14-02-PLAN.md — Sprint-0-B: bdk_wallet 2.3 P2TR BIP-322 sign PoC (resolves Open Decision #4)
- [ ] 14-03-PLAN.md — v1.4 ADR ratification + Phase 14 closeout (records all 4 Open Decisions; structural D-21 gate)

## Cross-Phase Invariant (v1.4)

> **At every v1.4 phase boundary, the v1.3 P2WPKH-only `full_round::*` integration tests MUST remain green.** This is the rollback safety net inherited from v1.3 REPAIR-01 forensics: if a phase breaks the v1.3 path, abandon the structured plan and pivot to `/gsd:debug` per REPAIR-01 lesson #4 (when 2-3 carry-forward plans appear with the same shape, the structured path has ceased to be load-bearing).

## Carry-Forward (explicitly NOT v1.4)

These items appear in `REQUIREMENTS.md` Future Requirements and are NOT mapped to any v1.4 phase. They are tracked for v1.5+ scheduling:

- **CARRY-TOR-UAT**: Tor-mode verification harness (Phase 8 HUMAN-UAT item 3).
- **CARRY-REPAIR-01-PR**: REPAIR-01 PR observation closure (the v1.4 cut PR is the natural moment to discharge this but is NOT a v1.4 code deliverable per REPAIR-01 lesson #5).
- **B-03**: Dynamic fee estimation (mempool-aware polling + RBF).
- **TEST-EXT-01/02/03**: Cross-implementation differential fixtures, on-chain anchor test, automated backwards-compat matrix.

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
| 14. Sprint-0 Spikes + Discuss-Phase Decisions | v1.4 | 3/3 | Complete    | 2026-05-30 |
| 15. Shared Crate Multi-Script Contract | v1.4 | 0/0 | Not started | — |
| 16. Coordinator Integration & Advertisement | v1.4 | 0/0 | Not started | — |
| 17. Client Multi-Script Wallet & Discovery | v1.4 | 0/0 | Not started | — |
| 18. Mixed-Script E2E + Liquidity Bot | v1.4 | 0/0 | Not started | — |

Full v1.0 details: `.planning/milestones/v1.0-ROADMAP.md`
Full v1.1 details: `.planning/milestones/v1.1-ROADMAP.md`
Full v1.2 details: `.planning/milestones/v1.2-ROADMAP.md`
Full v1.3 details: `.planning/milestones/v1.3-ROADMAP.md`
