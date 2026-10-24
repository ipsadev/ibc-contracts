// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings,reason-string

import { stdJson } from "forge-std/StdJson.sol";
import { Script } from "forge-std/Script.sol";
import { console } from "forge-std/console.sol";

import { AccessManager } from "@openzeppelin-contracts/access/manager/AccessManager.sol";
import { ERC1967Proxy } from "@openzeppelin-contracts/proxy/ERC1967/ERC1967Proxy.sol";

import { ICS26Router } from "../contracts/ICS26Router.sol";
import { IICS02ClientMsgs } from "../contracts/msgs/IICS02ClientMsgs.sol";
import { IBCRolesLib } from "../contracts/utils/IBCRolesLib.sol";

/// @title Deploy the IBC core for the Stellar corridor
/// @notice Deploys an AccessManager and ICS26Router, then registers an existing SP1ICSStellar as
/// the client for the Stellar counterparty.
/// @dev The router is deliberately deployed **unchanged** from upstream: it is chain-agnostic, and
/// only the light client knows anything about Stellar. If this script ever needs to modify the
/// router, something has been put in the wrong place.
///
/// ICS20Transfer and ICS27GMP are not deployed. This exists to give the Stellar light client
/// something to be registered against, so a membership proof has an ICS26Router storage slot to
/// point at. Token transfer is Phase 5's own step and needs the denom and voucher decisions.
///
/// ```sh
/// forge script scripts/DeployIbcCoreForStellar.s.sol --rpc-url "$RPC_URL" \
///   --sig "run(address,string)" <sp1IcsStellar> <counterpartyClientId>
/// ```
contract DeployIbcCoreForStellar is Script {
    using stdJson for string;

    /// @notice Deploys the router and registers the client.
    /// @param stellarClient The already-deployed SP1ICSStellar.
    /// @param counterpartyClientId The client id the Soroban router knows this chain by.
    /// @return router The deployed ICS26Router proxy.
    /// @return clientId The client id the router assigned.
    function run(
        address stellarClient,
        string memory counterpartyClientId
    )
        public
        returns (address router, string memory clientId)
    {
        require(stellarClient != address(0), "the Stellar light client address is unset");
        require(stellarClient.code.length > 0, "no contract at the Stellar light client address");
        require(bytes(counterpartyClientId).length > 0, "the counterparty client id is empty");

        vm.startBroadcast();

        address deployer = msg.sender;

        AccessManager accessManager = new AccessManager(deployer);

        address logic = address(new ICS26Router());

        router = address(new ERC1967Proxy(logic, abi.encodeCall(ICS26Router.initialize, (address(accessManager)))));

        // Only the router's own roles are set. The helper in deployments/ also wires ICS20 and
        // ICS27, which are not deployed here.
        accessManager.setTargetFunctionRole(
            router, IBCRolesLib.ics26IdCustomizerSelectors(), IBCRolesLib.ID_CUSTOMIZER_ROLE
        );
        accessManager.setTargetFunctionRole(router, IBCRolesLib.ics26RelayerSelectors(), IBCRolesLib.RELAYER_ROLE);

        accessManager.grantRole(IBCRolesLib.RELAYER_ROLE, deployer, 0);
        accessManager.grantRole(IBCRolesLib.ID_CUSTOMIZER_ROLE, deployer, 0);

        // The merkle prefix is what the counterparty's proofs are rooted under. The Soroban router
        // keys its SMT by the raw IBC path, so the prefix is empty rather than a store name; a
        // mismatch here surfaces as a proof failure and reads like a cryptography bug.
        bytes[] memory merklePrefix = new bytes[](1);
        merklePrefix[0] = bytes("");

        clientId = ICS26Router(router)
            .addClient(
                IICS02ClientMsgs.CounterpartyInfo({ clientId: counterpartyClientId, merklePrefix: merklePrefix }),
                stellarClient
            );

        vm.stopBroadcast();

        console.log("AccessManager       ", address(accessManager));
        console.log("ICS26Router logic   ", logic);
        console.log("ICS26Router         ", router);
        console.log("Stellar light client", stellarClient);
        console.log("client id           ", clientId);
        console.log("counterparty        ", counterpartyClientId);
    }
}
