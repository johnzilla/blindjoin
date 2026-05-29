---
workstream: backlog-deferred-items
created: 2026-05-25
status: partial
last_audit: 2026-05-29
---

# Project State

## Current Position
**Status:** Partial — 1 of 3 items shipped, 2 still open as v1.4+ work
**Last Activity:** 2026-05-29 -- audit confirmed item 1 shipped; STATE updated

## Item status

1. **Public-endpoint hardening** — **SHIPPED** as v1.2 Phase 8 (`08-VERIFICATION.md`).
   Per-route rate limits via `tower_governor`, request timeouts via
   `tower_http::TimeoutLayer`, connection cap via tokio Semaphore.
   `GlobalKeyExtractor` is the documented design choice for Tor (no
   per-IP throttling on hidden services).

2. **BIP-322 multi-script support (P2TR, P2SH-P2WPKH)** — **STILL OPEN.**
   Hard gate at [coordinator/src/bitcoin/utxo.rs:119](../../../coordinator/src/bitcoin/utxo.rs)
   (`if !script_pubkey.is_p2wpkh()`). P2WPKH only. Listed as
   "Future Requirements" in `.planning/REQUIREMENTS.md`.

3. **Dynamic fee estimation** — **STILL OPEN.** Static `fee_rate_sat_per_vbyte`
   config value used at [coordinator/src/bitcoin/tx.rs](../../../coordinator/src/bitcoin/tx.rs).
   No mempool awareness, no RBF, no CPFP. Listed as "Future Requirements"
   in `.planning/REQUIREMENTS.md`.
