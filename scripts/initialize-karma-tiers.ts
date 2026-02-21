#!/usr/bin/env ts-node
/**
 * Initialize Karma Tiers Contract
 *
 * This script sets up the karma tier system with the following tiers:
 *
 * | Tier ID | Name           | Karma Range (tokens)     | TX per Epoch |
 * |---------|----------------|--------------------------|--------------|
 * | 0       | none           | 0 - <1                   | 0            |
 * | 1       | entry          | 1                        | 2            |
 * | 2       | newbie         | >1 - <50                 | 6            |
 * | 3       | basic          | 50 - <500                | 16           |
 * | 4       | active         | 500 - <5,000             | 96           |
 * | 5       | regular        | 5,000 - <20,000          | 480          |
 * | 6       | power          | 20,000 - <100,000        | 960          |
 * | 7       | pro            | 100,000 - <500,000       | 10,080       |
 * | 8       | high-throughput| 500,000 - <5,000,000     | 108,000      |
 * | 9       | s-tier         | 5,000,000 - <10,000,000  | 240,000      |
 * | 10      | legendary      | 10,000,000+              | 480,000      |
 *
 * Tier 0 ("none") has txPerEpoch=0, so users with 0 karma cannot make gasless
 * transactions. The contract requires tiers to start at minKarma=0.
 *
 * Karma amounts use 18 decimals (same as the Karma ERC20 token), so values
 * are specified via ethers.parseEther() to convert token amounts to wei.
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

// Tier definitions - matches InitializeKarmaTiers.s.sol (Foundry script)
// Note: Tiers must be contiguous - each tier's minKarma = previous tier's maxKarma + 1
// Note: KarmaTiers.sol contract REQUIRES first tier to start at minKarma = 0
// Note: Karma ERC20 uses 18 decimals, so all amounts are in wei (use parseEther)
// Note: Tier 0 ("none") has txPerEpoch=0 - users without karma cannot transact gaslessly
const e = (n: string) => ethers.parseEther(n);
const KARMA_TIERS = [
  { name: "none",            minKarma: 0n,                        maxKarma: e("1") - 1n,            txPerEpoch: 0 },
  { name: "entry",           minKarma: e("1"),                    maxKarma: e("1"),                  txPerEpoch: 2 },
  { name: "newbie",          minKarma: e("1") + 1n,               maxKarma: e("50") - 1n,            txPerEpoch: 6 },
  { name: "basic",           minKarma: e("50"),                   maxKarma: e("500") - 1n,           txPerEpoch: 16 },
  { name: "active",          minKarma: e("500"),                  maxKarma: e("5000") - 1n,          txPerEpoch: 96 },
  { name: "regular",         minKarma: e("5000"),                 maxKarma: e("20000") - 1n,         txPerEpoch: 480 },
  { name: "power",           minKarma: e("20000"),                maxKarma: e("100000") - 1n,        txPerEpoch: 960 },
  { name: "pro",             minKarma: e("100000"),               maxKarma: e("500000") - 1n,        txPerEpoch: 10080 },
  { name: "high-throughput", minKarma: e("500000"),               maxKarma: e("5000000") - 1n,       txPerEpoch: 108000 },
  { name: "s-tier",          minKarma: e("5000000"),              maxKarma: e("10000000") - 1n,      txPerEpoch: 240000 },
  { name: "legendary",       minKarma: e("10000000"),             maxKarma: ethers.MaxUint256,       txPerEpoch: 480000 },
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
  console.log("─".repeat(75));
  console.log("| ID | Name             | Karma Range (tokens)      | TX/Epoch |");
  console.log("─".repeat(75));
  KARMA_TIERS.forEach((tier, i) => {
    const fmt = (v: bigint) => ethers.formatEther(v);
    const maxDisplay = tier.maxKarma === ethers.MaxUint256 ? "∞" : fmt(tier.maxKarma);
    const range = tier.minKarma === tier.maxKarma
      ? fmt(tier.minKarma).padEnd(25)
      : `${fmt(tier.minKarma)} - ${maxDisplay}`.padEnd(25);
    console.log(`| ${i.toString().padEnd(2)} | ${tier.name.padEnd(16)} | ${range} | ${tier.txPerEpoch.toString().padStart(8)} |`);
  });
  console.log("─".repeat(75));
  
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

