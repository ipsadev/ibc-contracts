use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use ipsa_scp_exporter::{
    fixture::Fixture as Exported,
    network::Network,
    steps::{self, HeaderInput, QuorumConfig},
};
use serde_json::Value;
use sp1_stellar_prover::Fixture;
use stellar_xdr::{LedgerCloseMeta, Limits, ReadXdr, WriteXdr};

const POLL: Duration = Duration::from_secs(2);

pub struct StellarApi {
    url: String,
    http: reqwest::Client,
    network: Network,
    timeout: Duration,
}

impl StellarApi {
    pub fn new(url: &str, network: &str, timeout: Duration) -> Result<Self> {
        let network = Network::parse(network)
            .map_err(|error| anyhow!("unknown Stellar network {network}: {error}"))?;

        Ok(Self {
            url: url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
            network,
            timeout,
        })
    }

    pub fn network_label(&self) -> &'static str {
        self.network.label()
    }

    async fn get(&self, path: &str) -> Result<Option<Value>> {
        let url = format!("{}{path}", self.url);
        let response = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("reading {url}"))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        Ok(Some(
            response
                .json()
                .await
                .with_context(|| format!("{url} is not JSON"))?,
        ))
    }

    async fn require(&self, path: &str) -> Result<Value> {
        self.get(path)
            .await?
            .ok_or_else(|| anyhow!("{}{path} has nothing for that ledger yet", self.url))
    }

    pub async fn current_ledger(&self) -> Result<u32> {
        let latest = self.require("/ledger/latest").await?;

        u32::try_from(number(&latest, "sequence")?)
            .map_err(|_| anyhow!("the latest ledger does not fit a u32"))
    }

    pub async fn wait_for(&self, ledger: u32) -> Result<()> {
        let started = Instant::now();

        loop {
            if self
                .get(&format!("/archive/scp/{}", ledger + 1))
                .await?
                .is_some()
            {
                tracing::info!(
                    ledger,
                    waited = started.elapsed().as_secs(),
                    "the api holds the ledger's consensus messages and its successor's"
                );

                return Ok(());
            }

            if started.elapsed() > self.timeout {
                bail!(
                    "the api had no consensus messages for ledger {} within {}s",
                    ledger + 1,
                    self.timeout.as_secs()
                );
            }

            tokio::time::sleep(POLL).await;
        }
    }

    pub async fn fixture(&self, ledger: u32) -> Result<Fixture> {
        let json = serde_json::to_string(&self.exported(ledger).await?)
            .context("serialising the ledger evidence")?;

        Fixture::from_json(&json).context("reading the ledger evidence as a prover fixture")
    }

    pub async fn exported(&self, ledger: u32) -> Result<Exported> {
        let following = ledger + 1;

        let scp = self.require(&format!("/archive/scp/{ledger}")).await?;
        let next_scp = self.require(&format!("/archive/scp/{following}")).await?;
        let header = self
            .require(&format!("/archive/ledger-header/{ledger}"))
            .await?;
        let previous = self
            .get(&format!("/archive/ledger-header/{}", ledger - 1))
            .await?;
        let closed = self.require(&format!("/ledger/{ledger}")).await?;
        let next_tx_set = hex_field(
            &self
                .require(&format!("/archive/tx-set/{following}"))
                .await?,
            "generalized_tx_set_xdr",
        )?;

        let input = HeaderInput {
            slot: ledger as u64,
            network_id: self.network.id(),
            ledger_header_xdr: hex_field(&header, "header_xdr")?,
            envelopes_xdr: hex_list(&scp, "envelopes_xdr")?,
            quorum_sets_xdr: hex_list(&scp, "quorum_sets_xdr")?,
            recorded_ledger_hash: Some(hash32(&header, "hash")?),
            result_pairs_xdr: result_pairs(&hex_field(&closed, "metadata_xdr")?)?,
            trusted_previous_hash: previous.map(|value| hash32(&value, "hash")).transpose()?,
        };

        let config = QuorumConfig::from_xdr(
            input
                .quorum_sets_xdr
                .first()
                .ok_or_else(|| anyhow!("ledger {ledger} carries no quorum set"))?,
        )
        .map_err(|error| anyhow!("the quorum set of ledger {ledger}: {error}"))?;

        let verified = steps::verify_header(&input, &config)
            .map_err(|error| anyhow!("verifying ledger {ledger}: {error}"))?;
        steps::verify_result_set(&input.result_pairs_xdr, &verified.tx_set_result_hash)
            .map_err(|error| anyhow!("the results of ledger {ledger}: {error}"))?;

        let next_envelopes = hex_list(&next_scp, "envelopes_xdr")?;
        let next_value = externalized_value(
            following,
            self.network.id(),
            &next_envelopes,
            &hex_list(&next_scp, "quorum_sets_xdr")?,
        )
        .map_err(|error| anyhow!("ledger {following}'s consensus: {error}"))?;
        steps::verify_next_slot_binding(&next_value, &next_tx_set, &verified.ledger_hash)
            .map_err(|error| anyhow!("binding ledger {ledger} to the next slot: {error}"))?;

        Ok(Exported {
            network: self.network.label().to_string(),
            ledger_seq: input.slot,
            network_id: hex::encode(input.network_id),
            ledger_hash: hex::encode(verified.ledger_hash),
            close_time: verified.close_time,
            tx_set_result_hash: hex::encode(verified.tx_set_result_hash),
            bucket_list_hash: hex::encode(verified.bucket_list_hash),
            ledger_header_xdr: hex::encode(&input.ledger_header_xdr),
            quorum_sets_xdr: input.quorum_sets_xdr.iter().map(hex::encode).collect(),
            envelopes_xdr: input.envelopes_xdr.iter().map(hex::encode).collect(),
            next_envelopes_xdr: next_envelopes.iter().map(hex::encode).collect(),
            next_tx_set_xdr: hex::encode(&next_tx_set),
            result_pairs_xdr: input.result_pairs_xdr.iter().map(hex::encode).collect(),
        })
    }
}

fn externalized_value(
    slot: u32,
    network_id: [u8; 32],
    envelopes_xdr: &[Vec<u8>],
    quorum_sets_xdr: &[Vec<u8>],
) -> Result<Vec<u8>> {
    let envelopes = envelopes_xdr
        .iter()
        .map(|raw| steps::validate_externalize(raw, slot as u64))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| anyhow!("{error}"))?;

    steps::reject_duplicate_signers(&envelopes).map_err(|error| anyhow!("{error}"))?;

    for envelope in &envelopes {
        steps::verify_envelope_signature(envelope, &network_id)
            .map_err(|error| anyhow!("{error}"))?;
    }

    let preimages =
        steps::index_quorum_sets(quorum_sets_xdr).map_err(|error| anyhow!("{error}"))?;
    let signers = steps::verify_quorum_set_hashes(&envelopes, &preimages)
        .map_err(|error| anyhow!("{error}"))?;
    let config = QuorumConfig::from_xdr(
        quorum_sets_xdr
            .first()
            .ok_or_else(|| anyhow!("no quorum set"))?,
    )
    .map_err(|error| anyhow!("{error}"))?;

    steps::verify_externalized_quorum(&config, &signers, &envelopes)
        .map(|externalized| externalized.value)
        .map_err(|error| anyhow!("{error}"))
}

pub fn result_pairs(metadata_xdr: &[u8]) -> Result<Vec<Vec<u8>>> {
    let meta = LedgerCloseMeta::from_xdr(metadata_xdr, Limits::none())
        .context("decoding the ledger's close metadata")?;

    let pairs = match meta {
        LedgerCloseMeta::V0(meta) => meta
            .tx_processing
            .iter()
            .map(|processed| processed.result.to_xdr(Limits::none()))
            .collect::<std::result::Result<Vec<_>, _>>(),
        LedgerCloseMeta::V1(meta) => meta
            .tx_processing
            .iter()
            .map(|processed| processed.result.to_xdr(Limits::none()))
            .collect::<std::result::Result<Vec<_>, _>>(),
        LedgerCloseMeta::V2(meta) => meta
            .tx_processing
            .iter()
            .map(|processed| processed.result.to_xdr(Limits::none()))
            .collect::<std::result::Result<Vec<_>, _>>(),
    };

    pairs.context("encoding the transaction results")
}

fn number(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .and_then(|field| {
            field
                .as_u64()
                .or_else(|| field.as_str().and_then(|raw| raw.parse().ok()))
        })
        .ok_or_else(|| anyhow!("no number {key} in the api's answer"))
}

fn hex_field(value: &Value, key: &str) -> Result<Vec<u8>> {
    let raw = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("no {key} in the api's answer"))?;

    hex::decode(raw).with_context(|| format!("{key} is not hex"))
}

fn hex_list(value: &Value, key: &str) -> Result<Vec<Vec<u8>>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("no {key} in the api's answer"))?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| anyhow!("{key} holds a value that is not a string"))
                .and_then(|raw| hex::decode(raw).with_context(|| format!("{key} is not hex")))
        })
        .collect()
}

fn hash32(value: &Value, key: &str) -> Result<[u8; 32]> {
    hex_field(value, key)?
        .try_into()
        .map_err(|_| anyhow!("{key} is not 32 bytes"))
}
