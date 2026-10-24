// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use stellar_light_client_update_client::update_client;
use stellar_light_client_update_client::ClientState;
use stellar_light_client_update_client::ConsensusState;
use stellar_light_client_update_client::Header;
use stellar_light_client_update_client::Height;
use stellar_light_client_update_client::SupportedZkAlgorithm;
use stellar_light_client_update_client::UpdateClientError;

const VERIFICATION_TIME: u128 = 1_700_000_000_000_000_000;

#[derive(Deserialize)]
struct Fixture {
    ledger_seq: u64,
    network_id: String,
    ledger_hash: String,
    close_time: u64,
    bucket_list_hash: String,
    #[serde(default)]
    previous_ledger_hash: String,
    ledger_header_xdr: String,
    quorum_sets_xdr: Vec<String>,
    envelopes_xdr: Vec<String>,
    #[serde(default)]
    next_envelopes_xdr: Vec<String>,
    #[serde(default)]
    next_tx_set_xdr: String,
}

struct Case {
    fixture: Fixture,
    client_state: ClientState,
    trusted_consensus_state: ConsensusState,
    proposed_header: Header,
}

fn decode(hexed: &str) -> Vec<u8> {
    hex::decode(hexed).expect("bad hex in the fixture")
}

fn decode32(hexed: &str) -> [u8; 32] {
    decode(hexed)
        .as_slice()
        .try_into()
        .expect("expected 32 bytes")
}

const BUCKET_LIST_HASH_OFFSET: usize = 220;
const TX_SET_RESULT_HASH_OFFSET: usize = 188;

fn load(name: &str) -> Case {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name);

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

    let fixture: Fixture = serde_json::from_str(&raw).expect("cannot parse the fixture");

    let trust_root_xdr = decode(&fixture.quorum_sets_xdr[0]);

    let fingerprint: [u8; 32] = Sha256::digest(&trust_root_xdr).into();

    let client_state = ClientState {
        networkId: decode32(&fixture.network_id).into(),
        trustRootFingerprint: fingerprint.into(),
        routerContractId: [0x5a_u8; 32].into(),
        rootEventTopic: b"ibc_root".to_vec().into(),
        latestHeight: Height {
            revisionNumber: 0,
            revisionHeight: fixture.ledger_seq - 1,
        },
        isFrozen: false,
        zkAlgorithm: SupportedZkAlgorithm::Groth16,
    };

    let proposed_header = Header {
        slot: fixture.ledger_seq,
        ledgerHeaderXdr: decode(&fixture.ledger_header_xdr).into(),
        envelopesXdr: fixture
            .envelopes_xdr
            .iter()
            .map(|one| decode(one).into())
            .collect(),
        quorumSetsXdr: fixture
            .quorum_sets_xdr
            .iter()
            .map(|one| decode(one).into())
            .collect(),
        trustRootXdr: trust_root_xdr.into(),
        nextEnvelopesXdr: fixture
            .next_envelopes_xdr
            .iter()
            .map(|one| decode(one).into())
            .collect(),
        nextTxSetXdr: decode(&fixture.next_tx_set_xdr).into(),
    };

    let unbound = ConsensusState {
        closeTime: 0,
        ledgerHash: [0_u8; 32].into(),
        previousLedgerHash: [0_u8; 32].into(),
        bucketListHash: [0_u8; 32].into(),
        transactionSetResultHash: [0_u8; 32].into(),
    };

    let mut case = Case {
        fixture,
        client_state,
        trusted_consensus_state: unbound,
        proposed_header,
    };

    let mut lenient = case.client_state.clone();

    lenient.latestHeight.revisionHeight = case.proposed_header.slot - 2;

    let verified = update_client(
        &lenient,
        &case.trusted_consensus_state,
        &case.proposed_header,
        VERIFICATION_TIME,
    )
    .expect("the fixture does not verify at all");

    case.trusted_consensus_state.ledgerHash = verified.newConsensusState.previousLedgerHash;

    case
}

fn run(
    case: &Case,
) -> Result<stellar_light_client_update_client::UpdateClientOutput, UpdateClientError> {
    update_client(
        &case.client_state,
        &case.trusted_consensus_state,
        &case.proposed_header,
        VERIFICATION_TIME,
    )
}

#[test]
fn mainnet_ledger_verifies_and_matches_the_archive() {
    let case = load("mainnet-63907880.json");

    let output = run(&case).expect("the mainnet fixture should verify");

    assert_eq!(
        output.newConsensusState.ledgerHash.0,
        decode32(&case.fixture.ledger_hash)
    );
    assert_eq!(
        output.newConsensusState.bucketListHash.0,
        decode32(&case.fixture.bucket_list_hash)
    );
    assert_eq!(
        output.newConsensusState.previousLedgerHash.0,
        decode32(&case.fixture.previous_ledger_hash)
    );
    assert_eq!(output.newConsensusState.closeTime, case.fixture.close_time);
    assert_eq!(output.newHeight.revisionHeight, case.fixture.ledger_seq);
    assert_eq!(
        output.trustedHeight.revisionHeight,
        case.fixture.ledger_seq - 1
    );
}

#[test]
fn testnet_ledger_verifies_and_matches_the_archive() {
    let case = load("testnet-4160279.json");

    let output = run(&case).expect("the testnet fixture should verify");

    assert_eq!(
        output.newConsensusState.ledgerHash.0,
        decode32(&case.fixture.ledger_hash)
    );
    assert_eq!(output.newConsensusState.closeTime, case.fixture.close_time);
    assert_eq!(
        output.newConsensusState.bucketListHash.0,
        decode32(&case.fixture.bucket_list_hash)
    );
    assert_eq!(
        output.newConsensusState.previousLedgerHash.0,
        decode32(&case.fixture.previous_ledger_hash)
    );
}

#[test]
fn a_trust_root_the_client_state_does_not_pin_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    case.client_state.trustRootFingerprint = [0x11_u8; 32].into();

    let Err(error) = run(&case) else {
        panic!("an unpinned trust root must not verify");
    };

    assert!(matches!(error, UpdateClientError::TrustRootMismatch { .. }));
}

#[test]
fn a_prover_supplied_quorum_set_cannot_replace_the_pinned_one() {
    let mut case = load("mainnet-63907880.json");

    let substituted = case.proposed_header.quorumSetsXdr.last().unwrap().clone();

    if substituted == case.proposed_header.trustRootXdr {
        return;
    }

    case.proposed_header.trustRootXdr = substituted;

    let Err(error) = run(&case) else {
        panic!("a substituted quorum set must not verify");
    };

    assert!(matches!(error, UpdateClientError::TrustRootMismatch { .. }));
}

#[test]
fn a_frozen_client_refuses_every_update() {
    let mut case = load("mainnet-63907880.json");

    case.client_state.isFrozen = true;

    let Err(error) = run(&case) else {
        panic!("a frozen client must not update");
    };

    assert!(matches!(error, UpdateClientError::ClientFrozen));
}

#[test]
fn a_slot_that_does_not_advance_the_client_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    case.client_state.latestHeight.revisionHeight = case.proposed_header.slot;

    let Err(error) = run(&case) else {
        panic!("a repeated slot must not verify");
    };

    assert!(matches!(
        error,
        UpdateClientError::SlotDoesNotAdvance { .. }
    ));
}

#[test]
fn an_adjacent_header_that_does_not_chain_back_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    case.trusted_consensus_state.ledgerHash = [0x22_u8; 32].into();

    let Err(error) = run(&case) else {
        panic!("a broken chain must not verify");
    };

    assert!(matches!(
        error,
        UpdateClientError::HeaderVerificationFailed(_)
    ));
}

#[test]
fn the_network_id_comes_from_the_client_state_not_the_header() {
    let mut case = load("mainnet-63907880.json");

    case.client_state.networkId = [0x33_u8; 32].into();

    let Err(error) = run(&case) else {
        panic!("a wrong network id must not verify");
    };

    assert!(matches!(
        error,
        UpdateClientError::HeaderVerificationFailed(_)
    ));
}

#[test]
fn a_missing_quorum_set_preimage_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    case.proposed_header.quorumSetsXdr = Vec::new();

    let Err(error) = run(&case) else {
        panic!("a header with no quorum set preimages must not verify");
    };

    assert!(matches!(
        error,
        UpdateClientError::QuorumSetAuthenticationFailed(_)
    ));
}

#[test]
fn a_tampered_quorum_set_preimage_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    let mut tampered = case.proposed_header.quorumSetsXdr[0].to_vec();

    *tampered.last_mut().unwrap() ^= 0x01;
    case.proposed_header.quorumSetsXdr[0] = tampered.into();

    let Err(error) = run(&case) else {
        panic!("a tampered quorum set preimage must not verify");
    };

    assert!(matches!(error, UpdateClientError::MalformedQuorumSet(_)));
}

#[test]
fn the_signers_must_form_a_quorum_under_the_pinned_trust_root() {
    let mut case = load("mainnet-63907880.json");

    case.proposed_header.envelopesXdr.truncate(1);

    let Err(error) = run(&case) else {
        panic!("a single signer must not carry a ledger");
    };

    assert!(matches!(
        error,
        UpdateClientError::QuorumCheckFailed(_) | UpdateClientError::HeaderVerificationFailed(_)
    ));
}

#[test]
fn a_forged_bucket_list_hash_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    let mut header = case.proposed_header.ledgerHeaderXdr.to_vec();

    header[BUCKET_LIST_HASH_OFFSET] ^= 0x01;
    case.proposed_header.ledgerHeaderXdr = header.into();

    let Err(error) = run(&case) else {
        panic!("a forged bucketListHash must not verify: it is what commits to all ledger state");
    };

    assert!(matches!(error, UpdateClientError::NextSlotBindingFailed(_)));
}

#[test]
fn a_forged_transaction_set_result_hash_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    let mut header = case.proposed_header.ledgerHeaderXdr.to_vec();

    header[TX_SET_RESULT_HASH_OFFSET] ^= 0x01;
    case.proposed_header.ledgerHeaderXdr = header.into();

    let Err(error) = run(&case) else {
        panic!("a forged txSetResultHash must not verify: membership proofs anchor on it");
    };

    assert!(matches!(error, UpdateClientError::NextSlotBindingFailed(_)));
}

#[test]
fn a_header_with_no_next_slot_evidence_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    case.proposed_header.nextEnvelopesXdr = Vec::new();

    let Err(error) = run(&case) else {
        panic!("without slot N+1 evidence the header's own hashes are unauthenticated");
    };

    assert!(matches!(
        error,
        UpdateClientError::NextSlotEvidenceMissing(_)
    ));
}

#[test]
fn next_slot_envelopes_from_another_ledger_are_rejected() {
    let mut case = load("mainnet-63907880.json");
    let other = load("testnet-4160279.json");

    case.proposed_header.nextEnvelopesXdr = other.proposed_header.nextEnvelopesXdr;

    let Err(error) = run(&case) else {
        panic!("another ledger's next-slot envelopes must not bind this header");
    };

    assert!(matches!(
        error,
        UpdateClientError::NextSlotAuthenticationFailed { .. }
            | UpdateClientError::MalformedNextEnvelope { .. }
    ));
}

#[test]
fn a_tampered_next_slot_transaction_set_is_rejected() {
    let mut case = load("mainnet-63907880.json");

    let mut tx_set = case.proposed_header.nextTxSetXdr.to_vec();

    *tx_set.last_mut().unwrap() ^= 0x01;
    case.proposed_header.nextTxSetXdr = tx_set.into();

    let Err(error) = run(&case) else {
        panic!("the next slot's tx set must hash to the txSetHash its SCP value names");
    };

    assert!(matches!(error, UpdateClientError::NextSlotBindingFailed(_)));
}
