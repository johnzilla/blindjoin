use bitcoin::{Network, OutPoint, Psbt, ScriptBuf, Txid};
use bitcoin::hashes::Hash;
use std::str::FromStr;
use anyhow::Result;

/// Lightweight client wallet for CoinJoin participation.
///
/// **D-14 Phase 1 simplification:** Uses raw WIF private key and manual P2WPKH derivation
/// instead of bdk_wallet descriptor wallet. This is intentional — bdk_wallet with BIP-84
/// descriptors, `Wallet::create`, and `wallet.sign()` PSBT integration is implemented
/// in Phase 3. Do not refactor to bdk_wallet here.
///
/// Phase 1 capabilities: derive P2WPKH address/script from WIF key, sign PSBT inputs
/// via raw ECDSA sighash computation, generate BIP-322 ownership proofs.
pub struct ClientWallet {
    pub network: Network,
    pub utxo_outpoint: OutPoint,
    pub utxo_value_sats: u64,
    /// The private key controlling the UTXO (WIF format, for testing)
    secret_key: bitcoin::PrivateKey,
}

impl ClientWallet {
    pub fn from_wif(
        wif: &str,
        utxo_outpoint_str: &str,
        utxo_value_sats: u64,
        network: Network,
    ) -> Result<Self> {
        let secret_key = bitcoin::PrivateKey::from_wif(wif)?;
        let outpoint = parse_outpoint(utxo_outpoint_str)?;
        Ok(Self { network, utxo_outpoint: outpoint, utxo_value_sats, secret_key })
    }

    /// Expose the inner secp256k1 secret key for signing operations.
    pub fn secret_key_for_signing(&self) -> bitcoin::secp256k1::SecretKey {
        self.secret_key.inner
    }

    /// Returns the P2WPKH script_pubkey for this key
    pub fn script_pubkey(&self) -> ScriptBuf {
        use bitcoin::CompressedPublicKey;
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let raw_pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &self.secret_key.inner);
        let cpk = CompressedPublicKey(raw_pk);
        ScriptBuf::new_p2wpkh(&cpk.wpubkey_hash())
    }

    /// Derive a fresh receive address for the CoinJoin output.
    /// For Phase 1 simplicity: use a derived key from the UTXO key (deterministic).
    /// Phase 3 will use bdk_wallet for proper HD derivation.
    pub fn coinjoin_output_address(&self) -> bitcoin::Address {
        use bitcoin::{secp256k1::Secp256k1, CompressedPublicKey, Address};
        let secp = Secp256k1::new();
        // Simple derivation: tweak private key by 1 for a distinct output address
        let sk = self.secret_key.inner;
        let mut tweak_bytes = [0u8; 32];
        tweak_bytes[31] = 1u8;
        let scalar = bitcoin::secp256k1::Scalar::from_be_bytes(tweak_bytes).unwrap();
        let tweaked = sk.add_tweak(&scalar).unwrap();
        let raw_pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &tweaked);
        let cpk = CompressedPublicKey(raw_pk);
        Address::p2wpkh(&cpk, self.network)
    }

    /// Derive a change address (different from coinjoin output).
    pub fn change_address(&self) -> bitcoin::Address {
        use bitcoin::{secp256k1::Secp256k1, CompressedPublicKey, Address};
        let secp = Secp256k1::new();
        let sk = self.secret_key.inner;
        let mut tweak_bytes = [0u8; 32];
        tweak_bytes[31] = 2u8;
        let scalar = bitcoin::secp256k1::Scalar::from_be_bytes(tweak_bytes).unwrap();
        let tweaked = sk.add_tweak(&scalar).unwrap();
        let raw_pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &tweaked);
        let cpk = CompressedPublicKey(raw_pk);
        Address::p2wpkh(&cpk, self.network)
    }

    /// Sign a PSBT input corresponding to our UTXO.
    /// Returns the partial signature bytes for POST /round/sign.
    pub fn sign_psbt_input(&self, psbt: &mut Psbt) -> Result<Vec<u8>> {
        use bitcoin::sighash::{SighashCache, EcdsaSighashType};
        use bitcoin::secp256k1::{Secp256k1, Message};
        use bitcoin::Amount;

        let secp = Secp256k1::new();
        let script_pubkey = self.script_pubkey();

        // Find our input in the PSBT
        let input_idx = psbt.unsigned_tx.input.iter()
            .position(|inp| inp.previous_output == self.utxo_outpoint)
            .ok_or_else(|| anyhow::anyhow!("Our UTXO not found in PSBT"))?;

        let mut cache = SighashCache::new(&psbt.unsigned_tx);
        let sighash = cache.p2wpkh_signature_hash(
            input_idx,
            &script_pubkey,
            Amount::from_sat(self.utxo_value_sats),
            EcdsaSighashType::All,
        )?;
        let msg = Message::from_digest(sighash.to_byte_array());
        let sig = secp.sign_ecdsa(&msg, &self.secret_key.inner);
        let mut sig_bytes = sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL
        Ok(sig_bytes)
    }
}

pub fn parse_outpoint(s: &str) -> Result<OutPoint> {
    let mut parts = s.splitn(2, ':');
    let txid_str = parts.next().ok_or_else(|| anyhow::anyhow!("Missing txid"))?;
    let vout_str = parts.next().ok_or_else(|| anyhow::anyhow!("Missing vout"))?;
    let txid = Txid::from_str(txid_str)?;
    let vout: u32 = vout_str.parse()?;
    Ok(OutPoint::new(txid, vout))
}
