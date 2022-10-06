use std::sync::Arc;
use std::time::Duration;


use async_trait::async_trait;
use jsonrpsee_core::client::{SubscriptionClientT, ClientT};
use jsonrpsee_ws_client::types::params::ParamsSer as JsonRpcParams;
use jsonrpsee_core::client::Subscription;
use jsonrpsee_core::error::Error as JsonRpcWsError;
use serde_json::Value as JsonValue;
use log::error;
use sp_core::H256 as Hash;

use crate::ApiClientError;
use crate::ApiResult;
use crate::FromHexString;
use crate::RpcClient as RpcClientTrait;
use crate::XtStatus;

#[derive(Debug, Clone)]
pub struct WsRpcClient {
    client: Arc<jsonrpsee_ws_client::WsClient>,
    url: String,
}

impl WsRpcClient {
    pub async fn new(url: &str, timeout: Option<Duration>) -> Result<Self, JsonRpcWsError> {
        let mut builder = jsonrpsee_ws_client::WsClientBuilder::default()
            .max_request_body_size(1024 * 1024 * 1024); // 1 GiB
        if let Some(t) = timeout {
            builder = builder.request_timeout(t);
        }
        Ok(WsRpcClient {
            client: Arc::new(builder.build(url).await?),
            url: url.to_owned(),
        })
    }
}

#[async_trait]
impl RpcClientTrait for WsRpcClient {
    async fn get_request(&self, method: &str, params: JsonRpcParams<'_>) -> ApiResult<JsonValue> {
        let str: JsonValue = self.client.request(method, Some(params)).await?;
        Ok(str)
    }

    async fn send_extrinsic(
        &self,
        xt_hex_prefixed: String,
        exit_on: XtStatus,
    ) -> ApiResult<Option<sp_core::H256>> {
        let mut sub: Subscription<serde_json::Value> = self
            .client
            .subscribe(
                "author_submitAndWatchExtrinsic",
                Some(JsonRpcParams::Array(vec![JsonValue::String(xt_hex_prefixed)])),
                "author_unwatchExtrinsic",
            )
            .await?;

        loop {
            let maybe_update = sub.next().await;
            if let Some(update) = maybe_update {
                // log::info!("Status update for pending extrinsic: {}", update);
                // todo check why not display
                let (status, val) = parse_status(update?)?;
                log::info!("Parsed status update for pending extrinsic: {:?}", status);

                if status == XtStatus::Future {
                    log::warn!("Extrinsic has 'future' status. aborting");
                    return Ok(None);
                }
                if status_reached(&status, &exit_on) {
                    return match val {
                        None => Ok(None),
                        Some(x) => Hash::from_hex(x)
                            .map(|h| Some(h))
                            .map_err(|e| ApiClientError::InvalidHexString(e)),
                    };
                }
            } else {
                let msg = format!(
                    "Subscription ended before reaching desired status {:?}",
                    exit_on
                );
                error!("{}", msg);
                return Err(ApiClientError::Extrinsic(msg));
            }
        }
    }
}

fn parse_status(value: serde_json::Value) -> Result<(XtStatus, Option<String>), ApiClientError> {
    match value["error"].as_object() {
        Some(obj) => {
            let error = obj
                .get("message")
                .map_or_else(|| "", |e| e.as_str().unwrap());
            let code = obj.get("code").map_or_else(|| -1, |c| c.as_i64().unwrap());
            let details = obj.get("data").map_or_else(|| "", |d| d.as_str().unwrap());

            Err(ApiClientError::Extrinsic(format!(
                "extrinsic error code {}: {}: {}",
                code, error, details
            )))
        }
        None => match value.as_object() {
            Some(obj) => {
                if let Some(hash) = obj.get("finalized") {
                    log::info!("finalized: {:?}", hash);
                    Ok((XtStatus::Finalized, Some(hash.to_string())))
                } else if let Some(hash) = obj.get("inBlock") {
                    log::info!("inBlock: {:?}", hash);
                    Ok((XtStatus::InBlock, Some(hash.to_string())))
                } else if let Some(array) = obj.get("broadcast") {
                    log::info!("broadcast: {:?}", array);
                    Ok((XtStatus::Broadcast, Some(array.to_string())))
                } else {
                    Ok((XtStatus::Unknown, None))
                }
            }
            None => match value.as_str() {
                Some("ready") => Ok((XtStatus::Ready, None)),
                Some("future") => Ok((XtStatus::Future, None)),
                Some("invalid") => Err(ApiClientError::Extrinsic("Transaction is invalid or lifetime expired".to_owned())),
                Some(&_) => Ok((XtStatus::Unknown, None)),
                None => Ok((XtStatus::Unknown, None)),
            },
        },
    }
}

fn status_numeric(status: &XtStatus) -> i32 {
    match status {
        XtStatus::Finalized => 3,
        XtStatus::InBlock => 2,
        XtStatus::Broadcast => 1,
        XtStatus::Ready => 0,
        XtStatus::Future => -1,
        XtStatus::Error => -1,
        XtStatus::Unknown => -1,
    }
}

fn status_reached(current_status: &XtStatus, need_status: &XtStatus) -> bool {
    status_numeric(need_status) <= status_numeric(current_status)
}
