// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

/// @title SP1 ICS Stellar Errors
/// @notice Defines the errors of the SP1 Stellar light client.
interface ISP1ICSStellarErrors {
    /// @notice The proof was produced by a different program than this contract trusts.
    /// @param expected The verification key this contract pins.
    /// @param actual The verification key the proof carries.
    error VerificationKeyMismatch(bytes32 expected, bytes32 actual);

    /// @notice The proof was produced against client parameters this contract does not hold.
    /// @dev This is the check that makes the pinned trust root, network and router mean something.
    /// @param expected The hash of the client state this contract holds.
    /// @param actual The hash of the client state the proof echoed back.
    error ClientStateMismatch(bytes32 expected, bytes32 actual);

    /// @notice No consensus state has been stored at the height the proof is anchored to.
    error ConsensusStateNotFound();

    /// @notice The trusted consensus state in the proof is not the one stored at that height.
    /// @param expected The hash this contract stored.
    /// @param actual The hash the proof echoed back.
    error ConsensusStateHashMismatch(bytes32 expected, bytes32 actual);

    /// @notice A consensus state already exists at that height and differs from this one.
    /// @dev Raised by the combined update-and-membership path, which cannot freeze without failing the packet. The
    /// same proof submitted through updateClient freezes the client as misbehaviour.
    /// @param height The height in question.
    error ConsensusStateConflict(uint64 height);

    /// @notice The proof does not anchor to the transaction result hash of the stored consensus state.
    /// @param expected The hash from the stored consensus state.
    /// @param actual The hash the proof was anchored to.
    error ResultHashMismatch(bytes32 expected, bytes32 actual);

    /// @notice The proven ledger closed later than this chain's clock allows, even after the allowed drift.
    /// @param currentTimestamp The current block timestamp, in unix seconds.
    /// @param closeTime The proven ledger's close time, in unix seconds.
    error ProofIsInTheFuture(uint256 currentTimestamp, uint256 closeTime);

    /// @notice The client is frozen permanently and verifies nothing further.
    error FrozenClient();

    /// @notice The proof proves a different path or value than the one being asked about.
    error KeyValueMismatch();

    /// @notice A membership proof carried no key-value pairs.
    error EmptyKeyValuePairs();

    /// @notice A membership request carried an empty value, which is a non-membership request.
    error EmptyValue();

    /// @notice The revision number is not the one this client was created with.
    /// @param expected The revision this client uses.
    /// @param actual The revision the proof carries.
    error RevisionMismatch(uint64 expected, uint64 actual);

    /// @notice The combined proof proves a different height than the one being asked about.
    /// @param expected The height asked about.
    /// @param actual The height the proof establishes.
    error ProofHeightMismatch(uint64 expected, uint64 actual);

    /// @notice Submitting misbehaviour evidence through misbehaviour() is not supported.
    /// @dev Conflicting headers are still caught: an updateClient proof of a different consensus state at a stored
    /// height freezes the client. Otherwise only CLIENT_FREEZER_ROLE can freeze it.
    error MisbehaviourNotImplemented();
}
