use pkarr::{Client, PublicKey};
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug)]
pub struct CoordinatorInfo {
    pub coordinator_url: String,
}

/// Resolve a coordinator URL from a PKARR public key string (z32 format).
///
/// Uses resolve_most_recent() to force a fresh DHT query (avoids stale cache).
/// Latency: 500ms–2s typical. Acceptable for one-time coordinator discovery.
///
/// The coordinator publishes a JSON blob under label "_blindjoin". Example:
///   {"type":"blindjoin-coordinator","onion":"127.0.0.1:8080","status":"idle",...}
/// In Phase 4 the "onion" field holds a clearnet address. Phase 5 will contain .onion.
pub async fn discover_coordinator(pkarr_pubkey: &str) -> Result<CoordinatorInfo> {
    let public_key: PublicKey = pkarr_pubkey
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid PKARR public key '{pkarr_pubkey}': {e}"))?;

    let client = Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build PKARR client: {e}"))?;

    let packet = client
        .resolve_most_recent(&public_key)
        .await
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Coordinator not found in DHT for key '{pkarr_pubkey}'"
            )
        })?;

    // Parse the _blindjoin TXT record JSON to extract the "onion" field.
    let coordinator_addr = packet
        .resource_records("_blindjoin")
        .find_map(|rr| parse_onion_from_rr(rr))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No 'onion' field found in PKARR record for key '{pkarr_pubkey}'"
            )
        })?;

    // Phase 4: coordinator_addr is "host:port" clearnet. Phase 5: will be .onion.
    // Build http:// URL for the existing CoordinatorClient.
    let coordinator_url = if coordinator_addr.starts_with("http") {
        coordinator_addr
    } else {
        format!("http://{coordinator_addr}")
    };

    tracing::info!(
        coordinator_url = %coordinator_url,
        "Resolved coordinator via PKARR DHT"
    );

    Ok(CoordinatorInfo { coordinator_url })
}

/// Extract the "onion" field value from a DNS TXT resource record's rdata.
///
/// pkarr TXT rdata is raw DNS wire bytes. TXT is a sequence of character strings
/// (each length-prefixed). We join all strings, interpret as UTF-8 JSON, then
/// extract the "onion" key.
fn parse_onion_from_rr(rr: &pkarr::dns::ResourceRecord<'_>) -> Option<String> {
    use pkarr::dns::rdata::RData;
    let txt = match &rr.rdata {
        RData::TXT(txt) => txt,
        _ => return None,
    };
    // TXT implements TryFrom<TXT> for String by joining all character string bytes
    let s = String::try_from(txt.clone()).ok()?;
    // Parse as JSON and extract "onion"
    #[derive(Deserialize)]
    struct Partial {
        onion: Option<String>,
    }
    serde_json::from_str::<Partial>(&s).ok()?.onion
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_pubkey_returns_error() {
        let err = discover_coordinator("not-a-valid-pkarr-key")
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Invalid PKARR public key"),
            "got: {err}"
        );
    }

    // Note: test for "not found in DHT" requires a real DHT query with an unknown key.
    // This is a live network test; run manually with:
    //   cargo test --lib -p client -- discover::tests --nocapture
    // For CI, the invalid-key test is sufficient to verify error path wiring.
}
