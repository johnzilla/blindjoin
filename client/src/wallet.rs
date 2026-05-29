use bitcoin::{Network, OutPoint, Psbt, ScriptBuf, Txid};
use std::str::FromStr;
use anyhow::{anyhow, Result};
use bdk_wallet::{KeychainKind, Wallet};
#[allow(deprecated)]
use bdk_wallet::signer::SignOptions;

/// HD-wallet backed CoinJoin client using bdk_wallet 2.3.
///
/// Replaces the Phase 1 raw-WIF implementation (D-14). Supports three construction paths:
///   - from_wif: backward-compat constructor for integration tests (wpkh(WIF) descriptor)
///   - from_descriptor: BIP-84 xprv descriptor wallet from CLI --descriptor flag
///   - generate: generates a fresh BIP-84 wallet and prints descriptors to stdout
///
/// Address derivation uses bdk_wallet::Wallet::peek_address (index 0, no state mutation).
/// PSBT signing uses bdk_wallet::Wallet::sign with witness_utxo populated.
pub struct BdkClientWallet {
    #[allow(dead_code)]
    pub network: Network,
    pub utxo_outpoint: OutPoint,
    /// The P2WPKH script_pubkey controlling the UTXO (needed for BIP-322 and PSBT signing).
    utxo_script_pubkey: ScriptBuf,
    /// The WIF key string, stored for secret_key_for_signing (WIF wallets only).
    wif_key: Option<String>,
    inner: Wallet,
}

impl BdkClientWallet {
    /// Backward-compat constructor: builds a single-key wpkh(WIF) descriptor wallet.
    ///
    /// Accepts the same arguments as the Phase 1 ClientWallet::from_wif.
    /// Integration tests use this path — the API contract is preserved.
    pub fn from_wif(
        wif: &str,
        utxo_outpoint_str: &str,
        network: Network,
    ) -> Result<Self> {
        let secret_key = bitcoin::PrivateKey::from_wif(wif)?;

        // Derive P2WPKH script for the UTXO (same as Phase 1)
        let utxo_script_pubkey = {
            use bitcoin::CompressedPublicKey;
            let secp = bitcoin::secp256k1::Secp256k1::new();
            let raw_pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &secret_key.inner);
            let cpk = CompressedPublicKey(raw_pk);
            ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash())
        };

        let bdk_net = bdk_network(network);
        // wpkh(WIF) is a single-key descriptor with no change keychain. bdk_wallet 2.3
        // rejects Wallet::create(d, d) with "External and internal descriptors are the
        // same" — use Wallet::create_single(d) instead, which is bdk's purpose-built
        // API for keychain-less wallets (added in 2.x for exactly this case).
        let descriptor = format!("wpkh({})", wif);
        let inner = Wallet::create_single(descriptor)
            .network(bdk_net)
            .create_wallet_no_persist()
            .map_err(|e| anyhow!("Failed to create bdk wallet from WIF: {e}"))?;

        let outpoint = parse_outpoint(utxo_outpoint_str)?;
        Ok(Self {
            network,
            utxo_outpoint: outpoint,
            utxo_script_pubkey,
            wif_key: Some(wif.to_string()),
            inner,
        })
    }

    /// Build a wallet from user-provided BIP-84 xprv descriptor strings.
    ///
    /// external_desc: e.g. "wpkh(xprv.../84'/0'/0'/0/*)"
    /// utxo_address: bech32 address of the UTXO being registered (needed to derive
    ///   utxo_script_pubkey without chain data).
    pub fn from_descriptor(
        external_desc: &str,
        utxo_outpoint_str: &str,
        utxo_address: &str,
        network: Network,
    ) -> Result<Self> {
        // Derive internal (change) descriptor by replacing last "/0/*" with "/1/*"
        let internal_desc = if external_desc.contains("/0/*)") {
            external_desc.replacen("/0/*)", "/1/*)", 1)
        } else {
            // Fallback: use external for both (single-key non-derivation case)
            external_desc.to_string()
        };

        let bdk_net = bdk_network(network);
        let inner = Wallet::create(external_desc.to_string(), internal_desc)
            .network(bdk_net)
            .create_wallet_no_persist()
            .map_err(|e| anyhow!("Failed to create bdk wallet from descriptor: {e}"))?;

        // Derive utxo_script_pubkey from the provided bech32 address
        let addr = bitcoin::Address::from_str(utxo_address)
            .map_err(|e| anyhow!("Invalid --utxo-address: {e}"))?
            .require_network(network)
            .map_err(|e| anyhow!("Address network mismatch: {e}"))?;
        let utxo_script_pubkey = addr.script_pubkey();

        let outpoint = parse_outpoint(utxo_outpoint_str)?;
        Ok(Self {
            network,
            utxo_outpoint: outpoint,
            utxo_script_pubkey,
            wif_key: None,
            inner,
        })
    }

    /// Generate a fresh BIP-84 (P2WPKH) wallet using a random mnemonic.
    ///
    /// Prints the external and internal descriptors to stdout with a prominent warning.
    /// Also writes a descriptors.txt file in cwd with 0600 permissions (T-03-04 mitigation).
    /// Returns a wallet ready for immediate use in the current round.
    pub fn generate(
        utxo_outpoint_str: &str,
        network: Network,
    ) -> Result<Self> {
        use bdk_wallet::keys::GeneratableKey;
        use bdk_wallet::keys::bip39::{Mnemonic, Language, WordCount};
        use bdk_wallet::keys::DerivableKey;
        use bdk_wallet::keys::ExtendedKey;

        let bdk_net = bdk_network(network);

        // Generate a fresh 12-word BIP-39 mnemonic
        let mnemonic = Mnemonic::generate((WordCount::Words12, Language::English))
            .map_err(|_| anyhow!("BIP-39 mnemonic generation failed"))?;
        let mnemonic_str = mnemonic.to_string();

        // Derive xprv from mnemonic (no passphrase)
        let xkey: ExtendedKey = mnemonic
            .into_extended_key()
            .map_err(|e| anyhow!("Failed to derive extended key: {e}"))?;
        let xprv = xkey.into_xprv(bdk_net)
            .ok_or_else(|| anyhow!("Failed to get xprv from extended key"))?;

        let external_desc = format!("wpkh({}/84'/0'/0'/0/*)", xprv);
        let internal_desc = format!("wpkh({}/84'/0'/0'/1/*)", xprv);

        let inner = Wallet::create(external_desc.clone(), internal_desc.clone())
            .network(bdk_net)
            .create_wallet_no_persist()
            .map_err(|e| anyhow!("Failed to create bdk wallet from generated key: {e}"))?;

        // The first external address (m/84'/0'/0'/0/0) is the only address this
        // wallet can sign for in a round when called via generate(). Surface it
        // prominently so the user funds the right place — funds sent to any other
        // derivation will not produce valid signatures.
        let first_address = inner.peek_address(KeychainKind::External, 0).address;

        // T-03-04: print prominent warning and write descriptors.txt with restricted permissions
        println!();
        println!("=============================================================");
        println!("  WARNING: MASTER PRIVATE KEY MATERIAL — KEEP SECURE");
        println!("=============================================================");
        println!("Mnemonic (12 words — BACK THIS UP):");
        println!("  {}", mnemonic_str);
        println!();
        println!("External descriptor (receiving addresses):");
        println!("  {}", external_desc);
        println!();
        println!("Internal descriptor (change addresses):");
        println!("  {}", internal_desc);
        println!();
        println!("SAVE these descriptors. They are your wallet. Anyone with");
        println!("these descriptors can spend all funds derived from this key.");
        println!("=============================================================");
        println!();
        println!("=============================================================");
        println!("  FUND THIS ADDRESS TO PARTICIPATE IN A ROUND:");
        println!("=============================================================");
        println!("  {}", first_address);
        println!();
        println!("  (BIP-84 path: m/84'/0'/0'/0/0)");
        println!();
        println!("This is the wallet's first external address. The signer will");
        println!("ONLY produce valid signatures for a UTXO at this address.");
        println!("Funds sent to any other derivation will fail to sign.");
        println!("=============================================================");
        println!();

        // Write to descriptors.txt with 0600 permissions
        let content = format!(
            "# blindjoin wallet descriptors\n\
             # WARNING: This file contains MASTER PRIVATE KEY MATERIAL.\n\
             # Anyone with this file can spend your funds. Keep it secure.\n\n\
             mnemonic={}\n\
             external_descriptor={}\n\
             internal_descriptor={}\n\
             fund_address={}\n",
            mnemonic_str, external_desc, internal_desc, first_address
        );
        std::fs::write("descriptors.txt", &content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions("descriptors.txt", std::fs::Permissions::from_mode(0o600))?;
        }
        println!("Descriptors also written to: descriptors.txt (permissions: 0600)");

        let outpoint = parse_outpoint(utxo_outpoint_str)?;
        let utxo_script_pubkey = first_address.script_pubkey();

        Ok(Self {
            network,
            utxo_outpoint: outpoint,
            utxo_script_pubkey,
            wif_key: None,
            inner,
        })
    }

    /// Expose the inner secp256k1 secret key for BIP-322 signing (WIF wallets only).
    ///
    /// Used by generate_bip322_witness in input.rs. Only valid for from_wif wallets.
    /// Panics for descriptor/generated wallets — those should use wallet.sign() directly.
    pub fn secret_key_for_signing(&self) -> bitcoin::secp256k1::SecretKey {
        let wif = self.wif_key.as_deref()
            .expect("secret_key_for_signing: not available for descriptor wallets. \
                     Use wallet.sign() for BIP-322 with non-WIF wallets.");
        bitcoin::PrivateKey::from_wif(wif)
            .expect("stored WIF is always valid")
            .inner
    }

    /// Returns the P2WPKH script_pubkey for the UTXO being registered.
    pub fn script_pubkey(&self) -> ScriptBuf {
        self.utxo_script_pubkey.clone()
    }

    /// Derive the receive address for the CoinJoin output (index 0).
    ///
    /// Uses peek_address — no wallet state mutation needed for single-use CLI wallet.
    pub fn coinjoin_output_address(&self) -> bitcoin::Address {
        self.inner.peek_address(KeychainKind::External, 0).address
    }

    /// Derive the change address (index 0, internal keychain).
    ///
    /// Uses peek_address — no wallet state mutation needed for single-use CLI wallet.
    pub fn change_address(&self) -> bitcoin::Address {
        self.inner.peek_address(KeychainKind::Internal, 0).address
    }

    /// Sign a PSBT input corresponding to our UTXO.
    ///
    /// Trusts the coordinator's witness_utxo (sourced from Bitcoin Core gettxout
    /// at registration time). Returns the consensus-serialized Witness for the
    /// signed input so the coordinator can deserialize it on /round/sign.
    pub fn sign_psbt_input(&self, psbt: &mut Psbt) -> Result<Vec<u8>> {
        // Find our input in the PSBT
        let input_idx = psbt.unsigned_tx.input.iter()
            .position(|inp| inp.previous_output == self.utxo_outpoint)
            .ok_or_else(|| anyhow!("Our UTXO not found in PSBT"))?;

        // trust_witness_utxo: true is required because we sign over a segwit witness_utxo
        // without populating non_witness_utxo (which would require fetching the full prevout
        // tx via RPC — a non-goal for the CLI client). The coordinator's witness_utxo is
        // authoritative: validate_utxo populates it from Bitcoin Core's gettxout at
        // registration time, before any participant signs. A malicious coordinator that
        // lies about value cannot steal funds (bitcoind validates the broadcast against
        // the real on-chain UTXO and rejects mismatched signatures — the attacker gets DoS,
        // not theft). The deprecation marker stays until a future migration to non_witness_utxo
        // (requires an RPC client in the wallet — out of scope for v1.x CLI).
        #[allow(deprecated)]
        self.inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..SignOptions::default() })
            .map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;

        // Encode the signed witness as consensus-serialized bytes so the coordinator's
        // bitcoin::consensus::deserialize::<Witness> round-trips. bdk_wallet finalizes
        // single-key P2WPKH inputs (sets final_script_witness), so prefer that;
        // fall back to constructing a 2-item stack from partial_sigs if not finalized.
        let input = &psbt.inputs[input_idx];
        if let Some(witness) = &input.final_script_witness {
            return Ok(bitcoin::consensus::serialize(witness));
        }
        if let Some((pk, sig)) = input.partial_sigs.iter().next() {
            let mut witness = bitcoin::Witness::new();
            witness.push(sig.to_vec());
            witness.push(pk.to_bytes());
            return Ok(bitcoin::consensus::serialize(&witness));
        }

        Err(anyhow!("bdk_wallet did not produce a witness for our input"))
    }
}

/// Type alias for backward compatibility. All callers use ClientWallet.
pub type ClientWallet = BdkClientWallet;

/// Parse a "txid:vout" outpoint string.
pub fn parse_outpoint(s: &str) -> Result<OutPoint> {
    let mut parts = s.splitn(2, ':');
    let txid_str = parts.next().ok_or_else(|| anyhow!("Missing txid"))?;
    let vout_str = parts.next().ok_or_else(|| anyhow!("Missing vout"))?;
    let txid = Txid::from_str(txid_str)?;
    let vout: u32 = vout_str.parse()?;
    Ok(OutPoint::new(txid, vout))
}

/// Map bitcoin::Network to bdk_wallet::bitcoin::Network.
///
/// Both crates re-export bitcoin 0.32 — these are identical types, no conversion needed.
fn bdk_network(network: Network) -> bdk_wallet::bitcoin::Network {
    network
}
