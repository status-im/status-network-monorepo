// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVault } from "../../src/StakeVault.sol";
import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract MigrateToTest is StakeVaultTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_NotOwner() public {
        vm.prank(bob);
        vm.expectRevert("Ownable: caller is not the owner");
        stakeVault.migrateToVault(makeAddr("new vault"));
    }

    function test_RevertWhen_MigratingToSelf() public {
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(StakeVault.StakeVault__InvalidMigrationTarget.selector));
        stakeVault.migrateToVault(address(stakeVault));
    }
}

