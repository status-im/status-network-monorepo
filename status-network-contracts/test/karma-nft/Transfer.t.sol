// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { KarmaNFT } from "../../src/KarmaNFT.sol";
import { KarmaNFTTest } from "./KarmaNFTBase.t.sol";

contract TransferTest is KarmaNFTTest {
    function setUp() public override {
        super.setUp();
    }

    function testTransferNotAllowed() public {
        vm.expectRevert(KarmaNFT.KarmaNFT__TransferNotAllowed.selector);
        nft.transferFrom(alice, address(0), addressToId(alice));
    }

    function testSafeTransferNotAllowed() public {
        vm.expectRevert(KarmaNFT.KarmaNFT__TransferNotAllowed.selector);
        nft.safeTransferFrom(alice, address(0), addressToId(alice));
    }

    function testSafeTransferWithDataNotAllowed() public {
        vm.expectRevert(KarmaNFT.KarmaNFT__TransferNotAllowed.selector);
        nft.safeTransferFrom(alice, address(0), addressToId(alice), "");
    }
}

