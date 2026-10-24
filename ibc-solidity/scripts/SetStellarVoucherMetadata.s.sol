// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

// solhint-disable custom-errors,gas-custom-errors,gas-small-strings,reason-string

import { Script } from "forge-std/Script.sol";
import { console } from "forge-std/console.sol";

import { ICS20Transfer } from "../contracts/ICS20Transfer.sol";
import { IBCERC20 } from "../contracts/utils/IBCERC20.sol";

/// @title Set the metadata of a voucher for a Stellar asset
/// @notice Upgrades ICS20Transfer's IBCERC20 beacon to the logic with `setMetadata`, then gives the voucher of a
/// Stellar asset the asset's 7 decimals, name and symbol, so wallets show the amounts Stellar sent.
/// @dev The upgrade needs ADMIN_ROLE in the access manager that governs ICS20Transfer, and `setMetadata` needs
/// ERC20_CUSTOMIZER_ROLE. Pass `upgrade` false to set the metadata without upgrading again.
///
/// ```sh
/// forge script scripts/SetStellarVoucherMetadata.s.sol --rpc-url "$RPC_URL" --broadcast --skip-simulation \
///   --sig "run(address,string,string,string,bool)" <ics20Transfer> transfer/stellar-testnet-2/XLM "Stellar Lumens" XLM
/// true ```
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

        vm.startBroadcast();

        if (upgrade) {
            address logic = address(new IBCERC20());
            ICS20Transfer(ics20).upgradeIBCERC20To(logic);
            console.log("IBCERC20 logic", logic);
        }

        IBCERC20(voucher).setMetadata(7, name, symbol);

        vm.stopBroadcast();

        console.log("voucher", voucher);
        console.log("decimals", IBCERC20(voucher).decimals());
        console.log("symbol", IBCERC20(voucher).symbol());
    }
}
