# Phase 12: Repair client/src/wallet.rs:260 (bdk_wallet 2.3 SignOptions) + complete Plan 11-02 unmute cycle - Context

**Gathered:** 2026-05-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 12 delivers two coupled fixes that together close out the 4th orthogonal blocker surfaced by Plan 11-02 and unblock REPAIR-01:

1. **Repair `client/src/wallet.rs:260` for bdk_wallet 2.3 SignOptions semantics.** In bdk_wallet 2.3.0, `SignOptions::default()` sets `trust_witness_utxo: false` as a BIP-143 fee-spoof mitigation, which causes signing to demand `non_witness_utxo` even when only `witness_utxo` is populated. Apply the minimal Option A repair: `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }`. The client populates `witness_utxo` from its own trusted regtest RPC, so `trust_witness_utxo: true` is contextually safe.

2. **Re-execute Plan 11-02's six-unmute cycle verbatim.** Once the wallet repair lands, the spec from `11-02-PLAN.md` (canonical-first `full_round_three_clients` then 5 in file order, one bisectable commit per unmute, CD-1 PASS-proof bodies) becomes executable as written. REPAIR-01 closes when all 8 full_round tests are green locally against pinned brew bitcoind v31.

**Net effect:** REPAIR-01 closes in Phase 12; REPAIR-02 closes on the Phase 11+12 combined PR being observed green in CI (per Phase 11 D-11).

**Not in scope:** mainnet enablement, Option B (populating `non_witness_utxo` from RPC), rewriting Plan 11-02's unmute spec (it is reused verbatim), rename of `rsa_pubkey_der_b64`, `-txindex=1` in `bootstrap_regtest_bitcoind`, v1.3 ship notes, retiring full_round tests under D-10, adding a wallet-level unit test for `sign_psbt_input`.

</domain>

<decisions>
## Implementation Decisions

### Repair Approach

- **D-01:** **Option A — `trust_witness_utxo: true`.** At `client/src/wallet.rs:260`, change `SignOptions::default()` to `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }`. Single-line functional change. Option B (populate `non_witness_utxo` from RPC) is rejected for Phase 12: it requires plumbing a bitcoind RPC handle into `BdkClientWallet` (currently absent), which is ~30-50 LOC of new field + caller updates and crosses the wallet/RPC boundary. Option B re-surfaces as a deferred item gated on mainnet enablement (see Deferred Ideas + the in-source TODO is explicitly NOT added per D-04).
- **D-02:** **Fix locus is `client/src/wallet.rs:260` only.** No changes elsewhere in `client/src/wallet.rs` (the wallet struct, constructors, and other methods are unaffected). The `#[allow(deprecated)]` on the `SignOptions` import at line 5-6 stays as-is — it was added in Phase 10's WIF-D fix and is orthogonal to this repair.

### Scope Coupling — Wallet Repair + Plan 11-02 Six Unmute Commits

- **D-03:** **Phase 12 owns REPAIR-01 closure.** The wallet repair AND Plan 11-02's locked-in six-unmute cycle both land in Phase 12. This matches the ROADMAP phase description verbatim ("After the wallet repair lands, Plan 11-02 six unmute commits become executable verbatim"). REPAIR-01 flips to `[x]` when all 8 `full_round::*` tests are green locally against pinned brew bitcoind v31 in Phase 12. REPAIR-02 closure stays tied to PR observation per Phase 11 D-11 — a single PR will carry Phase 11 (RSA fix) + Phase 12 (wallet repair + 6 unmutes), and REPAIR-02 closes on its CI green observation.
- **D-04:** **Reuse Plan 11-02's unmute spec verbatim.** Phase 12 must NOT rewrite, re-order, or re-justify the six-unmute cycle. The locked-in artifacts from `11-02-PLAN.md` carry forward unchanged:
  - **Canonical-first order:** `full_round_three_clients` (line 164) → then file order: lines 462, 730, 854, 911, 1236
  - **Per-test commit cycle (×6):** one atomic commit per unmute; each commit removes only the single `#[ignore = …]` line for that one test
  - **CD-1 commit body shape:** one-line `cargo test --test integration full_round::<name> -- --ignored` invocation + the cargo PASS verdict line + SHA references to the wallet-fix commit (Phase 12) and the RSA-fix commit (Phase 11 `cc20f6f`)
  - **No drive-by edits** to other tests, helpers, or unrelated code in any unmute commit
- **D-05:** **Two-plan structure inside Phase 12.** Suggested plan partition for the planner: Plan 12-01 = wallet repair (one commit) — must land first; Plan 12-02 = the six-test unmute cycle (six commits, canonical-first then file order) — depends on Plan 12-01. The canonical-first happy-path test functions as the gate between the two plans: if `full_round_three_clients` passes after the wallet repair, the remaining 5 proceed mechanically; if it fails, the next blocker surfaces against the simplest test.

### Regression Coverage

- **D-06:** **No new wallet-level unit test.** The unmuted `full_round.rs` suite is the end-to-end coverage and exercises the signing path with real bitcoind in regtest. A wallet-level `#[test] fn sign_psbt_input_signs_segwit_input` would need PSBT fixture setup and would duplicate the integration coverage. This mirrors Phase 11 D-04's reasoning that the full_round suite is the canonical end-to-end coverage for the RSA fix; the same logic applies here for the wallet fix.
- **D-07:** **No belt-and-braces.** No additional integration test file (e.g., `tests/integration/wallet_signing.rs`) — Phase 11 D-04 rejected the analogous option for the RSA fix and the same reasoning applies. If a future bdk_wallet bump silently flips `trust_witness_utxo` semantics again, the next CI run on PR will catch it via the full_round suite.

### In-Source Rationale (the safety contract)

- **D-08:** **Multi-line block comment above the `self.inner.sign(...)` call.** 5-10 line comment that explains:
  1. **What bdk_wallet 2.3 changed and why** — `SignOptions::default()` now sets `trust_witness_utxo: false` as a BIP-143 fee-spoof mitigation. With `witness_utxo` only (no `non_witness_utxo`), an attacker who has only `witness_utxo` set could spoof a higher value than the actual previous output, tricking the signer into authorizing more fee than expected.
  2. **Why `trust_witness_utxo: true` is safe here** — the client populates `witness_utxo` from `self.utxo_value_sats`, which was set at wallet construction from the same regtest RPC that we treat as the ground-truth source. There is no attacker-controlled value to spoof in this CoinJoin client's signing path: the witness value originates inside the client, not from a counterparty's PSBT.
  3. **What would change this** — mainnet enablement or any future code path that signs PSBTs whose `witness_utxo` was supplied by an untrusted counterparty. At that point Option B (populate `non_witness_utxo` from RPC) becomes the required repair.
- **D-09:** **No `// TODO(mainnet):` marker in-source.** The mainnet revisit is captured in `12-CONTEXT.md` Deferred Ideas (this file). Sprinkling TODO markers across the codebase for future-mainnet work is a known anti-pattern that produces stale comments; the planning archive is the canonical store for "what to revisit before mainnet."
- **D-10:** **Cross-reference Phase 11-02-SUMMARY in the block comment.** Include a one-line cite: `// See .planning/phases/11-.../11-02-SUMMARY.md §"Two minimal-repair candidates" for the alternative.` This gives a future reader the full diagnostic walkthrough without bloating the in-source comment.

### Escape Valve & Drift Discipline

- **D-11:** **Phase 11's D-08 escape-valve discipline applies to Plan 12-02 unmodified.** If during the six-unmute cycle a 5th orthogonal blocker appears in ≥1 test, the executor halts after the first encounter and emits a checkpoint with the failure mode and a proposed minimal repair. Pre-authorized in-flight scope expansion is **zero**. Phase 11 already absorbed three orthogonal blockers (vout-after-mine, bdk 2.3 wallet API, RSA SPKI) plus this 4th (bdk 2.3 SignOptions); a 5th deserves an explicit user decision.
- **D-12:** **A Phase 13 (if needed) absorbs any 5th-blocker overflow.** Phase 12 is execution-only of the locked spec: wallet repair + 6 unmutes from the unchanged 11-02-PLAN.md.

### Closure Bookkeeping

- **D-13:** When all 8 full_round tests pass locally, mark `REPAIR-01` as `[x]` in `.planning/REQUIREMENTS.md` and update the Phase 10 + Phase 11 + Phase 12 rows in `.planning/ROADMAP.md` accordingly. REPAIR-02 status remains `[ ]` until the combined Phase 11+12 PR is observed green in CI.
- **D-14:** **v1.3 ship notes / `gsd-complete-milestone` are NOT in scope for Phase 12.** Either a tiny wrap-up phase or `/gsd-ship` handles those after CI confirms the PR (same disposition as Phase 11 D-12).

### Claude's Discretion

- **CD-1:** Exact commit message wording for the wallet-repair commit. Default: `fix(12): trust_witness_utxo for bdk_wallet 2.3 SignOptions (client/src/wallet.rs:260)` with the safety rationale in the commit body. Bisect cleanliness > commit message length.
- **CD-2:** Whether to use the brew bitcoind v31 invocation literally (`BITCOIND_EXE=$(brew --prefix)/bin/bitcoind`) in commit bodies, or substitute the resolved path. Default: keep the `$(brew --prefix)` form per CONTRIBUTING.md §"Running integration tests" — it reproduces on any reviewer's machine.
- **CD-3:** Whether the wallet-fix commit and the unit-test commit collapse — N/A here because D-06 forbids a new unit test. Default: Plan 12-01 = single commit (the wallet fix).
- **CD-4:** Local-machine bitcoind version asserted at PASS-proof capture time. Default: capture `bitcoind --version | head -1` output in the commit body alongside the cargo verdict line. Catches the case where a brew bump silently moves bitcoind off v31 between Phase 11 and Phase 12.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 11 carry-over (the trigger for Phase 12)
- [.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-SUMMARY.md) — Records the 4th-blocker discovery, the verbatim panic trace, both repair candidates (Option A + Option B), and the resume protocol. **§"Failure Diagnosis" + §"Two minimal-repair candidates" + §"Resume protocol" are required reading** — Phase 12 is the resume protocol's execution.
- [.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md) — The locked-in unmute spec Phase 12 reuses VERBATIM. Plan 12-02 is conceptually this plan re-executed; the canonical-first order, per-test commit cycle, and CD-1 body shape carry forward unchanged.
- [.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-CONTEXT.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-CONTEXT.md) — D-05 (canonical-first order rationale), D-07 (per-test commit discipline), D-08 (escape-valve) — Phase 12's Plan 12-02 inherits all three.

### Phase 10 carry-over (the fixes Phase 12 depends on being in history)
- [.planning/phases/10-full-round-rs-decision-execution/10-02-SUMMARY.md](.planning/phases/10-full-round-rs-decision-execution/10-02-SUMMARY.md) — Records Fix A (d99b3a4) and Fix WIF-D (e02ce55) that Phase 12's wallet fix builds on.
- [.planning/phases/10-full-round-rs-decision-execution/10-CONTEXT.md](.planning/phases/10-full-round-rs-decision-execution/10-CONTEXT.md) — D-07 per-test commit cycle (origin) + D-11 escape valve discipline (origin).

### Fix locus
- [client/src/wallet.rs](client/src/wallet.rs) lines 243-278 — `sign_psbt_input` method body. **Line 260 is the single-line fix locus.** Lines 252-256 (witness_utxo population from `self.utxo_value_sats`) are the safety contract anchor — that data path is what makes `trust_witness_utxo: true` safe in this context.
- [client/src/wallet.rs](client/src/wallet.rs) lines 17-27 — `BdkClientWallet` struct definition. `utxo_value_sats: u64` is the field whose trusted origin (regtest RPC at wallet construction) underwrites the D-08 safety rationale.

### Test infrastructure & invocation (Phase 9/10 carry-over)
- [tests/integration/mod.rs](tests/integration/mod.rs) — `require_bitcoind!`, `BitcoindGuard`, `fund_regtest`, `FundedSetup`. Phase 12 does NOT modify these.
- [tests/integration/full_round.rs](tests/integration/full_round.rs) — six `#[ignore = "TODO(Phase-10): ..."]` sites at lines 164, 462, 730, 854, 911, 1236. Plan 12-02 unmutes these six in canonical-first then file order.
- [CONTRIBUTING.md](CONTRIBUTING.md) §"Running integration tests" — the canonical local invocation. CD-2 keeps the `$(brew --prefix)` form for reviewer reproducibility.

### Protocol & dependencies (read-only context)
- [coordinator/src/blind/rsa.rs](coordinator/src/blind/rsa.rs) — unchanged in Phase 12; only relevant for Phase 11 cross-reference.
- [client/src/round/input.rs](client/src/round/input.rs) line 40 — Phase 11 fix locus (`from_spki`); confirmed landed at `cc20f6f`. Phase 12 commit bodies cite this SHA.
- `~/.cargo/registry/src/index.crates.io-*/bdk_wallet-2.3.*/src/signer.rs` — `SignOptions` struct definition. Researcher should re-confirm the `trust_witness_utxo` field exists and defaults to `false` against the pinned 2.3 release before planning.

### Project ground truth
- [.planning/ROADMAP.md](.planning/ROADMAP.md) §"Phase 12" — the phase description Phase 12 fulfills.
- [.planning/REQUIREMENTS.md](.planning/REQUIREMENTS.md) §REPAIR-01, §REPAIR-02 — closure criteria; REPAIR-01 flips in-phase, REPAIR-02 stays PR-gated per Phase 11 D-11.
- [.planning/STATE.md](.planning/STATE.md) — current resume pointer.
- [CLAUDE.md](CLAUDE.md) — recommended stack (bdk_wallet 2.3, blind-rsa-signatures jedisct1, corepc-node feature-pinned).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`BdkClientWallet::sign_psbt_input`** ([client/src/wallet.rs:243-278](client/src/wallet.rs:243)) — the method that owns line 260. Its existing structure (find input by `previous_output`, populate `witness_utxo`, call `sign`, extract partial sig) is unchanged by Phase 12; only the `SignOptions` argument flips.
- **`self.utxo_value_sats: u64`** ([client/src/wallet.rs:21](client/src/wallet.rs:21)) — the trusted-origin field that underwrites D-08's safety rationale. Set at wallet construction time from regtest RPC; never mutated; never derived from a counterparty PSBT.
- **`SignOptions` from `bdk_wallet::signer`** ([client/src/wallet.rs:5-6](client/src/wallet.rs:5)) — already imported with `#[allow(deprecated)]`. No new import needed; only the construction form changes from `SignOptions::default()` to `SignOptions { trust_witness_utxo: true, ..SignOptions::default() }`.
- **Plan 11-02's locked unmute spec** ([.planning/phases/11-.../11-02-PLAN.md](.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-02-PLAN.md)) — Plan 12-02 reuses it verbatim. The 6 unmute commits, the canonical-first order, the CD-1 body shape, and the per-test commit discipline all carry over without modification.
- **Phase 9-05 CONTRIBUTING.md invocation pattern** — Plan 12-02's six PASS-proof captures use it byte-for-byte: `BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind cargo test --test integration full_round::<name> -- --ignored`.

### Established Patterns
- **Per-test commit cycle with PASS proof in commit body** (Phase 10 D-07, Phase 11 D-07) — Plan 12-02's six unmute commits each follow this; commit body captures the exact `cargo test … -- --ignored` invocation + cargo verdict line + SHA refs to Phase 11 RSA fix (`cc20f6f`) and Phase 12 wallet fix (Plan 12-01's commit SHA).
- **Multi-line in-source safety comments at threat-model-sensitive sites** (e.g., the Tor accept-loop `Semaphore` rationale in coordinator from Phase 8) — Phase 12's wallet-repair comment follows the same shape: explain the default's threat model, explain why this context is safe, name the precondition for revisit.
- **Cross-phase SHA references in commit bodies** (Phase 11 commit bodies reference `cc20f6f` for the RSA fix) — Plan 12-02's six unmute commit bodies reference BOTH the Phase 11 RSA-fix SHA (`cc20f6f`) AND the Phase 12 wallet-fix SHA (Plan 12-01's commit).
- **D-08 escape-valve halt-and-surface protocol** (Phase 11 origin) — Plan 12-02 inherits unmodified. If a 5th orthogonal blocker appears, halt at first encounter, emit checkpoint with failure mode and minimal repair proposal, Phase 13 absorbs.

### Integration Points
- **The wallet-fix and the unmute cycle MUST land in the order Plan 12-01 → Plan 12-02.** Plan 12-02's PASS-proof captures will fail with the same `Missing non-witness UTXO` panic if Plan 12-01's commit is not in history. The planner should mark Plan 12-02 as `depends_on: 12-01`.
- **Brew bitcoind v31 is the test prerequisite.** Plan 12-02 must capture `bitcoind --version | head -1` output in commit bodies (CD-4) to detect silent brew bumps moving bitcoind off the pinned v31 between Phase 11 and Phase 12.
- **No coordinator-side changes in Phase 12.** Phase 11's `cc20f6f` (RSA fix) and `13da4b5` (SPKI roundtrip test) are the only coordinator/client crypto-path changes; Phase 12 is purely client-wallet + integration-test unmutes.

</code_context>

<specifics>
## Specific Ideas

- **The fix is literally one line of code** at `client/src/wallet.rs:260`. Plan 12-01's diff scope is: one struct-literal swap on the `SignOptions` argument, plus the multi-line block comment above it (D-08). Total ≤ 15 LOC of source change. Anything larger is scope drift.
- **Block comment must answer three questions in order** (D-08): (1) what changed in bdk_wallet 2.3 and why; (2) why `trust_witness_utxo: true` is safe HERE specifically; (3) what would invalidate this safety assumption. Order matters — a future reader scanning top-to-bottom should hit the threat model first, then the local-context safety argument, then the precondition for revisit.
- **Plan 12-02 reuses Plan 11-02 verbatim.** The planner should treat `11-02-PLAN.md` as a pinned template and either (a) copy it under a new plan ID with only the dependency/SHA references updated, or (b) reference it by path and only enumerate the additions (PASS-proof commit body must reference both Phase 11 RSA fix SHA `cc20f6f` AND Phase 12 wallet-fix SHA). Either packaging is acceptable; rewriting the spec itself is not.
- **Canonical-first remains the non-negotiable gate.** Plan 12-02's first PASS-proof capture is `full_round_three_clients` (line 164). If it fails, Plan 12-02 halts after the first attempt per D-11 inherited escape valve — the wallet repair has surfaced a new (5th) blocker and Phase 13 absorbs.
- **REPAIR-01 closes ONLY when all 8 tests are green.** Partial green (e.g., 7/8) does NOT close REPAIR-01. The criterion is the same as Phase 11 D-10 — locally green full_round suite + 6 unmute commits landed.

</specifics>

<deferred>
## Deferred Ideas

- **Option B — populate `non_witness_utxo` from RPC in `sign_psbt_input`.** Plumbs a bitcoind RPC handle into `BdkClientWallet`, fetches the full previous transaction via `get_raw_transaction(<txid>)`, and sets `psbt.inputs[input_idx].non_witness_utxo`. Required path before mainnet enablement (any PSBT whose `witness_utxo` was supplied by an untrusted counterparty). NOT applied in Phase 12: Option A is contextually safe today, and the wallet/RPC boundary should be re-thought as part of mainnet design rather than bolted on as a one-off.
- **Wallet-level unit test (`#[test] fn sign_psbt_input_signs_segwit_input` in `client/src/wallet.rs`).** Rejected for Phase 12 (D-06) — full_round.rs is the end-to-end coverage. Reconsider if a future bdk_wallet bump silently flips signing semantics again AND full_round catches it late enough to be expensive.
- **Wallet-level integration test (`tests/integration/wallet_signing.rs`).** Rejected for Phase 12 (D-07) — same reasoning as Phase 11 D-04. Reconsider if a real driver appears (e.g., a test exercising `sign_psbt_input` without driving a full CoinJoin round).
- **Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64`.** Inherited deferred from Phase 11 — still a wire-format-change-disguised-as-refactor, still out of scope.
- **`-txindex=1` in Phase 9-02's `bootstrap_regtest_bitcoind`.** Inherited deferred from Phase 11 — Fix A made it unnecessary; if a future test needs txindex it gets its own scoped change.
- **v1.3 ship notes and `/gsd-complete-milestone v1.3`.** Deferred to a wrap-up phase or direct `/gsd-ship` after Phase 12's PR (Phase 11+12 combined) is observed green in CI.
- **In-source `// TODO(mainnet):` markers anywhere in the wallet code.** Rejected per D-09 — the mainnet revisit list lives in this CONTEXT.md, not in stale in-source comments. The Option B revisit is captured here under "Deferred Ideas" so future planners see it when scoping mainnet work.
- **Investigating other RPC paths in the workspace that might expect the old `SignOptions::default()` semantics.** Phase 12 fixes only the known site at `client/src/wallet.rs:260`. A grep sweep for `SignOptions::default()` is a 30-second follow-up if any other consumer is suspected.

</deferred>

---

*Phase: 12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r*
*Context gathered: 2026-05-28*
