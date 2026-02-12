// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Test, console } from "forge-std/Test.sol";
import { Karma } from "../../src/Karma.sol";
import { IGaugeVoter } from "../../src/interfaces/IGaugeVoter.sol";
import { KarmaTest } from "./Karma.t.sol";

contract MockGaugeVoter is IGaugeVoter {
    struct Call {
        address from;
        address to;
    }

    Call[] public calls;

    function updateVotingPower(address from, address to) external override {
        calls.push(Call(from, to));
    }

    function callCount() external view returns (uint256) {
        return calls.length;
    }

    function getCall(uint256 index) external view returns (address from, address to) {
        Call storage c = calls[index];
        return (c.from, c.to);
    }

    function reset() external {
        delete calls;
    }
}

contract GaugeVoterHookTest is KarmaTest {
    MockGaugeVoter public mockGaugeVoter;

    function setUp() public override {
        super.setUp();
        mockGaugeVoter = new MockGaugeVoter();
        vm.prank(owner);
        karma.setGaugeVoter(address(mockGaugeVoter));
    }

    function test_SetGaugeVoter() public {
        MockGaugeVoter newVoter = new MockGaugeVoter();

        vm.prank(owner);
        vm.expectEmit(true, true, true, true);
        emit Karma.GaugeVoterUpdated(address(mockGaugeVoter), address(newVoter));
        karma.setGaugeVoter(address(newVoter));

        assertEq(address(karma.gaugeVoter()), address(newVoter));
    }

    function test_SetGaugeVoterOnlyAdmin() public {
        bytes memory expectedError = _accessControlError(alice, karma.DEFAULT_ADMIN_ROLE());
        vm.prank(alice);
        vm.expectRevert(expectedError);
        karma.setGaugeVoter(address(mockGaugeVoter));
    }

    function test_DelegateNotifiesGaugeVoter() public {
        uint256 amount = 1000 ether;
        vm.prank(owner);
        karma.mint(alice, amount);

        // Alice delegates to herself — old delegatee is address(0)
        vm.prank(alice);
        karma.delegate(alice);

        uint256 count = mockGaugeVoter.callCount();
        assertTrue(count > 0, "Should have at least one call");

        // Last call should be from the delegation
        (address from, address to) = mockGaugeVoter.getCall(count - 1);
        assertEq(from, address(0), "Old delegatee should be address(0)");
        assertEq(to, alice, "New delegatee should be alice");
    }

    function test_TransferNotifiesGaugeVoter() public {
        uint256 amount = 1000 ether;

        vm.prank(owner);
        karma.mint(alice, amount);
        vm.prank(alice);
        karma.delegate(alice);
        vm.prank(bob);
        karma.delegate(bob);

        vm.prank(owner);
        karma.setAllowedToTransfer(alice, true);

        mockGaugeVoter.reset();
        // Transfer from alice to bob
        vm.prank(alice);
        /// forge-lint: disable-next-line(erc20-unchecked-transfer)
        karma.transfer(bob, 100 ether);

        assertEq(mockGaugeVoter.callCount(), 1, "Should have exactly one call");
        (address from, address to) = mockGaugeVoter.getCall(0);
        assertEq(from, alice, "From should be alice");
        assertEq(to, bob, "To should be bob");
    }

    function test_MintNotifiesGaugeVoter() public {
        // Bob delegates to himself before receiving mint
        vm.prank(bob);
        karma.delegate(bob);

        mockGaugeVoter.reset();

        // Mint to bob
        vm.prank(owner);
        karma.mint(bob, 500 ether);

        // _afterTokenTransfer: from=address(0), to=bob
        assertEq(mockGaugeVoter.callCount(), 1, "Should have exactly one call");
        (address from, address to) = mockGaugeVoter.getCall(0);
        assertEq(from, address(0), "From should be address(0) for mint");
        assertEq(to, bob, "To should be bob (bob's delegatee)");
    }

    function test_BurnNotifiesGaugeVoter() public {
        uint256 amount = 1000 ether;

        // Mint and delegate
        vm.prank(owner);
        karma.mint(alice, amount);
        vm.prank(alice);
        karma.delegate(alice);

        // Setup slasher
        bytes32 slasherRole = karma.SLASHER_ROLE();
        vm.prank(owner);
        karma.grantRole(slasherRole, owner);

        mockGaugeVoter.reset();

        // Slash alice (which burns tokens)
        vm.prank(owner);
        karma.slash(alice, address(0));

        // redeemRewards triggers zero-amount transfers from 2 distributors before the burn,
        // so the burn notification is the last call
        uint256 count = mockGaugeVoter.callCount();
        assertTrue(count == 3, "Should have 3 calls");

        // Redeem before burning from distributor 1
        (address from, address to) = mockGaugeVoter.getCall(0);
        assertEq(from, address(0), "From should be address(0)");
        assertEq(to, alice, "To should be alice");

        // Redeem before burning from distributor 2
        (from, to) = mockGaugeVoter.getCall(1);
        assertEq(from, address(0), "From should be address(0)");
        assertEq(to, alice, "To should be alice");

        // Burn
        (from, to) = mockGaugeVoter.getCall(2);
        assertEq(from, alice, "From should be alice");
        assertEq(to, address(0), "To should be address(0) for burn");
    }

    function test_NoNotificationWhenGaugeVoterNotSet() public {
        // Remove gauge voter
        vm.prank(owner);
        karma.setGaugeVoter(address(0));

        // Should not revert when minting without gauge voter set
        vm.prank(owner);
        karma.mint(alice, 1000 ether);

        // Should not revert when delegating without gauge voter set
        vm.prank(alice);
        karma.delegate(alice);

        // No calls since gauge voter is address(0)
        assertEq(mockGaugeVoter.callCount(), 0, "Should have no calls");
    }

    function test_ChangeDelegateNotifiesGaugeVoter() public {
        uint256 amount = 1000 ether;

        // Mint and delegate to alice
        vm.prank(owner);
        karma.mint(alice, amount);
        vm.prank(alice);
        karma.delegate(alice);

        mockGaugeVoter.reset();

        // Change delegation from alice to bob
        vm.prank(alice);
        karma.delegate(bob);

        uint256 count = mockGaugeVoter.callCount();
        assertEq(count, 1, "Should have exactly one call");
        (address from, address to) = mockGaugeVoter.getCall(0);
        assertEq(from, alice, "Old delegatee should be alice");
        assertEq(to, bob, "New delegatee should be bob");
    }
}
