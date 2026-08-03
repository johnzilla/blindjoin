# Milestones

## v1.5 Audit-Readiness & Multi-Script Finish (Shipped: 2026-06-01)

**Phases completed:** 3 phases (19, 20, 21), 5 plans, 12 tasks. ~22-hour wallclock from Phase 19 context capture (2026-05-30 22:40 EDT) to Phase 21 verification close (2026-05-31 20:40 EDT). 31 phase-tagged commits, 61 files changed (+12,824 / −293), workspace at 11,041 production Rust LOC.

**Goal:** Close the v1.4 follow-throughs (production sign bodies for P2TR + P2SH-P2WPKH, accurate fees for mixed-script rounds) and ready the codebase for external security audit.

**Key accomplishments:**

- **Phase 19 (BIP322-05/06/07):** Shipped production `sign` bodies for P2TR (BIP-341 Schnorr keypath via `sign_schnorr_no_aux_rand`) and P2SH-P2WPKH (BIP-143 ECDSA over unwrapped P2WPKH redeem) in `shared::bip322`; added the `p2sh_p2wpkh_final_script_sig` helper (23-byte BIP-141 nested-SegWit scriptSig); D-111 spk↔key cross-check at the top of each new sign body (defense-in-depth for charter T-19-A); byte-equality with `BdkClientWallet::sign_bip322` proven empirically in `client/tests/wallet_sign_roundtrip.rs`. Deleted `#[doc(hidden)] sign_simple_test_only` and per-script `sign_for_tests` helpers, migrating all 6 callsites to the production `sign_simple` dispatcher; `shared::bip322` public surface is now exactly 9 symbols, with V1.4-CRIT-01 dispatcher-only invariant load-bearing at the type level with zero test-only escape hatches.

- **Phase 20 (FEE-01/02/03):** Replaced the hardcoded P2WPKH-only `INPUT_WEIGHT_VBYTES = 68` / `OUTPUT_WEIGHT_VBYTES = 31` in `coordinator/src/bitcoin/tx.rs` with `script_input_vbytes(ScriptType)` / `script_output_vbytes(ScriptType)` `pub const fn` lookups (P2WPKH 68/31, P2TR 58/43 round-UP, P2SH-P2WPKH 91/32). Plumbed `ParticipantInput.script_type` coordinator-side from the existing `dispatch_ownership_proof` derivation through `UtxoDetails → RegisteredInput` (CRIT-01 invariant preserved into the fee path — zero new `detect_script_type` call sites). Two regression tests pin v1.4 P2WPKH-only `fee_share == 266` byte-equal (`fee_share_p2wpkh_only_matches_v14_baseline`) and prove the mixed-script branch fires with a 9-sats/participant delta at `fee_rate=2` (`fee_share_mixed_script_differs_from_uniform_baseline`).

- **Phase 21 (AUDIT-03):** Tightened the RSA SecretKey zeroization window from prose into a Rust type signature. Introduced `pub struct RoundSecretKey(BjSecretKey)` newtype in `coordinator/src/blind/rsa.rs` with empty-crypto-body `Drop` impl (PII-safe `tracing::debug!` only; transitive `rsa-0.9.10` `ZeroizeOnDrop` on `RsaPrivateKey` does the cryptographic work). Refactored `RoundStateInner.rsa_signer: RsaBlindSigner` → `Option<RsaBlindSigner>`; the bounded lifetime is expressible as a Rust type signature, and `transition_to(Phase::Idle)` at `state.rs:202` (inside the validated-transition block 201-207) is the SOLE FSM chokepoint that nulls the Option — verified by grep of the entire `coordinator/src/` tree. Structural FSM test `round_secret_key_dropped_on_round_end` (unconditional CI gate) + best-effort buffer-scrub test `round_secret_key_buffer_overwritten_on_drop` (CD-50 Linux-gated sanity check).

- **Phase 21 (AUDIT-01 + AUDIT-02):** Shipped `docs/AUDIT-CHARTER.md` (574 LOC, 8 H2 sections in the AUDIT-01 mandated order: in-scope modules with file:symbol refs, threat models per module, 9 cross-shape rejection properties, v=2 PSBT handling boundary, RSA SecretKey zeroization window in bounded form, out-of-scope dependencies, residual risks in 3 sub-buckets, glossary mapping ~25 active v1.4/v1.5 identifiers to plain audit language). Refreshed `.cargo/audit.toml` with charter-anchor refs and a RUSTSEC-2023-0071 rationale rewrite that explicitly names the AUDIT-03 bounded-window mitigation (the "best-effort" framing is gone). Added a one-paragraph README §Security Model callout linking to the charter. All 3 files landed in commit `92ae533` as ONE atomic commit per D-133a (prevents anchor-drift window between artifacts).

- **Code review CR-01 closure:** The v1.5 internal code review flagged that 3 production FSM trigger sites use `let _ = state.transition_to(...)` (discarding the FSM transition Result). Accepted as defense-in-depth gap per user disposition and documented in AUDIT-CHARTER.md §7 Residual Risks: Protocol-level with explicit closure-trigger language (any future FSM concurrency-model or `can_transition_to`-edge change MUST audit these 3 sites first). Closure deferred to v1.6+.

**Cross-phase invariants — all green at v1.5 close:**
- v1.3 `full_round::*` 8/8 (P2WPKH end-to-end suite, untouched since v1.3 REPAIR-01)
- v1.4 `mixed_script_e2e_three_clients_broadcast` 1/1 (1×P2WPKH + 1×P2TR + 1×P2SH-P2WPKH end-to-end)
- Phase 20 `fee_share` 2/2 (v1.4-baseline byte-equality + mixed-script delta sanity)
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo audit` 0 vulnerabilities + 0 warnings (3 intentional RUSTSEC ignores: rsa Marvin, bincode unmaintained, paste unmaintained)
- `shared/` and `client/` untouched in Phase 20/21 (V1.4-CRIT-01 dispatcher-only invariant structurally preserved)

**Known deferred items at close:** 3: 2 audit-open scanner false-positives (21-HUMAN-UAT.md flagged despite `status: resolved` with 0 pending scenarios; quick-task 260526-d7m-ci-hygiene-bump-rand-0-8-5-to-0-8-6-clos flagged as `missing` despite SUMMARY.md present — quick-task template lacks a `status:` frontmatter field) + the AUDIT-03 chokepoint `let _ =` closure (REVIEW.md CR-01, accepted as defense-in-depth gap per charter §7).

**Carry-forward to v1.6+:** CARRY-TOR-UAT (Tor-mode verification harness), CARRY-REPAIR-01-PR (v1.3 REPAIR-01 PR observation closure), B-03 (dynamic fee estimation, pre-mainnet), TEST-EXT-01/02/03 (cross-impl differential fixtures + on-chain anchor test + v1.3↔v1.4 backwards-compat CI matrix), P2WSH multisig BIP-322, Wasabi 2.0.3-style mixed output scripts, per-input variable `fee_share`, and AUDIT-03 `let _ =` closure.

---

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
