use reqwest::{Client, Proxy};
use shared::protocol::*;
use anyhow::Result;

/// Bounded 429 retry budget for write endpoints (H4). The coordinator rate-limits
/// through a single GLOBAL bucket (Tor gives no per-client key), so a 429 does not
/// mean this client misbehaved — a burst of final-phase clients or an attacker can
/// momentarily empty the shared bucket. Give up after this many attempts so a
/// persistently-saturated coordinator surfaces an error rather than hanging.
const MAX_429_RETRIES: u32 = 5;

/// Floor for 429 backoff so a `Retry-After: 0` (sub-second bucket) never busy-spins.
const RETRY_FLOOR_MS: u64 = 250;

/// Parse a `Retry-After` delta-seconds header into a millisecond backoff, floored
/// at `floor_ms`. Shared by the read poll loop and the write-endpoint retry.
fn retry_after_backoff_ms(resp: &reqwest::Response, floor_ms: u64) -> u64 {
    resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|secs| secs.saturating_mul(1000))
        .unwrap_or(floor_ms)
        .max(floor_ms)
}

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

    /// Send a request, transparently retrying on HTTP 429 up to [`MAX_429_RETRIES`]
    /// times, honoring `Retry-After` (H4). `build` reconstructs the request each
    /// attempt because a reqwest request is single-use. A client 429'd on
    /// `/round/sign` and given up on would miss the signing window and be banned as
    /// a non-signer, so writes must tolerate the shared-bucket 429 the same way the
    /// read poll loop does.
    async fn send_with_429_retry(
        &self,
        build: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        let mut attempt = 0u32;
        loop {
            let resp = build().send().await?;
            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
                && attempt < MAX_429_RETRIES
            {
                attempt += 1;
                let backoff = retry_after_backoff_ms(&resp, RETRY_FLOOR_MS);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                continue;
            }
            return Ok(resp);
        }
    }

    pub async fn get_info(&self) -> Result<InfoResponse> {
        // H4: also 429-tolerant. `register_output` re-fetches /info AFTER input
        // registration, so a fatal 429 here would abort a client that has already
        // registered its input → never signs → banned as a non-signer. Same
        // read-bucket exhaustion path as /round/tx. (The signing poll loop does its
        // own request with bespoke 429 handling and does not route through here.)
        let url = format!("{}/info", self.base_url);
        let resp = self
            .send_with_429_retry(|| self.alice_client.get(&url))
            .await?
            .error_for_status()?
            .json::<InfoResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_input(&self, req: InputRegRequest) -> Result<InputRegResponse> {
        let url = format!("{}/round/input", self.base_url);
        let resp = self
            .send_with_429_retry(|| self.alice_client.post(&url).json(&req))
            .await?
            .error_for_status()?
            .json::<InputRegResponse>()
            .await?;
        Ok(resp)
    }

    /// Output registration uses the Bob circuit — the isolated Tor circuit that
    /// cannot be linked to the Alice (input registration) circuit by a network observer.
    pub async fn post_output(&self, req: OutputRegRequest) -> Result<OutputRegResponse> {
        let url = format!("{}/round/output", self.base_url);
        let resp = self
            .send_with_429_retry(|| self.bob().post(&url).json(&req))
            .await?
            .error_for_status()?
            .json::<OutputRegResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn get_tx(&self) -> Result<RoundTxResponse> {
        // H4: the last 429-fatal call. /round/tx is read-bucket rate-limited, and a
        // client fetching the tx AFTER the signing poll succeeded would otherwise
        // abort on a 429 → never sign → get banned as a non-signer. Same
        // Retry-After-tolerant retry as the poll loop and the write endpoints.
        let url = format!("{}/round/tx", self.base_url);
        let resp = self
            .send_with_429_retry(|| self.alice_client.get(&url))
            .await?
            .error_for_status()?
            .json::<RoundTxResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn post_sign(&self, req: SignRequest) -> Result<()> {
        let url = format!("{}/round/sign", self.base_url);
        let resp = self
            .send_with_429_retry(|| self.alice_client.post(&url).json(&req))
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("POST /round/sign failed: {} — body: {}", status, body);
        }
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
                let resp = self.alice_client
                    .get(format!("{}/info", self.base_url))
                    .send().await?;

                // H4: the coordinator rate-limits reads through a single GLOBAL bucket
                // — under Tor there is no per-client key, so a 429 does NOT mean *this*
                // client misbehaved, only that the shared bucket is momentarily empty.
                // Treating it as fatal (the old `error_for_status()?`) meant a client
                // that had already registered its output would abort here, never sign,
                // and get its UTXO banned as a non-signer. Instead, back off — honoring
                // the coordinator's `Retry-After` — and keep polling until `max_wait`.
                if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    let backoff_ms = retry_after_backoff_ms(&resp, interval_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    continue;
                }

                let info = resp.error_for_status()?.json::<InfoResponse>().await?;
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
