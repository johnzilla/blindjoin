---
phase: 07-coordinator-dos-hardening
plan: "03"
status: complete
started: 2026-04-10
completed: 2026-04-10
gap_closure: true
tasks_completed: 1
tasks_total: 1
key_files:
  created: []
  modified:
    - coordinator/src/round/input_reg.rs
    - coordinator/src/api/handlers.rs
commits:
  - hash: 6d2e84d
    message: "fix(07-03): eliminate per-request RSA key deserialization in post_input (AVAIL-02)"
deviations: []
decisions: []
---

# Plan 07-03 Summary: AVAIL-02 Gap Closure

## One-liner

Removed per-request `from_der_secret_key` from `post_input` — `register_input` now accesses `inner.rsa_signer` directly.

## What changed

- `register_input()` no longer takes a `signer: &RsaBlindSigner` parameter
- It accesses `inner.rsa_signer.blind_sign(...)` directly through the already-held `&mut RoundStateInner`
- `post_input` handler no longer calls `rsa_signing_key.clone()` or `from_der_secret_key()`
- Two test call sites updated to match new signature

## Gap closed

The AVAIL-02 gap identified in 07-VERIFICATION.md (handlers.rs lines 207-213) is eliminated. RSA key deserialization no longer appears in the per-request hot path for any handler.

## Self-Check: PASSED

- `grep "from_der_secret_key" handlers.rs` → no matches
- `grep "rsa_signing_key.clone" handlers.rs` → no matches  
- `grep "inner.rsa_signer.blind_sign" input_reg.rs` → 1 match (line 64)
- `cargo check --package coordinator` → success (9 pre-existing warnings only)
