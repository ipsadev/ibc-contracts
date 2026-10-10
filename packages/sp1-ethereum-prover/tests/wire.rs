use std::path::PathBuf;

use ethereum_light_client::{client_state::ClientState, consensus_state::ConsensusState};
use sp1_ethereum_prover::update_wire::{
    client_state_commitment, consensus_hash, CLIENT_STATE_COMMITMENT, NEW_CONSENSUS_HASH,
    TRUSTED_CONSENSUS_HASH, TRUSTED_SLOT,
};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sepolia-11306528")
        .join(name);

    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn committed() -> Vec<u8> {
    let hexed = fixture("public-values.hex");

    hex::decode(hexed.trim().trim_start_matches("0x")).expect("hex")
}

#[test]
fn the_host_hashes_what_the_guest_committed_for_a_real_gloas_sepolia_update() {
    let client_state: ClientState =
        serde_json::from_str(&fixture("client-state.json")).expect("client state");
    let trusted: ConsensusState =
        serde_json::from_str(&fixture("consensus-state.json")).expect("trusted");
    let updated: ConsensusState =
        serde_json::from_str(&fixture("consensus-state-updated.json")).expect("updated");
    let values = committed();

    assert_eq!(
        client_state_commitment(&client_state).as_slice(),
        &values[CLIENT_STATE_COMMITMENT..TRUSTED_CONSENSUS_HASH]
    );
    assert_eq!(
        consensus_hash(&trusted).as_slice(),
        &values[TRUSTED_CONSENSUS_HASH..NEW_CONSENSUS_HASH]
    );
    assert_eq!(
        consensus_hash(&updated).as_slice(),
        &values[NEW_CONSENSUS_HASH..TRUSTED_SLOT]
    );
}

#[test]
fn a_known_next_committee_changes_the_hash() {
    let mut trusted: ConsensusState =
        serde_json::from_str(&fixture("consensus-state.json")).expect("trusted");
    let without = consensus_hash(&trusted);

    trusted.next_sync_committee = Some(trusted.current_sync_committee.clone());

    assert_ne!(consensus_hash(&trusted), without);
}
