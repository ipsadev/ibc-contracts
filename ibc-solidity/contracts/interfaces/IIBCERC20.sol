// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

import { IMintableAndBurnable } from "./IMintableAndBurnable.sol";

/// @title IIBCERC20
/// @notice The interface for received tokens, deployed on receive by ICS20
interface IIBCERC20 is IMintableAndBurnable {
    /// @notice Get the full denom path of the token
    /// @return The full path of the token's denom
    function fullDenomPath() external view returns (string memory);

    /// @notice Get the escrow contract address
    /// @return The escrow contract address
    function escrow() external view returns (address);

    /// @notice Get the ICS20 contract address
    /// @return The ICS20 contract address
    function ics20() external view returns (address);

    /// @notice Sets the name, symbol and decimals wallets show for the token
    /// @dev Only an account holding ERC20_CUSTOMIZER_ROLE in the ICS20 contract's access manager can call it. A token
    /// carrying the amounts of a chain with fewer decimals, such as Stellar's 7, needs its decimals set to match.
    /// @param customDecimals The decimals to report
    /// @param customName The name to report
    /// @param customSymbol The symbol to report
    function setMetadata(uint8 customDecimals, string calldata customName, string calldata customSymbol) external;

    /// @notice Initializes the IBCERC20 contract
    /// @dev This function is meant to be called by a proxy
    /// @param ics20_ The ICS20 contract address
    /// @param escrow_ The escrow contract address, can burn and mint tokens
    /// @param fullDenomPath_ The full IBC denom path for this token
    function initialize(address ics20_, address escrow_, string calldata fullDenomPath_) external;
}
