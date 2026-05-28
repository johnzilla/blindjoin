# Phase 12: Repair client/src/wallet.rs:260 (bdk_wallet 2.3 SignOptions) + complete Plan 11-02 unmute cycle - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-28
**Phase:** 12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r
**Areas discussed:** Repair option, Phase scope, Regression coverage, In-source rationale

---

## Repair option

| Option | Description | Selected |
|--------|-------------|----------|
| Option A — `trust_witness_utxo: true` | Single-line change at line 260: `SignOptions { trust_witness_utxo: true, ..default() }`. Safe in our context since the client populated `witness_utxo` from its own trusted regtest RPC. Mirrors how BDK examples handle CLI-controlled PSBTs. No new RPC dependency. | ✓ |
| Option B — populate `non_witness_utxo` from RPC | Fetch full previous tx via `get_raw_transaction` in `sign_psbt_input`. Requires plumbing a bitcoind RPC handle into `ClientWallet` (currently absent). Heavier (~30-50 LOC + new field + caller updates), but byte-for-byte BIP-143 fee-spoof protection on the client side. | |
| Option C — both, gated by config/network | Default to Option A on regtest/signet (matches today), use Option B path on mainnet. Defers the harder work until mainnet is real. | |

**User's choice:** Option A — `trust_witness_utxo: true`
**Notes:** Aligns with Phase 11-02-SUMMARY's recommended path. Option B is captured under Deferred Ideas as the required pre-mainnet repair; the wallet/RPC boundary should be re-thought as part of mainnet design rather than bolted on. Option C was rejected because the conditional adds complexity that's only useful between "today" and "mainnet day"; cleaner to apply Option A now and lift to Option B in a single mainnet-prep phase.

---

## Phase scope

| Option | Description | Selected |
|--------|-------------|----------|
| Wallet repair + bundled 11-02 six unmute commits | Phase 12 absorbs Plan 11-02's locked-in unmute spec verbatim (canonical-first, then file order, one bisectable commit per test, CD-1 PASS-proof bodies). REPAIR-01 closes in Phase 12. Matches the ROADMAP phase description directly. | ✓ |
| Wallet repair only — leave 11-02 for `/gsd:execute-phase 11 --resume` | Phase 12 lands only the SignOptions fix + regression coverage. Plan 11-02 stays untouched and is re-executed under Phase 11 once Phase 12 ships. Cleaner phase boundary but adds a manual hop. | |
| Wallet repair + smoke-prove canonical-first, leave remaining 5 for Phase 11 resume | Phase 12 lands the fix and the first unmute commit (`full_round_three_clients`) as proof-of-life, then hands the remaining 5 back to Phase 11. Compromise — confirms repair works end-to-end but doesn't claim REPAIR-01 closure. | |

**User's choice:** Wallet repair + bundled 11-02 six unmute commits
**Notes:** ROADMAP phase description was explicit: "After the wallet repair lands, Plan 11-02 six unmute commits become executable verbatim." Folding the cycle into Phase 12 keeps the closure of REPAIR-01 single-phased and avoids cross-phase resume coordination. Plan 11-02's locked spec is reused unmodified (canonical-first order, per-test commit cycle, CD-1 body shape).

---

## Regression coverage

| Option | Description | Selected |
|--------|-------------|----------|
| No new unit test — rely on full_round.rs | The unmuted full_round tests exercise the signing path end-to-end with real bitcoind. A wallet-level unit test would need fixture PSBT setup and would duplicate coverage. Matches Phase 11's D-04 reasoning. | ✓ |
| Add a focused unit test in `client/src/wallet.rs` | Add `#[test] fn sign_psbt_input_signs_segwit_input` — construct a wallet, build a minimal PSBT, call `sign_psbt_input`, assert a partial_sig is produced. Catches future bdk_wallet API drift without requiring bitcoind. ~30-50 LOC test fixture. | |
| Add an inline doc comment + assert in the function body | No new test — instead document the `trust_witness_utxo: true` choice with a long rationale comment, plus a `debug_assert!` that `witness_utxo.value` matches `self.utxo_value_sats`. Lightweight defensive check at the actual fix locus. | |

**User's choice:** No new unit test — rely on full_round.rs
**Notes:** Symmetric with Phase 11's D-04 reasoning (full_round suite is the canonical end-to-end coverage). A wallet-level fixture would duplicate integration coverage at the cost of mock PSBT setup. The inline `debug_assert!` option was attractive as defensive coding, but it would assert a tautology in the current code (witness_utxo.value is constructed from self.utxo_value_sats on the line immediately above) — the assertion would prevent a regression that the code shape already prevents.

---

## In-source rationale

| Option | Description | Selected |
|--------|-------------|----------|
| Multi-line block comment above the `sign()` call | 5-10 line comment explaining (1) why bdk 2.3 demands `non_witness_utxo` by default — BIP-143 fee-spoof mitigation; (2) why `trust_witness_utxo: true` is safe here — `witness_utxo` populated from trusted regtest RPC; (3) what would change this — mainnet enablement or untrusted PSBT inputs. | ✓ |
| Single-line comment + reference to 11-02-SUMMARY | Terse: `// bdk 2.3 demands non_witness_utxo with trust_witness_utxo: false; safe here — see .planning/phases/11-.../11-02-SUMMARY.md §Two-minimal-repair-candidates`. Keeps the file small; relies on planning archive for context. | |
| Both — block comment + a TODO marker for mainnet revisit | Block comment as above, plus `// TODO(mainnet): switch to Option B (populate non_witness_utxo from RPC) before clearing the BLINDJOIN_ALLOW_CLEARNET refuse-on-mainnet gate`. Surfaces the future decision at the code site. | |

**User's choice:** Multi-line block comment above the `sign()` call
**Notes:** The safety contract is too important to leave to a 1-line cite — anyone touching this method later needs the threat model in front of them. The TODO(mainnet) marker was tempting but in-source TODOs for distant future work go stale and become noise (well-known anti-pattern); the mainnet revisit is recorded in CONTEXT.md "Deferred Ideas" instead, which is the project's canonical "what to revisit before mainnet" store. The block comment will cross-reference 11-02-SUMMARY for the full diagnostic walkthrough so it doesn't duplicate that history in-source.

---

## Claude's Discretion

- **CD-1:** Exact commit message wording for the wallet-repair commit.
- **CD-2:** Whether to use the `$(brew --prefix)` form literally in commit bodies or substitute the resolved path.
- **CD-3:** N/A here (a unit-test commit was rejected by D-06).
- **CD-4:** Capture `bitcoind --version | head -1` output in commit bodies alongside cargo verdict line.

## Deferred Ideas

- Option B (populate `non_witness_utxo` from RPC) — required path before mainnet enablement; rethink wallet/RPC boundary at mainnet design time.
- Wallet-level unit test for `sign_psbt_input` — reconsider if future bdk_wallet bump silently flips signing semantics.
- Wallet-level integration test (`tests/integration/wallet_signing.rs`) — reconsider if a real driver appears.
- Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64` (inherited from Phase 11).
- `-txindex=1` in Phase 9-02's `bootstrap_regtest_bitcoind` (inherited from Phase 11).
- v1.3 ship notes / `/gsd-complete-milestone v1.3` — after combined Phase 11+12 PR is observed green in CI.
- `// TODO(mainnet):` markers in wallet code (rejected per D-09 — planning archive is the canonical store).
- Workspace grep sweep for other `SignOptions::default()` consumers (30-second follow-up if any other consumer suspected).
