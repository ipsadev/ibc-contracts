// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { IStellarUpdateClientMsgs } from "./IStellarUpdateClientMsgs.sol";
import { IStellarMembershipMsgs } from "./IStellarMembershipMsgs.sol";
import { ISP1Msgs } from "../../sp1-ics07/msgs/ISP1Msgs.sol";

/// @title Stellar Update Client and Membership Program Messages
/// @notice Defines shared types for the combined update client and membership program.
interface IStellarUcAndMembershipMsgs {
    /// @notice The public value output for the combined program.
    /// @dev A relayer whose client is stale would otherwise pay for two proofs to deliver one
    /// packet: a header update, then a membership proof against the height it just established.
    /// This proves both at once. The membership is anchored on the transactionSetResultHash of the
    /// header verified in the same run, not on one the contract already stored, which is why the
    /// two cannot be separated after the fact.
    /// @param updateClientOutput The output of the update client half.
    /// @param ibcRoot The router's state root, read from the ibc_root event in the proven ledger.
    /// @param kvPairs The key-value pairs proven against that root.
    struct UcAndMembershipOutput {
        IStellarUpdateClientMsgs.UpdateClientOutput updateClientOutput;
        bytes32 ibcRoot;
        IStellarMembershipMsgs.KVPair[] kvPairs;
    }

    /// @notice A combined proof, submitted where a membership proof would otherwise go.
    /// @param sp1Proof The SP1 proof of the combined program.
    struct SP1UcAndMembershipProof {
        ISP1Msgs.SP1Proof sp1Proof;
    }
}
