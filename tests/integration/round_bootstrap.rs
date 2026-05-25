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
//! Requires bitcoind in PATH (or BITCOIND_EXE env var). Gracefully skips
//! otherwise — matches the pattern in `full_round.rs`.
//!
//! Threat-model compliance:
//!   T-06-02: NO test-only backdoors. Calls `coordinator::run` directly.
//!   T-06-03: Uses port 0 (OS assigns ephemeral port) to avoid conflicts.

use std::time::Duration;

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
        CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig,
    };

    // ----- Skip gracefully if bitcoind is unavailable -----
    let exe = match corepc_node::exe_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "bitcoind not found ({}), skipping run_bootstraps_round_into_input_reg",
                e
            );
            return;
        }
    };

    // ----- Spin up regtest bitcoind so startup_health_check passes -----
    // run() calls startup_health_check which requires reachable bitcoind.
    // We mine 101 blocks so block_count > 0 (also required by the health check).
    let (rpc_url, rpc_user, rpc_pass) = tokio::task::spawn_blocking(move || {
        use bitcoin::Address;
        use corepc_node::{Conf, Node};

        let mut conf = Conf::default();
        conf.network = "regtest";

        let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");
        let cookie = node
            .params
            .get_cookie_values()
            .expect("read cookie file")
            .expect("parse cookie values");
        let rpc_url = node.rpc_url();
        let rpc_user = cookie.user.clone();
        let rpc_pass = cookie.password.clone();

        // Mine a single block so the health check's block_count > 0 assertion holds.
        let mine_addr: Address = node.client.new_address().expect("get new address");
        node.client
            .generate_to_address(101, &mine_addr)
            .expect("generate 101 blocks");

        // Leak the node — OS reaps it at test exit.
        let node_box = Box::new(node);
        Box::leak(node_box);

        (rpc_url, rpc_user, rpc_pass)
    })
    .await
    .expect("regtest bootstrap spawn_blocking panicked");

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
            tor_mode: false,
        },
        discovery: DiscoveryConfig {
            pkarr_key_file,
            // PKARR heartbeat publishes are best-effort; failures here log a
            // warning but don't affect the bootstrap assertion. Setting the
            // interval high so it doesn't fire during the test window.
            heartbeat_interval_secs: 3600,
            coordinator_public_addr: "127.0.0.1:0".into(),
        },
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
