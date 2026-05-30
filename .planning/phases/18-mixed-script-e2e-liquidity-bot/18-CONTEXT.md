# Phase 18: Mixed-Script E2E + Liquidity Bot - Context

**Gathered:** 2026-05-30
**Status:** Ready for planning
**Mode:** --auto (autonomous decisions per recommended defaults; reviewable in this file)

<domain>
## Phase Boundary

Phase 18 is the v1.4 milestone **acceptance gate**. It exercises every prior v1.4 deliverable end-to-end on regtest and unblocks the v1.4 milestone cut. Two requirements:

1. **INTEG-01 — Mixed-script E2E test on regtest.** A new integration test where ≥1 P2WPKH + ≥1 P2TR + ≥1 P2SH-P2WPKH client each register a typed UTXO, complete INPUT_REG → OUTPUT_REG → SIGNING, and the broadcast txid appears in the regtest mempool. Reuses `BitcoindGuard` + `require_bitcoind!()` + `fund_regtest_typed` from v1.3/Phase 16 unchanged. v1.3 P2WPKH-only `tests/integration/full_round.rs` remains green at this phase boundary (cross-phase invariant — REPAIR-01 rollback safety net).
2. **INTEG-02 — Liquidity bot multi-script + per-round rotation.** Bot accepts a new CSV env var `BLINDJOIN_BOT_SCRIPT_TYPES` enumerating enabled types. Bot rotates the script type used per round so its registrations are not a uniform-script fingerprint (V1.4-MIN-02 mitigation). Per-run keychain derivation (fresh wallet per single-shot run → fresh index-0 output address) preserves output non-clustering.

The phase also ships two ancillary deliverables that close out the v1.4 milestone:

3. **v1.3-client ↔ v1.4-coordinator compatibility gate (ROADMAP Phase 18 success criterion #5).** A v1.3 client BINARY (built from a pinned pre-Phase-14 commit SHA) registers a P2WPKH UTXO against the v1.4 coordinator and the round completes — discharges the WALLET-04 compat shim against a real v1.3 build artifact. Phase 17 17-03 already proved the shim's wire-shape correctness with a STUBBED v1.3 record; this is the binary-level integration gate Phase 17 D-79 promised.
4. **README §"Privacy Considerations" prose (Phase 14 CD-3 carry-forward).** Two paragraphs in `README.md` documenting the mixed-input chain-analysis fingerprint (V1.4-MOD-06) and the liquidity-bot per-round rotation mitigation (V1.4-MIN-02). Honest disclaimer; plain language; no scary uppercase.

**Requirements mapped to this phase** (per `.planning/REQUIREMENTS.md §Traceability`): INTEG-01, INTEG-02.

**Not in scope:**
- Coordinator runtime enforcement that registered output addresses match the advertised `ost` (the coordinator currently parses+stores output addresses without script-type check at `coordinator/src/api/handlers.rs:347-413`; per Phase 16 D-07 + Phase 17 D-76 this is enforced CLIENT-SIDE at discovery via `DiscoveryError::UnsupportedOutputScriptType`, NOT at register_output runtime). Phase 18 reuses the existing behaviour. A runtime gate is a v1.5+ candidate.
- Bot HD wallet auto-funding / scan-bitcoind-for-spendable-UTXOs (H2 path below — deferred to v1.5).
- Mainnet config flip (signet-first per PROJECT.md constraints; mainnet remains a one-line `BLINDJOIN__NETWORK__BITCOIN_NETWORK` change, NOT a code change).
- BIP-322 cross-implementation differential test fixtures (TEST-EXT-01 — v1.5+ per REQUIREMENTS Future Requirements).
- On-chain BIP-322 anchor test (TEST-EXT-02 — v1.5+).
- Automated full backwards-compat matrix (TEST-EXT-03 — v1.5+; Phase 18 covers only the v1.3↔v1.4 cell that ROADMAP success criterion #5 names).
- REPAIR-01 PR observation closure (CARRY-REPAIR-01-PR — per PROJECT.md "the v1.4 cut PR is the natural moment to discharge this but is NOT a v1.4 code deliverable per REPAIR-01 lesson #5"). The v1.4 cut PR opens AFTER Phase 18 completes; Phase 18 itself does not touch REPAIR-01 observation.
- Tor-mode verification harness (CARRY-TOR-UAT — Phase 8 HUMAN-UAT item 3 carry-forward to v1.5+).
- P2WSH multisig (Future Requirements — v1.5+).
- B-03 dynamic fee estimation (v1.5+).

**Boundary-only changes in this phase:**
- `tests/integration/mixed_script_e2e.rs` — NEW file (INTEG-01).
- `tests/integration/mod.rs` — extend `mod` declarations with `mixed_script_e2e`; no fixture changes (reuses `fund_regtest_typed` + `BitcoindGuard` + `require_bitcoind!()` unchanged).
- `liquidity-bot/src/main.rs` — extend env-var surface for `BLINDJOIN_BOT_SCRIPT_TYPES` + per-type tuples (INTEG-02 18-02 work); replace single-WIF-only construction with type-dispatched descriptor-or-WIF wallet build.
- `liquidity-bot/src/strategy.rs` — add `pub struct RotationCounter` (or equivalent) wrapping the atomic-file-backed counter; add `JoinStrategy::pick_script_type(&[ScriptType]) -> ScriptType`; existing `should_join` shape unchanged.
- `liquidity-bot/Cargo.toml` — may add `tempfile` (test-only) for rotation-counter unit tests; no runtime dep additions.
- `docker/docker-compose.yml` — extend `liquidity-bot.environment` block with the new env vars (default values preserve v1.3 single-WIF-P2WPKH behaviour).
- `docker/Dockerfile` — extend the `liquidity-bot` stage with `/app/data` volume for the rotation-counter file (mirrors coordinator's `/app/data/ban_list.jsonl` pattern).
- `.env.example` — add the new bot env vars with comments.
- `tests/v13_compat/` (or `tests/integration/v13_binary_compat.rs` — plan-phase decides) — NEW build infra + test for v1.3-binary gate (INTEG-01 success criterion #5). Plan-phase decides whether this is fully automated (cargo-build-from-pinned-SHA) or documented UAT in `18-VERIFICATION.md`.
- `README.md` — add §"Privacy Considerations" section (~ 2 paragraphs; Phase 14 CD-3 prose).
- NO changes to: `tests/integration/full_round.rs` (v1.3 invariant gate); `coordinator/**` (no runtime behaviour changes — `BipConfig::default()` already permits all 3 types); `shared/**` (the BIP-322 contract is closed in Phase 15); `client/src/wallet.rs`, `client/src/round/**`, `client/src/discover.rs`, `client/src/config.rs` (Phase 17 surface is closed and consumed verbatim).

**Cross-phase invariant (carries to every v1.4 phase boundary, including this one):** v1.3 P2WPKH-only `tests/integration/full_round.rs` MUST remain green at this phase boundary. Phase 18 makes NO changes to `full_round.rs` — its 8 tests should pass identically (42.23s wall-clock per Phase 17 verification baseline). If `full_round` goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phase 14 ADR + Phases 15/16/17 (NOT re-asked)

LOCKED upstream. Plan-phase consumes verbatim — no re-litigation.

- **ADR #2 / Phase 14 D-06:** MIXED rounds. Coordinator's per-script dispatcher accepts heterogeneous inputs in one round. Phase 18 INTEG-01 demonstrates this end-to-end with 3 distinct input types.
- **Phase 14 D-07:** Single output script type per round, operator-configured via `BipConfig::output_script_type` ("ost"). Phase 18 does NOT add a runtime coordinator gate on submitted output addresses — the existing client-side fail-fast at `client::discover::discover_coordinator` (Phase 17 D-76, `UnsupportedOutputScriptType`) is the canonical enforcement. The integration tests bypass discovery (per the existing `v14_p2wpkh_coordinator_info()` helper at `tests/integration/full_round.rs:49-59`) so they can drive heterogeneous outputs through the coordinator unchanged.
- **Phase 14 D-08:** Heterogeneous-input chain-analysis fingerprint (V1.4-MOD-06) is a KNOWN LIMITATION; documented in README §"Privacy Considerations" per Phase 14 CD-3 → Phase 18 deliverable (this phase).
- **Phase 14 D-10 / CRIT-01:** Client declares `script_type` on v=2 OwnershipProof; coordinator cross-checks against derived. Already enforced in production at `coordinator/src/bitcoin/utxo.rs` (Phase 16) and `client/src/round/input.rs::register_input` (Phase 17). Phase 18 reuses verbatim — no new mitigations.
- **Phase 15 LOCKED API:** `shared::bip322::{ScriptType, Bip322Error, detect_script_type, verify_simple, sign_simple, sign_simple_test_only, bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign}`. Phase 18 integration tests consume `sign_simple_test_only` only for raw-key signing in `multi_script_validate.rs`-style assertions (already used at `tests/integration/multi_script_validate.rs:23`); the mixed-script E2E test consumes the production `client::wallet::BdkClientWallet::sign_bip322` path (Phase 17 17-02 contract).
- **Phase 15 wire shape:** `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` flat struct + CD-7 byte-identity branch for v=1. Phase 18 v1.3-binary gate exercises the CD-7 branch as the WALLET-04 acceptance gate.
- **Phase 16 PKARR record + `/round/info` shape:** `version: "0.2.0"`, `sst` (CSV), `ost` (scalar); InfoResponse `supported_script_types` + `output_script_type` with `#[serde(default)]` legacy fallbacks. Phase 18 mixed-script test reuses Phase 17's `v14_*_coordinator_info()` helper pattern to bypass discovery; the v1.3-binary gate connects to the v1.4 coordinator over HTTP (not Tor; this is regtest infra) and exercises the InfoResponse legacy-defaults path implicitly (v1.3 client decoders read the new fields via `#[serde(default)]`).
- **Phase 16 `BipConfig::default()`:** `allow_p2wpkh = allow_p2tr = allow_p2sh_p2wpkh = true`, `output_script_type = P2wpkh`. Phase 18 mixed-script E2E coordinator uses this default unchanged — no per-test `BipConfig` overrides needed.
- **Phase 16 `fund_regtest_typed`:** Already in place at `tests/integration/mod.rs:617-823` accepting `&[(ScriptType, usize)]` and returning `FundedTypedSetup { utxos: Vec<TypedUtxoHandle> }`. Phase 18 INTEG-01 reuses verbatim for the MIXED-input requirement: `fund_regtest_typed(exe, &[(P2wpkh, 1), (P2tr, 1), (P2shP2wpkh, 1)]).await`.
- **Phase 17 D-61:** `BdkClientWallet::from_wif` stays P2WPKH-only. Phase 18 bot's P2TR + P2SH-P2WPKH paths MUST go through `BdkClientWallet::from_descriptor` (descriptor mode), NOT through an extended `from_wif`.
- **Phase 17 D-78 / D-79:** Phase 17 verified WALLET-04 against a STUBBED v1.3 PKARR record. Phase 18 verifies WALLET-04 against a REAL v1.3 BINARY. Clean phase boundary; no duplication of test scope.
- **Cross-phase invariant:** `cargo test --test integration full_round` MUST remain GREEN at every plan boundary in Phase 18.

### A. Mixed-script E2E test file location

- **D-81:** **NEW file `tests/integration/mixed_script_e2e.rs`** (sibling to `tests/integration/full_round.rs` and `tests/integration/multi_script_validate.rs`). Add `mod mixed_script_e2e;` to `tests/integration/mod.rs:19-24` block (alphabetical sort: `ban_list_persistence`, `full_round`, **`mixed_script_e2e`**, `multi_script_client`, `multi_script_validate`, `rate_limiting`, `round_bootstrap`). **Rationale:** keeps `full_round.rs` bit-exactly untouched (cross-phase invariant gate); mirrors Phase 16's `multi_script_validate.rs` and Phase 17's `multi_script_client.rs` per-domain isolation; the new file's structure mirrors `full_round_three_clients` exactly with 3 differences (typed funding, per-script-type descriptor wallets, post-broadcast denomination-output assertion expects 3 outputs of denomination_sats with heterogeneous script types).
- **D-82:** **Test fn name `mixed_script_e2e_three_clients` (or `full_round_mixed_script`)** — plan-phase discretion on the exact name; the public-visible behaviour is: `#[tokio::test]` async fn that on success prints `eprintln!("MIXED-SCRIPT integration test PASSED: ...")` with the broadcast txid. Recommended: `mixed_script_e2e_three_clients_broadcast` (the verb is the load-bearing assertion).

### B. Funding strategy for mixed-script UTXOs

- **D-83:** **Two-stage funding** for the mixed-script E2E test:
  - **Stage 1 (raw-key UTXOs for INPUT-side ownership proofs):** Call `crate::fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1), (ScriptType::P2tr, 1), (ScriptType::P2shP2wpkh, 1)]).await`. Returns 3 `TypedUtxoHandle { secret_key, outpoint, script_pubkey, value_sats, p2sh_redeem_script }` per the existing struct shape at `tests/integration/mod.rs:565-580`.
  - **Stage 2 (per-client `BdkClientWallet`):** For each TypedUtxoHandle:
    - **P2WPKH (TypedUtxoHandle[0]):** `BdkClientWallet::from_wif(wif_from_secret_key, &outpoint_str, Network::Regtest)` where `wif_from_secret_key = PrivateKey::new(handle.secret_key, Network::Regtest).to_wif()`. Existing v1.3 single-key path (Phase 17 D-61 unchanged).
    - **P2TR (TypedUtxoHandle[1]):** Phase 17 leaves single-key descriptor wallets unmodelled (descriptors require a tprv root, not a raw `secp256k1::SecretKey`). Plan-phase decides between two paths:
      - **B1.a (recommended for plan minimality):** Wrap the raw P2TR SecretKey in a `tr(xprv/86'/...)` descriptor by constructing a `bitcoin::bip32::Xpriv` from a deterministic seed and `bdk_wallet::Wallet::create(...)` with the matching descriptor. This requires fabricating an xprv whose first external-keychain key matches `TypedUtxoHandle.secret_key` — which is structurally NOT possible (xprv derivation is one-way). So this path requires re-funding via the descriptor's `peek_address(External, 0)` instead of consuming the raw-key UTXO.
      - **B1.b (recommended for B1.a-blocked):** **Stage 2 generates a fresh descriptor wallet per type** via `BdkClientWallet::generate(dummy_outpoint, Network::Regtest, script_type)?`, derives `wallet.coinjoin_output_address()` (which is `peek_address(External, 0)`), funds THAT address via the bitcoind RPC `send_to_address` + vout discovery, then **overrides** `wallet.utxo_outpoint` to the freshly funded outpoint. The raw-key `fund_regtest_typed` stage is then unnecessary for P2TR + P2SH-P2WPKH. RECOMMENDED.
    - **P2SH-P2WPKH:** Same as P2TR (B1.b path).
  - **Plan-phase consolidation guidance:** if B1.b is taken (recommended), the mixed-script E2E test does NOT call `fund_regtest_typed` — it calls `bootstrap_regtest_bitcoind(exe)` directly, generates 3 descriptor wallets via `BdkClientWallet::generate`, funds each wallet's external-index-0 address via a new helper or inline `send_to_address` + `get_raw_transaction_verbose` vout walk (mirrors the inner body of `fund_regtest` at `tests/integration/mod.rs:413-535`), and proceeds. **NOTE the apparent tension with Phase 18 ROADMAP success criterion #2 ("reuses BitcoindGuard + require_bitcoind!() unchanged from v1.3"):** that criterion names `BitcoindGuard` + the macro, NOT `fund_regtest` specifically. Both `fund_regtest_typed` and a B1.b-style inline approach satisfy the criterion via `bootstrap_regtest_bitcoind` (which is the BitcoindGuard producer). Plan-phase records this rationale in 18-01-PLAN.md to forestall lint-review pushback.
- **D-84:** **Wallet construction discipline:** P2WPKH client uses `BdkClientWallet::from_wif` (legacy WIF path, byte-exact match with v1.3 client behaviour, exercises the `shared::bip322::sign_simple(P2wpkh, ...)` path per Phase 17 D-65). P2TR + P2SH-P2WPKH clients use `BdkClientWallet::generate` then `from_descriptor` for `utxo_outpoint` override — exercising the bdk PSBT sign path per Phase 17 D-65 P2TR + D-65 P2SH-P2WPKH bodies. The 3-client mix thus covers BOTH the WIF code path (P2WPKH) AND the descriptor code path (P2TR + P2SH-P2WPKH) in a single test, mirroring real-world deployment.
- **D-85:** **Synthetic `CoordinatorInfo` per client** in the mixed-script E2E test (matches Phase 17's `v14_p2wpkh_coordinator_info()` pattern at `tests/integration/full_round.rs:49-59`). For each client, the synthetic `CoordinatorInfo.capabilities.supported_script_types` MUST include the client's own `wallet.script_type()` (else the test fails Phase 17 D-72's resolver fail-fast), AND `output_script_type = wallet.script_type()` so the Phase 17 D-76 mismatch check passes. **Note:** in production, all 3 clients would discover the SAME coordinator and see the SAME advertised `output_script_type` (e.g., `P2wpkh`); a P2TR-wallet client would then fail discovery against a P2wpkh-ost coordinator. The mixed-script E2E test bypasses this discovery layer (synthetic CoordinatorInfo per client) to drive the coordinator's per-script dispatcher directly. This is consistent with `tests/integration/full_round.rs`'s test pattern (it ALSO uses a synthetic CoordinatorInfo). The test exercises the coordinator's MIXED-input acceptance + per-script verify dispatcher; the heterogeneous-output property is a known limitation per V1.4-MOD-06 (Phase 14 D-08).

### C. v1.3-client binary compatibility gate (success criterion #5)

- **D-86:** **Automated test target: build v1.3 from a pinned commit SHA.** A new integration test (`tests/integration/v13_binary_compat.rs` or similar — plan-phase decides) that:
  1. **At build time:** identifies the pinned v1.3 commit SHA — the last `main` commit BEFORE the first Phase 14 commit (~ `e423beb`/`b2b9773`-ancestor; plan-phase resolves to the exact SHA via `git log --format=%H .planning/decisions/v1.4-adr.md | tail -1` then bumps back one commit, OR via `git merge-base HEAD <phase-13-closeout-commit>`). Records the SHA in a `.planning/phases/18-.../v13_pinned_sha.txt` file checked into the repo.
  2. **At test runtime:** uses `git worktree add /tmp/blindjoin-v13-<sha> <sha>` to materialize the v1.3 source in a side path; `cargo build --release --bin client --manifest-path /tmp/blindjoin-v13-<sha>/client/Cargo.toml` to produce the v1.3 binary; caches the binary at `/tmp/blindjoin-v13-<sha>-bin/client` for subsequent test invocations (idempotent — first run takes ~30s, subsequent runs are ~0s).
  3. **Drives the binary** via `tokio::process::Command::new("/tmp/blindjoin-v13-<sha>-bin/client")` with `--utxo-wif`, `--coordinator-url http://...` (regtest in-process v1.4 coordinator launched via `spawn_coordinator` from `full_round.rs`), no `--use-tor` (regtest infra). Asserts: child exit code 0, round broadcast txid appears in mempool, coordinator log includes `"ownership_proof verified script_type=p2wpkh"` line.
  4. **Skip discipline:** Same as other regtest integration tests — `require_bitcoind!()` first, gracefully skips when bitcoind absent in local-dev mode.
- **D-87:** **Fallback path if D-86 proves heavy in plan-phase:** documented UAT in `18-VERIFICATION.md` listing the exact reproducible steps (build v1.3 from SHA, run against v1.4 coordinator, observe success). The fallback IS structurally valid per ROADMAP wording — "verified inline" can mean "verified in the Phase 18 acceptance run", whether automated or manually executed once. Plan-phase decides D-86 vs D-87 based on the v1.3 build-time-and-disk budget vs Phase 18's overall plan-count constraint. RECOMMENDED: D-86 automated, with D-87 as escape valve.
- **D-88:** **Pinned v1.3 SHA identification convention:** the SHA file at `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt` contains exactly one line: the 40-char SHA. A second line (optional, plan-phase) names the commit-message subject for human readability. Committed alongside 18-01-PLAN.md so the gate is reproducible by anyone checking out the repo at any future point.

### D. Coordinator config for the mixed-script test

- **D-89:** **`BipConfig::default()`** (all-allowed + `output_script_type = P2wpkh`) — already validated by Phase 16 `BipConfig::default()` smoke tests + Phase 17 `full_round.rs` carry-forward; no test-specific override needed. The coordinator accepts P2WPKH + P2TR + P2SH-P2WPKH inputs in one round, dispatches per-script verify via `coordinator::bitcoin::utxo::validate_utxo`, and writes a single broadcast txid containing 3 heterogeneous-input + 3 (potentially heterogeneous-output, see D-85) denomination outputs to the regtest mempool.
- **D-90:** **No runtime check on submitted output script type.** The coordinator's `register_output` handler at `coordinator/src/api/handlers.rs:296-440` parses + stores the output address WITHOUT cross-checking against `state.config.bip.output_script_type`. Phase 18 does NOT add such a check — adding it would break the mixed-script E2E test (heterogeneous outputs by design) AND would require a follow-up to make the existing client-side fail-fast (Phase 17 D-76) redundant. The mismatch between advertised `ost` and submitted output script type is a CLIENT-side soft enforcement (discovery layer); runtime enforcement is a v1.5+ candidate captured in Deferred below.

### E. Cross-phase invariant verification

- **D-91:** **Run `cargo test --test integration full_round` after each Phase 18 plan lands.** Document in `18-VERIFICATION.md` (carries from Phase 14/15/16/17 verification convention). Expected baseline: 8 passed, 0 failed, ~42s wall-clock (per Phase 17 verification baseline). Any drift in pass count or any new test failure is a Phase 18 BLOCKER and triggers REPAIR-01 lesson #4 (`/gsd:debug` pivot).

### F. Liquidity bot config field for enabled script types

- **D-92:** **New CSV env var `BLINDJOIN_BOT_SCRIPT_TYPES`** (default `"p2wpkh"` for v1.3 backwards compat). Parsed via `client::config::parse_script_type` token-by-token over a split-on-comma, mirroring Phase 16's PKARR `sst` CSV producer convention (alphabetical lowercase kebab-case wire form). Empty CSV / unparseable token → bot exits with `bail!("BLINDJOIN_BOT_SCRIPT_TYPES = '{}' is invalid (...)")` at startup, mirroring the existing safety-guard pattern at `liquidity-bot/src/main.rs:43-49`.
- **D-93:** **Bot env-var naming convention preserved.** The bot uses SINGLE-underscore env-vars (`BLINDJOIN_COORDINATOR_URL`, `BLINDJOIN_UTXO`, `BLINDJOIN_UTXO_WIF` — see `liquidity-bot/src/main.rs:35-69`), distinct from the COORDINATOR's double-underscore-namespaced config (`BLINDJOIN__BIP__OUTPUT_SCRIPT_TYPE`). Phase 17 CD-22 already locked single-underscore for client-side; Phase 18 extends the same convention to the bot: `BLINDJOIN_BOT_SCRIPT_TYPES` (NOT `BLINDJOIN__BOT__SCRIPT_TYPES`). Plan-phase MUST NOT mix conventions.

### G. Bot per-round type rotation strategy

- **D-94:** **Persistent counter file at `/app/data/bot_round_counter`.** The bot reads the counter on startup, computes `enabled_types[counter % enabled_types.len()]` as the type for the current run, increments + atomically writes the counter file ONLY on successful round completion (matches the "exits after one successful round" pattern at `liquidity-bot/src/main.rs:140-146`). On failure or restart-before-completion, the counter is NOT bumped — the bot retries the same type next run.
  - **Atomic write idiom:** `tokio::fs::write` to a temp path under `/app/data/`, then `tokio::fs::rename` to the final path (matches the BLAME-05 ban-list append pattern at coordinator-side).
  - **File schema:** a single line containing the decimal counter (`u64`). Missing file = counter `0`. Malformed file (parse error) = bot exits with a triage-friendly bail.
  - **Volume mount:** the `/app/data` volume already exists in `docker/docker-compose.yml:60-61` for the COORDINATOR; the bot stage gets its own volume entry (or shares the coordinator's — plan-phase decides; recommended: separate `bot-data` volume to keep concerns separable).
- **D-95:** **Single-shot model preserved.** The bot continues to exit after one successful round (Phase 4 Pitfall 3 carry-forward at `liquidity-bot/src/main.rs:140-146`); Docker `restart: unless-stopped` re-launches it, which re-reads `/app/data/bot_round_counter` (now incremented), which selects the NEXT type. The "rotates type per round" wording is satisfied via the restart cycle — no in-process multi-round driver added.
- **D-96:** **No randomization.** Round-robin is deterministic: counter 0 → first type, counter 1 → second type, etc. (modulo `enabled_types.len()`). Randomized rotation would fail the "rotates per round" wording (could pick the same type 3 runs in a row). Determinism also makes a failing-bot diagnosable from the counter file alone.

### H. Bot UTXO sourcing across multiple script types

- **D-97:** **Per-type env-var tuples for v1.4 minimal viability.** New env vars (default `""`/unset means "type disabled"):
  ```
  BLINDJOIN_BOT_P2WPKH_UTXO         (txid:vout)
  BLINDJOIN_BOT_P2WPKH_WIF          (WIF — used only by P2WPKH via from_wif)
  BLINDJOIN_BOT_P2TR_UTXO           (txid:vout)
  BLINDJOIN_BOT_P2TR_DESCRIPTOR     (tr(xprv/86'/0'/0'/0/*) — used by from_descriptor)
  BLINDJOIN_BOT_P2SH_P2WPKH_UTXO    (txid:vout)
  BLINDJOIN_BOT_P2SH_P2WPKH_DESCRIPTOR (sh(wpkh(xprv/49'/0'/0'/0/*)) — used by from_descriptor)
  ```
  Bot's runtime selection logic:
  - Parse `BLINDJOIN_BOT_SCRIPT_TYPES` → `enabled: Vec<ScriptType>`.
  - For each enabled type, validate the matching tuple is present and well-formed at startup (fail-fast at boot per Phase 8 hardening pattern); accumulate into `tuples: HashMap<ScriptType, (Utxo, Credentials)>`.
  - On each round attempt, pick `script_type = enabled[counter % enabled.len()]`; load `(utxo, creds)` for that type; build wallet via `BdkClientWallet::from_wif(...)` (P2WPKH) or `BdkClientWallet::from_descriptor(...)` with the matching script_type (P2TR / P2SH-P2WPKH) per Phase 17 D-61 + D-65.
- **D-98:** **Backwards-compat: legacy single-WIF env-vars stay.** The existing `BLINDJOIN_UTXO` + `BLINDJOIN_UTXO_WIF` (single-underscore) env-vars continue to work — interpreted as the P2WPKH tuple when `BLINDJOIN_BOT_SCRIPT_TYPES` is unset (default `"p2wpkh"`). A v1.3 deployment (operator who never set `BLINDJOIN_BOT_SCRIPT_TYPES`) sees byte-identical bot behaviour. Plan-phase decides whether to ALSO accept `BLINDJOIN_BOT_P2WPKH_UTXO/WIF` as aliases (recommended: yes, with a startup log line noting which env var path resolved).
- **D-99:** **HD wallet model (H2) deferred to v1.5.** The bot does NOT scan bitcoind for spendable UTXOs at derived addresses, nor does it carry a master seed that derives per-round indices. Per-run keychain derivation (the "per-round keychain derivation continues to prevent output-address clustering" wording in INTEG-02) is satisfied by the SINGLE-SHOT pattern: each bot run rebuilds a fresh wallet from the env-var-supplied credentials → `coinjoin_output_address()` returns `peek_address(External, 0)` of that fresh wallet → fresh wallets across runs = fresh output addresses across runs. **No new code needed to satisfy this property** — the Phase 4 single-shot model + Phase 17 descriptor wallets already provide it. Confirmed via `client/src/wallet.rs:380-382` (peek_address-based; no state mutation; fresh wallet per `generate`/`from_descriptor` call).

### I. Per-round-index derivation interpretation

- **D-100:** **Single-shot model preserves output non-clustering.** Each bot run constructs a NEW `BdkClientWallet` from env-var-supplied credentials; the wallet's `coinjoin_output_address()` returns `peek_address(External, 0)`, which is keyed entirely off the descriptor (xprv) — different runs with different xprvs (operator-supplied or generated) give different output addresses. The "per-round keychain derivation continues to prevent output-address clustering" wording is interpreted as a property assertion (not a new requirement), and it is structurally satisfied by D-99's reading. No code change is needed for this clause — the property already holds in the v1.3+Phase 17 codebase.

### J. Bot test strategy

- **D-101:** **Unit tests in `liquidity-bot/src/strategy.rs`** for the rotation logic:
  - `pick_script_type_round_robin_advances_counter` (counter 0 → first; counter 1 → second; counter `len` → first again).
  - `pick_script_type_with_single_type_does_not_rotate` (degenerate len=1 case).
  - `pick_script_type_empty_enabled_returns_err` (defensive — should never fire if startup validation runs, but unit-test the function-level invariant).
  - Counter file roundtrip tests (parse, increment, atomic-write, malformed-file bail).
- **D-102:** **Integration test for bot-rotation-across-restarts:** new file `tests/integration/bot_rotation.rs` (or similar — plan-phase decides whether this lives under `tests/integration/` or `liquidity-bot/tests/`). 3-run cycle: bot runs once with counter=0 → asserts `script_type==P2wpkh`; counter file bumped to 1; bot runs again → asserts `P2tr`; counter file bumped to 2; bot runs again → asserts `P2shP2wpkh`. Each run drives an in-process v1.4 coordinator (reuse `spawn_coordinator` from `full_round.rs`) and a per-type funded UTXO (via `fund_regtest_typed` for the input UTXO; descriptor-derived for the bot's OWN output address). **Caveat:** the integration test will require the bot binary to be runnable as a library or extracted into a callable function — currently the bot is a `#[tokio::main]` binary. Plan-phase decides whether to (a) extract a `liquidity_bot::run(config)` library function callable from tests OR (b) drive the bot via `tokio::process::Command::new` against the built binary (parallels the v1.3-binary gate at D-86). RECOMMENDED: (a) — cleaner and reuses workspace test infra.
- **D-103:** **Test isolation: one bitcoind per `#[tokio::test]` fn.** Matches `full_round.rs` + `multi_script_validate.rs` pattern. Slow (each test spins up bitcoind) but bulletproof against UTXO/state cross-pollution.

### K. Acceptance-gate broadcast verification

- **D-104:** **Same `get_raw_mempool` polling pattern as `full_round.rs:296-326`.** 10s deadline, 100ms cadence, panic on miss with a diagnostic. Asserts non-empty mempool AND `denom_output_count == 3` AND the broadcast tx contains 3 distinct input script types (assertion via `tx.inputs` walk: read each prevout's SPK from the typed-funding handles, classify via `shared::bip322::detect_script_type`, assert the set is `{P2wpkh, P2tr, P2shP2wpkh}`). **Plan-phase consolidation guidance:** this is the load-bearing assertion that closes ROADMAP success criterion #1 — "at least 1 P2WPKH + 1 P2TR + 1 P2SH-P2WPKH input register". The set-equality check is preferred over the "at-least-one-of-each" check because the test funds exactly one of each by construction.

### L. Plan ordering

- **D-105 (plan ordering — 3 plans):**
  - **18-01-PLAN.md** = INTEG-01 — `tests/integration/mixed_script_e2e.rs` mixed-script E2E test (NEW file), wired through `mod.rs`. Tests: 1 fn (`mixed_script_e2e_three_clients_broadcast`) asserting the full INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST flow with 1 P2WPKH WIF wallet + 1 P2TR descriptor wallet + 1 P2SH-P2WPKH descriptor wallet, all against a `BipConfig::default()` in-process coordinator. Maps requirement **INTEG-01**. Sequential dependency chain: independent of 18-02 and 18-03 (can land first; reduces uncertainty on the acceptance gate before touching the bot).
  - **18-02-PLAN.md** = INTEG-02 — `liquidity-bot/src/main.rs` + `liquidity-bot/src/strategy.rs` multi-script + rotation. Adds `BLINDJOIN_BOT_SCRIPT_TYPES` CSV + per-type tuple env vars + rotation-counter file at `/app/data/bot_round_counter` + per-type wallet construction via WIF (P2WPKH) or descriptor (P2TR / P2SH-P2WPKH). Tests: unit tests for rotation logic + integration test `tests/integration/bot_rotation.rs` driving 3 sequential runs and asserting type rotation across counter values. Maps requirement **INTEG-02**. Depends on 18-01 only loosely (18-01 establishes the descriptor-wallet funding pattern that 18-02's integration test reuses for the bot's own UTXOs).
  - **18-03-PLAN.md** = v1.3-binary compat gate + README §"Privacy Considerations" prose + cross-phase invariant verification + milestone-cut readiness checklist. Tests: 1 fn (`v13_client_p2wpkh_against_v14_coordinator`) building v1.3 from pinned SHA and driving via `Command::new` (per D-86; fallback to documented UAT per D-87). Closes ROADMAP success criterion #5. README prose closes Phase 14 CD-3 carry-forward. Final 18-VERIFICATION.md gathers Phase 18 + v1.4 milestone readiness signals (8/8 full_round green, mixed_script_e2e green, bot_rotation green, v13 compat green, README §"Privacy Considerations" prose lands, all 5 ROADMAP Phase 18 success criteria observable in codebase). Sequential dependency: depends on 18-01 (mixed-script test must exist for verification) and 18-02 (bot rotation must exist for verification).
  - Wave structure: 18-01 (wave 1) → 18-02 (wave 2, descriptor-funding pattern from 18-01 carries) → 18-03 (wave 3, gathers final invariants).

### M. README §"Privacy Considerations" prose (Phase 14 CD-3)

- **D-106:** **NEW section `## Privacy Considerations` in `README.md`,** placed AFTER the existing user-facing run-instructions section and BEFORE the §"Architecture" / §"Tech Stack" sections (plan-phase locates precisely). Two paragraphs:
  - **Paragraph 1 (V1.4-MOD-06 chain-analysis fingerprint):** "blindjoin accepts mixed input script types (P2WPKH, P2TR, P2SH-P2WPKH) in a single round. This maximizes the anonymity set across address types but creates a chain-analysis signal: a CoinJoin transaction with a wildly heterogeneous input set is visually distinguishable from a uniform-script CoinJoin. Privacy-sensitive users who require uniform-script rounds can run a dedicated coordinator with a single `allow_*` flag enabled."
  - **Paragraph 2 (V1.4-MIN-02 liquidity-bot rotation mitigation):** "The bundled liquidity bot rotates the script type it submits across rounds. This prevents the bot's UTXOs from forming a uniform-script-type fingerprint (which would otherwise identify the bot's participation by cross-round correlation). Rotation is round-robin across the operator-configured `BLINDJOIN_BOT_SCRIPT_TYPES`; each run is single-shot and uses a fresh wallet, so output addresses do not cluster across rounds."
  - Plain language; no scary uppercase; no marketing claims. Mirrors PROJECT.md's "infrastructure, not a product" tone.

### Claude's Discretion

- **CD-25:** Whether the v1.3-binary gate goes the automated path (D-86) or the documented-UAT path (D-87). Default: **D-86 automated** with D-87 as escape valve if plan-phase's research surfaces unexpected build-infra cost (e.g., v1.3 commit has cargo-version drift requiring a different rustc). Plan-phase records the decision in 18-03-PLAN.md.
- **CD-26:** Whether `tests/integration/bot_rotation.rs` lives under `tests/integration/` (shared with the rest of the integration suite) or under `liquidity-bot/tests/` (crate-local). Default: **`tests/integration/`** — keeps the integration-test feedback loop centralized; the bot rotation test depends on `coordinator::spawn_coordinator` infra that already lives in the integration crate.
- **CD-27:** Whether the bot's `pick_script_type` accessor lives on `JoinStrategy` (current strategy module) or on a new `RotationState` type. Default: **new `RotationState` type in `strategy.rs`** — keeps the "should-I-join" concern separate from the "which-type-this-round" concern; both consumed by `main.rs`.
- **CD-28:** Whether the rotation-counter file path is hardcoded as `/app/data/bot_round_counter` or configurable via env var (`BLINDJOIN_BOT_COUNTER_FILE`). Default: **configurable** — mirrors `BLINDJOIN__COORDINATOR__BAN_FILE_PATH` configurability at coordinator side; ergonomic for tests (point at a tempdir-backed path).
- **CD-29:** Whether the descriptor-mode bot accepts a raw `xprv` (in env var) vs a full BIP-380 descriptor string (`tr(xprv/86'/...)`). Default: **full descriptor string** — matches the client's `--descriptor` flag (per Phase 17 `BdkClientWallet::from_descriptor` signature); avoids the bot owning xprv derivation logic; operator already understands the descriptor shape from `blindjoin client --generate-wallet` output (per Phase 17 verification log at line 219-229).
- **CD-30:** Whether the mixed-script E2E test asserts the broadcast tx's INPUT script types via `tx.inputs[i].witness` inspection OR via re-querying bitcoind for each prevout SPK. Default: **re-query bitcoind** (matches `full_round.rs:357-365` style); witness-inspection works but is brittle if a future bdk_wallet version changes finalisation byte form.
- **CD-31:** Whether 18-02 extends `liquidity-bot/Cargo.toml` to depend on `tempfile` (test-only `[dev-dependencies]`) for counter-file unit tests. Default: **yes** — already a workspace dependency in `tests/integration/mod.rs:101` (`tempfile::tempdir`); reuse the workspace pin.
- **CD-32:** Whether the v1.3-binary gate test runs by default (`#[tokio::test]`) or behind an opt-in feature flag like `--features v13-binary`. Default: **opt-in feature flag** — the build of v1.3 from a pinned SHA is heavyweight (~ 30s first-run, even with cache); gating it via a feature avoids slowing the default `cargo test` invocation. CI can run it as a separate job. Plan-phase decides the exact feature name (recommended: `v13-binary-compat`).
- **CD-33:** Whether the README §"Privacy Considerations" prose mentions the WabiSabi / variable-amount-credentials roadmap absence (mentioned in PROJECT.md Out of Scope). Default: **no** — the existing PROJECT.md "Out of Scope" section already covers this; the README disclaimer focuses narrowly on the V1.4-MOD-06 fingerprint and V1.4-MIN-02 mitigation per Phase 14 CD-3 wording. Adding WabiSabi context here would dilute the disclaimer.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing.**

### Phase 14 ADR + Phases 15/16/17 outputs (LOCKED inputs)

- `.planning/decisions/v1.4-adr.md` §`#decision-2` (Mixed vs segregated rounds) — RECORDS MIXED with V1.4-MOD-06 fingerprint as known limitation; binds Phase 18 INTEG-01 mixed-input-acceptance gate AND the README §"Privacy Considerations" prose (D-106).
- `.planning/decisions/v1.4-adr.md` §`#decision-2` Consequences (negative) — "Exact phrasing is deferred to Phase 18 prose work per CD-3 — the ADR records the limitation here without pre-writing the README copy." Binds D-106 verbatim.
- `.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` §D-06..D-10 — MIXED rounds + per-round output single-script-type + CRIT-01 invariant. Phase 18 mixed-script E2E test exercises D-06 + D-10 end-to-end; D-07 (single output type per round) is satisfied by client-side discovery cross-check (Phase 17 D-76), NOT by coordinator runtime enforcement (D-90 below).
- `.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` §CD-3 — "Defer the exact wording to the planner who's already in the prose-writing mode for v1.4 docs (likely Phase 18)." Binds D-106 prose deliverable.
- `.planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-22..D-32 — wire shape locked (`OwnershipProof` flat struct + `version` envelope + `script_type` sibling field). Phase 18 reuses verbatim — no shared/ changes.
- `.planning/phases/15-shared-crate-multi-script-contract/15-VERIFICATION.md` — confirms shared::bip322 + shared::protocol API surface stable. Phase 18 boundary-test verifies these via reused-mid-stack integration.
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §D-35..D-44 — coordinator BipConfig (allow_p2wpkh/tr/sh_p2wpkh + output_script_type) + PKARR record + InfoResponse legacy defaults. Phase 18 mixed-script E2E coordinator uses `BipConfig::default()` (all-allowed + p2wpkh-output); the v1.3-binary gate exercises InfoResponse legacy defaults transparently.
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §`fund_regtest_typed` (Phase 16 16-02 Task 2) — already in place at `tests/integration/mod.rs:617-823`; Phase 18 INTEG-01 reuses verbatim per D-83.
- `.planning/phases/17-client-multi-script-wallet-discovery/17-CONTEXT.md` §D-61..D-65 — `from_wif` P2WPKH-only + per-type sign dispatch via `wallet.sign_bip322`. Phase 18 mixed-script E2E client construction follows verbatim (P2WPKH via from_wif; P2TR + P2SH-P2WPKH via from_descriptor).
- `.planning/phases/17-client-multi-script-wallet-discovery/17-CONTEXT.md` §D-71..D-76 — extended CoordinatorInfo + capabilities + DiscoveryError + pre-Tor fail-fast + WALLET-04 compat shim. Phase 18 mixed-script E2E test uses the synthetic CoordinatorInfo pattern (D-85) to bypass discovery; the v1.3-binary gate exercises the actual v1.3 wire path against the v1.4 coordinator's compat shim (D-86).
- `.planning/phases/17-client-multi-script-wallet-discovery/17-CONTEXT.md` §D-78 / D-79 — Phase 17's stubbed-coordinator WALLET-04 test + explicit handoff to Phase 18 for the binary acceptance gate. Phase 18 INTEG-01 closes the binary gate per D-79.
- `.planning/phases/17-client-multi-script-wallet-discovery/17-VERIFICATION.md` §"5 ROADMAP Success Criteria" — Phase 17 fully passes; the 8/8 `full_round.rs` baseline (42.23s) is the cross-phase invariant gate Phase 18 must preserve.
- `.planning/research/PITFALLS.md` §V1.4-MOD-06 / V1.4-MIN-02 — privacy disclaimers Phase 18 README prose echoes (D-106).
- `.planning/research/PITFALLS.md` §"Mitigation overview" table line 263 — "Liquidity bot update | Uniform-script fingerprint (V1.4-MIN-02) | Rotate script types; honest README disclaimer". Binds D-94..D-96 (rotation) + D-106 (disclaimer).

### Project-level anchors

- `.planning/PROJECT.md` §"Current Milestone: v1.4 BIP-322 Multi-Script Support" — milestone goal includes "End-to-end integration test: full CoinJoin round with mixed P2WPKH + P2TR + P2SH-P2WPKH participants on regtest" and "Liquidity bot updated to generate test UTXOs across all supported script types". Phase 18 is the final coding phase of v1.4.
- `.planning/PROJECT.md` §"Constraints" — no custom crypto + Tor-native (production; regtest tests bypass) + signet-first + NO PII logging. Binds the bot's per-run-counter file to be PII-free (just a counter) and the README §"Privacy Considerations" prose to use plain language.
- `.planning/PROJECT.md` §"Out of Scope" — Mainnet as default; metrics dashboards; Tor mode UAT; offline mode. Phase 18 respects all 4 (regtest test infra; no telemetry; no Tor toggle changes; no offline mode).
- `.planning/REQUIREMENTS.md` §INTEG-01 (line 33) — "Mixed-script E2E integration test on regtest — full CoinJoin round with at least 1 P2WPKH + 1 P2TR + 1 P2SH-P2WPKH input completes through INPUT_REG → OUTPUT_REG → SIGNING → BROADCAST; reuses BitcoindGuard + require_bitcoind!() macro from v1.3 unchanged; v1.3 P2WPKH-only full_round::* tests remain green at this phase boundary (rollback safety net)". Phase 18 18-01-PLAN closes this verbatim.
- `.planning/REQUIREMENTS.md` §INTEG-02 (line 34) — "Liquidity bot generates UTXOs across all enabled script types (new config field script_types: ["p2wpkh", "p2tr", "p2sh-p2wpkh"]); rotates type per round so bot's UTXOs aren't a uniform fingerprint (V1.4-MIN-02 mitigation); per-round keychain derivation continues to prevent output-address clustering". Phase 18 18-02-PLAN closes this verbatim (note: "script_types" wire-form is a CSV in our env-var implementation per D-92 — REQUIREMENTS wording uses JSON-array notation as a conceptual hint, not a strict format requirement).
- `.planning/REQUIREMENTS.md` §"Traceability table" (line 91-92, 108) — INTEG-01 + INTEG-02 → Phase 18.
- `.planning/ROADMAP.md` §"Phase 18" — 5 success criteria. Phase 18 18-01..18-03 close all 5.
- `.planning/STATE.md` §"Accumulated Context" — 4/5 v1.4 phases complete; Phase 18 is the final v1.4 coding gate.
- `.planning/research/PITFALLS.md` §"v1.0 cross-cutting CoinJoin pitfalls remain in force" — Phase 18 inherits v1.0 pitfalls (tagging attack, blame-shrinkage, denomination fingerprinting) without re-litigation; the mixed-script test does NOT need to assert these are mitigated — Phase 14/15 already does.

### Phase 17 + Phase 16 + Phase 15 API surface (what Phase 18 consumes)

- `client::wallet::BdkClientWallet::generate(outpoint, network, script_type)` (Phase 17) — Phase 18 INTEG-01 uses this for the 3 descriptor wallets; INTEG-02 bot uses this for the P2TR + P2SH-P2WPKH paths.
- `client::wallet::BdkClientWallet::from_descriptor(descriptor_str, outpoint, network, script_type)` (Phase 17) — Phase 18 bot uses this when descriptor-mode env vars supplied.
- `client::wallet::BdkClientWallet::from_wif(wif, outpoint, network)` (v1.3, preserved by Phase 17 D-61) — Phase 18 INTEG-01 P2WPKH path + bot P2WPKH path.
- `client::wallet::BdkClientWallet::sign_bip322(message)` (Phase 17) — Phase 18 INTEG-01 + bot use this verbatim; no extension.
- `client::round::input::register_input(http, wallet, info, &coordinator_info)` (Phase 17) — Phase 18 INTEG-01 + bot call sites unchanged; per-test synthetic `CoordinatorInfo` per D-85.
- `client::discover::{CoordinatorInfo, CoordinatorCapabilities, DiscoveryError}` (Phase 17) — Phase 18 mixed-script E2E + bot construct synthetic instances per D-85; the v1.3-binary gate exercises the actual `discover_coordinator` flow.
- `coordinator::config::BipConfig::default()` (Phase 16) — Phase 18 in-process coordinator uses this default.
- `coordinator::bitcoin::utxo::validate_utxo` (Phase 16 dispatcher) — Phase 18 exercises this end-to-end with mixed-script inputs.
- `shared::bip322::{ScriptType, detect_script_type, verify_simple, sign_simple, sign_simple_test_only}` (Phase 15) — Phase 18 mixed-script E2E test uses `detect_script_type` in the post-broadcast input-type assertion (D-104); `sign_simple_test_only` is NOT used by 18-01 (production sign path goes through `wallet.sign_bip322`).
- `tests/integration/mod.rs::{BitcoindGuard, require_bitcoind!, bootstrap_regtest_bitcoind, fund_regtest_typed, TypedUtxoHandle, FundedTypedSetup, fund_regtest, FundedSetup, RpcCreds}` — Phase 18 imports and reuses unchanged.
- `tests/integration/full_round.rs::{spawn_coordinator, wait_for_coordinator, v14_p2wpkh_coordinator_info, build_input_reg_round_state}` — Phase 18 18-01 may PROMOTE these to `mod.rs` (or import via `crate::full_round::*` if Rust allows) for reuse. Plan-phase decides; recommended: promote to `mod.rs` to keep `full_round.rs` zero-touch (cross-phase invariant).

### Code anchors (Phase 18 reads OR modifies)

- `tests/integration/mod.rs:19-24` (mod declarations) — extend with `mod mixed_script_e2e;` (alphabetically positioned).
- `tests/integration/mod.rs:617-823` (`fund_regtest_typed` body + `TypedUtxoHandle` + `FundedTypedSetup`) — Phase 18 INTEG-01 may reuse for the input UTXO funding stage (D-83 stage 1 or skipped depending on plan-phase's B1.a-vs-B1.b call).
- `tests/integration/full_round.rs:85-153` (`spawn_coordinator` helper + tempdir guard) — Phase 18 INTEG-01 + bot_rotation tests reuse via promotion to `mod.rs` or direct import.
- `tests/integration/full_round.rs:49-59` (`v14_p2wpkh_coordinator_info` synthetic helper) — Phase 18 mixed-script E2E uses an analogous helper per script type (e.g., `v14_p2tr_coordinator_info()` etc.) — plan-phase generalises into a parameterized factory.
- `tests/integration/full_round.rs:296-326` (`get_raw_mempool` polling pattern + 10s deadline) — Phase 18 INTEG-01 mirrors verbatim (D-104).
- `tests/integration/full_round.rs:357-365` (post-broadcast denomination-output counter via `get_raw_transaction_verbose`) — Phase 18 INTEG-01 reuses, extended with the input-script-type set-equality assertion (D-104).
- `liquidity-bot/src/main.rs` (full file, 209 LOC) — Phase 18 18-02 extends env-var surface + adds script-type selection + descriptor-mode wallet construction + rotation-counter file I/O.
- `liquidity-bot/src/strategy.rs` (full file, 101 LOC) — Phase 18 18-02 extends with `RotationState` (or analogous) per CD-27.
- `liquidity-bot/Cargo.toml` (17 lines) — Phase 18 18-02 may add `tempfile` to `[dev-dependencies]` per CD-31; no runtime dep additions.
- `docker/docker-compose.yml:78-97` (liquidity-bot service) — Phase 18 18-02 extends `environment` block with new env vars + adds `bot-data` volume mount.
- `docker/Dockerfile` — Phase 18 18-02 may add a `VOLUME ["/app/data"]` directive to the liquidity-bot stage if not already present.
- `docker/docker-compose.yml:99-106` (volumes block) — Phase 18 18-02 adds `bot-data` volume entry (mirrors `coordinator-data`).
- `README.md` — Phase 18 18-03 adds §"Privacy Considerations" section per D-106. Plan-phase identifies the exact insertion point; recommended: after the "Quick Start" / run-instructions section and before "Architecture" / "Tech Stack".
- `coordinator/src/api/handlers.rs:296-440` (`post_output` handler) — Phase 18 reads only (verifies the absence of an output-script-type runtime check, per D-90); zero modifications.

### Cross-phase invariant references

- `tests/integration/full_round.rs` (full file, 1597 LOC) — Phase 18 INVARIANT GATE. Zero modifications. Run `cargo test --test integration full_round` after each Phase 18 plan; expect 8/8 green, ~42s.
- `tests/integration/full_round.rs:209` `let _bitcoind_guard = bitcoind_guard;` — example of the BitcoindGuard scoping invariant Phase 18 mixed-script E2E test mirrors verbatim.
- `tests/integration/multi_script_validate.rs` (full file, 425 LOC) — Phase 18 INTEG-01 mirrors its `fund_regtest_typed` + per-script-type assertion pattern at the test-fn level.
- `tests/integration/multi_script_client.rs` (full file, 373 LOC) — Phase 17 boundary test. Phase 18 INTEG-01 is the FULL-ROUND extension of this file's single-input-registration scope.

### External specs (Phase 18 references)

- BIP-322 §"Simple" — the message-signing spec the production sign path consumes. Phase 18 INTEG-01 verifies this end-to-end across 3 script types in one round.
- BIP-86 / BIP-49 / BIP-84 — descriptor format references. Phase 18 INTEG-01 + bot use these via `BdkClientWallet::{generate, from_descriptor}` (Phase 17 surface).
- BIP-174 (PSBT v2) — wire format for the v=2 OwnershipProof.psbt_input_b64 field. Phase 18 reads only (no producer/consumer changes).

### Tools / commands relevant to Phase 18 execution

- `cargo test --test integration mixed_script_e2e` — Phase 18 18-01 acceptance gate.
- `cargo test --test integration bot_rotation` — Phase 18 18-02 acceptance gate.
- `cargo test --test integration full_round` — cross-phase invariant gate. Must remain GREEN at every Phase 18 plan boundary (D-91).
- `cargo test -p liquidity-bot` — Phase 18 18-02 unit tests for rotation logic.
- `cargo test --test integration v13_binary_compat --features v13-binary-compat` (per CD-32) — Phase 18 18-03 v1.3 acceptance gate.
- `cargo build --workspace` — compile sanity.
- `cargo audit` — must remain clean (no new dependency additions in Phase 18 production; `tempfile` is dev-only).
- `git worktree add /tmp/blindjoin-v13-<sha> <sha>` — used by 18-03's automated v1.3-binary build path (D-86).
- `docker compose up -d` — operator-side smoke check post-18-02 (the bot's new env-var surface should not break the docker-compose stack; plan-phase 18-02 records a `.env.example` update so `docker compose up` succeeds without operator changes for the default P2WPKH-only path).
- `bitcoind` (regtest) — same v30.2 pinned version as v1.3 Phase 9 (`.bitcoind-version`).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`tests/integration/mod.rs::fund_regtest_typed`** (lines 617-823) — Phase 16 16-02 Task 2 multi-script regtest UTXO funding helper. Accepts `&[(ScriptType, usize)]`; returns `FundedTypedSetup { utxos: Vec<TypedUtxoHandle> }`. Each handle carries `secret_key: SecretKey` for BIP-322 signing via `shared::bip322::sign_simple_test_only`. **Phase 18 INTEG-01 reuses verbatim** for the input-UTXO funding stage. The handle's per-type address derivation (lines 692-740) is byte-deterministic via seeded SecretKey, so test failures are reproducible from source alone.
- **`tests/integration/mod.rs::bootstrap_regtest_bitcoind`** (lines 273-324) — single-locus regtest bring-up. Sets `view_stdout = false` + `-printtoconsole=0` for the cargo-stdout-pipe-hang prevention (v1.3 lesson #1). Mines 101 blocks. Returns `(BitcoindGuard, RpcCreds)`. Phase 18 INTEG-01 + bot_rotation tests reuse verbatim.
- **`tests/integration/mod.rs::BitcoindGuard`** (lines 150-228) — RAII guard with `Drop` impl that spawns blocking `n.stop()` onto tokio blocking pool (CR-01) + falls back to `Node::Drop` SIGKILL. **Phase 18 INTEG-01 uses verbatim — this is the load-bearing "rollback safety net" infrastructure named in ROADMAP success criterion #2.**
- **`tests/integration/mod.rs::require_bitcoind!()`** macro (lines 100-108) — graceful-skip in local-dev mode; panic in CI mode (`BLINDJOIN_REQUIRE_BITCOIND=1`). Phase 18 INTEG-01 + bot_rotation tests use verbatim — the macro's `None => return` expansion only works in a function returning `()`, so each test fn calls `let exe = require_bitcoind!()` as its FIRST line then forwards `exe` to bootstrap helpers.
- **`tests/integration/full_round.rs::spawn_coordinator`** (lines 85-153) — In-process axum coordinator with port-0 ephemeral binding + tempdir-backed ban file. **Phase 18 INTEG-01 + bot_rotation reuse this exactly** — plan-phase decides whether to promote it from `full_round.rs` into `mod.rs` (cleaner) or to factor a `crate::testing::spawn_coordinator` accessor (also clean). Either way, `full_round.rs` itself stays zero-touch per the cross-phase invariant.
- **`tests/integration/full_round.rs::v14_p2wpkh_coordinator_info`** (lines 49-59) — synthetic CoordinatorInfo factory that defaults `is_legacy: false` + P2WPKH-only capabilities. **Phase 18 INTEG-01 generalises into a parameterized factory per script type** (e.g., `v14_coordinator_info(script_type)` taking a `ScriptType` and returning a CoordinatorInfo with `supported = vec![script_type]` and `output = script_type`).
- **`client::wallet::BdkClientWallet::generate`** (Phase 17, `client/src/wallet.rs:209`) — Generates a fresh descriptor wallet per script type. Returns a wallet whose `coinjoin_output_address()` is the descriptor-derived `peek_address(External, 0)`. Phase 18 INTEG-01 uses this for the P2TR + P2SH-P2WPKH client wallets; the resulting external-index-0 address is funded via regtest `send_to_address` then `wallet.utxo_outpoint` is overridden to point at the freshly funded outpoint.
- **`client::round::input::register_input(http, wallet, info, &coordinator_info)`** (Phase 17 17-03) — single-call helper for the input-registration phase. Phase 18 INTEG-01 + bot_rotation use this verbatim.
- **`liquidity_bot::strategy::JoinStrategy`** (`liquidity-bot/src/strategy.rs:9-37`) — current "when to join" decision logic. Phase 18 18-02 extends with `RotationState` (or analogous) per CD-27 without modifying `should_join`'s shape.
- **`docker/docker-compose.yml::coordinator-data` volume** (lines 60-61, 105-106) — pattern for persistent state volumes. Phase 18 18-02 adds an analogous `bot-data` volume for the rotation-counter file.
- **`coordinator::config::BipConfig::default()`** (`coordinator/src/config.rs:256-265`) — all-allowed + p2wpkh-output. Phase 18 mixed-script E2E coordinator uses this default; no test-specific override.
- **`liquidity-bot/src/main.rs::synthetic_info`** (lines 171-183) — synthetic CoordinatorInfo for the direct `--coordinator-url` path (bypasses PKARR). Phase 18 18-02 extends `supported_script_types` to include the rotated-to type AND sets `output_script_type` to that type so the WALLET-03 + WALLET-04 cross-checks in `register_input` pass.

### Established Patterns

- **"Cross-phase invariant — never touch full_round.rs"** (Phase 14/15/16/17 carry-forward) — Every v1.4 phase adds NEW test files; never modifies `full_round.rs`. Phase 18 inherits this discipline strictly.
- **"Single locus for funded regtest setup"** (Phase 10-01 D-06 + Phase 16 16-02 promotion) — `fund_regtest` + `fund_regtest_typed` are the single sources of truth for regtest UTXO setup. Phase 18 does NOT add a third variant; if descriptor-wallet-driven funding is needed (per B1.b D-83), the new helper lives in `mod.rs` adjacent to the existing two and is named consistently (e.g., `fund_descriptor_wallet`).
- **"`require_bitcoind!()` first; forward the path"** (Phase 9 WR-03) — Phase 18 tests open with `let exe = require_bitcoind!();` and forward `exe` to bootstrap helpers (single source of truth for the bitcoind path per test invocation).
- **"`#[tokio::test]` per fn = one bitcoind per fn"** (Phase 9 + Phase 16 + Phase 17 carry) — Phase 18 INTEG-01 + bot_rotation tests follow.
- **"Atomic file write idiom"** (BLAME-05 + tempfile-then-rename) — Phase 18 18-02 bot rotation-counter file uses this pattern: tempfile under `/app/data/`, then `tokio::fs::rename` to final path.
- **"Single-underscore env vars on bot, double-underscore on coordinator"** (Phase 4 + Phase 17 CD-22 + Phase 8 patterns) — Phase 18 18-02 `BLINDJOIN_BOT_SCRIPT_TYPES` follows single-underscore.
- **"Lowercase kebab-case ScriptType wire form"** (Phase 15 `#[serde(rename_all = "snake_case")]` + `rename = "p2sh-p2wpkh"` + Phase 17 `parse_script_type`) — Phase 18 18-02 bot CSV parser reuses `client::config::parse_script_type` directly per D-92 + D-93.
- **"No PII in logs"** (PROJECT.md constraint) — Phase 18 bot logs the rotated-to script type (public info) + counter value (not PII) + any UTXO outpoint (public on-chain info, not PII per Phase 1 baseline). No new PII surface.
- **"Test names are documentation"** (Phase 15 D-34, Phase 16 D-54, Phase 17 D-78) — Phase 18 test names follow `<scenario>_<assertion>` shape (e.g., `mixed_script_e2e_three_clients_broadcast`, `bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs`, `v13_client_p2wpkh_against_v14_coordinator`).
- **"`matches!()` over string-equality on Bip322Error"** (Phase 15 D-34 + Phase 16 D-54) — Phase 18 tests assert on specific error variants via `matches!(...)`. Inherited; no new errors introduced.

### Integration Points

- **Phase 18 → Phase 14 (consumes ADR + CD-3 deferral):** Phase 18 18-03 closes the README §"Privacy Considerations" prose deliverable Phase 14 CD-3 deferred. ADR Decision #2 Consequences (negative) is the authoritative source for the disclaimer language.
- **Phase 18 → Phase 15 (consumes BIP-322 contract):** Phase 18 mixed-script E2E test uses `wallet.sign_bip322` (Phase 17 path) which routes through `shared::bip322::sign_simple(P2wpkh)` for P2WPKH + `bdk_wallet::Wallet::sign` for P2TR + P2SH-P2WPKH per Phase 17 D-65. Post-broadcast input-type classification via `shared::bip322::detect_script_type`.
- **Phase 18 → Phase 16 (consumes coordinator dispatcher + BipConfig + fund_regtest_typed):** Phase 18 INTEG-01 exercises `coordinator::bitcoin::utxo::validate_utxo` per-script-type dispatcher end-to-end. `BipConfig::default()` is the test coordinator's config. `fund_regtest_typed` funds the input UTXOs.
- **Phase 18 → Phase 17 (consumes client multi-script wallet + sign_bip322 + register_input):** Phase 18 INTEG-01 + bot use `BdkClientWallet::{generate, from_descriptor, from_wif, sign_bip322, script_type}` + `client::round::input::register_input` verbatim. The synthetic `CoordinatorInfo` pattern Phase 17 17-03 introduced for `full_round.rs` is generalised per D-85 + the established-pattern note above.
- **Phase 18 → Phase 4 (Docker Compose pattern):** Phase 18 18-02 extends `docker/docker-compose.yml::liquidity-bot.environment` + adds `bot-data` volume; mirrors the coordinator-data + coordinator-keys volume pattern from Phase 4.
- **Phase 18 → v1.4 milestone cut:** Phase 18 is the LAST coding phase before the v1.4 cut PR. Plan-phase 18-03 includes a "milestone readiness checklist" that gathers (a) 8/8 full_round green, (b) Phase 18 test gates all green, (c) v1.3-binary gate green (or UAT-documented per D-87), (d) README §"Privacy Considerations" lands, (e) all 5 ROADMAP Phase 18 success criteria observable in codebase, (f) RETROSPECTIVE.md + RETRO note for milestone closeout. The v1.4 cut PR itself is a separate `/gsd:ship` invocation AFTER Phase 18 closes.

</code_context>

<specifics>
## Specific Ideas

- **`mixed_script_e2e_three_clients_broadcast` test name** (D-82): the verb "broadcast" in the name is load-bearing — names the success-criterion-#1 endpoint (txid in mempool). Plan-phase may polish but should preserve the verb.
- **3-tuple shape `(P2WPKH WIF + P2TR descriptor + P2SH-P2WPKH descriptor)`** in INTEG-01: covers BOTH client wallet code paths (WIF-style legacy + descriptor-style modern) in one acceptance test. The WIF arm is the v1.3-byte-exact path; the two descriptor arms are the v1.4-new paths. Plan-phase may swap which type is WIF vs descriptor; recommended: P2WPKH stays WIF for maximum v1.3 carry-forward signal.
- **Post-broadcast assertion shape** (D-104): assert `denom_output_count == 3` AND `input_script_types == {P2wpkh, P2tr, P2shP2wpkh}` (set equality). The output-script-type assertion is OMITTED (outputs are heterogeneous per D-85's reading of D-07). Plan-phase decides whether to ALSO assert `output_script_types ⊆ supported_types` (no new types unexpectedly appearing) — recommended: add this as a defensive guard.
- **v1.3 pinned-SHA convention** (D-88): the SHA file at `.planning/phases/18-.../v13_pinned_sha.txt` is a 40-char hex string + optional comment line. The SHA targets the LAST commit on `main` before Phase 14's first commit (`1993436` is Phase 14-01 / Plan 17-03; plan-phase resolves the actual Phase 14 first commit). The pinned commit MUST predate any Phase 14+ code changes so that the v1.3 client built from it has the v1.3 P2WPKH-only wire shape and the v1.3 single-WIF wallet.
- **`BLINDJOIN_BOT_SCRIPT_TYPES` parsing** (D-92): use `s.split(',').map(str::trim).map(parse_script_type).collect::<Result<Vec<_>, _>>()?` mirroring the coordinator-side PKARR parser. Reject empty strings, duplicates (defensive — would never rotate predictably), and tokens not in `{p2wpkh, p2tr, p2sh-p2wpkh}`.
- **Rotation-counter file schema** (D-94 / CD-28): single-line `u64` decimal; missing file ⇒ counter `0`; malformed ⇒ bot bails at startup with `bail!("BLINDJOIN_BOT_COUNTER_FILE = '{path}' contains malformed counter (line 1): '{contents}'")`. Atomic write via tempfile-then-rename.
- **Per-type env-var tuples** (D-97): nullable env vars (default `""` = type-disabled). At startup, for each script type in `enabled_types`, REQUIRE the matching tuple env vars are populated and well-formed; bail otherwise with operator-facing error message naming the missing env var and the enabled-type that requires it.
- **README §"Privacy Considerations" prose tone** (D-106): plain language, no scary uppercase, ~ 2 paragraphs (~ 200 words total), matches PROJECT.md's "infrastructure, not a product" tone. No marketing claims; no minimization of the limitation.
- **3-plan structure** (D-105): 18-01 (INTEG-01, ~ 200-300 LOC including the new test file) → 18-02 (INTEG-02, ~ 250-400 LOC across main.rs + strategy.rs + docker-compose) → 18-03 (v1.3-binary gate + README prose + closeout, ~ 100-200 LOC code + ~ 50 LOC README). Plan-phase may rebalance; recommended order is locked per D-105.
- **CRIT-01 client-side discipline carries:** Phase 17 D-80's `// CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo` comment at `client/src/round/input.rs:152` is unchanged in Phase 18 — the bot's `wallet.script_type()` continues to flow through register_input verbatim. The grep gate `grep -c "CRIT-01" client/src/round/input.rs ≥ 1` is preserved.

</specifics>

<deferred>
## Deferred Ideas

- **Coordinator runtime check that submitted output script type matches advertised `ost`** — currently soft-enforced client-side via Phase 17 D-76 (`DiscoveryError::UnsupportedOutputScriptType`). A server-side gate at `coordinator/src/api/handlers.rs::post_output` would close the gap where a malicious client could submit a non-matching output address. Pre-condition for adding it: the mixed-script E2E test must EITHER drop the heterogeneous-output property OR opt-out of the gate via a test-only switch. v1.5+ candidate; out of v1.4 scope.
- **HD wallet (BIP-32/39 seed-driven) bot model with auto-discovery of spendable UTXOs** — H2 path per D-99. Bot holds a master seed (env var or volume-mounted file), derives per-round-index addresses via BIP-84/86/49 (matching client-side D-58), scans bitcoind at startup via `scantxoutset` for spendable UTXOs at its derived addresses, picks the next funded UTXO for the rotated-to type. Replaces D-97's per-type env-var tuples with a single seed + on-chain scan. v1.5+ candidate.
- **`scantxoutset`-driven discovery of operator-funded UTXOs** — even without HD wallet, a less aggressive enhancement: bot scans bitcoind for UTXOs at a single operator-supplied address per type; eliminates the need for operator to track `(txid, vout)` pairs in env vars. Lighter than HD wallet; still v1.5 ergonomic polish.
- **TEST-EXT-01/02/03 (cross-impl differential fixtures, on-chain anchor test, automated backwards-compat matrix)** — v1.5+ per REQUIREMENTS Future Requirements. The Phase 18 v1.3-binary gate (D-86) is a NARROW cell of the matrix (v1.3-client × v1.4-coordinator × P2WPKH), discharging WALLET-04's binary-acceptance requirement; the full N×M matrix is v1.5.
- **CARRY-TOR-UAT (Tor-mode verification harness)** — v1.5+ per PROJECT.md / REQUIREMENTS Future Requirements + Phase 8 HUMAN-UAT item 3.
- **CARRY-REPAIR-01-PR (REPAIR-01 PR observation closure)** — discharged at v1.4 cut PR per PROJECT.md "the v1.4 cut PR is the natural moment to discharge this but is NOT a v1.4 code deliverable per REPAIR-01 lesson #5". Phase 18 does NOT touch this; the v1.4 cut PR (separate `/gsd:ship` invocation post-Phase-18) does.
- **Bot rotation-counter rolling persistence with TTL** — currently the counter grows monotonically. After 2^64 rounds (impossibly far away), it would overflow. Wrap-around is implicit via `% enabled_types.len()`, so no real bug; but plan-phase may consider a graceful `counter.checked_add(1)` with overflow-reset. v1.5+ polish.
- **Bot-side cancellation / shutdown signal handling** — currently the bot's `tokio::time::sleep` loop is uninterruptible by SIGTERM (Docker's stop-then-kill cycle works because the loop's sleeps are short). Cleaner: a `tokio::select!` between the loop body and a `tokio::signal::ctrl_c()` future. v1.5+ deployment polish.
- **`docker compose down -v` cleanup of bot-data volume** — operator-facing concern (losing the rotation counter resets to type-0). Doc-only fix in v1.4 README; runtime safeguard (warn-on-stale-counter-file at startup) is v1.5+.
- **v1.3-binary gate as CI-required check** — Phase 18 18-03 lands the gate as opt-in (`--features v13-binary-compat`). Plan-phase may decide to promote it to CI-required in a v1.5 follow-up once the build infra is proven stable.
- **`fund_descriptor_wallet` helper consolidation** — if Phase 18 INTEG-01 takes the D-83 B1.b path (descriptor-wallet-driven funding inline in the test), the inline body may grow to a size that warrants extracting a `fund_descriptor_wallet(node, wallet, fund_sats) -> (OutPoint, u64)` helper in `mod.rs`. Decision deferred to plan-phase 18-01.
- **Per-type denomination / per-round breakdown via wire-format additions** — out of scope per Phase 16 D-08 / D-09 REJECTIONS. v1.5+ candidate if operators request it.
- **Cross-coordinator round cascades / multi-hop joins** — out of scope per PROJECT.md.
- **Mobile client (iOS/Android)** — out of scope per PROJECT.md.
- **Mainnet as default** — out of scope per PROJECT.md; mainnet remains a single env-var flip post-v1.4 with no Phase 18 dependency.
- **DECISIONS-INDEX.md rolling summary** — v1.5 candidate per Phase 14/15/16/17 CONTEXT carry-overs. The volume of `D-*` decisions (now 100+ across v1.4 with Phase 18's D-81..D-106) is approaching the threshold where a rolling decisions index would help downstream agents avoid full-CONTEXT reads.
- **`bdk_wallet = "=2.3.x"` exact-pin tightening** (Phase 15 RESEARCH A7 + Phase 16 + Phase 17 deferred carry-forward) — Phase 18 is the FOURTH consumer of bdk_wallet 2.3 (coordinator + Phase 15 shared/ + Phase 17 client/ + Phase 18 liquidity-bot/). Pin tightening becomes load-bearing if a 2.4 release breaks taproot finalisation. Currently a small drift surface, not load-bearing for Phase 18 behaviour. v1.5+ candidate.

</deferred>

---

*Phase: 18-Mixed-Script E2E + Liquidity Bot*
*Context gathered: 2026-05-30 via /gsd:discuss-phase --auto*
*All gray areas auto-resolved per recommended defaults; review CONTEXT.md before /gsd:plan-phase or override specific decisions inline.*
