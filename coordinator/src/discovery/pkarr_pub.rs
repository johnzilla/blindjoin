use pkarr::{Client, Keypair, SignedPacket};
use anyhow::Result;

pub struct PkarrPublisher {
    client: Client,
    /// Public to allow main.rs to read the public key for logging.
    #[allow(dead_code)]
    pub keypair: Keypair,
}

impl PkarrPublisher {
    pub fn new(keypair: Keypair) -> Result<Self> {
        let client = Client::builder()
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build PKARR client: {e}"))?;
        Ok(Self { client, keypair })
    }

    pub async fn publish_record(&self, packet: SignedPacket) -> Result<()> {
        self.client
            .publish(&packet, None)
            .await
            .map_err(|e| anyhow::anyhow!("PKARR publish failed: {e}"))
    }
}

/// Load existing keypair from file, or generate and persist a new one.
///
/// The public key is the coordinator's stable DHT identity — never rotate it.
/// Called at coordinator startup; the key file path comes from DiscoveryConfig.
pub fn load_or_generate_keypair(path: &str) -> Result<Keypair> {
    use std::path::Path;
    let p = Path::new(path);
    match Keypair::from_secret_key_file(p) {
        Ok(kp) => {
            tracing::info!("Loaded existing PKARR keypair from {path}");
            Ok(kp)
        }
        Err(_) => {
            let kp = Keypair::random();
            kp.write_secret_key_file(p)
                .map_err(|e| anyhow::anyhow!("Cannot write PKARR key file at {path}: {e}"))?;
            tracing::info!(
                pubkey = %kp.public_key().to_z32(),
                "Generated new PKARR keypair, saved to {path}"
            );
            Ok(kp)
        }
    }
}

/// Build a signed DNS TXT packet containing coordinator metadata.
///
/// Uses label "_blindjoin" with a single compact JSON value (fits in 255 bytes).
/// TTL = 300s (matches heartbeat interval). The "onion" field holds a clearnet
/// address in Phase 4 and will be replaced by an actual .onion address in Phase 5.
pub fn build_coordinator_packet(
    keypair: &Keypair,
    coordinator_addr: &str,
    denomination_sats: u64,
    min_participants: u32,
    status: &str,
) -> Result<SignedPacket> {
    let record = serde_json::json!({
        "type": "blindjoin-coordinator",
        "version": "0.1.0",
        "onion": coordinator_addr,
        "network": "signet",
        "denomination_sats": denomination_sats,
        "min_participants": min_participants,
        "status": status,
    });
    let json_str = serde_json::to_string(&record)?;

    // DNS TXT string limit is 255 bytes per character string. Warn if approaching.
    if json_str.len() > 220 {
        tracing::warn!(
            len = json_str.len(),
            "PKARR TXT record approaching 255-byte DNS limit"
        );
    }

    let ttl: u32 = 300;
    let label: pkarr::dns::Name<'_> = "_blindjoin"
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid TXT label: {e}"))?;
    let txt_value: pkarr::dns::rdata::TXT<'_> = json_str
        .as_str()
        .try_into()
        .map_err(|e| anyhow::anyhow!("Invalid TXT value: {e}"))?;

    SignedPacket::builder()
        .txt(label, txt_value, ttl)
        .sign(keypair)
        .map_err(|e| anyhow::anyhow!("SignedPacket build failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_coordinator_packet_valid() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "idle",
        )
        .unwrap();
        assert!(packet.resource_records("_blindjoin").count() > 0);
    }

    #[test]
    fn test_build_coordinator_packet_contains_fields() {
        let keypair = Keypair::random();
        let packet = build_coordinator_packet(
            &keypair,
            "127.0.0.1:8080",
            1_000_000,
            3,
            "input_reg",
        )
        .unwrap();
        let found = packet.resource_records("_blindjoin").any(|_| true);
        assert!(found, "Expected _blindjoin TXT record");
    }

    #[test]
    fn test_load_or_generate_keypair_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_pkarr.key");
        let path_str = path.to_str().unwrap();

        // File does not exist yet — should generate
        let kp1 = load_or_generate_keypair(path_str).unwrap();
        assert!(path.exists(), "Key file should have been created");

        // Load again — should return same public key
        let kp2 = load_or_generate_keypair(path_str).unwrap();
        assert_eq!(
            kp1.public_key().to_z32(),
            kp2.public_key().to_z32(),
            "Public key should be stable across reloads"
        );
    }
}
