// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Test } from "forge-std/Test.sol";

import { DeploymentConfig } from "../../script/DeploymentConfig.s.sol";
import { DeployStakeManagerScript } from "../../script/DeployStakeManager.s.sol";
import { DeployVaultFactoryScript } from "../../script/DeployVaultFactory.s.sol";
import { VaultFactory } from "../../src/VaultFactory.sol";
import { StakeManager } from "../../src/StakeManager.sol";
import { StakeVault } from "../../src/StakeVault.sol";
import { MockToken } from "../mocks/MockToken.sol";

contract VaultFactoryTest is Test {
    address public admin;

    VaultFactory public vaultFactory;
    StakeManager public stakeManager;

    function _createTestVault(address owner) internal returns (StakeVault) {
        vm.prank(owner);
        return vaultFactory.createVault();
    }

    function setUp() public virtual {
        MockToken stakingToken = new MockToken("Staking Token", "ST");
        DeployStakeManagerScript stakeManagerDeployment = new DeployStakeManagerScript();
        DeployVaultFactoryScript vaultFactoryDeployment = new DeployVaultFactoryScript();

        (StakeManager _stakeManager, DeploymentConfig deploymentConfig) =
            stakeManagerDeployment.runForTest(address(stakingToken), 5);
        (address _deployer,) = deploymentConfig.activeNetworkConfig();
        (VaultFactory _vaultFactory,, address vaultProxyClone,) =
            vaultFactoryDeployment.runForTest(address(_stakeManager), address(stakingToken));

        vaultFactory = _vaultFactory;
        stakeManager = _stakeManager;
        admin = _deployer;

        vm.startPrank(admin);
        stakeManager.setVaultFactory(address(vaultFactory));
        stakeManager.setTrustedCodehash(vaultProxyClone.codehash, true);
        vm.stopPrank();
    }
}

