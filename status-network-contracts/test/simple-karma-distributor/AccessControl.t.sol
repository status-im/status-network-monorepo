// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { SimpleKarmaDistributorTest } from "./SimpleKarmaDistributorBase.t.sol";

contract AccessControlTest is SimpleKarmaDistributorTest {
    function setUp() public override {
        super.setUp();
    }

    function test_GrantRoleRevertsIfNotAdmin() public {
        bytes memory expectedError = _accessControlError(alice, distributor.DEFAULT_ADMIN_ROLE());
        bytes32 operatorRole = distributor.OPERATOR_ROLE();
        vm.prank(alice);
        vm.expectRevert(expectedError);
        distributor.grantRole(operatorRole, alice);
    }

    function test_GrantRoleCanBeUsedByAdmin() public {
        bytes32 operatorRole = distributor.OPERATOR_ROLE();
        vm.prank(owner);
        distributor.grantRole(operatorRole, alice);
        assert(distributor.hasRole(distributor.OPERATOR_ROLE(), alice));
    }
}
