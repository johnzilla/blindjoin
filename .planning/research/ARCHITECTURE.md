# Architecture Patterns

**Domain:** CoinJoin Coordinator — Chaumian blind-signature Bitcoin privacy protocol
**Researched:** 2026-04-07
**Confidence:** MEDIUM-HIGH (ZeroLink/Wasabi v1 protocol well-documented; Rust-native PKARR+Arti integration is novel and lightly documented)

---

## Recommended Architecture

blindjoin is composed of three independently deployable binaries that communicate through well-defined interfaces. A fourth component (the liquidity bot) is structurally identical to the client CLI.

```
┌─────────────────────────────────────────────────────────────────┐
│                        Docker Compose                           │
│                                                                 │
│  ┌─────────────┐    ┌──────────────────────────────────┐       │
│  │  bitcoind   │◄───│           coordinator             │       │
│  │  (signet)   │    │                                   │       │
│  └─────────────┘    │  ┌────────────┐ ┌─────────────┐  │       │
│                     │  │ Round      │ │ Blind-sig   │  │       │
│                     │  │ State Mgr  │ │ Issuer      │  │       │
│                     │  └────────────┘ └─────────────┘  │       │
│                     │  ┌────────────┐ ┌─────────────┐  │       │
│                     │  │ UTXO       │ │ Ban List    │  │       │
│                     │  │ Validator  │ │ Manager     │  │       │
│                     │  └────────────┘ └─────────────┘  │       │
│                     │  ┌────────────┐ ┌─────────────┐  │       │
│                     │  │ HTTP API   │ │ PKARR       │  │       │
│                     │  │ (axum)     │ │ Publisher   │  │       │
│                     │  └────────────┘ └─────────────┘  │       │
│                     │  ┌────────────────────────────┐   │       │
│                     │  │   Arti Onion Service       │   │       │
│                     │  │   (transport layer)        │   │       │
│                     │  └────────────────────────────┘   │       │
│                     └──────────────────────────────────┘       │
│                                                                 │
│  ┌──────────────────────────────────────────────────────┐      │
│  │                   liquidity-bot                       │      │
│  │  (client CLI logic, auto-joins rounds for cold-start) │      │
│  └──────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────┘

                        ▲  Tor (.onion)
                        │
              ┌──────────────────┐
              │   client CLI     │
              │                  │
              │  fresh Tor       │
              │  circuit per     │
              │  phase           │
              └──────────────────┘
```

---

## Component Boundaries

### Coordinator

The coordinator is the server process. It owns all mutable round state and is the single authority on round progress. It never stores PII.

| Sub-Component | Responsibility | Communicates With |
|---------------|---------------|-------------------|
| **HTTP API** (axum) | Exposes JSON-over-HTTP endpoints for all round phases; receives requests from clients via Arti | Round State Manager, Blind-sig Issuer, UTXO Validator |
| **Round State Manager** | Owns the round state machine (IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE); drives phase transitions on timeout or quorum | HTTP API, UTXO Validator, Blind-sig Issuer, Ban List Manager, Bitcoin RPC |
| **Blind-sig Issuer** | Generates RSA keypair per round; signs blinded tokens during input registration; verifies unblinded tokens during output registration | Round State Manager, HTTP API |
| **UTXO Validator** | Calls Bitcoin RPC to confirm UTXO existence, confirmation status, and value; verifies BIP-322 ownership proofs | bitcoind (JSON-RPC), Round State Manager |
| **Ban List Manager** | Tracks temporarily banned UTXOs (7-hour window); records non-signers after blame detection | Round State Manager |
| **PKARR Publisher** | Signs and publishes DNS packets to Mainline DHT on startup and periodically; announces .onion address, denomination, and round status | DHT (via pkarr relay), Round State Manager (round params) |
| **Arti Onion Service** | Creates and maintains .onion hidden service; passes inbound TCP streams to HTTP API | arti-client crate, HTTP API |

### Client CLI

The client is a stateless command-line tool that executes one round participation. It uses fresh Tor circuits per phase to prevent the coordinator from correlating Alice (input) with Bob (output).

| Sub-Component | Responsibility | Communicates With |
|---------------|---------------|-------------------|
| **PKARR Resolver** | Looks up coordinator .onion address and round parameters from DHT | DHT (via pkarr relay) |
| **BDK Wallet** | Key management, UTXO selection, PSBT construction, BIP-322 signature generation | local keystore |
| **Blind Token Handler** | Blinds coordinator's RSA challenge during input registration; unblinds signed token for output registration | Coordinator HTTP API |
| **Tor Circuit Manager** | Uses arti-client to create separate Tor circuits for input-reg phase (Alice) vs. output-reg phase (Bob) | Coordinator .onion via Arti |
| **Phase Orchestrator** | Drives client through discover → register-input → blind → register-output → verify-tx → sign sequence | all above sub-components |

### Liquidity Bot

Structurally identical to the client CLI. Runs as a Docker service, configured with signet keys and a target denomination. Polls coordinator PKARR record and auto-joins rounds. No unique architecture; it is a wrapper around the client library.

---

## Data Flow

### Round Lifecycle Data Flow

```
[Coordinator Startup]
  1. Coordinator generates RSA keypair (per-round material — one keypair per round)
  2. PKARR Publisher signs record {.onion, denomination, round params} and pushes to DHT
  3. Round State Manager enters IDLE; waits for min_participants threshold

[Input Registration Phase]
  Client (Alice identity, fresh Tor circuit)
    → POST /register-input {utxo, bip322_proof, blinded_token}
  Coordinator:
    → UTXO Validator: verify UTXO via bitcoind RPC (exists, confirmed, not double-spent)
    → Ban List Manager: check UTXO not banned
    → Blind-sig Issuer: sign blinded_token with round RSA key
    ← return signed_blinded_token, alice_id

[Coordinator] Collects inputs. On timeout or max participants:
  → transition to OUTPUT_REG

[Output Registration Phase]
  Client (Bob identity, NEW Tor circuit — unlinkable from Alice)
    → POST /register-output {output_address, unblinded_signed_token}
  Coordinator:
    → Blind-sig Issuer: verify unblinded_signed_token is valid RSA signature
    → (Cannot link to Alice — that is the privacy guarantee)
    → Add output to candidate transaction

[Coordinator] When sum(outputs) ≈ sum(inputs) - fees:
  → Build unsigned CoinJoin transaction
  → transition to SIGNING

[Signing Phase]
  Client (Alice identity, same circuit as input-reg)
    → GET /transaction (unsigned PSBT)
    → Client verifies output appears in transaction
    → POST /sign {alice_id, input_signature}
  Coordinator:
    → Collect signatures; on timeout with missing signers:
       → Ban List Manager: mark missing UTXOs as non-signers
       → transition to BLAME

[Blame Round]
  → New round with only successful signers from prior round
  → Non-signers' UTXOs added to ban list (temporary)
  → Full round restarts (INPUT_REG) — missing participants cannot rejoin

[Broadcast]
  → Coordinator assembles fully-signed transaction
  → Submits via bitcoind RPC (sendrawtransaction)
  → Round State Manager zeros all round state from memory
  → transition to IDLE
```

### PKARR Discovery Data Flow

```
[Coordinator]
  Ed25519 key → sign DNS packet → pkarr relay → Mainline DHT
  (republish every ~1 hour; DHT TTL is a few hours)

[Client]
  pkarr relay OR direct DHT → resolve coordinator pubkey → get .onion + round params
  (or: use direct .onion if coordinator address is known)
```

### Network Topology

```
Client (Alice identity)  ──[Tor circuit A]──►  Coordinator .onion
Client (Bob identity)    ──[Tor circuit B]──►  Coordinator .onion
                         (circuit B is a fresh identity — unlinked from A)

Coordinator .onion surface:
  arti-client (in-process) accepts connections, no separate Tor process
  HTTP API served on localhost; arti bridges to .onion address
```

---

## Patterns to Follow

### Pattern 1: Phase-Gated HTTP API

**What:** Each endpoint checks the current round phase before processing the request. Requests arriving in wrong phase return an immediate error with current phase and expected retry timing.

**When:** All coordinator HTTP handlers.

**Rationale:** Prevents state corruption from stale or replayed requests. The phase gate is the coordinator's primary consistency invariant.

```rust
// Pseudocode — each handler does this first
async fn register_input(State(round): State<Arc<Mutex<Round>>>, ...) {
    let guard = round.lock().await;
    if guard.phase != Phase::InputRegistration {
        return Err(ApiError::WrongPhase { current: guard.phase });
    }
    // ... proceed
}
```

### Pattern 2: Alice/Bob Identity Separation

**What:** Clients use two entirely separate HTTP connections (different Tor circuits) for input registration (Alice) vs. output registration (Bob). The coordinator must never receive both in the same connection or session.

**When:** Client implementation of output registration.

**Rationale:** This is the core unlinkability guarantee. If Alice and Bob share a connection, the coordinator can trivially link inputs to outputs. Use arti-client to construct a fresh circuit with a different guard node before the Bob connection.

### Pattern 3: Per-Round RSA Keypair

**What:** Generate a fresh RSA keypair at the start of each round. Publish the public key to clients during input registration. Destroy the private key after transitioning out of OUTPUT_REG.

**When:** Blind-sig Issuer.

**Rationale:** Limits key reuse. If a single keypair were used across rounds, a compromised key would retroactively link historical input/output pairs. Per-round keys bound exposure to one round.

### Pattern 4: Memory-Only Round State

**What:** All round state (inputs, blinded tokens, signed tokens, outputs, partial signatures) lives in memory only. After broadcast or round abandonment, the state struct is dropped/zeroed.

**When:** Round State Manager.

**Rationale:** Required by the no-PII-logging constraint. A coordinator that logs input→output mappings is a deanonymization honeypot. No SQLite, no persistent state between rounds.

### Pattern 5: Tokio-Based Phase Timer

**What:** Each phase has a configurable timeout managed by a tokio timer. When the timer fires, the Round State Manager evaluates quorum: if minimum participants met, advance; if not, restart with blame or abandon.

**When:** Round State Manager phase transitions.

**Rationale:** Standard async pattern; avoids blocking threads on round waits. Timeouts must be generous enough for Tor latency (expect 2-5x clearnet latency).

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Shared RSA Key Across Rounds

**What:** Reusing the same RSA signing key for multiple rounds.

**Why bad:** Allows a compromised key to link inputs to outputs across all historical rounds. Also allows a malicious coordinator to correlate blind-signing requests if keys are unique per user — the spec requires all users get the same public key.

**Instead:** Generate a fresh RSA keypair per round. Rotate on every IDLE → INPUT_REG transition.

### Anti-Pattern 2: Session or Cookie State

**What:** Using HTTP sessions, cookies, or any persistent client identifier across the Alice and Bob connections.

**Why bad:** Defeats the Alice/Bob separation. Even a randomly-assigned session ID common to both connections leaks the link.

**Instead:** Coordinator is stateless from the HTTP layer's perspective. Alice identity (`alice_id`) is a coordinator-assigned token issued during input registration and returned by the client explicitly; Bob never presents it.

### Anti-Pattern 3: IP-Based Banning

**What:** Banning clients by IP address or Tor circuit when they misbehave.

**Why bad:** Tor exit/guard nodes are shared; banning by IP bans honest users sharing that node. Also ineffective — attacker creates new circuit.

**Instead:** Ban by UTXO only. Temporary (7-hour window). UTXO-based banning has an economic cost: attacker must control and sacrifice inputs.

### Anti-Pattern 4: Persisting Round State to Disk

**What:** Writing input registrations, blinded tokens, partial signatures, or input→output pairings to disk or database for crash recovery.

**Why bad:** Creates a forensic record of the exact data that must not be retained. Any disk persistence of this data violates the core privacy guarantee and potentially creates legal liability.

**Instead:** Accept that in-progress rounds are lost on coordinator restart. Round is short (minutes). Clients detect coordinator unavailability via PKARR record updates and retry.

### Anti-Pattern 5: Clearnet Endpoint in Production

**What:** Exposing the coordinator API on a clearnet TCP address as a production mode.

**Why bad:** Exposes the coordinator operator's IP, allows traffic analysis correlating client connections across phases, and undermines the Tor unlinkability model.

**Instead:** Development/test uses clearnet TCP explicitly gated behind a config flag. Production always uses Arti onion service. The PKARR record only publishes .onion addresses.

---

## Component Build Order

Dependencies between components dictate a natural build sequence. Later components depend on earlier ones being stable.

```
Layer 0: Bitcoin plumbing (no coordinator deps)
  └── bitcoind connection + UTXO validation via bitcoincore-rpc
  └── BIP-322 ownership proof verification

Layer 1: Protocol core (depends on Layer 0)
  └── Round state machine (state enum, phase transitions, timers)
  └── RSA blind signature issuance (blind-rsa-signatures crate)
  └── In-memory data structures (alice registry, output registry, ban list)

Layer 2: HTTP API (depends on Layer 1)
  └── axum handlers for all round endpoints
  └── JSON request/response types
  └── Phase-gate middleware

Layer 3: Transport (depends on Layer 2)
  └── Arti onion service wrapping the axum server
  └── Client Tor circuit management (Alice vs. Bob separation)

Layer 4: Discovery (depends on Layer 3)
  └── PKARR record publication (coordinator side)
  └── PKARR resolution (client side)

Layer 5: Packaging and tooling
  └── Docker Compose stack (bitcoind + coordinator + liquidity-bot)
  └── Liquidity bot (wraps client library)
  └── Integration test harness (3+ participants, blame round, adversarial)
```

**Rationale for this order:** Matches the project's stated "Approach B: Prove-Then-Layer" decision. Get a txid on signet using clearnet TCP (Layers 0-2), prove the round protocol correct, then layer Tor (Layer 3) and PKARR (Layer 4). This isolates protocol bugs from network bugs and makes each layer independently testable.

---

## Scalability Considerations

blindjoin targets signet/testnet and small-scale mainnet use. It is infrastructure, not a service. Scalability here means "stays correct under adversarial participants," not "handles millions of users."

| Concern | At 3 participants (MVP) | At 50 participants | At 200+ participants |
|---------|------------------------|-------------------|----------------------|
| Round coordination | Single-process, in-memory, trivial | Still in-memory; tokio handles concurrency | Transaction size grows; may hit Bitcoin standardness limits on input count |
| Blame round complexity | O(n) re-registration | O(n); still fast | O(n); blame round protocol unchanged but more HTTP traffic |
| Ban list size | Trivial | Small in-memory hashset | Periodic pruning needed; still in-memory is fine |
| PKARR DHT load | 1 publish per hour | Same | Same; PKARR is not round-frequency |
| Arti connection handling | Low; Tor connection setup is slow | Tor latency dominates; need generous timeouts | May need increased arti connection pool settings |
| Bitcoin RPC | 1 call per input registration | Still synchronous per-input; parallelizable | Batch UTXO validation if bitcoind supports; or parallelize with tokio::spawn |

The single-coordinator, in-memory design is correct for v1. The bottleneck at scale is Tor latency and Bitcoin transaction size limits, not the coordinator's data structures.

---

## Key Interface Contracts

### Coordinator HTTP API (canonical endpoints)

```
POST /api/v1/register-input
  Body: { utxo: OutPoint, value_sats: u64, bip322_proof: hex, blinded_token: hex }
  Response: { alice_id: uuid, signed_blinded_token: hex, round_id: uuid }

POST /api/v1/register-output
  Body: { output_address: string, unblinded_token: hex, round_id: uuid }
  Response: { ok: bool }

GET /api/v1/round-status
  Response: { phase: string, round_id: uuid, denomination_sats: u64,
              registered_inputs: u32, min_participants: u32,
              phase_deadline: timestamp }

GET /api/v1/transaction
  Response: { psbt: base64 }  # available in SIGNING phase only

POST /api/v1/sign
  Body: { alice_id: uuid, input_index: u32, signature: hex }
  Response: { ok: bool }
```

### PKARR Record Structure

DNS TXT records signed with coordinator Ed25519 key, published to Mainline DHT:

```
_coordinator.blindjoin TXT "onion=<52-char-v3-onion>.onion"
_coordinator.blindjoin TXT "denomination=1000000"   # sats
_coordinator.blindjoin TXT "min_participants=3"
_coordinator.blindjoin TXT "status=idle|input_reg|output_reg|signing"
_coordinator.blindjoin TXT "version=1"
```

Clients verify the Ed25519 signature on the DNS packet. The coordinator's public key is the discovery identifier (clients discover coordinators by Ed25519 pubkey, not by name).

---

## Sources

- [ZeroLink: The Bitcoin Fungibility Framework](https://github.com/nopara73/ZeroLink) — canonical Chaumian CoinJoin protocol spec; input/output reg, blame round design (HIGH confidence)
- [WabiSabi Paper](https://eprint.iacr.org/2021/206.pdf) — WabiSabi protocol architecture; round phase structure reference (HIGH confidence)
- [Bitcoin Optech: CoinJoin](https://bitcoinops.org/en/topics/coinjoin/) — round phase overview, Alice/Bob identity model (HIGH confidence)
- [Wasabi Denial of Service Protection](https://lontivero.github.io/Wiki/html/wasabi/wasabito_dos_protection.html) — ban list design, blame round mechanics (MEDIUM confidence)
- [PKARR crate docs](https://docs.rs/pkarr) — Rust DHT record publishing API (MEDIUM confidence; current as of early 2026)
- [pkarr GitHub](https://github.com/pubky/pkarr) — record structure, relay design, TTL behavior (MEDIUM confidence)
- [arti-axum crate](https://docs.rs/arti-axum/latest/arti_axum/) — Arti + axum onion service integration pattern (MEDIUM confidence)
- [ZeroLink DoS Defense issue](https://github.com/nopara73/ZeroLink/issues/6) — UTXO-based banning rationale, IP ban anti-pattern (HIGH confidence)
- [WabiSabi Coordinator Status API](https://github.com/WalletWasabi/WabiSabi/blob/master/protocol.md) — HTTP endpoint design reference (MEDIUM confidence)
- [BIP-322: Generic Signed Message Format](https://bips.dev/322/) — UTXO ownership proof mechanism (HIGH confidence on spec; MEDIUM on ecosystem adoption)
