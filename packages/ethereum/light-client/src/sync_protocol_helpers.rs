// SPDX-License-Identifier: Apache-2.0

//! This module implements the sync protocol helpers defined in [consensus-specs](https://github.com/ethereum/consensus-specs)

use alloy_primitives::B256;
use ethereum_types::consensus::{
    light_client_header::LightClientHeader,
    merkle::{
        floorlog2, CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA, CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS,
        EXECUTION_BLOCK_HASH_GINDEX_DENEB, EXECUTION_BLOCK_HASH_GINDEX_GLOAS,
        EXECUTION_PAYLOAD_GINDEX, FINALIZED_ROOT_GINDEX_ELECTRA, FINALIZED_ROOT_GINDEX_GLOAS,
        NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA, NEXT_SYNC_COMMITTEE_GINDEX_GLOAS,
    },
};
use tree_hash::TreeHash;

use crate::{
    client_state::ClientState,
    error::EthereumIBCError,
    trie::{is_valid_normalized_merkle_branch, validate_merkle_branch},
};

// See spec: https://github.com/ethereum/consensus-specs/blob/dev/specs/electra/light-client/sync-protocol.md#modified-finalized_root_gindex_at_slot
/// Returns the finalized root gindex at the given slot.
/// # Errors
/// Returns an error if the epoch is in a non-supported fork.
pub fn finalized_root_gindex_at_slot(
    client_state: &ClientState,
    slot: u64,
) -> Result<u64, EthereumIBCError> {
    let epoch = client_state.compute_epoch_at_slot(slot);
    client_state.verify_supported_fork_at_epoch(epoch)?;

    if epoch >= client_state.fork_parameters.gloas.epoch {
        return Ok(FINALIZED_ROOT_GINDEX_GLOAS);
    }

    Ok(FINALIZED_ROOT_GINDEX_ELECTRA)
}

// See spec: https://github.com/ethereum/consensus-specs/blob/dev/specs/electra/light-client/sync-protocol.md#modified-current_sync_committee_gindex_at_slot
/// Returns the current sync committee gindex at the given slot.
/// # Errors
/// Returns an error if the epoch is in a non-supported fork.
pub fn current_sync_committee_gindex_at_slot(
    client_state: &ClientState,
    slot: u64,
) -> Result<u64, EthereumIBCError> {
    let epoch = client_state.compute_epoch_at_slot(slot);
    client_state.verify_supported_fork_at_epoch(epoch)?;

    if epoch >= client_state.fork_parameters.gloas.epoch {
        return Ok(CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS);
    }

    Ok(CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA)
}

// See spec: https://github.com/ethereum/consensus-specs/blob/dev/specs/electra/light-client/sync-protocol.md#modified-next_sync_committee_gindex_at_slot
/// Returns the next sync committee gindex at the given slot.
/// # Errors
/// Returns an error if the epoch is in a non-supported fork.
pub fn next_sync_committee_gindex_at_slot(
    client_state: &ClientState,
    slot: u64,
) -> Result<u64, EthereumIBCError> {
    let epoch = client_state.compute_epoch_at_slot(slot);
    client_state.verify_supported_fork_at_epoch(epoch)?;

    if epoch >= client_state.fork_parameters.gloas.epoch {
        return Ok(NEXT_SYNC_COMMITTEE_GINDEX_GLOAS);
    }

    Ok(NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA)
}

// See spec: https://github.com/ethereum/consensus-specs/blob/master/specs/gloas/light-client/sync-protocol.md#modified-get_lc_execution_root
/// Returns the execution root of the light client header: the execution payload header root
/// before gloas, the execution block hash from gloas on.
/// # Errors
/// Returns an error if the epoch is in a non-supported fork, or the header is not in the shape
/// of its fork.
pub fn get_lc_execution_root(
    client_state: &ClientState,
    header: &LightClientHeader,
) -> Result<B256, EthereumIBCError> {
    let epoch = client_state.compute_epoch_at_slot(header.beacon().slot);
    client_state.verify_supported_fork_at_epoch(epoch)?;

    match header {
        LightClientHeader::Electra(electra) => {
            ensure!(
                epoch < client_state.fork_parameters.gloas.epoch,
                EthereumIBCError::LightClientHeaderForkMismatch { epoch }
            );

            Ok(electra.execution.tree_hash_root())
        }
        LightClientHeader::Gloas(gloas) => Ok(gloas.execution_block_hash),
    }
}

// See spec: https://github.com/ethereum/consensus-specs/blob/master/specs/gloas/light-client/sync-protocol.md#modified-is_valid_light_client_header
/// Validates a light client header. A header in the gloas shape proves its execution block hash
/// against the beacon block body: at the gloas gindex from gloas on, at the deneb one before.
/// # Errors
/// Returns an error if the header cannot be validated.
pub fn is_valid_light_client_header(
    client_state: &ClientState,
    header: &LightClientHeader,
) -> Result<(), EthereumIBCError> {
    let epoch = client_state.compute_epoch_at_slot(header.beacon().slot);

    client_state.verify_supported_fork_at_epoch(epoch)?;

    match header {
        LightClientHeader::Electra(electra) => validate_merkle_branch(
            get_lc_execution_root(client_state, header)?,
            electra.execution_branch.to_vec(),
            floorlog2(EXECUTION_PAYLOAD_GINDEX),
            get_subtree_index(EXECUTION_PAYLOAD_GINDEX),
            electra.beacon.body_root,
        ),
        LightClientHeader::Gloas(gloas) => {
            let gindex = if epoch >= client_state.fork_parameters.gloas.epoch {
                EXECUTION_BLOCK_HASH_GINDEX_GLOAS
            } else {
                EXECUTION_BLOCK_HASH_GINDEX_DENEB
            };

            is_valid_normalized_merkle_branch(
                gloas.execution_block_hash,
                &gloas.execution_branch,
                gindex,
                gloas.beacon.body_root,
            )
        }
    }
}

// See spec: <https://github.com/ethereum/consensus-specs/blob/dev/specs/altair/light-client/sync-protocol.md#get_subtree_index>
/// Values that are constant across all configurations.
#[must_use]
pub const fn get_subtree_index(idx: u64) -> u64 {
    idx % 2_u64.pow(idx.ilog2())
}

// See spec: <https://github.com/ethereum/consensus-specs/blob/ff99bc03d6da29d9ef6e055bdb8500e1b2942f1e/specs/electra/light-client/fork.md#L26>
/// Normalize a merkle branch to a depth for given gindex.
/// # Errors
/// Returns an error if the merkle branch is deeper than the given gindex allows.
pub fn normalize_merkle_branch(
    branch: &[B256],
    gindex: u64,
) -> Result<Vec<B256>, EthereumIBCError> {
    let depth = floorlog2(gindex);
    let num_extra =
        depth
            .checked_sub(branch.len())
            .ok_or(EthereumIBCError::MerkleBranchTooDeep {
                length: branch.len(),
                gindex,
            })?;

    // TODO: Switch to std::iter::repeat_n when cosmwasm supports rust 1.85 (https://github.com/CosmWasm/cosmwasm/issues/2292)
    Ok(vec![B256::default(); num_extra]
        .into_iter()
        .chain(branch.to_vec())
        .collect())
}

#[cfg(test)]
mod test {
    use alloy_primitives::{hex::FromHex, B256};

    use crate::sync_protocol_helpers::normalize_merkle_branch;

    #[test]
    fn test_nomralize_merkle_branch() {
        let branch = vec![B256::from_hex(
            "0x75d7411cb01daad167713b5a9b7219670f0e500653cbbcd45cfe1bfe04222459",
        )
        .unwrap()];
        let gindex = 4;
        let normalized = normalize_merkle_branch(&branch, gindex).unwrap();

        let expected_branch = vec![B256::default(), branch[0]];
        assert_eq!(normalized, expected_branch);
    }

    #[test]
    fn test_normalize_merkle_branch_with_no_extra() {
        let branch = vec![B256::default(); 3];
        let gindex = 8;
        let normalized = normalize_merkle_branch(&branch, gindex).unwrap();

        assert_eq!(normalized, branch);
    }

    #[test]
    fn test_normalize_merkle_branch_rejects_a_branch_deeper_than_the_gindex() {
        let result = normalize_merkle_branch(&[B256::default(); 3], 2);

        assert_eq!(
            result,
            Err(crate::error::EthereumIBCError::MerkleBranchTooDeep {
                length: 3,
                gindex: 2
            })
        );
    }
}
