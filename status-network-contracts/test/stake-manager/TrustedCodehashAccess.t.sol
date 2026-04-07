// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import { Clones } from "@openzeppelin/contracts/proxy/Clones.sol";

import { StakeManagerTest } from "./StakeManagerBase.t.sol";
import { StakeVault } from "../../src/StakeVault.sol";

import { ITrustedCodehashAccess } from "../../src/interfaces/ITrustedCodehashAccess.sol";
import { IStakeManager } from "../../src/interfaces/IStakeManager.sol";

contract TrustedCodehashAccessTest is StakeManagerTest {
    function setUp() public virtual override {
        super.setUp();
    }

    function test_RevertWhenProxyCloneCodehashNotTrusted() public {
        // create independent (possibly malicious) StakeVault
        // the reason this tpl will fail is because StakeManager only trusts
        // the proxy clone codehash, not the implementation codehash
        address vaultTpl = address(new StakeVault(stakingToken));
        vm.prank(admin);
        vaultFactory.setVaultImplementation(vaultTpl);

        // registering already fails as codehash is not trusted
        vm.expectRevert(ITrustedCodehashAccess.TrustedCodehashAccess__UnauthorizedCodehash.selector);
        vaultFactory.createVault();
    }

    function test_RevertWhenRogueCloneOfSameImplementationTriesToStake() public {
        // Get the implementation address the factory is currently using.
        // Cloning it produces the exact same bytecode — and therefore the same
        // codehash — as any vault the factory would legitimately deploy.
        address impl = vaultFactory.vaultImplementation();
        StakeVault rogueVault = StakeVault(Clones.clone(impl));
        rogueVault.initialize(address(this), address(streamer));

        // Confirm the codehashes match: the rogue clone is indistinguishable
        // from a legitimate vault at the bytecode level.
        StakeVault legitimateVault = vaultFactory.createVault();
        assertEq(address(rogueVault).codehash, address(legitimateVault).codehash);

        // The rogue vault was never registered through VaultFactory, so
        // StakeManager must reject it even though its codehash is trusted.
        vm.expectRevert(IStakeManager.StakeManager__VaultNotRegistered.selector);
        vm.prank(address(rogueVault));
        streamer.stake(100, 0, 0);
    }

    function test_ClonesOfDifferentImplementationsHaveDifferentCodehashes() public {
        // Deploy a second, independent StakeVault implementation — different
        // deployment address means different bytecode inlined by EIP-1167.
        address implA = vaultFactory.vaultImplementation();
        address implB = address(new StakeVault(stakingToken));

        // The two implementations are at different addresses, so their codehashes
        // differ (immutable constructor arg is baked in).
        assertTrue(implA != implB, "implementations must be at different addresses");

        address cloneA = Clones.clone(implA);
        address cloneB = Clones.clone(implB);

        // EIP-1167 packs the implementation address into the 45-byte proxy
        // bytecode, so clones of different implementations have different codehashes.
        assertTrue(cloneA.codehash != cloneB.codehash, "clones of different impls must have different codehashes");
    }
}
