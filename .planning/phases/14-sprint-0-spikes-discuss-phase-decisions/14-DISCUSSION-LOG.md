# Phase 14: Sprint-0 Spikes + Discuss-Phase Decisions - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-29
**Phase:** 14-Sprint-0 Spikes + Discuss-Phase Decisions
**Areas discussed:** Decision #1 (bip322 crate adopt vs extend), Decision #2 (mixed vs segregated rounds), Decision #3 (P2SH-P2WPKH wire format), Spike protocol + ADR shape

---

## Decision #1 — bip322 crate adopt vs extend

### Q1.1: Posture going into Sprint-0-A

| Option | Description | Selected |
|--------|-------------|----------|
| Default EXTEND, adopt only if all gates pass | Conservative posture per PITFALLS researcher and v1.3 REPAIR-01 forensics. ADR encodes adopt criteria as explicit gates over the cargo tree output. Misses default → extend (~205 LOC ownership). | ✓ |
| Default ADOPT, extend only on hard fail | Lean on the crate's existence (FEATURES position). Faster Phase 15 if it works, inherits 0.0.x SemVer + 9-month maintenance stall. | |
| Decide AFTER Sprint-0-A only | No upfront posture. Cleanest evidence-based call, risk of late ambiguity surprise. | |

**User's choice:** Default EXTEND, adopt only if all gates pass.
**Notes:** Pre-question, user asked for an explanation of "spike" methodology and "branch hygiene"; the workflow paused for plain-language framing (XP-style timeboxed experiment; throwaway code; no merge to main) before re-posing this question.

### Q1.2: Explicit GO-criteria for Sprint-0-A to flip to ADOPT

| Option | Description | Selected |
|--------|-------------|----------|
| Three gates | (1) `cargo tree -p bip322` shows bitcoin = 0.32.x, (2) cargo audit clean, (3) adapter wrapping crate's verify_simple is <50 LOC with zero lossy conversions. | ✓ |
| Five gates | Three above PLUS BIP-322 spec contract test + CI grep gate (only `bip322::verify_simple` allowed). ~+0.5 day of Sprint-0-A work. | |
| Two gates | Just bitcoin pin + cargo audit. Defers adapter-LOC surprise into Phase 15. | |

**User's choice:** Three gates.
**Notes:** Combined with Q1.1's conservative posture, the binary GO/NO-GO is strict but achievable.

### Q1.3: Policy on AMBIGUOUS Sprint-0-A outcome

| Option | Description | Selected |
|--------|-------------|----------|
| Ambiguous = EXTEND, no escalation | Conservative posture extends to ambiguity. Binary GO/NO-GO by design. Removes mid-spike user interrupt need. | ✓ |
| Ambiguous = escalate to user mid-spike | Spike halts at borderline gate, writes checkpoint, user calls inline. Breaks timebox, requires Phase 14 mid-stream touch. | |
| Ambiguous = ADOPT with documented risk | If bitcoin pin + cargo audit clean, borderline adapter is acceptable; ADR records risk. Maximizes adopt chance but accepts Phase 17 surprise. | |

**User's choice:** Ambiguous = EXTEND, no escalation.

### Q1.4: Code organization for EXTEND (the default outcome)

| Option | Description | Selected |
|--------|-------------|----------|
| Module split per script type | `shared/src/bip322/{mod.rs, p2wpkh.rs, p2tr.rs, p2sh_p2wpkh.rs}`. Each per-type file isolated. Easy v1.5 swap. Phase 15 cost: +0.25 day. | ✓ |
| Keep single file, extend in place | shared/src/bip322.rs grows to ~330 LOC. Single narrative; current convention. Harder to swap independently later. | |
| You decide (Claude discretion) | Default to module split; planner can downgrade if Phase 15 review shows split is over-engineered. Noted as CD item. | |

**User's choice:** Module split per script type.

---

## Decision #2 — mixed vs segregated script-type rounds

### Q2.1: Core call — mixed or segregated?

| Option | Description | Selected |
|--------|-------------|----------|
| MIXED rounds | One round accepts P2WPKH + P2TR + P2SH-P2WPKH together. Wasabi 2.0.3 precedent. Coordinator round-state machine unchanged. Outputs single-type per round. Tradeoff: heterogeneous-input chain-analysis fingerprint. | ✓ |
| SEGREGATED rounds | Separate round queue per script type. Up to 3 parallel round-state machines. Preserves v1.0 "don't mix" invariant. Tradeoff: anon set fragments at small signet participant counts; liquidity bot must backstop 3 queues. | |
| MIXED with per-script minimum (hybrid) | Mixed queue, round fires only if min-N of EACH allowed type. Avoids "lone P2TR" worst case. Adds matchmaking complexity. Not researcher-recommended. | |

**User's choice:** MIXED rounds.
**Notes:** Researchers split (FEATURES → mixed, PITFALLS → segregated). User's call breaks the tie toward broader participation; chain-analysis fingerprint accepted as a known limitation (see Q2.3).

### Q2.2: Output script type for mixed-input rounds

| Option | Description | Selected |
|--------|-------------|----------|
| Operator-configured per-coordinator | coordinator.toml carries `[bip] output_script_type = "p2wpkh"`. Coordinator-wide setting; advertised via PKARR. Participants pick a different coordinator if they want a different output type. | ✓ |
| Match dominant input type per round | Adaptive but adds round-state complexity; clients can't predict output type at registration. | |
| Per-round rotation | Coordinator rotates output type across rounds. Adds rotation-state complexity. | |

**User's choice:** Operator-configured per-coordinator (default p2wpkh).

### Q2.3: Per-script-type minimum participants gate?

| Option | Description | Selected |
|--------|-------------|----------|
| No per-type minimum | Keep v1.0 round minimum (total count only). Document heterogeneous-input fingerprint as known limitation. Liquidity bot per-round type rotation softens worst case. | ✓ |
| Per-type minimum = 2 (when allowed) | Hardens against lone-rare-type fingerprint. Tradeoff: signet's small participant counts make this expensive; liquidity bot would need to backstop both seats. | |
| Operator-configurable per-type minimum | `[bip] min_per_type = {...}`. Maximizes flexibility, adds another config knob, most operators won't tune. | |

**User's choice:** No per-type minimum.

---

## Decision #3 — P2SH-P2WPKH wire format

### Q3.1: Wire shape — B1, B2, or B3?

| Option | Description | Selected |
|--------|-------------|----------|
| B2: base64 PSBT-input shape | `psbt_input_b64: String` holding base64(bitcoin::psbt::Input). PSBT-everywhere aligned. ARCHITECTURE recommendation. Tradeoff: ~+100 bytes per proof. | ✓ |
| B1: tagged enum with version byte | `enum OwnershipProofData { WitnessOnly v1, WitnessAndScriptSig v2 }`. Minimal byte overhead. Two cases to deserialize; bespoke. | |
| B3: additive optional field (Recommended in question text) | Add `#[serde(default, skip_serializing_if = "Option::is_none")] script_sig: Option<...>` to existing OwnershipProof. Zero bytes on wire for P2WPKH/P2TR; wire-minimal. | |

**User's choice:** B2 — base64 PSBT-input shape (Architecture's pick).
**Notes:** User accepted the byte overhead in exchange for PSBT-everywhere alignment and future-PSBT-field extensibility, against the wire-minimal B3 alternative surfaced in the question.

### Q3.2: Backwards-compat deserialization strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit `version: u8` field | OwnershipProof gains `version` (default 1; v1.4 sets 2). Coordinator branches on version. Clear, statically-typed dispatch, easy v1.5 extension. | ✓ |
| Untagged serde enum | `#[serde(untagged)] enum { V1, V14 }`. Silent failure if malformed v1.4 coincidentally parses as v1.3. Double-parse cost. | |
| Optional fields with serde defaults | All v1.4 fields are `Option<T>` with `#[serde(default)]`. Muddier mix of additive-field pattern with B2's PSBT choice. | |

**User's choice:** Explicit version field.

### Q3.3: Roundtrip test discipline

| Option | Description | Selected |
|--------|-------------|----------|
| Cross-version + reject malformed | (1) v=2 self-roundtrip all 3 types, (2) v=1 backwards-compat, (3) v=2 mismatched script_type vs PSBT contents rejects, (4) version=3 rejects, (5) corrupted base64 rejects. ~10-15 cases. Phase 15 +0.5 day. | ✓ |
| Self-roundtrip + cross-version only | Skip malformed-input rejection (push to Phase 18 integration). ~6-8 cases. Minimum for REPAIR-01 lesson #1. | |
| Self-roundtrip only | v=2 self-roundtrip all 3 types. ~3 cases. Risks v=1 acceptance regressing silently. | |

**User's choice:** Cross-version + reject malformed.

---

## Spike protocol + ADR shape

### Q4.1: Sprint-0-A and Sprint-0-B sequencing

| Option | Description | Selected |
|--------|-------------|----------|
| Parallel on separate branches | spike/14-A-bip322-cargo-tree and spike/14-B-bdk-p2tr-poc run concurrently. Halves calendar. ADR ratification waits for both. | ✓ |
| Serial — A first, then B | Decision #1 resolved before B starts. Doubles calendar; user's conservative EXTEND default means A's outcome unlikely to change B's plan. | |
| Serial — B first, then A | Harder spike first; same calendar cost as A-then-B; A doesn't depend on B. | |

**User's choice:** Parallel on separate branches.

### Q4.2: Timebox escalation policy

| Option | Description | Selected |
|--------|-------------|----------|
| Halt + escalate, no extension | At 2-day cap, write sprint-0-X.md with `INCONCLUSIVE`; user decides in ADR. Matches XP discipline. Inconclusive #1 → EXTEND (per Q1.3); inconclusive #4 → manual fallback (per Q4.4). | ✓ |
| +50% extension rule (3 days max) | Standard XP +50% extension if "one or two insights away." Tradeoff: tends to slide toward +100%. | |
| Executor judgment, no hard cap | Max data quality, defeats timebox discipline. | |

**User's choice:** Halt + escalate, no extension.

### Q4.3: ADR file structure

| Option | Description | Selected |
|--------|-------------|----------|
| Nygard template per decision | One section per decision: Context / Decision / Status / Consequences (positive, negative, neutral) / Rejected Alternatives. Top-level Spike Outputs section. Disciplined, extensible. | ✓ |
| Tabular summary + per-decision narrative | Top table + paragraph rationale. Faster scan, weaker consequences-tracking. | |
| Conversational decision log | Narrative format. Lowest ceremony. Hardest for agents to parse by section. | |

**User's choice:** Nygard template per decision.

### Q4.4: Decision #4 (bdk_wallet 2.3 P2TR) fallback pre-spec

| Option | Description | Selected |
|--------|-------------|----------|
| Pre-spec'd location + bounded LOC | ADR: "if Sprint-0-B fails, manual lives in shared/src/bip322/p2tr.rs as sign_p2tr_keypath() using secp256k1::sign_schnorr over hand-rolled BIP-341 sighash. Budget 80 LOC." Symmetric with Decision #1 extend organization. | ✓ |
| Pre-spec'd shape, no LOC budget | Location + approach locked; budget left open for Phase 17 planner. More flexible, slightly more surprise. | |
| Defer fallback shape to Phase 17 plan-phase | ADR records verdict only; Phase 17 plan-phase decides shape. Lowest pre-commitment, highest Phase 17 friction. | |

**User's choice:** Pre-spec'd fallback location + bounded LOC.

---

## Claude's Discretion

Captured in CONTEXT.md `<decisions>` § "Claude's Discretion":

- **CD-1:** Embed FULL command output in sprint-0-X.md vs hash-and-link to branch.
- **CD-2:** ADR Consequences sections — synthesized view with inline researcher attribution.
- **CD-3:** Exact phrasing of v1.4 README "Privacy Considerations" disclaimer (D-08 known limitation).
- **CD-4:** PoC binary location — `examples/spike-p2tr.rs` per Cargo convention.
- **CD-5:** Closeout commit pattern — separate doc commit for ROADMAP.md / STATE.md status update, per existing v1.3 phase-closure convention.

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section:

- bip322 0.0.10 → 1.0 reconsider trigger (v1.5 STATE.md marker).
- TEST-EXT-01 cross-impl differential (v1.5 candidate; relevant if D-15 manual fallback fires).
- TEST-EXT-02 regtest on-chain anchor test (v1.5; strongest CRIT-02 mitigation).
- TEST-EXT-03 automated backwards-compat integration matrix (v1.5).
- Mixed-output script types (REQUIREMENTS.md Out-of-Scope; separate milestone).
- Per-script-type ban tracking, per-script-type rate limits (anti-features; NOT v1.5).
- P2WSH multisig BIP-322 support (v1.5 if demand materializes).
- CARRY-TOR-UAT, CARRY-REPAIR-01-PR, B-03 (v1.5+).
- `.planning/DECISIONS-INDEX.md` rolling summary (v1.5 if per-phase CONTEXT.md count grows).
