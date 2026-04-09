use reqwest::Client;
use shared::protocol::*;
use anyhow::Result;

pub struct CoordinatorClient {
    client: Client,
    base_url: String,
}

impl CoordinatorClient {
    pub fn new(base_url: String) -> Self {
        Self { client: Client::new(), base_url }
    }

    pub async fn get_info(&self) -> Result<InfoResponse> {
        let resp = self.client
            .get(format!("{}/info", self.base_url))
            .send().await?
            .error_for_status()?
            .json::<InfoResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_input(&self, req: InputRegRequest) -> Result<InputRegResponse> {
        let resp = self.client
            .post(format!("{}/round/input", self.base_url))
            .json(&req)
            .send().await?
            .error_for_status()?
            .json::<InputRegResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_output(&self, req: OutputRegRequest) -> Result<OutputRegResponse> {
        let resp = self.client
            .post(format!("{}/round/output", self.base_url))
            .json(&req)
            .send().await?
            .error_for_status()?
            .json::<OutputRegResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn get_tx(&self) -> Result<RoundTxResponse> {
        let resp = self.client
            .get(format!("{}/round/tx", self.base_url))
            .send().await?
            .error_for_status()?
            .json::<RoundTxResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_sign(&self, req: SignRequest) -> Result<()> {
        self.client
            .post(format!("{}/round/sign", self.base_url))
            .json(&req)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    /// Poll GET /info until round_state matches expected, with interval
    pub async fn poll_until_phase(&self, expected_phase: &str, interval_ms: u64) -> Result<InfoResponse> {
        loop {
            let info = self.get_info().await?;
            if info.round_state == expected_phase {
                return Ok(info);
            }
            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
        }
    }
}
