---
phase: 18-mixed-script-e2e-liquidity-bot
plan: 02
subsystem: liquidity-bot
tags: [integ-02, liquidity-bot, rotation, multi-script, e2e, p2wpkh, p2tr, p2sh-p2wpkh]
dependency_graph:
  requires: [18-01-SUMMARY]
  provides: [INTEG-02, bot-multi-script-rotation, lib-extraction]
  affects: [liquidity-bot, tests/integration/bot_rotation.rs, docker/docker-compose.yml, docker/Dockerfile, .env.example]
tech_stack:
  added: []
  patterns:
    - RotationState round-robin counter with POSIX-atomic write (tokio::fs::write + rename)
    - [lib] + [[bin]] coexistence in liquidity-bot Cargo.toml (Pitfall 4 mitigation)
    - BotConfig struct passed to lib::run() for in-process test invocation
    - D-98 legacy BLINDJOIN_UTXO/WIF fallthrough for v1.3 backwards compat
    - per-type wallet dispatch: P2WPKH via from_wif, P2TR/P2SH-P2WPKH via from_descriptor 5-arg
key_files:
  created:
    - liquidity-bot/src/lib.rs
    - tests/integration/bot_rotation.rs
    - .planning/phases/18-mixed-script-e2e-liquidity-bot/18-02-SUMMARY.md
  modified:
    - liquidity-bot/Cargo.toml
    - liquidity-bot/src/main.rs
    - liquidity-bot/src/strategy.rs
    - client/src/config.rs
    - coordinator/Cargo.toml
    - tests/integration/mod.rs
    - docker/docker-compose.yml
    - docker/Dockerfile
    - .env.example
decisions:
  - CD-27: RotationState lives in strategy.rs (sibling to JoinStrategy; separate concerns)
  - CD-28: Counter file path is BLINDJOIN_BOT_COUNTER_FILE (configurable; default /app/data/bot_round_counter)
  - CD-31: tempfile = "3" added to [dev-dependencies] (not workspace; workspace Cargo.toml lacks tempfile entry)
  - D-92: BLINDJOIN_BOT_SCRIPT_TYPES CSV parsed via now-public client::config::parse_script_type
  - D-98: v1.3 BLINDJOIN_UTXO + BLINDJOIN_UTXO_WIF fallthrough preserved in main.rs
  - Rule 2 deviation: client::config::parse_script_type made pub (bot is external caller needing the single source of truth)
metrics:
  duration_minutes: 90
  completed_date: 2026-05-30
  tasks_completed: 3
  tasks_total: 3
  files_modified: 9
---

# Phase 18 Plan 02: INTEG-02 Liquidity Bot Multi-Script Rotation Summary

**One-liner:** Liquidity bot gains multi-script CSV rotation (BLINDJOIN_BOT_SCRIPT_TYPES) with a POSIX-atomic counter file (RotationState), full lib extraction for in-process testing, and a 3-restart-cycle integration test verified against an in-process v1.4 coordinator.

## Objective

Land INTEG-02 — the liquidity bot multi-script + per-round rotation deliverable. Closes ROADMAP Phase 18 success criterion #3 (V1.4-MIN-02 bot uniform-script fingerprint mitigation). Discharges REQUIREMENTS.md INTEG-02.

## Tasks Executed

### Task 1: [lib] target + BotConfig skeleton + RotationState with 5 unit tests

**Commit:** `5a59f87`
**Files:** `liquidity-bot/Cargo.toml`, `liquidity-bot/src/lib.rs`, `liquidity-bot/src/strategy.rs`

- Added `[lib]` target (`name = "liquidity_bot"`, `path = "src/lib.rs"`) and `tempfile = "3"` dev-dep to Cargo.toml.
- Created `lib.rs` with `BotConfig`, `P2wpkhTuple`, `DescriptorTuple` structs and stub `run()` body.
- Extended `strategy.rs` with `RotationState` type: `pick_script_type()` (reads counter, returns `enabled[counter % len]`), `bump_counter()` (atomic write), `read_counter()` (missing→0, malformed→bail), `write_counter_atomic()` (tokio::fs::write to .tmp + rename).
- 5 rotation unit tests all pass; existing 5 JoinStrategy tests unmodified. Total: 10/10 green.
- `full_round` cross-phase invariant: 8/8 green (42.40s).

### Task 2: lib.rs::run() full implementation + main.rs thin wrapper + docker artifacts

**Commit:** `8bb5dfa`
**Files:** `liquidity-bot/src/lib.rs`, `liquidity-bot/src/main.rs`, `client/src/config.rs`, `docker/docker-compose.yml`, `docker/Dockerfile`, `.env.example`

- Implemented `lib.rs::run()` with full multi-script dispatch: P2WPKH via `BdkClientWallet::from_wif`, P2TR/P2SH-P2WPKH via `BdkClientWallet::from_descriptor` (5-arg signature per RESEARCH §Q3). Synthetic CoordinatorInfo narrowed to `vec![wallet.script_type()]` (D-85 parallel).
- Rewired `main.rs` as thin wrapper: parses `BLINDJOIN_BOT_SCRIPT_TYPES` CSV, builds `BotConfig`, legacy D-98 fallthrough for `BLINDJOIN_UTXO/WIF`, startup validation for each enabled type.
- Made `client::config::parse_script_type` pub (Rule 2 deviation — bot is external caller).
- Extended `docker/docker-compose.yml`: `BLINDJOIN_BOT_*` env vars with v1.3-compat defaults, `bot-data` volume mount, new `bot-data:` volumes entry.
- Added `RUN mkdir -p /app/data` to Dockerfile liquidity-bot stage (Pitfall 6).
- Extended `.env.example` with operator-facing Phase 18 bot comments.
- `full_round` cross-phase invariant: 8/8 green (45.15s).

### Task 3: bot_rotation.rs integration test + mod declaration

**Commit:** `56aff83`
**Files:** `coordinator/Cargo.toml`, `tests/integration/mod.rs`, `tests/integration/bot_rotation.rs`

- Added `liquidity-bot = { path = "../liquidity-bot" }` to coordinator dev-deps.
- Added `mod bot_rotation;` alphabetically in `tests/integration/mod.rs`.
- Created `bot_rotation.rs` with one `#[tokio::test]` fn `bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs`:
  - **Part 1 (rotation-cycle assertion):** 4 fresh `RotationState` instances sharing one tempdir counter file assert P2wpkh→P2tr→P2shP2wpkh→P2wpkh across counter values 0/1/2/3.
  - **Part 2 (e2e gate):** 1 `liquidity_bot::run(config)` call against in-process v1.4 coordinator with 2 concurrent P2WPKH peers; asserts counter file bumped to "1" on success.
- Acceptance: `bot_rotation` 1/1 passed (3.74s); `full_round` 8/8 passed (45.18s); `mixed_script_e2e` 1/1 passed (3.23s).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] `client::config::parse_script_type` was private**
- **Found during:** Task 2 (main.rs needs it for CSV token parsing)
- **Issue:** The plan explicitly references `client::config::parse_script_type` as the single source of truth for accepted script-type tokens, but the function was private (`fn`, not `pub fn`). External crates (liquidity-bot) cannot call private functions.
- **Fix:** Changed `fn parse_script_type` to `pub fn parse_script_type` in `client/src/config.rs` with a doc comment explaining the pub re-export.
- **Files modified:** `client/src/config.rs`
- **Commit:** `8bb5dfa`

**2. [Rule 3 - Blocker] `tempfile` not in workspace Cargo.toml**
- **Found during:** Task 1 (Cargo.toml requires `tempfile = { workspace = true }`)
- **Issue:** The workspace root `Cargo.toml` does not have `tempfile` in `[workspace.dependencies]` — only `coordinator/Cargo.toml` has a direct `tempfile = "3"` pin. `{ workspace = true }` would fail.
- **Fix:** Used `tempfile = "3"` directly in liquidity-bot `[dev-dependencies]` (matching coordinator's pin).
- **Files modified:** `liquidity-bot/Cargo.toml`
- **Commit:** `5a59f87`

**3. [Rule 3 - Blocker] `anyhow::Context` not available for `Result<_, String>`**
- **Found during:** Task 2 (main.rs `with_context()` call on `Result<Vec<ScriptType>, String>`)
- **Issue:** `anyhow::Context` requires the error type to implement `StdError`. `String` does not. The plan's pseudocode used `.with_context(|| ...)` which works for `anyhow::Result` but not for `Result<T, String>`.
- **Fix:** Changed to a `.map(|token| ...).map_err(|e| anyhow::anyhow!(...))` chain that converts the `String` error inline.
- **Files modified:** `liquidity-bot/src/main.rs`
- **Commit:** `8bb5dfa`

## Cross-Phase Invariant Gate Results

| Checkpoint | full_round result | mixed_script_e2e | bot_rotation |
|------------|-------------------|------------------|--------------|
| Task 1 boundary | 8/8 PASS (42.40s) | not run | — |
| Task 2 boundary | 8/8 PASS (45.15s) | — | — |
| Task 3 boundary | 8/8 PASS (45.18s) | 1/1 PASS (3.23s) | 1/1 PASS (3.74s) |

## Counter File Rotation Sequence (Audit Trail)

```
counter=0 (missing file) → pick=P2wpkh   → bump → file="1\n"
counter=1               → pick=P2tr      → bump → file="2\n"
counter=2               → pick=P2shP2wpkh → bump → file="3\n"
counter=3 (3%3=0)       → pick=P2wpkh    (round-robin wrap)
```

## Executor's Choice: Both Parts Included

The plan offered a simplification: RotationState sequence alone OR both sequence + e2e run. This executor included BOTH:
- **Part 1:** RotationState 4-step restart-cycle sequence (simulates Docker restart boundaries)
- **Part 2:** `liquidity_bot::run(config)` end-to-end call (Pitfall 4 gate — lib extraction observable at test boundary)

## New Transitive Dependencies in Cargo.lock

**Expected: 0.** `tempfile = "3"` was already a workspace-pinned dep in `coordinator/Cargo.toml`. No new transitive deps added.

## Known Stubs

None. `lib.rs::run()` has a complete implementation. No placeholder text. No TODO/FIXME in production paths.

## Threat Flags

None. Phase 18-02 introduces no new production attack surface beyond what the threat model documents (BLINDJOIN_BOT_* env vars, counter file — both documented in STRIDE register in 18-02-PLAN.md T-18-02-01 through T-18-02-06).

## Self-Check

### Created files exist:
- [x] `liquidity-bot/src/lib.rs` — `pub async fn run(config: BotConfig) -> Result<()>` exists
- [x] `tests/integration/bot_rotation.rs` — `fn bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs` exists
- [x] `.planning/phases/18-mixed-script-e2e-liquidity-bot/18-02-SUMMARY.md` (this file)

### Commits exist:
- [x] `5a59f87` (Task 1 — [lib] target + RotationState + 5 unit tests)
- [x] `8bb5dfa` (Task 2 — lib.rs::run() + main.rs wrapper + docker artifacts)
- [x] `56aff83` (Task 3 — bot_rotation.rs integration test + mod declaration)
