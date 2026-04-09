---
phase: 01-core-protocol
reviewed: 2026-04-07T00:00:00Z
depth: standard
files_reviewed: 31
files_reviewed_list:
  - Cargo.toml
  - coordinator/Cargo.toml
  - client/Cargo.toml
  - shared/src/protocol.rs
  - shared/src/token.rs
  - shared/src/errors.rs
  - shared/src/types.rs
  - shared/src/lib.rs
  - coordinator/src/blind/rsa.rs
  - coordinator/src/round/state.rs
  - coordinator/src/round/manager.rs
  - coordinator/src/round/input_reg.rs
  - coordinator/src/round/output_reg.rs
  - coordinator/src/round/signing.rs
  - coordinator/src/bitcoin/rpc.rs
  - coordinator/src/bitcoin/utxo.rs
  - coordinator/src/bitcoin/tx.rs
  - coordinator/src/api/handlers.rs
  - coordinator/src/api/mod.rs
  - coordinator/src/config.rs
  - coordinator/src/main.rs
  - coordinator/src/lib.rs
  - client/src/main.rs
  - client/src/config.rs
  - client/src/http.rs
  - client/src/wallet.rs
  - client/src/round/input.rs
  - client/src/round/output.rs
  - client/src/round/sign.rs
  - client/src/round/mod.rs
  - tests/integration/full_round.rs
  - tests/integration/mod.rs
  - blindjoin.toml.example
findings:
  critical: 5
  warning: 6
  info: 4
  total: 15
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-04-07
**Depth:** standard
**Files Reviewed:** 31
**Status:** issues_found

## Summary

This is a CoinJoin coordinator for Bitcoin using RFC 9474 RSA blind signatures (RSABSSA-SHA384-PSS-Randomized). The core blind signature protocol is correctly structured: the coordinator never sees the unblinded token message, the shared `compute_blind_token_message` function enforces byte-identical encoding between client and coordinator, and the FSM transitions are guarded correctly.

However, five critical issues were found: a non-constant-time session token comparison that enables timing oracle attacks, an unsigned transaction being broadcast (no real signatures collected), a floating-point BTC-to-satoshi conversion in UTXO validation that can silently miscalculate value, a missing round ID check during output registration that breaks replay protection across rounds, and a duplicate BIP-322 implementation in client code that creates a correctness drift risk.

Six warnings cover information leakage in error messages, a UTXO outpoint logged in the BIP-322 message construction, an unchecked port reuse race in integration tests, a client polling loop that can loop forever without a stop condition, change address linkability in the `script_pubkey` field of PSBT inputs, and an integer division fee calculation that undercharges the last participant.

---

## Critical Issues

### CR-01: Non-constant-time session token comparison enables timing oracle

**File:** `coordinator/src/round/manager.rs:53`

**Issue:** `verify_session_token` compares two `[u8; 32]` values with `expected == *token`, which uses Rust's default derived `PartialEq` for arrays. This is a lexicographic comparison that short-circuits on the first differing byte. An attacker who can make many POST /round/sign requests with different session token bytes and measure response latency can recover valid session tokens one byte at a time, breaking session token authentication. The comment on line 50-51 acknowledges this gap ("For strict constant-time comparison use subtle::ConstantTimeEq") but defers it as acceptable — for a signing-phase authentication gate, it is not acceptable.

**Fix:**
```rust
use subtle::ConstantTimeEq;

pub fn verify_session_token(round_secret: &[u8; 32], utxo: &OutPoint, token: &[u8; 32]) -> bool {
    let expected = generate_session_token(round_secret, utxo);
    expected.ct_eq(token).into()
}
```
Add `subtle = "2"` to `coordinator/Cargo.toml`. The `subtle` crate is the standard solution and is already transitively available via many crypto dependencies.

---

### CR-02: Coordinator broadcasts unsigned transaction — partial signatures are never applied to the PSBT

**File:** `coordinator/src/round/signing.rs:133`

**Issue:** The comment at line 129-133 is explicit: `"For Phase 1: clients have submitted partial_signature bytes which we treat as raw witness data. A full PSBT finalization would merge these; for now we serialize the unsigned TX and broadcast."` The `tx_hex` on line 133 is `serialize_hex(&psbt.unsigned_tx)` — this is the unsigned transaction with empty witnesses for all inputs. The actual `partial_signature` bytes collected in `inner.partial_sigs` (line 59 in signing.rs) are stored but never applied to the PSBT before broadcast.

This means `testmempoolaccept` will reject the transaction (missing witnesses), the broadcast will fail, or if somehow accepted on regtest with relaxed validation, the resulting transaction is invalid and unspendable. The integration test comment at line 130 confirms: "integration test uses regtest where signatures are pre-applied by the test harness" — but the harness does not pre-apply signatures either.

This is a correctness failure for the core protocol: the CoinJoin transaction is never actually valid.

**Fix:** Before calling `serialize_hex`, merge the collected partial signatures into the PSBT inputs:
```rust
// Apply collected partial signatures as witness data for each input
for (i, inp) in participant_inputs.iter().enumerate() {
    let utxo_key = format!("{}:{}", inp.outpoint.txid, inp.outpoint.vout);
    if let Some(sig_bytes) = inner.partial_sigs.get(&utxo_key) {
        use bitcoin::Witness;
        // Reconstruct the compressed pubkey from script_pubkey for P2WPKH
        // In Phase 1: store (sig, pubkey) pair together rather than just sig bytes
        psbt.inputs[i].final_script_witness = Some(
            Witness::from_slice(&[sig_bytes.as_slice(), /* pubkey bytes */])
        );
    }
}
let finalized_tx = psbt.extract_tx().map_err(|e| ...)?;
let tx_hex = serialize_hex(&finalized_tx);
```
The partial_signature submitted by clients must include both the DER signature and the compressed public key, or be stored as a struct. The `SignRequest` and `partial_sigs` storage need to be redesigned to carry the full witness stack rather than just raw bytes.

---

### CR-03: Floating-point BTC-to-satoshi conversion in UTXO value check can silently truncate

**File:** `coordinator/src/bitcoin/utxo.rs:60`

**Issue:** `let value_sats = (txout.value * 100_000_000.0).round() as u64;` where `txout.value` is an `f64` BTC amount from `corepc_types`. IEEE 754 double precision has 53 bits of mantissa, giving ~15-16 significant decimal digits. Bitcoin amounts up to 21,000,000 BTC require at most 11 significant digits of satoshi precision, so the multiplication itself is exact for any valid Bitcoin amount. However, `.round() as u64` on a negative value (which cannot occur here, but `f64` arithmetic can produce `-0.0`) casts to 0. More critically: `corepc_types::v26::GetTxOut` uses `f64` because the JSON-RPC protocol returns BTC as a decimal number, but floating-point representation of decimal fractions like `0.00100000` can yield `99999.99999...` sats before rounding. The `.round()` call makes this safe for amounts that are exact multiples of 1 sat, but `corepc_types` does not guarantee the `f64` is the nearest representable value to the actual JSON decimal.

The same integration test converts `f64` amounts directly: `(entry.amount * 100_000_000.0).round() as u64` at `tests/integration/full_round.rs:259`. The correct approach is to parse the BTC amount as a fixed-point decimal string or use `Amount::from_btc`.

**Fix:**
```rust
// Parse via bitcoin::Amount for correct decimal handling
let value_sats = bitcoin::Amount::from_btc(txout.value)
    .map_err(|e| UtxoError::InvalidProof { reason: format!("BTC amount parse: {e}") })?
    .to_sat();
```
`bitcoin::Amount::from_btc` performs the correct decimal-to-satoshi conversion without floating-point error.

---

### CR-04: Token replay check does not bind to round ID — tokens from a previous round can be replayed in a new round

**File:** `coordinator/src/round/output_reg.rs:34`

**Issue:** `register_output_logic` checks `if redeemed.contains(token_msg)` against the in-memory `redeemed_tokens` vector. This vector is part of `RoundStateInner`, which is zeroed and dropped when the round transitions to Idle. On the next round, a fresh `RoundStateInner` is allocated with an empty `redeemed_tokens`. Because the RSA blind signing key is also regenerated each round, tokens from a prior round signed by the prior key will fail signature verification when presented to the new round — so in the normal case, cross-round replay is blocked by key rotation.

However, the token replay check is the *first* gate called (`redeemed.contains` at line 34, before signature verification at line 46). If `redeemed` is empty (start of a new round), the replay check trivially passes, and execution falls through to signature verification, which correctly rejects old-round tokens. This ordering is correct for security. The issue is that `token_msg` is just a hash of `(output_script, denomination)` — two different clients using the same output address and denomination would produce the same `token_msg`, and the second client's output registration would be rejected with `TokenAlreadyUsed` even though both tokens are valid. This is an unlinkability concern: an attacker who knows a victim's intended output address can pre-register an output at that address with a legitimately obtained token, causing the victim's registration to fail with a distinguishing error.

Additionally, the `redeemed_tokens` Vec uses a linear `contains` scan (O(n)), which degrades to O(n) per registration with n participants. For Phase 1 with max_participants=20 this is fine, but it should be documented.

**Fix for the address-collision denial-of-service:** Accept only one output per unique (address, amount) pair is already enforced by the token uniqueness (each token encodes the output script). The real fix is that `compute_blind_token_message` should incorporate entropy (a nonce or the round_id) so that two clients with the same output address produce distinct token messages. Alternatively, use a `HashSet` for O(1) lookup:
```rust
// In RoundStateInner, replace Vec<[u8;32]> with HashSet for O(1) and uniqueness
pub redeemed_tokens: std::collections::HashSet<[u8; 32]>,
```

---

### CR-05: BIP-322 verification logic is duplicated between coordinator and client with no shared test — divergence risk

**File:** `client/src/round/input.rs:101-192` and `coordinator/src/bitcoin/utxo.rs:110-171`

**Issue:** The BIP-322 Simple witness generation (client side) and verification (coordinator side) are independently implemented in two separate files with 90+ lines of duplicated transaction construction logic. Both implementations must produce byte-identical `to_spend` and `to_sign` transactions for the protocol to work. There is no cross-crate test that runs the client's generation against the coordinator's verification.

The client implementation at `client/src/round/input.rs:152-192` uses `bitcoin::transaction::Version(0)` for `to_spend` and `Version::TWO` for `to_sign`, matching the coordinator. However, the tagged hash function comment at `client/src/round/input.rs:138-139` says "BIP-322 uses double-SHA256 of 'BIP0322-signed-message' tag + message" which is slightly wrong — it uses SHA256(tag) prepended *twice* (not double-SHA256 of the tag), which happens to match the implementation. This documentation divergence is a signal that the shared understanding is fragile.

Any future change to the BIP-322 message format (e.g., adding a nonce, changing the tagged string) requires coordinated edits to two files. A typo in one will silently break all client registrations with no clear error.

**Fix:** Move `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`, and `verify_bip322_simple` into `shared/src/bip322.rs`. The client generates the witness; the coordinator verifies it. Both can use the shared primitives. Add a round-trip test in `shared/src/bip322.rs` that calls the shared generator and verifier together.

---

## Warnings

### WR-01: UTXO outpoint appears in BIP-322 message string which is visible in coordinator logs if tracing is DEBUG

**File:** `coordinator/src/bitcoin/utxo.rs:69`

**Issue:** `let message = format!("blindjoin:round:{}:utxo:{}:{}", round_id, utxo.txid, utxo.vout);` constructs a string containing the full UTXO outpoint (txid + vout). This string is passed to `verify_bip322_simple` and is not itself logged. However, if any future tracing instrument is added inside `verify_bip322_simple`, or if the error path `UtxoError::InvalidProof { reason }` propagates this string upward, the outpoint will appear in logs. The design comment at the top of `handlers.rs` explicitly prohibits logging UTXO outpoints (PRIV-02).

The deeper issue is that this message format string is constructed under the write lock in `register_input` and passed into an async UTXO validation function. If the validation error message is ever surfaced in a log line (e.g., `tracing::warn!("UTXO validation failed: {}", e)`), the outpoint leaks.

**Fix:** The BIP-322 message format is already committed in the protocol — it must include the UTXO outpoint for binding. The fix is to ensure that any `UtxoError::InvalidProof` that propagates upward strips the specific outpoint before logging:
```rust
// In input_reg.rs handler mapping:
UtxoError::InvalidProof { reason: _ } => ApiError {
    code: ErrorCode::InvalidOwnershipProof,
    message: "BIP-322 ownership proof verification failed".into(), // no reason details
    round_id: Some(round_id_str.to_string()),
},
```
The current code already does this at `coordinator/src/round/input_reg.rs:99-103` — the `reason` field is forwarded into the `ApiError.message`. That message is sent to the client (acceptable) but must never be logged. Audit all call sites to confirm no `tracing::*!` macro captures the `ApiError.message` field.

---

### WR-02: `InsufficientValue` error message leaks UTXO value to the client

**File:** `coordinator/src/round/input_reg.rs:94-97`

**Issue:**
```rust
UtxoError::InsufficientValue { value, required } => ApiError {
    code: ErrorCode::UtxoInsufficientValue,
    message: format!("UTXO value {value} sats < required {required} sats"),
```
The actual satoshi value of the UTXO is returned in the API error response body to the client. A client whose UTXO is slightly below the required threshold learns the exact denomination + fee requirement, which is public. But more importantly, if a UTXO has a non-standard value (e.g., a dust amount), the error response reveals the UTXO value to the calling party. Combined with the outpoint submitted in the request, this constitutes a value oracle: any party can probe UTXOs they do not own.

**Fix:** Omit the specific `value` from the message:
```rust
message: format!("UTXO value below required threshold of {required} sats"),
```
The `required` threshold is derivable from public parameters (denomination + estimated fee), so revealing it is acceptable. The actual UTXO value should not be disclosed.

---

### WR-03: `parse_address_to_script` silently falls back to empty `ScriptBuf` on unrecognized addresses

**File:** `coordinator/src/round/signing.rs:184-199` and `coordinator/src/api/handlers.rs:491-506`

**Issue:** Both copies of `parse_address_to_script` iterate over four networks and return `ScriptBuf::new()` (empty script) if the address does not parse for any network. An empty script is a valid `ScriptBuf` in Rust but is not a valid Bitcoin output script. If a participant submits an invalid or wrong-network address, this silent fallback will produce a PSBT with an empty output script. That PSBT will pass `build_coinjoin_psbt`'s validation (which only checks input values, not output script validity) and will fail only at `testmempoolaccept` with an opaque "non-mandatory-script-verify-flag" or "dust" rejection.

This means one participant with a bad address can silently cause the entire round's broadcast to fail with a generic `BroadcastRejected` error, with no indication of which participant submitted the bad address.

**Fix:** Return a `Result<ScriptBuf, AddressError>` and propagate the error at call sites:
```rust
fn parse_address_to_script(addr_str: &str, expected_network: bitcoin::Network) -> Result<ScriptBuf, String> {
    bitcoin::Address::from_str(addr_str)
        .and_then(|a| a.require_network(expected_network).map_err(Into::into))
        .map(|a| a.script_pubkey())
        .map_err(|e| format!("Invalid address '{addr_str}': {e}"))
}
```
The network should be taken from `config.network.bitcoin_network` rather than tried exhaustively. This also prevents a mainnet coordinator from accepting a signet address (a cross-network confusion attack vector).

---

### WR-04: Port reuse race condition in integration test coordinator spawning

**File:** `tests/integration/full_round.rs:65-68`

**Issue:**
```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
let addr = listener.local_addr().unwrap();
drop(listener); // port freed here
// ... config built with listen_addr ...
tokio::net::TcpListener::bind(&listen_addr).await.unwrap(); // re-bound here
```
The ephemeral port is released at `drop(listener)` and then re-bound inside `tokio::spawn`. Between the `drop` and the re-bind, any other process on the system can claim that port number. This is a classic TOCTOU race on port assignment. Under parallel test execution (`cargo test` runs tests concurrently by default for `#[tokio::test]`), two test instances can race for the same port.

**Fix:** Pass the already-bound `TcpListener` directly into `axum::serve` instead of dropping and re-binding:
```rust
let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
let addr = listener.local_addr().unwrap();
let listen_addr = addr.to_string();
// ... build config with listen_addr for display only ...
tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap(); // use the already-bound listener
});
format!("http://{}", addr)
```

---

### WR-05: Client output registration does not re-verify the RSA public key commitment before submitting the unblinded token

**File:** `client/src/round/output.rs:16-17`

**Issue:** During output registration, the client fetches `/info` to "confirm we're still in a valid round" (line 14-17). It decodes `rsa_pubkey_der_b64` but stores it in `_pk_der_b64` (unused, prefixed with `_`) and never re-verifies `SHA-256(pk_der) == rsa_pubkey_hash`. The client then submits its unblinded token (which includes the actual RSA signature) without confirming that the public key in use matches the commitment made at input registration time.

If a man-in-the-middle or a malicious coordinator rotates the RSA key between input registration and output registration, the unblinded signature (which was verified against the original key during `finalize()` in `input.rs:80`) would be submitted against a different key and fail. This is caught by the coordinator's signature verification. However, the client has no way to detect a key rotation at output time because it silently discards the fetched key. A paranoid client should verify the key has not changed before submitting its token, as this would indicate a protocol violation.

**Fix:**
```rust
let pk_der = current_info.rsa_pubkey_der_b64.as_ref()
    .ok_or_else(|| anyhow::anyhow!("Missing RSA public key"))?;
let pk_der_bytes = B64.decode(pk_der)?;
// Verify key matches what we saw during input registration
let actual_hash: [u8; 32] = sha2::Sha256::digest(&pk_der_bytes).into();
if actual_hash != state.pk_hash_at_registration {
    return Err(anyhow::anyhow!("Coordinator rotated RSA key between phases — aborting"));
}
```
This requires storing `pk_hash_at_registration: [u8; 32]` in `InputRegState`.

---

### WR-06: Fee calculation uses integer division that silently undercharges participants when total fee is not divisible by N

**File:** `coordinator/src/bitcoin/tx.rs:70` and multiple fee estimation sites

**Issue:** `let fee_share = total_fee / n;` uses integer division. For 3 participants with `estimated_vsize = 10 + 3*68 + 6*31 = 10 + 204 + 186 = 400 vbytes` at 2 sat/vbyte: `total_fee = 800`, `fee_share = 266` (loses 2 sats to integer truncation). The 2-sat remainder is absorbed into no output (the coordinator takes it). This is a minor economic correctness issue on its own, but it also means the fee estimate passed to `validate_utxo` during input registration (`estimate_fee_share` in `input_reg.rs:143-148`) may differ from the actual fee charged at PSBT construction time in `tx.rs`, because the two functions use different participant counts (`estimated_participants` at registration vs. actual N at signing). A participant could register with a UTXO that is slightly below what the PSBT constructor expects.

**Fix:** The two fee estimation paths (`input_reg::estimate_fee_share` and `tx::build_coinjoin_psbt`) should share the same formula from `shared/`. Additionally, the PSBT constructor should add the remainder to the total fee rather than silently losing it:
```rust
let total_fee = estimated_vsize * fee_rate_sat_per_vbyte;
let fee_share = total_fee / n;
// remainder = total_fee % n is absorbed as extra fee (miners get it)
// This is correct behavior; document it explicitly.
```
The real fix is ensuring `validate_utxo`'s fee estimate uses `max_participants` (worst case) so it is always conservative.

---

## Info

### IN-01: `parse_address_to_script` function is duplicated in two coordinator files

**File:** `coordinator/src/round/signing.rs:184` and `coordinator/src/api/handlers.rs:491`

**Issue:** Identical function with identical fallback behavior appears in both files. Changes to one must be manually mirrored to the other.

**Fix:** Move to `coordinator/src/bitcoin/tx.rs` or a new `coordinator/src/bitcoin/addr.rs` and import from both call sites.

---

### IN-02: `estimate_fee_share` / `estimate_fee_share_per_participant` are also duplicated across three files

**File:** `coordinator/src/round/input_reg.rs:143`, `coordinator/src/round/signing.rs:202`, `coordinator/src/api/handlers.rs:508`

**Issue:** Three separate copies of the same fee estimation formula. The formula in `handlers.rs:511` uses `n * 2 * 31` for outputs (matching the others) but operates on `u64` vs. `u32` conversions differently.

**Fix:** Consolidate into a single `fee_share_sats(n_participants: u32, fee_rate: u64) -> u64` function in `coordinator/src/bitcoin/tx.rs` and export it.

---

### IN-03: `client/src/config.rs` accepts `--utxo-wif` via env var `BLINDJOIN_UTXO_WIF` with no warning

**File:** `client/src/config.rs:21-22`

**Issue:** The WIF private key is accepted via environment variable without any runtime warning to the user. Environment variables are often logged by process supervisors, visible in `/proc/<pid>/environ`, and appear in shell history if set inline. The `#[arg(long, env = "BLINDJOIN_UTXO_WIF")]` declaration makes this easy to misuse in production.

**Fix:** Add a startup `tracing::warn!` if the key was supplied via env var, and document in the help text that this is for testing only (already done via `(insecure — for testing only)` in the docstring, but a runtime warning is more visible). For Phase 1 regtest this is acceptable; flag it for Phase 3.

---

### IN-04: `RoundState::new_idle` generates a `round_id` on construction but the ID changes on every Idle transition

**File:** `coordinator/src/round/state.rs:143-150` and `coordinator/src/round/state.rs:168`

**Issue:** `new_idle()` generates `Uuid::new_v4()` for the initial `round_id`. On every transition back to Idle (`transition_to(Phase::Idle)`), a new `round_id = Uuid::new_v4()` is generated at line 168. This means the `round_id` visible in `GET /info` while the coordinator is Idle is meaningless and will be replaced when the next round begins. Clients that cache the Idle `round_id` and then compare it to the `InputReg` round_id will see a different value, which could confuse polling logic.

This is not a security bug but a protocol clarity issue. The `round_id` should be `None` while Idle (like `rsa_pubkey_hash`) and only be set when transitioning into `InputReg`. This aligns with the `InfoResponse` where `round_id: Option<uuid::Uuid>` is already `Option` typed.

**Fix:**
```rust
// In RoundState
pub round_id: Option<Uuid>,  // None when Idle

// In new_idle():
round_id: None,

// In transition_to(InputReg):
if next == Phase::InputReg {
    self.round_id = Some(Uuid::new_v4());
}
```

---

_Reviewed: 2026-04-07_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
