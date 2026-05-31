# Phase 16: Coordinator Integration & Advertisement - Context

**Gathered:** 2026-05-30
**Status:** Ready for planning
**Mode:** --auto (autonomous decisions per recommended defaults; reviewable in this file)

<domain>
## Phase Boundary

Phase 16 wires the v1.4 multi-script support into the coordinator:

1. **`BipConfig` section** added to `coordinator/src/config.rs` with `allow_p2wpkh`, `allow_p2tr`, `allow_p2sh_p2wpkh` (default all `true`) + `output_script_type` (default `"p2wpkh"`); validated at startup by extending `CoordinatorConfig::validate()`. Env-var overrides via `BLINDJOIN__COORDINATOR__BIP__*` per the existing double-underscore pattern.
2. **`validate_utxo` dispatcher swap** at `coordinator/src/bitcoin/utxo.rs`: deletes the `is_p2wpkh()` hard gate (line ~119 in v1.0); replaces with two-step `detect_script_type(on_chain_spk)` → allowlist-check → `shared::bip322::verify_simple(...)`. Branches on `OwnershipProof.version` (v1=witness-only legacy path, v2=PSBT-input shape). CRIT-01 cross-check (declared `script_type` vs derived) at the dispatch boundary.
3. **PKARR record bump** at `coordinator/src/discovery/pkarr_pub.rs`: schema `"version": "0.1.0"` → `"0.2.0"`, adds `"sst"` (compact name for `supported_script_types`, CSV-encoded `"p2wpkh,p2tr,p2sh-p2wpkh"`) + `"ost"` (compact name for `output_script_type`), keeps payload under the 220-byte warn at `pkarr_pub.rs:76`.
4. **`shared::protocol::InfoResponse` extension**: adds `supported_script_types: Vec<ScriptType>` with `#[serde(default = "default_legacy_supported")]` returning `vec![ScriptType::P2wpkh]` for v1.3↔v1.4 bidirectional compat. `/round/info` handler at `coordinator/src/api/handlers.rs::get_info` populates from config. JSON array on the wire (no byte budget for HTTP JSON).
5. **Structured log line** at the verify-success path: `tracing::info!(round_id = %guard.round_id, script_type = ?derived, "ownership proof verified")` — round-id + script-type only (no PII; no per-participant identifier; no outpoint at INFO level).

**Requirements mapped to this phase** (per `.planning/REQUIREMENTS.md` traceability): ADVERT-01, ADVERT-02, ADVERT-03.

**Not in scope:**
- `shared::bip322` API extension (Phase 15 closed; Phase 16 only CONSUMES via `verify_simple` + `detect_script_type` + `ScriptType` enum).
- Client wallet descriptors / sign path (Phase 17 WALLET-01/02).
- Client discovery-time fail-fast (`WALLET-03`) and v1.4→v1.3 compat shim (`WALLET-04`) — Phase 17.
- Mixed-script E2E integration test + liquidity-bot multi-script — Phase 18 (INTEG-01, INTEG-02).
- Round-state machine fork (D-06 LOCKED — MIXED rounds; round-state machine carries v1.3 shape forward unchanged).
- Per-script-type round denominations, per-script-type ban tracking, per-script-type rate limits — all REQUIREMENTS Out-of-Scope.
- Per-round-per-script-type registration breakdown advertising (D-09 LOCKED — privacy anti-feature).

**Cross-phase invariant (carries to every v1.4 phase boundary):** v1.3 P2WPKH-only `tests/integration/full_round.rs` MUST remain green at this phase boundary. The dispatcher swap routes v1 (`version = 1`) proofs through `shared::bip322::verify_simple(ScriptType::P2wpkh, ...)` with identical byte semantics to the existing `verify_bip322_simple` (Phase 15 confirmed bit-exact compat). If `full_round` goes red, REPAIR-01 lesson #4 applies: abandon the structured plan and pivot to `/gsd:debug`.

**Boundary-only delete in this phase:** `coordinator/src/bitcoin/utxo.rs::verify_bip322_simple` AND the `is_p2wpkh()` gate at the dispatch call site are removed in Phase 16 (their replacement is `shared::bip322::verify_simple` via the new dispatcher). Phase 15 already deleted the coordinator-local `Bip322Error`; Phase 16 completes the coordinator-side migration.

</domain>

<decisions>
## Implementation Decisions

### Carried forward from Phase 14 ADR + Phase 15 (NOT re-asked)

LOCKED upstream. Plan-phase consumes verbatim — no re-litigation.

- **ADR #2 (#decision-2) / D-06:** MIXED rounds. One round queue accepts heterogeneous P2WPKH + P2TR + P2SH-P2WPKH inputs. Round-state machine carries v1.3 shape forward unchanged.
- **D-07** [informational — Phase 14 ADR inheritance]: Outputs single-script-type per round; operator-configured via `[bip] output_script_type` (default `p2wpkh`).
- **D-08** [informational — Phase 14 ADR inheritance]: No per-script-type minimum participants gate.
- **D-09** [informational — Phase 14 ADR inheritance]: Coordinator advertises SUPPORTED SET only. Does NOT advertise per-round per-script-type registration counts.
- **D-10 / CRIT-01:** Coordinator MUST derive `script_type` from on-chain `txout.script_pubkey` and cross-check against client-declared `script_type` at validate-utxo time. Non-negotiable, load-bearing, code-review checked.
- **D-12:** `OwnershipProof.version: u8` envelope. v=1 = v1.3 witness-only, v=2 = v1.4 PSBT-input. Coordinator branches `match proof.version`; unknown version → `UnsupportedProofVersion`.
- **Phase 15 outputs (LOCKED API surface):** `shared::bip322::{ScriptType, Bip322Error, detect_script_type, verify_simple(script_type, spk, witness, message, network), sign_simple(script_type, spk, key, message)}` — Phase 16 calls into this API exclusively; per-script files (`p2wpkh.rs / p2tr.rs / p2sh_p2wpkh.rs`) are `pub(crate)` and unreachable from coordinator.
- **Phase 15 outputs:** `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` flat struct with serde defaults. `InputRegRequest.ownership_proof: String` stays a JSON-encoded envelope. Two-phase try-parse (`Vec<String>` then flat-struct) preserves v1.3 array-of-hex bit-exactness.
- **`Bip322Error` taxonomy (10 variants from Phase 15):** Phase 16 maps ALL variants to `ApiError { code: ErrorCode::InvalidOwnershipProof, message: e.to_string() }` at the handler layer — no new wire `ErrorCode` variants per D-32 / REQUIREMENTS Out-of-Scope.
- **Cross-phase invariant:** `cargo test --test full_round` MUST remain green at every plan boundary in Phase 16.

### A. BipConfig struct shape + location

- **D-35:** **Top-level `[bip]` section** in `coordinator.toml`. Mirrors REQUIREMENTS env-var prefix `BLINDJOIN__COORDINATOR__BIP__*` and matches the existing top-level `[network] / [coordinator] / [discovery]` shape. New struct: `coordinator/src/config.rs::BipConfig` deriving `Debug, Deserialize, Clone`. Field on `CoordinatorConfig` is `pub bip: BipConfig` with `#[serde(default)]` for v1.3-compat config files (legacy = all defaults = all 3 script types allowed + output `p2wpkh`).
- **D-36:** **`BipConfig.validate()` rejects all-false.** At least one of `allow_p2wpkh / allow_p2tr / allow_p2sh_p2wpkh` MUST be true (a coordinator that accepts zero script types is non-functional). `CoordinatorConfig::validate()` extension calls `self.bip.validate()`; error message names the env-var override path. Fail-fast at boot per Phase 8 hardening pattern.
- **D-37:** **`output_script_type` lives in `[bip]`.** Same config domain as the allow_* flags. Type: `ScriptType` (re-exported from `shared::bip322`). Default `ScriptType::P2wpkh`. `BipConfig.validate()` additionally asserts the chosen `output_script_type` is one of the allowed script types — otherwise the coordinator advertises an output it cannot construct.
- **D-38:** **`BipConfig` field shape:**
  ```rust
  #[derive(Debug, Deserialize, Clone)]
  pub struct BipConfig {
      #[serde(default = "default_true")] pub allow_p2wpkh: bool,
      #[serde(default = "default_true")] pub allow_p2tr: bool,
      #[serde(default = "default_true")] pub allow_p2sh_p2wpkh: bool,
      #[serde(default = "default_output_script_type")] pub output_script_type: ScriptType,
  }
  impl BipConfig {
      pub fn allows(&self, script_type: ScriptType) -> bool { ... }
      pub fn supported(&self) -> Vec<ScriptType> { ... } // canonical alphabetical order
      pub fn validate(&self) -> anyhow::Result<()> { ... }
  }
  ```
  `allows()` / `supported()` are the read-side APIs the dispatcher + PKARR producer + `/round/info` handler call.

### B. PKARR + /round/info advertisement encoding

- **D-39 (PKARR field naming — byte budget binding):** Use **compact field names** in the PKARR TXT JSON: `"sst"` for supported_script_types, `"ost"` for output_script_type. Reason: full names + values would push payload from ~175 bytes to ~226 bytes, breaching the 220-byte warn at `pkarr_pub.rs:76` (REQUIREMENTS says "stay under the 220-byte warn threshold"). Compact names land at ~205 bytes. The label `_blindjoin` + PKARR `type: blindjoin-coordinator` provide the schema context for clients to resolve the abbreviations. v1.3 clients fall back to `supported = ["p2wpkh"]` regardless of which fields are present (legacy `#[serde(default)]`).
- **D-40 (CSV encoding):** `"sst"` value is a **comma-separated, alphabetically sorted, lowercase** string: `"p2sh-p2wpkh,p2tr,p2wpkh"`. Alphabetical order is canonical for record-equality tests + makes byte length deterministic. Empty case is impossible (validate() rejects all-false).
- **D-41 (output_script_type advertisement):** Also advertised via PKARR as `"ost": "p2wpkh"` (or whichever output type the operator picked). Client uses this for coordinator selection per D-07 — a participant who wants P2TR outputs picks a coordinator advertising `ost: p2tr`.
- **D-42 (`/round/info` JSON):** Field on `shared::protocol::InfoResponse`:
  ```rust
  #[serde(default = "default_legacy_supported")]
  pub supported_script_types: Vec<ScriptType>,
  #[serde(default = "default_legacy_output")]
  pub output_script_type: ScriptType,
  ```
  Wire form: `"supported_script_types": ["p2wpkh", "p2tr", "p2sh-p2wpkh"]` (alphabetical, kebab-case per Phase 15 D-Q3 RESOLVED). `default_legacy_supported() -> vec![ScriptType::P2wpkh]` for v1.3 client backwards-compat (missing field on the v1.3 wire). `default_legacy_output() -> ScriptType::P2wpkh`.
- **D-43 (PKARR version bump):** `"version": "0.1.0"` → `"0.2.0"`. Coordinator always emits v0.2.0 records starting Phase 16. v1.3 client resolving a v0.2.0 record reads `"version": "0.2.0"` as a string — does not error (v1.3 record parser does no version-equality check on resolved records). The `version` field is informational + future-extensibility hook; the structural compat is via `#[serde(default)]` on the new field both ends.
- **D-44 (byte-budget assertion):** Plan-time inline test in `pkarr_pub.rs::tests`: build a default-config packet, serialize, assert `packet_size < 220` AND verify the warn does NOT fire at the default. This locks the byte budget as a CI gate against future field additions.

### C. validate_utxo dispatcher integration + CRIT-01 cross-check

- **D-45 (version branch location):** Branch lives **inline in `validate_utxo`** via `match proof.version { 1 => ..., 2 => ..., _ => Err(UnsupportedProofVersion) }`. Per D-12 verbatim. Single function, single decision point; no pre-dispatcher abstraction.
- **D-46 (v1 path — legacy P2WPKH):** v=1 path constructs `script_type: None` (no client declaration on the wire), routes through:
  ```rust
  let derived = shared::bip322::detect_script_type(&on_chain_spk)?;
  if !config.bip.allows(derived) { return Err(UnsupportedScriptType); }
  shared::bip322::verify_simple(derived, &on_chain_spk, &witness, &message, network)?;
  ```
  v1.3 P2WPKH proofs route through `verify_simple(ScriptType::P2wpkh, ...)` — Phase 15's bip322-crate adapter is bit-exact on the P2WPKH path; `full_round::*` stays green.
- **D-47 (v2 path — PSBT-input shape):** v=2 path:
  ```rust
  // Decode PSBT envelope
  let psbt_input = decode_psbt_input(&proof.psbt_input_b64)?;
  let witness = extract_witness(&psbt_input)?;
  let declared = proof.script_type.ok_or(Bip322Error::WireFormatMismatch(
      "v2 OwnershipProof requires script_type field".into()))?;

  // CRIT-01: derive from CHAIN, not from client field
  let derived = shared::bip322::detect_script_type(&on_chain_spk)?;
  if declared != derived {
      return Err(Bip322Error::ScriptTypeMismatch { declared, derived });
  }
  if !config.bip.allows(derived) { return Err(Bip322Error::UnsupportedScriptType); }
  shared::bip322::verify_simple(derived, &on_chain_spk, &witness, &message, network)?;
  ```
- **D-48 (v2 with `script_type: None` → WireFormatMismatch):** v2 envelope MUST declare `script_type` (the whole point of v2 is multi-script support). Missing field on v2 = `Bip322Error::WireFormatMismatch("v2 OwnershipProof requires script_type field")`. v1 envelope omits the field naturally — `#[serde(default)]` from Phase 15 produces `None` and the v1 branch handles it.
- **D-49 (CRIT-01 inline comment):** A `// CRIT-01: script_type derived from on-chain script_pubkey, never from client field` comment lives directly above the `let derived = shared::bip322::detect_script_type(&on_chain_spk)?;` line in both v1 and v2 branches. Code review uses the grep `grep -n "CRIT-01" coordinator/src/bitcoin/utxo.rs` to confirm the invariant is present at each dispatch point.
- **D-50 (structured log line):** At verify-success path inside `validate_utxo`:
  ```rust
  tracing::info!(
      round_id = %round_id,
      script_type = ?derived,
      "ownership proof verified"
  );
  ```
  `round_id` is `%`-formatted (Display); `script_type` is `?`-formatted (Debug — emits `P2wpkh / P2tr / P2shP2wpkh`). No outpoint, no address, no witness bytes at INFO level. Matches ROADMAP success criterion #1 phrasing.
- **D-51 (Network parameter source):** `validate_utxo` reads `bitcoin::Network` from `state.config.network.bitcoin_network` (parsed once at startup; cached on `AppState`). Threaded into `verify_simple`'s `network` parameter (CONTEXT D-CD-8 LOCKED).
- **D-52 (error → wire mapping):** All `Bip322Error` variants map to `ApiError { code: ErrorCode::InvalidOwnershipProof, message: format!("{}", e) }` at the handler layer in `coordinator/src/api/handlers.rs::post_input`. Internal `tracing::warn!(error = ?e, "ownership proof rejected")` preserves the typed variant for operator logs. D-32 LOCKED — no new wire `ErrorCode` per script type.

### D. Plan ordering + test strategy

- **D-53 (plan ordering — 3 plans):**
  - **16-01-PLAN.md** = `BipConfig` struct + `BipConfig::validate()` + `CoordinatorConfig::validate()` extension + `shared::protocol::InfoResponse` field extension + `/round/info` handler population. This is the **wire/config-first** atomic commit — landing the wire shape before the behavior change preserves v1.3 REPAIR-01 lesson #1. Tests: `BipConfig::validate()` unit tests (rejects all-false, accepts defaults, env-var override roundtrip); `InfoResponse` serde roundtrip with v1.3 missing-field + v1.4 present-field; `/round/info` integration test reading the new field. Maps requirement **ADVERT-01 (partial — config struct + validation)**.
  - **16-02-PLAN.md** = `validate_utxo` dispatcher swap + CRIT-01 cross-check + version branching + log line + `Bip322Error` import wire-up. Deletes the local `verify_bip322_simple` + `is_p2wpkh()` gate at `coordinator/src/bitcoin/utxo.rs:119`. Tests: `tests/integration/multi_script_validate.rs` new file targeting dispatch decisions per (script_type × allow_config × declared_vs_derived). Maps requirements **ADVERT-01 (wiring) + ADVERT-03 (CRIT-01)**.
  - **16-03-PLAN.md** = PKARR producer schema bump (`"version": "0.2.0"`, `"sst"` + `"ost"` fields) + byte-budget assertion + PKARR resolver-side `#[serde(default)]` shim on the resolved-record type (so the coordinator can re-resolve its own records via the existing client crate without erroring). Tests: byte-budget inline test in `pkarr_pub.rs::tests`; record-resolve roundtrip test. Maps requirement **ADVERT-02**.
  - Each plan is an atomic commit; sequential dependency chain `16-01 → 16-02 → 16-03` (wave 1 → wave 2 → wave 3). Same shape as Phase 15 to maintain CD-10 / REPAIR-01 lesson #1 discipline.
- **D-54 (integration test strategy):** **New file `tests/integration/multi_script_validate.rs`** targeting v1.4 multi-script dispatch + CRIT-01 cross-check. Reuses `BitcoindGuard` + `require_bitcoind!()` from v1.3 unchanged. Test cases (per 16-02 acceptance):
  - P2WPKH UTXO + v1 proof → accept (cross-phase invariant).
  - P2WPKH UTXO + v2 proof with declared=p2wpkh → accept (CRIT-01 declared==derived passes).
  - P2TR UTXO + v2 proof with declared=p2tr → accept.
  - P2SH-P2WPKH UTXO + v2 proof with declared=p2sh-p2wpkh → accept.
  - P2WPKH UTXO + v2 proof with declared=p2tr → reject with `ScriptTypeMismatch` (CRIT-01 spoofing).
  - P2TR UTXO + v2 proof with declared=p2wpkh → reject with `ScriptTypeMismatch`.
  - v2 proof with `script_type: None` → reject with `WireFormatMismatch`.
  - P2TR UTXO with `allow_p2tr = false` config → reject with `UnsupportedScriptType` (allowlist gate).
  - v=3 (unknown version) → reject with `UnsupportedProofVersion`.
  `tests/integration/full_round.rs` remains UNCHANGED at this phase boundary (v1.3 invariant gate).
- **D-55 (PKARR byte-budget inline test):** Inside `coordinator/src/discovery/pkarr_pub.rs::tests`:
  ```rust
  #[test]
  fn coordinator_packet_under_220_byte_budget() {
      let kp = Keypair::random();
      let packet = build_coordinator_packet(&kp, "127.0.0.1:8080", 1_000_000, 3, "ready",
          &["p2wpkh", "p2tr", "p2sh-p2wpkh"], "p2wpkh").unwrap();
      let serialized = serde_json::to_string(&packet).unwrap();
      assert!(serialized.len() < 220, "packet {} bytes exceeds 220 warn budget", serialized.len());
  }
  ```
  Locks the budget as a regression gate. Default-config (all 3 allowed) is the worst case (longest `sst` string).
- **D-56 (no PKARR client-side resolver change in Phase 16):** Phase 17 WALLET-03 owns the client-side discovery resolver changes (read `sst` + `ost` from the resolved record; fail-fast at discovery before opening a Tor circuit). Phase 16 ships the producer side only; resolver-side `#[serde(default)]` shims live in the resolver crate which is shared but not edited in Phase 16 unless the existing `client::discover` code can't roundtrip — verify at plan time.

### Claude's Discretion

- **CD-11:** Whether `BipConfig::supported()` returns `Vec<ScriptType>` in alphabetical order or insertion order from the config struct. Default: **alphabetical** (determinism for record-equality tests + canonical CSV in PKARR). Plan-phase may override if alphabetical-ordering bytes vary across script-type renames.
- **CD-12:** Whether the `validate_utxo` log line emits at INFO level always, or DEBUG when the script_type is P2WPKH (matches v1.3 silence) and INFO when P2TR / P2SH-P2WPKH. Default: **INFO always** (consistent operator-side log shape; v1.3 was silent — small operator-log delta is acceptable). Plan-phase may override to DEBUG to match v1.3 verbosity exactly.
- **CD-13:** Whether the env-var override for `output_script_type` accepts `"p2wpkh" / "p2tr" / "p2sh-p2wpkh"` strings (matches the wire form) or `"P2wpkh" / ...` (matches the Rust enum form). Default: **wire-form lowercase kebab-case** (matches Phase 15's `#[serde(rename_all = "snake_case")]` + `rename = "p2sh-p2wpkh"` shape). Plan-phase verifies the `config` crate's env-var deserializer routes via serde (yes — it does).
- **CD-14:** Whether the v1 proof path passes `network: bitcoin::Network` to `verify_simple` or hardcodes signet (legacy v1.3 was network-agnostic at this layer). Default: **always pass the config network**; the bip322 crate adapter's `Address::from_script(spk, network)` is network-sensitive for P2SH only, but routing all paths through the same parameter is cleaner than a v1-vs-v2 fork. Plan-phase verifies.
- **CD-15:** Plan boundaries for the `verify_bip322_simple` + `is_p2wpkh()` deletion. Default: **delete inside 16-02-PLAN.md** atomic commit (the dispatcher swap commit). Plan-phase may choose to defer the deletion to a follow-up 16-04 if the diff gets unwieldy.
- **CD-16:** Whether the `multi_script_validate` integration test mocks the bitcoind RPC (faster) or uses `BitcoindGuard` (real, matches `full_round`). Default: **`BitcoindGuard`** — real regtest UTXOs are the only reliable way to exercise the `on_chain_spk` path. Mocking would bypass the very layer CRIT-01 is enforced at. Plan-phase verifies the regtest helper can generate P2TR + P2SH-P2WPKH UTXOs via `getnewaddress -address_type bech32m` / `p2sh-segwit`.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner, gsd-executor) MUST read these before planning or implementing.**

### Phase 14 ADR + Phase 15 outputs (LOCKED inputs)

- `.planning/decisions/v1.4-adr.md` §`#decision-2` — MIXED rounds, D-06..D-10 rationale; Phase 16's input contract.
- `.planning/phases/14-sprint-0-spikes-discuss-phase-decisions/14-CONTEXT.md` §D-06..D-10 — output_script_type config knob, no-per-script-type-min-participants gate, advertisement-boundary lock, CRIT-01 invariant.
- `.planning/phases/15-shared-crate-multi-script-contract/15-CONTEXT.md` §D-22..D-32 — wire shape locked (OwnershipProof flat struct + `version` envelope + `script_type` sibling field).
- `.planning/phases/15-shared-crate-multi-script-contract/15-RESEARCH.md` §"Open Questions (RESOLVED)" Q3 — `#[serde(rename_all = "snake_case")]` + explicit `#[serde(rename = "p2sh-p2wpkh")]` on `P2shP2wpkh` — Phase 16 uses this same shape on the new `InfoResponse.supported_script_types` + `BipConfig.output_script_type` fields.
- `.planning/phases/15-shared-crate-multi-script-contract/15-VERIFICATION.md` — Confirms shared::bip322 API surface stable + CRIT-01 mitigated at the API level (per-script `pub(crate)` files).
- `.planning/phases/15-shared-crate-multi-script-contract/15-01-SUMMARY.md`, `15-02-SUMMARY.md`, `15-03-SUMMARY.md` — Implementation details + auto-fix deviations (BIP-322 `to_sign` Version(0) + OP_RETURN 1-byte) Phase 16 inherits.

### Project-level anchors

- `.planning/PROJECT.md` §"Current Milestone: v1.4 BIP-322 Multi-Script Support" — milestone goal, target features, out-of-v1.4-scope list.
- `.planning/PROJECT.md` §"Constraints" — no custom crypto, MIT, Tor-native, signet-first, NO PII logging (binds D-50 log-line shape).
- `.planning/REQUIREMENTS.md` §"v1.4 Requirements" — ADVERT-01, ADVERT-02, ADVERT-03 mapped to Phase 16 §"Traceability". Out-of-Scope table is binding (no per-script-type ban list, no per-script-type rate limits, no per-round breakdown advertising).
- `.planning/ROADMAP.md` §"Phase 16" — 5 success criteria (P2TR accepted with default config; allowlist gate works; PKARR record bump + byte budget; CRIT-01 cross-check rejects spoof; v1.3 invariant + v1.3-client interop verified inline).
- `.planning/STATE.md` §"Accumulated Context" + §"Carry-forward constraints" — v1.3 REPAIR-01 lessons #1 (wire-format ships first) and #4 (pivot to /gsd:debug on carry-forward shape).

### Phase 15 outputs (the API Phase 16 consumes)

- `shared/src/bip322/mod.rs` — `ScriptType`, `Bip322Error` (10 variants), `detect_script_type`, `verify_simple(script_type, spk, witness, message, network)`. Phase 16 calls these exclusively; no per-script function visible from coordinator/.
- `shared/src/protocol.rs` (post-15-01) — `OwnershipProof { version, witness_stack, psbt_input_b64, script_type }` flat struct; `InputRegRequest.ownership_proof: String` envelope.

### Code anchors (Phase 16 modifies the first 4, references the rest)

- `coordinator/src/config.rs` — Add `pub struct BipConfig` + `pub bip: BipConfig` field on `CoordinatorConfig` + extend `CoordinatorConfig::validate()` (existing at lines 157-188).
- `coordinator/src/bitcoin/utxo.rs` — Swap `validate_utxo` to call `shared::bip322::detect_script_type` + `verify_simple`; delete `verify_bip322_simple` (current at lines 112+) + `is_p2wpkh()` gate (current at line ~119); CRIT-01 comments at both branches of the version match.
- `coordinator/src/discovery/pkarr_pub.rs` — Add `sst` + `ost` fields to JSON record; bump `"version"` to `"0.2.0"`; byte-budget test addition.
- `shared/src/protocol.rs::InfoResponse` (lines 13-28) — Add `supported_script_types: Vec<ScriptType>` + `output_script_type: ScriptType` with `#[serde(default = "...")]` for v1.3 compat.
- `coordinator/src/api/handlers.rs::get_info` (lines 46-67) — Populate new `InfoResponse` fields from `state.config.bip`.
- `coordinator/src/api/handlers.rs::post_input` (around line 136) — Already calls `OwnershipProof::from_json_hex_str` and routes to `validate_utxo`. No change here in Phase 16; the dispatch happens INSIDE validate_utxo.
- `client/src/discover.rs` — Phase 16 does NOT edit; Phase 17 WALLET-03 owns resolver-side changes. Phase 16 verifies the existing parser tolerates new fields via `#[serde(default)]`.
- `tests/integration/full_round.rs` — DO NOT MODIFY. v1.3 invariant gate.
- `tests/integration/multi_script_validate.rs` — NEW file in 16-02 covering 9 dispatch + CRIT-01 cases.

### Cross-phase invariant references

- `tests/integration/full_round.rs` lines 1-30 — header comment + `require_bitcoind!()` usage; pattern for the new multi_script test.
- `tests/integration/mod.rs` — `BitcoindGuard` + `fund_regtest` helpers carry forward unchanged.

### External specs (Phase 16 references)

- BIP-86 / BIP-49 / BIP-84 descriptor formats — referenced for `output_script_type` semantics; consumed by Phase 17 WALLET-01.
- Wasabi PR #8912 (`AllowP2trInputs`) — precedent for the operator-tunable allowlist + the "per-coordinator output type" pattern.

### Tools / commands relevant to Phase 16 execution

- `cargo test -p coordinator` — primary unit + integration test gate.
- `cargo test --test integration full_round` — cross-phase invariant gate.
- `cargo test --test integration multi_script_validate` — Phase 16 new acceptance gate.
- `cargo build --workspace` — compile sanity.
- `cargo audit` — must remain clean.
- `bitcoind -addresstype=bech32m / p2sh-segwit / bech32` — regtest UTXO generation for the multi-script integration test.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`coordinator/src/config.rs::CoordinatorConfig::validate()`** (lines 157-188) — Existing Phase 8 pattern (`anyhow::ensure!` with named env-var override hints in the error message). `BipConfig::validate()` reuses this style verbatim. The extension is one new section at the end of the existing function body.
- **`coordinator/src/config.rs::with_defaults()`** (around line 200) — Test-only default builder. `BipConfig::default_for_tests()` follows the same pattern (all_3_allowed = true, output = P2wpkh).
- **`coordinator/src/discovery/pkarr_pub.rs::build_coordinator_packet`** (lines 50-90) — Existing JSON record builder; Phase 16 extends the `serde_json::json!({...})` literal with two new keys. The 220-byte warn at line 76 is the budget gate; Phase 16's `D-55` test makes the warn a hard CI assertion at the default config.
- **`coordinator/src/api/handlers.rs::get_info`** (lines 46-67) — Existing handler builds `InfoResponse` from `state.config.coordinator` + round state. Phase 16 adds two field reads from `state.config.bip` to the JSON construction.
- **`shared::bip322::verify_simple` + `detect_script_type` + `ScriptType`** (Phase 15 outputs) — Phase 16 imports these. The bip322-crate adapter inside `verify_simple` handles the bit-exact P2WPKH path so v1.3 proofs route through identically.
- **`shared::protocol::OwnershipProof::from_json_hex_str`** (Phase 15 two-phase try-parse) — Accepts both v1.3 array-of-hex AND v2 flat-struct shapes; the coordinator handler call site at `coordinator/src/api/handlers.rs:136` is unchanged.
- **`tests/integration/mod.rs::BitcoindGuard` + `fund_regtest`** (v1.3 Phase 9) — Script-type-agnostic; reused by the new `multi_script_validate.rs` for P2WPKH + P2TR + P2SH-P2WPKH UTXO generation.
- **The `is_p2wpkh()` boolean check at `coordinator/src/bitcoin/utxo.rs:119`** — Reference for what gets DELETED. Its single use site is in `validate_utxo`; nothing else in the codebase calls `is_p2wpkh()` directly on `script_pubkey`.
- **Existing `tracing::info!` patterns in `coordinator/src/`** — Phase 16's new log line at D-50 matches the project's structured-field convention (already used in handlers + round state machine).

### Established Patterns

- **"shared crate is the contract"** (v1.0 pattern, preserved through v1.3 + reinforced in v1.4) — `coordinator` imports `shared::bip322::*` exclusively for BIP-322 work; no coordinator-local re-implementation.
- **Config-validated-at-startup** (Phase 8 hardening) — `CoordinatorConfig::validate()` is called once from `coordinator/src/main.rs` startup; Phase 16's `BipConfig::validate()` extension follows the same path.
- **Per-route rate limiting** (v1.2 Phase 8) — `/round/info`'s existing read-bucket rate limit (default 60/min) applies unchanged to the extended response. No new endpoint added.
- **`tower_governor` + `tower-http` middleware stack** (v1.2 Phase 8) — Untouched in Phase 16.
- **Memory-only round state with `#[zeroize(skip)]` on non-PII fields** (v1.0) — Phase 16 doesn't add fields to `RoundState` directly; the new `script_type` is derived per-registration, not persisted.
- **PKARR record sign-and-publish flow** (v1.0 Phase 4) — Untouched in Phase 16; only the JSON inside the TXT record changes. Signing + DHT publish path identical.
- **`Bip322Error` mapped to `ErrorCode::InvalidOwnershipProof`** (Phase 15 D-32) — Phase 16's new variants (`WireFormatMismatch`, `UnsupportedProofVersion`, `ScriptTypeMismatch`, `UnsupportedScriptType`) all route through the same handler-layer mapping. Zero new wire `ErrorCode` variants.
- **v1.3 REPAIR-01 lesson #1** — Wire-format ships FIRST. Plan ordering (16-01 = wire/config, 16-02 = behavior, 16-03 = advertisement) honors this.

### Integration Points

- **Phase 16 → Phase 17 (WALLET-03):** `InfoResponse.supported_script_types` + PKARR `sst` field are what the Phase 17 client reads to fail-fast at discovery. Phase 16 fixes the wire shape; Phase 17 builds the client-side resolver check.
- **Phase 16 → Phase 17 (WALLET-04):** v1.3-client-against-v1.4-coordinator compat shim: the v1.3 client sends `version: 1` proof; Phase 16's v1 branch handles it identically to v1.3 behavior (same `verify_simple(ScriptType::P2wpkh, ...)` call). Phase 17 WALLET-04 implements the inverse (v1.4 client against v1.3 coordinator) via `#[serde(default)]` on the resolved-record type.
- **Phase 16 → Phase 18 (INTEG-01):** The `multi_script_validate.rs` integration test in 16-02 covers per-input dispatch; Phase 18 INTEG-01 builds on top with a full E2E (input + output + signing + broadcast). Phase 16 unblocks Phase 18 by landing the dispatcher behavior on `main`.
- **Phase 16 → Phase 18 (INTEG-02):** Liquidity bot reads `supported_script_types` from PKARR to know what UTXO types to generate. Phase 16's `sst` field + `ost` field are the wire contract Phase 18's bot consumes.

</code_context>

<specifics>
## Specific Ideas

- **Compact PKARR field names** (D-39): `"sst"` and `"ost"`. The byte budget forces compactness; verbose names would breach the 220-byte warn. Future extensions (v1.5+) MUST also use 3-character compact names or accept the breach + bump the warn threshold.
- **CSV alphabetical canonical order** (D-40): `"p2sh-p2wpkh,p2tr,p2wpkh"` is the deterministic form. Encoding via `BipConfig::supported()` (returns sorted Vec) + `.join(",")` produces this canonically.
- **The `multi_script_validate.rs` test cases** (D-54): 9 named tests covering the 3×3 (script_type × declared/derived) matrix + 2 envelope-shape edge cases (v1, v=3). Each test asserts a specific `Bip322Error` variant via `matches!()` (matches Phase 15's D-34 discipline). v1.4-CRIT-01 spoofing test names mirror Phase 15's:
  ```
  validate_p2wpkh_utxo_with_v1_legacy_proof_ok
  validate_p2wpkh_utxo_with_v2_declared_p2wpkh_ok
  validate_p2tr_utxo_with_v2_declared_p2tr_ok
  validate_p2sh_p2wpkh_utxo_with_v2_declared_p2sh_p2wpkh_ok
  validate_p2wpkh_utxo_with_v2_declared_p2tr_rejects_spoofing
  validate_p2tr_utxo_with_v2_declared_p2wpkh_rejects_spoofing
  validate_v2_proof_without_script_type_rejects_wireformat_mismatch
  validate_p2tr_utxo_with_allow_p2tr_false_rejects_unsupported
  validate_unknown_version_3_rejects_unsupported_proof_version
  ```
- **`BipConfig::validate()` error message format** (D-36): Use the existing Phase 8 pattern verbatim. Example:
  ```rust
  anyhow::ensure!(
      self.allow_p2wpkh || self.allow_p2tr || self.allow_p2sh_p2wpkh,
      "bip section requires at least one allow_* flag = true; got all false. \
       Set BLINDJOIN__COORDINATOR__BIP__ALLOW_P2WPKH=true (or another flag) \
       to enable input acceptance for that script type.",
  );
  anyhow::ensure!(
      self.allows(self.output_script_type),
      "bip.output_script_type = {:?} but the matching allow_* flag is false. \
       The coordinator cannot advertise an output type it cannot accept on its own \
       round outputs. Set the matching BLINDJOIN__COORDINATOR__BIP__ALLOW_{...}=true \
       or change BLINDJOIN__COORDINATOR__BIP__OUTPUT_SCRIPT_TYPE.",
      self.output_script_type,
  );
  ```
- **PKARR byte-budget worst case** (D-44): Default config (all 3 allowed) maximises the `sst` string. Current ~175 bytes + `"sst":"p2sh-p2wpkh,p2tr,p2wpkh"` = ~28 bytes new + `"ost":"p2wpkh"` = ~15 bytes new + 2 comma separators = ~220 bytes worst case. Tight — the inline `D-55` test catches any future field that pushes over.
- **The CRIT-01 grep gate**: a CI step like `grep -c "CRIT-01" coordinator/src/bitcoin/utxo.rs` must return ≥ 2 (one comment for v1 branch, one for v2 branch). Plan-phase decides whether this is a CI step or just a manual code-review item. Defaulting to CI step for symmetry with Phase 15's `bip322-pin-check` job.

</specifics>

<deferred>
## Deferred Ideas

- **Per-round-per-script-type registration breakdown advertisement** — Anti-feature per REQUIREMENTS Out-of-Scope (leaks correlation). NOT a v1.5 candidate either.
- **Per-script-type ban tracking / rate limits / denominations** — All anti-features per REQUIREMENTS Out-of-Scope. The Tor-safe `GlobalKeyExtractor` (v1.2 hardening) is incompatible with per-script-type buckets.
- **Mixed output script types per round (Wasabi 2.0.3-style)** — REQUIREMENTS Out-of-Scope; separate output-policy milestone, v1.5+.
- **PKARR resolver-side `#[serde(default)]` shim on the resolved-record type in `client/src/discover.rs`** — Phase 17 WALLET-03/04 territory; Phase 16 producer side only.
- **Tor-mode UAT harness** — Phase 8 HUMAN-UAT item 3 carry-forward to v1.5+.
- **REPAIR-01 PR observation closure** — v1.5 process step, not a v1.4 code deliverable.
- **B-03 dynamic fee estimation** — pre-mainnet requirement; v1.5+.
- **TEST-EXT-01/02/03 cross-impl differential, on-chain anchor, automated backwards-compat matrix** — v1.5+ per REQUIREMENTS Future Requirements.
- **DECISIONS-INDEX.md rolling summary** — v1.5 candidate per Phase 14 + Phase 15 CONTEXT carry-overs.
- **CSV-vs-array PKARR record format reconsidertion if v1.5 adds more script types** — At ~4+ script types the byte budget breaches regardless of compact names; reconsider the `sst` encoding (e.g., bitmask, single-char codes). v1.5 problem.
- **`bdk_wallet = "=2.3.x"` exact-pin tightening** (Phase 15 RESEARCH A7 deferred) — v1.5+ candidate; current caret pin is a small drift surface but not load-bearing for Phase 16 behavior.

</deferred>

---

*Phase: 16-Coordinator Integration & Advertisement*
*Context gathered: 2026-05-30 via /gsd:discuss-phase --auto*
*All gray areas auto-resolved per recommended defaults; review CONTEXT.md before /gsd:plan-phase or override specific decisions inline.*
