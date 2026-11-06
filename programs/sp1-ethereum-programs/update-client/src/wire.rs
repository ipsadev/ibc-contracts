// SPDX-License-Identifier: Apache-2.0

use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use ethereum_types::consensus::sync_committee::SummarizedSyncCommittee;
use sha2::Digest;
use sha2::Sha256;

pub const OUTPUT_LEN: usize = 120;

pub const CLIENT_STATE_COMMITMENT: usize = 0;
pub const TRUSTED_CONSENSUS_HASH: usize = 32;
pub const NEW_CONSENSUS_HASH: usize = 64;
pub const TRUSTED_SLOT: usize = 96;
pub const NEW_SLOT: usize = 104;
pub const TIMESTAMP: usize = 112;

pub fn output(
    client_state: &ClientState,
    trusted: &ConsensusState,
    new: &ConsensusState,
) -> [u8; OUTPUT_LEN] {
    let mut out = [0_u8; OUTPUT_LEN];

    out[CLIENT_STATE_COMMITMENT..TRUSTED_CONSENSUS_HASH]
        .copy_from_slice(&client_state_commitment(client_state));
    out[TRUSTED_CONSENSUS_HASH..NEW_CONSENSUS_HASH].copy_from_slice(&consensus_hash(trusted));
    out[NEW_CONSENSUS_HASH..TRUSTED_SLOT].copy_from_slice(&consensus_hash(new));
    out[TRUSTED_SLOT..NEW_SLOT].copy_from_slice(&trusted.slot.to_be_bytes());
    out[NEW_SLOT..TIMESTAMP].copy_from_slice(&new.slot.to_be_bytes());
    out[TIMESTAMP..OUTPUT_LEN].copy_from_slice(&new.timestamp.to_be_bytes());

    out
}

/// The parameters the Soroban contract pins, in a fixed order.
///
/// Only the values a prover must not be free to choose are included: the chain, its timing, the
/// tracked contract, the signer threshold and the fork schedule, which selects the gindices. `latest_slot` and
/// `latest_execution_block_number` move on every update, so a proof echoing an older one is normal
/// and must not be rejected — the same reasoning as `SP1ICSStellar._requireOurClientState`.
pub fn client_state_commitment(client_state: &ClientState) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(client_state.chain_id.to_be_bytes());
    hasher.update(client_state.genesis_validators_root.as_slice());
    hasher.update(client_state.genesis_time.to_be_bytes());
    hasher.update(client_state.genesis_slot.to_be_bytes());
    hasher.update(client_state.seconds_per_slot.to_be_bytes());
    hasher.update(client_state.slots_per_epoch.to_be_bytes());
    hasher.update(client_state.epochs_per_sync_committee_period.to_be_bytes());
    hasher.update(client_state.ibc_contract_address.as_slice());
    hasher.update(client_state.ibc_commitment_slot.to_be_bytes::<32>());
    hasher.update(client_state.min_sync_committee_participants.to_be_bytes());
    hasher.update(client_state.sync_committee_size.to_be_bytes());

    let forks = &client_state.fork_parameters;

    hasher.update(forks.genesis_fork_version.as_slice());
    hasher.update(forks.genesis_slot.to_be_bytes());

    for fork in [
        &forks.altair,
        &forks.bellatrix,
        &forks.capella,
        &forks.deneb,
        &forks.electra,
        &forks.fulu,
        &forks.gloas,
    ] {
        hasher.update(fork.version.as_slice());
        hasher.update(fork.epoch.to_be_bytes());
    }

    hasher.finalize().into()
}

/// Hashed field by field rather than over a serialisation, so the Soroban side can recompute it
/// without a serde implementation and without the two sides having to agree on a JSON encoding.
pub fn consensus_hash(state: &ConsensusState) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(state.slot.to_be_bytes());
    hasher.update(state.state_root.as_slice());
    hasher.update(state.timestamp.to_be_bytes());
    hasher.update(committee_bytes(&state.current_sync_committee));

    match &state.next_sync_committee {
        Some(next) => {
            hasher.update([1_u8]);
            hasher.update(committee_bytes(next));
        }
        None => hasher.update([0_u8]),
    }

    hasher.finalize().into()
}

fn committee_bytes(committee: &SummarizedSyncCommittee) -> [u8; 80] {
    let mut out = [0_u8; 80];

    out[..32].copy_from_slice(committee.pubkeys_hash.as_slice());
    out[32..].copy_from_slice(committee.aggregate_pubkey.as_slice());

    out
}
