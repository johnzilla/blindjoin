# Backlog

Items that are scoped but not scheduled. Each entry is detailed enough that a future milestone planning session can pull it directly into a phase without rediscovery.

**Convention:** Backlog entries are NOT bugs. Bugs go through `/gsd-debug` and ship as fixes. Backlog entries are features, refactors, or hardening work that was deliberately deferred from a prior milestone's scope. Each entry must include enough context — code references, recommended approach, dependencies — that "when do we schedule this" is the only open question.

**Source attribution:** When promoting an entry from this backlog into a milestone, link back to the entry's source workstream so the rationale survives.

---

## B-01 — Public-endpoint hardening

**Status:** Deferred from v1.1. Acknowledged by the codebase itself ([coordinator/src/api/middleware.rs:1-2](coordinator/src/api/middleware.rs:1)): `// Rate limiting and additional middleware will be added in Phase 2.`

**Why deferred:** v1.1 shipped clearnet-first; the only middleware applied is `RequestBodyLimitLayer(64KB)` ([coordinator/src/api/mod.rs:51](coordinator/src/api/mod.rs:51)). Adequate for the demo story, not for an exposed Tor service.

**Why it matters:** On Tor, an open coordinator is trivially DoS-able and sybil-fillable. Without rate limiting, a single attacker can exhaust the input registration phase before honest clients arrive. This is a real concern the moment we move to mainnet or recommend production use.

**Scope:**
- Per-route rate limiting (`tower-governor` is the obvious pick — already tokio/tower-native). Tighter limits on `/round/register_input` and `/round/sign` than on `/round/info`.
- Per-route timeouts via `tower::timeout`.
- Connection caps at the listener level (axum/hyper `Server::tcp_nodelay`-style controls, or arti's onion-service equivalent).
- IP-based throttling — note: on Tor this is the `.onion` peer address, not an IP. Design accordingly; the abstraction should accept "remote peer identifier" not "IP."
- Audit `coordinator/src/api/mod.rs` for missing `tower` layers that an experienced reviewer would expect on a public service.

**Dependencies:** None blocking; can be scheduled independently. Should land **before** mainnet rollout.

**Estimated complexity:** Small-to-medium phase. ~3-5 plans. Most of the surgery is in `coordinator/src/api/`. Touching the Tor adapter (`arti-axum`) is the only non-obvious bit — peer identity on Tor is not the same shape as a clearnet `SocketAddr`.

**Recommended entry:** `/gsd-discuss-phase` (the rate-limit policy has gray-area decisions about per-route weights). Then `/gsd-plan-phase`.

**Source:** Surfaced by external code review 2026-05-25 via workstream [`fix-verification-gap`'s adjacent context](workstreams/backlog-deferred-items/CONTEXT.md). Reviewer's finding was the trigger; the v1.1 scope deferral was the underlying decision.

---

## B-02 — BIP-322 multi-script support

**Status:** Deferred. Code currently enforces P2WPKH-only at [coordinator/src/bitcoin/utxo.rs:119](coordinator/src/bitcoin/utxo.rs:119) via an explicit `is_p2wpkh()` hard gate.

**Why deferred:** v1.0 / v1.1 prioritized a working CoinJoin loop over input-type breadth. P2WPKH coverage was sufficient for the demo story.

**Why it matters:** [PROJECT.md:87](PROJECT.md:87) claims "Forward compatible with all address types" but the code enforces P2WPKH only. Any Taproot, P2SH-P2WPKH, or P2WSH input fails registration with `UnsupportedScriptType`. This is both a privacy reduction (smaller anon set) and a documentation-vs-code mismatch.

**Scope:**
- Replace the custom `shared/src/bip322.rs` implementation with the `bip322` crate from the rust-bitcoin organization (already called out in `research/STACK.md` as the preferred choice — it wasn't adopted at the time because the crate was 0.0.x).
- Remove the `is_p2wpkh()` hard gate at [coordinator/src/bitcoin/utxo.rs:119](coordinator/src/bitcoin/utxo.rs:119).
- Add support for P2TR (Taproot) and P2SH-P2WPKH at minimum. P2WSH is a stretch goal.
- Update `PROJECT.md:87` once code matches the claim (or before, with a `## Supported address types` table that's accurate during the rollout).
- Add property tests around sighash construction for each script type — the `bip322` crate is still pre-1.0, and getting sighashes wrong silently produces invalid signatures.

**Dependencies:** None blocking. Best paired with B-01 if you're going to do a v1.2 / "production readiness" milestone.

**Estimated complexity:** Medium phase. ~3-4 plans. The risk surface is the crate version pin — verify the API matches what's published and what we expect before committing to it.

**Recommended entry:** `/gsd-discuss-phase` to lock in script-type coverage and crate pinning strategy. Then `/gsd-plan-phase`.

**Source:** External code review 2026-05-25. The bip322 crate adoption was an explicit deferral noted in `research/STACK.md`.

---

## B-03 — Dynamic fee estimation

**Status:** Deferred. Coordinator uses a static fee rate from config at [coordinator/src/bitcoin/tx.rs:66-70](coordinator/src/bitcoin/tx.rs:66) and [coordinator/src/bitcoin/fee.rs](coordinator/src/bitcoin/fee.rs).

**Why deferred:** Wasabi v1 also used static fee estimation; it was acceptable for v1.x scope. Signet has predictable fees, so the impact on the demo story was nil.

**Why it matters:** Static fee estimation cuts both ways. Under-paying causes stuck rounds (waits for confirmation, ties up UTXOs, frustrates participants). Over-paying causes unusual fee rates that are a privacy signal (every CoinJoin round transaction is distinguishable from organic spending by its uniform fee rate). Real production needs mempool awareness and an RBF strategy.

**Scope:**
- Mempool-aware fee polling. Use the existing Bitcoin Core RPC client to call `estimatesmartfee` periodically. Pick a target confirmation depth (e.g. 6 blocks) and use the returned rate as the baseline.
- Configurable safety margin on top of the estimate (e.g. 20% headroom by default; configurable per operator).
- RBF strategy: if the round transaction isn't confirmed within N blocks, bump the fee. Need to design who pays for the bump (coordinator? per-participant?) — this has UX implications the discuss phase should resolve.
- Optional: CPFP fallback for the rare case where RBF isn't possible (e.g. parent has BIP-125 disabled).
- Replace the static config lookup at [coordinator/src/bitcoin/tx.rs:66-70](coordinator/src/bitcoin/tx.rs:66) with the dynamic estimate.

**Dependencies:** None blocking. Most useful for mainnet readiness. Doesn't depend on B-01 or B-02.

**Estimated complexity:** Medium-to-large phase. ~4-6 plans. The RBF accounting (who pays) is the gray area; the rest is mechanical. Mempool polling needs to be bounded — don't hammer the RPC.

**Recommended entry:** `/gsd-discuss-phase` is essential here — the "who pays for the RBF bump" question has real UX trade-offs. Then `/gsd-plan-phase`.

**Source:** External code review 2026-05-25. The static-fee approach was an explicit v1.x scope decision matching Wasabi v1 precedent.

---

## When to schedule

These three items together would naturally form a **v1.2 Production Readiness** milestone. Suggested ordering:

1. **B-01 (public-endpoint hardening)** — first. Smallest scope, immediate DoS/sybil protection benefit. No design gray areas.
2. **B-03 (dynamic fee estimation)** — second. Higher complexity, needs RBF design call.
3. **B-02 (BIP-322 multi-script)** — third or in parallel with B-03. Independent of both others; broadens compatibility once the operational basics are solid.

But there's no requirement they ship together — any of the three can be picked up as a standalone phase under a different milestone. The dependencies field on each entry is authoritative.

## Promotion checklist

When promoting a backlog entry into a milestone:
1. Read the entry end-to-end.
2. Verify code references still resolve (file paths, line numbers) — these can rot between writing and scheduling.
3. Re-check dependencies. If the upstream changed, the entry may need re-scoping.
4. Add a `Promoted: <YYYY-MM-DD> to milestone v<X.Y>` line to the entry. Don't delete the entry until the milestone ships — the entry's rationale should survive in git history with the link.
