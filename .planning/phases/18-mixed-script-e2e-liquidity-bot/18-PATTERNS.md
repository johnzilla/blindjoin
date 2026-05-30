# Phase 18: Mixed-Script E2E + Liquidity Bot — Pattern Map

**Generated:** 2026-05-30
**Source:** 18-CONTEXT.md (§Boundary-only changes, §Code anchors, §Implementation Decisions D-81..D-106) + 18-RESEARCH.md (§Architecture Patterns, §Decisions Q1..Q4, §R1..R7)
**Files analyzed:** 13 (11 CREATE/MODIFY + 2 invariant-gate read-only)
**Analogs found:** 11/13 (2 files — `v13_binary_compat.rs` external-binary driving and `README.md §Privacy Considerations` — have no direct in-repo precedent; partial analogs flagged below)

## File Inventory (Phase 18 creates or modifies)

| File | Role | Action | Closest Analog | Match Quality | LOC est |
|------|------|--------|----------------|---------------|---------|
| `tests/integration/mixed_script_e2e.rs` | Integration test (INTEG-01, request-response + broadcast assert) | CREATE | `tests/integration/full_round.rs::full_round_three_clients` (194-379) | exact (8-step structure mirror, 3 documented differences) | 200-300 |
| `tests/integration/bot_rotation.rs` | Integration test (INTEG-02, multi-run state) | CREATE | `tests/integration/full_round.rs::full_round_three_clients` (194-379) for spawn_coordinator + fund driving; `liquidity-bot/src/strategy.rs::tests` (39-101) for the JoinStrategy 5-test smoke pattern | role-match (no existing 3-run cycle test in repo) | 200-280 |
| `tests/integration/v13_binary_compat.rs` | Integration test (external-binary driver, request-response) | CREATE | `tests/integration/full_round.rs::full_round_three_clients` (194-379) for coordinator bring-up + mempool poll; **no in-repo precedent for `tokio::process::Command` external-binary pattern** | partial (mixed — coordinator side has analog, external-binary side does not) | 150-250 |
| `tests/integration/mod.rs` | Integration test root (module declarations + optional helper promotion) | MODIFY | self — existing `mod X;` block at lines 19-24; existing typed-funding smoke tests pattern at 825+ | exact | +3 mod lines; +0-200 LOC if `spawn_coordinator` promoted |
| `liquidity-bot/src/main.rs` | Bot runtime (env-var config, multi-script dispatch, counter file I/O, request-response) | MODIFY | self — current 209-LOC shape (env-var surface at 35-69, synthetic_info at 171-183, single-shot exit at 140-146) | exact (extending current file) | +120-180 |
| `liquidity-bot/src/strategy.rs` | Bot logic (new `RotationState` type + atomic counter-file I/O) | MODIFY | `liquidity-bot/src/strategy.rs::JoinStrategy` (9-37 + tests 39-101) for the side-by-side type pattern; **no in-repo atomic-write idiom** (BLAME-05 `append_ban_entry` at `coordinator/src/round/blame.rs:114-128` is APPEND-only, not the right idiom) — RESEARCH §Q4 recommends NEW idiom | role-match | +80-150 prod + ~60 LOC tests |
| `liquidity-bot/Cargo.toml` | Manifest (dev-dep addition per CD-31) | MODIFY | `coordinator/Cargo.toml:69` (workspace-pinned `tempfile`) | exact | +3 lines |
| `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt` | Pinned-SHA artifact (single-line text file) | CREATE | no in-repo precedent; pattern is documented in CONTEXT D-88 | partial | 2 lines |
| `docker/docker-compose.yml` | Service config (env vars + volume) | MODIFY | `docker/docker-compose.yml::coordinator` (37-76) for the env-var + named-volume pattern (`coordinator-data` at 60-61 + 105-106) | exact (mirror coordinator-data → bot-data) | +10-15 lines |
| `docker/Dockerfile` | Multi-stage build (bot-stage `mkdir -p /app/data`) | MODIFY | `docker/Dockerfile:23-27` (coordinator stage `RUN mkdir -p /app/keys /app/data`) | exact | +1 line |
| `.env.example` | Operator template (new bot env vars with comments) | MODIFY | self (per CONTEXT §Boundary-only changes) — no in-repo content inspected | partial (file structure not loaded) | +10-15 lines |
| `README.md` | Documentation (§"Privacy Considerations" prose insertion) | MODIFY | `.planning/PROJECT.md` ("infrastructure, not a product" tone) + `.planning/research/PITFALLS.md §V1.4-MOD-06 / §V1.4-MIN-02` (technical content) — **no in-repo §-disclaimer-style precedent** | partial | +18-22 lines |

### Read-only invariant-gate files (Phase 18 must NOT modify)

| File | Role | Why read-only | Lines Phase 18 patterns reference |
|------|------|----------------|-----------------------------------|
| `tests/integration/full_round.rs` | v1.3 invariant gate (8/8 must stay green) | Cross-phase invariant per Phase 14/15/16/17 carry-forward; REPAIR-01 lesson #4 | 49-59, 85-153, 158-177, 183-379 (all read-only references) |
| `coordinator/src/api/handlers.rs:296-440` | `post_output` handler | Phase 18 verifies the absence of output-script-type runtime check per D-90; zero modifications | 296-440 (read-only verify) |

---

## Per-File Pattern Excerpts

### `tests/integration/mixed_script_e2e.rs` (CREATE — INTEG-01 acceptance gate)

**Analog:** `tests/integration/full_round.rs::full_round_three_clients` (194-379)

**Load-bearing idioms (the integration test contract):**
- `let exe = require_bitcoind!();` as FIRST line — the `match { None => return }` expansion `return`s from the calling test fn to skip cleanly when bitcoind is absent (the macro-form is load-bearing per `tests/integration/mod.rs:89-93`; a plain function would have to `panic!` or `std::process::exit` which abort the whole binary).
- `let (bitcoind_guard, setup) = crate::fund_regtest(exe).await; let _bitcoind_guard = bitcoind_guard;` — guard MUST stay in scope for the test's full duration; RAII `Drop` impl calls `n.stop()` on tokio blocking pool then falls back to corepc-node `Drop` SIGKILL (load-bearing per `tests/integration/mod.rs:130-149`).
- `spawn_coordinator(setup.rpc_url, setup.rpc_user, setup.rpc_pass).await` returns `(url, tempfile::TempDir)` — caller MUST bind tempdir to `let _tmp_dir` so the ban-file parent directory survives (WR-06 invariant per `tests/integration/full_round.rs:81-84`).
- `tokio::spawn` 3 concurrent client tasks; each awaits `handle.await.expect("client task panicked")` to surface task failures (avoids the silent-await-drop pitfall).
- `get_raw_mempool` polling: 10s deadline, 100ms cadence, `spawn_blocking` per iteration with cloned RPC creds (the spawn_blocking closure takes 'static + Send so the loop owns the originals); panic on deadline miss with a diagnostic naming the broadcast path.
- `get_raw_transaction_verbose(txid).outputs[i].value` is `f64 BTC` — convert via `(value * 100_000_000.0).round() as u64` (existing pattern at `tests/integration/full_round.rs:360-362`).

**Excerpt — the canonical 8-step structure (full_round.rs:194-326):**

```rust
#[tokio::test]
async fn full_round_three_clients() {
    // Step 1: skip gracefully if bitcoind missing (local-dev), panic in CI
    let exe = require_bitcoind!();

    // Steps 2-4: bring up regtest bitcoind + fund 3 UTXOs (RAII guard)
    let (bitcoind_guard, setup) = crate::fund_regtest(exe).await;
    let _bitcoind_guard = bitcoind_guard;

    let denomination: u64 = 100_000;

    // Step 5: spawn coordinator in-process (port 0, tempdir ban-file)
    let (coordinator_url, _tmp_dir) = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;

    // Step 6: 3 concurrent client tasks (tokio::spawn × 3)
    let handles: Vec<_> = test_wifs.iter().enumerate().map(|(i, wif)| {
        let url = coordinator_url.clone();
        let wif = wif.to_string();
        let (utxo_str, _utxo_value) = setup.utxos[i].clone();
        tokio::spawn(async move {
            let wallet = ClientWallet::from_wif(&wif, &utxo_str, Network::Regtest).expect("...");
            let coordinator_client = CoordinatorClient::new(url);
            let info = coordinator_client.poll_until_phase("input_reg", 100, Duration::from_secs(600)).await.expect("...");
            let reg = round::input::register_input(&coordinator_client, &wallet, &info, &v14_p2wpkh_coordinator_info()).await.expect("...");
            // ... poll output_reg → register_output → poll signing → verify_and_sign ...
        })
    }).collect();
    for handle in handles { handle.await.expect("client task panicked"); }

    // Step 7: poll mempool (10s deadline, 100ms cadence)
    let mempool_txids: Vec<String> = {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let txids: Vec<String> = tokio::task::spawn_blocking(move || {
                let auth = Auth::UserPass(rpc_user_c, rpc_pass_c);
                let client = corepc_node::Client::new_with_auth(&rpc_url_c, auth).expect("...");
                client.get_raw_mempool().expect("...").0
            }).await.expect("...");
            if !txids.is_empty() { break txids; }
            if tokio::time::Instant::now() >= deadline { panic!("CoinJoin tx never appeared..."); }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    // Step 8: assert denom_output_count == 3
}
```

**Phase 18 differences (3 of them, per RESEARCH §"Pattern 1"):**

1. **Step 2-4 funding swap (B1.b path per CONTEXT D-83 + RESEARCH Q1):**
   - P2WPKH client: keep WIF path via `crate::fund_regtest_typed(exe, &[(ScriptType::P2wpkh, 1)])` → 1 `TypedUtxoHandle` → `BdkClientWallet::from_wif(wif_from_secret_key, &outpoint_str, Network::Regtest)`.
   - P2TR + P2SH-P2WPKH clients: B1.b descriptor-wallet-driven funding. `BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Regtest, ScriptType::P2tr)` produces a wallet whose `coinjoin_output_address()` is `peek_address(External, 0)`. Fund THAT address via bitcoind RPC `send_to_address`, walk `get_raw_transaction_verbose` outputs to discover vout by SPK match, then assign `wallet.utxo_outpoint = OutPoint::new(funding_txid, vout)`. The `utxo_outpoint` field is `pub` at `client/src/wallet.rs:52` (load-bearing post-construction override; confirmed structurally available per RESEARCH §Q1).
   - **No structural blocker** — `BdkClientWallet::generate` signature is `(utxo_outpoint_str: &str, network: Network, script_type: ScriptType) -> Result<Self>` at `client/src/wallet.rs:209-212`.

2. **Step 5 synthetic CoordinatorInfo generalised (per CONTEXT D-85):** Replace `v14_p2wpkh_coordinator_info()` with a parameterized factory `v14_coordinator_info(script_type)`. Each client passes its OWN-typed CoordinatorInfo to `register_input`:
   ```rust
   fn v14_coordinator_info(st: ScriptType) -> CoordinatorInfo {
       CoordinatorInfo {
           coordinator_url: String::new(),
           capabilities: CoordinatorCapabilities {
               record_version: "manual".to_string(),
               is_legacy: false,
               supported_script_types: vec![st],
               output_script_type: st,
           },
       }
   }
   ```
   Both `supported_script_types` AND `output_script_type` MUST equal the wallet's own type (else Phase 17 D-76 `DiscoveryError::UnsupportedOutputScriptType` fires — RESEARCH Pitfall 2).

3. **Step 8 post-broadcast assertion extended (per CONTEXT D-104 + CD-30):** Keep `denom_output_count == 3` assertion (existing pattern at full_round.rs:369-373). ADD input script-type set-equality:
   ```rust
   let input_script_types: HashSet<ScriptType> = tx.inputs.iter().map(|inp| {
       let prev_txid = bitcoin::Txid::from_str(&inp.txid).unwrap();
       let prev_tx = rpc.get_raw_transaction_verbose(prev_txid).unwrap();
       let prev_vout = &prev_tx.outputs[inp.vout as usize];
       let spk_bytes = hex::decode(&prev_vout.script_pubkey.hex).unwrap();
       let spk = bitcoin::ScriptBuf::from_bytes(spk_bytes);
       shared::bip322::detect_script_type(&spk).expect("regtest funding SPK is one of P2WPKH/P2TR/P2SH-P2WPKH")
   }).collect();
   assert_eq!(
       input_script_types,
       HashSet::from([ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh]),
       "broadcast tx must contain exactly one input of each script type; got: {input_script_types:?}"
   );
   ```
   CD-30: re-query bitcoind for prevout SPKs (NOT witness-byte inspection — bdk_wallet may change finalisation byte form).

---

### `tests/integration/bot_rotation.rs` (CREATE — INTEG-02 integration test, per CD-26)

**Analogs (two-layer):**
- `tests/integration/full_round.rs::full_round_three_clients` (194-379) — for `spawn_coordinator` + per-test bitcoind isolation + the input-output-sign flow each bot run drives.
- `liquidity-bot/src/strategy.rs::tests` (39-101) — for the smoke-test pattern of asserting strategy state across multiple invocations (current 5-test pattern; Phase 18 extends with rotation).

**Load-bearing idioms:**
- Each of the 3 bot runs needs its OWN typed UTXO. Use `fund_regtest_typed(exe, &[(P2wpkh, 1), (P2tr, 1), (P2shP2wpkh, 1)])` once at test fn start, then drive runs sequentially against the same in-process coordinator (the coordinator's `min_participants = 3` default means each "round" needs 3 client tasks total — see RESEARCH §Test Strategy 18-02).
- Counter file in `tempfile::tempdir().path().join("bot_round_counter")` — hermetic per test, NEVER touches `/app/data` (bot_rotation runs OUT-OF-DOCKER).
- Pitfall 4 mitigation: extract bot main-loop into `pub async fn liquidity_bot::run(config: BotConfig) -> Result<()>`. This requires adding `[lib]` to `liquidity-bot/Cargo.toml` (analogous to `coordinator/Cargo.toml` lib declaration; current bot has only `[[bin]]` at lines 6-8).

**Excerpt — the `spawn_coordinator` reuse pattern (full_round.rs:85-153):**

```rust
async fn spawn_coordinator(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
) -> (String, tempfile::TempDir) {
    // Bind to port 0 (OS assigns ephemeral) + keep the listener open
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let listen_addr = addr.to_string();

    // WR-06: per-test temp dir so parallel tests cannot race the ban file
    let tmp = tempfile::tempdir().expect("create temp dir");
    let ban_file_path = tmp.path().join("ban_list.jsonl").to_string_lossy().into_owned();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: rpc_url.clone(),
            // ...
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000,
            min_participants: 3,
            // ...
        },
        bip: coordinator::config::BipConfig::default(),  // all-allowed + P2WPKH-output
    });
    // ... build router, spawn axum::serve(listener, app) ...
    (format!("http://{}", addr), tmp)
}
```

**Phase 18 18-02 plan recommendation (per RESEARCH §R1):**

PROMOTE `spawn_coordinator` + `wait_for_coordinator` + `v14_p2wpkh_coordinator_info` + `build_input_reg_round_state` from `full_round.rs` to `tests/integration/mod.rs`. Rationale: visibility — current `async fn spawn_coordinator(...)` at full_round.rs:85 lacks `pub`/`pub(crate)`, so `mixed_script_e2e.rs` cannot import via `use crate::full_round::spawn_coordinator`. Two options:
- **Add `pub(crate)`** to each definition (4-keyword add to `full_round.rs` — minimal touch, but breaks the "zero-touch" framing of the cross-phase invariant).
- **Promote bodies to `mod.rs`** (cleaner; full_round.rs gets a `use crate::{spawn_coordinator, ...};` line — still a single-line touch, but architecturally crisper).

CONTEXT/RESEARCH recommend promotion; plan-phase decides.

---

### `tests/integration/v13_binary_compat.rs` (CREATE — gated behind `--features v13-binary-compat` per CD-32)

**Analogs (partial — mixed match):**
- `tests/integration/full_round.rs::full_round_three_clients` (194-379) — for the in-process coordinator + regtest bitcoind + 3-client min_participants drive.
- **No in-repo precedent for `tokio::process::Command::new` external-binary driver pattern.** Closest stdlib pattern: `tokio::task::spawn_blocking` for sync RPC work at `tests/integration/full_round.rs:302-311`, but spawning an external binary requires `tokio::process::Command::new(...).args(...).status().await`.

**Load-bearing idioms (RESEARCH §Q2 + §Pitfall 5):**
- Pinned SHA at `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt` line 1 = `05f21438a7072987773bfe2eafaac5c51c68c61a` (40-char hex). Line 2 (optional): commit subject `docs(15): create phase plan`. Resolution verified via `git log --first-parent --oneline 622ccf0^ -1` returning `05f21438 docs(15): create phase plan`.
- `git worktree add /tmp/blindjoin-v13-<sha8> <full_sha>` is IDEMPOTENT — check if path exists before calling. Subsequent `cargo build` invocations are ~1-2s via cargo's incremental cache.
- `cargo build --release --bin client --manifest-path /tmp/blindjoin-v13-<sha8>/client/Cargo.toml` (~30-45s first-run; the Cargo.toml at SHA 05f21438 is workspace-deps-byte-identical to HEAD per RESEARCH §Q2).
- v1.3 binary invocation: `--coordinator-url http://<127.0.0.1:port> --utxo <txid:vout> --utxo-wif <wif> --network signet` (no `--use-tor`, no `--pkarr-pubkey`; v1.3 main.rs supports the direct-URL path).
- 3-client min_participants requirement: drive 1 v1.3 binary client + 2 v1.4 in-process clients in parallel (RESEARCH §Test Strategy "REFINED" note — the coordinator's `min_participants = 3` default at full_round.rs:117 means a 1-of-1 sub-round won't broadcast).
- Test gated behind `#[cfg(feature = "v13-binary-compat")]` per CD-32. Requires adding `[features] v13-binary-compat = []` to `coordinator/Cargo.toml` (RESEARCH §R7).

**Excerpt — the in-process coordinator bring-up pattern (carries verbatim from full_round.rs):**

```rust
let exe = require_bitcoind!();
let (bitcoind_guard, _setup) = crate::bootstrap_regtest_bitcoind(exe).await;
let _bitcoind_guard = bitcoind_guard;

let (coordinator_url, _tmp_dir) = spawn_coordinator(...).await;
wait_for_coordinator(&coordinator_url).await;
```

**Excerpt — the external-binary driver pattern (NEW, no in-repo precedent; recipe from RESEARCH §Q2):**

```rust
use std::process::Stdio;

let sha = std::fs::read_to_string(".planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt")
    .expect("v13_pinned_sha.txt present")
    .lines().next().expect("SHA on line 1").to_string();
let worktree = format!("/tmp/blindjoin-v13-{}", &sha[..8]);

// Idempotent worktree add — check if exists first
if !std::path::Path::new(&worktree).exists() {
    std::process::Command::new("git").args(["worktree", "add", &worktree, &sha])
        .status().expect("git worktree add").success();
}

// Idempotent build — cargo's incremental cache handles re-runs
std::process::Command::new("cargo")
    .args(["build", "--release", "--bin", "client", "--manifest-path", &format!("{}/client/Cargo.toml", worktree)])
    .status().expect("cargo build v1.3").success();

let v13_bin = format!("{}/target/release/client", worktree);

// Drive the v1.3 binary as one of 3 concurrent participants
let v13_handle = tokio::spawn(async move {
    tokio::process::Command::new(&v13_bin)
        .args(["--coordinator-url", &url, "--utxo", &utxo_str, "--utxo-wif", &wif, "--network", "signet"])
        .stdout(Stdio::inherit()).stderr(Stdio::inherit())
        .status().await.expect("v1.3 client exit")
        .success()
});
```

---

### `tests/integration/mod.rs` (MODIFY)

**Analog:** self — existing `mod X;` block at lines 19-24 (alphabetically sorted).

**Load-bearing idioms:**
- Each test file is declared via `mod X;` (private to the integration test binary crate). Phase 18 adds 3 new declarations, alphabetically sorted:
  - `mod bot_rotation;` inserted between `mod ban_list_persistence;` (19) and `mod full_round;` (20).
  - `mod mixed_script_e2e;` inserted between `mod full_round;` (20) and `mod multi_script_client;` (21).
  - `mod v13_binary_compat;` appended at end (alphabetically last), gated behind `#[cfg(feature = "v13-binary-compat")]` per CD-32.
- Optional: PROMOTE `spawn_coordinator` + `wait_for_coordinator` + `v14_p2wpkh_coordinator_info` + `build_input_reg_round_state` from `full_round.rs` per RESEARCH §R1 (visibility constraint — current definitions lack `pub(crate)`).
- The `[[test]] name = "integration" path = "../tests/integration/mod.rs"` at `coordinator/Cargo.toml:71-73` picks up any `mod X;` declaration — no `Cargo.toml` test-target changes needed (RESEARCH §R7).

**Excerpt — current declaration block (mod.rs:19-24):**

```rust
mod ban_list_persistence;
mod full_round;
mod multi_script_client;
mod multi_script_validate;
mod rate_limiting;
mod round_bootstrap;
```

**Phase 18 target state:**

```rust
mod ban_list_persistence;
mod bot_rotation;          // NEW — 18-02
mod full_round;
mod mixed_script_e2e;      // NEW — 18-01
mod multi_script_client;
mod multi_script_validate;
mod rate_limiting;
mod round_bootstrap;
#[cfg(feature = "v13-binary-compat")]
mod v13_binary_compat;     // NEW — 18-03 (opt-in)
```

---

### `liquidity-bot/src/main.rs` (MODIFY)

**Analog (env-var-driven config):** self — current 209-LOC shape; the env-var parsing pattern at lines 35-69 with `std::env::var(...).context(...)?` for required vars and `.unwrap_or_else(|_| "default".to_string())` for optional vars.

**Analog (synthetic_info pattern that Phase 18 extends):** `liquidity-bot/src/main.rs:171-183` (the existing synthetic CoordinatorInfo construction that goes into `register_input`).

**Load-bearing idioms:**
- Single-underscore env-var naming convention (`BLINDJOIN_X` NOT `BLINDJOIN__X__Y`) — diverges from coordinator's double-underscore hierarchical scheme; locked by Phase 17 CD-22 + CONTEXT D-93.
- Fail-fast at startup with operator-facing error messages naming the missing/malformed env var (mirrors the existing safety guard at lines 43-49 for `BLINDJOIN_NETWORK != "signet"`).
- Single-shot pattern: bot calls `participate_in_round(...).await` once, on success calls `return Ok(())` (lines 140-146). Phase 18 preserves this — counter bump happens BEFORE the return (per CONTEXT D-94: "increments + atomically writes the counter file ONLY on successful round completion").

**Excerpt — current env-var parsing pattern (main.rs:35-69) Phase 18 extends:**

```rust
let coordinator_url = std::env::var("BLINDJOIN_COORDINATOR_URL")
    .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

let network_str = std::env::var("BLINDJOIN_NETWORK")
    .unwrap_or_else(|_| "signet".to_string());

// SAFETY GUARD: refuse to run on non-signet.
if network_str != "signet" {
    bail!(
        "Liquidity bot refuses to start: BLINDJOIN_NETWORK='{}' is not 'signet'. \
         The bot is a signet-only testing tool.",
        network_str
    );
}
let network = bitcoin::Network::Signet;

let utxo = std::env::var("BLINDJOIN_UTXO")
    .context("BLINDJOIN_UTXO env var required (format: txid:vout)")?;
// ...
let utxo_wif = std::env::var("BLINDJOIN_UTXO_WIF")
    .context("BLINDJOIN_UTXO_WIF env var required")?;
```

**Excerpt — current synthetic_info pattern (main.rs:171-183) Phase 18 extends with rotation:**

```rust
let synthetic_info = client::discover::CoordinatorInfo {
    coordinator_url: String::new(),
    capabilities: client::discover::CoordinatorCapabilities {
        record_version: "manual".to_string(),
        is_legacy: false,
        supported_script_types: vec![
            shared::bip322::ScriptType::P2wpkh,
            shared::bip322::ScriptType::P2tr,
            shared::bip322::ScriptType::P2shP2wpkh,
        ],
        output_script_type: wallet.script_type(),
    },
};
```

**Phase 18 changes (per CONTEXT D-92..D-98 + RESEARCH §Q3):**

1. Parse `BLINDJOIN_BOT_SCRIPT_TYPES` (CSV; default `"p2wpkh"`) via `s.split(',').map(str::trim).map(client::config::parse_script_type).collect::<Result<Vec<_>, _>>()`. Reject empty + duplicates per CONTEXT §Specifics.
2. Read rotation counter from `BLINDJOIN_BOT_COUNTER_FILE` (default `/app/data/bot_round_counter` per CD-28).
3. Compute `script_type = enabled_types[counter % enabled_types.len()]`.
4. Per-type wallet construction:
   - P2WPKH: `BdkClientWallet::from_wif(wif, &utxo, network)` — legacy WIF path.
   - P2TR + P2SH-P2WPKH: `BdkClientWallet::from_descriptor(external_desc, &utxo, &utxo_address, network, script_type)` — 5-arg signature confirmed at `client/src/wallet.rs:135-141`. Auto-derives internal descriptor (no separate env var needed — RESEARCH §Q3).
5. Extend `synthetic_info.supported_script_types` to match the rotated-to type (not the hardcoded 3-element vec) AND `output_script_type = wallet.script_type()`.
6. ON SUCCESS (before `return Ok(())` at line 145): bump counter, atomic-write counter file (idiom in `strategy.rs`).

---

### `liquidity-bot/src/strategy.rs` (MODIFY — new `RotationState` type per CD-27)

**Analog (side-by-side type pattern):** self — current `JoinStrategy` struct + `impl` + `#[cfg(test)] mod tests` (lines 9-101).

**Analog for atomic file write idiom:** **NONE in repo.** `coordinator/src/round/blame.rs::append_ban_entry` (114-128) uses `std::fs::OpenOptions::create + append + writeln!` which is APPEND-only — wrong shape for the bot's OVERWRITE counter file. RESEARCH §Q4 prescribes a NEW idiom: `tokio::fs::write` to `${path}.tmp` + `tokio::fs::rename` to `${path}`.

**Excerpt — current strategy.rs side-by-side type + tests pattern (lines 9-37 + 39-101):**

```rust
pub struct JoinStrategy {
    pub target_denomination_sats: u64,
    pub max_consecutive_failures: u32,
    pub join_threshold: u32,
    pub poll_interval_secs: u64,
}

impl JoinStrategy {
    pub fn new(target_denomination_sats: u64) -> Self { /* ... */ }
    pub fn should_join(&self, info: &InfoResponse) -> bool { /* ... */ }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_info(round_state: &str, denomination_sats: u64, participants_registered: u32) -> InfoResponse { /* fixture */ }

    #[test]
    fn test_should_join_true_when_input_reg_and_denomination_matches() { /* ... */ }
    // ... 4 more tests ...
}
```

**Excerpt — BLAME-05 append-only ban write (coordinator/src/round/blame.rs:114-128) — THIS IS THE WRONG IDIOM for the bot counter file:**

```rust
pub fn append_ban_entry(path: &str, utxo_str: &str, entry: &BanEntry) -> std::io::Result<()> {
    use std::io::Write;
    let record = BanRecord { /* ... */ };
    let line = serde_json::to_string(&record).map_err(std::io::Error::other)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)
}
```

This is append-only + sync stdio (not atomic-replace + not async). The bot needs the OPPOSITE shape.

**Excerpt — RECOMMENDED atomic-write idiom (RESEARCH §Q4; NEW pattern, no in-repo precedent):**

```rust
async fn write_counter_atomic(path: &Path, counter: u64) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, format!("{}\n", counter)).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

async fn read_counter(path: &Path) -> anyhow::Result<u64> {
    match tokio::fs::read_to_string(path).await {
        Ok(s) => s.trim().parse::<u64>().map_err(|e| anyhow::anyhow!(
            "BLINDJOIN_BOT_COUNTER_FILE = '{}' contains malformed counter (line 1): '{}' ({e})",
            path.display(), s.trim()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(anyhow::anyhow!("BLINDJOIN_BOT_COUNTER_FILE read failed: {e}")),
    }
}
```

**Why `tokio::fs::*` over stdlib (RESEARCH §Q4):**
- Bot is already async (`#[tokio::main]` + `tokio::time::sleep` interleavings at main.rs:99). `tokio::fs` keeps I/O on the runtime without blocking.
- Avoids adding `tempfile` as runtime dep (CD-31 keeps it dev-only).
- `rename(2)` is atomic within a single filesystem on Linux (POSIX); `/app/data` is a single tmpfs/volume.

**Phase 18 strategy.rs target state — new `RotationState` type alongside existing `JoinStrategy`:**

```rust
pub struct RotationState {
    pub counter_file_path: PathBuf,
    pub enabled_types: Vec<ScriptType>,
}

impl RotationState {
    pub async fn pick_script_type(&self) -> Result<ScriptType> { /* read counter, mod len, return */ }
    pub async fn bump_counter(&self) -> Result<()> { /* read, +1, atomic-write */ }
}

#[cfg(test)]
mod rotation_tests {
    // Per CONTEXT D-101 / RESEARCH §Test Strategy 18-02:
    //   rotation_state_round_robin_advances_counter
    //   rotation_state_single_type_does_not_rotate
    //   rotation_state_empty_enabled_returns_err
    //   rotation_state_counter_file_roundtrip
    //   rotation_state_atomic_write_via_tmp_then_rename  (uses tempfile::tempdir)
}
```

---

### `liquidity-bot/Cargo.toml` (MODIFY — add `tempfile` to `[dev-dependencies]` per CD-31)

**Analog:** `coordinator/Cargo.toml:69` (workspace-pinned `tempfile = { workspace = true }`).

**Excerpt — current liquidity-bot/Cargo.toml:**

```toml
[package]
name = "liquidity-bot"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "liquidity-bot"
path = "src/main.rs"

[dependencies]
client = { path = "../client" }
shared = { path = "../shared" }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
bitcoin = { workspace = true }
```

**Phase 18 target additions:**

```toml
# Add [lib] section (Pitfall 4 — enables bot main-loop extraction for integration tests)
[lib]
name = "liquidity_bot"
path = "src/lib.rs"

[dev-dependencies]
tempfile = { workspace = true }   # CD-31 — counter-file unit tests + bot_rotation hermetic tempdir
```

---

### `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt` (CREATE)

**Analog:** no in-repo precedent — first pinned-SHA artifact in the project. Pattern is fully specified in CONTEXT D-88 + RESEARCH §Q2.

**Content (exactly per RESEARCH §Q2; SHA verified via `git log --first-parent --oneline 622ccf0^ -1`):**

```
05f21438a7072987773bfe2eafaac5c51c68c61a
docs(15): create phase plan
```

Line 1: 40-char SHA (load-bearing — `v13_binary_compat.rs` reads `.lines().next()`).
Line 2: optional human-readable commit subject for triage.

**Verification result:** confirmed via `git log --first-parent --oneline 622ccf0^ -1 --format="%H %s"` returning `05f21438a7072987773bfe2eafaac5c51c68c61a docs(15): create phase plan`. Workspace `Cargo.toml` diff `05f21438..HEAD` is empty (deps byte-identical at workspace level).

---

### `docker/docker-compose.yml` (MODIFY)

**Analog:** `docker/docker-compose.yml::coordinator` block at lines 37-76 (env-var + volume + healthcheck pattern). Specifically the `coordinator-data` named volume at lines 60-61 (mount) + 105-106 (declaration).

**Load-bearing idioms:**
- `${VAR}` substitution from `.env` file (current pattern at lines 88-90 for `BOT_UTXO`/`BOT_UTXO_VALUE_SATS`/`BOT_WIF`).
- Named volume declaration at the bottom of file (lines 99-106) with optional inline comment documenting persistence intent.
- Mount syntax: `- coordinator-data:/app/data` (lines 60-61).

**Excerpt — current coordinator volume + env pattern (lines 50-61 + 99-106):**

```yaml
  coordinator:
    environment:
      # ...
      BLINDJOIN__COORDINATOR__BAN_FILE_PATH: "/app/data/ban_list.jsonl"
      # ...
    volumes:
      - coordinator-keys:/app/keys
      # Named volume persists the append-only ban file (BLAME-05) across restarts.
      - coordinator-data:/app/data

volumes:
  bitcoin-data:
  coordinator-keys:
  coordinator-data:
    # Stores the append-only ban file (BLAME-05). Survives container restarts.
```

**Phase 18 target additions to `liquidity-bot` service (lines 78-97) + volumes block:**

```yaml
  liquidity-bot:
    environment:
      # ... existing env vars ...
      BLINDJOIN_BOT_SCRIPT_TYPES: "${BOT_SCRIPT_TYPES:-p2wpkh}"   # CSV; default = v1.3 backwards compat
      BLINDJOIN_BOT_P2WPKH_UTXO: "${BOT_P2WPKH_UTXO:-}"
      BLINDJOIN_BOT_P2WPKH_WIF: "${BOT_P2WPKH_WIF:-}"
      BLINDJOIN_BOT_P2TR_UTXO: "${BOT_P2TR_UTXO:-}"
      BLINDJOIN_BOT_P2TR_DESCRIPTOR: "${BOT_P2TR_DESCRIPTOR:-}"
      BLINDJOIN_BOT_P2TR_UTXO_ADDRESS: "${BOT_P2TR_UTXO_ADDRESS:-}"
      BLINDJOIN_BOT_P2SH_P2WPKH_UTXO: "${BOT_P2SH_P2WPKH_UTXO:-}"
      BLINDJOIN_BOT_P2SH_P2WPKH_DESCRIPTOR: "${BOT_P2SH_P2WPKH_DESCRIPTOR:-}"
      BLINDJOIN_BOT_P2SH_P2WPKH_UTXO_ADDRESS: "${BOT_P2SH_P2WPKH_UTXO_ADDRESS:-}"
      BLINDJOIN_BOT_COUNTER_FILE: "/app/data/bot_round_counter"
    volumes:
      - bot-data:/app/data   # NEW — analogous to coordinator-data

volumes:
  # ... existing entries ...
  bot-data:
    # Stores the bot rotation counter (BLINDJOIN_BOT_COUNTER_FILE). Survives restarts.
```

---

### `docker/Dockerfile` (MODIFY)

**Analog:** `docker/Dockerfile:23-27` (coordinator stage `RUN mkdir -p /app/keys /app/data`). The bot stage at lines 34-37 currently lacks the mkdir.

**Excerpt — current coordinator stage (analog):**

```dockerfile
FROM runtime-base AS coordinator
WORKDIR /app
RUN mkdir -p /app/keys /app/data
COPY --from=builder /app/target/release/coordinator /usr/local/bin/coordinator
ENTRYPOINT ["/usr/local/bin/coordinator"]
```

**Phase 18 target — extend bot stage to mirror coordinator:**

```dockerfile
FROM runtime-base AS liquidity-bot
WORKDIR /app
RUN mkdir -p /app/data                                                    # NEW (Pitfall 6 fix)
COPY --from=builder /app/target/release/liquidity-bot /usr/local/bin/liquidity-bot
ENTRYPOINT ["/usr/local/bin/liquidity-bot"]
```

---

### `README.md` (MODIFY — §"Privacy Considerations" insertion)

**Analog:** no in-repo precedent for §-disclaimer-style sections. Closest tonal anchor: `.planning/PROJECT.md` ("infrastructure, not a product" framing; MIT public good). Technical content anchor: `.planning/research/PITFALLS.md §V1.4-MOD-06` (chain-analysis fingerprint) + `.planning/research/PITFALLS.md §V1.4-MIN-02` (uniform-script bot fingerprint mitigation).

**Load-bearing idioms (per CONTEXT D-106 + RESEARCH §R5):**
- Plain language; no scary uppercase; no marketing claims.
- ~2 paragraphs (~200 words total).
- Level-2 heading `## Privacy Considerations` to match the rest of the README's section hierarchy.
- Insertion point: after "Quick Start (Docker)" section (ends line 42 with signet faucet URL), BEFORE "Build from Source" (starts line 44).

**Excerpt — exact prose per CONTEXT D-106 + RESEARCH §R5 verbatim:**

```markdown
## Privacy Considerations

blindjoin accepts mixed input script types (P2WPKH, P2TR, P2SH-P2WPKH) in a
single round. This maximizes the anonymity set across address types but
creates a chain-analysis signal: a CoinJoin transaction with a wildly
heterogeneous input set is visually distinguishable from a uniform-script
CoinJoin. Privacy-sensitive users who require uniform-script rounds can run
a dedicated coordinator with a single `allow_*` flag enabled.

The bundled liquidity bot rotates the script type it submits across rounds.
This prevents the bot's UTXOs from forming a uniform-script-type fingerprint
(which would otherwise identify the bot's participation by cross-round
correlation). Rotation is round-robin across the operator-configured
`BLINDJOIN_BOT_SCRIPT_TYPES`; each run is single-shot and uses a fresh
wallet, so output addresses do not cluster across rounds.
```

---

## Shared Patterns (cross-cutting; apply to multiple new files)

### Cross-Cutting Pattern 1: `require_bitcoind!() → bootstrap_regtest_bitcoind → guard binding → fund → spawn_coordinator`

**Source:** `tests/integration/mod.rs:100-108` (macro) + `tests/integration/mod.rs:617-823` (`fund_regtest_typed`) + `tests/integration/full_round.rs:85-153` (`spawn_coordinator`).

**Apply to:** `mixed_script_e2e.rs`, `bot_rotation.rs`, `v13_binary_compat.rs` — all 3 NEW integration tests use this exact 5-step preamble.

**Load-bearing details:**
- Macro form (NOT a plain fn) — only `return` from the macro expansion correctly skips a single test fn without aborting the test binary (mod.rs:89-93).
- Guard binding via `let _bitcoind_guard = bitcoind_guard;` — naming the variable `_bitcoind_guard` (NOT `_`) is load-bearing per Rust semantics: `let _ = ...` drops immediately, `let _name = ...` holds until end-of-scope.
- `spawn_coordinator` returns `(url, tempfile::TempDir)`; caller MUST bind tempdir to a name (`_tmp_dir`) to keep ban-file's parent alive (WR-06 invariant at full_round.rs:81-84).

### Cross-Cutting Pattern 2: Synthetic CoordinatorInfo factory

**Source:** `tests/integration/full_round.rs:49-59` (`v14_p2wpkh_coordinator_info`).

**Apply to:** `mixed_script_e2e.rs` (parameterized per script type per D-85) + `liquidity-bot/src/main.rs:171-183` (extend to populate per-rotated-type).

**Load-bearing detail:** Both `supported_script_types` AND `output_script_type` MUST equal the wallet's own type — Phase 17 D-76 fail-fast at `client::discover::discover_coordinator` checks this cross-product, and `client::round::input::register_input` would propagate `DiscoveryError::UnsupportedOutputScriptType` if either side is wrong (RESEARCH Pitfall 2).

### Cross-Cutting Pattern 3: Test isolation — one bitcoind per `#[tokio::test]` fn

**Source:** `tests/integration/full_round.rs` + `tests/integration/multi_script_validate.rs` + `tests/integration/multi_script_client.rs`.

**Apply to:** All 3 NEW integration tests (per CONTEXT D-103).

**Load-bearing detail:** Slow (each test spins up bitcoind, ~3-5s) but bulletproof against UTXO/state cross-pollution. The `BitcoindGuard` RAII pattern (mod.rs:150-228) ensures clean shutdown via `n.stop()` on tokio blocking pool + `Node::Drop` SIGKILL fallback.

### Cross-Cutting Pattern 4: ScriptType wire-form parsing

**Source:** `client/src/config.rs:10-15` (`parse_script_type`).

**Apply to:** `liquidity-bot/src/main.rs` (parse `BLINDJOIN_BOT_SCRIPT_TYPES` CSV via repeated calls to `parse_script_type`).

**Load-bearing detail:** Wraps string in JSON quotes and routes through `serde_json::from_str::<ScriptType>` so the accepted tokens are the SINGLE source of truth from Phase 15's `#[serde(rename_all = "snake_case")]` + `#[serde(rename = "p2sh-p2wpkh")]` (CD-17 lowercase kebab-case only). NEVER hand-roll a string match in the bot.

### Cross-Cutting Pattern 5: Single-underscore env-var convention (bot-side)

**Source:** Phase 4 + Phase 17 CD-22 + `liquidity-bot/src/main.rs:35-69`.

**Apply to:** All new bot env vars (`BLINDJOIN_BOT_SCRIPT_TYPES`, `BLINDJOIN_BOT_P2WPKH_UTXO`, etc.).

**Load-bearing detail:** Bot uses `BLINDJOIN_*` flat namespace; coordinator uses `BLINDJOIN__*__*` hierarchical (the `config` crate's hierarchical wire form). Phase 18 MUST NOT mix conventions on the bot side per CONTEXT D-93.

### Cross-Cutting Pattern 6: `tokio::task::spawn_blocking` for sync corepc-node RPC

**Source:** `tests/integration/full_round.rs:302-311` (mempool poll) + 345-367 (output count verify).

**Apply to:** `mixed_script_e2e.rs` (mempool poll + per-prevout SPK re-query for the input-type set assertion).

**Load-bearing detail:** corepc-node `Client` is sync. The closure passed to `spawn_blocking` takes `'static + Send`, so each iteration must CLONE the RPC creds (the loop body cannot borrow them).

---

## No Analog Found

Files with no close in-repo match (planner uses RESEARCH.md prescriptions instead):

| File / Pattern | Role | Reason | RESEARCH section consulted |
|----------------|------|--------|----------------------------|
| `tests/integration/v13_binary_compat.rs` (external-binary driving via `tokio::process::Command::new`) | Integration test | First test in repo that builds and drives an external binary; no precedent for the `git worktree add` + `cargo build` + `Command::new` chain | §Q2 (recipe) + §Pitfall 5 (Cargo.lock drift safeguards) |
| `liquidity-bot/src/strategy.rs` atomic counter-file write | Service-side state | No pre-existing atomic-write idiom in repo; `BLAME-05 append_ban_entry` is append-only (wrong shape for overwrite-counter) | §Q4 (`tokio::fs::write + rename` idiom prescribed) |
| `README.md` §"Privacy Considerations" | Documentation | README has no §-disclaimer-style sections currently | §R5 (insertion point + verbatim prose from CONTEXT D-106) |
| `.planning/phases/18-.../v13_pinned_sha.txt` | Pinned-artifact convention | First pinned-SHA file in project | §Q2 (verified SHA `05f21438` + commit subject) |

---

## Metadata

**Analog search scope:**
- `tests/integration/*.rs` (8 files inspected; full_round.rs + multi_script_validate.rs + multi_script_client.rs + mod.rs as primary analogs).
- `liquidity-bot/src/*.rs` (2 files inspected; both consumed as self-analogs for the MODIFY operations).
- `client/src/wallet.rs` (signatures for `from_wif`, `from_descriptor`, `generate`, `coinjoin_output_address`; `utxo_outpoint` public field verified).
- `client/src/config.rs` (`parse_script_type` reuse target for bot CSV).
- `coordinator/src/round/blame.rs` (BLAME-05 `append_ban_entry` — confirmed wrong shape for the bot's overwrite-counter use case; informs the NEW idiom recommendation).
- `coordinator/src/api/handlers.rs:296-440` (read-only verification per D-90).
- `docker/docker-compose.yml` + `docker/Dockerfile` (volume + multi-stage build patterns).
- `coordinator/Cargo.toml` (`[[test]]` declaration + `[features]` location for `v13-binary-compat`).

**Files scanned:** ~14 inspected directly + 5+ via Grep / Bash verification.

**Pinned SHA verification:** `git log --first-parent --oneline 622ccf0^ -1` returned `05f21438a7072987773bfe2eafaac5c51c68c61a docs(15): create phase plan` — RESEARCH §Q2 claim confirmed live.

**Pattern extraction date:** 2026-05-30.

**Downstream consumer:** `gsd-planner` reads this PATTERNS.md to populate each 18-0N-PLAN.md's `read_first` (analog file + line range) and `action` (concrete pattern to mirror) sections. Where "No Analog Found" applies, planner falls back to the RESEARCH.md section named in that row.
