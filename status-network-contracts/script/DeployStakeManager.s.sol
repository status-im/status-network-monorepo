// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { ERC1967Proxy } from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import { BaseScript } from "./Base.s.sol";
import { DeploymentConfig } from "./DeploymentConfig.s.sol";

import { StakeManager } from "../src/StakeManager.sol";

/**
 * @dev This script deploys the StakeManager contract as an upgradeable proxy using a Transparent Proxy pattern.
 * It provides functions to deploy for production use and for testing purposes.
 * The deploy function handles the deployment of the logic contract and the creation of the proxy.
 * The staking token address is obtained from the active network configuration in DeploymentConfig.
 * The reward token address is provided via an environment variable "KARMA_ADDRESS" for production deployments.
 */
contract DeployStakeManagerScript is BaseScript {
    /**
     * @dev Deploys StakeManager contract for production use and returns the instance along with deployment config.
     * The address of the Karma contract must be provided via the "KARMA_ADDRESS" environment variable.
     * The deployer/owner of the StakeManager contract will be set to the broadcaster address.
     * @return stakeManager The deployed StakeManager contract instance.
     * @return deploymentConfig The DeploymentConfig instance for the current network.
     */
    function run() public returns (StakeManager stakeManager, DeploymentConfig deploymentConfig) {
        address karmaAddress = vm.envAddress("KARMA_ADDRESS");
        require(karmaAddress != address(0), "KARMA_ADDRESS is not set");
        uint256 maxVaultsPerUser = vm.envUint("MAX_VAULTS_PER_USER");
        require(maxVaultsPerUser > 0, "MAX_VAULTS_PER_USER is not set or zero");

        deploymentConfig = new DeploymentConfig(broadcaster);
        (, address stakingToken) = deploymentConfig.activeNetworkConfig();
        (stakeManager,) = deploy(broadcaster, stakingToken, karmaAddress, maxVaultsPerUser);
    }

    /**
     * @dev Deploys StakeManager contract for testing purposes and returns the instance along with deployment config.
     * @param rewardToken The address of the reward token (Karma) to be used in the StakeManager.
     * @param maxVaultsPerUser The maximum number of vaults a user can create.
     * @return stakeManager The deployed StakeManager contract instance.
     * @return deploymentConfig The DeploymentConfig instance for the current network.
     */
    function runForTest(
        address rewardToken,
        uint256 maxVaultsPerUser
    )
        public
        returns (StakeManager stakeManager, DeploymentConfig deploymentConfig)
    {
        deploymentConfig = new DeploymentConfig(broadcaster);
        (, address stakingToken) = deploymentConfig.activeNetworkConfig();
        (stakeManager,) = deploy(broadcaster, stakingToken, rewardToken, maxVaultsPerUser);
    }

    /**
     * @dev Deploys StakeManager contract and returns the instance.
     * @param deployer The address that will be set as the deployer/owner of the StakeManager contract.
     * @param stakingToken The address of the staking token to be used in the StakeManager.
     * @param rewardToken The address of the reward token (Karma) to be used in the StakeManager.
     * @param maxVaultsPerUser The maximum number of vaults a user can create.
     * @return proxy The deployed StakeManager proxy contract instance.
     * @return impl The address of the StakeManager logic contract.
     */
    function deploy(
        address deployer,
        address stakingToken,
        address rewardToken,
        uint256 maxVaultsPerUser
    )
        public
        returns (StakeManager proxy, address impl)
    {
        vm.startBroadcast(deployer);
        bytes memory initializeData =
            abi.encodeCall(StakeManager.initialize, (deployer, stakingToken, rewardToken, maxVaultsPerUser));

        // Deploy StakeManager logic contract
        impl = address(new StakeManager());
        // Create upgradeable proxy
        proxy = StakeManager(address(new ERC1967Proxy(impl, initializeData)));
        vm.stopBroadcast();
    }
}
