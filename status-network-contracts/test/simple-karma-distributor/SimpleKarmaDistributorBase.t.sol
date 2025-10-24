// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Strings } from "@openzeppelin/contracts/utils/Strings.sol";

import { Test } from "forge-std/Test.sol";

import { DeployKarmaScript } from "../../script/DeployKarma.s.sol";
import { DeploySimpleKarmaDistributorScript } from "../../script/DeploySimpleKarmaDistributor.s.sol";
import { DeploymentConfig } from "../../script/DeploymentConfig.s.sol";
import { Karma } from "../../src/Karma.sol";
import { SimpleKarmaDistributor } from "../../src/SimpleKarmaDistributor.sol";

contract SimpleKarmaDistributorTest is Test {
    SimpleKarmaDistributor internal distributor;
    Karma internal karma;

    address internal owner;
    address internal operator = makeAddr("operator");
    address internal alice = makeAddr("alice");

    function setUp() public virtual {
        DeployKarmaScript karmaDeployment = new DeployKarmaScript();
        (Karma _karma, DeploymentConfig deploymentConfig) = karmaDeployment.runForTest();
        karma = _karma;
        (address deployer,) = deploymentConfig.activeNetworkConfig();
        owner = deployer;

        DeploySimpleKarmaDistributorScript distributorDeployment = new DeploySimpleKarmaDistributorScript();
        (distributor,) = distributorDeployment.deploy(owner, address(karma));

        vm.startPrank(owner);
        distributor.setRewardsSupplier(address(karma));
        distributor.grantRole(distributor.OPERATOR_ROLE(), operator);
        karma.addRewardDistributor(address(distributor));
        karma.setAllowedToTransfer(address(distributor), true);
        vm.stopPrank();
    }

    function _accessControlError(address account, bytes32 role) internal pure returns (bytes memory) {
        return bytes(
            string(
                abi.encodePacked(
                    "AccessControl: account ",
                    Strings.toHexString(uint160(account)),
                    " is missing role ",
                    Strings.toHexString(uint256(role), 32)
                )
            )
        );
    }
}

