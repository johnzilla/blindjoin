# Phase 7: Coordinator DoS Hardening - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Eliminate two denial-of-service vectors in the coordinator's input/output registration handlers:
1. Async bitcoind RPC call held under RoundState write lock (blocks all concurrent participants)
2. RSA private key deserialized from DER bytes on every request (CPU-intensive, abusable)

</domain>

<decisions>
## Implementation Decisions

### RPC Refactor (AVAIL-01)
- **D-01:** Use validate-then-lock pattern: perform full UTXO validation (`validate_utxo` RPC call) BEFORE acquiring `state.round.write().await`
- **D-02:** After RPC validation, acquire write lock and re-check phase + double-registration under lock (TOCTOU prevention — pattern already exists at handlers.rs:120-129)
- **D-03:** The existing read-lock phase check (handlers.rs:76-87) stays as-is — it's a fast rejection before RPC

### RSA Key Caching (AVAIL-02)
- **D-04:** Add a parsed `rsa_signer: Option<RsaBlindSigner>` field to `RoundStateInner` (coordinator/src/round/state.rs)
- **D-05:** Set the parsed signer once at round creation (when `RoundStateInner` is constructed)
- **D-06:** Both `post_input` (handlers.rs:158-164) and `post_output` (handlers.rs:281-289) read the cached signer instead of calling `RsaBlindSigner::from_der_secret_key()` per-request
- **D-07:** Keep raw `rsa_signing_key: Vec<u8>` for zeroize-on-drop — the cached signer is a convenience, the raw bytes are the canonical secret

### Testing
- **D-08:** Unit tests only — verify lock ordering and signer reuse without requiring bitcoind
- **D-09:** No integration tests for concurrency in this phase

### Claude's Discretion
- Exact function signature changes in `register_input` and `register_output_logic`
- Whether to extract the RPC validation into a separate function or keep inline
- How to handle the `RsaBlindSigner` not implementing `Clone` (if applicable — check the type)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Coordinator Handlers (primary modification targets)
- `coordinator/src/api/handlers.rs` — `post_input` (lines 71-205) and `post_output` (lines 208-340) — both have the RPC-under-lock and key deserialization issues
- `coordinator/src/round/input_reg.rs` — `register_input()` function that currently does RPC inside write lock scope
- `coordinator/src/round/output_reg.rs` — `register_output_logic()` called from post_output

### Round State (structure changes)
- `coordinator/src/round/state.rs` — `RoundStateInner` struct (line 72) where `rsa_signing_key` lives and `rsa_signer` will be added
- `coordinator/src/round/manager.rs` — round creation logic where the signer should be parsed once

### RSA Blind Signing
- `coordinator/src/blind/rsa.rs` — `RsaBlindSigner` type definition and `from_der_secret_key` constructor

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- TOCTOU pattern at handlers.rs:76-87 + 120-129 — read-lock check, then write-lock re-check. Extend this for validate-then-lock.
- `AppState` struct at coordinator/src/api/mod.rs — holds `round: Arc<RwLock<RoundState>>`, `rpc: BitcoinRpc`, `ban_list`, `config`
- Ban check at handlers.rs:104-115 — already done outside write lock (good pattern to follow)

### Established Patterns
- Phase checks done under read lock first, then re-verified under write lock (T-04-01 TOCTOU prevention)
- `RoundStateInner` is Option<> inside RoundState — must check `guard.inner.is_some()` before accessing
- Zeroize on drop for sensitive data (rsa_signing_key, round_secret)

### Integration Points
- `register_input()` signature needs to change — currently takes `&mut RoundState` (implying caller holds write lock). Will need to split: RPC validation takes `&RoundState` or extracted params, state mutation takes `&mut RoundState`
- `post_output` has the same RSA deserialization pattern but does NOT do RPC, so only needs D-04/D-06 fix

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard refactoring patterns apply.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-coordinator-dos-hardening*
*Context gathered: 2026-04-10*
