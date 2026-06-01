---
status: resolved
phase: 21-audit-charter-zeroization-tightening
source: [21-VERIFICATION.md]
started: 2026-05-31T23:55:00Z
updated: 2026-05-31T23:55:00Z
---

## Current Test

[all items resolved]

## Tests

### 1. CR-01 disposition — let _ = on FSM transitions
expected: User decides one of:
  (a) BLOCKER — Phase 21 must add explicit transition-result handling + `state.inner = None` fallback at signing.rs:279-280, blame.rs:219-220, output_reg.rs:30-31 before close
  (b) WARNING — accept with rationale (write-lock semantics make the failures unreachable today); document as Residual Risk in AUDIT-CHARTER.md §7 sub-bucket (b) Protocol-level
  (c) DEFER — to Phase 22 follow-up
result: passed
resolution: (b) WARNING — accepted. Documented as `AUDIT-03 chokepoint result-discard pattern (let _ = on FSM transitions)` in AUDIT-CHARTER.md §7 Residual Risks: Protocol-level. Names the 3 sites, the write-lock invariant that makes the failures unreachable today, and the explicit closure trigger (any future FSM concurrency-model or can_transition_to-edge change MUST audit these 3 sites). Closure deferred to v1.6+.

### 2. Line-number drift across rsa.rs / audit.toml / AUDIT-CHARTER.md
expected: User decides one of:
  (a) ACCEPT — file:symbol form is the durable anchor; parenthetical line numbers are orientation-only; current drift is acceptable
  (b) FIX-NOW — require line-number citations corrected before phase complete (state.rs:194-200 → state.rs:202; transition_to at line 186 → line 193)
  (c) REPLACE — convert every numeric line-anchor to file:symbol form per charter §1's own preferred convention
result: passed
resolution: (b) FIX-NOW — applied. Corrected 5 citation sites: rsa.rs D-07 comment (now cites state.rs:193 decl + state.rs:202 chokepoint), state.rs `rsa_signer` field doc-comment (state.rs:195 → state.rs:202), audit.toml RUSTSEC-2023-0071 rationale paragraph, AUDIT-CHARTER.md §2 RSA Marvin Attack mitigation paragraph, AUDIT-CHARTER.md §5 The trigger paragraph + Drop-chain ASCII diagram.

### 3. README link rendering on GitHub
expected: Open README.md on github.com/johnzilla/blindjoin (or local rendered preview); confirm:
  (a) the §Security Model paragraph is between Supply-chain hygiene and Test infrastructure paragraphs
  (b) the `docs/AUDIT-CHARTER.md` link is blue + clickable
  (c) clicking it loads AUDIT-CHARTER.md correctly
result: passed
resolution: User confirmed all 3 sub-criteria via local GitHub-faithful render. §Security Model paragraph placement correct, link rendered as expected, AUDIT-CHARTER.md loads with the 8 H2 sections intact.

## Summary

total: 3
passed: 3
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps
