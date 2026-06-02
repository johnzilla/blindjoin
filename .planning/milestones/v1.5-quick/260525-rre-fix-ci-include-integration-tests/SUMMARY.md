---
quick_id: 260525-rre
slug: fix-ci-include-integration-tests
workstream: fix-verification-gap
created: 2026-05-25
completed: 2026-05-25
status: complete
pr: https://github.com/johnzilla/blindjoin/pull/2
ci_run: https://github.com/johnzilla/blindjoin/actions/runs/26424797812
---

# Summary

CI now actually exercises integration tests, including the `round_bootstrap` test added in workstream `fix-round-bootstrap`. The class of regression that shipped in v1.1 — a missing production code path masked by test-only setup — is now detectable in CI.

## Commits (on branch `fix-ci-include-integration-tests`)
- `bb69580` — `ci: include integration tests and main-branch pushes in CI` — added `push: branches: [main]` trigger; switched test step from `cargo test --workspace --lib` to `cargo test --workspace --all-targets`.
- `abcc115` — `ci: add coordinator binary smoke job` — new job builds the `coordinator` binary in release mode and verifies it links successfully.
- `b9c33dc` — `ci: document deferred actions/checkout bump` — TODO comment referencing this workstream for a future v4→v6 bump.

## PR
[apps#2 — ci: include integration tests + smoke job + main push trigger](https://github.com/johnzilla/blindjoin/pull/2)

## CI verdict
All 4 jobs green on PR run [#26424797812](https://github.com/johnzilla/blindjoin/actions/runs/26424797812):

| Job | Result | Duration |
|---|---|---|
| cargo test | ✓ pass | 55s |
| cargo clippy | ✓ pass | 33s |
| cargo audit | ✓ pass | 2m48s |
| coordinator binary builds (smoke) | ✓ pass | 5m12s |

**Critically:** the `round_bootstrap` integration test executed and passed:
```
test round_bootstrap::run_bootstraps_round_into_input_reg ... ok
test result: ok. 9 passed; 0 failed
```

No bitcoind needed — the test exercises the in-process `run()` path directly.

## Deviations from plan
1. **Smoke step uses `cargo build --release` instead of `cargo run --bin coordinator -- --help`.** The coordinator binary doesn't expose a `--help` flag; running it without arguments attempts a real start and exits non-zero when bitcoind is unreachable. Used the planned fallback: validate the binary links cleanly. Rationale documented in commit message and inline YAML comment.
2. **`actions/checkout` SHA bump deferred.** Latest is v6.0.2 (Node 24); current pinned SHA is v4.3.1 (Node 20). v4→v6 is a major version bump with potential behavioral changes, so deferred per the "revert and TODO" rule. TODO comment added at top of `ci.yml` referencing this workstream.

## Impact on other workstreams
- **`fix-round-bootstrap`:** the bootstrap fix is now backed by automated CI proof, not just local `cargo test`. Workstream A is fully verified.
- **`fix-verification-gap` (this workstream):** Deliverable #1 complete. Remaining deliverables (backdoor inventory across all integration tests; verification-template heuristic) are independent follow-on phases.
- **`fix-ban-list-persistence` and `backlog-deferred-items`:** can now proceed with confidence that any regression they introduce will be caught by CI.

## Next
Merge [PR #2](https://github.com/johnzilla/blindjoin/pull/2) when user approves.
