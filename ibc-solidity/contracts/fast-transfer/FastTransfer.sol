// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { IICS20Transfer } from "../interfaces/IICS20Transfer.sol";
import { IICS27GMP } from "../interfaces/IICS27GMP.sol";
import { IIBCAppCallbacks } from "../msgs/IIBCAppCallbacks.sol";
import { IIBCSenderCallbacks } from "../interfaces/IIBCSenderCallbacks.sol";
import { IICS20TransferMsgs } from "../msgs/IICS20TransferMsgs.sol";
import { IICS27GMPMsgs } from "../msgs/IICS27GMPMsgs.sol";

import { IFastTransfer } from "./IFastTransfer.sol";

import { IBCCallbackReceiver } from "../utils/IBCCallbackReceiver.sol";
import { ICS20Lib } from "../utils/ICS20Lib.sol";

import { IERC20 } from "@openzeppelin-contracts/token/ERC20/IERC20.sol";
import { SafeERC20 } from "@openzeppelin-contracts/token/ERC20/utils/SafeERC20.sol";
import { ReentrancyGuardTransient } from "@openzeppelin-contracts/utils/ReentrancyGuardTransient.sol";
import { Strings } from "@openzeppelin-contracts/utils/Strings.sol";

/// @title Fast Transfer
/// @notice Sends ICS-20 transfers that a liquidity provider can pay out early on Stellar, and routes failure refunds to
/// that provider
contract FastTransfer is IFastTransfer, IBCCallbackReceiver, ReentrancyGuardTransient {
    using SafeERC20 for IERC20;

    /// @notice The length of a Stellar address
    uint256 private constant STELLAR_ADDRESS_LENGTH = 56;

    /// @inheritdoc IFastTransfer
    IICS20Transfer public immutable ICS20;

    /// @inheritdoc IFastTransfer
    address public immutable NOTICE_ACCOUNT;

    /// @inheritdoc IFastTransfer
    uint64 public immutable PACKET_TIMEOUT_SECONDS;

    /// @inheritdoc IFastTransfer
    uint64 public immutable GRACE_PERIOD_SECONDS;

    /// @inheritdoc IFastTransfer
    string public clientId;

    /// @inheritdoc IFastTransfer
    string public fillContract;

    /// @notice The orders, by order key
    mapping(bytes32 orderKey => Order order) private _orders;

    /// @notice Configures the contract and derives the fill contract's ICS-27 account
    /// @param ics20 The ICS-20 transfer app
    /// @param ics27 The ICS-27 GMP app
    /// @param clientId_ The local client for Stellar
    /// @param fillContract_ The Stellar fill contract
    /// @param packetTimeoutSeconds How long a packet may take
    /// @param gracePeriodSeconds How long a refund waits for a fill notice
    constructor(
        address ics20,
        address ics27,
        string memory clientId_,
        string memory fillContract_,
        uint64 packetTimeoutSeconds,
        uint64 gracePeriodSeconds
    ) {
        ICS20 = IICS20Transfer(ics20);
        PACKET_TIMEOUT_SECONDS = packetTimeoutSeconds;
        GRACE_PERIOD_SECONDS = gracePeriodSeconds;
        clientId = clientId_;
        fillContract = fillContract_;
        NOTICE_ACCOUNT = IICS27GMP(ics27)
            .getOrComputeAccountAddress(
                IICS27GMPMsgs.AccountIdentifier({ clientId: clientId_, sender: fillContract_, salt: "" })
            );
    }

    /// @inheritdoc IFastTransfer
    function fastTransfer(
        address token,
        uint256 amount,
        string calldata recipient,
        uint256 fee,
        uint64 fillDeadline
    )
        external
        nonReentrant
        returns (bytes32 orderKey, uint64 sequence)
    {
        require(fee < amount, FastTransferInvalidFee());
        require(_isStellarAddress(bytes(recipient)), FastTransferInvalidRecipient());

        uint64 timeoutTimestamp = uint64(block.timestamp) + PACKET_TIMEOUT_SECONDS;

        require(fillDeadline > block.timestamp && fillDeadline < timeoutTimestamp, FastTransferInvalidDeadline());

        IERC20(token).safeTransferFrom(msg.sender, address(this), amount);
        IERC20(token).forceApprove(address(ICS20), amount);

        string memory memo = string.concat(
            "{\"dest_callback\":{\"address\":\"",
            fillContract,
            "\"},\"fast_fill\":\"",
            recipient,
            ":",
            Strings.toString(fee),
            ":",
            Strings.toString(fillDeadline),
            "\"}"
        );

        sequence = ICS20.sendTransfer(
            IICS20TransferMsgs.SendTransferMsg({
                denom: token,
                amount: amount,
                receiver: fillContract,
                sourceClient: clientId,
                destPort: ICS20Lib.DEFAULT_PORT_ID,
                timeoutTimestamp: timeoutTimestamp,
                memo: memo
            })
        );

        string memory denom = _packetDenom(token);

        orderKey = orderKeyOf(sequence, amount, clientId, denom, memo);

        _orders[orderKey] = Order({
            user: msg.sender, token: token, amount: amount, provider: address(0), refundedAt: 0, status: Status.Open
        });

        emit OrderOpened(orderKey, msg.sender, sequence, denom, amount, recipient, fee, fillDeadline, memo);
    }

    /// @inheritdoc IFastTransfer
    function assignRefund(bytes32 orderKey, address provider) external {
        require(msg.sender == NOTICE_ACCOUNT, FastTransferUnauthorized(msg.sender));

        Order storage order = _orders[orderKey];

        require(order.status != Status.None, FastTransferUnknownOrder(orderKey));
        require(order.provider == address(0), FastTransferAlreadyAssigned(orderKey));

        order.provider = provider;

        emit RefundAssigned(orderKey, provider);
    }

    /// @inheritdoc IFastTransfer
    function claimRefund(bytes32 orderKey) external nonReentrant {
        Order storage order = _orders[orderKey];

        require(order.status == Status.Refunded, FastTransferNotRefunded(orderKey));

        address to = order.provider;

        if (to == address(0)) {
            uint256 claimableAt = uint256(order.refundedAt) + GRACE_PERIOD_SECONDS;

            require(block.timestamp >= claimableAt, FastTransferGracePeriod(orderKey, claimableAt));

            to = order.user;
        }

        order.status = Status.Paid;

        IERC20(order.token).safeTransfer(to, order.amount);

        emit RefundPaid(orderKey, to, order.amount);
    }

    /// @inheritdoc IIBCSenderCallbacks
    function onAckPacket(bool success, IIBCAppCallbacks.OnAcknowledgementPacketCallback calldata msg_) external {
        require(msg.sender == address(ICS20), FastTransferUnauthorized(msg.sender));

        bytes32 orderKey = _orderKeyOfPacket(msg_.sequence, msg_.sourceClient, msg_.payload.value);

        _settle(orderKey, success);
    }

    /// @inheritdoc IIBCSenderCallbacks
    function onTimeoutPacket(IIBCAppCallbacks.OnTimeoutPacketCallback calldata msg_) external {
        require(msg.sender == address(ICS20), FastTransferUnauthorized(msg.sender));

        bytes32 orderKey = _orderKeyOfPacket(msg_.sequence, msg_.sourceClient, msg_.payload.value);

        _settle(orderKey, false);
    }

    /// @inheritdoc IFastTransfer
    function orderOf(bytes32 orderKey) external view returns (Order memory) {
        return _orders[orderKey];
    }

    /// @inheritdoc IFastTransfer
    function orderKeyOf(
        uint64 sequence,
        uint256 amount,
        string memory sourceClient,
        string memory denom,
        string memory memo
    )
        public
        pure
        returns (bytes32)
    {
        return sha256(
            abi.encodePacked(sequence, amount, sha256(bytes(sourceClient)), sha256(bytes(denom)), sha256(bytes(memo)))
        );
    }

    /// @notice Records a packet's outcome
    /// @param orderKey The order key
    /// @param delivered Whether the packet succeeded
    function _settle(bytes32 orderKey, bool delivered) private {
        Order storage order = _orders[orderKey];

        if (order.status != Status.Open) {
            return;
        }

        if (delivered) {
            order.status = Status.Delivered;

            emit OrderDelivered(orderKey);

            return;
        }

        order.status = Status.Refunded;
        order.refundedAt = uint64(block.timestamp);

        emit OrderRefunded(orderKey);
    }

    /// @notice Computes the order key from a packet
    /// @param sequence The packet sequence
    /// @param sourceClient The local client
    /// @param value The encoded packet data
    /// @return The order key
    function _orderKeyOfPacket(
        uint64 sequence,
        string calldata sourceClient,
        bytes calldata value
    )
        private
        pure
        returns (bytes32)
    {
        IICS20TransferMsgs.FungibleTokenPacketData memory data =
            abi.decode(value, (IICS20TransferMsgs.FungibleTokenPacketData));

        return orderKeyOf(sequence, data.amount, sourceClient, data.denom, data.memo);
    }

    /// @notice Returns the denomination ICS-20 puts in the packet for `token`
    /// @param token The ERC-20
    /// @return The packet denomination
    function _packetDenom(address token) private view returns (string memory) {
        string memory mapped = ICS20.ibcERC20Denom(token);

        if (bytes(mapped).length > 0) {
            return mapped;
        }

        return Strings.toHexString(token);
    }

    /// @notice Checks that `value` has the shape of a Stellar address
    /// @param value The candidate address
    /// @return Whether it is one
    function _isStellarAddress(bytes calldata value) private pure returns (bool) {
        if (value.length != STELLAR_ADDRESS_LENGTH) {
            return false;
        }

        for (uint256 i = 0; i < value.length; ++i) {
            bytes1 character = value[i];
            bool letter = character >= "A" && character <= "Z";
            bool digit = character >= "2" && character <= "7";

            if (!letter && !digit) {
                return false;
            }
        }

        return true;
    }
}
