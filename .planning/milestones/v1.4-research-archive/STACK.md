# Technology Stack — v1.4 BIP-322 Multi-Script Support

**Project:** blindjoin — Rust CoinJoin coordinator + client
**Milestone:** v1.4 (subsequent — extending v1.0/1.1/1.2/1.3 baseline)
**Researched:** 2026-05-29
**Overall confidence:** MEDIUM-HIGH

> **Scope note:** This file covers ONLY stack changes needed for v1.4's multi-script-type BIP-322 additions. The full v1.0 baseline stack (tokio, axum, arti-client, bdk_wallet 2.3, blind-rsa-signatures, pkarr, sqlx, corepc-types, etc.) is preserved in [`.planning/PROJECT.md`](../../.planning/PROJECT.md) and is NOT re-evaluated here. Do not consult this file for v1.0 stack questions.

---

## TL;DR — The Headline Decision

**Recommendation: Extend the existing custom `shared/src/bip322.rs` to cover P2TR and P2SH-P2WPKH. Do NOT adopt the official `bip322` crate in v1.4.**

The official `bip322` crate is still pinned at **0.0.10** — the same major-zero/minor-zero version range that caused us to defer adoption in v1.0. There has been **no release in approximately nine months** (last published ~September 2025). The blocking concerns from v1.0 STACK.md ("This is a young crate (0.0.x) — pin the exact version and test carefully") are unchanged. Worse, the crate's API surface forces us into a strict `Address`-based verification contract that does not cleanly match our wire protocol, where the coordinator receives a `(script_pubkey, witness_stack, message)` tuple and must verify without round-tripping through `Address` reconstruction.

The custom implementation in [`shared/src/bip322.rs`](../../shared/src/bip322.rs) is already ~133 LOC and the to_spend/to_sign virtual transaction construction is script-type-agnostic. Adding P2TR + P2SH-P2WPKH is a ~150-200 LOC extension localized to the verification dispatcher and three new sighash paths — smaller than the integration surface of swapping to an external crate plus the property-test work to cover the swap.

**B-02's "adopt the official crate" suggestion (written 2026-05-25) should be revised in light of the crate's stalled release cadence.** The fallback path it identified ("if the version pin still concerns us, extend the custom impl") is now the primary path.

The remainder of this file justifies that recommendation and pins exact versions for the v1.4 work.

---

## 1. The `bip322` crate (rust-bitcoin/bip322) — Current State

### Facts (verified)

| Property | Value | Confidence | Source |
|----------|-------|------------|--------|
| Latest version on crates.io | **0.0.10** | HIGH | crates.io search 2026-05-29 |
| Release date of 0.0.10 | ~September 2025 (~9 months ago) | HIGH | crates.io listing |
| Releases since | **None** | HIGH | crates.io listing |
| MSRV | 1.63.0 | HIGH | crate metadata |
| License | CC0-1.0 | HIGH | crate metadata |
| Maintainers | raph + 3 contributors, co-owned by Andrew Poelstra | MEDIUM | crates.io metadata |
| Monthly downloads | ~7,619 | MEDIUM | crates.io listing |
| Reverse-deps on crates.io | 14 crates (6 direct) | MEDIUM | crates.io listing |
| Production user (signal) | `ord` 0.24.2 (Ordinals reference impl) depends on `bip322 ^0.0.10` | HIGH | crates.io reverse-deps |
| Supported script types | P2TR, P2WPKH, P2SH-P2WPKH (single-sig) | HIGH | crate README + docs.rs |
| Multi-sig / P2WSH support | **NOT supported** | HIGH | crate scope statement |
| Public API (verification) | `pub fn verify_simple(address: &Address, message: impl AsRef<[u8]>, signature: Witness) -> Result<(), Error>` | HIGH | docs.rs |
| Public API (signing) | `sign_simple(...)` returning `Witness`; also `sign_full`, `verify_full`, `_encoded` variants for base64 wire format | HIGH | docs.rs |

### Interpretation

**Positives:**
- It IS the official rust-bitcoin org crate (co-owned by Andrew Poelstra — substantive maintainership).
- It IS used in production by `ord` for Ordinals — the highest-profile BIP-322 consumer in the Rust ecosystem.
- The script-type coverage (P2TR, P2WPKH, P2SH-P2WPKH single-sig) is **exactly** the v1.4 target set.
- The `verify_simple(&Address, message, Witness)` shape is conceptually right.

**Negatives:**
- **Nine months since last release.** This is not a fresh stall (one release cycle missed), it is a sustained pause covering multiple bitcoin-crate 0.32.x patch releases. The crate either is "done enough" for ord's use case (positive interpretation) or has stalled (negative interpretation). Without a public maintenance signal we cannot disambiguate.
- **Still 0.0.x.** SemVer-wise this is `MAJOR=0, MINOR=0`. Any change can break the API. Cargo treats `bip322 = "0.0.10"` as exact-pin-equivalent (`^0.0.10` matches only `>=0.0.10, <0.0.11`). There is no SemVer compatibility commitment from upstream.
- **API mismatch with our wire format.** `verify_simple` requires a `bitcoin::Address`. Our `/round/register_input` endpoint receives `{outpoint, witness_stack, message}` — the coordinator looks up the on-chain `scriptPubKey` from bitcoind's `gettxout` and verifies against the script bytes, not against an address string. To use `verify_simple` we would need to reconstruct an `Address` from `(scriptPubKey, network)`. This is doable but: (a) adds a `Network` parameter to a function that today operates purely on bytes, (b) requires the address to be canonically representable (rust-bitcoin's `Address::from_script` returns `Option<Address>` and the failure modes for non-standard scripts are silent), and (c) makes our verification path strictly less general than it needs to be.
- **The blocking concern from v1.0 STACK.md is unchanged.** That note said: "This is a young crate (0.0.x) — pin the exact version and test carefully. Alternative: implement BIP-322 Simple verification directly using `bitcoin` primitives (about 50 lines) to avoid the dependency risk." We took the alternative; nine months later the dependency risk has not lessened.

**bitcoin-crate compatibility risk (unverifiable from web search):** We could not directly read the `bip322 = 0.0.10` Cargo.toml from search results to confirm it pins to `bitcoin 0.32.x`. The repository link is in sources below; **Sprint 0 of v1.4 phase planning must `cargo tree -p bip322` against a scratch dependency to confirm bitcoin-version alignment before any adoption decision is irreversible.** If `bip322 0.0.10` pins to an older bitcoin crate (0.31.x or earlier), we cannot use it without either (a) waiting for upstream to bump or (b) running parallel bitcoin-crate versions in the dep graph (a sin we have been clean of so far).

### What we are NOT recommending

- **`bip322-rs` (fork at 0.0.11)** — A fork of the official crate. One patch version newer, but a fork in an already-young crate space doubles the maintainership risk rather than halving it. No production users of note. Reject.
- **`bip322-simple` (0.3.1)** — A different simpler crate, mostly aimed at message-signing demos. Higher version number is misleading; it has narrower script coverage and a less idiomatic API. Reject.
- **`bip322-signer` (Meczka)** — GitHub-only, no crates.io presence at a recent version. Reject.

The official `bip322` crate is the only credible external candidate. The decision is "official crate vs. extend custom" — not "which third-party crate."

---

## 2. Extending the Custom `shared/src/bip322.rs`

### What we already have (P2WPKH path, validated v1.0 → v1.3)

[`shared/src/bip322.rs`](../../shared/src/bip322.rs) (133 LOC) currently implements:

- `bip322_message_hash(message: &[u8]) -> [u8; 32]` — BIP-340 tagged hash with tag `b"BIP0322-signed-message"`. **Script-type-agnostic.** Reuse as-is.
- `build_bip322_to_spend(script_pubkey: &Script, msg_hash: &[u8; 32]) -> Transaction` — virtual to_spend per BIP-322 §4. **Script-type-agnostic.** Reuse as-is.
- `build_bip322_to_sign(to_spend: &Transaction) -> Transaction` — virtual to_sign per BIP-322 §5. **Script-type-agnostic.** Reuse as-is.

[`coordinator/src/bitcoin/utxo.rs:114-160ish`](../../coordinator/src/bitcoin/utxo.rs) currently implements `verify_bip322_simple` which: hard-rejects non-P2WPKH at line 119, computes the P2WPKH sighash, ECDSA-verifies. This is the dispatcher that needs to change.

### What we add for v1.4

The to_spend/to_sign tx construction does NOT change per script type — only the sighash computation and the signature verification do. The dispatcher becomes a small match on script-type detection, branching to three verifier functions.

| Script type | Sighash function (bitcoin 0.32.x) | Verification primitive | Witness stack shape |
|-------------|----------------------------------|------------------------|---------------------|
| **P2WPKH** (existing) | `SighashCache::p2wpkh_signature_hash(0, &script_pubkey, Amount::ZERO, EcdsaSighashType::All)` | `secp.verify_ecdsa(sighash, &sig_der, &compressed_pubkey)` | `[sig_der_with_sighash_byte, compressed_pubkey_33b]` (2 items) |
| **P2TR (BIP-86 single-key)** (NEW) | `SighashCache::taproot_key_spend_signature_hash(0, &Prevouts::All(&[txout]), TapSighashType::Default)` | `secp.verify_schnorr(sighash, &schnorr_sig_64b, &x_only_pubkey)` | `[schnorr_sig_64b]` (1 item — or 65b if a non-default sighash type byte is appended; reject non-default for v1.4) |
| **P2SH-P2WPKH** (NEW) | `SighashCache::p2wpkh_signature_hash(0, &redeem_script, Amount::ZERO, EcdsaSighashType::All)` where `redeem_script` is the P2WPKH inner script derived from witness | `secp.verify_ecdsa(sighash, &sig_der, &compressed_pubkey)` | `[sig_der_with_sighash_byte, compressed_pubkey_33b]` + a non-empty `script_sig` containing the P2WPKH redeem script (the to_sign tx's input has `script_sig = push(redeem_script)`) |

**Sighash construction differences in plain English:**

- **P2WPKH:** segwit v0 sighash. ECDSA over the sighash. The script signed is the script_pubkey itself (BIP-143 substitution gives `OP_DUP OP_HASH160 <pubkeyhash> OP_EQUALVERIFY OP_CHECKSIG`). Already working.
- **P2TR keypath spend:** BIP-341 taproot sighash. Schnorr signature (BIP-340) over the sighash. The pubkey on the wire is the **x-only** 32-byte pubkey (tweaked output key); witness is just `[signature]` for default sighash. Verification uses `secp256k1::verify_schnorr` with `XOnlyPublicKey`. **Property tests must cover SIGHASH_DEFAULT (no byte appended) and reject SIGHASH_ALL-with-explicit-byte to keep verification deterministic.** For v1.4 we only support BIP-86 single-key (no script-path spends, no taptree).
- **P2SH-P2WPKH:** the on-chain UTXO has `script_pubkey = OP_HASH160 <hash160(redeemScript)> OP_EQUAL` where `redeemScript = OP_0 <pubkeyhash>` (a P2WPKH script). Per BIP-322 §5 the to_sign input must carry the redeem script in its `script_sig` (as a single push), and the witness is the P2WPKH witness `[sig, pubkey]`. The sighash is the BIP-143 segwit sighash computed over the **inner P2WPKH redeem script** (NOT the outer P2SH `script_pubkey`). Verification then proceeds as P2WPKH. This requires the verifier to: (1) parse the redeem script from `script_sig`, (2) check `hash160(redeem_script) == hash from script_pubkey`, (3) check redeem script is canonical P2WPKH form, (4) compute sighash over redeem script, (5) ECDSA-verify.

**Schnorr-vs-ECDSA verification paths** (bitcoin 0.32.x):

```rust
// ECDSA (P2WPKH + P2SH-P2WPKH):
let secp = bitcoin::secp256k1::Secp256k1::verification_only();
let msg = bitcoin::secp256k1::Message::from_digest(sighash.to_byte_array());
let sig = bitcoin::secp256k1::ecdsa::Signature::from_der(sig_der)?;
let pk = bitcoin::secp256k1::PublicKey::from_slice(pubkey_bytes)?;
secp.verify_ecdsa(&msg, &sig, &pk)?;

// Schnorr (P2TR):
let secp = bitcoin::secp256k1::Secp256k1::verification_only();
let msg = bitcoin::secp256k1::Message::from_digest(sighash.to_byte_array());
let sig = bitcoin::secp256k1::schnorr::Signature::from_slice(sig_64b)?;
let pk = bitcoin::secp256k1::XOnlyPublicKey::from_slice(pubkey_xonly_32b)?;
secp.verify_schnorr(&sig, &msg, &pk)?;
```

Both paths are stable in `bitcoin 0.32.x` and re-exported through it; no additional crate is needed.

### Lines-of-code estimate

| Change | New LOC | Touched LOC |
|--------|---------|-------------|
| `Bip322Error` variants for new failure modes (e.g. `InvalidSchnorrSig`, `RedeemScriptMismatch`, `XOnlyPubkeyInvalid`) | +5 | 0 |
| `verify_bip322_simple` dispatcher: detect `is_p2wpkh`, `is_p2tr`, `is_p2sh` and route | +30 | -1 (remove the hard reject) |
| `verify_p2tr_keypath` function (sighash + Schnorr verify) | +40 | 0 |
| `verify_p2sh_p2wpkh` function (parse redeem script, re-route to P2WPKH-style sighash + ECDSA) | +50 | 0 |
| Tests: per-script-type happy path + adversarial vectors (BIP-322 spec test vectors for each) | +80 | 0 |
| **Total** | **~205 LOC** | ~1 |

For comparison, the official-crate adoption path involves:

- Removing existing `shared/src/bip322.rs` (-133 LOC) and `verify_bip322_simple` in utxo.rs (-50 LOC)
- Adding `bip322 = "=0.0.10"` to `shared/Cargo.toml` (1 line)
- Writing an adapter that converts `(scriptPubKey, network) -> Address` and calls `bip322::verify_simple` (+30 LOC including error mapping)
- Updating the client wallet's signing path to produce the witness stack that `bip322::verify_simple` accepts — almost certainly a rewrite given bdk_wallet 2.3 does not natively expose BIP-322 signing (see §3 below) (+100 LOC)
- Adversarial property tests still required because the crate is 0.0.x (+80 LOC)
- **Total swap delta:** ~100 LOC net change but with a one-shot dependency-removal step that is hard to reverse mid-sprint

The custom-extension path is roughly the same LOC but stays in code we already understand, debugged in v1.3, and own.

### Risk profile

| Risk | Extend custom | Adopt official crate |
|------|---------------|----------------------|
| Sighash construction bug | HIGH — three new sighash paths to get right; mitigation = BIP-322 spec test vectors | LOW — upstream handles it |
| Wire-format break vs. real Bitcoin Core BIP-322 | MEDIUM — we are reimplementing the spec | LOW — upstream conforms |
| Crate API breakage at next release | N/A | HIGH — 0.0.x has no SemVer guarantee, and there has been no release in 9 months so the next one is unpredictable |
| bitcoin-crate version drift | LOW — same crate as the rest of the workspace | MEDIUM — must verify `bip322 0.0.10` pins to `bitcoin 0.32.x` |
| Future maintenance | OUR PROBLEM — full ownership | UPSTREAM PROBLEM — but upstream is silent for 9 months |
| Reversibility mid-sprint | HIGH — each script type can ship independently | LOW — adoption is one big PR with a Cargo.toml change |

**Net:** the extend-custom path trades a known, mitigable, test-vector-coverable correctness risk for a smaller, more reversible delivery risk. The official-crate path trades the spec-correctness risk for an unbounded upstream-stability risk on a crate that has not shipped in nine months.

### Mitigation for the spec-correctness risk

This is the load-bearing concern. Mitigations:

1. **BIP-322 spec test vectors.** The BIP itself ships test vectors for P2WPKH and P2TR. Use `proptest` to apply these as integration tests at each script-type boundary. (PROJECT.md already lists `proptest 1.x` in the stack.)
2. **Cross-implementation differential tests.** Generate signatures with the `bip322-js` JavaScript reference (ACken2/bip322-js — high-quality, used as the de facto reference) for each script type. Persist the test vectors as fixtures. Our verifier must accept them; our signer must produce vectors that `bip322-js` accepts.
3. **`ord` differential test.** `ord` uses `bip322 0.0.10` for verification. Spin up a minimal `ord` integration in a test binary, sign with our impl, verify with `ord`'s. (Optional Sprint 0 PoC; not blocking.)
4. **Coordinator-side: never trust the witness.** The coordinator must always re-derive the script_pubkey hash from the witness pubkey and check it matches the on-chain UTXO's script_pubkey, in addition to verifying the signature. This catches "valid signature for the wrong pubkey" attacks that a pure verifier might miss.

---

## 3. `bdk_wallet` 2.3 — What it gives us for Free, What it Doesn't

### Address generation (free)

bdk_wallet 2.3 supports descriptor types:

- `wpkh(...)` — P2WPKH (already in use)
- `tr(...)` — P2TR keypath (BIP-86 when used with a single key)
- `sh(wpkh(...))` — P2SH-wrapped P2WPKH

For client wallet generation we need to derive HD descriptors per BIP-49 (P2SH-P2WPKH), BIP-84 (P2WPKH, already supported), and BIP-86 (P2TR). The current `client/src/wallet.rs::generate()` hardcodes BIP-84. We need:

- A `--script-type {p2wpkh,p2tr,p2sh-p2wpkh}` CLI flag (or auto-detect from a richer descriptor input).
- Three template descriptors (BIP-49 `sh(wpkh(...m/49'/0'/0'/0/*))`, BIP-84 `wpkh(...m/84'/0'/0'/0/*)` already there, BIP-86 `tr(...m/86'/0'/0'/0/*)`).
- BIP-84 stays the default for backward compatibility.

This is a ~50-LOC change in `wallet.rs::generate()` and `wallet.rs::from_descriptor()` plus argument plumbing.

### CoinJoin TX signing (free)

bdk_wallet 2.3's `wallet.sign(psbt, SignOptions)` already handles taproot keypath signing and P2SH-P2WPKH segwit signing when the wallet's descriptor matches. The existing `trust_witness_utxo: true` workaround we use for P2WPKH ([client/src/wallet.rs:269](../../client/src/wallet.rs#L269)) is needed for P2TR too — confirm in integration tests that `bdk_wallet 2.3` populates `tap_internal_key` and consumes the `witness_utxo` for taproot signing without requiring full `non_witness_utxo`. (Documented in BDK blog "First BDK Taproot TX, Part 2".) **Sprint 0 PoC should validate this for taproot + P2SH-P2WPKH before committing to the descriptor-flag plumbing.**

### BIP-322 signing (NOT free — open issue)

**bdk_wallet does NOT natively provide BIP-322 message signing.** `bitcoindevkit/bdk_wallet` issue [#150 "Feature Proposal: BIP322 message signing"](https://github.com/bitcoindevkit/bdk_wallet/issues/150) is still **open** (filed May 2023, no resolution as of search 2026-05-29).

This is the structural reason the "let bdk_wallet do it for us" path does not exist for v1.4. We must do BIP-322 ourselves (extend custom) OR via the standalone `bip322` crate. There is no third option through BDK.

The signing-key access path we already use (`secret_key_for_signing()` for WIF wallets — see [client/src/wallet.rs:220](../../client/src/wallet.rs#L220)) needs to be generalized. For descriptor wallets where we currently `expect("not available")`, we have to extract the signing key for the first external address by descriptor-introspection. bdk_wallet 2.3 exposes signer plumbing (`SignerOrdering`, `TransactionSigner`, etc.) — the pragmatic approach for v1.4 is to extract the derived key by deriving the descriptor manually using `miniscript` + the inner xprv, rather than wedging into BDK's signer API. This is a ~30-LOC helper, keeps BIP-322 signing out of BDK's PSBT signer entirely, and stays within the BDK boundary we already understand.

### What we do NOT add

- ❌ **rust-miniscript as a direct dep.** bdk_wallet 2.3 already pulls in miniscript transitively. If we need miniscript types directly for descriptor parsing, use `bdk_wallet::miniscript::*` re-exports. Adding `miniscript` as a top-level dep would create a version-drift risk against BDK's pin.
- ❌ **`bdk_chain` or `bdk_electrum`.** Not needed for BIP-322 signing or PSBT signing of new script types. Client already has what it needs.
- ❌ **Hardware-wallet signer crates (`bdk_hwi`, etc.).** v1.4 scope is HD-software-wallet only.

---

## 4. `bitcoin 0.32.x` Built-in Capabilities (and why we don't need a separate sighash crate)

The `bitcoin` crate 0.32.x already provides every primitive needed for the verifier:

| Need | API in `bitcoin 0.32.x` |
|------|-------------------------|
| Script type detection | `Script::is_p2wpkh()`, `Script::is_p2tr()`, `Script::is_p2sh()` |
| P2WPKH sighash | `SighashCache::p2wpkh_signature_hash(input_index, script_code, value, sighash_type)` |
| P2TR keypath sighash | `SighashCache::taproot_key_spend_signature_hash(input_index, &Prevouts::All(&[txout]), TapSighashType::Default)` |
| BIP-340 tagged hashes (generic) | `bitcoin::hashes::sha256::Hash` + `HashEngine` (already used) |
| Schnorr signature verify | `secp256k1::Secp256k1::verify_schnorr` |
| ECDSA signature verify | `secp256k1::Secp256k1::verify_ecdsa` |
| `XOnlyPublicKey` parsing | `bitcoin::secp256k1::XOnlyPublicKey::from_slice` |
| `Witness` consensus serialize/deserialize | already in use for wire format (see [client/src/wallet.rs:283](../../client/src/wallet.rs#L283)) |

**No additional crate is needed for the sighash + verification work.** The only library decision in v1.4 is "adopt `bip322` for the verifier glue, or write the glue ourselves." Everything underneath is supplied by `bitcoin 0.32.x` already in the workspace.

This is the strongest reason the extend-custom path is cheap: we are not reimplementing crypto, we are writing a ~30-LOC dispatcher and three ~50-LOC verifier functions on top of primitives we already depend on.

---

## 5. `miniscript` / Interpreter — What it does NOT solve here

The `rust-miniscript` ecosystem provides:

- `miniscript::interpreter::Interpreter` — generalized script interpreter, can return a sighash for arbitrary Miniscript-expressible scripts.
- `miniscript::descriptor::Descriptor` — descriptor parsing and address derivation.

**This is not the right tool for v1.4 BIP-322 verification.** The Interpreter is built for spending-transaction verification — it consumes a real `Transaction` + `UTXO` set and verifies witness execution under consensus rules. It is overkill for BIP-322 Simple, which has a fixed virtual-transaction structure and three known script types. Reaching for the Interpreter would couple our BIP-322 verifier to the much larger Miniscript API surface, add LOC, and obscure the spec correspondence.

**Where Miniscript may help:** if v1.5+ extends to BIP-322 Full (script-path spends, P2WSH multisig, taptree script-path), the Interpreter becomes more attractive. For v1.4's targeted three-script-type Simple coverage, skip it.

---

## 6. Coordinator Discovery of Supported Script Types

v1.4 needs the coordinator to advertise its supported script types so clients reject mismatched coordinators pre-registration. Two surfaces:

### PKARR record extension

The existing PKARR-published TXT record already carries `denomination` and `status`. Add a `supported_scripts` field containing a comma-separated list of canonical tokens: `p2wpkh,p2tr,p2sh-p2wpkh`. Order-insensitive; clients should accept any order.

- **No new crates required.** PKARR record is application-level JSON we already serialize via serde.
- Token vocabulary should be the union of what `Script::is_p2wpkh()` / `is_p2tr()` / `is_p2sh()` detect; document the wire token strings in `shared/src/protocol.rs` as a `ScriptType` enum with `serde(rename)` annotations.

### `/round/info` endpoint extension

Mirror the same `supported_scripts` field in the JSON response. Existing axum + serde plumbing covers this — no new dependencies.

**Why not negotiate per-request:** clients can detect mismatch from a static record before any TCP roundtrip to the coordinator. Per-request negotiation would add a phase to the protocol and is unnecessary for v1.4's fixed coordinator-advertises-supported-list model.

---

## 7. Liquidity Bot Updates

The liquidity bot (under `coordinator/src/liquidity_bot/...`, see PROJECT.md) generates P2WPKH UTXOs for test rounds. v1.4 needs it to generate UTXOs across all supported script types.

- Use bdk_wallet 2.3's `tr(...)` and `sh(wpkh(...))` descriptors to derive addresses for the bot's three script types.
- Distribute test UTXOs across script types proportionally (e.g., round-robin or random) to exercise mixed-type rounds.
- No new crates — same bdk_wallet 2.3 plumbing as the client wallet.

---

## 8. Testing — Crates Already In-Stack

| Need | Already-in-stack crate | New crate? |
|------|------------------------|------------|
| Property tests over BIP-322 spec vectors | `proptest 1.x` (PROJECT.md) | **No** |
| Integration tests with bitcoind regtest | `corepc-node 0.12` (feature-pinned per v1.3 REPAIR-02) | **No** |
| Differential tests against `bip322-js` reference | Fixtures only — pre-generate vectors and ship them as JSON in `tests/fixtures/bip322/`. JS regeneration is a developer-tool one-time script, not a runtime dep. | **No** |
| Differential tests against `ord` BIP-322 | Optional stretch; if needed, run as a CI job using `ord` binary, not as a Cargo dep | **No** |

**No new testing crates are needed for v1.4.**

---

## 9. Version Pins for v1.4 Cargo.toml (additions / changes only)

The v1.0 baseline pins in PROJECT.md stay unchanged. v1.4 adds no new top-level dependencies under the extend-custom recommendation. If the discuss phase pivots to adopt-official, add exactly one line:

```toml
# Fallback path (NOT recommended for v1.4): if discuss phase votes to adopt official crate
# shared/Cargo.toml [dependencies]
bip322 = "=0.0.10"  # exact pin; 0.0.x has no SemVer guarantee. Required Sprint 0 check:
                    # `cargo tree -p bip322` to confirm pin against `bitcoin 0.32.x`.
                    # If `bip322 0.0.10` pulls in `bitcoin 0.31.x` or earlier, this path is BLOCKED
                    # — do not allow two bitcoin versions in the workspace graph.
```

Under the recommended extend-custom path, **zero Cargo.toml changes** are required for the BIP-322 work itself. The script-type plumbing through `client/src/wallet.rs` for BIP-49/BIP-86 descriptor support is also dep-free (bdk_wallet 2.3 already exposes `tr()` and `sh(wpkh())` descriptors).

### What stays exactly as v1.3

| Crate | Pinned at | Why no change in v1.4 |
|-------|-----------|----------------------|
| `bitcoin` | 0.32.x | All sighash + verification primitives for P2TR / P2SH-P2WPKH are already here |
| `bdk_wallet` | 2.3.x | Already supports `tr()` and `sh(wpkh())` descriptors and signing for both |
| `secp256k1` | (via bitcoin) | Already exposes `verify_schnorr` and `XOnlyPublicKey` |
| `proptest` | 1.x | Sufficient for per-script-type property tests |
| `corepc-node` | 0.12 (features pinned) | Regtest harness for integration tests; no script-type-specific blocker |
| All other v1.0 baseline crates | (PROJECT.md) | Unaffected by v1.4 scope |

---

## 10. Alternatives Considered (v1.4-specific)

| Decision | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| BIP-322 impl source | **Extend `shared/src/bip322.rs`** | Adopt `bip322 = "=0.0.10"` | Crate stalled at 0.0.10 for ~9 months. No SemVer guarantee. API surface (`&Address` based) mismatches our `(scriptPubKey, witness, message)` wire format. Risk of bitcoin-crate version drift unverified. |
| BIP-322 impl source | **Extend `shared/src/bip322.rs`** | Adopt `bip322-rs = 0.0.11` (fork) | Forked from already-young upstream. Doubles maintainership risk. No production users of note. |
| Sighash construction | **`bitcoin 0.32.x` `SighashCache`** | Pull `rust-miniscript` Interpreter | Interpreter is overkill for BIP-322 Simple (three known script types). Couples to a much larger API surface. Reach for it only if scope expands to BIP-322 Full / script-path / P2WSH in v1.5+. |
| BIP-322 signing on client | **Custom helper using descriptor-derived key** | Wait for bdk_wallet to ship BIP-322 (issue #150) | Issue #150 open since May 2023, no resolution signal. Blocking on it indefinitely stalls v1.4. |
| New script-type wallet descriptors | **BIP-49 (P2SH-P2WPKH), BIP-84 (P2WPKH, already), BIP-86 (P2TR)** | Custom non-standard derivation paths | BIP-49/84/86 are interop standards; users importing existing seeds get the right addresses without surprise. |
| Coordinator script-type advertisement | **PKARR `supported_scripts` field + `/round/info` mirror** | Per-request capability negotiation | Adds a protocol phase. Static advertisement is sufficient and lets clients reject pre-registration. |
| Test reference for differential vectors | **`bip322-js` (ACken2) for fixture generation** | Cross-validate against `ord` at runtime | ord differential adds a CI dep and a binary install step; fixture-based is hermetic. ord-cross-check is a stretch goal. |
| Cargo.toml change | **None for the BIP-322 work itself** | Add `bip322 = "=0.0.10"` | See first row. |

---

## 11. What This v1.4 Stack Does NOT Add

Explicit anti-list to prevent scope creep during planning:

- ❌ `bip322` crate (deferred — see TL;DR rationale)
- ❌ `bip322-rs` fork
- ❌ `bip322-simple` (different crate, narrower scope, wrong fit)
- ❌ `bip322-signer` (Meczka, no crates.io presence)
- ❌ `rust-miniscript` as a direct dep (use BDK's transitive pin)
- ❌ `bdk_chain`, `bdk_electrum`, `bdk_esplora` (not needed for signing)
- ❌ `bdk_hwi` and any hardware-wallet signer crates (software-wallet only for v1.4)
- ❌ A separate `taproot` / `schnorr` crate (`bitcoin 0.32.x` already includes both)
- ❌ A new `descriptor` parser (use BDK's)
- ❌ A property-testing framework swap (`proptest 1.x` is sufficient)
- ❌ JS-runtime cross-validation in the test binary (use file fixtures, regenerate offline)

If discuss-phase votes to relax the "no `bip322` crate" recommendation, **exactly one** additional line goes in Cargo.toml and the cost of the swap should be re-estimated with the Sprint 0 `cargo tree` check as a hard prerequisite.

---

## 12. Open Questions for Discuss Phase

1. **`cargo tree -p bip322 0.0.10` against bitcoin 0.32.x — is the version aligned?** This is the single experiment that flips the recommendation. If `bip322 0.0.10` pins to `bitcoin 0.32.x`, the "official crate" path is at least technically viable. If it pins older, the path is BLOCKED until upstream bumps. Run this check in Sprint 0 before any commit to a direction.
2. **Are we willing to lock v1.4 BIP-322 verification semantics to a 0.0.x dependency with a 9-month silence?** Even if `cargo tree` is clean, the upstream-stability risk is real. This is the framing for the discuss-phase decision.
3. **P2TR sighash policy: SIGHASH_DEFAULT only, or accept SIGHASH_ALL too?** The BIP-322 spec doesn't pin a sighash type. For v1.4 we recommend SIGHASH_DEFAULT only (1-item witness, no appended byte) to keep the verifier deterministic and the wire format stable. Discuss whether this constrains real-world client wallets producing test signatures.
4. **Are mixed-script-type rounds first-class or second-class?** PKARR-published `supported_scripts` is a per-coordinator setting. Should we allow a coordinator to advertise `[p2wpkh, p2tr]` but reject P2SH-P2WPKH? Or is it all-or-nothing per coordinator? v1.4 minimum is "coordinator advertises a set; client checks intersection," but the policy on partial subsets is a discuss-phase call.
5. **Does the liquidity bot need to participate in mixed-script-type rounds, or only same-script-type rounds for now?** Mixed rounds exercise the full path but increase test-runtime complexity. Same-script-type with per-round-type sweeps is easier to debug.

---

## Sources

### High-confidence (multiple sources or official)

- [crates.io/crates/bip322](https://crates.io/crates/bip322) — Confirms `0.0.10` latest, ~7619 downloads/month, ~14 reverse-deps, supports P2TR/P2WPKH/P2SH-P2WPKH single-sig, MSRV 1.63, CC0-1.0, co-owned by Andrew Poelstra [HIGH]
- [docs.rs/bip322/latest/bip322/fn.verify_simple.html](https://docs.rs/bip322/latest/bip322/fn.verify_simple.html) — `pub fn verify_simple(address: &Address, message: impl AsRef<[u8]>, signature: Witness) -> Result<(), Error>` [HIGH]
- [github.com/rust-bitcoin/bip322](https://github.com/rust-bitcoin/bip322) — Official repo; README states "P2TR, P2WPKH and P2SH-P2WPKH single-sig addresses" supported [HIGH]
- [github.com/bitcoindevkit/bdk_wallet/issues/150](https://github.com/bitcoindevkit/bdk_wallet/issues/150) — "Feature Proposal: BIP322 message signing" open since May 2023, no resolution as of search 2026-05-29 [HIGH]
- [crates.io/crates/ord](https://crates.io/crates/ord) — `ord 0.24.2` reverse-deps on `bip322 ^0.0.10` (production user signal) [HIGH]
- [docs.rs/bitcoin/0.32.6/bitcoin/](https://docs.rs/bitcoin/0.32.6/bitcoin/) — Confirms `SighashCache::p2wpkh_signature_hash`, `taproot_key_spend_signature_hash`, `Script::is_p2tr/is_p2wpkh/is_p2sh` all in 0.32.x [HIGH]
- [rust-bitcoin.org/book/tx_taproot.html](https://rust-bitcoin.org/book/tx_taproot.html) — Confirms taproot keypath spending recipe using `SighashCache::new()` + `taproot_key_spend_signature_hash` in `bitcoin 0.32.x` [HIGH]

### Medium-confidence

- [crates.io/crates/bip322-rs](https://crates.io/crates/bip322-rs) — Fork at 0.0.11; explicitly a fork of `rust-bitcoin/bip322`; same script-type coverage [MEDIUM]
- [crates.io/crates/bip322-simple](https://crates.io/crates/bip322-simple) — Separate crate (`0.3.1`), narrower scope, primarily nested-segwit + taproot message signer; less idiomatic Rust API [MEDIUM]
- [bitcoindevkit.org/blog/2021/12/first-bdk-taproot-tx-look-at-the-code-part-2/](https://bitcoindevkit.org/blog/2021/12/first-bdk-taproot-tx-look-at-the-code-part-2/) — BDK taproot PSBT signing semantics (older blog but pattern still applies in 2.3) [MEDIUM]
- [github.com/ACken2/bip322-js](https://github.com/ACken2/bip322-js) — JS reference implementation; supports P2TR + P2WPKH + P2SH-P2WPKH; suitable as a differential-test oracle for our extend-custom path [MEDIUM]
- [bips.dev/322](https://bips.dev/322/) — BIP-322 spec text (the canonical source for to_spend / to_sign virtual transaction construction) [HIGH for spec, but spec ambiguity around sighash type for P2TR is itself a known issue]

### Lower-confidence (single source or inferred)

- 9-month release-silence on `bip322` crate — inferred from "Metadata age September 5, 2025" reported by crates.io listing fetched 2026-05-29; **verify directly via the crate's release page in Sprint 0** [MEDIUM]
- `bip322 0.0.10` Cargo.toml exact dependency on `bitcoin` crate version — **NOT verified from web search**; must be checked via `cargo tree -p bip322` in Sprint 0 before any adoption decision is irreversible [LOW]

---

## Quality-Gate Self-Check

- ✅ Versions current as of 2026-05-29 (verified via crates.io search same day)
- ✅ Adopt-vs-extend rationale explains WHY for v1.4 specifically (TL;DR + §1 + §2 risk profile table)
- ✅ Specific crate version pins recommended (zero new deps under recommended path; exact pin `bip322 = "=0.0.10"` documented for fallback)
- ✅ Integration with bdk_wallet 2.3 considered explicitly (§3, including address generation = free, BIP-322 signing = not free)
- ✅ Anti-list of crates NOT to add documented (§11)
- ✅ Sources marked with confidence levels
- ✅ Open questions for discuss-phase enumerated (§12)
- ⚠ One source could not be verified by search and is flagged for Sprint 0: `bip322 0.0.10`'s exact pin to `bitcoin 0.32.x` (§9 fallback footnote + §12 Q1)

---

*v1.0 baseline stack rationale is preserved in [`.planning/PROJECT.md`](../../.planning/PROJECT.md). This document supersedes the prior `.planning/research/STACK.md` for v1.4 multi-script-type scope only.*
