# Phase 11: coordinator RSA pubkey encoding + full_round.rs unmute completion - Context

**Gathered:** 2026-05-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 11 delivers two coupled fixes that together close out Phase 10's blocked Task 3:

1. **Repair the coordinator↔client RSA-pubkey handshake.** The coordinator emits RFC-9474 SPKI bytes (`PublicKey::to_spki()`), but the client decodes via `BjPublicKey::from_der` — which only accepts generic `rsaEncryption`-OID SubjectPublicKeyInfo or PKCS#1, neither of which matches the PSS-flavored SPKI the coordinator produces. Fix the asymmetry on the client side so the wire format and D-02 hash-commitment domain (SHA-256 of SPKI bytes) are preserved.

2. **Complete the Phase 10 Task 3 unmute cycle.** Remove the 6 `#[ignore = "TODO(Phase-10): RPC schema drift…"]` markers in `tests/integration/full_round.rs` (lines 164, 462, 730, 854, 911, 1236), driving all 8 full_round tests to PASS locally against pinned brew bitcoind v31. The Phase 10 fixes (Fix A at d99b3a4, Fix WIF-D at e02ce55) plus the Phase 11 RSA repair clear the three known blockers; the suite should now be end-to-end green.

**Net effect:** REPAIR-01 closes when Phase 11 lands locally green; REPAIR-02 closes when the Phase 11 PR is observed green in CI.

**Not in scope:** New protocol versioning, bilateral wire-format changes, mainnet enablement, retiring full_round tests under D-10, modifying Phase 9-02's `bootstrap_regtest_bitcoind` or its `-txindex` story, the v1.3 ship notes (separate follow-up).

</domain>

<decisions>
## Implementation Decisions

### RSA Pubkey Encoding Repair

- **D-01:** Fix locus is **client-side decode**. Change `BjPublicKey::from_der(&pk_der)` to `BjPublicKey::from_spki(&pk_der)` at [client/src/round/input.rs:40](client/src/round/input.rs:40). This is a one-line change; coordinator wire format and the SHA-256-over-SPKI-bytes hash-commitment domain stay exactly as specified by D-02 (Phase 1). No coordinator code touched; no protocol drift.
- **D-02:** **No bilateral wire-format change.** Adding a versioned `rsa_pubkey_spki_b64` field is rejected — there's no second consumer that would benefit, and protocol-shape additions are heavier than the bug warrants. The existing `rsa_pubkey_der_b64` field name keeps its current contents (SPKI bytes); the field name is mildly misleading but renaming it is out of scope for this fix.

### Regression Coverage

- **D-03:** Add a focused **roundtrip unit test in [coordinator/src/blind/rsa.rs](coordinator/src/blind/rsa.rs)**, co-located with the existing 4 RSA tests. The test must:
  1. Generate an `RsaBlindSigner`
  2. Call `signer.public_key_spki_der()` (the production emit path)
  3. Compute `SHA-256` over the emitted bytes
  4. Re-parse via `BjPublicKey::from_spki` (the production client decode path)
  5. Assert the recomputed hash matches and the re-parsed key successfully blinds a test message that the original signer can blind-sign
  Catches future format drift in either direction (blind-rsa-signatures bumps, coordinator emit changes, client decode changes) without requiring bitcoind.
- **D-04:** **No new integration-test file** (`tests/integration/encoding_roundtrip.rs` rejected). The full_round suite, once unmuted, is the end-to-end coverage; a unit test gives faster CI signal and isolates the failure mode if drift recurs. Belt-and-braces option (both layers) rejected as over-engineering for the actual fault surface (one re-decode line).

### Unmute Cycle Discipline

- **D-05:** **Strict per-test commit cycle (×6)** per Phase 10's D-07. One atomic commit per unmute, in canonical-first order:
  1. `full_round_three_clients` (line 164) — **canonical happy path, unmute FIRST**
  2. Remaining 5 in file order: lines 462, 730, 854, 911, 1236
  Rationale for canonical-first: if the happy path passes, the blame/restart paths almost certainly will too; if it fails, the next blocker surfaces against the simplest test (best diagnostic signal). After the first test goes green, the remaining 5 proceed in file order with no re-ordering judgment.
- **D-06:** **No batch removal.** Single-commit-removes-all-6 was rejected — it loses per-test bisect anchors and weakens D-11's escape-valve gate. Six bisectable commits are worth the slightly longer turn.
- **D-07:** Each unmute commit must contain only:
  - The `#[ignore = …]` line removed from that one test
  - Local PASS proof captured in the commit body (one-line `cargo test` invocation + verdict line)
  - No drive-by edits to other tests, helpers, or unrelated code

### Escape-Valve Budget

- **D-08:** **Strict D-11 — halt and surface.** If a 4th orthogonal blocker appears during the unmute cycle and affects ≥1 test, the executor halts after the first encounter and emits a checkpoint with the failure mode and a proposed minimal repair. Pre-authorized in-flight scope expansion is **zero**. Rationale: Phase 10 already absorbed three orthogonal Fix-then-resume blockers (vout-after-mine, bdk 2.3 wallet API, RSA SPKI); a 4th deserves an explicit user decision rather than executor judgment, regardless of perceived smallness.
- **D-09:** A Phase 12 (if needed) absorbs any newly-discovered blocker and any non-unmute scope that surfaces. Phase 11 is execution-only of the locked plan: RSA fix + roundtrip unit test + 6 unmute commits.

### Closure Scope

- **D-10:** **Phase 11 closes REPAIR-01.** Locally green full_round suite + 6 unmute commits landed = REPAIR-01 satisfied; mark `[x]` in REQUIREMENTS.md and update ROADMAP.md Phase 10/11 status accordingly.
- **D-11:** **REPAIR-02 closes on the Phase 11 PR.** Same observation pattern Phase 10 established: the corepc-node feature-pin CI gate must be observed green on the PR before REPAIR-02 flips to `[x]`. Phase 11's executor does NOT self-attest REPAIR-02; it lands the code, the PR closes the requirement.
- **D-12:** v1.3 ship notes / `gsd-complete-milestone` are **not** in scope for Phase 11. Either a tiny wrap-up phase or `/gsd-ship` handles those after CI confirms the PR.

### Claude's Discretion

- **CD-1:** Diagnostic detail in commit bodies (one-line verdict vs full test output). Default to a minimal stamp: `cargo test --test integration full_round::<name> -- --ignored` invocation + the PASS line + a SHA reference to the RSA fix commit. Full output not required.
- **CD-2:** Whether the RSA fix commit precedes the unit-test commit or they collapse into one commit. Default: **two commits** — `fix(11): switch client RSA pubkey decode to from_spki (SPKI-symmetric with coordinator emit)` then `test(11): add SPKI handshake roundtrip in coordinator/src/blind/rsa.rs`. Bisect cleanliness > commit count.
- **CD-3:** Whether to update the (mildly misleading) field name `rsa_pubkey_der_b64` to `rsa_pubkey_spki_b64`. Default: **leave it.** Rename is a wire-format change disguised as a refactor; out of scope per D-02.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 10 carry-over (the trigger for Phase 11)
- [.planning/phases/10-full-round-rs-decision-execution/10-02-SUMMARY.md](.planning/phases/10-full-round-rs-decision-execution/10-02-SUMMARY.md) — Records the third-blocker discovery (RSA pubkey encoding mismatch at [client/src/round/input.rs:41](client/src/round/input.rs:41)) plus Fix A and Fix WIF-D context. Read §"Issues Encountered" and §"Decisions Made" — the diagnostic walkthrough that produced Phase 11.
- [.planning/phases/10-full-round-rs-decision-execution/10-CONTEXT.md](.planning/phases/10-full-round-rs-decision-execution/10-CONTEXT.md) — D-01 through D-11 establish the cycle/escape-valve discipline Phase 11 inherits.
- [.planning/phases/10-full-round-rs-decision-execution/10-02-PLAN.md](.planning/phases/10-full-round-rs-decision-execution/10-02-PLAN.md) — Task 3 design (per-test unmute cycle) is the template Phase 11 completes.

### Protocol & crypto (D-02 anchor)
- [shared/src/protocol.rs](shared/src/protocol.rs) §`InfoResponse` (line 10-26) — the `/info` schema documenting `rsa_pubkey_der_b64` as base64 DER SubjectPublicKeyInfo and the hash-commitment invariant (`SHA-256(decode(rsa_pubkey_der_b64)) == rsa_pubkey_hash`).
- [coordinator/src/blind/rsa.rs](coordinator/src/blind/rsa.rs) — `RsaBlindSigner::public_key_spki_der` (the emit path Phase 11 is symmetric with) and `public_key_hash` (the commitment).
- [client/src/round/input.rs](client/src/round/input.rs) lines 23-41 — the exact decode site to repair.

### Test infrastructure (Phase 9/10 carry-over)
- [tests/integration/mod.rs](tests/integration/mod.rs) — `require_bitcoind!`, `BitcoindGuard`, `fund_regtest`, `FundedSetup`. Phase 11 does NOT modify these; it depends on the post-Fix-A version.
- [tests/integration/full_round.rs](tests/integration/full_round.rs) — the 6 `#[ignore]` sites at lines 164, 462, 730, 854, 911, 1236.
- [CONTRIBUTING.md](CONTRIBUTING.md) §"Running integration tests" — the canonical local invocation (Phase 9-05's deliverable). Phase 11 must follow this pattern for the PASS-proof captures in commit bodies.

### Project ground truth
- [.planning/ROADMAP.md](.planning/ROADMAP.md) §"Phase 11" — phase goal stub Phase 11 fills in.
- [.planning/REQUIREMENTS.md](.planning/REQUIREMENTS.md) §REPAIR-01, §REPAIR-02 — closure criteria Phase 11 targets.
- [.planning/STATE.md](.planning/STATE.md) — current resume pointer.
- [CLAUDE.md](CLAUDE.md) — recommended stack (blind-rsa-signatures jedisct1, bdk_wallet 2.3, corepc-node feature-pinned).

### Library reference (read on demand)
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/blind-rsa-signatures-0.17.1/src/lib.rs` lines 654-731 — the `from_der` / `from_spki` / `to_spki` implementations that document the asymmetry Phase 11 repairs. Researcher should re-confirm against the pinned version before planning.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`RsaBlindSigner::public_key_spki_der()`** ([coordinator/src/blind/rsa.rs:65](coordinator/src/blind/rsa.rs:65)) — the production emit path the new unit test will exercise. No new helper needed.
- **`BjPublicKey::from_spki()`** (blind-rsa-signatures 0.17.1, lib.rs:705) — the symmetric decode method the client switches to. Already exists; no shim required.
- **`require_bitcoind!`, `BitcoindGuard`, `fund_regtest`, `FundedSetup`** ([tests/integration/mod.rs](tests/integration/mod.rs)) — Phase 9/10 shared fixtures the unmuted tests rely on. Phase 11 must NOT modify them.
- **CI `corepc-node-feature-pin-check` job** ([.github/workflows/ci.yml](.github/workflows/ci.yml)) — the gate Phase 11 PR observation closes REPAIR-02 against. Already landed at commit 4026f50; Phase 11 only needs the PR to remain green.

### Established Patterns
- **Per-test commit cycle with PASS proof in body** (Phase 10 D-07) — Phase 11's 6 unmute commits each follow this; commit body captures the exact `cargo test … -- --ignored` invocation + verdict line.
- **SHA-pinned `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4.3.1`** in `.github/workflows/ci.yml` — Phase 11 makes zero CI changes; this pattern stays untouched.
- **D-02 hash commitment** (Phase 1) — coordinator publishes `SHA-256(SPKI_bytes)` on `/info`; client recomputes and verifies BEFORE blinding. Phase 11 preserves this contract bit-for-bit.
- **D-07 unit-test convention** in `coordinator/src/blind/rsa.rs:71-127` — 4 existing tests (`blind_sign_round_trip`, `public_key_hash_is_32_bytes`, `public_key_hash_is_deterministic`, `unlinkability_two_tokens`). New roundtrip test (D-03) follows the same `#[test] fn …` shape, no async runtime.

### Integration Points
- **Client decode site** — [client/src/round/input.rs:40](client/src/round/input.rs:40) inside `register_input`. Single-line change; surrounding hash-verify logic (lines 29-38) is unaffected because both sides hash the same bytes.
- **Coordinator emit site** — [coordinator/src/blind/rsa.rs:65](coordinator/src/blind/rsa.rs:65) `public_key_spki_der()`. Unchanged in Phase 11; explicitly preserved as the contract anchor.
- **`/info` HTTP handler** — [coordinator/src/api/handlers.rs:49-65](coordinator/src/api/handlers.rs:49) reads `state.rsa_pubkey_der` (set by [coordinator/src/round/manager.rs:59](coordinator/src/round/manager.rs:59) from `public_key_spki_der()`). Phase 11 does NOT modify this path.
- **Unmute sites** — `tests/integration/full_round.rs:{164, 462, 730, 854, 911, 1236}`. Each `#[ignore = "…"]` line is the only edit per test; no test-body changes (those would be a different phase if needed).

</code_context>

<specifics>
## Specific Ideas

- **Canonical-first unmute order is non-negotiable.** `full_round_three_clients` (line 164) is the happy-path canonical test; it must be the FIRST unmute. If it fails, planning the remaining 5 is moot until the next blocker is surfaced. If it passes, the remaining 5 in file order is mechanical.
- **One-line client fix is the entire RSA repair.** `from_der` → `from_spki` at [client/src/round/input.rs:40](client/src/round/input.rs:40). The hash-verify block at lines 29-38 stays exactly as written — it already correctly hashes the bytes the coordinator sends (which are SPKI bytes both before and after the fix).
- **Commit naming follows Phase 10's convention:** `fix(11): …`, `test(11): …`, `test(11): unmute full_round_three_clients (Phase-10 carve-out 1/6)` etc. The PASS verdict line goes in the commit body, not the subject.
- **Use the brew bitcoind v31 BITCOIND_EXE invocation** that Phase 9-05 documented in CONTRIBUTING.md. Don't invent a new test invocation — the one that produced the Phase 10 blocker is the same one Phase 11 must demonstrate green.

</specifics>

<deferred>
## Deferred Ideas

- **Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64`** for accuracy. Pure wire-format change disguised as a refactor. If touched at all, belongs in a dedicated "protocol field naming pass" phase — explicitly out of Phase 11 (D-02 / CD-3).
- **Bilateral protocol versioning of the pubkey field.** Adding `rsa_pubkey_spki_b64` alongside the existing field. No second consumer exists today; deferred until a real driver appears.
- **Switching coordinator emit from `to_spki()` to `to_der()`** (PKCS#1). Would invert the wire-format choice and force re-deciding the D-02 commitment domain. Substantively a protocol change; out of scope.
- **Adding a `tests/integration/encoding_roundtrip.rs` integration test** that exercises the actual `/info` HTTP path. Real driver only emerges if the unit test (D-03) misses a future drift — file the gap as a follow-up if/when it happens.
- **`-txindex=1` for bitcoind in Phase 9-02's `bootstrap_regtest_bitcoind`** (Fix B from Phase 10's checkpoint). Phase 10's Fix A made this unnecessary; if a future test needs txindex it gets its own scoped change.
- **Investigating whether any other RPC path in the workspace also expects PSS-flavored SPKI** vs generic `rsaEncryption` OID. Phase 11 fixes only the known site at `client/src/round/input.rs:40`. A grep sweep for `from_der.*pk_der` is a 30-second follow-up if any other consumer is suspected, but no other consumers are known today.
- **v1.3 ship notes and `/gsd-complete-milestone v1.3`.** Belongs after CI confirms the Phase 11 PR green. Either a Phase 12 stub or direct `/gsd-ship` — user decides at that point.

</deferred>

---

*Phase: 11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet*
*Context gathered: 2026-05-28*
