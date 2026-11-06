// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,max-line-length,max-states-count,gas-small-strings

import { Test } from "forge-std/Test.sol";
import { Vm } from "forge-std/Vm.sol";

import { ILightClientMsgs } from "../../contracts/msgs/ILightClientMsgs.sol";
import { IICS02ClientMsgs } from "../../contracts/msgs/IICS02ClientMsgs.sol";
import { IICS26RouterMsgs } from "../../contracts/msgs/IICS26RouterMsgs.sol";
import { IICS27GMPMsgs } from "../../contracts/msgs/IICS27GMPMsgs.sol";
import { IIBCAppCallbacks } from "../../contracts/msgs/IIBCAppCallbacks.sol";
import { IICS20TransferMsgs } from "../../contracts/msgs/IICS20TransferMsgs.sol";

import { IICS26Router } from "../../contracts/interfaces/IICS26Router.sol";

import { ICS20Transfer } from "../../contracts/ICS20Transfer.sol";
import { ICS26Router } from "../../contracts/ICS26Router.sol";
import { FastTransfer } from "../../contracts/fast-transfer/FastTransfer.sol";
import { IFastTransfer } from "../../contracts/fast-transfer/IFastTransfer.sol";
import { ICS20Lib } from "../../contracts/utils/ICS20Lib.sol";
import { ICS24Host } from "../../contracts/utils/ICS24Host.sol";
import { IBCERC20 } from "../../contracts/utils/IBCERC20.sol";
import { Escrow } from "../../contracts/utils/Escrow.sol";
import { IBCRolesLib } from "../../contracts/utils/IBCRolesLib.sol";
import { DummyLightClient } from "../solidity-ibc/mocks/DummyLightClient.sol";
import { TestERC20 } from "../solidity-ibc/mocks/TestERC20.sol";
import { DeployPermit2 } from "@uniswap/permit2/test/utils/DeployPermit2.sol";
import { DeployAccessManagerWithRoles } from "../../scripts/deployments/DeployAccessManagerWithRoles.sol";

import { AccessManager } from "@openzeppelin-contracts/access/manager/AccessManager.sol";
import { ERC1967Proxy } from "@openzeppelin-contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { Strings } from "@openzeppelin-contracts/utils/Strings.sol";

contract NoticeAccountDirectory {
    address public immutable ACCOUNT;

    constructor(address account) {
        ACCOUNT = account;
    }

    function getOrComputeAccountAddress(IICS27GMPMsgs.AccountIdentifier calldata) external view returns (address) {
        return ACCOUNT;
    }
}

contract FastTransferTest is Test, DeployPermit2, DeployAccessManagerWithRoles {
    string internal constant FILL_CONTRACT = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";

    string internal constant RECIPIENT = "GBXDMJBUXZ6WQXMPJXCH4G3XDFQDH4WJZNHAPGXMSOQFFRXSN3RXA22H";

    uint64 internal constant PACKET_TIMEOUT = 3 hours;

    uint64 internal constant GRACE_PERIOD = 1 hours;

    uint256 internal constant AMOUNT = 1_000_000;

    uint256 internal constant FEE = 2500;

    ICS26Router internal router;
    ICS20Transfer internal ics20;
    DummyLightClient internal lightClient;
    FastTransfer internal fast;
    TestERC20 internal token;
    string internal clientId;

    address internal user = makeAddr("user");
    address internal provider = makeAddr("provider");
    address internal noticeAccount = makeAddr("notice account");

    function setUp() public {
        lightClient = new DummyLightClient(ILightClientMsgs.UpdateResult.Update, 0, false);

        AccessManager accessManager = new AccessManager(address(this));

        ERC1967Proxy routerProxy = new ERC1967Proxy(
            address(new ICS26Router()), abi.encodeCall(ICS26Router.initialize, (address(accessManager)))
        );
        ERC1967Proxy transferProxy = new ERC1967Proxy(
            address(new ICS20Transfer()),
            abi.encodeCall(
                ICS20Transfer.initialize,
                (
                    address(routerProxy),
                    address(new Escrow()),
                    address(new IBCERC20()),
                    deployPermit2(),
                    address(accessManager)
                )
            )
        );

        router = ICS26Router(address(routerProxy));
        ics20 = ICS20Transfer(address(transferProxy));

        accessManagerSetTargetRoles(accessManager, address(routerProxy), address(transferProxy), address(0), true);
        accessManager.grantRole(IBCRolesLib.ID_CUSTOMIZER_ROLE, address(this), 0);

        bytes[] memory prefix = new bytes[](2);

        prefix[0] = bytes("ibc");
        prefix[1] = bytes("");

        clientId = router.addClient(IICS02ClientMsgs.CounterpartyInfo("stellar-0", prefix), address(lightClient));
        router.addIBCApp(ICS20Lib.DEFAULT_PORT_ID, address(ics20));

        fast = new FastTransfer(
            address(ics20),
            address(new NoticeAccountDirectory(noticeAccount)),
            clientId,
            FILL_CONTRACT,
            PACKET_TIMEOUT,
            GRACE_PERIOD
        );

        token = new TestERC20();
        token.mint(user, AMOUNT * 10);

        vm.prank(user);
        token.approve(address(fast), type(uint256).max);
    }

    function test_orderKeyMatchesTheStellarVector() public view {
        bytes32 key = fast.orderKeyOf(
            42,
            1_000_000,
            "sp1-stellar-0",
            "0xfff9976782d46cc05630d1f6ebab18b2324d6b14",
            "{\"dest_callback\":{\"address\":\"CFASTFILL\"},\"fast_fill\":\"GRECIPIENT:2500:1600\"}"
        );

        assertEq(key, 0xa3ed8391a5a8e590688183098792b11050c8456560ad9a0b1f110df2363e0dff);
    }

    function test_fastTransferSendsThePacketToTheFillContractWithTheTerms() public {
        (bytes32 key, IICS26RouterMsgs.Packet memory packet) = _open();

        IICS20TransferMsgs.FungibleTokenPacketData memory decoded =
            abi.decode(packet.payloads[0].value, (IICS20TransferMsgs.FungibleTokenPacketData));

        assertEq(decoded.receiver, FILL_CONTRACT);
        assertEq(decoded.sender, Strings.toHexString(address(fast)));
        assertEq(decoded.denom, Strings.toHexString(address(token)));
        assertEq(decoded.amount, AMOUNT);
        assertEq(
            decoded.memo,
            string.concat(
                "{\"dest_callback\":{\"address\":\"",
                FILL_CONTRACT,
                "\"},\"fast_fill\":\"",
                RECIPIENT,
                ":2500:",
                Strings.toString(block.timestamp + 10 minutes),
                "\"}"
            )
        );
        assertEq(key, fast.orderKeyOf(packet.sequence, AMOUNT, clientId, decoded.denom, decoded.memo));
        assertEq(packet.timeoutTimestamp, block.timestamp + PACKET_TIMEOUT);
        assertEq(token.balanceOf(user), AMOUNT * 9);
        assertEq(token.balanceOf(address(fast)), 0);
        assertEq(uint8(fast.orderOf(key).status), uint8(IFastTransfer.Status.Open));
    }

    function test_aDeliveredOrderHasNoRefund() public {
        (bytes32 key, IICS26RouterMsgs.Packet memory packet) = _open();

        _ack(packet, ICS20Lib.SUCCESSFUL_ACKNOWLEDGEMENT_JSON);

        assertEq(uint8(fast.orderOf(key).status), uint8(IFastTransfer.Status.Delivered));

        vm.expectRevert(abi.encodeWithSelector(IFastTransfer.FastTransferNotRefunded.selector, key));
        fast.claimRefund(key);
    }

    function test_anUnfilledFailedOrderRefundsTheUserAfterTheGracePeriod() public {
        (bytes32 key, IICS26RouterMsgs.Packet memory packet) = _open();

        _ack(packet, ICS24Host.UNIVERSAL_ERROR_ACK);

        assertEq(uint8(fast.orderOf(key).status), uint8(IFastTransfer.Status.Refunded));
        assertEq(token.balanceOf(address(fast)), AMOUNT);

        vm.expectRevert(
            abi.encodeWithSelector(IFastTransfer.FastTransferGracePeriod.selector, key, block.timestamp + GRACE_PERIOD)
        );
        fast.claimRefund(key);

        vm.warp(block.timestamp + GRACE_PERIOD);
        fast.claimRefund(key);

        assertEq(token.balanceOf(user), AMOUNT * 10);
        assertEq(uint8(fast.orderOf(key).status), uint8(IFastTransfer.Status.Paid));
    }

    function test_aFilledOrderThatTimesOutRefundsTheProvider() public {
        (bytes32 key, IICS26RouterMsgs.Packet memory packet) = _open();

        _timeout(packet);

        vm.prank(noticeAccount);
        fast.assignRefund(key, provider);

        fast.claimRefund(key);

        assertEq(token.balanceOf(provider), AMOUNT);
        assertEq(token.balanceOf(user), AMOUNT * 9);
    }

    function test_aNoticeThatArrivesBeforeTheRefundStillRoutesItToTheProvider() public {
        (bytes32 key, IICS26RouterMsgs.Packet memory packet) = _open();

        vm.prank(noticeAccount);
        fast.assignRefund(key, provider);

        _ack(packet, ICS24Host.UNIVERSAL_ERROR_ACK);

        fast.claimRefund(key);

        assertEq(token.balanceOf(provider), AMOUNT);
    }

    function test_aNoticeAfterTheUserClaimedIsTooLate() public {
        (bytes32 key, IICS26RouterMsgs.Packet memory packet) = _open();

        _ack(packet, ICS24Host.UNIVERSAL_ERROR_ACK);
        vm.warp(block.timestamp + GRACE_PERIOD);
        fast.claimRefund(key);

        vm.prank(noticeAccount);
        fast.assignRefund(key, provider);

        vm.expectRevert(abi.encodeWithSelector(IFastTransfer.FastTransferNotRefunded.selector, key));
        fast.claimRefund(key);

        assertEq(token.balanceOf(provider), 0);
    }

    function test_onlyTheFillContractsAccountCanAssignARefund() public {
        (bytes32 key,) = _open();

        vm.expectRevert(abi.encodeWithSelector(IFastTransfer.FastTransferUnauthorized.selector, provider));
        vm.prank(provider);
        fast.assignRefund(key, provider);
    }

    function test_aRefundIsAssignedOnce() public {
        (bytes32 key,) = _open();

        vm.startPrank(noticeAccount);
        fast.assignRefund(key, provider);

        vm.expectRevert(abi.encodeWithSelector(IFastTransfer.FastTransferAlreadyAssigned.selector, key));
        fast.assignRefund(key, makeAddr("other"));
        vm.stopPrank();
    }

    function test_anUnknownOrderCannotBeAssigned() public {
        vm.expectRevert(abi.encodeWithSelector(IFastTransfer.FastTransferUnknownOrder.selector, bytes32(uint256(7))));
        vm.prank(noticeAccount);
        fast.assignRefund(bytes32(uint256(7)), provider);
    }

    function test_onlyTheTransferAppCanReportOutcomes() public {
        IIBCAppCallbacks.OnTimeoutPacketCallback memory callback;

        vm.expectRevert(abi.encodeWithSelector(IFastTransfer.FastTransferUnauthorized.selector, user));
        vm.prank(user);
        fast.onTimeoutPacket(callback);
    }

    function test_aFeeMustBeBelowTheAmount() public {
        vm.expectRevert(IFastTransfer.FastTransferInvalidFee.selector);
        vm.prank(user);
        fast.fastTransfer(address(token), AMOUNT, RECIPIENT, AMOUNT, uint64(block.timestamp + 10 minutes));
    }

    function test_theFillDeadlineMustFallBeforeThePacketTimeout() public {
        vm.startPrank(user);

        vm.expectRevert(IFastTransfer.FastTransferInvalidDeadline.selector);
        fast.fastTransfer(address(token), AMOUNT, RECIPIENT, FEE, uint64(block.timestamp));

        vm.expectRevert(IFastTransfer.FastTransferInvalidDeadline.selector);
        fast.fastTransfer(address(token), AMOUNT, RECIPIENT, FEE, uint64(block.timestamp + PACKET_TIMEOUT));

        vm.stopPrank();
    }

    function test_theRecipientMustBeAStellarAddress() public {
        vm.startPrank(user);

        vm.expectRevert(IFastTransfer.FastTransferInvalidRecipient.selector);
        fast.fastTransfer(address(token), AMOUNT, "GSHORT", FEE, uint64(block.timestamp + 10 minutes));

        vm.expectRevert(IFastTransfer.FastTransferInvalidRecipient.selector);
        fast.fastTransfer(
            address(token),
            AMOUNT,
            "GBXDMJBUXZ6WQXMPJXCH4G3XDFQDH4WJZNHAPGXMSOQFFRXSN3RXA2\":",
            FEE,
            uint64(block.timestamp + 10 minutes)
        );

        vm.stopPrank();
    }

    function _open() internal returns (bytes32 key, IICS26RouterMsgs.Packet memory packet) {
        vm.recordLogs();
        vm.prank(user);
        (key,) = fast.fastTransfer(address(token), AMOUNT, RECIPIENT, FEE, uint64(block.timestamp + 10 minutes));

        packet = _sentPacket();
    }

    function _ack(IICS26RouterMsgs.Packet memory packet, bytes memory acknowledgement) internal {
        router.ackPacket(
            IICS26RouterMsgs.MsgAckPacket({
                packet: packet,
                acknowledgement: acknowledgement,
                proofAcked: bytes("proof"),
                proofHeight: IICS02ClientMsgs.Height({ revisionNumber: 0, revisionHeight: 1 })
            })
        );
    }

    function _timeout(IICS26RouterMsgs.Packet memory packet) internal {
        lightClient.setMembershipResult(packet.timeoutTimestamp + 1, false);

        router.timeoutPacket(
            IICS26RouterMsgs.MsgTimeoutPacket({
                packet: packet,
                proofTimeout: bytes("proof"),
                proofHeight: IICS02ClientMsgs.Height({ revisionNumber: 0, revisionHeight: 1 })
            })
        );
    }

    function _sentPacket() internal view returns (IICS26RouterMsgs.Packet memory) {
        Vm.Log[] memory logs = vm.getRecordedLogs();

        for (uint256 i = 0; i < logs.length; ++i) {
            for (uint256 j = 0; j < logs[i].topics.length; ++j) {
                if (logs[i].topics[j] == IICS26Router.SendPacket.selector) {
                    return abi.decode(logs[i].data, (IICS26RouterMsgs.Packet));
                }
            }
        }

        revert("SendPacket event not found");
    }
}
