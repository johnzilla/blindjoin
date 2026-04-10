use blind_rsa_signatures::{
    BlindMessage, BlindSignature, DefaultRng, KeyPair, PublicKey, SecretKey,
    Sha384, PSS, Randomized,
};
use sha2::{Sha256, Digest};

/// Type aliases for the chosen parameter set: SHA-384, PSS, Randomized (RFC 9474 default).
pub type BjPublicKey = PublicKey<Sha384, PSS, Randomized>;
pub type BjSecretKey = SecretKey<Sha384, PSS, Randomized>;
#[allow(dead_code)]
pub type BjKeyPair = KeyPair<Sha384, PSS, Randomized>;

/// RFC 9474 RSA blind signer — one instance per round.
///
/// The secret key is NEVER exposed outside this module.
/// The public key hash is the SHA-256 of the SPKI-encoded (DER SubjectPublicKeyInfo) bytes.
///
/// NOTE on memory zeroing (D-07): As of blind-rsa-signatures 0.17.x, SecretKey does not
/// implement Zeroize. The RSA private key bytes held in this struct are not explicitly
/// zeroed on drop — this is a known upstream limitation. RoundStateInner (round/state.rs)
/// stores the serialized key under ZeroizeOnDrop; that serialized copy IS zeroed on round
/// completion. The in-process copy in SecretKey here is best-effort only.
pub struct RsaBlindSigner {
    pub public_key: BjPublicKey,
    secret_key: BjSecretKey, // Never pub — unlinkability depends on key secrecy
}

#[allow(dead_code)]
impl RsaBlindSigner {
    /// Generate a fresh RSA-2048 blind signing keypair.
    /// Called once at the start of each round (D-02: per-round ephemeral keys).
    pub fn generate() -> Result<Self, blind_rsa_signatures::Error> {
        let kp = BjKeyPair::generate(&mut DefaultRng, 2048)?;
        Ok(Self { public_key: kp.pk, secret_key: kp.sk })
    }

    /// SHA-256 of the SPKI DER-encoded SubjectPublicKeyInfo bytes.
    /// Published in GET /info response so clients can verify key matches commitment (D-02).
    pub fn public_key_hash(&self) -> [u8; 32] {
        let spki = self.public_key
            .to_spki()
            .expect("RSA public key must be SPKI-encodable");
        Sha256::digest(&spki).into()
    }

    /// Blind-sign a blinded message. The coordinator never sees the original message M.
    pub fn blind_sign(&self, blinded_msg: &BlindMessage) -> Result<BlindSignature, blind_rsa_signatures::Error> {
        self.secret_key.blind_sign(blinded_msg)
    }

    /// Reconstruct an RsaBlindSigner from DER-encoded secret key bytes.
    /// Used to reload the signer from round state inner storage.
    pub fn from_der_secret_key(der: &[u8]) -> Result<Self, blind_rsa_signatures::Error> {
        let secret_key = BjSecretKey::from_der(der)?;
        let public_key = secret_key.public_key()?;
        Ok(Self { public_key, secret_key })
    }

    /// Export the secret key as DER bytes for storage in round state inner.
    pub fn secret_key_der(&self) -> Result<Vec<u8>, blind_rsa_signatures::Error> {
        self.secret_key.to_der()
    }

    /// Export the public key as SPKI DER bytes.
    pub fn public_key_spki_der(&self) -> Result<Vec<u8>, blind_rsa_signatures::Error> {
        self.public_key.to_spki()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blind_rsa_signatures::DefaultRng;

    fn test_message() -> Vec<u8> {
        b"blindjoin-test-message-32-bytes!".to_vec()
    }

    #[test]
    fn blind_sign_round_trip() {
        let signer = RsaBlindSigner::generate().unwrap();
        let pk = &signer.public_key;

        let msg = test_message();
        // Client blinds the message
        let blinding_result = pk.blind(&mut DefaultRng, &msg).unwrap();
        // Coordinator blind-signs (never sees msg)
        let blind_sig = signer.blind_sign(&blinding_result.blind_message).unwrap();
        // Client unblinds + verifies (finalize also verifies internally)
        let sig = pk.finalize(&blind_sig, &blinding_result, &msg).unwrap();
        // Explicit verify: signature on msg is valid under public key
        pk.verify(&sig, blinding_result.msg_randomizer, &msg).unwrap();
    }

    #[test]
    fn public_key_hash_is_32_bytes() {
        let signer = RsaBlindSigner::generate().unwrap();
        let hash = signer.public_key_hash();
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn public_key_hash_is_deterministic() {
        let signer = RsaBlindSigner::generate().unwrap();
        assert_eq!(signer.public_key_hash(), signer.public_key_hash());
    }

    #[test]
    fn unlinkability_two_tokens() {
        // Two clients blinding two different messages: blinded forms should differ
        let signer = RsaBlindSigner::generate().unwrap();
        let pk = &signer.public_key;

        let msg1 = b"message-one-32-bytes-padding-000";
        let msg2 = b"message-two-32-bytes-padding-000";
        let b1 = pk.blind(&mut DefaultRng, msg1).unwrap();
        let b2 = pk.blind(&mut DefaultRng, msg2).unwrap();
        // Blinded messages must differ (trivially true for different messages)
        assert_ne!(<BlindMessage as AsRef<[u8]>>::as_ref(&b1.blind_message),
                   <BlindMessage as AsRef<[u8]>>::as_ref(&b2.blind_message));
        // Furthermore, blind the SAME message twice — blinded forms must differ (randomized blinding)
        let b3 = pk.blind(&mut DefaultRng, msg1).unwrap();
        assert_ne!(<BlindMessage as AsRef<[u8]>>::as_ref(&b1.blind_message),
                   <BlindMessage as AsRef<[u8]>>::as_ref(&b3.blind_message),
            "Blinding the same message twice must produce different blinded forms (randomized)");
    }
}
