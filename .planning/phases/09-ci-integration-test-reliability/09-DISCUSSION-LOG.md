# Phase 9: CI integration-test reliability - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `09-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-05-27
**Phase:** 9-CI integration-test reliability
**Areas discussed:** bitcoind install in CI, Skip-vs-fail policy, Box::leak replacement, CONTRIBUTING.md invocation

---

## bitcoind install in CI

### Q1: How should the CI runner acquire the pinned bitcoind binary?

| Option | Description | Selected |
|--------|-------------|----------|
| Tarball + actions/cache | Download bitcoincore.org prebuilt linux-x86_64 tarball, verify against SHA256SUMS.asc, extract to ~/.local/bin, cache by version. | ✓ |
| Build from source | Clone bitcoin/bitcoin at a pinned tag and build. ~15-25min uncached. | |
| Third-party GitHub Action | Use an off-the-shelf bitcoind action. Adds external supply-chain surface. | |
| Docker service container | GitHub Actions `services:` block; doesn't fit corepc-node's spawn-the-child pattern. | |

**User's choice:** Tarball + actions/cache
**Notes:** Aligned with Phase 6 SHA-pin baseline and existing `Swatinem/rust-cache@v2` pattern.

### Q2: Where should the bitcoind version pin live (single source of truth)?

| Option | Description | Selected |
|--------|-------------|----------|
| `.bitcoind-version` file | Plain-text at repo root. Single source of truth, easy to grep, single-line bump PR. | ✓ |
| CI workflow env var | Hardcoded in `ci.yml`. Less discoverable from local dev. | |
| Cargo.toml [workspace.metadata] | Cargo-native but needs TOML parsing in the workflow. | |

**User's choice:** `.bitcoind-version` file

### Q3: Which Bitcoin Core version should we pin at?

| Option | Description | Selected |
|--------|-------------|----------|
| 30.0 (latest 30.x) | Matches corepc-node `features=["30_2"]` schema declaration. | ✓ |
| Latest stable (whatever's current) | Pull whatever bitcoincore.org currently shows. Version-drift risk. | |
| Match brew (v31) | Phase 8 verified RPC compatibility with brew's v31, but features=["30_2"] declares v30 schema. | |

**User's choice:** 30.0

### Q4: How should the tarball's integrity be verified?

| Option | Description | Selected |
|--------|-------------|----------|
| SHA256SUMS + signed manifest | PGP verify against pinned guix-signer key, then hash check. | ✓ |
| SHA256SUMS only (no PGP) | Hash check against bitcoincore.org's SHA256SUMS. No signature verify. | |
| Inline hash literal in workflow | Hardcode SHA256 in the workflow file. | |

**User's choice:** SHA256SUMS + signed manifest

### Q5: Where in the CI workflow should the bitcoind install step live?

| Option | Description | Selected |
|--------|-------------|----------|
| Inline step in test job | Simple, no composite-action overhead. | ✓ |
| Composite action under .github/actions/ | Reusable across jobs but more indirection. | |
| Shell script + step | scripts/ci/install-bitcoind.sh invoked from workflow. Adds a /scripts directory. | |

**User's choice:** Inline step in test job

### Q6: How should corepc-node find the bitcoind binary at runtime?

| Option | Description | Selected |
|--------|-------------|----------|
| BITCOIND_EXE env var | corepc-node honors this env var first; works for CI and local dev. | ✓ |
| Add to $PATH | Append to $GITHUB_PATH; brittle to PATH ordering. | |
| Cargo dev-dep download feature | corepc-node's download feature pulls bitcoind itself; bypasses our version pin. | |

**User's choice:** BITCOIND_EXE env var

---

## Skip-vs-fail policy

### Q7: What should happen when a bitcoind-requiring test runs and bitcoind is absent?

| Option | Description | Selected |
|--------|-------------|----------|
| Env-var gate: BLINDJOIN_REQUIRE_BITCOIND=1 | Panic when set and bitcoind absent; graceful-skip otherwise. Matches Phase 8's BLINDJOIN_ALLOW_CLEARNET=1 pattern. | ✓ |
| Hard fail always | Remove the graceful-skip entirely; every dev needs bitcoind. | |
| Cargo feature flag: --features integration-bitcoind | Tests behind cfg(feature). Cargo features compose poorly with --test. | |
| Split test binary | Separate `tests/integration_bitcoind.rs`. Two-binary topology. | |

**User's choice:** Env-var gate: BLINDJOIN_REQUIRE_BITCOIND=1

### Q8: How should the env-var gate be implemented across the 3 test files?

| Option | Description | Selected |
|--------|-------------|----------|
| Shared helper in tests/integration/mod.rs | Single point of policy, ~7 callsites updated. | ✓ |
| Macro: require_bitcoind!() | Same semantics as helper; declarative-magic with no real upside. | |
| Inline copy-paste in each test | Most surgical change; policy duplicated 7 times. | |

**User's choice:** Shared helper in tests/integration/mod.rs

### Q9: Where should BLINDJOIN_REQUIRE_BITCOIND=1 be set in CI?

| Option | Description | Selected |
|--------|-------------|----------|
| Workflow-level env block in ci.yml | Alongside FORCE_JAVASCRIPT_ACTIONS_TO_NODE24. Applies uniformly. | ✓ |
| Step-level env in test job only | Tighter scope but doesn't carry to future jobs. | |
| Implicit via CI=true detection | Couples policy to provider convention. | |

**User's choice:** Workflow-level env block in ci.yml

### Q10: Should Phase 10's full_round.rs (6 RPC-drift tests) be exempt from CI enforcement?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — `#[ignore]` the 6 known-broken tests | TODO(Phase-10) comment; Phase 10 removes markers as it repairs. | ✓ |
| No — fail CI on full_round.rs until Phase 10 lands | Honest signal but blocks Phase 9 merge. | |
| Delete full_round.rs as part of Phase 9 | Unilateral retirement; conflicts with REPAIR-01's optionality. | |

**User's choice:** Yes — exempt the 6 broken tests

---

## Box::leak replacement

### Q11: What's the bitcoind Node lifecycle pattern to replace Box::leak with?

| Option | Description | Selected |
|--------|-------------|----------|
| RAII drop guard returned from spawn_blocking | BitcoindGuard owns Node; Drop calls node.stop(). Idiomatic, scoped per-test. | ✓ |
| Arc<Mutex<Option<Node>>> + explicit teardown | Tests drain on cleanup; panic-during-test still leaks unless catch_unwind. | |
| Shared static + #[ctor::dtor] | Single shared bitcoind; breaks test isolation. | |
| Per-test fixture function with FnOnce closure | Limits async structure; !Send constraints make this awkward. | |

**User's choice:** RAII drop guard returned from spawn_blocking

### Q12: How should BitcoindGuard's Drop actually terminate bitcoind?

| Option | Description | Selected |
|--------|-------------|----------|
| node.stop() RPC + brief wait | Graceful shutdown, SIGKILL fallback if needed. | ✓ |
| Drop without explicit stop (rely on corepc-node) | Equivalent to today's broken behavior. | |
| SIGTERM + drop | Bypass RPC; no state flush. | |

**User's choice:** node.stop() RPC + brief wait

### Q13: How should the guard survive a test panic?

| Option | Description | Selected |
|--------|-------------|----------|
| Rely on stack-unwinding Drop | Standard Rust idiom; guard in let-binding drops on panic. | ✓ |
| Add a #[ctor::dtor] backstop registry | Belt-and-suspenders for abort() panics; adds ctor dev-dep. | |
| Use std::panic::catch_unwind per test | Heavy-handed; unwinding already handles this. | |

**User's choice:** Rely on stack-unwinding Drop

### Q14: Where should BitcoindGuard + require_bitcoind() helper live?

| Option | Description | Selected |
|--------|-------------|----------|
| tests/integration/mod.rs (existing module) | Matches existing structure; one import path. | ✓ |
| tests/common/mod.rs (new submodule) | Two import paths once tests/integration/ stays. | |
| Inline + per-file copy | Logic duplicated 3-4 times. | |

**User's choice:** tests/integration/mod.rs

### Q15: Should bootstrap_regtest_bitcoind also become a shared helper?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — shared bootstrap_regtest_bitcoind() | Collapses duplicated `Node::with_conf` + mine-101 + cookie logic across 3 files. | ✓ |
| No — just fix the leak, keep bootstrap per-file | Smallest diff; leaves duplication. | |
| Helper only for new tests | Creates two patterns coexisting. | |

**User's choice:** Yes — shared helper

### Q16: How should bitcoind's stdout/stderr be handled to prevent cargo-test pipe-hang?

| Option | Description | Selected |
|--------|-------------|----------|
| -printtoconsole=0 + redirect to per-test temp log | Defense in depth; even slow Drop doesn't hold cargo's pipe. | ✓ |
| Redirect to /dev/null | Quieter; loses postmortem context on failure. | |
| Leave stdio as-is; trust Drop | Cleanest if it works; previous evidence says it didn't. | |

**User's choice:** -printtoconsole=0 + redirect to per-test temp log

---

## CONTRIBUTING.md invocation

### Q17: What's the canonical command in 'Running integration tests'?

| Option | Description | Selected |
|--------|-------------|----------|
| Plain cargo test with explicit redirect | `BLINDJOIN_REQUIRE_BITCOIND=1 ... cargo test --test integration -- --include-ignored 2>&1 \| tee target/integration-test.log` | ✓ |
| scripts/test-integration.sh wrapper | Adds /scripts directory; structurally prevents pipe pitfall. | |
| cargo-nextest wrapper | New dev tool dependency; bigger ecosystem shift than the phase warrants. | |

**User's choice:** Plain cargo test with explicit redirect

### Q18: Should CONTRIBUTING.md be brand-new and scoped to integration testing only?

| Option | Description | Selected |
|--------|-------------|----------|
| Narrow: integration tests + dev setup only | Tight scope; ships in this phase. | ✓ |
| Broad: full contributor guide | Documents conventions that aren't yet locked. | |
| Integrate into README.md | Dilutes the user-facing surface. | |

**User's choice:** Narrow

### Q19: What pitfalls/cautionary backstory should the section include?

| Option | Description | Selected |
|--------|-------------|----------|
| Brief 'why the redirect' callout, no full backstory | One sentence on the pipe-hang. | ✓ |
| Full incident-style backstory | Educational but ages poorly. | |
| No pitfall explanation | Saves words; loses institutional knowledge. | |

**User's choice:** Brief callout

### Q20: Should the section show how to run a single bitcoind-dependent test?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — single-test invocation example | Real productivity win for contributors. | ✓ |
| No — full-suite invocation only | Less helpful to first-timers. | |

**User's choice:** Yes

### Q21: Where should the log file land (the redirect target)?

| Option | Description | Selected |
|--------|-------------|----------|
| target/integration-test.log | Lives under cargo's gitignored build dir; auto-cleaned by cargo clean. | ✓ |
| logs/integration-test.log | Dedicated dir at repo root; needs .gitignore update. | |
| /tmp/integration-test.log | OS-dependent ($TMPDIR varies). | |

**User's choice:** target/integration-test.log

### Q22: How should we explain pass/fail/skip interpretation?

| Option | Description | Selected |
|--------|-------------|----------|
| Short 3-line table mapping output strings to verdicts | Scannable reference card. | ✓ |
| Free-form prose paragraph | Wordier, less scannable. | |

**User's choice:** Short 3-line table

---

## Claude's Discretion

- Exact PGP fingerprint for SHA256SUMS.asc verification (D-04) — planner picks from current Bitcoin Core release-signer set.
- Exact tarball filename pattern (controlled by bitcoincore.org).
- Whether the per-test temp log path is parameterised or hardcoded to `$TMPDIR` — hardcoded matches existing fixtures.
- Tone and prose of CONTRIBUTING.md sections — decisions specify content, not phrasing.
- Single consolidated log vs one log per test binary — single matches the canonical `tee` target.

## Deferred Ideas

- Composite GitHub Action for bitcoind install — defer until Tor-mode harness needs it (v1.4+).
- cargo-nextest adoption — considered for D-16; deferred.
- scripts/test-integration.sh wrapper — considered for D-16; deferred.
- Tor-mode integration harness — already deferred to v1.4+ per REQUIREMENTS.md.
- Workspace-wide audit of `corepc-node` features (REPAIR-02) — Phase 10.
- Repair of 6 RPC-schema-drift `full_round.rs` tests (REPAIR-01) — Phase 10.
