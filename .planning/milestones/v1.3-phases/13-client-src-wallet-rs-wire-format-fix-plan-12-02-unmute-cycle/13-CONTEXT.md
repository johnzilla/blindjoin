# Phase 13: client/src/wallet.rs wire-format fix + Plan 12-02 unmute cycle re-execution - Context

**Gathered:** 2026-05-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 13 absorbs the 5th orthogonal blocker surfaced by Plan 12-02's canonical-first gate (per Phase 12 D-11 escape-valve, Phase 12 D-12 next-phase absorption) and closes out REPAIR-01. Two coupled deliverables:

1. **Wire-format fix at `client/src/wallet.rs:276-291` (`sign_psbt_input` partial-sig extraction).** Replace the raw-DER return path (`Ok(sig.to_vec())`) with a properly-encoded `bitcoin::Witness` (2-item P2WPKH stack: [sig_bytes, pubkey_bytes]) consensus-serialized for transmission. The coordinator already calls `bitcoin::consensus::deserialize::<bitcoin::Witness>()` at `coordinator/src/round/signing.rs:160` and inserts the result into `psbt.inputs[i].final_script_witness` before `psbt.extract_tx()`. The client side has been incorrect since the wire protocol was first written; the bug was latent because earlier orthogonal blockers (Phase 11 RSA SPKI, Phase 12 bdk_wallet 2.3 SignOptions) prevented the test from ever reaching `/round/sign`.

2. **Re-execute Plan 12-02's six-unmute cycle verbatim** with **three-SHA commit bodies** (RSA fix `cc20f6f` + wallet-trust fix `0bbcf3c` + new wire-format fix from Plan 13-01). The locked spec from `11-02-PLAN.md` (canonical-first → file order, one bisectable commit per unmute, CD-1 PASS-proof body shape) carries forward unchanged; only the SHA-reference list grows by one.

**Net effect:** REPAIR-01 closes in Phase 13 when all 8 `full_round::*` tests are green locally against pinned brew bitcoind v31. REPAIR-02 closure remains tied to the combined Phase 11+12+13 PR being observed green in CI (per Phase 11 D-11).

**Not in scope:** mainnet enablement; Option B coordinator-side repair (parse raw DER + reconstruct Witness server-side); coordinator hardening (richer error at signing.rs:165); rewriting Plan 12-02's locked unmute spec (reused verbatim); a wallet-level unit test for Witness encoding; an integration test file `tests/integration/wallet_signing.rs`; rename of `rsa_pubkey_der_b64`; `-txindex=1` in `bootstrap_regtest_bitcoind`; v1.3 ship notes; retiring `full_round` tests under D-10; in-source `// TODO(mainnet)` markers.

</domain>

<decisions>
## Implementation Decisions

### Repair Approach

- **D-01:** **Option A — client encodes as `bitcoin::Witness`.** At `client/src/wallet.rs:279-281` (the `partial_sigs.iter().next()` extraction block), change destructuring from `(_pk, sig)` to `(pk, sig)` and replace `Ok(sig.to_vec())` with:
  ```rust
  let mut witness = bitcoin::Witness::new();
  witness.push(sig.to_vec());        // ECDSA sig: DER + SIGHASH_ALL
  witness.push(pk.to_bytes());       // compressed pubkey (33 bytes)
  Ok(bitcoin::consensus::serialize(&witness))
  ```
  Single-file, client-only, ~4 LOC. Matches what the coordinator already deserializes at `coordinator/src/round/signing.rs:160`. Option B (coordinator parses raw DER + reconstructs Witness from a pubkey it would have to store at input-registration time) is rejected: heavier, crosses the wire-protocol contract, requires `coordinator/src/round/input_reg.rs` to retain pubkey state, and the client is the side that is producing the wrong bytes.
- **D-02:** **Fix locus is `client/src/wallet.rs` lines 276-291 only.** No changes to the wallet struct, constructors, or other methods. No changes to `client/src/round/sign.rs`. No coordinator-side changes. The `final_script_witness` fallback at lines 283-288 is also obsolete-by-design once the fix lands (P2WPKH always populates `partial_sigs`, not `final_script_witness`) — leave it untouched, the planner will assess whether removal is in scope or deferred.
- **D-03:** **Coordinator-side error-message hardening is OUT of scope.** The "Invalid witness data for input {i}" at `coordinator/src/round/signing.rs:165` cost ~30 minutes of grep-archaeology to diagnose in Plan 12-02. A richer error (include `sig_bytes.len()` + first byte) would have made the diagnosis 30 seconds. But: Phase 13 is execution-scoped on the locked seed from 12-02-SUMMARY.md; coordinator hardening is its own concern and would dilute the bisect cleanliness of Plan 13-01. Deferred — see Deferred Ideas.

### Scope Coupling — Wire-Format Fix + Plan 12-02 Six Unmute Commits Re-Execution

- **D-04:** **Phase 13 owns REPAIR-01 closure.** The wire-format fix AND the six-unmute cycle both land in Phase 13. This matches the ROADMAP Phase 13 description verbatim. REPAIR-01 flips to `[x]` when all 8 `full_round::*` tests are green locally against pinned brew bitcoind v31 in Phase 13. REPAIR-02 closure stays tied to PR observation (Phase 11 D-11, Phase 12 D-13).
- **D-05:** **Reuse Plan 12-02's locked spec verbatim — only the SHA-reference list grows.** Phase 13 must NOT rewrite, re-order, or re-justify the six-unmute cycle. The locked artifacts from `11-02-PLAN.md` (re-spec'd by Plan 12-02) carry forward unchanged:
  - **Canonical-first order:** `full_round_three_clients` (line 164) → then file order: lines 462, 730, 854, 911, 1236
  - **Per-test commit cycle (×6):** one atomic commit per unmute; each commit removes only the single `#[ignore = …]` line for that one test
  - **PASS-proof commit body shape:** one-line `cargo test --test integration full_round::<name> -- --ignored` invocation + the cargo PASS verdict line + `bitcoind --version | head -1` output + SHA references to **all three** prerequisite fixes:
    - Phase 11 RSA fix: `cc20f6fbca4d292bf7b394a3850b18d244b5b602`
    - Phase 12 wallet-trust fix: `0bbcf3c76ca251c14aa64216ca6955be1f880b9a`
    - Phase 13 wire-format fix: Plan 13-01's commit SHA (substituted by planner at execution time)
  - **No drive-by edits** to other tests, helpers, or unrelated code in any unmute commit
- **D-06:** **Two-plan structure inside Phase 13.** Plan 13-01 = wire-format fix (one commit) + local canonical-first sanity capture in the commit body — must land first; Plan 13-02 = six-test unmute cycle (six commits, canonical-first then file order) — depends on Plan 13-01. The local sanity capture inside Plan 13-01 acts as the bisect gate: if `full_round_three_clients` does not PASS after the wire-format fix, Plan 13-02 is never opened — Phase 14 absorbs the 6th blocker per the inherited D-11/D-12 protocol.

### Sanity Gate — Plan 13-01 Includes Canonical-First Capture

- **D-07:** **Plan 13-01 captures local PASS of `full_round_three_clients` in its commit body BEFORE Plan 13-02 starts.** Mirrors Plan 12-01's bisect-clean discipline. Avoids the failure mode where Plan 13-02 opens with the canonical-first gate, fails on a 6th orthogonal blocker, and the wire-format fix commit lands without proof that it actually unblocked the canonical-first path. The capture is the same invocation pattern from CONTRIBUTING.md §"Running integration tests":
  ```
  BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
    cargo test --test integration full_round::full_round_three_clients -- --ignored
  ```
  Note: Plan 13-02's canonical-first commit (the FIRST of the six unmute commits, which actually removes the `#[ignore]` line) still runs the same test — that re-run is the bisect-proof for the unmute commit itself. Plan 13-01's capture proves the source fix works; Plan 13-02's first commit proves the unmute discipline holds.

### Regression Coverage

- **D-08:** **No new wallet-level unit test.** The unmuted `full_round.rs` suite is the end-to-end coverage and exercises the wire-format path with a real coordinator. A wallet-level `#[test] fn sign_psbt_input_returns_serialized_witness()` would need PSBT fixture setup and would duplicate the integration coverage. Mirrors Phase 11 D-04 and Phase 12 D-06.
- **D-09:** **No belt-and-braces integration test file.** No `tests/integration/wallet_signing.rs`. Same reasoning as Phase 11 D-04 / Phase 12 D-07.

### In-Source Rationale (Departure from Phase 12 D-08)

- **D-10:** **No in-source block comment at the wire-format fix locus.** Departure from Phase 12 D-08's multi-line safety contract. Rationale:
  - The wire-format encoding (build a `bitcoin::Witness` from sig + pubkey, consensus-serialize it) reads as ordinary `bitcoin::Witness` construction. There is no threat model to explain — this is the canonical P2WPKH witness shape.
  - Phase 12 D-08's comment defended `trust_witness_utxo: true` against a specific fee-spoofing attack — that warranted an in-source rationale because the code looks suspicious without it. The Phase 13 fix looks correct on its face; commenting it would explain WHAT, not WHY, and that's an anti-pattern (per `auto memory` guidance: comments explain WHY non-obvious, not WHAT).
  - The commit body carries the full rationale (link to 12-02-SUMMARY.md §"Root Cause", cite to coordinator/src/round/signing.rs:160 as the deserialization site). Bisect-friendly via `git log -p`, code stays terse.
- **D-11:** **No `// TODO(mainnet):` markers.** Same as Phase 12 D-09 — the planning archive (this CONTEXT.md + 12-02-SUMMARY.md) is the canonical revisit list. In-source TODO markers go stale.

### Escape Valve & Drift Discipline

- **D-12:** **D-11 escape-valve discipline applies to Plan 13-02 unmodified.** If during the six-unmute cycle a 6th orthogonal blocker appears in ≥1 test, the executor halts after the first encounter and emits a checkpoint with the failure mode and a proposed minimal repair. Pre-authorized in-flight scope expansion is **zero**. Phase 11 absorbed three orthogonal blockers, Phase 12 absorbed the 4th (wallet trust) and surfaced the 5th (wire format) before halting, Phase 13 absorbs the 5th — a 6th deserves an explicit user decision.
- **D-13:** **A Phase 14 (if needed) absorbs any 6th-blocker overflow.** Phase 13 is execution-only of the locked seed from 12-02-SUMMARY.md: wire-format fix + 6 unmutes from the unchanged 11-02-PLAN.md spec.

### Closure Bookkeeping & Doc-State Reconciliation

- **D-14:** **Phase 13's closeout commit reconciles the REPAIR-01 doc drift in REQUIREMENTS.md.** Line 20 of `.planning/REQUIREMENTS.md` currently shows REPAIR-01 as `[x]` even though the full_round suite was never green (Plan 12-02 halted before any unmute). The closeout commit (Plan 13-02's final atomic doc commit after all 6 PASS-proof captures land):
  - **If all 8 full_round tests are green locally** (REPAIR-01 actually closes): REPAIR-01 stays `[x]` and the commit message documents the corrected provenance (Phase 13 SHAs, not Phase 10 SHAs). The Phase 13 ROADMAP entry is marked complete in the same commit.
  - **If any test is red**: REPAIR-01 flips back to `[ ]` with the failure surfaced in the commit message; ROADMAP Phase 13 entry is NOT marked complete; Phase 14 opens.
  This is the same per-row bookkeeping pattern from Phase 12 D-13, plus an explicit reconciliation step for the pre-existing drift.
- **D-15:** **v1.3 ship notes / `gsd-complete-milestone` are NOT in scope for Phase 13.** Either a tiny wrap-up phase or `/gsd-ship` handles those after CI confirms the combined Phase 11+12+13 PR. Same disposition as Phase 11 D-12 / Phase 12 D-14.

### Claude's Discretion

- **CD-1:** Exact commit message wording for the wire-format fix commit (Plan 13-01). Default: `fix(13): encode partial sig as bitcoin::Witness for /round/sign wire format (client/src/wallet.rs)` with the safety/diagnosis rationale in the commit body, including: (a) the failure signature (HTTP 400), (b) the coordinator-side deserialization site `coordinator/src/round/signing.rs:160`, (c) cite to `.planning/phases/12-…/12-02-SUMMARY.md §"Root Cause"`, (d) the canonical-first PASS-proof per D-07. Bisect cleanliness > commit message length.
- **CD-2:** Whether to use the brew bitcoind v31 invocation literally (`BITCOIND_EXE=$(brew --prefix)/bin/bitcoind`) in commit bodies, or substitute the resolved path. Default: keep the `$(brew --prefix)` form per CONTRIBUTING.md §"Running integration tests" (reproduces on any reviewer's machine). Same default as Phase 12 CD-2.
- **CD-3:** Whether the unused `final_script_witness` fallback at `client/src/wallet.rs:283-288` is removed in Plan 13-01 or deferred. Default: defer (don't touch unrelated dead code in a wire-format fix commit — bisect cleanliness). If the planner sees a clean way to scope it as a separate atomic commit inside Plan 13-01 (e.g., `refactor(13): remove obsolete final_script_witness fallback`), that is acceptable but optional.
- **CD-4:** Local-machine `bitcoind --version | head -1` captured at PASS-proof capture time, included alongside cargo verdict in every commit body. Same as Phase 12 CD-4 — detects silent brew bumps.
- **CD-5:** Whether Plan 13-02's six commits are sequenced over a single execution session (recommended for tight bisect history) or across sessions (acceptable if the executor needs to checkpoint). Plan 12-02 attempted single-session and halted at canonical-first; Plan 13-02 should also default to single-session for the same bisect-quality reason.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 12 carry-over (the trigger and seed for Phase 13)
- [.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-02-SUMMARY.md](.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-02-SUMMARY.md) — **REQUIRED READING.** Records the 5th-blocker discovery (HTTP 400 from `/round/sign`), the verbatim panic trace, the root-cause diagnosis (raw DER vs `bitcoin::Witness` consensus encoding), both repair options (Option A client-side, Option B coordinator-side), the suggested Phase 13 plan structure, and the canonical-first invocation pattern. **§"Failure Diagnosis" + §"Proposed Minimal Repair (Phase 13 Seed)" + §"Recovery Path — Phase 13" are required reading** — Phase 13 is the recovery path's execution.
- [.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-CONTEXT.md](.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-CONTEXT.md) — D-11 escape-valve discipline (inherited unchanged), D-12 next-phase absorption (Phase 13 = the absorber), D-04 verbatim-reuse of Plan 11-02 spec (now extended one phase further to Plan 13-02), CD-2 brew invocation form, CD-4 bitcoind version capture.
- [.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-01-SUMMARY.md](.planning/phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-01-SUMMARY.md) — Plan 12-01's bisect-clean discipline (one-commit wallet-trust fix at `0bbcf3c`). Plan 13-01 mirrors this shape with the added D-07 sanity capture.

### Phase 11 carry-over (the locked unmute spec Plan 13-02 reuses)
- [.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md) — The locked-in unmute spec Plan 13-02 reuses VERBATIM. Plan 13-02 is conceptually this plan re-executed for the third time (Plan 12-02 was the second attempt); the canonical-first order, per-test commit cycle, and PASS-proof body shape carry forward unchanged. Only the SHA-reference list grows from two-SHA (Phase 12) to three-SHA (Phase 13).
- [.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md) — Records the 4th-blocker discovery (Phase 12's seed) and the per-test commit cycle origin. The same "escape-valve halt then next-phase absorbs" pattern is now in its third iteration; Phase 13 inherits the pattern unmodified.
- [.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-CONTEXT.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-CONTEXT.md) — D-05 (canonical-first order rationale), D-07 (per-test commit discipline), D-08 (escape-valve origin) — Phase 13's Plan 13-02 inherits all three.

### Phase 10 carry-over (the test infrastructure)
- [.planning/phases/10-full-round-rs-decision-execution/10-02-SUMMARY.md](.planning/phases/10-full-round-rs-decision-execution/10-02-SUMMARY.md) — Records Fix A (`d99b3a4`) and Fix WIF-D (`e02ce55`) that Phase 13's wire-format fix builds on. Plan 13-02's PASS-proof captures depend on these being in history.
- [.planning/phases/10-full-round-rs-decision-execution/10-CONTEXT.md](.planning/phases/10-full-round-rs-decision-execution/10-CONTEXT.md) — D-07 per-test commit cycle (origin) + D-11 escape valve discipline (origin).

### Fix locus (the file Plan 13-01 modifies)
- [client/src/wallet.rs](client/src/wallet.rs) lines 276-291 — `sign_psbt_input` partial-sig extraction block. **Single-file fix locus.** Line 279 currently destructures `(_pk, sig)` and returns `Ok(sig.to_vec())` (raw DER bytes). Plan 13-01 turns this into a 2-item `bitcoin::Witness` and returns `consensus::serialize(&witness)`.
- [client/src/wallet.rs](client/src/wallet.rs) lines 283-288 — `final_script_witness` fallback. Obsolete by design after the fix (P2WPKH always populates `partial_sigs`); leave untouched per D-02 / CD-3.
- [client/src/wallet.rs](client/src/wallet.rs) line 5 — `bitcoin::Witness` import already exists transitively via `use bitcoin::*` patterns elsewhere; planner verifies whether an explicit `use bitcoin::Witness;` is needed.

### Deserialization site (the contract the client must match — read-only context)
- [coordinator/src/round/signing.rs](coordinator/src/round/signing.rs) lines 156-179 — the loop that calls `bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes)` and inserts the result into `psbt.inputs[i].final_script_witness`. **This is the contract Plan 13-01's encoding must satisfy.** Phase 13 makes NO changes here.
- [coordinator/src/round/signing.rs](coordinator/src/round/signing.rs) line 165 — the `Invalid witness data for input {i}` error path. Coordinator-side hardening (richer error) is deferred per D-03.

### Test infrastructure & invocation (Phase 9/10/11/12 carry-over)
- [tests/integration/full_round.rs](tests/integration/full_round.rs) — six `#[ignore = "TODO(Phase-10): ..."]` sites at lines 164, 462, 730, 854, 911, 1236. Plan 13-02 unmutes these six in canonical-first then file order. **Same six sites Plan 12-02 was going to unmute** — `grep -c 'TODO(Phase-10)'` should equal 6 before Plan 13-02 starts.
- [tests/integration/mod.rs](tests/integration/mod.rs) — `require_bitcoind!`, `BitcoindGuard`, `fund_regtest`, `FundedSetup`. Phase 13 does NOT modify these.
- [CONTRIBUTING.md](CONTRIBUTING.md) §"Running integration tests" — the canonical local invocation. Plan 13-01's D-07 sanity capture and Plan 13-02's six PASS captures all use this pattern byte-for-byte.

### Wire-protocol surface (read-only context)
- [shared/src/protocol.rs](shared/src/protocol.rs) — `SignRequest` / `PartialSig` wire structs (if defined). The bytes Plan 13-01 produces are transmitted over this surface; planner verifies the field on the request is `Vec<u8>` / opaque-bytes (not a typed `Witness`) so no shared protocol change is needed.
- [client/src/round/sign.rs](client/src/round/sign.rs) — the client-side caller of `wallet.sign_psbt_input(...)`. Phase 13 does NOT modify this; the wire-format change is encapsulated in `sign_psbt_input`'s return value.

### Project ground truth
- [.planning/ROADMAP.md](.planning/ROADMAP.md) §"Phase 13" — the phase description Phase 13 fulfills.
- [.planning/REQUIREMENTS.md](.planning/REQUIREMENTS.md) line 20 §REPAIR-01 — **doc drift to be reconciled per D-14.** Currently shows `[x]` despite full_round being red.
- [.planning/REQUIREMENTS.md](.planning/REQUIREMENTS.md) line 21 §REPAIR-02 — stays `[ ]` pending PR observation (Phase 11 D-11 / Phase 12 D-13).
- [.planning/STATE.md](.planning/STATE.md) — current resume pointer (records Phase 12 halt + Phase 13 absorption).
- [CLAUDE.md](CLAUDE.md) — recommended stack (bdk_wallet 2.3, blind-rsa-signatures jedisct1, corepc-node feature-pinned).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`BdkClientWallet::sign_psbt_input`** ([client/src/wallet.rs:243-291](client/src/wallet.rs:243)) — the method that owns the fix locus. Its existing structure (find input by `previous_output`, populate `witness_utxo`, call `sign` with `trust_witness_utxo: true` per Phase 12, extract partial sig) is unchanged by Phase 13; only the extraction-and-return at lines 279-281 changes.
- **`pk` from `partial_sigs.iter().next()`** ([client/src/wallet.rs:279](client/src/wallet.rs:279)) — bdk_wallet populates `partial_sigs: BTreeMap<bitcoin::PublicKey, bitcoin::ecdsa::Signature>` after signing. The pubkey is by construction the one bdk used to sign — exactly what the witness needs. No need to thread a separate pubkey through the wallet struct.
- **`bitcoin::Witness::new() + .push() + consensus::serialize`** — standard `rust-bitcoin` 0.32.x pattern, identical to what the coordinator deserializes at `signing.rs:160`. Symmetric round-trip means Plan 13-01 cannot introduce a divergent encoding.
- **Plan 11-02's locked unmute spec** ([.planning/phases/11-…/11-02-PLAN.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md)) — Plan 13-02 reuses it verbatim. Third iteration of the same spec; only the SHA-list grows.
- **Plan 12-01's bisect-clean discipline** — Plan 13-01 mirrors the shape (single source-fix commit + commit body with safety/diagnosis rationale) with the D-07 addition of an in-body canonical-first PASS capture.
- **Phase 9-05 CONTRIBUTING.md invocation pattern** — Plan 13-01's sanity capture and Plan 13-02's six PASS captures use it byte-for-byte: `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round::<name> -- --ignored`.

### Established Patterns
- **Per-test commit cycle with PASS proof in commit body** (Phase 10 D-07, Phase 11 D-07, Phase 12 D-04) — Plan 13-02's six unmute commits each follow this; commit body captures the exact `cargo test … -- --ignored` invocation + cargo verdict line + `bitcoind --version | head -1` + SHA refs to all three prerequisite fixes (RSA `cc20f6f`, wallet-trust `0bbcf3c`, wire-format Plan 13-01's SHA).
- **One-commit source fix with rationale-in-body, not in-source** (D-10 departure from Phase 12 D-08) — Plan 13-01's commit body carries the failure-signature + cross-ref to 12-02-SUMMARY.md instead of an in-source block comment. Reduces in-source comment noise; bisect-friendly via `git log -p`.
- **Cross-phase SHA references in commit bodies** (Phase 11 → `cc20f6f`, Phase 12 → adds `0bbcf3c`) — Plan 13-02 extends this to three SHAs (RSA + wallet-trust + wire-format). The pattern accumulates; the discipline holds.
- **D-11 escape-valve halt-and-surface protocol** (Phase 11 origin, Phase 12 first invocation) — Plan 13-02 inherits unmodified. If a 6th orthogonal blocker appears, halt at first encounter, emit checkpoint with failure mode and minimal repair proposal, Phase 14 absorbs.
- **Sanity capture in commit body as bisect gate** (Plan 12-01 → Plan 13-01 new under D-07) — Plan 13-01's commit body includes the canonical-first PASS verdict; the same canonical-first test re-runs as Plan 13-02's first unmute commit (the `#[ignore]` removal), and that re-run is the bisect-proof for the unmute itself.

### Integration Points
- **Plan 13-01 → Plan 13-02 dependency.** Plan 13-02's PASS captures cannot succeed without Plan 13-01's wire-format fix in history (HTTP 400 will recur). The planner should mark Plan 13-02 as `depends_on: 13-01`.
- **Three prerequisite SHAs MUST be in history before Plan 13-02 starts:** Phase 11 `cc20f6f` (RSA SPKI), Phase 12 `0bbcf3c` (wallet trust_witness_utxo), Phase 13 Plan 13-01 (wire format). Plan 13-02's first task verifies all three via `git log --oneline | grep -E 'cc20f6f|0bbcf3c|<plan-13-01-sha>'`.
- **Brew bitcoind v31 is the test prerequisite.** Plan 13-01 and all six Plan 13-02 commits capture `bitcoind --version | head -1` output in commit bodies (CD-4) to detect silent brew bumps moving bitcoind off the pinned v31.
- **No coordinator-side changes in Phase 13.** Phase 11's `cc20f6f` (RSA fix) + `13da4b5` (SPKI roundtrip test) and Phase 12's `0bbcf3c` (wallet trust) are the only coordinator/client crypto-path changes carried forward; Phase 13 is purely a client-wallet wire-format fix + integration-test unmutes.
- **Wire-protocol contract is fixed by the coordinator deserializer.** Any future change to the wire format on either side becomes a coordinated multi-phase effort because the coordinator deserializer is the implicit spec. After Phase 13 lands, the wire format is durably consensus-serialized `bitcoin::Witness` — codify in shared/src/protocol.rs documentation if the planner sees a clean scope for it (otherwise defer).

</code_context>

<specifics>
## Specific Ideas

- **The fix is literally ~4 lines of code** at `client/src/wallet.rs:279-281`. Plan 13-01's diff scope is: change destructuring from `(_pk, sig)` to `(pk, sig)`, replace one return-line with 4 lines that build a `bitcoin::Witness` and consensus-serialize it. Total ≤ 6 LOC of source change in the partial_sigs branch. Anything larger is scope drift.
- **The commit body MUST carry the rationale** (D-10): failure signature (`HTTP 400 Bad Request from /round/sign`), the coordinator-side deserialization site (`coordinator/src/round/signing.rs:160`), cross-ref to `.planning/phases/12-…/12-02-SUMMARY.md §"Root Cause"`, and the canonical-first PASS verdict per D-07. No in-source block comment.
- **Plan 13-02 reuses Plan 11-02 / Plan 12-02 verbatim** modulo the three-SHA body shape. The planner should treat `11-02-PLAN.md` as a pinned template and either (a) copy it under Plan 13-02 with only the SHA-reference list updated, or (b) reference it by path and only enumerate the additions (three-SHA commit-body shape). Either packaging is acceptable; rewriting the spec itself is not.
- **Canonical-first remains the non-negotiable gate** in Plan 13-02's first commit (line 164 `full_round_three_clients` unmute). The D-07 sanity capture inside Plan 13-01 should pass before Plan 13-02 even opens; if Plan 13-01's capture fails, Phase 14 absorbs the 6th blocker (Plan 13-02 never opens).
- **REPAIR-01 closes ONLY when all 8 tests are green.** Partial green (e.g., 7/8) does NOT close REPAIR-01 — D-14 flips the line-20 `[x]` back to `[ ]`. The criterion is the same as Phase 11 D-10 and Phase 12 D-13.
- **The unused `final_script_witness` fallback at lines 283-288** is leftover from an earlier signing path that's now unreachable. CD-3 defaults to leaving it untouched (bisect cleanliness) but allows the planner to scope a separate atomic refactor commit if it's clean.

</specifics>

<deferred>
## Deferred Ideas

- **Option B — coordinator-side parse raw DER + reconstruct Witness server-side.** Requires `coordinator/src/round/input_reg.rs` to retain the participant's pubkey at input-registration time, then `signing.rs` constructs the `bitcoin::Witness` from stored pubkey + the raw DER sig the client sent. Heavier than Option A; crosses the wire-protocol contract; not needed because the client is unambiguously the side producing wrong bytes. Reconsider only if a future client implementation cannot encode `bitcoin::Witness` (e.g., a hardware-wallet-driven client where the partial sig is returned as raw DER bytes from the device and the host has no way to build a Witness around it).
- **Coordinator error-message hardening at `coordinator/src/round/signing.rs:165`.** "Invalid witness data for input {i}" cost ~30 min of grep-archaeology in Plan 12-02. Including `sig_bytes.len()` + first byte in the error would make any future format drift surface in 30s. Out of scope per D-03 (would dilute Plan 13-01's bisect cleanliness). Reconsider as a small standalone phase or `/gsd-quick` after Phase 13 ships.
- **Removal of the obsolete `final_script_witness` fallback at `client/src/wallet.rs:283-288`.** Dead code path after Plan 13-01's fix (P2WPKH always populates `partial_sigs`). CD-3 allows the planner to scope it as a separate atomic commit inside Plan 13-01; otherwise defer to a follow-up cleanup phase or `/gsd-quick`.
- **Wallet-level unit test (`#[test] fn sign_psbt_input_returns_serialized_witness()` in `client/src/wallet.rs`).** Rejected for Phase 13 (D-08) — `full_round.rs` is the end-to-end coverage. Reconsider if a future `bitcoin::Witness` encoding bug slips past the integration suite.
- **Wallet-level integration test (`tests/integration/wallet_signing.rs`).** Rejected for Phase 13 (D-09). Reconsider if a real driver appears.
- **Mainnet enablement (and the Option B repair that mainnet would require).** Inherited deferred from Phase 12. Still gated on mainnet-design work.
- **Codify the wire format in `shared/src/protocol.rs` documentation.** Currently the wire format is the implicit spec defined by the coordinator's deserializer. A doc comment on the `SignRequest`/`PartialSig` field stating "consensus-serialized `bitcoin::Witness` (P2WPKH 2-item stack: sig + pubkey)" would prevent future client implementations from reproducing the Phase 12 bug. CD-3 allows the planner discretion; otherwise defer.
- **Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64`.** Inherited deferred from Phase 11 / Phase 12 — still a wire-format-change-disguised-as-refactor, still out of scope.
- **`-txindex=1` in Phase 9-02's `bootstrap_regtest_bitcoind`.** Inherited deferred — Fix A made it unnecessary.
- **v1.3 ship notes and `/gsd-complete-milestone v1.3`.** Deferred to a wrap-up phase or direct `/gsd-ship` after Phase 13's PR (Phase 11+12+13 combined) is observed green in CI. Same disposition as Phase 11 D-12 / Phase 12 D-14.
- **In-source `// TODO(mainnet):` markers anywhere in the wallet code.** Rejected per D-11. The mainnet revisit list lives in CONTEXT.md, not in stale in-source comments.

</deferred>

---

*Phase: 13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle*
*Context gathered: 2026-05-28*
