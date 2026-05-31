# Phase 18: Mixed-Script E2E + Liquidity Bot - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `18-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-05-30
**Phase:** 18-Mixed-Script E2E + Liquidity Bot
**Mode:** `--auto` (autonomous; recommended defaults selected for every area without AskUserQuestion)
**Areas discussed:** Mixed-script E2E test file location; Funding strategy for mixed-script UTXOs; v1.3-client binary compatibility gate; Coordinator config for the mixed-script test; Cross-phase invariant verification; Liquidity bot config field for enabled script types; Bot per-round type rotation strategy; Bot UTXO sourcing across multiple script types; Per-round-index derivation interpretation; Bot test strategy; Acceptance-gate broadcast verification; Plan ordering; README §"Privacy Considerations" prose

---

## A. Mixed-script E2E test file location

| Option | Description | Selected |
|--------|-------------|----------|
| A1 | NEW file `tests/integration/mixed_script_e2e.rs` (sibling to `full_round.rs` + `multi_script_validate.rs`); keeps `full_round.rs` zero-touch (cross-phase invariant). | ✓ (recommended default) |
| A2 | Add new test fns into `tests/integration/full_round.rs`. | |
| A3 | Add tests into `tests/integration/multi_script_client.rs` (Phase 17 owns this file). | |

**Auto-selected:** A1.
**Notes:** Preserves the cross-phase invariant ("never touch full_round.rs"). Mirrors Phase 16 `multi_script_validate.rs` + Phase 17 `multi_script_client.rs` per-domain-isolation pattern. Recorded as D-81 in CONTEXT.md.

---

## B. Funding strategy for mixed-script UTXOs

| Option | Description | Selected |
|--------|-------------|----------|
| B1.a | Reuse `fund_regtest_typed(P2WPKH+P2TR+P2SH-P2WPKH)`; build descriptor wallets that match the seeded raw-key UTXOs. Structurally blocked — xprv-derivation is one-way; raw SecretKey cannot become a descriptor wallet's keychain root. | |
| B1.b | Generate fresh descriptor wallets via `BdkClientWallet::generate(...)`; fund `wallet.coinjoin_output_address()` via regtest `send_to_address`; locate vout; override `wallet.utxo_outpoint`. | ✓ (recommended default) |
| B2 | Extend `fund_regtest_typed` to take descriptor-derived addresses. Heavy refactor of a load-bearing v1.3 fixture. | |
| B3 | Mixed: P2WPKH via WIF (`from_wif` + `fund_regtest_typed`) + P2TR/P2SH-P2WPKH via B1.b descriptor-wallet-driven funding. | (composite — adopted) |

**Auto-selected:** B1.b for P2TR + P2SH-P2WPKH; WIF path (existing `from_wif`) for P2WPKH. Recorded as D-83 + D-84 in CONTEXT.md.
**Notes:** Covers BOTH client wallet code paths (legacy WIF + modern descriptor) in one acceptance test. Plan-phase may consolidate via a `fund_descriptor_wallet` helper if the inline body grows large (Deferred Ideas).

---

## C. v1.3-client binary compatibility gate (success criterion #5)

| Option | Description | Selected |
|--------|-------------|----------|
| C1 | Automated test: `git worktree add` at pinned commit SHA + `cargo build --release` + drive via `tokio::process::Command::new`; opt-in feature flag for default-test-suite speed. | ✓ (recommended default) |
| C2 | Manual UAT documented in `18-VERIFICATION.md`: operator runs v1.3 binary against v1.4 coordinator before milestone cut. Escape valve if C1 plan-phase research exposes build-infra cost. | (fallback) |
| C3 | Defer to v1.5 TEST-EXT-03 backwards-compat matrix; reword Phase 18 ROADMAP success criterion #5. | (rejected — violates ROADMAP wording) |

**Auto-selected:** C1 with C2 as escape valve. Recorded as D-86, D-87, D-88, CD-25, CD-32 in CONTEXT.md.
**Notes:** Phase 17 D-79 explicitly handed this off to Phase 18 as the binary acceptance gate. Plan-phase decides the exact mechanism in 18-03-PLAN.md based on v1.3 build-time/disk budget.

---

## D. Coordinator config for the mixed-script test

| Option | Description | Selected |
|--------|-------------|----------|
| D1 | `BipConfig::default()` (all-allowed, `output_script_type = P2wpkh`); already validated by Phase 16 + Phase 17 `full_round.rs` carry-forward. | ✓ (recommended default) |
| D2 | Custom BipConfig per test (e.g., `output_script_type = P2tr` with all-allowed inputs); test-specific override. | |
| D3 | Add a runtime gate on submitted output addresses matching `ost`; Phase 18 NEW coordinator enforcement. | (rejected — would break the heterogeneous-output property + duplicate Phase 17 D-76 client-side check) |

**Auto-selected:** D1 + D2.B (heterogeneous outputs flow through coordinator unchanged). Recorded as D-89, D-90 in CONTEXT.md.
**Notes:** The coordinator does not currently runtime-check output script type against advertised `ost`. The output-type advertisement is a client-side fail-fast (Phase 17 D-76). Runtime enforcement is Deferred to v1.5.

---

## E. Cross-phase invariant verification

| Option | Description | Selected |
|--------|-------------|----------|
| E1 | Document the run-twice convention (`cargo test --test integration full_round` after each plan; expect 8/8 green, ~42s) in `18-VERIFICATION.md`. Carries from Phase 14/15/16/17. | ✓ (recommended default) |
| E2 | Add a `[[test]]` invariant-job in CI gating each merge. | (Deferred — out of Phase 18 scope; v1.5 CI hardening candidate) |

**Auto-selected:** E1. Recorded as D-91 in CONTEXT.md.

---

## F. Liquidity bot config field for enabled script types

| Option | Description | Selected |
|--------|-------------|----------|
| F1 | CSV env var `BLINDJOIN_BOT_SCRIPT_TYPES` (default `"p2wpkh"`); parses via `client::config::parse_script_type`. Single-underscore convention (Phase 17 CD-22 + Phase 8 bot pattern). | ✓ (recommended default) |
| F2 | Multi-value envs: separate `BLINDJOIN_BOT_ALLOW_P2WPKH/P2TR/P2SH_P2WPKH=true` flags. | |
| F3 | TOML config file added to the bot (currently env-var-only). | (rejected — bot stays env-var-only per Phase 4 baseline) |

**Auto-selected:** F1. Recorded as D-92, D-93 in CONTEXT.md.

---

## G. Bot per-round type rotation strategy

| Option | Description | Selected |
|--------|-------------|----------|
| G1 | Persistent counter file at `/app/data/bot_round_counter`; atomic tempfile-then-rename writes; bump on round success only; rotation = `enabled[counter % len]`. | ✓ (recommended default) |
| G2 | Randomized rotation (no persistence). Fails "rotates per round" guarantee — could pick the same type 3 runs in a row. | |
| G3 | Operator-supplied rotation table via env var. Heavier; G1 covers via env-var update on each restart. | |

**Auto-selected:** G1. Recorded as D-94, D-95, D-96, CD-28 in CONTEXT.md.

---

## H. Bot UTXO sourcing across multiple script types

| Option | Description | Selected |
|--------|-------------|----------|
| H1 | Per-type env-var tuples: `BLINDJOIN_BOT_{P2WPKH,P2TR,P2SH_P2WPKH}_{DESC,UTXO,WIF}`. Bot loads matching tuple per rotated-to type. | ✓ (recommended default) |
| H2 | HD wallet (seed-driven, auto-scan bitcoind for spendable UTXOs at derived addresses). | (Deferred to v1.5) |
| H3 | Single-WIF only; bot stays P2WPKH. Fails INTEG-02 wording. | |

**Auto-selected:** H1 with H3 BACKWARDS-COMPAT preserved (default `BLINDJOIN_BOT_SCRIPT_TYPES = "p2wpkh"` + legacy `BLINDJOIN_UTXO` + `BLINDJOIN_UTXO_WIF` continue to drive a single-WIF P2WPKH bot for v1.3 deployments that never set the new envvar). Recorded as D-97, D-98, D-99 in CONTEXT.md.

---

## I. Per-round-index derivation interpretation

| Option | Description | Selected |
|--------|-------------|----------|
| I1 | Property assertion: single-shot pattern + fresh wallet per run → fresh index-0 output address per run → no clustering. Already structurally satisfied. | ✓ (recommended default) |
| I2 | Requirement to advance the address index within a single bot process lifetime. Heavier; redundant given single-shot model. | |

**Auto-selected:** I1. Recorded as D-100 in CONTEXT.md.

---

## J. Bot test strategy

| Option | Description | Selected |
|--------|-------------|----------|
| J1 | Unit tests for rotation logic in `liquidity-bot/src/strategy.rs` + integration test for 3-run bot lifetime. | ✓ (recommended default) |
| J2 | Unit-only (no integration test); rely on Phase 18 INTEG-01 mixed-script E2E to indirectly cover. | |
| J3 | Integration-only; skip unit tests. | (rejected — unit-level rotation correctness is independent of bitcoind availability) |

**Auto-selected:** J1. Recorded as D-101, D-102, D-103, CD-26 in CONTEXT.md.

---

## K. Acceptance-gate broadcast verification

| Option | Description | Selected |
|--------|-------------|----------|
| K1 | Same `get_raw_mempool` polling pattern as `full_round.rs:296-326` (10s deadline, 100ms cadence). Asserts denom-output-count = 3 + input-script-types set equality. | ✓ (recommended default) |
| K2 | Coordinator broadcast callback / signal (would require coordinator API surface change). | (rejected — out of scope) |

**Auto-selected:** K1. Recorded as D-104, CD-30 in CONTEXT.md.

---

## L. Plan ordering

| Option | Description | Selected |
|--------|-------------|----------|
| M1 | 18-01 = INTEG-02 (bot rotation) → 18-02 = INTEG-01 (E2E test, depends on bot's descriptor-mode for funding) → 18-03 = v1.3 binary gate + closeout. | |
| M2 | 18-01 = INTEG-01 (E2E test, independent) → 18-02 = INTEG-02 (bot rotation) → 18-03 = v1.3 binary gate + README prose + closeout. | ✓ (recommended default) |
| M3 | 18-01 = v1.3 binary gate → 18-02 = INTEG-01 → 18-03 = INTEG-02. | (rejected — acceptance gate should land before the auxiliary gates) |

**Auto-selected:** M2. Recorded as D-105 in CONTEXT.md.

---

## M. README §"Privacy Considerations" prose

| Option | Description | Selected |
|--------|-------------|----------|
| N1 | NEW section in `README.md` (~2 paragraphs); plain language; documents V1.4-MOD-06 fingerprint + V1.4-MIN-02 rotation mitigation. | ✓ (recommended default) |
| N2 | Inline disclaimer within an existing section (e.g., "Quick Start"); less prominent. | |
| N3 | Skip; rely on PITFALLS.md + ADR. ROADMAP success criteria do not require a README addition. | (rejected — Phase 14 CD-3 explicitly deferred prose to Phase 18) |

**Auto-selected:** N1. Recorded as D-106, CD-33 in CONTEXT.md.

---

## Claude's Discretion

Areas the user did NOT explicitly select (auto-mode picked the recommended option); flagged for review:

- **CD-25:** Automated v1.3-binary gate (D-86) vs documented UAT (D-87). Default: automated.
- **CD-26:** `tests/integration/bot_rotation.rs` vs `liquidity-bot/tests/`. Default: shared integration suite.
- **CD-27:** `pick_script_type` on `JoinStrategy` vs new `RotationState` type. Default: new `RotationState` type.
- **CD-28:** Hardcoded rotation-counter path vs env-var-configurable. Default: env-var configurable.
- **CD-29:** Bot accepts raw `xprv` vs full BIP-380 descriptor string. Default: full descriptor.
- **CD-30:** Broadcast-tx input-type assertion via witness inspection vs re-query bitcoind. Default: re-query bitcoind.
- **CD-31:** `tempfile` dev-dependency for counter-file unit tests. Default: yes.
- **CD-32:** v1.3-binary gate runs by default vs opt-in feature flag. Default: opt-in (`--features v13-binary-compat`).
- **CD-33:** README §"Privacy Considerations" mentions WabiSabi absence. Default: no.

---

## Deferred Ideas

See CONTEXT.md `<deferred>` section for the full list. Highlights:

- Coordinator runtime check on submitted output script types matching advertised `ost` (v1.5+).
- HD wallet bot model with `scantxoutset` auto-discovery (v1.5+).
- TEST-EXT-01/02/03 cross-implementation + on-chain anchor + automated matrix (v1.5+).
- CARRY-TOR-UAT + CARRY-REPAIR-01-PR (v1.5+ / v1.4 cut PR respectively — Phase 18 does not touch).
- `DECISIONS-INDEX.md` rolling summary (v1.5+; D-* count now 100+).
- `bdk_wallet = "=2.3.x"` exact-pin tightening (v1.5+).
