// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { ISP1Msgs } from "../../sp1-ics07/msgs/ISP1Msgs.sol";
import { IICSStellarMsgs } from "./IICSStellarMsgs.sol";
import { IICS02ClientMsgs } from "../../../msgs/IICS02ClientMsgs.sol";

/// @title Stellar Update Client Program Messages
/// @notice Defines shared types for the Stellar update client program.
interface IStellarUpdateClientMsgs {
    /// @notice The message that is submitted to the updateClient function.
    /// @param sp1Proof The SP1 proof for updating the client.
    struct MsgUpdateClient {
        ISP1Msgs.SP1Proof sp1Proof;
    }

    /// @notice The public value output for the sp1 Stellar update client program.
    /// @dev The client state is echoed back so the contract can check the proof was produced
    /// against the state it already held, rather than one the prover chose.
    /// @param clientState The client state that was used to verify the header.
    /// @param trustedConsensusState The trusted consensus state.
    /// @param newConsensusState The new consensus state with the verified header.
    /// @param time The time at which the header was verified, in unix nanoseconds.
    /// @param trustedHeight The trusted height.
    /// @param newHeight The new height.
    struct UpdateClientOutput {
        IICSStellarMsgs.ClientState clientState;
        IICSStellarMsgs.ConsensusState trustedConsensusState;
        IICSStellarMsgs.ConsensusState newConsensusState;
        uint128 time;
        IICS02ClientMsgs.Height trustedHeight;
        IICS02ClientMsgs.Height newHeight;
    }
}
