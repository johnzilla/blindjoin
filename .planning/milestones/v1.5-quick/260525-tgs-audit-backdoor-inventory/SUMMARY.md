---
quick_id: 260525-tgs
slug: audit-backdoor-inventory
workstream: fix-verification-gap
created: 2026-05-26
completed: 2026-05-26
status: complete
pr_url: https://github.com/johnzilla/blindjoin/pull/4
branch: audit-backdoor-inventory
base_commit: edee4ce
---

# Summary

Deliverable #2 of fix-verification-gap complete. Audit of test-only helpers across the blindjoin codebase that construct production state types.

## Headline
**0 HIGH, 2 MEDIUM, 7 LOW.** The original `build_input_reg_round_state` backdoor was the only HIGH-class case; already removed in `d342359`. No additional production-unreachable state was found in the codebase.

## Commits
- `f61d8ab` — `docs(audit): backdoor inventory report for fix-verification-gap` — new BACKDOOR-INVENTORY.md
- `a2b35a3` — `chore(tests): mark MEDIUM-risk test helpers with verification-gap TODOs` — 5 lines added across 2 test helpers

## PR
[apps#4 — audit: backdoor inventory for fix-verification-gap](https://github.com/johnzilla/blindjoin/pull/4)

## MEDIUM findings (both TODO-flagged)
1. **`make_input_reg_state`** ([coordinator/src/round/input_reg.rs:111](coordinator/src/round/input_reg.rs:111)) — struct-literal `RoundStateInner` construction. Mitigated by real RSA generation and state-machine transition. Bounded risk.
2. **`make_signing_state`** ([coordinator/src/round/signing.rs:279](coordinator/src/round/signing.rs:279)) — direct `state.phase = Phase::Signing` mutation + placeholder RSA key. The worse case. Used by 4 sign-handler tests.

## Process notes
- Investigation delegated to Explore agent (read-only, ~150 files searched).
- Synthesis + writes done by orchestrator (gsd-executor cannot write under `.planning/`).
- Verification: spot-read of cited file:line ranges before writing the inventory; `cargo check --all-targets` clean before commit.

## Follow-on work surfaced
- Suggested backlog item (NOT a separate workstream): migrate `make_input_reg_state` and `make_signing_state` to use production state-machine transitions. ~30 min refactor. Fold into a future test-hygiene pass.
- Verification-template heuristic (Deliverable #3) — the inventory's "Patterns to avoid" section and the "cite production analog or NOT VERIFIED" rule directly feed that work.

## Next
Merge [PR #4](https://github.com/johnzilla/blindjoin/pull/4) when CI passes and reviewer approves.
