// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Test.sol";
import "../src/rln/SlashingKarmaLeader.sol";

contract MockKarma is IERC20 {
    mapping(address => uint256) public bal;

    function balanceOf(address a) external view returns (uint256) {
        return bal[a];
    }

    function setBalance(address a, uint256 v) external {
        bal[a] = v;
    }
}

contract SlashingKarmaLeaderTest is Test {
    MockKarma public karma;
    SlashingKarmaLeader public skl;

    address[10] private W = [
        makeAddr("W0"),
        makeAddr("W1"),
        makeAddr("W2"),
        makeAddr("W3"),
        makeAddr("W4"),
        makeAddr("W5"),
        makeAddr("W6"),
        makeAddr("W7"),
        makeAddr("W8"),
        makeAddr("W9")
    ];

    uint256[10] private karmaBalances = [
        50_000_000, // 50%
        25_000_000, // 25%
        10_000_000, // 10%
        5_000_000, // 5%
        2_500_000, // 2.5%
        1_250_000, // 1.25%
        625_000, // 0.625%
        312_500, // 0.3125%
        156_250, // 0.15625%
        156_250 // 0.15625%
    ];
    uint256 private karmaHolders;

    function setUp() public {
        karma = new MockKarma();
        for (uint256 i = 0; i < 10; i++) {
            karma.setBalance(W[i], karmaBalances[i]);
            karmaHolders += karmaBalances[i];
        }
        skl = new SlashingKarmaLeader(address(karma), 8);
        for (uint256 i = 0; i < 10; i++) {
            skl.updateKarma(W[i]);
        }
    }

    function _commit(uint256 period) internal {
        for (uint256 i = 0; i < 10; i++) {
            bytes32 c = keccak256(abi.encodePacked(period, W[i], _salt(period, W[i])));
            vm.prank(W[i]);
            skl.commit(period, c);
        }
    }

    function _reveal(uint256 period) internal {
        uint256 start = skl.periodStart(period);
        vm.roll(start + 1);

        for (uint256 i = 0; i < 10; i++) {
            vm.prank(W[i]);
            skl.reveal(period, _salt(period, W[i]));
        }

        vm.roll(skl.periodEnd(period) - 1);
    }

    function _salt(uint256 period, address who) internal pure returns (uint256) {
        return uint256(keccak256(abi.encodePacked("SALT", period, who)));
    }

    function testCommitRequiresSnapshotOrAuto() public {
        uint256 p2 = 2;
        bytes32 c = keccak256(abi.encodePacked(p2, W[0], _salt(p2, W[0])));
        vm.prank(W[0]);
        skl.commit(p2, c);
        assertGt(skl.karmaSnapshot(W[0], p2), 0);
    }

    function testRevealRequiresCommit() public {
        uint256 p = 3;
        vm.roll(skl.periodStart(p) + 1);
        vm.prank(W[1]);
        vm.expectRevert("no commit");
        skl.reveal(p, _salt(p, W[1]));
    }

    function testStakeWeightedLottery_ApproximateProportions() public {
        uint256 rounds = 1000; //TODO: Find a good amount that does not take too long, but still gives good
            // probabilitistic results
        uint256[10] memory wins;
        vm.pauseGasMetering(); //IMPORTANT: pause metering to avoid crashing when running alongside other tests
        for (uint256 p = 1; p <= rounds; p++) {
            _commit(p);
            _reveal(p);
            address leader = skl.leaderOf(p);
            for (uint256 i = 0; i < 10; i++) {
                if (leader == W[i]) {
                    wins[i] += 1;
                    break;
                }
            }
        }
        vm.resumeGasMetering();
        // TODO: automate this tests somehow, for now use manual inspection (using -vv to see logs)
        for (uint256 i = 0; i < 10; i++) {
            uint256 exp_i = (rounds * karmaBalances[i]) / karmaHolders;
            emit log_named_uint(string(abi.encodePacked("wins[", vm.toString(i), "]")), wins[i]);
            emit log_named_uint(string(abi.encodePacked("exp[", vm.toString(i), "]")), exp_i);
        }
    }
}
