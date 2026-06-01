# Feature Landscape — v1.4 BIP-322 Multi-Script Support

**Domain:** BIP-322 ownership-proof support inside an existing CoinJoin coordinator + client
**Researched:** 2026-05-29
**Confidence:** HIGH on spec wire-format (BIP-322 + BIP-141/143/341 are settled), HIGH on Wasabi precedent (PR #8912 / discussion #9216 are explicit), MEDIUM on the `bip322` crate API surface (still 0.0.x, last release 9 months ago — see PITFALLS)

This document replaces the v1.0 ecosystem landscape with a v1.4-scoped feature surface. The CoinJoin coordination features (rounds, blame, blind tokens, PKARR, Tor) are already shipped (see `.planning/PROJECT.md` Validated) — this milestone broadens *one* surface: the UTXO ownership-proof verifier and the address-type acceptance gate.

The single sentence definition of v1.4:

> Replace the P2WPKH-only `is_p2wpkh()` hard gate at [`coordinator/src/bitcoin/utxo.rs:119`](coordinator/src/bitcoin/utxo.rs:119) with verification for P2WPKH + P2TR (BIP-86, key-path) + P2SH-P2WPKH, advertise the set over PKARR, and let clients sign over all three.

---

## BIP-322 Simple by script type — wire format and verification path

BIP-322 Simple is a witness-only encoding: the signer constructs virtual `to_spend` and `to_sign` transactions per spec sections 4–5, signs the `to_sign` input as if it were a real spend of `to_spend`, and the serialized **witness stack** of that input is the signature payload. The verifier reconstructs the same `to_spend`/`to_sign`, plugs the witness stack into `to_sign` input 0, and runs script verification — or, equivalently, recomputes the sighash and verifies the signature directly. Wire format on the protocol surface is "the consensus-encoded `bitcoin::Witness` for input 0 of `to_sign`."

Per BIP-322 §"Signature Hash" the sighash type **MUST** be `SIGHASH_ALL` for ECDSA paths, and **MAY** be `SIGHASH_DEFAULT` (the implicit Taproot all-bytes-covered mode) for any output type that supports it — i.e. P2TR key-path. Each script type differs in three places:

| Dimension | P2WPKH (baseline, shipped) | P2TR key-path (BIP-86) | P2SH-P2WPKH |
|-----------|----------------------------|------------------------|-------------|
| `to_spend.output[0].scriptPubKey` | `0 <20-byte HASH160(pubkey)>` (`OP_0` + push20) | `1 <32-byte tweaked x-only pubkey>` (`OP_1` + push32) | `HASH160 <20-byte HASH160(0 <HASH160(pubkey)>)> EQUAL` (P2SH wrapping the witness program) |
| `to_sign.input[0].scriptSig` | empty | empty | **single push of the 22-byte redeemScript** `0x160014{20-byte-key-hash}` — this is the P2SH-segwit witness-version byte trick, not a literal signature push |
| `to_sign.input[0].witness` (this is the BIP-322 Simple payload on the wire) | `[ECDSA-DER-sig\|\|SIGHASH_ALL, compressed-pubkey]` — 2 items | `[Schnorr-sig]` — 1 item, 64 bytes if SIGHASH_DEFAULT or 65 bytes if explicit SIGHASH_ALL appended | Same 2-item shape as P2WPKH (`[ECDSA-DER-sig\|\|SIGHASH_ALL, compressed-pubkey]`) — the redeem-script is in `scriptSig`, **not** in the witness |
| Sighash algorithm | BIP-143 v0 (`SighashCache::p2wpkh_signature_hash`) | BIP-341 (`SighashCache::taproot_key_spend_signature_hash` with the empty `Prevouts::All(&[to_spend.output[0].clone()])`) | BIP-143 v0 — identical to P2WPKH, because the embedded witness program is exactly P2WPKH (`SighashCache::p2wpkh_signature_hash` against the **inner** wpkh scriptPubKey `0 <HASH160(pubkey)>`, not the outer P2SH scriptPubKey) |
| Signature primitive | secp256k1 ECDSA | secp256k1 Schnorr (BIP-340) over the BIP-340-tweaked key | secp256k1 ECDSA |
| Sighash type byte appended? | yes — `0x01` (`SIGHASH_ALL`) | optional — absent ⇒ SIGHASH_DEFAULT, `0x01` ⇒ explicit SIGHASH_ALL; **prefer absent for canonical form** | yes — `0x01` |
| Pubkey used to derive scriptPubKey | compressed 33-byte secp pubkey → HASH160 → P2WPKH | x-only 32-byte secp pubkey → BIP-86 unkeyed-Merkle-root tweak → P2TR | same as P2WPKH, then wrapped in P2SH-HASH160 of the 22-byte witness program |

**Concrete operational consequence**: in the current `shared/src/bip322.rs:114` verifier, swapping P2WPKH for P2TR is **not** "different sighash function only" — it's a different signature curve (Schnorr not ECDSA), a different sighash algorithm (BIP-341 not BIP-143), a different pubkey representation (x-only not compressed), a different witness-stack arity (1 not 2), and an optional sighash-type byte. P2SH-P2WPKH on the other hand is "the P2WPKH path **plus** a non-empty `scriptSig` on the `to_sign` input" — the witness stack and verification are identical to P2WPKH against the unwrapped witness program. This is why the `bip322` crate groups P2WPKH and P2SH-P2WPKH together but treats P2TR as a separate code path.

**Why we should adopt `bip322` v0.0.x rather than extend `shared/src/bip322.rs`**: every line of the table above is exactly what the `bip322` crate already encodes ([docs.rs/bip322](https://docs.rs/bip322/latest/bip322/)). Recreating it ourselves would be three more places to get the witness-stack arity wrong silently — and the v1.0 implementation already commits the "strip trailing 0x01 if present" hack ([`shared/src/bip322.rs:155`](shared/src/bip322.rs:155)) which is a smell that this code path wants a real library. Adopt the crate; pin the exact `0.0.10` version; gate via discuss-phase if the API shape doesn't match what the table predicts.

---

## Table Stakes

Features absent in v1.0 that an honest `/round/info` advertising "P2WPKH + P2TR + P2SH-P2WPKH" must provide.

| Feature | Why expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Address-type detection at registration | Coordinator dispatch must pick the right verifier per input | Low | `bitcoin::Script::is_p2wpkh() \|\| is_p2tr() \|\| is_p2sh()`; the P2SH branch needs a further "is the redeem script a P2WPKH witness program" check before accepting (raw P2SH multisig must still be rejected) |
| Per-script-type sighash construction | Each script type uses a different sighash algorithm (see table above) | Medium | Delegate to `bip322::verify_simple(address, message, witness)` once on the crate; do not hand-roll the three paths |
| Witness-stack arity validation | P2WPKH = 2 items, P2TR = 1 item, P2SH-P2WPKH = 2 items + non-empty scriptSig | Low | Reject early with `Bip322Error::InvalidWitnessLength` before signature verification — saves a secp op on hostile inputs |
| Schnorr signature verification path | New cryptographic primitive vs v1.0's ECDSA-only path | Medium | Pulled in transitively via `bitcoin` crate; no new dep. Use `secp256k1::Secp256k1::verification_only()` and `verify_schnorr` |
| Canonical sighash-type acceptance | P2TR key-path must accept both 64-byte (SIGHASH_DEFAULT) and 65-byte (explicit SIGHASH_ALL) witness items | Low | Per BIP-341: 64 bytes ⇒ implicit SIGHASH_DEFAULT, 65 bytes ⇒ last byte is sighash type and must be non-zero. **Reject anything else** — non-zero sighash byte ≠ SIGHASH_ALL is a fingerprint vector |
| Reject P2SH redeem scripts that are not P2WPKH | A raw P2SH multisig or arbitrary P2SH would otherwise fall through the "P2SH path" verifier | Medium | After parsing the `scriptSig` push as 22 bytes, assert `bytes[0] == 0x00 && bytes[1] == 0x14` (witness-v0 + push20). Anything else: `Bip322Error::UnsupportedScriptType` |
| `BIP-322 Simple` wire format on `/round/register_input` | API contract for the witness payload across all three script types | Low | Already in v1.0 as `Vec<Vec<u8>>` consensus-serialized — extend to optionally accept the 1-item witness stack for P2TR. **No new endpoint**; the existing endpoint stays type-uniform |
| Client-side signer for P2TR | Client must be able to BIP-86-derive a Taproot key and produce a Schnorr signature over BIP-341 sighash | Medium | `bdk_wallet 2.3` already supports `tr(...)` descriptors and PSBT Taproot signing — leverage existing PSBT-signing path, do **not** add a parallel BIP-322 sign path for now (PSBT already builds the sighash correctly) |
| Client-side signer for P2SH-P2WPKH | BDK supports `sh(wpkh(...))` descriptors out of the box | Low | Same PSBT-signing path as P2WPKH; the only delta is that `scriptSig` will be populated automatically by the wallet |
| Coordinator advertises `supported_script_types` over PKARR | Client must reject mismatched coordinator before registration to avoid wasted Tor circuit + bad UX | Low | Add `supported_script_types: ["p2wpkh", "p2tr", "p2sh-p2wpkh"]` to the `_blindjoin` TXT JSON in [`coordinator/src/discovery/pkarr_pub.rs:64`](coordinator/src/discovery/pkarr_pub.rs:64); mirrored on `/round/info` |
| Liquidity-bot script-type coverage | Cold-start signet rounds must demonstrably exercise all three paths, not just P2WPKH | Medium | Bot should be able to generate UTXOs of each type and register a configurable mix per round |

---

## Differentiators

Features that go beyond the minimum honest implementation. None are strictly required for v1.4 to ship; they distinguish the coordinator's operational story.

| Feature | Value proposition | Complexity | Notes / recommendation |
|---------|-------------------|------------|------------------------|
| **Per-script-type ban tracking** | An attacker who burns one P2WPKH UTXO via blame-protocol abuse cannot trivially come back with P2TR UTXOs from the same wallet — but the inverse argument is also true: an honest operator's ban list "leaks" which script-type bans are common | Low | **Recommend: single unified ban table** keyed on `OutPoint` (txid:vout), already the v1.0 design. Per-script-type partitioning would buy nothing — the OutPoint is the UTXO, full stop. Adding script-type as a dimension would only fragment the ban list and weaken DoS deterrence |
| **Per-script-type rate limits on `/round/register_input`** | A Taproot-specific DoS could be throttled separately from a P2WPKH DoS | Low | **Recommend: do not add.** v1.2 ships a `GlobalKeyExtractor` rate limiter (Tor-safe by design — `PeerIpKeyExtractor` is unsafe on Tor). Per-script-type buckets would require parsing the request body before throttling, defeating the point of rate-limiting at middleware layer. Keep the global limit; tune the bound if needed |
| **Operator opt-in / opt-out per script type** | Operators may want to allow P2WPKH + P2TR but disable P2SH-P2WPKH for "modern wallets only" rounds | Low | **Recommend: yes, config flag.** Mirror Wasabi's `AllowP2trInputs` / `AllowP2trOutputs` design ([WalletWasabi PR #8912](https://github.com/zkSNACKs/WalletWasabi/pull/8912)) — three booleans `allow_p2wpkh`, `allow_p2tr`, `allow_p2sh_p2wpkh` in `coordinator.toml`. Default all to `true`. The advertised PKARR `supported_script_types` is computed from these flags so the wire and the config are always consistent. **This is the right level of operator control** — finer-grained (per-round) is overkill, coarser (single "modern only" flag) loses the precedent |
| **Per-script-type metrics** | Operator can see "70% of registrations are P2WPKH, 25% P2TR, 5% P2SH-P2WPKH" for capacity planning | Low | **Recommend: aggregate counters only, no per-round / per-participant.** Per-round-per-script-type counters become a fingerprint if they leak to the public `/round/info`. Internal counters are fine. Anti-feature: a public dashboard would partition rounds by script type, defeating part of the mixing |
| **Round transition advertises script-type breakdown** | Clients could see "this round has 3 P2WPKH + 2 P2TR registrants, choose your own type strategically" | Medium | **Anti-feature, do not build.** This is exactly the partitioning vector. The whole point of accepting mixed script types is that the round transaction is *less* fingerprinted, not more. See "uniform vs mixed-script rounds" below |
| Liquidity-bot configurable script-type mix | Operator can tune signet bot to bias toward whichever type has lowest organic participation | Low | Useful for testing; ship the knob, default to "uniform random across enabled types" |

---

## Anti-Features (v1.4 carry-over + new)

| Anti-feature | Why avoid | What to do instead |
|--------------|-----------|-------------------|
| **P2WSH multisig ownership proofs** | Multi-key sighash construction, redeem-script verification, M-of-N policy gates — high crypto complexity for a privacy gain (multisig wallets joining CoinJoin) that has no demonstrated user demand. Wasabi shipped without it for two years | Document as "post-v1.4, possibly never" in PROJECT.md Out of Scope. The `bip322` crate v0.0.10 explicitly scopes to single-sig only — adopting the crate enforces this naturally |
| **P2TR script-path spending in ownership proofs** | A script-path BIP-322 Simple signature requires control-block + leaf-script + per-leaf witness construction. The `bip322` crate does not implement it. Verifying it requires a complete Taproot script interpreter. There is no scenario in which a CoinJoin participant needs to prove ownership via the script-path: if they have the key, key-path is shorter and cheaper | Explicit reject: P2TR inputs must use key-path BIP-322. Witness stack length > 1 ⇒ `Bip322Error::UnsupportedScriptType`. Document loudly in the verifier |
| **Legacy P2PKH ownership proofs** | The `bip322` crate supports it per docs, but P2PKH inputs in a CoinJoin round are a privacy anti-pattern: they are non-segwit, their txid is malleable, their fingerprint is heavy, and "P2PKH-using" is itself a small-anon-set marker. Adding the verifier is cheap but the wire-format / round-policy implications point the wrong way | Reject explicitly. Operators who want it can fork. The v1.4 scope sentence is "P2WPKH + P2TR + P2SH-P2WPKH" not "every script type the crate supports" |
| **Per-participant choice of which script type their output goes to** | This is the Wasabi 2.0.3 model (50/50 SegWit v0 / Taproot — [WalletWasabi #9216](https://github.com/WalletWasabi/WalletWasabi/discussions/9216)). It is genuinely useful — but v1.0 ships fixed-denomination *equal-script-type* outputs ([output construction is at coordinator/src/bitcoin/tx.rs](coordinator/src/bitcoin/tx.rs)), and changing output script-type policy is independent of changing **input** ownership-proof verification. Scope creep into output policy turns "support BIP-322 multi-script" into "redesign output construction" | Defer. v1.4 keeps outputs at the round's denomination + a single output script-type (most likely P2WPKH for now, since that's what the v1.0 PSBT assembly produces). Outputs-per-script-type is a follow-up, naturally a v1.5+ "mixed output policy" milestone |
| Per-script-type ban-duration policies | Inviting operators to invent different ban policies per script type is asking for them to leak script-type info via observed ban durations | One ban duration, applied uniformly. Already the v1.0 design |
| Per-script-type round denominations | E.g. "P2TR round at 0.05 BTC, P2WPKH round at 0.01 BTC". Splits liquidity, fragments anon sets | One denomination per round, scripts mix inside it. Already the v1.0 design |

---

## Uniform-vs-mixed-script rounds — privacy + protocol implications

This is the discuss-phase decision flagged in the v1.4 milestone goal. Two coherent answers exist; v1.4 should pick one and document it explicitly.

### Option A — Uniform per round (one script type per round)

**How it works**: coordinator runs separate rounds for P2WPKH, P2TR, and P2SH-P2WPKH inputs. Each round's `/round/info` advertises a single script type. A client wanting to mix a P2TR input must wait for the next P2TR round.

**Privacy:** stronger per-round anon-set semantics — every input in the round looks identical at the script level, so the round transaction itself is *uniformly* P2WPKH-spending or uniformly P2TR-spending. No partition vector inside the round.

**Cost:** liquidity fragmentation. With three script types and the v1.0 `min_participants` threshold, cold-start time triples on signet, and even on mainnet you split the addressable participant pool by 3.

**Implementation cost:** medium. Round state machine already supports one round at a time; you'd queue three independent round contexts, advertise three independent PKARR records (one per supported type), and either run them sequentially or run multiple coordinators with different keypairs.

### Option B — Mixed within a round (Wasabi precedent)

**How it works**: a single round accepts any of the three script types as inputs. The final CoinJoin transaction has heterogeneous input scriptPubKeys. Outputs are still uniform (a single output script type for the round).

**Privacy:** the round transaction is *fingerprinted as a multi-input-type transaction*, which is itself rare for organic spending and so already screams "CoinJoin." But this is already the case for v1.0 with uniform P2WPKH inputs and uniform P2WPKH outputs — Wasabi rounds are described as "very easily fingerprinted" today. The marginal fingerprint cost of mixing input types in a round that's already fingerprinted as CoinJoin is low. The *gain* is anon-set: a 30-participant mixed round provides a 30-input anon set, whereas three 10-participant per-script-type rounds provide only 10 each.

**Cost:** input-side partitioning analysis. A chain analyst can still cluster the round's inputs by script type and observe that, e.g., the 5 P2TR inputs almost certainly map to the 5 outputs that some wallet later spends as P2TR (if outputs were heterogeneous — but they aren't, so this partition collapses).

**Implementation cost:** low. The verifier dispatches per input on the script type, but the round-level invariants (denomination, participant count, blame, etc.) are unchanged. This is closer to "remove the gate, add three verifiers" — which is the literal v1.4 milestone goal.

**Wasabi precedent ([WabiSabi PR #8912 "Support taproot (coordinator side)"](https://github.com/zkSNACKs/WalletWasabi/pull/8912) + [discussion #9216](https://github.com/WalletWasabi/WalletWasabi/discussions/9216))**: Wasabi accepts mixed input types within a round, with operator opt-in via `AllowP2trInputs`. The output-type choice is then made per participant ~50/50 to reduce output-side fingerprinting. Wasabi shipped 2.0.3 with mixed input rounds and has not retracted the decision.

### Recommendation: Option B — mixed within a round

**Rationale:**

1. The v1.4 milestone goal is "broaden CoinJoin participation," not "redesign the round state machine." Option A is a state-machine change; Option B is a verifier dispatch change.
2. Wasabi shipped Option B and the privacy analysis stood up to two years of community review. Diverging from precedent here requires a stronger argument than "uniformity feels safer."
3. Outputs stay uniform in v1.4 (single output script type, denomination-equal). The dominant privacy lever — output uniformity — is unaffected by input-side mixing.
4. Anon-set math favours mixed rounds at small participant counts, which is exactly the regime blindjoin operates in on signet.
5. The Option A separation can be retrofitted as a v1.5+ operator policy ("uniform-input rounds only" flag) without breaking the v1.4 wire format. The reverse retrofit is much more invasive.

**Discuss-phase action**: this is the decision to ratify or overturn. If overturned in favour of Option A, the PKARR record schema and `/round/info` shape change significantly (per-script-type round IDs); plan-phase should not derive tasks until this is settled.

---

## How "supported script types" is typically advertised (ecosystem precedent)

### Wasabi / WabiSabi

Operator config (`WabiSabiConfig.json`) has dedicated boolean flags. Two relevant pairs:

- `AllowP2trInputs` / `AllowP2trOutputs` ([WalletWasabi PR #8912](https://github.com/zkSNACKs/WalletWasabi/pull/8912))
- The default-enabled SegWit v0 input/output flags (not explicitly named in the public config since SegWit v0 is always on)

The coordinator does not "advertise" supported types in a discovery sense — clients query the coordinator's `/api/v4/btc/{network}/wabisabi/status` and receive the active round parameters, which implicitly indicate what's accepted. This works because Wasabi clients know in advance which coordinator they're using (hardcoded list, no DHT).

### JoinMarket

Wallet-wide script-type setting (bech32 default since v0.8.0, BIP-49 wrapped-segwit alternative — see [JoinMarket-Docs/High-level-design.md](https://github.com/JoinMarket-Org/JoinMarket-Docs/blob/master/High-level-design.md)). Coordination (the IRC message-board protocol) does not negotiate script types per round; the wallet defaults to one type at setup time. Taproot is not in current JoinMarket releases as of mid-2026.

### Bitcoin Core `getdescriptorinfo` / `listdescriptors`

Returns a descriptor string (`wpkh(...)`, `tr(...)`, `sh(wpkh(...))`) and a `checksum`. The descriptor language *is* the script-type advertisement — there's no separate "supported types" list, the descriptor encodes it. Not directly applicable to blindjoin's wire format but worth noting as the canonical "how Bitcoin Core talks about script types."

### Recommended advertisement for blindjoin

**PKARR `_blindjoin` TXT record** — add field `script_types` as a sorted, lowercase, comma-separated string:

```json
{
  "type": "blindjoin-coordinator",
  "version": "0.2.0",
  "onion": "...",
  "network": "signet",
  "denomination_sats": 1000000,
  "min_participants": 3,
  "status": "input_reg",
  "script_types": "p2sh-p2wpkh,p2tr,p2wpkh"
}
```

Rationale for comma-separated string (not JSON array):

- The PKARR record is already JSON-in-a-TXT, capped at 255 bytes per DNS character-string. A comma-separated string saves 4 bytes per item over a JSON array (`"p2wpkh","p2tr","p2sh-p2wpkh"` vs `p2wpkh,p2tr,p2sh-p2wpkh`) — meaningful given we're already at warning threshold 220 bytes in [`pkarr_pub.rs:76`](coordinator/src/discovery/pkarr_pub.rs:76).
- DNS-SD convention (RFC 6763) is key=value strings, and value-as-comma-list is a recognized DNS-SD pattern. PKARR inherits DNS-SD's conventions through its TXT record format.
- Sorted lexicographically + lowercased so the field is canonical (clients can compare strings byte-for-byte without parsing).

**`/round/info` JSON** — same field, this time as a proper JSON array (no byte budget):

```json
{
  "round_id": "...",
  "denomination_sats": 1000000,
  "min_participants": 3,
  "current_participants": 7,
  "phase": "input_reg",
  "script_types": ["p2sh-p2wpkh", "p2tr", "p2wpkh"]
}
```

Client behaviour: if the user's input is of a script type **not in** `script_types`, the client must abort with a clear error *before* opening a Tor circuit for registration. This is a hard gate on the client side — saves the user a wasted circuit + saves the coordinator a rejected registration.

**String values**: lowercase, hyphen-separated, no version suffix. Use `p2wpkh`, `p2tr`, `p2sh-p2wpkh`. Reserved for the future: `p2tr-script`, `p2wsh`. **Do not** use the descriptor language (`wpkh`, `tr`, `sh(wpkh)`) — Bitcoin Core's descriptor strings have nested parens that survive poorly in JSON-in-DNS-TXT.

---

## Spec test vectors — what exists, where to source

Test vectors are essential for the per-script-type property tests called out in the v1.4 milestone goal. Source priority:

1. **`bip-0322/basic-test-vectors.json` in the bitcoin/bips repo** — official spec vectors covering message hashing, `to_spend` / `to_sign` transaction hashes, and the "simple" variant. Includes empty-message and "Hello World" cases. Path: [`github.com/bitcoin/bips/blob/master/bip-0322/`](https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki) (the test-vector subdirectory). **HIGH confidence — this is the canonical spec source.** Status: a PR ([#1323 — "Fix incorrect signature test vectors in BIP322"](https://github.com/bitcoin/bips/pull/1323)) and the recently merged "BIP-322: add clarifications and more test vectors" indicate the vectors are being actively expanded — pin a specific commit SHA when checking them in.
2. **`rust-bitcoin/bip322` crate's own test suite** — the v0.0.10 crate runs against (a subset of) the BIP-322 vectors plus its own constructed cases. Use these to validate that our adoption of the crate gives matching results, but don't treat them as authoritative — the crate is 0.0.x and could have its own bugs.
3. **`andrewtoth/bip322` (referenced in spec discussions)** — a separate Rust impl with its own vectors. Useful as a third-party cross-check if discrepancies appear.
4. **`btcsuite/btcd/btcutil/bip322` test suite** ([guggero/btcd test fork](https://github.com/guggero/btcd/blob/f0d8719873ac70412dd813ef6e81358864c4eaa3/btcutil/bip322/bip322_test.go)) — Go implementation referenced by the spec community as a "complete" vector source. Use for cross-language sanity-check on edge cases.
5. **`bip322-js` (ACken2)** ([github.com/ACken2/bip322-js](https://github.com/ACken2/bip322-js)) — JS implementation with verifier tests. Helpful for understanding the edge cases the JS community has found, especially around Taproot 64-vs-65 byte signatures.

**For Taproot key-path specifically** — vectors are sparse in the BIP-322 file because BIP-322 was written before Taproot was widely deployed. Cross-reference with `bip-0341/wallet-test-vectors.json` ([sipa/bip341 test PR #1225](https://github.com/bitcoin/bips/pull/1225)) to validate sighash construction independently, then exercise the BIP-322 sighash by signing a known message with a known x-only pubkey and confirming `bip322::verify_simple` accepts it.

**For P2SH-P2WPKH** — the official vectors **do** include this path. Use them directly.

**Property test strategy** (per the milestone goal "Per-script-type property tests over BIP-322 spec vectors"):

- Use `proptest 1.x` (already in the stack).
- Generate arbitrary (privkey, message) pairs for each script type.
- Sign with our client path (BDK descriptor → PSBT sign → extract witness), verify with `bip322::verify_simple`.
- Cross-property: parse our existing v1.0 P2WPKH witness with `bip322::verify_simple` and confirm it accepts — this is the regression-safety guarantee for the migration off `shared/src/bip322.rs`.
- Spec-vector test: for each documented vector in `basic-test-vectors.json`, assert `verify_simple` returns `Ok(())`.

---

## Feature dependencies

```
PKARR `script_types` field (compact CSV in TXT, JSON array in /round/info)
  ↓ depends on
Coordinator config (allow_p2wpkh, allow_p2tr, allow_p2sh_p2wpkh booleans)
  ↓ feeds
Coordinator verifier dispatch (replace is_p2wpkh hard gate at utxo.rs:119)
  ↓ depends on
`bip322` crate v0.0.10 adoption (verify_simple path for all three types)
  ↓ feeds back into
shared/src/bip322.rs removal or thin wrapper (DELETE if crate is solid; thin
  wrapper if discuss-phase decides to keep the constructor for testing)

Client-side script-type detection on user input
  ↓ depends on
Coordinator `script_types` advertisement (PKARR + /round/info)
  ↓ feeds
Client pre-flight rejection of mismatched coordinator

BDK descriptor / wallet supports tr() and sh(wpkh()) (already shipped in bdk_wallet 2.3)
  ↓ feeds
Client PSBT signing for P2TR + P2SH-P2WPKH (extract witness post-sign for BIP-322 payload)

Liquidity bot script-type-aware UTXO generation
  ↓ depends on
Operator config (which script types to generate for)
  ↓ feeds
Cold-start coverage of all enabled types in integration tests

Spec test vectors (BIP-322 basic-test-vectors.json)
  ↓ feed
Property tests (proptest) per script type
  ↓ gate
Removal of the P2WPKH-only hard gate
```

The critical path for v1.4 is: **`bip322` crate adoption → verifier dispatch → property tests against spec vectors → remove hard gate → advertise → update client → update bot.** Discuss-phase resolves the crate-vs-extend-custom decision and the uniform-vs-mixed-rounds decision before plan-phase derives tasks.

---

## MVP Recommendation for v1.4

Given v1.0–v1.3 is shipped and the coordination protocol works, "MVP" here means "smallest credible delivery of multi-script support that lets us update PROJECT.md's 'forward compatible' claim to match code."

**Must ship in v1.4:**

1. Adopt `bip322 0.0.10`, replace `shared/src/bip322.rs` verification path (constructor may stay as thin wrapper for test convenience — discuss-phase decides)
2. Coordinator verifier accepts P2WPKH, P2TR (BIP-86 key-path), P2SH-P2WPKH; hard gate at `utxo.rs:119` removed
3. Three operator-tunable config flags (`allow_p2wpkh`, `allow_p2tr`, `allow_p2sh_p2wpkh`, defaults all `true`), validated at startup
4. PKARR + `/round/info` advertise `script_types` derived from the config
5. Client rejects mismatched coordinator before registration
6. Client signs ownership proofs for all enabled types via existing PSBT path
7. Liquidity bot generates UTXOs across enabled types
8. Mixed-script-type end-to-end integration test on regtest (the v1.4 acceptance gate)
9. Property tests against BIP-322 basic-test-vectors.json for each script type

**Defer to v1.5+ explicitly:**

- Mixed output script types (Wasabi 2.0.3-style per-participant output choice) — separate output-policy milestone
- P2TR script-path ownership proofs — needs a script interpreter, no demonstrated demand
- P2WSH multisig — Wasabi never shipped it; high crypto complexity, low user-base impact
- Per-script-type metrics, ban tracking, rate limits — counterproductive (see Differentiators)

**Out of scope, forever (in current PROJECT.md sense):**

- Legacy P2PKH ownership proofs — privacy anti-pattern
- Bare P2SH (raw multisig in P2SH wrapper) — must remain a hard reject; only P2SH-P2WPKH (P2SH-wrapped SegWit v0 P2WPKH) is accepted under the "P2SH" umbrella

---

## Phase-specific notes for plan-phase

| Topic | Concern | Mitigation |
|-------|---------|-----------|
| `bip322` crate stability | 0.0.x crate, last release 9 months ago — risk that the API doesn't match what docs.rs shows, or has a known bug | Discuss-phase first task: verify crate version + run a smoke test against BIP-322 basic-test-vectors before commit. Fallback: extend `shared/src/bip322.rs` with P2TR + P2SH-P2WPKH paths inline |
| Witness wire format breaking change | v1.0 wire format is `Vec<Vec<u8>>` (P2WPKH 2-item). P2TR is 1-item — is the existing deserializer permissive about 1-item lists? | Audit `coordinator/src/api/round.rs` request body deserialization. If it asserts length == 2 anywhere, that's a breaking change. Bump the wire-version field on `/round/register_input` to v2 if needed |
| Sighash-type byte handling for P2TR | The current code strips a trailing `0x01` for ECDSA. P2TR allows 64-byte sigs (no sighash byte) — make sure the new path doesn't accidentally over-strip | The `bip322` crate handles this; adopting it eliminates the manual branch. Property test: round-trip a 64-byte Schnorr sig and a 65-byte (with explicit SIGHASH_ALL) Schnorr sig |
| PKARR record byte budget | Already at warning threshold 220 / 255 bytes per [`pkarr_pub.rs:76`](coordinator/src/discovery/pkarr_pub.rs:76). Adding `script_types` CSV pushes closer | Compact CSV not JSON array; consider trimming `"type": "blindjoin-coordinator"` field if budget actually breaches. If we exceed 255 bytes, PKARR supports multiple TXT character-strings per record (RFC 1035 §3.3.14), but parsing concatenation correctly is fragile — prefer staying under 255 |
| Liquidity bot determinism on signet | Generating a P2TR or P2SH-P2WPKH UTXO needs a derived key + funding txn; on signet, this means waiting for a confirmation | Bot already does this for P2WPKH; the per-script-type cost is just additional derivation + funding txns. Should not block. |
| Cross-script-type round invariants | Round-state assertions like "all inputs are the same denomination" still apply; "all inputs are the same script type" must NOT apply | Audit `coordinator/src/round/state.rs` for any latent per-script-type assertions. The original P2WPKH-only constraint may have crept into more than just the `is_p2wpkh()` gate |

---

## Sources

### BIP-322 spec
- [bip-0322.mediawiki (bitcoin/bips repo)](https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki) — canonical spec, HIGH confidence
- [bips.dev/322/](https://bips.dev/322/) — rendered spec, HIGH confidence
- [bips.xyz/322](https://bips.xyz/322) — alt rendering with simple-variant flow diagram, HIGH confidence
- [BIP-322 PR #1323 — Fix incorrect signature test vectors](https://github.com/bitcoin/bips/pull/1323) — confirms vectors are still being corrected, MEDIUM confidence on stability of any given vector

### Rust BIP-322 crates
- [crates.io/crates/bip322](https://crates.io/crates/bip322) — v0.0.10, supports P2WPKH / P2TR / P2SH-P2WPKH single-sig, HIGH confidence on script-type coverage / MEDIUM on API stability
- [docs.rs/bip322/latest/bip322/fn.verify_simple.html](https://docs.rs/bip322/latest/bip322/fn.verify_simple.html) — `verify_simple(address: &Address, message: impl AsRef<[u8]>, signature: Witness) -> Result<(), Error>`, HIGH confidence
- [github.com/rust-bitcoin/bip322](https://github.com/rust-bitcoin/bip322) — repo + issue tracker, HIGH confidence on scope
- [lib.rs/crates/bip322](https://lib.rs/crates/bip322) — version, MSRV (1.63), HIGH confidence

### Wasabi / WabiSabi precedent
- [WalletWasabi PR #8912 "Support taproot (coordinator side)"](https://github.com/zkSNACKs/WalletWasabi/pull/8912) — `AllowP2trInputs` / `AllowP2trOutputs` config flag pattern, HIGH confidence
- [WalletWasabi #9216 Taproot support discussion](https://github.com/WalletWasabi/WalletWasabi/discussions/9216) — design rationale + 50/50 output policy, HIGH confidence
- [lontivero.github.io/Wiki Taproot support](https://lontivero.github.io/Wiki/html/wasabi/support_taproot.html) — operational confirmation, MEDIUM confidence
- [docs.wasabiwallet.io CoinJoin](https://docs.wasabiwallet.io/using-wasabi/CoinJoin.html) — confirms mixed input + 50/50 output behaviour since v2.0.3, HIGH confidence

### JoinMarket precedent
- [JoinMarket-Docs/High-level-design.md](https://github.com/JoinMarket-Org/JoinMarket-Docs/blob/master/High-level-design.md) — single-script-type per wallet, MEDIUM confidence

### Test vectors
- [bip-0341/wallet-test-vectors.json](https://github.com/bitcoin/bips/blob/master/bip-0341/wallet-test-vectors.json) — Taproot sighash cross-check, HIGH confidence
- [btcd bip322 test suite (guggero fork)](https://github.com/guggero/btcd/blob/f0d8719873ac70412dd813ef6e81358864c4eaa3/btcutil/bip322/bip322_test.go) — Go cross-impl vectors, MEDIUM confidence
- [bip322-js Verifier tests](https://github.com/ACken2/bip322-js/blob/main/test/Verifier.test.ts) — JS cross-impl, especially Taproot 64-vs-65 byte edge cases, MEDIUM confidence

### PKARR / DNS-SD encoding
- [github.com/pubky/pkarr](https://github.com/pubky/pkarr) — canonical PKARR repo, HIGH confidence
- [crates.io/crates/pkarr](https://crates.io/crates/pkarr) — Rust crate version, HIGH confidence
- [RFC 6763 (DNS-SD)](https://www.ietf.org/rfc/rfc6763.txt) — TXT record key=value convention, value-as-list pattern, HIGH confidence
- [iroh DNS blog](https://www.iroh.computer/blog/iroh-dns) — concrete PKARR TXT key=value usage example, MEDIUM confidence

### Script-type technical references
- [learnmeabitcoin P2WPKH](https://learnmeabitcoin.com/technical/script/p2wpkh/) — witness stack arity reference, HIGH confidence
- [learnmeabitcoin P2TR](https://learnmeabitcoin.com/technical/script/p2tr/) — Taproot key-path mechanics, HIGH confidence
- [BIP-143 (segwit v0 sighash)](https://github.com/bitcoin/bips/blob/master/bip-0143.mediawiki) — P2WPKH + P2SH-P2WPKH sighash algorithm, HIGH confidence
- [BIP-341 (taproot)](https://bips.dev/341/) — key-path sighash, Schnorr, SIGHASH_DEFAULT, HIGH confidence
- [BIP-86 (single-key P2TR derivation)](https://bips.dev/86/) — derivation scheme used by BDK `tr(...)` descriptors, HIGH confidence
- [Coldcard proof-of-reserves-bip-322 doc](https://github.com/Coldcard/firmware/blob/master/docs/proof-of-reserves-bip-322.md) — operational notes on P2SH-P2WPKH redeem-script handling, MEDIUM confidence
