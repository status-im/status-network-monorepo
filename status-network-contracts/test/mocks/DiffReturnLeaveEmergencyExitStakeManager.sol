// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { UUPSUpgradeable } from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

contract DiffReturnLeaveEmergencyExitStakeManager is UUPSUpgradeable {
    function _authorizeUpgrade(address) internal view override {
        require(false, "Deadlock");
    }

    function leave() external returns (string memory) {
        return "not a bool type";
    }

    function emergencyModeEnabled() external view returns (string memory) {
        return "not a bool type";
    }
}
