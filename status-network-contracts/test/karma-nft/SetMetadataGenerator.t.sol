// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { MockMetadataGenerator } from "../mocks/MockMetadataGenerator.sol";
import { KarmaNFTTest } from "./KarmaNFTBase.t.sol";

contract SetMetadataGeneratorTest is KarmaNFTTest {
    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_NotOwner() public {
        MockMetadataGenerator newMetadataGenerator = new MockMetadataGenerator("https://new-test.local/");

        vm.prank(alice);
        vm.expectRevert("Ownable: caller is not the owner");
        nft.setMetadataGenerator(address(newMetadataGenerator));
    }

    function testSetMetadataGenerator() public {
        MockMetadataGenerator newMetadataGenerator = new MockMetadataGenerator("https://new-test.local/");

        vm.prank(deployer);
        nft.setMetadataGenerator(address(newMetadataGenerator));

        assertEq(address(nft.metadataGenerator()), address(newMetadataGenerator));
    }
}

