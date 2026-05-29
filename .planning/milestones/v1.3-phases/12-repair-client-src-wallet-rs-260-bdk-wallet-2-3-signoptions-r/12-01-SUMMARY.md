---
phase: 12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r
plan: 01
subsystem: client-wallet
tags:
  - client-wallet
  - bdk-wallet-2-3
  - sign-options
  - trust-witness-utxo
  - bip-143
  - one-line-fix
  - d-08-block-comment
  - 5th-orthogonal-blocker-400-bad-request
requires:
  - "Phase 11 Plan 01 (cc20f6f RSA SPKI fix) — in history"
  - "bdk_wallet 2.3.0 — pinned in Cargo.lock"
provides:
  - "client/src/wallet.rs sign_psbt_input: trust_witness_utxo: true at sign call — bdk_wallet 2.3 MissingNonWitnessUtxo guard bypassed for P2WPKH inputs"
  - "Wallet-fix SHA 0bbcf3c76ca251c14aa64216ca6955be1f880b9a — for Plan 12-02 commit bodies"
affects:
  - "client/src/wallet.rs (line 258-273 — sign call vicinity)"
tech-stack:
  added: []
  patterns:
    - "D-08 multi-line safety comment above bdk_wallet sign call (three-part: threat model / why safe here / precondition for revisit)"
key-files:
  created: []
  modified:
    - path: client/src/wallet.rs
      lines_before: 258-261
      lines_after: 258-275
      change: "Sign via bdk_wallet (1 comment + 1 call + 1 continuation) → D-08 block comment (13 lines) + #[allow(deprecated)] + struct-literal call + continuation"
decisions:
  - "D-01 Option A applied: trust_witness_utxo: true via struct-literal, not Option B (populate non_witness_utxo from RPC)"
  - "D-08 block comment includes all three required parts: BIP-143 threat model (Part 1), local-context safety argument citing self.utxo_value_sats (Part 2), precondition for revisit naming Option B and mainnet enablement (Part 3)"
  - "D-10 cross-reference: 11-02-SUMMARY.md cited by exact path in Part 3 of block comment"
  - "5th orthogonal blocker discovered: coordinator /round/sign returns 400 Bad Request (not Missing non-witness UTXO) — wallet repair is bisect-clean; Plan 12-02 D-11 escape-valve absorbs"
metrics:
  tasks_completed: 1
  files_modified: 1
  lines_added: 13
  lines_removed: 2
  commits: 1
  duration_seconds: ~480
  completed: 2026-05-28
  status: complete
---

# Phase 12 Plan 01: Wallet SignOptions Repair Summary

## One-liner

Applied `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }` with a three-part D-08 BIP-143 safety comment at the sole bdk_wallet sign call in `client/src/wallet.rs`; wallet repair is compile-clean and bisect-clean; a 5th orthogonal blocker (coordinator 400 Bad Request at `/round/sign`) surfaces for Plan 12-02.

## What Was Built

### Task 1: Apply Option A — trust_witness_utxo: true with D-08 multi-line block comment

**Fix locus:** `client/src/wallet.rs` lines 258-261 (before edit) → lines 258-275 (after edit, +13 lines from D-08 comment).

**Functional change (1 line):**

Before:
```rust
self.inner.sign(psbt, SignOptions::default())
```

After:
```rust
self.inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..SignOptions::default() })
```

**D-08 block comment (13 lines, immediately above the `#[allow(deprecated)]` attribute):** Three-part safety contract:
- Part 1: bdk_wallet 2.3 changed `SignOptions::default()` to set `trust_witness_utxo: false` as a BIP-143 fee-spoof mitigation; cites the Trezor firmware blog URL from bdk_wallet's own doc comment.
- Part 2: `trust_witness_utxo: true` is safe HERE because the client constructs `witness_utxo` from `self.utxo_value_sats` (trusted regtest RPC origin), not from a counterparty PSBT.
- Part 3: Precondition for revisit — any future code path where `witness_utxo.value` comes from an untrusted counterparty PSBT; cites `.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md §"Two minimal-repair candidates"` by exact path (D-10).

## Verification Results

### Source Assertions (all PASS)

| Assertion | Expected | Actual |
|-----------|----------|--------|
| `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }` count | 1 | 1 |
| `SignOptions::default()` occurrences (1 in comment + 1 in struct-update) | 2 | 2 |
| Standalone `sign(psbt, SignOptions::default())` form | 0 | 0 |
| `BIP-143` count | ≥1 | 1 |
| `11-02-SUMMARY.md` cross-reference | 1 | 1 |
| `trust_witness_utxo: true is safe HERE` | ≥1 | 1 |
| `utxo_value_sats` in comment | ≥1 | 1 (in struct field listing) |
| `get_raw_transaction` (Option B revisit phrase) | ≥1 | 1 |
| `TODO(mainnet)` markers (D-09 — MUST be 0) | 0 | 0 |
| `^#[cfg(test)]` (D-06 — MUST be 0) | 0 | 0 |
| `#[allow(deprecated)]` count (D-02 — preserved) | 2 | 2 |

### Build and Lint (both PASS)

```
cargo build --workspace --all-targets
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.83s

cargo clippy --workspace --all-targets -- -D warnings
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.03s
```

### Local-bitcoind Sanity (GO/NO-GO signal for Plan 12-02)

```
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::full_round_three_clients -- --ignored

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 14 filtered out; finished in 4.29s
```

**Verdict: NO-GO (5th orthogonal blocker) — but wallet repair is bisect-clean.**

The `Missing non-witness UTXO` error is GONE. The new failure signature is:

```
verify_and_sign: HTTP status client error (400 Bad Request) for url (http://127.0.0.1:.../round/sign)
```

The sign call now completes without error (past the bdk_wallet guard), but the coordinator rejects the `/round/sign` POST with 400. This is a 5th orthogonal blocker that surfaces upstream of the wallet's sign call — the wallet repair landed correctly and is bisect-clean.

**bitcoind version (CD-4):** `Bitcoin Core daemon version v31.0.0 bitcoind`

### Commit Verification

```
git log -1 --format=%s
fix(12): trust_witness_utxo for bdk_wallet 2.3 SignOptions (client/src/wallet.rs sign call)

git show HEAD --name-only | tail -3
client/src/wallet.rs   (exactly 1 file — D-02 honored)

Wallet-fix SHA: 0bbcf3c76ca251c14aa64216ca6955be1f880b9a
```

**Commit diff:** 1 file, 15 insertions, 2 deletions (within the ≤15 LOC bound).

## Decision Coverage

| Decision | Status |
|----------|--------|
| D-01 (Option A — trust_witness_utxo: true) | APPLIED |
| D-02 (fix locus is sign call only — no other wallet edits) | HONORED |
| D-06 (no new wallet-level unit test) | HONORED |
| D-08 (multi-line block comment with three-part safety contract) | APPLIED |
| D-09 (no in-source mainnet TODO) | HONORED |
| D-10 (cross-reference 11-02-SUMMARY.md by exact path) | APPLIED |
| CD-1 (commit subject) | APPLIED |
| CD-4 (bitcoind version in commit body) | APPLIED |

## 5th Orthogonal Blocker — Plan 12-02 Input

**Failure signature:** `verify_and_sign: HTTP status client error (400 Bad Request) for url (.../round/sign)`

**What this means:** The sign call itself now succeeds (the `Missing non-witness UTXO` guard is bypassed). The 400 occurs when the client posts the partial signature to the coordinator's `/round/sign` endpoint. Root cause is unknown — likely a signature format mismatch or a changed PSBT/signature encoding that the coordinator now rejects. Plan 12-02's D-11 escape-valve should surface the exact coordinator error body.

**Plan 12-02 required action:** Before the canonical-first unmute, investigate the 400 failure. Run with `RUST_LOG=debug` or check the coordinator logs from the test. If the 400 has a clear coordinator-side error message, include it in the Plan 12-02 checkpoint with a proposed minimal repair.

## Deviations from Plan

**1. [Rule None - 5th Orthogonal Blocker Discovery] coordinator /round/sign returns 400 Bad Request**

- **Found during:** Task 1 step (e) — local-bitcoind sanity invocation
- **Issue:** After the wallet repair, the sign call completes, but the coordinator rejects the `/round/sign` POST with 400 Bad Request. This is NOT `Missing non-witness UTXO`.
- **Action taken:** Per plan step (e) — "If FAIL with a NEW signature: commit the wallet fix as-is (the source-level repair is complete and bisect-clean), capture the new failure signature in the commit body as a NOTE, and let Plan 12-02's own D-11 escape-valve handle the new blocker."
- **No source changes made as a result** — wallet fix is complete; 400 blocker is for Plan 12-02.
- **Commit:** 0bbcf3c includes the new failure signature in the commit body as a NOTE.

## Boundary Verification (D-02, D-06, D-07, D-09)

- **D-02 (no edits outside sign-call vicinity):** `git show HEAD --name-only` returns exactly `client/src/wallet.rs`. Within the file, only lines 258-261 changed (replaced by lines 258-275). Struct, constructors, peek_address, secret_key_for_signing, witness_utxo population (lines 252-256), and partial-sig extraction (lines 263+) are byte-identical.
- **D-06 (no unit tests):** `grep -c "^#[cfg(test)]" client/src/wallet.rs` returns 0.
- **D-07 (no new integration test files):** No new files created. `tests/integration/full_round.rs` unchanged.
- **D-09 (no in-source mainnet TODO):** `grep -c "TODO(mainnet)" client/src/wallet.rs` returns 0.

## Plan 12-02 Pointer

Re-capture the wallet-fix SHA via:
```bash
git log --grep="^fix(12):" --format=%H -1
# Returns: 0bbcf3c76ca251c14aa64216ca6955be1f880b9a
```

Use this alongside `cc20f6fbca4d292bf7b394a3850b18d244b5b602` (Phase 11 RSA fix) in each of the six unmute commit bodies.

**Critical for Plan 12-02:** The 5th orthogonal blocker (400 Bad Request from coordinator `/round/sign`) must be diagnosed before the canonical-first unmute commit can be made. Plan 12-02 should open with the D-11 escape-valve investigation rather than directly proceeding to the unmute cycle.

## Known Stubs

None — the sign call change is fully wired. `self.utxo_value_sats` is the live field from wallet construction. No placeholder values, no hardcoded empty data.

## Threat Flags

No new threat surface introduced. The D-08 comment strengthens the existing trust boundary documentation. The only pre-existing surface (`sign_psbt_input` accepting `witness_utxo.value` without `non_witness_utxo`) is now explicitly documented with its safety argument and the precondition for revisit.

## Self-Check: PASSED

- Files exist:
  - `client/src/wallet.rs` — FOUND and modified correctly (verified via grep).
  - `.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-01-SUMMARY.md` — FOUND (this file).
  - `target/integration-test-12-01-sanity.log` — FOUND (tee output from sanity invocation).
- Commits exist:
  - `0bbcf3c` (Plan 12-01 wallet fix) — VERIFIED (`git log -1 --format=%H`).
- Source assertions:
  - `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }` count = 1 — VERIFIED.
  - `BIP-143` count ≥ 1 — VERIFIED.
  - `11-02-SUMMARY.md` count = 1 — VERIFIED.
  - `TODO(mainnet)` count = 0 — VERIFIED.
  - `#[allow(deprecated)]` count = 2 (unchanged) — VERIFIED.
  - `^#[cfg(test)]` count = 0 — VERIFIED.
- Build/lint: `cargo build` and `cargo clippy` both `Finished` with no errors — VERIFIED.
- Sanity log: `target/integration-test-12-01-sanity.log` verdict line is NOT `Missing non-witness UTXO` — VERIFIED (it is `400 Bad Request`, a new failure signature).
