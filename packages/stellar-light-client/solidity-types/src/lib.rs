// SPDX-License-Identifier: Apache-2.0

use alloy_sol_types::sol;

sol!("../../../ibc-solidity/contracts/msgs/IICS02ClientMsgs.sol");
sol!("../../../ibc-solidity/contracts/light-clients/sp1-ics07/msgs/ISP1Msgs.sol");
sol!("../../../ibc-solidity/contracts/light-clients/sp1-stellar/msgs/IICSStellarMsgs.sol");
sol!("../../../ibc-solidity/contracts/light-clients/sp1-stellar/msgs/IStellarUpdateClientMsgs.sol");
sol!("../../../ibc-solidity/contracts/light-clients/sp1-stellar/msgs/IStellarMembershipMsgs.sol");
sol!("../../../ibc-solidity/contracts/light-clients/sp1-stellar/msgs/IStellarUcAndMembershipMsgs.sol");

pub use IICS02ClientMsgs::Height;
pub use IICSStellarMsgs::ClientState;
pub use IICSStellarMsgs::ConsensusState;
pub use IICSStellarMsgs::Header;
pub use IICSStellarMsgs::StateRootProof;
pub use IICSStellarMsgs::SupportedZkAlgorithm;
pub use IStellarMembershipMsgs::KVPair;
pub use IStellarMembershipMsgs::MembershipOutput;
pub use IStellarMembershipMsgs::MembershipProofEntry;
pub use IStellarUcAndMembershipMsgs::UcAndMembershipOutput;
pub use IStellarUpdateClientMsgs::UpdateClientOutput;
