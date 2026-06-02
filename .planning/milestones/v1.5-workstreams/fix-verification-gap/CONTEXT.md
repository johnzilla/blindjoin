---
workstream: fix-verification-gap
priority: Medium
created: 2026-05-25
trigger: v1.1 verification missed a P0 production regression; need to understand why so it doesn't happen again
related: [fix-round-bootstrap]
---

# Context

## Why this exists
v1.1 shipped 2026-04-10 with a coordinator binary that **could never run a real
CoinJoin round**. Every Idle→InputReg transition lived in `#[cfg(test)]` code,
and CI plus the verification phase both signed off without catching it. The fix
landed in workstream `fix-round-bootstrap` (commits 4a5c2b3..d342359), but the
*process gap* that allowed it to ship is what this workstream addresses.

Three independent failures stacked to let this through. Any one of them being
absent would have caught the bug.

## Failure 1 — CI test command excludes integration tests
**Smoking gun.** `.github/workflows/ci.yml` test step runs:

```yaml
- name: Run tests
  run: cargo test --workspace --lib
```

`--lib` runs **library tests only** — it does NOT include anything under the
`tests/` directory. So even with a "real coordinator bootstrap" integration test
in place, CI would never have executed it.

**Remediation:** change to `cargo test --workspace --all-targets` (or just
`cargo test --workspace`) so integration tests run. Add a coordinator-binary
smoke step that spawns `target/debug/blindjoin-coordinator`, polls
`GET /info`, and asserts `phase != "idle"` within 10s. Without that smoke step,
nothing in CI exercises the actual production startup path.

**Sub-finding:** CI also only triggers on `pull_request` and `workflow_dispatch`
— not on push-to-main. The four commits for the round-bootstrap fix landed on
main without auto-triggering CI; had to be dispatched manually. Worth adding
`push: branches: [main]` so main is always green-tested.

## Failure 2 — test-only backdoor masked the missing production path
`tests/integration/full_round.rs:9-11` declares "no test-only backdoors in
coordinator code paths" (threat-model invariant T-06-02), then at lines 97-98,
487, 853, 885 does `Arc::new(RwLock::new(build_input_reg_round_state()))` —
which IS the backdoor. The test invariant was lampshaded and immediately
contradicted.

The same file uses `*round = build_input_reg_round_state()` (line 885) to
"restart" a round after blame. So even if the first round had a production
bootstrap path, the blame-restart path was also missing — masked by the same
backdoor.

The `fix-round-bootstrap` fix eliminated the backdoor (commit d342359) by
delegating to the new production `start_round()`. But this workstream should
inventory ALL test files for similar patterns:

- `grep -rn "build_.*_state\|fake_\|mock_" tests/`
- For each match, ask: "what production code path does this simulate?" If
  no production analog exists, that's a verification failure waiting to happen.

## Failure 3 — verification template accepts test helpers without asking
`.planning/milestones/v1.1-phases/07-coordinator-dos-hardening/07-VERIFICATION.md:95`
marks the test-only `from_der_secret_key` in `make_input_reg_state` as:

> "Test-only setup code simulating round initialization; not in production hot
> path. **Acceptable.**"

The verifier saw the test helper, marked it acceptable for the AVAIL-02
question, and **never asked "where does production simulate round
initialization?"** — the answer being "nowhere."

**Remediation:** add a verification-template heuristic. When a verifier
encounters test-only setup code that constructs production types, the verifier
MUST either:
- Cite the production analog (function name + file:line), or
- Mark the item as **NOT VERIFIED** and route to a phase that adds the missing
  production path.

"Acceptable because it's test-only" is no longer an acceptable disposition.

## Scope of this workstream

Three deliverables, roughly in priority order:

### 1. Fix CI to actually run integration tests (P0 within this workstream)
- Change `.github/workflows/ci.yml` test command to `cargo test --workspace --all-targets`.
- Add a `push: branches: [main]` trigger so main pushes are always tested.
- Add a coordinator-binary smoke job: `cargo run --bin blindjoin-coordinator &`, `curl --retry 10 http://localhost:8081/info | jq -e '.phase != "idle"'`. ~5 lines of bash, blocks the entire workflow if it fails.
- Re-trigger CI on `main` to confirm the new `round_bootstrap` integration test (added in commit 2bf312c) now runs and passes.

### 2. Backdoor inventory across all integration tests
- `grep -rn "build_.*_state\|fake_\|mock_" tests/` — enumerate everything.
- For each match, document the production analog (or absence thereof).
- File issues / TODOs for any backdoor without a production counterpart.

### 3. Verification-template heuristic
- Update whatever template/checklist `.planning/` uses for VERIFICATION.md generation.
- Add the "cite production analog or NOT VERIFIED" rule.
- If the template lives in a skill/agent rather than `.planning/`, route the change to that skill.

## Entry
Recommend `/gsd-quick` for the CI fix (small, high-leverage) and `/gsd-phase`
for the backdoor inventory + template change (more discovery work).

## Dependencies
- **Not blocked by anything.** The forensics findings already live in
  `.planning/workstreams/fix-round-bootstrap/root-cause-report.md`; this
  workstream consumes them.
- Should land before any future milestone work so the same gap doesn't
  reappear.
