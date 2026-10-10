use std::{path::PathBuf, str::FromStr, time::Duration};

use alloy::primitives::{Address, U256};
use anyhow::{anyhow, bail, Context, Result};
use ethereum_light_client::client_state::ClientState;
use ibc_eureka_solidity_types::ics26::ICS26_IBC_STORAGE_SLOT;
use proof_api_eth_to_stellar::{
    beacon::{to_json, Beacon, Step},
    soroban::SorobanRpc,
};
use sp1_ethereum_prover::{
    proving::{header_stdin, run, verifying_key, Mode},
    update_wire,
};
use stellar_xdr::{ContractId, Hash, ScAddress};

const ELF_PATH: &str = "programs/sp1-ethereum-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-update-client";

struct Settings {
    beacon_api_url: String,
    eth_rpc_url: String,
    soroban_rpc_url: String,
    soroban_router: String,
    client_id: String,
    ics26_address: String,
    chain_id: u64,
    elf_path: PathBuf,
}

impl Settings {
    fn from_environment() -> Result<Self> {
        let read =
            |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());

        let elf_path = std::env::var("SP1_ETHEREUM_ELF")
            .map(PathBuf::from)
            .unwrap_or_else(|_| repository_root().join(ELF_PATH));

        let chain_id = read("EVM_CHAIN_ID", "11155111")
            .parse()
            .context("EVM_CHAIN_ID is not a number")?;

        Ok(Self {
            beacon_api_url: read("BEACON_API_URL", "https://lodestar-sepolia.chainsafe.io"),
            eth_rpc_url: read("EVM_RPC_URL", "https://ethereum-sepolia-rpc.publicnode.com"),
            soroban_rpc_url: read("SOROBAN_RPC_URL", "https://soroban-testnet.stellar.org"),
            soroban_router: read(
                "SOROBAN_ROUTER",
                "CAWG2HO4YHCHBEVMNZDPBGHPRIJSJHFLGNWY27QMA53WZYPRXTA23KB4",
            ),
            client_id: read("STELLAR_EVM_CLIENT_ID", "sepolia-0"),
            ics26_address: read(
                "EVM_ICS26_ROUTER",
                "0x3f09B9D0E8453C91FBA256F8e2A39A7200D7604B",
            ),
            chain_id,
            elf_path,
        })
    }
}

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn");
    }

    let settings = Settings::from_environment()?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let (beacon, client_state, steps) = runtime.block_on(gather(&settings))?;

    drop(runtime);

    let elf = std::fs::read(&settings.elf_path)
        .with_context(|| format!("reading {}", settings.elf_path.display()))?;

    println!("update-client vkey 0x{}", hex::encode(verifying_key(&elf)?));

    execute(&beacon, &client_state, &steps, &elf)
}

async fn gather(settings: &Settings) -> Result<(Beacon, ClientState, Vec<Step>)> {
    let router = contract_id(&settings.soroban_router)?;
    let soroban = SorobanRpc::new(settings.soroban_rpc_url.clone());
    let beacon = Beacon::connect(
        &settings.beacon_api_url,
        &settings.eth_rpc_url,
        Duration::from_secs(60),
    )
    .await?;
    let client_id = settings.client_id.as_str();

    let light_client = soroban.light_client(router, client_id).await?;
    let trusted_slot = soroban.latest_slot(light_client, client_id).await?;
    let stored_hash = soroban
        .consensus_hash(light_client, client_id, trusted_slot)
        .await?;
    let stored_commitment = soroban
        .client_state_commitment(light_client, client_id)
        .await?;

    println!(
        "stored latest slot {trusted_slot} (period {})",
        beacon.period_of(trusted_slot)
    );

    let (trusted, trusted_block) = beacon.rebuild(trusted_slot, stored_hash).await?;

    println!(
        "rebuilt the trusted state; next committee known: {}",
        trusted.next_sync_committee.is_some()
    );

    let client_state = beacon.client_state(
        settings.chain_id,
        Address::from_str(&settings.ics26_address)?,
        U256::from_be_slice(&ICS26_IBC_STORAGE_SLOT),
        trusted_slot,
        trusted_block,
    );

    if update_wire::client_state_commitment(&client_state) != stored_commitment {
        bail!("the client state built here does not match the commitment the Soroban client pins");
    }

    println!("the client state matches the pinned commitment");

    let target = beacon.finalized_covering(0).await?;

    println!(
        "target finalized slot {} (period {}), signature slot {}",
        target.finalized_header.beacon().slot,
        beacon.period_of(target.finalized_header.beacon().slot),
        target.signature_slot
    );

    let steps = beacon.steps(&client_state, trusted, target).await?;

    println!(
        "{} steps built; update_consensus_state accepted each on the host",
        steps.len()
    );

    Ok((beacon, client_state, steps))
}

fn execute(beacon: &Beacon, client_state: &ClientState, steps: &[Step], elf: &[u8]) -> Result<()> {
    let mut state = client_state.clone();

    for (index, step) in steps.iter().enumerate() {
        state.latest_slot = step.trusted.slot;

        let stdin = header_stdin(
            &to_json(&state)?,
            &to_json(&step.trusted)?,
            &to_json(&step.header)?,
            step.timestamp,
        );

        let outcome = run(elf, stdin, Mode::Execute, "update-client")?;
        let expected = update_wire::output(&state, &step.trusted, &step.updated);

        if outcome.public_values() != expected.as_slice() {
            bail!(
                "step {}: the guest committed something other than the host computed",
                index + 1
            );
        }

        println!(
            "step {}: slot {} (period {}) -> {} (period {}), next committee known: {}, executed and matched",
            index + 1,
            step.trusted.slot,
            beacon.period_of(step.trusted.slot),
            step.updated.slot,
            beacon.period_of(step.updated.slot),
            step.updated.next_sync_committee.is_some(),
        );
    }

    println!("the client can be walked to the finalized slot");

    Ok(())
}

fn contract_id(strkey: &str) -> Result<[u8; 32]> {
    match ScAddress::from_str(strkey).map_err(|error| anyhow!("{strkey}: {error}"))? {
        ScAddress::Contract(ContractId(Hash(id))) => Ok(id),
        _ => bail!("{strkey} is not a contract address"),
    }
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("the manifest directory is four levels below the repository root")
        .to_path_buf()
}
