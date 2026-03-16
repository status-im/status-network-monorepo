// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { Ownable2Step } from "@openzeppelin/contracts/access/Ownable2Step.sol";
import { Karma } from "./Karma.sol";

/**
 * @title KarmaBatchTransfer
 * @notice Utility contract for distributing Karma to many recipients in a single transaction.
 * @dev The Karma admin must whitelist this contract via `karma.setAllowedToTransfer(address(this), true)`
 * and fund it via `karma.mint(address(this), totalAmount)` before calling `batchTransfer`.
 * The owner can recover any remaining Karma balance via `withdraw`.
 */
contract KarmaBatchTransfer is Ownable2Step {
    /// @notice Emitted when the address is zero
    error KarmaBatchTransfer__InvalidAddress();
    /// @notice Emitted when recipients and amounts arrays have different lengths
    error KarmaBatchTransfer__LengthMismatch();
    /// @notice Emitted when the recipients array is empty
    error KarmaBatchTransfer__EmptyBatch();

    /// @notice Emitted when a batch transfer is executed
    event BatchTransferExecuted(uint256 count, uint256 totalAmount);
    /// @notice Emitted when the owner withdraws remaining Karma
    event Withdrawn(address indexed to, uint256 amount);

    /// @notice The Karma token contract
    Karma public immutable KARMA;

    constructor(address _karma, address _owner) {
        if (_karma == address(0) || _owner == address(0)) {
            revert KarmaBatchTransfer__InvalidAddress();
        }
        KARMA = Karma(_karma);
        _transferOwnership(_owner);
    }

    /**
     * @notice Withdraws the entire Karma balance of this contract to the given address.
     * @dev Only callable by the owner.
     * @param to The address to send the remaining Karma to.
     */
    function withdraw(address to) external onlyOwner {
        if (to == address(0)) {
            revert KarmaBatchTransfer__InvalidAddress();
        }
        uint256 amount = KARMA.balanceOf(address(this));
        /// forge-lint: disable-next-line(erc20-unchecked-transfer)
        KARMA.transfer(to, amount);
        emit Withdrawn(to, amount);
    }

    /**
     * @notice Transfers Karma from this contract's balance to each recipient.
     * @dev This contract must be whitelisted in Karma and have sufficient balance.
     * @param recipients Array of recipient addresses.
     * @param amounts Array of amounts to transfer to each recipient.
     */
    function batchTransfer(address[] calldata recipients, uint256[] calldata amounts) external onlyOwner {
        if (recipients.length == 0) {
            revert KarmaBatchTransfer__EmptyBatch();
        }

        if (recipients.length != amounts.length) {
            revert KarmaBatchTransfer__LengthMismatch();
        }

        uint256 totalAmount = 0;
        for (uint256 i = 0; i < recipients.length; i++) {
            /// forge-lint: disable-next-line(erc20-unchecked-transfer)
            KARMA.transfer(recipients[i], amounts[i]);
            totalAmount += amounts[i];
        }

        emit BatchTransferExecuted(recipients.length, totalAmount);
    }
}
