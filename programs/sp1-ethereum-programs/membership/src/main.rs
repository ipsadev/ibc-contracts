// SPDX-License-Identifier: Apache-2.0

#![no_main]
sp1_zkvm::entrypoint!(main);

mod wire;

use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use ethereum_light_client::membership::verify_membership;
use ethereum_light_client::membership::verify_non_membership;

pub fn main() {
    let client_state: ClientState = serde_json::from_slice(&sp1_zkvm::io::read_vec()).unwrap();
    let consensus_state: ConsensusState = serde_json::from_slice(&sp1_zkvm::io::read_vec()).unwrap();

    let proof = sp1_zkvm::io::read_vec();
    let path: Vec<Vec<u8>> = serde_json::from_slice(&sp1_zkvm::io::read_vec()).unwrap();
    let value = sp1_zkvm::io::read_vec();

    if value.is_empty() {
        verify_non_membership(
            consensus_state.clone(),
            client_state.clone(),
            proof,
            path.clone(),
        )
        .unwrap();
    } else {
        verify_membership(
            consensus_state.clone(),
            client_state.clone(),
            proof,
            path.clone(),
            value.clone(),
        )
        .unwrap();
    }

    sp1_zkvm::io::commit_slice(&wire::output(
        &client_state,
        &consensus_state,
        &path,
        &value,
    ));
}
