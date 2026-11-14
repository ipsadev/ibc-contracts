// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

/// @title IIBCERC20Errors
/// @notice Interface for IBCERC20 errors
interface IIBCERC20Errors {
    /// @notice Unauthorized function call
    /// @param caller The caller of the function
    error IBCERC20Unauthorized(address caller);

    /// @notice Minting or burining is only allowed for escrow
    /// @param escrow The escrow contract address
    /// @param mintAddress The address funds are being minted or burned from
    error IBCERC20NotEscrow(address escrow, address mintAddress);

    /// @notice The decimals were already set by `setMetadata` and cannot change
    /// @param decimals The decimals already set
    /// @param requestedDecimals The decimals the call tried to set
    error IBCERC20DecimalsAlreadySet(uint8 decimals, uint8 requestedDecimals);
}
