pub mod beacon;
mod evm;
pub mod soroban;

use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use alloy::primitives::{Address, TxHash, U256};
use anyhow::{anyhow, bail, Context, Result};
use ibc_eureka_solidity_types::ics26::ICS26_IBC_STORAGE_SLOT;
use proof_api_core::{
    api::{self, proof_api_service_server::ProofApiService},
    modules::ProofApiModule,
};
use proof_api_history::{NetworkRequest, ProofEntry, ProofHistory};
use serde::{Deserialize, Serialize};
use sp1_ethereum_prover::{
    membership_wire,
    proving::{header_stdin, membership_stdin, run, verifying_key, Mode, Outcome},
    update_wire,
};
use stellar_xdr::{ContractId, Hash, ScAddress};
use tonic::{Request, Response, Status};

use crate::{
    beacon::{to_json, Beacon, Step},
    evm::{Evm, Relayable},
    soroban::SorobanRpc,
};

#[derive(Clone, Copy, Debug)]
pub struct EthToStellarProofApiModule;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EthToStellarConfiguration {
    pub beacon_api_url: String,
    pub eth_rpc_url: String,
    pub ics26_address: Address,
    #[serde(default)]
    pub ibc_commitment_slot: Option<U256>,
    pub soroban_rpc_url: String,
    pub soroban_router: String,
    pub sp1_programs: ProgramPaths,
    #[serde(default)]
    pub prover: ProverMode,
    #[serde(default = "default_finality_timeout")]
    pub finality_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProgramPaths {
    pub update_client: String,
    pub membership: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProverMode {
    #[default]
    Network,
    Execute,
}

const fn default_finality_timeout() -> u64 {
    1_800
}

struct Programs {
    update_client: Vec<u8>,
    membership: Vec<u8>,
}

struct EthToStellarService {
    beacon: Beacon,
    evm: Evm,
    soroban: SorobanRpc,
    router: [u8; 32],
    router_strkey: String,
    programs: Arc<Programs>,
    chain_id: u64,
    mode: Mode,
    history: ProofHistory,
}

struct ProofContext {
    src_chain: String,
    dst_chain: String,
    kind: &'static str,
    tx_hash: String,
    sequences: Vec<u64>,
    trusted_height: u64,
    target_height: u64,
}

impl ProofContext {
    fn entry(&self, step: u32, program: &str, status: &str) -> ProofEntry {
        ProofEntry {
            module: "eth_to_stellar".to_string(),
            src_chain: self.src_chain.clone(),
            dst_chain: self.dst_chain.clone(),
            kind: self.kind.to_string(),
            tx_hash: self.tx_hash.clone(),
            sequences: self.sequences.clone(),
            step,
            program: program.to_string(),
            prover: "network".to_string(),
            status: status.to_string(),
            error: None,
            vkey: None,
            trusted_height: Some(self.trusted_height),
            target_height: Some(self.target_height),
            public_values: None,
            proof: None,
            request: None,
        }
    }

    fn outcome(&self, step: u32, program: &str, outcome: &Outcome) -> Option<ProofEntry> {
        let Outcome::Proved(proof) = outcome else {
            return None;
        };

        Some(ProofEntry {
            vkey: Some(proof.vkey),
            public_values: Some(proof.public_values.clone()),
            proof: Some(proof.proof.clone()),
            request: Some(NetworkRequest::from_sdk(
                proof.request_id,
                proof.request.as_deref(),
            )),
            ..self.entry(step, program, "fulfilled")
        })
    }

    fn failed(&self, error: &anyhow::Error) -> ProofEntry {
        ProofEntry {
            error: Some(format!("{error:#}")),
            ..self.entry(0, "update_client and membership", "failed")
        }
    }
}

fn contract_id(strkey: &str) -> Result<[u8; 32]> {
    match ScAddress::from_str(strkey).map_err(|error| anyhow!("{strkey}: {error}"))? {
        ScAddress::Contract(ContractId(Hash(id))) => Ok(id),
        _ => bail!("{strkey} is not a contract address"),
    }
}

fn soroban_path(client: &str, discriminator: u8, sequence: u64) -> Vec<u8> {
    let mut path = client.as_bytes().to_vec();

    path.push(discriminator);
    path.extend_from_slice(&sequence.to_be_bytes());

    path
}

struct Proven {
    updates: Vec<Outcome>,
    membership: Outcome,
}

struct Commitment {
    storage_proof_json: Vec<u8>,
    path: Vec<u8>,
    value: Vec<u8>,
}

fn prove(
    programs: &Programs,
    mode: Mode,
    client_state: &ethereum_light_client::client_state::ClientState,
    steps: &[Step],
    anchor: &ethereum_light_client::consensus_state::ConsensusState,
    commitment: &Commitment,
) -> Result<Proven> {
    let Commitment {
        storage_proof_json,
        path,
        value,
    } = commitment;

    let mut updates = Vec::new();
    let mut state = client_state.clone();

    for step in steps {
        state.latest_slot = step.trusted.slot;

        let stdin = header_stdin(
            &to_json(&state)?,
            &to_json(&step.trusted)?,
            &to_json(&step.header)?,
            step.timestamp,
        );
        let outcome = run(&programs.update_client, stdin, mode, "update-client")?;
        let expected = update_wire::output(&state, &step.trusted, &step.updated);

        if outcome.public_values() != expected.as_slice() {
            bail!(
                "the update-client guest committed something other than the host computed for slot {}",
                step.updated.slot
            );
        }

        tracing::info!(
            from = step.trusted.slot,
            to = step.updated.slot,
            "update-client done"
        );

        updates.push(outcome);
    }

    let last = anchor;

    if steps.is_empty() {
        state.latest_slot = anchor.slot;
    }

    let path_segments = vec![path.to_vec()];
    let stdin = membership_stdin(
        &to_json(&state)?,
        &to_json(last)?,
        storage_proof_json,
        &to_json(&path_segments)?,
        value,
    );
    let membership = run(&programs.membership, stdin, mode, "membership")?;
    let expected = membership_wire::output(&state, last, &path_segments, value);

    if membership.public_values() != expected.as_slice() {
        bail!("the membership guest committed something other than the host computed");
    }

    Ok(Proven {
        updates,
        membership,
    })
}

impl EthToStellarService {
    async fn new(configuration: EthToStellarConfiguration) -> Result<Self> {
        let router = contract_id(&configuration.soroban_router)?;
        let programs = Programs {
            update_client: std::fs::read(&configuration.sp1_programs.update_client)
                .with_context(|| format!("reading {}", configuration.sp1_programs.update_client))?,
            membership: std::fs::read(&configuration.sp1_programs.membership)
                .with_context(|| format!("reading {}", configuration.sp1_programs.membership))?,
        };
        let commitment_slot = configuration
            .ibc_commitment_slot
            .unwrap_or_else(|| U256::from_be_slice(&ICS26_IBC_STORAGE_SLOT));
        let evm = Evm::connect(
            &configuration.eth_rpc_url,
            configuration.ics26_address,
            commitment_slot,
        )
        .await?;
        let chain_id = evm.chain_id().await?;
        let beacon = Beacon::connect(
            &configuration.beacon_api_url,
            &configuration.eth_rpc_url,
            Duration::from_secs(configuration.finality_timeout_seconds),
        )
        .await?;

        let history = ProofHistory::from_env();

        if let Err(error) = history.prepare().await {
            tracing::warn!(%error, "the proof history is unavailable; proofs are not recorded");
        }

        Ok(Self {
            history,
            beacon,
            evm,
            soroban: SorobanRpc::new(configuration.soroban_rpc_url),
            router,
            router_strkey: configuration.soroban_router,
            programs: Arc::new(programs),
            chain_id,
            mode: match configuration.prover {
                ProverMode::Network => Mode::Network,
                ProverMode::Execute => Mode::Execute,
            },
        })
    }

    async fn create(&self, parameters: &HashMap<String, String>) -> Result<Vec<u8>> {
        if let Some(unexpected) = parameters.keys().find(|key| key.as_str() != "client_type") {
            bail!("unexpected parameter `{unexpected}`; only `client_type` is accepted");
        }

        let client_type = parameters
            .get("client_type")
            .ok_or_else(|| anyhow!("the `client_type` parameter is required"))?;

        let (consensus_state, block_number) = self.beacon.genesis().await?;
        let client_state = self.beacon.client_state(
            self.chain_id,
            self.evm.router(),
            self.evm.commitment_slot(),
            consensus_state.slot,
            block_number,
        );

        let programs = Arc::clone(&self.programs);
        let (update_vkey, membership_vkey) = tokio::task::spawn_blocking(move || {
            Ok::<_, anyhow::Error>((
                verifying_key(&programs.update_client)?,
                verifying_key(&programs.membership)?,
            ))
        })
        .await
        .map_err(|error| anyhow!("deriving the verifying keys panicked: {error}"))??;

        let commitment = update_wire::client_state_commitment(&client_state);
        let hash = update_wire::consensus_hash(&consensus_state);

        let mut client_state_bytes = update_vkey.to_vec();

        client_state_bytes.extend_from_slice(&membership_vkey);
        client_state_bytes.extend_from_slice(&commitment);

        let mut consensus_state_bytes = hash.to_vec();

        consensus_state_bytes.extend_from_slice(&consensus_state.timestamp.to_be_bytes());

        tracing::info!(
            client_type = %client_type,
            slot = consensus_state.slot,
            block_number,
            period = self.beacon.period_of(consensus_state.slot),
            update_vkey = %hex::encode(update_vkey),
            membership_vkey = %hex::encode(membership_vkey),
            commitment = %hex::encode(commitment),
            consensus_hash = %hex::encode(hash),
            "built a genesis with the next sync committee seeded"
        );

        soroban::encode_host_functions(&[soroban::create_client(
            self.router,
            client_type,
            &client_state_bytes,
            &consensus_state_bytes,
            consensus_state.slot,
        )?])
    }

    async fn relay(&self, request: api::RelayByTxRequest) -> Result<Vec<u8>> {
        let [identifier] = request.source_tx_ids.as_slice() else {
            bail!(
                "relaying {} EVM transactions at once is not built yet; send one",
                request.source_tx_ids.len()
            );
        };

        if !request.timeout_tx_ids.is_empty() {
            bail!("timeouts are not built yet");
        }

        let transaction = transaction_hash(identifier)?;
        let evm_client = request.src_client_id.as_str();
        let stellar_client = request.dst_client_id.as_str();
        let found = self
            .evm
            .relayable(
                transaction,
                evm_client,
                stellar_client,
                &request.src_packet_sequences,
            )
            .await?;
        let packet = found.relayable.packet().clone();

        tracing::info!(
            %transaction,
            block = found.block_number,
            sequence = packet.sequence,
            acknowledgement = matches!(found.relayable, Relayable::Acknowledge(..)),
            "found the packet to relay"
        );

        match &found.relayable {
            Relayable::Receive(_) => {
                if self
                    .soroban
                    .provable_entry_exists(
                        self.router,
                        &soroban_path(&packet.destClient, 2, packet.sequence),
                    )
                    .await?
                {
                    bail!(
                        "the Soroban router already holds a receipt for {} sequence {}",
                        packet.destClient,
                        packet.sequence
                    );
                }
            }
            Relayable::Acknowledge(..) => {
                if !self
                    .soroban
                    .provable_entry_exists(
                        self.router,
                        &soroban_path(&packet.sourceClient, 1, packet.sequence),
                    )
                    .await?
                {
                    bail!(
                        "the Soroban router holds no commitment for {} sequence {}: already acknowledged or timed out",
                        packet.sourceClient,
                        packet.sequence
                    );
                }
            }
        }

        let light_client = self
            .soroban
            .light_client(self.router, stellar_client)
            .await?;
        let trusted_slot = self
            .soroban
            .latest_slot(light_client, stellar_client)
            .await?;
        let stored_hash = self
            .soroban
            .consensus_hash(light_client, stellar_client, trusted_slot)
            .await?;
        let stored_commitment = self
            .soroban
            .client_state_commitment(light_client, stellar_client)
            .await?;

        let (trusted, trusted_block) = self.beacon.rebuild(trusted_slot, stored_hash).await?;
        let client_state = self.beacon.client_state(
            self.chain_id,
            self.evm.router(),
            self.evm.commitment_slot(),
            trusted_slot,
            trusted_block,
        );

        if update_wire::client_state_commitment(&client_state) != stored_commitment {
            bail!(
                "the client state built here does not match the commitment the Soroban client pins"
            );
        }

        let (steps, anchor, proof_block) = if trusted_block >= found.block_number {
            tracing::info!(
                trusted_slot,
                trusted_block,
                "the client already covers the transaction's block; proving membership without an update"
            );

            (Vec::new(), trusted, trusted_block)
        } else {
            let target = self.beacon.finalized_covering(found.block_number).await?;
            let steps = self.beacon.steps(&client_state, trusted, target).await?;
            let last_step = steps.last().ok_or_else(|| anyhow!("no update steps"))?;
            let anchor = last_step.updated.clone();
            let proof_block = ethereum_light_client::execution::execution_data(
                &last_step.header.consensus_update.finalized_header,
                last_step.header.execution_header.as_ref(),
            )
            .map_err(|error| anyhow!("the last step's execution header does not match: {error}"))?
            .block_number;

            (steps, anchor, proof_block)
        };
        let proof_slot = anchor.slot;

        tracing::info!(
            trusted_slot,
            proof_slot,
            proof_block,
            headers = steps.len(),
            "built the update chain"
        );

        let path = found.relayable.evm_path();
        let value = found.relayable.expected_value();
        let membership_proof = self.evm.membership_proof(&path, proof_block).await?;

        if membership_proof.storage_proof.value != U256::from_be_slice(&value) {
            bail!(
                "the EVM router stores {} at {}, the packet implies {}",
                membership_proof.storage_proof.value,
                hex::encode(&path),
                hex::encode(&value)
            );
        }

        let storage_proof_json = to_json(&membership_proof)?;
        let programs = Arc::clone(&self.programs);
        let mode = self.mode;
        let proving_state = client_state.clone();
        let proving_path = path.clone();
        let proving_value = value.clone();

        let context = ProofContext {
            src_chain: request.src_chain.clone(),
            dst_chain: request.dst_chain.clone(),
            kind: match found.relayable {
                Relayable::Receive(..) => "recv",
                Relayable::Acknowledge(..) => "ack",
            },
            tx_hash: transaction.to_string(),
            sequences: vec![packet.sequence],
            trusted_height: trusted_slot,
            target_height: proof_slot,
        };

        let proven = tokio::task::spawn_blocking(move || {
            prove(
                &programs,
                mode,
                &proving_state,
                &steps,
                &anchor,
                &Commitment {
                    storage_proof_json,
                    path: proving_path,
                    value: proving_value,
                },
            )
        })
        .await
        .map_err(|error| anyhow!("proving panicked: {error}"))?;

        let proven = match proven {
            Ok(proven) => proven,
            Err(error) => {
                if self.mode == Mode::Network {
                    self.history.record(&context.failed(&error)).await;
                }

                return Err(error);
            }
        };

        let steps = proven.updates.len() as u32;

        for (step, update) in proven.updates.iter().enumerate() {
            if let Some(entry) = context.outcome(step as u32, "update_client", update) {
                self.history.record(&entry).await;
            }
        }

        if let Some(entry) = context.outcome(steps, "membership", &proven.membership) {
            self.history.record(&entry).await;
        }

        if self.mode == Mode::Execute {
            tracing::info!(
                trusted_slot,
                proof_slot,
                headers = proven.updates.len(),
                "dry run passed: every header and the membership executed and matched the host; no proof made"
            );

            return Ok(Vec::new());
        }

        let mut functions = Vec::new();

        for outcome in &proven.updates {
            let message = outcome
                .client_message()
                .ok_or_else(|| anyhow!("an update was executed, not proven"))?;

            functions.push(soroban::update_client(
                self.router,
                stellar_client,
                &message,
            )?);
        }

        let membership_message = proven
            .membership
            .client_message()
            .ok_or_else(|| anyhow!("the membership was executed, not proven"))?;

        functions.push(match &found.relayable {
            Relayable::Receive(packet) => {
                soroban::recv_packet(self.router, packet, &membership_message, proof_slot)?
            }
            Relayable::Acknowledge(packet, acknowledgement) => soroban::acknowledge_packet(
                self.router,
                packet,
                acknowledgement,
                &membership_message,
                proof_slot,
            )?,
        });

        soroban::encode_host_functions(&functions)
    }
}

fn transaction_hash(identifier: &[u8]) -> Result<TxHash> {
    if identifier.len() == 32 {
        return Ok(TxHash::from_slice(identifier));
    }

    let text = std::str::from_utf8(identifier)
        .context("the transaction id is neither 32 bytes nor text")?;

    TxHash::from_str(text.trim())
        .map_err(|error| anyhow!("{text} is not an EVM transaction hash: {error}"))
}

#[tonic::async_trait]
impl ProofApiService for EthToStellarService {
    async fn info(
        &self,
        _request: Request<api::InfoRequest>,
    ) -> Result<Response<api::InfoResponse>, Status> {
        Ok(Response::new(api::InfoResponse {
            target_chain: Some(api::Chain {
                chain_id: "stellar".to_string(),
                ibc_version: "2".to_string(),
                ibc_contract: self.router_strkey.clone(),
            }),
            source_chain: Some(api::Chain {
                chain_id: self.chain_id.to_string(),
                ibc_version: "2".to_string(),
                ibc_contract: self.evm.router().to_string(),
            }),
            metadata: HashMap::new(),
        }))
    }

    async fn relay_by_tx(
        &self,
        request: Request<api::RelayByTxRequest>,
    ) -> Result<Response<api::RelayByTxResponse>, Status> {
        let transaction = self
            .relay(request.into_inner())
            .await
            .map_err(|error| Status::failed_precondition(format!("{error:#}")))?;

        Ok(Response::new(api::RelayByTxResponse {
            tx: transaction,
            address: self.router_strkey.clone(),
        }))
    }

    async fn create_client(
        &self,
        request: Request<api::CreateClientRequest>,
    ) -> Result<Response<api::CreateClientResponse>, Status> {
        let transaction = self
            .create(&request.into_inner().parameters)
            .await
            .map_err(|error| Status::failed_precondition(format!("{error:#}")))?;

        Ok(Response::new(api::CreateClientResponse {
            tx: transaction,
            address: self.router_strkey.clone(),
        }))
    }

    async fn update_client(
        &self,
        _request: Request<api::UpdateClientRequest>,
    ) -> Result<Response<api::UpdateClientResponse>, Status> {
        Err(Status::unimplemented(
            "eth_to_stellar updates clients only as part of a relay",
        ))
    }
}

#[tonic::async_trait]
impl ProofApiModule for EthToStellarProofApiModule {
    fn name(&self) -> &'static str {
        "eth_to_stellar"
    }

    async fn create_service(
        &self,
        configuration: serde_json::Value,
    ) -> Result<Box<dyn ProofApiService>> {
        let configuration = serde_json::from_value::<EthToStellarConfiguration>(configuration)
            .map_err(|error| {
                anyhow!("failed to parse the eth_to_stellar configuration: {error}")
            })?;

        tracing::info!(router = %configuration.soroban_router, "starting the Ethereum to Stellar proof API module");

        Ok(Box::new(EthToStellarService::new(configuration).await?))
    }
}
