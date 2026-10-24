// SPDX-License-Identifier: Apache-2.0

use stellar_light_client_membership::membership;
use stellar_light_client_membership::scenario::client_state;
use stellar_light_client_membership::scenario::contract_event;
use stellar_light_client_membership::scenario::entry;
use stellar_light_client_membership::scenario::proof_publishing;
use stellar_light_client_membership::scenario::scval_bytes;
use stellar_light_client_membership::scenario::state_root_proof_for;
use stellar_light_client_membership::scenario::Tree;
use stellar_light_client_membership::scenario::ROOT_TOPIC;
use stellar_light_client_membership::scenario::ROUTER_ID;
use stellar_light_client_membership::MembershipError;

fn leaves() -> Vec<(Vec<u8>, Vec<u8>)> {
    vec![
        (
            b"commitments/ports/transfer/channels/0/1".to_vec(),
            b"packet-one".to_vec(),
        ),
        (
            b"commitments/ports/transfer/channels/0/2".to_vec(),
            b"packet-two".to_vec(),
        ),
        (
            b"receipts/ports/transfer/channels/0/7".to_vec(),
            b"ok".to_vec(),
        ),
    ]
}

#[test]
fn a_packet_commitment_in_the_routers_tree_is_proven() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let output = membership(&client_state(), result_hash, &proof, &entries)
        .expect("a real commitment should verify");

    assert_eq!(output.ibcRoot.0, tree.root);
    assert_eq!(output.transactionSetResultHash.0, result_hash);
    assert_eq!(output.kvPairs.len(), 1);
    assert_eq!(output.clientState.routerContractId.0, ROUTER_ID);
}

#[test]
fn several_pairs_are_proven_in_one_pass() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let entries = vec![
        entry(
            &tree,
            b"commitments/ports/transfer/channels/0/1",
            b"packet-one",
        ),
        entry(
            &tree,
            b"commitments/ports/transfer/channels/0/2",
            b"packet-two",
        ),
        entry(&tree, b"receipts/ports/transfer/channels/0/7", b"ok"),
    ];

    let output = membership(&client_state(), result_hash, &proof, &entries)
        .expect("every pair should verify");

    assert_eq!(output.kvPairs.len(), 3);
}

#[test]
fn an_absent_key_is_proven_absent_which_is_what_a_timeout_needs() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let entries = vec![entry(&tree, b"receipts/ports/transfer/channels/0/999", b"")];

    let output = membership(&client_state(), result_hash, &proof, &entries)
        .expect("an absent key should prove absent");

    assert_eq!(output.ibcRoot.0, tree.root);
}

#[test]
fn a_key_that_is_present_cannot_be_proven_absent() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let mut entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    entries[0].kvPair.value = Vec::new().into();

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("a present key must not prove absent");
    };

    assert!(matches!(error, MembershipError::NotAbsent { .. }));
}

#[test]
fn a_value_that_is_not_in_the_tree_is_rejected() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"tampered",
    )];

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("a substituted value must not verify");
    };

    assert!(matches!(error, MembershipError::NotAMember { .. }));
}

#[test]
fn a_router_the_client_state_does_not_pin_is_rejected() {
    let tree = Tree::build(&leaves());

    let impostor = [0x77_u8; 32];

    let (proof, result_hash) = state_root_proof_for(&[contract_event(
        &impostor,
        ROOT_TOPIC,
        &scval_bytes(&tree.root),
    )]);

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("another contract's event must not be believed");
    };

    assert!(matches!(error, MembershipError::StateRootNotBound(_)));
}

#[test]
fn an_event_topic_the_client_state_does_not_pin_is_rejected() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = state_root_proof_for(&[contract_event(
        &ROUTER_ID,
        b"not_the_root",
        &scval_bytes(&tree.root),
    )]);

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("another topic must not be believed");
    };

    assert!(matches!(error, MembershipError::StateRootNotBound(_)));
}

#[test]
fn results_that_do_not_belong_to_the_ledger_are_rejected() {
    let tree = Tree::build(&leaves());

    let (proof, _) = proof_publishing(tree.root);

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let Err(error) = membership(&client_state(), [0xab_u8; 32], &proof, &entries) else {
        panic!("results must be bound to the ledger's result hash");
    };

    assert!(matches!(error, MembershipError::StateRootNotBound(_)));
}

#[test]
fn a_tampered_result_pair_is_rejected() {
    let tree = Tree::build(&leaves());

    let (mut proof, result_hash) = proof_publishing(tree.root);

    let mut pair = proof.resultPairs[0].to_vec();

    *pair.last_mut().unwrap() ^= 0x01;
    proof.resultPairs[0] = pair.into();

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("a tampered result must not verify");
    };

    assert!(matches!(error, MembershipError::StateRootNotBound(_)));
}

#[test]
fn a_result_index_outside_the_set_is_rejected() {
    let tree = Tree::build(&leaves());

    let (mut proof, result_hash) = proof_publishing(tree.root);

    proof.resultIndex = 9;

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("an out of range index must not verify");
    };

    assert!(matches!(error, MembershipError::StateRootNotBound(_)));
}

#[test]
fn a_proof_of_the_wrong_depth_is_rejected() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let mut entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    entries[0].siblings.truncate(63);

    let Err(error) = membership(&client_state(), result_hash, &proof, &entries) else {
        panic!("a short proof must not verify");
    };

    assert!(matches!(
        error,
        MembershipError::WrongProofDepth { got: 63, .. }
    ));
}

#[test]
fn a_frozen_client_proves_nothing() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let mut state = client_state();

    state.isFrozen = true;

    let entries = vec![entry(
        &tree,
        b"commitments/ports/transfer/channels/0/1",
        b"packet-one",
    )];

    let Err(error) = membership(&state, result_hash, &proof, &entries) else {
        panic!("a frozen client must prove nothing");
    };

    assert!(matches!(error, MembershipError::ClientFrozen));
}

#[test]
fn an_empty_request_is_rejected() {
    let tree = Tree::build(&leaves());

    let (proof, result_hash) = proof_publishing(tree.root);

    let Err(error) = membership(&client_state(), result_hash, &proof, &[]) else {
        panic!("proving nothing must not succeed");
    };

    assert!(matches!(error, MembershipError::NothingToProve));
}
