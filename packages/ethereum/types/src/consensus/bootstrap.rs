// SPDX-License-Identifier: Apache-2.0

//! This module defines types related to the bootstrap endpoint of the Beacon API.

use alloy_primitives::B256;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{
    light_client_header::{fork_branch, LightClientHeader},
    merkle::{
        floorlog2, CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA, CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS,
    },
    sync_committee::SyncCommittee,
};

fork_branch!(
    /// The current sync committee branch: depth 6 from electra up to fulu, 11 from gloas on
    CurrentSyncCommitteeBranch,
    CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA,
    CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS
);

/// The light client bootstrap
#[serde_as]
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
#[allow(clippy::module_name_repetitions)]
pub struct LightClientBootstrap {
    /// The light client header
    pub header: LightClientHeader,
    /// The current sync committee
    pub current_sync_committee: SyncCommittee,
    /// The branch of the current sync committee
    pub current_sync_committee_branch: CurrentSyncCommitteeBranch,
}
