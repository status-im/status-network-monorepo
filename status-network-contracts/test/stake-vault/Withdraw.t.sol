// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVault } from "../../src/StakeVault.sol";
import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract WithdrawTest is StakeVaultTest {
    function test_RevertWhen_InsufficientAvailableBalance() public {
        vm.prank(alice);
        stakeVault.stake(3e18, 0);
        vm.prank(alice);
        vm.expectRevert(StakeVault.StakeVault__NotEnoughAvailableBalance.selector);
        stakeVault.withdraw(stakingToken, 3e19);
    }

    function test_RevertWhen_InvalidDestination() public {
        otherToken.mint(address(stakeVault), 1e18);
        vm.prank(alice);
        vm.expectRevert(StakeVault.StakeVault__InvalidDestinationAddress.selector);
        stakeVault.withdraw(otherToken, 1e18, address(0));
    }

    function test_WithdrawOtherTokenTransfersToDestination() public {
        otherToken.mint(address(stakeVault), 1e18);
        vm.prank(alice);
        stakeVault.withdraw(otherToken, 1e18, bob);
        assertEq(otherToken.balanceOf(bob), 1e18);
    }

    function test_WithdrawTransfersGenericTokenToOwner() public {
        otherToken.mint(address(stakeVault), 5e17);
        vm.prank(alice);
        stakeVault.withdraw(otherToken, 5e17);
        assertEq(otherToken.balanceOf(alice), 5e17);
    }
}
