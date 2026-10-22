// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings,reason-string

import { Script } from "forge-std/Script.sol";
import { console } from "forge-std/console.sol";

import { ERC1967Proxy } from "@openzeppelin-contracts/proxy/ERC1967/ERC1967Proxy.sol";

import { ICS26Router } from "../contracts/ICS26Router.sol";
import { ICS27GMP } from "../contracts/ICS27GMP.sol";
import { ICS27Account } from "../contracts/utils/ICS27Account.sol";
import { GmpTarget } from "./GmpTarget.sol";

/// @title Deploy GMP for the Stellar corridor
/// @notice Deploys ICS27GMP behind a proxy with its account logic, registers it on the existing router's gmpport,
/// and deploys a GmpTarget to call from Stellar.
/// @dev ICS27GMP is deployed unchanged from upstream. The deployer must hold the router's ID_CUSTOMIZER_ROLE for
/// addIBCApp, as the deployer of DeployIbcCoreForStellar does.
///
/// ```sh
/// forge script scripts/DeployGmpForStellar.s.sol --rpc-url "$RPC_URL" --broadcast \
///   --sig "run(address,address)" <ics26Router> <accessManager>
/// ```
contract DeployGmpForStellar is Script {
    /// @notice Deploys GMP, registers it and deploys the target.
    /// @param router The ICS26Router proxy the Stellar client is registered on.
    /// @param accessManager The AccessManager that governs the router.
    /// @return gmp The ICS27GMP proxy.
    /// @return target The GmpTarget.
    function run(address router, address accessManager) public returns (address gmp, address target) {
        require(router.code.length > 0, "no contract at the router address");
        require(accessManager.code.length > 0, "no contract at the access manager address");

        vm.startBroadcast();

        address accountLogic = address(new ICS27Account());
        address gmpLogic = address(new ICS27GMP());
        gmp = address(
            new ERC1967Proxy(gmpLogic, abi.encodeCall(ICS27GMP.initialize, (router, accountLogic, accessManager)))
        );

        ICS26Router(router).addIBCApp("gmpport", gmp);

        target = address(new GmpTarget());

        vm.stopBroadcast();

        console.log("ICS27Account logic", accountLogic);
        console.log("ICS27GMP logic    ", gmpLogic);
        console.log("ICS27GMP          ", gmp);
        console.log("GmpTarget         ", target);
    }
}
