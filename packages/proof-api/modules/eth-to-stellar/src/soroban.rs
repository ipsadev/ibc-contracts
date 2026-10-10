use anyhow::{anyhow, bail, Context, Result};
use ibc_eureka_solidity_types::ics26::IICS26RouterMsgs::{Packet, Payload};
use serde_json::{json, Value};
use stellar_xdr::{
    ContractDataDurability, ContractId, Hash, HostFunction, InvokeContractArgs, LedgerEntryData,
    LedgerKey, LedgerKeyContractData, Limits, ReadXdr, ScAddress, ScBytes, ScMap, ScMapEntry,
    ScString, ScSymbol, ScVal, ScVec, WriteXdr,
};

pub struct SorobanRpc {
    http: reqwest::Client,
    url: String,
}

fn symbol(name: &str) -> Result<ScVal> {
    Ok(ScVal::Symbol(ScSymbol(
        name.try_into()
            .map_err(|_| anyhow!("{name} is not a Soroban symbol"))?,
    )))
}

fn string(text: &str) -> Result<ScVal> {
    Ok(ScVal::String(ScString(text.try_into().map_err(|_| {
        anyhow!("{text} is too long for a Soroban string")
    })?)))
}

fn bytes(raw: &[u8]) -> Result<ScVal> {
    Ok(ScVal::Bytes(ScBytes(raw.to_vec().try_into().map_err(
        |_| anyhow!("{} bytes do not fit Soroban bytes", raw.len()),
    )?)))
}

fn vector(items: Vec<ScVal>) -> Result<ScVal> {
    Ok(ScVal::Vec(Some(ScVec(items.try_into().map_err(|_| {
        anyhow!("too many items for a Soroban vector")
    })?))))
}

fn map(entries: Vec<(&str, ScVal)>) -> Result<ScVal> {
    let mut sorted = entries;

    sorted.sort_by(|left, right| left.0.cmp(right.0));

    let entries = sorted
        .into_iter()
        .map(|(key, val)| {
            Ok(ScMapEntry {
                key: symbol(key)?,
                val,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ScVal::Map(Some(ScMap(entries.try_into().map_err(
        |_| anyhow!("too many entries for a Soroban map"),
    )?))))
}

fn contract(id: [u8; 32]) -> ScAddress {
    ScAddress::Contract(ContractId(Hash(id)))
}

fn payload_value(payload: &Payload) -> Result<ScVal> {
    map(vec![
        ("source_port", string(&payload.sourcePort)?),
        ("dest_port", string(&payload.destPort)?),
        ("version", string(&payload.version)?),
        ("encoding", string(&payload.encoding)?),
        ("value", bytes(&payload.value)?),
    ])
}

pub fn packet_value(packet: &Packet) -> Result<ScVal> {
    let payloads = packet
        .payloads
        .iter()
        .map(payload_value)
        .collect::<Result<Vec<_>>>()?;

    map(vec![
        ("sequence", ScVal::U64(packet.sequence)),
        ("source_client", string(&packet.sourceClient)?),
        ("dest_client", string(&packet.destClient)?),
        ("timeout_timestamp", ScVal::U64(packet.timeoutTimestamp)),
        ("payloads", vector(payloads)?),
    ])
}

pub fn invoke(router: [u8; 32], function: &str, arguments: Vec<ScVal>) -> Result<HostFunction> {
    Ok(HostFunction::InvokeContract(InvokeContractArgs {
        contract_address: contract(router),
        function_name: ScSymbol(
            function
                .try_into()
                .map_err(|_| anyhow!("{function} is not a Soroban symbol"))?,
        ),
        args: arguments
            .try_into()
            .map_err(|_| anyhow!("too many arguments"))?,
    }))
}

pub fn update_client(router: [u8; 32], client_id: &str, message: &[u8]) -> Result<HostFunction> {
    invoke(
        router,
        "update_client",
        vec![string(client_id)?, bytes(message)?],
    )
}

pub fn recv_packet(
    router: [u8; 32],
    packet: &Packet,
    proof: &[u8],
    height: u64,
) -> Result<HostFunction> {
    invoke(
        router,
        "recv_packet",
        vec![packet_value(packet)?, bytes(proof)?, ScVal::U64(height)],
    )
}

pub fn acknowledge_packet(
    router: [u8; 32],
    packet: &Packet,
    acknowledgement: &[u8],
    proof: &[u8],
    height: u64,
) -> Result<HostFunction> {
    invoke(
        router,
        "acknowledge_packet",
        vec![
            packet_value(packet)?,
            vector(vec![bytes(acknowledgement)?])?,
            bytes(proof)?,
            ScVal::U64(height),
        ],
    )
}

pub fn create_client(
    router: [u8; 32],
    client_type: &str,
    client_state: &[u8],
    consensus_state: &[u8],
    height: u64,
) -> Result<HostFunction> {
    invoke(
        router,
        "create_client",
        vec![
            string(client_type)?,
            bytes(client_state)?,
            bytes(consensus_state)?,
            ScVal::U64(height),
        ],
    )
}

pub fn encode_host_functions(functions: &[HostFunction]) -> Result<Vec<u8>> {
    let count = u32::try_from(functions.len()).context("too many host functions")?;
    let mut encoded = count.to_be_bytes().to_vec();

    for function in functions {
        encoded.extend_from_slice(
            &function
                .to_xdr(Limits::none())
                .context("encoding a host function")?,
        );
    }

    Ok(encoded)
}

impl SorobanRpc {
    pub fn new(url: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            url,
        }
    }

    async fn entry(&self, contract_id: [u8; 32], key: ScVal) -> Result<Option<ScVal>> {
        let ledger_key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: contract(contract_id),
            key,
            durability: ContractDataDurability::Persistent,
        });
        let encoded = ledger_key
            .to_xdr_base64(Limits::none())
            .context("encoding a ledger key")?;
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLedgerEntries",
            "params": { "keys": [encoded] },
        });

        let answer: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("getLedgerEntries to {}", self.url))?
            .json()
            .await
            .context("getLedgerEntries answered with something that is not JSON")?;

        if let Some(error) = answer.get("error") {
            bail!("getLedgerEntries: {error}");
        }

        let Some(first) = answer
            .pointer("/result/entries")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
        else {
            return Ok(None);
        };

        let data = first
            .get("xdr")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("a ledger entry carries no xdr"))?;

        match LedgerEntryData::from_xdr_base64(data, Limits::none())
            .context("decoding a ledger entry")?
        {
            LedgerEntryData::ContractData(contract_data) => Ok(Some(contract_data.val)),
            _ => bail!("the ledger entry is not contract data"),
        }
    }

    pub async fn light_client(&self, router: [u8; 32], client_id: &str) -> Result<[u8; 32]> {
        let key = vector(vec![symbol("LightClient")?, string(client_id)?])?;

        match self.entry(router, key).await? {
            Some(ScVal::Address(ScAddress::Contract(ContractId(Hash(id))))) => Ok(id),
            Some(_) => bail!("the router's light client entry for {client_id} is not a contract"),
            None => bail!("the router has no client {client_id}"),
        }
    }

    pub async fn latest_slot(&self, light_client: [u8; 32], client_id: &str) -> Result<u64> {
        let key = vector(vec![symbol("LatestSlot")?, string(client_id)?])?;

        match self.entry(light_client, key).await? {
            Some(ScVal::U64(slot)) => Ok(slot),
            _ => bail!("the light client holds no latest slot for {client_id}"),
        }
    }

    pub async fn consensus_hash(
        &self,
        light_client: [u8; 32],
        client_id: &str,
        slot: u64,
    ) -> Result<[u8; 32]> {
        let key = vector(vec![
            symbol("ConsensusHash")?,
            string(client_id)?,
            ScVal::U64(slot),
        ])?;

        match self.entry(light_client, key).await? {
            Some(ScVal::Bytes(hash)) => hash
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("the stored consensus hash is not 32 bytes")),
            _ => bail!("the light client holds no consensus state for {client_id} at slot {slot}"),
        }
    }

    pub async fn client_state_commitment(
        &self,
        light_client: [u8; 32],
        client_id: &str,
    ) -> Result<[u8; 32]> {
        let key = vector(vec![symbol("ClientStateCommitment")?, string(client_id)?])?;

        match self.entry(light_client, key).await? {
            Some(ScVal::Bytes(hash)) => hash
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("the stored client state commitment is not 32 bytes")),
            _ => bail!("the light client holds no client state commitment for {client_id}"),
        }
    }

    pub async fn provable_entry_exists(&self, router: [u8; 32], path: &[u8]) -> Result<bool> {
        Ok(self.entry(router, bytes(path)?).await?.is_some())
    }
}
