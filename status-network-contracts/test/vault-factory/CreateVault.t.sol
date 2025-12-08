// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { StakeVault } from "../../src/StakeVault.sol";
import { VaultFactoryTest } from "./VaultFactoryBase.t.sol";

contract CreateVaultTest is VaultFactoryTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function testCreateVault() public {
        address owner = makeAddr("vault owner");
        StakeVault vault = _createTestVault(owner);

        assertEq(stakeManager.vaultOwners(address(vault)), owner);
        address[] memory ownerVaults = stakeManager.getAccountVaults(owner);
        assertEq(ownerVaults.length, 1);
        assertEq(ownerVaults[0], address(vault));
    }
}

