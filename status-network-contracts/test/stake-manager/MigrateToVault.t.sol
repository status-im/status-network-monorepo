// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Clones } from "@openzeppelin/contracts/proxy/Clones.sol";

import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";
import { StakeVault } from "../../src/StakeVault.sol";

import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract StakeVaultMigrateToVaultTest is StakeManagerTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_RevertWhenNotOwnerOfMigrationVault() public {
        // alice tries to migrate to a vault she doesn't own
        vm.prank(alice);
        vm.expectRevert(StakeVault.StakeVault__NotAuthorized.selector);
        StakeVault(vaults[alice]).migrateToVault(vaults[bob]);
    }

    function test_RevertWhenMigrationVaultNotEmpty() public {
        // alice creates new vault
        vm.startPrank(alice);
        StakeVault newVault = vaultFactory.createVault();

        // ensure new vault is in use
        stakingToken.approve(address(newVault), 10e18);
        newVault.stake(10e18, 0);

        // alice tries to migrate to a vault that is not empty
        vm.expectRevert(IStakeManager.StakeManager__MigrationTargetHasFunds.selector);
        StakeVault(vaults[alice]).migrateToVault(address(newVault));
    }

    function test_RevertWhenDestinationVaultIsNotRegistered() public {
        // alice creates vaults that's not registered with the stake manager
        vm.startPrank(alice);
        address faultyVault = address(Clones.clone(vaultFactory.vaultImplementation()));
        StakeVault(faultyVault).initialize(alice, address(streamer));

        // alice tries to migrate to a vault that is not registered
        vm.expectRevert(IStakeManager.StakeManager__InvalidVault.selector);
        StakeVault(vaults[alice]).migrateToVault(address(faultyVault));
    }

    function testMigrateToVault() public {
        uint256 stakeAmount = 100e18;

        uint256 initialAccountMP = stakeAmount;
        uint256 initialMaxMP = stakeAmount * streamer.MAX_MULTIPLIER() + stakeAmount;

        // first, ensure alice has a vault with staked funds
        _stake(alice, stakeAmount, 0);

        checkVault(
            CheckVaultParams({
                account: vaults[alice],
                rewardBalance: 0,
                stakedBalance: stakeAmount,
                vaultBalance: stakeAmount,
                rewardIndex: 0,
                mpAccrued: initialAccountMP,
                maxMP: initialMaxMP,
                rewardsAccrued: 0
            })
        );

        checkStreamer(
            CheckStreamerParams({
                totalStaked: stakeAmount,
                totalMPStaked: initialAccountMP,
                stakingBalance: stakeAmount,
                rewardBalance: 0,
                rewardIndex: 0
            })
        );

        // some time passes
        uint256 currentTime = vm.getBlockTimestamp();
        vm.warp(currentTime + 365 days);

        streamer.updateVault(vaults[alice]);

        // ensure vault has accumulated MPs
        checkVault(
            CheckVaultParams({
                account: vaults[alice],
                rewardBalance: 0,
                stakedBalance: stakeAmount,
                vaultBalance: stakeAmount,
                rewardIndex: 0,
                mpAccrued: initialAccountMP * 2, // alice now has twice the amount after a year
                maxMP: initialMaxMP,
                rewardsAccrued: 0
            })
        );

        checkStreamer(
            CheckStreamerParams({
                totalStaked: stakeAmount,
                totalMPStaked: stakeAmount * 2,
                stakingBalance: stakeAmount,
                rewardBalance: 0,
                rewardIndex: 0
            })
        );

        // alice creates new vault
        vm.prank(alice);
        address newVault = address(vaultFactory.createVault());
        uint256 prevVaultLockUntil = StakeVault(vaults[alice]).lockUntil();

        uint256 prevVaultDepositedBalance = StakeVault(vaults[alice]).depositedBalance();
        uint256 prevVaultAmount = streamer.getAccountVaults(alice).length;

        // alice migrates to new vault
        vm.prank(alice);
        StakeVault(vaults[alice]).migrateToVault(newVault);

        // ensure stake manager's total stats have not changed
        checkStreamer(
            CheckStreamerParams({
                totalStaked: stakeAmount,
                totalMPStaked: initialAccountMP * 2,
                stakingBalance: stakeAmount,
                rewardBalance: 0,
                rewardIndex: 0
            })
        );

        // check that alice's funds are now in the new vault
        checkVault(
            CheckVaultParams({
                account: newVault,
                rewardBalance: 0,
                stakedBalance: stakeAmount,
                vaultBalance: stakeAmount,
                rewardIndex: 0,
                mpAccrued: initialAccountMP * 2, // alice now has twice the amount after a year
                maxMP: initialMaxMP,
                rewardsAccrued: 0
            })
        );

        assertEq(
            streamer.getAccountVaults(alice).length,
            prevVaultAmount - 1,
            "alice should have one vault less, as the old one is deregistered"
        );
        assertEq(
            StakeVault(newVault).depositedBalance(), prevVaultDepositedBalance, "deposited balance should be preserved"
        );
        assertEq(StakeVault(newVault).lockUntil(), prevVaultLockUntil, "lock time should be preserved");

        // check that alice's old vault is empty
        checkVault(
            CheckVaultParams({
                account: vaults[alice],
                rewardBalance: 0,
                stakedBalance: 0,
                vaultBalance: 0,
                rewardIndex: 0,
                mpAccrued: 0,
                maxMP: 0,
                rewardsAccrued: 0
            })
        );

        assertEq(StakeVault(vaults[alice]).depositedBalance(), 0, "old vault deposited balance should be 0");
        assertEq(StakeVault(vaults[alice]).lockUntil(), 0, "old vault lock time should be reset");
    }

    function test_RevertWhen_MigratingToVaultThatHasLeft() public {
        uint256 stakeAmount = 1000e18;
        uint256 lockPeriod = 4 * 365 days; // 4 years

        StakeVault lockedVault = StakeVault(vaults[alice]);

        vm.startPrank(alice);
        stakingToken.approve(address(lockedVault), stakeAmount);
        lockedVault.stake(stakeAmount, lockPeriod);
        vm.stopPrank();

        vm.prank(alice);
        StakeVault emptyVault = vaultFactory.createVault();

        vm.prank(alice);
        lockedVault.leave(alice);

        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(StakeVault.StakeVault__InvalidMigrationTarget.selector));
        emptyVault.migrateToVault(address(lockedVault));
    }

    function test_RevertWhenMigrationTargetHasRewardsAccrued() public {
        uint256 stakeAmount = 100e18;

        // Alice stakes in her original vault
        _stake(alice, stakeAmount, 0);

        // Alice creates a new target vault
        vm.prank(alice);
        StakeVault targetVault = vaultFactory.createVault();

        // Alice stakes in the target vault to make it eligible for rewards
        vm.startPrank(alice);
        stakingToken.approve(address(targetVault), stakeAmount);
        targetVault.stake(stakeAmount, 0);
        vm.stopPrank();

        // Set up rewards
        uint256 rewardAmount = 1000e18;
        uint256 rewardDuration = 10 days;
        _setRewards(rewardAmount, rewardDuration);

        // Warp time forward to accrue rewards
        vm.warp(vm.getBlockTimestamp() + 5 days);

        // Update the target vault to accrue rewards (this will make rewardsAccrued > 0)
        streamer.updateVault(address(targetVault));

        // Verify target vault has rewards accrued
        assertGt(streamer.getVault(address(targetVault)).rewardsAccrued, 0, "Target vault should have rewards accrued");

        // Alice unstakes from target vault to make stakedBalance 0 (but rewardsAccrued remains > 0)
        vm.prank(alice);
        targetVault.unstake(stakeAmount);

        // Verify target vault has 0 staked balance but still has rewards accrued
        assertEq(streamer.getVault(address(targetVault)).stakedBalance, 0, "Target vault should have 0 staked balance");
        assertGt(streamer.getVault(address(targetVault)).rewardsAccrued, 0, "Target vault should have rewards accrued");

        // Try to migrate to target vault - should revert because target has rewardsAccrued > 0
        vm.prank(alice);
        vm.expectRevert(IStakeManager.StakeManager__MigrationTargetHasFunds.selector);
        StakeVault(vaults[alice]).migrateToVault(address(targetVault));
    }
}
