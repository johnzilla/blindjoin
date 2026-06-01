# Architecture Patterns — v1.4 BIP-322 Multi-Script Integration

**Domain:** Integrating BIP-322 Simple multi-script-type ownership proofs (P2TR, P2SH-P2WPKH alongside existing P2WPKH) into the shipped blindjoin coordinator + client + shared crate architecture.
**Researched:** 2026-05-29
**Confidence:** HIGH on integration-point identification (codebase is read in full); MEDIUM on the `bip322` crate adoption path (Sprint 0 spike still required — see Pitfall 1).

> **Scope note:** This document overwrites the v1.0 ARCHITECTURE.md. v1.0's component map remains accurate as the baseline — that material now lives in `.planning/research/STACK.md` and the shipped code itself. This document focuses *only* on the deltas required by v1.4. Treat the v1.0 layer diagram as still load-bearing; v1.4 changes are surgical.

---

## 1. Recommended Architecture (Deltas Only)

v1.4 is a **per-crate surgical extension**, not a re-architecture. No new components. No new layers. No new external dependencies beyond (optionally) the upstream `bip322` crate. The Tor/PKARR/HTTP/round-FSM layers are unaffected at the architectural level.

```
shared/                               coordinator/                          client/
─────────                             ────────────                          ───────
src/bip322.rs                         src/bitcoin/utxo.rs                   src/wallet.rs
  (MODIFY: per-script               ←  (MODIFY: replace is_p2wpkh()         (MODIFY: BIP-84 →
   verify dispatch +                    gate with allowlist + dispatch)      [BIP-84, BIP-86,
   to_spend / to_sign                                                        BIP-49] descriptors)
   transactions are
   already script-type-               src/round/state.rs                    src/round/input.rs
   neutral — keep them)               (MODIFY: RegisteredInput +              (MODIFY: per-type
                                       script_type field for                   BIP-322 witness
src/protocol.rs                        diagnostics/logging)                    generation; dispatch
  (MODIFY: OwnershipProof                                                     by descriptor type)
   gains script_type;                 src/config.rs
   InfoResponse +                     (NEW: BipConfig section
   supported_script_types)             with supported_script_types)         liquidity-bot
                                                                            (MODIFY: generate test
                                      src/discovery/pkarr_pub.rs              UTXOs across script
                                      (MODIFY: include                         types via separate
                                       supported_script_types in              BIP descriptors)
                                       PKARR DNS TXT JSON payload)

           ▲                                    ▲                                    ▲
           │                                    │                                    │
           └──── single shared verifier ────────┴──────── single shared signer ──────┘
                                                    (via shared::bip322 module)
```

**Architectural principle preserved:** `shared/src/bip322.rs` remains the single source of truth so coordinator (verify) and client (sign) produce byte-identical to_spend/to_sign transactions per script type. v1.0's protection against format-mismatch (threat T-01-05) extends to v1.4: each new script-type implementation adds matched sign/verify code paths to the same module.

---

## 2. Per-Crate Changes

### 2.1 `shared` crate — new public surface, fully backwards-compatible

| File | Change | Rationale |
|------|--------|-----------|
| `shared/src/bip322.rs` | **MODIFY.** Keep existing `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign` unchanged — they are script-type-neutral per spec (Sections 4/5). Add a new `ScriptType` enum, a `detect_script_type(&Script) -> Option<ScriptType>` helper, and per-type `sign_simple` / `verify_simple` dispatch functions. | The `to_spend`/`to_sign` virtual TX construction is identical across script types; only the **sighash function** and **witness stack interpretation** differ. Reusing the shared transaction builders prevents drift. |
| `shared/src/protocol.rs` | **MODIFY `OwnershipProof`** (line 113): add `script_type: ScriptType` field. The witness_stack stays as `Vec<Vec<u8>>` — it remains opaque bytes interpreted by the per-type verifier. The wire format upgrades from JSON `[...]` array to a JSON object `{"script_type":"p2wpkh","witness":[...]}`. Helper methods `to_json_str` / `from_json_str` replace the existing `to_json_hex_str` / `from_json_hex_str` (kept as deprecated aliases for one release). | Coordinator needs script type to dispatch; relying on `detect_script_type(spk)` from gettxout is fine for verify but the client must declare intent because P2SH-P2WPKH and P2WPKH have different witness layouts that the coordinator must validate **before** running the verifier. |
| `shared/src/protocol.rs::InfoResponse` | **MODIFY:** add `supported_script_types: Vec<ScriptType>` field with `#[serde(default)]` so old coordinators that omit it deserialize as empty (interpreted as "P2WPKH only" — backwards-compat sentinel). | Allows client to validate compatibility before doing any signing work. |
| `shared/src/bip322.rs` (new fn) | **NEW `sign_simple(script_type, secret, message) -> Witness`** — internal helper used by `client::wallet`. **NEW `verify_simple(script_type, script_pubkey, witness_stack, message) -> Result<(), Bip322Error>`** — replaces the existing coordinator-side `verify_bip322_simple` (currently inlined at `coordinator/src/bitcoin/utxo.rs:114`). | Moves the verification logic out of the coordinator and into `shared` so the client's own test suite can prove sign↔verify round-trip without depending on the coordinator crate. |

**Decision: roll our own multi-script verifier vs. adopt `bip322` crate.** This is the largest open question and gates the Phase 1 plan. See Section 6 (Open Decisions). The architecture is identical either way — only the implementation of `shared::bip322::{sign_simple, verify_simple}` changes. If we adopt the crate, those become thin shims; if we extend our impl, they grow ~150 LOC per new script type.

### 2.2 `coordinator` crate — replace the gate, thread the script type

| File | Change | Rationale |
|------|--------|-----------|
| `coordinator/src/bitcoin/utxo.rs` | **REPLACE the is_p2wpkh gate.** Current line 119: `if !script_pubkey.is_p2wpkh() { return Err(Bip322Error::UnsupportedScriptType); }`. New logic: detect script type from the gettxout-returned scriptPubKey, check against `config.bip322.supported_script_types` allowlist, then dispatch to `shared::bip322::verify_simple(script_type, ...)`. The local `verify_bip322_simple` function is **deleted** — its responsibilities move to `shared::bip322`. | This is the literal load-bearing change for v1.4. Everything else exists to support it. |
| `coordinator/src/round/state.rs::RegisteredInput` | **ADD `script_type: ScriptType`** field (mark `#[zeroize(skip)]` — it's a public-chain attribute, not PII). | Useful for: (a) structured-log diagnostics ("round N had 2 P2WPKH + 1 P2TR inputs"), (b) blame-protocol input-mix analysis if needed for future invariants, (c) downstream PSBT assembly knowing per-input witness shape. Not strictly required for v1.4 correctness, but free-of-cost and forward-compatible. |
| `coordinator/src/round/state.rs::RoundStateInner` | **NO CHANGE.** Does not need a `supported_script_types` field — it's a config concern, not a per-round state concern. Read from `config.bip322.supported_script_types` at validate-utxo time. | RoundStateInner is the sensitive material struct that gets zeroized on Drop; adding non-sensitive configuration there is wrong-layered. |
| `coordinator/src/config.rs` | **ADD `BipConfig` section** with one field: `supported_script_types: Vec<String>` (e.g. `["p2wpkh","p2tr","p2sh-p2wpkh"]`). Default in `with_defaults()` = all three. Surface via env: `BLINDJOIN__BIP322__SUPPORTED_SCRIPT_TYPES`. Validate at startup: empty vec or unknown string is a fatal config error. | Operator knob. Lets an operator who is on a stale code path opt back into P2WPKH-only. Critical for the rollout story (an operator can pin to P2WPKH-only and upgrade clients independently). |
| `coordinator/src/discovery/pkarr_pub.rs` | **MODIFY `build_coordinator_packet`** to add `supported_script_types: ["p2wpkh","p2tr","p2sh-p2wpkh"]` array field in the JSON payload. **Update the 220-byte warning threshold** check — JSON grows ~40 bytes — and validate the new payload fits 255 bytes (it does: roughly 175→215 bytes). Bump the version field from `"0.1.0"` to `"0.2.0"` to signal schema evolution. | Clients use this to filter coordinators **before** registering, avoiding wasted Tor circuits and avoiding a round-failure surface. The version bump is observable but harmless; the JSON parser is `serde_json::Value` so unknown fields don't break old clients. |
| `coordinator/src/api/handlers.rs` (GET `/info` handler) | **NO STRUCTURAL CHANGE — already returns `InfoResponse` from `shared::protocol`.** It automatically picks up the new `supported_script_types` field once the shared struct is extended and the handler populates it from config. | `InfoResponse` is the single contract; extending it propagates automatically. |
| `coordinator/src/round/blame.rs::detect_non_signers` | **NO CHANGE.** Logic is `registered_inputs.keys() \ partial_sigs.keys()` (line 71-77) — it operates on outpoint strings and is script-type-agnostic. | See Section 4 — blame protocol does not care about script type. |
| `coordinator/src/round/signing.rs::assemble_and_broadcast` | **NO CHANGE required at the architecture level**, but the **witness-deserialization step at line 163** (`bitcoin::consensus::deserialize::<bitcoin::Witness>(sig_bytes)`) already accepts arbitrary witness shapes, so P2TR (1-item Schnorr witness) and P2SH-P2WPKH (2-item ECDSA witness + scriptSig push of redeem script) just flow through. **Add a per-input scriptSig field** for P2SH-P2WPKH (the redeem script push) — currently coordinator only sets `final_script_witness`; for P2SH-P2WPKH it must also set `final_script_sig`. **VERIFY THIS DURING SPRINT 0.** | This is the second-largest implementation risk. Witness-only segwit inputs (P2WPKH, P2TR) need only `final_script_witness`. P2SH-P2WPKH is a wrapped-segwit input needing both `final_script_sig` (push of redeem script) and `final_script_witness`. The client must produce both; the wire format must transport both. See Pitfall 2. |

### 2.3 `client` crate — multi-script wallet + multi-script BIP-322 signing

| File | Change | Rationale |
|------|--------|-----------|
| `client/src/wallet.rs` | **EXTEND `BdkClientWallet`** to support three descriptor templates: `wpkh(...)` (P2WPKH, existing), `tr(...)` (P2TR — BIP-86), `sh(wpkh(...))` (P2SH-P2WPKH — BIP-49). `from_descriptor` already accepts arbitrary descriptor strings — extend the `internal_desc` derivation heuristic (line 82) to handle `tr(...)` and `sh(wpkh(...))` shapes, and **add a `script_type()` accessor** so the input-reg path knows which sighash to use. `generate()` extension: add `--type {p2wpkh,p2tr,p2sh-p2wpkh}` CLI flag. Detect script type from the descriptor at wallet construction. | bdk_wallet 2.3 supports all three descriptor types natively — see [bdk_wallet 2.x docs](https://docs.rs/bdk_wallet). The signing path via `wallet.sign(psbt, SignOptions { trust_witness_utxo: true })` already does the right thing per-input; the only client-side gap is BIP-322 ownership proof signing, not the round signing. |
| `client/src/round/input.rs::generate_bip322_witness` | **DELETE the inlined P2WPKH-only implementation** (lines 105-139). Replace with a call to `shared::bip322::sign_simple(script_type, secret_or_signer, message)`. For descriptor wallets that don't expose a raw secret key (line 220), use bdk_wallet's PSBT signing path against a constructed BIP-322 to_sign transaction — this is the standard "sign-via-PSBT" trick used by other BIP-322 implementations. | Single signing primitive shared across all script types. The `wallet.sign(psbt, ...)` flow already works for all three — we just need the BIP-322 message hash translated through to_sign/to_spend for each type. |
| `client/src/discover.rs` | **MODIFY** to read `info.supported_script_types` and verify the wallet's script type is in the allowlist BEFORE input registration. Hard fail with clear error: `"coordinator does not support P2TR — supported: [p2wpkh]"`. | Avoids fail-late round-failure surface; the client should never start a round it cannot complete. |
| `client/src/round/sign.rs` | **NO CHANGE** at the architecture level — the existing `sign_psbt_input` path through `bdk_wallet::Wallet::sign` already handles per-input script types. P2TR signing through bdk_wallet 2.3 with a `tr(...)` descriptor produces a 1-item Schnorr witness. The coordinator-side witness deserialization is script-type-blind. | bdk_wallet does the right thing per descriptor type. |

### 2.4 `liquidity-bot` — multi-script test UTXO generation

The liquidity bot is the test-funding rail for CI integration tests. v1.4 needs it to mint UTXOs across all three script types so the end-to-end test can register a mixed-script round.

| File | Change | Rationale |
|------|--------|-----------|
| `liquidity-bot/src/main.rs` (or wherever bot lives — see Bash `find` if not yet stubbed) | **ADD** logic to derive one address per script type from the bot's seed (`m/84'`, `m/86'`, `m/49'`), fund each via regtest `sendtoaddress` (or rely on the existing funding flow), and present each as a separately registrable UTXO. Bot needs **N descriptor wallets**, not one — or one wallet with three keychains. | Without this, the integration test cannot exercise mixed-script rounds and v1.4 ships without a smoke test that the new code paths actually compose end-to-end. |

---

## 3. Data Flow Changes

### 3.1 Input Registration — what's new

```
Client (Alice, fresh Tor circuit, P2TR wallet)
  1. GET /info → InfoResponse { supported_script_types: [p2wpkh, p2tr, p2sh-p2wpkh], ... }
  2. Verify own wallet.script_type() ∈ supported_script_types — else hard fail BEFORE registering
  3. Compute BIP-322 message: "blindjoin:round:{round_id}:utxo:{txid}:{vout}"
  4. Per script type:
       - P2WPKH:       bdk signs to_sign with p2wpkh_signature_hash (ECDSA, sighash_all) — existing
       - P2TR:         bdk signs to_sign with taproot_key_spend_signature_hash (Schnorr, sighash_default)
       - P2SH-P2WPKH:  bdk signs to_sign with p2wpkh_signature_hash AND constructs the redeem-script
                       push for scriptSig; witness format is [sig, pubkey] same as P2WPKH
  5. Wrap witness in OwnershipProof { script_type: ScriptType::P2tr, witness_stack: vec![sig_bytes] }
  6. POST /round/input with ownership_proof = OwnershipProof::to_json_str()

Coordinator
  7. Receive InputRegRequest, deserialize OwnershipProof — script_type field tells dispatcher what to verify
  8. validate_utxo:
       a. RPC gettxout → on-chain script_pubkey + value
       b. detect_script_type(&on_chain_spk) — verify it matches claimed script_type (mismatch → reject)
       c. Check script_type ∈ config.bip322.supported_script_types — else reject
       d. shared::bip322::verify_simple(script_type, &script_pubkey, &witness_stack, &message)
  9. register_input: store RegisteredInput { ..., script_type } in RoundStateInner.registered_inputs
```

**Key invariant added in v1.4:** Coordinator MUST cross-check the client-claimed `script_type` against the script type detected from the actual on-chain scriptPubKey (step 8b). A client claiming P2WPKH for a P2TR UTXO would otherwise bypass the dispatcher into the wrong verifier. This is a one-line check but it is load-bearing.

### 3.2 PKARR Discovery — schema evolution

```
v1.3 (current) JSON payload:
  { "type": "blindjoin-coordinator", "version": "0.1.0", "onion": ..., "network": ...,
    "denomination_sats": ..., "min_participants": ..., "status": ... }

v1.4 JSON payload:
  { "type": "blindjoin-coordinator", "version": "0.2.0", "onion": ..., "network": ...,
    "denomination_sats": ..., "min_participants": ..., "status": ...,
    "supported_script_types": ["p2wpkh", "p2tr", "p2sh-p2wpkh"] }
```

**Backwards-compatibility behaviour:**
- v1.3 clients reading a v1.4 record: `serde_json` deserialization with `#[serde(default)]` on the new field works fine; clients ignore the field.
- v1.4 clients reading a v1.3 record: missing `supported_script_types` deserializes as `None` / empty vec. Client interprets empty/missing as "P2WPKH only" — proven safe since v1.3 was P2WPKH-only.

Size check: current payload is ~175 bytes; new payload adds ~40 bytes → ~215 bytes, still under the 220-byte warn threshold and well under the 255-byte DNS TXT limit.

### 3.3 PSBT Assembly + Broadcast — what's new

The witness-application loop in `coordinator/src/round/signing.rs:159-187` is script-type-blind because `bitcoin::Witness` is a tagged length-prefixed byte sequence. **One change needed:** for P2SH-P2WPKH inputs, the coordinator must also populate `psbt.inputs[i].final_script_sig` with the redeem-script push, not only `final_script_witness`. The client's `sign_psbt_input` already produces both fields via bdk_wallet's full-PSBT-sign path — but the **wire format** currently transports only the witness (line 282-287). **For P2SH-P2WPKH the wire format must transport both `final_script_witness` AND `final_script_sig`.**

**Recommended wire-format extension:** Replace the current `partial_signature: base64(consensus(Witness))` field in `SignRequest` (shared/src/protocol.rs:88-93) with `partial_signature: base64(consensus(SignedInputPayload))` where `SignedInputPayload` is a new wire struct containing both witness and (optional) script_sig. Version the wire format and use a tagged enum so v1.3 partial-sig submissions (bare-witness consensus encoding) still parse. **OR** simply switch to base64-encoded full PSBT-input shape (which encodes both natively) — this is the more conservative approach that aligns with PSBT-everywhere semantics.

This is the second-most-load-bearing decision in v1.4 — see Section 6.

---

## 4. BLAME Protocol Interaction

**The blame protocol does not care about script type.** Reasoning:

1. `detect_non_signers` (coordinator/src/round/blame.rs:68-77) computes set-difference between `registered_inputs.keys()` (txid:vout strings) and `partial_sigs.keys()` (same txid:vout strings). Both keys are script-type-agnostic outpoint identifiers.
2. The ban list (BanList) keys by `SHA-256(utxo_outpoint_str)` (coordinator/src/round/blame.rs:32-37). Hashing a non-PII outpoint string; no script-type signal is stored, looked up, or compared.
3. Per-script-type ban-list partitioning would leak which script type a banned UTXO came from across rounds — undesirable, and there is no reason to want this. Keep blame uniform.

**However**, one v1.4 implication for blame: a mixed-script round where one participant submits a malformed witness should fail-fast at `bitcoin::consensus::deserialize::<Witness>` (line 163-178) with a per-input error message that includes the script type for operator diagnostics. Add `script_type` to the error log line — not the error response (don't leak per-input metadata in API responses). This is a one-line diagnostics improvement, not an architectural change.

---

## 5. Build Order

Dependencies dictate the sequence. Each phase is testable in isolation.

```
Phase 0: Sprint 0 spike (1-2 days, off-roadmap)
  └── Vendor or evaluate the upstream `bip322` crate against three test vectors per script type.
  └── Output: GO/NO-GO on `bip322` crate adoption. (See Section 6, Open Decision A.)
  └── Risk: HIGH — defers all downstream work if this slips. Cap at 2 days; if neither path is
      clearly better, default to extending the custom impl (lower migration cost, faster).

Phase 1: shared crate
  └── 1a. Add ScriptType enum + detect_script_type helper to shared/src/bip322.rs
  └── 1b. Implement sign_simple + verify_simple per script type (or wire to bip322 crate)
  └── 1c. Extend OwnershipProof with script_type; new to_json_str / from_json_str helpers
  └── 1d. Extend InfoResponse with supported_script_types (#[serde(default)])
  └── 1e. Unit tests: per-script-type sign↔verify round-trip (proptest where possible)
  └── Exits with: shared crate fully tested independent of coordinator + client

Phase 2: coordinator integration
  └── 2a. Add BipConfig section to config.rs + startup validation
  └── 2b. Replace is_p2wpkh gate in utxo.rs with allowlist + dispatch
  └── 2c. Add script_type field to RegisteredInput (no zeroize)
  └── 2d. Update PKARR publisher to include supported_script_types
  └── 2e. Update PSBT assembly to set final_script_sig for P2SH-P2WPKH inputs
  └── 2f. Unit tests: registration accepts/rejects per script-type allowlist correctly
  └── Exits with: coordinator compiles, gates on config, ignores client requests
      with wrong script_type, still passes existing v1.3 P2WPKH integration tests

Phase 3: client integration
  └── 3a. Extend wallet.rs descriptor parsing to handle tr() and sh(wpkh()) shapes
  └── 3b. Add wallet.script_type() accessor
  └── 3c. Rewire generate_bip322_witness to call shared::bip322::sign_simple
  └── 3d. Add discover.rs pre-flight check against supported_script_types
  └── 3e. Extend `client generate-wallet --type` CLI flag
  └── 3f. Update wire format for sign request if PSBT-input shape adopted (Open Decision B)
  └── Exits with: client can register and sign across all three script types against
      both v1.3 (P2WPKH-only) and v1.4 (mixed) coordinators

Phase 4: liquidity-bot + end-to-end integration
  └── 4a. Liquidity-bot extends to generate UTXOs across all three script types
  └── 4b. Integration test: mixed-script round (1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH)
            completes a full round on regtest
  └── 4c. Backwards-compat tests: v1.3 client ↔ v1.4 coordinator (P2WPKH only path)
            + v1.4 client ↔ v1.3 coordinator (refuse non-P2WPKH at discovery)
  └── 4d. Per-script-type property tests against BIP-322 spec vectors
  └── Exits with: v1.4 milestone done
```

**Rationale for this order:**
- **Phase 0 gates everything.** The crate-vs-custom decision changes effort estimates for Phase 1 by 3-5x.
- **Phase 1 (shared) first** because both coordinator and client compile against it. Without it, Phase 2 and 3 cannot iterate independently. This matches v1.0's "shared crate is the contract" pattern that was decisive in REPAIR-01.
- **Phase 2 (coordinator) before Phase 3 (client)** because the integration test runs the coordinator and the client side-by-side — the client cannot validate without something to talk to. Coordinator with a config-disabled allowlist is harmless to ship intermediate.
- **Phase 4 last** because it requires both halves working. Don't try to parallelize Phase 4 with Phase 3 — the integration test is the smoke gate.
- **At every phase boundary, the v1.3 P2WPKH integration tests must remain green.** This is the rollback safety net.

---

## 6. Open Decisions for Discuss-Phase

These are the calls Plan-phase cannot make without an explicit decision because the answer changes the build plan materially.

### Decision A: Adopt `bip322` crate vs. extend custom `shared/src/bip322.rs`

**Adopt the crate (recommended pending Sprint 0 spike):**
- **Pros:** Less code to own, less crypto surface to audit, reference test vectors included, P2WSH stretch goal becomes free, upstream is the rust-bitcoin org (governance alignment).
- **Cons:** 0.0.x version — API instability risk; the crate's signing API may not interoperate cleanly with bdk_wallet 2.3's PSBT-sign path; verify-only feature may not be a thing (we don't need signing in coordinator).
- **Migration cost:** Replace ~50 LOC of `verify_bip322_simple` with ~10 LOC of crate calls per script type.

**Extend custom impl:**
- **Pros:** Zero new dependencies, fully controlled, leverages our existing `build_bip322_to_spend` / `build_bip322_to_sign` primitives, no API instability risk.
- **Cons:** ~150 LOC per new script type, full crypto-test burden on us, slower delivery.

**Discuss-phase decision needed:** GO/NO-GO on crate adoption based on Sprint 0 findings. If GO, pin to a specific version and document the upgrade story in `STACK.md`. The constraint "No custom crypto — blind-rsa-signatures, rust-bitcoin, bdk, secp256k1 only" (PROJECT.md line 119) leans toward the crate as long as it's not in flux.

### Decision B: Partial-sig wire format for P2SH-P2WPKH

**Option B1: Extend the existing consensus-encoded Witness with a tagged enum.**
```rust
enum PartialSigPayload {
    WitnessOnly(Witness),                    // P2WPKH, P2TR
    WitnessAndScriptSig { witness: Witness, script_sig: ScriptBuf }, // P2SH-P2WPKH
}
```
Versioned tag byte at the front. Backwards-incompatible at the deserializer.

**Option B2: Switch to base64-encoded `bitcoin::psbt::Input` shape.**
PSBT inputs natively carry both `final_script_witness` and `final_script_sig`. Coordinator deserializes the PSBT input and merges into its full PSBT. This is the more conservative, future-proof option but is a larger wire-format change.

**Recommendation:** Option B2. PSBT-everywhere keeps the wire model consistent with the round PSBT contract the rest of the protocol uses, and the deserialization cost is negligible. The breaking-change cost is identical to B1 (clients have to upgrade anyway), so pick the cleaner format. Document in PROJECT.md Key Decisions table.

### Decision C: Wallet descriptor migration for existing users

Existing v1.3 users have BIP-84 (`m/84'`) wallets persisted in `descriptors.txt` (via `BdkClientWallet::generate`). v1.4 introduces optional BIP-86 (`m/86'`) and BIP-49 (`m/49'`) descriptors. Users who want to participate in P2TR rounds need a new wallet — there is no in-place migration possible because the keyspaces are derived from different paths.

**Recommendation:** Treat this as additive. `client generate-wallet` with no `--type` flag keeps producing a P2WPKH wallet (current behaviour). Users opting into P2TR run `client generate-wallet --type p2tr` and get a separate wallet file. Document this clearly — users should not expect their existing P2WPKH wallet to suddenly hold P2TR addresses. This is the standard wallet UX pattern.

---

## 7. Pitfalls and Integration Surprises

### Pitfall 1: `bip322` crate API instability (HIGH risk)
**What goes wrong:** Crate is at 0.0.x. Pinning to 0.0.N may force a breaking-change upgrade two months from now. Worse: the crate's signing API may require a `Wallet` trait shape that bdk_wallet 2.3 does not implement.
**Mitigation:** Sprint 0 spike (Phase 0). If signing API is incompatible, use the crate verify-only and keep our own sign path. If neither path is acceptable, fall back to custom impl — design `shared::bip322::sign_simple` / `verify_simple` as a stable internal trait so either backend can be swapped behind it.

### Pitfall 2: P2SH-P2WPKH wire-format surprise (HIGH risk)
**What goes wrong:** Today's wire format transports only `final_script_witness`. P2SH-P2WPKH requires `final_script_sig` AND `final_script_witness`. Shipping the new script type with the old wire format produces silent broadcast failure (the redeem script push is missing). The v1.3 REPAIR-01 chain (5 commits to fix one partial-sig wire-format mismatch) is the cautionary tale here.
**Mitigation:** Resolve Decision B before Phase 3 starts. The integration test (Phase 4) MUST broadcast a P2SH-P2WPKH input — pure unit-test coverage will not catch a deserializer-side script_sig omission.

### Pitfall 3: Coordinator script-type-claim mismatch (MEDIUM risk)
**What goes wrong:** A malicious client claims `script_type: P2WPKH` for a P2TR UTXO, hoping the dispatcher runs the wrong verifier and passes (it wouldn't, but the principle of cross-checking matters).
**Mitigation:** Cross-check `detect_script_type(on_chain_spk) == claimed_script_type` at validate-utxo time (Section 3.1 step 8b). One-line check, documented in code.

### Pitfall 4: Stale operator config (LOW risk, HIGH impact when triggered)
**What goes wrong:** Operator runs v1.4 binary but `coordinator.toml` is from v1.3 — no `[bip322]` section. Default sane behavior is critical here.
**Mitigation:** `#[serde(default)]` on the new section, with `with_defaults()` defaulting to `["p2wpkh", "p2tr", "p2sh-p2wpkh"]`. Also add a startup log line: "BIP-322 supported script types: [p2wpkh, p2tr, p2sh-p2wpkh]" so operators see the actual config.

### Pitfall 5: Tor/PKARR layer assumption (verified: NO impact)
**What might go wrong:** v1.4 might subtly need a Tor or arti-client API change.
**Investigation result:** No. The Tor layer is bytes-in/bytes-out; it does not know what BIP-322 is. The PKARR layer changes only in the JSON payload content, not in the publish/resolve API surface. `arti-client 0.41` and `pkarr 2.x` are unaffected at the integration boundary.

### Pitfall 6: bdk_wallet 2.3 P2TR support gotcha (MEDIUM risk)
**What might go wrong:** bdk_wallet's `tr(...)` descriptor support is well-tested, but the BIP-322 message hash signing requires constructing a virtual to_sign transaction; calling bdk's `wallet.sign` on a manually-built non-wallet PSBT may not work cleanly if bdk insists on its own UTXO set.
**Mitigation:** Sprint 0 should validate the BIP-322 sign-via-bdk path for P2TR, not just P2WPKH. If it doesn't work, fall back to direct `secp256k1` Schnorr signing via `secret_key_for_signing()` (extend that accessor to expose Taproot keypath secret for `tr(WIF)` descriptors). The `from_wif` path may need a P2TR variant.

### Pitfall 7: Property-test surface explosion (LOW risk, MEDIUM effort)
**What might go wrong:** Per-script-type property tests against the BIP-322 spec vectors multiply test count 3x and require holding three known-good test vectors per script type.
**Mitigation:** BIP-322 spec includes reference vectors for P2WPKH; rust-bitcoin/bip322 crate (if adopted) includes vectors for the rest. Use those rather than generating our own.

---

## 8. Backwards Compatibility Matrix

| Client | Coordinator | Result |
|--------|-------------|--------|
| v1.3 (P2WPKH) | v1.3 (P2WPKH) | Works as today |
| v1.3 (P2WPKH) | v1.4 (allowlist includes P2WPKH) | **Works.** v1.4 coordinator's `BipConfig.supported_script_types` defaults to all three including P2WPKH. The PKARR record gains `supported_script_types` but v1.3 client ignores unknown fields (no `deny_unknown_fields` per shared/src/protocol.rs line 2). The OwnershipProof wire format change matters: if we use a new `to_json_str` format with `script_type`, v1.3 clients send the old format. **Recommendation:** make `from_json_str` accept both the old array-only format (interpret as `script_type: P2WPKH`) and the new object format. Backwards-compatible deserialization is a one-time, surgical concession. |
| v1.4 (P2WPKH) | v1.3 (P2WPKH only) | **Works** if client treats missing `supported_script_types` in `/info` as `[P2WPKH]` and uses the old wire format when talking to a v1.3 coordinator. **Recommendation:** client always emits the new object format; v1.3 coordinator's `from_json_hex_str` would reject this as a JSON parse error. **Therefore:** client v1.4 MUST detect "old coordinator" (e.g. via `supported_script_types == None` in InfoResponse) and switch to legacy wire format for the input registration. This is a client-side dual-write fallback, ugly but contained. |
| v1.4 (P2TR) | v1.3 (P2WPKH only) | **Rejected at discovery** (good outcome). Client sees no `supported_script_types` → interprets as `[P2WPKH]` → refuses to register with hard error: `"coordinator does not support P2TR ownership proofs (legacy v1.3 coordinator)"`. |
| v1.4 (P2TR) | v1.4 (allowlist includes P2TR) | **Works as designed.** |
| v1.4 (P2TR) | v1.4 (allowlist = [P2WPKH] only, operator-pinned) | **Rejected at discovery** — exact same path as v1.3 coordinator case. Critical to test. |

**Single-sentence backwards-compat story:** v1.3 wire format is treated as a legacy-P2WPKH dialect of v1.4; clients and coordinators auto-detect peer version from the presence/absence of `supported_script_types` and switch formats accordingly. The dual-format burden lives at the wire-deserialization seam in `shared::protocol::OwnershipProof::from_json*` and at the pre-registration discovery check in `client::discover`. Both are <30 LOC each.

---

## 9. Patterns Preserved (from v1.0)

These patterns from v1.0's ARCHITECTURE.md are unchanged by v1.4:

- **Phase-Gated HTTP API** — still applies; new gate is `BipConfig.supported_script_types` allowlist, layered after the phase gate.
- **Alice/Bob Identity Separation** — fully unchanged.
- **Per-Round RSA Keypair** — unchanged.
- **Memory-Only Round State** — unchanged. New `script_type` field on `RegisteredInput` is non-sensitive and `#[zeroize(skip)]`-marked.
- **Tokio-Based Phase Timer** — unchanged.

## 10. Anti-Patterns Newly Introduced — and Avoided

### Anti-Pattern Avoided: Per-script-type ban lists

**What might be tempting:** Ban-list partition by script type so "P2TR bans don't affect P2WPKH participants".
**Why it's wrong:** Leaks correlation between rounds. Keep the ban list uniform — outpoints are outpoints.

### Anti-Pattern Avoided: Script-type leakage in error responses

**What might be tempting:** Return `"Invalid P2TR ownership proof: bad sighash"` to the client for diagnostics.
**Why it's wrong:** Leaks which script type the client is using to a passive observer (Tor traffic analysis). Use a generic `"Invalid ownership proof"` to the client and emit the detailed reason to the operator log only. The current `Bip322Error` enum already gives detailed reasons; the dispatcher must coarsen them before returning to the client.

### Anti-Pattern Avoided: Coordinator inferring script type instead of requiring client declaration

**What might be tempting:** Coordinator runs `detect_script_type(on_chain_spk)` and dispatches; no `script_type` field on the wire. Saves wire bytes.
**Why it's wrong:** Loses the cross-check (Pitfall 3) and makes the wire format script-type-blind which is exactly what we're trying to escape. Require explicit declaration; cross-check on the server.

---

## 11. Scalability Notes (v1.4-specific)

| Concern | Impact |
|---------|--------|
| Verification cost per script type | P2WPKH ECDSA: ~30µs. P2TR Schnorr: ~25µs (faster, batch-verifiable in future). P2SH-P2WPKH: ~30µs + redeem-script parse (~1µs). Negligible at v1.0 scale (3-50 participants). |
| Memory per RegisteredInput | +1 byte for ScriptType enum. Negligible. |
| PKARR record size | +40 bytes JSON. Still <220 byte warn threshold; <255 byte DNS limit. Documented. |
| BIP-322 unit-test suite runtime | 3x current count; runs in <2s today, projected <6s after v1.4. Acceptable. |

No scalability concern from v1.4. The coordinator stays single-instance, in-memory, in-process.

---

## 12. Sources

- **In-tree files read in full (HIGH confidence on integration points):**
  - [shared/src/bip322.rs](shared/src/bip322.rs) — current sign/verify primitives
  - [shared/src/protocol.rs](shared/src/protocol.rs) — wire types (OwnershipProof, InfoResponse, SignRequest)
  - [coordinator/src/bitcoin/utxo.rs](coordinator/src/bitcoin/utxo.rs) — current is_p2wpkh gate at line 119
  - [coordinator/src/round/state.rs](coordinator/src/round/state.rs) — RoundStateInner + RegisteredInput
  - [coordinator/src/round/input_reg.rs](coordinator/src/round/input_reg.rs) — input registration flow
  - [coordinator/src/round/signing.rs](coordinator/src/round/signing.rs) — PSBT assembly + broadcast, witness deserialization at line 163
  - [coordinator/src/round/blame.rs](coordinator/src/round/blame.rs) — detect_non_signers
  - [coordinator/src/config.rs](coordinator/src/config.rs) — CoordinatorConfig structure + validation
  - [coordinator/src/discovery/pkarr_pub.rs](coordinator/src/discovery/pkarr_pub.rs) — DHT publish payload
  - [client/src/wallet.rs](client/src/wallet.rs) — BdkClientWallet descriptor paths
  - [client/src/round/input.rs](client/src/round/input.rs) — current generate_bip322_witness
- **External references (LOW-MEDIUM confidence — Sprint 0 spike required):**
  - `bip322` crate on crates.io (0.0.x — version stability is the primary v1.4 risk)
  - bdk_wallet 2.3 `tr(...)` and `sh(wpkh(...))` descriptor support (well-documented; assumed to work)
- **PROJECT.md v1.4 milestone definition** — Active requirements and out-of-scope items above
- **v1.0 ARCHITECTURE.md (this file's predecessor)** — patterns/anti-patterns preserved unchanged
