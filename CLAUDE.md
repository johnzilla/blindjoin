## Skill routing

When the user's request matches an available skill, ALWAYS invoke it using the Skill
tool as your FIRST action. Do NOT answer directly, do NOT use other tools first.
The skill has specialized workflows that produce better results than ad-hoc answers.

Key routing rules:
- Product ideas, "is this worth building", brainstorming → invoke office-hours
- Bugs, errors, "why is this broken", 500 errors → invoke investigate
- Ship, deploy, push, create PR → invoke ship
- QA, test the site, find bugs → invoke qa
- Code review, check my diff → invoke review
- Update docs after shipping → invoke document-release
- Weekly retro → invoke retro
- Design system, brand → invoke design-consultation
- Visual audit, design polish → invoke design-review
- Architecture review → invoke plan-eng-review
- Save progress, checkpoint, resume → invoke checkpoint
- Code quality, health check → invoke health

<!-- GSD:project-start source:PROJECT.md -->
## Project

**blindjoin**

A standalone, open-source CoinJoin coordinator and client for Bitcoin signet/testnet. Uses RSA blind signatures (RFC 9474) to ensure the coordinator cannot link transaction inputs to outputs. Participants discover coordinators through Pubky's decentralized DHT (PKARR), and all protocol traffic flows over Tor hidden services. Ships as a Docker Compose stack that goes from zero to a working CoinJoin round in under five minutes on Bitcoin signet.

This is infrastructure, not a product. MIT licensed. No fees. No company. No terms of service.

**Core Value:** Anyone can run a CoinJoin coordinator that cryptographically cannot link inputs to outputs, and coordinators are disposable — discoverable and replaceable via DHT.

### Constraints

- **Cryptography**: No custom crypto — blind-rsa-signatures (jedisct1), rust-bitcoin, bdk, secp256k1 only
- **Network**: Tor-native in production; development/testing may use clearnet TCP
- **Scope**: Signet-first; mainnet is a config flag, not a code change
- **Privacy**: No PII logging; round state zeroed after broadcast
- **License**: MIT — public good, not a business
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## Recommended Stack
### Async Runtime
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| tokio | 1.51.x (LTS) | Async runtime, channels, timers | Only viable async runtime for this stack. Axum, arti-client, and BDK async all target tokio. 1.51.x is LTS until March 2027 — pin this for stability. |
### Core Bitcoin Libraries
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| bitcoin | 0.32.x | Primitives: transactions, scripts, PSBTs, addresses | The canonical rust-bitcoin crate. Everything else (BDK, bitcoincore-rpc replacement, bip322) depends on it. Use this version to keep dependency graph consistent. |
| bdk_wallet | 2.2.x | Client-side wallet: key management, UTXO selection, PSBT construction and signing | BDK went through a major rename (old `bdk` crate is deprecated, `bdk_wallet` is the current package). 2.2.0 released March 2026. Use the `bdk_wallet` crate, not the old `bdk` crate. The client needs descriptor-based key management and partial PSBT signing — exactly what BDK provides. Coordinator does NOT need BDK; it only assembles and broadcasts, not key management. |
| corepc-types | latest | Type-safe Bitcoin Core JSON-RPC types for coordinator | `bitcoincore-rpc` was archived November 2025. The rust-bitcoin team replaced it with `corepc`. Use `corepc-types` for the types (production-safe); the `corepc-client` is a blocking test client, not production-appropriate. For async RPC calls, drive JSON-RPC manually via `reqwest` + `corepc-types`. |
### Cryptography
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| blind-rsa-signatures | latest (jedisct1) | RFC 9474 RSA blind signatures — the core unlinkability primitive | The only RFC 9474-compliant Rust implementation. Written by jedisct1 (libsodium author), audited, production-grade. No alternative. Do not roll your own. |
| secp256k1 | (via rust-bitcoin) | ECDSA/Schnorr for UTXO ownership proofs | Pull through bitcoin/bdk — do not add a direct dependency unless you need low-level ops. |
| ring or openssl (via blind-rsa-signatures) | (transitive) | RSA key generation for coordinator signing key | `blind-rsa-signatures` handles this; do not pull ring/openssl directly. |
### UTXO Ownership Proofs (BIP-322)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| bip322 | 0.0.x (rust-bitcoin/bip322) | BIP-322 generic message signing for proving UTXO ownership | Official rust-bitcoin project implementation. Supports P2WPKH, P2TR, P2SH-P2WPKH. Coordinator uses this to verify client input ownership without revealing keys. This is a young crate (0.0.x) — pin the exact version and test carefully. |
### Tor Integration
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| arti-client | 2.x (matches Arti 2.2.0) | Native Tor hidden service (coordinator .onion) and Tor client (client's fresh circuits) | Arti 2.0.0 (February 2026) declared LTS for the 2.x branch and stabilized hidden service hosting. Arti 2.2.0 (April 2026) adds HTTP CONNECT by default. This eliminates the separate `tor` process dependency from Docker. Both coordinator (as HS server) and client (as Tor client switching circuits per phase) use this. |
| tor-hsservice | (via arti-client feature) | Onion service server-side implementation | Pull via `arti-client` with `onion-service-service` feature — do not add separately unless the API requires it. |
| arti-axum | 0.1.x | Glue: serve axum router over arti onion service | Community crate (jgraef/arti-axum) that bridges Axum's `serve` interface with arti's onion service listener. Saves significant glue code. Check for version compatibility with arti-client 2.x before pinning — this is a small community crate. |
### API Layer (Coordinator HTTP)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| axum | 0.8.x | HTTP API for coordinator round endpoints | The leading Rust web framework in 2026. Tokio-native, ergonomic, tower-compatible middleware. Coordinator exposes REST endpoints for: input registration, output registration, signing, blame. 0.8.x is current stable; 0.9 in development with breaking changes — stay on 0.8.x for this project. |
| tower | (via axum) | Middleware: rate limiting, timeouts, per-route logic | Tower comes with axum — use it for per-endpoint rate limiting (critical for abuse prevention) without adding a separate crate. |
| serde / serde_json | 1.x | JSON serialization for all API request/response types | Universal. No alternative considered. |
### Coordinator Discovery (PKARR)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| pkarr | latest (2.x) | Publish coordinator's .onion address + round parameters to Mainline DHT; client resolution | The Pubky project's Rust implementation. Coordinator publishes a signed DNS-like packet containing `.onion` address, denomination, round status. Client discovers coordinators without a hardcoded address. Crates.io page last updated February 2026. The `mainline` crate is the underlying DHT transport that pkarr wraps. |
### State Machine & Concurrency
- `tokio::sync::RwLock<RoundState>` — shared mutable round state, reader-biased
- `tokio::sync::broadcast` — notify all connected clients of phase transitions
- `tokio::time::interval` + `tokio::time::timeout` — phase deadline enforcement (INPUT_REG timer, SIGNING timeout)
- `Arc<Mutex<>>` sparingly — only where `RwLock` is insufficient
### Persistence
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| sqlx | 0.8.x | Async SQLite for coordinator: UTXO ban list, round history, in-progress round checkpoint | SQLx is async-native, tokio-compatible, and supports compile-time SQL verification. SQLite is sufficient: the coordinator is single-instance, ban data is small, and round state is mostly in-memory. No ORM needed — the schema is simple (3-4 tables). Use sqlx macros with `query!` for compile-time checked SQL. |
### CLI (Client)
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| clap | 4.x | Client CLI argument parsing | Ecosystem standard. Derive macros for subcommands. `clap` 4.x is mature and stable. |
| indicatif | 0.17.x | Progress display for multi-phase round participation | Multi-step interactive flows (input reg → blind → output reg → sign) benefit from phase progress display. Optional but improves UX significantly for the demo story. |
### Configuration
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| config | 0.14.x | Layered configuration: file + env vars + CLI overrides | Supports TOML (primary) with env var overlay. Coordinator config (denomination, round size, Bitcoin RPC URL) and client config (coordinator .onion, wallet descriptor) use this. |
| toml | (via config) | Config file format | Idiomatic Rust config format. |
### Observability
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| tracing | 0.1.x | Structured, async-aware logging | Tokio ecosystem standard. Span-based — captures async context. Critical constraint: zero PII fields. Use structured fields (`tracing::info!(round_id, phase, participant_count)`) not string interpolation. Never log IP addresses, amounts per participant, or input-output pairs. |
| tracing-subscriber | 0.3.x | Log formatting and filtering (RUST_LOG env var) | `EnvFilter` for `RUST_LOG`-based level control. `fmt` layer for human-readable output in dev; JSON layer optional for production. |
### Testing
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| bitcoincore-testing / regtest via corepc-node | latest | Spin up regtest Bitcoin Core for integration tests | `corepc-node` (from the same rust-bitcoin/corepc repo) manages test Bitcoin Core instances. Use signet for integration tests per spec, regtest for unit-level tests. |
| tokio::test | (via tokio) | Async test runtime | Use `#[tokio::test]` for all async unit and integration tests. |
| proptest | 1.x | Property-based testing for state machine transitions and crypto operations | Valuable for round state machine: generate arbitrary sequences of participant actions and verify invariants hold (e.g., coordinator never links inputs to outputs in any execution path). |
## Alternatives Considered
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Web framework | axum 0.8 | actix-web 4 | actix-web uses a separate actor runtime; tokio integration is messier. axum is native tokio. For a project already deep in tokio (arti, BDK), axum is the natural fit. |
| Bitcoin Core RPC | reqwest + corepc-types | `bitcoincore-rpc` crate | Archived November 2025. Dead project. |
| Database | sqlx + SQLite | PostgreSQL | No HA needed; single coordinator instance; SQLite is trivial to ship in Docker. |
| Blind signatures | blind-rsa-signatures | WabiSabi (zkSNACKs) | No production Rust WabiSabi implementation exists. Project explicitly out-of-scoped. |
| Tor | arti-client | Standalone `tor` binary via SOCKS5 | arti is native Rust, no separate process, LTS on 2.x. Docker Tor container only as SOCKS5 fallback for clients that can't link arti. |
| State machine crate | None (tokio primitives) | statig, smlang, rust-fsm | All add complexity. Round has 6 states and <10 transitions. A Tokio task + enum + match is cleaner and more inspectable. |
| ORM | sqlx raw queries | SeaORM, Diesel | 3-4 table schema doesn't warrant an ORM. Diesel's sync-first model is wrong for this stack. SeaORM 2.0 is good but overkill. |
| Config format | TOML via config crate | JSON, YAML | TOML is idiomatic Rust config. YAML has footguns (Norway problem). JSON lacks comments. |
| CLI framework | clap 4 | argh, lexopt | clap is the ecosystem standard with the richest feature set and subcommand support. |
## Version Summary (Cargo.toml reference)
# Async runtime
# Bitcoin
# corepc-types for coordinator RPC response deserialization
# Cryptography
# Tor
# arti-client version tracks arti 2.2.0 release tag; check crates.io for exact version
# API
# Discovery
# Persistence
# Observability
# Config
# CLI (client binary only)
## Sources
- [crates.io/crates/bitcoin](https://crates.io/crates/bitcoin) — rust-bitcoin 0.32.7 (August 2025 release) [MEDIUM confidence]
- [blog.torproject.org/arti_2_0_0_released](https://blog.torproject.org/arti_2_0_0_released/) — Arti 2.0.0 February 2026, LTS on 2.x branch [HIGH confidence]
- [blog.torproject.org/arti_2_2_0_released](https://blog.torproject.org/arti_2_2_0_released/) — Arti 2.2.0 released, HTTP CONNECT default [HIGH confidence]
- [crates.io/crates/bdk_wallet](https://crates.io/crates/bdk_wallet) — bdk_wallet 2.2.0 current (March 2026) [HIGH confidence]
- [github.com/rust-bitcoin/rust-bitcoincore-rpc](https://github.com/rust-bitcoin/rust-bitcoincore-rpc) — Archived November 2025 [HIGH confidence]
- [github.com/rust-bitcoin/corepc](https://github.com/rust-bitcoin/corepc) — Replacement for bitcoincore-rpc [HIGH confidence]
- [github.com/jedisct1/rust-blind-rsa-signatures](https://github.com/jedisct1/rust-blind-rsa-signatures) — RFC 9474 implementation [HIGH confidence]
- [crates.io/crates/blind-rsa-signatures](https://crates.io/crates/blind-rsa-signatures) — crates.io listing [MEDIUM confidence on version]
- [github.com/rust-bitcoin/bip322](https://github.com/rust-bitcoin/bip322) — BIP-322 rust-bitcoin implementation [MEDIUM confidence; 0.0.x stability concerns]
- [crates.io/crates/pkarr](https://crates.io/crates/pkarr) — PKARR Rust crate, updated February 2026 [MEDIUM confidence]
- [github.com/jgraef/arti-axum](https://github.com/jgraef/arti-axum) — arti-axum community crate [LOW-MEDIUM confidence; needs Sprint 0 validation]
- [tokio.rs](https://tokio.rs/) — Tokio 1.51.x LTS until March 2027 [HIGH confidence]
- [aarambhdevhub.medium.com — Rust Web Frameworks in 2026](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2) — axum 0.8.x recommended for 2026 [MEDIUM confidence]
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
