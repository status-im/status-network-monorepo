// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Test } from "forge-std/Test.sol";

import { VaultFactory } from "../../src/VaultFactory.sol";
import { MockStakeManager } from "../mocks/MockStakeManager.sol";
import { StakeVault } from "../../src/StakeVault.sol";
import { MockToken } from "../mocks/MockToken.sol";

contract StakeVaultTest is Test {
    VaultFactory internal vaultFactory;
    MockStakeManager internal streamer;
    StakeVault internal stakeVault;
    MockToken internal rewardToken;
    MockToken internal stakingToken;
    MockToken internal otherToken;
    address internal alice = makeAddr("alice");
    address internal bob = makeAddr("bob");
    address internal deployer = makeAddr("deployer");

    function _createTestVault(address owner) internal returns (StakeVault) {
        vm.prank(owner);
        return vaultFactory.createVault();
    }

    function setUp() public virtual {
        rewardToken = new MockToken("Reward Token", "RT");
        stakingToken = new MockToken("Staking Token", "ST");
        otherToken = new MockToken("Other Token", "OT");
        streamer = new MockStakeManager();

        vaultFactory = new VaultFactory(deployer, address(streamer), address(new StakeVault(stakingToken)));

        stakingToken.mint(alice, 10_000e18);

        stakeVault = _createTestVault(alice);

        vm.prank(alice);
        stakingToken.approve(address(stakeVault), 10_000e18);
    }

    function testOwner() public view {
        assertEq(stakeVault.owner(), alice);
    }
}
