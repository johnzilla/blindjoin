//! Integration test: production round bootstrap via `coordinator::run`.
//!
//! This test exercises the same startup path as the production binary —
//! `coordinator::run(cfg)` — and asserts that the coordinator transitions
//! out of `Phase::Idle` and into `Phase::InputReg` with a freshly generated
//! RSA public key, without any test-only state mutation.
//!
//! This is the test that would have caught the v1.1 round-bootstrap regression
//! pre-ship. Prior integration tests hand-built a router with a pre-populated
//! `RoundStateInner` and never invoked the production startup path; this test
//! does not.
//!
//! Requires bitcoind discoverable by `corepc_node::exe_path()` (via PATH or
//! BITCOIND_EXE env var). Behavior in CI: panics if env
//! `BLINDJOIN_REQUIRE_BITCOIND=1` is set and bitcoind is missing (per Phase 9
//! D-07). Behavior locally: gracefully skips when `BLINDJOIN_REQUIRE_BITCOIND`
//! is unset. Routes via the shared `crate::require_bitcoind!()` macro and
//! `crate::bootstrap_regtest_bitcoind()` helper in `tests/integration/mod.rs`.
//!
//! Threat-model compliance:
//!   T-06-02: NO test-only backdoors. Calls `coordinator::run` directly.
//!   T-06-03: Uses port 0 (OS assigns ephemeral port) to avoid conflicts.

use std::time::Duration;

use crate::{bootstrap_regtest_bitcoind, require_bitcoind, BitcoindGuard, RpcCreds};

/// Reserve a free localhost port by binding to port 0, reading the port, then
/// dropping the listener. There is a TOCTOU race where another process could
/// claim the port between drop() and `coordinator::run` re-binding, but in a
/// single-test scenario this is acceptable.
async fn reserve_free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);
    port
}

/// Production round bootstrap: `coordinator::run(cfg)` must transition Idle →
/// InputReg with a populated RSA public key, without any test-side state
/// mutation. This is the regression guard for the v1.1 bootstrap bug.
#[tokio::test]
async fn run_bootstraps_round_into_input_reg() {
    use coordinator::config::{
        BipConfig, CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig,
    };

    // ----- Skip gracefully if bitcoind is unavailable (local-dev); panic in CI -----
    let exe = require_bitcoind!();

    // ----- Spin up regtest bitcoind so startup_health_check passes -----
    // run() calls startup_health_check which requires reachable bitcoind.
    // bootstrap_regtest_bitcoind mines 101 blocks so block_count > 0 (also
    // required by the health check), and returns a BitcoindGuard whose Drop
    // runs node.stop() + Node::Drop's process.kill() — replaces the historical
    // leak-the-Node pattern (Phase 9 TEST-03 / TEST-04 closure).
    let (bitcoind_guard, creds): (BitcoindGuard, RpcCreds) = bootstrap_regtest_bitcoind(exe).await;
    let rpc_url = creds.url.clone();
    let rpc_user = creds.user.clone();
    let rpc_pass = creds.pass.clone();
    // Hold the guard for the test's full duration; drops at end-of-scope so
    // Drop::drop runs node.stop() before the test function returns.
    let _bitcoind_guard = bitcoind_guard;

    // ----- Build a coordinator config wired for clearnet + free port -----
    let port = reserve_free_port().await;
    let listen_addr = format!("127.0.0.1:{port}");
    let coordinator_url = format!("http://{listen_addr}");

    // Use a temp dir for the PKARR key file and ban file so the test doesn't
    // touch any shared on-disk state.
    let tmp = tempfile::tempdir().expect("create temp dir");
    let pkarr_key_file = tmp
        .path()
        .join("pkarr.key")
        .to_string_lossy()
        .into_owned();
    let ban_file_path = tmp
        .path()
        .join("ban_list.jsonl")
        .to_string_lossy()
        .into_owned();

    let cfg = CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: rpc_url,
            bitcoin_rpc_user: rpc_user,
            bitcoin_rpc_pass: rpc_pass,
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000,
            min_participants: 3,
            max_participants: 3,
            round_timeout_input_reg_secs: 60,
            round_timeout_output_reg_secs: 60,
            round_timeout_signing_secs: 30,
            blame_ban_duration_secs: 3600,
            fee_rate_sat_per_vbyte: 1,
            listen_addr,
            ban_file_path,
            rate_limit_info_per_min: 60_000,
            rate_limit_writes_per_min: 60_000,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            tor_mode: false,
            blame_full_abort_backoff_secs: 0,
        },
        discovery: DiscoveryConfig {
            pkarr_key_file,
            // PKARR heartbeat publishes are best-effort; failures here log a
            // warning but don't affect the bootstrap assertion. Setting the
            // interval high so it doesn't fire during the test window.
            heartbeat_interval_secs: 3600,
            coordinator_public_addr: "127.0.0.1:0".into(),
        },
        // Phase 16 Plan 16-01 (Rule 3 — Blocker): CoordinatorConfig gained
        // a top-level `bip` field. Use BipConfig::default() to opt into the
        // all-allowed + p2wpkh-output defaults so this test's bootstrap path
        // behaves identically to the v1.3 byte-shape (cross-phase invariant).
        bip: BipConfig::default(),
    };

    // ----- Spawn the production startup path -----
    let run_handle = tokio::spawn(async move {
        if let Err(e) = coordinator::run(cfg).await {
            eprintln!("coordinator::run returned Err: {e}");
        }
    });

    // ----- Poll /info until phase != "idle" (max 10s) -----
    let http_client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut last_info: Option<shared::protocol::InfoResponse> = None;
    loop {
        if tokio::time::Instant::now() > deadline {
            run_handle.abort();
            panic!(
                "Coordinator never left Idle within 10s. Last /info: {:?}",
                last_info
            );
        }

        // Wait briefly for the HTTP server to come up, then poll.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let resp = match http_client
            .get(format!("{coordinator_url}/info"))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue, // server not up yet
        };
        let info: shared::protocol::InfoResponse = match resp.json().await {
            Ok(i) => i,
            Err(_) => continue,
        };
        last_info = Some(info.clone());
        if info.round_state != "idle" {
            // ----- Assertions on the bootstrapped round -----
            assert_eq!(
                info.round_state, "input_reg",
                "First phase after Idle must be input_reg, got {}",
                info.round_state
            );
            assert!(
                info.rsa_pubkey_der_b64.is_some(),
                "rsa_pubkey_der_b64 must be set in input_reg phase"
            );
            assert!(
                info.rsa_pubkey_hash.is_some(),
                "rsa_pubkey_hash must be set in input_reg phase"
            );
            assert!(
                info.round_id.is_some(),
                "round_id must be set in input_reg phase"
            );
            assert_eq!(
                info.participants_registered, 0,
                "Fresh round must report 0 participants registered"
            );
            assert_eq!(info.denomination_sats, 100_000);
            assert_eq!(info.min_participants, 3);
            assert_eq!(info.max_participants, 3);

            // Decode the published DER key and verify SHA-256(der) == hash (D-02).
            use base64::Engine;
            use sha2::{Digest, Sha256};
            let b64 = base64::engine::general_purpose::STANDARD;
            let der = b64
                .decode(info.rsa_pubkey_der_b64.as_ref().unwrap())
                .expect("rsa_pubkey_der_b64 must be valid base64");
            let computed_hash = hex::encode(Sha256::digest(&der));
            assert_eq!(
                Some(computed_hash),
                info.rsa_pubkey_hash,
                "D-02: SHA-256(decoded_der) must equal published rsa_pubkey_hash"
            );

            eprintln!(
                "round_bootstrap PASSED: phase={}, round_id={:?}, rsa_pubkey_hash={:?}",
                info.round_state, info.round_id, info.rsa_pubkey_hash
            );
            run_handle.abort();
            return;
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 16 Plan 16-01 Task 3: GET /info exposes BipConfig-derived
// supported_script_types + output_script_type.
//
// These tests use `build_router` directly with a sentinel bitcoind RPC URL
// (same pattern as `full_round::coordinator_info_endpoint_fields`) so no
// bitcoind is required — GET /info reads only in-memory round state +
// state.config.bip.
// ---------------------------------------------------------------------------

/// Helper: bring up an in-process coordinator router with a custom
/// `CoordinatorConfig`, return its base URL + the temp dir owning the ban
/// file path. No bitcoind required — sentinel RPC URL is intentionally
/// unbindable (mirrors `full_round::coordinator_info_endpoint_fields`'s
/// `invalid-rpc-not-running.localhost:1` rationale).
async fn spawn_info_only_coordinator(
    cfg: coordinator::config::CoordinatorConfig,
) -> (String, tempfile::TempDir) {
    use std::sync::Arc;
    use coordinator::api::build_router;
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::round::state::RoundState;
    use tokio::sync::RwLock;

    let tmp = tempfile::tempdir().expect("create temp dir");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let sentinel_rpc_url = "http://invalid-rpc-not-running.localhost:1";
    let rpc = Arc::new(BitcoinRpc::new(
        sentinel_rpc_url.into(),
        String::new(),
        String::new(),
    ));
    let cfg_arc = Arc::new(cfg);
    let round_state = Arc::new(RwLock::new(RoundState::new_idle()));
    let app = build_router(round_state, rpc, cfg_arc);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for the server to come up before returning.
    let http_client = reqwest::Client::new();
    let base = format!("http://{}", addr);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let ok = http_client
            .get(format!("{}/info", base))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Phase 16 info-only coordinator did not start within 3s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    (base, tmp)
}

/// Build the test config Phase 16-01 Task 3 needs: a default
/// `CoordinatorConfig::with_defaults()` with the listen_addr / ban_file_path
/// rewired for the test, and the `bip` field overridable by the caller.
fn make_phase16_test_cfg(
    tmp: &std::path::Path,
    bip: coordinator::config::BipConfig,
) -> coordinator::config::CoordinatorConfig {
    use coordinator::config::CoordinatorConfig;

    let ban_file_path = tmp
        .join("ban_list.jsonl")
        .to_string_lossy()
        .into_owned();

    let mut cfg = CoordinatorConfig::with_defaults();
    // Rewire knobs that the test-only path needs:
    cfg.coordinator.listen_addr = "127.0.0.1:0".into();
    cfg.coordinator.ban_file_path = ban_file_path;
    cfg.network.bitcoin_network = "regtest".into();
    cfg.bip = bip;
    cfg
}

/// Default-config /info: all 3 script types allowed (alphabetical canonical
/// order per CD-11) + output_script_type defaults to P2WPKH.
#[tokio::test]
async fn get_info_supports_all_three_script_types_with_defaults() {
    use coordinator::config::BipConfig;
    use shared::bip322::ScriptType;

    let tmp = tempfile::tempdir().expect("create temp dir");
    let cfg = make_phase16_test_cfg(tmp.path(), BipConfig::default());

    let (base, _tmp_dir_keep) = spawn_info_only_coordinator(cfg).await;
    let http_client = reqwest::Client::new();
    let info: shared::protocol::InfoResponse = http_client
        .get(format!("{}/info", base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(
        info.supported_script_types,
        vec![
            ScriptType::P2shP2wpkh,
            ScriptType::P2tr,
            ScriptType::P2wpkh,
        ],
        "CD-11 alphabetical canonical order: p2sh-p2wpkh < p2tr < p2wpkh"
    );
    assert_eq!(
        info.output_script_type,
        ScriptType::P2wpkh,
        "default output_script_type per D-37"
    );

    // Keep tmp alive (ban_file_path lives in it) through the assertion.
    drop(tmp);
}

/// Operator allowlist filters supported_script_types: setting
/// `allow_p2tr = false` removes P2TR from the advertised set while
/// preserving the alphabetical canonical order of the remaining types.
#[tokio::test]
async fn get_info_filters_supported_by_allowlist() {
    use coordinator::config::BipConfig;
    use shared::bip322::ScriptType;

    let tmp = tempfile::tempdir().expect("create temp dir");
    let bip = BipConfig {
        allow_p2wpkh: true,
        allow_p2tr: false,
        allow_p2sh_p2wpkh: true,
        output_script_type: ScriptType::P2wpkh,
    };
    let cfg = make_phase16_test_cfg(tmp.path(), bip);

    let (base, _tmp_dir_keep) = spawn_info_only_coordinator(cfg).await;
    let http_client = reqwest::Client::new();
    let info: shared::protocol::InfoResponse = http_client
        .get(format!("{}/info", base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(
        info.supported_script_types,
        vec![ScriptType::P2shP2wpkh, ScriptType::P2wpkh],
        "P2tr filtered out; alphabetical canonical preserved"
    );
    assert_eq!(info.output_script_type, ScriptType::P2wpkh);

    drop(tmp);
}
