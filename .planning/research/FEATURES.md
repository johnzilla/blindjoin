# Feature Landscape

**Domain:** CoinJoin coordinator (Bitcoin privacy infrastructure)
**Researched:** 2026-04-07
**Confidence:** MEDIUM-HIGH (protocol specs from WabiSabi paper and ZeroLink spec; ecosystem from multiple community sources)

---

## Table Stakes

Features users expect. Missing = coordinator feels incomplete or broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Round state machine (IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST) | Core CoinJoin protocol; all implementations share this structure | High | ZeroLink and WabiSabi both define this flow; deviating is a protocol break |
| UTXO ownership proof at input registration | Prevents spam/DoS without scarce collateral; ban list only works if proof is verified | Medium | WabiSabi uses SLIP-0019; blindjoin uses BIP-322 — functionally equivalent |
| Fixed-denomination equal outputs | This is what creates the anonymity set; without equal outputs the tool doesn't mix | Medium | WabiSabi added variable amounts later; v1 with fixed denoms is simpler and proven |
| Blind signatures for input→output unlinkability | The core cryptographic guarantee; without it the coordinator can deanonymize participants | High | RFC 9474 RSA blind sigs (Wasabi v1 approach); WabiSabi uses keyed-verification anonymous credentials |
| Blame round / non-signer detection | Without blame rounds, a single non-signer aborts the round indefinitely; rounds never complete under adversarial conditions | Medium | Temporary UTXO ban after failed signing is how implementations make DoS expensive |
| Temporary UTXO banning after misbehavior | DoS attacks become costly if the UTXO used to disrupt gets banned | Low | Duration matters: too short = cheap DoS; too long = accidental punishments |
| Tor transport (hidden service) | Privacy is the product; running coordinator on clearnet defeats the threat model | High | Post-Arti 2.0 this is achievable natively in Rust; clearnet is acceptable for dev/test |
| Configurable round parameters (denomination, min participants, timeouts) | Every operator has different liquidity and latency constraints | Low | These are coordinator config values, not protocol changes |
| No PII logging | Storing IP addresses or input→output mappings makes the coordinator a honeypot | Low | Policy + code discipline; the coordinator must not log what it can't see anyway |
| Round state cleared after broadcast | A coordinator that retains signed round data is a liability; the mapping must never persist | Low | Zero the in-memory structures; do not write to disk |
| UTXO validation against Bitcoin node | Coordinator must verify inputs are unspent before issuing blind tokens | Medium | Requires bitcoind/Bitcoin Core RPC or equivalent indexer |
| Transaction broadcast | Coordinator must broadcast the final signed transaction to the network | Low | Trivially done via bitcoind sendrawtransaction |
| Client CLI (register input, blind token, register output, sign) | Without a working client there is no way to test the coordinator | High | The two sides of the protocol; library + CLI binaries both needed |

---

## Differentiators

Features that set this coordinator apart. Not universally expected, but valued.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| PKARR / DHT coordinator discovery | Coordinators become disposable and replaceable; no hardcoded address; resilient to shutdown | High | Novel contribution of blindjoin; existing coordinators use hardcoded URLs or manual community lists (Wabisator, Liquisabi bot, Wasabist) |
| arti-native Tor hidden service (no separate Tor process) | Eliminates a runtime dependency; simpler Docker stack; fewer attack surfaces | High | Arti 2.0.0 (Feb 2026) made this viable; previous implementations required a Tor daemon sidecar |
| Docker Compose zero-to-working-round in 5 minutes | Dramatically lowers the bar for operating a coordinator; existing coordinators require C# runtime or manual Python setup | Medium | Packaging decision; the only comparable is BTCPay plugin (also Docker but heavyweight) |
| Liquidity bot (auto-join on signet) | Solves cold-start problem; coordinator is usable immediately for testing without external participants | Medium | JoinMarket has the yieldgenerator bot for makers; no equivalent exists for WabiSabi-style coordinators |
| First Rust implementation | Rust gives memory safety, async performance, and a modern packaging story; C# (Wasabi) and Python (JoinMarket) are the only prior implementations | High | Not a user-facing feature, but matters for auditability and community adoption |
| Fresh Tor circuit per phase (input vs output registration) | Prevents the coordinator's Tor guard nodes from linking input registration identity to output registration identity | Medium | This is a client-side feature; Wasabi Wallet does this with separate Alice/Bob Tor identities |
| Signet-first + mainnet as config flag | Safe-by-default; operators can experiment without real funds | Low | Network selection is a config value; no code paths differ |
| MIT license / no fees / no company | No legal entity to receive regulatory pressure; no fee to capture; fully public good | Low (policy) | zkSNACKs shutdown was a regulatory action against a company; this design has no such target |
| Integration tests covering blame protocol and adversarial scenarios | No other open-source coordinator ships adversarial test coverage; this makes the codebase auditable | High | Test infrastructure is real engineering work; signals production-readiness to the community |

---

## Anti-Features

Features to explicitly NOT build. These would add complexity, legal risk, or undermine the privacy model.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| UTXO blocklists / country blocklists | Blocklists require trust in a blocklist authority; they create a compliance surface that attracts regulatory pressure; OpenCoordinator's value proposition is explicitly "no blocklists" | Leave UTXO acceptance to UTXO validation only (unspent, minimum amount, valid proof) |
| Coordinator fee collection | Wasabi removed coordinator fees because they required trust and could be abused; collecting fees requires an on-chain identity for the coordinator | Zero fees; MIT license; this is infrastructure, not a business |
| Account / user management / identity layer | Defeats the anonymity model; any persistent identity is a privacy leak | All state is per-round; no user accounts, no sessions beyond round lifetime |
| WabiSabi variable-amount credentials (v1 scope) | No production Rust implementation exists; adds significant cryptographic complexity for marginal privacy gain in v1 | Fixed-denomination CoinJoin in v1; design protocol to extend to WabiSabi later |
| Mobile client (iOS/Android) | Tor + PSBT signing on mobile requires platform-specific work that is out of scope; CLI-first is sufficient to validate the protocol | CLI binary only for v1 |
| Metrics dashboard (Prometheus/Grafana) | Logging round metrics can leak information about participant timing and behavior | If needed, emit aggregated counts only (rounds completed, tx count) without per-round timing |
| PayJoin mode | Different protocol; different threat model; adding it to v1 diffuses focus | Document as post-v1 extension |
| Cross-coordinator rounds / multi-hop cascade | Coordination across coordinators adds liveness dependencies and protocol complexity | Document as post-v1 future work |
| OAuth / SSO / external auth | No identity layer means nothing to authenticate | Not applicable to the threat model |
| Mainnet as default | Risk of user loss on mainnet before protocol is battle-tested | Signet default; mainnet is a one-line config change |

---

## Feature Dependencies

```
UTXO validation (bitcoind RPC) → Input registration
Input registration → Blind signature issuance
Blind signature issuance → Output registration (Bob unblind + present)
Output registration → Transaction construction
Transaction construction → Signing phase
Signing phase → Broadcast OR Blame
Blame round → Temporary UTXO ban → New round with remaining participants

Tor hidden service (arti) → Production deployment
PKARR publishing → Coordinator discoverable via DHT
PKARR client discovery → Client can find coordinator without hardcoded URL

Liquidity bot → Cold-start testing on signet
Docker Compose → Zero-to-round in 5 minutes (depends on bitcoind + coordinator + liquidity bot)

Fresh Tor circuit per phase → Input/output unlinkability at network layer
Blind signatures → Input/output unlinkability at coordinator layer
Both required for full threat model
```

---

## MVP Recommendation

The MVP is the round protocol on signet with full cryptographic guarantees. Discovery and transport are layered after.

**Prioritize (Approach B: Prove-Then-Layer):**

1. Round state machine with RSA blind signatures — the core invariant; everything else is packaging
2. UTXO ownership proof (BIP-322) at input registration — required for DoS resistance
3. Blame round and temporary UTXO banning — required for rounds to complete under adversarial conditions
4. Client CLI (input registration through signing) — required to run integration tests
5. Liquidity bot — required to make signet testing self-contained
6. Integration tests (full round + blame + adversarial) — required before claiming the protocol works

**Defer (layer on after first txid on signet):**

- Tor hidden service via arti — isolate protocol bugs from network bugs first
- PKARR coordinator discovery — novel contribution but not required for round correctness
- Docker Compose stack — packaging last; validate protocol first
- Metrics / observability — post-v1

---

## Phase-Specific Notes

| Phase Topic | Feature Concern | Notes |
|-------------|----------------|-------|
| Round protocol | Blame round timeout tuning | Too short triggers false blame; too long stalls honest rounds |
| Blind signatures | Token reuse prevention | Coordinator must track issued tokens per round; replay = deanonymization |
| Tor integration | arti hidden service stability | Sprint 0 PoC required to verify arti 2.0 HS reliability before committing |
| PKARR | DHT record freshness and TTL | Stale records mean clients connect to dead coordinators |
| Client CLI | Separate Tor circuits per phase | Must use fresh circuit for output registration; same circuit leaks identity |
| DoS protection | Minimum participant threshold | Round must not proceed with 1 participant; anonymity set of 1 is no anonymity |

---

## Sources

- WabiSabi protocol specification: https://github.com/WalletWasabi/WabiSabi/blob/master/protocol.md (HIGH confidence)
- WabiSabi paper (IACR 2021): https://eprint.iacr.org/2021/206.pdf (HIGH confidence)
- ZeroLink specification: https://github.com/nopara73/ZeroLink (HIGH confidence)
- Wasabi Wallet docs: https://docs.wasabiwallet.io/using-wasabi/CoinJoin.html (HIGH confidence)
- OpenCoordinator: https://github.com/opencoordinator/opencoordinator (MEDIUM confidence — operational data only)
- BTCPay Server CoinJoin plugin: https://docs.btcpayserver.org/Wabisabi/ (MEDIUM confidence)
- SLIP-0019 (ownership proofs): https://github.com/satoshilabs/slips/blob/master/slip-0019.md (HIGH confidence)
- Peter Todd CoinJoin comparison (2025): https://petertodd.org/2025/coinjoin-comparison (MEDIUM confidence — independent analysis)
- Wabisator coordinator list: https://wabisator.com/ (LOW confidence — community list)
- Delving Bitcoin: Who will run the coordinators?: https://delvingbitcoin.org/t/who-will-run-the-coinjoin-coordinators/934 (MEDIUM confidence)
- GingerWallet WabiSabi vulnerability report: https://github.com/GingerPrivacy/GingerWallet/discussions/116 (MEDIUM confidence — relevant to credential validation pitfall)
