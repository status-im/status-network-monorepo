// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVault } from "../../src/StakeVault.sol";
import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract TransferOwnershipTest is StakeVaultTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_TransferOwnership() public {
        vm.prank(alice);
        vm.expectRevert(StakeVault.StakeVault__NotAllowedToTransferOwnership.selector);
        stakeVault.transferOwnership(bob);
    }
}
