// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Clones } from "@openzeppelin/contracts/proxy/Clones.sol";

import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";
import { StakeVault } from "../../src/StakeVault.sol";

import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract StakeVaultReplaceVaultTest is StakeManagerTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_RevertWhenNotOwnerOfMigrationVault() public {
        // alice tries to migrate to a vault she doesn't own
        vm.prank(bob);
        vm.expectRevert("Ownable: caller is not the owner");
        StakeVault(vaults[alice]).replaceVault();
    }

    function testReplaceVault() public {
        address[] memory vaultsBefore = streamer.getAccountVaults(alice);
        assertEq(vaultsBefore.length, 1);

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

        vm.prank(alice);
        address newVault = StakeVault(vaults[alice]).replaceVault();

        // ensure alice's amount of vaults have not changed
        address[] memory vaultsAfter = streamer.getAccountVaults(alice);
        assertEq(vaultsAfter.length, vaultsBefore.length);

        // old vault should have no balances anymore
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

        // data is migrated to new vault
        checkVault(
            CheckVaultParams({
                account: newVault,
                rewardBalance: 0,
                stakedBalance: stakeAmount,
                vaultBalance: stakeAmount,
                rewardIndex: 0,
                mpAccrued: initialAccountMP,
                maxMP: initialMaxMP,
                rewardsAccrued: 0
            })
        );
    }

    function testReplaceVaultWithMaxVaultAmount() public {
        // create user with vaults at max limit
        address owner = makeAddr("vault owner");
        _createTestVault(owner);
        _createTestVault(owner);
        _createTestVault(owner);
        _createTestVault(owner);
        StakeVault vault5 = _createTestVault(owner);

        address[] memory vaultsBefore = streamer.getAccountVaults(owner);
        assertEq(vaultsBefore.length, 5);

        stakingToken.mint(owner, 10_000e18);

        uint256 stakeAmount = 100e18;
        uint256 initialAccountMP = stakeAmount;
        uint256 initialMaxMP = stakeAmount * streamer.MAX_MULTIPLIER() + stakeAmount;

        vm.prank(owner);
        stakingToken.approve(address(vault5), 10_000e18);

        vm.prank(owner);
        vault5.stake(stakeAmount, 0);

        checkVault(
            CheckVaultParams({
                account: address(vault5),
                rewardBalance: 0,
                stakedBalance: stakeAmount,
                vaultBalance: stakeAmount,
                rewardIndex: 0,
                mpAccrued: initialAccountMP,
                maxMP: initialMaxMP,
                rewardsAccrued: 0
            })
        );

        vm.prank(owner);
        address newVault = vault5.replaceVault();

        // ensure amount of vaults have not changed
        address[] memory vaultsAfter = streamer.getAccountVaults(owner);
        assertEq(vaultsAfter.length, vaultsBefore.length);

        // old vault should have no balances anymore
        checkVault(
            CheckVaultParams({
                account: address(vault5),
                rewardBalance: 0,
                stakedBalance: 0,
                vaultBalance: 0,
                rewardIndex: 0,
                mpAccrued: 0,
                maxMP: 0,
                rewardsAccrued: 0
            })
        );

        // data is migrated to new vault
        checkVault(
            CheckVaultParams({
                account: newVault,
                rewardBalance: 0,
                stakedBalance: stakeAmount,
                vaultBalance: stakeAmount,
                rewardIndex: 0,
                mpAccrued: initialAccountMP,
                maxMP: initialMaxMP,
                rewardsAccrued: 0
            })
        );
    }
}
