// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { KarmaAirdrop } from "../../src/KarmaAirdrop.sol";
import { KarmaAirdropTest } from "./KarmaAirdropBase.t.sol";

contract ClaimTest is KarmaAirdropTest {
    uint256 internal alicePrivateKey;
    address internal alice;

    function setUp() public override {
        super.setUp();
        alicePrivateKey = 0xa11ce;
        alice = vm.addr(alicePrivateKey);
    }

    function _hashPair(bytes32 a, bytes32 b) public pure returns (bytes32) {
        return a < b ? keccak256(abi.encodePacked(a, b)) : keccak256(abi.encodePacked(b, a));
    }

    function test_RevertWhen_Paused() public {
        uint256 index = 0;
        uint256 amount = 100e18;

        bytes32 leaf = keccak256(abi.encodePacked(index, alice, amount));
        bytes32 merkleRoot = leaf;
        bytes32[] memory merkleProof = new bytes32[](0);

        rewardToken.mint(address(airdrop), amount);

        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);

        // Pause the contract
        vm.prank(owner);
        airdrop.pause();

        // Generate delegation signature
        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) =
            _generateDelegationSignature(alicePrivateKey, defaultDelegatee, nonce, expiry);

        // Attempt to claim while paused should revert
        vm.expectRevert("Pausable: paused");
        airdrop.claim(index, alice, amount, merkleProof, nonce, expiry, v, r, s);
    }

    function test_RevertWhen_ClaimBeforeMerkleRootSet() public {
        uint256 index = 0;
        uint256 amount = 100e18;
        bytes32[] memory merkleProof = new bytes32[](0);

        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) =
            _generateDelegationSignature(alicePrivateKey, defaultDelegatee, nonce, expiry);

        vm.expectRevert(KarmaAirdrop.KarmaAirdrop__MerkleRootNotSet.selector);
        airdrop.claim(index, alice, amount, merkleProof, nonce, expiry, v, r, s);
    }

    function test_ClaimWithValidProof() public {
        // Set up test data
        uint256 index = 0;
        uint256 amount = 100e18;

        // Create a simple merkle tree with one leaf
        // Leaf: keccak256(abi.encodePacked(index, account, amount))
        bytes32 leaf = keccak256(abi.encodePacked(index, alice, amount));
        bytes32 merkleRoot = leaf; // Single leaf tree - root equals leaf
        bytes32[] memory merkleProof = new bytes32[](0); // Empty proof for single leaf

        // Fund the airdrop contract with tokens
        rewardToken.mint(address(airdrop), amount);

        // Set merkle root
        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);

        // Verify initial state
        assertFalse(airdrop.isClaimed(index));
        assertEq(rewardToken.balanceOf(alice), 0);
        assertEq(rewardToken.balanceOf(address(airdrop)), amount);

        // Generate delegation signature
        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) =
            _generateDelegationSignature(alicePrivateKey, defaultDelegatee, nonce, expiry);

        // Claim tokens
        vm.expectEmit(true, true, true, true);
        emit KarmaAirdrop.Claimed(index, alice, amount);
        airdrop.claim(index, alice, amount, merkleProof, nonce, expiry, v, r, s);

        // Verify final state
        assertTrue(airdrop.isClaimed(index));
        assertEq(rewardToken.balanceOf(alice), amount);
        assertEq(rewardToken.balanceOf(address(airdrop)), 0);
    }

    function test_ClaimFromComplexMerkleTree() public {
        //          root
        //         /    \
        //      node01  node23
        //      /  \    /  \
        //   leaf0 leaf1 leaf2 leaf3
        //   (alice)(bob)(charlie)(david)
        //
        //   For Bob's claim (index 1), the proof consists of:
        //   1. leaf0 (Alice's leaf) - Bob's sibling
        //   2. node23 (Charlie+David's combined node) - The uncle node

        bytes32 leaf0 = keccak256(abi.encodePacked(uint256(0), vm.addr(0xa11ce), uint256(100e18))); // alice
        bytes32 leaf1 = keccak256(abi.encodePacked(uint256(1), vm.addr(0xb0b), uint256(200e18))); // bob
        bytes32 leaf2 = keccak256(abi.encodePacked(uint256(2), vm.addr(0xc4a411e), uint256(300e18))); // charlie
        bytes32 leaf3 = keccak256(abi.encodePacked(uint256(3), makeAddr("david"), uint256(400e18)));

        bytes32 node01 = _hashPair(leaf0, leaf1);
        bytes32 node23 = _hashPair(leaf2, leaf3);
        bytes32 merkleRoot = _hashPair(node01, node23);

        rewardToken.mint(address(airdrop), 1000e18);

        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);

        bytes32[] memory merkleProofBob = new bytes32[](2);
        merkleProofBob[0] = leaf0; // Sibling of leaf1
        merkleProofBob[1] = node23; // Uncle node

        bytes32[] memory merkleProofCharlie = new bytes32[](2);
        merkleProofCharlie[0] = leaf3;
        merkleProofCharlie[1] = node01; // Uncle node

        // Verify initial state
        assertFalse(airdrop.isClaimed(1));
        assertEq(rewardToken.balanceOf(vm.addr(0xb0b)), 0);

        // Generate delegation signature for Bob
        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _generateDelegationSignature(0xb0b, defaultDelegatee, nonce, expiry);

        // Claim tokens
        vm.expectEmit(true, true, true, true);
        emit KarmaAirdrop.Claimed(1, vm.addr(0xb0b), 200e18);
        airdrop.claim(1, vm.addr(0xb0b), 200e18, merkleProofBob, nonce, expiry, v, r, s);

        // Verify final state
        assertTrue(airdrop.isClaimed(1));
        assertEq(rewardToken.balanceOf(vm.addr(0xb0b)), 200e18);
        assertEq(rewardToken.balanceOf(address(airdrop)), 800e18);

        // Generate delegation signature for Charlie
        (v, r, s) = _generateDelegationSignature(0xc4a411e, defaultDelegatee, nonce, expiry);

        vm.expectEmit(true, true, true, true);
        emit KarmaAirdrop.Claimed(2, vm.addr(0xc4a411e), 300e18);
        airdrop.claim(2, vm.addr(0xc4a411e), 300e18, merkleProofCharlie, nonce, expiry, v, r, s);
    }

    function test_ClaimDelegatesToDefaultDelegatee() public {
        uint256 index = 0;
        uint256 amount = 100e18;

        bytes32 leaf = keccak256(abi.encodePacked(index, alice, amount));
        bytes32 merkleRoot = leaf;
        bytes32[] memory merkleProof = new bytes32[](0);

        rewardToken.mint(address(airdrop), amount);

        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);

        // Verify alice has no karma balance before claim
        assertEq(rewardToken.balanceOf(alice), 0);

        // Generate delegation signature
        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) =
            _generateDelegationSignature(alicePrivateKey, defaultDelegatee, nonce, expiry);

        // Claim tokens
        airdrop.claim(index, alice, amount, merkleProof, nonce, expiry, v, r, s);

        // Verify the claimed karma is delegated to the default delegatee
        assertEq(rewardToken.delegates(alice), defaultDelegatee);
        assertEq(rewardToken.getVotes(defaultDelegatee), amount);
    }

    function test_ClaimSucceedsWhenDelegationSignatureIsFrontRun() public {
        // This test simulates a griefing attack where an attacker observes the signature
        // from the mempool and front-runs the claim by directly calling delegateBySig,
        // consuming the nonce. The claim should still succeed due to the try/catch block.

        uint256 index = 0;
        uint256 amount = 100e18;

        bytes32 leaf = keccak256(abi.encodePacked(index, alice, amount));
        bytes32 merkleRoot = leaf;
        bytes32[] memory merkleProof = new bytes32[](0);

        rewardToken.mint(address(airdrop), amount);

        vm.prank(owner);
        airdrop.setMerkleRoot(merkleRoot);

        // Verify alice has no karma balance before claim
        assertEq(rewardToken.balanceOf(alice), 0);

        // Generate delegation signature
        uint256 nonce = 0;
        uint256 expiry = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) =
            _generateDelegationSignature(alicePrivateKey, defaultDelegatee, nonce, expiry);

        // Simulate griefing attack: attacker front-runs by calling delegateBySig directly
        // This consumes the nonce before the claim transaction
        address attacker = makeAddr("attacker");
        vm.prank(attacker);
        rewardToken.delegateBySig(defaultDelegatee, nonce, expiry, v, r, s);

        // Verify the nonce was consumed by checking alice's nonce increased
        assertEq(rewardToken.nonces(alice), 1);

        // Now attempt the claim - it should still succeed even though the delegation will fail
        vm.expectEmit(true, true, true, true);
        emit KarmaAirdrop.Claimed(index, alice, amount);
        airdrop.claim(index, alice, amount, merkleProof, nonce, expiry, v, r, s);

        // Verify the claim succeeded
        assertTrue(airdrop.isClaimed(index));
        assertEq(rewardToken.balanceOf(alice), amount);
        assertEq(rewardToken.balanceOf(address(airdrop)), 0);

        // Verify delegation happened in the front-run transaction (before claim)
        // Note: The delegation still succeeded, just not through the claim function
        assertEq(rewardToken.delegates(alice), defaultDelegatee);
        assertEq(rewardToken.getVotes(defaultDelegatee), amount);
    }
}
