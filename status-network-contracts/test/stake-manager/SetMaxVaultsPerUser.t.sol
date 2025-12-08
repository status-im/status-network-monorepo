// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";
import { Strings } from "@openzeppelin/contracts/utils/Strings.sol";
import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract SetMaxVaultsPerUserTest is StakeManagerTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_RevertWhen_EmergencyModeEnabled() public {
        // enable emergency mode
        vm.prank(admin);
        streamer.enableEmergencyMode();

        // attempt to set reward supplier
        vm.prank(admin);
        vm.expectRevert(IStakeManager.StakeManager__EmergencyModeEnabled.selector);
        streamer.setMaxVaultsPerUser(10);
    }

    function test_RevertWhen_NotAdmin() public {
        vm.expectRevert(
            bytes(
                string(
                    abi.encodePacked(
                        "AccessControl: account ",
                        Strings.toHexString(alice),
                        " is missing role ",
                        Strings.toHexString(uint256(streamer.DEFAULT_ADMIN_ROLE()), 32)
                    )
                )
            )
        );
        vm.prank(alice);
        streamer.setMaxVaultsPerUser(10);
    }

    function test_RevertWhen_SettingZero() public {
        vm.prank(admin);
        vm.expectRevert(IStakeManager.StakeManager__MaxVaultsPerUserCannotBeZero.selector);
        streamer.setMaxVaultsPerUser(0);
    }

    function test_SetMaxVaultsPerUser_Success() public {
        vm.prank(admin);
        streamer.setMaxVaultsPerUser(10);
        assertEq(streamer.maxVaultsPerUser(), 10);
    }
}

