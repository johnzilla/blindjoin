# Research Synthesis — v1.4 BIP-322 Multi-Script Support

**Milestone:** v1.4 — extend the existing P2WPKH-only BIP-322 ownership-proof verifier to also accept **P2TR (BIP-86 single-key Taproot)** and **P2SH-P2WPKH**.
**Synthesized:** 2026-05-29
**Inputs:** STACK.md, FEATURES.md, ARCHITECTURE.md, PITFALLS.md
**Overall confidence:** MEDIUM-HIGH

> **Scope discipline:** v1.0 baseline stack (tokio, axum, arti-client, bdk_wallet 2.3, blind-rsa-signatures, pkarr, sqlx, corepc-types) is NOT re-evaluated. v1.4 is a surgical extension to ONE surface — the UTXO ownership-proof verifier and the address-type acceptance gate. No new layers, no new components, no re-architecture.

---

## Executive Summary

v1.4 replaces a single line — the `is_p2wpkh()` hard gate at `coordinator/src/bitcoin/utxo.rs:119` — with a script-type allowlist + dispatcher that handles P2WPKH (already shipped), P2TR (BIP-86 key-path), and P2SH-P2WPKH (single-sig wrapped segwit). The to_spend/to_sign virtual-transaction primitives in `shared/src/bip322.rs` (lines 19–76) are already script-type-neutral and stay unchanged; what's new is **two additional sighash paths (BIP-341 Taproot keypath + BIP-143 over the unwrapped P2WPKH redeem script)** and **two additional signature primitives (Schnorr via `verify_schnorr`/`XOnlyPublicKey` and a per-script witness-stack arity validator)**. All of these primitives already exist in `bitcoin 0.32.x` and are pulled in transitively today — v1.4 adds **zero new mandatory dependencies** under the recommended path.

The risks are not in the new code — they are in **two load-bearing decisions left open by the researchers** (crate adoption + mixed-vs-segregated round policy) and in **five v1.3 REPAIR-01 lessons** that v1.4 must not re-trigger. Both open decisions must be settled in discuss-phase before plan-phase derives tasks.

The recommended phase numbering for v1.4 is **Phase 14 through Phase 18** (continuing from v1.3 which ended at Phase 13), structured as: Sprint-0 spikes → shared crate → coordinator integration → client integration → liquidity-bot + end-to-end. Phase 14 is gating — its two spikes determine the effort estimate for everything downstream.

---

## Stack Recommendations

### Recommended path: zero new top-level dependencies

| Crate | Pinned at | Why no change in v1.4 |
|-------|-----------|----------------------|
| `bitcoin` | 0.32.x | Already provides `SighashCache::p2wpkh_signature_hash`, `SighashCache::taproot_key_spend_signature_hash`, `Script::is_p2wpkh/is_p2tr/is_p2sh`, `secp256k1::verify_schnorr`, `XOnlyPublicKey::from_slice`. Every primitive needed. |
| `bdk_wallet` | =2.3.x (exact pin; v1.3 REPAIR-02 lesson) | Already supports `tr(...)` (P2TR/BIP-86), `sh(wpkh(...))` (P2SH-P2WPKH/BIP-49), and `wpkh(...)` (P2WPKH/BIP-84) descriptors and per-descriptor PSBT signing. |
| `proptest` | 1.x | Sufficient for per-script-type property tests against BIP-322 spec vectors. |
| `corepc-node` | 0.12 with feature pin (CI grep gate per v1.3 REPAIR-02) | Regtest harness already script-type-agnostic; just call `getnewaddress` with explicit `address_type` parameter for each script type. |
| All other v1.0 baseline crates (arti-client, pkarr, sqlx, blind-rsa-signatures, tower_governor, axum, tokio…) | Unchanged | v1.4 is bytes-in/bytes-out at every layer below the verifier dispatcher; Tor and PKARR don't know what BIP-322 is. |

### Conditional path: adopt the upstream `bip322` crate

If discuss-phase votes to adopt the upstream crate, **exactly one line** changes in `shared/Cargo.toml`:

```toml
bip322 = "=0.0.10"  # exact pin; 0.0.x has no SemVer guarantee
```

This is **contested between researchers** — see Open Decisions §1 below. Under no circumstances should this line be added before **Sprint-0-A** (the `cargo tree` check) confirms `bip322 0.0.10` pins to `bitcoin 0.32.x` (not 0.31.x or earlier).

### Anti-list (crates v1.4 does NOT add)

`bip322-rs` (fork at 0.0.11), `bip322-simple` (different scope), `bip322-signer` (no recent presence), `rust-miniscript` as direct dep, `bdk_chain`/`bdk_electrum`/`bdk_esplora`/`bdk_hwi`, separate `taproot` or `schnorr` crate.

---

## Feature Scope

### Must-ship (v1.4 acceptance gate)

1. **Coordinator verifier accepts P2WPKH + P2TR (BIP-86 keypath) + P2SH-P2WPKH** (single-sig only); hard `is_p2wpkh()` gate at `coordinator/src/bitcoin/utxo.rs:119` is replaced with a config-driven allowlist + per-type dispatcher.
2. **Three operator-tunable config flags** — `allow_p2wpkh`, `allow_p2tr`, `allow_p2sh_p2wpkh` — defaulting all to `true`, validated at startup (Wasabi PR #8912 precedent for `AllowP2trInputs`).
3. **Coordinator advertises `supported_script_types` over PKARR + `/round/info`** so clients reject mismatched coordinators before opening a Tor circuit. Field is derived from the config flags so the wire and the config are always consistent.
4. **Client wallet supports three BIP descriptor types** — BIP-84 (P2WPKH, already shipped), BIP-86 (P2TR), BIP-49 (P2SH-P2WPKH) — selected by a new `--type` CLI flag at `generate-wallet` time. Defaults to P2WPKH for backwards compatibility.
5. **Client signs BIP-322 ownership proofs for all three types** via the existing bdk_wallet PSBT-sign path (assuming Sprint-0-B confirms bdk_wallet 2.3 produces correct Taproot keypath witnesses).
6. **Client pre-flight rejects mismatched coordinators** at discovery time, before any registration attempt.
7. **Liquidity bot generates UTXOs across all enabled script types** so cold-start signet rounds exercise every code path.
8. **Mixed-script-type end-to-end integration test on regtest** — at least one P2WPKH + one P2TR + one P2SH-P2WPKH input completing a full round. This is the v1.4 acceptance gate.
9. **Per-script-type property tests against BIP-322 `basic-test-vectors.json`** from the official `bitcoin/bips` repo, pinned by commit SHA.
10. **Cross-implementation differential test fixtures** generated by `ACken2/bip322-js` (JS reference impl) and checked into `tests/fixtures/bip322/` as static JSON files.

### Differentiators worth shipping

- Operator opt-in / opt-out per script type (already in must-ship #2)
- Internal aggregate counters per script type for capacity planning — but **NOT** publicly exposed (would fingerprint)

### Explicit anti-features (do NOT build)

- Per-script-type ban tracking — leaks correlation across rounds; keep ban list uniform on `OutPoint`
- Per-script-type rate limits — defeats Tor-safe `GlobalKeyExtractor` (v1.2 hardening)
- Per-script-type round denominations — fragments anon sets
- Public round-state advertising script-type breakdown
- Legacy P2PKH, bare P2SH (raw multisig), P2TR script-path, P2WSH multisig

### Deferred to v1.5+

- Mixed output script types (Wasabi 2.0.3-style per-participant output choice)
- Tor-mode UAT harness (carried forward from v1.3 Phase 8 HUMAN-UAT item 3)
- REPAIR-01 PR observation closure

---

## Architecture Plan

### Touchpoints (deltas only — no new components)

```
shared/                               coordinator/                          client/
─────────                             ────────────                          ───────
src/bip322.rs                         src/bitcoin/utxo.rs:119                src/wallet.rs
  + ScriptType enum                  ←  REPLACE is_p2wpkh() gate              + tr() and sh(wpkh())
  + detect_script_type(spk)             with allowlist + dispatch              descriptor templates
  + sign_simple(type,…)                 to shared::bip322::verify_simple       + script_type() accessor
  + verify_simple(type,…)                                                      + --type CLI flag
  KEEP: to_spend / to_sign            src/round/state.rs
  (script-type-neutral)               + RegisteredInput.script_type            src/round/input.rs
                                        (#[zeroize(skip)])                     REPLACE inlined P2WPKH
src/protocol.rs:13-28                                                          BIP-322 sign path with
  + OwnershipProof.script_type        src/config.rs                            shared::bip322::sign_simple
  + InfoResponse.                     + BipConfig section
    supported_script_types              + supported_script_types Vec           src/discover.rs
    (#[serde(default)])                                                        + pre-flight check
                                      src/discovery/pkarr_pub.rs:76            against supported_script_types
                                      + supported_script_types in JSON
                                      + bump version 0.1.0 → 0.2.0             liquidity-bot/
                                                                               + three keychains
                                      src/round/signing.rs                       (m/49' + m/84' + m/86')
                                      + final_script_sig for P2SH-P2WPKH       + per-round script-type
                                        (wire-format change — see              rotation
                                        Open Decision #3 below)
```

### Key invariant added in v1.4 (LOAD-BEARING)

> **Coordinator MUST cross-check the client-declared `script_type` against `detect_script_type(on_chain_spk)` at validate-utxo time.** A client claiming P2WPKH for a P2TR UTXO must be rejected, even if the witness happens to verify under both dispatchers. See PITFALLS V1.4-CRIT-01.

### Build order (testable in isolation)

| Phase | What | Why this order |
|-------|------|----------------|
| **Phase 14 — Sprint-0 spikes** | (A) `cargo tree -p bip322 0.0.10` against `bitcoin 0.32.x` to verify version alignment. (B) bdk_wallet 2.3 P2TR descriptor + BIP-322 message signing throwaway PoC. | Both gate the rest. Spike (A) settles Open Decision #1. Spike (B) settles Open Decision #4. Cap each at 2 days. |
| **Phase 15 — shared crate** | Add `ScriptType` enum + `detect_script_type` + per-type `sign_simple` / `verify_simple`; extend `OwnershipProof` and `InfoResponse`; per-script-type sign↔verify property tests against BIP-322 spec vectors. | Both coordinator and client compile against `shared`. Without it, downstream crates cannot iterate independently. Matches v1.0's "shared crate is the contract" pattern. |
| **Phase 16 — coordinator integration** | Add `BipConfig` + startup validation; replace `is_p2wpkh()` gate with dispatcher; update PKARR publisher (`pkarr_pub.rs:76`); add `script_type` to `RegisteredInput`; handle `final_script_sig` for P2SH-P2WPKH in PSBT assembly. | Coordinator with config-disabled allowlist is harmless to ship intermediate. Existing v1.3 P2WPKH integration tests must remain green at this phase boundary. |
| **Phase 17 — client integration** | Extend `wallet.rs` for `tr()` and `sh(wpkh())` descriptors; rewire `generate_bip322_witness` to `shared::bip322::sign_simple`; add `discover.rs` pre-flight check; `--type` CLI flag; client-side compatibility shim for v1.3 coordinators. | Client validates against coordinator; cannot iterate without Phase 16 in place. |
| **Phase 18 — liquidity-bot + end-to-end** | Bot generates UTXOs across three script types; mixed-script integration test on regtest; backwards-compat tests (v1.3 client ↔ v1.4 coordinator, v1.4 client ↔ v1.3 coordinator); per-script-type property tests against BIP-322 spec vectors. | The integration test is the acceptance gate. |

### Patterns preserved from v1.0/v1.3

Phase-Gated HTTP API (new gate layers after existing phase gate), Alice/Bob identity separation, per-round RSA keypair, memory-only round state (new `script_type` field is `#[zeroize(skip)]`), Tokio phase timer, **shared crate as the contract** (extended not replaced), `BitcoindGuard` RAII + `require_bitcoind!()` macro (script-type-agnostic; reused unchanged), CI feature pin for `corepc-node` (unchanged).

---

## Pitfalls Watchlist

### Critical (must be mitigated in code or design)

1. **V1.4-CRIT-01 — Script-type spoofing.** Coordinator MUST derive `script_type` from the on-chain `script_pubkey` and cross-check against the client's declared type. Property test all 9 (script_pubkey × witness-shape) combinations. Code review: any `match ownership_proof.script_type` block in the coordinator → reject.
2. **V1.4-CRIT-02 — Silent sighash failures across script types.** Three distinct verifier functions, not one parameterized helper. Per-type property tests against BIP-322 `basic-test-vectors.json`, cross-impl differential tests against `bip322-js`, regtest on-chain anchor test (sign a BIP-322 message AND a real spend with the same key; bitcoind acceptance proves sighash math correct).
3. **V1.4-CRIT-03 — `bip322` crate pre-1.0 API risk.** Crate stalled at 0.0.10 for ~9 months. **Open Decision #1 below.** If adopted: exact-pin (`=0.0.10`), CI contract tests for every method we call, fallback path documented.

### Moderate (must inform design choices)

4. **V1.4-MOD-01 — OwnershipProof wire-format evolution.** P2SH-P2WPKH needs both `final_script_witness` AND `final_script_sig`. **Open Decision #3 below.** Roundtrip serialization test in `shared/` ships before either side uses new shape.
5. **V1.4-MOD-02 — PKARR record schema evolution.** Current payload ~175 bytes; adding `supported_script_types` brings it to ~215 bytes (under 220-byte warn at `pkarr_pub.rs:76`, under 255-byte DNS TXT limit). Bump version `"0.1.0"` → `"0.2.0"`. Backwards-compat via `#[serde(default)]` both ways.
6. **V1.4-MOD-03 — `/round/info` field addition.** Client uses `supported_script_types` if present; falls back to `["p2wpkh"]` if absent. Pre-flight check BEFORE Tor circuit open.
7. **V1.4-MOD-04 — bdk_wallet 2.3 multi-script support unclear.** Issue #150 (BIP-322 signing) open since May 2023; issue #394/#590 (multi-script) active without 2.3 resolution. **Resolved by Sprint-0-B spike.** Fallback: manual `secp256k1::Secp256k1::sign_schnorr` over direct sighash construction.
8. **V1.4-MOD-05 — bdk_wallet 2.3 → 2.4+ minor-version churn.** Exact pin (`bdk_wallet = "=2.3.x"`), CI grep gate.
9. **V1.4-MOD-06 — Mixed vs segregated script-type rounds.** **Open Decision #2 below.**
10. **V1.4-MOD-07 — BIP-322 vs legacy `signmessage` confusion.** `bip322_message_hash` in `shared/src/bip322.rs` is the single source of truth; all three verifiers reuse it; property test against spec `to_spend_txid` values.

### Minor (operational hygiene)

11. **V1.4-MIN-01 — Regtest harness brittleness.** Explicit `getnewaddress` `address_type` per test (`bech32m`, `p2sh-segwit`, `bech32`).
12. **V1.4-MIN-02 — Liquidity bot becomes uniform-script fingerprint.** Bot rotates script types; honest README disclaimer.
13. **V1.4-MIN-03 — BIP-322 Simple-vs-Full wire form ambiguity.** Single adapter function if crate adopted; contract-tested.

---

## Open Decisions for Discuss-Phase

These are the calls plan-phase **CANNOT** make without an explicit decision because the answer materially changes the build plan. The synthesizer deliberately presents both sides — these are **not** silently resolved.

### Open Decision #1 — Adopt `bip322` crate vs. extend custom `shared/src/bip322.rs`

| Researcher | Position | Strongest argument |
|------------|----------|-------------------|
| **STACK** | **EXTEND CUSTOM** (~205 LOC) | Crate stalled at 0.0.10 for ~9 months; bdk_wallet doesn't ship BIP-322 signing anyway (issue #150 open since May 2023) so the client signer must be ours either way; `bitcoin 0.32.x` already provides every sighash + verification primitive needed; API mismatch (`verify_simple(&Address, message, Witness)` vs our `(scriptPubKey, witness, message)` wire format) forces an adapter regardless. |
| **FEATURES** | **ADOPT crate** | Crate covers exactly our target set; `verify_simple` API matches our verification flow conceptually; production user signal (`ord 0.24.2` reverse-deps on `bip322 ^0.0.10`); reference test vectors included. |
| **ARCHITECTURE** | **OPEN — flagged as Decision A** | Architecture identical either way; only the implementation of `shared::bip322::{sign_simple, verify_simple}` changes. Sprint-0 spike required. |
| **PITFALLS** | **DEFAULT EXTEND CUSTOM**, gates adoption on explicit risk acceptance | 0.0.x SemVer has no guarantee; 9 months of silence is a maintenance-cadence red flag; v1.3 REPAIR-01 forensics show wire-format mismatches are catastrophic and hard to find. |

**Discuss-phase must decide:** GO/NO-GO on the `bip322` crate. The decision changes the effort estimate for Phase 15 by 3-5x. **Gating prerequisite either way: Sprint-0-A** (the `cargo tree` check) must complete before this decision is irreversible.

### Open Decision #2 — Mixed-script rounds vs. segregated rounds

| Researcher | Position | Strongest argument |
|------------|----------|-------------------|
| **FEATURES** | **MIXED rounds (Option B)** | Wasabi 2.0.3 precedent (PR #8912 + discussion #9216); milestone goal is "broaden CoinJoin participation," not "redesign round state machine"; output uniformity (still single-script-type outputs in v1.4) is the dominant privacy lever; anon-set math favours mixed at small participant counts. |
| **PITFALLS** | **SEGREGATED rounds (privacy-conservative)** | Wasabi's argument relies on credential-equivalence which does NOT transfer to blindjoin's RSA blind-signature model; mixed rounds create a chain-analysis signal (heterogeneous-input + equal-value-output = CoinJoin fingerprint) and per-input script-type leaks individual UTXOs to the round; v1.0 invariant "do not mix address types in the same round" is conservative for a reason. |
| **STACK / ARCHITECTURE** | **NEUTRAL** | Architecture assumes mixed in §3.1 but if segregated wins it's a Phase 16 design change (per-script-type round queue), not a Phase 15 shared-crate change. |

**Discuss-phase must decide.** The decision changes PKARR record schema, `/round/info` shape, liquidity bot strategy, and the coordinator round-state machine. If overturned in favor of segregated, the PKARR schema change is **wire-incompatible** with what Phase 14-15 would otherwise produce.

### Open Decision #3 — Partial-sig wire format for P2SH-P2WPKH

| Option | Pros | Cons |
|--------|------|------|
| **B1: Tagged enum** — `enum PartialSigPayload { WitnessOnly(Witness), WitnessAndScriptSig { witness, script_sig } }` with a version byte | Minimal byte overhead for P2WPKH/P2TR (still witness-only on the wire); easy to extend | Two cases to deserialize; another bespoke shape to maintain |
| **B2: PSBT-input shape** — base64-encoded `bitcoin::psbt::Input` | PSBT-everywhere aligns with the round PSBT contract; natively carries `final_script_sig` + `final_script_witness` + future fields | Larger byte overhead per partial-sig |

**Architecture research recommends B2.** Either way: roundtrip serialization test in `shared/` ships before either side uses the new shape (v1.3 REPAIR-01 lesson #1).

### Open Decision #4 — bdk_wallet 2.3 multi-descriptor strategy (resolved by Sprint-0-B)

If Sprint-0-B confirms `bdk_wallet 2.3`'s `wallet.sign(psbt, SignOptions { trust_witness_utxo: true })` produces a correct Taproot keypath witness for a `tr(...)` descriptor, this decision is closed.

If the spike reveals bdk_wallet 2.3 does not produce a valid Taproot witness, the fallback is **manual `secp256k1::Secp256k1::sign_schnorr` over a direct sighash construction**, bypassing bdk_wallet for the BIP-322 sign path. **Sprint-0-B is on Phase 14's critical path.** Cap at 2 days.

---

## v1.3 REPAIR-01 Lessons — Carry-Forward Constraints

1. **Wire format ≠ API shape.** Any wire-format change ships with a roundtrip serialization test in `shared/` **before** either coordinator or client uses the new shape. (Open Decisions #3 + V1.4-MOD-01.)
2. **bdk_wallet 2.3 segwit signing requires `SignOptions { trust_witness_utxo: true }`** and real on-chain `witness_utxo` values. P2SH-P2WPKH path uses the same machinery — do not retry zero placeholders.
3. **Pin every dependency referenced by version in a test fixture, CI-enforce.** `bip322` crate (if adopted), `bdk_wallet`, `corepc-node` feature pin.
4. **When 2-3 carry-forward plans appear with the same shape, abandon Plan.md and pivot to direct bisectable commits.** Pivot to `/gsd:debug` early.
5. **"Closed-local" creates tracking debt.** REPAIR-01 PR observation closure is **explicitly out of v1.4 scope** (carried forward to v1.5). The v1.4 cut PR is the natural moment to discharge it but is not a v1.4 deliverable.

---

## Phase-Numbering Implications

v1.3 ended at Phase 13. **v1.4 begins at Phase 14.**

| Phase | Name | Acceptance gate | Effort (extend-custom) | Effort (adopt-crate) |
|-------|------|----------------|----------------------|--------------------|
| **14** | Sprint-0 spikes (A + B) | GO/NO-GO on crate adoption; bdk_wallet 2.3 P2TR sign path validated | 2 days | 2 days |
| **15** | Shared crate (`ScriptType` + dispatch + sign/verify + wire-type evolution) | Per-script-type sign↔verify property tests pass against BIP-322 spec vectors | 3-4 days (~205 LOC) | 1-2 days (~50 LOC) |
| **16** | Coordinator integration | v1.3 P2WPKH integration tests still green; per-script-type registration tests pass | 2-3 days | 2-3 days |
| **17** | Client integration | v1.4 client registers against both v1.4 and v1.3 coordinators | 3-4 days | 2-3 days |
| **18** | Liquidity-bot + end-to-end mixed-script integration test | Mixed P2WPKH + P2TR + P2SH-P2WPKH round completes on regtest; backwards-compat matrix passes | 2-3 days | 2-3 days |

**Total range:** 12-16 days (extend-custom) or 9-13 days (adopt-crate, if Sprint-0-A is clean).

**At every phase boundary, v1.3 P2WPKH integration tests must remain green.** This is the rollback safety net.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack (versions, primitives, anti-list) | **HIGH** | One ambiguity (`bip322 0.0.10` ↔ `bitcoin 0.32.x` pin) bounded by Sprint-0-A. |
| Features (table-stakes, anti-features, ecosystem precedent) | **HIGH** | Wasabi PR #8912 + discussion #9216 explicit; BIP-322 wire format settled; only mixed-vs-segregated decision contested. |
| Architecture (touchpoints, build order, backwards-compat) | **HIGH** on integration points (codebase read in full); **MEDIUM** on crate-vs-custom path (Sprint-0). All file paths and line numbers verified. |
| Pitfalls (spoofing, sighash, crate risk, wire-format) | **MEDIUM-HIGH** | All 13 pitfalls map to concrete mitigations. Two are open decisions, not closed mitigations. |

### Gaps for planning to address

1. Sprint-0-A outcome by end of Phase 14 day 1. If `bip322 0.0.10` pins to `bitcoin 0.31.x` or earlier, adopt-crate is **BLOCKED**; revert to extend-custom.
2. Sprint-0-B outcome by end of Phase 14 day 2. If PoC fails, `shared::bip322::sign_simple` takes a `Box<dyn Signer>`-shaped trait abstraction (~30 LOC).
3. Discuss-phase ratification of Open Decisions #1, #2, #3 — human-decision-only.
4. REPAIR-01 PR observation closure tracked as v1.5 follow-up, not v1.4 deliverable.

---

## Sources

### Spec (canonical)
- BIP-322 (Generic Signed Message Format), BIP-341 (Taproot), BIP-340 (Schnorr), BIP-143 (Segwit v0 sighash), BIP-86 (P2TR derivation), BIP-49 (P2SH-P2WPKH derivation) — all HIGH

### Rust crates
- `crates.io/crates/bip322` 0.0.10 — HIGH on version, MEDIUM on API stability
- `docs.rs/bitcoin/0.32.6` — confirms primitives — HIGH
- `bitcoindevkit/bdk_wallet#150` open since May 2023 — HIGH
- `crates.io/crates/ord` production user — HIGH

### Ecosystem precedent
- WalletWasabi PR #8912 (Taproot coordinator-side `AllowP2trInputs`) — HIGH
- WalletWasabi #9216 (Taproot discussion — 50/50 output policy) — HIGH
- `docs.wasabiwallet.io/using-wasabi/CoinJoin.html` — HIGH
- JoinMarket high-level design — MEDIUM

### Test vectors and cross-implementation
- `bip-0341/wallet-test-vectors.json` — HIGH
- `github.com/ACken2/bip322-js` JS reference — MEDIUM
- guggero/btcd bip322 test suite Go cross-impl — MEDIUM

### In-tree files (HIGH confidence — read in full by ARCHITECTURE researcher)
- `shared/src/bip322.rs` — script-type-neutral primitives at lines 19-76
- `shared/src/protocol.rs:13-28` — `InfoResponse` + `OwnershipProof` wire types
- `coordinator/src/bitcoin/utxo.rs:119` — the `is_p2wpkh()` gate to remove
- `coordinator/src/discovery/pkarr_pub.rs:76` — 220-byte warn threshold
- `coordinator/src/round/state.rs` — `RegisteredInput` + `RoundStateInner`
- `coordinator/src/round/signing.rs:163` — witness deserialization
- `coordinator/src/round/blame.rs` — confirmed script-type-agnostic
- `coordinator/src/config.rs` — where `BipConfig` lands
- `client/src/wallet.rs` — `BdkClientWallet` descriptor paths
- `client/src/round/input.rs:105-139` — current inlined P2WPKH BIP-322 signer
- `client/src/discover.rs` — pre-flight check landing point

---

*Synthesis date: 2026-05-29. This document does not silently resolve the two contested findings (Open Decision #1: crate vs custom; Open Decision #2: mixed vs segregated rounds). Plan-phase should not derive tasks until discuss-phase ratifies both. Sprint-0-A and Sprint-0-B are the Phase 14 deliverables that gate downstream effort estimates.*
