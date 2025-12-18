/**
 * RLN Test Configuration
 * Contains deployed contract addresses and service URLs
 *
 * PRODUCTION MODE: All tests run against real RLN infrastructure
 * Tests will mint Karma, wait for registration, and verify actual quota enforcement
 */

// Helper to read contract addresses from environment or use defaults
const getContractAddress = (envVar: string, defaultAddr: string): string => {
  return process.env[envVar] || defaultAddr;
};

const getEnvNumber = (envVar: string, defaultVal: number): number => {
  const val = process.env[envVar];
  return val ? parseInt(val, 10) : defaultVal;
};

export const RLN_CONFIG = {
  // Mode detection - default to production mode for real testing
  isProductionMode: process.env.RLN_PRODUCTION_MODE !== "false",

  // Contract addresses (deployed via make start-env-with-rln-and-contracts)
  // NOTE: These are PROXY addresses for upgradeable contracts (Karma, RLN, StakeManager)
  // IMPORTANT: The deployment files may have saved IMPLEMENTATION addresses instead of PROXY addresses
  // These must be the PROXY addresses for the contracts to work correctly
  contracts: {
    karmaTiers: getContractAddress("KARMA_TIERS_SC_ADDRESS", "0x729409fad88cafda895e41f9ed00ef4094f8d130"),
    karma: getContractAddress("KARMA_SC_ADDRESS", "0xe537D669CA013d86EBeF1D64e40fC74CADC91987"),
    rln: getContractAddress("RLN_SC_ADDRESS", "0x5C95Bcd50E6D1B4E3CDC478484C9030Ff0a7D493"),
    stakeManager: getContractAddress("STAKE_MANAGER_SC_ADDRESS", "0xeb0b0a14f92e3ba35aef3a2b6a24d7ed1d11631b"),
    karmaNFT: getContractAddress("KARMA_NFT_SC_ADDRESS", "0xcc1b08b17301e090cbb4c1f5598cbaa096d591fb"),
  },

  // Service URLs
  // Note: RLN Prover handles both proof generation and karma/deny-list services
  services: {
    rpcUrl: process.env.RPC_URL || "http://localhost:9045",
    sequencerUrl: process.env.SEQUENCER_URL || "http://localhost:8545",
    rlnProverUrl: process.env.RLN_PROVER_URL || "http://localhost:50051",
    // karmaServiceUrl points to the same RLN prover (unified service)
    karmaServiceUrl: process.env.KARMA_SERVICE_URL || "http://localhost:50051",
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
    premiumGasThresholdGwei: 10,
    premiumGasMultiplier: 1.5,
    // Epoch duration must match prover's --epoch-duration-secs (30s in production test mode)
    // Quotas reset every epoch, enabling epoch boundary tests
    epochDurationSeconds: getEnvNumber("RLN_EPOCH_DURATION_SECONDS", 30),
    // MEASURED TIMINGS (from benchmarks with fast polling):
    // - Block production: ~0.1-0.8s (blocks produced on-demand with txs)
    // - Proof generation: ~380ms
    // - Polling interval: 250ms
    // - Expected gasless TX: ~1-2s (proof + block + poll)
    // - Expected premium TX: ~0.5-1s (block + poll)
    // - User setup: ~3-4s (fund TX + mint TX + registration)
    // - Prover DB sync: Can take 1-2s after contract event
    proofTimeoutMs: getEnvNumber("RLN_PROOF_TIMEOUT_MS", 3000),
    registrationTimeoutMs: getEnvNumber("RLN_REGISTRATION_TIMEOUT_MS", 10000), // Includes prover sync time
    transactionTimeoutMs: getEnvNumber("RLN_TX_TIMEOUT_MS", 5000),
    // Wait times for polling operations
    denyListPollIntervalMs: 200,
    maxWaitForDenyListMs: 10000,
  },

  // Karma tiers with quotas per epoch
  // These match the production tier configuration
  // Note: Users with 0 karma are not registered in RLN, so no "none" tier
  // Entry tier starts at min=0 but users need 1+ karma to be registered
  tiers: {
    entry: { karma: 1n, quota: 2, name: "entry" },
    newbie: { karma: 2n, quota: 6, name: "newbie" },
    basic: { karma: 50n, quota: 16, name: "basic" },
    active: { karma: 500n, quota: 96, name: "active" },
    regular: { karma: 5000n, quota: 480, name: "regular" },
    power: { karma: 20000n, quota: 960, name: "power" },
    pro: { karma: 100000n, quota: 10080, name: "pro" },
    "high-throughput": { karma: 500000n, quota: 108000, name: "high-throughput" },
    "s-tier": { karma: 5000000n, quota: 240000, name: "s-tier" },
    legendary: { karma: 10000000n, quota: 480000, name: "legendary" },
  } as const,

  // Tier boundaries for testing edge cases (max karma for each tier)
  tierBoundaries: {
    entry: { min: 0n, max: 1n },
    newbie: { min: 2n, max: 49n },
    basic: { min: 50n, max: 499n },
    active: { min: 500n, max: 4999n },
    regular: { min: 5000n, max: 19999n },
    power: { min: 20000n, max: 99999n },
    pro: { min: 100000n, max: 499999n },
    "high-throughput": { min: 500000n, max: 4999999n },
    "s-tier": { min: 5000000n, max: 9999999n },
    legendary: { min: 10000000n, max: BigInt("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff") },
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
