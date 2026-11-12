// SPDX-License-Identifier: Apache-2.0

use ipsa_scp_verifier::error::VerifyError;
use ipsa_scp_verifier::steps::authenticate_evidence;
use ipsa_scp_verifier::steps::index_quorum_sets;
use ipsa_scp_verifier::steps::reject_repeated_signers;
use ipsa_scp_verifier::steps::validate_externalize;
use ipsa_scp_verifier::steps::verify_externalized_quorum;
use ipsa_scp_verifier::steps::verify_header;
use ipsa_scp_verifier::steps::verify_next_slot_binding;
use ipsa_scp_verifier::steps::verify_quorum_set_hashes;
use ipsa_scp_verifier::steps::HeaderInput;
use ipsa_scp_verifier::steps::QuorumConfig;
use sha2::Digest;
use sha2::Sha256;
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
    MalformedTrustRoot(VerifyError),

    #[error("proposed slot {proposed} does not advance the client, which is at {trusted}")]
    SlotDoesNotAdvance { proposed: u64, trusted: u64 },

    #[error("an envelope is not a usable EXTERNALIZE for this slot: {0}")]
    MalformedEnvelope(VerifyError),

    #[error("a quorum set preimage does not decode: {0}")]
    MalformedQuorumSet(VerifyError),

    #[error("a signer's pledged quorum set is not the one it signed: {0}")]
    QuorumSetAuthenticationFailed(VerifyError),

    #[error("the signers do not form a quorum under the pinned trust root: {0}")]
    QuorumCheckFailed(VerifyError),

    #[error("header verification failed: {0}")]
    HeaderVerificationFailed(VerifyError),

    #[error("no slot {0} evidence was supplied, so this header's bucketListHash and txSetResultHash would be unauthenticated")]
    NextSlotEvidenceMissing(u64),

    #[error("a slot {slot} envelope is not usable: {source}")]
    MalformedNextEnvelope { slot: u64, source: VerifyError },

    #[error("the slot {slot} signers do not authenticate: {source}")]
    NextSlotAuthenticationFailed { slot: u64, source: VerifyError },

    #[error("the next ledger does not point back at this header: {0}")]
    NextSlotBindingFailed(VerifyError),
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

    let quorum_config = QuorumConfig::from_xdr(&proposed_header.trustRootXdr)
        .map_err(UpdateClientError::MalformedTrustRoot)?;

    let trusted_previous_hash = if proposed_header.slot == trusted_slot + 1 {
        Some(trusted_consensus_state.ledgerHash.0)
    } else {
        None
    };

    let input = HeaderInput {
        slot: proposed_header.slot,
        network_id: client_state.networkId.0,
        ledger_header_xdr: proposed_header.ledgerHeaderXdr.to_vec(),
        envelopes_xdr: proposed_header
            .envelopesXdr
            .iter()
            .map(|envelope| envelope.to_vec())
            .collect(),
        quorum_sets_xdr: proposed_header
            .quorumSetsXdr
            .iter()
            .map(|quorum_set| quorum_set.to_vec())
            .collect(),
        result_pairs_xdr: Vec::new(),
        recorded_ledger_hash: None,
        trusted_previous_hash,
    };

    let envelopes = input
        .envelopes_xdr
        .iter()
        .map(|raw| validate_externalize(raw, input.slot))
        .collect::<Result<Vec<_>, _>>()
        .map_err(UpdateClientError::MalformedEnvelope)?;

    let preimages =
        index_quorum_sets(&input.quorum_sets_xdr).map_err(UpdateClientError::MalformedQuorumSet)?;

    let signers = verify_quorum_set_hashes(&envelopes, &preimages)
        .map_err(UpdateClientError::QuorumSetAuthenticationFailed)?;

    verify_externalized_quorum(&quorum_config, &signers, &envelopes)
        .map_err(UpdateClientError::QuorumCheckFailed)?;

    let verified = verify_header(&input, &quorum_config)
        .map_err(UpdateClientError::HeaderVerificationFailed)?;

    let next_slot = proposed_header.slot + 1;

    if proposed_header.nextEnvelopesXdr.is_empty() || proposed_header.nextTxSetXdr.is_empty() {
        return Err(UpdateClientError::NextSlotEvidenceMissing(next_slot));
    }

    let next_envelopes_xdr: Vec<Vec<u8>> = proposed_header
        .nextEnvelopesXdr
        .iter()
        .map(|envelope| envelope.to_vec())
        .collect();

    let next_evidence = authenticate_evidence(
        &client_state.networkId.0,
        next_slot,
        &next_envelopes_xdr,
        &input.quorum_sets_xdr,
    )
    .map_err(|source| UpdateClientError::NextSlotAuthenticationFailed {
        slot: next_slot,
        source,
    })?;

    reject_repeated_signers(&next_evidence).map_err(|source| {
        UpdateClientError::NextSlotAuthenticationFailed {
            slot: next_slot,
            source,
        }
    })?;

    let next_envelopes = next_envelopes_xdr
        .iter()
        .map(|raw| validate_externalize(raw, next_slot))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| UpdateClientError::MalformedNextEnvelope {
            slot: next_slot,
            source,
        })?;

    let next_signers = verify_quorum_set_hashes(&next_envelopes, &preimages).map_err(|source| {
        UpdateClientError::NextSlotAuthenticationFailed {
            slot: next_slot,
            source,
        }
    })?;

    let next_externalized =
        verify_externalized_quorum(&quorum_config, &next_signers, &next_envelopes).map_err(
            |source| UpdateClientError::NextSlotAuthenticationFailed {
                slot: next_slot,
                source,
            },
        )?;

    verify_next_slot_binding(
        &next_externalized.value,
        &proposed_header.nextTxSetXdr,
        &verified.ledger_hash,
    )
    .map_err(UpdateClientError::NextSlotBindingFailed)?;

    let new_consensus_state = ConsensusState {
        closeTime: verified.close_time,
        ledgerHash: verified.ledger_hash.into(),
        previousLedgerHash: verified.previous_ledger_hash.into(),
        bucketListHash: verified.bucket_list_hash.into(),
        transactionSetResultHash: verified.tx_set_result_hash.into(),
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
