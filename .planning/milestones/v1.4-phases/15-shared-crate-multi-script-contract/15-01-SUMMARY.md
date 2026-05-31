---
phase: 15
plan: 01
subsystem: shared/protocol + shared/bip322
tags: [bip322, wire-format, ownership-proof, serde, roundtrip, v2-envelope]
requires: [v1.4-adr.md#decision-3, 14-CONTEXT.md#D-22..D-25, 14-CONTEXT.md#CD-7, 14-CONTEXT.md#CD-10]
provides:
  - "shared::bip322::ScriptType stub enum (3 variants: P2wpkh, P2tr, P2shP2wpkh) with snake_case + kebab-case serde wire form"
  - "shared::protocol::OwnershipProof v2 four-field flat envelope (version + witness_stack + psbt_input_b64 + script_type)"
  - "CD-7 two-phase try-parse encode/decode preserving bit-exact v1.3 wire compatibility"
  - "shared/tests/ownership_proof_roundtrip.rs — 5 D-13 cases + 1 sibling, 6 #[test] fns total"
affects:
  - shared/Cargo.toml (added base64 = "0.22" as the only new direct dep)
  - shared/src/bip322.rs (added pub ScriptType + 5 inline tests; primitives untouched)
  - shared/src/protocol.rs (replaced 2-field OwnershipProof with 4-field v2 envelope + 6 inline tests)
  - shared/src/token.rs (Rule 3 auto-fix: updated struct literal to 4-field shape)
  - client/src/round/input.rs (Rule 3 auto-fix: updated struct literal to 4-field shape)
tech-stack:
  added:
    - base64 = "0.22" (direct dep on shared; RESEARCH A8 fallback path)
  patterns:
    - "CD-7 two-phase try-parse: Vec<String> first (v1.3 array-of-hex), then Self (flat-struct) — RESEARCH Pattern 3"
    - "to_json_hex_str v1.3-compat branch: emits array-of-hex iff (version==1 && both Option fields none); flat-struct JSON otherwise"
    - "#[serde(default = ...)] + #[serde(skip_serializing_if = Option::is_none)] on wire-evolution fields — preserves T-01-04 no-deny-unknown-fields invariant"
    - "stub ScriptType in flat bip322.rs at this plan boundary; Plan 15-02 splits the file into the directory module per D-04"
key-files:
  created:
    - shared/tests/ownership_proof_roundtrip.rs
    - .planning/phases/15-shared-crate-multi-script-contract/15-01-SUMMARY.md
  modified:
    - shared/Cargo.toml
    - shared/src/bip322.rs
    - shared/src/protocol.rs
    - shared/src/token.rs
    - client/src/round/input.rs
decisions:
  - "Plan 15-01 ships as 3 atomic commits per CD-10 — Task 1 (stub enum + base64), Task 2 (OwnershipProof v2 + CD-7 try-parse), Task 3 (roundtrip test)"
  - "Stub ScriptType lives at shared::bip322::ScriptType (flat file), NOT shared::bip322::script_type sub-module — the directory split is Plan 15-02's responsibility per D-04"
  - "from_json_hex_str stays Result<Self, String> (untyped) per RESEARCH Pitfall 7 — typing it as Bip322Error would force protocol.rs to import from bip322 and create a module cycle"
  - "base64 = \"0.22\" added as direct dep per RESEARCH A8 — bitcoin's transitive base64 v0.21 cannot be relied upon as a stable engine; the workspace's bdk_wallet pin pulls 0.22 already"
metrics:
  duration: "~9 minutes"
  completed: 2026-05-30
  tasks: 3
  files_modified: 5
  files_created: 2
---

# Phase 15 Plan 01: ScriptType stub + OwnershipProof v2 envelope + wire-format roundtrip test Summary

One-liner: Lands the v2 four-field `OwnershipProof` envelope with the CD-7 two-phase try-parse and the 5 D-13 roundtrip cases as the first atomic commit of Phase 15 — wire-format gate ships BEFORE the bip322 module split per CD-10 / REPAIR-01 lesson #1.

## What changed

Plan 15-01 establishes the v1.4 multi-script wire-format contract in `shared/` without introducing any new BIP-322 crypto code. Three atomic commits ship in the order CD-10 mandates:

1. **`622ccf0` — Task 1: stub `ScriptType` enum + `base64 = "0.22"` direct dep.**
   - Adds `pub enum ScriptType { P2wpkh, P2tr, P2shP2wpkh }` to `shared/src/bip322.rs` with `#[serde(rename_all = "snake_case")]` on the enum and `#[serde(rename = "p2sh-p2wpkh")]` on the `P2shP2wpkh` variant — matches ADVERT-02's kebab-case wire form verbatim (RESEARCH Open Question #3 RESOLVED).
   - Derives `Debug + Clone + Copy + PartialEq + Eq + Serialize + Deserialize` so the v2 envelope's `script_type: Option<ScriptType>` field type-checks without any dispatcher logic.
   - 5 inline tests assert the wire form for all 3 variants + kebab-case deserialise + `Copy/Clone/Eq` derives.
   - `base64 = "0.22"` is the only new direct dep. `bip322`, `thiserror`, `proptest` are **deliberately NOT added** — they land in Plans 15-02 / 15-03 per CD-10's atomic-commit ordering.
   - The flat file `shared/src/bip322.rs` stays; the directory module split per D-04 is Plan 15-02's responsibility.

2. **`25a7dba` — Task 2: `OwnershipProof` v2 four-field flat envelope + CD-7 two-phase try-parse.**
   - Replaces the v1.3 two-field `OwnershipProof { witness_stack }` shape with the four-field v2 envelope per D-22 verbatim:
     - `pub version: u8` with `#[serde(default = "default_proof_version")]` returning `1` (D-25)
     - `pub witness_stack: Vec<Vec<u8>>` with `#[serde(default)]` (D-22)
     - `pub psbt_input_b64: Option<String>` with `#[serde(skip_serializing_if = "Option::is_none")]` (D-22)
     - `pub script_type: Option<crate::bip322::ScriptType>` with `#[serde(skip_serializing_if = "Option::is_none")]` (D-22, D-24)
   - **NO `#[serde(deny_unknown_fields)]`** anywhere — T-01-04 / D-06 forward-compat invariant locked at `shared/src/protocol.rs:3-5` is preserved.
   - `from_json_hex_str` signature stays `Result<Self, String>` per RESEARCH Pitfall 7 (typing it as `Bip322Error` would force `protocol.rs` to import from `shared::bip322` for the typed error and create a module cycle). Body now does the CD-7 two-phase try-parse:
     - Phase 1: `serde_json::from_str::<Vec<String>>` — v1.3 array-of-hex shape
     - Phase 2: `serde_json::from_str::<Self>` — flat-struct shape (covers both v1-explicit and v2 envelopes)
   - `to_json_hex_str` emits the v1.3 array-of-hex form when `version == 1 && psbt_input_b64.is_none() && script_type.is_none()` (CD-7 wire-compat branch). Otherwise emits the flat-struct JSON. This is the **load-bearing** invariant for the cross-phase gate.
   - 6 inline tests in `protocol::tests` cover the encode/decode matrix.

3. **`8a202bc` — Task 3: `shared/tests/ownership_proof_roundtrip.rs` (5 D-13 cases + 1 sibling).**
   - First file under `shared/tests/` (directory did not exist before this commit).
   - 6 `#[test]` fns total, all green, all importing ONLY `shared::bip322::ScriptType` + `shared::protocol::OwnershipProof` (no `bip322` crate, no `thiserror`, no `proptest`).
   - Defensive assertion in `v2_roundtrip_p2sh_p2wpkh`: the encoded JSON MUST contain `"p2sh-p2wpkh"` AND MUST NOT contain `"p2sh_p2wpkh"` — guards against a future regression that drops the explicit `#[serde(rename = ...)]` and silently falls back to `rename_all = "snake_case"`.

## D-13 case coverage

| # | Test fn | Wire shape | Asserts |
|---|---------|-----------|---------|
| 1 | `v2_roundtrip_p2wpkh` | `{"version":2, witness_stack:[], psbt_input_b64:"...", script_type:"p2wpkh"}` | encoded JSON contains `"version":2` + `"p2wpkh"`; roundtrip preserves all 4 fields |
| 2 | `v2_roundtrip_p2tr` | same shape, `"p2tr"` | encoded JSON contains `"p2tr"`; roundtrip preserves all 4 fields |
| 3 | `v2_roundtrip_p2sh_p2wpkh` | same shape, kebab-case `"p2sh-p2wpkh"` | encoded JSON contains `"p2sh-p2wpkh"` AND NOT `"p2sh_p2wpkh"`; roundtrip preserves all 4 fields |
| 4 | `v1_legacy_decode_array_of_hex` | `["3045022100abcd","02ab1234"]` (v1.3 wire) | decodes to `version=1`, 2-element witness_stack, both Option fields = None |
| 5 | `unknown_version_permissive_decode` | `{"version":3, witness_stack:[]}` | decodes Ok with `version=3` (verify-dispatch rejection layer lands in Plan 15-02) |
| sibling | `corrupted_base64_in_psbt_input_permissive_decode` | `{"version":2, psbt_input_b64:"not-base64-!!!", script_type:"p2wpkh"}` | decodes Ok; raw corrupt string surfaces in `psbt_input_b64` (downstream base64-decode failure surfaces as `Bip322Error::DecodeError` in Plan 15-02 / Phase 16) |

## OwnershipProof serde attribute placement (for Plan 15-02 cycle-avoidance reasoning)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnershipProof {
    #[serde(default = "default_proof_version")]
    pub version: u8,
    #[serde(default)]
    pub witness_stack: Vec<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psbt_input_b64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_type: Option<crate::bip322::ScriptType>,
}

fn default_proof_version() -> u8 { 1 }
```

**Module-cycle anchor:** `protocol.rs` imports `crate::bip322::ScriptType` via the fully-qualified path on the field type — no `use crate::bip322::ScriptType;` at the top of `protocol.rs`. Plan 15-02 can introduce `pub enum Bip322Error` in `shared/src/bip322/mod.rs` without breaking this because no error type from `bip322` ever appears in `protocol.rs` — the helper return type stays `Result<Self, String>` per Pitfall 7.

## Fixture choice for v2 roundtrip tests

`FIXTURE_PSBT_B64 = "cHNidP8BAAA="` — six raw bytes (`psbt\xff\x01\0\0`) encoded as base64. Looks structurally realistic (the `psbt\xff` magic prefix appears at the start) without claiming to be a valid PSBT byte stream. Plan 15-01 does NOT decode the payload; Plan 15-02 / Phase 16 own the base64+PSBT decode step and surface failures as `Bip322Error::DecodeError`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocker] Updated `client/src/round/input.rs:64` and `shared/src/token.rs:71` struct literals.**
- **Found during:** Task 2 (cargo build --workspace).
- **Issue:** Rust's struct literal syntax (`OwnershipProof { witness_stack }`) requires all fields. After the v2 evolution, three new fields (`version`, `psbt_input_b64`, `script_type`) are missing from both call sites; both fail to compile with `error[E0063]`.
- **Plan claim:** Task 2's `<action>` block stated "Do NOT touch coordinator/src/api/handlers.rs:136 or client/src/round/input.rs:64-65 — both call sites continue to compile because the helper signatures... AND the v1.3 wire encoding path is preserved". The helper signatures DO stay unchanged (`from_json_hex_str` returns `Result<Self, String>`; `to_json_hex_str` returns `String`) and the wire encoding IS preserved by the CD-7 branch — but the struct literal syntax is what breaks, not the helper API.
- **Fix:** Updated both call sites to the four-field struct literal with `version: 1, psbt_input_b64: None, script_type: None`. The v1.3 wire path is preserved bit-exactly because `to_json_hex_str`'s CD-7 branch emits the array-of-hex form when `(version == 1 && both Options None)` — verified by `cargo test --test integration full_round` exiting 0 with all 8 tests green.
- **Files modified:** `client/src/round/input.rs:63-71`, `shared/src/token.rs:71-77`
- **Commit:** `25a7dba` (folded into Task 2's atomic commit)

`coordinator/src/api/handlers.rs:136` was NOT touched — it calls `OwnershipProof::from_json_hex_str(&req.ownership_proof)` which is helper-only (no struct literal), and its return type is unchanged.

### Authentication Gates

None. Plan 15-01 is a pure-crate refactor; no auth surface.

## Cross-phase invariant verification

`cargo test --test integration full_round` — **8 passed, 0 failed** (47.22s with bitcoind v31 from brew). All v1.3 P2WPKH-only integration tests stay green at this plan boundary:

```
test full_round::coordinator_info_endpoint_fields ... ok
test full_round::adversarial_tampered_psbt_rejected ... ok
test full_round::adversarial_replay_token ... ok
test full_round::adversarial_invalid_utxo ... ok
test full_round::full_round_three_clients ... ok
test full_round::adversarial_wrong_denomination ... ok
test full_round::blame_non_signer_timeout ... ok
test full_round::round_restart_and_completion_after_blame ... ok
```

This proves the CD-7 wire-compat branch correctly preserves bit-exact v1.3 encoding — coordinator and client roundtrip the legacy array-of-hex wire shape over the live HTTP boundary without any change in observable behaviour.

## Verification

All success criteria PASS:

- [x] All 3 tasks executed per 15-01-PLAN.md
- [x] Each task committed individually with conventional commit messages (3 commits: `622ccf0`, `25a7dba`, `8a202bc`)
- [x] `cargo test -p shared --test ownership_proof_roundtrip` exits 0 with `6 passed, 0 failed`
- [x] `cargo test --test integration full_round` exits 0 with `8 passed, 0 failed` (cross-phase invariant)
- [x] `cargo test -p shared --lib` exits 0 (inline `protocol::tests` 6 passed + `bip322::tests` 8 passed = 14 unit tests, plus the existing tests carrying over)
- [x] `cargo build -p shared` exits 0
- [x] `cargo build --workspace` exits 0
- [x] `grep -E '^base64\s*=\s*"0\.22"' shared/Cargo.toml` succeeds
- [x] `grep -E '^bip322' shared/Cargo.toml` exits non-zero (the bip322 dep is NOT yet added in 15-01)
- [x] `grep -E '^thiserror' shared/Cargo.toml` exits non-zero
- [x] `grep -E '^proptest' shared/Cargo.toml` exits non-zero (proptest stays a workspace pin; Plan 15-03 will add as a dev-dep via workspace inheritance)
- [x] `grep -c '^#\[test\]' shared/tests/ownership_proof_roundtrip.rs` outputs exactly `6`
- [x] STATE.md / ROADMAP.md / REQUIREMENTS.md updates folded into the final metadata commit (per workflow)

## Known Stubs

`shared::bip322::ScriptType` ships as a **stub enum** in this plan — three variants (`P2wpkh`, `P2tr`, `P2shP2wpkh`) with serde derives, no dispatcher logic, no `verify_simple`/`sign_simple` API surface. This is intentional and documented in the inline doc-comment at `shared/src/bip322.rs`. Plan 15-02 replaces it with the full dispatcher-backed enum + per-script `pub(crate)` modules + the 26-LOC bip322-crate adapter per D-04 / D-27 / Sprint-0-A. Nothing in this plan's deliverable depends on the dispatcher; the stub is sufficient for the wire-format roundtrip contract.

## Self-Check: PASSED

- `shared/Cargo.toml` — `base64 = "0.22"` line present (verified via grep above)
- `shared/src/bip322.rs` — `pub enum ScriptType` + `#[serde(rename = "p2sh-p2wpkh")]` lines present
- `shared/src/protocol.rs` — `pub struct OwnershipProof` with all four fields + `default_proof_version` helper
- `shared/tests/ownership_proof_roundtrip.rs` — exists, 6 `#[test]` fns, imports `shared::bip322::ScriptType` + `shared::protocol::OwnershipProof`
- `client/src/round/input.rs` — `OwnershipProof { version: 1, witness_stack, psbt_input_b64: None, script_type: None }`
- `shared/src/token.rs` — same four-field literal in the `ownership_proof_roundtrip` inline test
- Git history `git log --oneline -4` shows the three feature commits in CD-10 order: stub enum → struct evolution → roundtrip test
- Commit hashes verified via `git log --oneline --all | grep <hash>` for all three: `622ccf0`, `25a7dba`, `8a202bc` — all present

---

*Plan: 15-01*
*Phase: 15 — Shared Crate Multi-Script Contract*
*Completed: 2026-05-30*
*Next plan: 15-02 (bip322 module split + dispatcher + crate adapter + Bip322Error)*
