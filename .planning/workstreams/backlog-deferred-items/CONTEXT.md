---
workstream: backlog-deferred-items
priority: Medium
created: 2026-05-25
trigger: External code review surfaced three deferred-by-design items worth scheduling
blocked_by: [fix-round-bootstrap]
---

# Context

## Why this exists
External code review flagged three gaps that are not regressions — they were
explicitly deferred from v1.1 scope — but the reviewer's writeup is a useful
prompt to promote them into the formal backlog with concrete entry points and
dependencies, so they don't drift into "we'll get to it" oblivion.

## Three phases to add to ROADMAP.md

### Phase: Public-endpoint hardening
Replace the stub at `coordinator/src/api/middleware.rs` (currently a 2-line
"will be added in Phase 2" comment) with:
- Per-route rate limiting (`tower-governor` or equivalent). Tighter limits on `/round/register_input` and `/round/sign` than on `/round/info`.
- Per-route timeouts via `tower::timeout`.
- Connection caps at the listener level.
- IP-based throttling layer (note: on Tor, this is `.onion` peer address — design accordingly).
- Verify no `tower` layers are missing from the router setup in `coordinator/src/api/mod.rs:51`.

### Phase: BIP-322 multi-script support
- Replace the custom implementation in `shared/src/bip322.rs` with the `bip322` crate from rust-bitcoin (already called out in research/STACK.md as the preferred choice).
- Remove the `is_p2wpkh()` hard gate at `coordinator/src/bitcoin/utxo.rs:119`.
- Add support for P2TR (Taproot) and P2SH-P2WPKH.
- Update PROJECT.md compatibility claims (line 87 currently says "forward compatible with all address types" but code enforces P2WPKH only).
- Risk note: `bip322` crate is 0.0.x — pin exact version and add property tests around sighash construction.

### Phase: Dynamic fee estimation
On top of the static model at `coordinator/src/bitcoin/fee.rs`:
- Mempool-aware fee polling (via the existing Bitcoin Core RPC client).
- Configurable safety margin on top of estimated fee rate.
- RBF strategy: bump fee if round transaction not confirmed within N blocks.
- Optional: CPFP fallback for stuck rounds.
- Update `coordinator/src/bitcoin/tx.rs:66-70` to use dynamic estimate instead of static config value.

## Entry
Recommend `/gsd-phase` to add all three phases to ROADMAP.md with appropriate
ordering and dependencies. Then `/gsd-discuss-phase` per phase when ready to
plan it.

## Dependencies
- **Blocked by `fix-round-bootstrap`** for any phase that needs end-to-end testing (all three eventually do).
- Public-endpoint hardening should land before any mainnet consideration.
- BIP-322 multi-script and Dynamic fee estimation are independent of each other.
