mod archive;
mod evm;
pub mod gateway;
pub mod soroban;
pub mod witness;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use alloy::{
    primitives::Address,
    sol_types::{SolCall, SolValue},
};
use anyhow::{anyhow, bail, Context, Result};
use ibc_eureka_solidity_types::ics26::{
    router::{ackPacketCall, multicallCall, recvPacketCall, updateClientCall},
    IICS02ClientMsgs::Height,
    IICS26RouterMsgs::{MsgAckPacket, MsgRecvPacket, Packet},
};
use proof_api_core::{
    api::{self, proof_api_service_server::ProofApiService},
    modules::ProofApiModule,
};
use proof_api_history::{NetworkRequest, ProofEntry, ProofHistory};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sp1_stellar_prover::{
    consensus_state_of, decode_contract_id,
    relay::{
        execute_membership_on_cpu, execute_on_cpu, prove_membership_on_network, prove_on_network,
        run_membership_natively, run_natively, verifying_key, Programs, Proof, RelayProofs,
        Trusted,
    },
};
use stellar_light_client_solidity_types::{
    ClientState, Height as StellarHeight,
    ISP1Msgs::SP1Proof,
    IStellarMembershipMsgs::{
        SP1MembershipProof, StellarMembershipProof, StellarMembershipProofType,
    },
    IStellarUpdateClientMsgs::MsgUpdateClient,
    SupportedZkAlgorithm,
};
use tonic::{Request, Response, Status};

use crate::{
    archive::Archive,
    evm::{consensus_hash_of, Evm},
    soroban::{bytes, field, packet, router_events, RouterEvent, SorobanRpc},
};

#[derive(Clone, Copy, Debug)]
pub struct StellarToEthProofApiModule;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StellarToEthConfiguration {
    pub soroban_rpc_url: String,
    pub stellar_network: String,
    #[serde(default)]
    pub archive_urls: Option<Vec<String>>,
    pub soroban_router: String,
    pub eth_rpc_url: String,
    pub ics26_address: Address,
    pub sp1_programs: ProgramPaths,
    #[serde(default = "default_checkpoint_timeout")]
    pub checkpoint_timeout_seconds: u64,
    #[serde(default)]
    pub prover: ProverMode,
    #[serde(default)]
    pub client_creation: Option<ClientCreation>,
    #[serde(default)]
    pub gateway_grpc_address: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientCreation {
    pub artifact: String,
    pub sp1_verifier: Address,
    pub uc_and_membership: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProverMode {
    #[default]
    Network,
    Execute,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProgramPaths {
    pub update_client: String,
    pub membership: String,
}

const fn default_checkpoint_timeout() -> u64 {
    1_200
}

struct StellarToEthService {
    soroban: SorobanRpc,
    archive: Archive,
    evm: Evm,
    router: [u8; 32],
    router_strkey: String,
    programs: Arc<Programs>,
    chain_id: u64,
    prover: ProverMode,
    client_creation: Option<ClientCreation>,
    gateway: Option<String>,
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
            module: "stellar_to_eth".to_string(),
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

    fn proved(&self, step: u32, program: &str, proof: &Proof) -> ProofEntry {
        ProofEntry {
            vkey: Some(proof.vkey),
            public_values: Some(proof.public_values.clone()),
            proof: Some(proof.proof.clone()),
            request: Some(NetworkRequest::from_sdk(
                proof.request_id,
                proof.request.as_deref(),
            )),
            ..self.entry(step, program, "fulfilled")
        }
    }

    fn failed(&self, program: &str, error: &anyhow::Error) -> ProofEntry {
        ProofEntry {
            error: Some(format!("{error:#}")),
            ..self.entry(0, program, "failed")
        }
    }
}

enum Relayable {
    Receive(Packet),
    Acknowledge(Packet, Vec<u8>),
}

impl Relayable {
    fn packet(&self) -> &Packet {
        match self {
            Self::Receive(packet) | Self::Acknowledge(packet, _) => packet,
        }
    }

    fn path(&self) -> Vec<u8> {
        let (client, discriminator) = match self {
            Self::Receive(packet) => (&packet.sourceClient, 1_u8),
            Self::Acknowledge(packet, _) => (&packet.destClient, 3_u8),
        };

        let mut path = client.as_bytes().to_vec();

        path.push(discriminator);
        path.extend_from_slice(&self.packet().sequence.to_be_bytes());

        path
    }

    fn expected_value(&self) -> Vec<u8> {
        match self {
            Self::Receive(packet) => packet.commitment(),
            Self::Acknowledge(_, acknowledgement) => {
                let mut preimage = vec![2_u8];

                preimage.extend_from_slice(&Sha256::digest(acknowledgement));

                Sha256::digest(&preimage).to_vec()
            }
        }
    }
}

fn transaction_hash(identifier: &[u8]) -> Result<String> {
    if identifier.len() == 32 {
        return Ok(hex::encode(identifier));
    }

    let text = std::str::from_utf8(identifier)
        .context("the transaction id is neither 32 bytes nor text")?;
    let hash = text.trim().trim_start_matches("0x").to_lowercase();

    if hash.len() != 64 || hex::decode(&hash).is_err() {
        bail!("{text} is not a Stellar transaction hash");
    }

    Ok(hash)
}

fn relayable(
    events: &[RouterEvent],
    stellar_client: &str,
    evm_client: &str,
    sequences: &[u64],
) -> Result<Relayable> {
    let wanted = |packet: &Packet| sequences.is_empty() || sequences.contains(&packet.sequence);
    let mut found = Vec::new();
    let mut acknowledgements = Vec::new();

    for event in events {
        match event.name.as_str() {
            "send_packet" => {
                let sent = packet(field(&event.data, "packet")?)?;

                if sent.sourceClient == stellar_client
                    && sent.destClient == evm_client
                    && wanted(&sent)
                {
                    found.push(Relayable::Receive(sent));
                }
            }
            "recv_packet" => {
                let received = packet(field(&event.data, "packet")?)?;

                if received.destClient == stellar_client
                    && received.sourceClient == evm_client
                    && wanted(&received)
                {
                    found.push(Relayable::Acknowledge(received, Vec::new()));
                }
            }
            "write_ack" => {
                let stellar_xdr::ScVal::Vec(Some(written)) =
                    field(&event.data, "acknowledgements")?
                else {
                    bail!("write_ack carries no acknowledgement vector");
                };

                let first = written
                    .first()
                    .ok_or_else(|| anyhow!("write_ack carries no acknowledgement"))?;

                acknowledgements.push(bytes(first)?);
            }
            _ => {}
        }
    }

    let mut pending = acknowledgements.into_iter();

    for item in &mut found {
        if let Relayable::Acknowledge(_, acknowledgement) = item {
            *acknowledgement = pending
                .next()
                .ok_or_else(|| anyhow!("a received packet has no written acknowledgement"))?;
        }
    }

    match found.len() {
        1 => Ok(found.remove(0)),
        0 => bail!(
            "the transaction holds no packet from {stellar_client} to {evm_client} and no acknowledgement for one"
        ),
        count => bail!(
            "the transaction holds {count} relayable packets; relaying more than one per transaction is not built yet"
        ),
    }
}

fn now_in_nanoseconds() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default()
}

fn sp1_proof(proof: &sp1_stellar_prover::relay::Proof) -> SP1Proof {
    SP1Proof {
        vKey: proof.vkey.into(),
        publicValues: proof.public_values.clone().into(),
        proof: proof.proof.clone().into(),
    }
}

fn membership_call(
    item: Relayable,
    membership: &sp1_stellar_prover::relay::Proof,
    trusted: stellar_light_client_solidity_types::ConsensusState,
    ledger: u64,
) -> Vec<u8> {
    let proof_envelope = StellarMembershipProof {
        proofType: StellarMembershipProofType::SP1MembershipProof,
        proof: SP1MembershipProof {
            sp1Proof: sp1_proof(membership),
            trustedConsensusState: trusted,
        }
        .abi_encode()
        .into(),
    }
    .abi_encode();

    let height = Height {
        revisionNumber: 0,
        revisionHeight: ledger,
    };

    match item {
        Relayable::Receive(packet) => recvPacketCall {
            msg_: MsgRecvPacket {
                packet,
                proofCommitment: proof_envelope.into(),
                proofHeight: height,
            },
        }
        .abi_encode(),
        Relayable::Acknowledge(packet, acknowledgement) => ackPacketCall {
            msg_: MsgAckPacket {
                packet,
                acknowledgement: acknowledgement.into(),
                proofAcked: proof_envelope.into(),
                proofHeight: height,
            },
        }
        .abi_encode(),
    }
}

fn router_calls(
    evm_client: String,
    item: Relayable,
    proofs: &RelayProofs,
    ledger: u64,
) -> Vec<Vec<u8>> {
    let update = updateClientCall {
        clientId: evm_client,
        updateMsg: MsgUpdateClient {
            sp1Proof: sp1_proof(&proofs.update),
        }
        .abi_encode()
        .into(),
    };

    let membership = membership_call(
        item,
        &proofs.membership,
        proofs.update_output.newConsensusState.clone(),
        ledger,
    );

    vec![update.abi_encode(), membership]
}

impl StellarToEthService {
    async fn siblings(&self, ledger: u32, path: &[u8]) -> Result<Vec<[u8; 32]>> {
        let address = self.gateway.as_deref().ok_or_else(|| {
            anyhow!(
                "gateway_grpc_address is not configured: the router keeps its tree as separate \
                 entries, so a packet's siblings come from the gateway's mirror of it"
            )
        })?;

        let tree = gateway::tree_siblings(address, ledger, path).await?;

        tracing::info!(
            ledger,
            root = %hex::encode(tree.root),
            "the gateway supplied the tree siblings"
        );

        Ok(tree.siblings)
    }

    async fn new(configuration: StellarToEthConfiguration) -> Result<Self> {
        let router = decode_contract_id(&configuration.soroban_router)
            .map_err(|error| anyhow!("soroban_router: {error}"))?;
        let programs = Programs {
            update_client: std::fs::read(&configuration.sp1_programs.update_client)
                .with_context(|| format!("reading {}", configuration.sp1_programs.update_client))?,
            membership: std::fs::read(&configuration.sp1_programs.membership)
                .with_context(|| format!("reading {}", configuration.sp1_programs.membership))?,
        };
        let evm = Evm::connect(&configuration.eth_rpc_url, configuration.ics26_address).await?;
        let chain_id = evm.chain_id().await?;
        let archive = Archive::new(
            &configuration.stellar_network,
            configuration.archive_urls.clone(),
            Duration::from_secs(configuration.checkpoint_timeout_seconds),
        )?;

        let history = ProofHistory::from_env();

        if let Err(error) = history.prepare().await {
            tracing::warn!(%error, "the proof history is unavailable; proofs are not recorded");
        }

        Ok(Self {
            soroban: SorobanRpc::new(configuration.soroban_rpc_url),
            archive,
            evm,
            router,
            router_strkey: configuration.soroban_router,
            programs: Arc::new(programs),
            chain_id,
            prover: configuration.prover,
            client_creation: configuration.client_creation,
            gateway: configuration.gateway_grpc_address,
            history,
        })
    }

    async fn refuse_settled(&self, item: &Relayable) -> Result<()> {
        let packet = item.packet();
        let sequence = packet.sequence.to_be_bytes();

        match item {
            Relayable::Receive(_) => {
                let mut receipt_path = packet.destClient.as_bytes().to_vec();

                receipt_path.push(2);
                receipt_path.extend_from_slice(&sequence);

                let receipt = self.evm.commitment(&receipt_path).await?;

                if receipt != [0; 32] {
                    bail!(
                        "the EVM router already holds a receipt for {} sequence {} ({}): the packet was received, \
                         or its sequence collides with a packet from an earlier counterparty under the same client id",
                        packet.destClient,
                        packet.sequence,
                        hex::encode(receipt)
                    );
                }
            }
            Relayable::Acknowledge(..) => {
                let mut commitment_path = packet.sourceClient.as_bytes().to_vec();

                commitment_path.push(1);
                commitment_path.extend_from_slice(&sequence);

                if self.evm.commitment(&commitment_path).await? == [0; 32] {
                    bail!(
                        "the EVM router holds no commitment for {} sequence {}: it was already acknowledged or timed out",
                        packet.sourceClient,
                        packet.sequence
                    );
                }
            }
        }

        Ok(())
    }

    async fn create(&self, parameters: &HashMap<String, String>) -> Result<Vec<u8>> {
        if let Some(unexpected) = parameters.keys().find(|key| key.as_str() != "ledger") {
            bail!("unexpected parameter `{unexpected}`; only an optional `ledger` is accepted");
        }

        let creation = self.client_creation.as_ref().ok_or_else(|| {
            anyhow!("client creation needs `client_creation` in this module's configuration")
        })?;

        let ledger = match parameters.get("ledger") {
            Some(raw) => raw
                .parse::<u32>()
                .with_context(|| format!("ledger {raw} is not a number"))?,
            None => self.archive.current_ledger().await? - 2,
        };

        let fixture = self.archive.fixture(ledger).await?;
        let trust_root: [u8; 32] = Sha256::digest(fixture.trust_root_xdr()).into();
        let network_id = sp1_stellar_prover::decode_hex32(&fixture.network_id);

        let consensus_state =
            tokio::task::spawn_blocking(move || consensus_state_of(&fixture.verify_natively(None)))
                .await
                .map_err(|error| anyhow!("verifying the genesis ledger panicked: {error}"))?;

        let client_state = ClientState {
            networkId: network_id.into(),
            trustRootFingerprint: trust_root.into(),
            routerContractId: self.router.into(),
            rootEventTopic: sp1_stellar_prover::ROOT_EVENT_TOPIC.to_vec().into(),
            latestHeight: StellarHeight {
                revisionNumber: 0,
                revisionHeight: u64::from(ledger),
            },
            isFrozen: false,
            zkAlgorithm: SupportedZkAlgorithm::Groth16,
        };

        let artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&creation.artifact)
                .with_context(|| format!("reading {}", creation.artifact))?,
        )
        .context("the client artifact is not JSON")?;
        let bytecode = artifact
            .pointer("/bytecode/object")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("the client artifact carries no bytecode"))?;
        let mut creation_code =
            hex::decode(bytecode.trim_start_matches("0x")).context("the bytecode is not hex")?;

        let programs = Arc::clone(&self.programs);
        let combined = std::fs::read(&creation.uc_and_membership)
            .with_context(|| format!("reading {}", creation.uc_and_membership))?;
        let (update_vkey, membership_vkey, combined_vkey) =
            tokio::task::spawn_blocking(move || {
                Ok::<_, anyhow::Error>((
                    verifying_key(&programs.update_client)?,
                    verifying_key(&programs.membership)?,
                    verifying_key(&combined)?,
                ))
            })
            .await
            .map_err(|error| anyhow!("deriving the verifying keys panicked: {error}"))??;

        let consensus_hash = consensus_hash_of(&consensus_state);
        let arguments = (
            alloy::primitives::B256::from(update_vkey),
            alloy::primitives::B256::from(membership_vkey),
            alloy::primitives::B256::from(combined_vkey),
            creation.sp1_verifier,
            alloy::primitives::Bytes::from(client_state.abi_encode()),
            alloy::primitives::B256::from(consensus_hash),
            self.evm.router(),
        )
            .abi_encode_params();

        creation_code.extend_from_slice(&arguments);

        tracing::info!(
            ledger,
            ledger_hash = %hex::encode(consensus_state.ledgerHash.0),
            trust_root = %hex::encode(trust_root),
            router = %self.router_strkey,
            role_manager = %self.evm.router(),
            update_vkey = %hex::encode(update_vkey),
            membership_vkey = %hex::encode(membership_vkey),
            combined_vkey = %hex::encode(combined_vkey),
            "built an SP1ICSStellar deployment from a verified genesis ledger"
        );

        Ok(creation_code)
    }

    async fn relay_at_stored_height(
        &self,
        client: Address,
        trusted_height: u64,
        item: Relayable,
        transaction: &soroban::Transaction,
        executed: &soroban::Executed,
        context: &ProofContext,
    ) -> Result<Vec<u8>> {
        let ledger = u64::from(transaction.ledger);
        let stored = self.evm.consensus_hash(client, ledger).await.map_err(|_| {
            anyhow!(
                "the client is at ledger {trusted_height}, past ledger {ledger}, and holds no consensus state for it; \
                 an earlier relay moved it on first, so this packet can only be proven through an update to that exact ledger, \
                 which is not built yet"
            )
        })?;

        self.archive.wait_for(transaction.ledger).await?;

        let fixture = self.archive.fixture(transaction.ledger).await?;
        let (mut fixture, consensus_state) = tokio::task::spawn_blocking(move || {
            let consensus_state = consensus_state_of(&fixture.verify_natively(None));

            (fixture, consensus_state)
        })
        .await
        .map_err(|error| anyhow!("rebuilding the consensus state panicked: {error}"))?;

        if consensus_hash_of(&consensus_state) != stored {
            bail!("the consensus state rebuilt from ledger {ledger} does not hash to the one the client stored");
        }

        let path = item.path();
        let siblings = self.siblings(transaction.ledger, &path).await?;
        let value = witness::attach(
            &mut fixture,
            transaction,
            executed,
            self.router,
            &path,
            &siblings,
        )?;

        if value != item.expected_value() {
            bail!(
                "the router committed {} at {}, the packet implies {}",
                hex::encode(&value),
                hex::encode(&path),
                hex::encode(item.expected_value())
            );
        }

        tracing::info!(
            ledger,
            trusted_height,
            "the client already holds this ledger; proving membership without an update"
        );

        let router = self.router;
        let programs = Arc::clone(&self.programs);
        let stored_state = consensus_state.clone();

        if self.prover == ProverMode::Execute {
            let cycles = tokio::task::spawn_blocking(move || {
                let native = run_membership_natively(&fixture, &stored_state, router)?;

                execute_membership_on_cpu(native, &programs)
            })
            .await
            .map_err(|error| anyhow!("executing panicked: {error}"))??;

            tracing::info!(
                ledger,
                cycles,
                "dry run passed: membership executed; no proof made"
            );

            return Ok(Vec::new());
        }

        let membership = match tokio::task::spawn_blocking(move || {
            let native = run_membership_natively(&fixture, &stored_state, router)?;

            prove_membership_on_network(native, &programs)
        })
        .await
        .map_err(|error| anyhow!("proving panicked: {error}"))?
        {
            Ok(membership) => membership,
            Err(error) => {
                self.history
                    .record(&context.failed("membership", &error))
                    .await;

                return Err(error);
            }
        };

        self.history
            .record(&context.proved(0, "membership", &membership))
            .await;

        Ok(multicallCall {
            data: vec![membership_call(item, &membership, consensus_state, ledger).into()],
        }
        .abi_encode())
    }

    async fn relay(&self, request: api::RelayByTxRequest) -> Result<Vec<u8>> {
        let [identifier] = request.source_tx_ids.as_slice() else {
            bail!(
                "relaying {} Stellar transactions at once is not built yet; send one",
                request.source_tx_ids.len()
            );
        };

        if !request.timeout_tx_ids.is_empty() {
            bail!("timeouts are not built yet");
        }

        let hash = transaction_hash(identifier)?;
        let transaction = self.soroban.transaction(&hash).await?;
        let executed = soroban::executed(&transaction.meta)?;
        let events = router_events(&executed, self.router);
        let item = relayable(
            &events,
            &request.src_client_id,
            &request.dst_client_id,
            &request.src_packet_sequences,
        )?;

        tracing::info!(
            transaction = %hash,
            ledger = transaction.ledger,
            sequence = item.packet().sequence,
            acknowledgement = matches!(item, Relayable::Acknowledge(..)),
            "found the packet to relay"
        );

        let mut context = ProofContext {
            src_chain: request.src_chain.clone(),
            dst_chain: request.dst_chain.clone(),
            kind: match item {
                Relayable::Receive(..) => "recv",
                Relayable::Acknowledge(..) => "ack",
            },
            tx_hash: hash.to_string(),
            sequences: vec![item.packet().sequence],
            trusted_height: 0,
            target_height: u64::from(transaction.ledger),
        };

        self.refuse_settled(&item).await?;

        let client = self.evm.client(&request.dst_client_id).await?;
        let client_state = self.evm.client_state(client).await?;

        if client_state.routerContractId.0 != self.router {
            bail!(
                "{} pins router {}, this module relays from {}",
                request.dst_client_id,
                hex::encode(client_state.routerContractId.0),
                self.router_strkey
            );
        }

        let trusted_height = client_state.latestHeight.revisionHeight;

        context.trusted_height = trusted_height;

        if u64::from(transaction.ledger) <= trusted_height {
            return self
                .relay_at_stored_height(
                    client,
                    trusted_height,
                    item,
                    &transaction,
                    &executed,
                    &context,
                )
                .await;
        }

        self.archive.wait_for(transaction.ledger).await?;

        let trusted_ledger =
            u32::try_from(trusted_height).context("the trusted height does not fit a u32")?;
        let trusted_fixture = self.archive.fixture(trusted_ledger).await?;
        let mut fixture = self.archive.fixture(transaction.ledger).await?;

        let trusted_consensus_state = tokio::task::spawn_blocking(move || {
            consensus_state_of(&trusted_fixture.verify_natively(None))
        })
        .await
        .map_err(|error| anyhow!("rebuilding the trusted consensus state failed: {error}"))?;

        let stored = self.evm.consensus_hash(client, trusted_height).await?;

        if consensus_hash_of(&trusted_consensus_state) != stored {
            bail!("the consensus state rebuilt from ledger {trusted_height} does not hash to the one the client stored");
        }

        let path = item.path();
        let siblings = self.siblings(transaction.ledger, &path).await?;
        let value = witness::attach(
            &mut fixture,
            &transaction,
            &executed,
            self.router,
            &path,
            &siblings,
        )?;

        if value != item.expected_value() {
            bail!(
                "the router committed {} at {}, the packet implies {}",
                hex::encode(&value),
                hex::encode(&path),
                hex::encode(item.expected_value())
            );
        }

        let router = self.router;
        let programs = Arc::clone(&self.programs);
        let trusted = Trusted {
            height: trusted_height,
            consensus_state: trusted_consensus_state,
        };

        if self.prover == ProverMode::Execute {
            let executed = tokio::task::spawn_blocking(move || {
                let native = run_natively(&fixture, &trusted, router, now_in_nanoseconds())?;

                execute_on_cpu(native, &programs)
            })
            .await
            .map_err(|error| anyhow!("executing panicked: {error}"))??;

            tracing::info!(
                ledger = transaction.ledger,
                trusted_height,
                update_cycles = executed.update_cycles,
                membership_cycles = executed.membership_cycles,
                "dry run passed: verified natively and both guests executed; no proof made"
            );

            return Ok(Vec::new());
        }

        tracing::info!(
            ledger = transaction.ledger,
            trusted_height,
            "proving on the Succinct network"
        );

        let proofs = match tokio::task::spawn_blocking(move || {
            let native = run_natively(&fixture, &trusted, router, now_in_nanoseconds())?;

            prove_on_network(native, &programs)
        })
        .await
        .map_err(|error| anyhow!("proving panicked: {error}"))?
        {
            Ok(proofs) => proofs,
            Err(error) => {
                self.history
                    .record(&context.failed("update_client and membership", &error))
                    .await;

                return Err(error);
            }
        };

        self.history
            .record(&context.proved(0, "update_client", &proofs.update))
            .await;
        self.history
            .record(&context.proved(1, "membership", &proofs.membership))
            .await;

        let calls = router_calls(
            request.dst_client_id,
            item,
            &proofs,
            u64::from(transaction.ledger),
        );

        Ok(multicallCall {
            data: calls.into_iter().map(Into::into).collect(),
        }
        .abi_encode())
    }
}

#[tonic::async_trait]
impl ProofApiService for StellarToEthService {
    async fn info(
        &self,
        _request: Request<api::InfoRequest>,
    ) -> Result<Response<api::InfoResponse>, Status> {
        Ok(Response::new(api::InfoResponse {
            target_chain: Some(api::Chain {
                chain_id: self.chain_id.to_string(),
                ibc_version: "2".to_string(),
                ibc_contract: self.evm.router().to_string(),
            }),
            source_chain: Some(api::Chain {
                chain_id: self.archive.network_label().to_string(),
                ibc_version: "2".to_string(),
                ibc_contract: self.router_strkey.clone(),
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
            address: self.evm.router().to_string(),
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
            address: String::new(),
        }))
    }

    async fn update_client(
        &self,
        _request: Request<api::UpdateClientRequest>,
    ) -> Result<Response<api::UpdateClientResponse>, Status> {
        Err(Status::unimplemented(
            "stellar_to_eth updates clients only as part of a relay",
        ))
    }
}

#[tonic::async_trait]
impl ProofApiModule for StellarToEthProofApiModule {
    fn name(&self) -> &'static str {
        "stellar_to_eth"
    }

    async fn create_service(
        &self,
        configuration: serde_json::Value,
    ) -> Result<Box<dyn ProofApiService>> {
        let configuration = serde_json::from_value::<StellarToEthConfiguration>(configuration)
            .map_err(|error| {
                anyhow!("failed to parse the stellar_to_eth configuration: {error}")
            })?;

        tracing::info!(router = %configuration.soroban_router, "starting the Stellar to Ethereum proof API module");

        Ok(Box::new(StellarToEthService::new(configuration).await?))
    }
}
