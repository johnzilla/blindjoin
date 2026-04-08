use bitcoin::Script;
use sha2::{Digest, Sha256};

/// Compute the blind token message for a CoinJoin output.
///
/// Format: SHA-256("blindjoin-v1" || script_pubkey_bytes || amount_sats_le64)
///
/// CANONICAL RULES (must match between coordinator and client):
/// - script_pubkey_bytes = output_script.as_bytes() — raw script bytes WITHOUT CompactSize length prefix
/// - amount_sats_le64 = amount_sats.to_le_bytes() — 8 bytes, little-endian u64
///
/// This function MUST live in shared/ and be used by both coordinator and client
/// to guarantee byte-identical output (T-01-01 threat mitigation).
pub fn compute_blind_token_message(output_script: &Script, amount_sats: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"blindjoin-v1");
    hasher.update(output_script.as_bytes());
    hasher.update(amount_sats.to_le_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::ScriptBuf;

    #[test]
    fn compute_blind_token_message_deterministic() {
        // P2WPKH script: OP_0 <20-byte-hash>
        let script = ScriptBuf::from_bytes(vec![
            0x00, 0x14, // OP_0, PUSH20
            0x89, 0xab, 0xcd, 0xef, 0xab, 0xcd, 0xef, 0xab, 0xcd, 0xef, 0xab, 0xcd, 0xef, 0xab,
            0xcd, 0xef, 0xab, 0xcd, 0xef, 0xab,
        ]);
        let amount_sats: u64 = 1_000_000;
        let result1 = compute_blind_token_message(&script, amount_sats);
        let result2 = compute_blind_token_message(&script, amount_sats);
        assert_eq!(result1, result2);
        assert_eq!(result1.len(), 32);
    }

    #[test]
    fn compute_blind_token_message_different_amounts() {
        let script = ScriptBuf::from_bytes(vec![0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let r1 = compute_blind_token_message(&script, 1_000_000);
        let r2 = compute_blind_token_message(&script, 1_000_001);
        assert_ne!(r1, r2, "Different amounts must produce different tokens");
    }

    #[test]
    fn forward_compat_unknown_fields() {
        use crate::protocol::InputRegRequest;
        let json = r#"{
            "utxo_outpoint": "abcd:0",
            "ownership_proof": "[\"deadbeef\",\"cafebabe\"]",
            "blinded_token": "token",
            "change_address": "tb1q...",
            "unknown_future_field": "some_value"
        }"#;
        let result: Result<InputRegRequest, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Unknown fields must not cause deserialization error");
    }

    #[test]
    fn ownership_proof_roundtrip() {
        use crate::protocol::OwnershipProof;
        let stack = vec![
            vec![0x30u8, 0x45, 0x02, 0x21],
            vec![0x02u8, 0xab, 0xcd, 0xef],
        ];
        let proof = OwnershipProof {
            witness_stack: stack.clone(),
        };
        let encoded = proof.to_json_hex_str();
        let decoded = OwnershipProof::from_json_hex_str(&encoded).unwrap();
        assert_eq!(decoded.witness_stack, stack);
    }
}
