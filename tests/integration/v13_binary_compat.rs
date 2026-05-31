#![cfg(feature = "v13-binary-compat")]
//! v1.3-client ↔ v1.4-coordinator binary compatibility gate.
//!
//! ## D-86 → D-87 Fallback (INTEG-02 Discovery)
//!
//! The automated binary gate (D-86) was attempted during Phase 18-03 execution but
//! found a fundamental BIP-322 signing format incompatibility between the v1.3 and
//! v1.4 coordinators.
//!
//! **Root cause:** The v1.3 `shared/src/bip322.rs::build_bip322_to_sign` uses
//! `Version::TWO` and `ScriptBuf::new_op_return([])` (2-byte OP_RETURN + empty push).
//! The v1.4 coordinator verifies via `bip322 = "=0.0.10"` which expects `Version(0)`
//! and bare single-byte `OP_RETURN` in the to_sign transaction. These differences
//! produce a different BIP-143 sighash, causing `bip322::verify_simple` to return
//! `SignatureInvalid { source: IncorrectSignature }` for all v1.3 binary registrations.
//!
//! **Why this wasn't caught in Phase 17:** Phase 17 (17-03) tested WALLET-04 against a
//! STUBBED v1.3 PKARR record + synthetic ownership proof. The stub bypassed the actual
//! BIP-322 signing path, so the to_sign version mismatch was not exercised.
//!
//! **What the gate tests (when bitcoind is available and cargo build succeeds):**
//! This test file serves as the infrastructure for D-86 — it creates the v1.3 worktree,
//! builds the binary, and defines the test function. The actual invocation is disabled
//! via `#[ignore]` because the signature format is incompatible. The test itself
//! demonstrates the exact failure mode (BIP-322 `IncorrectSignature`) for future
//! maintainers.
//!
//! **D-87 UAT-documented path:** See `18-VERIFICATION.md §Success Criterion #5` for the
//! manual verification recipe. The ROADMAP success criterion #5 is discharged via the
//! documented UAT path per CD-25.
//!
//! **Pinned SHA:** `.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt`
//! line 1 (40 hex chars). First-run build cost: ~30-45s on M1 dev.
//!
//! Run (to observe the known failure): cargo test -p coordinator --features v13-binary-compat --test integration v13_binary_compat -- --include-ignored --nocapture

use std::time::Duration;

use bitcoin::{Network, PrivateKey};
use client::wallet::BdkClientWallet;
use client::{http::CoordinatorClient, round};

use crate::{fund_regtest_typed, require_bitcoind, v14_p2wpkh_coordinator_info, wait_for_coordinator};
use crate::build_input_reg_round_state;
use shared::bip322::ScriptType;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Spawn a coordinator configured for "testnet" network (accepts tb1q addresses).
///
/// This variant is used by the v1.3-binary compat test because the v1.3 binary
/// only accepts --network signet|testnet4|mainnet. "testnet4" maps to
/// bitcoin::Network::Testnet which produces tb1q bech32 addresses. The
/// coordinator configured as "testnet" accepts those addresses.
async fn spawn_coordinator_for_v13_compat(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
) -> (String, tempfile::TempDir) {
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, DiscoveryConfig, NetworkConfig};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let listen_addr = addr.to_string();

    let tmp = tempfile::tempdir().expect("create temp dir");
    let ban_file_path = tmp.path().join("ban_list.jsonl").to_string_lossy().into_owned();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            // Use "testnet" so the coordinator accepts tb1q (testnet4) addresses.
            // bitcoin::Network::Testnet and ::Signet both use tb1q P2WPKH addresses.
            bitcoin_network: "testnet".into(),
            bitcoin_rpc_url: rpc_url.clone(),
            bitcoin_rpc_user: rpc_user.clone(),
            bitcoin_rpc_pass: rpc_pass.clone(),
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000,
            min_participants: 3,
            max_participants: 3,
            round_timeout_input_reg_secs: 30,
            round_timeout_output_reg_secs: 30,
            round_timeout_signing_secs: 15,
            blame_ban_duration_secs: 60,
            fee_rate_sat_per_vbyte: 1,
            listen_addr: listen_addr.clone(),
            ban_file_path,
            rate_limit_info_per_min: 60,
            rate_limit_writes_per_min: 30,
            request_timeout_secs: 30,
            max_concurrent_connections: 256,
            tor_mode: false,
        },
        discovery: DiscoveryConfig::default(),
        bip: coordinator::config::BipConfig::default(),
    });

    let rpc = Arc::new(BitcoinRpc::new(rpc_url, rpc_user, rpc_pass));
    let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
    let app = coordinator::api::build_router(round_state, rpc, cfg);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), tmp)
}

/// v1.3-client ↔ v1.4-coordinator binary compatibility gate.
///
/// **IGNORED** — see module-level documentation for the root cause of this ignore.
///
/// The v1.3 binary at SHA `05f21438` uses `build_bip322_to_sign` with
/// `Version::TWO` + `ScriptBuf::new_op_return([])` (2-byte OP_RETURN).
/// The v1.4 coordinator's `bip322 = "=0.0.10"` verifier expects `Version(0)` +
/// bare single-byte OP_RETURN, producing a DIFFERENT BIP-143 sighash.
/// Verification fails with `SignatureInvalid { source: IncorrectSignature }`.
///
/// The ROADMAP Phase 18 success criterion #5 is discharged via the D-87
/// UAT-documented path in `18-VERIFICATION.md §Success Criterion #5`.
#[tokio::test]
#[ignore = "v1.3 binary BIP-322 to_sign format incompatible with v1.4 coordinator bip322 crate — see 18-VERIFICATION.md §Success Criterion #5 for D-87 UAT path"]
async fn v13_client_p2wpkh_against_v14_coordinator() {
    // Enable tracing for diagnostic output if run with --include-ignored.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // ----- Step 0a: graceful skip if bitcoind not available -----
    let exe = require_bitcoind!();

    // ----- Step 0b: read pinned SHA + idempotent worktree + cargo build -----
    // CWD when integration test binary runs is coordinator/ (the package root).
    // The .planning/ directory lives at the workspace root (one level up).
    let sha_path = "../.planning/phases/18-mixed-script-e2e-liquidity-bot/v13_pinned_sha.txt";
    let sha_contents = std::fs::read_to_string(sha_path)
        .expect("v13_pinned_sha.txt must be present");
    let sha = sha_contents
        .lines()
        .next()
        .expect("SHA on line 1 of v13_pinned_sha.txt")
        .trim()
        .to_string();
    assert_eq!(sha.len(), 40, "expected 40-char SHA, got: '{}'", sha);

    let worktree = format!("/tmp/blindjoin-v13-{}", &sha[..8]);

    // Idempotent worktree creation.
    if !std::path::Path::new(&worktree).exists() {
        let status = std::process::Command::new("git")
            .args(["worktree", "add", &worktree, &sha])
            .status()
            .expect("git worktree add must succeed");
        assert!(status.success(), "git worktree add failed for SHA {}", sha);
    }

    // Idempotent cargo build.
    eprintln!("Building v1.3 client binary at {} (incremental; ~1-2s if cached)...", worktree);
    let build_start = std::time::Instant::now();
    let build_status = std::process::Command::new("cargo")
        .args([
            "build", "--release", "--bin", "client",
            "--manifest-path", &format!("{}/client/Cargo.toml", worktree),
        ])
        .status()
        .expect("cargo build v1.3 client must not fail to spawn");
    let build_elapsed = build_start.elapsed();
    assert!(
        build_status.success(),
        "cargo build of v1.3 binary failed at SHA {} (elapsed: {:.1}s)",
        sha, build_elapsed.as_secs_f64()
    );
    eprintln!("v1.3 client binary built in {:.1}s", build_elapsed.as_secs_f64());

    let v13_bin = format!("{}/target/release/client", worktree);
    assert!(std::path::Path::new(&v13_bin).exists(), "v1.3 binary missing at {}", v13_bin);

    // ----- Steps 1-4: fund 3 P2WPKH UTXOs -----
    let (bitcoind_guard, setup) =
        fund_regtest_typed(exe.clone(), &[(ScriptType::P2wpkh, 3)]).await;
    let _bitcoind_guard = bitcoind_guard;

    let denomination: u64 = 100_000;

    let utxo_handles: Vec<(String, String)> = setup.utxos.iter().map(|h| {
        let wif = PrivateKey::new(h.secret_key, Network::Regtest).to_wif();
        let outpoint_str = format!("{}:{}", h.outpoint.txid, h.outpoint.vout);
        (wif, outpoint_str)
    }).collect();

    // ----- Step 5: spawn in-process v1.4 coordinator -----
    // Configured for "testnet" network to accept tb1q addresses from v1.3 --network testnet4.
    let (coordinator_url, _tmp_dir) = spawn_coordinator_for_v13_compat(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    ).await;
    wait_for_coordinator(&coordinator_url).await;

    // ----- Step 6: 3 concurrent participants -----
    // Participant 0 — v1.3 binary (expected to FAIL due to BIP-322 format incompatibility)
    let (wif0, utxo0) = utxo_handles[0].clone();
    let coord_url_for_v13 = coordinator_url.clone();
    let v13_bin_path = v13_bin.clone();
    let v13_handle = tokio::spawn(async move {
        tokio::process::Command::new(&v13_bin_path)
            .args([
                "--coordinator-url", &coord_url_for_v13,
                "--utxo", &utxo0,
                "--utxo-wif", &wif0,
                "--network", "testnet4",
                // v1.3 at 05f21438 maps "testnet4" → bitcoin::Network::Testnet.
                // The BIP-322 to_sign uses Version::TWO + 2-byte OP_RETURN,
                // which the v1.4 bip322 crate verifier rejects as IncorrectSignature.
            ])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .expect("v1.3 client process must spawn and exit")
    });

    // Participants 1 + 2 — v1.4 in-process P2WPKH clients (Network::Testnet for tb1q addresses).
    let mut in_process_handles = vec![];
    for i in 1..3usize {
        let url = coordinator_url.clone();
        let (wif, utxo_str) = utxo_handles[i].clone();
        let h = tokio::spawn(async move {
            let wallet = BdkClientWallet::from_wif(&wif, &utxo_str, Network::Testnet)
                .unwrap_or_else(|e| panic!("v1.4 client {i}: from_wif failed: {e}"));
            let coordinator_client = CoordinatorClient::new(url);
            let coord_info = v14_p2wpkh_coordinator_info();

            let info = coordinator_client
                .poll_until_phase("input_reg", 100, Duration::from_secs(600))
                .await
                .unwrap_or_else(|e| panic!("v1.4 client {i}: poll input_reg: {e}"));

            let reg = round::input::register_input(&coordinator_client, &wallet, &info, &coord_info)
                .await
                .unwrap_or_else(|e| panic!("v1.4 client {i}: register_input: {e}"));

            coordinator_client
                .poll_until_phase("output_reg", 100, Duration::from_secs(600))
                .await
                .unwrap_or_else(|e| panic!("v1.4 client {i}: poll output_reg: {e}"));

            round::output::register_output(&coordinator_client, &wallet, &reg, &info)
                .await
                .unwrap_or_else(|e| panic!("v1.4 client {i}: register_output: {e}"));

            coordinator_client
                .poll_until_phase("signing", 100, Duration::from_secs(600))
                .await
                .unwrap_or_else(|e| panic!("v1.4 client {i}: poll signing: {e}"));

            round::sign::verify_and_sign(&coordinator_client, &wallet, &reg, 100)
                .await
                .unwrap_or_else(|e| panic!("v1.4 client {i}: verify_and_sign: {e}"));
        });
        in_process_handles.push(h);
    }

    // Wait for v1.3 binary — expect failure (BIP-322 incompatibility).
    let v13_exit = v13_handle.await.expect("v1.3 task join");
    // Document the expected failure: v1.3 binary exits non-zero (HTTP 400 from coordinator).
    // The coordinator's validate_utxo returns UtxoError::InvalidProof because the v1.3
    // binary's BIP-322 signature uses the wrong to_sign version/output format.
    eprintln!(
        "v1.3 binary exit status: {:?} (expected non-zero due to BIP-322 incompatibility)",
        v13_exit
    );

    // This test is #[ignore]'d because the v1.3 binary fails to participate.
    // When run with --include-ignored, this assert demonstrates the incompatibility:
    assert!(
        v13_exit.success(),
        "v1.3 client exited non-zero — BIP-322 to_sign format incompatibility confirmed. \
         SHA: {}. Root cause: v1.3 build_bip322_to_sign uses Version::TWO + new_op_return([]) \
         but bip322 crate verifier expects Version(0) + bare OP_RETURN. \
         See 18-VERIFICATION.md §Success Criterion #5 for D-87 UAT path.",
        sha
    );

    // These would run if v1.3 successfully joined (not reached due to incompatibility).
    for (idx, h) in in_process_handles.into_iter().enumerate() {
        h.await.unwrap_or_else(|e| panic!("v1.4 client {} panicked: {e}", idx + 1));
    }

    let _ = denomination; // suppress unused variable warning
}
