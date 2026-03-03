// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { BaseScript } from "./Base.s.sol";
import { DeploymentConfig } from "./DeploymentConfig.s.sol";

import { KarmaBatchTransfer } from "../src/KarmaBatchTransfer.sol";

/**
 * @dev This script deploys the KarmaBatchTransfer contract.
 *
 * Required environment variables:
 *   - KARMA_ADDRESS:               address of the Karma token contract
 *   - KARMA_BATCH_TRANSFER_OWNER:  address that will own the contract and can withdraw remaining funds
 *
 * After deployment, the Karma admin must:
 *   1. Whitelist: karma.setAllowedToTransfer(address(batchTransfer), true)
 *   2. Fund:      karma.mint(address(batchTransfer), totalAmount)
 *
 * Then anyone can execute transfers via ExecuteKarmaBatchTransfer.s.sol.
 * The owner can recover any remaining Karma via batchTransfer.withdraw(to).
 */
contract DeployKarmaBatchTransferScript is BaseScript {
    /**
     * @dev Deploys KarmaBatchTransfer for production use.
     * Reads KARMA_ADDRESS and KARMA_BATCH_TRANSFER_OWNER from environment variables.
     * @return The deployed KarmaBatchTransfer contract instance.
     */
    function run() public returns (KarmaBatchTransfer) {
        address karmaAddress = vm.envAddress("KARMA_ADDRESS");
        require(karmaAddress != address(0), "KARMA_ADDRESS is not set");

        address ownerAddress = vm.envAddress("KARMA_BATCH_TRANSFER_OWNER");
        require(ownerAddress != address(0), "KARMA_BATCH_TRANSFER_OWNER is not set");

        return _run(karmaAddress, ownerAddress);
    }

    /**
     * @dev Deploys KarmaBatchTransfer for testing purposes.
     * @param karma The address of the Karma token contract.
     * @param owner The address that will own the contract.
     * @return batchTransfer The deployed KarmaBatchTransfer contract instance.
     * @return deploymentConfig The DeploymentConfig instance for the current network.
     */
    function runForTest(
        address karma,
        address owner
    )
        public
        returns (KarmaBatchTransfer batchTransfer, DeploymentConfig deploymentConfig)
    {
        deploymentConfig = new DeploymentConfig(broadcaster);
        batchTransfer = _run(karma, owner);
    }

    /**
     * @dev Deploys KarmaBatchTransfer within a broadcast context.
     * @param karma The address of the Karma token contract.
     * @param owner The address that will own the contract.
     * @return The deployed KarmaBatchTransfer contract instance.
     */
    function _run(address karma, address owner) internal broadcast returns (KarmaBatchTransfer) {
        return new KarmaBatchTransfer(karma, owner);
    }
}
