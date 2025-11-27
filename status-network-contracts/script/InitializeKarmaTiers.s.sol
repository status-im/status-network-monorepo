// SPDX-License-Identifier: MIT
pragma solidity 0.8.26;

import "forge-std/Script.sol";
import "../src/KarmaTiers.sol";

contract InitializeKarmaTiersScript is Script {
    function run() external {
        address karmaTiersAddr = vm.envOr("KARMA_TIERS_ADDRESS", address(0x729409FAD88CafdA895E41f9ED00Ef4094F8d130));
        
        KarmaTiers karmaTiers = KarmaTiers(karmaTiersAddr);
        
        console.log("KarmaTiers address:", karmaTiersAddr);
        console.log("Owner:", karmaTiers.owner());
        console.log("Current tier count:", karmaTiers.getTierCount());
        
        // Build tiers array - MUST start from minKarma=0 for contiguity
        KarmaTiers.Tier[] memory tiers = new KarmaTiers.Tier[](10);
        
        // Tier 0: 0 karma = 0 tx (no gasless for users without karma)
        tiers[0] = KarmaTiers.Tier({ minKarma: 0, maxKarma: 1, name: "entry", txPerEpoch: 2 });
        tiers[1] = KarmaTiers.Tier({ minKarma: 2, maxKarma: 49, name: "newbie", txPerEpoch: 6 });
        tiers[2] = KarmaTiers.Tier({ minKarma: 50, maxKarma: 499, name: "basic", txPerEpoch: 16 });
        tiers[3] = KarmaTiers.Tier({ minKarma: 500, maxKarma: 4999, name: "active", txPerEpoch: 96 });
        tiers[4] = KarmaTiers.Tier({ minKarma: 5000, maxKarma: 19999, name: "regular", txPerEpoch: 480 });
        tiers[5] = KarmaTiers.Tier({ minKarma: 20000, maxKarma: 99999, name: "power", txPerEpoch: 960 });
        tiers[6] = KarmaTiers.Tier({ minKarma: 100000, maxKarma: 499999, name: "pro", txPerEpoch: 10080 });
        tiers[7] = KarmaTiers.Tier({ minKarma: 500000, maxKarma: 4999999, name: "high-throughput", txPerEpoch: 108000 });
        tiers[8] = KarmaTiers.Tier({ minKarma: 5000000, maxKarma: 9999999, name: "s-tier", txPerEpoch: 240000 });
        tiers[9] = KarmaTiers.Tier({ minKarma: 10000000, maxKarma: type(uint256).max, name: "legendary", txPerEpoch: 480000 });
        
        vm.startBroadcast();
        karmaTiers.updateTiers(tiers);
        vm.stopBroadcast();
        
        console.log("New tier count:", karmaTiers.getTierCount());
        console.log("Tiers initialized successfully!");
    }
}
