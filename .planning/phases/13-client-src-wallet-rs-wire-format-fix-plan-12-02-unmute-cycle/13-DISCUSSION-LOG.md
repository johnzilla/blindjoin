# Phase 13: client/src/wallet.rs wire-format fix + Plan 12-02 unmute cycle re-execution - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-28
**Phase:** 13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle
**Areas discussed:** Repair side, Sanity gate, Comment scope, Doc drift

---

## Repair side

| Option | Description | Selected |
|--------|-------------|----------|
| Client encodes as Witness (Option A, recommended) | Change client/src/wallet.rs:279-281 to build a 2-item Witness (sig + pubkey) and consensus::serialize it. Client-only, ~4 LOC. Matches what coordinator already deserializes. SUMMARY-recommended. | ✓ |
| Coordinator parses raw DER + reconstructs Witness | Change coordinator/src/round/signing.rs to deserialize raw DER + fetch the participant's pubkey (requires storing pubkey at input registration). Heavier; crosses the coordinator/client wire contract. | |
| Both — client fix + coordinator hardening | Apply Option A AND add a more diagnostic error at signing.rs:165 (include sig_bytes length + first byte) so any future format drift surfaces in 30s instead of via grep-archaeology. | |

**User's choice:** Client encodes as Witness (Option A)
**Notes:** Client is unambiguously the side producing wrong bytes. Coordinator hardening captured as a deferred standalone item — out of scope for Phase 13's bisect-clean discipline. Mapped to D-01.

---

## Sanity gate

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — sanity capture inside Plan 13-01 (recommended) | Plan 13-01 commits the fix + captures PASS verdict of full_round_three_clients in the commit body. If this fails, Plan 13-02 is never opened — Phase 14 absorbs the 6th blocker. Mirrors Plan 12-01's bisect-clean discipline. | ✓ |
| No — Plan 13-02 is the gate via canonical-first like Plan 12-02 | Plan 13-01 lands the source fix only (no test capture). Plan 13-02's first commit is canonical-first — if it fails, D-11 halts. Avoids duplicating the canonical-first run across both plans. | |

**User's choice:** Yes — sanity capture inside Plan 13-01
**Notes:** Mapped to D-06 (two-plan structure) and D-07 (sanity capture lives inside Plan 13-01's commit body). Plan 13-02's first commit re-runs the same test as the unmute-commit bisect-proof; the two captures are complementary, not duplicate.

---

## Comment scope

| Option | Description | Selected |
|--------|-------------|----------|
| Multi-line block comment with cross-ref (Phase 12 D-10 analog) | 5-10 line comment explaining: (1) what wire format the coordinator expects + why (consensus::deserialize<Witness> at signing.rs:160); (2) why this encoding is canonical for P2WPKH (sig + pubkey witness stack); (3) cite 12-02-SUMMARY.md §"Root Cause". Maintains the safety-contract pattern. | |
| One-line comment + SUMMARY cross-ref | Single comment line: '// Coordinator deserializes as bitcoin::Witness — see 12-02-SUMMARY.md §"Root Cause".' The fix is self-explanatory; defer detail to the planning archive. | |
| No comment — commit body carries the rationale | The fix commit's body explains why; in-source code reads as ordinary Witness construction. Bisect-friendly via git log, code stays terse. | ✓ |

**User's choice:** No comment — commit body carries the rationale
**Notes:** Departure from Phase 12 D-08's multi-line block comment. Rationale captured in D-10: Phase 12 D-08 defended an inherently suspicious-looking flag (`trust_witness_utxo: true`); Phase 13's `bitcoin::Witness` construction reads as ordinary code and commenting it would explain WHAT, not WHY (anti-pattern per `auto memory` guidance). The commit body's failure-signature + cross-ref carries the durable record.

---

## Doc drift

| Option | Description | Selected |
|--------|-------------|----------|
| Phase 13 D-13 corrects REQUIREMENTS.md + closes REPAIR-01 on green | Phase 13 D-13 (bookkeeping commit) revisits REQUIREMENTS.md line 20: if all 8 full_round tests green locally, REPAIR-01 stays `[x]` with correct provenance (Phase 13 commit SHA); if any test red, flip back to `[ ]` and surface the drift in the commit message. ROADMAP.md Phase 13 entry marked complete when REPAIR-01 confirmed green. | ✓ |
| Leave REQUIREMENTS.md as-is (don't touch the [x]) | Phase 13 only updates ROADMAP.md Phase 13 entry on green; REQUIREMENTS.md stays untouched. Risk: the drift compounds and future audits accept aspirational doc state as ground truth. | |

**User's choice:** Phase 13 D-14 reconciles the doc drift
**Notes:** Mapped to D-14 (renumbered from D-13 in the question after additional decisions slotted in). The reconciliation happens in Plan 13-02's final atomic doc commit, after all six PASS-proof captures land. If any test red → flip `[x]` to `[ ]` and surface in commit message.

---

## Claude's Discretion

- CD-1: Exact commit message wording for Plan 13-01 (default proposed; bisect cleanliness > length)
- CD-2: brew bitcoind invocation form (default `$(brew --prefix)` per CONTRIBUTING.md; same as Phase 12 CD-2)
- CD-3: Whether the obsolete `final_script_witness` fallback at lines 283-288 is removed in Plan 13-01 (default: defer for bisect cleanliness; planner may scope as separate atomic refactor commit)
- CD-4: `bitcoind --version | head -1` capture pattern (same as Phase 12 CD-4)
- CD-5: Plan 13-02 single-session vs cross-session execution (default: single-session for bisect quality)

## Deferred Ideas

- Option B coordinator-side repair (heavier; gated on a future client that can't encode Witness)
- Coordinator error-message hardening at signing.rs:165 (out of scope to keep Plan 13-01 bisect-clean; revisit as `/gsd-quick` or standalone phase)
- Removal of obsolete `final_script_witness` fallback at client/src/wallet.rs:283-288 (CD-3 discretion)
- Codify wire format in shared/src/protocol.rs doc comments (CD-3 discretion)
- Wallet-level unit/integration tests (rejected per D-08/D-09 — full_round is the canonical end-to-end coverage)
- Mainnet enablement + Option B (gated on mainnet-design work)
- Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64` (inherited from Phase 11/12)
- `-txindex=1` in `bootstrap_regtest_bitcoind` (inherited from Phase 11/12)
- v1.3 ship notes / `/gsd-complete-milestone` (deferred to wrap-up phase or `/gsd-ship` after CI green)
- In-source `// TODO(mainnet):` markers (anti-pattern per D-11)
