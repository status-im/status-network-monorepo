// SPDX-License-Identifier: Apache-2.0 OR MIT
pragma solidity ^0.8.26;

import { Test } from "forge-std/Test.sol";
import { Strings } from "@openzeppelin/contracts/utils/Strings.sol";
import { IERC20 } from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import { ERC1967Proxy } from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { RLN } from "../src/rln/RLN.sol";
import { Karma } from "../src/Karma.sol";
import { KarmaDistributorMock } from "./mocks/KarmaDistributorMock.sol";
import { DeployKarmaScript } from "../script/DeployKarma.s.sol";
import { PoseidonHasher } from "../src/rln/PoseidonHasher.sol";
import { IERC20Upgradeable } from "@openzeppelin/contracts-upgradeable/token/ERC20/IERC20Upgradeable.sol";

import { DeploymentConfig } from "../script/DeploymentConfig.s.sol";

contract RLNTest is Test {
    RLN public rln;
    PoseidonHasher public poseidonHasher;

    // Sample private keys (32 bytes)
    bytes32 private privateKey0 = bytes32(uint256(1234));
    bytes32 private privateKey1 = bytes32(uint256(5678));
    bytes32 private privateKey2 = bytes32(uint256(9999));

    // Identity commitments derived from private keys
    uint256 private identityCommitment0;
    uint256 private identityCommitment1;
    uint256 private identityCommitment2;

    // Role‐holders
    address private owner;
    Karma private karma;
    KarmaDistributorMock public distributor1;
    KarmaDistributorMock public distributor2;

    address private adminAddr;
    address private registerAddr;
    address private slasherAddr;

    address private user1Addr = makeAddr("user1");
    address private user2Addr = makeAddr("user2");
    address private user3Addr = makeAddr("user3");
    address private rewardRecipientAddr = makeAddr("rewardRecipient");

    function setUp() public {
        DeployKarmaScript karmaDeployment = new DeployKarmaScript();
        (Karma _karma, DeploymentConfig deploymentConfig) = karmaDeployment.runForTest();
        karma = _karma;
        (address deployer,) = deploymentConfig.activeNetworkConfig();
        owner = deployer;
        distributor1 = new KarmaDistributorMock(IERC20(address(_karma)));
        distributor2 = new KarmaDistributorMock(IERC20(address(_karma)));

        // Deploy PoseidonHasher
        poseidonHasher = new PoseidonHasher();

        // Compute identity commitments from private keys
        identityCommitment0 = poseidonHasher.hash(uint256(privateKey0));
        identityCommitment1 = poseidonHasher.hash(uint256(privateKey1));
        identityCommitment2 = poseidonHasher.hash(uint256(privateKey2));

        // Assign deterministic addresses
        adminAddr = makeAddr("admin");
        registerAddr = makeAddr("register");
        slasherAddr = makeAddr("slasher");

        // Deploy RLN via UUPS proxy
        rln = _deployRLN(karma);

        // Sanity‐check that roles were assigned correctly
        assertTrue(rln.hasRole(rln.DEFAULT_ADMIN_ROLE(), adminAddr));
        assertTrue(rln.hasRole(rln.REGISTER_ROLE(), registerAddr));
        assertTrue(rln.hasRole(rln.SLASHER_ROLE(), slasherAddr));

        vm.startBroadcast(owner);
        karma.addRewardDistributor(address(distributor1));
        karma.addRewardDistributor(address(distributor2));
        karma.grantRole(karma.SLASHER_ROLE(), address(rln));
        karma.setAllowedToTransfer(address(distributor1), true);
        karma.setAllowedToTransfer(address(distributor2), true);
        vm.stopBroadcast();
    }

    /// @dev Deploys a new RLN instance (behind ERC1967Proxy).
    function _deployRLN(Karma karmaToken) internal returns (RLN) {
        bytes memory initData = abi.encodeCall(
            RLN.initialize, (adminAddr, slasherAddr, registerAddr, address(karmaToken), address(poseidonHasher))
        );
        address impl = address(new RLN());
        address proxy = address(new ERC1967Proxy(impl, initData));
        return RLN(proxy);
    }

    /* ---------- INITIAL STATE ---------- */

    function test_initial_state() public view {
        // No identities registered yet
        assertEq(rln.identityCommitmentIndex(), 0);

        // members(...) should return (address(0), 0) for any commitment
        (address user0, uint256 idx0) = _memberData(identityCommitment0);
        assertEq(user0, address(0));
        assertEq(idx0, 0);
    }

    /* ---------- REGISTER ---------- */

    function test_register_succeeds() public {
        // Register first identity
        uint256 indexBefore = rln.identityCommitmentIndex();
        vm.startPrank(registerAddr);
        vm.expectEmit(true, true, true, true);
        emit RLN.MemberRegistered(identityCommitment0, indexBefore);
        rln.register(identityCommitment0, user1Addr);
        vm.stopPrank();

        assertEq(rln.identityCommitmentIndex(), indexBefore + 1);
        (address u0, uint256 i0) = _memberData(identityCommitment0);
        assertEq(u0, user1Addr);
        assertEq(i0, indexBefore);

        // Register second identity
        indexBefore = rln.identityCommitmentIndex();
        vm.startPrank(registerAddr);
        vm.expectEmit(true, true, true, true);
        emit RLN.MemberRegistered(identityCommitment1, indexBefore);
        rln.register(identityCommitment1, user2Addr);
        vm.stopPrank();

        assertEq(rln.identityCommitmentIndex(), indexBefore + 1);
        (address u1, uint256 i1) = _memberData(identityCommitment1);
        assertEq(u1, user2Addr);
        assertEq(i1, indexBefore);
    }

    function test_register_fails_when_duplicate_identity_commitment() public {
        // Register once
        vm.startPrank(registerAddr);
        rln.register(identityCommitment0, user1Addr);
        vm.stopPrank();

        // Attempt to register the same commitment again
        vm.startPrank(registerAddr);
        vm.expectRevert(RLN.RLN__IdCommitmentAlreadyRegistered.selector);
        rln.register(identityCommitment0, user1Addr);
        vm.stopPrank();
    }

    /* ---------- SLASH COMMIT/REVEAL ---------- */
    function test_SlashCommitRevertsIfNoSlashRole() public {
        bytes32 hash = keccak256(abi.encodePacked(privateKey0, rewardRecipientAddr));

        // Attempt to commit without slash role
        vm.expectRevert(
            abi.encodePacked(
                "AccessControl: account ",
                Strings.toHexString(uint160(user1Addr)),
                " is missing role ",
                Strings.toHexString(uint256(rln.SLASHER_ROLE()), 32)
            )
        );
        vm.prank(user1Addr);
        rln.slashCommit(user1Addr, hash);
    }

    function test_SlashCommitAddsNewHashWithSlashRole() public {
        bytes32 hash = keccak256(abi.encodePacked(privateKey0, rewardRecipientAddr));
        bytes32 key = keccak256(abi.encodePacked(slasherAddr, hash));

        // Verify commitment doesn't exist yet
        assertEq(rln.slashCommitments(user1Addr, hash), 0);

        // Commit with slash role
        vm.prank(slasherAddr);
        rln.slashCommit(user1Addr, hash);

        // Verify commitment was added with a revealStartTime
        assertGt(rln.slashCommitments(user1Addr, key), 0);

        // Verify lastRevealStartTime was updated
        assertGt(rln.lastRevealStartTime(user1Addr), 0);
    }

    function test_SlashRevealRevertsIfNoSlashRole() public {
        // Attempt to reveal without slash role
        vm.expectRevert(
            abi.encodePacked(
                "AccessControl: account ",
                Strings.toHexString(uint160(user1Addr)),
                " is missing role ",
                Strings.toHexString(uint256(rln.SLASHER_ROLE()), 32)
            )
        );
        vm.prank(user1Addr);
        rln.slashReveal(user1Addr, privateKey0, rewardRecipientAddr);
    }

    function test_SlashRevealRevertsIfCommitmentDoesntExist() public {
        // Attempt to reveal without committing first
        vm.expectRevert(RLN.RLN__InvalidCommitment.selector);
        vm.prank(slasherAddr);
        rln.slashReveal(user1Addr, "1234", rewardRecipientAddr);
    }

    function test_SlashRevealSlashesAccountAndRemovesHash() public {
        vm.prank(owner);
        karma.mint(user1Addr, 10 ether);

        vm.prank(registerAddr);
        rln.register(identityCommitment0, user1Addr);

        // Retrieve the assigned index
        (, uint256 index0) = _memberData(identityCommitment0);

        // Commit the slash
        bytes32 hash = keccak256(abi.encodePacked(privateKey0, rewardRecipientAddr));
        bytes32 key = keccak256(abi.encodePacked(slasherAddr, hash));

        vm.prank(slasherAddr);
        rln.slashCommit(user1Addr, hash);

        // Verify commitment exists with a revealStartTime
        uint256 revealStartTime = rln.slashCommitments(user1Addr, key);
        assertGt(revealStartTime, 0);

        // Warp time to allow reveal (skip to the reveal window)
        vm.warp(revealStartTime);

        // Burn event
        vm.expectEmit(true, true, true, true);
        emit IERC20Upgradeable.Transfer(user1Addr, address(0), 5 ether);

        // Reward mint event
        vm.expectEmit(true, true, true, true);
        emit IERC20Upgradeable.Transfer(address(0), rewardRecipientAddr, 0.5 ether);

        // Slash event
        vm.expectEmit(true, true, true, true);
        emit RLN.MemberSlashed(index0, slasherAddr);

        // Reveal and slash
        vm.prank(slasherAddr);
        rln.slashReveal(user1Addr, privateKey0, rewardRecipientAddr);

        // Verify commitment was removed
        assertEq(rln.slashCommitments(user1Addr, key), 0);

        // Verify member was slashed
        (address userAddress, uint256 userIndex) = _memberData(identityCommitment0);
        assertEq(userAddress, address(0));
        assertEq(userIndex, 0);
    }

    function test_SlashRevealRevertsIfRevealWindowNotStarted() public {
        vm.prank(owner);
        karma.mint(user1Addr, 10 ether);

        vm.prank(registerAddr);
        rln.register(identityCommitment0, user1Addr);

        // Commit 1 (wrong key)
        bytes32 hash1 = keccak256(abi.encodePacked(privateKey1, rewardRecipientAddr));
        vm.prank(slasherAddr);
        rln.slashCommit(user1Addr, hash1);

        // Commit 2 (right key)
        bytes32 hash2 = keccak256(abi.encodePacked(privateKey0, rewardRecipientAddr));
        vm.prank(slasherAddr);
        rln.slashCommit(user1Addr, hash2);

        // Attempt to reveal before the window starts for the second slash commitment
        vm.expectRevert(RLN.RLN__RevealWindowNotStarted.selector);
        vm.prank(slasherAddr);
        rln.slashReveal(user1Addr, privateKey0, rewardRecipientAddr);
    }

    function test_SlashRevealRevertsIfAccountIsDifferentFromTheOneUsedDuringCommit() public {
        vm.startPrank(owner);
        karma.mint(user1Addr, 10 ether);
        karma.mint(user2Addr, 10 ether);
        vm.stopPrank();

        vm.startPrank(registerAddr);
        rln.register(identityCommitment1, user1Addr);
        rln.register(identityCommitment2, user2Addr);
        vm.stopPrank();

        // commit slash for pk1 and user1
        bytes32 hash1 = keccak256(abi.encodePacked(privateKey1, rewardRecipientAddr));
        vm.prank(slasherAddr);
        rln.slashCommit(user1Addr, hash1);

        // malicious commit trying to slash pk1 using the empty queue of user2
        vm.prank(slasherAddr);
        rln.slashCommit(user2Addr, hash1);

        // Attempt to reveal pk1 using queue for user2, so we skip the first commit in the queue
        vm.expectRevert(RLN.RLN__InvalidCommitment.selector);
        vm.prank(slasherAddr);
        rln.slashReveal(user2Addr, privateKey1, rewardRecipientAddr);
    }

    function test_SlashCommitCreatesQueueForMultipleCommits() public {
        // Commit three slashes for the same account
        bytes32 hash1 = keccak256(abi.encodePacked(privateKey0, rewardRecipientAddr));
        bytes32 key1 = keccak256(abi.encodePacked(slasherAddr, hash1));

        bytes32 hash2 = keccak256(abi.encodePacked(privateKey1, rewardRecipientAddr));
        bytes32 key2 = keccak256(abi.encodePacked(slasherAddr, hash2));

        bytes32 hash3 = keccak256(abi.encodePacked(privateKey2, rewardRecipientAddr));
        bytes32 key3 = keccak256(abi.encodePacked(slasherAddr, hash3));

        vm.startPrank(slasherAddr);
        rln.slashCommit(user1Addr, hash1);
        uint256 revealTime1 = rln.slashCommitments(user1Addr, key1);

        rln.slashCommit(user1Addr, hash2);
        uint256 revealTime2 = rln.slashCommitments(user1Addr, key2);

        rln.slashCommit(user1Addr, hash3);
        uint256 revealTime3 = rln.slashCommitments(user1Addr, key3);
        vm.stopPrank();

        // Verify that each subsequent commit has a later reveal time
        assertGt(revealTime1, 0);
        assertEq(revealTime2, revealTime1 + rln.slashRevealWindowTime());
        assertEq(revealTime3, revealTime2 + rln.slashRevealWindowTime());

        // Verify lastRevealStartTime was updated to the last commit's time
        assertEq(rln.lastRevealStartTime(user1Addr), revealTime3);
    }

    function test_SetSlashRevealWindowTime() public {
        uint256 newWindowTime = 2 hours;

        // Set new window time as admin
        vm.prank(adminAddr);
        rln.setSlashRevealWindowTime(newWindowTime);

        // Verify it was updated
        assertEq(rln.slashRevealWindowTime(), newWindowTime);
    }

    function test_SetSlashRevealWindowTimeRevertsIfNotAdmin() public {
        uint256 newWindowTime = 2 hours;

        // Attempt to set new window time without admin role
        vm.expectRevert(
            abi.encodePacked(
                "AccessControl: account ",
                Strings.toHexString(uint160(user1Addr)),
                " is missing role ",
                Strings.toHexString(uint256(rln.DEFAULT_ADMIN_ROLE()), 32)
            )
        );
        vm.prank(user1Addr);
        rln.setSlashRevealWindowTime(newWindowTime);
    }

    /* ========== HELPERS ========== */

    /// @dev Returns (userAddress, index) for a given identityCommitment.
    function _memberData(uint256 commitment) internal view returns (address userAddress, uint256 index) {
        (userAddress, index) = rln.members(commitment);
        return (userAddress, index);
    }
}
