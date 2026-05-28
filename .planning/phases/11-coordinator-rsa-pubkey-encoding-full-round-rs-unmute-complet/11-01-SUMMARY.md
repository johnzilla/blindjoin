---
phase: 11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet
plan: 01
subsystem: blind-signatures
tags:
  - rsa
  - blind-signatures
  - rfc-9474
  - spki
  - client-decode
  - coordinator-emit
  - hash-commitment
requires:
  - "blind-rsa-signatures 0.17.1 (BjPublicKey::from_spki / to_spki) — pre-existing"
  - "Phase 1 D-02 hash-commitment invariant: SHA-256(SPKI bytes) == rsa_pubkey_hash"
  - "coordinator/src/blind/rsa.rs::public_key_spki_der() — pre-existing emit path (unchanged)"
  - "coordinator/src/blind/rsa.rs::public_key_hash() — pre-existing commitment (unchanged)"
  - "client/src/round/input.rs hash-verify block at lines 23-38 — pre-existing (preserved byte-identical)"
provides:
  - "Client RSA pubkey decode that is SPKI-symmetric with the coordinator emit path"
  - "Locked-in regression test for SHA-256(SPKI) == public_key_hash() commitment AND the emit/decode roundtrip blind-sign chain"
  - "Bisect anchors for Plan 11-02's six unmute commits (fix SHA cc20f6f, test SHA 13da4b5)"
affects:
  - "client/src/round/input.rs:40 (one-token swap)"
  - "coordinator/src/blind/rsa.rs::tests (one new sync #[test] fn appended)"
tech-stack:
  added: []
  patterns:
    - "Sync co-located unit test in `mod tests` (matches the 4 existing RSA tests)"
    - "Per-fn re-import of `sha2::{Sha256, Digest}` inside the test body (matches client/src/round/input.rs:30-31 pattern)"
    - "Use of the production `BjPublicKey` alias in scope via `use super::*;` — no re-aliasing in test body"
key-files:
  created: []
  modified:
    - client/src/round/input.rs
    - coordinator/src/blind/rsa.rs
decisions:
  - "D-01 fix locus client-side: one-line swap `from_der` → `from_spki` at client/src/round/input.rs:40"
  - "D-02 wire format unchanged: rsa_pubkey_der_b64 keeps SPKI contents; commitment domain stays SHA-256 over SPKI"
  - "D-03 roundtrip regression test colocated with 4 existing RSA tests in coordinator/src/blind/rsa.rs"
  - "D-04 unit test over integration test (faster CI signal, isolates emit-vs-decode drift)"
  - "CD-2 two atomic commits: fix(11) precedes test(11)"
  - "CD-3 no rename of rsa_pubkey_der_b64 (deferred — would be a wire-format change disguised as refactor)"
metrics:
  tasks_completed: 2
  files_modified: 2
  lines_added: 37
  lines_removed: 1
  commits: 2
  duration_seconds: ~1500
  completed: 2026-05-27
---

# Phase 11 Plan 01: Coordinator RSA Pubkey Encoding Repair Summary

Repaired the coordinator↔client RSA pubkey handshake by switching the client-side decode from `BjPublicKey::from_der` to `BjPublicKey::from_spki` (a single-token swap at `client/src/round/input.rs:40`), and locked in regression coverage with a sync unit test `spki_handshake_round_trip` in `coordinator/src/blind/rsa.rs` that asserts both the D-02 SHA-256-over-SPKI hash commitment and the full emit → reparse → blind-sign → finalize → verify chain.

## What Was Built

### Task 1 — `fix(11)` commit `cc20f6f`

One-token swap on a single line at `client/src/round/input.rs:40`:

```rust
-    let pk = BjPublicKey::from_der(&pk_der)
+    let pk = BjPublicKey::from_spki(&pk_der)
         .map_err(|e| anyhow!("Failed to parse coordinator RSA public key: {e}"))?;
```

The coordinator emits its RSA public key via `PublicKey::to_spki()` (PSS-flavored `SubjectPublicKeyInfo`). `BjPublicKey::from_der` only accepts generic `rsaEncryption`-OID SPKI or PKCS#1, neither of which matches what the coordinator produces. `from_spki` is the symmetric inverse of `to_spki`, which is what was needed all along.

The pre-existing hash-verify block at `client/src/round/input.rs:23-38` (T-05-01 mitigation, D-02) is preserved byte-identical — both sides hash the same `pk_der` bytes regardless of which parser consumes them next, so the threat model (tamper-in-flight detection, malicious-coordinator hash mismatch) is unchanged. The variable name `pk_der` is intentionally NOT renamed (CD-3 defers any wire-format-touching rename).

### Task 2 — `test(11)` commit `13da4b5`

Appended one sync `#[test] fn spki_handshake_round_trip` inside the existing `mod tests` block in `coordinator/src/blind/rsa.rs`, placed after `unlinkability_two_tokens`. The test:

1. Generates an `RsaBlindSigner` via `RsaBlindSigner::generate()`.
2. Emits the public key via the production path `signer.public_key_spki_der()`.
3. Asserts the D-02 commitment: `Sha256::digest(&spki) == signer.public_key_hash()`.
4. Re-parses via `BjPublicKey::from_spki(&spki)` — mirrors `client/src/round/input.rs:40`.
5. Exercises the full blind → blind-sign → finalize → verify chain through the re-parsed key.

The test is sync (`#[test]`, not `#[tokio::test]`), uses the in-scope `BjPublicKey` alias and `DefaultRng` import without re-declaring either, re-uses the existing `test_message()` helper, and runs in <1 s without bitcoind.

## Files Touched

| File | Change | Net lines |
|------|--------|-----------|
| `client/src/round/input.rs` | one-token swap on line 40 | +1, −1 |
| `coordinator/src/blind/rsa.rs` | append `fn spki_handshake_round_trip` inside `mod tests`; production body lines 1-68 byte-identical | +36 |

No other files modified. No new dependencies added to any `Cargo.toml`. No protocol-shape change in `shared/src/protocol.rs`. No changes to the coordinator emit path (`coordinator/src/blind/rsa.rs` lines 1-68 are byte-identical to before).

## Key Decisions

- **D-01 fix locus client-side, single token swap.** No coordinator code touched; no protocol drift.
- **D-02 wire format unchanged.** `rsa_pubkey_der_b64` keeps its SPKI contents; commitment domain stays SHA-256 over SPKI bytes. No `rsa_pubkey_spki_b64` field added (no second consumer).
- **D-03 roundtrip regression colocated with existing RSA tests.** Single source of crypto test truth.
- **D-04 unit test over integration test.** Faster CI signal, isolates emit-vs-decode drift if it recurs.
- **CD-2 two atomic commits, fix → test order.** Bisect cleanliness over commit count.
- **CD-3 no rename of `rsa_pubkey_der_b64`.** Would be a wire-format change disguised as a refactor.

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build` (workspace) | green |
| `cargo build -p client` | green |
| `cargo test --lib -p coordinator blind::rsa::` | `test result: ok. 5 passed; 0 failed` |
| `cargo test --lib -p coordinator blind::rsa::tests::spki_handshake_round_trip` | `test result: ok. 1 passed; 0 failed` |
| `git log --oneline -2` | `13da4b5 test(11): …` on top of `cc20f6f fix(11): …` (CD-2 verified) |
| `git diff HEAD~2 -- client/src/round/input.rs` | exactly one changed line (line 40: `from_der` → `from_spki`) |
| `git diff HEAD~2 -- coordinator/src/blind/rsa.rs` | additions only, all inside `mod tests`; lines 1-68 byte-identical |
| `git diff HEAD~2 --stat` | exactly two paths: `client/src/round/input.rs` and `coordinator/src/blind/rsa.rs` |

## Acceptance Criteria

| Criterion | Status |
|-----------|--------|
| `client/src/round/input.rs:40` contains `BjPublicKey::from_spki(&pk_der)` | done |
| Zero occurrences of `BjPublicKey::from_der(&pk_der)` in `client/src/round/input.rs` | done |
| `client/src/round/input.rs` lines 23-38 (hash-verify block) byte-identical to pre-plan | done |
| `coordinator/src/blind/rsa.rs` contains new sync `#[test] fn spki_handshake_round_trip` inside `mod tests`, placed after `unlinkability_two_tokens` | done |
| New test asserts `Sha256::digest(public_key_spki_der()) == public_key_hash()` AND exercises full blind / blind-sign / finalize / verify chain through `BjPublicKey::from_spki`-reparsed key | done |
| `cargo test --lib -p coordinator blind::rsa::` is green (5 passed, 0 failed) | done |
| `cargo build` is green workspace-wide | done |
| Two atomic commits in CD-2 order (`fix(11):` then `test(11):`) | done |
| No other files touched (no `manager.rs`, `handlers.rs`, `protocol.rs`, `full_round.rs`, `ci.yml`, `CONTRIBUTING.md` edits) | done |
| No new dependencies added to any `Cargo.toml` | done |
| REPAIR-01 partially satisfied | done (full closure waits on Plan 11-02's six-test unmute cycle) |

## Deviations from Plan

None. Plan executed exactly as written. The TDD discipline collapsed naturally because Task 1 was a single-token fix whose proof-of-correctness IS Task 2's new unit test (the test was written second per CD-2, but it is what proves the swap is the right swap — and it passes on the first run after both commits land).

## Commit Hashes for Plan 11-02 to Reference

Plan 11-02's six unmute commits should reference these SHAs in their `RSA fix:` commit-body footer (per PATTERNS.md):

- `fix(11)`: **`cc20f6f`** — `fix(11): switch client RSA pubkey decode to from_spki (SPKI-symmetric with coordinator emit)`
- `test(11)`: **`13da4b5`** — `test(11): add SPKI handshake roundtrip in coordinator/src/blind/rsa.rs`

## REPAIR-01 Closure Status

**Partial.** Phase 11 Plan 01 lands the third-blocker repair on the RSA pubkey handshake. REPAIR-01 closes when Plan 11-02's six-test unmute cycle proves the full `full_round.rs` integration suite green locally (8 tests passing under brew bitcoind v31). Plan 11-01 is one half of that closure; Plan 11-02 completes it.

## Threat Surface Scan

No new threat surface introduced. The plan touches only:
- An existing parser call (`from_der` → `from_spki`) inside an existing function that already guards its input with a hash commitment check.
- A new unit test that exercises an already-public API surface via the existing `super::*` import inside `mod tests`.

No new endpoints, no new auth paths, no new file access patterns, no schema changes at trust boundaries. The threat register in `11-01-PLAN.md` already enumerates T-11-01 through T-11-06 (all mitigated or accepted with reason). No new flags to add.

## Self-Check: PASSED

- Files exist:
  - `client/src/round/input.rs` — FOUND (modified, line 40 verified)
  - `coordinator/src/blind/rsa.rs` — FOUND (new test verified)
  - `.planning/phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-01-SUMMARY.md` — FOUND (this file)
- Commits exist:
  - `cc20f6f` — FOUND in `git log`
  - `13da4b5` — FOUND in `git log`
- Tests green:
  - `cargo test --lib -p coordinator blind::rsa::` — 5 passed, 0 failed
  - `cargo build` (workspace) — exit 0
