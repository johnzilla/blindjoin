# Phase 20: Mixed-Round Fee Accuracy - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-31
**Phase:** 20-mixed-round-fee-accuracy
**Areas discussed:** Pre-registration fee_share estimate (fee.rs); Where ScriptType lives on the path; vbyte source (hardcoded vs derived); Baseline capture for v1.4-parity test

---

## Pre-registration fee_share estimate (fee.rs)

| Option | Description | Selected |
|--------|-------------|----------|
| Worst-case across BipConfig allowed set | `estimate_fee_share` takes `&BipConfig`, uses `max(script_input_vbytes)` across enabled types + `script_output_vbytes(output_script_type)`. Guaranteed not to underpay; over-charges P2WPKH in mixed-allowed rounds (acceptable — build-time fee_share is the load-bearing number). | ✓ |
| Use `bip.output_script_type` as the proxy | Assume worst-case-equals-output type. Wrong for mixed rounds; breaks the invariant. | |
| Delete fee.rs; single estimator in tx.rs | Merge into tx.rs. Eliminates dual-magic-number risk; higher churn. | |
| Keep fee.rs hardcoded P2WPKH-max | Leave fee.rs unchanged. Unsafe — build-time fee_share could exceed validate_utxo's pre-check; InsufficientFunds at PSBT build, round aborts. | |

**User's choice:** Worst-case across BipConfig allowed set (recommended)
**Notes:** Recorded as D-122 in CONTEXT.md. Two call sites updated (handlers.rs:165, handlers.rs:505). New helper `BipConfig::allowed_set()` added per D-122a + CD-43.

---

## Where ScriptType lives on the path (state struct vs re-derive)

| Option | Description | Selected |
|--------|-------------|----------|
| Full plumbing: UtxoDetails → RegisteredInput → ParticipantInput | Single source of truth at validate_utxo time; matches REQUIREMENTS FEE-02 verbatim; never re-runs detect_script_type. ~4 file touches. | ✓ |
| Re-derive in build_coinjoin_psbt | No new field; call detect_script_type inside the build loop. Cost: 1 hash per input per build; audit prose has to explain why we compute it twice. | |
| ParticipantInput stores ScriptType, no field on RegisteredInput | Skip RegisteredInput; re-derive at handler/signing call sites. Same hash cost as option B; one less struct field but duplicates the derive across two callers. | |

**User's choice:** Full plumbing (recommended)
**Notes:** Recorded as D-123 in CONTEXT.md. RegisteredInput.script_type carries `#[zeroize(skip)]` annotation per D-123a (mirrors the existing `script_pubkey` field — public chain data, no privacy concern). UtxoDetails is extended not replaced per D-123b.

---

## vbyte source: hardcoded constants vs derived

| Option | Description | Selected |
|--------|-------------|----------|
| Hand-derive with BIP-141 math worked inline + ceil-rounding | const fn with each arm carrying 4-6 line BIP-141 derivation comment; integer-arithmetic ceil `(witness + 3) / 4`. P2TR resolves to 58 (NOT roadmap's 57 — ceil of 57.5). | ✓ |
| Pin roadmap numbers verbatim with citation comments | const fn returning literal roadmap numbers (68/31, 57/43, 91/32). Contradicts STATE.md's "round UP" rule for P2TR. | |
| Compute at startup via bitcoin::Weight + assert against pinned consts | Runtime sanity check at boot. Overkill for v1.5; high audit value but high boot complexity. | |

**User's choice:** Hand-derive with BIP-141 math worked inline (recommended)
**Notes:** Recorded as D-124 in CONTEXT.md with full vbyte derivation table (D-124a for inputs, D-124b for outputs). Plan-phase research task per CD-42 to verify P2TR keypath worst-case against rust-bitcoin's `predict_weight` and confirm the 58 (round UP) divergence from roadmap's 57. Six unit tests pin the table values per D-124c.

---

## Baseline capture for v1.4-parity test

| Option | Description | Selected |
|--------|-------------|----------|
| Hardcode the numeric baseline + derivation comment | `assert_eq!(fee_share, 266)` with comment block showing v1.4 formula: `(10 + 3*68 + 3*2*31)*2/3 = 266`. Static, self-contained, durable to refactors. | ✓ |
| Embed the v1.4 formula inline in the test | `fn v14_formula(n, rate) -> u64 { (10 + n*68 + n*2*31)*rate/n }` helper at top of test. Self-documenting but creates load-bearing test code that future cleanups might delete. | |
| Snapshot via fixture file | `.planning/fixtures/v14-fee-baseline.txt`. Overkill for a single u64 number. | |

**User's choice:** Hardcode the numeric baseline + derivation comment (recommended)
**Notes:** Recorded as D-125 in CONTEXT.md. Math verified: 10+3·68+3·2·31=400 vsize → 400·2=800 fee → 800/3=266 fee_share (with 2 sat remainder absorbed). Mixed-script regression test D-126 derives to 275, giving 9 sat/participant divergence at fee_rate=2 — comfortable headroom above the ≥1 sat ROADMAP SC#4 requirement.

---

## Claude's Discretion

Recorded in CONTEXT.md §"Claude's Discretion" — five items deferred to plan-phase:

- **CD-40:** Location of `script_input_vbytes` / `script_output_vbytes` — default fee.rs (next to `estimate_fee_share`), fall back to tx.rs if import dependency becomes circular.
- **CD-41:** `const fn` vs plain `pub fn` — default `const fn` (cheaper at all call sites in optimised binary).
- **CD-42:** P2TR vbyte resolution — plan-phase verifies against rust-bitcoin's `predict_weight`; defaults to **58** (ceil of 57.5) per STATE.md's "round UP" rule even though it diverges from roadmap's 57.
- **CD-43:** `BipConfig::allowed_set` method name + return shape — default `pub fn allowed_set(&self) -> impl Iterator<Item = ScriptType>`.
- **CD-44:** Test fixture amount for `fee_share_p2wpkh_only_matches_v14_baseline` — default reuse existing `make_inputs(3, 1_100_000)`.
- **CD-45:** `mixed_script_e2e.rs` amount-assertion refresh — plan-phase greps for hardcoded sats values; refresh per new fee math if any exist.

## Deferred Ideas

Recorded in CONTEXT.md §"Deferred Ideas" — six items noted for future phases:

- Validating `change_address` script_type matches `bip.output_script_type` (v1.6+ if audit-charter review flags).
- Per-input variable `fee_share` (REQUIREMENTS.md `Future requirements`; changes wire protocol, separate milestone).
- Mixed output script types per participant (Wasabi 2.0.3-style; REQUIREMENTS.md out of v1.5 scope).
- B-03 dynamic fee estimation (carry-forward to v1.6+; pre-mainnet requirement).
- Compute vbytes at coordinator startup via `bitcoin::Weight` + assert against pinned consts (overkill for v1.5; v1.6+ if audit review wants stronger machine-verification).
- Promote `script_input_vbytes` / `script_output_vbytes` to `shared/` crate (v1.6+ if client-side fee preview becomes a feature).
- `BipConfig::allowed_set` as a cached `Vec<ScriptType>` field rather than a method (v1.6+ if usage patterns warrant).
