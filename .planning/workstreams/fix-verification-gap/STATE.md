---
workstream: fix-verification-gap
created: 2026-05-25
status: completed
resolved: 2026-05-29
---

# Project State

## Current Position
**Status:** Completed (2026-05-29)
**Last Activity:** 2026-05-29 -- 2 MEDIUM backdoors fixed, 0 HIGH backdoors remain

## Resolution

Workstream produced two deliverables, both shipped:

1. **VERIFICATION-HEURISTIC.md** — a checklist for future phase verifications
   (committed at the time of the workstream).

2. **BACKDOOR-INVENTORY.md** — audit of test-only helpers that bypass
   production state-construction. Classified 9 candidates: 0 HIGH, 2 MEDIUM,
   7 LOW. The 2 MEDIUM cases (`make_input_reg_state` and `make_signing_state`)
   were the action items.

The 2 MEDIUM backdoors are fixed in commit `6f8c7e5`:
- `make_input_reg_state` now calls `crate::round::manager::start_round()`
  and extracts the signer from the start_round-produced inner state.
- `make_signing_state` now walks Idle → InputReg → OutputReg → Signing
  via `transition_to()` calls (no direct `state.phase =` assignment).
  Uses a real RSA key (no 1-byte placeholder).

The 7 LOW cases were classified acceptable in the original audit and need
no action — they are fixtures that wrap production builders.
