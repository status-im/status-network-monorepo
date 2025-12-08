// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { VaultFactoryTest } from "./VaultFactoryBase.t.sol";

contract SetStakeManagerTest is VaultFactoryTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_RevertWhen_NotOwner() public {
        vm.prank(makeAddr("some address"));
        vm.expectRevert("Ownable: caller is not the owner");
        vaultFactory.setStakeManager(makeAddr("some address"));
    }

    function testSetStakeManager() public {
        vm.prank(admin);
        vaultFactory.setStakeManager(makeAddr("some address"));
        assertEq(address(vaultFactory.stakeManager()), makeAddr("some address"));
    }
}
