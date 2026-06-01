# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MVP

**Shipped:** 2026-04-09
**Phases:** 5 | **Plans:** 17 | **Timeline:** 3 days (2026-04-07 → 2026-04-09)

### What Was Built
- Full CoinJoin coordinator with RSA blind signatures (RFC 9474) ensuring cryptographic input-output unlinkability
- Client CLI with bdk_wallet, per-phase Tor circuit isolation (alice/bob), and anti-censorship PSBT verification
- Blame protocol with non-signer detection, UTXO banning with persistence, and automatic round restart
- PKARR DHT discovery — coordinators publish .onion addresses, clients resolve without hardcoded addresses
- Tor v3 hidden service via arti-client — no clearnet endpoint in production
- Docker Compose stack (bitcoind + coordinator + liquidity bot) for zero-to-CoinJoin in 5 minutes
- GitHub Actions CI: cross-compiled binaries (4 targets) and multi-arch Docker images to ghcr.io

### What Worked
- **Prove-Then-Layer build order**: Protocol bugs and network bugs never entangled. Each phase was independently verifiable.
- **Coarse 5-phase roadmap**: Kept scope tight. No phase sprawl. Each phase had a clear, testable goal.
- **Sequential execution (no worktrees)**: Avoided file conflicts and dangling commits. Simpler mental model.
- **Code review + auto-fix pipeline**: Caught real issues (SOCKS5 listener leak, silent oneshot failure) and fixed them automatically.

### What Was Inefficient
- **SUMMARY.md one-liner extraction**: Many summaries had malformed one-liners (literal "One-liner:" text). The summary template or agent needs stronger enforcement.
- **DEPL-01 tracking artifact**: Docker Compose was delivered in Phase 4 but the REQUIREMENTS.md checkbox wasn't checked, creating a false "incomplete" signal at milestone close.
- **arti-client API discovery**: Plans assumed APIs (`launch_socks5_listener`, `ConnectedFlags::new_empty`) that don't exist in arti-client 0.41. Required runtime deviation handling in every Tor-related plan.

### Patterns Established
- Thin reqwest RPC client over corepc-types (not the archived bitcoincore-rpc crate)
- In-process SOCKS5 proxy pattern for bridging arti TorClient to reqwest
- cargo-chef multi-stage Dockerfiles for all workspace binaries
- Domain-separated blind tokens: SHA-256("blindjoin-v1" || scriptPubKey || amount_sats_le64)

### Key Lessons
1. **Pin arti-client to exact version** — the API surface changes significantly between minor releases. Plan tasks should reference specific method signatures, not assumed APIs.
2. **Check crate APIs at research time** — Phase 5 research didn't verify `launch_socks5_listener` existence. A 5-minute `cargo doc` check would have avoided the deviation.
3. **Requirements checkboxes need a completion gate** — the executor agent should verify REQUIREMENTS.md checkboxes match SUMMARY.md claims at phase completion time.

### Cost Observations
- Model mix: ~20% opus (planning, verification), ~80% sonnet (execution, code review)
- Notable: Code review + fix pass added ~10% overhead but caught 2 critical bugs (listener leak, silent send failure)

---

## Milestone: v1.1 — Security & Availability Hardening

**Shipped:** 2026-04-10
**Phases:** 2 | **Plans:** 4 | **Timeline:** 1 day (2026-04-09 → 2026-04-10)

### What Was Built
- CI/CD security pipeline: PR-triggered test/clippy/audit gates, release and Docker workflows gated on check prerequisites
- Supply-chain hardening: all GitHub Actions SHA-pinned, SHA-256 checksums on release archives, per-job permission scoping
- Coordinator DoS hardening: validate_utxo RPC moved before write lock (AVAIL-01), RsaBlindSigner cached per-round (AVAIL-02)
- Input validation: blinded token size bounds, address pre-validation, duplicate partial-sig guard, fee formula consolidation

### What Worked
- **Targeted discuss-phase**: Only 3 gray areas per phase — kept discussion fast and focused for well-defined hardening work.
- **Gap closure cycle**: Verification caught that 07-02 executor re-introduced the RSA deserialization bug. The gap closure plan (07-03) fixed it cleanly in one task.
- **Code review + fix pipeline**: Caught and auto-fixed 8 findings across both phases (SHA pinning, audit in releases, checksums, permission scoping, dup-sig, token bounds, address validation, fee duplication).
- **Inline execution for gap closure**: Small fix executed inline without subagent overhead — faster and cheaper.

### What Was Inefficient
- **Borrow checker deviations**: Both 07-01 and 07-02 executors hit Rust borrow conflicts not anticipated in the plan. The plan assumed a signer parameter pattern that conflicted with &mut state. Plan-time should verify borrow patterns with cargo check.
- **SUMMARY.md one-liners still broken**: The one_liner extraction from summaries still returns "One-liner:" literal text. Agents aren't populating the frontmatter field correctly.

### Patterns Established
- Validate-then-lock pattern for coordinator handlers with async I/O
- Cached parsed crypto objects in state structs (keep raw bytes for zeroize, parsed object for hot path)
- CI workflow structure: separate ci.yml for PRs + check prereqs in release/docker workflows

### Key Lessons
1. **Verify Rust borrow patterns at plan time** — if a plan passes &signer and &mut state to the same function, check that the borrow checker accepts it. A quick `cargo check` during planning saves a gap closure cycle.
2. **Gap closure works well** — the verify → plan-gaps → execute-gaps → re-verify cycle closed AVAIL-02 cleanly. Don't fear gaps; the system handles them.
3. **Code review fixes are high-value** — the 8 auto-fixes improved supply-chain security, input validation, and code quality with minimal effort.

### Cost Observations
- Model mix: ~15% opus (planning), ~85% sonnet (execution, review, verification)
- Notable: Gap closure (07-03) added 1 extra plan but caught a real regression — worth the cost

---

## Milestone: v1.3 — Test Infrastructure & Operational Hardening

**Shipped:** 2026-05-29
**Phases:** 5 (9-13) | **Plans:** 13 | **Timeline:** 4 days (2026-05-26 → 2026-05-29)

### What Was Built
- Pinned bitcoind v30.2 substrate in CI (`.bitcoind-version`, `actions/cache@v4`, PGP+SHA256-verified install on cache miss, `BITCOIND_EXE` export, workflow-level `BLINDJOIN_REQUIRE_BITCOIND=1`)
- Shared test fixtures (`require_bitcoind!()` macro, `BitcoindGuard` RAII, `RpcCreds`, `bootstrap_regtest_bitcoind`); entire `tests/integration/` tree has zero `Box::leak` and zero inline skip blocks
- `CONTRIBUTING.md` canonical pattern (61 lines: prerequisites, copy-pasteable invocation, `--include-ignored` opt-in, 4-row verdict reference card)
- REPAIR-02 corepc-node feature pin CI gate (grep job); 4 WR-05 bare sleeps → bounded poll-until-deadline loops
- REPAIR-01 closed-local: all 8 `full_round::*` tests green via direct fixes — RSA SPKI handshake (`from_der`→`from_spki`); bdk_wallet 2.3 `SignOptions { trust_witness_utxo: true }`; partial-sig wire format = consensus-serialized `bitcoin::Witness`; coordinator real on-chain `witness_utxo`; ban-check ordering before blinded-token validation; coordinator error body surfaced in client error path
- Hygiene: 2 MEDIUM test backdoors replaced with production state-machine path; dead `--utxo-value-sats` CLI flag dropped; `--generate-wallet` placeholder documented; planning state reconciled with shipped reality

### What Worked
- **Phase 9 5-plan structure** — TEST-* requirements were tightly interlocking and each plan delivered a complete, observable substrate (pin → fixtures → migrate-callers-A → migrate-callers-B → docs). Wave dependencies were obvious; no phase-internal replans.
- **Pin-manifest pattern** — `.bitcoind-version` as a plain-text version file with no metadata composed cleanly into URLs, install commands, and docs. The cache-then-verify-on-miss pattern preserved the integrity gate without amortizing it away.
- **RAII + macro for test fixtures** — `BitcoindGuard`'s graceful `node.stop()` then `Node::Drop`'s `process.kill()` belt-and-suspenders meant no panic-in-Drop and no leaked processes regardless of test exit path. `require_bitcoind!()` as a macro (not a fn) was load-bearing because a plain function cannot return from the calling test scope.
- **Direct commits as escape valve** — after 3 escape-valve halts (Plans 11-02, 12-02, 13-01) under D-08/D-11/D-12, the structured Plan.md execution path had clearly ceased to be load-bearing. Pivoting to bisectable direct commits closed REPAIR-01 cleanly while preserving the original execution trace as a forensic audit log.

### What Was Inefficient
- **6 orthogonal blockers between Phase 10 and Phase 13** — REPAIR-01 had no clean Plan.md decomposition because the blocker chain (RPC schema → RSA SPKI → bdk_wallet 2.3 → wire format → witness_utxo correctness → ban ordering) was only discoverable serially. Each carry-forward (10→11→12→13) added planning overhead that didn't translate to shipped value. A "discover and patch as you find" debugging session would have shipped the same fixes in fewer artifacts.
- **D-08 → D-11 → D-12 escape-valve discipline correctly halted unwinnable plans** — but the protocol of "create a new phase to absorb the carry-forward" produced phases (11, 12, 13) whose Plan.md files were largely abandoned. The forensic value of preserving them is real, but the planning overhead per blocker was high.
- **HUMAN-UAT item 3 still deferred** (Tor-mode connection-cap test) — closed-local proof never arrived; the v1.4 milestone will inherit it.
- **REPAIR-01 closed-local only** — full PR observation closure deferred to v1.4 cut PR. The "closed-local" status is honest but adds a tracking burden the next milestone has to discharge.

### Patterns Established
- **Pin manifest** — plain-text version file at repo root (e.g. `.bitcoind-version`), no metadata, `$(cat)` substitutes cleanly into URLs and install scripts
- **Cache-then-verify-on-miss** — `actions/cache` restores the binary directly when warm; install step runs the full PGP+SHA256 integrity gate on miss before populating the cache. Cache poisoning recoverable by bumping the pin manifest.
- **Content-addressed key fetch from SHA-pinned upstream commit** — `bitcoin-core/guix.sigs` at a specific commit, not `main` HEAD, defeats a hostile upstream push between research and execution.
- **RAII guard with explicit `impl Drop`** — extends the `ConnectionGuard` pattern from `coordinator/src/network/tor.rs` for test-fixture resources requiring graceful shutdown before SIGKILL fallback.
- **Test-fixture macro pattern** — `#[macro_export] macro_rules!` in `tests/integration/mod.rs`, reachable as `$crate::macro_name!()` from submodules via the `[[test]]` declaration in `Cargo.toml`. Macro form is load-bearing for return-from-caller skip semantics.
- **Belt-and-suspenders against default flips** — set `view_stdout=false` and `-printtoconsole=0` explicitly even when one is the dependency default; protects against a future default flip silently re-introducing a root cause.
- **D-* escape-valve numbering** — D-08/D-11/D-12 as named halt conditions in CONTEXT.md let the executor and the human reviewer share a vocabulary about "when to stop" without litigating each blocker.

### Key Lessons
1. **For multi-orthogonal-blocker debugging, abandon Plan.md execution sooner.** When the 3rd carry-forward phase appears with the same shape as phases 11 and 12, the structured path has stopped paying for itself. Pivot to direct bisectable commits and preserve the planning trail as a forensic log. Future debugging sessions of this shape should be marked as `/gsd:debug` cycles from the start, not phase carry-forwards.
2. **Pin every dependency that's referenced by version in a test fixture.** Bitcoin Core via `.bitcoind-version`; corepc-node via explicit `features = ["NN_M"]`; both are now CI-enforced. The same discipline applies to any future test infra (Tor-mode harness, etc.).
3. **RAII over `Box::leak` for spawned external processes in tests.** `Box::leak` is fast to type and silently corrupts test isolation; `BitcoindGuard` is 10 more lines of code and eliminates an entire class of "tests pass locally, hang in CI" bugs. Apply this pattern to any future spawned-process test fixture.
4. **Distinguish "wire format" from "API shape" in client/coordinator contracts.** The 6th orthogonal blocker (HTTP 400 from /round/sign) was a wire-format encoding mismatch — coordinator deserialized `bitcoin::Witness` via consensus encoding; client sent raw DER bytes. Both sides "looked right" in isolation; only the wire byte stream surfaced it. Future wire-format changes should ship with a roundtrip serialization test in `shared/` before either side ships.
5. **`closed-local` is honest but adds tracking debt.** REPAIR-01 closed locally on 2026-05-29 but full PR observation requires the v1.4 cut PR. Future milestones should treat "closed-local" requirements as inherited todos, not as closed.

### Cost Observations
- Model mix: ~5% opus (audit-phase, planning narrowings), ~85% sonnet (execution, code review, fix loops), ~10% haiku (mechanical reformatting)
- Notable: The 6-blocker carry-forward chain meant ~3× the planning overhead of v1.1 for similar shipped LOC — the lesson is that debugging sessions and milestone phases have different cost curves, and confusing them is expensive

---

## Milestone: v1.5 — Audit-Readiness & Multi-Script Finish

**Shipped:** 2026-06-01
**Phases:** 3 (19, 20, 21) | **Plans:** 5 | **Tasks:** 12 | **Wallclock:** ~22 hours | **Commits:** 31 phase-tagged | **Diff:** 61 files, +12,824 / −293

### What Was Built
- **Phase 19:** Production BIP-322 `sign` bodies for P2TR (Schnorr keypath) + P2SH-P2WPKH (BIP-143) in `shared::bip322`; `sign_simple_test_only` + per-script `sign_for_tests` helpers deleted; `shared::bip322` public surface shrunk to exactly 9 symbols with V1.4-CRIT-01 dispatcher-only invariant load-bearing at the type level. Byte-equality with `BdkClientWallet::sign_bip322` proven empirically.
- **Phase 20:** Per-script BIP-141 vbyte table in `coordinator/src/bitcoin/tx.rs` (P2WPKH 68/31, P2TR 58/43 round-UP, P2SH-P2WPKH 91/32) replacing the legacy P2WPKH-only consts. `ParticipantInput.script_type` plumbed from `dispatch_ownership_proof` through `UtxoDetails → RegisteredInput`. v1.4 P2WPKH-only `fee_share == 266` byte-equality preserved by regression test.
- **Phase 21:** RSA SecretKey lifetime tightened from prose to Rust type signature (`Option<RsaBlindSigner>` on `RoundStateInner`, SOLE FSM chokepoint at `state.rs:202`). Shipped `docs/AUDIT-CHARTER.md` (574 LOC, 8 H2 sections), refreshed `.cargo/audit.toml` to cite the charter, added README §Security Model callout — all 3 files landed atomically per D-133a.

### What Worked
- **Wave-based parallel execution accelerated throughput.** 22-hour wallclock for a 3-phase milestone (5 plans, 12 tasks) is exceptional. Pure executor time was ~53 minutes (Phase 19: 11+7 min, Phase 20: 17 min, Phase 21: 7+11 min); the rest was discuss → research → plan overhead.
- **`must_haves.truths` + `must_haves.artifacts` + `key_links` plan frontmatter pattern made verification trivial.** The verifier could grep for each truth/artifact instead of re-deriving semantics. Phase 21's verifier accomplished 5/5 must-haves in 6 minutes because the plan front-loaded what "done" looks like.
- **The D-133a atomic-commit pattern prevented anchor-drift.** Phase 21-02 landed AUDIT-CHARTER.md + .cargo/audit.toml + README.md in ONE commit (92ae533), so the cross-artifact reference graph (audit.toml → charter §X, README → charter, charter → rsa.rs file:symbol) was consistent at every git revision.
- **Cross-phase invariant gate held at every phase boundary.** v1.3 `full_round::*` 8/8 + v1.4 `mixed_script_e2e_three_clients_broadcast` 1/1 never broke across the milestone. The pre-flight check was "would this PR break the v1.4 acceptance gate?" and the answer was always "no, because plumbing is additive."
- **The code reviewer's adversarial pass found a real Critical that the human-verifier would have missed.** CR-01 (`let _ = state.transition_to(...)` discarding the Result at 3 success-path FSM trigger sites) directly weakens AUDIT-03's structural-bound claim. Without the reviewer pass, this would have shipped without acknowledgment in the charter — an external auditor would have eventually flagged it.

### What Was Inefficient
- **The CLI auto-extracted MILESTONES.md accomplishments were low-quality.** `gsd-sdk query milestone.complete` extracted entries like "New helper" and "Dependency graph" because the SUMMARY.md frontmatter uses `key-files`/`key-decisions`/`provides` instead of a single `one_liner:` field that the extractor expects. Required ~5 min of manual rewriting at close.
- **The `audit-open` scanner over-reports.** A `21-HUMAN-UAT.md` with `status: resolved` and 0 pending scenarios got flagged as a `uat_gap`; a quick-task with a present SUMMARY.md but no `status:` frontmatter field got flagged as `missing`. Both are tooling false-positives that consumed user-facing decision time at close.
- **Verifier ordering vs human-UAT.** The verifier ran on Phase 21 BEFORE the user dispositioned the 3 HUMAN-UAT items, so VERIFICATION.md was stuck at `status: human_needed` until I manually updated it post-disposition. The workflow would be cleaner if the verifier re-ran after HUMAN-UAT resolved, or if the verifier's frontmatter auto-tracked HUMAN-UAT.md changes.
- **README link rendering check required a human round-trip.** No avoiding it (GitHub-rendered Markdown anchor behavior can't be grep-verified), but it interrupted the close flow. Recommending `grip` for local GitHub-faithful render at the point the link is added (not at close) would compress the loop.

### Patterns Established
- **Newtype as lifetime expression, not as scrub site.** When an upstream type already has correct Drop semantics (`rsa-0.9.10` `ZeroizeOnDrop` on `RsaPrivateKey`), the wrapper's value is making the lifetime a Rust value the FSM can null at one chokepoint. The `RoundSecretKey(BjSecretKey)` newtype's `Drop` body is empty-crypto-body (PII-safe `tracing::debug!` only) — the cryptographic work is delegated. Captured as D-129 / OQ1 lock.
- **Single-chokepoint Drop trigger, grep-verified.** All Phase → Idle FSM edges route through `transition_to(Phase::Idle)` at `state.rs:202`, the SOLE site setting `self.inner = None`. This is verifiable by grep over the entire `coordinator/src/` tree — the auditor's question "when does this secret die?" gets a one-line answer. Pattern is broadly applicable to any FSM-managed resource lifetime.
- **D-133a atomic-commit for cross-artifact reference loops.** When N files cite each other via anchors that would drift if landed separately (e.g., audit.toml cites charter §X, charter cites rsa.rs file:symbol, rsa.rs cites charter anchor), land all N in ONE commit. Prevents the "git checkout HEAD~1 → references broken" failure mode in audit-readiness contexts.
- **Production sign body = `#[cfg(test)] sign_for_tests` almost verbatim.** When the test helpers are already correct (they produce the witnesses the existing tests verify against), the production change is "make them production, remove the test-only escape hatch." Lower risk than greenfield implementation. Phase 19-01 was 11 min of executor time because of this.
- **Best-effort → structurally-bounded is a type-signature change, not a wording change.** AUDIT-03's value came from making `Option<RsaBlindSigner>` an expressible-in-Rust lifetime bound, not from rewriting the D-07 comment. The comment rewrite is downstream of the type change.
- **Pre-flight discuss-phase decisions amortize verification cost.** Phase 21's discuss-phase decisions (D-128 through D-143) covered 5 of the 6 anomalies that came up during execution; only CD-49 (slug-refinement) was a true execution-time deviation. Front-loading judgment calls before execution is cheaper than re-litigating them mid-plan.

### Key Lessons
1. **Schedule audit-readiness milestones AFTER the production code is solid, not before.** The charter was much easier to write when Phase 19 had already removed every `todo!()` — there was nothing to apologize for. Phase 19 unblocks Phase 21 was the right dependency direction.
2. **Document deferred items in PROJECT.md §Carry-Forward Items at close, not just in the milestone-specific archive.** Future-me will look at PROJECT.md first when scoping v1.6. Milestone archives are a forensic resource, not a discovery surface.
3. **The code-review pass before phase verification is worth its tokens.** CR-01 surfaced a defense-in-depth gap the verifier didn't catch (because the structural test passes — the let_ pattern is a runtime risk, not a build-time one). Adding a fast adversarial review to every audit-readiness phase paid off.
4. **`grip` is the right tool for local GitHub-faithful render verification.** Compresses the README-link-rendering loop from "push and check live" to "local preview, no push." Surface this earlier in workflows that touch user-facing Markdown.
5. **When `must_haves.truths` references shipped code (file:symbol form), the verifier becomes a grep + sanity-check exercise.** This is the structural inverse of "verify by running tests" — both are useful, but truth-driven verification is faster for type-signature-bearing requirements (AUDIT-03 in v1.5; V1.4-CRIT-01 in v1.4).

### Cost Observations
- Model mix: 100% sonnet for execution, code review, and verification. Opus not needed for v1.5 (no Opus-class architectural decisions — every phase was incremental refinement of existing patterns).
- Sessions: 1 session for v1.5 close (this one); ~3 sessions for the 3 phase executions (1 per phase boundary)
- Notable: The 5-plan milestone fit comfortably in a single session because each phase's discuss → plan → execute → verify loop was tight (Phase 21 from kickoff to verification was under 90 minutes wallclock). Compare to v1.3's 4-day debugging milestone (5 phases, 13 plans, multi-session) — phase scope matters more than phase count for session-level fit.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Timeline | Phases | Key Change |
|-----------|----------|--------|------------|
| v1.0 | 3 days | 5 | Initial milestone — established Prove-Then-Layer pattern |
| v1.1 | 1 day | 2 | Hardening milestone — established gap closure cycle and code review fix pipeline |
| v1.2 | 1 day | 1 | Single-phase milestone — promoted BACKLOG B-01 directly to plan; established design-contract-in-CONTEXT.md (no parallel REQ-IDs) |
| v1.3 | 4 days | 5 | Test infrastructure milestone — established pinned-binary + RAII fixture patterns; surfaced multi-orthogonal-blocker debugging mode where Plan.md execution stops paying for itself and direct commits are the right escape valve |
| v1.5 | 22 hours | 3 | Audit-readiness milestone — established newtype-as-lifetime-expression pattern, D-133a atomic-commit for cross-artifact loops, code-review-before-verification gate; pure executor time was ~53 min, the rest was discuss → research → plan overhead |

### Cumulative Quality

| Milestone | Rust LOC | Plans | Requirements |
|-----------|----------|-------|-------------|
| v1.0 | 7,353 | 17 | 52 (51 checked) |
| v1.1 | 5,918 | 4 | 6 (6 checked) |
| v1.2 | 5,918 | 4 | 6 design decisions (no REQ-IDs) — all closed |
| v1.3 | 6,490 | 13 | 7 (7 closed; REPAIR-01 closed-local pending PR observation) |
| v1.5 | 11,041 | 5 | 9 (9 closed) |

### Top Lessons (Verified Across Milestones)

1. Verify crate APIs exist before writing plans that depend on them
2. Sequential execution without worktrees is simpler and more reliable for solo builders
3. Verify Rust borrow patterns at plan time — cargo check during planning prevents gap closure cycles
4. Code review + auto-fix pipeline catches real issues with minimal overhead (~10%)
5. **For multi-orthogonal-blocker debugging, abandon Plan.md execution sooner.** After 2-3 carry-forward phases with the same shape, the structured path has stopped paying — pivot to direct bisectable commits and preserve the planning trail as a forensic log.
6. **Pin every dependency referenced by version in a test fixture, and CI-enforce the pin.** `.bitcoind-version` + corepc-node feature pin via grep gate is the pattern; apply to any future test infra.
7. **RAII over `Box::leak` for spawned external processes in tests.** 10 lines of code eliminates a whole class of "passes locally, hangs in CI" bugs.
8. **Newtype as lifetime expression, not as scrub site.** When upstream Drop semantics are correct, the newtype's value is making the lifetime a Rust value the FSM can null at one chokepoint. (v1.5 AUDIT-03)
9. **Run code review BEFORE phase verification, not after.** The reviewer catches defense-in-depth gaps the verifier doesn't (structural tests pass on the happy path; reviewers find runtime risks the structure permits). (v1.5 Phase 21 CR-01)
10. **D-133a atomic-commit for cross-artifact reference loops.** When N files cite each other via anchors that drift if landed separately, land all N in ONE commit. (v1.5 Phase 21)
11. **Schedule audit-readiness AFTER the production code is solid.** Charters are much easier to write when there's nothing to apologize for. (v1.5 Phase 19 unblocks Phase 21)
