# Phase 11: coordinator RSA pubkey encoding + full_round.rs unmute completion - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-28
**Phase:** 11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet
**Areas discussed:** Fix locus, Unmute cycle, Regression test, Closure scope, Unmute order, Test home, Escape budget

---

## Fix locus — Where to repair the RSA-pubkey encoding mismatch

| Option | Description | Selected |
|--------|-------------|----------|
| Client decode → from_spki | 1-line change at client/src/round/input.rs:40: BjPublicKey::from_der → from_spki. Coordinator wire format and SHA-256 hash domain (over SPKI bytes) stay exactly as D-02 specified. Smallest surface area, no protocol drift. | ✓ |
| Coordinator emit → to_der | Change coordinator/src/blind/rsa.rs:65 to emit generic public-key DER instead of PSS-flavored SPKI. Client's existing from_der starts working, but hash-commitment domain shifts from SPKI → PKCS#1 — that's a wire-format and D-02 re-decision. | |
| Bilateral versioned field | Add rsa_pubkey_spki_b64 alongside rsa_pubkey_der_b64 in /info; client prefers spki when present. Heavier; warranted only if you anticipate multiple format consumers. Adds protocol shape we don't yet need. | |

**User's choice:** Client decode → from_spki (Recommended)
**Notes:** Preserves D-02's wire format and hash-commitment domain exactly. One-line change at [client/src/round/input.rs:40](client/src/round/input.rs:40). The existing field name `rsa_pubkey_der_b64` stays as-is (mildly misleading but renaming is its own change).

---

## Unmute cycle — Strictness of the per-test commit discipline

| Option | Description | Selected |
|--------|-------------|----------|
| Strict per-test | 6 atomic commits, one per test, in the order they appear in full_round.rs (lines 164, 462, 730, 854, 911, 1236). Each commit is bisectable. D-11 escape valve stays capped at 1 retirement — if a 4th orthogonal blocker appears, halt and surface. | ✓ |
| Single batch commit | One commit removes all 6 #[ignore] markers + verifies all 8 tests green in a single cargo test invocation. Faster, but loses per-test bisect anchors and weakens D-11's escape-valve gate. | |
| Hybrid: 1 verification commit then batch removal | Land the RSA fix, run all 8 tests green locally as proof-of-life, then a single commit removes the 6 #[ignore] markers with the proof-of-life output captured in the commit body. | |

**User's choice:** Strict per-test (Recommended)
**Notes:** Inherits Phase 10 D-07 directly. 6 atomic commits, each carrying a PASS proof in its body. Six bisectable anchors justify the slightly longer turnaround.

---

## Regression test — Coverage strategy for the encoding fix

| Option | Description | Selected |
|--------|-------------|----------|
| Focused unit roundtrip | Add a #[test] that does coordinator-side to_spki → SHA-256 commitment → client-side from_spki + recomputed-hash-match, asserting the full handshake invariant inside one file. Catches future format drift without needing bitcoind. | ✓ |
| Rely on full_round suite | No new unit test. Once the 6 #[ignore]s are removed and the 8 tests pass against pinned bitcoind, the encoding path is covered end-to-end. Fast to deliver; weaker signal on what specifically broke if drift recurs. | |
| Both — unit roundtrip AND full_round green | Belt-and-braces: unit test for fast feedback in CI, full_round suite for integration coverage. Slightly more work; gives clean failure isolation if a future blind-rsa-signatures version changes either side. | |

**User's choice:** Focused unit roundtrip (Recommended)
**Notes:** No bitcoind dependency; fast CI signal; isolates failure mode if blind-rsa-signatures bumps shift either side's format.

---

## Closure scope — What does Phase 11 close vs hand off

| Option | Description | Selected |
|--------|-------------|----------|
| Close REPAIR-01 here; REPAIR-02 closes on PR | Phase 11 closes REPAIR-01 when all 8 full_round tests pass locally + the unmute commits land. REPAIR-02 (the corepc-node feature-pin CI gate) closes when the Phase 11 PR is observed green in CI — same pattern Phase 10 already established. | ✓ |
| Close both in Phase 11 | Treat the PR landing this phase as the REPAIR-02 closure observation; Phase 11 self-attests both REPAIR-01 and REPAIR-02 once CI on the PR returns green. Slightly slimmer state ledger. | |
| Defer all closure to a wrap-up phase | Phase 11 delivers the code; a tiny Phase 12 closes REPAIR-01 + REPAIR-02 + writes the v1.3 ship notes. Clean separation; more ROADMAP overhead for what's effectively a checkbox phase. | |

**User's choice:** Close REPAIR-01 here; REPAIR-02 closes on PR (Recommended)
**Notes:** Same observation pattern Phase 10 established. Executor closes REPAIR-01 inline; REPAIR-02 closes on PR CI green, not on executor self-attestation.

---

## Unmute order — Sequencing the 6 #[ignore] removals

| Option | Description | Selected |
|--------|-------------|----------|
| Canonical-first | Start with full_round_three_clients (line 164) — the happy-path canonical test. If that passes, the other 5 (blame paths, restart, recovery) almost certainly will too; if it fails, you've found the next blocker fast. Remaining 5 in file order. | ✓ |
| Strict file order | Lines 164, 462, 730, 854, 911, 1236 — in the order grep prints them. Mechanical, no judgment. Matches what Phase 10 Task 3's plan implied. | |
| Cheap-first — by test complexity | Sort by lines-of-test-body, unmute shortest first. Optimizes for fastest green-light on the simplest paths. Useful if you expect more flakiness; less useful if the suite is sound. | |

**User's choice:** Canonical-first (Recommended)
**Notes:** `full_round_three_clients` at line 164 must be first — it's the strongest diagnostic signal for any remaining blocker, and a green there strongly predicts the other 5 will also be green.

---

## Test home — Where the focused RSA-pubkey roundtrip unit test lives

| Option | Description | Selected |
|--------|-------------|----------|
| coordinator/src/blind/rsa.rs | Co-locate with the existing 4 RSA tests (blind_sign_round_trip, public_key_hash_*). The new test calls signer.to_spki() then asserts BjPublicKey::from_spki round-trips with a hash-commitment check. Coordinator already owns the keypair lifecycle; client side is a single re-decode line that the test exercises symmetrically. | ✓ |
| shared/ (new module) | Create shared/src/blind_handshake.rs and put the roundtrip test there. Makes the handshake invariant a first-class shared concern rather than a coordinator-internal one. Slightly heavier scaffolding. | |
| tests/integration/encoding_roundtrip.rs | New integration-test file that exercises the real /info response path — coordinator emits, HTTP client receives, client-side parse succeeds. No bitcoind required (only round/state.rs). Highest fidelity to the actual failure mode; more setup than a unit test. | |

**User's choice:** coordinator/src/blind/rsa.rs (Recommended)
**Notes:** Co-located with the existing 4 RSA unit tests. Keeps scaffolding minimal; symmetric coverage in one file.

---

## Escape budget — D-11 escape-valve budget for Phase 11

| Option | Description | Selected |
|--------|-------------|----------|
| Strict D-11 — halt and surface | If a single new blocker affects ≥1 of the 6 tests, executor halts after the first encounter and surfaces a checkpoint — same as Phase 10 Task 3. Pre-authorize zero in-flight scope expansion; Phase 12 absorbs any newly-discovered blockers. | ✓ |
| Pre-auth one Fix-then-resume | If exactly one orthogonal blocker appears, executor may apply a minimal repair (analogous to Fix A / Fix WIF-D in Phase 10) without checkpointing, then resume the unmute cycle. >1 blocker still triggers D-11 halt. Faster turnaround; relies on executor judgment for 'minimal'. | |
| Pre-auth Fix-then-resume up to 2 | Same as above but up to 2 minimal repairs allowed before D-11 halt. Matches the pattern Phase 10 actually exhibited (Fix A + Fix WIF-D landed in-flight). Most pragmatic; weakest escape-valve discipline. | |

**User's choice:** Strict D-11 — halt and surface (Recommended)
**Notes:** Phase 10 already absorbed three orthogonal Fix-then-resume blockers; a 4th deserves an explicit user decision. Phase 12 (if needed) absorbs new blockers and scope.

---

## Claude's Discretion

- **CD-1:** Diagnostic detail captured in each unmute commit body. Default: minimal stamp — `cargo test --test integration full_round::<name> -- --ignored` invocation + PASS verdict line + SHA reference to the RSA fix commit. Full output not required.
- **CD-2:** Whether the RSA fix commit precedes the unit-test commit or they collapse. Default: two commits — `fix(11): switch client RSA pubkey decode to from_spki` then `test(11): add SPKI handshake roundtrip in coordinator/src/blind/rsa.rs`. Bisect cleanliness > commit count.
- **CD-3:** Whether to rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64` for accuracy. Default: leave it. Rename is a wire-format change disguised as a refactor; out of scope.

## Deferred Ideas

- Rename `rsa_pubkey_der_b64` → `rsa_pubkey_spki_b64` (own phase if pursued)
- Bilateral protocol versioning of the pubkey field
- Switching coordinator emit from `to_spki()` to `to_der()` (substantive protocol change)
- `tests/integration/encoding_roundtrip.rs` as an HTTP-path-level integration test
- `-txindex=1` for bitcoind in Phase 9-02's `bootstrap_regtest_bitcoind` (Fix B from Phase 10 — no longer needed after Fix A)
- Workspace-wide grep sweep for other `BjPublicKey::from_der` consumers that might also expect SPKI
- v1.3 ship notes / `/gsd-complete-milestone v1.3` (post-Phase-11 follow-up)
