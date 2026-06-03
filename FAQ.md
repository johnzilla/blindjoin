## FAQ

<details>
<summary>What is BlindJoin?</summary>

BlindJoin is a standalone CoinJoin coordinator and client for Bitcoin Signet. It uses RSA blind signatures ([RFC 9474](https://www.rfc-editor.org/rfc/rfc9474.html)) so the coordinator cryptographically cannot link transaction inputs to outputs.

Coordinators are discoverable via [PKARR](https://github.com/pubky/pkarr) DHT, and all production traffic runs over Tor hidden services. MIT licensed. No fees. No company. No terms of service.

</details>

<details>
<summary>How does the blind signature protection work?</summary>

1. Participants register inputs and receive a **blind-signed** token from the coordinator.
2. Participants unblind the token and register their output on a fresh Tor circuit.
3. The coordinator sees inputs and outputs but cannot link them because of the blind signature scheme.

Each round uses an ephemeral RSA key whose lifetime is bounded by a Rust type signature (`Option<RsaBlindSigner>` on the round state, set to `None` at one FSM chokepoint — see [docs/AUDIT-CHARTER.md §5](docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window)) that triggers the `rsa` crate's `ZeroizeOnDrop` chain. The remainder of the round state is zeroized from memory by the same Drop sequence.

</details>

<details>
<summary>Is this safe to use with real Bitcoin?</summary>

**No — this is currently for Signet only.**

BlindJoin is designed and tested on Bitcoin Signet. Mainnet support is planned but not yet enabled. Always treat it as experimental software.

</details>

<details>
<summary>Why use Tor hidden services?</summary>

- Prevents the coordinator from seeing participant IP addresses.
- Client uses per-phase circuit isolation (input registration on one circuit, output on another) to further reduce correlation risk.

</details>

<details>
<summary>What is the role of the liquidity bot?</summary>

The included liquidity bot automatically joins rounds with signet coins to help reach the minimum participant count and improve the anonymity set during testing and early adoption.

</details>

<details>
<summary>Can I run my own coordinator?</summary>

Yes — easily.

- Use Docker Compose (recommended) for a full stack including bitcoind.
- Or build from source and run the coordinator binary with Tor mode enabled for production.

Coordinators automatically announce themselves via [PKARR](https://github.com/pubky/pkarr) DHT so clients can discover them.

</details>

<details>
<summary>How private is this compared to other CoinJoin implementations?</summary>

BlindJoin provides **stronger coordinator-side privacy** than most centralized mixers because the coordinator is cryptographically blinded from input-output linkage.

It does **not** protect against:
- On-chain analysis (common to all CoinJoin tools)
- Sybil attacks by the coordinator (mitigated by minimum participant requirements)
- Global passive adversaries observing the entire network

</details>

<details>
<summary>Do I need to run my own Bitcoin node?</summary>

Yes. The coordinator requires a trusted Bitcoin Core node (Signet by default) for UTXO validation, PSBT construction, and broadcasting.

</details>

<details>
<summary>How does BlindJoin handle abuse and flood traffic?</summary>

Each HTTP route on the coordinator is rate-limited via `tower_governor`: read endpoints (`/info`, `/round/tx`) accept up to 60 req/min; write endpoints (`/round/input`, `/round/output`, `/round/sign`) accept up to 30 req/min. Floods receive HTTP 429 with a `Retry-After` header and a JSON envelope `{"error":{"code":"RATE_LIMITED",...}}`. Handlers that stall past `request_timeout_secs` (default 30s) return HTTP 408. Concurrent Tor streams are bounded by a `tokio::sync::Semaphore` (default 256). All four knobs are operator-tunable in `coordinator.toml` and validated at startup.

Per-peer rate limiting is intentionally not used: every Tor hidden-service stream looks identical to the coordinator, so the rate limiter uses `GlobalKeyExtractor` and sybil resistance lives in BIP-322 ownership proofs and the per-round denomination instead. See the [README §Security Model](README.md#security-model) and `blindjoin-technical-spec.md` §"Availability Threat Model" for the full breakdown.

</details>

<details>
<summary>Is BlindJoin production-ready?</summary>

It is a solid v1.x implementation with good testing, security hardening, and Docker support. However, it is still experimental privacy software on Signet. Use with caution and audit the code before relying on it for high-value mixing.

The current pre-production gap is documented openly: see the draft [protocol specification](docs/PROTOCOL.md), and the open-source security posture below.

</details>

<details>
<summary>Is there a protocol specification I can review?</summary>

Yes — a BIP-style normative specification is being drafted at [docs/PROTOCOL.md](docs/PROTOCOL.md). It is explicitly marked as a draft and will be filled in over the course of the planned formal-spec milestone. Sections without `[TODO]` markers are normative as written. Review issues and PRs against the spec are welcome.

</details>

<details>
<summary>What's the security posture of the dependency tree?</summary>

- TLS is pure-Rust [rustls](https://github.com/rustls/rustls) end-to-end. The openssl crate chain is not pulled in.
- `cargo audit` runs on every PR and **blocks merge** on any advisory not declared in [`.cargo/audit.toml`](.cargo/audit.toml). Each ignore in that file carries a written rationale.
- `cargo clippy --workspace --all-targets -- -D warnings` runs on every PR and blocks merge on any lint, including in integration-test code — so future struct/API drift in the test scaffolding surfaces as a CI failure rather than silent rot.
- GitHub Actions are pinned to immutable commit SHAs; release archives include SHA-256 checksums.

</details>

<details>
<summary>Which input address types does BlindJoin support?</summary>

As of v1.4, BlindJoin accepts three input script types in a single round:

- **P2WPKH** (BIP-84 native segwit, addresses starting with `bc1q...` / `tb1q...`)
- **P2TR** (BIP-86 single-key Taproot, `bc1p...` / `tb1p...`)
- **P2SH-P2WPKH** (BIP-49 wrapped segwit, `3...` / `2...`)

Pick the type at wallet generation with `client --generate-wallet --type {p2wpkh|p2tr|p2sh-p2wpkh}` (default: `p2wpkh` for backwards compatibility).

Operators can run a single-script-type coordinator by disabling the others in `coordinator.toml` (`[bip] allow_p2tr = false`, etc.) — the supported set is advertised over the [PKARR](https://github.com/pubky/pkarr) DHT and on `/info`, so clients reject mismatched coordinators **before** opening a Tor circuit.

The coordinator derives the input's script type from the on-chain `script_pubkey` (not from any client-declared field) and cross-checks against the client's declaration; a malicious client cannot lie about its script type to bypass the per-script-type sighash verifier. See the README §Security Model "Multi-script script-type integrity" for the CRIT-01 invariant.

</details>

<details>
<summary>What are the next planned features?</summary>

- Mainnet support
- Multiple denominations per coordinator
- Improved blame / round recovery mechanisms
- GUI client
- More robust Sybil resistance
- P2WSH (multisig) BIP-322 support — currently P2WPKH, P2TR, and P2SH-P2WPKH are supported

</details>

<details>
<summary>How can I help?</summary>

- Star the repo
- Run a coordinator and share feedback
- Test the client with the liquidity bot
- Review the technical spec and security model
- Contribute code, documentation, or security review

</details>
