use clap::Parser;
use tracing::info;

mod config;
mod discover;
mod http;
mod round;
mod tor;
mod wallet;

use config::ClientConfig;
use http::CoordinatorClient;
use wallet::ClientWallet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("client=info".parse().unwrap()))
        .init();

    let cfg = ClientConfig::parse();
    let network = match cfg.network.as_str() {
        "signet" => bitcoin::Network::Signet,
        "testnet4" => bitcoin::Network::Testnet,
        "mainnet" => bitcoin::Network::Bitcoin,
        other => anyhow::bail!("Unknown network: {other}"),
    };

    // --generate-wallet: print descriptors and exit (no round participation needed)
    if cfg.generate_wallet {
        let utxo = cfg.utxo.as_deref().unwrap_or("0000000000000000000000000000000000000000000000000000000000000000:0");
        ClientWallet::generate(utxo, network, cfg.script_type, !cfg.no_print_secrets)?;
        std::process::exit(0);
    }

    // Require --utxo for round participation. --utxo-value-sats is no longer
    // required: the coordinator queries Bitcoin Core's gettxout at registration
    // and supplies the real value via the PSBT's witness_utxo. The CLI flag is
    // accepted for backward compat but ignored.
    let utxo = cfg.utxo.as_deref()
        .ok_or_else(|| anyhow::anyhow!("--utxo is required for round participation"))?;

    let wallet = if let Some(descriptor) = cfg.descriptor.as_deref() {
        // --descriptor path: requires --utxo-address
        let utxo_address = cfg.utxo_address.as_deref()
            .ok_or_else(|| anyhow::anyhow!("--utxo-address is required when using --descriptor"))?;
        ClientWallet::from_descriptor(descriptor, utxo, utxo_address, network, cfg.script_type)?
    } else {
        // WIF path (backward compat, default for testing). Per Phase 17 D-61
        // from_wif takes NO script_type parameter; the wallet's script_type is
        // hardcoded to P2WPKH inside from_wif so the v1.3 cross-phase invariant
        // (tests/integration/full_round.rs) stays bit-exact unchanged.
        let wif = cfg.utxo_wif.as_deref()
            .ok_or_else(|| anyhow::anyhow!("--utxo-wif is required when not using --descriptor or --generate-wallet"))?;
        ClientWallet::from_wif(wif, utxo, network)?
    };

    // CLI-01: If --pkarr-pubkey is provided, resolve coordinator URL from DHT.
    //
    // WALLET-03: fail-fast runs here, BEFORE any Tor branch. Structural
    // ordering, not a runtime hack — the `discover::discover_coordinator`
    // call site runs UNCONDITIONALLY at this line, before the
    // `if cfg.use_tor` branch below at the `tor::init_tor` call site. Per
    // RESEARCH Pitfall 4 + D-74 a future refactor that moves the discover
    // call inside the Tor branch would silently break WALLET-03; this
    // comment is the in-source proof of the structural invariant.
    let coordinator_info = if let Some(ref pkarr_key) = cfg.pkarr_pubkey {
        let info = discover::discover_coordinator(pkarr_key, wallet.script_type())
            .await
            .map_err(|e| anyhow::anyhow!("PKARR discovery failed: {e}"))?;
        if info.capabilities.is_legacy {
            // CD-21 legacy-coordinator detection log — `coordinator_pubkey`
            // is public DHT data, `record_version` is the wire schema
            // version. No PII; symmetric with the structured-field
            // logging discipline elsewhere in the project.
            tracing::warn!(
                coordinator_pubkey = %pkarr_key,
                record_version = %info.capabilities.record_version,
                "Detected legacy v1.3 coordinator — using v1 OwnershipProof shim (WALLET-04)"
            );
        }
        info
    } else {
        // Non-PKARR path: the user pointed the client at --coordinator-url
        // directly (out-of-band trust per T-17-03-05). Construct a synthetic
        // v1.4 CoordinatorInfo that defaults `is_legacy: false` + supports
        // all 3 script types + output_script_type matching the wallet. The
        // operator is responsible for matching the wallet to a compatible
        // coordinator; a v1.4 client pointed at a v1.3 coordinator via
        // --coordinator-url will emit a v=2 envelope which the v1.3
        // coordinator rejects (graceful UX downgrade vs silent compat
        // failure).
        discover::CoordinatorInfo {
            coordinator_url: cfg.coordinator_url.clone(),
            capabilities: discover::CoordinatorCapabilities {
                record_version: "manual".to_string(),
                is_legacy: false,
                supported_script_types: vec![
                    shared::bip322::ScriptType::P2wpkh,
                    shared::bip322::ScriptType::P2tr,
                    shared::bip322::ScriptType::P2shP2wpkh,
                ],
                output_script_type: wallet.script_type(),
            },
        }
    };
    let coordinator_url = coordinator_info.coordinator_url.clone();
    // CLI-05: when --tor is set, use two isolated Tor circuits (alice for input reg,
    // bob for output reg). Otherwise fall back to plain clearnet reqwest.
    let client = if cfg.use_tor {
        let handle = tor::init_tor(coordinator_url.clone()).await
            .map_err(|e| anyhow::anyhow!("Tor initialization failed: {e}"))?;
        let alice_proxy = handle.alice_proxy_url().to_owned();
        let bob_proxy = handle.bob_proxy_url().to_owned();
        CoordinatorClient::new_tor(coordinator_url, alice_proxy, bob_proxy)?
    } else {
        CoordinatorClient::new(coordinator_url)
    };

    // 10-minute ceiling per phase — prevents infinite hangs if coordinator crashes.
    let phase_timeout = tokio::time::Duration::from_secs(600);

    info!("Polling for INPUT_REG phase");
    let info = client.poll_until_phase("input_reg", cfg.poll_interval_ms, phase_timeout).await?;
    info!(round_id = ?info.round_id, "INPUT_REG phase detected");

    // Phase 17 17-03: register_input now takes &CoordinatorInfo (replaces
    // the 17-02 transitional `is_legacy_coordinator: bool` 4th arg). The
    // v1/v2 envelope branch reads `coordinator_info.capabilities.is_legacy`.
    let reg_result = round::input::register_input(&client, &wallet, &info, &coordinator_info).await?;
    info!("Input registered successfully");

    client.poll_until_phase("output_reg", cfg.poll_interval_ms, phase_timeout).await?;
    info!("OUTPUT_REG phase detected");

    round::output::register_output(&client, &wallet, &reg_result, &info).await?;
    info!("Output registered successfully");

    client.poll_until_phase("signing", cfg.poll_interval_ms, phase_timeout).await?;
    info!("SIGNING phase detected");

    round::sign::verify_and_sign(&client, &wallet, &reg_result, cfg.min_anonymity_set, cfg.max_fee_sats).await?;
    info!("Signed successfully — round complete");

    Ok(())
}
