# Changelog

All notable changes to blindjoin are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/).

Each entry is a high-level summary of what shipped in the milestone — one
or two bullets per requirement-class. For the engineering detail behind
each milestone see [`.planning/MILESTONES.md`](.planning/MILESTONES.md)
and [`.planning/RETROSPECTIVE.md`](.planning/RETROSPECTIVE.md); for the
threat-model treatment of v1.4 / v1.5 invariants see
[`docs/AUDIT-CHARTER.md`](docs/AUDIT-CHARTER.md).

## [Unreleased]

## [1.8.1] — 2026-08-03

Security patch: move off the yanked `bip322 0.0.10` onto the patched `0.0.11`.

### Security

- **Bump `bip322` to the patched `=0.0.11`.** The upstream crate fixed the
  BIP-322 key-binding soundness gap in `0.0.11` (commit `e8accbe`): for
  P2WPKH / P2SH-P2WPKH it now derives the expected `scriptPubKey` from the
  witness public key and requires equality with the challenged address, failing
  with `PublicKeyMismatch` otherwise (the witness key must also be compressed).
  Versions `0.0.6`–`0.0.10` were **yanked from crates.io**, so the previous
  `=0.0.10` pin sat on a yanked crate and would break a fresh `cargo build` /
  lockfile regeneration. blindjoin's own key-binding guard (added in v1.8.0)
  already closed this gap defensively and is **retained as defense-in-depth** —
  it fires first with a distinct `WitnessKeyMismatch`, so proof soundness never
  depends on the crate alone. The `bip322-pin-check` CI gate now enforces
  `=0.0.11`. Transitive `snafu` (0.8→0.9) and `base64` (adds 0.23) bumps come
  with it; `cargo audit` allowed-warnings dropped 8→7 (the bip322 yank cleared).
  All BIP-322 tests (guard regression, cross-shape, per-script vectors,
  roundtrips) pass unchanged against `0.0.11`.

## [1.8.0] — 2026-08-02

External security review (no criticals). This release resolves every high-, medium-,
and low-severity finding from an external review, a separately-disclosed BIP-322
ownership-proof soundness bug, and lands the M5b broadcast-lock restructure. All work
is covered by tests (coordinator lib 124, full integration suite green).

### Security

- **BIP-322 ownership-proof bypass for P2WPKH / P2SH-P2WPKH.** The pinned
  `bip322 = "=0.0.10"` verifier checks a witness signature against the public key
  carried *in the witness* but never checks that key is the one the address /
  scriptPubKey commits to (its "key-mismatch" check compares the witness key with
  itself). An attacker could therefore sign the BIP-322 challenge with **their own
  unrelated key** and have it accepted as ownership of a **victim's** UTXO. This does
  not let them spend the coin (they still lack its real key), but it defeats the
  ownership *gate* — letting them register UTXOs they do not control, disrupt CoinJoin
  rounds, and degrade availability/privacy. Affected: P2WPKH `0.0.6`–`0.0.10`,
  P2SH-P2WPKH `0.0.7`–`0.0.10`; P2TR unaffected. blindjoin now enforces a key-binding
  guard in `shared/src/bip322/mod.rs::verify_via_bip322_crate` before delegating to
  the crate: for non-P2TR scripts it requires a 2-item witness, parses `witness[1]` as
  a compressed `bitcoin::PublicKey`, and rejects unless
  `address.is_related_to_pubkey(&pubkey)` (new `Bip322Error::WitnessKeyMismatch`).
  Both OwnershipProof envelopes (v1 legacy and v2 PSBT) funnel through this one point.
  The guard is permanent and stays even after the crate is patched. New regression
  tests reject unrelated-key witnesses for both P2WPKH and P2SH-P2WPKH and confirm
  honest proofs still verify. Corrected the `p2wpkh.rs` / `p2sh_p2wpkh.rs` /
  `detect_script_type` doc comments that wrongly asserted the crate performed the
  HASH160 cross-check internally. Reported via private disclosure by an anonymous
  security researcher; upstream maintainer contacted for a patched release.

- **Registration fee estimate under-charged, wedging rounds and mass-banning honest
  participants (security/availability).** The input-registration gate estimated each
  participant's fee share at `max_participants`, but the per-participant share is
  largest at the *smallest* round size — so a UTXO funded for a near-minimum margin
  passed registration then failed `InsufficientFunds` when the round finalized below
  max. That failed the whole PSBT build for *every* signer, so the signing timeout
  banned all of them for the coordinator's own arithmetic. The gate now estimates at
  `min_participants` (the true worst case). Regression tests pin the monotonicity
  bound and the concrete boundary.
- **Post-registration double-spend griefing escaped blame (security).** A participant
  could register, submit a valid partial signature, then double-spend its registered
  UTXO in the mempool — the CoinJoin was rejected at broadcast, the round wedged until
  timeout, and because everyone had "signed" the blame path banned nobody. On a
  broadcast failure the coordinator now re-validates every input (mempool-aware),
  bans the ones provably spent, and ends the round with attribution. Unattributed
  failures (e.g. a static fee too low for the mempool) are counted toward the
  consecutive-blame cap so they can't fast-churn forever.
- **Consecutive-blame cap was inert and its counter drifted (correctness).** The
  "full abort after N blame rounds" cap never actually paused anything, and the
  counter was never reset on a successful broadcast, so it wasn't counting
  *consecutive* failures. A full abort now pauses the round re-armer for a
  configurable backoff, and a successful broadcast resets the counter.
- **Output tokens were transferable bearer assets (privacy/correctness).** The
  coordinator verified a blind-signed token's signature but never checked that the
  token was issued for the output address being registered, letting a token be
  redeemed to any address — contradicting the protocol's binding requirement. Output
  registration now enforces `token == H("blindjoin-v1" ‖ output_script ‖ amount)`, so
  tokens are non-transferable as specified.
- **Canonical CoinJoin PSBT ordering depended on HashMap iteration order
  (availability).** The display, per-signature-verification, and broadcast PSBTs were
  byte-identical only by accident (same map instance, registration closed). Inputs
  are now ordered canonically by outpoint (BIP-69), making byte-identity an explicit
  invariant — one refactor away from a silent sighash-mismatch outage otherwise.
- **Intermittent BIP-322 ownership-proof rejection on ECDSA descriptor wallets
  (security/availability).** The pinned `bip322 = "=0.0.10"` verifier only accepts
  ECDSA witness signatures of length 71/72 bytes, but bdk's deterministic signer
  emits ~5% at 70/73 bytes (valid, but rejected as malformed). The WIF client path
  already grinded signatures to 71/72 via `shared::bip322::sign_simple`; the
  **descriptor** path (`generate` / `from_descriptor`) signed proofs via bdk with
  no grinding, so any P2SH-P2WPKH or P2WPKH descriptor-wallet client had a ~5%+
  chance per round of having its ownership proof silently rejected
  (`register_input` → 400 `INVALID_PROOF`). ECDSA descriptor proofs now route
  through `sign_simple` (deriving the index-0 leaf key from the descriptor xprv
  when it controls the registered UTXO; non-index-0 falls back to bdk). P2TR is
  unaffected (fixed-length Schnorr). New regression test
  `descriptor_ecdsa_proofs_grind_to_verifiable_length` (64 seeds); root cause and
  detection notes in `docs/solutions/bip322-ecdsa-signature-length-flake.md`. This
  was surfacing as the intermittent `mixed_script_e2e` CI flake.
- **Signet dev-stack Bitcoin Core RPC was exposed to the open internet.** The Docker
  bitcoind ran with `rpcallowip=0.0.0.0/0` and published `38332` on the host's
  `0.0.0.0`, with well-known dev credentials — anyone reaching the host got full node
  RPC. `rpcallowip` is now scoped to loopback + the docker private range, and the
  published port is bound to `127.0.0.1`. The coordinator reaches bitcoind over the
  compose network. Documented that the dev credentials must be changed for any
  non-localhost deployment.
- **Dependency advisories cleared.** Bumped `quinn-proto` `0.11.14`→`0.11.16`
  (RUSTSEC-2026-0185, high — remote memory exhaustion from unbounded out-of-order
  QUIC stream reassembly, transitive via `reqwest`), and `serde_with` `3.18`→`3.21`
  (GHSA-7gcf-g7xr-8hxj) + `cmov` `0.5.3`→`0.5.4` (GHSA-3rjw-m598-pq24) to clear the
  two moderate Dependabot advisories.

### Changed

- **A bad coordinator config is now a fatal startup error — no silent fallback to
  defaults.** Previously any config load error (one typo) logged a warning and booted
  hardcoded defaults (signet, loopback RPC, clearnet), so a mainnet-intended daemon
  could come up as something else. Load/validation failure now aborts startup. An
  unrecognized `network.bitcoin_network` is rejected outright instead of silently
  mapping to signet. **The Docker Compose stack now specifies the full round config
  explicitly** (it previously relied on the silent fallback). Startup validation also
  gained round-parameter sanity checks: `min_participants ≥ 2`, `min ≤ max`,
  `max_participants ≤ 100` (also keeps the derived broadcast watchdog structurally
  ahead of the worst-case finalize), non-dust `denomination_sats`, `fee_rate ≥ 1`,
  and non-zero phase timeouts.
- **Broadcast RPC now runs outside the round write lock (M5b).** The final signer's
  `testmempoolaccept` + `sendrawtransaction` (and the double-spend re-validation) used
  to hold the `RoundState` write lock, stalling every handler for the RPC duration.
  Signing now records + assembles under the lock and moves the round `Signing →
  Broadcast` (so the signing-timeout monitor stops watching it); a detached task then
  broadcasts off the lock and re-acquires to finalize `Broadcast → Idle` (success) or
  `Broadcast → Blame → Idle` (failure, with attributed bans). A new FSM edge
  `Broadcast → Blame` and a run.rs Broadcast watchdog (derived from `max_participants`
  so it can never preempt a live finalize) back it up. `/info` `round_state` is now
  documented as a transient in-flight state; PROTOCOL.md's Broadcast section is written.
- **Client per-request timeouts + error-body propagation.** The reqwest clients
  (clearnet + Tor) now use a 60s per-request timeout (a hung coordinator previously
  blocked a write/poll indefinitely), and `post_input`/`post_output` surface the
  coordinator's JSON error envelope on failure instead of a bare "HTTP 400". The
  client also treats HTTP 429 as retryable (Retry-After, bounded) on every read and
  write call.
- **Ban file is compacted on startup.** The append-only ban file accumulated expired
  records unbounded; the coordinator now rewrites it (atomically) with only unexpired
  entries at boot.
- **Removed dead fields.** `RegisteredInput.blind_sig_hash` (stored, never consulted),
  `RoundStateInner.change_addresses` (write-only duplicate of `change_address`), and
  `BitcoinRpc::getrawtransaction` (unused).
- **Rate-limit defaults raised: reads 60→600/min, writes 30→120/min.** The old reads
  default (1 token/sec) exactly matched a single client polling `/info`, so two honest
  clients starved each other with no attacker present. The reference client now treats
  HTTP 429 as retryable (honoring `Retry-After`, bounded retries) on every read and
  write call rather than aborting the round — the global bucket can't be fixed by
  sizing alone under Tor, so graceful client backoff is the load-bearing mitigation.
- **RSA round keygen moved off the round write lock.** The per-round RSA-2048 keypair
  is now generated on a blocking thread pool and installed under the lock only
  briefly, so keygen no longer stalls `/info` reads and every handler once per round.
- **Bitcoin Core RPC calls now have an explicit 10s per-request timeout**, so a hung
  `bitcoind` fails the call cleanly instead of blocking indefinitely.
- **`InputRegState.participants_registered` removed.** It carried the
  coordinator-reported participant count, used only by the old denomination-count
  check, which was defeated (a first registrant sees `0`, making the check vacuous).
  Superseded by the PSBT-derived anonymity floor above.

### Added

- **`coordinator.blame_full_abort_backoff_secs` (default 300).** Seconds the round
  re-armer pauses after a full-abort blame outcome instead of instantly restarting
  into the same wedge; `0` restarts immediately (legacy behavior).
- **Client `--no-print-secrets` flag.** Suppresses printing the mnemonic and
  descriptors to stdout on `--generate-wallet` (still written to `descriptors.txt`,
  mode 0600) — for scripted, shared-terminal, or logged environments.
- **Property-based FSM tests (proptest).** Arbitrary sequences of attempted phase
  transitions are checked against `can_transition_to` (valid-edge-only, phase
  unchanged on an invalid edge, fresh `round_id` minted and state cleared exactly on
  `→ Idle`). Fuzz targets for the untrusted parse paths remain a deferred follow-up.
- **Per-input partial-signature verification at submission (security, H3).** The
  coordinator now cryptographically verifies each partial signature against the
  canonical CoinJoin transaction's sighash *before* recording it
  (`coordinator/src/bitcoin/sig_verify.rs`, wired into
  `coordinator/src/round/signing.rs::process_sign`). Previously the witness bytes
  were stored unchecked and only an aggregate `testmempoolaccept` caught a bad
  signature at broadcast — by which point every participant had "signed", so the
  blame path banned nobody and the round aborted. A single participant could
  therefore destroy every round at zero cost and escape blame entirely. An
  invalid submission is now rejected with `INVALID_SIGNATURE` and not recorded,
  so the sender remains a bannable non-signer at the signing deadline. Covers
  P2WPKH, P2SH-P2WPKH (BIP-143), and P2TR key-spend (BIP-341), and enforces
  SIGHASH_ALL / SIGHASH_DEFAULT (a non-ALL flag is refused). The verified
  transaction is built by the same `build_canonical_psbt` that
  `assemble_and_broadcast` broadcasts, so the sighash signed is exactly the
  sighash spent. New `INVALID_SIGNATURE` error code. 9 new unit tests (valid +
  tampered + wrong-key + non-SIGHASH_ALL + wrong-dispatch across all three script
  types, plus a real-signature happy path through `process_sign`).


- **Client-side fund-safety validation before signing (security, C1/H1).** The
  client now binds its own economic outcome to the PSBT it is about to sign,
  instead of trusting coordinator-reported numbers. `verify_and_sign`
  (`client/src/round/sign.rs`) refuses to sign unless: (a) our coinjoin output
  is present **and valued at exactly the denomination** — previously only its
  presence was checked, so a coordinator could short it to dust and pocket the
  difference (SIGHASH_ALL commits to whatever outputs the PSBT carries, and
  bitcoind accepts the result as valid); (b) the fee **we** pay, computed from
  the PSBT itself (`our input − our denomination output − our change output`),
  is within a cap — previously the bound used the coordinator's self-reported
  `fee_per_participant_sats` against the already-shortable output value, so it
  was unenforceable; (c) the PSBT carries at least `min_anonymity_set` distinct
  denomination outputs, counted **from the PSBT** rather than the
  coordinator-reported participant count (which a coordinator running one victim
  plus its own sybils sets to anything it likes). Two new client config knobs:
  `--min-anonymity-set` / `BLINDJOIN_MIN_ANONYMITY_SET` (default 3) and
  `--max-fee-sats` / `BLINDJOIN_MAX_FEE_SATS` (default: denomination / 10). 12
  new unit tests cover the shorted-output, fee-theft, floor, and PSBT-reader
  paths.

### Fixed

- **Ban evasion via non-canonical outpoint (security).** The input-registration
  ban check hashed the raw request string, but bans are stored under the
  canonical `{txid}:{vout}` form. Because `parse_outpoint` accepts non-canonical
  spellings like `txid:00` (vout 0), a banned griefer could re-register the same
  UTXO as `:00` and slip past the ban, defeating the blame mechanism at zero
  cost. The check now hashes the canonical form of the parsed `OutPoint`
  (`coordinator/src/api/handlers.rs`). Regression test:
  `parse_outpoint_canonicalizes_leading_zero_vout`.
- **Pre-broadcast redemption oracle at output registration (security).**
  `register_output_logic` ran the replay check before RSA signature
  verification, so an unauthenticated caller who guessed a candidate token
  message (`SHA-256(prefix || output_script || amount)`) could distinguish
  `TOKEN_ALREADY_USED` from `INVALID_TOKEN` and learn whether a given output was
  already redeemed in the live round — before broadcast, when the output set is
  meant to be secret. Signature verification now runs first
  (`coordinator/src/round/output_reg.rs`); a caller without a coordinator-issued
  signature always gets `INVALID_TOKEN` and learns nothing. Regression test:
  `output_reg_verifies_signature_before_replay_check`.

- **Docker stack started a release coordinator that refused to boot.** The
  release image builds `--release`, where `tor_mode = false` triggers the WR-04
  clearnet-refusal guardrail unless `BLINDJOIN_ALLOW_CLEARNET=1` is set. Neither
  the compose stack nor the README `docker run` one-liner set it, so the
  coordinator exited at startup and `restart: unless-stopped` looped it — the
  "five minutes to a round" quickstart could not work. Both deploy paths are
  clearnet-internal by design (the compose healthcheck and bot reach the
  coordinator over clearnet; `tor_mode` would create a `.onion` with no clearnet
  listener and break both), so both now set `BLINDJOIN_ALLOW_CLEARNET=1`. The
  README one-liner additionally binds its published port to loopback
  (`127.0.0.1:8080:8080`) and documents the `tor_mode=true` alternative for a
  built-in arti hidden service.

## [1.7.0] — 2026-06-03

### Simplification milestone

Net: -7 GitHub Actions, -1 docs file, -1 docker-compose file, -1 composite
action, -1 reproducibility recipe and dispatch verifier, -2 CI guard jobs
replaced by 1 stricter gate. Operator verify shifts from `cosign verify` to
`gh attestation verify` (no extra install). Release procedure shrinks from
a separate `docs/RELEASING.md` runbook to a 3-item pre-tag checklist in
`CONTRIBUTING.md`, with the two prior-silent foot-guns (2-part tags,
missing CHANGELOG entry) now hard-failing in CI before any build runs.
SLSA build provenance via `actions/attest-build-provenance` is the sole
supply-chain artifact going forward.

### Added

- `release-gate` job in `release.yml` and `docker.yml`. Runs first on
  any `push: tags: ['v*']` and hard-fails the build with a readable
  error message if (a) the tag isn't `vMAJOR.MINOR.PATCH` 3-part semver
  — `docker/metadata-action` silently produces zero image tags for
  2-part tags, so `v1.0`/`v1.1`/`v1.3` all silently failed before this
  gate existed — or (b) `CHANGELOG.md` has no `## [X.Y.Z]` section for
  the tag being pushed. Converts two prose foot-guns into runtime
  failures at the point of the mistake.

### Changed

- `docker.yml` metadata-action publishes `:latest` on every tagged
  release (in addition to `:X.Y.Z` and `:X.Y`).
- README badge row: "Signed by cosign + SLSA" → "Build provenance: SLSA
  v1.0" to match the cosign + SBOM strip. The "Reproducible (byte-equal)"
  badge was already removed alongside the reproducible pipeline.
- Docs hygiene: PKARR mentions link to <https://github.com/pubky/pkarr>;
  RFC 9474 mentions link to <https://www.rfc-editor.org/rfc/rfc9474>.

### Fixed

- `append_ban_entry` (`coordinator/src/round/blame.rs`) now calls
  `sync_all()` after each write, so a ban that returned `Ok(())` to the
  caller survives the power-loss / kernel-panic / OOM-kill window a
  solo-VPS operator actually hits. The ban file lives in the
  `coordinator-data` Docker volume — back it up alongside
  `coordinator-keys` if you want bans to outlive the host. Surfaced in
  `SECURITY.md` operating notes.
- Two intermittent flakes in the integration test suite eliminated.

### Removed

- `docker/docker-compose.coordinator-only.yml`. The file duplicated
  ~30 lines of the main compose file's coordinator service definition
  AND hardcoded the release tag in two places (file + the README
  paragraph that advertised it), so every release would have silently
  let the prod-launch example point at a stale image unless someone
  remembered to bump both. Replaced with a single `docker run` example
  in the README's Quick Start that uses the new `:latest` tag — no
  duplicate service definition, no per-release update, no new file.

- `.github/actions/read-base-digests/` composite action, `docker/digests.txt`
  canonical manifest, `.github/CODEOWNERS` file. Base-image sha256 digests
  now live inline in `docker/Dockerfile`'s two `FROM` lines (one image per
  PR to bump). The composite action's strict regex parser, 7-step contract,
  "Refusing to build without a valid manifest" auditor-pose error strings,
  and dual-source-of-truth between digests.txt + the Dockerfile ARGs all
  went with it. Outside-PR review is still gated by the `main` ruleset's
  `required_approving_review_count: 1` (see docs/branch-protection.md);
  the now-empty `require_code_owner_review: true` setting can be toggled
  off in the GitHub UI at the maintainer's leisure.

- `docs/RELEASING.md`. After the cosign + SBOM strip, the file had four
  remaining items: a `gh 2.50+` prereq, "watch release.yml until green,"
  a local `gh attestation verify` pre-flight, and a manual base-image
  digest inspect. The pre-flight re-confirmed what CI's
  `attest-build-provenance` step had already attested (same Rekor entry,
  same signer); the digest inspect already lives in CONTRIBUTING.md's
  `§Bumping base-image digests`; "watch CI" doesn't need a paragraph.
  The "remember to do X" prose was the same shape as the `[CONTRIBUTING.md:96]`
  cosign-verify-blob reference that survived the cosign strip — a
  load-bearing markdown sentence is a future drift opportunity.
  Surviving items folded into CONTRIBUTING.md `§Tagging releases` as a
  3-item pre-tag checklist; README.md and SECURITY.md base-image refs
  re-pointed at CONTRIBUTING.md (the actual home of that procedure).
  The tag-format and CHANGELOG-entry foot-guns the prose tried to
  prevent now hard-fail in CI via the `release-gate` job.

- Cosign blob/image signing (`sigstore/cosign-installer` step, `cosign sign-blob`
  + `cosign sign` steps, `blindjoin-linux-amd64.tar.gz.bundle` Release asset, the
  `ghcr.io/.../blindjoin-<image>@sha256-<HEX>.sig` legacy referrer) and SPDX SBOM
  attestation (`anchore/sbom-action` + `actions/attest-sbom`). For a solo project
  with no operator base, two artifact families bound to the same workflow
  identity by the same OIDC chain were one too many — the SLSA build provenance
  attestation produced by `actions/attest-build-provenance` strictly dominates
  the bare cosign signature (same signer, same Rekor entry, plus a verifiable
  build claim), and SBOM attestations exist for downstream scanners that don't
  exist here. Operator verify switches from `cosign verify` / `cosign verify-blob`
  (which required the `cosign >= 2.6.3, < 3.0.0` install + the pin-range docs)
  to `gh attestation verify` (no extra install on the operator's box). Also
  removed: the `sigstore-pin-check` CI job (it watched four actions; only one
  remains and a normal SHA-pin review covers it), the `cosign 2.6.3+`
  prerequisite in `docs/RELEASING.md`, and the `cosign verify-blob` pre-flight
  recipe (replaced with `gh attestation verify`). The SLSA provenance bundles
  already published to Rekor for v1.0–v1.6 stay there forever; the strip only
  changes what `vX.Y.Z` going forward will produce.

- Byte-equal reproducible build pipeline (`reproducible-verify.yml`,
  `docs/REPRODUCIBLE-BUILD.md`, `EXPECTED_SHA256` env, `SOURCE_DATE_EPOCH`
  derivation, `RUSTFLAGS=--remap-path-prefix=...`, `CARGO_INCREMENTAL=0`,
  deterministic 5-flag tar + `gzip -n`, `ubuntu-24.04` build-job pin). For
  a solo MIT project the cosign + SLSA provenance + committed
  `Cargo.lock` / `rust-toolchain.toml` / `Dockerfile` already cover the
  "this binary came from the official tagged workflow" question; byte-
  equality only catches a Sigstore-compromise scenario that isn't worth
  the ongoing fragility cost (every base-image rotation, Rust bump, or
  runner change is a potential divergence to debug). Anyone who wants to
  rebuild can still do so from source — see README §Build from Source.
  `cargo build --release --locked` (catches a stale `Cargo.lock`),
  `[profile.release] strip = "symbols"` (binary size), and
  `rust-toolchain.toml` (dev convenience) all stay. release.yml build
  job switched from `ubuntu-24.04` → `ubuntu-latest`.

## [1.6.0] — 2026-06-03

### Supply-chain attestation

Closed the v1.5 unsigned-build supply-chain gap. Every release tarball and ghcr.io image is cosign-signed (keyless OIDC, no maintainer key custody), SLSA v1.0 provenance-attested, and SPDX SBOM-attested. Base-image digests pin in `docker/digests.txt`. Release tarballs rebuild byte-for-byte from source on `ubuntu-24.04`.

12 of 14 v1 requirements shipped. SIGN-03 (PGP YubiKey path) deferred indefinitely 2026-06-02 (cosign+SLSA covers the threat model; PGP would be ongoing key-rotation cost for negligible solo-project benefit). REPRO-04 (reproducible-builds.org registry submission) descoped 2026-06-03.

- **Base-image digest pinning (DRIFT-01/02/03):** `docker/digests.txt` is the canonical manifest for `debian:bookworm-slim` and `lukemathwalker/cargo-chef:latest-rust-1`. `release.yml` + `docker.yml` thread the values into builds via the [`.github/actions/read-base-digests/`](.github/actions/read-base-digests/) composite action. [`digest-drift-check.yml`](.github/workflows/digest-drift-check.yml) runs on `workflow_dispatch`, exits non-zero on drift. `docker/digests.txt` is CODEOWNERS-gated.
- **Image attestations (ATTEST-01..04):** Every tagged ghcr.io push gets `cosign sign` (keyless OIDC), `actions/attest-build-provenance` (SLSA v1.0), `actions/attest-sbom` + `anchore/sbom-action` (SPDX SBOM via Syft). cosign-installer pinned at v3.10.1 / cosign 2.6.3. `sigstore-pin-check` CI job catches floating tags on the four sigstore/sbom actions.
- **Release tarball signatures (SIGN-01/02):** Every `blindjoin-linux-amd64.tar.gz` ships a cosign `.bundle` companion and a SLSA `.sigstore` provenance bundle via the same `actions/attest-build-provenance` machinery.
- **Reproducible builds (REPRO-01/02/03):** `rust-toolchain.toml` pins rustc 1.95.0; `Cargo.toml` adds `[profile.release] strip = "symbols"`; `release.yml` build job runs on the pinned `ubuntu-24.04` with deterministic `RUSTFLAGS` + `CARGO_INCREMENTAL=0` + `SOURCE_DATE_EPOCH` from the tagged commit + `cargo build --release --locked` + deterministic 5-flag `tar` piped through `gzip -n`. Recipe + per-tag expected sha256: [docs/REPRODUCIBLE-BUILD.md](docs/REPRODUCIBLE-BUILD.md). On-demand verifier: [`reproducible-verify.yml`](.github/workflows/reproducible-verify.yml).
- **SECURITY.md** rewritten with single-recipe verify commands for image and tarball signatures and a single-line pointer to the reproducibility recipe.

**Theater strip after Phase 25 close** (commits `bf102ab` + `54e5ea5`, ~1,400 lines deleted): removed scaffolded auto-issue creation systems with two-title schemes + label-create + dedup (both digest-drift and reproducibility verifiers), removed monthly/daily cron schedules in favor of `workflow_dispatch:` only, collapsed multi-path verify recipes to single commands, removed the colon-delimited expected-sha256 lookup file in favor of inline env values, removed the reproducible-builds.org registry submission procedure, removed `rust-toolchain-pin-check` and the two `crit-01-grep-check` CI gates (the latter enforced presence of a comment rather than behavior), removed SECURITY.md disclosure-policy ceremony and strikethrough bookkeeping, stripped planning cross-references and auditor-pose comments throughout workflow files. Real cryptographic verification and supply-chain pinning intact.

Tagged 2026-06-03. Reproducibility baseline captured post-tag by dispatching `reproducible-verify.yml` against the published v1.6.0 release and committing the rebuilt sha256 into `EXPECTED_SHA256:` + the markdown table.

## [1.5.0] — 2026-06-01

### Added

- **Production BIP-322 `sign` bodies for P2TR and P2SH-P2WPKH** in
  `shared::bip322`. P2TR uses BIP-341 Schnorr keypath via
  `sign_schnorr_no_aux_rand`; P2SH-P2WPKH uses BIP-143 ECDSA over the
  unwrapped P2WPKH redeem. Byte-equality with
  `BdkClientWallet::sign_bip322` proven empirically.
- **Per-script BIP-141 vbyte table** in `coordinator/src/bitcoin/tx.rs`
  (P2WPKH 68/31, P2TR 58/43 round-UP, P2SH-P2WPKH 91/32). Replaces the
  legacy P2WPKH-only constants. v1.4 P2WPKH-only `fee_share == 266`
  byte-equality preserved by regression test; mixed-script branch fires
  with a measurable per-participant delta.
- **External audit charter** at `docs/AUDIT-CHARTER.md` (574 LOC,
  8 H2 sections) covering in-scope modules with `file:symbol` references,
  threat models per module, the 9 cross-shape rejection properties, the
  v=2 PSBT handling boundary, the RSA SecretKey zeroization window, and
  residual risks with explicit dispositions.
- **`SECURITY.md`** at the repo root documenting the responsible
  disclosure policy, supported versions, audit-readiness status, and
  the known supply-chain gaps (unsigned Docker images, no reproducible-
  build pipeline at v1.5).
- **`CHANGELOG.md`** (this file).

### Changed

- **RSA SecretKey lifetime tightened from prose to type signature**:
  `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` is the bounded
  lifetime; SOLE FSM chokepoint is `transition_to(Phase::Idle)` (verified
  by grep — no other site sets `self.inner = None`). The Drop body on
  `RoundSecretKey` is empty-crypto (PII-safe `tracing::debug!` only);
  the transitive `rsa-0.9.10` `ZeroizeOnDrop` chain on `RsaPrivateKey`
  does the cryptographic work.
- **`.cargo/audit.toml`** refreshed: each of the three RUSTSEC ignores
  (RUSTSEC-2023-0071 Marvin Attack, RUSTSEC-2025-0141 bincode
  unmaintained, RUSTSEC-2024-0436 paste unmaintained) cites the charter
  anchor for its full rationale. The "best-effort" framing on
  RUSTSEC-2023-0071 is gone; the AUDIT-03 bounded-window mitigation is
  named explicitly.
- **README § Security Model** links into the charter.

### Removed

- `sign_simple_test_only` + per-script `sign_for_tests` helpers. All
  test callsites migrated to the production `sign_simple` dispatcher;
  the V1.4-CRIT-01 dispatcher-only invariant is now load-bearing at the
  type level with no test-only escape hatch.

### Security

- AUDIT-03 chokepoint result-discard pattern (`let _ =` on the 3
  success-path FSM trigger sites) accepted as a defense-in-depth gap
  and documented in `AUDIT-CHARTER.md` § Residual Risks: Protocol-level.
  Closure deferred to v1.6+; closure trigger = any future FSM
  concurrency-model or `can_transition_to`-edge change.

## [1.4.0] — 2026-05-31

### Added

- **Multi-script BIP-322 coordinator + client** accepting P2WPKH, P2TR,
  and P2SH-P2WPKH ownership proofs via a `match version` dispatcher
  with on-chain CRIT-01 cross-check (script_type derived from
  `script_pubkey`, never trusted from the client-declared field).
- **Upstream `bip322 = "=0.0.10"` crate** adopted via a 26-LOC zero-lossy
  adapter at `shared/src/bip322/mod.rs`, replacing the in-tree
  implementation. Pinned via the `bip322-pin-check` CI grep gate.
- **[PKARR](https://github.com/pubky/pkarr) record v0.2.0** with `sst`/`ost` compact field names
  advertising supported / output script types in 209 bytes (11-byte
  headroom under the 220-byte threshold). Clients reject mismatched
  coordinators with `DiscoveryError::UnsupportedScriptType` **before**
  opening any Tor circuit.
- **v1.3 ↔ v1.4 backwards-compat shim** — v1.4 client → v1.3 coordinator
  emits byte-identical v1.3 array-of-hex `OwnershipProof` via CD-7
  two-phase try-parse; v1.3 client → v1.4 coordinator verified inline
  against a pinned v1.3 binary SHA `05f21438`.
- **Liquidity bot script-type rotation** via `BLINDJOIN_BOT_SCRIPT_TYPES`
  CSV + per-round rotation counter file. Defeats V1.4-MIN-02
  uniform-script fingerprint.
- **Mixed-script end-to-end acceptance test**
  `mixed_script_e2e_three_clients_broadcast` (1× P2WPKH + 1× P2TR +
  1× P2SH-P2WPKH input through INPUT_REG → BROADCAST on regtest).

### Removed

- Coordinator's P2WPKH-only registration gate at
  `coordinator/src/bitcoin/utxo.rs`. Replaced by the multi-script
  dispatcher.

### Security

- **V1.4-CRIT-01 script_type spoofing** mitigated by making the
  dispatcher the only public verify entry on `shared::bip322` — direct
  per-script callers are now `pub(crate)` and statically unreachable
  from outside the crate.

## [1.3.0] — 2026-05-29

### Added

- **Pinned `bitcoind` v30.2 in CI**, PGP-key-fingerprint-verified +
  SHA-256-verified against `SHA256SUMS.asc`. Cached at runner level so
  cache hits cost ~0s; cache misses re-run the integrity gate.
- **`BitcoindGuard` RAII fixture** + **`require_bitcoind!()` macro**.
  Replaces `Box::leak`-based fixtures across all `tests/integration/`
  callsites; eliminates "passes locally, hangs in CI" class of bugs.
- **`CONTRIBUTING.md` canonical integration-test invocation pattern.**
- **CI grep gate for `corepc-node` feature pin** (REPAIR-02 invariant):
  every `corepc-node = ...` declaration in any `Cargo.toml` must carry
  an explicit `features = ...` clause, or the gate fails the build.

### Fixed

- **`full_round.rs` repaired** — all 8 tests pass against pinned
  `bitcoind` v31. Fixes spanned RSA SPKI handshake, bdk_wallet 2.3
  `trust_witness_utxo`, wire-format `Witness` consensus encoding, real
  on-chain `witness_utxo`, ban-check ordering, and error-body surfacing.

## [1.2.0] — 2026-05-26

### Added

- **Per-route rate limiting on the coordinator HTTP API** via
  `tower_governor`. Read split 60/min, write split 30/min. HTTP 429 +
  `Retry-After` + `RATE_LIMITED` JSON envelope on rejection.
- **Uniform request timeout** (HTTP 408) honoring `request_timeout_secs`
  from `coordinator.toml`.
- **Tor accept-loop connection cap** via `tokio::sync::Semaphore`
  (default 256) with a `ConnectionGuard` RAII pattern.
- **`GlobalKeyExtractor`** for rate limiting — Tor-safe.
  `PeerIpKeyExtractor` would break under Tor where peer-IP is uniform.
- **Operator-tunable knobs** via `coordinator.toml` and
  `BLINDJOIN__COORDINATOR__*` env vars, validated at startup.

### Security

- **Release clearnet refusal** — coordinator refuses to start a release
  build with a non-Tor listener configured (signet flag carves an
  explicit exception).

## [1.1.0] — 2026-04-10

### Added

- **CI/CD security pipeline**: `cargo test`, `cargo clippy`, `cargo audit`
  as gates on PRs and releases. All GitHub Actions SHA-pinned. SHA-256
  checksums on release archives. Per-job permission scoping.

### Fixed

- **Eliminated write-lock DoS**: moved async Bitcoin Core RPC call
  outside the `RoundState` write lock in `post_input` (AVAIL-01).
- **Eliminated key-deserialization DoS**: `RsaBlindSigner` parsed once
  at round creation and reused across requests (AVAIL-02).
- **Input validation hardening**: blinded-token size bounds, address
  pre-validation, duplicate partial-sig guard, fee-formula
  consolidation.

## [1.0.0] — 2026-04-09

### Added

- **CoinJoin coordinator with RSA blind signatures** ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) ensuring
  cryptographic input-output unlinkability.
- **Round state machine**: `IDLE → INPUT_REG → OUTPUT_REG → SIGNING →
  BROADCAST | BLAME → IDLE`.
- **Blame protocol**: non-signer detection, UTXO banning with
  persistence, automatic round restart with remaining participants.
- **Client CLI** with `bdk_wallet` key management, per-phase Tor circuit
  isolation (Alice / Bob), anti-censorship PSBT verification.
- **UTXO ownership proof via BIP-322** (P2WPKH at v1.0; multi-script
  added at v1.4).
- **[PKARR](https://github.com/pubky/pkarr) DHT discovery**: coordinators publish `.onion` addresses, round
  parameters, and status to Mainline. Clients resolve without
  hardcoded addresses.
- **Tor v3 hidden service** via `arti-client`. No clearnet endpoint in
  production.
- **Docker Compose stack** (`bitcoind` + coordinator + liquidity bot) —
  zero-to-CoinJoin in 5 minutes on signet.
- **Liquidity bot** auto-joins rounds for testing and cold-start.
- **Integration tests**: full round (3+ participants), blame protocol,
  adversarial scenarios on signet.
- **Pre-built binaries** via GitHub Releases; multi-arch Docker images
  via `ghcr.io`.

### Security

- All round state zeroed from memory after transaction broadcast.
- No logging of PII, IP addresses, or input-output mappings.
