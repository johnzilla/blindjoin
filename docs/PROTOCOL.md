# blindjoin Protocol Specification

| Field | Value |
|---|---|
| **Title** | blindjoin: A Blind-Signed CoinJoin Coordination Protocol with DHT-Based Discovery |
| **Authors** | John Turner (`<johnturner@gmail.com>`) |
| **Status** | Draft |
| **Layer** | Applications |
| **License** | MIT (this specification and the reference implementation) |
| **Created** | 2026-05 |
| **Implementation** | https://github.com/johnzilla/blindjoin |

> **Status — Draft.** This document is the in-progress normative specification of
> the blindjoin coordinator-client wire protocol. It is being developed as
> Milestone 1 deliverable of the OpenSats grant for blindjoin. Sections marked
> **[TODO]** are present to fix the structure of the document and will be filled
> in during the milestone. Sections without **[TODO]** are normative as written.
> Comments and review issues welcome via the project issue tracker.

---

## Abstract

This document specifies blindjoin, a protocol for coordinating CoinJoin
transactions on the Bitcoin network. blindjoin uses RSA blind signatures
(RFC 9474) to make it cryptographically infeasible for the coordinator to link
participant inputs to participant outputs. Coordinators are discovered via
records published to the Mainline DHT under the PKARR (Public-Key Addressable
Resource Records) convention, removing the need for hardcoded coordinator lists
or relay-operator-mediated rendezvous. All production transport runs over Tor
v3 hidden services; clients use isolated Tor circuits across protocol phases to
prevent correlation by circuit metadata.

## Motivation

The independent CoinJoin coordinator landscape has thinned since 2024:
Samourai's Whirlpool ceased operation under criminal indictment (founders
sentenced November 2025); Wasabi's default coordinator zkSNACKs shut down
under OFAC pressure in May 2024. Coordinator-based CoinJoin is now operating
under contested but live US case-law in which coordinator activity has been
held to constitute money transmission, despite FinCEN guidance to the contrary.

Surviving and emerging implementations (Wasabi-family custom coordinators
advertised over Nostr, joinstr's Nostr-relay model, JoinMarket's IRC + onion
service peer discovery) have converged on relay-based discovery. Relay
infrastructure is operator-mediated: a relay operator can be subpoenaed,
deplatformed, or sanctioned. blindjoin specifies a discovery layer that is
not operator-mediated: signed records on the Mainline DHT, retrievable from
any participating node, with cryptographic identity established by Ed25519
keypairs under the PKARR convention.

blindjoin's design goals are, in order:

1. **Cryptographic unlinkability.** The coordinator does not learn which
   input produced which output; this guarantee is structural, not policy.
2. **Discovery without operators.** No hardcoded coordinator list, no relay
   operator, no central registry.
3. **Standards adherence.** RFC 9474 for blind signatures, BIP-322 for
   ownership proofs, PSBT (BIP-174) for partial-signature exchange.
4. **Operator simplicity.** A new coordinator should be deployable by anyone
   with a Bitcoin Core node and Docker.

## Specification

### Notation and conventions

The key words "MUST", "MUST NOT", "REQUIRED", "SHOULD", "SHOULD NOT",
"RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted
as described in [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119)
and [RFC 8174](https://datatracker.ietf.org/doc/html/rfc8174).

### Roles

| Role | Description |
|---|---|
| Coordinator | Runs the round state machine; publishes a PKARR record; verifies inputs; issues blind signatures; assembles the PSBT; broadcasts the final transaction. |
| Participant (Client) | Discovers a coordinator via PKARR; registers a UTXO with a BIP-322 ownership proof; submits a blinded token; retrieves the blind signature; registers an unblinded output on a separate Tor circuit; signs its input in the assembled PSBT. |
| Bitcoin Node | A trusted (operator-controlled) Bitcoin Core node providing UTXO validation, mempool fee estimation, PSBT construction, and broadcast. |

### Cryptographic primitives

| Primitive | Specification | Implementation |
|---|---|---|
| Blind signatures | [RFC 9474](https://datatracker.ietf.org/doc/html/rfc9474) RSABSSA-SHA384-PSS-Randomized | [`blind-rsa-signatures` v0.17](https://crates.io/crates/blind-rsa-signatures) |
| Ownership proofs | [BIP-322 Simple](https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki) | Dispatcher in [`shared/src/bip322/`](../shared/src/bip322/) wrapping the upstream [`bip322 = "=0.0.10"`](https://crates.io/crates/bip322) crate (rust-bitcoin org) via a 26-LOC zero-lossy adapter. Per-script implementations (P2WPKH, P2TR, P2SH-P2WPKH) live in sibling files behind a `pub(crate)`-only surface so callers cannot reach them to bypass dispatch. |
| Coordinator identity | Ed25519 over [PKARR](https://github.com/pubky/pkarr) | [`pkarr` v5](https://crates.io/crates/pkarr) |
| Transport | Tor v3 hidden services (server); isolated client circuits | [`arti-client` v0.41](https://crates.io/crates/arti-client) |
| Session token integrity | HMAC-SHA-256 with constant-time comparison | `hmac` + `sha2` |
| Memory hygiene | Zeroize on drop | [`zeroize`](https://crates.io/crates/zeroize) |

### Discovery

A coordinator MUST publish a PKARR record to the Mainline DHT containing:

- The coordinator's reachable address: `.onion` hostname (production) or
  `host:port` (development only).
- The protocol version supported (current: `v0.2.0` for v1.4 coordinators; v1.3 used `0.1.0`).
- The advertised fixed denomination, in satoshis.
- The minimum and maximum participant counts.
- (v1.4+) The supported input script types (`sst`, CSV; alphabetical canonical
  order) and the output script type the coordinator will use for round outputs
  (`ost`, single kebab-case string). Clients that cannot sign for any
  advertised input type SHOULD reject the coordinator at discovery time
  rather than opening a Tor circuit.

The PKARR record MUST be signed by the coordinator's Ed25519 keypair (the
"coordinator identity key"); clients MUST verify the signature before
trusting any address contained in the record.

A coordinator SHOULD re-publish its PKARR record at intervals not shorter
than 60 seconds and not longer than 600 seconds. The default in the reference
implementation is 300 seconds (5 minutes), matching typical Mainline DHT TTL.

**[TODO]** Specify the canonical TXT record format, including field tags,
ordering, and size budget under DHT record-size limits.

### Round lifecycle

A round proceeds through the following phases:

```
Idle → InputReg → OutputReg → Signing → Broadcast → (Blame, on timeout) → Idle
```

#### Phase: Input Registration

A participant POSTs `/round/input` with a [`InputRegRequest`](../shared/src/protocol.rs)
containing:

- `utxo_outpoint`: the UTXO being registered, as `"txid:vout"`.
- `ownership_proof`: a BIP-322 Simple signature over the canonical
  blindjoin challenge, encoded per [`OwnershipProof`](../shared/src/protocol.rs).
  v1.3 used a JSON array of hex-encoded witness stack items
  (P2WPKH-only); v1.4 extends this to a versioned flat-struct envelope
  (`version: u8`, `witness_stack`, `psbt_input_b64`, `script_type`) so
  P2TR and P2SH-P2WPKH proofs can carry the additional bytes they need
  (Schnorr keypath witness, P2SH `final_script_sig`). The
  v1.4 decoder is a two-phase try-parse that accepts the legacy
  array-of-hex shape as `version = 1`, so a v1.3 client continues to
  interoperate with a v1.4 coordinator byte-for-byte.
- `blinded_token`: an RFC 9474-blinded message commitment, base64-encoded.
- `change_address`: the bech32 address for the participant's change output
  (this is linkable to the input; participants are advised so).

The coordinator MUST:

1. Resolve the UTXO via its Bitcoin RPC; reject if spent, unmatured, or of
   wrong denomination class.
2. Verify the BIP-322 ownership proof; reject on any failure.
3. Reject blinded tokens whose byte length exceeds the RSA modulus length.
4. Reject any UTXO present on the persisted ban list.
5. Issue an RFC 9474 blind signature over the blinded token using the
   round's ephemeral RSA private key.
6. Issue an HMAC-SHA-256 session token bound to the round and UTXO,
   returned alongside the blind signature, used to authorize the future
   `/round/sign` submission without re-correlating to the input.

**[TODO]** Specify the canonical blindjoin BIP-322 challenge string.

#### Phase: Output Registration

The participant unblinds the blind signature locally and POSTs `/round/output`
on a **separate Tor circuit** (the "Bob" circuit), with:

- `unblinded_token`: the original message M (the 32-byte hash computed from
  `compute_blind_token_message(output_script, amount_sats)`); base64-encoded.
- `signature`: the unblinded RSA signature over M; base64-encoded.
- `output_address`: the bech32 address for the participant's CoinJoin
  output.
- `amount_sats`: the output amount.
- `msg_randomizer`: 32 bytes of randomizer for the
  RSABSSA-SHA384-PSS-Randomized scheme (RFC 9474 §3.3.2).

The coordinator MUST:

1. Verify the signature against the round's ephemeral RSA public key.
2. Confirm `output_address` and `amount_sats` reconstruct M to match
   `unblinded_token`.
3. Accept the output without learning which input produced it.

**Tor circuit isolation requirement (NORMATIVE):** Clients MUST use a
different Tor circuit for `/round/output` than they used for `/round/input`.
The reference implementation obtains two `TorClient` handles via
`arti-client`'s `isolated_client()` to guarantee distinct guard nodes
and circuits.

#### Phase: Signing

**[TODO]** Specify PSBT construction order (input ordering, output ordering),
fee allocation per participant, signing-message canonicalization, and the
participant's `/round/sign` submission flow including session-token
verification.

#### Phase: Broadcast

**[TODO]** Specify final-signature aggregation, broadcast retry policy,
and successful-completion behavior (memory zeroing, ephemeral-key
destruction, ban-list updates).

#### Phase: Blame

**[TODO]** Specify the non-signer detection, blame outcome enumeration
(`FullAbort`, `RestartWithout`), and the ban-list persistence schema.

### Memory hygiene

The coordinator MUST destroy the round's ephemeral RSA private key after
broadcast (or after a `FullAbort` blame outcome). The reference
implementation expresses this requirement as a Rust type signature:
the parsed RSA blind signer is held on the round state as
`RoundStateInner.rsa_signer: Option<RsaBlindSigner>` (`Some(_)` during an
active round, `None` when Idle). On any FSM transition into
`Phase::Idle`, `RoundState::transition_to` sets `self.inner = None` at
the SOLE site (a single grep-verifiable chokepoint), which drops
`Option<RsaBlindSigner>` → drops `RsaBlindSigner` → drops the
`RoundSecretKey(BjSecretKey)` newtype → drops `BjSecretKey` → drops
`rsa::RsaPrivateKey`, whose unconditional `impl Drop` zeroizes the
secret exponent, prime factors, and CRT-precomputed values via
the `zeroize` crate (`rsa` declares `impl ZeroizeOnDrop for
RsaPrivateKey`). The serialized DER form held alongside the parsed
signer is zeroized by an explicit `zeroize::Zeroize` call inside
`Drop for RoundStateInner`, which also clears the registered-input
map, partial-signature collection, and blinded-token cache. The full
threat-model treatment, drop-chain diagram, and verification tests are
documented in [docs/AUDIT-CHARTER.md §5](AUDIT-CHARTER.md#rsa-secret-key-zeroization-window).

### Forward compatibility

All wire types defined in this specification MUST be deserialized without
`#[serde(deny_unknown_fields)]`. Implementations MUST silently ignore
unknown fields in any wire message to permit additive protocol evolution
without coordinated upgrade.

## Security considerations

**[TODO — full content delivered in companion document `THREAT-MODEL.md`,
also in Milestone 1.]**

Topics that THREAT-MODEL.md will address normatively, with this document
deferring to that file:

- Adversary classes: malicious coordinator, malicious participant, sybil
  participant, network observer, global passive adversary, chain analyst.
- Attack classes: input/output correlation, sybil attacks on anonymity
  set, coordinator-side timing sidechannels (including the
  [Marvin Attack](https://rustsec.org/advisories/RUSTSEC-2023-0071)
  applicability analysis given ephemeral per-round keys; see
  [`.cargo/audit.toml`](../.cargo/audit.toml) for the current
  residual-risk rationale), Tor traffic correlation, RBF-based
  fee-manipulation attacks.
- Accepted residual risks and the rationale for accepting each.

## Rationale

### Why RFC 9474 (RSABSSA) rather than chaumian e-cash or Schnorr blind sigs

**[TODO]** Discuss the trade-off between RSABSSA (RFC-standardized,
mature primitive, larger signatures) versus Wasabi's chaumian e-cash
construction (smaller, but not standardized as an IETF/IRTF deliverable)
and emerging Schnorr blind signature schemes (smaller still, but lack
mature implementations and clearly published security proofs at time of
writing).

### Why PKARR/DHT rather than Nostr

**[TODO]** Discuss the trade-off between Mainline DHT (no relay operator,
peer-to-peer, established with millions of nodes for BitTorrent) and
Nostr relays (operator-mediated but well-tooled and increasingly used
by other CoinJoin discovery layers).

### Why Tor circuit isolation per phase

The blind-signature unlinkability guarantee is undermined if input
registration and output registration can be correlated by Tor circuit
metadata. Using `isolated_client()` ensures the two phases traverse
disjoint guard nodes and circuits, defeating circuit-fingerprint
correlation even in the presence of a partial network observer who can
see both phases' connections.

## Reference implementation

The reference implementation is the [blindjoin](https://github.com/johnzilla/blindjoin)
repository, MIT licensed. The wire-format types backing this specification
live in [`shared/src/protocol.rs`](../shared/src/protocol.rs); the blind
signature engine in [`coordinator/src/blind/rsa.rs`](../coordinator/src/blind/rsa.rs);
the Tor isolation harness in [`client/src/tor.rs`](../client/src/tor.rs).

## Acknowledgements

This work is funded by an OpenSats Bitcoin grant (application submitted
2026-06; this document is a Milestone 1 deliverable). The PKARR family of
primitives is the work of the Pubky team and the broader Mainline DHT
community. The `blind-rsa-signatures` crate is maintained by Frank Denis.

## Copyright

This document is licensed under the MIT License.
