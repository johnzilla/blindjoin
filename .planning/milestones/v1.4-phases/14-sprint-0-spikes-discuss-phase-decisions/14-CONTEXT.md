# Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions - Context

**Gathered:** 2026-05-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 14 is the **gating, ADR-producing** phase that opens v1.4. Its job is to resolve every load-bearing decision before any production code is written, so Phases 15-18 have unambiguous specifications. Three deliverables:

1. **Sprint-0-A** — `cargo tree -p bip322 0.0.10` check on a throwaway branch (`spike/14-A-bip322-cargo-tree`). Output captured in `.planning/research/sprint-0-A.md` with GO/NO-GO verdict against the 3 explicit gates (bitcoin pin = 0.32.x, cargo audit clean, adapter <50 LOC zero-lossy). Settles Open Decision #1.

2. **Sprint-0-B** — bdk_wallet 2.3 P2TR descriptor + BIP-322 message signing proof-of-concept on a separate throwaway branch (`spike/14-B-bdk-p2tr-poc`). Output captured in `.planning/research/sprint-0-B.md`: either bdk produces a valid 64-byte Schnorr keypath witness (use bdk path in Phase 17) OR it does not (manual `secp256k1::Secp256k1::sign_schnorr` fallback pre-spec'd in Decision #4 below). Settles Open Decision #4.

3. **ADR** — `.planning/decisions/v1.4-adr.md` using the Michael Nygard ADR template per decision. Records resolutions for all four Open Decisions (#1 crate adopt/extend, #2 mixed vs segregated rounds, #3 wire format B1/B2/B3, #4 bdk P2TR sign path) with one section per decision: Context / Decision / Status / Consequences (positive, negative, neutral) / Rejected Alternatives. Top-level `## Spike Outputs` section links sprint-0-A.md and sprint-0-B.md.

**Net effect:** Phase 15 plan-phase can derive concrete tasks for BIP322-01..04 and ADVERT-04 from the ADR without re-litigating the choice between adopt and extend, mixed and segregated, or the three wire-format options. Phase 17 plan-phase has unambiguous instruction on the P2TR client sign path (bdk OR pre-spec'd manual fallback).

**Not in scope:** any production code change to `coordinator/`, `client/`, `shared/`, or `liquidity-bot/`; merging spike branches into `main`; tackling Decision #3 wire-format roundtrip test (that's Phase 15's first deliverable); v1.5+ carry-forwards (Tor-mode UAT, REPAIR-01 PR observation, B-03 dynamic fee estimation, P2WSH multisig); reconsidering the v1.0 baseline stack; resolving items beyond the 4 Open Decisions identified in `.planning/research/SUMMARY.md`.

**Cross-phase invariant (carries to every v1.4 phase boundary):** v1.3 P2WPKH-only `full_round::*` integration tests must remain green. Phase 14 closes this gate trivially (no production code touched), but the discipline is documented here for Phases 15-18 to inherit.

</domain>

<decisions>
## Implementation Decisions

### Decision #1 — `bip322` crate adopt vs extend

- **D-01 (posture):** **Default EXTEND**, adopt only if Sprint-0-A passes all gates. Conservative posture per PITFALLS researcher and v1.3 REPAIR-01 forensics. ADR Decision #1 STATUS = "ACCEPTED (with conditional flip on Sprint-0-A pass)".
- **D-02 (GO-criteria for ADOPT, all must pass):**
  1. `cargo tree -p bip322` shows `bitcoin = 0.32.x` (NOT 0.31.x or earlier).
  2. `cargo audit` clean on the transitive graph (no advisories on the new edges).
  3. Adapter wrapping the crate's `verify_simple(&Address, message, Witness)` to our wire shape `(scriptPubKey, witness, message)` is **< 50 LOC** with **zero lossy conversions** (no `unwrap_or`, no field-shape squashing).
- **D-03 (ambiguous policy):** **Anything short of all three gates passing cleanly = EXTEND.** Sprint-0-A's verdict is binary by design. No mid-spike user escalation. Removes the "borderline adapter" risk that Open Decision #1 carries.
- **D-04 (extend code organization, applies when D-01's default fires):** Module split per script type:
  ```
  shared/src/bip322/
    mod.rs           # public API + dispatcher (ScriptType enum, detect_script_type, verify_simple, sign_simple)
    p2wpkh.rs        # existing P2WPKH BIP-143 sighash + verify + sign (carried over from shared/src/bip322.rs)
    p2tr.rs          # new — BIP-341 Taproot keypath sighash + Schnorr verify + sign
    p2sh_p2wpkh.rs   # new — BIP-143 sighash over unwrapped P2WPKH redeem script + HASH160 check
  ```
  Each per-type file owns its sighash + signature primitive + witness arity check in isolation. Makes Phase 15 spec-vector test failures localizable to one file. Keeps the door open for v1.5 swap (crate adoption OR script-type addition/removal) without re-architecture.
- **D-05 (asymmetry note for the ADR):** Per STACK researcher, `bdk_wallet` does NOT ship BIP-322 signing (issue #150 open since May 2023). **The client signer is OURS regardless of Decision #1.** Adopt-vs-extend only affects the `verify` path in `shared/`, never the `sign` path. The ADR Decision #1 Consequences section MUST surface this asymmetry — it sharpens the conservative posture.

### Decision #2 — mixed vs segregated script-type rounds

- **D-06:** **MIXED rounds.** One round queue accepts heterogeneous inputs (P2WPKH + P2TR + P2SH-P2WPKH together). Coordinator round-state machine unchanged from v1.3 conceptually. ADR Decision #2 STATUS = "ACCEPTED". Researcher split (FEATURES → mixed, PITFALLS → segregated) is recorded in the ADR's Consequences (negative) section.
- **D-07:** **Outputs remain single-script-type per round** (REQUIREMENTS.md Out-of-Scope, locked). The output type for any given round is **operator-configured per-coordinator** via `coordinator.toml`:
  ```toml
  [bip]
  allow_p2wpkh      = true
  allow_p2tr        = true
  allow_p2sh_p2wpkh = true
  output_script_type = "p2wpkh"   # default; coordinator-wide; advertised via PKARR
  ```
  Default = `p2wpkh` (most common, most anonymous-looking). Same output type for every round this coordinator runs. Participants who want a different output type pick a different coordinator (PKARR's whole point — coordinators are disposable).
- **D-08:** **No per-script-type minimum participants gate.** Keep the existing v1.0 round minimum (total participant count only). Per-type minimums would fragment matchmaking the same way segregated rounds do, defeating the reason MIXED was chosen. The heterogeneous-input chain-analysis fingerprint (per V1.4-MOD-06) is documented as a known limitation in the v1.4 README; liquidity bot's per-round type rotation (V1.4-MIN-02 mitigation) softens the worst case (lone rare-type participant).
- **D-09 (advertisement boundary, lock):** Coordinator advertises the **supported set** over PKARR (`supported_script_types`) and `/round/info`. Coordinator does NOT advertise **per-round registration breakdown** by script type — that's an REQUIREMENTS.md anti-feature (leaks correlation). Internal aggregate counters per script type are operator-facing only (log-level / metrics, not on a public endpoint).
- **D-10 (load-bearing invariant — CARRIES TO PHASE 16):** Coordinator MUST derive `script_type` from the on-chain `txout.script_pubkey` and cross-check against the client-declared `script_type` at validate-utxo time. A client claiming P2WPKH for a P2TR UTXO must be rejected even if its witness happens to verify. CRIT-01 mitigation, code-review checked in Phase 16.

### Decision #3 — P2SH-P2WPKH `OwnershipProof` wire format

- **D-11:** **B2 — base64 PSBT-input shape.** `OwnershipProof.psbt_input_b64: String` holding `base64(bitcoin::psbt::Input)`. Natively carries `final_script_sig` + `final_script_witness` + future PSBT fields. Aligns with the PSBT-everywhere round contract. ARCHITECTURE researcher's recommendation. Accepts the byte-overhead tradeoff (~+100 bytes per P2WPKH proof vs witness-only) because the PSBT-everywhere alignment is the bigger lever for v1.5 wire-format extensions.
- **D-12:** **Explicit `version: u8` field** on the OwnershipProof envelope for backwards-compat. **`version = 1`** = v1.3 shape (witness-only, defaults from `#[serde(default)]`). **`version = 2`** = v1.4 PSBT-input shape. Coordinator branches on version:
  ```rust
  match proof.version {
      1 => parse_v1_witness_path(&proof.witness, ...),
      2 => parse_v2_psbt_path(&proof.psbt_input_b64, ...),
      _ => return Err(UnsupportedProofVersion),
  }
  ```
  WALLET-04 fallback shim becomes: "if PKARR record `version = "0.1.0"` OR `/round/info` lacks `supported_script_types`, send `version = 1` shape with `witness` field; otherwise send `version = 2` PSBT shape." Rejected alternatives (untagged serde enum, optional-fields-with-default) recorded in ADR Decision #3 Rejected Alternatives.
- **D-13:** **Cross-version + reject-malformed roundtrip tests** for the new `OwnershipProof`. The test ships in `shared/` BEFORE either coordinator or client uses the new shape (v1.3 REPAIR-01 lesson #1, non-negotiable phase boundary for Phase 15). Test cases:
  1. `version = 2` self-roundtrip for all 3 script types (P2WPKH, P2TR, P2SH-P2WPKH) → passes.
  2. `version = 1` (v1.3 shape, just `script_pubkey + witness + message`) deserializes into v1.4 type with `script_type` defaulted to `P2WPKH` → passes.
  3. `version = 2` with mismatched declared `script_type` vs PSBT-input contents (e.g., declares P2TR but the PSBT has a P2WPKH redeem script) → rejects with `WireFormatMismatch`.
  4. `version = 3` (or any unknown version) → rejects with `UnsupportedProofVersion`.
  5. `version = 2` with corrupted base64 / truncated PSBT → rejects without panicking; surfaces decode error type.
  ~10-15 cases total. Phase 15 cost: ~+0.5 day vs minimal roundtrip-only.

### Decision #4 — bdk_wallet 2.3 multi-descriptor sign path (resolved by Sprint-0-B; fallback pre-spec'd here)

- **D-14 (resolved by spike):** ADR Decision #4 STATUS is left **"PENDING (Sprint-0-B)"** at Phase 14 start; updated to "ACCEPTED (bdk path)" or "ACCEPTED (manual fallback)" when Sprint-0-B's `.planning/research/sprint-0-B.md` lands. The verdict is binary by design (matches Decision #1 ambiguous policy): bdk produces a 64-byte Schnorr keypath witness that verifies under our `shared::bip322::p2tr::verify_simple`, or it does not.
- **D-15 (fallback PRE-SPEC, applies if Sprint-0-B fails):** Manual sign path lives in **`shared/src/bip322/p2tr.rs`** as `sign_p2tr_keypath()` using `bitcoin::secp256k1::Secp256k1::sign_schnorr` over a hand-rolled BIP-341 sighash. **Budget: 80 LOC** (function + helpers). Reuses `shared::bip322::bip322_message_hash` for the message input (single source of truth — PITFALLS V1.4-MOD-07). Symmetric with the extend-path module split locked in D-04. Phase 17 planner consumes this verbatim — no Phase 17 friction if Sprint-0-B fails.
- **D-16 (test discipline for the fallback):** If D-15 fires, the manual signing path needs MORE test coverage than the bdk path (no library validation backstop). Property test against BIP-322 `basic-test-vectors.json` for P2TR + cross-impl differential test deferred to v1.5 (TEST-EXT-01 in REQUIREMENTS.md). Phase 15's per-script-type property test (BIP322-04) is the minimum gate.

### Spike protocol + ADR shape

- **D-17 (sequencing):** Sprint-0-A and Sprint-0-B run **in parallel** on separate throwaway branches (`spike/14-A-bip322-cargo-tree`, `spike/14-B-bdk-p2tr-poc`). The two spikes are independent (A answers Decision #1 only via cargo tree; B answers Decision #4 only via PoC); given D-01's conservative EXTEND default for Decision #1, A's outcome doesn't change B's plan. Halves calendar cost.
- **D-18 (timebox discipline):** **2-day cap per spike, halt + escalate with no extension.** At the cap the spike author writes `sprint-0-X.md` with what's been learned and marks the verdict `INCONCLUSIVE`. The user makes the call in the ADR from partial data. Inconclusive Decision #1 → EXTEND (per D-03). Inconclusive Decision #4 → manual fallback (per D-15). Matches XP spike discipline; prevents spike scope creep into production-quality code.
- **D-19 (branch hygiene):** Spike branches are **NOT merged into `main`**. ROADMAP success criterion #5 carries unchanged. After Phase 14 closes, the branches are preserved as git refs (push to `origin` for reproducibility) but never enter `main`'s history. The ADR's `## Spike Outputs` section links to `.planning/research/sprint-0-A.md` and `.planning/research/sprint-0-B.md`, which embed the relevant captured output (cargo tree text, PoC verdict + 64-byte witness hex) — the branches themselves are the reproducibility artifact, not the canonical record.
- **D-20 (ADR file structure):** Single file `.planning/decisions/v1.4-adr.md` (per ROADMAP). Internal structure follows **Michael Nygard ADR template per decision**:
  ```markdown
  # v1.4 ADR — Multi-Script BIP-322 Decisions

  ## Decision #1 — bip322 crate adopt vs extend
  **Status:** ACCEPTED (with conditional flip on Sprint-0-A pass)
  **Context:** [research SUMMARY §"Open Decision #1" condensed]
  **Decision:** Default EXTEND. Adopt only if Sprint-0-A passes 3 gates [D-02].
  **Consequences:**
    - Positive: ...
    - Negative: ...
    - Neutral: ...
  **Rejected Alternatives:** Default ADOPT (FEATURES position); Decide-AFTER-spike.

  ## Decision #2 — mixed vs segregated rounds
  ...

  ## Decision #3 — wire format
  ...

  ## Decision #4 — bdk_wallet 2.3 P2TR sign path
  **Status:** PENDING (Sprint-0-B); flips to ACCEPTED (bdk) or ACCEPTED (manual fallback)
  ...

  ## Spike Outputs
  - sprint-0-A.md — bip322 0.0.10 cargo tree analysis, GO/NO-GO verdict
  - sprint-0-B.md — bdk_wallet 2.3 P2TR PoC, viable/fallback verdict
  ```
  Phase 15-18 planners read by anchor (`#decision-1`, `#decision-2`, `#decision-3`, `#decision-4`). Extensible if v1.5 adds Decision #5.
- **D-21 (cross-phase invariant gate at Phase 14 boundary):** Phase 14 produces ZERO production code commits. The v1.3 `full_round::*` cross-phase invariant gate is closed trivially: the closing commit body for Phase 14 (the ADR commit) records `# git diff main -- coordinator/ client/ shared/ liquidity-bot/` → empty. No `cargo test` invocation needed for Phase 14 specifically; the gate is structural, not behavioral.

### Claude's Discretion

- **CD-1:** Whether `sprint-0-A.md` and `sprint-0-B.md` embed the FULL command output (cargo tree verbatim, PoC code excerpt + witness hex) or just the verdict + a hash of the captured branch HEAD. Default: embed FULL output for self-containment — sprint files are the canonical record per D-19; branches are reproducibility, not the canonical record. Bias toward larger files; future readers should not need to check out the branch to understand the verdict.
- **CD-2:** Whether the ADR's "Consequences" sections enumerate cost/benefit per researcher position (STACK / FEATURES / ARCHITECTURE / PITFALLS) or a synthesized view. Default: synthesized, with one sentence per researcher position attributed inline (e.g., "PITFALLS warns this leaves us owning ~205 LOC forever"). Avoids the "consequences section is a 4-column matrix" trap.
- **CD-3:** Exact phrasing of the v1.4 README's "known limitation" note about the mixed-input chain-analysis fingerprint (D-08). Default: one paragraph in `README.md §"Privacy Considerations"`, plain language, no scary uppercase. Defer the exact wording to the planner who's already in the prose-writing mode for v1.4 docs (likely Phase 18).
- **CD-4:** Whether Sprint-0-B writes the PoC binary at `client/src/bin/spike-p2tr.rs` or `examples/spike-p2tr.rs`. Default: `examples/` — that's the Cargo convention for throwaway code that doesn't need to be in the release artifact; keeps the binary out of `cargo build --release` by default. Either works.
- **CD-5:** Whether the closeout commit also touches `.planning/STATE.md` and `.planning/ROADMAP.md` (mark Phase 14 complete) in the same commit as the ADR, or in a separate doc commit. Default: separate doc commit per `commit_docs: true` config and existing convention from v1.3 phase closures.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing Phase 14.**

### Prior planning (v1.4 milestone)

- `.planning/PROJECT.md` — Project mission, constraints (no custom crypto, MIT, Tor-native, signet-first), v1.3 ship state, v1.4 milestone goal, deferred items.
- `.planning/REQUIREMENTS.md` — Full v1.4 requirements (14 items: BIP322-01..04, ADVERT-01..04, WALLET-01..04, INTEG-01..02). Phase 14 maps zero requirements (gating ADR-producing phase). REQUIREMENTS.md Out-of-Scope table is load-bearing.
- `.planning/ROADMAP.md` §"Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions" — Phase goal, success criteria (5 numbered), cross-phase invariant.
- `.planning/STATE.md` §"Open Decisions for Phase 14 discuss-phase" — Original framing of the 4 Open Decisions (now resolved here).
- `.planning/research/SUMMARY.md` — Synthesized researcher output. Most relevant sections: §"Stack Recommendations" (no new deps unless adopt), §"Architecture Plan / Touchpoints", §"Pitfalls Watchlist" (V1.4-CRIT-01, CRIT-02, MOD-01..07), §"Open Decisions for Discuss-Phase" (the 4 decisions this phase resolves).

### v1.3 carry-forward (forensics + invariants)

- `.planning/milestones/v1.3-phases/13-client-src-wallet-rs-wire-format-fix-plan-12-02-unmute-cycle/13-CONTEXT.md` — REPAIR-01 closure context. Lesson #1 (wire-format roundtrip test ships FIRST) is enforced by D-13.
- `.planning/milestones/v1.3-phases/12-repair-client-src-wallet-rs-260-bdk-wallet-2-3-signoptions-r/12-CONTEXT.md` — `trust_witness_utxo: true` + real on-chain `witness_utxo` requirement; load-bearing for Sprint-0-B PoC construction (bdk's PSBT-sign needs both).
- `.planning/milestones/v1.3-phases/11-coordinator-rsa-pubkey-encoding-full-round-rs-unmute-complet/11-CONTEXT.md` — Escape-valve / D-11 / D-12 protocol referenced in v1.3 STATE.md "Accumulated Context" → "if 2-3 carry-forward plans appear with the same shape, abandon Plan.md and pivot to /gsd:debug".

### Code anchors (read-only references; Phase 14 modifies none)

- `coordinator/src/bitcoin/utxo.rs:119` — The `is_p2wpkh()` hard gate that Phase 16 replaces with the allowlist + dispatcher. Phase 14 references for context only; no edit.
- `shared/src/bip322.rs` (entire file, 133 LOC) — The custom BIP-322 implementation that EXTEND extends and ADOPT replaces. Module split locked in D-04 reorganizes this into `shared/src/bip322/{mod.rs, p2wpkh.rs, ...}` in Phase 15, not Phase 14.
- `shared/src/protocol.rs:13-28` — The `OwnershipProof` and `InfoResponse` types that Phase 15 extends (per D-11, D-12, D-13).
- `coordinator/src/discovery/pkarr_pub.rs:76` — The 220-byte PKARR payload warn threshold. The v1.4 PKARR record schema bump (`0.1.0` → `0.2.0`) lives in Phase 16, not Phase 14.
- `client/src/wallet.rs:276-291` — Phase 13's Witness wire-format fix locus. Phase 17 extends this for the 3-descriptor sign path (per D-15 fallback location if Sprint-0-B fails).

### External specs (referenced by the ADR; downstream Phase 15 work)

- BIP-322 specification — `https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki`. Pin the commit SHA in Phase 15's BIP322-04 property test.
- BIP-322 `basic-test-vectors.json` — pinned by commit SHA from `bitcoin/bips` repo in Phase 15.
- BIP-86 (single-key Taproot) — descriptor format for `tr(.../86'/...)` (Phase 17 WALLET-01).
- BIP-49 (P2SH-wrapped segwit) — descriptor format for `sh(wpkh(.../49'/...))` (Phase 17 WALLET-01).
- BIP-84 (native segwit) — descriptor format for `wpkh(.../84'/...)` (already shipped; v1.4 default).
- Wasabi PR #8912 (`AllowP2trInputs`) — precedent for the operator-tunable allowlist (D-07 / ADVERT-01).
- `jedisct1/rust-blind-rsa-signatures` README — pinned in PROJECT.md tech-stack section.

### Tools / commands relevant to Sprint-0

- `cargo tree -p bip322` (Sprint-0-A primary command).
- `cargo audit` (Sprint-0-A gate #2).
- `cargo run --example spike-p2tr` (Sprint-0-B primary command, CD-4 default location).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`shared/src/bip322.rs`** — `bip322_message_hash`, `build_bip322_to_spend`, `build_bip322_to_sign` are script-type-NEUTRAL primitives (lines 19-76 per research SUMMARY). These are the single source of truth (V1.4-MOD-07). All three Phase 15 verifiers (P2WPKH, P2TR, P2SH-P2WPKH) reuse them. Whether Decision #1 lands EXTEND or ADOPT, these primitives stay — the crate's `bip322::to_spend` / `to_sign` helpers would be wrapped by an adapter, not replace these.
- **`bitcoin 0.32.x` primitives** — `SighashCache::p2wpkh_signature_hash`, `SighashCache::taproot_key_spend_signature_hash`, `Script::is_p2wpkh / is_p2tr / is_p2sh`, `secp256k1::verify_schnorr`, `XOnlyPublicKey::from_slice`. Every primitive Phase 15 needs is already in the dep graph; v1.4 adds zero new mandatory crates under the EXTEND path.
- **`BitcoindGuard` + `require_bitcoind!()` macro** (v1.3 Phase 9) — Script-type-AGNOSTIC; Phase 18's mixed-script E2E test reuses these unchanged per INTEG-01 success criterion #3.
- **bdk_wallet 2.3 SignOptions { trust_witness_utxo: true }** + real on-chain `witness_utxo` values (v1.3 Phase 12 lesson) — Sprint-0-B's PoC MUST construct PSBTs with both, or it tests a strawman. This is load-bearing for the PoC's validity.
- **Existing CI grep gate for `corepc-node` feature pin** (v1.3 Phase 10 REPAIR-02) — Pattern reusable for `bdk_wallet = "=2.3.x"` exact pin and (if Sprint-0-A passes) `bip322 = "=0.0.10"` exact pin. Phase 15 adds the new grep targets, not Phase 14.

### Established Patterns

- **Shared crate is the contract** (v1.0 pattern) — Both coordinator and client compile against `shared/`. Phase 15 extends, never replaces, this pattern. D-04's module split is consistent.
- **Phase-Gated HTTP API** (v1.0) — New gate (script-type allowlist) layers in Phase 16 after the existing phase gate. No new gating shape; just one more allowlist check at validate-utxo time.
- **Per-round RSA keypair + memory-only round state** (v1.0) — The new `script_type` field on `RegisteredInput` (Phase 16) is `#[zeroize(skip)]` per memory-only invariant. Phase 14 doesn't touch this; locked here for Phase 16's reference.
- **Exact-pin all dependencies + CI grep gate** (v1.3 REPAIR-02) — Carries to v1.4. Both `bdk_wallet` exact pin (already enforced) and (conditional on Sprint-0-A) `bip322` exact pin.
- **Wire-format roundtrip test ships FIRST** (v1.3 REPAIR-01 lesson #1) — D-13 enforces this for Phase 15. Non-negotiable phase boundary.

### Integration Points

- **Phase 14 → Phase 15:** ADR Decisions #1 (D-01..D-05), #3 (D-11..D-13), #4 fallback shape (D-15..D-16) directly drive Phase 15 plan-phase tasks for BIP322-01..04 and ADVERT-04. The ADR is the input contract.
- **Phase 14 → Phase 16:** ADR Decision #2 (D-06..D-10) drives Phase 16 plan-phase tasks for ADVERT-01..03 and the load-bearing CRIT-01 cross-check.
- **Phase 14 → Phase 17:** ADR Decision #4 verdict (bdk path OR manual fallback per D-14..D-16) directly drives Phase 17 plan-phase tasks for WALLET-02 (BIP-322 signing for all 3 script types).
- **Phase 14 → Phase 18:** No direct dependency; Phase 18 consumes the wire shape locked in Phase 15 + the coordinator config locked in Phase 16.
- **Phase 14 closes no production code integration points** — by design (D-21).

</code_context>

<specifics>
## Specific Ideas

- **Spike branch naming convention:** `spike/14-A-bip322-cargo-tree`, `spike/14-B-bdk-p2tr-poc`. Numbered to allow future Phase 14B / 14C extensions (e.g., if v1.5 needs more spikes). The `spike/` prefix isolates them from `gsd/` (the gsd workflow phase-branch convention from `.planning/config.json`).
- **bdk_wallet 2.3 PoC concrete shape (Sprint-0-B):** A small binary at `examples/spike-p2tr.rs` (per CD-4) that: (1) generates a deterministic seed, (2) derives a `tr(.../86'/...)` BIP-86 descriptor via bdk's descriptor builder, (3) constructs the BIP-322 `to_spend` virtual transaction using `shared::bip322::build_bip322_to_spend`, (4) constructs the `to_sign` virtual transaction using `shared::bip322::build_bip322_to_sign`, (5) populates a PSBT with `trust_witness_utxo: true` SignOptions and real on-chain `witness_utxo` (load-bearing per v1.3 Phase 12), (6) asks bdk to sign, (7) extracts `psbt.inputs[0].tap_key_sig`, (8) verifies the resulting 64-byte Schnorr signature against the expected sighash using `secp256k1::verify_schnorr`. PASS = step 8 returns Ok. FAIL = any of steps 6-8 fails or produces wrong-sized witness.
- **cargo tree concrete shape (Sprint-0-A):** Two-line addition to a throwaway `shared/Cargo.toml`: `bip322 = "=0.0.10"`. Then `cargo tree -p bip322 -e normal --format "{p} {f}"` captures version pins; `cargo audit` is run on the throwaway lockfile. Captured verbatim into `sprint-0-A.md` (per CD-1).
- **The ADR's Decision #2 Consequences (negative) MUST surface the chain-analysis fingerprint candidly** — D-08's "known limitation" is real, not papered-over. Use language matching the v1.4 README disclaimer (CD-3).

</specifics>

<deferred>
## Deferred Ideas

- **bip322 0.0.10 → 1.0 reconsider trigger** — If `bip322` crate ships a 1.0 SemVer release before v1.5 starts, re-open Decision #1 in v1.5's milestone-research phase. (Marker for v1.5 STATE.md; not actionable in v1.4.)
- **TEST-EXT-01 cross-impl differential test** (`ACken2/bip322-js` reference vectors) — Already in REQUIREMENTS.md Future Requirements; the manual sign path under D-15 would benefit from this. v1.5 candidate.
- **TEST-EXT-02 regtest on-chain anchor test** (sign BIP-322 message + real spend with same key; bitcoind acceptance proves sighash math) — REQUIREMENTS.md Future Requirements; strongest correctness gate against V1.4-CRIT-02. v1.5 candidate.
- **TEST-EXT-03 automated backwards-compat integration matrix** — WALLET-04 covers v1.4→v1.3 informally in v1.4; full grid (v1.3↔v1.4, mixed-version rounds) is REQUIREMENTS.md Future Requirements. v1.5 candidate.
- **Mixed-output script types (Wasabi 2.0.3-style per-participant output choice)** — REQUIREMENTS.md Out-of-Scope. Separate output-policy milestone, v1.5+.
- **Per-script-type ban tracking** — Anti-feature (REQUIREMENTS.md); leaks correlation. NOT a v1.5 candidate either — keeping ban list uniform on `OutPoint` is the design invariant.
- **Per-script-type rate limits** — Anti-feature; defeats Tor-safe `GlobalKeyExtractor`. NOT a v1.5 candidate.
- **P2WSH multisig BIP-322 support** — REQUIREMENTS.md "Out of v1.4 Scope but Not Anti-Features"; stretch dropped for scope discipline. v1.5 candidate if demand materializes.
- **CARRY-TOR-UAT** (Tor-mode verification harness, v1.2 Phase 8 HUMAN-UAT item 3) — Confirmed deferred to v1.5+ per STATE.md.
- **CARRY-REPAIR-01-PR** (REPAIR-01 PR observation closure) — Confirmed v1.5 process step per v1.3 STATE.md; v1.4 cut PR is the natural moment but NOT a v1.4 code deliverable.
- **B-03** (dynamic fee estimation, mempool-aware polling + RBF) — Pre-mainnet requirement; v1.5+ scheduling.
- **DECISIONS-INDEX.md rolling summary** — `.planning/DECISIONS-INDEX.md` doesn't exist; per the discuss-phase workflow it's a bounded rolling summary that supersedes per-phase reads. Worth creating in v1.5 if the per-phase CONTEXT.md count grows. Not v1.4 scope.

</deferred>

---

*Phase: 14-Sprint-0 Spikes + Discuss-Phase Decisions*
*Context gathered: 2026-05-29*
