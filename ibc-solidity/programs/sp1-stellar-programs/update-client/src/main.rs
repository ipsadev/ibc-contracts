// SPDX-License-Identifier: Apache-2.0

#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolValue;
use stellar_light_client_update_client::update_client;
use stellar_light_client_update_client::ClientState;
use stellar_light_client_update_client::ConsensusState;
use stellar_light_client_update_client::Header;

pub fn main() {
    let encoded_client_state = sp1_zkvm::io::read_vec();
    let encoded_trusted_consensus_state = sp1_zkvm::io::read_vec();
    let encoded_proposed_header = sp1_zkvm::io::read_vec();
    let encoded_time = sp1_zkvm::io::read_vec();

    let client_state = ClientState::abi_decode(&encoded_client_state).unwrap();
    let trusted_consensus_state =
        ConsensusState::abi_decode(&encoded_trusted_consensus_state).unwrap();
    let proposed_header = Header::abi_decode(&encoded_proposed_header).unwrap();
    let time = u128::from_le_bytes(encoded_time.try_into().unwrap());

    let output = update_client(
        &client_state,
        &trusted_consensus_state,
        &proposed_header,
        time,
    )
    .unwrap();

    sp1_zkvm::io::commit_slice(&output.abi_encode());
}
