# Phase 11: coordinator RSA pubkey encoding + full_round.rs unmute completion - Pattern Map

**Mapped:** 2026-05-27
**Files analyzed:** 3 (1 surgical edit + 1 unit-test addition + 6 attribute removals in 1 file)
**Analogs found:** 3 / 3 (all in-file analogs — no cross-file pattern hunt needed)

## Scope Note

Phase 11 is an unusually narrow execution-only phase. All three modification surfaces have **in-file analogs already established by Phase 1 and Phase 10**. No new architectural patterns are introduced. PATTERNS.md exists primarily to:

1. Lock the exact analog lines an executor must mirror.
2. Restate the Phase 10 D-07 commit convention with concrete examples.
3. Enumerate the explicitly preserved boundary files the executor must NOT touch.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `client/src/round/input.rs` (line 40 only) | client-protocol stage | request-response (HTTP + crypto handshake) | `coordinator/src/blind/rsa.rs:65` (the symmetric emit `public_key.to_spki()`) — same file, lines 23-41 (surrounding hash-verify block) | exact (symmetric inverse of coordinator emit) |
| `coordinator/src/blind/rsa.rs` (append one `#[test] fn` to `mod tests` at lines 71-127) | unit test | transform / property-style assertion | Four existing tests in the same `mod tests` block (lines 79-126): `blind_sign_round_trip`, `public_key_hash_is_32_bytes`, `public_key_hash_is_deterministic`, `unlinkability_two_tokens` | exact (co-located, same module) |
| `tests/integration/full_round.rs` (delete 6 `#[ignore]` lines at 164, 462, 730, 854, 911, 1236) | integration-test attribute removal | n/a (attribute-only edit) | The 6 lines themselves are byte-identical; the surrounding `#[tokio::test]` + `async fn …` shape is preserved untouched | exact |

## Pattern Assignments

### Surface 1: `client/src/round/input.rs:40` — one-line decode swap

**Target line (current):**

```rust
let pk = BjPublicKey::from_der(&pk_der)
    .map_err(|e| anyhow!("Failed to parse coordinator RSA public key: {e}"))?;
```

**Target line (after Phase 11):**

```rust
let pk = BjPublicKey::from_spki(&pk_der)
    .map_err(|e| anyhow!("Failed to parse coordinator RSA public key: {e}"))?;
```

**Analog — coordinator emit path** (`coordinator/src/blind/rsa.rs:64-67`):

```rust
/// Export the public key as SPKI DER bytes.
pub fn public_key_spki_der(&self) -> Result<Vec<u8>, blind_rsa_signatures::Error> {
    self.public_key.to_spki()
}
```

**Analog — surrounding hash-verify block that must stay unchanged** (`client/src/round/input.rs:23-41`):

```rust
// 1. Decode and verify coordinator RSA public key (T-05-01 mitigation: D-02)
let pk_der_b64 = info.rsa_pubkey_der_b64.as_ref()
    .ok_or_else(|| anyhow!("Coordinator did not provide RSA public key in /info"))?;
let pk_der = B64.decode(pk_der_b64)?;

// Verify SHA-256(pk_der) == announced rsa_pubkey_hash
let pk_hash_actual: [u8; 32] = {
    use sha2::{Sha256, Digest};
    Sha256::digest(&pk_der).into()
};
let announced_hash = info.rsa_pubkey_hash.as_ref()
    .ok_or_else(|| anyhow!("Coordinator did not announce RSA key hash"))?;
let announced_bytes = hex::decode(announced_hash)?;
if announced_bytes != pk_hash_actual {
    return Err(anyhow!("RSA public key hash mismatch — coordinator key commitment violated"));
}

let pk = BjPublicKey::from_der(&pk_der)   // <-- only this token changes: from_der → from_spki
    .map_err(|e| anyhow!("Failed to parse coordinator RSA public key: {e}"))?;
```

**Pattern signature the executor must match:**
- Single-token swap: `from_der` → `from_spki`. Nothing else on line 40-41.
- The variable name `pk_der` is intentionally NOT renamed (CD-3 defers the field/var rename).
- The `.map_err` chain and error message stay byte-identical.
- The hash-verify block at lines 29-38 is **load-bearing and must not be touched** — both sides hash the same `pk_der` bytes regardless of which parser consumes them next.

---

### Surface 2: `coordinator/src/blind/rsa.rs` — append SPKI roundtrip unit test

**Target:** append a 5th `#[test] fn` inside `mod tests` at the bottom of the existing block (after line 126, before the closing `}` at line 127). Per D-03 it must:
1. Generate an `RsaBlindSigner`
2. Call `signer.public_key_spki_der()` (production emit path)
3. Compute SHA-256 over the emitted bytes
4. Re-parse via `BjPublicKey::from_spki` (production client decode path)
5. Assert the recomputed hash matches and the re-parsed key successfully blinds a test message that the original signer can blind-sign

**Analog — closest existing test in the same block** (`coordinator/src/blind/rsa.rs:79-93`):

```rust
#[test]
fn blind_sign_round_trip() {
    let signer = RsaBlindSigner::generate().unwrap();
    let pk = &signer.public_key;

    let msg = test_message();
    // Client blinds the message
    let blinding_result = pk.blind(&mut DefaultRng, &msg).unwrap();
    // Coordinator blind-signs (never sees msg)
    let blind_sig = signer.blind_sign(&blinding_result.blind_message).unwrap();
    // Client unblinds + verifies (finalize also verifies internally)
    let sig = pk.finalize(&blind_sig, &blinding_result, &msg).unwrap();
    // Explicit verify: signature on msg is valid under public key
    pk.verify(&sig, blinding_result.msg_randomizer, &msg).unwrap();
}
```

**Analog — hash determinism shape** (`coordinator/src/blind/rsa.rs:102-106`):

```rust
#[test]
fn public_key_hash_is_deterministic() {
    let signer = RsaBlindSigner::generate().unwrap();
    assert_eq!(signer.public_key_hash(), signer.public_key_hash());
}
```

**Pattern signature the executor must match:**
- Plain `#[test]` attribute — **not** `#[tokio::test]`. The `mod tests` block is sync-only; no async runtime here. (Confirmed: all 4 existing tests are sync.)
- Use the already-imported `super::*` and `blind_rsa_signatures::DefaultRng` (lines 72-73). Do NOT add new use-statements at module scope; add per-test `use sha2::{Sha256, Digest};` only if the helper isn't already in scope (the file already imports `sha2::{Sha256, Digest}` at line 5, but `mod tests` does not auto-inherit non-`super::*` items — re-import inside the fn body, matching the in-file pattern at lines 30-31 of `client/src/round/input.rs`).
- Re-use the existing `test_message()` helper at lines 75-77 rather than introducing a new message constant.
- `.unwrap()` on all `Result`s — consistent with the surrounding 4 tests; no `?` operator, no `anyhow`.
- Assertions: `assert_eq!` for the hash equality; the message round-trip via `pk_reparsed.blind(...)` → `signer.blind_sign(...)` → `pk_reparsed.finalize(...)` mirrors `blind_sign_round_trip` exactly.
- Name suggestion (executor's CD): `spki_handshake_round_trip` or `client_decode_matches_coordinator_emit` — consistent with the snake_case descriptive style of the surrounding 4 tests.

**Skeleton (illustrative; executor adapts to compile):**

```rust
#[test]
fn spki_handshake_round_trip() {
    use sha2::{Sha256, Digest};

    let signer = RsaBlindSigner::generate().unwrap();

    // 1+2. Emit via production path
    let spki = signer.public_key_spki_der().unwrap();

    // 3. Hash matches the public_key_hash() commitment
    let hash_via_emit: [u8; 32] = Sha256::digest(&spki).into();
    assert_eq!(hash_via_emit, signer.public_key_hash(),
        "SHA-256(public_key_spki_der()) must equal public_key_hash()");

    // 4. Re-parse via the production client decode path
    let pk_reparsed = BjPublicKey::from_spki(&spki).unwrap();

    // 5. Re-parsed key blinds a message the original signer can blind-sign
    let msg = test_message();
    let blinding_result = pk_reparsed.blind(&mut DefaultRng, &msg).unwrap();
    let blind_sig = signer.blind_sign(&blinding_result.blind_message).unwrap();
    let sig = pk_reparsed.finalize(&blind_sig, &blinding_result, &msg).unwrap();
    pk_reparsed.verify(&sig, blinding_result.msg_randomizer, &msg).unwrap();
}
```

---

### Surface 3: `tests/integration/full_round.rs` — six `#[ignore]` line removals

**Target lines (each is a single-line attribute deletion; the line below — `async fn …` — is preserved unchanged):**

| File line | Test fn | Unmute order (D-05) |
|-----------|---------|---------------------|
| 164 | `full_round_three_clients` | **1st (canonical-first, mandatory)** |
| 462 | `blame_non_signer_timeout` | 2nd |
| 730 | `adversarial_replay_token` | 3rd |
| 854 | `adversarial_invalid_utxo` | 4th |
| 911 | `adversarial_wrong_denomination` | 5th |
| 1236 | `round_restart_and_completion_after_blame` | 6th |

**Analog — exact attribute pair the executor edits** (`tests/integration/full_round.rs:163-165`, identical shape at all six sites):

```rust
#[tokio::test]
#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction -- see TODO.md"]   // <-- DELETE THIS LINE
async fn full_round_three_clients() {
```

**Pattern signature the executor must match:**
- Delete the entire `#[ignore = "…"]` line. Leave `#[tokio::test]` (line above) and `async fn …` (line below) untouched.
- No trailing whitespace, no comment substitution. The line is fully removed, not commented out.
- No test-body edits. Per D-07: "no drive-by edits to other tests, helpers, or unrelated code."

---

## Shared Patterns

### Phase 10 D-07 commit-message convention (applies to all 8 Phase 11 commits)

**Source pattern** — Phase 10 commit history (verified via `git log --oneline`):

| Commit SHA | Subject | Notes |
|-----------|---------|-------|
| `e02ce55` | `fix(10): switch ClientWallet::from_wif to Wallet::create_single (bdk 2.3)` | Pattern for `fix(11): …` |
| `d99b3a4` | `refactor(10-01): read vouts before mining confirmation block` | Behavior-changing repair in test infra |
| `4026f50` | `ci(10-02): add corepc-node feature pin check; correct 15→8 tests doc count` | Scope-tagged subject |
| `83a65cc` | `docs(10-02): record Fix A + Fix WIF-D + third blocker discovery` | Phase-summary docs commit |

**Apply to:** all 8 Phase 11 commits.

**Commit 1 (RSA fix):** `fix(11): switch client RSA pubkey decode to from_spki (SPKI-symmetric with coordinator emit)`

**Commit 2 (unit test):** `test(11): add SPKI handshake roundtrip in coordinator/src/blind/rsa.rs`

**Commits 3-8 (six unmute commits, canonical-first order from D-05):**

```
test(11): unmute full_round_three_clients (Phase-10 carve-out 1/6)
test(11): unmute blame_non_signer_timeout (Phase-10 carve-out 2/6)
test(11): unmute adversarial_replay_token (Phase-10 carve-out 3/6)
test(11): unmute adversarial_invalid_utxo (Phase-10 carve-out 4/6)
test(11): unmute adversarial_wrong_denomination (Phase-10 carve-out 5/6)
test(11): unmute round_restart_and_completion_after_blame (Phase-10 carve-out 6/6)
```

**Commit body convention (D-07 + CD-1) — minimal stamp:**

```
cargo test --test integration full_round::<name> -- --ignored
test result: ok. 1 passed; 0 failed; <…>

RSA fix: <sha-of-commit-1>
```

The PASS verdict goes in the body, not the subject. Reference the Phase 11 RSA-fix commit SHA so each unmute commit is self-contained for bisect.

### Test invocation pattern (per CONTRIBUTING.md — Phase 9-05's deliverable)

**Source:** `CONTRIBUTING.md` §"Running integration tests" (referenced in CONTEXT.md `<canonical_refs>` — read by executor at the time of running tests; not duplicated here to avoid drift).

**Apply to:** all PASS-proof captures in commit bodies (CD-1).

**Constraint:** Use the brew bitcoind v31 `BITCOIND_EXE` invocation that Phase 9-05 documented. Do not invent a new test invocation — the one that produced the Phase 10 blocker is the same one Phase 11 must demonstrate green.

### Hash-commitment domain invariant (D-02)

**Source pattern** — `coordinator/src/blind/rsa.rs:37-44` (the production commitment side):

```rust
/// SHA-256 of the SPKI DER-encoded SubjectPublicKeyInfo bytes.
/// Published in GET /info response so clients can verify key matches commitment (D-02).
pub fn public_key_hash(&self) -> [u8; 32] {
    let spki = self.public_key
        .to_spki()
        .expect("RSA public key must be SPKI-encodable");
    Sha256::digest(&spki).into()
}
```

**Apply to:** the new unit test (Surface 2) — it must assert this contract bit-for-bit by emitting via `public_key_spki_der()` and confirming `Sha256::digest(&spki)` equals `signer.public_key_hash()`. The client-side mirror at `client/src/round/input.rs:29-38` (already correct) is what this hash commitment ultimately reaches; Phase 11 preserves it byte-for-byte.

---

## No Analog Found

None. All three surfaces have in-file analogs.

---

## Explicitly Preserved Boundaries (must NOT be modified)

Per CONTEXT.md `<domain>` "Not in scope" and `<code_context>` "Integration Points", these files are explicit preservation boundaries for Phase 11:

| File | Why it is preserved |
|------|---------------------|
| `tests/integration/mod.rs` | Houses `require_bitcoind!`, `BitcoindGuard`, `fund_regtest`, `FundedSetup` — Phase 9/10 fixtures the unmuted tests depend on. Phase 11 consumes the post-Fix-A version unchanged. |
| `coordinator/src/api/handlers.rs` (esp. lines 49-65, the `/info` handler) | Reads `state.rsa_pubkey_der`. Phase 11 fix is client-side only; this path is untouched. |
| `coordinator/src/round/manager.rs` (esp. line 59, `public_key_spki_der()` call) | Sets `state.rsa_pubkey_der` from the coordinator emit path. The contract anchor — preserved. |
| `shared/src/protocol.rs` (esp. `InfoResponse` lines 10-26) | `rsa_pubkey_der_b64` field name stays misleading-but-stable per D-02 / CD-3. Wire format unchanged. |
| `.github/workflows/ci.yml` | The SHA-pinned actions/checkout pattern and the `corepc-node-feature-pin-check` job stay exactly as landed in commit `4026f50`. Phase 11 makes zero CI changes. |
| `CONTRIBUTING.md` | Phase 11 consumes the test invocation it documents; it does not modify it. |
| `coordinator/src/blind/rsa.rs` lines 1-68 (non-test module body) | Only `mod tests` (lines 70-127) is edited; the production `RsaBlindSigner` impl is preserved bit-for-bit. |
| All bodies of the 6 unmuted tests in `tests/integration/full_round.rs` | Per D-07: only the `#[ignore]` attribute line is removed. Zero test-body edits. |

A test-body edit, a coordinator emit change, or a protocol field rename would break Phase 11's scope contract and trigger D-08 escape-valve (halt + surface).

---

## Metadata

**Analog search scope:** Confined to the three target files and their direct dependencies (the rust-bitcoin / blind-rsa-signatures call sites already in use). No cross-cutting hunt needed because all analogs are in-file.

**Files scanned:** 3 (read in full or targeted ranges):
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/client/src/round/input.rs` (full, 143 lines)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/coordinator/src/blind/rsa.rs` (full, 128 lines)
- `/Users/john/Desktop/vault/projects/github.com/blindjoin/tests/integration/full_round.rs` (6 targeted 12-line windows around each `#[ignore]` site)

**Pattern extraction date:** 2026-05-27
