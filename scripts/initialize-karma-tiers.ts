#!/usr/bin/env ts-node
/**
 * Initialize Karma Tiers Contract
 * 
 * This script sets up the karma tier system with the following tiers:
 * 
 * | Tier ID | Name           | Karma Range           | TX per Epoch |
 * |---------|----------------|----------------------|--------------|
 * | 0       | entry          | 0 - 1                | 2            |
 * | 1       | newbie         | 2 - 49               | 6            |
 * | 2       | basic          | 50 - 499             | 16           |
 * | 3       | active         | 500 - 4,999          | 96           |
 * | 4       | regular        | 5,000 - 19,999       | 480          |
 * | 5       | power          | 20,000 - 99,999      | 960          |
 * | 6       | pro            | 100,000 - 499,999    | 10,080       |
 * | 7       | high-throughput| 500,000 - 4,999,999  | 108,000      |
 * | 8       | s-tier         | 5,000,000 - 9,999,999| 240,000      |
 * | 9       | legendary      | 10,000,000+          | 480,000      |
 * 
 * Note: Users with 0 karma are NOT registered in RLN by the registrar service,
 * so they cannot use gasless transactions regardless of tier.
 * 
 * Usage: npx ts-node scripts/initialize-karma-tiers.ts
 */

import { ethers } from "ethers";

// KarmaTiers ABI (only what we need)
const KARMA_TIERS_ABI = [
  {
    "inputs": [
      {
        "components": [
          { "internalType": "uint256", "name": "minKarma", "type": "uint256" },
          { "internalType": "uint256", "name": "maxKarma", "type": "uint256" },
          { "internalType": "string", "name": "name", "type": "string" },
          { "internalType": "uint32", "name": "txPerEpoch", "type": "uint32" }
        ],
        "internalType": "struct KarmaTiers.Tier[]",
        "name": "newTiers",
        "type": "tuple[]"
      }
    ],
    "name": "updateTiers",
    "outputs": [],
    "stateMutability": "nonpayable",
    "type": "function"
  },
  "function getTierCount() external view returns (uint256)",
  "function getTierById(uint8 tierId) external view returns ((uint256 minKarma, uint256 maxKarma, string name, uint32 txPerEpoch))",
  "function owner() external view returns (address)",
];

// Configuration
const RPC_URL = process.env.RPC_URL || "http://localhost:9045";
const KARMA_TIERS_ADDRESS = process.env.KARMA_TIERS_ADDRESS || "";
// Default: L2 contract deployer account that owns KarmaTiers contract
const PRIVATE_KEY = process.env.PRIVATE_KEY || "0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae";

// Tier definitions based on requirements
// Note: Tiers must be contiguous - each tier's minKarma = previous tier's maxKarma + 1
// Note: Users with 0 karma are NOT registered in RLN, so no "none" tier needed
// The contract requires maxKarma > minKarma, so we start at min=0, max=1 for entry
const KARMA_TIERS = [
  { name: "entry",           minKarma: 0n,          maxKarma: 1n,          txPerEpoch: 2 },
  { name: "newbie",          minKarma: 2n,          maxKarma: 49n,         txPerEpoch: 6 },
  { name: "basic",           minKarma: 50n,         maxKarma: 499n,        txPerEpoch: 16 },
  { name: "active",          minKarma: 500n,        maxKarma: 4999n,       txPerEpoch: 96 },
  { name: "regular",         minKarma: 5000n,       maxKarma: 19999n,      txPerEpoch: 480 },
  { name: "power",           minKarma: 20000n,      maxKarma: 99999n,      txPerEpoch: 960 },
  { name: "pro",             minKarma: 100000n,     maxKarma: 499999n,     txPerEpoch: 10080 },
  { name: "high-throughput", minKarma: 500000n,     maxKarma: 4999999n,    txPerEpoch: 108000 },
  { name: "s-tier",          minKarma: 5000000n,    maxKarma: 9999999n,    txPerEpoch: 240000 },
  { name: "legendary",       minKarma: 10000000n,   maxKarma: ethers.MaxUint256, txPerEpoch: 480000 },
];

async function main() {
  console.log("🎯 Initializing Karma Tiers Contract");
  console.log("=====================================");
  
  if (!KARMA_TIERS_ADDRESS) {
    console.error("❌ KARMA_TIERS_ADDRESS environment variable not set");
    console.log("Usage: KARMA_TIERS_ADDRESS=0x... npx ts-node scripts/initialize-karma-tiers.ts");
    process.exit(1);
  }

  // Connect to network
  const provider = new ethers.JsonRpcProvider(RPC_URL);
  const wallet = new ethers.Wallet(PRIVATE_KEY, provider);
  
  console.log(`📡 RPC URL: ${RPC_URL}`);
  console.log(`👛 Wallet: ${wallet.address}`);
  console.log(`📜 KarmaTiers: ${KARMA_TIERS_ADDRESS}`);
  
  // Connect to contract
  const karmaTiers = new ethers.Contract(KARMA_TIERS_ADDRESS, KARMA_TIERS_ABI, wallet);
  
  // Check ownership
  const owner = await karmaTiers.owner();
  console.log(`👑 Contract owner: ${owner}`);
  
  if (owner.toLowerCase() !== wallet.address.toLowerCase()) {
    console.error(`❌ Wallet ${wallet.address} is not the contract owner`);
    process.exit(1);
  }
  
  // Check current tier count
  const currentTierCount = await karmaTiers.getTierCount();
  console.log(`📊 Current tier count: ${currentTierCount}`);
  
  // Prepare tiers for contract call
  const tierStructs = KARMA_TIERS.map(t => ({
    minKarma: t.minKarma,
    maxKarma: t.maxKarma,
    name: t.name,
    txPerEpoch: t.txPerEpoch,
  }));
  
  console.log("\n📋 Tiers to be set:");
  console.log("─".repeat(70));
  console.log("| ID | Name             | Karma Range               | TX/Epoch |");
  console.log("─".repeat(70));
  KARMA_TIERS.forEach((tier, i) => {
    const maxDisplay = tier.maxKarma === ethers.MaxUint256 ? "∞" : tier.maxKarma.toString();
    const range = tier.minKarma === tier.maxKarma 
      ? tier.minKarma.toString().padEnd(25)
      : `${tier.minKarma} - ${maxDisplay}`.padEnd(25);
    console.log(`| ${i.toString().padEnd(2)} | ${tier.name.padEnd(16)} | ${range} | ${tier.txPerEpoch.toString().padStart(8)} |`);
  });
  console.log("─".repeat(70));
  
  // Update tiers
  console.log("\n🚀 Sending updateTiers transaction...");
  
  try {
    const tx = await karmaTiers.updateTiers(tierStructs, {
      gasLimit: 1000000,
      gasPrice: ethers.parseUnits("15", "gwei"), // Premium gas to bypass RLN
    });
    console.log(`📤 TX Hash: ${tx.hash}`);
    
    const receipt = await tx.wait();
    console.log(`✅ Transaction confirmed in block ${receipt.blockNumber}`);
    
    // Verify tiers were set correctly
    const newTierCount = await karmaTiers.getTierCount();
    console.log(`\n📊 New tier count: ${newTierCount}`);
    
    if (Number(newTierCount) === KARMA_TIERS.length) {
      console.log("✅ All tiers initialized successfully!");
    } else {
      console.error(`❌ Expected ${KARMA_TIERS.length} tiers, got ${newTierCount}`);
      process.exit(1);
    }
    
  } catch (error: unknown) {
    const err = error instanceof Error ? error : new Error(String(error));
    console.error("❌ Failed to update tiers:", err.message);
    process.exit(1);
  }
}

main().catch(console.error);

