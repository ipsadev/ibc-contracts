// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { IICSStellarMsgs } from "./msgs/IICSStellarMsgs.sol";
import { IStellarUpdateClientMsgs } from "./msgs/IStellarUpdateClientMsgs.sol";
import { IStellarMembershipMsgs } from "./msgs/IStellarMembershipMsgs.sol";
import { IStellarUcAndMembershipMsgs } from "./msgs/IStellarUcAndMembershipMsgs.sol";
import { ISP1Msgs } from "../sp1-ics07/msgs/ISP1Msgs.sol";
import { ILightClientMsgs } from "../../msgs/ILightClientMsgs.sol";
import { IICS02ClientMsgs } from "../../msgs/IICS02ClientMsgs.sol";

import { ISP1ICSStellarErrors } from "./errors/ISP1ICSStellarErrors.sol";
import { ISP1ICSStellar } from "./interfaces/ISP1ICSStellar.sol";
import { ILightClient } from "../../interfaces/ILightClient.sol";
import { ISP1Verifier } from "@sp1-contracts/ISP1Verifier.sol";

import { Multicall } from "@openzeppelin-contracts/utils/Multicall.sol";
import { AccessControl } from "@openzeppelin-contracts/access/AccessControl.sol";

/// @title SP1 ICS Stellar Light Client
/// @notice An IBC light client of Stellar for the EVM, verifying SCP consensus through an SP1 proof.
/// @dev Adapted from SP1ICS07Tendermint.sol in this repository, which carries the same licence.
/// Three things differ from the Tendermint client, and each is a property of Stellar rather than a
/// simplification:
/// - There is no trusting period or unbonding period. Stellar has instant finality and no
///   unbonding, so a verified ledger does not go stale. Defining an age policy is deliberately
///   left open rather than inherited from Tendermint.
/// - Membership is anchored on the consensus state's transactionSetResultHash rather than an app
///   hash, because the router publishes its state root as a contract event.
/// - The client state pins the network, the trust root, the router contract and its event topic.
///   Every proof echoes the client state back and this contract rejects any that used different
///   parameters. That echo is what makes the pinning mean anything.
contract SP1ICSStellar is ISP1ICSStellarErrors, ISP1ICSStellar, ILightClient, Multicall, AccessControl {
    /// @inheritdoc ISP1ICSStellar
    bytes32 public immutable UPDATE_CLIENT_PROGRAM_VKEY;
    /// @inheritdoc ISP1ICSStellar
    bytes32 public immutable MEMBERSHIP_PROGRAM_VKEY;
    /// @inheritdoc ISP1ICSStellar
    bytes32 public immutable UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY;

    /// @notice The SP1 verifier this client forwards proofs to.
    // natlint-disable-next-line MissingInheritdoc
    ISP1Verifier public immutable VERIFIER;

    /// @notice The Stellar client state.
    // natlint-disable-next-line MissingInheritdoc
    IICSStellarMsgs.ClientState public clientState;

    /// @notice The keccak256 hash of the consensus state stored at each height.
    mapping(uint64 height => bytes32 hash) private _consensusStateHashes;

    /// @inheritdoc ISP1ICSStellar
    bytes32 public constant PROOF_SUBMITTER_ROLE = keccak256("PROOF_SUBMITTER_ROLE");

    /// @inheritdoc ISP1ICSStellar
    bytes32 public constant CLIENT_FREEZER_ROLE = keccak256("CLIENT_FREEZER_ROLE");

    /// @notice Sets the program verification keys and the initial client and consensus states.
    /// @param updateClientProgramVkey The verification key of the update client program.
    /// @param membershipProgramVkey The verification key of the verify (non)membership program.
    /// @param ucAndMembershipProgramVkey The verification key of the combined program.
    /// @param sp1Verifier The address of the SP1 verifier contract.
    /// @param initialClientState The abi encoded initial client state.
    /// @param initialConsensusState The keccak256 hash of the initial consensus state.
    /// @param roleManager Manages proof submitters and can submit proofs. The ICS26Router, in IBC.
    constructor(
        bytes32 updateClientProgramVkey,
        bytes32 membershipProgramVkey,
        bytes32 ucAndMembershipProgramVkey,
        address sp1Verifier,
        bytes memory initialClientState,
        bytes32 initialConsensusState,
        address roleManager
    ) {
        UPDATE_CLIENT_PROGRAM_VKEY = updateClientProgramVkey;
        MEMBERSHIP_PROGRAM_VKEY = membershipProgramVkey;
        UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY = ucAndMembershipProgramVkey;
        VERIFIER = ISP1Verifier(sp1Verifier);

        clientState = abi.decode(initialClientState, (IICSStellarMsgs.ClientState));
        _consensusStateHashes[clientState.latestHeight.revisionHeight] = initialConsensusState;

        if (roleManager == address(0)) {
            _grantRole(PROOF_SUBMITTER_ROLE, address(0));
        } else {
            _grantRole(DEFAULT_ADMIN_ROLE, roleManager);
            _grantRole(PROOF_SUBMITTER_ROLE, roleManager);
            _grantRole(CLIENT_FREEZER_ROLE, roleManager);
        }
    }

    /// @inheritdoc ILightClient
    function getClientState() external view returns (bytes memory) {
        return abi.encode(clientState);
    }

    /// @inheritdoc ISP1ICSStellar
    function getConsensusStateHash(uint64 revisionHeight) public view returns (bytes32) {
        bytes32 hash = _consensusStateHashes[revisionHeight];
        require(hash != 0, ConsensusStateNotFound());
        return hash;
    }

    /// @inheritdoc ILightClient
    function updateClient(bytes calldata updateMsg)
        external
        notFrozen
        onlyProofSubmitter
        returns (ILightClientMsgs.UpdateResult)
    {
        IStellarUpdateClientMsgs.MsgUpdateClient memory message =
            abi.decode(updateMsg, (IStellarUpdateClientMsgs.MsgUpdateClient));

        require(
            message.sp1Proof.vKey == UPDATE_CLIENT_PROGRAM_VKEY,
            VerificationKeyMismatch(UPDATE_CLIENT_PROGRAM_VKEY, message.sp1Proof.vKey)
        );

        IStellarUpdateClientMsgs.UpdateClientOutput memory output =
            abi.decode(message.sp1Proof.publicValues, (IStellarUpdateClientMsgs.UpdateClientOutput));

        _requireOurClientState(output.clientState);
        _requireRevision(output.newHeight.revisionNumber);

        bytes32 trustedHash = keccak256(abi.encode(output.trustedConsensusState));
        bytes32 storedHash = getConsensusStateHash(output.trustedHeight.revisionHeight);
        require(trustedHash == storedHash, ConsensusStateHashMismatch(storedHash, trustedHash));

        bytes32 newHash = keccak256(abi.encode(output.newConsensusState));
        bytes32 existing = _consensusStateHashes[output.newHeight.revisionHeight];

        if (existing != 0) {
            // A second, different consensus state at the same height is exactly what misbehaviour
            // looks like. Until Phase 8 builds misbehaviour proving, refuse it rather than
            // silently overwrite, and leave freezing to the freezer role.
            require(existing == newHash, ConsensusStateConflict(output.newHeight.revisionHeight));

            _verifySP1Proof(message.sp1Proof);

            return ILightClientMsgs.UpdateResult.NoOp;
        }

        _consensusStateHashes[output.newHeight.revisionHeight] = newHash;

        if (output.newHeight.revisionHeight > clientState.latestHeight.revisionHeight) {
            clientState.latestHeight = output.newHeight;
        }

        _verifySP1Proof(message.sp1Proof);

        return ILightClientMsgs.UpdateResult.Update;
    }

    /// @inheritdoc ILightClient
    function verifyMembership(ILightClientMsgs.MsgVerifyMembership calldata msg_)
        external
        notFrozen
        onlyProofSubmitter
        returns (uint256)
    {
        require(msg_.value.length > 0, EmptyValue());
        return _membership(msg_.proof, msg_.proofHeight, msg_.path, msg_.value);
    }

    /// @inheritdoc ILightClient
    function verifyNonMembership(ILightClientMsgs.MsgVerifyNonMembership calldata msg_)
        external
        notFrozen
        onlyProofSubmitter
        returns (uint256)
    {
        return _membership(msg_.proof, msg_.proofHeight, msg_.path, bytes(""));
    }

    /// @inheritdoc ILightClient
    function misbehaviour(bytes calldata) external view notFrozen onlyProofSubmitter {
        revert MisbehaviourNotImplemented();
    }

    /// @inheritdoc ISP1ICSStellar
    function freeze() external onlyRole(CLIENT_FREEZER_ROLE) {
        clientState.isFrozen = true;
    }

    /// @notice Verifies a (non)membership proof against the consensus state stored at a height.
    /// @dev A non-membership request is one whose value is empty.
    /// @param proof The abi encoded SP1MembershipProof.
    /// @param proofHeight The height the proof is anchored to.
    /// @param path The path of the key-value pair.
    /// @param value The value, or empty for non-membership.
    /// @return The close time of the consensus state at that height, in unix seconds.
    function _membership(
        bytes calldata proof,
        IICS02ClientMsgs.Height calldata proofHeight,
        bytes[] calldata path,
        bytes memory value
    )
        private
        returns (uint256)
    {
        _requireRevision(proofHeight.revisionNumber);

        IStellarMembershipMsgs.StellarMembershipProof memory envelope =
            abi.decode(proof, (IStellarMembershipMsgs.StellarMembershipProof));

        if (envelope.proofType == IStellarMembershipMsgs.StellarMembershipProofType.SP1UcAndMembershipProof) {
            return _ucAndMembership(envelope.proof, proofHeight, path, value);
        }

        IStellarMembershipMsgs.SP1MembershipProof memory membershipProof =
            abi.decode(envelope.proof, (IStellarMembershipMsgs.SP1MembershipProof));

        require(
            membershipProof.sp1Proof.vKey == MEMBERSHIP_PROGRAM_VKEY,
            VerificationKeyMismatch(MEMBERSHIP_PROGRAM_VKEY, membershipProof.sp1Proof.vKey)
        );

        bytes32 trustedHash = keccak256(abi.encode(membershipProof.trustedConsensusState));
        bytes32 storedHash = getConsensusStateHash(proofHeight.revisionHeight);
        require(trustedHash == storedHash, ConsensusStateHashMismatch(storedHash, trustedHash));

        IStellarMembershipMsgs.MembershipOutput memory output =
            abi.decode(membershipProof.sp1Proof.publicValues, (IStellarMembershipMsgs.MembershipOutput));

        _requireOurClientState(output.clientState);

        // The anchor. Without this the proof could be of some other ledger's results entirely.
        require(
            output.transactionSetResultHash == membershipProof.trustedConsensusState.transactionSetResultHash,
            ResultHashMismatch(
                membershipProof.trustedConsensusState.transactionSetResultHash, output.transactionSetResultHash
            )
        );

        require(output.kvPairs.length > 0, EmptyKeyValuePairs());
        _requireProvenPair(output.kvPairs, path, value);

        _verifySP1Proof(membershipProof.sp1Proof);

        return membershipProof.trustedConsensusState.closeTime;
    }

    /// @notice Verifies a combined header-and-packet proof, advancing the client as it goes.
    /// @dev The packet is anchored on the transactionSetResultHash of the header proven in the same
    /// run, so unlike the plain membership path there is no stored consensus state to check it
    /// against — the header verification inside the circuit is what establishes it. That is also
    /// why this stores the new consensus state: the proof establishes the height, so refusing to
    /// record it would throw away work already paid for.
    /// @param proof The abi encoded SP1UcAndMembershipProof.
    /// @param proofHeight The height the proof is anchored to, which must be the height it proves.
    /// @param path The path of the key-value pair.
    /// @param value The value, or empty for non-membership.
    /// @return The close time of the newly proven consensus state, in unix seconds.
    function _ucAndMembership(
        bytes memory proof,
        IICS02ClientMsgs.Height calldata proofHeight,
        bytes[] calldata path,
        bytes memory value
    )
        private
        returns (uint256)
    {
        IStellarUcAndMembershipMsgs.SP1UcAndMembershipProof memory combined =
            abi.decode(proof, (IStellarUcAndMembershipMsgs.SP1UcAndMembershipProof));

        require(
            combined.sp1Proof.vKey == UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY,
            VerificationKeyMismatch(UPDATE_CLIENT_AND_MEMBERSHIP_PROGRAM_VKEY, combined.sp1Proof.vKey)
        );

        IStellarUcAndMembershipMsgs.UcAndMembershipOutput memory output =
            abi.decode(combined.sp1Proof.publicValues, (IStellarUcAndMembershipMsgs.UcAndMembershipOutput));

        _requireOurClientState(output.updateClientOutput.clientState);
        _requireRevision(output.updateClientOutput.newHeight.revisionNumber);

        require(
            proofHeight.revisionHeight == output.updateClientOutput.newHeight.revisionHeight,
            ProofHeightMismatch(proofHeight.revisionHeight, output.updateClientOutput.newHeight.revisionHeight)
        );

        bytes32 trustedHash = keccak256(abi.encode(output.updateClientOutput.trustedConsensusState));
        bytes32 storedHash = getConsensusStateHash(output.updateClientOutput.trustedHeight.revisionHeight);
        require(trustedHash == storedHash, ConsensusStateHashMismatch(storedHash, trustedHash));

        require(output.kvPairs.length > 0, EmptyKeyValuePairs());
        _requireProvenPair(output.kvPairs, path, value);

        bytes32 newHash = keccak256(abi.encode(output.updateClientOutput.newConsensusState));
        bytes32 existing = _consensusStateHashes[output.updateClientOutput.newHeight.revisionHeight];

        if (existing == 0) {
            _consensusStateHashes[output.updateClientOutput.newHeight.revisionHeight] = newHash;

            if (output.updateClientOutput.newHeight.revisionHeight > clientState.latestHeight.revisionHeight) {
                clientState.latestHeight = output.updateClientOutput.newHeight;
            }
        } else {
            // Same reasoning as updateClient: a second, different consensus state at a height is
            // misbehaviour, and refusing it is safer than overwriting.
            require(existing == newHash, ConsensusStateConflict(output.updateClientOutput.newHeight.revisionHeight));
        }

        _verifySP1Proof(combined.sp1Proof);

        return output.updateClientOutput.newConsensusState.closeTime;
    }

    /// @notice Requires that the pairs the program proved contain the one being asked about.
    /// @param kvPairs The pairs the program proved.
    /// @param path The path being asked about.
    /// @param value The value being asked about, or empty for non-membership.
    function _requireProvenPair(
        IStellarMembershipMsgs.KVPair[] memory kvPairs,
        bytes[] calldata path,
        bytes memory value
    )
        private
        pure
    {
        bytes32 wanted = keccak256(abi.encode(path, value));

        uint256 total = kvPairs.length;

        for (uint256 index = 0; index < total; ++index) {
            if (keccak256(abi.encode(kvPairs[index].path, kvPairs[index].value)) == wanted) {
                return;
            }
        }

        revert KeyValueMismatch();
    }

    /// @notice Requires that a proof was produced against the parameters this contract pins.
    /// @dev Only the pinned parameters are compared. `latestHeight` moves on every update and
    /// `isFrozen` is this contract's own business, so a proof echoing an older height is normal
    /// and must not be rejected. What must never differ is the network, the trust root, the
    /// router, and the event topic: those four are what make the proof mean anything.
    /// @param supplied The client state the proof echoed back.
    function _requireOurClientState(IICSStellarMsgs.ClientState memory supplied) private view {
        bytes32 ours = keccak256(
            abi.encode(
                clientState.networkId,
                clientState.trustRootFingerprint,
                clientState.routerContractId,
                clientState.rootEventTopic
            )
        );
        bytes32 theirs = keccak256(
            abi.encode(
                supplied.networkId, supplied.trustRootFingerprint, supplied.routerContractId, supplied.rootEventTopic
            )
        );
        require(ours == theirs, ClientStateMismatch(ours, theirs));
    }

    /// @notice Requires that a height carries this client's revision number.
    /// @param revisionNumber The revision number to check.
    function _requireRevision(uint64 revisionNumber) private view {
        uint64 ours = clientState.latestHeight.revisionNumber;
        require(revisionNumber == ours, RevisionMismatch(ours, revisionNumber));
    }

    /// @notice Forwards the proof to the SP1 verifier.
    /// @param proof The SP1 proof.
    function _verifySP1Proof(ISP1Msgs.SP1Proof memory proof) private view {
        VERIFIER.verifyProof(proof.vKey, proof.publicValues, proof.proof);
    }

    /// @notice Reverts if the client is frozen.
    modifier notFrozen() {
        require(!clientState.isFrozen, FrozenClient());
        _;
    }

    /// @notice Reverts if the caller may not submit proofs.
    modifier onlyProofSubmitter() {
        if (!hasRole(PROOF_SUBMITTER_ROLE, address(0))) {
            _checkRole(PROOF_SUBMITTER_ROLE);
        }
        _;
    }
}
