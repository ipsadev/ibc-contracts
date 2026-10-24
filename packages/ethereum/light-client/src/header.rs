// SPDX-License-Identifier: Apache-2.0

//! This module defines types related to the Header we use for the Ethereum light client

use alloy_primitives::Bytes;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use ethereum_types::consensus::{
    light_client_header::LightClientUpdate, sync_committee::SyncCommittee,
};

/// The header of a light client update
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug)]
pub struct Header {
    /// The active sync committee (untrusted)
    pub active_sync_committee: ActiveSyncCommittee,
    /// The consensus update
    pub consensus_update: LightClientUpdate,
    /// Trusted slot to verify the new update against
    // The client **must** have a consensus state for the provided slot
    pub trusted_slot: u64,
    /// The RLP encoded execution block header the finalized header commits to.
    /// Required from gloas on, where light client headers carry only the execution block hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub execution_header: Option<Bytes>,
}

/// The active sync committee
#[derive(Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone, Debug)]
pub enum ActiveSyncCommittee {
    /// The current sync committee
    Current(SyncCommittee),
    /// The next sync committee
    Next(SyncCommittee),
}
