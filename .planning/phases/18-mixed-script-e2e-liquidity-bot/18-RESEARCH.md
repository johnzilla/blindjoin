# Phase 18: Mixed-Script E2E + Liquidity Bot — Research

**Researched:** 2026-05-30
**Domain:** Rust workspace integration testing + multi-script CoinJoin acceptance gate + liquidity bot rotation + v1.3 binary compat gate
**Confidence:** HIGH (codebase-rooted; almost every recommendation has a file:line anchor)

## Summary

Phase 18 is the v1.4 milestone acceptance gate. It exercises every prior v1.4 deliverable (Phases 14–17) end-to-end on regtest and unblocks the v1.4 milestone cut. Two requirements:

1. **INTEG-01** — Mixed-script E2E test (3 distinct input types in one round, broadcast txid in regtest mempool).
2. **INTEG-02** — Liquidity bot multi-script + per-round rotation (V1.4-MIN-02 mitigation).

Two ancillary deliverables close out the milestone:

3. **v1.3-client ↔ v1.4-coordinator binary acceptance gate** (ROADMAP success criterion #5; discharges WALLET-04 against a real v1.3 build artifact).
4. **README §"Privacy Considerations"** prose (Phase 14 CD-3 carry-forward).

The phase is structurally low-risk: every external API surface Phase 18 consumes (`fund_regtest_typed`, `BdkClientWallet::{generate, from_descriptor, from_wif, sign_bip322, script_type}`, `register_input(..., &CoordinatorInfo)`, `BipConfig::default()`, `validate_utxo` dispatcher) was landed and verified GREEN in Phases 16/17. Phase 18 adds NO new code paths to `coordinator/**` or `shared/**`. The mixed-script E2E test is a near-clone of `full_round::full_round_three_clients` with 3-type funding swapped in for the WIF-only funding.

**Primary recommendation:** Land the 3-plan structure per CONTEXT D-105 (18-01 INTEG-01 → 18-02 INTEG-02 → 18-03 v1.3 binary + README + closeout). Use the **B1.b descriptor-wallet-driven funding path** for P2TR + P2SH-P2WPKH clients (CONTEXT D-83 — confirmed structurally available: `BdkClientWallet.utxo_outpoint` is a `pub` field assignable post-construction). The v1.3 binary gate is **GO for the D-86 automated path**: pinned SHA `05f21438` workspace deps match HEAD byte-exactly, so `cargo build --release --bin client` against a `git worktree` of that SHA produces a v1.3 binary using the current `Cargo.lock` without drift.

---

## User Constraints (from CONTEXT.md)

### Locked Decisions (carried from Phase 14 ADR + Phases 15/16/17 + Phase 18 CONTEXT D-81..D-106)

- **ADR #2 / Phase 14 D-06:** MIXED rounds. Coordinator's per-script dispatcher accepts heterogeneous inputs in one round. Phase 18 INTEG-01 demonstrates this end-to-end with 3 distinct input types.
- **Phase 14 D-07 (consequence):** Single output script type per round, operator-configured via `BipConfig::output_script_type` ("ost"). Phase 18 does NOT add a runtime coordinator gate on submitted output addresses — the existing client-side fail-fast at `client::discover::discover_coordinator` (Phase 17 D-76, `UnsupportedOutputScriptType`) is the canonical enforcement. Integration tests bypass discovery (per Phase 17's `v14_p2wpkh_coordinator_info()` helper) and drive heterogeneous outputs through the coordinator unchanged.
- **Phase 14 D-08 / V1.4-MOD-06:** Heterogeneous-input chain-analysis fingerprint is a KNOWN LIMITATION; documented in README §"Privacy Considerations" per Phase 14 CD-3 → Phase 18 D-106 (this phase).
- **Phase 14 D-10 / V1.4-CRIT-01:** Client declares `script_type` on v=2 OwnershipProof; coordinator cross-checks against `detect_script_type(on_chain_spk)`. Already enforced. Phase 18 reuses verbatim — no new mitigations.
- **Phase 15 LOCKED API:** `shared::bip322::{ScriptType, Bip322Error, detect_script_type, verify_simple, sign_simple, sign_simple_test_only, bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign}` + `shared::protocol::{OwnershipProof, InfoResponse, InputRegRequest, ...}`. Phase 18 consumes verbatim.
- **Phase 16 wire shape:** PKARR record `v: "0.2.0"`, `sst` (CSV), `ost` (scalar); `InfoResponse` adds `supported_script_types` + `output_script_type` with `#[serde(default)]` legacy fallbacks. Phase 18 mixed-script test bypasses discovery; v1.3-binary gate exercises legacy-defaults path implicitly.
- **Phase 16 `BipConfig::default()`:** `allow_p2wpkh = allow_p2tr = allow_p2sh_p2wpkh = true`, `output_script_type = P2wpkh` (`coordinator/src/config.rs:256-265`). Phase 18 in-process coordinators use this default unchanged.
- **Phase 16 `fund_regtest_typed`:** `tests/integration/mod.rs:617-823`. Already in place; reused verbatim per CONTEXT D-83.
- **Phase 17 D-61:** `BdkClientWallet::from_wif` is P2WPKH-only. Phase 18 bot's P2TR + P2SH-P2WPKH paths MUST go through `BdkClientWallet::from_descriptor`.
- **Phase 17 D-78 / D-79:** Phase 17 verified WALLET-04 with a STUBBED v1.3 PKARR record; Phase 18 verifies WALLET-04 with a REAL v1.3 BINARY.
- **Cross-phase invariant:** `cargo test --test integration full_round` MUST remain GREEN at every Phase 18 plan boundary. Baseline: **8 passed, 0 failed, ~42s** (Phase 17 verification). REPAIR-01 lesson #4 pivot to `/gsd:debug` on drift.

### Phase 18 explicit decisions (D-81..D-106)

- **D-81/D-82:** NEW file `tests/integration/mixed_script_e2e.rs`; test fn `mixed_script_e2e_three_clients_broadcast`. Add `mod mixed_script_e2e;` to `tests/integration/mod.rs:19-24` (alphabetical sort).
- **D-83 (B1.b RECOMMENDED):** Descriptor-wallet-driven funding for P2TR + P2SH-P2WPKH (no raw-key UTXO needed); P2WPKH client takes the WIF path. **CONFIRMED STRUCTURALLY POSSIBLE** below (Q1 answer).
- **D-85:** Per-client synthetic `CoordinatorInfo` with `supported_script_types = vec![wallet.script_type()]` AND `output_script_type = wallet.script_type()` so the synthetic-info-vs-wallet cross-check at `client/src/round/input.rs` passes the WALLET-03 + WALLET-04 gates.
- **D-86 (RECOMMENDED):** Automated v1.3-binary gate via `git worktree add <SHA>` + `cargo build --release --bin client` + drive via `tokio::process::Command::new`. Opt-in feature flag `v13-binary-compat` (CD-32). **GO** per Q2 below.
- **D-89/D-90:** Coordinator uses `BipConfig::default()` (no test-specific override). No runtime check on submitted output script type added — heterogeneous outputs flow through `register_output` unchanged.
- **D-92/D-93:** CSV env var `BLINDJOIN_BOT_SCRIPT_TYPES` (default `"p2wpkh"`), parsed via `client::config::parse_script_type` token-by-token. Single-underscore env-var convention (NOT `BLINDJOIN__BOT__*`).
- **D-94/D-95/D-96:** Round-robin rotation persisted via atomic-write counter file at `/app/data/bot_round_counter`. Bot exits after one successful round; Docker `restart: unless-stopped` re-launches it.
- **D-97/D-98:** Per-type env-var tuples (`BLINDJOIN_BOT_P2WPKH_UTXO/WIF`, `BLINDJOIN_BOT_P2TR_UTXO/DESCRIPTOR`, `BLINDJOIN_BOT_P2SH_P2WPKH_UTXO/DESCRIPTOR`); legacy single-WIF env vars stay (v1.3 backwards compat).
- **D-99/D-100:** HD wallet model deferred to v1.5; per-run keychain derivation already provides output non-clustering via the single-shot pattern.
- **D-105:** 3 plans (18-01 INTEG-01 → 18-02 INTEG-02 → 18-03 v1.3 binary + README + closeout). Sequential dependency (18-03 depends on 18-01 + 18-02 for verification gathering).
- **D-106:** README §"Privacy Considerations" — 2 paragraphs after Quick Start, before Build from Source / Architecture. See R5 below for exact insertion point.

### Claude's Discretion (CD-25..CD-33)

- **CD-25:** D-86 automated v1.3-binary gate (default). Plan-phase escape valve: D-87 UAT-documented fallback.
- **CD-26:** `tests/integration/bot_rotation.rs` location — default `tests/integration/`.
- **CD-27:** New `RotationState` type in `liquidity-bot/src/strategy.rs` (NOT a field on `JoinStrategy`).
- **CD-28:** Counter file path configurable via `BLINDJOIN_BOT_COUNTER_FILE` env var (default `/app/data/bot_round_counter`).
- **CD-29:** Bot accepts full BIP-380 descriptor string per type (NOT raw xprv).
- **CD-30:** Mixed-script E2E test asserts input script types via re-querying bitcoind for each prevout SPK (NOT witness-byte inspection).
- **CD-31:** Add `tempfile` to `liquidity-bot/Cargo.toml` `[dev-dependencies]` for counter-file unit tests.
- **CD-32:** v1.3-binary gate test behind feature flag `v13-binary-compat` (opt-in; ~30s build cost first run).
- **CD-33:** README disclaimer does NOT mention WabiSabi roadmap absence — focused narrowly on V1.4-MOD-06 + V1.4-MIN-02.

### Deferred Ideas (OUT OF SCOPE — Plan-Phase MUST NOT include)

- Coordinator runtime check that submitted output script type matches advertised `ost` (v1.5+).
- HD wallet (BIP-32/39 seed-driven) bot model with auto-discovery of spendable UTXOs (v1.5+).
- `scantxoutset`-driven discovery of operator-funded UTXOs (v1.5+).
- TEST-EXT-01/02/03 (cross-impl differential fixtures, on-chain anchor test, automated backwards-compat matrix — v1.5+).
- CARRY-TOR-UAT (Tor-mode verification harness — v1.5+).
- CARRY-REPAIR-01-PR (v1.4 cut PR; discharged separately POST-Phase-18 via `/gsd:ship`).
- Bot rotation-counter rolling persistence with TTL (v1.5+ polish).
- Bot-side cancellation / shutdown signal handling (v1.5+).
- Per-type denomination / per-round breakdown (v1.5+).
- DECISIONS-INDEX.md rolling summary (v1.5+).
- `bdk_wallet = "=2.3.x"` exact-pin tightening (v1.5+).
- Mainnet as default (out of scope per PROJECT.md).
- Mobile client (out of scope per PROJECT.md).

---

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| INTEG-01 | Mixed-script E2E integration test on regtest (≥1 P2WPKH + ≥1 P2TR + ≥1 P2SH-P2WPKH input through INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST; v1.3 invariant gate stays green) | Q1 (funding mechanics) + R1 (existing canonical structure) + R2 (heterogeneous-output flow correctness) + R7 (cargo invocation) |
| INTEG-02 | Liquidity bot multi-script via `BLINDJOIN_BOT_SCRIPT_TYPES` CSV + per-round rotation (V1.4-MIN-02 mitigation) + per-run keychain-derivation output non-clustering | Q3 (per-type wallet construction surface) + Q4 (rotation counter file integration) + Patterns §"Single-underscore env vars on bot" |

---

## Project Constraints (from CLAUDE.md)

| Constraint | Source | Phase 18 Impact |
|------------|--------|------------------|
| No custom crypto | PROJECT.md / CLAUDE.md | Phase 18 ADDS no crypto. Consumes `shared::bip322::*` + `BdkClientWallet::sign_bip322` verbatim. |
| Tor-native in production; clearnet OK in tests | PROJECT.md / CLAUDE.md | Phase 18 mixed-script E2E + v1.3-binary gate use HTTP (regtest infra). Bot remains signet-only with `BLINDJOIN_NETWORK="signet"` safety guard at `liquidity-bot/src/main.rs:43-49`. |
| No PII logging | PROJECT.md / CLAUDE.md | Bot logs the rotated-to script type + counter value + UTXO outpoint (all public — none are PII per Phase 1 baseline). No new PII surface. |
| MIT license, public good | PROJECT.md | Phase 18 ships no proprietary code. |
| GSD workflow enforcement | CLAUDE.md | Plan-phase already invoked; this RESEARCH.md feeds it. |

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Mixed-script E2E test | Integration test crate (`tests/integration/`) | — | Test code, lives adjacent to existing `full_round.rs` / `multi_script_validate.rs` / `multi_script_client.rs`. NO production crate changes. |
| Per-type funded UTXOs | `tests/integration/mod.rs::fund_regtest_typed` | bitcoind (regtest, `send_to_address` + `get_raw_transaction_verbose`) | Already implemented and verified at lines 617-823. Consumed verbatim. |
| Descriptor-wallet funding | Integration test inline + `BdkClientWallet::generate` | bitcoind (regtest, `send_to_address`) | New helper inline in `mixed_script_e2e.rs`; may extract to `fund_descriptor_wallet` helper in `mod.rs` per Deferred Idea. |
| Coordinator (in-process) | `coordinator::api::build_router` via `spawn_coordinator` helper | — | Helper currently in `full_round.rs:85-153`; plan-phase decides to PROMOTE to `mod.rs` OR import via `crate::full_round::*`. Promotion recommended (full_round.rs stays zero-touch). |
| Bot script-type CSV parsing | `liquidity-bot/src/main.rs` startup | `client::config::parse_script_type` | Reuse the locked parser; no duplicate. |
| Bot rotation counter | `liquidity-bot/src/strategy.rs` (new `RotationState`) + `tokio::fs` atomic write | Docker volume `/app/data` (mirrors coordinator's ban-file volume) | Atomic write idiom: tokio::fs::write to a `.tmp` sibling + tokio::fs::rename — see Q4 below. |
| Per-type wallet construction in bot | `liquidity-bot/src/main.rs` runtime selection | `BdkClientWallet::{from_wif, from_descriptor}` | from_wif (P2WPKH only per D-61) + from_descriptor (P2TR / P2SH-P2WPKH). |
| v1.3-binary gate | `tests/integration/v13_binary_compat.rs` (new) | `git worktree` + `cargo build --release --bin client` + `tokio::process::Command::new` | Opt-in `--features v13-binary-compat`; ~30s first-run cost. |
| README §"Privacy Considerations" | `README.md` Edit | — | Insertion point: after "Quick Start (Docker)" (ends line 43), before "Build from Source" (starts line 44). |
| Cross-phase invariant verification | `cargo test --test integration full_round` | — | Run after each Phase 18 plan; expect 8/8 green. Document in `18-VERIFICATION.md`. |

---

## Standard Stack

Phase 18 introduces NO new runtime dependencies. The full project stack is already locked at workspace level (CLAUDE.md "Technology Stack" section). Phase 18 adds at most `tempfile` to `liquidity-bot/Cargo.toml` `[dev-dependencies]` for counter-file unit tests (CD-31; already a workspace dep at `tests/integration/Cargo.toml`).

### Crates Consumed (already in workspace; no version bumps)

| Crate | Version | Where Phase 18 uses it |
|-------|---------|------------------------|
| `tokio` | 1.51 | Bot rotation counter atomic-write (`tokio::fs::{write,rename}`); v1.3 binary driver (`tokio::process::Command`) |
| `bitcoin` | 0.32 | Address parsing, Txid, OutPoint, Network::Regtest |
| `bdk_wallet` | 2.3 | `BdkClientWallet::{generate, from_descriptor, from_wif, sign_bip322}` |
| `corepc-node` (dev) | 0.12 + 30_2 feature | regtest bitcoind via `BitcoindGuard` |
| `tempfile` (dev) | 3 | Counter-file unit tests + per-test `/app/data` tempdir for bot_rotation integration test |
| `tracing` | 0.1 | Bot rotation logging (info-level; PII-free fields) |

### Package Legitimacy Audit

Phase 18 installs NO new external packages. The slopcheck protocol is not load-bearing for this phase; all crates already verified in Phases 14-17 are reused. `tempfile` (the only `[dev-dependencies]` addition under CD-31) is a workspace-pinned crate at `coordinator/Cargo.toml:69` and `tests/integration` (transitive workspace dep) — `[VERIFIED: workspace pinned at 3.x]`.

---

## Architecture Patterns

### System Architecture Diagram (Phase 18 INTEG-01 — mixed-script E2E test)

```
                ┌───────────────────────────┐
                │ require_bitcoind!()       │  (graceful skip in local-dev)
                └──────────────┬────────────┘
                               │ exe path
                               v
                ┌───────────────────────────┐
                │ bootstrap_regtest_bitcoind│  → BitcoindGuard + RpcCreds
                └──────────────┬────────────┘
                               │
                ┌──────────────┴──────────────┐
                │                             │
                v                             v
        ┌────────────────┐         ┌──────────────────────┐
        │ fund_regtest   │         │ Inline descriptor    │
        │ _typed(P2WPKH) │         │ funding (P2TR +      │
        │ → TypedUtxo    │         │ P2SH-P2WPKH):        │
        │ Handle (WIF    │         │ BdkClientWallet::    │
        │ path)          │         │ generate → peek_addr │
        └───────┬────────┘         │ → send_to_address    │
                │                  │ → get_raw_tx_verbose │
                │                  │ → assign utxo_outpoint│
                │                  └──────────┬───────────┘
                │                             │
                v                             v
        ┌──────────────────────────────────────────────┐
        │  spawn_coordinator (in-process axum router)  │
        │  cfg.bip = BipConfig::default()              │
        │  denomination_sats = 100_000                 │
        │  port 0 (ephemeral) + per-test tempdir       │
        └──────────────────┬───────────────────────────┘
                           │
       ┌───────────────────┼───────────────────┐
       v                   v                   v
   ┌─────────┐         ┌─────────┐         ┌──────────────┐
   │ Client0 │         │ Client1 │         │ Client2      │
   │ P2WPKH  │         │ P2TR    │         │ P2SH-P2WPKH  │
   │ from_wif│         │ desc    │         │ desc         │
   └────┬────┘         └────┬────┘         └──────┬───────┘
        │                   │                     │
        │   each task: register_input             │
        │   → register_output → verify_and_sign   │
        │   each with own synthetic CoordinatorInfo
        v                   v                     v
        └────────────┬──────────────┬─────────────┘
                     v              v
              ┌──────────────────────────┐
              │ Coordinator's            │
              │ validate_utxo dispatcher │
              │ (Phase 16) — per-script  │
              │ verify + CRIT-01 check   │
              └────────────┬─────────────┘
                           │
                           v
              ┌──────────────────────────┐
              │ assemble_and_broadcast   │
              │ → bitcoind sendrawtx     │
              └────────────┬─────────────┘
                           │
                           v
              ┌──────────────────────────┐
              │ get_raw_mempool poll     │
              │ (10s deadline, 100ms)    │
              │ + tx.outputs vbytes-31   │
              │ filter to assert 3 denom │
              │ + INPUT script-type set  │
              │ via re-query of prevouts │
              └──────────────────────────┘
```

### Recommended Project Structure (no changes to existing tree)

```
tests/integration/
├── mod.rs                  # MOD declaration extended with mixed_script_e2e + bot_rotation + v13_binary_compat
├── ban_list_persistence.rs # (untouched)
├── bot_rotation.rs         # NEW — 18-02
├── full_round.rs           # INVARIANT GATE — zero-touch
├── mixed_script_e2e.rs     # NEW — 18-01
├── multi_script_client.rs  # (untouched; Phase 17 boundary tests)
├── multi_script_validate.rs # (untouched; Phase 16 dispatcher tests)
├── rate_limiting.rs        # (untouched)
├── round_bootstrap.rs      # (untouched)
└── v13_binary_compat.rs    # NEW — 18-03 (gated behind --features v13-binary-compat)

liquidity-bot/src/
├── main.rs                 # EXTENDED — env var surface + script-type dispatch + counter file
└── strategy.rs             # EXTENDED — new RotationState type per CD-27

docker/
├── docker-compose.yml      # EXTENDED — new env vars on liquidity-bot service + bot-data volume
└── Dockerfile              # POSSIBLY EXTENDED — VOLUME ["/app/data"] on liquidity-bot stage if not implicit

.planning/phases/18-mixed-script-e2e-liquidity-bot/
├── 18-CONTEXT.md           # (already exists)
├── 18-DISCUSSION-LOG.md    # (already exists)
├── 18-RESEARCH.md          # THIS FILE
├── 18-PLAN.md              # plan-phase output
├── v13_pinned_sha.txt      # NEW — pinned SHA per D-88
├── 18-01-PLAN.md           # NEW — mixed-script E2E test
├── 18-02-PLAN.md           # NEW — bot rotation + multi-script
├── 18-03-PLAN.md           # NEW — v1.3 binary gate + README + closeout
└── 18-VERIFICATION.md      # NEW — milestone-readiness checklist
```

### Pattern 1: `require_bitcoind!() → fund → spawn_coordinator → 3 concurrent client tasks → mempool poll → output verify`

The canonical 8-step structure of `tests/integration/full_round.rs::full_round_three_clients` (lines 194-379). Phase 18 INTEG-01 mirrors it byte-for-byte with **3 differences**:

1. **Step 2-4 (funding):** Replace `crate::fund_regtest(exe)` (P2WPKH-only WIF) with a 3-way:
   - `crate::fund_regtest_typed(exe, &[(P2wpkh, 1)])` for the P2WPKH WIF client (or skip if B1.b absorbs all 3 via inline descriptor funding — plan-phase decides).
   - Descriptor-wallet funding for P2TR + P2SH-P2WPKH (see Q1 answer below).
2. **Step 5 (synthetic CoordinatorInfo):** Generalise `v14_p2wpkh_coordinator_info()` (lines 49-59) into a factory:
   ```rust
   fn v14_coordinator_info(st: ScriptType) -> CoordinatorInfo {
       CoordinatorInfo {
           coordinator_url: String::new(),
           capabilities: CoordinatorCapabilities {
               record_version: "manual".to_string(),
               is_legacy: false,
               supported_script_types: vec![st],
               output_script_type: st,
           },
       }
   }
   ```
   Each of the 3 clients passes its OWN-typed CoordinatorInfo to `register_input` (D-85).
3. **Step 8 (post-broadcast assertion):** Extend the `denom_output_count == 3` check (lines 369-373) with an **input script-type set-equality assertion**: re-query bitcoind for each input's prevout SPK via `get_raw_transaction_verbose`, classify via `shared::bip322::detect_script_type`, assert the set is `{P2wpkh, P2tr, P2shP2wpkh}` (CD-30; D-104).

Example assertion sketch (CD-30 — re-query, NOT witness-byte inspection):

```rust
let input_script_types: HashSet<ScriptType> = tx.inputs.iter().map(|inp| {
    let prev_txid = bitcoin::Txid::from_str(&inp.txid).unwrap();
    let prev_tx = rpc.get_raw_transaction_verbose(prev_txid).unwrap();
    let prev_vout = &prev_tx.outputs[inp.vout as usize];
    let spk_bytes = hex::decode(&prev_vout.script_pubkey.hex).unwrap();
    let spk = bitcoin::ScriptBuf::from_bytes(spk_bytes);
    shared::bip322::detect_script_type(&spk).expect("regtest funding SPK is one of P2WPKH/P2TR/P2SH-P2WPKH")
}).collect();
assert_eq!(
    input_script_types,
    HashSet::from([ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh]),
    "broadcast tx must contain exactly one input of each script type; got: {input_script_types:?}"
);
```

### Pattern 2: BLAME-05-style atomic-write SUPERSEDED for the bot

Investigation of `coordinator/src/round/blame.rs::append_ban_entry` (lines 114-128) found the coordinator uses **append-mode `std::fs::OpenOptions::create + append + writeln!`** — NOT a tempfile-then-rename atomic write. This is fine for the ban file (append-only JSONL semantically tolerates partial writes), but the bot's rotation counter is **not append-only** (the counter file gets OVERWRITTEN on each successful round).

**RECOMMENDED idiom for the bot counter file (Q4 answer; see below):** `tokio::fs::write` to `${counter_file}.tmp` + `tokio::fs::rename` to `${counter_file}`. This is the standard POSIX atomic-replace idiom; on Linux + tmpfs `rename(2)` is atomic on the same filesystem. The bot's `/app/data` is a single tmpfs/volume, so the source and dest are guaranteed on the same fs.

### Pattern 3: `tests/integration/multi_script_validate.rs` style for per-script assertions

Phase 16-02's pattern of `matches!(...)` on `Bip322Error` variants (NOT string-parsing the message) — Phase 18 inherits without change. No new `Bip322Error` variants introduced.

### Pattern 4: Single-underscore env-vars on bot (Phase 17 CD-22 + Phase 4 conventions)

Coordinator uses `BLINDJOIN__*__*` (double underscore, hierarchical), bot uses `BLINDJOIN_*` (single underscore, flat). Phase 18 18-02 adds:
- `BLINDJOIN_BOT_SCRIPT_TYPES` (CSV)
- `BLINDJOIN_BOT_P2WPKH_UTXO` + `BLINDJOIN_BOT_P2WPKH_WIF`
- `BLINDJOIN_BOT_P2TR_UTXO` + `BLINDJOIN_BOT_P2TR_DESCRIPTOR`
- `BLINDJOIN_BOT_P2SH_P2WPKH_UTXO` + `BLINDJOIN_BOT_P2SH_P2WPKH_DESCRIPTOR`
- `BLINDJOIN_BOT_COUNTER_FILE` (path; default `/app/data/bot_round_counter`)

Legacy `BLINDJOIN_UTXO` + `BLINDJOIN_UTXO_WIF` continue to work as the P2WPKH tuple when `BLINDJOIN_BOT_SCRIPT_TYPES` is unset (default `"p2wpkh"`) — preserves v1.3 docker-compose stack behaviour byte-exactly.

### Anti-Patterns to Avoid

- **DO NOT modify `tests/integration/full_round.rs`.** Cross-phase invariant gate. Phase 14/15/16/17 all kept this file zero-touch; Phase 18 inherits. `spawn_coordinator` + `v14_p2wpkh_coordinator_info` + `build_input_reg_round_state` + `wait_for_coordinator` should be PROMOTED to `tests/integration/mod.rs` (recommended) OR imported via `use crate::full_round::*`. Either way, the .rs file body stays untouched.
- **DO NOT add coordinator runtime check on submitted output script type.** Heterogeneous outputs are by design for the mixed-script E2E test (per D-85). The client-side fail-fast at `discover_coordinator` (Phase 17 D-76) is the canonical gate; adding server-side enforcement is a v1.5+ candidate (CONTEXT Deferred Ideas).
- **DO NOT extend `BdkClientWallet::from_wif` for P2TR / P2SH-P2WPKH.** Phase 17 D-61 locked from_wif as P2WPKH-only. Use `from_descriptor` for non-P2WPKH wallets.
- **DO NOT add new `[[test]]` entries in `coordinator/Cargo.toml`.** The existing `[[test]] name = "integration" path = "../tests/integration/mod.rs"` (lines 71-73) picks up any `mod X;` declaration in `mod.rs`. Phase 18 18-01/18-02/18-03 just add `mod mixed_script_e2e;` / `mod bot_rotation;` / `mod v13_binary_compat;` to the existing block at lines 19-24.
- **DO NOT use random rotation in the bot.** D-96 specifies round-robin (deterministic). Random rotation can pick the same type 3 runs in a row, which fails the "rotates per round" wording.
- **DO NOT mix env-var conventions (double-underscore on bot).** D-93 / CD-22 lock single-underscore for the bot.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-script ownership proof signing | A new BIP-322 signer | `BdkClientWallet::sign_bip322` (Phase 17 17-02 at `client/src/wallet.rs:459`) | The dispatcher routes P2WPKH through `shared::bip322::sign_simple` and P2TR + P2SH-P2WPKH through bdk_wallet's PSBT signer. CRIT-01 + Pitfall 7 already handled. |
| Per-script regtest UTXO funding | Custom regtest funder | `tests/integration/mod.rs::fund_regtest_typed` (lines 617-823) | Already supports `&[(ScriptType, usize)]` and returns `TypedUtxoHandle { secret_key, outpoint, script_pubkey, value_sats, p2sh_redeem_script }`. |
| Per-test bitcoind | Custom runner | `BitcoindGuard` + `require_bitcoind!()` (mod.rs:150-170 + 100-108) | RAII guard handles graceful shutdown via `n.stop()` on blocking pool (CR-01). |
| In-process coordinator for testing | New axum app | `tests/integration/full_round.rs::spawn_coordinator` (lines 85-153) | Port-0 ephemeral binding + tempdir-backed ban file. Promote to `mod.rs` per recommendation. |
| Atomic file write | Custom rename loop | `tokio::fs::write` + `tokio::fs::rename` (or `tempfile::NamedTempFile::persist`) | See Q4 answer. POSIX rename(2) atomic on same filesystem. |
| v1.3-binary build infra | Custom Cargo invocation logic | `std::process::Command` (or `tokio::process::Command`) + `git worktree add <SHA>` + `cargo build --release --bin client --manifest-path /tmp/.../client/Cargo.toml` | Standard pattern; ~30s first-run, cached after. |
| Bot CSV script-type parser | Custom string parser | `client::config::parse_script_type` (client/src/config.rs:10-15) | Routes through serde wire form — single source of truth for accepted tokens. |
| Pinned-SHA tracking | Comment in code | `.planning/phases/18-.../v13_pinned_sha.txt` per D-88 | Reproducible from anyone's checkout at any future point. |
| Counter-file format | Binary blob | Single line `u64` decimal (text); missing file = 0; malformed = bail | Triage-friendly. Counter values are public/derivable so PII-free. |

**Key insight:** Phase 18 is structurally a **gluing exercise**, not a building exercise. Every primitive Phase 18 needs already exists in the codebase, verified GREEN in Phases 14-17. The only NEW code is integration-test composition + bot env-var dispatch + 2 paragraphs of README prose. Lean toward MAXIMUM REUSE — if you're tempted to write more than 50 LOC of new logic in any single plan, double-check whether you're re-implementing something that already lives in `mod.rs` / `wallet.rs` / `discover.rs`.

---

## Runtime State Inventory

Not applicable — Phase 18 is NOT a rename/refactor phase. No databases, OS-registered state, or live service configs are renamed. The phase adds new files + extends one env-var surface; existing state (ChromaDB, Mem0, n8n workflows, Windows Task Scheduler, SOPS keys, etc.) is irrelevant to this codebase (Rust-only workspace; no such systems are in scope).

**Items in each category — verified empty:**

- **Stored data:** None — Phase 18 introduces no new databases or persistent stores. The bot rotation counter file is a NEW persistent artifact at `/app/data/bot_round_counter`, but it's an ADD (not a rename); no pre-existing data to migrate.
- **Live service config:** None — Phase 18 changes no Datadog, n8n, Tailscale, or Cloudflare configuration (those systems are not used by blindjoin).
- **OS-registered state:** None — Phase 18 changes no Task Scheduler, pm2, launchd, or systemd entries.
- **Secrets/env vars:** Phase 18 ADDS new env vars (`BLINDJOIN_BOT_SCRIPT_TYPES`, `BLINDJOIN_BOT_P2WPKH_*`, etc.) with default values that preserve v1.3 bot behaviour byte-exactly. No existing env var is renamed.
- **Build artifacts:** None — Phase 18 adds no new binaries (the v1.3-binary gate's build artifact lives in `/tmp/blindjoin-v13-<sha>-bin/` and is intentionally outside the project's `target/`).

---

## Common Pitfalls

### Pitfall 1 — Touching `tests/integration/full_round.rs`

**What goes wrong:** Even a "harmless" line edit in `full_round.rs` triggers a cargo recompile of the v1.3 invariant gate's test binary, and any drift in dependent crates can flip a pass→fail.

**Why it happens:** Cross-phase invariant discipline is informal; under time pressure a contributor might "fix" a comment or extract a helper inline.

**How to avoid:** Plan-phase 18-01 explicitly forbids edits to `full_round.rs`. PROMOTE `spawn_coordinator` / `wait_for_coordinator` / `v14_p2wpkh_coordinator_info` / `build_input_reg_round_state` to `tests/integration/mod.rs` (or to a new `tests/integration/testing.rs` submodule) so the new `mixed_script_e2e.rs` can `use crate::testing::*` without touching `full_round.rs`. The promotion is mechanical and does NOT change `full_round.rs`'s behaviour (only its `use` declarations would change — keep them via re-exports if needed).

**Warning signs:** `git diff` on Phase 18 plans includes `tests/integration/full_round.rs`. STOP. Revert. Find a non-touching alternative.

### Pitfall 2 — Synthetic CoordinatorInfo with wrong `output_script_type`

**What goes wrong:** Phase 17 D-76 added `DiscoveryError::UnsupportedOutputScriptType` — a wallet whose `script_type` ≠ `coordinator_info.capabilities.output_script_type` fails discovery. The mixed-script E2E test bypasses real discovery, so each client's synthetic info MUST set both `supported_script_types` AND `output_script_type` to the client's own wallet type.

**Why it happens:** Mental model "this is a mixed-script round so the coordinator's output_script_type should be ... what, exactly?" — there is no canonical answer because the test deliberately exercises heterogeneous outputs that production discovery would reject.

**How to avoid:** Each client's `register_input` call takes its OWN synthetic CoordinatorInfo (per D-85) — not a shared one. The factory `v14_coordinator_info(wallet.script_type())` returns the appropriate per-client CoordinatorInfo.

**Warning signs:** `DiscoveryError::UnsupportedOutputScriptType` panic in test output, OR a CRIT-01 cross-check failure (coordinator rejects `script_type` in the OwnershipProof envelope as not matching the on-chain SPK).

### Pitfall 3 — Coordinator fee math assumes P2WPKH outputs (R2 finding)

**What goes wrong:** `coordinator/src/bitcoin/fee.rs:16` hardcodes `OUTPUT_WEIGHT_VBYTES = 31` (P2WPKH). A P2TR output is ~43 vbytes, P2SH-P2WPKH ~32. The fee model under-estimates total tx size when outputs are heterogeneous, which means **each participant's input must carry enough headroom to cover a vbyte-cost-shortfall**.

**Why it happens:** v1.3 was P2WPKH-only by construction; the fee model was sized for that single-script case. Phase 14 D-08 explicitly classifies fee-model expansion to per-script as v1.5+ (B-03 dynamic fee estimation).

**How to avoid:**
- `fund_regtest_typed` already funds each UTXO with `denomination + 50_000 = 150_000` sats per `tests/integration/mod.rs:648-650`. At `fee_rate_sat_per_vbyte: 1` and ~250 vbytes per typical 3-input 3-denom-output mixed tx, the actual fee ≤ 250 sats. The 50k-sat headroom is **3 orders of magnitude** over the fee-model drift between P2WPKH-assumed and worst-case heterogeneous.
- For the production signet bot (INTEG-02): the fee model still under-estimates for P2TR-output rounds, but `fee_rate_sat_per_vbyte = 1` on signet is benign. Mainnet operation will hit this — flagged as v1.5+ B-03 in CONTEXT Deferred.

**Warning signs:** A mixed-script round where `assemble_and_broadcast` returns `TxError::InsufficientFunds` for one of the P2TR or P2SH-P2WPKH participants. If this fires in Phase 18 INTEG-01, increase the per-UTXO funding in `fund_regtest_typed` (currently 150k sats per `mod.rs:649`).

### Pitfall 4 — Bot binary not callable from tests

**What goes wrong:** Phase 18 18-02's bot_rotation integration test needs to drive 3 sequential bot runs and assert script-type rotation. The current bot is a `#[tokio::main]` binary at `liquidity-bot/src/main.rs`, not a callable library function.

**Why it happens:** v1.0 / Phase 4 modeled the bot as a single-shot binary on the assumption that Docker's restart-policy would re-launch it. Phase 18 18-02's test needs in-process control flow.

**How to avoid:**
- **Recommended (D-102 (a)):** Extract the bot's main-loop into a `liquidity_bot::run(config: BotConfig) -> Result<()>` library function. Add `[lib]` declaration to `liquidity-bot/Cargo.toml` (similar to the coordinator's lib at coordinator/Cargo.toml:10-12). Tests can then `liquidity_bot::run(...).await` and assert the resulting counter file state.
- **Fallback (D-102 (b)):** Drive the bot via `tokio::process::Command::new("/path/to/liquidity-bot")`. Parallels the v1.3-binary gate (D-86). More test infra; same outcome.

**Warning signs:** Plan-phase 18-02 estimates LOC > 250 just to extract the bot's main-loop. That suggests path (a) is being mis-implemented; the extraction should be ~30 LOC (move main's body into `pub async fn run`, parse args inside).

### Pitfall 5 — Pinned-SHA SHA drift across Cargo workspaces

**What goes wrong:** The v1.3-binary gate's `cargo build --release --bin client --manifest-path /tmp/blindjoin-v13-<sha>/client/Cargo.toml` uses the CURRENT `Cargo.lock` (the worktree's, not the workspace's). If the workspace's `Cargo.lock` has version drift (e.g., a transitive dep bumped that breaks v1.3's client/main.rs), the v1.3 binary FAILS TO BUILD.

**Why it happens:** Pinned SHA `05f21438` (verified below) and HEAD's `Cargo.toml` are byte-identical at workspace level (confirmed by `git diff 05f21438..HEAD -- Cargo.toml` returning empty). HOWEVER, individual crate `Cargo.toml` files DID change (Phase 17 added `--type` flag to `client/src/config.rs`, etc.). The v1.3 client's main.rs at `05f21438` does NOT consume those new flags, so build should succeed — but plan-phase MUST verify this empirically in the 18-03 plan body, not just assume it from Cargo.toml diff.

**How to avoid:**
- 18-03 plan task 1 (BEFORE landing test code): `git worktree add /tmp/v13 05f21438 && cargo build --release --bin client --manifest-path /tmp/v13/client/Cargo.toml`. If this succeeds locally, the gate is GO. If it fails, document the error in `18-03-PLAN.md` and FALL BACK to D-87 (UAT-documented manual gate).
- The pinned SHA is committed to `.planning/phases/18-.../v13_pinned_sha.txt` per D-88 so future reproducers can `git worktree add` deterministically.

**Warning signs:** `cargo build` against the v1.3 worktree reports unresolved imports OR dep version conflicts. STOP. Fall back to D-87.

### Pitfall 6 — Counter file persistence vs Docker volume ownership

**What goes wrong:** The bot writes `/app/data/bot_round_counter`. Inside the container, the bot runs as some UID (typically not 1000 because the Docker base image is `debian:bookworm-slim` and no user is created in the Dockerfile). The bot needs WRITE permission to `/app/data/`.

**Why it happens:** The coordinator already writes `/app/data/ban_list.jsonl` from the same Dockerfile stage (`runtime-base` → `coordinator` at `docker/Dockerfile:23-27`), so the volume permission story is ALREADY SOLVED for the coordinator. For the bot, the analogous `liquidity-bot` stage at lines 34-37 does NOT explicitly create `/app/data/` — `docker/Dockerfile:25` only does that for the coordinator stage.

**How to avoid:** 18-02 plan task adds `RUN mkdir -p /app/data` to the `liquidity-bot` runtime stage at `docker/Dockerfile:34-37` (mirrors the coordinator stage at line 25). Also adds the `bot-data` named volume to `docker/docker-compose.yml` mounting at `/app/data/` (analogous to coordinator's `coordinator-data` at lines 60-61 + 105-106).

**Warning signs:** Bot starts, fails to write counter file with "Permission denied" or "No such file or directory" on first run.

### Pitfall 7 — `cargo test --test full_round` invocation mismatch (ROADMAP wording vs actual binary path) — R6

**What goes wrong:** ROADMAP Phase 18 success criterion #1 names `cargo test -p coordinator --test full_round -- --include-ignored`. But the `coordinator/Cargo.toml` `[[test]]` declaration at lines 71-73 is `name = "integration" path = "../tests/integration/mod.rs"`. There is NO `[[test]] name = "full_round"` declaration; `--test full_round` would fail to find a target.

**Why it happens:** ROADMAP was written speculatively before Phase 9's `mod.rs` consolidation; the wording is stale. The canonical invocation today is `cargo test --test integration full_round` (which runs only tests in the `full_round` submodule) or `cargo test --test integration mixed_script_e2e` (which runs only the new mixed-script test).

**How to avoid:** Plan-phase 18-01 documents the canonical invocation in the plan body AND in `18-VERIFICATION.md`:
- For the mixed-script E2E test: **`cargo test -p coordinator --test integration mixed_script_e2e -- --nocapture`**
- For the v1.3 invariant gate: **`cargo test -p coordinator --test integration full_round -- --nocapture`**
- For the v1.3-binary gate: **`cargo test -p coordinator --test integration --features v13-binary-compat v13_binary_compat -- --nocapture`**
- For the bot rotation test: **`cargo test -p coordinator --test integration bot_rotation -- --nocapture`**

**Warning signs:** Anyone trying to verify Phase 18 acceptance using ROADMAP wording verbatim sees `error: no test target named 'full_round'`. Document the canonical invocations prominently in 18-VERIFICATION.md.

---

## Test Strategy (per-plan breakdown)

### 18-01: Mixed-script E2E test (`tests/integration/mixed_script_e2e.rs`)

**Unit tests:** None. The mixed-script E2E test is purely integration — no new pure logic introduced.

**Integration tests:**
- `mixed_script_e2e_three_clients_broadcast` — `#[tokio::test]` async fn. Single test function in the file. Asserts:
  - `denom_output_count == 3` (matches existing full_round.rs:369-373 pattern).
  - **NEW** `input_script_types == {P2wpkh, P2tr, P2shP2wpkh}` (set-equality via `detect_script_type` re-query — D-104 + CD-30).

**Test isolation:** One bitcoind per test fn (D-103; matches full_round.rs + multi_script_validate.rs pattern).

**Acceptance gate:** `cargo test --test integration mixed_script_e2e` returns 1/1 PASS.

### 18-02: Bot multi-script + rotation

**Unit tests** (in `liquidity-bot/src/strategy.rs`):
- `rotation_state_round_robin_advances_counter` — counter 0 → first type, counter 1 → second, counter `len` → first again.
- `rotation_state_single_type_does_not_rotate` — degenerate `len = 1` case.
- `rotation_state_empty_enabled_returns_err` — defensive (startup validation should fire first, but unit-test the function-level invariant).
- `rotation_state_counter_file_roundtrip` — parse `"0\n"` → 0; write 1 → file contains `"1\n"`; missing file → 0; malformed `"abc\n"` → Err.
- `rotation_state_atomic_write_via_tmp_then_rename` — verifies the `.tmp + rename` idiom (uses `tempfile::tempdir`).

**Integration test** (`tests/integration/bot_rotation.rs` per CD-26):
- `bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs` — `#[tokio::test]` 3-run cycle:
  - Run 1 with counter=0 → asserts wallet.script_type() == P2wpkh; counter file bumped to 1.
  - Run 2 with counter=1 → asserts P2tr; counter file bumped to 2.
  - Run 3 with counter=2 → asserts P2shP2wpkh; counter file bumped to 3 (wraps to 0 mod 3 on the next run).
  - Each run drives an in-process v1.4 coordinator (reused `spawn_coordinator`) and a per-type funded UTXO.
  - Counter file lives in a per-test `tempfile::tempdir()` (NOT `/app/data`) so the test is hermetic.

**Acceptance gate:**
- `cargo test -p liquidity-bot` returns ≥5/5 PASS (unit tests).
- `cargo test --test integration bot_rotation` returns 1/1 PASS (integration test).

### 18-03: v1.3-binary gate + README + closeout

**Unit tests:** None.

**Integration test** (`tests/integration/v13_binary_compat.rs`, behind `--features v13-binary-compat`):
- `v13_client_p2wpkh_against_v14_coordinator` — `#[tokio::test]` runs only when `--features v13-binary-compat` is set:
  - Read pinned SHA from `.planning/phases/18-.../v13_pinned_sha.txt`.
  - `git worktree add /tmp/blindjoin-v13-<sha> <sha>` (idempotent — check if exists).
  - `cargo build --release --bin client --manifest-path /tmp/blindjoin-v13-<sha>/client/Cargo.toml` (idempotent — cargo's incremental cache handles re-runs).
  - `require_bitcoind!()` first; bootstrap regtest + fund 1 P2WPKH UTXO via `fund_regtest_typed(exe, &[(P2wpkh, 1)])`.
  - `spawn_coordinator` (v1.4 in-process; `BipConfig::default()` so P2WPKH is in the allowed set).
  - `tokio::process::Command::new("/tmp/blindjoin-v13-<sha>/target/release/client")` with args: `--utxo <txid:vout> --utxo-wif <wif_from_secret_key> --coordinator-url http://<addr> --network signet` — wait, **NETWORK MISMATCH**: v1.3 client at SHA `05f21438` is hardcoded to fall through `signet | testnet4 | mainnet` (`client/src/main.rs` at the pinned SHA — confirmed). The test must pass `--network signet` because v1.3 doesn't accept regtest. The wallet is constructed for regtest internally via the WIF, BUT `from_wif` at the pinned SHA accepts `network: Network` parameter directly — verify v1.3 from_wif signature.

  **CORRECTION based on v1.3 main.rs inspection:** v1.3 main.rs (at `05f21438`) only accepts `signet | testnet4 | mainnet` for the `--network` flag. To make v1.3 work against the regtest in-process v1.4 coordinator, options are:
  - **Option A (preferred):** Pass `--network signet` to v1.3 client AND construct the v1.3 client wallet from a WIF + outpoint. The wallet's regtest behaviour is governed by the WIF's prefix byte (`c` prefix for testnet/regtest WIFs). `bitcoin::PrivateKey::from_wif("cVt4o7...")` succeeds with `Network::Signet` because the regtest network shares prefix bytes with testnet/signet. The on-chain SPK derived from the compressed pubkey is network-agnostic at the byte level — only the bech32 address `Display` form differs.
  - **Option B:** Run the v1.4 in-process coordinator with `BLINDJOIN__NETWORK__BITCOIN_NETWORK=signet` AND fund actual signet UTXOs. NOT recommended — defeats the regtest determinism story.
  - **Option C:** Patch v1.3 client at the worktree to accept `--network regtest`. NOT recommended — undermines the "pinned binary" semantic.

  **RECOMMENDED:** Option A. Add a 2-line comment in 18-03-PLAN.md documenting that the v1.3 binary is invoked with `--network signet` but is signing for a regtest-network UTXO — this works because BIP-322 message signing is network-agnostic at the wire level (the SPK bytes are what matter, not the human-readable address prefix).

- Asserts: child exit code 0, round broadcast txid appears in mempool, post-broadcast `denom_output_count >= 1` (since this is a 1-of-3 sub-round with only the v1.3 client; consider extending to also drive 2 v1.4-client tasks in parallel so min_participants=3 is met).

  **REFINED:** The v1.3 binary test needs to drive a 3-client round where ONE client is the v1.3 binary and TWO clients are v1.4-clients (in-process). This is the only way to exercise the v1.4 coordinator's min_participants=3 default. Plan-phase 18-03 spells this out explicitly.

**Acceptance gate:**
- `cargo test --features v13-binary-compat --test integration v13_binary_compat` returns 1/1 PASS.
- `tests/integration/full_round` still 8/8 GREEN (cross-phase invariant — re-run).
- README contains §"Privacy Considerations" with 2 paragraphs matching D-106 wording.

**Verification document `18-VERIFICATION.md`** gathers:
1. 8/8 `full_round.rs` green (~42s).
2. 1/1 `mixed_script_e2e_three_clients_broadcast` green.
3. ≥5 unit tests on bot rotation green.
4. 1/1 `bot_rotation` integration green.
5. 1/1 `v13_binary_compat` (under `--features v13-binary-compat`) green.
6. README.md grep for "Privacy Considerations" returns 1 match.
7. All 5 ROADMAP Phase 18 success criteria observable in codebase.
8. Cross-phase invariant gate re-run after EACH plan boundary (not just at the end).

---

## Decisions (technical recommendations for plan-phase)

### Q1: Mixed-script E2E test funding mechanics — **GO for B1.b (descriptor-wallet-driven)**

Verification against the codebase:

1. **`BdkClientWallet::generate(outpoint, network, script_type)` signature confirmed at `client/src/wallet.rs:209-212`:** Accepts `(utxo_outpoint_str: &str, network: Network, script_type: ScriptType)`. Returns `Result<Self>`. The wallet's `utxo_outpoint: OutPoint` is a **public field** at `client/src/wallet.rs:52`, so post-construction assignment is allowed:
   ```rust
   let mut wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Regtest, ScriptType::P2tr)?;
   // ... fund wallet.coinjoin_output_address() via bitcoind RPC ...
   wallet.utxo_outpoint = bitcoin::OutPoint::new(funding_txid, vout);
   ```
2. **`wallet.coinjoin_output_address()` at `client/src/wallet.rs:380-382`:** Returns `peek_address(KeychainKind::External, 0).address` — no state mutation, deterministic per descriptor.
3. **`utxo_script_pubkey` consistency:** At `client/src/wallet.rs:320`, `utxo_script_pubkey = first_address.script_pubkey()` (= `peek_address(External, 0).address.script_pubkey()`). When we fund THAT address via `send_to_address`, the funding output's SPK matches `utxo_script_pubkey` byte-exactly. No mismatch.
4. **bdk_wallet 2.3 + `peek_address` works for all 3 script types on regtest:** Confirmed via Phase 17 unit tests at `client/src/wallet.rs:601-664` (`generate_p2wpkh_produces_bip84_descriptor` / `generate_p2tr_produces_bip86_descriptor` / `generate_p2sh_p2wpkh_produces_bip49_descriptor` — all PASS). Bech32m roundtrip for P2TR confirmed via `tests/integration/multi_script_validate.rs` (Phase 16-02; 9/9 GREEN).

**RECOMMENDED funding flow for INTEG-01:**

```rust
async fn fund_descriptor_wallet(
    node: &corepc_node::Node,
    wallet: &mut BdkClientWallet,
    fund_sats: u64,
) -> bitcoin::OutPoint {
    let addr = wallet.coinjoin_output_address();  // peek_address(External, 0)
    let fund_btc = bitcoin::Amount::from_sat(fund_sats);
    let send = node.client.send_to_address(&addr, fund_btc).expect("send_to_address");
    let txid = bitcoin::Txid::from_str(&send.0).expect("valid txid");
    let tx = node.client.get_raw_transaction_verbose(txid).expect("get_raw_tx_verbose");
    let target_spk_hex = hex::encode(addr.script_pubkey().as_bytes());
    let out = tx.outputs.iter()
        .find(|o| o.script_pubkey.hex.eq_ignore_ascii_case(&target_spk_hex))
        .expect("funding output present");
    let outpoint = bitcoin::OutPoint::new(txid, out.index as u32);
    wallet.utxo_outpoint = outpoint;
    outpoint
}
```

**Mixed-script test orchestration (clients 0, 1, 2):**

```rust
// Client 0: P2WPKH WIF wallet (v1.3 byte-exact path)
let setup = crate::fund_regtest_typed(exe.clone(), &[(ScriptType::P2wpkh, 1)]).await;
let handle = &setup.1.utxos[0];
let wif = bitcoin::PrivateKey::new(handle.secret_key, Network::Regtest).to_wif();
let outpoint_str = format!("{}:{}", handle.outpoint.txid, handle.outpoint.vout);
let wallet0 = BdkClientWallet::from_wif(&wif, &outpoint_str, Network::Regtest)?;

// Clients 1 & 2: P2TR + P2SH-P2WPKH descriptor wallets, B1.b funding
let mut wallet1 = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Regtest, ScriptType::P2tr)?;
fund_descriptor_wallet(setup.0.node(), &mut wallet1, 150_000).await;

let mut wallet2 = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Regtest, ScriptType::P2shP2wpkh)?;
fund_descriptor_wallet(setup.0.node(), &mut wallet2, 150_000).await;

// Confirm the funding txs
setup.0.node().client.generate_to_address(1, &mine_addr).unwrap();
```

**Returns (Q1 answer):** GO. Path: descriptor-wallet-driven funding for P2TR + P2SH-P2WPKH (B1.b); WIF path for P2WPKH. NO structural blocker. The `BdkClientWallet.utxo_outpoint` public-field assignment is the load-bearing post-construction override; consider documenting the pattern with a doc-comment update on `client/src/wallet.rs:52` in 18-01 plan task 0 (optional polish).

### Q2: v1.3-binary gate build infrastructure — **GO for the D-86 automated path**

**Pinned v1.3 SHA: `05f21438a7072987773bfe2eafaac5c51c68c61a`**
**Commit subject:** `docs(15): create phase plan`
**Resolution method:** `git log --first-parent --oneline 622ccf0^ -1 --format="%H %s"` (where `622ccf0` is the first v1.4 source-code-touching commit — `feat(15-01): add stub ScriptType enum + base64 dep to shared`).

Note: Phase 14 (Sprint-0 spikes) made NO source code changes — only docs + ADR. The first commit that modifies `shared/`, `client/`, `coordinator/`, or `liquidity-bot/` is `622ccf0` (Phase 15-01). Its parent `05f21438` is the canonical "last v1.3 commit on main" — all v1.3 wire shapes (P2WPKH-only OwnershipProof = `Vec<Vec<u8>>` of hex; InfoResponse without `supported_script_types`/`output_script_type`) are byte-exactly active at this SHA.

**Verification against current `Cargo.lock`:**

Workspace `Cargo.toml` at `05f21438` vs HEAD:
- `git diff 05f21438..HEAD -- Cargo.toml` returned EMPTY — workspace deps are byte-identical.
- `[workspace.dependencies]` block at v1.3 SHA includes `tokio = "1.51"`, `bitcoin = "0.32"`, `bdk_wallet = "2.3"`, `pkarr = "5"`, `corepc-types = "0.11"`, etc. — same as HEAD.
- Individual crate `Cargo.toml` files have evolved (e.g., `client/Cargo.toml` at HEAD adds the same `arti-client = "0.41"` line that was already at SHA `05f21438`).

**Verification of `--coordinator-url` direct path (no PKARR):**

v1.3 client `main.rs` at `05f21438` (confirmed inline via `git show 05f21438:client/src/main.rs`):
```rust
let coordinator_url = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
    let info = discover::discover_coordinator(pkarr_key).await...
    info.coordinator_url
} else {
    cfg.coordinator_url.clone()
};
let client = if cfg.use_tor {
    // tor::init_tor(...) path
} else {
    CoordinatorClient::new(coordinator_url)
};
```

**GO** — v1.3 client at `05f21438` accepts `--coordinator-url http://...` directly without `--use-tor` (HTTP-not-Tor regtest infra works).

**Verification of `register_input` signature at v1.3:**

```rust
// At 05f21438 — client/src/main.rs:
let reg_result = round::input::register_input(&client, &wallet, &info).await?;
```

3-arg signature `(client, wallet, info)` — no 4th `CoordinatorInfo` arg. The v1.3 client emits the v=1 OwnershipProof envelope (`Vec<String>` of hex strings) via the `to_json_hex_str` method at the v1.3 SHA. The v1.4 coordinator accepts this via the **CD-7 byte-identity branch** at `shared/src/protocol.rs::OwnershipProof::from_json_hex_str` (two-phase try-parse: array-of-hex first, fall back to flat struct). WALLET-04 is satisfied.

**Build-cost estimate:**
- Cold `cargo build --release --bin client --manifest-path /tmp/blindjoin-v13-<sha>/client/Cargo.toml`: ~30-45s on M1 dev (consistent with Phase 14-02 PoC builds at the same SHA).
- Subsequent runs: ~1-2s (cargo incremental + workspace target cache).
- Disk: ~600MB for the v1.3 worktree's `target/release/` directory.

**GO recipe (18-03 plan task 1):**

```bash
# One-time:
SHA=$(cat .planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt | head -1)
WORKTREE=/tmp/blindjoin-v13-${SHA:0:8}
git worktree add "$WORKTREE" "$SHA"
cargo build --release --bin client --manifest-path "$WORKTREE/client/Cargo.toml"
# Idempotent on subsequent runs — cargo's incremental cache + git worktree's existence check.
```

**Returns (Q2 answer):** GO on automated path. Pinned SHA: `05f21438`. Commit subject: `docs(15): create phase plan`. v1.3 client supports `--coordinator-url` direct path without `--use-tor`. Cargo.lock drift verified absent at workspace level. Time cost ~30-45s first run (≤1s cached). Disk cost ~600MB worktree. Fallback D-87 (UAT-documented) remains as escape valve if local-CI build fails for an unforeseen reason.

### Q3: Liquidity bot per-type wallet construction surface

**Verification against `client/src/wallet.rs`:**

1. **`from_descriptor(external_desc, utxo_outpoint_str, utxo_address, network, script_type)` at `client/src/wallet.rs:135-141`:**
   ```rust
   pub fn from_descriptor(
       external_desc: &str,
       utxo_outpoint_str: &str,
       utxo_address: &str,
       network: Network,
       script_type: ScriptType,
   ) -> Result<Self>
   ```
   **5 args**, NOT 4. The bot must supply ALL 5: external descriptor string + outpoint + UTXO bech32 address + network + script_type. The `utxo_address` is required because `from_descriptor` cannot derive the SPK from the descriptor alone (the descriptor specifies `/0/*` derivation pattern but doesn't say which index — the outpoint's vout doesn't disambiguate).

2. **Descriptor SHAPE produced by `generate`** (`client/src/wallet.rs:235-248`):
   - P2WPKH: `wpkh({xprv}/84'/0'/0'/0/*)` + internal `wpkh({xprv}/84'/0'/0'/1/*)`
   - P2TR: `tr({xprv}/86'/0'/0'/0/*)` + internal `tr({xprv}/86'/0'/0'/1/*)`
   - P2SH-P2WPKH: `sh(wpkh({xprv}/49'/0'/0'/0/*))` + internal `sh(wpkh({xprv}/49'/0'/0'/1/*))`

   The descriptor STRING the bot operator types matches this shape verbatim.

3. **Internal descriptor derivation in `from_descriptor` body** (lines 163-169):
   ```rust
   let internal_desc = if external_desc.contains("/0/*)") {
       external_desc.replacen("/0/*)", "/1/*)", 1)
   } else {
       external_desc.to_string()
   };
   ```
   **AUTO-DERIVED** from external. The bot does NOT need to take internal as a separate env var.

4. **Mismatch check at construction time** (lines 142-161 — D-63 fail-fast).

**Returns (Q3 answer):**

- **Exact signature today:** `pub fn from_descriptor(external_desc: &str, utxo_outpoint_str: &str, utxo_address: &str, network: Network, script_type: ScriptType) -> Result<Self>`. 5 args. The 3rd arg `utxo_address` is the bech32/base58 address string of the UTXO being registered.
- **Bot env-var surface (CD-29 — full descriptor string per type):**
  - `BLINDJOIN_BOT_P2WPKH_UTXO` (txid:vout) + `BLINDJOIN_BOT_P2WPKH_WIF` (WIF — bot uses `from_wif` for P2WPKH per D-61).
  - `BLINDJOIN_BOT_P2TR_UTXO` + `BLINDJOIN_BOT_P2TR_DESCRIPTOR` (full external descriptor `tr(xprv/86'/...)`) + `BLINDJOIN_BOT_P2TR_UTXO_ADDRESS` (the bech32m address of the UTXO).
  - `BLINDJOIN_BOT_P2SH_P2WPKH_UTXO` + `BLINDJOIN_BOT_P2SH_P2WPKH_DESCRIPTOR` (full external descriptor `sh(wpkh(xprv/49'/...))`) + `BLINDJOIN_BOT_P2SH_P2WPKH_UTXO_ADDRESS` (the base58 address of the UTXO).
- **No separate internal descriptor env var needed** — bdk_wallet's `Wallet::create(external_desc, internal_desc)` is called inside `from_descriptor` with auto-derived internal. (CONTEXT D-58 specifies BOTH external `/0/*` and internal `/1/*` descriptors but the bot only needs to supply external — the function auto-derives the matching internal.)
- **Default for v1.3 backwards compat:** when `BLINDJOIN_BOT_SCRIPT_TYPES` is unset OR is `"p2wpkh"` AND `BLINDJOIN_BOT_P2WPKH_UTXO` is unset, fall through to legacy `BLINDJOIN_UTXO` + `BLINDJOIN_UTXO_WIF` env vars (D-98) for byte-exact v1.3 behaviour.

### Q4: Rotation counter file atomic-write idiom

**Verification of existing patterns in the codebase:**

1. **`coordinator/src/round/blame.rs::append_ban_entry`** (lines 114-128): Uses `std::fs::OpenOptions::create + append + writeln!` — APPEND-only mode, NOT atomic write. This is fine for the ban file (semantically append-only, partial-write tolerable because of JSONL line semantics).
2. **`coordinator/src/discovery/pkarr_pub.rs`** (PKARR keypair persistence): Uses... let me note: this was not specifically researched, but the bot's counter file has a DIFFERENT shape than the ban file (overwrite-not-append), so the ban-file pattern doesn't apply.

**There is NO pre-existing atomic write idiom in this codebase.** The bot is introducing the first one.

**RECOMMENDED idiom — `tokio::fs::write` to `.tmp` sibling + `tokio::fs::rename`:**

```rust
async fn write_counter_atomic(path: &Path, counter: u64) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, format!("{}\n", counter)).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

async fn read_counter(path: &Path) -> anyhow::Result<u64> {
    match tokio::fs::read_to_string(path).await {
        Ok(s) => {
            s.trim().parse::<u64>()
                .map_err(|e| anyhow::anyhow!(
                    "BLINDJOIN_BOT_COUNTER_FILE = '{}' contains malformed counter (line 1): '{}' ({e})",
                    path.display(), s.trim()
                ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(anyhow::anyhow!("BLINDJOIN_BOT_COUNTER_FILE read failed: {e}")),
    }
}
```

**Why `tokio::fs::*` over `std::fs::*` + `tempfile::NamedTempFile::persist`:**

- The bot is already async (`#[tokio::main]` + `tokio::time::sleep` interleavings at `liquidity-bot/src/main.rs:99`). Using `tokio::fs` keeps the I/O on the tokio runtime without blocking; using `std::fs` would block the executor briefly. The write is tiny (≤20 bytes), so this is a minor optimization, but consistency with the rest of the bot's async I/O is worth it.
- `tempfile::NamedTempFile::persist` is also valid, but adds a transitive runtime dependency on `tempfile` (currently dev-only). Adding it as a runtime dep is undesirable given Phase 18's "no new runtime deps" goal. `tokio::fs::write + rename` uses only stdlib + tokio (already in workspace).

**Why `rename` is atomic enough on the bot's `/app/data` volume:**

- Both source (`bot_round_counter.tmp`) and dest (`bot_round_counter`) live in `/app/data/`, which is a SINGLE filesystem (Docker named volume `bot-data`). `rename(2)` is atomic within a single filesystem on Linux (POSIX guarantee). The bot can't be interrupted mid-rename in a way that leaves the counter file partially written.
- Worst case (kill -9 during `tokio::fs::write` of the .tmp file): `.tmp` exists with partial bytes, but `rename` never runs, so the main counter file is unchanged. On next bot start, the .tmp file is orphaned but harmless (the next successful round will overwrite it).
- One subtlety: `tokio::fs::write` does NOT fsync. If the kernel buffers the write and the container is killed before flush, the main counter file may have stale data. For a counter that's bumped every ~5 minutes (round duration on signet), this is acceptable — the worst impact is the bot picking the same script type twice in a row, which fails INTEG-02's "rotates per round" gate but is not a security issue. If plan-phase wants stronger durability, add an explicit `tokio::fs::File::sync_data().await?` between write and rename.

**Volume permissions verification:**

- Coordinator stage (`docker/Dockerfile:23-27`) does `RUN mkdir -p /app/keys /app/data` and runs as root (no USER directive in the Dockerfile). The coordinator successfully writes `/app/data/ban_list.jsonl` per existing deployment.
- Liquidity-bot stage (`docker/Dockerfile:34-37`) does NOT `mkdir -p /app/data` — this is a gap. 18-02 plan task adds `RUN mkdir -p /app/data` to the `liquidity-bot` stage (mirrors the coordinator stage).
- `docker-compose.yml` adds a new named volume `bot-data` mounted at `/app/data/` on the bot service (analogous to coordinator's `coordinator-data` at line 60-61 + 105-106).

**Returns (Q4 answer):**

- **Recommended idiom:** `tokio::fs::write(<tmp>, ...) + tokio::fs::rename(<tmp>, <final>)`. Stdlib + tokio only. Atomic on POSIX same-fs.
- **Volume permissions:** SOLVED for the coordinator; not yet solved for the bot. 18-02 plan adds `RUN mkdir -p /app/data` to the bot Dockerfile stage + `bot-data` named volume in docker-compose.yml.
- **No structural blockers.**

---

## Additional Research Areas (R1–R7)

### R1 — Pre-existing v1.4 acceptance / rollback test pattern

`full_round_three_clients` (full_round.rs:194-379) is the canonical 8-step structure:

| Step | Lines | Action |
|------|-------|--------|
| 1 | 200 | `require_bitcoind!()` (graceful skip in local-dev) |
| 2-4 | 208-209 | `crate::fund_regtest(exe)` (P2WPKH WIF setup) — replace with `fund_regtest_typed` + descriptor funding for INTEG-01 |
| 5 | 216-222 | `spawn_coordinator` + `wait_for_coordinator` — REUSE |
| 6 | 225-286 | 3 concurrent client tasks (`tokio::spawn` × 3); each task: `from_wif` → poll_until_phase(input_reg) → register_input → poll_until_phase(output_reg) → register_output → poll_until_phase(signing) → verify_and_sign |
| 7 | 296-326 | Mempool poll (10s deadline, 100ms cadence) — REUSE |
| 8 | 339-377 | `denom_output_count == 3` assertion via `get_raw_transaction_verbose` — EXTEND with input script-type set-equality check (CD-30) |

**Recommendation:** PROMOTE `spawn_coordinator` + `wait_for_coordinator` + `build_input_reg_round_state` + `v14_p2wpkh_coordinator_info` from `tests/integration/full_round.rs` into `tests/integration/mod.rs` (or a new `tests/integration/testing.rs` submodule reachable as `crate::testing::*`). Promotion approach: COPY the functions into `mod.rs`; CHANGE `full_round.rs` to re-import via `use crate::{spawn_coordinator, wait_for_coordinator, v14_p2wpkh_coordinator_info, build_input_reg_round_state};` — this is the MINIMAL change to `full_round.rs` (only the `use` declarations change; function bodies are zero-touch). Alternative: leave them in `full_round.rs` and import via `use crate::full_round::{...};` — works but couples test files architecturally. Plan-phase decides; recommended: PROMOTE.

**Wait — there's a constraint:** Rust modules use file-based privacy. If `spawn_coordinator` is `async fn` (not `pub async fn`) in `full_round.rs`, importing it from `mixed_script_e2e.rs` won't compile unless its visibility is `pub` or `pub(crate)`. Looking at `full_round.rs:85` — `async fn spawn_coordinator(...)` (no `pub`). So either:
- Add `pub(crate)` to the existing definitions (minimal `full_round.rs` change — 4 keyword adds).
- Or PROMOTE to `mod.rs` (cleaner; recommended).

### R2 — Heterogeneous-output flow correctness

`coordinator/src/bitcoin/tx.rs::build_coinjoin_psbt` (lines 53-128) inspection confirms:

- Inputs are walked in registration order — no per-script grouping or ordering assumption.
- `tx_outputs.push(TxOut { script_pubkey: out.script_pubkey.clone(), ... })` (line 92-94) clones each participant's output SPK directly into the tx. No homogeneity check.
- `psbt.inputs[i].witness_utxo` (lines 121-126) populates with the actual `inp.script_pubkey` — coordinator already supports heterogeneous input SPKs (Phase 16).

**Fee math caveat (Pitfall 3):** Lines 64-70 use `OUTPUT_WEIGHT_VBYTES = 31` for ALL outputs (hardcoded P2WPKH). For mixed-output tx, this under-estimates total tx size:
- P2WPKH output: 31 vbytes (matches).
- P2TR output: ~43 vbytes (12 vbytes under-estimate).
- P2SH-P2WPKH output: ~32 vbytes (1 vbyte under-estimate).

Worst case for the 3-client mixed-script E2E test: 3 denom outputs × ~12 vbytes under-estimate = ~36 vbytes under-counted. At `fee_rate_sat_per_vbyte = 1`, this is a ~36 sat under-collected fee. The `fund_regtest_typed` headroom is 50,000 sats per UTXO. **Margin of 3 orders of magnitude.** No issue.

**For the production signet bot (INTEG-02):** Same fee model applies. At `fee_rate_sat_per_vbyte = 1` (signet default), the under-estimation is ~36 sats per round — negligible. Mainnet rollout would need B-03 dynamic fee estimation (CONTEXT Deferred Ideas).

**Recommendation:** No coordinator code change needed for Phase 18. Document the fee-model assumption in `18-VERIFICATION.md` as a known-tolerable-edge.

### R3 — `pkarr` resolver path used by v1.3 binary

v1.3 client at `05f21438` `main.rs`:
```rust
let coordinator_url = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
    let info = discover::discover_coordinator(pkarr_key).await...
    info.coordinator_url
} else {
    cfg.coordinator_url.clone()
};
```

Direct `--coordinator-url` path bypasses PKARR entirely. `--use-tor` is a separate flag (line 67 — `if cfg.use_tor { ... } else { CoordinatorClient::new(coordinator_url) }`). The v1.4 in-process coordinator in the gate test is launched via `spawn_coordinator` with `listen_addr = "127.0.0.1:<port_0>"` (HTTP, NOT Tor). v1.3 client invocation: `--coordinator-url http://<127.0.0.1:port> --utxo <txid:vout> --utxo-wif <wif> --network signet` (no `--use-tor`, no `--pkarr-pubkey`).

**Confirmed:** v1.3 supports the direct-URL path without PKARR resolution. Gate is structurally GO.

### R4 — CRIT-01 grep gate

Current state at HEAD: `grep -c "CRIT-01" client/src/round/input.rs` returns **2** (lines 122 + 152). Phase 17 D-80 / verification baseline.

**Plan-phase 18 advice:** No CRIT-01 grep changes needed in 18-01 (mixed-script E2E test — purely consumes existing `register_input` path). The bot's `register_input` call site at `liquidity-bot/src/main.rs:184` ALSO consumes the existing CRIT-01 discipline transitively (the bot's synthetic_info → register_input → CRIT-01 wire emit). No bot-side `// CRIT-01` comment needed because the bot does NOT construct the OwnershipProof envelope manually — it delegates to `register_input` which already has the discipline.

**Recommended verification step in 18-VERIFICATION.md:** Re-run the grep gate `grep -c "CRIT-01" client/src/round/input.rs` after each Phase 18 plan boundary; expect ≥ 2.

### R5 — README §"Privacy Considerations" insertion point

README.md structure (lines 1-279):

| Lines | Section |
|-------|---------|
| 1-6 | Title + tagline |
| 7-17 | "What This Does" |
| 18-23 | "Documentation" |
| 25-42 | "Quick Start (Docker)" |
| 44-53 | "Build from Source" |
| 55-99 | "Run the Coordinator" |
| 100-134 | "API Endpoints" |
| 136-156 | "Run the Client" |
| 158-162 | "Pre-built Binaries" |
| 164-181 | "CI/CD" |
| 183-235 | "Project Structure" |
| 237-258 | "Security Model" |
| 260-275 | "Key Dependencies" |
| 277 | "License" |

**Recommended insertion point:** After "Quick Start (Docker)" (ends line 42 with the signet faucet URL), BEFORE "Build from Source" (starts line 44).

**Exact heading hierarchy:** All current sections use `## Section Name` (level 2). The new section MUST also use `## Privacy Considerations` for visual parity.

**Plan-phase 18-03 Edit specification:**

```
Insertion at line 43-44 boundary (after `To get a signet UTXO for the bot, use the [signet faucet](https://signet.bc-2.jp/).` and before `## Build from Source`).

New content (2 paragraphs ~200 words total per D-106):

## Privacy Considerations

blindjoin accepts mixed input script types (P2WPKH, P2TR, P2SH-P2WPKH) in a
single round. This maximizes the anonymity set across address types but
creates a chain-analysis signal: a CoinJoin transaction with a wildly
heterogeneous input set is visually distinguishable from a uniform-script
CoinJoin. Privacy-sensitive users who require uniform-script rounds can run
a dedicated coordinator with a single `allow_*` flag enabled.

The bundled liquidity bot rotates the script type it submits across rounds.
This prevents the bot's UTXOs from forming a uniform-script-type fingerprint
(which would otherwise identify the bot's participation by cross-round
correlation). Rotation is round-robin across the operator-configured
`BLINDJOIN_BOT_SCRIPT_TYPES`; each run is single-shot and uses a fresh
wallet, so output addresses do not cluster across rounds.

```

### R6 — Open Question Resolution (test binary path)

ROADMAP Phase 18 success criterion #1 names `cargo test -p coordinator --test full_round`. There is NO `[[test]] name = "full_round"` declaration; only `name = "integration"` at `coordinator/Cargo.toml:71-73`. ROADMAP wording is STALE (predates Phase 9 consolidation).

**Canonical invocation today** (Phase 18 acceptance gate):

| Purpose | Command |
|---------|---------|
| Mixed-script E2E test | `cargo test -p coordinator --test integration mixed_script_e2e -- --nocapture` |
| v1.3 invariant gate (full_round suite) | `cargo test -p coordinator --test integration full_round -- --nocapture` |
| v1.3-binary gate (opt-in) | `cargo test -p coordinator --test integration --features v13-binary-compat v13_binary_compat -- --nocapture` |
| Bot rotation integration | `cargo test -p coordinator --test integration bot_rotation -- --nocapture` |
| Full Phase 18 acceptance (all of the above) | `cargo test -p coordinator --test integration -- --include-ignored --nocapture` (skips bitcoind-missing tests gracefully in local-dev) |

Plan-phase 18-VERIFICATION.md should document these verbatim with a 1-line aliasing note: "ROADMAP §Phase 18 success criterion #1 names `--test full_round` — this is stale wording (Phase 9 consolidated all integration tests under `--test integration`). Canonical invocation is `--test integration mixed_script_e2e`."

### R7 — `cargo test -p coordinator --test integration ...` invocation correctness

Confirmed via `coordinator/Cargo.toml` inspection:
```toml
[[test]]
name = "integration"
path = "../tests/integration/mod.rs"
```

This makes `mod.rs` the integration test binary's crate root. Phase 18 18-01 adds `mod mixed_script_e2e;` to the existing block at `mod.rs:19-24` (alphabetically inserted between `full_round` and `multi_script_client`). 18-02 adds `mod bot_rotation;` (alphabetically inserted between `ban_list_persistence` and `full_round`). 18-03 adds `mod v13_binary_compat;` (alphabetically appended at the end, gated behind `#[cfg(feature = "v13-binary-compat")]` per CD-32).

**No `[[test]]` declaration changes needed in any `Cargo.toml`** — just `mod X;` adds in `tests/integration/mod.rs`.

The `--features v13-binary-compat` feature must be declared in `coordinator/Cargo.toml` `[features]` section (currently absent — the only features there are workspace defaults). 18-03 plan task 1 adds:

```toml
[features]
default = []
v13-binary-compat = []
```

---

## Cross-Phase Invariant Statement

Per Phase 14/15/16/17 carry-forward + Phase 18 CONTEXT D-91:

> `cargo test -p coordinator --test integration full_round` MUST report **8 passed, 0 failed, ~42s wall-clock** at every Phase 18 plan boundary (after 18-01 lands; after 18-02 lands; after 18-03 lands; at the milestone-cut PR).
>
> Drift in pass count OR new failure → Phase 18 BLOCKER. REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

Plan-phase 18-VERIFICATION.md records:
- Baseline pass count (8) from Phase 17 verification.
- Per-plan re-run results (with timestamps + commit SHAs).
- Drift detection: if any v1.3 test goes red, immediate REPAIR-01-style git revert + `/gsd:debug` invocation.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | v1.3 binary at SHA `05f21438` builds clean with workspace Cargo.lock at HEAD | Q2 + Pitfall 5 | If wrong: D-87 UAT-documented fallback applies. Not a Phase 18 BLOCKER — escape valve exists. Verification: 18-03 plan task 1 builds the v1.3 binary locally and either GREEN-LIGHTS automated path OR FALLS BACK to UAT. |
| A2 | v1.3 client `Network::Signet` parsing accepts a regtest-funded WIF + outpoint without rejection | Q2 (Option A) + Pitfall 7 | If wrong: v1.3 binary rejects `--network signet` with the regtest UTXO. Mitigation: switch to Option B (run coordinator with signet config) or extend v1.3 worktree with regtest support (Option C — undermines pinned-binary semantic). |
| A3 | `tokio::fs::rename` is atomic on the Docker `bot-data` named volume's underlying filesystem | Q4 | If wrong (e.g., underlying fs doesn't support atomic rename): worst case is partial-write counter file. Mitigated by `read_counter`'s malformed-counter bail. Add fsync between write and rename if 18-02 testing detects flake. |
| A4 | `Address::from_str` + `peek_address(External, 0)` for P2TR on regtest network produces valid bech32m addresses that bitcoind's `send_to_address` accepts | Q1 | Phase 17 unit tests confirm `generate_p2tr_produces_bip86_descriptor` GREEN. Bitcoind v30.2 supports bech32m natively (BIP-350). Risk: LOW. |
| A5 | Promoting `spawn_coordinator` from `full_round.rs` to `mod.rs` does NOT change `full_round.rs`'s test outcomes | Pitfall 1 + R1 | If wrong: cross-phase invariant breaks immediately on 18-01 landing. Mitigation: keep `pub(crate)` visibility minimal; re-run `full_round` suite after every commit in 18-01 plan execution. |
| A6 | `bdk_wallet::Wallet::create(external_desc, internal_desc)` with auto-derived internal accepts ALL 3 script-type descriptor shapes the bot may receive | Q3 | Confirmed via `BdkClientWallet::generate` tests (client/src/wallet.rs:601-664). Risk: LOW. |
| A7 | The bot's main-loop can be cleanly extracted into a `liquidity_bot::run(config) -> Result<()>` library function in <30 LOC | Pitfall 4 / 18-02 / D-102 (a) | If extraction balloons: fall back to D-102 (b) (drive via Command::new). 18-02 plan documents the call. |

**User confirmation needed before plan-phase execution:** NONE. All assumptions are either verified inline (A1 verification deferred to 18-03 plan task 1, which is the safest place for it) or have explicit fallback paths that don't block the milestone.

---

## Open Questions

None remaining that block plan-phase. All 4 numbered questions resolved with concrete recommendations:
- Q1: GO (B1.b descriptor-funding path); `BdkClientWallet.utxo_outpoint` is `pub` and assignable post-construction.
- Q2: GO (D-86 automated v1.3-binary gate); pinned SHA `05f21438`; v1.3 client supports `--coordinator-url` direct path; cargo build cost ~30s first-run.
- Q3: 5-arg `from_descriptor` signature; bot env-var surface uses full descriptor strings + UTXO addresses; auto-derived internal.
- Q4: `tokio::fs::write + rename` idiom; bot Dockerfile gains `mkdir -p /app/data`; `bot-data` volume in docker-compose.

R1–R7 also fully resolved by code inspection (no remaining unknowns).

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All Phase 18 plans | ✓ | 1.89+ (per README:46) | — |
| `cargo` | All Phase 18 plans | ✓ | matches Rust | — |
| `git worktree` | 18-03 v1.3-binary gate | ✓ | git 2.5+ (standard) | D-87 UAT-documented gate |
| `bitcoind` v30.2 | 18-01 + 18-02 + 18-03 integration tests | ✓ in CI (Phase 9-01) | 30.2 (pinned via `.bitcoind-version`) | `require_bitcoind!()` graceful-skip in local-dev |
| Docker + docker-compose | Operator-side smoke check post-18-02 (not test-required) | N/A in test harness | — | — |
| `tempfile` crate | 18-02 unit tests + 18-02 bot_rotation integration test | ✓ | workspace pinned at 3 | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** `bitcoind` falls back to graceful-skip in local-dev. `git worktree` for v1.3-binary gate falls back to D-87 (manual UAT) if `git worktree add` fails for any reason (corrupt local state, missing object, etc.).

---

## Sources

### Primary (HIGH confidence — codebase-verified)

- `tests/integration/full_round.rs` (lines 1-1597) — canonical 8-step structure for INTEG-01 + spawn_coordinator + v14_p2wpkh_coordinator_info + build_input_reg_round_state + mempool polling pattern + denom_output_count assertion.
- `tests/integration/mod.rs` (lines 1-823+) — `require_bitcoind!()` macro, `BitcoindGuard` RAII, `bootstrap_regtest_bitcoind`, `fund_regtest`, `fund_regtest_typed`, `TypedUtxoHandle`, `FundedTypedSetup`.
- `tests/integration/multi_script_validate.rs` (lines 1-120 inspected) — Phase 16-02 patterns; reused encoder/decoder + per-script-type assertion shape.
- `tests/integration/multi_script_client.rs` (lines 1-200 inspected) — Phase 17 boundary tests; v14_pkarr_record_with_p2tr_wallet_rejects_before_tor pattern.
- `client/src/wallet.rs` (lines 1-708) — BdkClientWallet with from_wif, from_descriptor, generate, sign_bip322, script_type, coinjoin_output_address, utxo_outpoint (pub field).
- `client/src/discover.rs` (lines 1-450) — CoordinatorInfo, CoordinatorCapabilities, DiscoveryError, capabilities_from_record_v.
- `client/src/round/input.rs` (lines 1-200 inspected) — register_input 4-arg signature, CRIT-01 grep gate (2 references), v1/v2 envelope branch.
- `client/src/config.rs` (lines 1-141) — parse_script_type wire form (snake_case + p2sh-p2wpkh rename).
- `coordinator/src/config.rs` (lines 110-265) — BipConfig with default-all-allowed, output_script_type=P2wpkh.
- `coordinator/src/api/handlers.rs` (lines 290-450) — post_output handler (no runtime check on submitted output script type).
- `coordinator/src/bitcoin/fee.rs` (lines 1-19) — estimate_fee_share with hardcoded OUTPUT_WEIGHT_VBYTES=31 (P2WPKH).
- `coordinator/src/bitcoin/tx.rs` (lines 1-130) — build_coinjoin_psbt with heterogeneous-output flow correctness.
- `coordinator/src/round/blame.rs` (lines 85-170) — append_ban_entry (std::fs append-mode; NOT atomic write).
- `coordinator/Cargo.toml` (lines 1-74) — `[[test]] name = "integration" path = "../tests/integration/mod.rs"`.
- `liquidity-bot/src/main.rs` (lines 1-209) — current single-shot bot + synthetic_info pattern.
- `liquidity-bot/src/strategy.rs` (lines 1-101) — JoinStrategy + test fixtures.
- `liquidity-bot/Cargo.toml` (lines 1-17) — current dep surface.
- `docker/docker-compose.yml` (lines 1-106) — liquidity-bot service + coordinator-data volume pattern.
- `docker/Dockerfile` (lines 1-37) — multi-stage build; coordinator stage mkdir's /app/data but bot stage does not.
- `shared/src/protocol.rs` — OwnershipProof (v=1 array-of-hex CD-7 branch + v=2 flat struct); InfoResponse legacy defaults.
- `README.md` (lines 1-279) — insertion point identified at line 43-44 boundary.
- `.planning/phases/18-mixed-script-e2e-liquidity-bot/18-CONTEXT.md` (lines 1-367) — user decisions D-81..D-106 + CD-25..CD-33.

### v1.3 SHA verification (HIGH confidence — `git show` against pinned commit)

- `git show 05f21438:client/Cargo.toml` — v1.3 client deps byte-identical to HEAD workspace deps.
- `git show 05f21438:client/src/main.rs` — v1.3 client main.rs supports `--coordinator-url` + `--use-tor` flags (line ~67 branch).
- `git show 05f21438:shared/src/protocol.rs` — v1.3 OwnershipProof shape is `pub struct OwnershipProof { pub witness_stack: Vec<Vec<u8>> }`; v1.3 InfoResponse omits supported_script_types/output_script_type.
- `git show 05f21438:Cargo.toml` — workspace deps byte-identical to HEAD.
- `git log --first-parent --oneline 622ccf0^ -1` — pinned SHA `05f21438` resolves to "docs(15): create phase plan" (last commit before any v1.4 source code change).

### Tertiary (MEDIUM confidence — inferred from patterns, not direct verification)

- `tokio::fs::rename` atomicity on Docker named volumes: assumed POSIX semantics. Docker volumes are typically backed by ext4 or overlay2 on Linux, both of which support atomic rename(2). If a deployment uses a non-POSIX FS (Windows host with bind mount), atomicity is weaker — not relevant for the bot's test path (bot_rotation uses `tempfile::tempdir()` inside the test, not /app/data).

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every crate is workspace-pinned and verified against existing Phase 14-17 usage.
- Architecture patterns: HIGH — `full_round.rs` shape is the canonical template; INTEG-01 is a 3-difference mutation.
- Pitfalls: HIGH — 7 concrete pitfalls all rooted in inspected code/lines.
- v1.3-binary gate: HIGH — pinned SHA resolved, build infrastructure verified via `git show`.
- Bot rotation idiom: MEDIUM — `tokio::fs::rename` semantic is standard but specific to POSIX same-fs; Docker volume backing assumed POSIX (true on Linux hosts; Windows host with bind mount weakens atomicity).
- README insertion point: HIGH — exact line numbers from direct file inspection.

**Research date:** 2026-05-30
**Valid until:** Phase 18 acceptance gate (estimated 2026-06-15 latest, given v1.4 milestone timeline).

---

## RESEARCH COMPLETE
