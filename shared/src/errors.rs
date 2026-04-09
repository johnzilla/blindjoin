use serde::{Deserialize, Serialize};

/// Error codes used in API responses.
///
/// Serializes to SCREAMING_SNAKE_CASE strings for machine-readable error handling.
/// Wire format: `{"error": {"code": "UTXO_SPENT", "message": "...", "round_id": "..."}}`
/// Note: ApiError is serialized directly; the axum response layer wraps it in {"error": ...}
/// using `Json(serde_json::json!({"error": api_error}))` — see plan 04 for handler impl.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    UtxoSpent,
    UtxoNotFound,
    UtxoInsufficientValue,
    UtxoAlreadyRegistered,
    InvalidOwnershipProof,
    WrongPhase,
    TokenAlreadyUsed,
    InvalidToken,
    WrongDenomination,
    RoundFull,
    RpcUnavailable,
    BroadcastRejected,
    SessionInvalid,
    DustOutput,
    UtxoBanned,
}

/// API error returned by all coordinator endpoints on failure.
///
/// Serialized as `{"code": "...", "message": "...", "round_id": "..."}`.
/// Axum handlers wrap this in `{"error": ...}` at the response layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_serializes_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&ErrorCode::UtxoSpent).unwrap(),
            "\"UTXO_SPENT\""
        );
    }

    #[test]
    fn api_error_serializes_without_round_id() {
        let err = ApiError {
            code: ErrorCode::UtxoSpent,
            message: "foo".into(),
            round_id: None,
        };
        let json = serde_json::to_string(&err).unwrap();
        // Axum wraps this in {"error": ...} at response layer
        assert!(json.contains("\"UTXO_SPENT\""));
        assert!(json.contains("\"foo\""));
        assert!(!json.contains("round_id"));
    }

    #[test]
    fn api_error_serializes_with_round_id() {
        let err = ApiError {
            code: ErrorCode::UtxoSpent,
            message: "spent".into(),
            round_id: Some("abc".into()),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"abc\""));
    }
}
