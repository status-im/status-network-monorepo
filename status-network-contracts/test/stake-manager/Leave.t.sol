// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { IERC20 } from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import { UUPSUpgradeable } from "@openzeppelin/contracts/proxy/utils/UUPSUpgradeable.sol";
import { MissingLeaveEmergencyExitStakeManager } from "../mocks/MissingLeaveEmergencyExitStakeManager.sol";
import { DiffReturnLeaveEmergencyExitStakeManager } from "../mocks/DiffReturnLeaveEmergencyExitStakeManager.sol";
import { StakeVault } from "../../src/StakeVault.sol";
import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract LeaveTest is StakeManagerTest {
    function setUp() public override {
        super.setUp();
    }

    function test_LeaveShouldProperlyUpdateAccounting() public {
        uint256 aliceInitialBalance = stakingToken.balanceOf(alice);

        _stake(alice, 100e18, 0);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance - 100e18, "Alice should have staked tokens");

        checkStreamer(
            CheckStreamerParams({
                totalStaked: 100e18, totalMPStaked: 100e18, stakingBalance: 100e18, rewardBalance: 0, rewardIndex: 0
            })
        );

        _upgradeStakeManager();
        _leave(alice);

        // stake manager properly updates accounting
        checkStreamer(
            CheckStreamerParams({
                totalStaked: 0, totalMPStaked: 0, stakingBalance: 0, rewardBalance: 0, rewardIndex: 0
            })
        );

        // vault should be empty as funds have been moved out
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

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance, "Alice has all her funds back");
    }

    function test_LeaveShouldDeregisterLeavingVault() public {
        uint256 stakeAmount = 100e18;

        _stake(alice, stakeAmount, 0);

        address[] memory vaultsBefore = streamer.getAccountVaults(alice);
        assertEq(vaultsBefore.length, 1, "Alice should have one vault registered");

        _leave(alice);

        address[] memory vaultsAfter = streamer.getAccountVaults(alice);
        assertEq(vaultsAfter.length, 0, "Alice should have no vaults registered after leaving");
    }

    function test_LeaveShouldKeepFundsLockedInStakeVault() public {
        uint256 aliceInitialBalance = stakingToken.balanceOf(alice);
        uint256 stakeAmount = 10e18;
        uint256 lockUpPeriod = streamer.MIN_LOCKUP_PERIOD();

        _stake(alice, stakeAmount, lockUpPeriod);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance - stakeAmount, "Alice should have staked tokens");

        _upgradeStakeManager();
        _leave(alice);

        assertEq(
            stakingToken.balanceOf(alice), aliceInitialBalance - stakeAmount, "Alice still doesn't have her funds back"
        );

        vm.warp(block.timestamp + lockUpPeriod);

        StakeVault vault = StakeVault(vaults[alice]);
        IERC20 token = vault.STAKING_TOKEN();
        vm.prank(alice);
        vault.withdraw(token, stakeAmount);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance, "Alice has withdrawn her funds");
    }

    function test_LeaveShouldNotRedeemAccruedRewardsWhenSystemIsPaused() public {
        vm.startPrank(admin);
        karma.setReward(address(streamer), 1000e18, 10 days);
        vm.stopPrank();

        uint256 aliceInitialBalance = stakingToken.balanceOf(alice);

        _stake(alice, 100e18, 0);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance - 100e18, "Alice should have staked tokens");

        checkStreamer(
            CheckStreamerParams({
                totalStaked: 100e18, totalMPStaked: 100e18, stakingBalance: 100e18, rewardBalance: 0, rewardIndex: 0
            })
        );

        vm.warp(block.timestamp + 5 days);

        // Check that rewards have accrued (before pause)
        uint256 aliceRewardsBeforePause = streamer.rewardsBalanceOf(vaults[alice]);
        assertEq(aliceRewardsBeforePause, 500e18, "Alice should have half the rewards (5 days)");

        // Update global state to reflect accrued rewards
        streamer.updateRewards();
        // Pause the system
        vm.prank(guardian);
        streamer.pause();

        assertTrue(streamer.paused(), "System should be paused");

        // Get totalRewardsAccrued before leave
        uint256 totalRewardsAccruedBeforeLeave = streamer.totalRewardsAccrued();
        assertGt(totalRewardsAccruedBeforeLeave, 0, "Total rewards accrued should be greater than 0");

        // Alice leaves while system is paused
        _leave(alice);

        // Verify totalRewardsAccrued was properly decremented
        assertEq(
            streamer.totalRewardsAccrued(),
            totalRewardsAccruedBeforeLeave,
            "Total rewards accrued should not be decremented"
        );

        // Vault should be empty
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

        assertEq(streamer.rewardsBalanceOf(vaults[alice]), 0, "Alice should have lost her unsettled rewards");

        // Alice gets her staked tokens back
        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance, "Alice should have all her staked tokens back");
    }

    function test_LeaveMissingLeaveStakeManager() public {
        uint256 aliceInitialBalance = stakingToken.balanceOf(alice);

        // first change the existing manager's state
        _stake(alice, 100e18, 0);
        checkStreamer(
            CheckStreamerParams({
                totalStaked: 100e18, totalMPStaked: 100e18, stakingBalance: 100e18, rewardBalance: 0, rewardIndex: 0
            })
        );

        // upgrade the manager to a malicious one
        address newImpl = address(new MissingLeaveEmergencyExitStakeManager());
        vm.prank(admin);
        UUPSUpgradeable(address(streamer)).upgradeTo(newImpl);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance - 100e18, "Alice should have less tokens before");

        // alice leaves system and is able to get funds out, despite malicious manager
        _leave(alice);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance, "Alice should get her tokens back");
    }

    function test_LeaveDiffReturnLeaveStakeManager() public {
        uint256 aliceInitialBalance = stakingToken.balanceOf(alice);

        // first change the existing manager's state
        _stake(alice, 100e18, 0);
        checkStreamer(
            CheckStreamerParams({
                totalStaked: 100e18, totalMPStaked: 100e18, stakingBalance: 100e18, rewardBalance: 0, rewardIndex: 0
            })
        );

        // upgrade the manager to a malicious one
        address newImpl = address(new DiffReturnLeaveEmergencyExitStakeManager());
        vm.prank(admin);
        UUPSUpgradeable(address(streamer)).upgradeTo(newImpl);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance - 100e18, "Alice should have less tokens before");

        // alice leaves system and is able to get funds out, despite malicious manager
        _leave(alice);

        assertEq(stakingToken.balanceOf(alice), aliceInitialBalance, "Alice should get her tokens back");
    }
}

