// SPDX-License-Identifier: Apache-2.0

#![no_main]
sp1_zkvm::entrypoint!(main);

mod bls;
mod wire;

use bls::Bls12381Verifier;
use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use ethereum_light_client::header::Header;
use ethereum_light_client::update::update_consensus_state;
use ethereum_light_client::verify::verify_header;

pub fn main() {
    let client_state: ClientState = serde_json::from_slice(&sp1_zkvm::io::read_vec()).unwrap();
    let consensus_state: ConsensusState = serde_json::from_slice(&sp1_zkvm::io::read_vec()).unwrap();
    let header: Header = serde_json::from_slice(&sp1_zkvm::io::read_vec()).unwrap();

    let timestamp = u64::from_le_bytes(sp1_zkvm::io::read_vec().try_into().unwrap());

    verify_header(
        &consensus_state,
        &client_state,
        timestamp,
        &header,
        Bls12381Verifier,
    )
    .unwrap();

    let (_, new_consensus_state, _) =
        update_consensus_state(consensus_state.clone(), client_state.clone(), header).unwrap();

    sp1_zkvm::io::commit_slice(&wire::output(
        &client_state,
        &consensus_state,
        &new_consensus_state,
    ));
}
