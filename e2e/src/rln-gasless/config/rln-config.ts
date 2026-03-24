/**
 * RLN Test Configuration
 * Contains deployed contract addresses and service URLs
 *
 * PRODUCTION MODE: All tests run against real RLN infrastructure
 * Tests will mint Karma, wait for registration, and verify actual quota enforcement
 */

import * as fs from "fs";
import * as path from "path";

// Path to deployment files (relative to workspace root)
const DEPLOYMENTS_DIR = path.resolve(__dirname, "../../../../status-network-contracts/deployments");

// Read contract address from deployment file, env var, or use default
const getContractAddress = (deploymentFile: string, envVar: string, defaultAddr: string): string => {
  // 1. Check environment variable first (allows override)
  if (process.env[envVar]) {
    return process.env[envVar]!;
  }

  // 2. Try to read from deployment file
  try {
    const filePath = path.join(DEPLOYMENTS_DIR, deploymentFile);
    const address = fs.readFileSync(filePath, "utf-8").trim();
    if (address && address.startsWith("0x")) {
      return address;
    }
  } catch {
    // File doesn't exist or can't be read, use default
  }

  // 3. Fall back to default
  return defaultAddr;
};

const getEnvNumber = (envVar: string, defaultVal: number): number => {
  const val = process.env[envVar];
  return val ? parseInt(val, 10) : defaultVal;
};

// Karma is an ERC20 with 18 decimals
const ETHER = 10n ** 18n;

export const RLN_CONFIG = {
  // Mode detection - default to production mode for real testing
  isProductionMode: process.env.RLN_PRODUCTION_MODE !== "false",

  // Contract addresses (read from deployment files, env vars, or defaults)
  // Deployment files are written by `make start-env-with-rln-production`
  contracts: {
    karmaTiers: getContractAddress(
      "karma_tiers_address.txt",
      "KARMA_TIERS_SC_ADDRESS",
      "0x729409fad88cafda895e41f9ed00ef4094f8d130",
    ),
    karma: getContractAddress("karma_address.txt", "KARMA_SC_ADDRESS", "0xe537D669CA013d86EBeF1D64e40fC74CADC91987"),
    rln: getContractAddress("rln_address.txt", "RLN_SC_ADDRESS", "0x5C95Bcd50E6D1B4E3CDC478484C9030Ff0a7D493"),
    stakeManager: getContractAddress(
      "stake_manager_address.txt",
      "STAKE_MANAGER_SC_ADDRESS",
      "0xeb0b0a14f92e3ba35aef3a2b6a24d7ed1d11631b",
    ),
    karmaNFT: getContractAddress(
      "karma_nft_address.txt",
      "KARMA_NFT_SC_ADDRESS",
      "0xcc1b08b17301e090cbb4c1f5598cbaa096d591fb",
    ),
  },

  // Service URLs
  // Note: RLN Prover is gRPC-only (no REST API). Deny list status is checked
  // via linea_estimateGas on the RPC node / sequencer.
  services: {
    // RPC node (l2-node-besu) has the prover forwarder plugin - user-facing
    rpcUrl: process.env.RPC_URL || "http://localhost:9045",
    // Gasless txs go to the RPC node which has the prover forwarder enabled
    sequencerUrl: process.env.SEQUENCER_URL || "http://localhost:9045",
    rlnProverUrl: process.env.RLN_PROVER_URL || "http://localhost:50051",
  },

  // Test configuration
  // Note: Deny list is now stored in the RLN prover's PostgreSQL database
  // and accessed via gRPC - no file path needed
  //
  // EPOCH CONFIGURATION:
  // - Epoch: Duration between quota resets (production: 24h, test: 60s)
  // - Epoch Slice: Subdivision of epoch for internal tracking (test: 10s)
  //
  // The prover is started with:
  //   --epoch-duration-secs=60  (quotas reset every 60 seconds)
  //   --epoch-slice-secs=10     (internal slices every 10 seconds)
  //
  // The sequencer uses --plugin-linea-rln-epoch-mode=TEST which uses a fixed epoch ID for proofs.
  // This is separate from quota tracking - proofs always validate, but quotas reset per epoch.
  test: {
    premiumGasThresholdGwei: 12,
    // Epoch duration must match prover's --epoch-duration-secs (30s in production test mode)
    // Quotas reset every epoch, enabling epoch boundary tests
    epochDurationSeconds: getEnvNumber("RLN_EPOCH_DURATION_SECONDS", 60),
    // MEASURED TIMINGS (from benchmarks with fast polling):
    // Timing expectations (2s block time):
    // - Karma mint TX: ~2-4s (submit + mine)
    // - Prover sees event + registers user: ~4-5s
    // - Total user setup: ~8-10s (fund + mint + registration wait)
    // - Gasless TX: ~2-4s (proof generation + mining)
    proofTimeoutMs: getEnvNumber("RLN_PROOF_TIMEOUT_MS", 5000),
    // Fixed wait time for prover to register user after karma mint
    registrationTimeoutMs: getEnvNumber("RLN_REGISTRATION_TIMEOUT_MS", 30000), // 30s fixed wait for batch registration
    transactionTimeoutMs: getEnvNumber("RLN_TX_TIMEOUT_MS", 40000), // 40s for tx mining (concurrent proof generation can queue up)
    // Wait times for polling operations
    denyListPollIntervalMs: 1000, // 1s between polls
    // Deny list entries are epoch-aligned — cleared when a new epoch starts.
    // Premium gas removes from deny list AND resets epoch counter (quota refresh).
    // 65s = epoch duration (60s) + buffer for epoch boundary tests like GAS_006.
    maxWaitForDenyListMs: 65000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1), // 65s local, 195s remote (3x)
    // Jest test timeouts, scaled by TEST_TIMEOUT_MULTIPLIER for remote testnets
    // Local Docker: TEST_TIMEOUT_MULTIPLIER=1 (default)
    // Remote testnet: TEST_TIMEOUT_MULTIPLIER=3
    timeouts: {
      singleTx: 30_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
      multiTx: 120_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
      denyList: 180_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
      highVolume: 180_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
      epoch: 240_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
      setup: 180_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
      setupLarge: 480_000 * getEnvNumber("TEST_TIMEOUT_MULTIPLIER", 1),
    },
  },

  // Karma tiers with quotas per epoch
  // These match the deployed KarmaTiers contract exactly
  // Users can send exactly 'quota' gasless transactions per epoch, then they're added to deny list
  // Tier 0 ("none") has txPerEpoch=0, users with <1 Karma cannot send gasless transactions
  // Karma is an ERC20 with 18 decimals, so all karma values are in wei (1 Karma = 1e18)
  tiers: {
    entry: { karma: 1n * ETHER, quota: 2, name: "entry" }, // 2 gasless txs per epoch
    newbie: { karma: 2n * ETHER, quota: 6, name: "newbie" }, // 6 gasless txs per epoch
    basic: { karma: 50n * ETHER, quota: 16, name: "basic" }, // 16 gasless txs per epoch
    active: { karma: 500n * ETHER, quota: 96, name: "active" },
    regular: { karma: 5000n * ETHER, quota: 480, name: "regular" },
    power: { karma: 20000n * ETHER, quota: 960, name: "power" },
    pro: { karma: 100000n * ETHER, quota: 10080, name: "pro" },
    "high-throughput": { karma: 500000n * ETHER, quota: 108000, name: "high-throughput" },
    "s-tier": { karma: 5000000n * ETHER, quota: 240000, name: "s-tier" },
    legendary: { karma: 10000000n * ETHER, quota: 480000, name: "legendary" },
  } as const,

  // Tier boundaries for testing edge cases (matches deployed KarmaTiers contract)
  // All values are in wei (18 decimals)
  tierBoundaries: {
    entry: { min: 1n * ETHER, max: 1n * ETHER },
    newbie: { min: 1n * ETHER + 1n, max: 50n * ETHER - 1n },
    basic: { min: 50n * ETHER, max: 500n * ETHER - 1n },
    active: { min: 500n * ETHER, max: 5000n * ETHER - 1n },
    regular: { min: 5000n * ETHER, max: 20000n * ETHER - 1n },
    power: { min: 20000n * ETHER, max: 100000n * ETHER - 1n },
    pro: { min: 100000n * ETHER, max: 500000n * ETHER - 1n },
    "high-throughput": { min: 500000n * ETHER, max: 5000000n * ETHER - 1n },
    "s-tier": { min: 5000000n * ETHER, max: 10000000n * ETHER - 1n },
    legendary: {
      min: 10000000n * ETHER,
      max: BigInt("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
    },
  } as const,

  // Test accounts (from genesis file - private keys)
  accounts: {
    admin: "0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae",
    user1: "0x8f2a55949038a9610f50fb23b5883af3b4ecb3c3bb792cbcefbd1542c692be63",
    user2: "0xc87509a1c067bbde78beb793e6fa76530b6382a4c0241e5e4a9ec0a0f44dc0d3",
    recipient: "0x627306090abaB3A6e1400e9345bC60c78a8BEf57",
  },

  // Pre-registered RLN mock users (from rln-prover mock_users.json)
  // Used as fallback in mock mode only
  mockUsers: {
    user0: {
      address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
      privateKey: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
      txCount: 0,
    },
    user1: {
      address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      privateKey: "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
      txCount: 0,
    },
    user2: {
      address: "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
      privateKey: "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
      txCount: 2,
    },
  },
};

// Type exports for type safety
export type TierName = keyof typeof RLN_CONFIG.tiers;
export type TierConfig = (typeof RLN_CONFIG.tiers)[TierName];
