// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import { console } from "forge-std/Test.sol";
import { Script } from "forge-std/Script.sol";
import { KarmaTiers } from "../src/KarmaTiers.sol";

contract InitializeKarmaTiersScript is Script {
    function run() external {
        address karmaTiersAddr = vm.envOr("KARMA_TIERS_ADDRESS", address(0x729409FAD88CafdA895E41f9ED00Ef4094F8d130));

        KarmaTiers karmaTiers = KarmaTiers(karmaTiersAddr);

        console.log("KarmaTiers address:", karmaTiersAddr);
        console.log("Owner:", karmaTiers.owner());
        console.log("Current tier count:", karmaTiers.getTierCount());

        // Build tiers array - MUST start from minKarma=0 for contiguity
        KarmaTiers.Tier[] memory tiers = new KarmaTiers.Tier[](11);

        // Tier 0: 0 karma = 0 tx (no gasless for users without karma)
        tiers[0] = KarmaTiers.Tier({ name: "none", minKarma: 0, maxKarma: 1 ether - 1, txPerEpoch: 0 });
        tiers[1] = KarmaTiers.Tier({ name: "entry", minKarma: 1 ether, maxKarma: 1 ether, txPerEpoch: 2 });
        tiers[2] = KarmaTiers.Tier({ name: "newbie", minKarma: 1 ether + 1, maxKarma: 50 ether - 1, txPerEpoch: 6 });
        tiers[3] = KarmaTiers.Tier({ name: "basic", minKarma: 50 ether, maxKarma: 500 ether - 1, txPerEpoch: 16 });
        tiers[4] = KarmaTiers.Tier({ name: "active", minKarma: 500 ether, maxKarma: 5000 ether - 1, txPerEpoch: 96 });
        tiers[5] =
            KarmaTiers.Tier({ name: "regular", minKarma: 5000 ether, maxKarma: 20_000 ether - 1, txPerEpoch: 480 });
        tiers[6] =
            KarmaTiers.Tier({ name: "power", minKarma: 20_000 ether, maxKarma: 100_000 ether - 1, txPerEpoch: 960 });
        tiers[7] =
            KarmaTiers.Tier({ name: "pro", minKarma: 100_000 ether, maxKarma: 500_000 ether - 1, txPerEpoch: 10_080 });
        tiers[8] = KarmaTiers.Tier({
            name: "high-throughput", minKarma: 500_000 ether, maxKarma: 5_000_000 ether - 1, txPerEpoch: 108_000
        });
        tiers[9] = KarmaTiers.Tier({
            name: "s-tier", minKarma: 5_000_000 ether, maxKarma: 10_000_000 ether - 1, txPerEpoch: 240_000
        });
        tiers[10] = KarmaTiers.Tier({
            name: "legendary", minKarma: 10_000_000 ether, maxKarma: type(uint256).max, txPerEpoch: 480_000
        });

        vm.startBroadcast();
        karmaTiers.updateTiers(tiers);
        vm.stopBroadcast();

        console.log("New tier count:", karmaTiers.getTierCount());
        console.log("Tiers initialized successfully!");
    }
}
