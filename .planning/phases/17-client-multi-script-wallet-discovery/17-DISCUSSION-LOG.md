# Phase 17: Client Multi-Script Wallet & Discovery - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-30
**Phase:** 17-Client Multi-Script Wallet & Discovery
**Mode:** --auto (autonomous: Claude selected the recommended option for every question without AskUserQuestion prompts; this log preserves the alternatives considered)
**Areas discussed:** A. CLI flag + descriptor templates; B. Wallet struct script_type storage; C. Sign-path dispatcher per script type; D. v1/v2 envelope selection; E. PSBT-input construction; F. Discovery resolver fail-fast; G. PKARR record version detection / WALLET-04 trigger; H. Plan ordering + test strategy; I. Output-address coordinator-`ost` mismatch handling

---

## A. `--type` CLI flag + descriptor templates (WALLET-01)

| Option | Description | Selected |
|--------|-------------|----------|
| `--type {p2wpkh\|p2tr\|p2sh-p2wpkh}` with `BLINDJOIN_SCRIPT_TYPE` env, lowercase kebab-case, default `p2wpkh` | Matches Phase 16 wire form + matches `coordinator.toml` `[bip] output_script_type` shape; default preserves v1.3 backwards compat | ✓ |
| `--script-type` long-form with no env var | More explicit but breaks symmetry with the coordinator-side naming | |
| Separate `--p2tr` / `--p2sh-p2wpkh` flags (mutually exclusive) | Cleaner for shell completion but doesn't compose with `--descriptor` parameter passing | |

**Auto selection:** Option 1 (Recommended) — matches existing client CLI conventions (`#[arg(long, env = "BLINDJOIN_*", default_value = "...")]`).
**Auto-mode log:**
```
[auto] A — Q: "How should the script-type CLI flag be shaped?" → Selected: "--type {wire-form}, BLINDJOIN_SCRIPT_TYPE env, lowercase kebab-case, default p2wpkh" (recommended default)
```

| Option | Description | Selected |
|--------|-------------|----------|
| BIP-84/86/49 per-type descriptors (`wpkh(.../84'/0'/...)`, `tr(.../86'/0'/...)`, `sh(wpkh(.../49'/0'/...))`) | Standard BIP paths; coin=0 across all networks preserves v1.3 byte-exact addresses | ✓ |
| BIP-44-correct coin-type per network (`84'/1'/...` for non-mainnet) | More spec-correct but silently breaks existing v1.3 wallet derivations | |

**Auto selection:** Option 1 — preserves v1.3 wallet compatibility per the cross-phase invariant. BIP-44-strict re-indexing deferred to v1.5 migration phase.
**Auto-mode log:**
```
[auto] A — Q: "Which BIP derivation paths for the per-type descriptors?" → Selected: "BIP-84/86/49 with coin=0 across all networks" (recommended; preserves v1.3 byte-exact addresses)
```

---

## B. Wallet struct script_type storage

| Option | Description | Selected |
|--------|-------------|----------|
| Store `script_type: ScriptType` on `BdkClientWallet`; set at construction from descriptor (or P2WPKH for from_wif) | Single source of truth; surfaces mismatch errors at construction time; cleanest for downstream callers | ✓ |
| Detect at use-time via `shared::bip322::detect_script_type(&utxo_script_pubkey)` | Avoids field; matches coordinator-side pattern but client already KNOWS its descriptor type | |
| Pass `script_type` as a parameter to every method that needs it | Stateless wallet API but every caller has to thread the type through | |

**Auto selection:** Option 1 — wallet OWNS its descriptor type explicitly; mismatch fails at the seam where the user-supplied info is fresh.
**Auto-mode log:**
```
[auto] B — Q: "Where does the wallet's ScriptType live?" → Selected: "Stored as field on BdkClientWallet, set at construction" (recommended)
```

---

## C. Sign-path dispatcher per script type (WALLET-02)

| Option | Description | Selected |
|--------|-------------|----------|
| `wallet.sign_bip322(message) -> Bip322SignedProof` returning {witness_stack, Witness, Option<ScriptBuf>, ScriptType}; per-type body: P2WPKH→`shared::bip322::sign_simple`, P2TR/P2SH-P2WPKH→bdk_wallet PSBT signer per Sprint-0-B verdict | Single dispatch point in the wallet; clean separation from the round/input envelope layer; production-ready paths only (no manual fallbacks per ADR #4) | ✓ |
| Per-type free functions in `client::bip322_sign` module (P2WPKH/P2TR/P2SH-P2WPKH sign separately) | More granular but spreads sign logic across multiple files | |
| Always-bdk path (deprecate `shared::bip322::sign_simple` consumption on the client) | Uniform PSBT-builder code; loses the existing P2WPKH carried-forward call site | |

**Auto selection:** Option 1 — wallet method dispatcher matches existing `sign_psbt_input` pattern; production-ready P2WPKH `sign_simple` is reused (no churn) while P2TR/P2SH-P2WPKH go through the validated Sprint-0-B path.
**Auto-mode log:**
```
[auto] C — Q: "Where does the per-script-type sign dispatcher live?" → Selected: "wallet.sign_bip322 method dispatching internally per script type" (recommended)
```

| Option | Description | Selected |
|--------|-------------|----------|
| WIF wallets → `shared::bip322::sign_simple` (raw key path); descriptor wallets → bdk PSBT path | Two paths: legacy WIF stays raw-key; multi-script descriptor wallets use bdk PSBT | ✓ |
| Always bdk PSBT path for ALL wallets including WIF | Uniform code; would change v1.3 carried-forward integration tests | |

**Auto selection:** Option 1 — preserves v1.3 WIF wallet bit-exact path (carries the `full_round.rs` invariant); descriptor wallets use the validated Sprint-0-B PSBT pattern for all 3 types.
**Auto-mode log:**
```
[auto] C — Q: "Sign path for WIF wallet (legacy) vs descriptor wallet?" → Selected: "WIF→sign_simple; descriptor→bdk PSBT" (recommended; preserves v1.3 invariant)
```

---

## D. v1/v2 envelope selection (WALLET-04 encoder)

| Option | Description | Selected |
|--------|-------------|----------|
| Branch inside `register_input` on `coordinator_info.capabilities.is_legacy`; v1.3 coord → emit v1 OwnershipProof; v1.4 coord → emit v2 OwnershipProof. Rely on shared::protocol::OwnershipProof::to_json_hex_str CD-7 branch for v1.3 byte-identity | Single decision point; reuses the Phase 15 CD-7 byte-identical v1 serialiser; no duplicated wire serialisation | ✓ |
| Always emit v2; rely on the coordinator's v1.3 → v1.4 forward compat | INCORRECT: v1.3 coordinator does not know v2 wire shape, would reject with serde error | |
| Maintain a separate v1 serialiser in client::round::input | Duplicates the CD-7 logic already in shared/ | |

**Auto selection:** Option 1 — only the v1.3 coord understands v1.3 shape; the CD-7 branch in `to_json_hex_str` makes the v1 path byte-identical to v1.3 without a separate serialiser.
**Auto-mode log:**
```
[auto] D — Q: "Where and how is the v1/v2 envelope selected?" → Selected: "Branch in register_input on capabilities.is_legacy" (recommended)
```

---

## E. PSBT-input construction for v2 envelope

| Option | Description | Selected |
|--------|-------------|----------|
| Wallet returns `(Witness, Option<ScriptBuf>)`; round/input.rs assembles `bitcoin::psbt::Input` and base64-encodes via `bitcoin::consensus::serialize` | Wallet stays pure (no PSBT envelope logic); round/input owns the wire-encoding | ✓ |
| Wallet returns a finalised `psbt::Input` directly | More PSBT logic in wallet; tightly couples wallet to the v2 wire encoding | |
| Wallet returns the base64 string already | Wallet knows wire format; conflates sign + serialise concerns | |

**Auto selection:** Option 1 — clean separation: wallet signs, round/input encodes.
**Auto-mode log:**
```
[auto] E — Q: "How does the wallet hand off the signed proof to the v2 envelope encoder?" → Selected: "Wallet returns (witness, Option<scriptSig>); round/input builds PSBT input" (recommended)
```

---

## F. Discovery resolver fail-fast (WALLET-03)

| Option | Description | Selected |
|--------|-------------|----------|
| Extend `discover_coordinator(pubkey, required_script_type)` signature; fail-fast inside resolver BEFORE returning; new typed `DiscoveryError` enum with `UnsupportedScriptType { pubkey, required, supported }` variant naming both coordinator + missing type | Pre-Tor by construction (PKARR resolution already runs before Tor init); fail-fast structurally enforced (cannot bypass) | ✓ |
| Resolver returns `CoordinatorInfo`; main.rs does the fail-fast check after | Bypass risk if a caller forgets the check; less structural enforcement | |
| Check at /round/info layer (after Tor circuit opens) | INCORRECT: ROADMAP success criterion #3 says "BEFORE opening a Tor circuit" | |

**Auto selection:** Option 1 — fail-fast happens at the resolver layer, structurally before Tor init. ROADMAP success criterion #3 satisfied by code-position discipline.
**Auto-mode log:**
```
[auto] F — Q: "Where does WALLET-03 fail-fast happen?" → Selected: "Inside discover_coordinator before returning" (recommended)
```

| Option | Description | Selected |
|--------|-------------|----------|
| ALSO fail-fast at discovery on `output_script_type` (ost) mismatch — separate `UnsupportedOutputScriptType` error variant | Catches wallet-output-type ≠ coordinator-`ost` mismatch before round work begins | ✓ |
| Discover only checks `sst` mismatch; `ost` mismatch fails later when output address is registered | Defers the error to mid-round (worse UX) | |
| Fold into the same `UnsupportedScriptType` variant with a `direction: Input | Output` enum | More variants but less actionable error message | |

**Auto selection:** Option 1 — split error variant for actionability (user fix is different for input vs output mismatch).
**Auto-mode log:**
```
[auto] F — Q: "How is the output_script_type (ost) mismatch handled?" → Selected: "Separate UnsupportedOutputScriptType discovery-time error variant" (recommended)
```

---

## G. PKARR record version detection / WALLET-04 trigger

| Option | Description | Selected |
|--------|-------------|----------|
| `is_legacy = record.version != "0.2.0" OR record.sst.is_none()`; either condition fires the compat shim. Default `version` to `"0.1.0"` if absent | Either of the two signals catches a v1.3 record; conservative + future-proof | ✓ |
| `is_legacy = record.version != "0.2.0"` (strict version check only) | Misses pre-version records (no version field at all) — would mis-classify ancient v1.0 records as v1.4 | |
| `is_legacy = record.sst.is_none()` (sst-presence only) | Future-proof for v1.5+ records that bump version but keep `sst` shape | |

**Auto selection:** Option 1 — conservative; either signal correctly triggers the v1.3 shim.
**Auto-mode log:**
```
[auto] G — Q: "What signal triggers the WALLET-04 compat shim?" → Selected: "record.version != 0.2.0 OR sst.is_none()" (recommended; conservative)
```

| Option | Description | Selected |
|--------|-------------|----------|
| Compat shim fires only for the supported intersection: `is_legacy && wallet.script_type == P2wpkh` emits v1 envelope; `is_legacy && wallet.script_type != P2wpkh` is rejected at discovery upstream | Reaches v1.3 only with P2WPKH wallets (only path v1.3 can handle); other types fail-fast cleanly | ✓ |
| Compat shim attempts non-P2WPKH against v1.3 coord too (best-effort) | INCORRECT: v1.3 coord cannot verify non-P2WPKH; would silently fail mid-round | |

**Auto selection:** Option 1 — the discovery layer rejects upstream so the round layer only sees the supported intersection.
**Auto-mode log:**
```
[auto] G — Q: "How does the compat shim handle non-P2WPKH wallets against v1.3 coordinators?" → Selected: "Discovery layer rejects; shim only fires for P2WPKH" (recommended)
```

---

## H. Plan ordering + test strategy

| Option | Description | Selected |
|--------|-------------|----------|
| 3 plans: 17-01 = WALLET-01 (descriptors); 17-02 = WALLET-02 + WALLET-04 encoder (signing + v1/v2 branch in round/input); 17-03 = WALLET-03 + WALLET-04 discovery (resolver fail-fast + PKARR version detection) | Sequential dependency chain matching Phase 15 + 16 shape; aligns with REPAIR-01 lesson #1 (wire-format ships first) | ✓ |
| 4 plans: split WALLET-04 across 17-03 (encoder) and 17-04 (discovery) | More atomic commits but encoder and discovery are coupled (encoder needs `is_legacy` from discovery) | |
| 1 monolithic plan covering all 4 requirements | Worst rollback granularity; doesn't match v1.4 pattern | |

**Auto selection:** Option 1 — matches Phase 15 + 16 cadence; clean sequential dependencies.
**Auto-mode log:**
```
[auto] H — Q: "How are Phase 17 plans split?" → Selected: "3 plans: WALLET-01, WALLET-02+WALLET-04-encoder, WALLET-03+WALLET-04-discovery" (recommended)
```

| Option | Description | Selected |
|--------|-------------|----------|
| New file `tests/integration/multi_script_client.rs` with 9 named tests (descriptor + sign-roundtrip + fail-fast + v1/v2 envelope assertions); inline unit tests in wallet.rs + discover.rs for fine-grained parser checks | Mirrors Phase 16's `multi_script_validate.rs` discipline; clean phase-boundary acceptance gate | ✓ |
| Extend existing `full_round.rs` with multi-script cases | Pollutes the v1.3-invariant gate; breaks the rollback safety net | |
| Per-plan integration test (3 separate files) | More overhead; less cross-cutting | |

**Auto selection:** Option 1 — symmetric with Phase 16; v1.3 invariant gate stays clean.
**Auto-mode log:**
```
[auto] H — Q: "Integration test layout?" → Selected: "New multi_script_client.rs file + inline unit tests" (recommended)
```

---

## I. CRIT-01 client-side discipline

| Option | Description | Selected |
|--------|-------------|----------|
| `// CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo` comment above the v2 envelope `script_type:` line + CI grep gate (≥ 1) on `client/src/round/input.rs` | Symmetric with Phase 16's coordinator-side CRIT-01 grep at `coordinator/src/bitcoin/utxo.rs` | ✓ |
| Inline comment only, no CI gate | Less enforcement | |
| No discipline marker (CRIT-01 is coordinator-side only) | INCORRECT: client populates the declared field; if client echoes the CLI flag instead of the wallet's actual type, CRIT-01's cross-check on the coordinator side becomes meaningless | |

**Auto selection:** Option 1 — symmetry with Phase 16 + CI gate is cheap.
**Auto-mode log:**
```
[auto] I — Q: "How is the CRIT-01 client-side invariant marked?" → Selected: "Inline comment + CI grep gate" (recommended)
```

---

## Claude's Discretion

Areas where the planner has flexibility (logged in CONTEXT.md §"Claude's Discretion" as CD-17..CD-24):

- **CD-17:** Case-insensitive variants for `--type` (default: lowercase kebab-case only)
- **CD-18:** Where `Bip322SignedProof` lives (default: `client::wallet`)
- **CD-19:** Visibility of `wallet.sign_bip322` (default: `pub(crate)`)
- **CD-20:** When to delete `generate_bip322_witness` (default: inside 17-02)
- **CD-21:** `CoordinatorCapabilities` exposure shape (default: public struct + WARN log on legacy)
- **CD-22:** `BLINDJOIN_SCRIPT_TYPE` namespace shape (default: single-underscore matching client convention)
- **CD-23:** Output-mismatch error variant fold-vs-split (default: split)
- **CD-24:** Uniform-bdk-path-for-descriptor-wallets vs P2WPKH-special-case (default: uniform; only WIF stays raw-key)

## Deferred Ideas

Ideas surfaced during analysis but belonging in other phases (preserved in CONTEXT.md §"Deferred"):

- Manual P2TR sign fallback (`shared/src/bip322/p2tr.rs::sign_p2tr_keypath` 80-LOC) — RETIRED for v1.4 per ADR #4; v1.5 reconsideration target if bdk regresses
- P2WSH multisig BIP-322 — REQUIREMENTS Future Requirements (v1.5+)
- TEST-EXT-01/02/03 cross-impl differential, on-chain anchor, automated backwards-compat matrix — v1.5+
- BIP-44-correct testnet/signet coin-type indexing — v1.5+ migration phase
- `--type` short form `-t` — Plan-phase discretion
- `bdk_wallet = "=2.3.x"` exact-pin tightening — v1.5+
- DECISIONS-INDEX.md rolling summary — v1.5 candidate (80+ D-* decisions across v1.4)
- CSV-vs-array PKARR record format reconsideration — v1.5+ (4+ script types breaches byte budget)
- WALLET-04 real-binary integration test against v1.3 client artifact — Phase 18 INTEG-01 (success criterion #5)
- Per-coordinator output-type pre-validation in `--generate-wallet` — v1.5 ergonomic polish
- `fund_regtest_typed` batched mixed-type funding — Phase 18 INTEG-01

---

*Discussion held: 2026-05-30 via /gsd:discuss-phase 17 --auto*
*This log is for human audit; downstream agents consume CONTEXT.md only.*
