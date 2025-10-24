// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVault } from "../../src/StakeVault.sol";
import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract LeaveTest is StakeVaultTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWHen_NotOwner() public {
        vm.prank(bob);
        vm.expectRevert("Ownable: caller is not the owner");
        stakeVault.leave(bob);
    }

    function test_RevertWhen_InvalidDestination() public {
        vm.prank(alice);
        vm.expectRevert(StakeVault.StakeVault__InvalidDestinationAddress.selector);
        stakeVault.leave(address(0));
    }

    function test_LeaveTransfersAllFunds() public {
        vm.prank(alice);
        stakeVault.stake(2e18, 0);
        vm.prank(alice);
        stakeVault.leave(bob);
        assertEq(stakingToken.balanceOf(bob), 2e18);
    }
}

