// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { SimpleKarmaDistributor } from "../../src/SimpleKarmaDistributor.sol";
import { SimpleKarmaDistributorTest } from "./SimpleKarmaDistributorBase.t.sol";

contract MintTest is SimpleKarmaDistributorTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_InsufficientSupply() public {
        uint256 rewards = 200 ether;
        uint256 mintAmount = 300 ether;

        vm.prank(owner);
        karma.setReward(address(distributor), rewards, 0);

        vm.prank(operator);
        vm.expectRevert(SimpleKarmaDistributor.SimpleKarmaDistributor__InsufficientAvailableSupply.selector);
        distributor.mint(alice, mintAmount);
    }

    function test_RevertWhen_ZeroAmount() public {
        vm.prank(operator);
        vm.expectRevert(SimpleKarmaDistributor.SimpleKarmaDistributor__AmountCannotBeZero.selector);
        distributor.mint(alice, 0);
    }

    function test_MintByAdminAdjustsSupply() public {
        uint256 rewards = 200 ether;
        uint256 mintAmount = 50 ether;

        vm.prank(owner);
        karma.setReward(address(distributor), rewards, 0);

        vm.prank(owner);
        distributor.mint(alice, mintAmount);

        assertEq(distributor.availableSupply(), rewards - mintAmount);
        assertEq(distributor.mintedSupply(), mintAmount);
        assertEq(distributor.totalRewardsSupply(), mintAmount);
        assertEq(distributor.rewardsBalanceOfAccount(alice), mintAmount);
    }

    function test_MintByOperator() public {
        uint256 rewards = 200 ether;
        uint256 mintAmount = 50 ether;

        vm.prank(owner);
        karma.setReward(address(distributor), rewards, 0);

        vm.prank(operator);
        distributor.mint(alice, mintAmount);

        assertEq(distributor.availableSupply(), rewards - mintAmount);
        assertEq(distributor.mintedSupply(), mintAmount);
        assertEq(distributor.totalRewardsSupply(), mintAmount);
        assertEq(distributor.rewardsBalanceOfAccount(alice), mintAmount);
    }
}

