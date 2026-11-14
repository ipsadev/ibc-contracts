// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings,reason-string

import { Script } from "forge-std/Script.sol";
import { console } from "forge-std/console.sol";

import { ICS20Transfer } from "../contracts/ICS20Transfer.sol";
import { IBCERC20 } from "../contracts/utils/IBCERC20.sol";

/// @title Set the metadata of a voucher for a Stellar asset
/// @notice Optionally upgrades ICS20Transfer's IBCERC20 beacon to the logic with `setMetadata`, then gives the voucher
/// of a Stellar asset the asset's decimals, name and symbol, so wallets show the amounts Stellar sent.
/// @dev Beacon upgrade: with `upgrade` true the script deploys a new IBCERC20 logic contract and points ICS20Transfer's
/// IBCERC20 beacon at it, which changes the logic of every voucher that ICS20Transfer has deployed, not just this one.
/// That needs ADMIN_ROLE in the access manager that governs ICS20Transfer. Pass `upgrade` false once the beacon already
/// runs logic with `setMetadata`.
///
/// Metadata: `setMetadata` needs ERC20_CUSTOMIZER_ROLE granted with no execution delay. The decimals come from the
/// VOUCHER_DECIMALS environment variable and default to 7, the decimals of every Stellar asset. The first call fixes
/// the decimals for good, so later runs must pass the same value; name and symbol can be changed by running again.
///
/// ```sh
/// VOUCHER_DECIMALS=7 forge script scripts/SetStellarVoucherMetadata.s.sol --rpc-url "$RPC_URL" --broadcast \
///   --skip-simulation --sig "run(address,string,string,string,bool)" \
///   <ics20Transfer> transfer/stellar-testnet-2/XLM "Stellar Lumens" XLM true
/// ```
contract SetStellarVoucherMetadata is Script {
    /// @notice Upgrades the beacon if asked, then sets the voucher's metadata.
    /// @param ics20 The ICS20Transfer proxy.
    /// @param denom The voucher's full denom path, `transfer/<client>/<denom>`.
    /// @param name The name wallets show.
    /// @param symbol The symbol wallets show.
    /// @param upgrade Whether to deploy the new IBCERC20 logic and upgrade the beacon first.
    /// @return voucher The voucher whose metadata was set.
    function run(
        address ics20,
        string calldata denom,
        string calldata name,
        string calldata symbol,
        bool upgrade
    )
        public
        returns (address voucher)
    {
        require(ics20.code.length > 0, "no contract at the ICS20Transfer address");

        voucher = ICS20Transfer(ics20).ibcERC20Contract(denom);
        require(voucher != address(0), "ICS20Transfer has no voucher for that denom");

        uint256 decimals = vm.envOr("VOUCHER_DECIMALS", uint256(7));
        require(decimals <= type(uint8).max, "VOUCHER_DECIMALS does not fit in a uint8");

        vm.startBroadcast();

        if (upgrade) {
            address logic = address(new IBCERC20());
            ICS20Transfer(ics20).upgradeIBCERC20To(logic);
            console.log("IBCERC20 logic", logic);
        }

        IBCERC20(voucher).setMetadata(uint8(decimals), name, symbol);

        vm.stopBroadcast();

        console.log("voucher", voucher);
        console.log("decimals", IBCERC20(voucher).decimals());
        console.log("symbol", IBCERC20(voucher).symbol());
    }
}
