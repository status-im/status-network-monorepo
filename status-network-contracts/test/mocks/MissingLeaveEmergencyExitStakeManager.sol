// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { UUPSUpgradeable } from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

contract MissingLeaveEmergencyExitStakeManager is UUPSUpgradeable {
    function _authorizeUpgrade(address) internal view override {
        require(false, "Deadlock");
    }
}
