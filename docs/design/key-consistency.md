# Design proposal: coordinator key / round-parameter consistency (rev 4)

| Field | Value |
|---|---|
| **Status** | Proposed — rev 4; folds in the rev-3 review (round_id collision bound, cert epoch binding, `/round/inputs` on the carol circuit, pre-generated-key vs AUDIT-03). Not yet implemented. |
| **Issues** | [#10](https://github.com/johnzilla/blindjoin/issues/10), [#11](https://github.com/johnzilla/blindjoin/issues/11), [#12](https://github.com/johnzilla/blindjoin/issues/12); unblocks [#15](https://github.com/johnzilla/blindjoin/issues/15) |
| **Affects** | [docs/PROTOCOL.md](../PROTOCOL.md) (normative spec, once accepted) |

This is a proposal under review. [docs/PROTOCOL.md](../PROTOCOL.md) remains the
normative specification; nothing here is normative until accepted and merged.

## Problem (#10, #11, #12)

A malicious coordinator can serve a **distinct RSA blind-signing key — or a distinct
`round_id` — to each client**; the unblinded signature at output registration then
identifies the per-client key and re-links input → output.

Enabling state: `round_id` is `Uuid::new_v4()` (commits to nothing, #11); the client's
key check is self-referential (`SHA-256(pk) == hash`, both from the same `/info`);
the PKARR record carries no key commitment; `/info` rides the Alice
(input-registration) circuit.

**Authentication ≠ consistency.** A signature by the coordinator proves a common
*issuer*, not a common *value* — the coordinator holds the identity key and can
certify any number of internally-consistent per-client triples. Closing #10 needs a
channel the coordinator cannot serve divergent values on: peer observation (KC-02,
primary), the DHT record (KC-03, deterrence), multi-fetch probing (depth).

## Key observation

The client already holds an out-of-band-trusted identity key (`--pkarr-pubkey`,
resolved from the DHT) before any round begins. No new trust root. It provides
*authenticity*; the mechanisms below supply *consistency*.

## Proposal

### 1. KC-01 — derive `round_id` from canonical parameters (#11)

Byte-exact canonical serialization (fixed order; LE integers; UTF-8 strings; `u8`/`u16`
length prefixes; `sst` in the spec's canonical order):

```
"blindjoin:round-params:v1"   u8 len ‖ wire_version   u8 len ‖ network
u64 denomination_sats   u64 fee_rate_sat_per_vbyte   u32 min_participants
u32 max_participants   u8 len ‖ supported_script_types   u8 len ‖ output_script_type
u16 len ‖ rsa_pubkey_SPKI_DER
```

```
round_id = Uuid::from_slice(&SHA-256(canonical_params)[0..16])
```

- **Truncation bound (rev 4 — corrected).** The 128-bit width is forced by the
  `uuid::Uuid` wire type. The collision-relevant attack is a coordinator serving two
  *real* participants keys `K_i ≠ K_j` whose truncated `round_id`s collide, so neither
  client's KC-02 cross-check fires on the other's proof — a **birthday collision over
  real RSA-2048 keygens, ~2⁶⁴ trials** (infeasible, since each trial is a full keygen —
  but it is a collision bound, **not** the exact-match hand-wave rev 3 used, and not
  2¹²⁸). `round_id` structurally cannot reach `kh`'s 2¹²⁸ because it must fit the UUID
  type; the full-width key commitment lives in `kh` (§KC-03). The UUID wire type,
  BIP-322 challenge string, and HMAC session-token format are unchanged.
- `/info` MUST expose every canonical field verbatim (verify `max_participants`;
  add additively if absent).
- **Sequencing:** keygen before derivation, at the arm edge (but see KC-03 for the
  one-round-ahead variant and its AUDIT-03 interaction). RSA-2048 keygen (~0.1–2 s)
  on the arming path is acceptable; on failure the round stays unarmed and the monitor
  retries.
- **No freshness guard:** a fresh key per round makes `round_id` fresh; a deliberately
  repeated `round_id` only replays an ownership proof that re-proves true ownership of
  a still-unspent UTXO — no gain over the sybil residual. Saves cross-invocation client
  state.

### 2. KC-02 — input-set observation: the primary closure (#12; #15 prerequisite)

New endpoint, live from input registration through output registration:

```
GET /round/inputs → [{ outpoint, prevout_spk, chain_derived_script_type, bip322_proof }]
```

**Fetched and verified on the carol circuit, before output registration (rev 4).** The
client fetches `/round/inputs` on the isolated `/info`/poll circuit (carol, §KC-05) and
verifies — *before* it registers its output on bob — that every proof is valid against
`prevout_spk`, every proof's challenge commits to **the `round_id` it derived itself**,
and (once the PSBT is available) every PSBT input is in the observed set. Doing the
verification on carol rather than bob avoids an Alice↔Bob link and the one-shot
output-credential problem entirely (rev 3 tied this to the Bob circuit and then had to
solve a non-linking-gate problem — removed). Because carol is not linked to the input,
the endpoint can be unauthenticated or merely rate-limited; its only cost is a marginal
pre-broadcast disclosure of an input set that goes on-chain within minutes.

**Closure argument — equivocation ⇒ sybil (or a 2⁶⁴ collision).** Proofs are unforgeable
and each commits to the `round_id` — hence to the key — its client was served. To show
victim *i* a consistent, broadcastable peer set under a per-client key, the coordinator
must either (a) populate that set with inputs **it controls** — real funded UTXOs with
valid proofs committing to `R_i` — i.e. the **coordinator-sybil attack**, a pre-existing,
separately-bounded residual (per-fake-participant funding cost; anonymity-set damage
bounded by the same `min_anonymity_set` and operator-bot-ratio analysis the threat model
already carries); or (b) find a real peer `j` whose served key `K_j ≠ K_i` collides to
`R_i` (the ~2⁶⁴ real-keygen birthday bound of KC-01, infeasible). The remaining
alternative — divergent sets without sybils or a collision — is a liveness failure (the
shortchanged cohort never broadcasts). Net: the free, zero-cost tagging channel is
closed; what remains is the sybil residual at its existing price (and an infeasible
collision).

This endpoint is also exactly the input-set-before-output-reg machinery #15's BIP-352
silent payments need; the duplicate-`A` mitigation rides on it.

### 3. KC-03 — DHT record pin: timing-dependent deterrence (not closure)

The record gains:

```
"kh": base64(SHA-256(rsa_pubkey_SPKI_DER))     // full 32-byte hash, 44 chars
```

**Factual correction (rev 2 was wrong):** today only `st` changes between publishes —
`ds` and `mp` are static config — and republish runs on the ~300 s heartbeat
(`run.rs`, DISC-03), not at round arm. KC-03 therefore requires **new behavior:**
republish with the fresh `kh` tied to round arming.

**Timing tension, stated honestly:** round cadence and DHT propagation are both
minutes-scale, so a client resolving at join frequently holds the previous round's
record → mismatch → re-resolve → possibly skip the round. Honest flapping dilutes the
abort-as-signal deterrence. Mitigations: (a) generate the next round's key and publish
its `kh` at the **Idle edge, one round ahead**, giving propagation the idle + input-reg
windows; (b) bounded re-resolve (e.g. 5 attempts, 5 s apart), then skip-and-rejoin;
(c) multi-node resolve against withholding. Residual cost accepted: this is why KC-03
is ranked deterrence — the deterministic closure is KC-02.

**AUDIT-03 interaction (rev 4 — new).** The one-round-ahead key in (a) is generated at
the previous round's `→ Idle` edge, so it **must survive** that transition — but `→ Idle`
sets `RoundStateInner = None`, which is the single AUDIT-03 zeroization chokepoint
(and the subject of #14). The pre-generated next-round key therefore **cannot live in
`RoundStateInner`**; it needs a separate, explicitly-scoped field with its own
`ZeroizeOnDrop` and a documented lifetime (generated at Idle → promoted into the round
at arm → zeroized at the following `→ Idle`). Design this up front in Phase 1; it
touches the exact invariant #14 is about. If the separate-slot lifetime proves awkward,
fall back to keygen-at-arm (KC-01 sequencing) and accept weaker KC-03 propagation — KC-02
is unaffected either way.

**Why full-width `kh`:** the operative attack is a birthday collision between two
coordinator-chosen keys sharing the published hash — ~2⁶⁴ at 128-bit truncation, each
trial a real RSA-2048 keygen (~10¹⁹ — infeasible, but the keygen cost is the real
barrier, and the honest bound is 2⁶⁴). Full width raises it to 2¹²⁸ for ~22 extra bytes.
Budget: apply the spec's size-compaction decision if the worst case crosses the 220-byte
warning; update the CI pin test.

### 4. KC-04 — identity-key certificate: authenticity + evidence

```
cert_msg = "blindjoin:round-cert:v1" ‖ u64 round_epoch ‖ round_id_bytes
cert     = Ed25519_sign(identity_key, cert_msg)        // /info carries cert + round_epoch
```

`round_epoch` is a per-coordinator monotonic round counter (exposed on `/info`, bound
into the cert — **rev 4**). Verified against the **out-of-band `--pkarr-pubkey`** — never
a key learned from `/info` (rev 1's `/info`-carried pubkey is dropped: it recreates the
self-referential check one level up). The domain tag blocks PKARR-packet-signature
replay as a cert.

Buys: round authenticity, and **two valid certs over the same `round_epoch` with
divergent `round_id`s = provable *concurrent* equivocation** that burns the identity.
The epoch is what makes this evidence: without it (rev 3), two divergent-`round_id`
certs are indistinguishable from two normal *sequential* rounds. A coordinator can still
try distinct epochs for concurrent victims to dodge the evidence, but that diverges what
honest peers observe (KC-02) and is itself anomalous. Does not buy consistency (the
coordinator holds this key) — KC-04 is evidence/authenticity, KC-02 is closure.

### 5. KC-05 — circuit hygiene + poll pinning

- Third isolated Tor handle (**"carol"**) for `/info`, all polls, and `/round/inputs`
  (§KC-02); alice = input reg, bob = output reg (CLI-05 extended). Resource note: three
  persistent isolated handles = three guard selections per client process — modest,
  accepted.
- **Poll pinning:** pin `(round_id, pk_der)` at first `/info`; every later poll in the
  round must match; mismatch → abort (closes mid-round key rotation).

### 6. Double-fetch probing — depth only

Optional; defeasible by time-slicing (stable key within short windows). Never counted
toward closure.

## Scope and compatibility

- **PKARR path:** protected. **`--coordinator-url`:** no anchor; trust-the-operator
  (charter residual, unchanged).
- **Wire:** additive record + `/info` fields (incl. `round_epoch`, `cert`); new
  `/round/inputs`. `WIRE_VERSION` 0.2.0 → **0.3.0**.
- **`/round/inputs` access (rev 4):** fetched on carol, unauthenticated or rate-limited.
  Decoupling verification (carol) from output registration (bob) removes rev 3's
  gate-credential-linkability problem; the only cost is marginal pre-broadcast disclosure
  of a soon-public input set.
- **Version gate — default ON:** a 0.3.0 coordinator MUST reject clients not attesting
  `X-Blindjoin-Wire-Version ≥ 0.3.0`. Otherwise 0.2.0 clients silently participate
  unprotected and a downgrade dynamic exists. New clients + 0.2.0 coordinators: **fail
  closed**. Deploy order: coordinators first.

## Residuals (explicitly not closed)

1. **Coordinator sybil** — the explicit floor: KC-02 reduces equivocation *to* it
   (plus the infeasible 2⁶⁴ `round_id` collision). Bounded as before (funding cost per
   fake participant; `min_anonymity_set`; bot-ratio analysis).
2. **Cross-identity partition** — visible in discovery; separate threat-model entry.
3. **Record time-shift + propagation flapping** (KC-03) — deterrence with diluted signal
   under adverse timing; deterministic closure is KC-02.
4. **DHT-node withholding/staleness** — DoS-class; multi-node resolve.

## Open questions (implementation-verify only)

1. `pkarr::Keypair` raw Ed25519 signing; else `ed25519-dalek` from the same seed,
   domain-tagged per §KC-04.
2. `/info` completeness (`max_participants`, `round_epoch`).
3. Record size vs the 220-byte warning; compaction decision; CI pin test update.
4. Pre-generated next-round key vs the AUDIT-03 zeroization chokepoint (§KC-03) — confirm
   the separate-slot lifetime, or fall back to keygen-at-arm.
5. #15 sequencing on the same endpoint; duplicate-`A` check rides along.

## Implementation plan (phased)

- **Phase 0 — spec:** canonical encoding, `round_id` derivation, `kh` field + arm-tied
  republish, `round_epoch`, cert format, `/round/inputs` + carol fetch, verification
  steps, compat matrix, threat-model goal-1 language ("equivocation reduced to the sybil
  residual for PKARR-path clients").
- **Phase 1 — coordinator:** keygen (at-arm, or one-round-ahead with the separate
  zeroize-scoped slot) + `kh` publish; derived `round_id`; `round_epoch`; domain-tagged
  cert; `/round/inputs`; version gate ON. (`round/state.rs`, `round/manager.rs`,
  `api/handlers.rs`, `run.rs`, `discovery/pkarr_pub.rs`, `shared/src/protocol.rs`.)
- **Phase 2 — client:** carol circuit; recompute `round_id`; cert vs pinned identity;
  `kh` vs re-resolved record; poll pinning; input-set verification on carol before output
  registration; fail-closed on 0.2.0. (`round/input.rs`, `round/output.rs`, `http.rs`,
  `tor.rs`, `discover.rs`.)
- **Phase 3 — tests + dispositions:**
  - `round_id_derivation_is_canonical` (byte-exact proptest)
  - `client_rejects_round_id_not_matching_params`
  - `client_rejects_certificate_not_signed_by_pinned_identity`
  - `client_rejects_cross_protocol_signature`; `cert_binds_round_epoch`
  - `client_pins_round_key_across_polls`
  - `record_kh_matches_served_key`; `record_republishes_kh_at_arm`
  - `client_skips_round_on_kh_mismatch_and_rejoins_after_republish`
  - `equivocation_requires_sybil_fill` (test-seam coordinator serves a distinct key with
    an honest peer visible → client aborts on the peer's divergent-`round_id` proof)
  - `input_set_commitments_match_round_id`; `psbt_inputs_subset_of_observed_set`
  - `input_set_verified_on_carol_before_output_reg`
  - `old_record_fail_closed`; `coordinator_rejects_pre_030_clients`
  - `round_id_changes_when_key_changes`
  - `pregenerated_key_zeroized_on_following_idle` (if the one-round-ahead slot is adopted)
  - Issues: #10 closed by KC-02 (primary) + KC-03/KC-04 (deterrence/evidence); #11 by
    KC-01; #12 by KC-02. CHANGELOG Reported-by.

## What this closes (honest accounting)

- **#11** — KC-01: `round_id` is a recomputable commitment to parameters + key (collision
  bound 2⁶⁴ real keygens, capped by the UUID type; `kh` carries the full-width pin).
- **#10** — **KC-02 is the closure:** per-client-key equivocation is reduced to the
  coordinator-sybil residual (or an infeasible 2⁶⁴ `round_id` collision), which the threat
  model already bounds. KC-03 adds timing-dependent deterrence (propagation cost stated),
  KC-04 adds authenticity and epoch-bound burnable evidence. No single mechanism suffices;
  the sybil residual is the floor and is not new.
- **#12** — closed by KC-02 (peer observation is the deterministic layer).
- **#15** — prerequisite (input set before output registration, on carol) delivered;
  silent payments tracked separately.
- Not addressed: cross-identity partition, #13, direct-path trust.
