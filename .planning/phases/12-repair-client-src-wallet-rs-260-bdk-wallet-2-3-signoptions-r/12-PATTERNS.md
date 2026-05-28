# Phase 12: Repair client/src/wallet.rs:260 (bdk_wallet 2.3 SignOptions) + Plan 11-02 unmute cycle - Pattern Map

**Mapped:** 2026-05-28
**Files analyzed:** 2 (1 surgical edit to `client/src/wallet.rs`, 6 attribute removals in `tests/integration/full_round.rs`)
**Analogs found:** 2 / 2

## Scope Note

Phase 12 is narrower than Phase 11: two modification surfaces, both with strong in-file or cross-phase analogs. No new architectural patterns are introduced.

1. **Surface 1 (`client/src/wallet.rs:260`)** — one-line `SignOptions` struct-literal change + multi-line D-08 block comment. The in-file context (the rest of `sign_psbt_input`) is the primary pattern reference; the block-comment shape follows the `coordinator/src/network/tor.rs` semaphore-rationale pattern.
2. **Surface 2 (`tests/integration/full_round.rs`)** — six `#[ignore]` line removals, one per atomic commit, in canonical-first then file order. The analog is Plan 11-02's locked unmute spec, reused verbatim with only an additional SHA reference in each commit body.

## Sign-site Survey: `client/src/wallet.rs:260` is the ONLY bdk_wallet sign call

Grep across the entire workspace (excluding `target/` and `.planning/`) for `.sign(`:

```
coordinator/src/discovery/pkarr_pub.rs:94:  .sign(keypair)   — PKARR SignedPacket signing, NOT bdk_wallet
client/src/wallet.rs:260:  self.inner.sign(psbt, SignOptions::default())   — THE fix locus
```

`coordinator/src/discovery/pkarr_pub.rs:94` is `SignedPacket::builder()...sign(keypair)` — a PKARR keypair API completely unrelated to `bdk_wallet::Wallet::sign`. There is no `SignOptions` anywhere except `client/src/wallet.rs`. The coordinator does not sign PSBTs.

**Planner implication:** Plan 12-01's scope is `client/src/wallet.rs` only. No sibling sign sites exist in coordinator/, tests/, or liquidity-bot/. No scoping-out decision is needed for other files.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `client/src/wallet.rs` (line 260 + D-08 block comment) | wallet/signing | request-response (PSBT sign) | `client/src/wallet.rs:243-278` (the `sign_psbt_input` method body itself — in-file) + `coordinator/src/network/tor.rs:73-84` (multi-line safety comment shape) | exact (in-file); role-match (comment pattern) |
| `tests/integration/full_round.rs` (six `#[ignore]` removals at lines 164, 462, 730, 854, 911, 1236) | integration-test attribute removal | n/a (attribute-only edit) | `11-02-PLAN.md` locked unmute spec (Surface 3 / Tasks 1-2) + Phase 11 PATTERNS.md Surface 3 | exact (verbatim carry-forward per D-04) |

## Pattern Assignments

### Surface 1: `client/src/wallet.rs:260` — SignOptions struct-literal change + D-08 block comment

**Analog:** `client/src/wallet.rs:243-278` (full `sign_psbt_input` method — read in one pass)

**Full method context** (`client/src/wallet.rs:243-278`):

```rust
/// Sign a PSBT input corresponding to our UTXO.
///
/// Sets witness_utxo on the correct input, then calls wallet.sign().
/// Returns the partial signature bytes (DER + SIGHASH_ALL) for POST /round/sign.
pub fn sign_psbt_input(&self, psbt: &mut Psbt) -> Result<Vec<u8>> {
    use bitcoin::Amount;
    use bitcoin::TxOut;

    // Find our input in the PSBT
    let input_idx = psbt.unsigned_tx.input.iter()
        .position(|inp| inp.previous_output == self.utxo_outpoint)
        .ok_or_else(|| anyhow!("Our UTXO not found in PSBT"))?;

    // Set witness_utxo — required for segwit signing
    psbt.inputs[input_idx].witness_utxo = Some(TxOut {
        value: Amount::from_sat(self.utxo_value_sats),   // <-- trusted-origin field
        script_pubkey: self.utxo_script_pubkey.clone(),
    });

    // Sign via bdk_wallet
    #[allow(deprecated)]
    self.inner.sign(psbt, SignOptions::default())    // <-- LINE 260: THE FIX LOCUS
        .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;

    // Extract the partial signature from the signed input.
    // For P2WPKH, bdk_wallet populates partial_sigs with the ECDSA signature.
    let input = &psbt.inputs[input_idx];
    if let Some((_pk, sig)) = input.partial_sigs.iter().next() {
        return Ok(sig.to_vec());
    }

    // Fallback: final_script_witness was set (fully finalized input)
    if let Some(witness) = &input.final_script_witness {
        if let Some(sig_bytes) = witness.nth(0) {
            return Ok(sig_bytes.to_vec());
        }
    }

    Err(anyhow!("bdk_wallet did not produce a partial signature for our input"))
}
```

**What changes at line 260 (the one-line functional diff):**

```rust
// BEFORE:
self.inner.sign(psbt, SignOptions::default())

// AFTER:
self.inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..SignOptions::default() })
```

The `#[allow(deprecated)]` on line 259 stays as-is (already present; orthogonal, from Phase 10 WIF-D fix). No new import needed: `SignOptions` is already imported at lines 5-6:

```rust
// client/src/wallet.rs:5-6 (unchanged)
#[allow(deprecated)]
use bdk_wallet::signer::SignOptions;
```

**D-08 block comment — pattern and shape:**

The block comment replaces the existing two-line `// Sign via bdk_wallet` comment above line 259. The analog for this multi-line safety-rationale comment is the semaphore guard in `coordinator/src/network/tor.rs:73-84`:

```rust
// coordinator/src/network/tor.rs:73-84
// Phase 8 CR-02 defense-in-depth: refuse to start the accept loop with a
// zero-capacity semaphore. `CoordinatorConfig::validate()` is the primary
// fence; this `ensure!` makes the requirement local so a future caller
// that bypasses `run::run` (custom embedding, direct test invocation)
// still gets an actionable error instead of a silent deadlock on the
// first `Semaphore::acquire_owned().await`.
anyhow::ensure!(
    max_concurrent_connections >= 1,
    ...
```

**Pattern signature from that analog:** explain (1) what the default/guard does and why, (2) why this call site is safe despite the guard, (3) what future condition would invalidate the safety argument. The comment is local to the call site, not a doc comment on the function.

**D-08 block comment content (copy verbatim from RESEARCH.md §Option A):**

```rust
// bdk_wallet 2.3 changed SignOptions::default() to set trust_witness_utxo: false as a BIP-143
// fee-spoof mitigation: with only witness_utxo populated (no non_witness_utxo), a malicious
// PSBT creator could set a falsified witness_utxo.value to trick the signer into authorizing
// excessive fee. See: https://blog.trezor.io/details-of-firmware-updates-for-trezor-one-...
//
// trust_witness_utxo: true is safe HERE because:
//   - This client constructs witness_utxo from self.utxo_value_sats (set at wallet construction
//     from the regtest RPC we already trust as ground truth) — not from a counterparty PSBT.
//   - The client is the sole signer over its own UTXO; no untrusted PSBT creator is involved.
//
// What would change this: any future code path where witness_utxo.value comes from an
// untrusted counterparty's PSBT. At that point, Option B (populate non_witness_utxo from
// RPC via get_raw_transaction) becomes required. See .planning/phases/11-.../11-02-SUMMARY.md
// §"Two minimal-repair candidates" for full analysis.
```

**Complete replacement block (lines 258-261 before → after):**

```rust
// BEFORE (lines 258-261):
        // Sign via bdk_wallet
        #[allow(deprecated)]
        self.inner.sign(psbt, SignOptions::default())
            .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;

// AFTER (replace those 4 lines):
        // bdk_wallet 2.3 changed SignOptions::default() to set trust_witness_utxo: false as a BIP-143
        // fee-spoof mitigation: with only witness_utxo populated (no non_witness_utxo), a malicious
        // PSBT creator could set a falsified witness_utxo.value to trick the signer into authorizing
        // excessive fee. See: https://blog.trezor.io/details-of-firmware-updates-for-trezor-one-...
        //
        // trust_witness_utxo: true is safe HERE because:
        //   - This client constructs witness_utxo from self.utxo_value_sats (set at wallet construction
        //     from the regtest RPC we already trust as ground truth) — not from a counterparty PSBT.
        //   - The client is the sole signer over its own UTXO; no untrusted PSBT creator is involved.
        //
        // What would change this: any future code path where witness_utxo.value comes from an
        // untrusted counterparty's PSBT. At that point, Option B (populate non_witness_utxo from
        // RPC via get_raw_transaction) becomes required. See .planning/phases/11-.../11-02-SUMMARY.md
        // §"Two minimal-repair candidates" for full analysis.
        #[allow(deprecated)]
        self.inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..SignOptions::default() })
            .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;
```

**Error handling pattern** (unchanged — lines 260-261 become lines 272-273, same structure):

```rust
.map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;
```

**D-06 note — no unit test:** `client/src/wallet.rs` has no `#[cfg(test)] mod tests` block (confirmed by grep). D-06 forbids adding one for Phase 12. The `sign_psbt_input` path is exercised end-to-end by the unmuted `full_round.rs` suite.

---

### Surface 2: `tests/integration/full_round.rs` — six `#[ignore]` line removals

**Analog:** `11-02-PLAN.md` Tasks 1 and 2 (the complete locked unmute spec, carried forward verbatim per D-04). Phase 11 PATTERNS.md Surface 3 also applies without modification.

**The six unmute sites, canonical-first order (D-04/D-05):**

```
Order  Original Line  Test fn                                  Unmute number
-----  -------------  ---------------------------------------- -------------
1st    164            full_round_three_clients                 carve-out 1/6  (canonical gate)
2nd    462            blame_non_signer_timeout                 carve-out 2/6
3rd    730            adversarial_replay_token                 carve-out 3/6
4th    854            adversarial_invalid_utxo                 carve-out 4/6
5th    911            adversarial_wrong_denomination           carve-out 5/6
6th    1236           round_restart_and_completion_after_blame carve-out 6/6
```

**Attribute pair shape at all six sites (verified: lines 163-165 as the canonical example):**

```rust
#[tokio::test]
#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]  // DELETE THIS LINE
async fn full_round_three_clients() {
```

Each unmute = ONE line deletion only. `#[tokio::test]` above and `async fn ...() {` below are preserved byte-identical.

**Per-test commit cycle pattern (from 11-02-PLAN.md Tasks 1-2, unchanged):**

For each test N/6, in order:
1. Locate current `#[ignore]` line by adjacent `async fn <test_fn>()` (re-grep after each prior delete — line numbers shift by 1 per deletion).
2. Run with `--ignored`, capture PASS verdict.
3. On PASS: delete the `#[ignore]` line, stage, commit atomically.
4. On FAIL: D-08 escape-valve — HALT, emit CHECKPOINT REACHED, do NOT delete the line, do NOT proceed.

**Phase 12 commit subject convention (CD-1, adapting from Phase 11):**

The six commit subjects change phase number from `test(11):` to `test(12):`:
```
test(12): unmute full_round_three_clients (Phase-10 carve-out 1/6)
test(12): unmute blame_non_signer_timeout (Phase-10 carve-out 2/6)
test(12): unmute adversarial_replay_token (Phase-10 carve-out 3/6)
test(12): unmute adversarial_invalid_utxo (Phase-10 carve-out 4/6)
test(12): unmute adversarial_wrong_denomination (Phase-10 carve-out 5/6)
test(12): unmute round_restart_and_completion_after_blame (Phase-10 carve-out 6/6)
```

**Phase 12 commit body convention (CD-1 — adapted from 11-02-PLAN.md to add second SHA):**

```
cargo test --test integration full_round::<test_fn> -- --ignored
<paste VERDICT verbatim — e.g. "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out">

bitcoind --version: Bitcoin Core Daemon version v31.0.0
RSA fix: cc20f6fbca4d292bf7b394a3850b18d244b5b602  (Phase 11 Plan 01)
Wallet fix: <Plan 12-01 commit SHA — computed at runtime via git log --grep="^fix(12):" --format=%H -1>
```

**Key delta from Phase 11 commit bodies:** two SHA references instead of one. Both are required per CONTEXT.md D-04/CD-1. The `bitcoind --version` capture is required per CD-4.

**SHA capture at execution time:**
```bash
RSA_FIX_SHA=cc20f6fbca4d292bf7b394a3850b18d244b5b602   # known from 11-02-SUMMARY.md
WALLET_FIX_SHA=$(git log --grep="^fix(12):" --format=%H -1)  # computed after Plan 12-01 commits
```

**Test invocation (verbatim from CONTRIBUTING.md §"Running integration tests"):**
```bash
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::<test_fn> -- --ignored 2>&1
```

---

## Shared Patterns

### Multi-line safety comment structure
**Source:** `coordinator/src/network/tor.rs:73-84`
**Apply to:** the D-08 block comment in `client/src/wallet.rs` (Surface 1)

Three-part structure:
1. What the default/guard does and its threat-model motivation.
2. Why THIS call site is safe despite the guard (local context argument).
3. What condition would invalidate the safety assumption (precondition for revisit).

Comment is `//`-prefixed prose, not a doc comment (`///`). It lives directly above the affected call, not on the function signature.

### Atomic per-test commit cycle with PASS-proof body
**Source:** `11-02-PLAN.md` Task 1 action (e)-(f) and Task 2 action (e)-(f)
**Apply to:** all six unmute commits in Plan 12-02

Pattern: run test `--ignored` → capture verdict → delete `#[ignore]` line → commit with verdict in body → verify incremental pass count. Each commit touches exactly one line in one file (`0 insertions(+), 1 deletion(-)`).

### Canonical-first gate + D-08 escape-valve
**Source:** `11-02-PLAN.md` Task 1 action (c) / `11-CONTEXT.md` D-08
**Apply to:** Plan 12-02 Task 1 (the `full_round_three_clients` unmute)

If `full_round_three_clients` fails after Plan 12-01's wallet fix is in history, halt immediately. Do NOT delete the `#[ignore]` line. Do NOT attempt any of the remaining 5 unmutes. Emit CHECKPOINT REACHED. Pre-authorized in-flight scope expansion is ZERO.

### REPAIR-01 closure criterion
**Source:** `11-02-PLAN.md` §success_criteria + CONTEXT.md D-13
**Apply to:** Plan 12-02 post-execution bookkeeping (NOT within any individual unmute task)

REPAIR-01 flips to `[x]` in `.planning/REQUIREMENTS.md` only when ALL 8 full_round tests are locally green. The whole-suite final invocation is:
```bash
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round 2>&1 | tee target/integration-test-12-02-final.log
```
Expected verdict: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

---

## No Analog Found

None. Both surfaces have direct analogs.

---

## Explicitly Preserved Boundaries

The following files are out of scope for Phase 12 and must NOT be touched:

| File | Why preserved |
|------|---------------|
| `tests/integration/mod.rs` | Phase 9/10 fixtures (`require_bitcoind!`, `BitcoindGuard`, `fund_regtest`, `FundedSetup`) — consumed unchanged by unmuted tests |
| `client/src/round/input.rs` | Phase 11 fix locus (cc20f6f) — already landed; Phase 12 is client-wallet only |
| `coordinator/src/blind/rsa.rs` | Phase 11 RSA fix — already landed; no coordinator changes in Phase 12 |
| `shared/src/protocol.rs` | Wire format (`rsa_pubkey_der_b64`) frozen; rename deferred indefinitely |
| `.github/workflows/ci.yml` | Phase 10 CI job (`corepc-node-feature-pin-check` at 4026f50) — preserved |
| `CONTRIBUTING.md` | Phase 12 consumes its test invocation; does not modify it |
| All test bodies in `full_round.rs` | D-07: only the 6 `#[ignore]` attribute lines are deleted; zero test-body edits |
| All other files in `client/src/wallet.rs` | D-02: only line 260 (+ its replacement block comment) changes; constructors, struct, other methods untouched |

---

## Metadata

**Analog search scope:** `client/src/wallet.rs` (full, 300 lines); `coordinator/src/network/tor.rs` (targeted 73-84); `tests/integration/full_round.rs` (lines 155-175, 455-470); `11-02-PLAN.md` (full); Phase 11 PATTERNS.md (full).

**Sign-site grep result:** `client/src/wallet.rs:260` is the sole `bdk_wallet::Wallet::sign(psbt, SignOptions...)` call in the workspace. No sibling sites in coordinator/, tests/, or liquidity-bot/.

**Files scanned:** 7 (client/src/wallet.rs, coordinator/src/network/tor.rs, tests/integration/full_round.rs, coordinator/src/discovery/pkarr_pub.rs, client/src/round/input.rs, 11-02-PLAN.md, 11-PATTERNS.md)

**Pattern extraction date:** 2026-05-28
