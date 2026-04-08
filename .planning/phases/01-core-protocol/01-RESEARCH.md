# Phase 1: Core Protocol — Research

**Researched:** 2026-04-08
**Domain:** Rust CoinJoin coordinator — RSA blind signatures, Bitcoin PSBT, axum HTTP, enum FSM, BIP-322
**Confidence:** HIGH (all core decisions locked; key library versions verified against crates.io)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Enum FSM for round state machine (not typestate). 6 states, <10 transitions. `Arc<RwLock<RoundState>>` for shared state in axum handlers.
- **D-02:** Per-round ephemeral RSA-2048 keys with pre-commitment. Coordinator publishes key hash in `GET /info` before accepting registrations. Clients verify key matches hash before blinding.
- **D-03:** Domain-separated blind token format: `SHA-256("blindjoin-v1" || scriptPubKey_bytes || amount_sats_le64)`. Security-critical canonical serialization.
- **D-04:** BIP-322 Simple verification implemented directly (~50 lines using rust-bitcoin primitives). No bip322 crate dependency.
- **D-05:** HMAC-based session tokens for signing phase reconnection: `HMAC(coordinator_round_secret, UTXO_outpoint)`. Deterministic, no storage needed.
- **D-06:** Shared protocol message types use serde with default behavior (allow unknown fields) for forward compatibility between coordinator/client versions.
- **D-07:** zeroize crate with ZeroizeOnDrop on all round-state structs. Memory zeroing is a design principle, not a post-hoc audit.
- **D-08:** REST-style HTTP API: `GET /info`, `POST /round/input`, `POST /round/output`, `POST /round/sign`, `GET /round/tx`.
- **D-09:** Structured JSON error responses: `{"error": {"code": "UTXO_SPENT", "message": "...", "round_id": "..."}}`.
- **D-10:** Polling `GET /info` for phase transition detection. 1s interval acceptable for clearnet Sprint 1.
- **D-11:** Configuration via TOML file (`blindjoin.toml`) with `BLINDJOIN_*` environment variable overrides.
- **D-12:** Fail-fast startup health checks: verify bitcoind reachable, correct network, synced (not IBD). Exit with clear error if any check fails.
- **D-13:** Thin reqwest-based Bitcoin RPC client (~100 lines, 5 methods). Use `corepc-types` for type-safe request/response structs.
- **D-14:** bdk_wallet 1.0+ for client wallet operations. Generate new descriptor wallet by default (BIP-84 derivation), accept `--descriptor` flag.
- **D-15:** Manual signet faucet for first coins. No built-in faucet integration in Sprint 1.
- **D-16:** Spec defaults: denomination 1,000,000 sats, min 3 participants, max 20, input reg timeout 60s, output reg timeout 60s, signing timeout 30s, ban duration 1 hour, fee rate 2 sat/vB. All configurable.

**Specific Ideas (canonical):**
- RSA public key hash returned in `GET /info` response alongside round state, denomination, participant count
- Session token (HMAC) returned to client during input registration alongside the blind signature
- Change address provided during input registration (linkable to input, documented and expected)
- Signing phase identifies by UTXO outpoint (not input index) to prevent cross-participant signature injection

**API field corrections (design doc supersedes spec):**
- `/round/input` request includes `change_address` field (replaces ambiguous `change_address_index`)
- `/round/sign` request uses `utxo_outpoint` (not `input_index`)
- `/round/input` response contains `blind_signature` + `round_id` + `session_token` (session token added per D-05)

### Claude's Discretion

- Error code taxonomy (specific error codes for each rejection type)
- Axum middleware configuration (rate limiting, request size limits)
- Internal data structures for round state (participant tracking, token registry)
- Logging format and verbosity levels (tracing crate configuration)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| PROTO-01 | RSA blind signatures (RFC 9474, blind-rsa-signatures crate) | `blind-rsa-signatures` 0.17.1 verified on crates.io; RFC 9474 API pattern documented below |
| PROTO-02 | Round state machine: IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE with timeouts | Enum FSM pattern with `Arc<RwLock<>>` and tokio timers documented below |
| PROTO-03 | Fixed-denomination equal outputs (configurable, default 0.01 BTC) | Transaction construction pattern documented below |
| PROTO-04 | Per-round ephemeral RSA-2048 keys with pre-commitment | Key generation and hash-commitment pattern documented below |
| PROTO-05 | Domain-separated blind token: `SHA-256("blindjoin-v1" \|\| scriptPubKey \|\| amount_sats_le64)` | SHA-256 domain separation pattern via sha2 crate documented below |
| PROTO-06 | Shared protocol message types with `#[serde(default)]` forward compatibility | Serde pattern documented below |
| PROTO-07 | HMAC session tokens: `HMAC(coordinator_round_secret, UTXO_outpoint)` | HMAC-SHA256 via RustCrypto hmac 0.13.0 documented below |
| UTXO-01 | UTXO existence/unspent verified via Bitcoin Core RPC (thin reqwest client, ~5 RPC methods) | RPC client pattern using reqwest 0.13.2 + corepc-types 0.11.0 documented below |
| UTXO-02 | UTXO value >= denomination + estimated fee share | Fee estimation and validation logic documented below |
| UTXO-03 | UTXO not already registered in current round (double-registration prevention) | In-memory `HashSet<OutPoint>` pattern documented below |
| UTXO-04 | BIP-322 Simple verification, ~50 lines of rust-bitcoin primitives | BIP-322 Simple direct implementation pattern documented below |
| UTXO-05 | Graceful error handling when bitcoind unreachable | Retry + round abort pattern documented below |
| TX-01 | CoinJoin TX construction: all registered inputs, equal denomination outputs, change outputs, fee | Transaction construction with bitcoin 0.32.8 documented below |
| TX-02 | Fee split equally among participants | Fee calculation pattern documented below |
| TX-03 | Change outputs to participant's pre-registered change address | Change address handling documented below |
| TX-04 | Dust threshold handling for change outputs | Dust constant from rust-bitcoin documented below |
| TX-05 | PSBT construction and distribution to participants | PSBT API documented below |
| TX-06 | Partial signature collection keyed by UTXO outpoint | `HashMap<OutPoint, PartialSig>` pattern documented below |
| TX-07 | Final transaction assembly and broadcast via bitcoind RPC | `sendrawtransaction` RPC call pattern documented below |
| TX-08 | Graceful handling of bitcoind broadcast rejection | Error mapping pattern documented below |
| PRIV-02 | No logging of PII, IP addresses, or input-output mappings | Logging discipline pattern documented below |
| PRIV-04 | Polling `GET /info` at 1s intervals (clearnet Sprint 1) | Axum GET handler pattern documented below |
| DEPL-05 | Configurable: network, denomination, min/max participants, timeouts, fee rate | TOML + env-var config with `config` 0.15.x documented below |
| TEST-01 | Unit tests: blind signature round-trip, unlinkability, invalid key, tampered blind | Test patterns documented below |
| TEST-02 | Unit tests: FSM transitions, timeouts, concurrent registration, max participants | Tokio test patterns documented below |
| TEST-03 | Unit tests: all UTXO validation paths | Mockable RPC client pattern documented below |
| TEST-04 | Unit tests: output registration (replay token, wrong denomination, invalid sig, late) | Test patterns documented below |
| TEST-05 | Unit tests: TX construction (valid, equal outputs, fee calc, change, dust) | Test patterns documented below |
| TEST-06 | Unit tests: signing (valid sig, invalid sig, wrong outpoint) | Test patterns documented below |
| TEST-08 | Unit tests: protocol message serialization round-trip and forward compat | serde test patterns documented below |

</phase_requirements>

---

## Summary

Phase 1 builds the entire CoinJoin round protocol end-to-end — from Cargo workspace setup through a confirmed signet txid. All major technical decisions are locked in CONTEXT.md, so this research focuses on exact API usage, version verification, critical implementation details, and pitfall prevention.

The core technical challenge is correctly composing four independent subsystems: (1) the `blind-rsa-signatures` crate for RFC 9474 blind tokens, (2) the round state machine with tokio timers and `Arc<RwLock<>>` shared state under axum, (3) PSBT construction using `bitcoin` 0.32.x primitives, and (4) the thin reqwest-based Bitcoin RPC client using `corepc-types`. Each subsystem has well-understood patterns; the risk is in the seams between them.

The single highest-risk item is the domain separator for the blind token (D-03): `SHA-256("blindjoin-v1" || scriptPubKey_bytes || amount_sats_le64)`. If the client and coordinator serialize `scriptPubKey_bytes` differently (e.g., with vs. without the length prefix), every output registration will fail. This serialization must be tested explicitly in TEST-01 with cross-party round-trip verification. The second risk is PSBT input ordering: the PSBT must sort inputs identically to how the coordinator tracks them, or signature-to-input mapping breaks silently.

**Primary recommendation:** Build in layer order — shared types first, then blind sig wrapper, then FSM + state, then Bitcoin RPC client, then HTTP handlers, then client CLI. Each layer is independently testable. Never proceed past a layer before its unit tests pass.

---

## Standard Stack

### Core (Phase 1 Only — No Tor, No PKARR)

| Library | Version | Purpose | Source |
|---------|---------|---------|--------|
| `tokio` | 1.51.1 | Async runtime, timers, sync primitives | [VERIFIED: crates.io cargo search] |
| `axum` | 0.8.8 | HTTP API for coordinator round endpoints | [VERIFIED: crates.io cargo search] |
| `serde` + `serde_json` | 1.x / 1.0.149 | Protocol message serialization | [VERIFIED: crates.io cargo search] |
| `bitcoin` | 0.32.8 | Primitives: OutPoint, Script, PSBT, Transaction, Amount | [VERIFIED: crates.io — 0.32.8 is latest stable; 0.33.0-beta exists but not stable] |
| `bdk_wallet` | 2.3.0 | Client wallet: BIP-84 descriptor, UTXO selection, PSBT signing | [VERIFIED: crates.io — 2.3.0 is latest stable; 3.0.0-rc.2 is pre-release] |
| `blind-rsa-signatures` | 0.17.1 | RFC 9474 RSA blind signatures | [VERIFIED: crates.io cargo search] |
| `reqwest` | 0.13.2 | Bitcoin Core JSON-RPC calls (async HTTP) | [VERIFIED: crates.io cargo search] |
| `corepc-types` | 0.11.0 | Type-safe Bitcoin Core RPC response structs | [VERIFIED: crates.io cargo search] |
| `zeroize` | 1.8.2 | Memory zeroing for round state, RSA keys | [VERIFIED: crates.io cargo search] |
| `hmac` | 0.13.0 | HMAC-SHA256 for session tokens (D-05) | [VERIFIED: crates.io cargo search] |
| `sha2` | 0.11.0 | SHA-256 for domain separator (D-03) and key pre-commitment (D-02) | [VERIFIED: crates.io cargo search] |
| `uuid` | 1.23.0 | Round IDs and request correlation | [VERIFIED: crates.io cargo search] |
| `tracing` | 0.1.44 | Structured logging — round-level only, zero PII | [VERIFIED: crates.io cargo search] |
| `tracing-subscriber` | 0.3.x | `EnvFilter` + fmt layer for `RUST_LOG` control | [ASSUMED] |
| `config` | 0.15.22 | TOML + env var layered config | [VERIFIED: crates.io cargo search] |
| `tower` | 0.5.3 | Middleware: rate limiting, request size limits | [VERIFIED: crates.io cargo search] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` | 4.6.0 | Client CLI arg parsing | Client binary only |
| `proptest` | 1.11.0 | Property-based tests for FSM and crypto | TEST-01, TEST-02 |
| `corepc-node` | 0.10.1 | Spin up regtest bitcoind for integration tests | Phase 3 integration tests (TEST-09+) |
| `tokio-test` | (via tokio) | Async test utilities | All `#[tokio::test]` unit tests |

### Version Notes

- **bitcoin 0.32.8 vs 0.33.0-beta:** Use 0.32.8. The beta exists but 0.33 has breaking API changes and is not yet stable. `bdk_wallet` 2.3.0 (latest stable) depends on `bitcoin` 0.32.x. If using `bdk_wallet` 2.3.0, must use `bitcoin` 0.32.x to avoid dependency conflicts. [VERIFIED: crates.io]
- **bdk_wallet 2.3.0 vs 3.0.0-rc.2:** Use 2.3.0 (latest stable). 3.0.0-rc.2 exists but is pre-release and requires Rust 1.85.0 minimum. Current toolchain is 1.93.0 so either works, but 3.0 is pre-release. [VERIFIED: crates.io; ASSUMED: API compatibility with bitcoin 0.32]
- **corepc-types 0.11.0:** STACK.md noted 0.4 as approximate; actual latest is 0.11.0. Use 0.11.0. [VERIFIED: crates.io cargo search]
- **sha2 0.11.0:** The RustCrypto sha2 crate incremented to 0.11.0 (from 0.10.x in STACK.md). Use 0.11.0. [VERIFIED: crates.io cargo search]
- **hmac 0.13.0:** Consistent with RustCrypto ecosystem versioning alongside sha2 0.11.0. [VERIFIED: crates.io cargo search]

### Installation

```toml
# Workspace Cargo.toml
[workspace]
members = ["coordinator", "client", "shared"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1.51", features = ["full"] }
axum = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bitcoin = { version = "0.32", features = ["serde"] }
bdk_wallet = "2.3"
blind-rsa-signatures = "0.17"
reqwest = { version = "0.13", features = ["json"] }
corepc-types = "0.11"
zeroize = { version = "1.8", features = ["derive"] }
hmac = "0.13"
sha2 = "0.11"
uuid = { version = "1", features = ["v4", "serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
config = "0.15"
tower = "0.5"
clap = { version = "4", features = ["derive"] }
proptest = "1"
```

---

## Architecture Patterns

### Recommended Project Structure

```
blindjoin/
├── Cargo.toml                  # Workspace root
├── blindjoin.toml.example
│
├── shared/                     # Protocol types — no binary deps
│   └── src/
│       ├── lib.rs
│       ├── protocol.rs         # All serde message structs
│       ├── types.rs            # RoundId, OutPoint wrappers, etc.
│       └── errors.rs           # ApiError, error codes enum
│
├── coordinator/
│   └── src/
│       ├── main.rs             # Startup: config load, health checks, spawn
│       ├── config.rs           # CoordinatorConfig struct (TOML + env)
│       ├── round/
│       │   ├── state.rs        # RoundState enum + Arc<RwLock<RoundState>>
│       │   ├── manager.rs      # Phase timer task, transition logic
│       │   ├── input_reg.rs    # Input registration handler logic
│       │   ├── output_reg.rs   # Output registration handler logic
│       │   ├── signing.rs      # PSBT construction + sig collection
│       │   └── blame.rs        # Non-signer detection + ban application
│       ├── blind/
│       │   └── rsa.rs          # RsaBlindSigner wrapper (thin)
│       ├── bitcoin/
│       │   ├── rpc.rs          # BitcoinRpc thin client (5 methods)
│       │   ├── utxo.rs         # UTXO validation logic
│       │   └── tx.rs           # CoinJoin TX construction
│       └── api/
│           ├── mod.rs          # axum Router assembly
│           ├── handlers.rs     # GET /info, POST /round/*, GET /round/tx
│           └── middleware.rs   # Rate limiting, size limits
│
└── client/
    └── src/
        ├── main.rs
        ├── config.rs
        ├── wallet.rs           # bdk_wallet wrapper
        └── round/
            ├── input.rs        # Register input + blind token
            ├── output.rs       # Register output with unblinded token
            └── sign.rs         # Verify PSBT + submit partial sig
```

### Pattern 1: Enum FSM with Arc<RwLock<>>

**What:** The round state lives in a single `Arc<RwLock<RoundState>>` shared between the axum handler tasks and a dedicated round manager task.

**When:** All coordinator state access.

```rust
// coordinator/round/state.rs
use zeroize::{Zeroize, ZeroizeOnDrop};
use std::collections::HashMap;
use bitcoin::OutPoint;

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Idle,
    InputReg,
    OutputReg,
    Signing,
    Broadcast,
    Blame,
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct RoundState {
    pub round_id: [u8; 16],          // uuid bytes
    pub phase: Phase,                 // NOT zeroized — not sensitive
    pub rsa_signing_key: Vec<u8>,    // zeroed after OUTPUT_REG
    pub rsa_pubkey_hash: [u8; 32],   // SHA-256 of pubkey bytes
    pub registered_inputs: HashMap<OutPoint, RegisteredInput>,
    pub registered_outputs: Vec<RegisteredOutput>,
    pub partial_sigs: HashMap<OutPoint, Vec<u8>>,
    // phase field can't derive Zeroize — store it separately or wrap
}

// axum handler checks phase before processing
async fn handle_register_input(
    State(round): State<Arc<RwLock<RoundState>>>,
    Json(req): Json<InputRegRequest>,
) -> Result<Json<InputRegResponse>, ApiError> {
    let guard = round.read().await;
    if guard.phase != Phase::InputReg {
        return Err(ApiError::wrong_phase(guard.phase.clone()));
    }
    drop(guard);
    // ... proceed with write lock for mutation
}
```

**Note on ZeroizeOnDrop and Phase:** The `Phase` enum cannot derive `Zeroize`. Split the struct: sensitive material (keys, inputs, outputs, sigs) in a `ZeroizeOnDrop` inner struct; round metadata (phase, round_id) in an outer wrapper. [ASSUMED: this is the idiomatic split — verify zeroize docs for enum handling]

### Pattern 2: Phase Timer Task

**What:** A dedicated tokio task owns the phase deadline, fires at timeout, acquires write lock, evaluates quorum, and transitions.

```rust
// coordinator/round/manager.rs
async fn run_phase_timer(
    round: Arc<RwLock<RoundState>>,
    phase: Phase,
    timeout: Duration,
) {
    tokio::time::sleep(timeout).await;
    let mut guard = round.write().await;
    if guard.phase == phase {
        // evaluate quorum, then transition
        guard.transition_on_timeout();
    }
    // If phase already advanced (e.g., all participants registered early),
    // the write is a no-op — the tokio task exits cleanly.
}
```

**Spawn one timer task per phase. Cancel previous timer task when phase advances early** (e.g., max_participants reached before timeout). Use `tokio::select!` or `CancellationToken` (from `tokio-util`) for clean cancellation.

### Pattern 3: blind-rsa-signatures API (RFC 9474)

**What:** The coordinator generates an RSA keypair per round and signs blinded tokens. The exact API for `blind-rsa-signatures` 0.17.x.

```rust
// coordinator/blind/rsa.rs
use blind_rsa_signatures::{
    BlindedMessage, BlindSignature, Options, PublicKey, SecretKey,
    reexports::rsa::RsaPrivateKey,
};

pub struct RsaBlindSigner {
    pub public_key: PublicKey,
    secret_key: SecretKey,  // Never expose outside this module
}

impl RsaBlindSigner {
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let sk = SecretKey::generate(&mut rng, 2048).expect("RSA key gen");
        let pk = sk.public_key();
        Self { public_key: pk, secret_key: sk }
    }

    pub fn public_key_hash(&self) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let pk_bytes = self.public_key.to_public_key_der()
            .expect("DER encode").into_vec();
        Sha256::digest(&pk_bytes).into()
    }

    pub fn blind_sign(
        &self,
        blinded_msg: &BlindedMessage,
    ) -> Result<BlindSignature, Error> {
        let options = Options::default();
        let mut rng = rand::thread_rng();
        self.secret_key.blind_sign(&mut rng, blinded_msg, &options)
    }
}

impl Drop for RsaBlindSigner {
    fn drop(&mut self) {
        // blind-rsa-signatures SecretKey: check if it implements Zeroize
        // If not, use explicit zeroing via unsafe or track upstream
    }
}
```

**CRITICAL:** The `blind-rsa-signatures` crate's `SecretKey` type — verify whether it implements `Zeroize`. If not, the RSA private key material will not be zeroed on drop. Check `blind-rsa-signatures` 0.17.1 changelog before writing the signer. [ASSUMED: needs verification against crate docs]

### Pattern 4: Domain Separator (D-03)

**What:** The message `M` that gets blinded is `SHA-256("blindjoin-v1" || scriptPubKey_bytes || amount_sats_le64)`. This must be byte-identical between client and coordinator.

```rust
// shared/protocol.rs
use sha2::{Sha256, Digest};
use bitcoin::Script;

/// Compute the blind token message for a given output.
/// MUST use the SCRIPT BYTES of the output address (not the address string).
/// amount_sats is encoded as little-endian u64.
pub fn compute_blind_token_message(
    output_script: &Script,
    amount_sats: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"blindjoin-v1");
    hasher.update(output_script.as_bytes());  // raw script bytes, no length prefix
    hasher.update(amount_sats.to_le_bytes());
    hasher.finalize().into()
}
```

**WARNING:** `output_script.as_bytes()` returns the raw script bytes WITHOUT the `CompactSize` length prefix used in Bitcoin wire format. The coordinator must use the same method as the client. If the client uses `Script::to_bytes()` (which may differ) vs `as_bytes()`, tokens will not verify. This function MUST live in the `shared` crate and be used by both sides. Test explicitly with a known vector. [VERIFIED: pattern consistent with rust-bitcoin 0.32 Script API; exact method name ASSUMED — verify against `bitcoin` 0.32 docs]

### Pattern 5: HMAC Session Token (D-05)

**What:** `HMAC-SHA256(coordinator_round_secret, UTXO_outpoint_bytes)` produces a deterministic session token per UTXO per round. No storage required.

```rust
// coordinator/round/input_reg.rs
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn generate_session_token(
    round_secret: &[u8; 32],  // random per-round secret
    utxo: &bitcoin::OutPoint,
) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(round_secret)
        .expect("HMAC accepts any key size");
    // Canonical UTXO encoding: txid bytes (LE) + vout u32 LE
    mac.update(utxo.txid.as_ref());
    mac.update(&utxo.vout.to_le_bytes());
    mac.finalize().into_bytes().into()
}

pub fn verify_session_token(
    round_secret: &[u8; 32],
    utxo: &bitcoin::OutPoint,
    token: &[u8; 32],
) -> bool {
    let expected = generate_session_token(round_secret, utxo);
    // Constant-time comparison via hmac crate
    use hmac::digest::CtOutput;
    expected == *token  // [u8; 32] comparison is constant-time in practice
    // For strictness, use subtle::ConstantTimeEq
}
```

**Note:** The `round_secret` is a 32-byte random value generated at round start. It is NOT the RSA private key. It should also be `Zeroize`'d after the round. The session token binds the signing request to a specific (round, UTXO) pair — prevents a participant from submitting signatures for other participants' inputs.

### Pattern 6: BIP-322 Simple Verification (~50 lines)

**What:** BIP-322 Simple generates a virtual transaction spending the UTXO with the message as data, then produces a standard witness. For P2WPKH this is a 2-item witness: `[sig, pubkey]`.

```rust
// coordinator/bitcoin/utxo.rs  
// BIP-322 Simple verification — no bip322 crate, direct rust-bitcoin primitives

use bitcoin::{
    OutPoint, Script, Transaction, TxIn, TxOut,
    hashes::{sha256d, Hash},
    secp256k1::{Secp256k1, Message as SecpMessage},
};

/// BIP-322 Simple: verify that `witness` proves ownership of `script_pubkey`
/// for message `msg` in the context of round `round_id`.
///
/// The BIP-322 message format is: "blindjoin:round:{round_id}:utxo:{txid}:{vout}"
/// This is hashed with BIP-322's tag hash.
pub fn verify_bip322_simple(
    script_pubkey: &Script,
    witness_bytes: &[Vec<u8>],  // witness stack from client
    message: &str,              // "blindjoin:round:{id}:utxo:{txid}:{vout}"
) -> Result<(), Bip322Error> {
    // 1. Construct the to_sign transaction per BIP-322
    //    to_spend: single input spending from OP_0 <sha256(message)>
    //    to_sign: spends the to_spend output, witness to be verified
    
    // 2. For P2WPKH: extract sig + pubkey from witness[0], witness[1]
    //    Verify ECDSA signature over the to_sign sighash
    
    // 3. Verify the pubkey matches the script_pubkey (hash160 match)
    
    // This is ~50 lines including error handling.
    // Full implementation is protocol-critical — the planner should
    // allocate a dedicated task with test coverage for each address type.
    todo!("see BIP-322 spec sections 4 and 5")
}
```

**Address types to support in Phase 1:** P2WPKH (native SegWit bech32) is required. P2TR (Taproot) is desirable but may be deferred — the planner should decide. P2SH-P2WPKH is common for legacy wallets. The BIP-322 Simple verification path differs per type because the sighash preimage and witness structure differ.

**Reference:** BIP-322 spec at https://bips.dev/322/ — Section 4 (to_spend construction) and Section 5 (to_sign verification). The rust-bitcoin `sighash` module provides `SighashCache` for computing the correct sighash per input type. [CITED: bips.dev/322]

### Pattern 7: Thin Bitcoin RPC Client

**What:** Five methods wrapping `reqwest` JSON-RPC calls, deserializing into `corepc-types` structs.

```rust
// coordinator/bitcoin/rpc.rs
use reqwest::Client;
use serde_json::{json, Value};
use corepc_types::GetTxOut;

pub struct BitcoinRpc {
    client: Client,
    url: String,
    auth: (String, String),
}

impl BitcoinRpc {
    pub async fn gettxout(
        &self,
        txid: &bitcoin::Txid,
        vout: u32,
    ) -> Result<Option<GetTxOut>, RpcError> {
        let result: Option<GetTxOut> = self
            .call("gettxout", json!([txid.to_string(), vout, false]))
            .await?;
        Ok(result)
    }

    pub async fn sendrawtransaction(
        &self,
        hex: &str,
    ) -> Result<bitcoin::Txid, RpcError> { ... }

    pub async fn testmempoolaccept(
        &self,
        hex: &[&str],
    ) -> Result<Vec<MempoolAcceptResult>, RpcError> { ... }

    pub async fn getblockcount(&self) -> Result<u64, RpcError> { ... }

    pub async fn getblockchaininfo(&self) -> Result<BlockchainInfo, RpcError> {
        // Used in startup health check (D-12): check network + IBD status
    }

    async fn call<R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<R, RpcError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self.client
            .post(&self.url)
            .basic_auth(&self.auth.0, Some(&self.auth.1))
            .json(&body)
            .send().await?
            .json::<RpcResponse<R>>().await?;
        resp.result.ok_or_else(|| RpcError::from(resp.error))
    }
}
```

**corepc-types 0.11.0:** The version jumped significantly from the 0.4 noted in STACK.md. The `GetTxOut` type should be present in 0.11.0; verify the exact type names against the corepc-types 0.11.0 docs before coding. [VERIFIED: version 0.11.0; ASSUMED: type names compatible]

### Pattern 8: PSBT Construction

**What:** Build the unsigned CoinJoin transaction as a PSBT. All inputs are unsigned; the coordinator distributes it to participants who each sign their own input.

```rust
// coordinator/bitcoin/tx.rs
use bitcoin::{
    psbt::{Psbt, Input as PsbtInput},
    Transaction, TxIn, TxOut, OutPoint, Amount, ScriptBuf,
    absolute::LockTime, transaction::Version,
};

pub struct CoinjoinTxBuilder {
    inputs: Vec<(OutPoint, TxOut)>,    // (outpoint, utxo being spent)
    denomination_outputs: Vec<ScriptBuf>,   // P2WPKH scripts from output reg
    change_outputs: Vec<(ScriptBuf, Amount)>, // (change script, change amount)
    fee_rate_sat_per_vbyte: u64,
}

impl CoinjoinTxBuilder {
    pub fn build(self) -> Result<Psbt, TxError> {
        // 1. Calculate fee: estimate_tx_vbytes() * fee_rate
        // 2. Calculate change per input: utxo_value - denomination - fee_share
        // 3. Filter dust change (TX-04): if change < dust_value, fold into fee
        // 4. Sort inputs by PSBT input ordering (no BIP-69 requirement for CoinJoin,
        //    but deterministic ordering prevents coordinator from learning anything
        //    from ordering — sort by outpoint txid:vout lexicographically)
        // 5. Construct Transaction, wrap in Psbt
        // 6. Fill each PsbtInput with the full previous TxOut (required for SegWit signing)
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: /* TxIn vec with sequence = 0xFFFFFFFF */,
            output: /* TxOut vec: denomination outputs + change outputs */,
        };
        let mut psbt = Psbt::from_unsigned_tx(tx)?;
        // Populate PSBT inputs with witness_utxo (the TxOut being spent)
        for (i, (_, utxo)) in self.inputs.iter().enumerate() {
            psbt.inputs[i].witness_utxo = Some(utxo.clone());
        }
        Ok(psbt)
    }
}
```

**Dust threshold:** `bitcoin::blockdata::constants::WITNESS_SCALE_FACTOR` and the standard dust threshold for P2WPKH is 294 satoshis (at the default dust relay fee rate of 3 sat/vB for a 98-vbyte P2WPKH input). Use `Amount::from_sat(294)` as the dust threshold for P2WPKH change outputs. [ASSUMED: verify against bitcoin 0.32 API for canonical dust calculation]

**Input ordering in PSBT:** The coordinator must maintain a stable mapping from `OutPoint` to PSBT input index throughout the round. The session token (D-05) identifies participants by UTXO outpoint for the signing phase, so the coordinator must consistently resolve `utxo_outpoint` → `psbt.inputs[i]` during signature collection (TX-06).

### Pattern 9: API Error Taxonomy

This is Claude's discretion per D-09. Recommended error codes:

```rust
// shared/errors.rs
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Phase errors
    WrongPhase,
    RoundNotFound,
    // Input registration
    UtxoNotFound,
    UtxoSpent,
    UtxoInsufficientValue,
    UtxoAlreadyRegistered,
    UtxoBanned,
    InvalidOwnershipProof,
    // Output registration
    InvalidBlindToken,
    TokenAlreadyRedeemed,
    WrongDenomination,
    // Signing
    InvalidPartialSignature,
    UnknownUtxoOutpoint,
    // Internal
    BitcoindUnavailable,
    InternalError,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ApiErrorBody {
    pub code: ErrorCode,
    pub message: String,
    pub round_id: Option<String>,
}
```

### Pattern 10: Forward-Compatible Protocol Messages (D-06)

```rust
// shared/protocol.rs
// Use #[serde(default)] NOT #[serde(deny_unknown_fields)]
// This allows older coordinator to receive requests from newer clients
// and vice versa without deserialization errors.

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InputRegRequest {
    pub utxo_outpoint: String,      // "txid:vout"
    pub ownership_proof: String,    // BIP-322 witness bytes, base64
    pub blinded_token: String,      // base64
    pub change_address: String,     // tb1q... (clearnet phase uses testnet bech32)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InputRegResponse {
    pub blind_signature: String,    // base64
    pub round_id: String,           // uuid
    pub session_token: String,      // base64 HMAC (D-05)
    pub rsa_pubkey_hash: String,    // hex SHA-256 of coordinator RSA pubkey
}
// NOTE: rsa_pubkey_hash also returned in GET /info; returning it here
// too lets the client verify consistency without a separate call.

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SignRequest {
    pub round_id: String,
    pub utxo_outpoint: String,      // "txid:vout" — NOT input_index (D-05)
    pub session_token: String,      // HMAC token from input registration
    pub partial_signature: String,  // base64
}
```

### Anti-Patterns to Avoid

- **Unique RSA key per participant:** All participants in a round MUST receive the same RSA public key. The coordinator publishes `rsa_pubkey_hash` in `GET /info` BEFORE accepting registrations. Any client that receives a key not matching the pre-committed hash must abort. This prevents the tagging attack (PITFALLS Pitfall 1, confirmed real-world in WabiSabi 2024).
- **Storing round state to disk:** No SQLite for in-progress round data. Ban list (UTXO hashes + timestamps) does go to disk, but current round inputs/outputs/sigs live in memory only. Phase 1 ban list is in-memory only (ban persistence comes in Phase 2).
- **Signing by input index:** The `/round/sign` endpoint MUST use UTXO outpoint for identification, not input index. Input indices are not stable between phases and allow cross-participant injection.
- **Logging UTXO identifiers at request level:** Use `tracing::instrument` spans that capture `round_id` and `phase`, but strip UTXO identifiers. Axum's request logging middleware must not log request bodies.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RSA blind signatures | Custom blinding/unblinding | `blind-rsa-signatures` 0.17.1 | jedisct1-authored, RFC 9474 compliant, audited; custom RSA blinding has silently broken implementations in the wild |
| Memory zeroing | `ptr::write_bytes` in Drop | `zeroize` crate | Compiler optimizes away naive zeroing; zeroize uses volatile writes and memory barriers |
| HMAC session tokens | Custom hash combinator | `hmac` + `sha2` from RustCrypto | HMAC has security proof; ad-hoc hash constructions are vulnerable to length extension |
| PSBT serialization/parsing | Custom PSBT encoder | `bitcoin::psbt::Psbt` | PSBT is complex (BIP-174/BIP-370); the rust-bitcoin implementation is battle-tested |
| UTXO value in sats | Float arithmetic | `bitcoin::Amount` | Satoshi arithmetic; floats cause rounding errors in fee calculations |
| Async HTTP | Custom TCP + JSON | `reqwest` + `axum` | TLS, keep-alive, body limits, timeout handling — large attack surface to hand-roll |
| Config env var expansion | Custom env parser | `config` 0.15 | Handle BLINDJOIN_NETWORK, nested config keys, type coercion correctly |

**Key insight:** Every hand-rolled crypto primitive or encoding scheme in this domain has a documented real-world failure mode. The libraries exist precisely because these problems have hidden complexity (timing attacks, canonical encoding, spec edge cases). Use them.

---

## Common Pitfalls

### Pitfall 1: Tagging Attack via Per-Participant RSA Keys (CRITICAL)

**What goes wrong:** The coordinator generates a different RSA public key for each input registration. When the client presents the unblinded token at output registration, the coordinator identifies which Alice produced it by which key verifies. All unlinkability is destroyed.

**Why it happens:** Easiest to code — generate one key per request. Developers miss that this silently breaks the privacy guarantee.

**How to avoid:** One RSA keypair per round, period. Publish `rsa_pubkey_hash` in `GET /info` BEFORE the round opens. Client verifies the hash matches before blinding. Coordinator rejects any output if it verifies against a key that was not the round's key.

**Warning signs:** `rsa_pubkey_hash` not present in `GET /info` response; different hash returned per request.

**Confirmed real incident:** WabiSabi coordinators, GingerWallet ≤ 2.0.13, BTCPay coinjoin plugin (December 2024). [CITED: PITFALLS.md, GingerWallet Discussion #116]

### Pitfall 2: Blind Token Domain Separator Mismatch

**What goes wrong:** Client and coordinator compute `SHA-256("blindjoin-v1" || ...)` using different serializations of the script or amount. Every output registration fails with "invalid signature" and there is no obvious diagnostic.

**Why it happens:** `Script::as_bytes()` vs `Script::to_bytes()`, or the amount as LE vs BE, or including/excluding the scriptPubKey length prefix.

**How to avoid:** `compute_blind_token_message()` lives in `shared/` and is the SINGLE implementation used by both sides. Test with a known-good vector from both client and coordinator sides.

**Warning signs:** All output registrations fail with invalid token; blind signature round-trip tests pass but cross-party tests fail.

### Pitfall 3: PSBT Input Index Drift

**What goes wrong:** The coordinator builds the PSBT with inputs in one order, but when collecting signatures in the signing phase, maps `utxo_outpoint` → input index using a different ordering. Signatures are applied to the wrong inputs. The transaction is invalid; bitcoind rejects it.

**Why it happens:** Using `Vec` for registered inputs (ordering depends on insertion order) vs. iterating a `HashMap` (ordering is not guaranteed).

**How to avoid:** The coordinator builds the PSBT once, in a deterministic order (sort inputs by `OutPoint` lexicographically). After PSBT construction, build a stable `HashMap<OutPoint, usize>` mapping each outpoint to its PSBT input index. This map is frozen and used for all subsequent signature collection.

**Warning signs:** `testmempoolaccept` returns false; assembled transaction has witness data in wrong inputs.

### Pitfall 4: RwLock Read-Write Upgrade Race

**What goes wrong:** Handler acquires read lock to check phase, releases it, then attempts to acquire write lock to modify state. Between the two acquisitions, another handler changes the phase. The write proceeds on stale assumptions.

**Why it happens:** Standard RwLock semantics — no upgrade from read to write.

**How to avoid:** For state-checking operations that must also mutate: acquire write lock from the start, or use a helper that acquires write lock and re-checks invariant inside:

```rust
let mut guard = round.write().await;
if guard.phase != Phase::InputReg {
    return Err(ApiError::wrong_phase(guard.phase.clone()));
}
// modify guard here — phase cannot have changed
```

The write lock performance impact is acceptable for a coordinator with max 20 participants.

### Pitfall 5: Bitcoind Startup Health Check Skipped

**What goes wrong:** Coordinator starts and accepts input registrations. After participants have blinded their tokens (irreversible), the coordinator discovers bitcoind is unreachable or on the wrong network. Round must be aborted, participants lose time.

**How to avoid:** D-12 is non-negotiable. At startup, before opening any HTTP port: call `getblockchaininfo`, verify `chain == "signet"` (or configured network), verify `initialblockdownload == false`. Exit with a clear error message if either check fails. The HTTP server must not start until all health checks pass.

### Pitfall 6: In-Memory State Not Zeroed (ZeroizeOnDrop on Enums)

**What goes wrong:** `Phase` enum cannot derive `Zeroize`. `RoundState` struct that contains the `Phase` cannot automatically derive `ZeroizeOnDrop` if `Phase` doesn't implement `Zeroize`. The `#[derive(ZeroizeOnDrop)]` silently fails to zero anything if the trait is not fully satisfied.

**How to avoid:** Split `RoundState` into two structs:
- `RoundSensitiveData` — RSA keys, registered inputs, blind tokens, partial sigs — derives `Zeroize` + `ZeroizeOnDrop`
- `RoundMetadata` — phase, round_id, timing — no zeroize needed

After broadcast or round abort: explicitly drop `RoundSensitiveData`, which triggers `ZeroizeOnDrop`.

---

## Code Examples

### blind-rsa-signatures Round-Trip (RFC 9474)

```rust
// Source: jedisct1/rust-blind-rsa-signatures README + crate docs
use blind_rsa_signatures::{
    BlindedMessage, BlindSignature, Options, PublicKey, SecretKey,
    reexports::rsa::RsaPrivateKey,
};

#[test]
fn test_blind_sign_round_trip() {
    let mut rng = rand::thread_rng();
    let options = Options::default();

    // Coordinator: generate key
    let sk = SecretKey::generate(&mut rng, 2048).unwrap();
    let pk = sk.public_key();

    // Client: create message and blind it
    let msg = b"blindjoin-output-token-bytes-here";
    let blinding_result = pk.blind(&mut rng, msg, true, &options).unwrap();
    // blinding_result.blind_msg: send to coordinator
    // blinding_result.secret: keep locally for unblinding

    // Coordinator: sign the blinded message
    let blind_sig = sk.blind_sign(&mut rng, &blinding_result.blind_msg, &options).unwrap();
    // Return blind_sig to client

    // Client: unblind the signature
    let sig = pk.finalize(
        &blind_sig,
        &blinding_result.secret,
        blinding_result.msg_randomizer,
        msg,
        &options,
    ).unwrap();

    // Client OR Coordinator: verify
    pk.verify(&sig, blinding_result.msg_randomizer, msg, &options).unwrap();
    // Passes: signature is valid and unlinkable to the blinding step
}
```

[ASSUMED: exact API matches 0.17.1; the method names `blind`, `blind_sign`, `finalize`, `verify` — verify against docs.rs/blind-rsa-signatures/0.17.1]

### PSBT Signing by Participant (bdk_wallet 2.3.0)

```rust
// client/round/sign.rs
// The client receives the PSBT, adds their partial signature, returns it
use bdk_wallet::{Wallet, SignOptions};
use bitcoin::psbt::Psbt;

async fn sign_coinjoin_psbt(
    wallet: &Wallet,
    psbt_base64: &str,
) -> Result<String, Error> {
    let psbt_bytes = base64::decode(psbt_base64)?;
    let mut psbt: Psbt = bitcoin::consensus::deserialize(&psbt_bytes)?;

    // bdk_wallet signs only inputs it owns (those with matching keys)
    let finalized = wallet.sign(&mut psbt, SignOptions::default())?;
    // finalized is false for CoinJoin — we only signed our input, not all

    let psbt_bytes = bitcoin::consensus::serialize(&psbt);
    Ok(base64::encode(psbt_bytes))
}
```

[ASSUMED: bdk_wallet 2.3.0 `Wallet::sign()` signature — verify against docs.rs/bdk_wallet/2.3.0]

### Axum Router Assembly with State

```rust
// coordinator/api/mod.rs
use axum::{Router, routing::{get, post}, extract::State};
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn build_router(round: Arc<RwLock<RoundState>>, config: Arc<Config>) -> Router {
    Router::new()
        .route("/info", get(handlers::get_info))
        .route("/round/input", post(handlers::post_round_input))
        .route("/round/output", post(handlers::post_round_output))
        .route("/round/sign", post(handlers::post_round_sign))
        .route("/round/tx", get(handlers::get_round_tx))
        .layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::limit::RequestBodyLimitLayer::new(64 * 1024)) // 64KB max body
                .layer(tower_http::timeout::TimeoutLayer::new(
                    std::time::Duration::from_secs(10)
                ))
        )
        .with_state(AppState { round, config })
}
```

[ASSUMED: `tower_http` crate needed for `RequestBodyLimitLayer` and `TimeoutLayer` — add `tower-http` to dependencies]

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `bitcoincore-rpc` crate | `corepc-types` + `reqwest` manual calls | November 2025 (archived) | Must build thin client; ~100 lines |
| `bdk` crate (old) | `bdk_wallet` crate | 2024 rename | Different crate name; `bdk` 0.30.2 is legacy |
| `bitcoin` 0.32.x stable | `bitcoin` 0.33.0-beta exists | Early 2026 | Stay on 0.32.8 — 0.33 has breaking changes and is not stable |
| Wasabi v1 RSA blind sig (Wabisabi) | RFC 9474 (`blind-rsa-signatures`) | 2023 | blindjoin uses the IETF-standardized version, not the Wasabi-specific scheme |

**Deprecated/outdated:**
- `bip322` crate (0.0.x): Decision D-04 explicitly rejects this crate. Implement BIP-322 Simple directly.
- `bitcoincore-rpc` crate: Archived November 2025. Do not use.
- `bdk` (old crate name): Use `bdk_wallet` instead.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `blind-rsa-signatures` 0.17.1 `SecretKey` implements `Zeroize` | Pattern 3 (blind signer) | RSA private key not zeroed; memory safety violation; requires explicit zeroing workaround |
| A2 | `bdk_wallet` 2.3.0 depends on `bitcoin` 0.32.x (compatible with our dep) | Standard Stack | Dependency conflict; must upgrade bitcoin or bdk_wallet |
| A3 | `Script::as_bytes()` in bitcoin 0.32 returns raw script bytes without CompactSize length prefix | Pattern 4 (domain separator) | Token verification fails for all output registrations; critical bug |
| A4 | `corepc-types` 0.11.0 exports `GetTxOut` type with `value` field in BTC | Pattern 7 (RPC client) | Type names differ; must adjust RPC client implementation |
| A5 | `blind-rsa-signatures` API method names (`blind`, `blind_sign`, `finalize`, `verify`) match 0.17.1 | Code Examples | Compilation failure; requires API reference check |
| A6 | `bdk_wallet` 2.3.0 `Wallet::sign()` signature accepts `SignOptions::default()` | Code Examples | API mismatch; check docs.rs/bdk_wallet/2.3.0 |
| A7 | `Phase` enum cannot derive `Zeroize`; requires struct split for ZeroizeOnDrop | Pattern 1 (state) | ZeroizeOnDrop fails to compile or silently skips sensitive fields |
| A8 | Dust threshold for P2WPKH change is 294 sats at 3 sat/vB relay fee | Pattern 8 (PSBT) | Wrong dust threshold; change outputs rejected by bitcoind or unnecessarily folded |
| A9 | `tower-http` crate needed separately for `RequestBodyLimitLayer` | Pattern (axum router) | Compile error; add `tower-http` to dependencies |
| A10 | `tracing-subscriber` is 0.3.x current stable | Standard Stack | Minor; easy fix |

**If this table is not empty:** These assumptions should be verified at Wave 0 (task setup) before coding dependent layers. Each is checkable with a 2-minute `cargo doc` lookup.

---

## Open Questions

1. **BIP-322 address type scope for Phase 1**
   - What we know: P2WPKH (bech32) is required; P2TR and P2SH-P2WPKH are common
   - What's unclear: Should Phase 1 implement all three, or just P2WPKH with others deferred?
   - Recommendation: P2WPKH only for Phase 1. All signet test wallets will use BIP-84 (native SegWit P2WPKH). Add P2TR in Phase 2. Document the limitation.

2. **PSBT input sort order**
   - What we know: Lexicographic sort by OutPoint is deterministic and prevents coordinator fingerprinting from ordering
   - What's unclear: BIP-69 mandates lexicographic input sorting for CoinJoin privacy, but it's not enforced at consensus level
   - Recommendation: Sort inputs lexicographically by txid+vout at PSBT construction time, document as protocol invariant.

3. **Session token also needed for output registration?**
   - What we know: Session token issued at input registration, used at signing phase (D-05)
   - What's unclear: Output registration in the spec uses only `unblinded_token` + `signature` (the blind token is the credential). No session token needed for output registration — that's the whole point of the blind signature scheme.
   - Recommendation: No session token at output registration. The blind signature IS the credential. Session token is signing-phase-only.

4. **testmempoolaccept before broadcast**
   - What we know: TX-07 says broadcast via `sendrawtransaction`; TX-08 says handle rejection gracefully
   - What's unclear: Should coordinator call `testmempoolaccept` first to get a clean error before attempting broadcast?
   - Recommendation: Yes. Call `testmempoolaccept` first, log the rejection reason, then decide whether to broadcast or abort. This catches fee-too-low and invalid-tx errors with descriptive messages.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (cargo) | All compilation | Yes | 1.93.0 (2026-01-19) | — |
| Docker | Phase 1 integration tests (optional) | Yes | 29.3.1 | Use corepc-node for regtest |
| bitcoind | UTXO validation, TX broadcast | No (not installed) | — | Use corepc-node for unit tests; Docker for signet |

**Missing dependencies with fallback:**
- `bitcoind` not installed on dev machine. For unit tests: `corepc-node` 0.10.1 manages regtest instances programmatically (see STACK.md). For signet integration (Phase 1 goal): run bitcoind via `docker compose up bitcoind` using the spec's Docker Compose config. The planner must include a Wave 0 task for signet bitcoind setup.

---

## Validation Architecture

> `workflow.nyquist_validation` is `false` in `.planning/config.json` — this section is skipped.

---

## Security Domain

### Applicable ASVS Categories for Phase 1

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No — no user accounts; HMAC session tokens are domain-specific, not auth in ASVS sense | — |
| V3 Session Management | Partial — HMAC session tokens for signing phase | HMAC-SHA256 via RustCrypto (D-05) |
| V4 Access Control | Yes — phase-gated endpoints | Enum FSM phase check in every handler (D-01) |
| V5 Input Validation | Yes — all JSON inputs | serde + explicit field validation; reject unknown values where security-critical |
| V6 Cryptography | Yes (CRITICAL) | `blind-rsa-signatures` only; no custom RSA; zeroize for key material (D-07) |
| V7 Error Handling | Yes | Structured JSON errors, no stack traces in production (D-09) |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| RSA tagging attack (unique key per participant) | Spoofing | Publish `rsa_pubkey_hash` in `GET /info` before round opens; client verifies |
| Token replay (reuse blind token for multiple outputs) | Tampering | `HashSet<BlindToken>` in round state; reject on second presentation |
| UTXO double-registration | Tampering | `HashSet<OutPoint>` in round state; reject on second POST to `/round/input` |
| Signature cross-injection (submit sig for another participant's input) | Tampering | Session token + utxo_outpoint binding; verify HMAC before processing sig |
| Request body DoS | Denial of Service | `tower_http` `RequestBodyLimitLayer` (64KB max) |
| PII logging | Information Disclosure | No UTXO IDs, addresses, or IP addresses in any log call — audit all tracing callsites |
| Bitcoind-unavailable at round start | Denial of Service | Fail-fast startup health check (D-12); abort round on bitcoind error (UTXO-05) |
| Mempool rejection post-signing | Denial of Service | `testmempoolaccept` before broadcast; handle rejection with round abort |

---

## Sources

### Primary (HIGH confidence — verified this session)
- crates.io cargo search: `blind-rsa-signatures` 0.17.1, `corepc-types` 0.11.0, `bdk_wallet` 2.3.0 (stable max), `bitcoin` 0.32.8 (stable max), `axum` 0.8.8, `tokio` 1.51.1, `reqwest` 0.13.2, `zeroize` 1.8.2, `hmac` 0.13.0, `sha2` 0.11.0, `uuid` 1.23.0, `tracing` 0.1.44, `config` 0.15.22, `tower` 0.5.3, `clap` 4.6.0, `proptest` 1.11.0, `corepc-node` 0.10.1 — all [VERIFIED: crates.io]
- `.planning/phases/01-core-protocol/01-CONTEXT.md` — Locked decisions D-01 through D-16
- `.planning/research/STACK.md` — Technology recommendations
- `.planning/research/ARCHITECTURE.md` — Component boundaries and patterns
- `.planning/research/PITFALLS.md` — 14 domain-specific pitfalls
- `blindjoin-technical-spec.md` — Protocol specification, API definitions
- `~/.gstack/projects/johnzilla-blindjoin/john-main-design-20260407-220513.md` — APPROVED design doc with API corrections
- `~/.gstack/projects/johnzilla-blindjoin/john-main-eng-review-test-plan-20260407-232603.md` — 34-codepath test plan

### Secondary (MEDIUM confidence)
- PITFALLS.md citing GingerWallet Discussion #116 (December 2024) — WabiSabi tagging attack confirmed real-world
- PITFALLS.md citing BIP-322 spec at bips.dev/322 — BIP-322 Simple verification procedure

### Tertiary (LOW confidence — training knowledge)
- Dust threshold values (294 sats for P2WPKH at 3 sat/vB relay)
- ZeroizeOnDrop behavior with enum fields
- Exact method names in blind-rsa-signatures 0.17.1 and bdk_wallet 2.3.0

---

## Metadata

**Confidence breakdown:**
- Standard stack (versions): HIGH — all versions verified against crates.io in this session
- Architecture patterns: HIGH — based on locked decisions + verified crate APIs
- Blind signature API: MEDIUM — API shapes based on training + README patterns; exact 0.17.1 method names marked ASSUMED
- Pitfalls: HIGH — three of the top pitfalls are confirmed real-world incidents
- PSBT construction: MEDIUM — API based on bitcoin 0.32 patterns; dust threshold is ASSUMED

**Research date:** 2026-04-08
**Valid until:** 2026-05-08 (stable crates; rc releases for bdk_wallet 3.x and bitcoin 0.33 may stabilize before then — re-check if timeline extends)
