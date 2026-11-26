# Design proposal: coordinator key / round-parameter consistency

| Field | Value |
|---|---|
| **Status** | Proposed — for review, not yet implemented |
| **Issues** | [#10](https://github.com/johnzilla/blindjoin/issues/10), [#11](https://github.com/johnzilla/blindjoin/issues/11), [#12](https://github.com/johnzilla/blindjoin/issues/12) |
| **Affects** | [docs/PROTOCOL.md](../PROTOCOL.md) (normative spec, once accepted) |

This is a proposal under review. [docs/PROTOCOL.md](../PROTOCOL.md) remains the
normative specification; nothing here is implemented or normative until accepted
and merged into it.

## Problem (#10)

A malicious coordinator can serve a **distinct RSA blind-signing key — or a
distinct `round_id` — to each client**. Each client blinds under the key it was
given; at output registration the unblinded signature verifies under that
per-client key, so the coordinator re-links input → output. This breaks the primary
unlinkability goal under exactly the malicious-coordinator model the blind signature
is meant to defend against.

Current state that enables it:

- `round_id` is `Uuid::new_v4()` (`coordinator/src/round/state.rs`) — opaque, commits
  to nothing (**#11**).
- The client's only key check is self-referential: `SHA-256(pk_der) ==
  rsa_pubkey_hash`, both taken from the same `/info` response
  (`client/src/round/input.rs`). It catches corruption, not equivocation.
- The PKARR record carries no key/round commitment (`type, v, onion, n, ds, mp, st,
  sst, ost`), and clients never cross-check keys (**#12**).
- `/info` is fetched over the Alice (input-registration) circuit
  (`client/src/http.rs`).

## Key observation: the client already trusts an identity key

The production client is launched with `--pkarr-pubkey <coordinator Ed25519
identity>` and discovers the coordinator by resolving that key from the DHT. So the
client holds an **out-of-band-trusted identity key before any round begins**. That is
the anchor this design uses — no new trust root, and no per-round DHT write.

## Proposal

### 1. Bind round parameters (including the RSA key) into `round_id` (#11)

Define a canonical, byte-exact serialization of the round parameters:

```
canonical_params =
    wire_version ‖ network ‖ denomination_sats ‖ min_participants ‖ max_participants
      ‖ supported_script_types (canonical order) ‖ output_script_type
      ‖ rsa_pubkey_SPKI_DER
```

`round_id = SHA-256(canonical_params)`. The client recomputes it from the `/info`
fields and rejects on mismatch. The signing key is now inseparable from the round
identity: the coordinator cannot vary the key without varying `round_id`.

Sequencing: the round RSA key must be generated **before** `round_id` is derived
(today `round_id` is minted at the `→ Idle` edge, before the next round's key
exists).

### 2. Certify the round with the identity key (closes #10)

Recomputing `round_id` alone does not stop a coordinator from serving per-client
`(params, key, round_id)` triples that are each internally consistent. Anchor it: the
coordinator **signs `round_id` with its PKARR Ed25519 identity key**, and `/info`
returns that signature. The client verifies it against the identity key it was given
on the command line (and resolved from the DHT).

A per-client key now requires a valid identity-key signature over the matching
`round_id` — forgery-hard — and every client verifies against the *same* identity
key. This also sidesteps DHT-propagation timing: the per-round ephemeral key never
needs to be published to the DHT (which propagates on the order of minutes); only the
stable identity key lives there, certifying each round live.

### 3. Circuit hygiene for `/info`

Fetch `/info` (and the certificate) over an isolated circuit distinct from input
registration — or fetch it twice over isolated circuits and compare — so the `/info`
connection cannot be used to correlate and tag. Lower value once (2) lands, but cheap.

### 4. Cross-client proof observation (#12) — defense-in-depth

Once `round_id` binds the parameters and is identity-signed, #12 is largely
subsumed: a coordinator cannot present different signed `round_id`s to different
clients. Options: keep #12 as optional defense-in-depth (expose the registered
ownership proofs, which commit to `round_id`, for clients to compare), or close it as
subsumed by (1)+(2). Recommended: the latter, with a note.

## Scope and compatibility

- Applies to the **PKARR-discovery path** (production). The `--coordinator-url` direct
  path has no identity anchor and remains "trust the operator out of band" — already
  the model; document it.
- Wire changes are additive: `/info` gains a signature and an identity-pubkey field,
  and `round_id` changes from random to derived. This bumps the wire version.

## Residual (explicitly not closed)

A coordinator can still run **two distinct identities** and split clients across them.
That is an anonymity-set **partition**, not within-identity equivocation, and it is
visible (different advertised pubkeys). Out of scope for this cluster; it belongs in
the threat model as a separate residual.

## Open questions / feasibility gates

1. **pkarr identity-key signing.** Does `pkarr::Keypair` expose raw Ed25519 signing of
   arbitrary bytes (to sign `round_id`)? Confirm before implementation; if not, derive
   a signing key from the same seed.
2. **Canonical parameter encoding** must be specified byte-exactly for
   cross-implementation clients, as the PKARR record and the BIP-322 challenge already
   are.
3. **Existing `round_id` consumers.** The BIP-322 challenge string and the HMAC session
   token embed `round_id` as an opaque string, so a derived value is drop-in; the
   re-sequencing (key before `round_id`) must preserve the `Idle → InputReg` transition
   invariants.
4. **Couples to #15 (silent payments).** SP needs the round input set revealed before
   output registration; the same identity-signed channel can carry that commitment.
5. **Is identity-key certification of `round_id` sufficient**, and does the canonical
   encoding need anything beyond the fields listed? (Review question.)

## Implementation plan (phased)

- **Phase 1 — coordinator.** Generate the round RSA key at round start; derive
  `round_id = SHA-256(canonical_params)`; sign it with the identity key; expose the
  signature + identity pubkey on `/info`. (`round/state.rs`, `round/manager.rs`,
  `api/handlers.rs`, `discovery/pkarr_pub.rs`, `shared/src/protocol.rs`.)
- **Phase 2 — client.** Recompute `round_id` from `/info`; verify the identity-key
  signature against the `--pkarr-pubkey` identity; reject on mismatch; fetch `/info`
  over an isolated circuit. (`client/src/round/input.rs`, `client/src/http.rs`,
  `client/src/discover.rs`.)
- **Phase 3 — #12** proof observation, or close as subsumed with a note.
- **Spec + tests.** Update PROTOCOL.md (`round_id` definition, `/info` certificate,
  client verification steps) and the threat model (goal 1 moves from "not met" to "met
  against within-identity equivocation; partition residual noted"). Tests: the client
  rejects a `round_id ≠ H(params)` and a key not covered by a valid identity-key
  signature.

## What this closes

- **#10** within-identity equivocation — closed (identity-signed `round_id`).
- **#11** — closed (`round_id = H(params incl. key)`).
- **#12** — subsumed by (1)+(2), or kept as defense-in-depth.
- Does **not** address the distinct-identity partition (threat-model note), or #13 /
  #15 (tracked separately).
