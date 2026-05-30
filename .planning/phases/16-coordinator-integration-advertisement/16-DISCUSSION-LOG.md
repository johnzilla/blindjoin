# Phase 16: Coordinator Integration & Advertisement - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 16-CONTEXT.md — this log preserves the auto-mode decision trail.

**Date:** 2026-05-30
**Phase:** 16-Coordinator Integration & Advertisement
**Mode:** --auto (autonomous; recommended defaults selected without AskUserQuestion)
**Areas discussed:** BipConfig struct shape + location, PKARR + /round/info advertisement encoding, validate_utxo dispatcher integration + CRIT-01, Plan ordering + test strategy

---

## A. BipConfig struct shape + location

### Q1: Where does the `[bip]` config section live?

| Option | Description | Selected |
|--------|-------------|----------|
| Top-level [bip] section | Mirrors REQUIREMENTS env-var prefix `BLINDJOIN__COORDINATOR__BIP__*` and existing top-level [network] / [coordinator] / [discovery] shape | ✓ (recommended) |
| Nested under [coordinator] | Cleaner grouping but requires env-var rename to `BLINDJOIN__COORDINATOR__COORDINATOR__BIP__*` (double `coordinator__`) | |

### Q2: Does `BipConfig::validate()` reject all-false?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — at least one allow_* must be true | Coordinator with zero allowed script types is non-functional; fail-fast at boot | ✓ (recommended) |
| No — allow all-false (rejects every input) | Lets operator effectively shut down inputs without changing binary; valid edge case | |

### Q3: Should `output_script_type` live in `[bip]`?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — alongside allow_* | Same config domain (per D-07) | ✓ (recommended) |
| No — under [coordinator] | Separate concern (operator policy vs ADVERT advertisement) | |

---

## B. PKARR + /round/info advertisement encoding

### Q1: PKARR field name verbose or compact?

| Option | Description | Selected |
|--------|-------------|----------|
| Compact `sst` (3-char) | REQUIRED — verbose name pushes payload to ~226 bytes, breaches 220-byte warn at pkarr_pub.rs:76; REQUIREMENTS says "stay under" | ✓ (recommended) |
| Verbose `supported_script_types` | Self-documenting but breaches byte budget | |

### Q2: CSV ordering?

| Option | Description | Selected |
|--------|-------------|----------|
| Alphabetical, canonical | Deterministic for record-equality tests + makes byte length deterministic | ✓ (recommended) |
| Insertion-order from BipConfig | Preserves operator intent but non-deterministic | |

### Q3: PKARR includes output_script_type?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — `"ost": "p2wpkh"` | Client uses for coordinator selection per D-07 | ✓ (recommended) |
| No — keep PKARR record minimal | Saves ~15 bytes but forces client to fetch /round/info to learn output type | |

---

## C. validate_utxo dispatcher integration + CRIT-01

### Q1: Version branch location?

| Option | Description | Selected |
|--------|-------------|----------|
| Inline in validate_utxo via `match proof.version` | One function, one decision point; D-12 verbatim | ✓ (recommended) |
| Extracted into a thin pre-dispatcher fn | More structure; adds an indirection layer | |

### Q2: v2 proof with `script_type: None` — error or proceed?

| Option | Description | Selected |
|--------|-------------|----------|
| WireFormatMismatch — v2 envelope MUST declare | v2's purpose IS multi-script; missing declaration is wire-shape violation; cleanest contract | ✓ (recommended) |
| Proceed with derived-only (skip cross-check) | More permissive; weakens CRIT-01 intent | |

### Q3: Structured log line shape?

| Option | Description | Selected |
|--------|-------------|----------|
| `tracing::info!(round_id=%, script_type=?, "ownership proof verified")` | ScriptType Debug-format; round_id at start; matches ROADMAP success criterion #1 phrasing | ✓ (recommended) |
| String-interpolated info!("script_type={} verified for round {}") | More direct text but less grep-able structured-field form | |

---

## D. Plan ordering + test strategy

### Q1: Plan ordering?

| Option | Description | Selected |
|--------|-------------|----------|
| 16-01 = BipConfig + InfoResponse; 16-02 = validate_utxo dispatcher swap + CRIT-01; 16-03 = PKARR schema bump | Wire/config first (REPAIR-01 lesson #1), then behavior, then discovery | ✓ (recommended) |
| Dispatcher-first, then config + advertisement | Behavior change ships first but wire shape evolves after — violates REPAIR-01 lesson #1 | |

### Q2: Integration test strategy?

| Option | Description | Selected |
|--------|-------------|----------|
| New tests/integration/multi_script_validate.rs | full_round.rs stays as v1.3 invariant gate; new file isolates v1.4 multi-script behavior; 9 named cases | ✓ (recommended) |
| Extend tests/integration/full_round.rs | Single file; risk of mixing v1.3 invariant gate with v1.4 behavior tests | |

### Q3: PKARR byte-budget assertion?

| Option | Description | Selected |
|--------|-------------|----------|
| Inline test in pkarr_pub.rs::tests | Co-located with the byte-warn threshold it asserts | ✓ (recommended) |
| Separate tests/integration/pkarr_byte_budget.rs | Adds a file for one assertion | |

---

## Claude's Discretion

User opt-in defaults (CD-11 through CD-16):
- **CD-11:** `BipConfig::supported()` returns alphabetical order.
- **CD-12:** Log line emits at INFO always (small operator-log delta acceptable vs v1.3 silence).
- **CD-13:** env-var override accepts wire-form lowercase kebab-case strings.
- **CD-14:** v1 proof path always passes `network: bitcoin::Network` to verify_simple.
- **CD-15:** verify_bip322_simple + is_p2wpkh() deletion lands inside 16-02 atomic commit (the dispatcher swap commit).
- **CD-16:** multi_script_validate.rs uses real BitcoindGuard (not mock) — mocking would bypass the very layer CRIT-01 is enforced at.

## Deferred Ideas

- Per-round-per-script-type registration breakdown advertisement → REQUIREMENTS anti-feature (not v1.5 either)
- Per-script-type ban/rate/denomination policies → anti-features
- Mixed output script types per round → v1.5+ separate output-policy milestone
- PKARR resolver-side `#[serde(default)]` shim → Phase 17 WALLET-03/04
- Tor-mode UAT harness, REPAIR-01 PR observation closure, B-03 fee estimation → v1.5+
- TEST-EXT-01/02/03 cross-impl differential, on-chain anchor, automated backwards-compat → v1.5+
- DECISIONS-INDEX.md rolling summary → v1.5
- CSV-vs-array PKARR record format reconsideration → v1.5 when script-type count grows
- `bdk_wallet = "=2.3.x"` exact-pin tightening (Phase 15 RESEARCH A7) → v1.5+
