// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { console } from "forge-std/Test.sol";

import { BaseScript } from "./Base.s.sol";
import { DeploymentConfig } from "./DeploymentConfig.s.sol";

import { DeployKarmaScript } from "./DeployKarma.s.sol";
import { DeployMetadataGeneratorScript } from "./DeployMetadataGenerator.s.sol";
import { DeployKarmaNFTScript } from "./DeployKarmaNFT.s.sol";
import { DeployStakeManagerScript } from "./DeployStakeManager.s.sol";
import { DeployVaultFactoryScript } from "./DeployVaultFactory.s.sol";
import { DeploySimpleKarmaDistributorScript } from "./DeploySimpleKarmaDistributor.s.sol";
import { DeployKarmaTiersScript } from "./DeployKarmaTiers.s.sol";

import { INFTMetadataGenerator } from "../src/interfaces/INFTMetadataGenerator.sol";
import { Karma } from "../src/Karma.sol";
import { KarmaNFT } from "../src/KarmaNFT.sol";
import { StakeManager } from "../src/StakeManager.sol";
import { VaultFactory } from "../src/VaultFactory.sol";
import { SimpleKarmaDistributor } from "../src/SimpleKarmaDistributor.sol";
import { KarmaTiers } from "../src/KarmaTiers.sol";

/**
 * @dev This script deploys the entire protocol including Karma, KarmaNFT, StakeManager, and VaultFactory.
 * It uses the DeploymentConfig to get network-specific parameters.
 * The script assumes that the staking token address is provided in the active network configuration.
 */
contract DeployProtocolScript is BaseScript {
    DeployKarmaScript deployKarma;

    DeployMetadataGeneratorScript deployMetadataGenerator;

    DeployKarmaNFTScript deployKarmaNFT;

    DeployStakeManagerScript deployStakeManager;

    DeployVaultFactoryScript deployVaultFactory;

    DeploySimpleKarmaDistributorScript deploySimpleKarmaDistributor;

    DeployKarmaTiersScript deployKarmaTiers;

    constructor() BaseScript() {
        deployKarma = new DeployKarmaScript();
        deployMetadataGenerator = new DeployMetadataGeneratorScript();
        deployKarmaNFT = new DeployKarmaNFTScript();
        deployStakeManager = new DeployStakeManagerScript();
        deployVaultFactory = new DeployVaultFactoryScript();
        deploySimpleKarmaDistributor = new DeploySimpleKarmaDistributorScript();
        deployKarmaTiers = new DeployKarmaTiersScript();
    }

    /**
     * @dev Deploys protocol for production use and returns the instances.
     * The address of the staking token must be provided via the active network configuration.
     * @return karma The deployed Karma contract instance.
     * @return karmaImpl The address of the Karma logic contract.
     * @return metadataGenerator The deployed NFT metadata generator contract instance.
     * @return karmaNFT The deployed KarmaNFT contract instance.
     * @return stakeManager The deployed StakeManager contract instance.
     * @return stakeManagerImpl The address of the StakeManager logic contract.
     * @return vaultFactory The deployed VaultFactory contract instance.
     * @return vaultImpl The address of the StakeVault logic contract.
     * @return vaultProxyClone The address of the StakeVault proxy clone used by the VaultFactory.
     */
    function run()
        public
        returns (
            Karma,
            address, /* karmaImpl */
            INFTMetadataGenerator,
            KarmaNFT,
            StakeManager,
            address, /* stakeManagerImpl */
            VaultFactory,
            address, /* vaultImpl */
            address /* vaultProxyClone */
        )
    {
        DeploymentConfig deploymentConfig = new DeploymentConfig(broadcaster);
        (, address stakingToken) = deploymentConfig.activeNetworkConfig();

        uint256 maxVaultsPerUser = vm.envUint("MAX_VAULTS_PER_USER");
        if (maxVaultsPerUser == 0) {
            revert("MAX_VAULTS_PER_USER is not set or zero");
        }
        return _run(stakingToken, maxVaultsPerUser);
    }

    /**
     * @dev Deploys protocol for test use and returns core contract instances.
     * @param stakingToken The address of the staking token to be used in the StakeManager and VaultFactory.
     * @param maxVaultsPerUser The maximum number of vaults a user can create in the StakeManager.
     * @return karma The deployed Karma contract instance.
     * @return metadataGenerator The deployed NFT metadata generator contract instance.
     * @return karmaNFT The deployed KarmaNFT contract instance.
     * @return stakeManager The deployed StakeManager contract instance.
     * @return vaultFactory The deployed VaultFactory contract instance.
     * @return vaultImpl The address of the StakeVault logic contract.
     * @return deploymentConfig The DeploymentConfig instance used for deployment.
     */
    function runForTest(
        address stakingToken,
        uint256 maxVaultsPerUser
    )
        public
        returns (
            Karma karma,
            INFTMetadataGenerator metadataGenerator,
            KarmaNFT karmaNFT,
            StakeManager stakeManager,
            VaultFactory vaultFactory,
            address vaultImpl,
            DeploymentConfig deploymentConfig
        )
    {
        deploymentConfig = new DeploymentConfig(broadcaster);
        (
            karma,/* karmaImpl */,
            metadataGenerator,
            karmaNFT,
            stakeManager,/* stakeManagerImpl */,
            vaultFactory,
            vaultImpl,
            /* vaultProxyClone */
        ) = _run(stakingToken, maxVaultsPerUser);
    }

    /**
     * @dev Deploys protocol by calling sub script `deploy()` functions and returns the instances.
     * @param stakingToken The address of the staking token to be used in the StakeManager and VaultFactory.
     * @param maxVaultsPerUser The maximum number of vaults a user can create in the StakeManager.
     * @return karma The deployed Karma contract instance.
     * @return karmaImpl The address of the Karma logic contract.
     * @return metadataGenerator The deployed NFT metadata generator contract instance.
     * @return karmaNFT The deployed KarmaNFT contract instance.
     * @return stakeManager The deployed StakeManager contract instance.
     * @return stakeManagerImpl The address of the StakeManager logic contract.
     * @return vaultFactory The deployed VaultFactory contract instance.
     * @return vaultImpl The address of the StakeVault logic contract.
     * @return vaultProxyClone The address of the StakeVault proxy clone used by the VaultFactory.
     */
    function _run(
        address stakingToken,
        uint256 maxVaultsPerUser
    )
        internal
        returns (
            Karma karma,
            address karmaImpl,
            INFTMetadataGenerator metadataGenerator,
            KarmaNFT karmaNFT,
            StakeManager stakeManager,
            address stakeManagerImpl,
            VaultFactory vaultFactory,
            address vaultImpl,
            address vaultProxyClone
        )
    {
        console.log("Deploying Karma...");
        (karma, karmaImpl) = deployKarma.deploy(broadcaster);

        console.log("Deploying NFTMetadataGeneratorSVG...");
        metadataGenerator = deployMetadataGenerator.deploy(broadcaster);

        console.log("Deploying KarmaNFT...");
        karmaNFT = deployKarmaNFT.deploy(broadcaster, address(metadataGenerator), address(karma));

        console.log("Deploying StakeManager...");
        (stakeManager, stakeManagerImpl) =
            deployStakeManager.deploy(broadcaster, stakingToken, address(karma), maxVaultsPerUser);

        console.log("Deploying VaultFactory...");
        (vaultFactory, vaultImpl, vaultProxyClone) =
            deployVaultFactory.deploy(broadcaster, address(stakeManager), stakingToken);

        console.log("Deploying SimpleRewardDistributor...");
        (SimpleKarmaDistributor simpleKarmaDistributor, address simpleKarmaDistributorImpl) =
            deploySimpleKarmaDistributor.deploy(broadcaster, address(karma));

        console.log("Deploying KarmaTiers...");
        KarmaTiers karmaTiers = deployKarmaTiers.deploy(broadcaster);

        console.log("\nContract addresses:");
        console.log(address(karma), ": Karma (proxy)");
        console.log(karmaImpl, ": Karma (implementation)");
        console.log(address(metadataGenerator), ": NFTMetadataGeneratorSVG");
        console.log(address(karmaNFT), ": KarmaNFT");
        console.log(address(stakeManager), ": StakeManager (proxy)");
        console.log(stakeManagerImpl, ": StakeManager (implementation)");
        console.log(address(vaultFactory), ": VaultFactory");
        console.log(vaultImpl, ": StakeVault (implementation)");
        console.log(vaultProxyClone, ": StakeVault (proxy clone)");
        console.log(address(simpleKarmaDistributor), ": SimpleKarmaDistributor (proxy)");
        console.log(simpleKarmaDistributorImpl, ": SimpleKarmaDistributor (implementation)");
        console.log(address(karmaTiers), ": KarmaTiers");

        /// INITIALIZATION
        vm.startBroadcast(broadcaster);
        console.log("\nInitializing contracts...");

        // add reward distributors to Karma
        karma.addRewardDistributor(address(stakeManager));
        console.log("Added reward distributor (StakeManager)", address(stakeManager));
        karma.addRewardDistributor(address(simpleKarmaDistributor));
        console.log("Added reward distributor (SimpleKarmaDistributor)", address(simpleKarmaDistributor));

        // whitelist reward distributors for transferring Karma tokens
        karma.setAllowedToTransfer(address(stakeManager), true);
        console.log("Whitelisted reward distributor (StakeManager)", address(stakeManager), "for transfer");
        karma.setAllowedToTransfer(address(simpleKarmaDistributor), true);
        console.log(
            "Whitelisted reward distributor (SimpleKarmaDistributor)", address(simpleKarmaDistributor), "for transfer"
        );

        // configure Karma as reward supplier for reward distributors
        stakeManager.setRewardsSupplier(address(karma));
        console.log("Set rewards supplier (Karma) for StakeManager");
        simpleKarmaDistributor.setRewardsSupplier(address(karma));
        console.log("Set rewards supplier (Karma) for SimpleRewardDistributor");

        stakeManager.setVaultFactory(address(vaultFactory));
        console.log("Set vault factory (VaultFactory) for StakeManager");
        // whitelist StakeVault proxy clone codehash in StakeManager
        stakeManager.setTrustedCodehash(vaultProxyClone.codehash, true);
        console.log("Set trusted codehash for StakeVault proxy clone:", vaultProxyClone);
        vm.stopBroadcast();
    }
}
