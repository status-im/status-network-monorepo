// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { SimpleKarmaDistributorTest } from "./SimpleKarmaDistributorBase.t.sol";

contract SetRewardsSupplierTest is SimpleKarmaDistributorTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_NotAdmin() public {
        bytes memory expectedError = _accessControlError(alice, distributor.DEFAULT_ADMIN_ROLE());
        vm.prank(alice);
        vm.expectRevert(expectedError);
        distributor.setRewardsSupplier(alice);
    }

    function test_SetRewardsUpdatesSupplierIfAdmin() public {
        assertNotEq(distributor.rewardsSupplier(), alice);
        vm.prank(owner);
        distributor.setRewardsSupplier(alice);
        assertEq(distributor.rewardsSupplier(), alice);
    }
}
