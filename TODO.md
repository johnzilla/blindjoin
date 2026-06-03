# TODO

## Resolved 2026-05-31

- [x] **v1.5 Audit-Readiness & Multi-Script Finish — SHIPPED.** 3 phases
  (19, 20, 21), 5 plans. Phase 19 (BIP322-05/06/07) shipped production
  `sign` bodies for P2TR (BIP-341 Schnorr keypath via
  `sign_schnorr_no_aux_rand`) and P2SH-P2WPKH (BIP-143 over unwrapped
  P2WPKH redeem) in `shared::bip322`, deleted `sign_simple_test_only`
  and per-script `sign_for_tests` helpers, migrated all callsites to
  the production `sign_simple` dispatcher; `shared::bip322` public
  surface is now exactly 9 symbols with V1.4-CRIT-01 dispatcher-only
  load-bearing at the type level. Byte-equality with
  `BdkClientWallet::sign_bip322` proven empirically in
  `client/tests/wallet_sign_roundtrip.rs`. Phase 20 (FEE-01/02/03)
  shipped a per-script BIP-141 witness-weight table in
  `coordinator/src/bitcoin/tx.rs` (P2WPKH 68/31, P2TR 58/43
  round-UP, P2SH-P2WPKH 91/32) replacing the legacy P2WPKH-only
  `INPUT_WEIGHT_VBYTES`/`OUTPUT_WEIGHT_VBYTES`; `ParticipantInput.script_type`
  plumbed coordinator-side from `dispatch_ownership_proof` through
  `UtxoDetails → RegisteredInput` (CRIT-01 invariant preserved into
  the fee path); regression test pins v1.4 P2WPKH-only
  `fee_share == 266` byte-equal. Phase 21 (AUDIT-01/02/03) readied
  the codebase for external audit: `docs/AUDIT-CHARTER.md` (574 LOC,
  8 H2 sections including in-scope modules with file:symbol refs,
  threat models, 9 cross-shape rejection properties, v=2 PSBT handling,
  RSA SecretKey zeroization window in bounded form, out-of-scope,
  residual risks in 3 sub-buckets, glossary); `.cargo/audit.toml`
  refreshed with charter-anchor refs and a RUSTSEC-2023-0071 rewrite
  citing the AUDIT-03 bounded-window mitigation (no more "best-effort");
  `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` tightens the RSA
  SecretKey lifetime into a Rust type signature with one FSM chokepoint
  at `state.rs:202` inside `transition_to(Phase::Idle)`. Code review
  surfaced 1 critical (`let _ =` on FSM transitions); accepted as a
  defense-in-depth gap with explicit closure-trigger language in
  charter §7. All cross-phase invariants green at v1.5 close: v1.3
  `full_round::*` 8/8, v1.4 `mixed_script_e2e_three_clients_broadcast`
  1/1, Phase 20 `fee_share` 2/2, clippy clean,
  `cargo audit` 0 vulns/0 warnings. v1.6+ carries:
  CARRY-TOR-UAT (Phase 8 HUMAN-UAT item 3), CARRY-REPAIR-01-PR (v1.4
  cut PR moment), B-03 (dynamic fee estimation, pre-mainnet),
  TEST-EXT-01/02/03 (cross-impl differential fixtures + on-chain
  anchor test + v1.3↔v1.4 backwards-compat CI matrix), P2WSH
  multisig BIP-322, Wasabi 2.0.3-style mixed output scripts, and
  AUDIT-03 `let _ =` closure (per charter §7 Residual Risks:
  Protocol-level). Full per-phase summary in
  `.planning/phases/19-multi-script-signing-finish/`,
  `.planning/phases/20-mixed-round-fee-accuracy/`, and
  `.planning/phases/21-audit-charter-zeroization-tightening/`.

- [x] **B-02: BIP-322 multi-script support — SHIPPED as v1.4 milestone.**
  Coordinator accepts P2WPKH + P2TR + P2SH-P2WPKH ownership proofs under an
  operator-configurable `[bip]` allowlist; advertises `supported_script_types`
  over PKARR (`v0.2.0` schema, compact `sst`/`ost` fields, 209-byte
  production-onion payload) and `/round/info`. Client wallet supports
  `--type {p2wpkh|p2tr|p2sh-p2wpkh}` with BIP-84/86/49 descriptor templates
  (literal templates with `coin=0'` across all networks per RESEARCH Pitfall 2
  — bdk_wallet's `Bip84/86/49` helpers auto-select `coin=1'` on testnet/signet
  and would break v1.3 byte-equivalence). Client rejects mismatched
  coordinators at discovery time **before** opening a Tor circuit
  (`DiscoveryError::UnsupportedScriptType` with the literal "does not support"
  wording naming both coordinator and missing type). V1.4-CRIT-01 spoofing
  vector mitigated three ways: (1) coordinator derives `ScriptType` from
  on-chain `script_pubkey` and cross-checks against client declaration before
  the per-script verifier runs; (2) `shared::bip322` dispatcher-only public
  surface — per-script verify/sign are `pub(crate)`-only so callers cannot
  reach them to bypass dispatch. (The historical `crit-01-grep-check` and
  `crit-01-client-grep-check` CI gates were removed in the v1.6 theater
  strip — they enforced a comment token, not the underlying behavior.)
  Adopted upstream `bip322 = "=0.0.10"` via a
  26-LOC zero-lossy adapter (no custom sighash math) behind a
  `bip322-pin-check` CI gate. 5 phases (14-18), 15 plans; v1.3 P2WPKH-only
  `full_round::*` invariant held green at every v1.4 phase boundary (8/8 PASS).
  Backwards-compat shim (WALLET-04) verified bidirectionally: v1.4→v1.3 via
  the CD-7 two-phase try-parse `OwnershipProof` encoder (byte-identical v1.3
  array-of-hex in the legacy branch); v1.3→v1.4 verified inline by Phase 18-03
  against a pinned v1.3 binary at SHA `05f21438`. Tagged `v1.4.0`. Full
  per-phase summary in `.planning/milestones/v1.4-ROADMAP.md`.

## Resolved 2026-05-28

- [x] **REPAIR-01: full_round.rs carve-outs repaired.** All 8 `full_round::*`
  end-to-end integration tests now pass locally against pinned brew bitcoind
  v31.0.0; the six `#[ignore = "TODO(Phase-10)"]` markers have been removed.
  Root cause was NOT the originally-hypothesized `listunspent`/RPC schema drift —
  it was a wire-format mismatch in client/server witness encoding:
  `BdkClientWallet::sign_psbt_input` returned the raw first stack item from
  `final_script_witness.nth(0)` (DER signature + sighash byte), but the
  coordinator's `assemble_and_broadcast` decoded that wire payload as a
  `bitcoin::consensus::deserialize::<bitcoin::Witness>` — incompatible shapes.
  Fix: client consensus-serializes the whole 2-item P2WPKH witness stack
  (`[sig, pubkey]`) before transmission. Two adjacent fixes shipped in the
  same cycle: `post_input` now runs the ban check before blinded_token size
  validation (banned UTXOs reliably get HTTP 403 instead of 400), and the
  blame-test helper arms an input_reg→output_reg timer on the restart path
  so Round 2's partial-quorum case advances via timeout rather than hanging.
  See commits `39a2d0a`, `8538238`, `0780935`, `489646f`.

## Resolved 2026-05-27

- [x] **v1.3 Phase 9: CI integration-test reliability — SHIPPED.** All five
  TEST-XX requirements closed. Closes the four "Integration test harness
  reliability" follow-up findings (originally raised below, 2026-05-26):

  1. **CI now actually runs the integration tests.** `.github/workflows/ci.yml`
     `test:` job provisions Bitcoin Core v30.2 via integrity-verified install
     (PGP+SHA256, achow101 release-signing fingerprint
     `152812300785C96444D3334D17565732E08E5E41` pinned via guix.sigs SHA pin),
     cached across runs via `actions/cache@v4.3.0` keyed on `.bitcoind-version`.
     Workflow-level `BLINDJOIN_REQUIRE_BITCOIND=1` env makes integration tests
     panic-on-miss in CI instead of silently graceful-skipping.

  2. **`Box::leak`-pipe-hang eliminated.** Replaced 4 `Box::leak(node)` callsites
     with a shared `BitcoindGuard` RAII type whose `Drop::drop` calls
     `node.stop()` via `tokio::spawn_blocking` (so the kill happens off the
     tokio runtime thread per code-review CR-01). Combined with
     `conf.view_stdout = false` + `-printtoconsole=0`, the child never holds
     cargo's stdout pipe. Whole-repo `Box::leak` count in `tests/integration/`
     is now 0.

  3. **`full_round.rs` RPC-drift quarantined.** Six known-broken tests carry
     `#[ignore = "TODO(Phase-10): RPC schema drift on listunspent/getrawtransaction
     -- see TODO.md"]` markers; CI shows them in the `ignored` column without
     executing. The repair-or-retire decision is Phase 10 (REPAIR-01 + REPAIR-02).

  4. **`CONTRIBUTING.md` documents the canonical invocation pattern.** Section
     "Running integration tests" with a copy-pasteable command, log file
     location (`target/integration-test.log`), single-test invocation example,
     and a 4-row pass/fail/skip/ignored reference card. New contributors no
     longer need to rediscover the pipe-buffering pitfall.

  UAT closure (this session):
  - **UAT-1** (live CI PASS verdict): PR #7 CI run 26512029044, 9 passed / 0
    failed / 6 ignored in 3m49s. First two CI runs caught real PGP-verify bugs
    (multi-signer SHA256SUMS.asc + long-key-id vs full-fingerprint matching) —
    fixed in `ea16787` + `6d10d05`. The live-CI signal Phase 9 SC1 was designed
    for worked as advertised.
  - **UAT-2** (bounded panic exit): Injected `panic!("UAT-2")` into
    `round_bootstrap`; 8s wallclock to clean exit, `panicked at` line in log,
    no hang.
  - **UAT-3** (no orphan bitcoind): Before/after `pgrep` both empty after full
    suite ran (7.51s, 9 passed / 0 failed / 6 ignored).

  Code review found 1 critical + 5 warnings + 5 info; 5 fixed atomically, 1
  deferred to Phase 10 (WR-05 bare-sleep migration in 4 already-ignored tests).
  See `.planning/milestones/v1.2-phases/08-public-endpoint-hardening/` for v1.2
  archive; see `.planning/phases/09-ci-integration-test-reliability/` for v1.3
  Phase 9 artifacts.

## Resolved 2026-05-26

- [x] **Phase 8 HUMAN-UAT items 1 & 2 closed by local runtime proof.** Ran
  `cargo test --test integration rate_limiting:: -- --include-ignored` against
  `bitcoind v31.0.0` (Homebrew). Both tests passed:
  `info_endpoint_returns_429_when_flooded` and `request_timeout_returns_408`.
  Item 3 (Tor connection-cap) remains deferred per Plan 04 A4 — see follow-up
  below.

  Getting there required two unrelated fixes that landed atomically:

  1. **Production bugfix in `coordinator/src/bitcoin/rpc.rs`:** JSON-RPC envelope
     bumped from `"1.1"` → `"2.0"`. Bitcoin Core 31 returns `-32600 JSON-RPC
     version not supported` for 1.1 requests; Bitcoin Core 27 (docker image) and
     all releases since ~v22 accept 2.0. This was a latent bug — the coordinator
     would have failed at startup against any modern bitcoind, but our docker
     stack runs v27 which still accepted 1.1.

  2. **Test harness bump:** `corepc-node` 0.10 → 0.12 with `features = ["30_2"]`.
     Defaults are silly — corepc-node 0.12 still defaults to a Bitcoin Core
     0.17.2 (2018) RPC schema unless an explicit version feature is enabled.
     Without the feature, the test harness `createwallet` RPC against bitcoind
     v31 hangs with "Could not create or load wallet".

  3. **Test fix in `tests/integration/full_round.rs`:** the hardcoded test WIFs
     had invalid Base58 checksums and would `panic!` on `PrivateKey::from_wif`.
     Replaced with deterministic valid regtest WIFs generated via
     `sha256("blindjoin-test-key-{A,B,C}")` → WIF encode.

- [x] **Integration test harness reliability** — all four findings closed by
  v1.3 Phase 9 (shipped 2026-05-27, PR #7). See "Resolved 2026-05-27" entry above
  for the closure breakdown. The single remaining item — Tor connection-cap
  runtime test (Phase 8 HUMAN-UAT #3) — remains deferred to v1.4+ pending a
  dedicated Tor-mode integration harness (arti + flooding client, ≥257 concurrent
  `.onion` streams). Tracked in `.planning/REQUIREMENTS.md` "Future Requirements"
  section.

- [x] **CI hygiene: full RUSTSEC-2026-0097 closure + Node 24 opt-in (quick
  task 260526-d7m + follow-up).** Bumped all three transitively-resolved rand
  tracks to their patched versions: `rand` 0.8.5 → 0.8.6 (closed initial 3
  Dependabot alerts), then 0.9.2 → 0.9.3 (via arti-client; closed alert #7
  after Dependabot's database expanded) and 0.10.0 → 0.10.1 (via
  blind-rsa-signatures; closed alert #1). All three are lockfile-only patch
  bumps within existing `"0.8"` / `"0.9"` / `"0.10"` semver constraints — no
  Cargo.toml edits. With every rand instance on a patched version, the
  `RUSTSEC-2026-0097` ignore in `.cargo/audit.toml` is no longer load-bearing
  and was removed entirely; cargo audit exits 0 with zero warnings.

  Added a workflow-root `env: FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"`
  block to `.github/workflows/{ci,release,docker}.yml` so all JS actions
  (`actions/checkout@v6.0.2`, `dtolnay/rust-toolchain@stable`, etc.) execute
  on Node 24 ahead of GitHub's June 2026 forced flip — silences the
  deprecation annotation without bumping pinned action SHAs. CI annotation
  language shifted from "may not work as expected" to "being forced to run
  on Node.js 24" — confirms the opt-in works.

  Notable finding: Dependabot's `vulnerable_version_range` field was narrower
  than RustSec's actual advisory coverage on the initial scan — the database
  expanded later to surface 0.9.x / 0.10.x instances that cargo audit had
  flagged from the start. Lesson: cross-check `cargo audit` output even when
  the Dependabot UI says "all resolved."

- [x] **B-01: Public-endpoint hardening shipped (v1.2 Phase 8).** Per-route
  rate limits via `tower_governor` 0.8 (reads 60/min, writes 30/min by default)
  return HTTP 429 + `Retry-After` + a `RATE_LIMITED` JSON envelope; uniform
  `tower_http::timeout::TimeoutLayer` (default 30s) returns HTTP 408 on stall;
  Tor accept-loop bounded by a `tokio::sync::Semaphore` (default 256) wrapped
  in a load-bearing `ConnectionGuard` RAII type. Four operator-tunable knobs
  on `[coordinator]` (`rate_limit_info_per_min`, `rate_limit_writes_per_min`,
  `request_timeout_secs`, `max_concurrent_connections`), all validated at
  startup via `CoordinatorConfig::validate()`. Release builds refuse to start
  in clearnet mode unless `BLINDJOIN_ALLOW_CLEARNET=1` is explicitly set.
  Rate limiter uses `GlobalKeyExtractor` (Tor-safe — `PeerIpKeyExtractor`
  would have been a critical bug). Integration test at
  `tests/integration/rate_limiting.rs` exercises both 429 and 408 end-to-end
  when bitcoind is available. Code review (`.planning/phases/08-public-endpoint-hardening/08-REVIEW.md`)
  found 2 BLOCKER + 6 WARNING items, all fixed in 7 atomic `fix(08): ...`
  commits before the phase landed. Phase verification 11/11 must-haves
  verified statically; 3 runtime items deferred to `08-HUMAN-UAT.md` for CI
  sign-off with bitcoind on PATH.

- [x] **Coordinator now bootstraps a round in production.** v1.1 shipped with the
  Idle→InputReg transition only inside `#[cfg(test)]` blocks; in production the
  coordinator stayed in Idle forever and every API request returned WRONG_PHASE.
  Promoted `start_round()` out of test scaffolding into
  `coordinator/src/round/manager.rs`, extracted `main`'s body into
  `coordinator::run()` so the integration test can spawn the real startup path,
  and extended the phase monitor to re-invoke `start_round()` on every return
  to Idle (continuous-rounds policy). New integration test
  `tests/integration/round_bootstrap.rs` exercises the in-process `run()` path
  and asserts InputReg + non-null RSA pubkey. Removed the
  `build_input_reg_round_state` test backdoor that masked this gap from v1.1
  verification — `full_round.rs` now delegates to the real `start_round()`.

- [x] **Ban list persists across coordinator restart in Docker.** The relative
  default `ban_list.jsonl` path resolved to the container's ephemeral working
  dir; restart wiped the list. Added `coordinator-data:/app/data` volume to
  `docker/docker-compose.yml`, set `BLINDJOIN__COORDINATOR__BAN_FILE_PATH` env
  var, and `RUN mkdir -p /app/data` in the Dockerfile. New integration test
  `tests/integration/ban_list_persistence.rs` exercises the production write
  path (`append_ban_entry`) and read path (`load_unexpired_entries`).

- [x] **CI runs integration tests.** v1.1 CI ran `cargo test --workspace --lib`
  which excluded the entire `tests/` directory — directly enabling the
  round-bootstrap regression to ship undetected. Switched to `--all-targets`,
  added `push: branches: [main]` trigger, and added a `coordinator binary
  builds` smoke job.

- [x] **Verification template hardened.** Added the heuristic to
  `~/.claude/agents/gsd-verifier.md`: when test setup constructs a production
  type, the verifier must cite the production analog (function + file:line) or
  mark the must-have as FAILED. Project-side record at
  `.planning/workstreams/fix-verification-gap/VERIFICATION-HEURISTIC.md`.

- [x] **Backdoor inventory complete.** Audited ~150 files for test-only
  helpers that construct production state types. 0 HIGH, 2 MEDIUM, 7 LOW. See
  `.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md`.

## Resolved 2026-05-25

- [x] **Integration tests now compile.** Reconciled struct/API drift in
  `tests/integration/full_round.rs` against current `CoordinatorSection`
  (`tor_mode`), `CoordinatorConfig` (`discovery`), and `Client::poll_until_phase`
  (`Duration` arg) signatures. Also fixed a needless-borrow lint in
  `client/src/round/sign.rs` and applied related lints in the integration
  test. CI clippy step upgraded to `--all-targets` so future test-code drift
  surfaces as a CI failure rather than silent rot. Integration tests still
  require a live bitcoind to run; without one they skip gracefully.

- [x] **`cargo audit` CI step is now blocking** (was previously
  `continue-on-error: true`). Documented residual-risk advisories declared in
  `.cargo/audit.toml`, each with a written rationale. `rustls-webpki` bumped
  to 0.103.13 to clear three open advisories.

## Open

### Tech debt
- [ ] **Migrate `make_input_reg_state` and `make_signing_state` to use production
  state-machine transitions.** Both build `RoundStateInner` via struct literal
  in `#[cfg(test)]` blocks; `make_signing_state` additionally does direct
  `state.phase = Phase::Signing` assignment (bypasses validator) and uses a
  placeholder RSA key. Currently flagged with `TODO(fix-verification-gap)`
  comments. Detail in
  [`.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md`](.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md)
  findings #1 and #2. Estimated ~30 min refactor.

### Scoped features (see BACKLOG.md)
- [ ] **B-03:** Dynamic fee estimation (mempool-aware, safety margin, RBF)

These are deferred features with full scoping in [`.planning/BACKLOG.md`](.planning/BACKLOG.md). **B-01 shipped 2026-05-26** as Phase 8 of the v1.2 Production Readiness milestone (see Resolved above). **B-02 shipped 2026-05-31** as the v1.4 BIP-322 Multi-Script Support milestone (see Resolved above). B-03 remains a candidate for future milestones (pre-mainnet requirement).
