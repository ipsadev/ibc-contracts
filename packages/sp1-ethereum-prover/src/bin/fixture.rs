// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use ethereum_apis::beacon_api::client::BeaconApiClient;
use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use ethereum_light_client::execution::execution_data;
use ethereum_light_client::header::ActiveSyncCommittee;
use ethereum_light_client::header::Header;
use ethereum_types::consensus::bootstrap::LightClientBootstrap;
use ethereum_types::consensus::light_client_header::{
    LightClientFinalityUpdate, LightClientHeader, LightClientUpdate,
};

const DEFAULT_BEACON: &str = "https://ethereum-beacon-api.publicnode.com";
const DEFAULT_EXECUTION_RPC: &str = "https://ethereum-rpc.publicnode.com";
const DEFAULT_CHAIN_ID: u64 = 1;
const OUTPUT_DIR: &str = "packages/ethereum-fixtures";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let beacon = std::env::var("BEACON_API").unwrap_or_else(|_| DEFAULT_BEACON.to_string());

    // The chain id is not derivable from the beacon API, and it is part of what the client pins,
    // so a client built for the wrong one silently believes the wrong chain.
    let chain_id: u64 = std::env::var("ETHEREUM_CHAIN_ID")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_CHAIN_ID);

    let execution_rpc =
        std::env::var("EXECUTION_RPC").unwrap_or_else(|_| DEFAULT_EXECUTION_RPC.to_string());

    println!("beacon   {beacon}");
    println!("rpc      {execution_rpc}");
    println!("chain    {chain_id}");

    let client = BeaconApiClient::new(beacon.clone());

    let spec = sp1_ethereum_prover::fetch_spec(&beacon).await?;
    let genesis = client.genesis().await?.data;

    println!(
        "genesis  {} validators root",
        genesis.genesis_validators_root
    );
    println!(
        "spec     {}s per slot, {} slots per epoch, {} epochs per period",
        spec.seconds_per_slot, spec.slots_per_epoch, spec.epochs_per_sync_committee_period
    );

    // Some beacon nodes (Lodestar on Sepolia after gloas) serve a bootstrap only for the latest
    // finalized checkpoint. `ETHEREUM_TRUSTED_SLOT=finalized` takes that bootstrap first and then
    // waits for finality to move past it, so the update still advances the client.
    let early_bootstrap = if std::env::var("ETHEREUM_TRUSTED_SLOT").as_deref() == Ok("finalized") {
        Some(latest_finalized_bootstrap(&client).await?)
    } else {
        None
    };

    let finality = match &early_bootstrap {
        Some((_, slot, _)) => finality_past(&client, *slot).await?,
        None => client.finality_update().await?.data,
    };
    let period = spec.epochs_per_sync_committee_period * spec.slots_per_epoch;
    let committee_update = std::env::var("ETHEREUM_UPDATE").as_deref() == Ok("committee");

    let update: LightClientUpdate = if committee_update {
        let current_period = finality.finalized_header.beacon().slot / period;
        let update = client
            .light_client_updates(current_period, 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "the beacon node has no light client update for period {current_period}"
                )
            })?
            .data;

        if update.next_sync_committee.is_none() {
            anyhow::bail!("the period {current_period} update carries no next sync committee");
        }

        println!(
            "update   period {current_period}'s best update, carrying the next sync committee"
        );

        update
    } else {
        finality.into()
    };

    let finalized_slot = update.finalized_header.beacon().slot;
    let attested_slot = update.attested_header.beacon().slot;
    let signature_slot = update.signature_slot;

    let participants = update
        .sync_aggregate
        .sync_committee_bits
        .iter()
        .map(|byte| byte.count_ones() as u64)
        .sum::<u64>();

    println!("finalized slot {finalized_slot}, attested {attested_slot}");
    println!("signers  {participants} of {}", spec.sync_committee_size);

    // A finality update fetched mid-slot can carry almost no signatures: this endpoint has served
    // 15 of 512 one minute and 509 the next. Such a fixture is not wrong, it is just useless — the
    // guest would reject it and the time would go on debugging the guest.
    let floor = spec.sync_committee_size * 2 / 3;

    if participants < floor {
        anyhow::bail!(
            "only {participants} of {} signed, below the {floor} the light client needs. The \
             update was probably assembled mid-slot; wait a few seconds and run it again.",
            spec.sync_committee_size
        );
    }

    // The update has to advance the client, so the trusted state must come from an earlier slot.
    // It also has to be in the same sync committee period, because the committee carried in the
    // header is the one that signed the update — a trusted state from a different period would
    // hold a different committee and the update would not chain.
    let period_start = (finalized_slot / period) * period;
    let latest_allowed = if committee_update {
        period_start + period
    } else {
        finalized_slot
    };

    let (root, bootstrap_slot, bootstrap) = match early_bootstrap {
        Some(early) => early,
        None => {
            let (root, slot) = match std::env::var("ETHEREUM_TRUSTED_SLOT") {
                Ok(raw) => {
                    pinned_bootstrap(&client, raw.parse()?, latest_allowed, period_start).await?
                }
                Err(_) => bootstrap_before(&client, finalized_slot, period_start).await?,
            };
            let bootstrap = client.light_client_bootstrap(&root).await?.data;

            (root, slot, bootstrap)
        }
    };

    if bootstrap_slot < period_start {
        anyhow::bail!(
            "the trusted slot {bootstrap_slot} is in an earlier sync committee period than the \
             update; run again, finality crossed a period boundary while waiting"
        );
    }

    println!("bootstrap at slot {bootstrap_slot}, root {root}");

    let trusted_witness = witness_for(&execution_rpc, &bootstrap.header).await?;
    let update_witness = witness_for(&execution_rpc, &update.finalized_header).await?;
    let trusted_execution = execution_data(&bootstrap.header, trusted_witness.as_ref())?;

    let client_state = ClientState {
        chain_id,
        genesis_validators_root: genesis.genesis_validators_root,
        min_sync_committee_participants: spec.sync_committee_size * 2 / 3,
        sync_committee_size: spec.sync_committee_size,
        genesis_time: genesis.genesis_time,
        genesis_slot: spec.genesis_slot,
        fork_parameters: spec.to_fork_parameters(),
        seconds_per_slot: spec.seconds_per_slot,
        slots_per_epoch: spec.slots_per_epoch,
        epochs_per_sync_committee_period: spec.epochs_per_sync_committee_period,
        latest_slot: bootstrap.header.beacon().slot,
        latest_execution_block_number: trusted_execution.block_number,
        // Deployment configuration, not chain data: the beacon API cannot know which contract on
        // the execution layer holds the IBC commitments, or at which slot. Left at zero they look
        // plausible and every membership proof fails against the wrong account.
        ibc_contract_address: std::env::var("IBC_CONTRACT_ADDRESS")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or_default(),
        ibc_commitment_slot: std::env::var("IBC_COMMITMENT_SLOT")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or_default(),
        ..Default::default()
    };

    let consensus_state = ConsensusState {
        slot: bootstrap.header.beacon().slot,
        state_root: trusted_execution.state_root,
        timestamp: trusted_execution.timestamp,
        current_sync_committee: bootstrap
            .current_sync_committee
            .to_summarized_sync_committee(),
        next_sync_committee: None,
    };

    let header = Header {
        active_sync_committee: ActiveSyncCommittee::Current(bootstrap.current_sync_committee),
        consensus_update: update,
        trusted_slot: bootstrap.header.beacon().slot,
        execution_header: update_witness,
    };

    let root_dir = repository_root();
    let out = root_dir.join(OUTPUT_DIR);

    std::fs::create_dir_all(&out)?;

    write(&out.join("client-state.json"), &client_state)?;
    write(&out.join("consensus-state.json"), &consensus_state)?;
    write(&out.join("header.json"), &header)?;

    // The guest computes the current slot from this timestamp and refuses an update whose
    // signature slot is in the future. The signature slot is one past the attested slot, so a
    // timestamp derived from the attested slot is always one slot too early.
    let timestamp = genesis.genesis_time + signature_slot * spec.seconds_per_slot;

    std::fs::write(out.join("timestamp.txt"), format!("{timestamp}\n"))?;

    println!();
    println!("wrote {}", out.display());
    println!("  trusted slot   {}", bootstrap.header.beacon().slot);
    println!("  finalized slot {finalized_slot}");
    println!("  signature slot {signature_slot}");
    println!("  timestamp      {timestamp}");
    println!("  ibc contract   {}", client_state.ibc_contract_address);
    println!();
    println!("These are live-chain values, so they change on every run and cannot be pinned the");
    println!("way the Stellar fixtures are. A finalized header is only useful while the sync");
    println!("committee that signed it is still in the client's trusted window.");

    Ok(())
}

/// Walks back an epoch at a time for a finalized slot the client can start from.
///
/// Slots can be empty, so the root lookup fails on some of them and the next one back is tried.
/// Anything before `period_start` is refused rather than returned: a trusted state from an
/// earlier sync committee period holds a different committee, and the update would not chain.
async fn bootstrap_before(
    client: &BeaconApiClient,
    finalized_slot: u64,
    period_start: u64,
) -> anyhow::Result<(String, u64)> {
    let mut slot = finalized_slot.saturating_sub(32);

    while slot > period_start {
        if let Ok(root) = client.beacon_block_root(&slot.to_string()).await {
            return Ok((root, slot));
        }

        slot = slot.saturating_sub(32);
    }

    anyhow::bail!(
        "no finalized slot between {period_start} and {finalized_slot} has a block to bootstrap \
         from. The chain has probably just entered a new sync committee period; wait a few \
         minutes and run it again."
    )
}

async fn latest_finalized_bootstrap(
    client: &BeaconApiClient,
) -> anyhow::Result<(String, u64, LightClientBootstrap)> {
    let slot = client
        .finality_update()
        .await?
        .data
        .finalized_header
        .beacon()
        .slot;
    let root = client.beacon_block_root(&slot.to_string()).await?;
    let bootstrap = client.light_client_bootstrap(&root).await?.data;

    println!("bootstrap at the latest finalized checkpoint, slot {slot}");

    Ok((root, slot, bootstrap))
}

async fn finality_past(
    client: &BeaconApiClient,
    trusted_slot: u64,
) -> anyhow::Result<LightClientFinalityUpdate> {
    loop {
        let finality = client.finality_update().await?.data;

        if finality.finalized_header.beacon().slot > trusted_slot {
            return Ok(finality);
        }

        println!("waiting for finality to pass slot {trusted_slot}");

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

async fn pinned_bootstrap(
    client: &BeaconApiClient,
    slot: u64,
    finalized_slot: u64,
    period_start: u64,
) -> anyhow::Result<(String, u64)> {
    if slot < period_start || slot >= finalized_slot {
        anyhow::bail!(
            "ETHEREUM_TRUSTED_SLOT {slot} is outside the current sync committee period \
             [{period_start}, {finalized_slot}). An update cannot chain onto a trusted state \
             from another period; the client needs a period-crossing update first."
        );
    }

    let root = client
        .beacon_block_root(&slot.to_string())
        .await
        .map_err(|error| anyhow::anyhow!("slot {slot} has no block to bootstrap from: {error}"))?;

    println!("bootstrap pinned by ETHEREUM_TRUSTED_SLOT");

    Ok((root, slot))
}

async fn witness_for(
    rpc: &str,
    header: &LightClientHeader,
) -> anyhow::Result<Option<alloy_primitives::Bytes>> {
    match header {
        LightClientHeader::Electra(_) => Ok(None),
        LightClientHeader::Gloas(gloas) => Ok(Some(
            sp1_ethereum_prover::fetch_execution_header(rpc, gloas.execution_block_hash).await?,
        )),
    }
}

fn write<T: serde::Serialize>(path: &PathBuf, value: &T) -> anyhow::Result<()> {
    let mut json = serde_json::to_string_pretty(value)?;

    json.push('\n');

    std::fs::write(path, json)?;

    Ok(())
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the manifest directory has no grandparent")
        .to_path_buf()
}
