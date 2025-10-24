// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract StakeTest is StakeVaultTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_NotOwner() public {
        vm.prank(bob);
        vm.expectRevert("Ownable: caller is not the owner");
        stakeVault.stake(1e18, 90 days);
    }

    function test_StakeTransfersTokensToVault() public {
        vm.prank(alice);
        stakeVault.stake(1e18, 90 days);
        assertEq(stakingToken.balanceOf(address(stakeVault)), 1e18);
    }
}

