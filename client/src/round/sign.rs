use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bitcoin::psbt::Psbt;
use shared::protocol::SignRequest;
use crate::http::CoordinatorClient;
use crate::wallet::ClientWallet;
use super::InputRegState;

/// Fetch the PSBT, verify our output is present, sign our input, and submit.
///
/// Security: verifies own output before signing (T-05-02 mitigation).
/// Refuses to sign if:
///   - Our output_script is absent from the PSBT outputs
///   - Fee per participant exceeds 10% of denomination
pub async fn verify_and_sign(
    client: &CoordinatorClient,
    wallet: &ClientWallet,
    state: &InputRegState,
    _poll_interval_ms: u64,
) -> Result<()> {
    // 1. Get the assembled PSBT
    let tx_resp = client.get_tx().await?;
    let psbt_bytes = B64.decode(&tx_resp.psbt)?;
    let mut psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| anyhow::anyhow!("PSBT parse error: {e}"))?;

    // 2. Verify our output is present (T-05-02: refuse tampered PSBT)
    let our_script = &state.output_script;
    let our_output = psbt.unsigned_tx.output.iter()
        .find(|o| &o.script_pubkey == our_script)
        .ok_or_else(|| anyhow::anyhow!("Our output not found in PSBT — refusing to sign"))?;

    // 3. Verify fee is reasonable: fee_per_participant < 10% of our output value
    if tx_resp.fee_per_participant_sats > our_output.value.to_sat() / 10 {
        return Err(anyhow::anyhow!(
            "Fee per participant ({}) exceeds 10% of output value ({}) — refusing to sign",
            tx_resp.fee_per_participant_sats,
            our_output.value.to_sat(),
        ));
    }

    // 4. Sign our PSBT input (T-05-04: finds input by outpoint, not index)
    let partial_sig = wallet.sign_psbt_input(&mut psbt)?;

    // 5. Submit partial signature
    let utxo_outpoint = format!("{}:{}", wallet.utxo_outpoint.txid, wallet.utxo_outpoint.vout);
    let session_token_b64 = B64.encode(&state.session_token);
    let req = SignRequest {
        round_id: state.round_id,
        utxo_outpoint,
        partial_signature: B64.encode(&partial_sig),
        session_token: session_token_b64,
    };
    client.post_sign(req).await?;
    Ok(())
}
