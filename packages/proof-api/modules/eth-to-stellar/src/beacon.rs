use std::time::{Duration, Instant};

use alloy::primitives::{Address, B256, U256};
use anyhow::{anyhow, bail, Context, Result};
use ethereum_apis::beacon_api::client::BeaconApiClient;
use ethereum_light_client::{
    client_state::ClientState,
    consensus_state::ConsensusState,
    execution::{execution_data, execution_data_for_block_hash, ExecutionData},
    header::{ActiveSyncCommittee, Header},
    update::update_consensus_state,
};
use ethereum_types::consensus::{
    bootstrap::LightClientBootstrap,
    light_client_header::{LightClientHeader, LightClientUpdate},
    spec::Spec,
    sync_committee::SyncCommittee,
};
use sp1_ethereum_prover::update_wire::consensus_hash;

pub struct Beacon {
    client: BeaconApiClient,
    url: String,
    execution_rpc: String,
    spec: Spec,
    genesis_time: u64,
    genesis_validators_root: B256,
    finality_timeout: Duration,
}

pub struct Step {
    pub header: Header,
    pub trusted: ConsensusState,
    pub updated: ConsensusState,
    pub timestamp: u64,
}

impl Beacon {
    pub async fn connect(
        url: &str,
        execution_rpc: &str,
        finality_timeout: Duration,
    ) -> Result<Self> {
        let client = BeaconApiClient::new(url.to_string());
        let spec = sp1_ethereum_prover::fetch_spec(url).await?;
        let genesis = client
            .genesis()
            .await
            .map_err(|error| anyhow!("reading the beacon genesis: {error}"))?
            .data;

        Ok(Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            execution_rpc: execution_rpc.to_string(),
            spec,
            genesis_time: genesis.genesis_time,
            genesis_validators_root: genesis.genesis_validators_root,
            finality_timeout,
        })
    }

    fn period_length(&self) -> u64 {
        self.spec.epochs_per_sync_committee_period * self.spec.slots_per_epoch
    }

    pub fn period_of(&self, slot: u64) -> u64 {
        slot / self.period_length()
    }

    pub async fn witness(
        &self,
        header: &LightClientHeader,
    ) -> Result<Option<alloy::primitives::Bytes>> {
        match header {
            LightClientHeader::Electra(_) => Ok(None),
            LightClientHeader::Gloas(gloas) => sp1_ethereum_prover::fetch_execution_header(
                &self.execution_rpc,
                gloas.execution_block_hash,
            )
            .await
            .map(Some),
        }
    }

    pub async fn execution(&self, header: &LightClientHeader) -> Result<ExecutionData> {
        let witness = self.witness(header).await?;

        execution_data(header, witness.as_ref()).map_err(|error| {
            anyhow!("the execution header does not match the light client header: {error}")
        })
    }

    async fn bootstrap_at(&self, slot: u64) -> Result<LightClientBootstrap> {
        let root = self
            .client
            .beacon_block_root(&slot.to_string())
            .await
            .map_err(|error| anyhow!("slot {slot} has no block root: {error}"))?;

        Ok(self
            .client
            .light_client_bootstrap(&root)
            .await
            .map_err(|error| {
                anyhow!("the beacon node serves no bootstrap for slot {slot}: {error}")
            })?
            .data)
    }

    pub fn client_state(
        &self,
        chain_id: u64,
        ibc_contract_address: Address,
        ibc_commitment_slot: U256,
        latest_slot: u64,
        latest_execution_block_number: u64,
    ) -> ClientState {
        ClientState {
            chain_id,
            genesis_validators_root: self.genesis_validators_root,
            min_sync_committee_participants: self.spec.sync_committee_size * 2 / 3,
            sync_committee_size: self.spec.sync_committee_size,
            genesis_time: self.genesis_time,
            genesis_slot: self.spec.genesis_slot,
            fork_parameters: self.spec.to_fork_parameters(),
            seconds_per_slot: self.spec.seconds_per_slot,
            slots_per_epoch: self.spec.slots_per_epoch,
            epochs_per_sync_committee_period: self.spec.epochs_per_sync_committee_period,
            latest_slot,
            latest_execution_block_number,
            ibc_contract_address,
            ibc_commitment_slot,
            ..Default::default()
        }
    }

    async fn period_update(&self, period: u64) -> Result<LightClientUpdate> {
        self.client
            .light_client_updates(period, 1)
            .await
            .map_err(|error| {
                anyhow!("reading the light client update for period {period}: {error}")
            })?
            .into_iter()
            .next()
            .map(|response| response.data)
            .ok_or_else(|| {
                anyhow!("the beacon node has no light client update for period {period}")
            })
    }

    pub async fn committee_of_period(&self, period: u64) -> Result<SyncCommittee> {
        self.period_update(period - 1)
            .await?
            .next_sync_committee
            .ok_or_else(|| anyhow!("the period {} update carries no next committee", period - 1))
    }

    async fn execution_block_hash_at(&self, slot: u64) -> Result<B256> {
        let block: serde_json::Value =
            reqwest::get(format!("{}/eth/v2/beacon/blocks/{slot}", self.url))
                .await
                .with_context(|| format!("reading the beacon block at slot {slot}"))?
                .json()
                .await
                .with_context(|| format!("decoding the beacon block at slot {slot}"))?;

        let body = &block["data"]["message"]["body"];
        let hash = body["signed_execution_payload_bid"]["message"]["parent_block_hash"]
            .as_str()
            .or_else(|| body["execution_payload"]["block_hash"].as_str())
            .ok_or_else(|| {
                anyhow!("the beacon block at slot {slot} carries no execution block hash: {block}")
            })?;

        hash.parse().with_context(|| {
            format!("the beacon block at slot {slot} has a malformed execution block hash")
        })
    }

    pub async fn rebuild(&self, slot: u64, stored: [u8; 32]) -> Result<(ConsensusState, u64)> {
        let block_hash = self.execution_block_hash_at(slot).await?;
        let encoded =
            sp1_ethereum_prover::fetch_execution_header(&self.execution_rpc, block_hash).await?;
        let execution = execution_data_for_block_hash(block_hash, &encoded).map_err(|error| {
            anyhow!("the execution header for slot {slot} does not match: {error}")
        })?;
        let current = self
            .committee_of_period(self.period_of(slot))
            .await?
            .to_summarized_sync_committee();

        let without_next = ConsensusState {
            slot,
            state_root: execution.state_root,
            timestamp: execution.timestamp,
            current_sync_committee: current,
            next_sync_committee: None,
        };

        let block_number = execution.block_number;

        if consensus_hash(&without_next) == stored {
            return Ok((without_next, block_number));
        }

        let next = self
            .committee_of_period(self.period_of(slot) + 1)
            .await?
            .to_summarized_sync_committee();

        let with_next = ConsensusState {
            next_sync_committee: Some(next),
            ..without_next
        };

        if consensus_hash(&with_next) == stored {
            return Ok((with_next, block_number));
        }

        bail!("no consensus state rebuilt for slot {slot} hashes to the one the Soroban client stored")
    }

    pub async fn finalized_covering(&self, block_number: u64) -> Result<LightClientUpdate> {
        let started = Instant::now();
        let floor = self.spec.sync_committee_size * 2 / 3;

        loop {
            let finality = self
                .client
                .finality_update()
                .await
                .map_err(|error| anyhow!("reading the finality update: {error}"))?
                .data;

            let participants = finality
                .sync_aggregate
                .sync_committee_bits
                .iter()
                .map(|byte| u64::from(byte.count_ones()))
                .sum::<u64>();
            let finalized_block = self
                .execution(&finality.finalized_header)
                .await?
                .block_number;

            if finalized_block >= block_number && participants >= floor {
                return Ok(finality.into());
            }

            if started.elapsed() > self.finality_timeout {
                bail!(
                    "block {block_number} was not finalized with at least {floor} signers within {}s (finalized block {finalized_block}, {participants} signers)",
                    self.finality_timeout.as_secs()
                );
            }

            tracing::info!(
                block_number,
                finalized_block,
                participants,
                "waiting for finality"
            );

            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }

    pub async fn genesis(&self) -> Result<(ConsensusState, u64)> {
        let finality = self
            .client
            .finality_update()
            .await
            .map_err(|error| anyhow!("reading the finality update: {error}"))?
            .data;
        let slot = finality.finalized_header.beacon().slot;
        let bootstrap = self.bootstrap_at(slot).await?;
        let execution = self.execution(&bootstrap.header).await?;
        let next = self
            .committee_of_period(self.period_of(slot) + 1)
            .await?
            .to_summarized_sync_committee();

        let consensus_state = ConsensusState {
            slot,
            state_root: execution.state_root,
            timestamp: execution.timestamp,
            current_sync_committee: bootstrap
                .current_sync_committee
                .to_summarized_sync_committee(),
            next_sync_committee: Some(next),
        };

        Ok((consensus_state, execution.block_number))
    }

    fn timestamp_of(&self, update: &LightClientUpdate) -> u64 {
        self.genesis_time + update.signature_slot * self.spec.seconds_per_slot
    }

    pub async fn steps(
        &self,
        client_state: &ClientState,
        trusted: ConsensusState,
        target: LightClientUpdate,
    ) -> Result<Vec<Step>> {
        let target_period = self.period_of(target.finalized_header.beacon().slot);
        let mut steps = Vec::new();
        let mut current = trusted;
        let mut state = client_state.clone();

        while self.period_of(current.slot) < target_period {
            let next_period = self.period_of(current.slot) + 1;

            if current.next_sync_committee.is_none() {
                bail!(
                    "the trusted state at slot {} does not know period {next_period}'s committee, so it cannot cross into it; \
                     the client needs to be replaced by one seeded with its next committee",
                    current.slot
                );
            }

            let update = self.period_update(next_period).await?;

            if update.finalized_header.beacon().slot <= current.slot {
                bail!(
                    "period {next_period}'s update does not advance past slot {}",
                    current.slot
                );
            }

            let committee = self.committee_of_period(next_period).await?;
            let execution_header = self.witness(&update.finalized_header).await?;
            let header = Header {
                active_sync_committee: ActiveSyncCommittee::Next(committee),
                consensus_update: update.clone(),
                trusted_slot: current.slot,
                execution_header,
            };

            let step = self.step(&state, current, header, &update)?;

            state.latest_slot = step.updated.slot;
            current = step.updated.clone();
            steps.push(step);
        }

        if target.finalized_header.beacon().slot > current.slot {
            let signature_period = self.period_of(target.signature_slot);
            let active = if signature_period == self.period_of(current.slot) {
                ActiveSyncCommittee::Current(self.committee_of_period(signature_period).await?)
            } else {
                ActiveSyncCommittee::Next(self.committee_of_period(signature_period).await?)
            };

            let execution_header = self.witness(&target.finalized_header).await?;
            let header = Header {
                active_sync_committee: active,
                consensus_update: target.clone(),
                trusted_slot: current.slot,
                execution_header,
            };

            steps.push(self.step(&state, current, header, &target)?);
        }

        if steps.is_empty() {
            bail!("the client is already at or past the finalized slot");
        }

        Ok(steps)
    }

    fn step(
        &self,
        client_state: &ClientState,
        trusted: ConsensusState,
        header: Header,
        update: &LightClientUpdate,
    ) -> Result<Step> {
        let (_, updated, _) =
            update_consensus_state(trusted.clone(), client_state.clone(), header.clone())
                .map_err(|error| anyhow!("the update would be refused: {error:?}"))?;

        Ok(Step {
            header,
            trusted,
            updated,
            timestamp: self.timestamp_of(update),
        })
    }
}

pub fn to_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(value).context("serialising for the guest")
}
