---
phase: 01-core-protocol
plan: "02"
subsystem: coordinator
tags: [rsa-blind-signatures, round-fsm, session-tokens, zeroize, hmac]
dependency_graph:
  requires: ["01-01"]
  provides: ["coordinator::blind::rsa::RsaBlindSigner", "coordinator::round::state::RoundState", "coordinator::round::state::Phase", "coordinator::round::manager::generate_session_token", "coordinator::round::manager::verify_session_token", "coordinator::round::manager::run_phase_timer"]
  affects: ["coordinator HTTP handlers (plan 03)", "coordinator transaction construction (plan 04)"]
tech_stack:
  added: ["blind-rsa-signatures 0.17.1", "sha2 0.11", "hmac 0.13", "thiserror 1", "rand 0.8"]
  patterns: ["KeyPair<Sha384, PSS, Randomized> for RFC 9474 blind signing", "manual Drop for HashMap zeroing", "Arc<RwLock<RoundState>> for shared FSM state", "HMAC-SHA256 session tokens"]
key_files:
  created:
    - coordinator/src/lib.rs
    - coordinator/src/blind/mod.rs
    - coordinator/src/blind/rsa.rs
    - coordinator/src/round/mod.rs
    - coordinator/src/round/state.rs
    - coordinator/src/round/manager.rs
  modified:
    - coordinator/Cargo.toml
decisions:
  - "Used KeyPair<Sha384, PSS, Randomized> (blind-rsa-signatures 0.17.x actual API) instead of SecretKey::generate (plan research referenced older API)"
  - "Manual Drop for RoundStateInner instead of #[derive(ZeroizeOnDrop)] — HashMap does not implement Zeroize; zeroing is field-by-field in impl Drop"
  - "Added [lib] target to coordinator/Cargo.toml — required for cargo test --lib to find unit tests"
  - "Added two phase timer tests (fires/noop) beyond plan spec to validate timer contract"
metrics:
  duration_seconds: 327
  completed_date: "2026-04-08"
  tasks_completed: 2
  tasks_total: 2
  files_created: 6
  files_modified: 1
  tests_added: 14
---

# Phase 01 Plan 02: RSA Blind Signer + Round FSM Summary

**One-liner:** RFC 9474 RSA blind signer (Sha384/PSS/Randomized), 6-state enum FSM with manual ZeroizeOnDrop via HashMap-aware impl Drop, and HMAC-SHA256 session tokens.

## What Was Built

### coordinator/src/blind/rsa.rs — RsaBlindSigner

- `RsaBlindSigner::generate()` — generates RSA-2048 keypair via `KeyPair::<Sha384, PSS, Randomized>::generate(&mut DefaultRng, 2048)`
- `public_key_hash()` — SHA-256 of SPKI DER-encoded public key bytes (32 bytes, deterministic)
- `blind_sign(blinded_msg)` — delegates to `SecretKey::blind_sign`; coordinator never sees original message M
- `secret_key` field is private; never exposed outside module
- Best-effort memory zeroing documented: blind-rsa-signatures 0.17.x `SecretKey` does not implement `Zeroize`; serialized key bytes in `RoundStateInner.rsa_signing_key` ARE zeroed via manual Drop

### coordinator/src/round/state.rs — Phase FSM + RoundState

- `Phase` enum: Idle, InputReg, OutputReg, Signing, Broadcast, Blame
- `Phase::can_transition_to()` — 7 valid edges, all others rejected
- `RoundState::transition_to()` — enforces FSM; on `Idle` transition drops `inner` (zeroing sensitive data)
- `RoundStateInner` — holds rsa_signing_key (Vec<u8>), round_secret ([u8;32]), registered_inputs, redeemed_tokens, registered_outputs, partial_sigs, change_addresses
- Manual `Drop` implementation zeroes all fields before clearing HashMaps

### coordinator/src/round/manager.rs — Phase Timer + Session Tokens

- `generate_session_token(round_secret, utxo)` — HMAC-SHA256(round_secret, txid_bytes || vout_le32); deterministic
- `verify_session_token(round_secret, utxo, token)` — constant-output comparison
- `run_phase_timer(round, expected_phase, timeout, on_timeout)` — async timer; no-ops if phase already advanced

## Test Results

```
test result: ok. 14 passed; 0 failed; 0 ignored
```

| Test | Module | Status |
|------|--------|--------|
| blind_sign_round_trip | blind::rsa | ok |
| public_key_hash_is_32_bytes | blind::rsa | ok |
| public_key_hash_is_deterministic | blind::rsa | ok |
| unlinkability_two_tokens | blind::rsa | ok |
| valid_fsm_transitions | round::state | ok |
| invalid_fsm_transitions | round::state | ok |
| transition_to_idle_clears_inner | round::state | ok |
| invalid_transition_returns_err | round::state | ok |
| session_token_deterministic | round::manager | ok |
| session_token_different_utxos_differ | round::manager | ok |
| session_token_verify_ok | round::manager | ok |
| session_token_verify_wrong_token | round::manager | ok |
| phase_timer_fires_on_expected_phase | round::manager | ok |
| phase_timer_noop_when_phase_already_advanced | round::manager | ok |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] blind-rsa-signatures 0.17.x API mismatch**
- **Found during:** Task 1 (compilation)
- **Issue:** Plan research documented `SecretKey::generate(&mut rng, 2048)`, `BlindedMessage`, `Options::default()`, `pk.blind(&mut rng, msg, false, &options)`, `to_public_key_der()`. Actual 0.17.1 API uses `KeyPair::generate`, `BlindMessage`, no `Options` type (generics serve that role), `pk.blind(&mut rng, msg)` (no bool/options args), `to_spki()`.
- **Fix:** Rewrote rsa.rs to use actual 0.17.1 API: `KeyPair::<Sha384, PSS, Randomized>`, `BlindMessage`, `DefaultRng`, `to_spki()`.
- **Files modified:** coordinator/src/blind/rsa.rs
- **Commit:** 3053526

**2. [Rule 1 - Bug] HashMap does not implement Zeroize**
- **Found during:** Task 2 (compilation)
- **Issue:** `#[derive(Zeroize, ZeroizeOnDrop)]` on `RoundStateInner` failed because `HashMap<K,V>` does not implement `Zeroize` (upstream limitation in zeroize 1.8.x).
- **Fix:** Replaced derive with manual `impl Drop for RoundStateInner` that: zeroizes `rsa_signing_key` and `round_secret` directly, iterates HashMap values calling `.zeroize()` on each, then clears the maps.
- **Files modified:** coordinator/src/round/state.rs
- **Commit:** 4df2519

**3. [Rule 1 - Bug] Missing `hmac::KeyInit` import**
- **Found during:** Task 2 (compilation)
- **Issue:** `HmacSha256::new_from_slice` requires `KeyInit` trait in scope.
- **Fix:** Added `KeyInit` to the `use hmac::{Hmac, Mac, KeyInit}` import.
- **Files modified:** coordinator/src/round/manager.rs
- **Commit:** 4df2519

**4. [Rule 2 - Missing] coordinator lib target absent**
- **Found during:** Task 1 (test run showed 0 tests — cargo used the binary target)
- **Issue:** coordinator/Cargo.toml had no `[lib]` section; `cargo test --lib` found no library target.
- **Fix:** Added `[lib] name = "coordinator" path = "src/lib.rs"` to Cargo.toml.
- **Files modified:** coordinator/Cargo.toml
- **Commit:** 3053526

## Interfaces Exported

```rust
// coordinator::blind::rsa
pub struct RsaBlindSigner { pub public_key: BjPublicKey, /* secret_key: private */ }
impl RsaBlindSigner {
    pub fn generate() -> Result<Self, Error>
    pub fn public_key_hash(&self) -> [u8; 32]
    pub fn blind_sign(&self, blinded_msg: &BlindMessage) -> Result<BlindSignature, Error>
}
pub type BjPublicKey = PublicKey<Sha384, PSS, Randomized>;
pub type BjSecretKey = SecretKey<Sha384, PSS, Randomized>;
pub type BjKeyPair   = KeyPair<Sha384, PSS, Randomized>;

// coordinator::round::state
pub enum Phase { Idle, InputReg, OutputReg, Signing, Broadcast, Blame }
impl Phase { pub fn can_transition_to(&self, next: &Phase) -> bool }
pub struct RoundState { pub phase: Phase, pub round_id: Uuid, pub rsa_pubkey_hash: Option<[u8;32]>, pub participant_count: u32, pub inner: Option<RoundStateInner> }
impl RoundState { pub fn new_idle() -> Self; pub fn transition_to(&mut self, next: Phase) -> Result<(), TransitionError> }
pub struct RoundStateInner { pub rsa_signing_key: Vec<u8>, pub round_secret: [u8;32], ... }

// coordinator::round::manager
pub fn generate_session_token(round_secret: &[u8; 32], utxo: &OutPoint) -> [u8; 32]
pub fn verify_session_token(round_secret: &[u8; 32], utxo: &OutPoint, token: &[u8; 32]) -> bool
pub async fn run_phase_timer(round: Arc<RwLock<RoundState>>, expected_phase: Phase, timeout: Duration, on_timeout: impl FnOnce(&mut RoundState) + Send + 'static)
```

## Known Stubs

None. All interfaces are fully implemented and tested.

## Threat Flags

None. No new network endpoints, auth paths, file access patterns, or schema changes introduced. All components are pure in-process logic matching the plan's threat model.

## Self-Check: PASSED

- coordinator/src/blind/rsa.rs: FOUND
- coordinator/src/round/state.rs: FOUND
- coordinator/src/round/manager.rs: FOUND
- commit 3053526 (Task 1): FOUND
- commit 4df2519 (Task 2): FOUND
