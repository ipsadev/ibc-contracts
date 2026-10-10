use std::path::PathBuf;

use alloy_primitives::{Bytes, B256};
use ethereum_light_client::{
    client_state::ClientState,
    consensus_state::ConsensusState,
    error::EthereumIBCError,
    execution::execution_data,
    header::{ActiveSyncCommittee, Header},
    sync_protocol_helpers::{
        current_sync_committee_gindex_at_slot, finalized_root_gindex_at_slot,
        is_valid_light_client_header, next_sync_committee_gindex_at_slot, normalize_merkle_branch,
    },
    test_utils::bls_verifier::{aggreagate, fast_aggregate_verify, BlsError},
    trie::is_valid_normalized_merkle_branch,
    update::update_consensus_state,
    verify::{verify_header, BlsVerify},
};
use ethereum_types::consensus::{
    bls::{BlsPublicKey, BlsSignature},
    bootstrap::LightClientBootstrap,
    genesis::Genesis,
    light_client_header::{
        ElectraLightClientHeader, LightClientFinalityUpdate, LightClientHeader, LightClientUpdate,
    },
    merkle::{
        CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS, FINALIZED_ROOT_GINDEX_GLOAS,
        NEXT_SYNC_COMMITTEE_GINDEX_GLOAS,
    },
    spec::Spec,
};
use serde::de::DeserializeOwned;
use tree_hash::TreeHash;

struct Verifier;

impl BlsVerify for Verifier {
    type Error = BlsError;

    fn fast_aggregate_verify(
        &self,
        public_keys: &[BlsPublicKey],
        msg: B256,
        signature: BlsSignature,
    ) -> Result<(), BlsError> {
        fast_aggregate_verify(public_keys, msg, signature)
    }

    fn aggregate(&self, public_keys: &[BlsPublicKey]) -> Result<BlsPublicKey, BlsError> {
        aggreagate(public_keys)
    }
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/gloas")
        .join(name)
}

fn data<T: DeserializeOwned>(name: &str) -> T {
    let raw = std::fs::read(fixture_path(name)).unwrap();
    let mut response: serde_json::Value = serde_json::from_slice(&raw).unwrap();

    serde_json::from_value(response["data"].take()).unwrap()
}

fn period_updates() -> Vec<LightClientUpdate> {
    let raw = std::fs::read(fixture_path("updates.json")).unwrap();
    let responses: Vec<serde_json::Value> = serde_json::from_slice(&raw).unwrap();

    responses
        .into_iter()
        .map(|mut response| serde_json::from_value(response["data"].take()).unwrap())
        .collect()
}

fn execution_header(name: &str) -> Bytes {
    let hex = std::fs::read_to_string(fixture_path(name)).unwrap();

    hex.trim().parse().unwrap()
}

fn spec() -> Spec {
    let raw = std::fs::read(fixture_path("spec.json")).unwrap();
    let mut response: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let mut spec = response["data"].take();

    if spec.get("GENESIS_SLOT").is_none() {
        spec["GENESIS_SLOT"] = "0".into();
    }

    serde_json::from_value(spec).unwrap()
}

fn client_state(latest_slot: u64) -> ClientState {
    let spec = spec();
    let genesis: Genesis = data("genesis.json");

    ClientState {
        chain_id: 11_155_111,
        genesis_validators_root: genesis.genesis_validators_root,
        min_sync_committee_participants: spec.sync_committee_size * 2 / 3,
        sync_committee_size: spec.sync_committee_size,
        genesis_time: genesis.genesis_time,
        genesis_slot: spec.genesis_slot,
        fork_parameters: spec.to_fork_parameters(),
        seconds_per_slot: spec.seconds_per_slot,
        slots_per_epoch: spec.slots_per_epoch,
        epochs_per_sync_committee_period: spec.epochs_per_sync_committee_period,
        latest_slot,
        ..Default::default()
    }
}

struct Scenario {
    client_state: ClientState,
    trusted: ConsensusState,
    header: Header,
    current_timestamp: u64,
}

fn scenario() -> Scenario {
    let bootstrap: LightClientBootstrap = data("bootstrap.json");
    let finality_update: LightClientFinalityUpdate = data("finality-update.json");
    let next_committee = period_updates()[1]
        .next_sync_committee
        .clone()
        .unwrap()
        .to_summarized_sync_committee();

    let trusted_slot = bootstrap.header.beacon().slot;
    let client_state = client_state(trusted_slot);

    let trusted_execution = execution_data(
        &bootstrap.header,
        Some(&execution_header("execution-header-bootstrap.hex")),
    )
    .unwrap();

    let trusted = ConsensusState {
        slot: trusted_slot,
        state_root: trusted_execution.state_root,
        timestamp: trusted_execution.timestamp,
        current_sync_committee: bootstrap
            .current_sync_committee
            .to_summarized_sync_committee(),
        next_sync_committee: Some(next_committee),
    };

    let signature_slot = finality_update.signature_slot;
    let current_timestamp = client_state.genesis_time
        + (signature_slot - client_state.genesis_slot + 1) * client_state.seconds_per_slot;

    let header = Header {
        active_sync_committee: ActiveSyncCommittee::Current(bootstrap.current_sync_committee),
        consensus_update: finality_update.into(),
        trusted_slot,
        execution_header: Some(execution_header("execution-header-finalized.hex")),
    };

    Scenario {
        client_state,
        trusted,
        header,
        current_timestamp,
    }
}

#[test]
fn gloas_light_client_data_deserializes_in_the_gloas_shape() {
    let bootstrap: LightClientBootstrap = data("bootstrap.json");
    let finality_update: LightClientFinalityUpdate = data("finality-update.json");

    assert!(matches!(bootstrap.header, LightClientHeader::Gloas(_)));
    assert_eq!(bootstrap.current_sync_committee_branch.as_slice().len(), 11);
    assert!(matches!(
        finality_update.finalized_header,
        LightClientHeader::Gloas(_)
    ));
    assert_eq!(finality_update.finality_branch.as_slice().len(), 9);

    for update in period_updates() {
        assert!(matches!(
            update.attested_header,
            LightClientHeader::Gloas(_)
        ));
        assert_eq!(
            update.next_sync_committee_branch.unwrap().as_slice().len(),
            11
        );
    }
}

#[test]
fn a_gloas_finality_update_verifies_and_moves_the_consensus_state() {
    let scenario = scenario();

    verify_header(
        &scenario.trusted,
        &scenario.client_state,
        scenario.current_timestamp,
        &scenario.header,
        Verifier,
    )
    .unwrap();

    let expected = execution_data(
        &scenario.header.consensus_update.finalized_header,
        scenario.header.execution_header.as_ref(),
    )
    .unwrap();

    let (slot, updated, client_state) = update_consensus_state(
        scenario.trusted.clone(),
        scenario.client_state.clone(),
        scenario.header.clone(),
    )
    .unwrap();

    assert_eq!(
        slot,
        scenario
            .header
            .consensus_update
            .finalized_header
            .beacon()
            .slot
    );
    assert_eq!(updated.state_root, expected.state_root);
    assert_eq!(updated.timestamp, expected.timestamp);
    assert!(updated.timestamp >= scenario.trusted.timestamp);
    assert_eq!(
        client_state.unwrap().latest_execution_block_number,
        expected.block_number
    );
}

#[test]
fn a_gloas_finality_update_signed_by_the_wrong_committee_is_refused() {
    let mut scenario = scenario();
    let next_period_committee = period_updates()[1].next_sync_committee.clone().unwrap();

    scenario.header.active_sync_committee = ActiveSyncCommittee::Current(next_period_committee);

    assert!(verify_header(
        &scenario.trusted,
        &scenario.client_state,
        scenario.current_timestamp,
        &scenario.header,
        Verifier,
    )
    .is_err());
}

#[test]
fn branches_verify_at_the_gloas_gindices() {
    let bootstrap: LightClientBootstrap = data("bootstrap.json");
    let update = period_updates().pop().unwrap();
    let client_state = client_state(bootstrap.header.beacon().slot);
    let attested_slot = update.attested_header.beacon().slot;

    let next_gindex = next_sync_committee_gindex_at_slot(&client_state, attested_slot).unwrap();
    let finality_gindex = finalized_root_gindex_at_slot(&client_state, attested_slot).unwrap();
    let current_gindex =
        current_sync_committee_gindex_at_slot(&client_state, bootstrap.header.beacon().slot)
            .unwrap();

    assert_eq!(next_gindex, NEXT_SYNC_COMMITTEE_GINDEX_GLOAS);
    assert_eq!(finality_gindex, FINALIZED_ROOT_GINDEX_GLOAS);
    assert_eq!(current_gindex, CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS);

    is_valid_normalized_merkle_branch(
        update.next_sync_committee.unwrap().tree_hash_root(),
        &normalize_merkle_branch(
            update.next_sync_committee_branch.unwrap().as_slice(),
            next_gindex,
        )
        .unwrap(),
        next_gindex,
        update.attested_header.beacon().state_root,
    )
    .unwrap();

    is_valid_normalized_merkle_branch(
        update.finalized_header.beacon().tree_hash_root(),
        &normalize_merkle_branch(update.finality_branch.as_slice(), finality_gindex).unwrap(),
        finality_gindex,
        update.attested_header.beacon().state_root,
    )
    .unwrap();

    is_valid_normalized_merkle_branch(
        bootstrap.current_sync_committee.tree_hash_root(),
        &normalize_merkle_branch(
            bootstrap.current_sync_committee_branch.as_slice(),
            current_gindex,
        )
        .unwrap(),
        current_gindex,
        bootstrap.header.beacon().state_root,
    )
    .unwrap();

    is_valid_light_client_header(&client_state, &update.finalized_header).unwrap();
    is_valid_light_client_header(&client_state, &update.attested_header).unwrap();
}

#[test]
fn the_execution_header_witness_yields_the_block_the_rpc_reports() {
    let scenario = scenario();
    let raw = std::fs::read(fixture_path("execution-block-finalized.json")).unwrap();
    let block: serde_json::Value = serde_json::from_slice(&raw).unwrap();

    let execution = execution_data(
        &scenario.header.consensus_update.finalized_header,
        scenario.header.execution_header.as_ref(),
    )
    .unwrap();

    assert_eq!(
        execution.state_root,
        block["state_root"]
            .as_str()
            .unwrap()
            .parse::<B256>()
            .unwrap()
    );
    assert_eq!(execution.block_number, block["number"].as_u64().unwrap());
    assert_eq!(execution.timestamp, block["timestamp"].as_u64().unwrap());
}

#[test]
fn a_gloas_header_without_the_execution_header_is_refused() {
    let scenario = scenario();

    assert_eq!(
        execution_data(&scenario.header.consensus_update.finalized_header, None),
        Err(EthereumIBCError::MissingExecutionHeader)
    );
}

#[test]
fn an_execution_header_for_another_block_is_refused() {
    let scenario = scenario();
    let other = execution_header("execution-header-bootstrap.hex");

    assert!(matches!(
        execution_data(
            &scenario.header.consensus_update.finalized_header,
            Some(&other)
        ),
        Err(EthereumIBCError::ExecutionHeaderHashMismatch { .. })
    ));
}

#[test]
fn a_tampered_state_root_is_refused() {
    let scenario = scenario();
    let mut tampered = scenario.header.execution_header.clone().unwrap().to_vec();
    let state_root_byte = 3 + 33 + 33 + 21 + 1;

    tampered[state_root_byte] ^= 0x01;

    assert!(matches!(
        execution_data(
            &scenario.header.consensus_update.finalized_header,
            Some(&Bytes::from(tampered))
        ),
        Err(EthereumIBCError::ExecutionHeaderHashMismatch { .. })
    ));
}

#[test]
fn a_tampered_execution_block_hash_fails_the_header_branch() {
    let scenario = scenario();
    let mut header = scenario.header.consensus_update.finalized_header.clone();

    if let LightClientHeader::Gloas(gloas) = &mut header {
        gloas.execution_block_hash.0[0] ^= 0x01;
    }

    assert!(is_valid_light_client_header(&scenario.client_state, &header).is_err());
}

#[test]
fn an_electra_shaped_header_at_a_gloas_slot_is_refused() {
    let scenario = scenario();
    let electra = ElectraLightClientHeader {
        beacon: scenario
            .header
            .consensus_update
            .finalized_header
            .beacon()
            .clone(),
        ..Default::default()
    };

    let epoch = scenario
        .client_state
        .compute_epoch_at_slot(electra.beacon.slot);

    assert_eq!(
        is_valid_light_client_header(&scenario.client_state, &LightClientHeader::Electra(electra)),
        Err(EthereumIBCError::LightClientHeaderForkMismatch { epoch })
    );
}
