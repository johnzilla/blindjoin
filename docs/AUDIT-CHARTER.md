# blindjoin External Audit Charter

| Field | Value |
|---|---|
| **Title** | blindjoin External Audit Charter: In-Scope Modules, Threat Models, Rejection Properties, Zeroization Window, and Residual Risks |
| **Authors** | John Turner (`<johnturner@gmail.com>`) |
| **Status** | v1.5 audit-readiness |
| **License** | MIT (this charter and the reference implementation) |
| **Created** | 2026-05 |
| **Implementation** | https://github.com/johnzilla/blindjoin |
| **Companion** | [docs/PROTOCOL.md](PROTOCOL.md) |

> **Status — v1.5 audit-readiness.** This charter scopes external review of
> the blindjoin coordinator + client codebase at the v1.5 ship. It enumerates
> in-scope modules with file:symbol references, threat models per module, the
> 9 cross-shape rejection properties locked by v1.4, the v=2 OwnershipProof
> PSBT handling boundary, the RSA secret key zeroization window (post AUDIT-03
> bounded form), out-of-scope dependencies, residual risks accepted with
> rationale, and a glossary mapping project terms to plain audit language.

---

## In-Scope Modules

This section enumerates every code surface an external auditor is asked to
review at the v1.5 ship. The durable anchor is the `file:symbol` form (per
the project's anchor-stability convention): symbols survive line-number
churn across reformats and minor patches, whereas a bare `file:NN` ref
bit-rots on the next edit. The orientation column is the approximate line
number at the v1.5 ship tag and is included only to help the reader scan
quickly — when in doubt, search the file by symbol name.

| File:Symbol | Description | Orientation (line at v1.5 ship) |
|---|---|---|
| `coordinator/src/blind/rsa.rs::RsaBlindSigner` | [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) RSA blind signer (per-round ephemeral). 6 public methods cached at round bootstrap: `generate`, `public_key_hash`, `blind_sign`, `from_der_secret_key`, `secret_key_der`, `public_key_spki_der`. | ~line 99 |
| `coordinator/src/blind/rsa.rs::RoundSecretKey` | Newtype wrapping `BjSecretKey` (`blind_rsa_signatures::SecretKey<Sha384, PSS, Randomized>`). AUDIT-03 lifetime-bound mitigation: makes the per-round key a value the FSM can null at one chokepoint. | ~line 33 |
| `coordinator/src/blind/rsa.rs::RoundSecretKey::drop` | Empty-crypto Drop body emitting one PII-safe `tracing::debug!` event under target `blindjoin::audit`. The transitive `<rsa::RsaPrivateKey as Drop>::drop` chain does the cryptographically meaningful work (see §5). | ~line 52 |
| `coordinator/src/round/state.rs::RoundStateInner` | Sensitive round material. Manual `Drop` zeroes `Vec<u8>` / `[u8; 32]` fields and iter-mut-zeroizes HashMap/HashSet values before clearing (HashMap does not implement `Zeroize`). | ~line 92 |
| `coordinator/src/round/state.rs::transition_to` | Single FSM chokepoint. On `next == Phase::Idle` sets `self.inner = None`, triggering the full Drop chain (see §5). The SOLE site setting `inner = None` (verified by grep of `coordinator/src/`). | ~line 186 |
| `coordinator/src/bitcoin/utxo.rs::validate_utxo` | V1.4-CRIT-01 cross-check. Derives `ScriptType` from on-chain `script_pubkey` via `detect_script_type(...)`; the client-declared `ParticipantInput.script_type` field is verified-against, never trusted. | ~line 67 |
| `coordinator/src/bitcoin/utxo.rs::dispatch_ownership_proof` | v=1 / v=2 `OwnershipProof` dispatcher. Routes the v=2 envelope to `decode_psbt_input_witness` and the v=1 legacy form to direct witness extraction. | ~line 158 |
| `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness` | v=2 OwnershipProof verification boundary. Decodes the full-BIP-174 PSBT envelope from `OwnershipProof.psbt_input_b64`, extracts the witness, hands off to `shared::bip322::verify_simple` via the dispatcher (see §4). | ~line 218 |
| `client/src/round/input.rs::build_v2_psbt_input_b64` | v=2 OwnershipProof construction boundary. Produces the full-BIP-174 PSBT envelope the coordinator decodes at `decode_psbt_input_witness`. Locked at v1.4 ADR Decision #3 (full PSBT, not bare `psbt::Input`). | ~line 35 |
| `shared/src/bip322/mod.rs::detect_script_type` | Script-type detection from `script_pubkey` bytes. Pattern-matches P2WPKH (OP_0 + 20-byte push), P2TR (OP_1 + 32-byte push), and P2SH-P2WPKH (OP_HASH160 + 20-byte push + OP_EQUAL). | ~line 238 |
| `shared/src/bip322/mod.rs::verify_simple` | BIP-322 Simple verify dispatcher. The ONLY public verify entry on `shared::bip322` (V1.4-CRIT-01 dispatcher-only invariant). | ~line 257 |
| `shared/src/bip322/mod.rs::sign_simple` | BIP-322 Simple sign dispatcher. The ONLY public sign entry on `shared::bip322` (V1.4-CRIT-01 dispatcher-only invariant; the Phase 19-02 close deleted the `#[doc(hidden)] sign_simple_test_only` escape hatch). | ~line 283 |
| `shared/src/bip322/p2wpkh::sign` / `p2tr::sign` / `p2sh_p2wpkh::sign` | Per-script production sign bodies. `pub(crate)`-only — callers outside the crate cannot reach them, by design. Shipped in Phase 19 (BIP322-05 / BIP322-06). | per-module |
| `coordinator/src/config.rs::BipConfig::validate` | `output_script_type` boot validation (D-37). Enforces that the operator-configured output type is in the allowed set before the coordinator accepts its first round. | ~line 249 |
| `shared/tests/bip322_cross_shape.rs` | 9 cross-shape rejection tests covering P2WPKH × P2TR × P2SH-P2WPKH × empty-witness combinations. Each test asserts the specific `Bip322Error` variant via `matches!()` so silent acceptance of the wrong rejection class is statically impossible (see §3). | full file |

The newtype `RoundSecretKey` and the structural `Option<RsaBlindSigner>` field
on `RoundStateInner` jointly express the bounded lifetime of the per-round
RSA secret key as a Rust type signature — the load-bearing AUDIT-03
mitigation per the REQUIREMENTS. The threat-model treatment of this
mitigation is in [§5 — RSA Secret Key Zeroization Window](#rsa-secret-key-zeroization-window).

---

## Threat Models per Module

This section enumerates the threat models the project has explicitly closed,
with citations to the code or tests that close them. Each subsection covers
one named threat from the REQUIREMENTS or the v1.4 retrospective.

### V1.4-CRIT-01 — script_type spoofing via client-declared field

**Threat:** A malicious client registers a UTXO whose on-chain `script_pubkey`
is P2WPKH while declaring `script_type = P2TR` on the wire. If the coordinator
trusts the client-declared field to route the BIP-322 verifier, it dispatches
to `p2tr::verify` against a P2WPKH witness — accepting an attacker-controlled
signature shape against a different key.

**Mitigation in code:** `coordinator/src/bitcoin/utxo.rs::validate_utxo`
(line ~67) derives `ScriptType` from the on-chain `script_pubkey` returned by
Bitcoin Core's `gettxout` RPC, then cross-checks against the client-declared
`ParticipantInput.script_type`. A mismatch returns
`Bip322Error::ScriptTypeMismatch` **before** the per-script verifier is
reached. The chain-derived value is the single derivation point that flows
through `RegisteredInput.script_type` to the fee path (Phase 20 FEE-02) and
to the signing path — CRIT-01 is preserved across every consumer.

**Mitigation in tests:** The 9 cross-shape rejection tests at
`shared/tests/bip322_cross_shape.rs` (see [§3](#cross-shape-rejection-properties))
enumerate every wrong SPK × witness combination across {P2WPKH, P2TR,
P2SH-P2WPKH} × {2-elem, 1-elem, empty} and assert the specific
`Bip322Error` variant via `matches!()`. The dispatcher-only public surface
on `shared::bip322` (D-27, hardened by the Phase 19-02 deletion of
`sign_simple_test_only`) makes the spoof impossible to reach from outside
the crate even if a future change introduces a script-type-trusting caller.

**Mitigation as defense-in-depth (Phase 19 D-111):** Each per-script
production sign body in `shared/src/bip322/{p2wpkh,p2tr,p2sh_p2wpkh}.rs`
includes a spk↔key cross-check at the top — if the wallet supplies a
keypair whose script-type derivation does not match the SPK being signed,
the sign body returns `Bip322Error::ScriptTypeMismatch` before producing a
witness. This catches the case where a benign client misconfiguration would
otherwise produce an unverifiable signature.

**Supply-chain mitigation:** The `bip322 = "=0.0.10"` exact pin is enforced
by the `bip322-pin-check` CI gate. The crate is pre-1.0; any minor release
can break the wire format. The pin + gate prevents a `cargo update` from
silently swapping in a behavior change.

**Behavioral assertions:** The 9 cross-shape rejection tests in
`shared/tests/bip322_cross_shape.rs` lock the V1.4-CRIT-01 spoofing-vector
closure at the `shared/` crate boundary. The `// CRIT-01: …` inline
markers at the v=1 and v=2 dispatcher arms in
`coordinator/src/bitcoin/utxo.rs` and at the v=2 envelope construction in
`client/src/round/input.rs` signal the invariant to human reviewers. (The
historical `crit-01-grep-check` / `crit-01-client-grep-check` CI gates
were removed in the v1.6 theater strip — they enforced the presence of a
comment, not the underlying behavior, and the behavioral cross-shape
tests are the real gate.)

### V1.4-CRIT-02 — silent sighash regression

**Threat:** A change to a per-script sign body (P2WPKH, P2TR, P2SH-P2WPKH)
silently alters the sighash computation in a way that still produces a
valid-looking signature but against a different message digest, breaking
wire-byte compatibility with v1.3 / v1.4 clients and verifiers.

**Mitigation in code:** v1.4 ADR Decision #3 locked the v=2 OwnershipProof
wire format byte-exactly. Phase 19 production sign bodies preserve the
v1.4 wire output. The production `sign_simple` dispatcher uses the same
sighash construction path as the verify path (`bip322_message_hash` +
`build_bip322_to_sign` from `shared/src/bip322/mod.rs`) — sighash drift
between sign and verify is structurally impossible.

**Mitigation in tests:** `client/tests/wallet_sign_roundtrip.rs` runs
byte-equality parity tests between the production `sign_simple` and
`bdk_wallet` 2.3's deterministic sign path for both P2TR (Schnorr keypath
via `sign_schnorr_no_aux_rand`) and P2SH-P2WPKH (ECDSA RFC 6979). Any
divergence in sighash, nonce derivation, or witness encoding surfaces as a
byte-mismatch assertion failure at CI time.

**Belt-and-suspenders:** The 9 cross-shape rejection tests (see [§3](#cross-shape-rejection-properties))
also catch sighash regressions indirectly — a future sign-body change that
produced a slightly-different witness shape would fail the `matches!()`
assertion on `Bip322Error::InvalidWitnessLength` before the sighash itself
is even checked.

### V1.4-MIN-02 — uniform-script fingerprint via output_script_type

**Threat:** A coordinator that accepts mixed-input script types but emits
uniform-output script types leaks a partial anonymity-set signal. An
on-chain observer sees, for example, a 3-participant round with 1 P2WPKH +
1 P2TR + 1 P2SH-P2WPKH input but 3 P2WPKH outputs — the round shape
narrows the anonymity set relative to single-script rounds.

**Mitigation as documented tradeoff:** v1.4 ADR Decision #2 explicitly
accepts this tradeoff. The privacy improvement of larger anonymity sets
(from accepting heterogeneous inputs) is judged to dominate the partial
fingerprint cost. The tradeoff is documented in `README.md` §Privacy
Considerations and re-stated in [§7 — Residual Risks Accepted](#residual-risks-accepted)
as an accepted operational residual.

**Operational mitigation:** Phase 18 INTEG-02 introduced liquidity-bot
rotation: an operator can configure a fraction of rounds to be served by
bots whose output choice rotates across the supported set, diluting the
uniform-output signal. Operator-set bot ratios CANNOT eliminate the
fingerprint, only dilute it; the dilution rate is an operational policy.

**Orthogonality with Phase 20 fee accuracy:** v1.5 Phase 20's per-script
weight table (FEE-01 / FEE-02 / FEE-03) corrects the coordinator's fee
math so a mixed-input round pays the correct on-chain fee regardless of
output script type. Fee correctness is independent of the output-script
fingerprint; the two concerns share only the single chain-derived
`ScriptType` source at `validate_utxo`.

### RSA Marvin Attack (RUSTSEC-2023-0071) — residual exposure

**Threat model preconditions:** The Marvin Attack on the `rsa` crate's
RSASSA-PKCS1-v1_5 decryption is a timing sidechannel that requires
(i) a long-lived private key, and (ii) the ability to submit a large
number of chosen ciphertexts and measure decryption timing precisely
across them. The attack recovers the private exponent through statistical
analysis of timing variance.

**Mitigation: per-round ephemeral keys (D-02).** The coordinator generates a
fresh RSA-2048 keypair at the start of each round via
`RsaBlindSigner::generate()`. The key never persists past round end. An
attacker cannot accumulate timing measurements across rounds because each
round uses a different key.

**Mitigation: bounded chosen-ciphertext count per round.** The default
`max_participants = 20` cap (operator-configurable in `coordinator.toml`)
bounds the number of blind-sign operations against a single per-round key
to a small constant. The Marvin Attack's "unlimited measurements"
precondition does not obtain.

**Mitigation: AUDIT-03 RoundSecretKey + bounded lifetime.** The newtype
`RoundSecretKey(BjSecretKey)` at `coordinator/src/blind/rsa.rs::RoundSecretKey`
wraps the per-round `blind_rsa_signatures::SecretKey`. The
`RoundStateInner.rsa_signer: Option<RsaBlindSigner>` field on
`coordinator/src/round/state.rs::RoundStateInner` is the lifetime bound
expressible as a Rust type signature. The SOLE FSM trigger that nulls this
Option is `RoundState::transition_to(Phase::Idle)` (declared at
`coordinator/src/round/state.rs:193`; the `self.inner = None` chokepoint is
at `coordinator/src/round/state.rs:202` inside the validated-transition
block at lines 201-207). When that trigger fires, the
Drop chain runs all the way through to `<rsa::RsaPrivateKey as Drop>::drop`
at `rsa-0.9.10/src/key.rs:76-82`, which calls `.zeroize()` on `d`,
`primes`, and `precomputed`. The full chain is documented in
[§5 — RSA Secret Key Zeroization Window](#rsa-secret-key-zeroization-window).

**Verification:** The structural FSM test
`coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end`
drives a Signing → Broadcast → Idle transition and asserts both that the
`rsa_signer` Option was `Some(_)` before the transition (so the Drop chain
had a non-None target) and that `state.inner.is_none()` after (so the
Drop chain fired). This is the load-bearing AUDIT-03 assertion per the
REQUIREMENTS.

**Conclusion:** The Marvin Attack's preconditions (long-lived key + unlimited
measurements) do not obtain in this codebase. Constant-time RSA decryption
is delegated to the upstream `rsa` crate's potential future rewrite, which
[§6](#out-of-scope-components) lists as out-of-scope (consensus-critical
primitive with separate upstream audit posture). The disposition is
documented in [§7 — Residual Risks: cargo-audit Advisories](#residual-risks-cargo-audit-advisories).

---

## Cross-shape Rejection Properties

These 9 tests live at `shared/tests/bip322_cross_shape.rs`. They were
locked at Phase 15 as part of the 10-variant `Bip322Error` taxonomy
(D-31). Each test asserts the SPECIFIC `Bip322Error` variant via the
`matches!()` macro, so silent acceptance of the wrong rejection class is
statically impossible — a future regression that returned, e.g.,
`Bip322Error::CrateVerifyFailed` instead of `Bip322Error::InvalidWitnessLength`
would fail the test even though both outcomes are "rejection". The 9 tests
together exhaustively cover the {P2WPKH SPK, P2TR SPK, P2SH-P2WPKH SPK}
× {P2WPKH witness, P2TR witness, P2SH-P2WPKH witness, empty witness}
mismatch matrix.

| # | Test Function | What It Rejects | Bip322Error Variant |
|---|---|---|---|
| 1 | `reject_p2wpkh_spk_with_p2tr_witness` | P2WPKH SPK paired with a 1-element (P2TR-shaped) witness | `InvalidWitnessLength { expected: 2, got: 1 }` |
| 2 | `reject_p2wpkh_spk_with_p2sh_p2wpkh_witness` | P2WPKH SPK paired with a 2-element witness whose pubkey does not hash to the SPK | `CrateVerifyFailed` (ECDSA verify fails inside the bip322 crate) |
| 3 | `reject_p2tr_spk_with_p2wpkh_witness` | P2TR SPK paired with a 2-element (P2WPKH-shaped) witness | `InvalidWitnessLength { expected: 1, got: 2 }` |
| 4 | `reject_p2tr_spk_with_p2sh_p2wpkh_witness` | P2TR SPK paired with a 2-element witness | `InvalidWitnessLength { expected: 1, got: 2 }` |
| 5 | `reject_p2sh_p2wpkh_spk_with_p2wpkh_witness` | P2SH-P2WPKH SPK paired with a 2-element witness whose redeem-script HASH160 does not match the SPK | `CrateVerifyFailed` (HASH160 mismatch inside the bip322 crate) |
| 6 | `reject_p2sh_p2wpkh_spk_with_p2tr_witness` | P2SH-P2WPKH SPK paired with a 1-element (P2TR-shaped) witness | `InvalidWitnessLength { expected: 2, got: 1 }` |
| 7 | `reject_p2wpkh_spk_with_empty_witness` | P2WPKH SPK paired with a zero-element witness | `InvalidWitnessLength { expected: 2, got: 0 }` |
| 8 | `reject_p2tr_spk_with_empty_witness` | P2TR SPK paired with a zero-element witness | `InvalidWitnessLength { expected: 1, got: 0 }` |
| 9 | `reject_p2sh_p2wpkh_spk_with_empty_witness` | P2SH-P2WPKH SPK paired with a zero-element witness | `InvalidWitnessLength { expected: 2, got: 0 }` |

The 9 tests fail closed on any drift in the witness-length expectations
the per-script `verify` paths enforce. The dispatcher-only public surface
on `shared::bip322` (per [§1](#in-scope-modules)) means an external caller
cannot reach `p2wpkh::verify` directly to bypass dispatch — the
script-type derivation at `validate_utxo` always runs first, the
dispatcher always routes by chain-derived type, and the per-script
verifier always sees the (SPK, witness) pair that matches its declared
shape.

---

## v=2 OwnershipProof PSBT Handling

The v=2 OwnershipProof wire format is locked at v1.4 (ADR Decision #3).
The `OwnershipProof.psbt_input_b64` field is encoded as a **full BIP-174
PSBT** (a single-input single-output template), NOT as a bare
`psbt::Input` object. This is RESEARCH Pitfall 1 from Phase 15 — an
auditor reading the field name would naturally assume "PSBT input means
`psbt::Input`", and v1.4 explicitly chose the full-envelope shape for
extensibility and rust-bitcoin parser compatibility.

**Construction boundary:** `client/src/round/input.rs::build_v2_psbt_input_b64`
(line ~35) is the SINGLE construction site. The client builds the
1-input PSBT template, fills the witness from the wallet's sign path,
and base64-encodes the serialized PSBT. The function is the only place a
v=2 OwnershipProof is constructed in the codebase; refactors to the v=2
wire shape touch exactly this one site.

**Verification boundary:** `coordinator/src/bitcoin/utxo.rs::decode_psbt_input_witness`
(line ~218) is the SINGLE verification site. The coordinator
base64-decodes the envelope, parses it via rust-bitcoin's PSBT decoder,
extracts the witness from the single input, and hands the (SPK, witness)
pair to `shared::bip322::verify_simple` via the dispatcher at
`dispatch_ownership_proof`. The coordinator never trusts the PSBT's
internal SPK field — the SPK passed to the dispatcher is the one
`validate_utxo` derived from Bitcoin Core's `gettxout`.

**V1.4-CRIT-01 cross-check fires BEFORE verify.** The dispatcher path is:
`dispatch_ownership_proof` → `validate_utxo` (chain-derived SPK + script
type) → script-type cross-check against client declaration → mismatch
returns `Bip322Error::ScriptTypeMismatch` and the verifier never runs;
match dispatches to `decode_psbt_input_witness` → `verify_simple`. This
ordering is the structural mitigation for the spoofing threat described
in [§2 — V1.4-CRIT-01](#threat-models-per-module).

**Wire-byte lock (v1.4 ADR #3 → Phase 19 production sign).** The v=2
wire format was locked at v1.4 with the test-only sign bodies emitting
exact wire bytes; the Phase 19 production sign bodies preserve byte
equality with the v1.4 wire output, verified via parity tests at
`client/tests/wallet_sign_roundtrip.rs`. A future change to a per-script
sign body that broke wire compatibility would fail the parity test
before any further regression could land.

**Phase 20 fee path shares the same derivation point.** Phase 20
(FEE-01 / FEE-02 / FEE-03) introduced per-script weight accuracy via
`script_input_vbytes(ScriptType)` and `script_output_vbytes(ScriptType)`
in `coordinator/src/bitcoin/tx.rs`. Critically, the `ScriptType` value
the fee path consumes is the SAME chain-derived value that
`validate_utxo` produces — CRIT-01 is preserved into the fee path with
zero new `detect_script_type` call sites. The v=2 OwnershipProof
verification and the per-script fee calculation are two consumers of one
derivation point.

---

## RSA Secret Key Zeroization Window

The per-round RSA private key lives in the coordinator's memory for the
duration of an active round and is structurally dropped (and
cryptographically zeroized) at the end of the round. This section is the
threat-model treatment of that window in its bounded form, post-AUDIT-03.

**The lifetime claim.** `RoundSecretKey` (at
`coordinator/src/blind/rsa.rs::RoundSecretKey`) wraps `BjSecretKey` —
itself `blind_rsa_signatures::SecretKey<Sha384, PSS, Randomized>`. The
field `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` (at
`coordinator/src/round/state.rs:110`) transitively owns the
`RoundSecretKey` and is the lifetime bound expressible as a Rust type
signature. An auditor reading the type signature can answer "when does
this secret die?" with a single grep: the secret dies when the Option is
set to `None`.

**The trigger.** `RoundState::transition_to(Phase::Idle)` (declared at
`coordinator/src/round/state.rs:193`) is the SOLE site that sets
`self.inner = None` — the assignment lives at `state.rs:202` inside the
validated-transition block 201-207. This is verified by grep of the entire
`coordinator/src/` tree — no other code path assigns `inner = None`. The
FSM has 4 valid edges to `Phase::Idle`: Broadcast → Idle (success path,
`coordinator/src/round/signing.rs:280`), Blame → Idle (signing timeout,
`coordinator/src/round/blame.rs:220`), Blame → Idle (missing output,
`coordinator/src/round/output_reg.rs:31`), and InputReg → Idle (quorum
fail, `coordinator/src/run.rs:195`). All 4 routes through
`transition_to(Phase::Idle)`; none bypass the chokepoint. The coordinator
holds a single `Arc<RwLock<RoundState>>` per process — there is no
`HashMap<RoundId, RoundState>` map, so no drop-on-map-removal pattern
exists to be analyzed separately.

**The cryptographic work.** The wrapped `BjSecretKey` holds
`inner: rsa::RsaPrivateKey` (verified at the installed registry source
`blind-rsa-signatures-0.17.1/src/lib.rs:825-828`). The `rsa = 0.9.10`
crate has an UNCONDITIONAL `impl Drop for RsaPrivateKey` at
`rsa-0.9.10/src/key.rs:76-82` that calls `.zeroize()` on `d` (the private
exponent), `primes` (the prime factors `p` and `q`), and `precomputed`
(the CRT-optimization values `dp`, `dq`, `qinv`, and `crt_values`). The
same file declares `impl ZeroizeOnDrop for RsaPrivateKey {}` at line 84.
Both impls compile without any feature flag — `zeroize` is a non-optional
dep of `rsa`. When the FSM trigger fires, the full Drop chain runs:

```text
RoundState::transition_to(Phase::Idle)             // state.rs:193 (decl); body at 194-228
  self.inner = None                                // state.rs:202 (chokepoint)
    drop(Option<RoundStateInner>)
      drop(RoundStateInner)                        // state.rs::Drop, lines 127-156
        // (zeroizes rsa_signing_key, round_secret, registered_inputs, ...)
        drop(Option<RsaBlindSigner>)
          drop(RsaBlindSigner)
            drop(RoundSecretKey)                   // rsa.rs::Drop, lines 52-69
              // (PII-safe tracing::debug! event; no in-place crypto)
              drop(BjSecretKey)
                drop(rsa::RsaPrivateKey)           // rsa-0.9.10/src/key.rs:76-82
                  self.d.zeroize()
                  self.primes.zeroize()
                  self.precomputed.zeroize()
```

**The newtype's value: lifetime expression, not redundant scrub.** The
`RoundSecretKey::drop` body emits one PII-safe `tracing::debug!` event
under target `blindjoin::audit` and otherwise does nothing. The
cryptographically meaningful work is delegated to the upstream
`rsa::RsaPrivateKey` Drop chain. The newtype's audit value is in making
the per-round key a value the FSM nulls at one chokepoint — converting
ambient ownership into an `Option<RsaBlindSigner>` type signature an
auditor can read at `state.rs:110`. The upstream `rsa` crate is listed
as out-of-scope per [§6](#out-of-scope-components) (consensus-critical
primitive with separate upstream audit posture).

**Verification.** Two tests together close the loop:

- **Structural** —
  `coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end`
  is the load-bearing CI gate per the REQUIREMENTS ("the structural
  lifetime bound is the load-bearing claim"). It constructs a
  `RoundStateInner` with `Some(RsaBlindSigner::generate().unwrap())`,
  asserts the Option is `Some` pre-transition (so the Drop chain has a
  non-None target), drives Signing → Broadcast → Idle, and asserts
  `state.inner.is_none()` post-transition. This test asserts on the FSM,
  not on memory contents — it cannot flake.

- **Best-effort scrub** —
  `coordinator/src/blind/rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop`
  is a sanity ceremony. It captures a 32-byte distinctive middle slice of
  the DER-encoded secret key, drops the signer, allocates an 8 MB probe
  buffer to occupy adjacent allocator pages, and sweeps for the captured
  fingerprint. A miss is the success condition. The test is gated
  `#[cfg_attr(not(target_os = "linux"), ignore = ...)]` per CD-50 because
  heap layout determinism is a Linux/glibc property; on other platforms
  it reports `ignored` with a reason string naming the structural sibling
  as the unconditional gate.

The D-07 doc-comment at `coordinator/src/blind/rsa.rs:18-32` and
`:76-98` references this section via the markdown anchor
`docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window`. The `.cargo/audit.toml`
RUSTSEC-2023-0071 rationale paragraph cites the same anchor. Code,
audit.toml, and charter are mutually anchored — an auditor reading any
one of the three is one click from the other two.

---

## Out-of-Scope Components

"Out-of-scope" here means the audit charter does NOT request review of
the listed components for the v1.5 audit engagement. Their security
postures rely on upstream audit work or on operational properties cited
per-row. blindjoin's USAGE shapes of these components ARE in scope — see
[§1](#in-scope-modules).

| Component | Rationale (relies on…) |
|---|---|
| `arti-client` (Tor circuit isolation + hidden-service hosting) | Upstream Tor Project Arti 2.x audit posture. The blindjoin codebase consumes the public `arti-client` API only; no fork, no patch, no custom transport layer. Arti is the only viable in-process Tor implementation in Rust. |
| `pkarr` (Mainline DHT discovery layer) | Pubky project audit posture. blindjoin publishes a signed DNS-like packet via the public `pkarr` API; no fork, no custom DHT transport. The [PKARR](https://github.com/pubky/pkarr) record contents are the in-scope surface, not the DHT machinery. |
| `blind-rsa-signatures` (jedisct1) crate **internals** | [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) RSA blind signature primitive. The upstream crate is written by jedisct1 (libsodium author), production-grade, and the only [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)-compliant Rust implementation. OUR USAGE shape — the AUDIT-03 `RoundSecretKey` wrapper at [§5](#rsa-secret-key-zeroization-window) — IS in-scope. |
| `bip322 = "=0.0.10"` (rust-bitcoin org) crate **internals** | BIP-322 verify path. Exact-pinned (`=0.0.10`) and enforced by the `bip322-pin-check` CI gate; the crate is pre-1.0 and any minor release can break the wire format. OUR 26-LOC zero-lossy adapter at `shared/src/bip322/mod.rs::verify_via_bip322_crate` IS in-scope ([§1](#in-scope-modules)). |
| `rust-bitcoin` (consensus primitives + PSBT) and `secp256k1` (curve primitives) | Upstream rust-bitcoin team audit posture. These are the consensus-critical primitives the entire Rust Bitcoin ecosystem depends on. blindjoin uses the public API exclusively. |
| `bdk_wallet` (client-side descriptor wallet) | The coordinator never runs bdk_wallet. The CLIENT's USAGE shape — `client/src/wallet.rs::sign_bip322` — IS in-scope insofar as it constructs the v=2 OwnershipProof PSBT (see [§4](#v-2-ownershipproof-psbt-handling)). The crate's descriptor parser, UTXO selector, and key derivation are out-of-scope. |
| External penetration test execution | v1.5 *prepares for* an external audit by shipping the charter, audit.toml refresh, and bounded RoundSecretKey window. The engagement itself is a separate milestone scheduled after v1.5 ships. |

---

## Residual Risks Accepted

Residual risks are dispositions documented at the v1.5 ship. Each item has
either an **ACCEPTED** disposition with a written rationale, or a
**DOCUMENTED GAP** disposition with a planned closure window (typically
v1.6+). The three sub-buckets below separate (a) advisories surfaced by
`cargo audit`, (b) protocol-level residuals inherent to the design, and
(c) operational residuals that depend on operator policy.

### Residual Risks: cargo-audit Advisories

This sub-bucket mirrors the three entries in `.cargo/audit.toml`. The audit
configuration file's rationale comments are the operational record; the
prose below is the threat-model treatment.

- **RUSTSEC-2023-0071 — `rsa` crate Marvin Attack timing sidechannel.**
  Mitigated by AUDIT-03 `RoundSecretKey` + `Option<RsaBlindSigner>`
  bounded lifetime (see [§5](#rsa-secret-key-zeroization-window) for the
  full chain). The Marvin Attack model's preconditions (long-lived key +
  unlimited measurements) do not obtain in this codebase: per-round
  ephemeral keys + bounded blind-sign operations per round
  (`max_participants` cap, default 20) + structural lifetime bound +
  ephemeral coordinator process (Tor HS rotation) jointly close the
  attack window. **Disposition: ACCEPTED** with planned v1.6+
  contribution to upstream `rsa` for constant-time decryption.

- **RUSTSEC-2025-0141 — `bincode 2.0.1` unmaintained.**
  Transitive dependency reached via the DHT discovery layer; not directly
  used by blindjoin's coordinator or client. Not a runtime vulnerability
  (the advisory is "unmaintained crate", not a CVE). **Disposition:
  ACCEPTED**; will track upstream for a maintained replacement.

- **RUSTSEC-2024-0436 — `paste 1.0.15` proc-macro unmaintained.**
  Compile-time-only macro; no runtime code path. The macro expands at
  build time and produces inline code that does not reference the crate
  at runtime. **Disposition: ACCEPTED**; no remediation needed unless a
  runtime path appears in a future dep update.

### Residual Risks: Protocol-level

- **Heterogeneous-input chain-analysis tradeoff (v1.4 ADR Decision #2).**
  Accepting mixed input script types in a single round trades a small
  on-chain analysis signal for the privacy improvement of larger
  anonymity sets. Documented at `README.md` §Privacy Considerations and
  in v1.4 ADR Decision #2. **Disposition: ACCEPTED** as a design tradeoff;
  v1.6+ may revisit if external audit findings indicate the signal is
  larger than estimated.

- **V1.4-MIN-02 uniform-output-script fingerprint.** A coordinator with
  mixed-input + uniform-output rounds emits a partial anonymity-set
  signal. Partially mitigated by Phase 18 INTEG-02 liquidity-bot
  rotation; operator-set bot ratios can dilute but not eliminate the
  signal. **Disposition: ACCEPTED**; v1.6+ may add per-participant
  output script choice (Wasabi 2.0.3-style) which is currently
  out-of-scope per REQUIREMENTS.

- **TEST-EXT-01/02/03 differential-fixture gap.** No automated
  cross-implementation differential tests (e.g., vs `ACken2/bip322-js`);
  no regtest on-chain anchor test that broadcasts a real CoinJoin
  transaction against a regtest node and parses the result; no
  v1.3↔v1.4 backwards-compat matrix CI job. Manual cross-impl
  verification has been performed during development. **Disposition:
  DOCUMENTED GAP**; closure deferred to v1.6+ per REQUIREMENTS Future
  Requirements.

- **AUDIT-03 chokepoint result-discard pattern (`let _ =` on FSM
  transitions).** The 3 success-path FSM trigger sites that ultimately
  route through `transition_to(Phase::Idle)` use the pattern
  `let _ = state.transition_to(Phase::{Broadcast,Idle})` at
  `coordinator/src/round/signing.rs:279-280` (Broadcast → Idle on success),
  `coordinator/src/round/blame.rs:219-220` (Blame → Idle on signing
  timeout), and `coordinator/src/round/output_reg.rs:30-31` (Blame → Idle
  on missing output). The discarded `Result<(), TransitionError>` would
  signal a failed FSM edge, but in the current concurrency model the
  preceding transitions guarantee a valid edge: the round-state
  `Arc<RwLock<RoundState>>` is held for the duration of each handler,
  no other writer can interleave a phase change, and the preceding
  transition (e.g., `Signing → Broadcast` in signing.rs:271) has already
  established a phase from which `→ Idle` is a valid edge per
  `Phase::can_transition_to`. If a future refactor introduces concurrent
  writers, a different middle phase, or a stricter FSM validator, a
  failed transition would silently leave `RoundStateInner.inner` (and
  hence `Option<RsaBlindSigner>`) live in memory until the next
  successful `→ Idle` transition, violating the AUDIT-03 bounded-window
  claim's spirit. **Disposition: ACCEPTED** as defense-in-depth gap
  with the explicit invariant that **any change to the FSM concurrency
  model or the set of `Phase::can_transition_to` edges MUST audit these
  3 sites first**; closure (replacing `let _ =` with explicit
  `.expect()` or `.unwrap_or_else(|e| { /* fallback drop */ })`)
  deferred to v1.6+. Surfaced by the v1.5 internal code review (Phase
  21 REVIEW.md CR-01).

### Residual Risks: Operational

- **Single-coordinator-per-round trust model.** DHT discovery makes
  coordinators *replaceable* (a participant can re-discover and switch
  if a coordinator misbehaves), but the protocol is NOT byzantine-fault-
  tolerant within a single round — a malicious coordinator can refuse to
  broadcast or selectively register inputs. **Disposition: ACCEPTED** as
  a design constraint (blindjoin is intentionally simple and disposable;
  see PROJECT.md "infrastructure, not a product").

- **Sybil dilution depends on operator-set `min_participants` cap.** The
  coordinator has no cryptographic sybil resistance beyond the BIP-322
  proof-of-UTXO-ownership. A well-funded attacker can populate a round
  with their own sybil inputs to reduce the anonymity set. Mitigation
  via operator policy: setting a higher `min_participants` floor
  increases the sybil cost. **Disposition: ACCEPTED**; mitigation via
  operator policy.

- **[PKARR](https://github.com/pubky/pkarr) replay window.** Pubky DHT records are versioned with a
  monotonic sequence number, but a stale record may resolve momentarily
  before the latest version propagates to all DHT nodes. A participant
  reaching a stale record may attempt to contact a former coordinator
  address. **Disposition: ACCEPTED**; relies on [PKARR](https://github.com/pubky/pkarr) protocol design
  (Pubky upstream) and the operator practice of leaving old `.onion`
  addresses online for a short grace period after rotation.

- **B-03 dynamic fee estimation.** blindjoin v1.5 uses a static
  `fee_rate` config (the operator sets a sat/vB rate at startup). No
  mempool-aware polling, no RBF-fee-bumping fallback. **Disposition:
  ACCEPTED for v1.5** (signet-only ship; static fee is fine on signet);
  **REQUIRED before mainnet flip** (deferred to v1.6+ per REQUIREMENTS
  Future Requirements).

---

## Glossary

Scope: active v1.4 / v1.5 identifiers that appear in the in-scope code,
threat models, or tests. ~30 entries. Retired pre-v1.4 identifiers
(e.g., the v1.3 REPAIR-01 forensics tags, the original v1.0 PRIV-*
numbering, the Phase 8 STREAM-* IDs) live in the
`.planning/milestones/v1.0-1.3-*` archives and are not cited in v1.5
code.

| Project Term | Plain Audit Language |
|---|---|
| V1.4-CRIT-01 | Coordinator-side script_type spoofing vector closed by chain-derived script_type at `validate_utxo` |
| V1.4-CRIT-02 | Silent sighash regression class closed by byte-equality parity tests at `client/tests/wallet_sign_roundtrip.rs` |
| V1.4-MIN-02 | Uniform-output-script fingerprint — partial anonymity-set leak accepted as design tradeoff |
| V1.4-MOD-03 | Mixed-script E2E acceptance gate — `mixed_script_e2e_three_clients_broadcast` test |
| AVAIL-01 | Async RPC calls execute before the write lock (slow bitcoind cannot serialize participants) |
| AVAIL-02 | RSA keys parsed once per round (not per request); cached `rsa_signer` field on `RoundStateInner` |
| CR-01 / CR-02 | Cryptographic-review checkpoints — v1.4 sprint-0 spikes covering blind-sig parameter choice + key derivation |
| WR-01 / WR-04 | Wire-format-regression checkpoints — v1.4 ADR Decision #3 byte-exact PSBT wire lock |
| D-07 | Manual `Drop for RoundStateInner` — memory zeroization of round-sensitive bytes |
| D-27 | Dispatcher-only public surface on `shared::bip322` — V1.4-CRIT-01 static-typing mitigation |
| D-31 | 10-variant `Bip322Error` taxonomy with PII safety |
| D-34 | 9 cross-shape rejection properties enumerated at `shared/tests/bip322_cross_shape.rs` |
| D-37 | `output_script_type` boot validation at `BipConfig::validate` |
| D-111 | spk↔key cross-check at the TOP of each per-script production sign body (Phase 19 defense-in-depth) |
| D-122 / D-124 | Phase 20 per-script vbyte table + `ScriptType` plumbing through `ParticipantInput` |
| D-128 | `RoundSecretKey` newtype inside `RsaBlindSigner` + `Option<RsaBlindSigner>` on `RoundStateInner` (Phase 21) |
| D-130 | `transition_to(Phase::Idle)` is the SOLE FSM trigger that nulls `RoundStateInner` and fires the Drop chain |
| ADR Decision #1 | RSA blind signatures ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) chosen over WabiSabi — no production Rust WabiSabi exists |
| ADR Decision #2 | Mixed-rounds privacy tradeoff — accept partial fingerprint signal for larger anonymity sets |
| ADR Decision #3 | v=2 OwnershipProof PSBT wire format — full BIP-174 PSBT envelope, not bare `psbt::Input` |
| ADR Decision #4 | `bdk_wallet` for client-side P2TR sign — descriptor-based, byte-equal Schnorr keypath |
| AUDIT-01 | This charter — `docs/AUDIT-CHARTER.md` (v1.5 Phase 21) |
| AUDIT-02 | `.cargo/audit.toml` charter-anchor refresh + RUSTSEC-2023-0071 rationale rewrite (v1.5 Phase 21) |
| AUDIT-03 | `RoundSecretKey` newtype + `Option<RsaBlindSigner>` bounded lifetime (v1.5 Phase 21) |
| PRIV-01 | Round-end memory zeroization gate — `transition_to_idle_clears_inner` test at `state.rs::tests` |
| CRIT-01 | Chain-derived `ScriptType` inheritance (v1.4) — the load-bearing CRIT-01 cross-check at `validate_utxo` |
| BLAME-02 | Missing-output blame edge — `OutputReg → Blame → Idle` FSM path |
| [RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html) | RSA Blind Signatures — IETF standard for the unlinkable signing primitive |
| BIP-174 | PSBT (Partially Signed Bitcoin Transaction) — the wire format used by the v=2 OwnershipProof envelope |
| BIP-322 | Generic Signed Message Format — the verification protocol used for UTXO ownership proofs |
| [PKARR](https://github.com/pubky/pkarr) | Public-Key Addressable Resource Records — Pubky's signed-DNS-packet discovery layer over Mainline DHT |

Retired pre-v1.4 identifiers (e.g., REPAIR-01 v1.3 forensics tags, v1.0
PRIV-* numbering, Phase 8 STREAM-* IDs) live in
`.planning/milestones/v1.0-1.3-*` archives.
