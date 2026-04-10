# Phase 7: Coordinator DoS Hardening - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-10
**Phase:** 07-coordinator-dos-hardening
**Areas discussed:** RPC refactor strategy, RSA key caching scope, Testing approach

---

## RPC Refactor Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Validate-then-lock | Do full RPC validation before acquiring write lock. Re-check phase + double-reg under write lock. | ✓ |
| Two-phase with read lock | Acquire read lock for phase check + config, release, do RPC, then write lock for mutation. | |
| You decide | Claude picks the cleanest approach | |

**User's choice:** Validate-then-lock
**Notes:** Simple and safe — the TOCTOU re-check pattern already exists in the codebase.

---

## RSA Key Caching Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Cache in RoundStateInner | Add parsed rsa_signer field alongside raw bytes. Set once at round creation. | ✓ |
| Cache in AppState | Store parsed signer outside RwLock. Less lock contention but more complex lifecycle. | |
| You decide | Claude picks based on existing architecture | |

**User's choice:** Cache in RoundStateInner
**Notes:** Keeps the signer co-located with the raw key bytes and the existing zeroize-on-drop lifecycle.

---

## Testing Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Unit tests only | Test lock ordering and signer reuse. Fast, no bitcoind needed. | ✓ |
| Unit + integration | Unit tests + concurrent registration integration test. Needs bitcoind. | |
| You decide | Claude picks appropriate test level | |

**User's choice:** Unit tests only
**Notes:** Integration tests not needed for this narrow refactor.

---

## Claude's Discretion

- Function signature changes for register_input split
- Whether to extract RPC validation into separate function
- RsaBlindSigner Clone handling

## Deferred Ideas

None
