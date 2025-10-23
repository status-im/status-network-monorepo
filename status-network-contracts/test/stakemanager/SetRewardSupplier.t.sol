// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";
import { Strings } from "@openzeppelin/contracts/utils/Strings.sol";
import { StakeManagerTest } from "./StakeManagerBase.t.sol";

contract SetRewardSupplierTest is StakeManagerTest {
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
        streamer.setRewardsSupplier(alice);
    }

    function test_RevertWhen_Paused() public {
        // pause the contract
        vm.prank(admin);
        streamer.pause();

        // attempt to set reward supplier
        vm.prank(admin);
        vm.expectRevert("Pausable: paused");
        streamer.setRewardsSupplier(alice);
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
        streamer.setRewardsSupplier(alice);
    }

    function test_SetRewardSupplier_Success() public {
        vm.prank(admin);
        streamer.setRewardsSupplier(alice);
        assertEq(streamer.rewardsSupplier(), alice);
    }
}
