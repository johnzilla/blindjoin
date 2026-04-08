use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use bitcoin::Txid;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("RPC error code {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Broadcast rejected: {reason}")]
    BroadcastRejected { reason: String },
    #[error("Bitcoin Core unreachable: {0}")]
    Unreachable(String),
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Value,
    error: Option<RpcErrorBody>,
}

#[derive(Deserialize)]
struct RpcErrorBody {
    code: i64,
    message: String,
}

pub struct BitcoinRpc {
    client: Client,
    url: String,
    user: String,
    pass: String,
}

impl BitcoinRpc {
    pub fn new(url: String, user: String, pass: String) -> Self {
        Self { client: Client::new(), url, user, pass }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let body = json!({
            "jsonrpc": "1.1",
            "id": "blindjoin",
            "method": method,
            "params": params,
        });
        let resp = self.client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.pass))
            .json(&body)
            .send()
            .await
            .map_err(|e| RpcError::Unreachable(e.to_string()))?
            .json::<RpcResponse>()
            .await?;
        if let Some(err) = resp.error {
            // -25 = missing inputs / broadcast rejected
            // -26 = mempool min fee not met
            // -27 = already in mempool
            if err.code == -25 || err.code == -26 || err.code == -27 {
                return Err(RpcError::BroadcastRejected { reason: err.message });
            }
            return Err(RpcError::Rpc { code: err.code, message: err.message });
        }
        Ok(resp.result)
    }

    /// Returns None if the UTXO is spent or doesn't exist.
    pub async fn gettxout(
        &self,
        txid: &Txid,
        vout: u32,
    ) -> Result<Option<corepc_types::v26::GetTxOut>, RpcError> {
        let result = self.call("gettxout", json!([txid.to_string(), vout, false])).await?;
        if result.is_null() {
            return Ok(None);
        }
        let txout = serde_json::from_value(result)
            .map_err(|e| RpcError::Parse(e.to_string()))?;
        Ok(Some(txout))
    }

    pub async fn sendrawtransaction(&self, hex: &str) -> Result<Txid, RpcError> {
        let result = self.call("sendrawtransaction", json!([hex])).await?;
        let txid_str = result.as_str()
            .ok_or_else(|| RpcError::Parse("expected txid string".into()))?;
        Txid::from_str(txid_str).map_err(|e| RpcError::Parse(e.to_string()))
    }

    pub async fn testmempoolaccept(&self, hex_list: &[&str]) -> Result<Value, RpcError> {
        self.call("testmempoolaccept", json!([hex_list])).await
    }

    pub async fn getblockcount(&self) -> Result<u64, RpcError> {
        let result = self.call("getblockcount", json!([])).await?;
        result.as_u64().ok_or_else(|| RpcError::Parse("expected u64".into()))
    }

    pub async fn getrawtransaction(&self, txid: &Txid, verbose: bool) -> Result<Value, RpcError> {
        self.call("getrawtransaction", json!([txid.to_string(), verbose])).await
    }
}
