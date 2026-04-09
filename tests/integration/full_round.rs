//! Integration test: 3-client CoinJoin round on regtest via corepc-node.
//!
//! This test requires `bitcoind` in PATH (or BITCOIND_EXE env var set).
//! It is automatically skipped if bitcoind is not available (graceful skip).
//!
//! Run: cargo test --test integration full_round -- --nocapture
//!
//! Threat model compliance:
//!   T-06-02: Integration tests use the same HTTP API as real clients —
//!            no test-only backdoors in coordinator code paths.
//!   T-06-03: Uses port 0 (OS assigns free port) to avoid conflicts.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Helper: initialise a round state in InputReg with a fresh RSA key.
// ---------------------------------------------------------------------------
fn build_input_reg_round_state() -> coordinator::round::state::RoundState {
    use coordinator::blind::rsa::RsaBlindSigner;
    use coordinator::round::state::{Phase, RoundState, RoundStateInner};
    use std::collections::HashMap;

    let signer = RsaBlindSigner::generate().expect("RSA keygen");
    let sk_der = signer.secret_key_der().expect("sk to DER");
    let pk_der = signer.public_key_spki_der().expect("pk to SPKI DER");
    let pk_hash = signer.public_key_hash();

    // Fixed test round secret (non-zero so HMAC generates valid tokens)
    let mut round_secret = [0u8; 32];
    for (i, b) in round_secret.iter_mut().enumerate() {
        *b = (i as u8).wrapping_add(1);
    }

    let mut state = RoundState::new_idle();
    state.rsa_pubkey_hash = Some(pk_hash);
    state.rsa_pubkey_der = Some(pk_der);
    state.inner = Some(RoundStateInner {
        rsa_signing_key: sk_der,
        round_secret,
        registered_inputs: HashMap::new(),
        redeemed_tokens: std::collections::HashSet::new(),
        registered_outputs: Vec::new(),
        partial_sigs: HashMap::new(),
        change_addresses: HashMap::new(),
    });
    // Transition Idle → InputReg
    state.transition_to(Phase::InputReg).expect("transition to InputReg");

    state
}

// ---------------------------------------------------------------------------
// Helper: spawn coordinator server in-process and return its listen URL.
// ---------------------------------------------------------------------------
async fn spawn_coordinator(
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
) -> String {
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, NetworkConfig};

    // Bind to port 0 (OS assigns an ephemeral port). Keep the listener open —
    // pass it directly into axum::serve to avoid the TOCTOU race where the port
    // could be claimed by another process between drop() and re-bind.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let listen_addr = addr.to_string();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: rpc_url.clone(),
            bitcoin_rpc_user: rpc_user.clone(),
            bitcoin_rpc_pass: rpc_pass.clone(),
        },
        coordinator: CoordinatorSection {
            denomination_sats: 100_000, // small denomination for test speed
            min_participants: 3,
            max_participants: 3,
            round_timeout_input_reg_secs: 30,
            round_timeout_output_reg_secs: 30,
            round_timeout_signing_secs: 15,
            blame_ban_duration_secs: 60,
            fee_rate_sat_per_vbyte: 1,
            listen_addr: listen_addr.clone(),
        },
    });

    let rpc = Arc::new(BitcoinRpc::new(rpc_url, rpc_user, rpc_pass));
    // Start coordinator in InputReg so clients can register immediately
    let round_state = Arc::new(RwLock::new(build_input_reg_round_state()));
    let app = coordinator::api::build_router(round_state, rpc, cfg);

    // Use the already-bound listener directly — no drop/re-bind race.
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", addr)
}

// ---------------------------------------------------------------------------
// Helper: wait for coordinator /info to return 200 OK.
// ---------------------------------------------------------------------------
async fn wait_for_coordinator(coordinator_url: &str) {
    let http_client = reqwest::Client::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let ok = http_client
            .get(format!("{}/info", coordinator_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Coordinator did not start within 5 seconds"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

// ---------------------------------------------------------------------------
// Data gathered from the synchronous bitcoind setup phase.
// ---------------------------------------------------------------------------
struct FundedSetup {
    rpc_url: String,
    rpc_user: String,
    rpc_pass: String,
    /// (outpoint "txid:vout", value_sats) for each participant
    utxos: [(String, u64); 3],
}

// ---------------------------------------------------------------------------
// Main integration test: full 3-client CoinJoin round on regtest.
// ---------------------------------------------------------------------------

/// Full 3-client CoinJoin round on regtest.
///
/// Steps:
/// 1. Skip if bitcoind unavailable (graceful skip — not a failure)
/// 2. Start regtest bitcoind via corepc-node
/// 3. Mine 101 blocks + fund 3 test P2WPKH addresses
/// 4. Mine 1 confirmation block
/// 5. Spawn coordinator in-process (InputReg phase, denomination=100_000)
/// 6. Run 3 concurrent client tasks (input→output→sign)
/// 7. Assert CoinJoin tx appears in bitcoind mempool
/// 8. Verify the transaction has 3 outputs of 100_000 sats
#[tokio::test]
async fn full_round_three_clients() {
    // ----- Step 1: skip if bitcoind not available -----
    let exe = match corepc_node::exe_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bitcoind not found ({}), skipping full_round_three_clients", e);
            return;
        }
    };

    // ----- Steps 2-4: all synchronous bitcoind work in one spawn_blocking -----
    // corepc-node's Client is not Clone, so we do all sync work here and
    // then leak the node to keep bitcoind alive for the coordinator's RPC calls.
    let setup: FundedSetup = tokio::task::spawn_blocking(move || {
        use bitcoin::{
            secp256k1::Secp256k1, Address, Amount, CompressedPublicKey, Network, PrivateKey,
        };
        use corepc_node::{Conf, Node};

        let mut conf = Conf::default();
        conf.network = "regtest";

        let node = Node::with_conf(&exe, &conf).expect("start regtest bitcoind");

        // Extract RPC credentials from cookie file
        let cookie = node
            .params
            .get_cookie_values()
            .expect("read cookie file")
            .expect("parse cookie values");
        let rpc_url = node.rpc_url();
        let rpc_user = cookie.user.clone();
        let rpc_pass = cookie.password.clone();

        // Hardcoded regtest WIF keys — REGTEST ONLY, zero monetary value.
        let test_wifs = [
            "cNJFgo1driFnPcBdBX8BrJrpxchBW2gSBicFB6Qz4JHaFKkLYVvQ",
            "cMa6jLZEigizHJkuFQ4RJ6D8nPRSKyEMDsKBvpEkTqGmtXsKxsgU",
            "cMkJRaXzKsRFvuQUbNPVMjXkHeHHTkQTEOWaabQJDRrXDmDx1RCe",
        ];
        let denomination: u64 = 100_000;
        let fund_sats: u64 = denomination + 50_000; // covers denomination + fee margin
        let fund_btc = Amount::from_sat(fund_sats);

        let secp = Secp256k1::new();

        // Derive P2WPKH addresses for each test key (regtest)
        let utxo_addresses: Vec<Address> = test_wifs
            .iter()
            .map(|wif| {
                let sk = PrivateKey::from_wif(wif).unwrap();
                let raw_pk =
                    bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk.inner);
                let cpk = CompressedPublicKey(raw_pk);
                Address::p2wpkh(&cpk, Network::Regtest)
            })
            .collect();

        // Mine 101 blocks to mature coinbase UTXOs
        let mine_addr: Address = node.client.new_address().expect("get new address");
        node.client
            .generate_to_address(101, &mine_addr)
            .expect("generate 101 blocks");

        // Fund each test address
        let mut funding_txids: Vec<String> = Vec::new();
        for addr in &utxo_addresses {
            let txid_result = node
                .client
                .send_to_address(addr, fund_btc)
                .expect("send_to_address");
            // SendToAddress is a newtype: SendToAddress(pub String)
            funding_txids.push(txid_result.0.clone());
        }

        // Mine 1 confirmation block
        node.client
            .generate_to_address(1, &mine_addr)
            .expect("generate confirmation block");

        // Locate each funded UTXO via list_unspent
        // ListUnspent wraps Vec<ListUnspentItem> where fields are plain Strings/f64
        let unspent_result = node.client.list_unspent().expect("list_unspent");
        let unspent_items = &unspent_result.0;

        let addr_strs: Vec<String> = utxo_addresses.iter().map(|a| a.to_string()).collect();

        let utxos_vec: Vec<(String, u64)> = funding_txids
            .iter()
            .zip(addr_strs.iter())
            .map(|(txid_str, addr_str)| {
                let entry = unspent_items
                    .iter()
                    .find(|u| {
                        u.txid == *txid_str && u.address == *addr_str
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "Could not find funded UTXO for txid={} addr={}",
                            txid_str, addr_str
                        )
                    });
                let outpoint = format!("{}:{}", entry.txid, entry.vout);
                // entry.amount is in BTC (f64); convert to sats
                let value_sats = (entry.amount * 100_000_000.0).round() as u64;
                (outpoint, value_sats)
            })
            .collect();

        assert_eq!(utxos_vec.len(), 3, "Must have 3 funded UTXOs");

        // Leak the node so bitcoind stays alive for the coordinator's RPC calls.
        // This is acceptable in a test — OS reaps the process at test exit.
        let node_box = Box::new(node);
        Box::leak(node_box);

        FundedSetup {
            rpc_url,
            rpc_user,
            rpc_pass,
            utxos: [utxos_vec[0].clone(), utxos_vec[1].clone(), utxos_vec[2].clone()],
        }
    })
    .await
    .expect("setup spawn_blocking panicked");

    let denomination: u64 = 100_000;

    // ----- Step 5: spawn coordinator in-process -----
    let coordinator_url = spawn_coordinator(
        setup.rpc_url.clone(),
        setup.rpc_user.clone(),
        setup.rpc_pass.clone(),
    )
    .await;
    wait_for_coordinator(&coordinator_url).await;

    // ----- Step 6: run 3 concurrent client tasks -----
    let test_wifs = [
        "cNJFgo1driFnPcBdBX8BrJrpxchBW2gSBicFB6Qz4JHaFKkLYVvQ",
        "cMa6jLZEigizHJkuFQ4RJ6D8nPRSKyEMDsKBvpEkTqGmtXsKxsgU",
        "cMkJRaXzKsRFvuQUbNPVMjXkHeHHTkQTEOWaabQJDRrXDmDx1RCe",
    ];

    let handles: Vec<_> = test_wifs
        .iter()
        .enumerate()
        .map(|(i, wif)| {
            let url = coordinator_url.clone();
            let wif = wif.to_string();
            let (utxo_str, utxo_value) = setup.utxos[i].clone();

            tokio::spawn(async move {
                use bitcoin::Network;
                use client::http::CoordinatorClient;
                use client::round;
                use client::wallet::ClientWallet;

                let wallet =
                    ClientWallet::from_wif(&wif, &utxo_str, utxo_value, Network::Regtest)
                        .expect("ClientWallet creation");
                let coordinator_client = CoordinatorClient::new(url);

                // Poll until INPUT_REG (coordinator already starts in input_reg)
                let info = coordinator_client
                    .poll_until_phase("input_reg", 100)
                    .await
                    .expect("poll for input_reg");

                let reg = round::input::register_input(&coordinator_client, &wallet, &info)
                    .await
                    .expect("register_input");

                // Poll until OUTPUT_REG
                coordinator_client
                    .poll_until_phase("output_reg", 100)
                    .await
                    .expect("poll for output_reg");

                round::output::register_output(&coordinator_client, &wallet, &reg, &info)
                    .await
                    .expect("register_output");

                // Poll until SIGNING
                coordinator_client
                    .poll_until_phase("signing", 100)
                    .await
                    .expect("poll for signing");

                round::sign::verify_and_sign(&coordinator_client, &wallet, &reg, 100)
                    .await
                    .expect("verify_and_sign");
            })
        })
        .collect();

    // Wait for all 3 clients to complete
    for handle in handles {
        handle.await.expect("client task panicked");
    }

    // ----- Step 7: wait for broadcast, then check mempool -----
    // Coordinator broadcasts after all 3 signatures are collected.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let rpc_url = setup.rpc_url.clone();
    let rpc_user = setup.rpc_user.clone();
    let rpc_pass = setup.rpc_pass.clone();

    let mempool_txids: Vec<String> = tokio::task::spawn_blocking(move || {
        use corepc_node::client::client_sync::Auth;
        // Create a new client using the same credentials (not clone of node.client)
        let auth = Auth::UserPass(rpc_user, rpc_pass);
        // corepc_node::Client is the version-specific client (e.g., v28::Client)
        let client = corepc_node::Client::new_with_auth(&rpc_url, auth)
            .expect("create rpc client for mempool check");
        // GetRawMempool(Vec<String>) — txids as hex strings
        client.get_raw_mempool().expect("get_raw_mempool").0
    })
    .await
    .expect("mempool check spawn_blocking");

    assert!(
        !mempool_txids.is_empty(),
        "CoinJoin tx must appear in mempool after all 3 clients submit signatures. \
         The coordinator broadcasts in signing.rs assemble_and_broadcast after collecting \
         all partial sigs. If this assertion fails, check that testmempoolaccept accepts \
         the transaction (Phase 1 broadcasts unsigned tx — see signing.rs)."
    );

    let coinjoin_txid_str = mempool_txids[0].clone();
    eprintln!("CoinJoin txid in mempool: {}", coinjoin_txid_str);

    // ----- Step 8: verify the transaction has 3 denomination outputs -----
    let rpc_url2 = setup.rpc_url.clone();
    let rpc_user2 = setup.rpc_user.clone();
    let rpc_pass2 = setup.rpc_pass.clone();
    let txid_for_verify = coinjoin_txid_str.clone();

    let denom_output_count: usize = tokio::task::spawn_blocking(move || {
        use std::str::FromStr;
        use corepc_node::client::client_sync::Auth;
        let auth = Auth::UserPass(rpc_user2, rpc_pass2);
        let client = corepc_node::Client::new_with_auth(&rpc_url2, auth)
            .expect("create rpc client for tx verify");
        let txid = bitcoin::Txid::from_str(&txid_for_verify).unwrap();
        // GetRawTransactionVerbose has field `outputs: Vec<RawTransactionOutput>`
        // RawTransactionOutput has `value: f64` (in BTC)
        let tx = client
            .get_raw_transaction_verbose(txid)
            .expect("get_raw_transaction_verbose");
        tx.outputs
            .iter()
            .filter(|out| {
                // value is BTC (f64); convert to sats
                let sats = (out.value * 100_000_000.0).round() as u64;
                sats == denomination
            })
            .count()
    })
    .await
    .expect("tx verify spawn_blocking");

    assert_eq!(
        denom_output_count, 3,
        "CoinJoin tx must have exactly 3 denomination outputs of {} sats; got {}",
        denomination, denom_output_count
    );

    eprintln!(
        "Integration test PASSED: CoinJoin round complete, txid={}, {} denomination outputs",
        coinjoin_txid_str, denom_output_count
    );
}

// ---------------------------------------------------------------------------
// Smoke test: /info endpoint fields, no bitcoind required.
// ---------------------------------------------------------------------------

/// Verify /info response fields when coordinator is in Idle state.
/// GET /info reads only in-memory round state — no bitcoind RPC calls needed.
#[tokio::test]
async fn coordinator_info_endpoint_fields() {
    use coordinator::api::build_router;
    use coordinator::bitcoin::rpc::BitcoinRpc;
    use coordinator::config::{CoordinatorConfig, CoordinatorSection, NetworkConfig};
    use coordinator::round::state::RoundState;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let cfg = Arc::new(CoordinatorConfig {
        network: NetworkConfig {
            bitcoin_network: "regtest".into(),
            bitcoin_rpc_url: "http://127.0.0.1:18443".into(),
            bitcoin_rpc_user: "test".into(),
            bitcoin_rpc_pass: "test".into(),
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
            listen_addr: addr.to_string(),
        },
    });

    let rpc = Arc::new(BitcoinRpc::new(
        "http://127.0.0.1:18443".into(),
        "test".into(),
        "test".into(),
    ));
    let round_state = Arc::new(RwLock::new(RoundState::new_idle()));
    let app = build_router(round_state, rpc, cfg.clone());

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for server to be ready
    let http_client = reqwest::Client::new();
    let coordinator_url = format!("http://{}", addr);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let ok = http_client
            .get(format!("{}/info", coordinator_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Coordinator smoke test server did not start within 3s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Fetch and verify /info fields
    let info: shared::protocol::InfoResponse = http_client
        .get(format!("{}/info", coordinator_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(info.round_state, "idle", "Coordinator starts in idle");
    assert_eq!(info.denomination_sats, 100_000);
    assert_eq!(info.min_participants, 3);
    assert_eq!(info.max_participants, 3);
    assert_eq!(info.network, "regtest");
    assert!(
        info.rsa_pubkey_hash.is_none(),
        "rsa_pubkey_hash must be None when idle"
    );
    assert!(
        info.rsa_pubkey_der_b64.is_none(),
        "rsa_pubkey_der_b64 must be None when idle"
    );
    assert!(info.round_id.is_some(), "round_id must always be present");
    assert!(!info.version.is_empty(), "version must be non-empty");

    eprintln!(
        "coordinator_info_endpoint_fields PASSED: round_state={}",
        info.round_state
    );
}
