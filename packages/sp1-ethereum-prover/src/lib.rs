#[path = "../../../programs/sp1-ethereum-programs/update-client/src/wire.rs"]
pub mod update_wire;

#[path = "../../../programs/sp1-ethereum-programs/membership/src/wire.rs"]
pub mod membership_wire;

pub mod proving;

use alloy_primitives::{keccak256, Bytes, B256};
use anyhow::{anyhow, bail, Result};
use ethereum_types::consensus::spec::Spec;

pub async fn fetch_execution_header(rpc: &str, block_hash: B256) -> Result<Bytes> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_getBlockByHash",
        "params": [block_hash, false],
    });

    let mut response: serde_json::Value = reqwest::Client::new()
        .post(rpc)
        .json(&request)
        .send()
        .await?
        .json()
        .await?;

    let block = response
        .get_mut("result")
        .map(serde_json::Value::take)
        .filter(|block| !block.is_null())
        .ok_or_else(|| anyhow!("{rpc} has no execution block {block_hash}: {response}"))?;

    let header: alloy_rpc_types_eth::Header = serde_json::from_value(block)?;
    let encoded = Bytes::from(alloy_rlp::encode(&header.inner));
    let found = keccak256(&encoded);

    if found != block_hash {
        bail!(
            "execution block {block_hash} re-encodes to a header hashing to {found}; this alloy \
             version does not know every field of the chain's execution header"
        );
    }

    Ok(encoded)
}

pub async fn fetch_spec(beacon: &str) -> Result<Spec> {
    let raw: serde_json::Value = reqwest::get(format!("{beacon}/eth/v1/config/spec"))
        .await?
        .json()
        .await?;

    let mut data = raw
        .get("data")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .ok_or_else(|| anyhow!("the spec response carries no data object"))?;

    data.entry("GENESIS_SLOT".to_string())
        .or_insert_with(|| serde_json::Value::String("0".to_string()));

    Ok(serde_json::from_value(serde_json::Value::Object(data))?)
}
