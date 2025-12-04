// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { VaultFactoryTest } from "./VaultFactoryBase.t.sol";

contract SetVaultImplementationTest is VaultFactoryTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_RevertWhen_NotOwner() public {
        vm.prank(makeAddr("some address"));
        vm.expectRevert("Ownable: caller is not the owner");
        vaultFactory.setVaultImplementation(makeAddr("some address"));
    }

    function testSetVaultImplementation() public {
        vm.prank(admin);
        vaultFactory.setVaultImplementation(makeAddr("some address"));
        assertEq(vaultFactory.vaultImplementation(), makeAddr("some address"));
    }
}

