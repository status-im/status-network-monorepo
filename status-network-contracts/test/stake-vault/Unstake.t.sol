// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVault } from "../../src/StakeVault.sol";
import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract UnstakeTest is StakeVaultTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_InvalidDestination() public {
        vm.prank(alice);
        stakeVault.stake(1e18, 0);
        vm.prank(alice);
        vm.expectRevert(StakeVault.StakeVault__InvalidDestinationAddress.selector);
        stakeVault.unstake(1e18, address(0));
    }

    function test_UnstakeTransfersTokensBackToOwner() public {
        uint256 startBalance = stakingToken.balanceOf(alice);
        vm.prank(alice);
        stakeVault.stake(5e18, 0);
        vm.prank(alice);
        stakeVault.unstake(5e18);
        assertEq(stakingToken.balanceOf(alice), startBalance);
    }
}

