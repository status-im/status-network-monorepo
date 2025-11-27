/**
 * RLN Test Configuration
 * Contains deployed contract addresses and service URLs
 */

export const RLN_CONFIG = {
  // Contract addresses (deployed via make start-env-with-rln-and-contracts)
  // Note: These addresses change on each fresh deployment - update after clean restart
  contracts: {
    karmaTiers: "0x729409fad88cafda895e41f9ed00ef4094f8d130",
    karma: "0x35a5e43d3d3195b49cbfe78cd944115eaa2e09db",
    rln: "0x9145615d34afba9f8ecb4e2384325646f2393dde",
    stakeManager: "0x2f6daaf8a81ab675fbd37ca6ed5b72cf86237453",
    karmaNFT: "0xcc1b08b17301e090cbb4c1f5598cbaa096d591fb",
  },

  // Service URLs
  services: {
    rpcUrl: "http://localhost:9045",
    sequencerUrl: "http://localhost:8545",
    karmaServiceUrl: "http://localhost:50053", // Docker maps 50052 -> 50053
    rlnProverUrl: "http://localhost:50051",
  },

  // Test configuration
  test: {
    denyListPath: "/tmp/test-deny-list.txt",
    premiumGasThresholdGwei: 10,
    premiumGasMultiplier: 1.5,
    epochMode: "TEST", // 60s epochs
    testTimeout: 45000, // 45s per test - L2 should be fast
  },

  // Test accounts (from genesis file - private keys)
  accounts: {
    // Contract deployer account (has balance in genesis)
    admin: "0x1dd171cec7e2995408b5513004e8207fe88d6820aeff0d82463b3e41df251aae",
    user1: "0x8f2a55949038a9610f50fb23b5883af3b4ecb3c3bb792cbcefbd1542c692be63",
    user2: "0xc87509a1c067bbde78beb793e6fa76530b6382a4c0241e5e4a9ec0a0f44dc0d3",
    recipient: "0x627306090abaB3A6e1400e9345bC60c78a8BEf57", // Address
  },

  // Pre-registered RLN mock users (from rln-prover mock_users.json)
  // These addresses are already registered in the RLN prover when running in mock mode
  // Use these for gasless transaction tests
  mockUsers: {
    // Hardhat account #0 - pre-registered in RLN prover mock mode
    user0: {
      address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
      privateKey: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
      txCount: 0,
    },
    // Hardhat account #1 - pre-registered in RLN prover mock mode
    user1: {
      address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      privateKey: "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
      txCount: 0,
    },
    // Hardhat account #2 - pre-registered in RLN prover mock mode (has 2 tx_count)
    user2: {
      address: "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC",
      privateKey: "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a",
      txCount: 2,
    },
    // Special test user from genesis - pre-registered in RLN prover mock mode
    testUser: {
      address: "0x1B9AbEeC3215D8AdE8a33607f2cF0f4F60e5F0D0",
      // Note: This key needs to be added if testing with this account
      privateKey: null,
      txCount: 0,
    },
  },
};
