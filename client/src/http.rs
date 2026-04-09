use reqwest::{Client, Proxy};
use shared::protocol::*;
use anyhow::Result;

pub struct CoordinatorClient {
    /// Alice client — used for: poll, input reg, signing
    alice_client: Client,
    /// Bob client — used for output registration ONLY (different Tor circuit).
    /// None in clearnet mode (alice_client handles all requests).
    bob_client: Option<Client>,
    base_url: String,
}

impl CoordinatorClient {
    /// Clearnet constructor — no change in behavior from Phase 4.
    pub fn new(base_url: String) -> Self {
        Self {
            alice_client: Client::new(),
            bob_client: None,
            base_url,
        }
    }

    /// Tor constructor — alice and bob use separate reqwest clients routed through
    /// isolated arti SOCKS5 proxies (CLI-05).
    ///
    /// `alice_proxy` and `bob_proxy` must be `socks5h://127.0.0.1:<port>` URLs
    /// produced by `TorHandle::alice_proxy_url()` / `TorHandle::bob_proxy_url()`.
    ///
    /// T-05-10: `Proxy::all()` routes ALL traffic for each reqwest::Client through
    /// the given SOCKS5 proxy — no clearnet fallback is possible for that client.
    pub fn new_tor(base_url: String, alice_proxy: String, bob_proxy: String) -> Result<Self> {
        let alice_client = Client::builder()
            .proxy(Proxy::all(&alice_proxy)?)
            .build()?;
        let bob_client = Client::builder()
            .proxy(Proxy::all(&bob_proxy)?)
            .build()?;
        Ok(Self {
            alice_client,
            bob_client: Some(bob_client),
            base_url,
        })
    }

    /// Returns the Bob client for output registration, falling back to alice in clearnet mode.
    fn bob(&self) -> &Client {
        self.bob_client.as_ref().unwrap_or(&self.alice_client)
    }

    pub async fn get_info(&self) -> Result<InfoResponse> {
        let resp = self.alice_client
            .get(format!("{}/info", self.base_url))
            .send().await?
            .error_for_status()?
            .json::<InfoResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_input(&self, req: InputRegRequest) -> Result<InputRegResponse> {
        let resp = self.alice_client
            .post(format!("{}/round/input", self.base_url))
            .json(&req)
            .send().await?
            .error_for_status()?
            .json::<InputRegResponse>()
            .await?;
        Ok(resp)
    }

    /// Output registration uses the Bob circuit — the isolated Tor circuit that
    /// cannot be linked to the Alice (input registration) circuit by a network observer.
    pub async fn post_output(&self, req: OutputRegRequest) -> Result<OutputRegResponse> {
        let resp = self.bob()
            .post(format!("{}/round/output", self.base_url))
            .json(&req)
            .send().await?
            .error_for_status()?
            .json::<OutputRegResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn get_tx(&self) -> Result<RoundTxResponse> {
        let resp = self.alice_client
            .get(format!("{}/round/tx", self.base_url))
            .send().await?
            .error_for_status()?
            .json::<RoundTxResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_sign(&self, req: SignRequest) -> Result<()> {
        self.alice_client
            .post(format!("{}/round/sign", self.base_url))
            .json(&req)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    /// Poll GET /info until round_state matches expected, with interval.
    ///
    /// Returns an error if the phase is not reached within `max_wait`.
    /// The default callers pass `Duration::from_secs(600)` (10 minutes) which is
    /// enough headroom for a slow coordinator while still preventing infinite hangs
    /// when the coordinator crashes or the wrong phase name is supplied.
    pub async fn poll_until_phase(
        &self,
        expected_phase: &str,
        interval_ms: u64,
        max_wait: tokio::time::Duration,
    ) -> Result<InfoResponse> {
        tokio::time::timeout(max_wait, async {
            loop {
                let info = self.get_info().await?;
                if info.round_state == expected_phase {
                    return Ok(info);
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for phase: {expected_phase}"))?
    }
}
