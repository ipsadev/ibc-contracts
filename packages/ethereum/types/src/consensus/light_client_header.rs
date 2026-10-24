// SPDX-License-Identifier: Apache-2.0

//! This module defines types related to beacon's light client functionality.

use alloy_primitives::{Address, Bloom, Bytes, B256, U256};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tree_hash_derive::TreeHash;

use super::{
    merkle::{
        floorlog2, EXECUTION_BLOCK_HASH_GINDEX_GLOAS, EXECUTION_PAYLOAD_GINDEX,
        FINALIZED_ROOT_GINDEX_ELECTRA, FINALIZED_ROOT_GINDEX_GLOAS,
        NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA, NEXT_SYNC_COMMITTEE_GINDEX_GLOAS,
    },
    sync_committee::{SyncAggregate, SyncCommittee},
};

macro_rules! fork_branch {
    ($(#[$meta:meta])* $name:ident, $electra:expr, $gloas:expr) => {
        $(#[$meta])*
        #[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug)]
        #[serde(untagged)]
        pub enum $name {
            /// A branch from electra up to fulu
            #[schemars(with = "Vec<String>")]
            Electra([B256; floorlog2($electra)]),
            /// A branch from gloas on
            #[schemars(with = "Vec<String>")]
            Gloas([B256; floorlog2($gloas)]),
        }

        impl $name {
            /// The branch nodes, from the leaf upwards
            #[must_use]
            pub const fn as_slice(&self) -> &[B256] {
                match self {
                    Self::Electra(branch) => branch,
                    Self::Gloas(branch) => branch,
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::Electra([B256::ZERO; floorlog2($electra)])
            }
        }
    };
}

pub(crate) use fork_branch;

fork_branch!(
    /// The finality branch: depth 7 from electra up to fulu, 9 from gloas on
    FinalityBranch,
    FINALIZED_ROOT_GINDEX_ELECTRA,
    FINALIZED_ROOT_GINDEX_GLOAS
);

fork_branch!(
    /// The next sync committee branch: depth 6 from electra up to fulu, 11 from gloas on
    NextSyncCommitteeBranch,
    NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA,
    NEXT_SYNC_COMMITTEE_GINDEX_GLOAS
);

/// A light client update
#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct LightClientUpdate {
    /// Header attested to by the sync committee
    pub attested_header: LightClientHeader,
    /// Next sync committee corresponding to `attested_header.state_root`
    pub next_sync_committee: Option<SyncCommittee>,
    /// The branch of the next sync committee
    pub next_sync_committee_branch: Option<NextSyncCommitteeBranch>,
    /// Finalized header corresponding to `attested_header.state_root`
    pub finalized_header: LightClientHeader,
    /// Branch of the finalized header
    pub finality_branch: FinalityBranch,
    /// Sync committee aggregate signature
    pub sync_aggregate: SyncAggregate,
    /// Slot at which the aggregate signature was created (untrusted)
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub signature_slot: u64,
}

/// A light client finality update
#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct LightClientFinalityUpdate {
    /// Header attested to by the sync committee
    pub attested_header: LightClientHeader,
    /// Finalized header corresponding to `attested_header.state_root`
    pub finalized_header: LightClientHeader,
    /// Branch of the finalized header
    pub finality_branch: FinalityBranch,
    /// Sync committee aggregate signature
    pub sync_aggregate: SyncAggregate,
    /// Slot at which the aggregate signature was created (untrusted)
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub signature_slot: u64,
}

// Light Client Finality Update to Light Client Update conversion
impl From<LightClientFinalityUpdate> for LightClientUpdate {
    fn from(finality_update: LightClientFinalityUpdate) -> Self {
        Self {
            attested_header: finality_update.attested_header,
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: finality_update.finalized_header,
            finality_branch: finality_update.finality_branch,
            sync_aggregate: finality_update.sync_aggregate,
            signature_slot: finality_update.signature_slot,
        }
    }
}

/// The header of a light client, in the shape of the fork that produced it
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug)]
#[serde(untagged)]
#[allow(clippy::module_name_repetitions, clippy::large_enum_variant)]
pub enum LightClientHeader {
    /// A header from electra up to fulu, carrying the execution payload header
    Electra(ElectraLightClientHeader),
    /// A header from gloas on, carrying only the execution block hash
    Gloas(GloasLightClientHeader),
}

impl Default for LightClientHeader {
    fn default() -> Self {
        Self::Electra(ElectraLightClientHeader::default())
    }
}

impl LightClientHeader {
    /// The beacon block header
    #[must_use]
    pub const fn beacon(&self) -> &BeaconBlockHeader {
        match self {
            Self::Electra(header) => &header.beacon,
            Self::Gloas(header) => &header.beacon,
        }
    }

    /// The execution payload header, which only headers before gloas carry
    #[must_use]
    pub const fn execution(&self) -> Option<&ExecutionPayloadHeader> {
        match self {
            Self::Electra(header) => Some(&header.execution),
            Self::Gloas(_) => None,
        }
    }

    /// The execution block hash this header commits to
    #[must_use]
    pub const fn execution_block_hash(&self) -> B256 {
        match self {
            Self::Electra(header) => header.execution.block_hash,
            Self::Gloas(header) => header.execution_block_hash,
        }
    }

    /// The execution branch, from the leaf upwards
    #[must_use]
    pub const fn execution_branch(&self) -> &[B256] {
        match self {
            Self::Electra(header) => &header.execution_branch,
            Self::Gloas(header) => &header.execution_branch,
        }
    }
}

/// The header of a light client from electra up to fulu
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug, Default, TreeHash)]
#[serde(deny_unknown_fields)]
pub struct ElectraLightClientHeader {
    /// The beacon block header
    pub beacon: BeaconBlockHeader,
    /// The execution payload header
    pub execution: ExecutionPayloadHeader,
    /// The execution branch
    #[schemars(with = "Vec<String>")]
    pub execution_branch: [B256; floorlog2(EXECUTION_PAYLOAD_GINDEX)],
}

/// The header of a light client from gloas on
/// [See in consensus-spec](https://github.com/ethereum/consensus-specs/blob/master/specs/gloas/light-client/sync-protocol.md#modified-lightclientheader)
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug, Default, TreeHash)]
#[serde(deny_unknown_fields)]
pub struct GloasLightClientHeader {
    /// The beacon block header
    pub beacon: BeaconBlockHeader,
    /// The hash of the latest revealed execution payload, the bid's `parent_block_hash`
    #[schemars(with = "String")]
    pub execution_block_hash: B256,
    /// The execution branch
    #[schemars(with = "Vec<String>")]
    pub execution_branch: [B256; floorlog2(EXECUTION_BLOCK_HASH_GINDEX_GLOAS)],
}

/// The beacon block header
#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug, Default, TreeHash)]
pub struct BeaconBlockHeader {
    /// The slot to which this block corresponds
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub slot: u64,
    /// The index of validator in validator registry
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub proposer_index: u64,
    /// The signing merkle root of the parent `BeaconBlock`
    #[schemars(with = "String")]
    pub parent_root: B256,
    /// The tree hash merkle root of the `BeaconState` for the `BeaconBlock`
    #[schemars(with = "String")]
    pub state_root: B256,
    /// The tree hash merkle root of the `BeaconBlockBody` for the `BeaconBlock`
    #[schemars(with = "String")]
    pub body_root: B256,
}

/// Header to track the execution block
#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug, Default, TreeHash)]
pub struct ExecutionPayloadHeader {
    /// The parent hash of the execution payload header
    #[schemars(with = "String")]
    pub parent_hash: B256,
    /// Block fee recipient
    #[schemars(with = "String")]
    pub fee_recipient: Address,
    /// The state root
    #[schemars(with = "String")]
    pub state_root: B256,
    /// The root of the receipts trie
    #[schemars(with = "String")]
    pub receipts_root: B256,
    /// The logs bloom filter
    #[schemars(with = "String")]
    pub logs_bloom: Bloom,
    /// The previous Randao value, used to compute the randomness on the execution layer.
    #[schemars(with = "String")]
    pub prev_randao: B256,
    /// The block number of the execution payload
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub block_number: u64,
    /// Execution block gas limit
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub gas_limit: u64,
    /// Execution block gas used
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub gas_used: u64,
    /// The timestamp of the execution payload
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub timestamp: u64,
    /// The extra data of the execution payload
    #[schemars(with = "String")]
    pub extra_data: Bytes,
    /// Block base fee per gas
    #[schemars(with = "String")]
    pub base_fee_per_gas: U256,
    /// The block hash
    #[schemars(with = "String")]
    pub block_hash: B256,
    /// SSZ hash tree root of the transaction list
    #[schemars(with = "String")]
    pub transactions_root: B256,
    /// Tree root of the withdrawals list
    #[schemars(with = "String")]
    pub withdrawals_root: B256,
    /// Blob gas used (new in Deneb)
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub blob_gas_used: u64,
    /// Excess blob gas (new in Deneb)
    #[serde_as(as = "DisplayFromStr")]
    #[schemars(with = "String")]
    pub excess_blob_gas: u64,
}
