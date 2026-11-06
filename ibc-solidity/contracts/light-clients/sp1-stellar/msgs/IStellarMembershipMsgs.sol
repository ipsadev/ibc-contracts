// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { ISP1Msgs } from "../../sp1-ics07/msgs/ISP1Msgs.sol";
import { IICSStellarMsgs } from "./IICSStellarMsgs.sol";

/// @title Stellar Membership Program Messages
/// @notice Defines shared types for the Stellar verify (non)membership program.
interface IStellarMembershipMsgs {
    /// @notice The key-value pair used in the verify (non)membership program.
    /// @dev An empty value means non-membership, which is what a timeout needs.
    /// @param path The path of the value in the key-value store.
    /// @param value The value of the key-value pair.
    struct KVPair {
        bytes[] path;
        bytes value;
    }

    /// @notice One key-value pair and the proof that it is, or is not, in the state tree.
    /// @dev The tree is a fixed-depth sparse Merkle tree, so a proof is exactly its siblings.
    /// ICS-23 carries the same siblings inside a protobuf envelope; the envelope buys nothing
    /// here because the contract that reads this output defines its own wire format.
    /// @param kvPair The key-value pair being proven.
    /// @param siblings The sibling hashes, leaf upwards. Exactly 64 of them.
    struct MembershipProofEntry {
        KVPair kvPair;
        bytes32[] siblings;
    }

    /// @notice The public value output for the sp1 Stellar verify (non)membership program.
    /// @dev transactionSetResultHash is the anchor: the contract must check it against the
    /// consensus state it already stored at the height being proven. ibcRoot is derived inside
    /// the circuit from that anchor, never supplied.
    /// @param clientState The client state that was used, echoed so the contract can check it.
    /// @param transactionSetResultHash The ledger's transaction result hash the proof is anchored to.
    /// @param ibcRoot The router's state root, derived from the transaction results.
    /// @param kvPairs The key-value pairs verified by the program.
    struct MembershipOutput {
        IICSStellarMsgs.ClientState clientState;
        bytes32 transactionSetResultHash;
        bytes32 ibcRoot;
        KVPair[] kvPairs;
    }

    /// @notice The membership proof that can be submitted to the SP1Verifier contract.
    /// @param sp1Proof The sp1 proof for the membership program.
    /// @param trustedConsensusState The trusted consensus state the proof is based on.
    struct SP1MembershipProof {
        ISP1Msgs.SP1Proof sp1Proof;
        IICSStellarMsgs.ConsensusState trustedConsensusState;
    }

    /// @notice Which program produced a membership proof.
    enum StellarMembershipProofType {
        /// The proof is of the membership program, against a height the client already holds.
        SP1MembershipProof,
        /// The proof is of the combined program, which proves the header and the packet together.
        SP1UcAndMembershipProof
    }

    /// @notice A membership proof and which program made it.
    /// @dev A relayer whose client is already at the height can send the first; one that would
    /// otherwise have to update the client first sends the second and pays for one proof rather
    /// than two.
    /// @param proofType Which program made it.
    /// @param proof The abi encoded proof of that type.
    struct StellarMembershipProof {
        StellarMembershipProofType proofType;
        bytes proof;
    }
}
