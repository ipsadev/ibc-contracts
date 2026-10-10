use std::{path::PathBuf, str::FromStr, time::Duration};

use alloy::primitives::{Address, U256};
use anyhow::{Context, Result};
use ibc_eureka_solidity_types::ics26::ICS26_IBC_STORAGE_SLOT;
use proof_api_eth_to_stellar::beacon::Beacon;
use sp1_ethereum_prover::{proving::verifying_key, update_wire};

const UPDATE_ELF: &str = "programs/sp1-ethereum-programs/update-client/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-update-client";
const MEMBERSHIP_ELF: &str = "programs/sp1-ethereum-programs/membership/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/sp1-ethereum-membership";

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn");
    }

    let read =
        |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());

    let beacon_api_url = read("BEACON_API_URL", "https://lodestar-sepolia.chainsafe.io");
    let eth_rpc_url = read("EVM_RPC_URL", "https://ethereum-sepolia-rpc.publicnode.com");
    let ics26_address = read(
        "EVM_ICS26_ROUTER",
        "0x3f09B9D0E8453C91FBA256F8e2A39A7200D7604B",
    );
    let chain_id: u64 = read("EVM_CHAIN_ID", "11155111")
        .parse()
        .context("EVM_CHAIN_ID is not a number")?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let (beacon, consensus_state, block_number) = runtime.block_on(async {
        let beacon =
            Beacon::connect(&beacon_api_url, &eth_rpc_url, Duration::from_secs(60)).await?;
        let (consensus_state, block_number) = beacon.genesis().await?;

        Ok::<_, anyhow::Error>((beacon, consensus_state, block_number))
    })?;

    drop(runtime);

    let client_state = beacon.client_state(
        chain_id,
        Address::from_str(&ics26_address)?,
        U256::from_be_slice(&ICS26_IBC_STORAGE_SLOT),
        consensus_state.slot,
        block_number,
    );

    let root = repository_root();
    let update_vkey = verifying_key(&std::fs::read(root.join(UPDATE_ELF))?)?;
    let membership_vkey = verifying_key(&std::fs::read(root.join(MEMBERSHIP_ELF))?)?;

    let commitment = update_wire::client_state_commitment(&client_state);
    let hash = update_wire::consensus_hash(&consensus_state);

    let mut client_state_bytes = update_vkey.to_vec();

    client_state_bytes.extend_from_slice(&membership_vkey);
    client_state_bytes.extend_from_slice(&commitment);

    let mut consensus_state_bytes = hash.to_vec();

    consensus_state_bytes.extend_from_slice(&consensus_state.timestamp.to_be_bytes());

    println!("slot              {}", consensus_state.slot);
    println!(
        "period            {}",
        beacon.period_of(consensus_state.slot)
    );
    println!("execution block   {block_number}");
    println!(
        "next committee    {}",
        consensus_state.next_sync_committee.is_some()
    );
    println!("update vkey       0x{}", hex::encode(update_vkey));
    println!("membership vkey   0x{}", hex::encode(membership_vkey));
    println!("commitment        0x{}", hex::encode(commitment));
    println!("consensus hash    0x{}", hex::encode(hash));
    println!("client_state      {}", hex::encode(&client_state_bytes));
    println!("consensus_state   {}", hex::encode(&consensus_state_bytes));

    Ok(())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("the manifest directory is four levels below the repository root")
        .to_path_buf()
}
