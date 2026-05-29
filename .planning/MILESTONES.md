# Milestones

## v1.3 Test Infrastructure & Operational Hardening (Shipped: 2026-05-29)

**Phases completed:** 5 phases (9-13), 13 plans, ~4-day milestone
**Code diff (ex-planning):** 24 files changed, +1,493 / -739 across coordinator, client, shared, CI
**Notes:** No pre-close milestone audit (close approved without it). REPAIR-01 closed locally — all 8 `full_round::*` tests green on pinned brew bitcoind v31; full PR observation closure deferred to v1.4 cut.

**Key accomplishments:**

- CI substrate for integration tests — `.bitcoind-version` pin (Bitcoin Core v30.2), `actions/cache@v4` with PGP+SHA256-verified install on miss, `BITCOIND_EXE` exported, workflow-level `BLINDJOIN_REQUIRE_BITCOIND=1`; tests now panic-on-miss instead of silent graceful-skip (TEST-01, TEST-02)
- Shared test fixtures + clean process lifecycle — `require_bitcoind!()` macro, `BitcoindGuard` RAII, `RpcCreds`, `bootstrap_regtest_bitcoind()`; entire `tests/integration/` tree now has **zero `Box::leak`** and zero inline skip blocks; `view_stdout=false` + `-printtoconsole=0` kill the pipe-buffer hang (TEST-03, TEST-04)
- `CONTRIBUTING.md` canonical pattern — 61-line local-dev manual with copy-pasteable invocation, single-test example, `--include-ignored` opt-in, and 4-row pass/fail/skip verdict reference card (TEST-05)
- REPAIR-02 corepc-node feature pin CI gate — `grep` job rejects any `Cargo.toml` declaring `corepc-node` without an explicit `features = ["NN_M"]`; doc count corrected 15→8 tests across ROADMAP+REQUIREMENTS; 4 WR-05 bare sleeps replaced with bounded poll-until-deadline loops (REPAIR-02 closed via commit `4026f50`)
- REPAIR-01 closed-local — all 8 `full_round::*` tests green via a chain of direct fixes: client RSA pubkey decode `from_der` → `from_spki` (SPKI-symmetric handshake); client signs PSBT with `SignOptions { trust_witness_utxo: true }` (bdk_wallet 2.3 segwit); partial-sig wire format = consensus-serialized `bitcoin::Witness`; coordinator uses real on-chain UTXO values in PSBT `witness_utxo`; ban-check ordered before blinded-token validation; coordinator error body surfaced in client error path
- Test-infra & CLI hygiene — 2 MEDIUM test backdoors removed and replaced with the production state-machine path; dead `--utxo-value-sats` CLI flag dropped; `--generate-wallet` placeholder documented; planning state reconciled with shipped reality (Phase 11-13 directories preserved as forensic audit log under `.planning/milestones/v1.3-phases/`)

**Known gaps recorded at close:**
- REPAIR-01 PR observation closure still pending — closed-local only. Full closure expected at v1.4 cut PR.
- Phase 11-13 Plan.md executions halted under D-08/D-11/D-12 escape-valves; the actual fixes shipped as direct code commits rather than Plan execution. Original execution trace preserved as forensic audit log.

---

## v1.2 Production Readiness (Shipped: 2026-05-26)

**Phases completed:** 1 phase (Phase 8), 4 plans

**Key accomplishments:**

- Per-route rate limiting on coordinator HTTP API via `tower_governor` 0.8 (60/min read split, 30/min write split, `GlobalKeyExtractor` for Tor-safety) returning HTTP 429 + `Retry-After` + `RATE_LIMITED` JSON envelope
- Uniform request timeout via `tower_http::TimeoutLayer` returning HTTP 408 honoring `request_timeout_secs`
- Tor accept-loop connection cap via `tokio::sync::Semaphore` (default 256) wrapped in `ConnectionGuard` RAII; load-bearing `let _permit = permit;` pattern documented
- 4 operator-tunable knobs in `[coordinator]` config (`coordinator.toml` + `BLINDJOIN__COORDINATOR__*` env vars) all validated at startup via `CoordinatorConfig::validate()`
- Release builds refuse to bind clearnet unless `BLINDJOIN_ALLOW_CLEARNET=1` is explicitly set
- 11/11 must-haves verified statically; HUMAN-UAT items 1 & 2 closed via local runtime proof (Homebrew bitcoind v31 + integration tests); item 3 (Tor-mode connection-cap runtime test) deferred to v1.4+

---

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
