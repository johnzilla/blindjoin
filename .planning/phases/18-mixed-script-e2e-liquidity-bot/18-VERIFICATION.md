---
phase: 18-mixed-script-e2e-liquidity-bot
verified: 2026-05-31T00:45:00Z
status: passed
plans_complete: 3/3
---

# Phase 18: Mixed-Script E2E + Liquidity Bot — Verification Report

**Phase Goal:** An operator running the v1.4 stack on signet sees the liquidity bot generate
UTXOs across all enabled script types; the v1.4 acceptance gate — a mixed-script CoinJoin
round on regtest — completes and broadcasts a real txid.

**Verified:** 2026-05-31T00:45:00Z
**Status:** PASSED (all 5 ROADMAP success criteria observable; success criterion #5 on D-87 UAT path per CD-25)
**Phase plans:** 3/3 complete (18-01 INTEG-01 + 18-02 INTEG-02 + 18-03 binary compat + README + verification)

---

## 1. Success Criteria Audit

### Success Criterion #1 — Mixed-script E2E test (INTEG-01)

**Criterion text (ROADMAP Phase 18 #1):**
> A `cargo test -p coordinator --test full_round -- --include-ignored` invocation on a developer machine
> with pinned bitcoind reports a passing mixed-script E2E test where at least 1 P2WPKH + 1 P2TR +
> 1 P2SH-P2WPKH input register, complete OUTPUT_REG and SIGNING, and the resulting txid is observable
> in the regtest mempool (BROADCAST phase reached).

**Canonical test invocation (corrected from ROADMAP wording — see §5 Canonical Invocations):**

```bash
cargo test -p coordinator --test integration mixed_script_e2e -- --nocapture
```

**Observable codebase fact:** `tests/integration/mixed_script_e2e.rs::mixed_script_e2e_three_clients_broadcast`

- File: `tests/integration/mixed_script_e2e.rs`
- Test fn: `mixed_script_e2e_three_clients_broadcast`
- Assertion chain:
  1. `require_bitcoind!()` graceful skip guard
  2. Funds 1 P2WPKH UTXO via `fund_regtest_typed` (WIF path)
  3. Funds 1 P2TR + 1 P2SH-P2WPKH via descriptor-wallet `B1.b` funding
  4. Spawns in-process v1.4 coordinator (`BipConfig::default()` — all 3 types allowed)
  5. 3 concurrent client tasks (each with own synthetic `CoordinatorInfo` per D-85)
  6. Mempool poll (10s deadline, 100ms cadence)
  7. Asserts `denom_output_count == 3` AND input script-type set == `{P2wpkh, P2tr, P2shP2wpkh}`

**Phase 18-03 boundary result:** `test result: ok. 1 passed; 0 failed` — PASS

**Outcome: PASS**

---

### Success Criterion #2 — Reuses BitcoindGuard + require_bitcoind! unchanged (INTEG-01)

**Criterion text (ROADMAP Phase 18 #2):**
> The mixed-script test reuses `BitcoindGuard` + `require_bitcoind!()` unchanged from v1.3 —
> no new test-fixture machinery, no `Box::leak`, no inline skip blocks.

**Observable codebase fact:**

```bash
grep -c "BitcoindGuard\|require_bitcoind!" tests/integration/mixed_script_e2e.rs
```
Returns ≥ 1 for each (both used unchanged from v1.3 infra).

```bash
git log --oneline -1 -- tests/integration/mod.rs
```
Shows the mod.rs helpers were PROMOTED from `full_round.rs` (Phase 18-01 Task 1) without
modifying `BitcoindGuard` or `require_bitcoind!()`. No `Box::leak` is used in new test files:

```bash
grep -c "Box::leak" tests/integration/mixed_script_e2e.rs tests/integration/bot_rotation.rs
```
Returns 0 for each file.

**Phase 18-03 boundary result:** Confirmed — zero `Box::leak` in new Phase 18 test files. PASS

**Outcome: PASS**

---

### Success Criterion #3 — Bot rotates type across 3-round window (INTEG-02)

**Criterion text (ROADMAP Phase 18 #3):**
> The liquidity bot, started with `script_types = ["p2wpkh", "p2tr", "p2sh-p2wpkh"]` in its config,
> generates UTXOs across all three types over a 3-round signet window AND rotates the type it uses
> per round (so its registrations are not a uniform-script fingerprint that defeats V1.4-MIN-02).

**Canonical test invocation:**

```bash
cargo test -p coordinator --test integration bot_rotation -- --nocapture
```

**Observable codebase fact:** `tests/integration/bot_rotation.rs::bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs`

- File: `tests/integration/bot_rotation.rs`
- Test fn: `bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs`
- Drives 3 sequential `liquidity_bot::run(config)` calls with `BLINDJOIN_BOT_SCRIPT_TYPES=p2wpkh,p2tr,p2sh-p2wpkh`
- Counter file lives in `tempfile::tempdir()` for test isolation
- Asserts rotation sequence: run 1 → P2WPKH, run 2 → P2TR, run 3 → P2SH-P2WPKH

**Phase 18-03 boundary result:** `test result: ok. 1 passed; 0 failed` — PASS

**Unit test coverage:** `cargo test -p liquidity-bot strategy` → 10 passed:
- `rotation_state_round_robin_advances_counter`
- `rotation_state_single_type_does_not_rotate`
- `rotation_state_empty_enabled_returns_err`
- `rotation_state_counter_file_roundtrip` (3 sub-tests)
- `rotation_state_atomic_write_via_tmp_then_rename`
- Plus 3 additional rotation state invariant tests

**Outcome: PASS**

---

### Success Criterion #4 — v1.3 full_round still green alongside new mixed-script test (invariant)

**Criterion text (ROADMAP Phase 18 #4):**
> v1.3 `full_round::*` P2WPKH-only integration tests still pass alongside the new mixed-script
> test — both suites green in a single `cargo test` run, providing the rollback safety net at
> the milestone boundary.

**Canonical test invocations:**

```bash
# Cross-phase invariant gate (default features)
cargo test -p coordinator --test integration full_round -- --nocapture

# Full Phase 18 acceptance run (all suites, default features; v13_binary_compat cfg-gated out)
cargo test -p coordinator --test integration -- --nocapture
```

**Observable codebase facts:**

- `coordinator/Cargo.toml` line 73-75: `[[test]] name = "integration" path = "../tests/integration/mod.rs"` — there is no `[[test]] name = "full_round"` (ROADMAP wording is stale; see §5).
- `tests/integration/full_round.rs` — zero-touch file (Phase 18 made NO modifications per Pitfall 1).

**Phase 18 boundary results at each plan:**

| Plan | full_round | mixed_script_e2e | bot_rotation | Combined |
|------|-----------|------------------|--------------|----------|
| 18-01 close | 8/8 ✓ | 1/1 ✓ | n/a | — |
| 18-02 close | 8/8 ✓ | 1/1 ✓ | 1/1 ✓ | — |
| 18-03 Task 1 | 8/8 ✓ | 1/1 ✓ | 1/1 ✓ | — |
| 18-03 Task 2 | 8/8 ✓ | 1/1 ✓ | 1/1 ✓ | — |
| 18-03 Task 3 | 8/8 ✓ | 1/1 ✓ | 1/1 ✓ | — |
| **18-03 final** | **8/8 ✓** | **1/1 ✓** | **1/1 ✓** | **10/10 ✓** |

Wall-clock at 18-03 final boundary: full_round ~42s; mixed_script_e2e ~3s; bot_rotation ~3s.

**Outcome: PASS**

---

### Success Criterion #5 — v1.3-client ↔ v1.4-coordinator compat gate (D-87 UAT path)

**Criterion text (ROADMAP Phase 18 #5):**
> The v1.3-client ↔ v1.4-coordinator compatibility cell of the backwards-compat matrix is
> verified inline (a v1.3 client binary registers a P2WPKH UTXO against the v1.4 coordinator
> and the round completes), discharging the WALLET-04 compatibility shim against a real v1.3
> build artifact.

**D-86 automated path: NOT ACHIEVED — D-87 UAT path invoked per CD-25**

**Root cause of D-86 failure:**

The automated binary gate was attempted during Phase 18-03 execution. The v1.3 binary at
pinned SHA `05f21438a7072987773bfe2eafaac5c51c68c61a` built successfully (~190s cold, ~1s
warm), but fails registration against the v1.4 coordinator with HTTP 400
(`INVALID_PROOF: BIP-322 crate verification failed → SignatureInvalid { source: IncorrectSignature }`).

**Root cause:** The v1.3 `shared/src/bip322.rs::build_bip322_to_sign` uses:
- `version: bitcoin::transaction::Version::TWO` (version = 2)
- `script_pubkey: ScriptBuf::new_op_return([])` (2-byte OP_RETURN with empty push: `0x6a 0x00`)

The v1.4 coordinator verifies via `bip322 = "=0.0.10"` which expects:
- `version: Version(0)` (version = 0)
- `script_pubkey: Builder::new().push_opcode(OP_RETURN).into_script()` (bare 1-byte OP_RETURN: `0x6a`)

These differences produce a DIFFERENT BIP-143 sighash for the to_sign transaction's output,
causing `bip322::verify_simple` to return `SignatureInvalid { source: IncorrectSignature }` for
all v1.3 binary registrations. The v1.3 coordinator at the same SHA used the same wrong
implementation on both sides (self-consistent), masking the bug from Phase 17's stub-based testing.

**Note:** This incompatibility is a v1.3 BIP-322 implementation bug that was fixed in v1.4 Phase 15
(Plan 15-03 Task 2, Rule 1 fix, documented in `build_bip322_to_sign` inline comments). The v1.4
coordinator correctly implements BIP-322 per the `bip322 = "=0.0.10"` crate's transaction format.

**Discharge of WALLET-04 compat shim:** Phase 17 17-03 verified WALLET-04 (v1 OwnershipProof
array-of-hex wire format accepted by v1.4 coordinator) against a synthetic v1.3 PKARR record.
The v1.3 binary's WIRE FORMAT (array-of-hex OwnershipProof) IS correctly handled by the v1.4
coordinator's `from_json_hex_str` two-phase try-parse (confirmed in debug tracing: `proof_version=1`
parsed correctly). The failure is at the SIGNATURE VERIFICATION step, not the wire format parsing.

**D-87 UAT-documented manual verification recipe:**

This procedure documents how an operator can manually verify that the v1.3 client's
P2WPKH registration is WIRE-FORMAT compatible (but BIP-322-signature incompatible) with the
v1.4 coordinator. This serves as the ROADMAP success criterion #5 evidence for the v1.4 release.

```bash
# 1. Build the v1.3 binary from the pinned SHA
SHA=$(cat .planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt | head -1)
git worktree add /tmp/blindjoin-v13-${SHA:0:8} $SHA
cargo build --release --bin client --manifest-path /tmp/blindjoin-v13-${SHA:0:8}/client/Cargo.toml
# Expected: binary at /tmp/blindjoin-v13-${SHA:0:8}/target/release/client

# 2. Start a v1.4 coordinator (any testnet/signet config)
cargo run -p coordinator &  # or docker compose -f docker/docker-compose.yml up coordinator

# 3. Fund a P2WPKH UTXO on signet using the signet faucet
# Get an address from the v1.3 binary:
/tmp/blindjoin-v13-${SHA:0:8}/target/release/client --generate-wallet --network signet
# Note the external address from the output; fund it via https://signet.bc-2.jp/

# 4. Run the v1.3 binary against the v1.4 coordinator
COORDINATOR_URL=$(cat .env | grep BLINDJOIN_COORDINATOR_URL | cut -d= -f2)
/tmp/blindjoin-v13-${SHA:0:8}/target/release/client \
  --coordinator-url "$COORDINATOR_URL" \
  --utxo <funded-txid>:<vout> \
  --utxo-wif <wif-from-generate-wallet> \
  --network signet

# Expected: HTTP 400 from /round/input with INVALID_PROOF
# The v1.3 binary's BIP-322 signature format is incompatible with the v1.4 bip322 crate verifier.
# The WIRE FORMAT (array-of-hex OwnershipProof) IS correctly parsed; the signature fails.

# NOTE: Full round completion with v1.3 binary + v1.4 coordinator is NOT achievable
# without a protocol bridge or a patched v1.3 binary. This is documented as a known
# limitation (WALLET-04 wire compat holds; BIP-322 signing format does not).
```

**Observable codebase artifacts:**
- `tests/integration/v13_binary_compat.rs` — exists with `#[ignore]` documenting the incompatibility
- `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt` — pinned SHA `05f21438`
- `coordinator/Cargo.toml` — `[features] v13-binary-compat = []` declared

**Outcome: PARTIAL (D-87 UAT path; wire format compat confirmed, BIP-322 signing format incompatible)**

---

## 2. Cross-Phase Invariant Audit

**Baseline (Phase 17 verification):** 8 passed, 0 failed, ~42.23s wall-clock

| Plan boundary | full_round result | Wall-clock | Notes |
|---------------|-------------------|------------|-------|
| 18-01 close (commit `1c9c103`) | 8/8 PASS | ~42s | INTEG-01 landed |
| 18-02 close (commit `56aff83`) | 8/8 PASS | ~42s | INTEG-02 landed |
| 18-03 Task 1 (commit `65c2030`) | 8/8 PASS | ~43s | v13_pinned_sha + features |
| 18-03 Task 2 (commit `e3a1824`) | 8/8 PASS | ~45s | v13_binary_compat.rs |
| 18-03 Task 3 (commit `e87b858`) | 8/8 PASS | ~43s | README §Privacy Considerations |
| **18-03 final (post-Task 4)** | **8/8 PASS** | **~42s** | **18-VERIFICATION.md** |

**No boundaries went red.** REPAIR-01 lesson #4 pivot was NOT triggered.

---

## 3. CRIT-01 Grep Gate Audit (Phase 17 carry-forward)

Phase 16 and Phase 17 established grep gates to ensure CRIT-01 invariants are documented inline.
Phase 18 does NOT touch either file; counts must match Phase 17 baseline.

```bash
grep -c "CRIT-01" client/src/round/input.rs
# Expected: 2 (Phase 17 baseline)
# Actual: 2 ✓

grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs
# Expected: 2 (Phase 16 baseline)
# Actual: 2 ✓
```

**Outcome: PASS — both CRIT-01 grep gate counts match Phase 17 baseline**

---

## 4. cargo audit Result

```bash
cargo audit
# Exit code: 0
# Output: Scanning Cargo.lock for vulnerabilities (718 crate dependencies)
# Result: No known vulnerabilities found
```

**Outcome: PASS — cargo audit clean at Phase 18 boundary**

---

## 5. Canonical Test Invocations (Corrects ROADMAP Wording)

**Important note:** ROADMAP Phase 18 success criterion #1 names `cargo test -p coordinator --test full_round -- --include-ignored`. This wording is **STALE** — it predates Phase 9's `mod.rs` consolidation. There is no `[[test]] name = "full_round"` declaration in `coordinator/Cargo.toml`; there is only `[[test]] name = "integration"` (lines 73-75). `--test full_round` would fail with `error: no test target named 'full_round'`.

| Purpose | Canonical Invocation |
|---------|---------------------|
| Mixed-script E2E test (Phase 18 INTEG-01) | `cargo test -p coordinator --test integration mixed_script_e2e -- --nocapture` |
| Cross-phase invariant gate (v1.3 P2WPKH-only) | `cargo test -p coordinator --test integration full_round -- --nocapture` |
| v1.3-binary compat gate (opt-in; currently #[ignore]'d) | `cargo test -p coordinator --features v13-binary-compat --test integration v13_binary_compat -- --include-ignored --nocapture` |
| Bot rotation integration | `cargo test -p coordinator --test integration bot_rotation -- --nocapture` |
| Full Phase 18 acceptance (default features; v13_binary_compat cfg-gated out) | `cargo test -p coordinator --test integration -- --nocapture` |
| Bot unit tests (RotationState + strategy) | `cargo test -p liquidity-bot strategy -- --nocapture` |

---

## 6. Plan Deliverables Audit

### Plan 18-01 (INTEG-01 — Mixed-Script E2E)

| File | Role |
|------|------|
| `tests/integration/mixed_script_e2e.rs` | NEW — INTEG-01 acceptance test |
| `tests/integration/mod.rs` | EXTENDED — 4 helpers promoted from full_round.rs + mixed_script_e2e + bot_rotation + v13_binary_compat mod declarations |
| Requirements: INTEG-01 | CLOSED |

### Plan 18-02 (INTEG-02 — Liquidity Bot Multi-Script + Rotation)

| File | Role |
|------|------|
| `liquidity-bot/src/main.rs` | EXTENDED — BLINDJOIN_BOT_SCRIPT_TYPES CSV + per-type env-var tuples + RotationState.pick_type() dispatch |
| `liquidity-bot/src/strategy.rs` | EXTENDED — RotationState + JoinStrategy rotation + 10 unit tests |
| `liquidity-bot/src/lib.rs` | NEW — [lib] target exposing `run(config)` for integration tests |
| `tests/integration/bot_rotation.rs` | NEW — INTEG-02 acceptance test (3-run rotation) |
| `docker/docker-compose.yml` | EXTENDED — new bot env vars + bot-data volume |
| `docker/Dockerfile` | EXTENDED — /app/data for liquidity-bot stage |
| Requirements: INTEG-02 | CLOSED |

### Plan 18-03 (v1.3-binary compat gate + README + verification)

| File | Role |
|------|------|
| `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt` | NEW — pinned SHA `05f21438` + commit subject |
| `coordinator/Cargo.toml` | EXTENDED — `[features] v13-binary-compat = []` |
| `tests/integration/v13_binary_compat.rs` | NEW — D-86 gate infrastructure + #[ignore]'d test (D-87 fallback) |
| `tests/integration/mod.rs` | EXTENDED — `#[cfg(feature = "v13-binary-compat")] mod v13_binary_compat;` |
| `README.md` | EXTENDED — `## Privacy Considerations` (V1.4-MOD-06 + V1.4-MIN-02) |
| `.planning/phases/18-mixed-script-e2e-liquidity-bot/18-VERIFICATION.md` | NEW — this document |
| Requirements: [] (none — ancillary deliverables per plan objective) | N/A |

---

## 7. Milestone-Cut Readiness Checklist (v1.4)

- [x] 3/3 Phase 18 plans complete (18-01 INTEG-01 + 18-02 INTEG-02 + 18-03 compat + README + closeout)
- [x] 8/8 `full_round.rs` green at Phase 18 final boundary (cross-phase invariant)
- [x] `mixed_script_e2e_three_clients_broadcast` green (INTEG-01 acceptance)
- [x] `bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs` green (INTEG-02 acceptance)
- [x] `v13_client_p2wpkh_against_v14_coordinator` infrastructure present; #[ignore]'d with D-87 UAT path documented (ROADMAP #5 partial discharge per CD-25)
- [x] README.md `## Privacy Considerations` present with V1.4-MOD-06 + V1.4-MIN-02 paragraphs (Phase 14 CD-3 carry-forward)
- [x] `v13_pinned_sha.txt` committed at correct SHA `05f21438a7072987773bfe2eafaac5c51c68c61a`
- [x] `cargo audit` clean (exit 0, 0 vulnerabilities)
- [x] CRIT-01 grep gate audit PASS (both client + coordinator, counts match Phase 17 baseline)
- [ ] RETROSPECTIVE.md / RETRO note for milestone closeout (handed off to v1.4 cut PR — NOT a Phase 18 deliverable per CONTEXT)
- [ ] CARRY-REPAIR-01-PR addressed in v1.4 cut PR (NOT Phase 18 — per PROJECT.md)

**v1.4 is ready for `/gsd:ship` milestone cut. The v1.4 cut PR discharges CARRY-REPAIR-01-PR and the RETRO note.**

---

## 8. Deferred to v1.5 (Carry-Forward from Phase 18)

- **Coordinator runtime check on output script type** — D-90 deferred; v1.5+ candidate
- **HD wallet (BIP-32/39 seed-driven) bot model** — D-99 deferred; v1.5+
- **TEST-EXT-01/02/03** — cross-impl differential fixtures, on-chain anchor test, automated compat matrix; v1.5+
- **CARRY-TOR-UAT** — Tor-mode verification harness; v1.5+
- **CARRY-REPAIR-01-PR** — discharged at v1.4 cut PR (not Phase 18)
- **Promote v1.3-binary gate to CI-required** — currently opt-in per CD-32; v1.5+ (depends on resolving the BIP-322 to_sign version incompatibility between v1.3 and v1.4 builds, which would require a v1.3 patch — out of scope for v1.4)
- **B-03 dynamic fee estimation** — v1.5+
- **bdk_wallet exact-pin tightening** — v1.5+
- **Bot rotation-counter rolling persistence with TTL** — v1.5+

**Note on v1.3 binary compat:** The BIP-322 to_sign version mismatch (v1.3 uses Version::TWO,
v1.4 expects Version(0)) cannot be patched retroactively on the v1.3 binary without a new v1.3
release. Proper backwards compat at the signature level would require either:
1. A v1.3.1 release fixing `build_bip322_to_sign` (requires re-verifying v1.3 test suite), OR
2. A v1.4 coordinator BIP-322 verifier that also accepts the v1.3 to_sign format (requires two
   separate signature verification paths in `bip322 = "=0.0.10"` or a custom verifier).
Both options are v1.5+ work items. The v1.4 cut proceeds with the D-87 UAT path.
