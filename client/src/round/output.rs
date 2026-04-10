use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use shared::protocol::OutputRegRequest;
use crate::http::CoordinatorClient;
use crate::wallet::ClientWallet;
use super::InputRegState;

pub async fn register_output(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    state: &InputRegState,
    _info: &shared::protocol::InfoResponse,
) -> Result<()> {
    // Re-fetch /info to confirm we're still in a valid round (and to get fresh state)
    let current_info = client.get_info().await?;
    let pk_der_b64 = current_info.rsa_pubkey_der_b64.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing RSA public key in /info during output phase"))?;

    // Verify the coordinator has not rotated its RSA key since input registration.
    // Key rotation between phases would indicate a protocol violation (T-05-01).
    let pk_der_bytes = B64.decode(pk_der_b64)?;
    let actual_pk_hash: [u8; 32] = {
        use sha2::{Sha256, Digest};
        Sha256::digest(&pk_der_bytes).into()
    };
    if actual_pk_hash != state.pk_hash_at_registration {
        return Err(anyhow::anyhow!(
            "Coordinator rotated RSA key between input and output registration phases — aborting"
        ));
    }

    // Encode msg_randomizer if present (RSABSSA-SHA384-PSS-Randomized requires it)
    let msg_randomizer_b64 = state.msg_randomizer.as_ref()
        .map(|mr| B64.encode::<&[u8]>(mr.as_ref()));

    let output_address = wallet.coinjoin_output_address();
    let denomination = current_info.denomination_sats;

    let req = OutputRegRequest {
        unblinded_token: B64.encode(state.message_bytes),
        signature: B64.encode(state.unblinded_sig_bytes()),
        output_address: output_address.to_string(),
        amount_sats: denomination,
        msg_randomizer: msg_randomizer_b64,
    };

    let resp = client.post_output(req).await?;
    if !resp.accepted {
        return Err(anyhow::anyhow!("Output registration rejected by coordinator"));
    }
    Ok(())
}
