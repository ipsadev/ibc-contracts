// SPDX-License-Identifier: Apache-2.0

use sha2::Digest;
use sha2::Sha256;
pub use stellar_consensus_verifier::error::ScpError;
use stellar_consensus_verifier::verify::verify_update;
use stellar_consensus_verifier::verify::TrustRoot;
use stellar_consensus_verifier::verify::UpdateEvidence;
use thiserror::Error;

pub use stellar_light_client_solidity_types::ClientState;
pub use stellar_light_client_solidity_types::ConsensusState;
pub use stellar_light_client_solidity_types::Header;
pub use stellar_light_client_solidity_types::Height;
pub use stellar_light_client_solidity_types::StateRootProof;
pub use stellar_light_client_solidity_types::SupportedZkAlgorithm;
pub use stellar_light_client_solidity_types::UpdateClientOutput;

#[derive(Debug, Error)]
pub enum UpdateClientError {
    #[error("the client is frozen")]
    ClientFrozen,

    #[error("trust root fingerprint mismatch: the client state pins {pinned}, the header supplied a quorum set hashing to {supplied}")]
    TrustRootMismatch { pinned: String, supplied: String },

    #[error("the supplied trust root is not a usable quorum set: {0}")]
    MalformedTrustRoot(ScpError),

    #[error("proposed slot {proposed} does not advance the client, which is at {trusted}")]
    SlotDoesNotAdvance { proposed: u64, trusted: u64 },

    #[error("no slot {0} evidence was supplied, so this header's bucketListHash and txSetResultHash would be unauthenticated")]
    NextSlotEvidenceMissing(u64),

    #[error("the proposed header does not chain from the trusted ledger: previousLedgerHash {found} is not the trusted {expected}")]
    NotChildOfTrusted { expected: String, found: String },

    #[error("header verification failed: {0}")]
    HeaderVerificationFailed(ScpError),
}

pub fn update_client(
    client_state: &ClientState,
    trusted_consensus_state: &ConsensusState,
    proposed_header: &Header,
    time: u128,
) -> Result<UpdateClientOutput, UpdateClientError> {
    if client_state.isFrozen {
        return Err(UpdateClientError::ClientFrozen);
    }

    let pinned_fingerprint = client_state.trustRootFingerprint.0;
    let supplied_fingerprint: [u8; 32] = Sha256::digest(&proposed_header.trustRootXdr).into();

    if supplied_fingerprint != pinned_fingerprint {
        return Err(UpdateClientError::TrustRootMismatch {
            pinned: hex::encode(pinned_fingerprint),
            supplied: hex::encode(supplied_fingerprint),
        });
    }

    let trusted_slot = client_state.latestHeight.revisionHeight;

    if proposed_header.slot <= trusted_slot {
        return Err(UpdateClientError::SlotDoesNotAdvance {
            proposed: proposed_header.slot,
            trusted: trusted_slot,
        });
    }

    let next_slot = proposed_header.slot + 1;

    if proposed_header.nextEnvelopesXdr.is_empty() || proposed_header.nextTxSetXdr.is_empty() {
        return Err(UpdateClientError::NextSlotEvidenceMissing(next_slot));
    }

    let trust_root = TrustRoot::from_pinned(&proposed_header.trustRootXdr, &pinned_fingerprint)
        .map_err(UpdateClientError::MalformedTrustRoot)?;

    let envelopes_xdr: Vec<Vec<u8>> = proposed_header
        .envelopesXdr
        .iter()
        .map(|envelope| envelope.to_vec())
        .collect();

    let quorum_sets_xdr: Vec<Vec<u8>> = proposed_header
        .quorumSetsXdr
        .iter()
        .map(|quorum_set| quorum_set.to_vec())
        .collect();

    let next_envelopes_xdr: Vec<Vec<u8>> = proposed_header
        .nextEnvelopesXdr
        .iter()
        .map(|envelope| envelope.to_vec())
        .collect();

    let authenticated = verify_update(
        &trust_root,
        &UpdateEvidence {
            network_id: &client_state.networkId.0,
            slot: proposed_header.slot,
            header_xdr: &proposed_header.ledgerHeaderXdr,
            envelopes_xdr: &envelopes_xdr,
            quorum_sets_xdr: &quorum_sets_xdr,
            successor_envelopes_xdr: &next_envelopes_xdr,
            successor_quorum_sets_xdr: &quorum_sets_xdr,
            successor_tx_set_xdr: &proposed_header.nextTxSetXdr,
        },
    )
    .map_err(UpdateClientError::HeaderVerificationFailed)?;

    let header = authenticated.header();

    if proposed_header.slot == trusted_slot + 1
        && header.previous_ledger_hash != trusted_consensus_state.ledgerHash.0
    {
        return Err(UpdateClientError::NotChildOfTrusted {
            expected: hex::encode(trusted_consensus_state.ledgerHash.0),
            found: hex::encode(header.previous_ledger_hash),
        });
    }

    let new_consensus_state = ConsensusState {
        closeTime: header.close_time,
        ledgerHash: header.ledger_hash.into(),
        previousLedgerHash: header.previous_ledger_hash.into(),
        bucketListHash: header.bucket_list_hash.into(),
        transactionSetResultHash: header.tx_set_result_hash.into(),
    };

    let revision = client_state.latestHeight.revisionNumber;

    Ok(UpdateClientOutput {
        clientState: client_state.clone(),
        trustedConsensusState: trusted_consensus_state.clone(),
        newConsensusState: new_consensus_state,
        time,
        trustedHeight: Height {
            revisionNumber: revision,
            revisionHeight: trusted_slot,
        },
        newHeight: Height {
            revisionNumber: revision,
            revisionHeight: proposed_header.slot,
        },
    })
}
