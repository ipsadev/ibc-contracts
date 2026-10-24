// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

/// @title SP1 ICS Stellar Light Client Interface
/// @notice The interface of the SP1 Stellar light client.
interface ISP1ICSStellar {
    /// @notice The verification key of the update client program.
    /// @return The verification key.
    function UPDATE_CLIENT_PROGRAM_VKEY() external view returns (bytes32);

    /// @notice The verification key of the verify (non)membership program.
    /// @return The verification key.
    function MEMBERSHIP_PROGRAM_VKEY() external view returns (bytes32);

    /// @notice The verification key of the combined update-client-and-membership program.
    /// @return The verification key.
    // solhint-disable-next-line func-name-mixedcase
    function UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY() external view returns (bytes32);

    /// @notice The role allowed to submit proofs.
    /// @return The role identifier.
    function PROOF_SUBMITTER_ROLE() external view returns (bytes32);

    /// @notice The role allowed to freeze the client.
    /// @return The role identifier.
    function CLIENT_FREEZER_ROLE() external view returns (bytes32);

    /// @notice The keccak256 hash of the consensus state stored at a height.
    /// @param revisionHeight The Stellar ledger sequence number.
    /// @return The hash.
    function getConsensusStateHash(uint64 revisionHeight) external view returns (bytes32);

    /// @notice Freezes the client, so that it verifies nothing further.
    /// @dev Until misbehaviour proving exists this is the only way a client is frozen.
    function freeze() external;
}
