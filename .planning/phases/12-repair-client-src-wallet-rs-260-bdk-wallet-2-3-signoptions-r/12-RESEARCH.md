# Phase 12: Repair bdk_wallet 2.3 SignOptions — Research

**Researched:** 2026-05-28
**Domain:** bdk_wallet 2.3.0 signing API, BIP-143 fee-spoof mitigations, PSBT signing in regtest context
**Confidence:** HIGH (all key claims verified from bdk_wallet 2.3.0 source in Cargo registry)

---

## Summary

Phase 12 is a two-part repair. First, a single-line fix at `client/src/wallet.rs:260` changes
`SignOptions::default()` to `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }`.
Second, Plan 11-02's six-unmute cycle runs verbatim once that fix is in history.

The root cause is fully confirmed from the pinned bdk_wallet 2.3.0 source: `SignOptions::default()`
sets `trust_witness_utxo: false`, and `wallet/mod.rs:1884-1893` enforces that every non-taproot,
non-finalized PSBT input must have `non_witness_utxo` populated when `trust_witness_utxo` is false.
The client populates only `witness_utxo`, so signing fails with `Missing non-witness UTXO`.

The fix is Option A. The threat that `trust_witness_utxo: false` defends against (BIP-143
fee-spoof via crafted `witness_utxo.value`) is irrelevant here: the client itself constructs the
`witness_utxo.value` from `self.utxo_value_sats`, a field set at wallet construction from the
same regtest RPC the client already trusts. No untrusted counterparty supplies that value. The
attack vector simply does not exist in this code path.

**Primary recommendation:** Apply Option A — `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }` — with a D-08-specified block comment explaining the threat model, why it doesn't apply here, and what would change that. Total diff: 1 line of code + ~10 lines of comment.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Option A — `trust_witness_utxo: true`. Single-line functional change at line 260. Option B (populate `non_witness_utxo` from RPC) is rejected for Phase 12.
- **D-02:** Fix locus is `client/src/wallet.rs:260` only. No other changes in `client/src/wallet.rs`. The `#[allow(deprecated)]` on the `SignOptions` import at lines 5-6 stays as-is.
- **D-03:** Phase 12 owns REPAIR-01 closure. Wallet repair + Plan 11-02's six-unmute cycle both land in Phase 12.
- **D-04:** Reuse Plan 11-02's unmute spec verbatim. No rewriting, reordering, or re-justifying. Canonical-first order: `full_round_three_clients` (line 164) → lines 462, 730, 854, 911, 1236.
- **D-05:** Two-plan structure. Plan 12-01 = wallet repair (one commit). Plan 12-02 = six-test unmute cycle (six commits, canonical-first then file order). Plan 12-02 depends on 12-01.
- **D-06:** No new wallet-level unit test. Integration coverage is the end-to-end path via `full_round.rs`.
- **D-07:** No additional integration test file (e.g., `tests/integration/wallet_signing.rs`).
- **D-08:** Multi-line block comment above the `self.inner.sign(...)` call. Must explain: (1) what bdk_wallet 2.3 changed and why, (2) why `trust_witness_utxo: true` is safe here, (3) what would change the safety assumption. Cross-reference `11-02-SUMMARY.md` §"Two minimal-repair candidates".
- **D-09:** No `// TODO(mainnet):` marker in-source. Mainnet revisit lives in CONTEXT.md Deferred Ideas.
- **D-10:** Cross-reference Phase 11-02-SUMMARY in the block comment.
- **D-11:** D-08 escape-valve discipline applies to Plan 12-02. If a 5th orthogonal blocker appears, halt at first encounter.
- **D-12:** Phase 13 absorbs any 5th-blocker overflow.
- **D-13:** Mark REPAIR-01 `[x]` in REQUIREMENTS.md and update ROADMAP.md when all 8 full_round tests pass locally.
- **D-14:** v1.3 ship notes are NOT in scope for Phase 12.

### Claude's Discretion

- **CD-1:** Exact commit message wording for the wallet-repair commit. Default: `fix(12): trust_witness_utxo for bdk_wallet 2.3 SignOptions (client/src/wallet.rs:260)` with safety rationale in commit body.
- **CD-2:** Whether to use `$(brew --prefix)` form in commit bodies (default: yes, per CONTRIBUTING.md §"Running integration tests").
- **CD-3:** N/A (no unit test to collapse).
- **CD-4:** Capture `bitcoind --version | head -1` output in each commit body alongside cargo verdict line.

### Deferred Ideas (OUT OF SCOPE)

- Option B — populate `non_witness_utxo` from RPC. Deferred to mainnet design.
- Wallet-level unit test for `sign_psbt_input`.
- Integration test file `tests/integration/wallet_signing.rs`.
- Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64`.
- `-txindex=1` in `bootstrap_regtest_bitcoind`.
- v1.3 ship notes and `/gsd-complete-milestone v1.3`.
- In-source `// TODO(mainnet):` markers.
- Grep sweep for other `SignOptions::default()` callsites (low-value in Phase 12; only one known site).

</user_constraints>

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| PSBT construction | Client (wallet) | Coordinator (assembles multi-input PSBT) | Client owns its own input signing; coordinator assembles the joint transaction |
| `witness_utxo` population | Client (wallet) | — | `sign_psbt_input` sets it from `self.utxo_value_sats` before calling `sign()` |
| `non_witness_utxo` population | NOT DONE (Option B, deferred) | — | Would require plumbing an RPC handle into `BdkClientWallet` — out of scope |
| `SignOptions` selection | Client (wallet) | — | Option A changes only the `sign()` call site |
| Integration test unmuting | Test harness (`full_round.rs`) | — | Plan 12-02's six commits each remove one `#[ignore]` line |

---

## bdk_wallet 2.3 SignOptions Semantics

**Verified from bdk_wallet 2.3.0 source** (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bdk_wallet-2.3.0/src/wallet/signer.rs` and `mod.rs`).

### SignOptions struct (verified source)

```rust
// signer.rs:750-828
pub struct SignOptions {
    /// Whether the signer should trust the `witness_utxo`, if the `non_witness_utxo` hasn't been provided.
    /// Defaults to `false` to mitigate the "SegWit bug" (BIP-143 fee-spoof).
    pub trust_witness_utxo: bool,        // default: false

    /// Override the "current height" for timelock resolution.
    pub assume_height: Option<u32>,      // default: None

    /// Whether to sign with any sighash type (not just SIGHASH_ALL).
    pub allow_all_sighashes: bool,       // default: false

    /// Whether to try finalizing the PSBT after signing.
    pub try_finalize: bool,              // default: true

    /// Which taproot script-spend leaves to sign for.
    pub tap_leaves_options: TapLeavesOptions,   // default: All

    /// Whether to sign with the taproot internal key.
    pub sign_with_tap_internal_key: bool,       // default: true

    /// Whether to grind ECDSA signature for low-r.
    pub allow_grinding: bool,            // default: true
}
```

[VERIFIED: bdk_wallet-2.3.0/src/wallet/signer.rs:750-828 in Cargo registry]

### Enforcement logic (verified source)

```rust
// wallet/mod.rs:1882-1893
// If we aren't allowed to use `witness_utxo`, ensure that every input (except p2tr
// and finalized ones) has the `non_witness_utxo`
if !sign_options.trust_witness_utxo
    && psbt
        .inputs
        .iter()
        .filter(|i| i.final_script_witness.is_none() && i.final_script_sig.is_none())
        .filter(|i| i.tap_internal_key.is_none() && i.tap_merkle_root.is_none())
        .any(|i| i.non_witness_utxo.is_none())
{
    return Err(SignerError::MissingNonWitnessUtxo);
}
```

[VERIFIED: bdk_wallet-2.3.0/src/wallet/mod.rs:1884-1893 in Cargo registry]

**Key insight:** The guard fires when ALL of: (1) `trust_witness_utxo` is false, AND (2) at least one non-taproot, non-finalized input lacks `non_witness_utxo`. P2WPKH inputs are non-taproot (no `tap_internal_key`/`tap_merkle_root`) and non-finalized (no `final_script_witness`/`final_script_sig`), so the guard fires for our inputs.

**Error string:** `SignerError::MissingNonWitnessUtxo` displays as `"Missing non-witness UTXO"` — exactly matching the panic message in `11-02-SUMMARY.md`. [VERIFIED: signer.rs:183]

### Was this default flipped between bdk_wallet 2.2 and 2.3?

The `trust_witness_utxo: false` default appears in bdk_wallet 2.3.0. The CLAUDE.md stack guidance recommended 2.2.x; the Cargo.lock shows 2.3.0 is pinned. This confirms the blocker is real and not a phantom version mismatch. [VERIFIED: Cargo.lock bdk_wallet version = "2.3.0"; default from signer.rs source]

The bdk_wallet 2.3.0 doc comment links to the Trezor blog post on the "SegWit bug" as the upstream rationale for this default: `<https://blog.trezor.io/details-of-firmware-updates-for-trezor-one-version-1-9-1-and-trezor-model-t-version-2-3-1-1eba8f60f2dd>` [CITED: bdk_wallet-2.3.0/src/wallet/signer.rs:762]

### Other SignOptions flags — interaction risk

- `try_finalize: true` (default) — harmless for our use. We extract from `partial_sigs` or `final_script_witness` either way (wallet.rs:266-275).
- `allow_all_sighashes: false` (default) — we use `SIGHASH_ALL`, so no conflict.
- `assume_height: None` (default) — no timelocks in P2WPKH scripts. No conflict.
- `allow_grinding: true` (default) — low-r grinding. No issue; output sig is DER-encoded regardless.
- `tap_leaves_options: All`, `sign_with_tap_internal_key: true` — ignored for P2WPKH inputs.

**No other SignOptions flags need changing.** Only `trust_witness_utxo` is relevant.

---

## BIP-143 Trade-off Analysis

### What attack does `trust_witness_utxo: false` defend against?

BIP-143 segwit signing commits to `hashPrevouts` + the per-input `amount` from `witness_utxo.value`. A malicious PSBT creator can set `witness_utxo.value` higher than the actual UTXO value. If a signing wallet trusts that value, it signs a transaction that pays more fee than the user authorized (the excess between the stated value and the real on-chain value becomes miner fee). Hardware wallets (Trezor, Ledger) were vulnerable to this before the 2021-era firmware fixes.

The `non_witness_utxo` mitigation: the full previous transaction is included in the PSBT. The signer can independently compute the UTXO value from the raw transaction's output, and verify it matches `witness_utxo.value`. This prevents a crafted `witness_utxo` from inflating the apparent value.

### Does this threat apply to our code path?

**No, for four reasons:**

1. **The client constructs `witness_utxo` itself.** `sign_psbt_input` (wallet.rs:252-256) sets:
   ```rust
   psbt.inputs[input_idx].witness_utxo = Some(TxOut {
       value: Amount::from_sat(self.utxo_value_sats),
       script_pubkey: self.utxo_script_pubkey.clone(),
   });
   ```
   `self.utxo_value_sats` is a field of `BdkClientWallet`, set at construction time (from regtest RPC, via `ClientWallet::from_wif`). There is no PSBT counterparty involved at the point `witness_utxo` is set.

2. **The client is the sole signer over its own UTXO.** In the CoinJoin protocol, each client signs only its own input. The coordinator assembles the PSBT and distributes it to clients, but each client only calls `sign_psbt_input` for its own input and uses its own locally-known `utxo_value_sats`. No client signs another client's input; the coordinator never signs.

3. **The regtest RPC is the trusted ground truth.** `utxo_value_sats` originates from `fund_regtest` → `list_unspent` RPC on the local bitcoind node the test controls. The client already trusts this RPC for its UTXO existence; trusting the value from the same source is consistent.

4. **Hardware wallet attack vector is absent.** The BIP-143 fee-spoof attack exploits the gap between what hardware wallets display (signed PSBT metadata) and what the signer actually commits to. In blindjoin's regtest client, the signer and the PSBT populator are the same software process. No hardware wallet boundary exists.

**Conclusion:** `trust_witness_utxo: true` is safe in the current code path. The attack the default guards against requires an untrusted PSBT creator, which does not exist in this signing context.

### Mainnet caveat (deferred)

If in a future mainnet scenario the coordinator could supply a crafted PSBT with a falsified `witness_utxo.value` for a client's own input, the attack would become relevant. However, even then: the client constructs its own `witness_utxo` in `sign_psbt_input` before calling `sign()`, overwriting any coordinator-supplied value. The only attack surface would be if the client's `utxo_value_sats` were wrong, which traces to a trusted RPC call the client makes for its own UTXO. Option B becomes the principled solution only when the client receives PSBTs where `witness_utxo` was PSBT-creator-supplied rather than self-constructed. That architectural change does not happen in Phase 12.

---

## Existing Code: client/src/wallet.rs

**Line 260 (verified from file read):**

```rust
// client/src/wallet.rs:258-261
// Sign via bdk_wallet
#[allow(deprecated)]
self.inner.sign(psbt, SignOptions::default())
    .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;
```

[VERIFIED: client/src/wallet.rs:259-261]

**Full `sign_psbt_input` method (lines 243-278):**

The method:
1. Finds our input by `previous_output` == `self.utxo_outpoint` (line 249-250)
2. Sets `witness_utxo` from `self.utxo_value_sats` + `self.utxo_script_pubkey` (lines 252-256) — this is the trusted-origin population
3. Calls `self.inner.sign(psbt, SignOptions::default())` — **THIS IS LINE 260, THE FIX LOCUS**
4. Extracts partial signature from `partial_sigs` or `final_script_witness` (lines 265-275)

**`BdkClientWallet` struct (lines 17-27):**

```rust
pub struct BdkClientWallet {
    pub network: Network,
    pub utxo_outpoint: OutPoint,
    pub utxo_value_sats: u64,      // <-- trusted-origin field; set at construction from RPC
    utxo_script_pubkey: ScriptBuf,
    wif_key: Option<String>,
    inner: Wallet,
}
```

[VERIFIED: client/src/wallet.rs:17-27]

**Import (lines 5-6):**

```rust
#[allow(deprecated)]
use bdk_wallet::signer::SignOptions;
```

The `#[allow(deprecated)]` attribute is already present — it was added in Phase 10's WIF-D fix and is orthogonal. No new import is needed. [VERIFIED: client/src/wallet.rs:5-6]

---

## Option A: trust_witness_utxo = true

**Mechanics:** Replace `SignOptions::default()` with `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }` on line 260.

**Complete diff (Plan 12-01 scope):**

```rust
// BEFORE (line 260):
self.inner.sign(psbt, SignOptions::default())

// AFTER (with D-08 block comment above, replacing lines 258-261):
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

**Risk assessment:**
- Zero regression risk for the current test harness (regtest, client-controlled UTXOs)
- Taproot inputs are EXEMPT from the `trust_witness_utxo` gate entirely (the enforcement code filters out `i.tap_internal_key.is_some()` inputs). So even if we add P2TR support later, the flag only matters for P2WPKH/P2SH-P2WPKH inputs.
- Future bdk_wallet version bumps that change `trust_witness_utxo` semantics again will be caught immediately by the `full_round.rs` CI gate (D-07 reasoning).

**Mainnet risk:** LOW — the architecture change that would make this unsafe (accepting untrusted `witness_utxo.value` from a counterparty) does not exist today, and if it were ever introduced, a code review of the PSBT population path would be mandatory. The D-08 comment names this precondition explicitly.

---

## Option B: populate non_witness_utxo (Rejected for Phase 12)

**Mechanics:** In `sign_psbt_input`, after setting `witness_utxo`, also fetch the full previous transaction from bitcoind and set `psbt.inputs[input_idx].non_witness_utxo = Some(prev_tx)`.

**Why rejected (D-01):**
- `BdkClientWallet` has no RPC handle. Adding one requires a new field, updated constructors (`from_wif`, `from_descriptor`, `generate`), and all callers of those constructors — approximately 30-50 LOC of new wiring.
- Crosses the wallet/RPC boundary deliberately kept separate in the current design.
- `get_raw_transaction` returns the raw transaction hex (or verbose JSON). The hex form needs `bitcoin::Transaction::consensus_decode` to produce the `Transaction` for `non_witness_utxo`. Additional parsing/error path.
- In regtest, the UTXO is always confirmed (the test fixture mines a confirmation block before running clients), so the RPC call would always succeed. But this "always succeeds in test" property is brittle — a test that registers an input before the confirmation block would fail at the Option B RPC step, not at the signing step.
- The principled deferred fix lives in mainnet design, not Phase 12 repair scope.

**Commit body reference requirement (D-04 / CD-1):** Plan 12-02's unmute commit bodies must reference both `cc20f6f` (Phase 11 RSA fix SHA) AND Plan 12-01's SHA (the wallet fix). This is forward-declared here so the planner encodes it in Plan 12-02's task spec.

---

## Plan 11-02 Compatibility

### Do the six unmute commits remain executable verbatim?

**Yes.** Plan 11-02 halted before making any commit to `tests/integration/full_round.rs`. All six `#[ignore = "TODO(Phase-10): ..."]` lines are intact at their original positions (164, 462, 730, 854, 911, 1236). The working tree is clean. [VERIFIED: 11-02-SUMMARY.md §"Working-Tree State"]

Plan 12-02 re-executes the per-test PASS-proof cycle unchanged:

1. Canonical-first: `full_round_three_clients` (line 164)
2. Then file order: lines 462, 730, 854, 911, 1236

The only delta from 11-02-PLAN.md is that each commit body now cites **two** SHAs: `cc20f6f` (Phase 11 RSA fix) AND the Plan 12-01 wallet-fix SHA (computed at runtime after Plan 12-01 commits).

### Does Option A's change affect any test fixtures?

**No.** `sign_psbt_input` is called the same way; the signing succeeds where it previously errored. The signature output (DER-encoded ECDSA for P2WPKH) is identical in byte content whether `trust_witness_utxo` is true or false — the flag only gates whether signing is attempted, not how the signature is computed. No test fixture depends on the specific bytes of a `SignOptions` field.

### Bisectability

Option A preserves bisect cleanliness:

- `git bisect` on the `full_round_three_clients` failure traces to Plan 12-01's single commit (the wallet fix).
- Each of Plan 12-02's six commits is independently bisectable (one `#[ignore]` removal per commit, passing before commit due to Plan 12-01 in history).
- No drive-by edits in any unmute commit.

---

## Test Surface

### Primary gate: `full_round_three_clients`

**File:** `tests/integration/full_round.rs:165` (with `#[ignore]` at line 164)

**What it exercises:**
- Fund 3 P2WPKH UTXOs via regtest bitcoind (lines 178-179)
- Spawn coordinator in-process (lines 186-191)
- Three concurrent client tasks: input registration, BIP-322 PSBT proof, output registration, signing (lines 201-250)
- Coordinator broadcasts CoinJoin tx; test asserts mempool presence + 3 outputs of 100,000 sats

The signing step at `tests/integration/full_round.rs:248` calls `round::sign::verify_and_sign(...)`, which calls `wallet.sign_psbt_input(psbt)` — directly exercising the fix locus.

**Invocation (PASS-proof capture):**
```bash
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration full_round::full_round_three_clients -- --ignored
```

### Remaining five tests (Plan 12-02 steps 2-6)

Lines 462, 730, 854, 911, 1236 — test names to be confirmed from `full_round.rs` at execution time. All exercise the same signing code path (all call `verify_and_sign` or equivalent). If `full_round_three_clients` passes after the wallet fix, all five should pass for the same reason (same fix locus, same signing path).

### No new unit tests (D-06)

The `full_round.rs` integration suite is the end-to-end coverage. A wallet-level unit test would need PSBT fixture construction and would duplicate integration coverage. D-06 forbids it.

---

## Common Pitfalls

### Pitfall 1: Taproot exemption misread

**What goes wrong:** Assuming `trust_witness_utxo: true` is needed for P2TR inputs.
**Why it happens:** Reading the enforcement code without noticing the `i.tap_internal_key.is_none() && i.tap_merkle_root.is_none()` filter — taproot inputs are already exempt.
**How to avoid:** The blindjoin client uses P2WPKH (from WIF/BDK descriptor). P2TR support is not in Phase 12 scope.

### Pitfall 2: Forgetting the `#[allow(deprecated)]` is already present

**What goes wrong:** Adding a second `#[allow(deprecated)]` annotation when changing the `SignOptions` construction form.
**How to avoid:** The `#[allow(deprecated)]` on line 5 covers the entire module's use of `SignOptions`. Line 259's `#[allow(deprecated)]` covers the specific call. Both are already present; neither needs changing.

### Pitfall 3: Citing the wrong Phase 11 SHA in commit bodies

**What goes wrong:** Plan 12-02 commit bodies cite `cc20f6f` as the RSA fix SHA but forget to add Plan 12-01's SHA.
**How to avoid:** Plan 12-02's task spec must explicitly require both SHAs: `cc20f6f` (Phase 11) AND `$(git log --grep="fix(12):" --format=%H -1)` (Phase 12 wallet fix).

### Pitfall 4: Scope creep into Option B

**What goes wrong:** While writing the D-08 comment, executor reaches for "the right thing" and starts wiring up an RPC handle.
**How to avoid:** D-01 is a locked decision. The comment explicitly names Option B as a deferred future path. The code change is exactly one line.

### Pitfall 5: Premature REPAIR-01 closure

**What goes wrong:** Marking REPAIR-01 `[x]` after `full_round_three_clients` passes, before all six unmutes land.
**How to avoid:** D-13 specifies REPAIR-01 closes when ALL 8 `full_round::*` tests are green (the 2 that were not previously unmuted + 6 from the unmute cycle). Partial green does not close REPAIR-01.

---

## Recommendation

**Option A is the correct choice for Phase 12.**

Evidence from the bdk_wallet 2.3.0 source (verified from Cargo registry):

1. The enforcement is at `wallet/mod.rs:1884-1893`: when `trust_witness_utxo` is false and any non-taproot, non-finalized input lacks `non_witness_utxo`, signing returns `Err(SignerError::MissingNonWitnessUtxo)` — exactly the observed error.

2. The threat model for `trust_witness_utxo: false` requires an untrusted PSBT creator to have set `witness_utxo.value`. In `sign_psbt_input`, the client sets `witness_utxo` itself, from `self.utxo_value_sats`, which originates from the regtest RPC at wallet construction time. The attack vector is architecturally absent.

3. The fix is one line. The required block comment (D-08) ensures a future reader has the full threat-model analysis without needing to re-derive it.

4. Plan 11-02's six unmute commits remain executable verbatim after the wallet fix lands. No fixture changes. No ordering changes. The only addition to each unmute commit body is a second SHA reference (Plan 12-01's commit).

5. Option B would require 30-50 LOC of new RPC wiring across the wallet struct, three constructors, and their callers — disproportionate to the risk level for regtest-only operation, and wrong architectural location for the change.

**The fix is: one line of code, ten lines of comment, six unmute commits.**

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `trust_witness_utxo` was `false` by default in bdk_wallet 2.2.x as well (the behavior existed, but 2.3 may have tightened something else) | bdk_wallet 2.3 SignOptions Semantics | Low — the fix works regardless of when the default was introduced; the error is observed in the 2.3.0 source |
| A2 | All six `full_round.rs` tests exercise the same `sign_psbt_input` code path | Test Surface | Medium — if any test uses a different signing path, it might fail for a different reason after Plan 12-01 lands |

---

## Open Questions

1. **Are all six `full_round.rs` tests at lines 462/730/854/911/1236 exercising `verify_and_sign`?**
   - What we know: `full_round_three_clients` panics at `full_round.rs:248` in `verify_and_sign`. The 11-02-SUMMARY confirms the error is wallet-side (signing phase, not input_reg).
   - What's unclear: Whether any of the other five tests use a different client path that bypasses `sign_psbt_input`. Skimming the other test bodies at execution time would confirm.
   - Recommendation: Execute Plan 12-02's canonical-first step first; if all five remaining tests follow the same client task pattern as `full_round_three_clients`, they will all benefit from the same fix.

2. **Does bdk_wallet 2.3.0 introduce any other breaking change vs 2.2.x relevant to this code path?**
   - What we know: The `trust_witness_utxo: false` default and the `Wallet::create_single` API (Phase 10's WIF-D fix) are the two confirmed 2.3 changes affecting this project.
   - What's unclear: Whether any bdk_wallet 2.3 changelog entries exist for signing-path changes beyond `trust_witness_utxo`.
   - Recommendation: If `full_round_three_clients` passes after Option A, this question is answered empirically. No separate investigation needed before planning.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| brew bitcoind v31 | Integration tests | Assumed ✓ | v31.0.0 (pinned) | None — tests skip if missing |
| bdk_wallet 2.3.0 | Client signing | ✓ | 2.3.0 (from Cargo.lock) | N/A |
| cargo test --test integration | Plan 12-02 PASS-proof | ✓ | (workspace test target) | N/A |

**Missing dependencies with no fallback:** none — the `require_bitcoind!` macro gracefully skips tests if bitcoind is absent in a local dev environment; CI enforces it via `BLINDJOIN_REQUIRE_BITCOIND=1`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) + tokio::test |
| Config file | none (workspace-level Cargo.toml) |
| Quick run command | `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round::full_round_three_clients -- --ignored` |
| Full suite command | `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round -- --ignored` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REPAIR-01 (Plan 12-01) | wallet.sign_psbt_input succeeds with witness_utxo only | integration | `cargo test --test integration full_round::full_round_three_clients -- --ignored` | Yes (currently ignored) |
| REPAIR-01 (Plan 12-02) | All 8 full_round tests green | integration | `cargo test --test integration full_round -- --ignored` | Yes (6 currently ignored) |

### Wave 0 Gaps

None — no new test files created in Phase 12. The existing `tests/integration/full_round.rs` is the coverage vehicle; Plan 12-01's fix makes it pass; Plan 12-02's unmutes expose it.

---

## Sources

### Primary (HIGH confidence — verified from authoritative sources)

- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bdk_wallet-2.3.0/src/wallet/signer.rs:750-828` — `SignOptions` struct definition and `Default` impl; `trust_witness_utxo: false` confirmed
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bdk_wallet-2.3.0/src/wallet/mod.rs:1882-1893` — enforcement logic; exact condition that triggers `MissingNonWitnessUtxo` error
- `Cargo.lock` (blindjoin repo) — `bdk_wallet` version pinned at `2.3.0` (checksum `b03f1e31ccc562f600981f747d2262b84428cbff52c9c9cdf14d15fb15bd2286`)
- `client/src/wallet.rs:243-278` — `sign_psbt_input` method body; `witness_utxo` population at lines 252-256; fix locus at line 260
- `client/src/wallet.rs:17-27` — `BdkClientWallet` struct; `utxo_value_sats: u64` trusted-origin field
- `client/src/wallet.rs:5-6` — `#[allow(deprecated)]` + `use bdk_wallet::signer::SignOptions` import
- `.planning/phases/11-.../11-02-SUMMARY.md` — verbatim panic trace, working-tree state verification, resume protocol
- `.planning/phases/12-.../12-CONTEXT.md` — all locked decisions (D-01 through D-14) and discretion items (CD-1 through CD-4)

### Secondary (MEDIUM confidence — cited from official sources)

- bdk_wallet 2.3.0 doc comment citing the Trezor firmware blog post (2021) as the BIP-143 fee-spoof background — [CITED: signer.rs:762]

---

## Metadata

**Confidence breakdown:**
- Fix locus and error cause: HIGH — directly verified from bdk_wallet 2.3.0 source and Cargo.lock
- Option A safety: HIGH — `witness_utxo` population is self-contained in client code, no untrusted counterparty path
- Plan 11-02 compatibility: HIGH — working tree confirmed clean in 11-02-SUMMARY; no unmute commits made
- Mainnet implications: MEDIUM — deferred architectural question, not a Phase 12 concern

**Research date:** 2026-05-28
**Valid until:** 2026-06-28 (bdk_wallet version is pinned; validity tracks Cargo.lock, not time)

---

## RESEARCH COMPLETE

**Phase:** 12 — Repair client/src/wallet.rs:260 (bdk_wallet 2.3 SignOptions) + Plan 11-02 unmute cycle
**Confidence:** HIGH

### Key Findings

- The bdk_wallet 2.3.0 source confirms `trust_witness_utxo: false` as the default (signer.rs:820), and the enforcement at mod.rs:1884-1893 returns `MissingNonWitnessUtxo` for any non-taproot, non-finalized input lacking `non_witness_utxo` when that flag is false. This is the exact error observed in Plan 11-02.
- `client/src/wallet.rs` line 260 is the sole fix locus. The `witness_utxo` is populated from `self.utxo_value_sats` — a trusted, client-owned field set from the regtest RPC at wallet construction. No untrusted counterparty supplies this value. Option A (`trust_witness_utxo: true`) is safe.
- Option B requires ~30-50 LOC of new RPC wiring through `BdkClientWallet` — architecturally disproportionate for a regtest-only fix; correctly deferred to mainnet design per D-01.
- Plan 11-02's six unmute commits are executable verbatim after Plan 12-01 lands. No working-tree changes were made in Plan 11-02 (all six `#[ignore]` lines remain at lines 164/462/730/854/911/1236). The only addition to commit bodies is a second SHA reference (Plan 12-01's hash alongside `cc20f6f`).
- All other `SignOptions` fields (`try_finalize`, `allow_all_sighashes`, `assume_height`, `allow_grinding`) are benign at their defaults for P2WPKH signing.

**Recommendation summary:** Apply the single-line Option A fix with the D-08 block comment, then execute the six unmute commits from Plan 11-02 verbatim. The research confirms Option A is safe, the fix is the minimum correct repair, and no test fixture changes are needed. Plan 12-01 = one commit (≤15 LOC total); Plan 12-02 = six commits (one `#[ignore]` removal each). REPAIR-01 closes when all 8 `full_round::*` tests are green locally.

### File Created
`.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-RESEARCH.md`

### Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| bdk_wallet 2.3 SignOptions semantics | HIGH | Verified from pinned bdk_wallet 2.3.0 source in Cargo registry |
| Option A safety | HIGH | Verified that witness_utxo.value originates client-side from trusted RPC, not from PSBT counterparty |
| Plan 11-02 compatibility | HIGH | 11-02-SUMMARY confirms clean working tree; all 6 ignore lines intact at original line numbers |
| Mainnet risk | MEDIUM | Architectural argument is sound, but deferred; documented in D-08 comment and CONTEXT.md Deferred Ideas |

### Open Questions

- Whether all 5 remaining `full_round.rs` tests (lines 462/730/854/911/1236) use the same `verify_and_sign` path — answerable empirically at Plan 12-02 execution time.

### Ready for Planning

Research complete. Planner can now create PLAN.md files: Plan 12-01 (single wallet-fix commit) and Plan 12-02 (six unmute commits reusing Plan 11-02's spec verbatim).
