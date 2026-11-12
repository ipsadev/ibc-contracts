// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,max-states-count,gas-custom-errors,gas-small-strings

import { Test } from "forge-std/Test.sol";
import { stdError } from "forge-std/StdError.sol";
import { SP1MockVerifier } from "sp1-contracts/contracts/src/SP1MockVerifier.sol";

import { SP1ICSStellar } from "../../contracts/light-clients/sp1-stellar/SP1ICSStellar.sol";
import { ISP1ICSStellarErrors } from "../../contracts/light-clients/sp1-stellar/errors/ISP1ICSStellarErrors.sol";
import { ISP1ICSStellar } from "../../contracts/light-clients/sp1-stellar/interfaces/ISP1ICSStellar.sol";
import { IICSStellarMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IICSStellarMsgs.sol";
import { IStellarUpdateClientMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IStellarUpdateClientMsgs.sol";
import { IStellarMembershipMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IStellarMembershipMsgs.sol";
import {
    IStellarUcAndMembershipMsgs
} from "../../contracts/light-clients/sp1-stellar/msgs/IStellarUcAndMembershipMsgs.sol";
import { ISP1Msgs } from "../../contracts/light-clients/sp1-ics07/msgs/ISP1Msgs.sol";
import { ILightClientMsgs } from "../../contracts/msgs/ILightClientMsgs.sol";
import { IICS02ClientMsgs } from "../../contracts/msgs/IICS02ClientMsgs.sol";
import { IAccessControl } from "@openzeppelin-contracts/access/IAccessControl.sol";

/// @title SP1 ICS Stellar light client tests
/// @notice Every encoded value here was produced by the Rust programs, so these tests fail if the
/// Solidity and Rust views of the ABI ever disagree.
contract SP1ICSStellarTest is Test {
    bytes32 public constant UPDATE_VKEY = bytes32(uint256(0x1111));
    bytes32 public constant MEMBERSHIP_VKEY = bytes32(uint256(0x2222));
    bytes32 public constant UC_MEMBERSHIP_VKEY = bytes32(uint256(0x3333));

    SP1ICSStellar public client;
    SP1MockVerifier public verifier;

    bytes public updateClientState;
    bytes public updateTrustedConsensusState;
    bytes public updateNewConsensusState;
    uint64 public updateTrustedHeight;
    uint64 public updateNewHeight;
    bytes public updateClientPublicValues;

    bytes public membershipClientState;
    bytes public membershipConsensusState;
    bytes public membershipPublicValues;
    bytes public nonMembershipPublicValues;
    bytes public membershipPath;
    bytes public membershipValue;
    bytes public nonMembershipPath;

    function setUp() public {
        string memory json = vm.readFile("test/sp1-stellar/fixtures/stellar.json");

        updateClientState = vm.parseJsonBytes(json, ".updateClientState");
        updateTrustedConsensusState = vm.parseJsonBytes(json, ".updateTrustedConsensusState");
        updateNewConsensusState = vm.parseJsonBytes(json, ".updateNewConsensusState");
        updateTrustedHeight = uint64(vm.parseJsonUint(json, ".updateTrustedHeight"));
        updateNewHeight = uint64(vm.parseJsonUint(json, ".updateNewHeight"));
        updateClientPublicValues = vm.parseJsonBytes(json, ".updateClientPublicValues");

        membershipClientState = vm.parseJsonBytes(json, ".membershipClientState");
        membershipConsensusState = vm.parseJsonBytes(json, ".membershipConsensusState");
        membershipPublicValues = vm.parseJsonBytes(json, ".membershipPublicValues");
        nonMembershipPublicValues = vm.parseJsonBytes(json, ".nonMembershipPublicValues");
        membershipPath = vm.parseJsonBytes(json, ".membershipPath");
        membershipValue = vm.parseJsonBytes(json, ".membershipValue");
        nonMembershipPath = vm.parseJsonBytes(json, ".nonMembershipPath");

        verifier = new SP1MockVerifier();

        vm.warp(_updateOutput().newConsensusState.closeTime);
    }

    function _deployForUpdate() internal {
        client = new SP1ICSStellar(
            UPDATE_VKEY,
            MEMBERSHIP_VKEY,
            UC_MEMBERSHIP_VKEY,
            address(verifier),
            updateClientState,
            keccak256(updateTrustedConsensusState),
            address(0)
        );
    }

    function _deployForMembership() internal {
        client = new SP1ICSStellar(
            UPDATE_VKEY,
            MEMBERSHIP_VKEY,
            UC_MEMBERSHIP_VKEY,
            address(verifier),
            membershipClientState,
            keccak256(membershipConsensusState),
            address(0)
        );
    }

    function _updateMessage(bytes32 vkey) internal view returns (bytes memory) {
        return abi.encode(
            IStellarUpdateClientMsgs.MsgUpdateClient(ISP1Msgs.SP1Proof(vkey, updateClientPublicValues, bytes("")))
        );
    }

    function _membershipProof(bytes32 vkey, bytes memory publicValues) internal view returns (bytes memory) {
        bytes memory inner = abi.encode(
            IStellarMembershipMsgs.SP1MembershipProof(
                ISP1Msgs.SP1Proof(vkey, publicValues, bytes("")),
                abi.decode(membershipConsensusState, (IICSStellarMsgs.ConsensusState))
            )
        );

        return abi.encode(
            IStellarMembershipMsgs.StellarMembershipProof(
                IStellarMembershipMsgs.StellarMembershipProofType.SP1MembershipProof, inner
            )
        );
    }

    function _pathOf(bytes memory single) internal pure returns (bytes[] memory) {
        bytes[] memory path = new bytes[](1);
        path[0] = single;
        return path;
    }

    function test_TheRustClientStateDecodesInSolidity() public view {
        IICSStellarMsgs.ClientState memory state = abi.decode(updateClientState, (IICSStellarMsgs.ClientState));

        assertEq(state.rootEventTopic, bytes("ibc_root"), "pinned topic survives ABI");
        assertEq(state.latestHeight.revisionHeight, updateTrustedHeight, "trusted height survives");
        assertFalse(state.isFrozen, "fresh client not frozen");
    }

    function test_UpdateClientStoresTheNewConsensusState() public {
        _deployForUpdate();

        ILightClientMsgs.UpdateResult result = client.updateClient(_updateMessage(UPDATE_VKEY));

        assertEq(uint8(result), uint8(ILightClientMsgs.UpdateResult.Update), "fresh height is an update");
        assertEq(
            client.getConsensusStateHash(updateNewHeight),
            keccak256(updateNewConsensusState),
            "stored hash is committed hash"
        );

        IICSStellarMsgs.ClientState memory state = abi.decode(client.getClientState(), (IICSStellarMsgs.ClientState));
        assertEq(state.latestHeight.revisionHeight, updateNewHeight, "latest height advances");
    }

    function test_ReplayingTheSameUpdateIsANoOp() public {
        _deployForUpdate();

        client.updateClient(_updateMessage(UPDATE_VKEY));

        ILightClientMsgs.UpdateResult again = client.updateClient(_updateMessage(UPDATE_VKEY));

        assertEq(uint8(again), uint8(ILightClientMsgs.UpdateResult.NoOp), "replay is a no-op");
    }

    function test_AProofFromAnotherProgramIsRejected() public {
        _deployForUpdate();

        vm.expectRevert(
            abi.encodeWithSelector(ISP1ICSStellarErrors.VerificationKeyMismatch.selector, UPDATE_VKEY, bytes32(0))
        );
        client.updateClient(_updateMessage(bytes32(0)));
    }

    /// @dev Deployed with the membership scenario's client state, which pins a different network, trust root and
    /// router than the update proof was produced against.
    function test_AProofAgainstDifferentClientParametersIsRejected() public {
        _deployForMembership();

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.ClientStateMismatch.selector,
                _pinnedHash(membershipClientState),
                _pinnedHash(updateClientState)
            )
        );
        client.updateClient(_updateMessage(UPDATE_VKEY));
    }

    function test_AFrozenClientVerifiesNothing() public {
        client = new SP1ICSStellar(
            UPDATE_VKEY,
            MEMBERSHIP_VKEY,
            UC_MEMBERSHIP_VKEY,
            address(verifier),
            updateClientState,
            keccak256(updateTrustedConsensusState),
            address(this)
        );

        client.freeze();

        vm.expectRevert(ISP1ICSStellarErrors.FrozenClient.selector);
        client.updateClient(_updateMessage(UPDATE_VKEY));
    }

    function test_MembershipOfAPacketCommitment() public {
        _deployForMembership();

        uint256 closeTime = client.verifyMembership(
            ILightClientMsgs.MsgVerifyMembership({
                proof: _membershipProof(MEMBERSHIP_VKEY, membershipPublicValues),
                proofHeight: IICS02ClientMsgs.Height(0, _membershipHeight()),
                path: _pathOf(membershipPath),
                value: membershipValue
            })
        );

        assertEq(closeTime, 1_700_000_000, "close time returned");
    }

    function test_NonMembershipOfAnAbsentKey() public {
        _deployForMembership();

        client.verifyNonMembership(
            ILightClientMsgs.MsgVerifyNonMembership({
                proof: _membershipProof(MEMBERSHIP_VKEY, nonMembershipPublicValues),
                proofHeight: IICS02ClientMsgs.Height(0, _membershipHeight()),
                path: _pathOf(nonMembershipPath)
            })
        );
    }

    function test_AProofOfADifferentPathIsRejected() public {
        _deployForMembership();

        vm.expectRevert(ISP1ICSStellarErrors.KeyValueMismatch.selector);
        client.verifyMembership(
            ILightClientMsgs.MsgVerifyMembership({
                proof: _membershipProof(MEMBERSHIP_VKEY, membershipPublicValues),
                proofHeight: IICS02ClientMsgs.Height(0, _membershipHeight()),
                path: _pathOf(bytes("commitments/ports/transfer/channels/0/7")),
                value: membershipValue
            })
        );
    }

    function test_AProofOfADifferentValueIsRejected() public {
        _deployForMembership();

        vm.expectRevert(ISP1ICSStellarErrors.KeyValueMismatch.selector);
        client.verifyMembership(
            ILightClientMsgs.MsgVerifyMembership({
                proof: _membershipProof(MEMBERSHIP_VKEY, membershipPublicValues),
                proofHeight: IICS02ClientMsgs.Height(0, _membershipHeight()),
                path: _pathOf(membershipPath),
                value: bytes("tampered")
            })
        );
    }

    function test_AMembershipProofFromAnotherProgramIsRejected() public {
        _deployForMembership();

        vm.expectRevert(
            abi.encodeWithSelector(ISP1ICSStellarErrors.VerificationKeyMismatch.selector, MEMBERSHIP_VKEY, bytes32(0))
        );
        client.verifyMembership(
            ILightClientMsgs.MsgVerifyMembership({
                proof: _membershipProof(bytes32(0), membershipPublicValues),
                proofHeight: IICS02ClientMsgs.Height(0, _membershipHeight()),
                path: _pathOf(membershipPath),
                value: membershipValue
            })
        );
    }

    function test_MisbehaviourIsNotImplementedYet() public {
        _deployForUpdate();

        vm.expectRevert(ISP1ICSStellarErrors.MisbehaviourNotImplemented.selector);
        client.misbehaviour(bytes(""));
    }

    function _membershipHeight() internal view returns (uint64) {
        IICSStellarMsgs.ClientState memory state = abi.decode(membershipClientState, (IICSStellarMsgs.ClientState));
        return state.latestHeight.revisionHeight;
    }

    function test_AConflictingUpdateFreezesTheClient() public {
        _deployForUpdate();
        client.updateClient(_updateMessage(UPDATE_VKEY));

        IStellarUpdateClientMsgs.UpdateClientOutput memory conflicting = _updateOutput();
        conflicting.newConsensusState.ledgerHash = keccak256("a different ledger");

        vm.expectEmit(address(client));
        emit ISP1ICSStellar.ClientFrozen(address(this));
        ILightClientMsgs.UpdateResult result = client.updateClient(_updateMessageFor(conflicting, bytes("")));

        assertEq(uint8(result), uint8(ILightClientMsgs.UpdateResult.Misbehaviour), "conflict is misbehaviour");
        assertTrue(_isFrozen(), "client frozen");
        assertEq(
            client.getConsensusStateHash(updateNewHeight),
            keccak256(updateNewConsensusState),
            "stored consensus state kept"
        );

        vm.expectRevert(ISP1ICSStellarErrors.FrozenClient.selector);
        client.updateClient(_updateMessage(UPDATE_VKEY));
    }

    function test_AConflictingUpdateIsVerifiedBeforeFreezing() public {
        _deployForUpdate();
        client.updateClient(_updateMessage(UPDATE_VKEY));

        IStellarUpdateClientMsgs.UpdateClientOutput memory conflicting = _updateOutput();
        conflicting.newConsensusState.ledgerHash = keccak256("a different ledger");

        vm.expectRevert(stdError.assertionError);
        client.updateClient(_updateMessageFor(conflicting, bytes("not a proof")));

        assertFalse(_isFrozen(), "an unverified conflict freezes nothing");
    }

    function test_AnUpdateFromTheFutureIsRejected() public {
        _deployForUpdate();

        uint64 closeTime = _updateOutput().newConsensusState.closeTime;
        vm.warp(closeTime - client.ALLOWED_CLOCK_DRIFT() - 1);

        vm.expectRevert(
            abi.encodeWithSelector(ISP1ICSStellarErrors.ProofIsInTheFuture.selector, block.timestamp, closeTime)
        );
        client.updateClient(_updateMessage(UPDATE_VKEY));
    }

    function test_AnUpdateWithinTheClockDriftIsAccepted() public {
        _deployForUpdate();

        vm.warp(_updateOutput().newConsensusState.closeTime - client.ALLOWED_CLOCK_DRIFT());

        ILightClientMsgs.UpdateResult result = client.updateClient(_updateMessage(UPDATE_VKEY));

        assertEq(uint8(result), uint8(ILightClientMsgs.UpdateResult.Update), "within drift is an update");
    }

    function test_AnUpdateWithAnotherRevisionIsRejected() public {
        _deployForUpdate();

        IStellarUpdateClientMsgs.UpdateClientOutput memory output = _updateOutput();
        output.newHeight.revisionNumber = 1;

        vm.expectRevert(abi.encodeWithSelector(ISP1ICSStellarErrors.RevisionMismatch.selector, 0, 1));
        client.updateClient(_updateMessageFor(output, bytes("")));
    }

    function test_AnUpdateFromAnUnknownTrustedStateIsRejected() public {
        _deployForUpdate();

        IStellarUpdateClientMsgs.UpdateClientOutput memory output = _updateOutput();
        output.trustedConsensusState.ledgerHash = keccak256("not the trusted ledger");

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.ConsensusStateHashMismatch.selector,
                keccak256(updateTrustedConsensusState),
                keccak256(abi.encode(output.trustedConsensusState))
            )
        );
        client.updateClient(_updateMessageFor(output, bytes("")));
    }

    function test_OnlyAProofSubmitterMayUpdate() public {
        _deployForUpdateWith(address(this));
        address stranger = makeAddr("stranger");

        bytes32 submitterRole = client.PROOF_SUBMITTER_ROLE();

        vm.prank(stranger);
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, stranger, submitterRole)
        );
        client.updateClient(_updateMessage(UPDATE_VKEY));
    }

    function test_OnlyAProofSubmitterMayVerifyMembership() public {
        client = new SP1ICSStellar(
            UPDATE_VKEY,
            MEMBERSHIP_VKEY,
            UC_MEMBERSHIP_VKEY,
            address(verifier),
            membershipClientState,
            keccak256(membershipConsensusState),
            address(this)
        );
        address stranger = makeAddr("stranger");

        bytes32 submitterRole = client.PROOF_SUBMITTER_ROLE();

        vm.prank(stranger);
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, stranger, submitterRole)
        );
        client.verifyMembership(_packetMembership(_membershipProof(MEMBERSHIP_VKEY, membershipPublicValues)));
    }

    function test_OnlyAFreezerMayFreeze() public {
        _deployForUpdateWith(address(this));
        address stranger = makeAddr("stranger");

        bytes32 freezerRole = client.CLIENT_FREEZER_ROLE();

        vm.prank(stranger);
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, stranger, freezerRole)
        );
        client.freeze();

        assertFalse(_isFrozen(), "not frozen");
    }

    function test_FreezingEmitsClientFrozen() public {
        _deployForUpdateWith(address(this));

        vm.expectEmit(address(client));
        emit ISP1ICSStellar.ClientFrozen(address(this));
        client.freeze();

        assertTrue(_isFrozen(), "frozen");
    }

    function test_WithoutARoleManagerAnyoneSubmitsAndNobodyFreezes() public {
        _deployForUpdate();
        address stranger = makeAddr("stranger");

        vm.prank(stranger);
        client.updateClient(_updateMessage(UPDATE_VKEY));

        assertFalse(client.hasRole(client.DEFAULT_ADMIN_ROLE(), address(this)), "no admin");
        assertFalse(client.hasRole(client.CLIENT_FREEZER_ROLE(), address(0)), "no open freezer");

        bytes32 freezerRole = client.CLIENT_FREEZER_ROLE();

        vm.prank(stranger);
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, stranger, freezerRole)
        );
        client.freeze();

        assertFalse(_isFrozen(), "not frozen");
    }

    function test_MembershipWithAnEmptyValueIsRejected() public {
        _deployForMembership();

        ILightClientMsgs.MsgVerifyMembership memory message =
            _packetMembership(_membershipProof(MEMBERSHIP_VKEY, membershipPublicValues));
        message.value = bytes("");

        vm.expectRevert(ISP1ICSStellarErrors.EmptyValue.selector);
        client.verifyMembership(message);
    }

    function test_MembershipWithAnotherRevisionIsRejected() public {
        _deployForMembership();

        ILightClientMsgs.MsgVerifyMembership memory message =
            _packetMembership(_membershipProof(MEMBERSHIP_VKEY, membershipPublicValues));
        message.proofHeight.revisionNumber = 1;

        vm.expectRevert(abi.encodeWithSelector(ISP1ICSStellarErrors.RevisionMismatch.selector, 0, 1));
        client.verifyMembership(message);
    }

    function test_MembershipAgainstAnUnknownConsensusStateIsRejected() public {
        _deployForMembership();

        IICSStellarMsgs.ConsensusState memory unknown =
            abi.decode(membershipConsensusState, (IICSStellarMsgs.ConsensusState));
        unknown.ledgerHash = keccak256("not the stored ledger");

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.ConsensusStateHashMismatch.selector,
                keccak256(membershipConsensusState),
                keccak256(abi.encode(unknown))
            )
        );
        client.verifyMembership(
            _packetMembership(_membershipProofAgainst(MEMBERSHIP_VKEY, membershipPublicValues, unknown))
        );
    }

    function test_MembershipAnchoredOnAnotherResultHashIsRejected() public {
        IICSStellarMsgs.ConsensusState memory stored =
            abi.decode(membershipConsensusState, (IICSStellarMsgs.ConsensusState));
        stored.transactionSetResultHash = keccak256("another ledger's results");

        client = new SP1ICSStellar(
            UPDATE_VKEY,
            MEMBERSHIP_VKEY,
            UC_MEMBERSHIP_VKEY,
            address(verifier),
            membershipClientState,
            keccak256(abi.encode(stored)),
            address(0)
        );

        IStellarMembershipMsgs.MembershipOutput memory output =
            abi.decode(membershipPublicValues, (IStellarMembershipMsgs.MembershipOutput));

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.ResultHashMismatch.selector,
                stored.transactionSetResultHash,
                output.transactionSetResultHash
            )
        );
        client.verifyMembership(
            _packetMembership(_membershipProofAgainst(MEMBERSHIP_VKEY, membershipPublicValues, stored))
        );
    }

    function test_MembershipWithNoProvenPairsIsRejected() public {
        _deployForMembership();

        IStellarMembershipMsgs.MembershipOutput memory output =
            abi.decode(membershipPublicValues, (IStellarMembershipMsgs.MembershipOutput));
        output.kvPairs = new IStellarMembershipMsgs.KVPair[](0);

        vm.expectRevert(ISP1ICSStellarErrors.EmptyKeyValuePairs.selector);
        client.verifyMembership(_packetMembership(_membershipProof(MEMBERSHIP_VKEY, abi.encode(output))));
    }

    function test_ACombinedProofAdvancesTheClientAndProvesThePacket() public {
        _deployForUpdate();

        uint256 closeTime = client.verifyMembership(_combinedMembership(_combinedOutput(), updateNewHeight));

        assertEq(closeTime, _updateOutput().newConsensusState.closeTime, "close time of the proven ledger");
        assertEq(
            client.getConsensusStateHash(updateNewHeight),
            keccak256(updateNewConsensusState),
            "combined proof stores the consensus state"
        );

        IICSStellarMsgs.ClientState memory state = abi.decode(client.getClientState(), (IICSStellarMsgs.ClientState));
        assertEq(state.latestHeight.revisionHeight, updateNewHeight, "latest height advances");
    }

    function test_ACombinedProofOfAStoredHeightStillProvesThePacket() public {
        _deployForUpdate();
        client.updateClient(_updateMessage(UPDATE_VKEY));

        uint256 closeTime = client.verifyMembership(_combinedMembership(_combinedOutput(), updateNewHeight));

        assertEq(closeTime, _updateOutput().newConsensusState.closeTime, "replayed height still proves");
    }

    function test_ACombinedProofOfAnotherHeightIsRejected() public {
        _deployForUpdate();

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.ProofHeightMismatch.selector, updateNewHeight + 1, updateNewHeight
            )
        );
        client.verifyMembership(_combinedMembership(_combinedOutput(), updateNewHeight + 1));
    }

    function test_ACombinedProofThatConflictsIsRejected() public {
        _deployForUpdate();
        client.updateClient(_updateMessage(UPDATE_VKEY));

        IStellarUcAndMembershipMsgs.UcAndMembershipOutput memory output = _combinedOutput();
        output.updateClientOutput.newConsensusState.ledgerHash = keccak256("a different ledger");

        vm.expectRevert(abi.encodeWithSelector(ISP1ICSStellarErrors.ConsensusStateConflict.selector, updateNewHeight));
        client.verifyMembership(_combinedMembership(output, updateNewHeight));

        assertFalse(_isFrozen(), "a membership call never freezes");
    }

    function test_ACombinedProofFromAnotherProgramIsRejected() public {
        _deployForUpdate();

        ILightClientMsgs.MsgVerifyMembership memory message = _combinedMembership(_combinedOutput(), updateNewHeight);
        message.proof = _combinedProof(MEMBERSHIP_VKEY, abi.encode(_combinedOutput()));

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.VerificationKeyMismatch.selector, UC_MEMBERSHIP_VKEY, MEMBERSHIP_VKEY
            )
        );
        client.verifyMembership(message);
    }

    function test_ACombinedProofFromAnUnknownTrustedStateIsRejected() public {
        _deployForUpdate();

        IStellarUcAndMembershipMsgs.UcAndMembershipOutput memory output = _combinedOutput();
        output.updateClientOutput.trustedConsensusState.ledgerHash = keccak256("not the trusted ledger");

        vm.expectRevert(
            abi.encodeWithSelector(
                ISP1ICSStellarErrors.ConsensusStateHashMismatch.selector,
                keccak256(updateTrustedConsensusState),
                keccak256(abi.encode(output.updateClientOutput.trustedConsensusState))
            )
        );
        client.verifyMembership(_combinedMembership(output, updateNewHeight));
    }

    function test_ACombinedProofWithNoProvenPairsIsRejected() public {
        _deployForUpdate();

        IStellarUcAndMembershipMsgs.UcAndMembershipOutput memory output = _combinedOutput();
        output.kvPairs = new IStellarMembershipMsgs.KVPair[](0);

        vm.expectRevert(ISP1ICSStellarErrors.EmptyKeyValuePairs.selector);
        client.verifyMembership(_combinedMembership(output, updateNewHeight));
    }

    function test_ACombinedProofFromTheFutureIsRejected() public {
        _deployForUpdate();

        uint64 closeTime = _updateOutput().newConsensusState.closeTime;
        vm.warp(closeTime - client.ALLOWED_CLOCK_DRIFT() - 1);

        vm.expectRevert(
            abi.encodeWithSelector(ISP1ICSStellarErrors.ProofIsInTheFuture.selector, block.timestamp, closeTime)
        );
        client.verifyMembership(_combinedMembership(_combinedOutput(), updateNewHeight));
    }

    function _deployForUpdateWith(address roleManager) internal {
        client = new SP1ICSStellar(
            UPDATE_VKEY,
            MEMBERSHIP_VKEY,
            UC_MEMBERSHIP_VKEY,
            address(verifier),
            updateClientState,
            keccak256(updateTrustedConsensusState),
            roleManager
        );
    }

    function _updateOutput() internal view returns (IStellarUpdateClientMsgs.UpdateClientOutput memory) {
        return abi.decode(updateClientPublicValues, (IStellarUpdateClientMsgs.UpdateClientOutput));
    }

    function _updateMessageFor(
        IStellarUpdateClientMsgs.UpdateClientOutput memory output,
        bytes memory proofBytes
    )
        internal
        pure
        returns (bytes memory)
    {
        return abi.encode(
            IStellarUpdateClientMsgs.MsgUpdateClient(ISP1Msgs.SP1Proof(UPDATE_VKEY, abi.encode(output), proofBytes))
        );
    }

    function _membershipProofAgainst(
        bytes32 vkey,
        bytes memory publicValues,
        IICSStellarMsgs.ConsensusState memory trusted
    )
        internal
        pure
        returns (bytes memory)
    {
        bytes memory inner = abi.encode(
            IStellarMembershipMsgs.SP1MembershipProof(ISP1Msgs.SP1Proof(vkey, publicValues, bytes("")), trusted)
        );

        return abi.encode(
            IStellarMembershipMsgs.StellarMembershipProof(
                IStellarMembershipMsgs.StellarMembershipProofType.SP1MembershipProof, inner
            )
        );
    }

    function _packetMembership(bytes memory proof) internal view returns (ILightClientMsgs.MsgVerifyMembership memory) {
        return ILightClientMsgs.MsgVerifyMembership({
            proof: proof,
            proofHeight: IICS02ClientMsgs.Height(0, _membershipHeight()),
            path: _pathOf(membershipPath),
            value: membershipValue
        });
    }

    function _combinedOutput() internal view returns (IStellarUcAndMembershipMsgs.UcAndMembershipOutput memory) {
        IStellarMembershipMsgs.KVPair[] memory kvPairs = new IStellarMembershipMsgs.KVPair[](1);
        kvPairs[0] = IStellarMembershipMsgs.KVPair(_pathOf(membershipPath), membershipValue);

        return IStellarUcAndMembershipMsgs.UcAndMembershipOutput(_updateOutput(), keccak256("ibc root"), kvPairs);
    }

    function _combinedProof(bytes32 vkey, bytes memory publicValues) internal pure returns (bytes memory) {
        bytes memory inner = abi.encode(
            IStellarUcAndMembershipMsgs.SP1UcAndMembershipProof(ISP1Msgs.SP1Proof(vkey, publicValues, bytes("")))
        );

        return abi.encode(
            IStellarMembershipMsgs.StellarMembershipProof(
                IStellarMembershipMsgs.StellarMembershipProofType.SP1UcAndMembershipProof, inner
            )
        );
    }

    function _combinedMembership(
        IStellarUcAndMembershipMsgs.UcAndMembershipOutput memory output,
        uint64 proofHeight
    )
        internal
        view
        returns (ILightClientMsgs.MsgVerifyMembership memory)
    {
        return ILightClientMsgs.MsgVerifyMembership({
            proof: _combinedProof(UC_MEMBERSHIP_VKEY, abi.encode(output)),
            proofHeight: IICS02ClientMsgs.Height(0, proofHeight),
            path: _pathOf(membershipPath),
            value: membershipValue
        });
    }

    function _isFrozen() internal view returns (bool) {
        return abi.decode(client.getClientState(), (IICSStellarMsgs.ClientState)).isFrozen;
    }

    function _pinnedHash(bytes memory encodedClientState) internal pure returns (bytes32) {
        IICSStellarMsgs.ClientState memory state = abi.decode(encodedClientState, (IICSStellarMsgs.ClientState));

        return keccak256(
            abi.encode(state.networkId, state.trustRootFingerprint, state.routerContractId, state.rootEventTopic)
        );
    }
}
