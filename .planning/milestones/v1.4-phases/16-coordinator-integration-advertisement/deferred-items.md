# Deferred items discovered during Phase 16 execution

## Plan 16-02 (this plan)

### Pre-existing clippy errors in shared/src/bip322/*.rs

Running `cargo clippy --workspace --all-targets -- -D warnings` exits non-zero
on 14 lints in `shared/src/bip322/{mod,p2wpkh,p2tr,p2sh_p2wpkh}.rs`:

- 12x `clippy::result_large_err` — the `Bip322Error` enum's `CrateVerifyFailed`
  variant contains a `bip322::Error` source (≥ 192 bytes), making
  `Result<_, Bip322Error>` exceed the default `result_large_err` size threshold.
- 2x `clippy::unnecessary_to_owned` — internal `to_vec()` calls in
  `p2wpkh.rs:70` and `p2tr.rs:93`.

These exist at HEAD before Plan 16-02 and persist through this commit. Verified
by checking out HEAD~3 (the 16-01 SUMMARY commit) — same clippy errors.

**Why not fixed here:** Plan 16-02 is the load-bearing CRIT-01 dispatcher swap
commit and explicitly scopes to coordinator-side wiring (per the plan's
`<scope>` and the get-shit-done SCOPE BOUNDARY rule). Touching shared/ at this
plan boundary would mix a security-critical commit with a typed-error
refactor (boxing or denesting the Bip322Error variant), violating CD-10
atomic-commit discipline.

**Suggested follow-up:** A separate Phase 17 pre-cleanup plan, OR a small
shared/-targeted lint-cleanup commit landed in the bip322 plan that opens
Phase 17 (WALLET-02). The fix is mechanical: either box the `bip322::Error`
source on `CrateVerifyFailed`, or `#[allow(clippy::result_large_err)]` at the
module level with a rationale comment.

**Scope status at this plan boundary:** Build (`cargo build --workspace`) and
test (`cargo test --workspace` / integration suite) both pass. Only the strict
clippy `-- -D warnings` gate exposes these warnings.

**RESOLVED at v1.4 milestone close (2026-05-31):** All 14 lints fixed at the
milestone-cut boundary so the pre-push hook would accept the `v1.4.0` tag push.
- `Bip322Error::CrateVerifyFailed { source: bip322::Error }` →
  `{ source: Box<bip322::Error> }` (eliminated 11 `result_large_err` lints).
- 3 `to_vec()` calls in `p2wpkh.rs:70`, `p2tr.rs:93`, `p2sh_p2wpkh.rs:106`
  dropped (`Witness::push` takes `impl AsRef<[u8]>`).
- Also surfaced after the shared/ fix: `validate_utxo` in
  `coordinator/src/bitcoin/utxo.rs` triggered `too_many_arguments` (9/7);
  added `#[allow(clippy::too_many_arguments)]` since restructuring the
  load-bearing CRIT-01 validator at milestone-close was out of scope.

## Plan 16-03 (re-confirmation)

Plan 16-03 (PKARR record v0.2.0 + B3 compact-name rename + byte-budget tests)
touches ONLY `coordinator/src/discovery/pkarr_pub.rs` and `coordinator/src/run.rs`.
At the 16-03 commit boundary the same 14 shared/src/bip322/* clippy lints
persist verbatim. They are demonstrably PRE-EXISTING (16-02 SUMMARY logged them
at HEAD~3; 16-03 changes no shared/ files) and remain deferred per the SCOPE
BOUNDARY rule.

Per-package clippy scope coverage:
- `cargo clippy -p coordinator --all-targets -- -D warnings` fails (transitively
  compiles shared/ which has the 14 pre-existing lints).
- The pkarr_pub.rs + run.rs edits themselves introduce ZERO new lints — the
  failures all point to shared/src/bip322/{mod,p2wpkh,p2tr,p2sh_p2wpkh}.rs.
- v1.3 cross-phase invariant (`cargo test --test integration full_round`) green
  (8/8 pass) at the 16-03 boundary.

No additional follow-up beyond the suggestion documented for 16-02 above.
