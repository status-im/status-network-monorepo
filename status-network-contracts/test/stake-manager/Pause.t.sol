// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";
import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract PauseTest is StakeManagerTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhenNotAdmin() public {
        vm.prank(alice);
        vm.expectRevert(IStakeManager.StakeManager__Unauthorized.selector);
        streamer.pause();
    }

    function test_RevertWhenNotGuardian() public {
        vm.prank(alice);
        vm.expectRevert(IStakeManager.StakeManager__Unauthorized.selector);
        streamer.pause();
    }

    function test_PauseAndUnpause() public {
        // pause the contract
        vm.prank(admin);
        streamer.pause();

        // ensure staking is paused
        vm.expectRevert("Pausable: paused");
        _stake(alice, 10e18, 0);

        // unpause the contract
        vm.prank(admin);
        streamer.unpause();

        // ensure staking works again
        _stake(alice, 10e18, 0);
    }

    function test_IsPausedReturnsFalseWhenNotPaused() public {
        assertFalse(streamer.isPaused(), "isPaused should return false initially");
    }

    function test_IsPausedReturnsTrueWhenPaused() public {
        vm.prank(admin);
        streamer.pause();

        assertTrue(streamer.isPaused(), "isPaused should return true when paused");
    }

    function test_IsPausedReturnsFalseAfterUnpause() public {
        vm.prank(admin);
        streamer.pause();

        vm.prank(admin);
        streamer.unpause();

        assertFalse(streamer.isPaused(), "isPaused should return false after unpause");
    }

    function test_IsPausedReturnsTrueWhenEmergencyModeEnabled() public {
        vm.prank(admin);
        streamer.enableEmergencyMode();

        assertTrue(streamer.isPaused(), "isPaused should return true when emergency mode is enabled");
    }

    function test_IsPausedReturnsTrueWhenBothPausedAndEmergencyMode() public {
        vm.prank(admin);
        streamer.pause();

        vm.prank(admin);
        streamer.enableEmergencyMode();

        assertTrue(streamer.isPaused(), "isPaused should return true when both paused and emergency mode");
    }
}

