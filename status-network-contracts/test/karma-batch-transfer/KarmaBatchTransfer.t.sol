// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Test } from "forge-std/Test.sol";
import { IERC20 } from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

import { DeployKarmaScript } from "../../script/DeployKarma.s.sol";
import { DeployKarmaBatchTransferScript } from "../../script/DeployKarmaBatchTransfer.s.sol";
import { DeploymentConfig } from "../../script/DeploymentConfig.s.sol";
import { Karma } from "../../src/Karma.sol";
import { KarmaBatchTransfer } from "../../src/KarmaBatchTransfer.sol";
import { KarmaDistributorMock } from "../mocks/KarmaDistributorMock.sol";

contract KarmaBatchTransferTest is Test {
    Karma public karma;
    KarmaBatchTransfer public batchTransfer;

    address public owner;
    address public batchOwner = makeAddr("batchOwner");
    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
    address public carol = makeAddr("carol");

    uint256 public constant INITIAL_SUPPLY = 100_000e18;

    function setUp() public virtual {
        DeployKarmaScript karmaDeployment = new DeployKarmaScript();
        (Karma _karma, DeploymentConfig deploymentConfig) = karmaDeployment.runForTest();
        karma = _karma;
        (address deployer,) = deploymentConfig.activeNetworkConfig();
        owner = deployer;

        DeployKarmaBatchTransferScript batchTransferDeployment = new DeployKarmaBatchTransferScript();
        (KarmaBatchTransfer _batchTransfer,) = batchTransferDeployment.runForTest(address(karma), batchOwner);
        batchTransfer = _batchTransfer;

        vm.startPrank(owner);
        karma.setAllowedToTransfer(address(batchTransfer), true);
        karma.mint(address(batchTransfer), INITIAL_SUPPLY);
        vm.stopPrank();
    }

    function test_Owner() public view {
        assertEq(batchTransfer.owner(), batchOwner);
    }

    function test_BatchTransfer() public {
        address[] memory recipients = new address[](3);
        recipients[0] = alice;
        recipients[1] = bob;
        recipients[2] = carol;

        uint256[] memory amounts = new uint256[](3);
        amounts[0] = 1000e18;
        amounts[1] = 2000e18;
        amounts[2] = 3000e18;

        uint256 totalAmount = 6000e18;

        vm.expectEmit();
        emit KarmaBatchTransfer.BatchTransferExecuted(3, totalAmount);

        vm.prank(batchOwner);
        batchTransfer.batchTransfer(recipients, amounts);

        assertEq(karma.balanceOf(alice), 1000e18);
        assertEq(karma.balanceOf(bob), 2000e18);
        assertEq(karma.balanceOf(carol), 3000e18);
        assertEq(karma.balanceOf(address(batchTransfer)), INITIAL_SUPPLY - totalAmount);
    }

    function test_Withdraw() public {
        uint256 contractBalance = karma.balanceOf(address(batchTransfer));

        vm.expectEmit();
        emit KarmaBatchTransfer.Withdrawn(batchOwner, contractBalance);

        vm.prank(batchOwner);
        batchTransfer.withdraw(batchOwner);

        assertEq(karma.balanceOf(batchOwner), contractBalance);
        assertEq(karma.balanceOf(address(batchTransfer)), 0);
    }

    function test_RevertWhen_BatchTransfer_NotOwner() public {
        address[] memory recipients = new address[](1);
        recipients[0] = alice;

        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1000e18;

        vm.prank(alice);
        vm.expectRevert("Ownable: caller is not the owner");
        batchTransfer.batchTransfer(recipients, amounts);
    }

    function test_RevertWhen_WithdrawNotOwner() public {
        vm.prank(alice);
        vm.expectRevert("Ownable: caller is not the owner");
        batchTransfer.withdraw(alice);
    }

    function test_RevertWhen_WithdrawToZeroAddress() public {
        vm.prank(batchOwner);
        vm.expectRevert(KarmaBatchTransfer.KarmaBatchTransfer__InvalidAddress.selector);
        batchTransfer.withdraw(address(0));
    }

    function test_RevertWhen_LengthMismatch() public {
        address[] memory recipients = new address[](2);
        recipients[0] = alice;
        recipients[1] = bob;

        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1000e18;

        vm.prank(batchOwner);
        vm.expectRevert(KarmaBatchTransfer.KarmaBatchTransfer__LengthMismatch.selector);
        batchTransfer.batchTransfer(recipients, amounts);
    }

    function test_RevertWhen_EmptyBatch() public {
        address[] memory recipients = new address[](0);
        uint256[] memory amounts = new uint256[](0);

        vm.prank(batchOwner);
        vm.expectRevert(KarmaBatchTransfer.KarmaBatchTransfer__EmptyBatch.selector);
        batchTransfer.batchTransfer(recipients, amounts);
    }

    function test_RevertWhen_InsufficientBalance() public {
        address[] memory recipients = new address[](1);
        recipients[0] = alice;

        uint256[] memory amounts = new uint256[](1);
        amounts[0] = INITIAL_SUPPLY + 1;

        vm.prank(batchOwner);
        vm.expectRevert("ERC20: transfer amount exceeds balance");
        batchTransfer.batchTransfer(recipients, amounts);
    }

    function test_RevertWhen_ZeroAddressRecipient() public {
        address[] memory recipients = new address[](1);
        recipients[0] = address(0);

        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1000e18;

        vm.prank(batchOwner);
        vm.expectRevert("ERC20: transfer to the zero address");
        batchTransfer.batchTransfer(recipients, amounts);
    }

    function test_RevertWhen_NotWhitelisted() public {
        DeployKarmaBatchTransferScript deployScript = new DeployKarmaBatchTransferScript();
        (KarmaBatchTransfer unwhitelisted,) = deployScript.runForTest(address(karma), batchOwner);

        vm.prank(owner);
        karma.mint(address(unwhitelisted), 10_000e18);

        address[] memory recipients = new address[](1);
        recipients[0] = alice;

        uint256[] memory amounts = new uint256[](1);
        amounts[0] = 1000e18;

        vm.prank(batchOwner);
        vm.expectRevert(Karma.Karma__TransfersNotAllowed.selector);
        unwhitelisted.batchTransfer(recipients, amounts);
    }
}
