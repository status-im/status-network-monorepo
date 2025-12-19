// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { console } from "forge-std/Test.sol";
import { StakeVault } from "../../src/StakeVault.sol";
import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract MultipleVaultsStakeTest is StakeManagerTest {
    StakeVault public vault1;
    StakeVault public vault2;
    StakeVault public vault3;

    function setUp() public override {
        super.setUp();

        vault1 = _createTestVault(alice);
        vault2 = _createTestVault(alice);
        vault3 = _createTestVault(alice);

        vm.startPrank(alice);
        stakingToken.approve(address(vault1), 10_000e18);
        stakingToken.approve(address(vault2), 10_000e18);
        stakingToken.approve(address(vault3), 10_000e18);
        vm.stopPrank();
    }

    function _stakeWithVault(address account, StakeVault vault, uint256 amount, uint256 lockupTime) public {
        vm.prank(account);
        vault.stake(amount, lockupTime);
    }

    function test_StakeMultipleVaults() public {
        console.log(MAX_BALANCE);

        // Alice vault1 stakes 10 tokens
        _stakeWithVault(alice, vault1, 10e18, 0);

        // Alice vault2 stakes 20 tokens
        _stakeWithVault(alice, vault2, 20e18, 0);

        // Alice vault3 stakes 30 tokens
        _stakeWithVault(alice, vault3, 60e18, 0);

        checkStreamer(
            CheckStreamerParams({
                totalStaked: 90e18, totalMPStaked: 90e18, stakingBalance: 90e18, rewardBalance: 0, rewardIndex: 0
            })
        );
    }
}
