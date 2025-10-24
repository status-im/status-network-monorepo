// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Base64 } from "@openzeppelin/contracts/utils/Base64.sol";
import { KarmaNFTTest } from "./KarmaNFTBase.t.sol";

contract TokenURITest is KarmaNFTTest {
    function setUp() public override {
        super.setUp();
    }

    function testTokenURI() public view {
        bytes memory expectedMetadata = abi.encodePacked(
            "{\"name\":\"KarmaNFT 0x328809bc894f92807417d2dad6b7c998c1afdac6\",",
            // solhint-disable-next-line
            "\"description\":\"This is a KarmaNFT for address 0x328809bc894f92807417d2dad6b7c998c1afdac6 with balance 10000000000000000000\",",
            "\"image\":\"https://test.local/0x328809bc894f92807417d2dad6b7c998c1afdac6\"}"
        );
        string memory metadata = nft.tokenURI(addressToId(alice));
        assertEq(metadata, string(abi.encodePacked("data:application/json;base64,", Base64.encode(expectedMetadata))));
    }
}
