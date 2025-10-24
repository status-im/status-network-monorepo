// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { SimpleKarmaDistributorTest } from "./SimpleKarmaDistributorBase.t.sol";

contract RedeemRewardsTest is SimpleKarmaDistributorTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RedeemRewardsWhenNoBalance() public {
        uint256 redeemed = distributor.redeemRewards(alice);
        assertEq(redeemed, 0);
    }

    function test_RedeemRewardsTransfersKarma() public {
        uint256 rewards = 200 ether;
        uint256 mintAmount = 50 ether;

        vm.prank(owner);
        karma.setReward(address(distributor), rewards, 0);

        vm.prank(operator);
        distributor.mint(alice, mintAmount);

        assertEq(karma.balanceOf(alice), mintAmount);
        assertEq(distributor.rewardsBalanceOfAccount(alice), mintAmount);

        vm.prank(alice);
        uint256 redeemed = distributor.redeemRewards(alice);

        assertEq(redeemed, mintAmount);
        assertEq(distributor.mintedSupply(), 0);
        assertEq(distributor.rewardsBalanceOfAccount(alice), 0);
        assertEq(karma.balanceOf(alice), mintAmount);
        assertEq(distributor.totalRewardsSupply(), 0);
    }
}

