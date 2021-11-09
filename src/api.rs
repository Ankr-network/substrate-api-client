use std::convert::TryFrom;

pub use ::metadata::RuntimeMetadataPrefixed;
use async_trait::async_trait;
use codec::{Decode, Encode};
use jsonrpsee_ws_client::types::error::Error as JsonRpcWsError;
use jsonrpsee_ws_client::types::v2::params::JsonRpcParams;
use jsonrpsee_ws_client::types::JsonValue;
use log::{debug, info};
use serde::de::DeserializeOwned;
pub use serde_json::Value;
pub use sp_core::crypto::Pair;
pub use sp_core::storage::StorageKey;
pub use sp_runtime::traits::{Block, Header};
pub use sp_runtime::{
    generic::SignedBlock, traits::IdentifyAccount, AccountId32 as AccountId, MultiSignature,
    MultiSigner,
};
pub use sp_std::prelude::*;
pub use sp_version::RuntimeVersion;
pub use transaction_payment::FeeDetails;

pub use crate::metadata::Metadata;
use crate::rpc::json_req;
pub use crate::rpc::XtStatus;
pub use crate::utils::FromHexString;
use crate::{extrinsic, metadata, Balance};
use crate::{AccountData, AccountInfo, Hash};

pub type ApiResult<T> = Result<T, ApiClientError>;

#[async_trait]
pub trait RpcClient {
    /// Sends a RPC request that returns a String
    async fn get_request(&self, method: &str, params: JsonRpcParams<'_>) -> ApiResult<JsonValue>;

    /// Send a RPC request that returns a SHA256 hash
    async fn send_extrinsic(
        &self,
        xthex_prefixed: String,
        exit_on: XtStatus,
    ) -> ApiResult<Option<Hash>>;
}

#[derive(Clone)]
pub struct Api<Client>
where
    Client: RpcClient,
{
    pub genesis_hash: Hash,
    pub metadata: Metadata,
    pub spec_version: u32,
    pub transaction_version: u32,
    client: Client,
}

impl<Client> Api<Client>
where
    Client: RpcClient,
{
    pub async fn new(client: Client) -> ApiResult<Self> {
        let genesis_hash = Self::_get_genesis_hash(&client).await?;
        info!("Got genesis hash: {:?}", genesis_hash);

        let metadata = Self::_get_metadata(&client)
            .await
            .map(Metadata::try_from)??;
        debug!("Metadata: {:?}", metadata);

        let (spec_version, transaction_version) = Self::_get_runtime_version(&client).await?;
        info!("Runtime version: {:?}", spec_version);

        Ok(Self {
            genesis_hash,
            metadata,
            spec_version,
            transaction_version,
            client,
        })
    }

    async fn _get_genesis_hash(client: &Client) -> ApiResult<Hash> {
        let genesis =
            Self::_get_request(client, "chain_getBlockHash", json_req::num_params(Some(0))).await?;

        match genesis.and_then(|g| g.as_str().map(|s| s.to_owned())) {
            Some(g) => Hash::from_hex(g).map_err(|e| e.into()),
            None => Err(ApiClientError::Genesis),
        }
    }

    async fn _get_runtime_version(client: &Client) -> ApiResult<(u32, u32)> {
        let version =
            Self::_get_request(client, "state_getRuntimeVersion", json_req::null_params()).await?;

        match version {
            Some(r) => r["specVersion"]
                .as_u64()
                .zip(r["transactionVersion"].as_u64())
                .ok_or_else(|| ApiClientError::RuntimeVersion)
                .map(|(s, v)| (s as u32, v as u32)),
            None => Err(ApiClientError::RuntimeVersion),
        }
    }

    async fn _get_metadata(client: &Client) -> ApiResult<RuntimeMetadataPrefixed> {
        let meta = Self::_get_request(client, "state_getMetadata", json_req::null_params())
            .await?
            .and_then(|m| m.as_str().map(str::to_owned));

        if meta.is_none() {
            return Err(ApiClientError::MetadataFetch);
        }
        let metadata = Vec::from_hex(meta.unwrap())?;
        RuntimeMetadataPrefixed::decode(&mut metadata.as_slice()).map_err(|e| e.into())
    }

    // low level access
    async fn _get_request(
        client: &Client,
        method: &str,
        params: JsonRpcParams<'_>,
    ) -> ApiResult<Option<JsonValue>> {
        let result = client.get_request(method, params).await?;
        if result.is_null() {
            return Ok(None);
        }
        Ok(Some(result))
    }

    pub async fn get_metadata(&self) -> ApiResult<RuntimeMetadataPrefixed> {
        Self::_get_metadata(&self.client).await
    }

    pub async fn get_spec_version(&self) -> ApiResult<u32> {
        Self::_get_runtime_version(&self.client)
            .await
            .map(|(x, _)| x)
    }

    pub async fn get_genesis_hash(&self) -> ApiResult<Hash> {
        Self::_get_genesis_hash(&self.client).await
    }

    pub async fn get_account_info(&self, address: &AccountId) -> ApiResult<Option<AccountInfo>> {
        let storagekey: sp_core::storage::StorageKey = self
            .metadata
            .storage_map_key::<AccountId, AccountInfo>("System", "Account", address.clone())?;
        info!("storagekey {:?}", storagekey);
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey, None).await
    }

    pub async fn get_account_data(&self, address: &AccountId) -> ApiResult<Option<AccountData>> {
        self.get_account_info(address)
            .await
            .map(|info| info.map(|i| i.data))
    }

    pub async fn get_finalized_head(&self) -> ApiResult<Option<Hash>> {
        let h = self
            .get_request("chain_getFinalizedHead", JsonRpcParams::Array(vec![]))
            .await?;
        match h.and_then(|v| v.as_str().map(str::to_owned)) {
            Some(hash) => Ok(Some(Hash::from_hex(hash)?)),
            None => Ok(None),
        }
    }

    pub async fn get_header<H>(&self, hash: Option<Hash>) -> ApiResult<Option<H>>
    where
        H: Header + DeserializeOwned,
    {
        let h = self
            .get_request("chain_getHeader", json_req::hash_params(hash))
            .await?;
        match h {
            Some(v) => Ok(Some(serde_json::from_value(v)?)),
            None => Ok(None),
        }
    }

    pub async fn get_block<B>(&self, hash: Option<Hash>) -> ApiResult<Option<B>>
    where
        B: Block + DeserializeOwned,
    {
        Self::get_signed_block(self, hash)
            .await
            .map(|sb_opt| sb_opt.map(|sb| sb.block))
    }

    /// A signed block is a block with Justification ,i.e., a Grandpa finality proof.
    /// The interval at which finality proofs are provided is set via the
    /// the `GrandpaConfig.justification_period` in a node's service.rs.
    /// The Justification may be none.
    pub async fn get_signed_block<B>(&self, hash: Option<Hash>) -> ApiResult<Option<SignedBlock<B>>>
    where
        B: Block + DeserializeOwned,
    {
        let b = self
            .get_request("chain_getBlock", json_req::hash_params(hash))
            .await?;
        match b {
            Some(block) => Ok(Some(serde_json::from_value(block)?)),
            None => Ok(None),
        }
    }

    pub async fn get_request(
        &self,
        method: &str,
        params: JsonRpcParams<'_>,
    ) -> ApiResult<Option<JsonValue>> {
        Self::_get_request(&self.client, method, params).await
    }

    pub async fn get_storage_value<V: Decode>(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<V>> {
        let storagekey = self
            .metadata
            .storage_value_key(storage_prefix, storage_key_name)?;
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey, at_block).await
    }

    pub async fn query_storage_at(
        &self,
        keys: Vec<StorageKey>,
        at_block: Option<Hash>,
    ) -> ApiResult<Vec<(StorageKey, StorageKey)>> {
        self.get_request(
            "state_queryStorageAt",
            json_req::state_query_storage_at(keys, at_block),
        )
        .await?
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_queryStorageAt did not return any data".to_owned())
        })?
        .as_array()
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_queryStorageAt returned not an array".to_owned())
        })?
        .get(0)
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_queryStorageAt returned empty array".to_owned())
        })?
        .as_object()
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_queryStorageAt: unexpected structure".to_owned())
        })?
        .get("changes")
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_queryStorageAt: no `changes` field".to_owned())
        })?
        .as_array()
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_queryStorageAt: `changes` is not an array".to_owned())
        })?
        .iter()
        .map(parse_pair)
        .collect()
    }

    pub async fn get_keys_paged(
        &self,
        prefix: StorageKey,
        count: u32,
        start_key: Option<StorageKey>,
        at: Option<Hash>,
    ) -> ApiResult<Vec<StorageKey>> {
        self.get_request(
            "state_getKeysPaged",
            json_req::state_get_keys_paged(prefix, count, start_key, at),
        )
        .await?
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_getKeysPaged did not return any data".to_owned())
        })?
        .as_array()
        .ok_or_else(|| {
            ApiClientError::RpcClient("state_getKeysPaged returned not an array".to_owned())
        })?
        .iter()
        .map(|v| {
            v.as_str().ok_or_else(|| {
                ApiClientError::RpcClient("state_getKeysPaged returned bad array".to_owned())
            })
        })
        .map(|v| v.and_then(|x| hex::decode(x.trim_start_matches("0x")).map_err(|e| e.into())))
        .map(|r| r.map(StorageKey))
        .collect()
    }

    pub async fn get_child_storage_by_hash<V: Decode + Clone>(
        &self,
        child: StorageKey,
        key_hash: StorageKey,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<V>> {
        let s = self
            .get_opaque_child_storage_by_hash(child, key_hash, at_block)
            .await?;
        match s {
            Some(storage) => Ok(Some(Decode::decode(&mut storage.as_slice())?)),
            None => Ok(None),
        }
    }

    pub async fn get_opaque_child_storage_by_hash(
        &self,
        child: StorageKey,
        key_hash: StorageKey,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<Vec<u8>>> {
        let s = self
            .get_request(
                "childstate_getStorage",
                json_req::state_get_child_storage(child, key_hash, at_block),
            )
            .await?;

        match s.and_then(|v| v.as_str().map(str::to_owned)) {
            Some(storage) => Ok(Some(Vec::from_hex(storage)?)),
            None => Ok(None),
        }
    }

    pub async fn get_storage_map<K: Encode, V: Decode + Clone>(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
        map_key: K,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<V>> {
        let storagekey =
            self.metadata
                .storage_map_key::<K, V>(storage_prefix, storage_key_name, map_key)?;
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey, at_block).await
    }

    pub fn get_storage_map_key_prefix(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
    ) -> ApiResult<StorageKey> {
        self.metadata
            .storage_map_key_prefix(storage_prefix, storage_key_name)
            .map_err(|e| e.into())
    }

    pub async fn get_storage_double_map<K: Encode, Q: Encode, V: Decode + Clone>(
        &self,
        storage_prefix: &'static str,
        storage_key_name: &'static str,
        first: K,
        second: Q,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<V>> {
        let storagekey = self.metadata.storage_double_map_key::<K, Q, V>(
            storage_prefix,
            storage_key_name,
            first,
            second,
        )?;
        info!("storage key is: 0x{}", hex::encode(storagekey.0.clone()));
        self.get_storage_by_key_hash(storagekey, at_block).await
    }

    pub async fn get_storage_by_key_hash<V: Decode>(
        &self,
        key: StorageKey,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<V>> {
        let s = self.get_opaque_storage_by_key_hash(key, at_block).await?;
        match s {
            Some(storage) => Ok(Some(Decode::decode(&mut storage.as_slice())?)),
            None => Ok(None),
        }
    }

    pub async fn get_opaque_storage_by_key_hash(
        &self,
        key: StorageKey,
        at_block: Option<Hash>,
    ) -> ApiResult<Option<Vec<u8>>> {
        let s = self
            .get_request(
                "state_getStorage",
                json_req::state_get_storage(key, at_block),
            )
            .await?;

        match s.and_then(|v| v.as_str().map(str::to_owned)) {
            Some(storage) => Ok(Some(Vec::from_hex(storage)?)),
            None => Ok(None),
        }
    }

    pub fn get_existential_deposit(&self) -> ApiResult<Balance> {
        let module = self.metadata.module_with_constants_by_name("Balances")?;
        let constant_metadata = module.constant_by_name("ExistentialDeposit")?;
        Decode::decode(&mut constant_metadata.get_value().as_slice()).map_err(|e| e.into())
    }

    pub async fn send_extrinsic(
        &self,
        xthex_prefixed: String,
        exit_on: XtStatus,
    ) -> ApiResult<Option<Hash>> {
        debug!("sending extrinsic: {:?}", xthex_prefixed);
        self.client.send_extrinsic(xthex_prefixed, exit_on).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiClientError {
    #[error("Fetching genesis hash failed. Are you connected to the correct endpoint?")]
    Genesis,
    #[error("Fetching runtime version failed. Are you connected to the correct endpoint?")]
    RuntimeVersion,
    #[error("Fetching Metadata failed. Are you connected to the correct endpoint?")]
    MetadataFetch,
    #[error("Operation needs a signer to be set in the api")]
    NoSigner,
    #[error("WebSocket Error: {0}")]
    WebSocket(#[from] JsonRpcWsError),
    #[error("RpcClient error: {0}")]
    RpcClient(String),
    #[error("ChannelReceiveError, sender is disconnected: {0}")]
    Disconnected(#[from] sp_std::sync::mpsc::RecvError),
    #[error("Metadata Error: {0}")]
    Metadata(#[from] metadata::MetadataError),
    #[error("Error decoding storage value: {0}")]
    StorageValueDecode(#[from] extrinsic::codec::Error),
    #[error("Received invalid hex string: {0}")]
    InvalidHexString(#[from] hex::FromHexError),
    #[error("Error deserializing with serde: {0}")]
    Deserializing(#[from] serde_json::Error),
    #[error("UnsupportedXtStatus Error: Can only wait for finalized, in block, broadcast and ready. Waited for: {0:?}")]
    UnsupportedXtStatus(XtStatus),
    #[error("Error converting NumberOrHex to Balance")]
    TryFromIntError,
    #[error("Error while sending extrinsic: {0}")]
    Extrinsic(String),
    #[error("Custom error: {0}")]
    Custom(String),
}

fn parse_pair(v: &Value) -> ApiResult<(StorageKey, StorageKey)> {
    let v = v
        .as_array()
        .ok_or_else(|| ApiClientError::RpcClient("key-value pair is malformed".to_owned()))?;
    if v.len() != 2 {
        return Err(ApiClientError::RpcClient(
            "key-value pair is malformed".to_owned(),
        ));
    }
    let key = hex::decode(
        v[0].as_str()
            .ok_or_else(|| ApiClientError::RpcClient("key-value pair has bad key".to_owned()))?
            .trim_start_matches("0x"),
    )?;
    let value = hex::decode(
        v[1].as_str()
            .ok_or_else(|| ApiClientError::RpcClient("key-value pair has bad value".to_owned()))?
            .trim_start_matches("0x"),
    )?;
    Ok((StorageKey(key), StorageKey(value)))
}
