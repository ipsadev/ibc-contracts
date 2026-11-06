// SPDX-License-Identifier: Apache-2.0

use stellar_light_client_membership::scenario::proof_publishing;
use stellar_light_client_membership::scenario::Tree;
use stellar_light_client_membership::MembershipError;
use stellar_light_client_membership::MembershipProofEntry;
use stellar_light_client_solidity_types::ClientState;
use stellar_light_client_solidity_types::ConsensusState;
use stellar_light_client_solidity_types::Header;
use stellar_light_client_solidity_types::Height;
use stellar_light_client_solidity_types::KVPair;
use stellar_light_client_solidity_types::SupportedZkAlgorithm;
use stellar_light_client_uc_and_membership::update_client_and_membership;
use stellar_light_client_uc_and_membership::UcAndMembershipError;

const FIXTURE: &str = "../fixtures/mainnet-63907880.json";
const ROUTER_ID: [u8; 32] = [0x5a; 32];
const ROOT_TOPIC: &[u8] = b"ibc_root";
const TIME: u128 = 1_700_000_000_000_000_000;

struct Case {
    client_state: ClientState,
    trusted: ConsensusState,
    header: Header,
}

fn decode(hexed: &str) -> Vec<u8> {
    hex::decode(hexed).expect("bad hex in the fixture")
}

fn load() -> Case {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let raw = std::fs::read_to_string(&path).expect("cannot read the fixture");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("cannot parse the fixture");

    let list = |key: &str| -> Vec<Vec<u8>> {
        json[key]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|one| decode(one.as_str().unwrap()))
            .collect()
    };

    let quorum_sets = list("quorum_sets_xdr");
    let trust_root = quorum_sets.first().expect("no quorum set").clone();

    let fingerprint: [u8; 32] = {
        use sha2::Digest;
        sha2::Sha256::digest(&trust_root).into()
    };

    let slot = json["ledger_seq"].as_u64().unwrap();

    let client_state = ClientState {
        networkId: decode(json["network_id"].as_str().unwrap())
            .as_slice()
            .try_into()
            .map(<[u8; 32]>::into)
            .unwrap(),
        trustRootFingerprint: fingerprint.into(),
        routerContractId: ROUTER_ID.into(),
        rootEventTopic: ROOT_TOPIC.to_vec().into(),
        latestHeight: Height {
            revisionNumber: 0,
            revisionHeight: slot - 1,
        },
        isFrozen: false,
        zkAlgorithm: SupportedZkAlgorithm::Groth16,
    };

    let previous: [u8; 32] = decode(json["previous_ledger_hash"].as_str().unwrap())
        .as_slice()
        .try_into()
        .unwrap();

    let trusted = ConsensusState {
        closeTime: 0,
        ledgerHash: previous.into(),
        previousLedgerHash: [0_u8; 32].into(),
        bucketListHash: [0_u8; 32].into(),
        transactionSetResultHash: [0_u8; 32].into(),
    };

    let header = Header {
        slot,
        ledgerHeaderXdr: decode(json["ledger_header_xdr"].as_str().unwrap()).into(),
        envelopesXdr: list("envelopes_xdr").into_iter().map(Into::into).collect(),
        quorumSetsXdr: quorum_sets.into_iter().map(Into::into).collect(),
        trustRootXdr: trust_root.into(),
        nextEnvelopesXdr: list("next_envelopes_xdr")
            .into_iter()
            .map(Into::into)
            .collect(),
        nextTxSetXdr: decode(json["next_tx_set_xdr"].as_str().unwrap()).into(),
    };

    Case {
        client_state,
        trusted,
        header,
    }
}

fn one_entry(tree: &Tree, key: &[u8], value: &[u8]) -> Vec<MembershipProofEntry> {
    vec![MembershipProofEntry {
        kvPair: KVPair {
            path: vec![key.to_vec().into()],
            value: value.to_vec().into(),
        },
        siblings: tree
            .siblings(key)
            .into_iter()
            .map(<[u8; 32]>::into)
            .collect(),
    }]
}

/// The point of the combined program: the packet is proven against the header verified in the same
/// run. A state root proof built for any other ledger's results cannot bind, which is what stops a
/// prover pairing a real header with results of their own choosing.
#[test]
fn the_packet_is_anchored_on_the_header_proven_in_the_same_run() {
    let case = load();

    let leaves = vec![(
        b"commitments/ports/transfer/channels/0/1".to_vec(),
        b"packet-one".to_vec(),
    )];

    let tree = Tree::build(&leaves);
    let (state_root_proof, _derived) = proof_publishing(tree.root);

    let entries = one_entry(&tree, &leaves[0].0, &leaves[0].1);

    let Err(error) = update_client_and_membership(
        &case.client_state,
        &case.trusted,
        &case.header,
        TIME,
        &state_root_proof,
        &entries,
    ) else {
        panic!("a constructed result set must not bind to a real ledger's txSetResultHash");
    };

    assert!(matches!(
        error,
        UcAndMembershipError::Membership(MembershipError::StateRootNotBound(_))
    ));
}

/// The header is verified first, so a header that does not verify never reaches the membership half
/// and the error says so rather than blaming the packet.
#[test]
fn a_header_that_does_not_verify_fails_before_the_packet_is_looked_at() {
    let mut case = load();

    case.header.nextEnvelopesXdr = Vec::new();

    let leaves = vec![(
        b"commitments/ports/transfer/channels/0/1".to_vec(),
        b"x".to_vec(),
    )];
    let tree = Tree::build(&leaves);
    let (state_root_proof, _) = proof_publishing(tree.root);
    let entries = one_entry(&tree, &leaves[0].0, &leaves[0].1);

    let Err(error) = update_client_and_membership(
        &case.client_state,
        &case.trusted,
        &case.header,
        TIME,
        &state_root_proof,
        &entries,
    ) else {
        panic!("a header with no next-slot evidence must not verify");
    };

    assert!(matches!(error, UcAndMembershipError::UpdateClient(_)));
}

/// A frozen client stops both halves, not just the one that happens to run first.
#[test]
fn a_frozen_client_proves_neither_half() {
    let mut case = load();

    case.client_state.isFrozen = true;

    let leaves = vec![(
        b"commitments/ports/transfer/channels/0/1".to_vec(),
        b"x".to_vec(),
    )];
    let tree = Tree::build(&leaves);
    let (state_root_proof, _) = proof_publishing(tree.root);
    let entries = one_entry(&tree, &leaves[0].0, &leaves[0].1);

    let Err(error) = update_client_and_membership(
        &case.client_state,
        &case.trusted,
        &case.header,
        TIME,
        &state_root_proof,
        &entries,
    ) else {
        panic!("a frozen client must refuse");
    };

    assert!(matches!(error, UcAndMembershipError::UpdateClient(_)));
}
