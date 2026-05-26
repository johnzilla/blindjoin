---
quick_id: 260525-tgs
slug: audit-backdoor-inventory
workstream: fix-verification-gap
created: 2026-05-26
status: in-progress
---

# Audit: backdoor inventory across integration tests

## Why
Deliverable #2 of `fix-verification-gap`. The v1.1 round-bootstrap regression was masked by `build_input_reg_round_state` — a test-only helper that constructed production state with no production analog. v1.1's verifier accepted it because the verification template never required citing a production code path for test setup. Workstream A deleted that one helper. This task inventories what else might be hiding.

## Approach
1. **Investigation** (read-only, delegated to Explore agent in main context).
2. **Synthesis** — orchestrator writes `.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md` from Explore's findings. (Necessary because gsd-executor agents are blocked from writing under `.planning/`.)
3. **TODO comments** — for any HIGH/MEDIUM findings, add `// TODO(fix-verification-gap): see .planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md` at the call site.
4. **Follow-on work** — if any HIGH finding warrants its own remediation workstream, list it in the inventory and the orchestrator will create the workstream.

## Classification rubric
- **HIGH**: production literally cannot reach the state the helper constructs (same class as round-bootstrap bug).
- **MEDIUM**: wraps production code but bypasses a step production always performs.
- **LOW**: fixture that wraps production builders — safe.

## Deliverables
- `.planning/workstreams/fix-verification-gap/BACKDOOR-INVENTORY.md`
- Any TODO comments in test/src files
- Possibly: new workstream(s) for HIGH findings

## Validation
- Branch `audit-backdoor-inventory`, PR, green CI (doc-heavy, low risk).
- Do NOT merge — return PR URL.

## Constraints
- Bar for HIGH is "production literally cannot reach this state."
- Don't touch `.planning/` git history.
- If the audit finds ZERO additional HIGH backdoors, that's a valid result — say so.
