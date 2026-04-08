# Domain Pitfalls: CoinJoin Coordinator

**Domain:** CoinJoin coordinator with RSA blind signatures, Tor hidden service, DHT discovery
**Researched:** 2026-04-07
**Confidence:** HIGH (multiple confirmed real-world incidents, protocol research, post-mortems)

---

## Critical Pitfalls

Mistakes that cause rewrites, total anonymity set collapse, or legal exposure.

---

### Pitfall 1: Per-Participant Unique RSA Public Keys (Tagging Attack)

**What goes wrong:** The coordinator issues a different RSA public key to each participant during input registration. When Bob presents his blinded token at output registration, the coordinator matches the unblinding against the unique key it gave that Alice, directly linking input to output. All anonymity is destroyed. This attack is passive from the participant's perspective — nothing looks wrong.

**Why it happens:** The natural implementation is to generate one RSA key per round. Developers miss that the coordinator controls key distribution and can trivially differentiate keys per-session without client detection.

**Consequences:** Complete deanonymization of every participant in every round, with zero warning signs visible to users. The attack is silent.

**Real-world incident:** This exact vulnerability was independently discovered in both Ashigaru Whirlpool (June 2025, reported by "nothingmuch") and was exploitable by malicious WabiSabi coordinators via the credential issuer tagging attack (GingerWallet Discussion #116, December 2024). The WabiSabi version affected Wasabi Wallet ≤ 2.2.1.0, GingerWallet ≤ 2.0.13, and BTCPay Server coinjoin plugin ≤ 1.0.101.0.

**Prevention:**
- Publish the RSA public key in the PKARR record and in the round parameters before any participant connects. The key must be fixed per coordinator identity, not per round or per session.
- Clients MUST fetch the coordinator's public key from the DHT record and verify it matches what the coordinator sends during input registration — reject any mismatch immediately.
- Include the RSA public key hash in the round commitment (see Pitfall 2).
- Consider using a single long-lived signing key rather than rotating per-round. If rotating, publish the new key in PKARR before accepting registrations.

**Warning signs:** Key changes between rounds without PKARR record update; coordinator key doesn't match DHT-published parameters.

**Phase:** Address in the blind signature protocol design phase (Phase 1 or equivalent core protocol sprint). The client-side verification of the coordinator's published key is the critical control.

---

### Pitfall 2: Round Parameters Not Committed in Client-Verifiable Hash (Round ID Forgery)

**What goes wrong:** The coordinator gives different round parameters (minimum participants, denomination, fee, or credential issuance parameters) to different participants. Since participants cannot see each other's parameters, they each believe they are in the same round, but the coordinator has partitioned them into singleton groups it can identify.

**Why it happens:** Developers implement a simple round identifier (UUID or sequence number) rather than deriving the round ID from a hash of all parameters. The coordinator can then silently modify what it tells each participant.

**Consequences:** Coordinator can isolate any individual participant into an effectively solo transaction, fully tracing their input to output without the participant knowing a round completed.

**Real-world precedent:** ZeroLink (the Wasabi v1 protocol on which blindjoin is based) introduced the RoundHash specifically to prevent this. The signing-phase check — Alice verifies that the RoundHash equals the hash of all registered inputs — is not optional decoration. Wasabi 2.x derives the RoundID by hashing all round parameters including the credential issuance key for the same reason.

**Prevention:**
- Round ID = SHA256(denomination || min_participants || coordinator_pubkey || round_sequence_number || ...). Derive deterministically from all parameters that matter for unlinkability.
- At signing phase: clients verify that the round ID committed in their BIP-322 ownership proof matches the round parameters they actually observed. Reject if they diverge.
- Clients MUST compare round parameters received against the PKARR-published round parameters. Any deviation is an abort signal.

**Warning signs:** Round ID not derived from a hash of parameters; round parameters not checked against DHT-published state.

**Phase:** Core round state machine implementation. Must be in the first working protocol sprint.

---

### Pitfall 3: Blame Round Exploited to Reduce Anonymity Set

**What goes wrong:** A malicious participant (or the coordinator itself) deliberately fails to sign in the signing phase, triggering a blame round. The blame round reconvenes with a subset of participants. If an adversary controls even one participant and can force multiple blame cycles, they progressively reduce the anonymity set — eventually to a set small enough to trace by elimination.

**Why it happens:** Blame rounds are necessary for liveness but create a shrinking-set vulnerability if an adversary can repeatedly force them at low cost. The Wasabi v2.x research confirmed "soft aborts can allow unbounded iteration of attacks."

**Consequences:** Privacy degradation proportional to number of blame cycles. In the worst case, an attacker with multiple UTXOs can reduce the surviving set to a known cluster.

**Prevention:**
- Apply a permanent ban on any UTXO that causes a blame round. Do not allow re-entry for the rest of the coordinator's lifetime (or at minimum a long cooldown period — days, not hours).
- Cap blame rounds at a fixed maximum (e.g., 1 or 2). If the maximum is hit, abort the entire round rather than continuing with a tiny set. A failed round is better than a deanonymized round.
- Require a fresh UTXO for each re-registration attempt after a ban. This makes repeated blame-forcing expensive since the attacker must own distinct UTXOs.
- Log blame events by UTXO hash (not IP — see Pitfall 8) so the ban list persists across restarts.

**Warning signs:** The same UTXO appearing in multiple blame rounds; rounds repeatedly failing at signing phase; round sizes shrinking across blame cycles.

**Phase:** Blame protocol implementation sprint. Cap and ban logic is not optional.

---

### Pitfall 4: Coordinator-Controlled Isolation (Singleton Deanonymization)

**What goes wrong:** The coordinator notices an input it wants to deanonymize, then refuses to admit any other participants into the round with that input — manufacturing a "round" of one participant whose input and output are trivially linked.

**Why it happens:** The coordinator controls which connection confirmations it accepts. Nothing in the blind signature scheme prevents the coordinator from selectively admitting participants.

**Real-world precedent:** The ZeroLink spec explicitly documents this attack and its mitigation.

**Prevention:**
- Enforce minimum participant thresholds client-side: clients MUST verify the CoinJoin transaction has the configured minimum number of equal-value outputs before signing. Refuse to sign if the output count is below the minimum.
- Publish the minimum participant count in the PKARR record; clients use the PKARR value, not what the coordinator tells them during the round.
- Clients compare the actual PSBT output count against the committed minimum before submitting their partial signature.

**Warning signs:** Rounds consistently completing with only 2-3 participants; minimum participant count not enforced at signing phase.

**Phase:** Client CLI implementation — the PSBT validation step before signing is the critical control. Must be part of the signing phase spec.

---

## Moderate Pitfalls

---

### Pitfall 5: DoS via Refusing to Sign (Griefing Attack)

**What goes wrong:** A participant registers a UTXO, participates through output registration, then refuses to provide their partial signature. This aborts the round (or forces blame) with no cost to the attacker beyond the UTXO lockup time.

**Why it happens:** Input registration has no proof of intent to complete. Any participant can abort at signing with zero on-chain cost on signet (fees are free or trivial).

**Prevention:**
- UTXO banning: permanently ban any UTXO that fails to provide a valid partial signature during the signing phase. Store bans in persistent state across restarts.
- On signet this is less critical (no real economic cost), but the ban mechanism must be in place before mainnet exposure. Design the ban store now even if the stakes are low.
- Rate-limit new UTXO registrations per Tor circuit guard (where identifiable) — but never log IPs directly.

**Warning signs:** The same UTXO appearing in multiple failed rounds; rounds consistently aborting at signing phase from the same UTXO fingerprint.

**Phase:** Round state machine and ban store implementation.

---

### Pitfall 6: Marvin Attack on the RSA Crate

**What goes wrong:** The underlying `rsa` Rust crate (which `blind-rsa-signatures` depends on) is vulnerable to the Marvin Attack — a timing side-channel that can enable private key recovery by a network attacker if the coordinator's RSA signing endpoint is accessible and measurable.

**Why it happens:** The `rsa` crate's decryption/signing operations are not fully constant-time. The Marvin Attack exploits timing variance in RSA private key operations.

**Prevention:**
- The coordinator's RSA signing endpoint is only reachable via a Tor hidden service. Tor's onion routing adds significant timing noise, substantially mitigating but not eliminating the attack surface.
- Track whether `blind-rsa-signatures` / the `rsa` crate ships a Marvin fix before v1 launch. Check the crate's changelog and open issues.
- If the fix is not available, consider key rotation (new RSA key per N rounds) to limit exposure window from any key compromise.
- Use RSA-4096 minimum. The `check_rsa_parameters()` function in `blind-rsa-signatures` had a bug rejecting valid 4096-bit keys in earlier versions — verify the version in use accepts 4096-bit keys.

**Warning signs:** Using an `rsa` crate version prior to any published Marvin patch; RSA signing endpoint reachable on clearnet.

**Phase:** Dependency audit sprint. Verify before any mainnet flag exposure.

---

### Pitfall 7: Tor Circuit Reuse Across Input and Output Registration

**What goes wrong:** The client uses the same Tor circuit (and thus the same exit guard node) for both input registration (Alice, tied to UTXO) and output registration (Bob, presenting the blind token). If the coordinator or a network observer can correlate Tor circuits, the phases are linked.

**Why it happens:** Default Tor behavior reuses circuits for connections to the same destination. A developer implementing Tor integration without explicit fresh-circuit requests will create this linkage.

**Prevention:**
- The PROJECT.md already specifies fresh Tor circuits per phase — this must be enforced in code, not just documented.
- Use `IsolateDestAddr` / stream isolation in Arti. Each phase (input reg, output reg) must use an explicitly isolated stream with no circuit reuse.
- Verify with integration tests: connect input phase and output phase through Tor, confirm they route through different guard nodes (observable in test logs).
- In Arti, use `TorClient::isolated_client()` or equivalent stream isolation API for each phase transition.

**Warning signs:** Input and output registration phases using the same `TorClient` without stream isolation; no test verifying circuit isolation.

**Phase:** Tor integration sprint. Must be verified before the full-stack integration test.

---

### Pitfall 8: Logging IP Addresses or Input-Output Associations

**What goes wrong:** Coordinator logs contain Tor exit IP addresses, UTXO identifiers alongside timestamps, or any data that correlates input registration events to output registration events. This data becomes a surveillance target and a legal liability.

**Why it happens:** Default frameworks log request details. Structured logging in async Rust (tracing crate) makes it easy to inadvertently include UTXO details in request spans.

**Prevention:**
- Audit every `tracing::info!`, `debug!`, `warn!` call site for UTXO identifiers, addresses, output script hashes, or connection metadata.
- Log only round-level events (round started, round completed, round aborted, participant count) — never participant-level events that could be correlated.
- The UTXO ban list must store a hash (e.g., TXID:vout SHA256) not the raw UTXO, and it must never be logged with timestamps that could correlate to a round.
- Add a logging policy to the README: what is logged, what is explicitly not logged, and why.

**Warning signs:** `span!` or request middleware that auto-captures all request fields; UTXO identifiers visible in log output; IP addresses in any log.

**Phase:** Core coordinator implementation. Establish logging discipline before any other code is written.

---

### Pitfall 9: Arti Hidden Service Stability Under Load

**What goes wrong:** Arti's onion service implementation, while stabilized in 2.0.0, has had resilience bugs in earlier versions (e.g., TROVE-2024-005 and TROVE-2024-006 — incorrect circuit construction and same-relay-in-multiple-positions bugs that increase traffic analysis vulnerability). The Tor hidden service drops connections under participant load spikes.

**Why it happens:** Hidden service circuit construction is complex; early Arti versions had algorithmic bugs that compiled fine but built malformed circuits. The Rust borrow checker catches memory bugs, not semantic circuit bugs.

**Prevention:**
- Pin to Arti 2.0.0+ and track changelogs for TROVE advisories. Subscribe to Tor Project security announcements.
- Test hidden service stability under concurrent load in the integration test suite (multiple clients connecting simultaneously for a round).
- Implement connection retry logic in the client with exponential backoff — Tor connections are inherently less reliable than TCP.
- The PROJECT.md notes a Sprint 0 PoC to verify arti-client works for hidden services. Do this before any other Tor-dependent development.

**Warning signs:** Participants reporting frequent connection drops; round timeouts clustering at connection establishment rather than signing.

**Phase:** Sprint 0 (Tor PoC) and integration test suite.

---

### Pitfall 10: Fixed Denomination Fingerprinting

**What goes wrong:** Fixed-denomination CoinJoin transactions are identifiable on-chain by blockchain analysis. All outputs being exactly 0.01 BTC (or any fixed value) is a strong heuristic for detecting CoinJoin, reducing post-mix privacy.

**Why it happens:** This is inherent to the ZeroLink design. It is not a bug — it is a known limitation. The mistake is not communicating this clearly to users, or building additional features (like change output handling) that make the fingerprint worse.

**Prevention:**
- Document the fingerprinting limitation explicitly in the README and in the client output. Users should know their CoinJoin transactions are identifiable as CoinJoins on-chain, just not linkable input-to-output.
- Ensure change outputs (outputs not equal to the denomination) are handled consistently across all participants so they don't create unique fingerprints that reduce the anonymity set.
- Do not mix address types (P2WPKH vs P2TR) in the same round — different output types break the equal-value guarantee visually.

**Warning signs:** Inconsistent output address types within a single round; change output amounts that are unique per participant.

**Phase:** Transaction construction and PSBT validation sprint.

---

## Minor Pitfalls

---

### Pitfall 11: PKARR Record Spoofing / Eclipse on Discovery

**What goes wrong:** A malicious actor publishes a PKARR record for a fake coordinator with a malicious .onion address. Clients discovering coordinators via DHT are routed to a surveillance coordinator rather than a legitimate one.

**Prevention:**
- PKARR records are signed with the coordinator's key pair. Clients MUST verify the signature on every PKARR record before trusting the contained .onion address.
- Use coordinator public key pinning: if a user has previously connected to a coordinator identified by a specific public key, warn on key change (similar to SSH host key verification).
- Provide a well-known "trusted coordinator" list for first-time users as a bootstrapping mechanism.

**Phase:** PKARR integration sprint.

---

### Pitfall 12: BIP-322 Ownership Proof Not Committing to Round ID

**What goes wrong:** The BIP-322 ownership proof (proving a participant controls a UTXO) does not include the round ID in the signed message. This means a valid ownership proof from one round could be replayed in a different round by the coordinator.

**Prevention:**
- The BIP-322 message signed for ownership proof MUST include the round ID (as defined in Pitfall 2) as a mandatory component. This binds the proof to the specific round and prevents replay.
- Validate at the coordinator: reject any ownership proof whose embedded round ID does not match the current round's ID.

**Phase:** Input registration protocol implementation.

---

### Pitfall 13: In-Memory State Not Actually Zeroed After Broadcast

**What goes wrong:** Rust's `Drop` semantics do not guarantee memory zeroing. Simply dropping a struct containing blinding factors, RSA signing keys, or partial signatures does not clear the memory — the compiler may optimize away the zeroing if it detects the memory is no longer read.

**Prevention:**
- Use `zeroize` crate (the de facto standard in the Rust cryptography ecosystem) for all sensitive state: blinding factors, RSA key material, partial signatures, round participant lists.
- Derive `Zeroize` and `ZeroizeOnDrop` on all state structs holding sensitive data.
- After broadcast, explicitly call `zeroize()` on round state before the struct drops.
- Integration test: verify round state structs implement `ZeroizeOnDrop` (can be a compile-time check with trait bounds).

**Phase:** Core coordinator and client implementation. Apply from the start — retrofitting zeroize is error-prone.

---

### Pitfall 14: Signet → Mainnet "Just a Config Flag" Assumption

**What goes wrong:** Developers treat the mainnet configuration flag as trivial. In practice, signet and mainnet differ in: UTXO value scale (actual economic risk), fee pressure (mainnet fees can spike 10-100x making rounds economically irrational), and the regulatory environment.

**Prevention:**
- Enforce a build-time feature flag for mainnet (`#[cfg(feature = "mainnet")]`), not just a runtime config value. This prevents accidental mainnet deployment.
- Add a mainnet-specific safety checklist to the repository: required review items before enabling the mainnet flag for a deployment.
- Test under realistic mainnet fee scenarios before promoting mainnet support.

**Phase:** Late in development, before any mainnet documentation or promotion.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|---|---|---|
| Blind signature protocol design | Per-participant key tagging (Pitfall 1) | Fix RSA key to coordinator identity, publish in PKARR, verify client-side |
| Round ID / parameter commitment | Parameter forgery partition attack (Pitfall 2) | Derive round ID from hash of all parameters |
| Blame protocol implementation | Blame cycle shrinkage attack (Pitfall 3) | Cap blame rounds, permanent UTXO banning |
| Client signing phase | Singleton isolation attack (Pitfall 4) | Client-side PSBT output count validation before signing |
| Tor integration (Sprint 0 PoC) | Arti hidden service stability (Pitfall 9) | Sprint 0 explicitly validates arti hidden service works |
| Tor client phase isolation | Circuit reuse linking input/output phases (Pitfall 7) | Stream isolation per phase, integration test verifying different guards |
| Logging setup | PII leakage in logs (Pitfall 8) | Establish logging policy before first line of coordinator code |
| Dependency selection | Marvin attack on RSA crate (Pitfall 6) | Track rsa crate advisories, hide behind Tor |
| Memory management | Sensitive state not zeroed (Pitfall 13) | zeroize crate from day one |
| Transaction construction | Fixed denomination fingerprinting (Pitfall 10) | Document limitation, enforce consistent address types per round |
| PKARR integration | Record spoofing on discovery (Pitfall 11) | Mandatory signature verification on every DHT record |
| Input registration | BIP-322 proof without round ID commitment (Pitfall 12) | Embed round ID in BIP-322 message |

---

## Sources

- GingerWallet WabiSabi vulnerability disclosure: https://github.com/GingerPrivacy/GingerWallet/discussions/116
- Bitcoin Magazine WabiSabi deanonymization report: https://bitcoinmagazine.com/technical/wabisabi-deanonymization-vulnerability-disclosed
- NoBSBitcoin WabiSabi coordinator deanonymization: https://www.nobsbitcoin.com/wabisabi-vulnerability-allows-malicious-coordinators-to-deanonymize-coinjoin-users/
- Ashigaru Whirlpool RSA blinding review: https://gist.github.com/84adam/e130b40cff5915de67b86fc8e452c8aa
- ZeroLink protocol specification (RoundHash defense): https://github.com/nopara73/ZeroLink
- WabiSabi protocol spec (Round ID): https://github.com/WalletWasabi/WabiSabi/blob/master/protocol.md
- WabiSabi timing/data withholding mitigations: https://github.com/WalletWasabi/WabiSabi/issues/83
- Peter Todd CoinJoin comparison (July 2025): https://petertodd.org/2025/coinjoin-comparison
- Reiterating centralized CoinJoin deanonymization attacks (bitcoindev): https://groups.google.com/g/bitcoindev/c/CbfbEGozG7c/m/fwwxCihmEQAJ
- JoinMarket Sybil attack issue: https://github.com/JoinMarket-Org/joinmarket-clientserver/issues/960
- Arti TROVE-2024-005/006 (hidden service circuit bugs): https://blog.torproject.org/arti_1_2_4_released/
- Arti 1.4.6 hidden service resilience: https://blog.torproject.org/arti_1_4_6_released/
- rust-blind-rsa-signatures (jedisct1): https://github.com/jedisct1/rust-blind-rsa-signatures
- Input-output mapping analysis in CoinJoin (2025): https://arxiv.org/html/2510.17284
- Samourai shutdown implications: https://cointelegraph.com/research/samourai-wallet-shutdown-implications-for-other-privacy-self-custody-tools
- Who will run CoinJoin coordinators (Delving Bitcoin): https://delvingbitcoin.org/t/who-will-run-the-coinjoin-coordinators/934
- FinCEN CVC mixing regulatory context: https://fintelegram.com/coinjoin-crackdown-global-regulators-re-draw-the-privacy-line-for-bitcoin/
