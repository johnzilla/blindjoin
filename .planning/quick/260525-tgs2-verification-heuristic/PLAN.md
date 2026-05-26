---
quick_id: 260525-tgs2
slug: verification-heuristic
workstream: fix-verification-gap
created: 2026-05-26
status: in-progress
---

# D-3: Verification-template heuristic

## Why
Deliverable #3 of fix-verification-gap. The v1.1 verifier saw a test-only helper (`build_input_reg_round_state`) constructing production state and accepted it as evidence. The heuristic now landing in `gsd-verifier`: when test setup constructs a production type, cite the production analog (function + file:line) or mark the must-have FAILED. "Test-only, acceptable" is no longer valid.

## What changed (tooling, outside this repo)
Edited `~/.claude/agents/gsd-verifier.md` (917 → 956 lines, +39):
1. Added a critical rule in `<critical_rules>` block.
2. Added Step 7d "Test Setup Audit" between Step 7c (probe execution) and Step 8 (human verification needs).
3. Added one item to the success criteria checklist.

`~/.claude/` is not git-managed — the edits live in the file as-is.

## What lands in this repo
- `.planning/workstreams/fix-verification-gap/VERIFICATION-HEURISTIC.md` — project's record of the tooling change with full content of the additions for traceability.

## Validation
- Branch `verification-heuristic-doc`, doc-only PR.
- CI should pass trivially (no source changes).
- Future verifications of blindjoin phases will exercise the new Step 7d.
