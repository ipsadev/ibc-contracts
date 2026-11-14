// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable gas-indexed-events

import { IICS20Transfer } from "../interfaces/IICS20Transfer.sol";

/// @title Fast Transfer interface
/// @notice Sends ICS-20 transfers that a liquidity provider can pay out early on Stellar, and routes failure refunds to
/// that provider
interface IFastTransfer {
    /// @notice The life cycle of an order
    enum Status {
        None,
        Open,
        Delivered,
        Refunded,
        Paid
    }

    /// @notice An order opened by `fastTransfer`
    /// @param user Who sent the transfer and receives an unfilled refund
    /// @param token The ERC-20 sent
    /// @param amount The amount sent
    /// @param provider Who receives the refund, named by the fill notice
    /// @param refundedAt When the packet's refund arrived
    /// @param status Where the order is in its life cycle
    struct Order {
        address user;
        address token;
        uint256 amount;
        address provider;
        uint64 refundedAt;
        Status status;
    }

    /// @notice Emitted when an order is opened; providers watch it
    /// @param orderKey The order key both chains compute
    /// @param user The sender
    /// @param sequence The packet sequence
    /// @param denom The packet denomination
    /// @param amount The amount sent
    /// @param recipient The Stellar recipient
    /// @param fee What the provider keeps
    /// @param fillDeadline The last moment the order can be filled
    /// @param memo The packet memo
    event OrderOpened(
        bytes32 indexed orderKey,
        address indexed user,
        uint64 sequence,
        string denom,
        uint256 amount,
        string recipient,
        uint256 fee,
        uint64 fillDeadline,
        string memo
    );

    /// @notice Emitted when the packet is acknowledged with success
    /// @param orderKey The order key
    event OrderDelivered(bytes32 indexed orderKey);

    /// @notice Emitted when the packet fails or times out and its refund arrives
    /// @param orderKey The order key
    event OrderRefunded(bytes32 indexed orderKey);

    /// @notice Emitted when a fill notice names the provider
    /// @param orderKey The order key
    /// @param provider Who receives the refund
    event RefundAssigned(bytes32 indexed orderKey, address indexed provider);

    /// @notice Emitted when a refund is paid out
    /// @param orderKey The order key
    /// @param to Who was paid
    /// @param amount The amount paid
    event RefundPaid(bytes32 indexed orderKey, address indexed to, uint256 amount);

    /// @notice The fee is not below the amount
    error FastTransferInvalidFee();

    /// @notice The fill deadline is not in the future or not before the packet timeout
    error FastTransferInvalidDeadline();

    /// @notice The recipient is not a 56-character Stellar address
    error FastTransferInvalidRecipient();

    /// @notice The caller may not call this function
    /// @param caller The caller
    error FastTransferUnauthorized(address caller);

    /// @notice No order has this key
    /// @param orderKey The order key
    error FastTransferUnknownOrder(bytes32 orderKey);

    /// @notice The order's refund already has a provider
    /// @param orderKey The order key
    error FastTransferAlreadyAssigned(bytes32 orderKey);

    /// @notice The order has no refund waiting
    /// @param orderKey The order key
    error FastTransferNotRefunded(bytes32 orderKey);

    /// @notice The refund is still waiting for a fill notice
    /// @param orderKey The order key
    /// @param claimableAt When the user can claim it
    error FastTransferGracePeriod(bytes32 orderKey, uint256 claimableAt);

    /// @notice The ICS-20 transfer app that sends the packets
    /// @return The transfer app
    function ICS20() external view returns (IICS20Transfer);

    /// @notice The ICS-27 account through which the Stellar fill contract sends fill notices
    /// @return The account
    function NOTICE_ACCOUNT() external view returns (address);

    /// @notice How long a packet may take before it times out
    /// @return The timeout in seconds
    function PACKET_TIMEOUT_SECONDS() external view returns (uint64);

    /// @notice How long a refund waits for a fill notice before the user can claim it
    /// @return The grace period in seconds
    function GRACE_PERIOD_SECONDS() external view returns (uint64);

    /// @notice The local client for Stellar that packets are sent over
    /// @return The client id
    function clientId() external view returns (string memory);

    /// @notice The Stellar fill contract that receives the packets
    /// @return The fill contract's address
    function fillContract() external view returns (string memory);

    /// @notice Sends `amount` of `token` to Stellar as an order a provider can fill
    /// @param token The ERC-20 to send
    /// @param amount The amount to send
    /// @param recipient The Stellar recipient
    /// @param fee What the provider keeps
    /// @param fillDeadline The last moment the order can be filled
    /// @return orderKey The order key
    /// @return sequence The packet sequence
    function fastTransfer(
        address token,
        uint256 amount,
        string calldata recipient,
        uint256 fee,
        uint64 fillDeadline
    )
        external
        returns (bytes32 orderKey, uint64 sequence);

    /// @notice Names the provider that filled an order, so its refund goes to them
    /// @param orderKey The order key
    /// @param provider Who receives the refund
    function assignRefund(bytes32 orderKey, address provider) external;

    /// @notice Pays a refund to the provider, or to the user once the grace period has passed
    /// @param orderKey The order key
    function claimRefund(bytes32 orderKey) external;

    /// @notice Returns an order
    /// @param orderKey The order key
    /// @return The order
    function orderOf(bytes32 orderKey) external view returns (Order memory);

    /// @notice Computes the order key both chains use
    /// @param sequence The packet sequence
    /// @param amount The amount
    /// @param sourceClient The local client for Stellar
    /// @param denom The packet denomination
    /// @param memo The packet memo
    /// @return The order key
    function orderKeyOf(
        uint64 sequence,
        uint256 amount,
        string memory sourceClient,
        string memory denom,
        string memory memo
    )
        external
        pure
        returns (bytes32);
}
