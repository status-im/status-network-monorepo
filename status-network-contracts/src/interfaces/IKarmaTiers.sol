// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

interface IKarmaTiers {
    function getTierIdByKarmaBalance(uint256 karmaBalance) external view returns (uint8);
}
