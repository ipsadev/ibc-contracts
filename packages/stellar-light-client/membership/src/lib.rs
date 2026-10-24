// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "scenario")]
pub mod scenario;
pub mod smt;

use ipsa_scp_verifier::scp::results::verify_state_root;
use ipsa_scp_verifier::scp::results::Inputs;
use stellar_light_client_solidity_types::ClientState;
use stellar_light_client_solidity_types::StateRootProof;
use thiserror::Error;

pub use stellar_light_client_solidity_types::KVPair;
pub use stellar_light_client_solidity_types::MembershipOutput;
pub use stellar_light_client_solidity_types::MembershipProofEntry;

#[derive(Debug, Error)]
pub enum MembershipError {
    #[error("the client is frozen")]
    ClientFrozen,

    #[error("no key-value pairs were submitted")]
    NothingToProve,

    #[error("the router did not publish a state root in this ledger: {0}")]
    StateRootNotBound(String),

    #[error("entry {index}: expected {TREE_DEPTH} siblings, got {got}", TREE_DEPTH = smt::TREE_DEPTH)]
    WrongProofDepth { index: usize, got: usize },

    #[error("entry {index}: the key is not in the tree under root {root}")]
    NotAMember { index: usize, root: String },

    #[error("entry {index}: the key is in the tree under root {root}, so it cannot be absent")]
    NotAbsent { index: usize, root: String },
}

pub fn membership(
    client_state: &ClientState,
    transaction_set_result_hash: [u8; 32],
    state_root_proof: &StateRootProof,
    entries: &[MembershipProofEntry],
) -> Result<MembershipOutput, MembershipError> {
    if client_state.isFrozen {
        return Err(MembershipError::ClientFrozen);
    }

    if entries.is_empty() {
        return Err(MembershipError::NothingToProve);
    }

    let result_pairs: Vec<Vec<u8>> = state_root_proof
        .resultPairs
        .iter()
        .map(|pair| pair.to_vec())
        .collect();

    let ibc_root = verify_state_root(&Inputs {
        tx_set_result_hash: &transaction_set_result_hash,
        result_pairs: &result_pairs,
        result_index: state_root_proof.resultIndex,
        success_preimage_xdr: &state_root_proof.successPreimageXdr,
        router_contract_id: &client_state.routerContractId.0,
        root_event_topic: &client_state.rootEventTopic,
    })
    .map_err(|error| MembershipError::StateRootNotBound(format!("{error:?}")))?;

    let mut verified_pairs = Vec::with_capacity(entries.len());

    for (index, entry) in entries.iter().enumerate() {
        if entry.siblings.len() != smt::TREE_DEPTH {
            return Err(MembershipError::WrongProofDepth {
                index,
                got: entry.siblings.len(),
            });
        }

        let siblings: Vec<[u8; 32]> = entry.siblings.iter().map(|one| one.0).collect();

        let key = concatenate_path(&entry.kvPair.path);

        if entry.kvPair.value.is_empty() {
            if !smt::verify_non_membership(&ibc_root, &key, &siblings) {
                return Err(MembershipError::NotAbsent {
                    index,
                    root: hex::encode(ibc_root),
                });
            }
        } else if !smt::verify_membership(&ibc_root, &key, &entry.kvPair.value, &siblings) {
            return Err(MembershipError::NotAMember {
                index,
                root: hex::encode(ibc_root),
            });
        }

        verified_pairs.push(entry.kvPair.clone());
    }

    Ok(MembershipOutput {
        clientState: client_state.clone(),
        transactionSetResultHash: transaction_set_result_hash.into(),
        ibcRoot: ibc_root.into(),
        kvPairs: verified_pairs,
    })
}

pub fn concatenate_path(path: &[alloy_sol_types::private::Bytes]) -> Vec<u8> {
    let total = path.iter().map(|segment| segment.len()).sum();

    let mut key = Vec::with_capacity(total);

    for segment in path {
        key.extend_from_slice(segment);
    }

    key
}
