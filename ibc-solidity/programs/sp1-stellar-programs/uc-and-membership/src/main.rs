// SPDX-License-Identifier: Apache-2.0

#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolValue;
use stellar_light_client_solidity_types::ClientState;
use stellar_light_client_solidity_types::ConsensusState;
use stellar_light_client_solidity_types::Header;
use stellar_light_client_solidity_types::MembershipProofEntry;
use stellar_light_client_solidity_types::StateRootProof;
use stellar_light_client_uc_and_membership::update_client_and_membership;

pub fn main() {
    let encoded_client_state = sp1_zkvm::io::read_vec();
    let encoded_trusted = sp1_zkvm::io::read_vec();
    let encoded_header = sp1_zkvm::io::read_vec();
    let encoded_time = sp1_zkvm::io::read_vec();
    let encoded_state_root_proof = sp1_zkvm::io::read_vec();
    let encoded_count = sp1_zkvm::io::read_vec();

    let client_state = ClientState::abi_decode(&encoded_client_state).unwrap();
    let trusted_consensus_state = ConsensusState::abi_decode(&encoded_trusted).unwrap();
    let proposed_header = Header::abi_decode(&encoded_header).unwrap();
    let time = u128::from_le_bytes(encoded_time.try_into().unwrap());
    let state_root_proof = StateRootProof::abi_decode(&encoded_state_root_proof).unwrap();
    let count = u16::from_le_bytes(encoded_count.try_into().unwrap());

    assert!(count != 0);

    let entries: Vec<MembershipProofEntry> = (0..count)
        .map(|_| MembershipProofEntry::abi_decode(&sp1_zkvm::io::read_vec()).unwrap())
        .collect();

    let output = update_client_and_membership(
        &client_state,
        &trusted_consensus_state,
        &proposed_header,
        time,
        &state_root_proof,
        &entries,
    )
    .unwrap();

    sp1_zkvm::io::commit_slice(&output.abi_encode());
}
