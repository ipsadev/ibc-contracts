// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.28;

/// @title A contract for testing GMP calls from Stellar
/// @notice Records the last value and caller, returns twice the value, and refuses on demand, so a test can see a
/// call land, read its result in the acknowledgement, and see a failure come back as an error acknowledgement.
contract GmpTarget {
    /// @notice The last value set.
    uint256 public value;

    /// @notice The account that set it: the GMP account derived for the Stellar sender.
    address public lastCaller;

    /// @notice Emitted when a value is set.
    /// @param caller The account that set it.
    /// @param value The value.
    event ValueSet(address indexed caller, uint256 value);

    /// @notice Sets the value and returns twice it.
    /// @param newValue The value to set.
    /// @return The doubled value, returned to the sender in the acknowledgement.
    function set(uint256 newValue) external returns (uint256) {
        value = newValue;
        lastCaller = msg.sender;
        emit ValueSet(msg.sender, newValue);
        return newValue * 2;
    }

    /// @notice Always reverts, so a test can see the error acknowledgement.
    function refuse() external pure {
        revert("GmpTarget: refused");
    }
}
