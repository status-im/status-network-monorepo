// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Test } from "forge-std/Test.sol";

import { DeployKarmaAirdropScript } from "../../script/DeployKarmaAirdrop.s.sol";
import { MockVotesToken } from "../mocks/MockVotesToken.sol";

import { KarmaAirdrop } from "../../src/KarmaAirdrop.sol";

contract KarmaAirdropTest is Test {
    KarmaAirdrop internal airdrop;
    MockVotesToken internal rewardToken;

    address internal owner = makeAddr("owner");
    address internal defaultDelegatee = makeAddr("defaultDelegatee");

    function setUp() public virtual {
        rewardToken = new MockVotesToken("Karma Token", "KT");

        DeployKarmaAirdropScript deployScript = new DeployKarmaAirdropScript();
        (airdrop,) = deployScript.runForTest(address(rewardToken), owner, defaultDelegatee);
    }

    function _generateDelegationSignature(
        uint256 signerPrivateKey,
        address delegatee,
        uint256 nonce,
        uint256 expiry
    )
        internal
        view
        returns (uint8 v, bytes32 r, bytes32 s)
    {
        bytes32 domainSeparator = rewardToken.DOMAIN_SEPARATOR();
        bytes32 structHash = keccak256(
            abi.encode(
                keccak256("Delegation(address delegatee,uint256 nonce,uint256 expiry)"), delegatee, nonce, expiry
            )
        );
        bytes32 digest = keccak256(abi.encodePacked("\x19\x01", domainSeparator, structHash));
        (v, r, s) = vm.sign(signerPrivateKey, digest);
    }

    function test_Owner() public view {
        assertEq(airdrop.owner(), owner);
    }
}

