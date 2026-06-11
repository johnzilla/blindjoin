---
title: "Intermittent \"BIP-322 crate verification failed\" on ECDSA descriptor wallets"
date: 2026-06-11
type: bug
tags: [bip322, ecdsa, descriptor-wallet, bdk, flaky-test, signature-grinding, p2sh-p2wpkh]
area: client/src/wallet.rs, shared/src/bip322
symptom: "register_input → 400 INVALID_PROOF / 'BIP-322 crate verification failed', intermittent (~5%+)"
---

# Intermittent BIP-322 verification failure on ECDSA descriptor wallets

## Symptom

`mixed_script_e2e` (and any round with a P2SH-P2WPKH or P2WPKH **descriptor**
client) intermittently fails at input registration:

```
reg_reject = InvalidProof { reason: "BIP-322 crate verification failed" }
client2: register_input: HTTP status client error (400 Bad Request)
```

Failure rate ~1-in-3 across CI runs; passes clean on re-run. The round_id is
stable (no quorum reset), all clients register within tens of ms — so it is
**not** a timeout, not `UTXO_NOT_FOUND`, and not load-related. It is the
P2SH-P2WPKH client's ownership proof failing cryptographic verification.

## Root cause

The pinned `bip322 = "=0.0.10"` crate hardcodes, in its witness parser,
`match signature_length { 71 | 72 => ok, _ => SignatureLength }`. It rejects
ECDSA witness signatures of **70 or 73 bytes** as malformed — even though those
are perfectly valid Bitcoin ECDSA DER signatures (70 when R and S both serialize
to 32 bytes with no `0x00` pad; 73 when both pad). ~5% of signatures land outside
71/72.

The codebase already knew this and worked around it with
`shared::bip322::sign_ecdsa_compat_bip322_length`, which re-signs (deterministic
nonce-counter retries) until the signature is 71/72 bytes. The **WIF** client
path (`from_wif` → `secret_key_for_signing` → `sign_simple`) used it and was
immune.

But **descriptor** wallets (`generate`, `from_descriptor`) signed their BIP-322
proof via **bdk** (`self.inner.sign(psbt)`), which does no such grinding. bdk's
ECDSA is RFC-6979 deterministic, so for a given (wallet key, message) the
signature length is fixed — and ~5%+ of (generated key, round_id) combinations
produced a 70/73-byte signature that the pinned verifier rejected. Because the
test generates a fresh random key and a random round_id each run, ~5%+ of runs
hit a bad combination → the flake. P2TR descriptor wallets were immune (Schnorr
signatures are a fixed 64 bytes; the length check never bites).

**This was a real product bug, not a test-only artifact:** any real
P2SH-P2WPKH (or P2WPKH) descriptor-wallet client had a ~5%+ chance per round of
being silently unable to register.

## Fix

Route ECDSA descriptor proofs through the same grinding-aware `sign_simple` the
WIF path uses, by deriving the External index-0 leaf secret key from the
descriptor's xprv at construction (`derive_external_leaf_sk`) and storing it as
`BdkClientWallet.external_leaf_sk` — but only when that leaf key actually controls
the registered UTXO (`ecdsa_leaf_controls_script`), so `from_descriptor` with a
non-index-0 UTXO safely falls back to bdk. `sign_bip322` uses it for P2WPKH /
P2SH-P2WPKH; P2TR stays on bdk. For in-range signatures `sign_simple` is
byte-identical to bdk (guarded by the existing `*_matches_bdk_sign_byte_for_byte`
tests), so behaviour only changes for the ~5% bdk would have gotten rejected.

Regression guard: `wallet::tests::descriptor_ecdsa_proofs_grind_to_verifiable_length`
iterates 64 fixed seeds, asserting every descriptor P2SH-P2WPKH proof grinds to
71/72 bytes and verifies through the same crate the coordinator uses. Confirmed
to FAIL when the fix is disabled.

## How to recognize this class of bug

- An **intermittent** crypto-verification failure that passes on re-run, with a
  **deterministic** signer (RFC-6979) is almost always a **signature-encoding /
  length** edge, not a randomness bug. The per-(key, message) determinism means
  ~X% of *inputs* fail, not ~X% of *attempts*.
- When a fragile pinned crate (here `bip322 = "=0.0.10"`, flagged pre-1.0 in
  CLAUDE.md) has a known length workaround, check that **every** signing path
  goes through it — not just the one the original author tested. We had two
  signing paths (WIF vs descriptor) and only one was grinded.
- Don't trust "flake" until you've captured the actual rejection reason. The
  decisive step here was instrumenting the coordinator's `UtxoError` mapping and
  reproducing locally (`BITCOIND_EXE=$(which bitcoind)`, poll timeouts temporarily
  cut 600→45s so failures self-terminate). The symptom (client2 400 + others
  stalling on `output_reg`) initially looked like a phase-timeout race; the logs
  proved otherwise.
