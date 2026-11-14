// SPDX-License-Identifier: Apache-2.0

use stellar_light_client_membership::membership;
use stellar_light_client_membership::MembershipError;
use stellar_light_client_membership::MembershipProofEntry;
use stellar_light_client_solidity_types::ClientState;
use stellar_light_client_solidity_types::ConsensusState;
use stellar_light_client_solidity_types::Header;
use stellar_light_client_solidity_types::StateRootProof;
use stellar_light_client_solidity_types::UcAndMembershipOutput;
use stellar_light_client_update_client::update_client;
use stellar_light_client_update_client::UpdateClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UcAndMembershipError {
    #[error("the header does not verify: {0}")]
    UpdateClient(#[from] UpdateClientError),

    #[error("the packet does not verify against the header just proven: {0}")]
    Membership(#[from] MembershipError),
}

pub fn update_client_and_membership(
    client_state: &ClientState,
    trusted_consensus_state: &ConsensusState,
    proposed_header: &Header,
    time: u128,
    state_root_proof: &StateRootProof,
    entries: &[MembershipProofEntry],
) -> Result<UcAndMembershipOutput, UcAndMembershipError> {
    let update = update_client(client_state, trusted_consensus_state, proposed_header, time)?;

    let proven = membership(
        client_state,
        update.newConsensusState.transactionSetResultHash.0,
        state_root_proof,
        entries,
    )?;

    Ok(UcAndMembershipOutput {
        updateClientOutput: update,
        ibcRoot: proven.ibcRoot,
        kvPairs: proven.kvPairs,
    })
}
