use anyhow::{anyhow, bail, Context, Result};
use ibc_eureka_solidity_types::ics26::IICS26RouterMsgs::{Packet, Payload};
use serde_json::{json, Value};
use stellar_xdr::{
    ContractEvent, ContractEventBody, LedgerEntry, LedgerEntryChange, LedgerEntryChanges, Limits,
    ReadXdr, ScMap, ScVal, TransactionMeta,
};

pub struct SorobanRpc {
    http: reqwest::Client,
    url: String,
}

pub struct Transaction {
    pub hash: String,
    pub ledger: u32,
    pub application_order: u32,
    pub meta: TransactionMeta,
}

pub struct Executed {
    pub events: Vec<ContractEvent>,
    pub return_value: ScVal,
    pub written: Vec<LedgerEntry>,
}

impl SorobanRpc {
    pub fn new(url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            url,
        }
    }

    async fn call(&self, method: &str, parameters: Value) -> Result<Value> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": parameters });

        let answer: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("{method} to {}", self.url))?
            .json()
            .await
            .with_context(|| format!("{method} answered with something that is not JSON"))?;

        if let Some(error) = answer.get("error") {
            bail!("{method}: {error}");
        }

        answer
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow!("{method} answered without a result"))
    }

    pub async fn transaction(&self, hash: &str) -> Result<Transaction> {
        let result = self.call("getTransaction", json!({ "hash": hash })).await?;

        transaction_from_result(hash, &result)
    }
}

pub fn transaction_from_result(hash: &str, result: &Value) -> Result<Transaction> {
    let status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if status != "SUCCESS" {
        bail!("Stellar transaction {hash} is {status}, not SUCCESS");
    }

    let ledger = number(result, "ledger")?;
    let application_order = number(result, "applicationOrder")?;
    let meta_xdr = result
        .get("resultMetaXdr")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("transaction {hash} carries no resultMetaXdr"))?;

    let meta = TransactionMeta::from_xdr_base64(meta_xdr, Limits::none())
        .context("decoding the transaction meta")?;

    Ok(Transaction {
        hash: hash.to_string(),
        ledger,
        application_order,
        meta,
    })
}

fn number(result: &Value, field: &str) -> Result<u32> {
    let value = result
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("the transaction carries no {field}"))?;

    u32::try_from(value).map_err(|_| anyhow!("{field} {value} does not fit a u32"))
}

pub fn executed(meta: &TransactionMeta) -> Result<Executed> {
    match meta {
        TransactionMeta::V4(version_four) => {
            let operation = version_four
                .operations
                .first()
                .ok_or_else(|| anyhow!("the transaction meta has no operations"))?;
            let return_value = version_four
                .soroban_meta
                .as_ref()
                .and_then(|soroban| soroban.return_value.clone())
                .ok_or_else(|| anyhow!("the transaction meta has no Soroban return value"))?;

            Ok(Executed {
                events: operation.events.to_vec(),
                return_value,
                written: written_entries(&operation.changes),
            })
        }
        TransactionMeta::V3(version_three) => {
            let soroban = version_three
                .soroban_meta
                .as_ref()
                .ok_or_else(|| anyhow!("the transaction meta has no Soroban section"))?;
            let operation = version_three
                .operations
                .first()
                .ok_or_else(|| anyhow!("the transaction meta has no operations"))?;

            Ok(Executed {
                events: soroban.events.to_vec(),
                return_value: soroban.return_value.clone(),
                written: written_entries(&operation.changes),
            })
        }
        _ => bail!("transaction meta of this version is not one this module reads"),
    }
}

fn written_entries(changes: &LedgerEntryChanges) -> Vec<LedgerEntry> {
    changes
        .iter()
        .filter_map(|change| match change {
            LedgerEntryChange::Created(entry)
            | LedgerEntryChange::Updated(entry)
            | LedgerEntryChange::Restored(entry) => Some(entry.clone()),
            _ => None,
        })
        .collect()
}

pub struct RouterEvent {
    pub name: String,
    pub data: ScVal,
}

pub fn router_events(executed: &Executed, router: [u8; 32]) -> Vec<RouterEvent> {
    executed
        .events
        .iter()
        .filter(|event| {
            event
                .contract_id
                .as_ref()
                .is_some_and(|contract| contract.0 .0 == router)
        })
        .filter_map(|event| {
            let ContractEventBody::V0(body) = &event.body;
            let name = match body.topics.first() {
                Some(ScVal::Symbol(symbol)) => symbol.to_utf8_string_lossy(),
                _ => return None,
            };

            Some(RouterEvent {
                name,
                data: body.data.clone(),
            })
        })
        .collect()
}

pub fn field<'a>(map: &'a ScVal, name: &str) -> Result<&'a ScVal> {
    let ScVal::Map(Some(ScMap(entries))) = map else {
        bail!("expected a map holding {name}");
    };

    entries
        .iter()
        .find(|entry| matches!(&entry.key, ScVal::Symbol(symbol) if symbol.to_utf8_string_lossy() == name))
        .map(|entry| &entry.val)
        .ok_or_else(|| anyhow!("the map has no field {name}"))
}

pub fn text(value: &ScVal) -> Result<String> {
    match value {
        ScVal::String(string) => Ok(string.to_utf8_string_lossy()),
        _ => bail!("expected a string"),
    }
}

pub fn unsigned(value: &ScVal) -> Result<u64> {
    match value {
        ScVal::U64(number) => Ok(*number),
        _ => bail!("expected a u64"),
    }
}

pub fn bytes(value: &ScVal) -> Result<Vec<u8>> {
    match value {
        ScVal::Bytes(raw) => Ok(raw.to_vec()),
        _ => bail!("expected bytes"),
    }
}

pub fn packet(value: &ScVal) -> Result<Packet> {
    let ScVal::Vec(Some(payload_values)) = field(value, "payloads")? else {
        bail!("the packet's payloads are not a vector");
    };

    let payloads = payload_values
        .iter()
        .map(|payload| {
            Ok(Payload {
                sourcePort: text(field(payload, "source_port")?)?,
                destPort: text(field(payload, "dest_port")?)?,
                version: text(field(payload, "version")?)?,
                encoding: text(field(payload, "encoding")?)?,
                value: bytes(field(payload, "value")?)?.into(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Packet {
        sequence: unsigned(field(value, "sequence")?)?,
        sourceClient: text(field(value, "source_client")?)?,
        destClient: text(field(value, "dest_client")?)?,
        timeoutTimestamp: unsigned(field(value, "timeout_timestamp")?)?,
        payloads,
    })
}
