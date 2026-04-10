# Phase 1: Core Protocol - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-08
**Phase:** 01-core-protocol
**Areas discussed:** Wire protocol format, Coordinator startup, Client wallet UX, Round parameters

---

## Wire Protocol Format

| Option | Description | Selected |
|--------|-------------|----------|
| REST (spec as-is) | GET /info, POST /round/input, etc. Standard HTTP verbs + paths. Simple, debuggable with curl. | ✓ |
| JSON-RPC 2.0 | Single endpoint, method field in body. Matches Bitcoin Core's style. More boilerplate. | |
| You decide | Claude picks based on simplicity and ecosystem fit | |

**User's choice:** REST (spec as-is)
**Notes:** Clean and standard, debuggable with curl.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Structured JSON errors | {"error": {"code": "UTXO_SPENT", "message": "...", "round_id": "..."}} — machine-parseable | ✓ |
| HTTP status + message | 400/403/409 status codes with plain text body. Simpler, less structured. | |
| You decide | Claude picks based on what makes the client implementation cleanest | |

**User's choice:** Structured JSON errors
**Notes:** Machine-parseable error codes enable programmatic retry/failover logic.

---

## Coordinator Startup

| Option | Description | Selected |
|--------|-------------|----------|
| TOML file + env overrides | blindjoin.toml as primary config, BLINDJOIN_* env vars override any field. | ✓ |
| TOML file only | Config file only. Simpler, but less Docker-friendly. | |
| You decide | Claude picks the most practical approach | |

**User's choice:** TOML file + env overrides
**Notes:** Standard for Docker and bare-metal deployment.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, fail fast | Check bitcoind reachable + correct network + synced. Exit with clear error if any fail. | ✓ |
| Yes, warn only | Check and warn but start anyway. Useful if bitcoind is still syncing. | |
| You decide | Claude picks based on operational safety | |

**User's choice:** Yes, fail fast
**Notes:** Prevents starting a coordinator that can't validate UTXOs.

---

## Client Wallet UX

| Option | Description | Selected |
|--------|-------------|----------|
| Generate new wallet | blindjoin-cli init creates a new descriptor wallet with BIP-84 derivation. | |
| Import from descriptor | User provides an output descriptor string. Supports external wallets. | |
| Both | Generate by default, accept --descriptor flag for import. | ✓ |

**User's choice:** Both
**Notes:** Generate by default, import via --descriptor flag.

---

| Option | Description | Selected |
|--------|-------------|----------|
| Manual faucet | User visits a signet faucet website and sends coins to the client address. | ✓ |
| Built-in faucet | blindjoin-cli faucet requests coins from a known signet faucet API automatically. | |
| You decide | Claude picks based on simplicity for Sprint 1 | |

**User's choice:** Manual faucet
**Notes:** Standard approach, no external API dependency.

---

## Round Parameters

| Option | Description | Selected |
|--------|-------------|----------|
| Spec defaults | Use the values exactly as specified. 3 min participants, 60s/60s/30s timeouts. | ✓ |
| Dev-friendly defaults | Min 2 participants, shorter timeouts. Spec values as 'production' profile. | |
| You decide | Claude picks sensible defaults for signet development | |

**User's choice:** Spec defaults
**Notes:** All configurable via blindjoin.toml.

---

## Claude's Discretion

- Error code taxonomy
- Axum middleware configuration
- Internal data structures for round state
- Logging format and verbosity levels

## Deferred Ideas

None — discussion stayed within phase scope
