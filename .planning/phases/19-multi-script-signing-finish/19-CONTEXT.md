# Phase 19: Multi-Script Signing Finish - Context

**Gathered:** 2026-05-30
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 19 ships production sign bodies for the two `shared::bip322` per-script `pub(crate) fn sign` sites that are currently `todo!("Phase 17 WALLET-02 wires bdk_wallet sign per ADR #4")` — `shared/src/bip322/p2tr.rs:38-44` and `shared/src/bip322/p2sh_p2wpkh.rs:44-50` — and removes the v1.4 test-only escape hatches that were carried forward as a CD-6 concession:

1. **BIP322-05** — `shared::bip322::p2tr::sign` ships a production body. Output: 1-element `Witness` containing a 64-byte BIP-341 Schnorr SIGHASH_DEFAULT signature over the canonical BIP-322 `to_sign` sighash. Verifies via `bip322::verify_simple` (the existing `verify_via_bip322_crate` path) AND parity-equal to `BdkClientWallet::sign_bip322` byte-for-byte for the same `(key, message)`.

2. **BIP322-06** — `shared::bip322::p2sh_p2wpkh::sign` ships a production body. Output: 2-element `Witness` `[der_sig + SIGHASH_ALL, compressed_pubkey]`. Round-trip-verifies via `shared::bip322::verify_simple(ScriptType::P2shP2wpkh, ...)`. The companion `final_script_sig = OP_PUSHBYTES_22 OP_0 <20-byte HASH160(pubkey)>` is exposed via a NEW sibling helper `pub fn shared::bip322::p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf` (per D-110 below); the BIP-322 wire payload does NOT include scriptSig (verify_simple consumes only the witness), so `sign_simple`'s return type stays `Result<Witness, Bip322Error>` unchanged.

3. **BIP322-07** — Delete `#[doc(hidden)] pub fn sign_simple_test_only` from `shared/src/bip322/mod.rs:302-314` AND delete `pub(crate) fn sign_for_tests` from `p2tr.rs:60-95`, `p2sh_p2wpkh.rs:68-108`, and `p2wpkh.rs:88-95`. Migrate all `sign_simple_test_only` / `sign_for_tests` callers to the real dispatcher `sign_simple`: `shared/tests/per_script_vectors.rs:21,274,311`, `tests/integration/multi_script_validate.rs:23,114,120`, and refresh the comment references at `tests/integration/mod.rs:707,723`. Net effect: `shared::bip322` public surface shrinks to `{ScriptType, Bip322Error, detect_script_type, verify_simple, sign_simple, p2sh_p2wpkh_final_script_sig, bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign}` — V1.4-CRIT-01 dispatcher-only invariant becomes load-bearing at the type level with NO test-only hole.

**Requirements mapped to this phase** (per `.planning/REQUIREMENTS.md` §Traceability): BIP322-05, BIP322-06, BIP322-07.

**Not in scope:**
- Any changes to the `bip322 = "=0.0.10"` crate adapter at `shared/src/bip322/mod.rs:323-333` (verify path unchanged; sign path does NOT route through the crate — the crate adapter is verify-only by design per Phase 14/15).
- Any changes to v=2 `OwnershipProof` wire format (Phase 15 LOCKED — CD-7 two-phase try-parse byte-compat preserved).
- Any changes to `client::wallet::BdkClientWallet::sign_bip322` body (descriptor + WIF paths preserved verbatim; Phase 19 only adds a parity test that calls into it from `client/tests/wallet_sign_roundtrip.rs`).
- Any changes to `coordinator::bitcoin::utxo::validate_utxo` (CRIT-01 cross-check on the verify side — Phase 19 is sign-side only).
- Per-script weight table / fee math (Phase 20 work).
- RSA `BjSecretKey` zeroization newtype (Phase 21 work).
- AUDIT-CHARTER.md (Phase 21 work — but Phase 21's charter prose will reference the production sign bodies Phase 19 ships, so the audit-readiness storyline depends on Phase 19 closing cleanly).

**Boundary-only changes in this phase:**
- `shared/src/bip322/mod.rs` — delete `sign_simple_test_only` (lines 274-314); add `pub fn p2sh_p2wpkh_final_script_sig(pubkey) -> ScriptBuf` sibling; possibly extend the `Bip322Error` PII-safety test if the spk-vs-key cross-check exercises a previously-unreached arm of `ScriptTypeMismatch`.
- `shared/src/bip322/p2tr.rs` — body of `pub(crate) fn sign` replaces `todo!()` with the Schnorr keypath sign sequence (lifted near-verbatim from `sign_for_tests` lines 60-95); delete `sign_for_tests` helper; add spk-vs-key cross-check at the top of the new body.
- `shared/src/bip322/p2sh_p2wpkh.rs` — body of `pub(crate) fn sign` replaces `todo!()` with the BIP-143 sign sequence (lifted near-verbatim from `sign_for_tests` lines 68-108) but USING the passed `spk` rather than rebuilding it from key; delete `sign_for_tests` helper; add spk-vs-key cross-check at the top of the new body.
- `shared/src/bip322/p2wpkh.rs` — delete the (already-unused) `pub(crate) fn sign_for_tests` alias at lines 88-95; no production-body changes (P2WPKH sign has shipped production since Phase 15).
- `shared/tests/per_script_vectors.rs` — migrate the 3 `sign_simple_test_only` callers to `sign_simple`; update the explanatory comments at lines 271-272, 308-309 to reflect that production sign now ships.
- `tests/integration/multi_script_validate.rs` — migrate the 1 `sign_simple_test_only` callsite at lines 23, 114, 120 to `sign_simple`.
- `tests/integration/mod.rs` — refresh comments at lines 707, 723 referencing the removed `sign_simple_test_only`.
- `client/tests/wallet_sign_roundtrip.rs` — ADD the BIP322-05 parity test: `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` (constructs a P2TR descriptor wallet with a known key, calls `wallet.sign_bip322(msg)`, calls `shared::bip322::sign_simple(ScriptType::P2tr, &spk, &key, msg.as_bytes())`, asserts witness bytes byte-equal). If plan-phase research finds bdk_wallet 2.3 emits aux-rand Schnorr, downgrade to a verify-roundtrip assertion and log the discovery in 19-VERIFICATION.md.
- NO changes to: `tests/integration/full_round.rs` (v1.3 cross-phase invariant gate); `tests/integration/mixed_script_e2e.rs` (v1.4 cross-phase invariant gate); `coordinator/**`, `client/src/**`, `liquidity-bot/**` (Phase 19 is shared/-internal + 1 test addition in client/tests/).

**Cross-phase invariants (carry to every Phase 19 plan boundary):**
1. **v1.3 P2WPKH invariant:** `cargo test --test integration full_round` 8/8 green (~42s). Phase 19 makes NO changes to `full_round.rs` — its 8 tests should pass identically.
2. **v1.4 multi-script invariant:** `cargo test --test integration mixed_script_e2e` 1/1 green (acceptance gate). Phase 19 makes NO changes to `mixed_script_e2e.rs`.
3. **Shared crate invariant:** `cargo test -p shared` 31/31 + the 7 per-script-vector + 9 cross-shape rejection integration tests green at every plan boundary.

If either invariant goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phase 14 ADR + Phases 15/16/17/18 (NOT re-asked)

LOCKED upstream. Plan-phase consumes verbatim — no re-litigation.

- **Phase 14 ADR Decision #4 + Phase 15 CD-6:** `shared::bip322` per-script sign bodies do NOT depend on `bdk_wallet` (no bdk crate in `shared/`). Phase 19 production sign uses pure `bitcoin` + `secp256k1` primitives (the `bdk_wallet` sign path stays at the client layer in `client::wallet::BdkClientWallet::sign_bip322` for descriptor wallets).
- **Phase 15 D-27 (dispatcher-only public surface):** Per-script `verify` and `sign` stay `pub(crate)`; only `sign_simple` / `verify_simple` / `detect_script_type` / `bip322_message_hash` / `build_bip322_to_spend` / `build_bip322_to_sign` are `pub`. Phase 19 STRENGTHENS this by removing the `#[doc(hidden)] sign_simple_test_only` hole — V1.4-CRIT-01 dispatcher-only invariant is then load-bearing at the type level with no test-only mirror.
- **Phase 15 D-31 (10-variant Bip322Error taxonomy + PII safety):** Phase 19 reuses `ScriptTypeMismatch { declared, derived }` for the new spk↔key cross-check (D-109 below). The existing PII-safety test at `mod.rs:512-565` covers `ScriptTypeMismatch`; no new variant means no PII-safety test extension is required.
- **Phase 15 wire shape:** `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` flat struct + CD-7 byte-identity branch for v=1. Phase 19 changes are SIGN-side internal — they do NOT touch `OwnershipProof` construction in `client/src/round/input.rs`, which calls `BdkClientWallet::sign_bip322` (descriptor path), not `shared::bip322::sign_simple` directly for v=2 proofs.
- **Phase 15 `verify_via_bip322_crate` adapter:** Verify path is locked. Phase 19 sign path does NOT route through the bip322 crate (the crate has no exported sign API for our wire shape) — it builds sighashes via `bitcoin::sighash::SighashCache` and signs via `secp256k1` directly, matching the existing `sign_for_tests` shape.
- **Phase 15 `build_bip322_to_spend` / `build_bip322_to_sign` byte-shape:** Version(0), bare `OP_RETURN` (1 byte), exact-match with the crate's verify path. Phase 19 sign sites consume these helpers verbatim — no shape changes.
- **Phase 17 `BdkClientWallet::sign_bip322` body:** Descriptor + WIF paths preserved. Phase 19 ADDs a parity test in `client/tests/wallet_sign_roundtrip.rs` that calls into it; does NOT modify it.
- **v1.3 + v1.4 cross-phase invariants:** `full_round` 8/8 + `mixed_script_e2e` 1/1 stay green at every plan boundary. Phase 19 makes NO changes to either test file.

### A. P2SH-P2WPKH sign API surface

- **D-107:** **`sign_simple` return type stays `Result<Witness, Bip322Error>`.** No `Bip322ProofPieces` struct. ROADMAP SC#2's clause "sign returns a 2-element witness AND a `final_script_sig = OP_PUSHBYTES_22 OP_0 <HASH160(pubkey)>`" is interpreted at the WIRE LEVEL of the BIP-322 proof, not at the Rust signature of `sign_simple`. **Rationale:** `bip322::verify_simple` (the crate that powers `verify_via_bip322_crate`) consumes only `Witness`, not scriptSig. The OwnershipProof v=2 wire envelope carries `psbt_input_b64` containing finalised scriptSig where the bdk descriptor path needs it (client wallet covers this at `client/src/wallet.rs:589-592`); the witness-only `shared::bip322::sign_simple` signature is correct for both the round-trip verify AND the production WIF path that calls it. Re-shaping the return type to `(Witness, Option<ScriptBuf>)` would force a downstream cascade through `client::wallet::sign_bip322:505-511` and every test caller in `shared/tests/per_script_vectors.rs` / `tests/integration/multi_script_validate.rs` for ~zero gain — the scriptSig is derivable from the pubkey alone via D-110.
- **D-108:** **CR test addition:** The Phase 19 plan adds an inline `#[test] fn p2sh_p2wpkh_final_script_sig_derives_correctly()` in `shared/src/bip322/mod.rs::tests` that constructs the helper output for a known key and asserts byte-equality against the BIP-141 spec-derived `OP_PUSHBYTES_22 OP_0 <HASH160(pubkey)>` bytes. This pins the wire shape of the scriptSig clause of ROADMAP SC#2 at the unit-test layer.

### B. New `p2sh_p2wpkh_final_script_sig` helper

- **D-109:** **NEW `pub fn` in `shared::bip322`** (NOT in a per-script module, NOT a dispatcher-style multi-script variant). Lives in `shared/src/bip322/mod.rs`. **Signature:** `pub fn p2sh_p2wpkh_final_script_sig(pubkey: &bitcoin::secp256k1::PublicKey) -> ScriptBuf`. **Rationale:** Sibling to `sign_simple` / `verify_simple` keeps the BIP-322 multi-script primitives in one place; doesn't widen the dispatcher surface (no `match script_type`); script-specific name signals it's a P2SH-P2WPKH-only helper, so the dispatcher contract stays intact. Takes `PublicKey`, NOT `SecretKey` — lowest-privilege input, no secret material crosses the function boundary, matches BIP-141 derivation (`redeem = OP_0 OP_PUSHBYTES_20 HASH160(pubkey)`, `final_script_sig = OP_PUSHBYTES_22 <redeem-script-bytes>`).
- **D-110:** **Body shape (informative, plan-phase confirms exact API choice from rust-bitcoin):**
  ```
  let compressed = bitcoin::PublicKey::new(*pubkey);
  let wpkh = compressed.wpubkey_hash().expect("compressed key");
  let redeem = bitcoin::ScriptBuf::new_p2wpkh(&wpkh);
  // final_script_sig = single OP_PUSHBYTES_22 push of the 22-byte redeem
  bitcoin::blockdata::script::Builder::new()
      .push_slice::<&bitcoin::script::PushBytesBuf>(&redeem.as_bytes().try_into().unwrap())
      .into_script()
  ```
  Plan-phase may use the more ergonomic `ScriptBuf::builder().push_slice(redeem.as_bytes()).into_script()` if `Builder::push_slice` accepts a generic `AsRef<PushBytes>` in rust-bitcoin 0.32.x. Either way the output bytes are `0x16 0x00 0x14 <20-byte-hash160>` (24 bytes total).

### C. Defense-in-depth: spk ↔ key cross-check inside `sign`

- **D-111:** **Both p2tr::sign AND p2sh_p2wpkh::sign cross-check that `spk` matches the supplied `key`** and return `Bip322Error::ScriptTypeMismatch { declared: <derived-from-spk>, derived: <derived-from-key> }` on a miss. **Rationale:** sign_for_tests today has a silent footgun on P2SH-P2WPKH (the helper ignores `_spk` and rebuilds it from `key`) and a softer one on P2TR (uses `spk` for to_spend without checking it matches the tap-tweaked key). Phase 21's audit charter wants to describe a structural mitigation, not "best-effort"; this cross-check IS that mitigation at the sign-side of the dispatcher. Cost: 1 hash + 1 byte compare per sign call (negligible).
- **D-112:** **Variant reuse:** `ScriptTypeMismatch { declared, derived }` is semantically a stretch (the mismatch is key↔spk, not declared-script-type↔derived-script-type), but reusing the existing variant means:
  - No new variant added to the 10-variant taxonomy → no PII-safety test extension required.
  - No new audit-charter line for a `KeyScriptMismatch` variant.
  - No churn at downstream callers' `match err { ... }` arms.
  Document the dual meaning inline at the variant's doc comment: "Reused for spk↔key derivation mismatch during sign — `declared` is the script type derived from the on-chain `script_pubkey` arg, `derived` is the script type derived from the supplied secret key's pubkey." Plan-phase confirms the variant doc-comment is updated.
- **D-113:** **Cross-check algorithm per script type:**
  - **P2TR:** Derive `keypair = Keypair::from_secret_key(secp, key)`; tap-tweak with empty merkle root; build the expected `ScriptBuf::new_p2tr_tweaked(tweaked.x_only_public_key().0.dangerous_assume_tweaked())`; compare to `spk` byte-equal. If `spk` is not even a P2TR SPK, the comparison fails (which is correct — `verify_simple` would reject downstream anyway, but we fail earlier with a better error variant).
  - **P2SH-P2WPKH:** Derive `compressed = PublicKey::new(key.public_key(secp))`; build `redeem = ScriptBuf::new_p2wpkh(&compressed.wpubkey_hash())`; build the expected `ScriptBuf::new_p2sh(&redeem.script_hash())`; compare to `spk` byte-equal.
  - Both: on mismatch, return `Err(Bip322Error::ScriptTypeMismatch { declared, derived })` where `declared` = `detect_script_type(spk)?` (or, if `spk` is unrecognisable as any single-key shape, return `Err(Bip322Error::UnrecognisedScriptPubkey { ... })`) and `derived` = the script type the key corresponds to.

### D. P2TR Schnorr nonce strategy

- **D-114:** **`secp.sign_schnorr_no_aux_rand(msg, tweaked)` — deterministic.** Required by BIP322-05 SC#1's byte-equality parity test (aux-rand would make `bdk_wallet::sign_bip322` and `shared::bip322::sign_simple` diverge on every call). Matches the existing `sign_for_tests:87-90` invocation. Matches BIP-340 §3.3 "signing without auxiliary randomness". Determinism is the right default for a CoinJoin participation flow (reproducible debug, no entropy budget consumed per round).
- **D-115:** **Plan-phase research task (Phase 19 RESEARCH.md):** Confirm bdk_wallet 2.3's BIP-322 sign path emits Schnorr signatures via `sign_schnorr_no_aux_rand` (NOT `sign_schnorr` with aux-rand). If the bdk source uses aux-rand, the BIP322-05 SC#1 byte-equality assertion in the parity test (D-118) MUST downgrade to "verify-roundtrip" (assert that both outputs verify under `verify_simple`, not that they are byte-equal). Document the discovery in 19-VERIFICATION.md.

### E. Production sign body promotion strategy

- **D-116:** **Lift `sign_for_tests` bodies near-verbatim into `sign`.** Both P2TR and P2SH-P2WPKH `sign_for_tests` already round-trip-verify against the bip322 crate (proven by the existing positive-vector tests at `shared/tests/per_script_vectors.rs:228-330`). The promotion is mechanical: rename `sign_for_tests` → `sign`, replace `todo!()` body, wrap the witness construction in `Ok(...)`, ADD the D-111 cross-check at the top, delete the now-unused `sign_for_tests` definition. The P2WPKH `sign_for_tests` at `p2wpkh.rs:88-95` is already a thin alias around production `sign` and is unused outside the deleted dispatcher mirror — delete it outright.
- **D-117:** **P2SH-P2WPKH `_spk` becomes `spk` (semantically meaningful).** The current sign_for_tests at `p2sh_p2wpkh.rs:68-108` rebuilds the P2SH SPK from `key` and uses that for `to_spend`. After D-111's cross-check, the passed `spk` byte-equals the derived one, so the production body uses `spk` directly for `to_spend.output[0].script_pubkey`. The sighash is still computed against the UNWRAPPED P2WPKH redeem derived from the pubkey (per BIP-143 — this is structural, not configurable). This makes the parameter list non-misleading: spk is now load-bearing.

### F. Parity test (BIP322-05 SC#1 closure)

- **D-118:** **Parity test lives at `client/tests/wallet_sign_roundtrip.rs`.** Test fn: `p2tr_shared_sign_matches_bdk_sign_byte_for_byte` (or similar). Body:
  1. Build a P2TR descriptor wallet with a known seed (existing pattern in the file).
  2. Call `wallet.sign_bip322(TEST_MESSAGE)` → captures `bdk_witness`.
  3. Extract the wallet's actual signing key (the `key.public_key()`-recoverable secret derived from the descriptor) — the test fixture seeds the wallet so the key is known statically.
  4. Call `shared::bip322::sign_simple(ScriptType::P2tr, &wallet.utxo_script_pubkey, &key, TEST_MESSAGE.as_bytes())` → captures `shared_witness`.
  5. `assert_eq!(bdk_witness, shared_witness)` (byte-equal). If D-115's research finds bdk uses aux-rand, downgrade to "both witnesses verify under `verify_simple`".
  **Rationale:** `client/tests/` already pulls bdk_wallet + shared + client; `shared/tests/` would need to add bdk_wallet as a dev-dep, violating Phase 15 CD-6 (no bdk in shared/). `tests/integration/` would add a bitcoind-skip surface for a test that doesn't need bitcoind. `client/tests/wallet_sign_roundtrip.rs` already has the descriptor-wallet construction boilerplate ready to reuse.
- **D-119:** **Analogous P2SH-P2WPKH parity test:** ROADMAP SC#2 doesn't explicitly require byte-equality (it requires roundtrip-verify), but for consistency add `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte` in the same file. The bdk descriptor path for P2SH-P2WPKH produces a DER-ECDSA signature which uses RFC 6979 deterministic nonce — byte-equality should hold without any aux-rand concern. Roundtrip-verify is the load-bearing assertion either way.

### G. Plan structure / sequencing

- **D-120:** **TWO plans:**
  - **19-01-PLAN.md = BIP322-05 + BIP322-06 + `p2sh_p2wpkh_final_script_sig` helper + parity tests.** Tasks: (1) p2tr::sign production body + cross-check; (2) p2sh_p2wpkh::sign production body + cross-check; (3) `p2sh_p2wpkh_final_script_sig` helper + unit test (D-108); (4) `client/tests/wallet_sign_roundtrip.rs` parity tests for P2TR + P2SH-P2WPKH (D-118, D-119). At this plan boundary `sign_for_tests` and `sign_simple_test_only` still exist — they're load-bearing for the existing positive-vector + cross-shape integration tests until Plan 19-02 migrates the callers. Tests on this plan boundary: `cargo test -p shared` 31/31 + 7 per-script-vector + 9 cross-shape green; `cargo test -p client --test wallet_sign_roundtrip` adds 2 new passing tests (4 existing + 2 new = 6 total); `cargo test --test integration full_round` 8/8; `cargo test --test integration mixed_script_e2e` 1/1.
  - **19-02-PLAN.md = BIP322-07 removal + caller migration.** Tasks: (1) delete `sign_simple_test_only` (mod.rs); (2) delete `sign_for_tests` (p2tr.rs + p2sh_p2wpkh.rs + p2wpkh.rs); (3) migrate `shared/tests/per_script_vectors.rs` callers at lines 21, 274, 311 to `sign_simple`; (4) migrate `tests/integration/multi_script_validate.rs` callers at lines 23, 114, 120 to `sign_simple`; (5) refresh `tests/integration/mod.rs:707,723` comments. At this plan boundary the public surface of `shared::bip322` has shrunk to its final v1.5 form. Tests: same set as Plan 19-01, all still green; the per-script-vector + cross-shape tests now exercise the production sign path (which is the load-bearing assertion of Plan 19-02).
  **Rationale:** Plan 19-01's tests prove production sign works against the SAME assertions as the test-only mirror. Plan 19-02 is then a pure removal against a known-green baseline — easy to forensically isolate if anything breaks. Atomic-commit boundary is clean per plan. Two commits, each verifiable against the full v1.3 + v1.4 invariant set.
- **D-121:** **Wave structure:** 19-01 (wave 1) → 19-02 (wave 2, depends on production sign body existing per Plan 19-01). No parallelism within the phase (the two plans are strictly sequential).

### Claude's Discretion

- **CD-34:** Plan-phase decides the EXACT helper signature for `p2sh_p2wpkh_final_script_sig` — whether to use `Builder::new().push_slice(...)` or `ScriptBuf::builder().push_slice(...)`, depending on which rust-bitcoin 0.32.x ergonomics the codebase already uses elsewhere (grep `bitcoin::blockdata::script::Builder` in coordinator + client to find the prevailing convention). The output bytes (`0x16 0x00 0x14 <hash160>`) are the load-bearing contract.
- **CD-35:** Plan-phase decides the exact name of the parity test functions (suggested: `p2tr_shared_sign_matches_bdk_sign_byte_for_byte`, `p2sh_p2wpkh_shared_sign_matches_bdk_sign_byte_for_byte`); the assertion shape (D-118, D-119) is the load-bearing contract. Plan-phase MAY consolidate both into a single parameterised test if the existing file's style supports it.
- **CD-36:** Plan-phase decides whether the new `Bip322Error::ScriptTypeMismatch` variant gets an inline doc-comment update reflecting the dual meaning (per D-112) OR whether a `// NOTE: also used for spk↔key derivation mismatch in p2tr::sign / p2sh_p2wpkh::sign per D-112` is added at the sign-site instead. Default: doc-comment update (more discoverable).
- **CD-37:** Plan-phase decides whether to add a `#[test] fn sign_rejects_p2tr_spk_with_p2sh_p2wpkh_key` (and analogous variants) to `shared/src/bip322/mod.rs::tests` to pin the D-111 cross-check behavior at the unit-test layer. Default: yes — small additions, exercise the new error path explicitly, helpful for the Phase 21 audit charter's "cross-shape rejection properties" section.
- **CD-38:** Plan-phase decides whether the BIP322-05 / BIP322-06 production bodies grow inline `// Plan 19-01 Task N` comments referencing this CONTEXT (mirrors Phase 17 D-65 / D-66 inline-comment convention) or whether the per-decision tracking lives in 19-01-PLAN.md only. Default: inline comments at the per-script `sign` body summary (the bdk sign route is a load-bearing reference for the audit charter).
- **CD-39:** Plan-phase decides whether Plan 19-02 includes the comment-only refreshes at `tests/integration/mod.rs:707,723` (per the boundary list above) OR whether those are folded into a docs-only follow-up commit. Default: folded into 19-02 — the comments reference the removed `sign_simple_test_only` and would point at dead code without the refresh.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing.**

### Phase 14 ADR + Phases 15/16/17 outputs (LOCKED inputs)

- `.planning/decisions/v1.4-adr.md` §`#decision-4` (Sign path = bdk_wallet for descriptor wallets, shared::bip322 for WIF path) — Phase 19 honors the split: production sign in `shared::bip322` does NOT depend on `bdk_wallet`; the bdk descriptor sign path stays at `client::wallet::BdkClientWallet::sign_bip322`.
- `.planning/milestones/v1.4-phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` §CD-6 — "P2TR + P2SH-P2WPKH sign bodies are `todo!()` in Phase 15; Phase 17 wires bdk_wallet sign path; the test-only mirror `sign_simple_test_only` + `sign_for_tests` helpers exist for Phase 15-03's per-script positive-vector tests." Phase 19 closes the CD-6 follow-through by shipping production bodies that do NOT depend on bdk_wallet (lifted from sign_for_tests) AND removes the test-only mirror.
- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-27 (dispatcher-only public surface) — Phase 19 STRENGTHENS by removing the `#[doc(hidden)]` hole.
- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-31 (10-variant `Bip322Error` taxonomy + PII safety) — Phase 19 reuses `ScriptTypeMismatch` for the new spk↔key cross-check (D-112); no new variant.
- `.planning/milestones/v1.4-phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-04 (per-script module split) — `p2wpkh.rs` / `p2tr.rs` / `p2sh_p2wpkh.rs` shape locked. Phase 19 modifies bodies, not module layout.
- `.planning/milestones/v1.4-phases/17-client-multi-script-wallet-discovery/17-CONTEXT.md` §D-65 (per-script sign dispatch via `wallet.sign_bip322`) — Phase 19's parity test (D-118) calls into this verbatim.
- `.planning/milestones/v1.4-phases/17-client-multi-script-wallet-discovery/17-CONTEXT.md` §D-61 (`from_wif` is P2WPKH-only) — Phase 19 doesn't change this; the WIF path's `sign_bip322:497-518` call to `shared::bip322::sign_simple(ScriptType::P2wpkh, ...)` is verified to keep working after Plan 19-02 removes the dispatcher mirror.

### Project-level anchors

- `.planning/PROJECT.md` §"Constraints" — No custom crypto (Phase 19 uses `bitcoin` + `secp256k1` primitives; no fork of `blind-rsa-signatures` or `bip322`); no PII logging (reuses existing PII-safe `Bip322Error` Display); MIT-licensed (existing crate dep graph unchanged).
- `.planning/PROJECT.md` §"Current Milestone: v1.5 Audit-Readiness & Multi-Script Finish" — Phase 19 is the first v1.5 phase; closes the externally-visible v1.4 sign-side follow-through; unblocks Phase 21's audit charter which describes production code, not `todo!()`.
- `.planning/REQUIREMENTS.md` §BIP322-05 (line 11) — Phase 19 Plan 19-01 closes verbatim.
- `.planning/REQUIREMENTS.md` §BIP322-06 (line 13) — Phase 19 Plan 19-01 closes verbatim.
- `.planning/REQUIREMENTS.md` §BIP322-07 (line 15) — Phase 19 Plan 19-02 closes verbatim.
- `.planning/REQUIREMENTS.md` §Traceability — BIP322-05/06/07 → Phase 19.
- `.planning/ROADMAP.md` §"Phase 19" — 5 success criteria. Phase 19 Plans 19-01 + 19-02 close all 5.
- `.planning/STATE.md` §"Accumulated Context (carried from v1.4)" — names the load-bearing v1.4 invariants Phase 19 preserves (V1.4-CRIT-01 dispatcher-only public surface + CRIT-01 cross-check + CD-7 two-phase try-parse + `bip322 = "=0.0.10"` pin). All 4 unchanged by Phase 19.
- `.planning/STATE.md` §"v1.5 design notes" line 1 — "Phase 19 sign bodies SHOULD reuse the existing `#[cfg(test)] sign_for_tests` implementations almost verbatim — those helpers are already correct (they produce the witnesses the existing tests verify against the bip322 crate); the change is mostly 'make them production, remove the test-only escape hatch.'" Phase 19 D-116 binds this verbatim.

### Specs / external references

- **BIP-322** (Generic Signed Message Format) — to_spend / to_sign tx shape, Section 4 (to_spend), Section 5 (to_sign); witness construction per script type. The existing `build_bip322_to_spend` / `build_bip322_to_sign` in `shared/src/bip322/mod.rs:45-138` are spec-compliant per the inline comments tracing the rust-bitcoin `bip322 = "=0.0.10"` crate's `util::create_to_sign` at `src/util.rs:62-69`. Phase 19 sign bodies consume these helpers verbatim.
- **BIP-340** (Schnorr Signatures for secp256k1) §3.3 — "signing without auxiliary randomness". Phase 19 D-114 binds this.
- **BIP-341** (Taproot: SegWit version 1 spending rules) §sign + §verify — keypath signing sequence: tap_tweak(internal_key, merkle_root=None) → BIP-341 sighash → Schnorr sign over `sighash` with tweaked keypair → 64-byte (SIGHASH_DEFAULT) witness. Phase 19 p2tr::sign body follows this verbatim (lifted from sign_for_tests).
- **BIP-143** (Transaction Signature Verification for Version 0 Witness Program) — BIP-143 sighash for P2WPKH/P2SH-P2WPKH. Phase 19 p2sh_p2wpkh::sign body uses BIP-143 sighash over the UNWRAPPED P2WPKH redeem (NOT the outer P2SH SPK), matching the bip322 crate's internal `verify_full_p2wpkh(is_p2sh=true)` at `verify.rs:167-169`.
- **BIP-141** (Segregated Witness, Consensus Layer) §"P2WPKH nested in BIP16 P2SH" — redeem script shape `OP_0 OP_PUSHBYTES_20 <HASH160(pubkey)>` (22 bytes), final_script_sig = `OP_PUSHBYTES_22 <redeem>` (24 bytes). Phase 19 D-110 / `p2sh_p2wpkh_final_script_sig` helper binds this.
- `bip322 = "=0.0.10"` crate documentation (https://docs.rs/bip322/0.0.10) — `verify_simple` API consumes only `(Address, message, Witness)`; no scriptSig in the verify-side interface. Binds D-107.

### Code anchors (Phase 19 reads OR modifies)

- `shared/src/bip322/mod.rs:209-272` (dispatcher `sign_simple` body) — Phase 19 does NOT modify the dispatcher body (the `match script_type` arms unchanged); Phase 19 adds the new `p2sh_p2wpkh_final_script_sig` helper SIBLING to it.
- `shared/src/bip322/mod.rs:274-314` (`sign_simple_test_only`) — Plan 19-02 DELETES this entire block (including the explanatory comment header at lines 274-300).
- `shared/src/bip322/p2tr.rs:18-31` (`pub(crate) fn verify`) — Phase 19 reads, does NOT modify.
- `shared/src/bip322/p2tr.rs:33-44` (`pub(crate) fn sign` with `todo!()`) — Plan 19-01 replaces the body with the production Schnorr keypath sign sequence (lifted near-verbatim from sign_for_tests at lines 60-95) + spk↔key cross-check at the top.
- `shared/src/bip322/p2tr.rs:46-95` (`pub(crate) fn sign_for_tests`) — Plan 19-02 DELETES this entire function.
- `shared/src/bip322/p2sh_p2wpkh.rs:24-37` (`pub(crate) fn verify`) — Phase 19 reads, does NOT modify.
- `shared/src/bip322/p2sh_p2wpkh.rs:39-50` (`pub(crate) fn sign` with `todo!()`) — Plan 19-01 replaces the body with the production BIP-143 sign sequence (lifted near-verbatim from sign_for_tests at lines 68-108) + spk↔key cross-check at the top + USES the passed `spk` for `to_spend` after the cross-check confirms it byte-equals the derived value.
- `shared/src/bip322/p2sh_p2wpkh.rs:52-108` (`pub(crate) fn sign_for_tests`) — Plan 19-02 DELETES this entire function.
- `shared/src/bip322/p2wpkh.rs:22-71` (`pub(crate) fn verify` + `pub(crate) fn sign` — production-shipped since Phase 15) — Phase 19 reads, does NOT modify.
- `shared/src/bip322/p2wpkh.rs:74-95` (`pub(crate) fn sign_for_tests`) — Plan 19-02 DELETES this unused alias.
- `shared/src/bip322/mod.rs:339-567` (existing test block) — Plan 19-01 ADDS `#[test] fn p2sh_p2wpkh_final_script_sig_derives_correctly` (per D-108) + optional negative-vector tests for the spk↔key cross-check per CD-37; Plan 19-02 does not touch this block.
- `shared/tests/per_script_vectors.rs:21` (`use shared::bip322::{sign_simple, sign_simple_test_only, ...}`) — Plan 19-02 removes `sign_simple_test_only` from the import.
- `shared/tests/per_script_vectors.rs:268-280` (P2TR positive-vector test) — Plan 19-02 migrates `sign_simple_test_only(ScriptType::P2tr, ...)` at line 274 to `sign_simple(ScriptType::P2tr, ...)`; refreshes the explanatory comment at lines 271-272.
- `shared/tests/per_script_vectors.rs:305-317` (P2SH-P2WPKH positive-vector test) — Plan 19-02 migrates `sign_simple_test_only(ScriptType::P2shP2wpkh, ...)` at line 311 to `sign_simple(ScriptType::P2shP2wpkh, ...)`; refreshes the explanatory comment at lines 308-309.
- `tests/integration/multi_script_validate.rs:23,114,120` — Plan 19-02 migrates the import + the 1 callsite to `sign_simple`.
- `tests/integration/mod.rs:707,723` — Plan 19-02 refreshes the 2 comment references to `sign_simple_test_only`.
- `client/src/wallet.rs:496-602` (`sign_bip322` body) — Phase 19 reads, does NOT modify. Phase 19 Plan 19-01 parity test calls into this verbatim.
- `client/tests/wallet_sign_roundtrip.rs` (existing file, ~180 LOC, 4 descriptor + 1 WIF tests) — Plan 19-01 ADDS the 2 parity tests per D-118 + D-119.

### Cross-phase invariant references

- `tests/integration/full_round.rs` (full file, 1597 LOC, v1.3 invariant gate) — Phase 19 makes NO changes. Run `cargo test --test integration full_round` after each Phase 19 plan; expect 8/8 green, ~42s.
- `tests/integration/mixed_script_e2e.rs` (v1.4 invariant gate) — Phase 19 makes NO changes. Run `cargo test --test integration mixed_script_e2e` after each Phase 19 plan; expect 1/1 green.
- `shared/tests/bip322_cross_shape.rs` (9 cross-shape rejection tests, Phase 15) — Phase 19 makes NO changes. Plan 19-02 verifies these 9 tests stay green after `sign_simple_test_only` removal (the tests use `verify_simple`, not the test-only mirror, so they should be unaffected).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`p2tr::sign_for_tests` body at `shared/src/bip322/p2tr.rs:60-95`** — The 8-step BIP-341 keypath sign sequence is already correct (build to_spend → build to_sign → Keypair::from_secret_key → tap_tweak → taproot_key_spend_signature_hash → sign_schnorr_no_aux_rand → push 64 bytes into Witness). Plan 19-01 lifts this near-verbatim into the production `sign` body. The body needs: (1) the D-111 spk↔key cross-check inserted at the top; (2) the infallible `Witness` return wrapped in `Ok(...)`; (3) the function visibility and signature changed from `pub(crate) fn sign_for_tests(spk, key, message) -> Witness` to `pub(crate) fn sign(spk, key, message) -> Result<Witness, super::Bip322Error>`.

- **`p2sh_p2wpkh::sign_for_tests` body at `shared/src/bip322/p2sh_p2wpkh.rs:68-108`** — The BIP-143 sign sequence is correct (derive unwrapped P2WPKH from compressed pubkey → sighash via `p2wpkh_signature_hash(0, &unwrapped_p2wpkh, Amount::ZERO, EcdsaSighashType::All)` → DER-encode + push SIGHASH_ALL byte → push pubkey). Plan 19-01 lifts this near-verbatim into the production `sign` body. The body needs: (1) the D-111 spk↔key cross-check inserted at the top; (2) the infallible `Witness` return wrapped in `Ok(...)`; (3) per D-117, the `to_spend` build uses the passed `spk` directly (now load-bearing after the cross-check) instead of re-deriving the P2SH SPK from the pubkey; (4) signature change from `pub(crate) fn sign_for_tests(_spk, key, message) -> Witness` to `pub(crate) fn sign(spk, key, message) -> Result<Witness, super::Bip322Error>` (`_spk` becomes `spk`).

- **`verify_via_bip322_crate` at `shared/src/bip322/mod.rs:323-333`** — The 26-LOC crate adapter is the verify-side reference. Phase 19's sign-side adds NO sibling adapter (the bip322 crate has no exported sign API for our wire shape); production sign builds sighashes via `SighashCache` and signs via `secp256k1` directly, matching the existing `sign_for_tests` shape.

- **`p2wpkh::sign` at `shared/src/bip322/p2wpkh.rs:46-72`** — Reference production sign body (shipped since Phase 15). Phase 19 p2tr::sign + p2sh_p2wpkh::sign bodies adopt the same shape: `let secp = Secp256k1::new();` → `let pubkey = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, key);` → cross-check (NEW for Phase 19) → build to_spend/to_sign → sighash → sign → assemble witness. The P2WPKH version doesn't need the cross-check because `p2wpkh::sign` doesn't have a sign_for_tests with a silent spk-rebuild footgun (P2WPKH's `sign` uses `spk` for sighash via `p2wpkh_signature_hash(0, spk, ...)` — a mismatched spk would silently produce a wrong-sighash signature that fails verify, which is functionally similar to the cross-check rejecting it earlier). Plan-phase MAY decide to add a P2WPKH cross-check for symmetry; default: leave it as is (P2WPKH already ships and is exercised by `full_round` 8/8 — not worth a code touch).

- **`bitcoin::sighash::SighashCache` + `p2wpkh_signature_hash` + `taproot_key_spend_signature_hash`** — rust-bitcoin 0.32.x primitives Phase 19 sign bodies use. Already imported in the per-script files. Versions pinned via `Cargo.lock`.

- **`bitcoin::secp256k1::Secp256k1::sign_schnorr_no_aux_rand` + `sign_ecdsa`** — secp256k1 primitives Phase 19 sign bodies use. The Schnorr call is `secp.sign_schnorr_no_aux_rand(&Message::from_digest(sighash.to_byte_array()), &tweaked)`; the ECDSA call is `secp.sign_ecdsa(&secp_msg, key).serialize_der()` + push SIGHASH_ALL byte (0x01).

### Established Patterns

- **PII-safe `Bip322Error::ScriptTypeMismatch` Display** — `{declared:?} does not match on-chain {derived:?}` interpolates only `ScriptType` enum values, no key/spk/address bytes. The PII-safety test at `mod.rs:512-565` covers this variant. D-112 reuses the variant unchanged.

- **`pub(crate)` per-script + `pub fn` dispatcher** — Phase 15 D-27. Phase 19 production sign bodies stay `pub(crate)`; only `sign_simple` is `pub`. The new `p2sh_p2wpkh_final_script_sig` helper is `pub` because it's a script-specific helper (not a sign variant) and callers (client::wallet, audit-charter prose, tests) need it.

- **Test fixture seed `[0x42_u8; 32]`** — `fixture_secret_key` at `mod.rs:441-443` and analogous in test files. Phase 19's Plan 19-01 unit tests + Plan 19-02-migrated integration tests reuse this fixture pattern.

- **`bip322_message_hash` + `build_bip322_to_spend` + `build_bip322_to_sign`** — Carried-over script-neutral primitives in `mod.rs:45-138`. Used by all 3 per-script signers (existing P2WPKH; lifted-from-sign_for_tests P2TR + P2SH-P2WPKH). No changes.

- **`#[cfg(test)] mod tests` block** in each module — Phase 19 Plan 19-01 adds tests to `mod.rs::tests` (per D-108 + CD-37); does NOT add a new test block to per-script modules (the per-script tests live in `shared/tests/per_script_vectors.rs` as integration tests).

### Integration Points

- **`client::wallet::BdkClientWallet::sign_bip322` (P2WPKH WIF path) at `client/src/wallet.rs:496-518`** — Calls `shared::bip322::sign_simple(ScriptType::P2wpkh, &self.utxo_script_pubkey, &sk, message.as_bytes())`. Plan 19-02 removes `sign_simple_test_only` from `shared::bip322`'s public surface — this `sign_simple` call is unaffected (the dispatcher itself stays unchanged).

- **`client::round::input::register_input` at `client/src/round/input.rs:103-...`** — Constructs the v=2 OwnershipProof by calling `wallet.sign_bip322(...)`. Phase 19 doesn't change this path; the parity test at D-118 exercises `wallet.sign_bip322(...)` from a fresh entry point in `client/tests/wallet_sign_roundtrip.rs`.

- **`shared/tests/per_script_vectors.rs:228` (P2WPKH positive-vector test)** — Already calls `sign_simple` (no test-only mirror). Phase 19 does NOT modify this test fn. Plan 19-02 migration touches only the P2TR + P2SH-P2WPKH callsites at lines 274 + 311.

</code_context>

<specifics>
## Specific Ideas

- **Lift sign_for_tests bodies near-verbatim into sign** — STATE.md §"v1.5 design notes" line 1 confirms this is the intended approach. The bodies are already correct (they produce witnesses the existing tests verify against the bip322 crate); the change is "make them production, remove the test-only escape hatch."

- **Defense-in-depth cross-check is a Phase 21 audit-charter prerequisite** — D-111 + D-112 set up Phase 21 to describe the cross-check as a STRUCTURAL mitigation (not "best-effort"). Phase 21 AUDIT-CHARTER.md §"Threat models per module" will cite the Phase 19 cross-check inline. Plan-phase confirms the cross-check is implemented at the top of EACH per-script `sign` body and has a unit-test pinning the rejection behavior (per CD-37 default = yes).

- **Determinism for parity test is load-bearing** — D-114 + D-115. The byte-equality assertion at D-118 is the strongest available SC#1 closure. If bdk uses aux-rand, downgrade gracefully — log the discovery so future v1.6+ work can address the divergence if needed.

- **API surface delta is minimal** — Phase 19 shrinks `shared::bip322` by 1 function (`sign_simple_test_only`) + 3 helpers (`sign_for_tests` x 3) and adds 1 helper (`p2sh_p2wpkh_final_script_sig`). Net: -3 public/crate symbols + 1 public symbol. Cleaner audit charter prose.

</specifics>

<deferred>
## Deferred Ideas

- **P2WPKH spk↔key cross-check for symmetry with P2TR + P2SH-P2WPKH (per Plan-phase optional, D-116):** Could add the same check to `p2wpkh::sign:46-72`. Default: leave as is — P2WPKH ships and is exercised by `full_round` 8/8; not worth a touch in v1.5. v1.6+ candidate if audit-charter review flags asymmetry.

- **Bip322Error `KeyScriptMismatch` variant (per D-112 alternative):** Cleaner semantics than reusing `ScriptTypeMismatch`. Adds 1 variant + 1 Display impl + 1 PII-safety test case. Deferred to v1.6+ if audit-charter review flags the dual-meaning reuse as a documentation smell.

- **Parameterised parity test (per CD-35 alternative):** Consolidating P2TR + P2SH-P2WPKH parity tests into a single `#[test_case]`-driven test fn instead of two separate fns. Deferred to v1.6+ — the existing `wallet_sign_roundtrip.rs` style uses per-script-type test fns; consistency wins over conciseness.

- **`p2sh_p2wpkh_final_script_sig` returning `Result<ScriptBuf, Bip322Error>` instead of infallible:** The helper's only failure mode is `wpubkey_hash()` returning None on an uncompressed pubkey — Phase 19 takes a `&bitcoin::secp256k1::PublicKey` which is always 33-byte compressed, so the helper is infallible. Plan-phase confirms `bitcoin::PublicKey::new(secp_pubkey).wpubkey_hash()` returns `Some(...)` for any 33-byte input. If wrong, helper grows a `Result` return. Default: infallible.

- **Shared::bip322 helper for `derive_p2tr_spk(key)` + `derive_p2sh_p2wpkh_spk(key)` (general-purpose derivation):** Phase 19 D-113 needs these as one-shot inline blocks in each per-script `sign`; they're not exposed as helpers. v1.6+ if other callers need the derivation independently of the cross-check.

</deferred>

---

*Phase: 19-multi-script-signing-finish*
*Context gathered: 2026-05-30*
