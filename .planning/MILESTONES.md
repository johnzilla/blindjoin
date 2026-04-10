# Milestones

## v1.1 Security & Availability Hardening (Shipped: 2026-04-10)

**Phases completed:** 2 phases, 4 plans, 7 tasks

**Key accomplishments:**

- PR-triggered CI gate with cargo test, clippy, and audit as independent jobs; release and Docker workflows gated on test+clippy prerequisite
- All GitHub Actions pinned to immutable commit SHAs; SHA-256 checksums on release archives; workflow permissions scoped per-job
- validate_utxo RPC moved before RoundState write lock — slow bitcoind cannot serialize concurrent input registrations (AVAIL-01)
- RsaBlindSigner cached per-round in RoundStateInner — no per-request RSA key deserialization on hot path (AVAIL-02)
- Address validation at registration time, blinded token size bounds, duplicate partial-sig guard, fee formula consolidated to single canonical function

---

## v1.0 MVP (Shipped: 2026-04-09)

**Phases completed:** 5 phases, 17 plans, 21 tasks

**Key accomplishments:**

- Cargo workspace with shared crate providing all wire types, domain-separated blind token hasher (SHA-256 blindjoin-v1 domain separator), serde forward-compatible message structs, and canonical OwnershipProof wire type
- One-liner:
- Thin Bitcoin Core RPC client (5 methods), UTXO validation with BIP-322 Simple P2WPKH proof verification, and CoinJoin PSBT construction with per-participant fee splitting and sub-294-sat dust folding
- 1. [Rule 2 - Missing Critical Functionality] Added msg_randomizer to OutputRegRequest
- One-liner:
- One-liner:
- SHA-256-keyed in-memory BanList with configurable expiry wired into POST /round/input (HTTP 403), plus detect_non_signers() diffing registered_inputs vs partial_sigs for BLAME-01/02 coverage
- JSONL ban file persistence with SHA-256 hashed utxo keys wired into signing/output-reg timeouts and coordinator startup
- 7 new tests (3 TEST-06 signing + 4 TEST-07 blame unit + 1 blame integration) verify non-signer banning, FSM zeroing (PRIV-01), and end-to-end blame timeout via shared BanList Arc
- bdk_wallet 2.3 descriptor wallet with BIP-39 mnemonic generation, BIP-84 HD derivation, and PSBT output-count anti-censorship check before signing
- 5 new integration tests covering replay token, invalid UTXO, wrong denomination, tampered PSBT (CLI-04), and round restart after blame with ban enforcement — all 8 integration tests pass, bitcoind-dependent tests skip gracefully
- 1. [Rule 1 - Bug] Keypair file API takes &Path not &str
- 1. [Rule 1 - Bug] InfoResponse has no Default derive — explicit field construction in tests
- Coordinator serves axum API over arti v3 onion service when tor_mode=true; TCP path unchanged for dev/test
- One-liner:
- Matrix binary release (4 targets, cross-rs for ARM64) + multi-arch Docker image push to ghcr.io via cargo-chef Dockerfiles

---
