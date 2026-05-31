use bitcoin::{Network, OutPoint, Psbt, ScriptBuf, Txid};
use std::str::FromStr;
use anyhow::{anyhow, Result};
use bdk_wallet::{KeychainKind, Wallet};
#[allow(deprecated)]
use bdk_wallet::signer::SignOptions;
use shared::bip322::ScriptType;

/// BIP-322 ownership proof produced by [`BdkClientWallet::sign_bip322`].
///
/// Phase 17 17-02 D-64 / CD-18 / CD-19: a non-wire intermediate carrying the
/// signed witness plus a few auxiliary fields needed by the round-input
/// envelope builder (`client/src/round/input.rs::register_input`). NOT a wire
/// type — no `Serialize`/`Deserialize` derives; the struct only crosses module
/// boundaries within the `client` crate.
///
/// Fields:
/// - `witness_stack`: flat `Vec<Vec<u8>>` form for the v=1 envelope (D-70 — populated
///   in BOTH envelopes for symmetry; the v=2 envelope uses it as a discoverability hint
///   while the load-bearing witness bytes travel in `psbt_input_b64`).
/// - `witness`: the bare `bitcoin::Witness` used by the v=2 envelope's
///   `build_v2_psbt_input_b64` helper.
/// - `final_script_sig`: P2SH-P2WPKH only (per RESEARCH Pitfall 7). For P2WPKH
///   and P2TR this is always `None`; for P2SH-P2WPKH `Some(redeem_script_sig)`
///   carries the P2SH unlocking scriptSig that bdk_wallet writes alongside the
///   final_script_witness when finalising `sh(wpkh(...))`.
/// - `script_type`: the wallet's stored descriptor outer-wrapper type. Used by
///   `register_input` as the CRIT-01 wire source — the v=2 envelope's
///   `script_type` field reads from here, NEVER from `cfg.script_type` (which
///   would allow a CLI-misconfigured user to declare P2TR over an on-chain
///   P2WPKH SPK and bypass per-script sighash verification).
#[derive(Debug, Clone)]
pub struct Bip322SignedProof {
    pub witness_stack: Vec<Vec<u8>>,
    pub witness: bitcoin::Witness,
    pub final_script_sig: Option<ScriptBuf>,
    pub script_type: ScriptType,
}

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
    /// The script_pubkey (P2WPKH / P2TR / P2SH-P2WPKH) controlling the UTXO,
    /// needed for BIP-322 and PSBT signing.
    utxo_script_pubkey: ScriptBuf,
    /// The WIF key string, stored for secret_key_for_signing (WIF wallets only).
    wif_key: Option<String>,
    /// The descriptor's outer-wrapper type. Set at construction; single source of
    /// truth for downstream consumers (Phase 17 17-02 sign dispatcher + 17-03
    /// discovery check). Per D-62 the wallet KNOWS its type — never re-detected
    /// at sign-time. Per D-61 from_wif always sets P2wpkh.
    #[allow(dead_code)] // consumed by 17-02 sign dispatcher + 17-03 discovery check
    script_type: ScriptType,
    inner: Wallet,
    /// Test-only mirror of the generated/loaded external descriptor string, so
    /// unit tests can deterministically assert the BIP-84/86/49 prefix without
    /// driving bdk_wallet's internal descriptor formatter.
    #[cfg(test)]
    external_desc_str: String,
}

impl BdkClientWallet {
    /// Backward-compat constructor: builds a single-key wpkh(WIF) descriptor wallet.
    ///
    /// Accepts the same arguments as the Phase 1 ClientWallet::from_wif.
    /// Integration tests use this path — the API contract is preserved.
    ///
    /// P2WPKH-only per Phase 17 D-61 — descriptor wallets always come through
    /// from_descriptor or generate. This signature does NOT accept a script_type
    /// parameter; the returned wallet's script_type is hardcoded to P2wpkh so
    /// the v1.3 cross-phase invariant (tests/integration/full_round.rs) stays
    /// bit-exact unchanged.
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
        let inner = Wallet::create_single(descriptor.clone())
            .network(bdk_net)
            .create_wallet_no_persist()
            .map_err(|e| anyhow!("Failed to create bdk wallet from WIF: {e}"))?;

        let outpoint = parse_outpoint(utxo_outpoint_str)?;
        Ok(Self {
            network,
            utxo_outpoint: outpoint,
            utxo_script_pubkey,
            wif_key: Some(wif.to_string()),
            // D-61: from_wif is P2WPKH-only — hardcode here so the cross-phase
            // invariant (tests/integration/full_round.rs) stays bit-exact.
            script_type: ScriptType::P2wpkh,
            inner,
            #[cfg(test)]
            external_desc_str: descriptor,
        })
    }

    /// Build a wallet from user-provided BIP descriptor strings.
    ///
    /// external_desc: e.g. "wpkh(xprv.../84'/0'/0'/0/*)" (P2WPKH/BIP-84),
    ///                     "tr(xprv.../86'/0'/0'/0/*)" (P2TR/BIP-86),
    ///                     "sh(wpkh(xprv.../49'/0'/0'/0/*))" (P2SH-P2WPKH/BIP-49).
    /// utxo_address: bech32 / bech32m / base58 address of the UTXO being registered
    ///   (needed to derive utxo_script_pubkey without chain data).
    /// script_type: the user-declared --type flag. Cross-checked against the
    ///   descriptor's outer wrapper at construction time per D-63 — a mismatch
    ///   fails fast with both names in the error message.
    pub fn from_descriptor(
        external_desc: &str,
        utxo_outpoint_str: &str,
        utxo_address: &str,
        network: Network,
        script_type: ScriptType,
    ) -> Result<Self> {
        // D-63: construction-time script-type vs descriptor wrapper cross-check.
        // Detect the wrapper by string-matching the well-known prefixes (sh(wpkh
        // FIRST, because "sh(" alone also matches sh(wpkh) — and the latter is
        // the only sh() shape we support in Phase 17).
        let detected = if external_desc.starts_with("sh(wpkh(") {
            ScriptType::P2shP2wpkh
        } else if external_desc.starts_with("wpkh(") {
            ScriptType::P2wpkh
        } else if external_desc.starts_with("tr(") {
            ScriptType::P2tr
        } else {
            return Err(anyhow!(
                "descriptor wrapper not recognised: expected `wpkh(...)`, `tr(...)`, or `sh(wpkh(...))` (got: {external_desc:?})"
            ));
        };
        if detected != script_type {
            return Err(anyhow!(
                "descriptor wrapper {detected:?} does not match --type {script_type:?}"
            ));
        }

        let bdk_net = bdk_network(network);
        // Single-key (non-derivation) descriptors lack the `/0/*` template path.
        // bdk_wallet 2.3 rejects `Wallet::create(d, d)` with "External and
        // internal descriptors are the same" — use `Wallet::create_single` for
        // the keychain-less case. Phase 19 Plan 19-01 Task 4 [Rule 3 — Bug]
        // surfaced this when constructing single-key WIF descriptor wallets
        // (`tr(<WIF>)`, `sh(wpkh(<WIF>))`) for the bdk-vs-shared parity tests
        // in client/tests/wallet_sign_roundtrip.rs.
        let inner = if external_desc.contains("/0/*)") {
            let internal_desc = external_desc.replacen("/0/*)", "/1/*)", 1);
            Wallet::create(external_desc.to_string(), internal_desc)
                .network(bdk_net)
                .create_wallet_no_persist()
                .map_err(|e| anyhow!("Failed to create bdk wallet from descriptor: {e}"))?
        } else {
            Wallet::create_single(external_desc.to_string())
                .network(bdk_net)
                .create_wallet_no_persist()
                .map_err(|e| anyhow!("Failed to create bdk wallet from descriptor: {e}"))?
        };

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
            script_type,
            inner,
            #[cfg(test)]
            external_desc_str: external_desc.to_string(),
        })
    }

    /// Generate a fresh BIP-84 / BIP-86 / BIP-49 wallet using a random mnemonic.
    ///
    /// The `script_type` arg selects the descriptor template (D-58): P2wpkh →
    /// BIP-84, P2tr → BIP-86, P2shP2wpkh → BIP-49. coin=0' is preserved across
    /// ALL networks per D-66 — DO NOT switch to `bdk_wallet::template::Bip84/86/49`
    /// because those auto-select coin=1' on testnet/signet and break v1.3
    /// byte-equivalence (RESEARCH Pitfall 2; load-bearing for the cross-phase
    /// invariant tests/integration/full_round.rs).
    ///
    /// Prints the external and internal descriptors to stdout with a prominent warning.
    /// Also writes a descriptors.txt file in cwd with 0600 permissions (T-03-04 mitigation).
    /// Returns a wallet ready for immediate use in the current round.
    pub fn generate(
        utxo_outpoint_str: &str,
        network: Network,
        script_type: ScriptType,
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

        // D-58 / D-66: literal descriptor templates, coin=0' across all networks.
        // The P2WPKH branch is UNCHANGED from v1.3 → cross-phase byte-equivalence.
        let (external_desc, internal_desc) = match script_type {
            ScriptType::P2wpkh => (
                format!("wpkh({}/84'/0'/0'/0/*)", xprv),
                format!("wpkh({}/84'/0'/0'/1/*)", xprv),
            ),
            ScriptType::P2tr => (
                format!("tr({}/86'/0'/0'/0/*)", xprv),
                format!("tr({}/86'/0'/0'/1/*)", xprv),
            ),
            ScriptType::P2shP2wpkh => (
                format!("sh(wpkh({}/49'/0'/0'/0/*))", xprv),
                format!("sh(wpkh({}/49'/0'/0'/1/*))", xprv),
            ),
        };
        let (script_type_kebab, bip) = match script_type {
            ScriptType::P2wpkh => ("p2wpkh", 84u32),
            ScriptType::P2tr => ("p2tr", 86u32),
            ScriptType::P2shP2wpkh => ("p2sh-p2wpkh", 49u32),
        };

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
        // D-60: per-type banner line so the user sees which script type the
        // address corresponds to (BIP-84 P2WPKH / BIP-86 P2TR / BIP-49 P2SH-P2WPKH).
        println!("  Script type: {} (BIP-{})", script_type_kebab, bip);
        println!();
        println!("  (BIP-{} path: m/{}'/0'/0'/0/0)", bip, bip);
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
            script_type,
            inner,
            #[cfg(test)]
            external_desc_str: external_desc,
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

    /// Returns the script_pubkey (P2WPKH / P2TR / P2SH-P2WPKH) for the UTXO being registered.
    ///
    /// Phase 17 17-02 Task 2 (CD-20): the prior in-bin consumer
    /// (`client/src/round/input.rs::generate_bip322_witness`) was deleted;
    /// callers now reach the SPK through `sign_bip322` internally. The
    /// accessor is still load-bearing for external integration tests
    /// (`client/tests/wallet_sign_roundtrip.rs`), hence the dead_code allow.
    #[allow(dead_code)]
    pub fn script_pubkey(&self) -> ScriptBuf {
        self.utxo_script_pubkey.clone()
    }

    /// Returns the wallet's descriptor outer-wrapper script type. Set at
    /// construction (per D-62) and treated as the single source of truth by
    /// downstream consumers (Phase 17 17-02 sign dispatcher + 17-03 discovery
    /// fail-fast + the v=2 OwnershipProof CRIT-01 wire source). ScriptType
    /// derives Copy so the accessor returns by value.
    #[allow(dead_code)] // consumed by 17-02 sign dispatcher + 17-03 discovery check
    pub fn script_type(&self) -> ScriptType {
        self.script_type
    }

    /// Test-only accessor returning the generated/loaded external descriptor
    /// string, so unit tests can deterministically assert the BIP-84/86/49
    /// prefix shape.
    #[cfg(test)]
    pub(crate) fn external_desc_str(&self) -> &str {
        &self.external_desc_str
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
        //
        // Mixed-script PSBT guard [Rule 1 fix]:
        //
        // In a mixed-script CoinJoin round the coordinator's PSBT contains inputs from
        // participants with different script types (e.g. P2WPKH, P2TR, P2SH-P2WPKH).
        // bdk_wallet's sign() iterates ALL signers against ALL inputs:
        //   - An ECDSA signer (P2WPKH/P2SH-P2WPKH) calls psbt.sighash_ecdsa(i) on EVERY
        //     input, which errors on Taproot inputs ("attempt to sign with the wrong signing
        //     algorithm").
        //   - finalize_psbt() errors on P2SH inputs without redeem_script ("missing redeem
        //     script") when called by a wallet that doesn't own those inputs.
        //
        // Fix: temporarily mark inputs we DON'T own as "already finalized" by setting
        // final_script_witness = Some(Witness::new()) (an empty witness, which signals to
        // bdk_wallet's sign_input that the input is done and should be skipped). After
        // signing, we clear those markers so the PSBT reflects only real witnesses.
        //
        // This preserves the correct sighash computation: bdk_wallet still receives the FULL
        // PSBT (all inputs/outputs) when computing the sighash for OUR input, which is
        // required for segwit sighash (BIP-143 / BIP-341 commit to all inputs).
        let guard_indices: Vec<usize> = (0..psbt.inputs.len())
            .filter(|&i| i != input_idx
                && psbt.inputs[i].final_script_sig.is_none()
                && psbt.inputs[i].final_script_witness.is_none())
            .collect();

        // Temporarily mark non-owned inputs as finalized (empty witness = skip signal).
        for &i in &guard_indices {
            psbt.inputs[i].final_script_witness = Some(bitcoin::Witness::new());
        }

        #[allow(deprecated)]
        let sign_result = self.inner.sign(psbt, SignOptions { trust_witness_utxo: true, ..SignOptions::default() });

        // Remove our temporary markers regardless of sign result.
        for &i in &guard_indices {
            psbt.inputs[i].final_script_witness = None;
        }

        sign_result.map_err(|e| anyhow!("bdk_wallet signing failed: {e}"))?;

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

    /// Produce a BIP-322 Simple ownership proof over `message` for this
    /// wallet's UTXO (Phase 17 17-02 D-64 / D-65 / CD-19 / CD-24).
    ///
    /// Two dispatch arms, branching on `self.wif_key.is_some()`:
    ///
    /// - **WIF wallets** (legacy carry-forward): route through
    ///   `shared::bip322::sign_simple(P2wpkh, ...)`. Phase 15 confirmed this is
    ///   bit-exact equivalent to the prior hand-rolled
    ///   `client/src/round/input.rs::generate_bip322_witness` (deleted in
    ///   Plan 17-02 Task 2 per CD-20). Per D-61 from_wif is P2WPKH-only.
    ///
    /// - **Descriptor wallets** (P2WPKH / P2TR / P2SH-P2WPKH uniformly per
    ///   CD-24): build the BIP-322 to_sign PSBT (script-neutral primitives in
    ///   `shared::bip322`) and route through bdk_wallet 2.3's PSBT signer
    ///   (Sprint-0-B PoC PASS, ADR Decision #4). Witness extraction is
    ///   script-type-specific:
    ///   - P2TR: prefer `psbt.inputs[0].final_script_witness` (Sprint-0-B
    ///     finding — bdk_wallet 2.3 clears `tap_key_sig` at finalisation).
    ///   - P2SH-P2WPKH (RESEARCH Pitfall 7): MUST extract BOTH
    ///     `final_script_witness` AND `final_script_sig`; missing
    ///     `final_script_sig` is an error.
    ///
    /// Visibility note (CD-19): the plan preferred `pub(crate)` but external
    /// integration-test crates at `client/tests/*.rs` only see `pub` items, so
    /// this is `pub` for test reach. Documented as a Rule-3 visibility
    /// escalation in Plan 17-02 Task 1 SUMMARY.
    pub fn sign_bip322(&self, message: &str) -> Result<Bip322SignedProof> {
        if self.wif_key.is_some() {
            // D-61: WIF wallets are always P2WPKH.
            debug_assert_eq!(
                self.script_type,
                ScriptType::P2wpkh,
                "from_wif must construct ScriptType::P2wpkh (D-61)"
            );
            let sk = self.secret_key_for_signing();
            let witness = shared::bip322::sign_simple(
                ScriptType::P2wpkh,
                &self.utxo_script_pubkey,
                &sk,
                message.as_bytes(),
            )
            .map_err(|e| anyhow!("shared::bip322::sign_simple failed: {e}"))?;
            let witness_stack = witness.iter().map(|s| s.to_vec()).collect::<Vec<_>>();
            return Ok(Bip322SignedProof {
                witness_stack,
                witness,
                final_script_sig: None,
                script_type: ScriptType::P2wpkh,
            });
        }

        // Descriptor branch — uniform bdk PSBT-sign path per CD-24, covering
        // P2WPKH / P2TR / P2SH-P2WPKH. Mirrors sign_psbt_input above:
        // trust_witness_utxo: true is required because the BIP-322 to_spend
        // output has value=0 and no on-chain provenance — the malicious-
        // -coordinator-lies-about-value reasoning at sign_psbt_input does not
        // apply here because BIP-322 has no value to lie about.
        let msg_hash = shared::bip322::bip322_message_hash(message.as_bytes());
        let to_spend = shared::bip322::build_bip322_to_spend(&self.utxo_script_pubkey, &msg_hash);
        let to_sign = shared::bip322::build_bip322_to_sign(&to_spend);
        let mut psbt = bitcoin::psbt::Psbt::from_unsigned_tx(to_sign)
            .map_err(|e| anyhow!("Psbt::from_unsigned_tx (BIP-322 to_sign): {e}"))?;
        psbt.inputs[0].witness_utxo = Some(bitcoin::TxOut {
            value: bitcoin::Amount::ZERO,
            script_pubkey: self.utxo_script_pubkey.clone(),
        });
        #[allow(deprecated)]
        self.inner
            .sign(
                &mut psbt,
                SignOptions { trust_witness_utxo: true, ..SignOptions::default() },
            )
            .map_err(|e| anyhow!("bdk_wallet BIP-322 sign failed: {e}"))?;

        // Witness extraction — per-script branch.
        let (witness, final_script_sig) = match self.script_type {
            ScriptType::P2wpkh => {
                let input = &psbt.inputs[0];
                let w = if let Some(w) = input.final_script_witness.clone() {
                    w
                } else if let Some((pk, sig)) = input.partial_sigs.iter().next() {
                    // Single-key fallback (mirrors sign_psbt_input lines 385-389).
                    let mut w = bitcoin::Witness::new();
                    w.push(sig.to_vec());
                    w.push(pk.to_bytes());
                    w
                } else {
                    return Err(anyhow!(
                        "bdk_wallet did not produce a P2WPKH BIP-322 witness"
                    ));
                };
                (w, None)
            }
            ScriptType::P2tr => {
                // Sprint-0-B finding: bdk_wallet 2.3 puts the keyspend sig in
                // final_script_witness[0]; tap_key_sig is cleared at
                // finalisation. Dual-path for future bdk-version resilience.
                let input = &psbt.inputs[0];
                let w = if let Some(w) = input.final_script_witness.clone() {
                    w
                } else if let Some(tap_key_sig) = input.tap_key_sig {
                    let mut w = bitcoin::Witness::new();
                    w.push(tap_key_sig.serialize());
                    w
                } else {
                    return Err(anyhow!(
                        "bdk_wallet did not produce a P2TR witness (neither final_script_witness nor tap_key_sig populated)"
                    ));
                };
                (w, None)
            }
            ScriptType::P2shP2wpkh => {
                // RESEARCH Pitfall 7: bdk_wallet finalises sh(wpkh(...)) by
                // populating BOTH final_script_witness AND final_script_sig.
                // Extract BOTH; missing final_script_sig is an error.
                let input = &psbt.inputs[0];
                let w = input.final_script_witness.clone().ok_or_else(|| {
                    anyhow!("bdk_wallet did not produce a P2SH-P2WPKH final_script_witness")
                })?;
                let ssig = input.final_script_sig.clone().ok_or_else(|| {
                    anyhow!("bdk_wallet did not produce a P2SH-P2WPKH final_script_sig")
                })?;
                (w, Some(ssig))
            }
        };

        let witness_stack = witness.iter().map(|s| s.to_vec()).collect::<Vec<_>>();
        Ok(Bip322SignedProof {
            witness_stack,
            witness,
            final_script_sig,
            script_type: self.script_type,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::bip322::ScriptType;

    // Phase 17 17-01 Task 2 — descriptor templates per script type (D-58),
    // construction-time mismatch fail-fast (D-63), accessor (D-62), and the
    // from_wif P2WPKH-only invariant (D-61).

    const DUMMY_OUTPOINT: &str =
        "0000000000000000000000000000000000000000000000000000000000000000:0";

    #[test]
    fn generate_p2wpkh_produces_bip84_descriptor() {
        let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2wpkh)
            .expect("P2WPKH generate should succeed");
        let desc = wallet.external_desc_str();
        assert!(
            desc.starts_with("wpkh("),
            "expected BIP-84 wpkh( prefix, got: {desc}"
        );
        assert!(
            desc.contains("/84'/0'/0'/0/*"),
            "expected BIP-84 derivation path /84'/0'/0'/0/* (coin=0' per D-66), got: {desc}"
        );
    }

    #[test]
    fn generate_p2tr_produces_bip86_descriptor() {
        let wallet = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2tr)
            .expect("P2TR generate should succeed");
        let desc = wallet.external_desc_str();
        assert!(
            desc.starts_with("tr("),
            "expected BIP-86 tr( prefix, got: {desc}"
        );
        assert!(
            desc.contains("/86'/0'/0'/0/*"),
            "expected BIP-86 derivation path /86'/0'/0'/0/* (coin=0' per D-66), got: {desc}"
        );
    }

    #[test]
    fn generate_p2sh_p2wpkh_produces_bip49_descriptor() {
        let wallet = BdkClientWallet::generate(
            DUMMY_OUTPOINT,
            Network::Signet,
            ScriptType::P2shP2wpkh,
        )
        .expect("P2SH-P2WPKH generate should succeed");
        let desc = wallet.external_desc_str();
        assert!(
            desc.starts_with("sh(wpkh("),
            "expected BIP-49 sh(wpkh( prefix, got: {desc}"
        );
        assert!(
            desc.contains("/49'/0'/0'/0/*"),
            "expected BIP-49 derivation path /49'/0'/0'/0/* (coin=0' per D-66), got: {desc}"
        );
    }

    #[test]
    fn script_type_accessor_matches_construction() {
        // For each of the 3 generate paths, wallet.script_type() must return
        // the script type passed to generate (D-62).
        let w1 = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2wpkh)
            .expect("P2WPKH generate should succeed");
        assert_eq!(w1.script_type(), ScriptType::P2wpkh);

        let w2 = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2tr)
            .expect("P2TR generate should succeed");
        assert_eq!(w2.script_type(), ScriptType::P2tr);

        let w3 = BdkClientWallet::generate(DUMMY_OUTPOINT, Network::Signet, ScriptType::P2shP2wpkh)
            .expect("P2SH-P2WPKH generate should succeed");
        assert_eq!(w3.script_type(), ScriptType::P2shP2wpkh);
    }

    #[test]
    fn from_descriptor_rejects_p2tr_flag_with_wpkh_descriptor() {
        // D-63 construction-time mismatch check. The error message must name
        // BOTH the declared --type AND the detected descriptor wrapper.
        //
        // We use a syntactically valid wpkh() descriptor with a known signet xprv
        // shape; the function should reject BEFORE attempting to call bdk's
        // Wallet::create, so even a fake-ish xprv is fine — the mismatch check
        // runs first by design.
        let bad_desc = "wpkh(tprv8ZgxMBicQKsPd7Uf69XL1XwhmjHopUGep8GuEiJDZmbQz6o58LninorQAfcKZWARbtRtfnLcJ5MQ2AtHcQJCCRUcMRvmDUjyEmNUWwx8UbK/84'/0'/0'/0/*)";
        let utxo_address = "tb1qrp33g0q5c5txsp9arysrx4k6zdkfs4nce4xj0g"; // signet wpkh address
        let result = BdkClientWallet::from_descriptor(
            bad_desc,
            DUMMY_OUTPOINT,
            utxo_address,
            Network::Signet,
            ScriptType::P2tr,
        );
        match result {
            Ok(_) => panic!("expected mismatch between --type p2tr and wpkh() descriptor to fail"),
            Err(err) => {
                let msg = format!("{err:?}").to_lowercase();
                assert!(
                    msg.contains("p2tr") && msg.contains("wpkh"),
                    "expected error to name BOTH 'p2tr' AND 'wpkh', got: {msg}"
                );
            }
        }
    }

    #[test]
    fn from_wif_asserts_p2wpkh() {
        // D-61: from_wif takes NO script_type parameter; the returned wallet
        // ALWAYS has script_type == P2wpkh, preserving the v1.3 cross-phase
        // invariant (full_round.rs uses the WIF path).
        // Use the canonical Bitcoin Core regtest "Hello World" WIF.
        let wif = "cVt4o7BGAig1UXywgGSmARhxMdzP5qvQsxKkSsc1XEkw3tDTQFpy";
        let wallet = BdkClientWallet::from_wif(wif, DUMMY_OUTPOINT, Network::Regtest)
            .expect("from_wif should succeed for a valid WIF");
        assert_eq!(wallet.script_type(), ScriptType::P2wpkh);
    }
}
