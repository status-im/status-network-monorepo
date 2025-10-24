// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Test } from "forge-std/Test.sol";
import { MockToken } from "../mocks/MockToken.sol";
import { KarmaNFT } from "../../src/KarmaNFT.sol";
import { DeploymentConfig } from "../../script/DeploymentConfig.s.sol";
import { DeployKarmaNFTScript } from "../../script/DeployKarmaNFT.s.sol";
import { INFTMetadataGenerator } from "../../src/interfaces/INFTMetadataGenerator.sol";
import { MockMetadataGenerator } from "../mocks/MockMetadataGenerator.sol";

contract KarmaNFTTest is Test {
    MockToken public erc20Token;
    INFTMetadataGenerator public metadataGenerator;
    KarmaNFT public nft;
    DeploymentConfig public deploymentConfig;
    address deployer;

    address public alice = makeAddr("alice");

    function setUp() public virtual {
        erc20Token = new MockToken("Test", "TEST");
        metadataGenerator = new MockMetadataGenerator("https://test.local/");
        (nft, deploymentConfig) =
            new DeployKarmaNFTScript().runForTest(address(metadataGenerator), address(erc20Token));
        (deployer,) = deploymentConfig.activeNetworkConfig();

        address[1] memory users = [alice];
        for (uint256 i = 0; i < users.length; i++) {
            erc20Token.mint(users[i], 10e18);
        }
    }

    function addressToId(address addr) internal pure returns (uint256) {
        return uint256(uint160(addr));
    }
}

