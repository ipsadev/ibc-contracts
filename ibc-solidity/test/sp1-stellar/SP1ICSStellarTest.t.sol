// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,max-states-count,gas-custom-errors,gas-small-strings

import { Test } from "forge-std/Test.sol";
import { SP1MockVerifier } from "sp1-contracts/contracts/src/SP1MockVerifier.sol";

import { SP1ICSStellar } from "../../contracts/light-clients/sp1-stellar/SP1ICSStellar.sol";
import { ISP1ICSStellarErrors } from "../../contracts/light-clients/sp1-stellar/errors/ISP1ICSStellarErrors.sol";
import { IICSStellarMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IICSStellarMsgs.sol";
import { IStellarUpdateClientMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IStellarUpdateClientMsgs.sol";
import { IStellarMembershipMsgs } from "../../contracts/light-clients/sp1-stellar/msgs/IStellarMembershipMsgs.sol";
import { ISP1Msgs } from "../../contracts/light-clients/sp1-ics07/msgs/ISP1Msgs.sol";
import { ILightClientMsgs } from "../../contracts/msgs/ILightClientMsgs.sol";
import { IICS02ClientMsgs } from "../../contracts/msgs/IICS02ClientMsgs.sol";

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

    function test_AProofAgainstDifferentClientParametersIsRejected() public {
        // Deployed with the membership scenario's client state, which pins a different network,
        // trust root and router than the update proof was produced against.
        _deployForMembership();

        vm.expectRevert();
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
}
