# TODO

## Tech Debt

### Integration tests don't compile (`tests/integration/full_round.rs`)

The integration test file has ~1500 lines of end-to-end CoinJoin round tests that
require a live bitcoind (signet/regtest). During v1.0 and v1.1 development, several
struct signatures and function APIs changed without updating this file:

- `CoordinatorSection` missing `tor_mode` field (added in Phase 5)
- `CoordinatorConfig` missing `discovery` field (added in Phase 4)
- `poll_until_phase` now takes a `Duration` timeout argument (added in v1.0 bugfix)

**Impact:** Integration tests don't compile. Unit tests (67 tests) all pass and cover
the coordinator, client, and shared crate logic. CI runs `cargo test --workspace --lib`.

**When to fix:** When setting up real signet testing — the integration tests need a
running bitcoind anyway, so fixing compilation without being able to run them is
premature. Fix the signatures and verify end-to-end on signet at the same time.

**Files:** `tests/integration/full_round.rs`, `coordinator/src/config.rs` (for struct shapes)
