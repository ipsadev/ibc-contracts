use std::{collections::HashMap, env};

use anyhow::{anyhow, bail, Context, Result};
use proof_api_history::{NetworkRequest, ProofEntry, ProofHistory};
use serde::{Deserialize, Serialize};
use sp1_sdk::{
    blocking::{NetworkProver, ProverClient},
    network::{
        proto::GetFilteredProofRequestsResponse, signer::NetworkSigner, NetworkClient, NetworkMode,
    },
};

const RPC_URL: &str = "https://rpc.mainnet.succinct.xyz";

const PAGE_SIZE: u32 = 100;

#[derive(Deserialize, Serialize)]
struct Listed {
    id: String,
    created_at: u64,
    vk_hash: String,
    fulfillment_status: i32,
    cycles: Option<u64>,
    deduction_amount: Option<String>,
    #[serde(default)]
    public_values_hash: Option<String>,
}

#[derive(Deserialize)]
struct Attribution {
    #[serde(rename = "r")]
    request_id: String,
    #[serde(rename = "m")]
    module: String,
    #[serde(rename = "s")]
    src_chain: String,
    #[serde(rename = "d")]
    dst_chain: String,
    #[serde(rename = "k")]
    kind: String,
    #[serde(rename = "t")]
    tx_hash: String,
    #[serde(rename = "q")]
    sequences: Vec<u64>,
    #[serde(rename = "n")]
    step: u32,
    #[serde(rename = "p")]
    program: String,
}

fn private_key() -> Result<String> {
    env::var("NETWORK_PRIVATE_KEY").context("NETWORK_PRIVATE_KEY is not set")
}

fn request_id(text: &str) -> Result<[u8; 32]> {
    hex::decode(text.trim_start_matches("0x"))
        .context("the request id is not hex")?
        .try_into()
        .map_err(|_| anyhow!("the request id is not 32 bytes"))
}

async fn list() -> Result<()> {
    for request in requests().await? {
        println!("{}", serde_json::to_string(&request)?);
    }

    Ok(())
}

async fn requests() -> Result<Vec<Listed>> {
    let signer = NetworkSigner::local(&private_key()?).map_err(|error| anyhow!("{error}"))?;
    let requester = signer.address().to_vec();
    let client = NetworkClient::new(signer, RPC_URL, NetworkMode::Mainnet);
    let mut all = Vec::new();

    for page in 1.. {
        let found = client
            .get_filtered_proof_requests(
                None,
                None,
                None,
                None,
                None,
                Some(requester.clone()),
                None,
                None,
                None,
                Some(PAGE_SIZE),
                Some(page),
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .context("listing the requester's proof requests")?;

        let listed: Vec<Listed> = match found {
            GetFilteredProofRequestsResponse::Auction(found) => found
                .requests
                .iter()
                .map(|request| Listed {
                    id: format!("0x{}", hex::encode(&request.request_id)),
                    created_at: request.created_at,
                    vk_hash: format!("0x{}", hex::encode(&request.vk_hash)),
                    fulfillment_status: request.fulfillment_status,
                    cycles: request.cycles,
                    deduction_amount: request.deduction_amount.clone(),
                    public_values_hash: request
                        .public_values_hash
                        .as_ref()
                        .map(|hash| format!("0x{}", hex::encode(hash))),
                })
                .collect(),
            GetFilteredProofRequestsResponse::Base(found) => found
                .requests
                .iter()
                .map(|request| Listed {
                    id: format!("0x{}", hex::encode(&request.request_id)),
                    created_at: request.created_at,
                    vk_hash: format!("0x{}", hex::encode(&request.vk_hash)),
                    fulfillment_status: request.fulfillment_status,
                    cycles: request.cycles,
                    deduction_amount: request.deduction_amount.clone(),
                    public_values_hash: request
                        .public_values_hash
                        .as_ref()
                        .map(|hash| format!("0x{}", hex::encode(hash))),
                })
                .collect(),
        };

        let last = listed.len() < PAGE_SIZE as usize;

        all.extend(listed);

        if last {
            break;
        }
    }

    Ok(all)
}

fn entry_for(prover: &NetworkProver, attribution: &Attribution) -> Result<ProofEntry> {
    let id = request_id(&attribution.request_id)?;
    let record = prover
        .get_proof_request(id.into())
        .context("reading the request from the network")?
        .ok_or_else(|| anyhow!("the network has no request {}", attribution.request_id))?;

    let fulfilled = record.fulfilled_at.is_some();
    let proof = if fulfilled {
        prover
            .get_proof_status(id.into())
            .ok()
            .and_then(|(_, proof)| proof)
    } else {
        None
    };

    let vkey: Option<[u8; 32]> = record.vk_hash.clone().try_into().ok();

    Ok(ProofEntry {
        module: attribution.module.clone(),
        src_chain: attribution.src_chain.clone(),
        dst_chain: attribution.dst_chain.clone(),
        kind: attribution.kind.clone(),
        tx_hash: attribution.tx_hash.clone(),
        sequences: attribution.sequences.clone(),
        step: attribution.step,
        program: attribution.program.clone(),
        prover: "network".to_string(),
        status: if fulfilled {
            "fulfilled"
        } else {
            "unfulfilled"
        }
        .to_string(),
        error: None,
        vkey,
        trusted_height: None,
        target_height: None,
        public_values: proof
            .as_ref()
            .map(|proof| proof.public_values.as_slice().to_vec()),
        proof: proof.as_ref().map(|proof| proof.bytes()),
        request: Some(NetworkRequest::from_sdk(id, Some(&record))),
    })
}

async fn insert() -> Result<()> {
    let known: Vec<Attribution> = serde_json::from_str(
        &env::var("PROOF_BACKFILL_JSON").context("PROOF_BACKFILL_JSON is not set")?,
    )
    .context("PROOF_BACKFILL_JSON is not a list of attributions")?;
    let mut known: HashMap<String, Attribution> = known
        .into_iter()
        .map(|attribution| (attribution.request_id.clone(), attribution))
        .collect();

    let listed = requests().await?;
    let attributions: Vec<Attribution> = listed
        .iter()
        .filter_map(|request| known.remove(&request.id))
        .collect();

    if !known.is_empty() {
        bail!(
            "{} attributed requests are not on the network: {}",
            known.len(),
            known.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    println!(
        "{} requests on the network, {} of them attributed and backfilled",
        listed.len(),
        attributions.len()
    );

    let history = ProofHistory::from_env();

    if !history.enabled() {
        bail!("PROOF_HISTORY_DB_URL is not set");
    }

    history.prepare().await?;

    let entries = tokio::task::spawn_blocking(move || {
        let prover = ProverClient::builder().network().build();

        attributions
            .iter()
            .map(|attribution| {
                entry_for(&prover, attribution)
                    .with_context(|| format!("request {}", attribution.request_id))
            })
            .collect::<Vec<_>>()
    })
    .await?;

    let mut written = 0;

    for entry in entries {
        match entry {
            Ok(entry) => {
                history.record(&entry).await;
                written += 1;
            }
            Err(error) => eprintln!("skipped: {error:#}"),
        }
    }

    println!("backfilled {written} proof requests");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    match env::args().nth(1).as_deref() {
        Some("list") => list().await,
        Some("insert") => insert().await,
        Some("proof") => {
            let id = request_id(&env::args().nth(2).context("usage: proof <request id>")?)?;
            let proof = tokio::task::spawn_blocking(move || {
                ProverClient::builder()
                    .network()
                    .build()
                    .get_proof_status(id.into())
            })
            .await??
            .1
            .context("the network holds no proof for that request")?;

            println!("{}", hex::encode(proof.bytes()));

            Ok(())
        }
        _ => bail!("usage: proof-history-backfill list | insert | proof <request id>"),
    }
}
