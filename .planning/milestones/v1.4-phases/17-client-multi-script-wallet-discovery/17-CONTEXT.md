# Phase 17: Client Multi-Script Wallet & Discovery - Context

**Gathered:** 2026-05-30
**Status:** Ready for planning
**Mode:** --auto (autonomous decisions per recommended defaults; reviewable in this file)

<domain>
## Phase Boundary

Phase 17 wires v1.4 multi-script BIP-322 support into the **client** half of the protocol:

1. **`--type {p2wpkh|p2tr|p2sh-p2wpkh}` CLI flag** on `client::config::ClientConfig` (and `BLINDJOIN_SCRIPT_TYPE` env-var) selecting the BIP-84 / BIP-86 / BIP-49 descriptor template at `client/src/wallet.rs::BdkClientWallet::generate` and `BdkClientWallet::from_descriptor`. Defaults to `p2wpkh` for v1.3 backwards compatibility (WALLET-01).
2. **Wallet stores its `ScriptType`** so the round/input.rs construction knows what shape to emit. `BdkClientWallet { script_type: ScriptType, ... }` is the single source of truth — derived from the descriptor at wallet construction time, then carried through `sign_bip322(message) -> (Witness, Option<ScriptBuf>)` and `coinjoin_output_address()`.
3. **BIP-322 sign dispatcher in `client::wallet`** routes per script type via `shared::bip322::sign_simple` for P2WPKH (existing carried-forward path) and a thin bdk-backed signer for P2TR + P2SH-P2WPKH per ADR Decision #4 / Sprint-0-B verdict (`bdk_wallet 2.3` PSBT signer with `SignOptions { trust_witness_utxo: true }` over a BIP-322-shaped PSBT; extract from `final_script_witness[0]` with `tap_key_sig` fallback per Sprint-0-B finding). Result is passed to `round::input::register_input` which assembles the v2 `OwnershipProof { version: 2, witness_stack, psbt_input_b64: Some(...), script_type: Some(...) }` envelope (WALLET-02).
4. **Discovery resolver fail-fast** at `client::discover::discover_coordinator`. Extended parser reads `version`, `sst`, and `ost` from the PKARR `_blindjoin` TXT record alongside the existing `onion` field. Takes the caller's desired `script_type` as a new parameter; rejects with a typed error naming both the coordinator pubkey (z32) and the missing script type BEFORE `tor::init_tor` opens any circuit (WALLET-03 — pre-Tor fail-fast at the resolver layer; PKARR resolution already runs before Tor init at `main.rs:60`).
5. **v1.4→v1.3 compatibility shim** in two coupled spots (WALLET-04):
   - **Discovery side:** the same resolver detects pre-`0.2.0` PKARR (record `version: "0.1.0"` or missing `sst` field) and surfaces a `CoordinatorCapabilities { is_legacy: true, supported: vec![P2wpkh], output: P2wpkh }` flag carried through `CoordinatorInfo`.
   - **Round side:** `round::input::register_input` consults the capabilities flag: if `is_legacy && wallet.script_type == P2wpkh`, emits the legacy `version = 1` `OwnershipProof` (witness-only, byte-identical to v1.3 via the existing `OwnershipProof::to_json_hex_str` CD-7 branch at `shared/src/protocol.rs:239`); if `is_legacy && wallet.script_type != P2wpkh`, the discovery layer rejected upstream (the compat shim only fires for the supported intersection).

**Requirements mapped to this phase** (per `.planning/REQUIREMENTS.md §Traceability`): WALLET-01, WALLET-02, WALLET-03, WALLET-04.

**Not in scope:**
- `shared::bip322` API extension (Phase 15 closed; Phase 17 only CONSUMES `sign_simple`, `verify_simple`, `detect_script_type`, `ScriptType`).
- Coordinator allowlist / advertisement / `validate_utxo` dispatcher / PKARR producer (Phase 16 closed).
- Mixed-script E2E integration test (Phase 18 INTEG-01).
- Liquidity-bot multi-script keychain (Phase 18 INTEG-02).
- Per-script-type denominations / ban tracking / rate limits / per-round breakdown (REQUIREMENTS Out-of-Scope, REJECTED per D-08 / D-09).
- P2WSH multisig BIP-322 (REQUIREMENTS Future Requirements — v1.5+ candidate).
- Mixed output script types per round (D-07 LOCKED — single script type per round output).
- BIP-44-correct coin-type indexing for testnet/signet (intentional v1.3 carry-forward per D-66 below — coin=`0'` across all networks preserves byte-exact v1.3 wallet addresses).
- TEST-EXT-01/02/03 (cross-impl differential vectors, on-chain anchor test, automated backwards-compat matrix) — v1.5+ per REQUIREMENTS.
- Resolver-side `signed_packet.signed_at` freshness check (legacy v1.0 PKARR semantics; carry-forward).

**Boundary-only changes in this phase:**
- `client/src/config.rs` — add `--type / BLINDJOIN_SCRIPT_TYPE` flag.
- `client/src/wallet.rs::BdkClientWallet` — extend struct + `generate` + `from_descriptor` for multi-script; new `script_type()` accessor; new `sign_bip322(message)` method; existing `sign_psbt_input` unchanged (output-signing remains BDK-driven on the wallet's own keychain).
- `client/src/discover.rs::CoordinatorInfo` — extend with `capabilities: CoordinatorCapabilities`; `discover_coordinator` signature changes to take the caller's desired `ScriptType` and return the typed error on mismatch.
- `client/src/round/input.rs::register_input` — replace `generate_bip322_witness` with `wallet.sign_bip322(message)`; switch envelope construction to v1/v2 based on `coordinator_info.capabilities.is_legacy`.
- `client/src/main.rs` — pass `cfg.script_type` into `BdkClientWallet::generate / from_descriptor` and into `discover_coordinator`.
- `tests/integration/multi_script_client.rs` — NEW file (descriptor construction; sign roundtrip vs `shared::bip322::verify_simple` for all 3 types; v1.3 PKARR + P2TR wallet → rejects pre-Tor; v1.3 PKARR + P2WPKH wallet → emits v1 envelope; v1.4 PKARR + any type → emits v2 envelope).

**Cross-phase invariant (carries to every v1.4 phase boundary):** v1.3 P2WPKH-only `tests/integration/full_round.rs` MUST remain green at this phase boundary. The wallet's existing P2WPKH path is preserved (the new dispatcher routes P2WPKH through `shared::bip322::sign_simple(ScriptType::P2wpkh, ...)`, which Phase 15 confirmed bit-exact vs the carried-forward inline `generate_bip322_witness`; the round/input.rs envelope construction for a v1.4 client against a v1.4 coordinator emits v2, but the underlying witness bytes match v1.3 because the P2WPKH BIP-143 sighash math is unchanged). If `full_round` goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phase 14 ADR + Phase 15 + Phase 16 (NOT re-asked)

LOCKED upstream. Plan-phase consumes verbatim — no re-litigation.

- **ADR #1 (#decision-1):** `bip322 = "=0.0.10"` ADOPTED for verify; sign path is OURS (D-05 asymmetry). Phase 17 client calls `shared::bip322::sign_simple` for P2WPKH (and dispatches per ADR #4 for P2TR / P2SH-P2WPKH).
- **ADR #2 (#decision-2) / D-06:** MIXED rounds. Client's single-input registration is unaware of the round's per-script-type composition; it always declares its own input's `script_type` and lets the coordinator dispatcher do the rest.
- **D-07 / ADR #2 Consequences:** Output is single-script-type per round, operator-configured (`ost`). A client whose wallet output type ≠ coordinator's `ost` MUST be rejected at discovery (D-64 below — same fail-fast layer as WALLET-03 script-type mismatch).
- **D-10 / CRIT-01:** Client MUST declare `script_type` on the v2 OwnershipProof; coordinator cross-checks against derived. If client lies (declared ≠ derived) coordinator rejects with `ScriptTypeMismatch`. Phase 17 client populates `script_type` from `wallet.script_type()` (never from user input directly).
- **ADR #3 / D-12:** `OwnershipProof.version: u8` envelope. v=1 = v1.3 witness-only, v=2 = v1.4 PSBT-input + declared script_type. Coordinator branches `match proof.version`; unknown version → `UnsupportedProofVersion`.
- **ADR #4 / D-15 retired / sprint-0-B verdict:** `bdk_wallet 2.3` PSBT signer is the P2TR sign path. Manual `sign_p2tr_keypath` fallback (D-15's 80-LOC budget) is freed and shelved for v1.5 if bdk regresses.
- **Sprint-0-B finding (load-bearing for WALLET-02):** bdk_wallet 2.3 finalises single-key taproot keyspend into `psbt.inputs[0].final_script_witness[0]` (64-byte witness element), NOT `psbt.inputs[0].tap_key_sig` (cleared during finalisation). Phase 17 witness extraction MUST check both fields, prefer `final_script_witness` if populated. Parallels the existing P2WPKH extraction fallback at `client/src/wallet.rs:277-285`.
- **Phase 15 LOCKED API (the contract Phase 17 consumes):** `shared::bip322::{ScriptType, Bip322Error, detect_script_type, verify_simple(script_type, spk, witness, message, network), sign_simple(script_type, spk, key, message) -> Result<Witness, Bip322Error>}`. Note: per Phase 15 CD-6, `sign_simple` for P2TR + P2SH-P2WPKH is `todo!()` in shared/ production (test-only impls live behind `sign_simple_test_only`). **Phase 17 client signs via `bdk_wallet::Wallet::sign(...)` on a BIP-322-shaped PSBT — it does NOT call `shared::bip322::sign_simple` for P2TR or P2SH-P2WPKH. For P2WPKH the client calls `shared::bip322::sign_simple(ScriptType::P2wpkh, ...)` directly (production-ready in shared/).**
- **Phase 15 wire shape:** `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` flat struct with serde defaults; legacy v1 path emitted via the CD-7 branch in `to_json_hex_str` (preserves v1.3 byte form when `version == 1 && psbt_input_b64.is_none() && script_type.is_none()`).
- **Phase 16 PKARR record shape:** record `version: "0.2.0"`, fields `sst` (CSV alphabetical lowercase: `"p2sh-p2wpkh,p2tr,p2wpkh"`), `ost` (kebab-case scalar: `"p2wpkh"`). v1.3 records carry `version: "0.1.0"` and lack both `sst` and `ost`. Phase 17 resolver reads all three.
- **Phase 16 `/round/info` shape:** `InfoResponse.supported_script_types: Vec<ScriptType>` + `InfoResponse.output_script_type: ScriptType` with `#[serde(default = "default_legacy_supported")]` / `#[serde(default = "default_legacy_output")]` → `vec![P2wpkh]` / `P2wpkh`. v1.3 `/round/info` omits both fields; v1.4 client decoder reads the legacy defaults transparently.
- **Cross-phase invariant:** `cargo test --test full_round` MUST remain green at every plan boundary in Phase 17.

### A. `--type` CLI flag + descriptor templates (WALLET-01)

- **D-57:** **Flag name `--type`** with env-var `BLINDJOIN_SCRIPT_TYPE`. Short form `-t` optional (plan-phase discretion). Mirrors the coordinator's `BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE` env-var pattern at the CLI level. Default `p2wpkh` (Display: lowercase kebab-case form matching `ScriptType`'s serde rename).
  ```rust
  /// Script type for wallet descriptor generation. Selects BIP-84 (p2wpkh),
  /// BIP-86 (p2tr), or BIP-49 (p2sh-p2wpkh). Default p2wpkh for v1.3 backwards
  /// compatibility — existing wallets continue working unchanged.
  #[arg(long = "type", env = "BLINDJOIN_SCRIPT_TYPE", default_value = "p2wpkh", value_parser = parse_script_type)]
  pub script_type: ScriptType,
  ```
  Recommended: use clap's `value_parser` to deserialize via the `ScriptType` serde impl (no manual string match in `main.rs`).
- **D-58:** **Descriptor templates** match BIP standards (purpose / coin / account / change / index):
  - `p2wpkh` (BIP-84): `wpkh({xprv}/84'/0'/0'/0/*)` external + `wpkh({xprv}/84'/0'/0'/1/*)` internal — **unchanged from v1.3 carry-forward at `client/src/wallet.rs:140-141`**.
  - `p2tr` (BIP-86): `tr({xprv}/86'/0'/0'/0/*)` external + `tr({xprv}/86'/0'/0'/1/*)` internal. Single-key taproot keyspend (no script tree). bdk_wallet 2.3 native support per Sprint-0-B PoC at `client/examples/spike-p2tr.rs`.
  - `p2sh-p2wpkh` (BIP-49): `sh(wpkh({xprv}/49'/0'/0'/0/*))` external + `sh(wpkh({xprv}/49'/0'/0'/1/*))` internal. bdk_wallet 2.3 native support via standard `sh(wpkh(...))` descriptor.
- **D-59:** **Network parameter unchanged** — bdk_wallet's `Wallet::create(...).network(bdk_net)` already handles signet/testnet/mainnet correctly across all three descriptor types. No per-script-type network-handling fork.
- **D-60:** **`generate` command — per-type prominent fund-address output.** The existing `descriptors.txt` write + 0600 permissions + `WARNING: MASTER PRIVATE KEY MATERIAL` banner pattern carries unchanged. The "FUND THIS ADDRESS" panel correctly works for all 3 types because `peek_address(External, 0)` returns an address whose `.script_pubkey()` matches the descriptor type. New line in the banner: `Script type: p2tr (BIP-86)` (or whichever was selected) so the user understands what kind of address they're funding.
- **D-61:** **`from_wif` backwards-compat constructor stays single-script (P2WPKH only).** Plan-phase MUST NOT extend `from_wif` to accept a script_type parameter — it is a legacy code path used by integration tests for direct-key wallets; multi-script wallets always come through `from_descriptor` or `generate`. Add an `assert!(script_type == P2wpkh, ...)` if a caller wires through, or document the limitation in the doc comment.

### B. Wallet struct: where `ScriptType` lives

- **D-62:** **Store `script_type: ScriptType` as a field on `BdkClientWallet`.** Set at construction from the descriptor (or `p2wpkh` for `from_wif`). The `script_type()` accessor is the single source of truth for the round and discovery layers. Detecting at use-time via `shared::bip322::detect_script_type(&utxo_script_pubkey)` is a Phase 16 coordinator pattern — on the client, the wallet KNOWS its descriptor type explicitly, so storing it is cleaner and lets the CLI surface mismatch errors at construction time (e.g., if the user passes `--type p2tr` with a `wpkh(...)` descriptor, fail at wallet build, not at sign time).
- **D-63:** **Construction-time script-type cross-check** in `from_descriptor`. Compare the user-supplied `--type` against the descriptor's outer wrapper (`wpkh(...)` / `tr(...)` / `sh(wpkh(...))`). On mismatch: fail-fast with an error message that names both the declared type and the detected descriptor kind. Reason: the wallet caches `utxo_script_pubkey` derived from `--utxo-address`; a mismatch downstream silently emits the wrong-shape proof and the coordinator rejects with a less actionable `ScriptTypeMismatch`. Catch at the seam where the user-supplied info is fresh.

### C. Sign-path dispatch per script type (WALLET-02)

- **D-64:** **New method `wallet.sign_bip322(message: &str) -> Result<Bip322SignedProof>`** where `Bip322SignedProof` is a small local struct (Phase 17 client-internal, NOT a wire type):
  ```rust
  pub struct Bip322SignedProof {
      pub witness_stack: Vec<Vec<u8>>,      // for v1 envelope serialisation
      pub witness: bitcoin::Witness,        // for v2 PSBT-input construction
      pub final_script_sig: Option<ScriptBuf>, // P2SH-P2WPKH only; None for P2WPKH/P2TR
      pub script_type: ScriptType,
  }
  ```
  Wallet owns the "I know how to sign for my own UTXO" responsibility; round/input.rs owns the "I know what wire shape the coordinator expects" responsibility. Clean separation.
- **D-65 (per-script dispatch body):**
  - **P2WPKH path:** Calls `shared::bip322::sign_simple(ScriptType::P2wpkh, &spk, &secret_key, message.as_bytes())?` — Phase 15 production-ready. Returns `Witness` directly; convert to `Vec<Vec<u8>>` via `.iter().map(|s| s.to_vec()).collect()` for `witness_stack` field. `final_script_sig: None`.
    - **WIF wallet (legacy `from_wif` path):** `secret_key` from `wallet.secret_key_for_signing()` (existing accessor at `client/src/wallet.rs:220`).
    - **Descriptor wallet (`from_descriptor` / `generate`):** **route through bdk_wallet sign** — descriptor wallets don't expose a raw `SecretKey` (BIP-32 derives per-address keys). Use the bdk path uniformly. See P2TR pattern below; P2WPKH descriptor wallets follow the same shape with `Network`-derived address from a BIP-322 `to_spend` SPK.
  - **P2TR path:** Per Sprint-0-B verbatim:
    1. `let msg_hash = shared::bip322::bip322_message_hash(message.as_bytes())`.
    2. `let to_spend = shared::bip322::build_bip322_to_spend(&wallet.utxo_script_pubkey, &msg_hash)`.
    3. `let to_sign_tx = shared::bip322::build_bip322_to_sign(&to_spend)`.
    4. Build PSBT: `let mut psbt = bitcoin::Psbt::from_unsigned_tx(to_sign_tx)?` then populate `psbt.inputs[0].witness_utxo = Some(TxOut { value: Amount::ZERO, script_pubkey: wallet.utxo_script_pubkey.clone() })`.
    5. `self.inner.sign(&mut psbt, SignOptions { trust_witness_utxo: true, ..Default::default() })?` (same `SignOptions` as the existing `sign_psbt_input` at `client/src/wallet.rs:269` — Phase 12 lesson #1 carries).
    6. Witness extraction (Sprint-0-B finding): `psbt.inputs[0].final_script_witness.clone().or_else(|| psbt.inputs[0].tap_key_sig.map(|sig| { let mut w = Witness::new(); w.push(sig.serialize()); w }))`. Prefer `final_script_witness`; fall back to `tap_key_sig`. If both `None`, return `anyhow!("bdk_wallet did not produce a P2TR witness")`.
    7. `final_script_sig: None`. `script_type: ScriptType::P2tr`.
  - **P2SH-P2WPKH path:** Mirror the P2TR PSBT path, with one extra extraction:
    1. Same steps 1-5 as P2TR (build BIP-322 to_spend/to_sign, populate PSBT, bdk sign).
    2. Extract witness from `psbt.inputs[0].final_script_witness`.
    3. **Extract `final_script_sig`** from `psbt.inputs[0].final_script_sig.clone()` — bdk_wallet finalises sh(wpkh()) by populating BOTH fields. If `final_script_sig.is_none()`, return `anyhow!("bdk_wallet did not produce a P2SH-P2WPKH final_script_sig")`.
    4. `script_type: ScriptType::P2shP2wpkh`. `final_script_sig: Some(...)`.
- **D-66:** **Network parameter** sourced from `wallet.network` (already cached at `client/src/wallet.rs:19`). No new field. The bdk signer is network-agnostic at the keypath layer; `Network` only matters if we ever round-trip through `bitcoin::Address::from_script` (which we don't on the client sign path).
- **D-67 (no manual fallback in Phase 17):** D-15's manual P2TR sign budget is RETIRED for v1.4 per ADR #4. Phase 17 plan-phase MUST NOT add a feature flag selecting bdk-vs-manual; if bdk_wallet 2.3 regresses on taproot finalisation, escalate per REPAIR-01 lesson #4 — `/gsd:debug` to confirm regression, then re-open ADR #4 in a v1.5 phase.

### D. v1/v2 envelope selection (WALLET-04 — the compat shim trigger)

- **D-68:** **Envelope decision lives in `round::input::register_input`.** Inputs to the decision:
  - `coordinator_info.capabilities.is_legacy` (boolean from discovery layer per D-71 below)
  - `wallet.script_type()`
  - The signed proof from `wallet.sign_bip322(message)`.
  Logic (single `if` branch, no helper abstraction):
  ```rust
  let proof = if coordinator_info.capabilities.is_legacy {
      // WALLET-04 compat shim — v1.3 coordinator path
      assert_eq!(wallet.script_type(), ScriptType::P2wpkh, "unreachable: discovery layer rejected non-P2wpkh against legacy coordinator");
      shared::protocol::OwnershipProof {
          version: 1,
          witness_stack: signed.witness_stack,
          psbt_input_b64: None,
          script_type: None,
      }
  } else {
      // v1.4 coordinator path — always v2, regardless of wallet script type
      let psbt_input = build_v2_psbt_input(&signed)?;  // see D-69
      let psbt_input_b64 = B64.encode(bitcoin::consensus::serialize(&psbt_input));
      shared::protocol::OwnershipProof {
          version: 2,
          witness_stack: signed.witness_stack,  // populated for symmetry; coordinator's v2 path ignores it
          psbt_input_b64: Some(psbt_input_b64),
          script_type: Some(signed.script_type),
      }
  };
  let ownership_proof = proof.to_json_hex_str();
  ```
  The CD-7 branch in `OwnershipProof::to_json_hex_str` at `shared/src/protocol.rs:239` makes the v1 path emit byte-identical v1.3 array-of-hex form when `version == 1 && psbt_input_b64.is_none() && script_type.is_none()` — no separate v1.3 serialiser needed.
- **D-69:** **`build_v2_psbt_input(signed) -> Result<bitcoin::psbt::Input>`** is a private helper in `client::round::input`:
  ```rust
  fn build_v2_psbt_input(signed: &Bip322SignedProof) -> Result<bitcoin::psbt::Input> {
      let mut input = bitcoin::psbt::Input::default();
      input.final_script_witness = Some(signed.witness.clone());
      if let Some(ref sig) = signed.final_script_sig {
          input.final_script_sig = Some(sig.clone());
      }
      Ok(input)
  }
  ```
  The PSBT envelope carries the script-type information implicitly via `final_script_sig` presence + the explicit `script_type` field on the OwnershipProof envelope (D-12 carries declared type alongside, so the coordinator doesn't have to parse the PSBT to know what to dispatch to — it cross-checks the declared `script_type` against `detect_script_type(on_chain_spk)` per CRIT-01).
- **D-70:** **`witness_stack` populated in BOTH v1 and v2 envelopes.** v2 coordinator's `validate_utxo` reads from `psbt_input_b64`, but the v2 wire keeps `witness_stack` non-empty for symmetry with v1 and for future v2 consumers that prefer the flat stack. Per Phase 15 D-22, both fields are present in the v2 envelope struct; the coordinator decides which to honour based on version. No data loss either way.

### E. Discovery resolver fail-fast (WALLET-03)

- **D-71:** **Extend `CoordinatorInfo` with capabilities:**
  ```rust
  pub struct CoordinatorInfo {
      pub coordinator_url: String,
      pub capabilities: CoordinatorCapabilities,
  }
  pub struct CoordinatorCapabilities {
      pub record_version: String,            // "0.1.0" or "0.2.0"
      pub is_legacy: bool,                   // record_version != "0.2.0"
      pub supported_script_types: Vec<ScriptType>,  // legacy → vec![P2wpkh]
      pub output_script_type: ScriptType,    // legacy → P2wpkh
  }
  ```
- **D-72:** **`discover_coordinator` signature changes** to take the caller's desired wallet `ScriptType` and return a `CoordinatorInfo`. The fail-fast happens INSIDE the resolver, BEFORE returning, so callers cannot accidentally bypass it:
  ```rust
  pub async fn discover_coordinator(
      pkarr_pubkey: &str,
      required_script_type: ScriptType,
  ) -> Result<CoordinatorInfo, DiscoveryError>;
  ```
  Failure types (new typed error enum at `client::discover::DiscoveryError`):
  - `InvalidPubkey(String)` — existing case carried forward.
  - `NotFound { pubkey: String }` — existing "not in DHT" case.
  - `MissingOnion { pubkey: String }` — existing case.
  - **NEW `UnsupportedScriptType { pubkey: String, required: ScriptType, supported: Vec<ScriptType> }`** — fired when `required_script_type` is not in `supported`. Error message: `"coordinator {pubkey} does not support {required:?} ownership proofs (supports: {supported:?})"` — matches ROADMAP success criterion #3 wording exactly.
- **D-73:** **PKARR record JSON parsing.** Extend `parse_onion_from_rr` to a richer `parse_blindjoin_record(rr) -> Option<BlindjoinRecord>`:
  ```rust
  #[derive(Deserialize)]
  struct BlindjoinRecord {
      #[serde(default = "default_legacy_version")]
      version: String,
      onion: String,
      #[serde(default)]
      sst: Option<String>,    // CSV: "p2sh-p2wpkh,p2tr,p2wpkh" (v0.2.0) | None (v0.1.0)
      #[serde(default)]
      ost: Option<String>,    // "p2wpkh" (v0.2.0) | None (v0.1.0)
  }
  fn default_legacy_version() -> String { "0.1.0".into() }
  ```
  `is_legacy = record.version != "0.2.0" || record.sst.is_none()` — either condition fires the compat shim. The `sst` CSV is parsed via `s.split(',').map(ScriptType::from_str).collect::<Result<Vec<_>, _>>()`; an invalid script-type token in the CSV is a coordinator-side bug → return `Err(DiscoveryError::MalformedRecord)` to be safe.
- **D-74:** **Pre-Tor placement verified by code location.** `client::main` calls `discover::discover_coordinator(...)` at `client/src/main.rs:60` BEFORE `tor::init_tor(...)` at `client/src/main.rs:68`. The fail-fast in `discover_coordinator` returns before `init_tor` runs — no circuit opens for a rejected coordinator. The ROADMAP success criterion #3 wording ("BEFORE opening a Tor circuit") is satisfied structurally, not via runtime ordering hacks.
- **D-75:** **No double-check at `/round/info`.** The PKARR `sst` is the load-bearing fail-fast signal because it's the only one that runs pre-Tor. The `/round/info` `supported_script_types` field is informational only after discovery passed (it's served via Tor and is therefore already in-circuit). Phase 17 does NOT add a redundant check on `/round/info` script types — the PKARR check is canonical. If a coordinator's PKARR claims P2TR but `/round/info` doesn't, that's an operator-side config bug; the coordinator's startup `BipConfig::validate()` already prevents this at the source (Phase 16 D-37 — `output_script_type` must be in allowed set; `supported()` set is wired from the same source as the PKARR producer).
- **D-76:** **`output_script_type` mismatch ALSO fails at discovery (WALLET-03 sibling check).** A client wallet whose output type (derived from its descriptor) differs from the coordinator's advertised `ost` would assemble a CoinJoin output the coordinator refuses. Same fail-fast layer:
  ```rust
  if capabilities.output_script_type != wallet_output_script_type {
      return Err(DiscoveryError::UnsupportedOutputScriptType { ... });
  }
  ```
  Plan-phase decides whether this is folded into the same `UnsupportedScriptType` error variant or split into `UnsupportedOutputScriptType` (recommended: split for actionability — the user-facing fix is different for input vs output mismatch).

### F. Plan ordering + test strategy

- **D-77 (plan ordering — 3 plans):**
  - **17-01-PLAN.md** = WALLET-01 — `--type` CLI flag + per-type descriptor templates in `BdkClientWallet::{generate, from_descriptor}` + `script_type` field on the struct + construction-time mismatch check + `generate` command per-type banner. Tests: unit tests for descriptor template generation per type, `from_descriptor` mismatch fail-fast, `generate --type {each}` smoke test producing the right address shape. Maps requirement **WALLET-01**. Atomic commit; sequential dependency chain into 17-02.
  - **17-02-PLAN.md** = WALLET-02 + WALLET-04 (envelope encoder side) — `wallet.sign_bip322(message)` per-script dispatcher (P2WPKH via shared::bip322::sign_simple; P2TR + P2SH-P2WPKH via bdk PSBT path per Sprint-0-B) + `Bip322SignedProof` struct + `build_v2_psbt_input` helper in round/input.rs + v1/v2 envelope branch in `register_input`. Tests: sign roundtrip vs `shared::bip322::verify_simple` for each of the 3 script types (new file `client/tests/wallet_sign_roundtrip.rs` — unit-test style, no bitcoind needed); v1 envelope byte-identity test (snapshot a v1.3 ownership_proof string and assert v1 path emits the same bytes). Maps requirements **WALLET-02 + WALLET-04 (encoder)**.
  - **17-03-PLAN.md** = WALLET-03 + WALLET-04 (discovery side) — `CoordinatorCapabilities` struct + extended `BlindjoinRecord` parser + new `DiscoveryError` enum + `discover_coordinator` signature change to take `required_script_type` and fail-fast on mismatch + v1.3-vs-v1.4 record-version detection that sets `is_legacy` + `main.rs` wiring. Tests: PKARR record parser tests (v0.1.0 record without `sst` → `is_legacy = true`, defaults `supported = [P2wpkh]`; v0.2.0 record with `sst="p2tr,p2wpkh"` → parses correctly; v0.2.0 record with malformed `sst` → `MalformedRecord`; v0.2.0 record with `sst` missing required type → `UnsupportedScriptType` error names both coordinator pubkey and missing type). Maps requirements **WALLET-03 + WALLET-04 (discovery)**.
  - Sequential dependency chain `17-01 → 17-02 → 17-03` (wave 1 → wave 2 → wave 3). 17-02 depends on 17-01's `script_type` accessor; 17-03 depends on 17-02's envelope encoder so the WALLET-04 compat shim is testable end-to-end at the 17-03 boundary.
- **D-78 (integration test strategy):** **New file `tests/integration/multi_script_client.rs`** owned by Phase 17. Reuses `BitcoindGuard` + `require_bitcoind!()` macro from v1.3 (Phase 9). Test cases (per 17-03 acceptance):
  - **17-01 boundary:** `generate_p2wpkh_wallet_emits_bip84_descriptor`, `generate_p2tr_wallet_emits_bip86_descriptor`, `generate_p2sh_p2wpkh_wallet_emits_bip49_descriptor` — assert the printed descriptor matches the expected `wpkh(.../84'/...)` / `tr(.../86'/...)` / `sh(wpkh(.../49'/...))` shape via regex.
  - **17-02 boundary:** `p2wpkh_sign_roundtrip_verifies`, `p2tr_sign_roundtrip_verifies`, `p2sh_p2wpkh_sign_roundtrip_verifies` — for each: build a wallet of that type, derive its `utxo_script_pubkey` from the wallet's first external address, call `wallet.sign_bip322("test-message")`, feed `(script_type, spk, witness, "test-message", Network::Signet)` to `shared::bip322::verify_simple`, assert `Ok(())`. P2SH-P2WPKH variant also asserts `signed.final_script_sig.is_some()`.
  - **17-03 boundary (WALLET-03):** `v13_pkarr_record_with_p2tr_wallet_rejects_before_tor` — mock a `BlindjoinRecord { version: "0.1.0", onion: "...", sst: None, ost: None }`, call `discover_coordinator(pubkey, ScriptType::P2tr)`, assert `Err(DiscoveryError::UnsupportedScriptType { required: P2tr, supported: vec![P2wpkh], ... })`. Verify `tor::init_tor` was NOT called (instrument with a test-only counter or rely on the fact that the resolver returns before main.rs gets to the tor branch).
  - **17-03 boundary (WALLET-04 — compat shim):** `v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope` — mock v0.1.0 record + P2WPKH wallet; call `register_input` against a stubbed coordinator HTTP server; assert the `ownership_proof` field of the POSTed `InputRegRequest` is in v1.3 array-of-hex form (NOT JSON object).
  - **17-03 boundary (v1.4 path):** `v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope` — mock v0.2.0 record with `sst="p2sh-p2wpkh,p2tr,p2wpkh"` + P2TR wallet; assert posted `ownership_proof` is v2 JSON object with `version: 2`, `script_type: "p2tr"`, `psbt_input_b64: Some(...)`.
  - `tests/integration/full_round.rs` remains UNCHANGED at this phase boundary (v1.3 invariant gate, P2WPKH path bit-exact).
- **D-79:** **Liquidity-bot integration test deferred to Phase 18.** Phase 17 verifies the WALLET-04 compat shim against a STUBBED v1.3 coordinator (HTTP wire shape mock); Phase 18 verifies against a REAL v1.3 binary as the WALLET-04 acceptance gate (ROADMAP Phase 18 success criterion #5). Phase 17's stub-based test is the structural gate; Phase 18's binary-based test is the integration gate. No duplication, clean handoff.
- **D-80 (CRIT-01 client-side discipline):** A `// CRIT-01: script_type populated from wallet (descriptor type), never from CLI-flag direct echo` comment lives directly above the `script_type: Some(signed.script_type),` line in `register_input`'s v2 envelope construction. The CLI `--type` flag flows into wallet construction; the wallet's stored `script_type` is the wire source. Plan-phase decides whether a `grep -c "CRIT-01" client/src/round/input.rs` CI assertion is added (recommended: yes, ≥ 1, for symmetry with Phase 16's coordinator-side CRIT-01 grep at `coordinator/src/bitcoin/utxo.rs`).

### Claude's Discretion

- **CD-17:** Whether `--type` accepts case-insensitive variants (`P2TR`, `P2tr`, `p2tr`). Default: **lowercase kebab-case only** (matches Phase 16's `#[serde(rename_all = "snake_case")]` + `rename = "p2sh-p2wpkh"` shape; matches `BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE` parser). Plan-phase may add a custom `value_parser` that lowercases-and-normalises if user-facing ergonomics warrant.
- **CD-18:** Whether `Bip322SignedProof` lives in `client::wallet` (recommended — wallet is the producer) or `client::round::input` (consumer). Default: **`client::wallet`**. Avoids the round module owning a sign-output type.
- **CD-19:** Whether to expose `wallet.sign_bip322` as a public method or `pub(crate)`. Default: **`pub(crate)`** — round/input is the only consumer; the library surface stays minimal. Plan-phase may go `pub` if a downstream library user (e.g., a third-party CLI building on `client` as a lib) needs it.
- **CD-20:** Whether to delete the old `generate_bip322_witness` helper at `client/src/round/input.rs:115` in 17-02 (recommended) or carry it as a private fallback. Default: **delete inside 17-02 atomic commit** — it becomes unreachable once `wallet.sign_bip322` is the only call site. Matches Phase 16 CD-15 (delete `verify_bip322_simple` inside the dispatcher swap commit).
- **CD-21:** Whether to expose `CoordinatorCapabilities` publicly on `CoordinatorInfo` or hide it behind accessor methods. Default: **public struct** — main.rs reads `info.capabilities.is_legacy` directly to log a `WARN: legacy v1.3 coordinator detected, using v1 OwnershipProof shim` line for operator visibility. Encapsulation buys nothing here.
- **CD-22:** Whether the `BLINDJOIN_SCRIPT_TYPE` env var follows the coordinator's double-underscore namespacing convention (`BLINDJOIN__CLIENT__SCRIPT_TYPE`) or the existing client single-underscore one (`BLINDJOIN_SCRIPT_TYPE` matching `BLINDJOIN_UTXO`, `BLINDJOIN_COORDINATOR_URL`). Default: **single-underscore `BLINDJOIN_SCRIPT_TYPE`** (matches the client's existing convention at `client/src/config.rs`; the double-underscore is a `config`-crate-driven coordinator-side pattern that the client does not currently use).
- **CD-23:** Whether to fold the `output_script_type` mismatch into the same `UnsupportedScriptType` error variant or split into `UnsupportedOutputScriptType` (D-76). Default: **split** — different user-facing fix (the user runs `--type p2tr` but coordinator's `ost` is `p2wpkh` → user picks a different coordinator OR generates a P2WPKH wallet for this round; the error needs to say which side mismatched). Plan-phase may collapse if the message is structurally clear from context.
- **CD-24:** Whether 17-02 ships a custom `Bip322SignedProof` for P2TR/P2SH-P2WPKH descriptor wallets vs reusing the existing P2WPKH descriptor wallet sign path. Default: **uniform path** — descriptor wallets ALL go through bdk_wallet's PSBT signer with the appropriate BIP-322-shaped PSBT input; P2WPKH descriptor wallets use the same bdk path as P2TR/P2SH-P2WPKH. The WIF wallet stays on the `secret_key_for_signing` + `shared::bip322::sign_simple` path (legacy). This makes 17-02 a single PSBT-construction code path with per-script witness extraction.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing.**

### Phase 14 ADR + Phase 15 + Phase 16 outputs (LOCKED inputs)

- `.planning/decisions/v1.4-adr.md` §`#decision-1` — bip322 crate ADOPT; sign path is OURS per D-05. Binds Phase 17's per-script sign-path dispatcher to bdk + shared::bip322 split (D-65 / D-67).
- `.planning/decisions/v1.4-adr.md` §`#decision-3` — `OwnershipProof.version: u8` envelope + v1/v2 branch semantics. Binds Phase 17 envelope encoder logic (D-68 / D-69 / D-70).
- `.planning/decisions/v1.4-adr.md` §`#decision-4` — bdk_wallet 2.3 P2TR sign path ACCEPTED. Sprint-0-B verdict (`final_script_witness[0]`, NOT `tap_key_sig`) is load-bearing for D-65 P2TR extraction.
- `.planning/research/sprint-0-B.md` — full P2TR PoC. Phase 17 17-02 plan replicates this PoC's PSBT construction + signing + witness extraction verbatim. See especially the verdict line at `sprint-0-B.md:315` and the extraction-path note at `sprint-0-B.md:317-319`.
- `.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` §D-06..D-10 — MIXED rounds, output-type-per-coordinator, CRIT-01 invariant. Phase 17 client populates `script_type` from wallet (D-80), not from CLI direct echo.
- `.planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-22..D-32 — wire shape locked (`OwnershipProof` flat struct + `version` envelope + `script_type` sibling field). Phase 17 calls `OwnershipProof::to_json_hex_str` which has the CD-7 branch for v1.3 byte-identity.
- `.planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §CD-6 — `shared::bip322::sign_simple` for P2TR + P2SH-P2WPKH is `todo!()` in production. Phase 17 client signs via bdk PATH for those types; uses `shared::bip322::sign_simple` ONLY for P2WPKH (which has a full body).
- `.planning/phases/15-shared-crate-multi-script-contract/15-VERIFICATION.md` — confirms `shared::bip322::*` API surface stable + CRIT-01 mitigated at the API level (per-script files are `pub(crate)`).
- `.planning/phases/15-shared-crate-multi-script-contract/15-RESEARCH.md` §"Open Questions (RESOLVED)" Q3 — `#[serde(rename_all = "snake_case")]` + `rename = "p2sh-p2wpkh"` on the `P2shP2wpkh` variant. Phase 17's `--type` CLI parser uses these exact wire-form strings.
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §D-39..D-44 — PKARR record schema (compact `"sst"` + `"ost"` field names; `version: "0.2.0"`; CSV alphabetical lowercase). Phase 17 resolver decodes against this schema.
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §D-56 — explicitly defers PKARR resolver-side `#[serde(default)]` shim on the resolved-record type to Phase 17. This is THIS PHASE'S work (D-73).
- `.planning/phases/16-coordinator-integration-advertisement/16-CONTEXT.md` §D-42 — `InfoResponse.supported_script_types` + `output_script_type` field shapes + `#[serde(default = "default_legacy_*")]` for v1.3 compat. Phase 17 client decoder reads these transparently.

### Project-level anchors

- `.planning/PROJECT.md` §"Current Milestone: v1.4 BIP-322 Multi-Script Support" — milestone goal, target features, out-of-v1.4-scope list. Phase 17 is the client-side complement to Phase 16's coordinator-side.
- `.planning/PROJECT.md` §"Constraints" — no custom crypto, MIT, Tor-native, signet-first, NO PII logging. Binds the discovery-time error message shape (D-72): name coordinator pubkey + missing script type ONLY; no IP address, no wallet identifier, no UTXO outpoint.
- `.planning/REQUIREMENTS.md` §"v1.4 Requirements" — WALLET-01, WALLET-02, WALLET-03, WALLET-04 mapped to Phase 17 §"Traceability". Out-of-Scope table is binding (no per-script-type ban tracking, no per-script-type rate limits — neither applies to client-side; the client only declares its own type).
- `.planning/ROADMAP.md` §"Phase 17" — 5 success criteria (per-type descriptor generation; sign roundtrip for all 3 types via v1.4 coordinator; fail-fast at discovery for P2TR-wallet-vs-v1.3-coordinator; WALLET-04 compat for P2WPKH-wallet-vs-v1.3-coordinator; v1.3 invariant green).
- `.planning/STATE.md` §"Accumulated Context" + §"Carry-forward constraints" — v1.3 REPAIR-01 lessons #1 (wire-format ships first — binds 17-02's envelope-encoder-before-discovery-shim ordering inside the plan body) and #4 (pivot to /gsd:debug on carry-forward shape — binds D-67's no-fallback escalation policy).
- `.planning/MILESTONES.md` (if present, otherwise skip) — v1.4 cut PR scope; Phase 17 is the last code-deliverable phase before the milestone's E2E acceptance gate (Phase 18).

### Phase 15 + Phase 16 API surface (what Phase 17 consumes)

- `shared/src/bip322/mod.rs` — `ScriptType` enum, `Bip322Error` (10 variants), `detect_script_type`, `verify_simple(script_type, spk, witness, message, network)`, `sign_simple(script_type, spk, key, message)`. Phase 17 imports `ScriptType` for the CLI flag (D-57) and method signatures (D-62 / D-64) and calls `sign_simple` only for P2WPKH (D-65).
- `shared/src/protocol.rs::OwnershipProof` — `{ version, witness_stack, psbt_input_b64, script_type }` flat struct. Phase 17 constructs v1 and v2 instances per D-68; relies on the CD-7 branch in `to_json_hex_str` for v1 byte-identity at `shared/src/protocol.rs:239`.
- `shared/src/protocol.rs::InfoResponse` (post-16) — `supported_script_types: Vec<ScriptType>` + `output_script_type: ScriptType` with `#[serde(default)]`. Phase 17 client reads these via `client::http::CoordinatorClient::get_info()` post-discovery (D-75).
- `coordinator/src/discovery/pkarr_pub.rs::build_coordinator_packet` (Phase 16) — defines the `sst` + `ost` JSON shape. Phase 17 resolver decodes against this shape (D-73 `BlindjoinRecord` parser).

### Code anchors (Phase 17 modifies)

- `client/src/config.rs::ClientConfig` (full file) — add `--type / BLINDJOIN_SCRIPT_TYPE` flag per D-57. Mirror the existing field pattern (`#[arg(long, env = "...", default_value = "...")]`).
- `client/src/wallet.rs::BdkClientWallet` (lines 17-26) — add `script_type: ScriptType` field per D-62.
- `client/src/wallet.rs::BdkClientWallet::generate` (lines 117-214) — extend with per-type descriptor template (D-58), update the FUND-THIS-ADDRESS banner with the script-type line (D-60).
- `client/src/wallet.rs::BdkClientWallet::from_descriptor` (lines 75-110) — add the construction-time mismatch check (D-63).
- `client/src/wallet.rs::BdkClientWallet::from_wif` (lines 33-68) — document the P2WPKH-only constraint (D-61); add assert.
- `client/src/wallet.rs` — NEW `sign_bip322(message: &str) -> Result<Bip322SignedProof>` method per D-64 / D-65.
- `client/src/wallet.rs` — NEW `pub fn script_type(&self) -> ScriptType` accessor.
- `client/src/round/input.rs::register_input` (lines 18-108) — replace `generate_bip322_witness` call site (line 63) with `wallet.sign_bip322(...)`; add the v1/v2 envelope branch per D-68; add the `build_v2_psbt_input` helper per D-69.
- `client/src/round/input.rs::generate_bip322_witness` (lines 115-end) — DELETE in 17-02 per CD-20.
- `client/src/discover.rs` (full file) — extend with `CoordinatorCapabilities` struct, `DiscoveryError` enum, signature change to `discover_coordinator(pubkey, required_script_type)`, richer `BlindjoinRecord` parser per D-71 / D-72 / D-73.
- `client/src/main.rs` — pass `cfg.script_type` into `BdkClientWallet::{generate, from_descriptor}` and into `discover_coordinator`; map `DiscoveryError` to actionable error messages; log `WARN` on legacy-coordinator detection per CD-21.
- `tests/integration/full_round.rs` — DO NOT MODIFY. v1.3 invariant gate (P2WPKH path; tests an existing WIF wallet + v1 envelope path).
- `tests/integration/multi_script_client.rs` — NEW file per D-78.
- `client/tests/wallet_sign_roundtrip.rs` — NEW unit-test-style file for per-script sign↔verify roundtrips without bitcoind (D-77 / 17-02 test scope).

### Cross-phase invariant references

- `tests/integration/full_round.rs` lines 1-30 — header comment + `require_bitcoind!()` usage; pattern for the new `multi_script_client.rs` test.
- `tests/integration/mod.rs` — `BitcoindGuard` + `fund_regtest` helpers carry forward unchanged. Phase 17 may extend with `fund_regtest_typed(script_type)` if needed (Phase 16 16-02 introduced this helper; verify it exists before Phase 17 uses it).

### External specs (Phase 17 references)

- BIP-84 (`wpkh(...)` / m/84'/...) — descriptor format for P2WPKH. Already used in v1.3.
- BIP-86 (`tr(...)` / m/86'/...) — descriptor format for P2TR single-key keyspend. NEW in Phase 17.
- BIP-49 (`sh(wpkh(...))` / m/49'/...) — descriptor format for P2SH-wrapped P2WPKH. NEW in Phase 17.
- BIP-322 §"Simple" — message signing spec; the to_spend/to_sign construction Phase 17 builds via `shared::bip322::build_bip322_to_spend / build_bip322_to_sign`.
- BIP-341 §"Key-path spending" — taproot keyspend Schnorr signature; what bdk_wallet 2.3 produces for the P2TR sign path.
- BIP-143 — segwit v0 sighash; what bdk_wallet 2.3 produces for the P2WPKH + P2SH-P2WPKH sign paths.

### Tools / commands relevant to Phase 17 execution

- `cargo test -p client` — primary unit + integration test gate. Phase 17's per-script sign-roundtrip tests + descriptor-generation tests live here.
- `cargo test --test integration full_round` — cross-phase invariant gate. Must remain GREEN at every Phase 17 plan boundary.
- `cargo test --test integration multi_script_client` — Phase 17 new acceptance gate.
- `cargo build --workspace` — compile sanity.
- `cargo audit` — must remain clean (no new dependency additions in Phase 17; reuses existing bdk_wallet + bitcoin + shared).
- `bitcoind -addresstype=bech32` (P2WPKH) / `-addresstype=bech32m` (P2TR) / `-addresstype=p2sh-segwit` (P2SH-P2WPKH) — regtest UTXO generation for the integration test. Phase 16 16-02 added `fund_regtest_typed` for this; Phase 17 reuses.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`client/src/wallet.rs::BdkClientWallet::sign_psbt_input`** (lines 248-288) — Existing PSBT-input signing path for the COINJOIN OUTPUT signature (NOT BIP-322). Uses `bdk_wallet::Wallet::sign(psbt, SignOptions { trust_witness_utxo: true, ..Default::default() })`. **The Phase 17 `sign_bip322` method mirrors this pattern exactly** for P2TR + P2SH-P2WPKH (D-65); the only differences are (a) the PSBT is BIP-322-shaped (to_sign over to_spend per shared::bip322 helpers), (b) the script_pubkey on the witness_utxo is the BIP-322 to_spend SPK, and (c) the witness extraction has a tap_key_sig fallback for P2TR. The trust_witness_utxo Phase 12 lesson #1 carries — the BIP-322 to_spend output has value=0, no non_witness_utxo populated.
- **`client/src/wallet.rs::BdkClientWallet::sign_psbt_input` witness extraction** (lines 273-285) — The exact dual-path pattern (`final_script_witness` primary, `partial_sigs` fallback) Phase 17 reuses for P2TR (`tap_key_sig` fallback) and P2SH-P2WPKH (`final_script_sig + final_script_witness` extraction).
- **`client::round::input::generate_bip322_witness`** (lines 115+) — The carried-forward P2WPKH BIP-322 hand-rolled sign path. Phase 17 17-02 DELETES this and routes through `wallet.sign_bip322(...)` which calls `shared::bip322::sign_simple(ScriptType::P2wpkh, ...)` for WIF wallets or the bdk path for descriptor wallets (CD-24).
- **`client::discover::discover_coordinator` + `parse_onion_from_rr`** (full file) — Existing PKARR resolver returning just `CoordinatorInfo { coordinator_url }`. Phase 17 17-03 extends with `capabilities`. The `pkarr::dns::ResourceRecord` + `RData::TXT` decode pattern + `serde_json::from_str` are reused; the `Partial { onion: Option<String> }` struct grows into `BlindjoinRecord { version, onion, sst, ost }`.
- **`client::config::ClientConfig`** (full file) — Existing clap struct with multiple flags using `#[arg(long, env = "BLINDJOIN_*", default_value = "...")]`. Phase 17's `--type` flag drops into this pattern verbatim.
- **`bdk_wallet::Wallet::create(external_desc, internal_desc).network(bdk_net).create_wallet_no_persist()`** (used at `client/src/wallet.rs:90-93` for `from_descriptor` and at `client/src/wallet.rs:143-146` for `generate`) — Works unchanged for `tr(...)` and `sh(wpkh(...))` descriptors. bdk_wallet 2.3 has full BIP-86 + BIP-49 descriptor support; no new bdk_wallet version pin needed for Phase 17.
- **`bdk_wallet::Wallet::peek_address(KeychainKind::External, 0).address`** (used at `client/src/wallet.rs:152` for generate-wallet banner + `client/src/wallet.rs:238` for coinjoin output address + `client/src/wallet.rs:245` for change address) — Returns a `bitcoin::Address` whose `.script_pubkey()` correctly produces P2TR or P2SH-P2WPKH SPKs when the wallet uses a `tr(...)` or `sh(wpkh(...))` descriptor. No per-type fork needed in address derivation.
- **`bdk_wallet::Wallet::create_single(descriptor)`** at `client/src/wallet.rs:55` — Used by `from_wif` for keychain-less wallets. P2WPKH-only by D-61 — `from_wif` does NOT need extension in Phase 17.
- **`shared::bip322::{bip322_message_hash, build_bip322_to_spend, build_bip322_to_sign}`** — V1.4-MOD-07 single source of truth. Phase 17's per-script `sign_bip322` impls call these for the message hash + to_spend/to_sign tx construction; bit-identical to coordinator-side verify.
- **`shared::protocol::OwnershipProof::to_json_hex_str`** — Has the CD-7 branch (`shared/src/protocol.rs:239`) that emits v1.3 byte-identical array-of-hex form when `version == 1 && psbt_input_b64.is_none() && script_type.is_none()`. Phase 17's WALLET-04 compat shim relies on this — no separate v1 serialiser.

### Established Patterns

- **"shared crate is the contract"** (v1.0+ pattern) — `client` imports `shared::bip322::*` and `shared::protocol::*`; no client-local BIP-322 re-implementation. Phase 17's per-script sign dispatcher calls `shared::bip322::sign_simple` for P2WPKH and uses the script-NEUTRAL primitives (`bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign`) for P2TR + P2SH-P2WPKH PSBT construction.
- **Single-use CLI wallet using `peek_address`** (v1.3 pattern) — `peek_address` does not mutate wallet state; no persistence layer needed. Phase 17's per-type wallets inherit this; the CoinJoin round uses index 0 only.
- **`#[arg(long, env = "BLINDJOIN_*", default_value = "...")]`** (client CLI pattern) — Existing convention at `client/src/config.rs`. Phase 17's `--type` follows. CD-22 keeps single-underscore env-var naming.
- **`trust_witness_utxo: true` PSBT sign discipline** (v1.3 Phase 12 lesson #1) — Required when signing over a witness_utxo without populating non_witness_utxo. Phase 17 reuses verbatim for the new P2TR + P2SH-P2WPKH BIP-322 PSBT signs. The BIP-322 to_spend tx has value=0; trust_witness_utxo's "malicious coordinator that lies about value cannot steal funds" reasoning at `client/src/wallet.rs:259-267` carries (BIP-322 is OFF-CHAIN — there is no value to lie about; the signature only proves message ownership).
- **PKARR pre-Tor resolution** (v1.0 Phase 4 / 5) — `discover::discover_coordinator` runs in `main.rs` BEFORE `tor::init_tor`. Phase 17 WALLET-03 fail-fast happens at the resolver layer, structurally before Tor init. No runtime ordering hack needed.
- **Typed error enum with `thiserror`** (client crate convention) — Phase 17 adds `DiscoveryError` per D-72; the `anyhow::Result` boundary at `main.rs` converts to actionable user-facing strings.
- **No PII in user-facing errors** (PROJECT.md constraint) — Phase 17 discovery errors name the coordinator pubkey (z32 string, public) + the missing script type (enum value); never the user's wallet info, IP, or UTXO. D-72 message shape encodes this.
- **Round/input.rs single-call helper pattern** (v1.0+ pattern) — `register_input` is a flat function that walks the 6 steps (decode pubkey, blind, sign BIP-322, post, decode blind sig, unblind). Phase 17 keeps this shape; the v1/v2 envelope branch is an `if` inside step 4, not a separate helper module.

### Integration Points

- **Phase 17 → Phase 16 (consumes the wire shapes):** Phase 17 client decodes the PKARR record + `/round/info` shapes Phase 16 produced. The `#[serde(default)]` defaults Phase 16 added make v1.3 records / responses decode without errors; Phase 17 detects "legacy" via the `version` field or absence of `sst`.
- **Phase 17 → Phase 15 (consumes the verify + sign API):** Phase 17 client calls `shared::bip322::sign_simple(ScriptType::P2wpkh, ...)` directly for WIF wallets; calls `verify_simple` in the sign-roundtrip test to assert the bdk-produced witness verifies. Per Phase 15 CD-6, P2TR + P2SH-P2WPKH `sign_simple` are `todo!()` in production; Phase 17 explicitly bypasses them via the bdk path.
- **Phase 17 → Phase 18 (INTEG-01):** The mixed-script E2E test in Phase 18 needs the Phase 17 client supporting all 3 types end-to-end (input registration through broadcast). Phase 17's `multi_script_client.rs` covers single-input registration per type; Phase 18 INTEG-01 chains the full round across multiple participants with mixed types.
- **Phase 17 → Phase 18 (INTEG-02 liquidity bot):** The liquidity bot consumes `coordinator.toml`'s `[bip]` config + the PKARR `sst` field. Phase 17 closes the client side of the WALLET-04 compat shim; Phase 18 INTEG-02 tests this against a real v1.3 client BINARY (ROADMAP Phase 18 success criterion #5). Phase 17's stubbed-coordinator test (D-78) is the structural gate; Phase 18 is the binary integration gate.
- **Phase 17 ↔ existing CLI flags:** `--utxo`, `--utxo-wif`, `--descriptor`, `--utxo-address`, `--generate-wallet`, `--coordinator-url`, `--pkarr-pubkey`, `--use-tor`. `--type` composes cleanly with all of them. Existing mutual-exclusion (`--utxo-wif` ↔ `--descriptor`) is unchanged; `--type` constrains BOTH paths.

</code_context>

<specifics>
## Specific Ideas

- **`--type` flag wire form** matches the coordinator: `p2wpkh | p2tr | p2sh-p2wpkh` (lowercase kebab-case; same string serde uses for `ScriptType`). The user types the same identifier they would set in `coordinator.toml` `[bip] output_script_type`. CD-17 keeps it case-sensitive.
- **The `Bip322SignedProof` struct** (D-64) is intentionally small — 4 fields, all owned by the wallet. It is NOT a wire type; do not derive `Serialize`/`Deserialize`; do not export from the client crate's public lib surface. It lives in `client::wallet` per CD-18.
- **Witness extraction priority for P2TR** (D-65 / Sprint-0-B `sprint-0-B.md:317-319`): `final_script_witness` FIRST (bdk_wallet 2.3 current behaviour), `tap_key_sig` fallback. The fallback is a future-proofing hedge — if a future bdk version moves the sig back into `tap_key_sig` (the BIP-371 PSBT v2 field), Phase 17 does not break. Document the dual-path inline as `// Sprint-0-B finding: bdk_wallet 2.3 puts the keyspend sig in final_script_witness[0]; tap_key_sig is cleared at finalisation. Dual-path for future bdk-version resilience.`
- **PKARR record `version: "0.1.0"` detection** (D-73) is the load-bearing legacy signal. The `_blindjoin` TXT record was added in Phase 4 with no version field; v1.3 records have `version: "0.1.0"` (Phase 4 baseline) or no version field at all. Phase 17 treats absent-version OR `version != "0.2.0"` as legacy via `default_legacy_version() -> "0.1.0"`. Future v0.3.0+ records would need their own compat handling, but that's a v1.5+ problem (CSV-vs-array reconsideration also flagged in Phase 16 deferred ideas).
- **The `multi_script_client.rs` test cases** (D-78): 8 named tests per the boundary plan above. Each test asserts a specific error type via `matches!()` (matches Phase 15's D-34 + Phase 16's D-54 discipline). Names:
  ```
  generate_p2wpkh_wallet_emits_bip84_descriptor
  generate_p2tr_wallet_emits_bip86_descriptor
  generate_p2sh_p2wpkh_wallet_emits_bip49_descriptor
  p2wpkh_sign_roundtrip_verifies
  p2tr_sign_roundtrip_verifies
  p2sh_p2wpkh_sign_roundtrip_verifies
  v13_pkarr_record_with_p2tr_wallet_rejects_before_tor
  v13_pkarr_record_with_p2wpkh_wallet_emits_v1_envelope
  v14_pkarr_record_with_p2tr_wallet_emits_v2_envelope
  ```
  (9 names — added the v1.4 path test as an explicit positive control. Plan-phase may collapse or expand; this is the input-contract sketch.)
- **CRIT-01 client-side grep gate** (D-80): a CI step like `grep -c "CRIT-01" client/src/round/input.rs` must return ≥ 1 (one comment above the `script_type: Some(signed.script_type),` line in the v2 envelope construction). Symmetric with Phase 16's coordinator-side gate at `coordinator/src/bitcoin/utxo.rs`.
- **`DiscoveryError::UnsupportedScriptType` message format** (D-72): exact format binding `"coordinator {pubkey} does not support {required:?} ownership proofs (supports: {supported:?})"` — matches ROADMAP success criterion #3 wording verbatim. Plan-phase may polish the formatting (e.g., wrap `supported` in `[...]` brackets), but the structural information (pubkey + required + supported) is mandatory.
- **Legacy coordinator WARN log** (CD-21): when `info.capabilities.is_legacy == true`, `main.rs` emits:
  ```
  tracing::warn!(
      coordinator_pubkey = %pkarr_key,
      record_version = %info.capabilities.record_version,
      "Detected legacy v1.3 coordinator — using v1 OwnershipProof shim (WALLET-04)"
  );
  ```
  Operator visibility into which coordinators on the network are still v1.3.

</specifics>

<deferred>
## Deferred Ideas

- **Manual P2TR sign fallback (`shared/src/bip322/p2tr.rs::sign_p2tr_keypath`, 80-LOC)** — RETIRED for v1.4 per ADR #4 Sprint-0-B PASS. Re-opens in v1.5 ONLY if bdk_wallet regresses on taproot keyspend finalisation. Documented in `.planning/decisions/v1.4-adr.md` §`#decision-4` Rejected Alternatives and §"Consequences/Negative" (D-15 carry-forward).
- **P2WSH multisig BIP-322 client support** — REQUIREMENTS Future Requirements; v1.5+ candidate. Would need a multi-key sign path on the client + a multi-key verify path in shared::bip322. Out of scope for v1.4 (single-sig P2SH-wrapped P2WPKH is the only P2SH variant accepted under the umbrella).
- **Cross-implementation differential test fixtures** (TEST-EXT-01) — Generate via `ACken2/bip322-js`, check into `tests/fixtures/bip322/`. Catches sighash drift between our impl and the JS reference. v1.5+ per REQUIREMENTS.
- **Regtest on-chain anchor test** (TEST-EXT-02) — Sign BIP-322 + real spend with the same key; broadcast both. bitcoind acceptance of the real spend is the strongest correctness gate against V1.4-CRIT-02. v1.5+ per REQUIREMENTS.
- **Automated backwards-compat integration matrix** (TEST-EXT-03) — Full grid (v1.3 client ↔ v1.4 coordinator, v1.4 client ↔ v1.3 coordinator, mixed-version rounds). Phase 17 covers v1.4→v1.3 informally via stub-mocked PKARR + Phase 18 INTEG-01 covers v1.3→v1.4 client against v1.4 coordinator binary; the full grid lives in v1.5.
- **BIP-44-correct testnet/signet coin-type indexing** (`m/84'/1'/...` for non-mainnet) — Phase 17 keeps `0'` across all networks per D-66 reasoning (preserves byte-exact v1.3 wallet addresses; addresses derive cleanly under BDK regardless of coin index; users with existing v1.3 wallets are not silently broken). BIP-44-strict re-indexing is a v1.5+ migration phase if it materialises (would need a wallet-version detector + lazy re-derivation path).
- **`--type` short form (`-t`)** — Plan-phase discretion per CD-17. Defer if naming collides with any future flag.
- **`bdk_wallet = "=2.3.x"` exact-pin tightening** (Phase 15 RESEARCH A7 + Phase 16 deferred carry-forward) — Phase 17 is the third consumer of bdk_wallet 2.3 (after coordinator + Phase 15 shared/); pin tightening becomes load-bearing if a 2.4 release breaks taproot finalisation. Currently a small drift surface, not load-bearing for Phase 17 behaviour. v1.5+ candidate.
- **`DECISIONS-INDEX.md` rolling summary** — v1.5 candidate per Phase 14 + 15 + 16 CONTEXT carry-overs. The volume of `D-*` decisions (now 80+ across v1.4) is approaching the threshold where a rolling decisions index would help downstream agents avoid full-CONTEXT reads.
- **CSV-vs-array PKARR record format reconsideration** (Phase 16 deferred carry-forward) — At ~4+ script types the byte budget breaches regardless of compact names. v1.5 problem; Phase 17 inherits the CSV decoder unchanged.
- **WALLET-04 binary integration test against real v1.3 client artifact** (ROADMAP Phase 18 success criterion #5) — Phase 17 mocks the v1.3 wire shape via stubbed PKARR record + HTTP body assertions. Phase 18 INTEG-01 holds the real-binary integration test. Clean phase boundary per D-79.
- **Per-coordinator output-type selection UX in `--generate-wallet`** — Future ergonomic improvement: when generating a wallet, optionally take a `--coordinator-pubkey` and pre-validate the wallet's output type against the coordinator's advertised `ost`. v1.5 ergonomic polish, not a v1.4 deliverable.
- **`fund_regtest_typed` helper extension to support batched mixed-type funding for Phase 18 INTEG-01** — Phase 16 16-02 added the per-type helper; Phase 18 may need batched multi-UTXO mixed-type generation. Out of Phase 17 scope.

</deferred>

---

*Phase: 17-Client Multi-Script Wallet & Discovery*
*Context gathered: 2026-05-30 via /gsd:discuss-phase --auto*
*All gray areas auto-resolved per recommended defaults; review CONTEXT.md before /gsd:plan-phase or override specific decisions inline.*
