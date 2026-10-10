// SPDX-License-Identifier: Apache-2.0

use ethereum_light_client::client_state::ClientState;
use ethereum_light_client::consensus_state::ConsensusState;
use sha2::Digest;
use sha2::Sha256;

pub const OUTPUT_LEN: usize = 104;

pub fn output(
    client_state: &ClientState,
    consensus_state: &ConsensusState,
    path: &[Vec<u8>],
    value: &[u8],
) -> [u8; OUTPUT_LEN] {
    let mut out = [0_u8; OUTPUT_LEN];

    out[0..32].copy_from_slice(&client_state_commitment(client_state));
    out[32..64].copy_from_slice(&consensus_hash(consensus_state));
    out[64..72].copy_from_slice(&consensus_state.slot.to_be_bytes());
    out[72..104].copy_from_slice(&kv_commitment(path, value));

    out
}

/// Commits the pair rather than carrying it, so the output stays fixed length.
///
/// The Soroban contract recomputes this from the path and value it was asked about, so a proof of
/// a different pair does not match. An empty value is non-membership, which is what a timeout
/// needs, and it commits differently from a pair whose value happens to be empty bytes because
/// the length is hashed first.
pub fn kv_commitment(path: &[Vec<u8>], value: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update((path.len() as u32).to_be_bytes());

    for segment in path {
        hasher.update((segment.len() as u32).to_be_bytes());
        hasher.update(segment);
    }

    hasher.update((value.len() as u32).to_be_bytes());
    hasher.update(value);

    hasher.finalize().into()
}

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

pub fn consensus_hash(state: &ConsensusState) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(state.slot.to_be_bytes());
    hasher.update(state.state_root.as_slice());
    hasher.update(state.timestamp.to_be_bytes());
    hasher.update(state.current_sync_committee.pubkeys_hash.as_slice());
    hasher.update(state.current_sync_committee.aggregate_pubkey.as_slice());

    match &state.next_sync_committee {
        Some(next) => {
            hasher.update([1_u8]);
            hasher.update(next.pubkeys_hash.as_slice());
            hasher.update(next.aggregate_pubkey.as_slice());
        }
        None => hasher.update([0_u8]),
    }

    hasher.finalize().into()
}
