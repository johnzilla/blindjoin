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

> **Status — Draft (content-complete).** This document is the normative
> specification of the blindjoin coordinator-client wire protocol, developed
> alongside the OpenSats grant for blindjoin. Every section is now specified
> normatively. Where this prose and the
> [reference implementation](https://github.com/johnzilla/blindjoin) disagree, the
> reference implementation is authoritative and the discrepancy is a specification
> bug — please report it. Comments and review issues welcome via the project issue
> tracker.

---

## Abstract

This document specifies blindjoin, a protocol for coordinating CoinJoin
transactions on the Bitcoin network. blindjoin uses RSA blind signatures
([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) to make it cryptographically infeasible for the coordinator to link
participant inputs to participant outputs. Coordinators are discovered via
records published to the Mainline DHT under the [PKARR](https://github.com/pubky/pkarr) (Public-Key Addressable
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
keypairs under the [PKARR](https://github.com/pubky/pkarr) convention.

blindjoin's design goals are, in order:

1. **Cryptographic unlinkability.** The coordinator does not learn which
   input produced which output; this guarantee is structural, not policy.
2. **Discovery without operators.** No hardcoded coordinator list, no relay
   operator, no central registry.
3. **Standards adherence.** [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) for blind signatures, BIP-322 for
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
| Coordinator | Runs the round state machine; publishes a [PKARR](https://github.com/pubky/pkarr) record; verifies inputs; issues blind signatures; assembles the PSBT; broadcasts the final transaction. |
| Participant (Client) | Discovers a coordinator via [PKARR](https://github.com/pubky/pkarr); registers a UTXO with a BIP-322 ownership proof; submits a blinded token; retrieves the blind signature; registers an unblinded output on a separate Tor circuit; signs its input in the assembled PSBT. |
| Bitcoin Node | A trusted (operator-controlled) Bitcoin Core node providing UTXO validation, mempool fee estimation, PSBT construction, and broadcast. |

### Cryptographic primitives

| Primitive | Specification | Implementation |
|---|---|---|
| Blind signatures | [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) RSABSSA-SHA384-PSS-Randomized | [`blind-rsa-signatures` v0.17](https://crates.io/crates/blind-rsa-signatures) |
| Ownership proofs | [BIP-322 Simple](https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki) | Dispatcher in [`shared/src/bip322/`](../shared/src/bip322/) wrapping the upstream [`bip322 = "=0.0.11"`](https://crates.io/crates/bip322) crate (rust-bitcoin org) via a compact zero-lossy adapter plus a key-binding guard (defense-in-depth). Per-script implementations (P2WPKH, P2TR, P2SH-P2WPKH) live in sibling files behind a `pub(crate)`-only surface so callers cannot reach them to bypass dispatch. |
| Coordinator identity | Ed25519 over [PKARR](https://github.com/pubky/pkarr) | [`pkarr` v5](https://crates.io/crates/pkarr) |
| Transport | Tor v3 hidden services (server); isolated client circuits | [`arti-client` v0.41](https://crates.io/crates/arti-client) |
| Session token integrity | HMAC-SHA-256 with constant-time comparison | `hmac` + `sha2` |
| Memory hygiene | Zeroize on drop | [`zeroize`](https://crates.io/crates/zeroize) |

### Discovery

A coordinator MUST publish a [PKARR](https://github.com/pubky/pkarr) record to the Mainline DHT containing:

- The coordinator's reachable address: `.onion` hostname (production) or
  `host:port` (development only).
- The protocol version supported (current: `v0.2.0` for v1.4 coordinators; v1.3 used `0.1.0`).
- The advertised fixed denomination, in satoshis.
- The minimum participant count.
- (v1.4+) The supported input script types (`sst`, CSV; alphabetical canonical
  order) and the output script type the coordinator will use for round outputs
  (`ost`, single kebab-case string). Clients that cannot sign for any
  advertised input type SHOULD reject the coordinator at discovery time
  rather than opening a Tor circuit.

The [PKARR](https://github.com/pubky/pkarr) record MUST be signed by the coordinator's Ed25519 keypair (the
"coordinator identity key"); clients MUST verify the signature before
trusting any address contained in the record.

A coordinator SHOULD re-publish its [PKARR](https://github.com/pubky/pkarr) record at intervals not shorter
than 60 seconds and not longer than 600 seconds. The default in the reference
implementation is 300 seconds (5 minutes), matching typical Mainline DHT TTL.

**Canonical record format (NORMATIVE).** The discovery record is a PKARR
`SignedPacket` carrying a single DNS TXT resource record under the label
`_blindjoin` with a TTL of 300 seconds. The TXT value is one compact JSON object
whose fields, in canonical serialization order, are:

| Tag | Meaning | Example |
|---|---|---|
| `type` | schema identifier; MUST be `"blindjoin-coordinator"` | `"blindjoin-coordinator"` |
| `v` | protocol wire version (`shared::protocol::WIRE_VERSION`) | `"0.2.0"` |
| `onion` | reachable address: `.onion` hostname (production) or `host:port` (development) | `"<56-char>.onion"` |
| `n` | Bitcoin network | `"signet"` / `"testnet4"` / `"mainnet"` |
| `ds` | advertised fixed denomination, in satoshis | `1000000` |
| `mp` | minimum participant count | `3` |
| `st` | current round status | `"idle"` / `"input_reg"` / … |
| `sst` | supported input script types, CSV in alphabetical canonical order | `"p2sh-p2wpkh,p2tr,p2wpkh"` |
| `ost` | output script type the coordinator uses for round outputs, single kebab-case value | `"p2wpkh"` |

The packet MUST be signed by the coordinator's Ed25519 identity key; clients MUST
verify the packet signature before trusting any field (restated from above). That
public key is the coordinator's stable DHT identity and MUST NOT be rotated —
rotating it is equivalent to standing up a new coordinator.

**Size budget.** A DNS TXT character string is limited to 255 bytes; the serialized
JSON value MUST stay within it. The reference implementation warns at 220 bytes to
leave headroom and pins, in CI, that the worst-case production payload — a 62-byte
Tor v3 `.onion` plus the full three-type `sst` CSV — stays under 220. A new field,
or a fourth script type, that would breach the budget requires an explicit
encoding-compaction decision (single-character tags, a bitmask, or a
hash-of-set-then-fetch indirection) rather than silently overflowing the limit.

**Forward compatibility.** Consumers MUST ignore unknown tags (the resolver sets no
`deny_unknown_fields`), so new tags are additive and need no coordinated upgrade.
Two tags deliberately keep verbose names: `type` identifies the schema for future
consumers, and `onion` is read by the pre-`0.2.0` resolver and MUST keep that name
for backward compatibility.

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
- `blinded_token`: an [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)-blinded message commitment, base64-encoded.
- `change_address`: the bech32 address for the participant's change output
  (this is linkable to the input; participants are advised so).

The coordinator MUST:

1. Resolve the UTXO via its Bitcoin RPC; reject if spent, unmatured, or of
   wrong denomination class.
2. Verify the BIP-322 ownership proof; reject on any failure.
3. Reject blinded tokens whose byte length exceeds the RSA modulus length.
4. Reject any UTXO present on the persisted ban list.
5. Issue an [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) blind signature over the blinded token using the
   round's ephemeral RSA private key.
6. Issue an HMAC-SHA-256 session token bound to the round and UTXO,
   returned alongside the blind signature, used to authorize the future
   `/round/sign` submission without re-correlating to the input.

**Canonical challenge (NORMATIVE).** The message signed by the BIP-322 Simple
proof is the ASCII string:

```
blindjoin:round:<round_id>:utxo:<txid>:<vout>
```

where `<round_id>` is the current round's UUID in its canonical hyphenated
lowercase form, `<txid>` is the registered UTXO's transaction id as a
big-endian hex string (as displayed by Bitcoin Core), and `<vout>` is its output
index in decimal. Binding the challenge to both the `round_id` and the specific
outpoint ensures a proof captured from one round cannot be replayed into another,
and that a proof for one UTXO cannot be presented for a different one. The client
and coordinator MUST construct this string identically; the coordinator verifies
against the value it computes from the live `round_id` and the registered
outpoint, never from any client-supplied copy.

The string is then run through BIP-322 Simple: it is hashed with the BIP-340
tagged hash `SHA-256(SHA-256(tag) ‖ SHA-256(tag) ‖ message)` under the tag
`BIP0322-signed-message`, and that digest drives the `to_spend` / `to_sign`
virtual-transaction construction the per-script verifier checks. The sign and
verify paths share one message-hash and `to_sign` builder, so the challenge
digest cannot drift between them.

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
  RSABSSA-SHA384-PSS-Randomized scheme ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) §3.3.2).

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

Once every registered participant has registered an output, the coordinator
transitions `OutputReg → Signing` and assembles the canonical CoinJoin
transaction. The transaction is deterministic: given the same registered inputs
and outputs it is byte-identical on every build.

**Canonical transaction construction (NORMATIVE).** The reference builder is
[`build_coinjoin_psbt`](../coordinator/src/bitcoin/tx.rs). All three build sites —
the `GET /round/tx` display path, the per-submission signature verification, and
the final broadcast assembly — MUST produce a byte-identical transaction, or a
participant signs a different sighash than the one broadcast and silently wedges
the round. The construction rules are:

- **Version / locktime.** `nVersion` MUST be `2` and `nLockTime` MUST be `0`.
- **Input ordering.** Inputs MUST be sorted in ascending `OutPoint` order (by
  `txid`, then `vout`) — *not* registration order. This makes the ordering
  independent of participant registration timing and of hash-map iteration order.
- **Input sequence.** Every input MUST set `nSequence = 0xFFFFFFFF`
  (`Sequence::MAX`), which does **not** signal BIP-125 replaceability. Opt-in RBF
  MUST NOT be used: the protocol performs a single broadcast with no fee-bump
  path, and a replaceable input would let a participant who has already signed
  replace the CoinJoin in the mempool with a higher-fee spend of their own input
  *after* the round has returned to `Idle` — evicting the whole round for the cost
  of one fee, with no blame attribution possible. Because `nLockTime` is zero,
  `Sequence::MAX` forfeits no locktime behaviour.
- **Outputs.** One equal-value denomination output per registered output (value =
  the round's fixed `denomination_sats`), followed by per-participant change
  outputs in the same canonical input order. A change output is emitted only if
  its value is at least the dust threshold (294 sats under the reference relay
  policy); change below the threshold is folded into the fee and no output is
  emitted. The coordinator does not enforce the script type of registered
  outputs; each output is priced and placed from its actual scriptPubKey.
- **Fee allocation.** The estimated virtual size is `10 (fixed overhead) + Σ
  per-input vbytes + Σ per-output vbytes`, where per-input vbytes are selected by
  the coordinator-derived input script type and per-output vbytes are taken from
  each output's actual scriptPubKey (denomination and change alike; change is
  upper-bounded by assuming every participant produces one). `total_fee =
  estimated_vsize × fee_rate_sat_per_vbyte`, and each participant bears
  `fee_share = total_fee / N` (integer division; the sub-satoshi remainder is
  absorbed into the realized fee). Each participant's change is `input_value −
  denomination_sats − fee_share`; an input that cannot cover `denomination_sats +
  fee_share` fails assembly. The input script type used for weighting is
  coordinator-derived from the on-chain scriptPubKey at input validation and is
  never taken from a client-supplied field.

**Transaction retrieval.** A participant retrieves the transaction to sign with
`GET /round/tx`, which returns a [`RoundTxResponse`](../shared/src/protocol.rs)
`{ round_id, psbt, fee_total_sats, fee_per_participant_sats }` while the round is
in `Signing` (any other phase returns `409 WRONG_PHASE`). Clients MUST verify that
`round_id` matches the round they registered in before signing. The `psbt` field
is authoritative: clients SHOULD verify the transaction they are about to sign —
the presence and value of their own output and change, the input set, and the
total fee — by inspecting the PSBT directly, rather than trusting the advisory
`fee_total_sats` / `fee_per_participant_sats` summary fields (an output-pessimistic
upper-bound estimate).

**Signature submission.** A participant signs its input and POSTs `/round/sign`
with a [`SignRequest`](../shared/src/protocol.rs) `{ round_id, utxo_outpoint
("txid:vout"), partial_signature, session_token }`, where `partial_signature` is
base64 of the consensus-serialized witness for that input and `session_token` is
the 32-byte token returned at input registration. Before recording the signature
the coordinator MUST, in order:

1. Confirm the round is in `Signing` (re-checked under the write lock).
2. Verify the session token. The token is `HMAC-SHA-256(key = round_secret,
   data = txid ‖ vout_le32)`, compared in constant time. This authorizes the
   submission against a specific `(round, UTXO)` pair without re-correlating it to
   the registration connection.
3. Confirm the outpoint is a registered input of the round.
4. Reject a second submission for an input that already has a recorded signature.
   Without this, a participant could overwrite a valid signature with malformed
   bytes and force a blame round.
5. Cryptographically verify the witness against the canonical transaction's
   sighash for that input, and reject — **without recording** — any signature that
   does not verify.

Rule 5 is load-bearing. The signature is verified at submission against the *same*
transaction that will be broadcast, so a malformed or invalid signature is refused
at the door and the sender remains unsigned (and therefore a bannable non-signer
at the signing deadline). Verifying only in aggregate at broadcast would let a
single participant submit garbage, have every honest participant "sign", and
collapse the round with blame attributable to no one.

**Per-script signing rules (NORMATIVE).** The reference verifier is
[`sig_verify.rs`](../coordinator/src/bitcoin/sig_verify.rs). All ECDSA inputs MUST
use `SIGHASH_ALL`; `SIGHASH_SINGLE`, `SIGHASH_NONE`, and any `ANYONECANPAY`
variant MUST be rejected, as they would let a participant disclaim commitment to
the other outputs of the shared transaction.

| Input type | Sighash | Witness stack | Coordinator checks |
|---|---|---|---|
| P2WPKH | BIP-143 over the input's scriptPubKey, `SIGHASH_ALL` | `[DER-sig‖0x01, compressed pubkey]` | pubkey hashes to the input's witness program |
| P2SH-P2WPKH | BIP-143 over the unwrapped P2WPKH redeem script, `SIGHASH_ALL` | `[DER-sig‖0x01, compressed pubkey]` | the input's P2SH script commits to the redeem script; the coordinator reconstructs the `scriptSig` redeem push deterministically from the pubkey |
| P2TR (key path) | BIP-341 over all prevouts (`SIGHASH_DEFAULT` for a 64-byte signature; a 65-byte signature's explicit type byte is accepted only if `SIGHASH_ALL`) | `[schnorr sig]` | the signature verifies against the input's x-only output key |

**Completion and the hand-off to Broadcast.** When the final submission completes
the signature set, the coordinator assembles the final transaction (pure
computation, no network I/O) while holding the round lock and, as the last action
under that lock, transitions `Signing → Broadcast` *before* any broadcast RPC.
This ordering is a race-safety invariant: once the round is in `Broadcast` the
signing-deadline monitor no longer watches it, so the off-lock broadcast can never
race a signing-timeout blame. Any failure before the transition leaves the round
in `Signing`, so the signing deadline still governs a failed assembly. The
broadcast itself runs off the lock in a detached task (see Phase: Broadcast) so it
completes even if the submitting client's connection drops.

#### Phase: Broadcast

**Aggregation.** When the coordinator has recorded a valid partial signature for
every registered input (each verified against the canonical transaction's sighash
at submission time), it assembles the final transaction: it applies each
participant's witness to the corresponding input of the canonical PSBT (and, for
P2SH-P2WPKH, reconstructs the deterministic `scriptSig` redeem push from the witness
pubkey), then extracts the network-serializable transaction. Assembly is pure
computation over already-collected data — no per-participant interaction occurs in
this phase. The round MUST transition `Signing → Broadcast` at the moment assembly
succeeds, **before** the transaction is submitted to Bitcoin Core, so that the
signing-deadline timer stops treating the round as awaiting signatures.

**Single broadcast, no retry.** The coordinator submits the assembled transaction
exactly once (a `testmempoolaccept` pre-check followed by `sendrawtransaction`). It
MUST NOT resubmit or rebuild the transaction within a round: every honest participant
signed one specific transaction, so a rebuild would invalidate their signatures. A
submission that is rejected is treated as a broadcast failure (see below), not
retried.

**Transient state.** `Broadcast` is an in-flight state, not a terminal one. The
coordinator performs the (network) broadcast without holding the round lock, so
`GET /info` MAY report `round_state = "broadcast"` for the brief window between
assembly and the broadcast RPC returning. Observing `"broadcast"` does not by itself
prove the transaction was accepted by the network — clients that require confirmation
MUST watch the mempool/chain for the transaction, not the coordinator's phase.

**Successful completion.** Once the transaction is accepted for relay, the
coordinator transitions `Broadcast → Idle`. On entering `Idle` all sensitive round
state is dropped and zeroed — the per-round ephemeral RSA key, the round secret, and
all registered-input/-output and partial-signature material — and a fresh `round_id`
is minted for the next round. A successful broadcast resets the consecutive-blame
counter (see Phase: Blame).

**Broadcast failure.** If `testmempoolaccept` rejects the transaction or
`sendrawtransaction` fails, the coordinator re-validates each registered input
against the mempool-aware UTXO set and bans (with attribution) any input that has
been spent since registration — the double-spend griefing case — then ends the round
via `Broadcast → Blame → Idle`. A failure that attributes no spent input (e.g. a fee
rate too low for current mempool conditions) bans no one; repeated such failures
count toward the consecutive-blame cap so the coordinator backs off rather than
re-attempting the identical broadcast every round.

**Crash window.** Because the broadcast is a single external submission, a coordinator
that is killed *after* `sendrawtransaction` succeeds but *before* it records the
`Broadcast → Idle` transition leaves the transaction propagating on the network while
the coordinator restarts into a clean `Idle`. This is benign: the CoinJoin completes
on-chain regardless of coordinator state, and the coordinator holds no authority over
a transaction already broadcast. A backstop watchdog also force-transitions a round
out of `Broadcast` if the in-flight broadcast task ever dies without finalizing, so a
crashed or hung broadcast cannot wedge the coordinator.

#### Phase: Blame

`Blame` is the coordinator's response to a round that cannot complete. It is
entered from three edges, is never driven by a client request (there is no blame
endpoint), and always ends by returning the round to `Idle` — which drops and
zeroes all sensitive round state per [Memory hygiene](#memory-hygiene) — after
which the phase monitor re-arms a fresh round (unless a backoff is armed; see
below). Participants observe a blame only indirectly: `GET /info` reports the
phase returning to `Idle` under a new `round_id`.

**Entry edges.**

1. **Output-registration timeout with missing outputs** (`OutputReg → Blame`). If
   fewer outputs than inputs have been registered when the output-registration
   deadline passes, at least one participant did not register an output. Outputs
   are unlinkable by design (a valid blind signature carries no link to the input
   that obtained it), so the coordinator CANNOT identify which participant
   defaulted and MUST NOT ban any UTXO for a missing output. It detects the gap in
   aggregate only, abandons the round, and restarts. Accepting non-attribution
   here, rather than breaking unlinkability to assign blame, is a deliberate
   design choice.
2. **Signing timeout with non-signers** (`Signing → Blame`). At the signing
   deadline, every registered input without a recorded, verified partial signature
   is a non-signer. Non-signers *are* attributable — the defaulting input is known
   — so each is banned. Because an invalid signature is rejected without being
   recorded (Phase: Signing, rule 5), a participant who submitted garbage is
   indistinguishable from one who never submitted and is banned identically.
3. **Broadcast failure** (`Broadcast → Blame`). Specified under Phase: Broadcast.
   The coordinator re-validates each registered input against the mempool-aware
   UTXO set and bans any input that has been spent since registration (the
   double-spend griefing case); a failure that attributes no spent input bans no
   one.

**Blame outcomes.** The signing-timeout path yields one of:

- `RestartWithout { banned_utxos }` — one or more non-signers were found and
  banned. The round returns to `Idle` and a fresh round is armed on the next
  monitor tick; the banned UTXOs fail the ban-list check at input registration for
  the ban duration, so they cannot rejoin.
- `FullAbort` — returned when either (a) the signing timeout fired but no input
  was actually missing (a restart would gain nothing), or (b) the consecutive-blame
  cap has been reached (see below). On `FullAbort` the coordinator pauses the round
  re-armer for `blame_full_abort_backoff_secs` (when configured) instead of
  restarting immediately.

**Consecutive-blame cap (anti-churn).** The coordinator tracks a consecutive-blame
counter to distinguish a griefer it can exclude from a condition that will recur
every round:

- A *productive* blame — one that banned a non-signer or attributed a spent input
  — resets the counter to zero and the coordinator restarts immediately, because
  the responsible UTXO is now excluded and cannot recur.
- An *unproductive* blame — one that attributed no one (e.g. a signing timeout
  with all inputs present but assembly still failing, or a broadcast rejected for a
  fee rate too low for current mempool conditions) — increments the counter. On
  reaching `CONSECUTIVE_BLAME_CAP` (2 in the reference implementation) the
  coordinator arms the `FullAbort` backoff and resets the counter, so a recurring
  wedge backs the coordinator off rather than burning fresh rounds indefinitely.

**Ban-list schema (NORMATIVE).** Bans are keyed by UTXO outpoint, but the raw
outpoint is NEVER stored; the key is `hex(SHA-256(utxo_outpoint_string))`, so a
leaked ban file does not disclose which coins were banned.

- *In memory*: a map from the hashed key to `{ banned_at, expires_at }` (Unix
  seconds). A UTXO is banned iff an entry exists and the current time is strictly
  before `expires_at`. The ban list is consulted on every input registration.
- *On disk*: an append-only, newline-delimited JSON file (one `BanRecord {
  utxo_hash, banned_at, expires_at }` per line). Each append is followed by an
  `fsync` so a ban survives a power loss or OOM kill on the coordinator host. On
  startup the coordinator loads all unexpired entries (expired records and
  unparseable lines are skipped) and then compacts the file atomically
  (write-to-temp, `fsync`, rename) to physically drop expired records, bounding its
  growth. The ban duration is `blame_ban_duration_secs` from configuration.

Both the signing-timeout and broadcast-failure paths write bans through this same
store and share the consecutive-blame counter, so the two agree on when the
re-armer should back off.

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

This section states blindjoin's security goals, the adversary classes the
protocol is designed against, and how each attack class is addressed by the
mechanisms specified above. It is the protocol-level threat model. The
code-level mitigations (`file:symbol` references, the RSA zeroization drop-chain,
the cross-shape rejection tests) and the full disposition of every accepted
residual risk are maintained in the companion
[docs/AUDIT-CHARTER.md](AUDIT-CHARTER.md); the `cargo audit` advisory
dispositions are in [`.cargo/audit.toml`](../.cargo/audit.toml).

### Security goals

1. **Unlinkability (primary).** The coordinator MUST NOT be able to link a
   participant's registered input to its registered output. This guarantee is
   structural — it rests on the [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)
   blind signature and per-phase Tor circuit isolation, not on coordinator policy
   or good behaviour.
2. **No custody.** The coordinator never holds participant funds. It assembles and
   broadcasts a transaction that participants sign with their own keys; because
   every input commits to the full output set under `SIGHASH_ALL`, the coordinator
   cannot move coins to any address a participant did not register without
   invalidating the signatures it has collected.
3. **Bounded griefing.** A malicious participant can abort a round, but MUST NOT be
   able to do so repeatedly at zero cost; disruption is attributed and penalized
   wherever attribution does not break goal 1.

### Adversary classes

| Adversary | Capabilities assumed | Protocol stance |
|---|---|---|
| **Malicious coordinator** | Full control of the coordinator process: sees all wire traffic, chooses round parameters, may refuse service or selectively register inputs. | Cannot break unlinkability (goal 1) or steal funds (goal 2). Can deny service — see the trust-model residual below. |
| **Malicious participant** | Registers real UTXOs with valid ownership proofs; may submit malformed data, refuse to sign, double-spend a registered input, or attempt to spoof a script type. | Confined by ownership proofs, verify-before-record, non-replaceable inputs, and the blame/ban machinery. |
| **Sybil participant** | Controls many funded UTXOs; floods a round to shrink the effective anonymity set. | Bounded only economically (proof-of-UTXO-ownership + operator `min_participants`); see residual below. |
| **Network observer (partial)** | Observes some links, possibly including both of a participant's phase connections. | Defeated for input↔output correlation by per-phase circuit isolation. |
| **Global passive adversary** | Observes all Tor traffic. | Out of scope: Tor's own threat model governs; blindjoin inherits it. |
| **Chain analyst** | Observes only the on-chain CoinJoin transaction and the DHT. | Sees equal-value outputs with no coordinator-held link to inputs; the residual signal is the round-shape fingerprint below. |

### Attack classes and protocol response

- **Input/output correlation (coordinator).** The coordinator issues an
  [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) blind signature over a
  blinded commitment at input registration and never sees the unblinded output
  until it is presented on a *separate* Tor circuit at output registration. The
  blind signature carries no link to the input that obtained it. This is the
  mechanism behind goal 1.
- **Input/output correlation (network).** The output-registration circuit MUST be
  distinct from the input-registration circuit (NORMATIVE — see Phase: Output
  Registration), so even an observer who sees both connections cannot correlate
  them by guard node.
- **Output substitution (coordinator).** `SIGHASH_ALL` on every input commits each
  signer to the exact output set; the coordinator cannot add, drop, or re-point an
  output after collecting signatures without invalidating them (goal 2).
- **Ownership / script-type spoofing (participant).** Every registered input
  carries a BIP-322 ownership proof, and the coordinator derives the script type
  from the on-chain scriptPubKey — never from the client-declared field — rejecting
  a mismatch before verification. See AUDIT-CHARTER §V1.4-CRIT-01.
- **Round poisoning via bad signatures (participant).** Each partial signature is
  verified against the canonical transaction's sighash at submission and rejected
  without being recorded if invalid (Phase: Signing, rule 5), so a participant
  cannot have every honest peer "sign" and then collapse the round with blame
  attributable to no one.
- **Post-broadcast RBF eviction (participant).** All inputs are non-replaceable
  (`Sequence::MAX`; see Phase: Signing), so a signer cannot replace the broadcast
  CoinJoin in the mempool with a higher-fee spend of their own input after the
  round has returned to Idle.
- **Double-spend griefing (participant).** A participant who spends a registered
  input out from under the round causes a broadcast failure; the coordinator
  re-validates inputs and bans the provably-spent one (Phase: Blame, edge 3).
- **Repeated free aborts (participant).** Non-signers are banned for
  `blame_ban_duration_secs`, and the consecutive-blame cap backs the coordinator
  off a recurring wedge (Phase: Blame). Ban records are stored hashed, so the ban
  list itself discloses no outpoints.
- **Coordinator-side timing sidechannel ([Marvin Attack](https://rustsec.org/advisories/RUSTSEC-2023-0071)).**
  The RSA signing key is a fresh per-round ephemeral (RSA-2048), destroyed on the
  transition to Idle (see Memory hygiene), and the number of blind-sign operations
  against it is bounded by `max_participants`. The Marvin Attack's preconditions —
  a long-lived key and effectively unlimited chosen-ciphertext timing measurements
  — therefore do not obtain. Full drop-chain and disposition: AUDIT-CHARTER §RSA
  Secret Key Zeroization Window and §Residual Risks: cargo-audit Advisories.
- **Sybil shrinking of the anonymity set.** Not cryptographically preventable
  beyond proof-of-UTXO-ownership; mitigated operationally by a higher
  `min_participants` floor, which raises the attacker's capital cost. Accepted
  residual.

### Accepted residual risks

The protocol knowingly accepts the following, each documented with its full
rationale and disposition in AUDIT-CHARTER §Residual Risks Accepted:

- **Single-coordinator trust (availability).** DHT discovery makes coordinators
  *replaceable*, not byzantine-fault-tolerant within a round: a malicious
  coordinator can refuse to broadcast or selectively register inputs. Participant
  funds are never at risk (goal 2); only liveness is, and the remedy is
  re-discovery of another coordinator.
- **Anonymity-set fingerprints.** Heterogeneous input script types, and
  output script types that are not enforced server-side, leave a partial
  round-shape signal for a chain analyst — accepted as a tradeoff for the larger
  anonymity sets that heterogeneous rounds allow (AUDIT-CHARTER §V1.4-MIN-02 and
  the output-script residual).
- **Sybil dilution** bounded only by operator `min_participants` policy.
- **PKARR replay window.** A stale discovery record may briefly resolve to a
  former address; relies on PKARR versioning and an operator rotation grace period.
- **Static fee estimation.** Acceptable on signet; mempool-aware / dynamic fees are
  REQUIRED before any mainnet deployment.
- **Upstream `cargo audit` advisories** in transitive Tor/DHT dependencies, none on
  a blindjoin protocol path; tracked in [`.cargo/audit.toml`](../.cargo/audit.toml).

Forward-looking items (dynamic fees for mainnet, differential
cross-implementation test fixtures) are tracked as gaps in AUDIT-CHARTER §Residual
Risks and in [`TODO.md`](../TODO.md).

## Rationale

### Why [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) (RSABSSA) rather than chaumian e-cash or Schnorr blind sigs

blindjoin uses [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)
RSABSSA-SHA384-PSS-Randomized. The choice follows from one hard project
constraint — **no custom cryptography** — and the state of audited Rust
primitives:

- **RSABSSA (chosen).** [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)
  is an IETF-published standard with a clearly stated security argument, and
  `blind-rsa-signatures` (jedisct1) is a production-grade, audited Rust
  implementation of it — the only RFC 9474-compliant one. The cost is size:
  RSA-2048 blinded tokens and signatures are large relative to the alternatives,
  and verification is heavier than elliptic-curve schemes. For a per-round,
  bounded-participant protocol this is acceptable — the tokens are exchanged once
  per round and never touch the chain.
- **Chaumian e-cash (WabiSabi / Wasabi 2.x).** Smaller credentials, and supports
  arbitrary-amount rather than fixed-denomination coordination. But it is not an
  IETF/IRTF deliverable and — decisively — there is no production-grade Rust
  implementation to depend on. Reimplementing it would violate the no-custom-crypto
  constraint and create exactly the audit surface the project avoids. Recorded as
  ADR Decision #1 in [docs/AUDIT-CHARTER.md](AUDIT-CHARTER.md).
- **Schnorr blind signatures.** Smaller still, and a natural fit with Taproot. But
  naive constructions are vulnerable to Wagner-style parallel (ROS) attacks; the
  hardened schemes that fix this are recent and, at the time of writing, lack both
  mature audited Rust implementations and the settled, widely-reviewed security
  proofs [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) carries. Adopting
  one now would again mean shipping unaudited crypto.

The ordering is therefore: a standardized primitive with an audited Rust
implementation beats a smaller-but-unstandardized or smaller-but-immature
alternative, given that the protocol can absorb the size cost. If an audited Rust
Schnorr-blind-signature or WabiSabi implementation reaches production maturity, the
primitive is a candidate for revisiting — the blind-signature engine is isolated
behind [`coordinator/src/blind/`](../coordinator/src/blind/).

### Why [PKARR](https://github.com/pubky/pkarr)/DHT rather than Nostr

Both PKARR-over-Mainline-DHT and Nostr-relay publication solve the same problem —
letting a client find a coordinator's current address without a hardcoded list —
but they differ in who can be coerced.

- **PKARR / Mainline DHT (chosen).** A coordinator publishes a signed record to
  the Mainline DHT (the same multi-million-node table that backs BitTorrent).
  There is no operator between publisher and resolver: any participating node can
  serve the record, records are self-authenticating under the publisher's Ed25519
  key, and there is no registrar to subpoena, deplatform, or sanction. This
  directly serves the design goal of *discovery without operators* (see
  Motivation). The costs are a weaker freshness model — DHT propagation and TTLs
  mean a stale record can resolve briefly (the replay-window residual in
  [docs/AUDIT-CHARTER.md](AUDIT-CHARTER.md)) — and a tight record-size budget (the
  TXT limit in [Discovery](#discovery)).
- **Nostr relays.** Better-tooled, richer query semantics, and increasingly used
  by other CoinJoin discovery layers. But a Nostr relay is operator-mediated
  infrastructure: an operator can be compelled to drop events, and a client depends
  on reaching relays that carry the coordinator's announcements. Given that the
  independent-coordinator landscape has thinned precisely because operator-mediated
  infrastructure is coercible (see Motivation), depending on relay operators would
  reintroduce the single point of coercion the project is designed to remove.

The decision mirrors the [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)
one in spirit: accept a more constrained, less convenient mechanism in exchange
for removing a party that can be coerced. A Nostr announcement remains a plausible
*additive* discovery channel in future — the two are not mutually exclusive — but
the DHT is the operator-free baseline.

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
2026-06; this document is a Milestone 1 deliverable). The [PKARR](https://github.com/pubky/pkarr) family of
primitives is the work of the Pubky team and the broader Mainline DHT
community. The `blind-rsa-signatures` crate is maintained by Frank Denis.

## Copyright

This document is licensed under the MIT License.
