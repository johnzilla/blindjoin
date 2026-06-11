pub mod input;
pub mod output;
pub mod sign;

/// State carried between phases
pub struct InputRegState {
    pub round_id: uuid::Uuid,
    pub session_token: Vec<u8>,
    #[allow(dead_code)]
    pub blinding_secret: blind_rsa_signatures::Secret,
    pub msg_randomizer: Option<blind_rsa_signatures::MessageRandomizer>,
    pub message_bytes: [u8; 32],
    pub output_script: bitcoin::ScriptBuf,
    pub unblinded_sig: blind_rsa_signatures::Signature,
    /// SHA-256 of the coordinator RSA public key DER bytes at input registration time.
    /// Used during output registration to detect coordinator key rotation (T-05-01).
    pub pk_hash_at_registration: [u8; 32],
    /// Denomination in satoshis at the time of input registration. Used in the
    /// signing phase to validate our own output value (C1) and to count the
    /// PSBT's denomination outputs for the client anonymity floor (H1).
    pub denomination_sats: u64,
}

impl InputRegState {
    /// Return the unblinded signature bytes (RSA sig) for output registration.
    pub fn unblinded_sig_bytes(&self) -> Vec<u8> {
        <blind_rsa_signatures::Signature as AsRef<[u8]>>::as_ref(&self.unblinded_sig).to_vec()
    }
}
