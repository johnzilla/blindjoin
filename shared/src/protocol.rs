use serde::{Deserialize, Serialize};

// NO #[serde(deny_unknown_fields)] on any struct — forward compat per D-06 / T-01-04.
// All structs silently drop unknown fields, allowing protocol evolution without breaking
// older clients or coordinators.

/// GET /info response — coordinator status and round parameters.
///
/// rsa_pubkey_hash: hex SHA-256(DER pubkey bytes) — client MUST verify
///   SHA-256(decode(rsa_pubkey_der_b64)) == rsa_pubkey_hash before blinding (D-02).
/// rsa_pubkey_der_b64: base64 DER SubjectPublicKeyInfo; None when coordinator is Idle
///   (no key has been generated yet for the upcoming round).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoResponse {
    pub version: String,
    pub network: String,
    pub denomination_sats: u64,
    pub min_participants: u32,
    pub max_participants: u32,
    /// "idle" | "input_reg" | "output_reg" | "signing" | "broadcast" | "blame"
    pub round_state: String,
    pub participants_registered: u32,
    /// hex SHA-256(DER pubkey bytes); None when Idle
    pub rsa_pubkey_hash: Option<String>,
    /// base64 DER SubjectPublicKeyInfo; None when Idle
    pub rsa_pubkey_der_b64: Option<String>,
    pub round_id: Option<uuid::Uuid>,
}

/// POST /round/input request — register a UTXO for an upcoming CoinJoin round.
///
/// ownership_proof uses canonical wire format: JSON array of hex strings.
/// Use OwnershipProof::from_json_hex_str / to_json_hex_str helpers — never ad-hoc encoding (T-01-05).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRegRequest {
    /// "txid:vout" format
    pub utxo_outpoint: String,
    /// Canonical wire format: JSON array of hex strings, e.g. "[\"3045...\",\"02ab...\"]"
    /// Coordinator decodes via OwnershipProof::from_json_hex_str.
    pub ownership_proof: String,
    /// base64-encoded blinded message
    pub blinded_token: String,
    /// bech32 address for change output (linkable, documented)
    pub change_address: String,
}

/// POST /round/input response — blind signature and session token for this registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRegResponse {
    /// base64-encoded blind signature
    pub blind_signature: String,
    pub round_id: uuid::Uuid,
    /// base64-encoded [u8; 32] HMAC session token (D-05)
    pub session_token: String,
}

/// POST /round/output request — register the CoinJoin output using the unblinded token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRegRequest {
    /// base64-encoded original message M (the 32-byte hash from compute_blind_token_message)
    pub unblinded_token: String,
    /// base64-encoded unblinded RSA signature
    pub signature: String,
    /// bech32 address for CoinJoin output
    pub output_address: String,
    pub amount_sats: u64,
    /// base64-encoded 32-byte MessageRandomizer from BlindingResult (required for Randomized mode).
    /// The client obtains this from BlindingResult.msg_randomizer during the blinding step.
    /// RSABSSA-SHA384-PSS-Randomized (RFC 9474 §3.3.2) requires this for signature verification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_randomizer: Option<String>,
}

/// POST /round/output response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRegResponse {
    pub accepted: bool,
    pub round_id: uuid::Uuid,
}

/// POST /round/sign request — submit a partial PSBT signature.
///
/// Uses utxo_outpoint (NOT input_index) per design doc correction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    pub round_id: uuid::Uuid,
    /// "txid:vout" — NOT input_index (design doc correction)
    pub utxo_outpoint: String,
    /// base64-encoded PSBT partial sig bytes
    pub partial_signature: String,
    /// base64-encoded session token from InputRegResponse (D-05)
    pub session_token: String,
}

/// GET /round/tx response — the assembled PSBT for participant signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundTxResponse {
    pub round_id: uuid::Uuid,
    /// base64-encoded PSBT
    pub psbt: String,
    pub fee_total_sats: u64,
    pub fee_per_participant_sats: u64,
}

/// Canonical wire representation of a BIP-322 Simple ownership proof.
///
/// Wire format: JSON array of hex strings, one per witness stack item.
/// Example: ["3045022100...01", "02abc123..."]
///
/// This type MUST be used identically by client (encoding) and coordinator (decoding)
/// to prevent format mismatch (T-01-05 threat mitigation).
/// Never pass raw bytes directly in the JSON ownership_proof field.
pub struct OwnershipProof {
    /// The raw witness stack items (decoded from hex on receive, encoded to hex on send)
    pub witness_stack: Vec<Vec<u8>>,
}

impl OwnershipProof {
    /// Decode from the canonical wire format: JSON array of hex strings.
    /// Used by coordinator to decode the ownership_proof field of InputRegRequest.
    pub fn from_json_hex_str(s: &str) -> Result<Self, String> {
        let items: Vec<String> = serde_json::from_str(s)
            .map_err(|e| format!("OwnershipProof: JSON parse error: {e}"))?;
        let witness_stack = items
            .iter()
            .map(|h| {
                hex::decode(h).map_err(|e| format!("OwnershipProof: hex decode error: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { witness_stack })
    }

    /// Encode to the canonical wire format: JSON array of hex strings.
    /// Used by client to populate the ownership_proof field of InputRegRequest.
    pub fn to_json_hex_str(&self) -> String {
        let hex_items: Vec<String> = self.witness_stack.iter().map(hex::encode).collect();
        serde_json::to_string(&hex_items).expect("Vec<String> always serializes")
    }
}
