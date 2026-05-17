// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Karma } from "../../src/Karma.sol";
import { KarmaTiers } from "../../src/KarmaTiers.sol";
import { KarmaTest } from "./Karma.t.sol";

contract SlashTest is KarmaTest {
    address public slasher = makeAddr("slasher");

    function _mintKarmaToAccount(address account, uint256 amount) internal {
        vm.startPrank(owner);
        karma.mint(account, amount);
        vm.stopPrank();
    }

    function setUp() public override {
        super.setUp();

        vm.startPrank(owner);
        karma.grantRole(karma.SLASHER_ROLE(), slasher);
        vm.stopPrank();
    }

    function test_RevertWhen_SenderIsNotDefaultAdminOrSlasher() public {
        vm.prank(makeAddr("someone"));
        vm.expectRevert(Karma.Karma__Unauthorized.selector);
        karma.slash(alice, address(0));
    }

    function test_RevertWhen_KarmaBalanceIsInvalid() public {
        vm.prank(slasher);
        vm.expectRevert(Karma.Karma__CannotSlashZeroBalance.selector);
        karma.slash(alice, address(0));
    }

    function test_SlashRemainingBalanceIfBalanceIsLow() public {
        uint256 initialBalance = karma.MIN_SLASH_AMOUNT() - 1;
        _mintKarmaToAccount(alice, initialBalance);

        address rewardRecipient = makeAddr("rewardRecipient");

        vm.prank(slasher);
        uint256 slashed = karma.slash(alice, rewardRecipient);

        // The entire balance should be slashed
        // slashRewardPercentage (default 10%) goes to recipient
        uint256 rewardAmount = (slashed * karma.slashRewardPercentage()) / 10_000;

        // Verify recipient received the reward
        assertEq(karma.balanceOf(rewardRecipient), rewardAmount);
        // Alice should have 0 balance (everything slashed, reward went to recipient)
        assertEq(karma.balanceOf(alice), 0);
    }

    function test_Slash() public {
        // ensure rewards
        uint256 currentBalance = 100 ether;
        _mintKarmaToAccount(alice, currentBalance);
        uint256 slashedAmount = karma.calculateSlashAmount(alice);

        // slash the account with no reward recipient
        vm.prank(slasher);
        karma.slash(alice, address(0));

        // With address(0) recipient, entire amount is burned (no reward minted back)
        assertEq(karma.balanceOf(alice), currentBalance - slashedAmount);

        currentBalance = karma.balanceOf(alice);
        slashedAmount = karma.calculateSlashAmount(alice);

        // slash again
        vm.prank(slasher);
        karma.slash(alice, address(0));

        // Same - entire amount burned
        assertEq(karma.balanceOf(alice), currentBalance - slashedAmount);
    }

    function testFuzz_Slash(uint256 rewardsAmount) public {
        vm.assume(rewardsAmount > 0);
        vm.assume(rewardsAmount <= type(uint128).max);
        _mintKarmaToAccount(alice, rewardsAmount);
        uint256 slashAmount = karma.calculateSlashAmount(alice);

        vm.prank(slasher);
        karma.slash(alice, address(0));

        // With address(0) recipient, entire amount is burned (no reward minted back)
        assertEq(karma.balanceOf(alice), rewardsAmount - slashAmount);
    }

    function test_SlashWithMultipleDistributorsDoesNotOverSlash() public {
        // This test ensures that slashing doesn't overestimate by rounding
        // slash amounts per distributor up to the MINIM_SLASH_AMOUNT.
        // Example:
        // - Actual balance: 0.9e18
        // - Distributor 1 virtual balance: 0.8e18
        // - Distributor 2 virtual balance: 0.7e18
        // - Total balance: 2.4e18
        // - Expected slash at 50%: 1.2e18 (not 2.4e18!)

        uint256 actualBalance = 0.9 ether;
        uint256 distributor1Balance = 0.8 ether;
        uint256 distributor2Balance = 0.7 ether;
        uint256 totalBalance = actualBalance + distributor1Balance + distributor2Balance;

        // Set up the balances
        _mintKarmaToAccount(alice, actualBalance);
        distributor1.setUserKarmaShare(alice, distributor1Balance);
        distributor2.setUserKarmaShare(alice, distributor2Balance);

        // Give distributors enough tokens to redeem
        vm.startPrank(owner);
        karma.mint(address(distributor1), distributor1Balance);
        karma.mint(address(distributor2), distributor2Balance);
        vm.stopPrank();

        // Verify total balance before slash
        assertEq(karma.balanceOf(alice), totalBalance);

        address rewardRecipient = makeAddr("rewardRecipient");

        // Slash alice
        vm.prank(slasher);
        uint256 slashedAmount = karma.slash(alice, rewardRecipient);

        // Expected slash: 50% of 2.4e18 = 1.2e18
        uint256 expectedSlash = (totalBalance * karma.slashPercentage()) / 10_000;
        assertEq(slashedAmount, expectedSlash, "Should slash exactly 50% of total balance");

        // Calculate reward (10% of slashed amount)
        uint256 rewardAmount = (slashedAmount * karma.slashRewardPercentage()) / 10_000;

        // Verify balances after slash
        assertEq(karma.balanceOf(alice), totalBalance - expectedSlash, "Alice should have 50% of original balance");
        assertEq(
            karma.balanceOf(rewardRecipient), rewardAmount, "Reward recipient should receive 10% of slashed amount"
        );
    }

    function test_SlashSkipsPausedDistributor() public {
        uint256 actualBalance = 500e18;
        uint256 distributor1Balance = 1000e18;
        uint256 distributor2Balance = 2000e18;
        uint256 totalBalance = actualBalance + distributor1Balance + distributor2Balance;

        // Set up the balances
        _mintKarmaToAccount(alice, actualBalance);
        distributor1.setUserKarmaShare(alice, distributor1Balance);
        distributor2.setUserKarmaShare(alice, distributor2Balance);

        // Give distributors enough tokens to redeem
        vm.startPrank(owner);
        karma.mint(address(distributor1), distributor1Balance);
        karma.mint(address(distributor2), distributor2Balance);
        vm.stopPrank();

        // Verify initial balance
        assertEq(karma.balanceOf(alice), totalBalance);

        // Pause distributor1
        distributor1.setPaused(true);

        address rewardRecipient = makeAddr("rewardRecipient");

        // Slash alice
        vm.prank(slasher);
        uint256 slashedAmount = karma.slash(alice, rewardRecipient);

        // Verify that distributor1's rewards were NOT redeemed (still has karma share)
        assertEq(
            distributor1.userKarmaShare(alice), distributor1Balance, "Paused distributor should not have redeemed"
        );

        // Verify that distributor2's rewards WERE redeemed (karma share reset to 0)
        assertEq(distributor2.userKarmaShare(alice), 0, "Unpaused distributor should have redeemed");

        // Calculate expected slash: 50% of (actualBalance + distributor2Balance)
        // Note: distributor1's balance is NOT redeemed because it's paused
        uint256 balanceAvailableForSlash = actualBalance + distributor2Balance;
        uint256 expectedSlash = (balanceAvailableForSlash * karma.slashPercentage()) / 10_000;
        assertEq(slashedAmount, expectedSlash, "Should slash 50% of available (non-paused) balance");

        // Calculate reward (10% of slashed amount)
        uint256 rewardAmount = (slashedAmount * karma.slashRewardPercentage()) / 10_000;

        // Verify alice's balance after slash (includes unredeemed distributor1 balance)
        assertEq(
            karma.balanceOf(alice),
            distributor1Balance + balanceAvailableForSlash - expectedSlash,
            "Alice should still have paused distributor balance plus remaining balance"
        );

        // Verify reward recipient received the slash reward
        assertEq(karma.balanceOf(rewardRecipient), rewardAmount, "Reward recipient should receive slash reward");
    }

    function _setupKarmaTiers() internal returns (KarmaTiers) {
        vm.prank(owner);
        KarmaTiers karmaTiers = new KarmaTiers();
        KarmaTiers.Tier[] memory tiers = new KarmaTiers.Tier[](3);
        tiers[0] = KarmaTiers.Tier({ minKarma: 0, maxKarma: 100 ether - 1, name: "Bronze", txPerEpoch: 10 });
        tiers[1] = KarmaTiers.Tier({ minKarma: 100 ether, maxKarma: 1000 ether - 1, name: "Silver", txPerEpoch: 50 });
        tiers[2] =
            KarmaTiers.Tier({ minKarma: 1000 ether, maxKarma: type(uint256).max, name: "Gold", txPerEpoch: 100 });
        vm.prank(owner);
        karmaTiers.updateTiers(tiers);
        return karmaTiers;
    }

    function test_RevertWhen_SlasherTierBelowRequirement() public {
        KarmaTiers karmaTiers = _setupKarmaTiers();

        vm.startPrank(owner);
        karma.setKarmaTiers(address(karmaTiers));
        karma.setSlashTierRequirement(1); // require Silver tier
        vm.stopPrank();

        // slasher has no karma (tier 0 = Bronze), but Silver (tier 1) is required
        _mintKarmaToAccount(alice, 100 ether);

        vm.prank(slasher);
        vm.expectRevert(Karma.Karma__SlashTierRequirementNotMet.selector);
        karma.slash(alice, address(0), slasher);
    }

    function test_SlashSucceedsWhenSlasherMeetsTierRequirement() public {
        KarmaTiers karmaTiers = _setupKarmaTiers();

        vm.startPrank(owner);
        karma.setKarmaTiers(address(karmaTiers));
        karma.setSlashTierRequirement(1); // require Silver tier
        vm.stopPrank();

        // give slasher enough karma to meet Silver tier
        _mintKarmaToAccount(slasher, 100 ether);
        _mintKarmaToAccount(alice, 100 ether);

        vm.prank(slasher);
        karma.slash(alice, address(0), slasher);

        assertLt(karma.balanceOf(alice), 100 ether);
    }

    function test_RevertWhen_AdminTierBelowRequirement() public {
        KarmaTiers karmaTiers = _setupKarmaTiers();

        vm.startPrank(owner);
        karma.setKarmaTiers(address(karmaTiers));
        karma.setSlashTierRequirement(2); // require Gold tier
        vm.stopPrank();

        // owner has no karma (tier 0), so even admin must be rejected
        _mintKarmaToAccount(alice, 100 ether);

        vm.prank(owner);
        vm.expectRevert(Karma.Karma__SlashTierRequirementNotMet.selector);
        karma.slash(alice, address(0), owner);
    }

    function test_TierCheckSkippedWhenKarmaTiersNotSet() public {
        // karmaTiers is address(0) by default — tier check must be skipped
        _mintKarmaToAccount(alice, 100 ether);

        vm.prank(slasher);
        karma.slash(alice, address(0), slasher);

        assertLt(karma.balanceOf(alice), 100 ether);
    }

    function test_SetSlashTierRequirementEmitsEvent() public {
        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit Karma.SlashTierRequirementUpdated(0, 2);
        karma.setSlashTierRequirement(2);

        assertEq(karma.slashTierRequirement(), 2);
    }

    function test_SetKarmaTiersEmitsEvent() public {
        KarmaTiers karmaTiers = _setupKarmaTiers();

        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit Karma.KarmaTiersUpdated(address(0), address(karmaTiers));
        karma.setKarmaTiers(address(karmaTiers));

        assertEq(address(karma.karmaTiers()), address(karmaTiers));
    }
}
