/**
 * RLN Test Configuration
 * Contains deployed contract addresses and service URLs
 */

export const RLN_CONFIG = {
  // Contract addresses (deployed via make start-env-with-rln-and-contracts)
  contracts: {
    karmaTiers: "0xe4392c8ecc46b304c83cdb5edaf742899b1bda93",
    karma: "0x997fc3af1f193cbdc013060076c67a13e218980e",
    rln: "0xc407c7bc2b3c109b8bcde7c681d84a6a4b600ea5",
    stakeManager: "0x7917abb0cdbf3d3c4057d6a2808ee85ec16260c1",
    karmaNFT: "0x438d5c7da79d918a26ad012c617066293f949d27",
  },

  // Service URLs
  services: {
    rpcUrl: "http://localhost:9045",
    sequencerUrl: "http://localhost:8545",
    karmaServiceUrl: "http://localhost:50053",  // Docker maps 50052 -> 50053
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

