//! Phase 18 18-02 INTEG-02 — liquidity bot multi-script rotation test.
//!
//! Verifies two properties:
//!
//! 1. **RotationState restart-cycle assertion (D-102):** Three sequential
//!    `RotationState` constructions sharing the same counter file assert that
//!    the type selection rotates P2wpkh → P2tr → P2shP2wpkh → P2wpkh (wraps)
//!    across simulated Docker restart boundaries (new RotationState per "run").
//!
//! 2. **End-to-end liquidity_bot::run (Pitfall 4 gate):** One actual
//!    `liquidity_bot::run(config)` call against an in-process v1.4 coordinator,
//!    driving the P2WPKH path. Verifies the [lib] extraction works and that the
//!    rotation counter is bumped to "1" after a successful round.
//!
//! D-103: one bitcoind per test fn (isolated; no UTXO/state cross-pollution).
//! CD-26: test lives in tests/integration/ (shares coordinator spawn infra).
//! CD-28: counter file in tempfile::tempdir() (hermetic — never /app/data).
//!
//! Run: cargo test -p coordinator --test integration bot_rotation -- --nocapture

use std::time::Duration;

use bitcoin::{Network, PrivateKey};
use liquidity_bot::strategy::RotationState;
use liquidity_bot::{run, BotConfig, P2wpkhTuple};
use shared::bip322::ScriptType;

use crate::{fund_regtest_typed, require_bitcoind, spawn_coordinator, v14_coordinator_info,
    wait_for_coordinator};

// Suppress unused import warning — v14_coordinator_info is imported for
// completeness (bot_rotation.rs mirrors mixed_script_e2e.rs import pattern)
// but the bot's run() constructs its own synthetic_info internally.
#[allow(unused_imports)]
use crate::fund_regtest_typed as _fund_regtest_typed_alias;

/// INTEG-02 acceptance gate — bot rotation across 3 simulated Docker restarts
/// plus one full end-to-end `liquidity_bot::run` invocation.
#[tokio::test]
async fn bot_rotates_p2wpkh_p2tr_p2sh_p2wpkh_across_three_runs() {
    // ----- Precondition: bitcoind must be available -----
    let exe = require_bitcoind!();

    // ----- Fund 3 P2WPKH UTXOs — used for the e2e run + 2 concurrent peers -----
    // D-103: one bitcoind per test fn.
    let (bitcoind_guard, setup) =
        fund_regtest_typed(exe.clone(), &[(ScriptType::P2wpkh, 3)]).await;
    let _bitcoind_guard = bitcoind_guard;

    // ----- Part 1: RotationState restart-cycle assertion -----
    //
    // Simulates 3 Docker restart cycles: each cycle creates a NEW RotationState
    // (same counter_file path) to simulate the bot process re-starting. The
    // counter file is in a tmpdir — hermetic, never /app/data.
    //
    // Expected sequence:
    //   cycle 0: counter=0 → pick=P2wpkh → bump → counter file = "1\n"
    //   cycle 1: counter=1 → pick=P2tr   → bump → counter file = "2\n"
    //   cycle 2: counter=2 → pick=P2shP2wpkh → bump → counter file = "3\n"
    //   cycle 3: counter=3 → pick=P2wpkh (wraps: 3%3=0)

    let rotation_dir = tempfile::tempdir().expect("tempdir for rotation counter");
    let counter_file = rotation_dir.path().join("bot_round_counter");

    let enabled = vec![ScriptType::P2wpkh, ScriptType::P2tr, ScriptType::P2shP2wpkh];

    // Cycle 0 — counter missing → P2wpkh
    let state0 = RotationState::new(counter_file.clone(), enabled.clone()).unwrap();
    assert_eq!(
        state0.pick_script_type().await.unwrap(),
        ScriptType::P2wpkh,
        "cycle 0 (counter=0): expected P2wpkh"
    );
    state0.bump_counter().await.unwrap();
    assert!(
        counter_file.exists(),
        "counter file must exist after first bump"
    );
    assert_eq!(
        tokio::fs::read_to_string(&counter_file).await.unwrap().trim(),
        "1",
        "counter file must contain '1' after first bump"
    );

    // Cycle 1 — fresh RotationState reads persisted counter=1 → P2tr
    let state1 = RotationState::new(counter_file.clone(), enabled.clone()).unwrap();
    assert_eq!(
        state1.pick_script_type().await.unwrap(),
        ScriptType::P2tr,
        "cycle 1 (counter=1): expected P2tr"
    );
    state1.bump_counter().await.unwrap();

    // Cycle 2 — fresh RotationState reads persisted counter=2 → P2shP2wpkh
    let state2 = RotationState::new(counter_file.clone(), enabled.clone()).unwrap();
    assert_eq!(
        state2.pick_script_type().await.unwrap(),
        ScriptType::P2shP2wpkh,
        "cycle 2 (counter=2): expected P2shP2wpkh"
    );
    state2.bump_counter().await.unwrap();

    // Cycle 3 — wraps: counter=3 → 3%3=0 → P2wpkh
    let state3 = RotationState::new(counter_file.clone(), enabled.clone()).unwrap();
    assert_eq!(
        state3.pick_script_type().await.unwrap(),
        ScriptType::P2wpkh,
        "cycle 3 (counter=3, 3%3=0): expected P2wpkh (round-robin wrap)"
    );

    eprintln!(
        "ROTATION assertion PASSED: P2wpkh → P2tr → P2shP2wpkh → P2wpkh (counter 0/1/2/3)"
    );

    // ----- Part 2: end-to-end liquidity_bot::run via [lib] target -----
    //
    // Drives one full P2WPKH round via the extracted `liquidity_bot::run(config)`
    // function. This verifies the Pitfall 4 mitigation (lib extraction) is
    // observable at the test boundary.
    //
    // Setup: 3 P2WPKH UTXOs funded above.
    //   - UTXO[0] → bot's BotConfig (liquidity_bot::run drives this)
    //   - UTXO[1] + UTXO[2] → 2 concurrent peers (complete the 3-participant round)

    let handle0 = &setup.utxos[0];
    let handle1 = &setup.utxos[1];
    let handle2 = &setup.utxos[2];

    let wif0 = PrivateKey::new(handle0.secret_key, Network::Regtest).to_wif();
    let utxo_str0 = format!("{}:{}", handle0.outpoint.txid, handle0.outpoint.vout);

    let wif1 = PrivateKey::new(handle1.secret_key, Network::Regtest).to_wif();
    let utxo_str1 = format!("{}:{}", handle1.outpoint.txid, handle1.outpoint.vout);

    let wif2 = PrivateKey::new(handle2.secret_key, Network::Regtest).to_wif();
    let utxo_str2 = format!("{}:{}", handle2.outpoint.txid, handle2.outpoint.vout);

    // Spawn in-process v1.4 coordinator.
    let (coordinator_url, _tmp_dir) = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    )
    .await;
    wait_for_coordinator(&coordinator_url).await;

    // Use a separate counter file for the e2e portion (hermetic isolation from Part 1).
    let e2e_dir = tempfile::tempdir().expect("tempdir for e2e counter");
    let e2e_counter_file = e2e_dir.path().join("bot_e2e_counter");

    // Spawn 2 concurrent peer client tasks (UTXO[1] + UTXO[2]).
    // These complete the 3-participant minimum so the round can proceed.
    let url1 = coordinator_url.clone();
    let url2 = coordinator_url.clone();

    let wallet1 = client::wallet::BdkClientWallet::from_wif(&wif1, &utxo_str1, Network::Regtest)
        .expect("peer wallet1 from_wif");
    let wallet2 = client::wallet::BdkClientWallet::from_wif(&wif2, &utxo_str2, Network::Regtest)
        .expect("peer wallet2 from_wif");

    let coord_info1 = v14_coordinator_info(wallet1.script_type());
    let coord_info2 = v14_coordinator_info(wallet2.script_type());

    let peer1 = tokio::spawn(async move {
        let http = client::http::CoordinatorClient::new(url1);
        let info = http
            .poll_until_phase("input_reg", 100, Duration::from_secs(600))
            .await
            .expect("peer1: poll for input_reg");
        let reg = client::round::input::register_input(&http, &wallet1, &info, &coord_info1)
            .await
            .expect("peer1: register_input");
        http.poll_until_phase("output_reg", 100, Duration::from_secs(600))
            .await
            .expect("peer1: poll for output_reg");
        client::round::output::register_output(&http, &wallet1, &reg, &info)
            .await
            .expect("peer1: register_output");
        http.poll_until_phase("signing", 100, Duration::from_secs(600))
            .await
            .expect("peer1: poll for signing");
        client::round::sign::verify_and_sign(&http, &wallet1, &reg, 1, None)
            .await
            .expect("peer1: verify_and_sign");
    });

    let peer2 = tokio::spawn(async move {
        let http = client::http::CoordinatorClient::new(url2);
        let info = http
            .poll_until_phase("input_reg", 100, Duration::from_secs(600))
            .await
            .expect("peer2: poll for input_reg");
        let reg = client::round::input::register_input(&http, &wallet2, &info, &coord_info2)
            .await
            .expect("peer2: register_input");
        http.poll_until_phase("output_reg", 100, Duration::from_secs(600))
            .await
            .expect("peer2: poll for output_reg");
        client::round::output::register_output(&http, &wallet2, &reg, &info)
            .await
            .expect("peer2: register_output");
        http.poll_until_phase("signing", 100, Duration::from_secs(600))
            .await
            .expect("peer2: poll for signing");
        client::round::sign::verify_and_sign(&http, &wallet2, &reg, 1, None)
            .await
            .expect("peer2: verify_and_sign");
    });

    // Drive the bot via liquidity_bot::run (Pitfall 4 gate — lib extraction).
    let config = BotConfig {
        coordinator_url: coordinator_url.clone(),
        // Bot runs against regtest (integration test environment).
        // The signet guard in main.rs is NOT invoked by run() — main.rs owns
        // the BLINDJOIN_NETWORK check; lib::run() is network-agnostic.
        network: Network::Regtest,
        enabled_types: vec![ScriptType::P2wpkh],
        counter_file: e2e_counter_file.clone(),
        p2wpkh_tuple: Some(P2wpkhTuple {
            utxo: utxo_str0,
            wif: wif0,
        }),
        p2tr_tuple: None,
        p2sh_p2wpkh_tuple: None,
    };

    run(config).await.expect("liquidity_bot::run must succeed");

    // Wait for peers to finish.
    peer1.await.expect("peer1 task panicked");
    peer2.await.expect("peer2 task panicked");

    // Assert: counter file bumped to "1" after successful round.
    assert!(
        e2e_counter_file.exists(),
        "counter file must exist after successful round"
    );
    assert_eq!(
        tokio::fs::read_to_string(&e2e_counter_file).await.unwrap().trim(),
        "1",
        "counter file must contain '1' after first successful round"
    );

    eprintln!(
        "INTEG-02 E2E assertion PASSED: liquidity_bot::run completed; \
         counter file bumped to '1'"
    );
}
