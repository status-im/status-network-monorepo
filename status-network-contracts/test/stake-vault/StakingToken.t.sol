// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { StakeVaultTest } from "./StakeVaultBase.t.sol";

contract StakingTokenTest is StakeVaultTest {
    function setUp() public override {
        super.setUp();
    }

    function testStakeToken() public view {
        assertEq(address(stakeVault.STAKING_TOKEN()), address(stakingToken));
    }
}
