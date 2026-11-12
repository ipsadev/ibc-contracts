// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings,reason-string

import { Script } from "forge-std/Script.sol";
import { console } from "forge-std/console.sol";

import { ERC1967Proxy } from "@openzeppelin-contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { IERC20 } from "@openzeppelin-contracts/token/ERC20/IERC20.sol";

import { ICS20Transfer } from "../contracts/ICS20Transfer.sol";
import { ICS26Router } from "../contracts/ICS26Router.sol";
import { Escrow } from "../contracts/utils/Escrow.sol";
import { IBCERC20 } from "../contracts/utils/IBCERC20.sol";
import { ICS20Lib } from "../contracts/utils/ICS20Lib.sol";
import { IICS20TransferMsgs } from "../contracts/msgs/IICS20TransferMsgs.sol";
import { TestERC20 } from "../test/solidity-ibc/mocks/TestERC20.sol";

/// @title Send a packet toward Stellar
/// @notice Deploys ICS20Transfer against an existing router and sends one transfer, so the router
/// holds a real packet commitment.
/// @dev The point is the commitment, not the transfer. The Ethereum membership guest proves a
/// storage slot in `ICS26Router`, and until something writes one there is nothing to prove — no
/// fixture can substitute for it, because the guest checks the slot against the ICS-26 commitment
/// path and the router's own address.
///
/// The token is a `TestERC20` minted to the sender. A real corridor needs the denom and voucher
/// decisions that are still open; this deliberately does not make them.
///
/// ```sh
/// forge script scripts/SendStellarPacket.s.sol --rpc-url "$RPC_URL" \
///   --sig "run(address,string)" <ics26Router> <sourceClientId>
/// ```
contract SendStellarPacket is Script {
    /// @notice Deploys the transfer app and sends one packet.
    /// @param routerAddress The already-deployed ICS26Router.
    /// @param sourceClient The client id to send over.
    /// @return sequence The sequence of the packet that was sent.
    function run(address routerAddress, string memory sourceClient) public returns (uint64 sequence) {
        require(routerAddress.code.length > 0, "no contract at the router address");

        ICS26Router router = ICS26Router(routerAddress);

        require(address(router.getClient(sourceClient)) != address(0), "the router has no client with that id");

        vm.startBroadcast();

        address sender = msg.sender;

        address transferLogic = address(new ICS20Transfer());

        address transfer = address(
            new ERC1967Proxy(
                transferLogic,
                abi.encodeCall(
                    ICS20Transfer.initialize,
                    (routerAddress, address(new Escrow()), address(new IBCERC20()), address(0), router.authority())
                )
            )
        );

        router.addIBCApp(ICS20Lib.DEFAULT_PORT_ID, transfer);

        TestERC20 token = new TestERC20();

        token.mint(sender, 1_000_000);
        IERC20(address(token)).approve(transfer, 1_000_000);

        sequence = ICS20Transfer(transfer)
            .sendTransfer(
                IICS20TransferMsgs.SendTransferMsg({
                    denom: address(token),
                    amount: 1000,
                    // A Stellar account, as the Soroban router would see it.
                    receiver: "GB2ONJBDQPX5JI7EG4XO4SIA4IMZNJ56Z3TBE2PVIOQ5FFQZEL3R3WU7",
                    sourceClient: sourceClient,
                    destPort: ICS20Lib.DEFAULT_PORT_ID,
                    timeoutTimestamp: uint64(block.timestamp) + 1 hours,
                    memo: ""
                })
            );

        vm.stopBroadcast();

        console.log("ICS20Transfer       ", transfer);
        console.log("TestERC20           ", address(token));
        console.log("source client       ", sourceClient);
        console.log("sequence            ", sequence);
        console.log("");
        console.log("The commitment now in the router's storage is what the Ethereum membership");
        console.log("guest proves. Read it with eth_getProof against the router.");
    }
}
