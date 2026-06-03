# blindjoin

## What This Is

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet. Uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. Ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. MIT licensed. No fees. No company. No terms of service.

## Core Value

Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

## Current State

v1.5 Audit-Readiness & Multi-Script Finish **shipped 2026-05-31** (3 phases, 5 plans). Phase 19 closed the v1.4 BIP-322 visibility-escalation hole by shipping production `sign` bodies for P2TR (Schnorr keypath via `sign_schnorr_no_aux_rand`) and P2SH-P2WPKH (BIP-143 over unwrapped P2WPKH redeem) and deleting `sign_simple_test_only` + per-script `sign_for_tests` helpers — `shared::bip322` public surface is now 9 symbols with V1.4-CRIT-01 dispatcher-only load-bearing at the type level. Phase 20 closed the v1.4 fee-accuracy gap with a per-script BIP-141 weight table in `coordinator/src/bitcoin/tx.rs` (P2WPKH 68/31, P2TR 58/43 round-UP, P2SH-P2WPKH 91/32); v1.4 P2WPKH-only `fee_share == 266` byte-equality preserved (regression test pinned). Phase 21 readied the codebase for external audit: `docs/AUDIT-CHARTER.md` (574 LOC, 8 sections per AUDIT-01), `.cargo/audit.toml` rewritten to cite the charter and the AUDIT-03 bounded-window mitigation (no more "best-effort"), and the RSA SecretKey lifetime tightened from prose into a Rust type signature (`RoundStateInner.rsa_signer: Option<RsaBlindSigner>` + `RoundSecretKey(BjSecretKey)` newtype with empty-crypto Drop; `transition_to(Phase::Idle)` at `state.rs:202` is the SOLE FSM chokepoint, verified by grep). One residual risk surfaced and documented in §7: `let _ = state.transition_to(...)` at the 3 success-path FSM trigger sites discards the transition Result — safe under current write-lock + `can_transition_to` invariants but explicitly flagged for review on any future FSM-concurrency or edge-set change. README §Security Model link to the charter pending visual confirmation via `/gsd:verify-work 21` (HUMAN-UAT.md item 3).

v1.4 BIP-322 Multi-Script Support **shipped 2026-05-31** (5 phases, 15 plans, ~3 days). Broadened CoinJoin participation from P2WPKH-only to P2WPKH + P2TR + P2SH-P2WPKH UTXOs. The `is_p2wpkh()` hard gate at `coordinator/src/bitcoin/utxo.rs:119` is gone — coordinator now accepts all three script types via a `match version { 1 => v1_path, 2 => v2_path }` dispatcher that derives `ScriptType` from on-chain `script_pubkey` and cross-checks against the client-declared type (V1.4-CRIT-01 spoofing mitigation; CI grep-gated on both client and coordinator sides). `shared::bip322` adopted the upstream `bip322 = "=0.0.10"` crate via a 26-LOC zero-lossy adapter behind a dispatcher-only public surface that makes V1.4-CRIT-01 spoofing **statically unreachable** at the type level. PKARR record bumped `0.1.0 → 0.2.0` with B3 compact field names (`v`/`sst`/`ost`) — production-onion measured 209 bytes, 11-byte headroom under the 220-byte threshold. Client wallet supports `--type {p2wpkh|p2tr|p2sh-p2wpkh}` with literal BIP-84/86/49 descriptors (coin=0' across all networks per RESEARCH Pitfall 2); `discover_coordinator(pkarr_pubkey, required_script_type)` rejects mismatched coordinators with `UnsupportedScriptType` BEFORE opening any Tor circuit (V1.4-MOD-03 fail-fast). WALLET-04 compatibility shim verified bidirectionally: v1.4→v1.3 via the CD-7 two-phase try-parse encoder (byte-identical v1.3 array-of-hex output in the legacy branch); v1.3→v1.4 inline against a pinned v1.3 binary at SHA `05f21438`. Acceptance gate `mixed_script_e2e_three_clients_broadcast` runs 1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH input through INPUT_REG → BROADCAST. Liquidity bot rotates `script_types` per round via `BLINDJOIN_BOT_SCRIPT_TYPES` CSV (defeats V1.4-MIN-02 uniform-script fingerprint). v1.3 P2WPKH-only `full_round::*` invariant held green at every v1.4 phase boundary (8/8 PASS).

Shipped to date: v1.0 MVP → v1.1 Security & Availability → v1.2 Production Readiness → v1.3 Test Infrastructure → v1.4 BIP-322 Multi-Script (15 phases total across 5 milestones; ~11,296 Rust LOC across coordinator/client/shared/liquidity-bot).
Tech stack: axum 0.8, arti-client 0.41, blind-rsa-signatures, bdk_wallet 2.3, pkarr, tokio, tower_governor 0.8, tower-http 0.6, corepc-node 0.12 (features pinned), bitcoind v30.2, `bip322 = "=0.0.10"` (pinned, adapter at `shared/src/bip322/mod.rs`).
Coordinator runs as Tor v3 hidden service. Client uses per-phase isolated Tor circuits.
PKARR DHT discovery live with `v0.2.0` schema advertising `sst`/`ost`. Docker Compose stack operational.
CI/CD: PR-triggered test/clippy/audit gates, release and Docker workflows gated on check jobs, actions SHA-pinned, bitcoind cached + PGP-verified, corepc-node feature pin enforced, `bip322-pin-check` enforces the `=0.0.10` pin, `crit-01-grep-check` (coordinator) + `crit-01-client-grep-check` (client) enforce CRIT-01 invariant tokens.
Coordinator hardened: RPC outside write lock, RSA signer cached per-round, address pre-validation, blinded token bounds, public-endpoint DoS resistance, real on-chain witness_utxo values in PSBT assembly, multi-script ownership-proof dispatcher with on-chain cross-check.

## Last Milestone: v1.6 Supply-Chain Attestation — Shipped 2026-06-03

4 phases (22, 23, 24, 25), 20 shipped plans + 1 superseded. cosign keyless OIDC signing on every ghcr.io image and every release tarball, SLSA v1.0 provenance, SPDX SBOM via Syft, base-image digest pinning with on-demand drift check, byte-equal reproducible build recipe with a dispatch-only verifier. SIGN-03 (PGP YubiKey path) deferred indefinitely 2026-06-02; REPRO-04 (reproducible-builds.org registry submission) descoped at close as vanity-badge theater. Stripped ~1,400 lines of process theater that the GSD planner had scaffolded into shipped artifacts. v1.6.0 release tag pending the reproducibility baseline-capture procedure in `docs/RELEASING.md`. Full summary in `.planning/milestones/v1.6-ROADMAP.md`.

## Next Milestone

TBD — run `/gsd:new-milestone` to define v1.7.

## Requirements

### Validated

- ✓ Coordinator runs CoinJoin rounds with RSA blind signatures (RFC 9474) ensuring unlinkability — v1.0
- ✓ Fixed-denomination CoinJoin with configurable denomination (default: 0.01 BTC) — v1.0
- ✓ Round state machine: IDLE → INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST/BLAME → IDLE — v1.0
- ✓ Blame protocol: non-signers detected, temporarily banned, round restarts with remaining participants — v1.0
- ✓ Client CLI: discover coordinator, register input, blind token, register output, verify TX, sign — v1.0
- ✓ Client uses fresh Tor circuit for each phase (input reg vs. output reg) — v1.0
- ✓ UTXO ownership proof via BIP-322 generic message signing — v1.0
- ✓ PKARR record publishing: coordinator announces .onion address, round parameters, status to DHT — v1.0
- ✓ PKARR discovery: client finds coordinators via DHT lookup or direct .onion address — v1.0
- ✓ Tor-native: coordinator runs as Tor hidden service via arti-client, no clearnet endpoint in production — v1.0
- ✓ Docker Compose stack: bitcoind + coordinator + liquidity bot, zero to CoinJoin in 5 minutes — v1.0
- ✓ Liquidity bot: auto-joins rounds on signet for testing and cold-start — v1.0
- ✓ All round state zeroed from memory after transaction broadcast — v1.0
- ✓ No logging of PII, IP addresses, or input-output mappings — v1.0
- ✓ Integration tests: full round (3+ participants), blame protocol, adversarial scenarios on signet — v1.0
- ✓ Pre-built binaries via GitHub Releases, Docker images via ghcr.io — v1.0

- ✓ CI/CD security pipeline: cargo test, cargo audit, cargo clippy as gates on PRs and releases — v1.1
- ✓ Eliminate write-lock DoS: move async RPC call outside RoundState write lock in post_input — v1.1
- ✓ Eliminate key deserialization DoS: parse RSA key once at round creation, reuse across requests — v1.1

- ✓ Per-route rate limiting on coordinator HTTP API (read split 60/min, write split 30/min) with HTTP 429 + Retry-After + RATE_LIMITED JSON envelope — v1.2 Phase 8
- ✓ Uniform request timeout (HTTP 408) honoring `request_timeout_secs` — v1.2 Phase 8
- ✓ Tor accept-loop connection cap via `tokio::sync::Semaphore` (default 256) with ConnectionGuard RAII — v1.2 Phase 8
- ✓ Operator-tunable knobs via `coordinator.toml` and `BLINDJOIN__COORDINATOR__*` env vars; validated at startup — v1.2 Phase 8
- ✓ GlobalKeyExtractor for rate limiting (Tor-safe; PeerIpKeyExtractor would break under Tor) — v1.2 Phase 8

- ✓ CI installs a pinned `bitcoind` binary (cached, PGP+SHA256-verified) so integration tests can spawn it without per-job download cost — v1.3 Phase 9 (TEST-01)
- ✓ Integration tests that require bitcoind actually execute in CI on every PR — no silent graceful-skips — v1.3 Phase 9 (TEST-02)
- ✓ `cargo test` output streams cleanly (no buffering pipes) and exits on test panic without blocking on leaked child processes — v1.3 Phase 9 (TEST-03)
- ✓ `corepc-node` test fixtures release their spawned `bitcoind` on test end (zero `Box::leak` across `tests/integration/`) — v1.3 Phase 9 (TEST-04)
- ✓ `CONTRIBUTING.md` documents the canonical integration-test invocation pattern — v1.3 Phase 9 (TEST-05)
- ✓ `full_round.rs` repaired — all 8 tests pass against pinned bitcoind v31 via direct code commits (RSA SPKI handshake, bdk_wallet 2.3 `trust_witness_utxo`, wire-format `Witness` consensus encoding, real on-chain `witness_utxo`, ban-check ordering, error-body surfacing) — v1.3 Phases 10-13 (REPAIR-01 closed-local; full PR observation pending v1.4 cut)
- ✓ Any test using corepc-node's typed `Client` declares an explicit version feature (CI grep gate enforces) — v1.3 Phase 10 (REPAIR-02)

- ✓ Adopted upstream `bip322 = "=0.0.10"` crate (rust-bitcoin org) behind a 26-LOC zero-lossy adapter at `shared/src/bip322/mod.rs`; pinned via `bip322-pin-check` CI grep gate — v1.4 Phase 15 (BIP322-01..03)
- ✓ Removed coordinator P2WPKH-only registration gate at `coordinator/src/bitcoin/utxo.rs`; multi-script ownership-proof dispatcher accepts P2WPKH + P2TR + P2SH-P2WPKH with on-chain `ScriptType` cross-check (V1.4-CRIT-01 mitigation, statically unreachable via dispatcher-only public surface) — v1.4 Phase 16 (ADVERT-03)
- ✓ Coordinator publishes `supported_script_types` via PKARR record `v0.2.0` (`sst`/`ost` compact fields, 209-byte production-onion measurement, 11-byte headroom) and `/round/info` JSON array; clients reject mismatched coordinators via `DiscoveryError::UnsupportedScriptType` BEFORE opening any Tor circuit — v1.4 Phase 16 + Phase 17 (ADVERT-01, ADVERT-02, WALLET-03)
- ✓ Client signs BIP-322 ownership proofs for P2WPKH + P2TR + P2SH-P2WPKH via `BdkClientWallet::sign_bip322` (WIF → `shared::bip322::sign_simple`; descriptor → bdk_wallet 2.3 PSBT signer per Sprint-0-B PASS) — v1.4 Phase 17 (WALLET-01, WALLET-02)
- ✓ v1.3↔v1.4 backwards-compat shim — v1.4 client → v1.3 coordinator emits byte-identical v1.3 array-of-hex `OwnershipProof` via CD-7 two-phase try-parse; v1.3 client → v1.4 coordinator verified inline against pinned v1.3 binary SHA `05f21438` — v1.4 Phase 17 + Phase 18 (WALLET-04)
- ✓ Liquidity bot generates UTXOs across all enabled script types via `BLINDJOIN_BOT_SCRIPT_TYPES` CSV + per-round rotation counter file (defeats V1.4-MIN-02 uniform-script fingerprint) — v1.4 Phase 18 (INTEG-02)
- ✓ End-to-end mixed-script integration test: `mixed_script_e2e_three_clients_broadcast` runs 1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH input through INPUT_REG → BROADCAST on regtest; reuses `BitcoindGuard` + `require_bitcoind!()` from v1.3 unchanged — v1.4 Phase 18 (INTEG-01)
- ✓ Per-script positive property tests + 9 cross-shape rejection tests against the vendored official BIP-322 `basic-test-vectors.json` (upstream SHA `d77863fb9e` pinned) — v1.4 Phase 15 (BIP322-04)
- ✓ Production `sign` bodies for P2TR (BIP-341 Schnorr keypath via `sign_schnorr_no_aux_rand`) and P2SH-P2WPKH (BIP-143 ECDSA over unwrapped P2WPKH redeem) shipped in `shared::bip322`; D-111 spk↔key cross-check at the top of each new body (T-19-A defense-in-depth); byte-equality with `BdkClientWallet::sign_bip322` proven empirically in `client/tests/wallet_sign_roundtrip.rs` (T-19-C) — v1.5 Phase 19 (BIP322-05, BIP322-06)
- ✓ `sign_simple_test_only` + per-script `sign_for_tests` helpers removed; all callsites (`shared/tests/per_script_vectors.rs`, `tests/integration/multi_script_validate.rs`) migrated to the production `sign_simple` dispatcher; V1.4-CRIT-01 dispatcher-only invariant now load-bearing at the type level with no test-only hole — v1.5 Phase 19 (BIP322-07)
- ✓ Per-script BIP-141 witness weight table in `coordinator/src/bitcoin/tx.rs` (P2WPKH 68/31, P2TR 58/43 [round-UP per STATE.md], P2SH-P2WPKH 91/32) replaces the legacy P2WPKH-only `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES` constants; `ParticipantInput.script_type` plumbed coordinator-side from the existing `dispatch_ownership_proof` derivation through `UtxoDetails → RegisteredInput` (CRIT-01 invariant preserved into the fee path — zero new `detect_script_type` call sites); two regression tests pin v1.4 P2WPKH-only `fee_share == 266` byte-equal and prove the mixed-script branch fires with a ≥1 sat/participant delta (computed delta = 9 sats at fee_rate=2); v1.3 `full_round::*` 8/8 + v1.4 `mixed_script_e2e_three_clients_broadcast` both green with the new fee math — v1.5 Phase 20 (FEE-01, FEE-02, FEE-03)

- ✓ `docs/AUDIT-CHARTER.md` (574 LOC, 8 H2 sections: in-scope modules with file:symbol references, threat models per module, 9 cross-shape rejection properties enumerated, v=2 PSBT handling boundary, RSA SecretKey zeroization window in bounded form, out-of-scope with rationale, residual risks in 3 sub-buckets with explicit dispositions, glossary mapping ~25 active v1.4/v1.5 identifiers to audit language); hybrid voice (tables §1/§3/§6/§8, narrative §2/§4/§5/§7); linked from `README.md` §Security Model between Supply-chain hygiene and Test-infrastructure paragraphs — v1.5 Phase 21 (AUDIT-01)
- ✓ `.cargo/audit.toml` refreshed: each of the 3 RUSTSEC ignores (RUSTSEC-2023-0071 Marvin Attack, RUSTSEC-2025-0141 bincode unmaintained, RUSTSEC-2024-0436 paste unmaintained) appends `See docs/AUDIT-CHARTER.md#<anchor> for the full rationale.`; RUSTSEC-2023-0071 rationale rewritten to name the AUDIT-03 bounded-window mitigation explicitly (the "best-effort" framing is gone); `Reviewed: 2026-05-31` bumped to v1.5 ship date; `cargo audit` returns 0 vulnerabilities + 0 warnings — v1.5 Phase 21 (AUDIT-02)
- ✓ RSA SecretKey lifetime tightened from prose to structurally-bounded Rust type signature: `pub struct RoundSecretKey(BjSecretKey)` newtype in `coordinator/src/blind/rsa.rs` with empty-crypto-body `Drop` impl (PII-safe `tracing::debug!` only; transitive `rsa-0.9.10` `ZeroizeOnDrop` does the cryptographic work at the `RsaPrivateKey` Drop chain); `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` is the bounded lifetime expressible as a Rust type signature; SOLE FSM chokepoint is `transition_to(Phase::Idle)` (declared `state.rs:193`, chokepoint at `state.rs:202` inside validated-transition block 201-207, verified by grep — no other site sets `self.inner = None`); structural FSM test `round_secret_key_dropped_on_round_end` (load-bearing CI gate) + best-effort buffer-scrub test `round_secret_key_buffer_overwritten_on_drop` (CD-50 Linux-gated sanity check); D-07 comment at `coordinator/src/blind/rsa.rs` rewritten — the "best-effort" framing is gone, replaced by language citing the transitive `rsa::RsaPrivateKey` Drop chain + `Option<RsaBlindSigner>` type-signature bound + `transition_to(Phase::Idle)` trigger + charter §5 anchor; AUDIT-03 chokepoint result-discard pattern (`let _ =` on the 3 success-path FSM trigger sites) accepted as defense-in-depth gap and documented in AUDIT-CHARTER.md §7 Residual Risks: Protocol-level — v1.5 Phase 21 (AUDIT-03)

### Active

v1.6 supply-chain attestation requirements pending REQUIREMENTS.md definition during this `/gsd:new-milestone` cycle. Headline items already named in the milestone goal:
- cosign image attestations on ghcr.io images (OIDC-based)
- Detached signatures on GitHub Release tarballs
- Reproducible-build recipe + independent verification command
- Automated base-image digest drift check

### Out of Scope

- WabiSabi variable-amount credentials — no production Rust implementation exists yet
- PayJoin mode — post-v1 protocol extension
- Cross-coordinator rounds (multi-hop cascade) — post-v1, captured as future work
- Mobile client (iOS/Android) — CLI-first for v1
- Mainnet as default — signet-first, mainnet is a config flag
- OAuth, accounts, user management — no identity layer, no accounts
- Metrics dashboard (Prometheus/Grafana) — optional post-v1
- Offline mode — real-time coordination is fundamental to the protocol

## Carry-Forward Items (v1.6+ candidates)

Surfaced during v1.4/v1.5 development but intentionally not in scope:

- **CARRY-TOR-UAT** — Tor-mode verification harness (Phase 8 HUMAN-UAT item 3, deferred since v1.2)
- **CARRY-REPAIR-01-PR** — v1.3 REPAIR-01 PR observation closure (natural moment is the next external PR cut)
- **B-03** — Dynamic fee estimation (mempool-aware polling + RBF strategy; pre-mainnet requirement)
- **TEST-EXT-01/02/03** — Cross-implementation differential fixtures (via `ACken2/bip322-js`); on-chain anchor test; automated v1.3↔v1.4 backwards-compat integration matrix
- **P2WSH multisig BIP-322 support** — currently P2WPKH + P2TR + P2SH-P2WPKH
- **Mixed output script types** (Wasabi 2.0.3-style per-participant output choice)
- **Per-input variable `fee_share`** — uniform `total_fee / N` preserved in v1.5; per-input variance would change the wire protocol
- **AUDIT-03 chokepoint `let _ =` closure** — REVIEW.md CR-01 from v1.5 Phase 21, accepted as defense-in-depth gap and documented in AUDIT-CHARTER.md §7 Residual Risks: Protocol-level; closure trigger = any future FSM concurrency-model or `can_transition_to`-edge change

## Context

- **Gap in ecosystem:** zkSNACKs (Wasabi Wallet's coordinator) shut down CoinJoin service in June 2024. No standalone coordinator exists that anyone can run.
- **No Rust implementation:** Existing CoinJoin implementations are C# (Wasabi) and Python (JoinMarket). This is the first Rust coordinator.
- **PKARR as novel contribution:** The round protocol is battle-tested (Wasabi v1). The PKARR discovery layer making coordinators disposable and replaceable is the thesis.
- **Dependency maturity:** Arti 2.0.0 (Feb 2026) stabilized Tor hidden services in Rust. blind-rsa-signatures by jedisct1 is RFC 9474 compliant.
- **Builder background:** Owner has Rust + Bitcoin experience (arbstr-vault treasury service with Cashu ecash + Lightning).
- **v1.0 shipped:** Full protocol working on signet with Tor, PKARR discovery, Docker deployment, and CI/CD.
- **v1.1 shipped:** CI/CD security gates (test/clippy/audit on PRs and releases), coordinator DoS hardening (RPC before lock, RSA caching), supply-chain hardening (SHA-pinned actions, release checksums).
- **v1.2 shipped:** Public-endpoint DoS resistance via tower_governor + tower-http (per-route rate limits, request timeouts, Tor accept-loop semaphore cap), 4 operator-tunable config knobs validated at startup, release clearnet refusal.
- **v1.3 shipped:** Trustworthy integration-test feedback loop — pinned bitcoind v30.2 in CI (cached + PGP-verified), `BitcoindGuard` RAII + `require_bitcoind!()` macro eliminate `Box::leak` and the pipe-buffer stdout-hang, `CONTRIBUTING.md` documents the canonical invocation, corepc-node feature pin enforced by CI grep gate, REPAIR-01 closed locally (all 8 `full_round::*` tests green) via direct fixes for RSA SPKI handshake, bdk_wallet 2.3 segwit signing, partial-sig wire format, and coordinator witness_utxo correctness.
- **v1.4 shipped:** Multi-script BIP-322 coordinator+client — accepts P2WPKH + P2TR + P2SH-P2WPKH ownership proofs via a `match version` dispatcher with on-chain CRIT-01 cross-check (dispatcher-only public surface on `shared::bip322` makes spoofing statically unreachable at the type level); upstream `bip322 = "=0.0.10"` crate adopted via a 26-LOC zero-lossy adapter; PKARR record `v0.2.0` with B3 compact field names advertises `sst`/`ost` in 209 bytes (11-byte headroom under 220-byte threshold); fail-fast discovery rejects mismatched coordinators BEFORE opening any Tor circuit; v1.3↔v1.4 backwards-compat shim verified bidirectionally (CD-7 two-phase try-parse encoder + pinned v1.3 binary at SHA `05f21438`); mixed-script acceptance gate completes 1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH input through BROADCAST; liquidity bot rotates `script_types` per round. v1.3 `full_round::*` invariant held green at every v1.4 phase boundary.

## Constraints

- **Cryptography**: No custom crypto — blind-rsa-signatures (jedisct1), rust-bitcoin, bdk, secp256k1 only
- **Network**: Tor-native in production; development/testing may use clearnet TCP
- **Scope**: Signet-first; mainnet is a config flag, not a code change
- **Privacy**: No PII logging; round state zeroed after broadcast
- **License**: MIT — public good, not a business

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| RSA blind signatures (RFC 9474) over WabiSabi | WabiSabi lacks production Rust implementation; RSA blind sigs are proven (Wasabi v1) | ✓ Good — delivered full unlinkability |
| Approach B: Prove-Then-Layer build order | Get a txid on signet first, layer Tor/PKARR after; isolates protocol bugs from network bugs | ✓ Good — clean 5-phase progression |
| arti-client for native Tor hidden services | Arti 2.0.0 stable; avoids separate Tor process dependency | ✓ Good — working v3 .onion service |
| BIP-322 for UTXO ownership proofs | Forward compatible with all address types (P2WPKH, P2TR) | ✓ Good — Simple verification working |
| corepc-types over bitcoincore-rpc | bitcoincore-rpc archived Nov 2025; thin reqwest RPC client | ✓ Good — 5 RPC methods, no dep issues |
| bdk_wallet for client key management | Descriptor-based wallet, PSBT signing, HD derivation | ✓ Good — clean integration |
| PKARR for coordinator discovery | Decentralized; makes coordinators replaceable; no hardcoded addresses | ✓ Good — DHT publish + resolve working |
| In-process SOCKS5 proxy for client Tor | arti-client 0.41 has no launch_socks5_listener(); minimal RFC 1928 bridge | ⚠ Revisit — works but adds ~80 LOC |
| Docker Compose with cargo-chef builds | Multi-stage Dockerfiles, separate images per binary | ✓ Good — clean separation |
| Validate-then-lock for post_input | RPC before write lock eliminates DoS from slow bitcoind | ✓ Good — AVAIL-01 verified |
| Cache RsaBlindSigner in RoundStateInner | Parse RSA key once per round, not per request | ✓ Good — AVAIL-02 verified |
| SHA-pin all GitHub Actions | Immutable commit SHAs prevent supply-chain tampering | ✓ Good — CR-01 resolved |
| cargo audit deny high+critical only | Low/medium advisories are informational, not blockers | ✓ Good — reduces false-positive friction |
| tower_governor 0.8 + tower-http TimeoutLayer (over rolling your own) | Audited tower middleware; saved DoS-window correctness from scratch | ✓ Good — Phase 8 shipped clean |
| GlobalKeyExtractor over PeerIpKeyExtractor | Tor-safe by design; per-peer throttling impossible on Tor (single IP) | ✓ Good — would have been a CRITICAL bug |
| Validate hardening knobs at startup (`config.validate()`) | Fail-fast at boot beats panic-at-first-request or silent deadlock | ✓ Good — CR-01 & CR-02 fixed pre-ship |
| ConnectionGuard RAII for Tor permits | Load-bearing `let _permit = permit;` is one careless cleanup away from disabling the cap | ✓ Good — WR-01 hardened |
| Pin bitcoind v30.2 in CI via `.bitcoind-version` + actions/cache + PGP-verified install | Defeats keyserver flake and a hostile main HEAD; cache-then-verify-on-miss preserves the integrity gate even with cache hits | ✓ Good — TEST-01/02 closed, CI install cost amortized |
| `BitcoindGuard` RAII + `require_bitcoind!()` macro over Box::leak + skip blocks | Macro returns from the calling test scope (a plain fn cannot); RAII guarantees graceful `node.stop()` then process.kill() fallback | ✓ Good — `tests/integration/` tree has zero Box::leak and zero inline skip blocks |
| `view_stdout=false` + `-printtoconsole=0` belt-and-suspenders | view_stdout=false is corepc-node 0.12 default; explicit guards a future default flip silently re-introducing the pipe-hang | ✓ Good — root cause cannot resurface silently |
| corepc-node feature pin enforced by CI grep gate | Silent dependence on the 0_17_2 default would mean tests target the wrong Bitcoin Core RPC surface | ✓ Good — REPAIR-02 closed via 4026f50 |
| Close REPAIR-01 via direct commits rather than re-executing halted Plans 11-13 | After 6 orthogonal blockers and 3 escape-valve halts, Plan.md execution had ceased to be the load-bearing path; direct bisectable commits delivered REPAIR-01 with full forensic audit trail preserved | ⚠ Revisit — closed-local but full PR observation pending; preserve the forensic trace as a reference for "when to abandon the structured path" |
| client signs PSBT with `SignOptions { trust_witness_utxo: true }` (bdk_wallet 2.3) | Client populates witness_utxo from its own trusted regtest RPC; full BIP-143 non_witness_utxo path was heavier and unnecessary given trust boundary | ✓ Good — narrowly scoped fix with D-08 block comment explaining safety boundary |
| Partial-sig wire format = consensus-serialized `bitcoin::Witness` (not raw DER) | Coordinator deserializes via `bitcoin::Witness` consensus encoding; raw DER on the wire produced silent HTTP 400 | ✓ Good — wire-format mismatch was the root of the 6th orthogonal blocker |
| Coordinator uses real on-chain UTXO values in PSBT `witness_utxo` (not zero placeholders) | bdk_wallet sighash computation requires the actual amount; zero placeholders produced invalid signatures | ✓ Good — closed the last input-amount source of REPAIR-01 failures |
| ADOPT upstream `bip322 = "=0.0.10"` over extending custom impl (v1.4 ADR Decision #1) | Sprint-0-A returned GO across all 3 D-02 gates: bitcoin v0.32.8 transitive pin (no graph fork), cargo audit clean, 26-LOC zero-lossy adapter sketch; 3 new transitives (`bip322`, `snafu`, `snafu-derive`) accepted | ✓ Good — single source of truth, no custom sighash math; CI grep-gated `bip322-pin-check` enforces the exact pin |
| Mixed rounds (one queue for all 3 script types, single output script type per round) — v1.4 ADR Decision #2 | Segregated rounds fragment anonymity sets; mixed rounds preserve denomination uniformity; outputs single-type per round per operator config; no per-script-type min-participant gate; coordinator advertises supported set only, not per-round counts (exact partition vector would leak correlation) | ✓ Good — heterogeneous-input chain-analysis tradeoff documented in README §Privacy Considerations |
| B2 base64 PSBT-input wire shape with `version: u8` envelope (v1.4 ADR Decision #3) | v1.4 `OwnershipProof` carries `psbt_input_b64: String` + `version: u8` where 1 = v1.3 shape, 2 = v1.4 PSBT shape; wire-format roundtrip test ships FIRST per v1.3 REPAIR-01 lesson #1 | ✓ Good — CD-7 two-phase try-parse preserves bit-exact v1.3 compat; FULL BIP-174 PSBT shape on the wire (NOT bare `psbt::Input`) per RESEARCH Pitfall 1 |
| bdk path for P2TR sign (v1.4 ADR Decision #4) | Sprint-0-B PASS — bdk_wallet 2.3 produces a valid 64-byte Schnorr keypath witness for BIP-322 P2TR PSBTs; `secp256k1::verify_schnorr` returned `Ok(())`; D-15 80-LOC manual fallback retired for v1.4 | ✓ Good — uniform PSBT-sign path across all 3 descriptor types; v1.5 swap target if bdk regresses on taproot keyspend |
| Sprike-on-quarantined-branches gating pattern (Phase 14) | D-21 structural invariant: zero production-code commits from a gating ADR phase; spike branches `spike/14-A-*` + `spike/14-B-*` pushed to origin for reproducibility, NEVER merged | ✓ Good — pattern worth repeating when load-bearing decisions are unresolved at milestone start |
| Dispatcher-only public surface on `shared::bip322` (D-27) | Per-script `verify`/`sign` are `pub(crate)`-only; only `verify_simple` + `sign_simple` are `pub`; client-declared `script_type` cannot bypass dispatch | ✓ Good — V1.4-CRIT-01 spoofing vector statically unreachable at the type level (defense-in-depth alongside coordinator on-chain cross-check + 2 CI grep gates) |
| CRIT-01 cross-check derives `ScriptType` from on-chain `script_pubkey`, never from client declaration | Spoofing vector: client declares `p2wpkh` for a P2TR UTXO → coordinator's per-script sighash math diverges from actual chain | ✓ Good — enforced by `crit-01-grep-check` (coordinator) + `crit-01-client-grep-check` (client) CI jobs requiring ≥1 `CRIT-01` token in the relevant files |
| B3 PKARR compact field names (`v`/`sst`/`ost`) landed atomically with the `v0.2.0` schema bump (Phase 16-03) | 56-byte savings preserved 11 bytes of headroom under the 220-byte budget for the all-3 supported_types CSV (production-onion measured 209 bytes); a 4th script type in v1.5 will need a new encoding strategy | ✓ Good — CI byte-budget regression gate at `coordinator_packet_under_220_byte_budget_production_onion` |
| Literal BIP-84/86/49 descriptor templates with coin=0' across ALL networks (NOT bdk_wallet's `Bip84/86/49` helpers) | bdk_wallet's helpers auto-select coin=1' on testnet/signet, which would break v1.3 byte-equivalence and the cross-phase invariant (Pitfall 2) | ✓ Good — v1.3 `full_round::*` invariant held green at every v1.4 phase boundary |
| Pitfall-5 `#[serde(rename = "v")]` correction on v1.4 client `BlindjoinRecord` decoder (Phase 17-03) | Without the rename, every v1.4 coordinator would silently appear legacy on every connection — breaking WALLET-04 in the wrong direction. Caught load-bearing during plan execution | ✓ Good — preserves the v1.3↔v1.4 compat invariant; the wrong-direction failure would have been silent |
| `final_script_sig` on `Bip322SignedProof` (NOT just witness stack) for P2SH-P2WPKH | RESEARCH Pitfall 7: P2SH-P2WPKH ownership proofs require BOTH the witness AND the P2SH script_sig (the wrapper); witness alone fails coordinator-side reconstruction | ✓ Good — v=2 envelope carries final_script_sig in the PSBT; encoder/decoder roundtrip tested in `shared/tests/ownership_proof_roundtrip.rs` |
| WALLET-04 compat shim is one-way (v1.4 → v1.3); other direction verified by running an actual pinned v1.3 binary | TEST-EXT-03 (automated v1.3↔v1.4 compat grid) is long-term fix; v1.4 ships informal one-direction shim + Phase 18-03 pinned-binary verification for the other direction (SHA `05f21438`) | ⚠ Revisit at v1.5 — automated CI grid would catch silent compat regressions that pinned-binary verification only catches at ship time |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-06-01 — v1.5 milestone archived. v1.5 = 3/3 phases, 5/5 plans, 9/9 requirements, 100%. Project is between milestones; ready for v1.6 scoping or external audit launch.*
