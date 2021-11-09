/*
   Copyright 2019 Supercomputing Systems AG

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.

*/

use crate::Hash;
use jsonrpsee_ws_client::types::v2::params::JsonRpcParams;
use jsonrpsee_ws_client::types::JsonValue;
use serde_json::{json, to_value};
use sp_core::storage::StorageKey;

pub const REQUEST_TRANSFER: u32 = 3;

pub fn num_params(number: Option<u32>) -> JsonRpcParams<'static> {
    match number {
        Some(n) => JsonRpcParams::Array(vec![json!(n)]),
        None => JsonRpcParams::Array(vec![JsonValue::Null]),
    }
}

pub fn hash_params(hash: Option<Hash>) -> JsonRpcParams<'static> {
    match hash {
        Some(h) => JsonRpcParams::Array(vec![JsonValue::String(
            "0x".to_owned() + hex::encode(h.as_bytes()).as_str(),
        )]),
        None => JsonRpcParams::Array(vec![JsonValue::Null]),
    }
}

pub fn null_params() -> JsonRpcParams<'static> {
    JsonRpcParams::Array(vec![JsonValue::Null])
}

pub fn state_get_storage<'a>(key: StorageKey, at_block: Option<Hash>) -> JsonRpcParams<'a> {
    JsonRpcParams::Array(vec![to_value(key).unwrap(), to_value(at_block).unwrap()])
}

pub fn state_get_child_storage<'a>(
    child: StorageKey,
    key: StorageKey,
    at_block: Option<Hash>,
) -> JsonRpcParams<'a> {
    JsonRpcParams::Array(vec![
        to_value(child).unwrap(),
        to_value(key).unwrap(),
        to_value(at_block).unwrap(),
    ])
}

pub fn state_query_storage_at<'a>(
    keys: Vec<StorageKey>,
    at_block: Option<Hash>,
) -> JsonRpcParams<'a> {
    JsonRpcParams::Array(vec![to_value(keys).unwrap(), to_value(at_block).unwrap()])
}

pub fn state_get_keys_paged<'a>(
    key: StorageKey,
    count: u32,
    start_key: Option<StorageKey>,
    at_block: Option<Hash>,
) -> JsonRpcParams<'a> {
    JsonRpcParams::Array(vec![
        to_value(key).unwrap(),
        to_value(count).unwrap(),
        to_value(start_key).unwrap(),
        to_value(at_block).unwrap(),
    ])
}
