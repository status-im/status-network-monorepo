// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { KarmaNFT } from "../../src/KarmaNFT.sol";
import { KarmaNFTTest } from "./KarmaNFTBase.t.sol";

contract ApprovalsTest is KarmaNFTTest {
    function setUp() public override {
        super.setUp();
    }

    function testApproveNotAllowed() public {
        vm.expectRevert(KarmaNFT.KarmaNFT__TransferNotAllowed.selector);
        nft.approve(address(0), addressToId(alice));
    }

    function testSetApprovalForAllNotAllowed() public {
        vm.expectRevert(KarmaNFT.KarmaNFT__TransferNotAllowed.selector);
        nft.setApprovalForAll(address(0), true);
    }

    function testGetApproved() public view {
        address approved = nft.getApproved(addressToId(alice));
        assertEq(approved, address(0));
    }

    function testIsApprovedForAll() public view {
        bool isApproved = nft.isApprovedForAll(alice, address(0));
        assertFalse(isApproved);
    }
}
