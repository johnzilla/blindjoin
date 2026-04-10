---
phase: 01-core-protocol
plan: 05
subsystem: client
tags: [client, wallet, bip322, blind-rsa, psbt, polling]
dependency_graph:
  requires:
    - "01-04: coordinator HTTP API (endpoints the client calls)"
    - "01-01: shared protocol types (InfoResponse, InputRegRequest, etc.)"
    - "01-01: shared::token::compute_blind_token_message"
  provides:
    - "client binary: full CoinJoin round participation (input → output → sign)"
    - "ClientWallet: WIF key, P2WPKH derivation, PSBT signing"
    - "CoordinatorClient: reqwest HTTP wrapper + poll_until_phase"
    - "round::input::register_input: BIP-322 proof + blind token + RSA key verification"
    - "round::output::register_output: unblinded token submission"
    - "round::sign::verify_and_sign: PSBT verification + partial signature"
  affects:
    - "01-06: integration test (client drives the round)"
tech_stack:
  added:
    - "reqwest 0.13 (HTTP client for coordinator REST API)"
    - "clap 4 with env feature (CLI args + env var overlay)"
    - "base64 0.22 (wire encoding)"
    - "blind-rsa-signatures 0.17 client-side: PublicKey<Sha384,PSS,Randomized>, BlindingResult, BlindSignature, DefaultRng"
    - "anyhow 1 (error propagation)"
  patterns:
    - "D-14: WIF key + raw P2WPKH derivation (bdk_wallet deferred to Phase 3)"
    - "BIP-322 Simple witness generation (P2WPKH): duplicated from coordinator for Phase 1"
    - "RSA key hash commitment check (T-05-01): SHA-256(pk_der) == rsa_pubkey_hash before blinding"
    - "PSBT own-output verification (T-05-02): refuses to sign if output absent or fee > 10%"
    - "poll_until_phase: spin-polls GET /info until round_state matches target"
key_files:
  created:
    - client/Cargo.toml
    - client/src/config.rs
    - client/src/http.rs
    - client/src/wallet.rs
    - client/src/main.rs
    - client/src/round/mod.rs
    - client/src/round/input.rs
    - client/src/round/output.rs
    - client/src/round/sign.rs
  modified:
    - Cargo.toml (added clap env feature)
decisions:
  - "Used blind_rsa_signatures::DefaultRng instead of rand::thread_rng() — DefaultRng implements TryCryptoRng<Error=Infallible> which satisfies the CryptoRng bound in blind-rsa-signatures 0.17"
  - "BIP-322 helpers duplicated in client/src/round/input.rs (not re-exported from coordinator) — coordinator helpers are not pub; Phase 3 refactor moves them to shared/"
  - "Signature.unblinded_sig stored in InputRegState so output.rs can submit it without re-fetching from coordinator response"
  - "finalize() (not blind+verify separately) used — it internally verifies after unblinding, avoiding a redundant verify() call"
  - "blinding_secret field kept in InputRegState for future use; currently unused (warning documented)"
metrics:
  duration_secs: 6095
  completed_date: "2026-04-07"
  tasks_completed: 2
  tasks_total: 2
  files_created: 9
  files_modified: 2
---

# Phase 01 Plan 05: Client Binary — CoinJoin Round Participation Summary

**One-liner:** WIF-key client with BIP-322 proof generation, RSA key commitment verification, and PSBT own-output check using blind-rsa-signatures 0.17 (Sha384/PSS/Randomized).

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Client wallet + HTTP client + polling | 7a09d59 | client/Cargo.toml, config.rs, http.rs, wallet.rs, main.rs |
| 2 | Round phase modules (input, output, sign) | d2d0704 | round/mod.rs, input.rs, output.rs, sign.rs |

## What Was Built

### `client/src/wallet.rs` — `ClientWallet`

D-14 Phase 1 simplification: uses raw WIF private key and manual P2WPKH derivation. Core methods:
- `from_wif()` — parse WIF key + UTXO outpoint
- `script_pubkey()` — P2WPKH script for the UTXO key
- `coinjoin_output_address()` — deterministic output key (sk+1 tweak)
- `change_address()` — deterministic change key (sk+2 tweak)
- `sign_psbt_input()` — finds input by outpoint (T-05-04), signs P2WPKH sighash
- `secret_key_for_signing()` — exposes secp256k1 secret key for BIP-322

### `client/src/http.rs` — `CoordinatorClient`

reqwest wrapper for all 5 endpoints with `poll_until_phase()` for phase detection.

### `client/src/round/input.rs` — `register_input`

1. Verifies SHA-256(pk_der) == rsa_pubkey_hash (T-05-01 mitigation, D-02)
2. Computes `compute_blind_token_message(output_script, denomination)` via shared crate
3. Blinds message with `BjPublicKey::blind(&mut DefaultRng, &msg)` 
4. Generates BIP-322 Simple witness (to_spend / to_sign / P2WPKH sighash / ECDSA)
5. POSTs to /round/input, receives blind_signature + session_token
6. Calls `finalize()` which internally unblids and verifies the RSA signature

### `client/src/round/output.rs` — `register_output`

Submits the unblinded token (message_bytes), RSA signature, and output address to /round/output. Includes msg_randomizer for Randomized mode.

### `client/src/round/sign.rs` — `verify_and_sign`

1. Fetches PSBT from GET /round/tx
2. Finds own output by script_pubkey match — refuses if absent (T-05-02)
3. Rejects if fee_per_participant > 10% of output value
4. Signs via wallet.sign_psbt_input() (finds input by outpoint, T-05-04)
5. Submits to POST /round/sign with session_token

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] blind-rsa-signatures 0.17 API differs from plan's description**
- **Found during:** Task 1/2 compile
- **Issue:** Plan referenced `Options::default()`, `pk.blind(rng, msg, false, &options)` (4 args), `pk.finalize(sig, secret, randomizer, msg, options)` (5 args), `blinding_result.blind_msg` field — none of these exist in v0.17.1
- **Fix:** Used actual 0.17 API: `BjPublicKey::blind(&mut DefaultRng, &msg)` (2 args), `pk.finalize(&blind_sig, &blinding_result, &msg)` (3 args), `blinding_result.blind_message` field, `BlindSignature(bytes)` tuple constructor
- **Files modified:** client/src/round/input.rs, client/src/round/mod.rs
- **Commits:** d2d0704

**2. [Rule 1 - Bug] bitcoin 0.32 API differences**
- **Found during:** Task 1 compile
- **Issue:** `Address::p2wpkh(&pk, network)` takes `CompressedPublicKey` (not `PublicKey`); `Scalar::from_u32` doesn't exist; `CompressedPublicKey::from_private_key` returns `Result` (not direct value)
- **Fix:** Used `CompressedPublicKey(secp256k1::PublicKey::from_secret_key(...))` direct construction; `Scalar::from_be_bytes([0..0, n])` for numeric scalars; added `use bitcoin::hashes::Hash` for `to_byte_array()`
- **Files modified:** client/src/wallet.rs
- **Commits:** 7a09d59

**3. [Rule 1 - Bug] clap `env` feature missing from workspace**
- **Found during:** Task 1 compile
- **Issue:** `#[arg(env = "...")]` requires `clap` env feature; workspace Cargo.toml only had `["derive"]`
- **Fix:** Added `"env"` to clap features in root Cargo.toml
- **Files modified:** Cargo.toml
- **Commits:** 7a09d59

**4. [Rule 1 - Bug] DefaultRng vs ThreadRng for CryptoRng bound**
- **Found during:** Task 2 compile
- **Issue:** Plan used `rand::thread_rng()` but `ThreadRng` (rand 0.8/0.10) only impl `TryCryptoRng`, not `CryptoRng`. blind-rsa-signatures `blind()` requires `CryptoRng`
- **Fix:** Used `blind_rsa_signatures::DefaultRng` (same as crate's own examples)
- **Files modified:** client/src/round/input.rs
- **Commits:** d2d0704

## BIP-322 Proof Generation Approach

Client duplicates coordinator's `bip322_message_hash` / `build_bip322_to_spend` / `build_bip322_to_sign` helpers inline in `client/src/round/input.rs`. This is documented as a Phase 1 shortcut; Phase 3 moves these to `shared/`. The message format matches the coordinator:

```
blindjoin:round:{round_id}:utxo:{txid}:{vout}
```

Witness stack: `[ecdsa_sig_der + SIGHASH_ALL byte, compressed_pubkey_33_bytes]` — same format the coordinator's `verify_bip322_simple()` expects.

## Protocol.rs Changes

`InfoResponse.rsa_pubkey_der_b64` was already present from Plan 01 (the field was added preemptively). No changes needed to shared/src/protocol.rs.

## Threat Mitigations Applied

| Threat | File | Mitigation |
|--------|------|-----------|
| T-05-01: RSA key substitution | round/input.rs:30-34 | SHA-256(pk_der) verified against rsa_pubkey_hash before blinding |
| T-05-02: PSBT tampered output | round/sign.rs:24-29 | own output searched by script_pubkey, error if not found |
| T-05-03: WIF key in logs | config.rs, main.rs | utxo_wif only used in wallet::from_wif(); never passed to tracing! |
| T-05-04: Wrong PSBT input signed | wallet.rs:87 | input found by previous_output == utxo_outpoint comparison |

## Known Stubs

None — all client phase functions are fully implemented. The `blinding_secret` field in `InputRegState` is stored but not used in Phase 1 (it would be needed if re-blinding were required). This is not a stub — the field is present for correctness and future use.

## Self-Check: PASSED

- All 8 client source files present
- Commits 7a09d59 and d2d0704 verified in git log
- `cargo build --workspace` exits 0 (all three crates compile, 0 errors)
- RSA key hash verification present in round/input.rs (rsa_pubkey_hash check)
- PSBT own-output check present in round/sign.rs ("Our output not found in PSBT")
- No private key material in any tracing! macro calls
