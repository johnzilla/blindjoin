# Milestones

## v1.4 BIP-322 Multi-Script Support (Shipped: 2026-05-31)

**Phases completed:** 5 phases (14-18), 15 plans, ~3-day milestone
**Code diff (ex-planning):** 48 files changed, +8,409 / -843 across coordinator, client, shared, liquidity-bot, tests, CI
**Notes:** No pre-close milestone audit (close approved without it, matching the v1.3 convention). All 14 v1.4 requirements validated; 0 known gaps. v1.3 P2WPKH-only `full_round::*` integration invariant held green at every phase boundary (8/8 PASS). v1.3 → v1.4 backwards-compat verified inline against a pinned v1.3 binary at SHA `05f21438`.

**Key accomplishments:**

- **Eliminated the `is_p2wpkh()` registration gate** — coordinator now accepts P2WPKH + P2TR + P2SH-P2WPKH ownership proofs via a `match version { 1 => v1_path, 2 => v2_path }` dispatcher that derives `ScriptType` from on-chain `script_pubkey` and cross-checks against the client-declared type (V1.4-CRIT-01 spoofing mitigation; load-bearing, CI grep-gated on both client AND coordinator side via the `crit-01-grep-check` jobs)
- **`shared::bip322` multi-script verifier/signer** — adopted `bip322 = "=0.0.10"` crate (Sprint-0-A GO) via a 26-LOC zero-lossy adapter behind a dispatcher-only public surface (makes V1.4-CRIT-01 spoofing **statically unreachable** at the type level); per-script positive vectors + 9 cross-shape rejection tests against the vendored official BIP-322 fixtures (upstream SHA `d77863fb9e` pinned); 10-variant `Bip322Error` taxonomy; `bip322-pin-check` CI grep gate; auto-fixed 2 latent v1.3 bugs in `build_bip322_to_sign` (`Version::TWO` should be `Version(0)`; `ScriptBuf::new_op_return([])` should be bare `OP_RETURN`)
- **Coordinator advertisement** — `BipConfig { allow_p2wpkh, allow_p2tr, allow_p2sh_p2wpkh, output_script_type }` validated fail-fast at boot via `CoordinatorConfig::validate()` (rejects all-false; rejects output_script_type outside the allowed set); PKARR record bumped `0.1.0 → 0.2.0` with B3 compact field names (`v`/`sst`/`ost`) and a CI byte-budget regression gate at **209 bytes** with a 62-byte real Tor v3 `.onion` (11-byte headroom under the 220-byte threshold); `/round/info` exposes `supported_script_types` as a JSON array with `#[serde(default)]` on both ends for v1.3↔v1.4 bidirectional compat
- **Client multi-script wallet + fail-fast discovery** — `--type {p2wpkh|p2tr|p2sh-p2wpkh}` CLI flag emits **literal** BIP-84/86/49 descriptor templates with coin=0' across all networks per RESEARCH Pitfall 2 (bdk_wallet's `Bip84/86/49` helpers would auto-select coin=1' on testnet/signet and break v1.3 byte-equivalence); `BdkClientWallet::sign_bip322` routes WIF wallets through `shared::bip322::sign_simple` and descriptor wallets through bdk_wallet 2.3's PSBT signer per Sprint-0-B PASS (bdk path adopted; D-15 80-LOC manual fallback retired for v1.4); `discover_coordinator(pkarr_pubkey, required_script_type)` rejects mismatched coordinators with `UnsupportedScriptType` *before* opening any Tor circuit (V1.4-MOD-03 fail-fast)
- **WALLET-04 v1.3↔v1.4 compatibility shim** — v1.4 client detects pre-`0.2.0` PKARR / missing `/round/info` field via `CoordinatorInfo.capabilities.is_legacy` and emits the legacy witness-only `OwnershipProof` array-of-hex wire format (CD-7 two-phase try-parse preserves bit-exact v1.3 compat); critical Pitfall 5 correction caught in Phase 17-03: without `#[serde(rename = "v")]` on the v1.4 client decoder, every v1.4 coordinator would silently appear legacy on every connection — breaking WALLET-04 in the wrong direction. Other direction (v1.3 client → v1.4 coordinator) verified inline against a pinned v1.3 binary (SHA `05f21438`)
- **Mixed-script E2E acceptance gate + liquidity bot multi-script** — `mixed_script_e2e_three_clients_broadcast` runs 1× P2WPKH + 1× P2TR + 1× P2SH-P2WPKH input through INPUT_REG → BROADCAST in a single `cargo test` run (reuses `BitcoindGuard` + `require_bitcoind!()` unchanged from v1.3, zero `Box::leak` in new test files); liquidity bot rotates `script_types` per round via `BLINDJOIN_BOT_SCRIPT_TYPES` CSV + persistent rotation counter file (defeats V1.4-MIN-02 uniform-script fingerprint); README §Privacy Considerations documents the chain-analysis tradeoff of mixed-script rounds (Phase 14 CD-3 carry-forward)

**Known gaps recorded at close:**
- TEST-EXT-03 (automated v1.3↔v1.4 backwards-compat integration matrix) deferred — WALLET-04 covers v1.4→v1.3 informally + Phase 18-03 verifies v1.3→v1.4 against a pinned binary, but no automated grid in v1.4.
- CARRY-REPAIR-01-PR (v1.3 REPAIR-01 PR observation closure) still pending — the v1.4 cut PR is the natural moment to discharge this but is NOT a v1.4 code deliverable per REPAIR-01 lesson #5.
- **Resolved at close:** 14 pre-existing clippy lints in `shared/src/bip322/*` (12× `result_large_err` + 2× `unnecessary_to_owned`) AND `coordinator::validate_utxo` `too_many_arguments` were fixed at the milestone-cut boundary so the `v1.4.0` tag push could clear the pre-push hook (`cargo clippy --workspace -- -D warnings`). Fixes: `Box<bip322::Error>` source, dropped redundant `.to_vec()` calls (`Witness::push` accepts `AsRef<[u8]>`), and `#[allow(clippy::too_many_arguments)]` on the CRIT-01 validator.

---

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
