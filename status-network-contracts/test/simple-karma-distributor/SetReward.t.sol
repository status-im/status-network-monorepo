// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { SimpleKarmaDistributor } from "../../src/SimpleKarmaDistributor.sol";
import { SimpleKarmaDistributorTest } from "./SimpleKarmaDistributorBase.t.sol";

contract SetRewardTest is SimpleKarmaDistributorTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWHen_Unauthorized() public {
        uint256 amount = 10 ether;
        vm.prank(owner);
        distributor.setRewardsSupplier(address(karma));

        vm.prank(alice);
        vm.expectRevert(SimpleKarmaDistributor.SimpleKarmaDistributor__Unauthorized.selector);
        distributor.setReward(amount, 0);
    }

    function test_SetRewardUpdatesAvailableSupply() public {
        uint256 amount = 100 ether;

        vm.prank(owner);
        karma.setReward(address(distributor), amount, 0);

        assertEq(distributor.availableSupply(), amount);
        assertEq(distributor.totalRewardsSupply(), 0);
    }
}
