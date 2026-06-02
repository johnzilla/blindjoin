---
workstream: fix-verification-gap
deliverable: 3-of-3
applied_date: 2026-05-26
applies_to: ~/.claude/agents/gsd-verifier.md (shared GSD tooling, not project-scoped)
---

# Verification-template heuristic

## Problem this solves
v1.1's verification phase saw the test-only `build_input_reg_round_state` helper constructing production state and marked the related must-have as "Acceptable." The verifier never asked "where does production simulate round initialization?" — the answer being "nowhere," which is the exact bug that shipped.

See [BACKDOOR-INVENTORY.md](BACKDOOR-INVENTORY.md) for the audit that confirmed this was the only HIGH-class case in the codebase, and [`../fix-round-bootstrap/root-cause-report.md`](../fix-round-bootstrap/root-cause-report.md) for the original bug analysis.

## What changed in shared tooling
Edited `~/.claude/agents/gsd-verifier.md` (917 → 956 lines, +39 net). Three additions:

### 1. New critical rule
Added to the `<critical_rules>` block:

> **DO NOT accept test-only setup as evidence of production capability.** If a test constructs production state via a `#[cfg(test)]` helper, fixture, or struct literal, cite the production function that constructs the same state (function name + file:line). If no production analog exists, the must-have is FAILED, not VERIFIED. See Step 7d.

### 2. New Step 7d: Test Setup Audit
Inserted between Step 7c (probe execution) and Step 8 (human verification needs). Full text:

> ## Step 7d: Test Setup Audit
>
> Anti-pattern scanning (Step 7) checks production code. Test setup audit checks that tests exercising production state types do so via paths production can actually reach. A test that constructs production state via a `#[cfg(test)]` helper or fixture is only evidence of production behavior if production has an equivalent construction path.
>
> **Why this matters:** A common failure mode — and the one that motivated this step — is the verifier accepting a test-only state constructor as evidence the production behavior works, when no production code path reaches that state. The test verifies a configuration production can never enter, and the goal-backward chain is broken at the setup layer. "Acceptable because it's test-only" is not a valid disposition.
>
> **When to run:** For phases with state machines, multi-step workflows, persistence layers, or anywhere the verifier relies on tests as evidence of production behavior. Skip for pure utility/data-transformation phases where every input is reachable from public API.
>
> **How:**
>
> 1. **Identify test setup helpers in the phase's modified files** (grep for `fn build_|make_|create_|new_test|fake_|mock_|stub_|dummy_` across .rs/.ts/.tsx/.js/.py).
> 2. **For each helper that constructs a production type**, cite the production analog (function name + `file:line`) OR confirm none exists.
> 3. **Classify and dispose** (LOW = acceptable fixture; MEDIUM = note + recommend migration; HIGH = must-have FAILED).
>
> **Decision rule:** If a must-have's only evidence is a test that uses a HIGH-risk helper, mark the must-have FAILED with reason "test setup constructs state production cannot reach; production analog missing." The override pattern (Step 3b) applies if the test setup is intentional, but the must-have stays FAILED until the override is recorded.

### 3. Updated success-criteria checklist
Added one item:

> - [ ] Test setup audit run on tests cited as evidence (Step 7d) — production analog cited or must-have FAILED

## Scope decisions (recorded for posterity)

| Decision | Choice | Rationale |
| --- | --- | --- |
| Tooling vs project-scoped | **gsd-verifier only** (shared tooling) | Direct fix for the exact agent that produced v1.1's miss. Lowest blast radius while still applying to every future project. |
| Other auditor agents (security, nyquist, eval, ui, doc) | Not modified | Recommended in inventory but not done in this pass. They face similar but not identical issues. If a future audit miss surfaces the same pattern in one of them, port the heuristic there too. |
| Edit depth | Critical rule + Step 7d + checklist item | Surgical — verifiers see the rule in `<critical_rules>` (high-visibility), the procedure in Step 7d (actionable), and the checklist enforces completion. |

## How to verify this works
The next time a verifier runs against a blindjoin phase, it should:
1. Surface any test-only state constructors in the phase's modified files.
2. Cite each one's production analog or mark the related must-have FAILED.
3. The success criteria checklist will be incomplete unless Step 7d ran.

If a future verifier produces a VERIFICATION.md that accepts test-only setup without citing a production analog, the heuristic failed in practice — revisit the agent definition.

## Workstream summary
All 3 deliverables of `fix-verification-gap` are now complete:
- **D-1:** CI runs integration tests (merged in [PR #2](https://github.com/johnzilla/blindjoin/pull/2))
- **D-2:** Backdoor inventory ([BACKDOOR-INVENTORY.md](BACKDOOR-INVENTORY.md), merged in [PR #4](https://github.com/johnzilla/blindjoin/pull/4))
- **D-3:** This heuristic ([VERIFICATION-HEURISTIC.md](VERIFICATION-HEURISTIC.md))
