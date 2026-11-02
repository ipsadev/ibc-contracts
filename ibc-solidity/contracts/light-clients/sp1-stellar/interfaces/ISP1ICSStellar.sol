// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

/// @title SP1 ICS Stellar Light Client Interface
/// @notice The interface of the SP1 Stellar light client.
interface ISP1ICSStellar {
    /// @notice Emitted when the client is frozen, by CLIENT_FREEZER_ROLE or by a proven misbehaviour.
    /// @param caller The account whose call froze the client.
    event ClientFrozen(address indexed caller);

    /// @notice The verification key of the update client program.
    /// @return The verification key.
    function UPDATE_CLIENT_PROGRAM_VKEY() external view returns (bytes32);

    /// @notice The verification key of the verify (non)membership program.
    /// @return The verification key.
    function MEMBERSHIP_PROGRAM_VKEY() external view returns (bytes32);

    /// @notice The verification key of the combined update-client-and-membership program.
    /// @return The verification key.
    function UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY() external view returns (bytes32);

    /// @notice The role allowed to submit proofs.
    /// @return The role identifier.
    function PROOF_SUBMITTER_ROLE() external view returns (bytes32);

    /// @notice The role allowed to freeze the client.
    /// @return The role identifier.
    function CLIENT_FREEZER_ROLE() external view returns (bytes32);

    /// @notice How far a proven ledger's close time may run ahead of this chain's block timestamp.
    /// @return The allowed clock drift in seconds.
    function ALLOWED_CLOCK_DRIFT() external view returns (uint16);

    /// @notice The keccak256 hash of the consensus state stored at a height.
    /// @param revisionHeight The Stellar ledger sequence number.
    /// @return The hash.
    function getConsensusStateHash(uint64 revisionHeight) external view returns (bytes32);

    /// @notice Freezes the client, so that it verifies nothing further, and emits ClientFrozen.
    /// @dev Freezing is permanent: nothing in this contract unfreezes it, and recovering means deploying a new client.
    /// Only CLIENT_FREEZER_ROLE may call it. That role is granted to the roleManager at construction, so a client
    /// deployed with roleManager == address(0) has no freezer and can only be frozen by a misbehaving update.
    function freeze() external;
}
