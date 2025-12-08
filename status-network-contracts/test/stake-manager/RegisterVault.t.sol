// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";
import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract RegisterVaultTest is StakeManagerTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_Revertwhen_EmergencyModeEnabled() public {
        vm.prank(admin);
        streamer.enableEmergencyMode();

        vm.expectRevert(IStakeManager.StakeManager__EmergencyModeEnabled.selector);
        _createTestVault(makeAddr("foo"));
    }

    function test_RevertWhen_SenderNotAuthorized() public {
        vm.expectRevert(IStakeManager.StakeManager__Unauthorized.selector);
        streamer.registerVault(makeAddr("foo"));
    }

    function test_RevertWHen_Paused() public {
        vm.prank(admin);
        streamer.pause();

        vm.expectRevert("Pausable: paused");
        _createTestVault(makeAddr("foo"));
    }

    function test_RevertWhen_MaxVaultLimitIsReached() public {
        address owner = makeAddr("vault owner");
        _createTestVault(owner);
        _createTestVault(owner);
        _createTestVault(owner);
        _createTestVault(owner);
        _createTestVault(owner);

        vm.expectRevert(IStakeManager.StakeManager__MaxVaultsPerUserReached.selector);
        _createTestVault(owner);
    }

    function test_VaultRegistration() public view {
        address[4] memory accounts = [alice, bob, charlie, dave];
        for (uint256 i = 0; i < accounts.length; i++) {
            address[] memory userVaults = streamer.getAccountVaults(accounts[i]);
            assertEq(userVaults.length, 1, "wrong number of vaults");
            assertEq(userVaults[0], vaults[accounts[i]], "wrong vault address");
        }
    }
}

