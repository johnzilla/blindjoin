# blindjoin Architecture

This is the orientation map for anyone reading the blindjoin codebase — a
reviewer, an auditor, or a first-time contributor. It covers the workspace
layout, where each piece of the protocol lives, how data flows through a round,
and — most importantly for reviewers — the **invariants ledger**: the properties
the system depends on, where each is enforced, and the test that fails if it
breaks.

## Document map

| You want… | Read |
|---|---|
| What the protocol *is* (normative wire behavior) | [docs/PROTOCOL.md](docs/PROTOCOL.md) |
| The threat model (goals, adversaries, attack→response) | [docs/PROTOCOL.md § Security considerations](docs/PROTOCOL.md#security-considerations) |
| Code-level audit surface (file:symbol, zeroization chain, residual risks) | [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md) |
| How to build, run the test loops, and cut releases | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Coordinated vulnerability disclosure | [SECURITY.md](SECURITY.md) |
| **Module map, data flow, invariants, how to verify them** | this file |

## Trust model in one paragraph

blindjoin has one coordinator and many clients per round. **The coordinator never
holds keys and never holds funds**: it assembles and broadcasts a transaction that
clients sign with their own keys, and because every input signs `SIGHASH_ALL` it
cannot alter the outputs after collecting signatures. **The coordinator must not be
able to link an input to an output** — that is the primary security goal, and it is
structural (RFC 9474 blind signatures + per-phase Tor circuit isolation), not a
matter of coordinator good behaviour. A malicious coordinator can deny service;
DHT discovery answers that by making coordinators *replaceable*, not by making any
single round byzantine-fault-tolerant. See [the threat model](docs/PROTOCOL.md#security-considerations)
for the full treatment.

## Workspace layout

Four crates (`Cargo.toml` workspace members):

| Crate | Role | Holds keys? | Holds funds? |
|---|---|---|---|
| `coordinator/` | Runs the round state machine, issues blind signatures, verifies ownership proofs and partial signatures, assembles and broadcasts the CoinJoin, publishes the PKARR discovery record, maintains the ban list. | A per-round **ephemeral RSA** signing key only (destroyed at round end). Never any Bitcoin key. | No |
| `client/` | Discovers a coordinator, drives a wallet (BDK), builds ownership proofs, blinds/unblinds tokens, registers input and output on separate Tor circuits, signs its own input. | Yes — the participant's Bitcoin keys, via BDK descriptors. | The participant's own |
| `shared/` | Wire types, the BIP-322 ownership-proof dispatcher + per-script sign/verify bodies, token hashing, error taxonomy. No I/O, no state. | No | No |
| `liquidity-bot/` | Optional automated participant that joins rounds to provide/rotate liquidity (operator tool; dilutes the uniform-output fingerprint). | Its own wallet keys | Its own |

The crypto boundary is `shared/src/bip322/` (ownership proofs) and
`coordinator/src/blind/` (blind signatures). No custom cryptography is written in
this repo — these wrap `bip322`, `blind-rsa-signatures`, `rust-bitcoin`, and
`secp256k1`. See [CLAUDE.md § Constraints](CLAUDE.md) and the dependency rationale
in [docs/AUDIT-CHARTER.md § Out-of-Scope Components](docs/AUDIT-CHARTER.md#out-of-scope-components).

## Module map

### `coordinator/`

```
main.rs / run.rs        process entry; builds config, spawns the phase monitor,
                        the PKARR heartbeat, and the Tor/axum server
config.rs               layered config (file + env), boot validation
api/
  handlers.rs           HTTP endpoints: GET /info, POST /round/input, /round/output,
                        GET /round/tx, POST /round/sign
  middleware.rs         per-route rate limiting (tower), request logging
round/
  state.rs              RoundState FSM + RoundStateInner (sensitive, Drop-zeroized);
                        transition_to is the SOLE chokepoint that nulls round state
  manager.rs            round bootstrap; HMAC session-token generate/verify
  input_reg.rs          input-registration logic + quorum/timeout
  output_reg.rs         output-registration logic + missing-output detection
  signing.rs            process_sign (verify + record + Signing→Broadcast),
                        finalize_broadcast (off-lock broadcast + blame)
  blame.rs              non-signer detection, BanList, append-only ban file
bitcoin/
  rpc.rs                async JSON-RPC to Bitcoin Core (reqwest + corepc-types)
  utxo.rs               validate_utxo: chain-derived script type (CRIT-01)
  tx.rs                 build_coinjoin_psbt: the canonical transaction + fees
  sig_verify.rs         per-script partial-signature verification (H3)
  fee.rs                estimate_fee_share: the output-pessimistic registration gate
blind/
  rsa.rs                RsaBlindSigner (per-round ephemeral RSA), RoundSecretKey
discovery/
  pkarr_pub.rs          builds + signs the PKARR coordinator record
network/
  tor.rs                arti onion-service hosting (arti-axum glue)
```

### `client/`

```
main.rs                 CLI entry (clap); orchestrates the round
config.rs               client config (coordinator key/url, wallet descriptor)
discover.rs             PKARR discovery + pre-Tor fail-fast (version / network /
                        script-type compatibility)
tor.rs                  isolated Tor circuits — distinct circuit per phase
http.rs                 HTTP client over the Tor circuits
wallet.rs               BDK wallet; signs BIP-322 proofs and the input
round/
  input.rs              build the v=2 OwnershipProof PSBT envelope + blinded token
  output.rs             unblind + register output on the second circuit
  sign.rs               fetch /round/tx, verify round_id, sign, submit
```

### `shared/`

```
protocol.rs             all wire request/response types; WIRE_VERSION
bip322/
  mod.rs                verify_simple / sign_simple dispatchers (the only public
                        surface); detect_script_type; key-binding guard
  p2wpkh.rs p2tr.rs p2sh_p2wpkh.rs   per-script sign/verify (pub(crate) only)
token.rs                blind-token message hashing
errors.rs               ApiError + ErrorCode / Bip322Error taxonomy (PII-safe)
types.rs                shared primitives
```

## Data flow: one round

The normative phase-by-phase behavior is in
[docs/PROTOCOL.md § Round lifecycle](docs/PROTOCOL.md#round-lifecycle). The shape:

```
client                                            coordinator
------                                            -----------
discover (PKARR, verify sig/version/network) ───▶ publishes signed record to DHT
                                                  Idle → InputReg
POST /round/input   (circuit A / "Alice")   ───▶ validate UTXO + BIP-322 proof,
  { utxo, ownership_proof, blinded_token }        chain-derive script type,
                                            ◀───   blind-sign token + session token
unblind locally
POST /round/output  (circuit B / "Bob")     ───▶ verify unblinded sig; accept output
  { unblinded_token, sig, output_addr }           (no link to any input)
                                                  OutputReg → Signing
GET  /round/tx                              ◀───  canonical PSBT (byte-identical
  verify round_id, inspect, sign own input        across all build sites)
POST /round/sign                            ───▶ verify session token + partial sig
  { round_id, utxo, partial_sig, token }          against canonical sighash, record
                                                  last sig → assemble, Signing→Broadcast
                                                  broadcast off-lock → Idle (state zeroed)
```

Failure edges all funnel through `Blame → Idle`: missing outputs (unattributable
by design), signing-timeout non-signers (banned), and broadcast failure
(provably-spent inputs banned). See
[docs/PROTOCOL.md § Phase: Blame](docs/PROTOCOL.md#phase-blame).

## Invariants ledger

These are the properties the protocol's guarantees rest on. Each row names where
the invariant is enforced and the test that fails if it regresses — this is the
review surface. Enforcement sites use `file::symbol` (stable across line churn);
test names are the function under `#[test]`.

| Invariant | Why it matters | Enforced at | Pinned by |
|---|---|---|---|
| Coordinator cannot link input ↔ output | unlinkability — the primary goal | `coordinator/src/blind/rsa.rs::RsaBlindSigner` (RFC 9474 blind sig) + `client/src/tor.rs` (distinct circuit per phase) | integration `tests/integration/full_round.rs`, `mixed_script_e2e.rs`; structural (no coordinator state records the link) |
| Canonical tx is byte-identical across the display / per-sign-verify / broadcast build sites | a participant must sign the exact sighash that is broadcast | `coordinator/src/bitcoin/tx.rs::build_coinjoin_psbt` (ascending-outpoint ordering) | `tx.rs::tests::coinjoin_psbt_input_order_is_canonical` |
| Every input is non-replaceable (`Sequence::MAX`) | closes the post-broadcast RBF-eviction grief | `build_coinjoin_psbt` | `tx.rs::tests::coinjoin_psbt_inputs_are_non_replaceable` |
| Partial signatures are verified **before** being recorded | a bad sig can't poison the round and escape blame | `coordinator/src/bitcoin/sig_verify.rs::verify_input_signature`, called from `round/signing.rs::process_sign` | `signing.rs::tests::test_process_sign_rejects_invalid_signature`; `sig_verify.rs::tests::*` |
| ECDSA inputs are `SIGHASH_ALL` only | a signer can't disclaim the other outputs | `sig_verify.rs::parse_ecdsa_sig` | `sig_verify.rs::tests::p2wpkh_non_sighash_all_rejected` |
| Script type is chain-derived, never client-declared (CRIT-01) | blocks script-type spoofing of the verifier | `coordinator/src/bitcoin/utxo.rs::validate_utxo` → `shared/src/bip322/mod.rs::detect_script_type` | `shared/tests/bip322_cross_shape.rs` (9 cross-shape rejections); `tests/integration/multi_script_validate.rs` |
| Per-round RSA key is zeroized on `→ Idle` (AUDIT-03) | bounds Marvin-attack exposure; no key lingers in memory | `coordinator/src/round/state.rs::transition_to` (sole `inner = None` chokepoint) → `rsa::RsaPrivateKey` `ZeroizeOnDrop` chain | `state.rs::tests::round_secret_key_dropped_on_round_end`, `transition_to_idle_clears_inner`; `blind/rsa.rs::tests::round_secret_key_buffer_overwritten_on_drop` |
| All round-sensitive memory is zeroed on `→ Idle` (PRIV-01) | no plaintext round state survives broadcast | `coordinator/src/round/state.rs` `Drop for RoundStateInner` | `state.rs::tests::transition_to_idle_clears_inner` |
| Session token binds `(round, UTXO)`, constant-time compared | the sign submission can't be forged or re-correlated to the input | `coordinator/src/round/manager.rs::{generate,verify}_session_token` (HMAC-SHA-256 + `subtle` ct-eq) | `manager.rs::tests::session_token_verify_ok`, `session_token_verify_wrong_token` |
| Ban list stores only `SHA-256(outpoint)` (PRIV-02) | a leaked ban file discloses no coins | `coordinator/src/round/blame.rs::{hash_utxo_str, BanList}` | `blame.rs::tests::ban_file_roundtrip` |
| Registration gate is output-pessimistic (H1 / N-1) | a UTXO that passes registration must clear the heaviest build | `coordinator/src/bitcoin/fee.rs::estimate_fee_share` (prices outputs at `MAX_STANDARD_OUTPUT_VBYTES`) | `tx.rs::tests::registration_gate_covers_build_with_heaviest_outputs` |
| Per-script fee weights are fixed | a mixed-script round pays the correct on-chain fee | `coordinator/src/bitcoin/tx.rs::{script_input_vbytes, script_output_vbytes}` | `tx.rs::tests::script_input_vbytes_*`, `script_output_vbytes_*`, `fee_share_*` |
| Production sign path is byte-equal to BDK (CRIT-02) | no silent sighash / witness regression | `shared/src/bip322/{p2wpkh,p2tr,p2sh_p2wpkh}.rs::sign` | `client/tests/wallet_sign_roundtrip.rs` |
| Client fails fast on incompatible version / network / script at discovery | no Tor circuit wasted on a coordinator it cannot use | `client/src/discover.rs` (`WIRE_VERSION`, network, supported-input-types) | `client/src/discover.rs` unit tests |
| Dependency wire/ABI pins hold | pre-1.0 `bip322` drift, wrong Bitcoin Core RPC schema | `Cargo.toml` (`bip322 = "=0.0.11"`, `corepc-node` explicit feature) | CI gates `bip322 exact-version pin check`, `corepc-node feature pin check` |

The deepest treatment of the zeroization chain and the cross-shape rejection
matrix is in [docs/AUDIT-CHARTER.md](docs/AUDIT-CHARTER.md) (§ RSA Secret Key
Zeroization Window, § Cross-shape Rejection Properties).

## How to verify the invariants

Fast unit suite (covers every `*.rs::tests` row above — no `bitcoind` needed):

```bash
cargo test --workspace --lib
```

Cross-crate property/parity tests (the `shared/tests` and `client/tests` rows):

```bash
cargo test --workspace --test '*'
```

Full integration suite — the `tests/integration/` rows; slow, and it needs a
`bitcoind` binary (regtest). The canonical invocation and output interpretation
are in [CONTRIBUTING.md § Running integration tests](CONTRIBUTING.md#running-integration-tests):

```bash
BLINDJOIN_REQUIRE_BITCOIND=1 BITCOIND_EXE=$(brew --prefix)/bin/bitcoind \
  cargo test --test integration 2>&1 | tee target/integration-test.log
```

Run a single pinning test (useful when reviewing one invariant):

```bash
cargo test -p coordinator coinjoin_psbt_inputs_are_non_replaceable
```

Every merge to `main` is gated by six required CI checks (`.github/workflows/ci.yml`):
**cargo test**, **cargo clippy** (`--workspace --all-targets -D warnings`),
**cargo audit** (every ignore in `.cargo/audit.toml` carries a written rationale),
**coordinator binary builds**, **corepc-node feature pin check**, and **bip322
exact-version pin check**. Branch protection requires all six strictly up to date;
see [docs/branch-protection.md](docs/branch-protection.md).

## Conventions

- **No custom cryptography.** Only `blind-rsa-signatures`, `rust-bitcoin`, `bdk`,
  `secp256k1`, and `bip322` (see [CLAUDE.md § Constraints](CLAUDE.md)).
- **PII discipline.** No IP addresses, UTXO outpoints, addresses, per-participant
  amounts, input↔output pairs, keys, or signatures in logs. `tracing` uses
  structured fields; error messages name the failure *kind* only.
- **Round state is ephemeral.** All sensitive round material is dropped and zeroed
  on the transition to `Idle`, at the single `transition_to` chokepoint.
- **Additive wire evolution.** Wire types MUST deserialize without
  `#[serde(deny_unknown_fields)]`; unknown fields are ignored so the protocol can
  grow without a coordinated upgrade (see [docs/PROTOCOL.md § Forward
  compatibility](docs/PROTOCOL.md#forward-compatibility)).
- **Exact pins for fragile deps.** `bip322` and `corepc-node` are pinned and
  CI-gated; bumps are reviewed single-line PRs.
- **Anchors are `file::symbol`,** not `file:line` — symbols survive reformatting;
  line numbers rot.
- **Size-matched process.** Small fix → just do it; larger/fuzzy work → plan
  first. No process scaffolding for what fits in one commit.
