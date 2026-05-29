# Phase 13: client/src/wallet.rs wire-format fix + Plan 12-02 unmute cycle re-execution — Pattern Map

**Mapped:** 2026-05-28
**Files analyzed:** 4 (1 source + 1 test + 2 planning-state docs)
**Analogs found:** 4 / 4

This is a tightly-scoped fix phase. The wire-format encoding is constrained by the coordinator's deserialization site (which Phase 13 does NOT modify); the unmute cycle is a verbatim re-use of Plan 11-02-PLAN.md (third iteration of the same pinned spec); REQUIREMENTS.md and ROADMAP.md edits follow the per-row bookkeeping pattern from prior phases.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `client/src/wallet.rs` (lines 276-291, `sign_psbt_input` partial-sig extraction block) | source (client wallet) | transform (sig → wire bytes) | `coordinator/src/round/signing.rs:156-179` (the deserializer — defines the wire contract Plan 13-01 must round-trip through) | **inverse-pair exact match** (encoder ↔ decoder) |
| `tests/integration/full_round.rs` (six `#[ignore]` line deletions at lines 164, 462, 730, 854, 911, 1236) | test (integration, attribute-only edits) | n/a (attribute removal) | `.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md` (the pinned spec Plan 13-02 reuses verbatim) | **verbatim reuse** — third iteration of identical spec |
| `.planning/REQUIREMENTS.md` (line 20, REPAIR-01 `[x]`/`[ ]` reconciliation per D-14) | planning-state (doc) | n/a (single-line status flip) | Phase 12 D-13 per-row bookkeeping pattern (same closeout shape) | **convention match** |
| `.planning/ROADMAP.md` (Phase 13 entry marked complete in closeout commit) | planning-state (doc) | n/a (single-line status flip) | Phase 12 D-13 per-row bookkeeping pattern | **convention match** |

---

## Pattern Assignments

### `client/src/wallet.rs` lines 276-291 (source — transform)

**Plan:** 13-01 (single atomic commit)

**Analog — the wire-format contract (read-only context, do NOT modify):**
`coordinator/src/round/signing.rs` lines 156-179

```rust
// Apply partial signatures as witness data to each input.
// Each participant submitted their signature + pubkey as serialized witness bytes.
// We decode these and set them as the witness for the corresponding input.
for (i, input) in psbt.unsigned_tx.input.iter().enumerate() {
    let outpoint_str = format!("{}:{}", input.previous_output.txid, input.previous_output.vout);
    if let Some(sig_bytes) = inner.partial_sigs.get(&outpoint_str) {
        // Deserialize the witness from the raw bytes the client sent
        match bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes) {
            Ok(witness) => {
                psbt.inputs[i].final_script_witness = Some(witness);
            }
            Err(_) => {
                return Err(ApiError {
                    code: ErrorCode::BroadcastRejected,
                    message: format!("Invalid witness data for input {}", i),
                    round_id: Some(round_id_str.to_string()),
                });
            }
        }
    } else {
        return Err(ApiError {
            code: ErrorCode::BroadcastRejected,
            message: format!("Missing signature for input {}", i),
            round_id: Some(round_id_str.to_string()),
        });
    }
}
```

**Contract Plan 13-01 must satisfy:** `bitcoin::consensus::deserialize::<bitcoin::Witness>(client_bytes)` MUST return `Ok(witness)` where `witness` is a 2-item P2WPKH stack `[sig_der_with_sighash, compressed_pubkey]`. The client encoder is the inverse of this decoder.

---

**Current state — lines 276-291 of `client/src/wallet.rs` (BROKEN):**

```rust
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
```

**Failure mode (the 5th orthogonal blocker):** `sig.to_vec()` returns raw DER + SIGHASH_ALL bytes (~71-72 bytes starting `0x30 0x4N ...`). The coordinator calls `consensus::deserialize::<Witness>()` on those bytes; the leading `0x30` is interpreted as a CompactInt witness-stack-item-count of 48, and deserialization fails. Coordinator returns HTTP 400 `Invalid witness data for input {i}` — exactly the panic captured at tests/integration/full_round.rs:248:22 in Plan 12-02.

---

**Encoding pattern — what to write at lines 279-281 (per D-01 / per Plan 12-02-SUMMARY.md §"Proposed Minimal Repair"):**

```rust
if let Some((pk, sig)) = input.partial_sigs.iter().next() {
    let mut witness = bitcoin::Witness::new();
    witness.push(sig.to_vec());        // ECDSA sig: DER + SIGHASH_ALL byte
    witness.push(pk.to_bytes());       // compressed pubkey (33 bytes)
    return Ok(bitcoin::consensus::serialize(&witness));
}
```

**Diff scope:** ~4 LOC changed (destructure `_pk` → `pk`; replace `Ok(sig.to_vec())` with three new lines + `return Ok(...)`).

---

**Imports pattern — currently in scope at `client/src/wallet.rs:1`:**

```rust
use bitcoin::{Network, OutPoint, Psbt, ScriptBuf, Txid};
```

`bitcoin::Witness` and `bitcoin::consensus` are NOT in the current `use` list. The fix uses fully-qualified paths (`bitcoin::Witness::new()`, `bitcoin::consensus::serialize(...)`) per the spec in CONTEXT.md D-01 — no `use` change is required. Verified: only the inline-scoped `use bitcoin::{Amount, TxOut, CompressedPublicKey}` patterns exist elsewhere in the file (lines 44, 244, 245), so the file's idiom is "inline `use` or fully-qualified path" rather than top-level imports for everything. Plan 13-01 follows the fully-qualified path idiom for minimal diff.

---

**Witness construction patterns elsewhere in the codebase (analogs for `bitcoin::Witness::new()`):**

Three call sites of `Witness::new()` exist, but all construct **empty** witnesses (placeholders for `TxIn`/`Transaction` fields). NONE currently populates a stack via `.push(...)` — Plan 13-01 is the first.

1. `coordinator/src/bitcoin/tx.rs:88` — `witness: Witness::new()` (empty placeholder in a `TxIn`).
2. `shared/src/bip322.rs:46` — `witness: Witness::new()` (empty in BIP-322 to_spend `TxIn`).
3. `shared/src/bip322.rs:69` — `witness: Witness::new()` (empty in BIP-322 to_sign `TxIn`).

The closest *conceptual* analog — a 2-item P2WPKH witness stack `[sig_with_sighash, serialized_pubkey]` — lives in **test code** at `shared/src/bip322.rs:86-108` (`make_bip322_witness`), where it is returned as a `Vec<Vec<u8>>` (not a `bitcoin::Witness`):

```rust
let mut sig_bytes = sig.serialize_der().to_vec();
sig_bytes.push(0x01); // SIGHASH_ALL

let witness_stack = vec![sig_bytes, pubkey.serialize().to_vec()];
(script_pubkey, witness_stack)
```

**Take-away for Plan 13-01:** The 2-item ordering `[sig, pubkey]` is the same as this test's `witness_stack`, but the test produces a `Vec<Vec<u8>>` while Plan 13-01 produces a `bitcoin::Witness` (then consensus-serializes). The `bitcoin::ecdsa::Signature::to_vec()` that bdk_wallet stores in `partial_sigs` already includes the SIGHASH byte appended (rust-bitcoin 0.32's `ecdsa::Signature::to_vec()` returns DER + 1-byte sighash), so Plan 13-01 does NOT need to manually `push(0x01)` the way `make_bip322_witness` does for raw secp256k1 sigs.

---

**Error-handling pattern — the existing `Err(anyhow!(...))` shape (lines 250, 274, 290):**

```rust
.ok_or_else(|| anyhow!("Our UTXO not found in PSBT"))?;
// ...
.map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;
// ...
Err(anyhow!("bdk_wallet did not produce a partial signature for our input"))
```

Plan 13-01 does NOT introduce any new error paths. The fix is purely a transform of the success-path return value. `consensus::serialize` is infallible for `bitcoin::Witness` (it always succeeds), so no `?` or `map_err` is needed on the new lines.

---

**In-source comment pattern — D-10 departure from Phase 12 D-08:**

Per D-10 (CONTEXT.md), Plan 13-01 does **NOT** add an in-source block comment at the fix locus. This is a deliberate departure from Phase 12 Plan 12-01's 13-line BIP-143 safety comment (which sat above the `sign(psbt, SignOptions { trust_witness_utxo: true, ... })` call at lines 258-273 — see `client/src/wallet.rs:258-271` for the existing comment style, which is the pattern Plan 13-01 chooses NOT to follow). Rationale: Witness construction is canonical P2WPKH shape — no threat model to explain. The rationale lives in the commit body (failure signature, deserialization-site cite, 12-02-SUMMARY.md cross-ref).

For reference (the Phase 12 D-08 comment Plan 13-01 does NOT emulate), `client/src/wallet.rs:258-271`:

```rust
// bdk_wallet 2.3 changed SignOptions::default() to set trust_witness_utxo: false as a BIP-143
// fee-spoof mitigation: with only witness_utxo populated (no non_witness_utxo), a malicious
// PSBT creator could set a falsified witness_utxo.value to trick the signer into authorizing
// excessive fee. See: https://blog.trezor.io/details-of-firmware-updates-for-trezor-one-version-1-9-1-and-trezor-model-t-version-2-3-1-1eba8f60f2dd
// ...
```

Plan 13-01 keeps the WIRE-FORMAT code terse — commit body carries the WHY.

---

**Commit body shape — Plan 12-01's bisect-clean discipline + D-07 sanity capture addition:**

Plan 13-01 mirrors Plan 12-01-SUMMARY.md's commit shape (single source-fix commit with rationale-in-body) and ADDS the D-07 canonical-first PASS-proof capture. Commit body MUST contain:

1. Failure signature: `HTTP 400 Bad Request from /round/sign`
2. Coordinator-side deserialization site cite: `coordinator/src/round/signing.rs:160`
3. Cross-ref: `.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-02-SUMMARY.md §"Root Cause"`
4. D-07 canonical-first PASS verdict (via the same invocation pattern Plan 13-02 reuses):
   ```
   BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
     cargo test --test integration full_round::full_round_three_clients -- --ignored
   ```
   followed by `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ...`
5. CD-4: `bitcoind --version | head -1` output (detects silent brew bumps)

Default subject per CD-1: `fix(13): encode partial sig as bitcoin::Witness for /round/sign wire format (client/src/wallet.rs)`

---

### `tests/integration/full_round.rs` (test — six attribute-only deletions)

**Plan:** 13-02 (six atomic commits, canonical-first then file order)

**Analog — the pinned spec Plan 13-02 reuses verbatim:**
`.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md` (whole-file template)

**Per-test commit cycle pattern (Task 1 from 11-02-PLAN.md, abbreviated):**

```
(a) Capture prerequisite-fix SHAs via `git log --grep="^fix(11): switch client RSA pubkey decode" --format=%H -1`
    For Plan 13-02: capture three SHAs:
      RSA_FIX_SHA       = cc20f6fbca4d292bf7b394a3850b18d244b5b602  (Phase 11)
      WALLET_TRUST_SHA  = 0bbcf3c76ca251c14aa64216ca6955be1f880b9a  (Phase 12)
      WIRE_FORMAT_SHA   = $(git log --grep="^fix(13): encode partial sig" --format=%H -1)  (Phase 13 Plan 01)

(b) Run per-test invocation: BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
    cargo test --test integration full_round::<test_fn> -- --ignored 2>&1 \
    | tee target/integration-test-13-02-${N}.log
    Capture verdict line: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; N filtered out`

(c) If verdict is NOT a PASS: HALT IMMEDIATELY per D-12 (CONTEXT.md). Do NOT delete the #[ignore] line.
    Emit CHECKPOINT REACHED marker — Phase 14 absorbs per D-13.

(d) On PASS: delete EXACTLY the single #[ignore] line. Verify with `git diff` that
    the diff is `1 file changed, 0 insertions(+), 1 deletion(-)`.

(e) Commit atomically with subject `test(13): unmute <test_fn> (Phase-10 carve-out N/6)`
    and three-SHA body per D-05 of Phase 13 CONTEXT.md.

(f) Verify post-commit incrementally: EXPECTED=$((2 + N)) and grep cargo output for
    `test result: ok. ${EXPECTED} passed`.
```

**Per-commit body shape — three-SHA extension of CD-1 (per D-05 of Phase 13 CONTEXT.md):**

```
cargo test --test integration full_round::<test_fn> -- --ignored
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; N filtered out

bitcoind --version | head -1: Bitcoin Core daemon version v31.0.0 bitcoind

Prereqs in history:
  RSA fix (Phase 11):        cc20f6fbca4d292bf7b394a3850b18d244b5b602
  Wallet-trust fix (Phase 12): 0bbcf3c76ca251c14aa64216ca6955be1f880b9a
  Wire-format fix (Phase 13):  <Plan 13-01 SHA — captured via git log --grep at execute time>
```

**Six unmute sites (verified by `grep -n "TODO(Phase-10)" tests/integration/full_round.rs`):**

```
164:#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
462:#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
730:#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
854:#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
911:#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
1236:#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]
```

Each `#[ignore]` is immediately above an `async fn <test_name>() {` line (verified at lines 165 and 463). The attribute-deletion edits are bit-identical in shape: remove ONE line, leave the `#[tokio::test]` above and the `async fn …()` below byte-identical.

**Canonical-first order:** `full_round_three_clients` (line 164) MUST be the 1st unmute. Remaining 5 follow in file order (462, 730, 854, 911, 1236) per D-05 of Plan 11-02-PLAN.md (inherited unchanged).

**Six prescribed commit subjects (per Plan 11-02-PLAN.md acceptance criteria, subject prefix `test(11):` rewritten to `test(13):`):**

```
test(13): unmute full_round_three_clients (Phase-10 carve-out 1/6)
test(13): unmute blame_non_signer_timeout (Phase-10 carve-out 2/6)
test(13): unmute adversarial_replay_token (Phase-10 carve-out 3/6)
test(13): unmute adversarial_invalid_utxo (Phase-10 carve-out 4/6)
test(13): unmute adversarial_wrong_denomination (Phase-10 carve-out 5/6)
test(13): unmute round_restart_and_completion_after_blame (Phase-10 carve-out 6/6)
```

**Line-drift discipline (T-11-13 from Plan 11-02-PLAN.md threat register, inherited):** After each delete, all subsequent line numbers shift down by 1. Plan 13-02 re-greps for `#[ignore = "TODO(Phase-10)`-prefixed lines by adjacent test-fn name at each iteration (NOT by trusting pre-Phase-13 line numbers from the spec).

---

### `.planning/REQUIREMENTS.md` line 20 (planning-state — D-14 reconciliation)

**Plan:** 13-02 (final closeout commit, after all six PASS-proof captures land)

**Analog:** Phase 12 D-13 per-row bookkeeping (same per-row `[x]`/`[ ]` flip + commit-body provenance documentation).

**Current state (the doc drift to be reconciled):**

```
20:- [x] **REPAIR-01**: `tests/integration/full_round.rs` is either repaired (all 8 tests pass against the pinned bitcoind version, including the 6 currently failing on `listunspent`/RPC schema drift) **OR** explicitly retired with rationale captured in TODO.md and the file deleted from the repo
```

REPAIR-01 currently shows `[x]` despite the suite never having been green (Plan 12-02 halted before any unmute). Per D-14:

- **If all 8 full_round tests pass locally** in Plan 13-02: REPAIR-01 **stays `[x]`**, and the commit message documents the corrected provenance (Phase 13 SHAs replacing the pre-Plan-12 drift attribution). ROADMAP Phase 13 entry is marked complete in the SAME commit.
- **If any test is red**: REPAIR-01 **flips back to `[ ]`** with the failure surfaced in the commit message; ROADMAP Phase 13 entry is NOT marked complete; Phase 14 opens.

**Edit pattern — single-character `[ ]` ↔ `[x]` flip on one line. No structural change to the file.**

---

### `.planning/ROADMAP.md` Phase 13 entry (planning-state — closeout)

**Plan:** 13-02 (same closeout commit as REPAIR-01 reconciliation, per D-14)

**Analog:** Phase 8 entry shape (lines 32-36) — completed phases use `✅` and a `[x]` checkbox; Phase 9 (lines 38-40) shows the same pattern for an in-progress milestone.

**Current state of Phase 13 entry (lines 139-147):**

```
### Phase 13: client/src/wallet.rs wire-format fix + Plan 12-02 unmute cycle re-execution

**Goal:** [To be planned]
**Requirements**: TBD
**Depends on:** Phase 12
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd-plan-phase 13 to break down)
```

**Edit pattern — same as Phase 12 D-13 closeout flips:** in the green-suite case, change the Plans list entries to `[x]` and add a completion date. The Phase 13 entry's `[ ]` becomes `[x]` only if REPAIR-01 closes locally (8/8 green).

**Same-commit coupling per D-14:** REQUIREMENTS.md line 20 flip + ROADMAP Phase 13 entry flip MUST land in the SAME closeout commit (per Phase 12 D-13 convention) — atomic doc reconciliation.

---

## Shared Patterns

### Canonical local test invocation (CONTRIBUTING.md §"Running integration tests")

**Source:** `CONTRIBUTING.md` §"Running integration tests" (referenced by both plans)
**Apply to:** Plan 13-01's D-07 sanity capture; each of Plan 13-02's six PASS-proof captures

```bash
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::<test_name> -- --ignored 2>&1 \
  | tee target/integration-test-13-<NN>.log
```

The `$(brew --prefix)` form per CD-2 is preserved literally in commit bodies (reproduces on any reviewer's machine).

### bitcoind version capture in every commit body (CD-4)

**Source:** Phase 12 CD-4 (inherited from Phase 11 CD-4)
**Apply to:** Plan 13-01's commit body; each of Plan 13-02's six commit bodies; the closeout commit body

```bash
bitcoind --version | head -1
# Bitcoin Core daemon version v31.0.0 bitcoind
```

Detects silent brew bumps moving bitcoind off pinned v31.

### Per-test atomic commit with PASS verdict in body (Phase 10 D-07, Phase 11 D-07, Phase 12 D-04 origin)

**Source:** `11-02-PLAN.md` acceptance criteria (lines 226-245)
**Apply to:** Each of Plan 13-02's six unmute commits

```
git diff <commit>~1 <commit> --stat
# Expected: 1 file changed, 0 insertions(+), 1 deletion(-)
```

Drift indicator: any commit with `N insertions(+)` for `N > 0` violates D-07 (no drive-by edits, no test-body changes, no whitespace edits).

### D-11/D-12 escape-valve halt-and-surface protocol (Phase 11 origin, Phase 12 first invocation)

**Source:** `12-02-SUMMARY.md` "D-11 Escape-Valve Invoked" section
**Apply to:** Plan 13-01 D-07 sanity capture; each of Plan 13-02's six per-test captures

If any per-test invocation FAILS:
1. Do NOT delete the corresponding `#[ignore]` line.
2. Do NOT commit.
3. Do NOT proceed to subsequent unmutes.
4. Emit a CHECKPOINT REACHED marker naming: the failing test, the failure signature (panic message / cargo verdict / first-encountered HTTP status), and a proposed minimal repair.
5. Phase 14 absorbs the 6th orthogonal blocker per D-13.

Pre-authorized in-flight scope expansion is ZERO (per CONTEXT.md D-12).

### Same-commit REQUIREMENTS.md + ROADMAP.md doc reconciliation (Phase 12 D-13)

**Source:** Phase 12 D-13 per-row bookkeeping convention
**Apply to:** Plan 13-02's closeout commit (after all six PASS-proof unmutes land)

Single atomic commit that:
1. Flips REQUIREMENTS.md line 20 REPAIR-01 status per D-14 (stays `[x]` if green, flips to `[ ]` if red).
2. Marks ROADMAP.md Phase 13 entry complete (only if green).
3. Documents provenance in commit message (Phase 13 SHAs, not pre-Plan-12 drift attribution).

---

## No Analog Found

No files in scope lack an analog. All four modification surfaces have direct precedent:

- Wire-format encoder ↔ existing coordinator-side decoder (inverse pair).
- Six-test unmute cycle ↔ Plan 11-02-PLAN.md verbatim (third iteration).
- REQUIREMENTS.md + ROADMAP.md flips ↔ Phase 12 D-13 closeout convention.

---

## Metadata

**Analog search scope:**
- `coordinator/src/round/signing.rs` (the wire-format contract — read-only)
- `coordinator/src/bitcoin/tx.rs` (Witness::new placeholder analog)
- `shared/src/bip322.rs` (the 2-item P2WPKH stack analog, in test code)
- `client/src/wallet.rs` (the fix locus — full import context + existing in-source comment style)
- `tests/integration/full_round.rs` (the six unmute sites — verified by grep)
- `.planning/phases/11-…/11-02-PLAN.md` (the pinned spec Plan 13-02 reuses verbatim)
- `.planning/phases/12-…/12-01-SUMMARY.md` (Plan 13-01's commit-shape analog)
- `.planning/phases/12-…/12-02-SUMMARY.md` (the 5th-blocker root-cause record — Plan 13-01's cross-ref target)
- `.planning/REQUIREMENTS.md` (REPAIR-01 doc-drift reconciliation target)
- `.planning/ROADMAP.md` (Phase 13 entry closeout target)

**Files scanned:** 10
**Pattern extraction date:** 2026-05-28
