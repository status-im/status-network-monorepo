// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { KarmaAirdrop } from "../../src/KarmaAirdrop.sol";
import { KarmaAirdropTest } from "./KarmaAirdropBase.t.sol";

contract SetMerkleRootTest is KarmaAirdropTest {
    bytes32 internal merkleRoot = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;

    function setUp() public override {
        super.setUp();
    }

    function test_RevertWhen_NotOwner() public {
        vm.prank(address(0x1234));
        vm.expectRevert("Ownable: caller is not the owner");
        airdrop.setMerkleRoot(merkleRoot);
    }

    function test_RevertWhen_UpdateMerkleRootNotAllowed() public {
        bytes32 newMerkleRoot = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabb;
        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);
        vm.prank(owner);
        vm.expectRevert(KarmaAirdrop.KarmaAirdrop__MerkleRootAlreadySet.selector);
        airdrop.setMerkleRoot(newMerkleRoot);
    }

    function test_RevertWhen_NotPaused() public {
        KarmaAirdrop updatableAirdrop = new KarmaAirdrop(address(rewardToken), owner, true, defaultDelegatee);

        bytes32 newMerkleRoot = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabb;

        // Set initial merkle root (first time, no pause required)
        vm.prank(owner);
        updatableAirdrop.setMerkleRoot(merkleRoot);
        assertEq(updatableAirdrop.merkleRoot(), merkleRoot);

        // Try to update merkle root without pausing (should fail)
        vm.prank(owner);
        vm.expectRevert(KarmaAirdrop.KarmaAirdrop__MustBePausedToUpdate.selector);
        updatableAirdrop.setMerkleRoot(newMerkleRoot);
    }

    function test__UpdateMerkleRootWhenAllowed() public {
        KarmaAirdrop updatableAirdrop = new KarmaAirdrop(address(rewardToken), owner, true, defaultDelegatee);

        bytes32 newMerkleRoot = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabb;

        // Set initial merkle root (first time, no pause required)
        vm.prank(owner);
        updatableAirdrop.setMerkleRoot(merkleRoot);
        assertEq(updatableAirdrop.merkleRoot(), merkleRoot);

        // Pause the contract before updating
        vm.prank(owner);
        updatableAirdrop.pause();

        // Update merkle root (should succeed when paused)
        vm.prank(owner);
        updatableAirdrop.setMerkleRoot(newMerkleRoot);
        assertEq(updatableAirdrop.merkleRoot(), newMerkleRoot);
    }

    function test_UpdateMerkleRootIncreasesEpoch() public {
        KarmaAirdrop updatableAirdrop = new KarmaAirdrop(address(rewardToken), owner, true, defaultDelegatee);

        bytes32 newMerkleRoot = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabb;
        bytes32 thirdMerkleRoot = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaacc;

        // Initial epoch should be 0
        assertEq(updatableAirdrop.epoch(), 0);

        // Set initial merkle root (first time) - epoch should remain 0
        vm.prank(owner);
        updatableAirdrop.setMerkleRoot(merkleRoot);
        assertEq(updatableAirdrop.epoch(), 0);

        // Pause and update merkle root - epoch should increase to 1
        vm.startPrank(owner);
        updatableAirdrop.pause();
        updatableAirdrop.setMerkleRoot(newMerkleRoot);
        assertEq(updatableAirdrop.epoch(), 1);

        // Update again - epoch should increase to 2
        updatableAirdrop.setMerkleRoot(thirdMerkleRoot);
        assertEq(updatableAirdrop.epoch(), 2);
        vm.stopPrank();
    }

    function test_UpdateMerkleRootResetsClaimedBitmap() public {
        KarmaAirdrop updatableAirdrop = new KarmaAirdrop(address(rewardToken), owner, true, defaultDelegatee);

        // Set up first merkle tree
        uint256 index = 0;
        uint256 accountPrivateKey = 0xa11ce;
        address account = vm.addr(accountPrivateKey);
        uint256 amount = 100e18;
        bytes32 leaf = keccak256(abi.encodePacked(index, account, amount));
        bytes32[] memory merkleProof = new bytes32[](0);

        // Fund the airdrop contract
        rewardToken.mint(address(updatableAirdrop), amount * 2);

        // Set initial merkle root and claim
        vm.prank(owner);
        updatableAirdrop.setMerkleRoot(leaf);

        // Generate delegation signature
        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) =
            _generateDelegationSignature(accountPrivateKey, defaultDelegatee, nonce, expiry);

        updatableAirdrop.claim(index, account, amount, merkleProof, nonce, expiry, v, r, s);
        assertTrue(updatableAirdrop.isClaimed(index));

        // Pause before updating merkle root
        vm.prank(owner);
        updatableAirdrop.pause();

        // Update merkle root - this should reset the bitmap
        bytes32 newMerkleRoot = keccak256(abi.encodePacked(index, account, amount));
        vm.prank(owner);
        updatableAirdrop.setMerkleRoot(newMerkleRoot);

        // Unpause to allow claims
        vm.prank(owner);
        updatableAirdrop.unpause();

        // Verify the claim was reset
        assertFalse(updatableAirdrop.isClaimed(index));

        // Should be able to claim again with new merkle tree
        // Generate new delegation signature (nonce would still be 0 for new epoch, but the account now has a balance)
        (v, r, s) = _generateDelegationSignature(accountPrivateKey, defaultDelegatee, nonce, expiry);
        updatableAirdrop.claim(index, account, amount, merkleProof, nonce, expiry, v, r, s);
        assertTrue(updatableAirdrop.isClaimed(index));
        assertEq(rewardToken.balanceOf(account), amount * 2);
    }

    function test_SetMerkleRoot() public {
        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);
        assertEq(airdrop.merkleRoot(), merkleRoot);
    }
}

