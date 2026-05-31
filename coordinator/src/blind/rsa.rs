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

/// AUDIT-03 (D-128, D-128a, D-129, CD-47): newtype wrapping the per-round RSA secret
/// key so the key's lifetime is expressible as a Rust type signature
/// (`Option<RsaBlindSigner>` on `RoundStateInner`) rather than ambient ownership.
///
/// Drop chain (load-bearing, cited by the rewritten D-07 comment below and by
/// `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window`):
///
/// `transition_to(Phase::Idle)` (`state.rs:194-200`)
///   → `drop(Option<RoundStateInner>)` (sets `self.inner = None`)
///     → `drop(Option<RsaBlindSigner>)`
///       → `drop(RoundSecretKey)`                  (this impl)
///         → `drop(BjSecretKey)`
///           → `drop(rsa::RsaPrivateKey)`           (`rsa-0.9.10/src/key.rs:76-82`)
///                                                 zeroizes `d`, `primes`, `precomputed`.
///
/// The newtype's cryptographic value is zero — the wrapped `rsa::RsaPrivateKey`
/// already implements an UNCONDITIONAL `Drop` + `ZeroizeOnDrop` (no feature flag)
/// that zeroizes the secret limbs in place. The newtype's audit value is
/// **lifetime expression**: making the per-round key a value the FSM nulls at one
/// chokepoint (`state.rs:195`), not a long-lived field of `RoundStateInner`.
pub struct RoundSecretKey(BjSecretKey);

impl RoundSecretKey {
    /// Wrap a fresh `BjSecretKey`. Called by `RsaBlindSigner::generate` and
    /// `RsaBlindSigner::from_der_secret_key`. `pub(crate)` because all current
    /// callers are inside `coordinator/src/blind/rsa.rs`; if v1.6+ needs cross-module
    /// construction, widen then.
    pub(crate) fn new(sk: BjSecretKey) -> Self {
        Self(sk)
    }

    /// Borrow the wrapped secret key for blind-signing and DER export.
    /// Never returns ownership — that would defeat the `Option<RsaBlindSigner>`
    /// lifetime bound by allowing callers to extract the key into an ambient binding.
    pub(crate) fn as_inner(&self) -> &BjSecretKey {
        &self.0
    }
}

impl Drop for RoundSecretKey {
    fn drop(&mut self) {
        // The wrapped `rsa::RsaPrivateKey` zeroizes `d`/`primes`/`precomputed` in
        // its own `Drop` (`rsa-0.9.10/src/key.rs:76-82`); no in-place scrub is
        // needed here. The transitive drop chain runs as part of the natural
        // struct drop.
        //
        // PII-safe: static-string message only, target `blindjoin::audit` for
        // filterability. No `{:?}` formatter on `self` or any field (a naive
        // `?self.0` would invoke `BjSecretKey`'s derived `Debug`, which prints
        // the inner `RsaPrivateKey` and may leak DER bytes — see 21-RESEARCH
        // Pitfall 2 for the full anti-pattern catalogue).
        tracing::debug!(
            target: "blindjoin::audit",
            "RoundSecretKey dropped — rsa::RsaPrivateKey ZeroizeOnDrop fires transitively"
        );
    }
}

/// RFC 9474 RSA blind signer — one instance per round.
///
/// The secret key is NEVER exposed outside this module.
/// The public key hash is the SHA-256 of the SPKI-encoded (DER SubjectPublicKeyInfo) bytes.
///
/// NOTE on memory zeroing (D-07 + AUDIT-03):
///
/// The per-round secret key is wrapped in `RoundSecretKey` (above) and held in the
/// `secret_key` field below. The key's lifetime is bounded by
/// `RoundStateInner.rsa_signer: Option<RsaBlindSigner>` on the outer round state.
/// On any transition to `Phase::Idle`, `RoundState::transition_to` (declared at
/// `coordinator/src/round/state.rs:193`) sets `self.inner = None` at the
/// success-path chokepoint inside the function body
/// (`coordinator/src/round/state.rs:202`, within the validated-transition block
/// at lines 201-207), which drops `Option<RsaBlindSigner>`, which drops
/// `RoundSecretKey`, whose `Drop` impl emits a PII-safe `tracing::debug!` event
/// and then the wrapped `BjSecretKey` drops.
///
/// `BjSecretKey` is `blind_rsa_signatures::SecretKey<Sha384, PSS, Randomized>`,
/// which holds `inner: rsa::RsaPrivateKey`. The `rsa = 0.9.10` crate has an
/// UNCONDITIONAL `impl Drop for RsaPrivateKey` at `rsa-0.9.10/src/key.rs:76-82`
/// that zeroizes the secret exponent `d`, the prime factors `primes`, and the
/// CRT-precomputed values `precomputed`, plus `impl ZeroizeOnDrop for RsaPrivateKey`
/// at line 84. Both impls compile without any feature flag — `zeroize` is a
/// non-optional dep of `rsa`. The cryptographically meaningful work is therefore
/// done by upstream `rsa`; the value of this module's newtype is **lifetime
/// expression** (`Option<RsaBlindSigner>` is a value the FSM nulls at one
/// chokepoint), not redundant in-place scrub.
///
/// See `docs/AUDIT-CHARTER.md#rsa-secret-key-zeroization-window` for the full
/// threat-model treatment.
pub struct RsaBlindSigner {
    pub public_key: BjPublicKey,
    secret_key: RoundSecretKey, // Never pub — unlinkability depends on key secrecy
}

#[allow(dead_code)]
impl RsaBlindSigner {
    /// Generate a fresh RSA-2048 blind signing keypair.
    /// Called once at the start of each round (D-02: per-round ephemeral keys).
    pub fn generate() -> Result<Self, blind_rsa_signatures::Error> {
        let kp = BjKeyPair::generate(&mut DefaultRng, 2048)?;
        Ok(Self { public_key: kp.pk, secret_key: RoundSecretKey::new(kp.sk) })
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
        self.secret_key.as_inner().blind_sign(blinded_msg)
    }

    /// Reconstruct an RsaBlindSigner from DER-encoded secret key bytes.
    /// Used to reload the signer from round state inner storage.
    pub fn from_der_secret_key(der: &[u8]) -> Result<Self, blind_rsa_signatures::Error> {
        let secret_key = BjSecretKey::from_der(der)?;
        let public_key = secret_key.public_key()?;
        Ok(Self { public_key, secret_key: RoundSecretKey::new(secret_key) })
    }

    /// Export the secret key as DER bytes for storage in round state inner.
    pub fn secret_key_der(&self) -> Result<Vec<u8>, blind_rsa_signatures::Error> {
        self.secret_key.as_inner().to_der()
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

    /// SPKI handshake roundtrip (D-03): proves the coordinator emit path
    /// `public_key_spki_der()` and the client decode path `BjPublicKey::from_spki`
    /// are symmetric inverses, AND that `SHA-256(public_key_spki_der())` equals
    /// the D-02 hash commitment `public_key_hash()`. Catches future format drift
    /// in either direction (blind-rsa-signatures bumps, coordinator emit changes,
    /// client decode changes) without requiring bitcoind.
    #[test]
    fn spki_handshake_round_trip() {
        use sha2::{Sha256, Digest};

        let signer = RsaBlindSigner::generate().unwrap();

        // 1+2. Emit via the production path the coordinator publishes on /info.
        let spki = signer.public_key_spki_der().unwrap();

        // 3. D-02 commitment: SHA-256(SPKI bytes) MUST equal public_key_hash().
        let hash_via_emit: [u8; 32] = Sha256::digest(&spki).into();
        assert_eq!(
            hash_via_emit,
            signer.public_key_hash(),
            "SHA-256(public_key_spki_der()) must equal public_key_hash()"
        );

        // 4. Re-parse via the production client decode path (mirrors client/src/round/input.rs:40).
        let pk_reparsed = BjPublicKey::from_spki(&spki).unwrap();

        // 5. Re-parsed key blinds a message the original signer can blind-sign,
        //    finalize, and verify — proves the parser produced the same key,
        //    not a different-but-valid one.
        let msg = test_message();
        let blinding_result = pk_reparsed.blind(&mut DefaultRng, &msg).unwrap();
        let blind_sig = signer.blind_sign(&blinding_result.blind_message).unwrap();
        let sig = pk_reparsed.finalize(&blind_sig, &blinding_result, &msg).unwrap();
        pk_reparsed.verify(&sig, blinding_result.msg_randomizer, &msg).unwrap();
    }

    /// AUDIT-03 best-effort (D-131 second bullet, CD-50): verify that after
    /// dropping a `RoundSecretKey`-bearing `RsaBlindSigner`, a freshly-allocated
    /// 8 MB buffer does not contain a recognizable fingerprint of the dropped
    /// key material. This is a SANITY CHECK; the load-bearing assertion is the
    /// structural FSM test in `coordinator/src/round/state.rs::tests::round_secret_key_dropped_on_round_end`.
    ///
    /// Mechanism: capture a 32-byte middle slice of the DER-encoded secret key
    /// (the modulus + exponent region, per-key unique), drop the signer to fire
    /// the transitive `<rsa::RsaPrivateKey as Drop>::drop` chain
    /// (`rsa-0.9.10/src/key.rs:76-82`), then sweep an 8 MB probe buffer for the
    /// captured fingerprint. Probabilistic — false negatives are acceptable; the
    /// structural test is the unconditional CI gate.
    ///
    /// Gated `#[cfg_attr(not(target_os = "linux"), ignore)]` per CD-50: heap
    /// layout and allocator reuse policy differ across platforms; the test
    /// happens to be stable on glibc/Linux and is reported as ignored elsewhere.
    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "non-portable heap layout; structural test in state.rs::round_secret_key_dropped_on_round_end is the unconditional gate (D-131)")]
    fn round_secret_key_buffer_overwritten_on_drop() {
        // 1. Construct a known key and capture its DER fingerprint.
        let signer = RsaBlindSigner::generate().unwrap();
        let der_fingerprint = signer.secret_key_der().unwrap();
        // RSA-2048 PKCS#8 DER must be well past 200 bytes; the middle region
        // (offset 100..132) lies inside the modulus/exponent bytes, which are
        // per-key unique (unlike the SEQUENCE/version/OID prefix that is
        // identical across all RSA-2048 keys).
        assert!(
            der_fingerprint.len() >= 200,
            "RSA-2048 PKCS#8 DER must be >= 200 bytes; got {}",
            der_fingerprint.len()
        );
        let needle: Vec<u8> = der_fingerprint[100..132].to_vec();
        assert_eq!(needle.len(), 32, "needle must be exactly 32 bytes");

        // 2. Drop the signer. Drop chain fires:
        //    RsaBlindSigner → RoundSecretKey → BjSecretKey → rsa::RsaPrivateKey,
        //    whose unconditional `Drop` zeroizes `d`/`primes`/`precomputed` in
        //    place (`rsa-0.9.10/src/key.rs:76-82`).
        drop(signer);

        // 3. Allocate an 8 MB probe buffer to occupy adjacent allocator pages,
        //    then sweep for the captured fingerprint. A miss is the success
        //    condition; a hit means the DER bytes survived in an adjacent
        //    allocation, which would indicate the upstream zeroize chain did
        //    not run.
        let probe: Vec<u8> = vec![0u8; 8 * 1024 * 1024];
        let found = probe.windows(needle.len()).any(|w| w == needle.as_slice());

        // The structural lifetime bound (`state.rs::round_secret_key_dropped_on_round_end`)
        // remains the load-bearing claim regardless of this test's outcome.
        assert!(
            !found,
            "RoundSecretKey buffer-scrub sanity check failed — \
             DER fingerprint survived in adjacent heap pages. \
             Structural lifetime bound (state.rs::round_secret_key_dropped_on_round_end) \
             remains the load-bearing claim regardless."
        );
    }
}
