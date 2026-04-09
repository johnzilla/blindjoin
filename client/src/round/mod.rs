pub mod input;
pub mod output;
pub mod sign;

/// State carried between phases
pub struct InputRegState {
    pub round_id: uuid::Uuid,
    pub session_token: Vec<u8>,
    pub blinding_secret: blind_rsa_signatures::Secret,
    pub msg_randomizer: Option<blind_rsa_signatures::MessageRandomizer>,
    pub message_bytes: [u8; 32],
    pub output_script: bitcoin::ScriptBuf,
    pub unblinded_sig: blind_rsa_signatures::Signature,
}

impl InputRegState {
    /// Return the unblinded signature bytes (RSA sig) for output registration.
    pub fn unblinded_sig_bytes(&self) -> Vec<u8> {
        <blind_rsa_signatures::Signature as AsRef<[u8]>>::as_ref(&self.unblinded_sig).to_vec()
    }
}
