import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager, resetAdminNonceManager } from "./utils/karma-manager";
import { createFundedWallet, uniqueTxData, TEST_RECIPIENT, PREMIUM_GAS_PRICE } from "./utils/test-helpers";
import { RLN_CONFIG } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";
import {
  formatScenario,
  INT_001,
  INT_002,
  INT_003,
  INT_004,
  INT_005,
  INT_006,
  ERR_001,
  ERR_002,
  ERR_003,
  EDGE_001,
  EDGE_002,
  EDGE_003,
  EDGE_004,
  EDGE_005,
} from "./helpers/scenario";

const logger = createTestLogger();

/**
 * Test Suite: Integration, Error Handling, and Edge Cases
 * (INT_001 to INT_006, ERR_001 to ERR_003, EDGE_001 to EDGE_005)
 *
 * Tests complete end-to-end flows, error handling, and edge cases:
 * - Complete lifecycle from registration to denial to recovery
 * - Multiple concurrent users with different tiers
 * - Service error handling
 * - Edge cases: zero-address, self-transfer, large data, etc.
 *
 */
describe("RLN Integration and Error Handling", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  // Pre-registered user pools
  const entryUsers: ethers.HDNodeWallet[] = [];
  const newbieUsers: ethers.HDNodeWallet[] = [];
  const basicUsers: ethers.HDNodeWallet[] = [];
  const activeUsers: ethers.HDNodeWallet[] = [];
  const fundedUsers: ethers.HDNodeWallet[] = [];
  let entryIdx = 0,
    newbieIdx = 0,
    basicIdx = 0,
    activeIdx = 0,
    fundedIdx = 0;

  const getEntryUser = () =>
    entryUsers[entryIdx++] ||
    (() => {
      throw new Error("Not enough entry users");
    })();
  const getNewbieUser = () =>
    newbieUsers[newbieIdx++] ||
    (() => {
      throw new Error("Not enough newbie users");
    })();
  const getBasicUser = () =>
    basicUsers[basicIdx++] ||
    (() => {
      throw new Error("Not enough basic users");
    })();
  const getActiveUser = () =>
    activeUsers[activeIdx++] ||
    (() => {
      throw new Error("Not enough active users");
    })();
  const getFundedUser = () =>
    fundedUsers[fundedIdx++] ||
    (() => {
      throw new Error("Not enough funded users");
    })();

  // Timeouts based on actual TX performance (~4-5s per gasless TX, P95: 4.7s)
  const TEST_TIMEOUT = 20000;
  // Multi-TX tests: ~5s per TX + buffer
  const MULTI_TX_TIMEOUT = 60000;
  // High volume tests
  const HIGH_VOLUME_TIMEOUT = 120000;
  // Extended timeout for epoch tests (30s epoch + buffer + deny list operations)
  const EPOCH_TEST_TIMEOUT = 240000;

  beforeAll(async () => {
    logger.info("=== Initializing Integration and Error Handling Test Suite ===");

    // Reset nonce manager to sync with blockchain state
    resetAdminNonceManager();

    // Setup providers with fast polling for quicker transaction confirmation detection
    rpcProvider = createFastProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = createFastProvider(RLN_CONFIG.services.sequencerUrl);
    admin = new ethers.Wallet(RLN_CONFIG.accounts.admin, rpcProvider);
    contracts = loadRlnContracts(rpcProvider, admin);

    rlnClient = new RlnTestClient(
      rpcProvider,
      sequencerProvider,
      RLN_CONFIG.services.rpcUrl,
      RLN_CONFIG.services.karmaServiceUrl,
    );

    denyListManager = new DenyListTestManager();
    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);

    // PRE-REGISTER ALL USERS NEEDED FOR THIS TEST SUITE
    logger.info("Pre-registering test users...");

    // Entry users (10 needed)
    for (let i = 0; i < 10; i++) {
      entryUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "entry"));
      logger.debug(`Pre-registered entry user ${i + 1}/10`);
    }
    // Newbie users (5 needed for INT_005)
    for (let i = 0; i < 5; i++) {
      newbieUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "newbie"));
      logger.debug(`Pre-registered newbie user ${i + 1}/5`);
    }
    // Basic users (2 needed)
    for (let i = 0; i < 2; i++) {
      basicUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "basic"));
      logger.debug(`Pre-registered basic user ${i + 1}/2`);
    }
    // Active users (3 needed for high-volume tests)
    for (let i = 0; i < 3; i++) {
      activeUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "active"));
      logger.debug(`Pre-registered active user ${i + 1}/3`);
    }
    // Funded-only users (5 needed)
    for (let i = 0; i < 5; i++) {
      fundedUsers.push(await createFundedWallet(rpcProvider, admin));
      logger.debug(`Pre-funded user ${i + 1}/5`);
    }

    logger.info("Test suite initialized", {
      entryUsers: entryUsers.length,
      newbieUsers: newbieUsers.length,
      basicUsers: basicUsers.length,
      activeUsers: activeUsers.length,
      fundedUsers: fundedUsers.length,
    });
  }, 300000); // 5 minute setup timeout (more users)

  afterAll(async () => {
    logger.info("=== Integration and Error Handling Test Suite Complete ===");
  });

  // ============================================================================
  // INTEGRATION TESTS (INT_001 to INT_006)
  // ============================================================================

  it(
    formatScenario(INT_001),
    async () => {
      const epochDuration = RLN_CONFIG.test.epochDurationSeconds;
      logger.info(`${INT_001.id}: Starting complete lifecycle test`, { epochDurationSeconds: epochDuration });

      // Step 1: Create new user
      const user = getFundedUser();
      logger.info("Step 1: User created", { address: user.address });

      // Step 2: Mint Karma and register (Entry tier)
      await karmaManager.mintKarma(user.address, 1n);
      await karmaManager.waitForRlnRegistration(user.address);
      logger.info("Step 2: User registered with Entry tier");

      // Step 3: Send gasless transactions (exhaust quota)
      const quota = RLN_CONFIG.tiers.entry.quota;
      for (let i = 0; i < quota; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int001-gasless-${i}`),
        });
        expect(receipt.status).toBe(1);
      }
      logger.info(`Step 3: Sent ${quota} gasless transactions`);

      // Step 4: Exhaust quota and get denied
      await rlnClient.sendGaslessTransactionExpectFailure(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int001-exceed"),
      });
      await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);
      expect(await denyListManager.isDenied(user.address)).toBe(true);
      logger.info("Step 4: User denied after quota exhaustion");

      // Step 5: Verify gasless is blocked
      // Denial manifests as timeout (no proof generated) or explicit rejection
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int001-blocked"),
      });
      expect(errorMessage).toMatch(/denied|reject|quota|timeout|resource.*exhausted/i);
      logger.info("Step 5: Gasless blocked while denied");

      // Step 6: Pay premium gas (removes from deny list)
      const premiumReceipt = await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("int001-premium"),
      });
      expect(premiumReceipt.status).toBe(1);
      await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);
      logger.info("Step 6: Premium gas paid, removed from deny list");

      // Step 7: Wait for new epoch (quota resets)
      logger.info(`Step 7: Waiting for new epoch (max ${epochDuration + 2}s)...`);
      await rlnClient.waitForNextEpoch((epochDuration + 2) * 1000);

      // Allow prover to sync state after deny list clearance + epoch change
      await rlnClient.sleep(1000);

      // Step 8: Send gasless again (should work!)
      const finalReceipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int001-final"),
      });
      expect(finalReceipt.status).toBe(1);
      logger.info("Step 8: Gasless working again");

      logger.info(`${INT_001.id}: PASSED ✓`);
    },
    EPOCH_TEST_TIMEOUT, // This test waits for epoch boundary (60s)
  );

  it(
    formatScenario(INT_002),
    async () => {
      logger.info(`${INT_002.id}: Testing multiple users with different tiers`);

      // Create users with different tiers
      const entryUser = getEntryUser();
      const newbieUser = getNewbieUser();
      const basicUser = getBasicUser();

      logger.info("Users created", {
        entry: entryUser.address,
        newbie: newbieUser.address,
        basic: basicUser.address,
      });

      // Send transactions from each user based on their tier
      // Entry: 2 tx
      for (let i = 0; i < 2; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(entryUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int002-entry-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      // Newbie: 3 tx (subset of 6 quota)
      for (let i = 0; i < 3; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(newbieUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int002-newbie-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      // Basic: 4 tx (subset of 16 quota)
      for (let i = 0; i < 4; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(basicUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int002-basic-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      logger.info(`${INT_002.id}: PASSED ✓`);
    },
    HIGH_VOLUME_TIMEOUT, // 2 + 3 + 4 = 9 TXs = ~36s
  );

  it(
    formatScenario(INT_003),
    async () => {
      const user = getActiveUser();
      const txCount = 5;

      logger.info(`${INT_003.id}: Testing rapid sequential transactions`, {
        user: user.address,
        txCount,
      });

      const startTime = Date.now();

      for (let i = 0; i < txCount; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int003-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      const duration = Date.now() - startTime;

      logger.info(`${INT_003.id}: PASSED ✓`, {
        txCount,
        durationMs: duration,
        avgPerTx: `${Math.round(duration / txCount)}ms`,
      });
    },
    MULTI_TX_TIMEOUT, // 5 TXs = ~20s
  );

  it(
    formatScenario(INT_004),
    async () => {
      const user = getEntryUser();
      const epochDuration = RLN_CONFIG.test.epochDurationSeconds;

      logger.info(`${INT_004.id}: Testing epoch transition`, {
        user: user.address,
        epochDurationSeconds: epochDuration,
      });

      const epoch1 = rlnClient.getCurrentEpoch();
      logger.info("Current epoch", { epoch: epoch1 });

      // Send transaction in current epoch
      const receipt1 = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int004-epoch1"),
      });
      expect(receipt1.status).toBe(1);

      // Wait for epoch transition
      logger.info(`Waiting for epoch transition (max ${epochDuration + 2}s)...`);
      const epoch2 = await rlnClient.waitForNextEpoch((epochDuration + 2) * 1000);
      logger.info("New epoch started", { epoch: epoch2 });

      // Send transaction in new epoch
      const receipt2 = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int004-epoch2"),
      });
      expect(receipt2.status).toBe(1);

      //  Both transactions succeeded across epoch boundary
      expect(epoch2).toBeGreaterThan(epoch1);

      logger.info(`${INT_004.id}: PASSED ✓`);
    },
    EPOCH_TEST_TIMEOUT, // This test waits for epoch boundary (60s)
  );

  it(
    formatScenario(INT_005),
    async () => {
      logger.info(`${INT_005.id}: Testing concurrent transaction safety`);

      const users = [getNewbieUser(), getNewbieUser(), getNewbieUser()];

      // Send 2 concurrent transactions from each user using proper nonce management
      const allResults: ethers.TransactionReceipt[] = [];
      for (let userIdx = 0; userIdx < users.length; userIdx++) {
        const user = users[userIdx];
        const receipts = await rlnClient.sendGaslessTransactionsConcurrent(user, [
          { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData(`int005-user${userIdx}-tx0`) },
          { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData(`int005-user${userIdx}-tx1`) },
        ]);
        allResults.push(...receipts);
      }

      const results = allResults;

      //  All transactions must succeed
      for (const receipt of results) {
        expect(receipt.status).toBe(1);
      }

      // Verify each user's quota was tracked independently
      for (let userIdx = 0; userIdx < users.length; userIdx++) {
        const user = users[userIdx];
        // Each user sent 2 tx, should have 4 remaining (newbie quota = 6)
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int005-user${userIdx}-verify`),
        });
        expect(receipt.status).toBe(1);
      }

      logger.info(`${INT_005.id}: PASSED ✓`);
    },
    HIGH_VOLUME_TIMEOUT, // 5 users × 3 TXs = 15 TXs = ~60s
  );

  it(
    formatScenario(INT_006),
    async () => {
      // Use Active tier (quota = 96) to test higher volume
      const user = getActiveUser();
      const txCount = 10;

      logger.info(`${INT_006.id}: Testing high-volume quota tracking`, {
        user: user.address,
        txCount,
      });

      for (let i = 0; i < txCount; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int006-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      logger.info(`${INT_006.id}: PASSED ✓`);
    },
    HIGH_VOLUME_TIMEOUT, // 10 TXs = ~40s
  );

  // ============================================================================
  // ERROR HANDLING TESTS (ERR_001 to ERR_003)
  // ============================================================================

  it(
    formatScenario(ERR_001),
    async () => {
      logger.info(`${ERR_001.id}: Testing karma service error handling`);

      // Create a client with invalid karma service URL
      const badClient = new RlnTestClient(
        rpcProvider,
        sequencerProvider,
        RLN_CONFIG.services.rpcUrl,
        "http://localhost:99999", // Invalid URL
      );

      const user = getEntryUser();

      // Try to get tier info with bad client - should fail gracefully
      try {
        await badClient.getUserTierInfo(user.address);
        logger.info(`${ERR_001.id}: Client handled unavailable service gracefully`);
      } catch (error: unknown) {
        const err = error instanceof Error ? error : new Error(String(error));
        // Expected to fail with connection error
        expect(err.message).toMatch(/failed|unavailable|ECONNREFUSED|fetch|connect/i);
        logger.info(`${ERR_001.id}: Error handled correctly`, { error: err.message });
      }

      // Verify the user can still send gasless transactions via the working client
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("err001-verify"),
      });
      expect(receipt.status).toBe(1);

      logger.info(`${ERR_001.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(ERR_002),
    async () => {
      // Test by using unregistered user (prover won't generate proof)
      const user = getFundedUser();

      logger.info(`${ERR_002.id}: Testing prover unavailable handling`, {
        user: user.address,
      });

      const startTime = Date.now();
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("err002"),
        },
        RLN_CONFIG.test.proofTimeoutMs,
      );
      const duration = Date.now() - startTime;

      // STRONG ASSERTIONS
      expect(errorMessage).toMatch(/timeout|rejected|proof/i);
      expect(duration).toBeLessThan(RLN_CONFIG.test.proofTimeoutMs + 3000);

      logger.info(`${ERR_002.id}: PASSED ✓`, {
        durationMs: duration,
      });
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(ERR_003),
    async () => {
      const user = getEntryUser();

      logger.info(`${ERR_003.id}: Testing larger data handling`, {
        user: user.address,
      });

      // Test with moderately large data (1KB)
      const largeData = "0x" + "ab".repeat(500);

      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: largeData,
        gasLimit: 50000, // Higher gas limit for larger data
      });

      //  Large data transaction must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${ERR_003.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  // ============================================================================
  // EDGE CASE TESTS (EDGE_001 to EDGE_005)
  // ============================================================================

  it(
    formatScenario(EDGE_001),
    async () => {
      const user = getEntryUser();

      logger.info(`${EDGE_001.id}: Testing self-transfer`, {
        user: user.address,
      });

      // Send gasless transaction to self
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: user.address, // Self-transfer
        value: 0n,
        data: uniqueTxData("edge001-self"),
      });

      //  Self-transfer must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${EDGE_001.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(EDGE_002),
    async () => {
      const user = getEntryUser();

      logger.info(`${EDGE_002.id}: Testing empty data transaction`, {
        user: user.address,
      });

      // Send gasless transaction with empty data
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: "0x", // Empty data
      });

      //  Empty data transaction must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${EDGE_002.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(EDGE_003),
    async () => {
      const user = getEntryUser();

      logger.info(`${EDGE_003.id}: Testing minimum gas limit`, {
        user: user.address,
      });

      // Send with standard minimum gas for simple transfer (21000)
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: "0x",
        gasLimit: 21000,
      });

      //  Minimum gas transaction must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${EDGE_003.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(EDGE_004),
    async () => {
      logger.info(`${EDGE_004.id}: Testing rapid user creation`);

      const userCount = 3;
      const users: ethers.HDNodeWallet[] = [];

      // Create users rapidly
      for (let i = 0; i < userCount; i++) {
        const user = getFundedUser();
        users.push(user);
      }

      // Register all users (rapid karma minting)
      const mintPromises = users.map((user) => karmaManager.mintKarma(user.address, 1n));
      await Promise.all(mintPromises);

      // Wait for all registrations
      for (const user of users) {
        await karmaManager.waitForRlnRegistration(user.address);
      }

      //  All users must be registered
      for (const user of users) {
        const isRegistered = await karmaManager.isUserRegistered(user.address);
        expect(isRegistered).toBe(true);
      }

      logger.info(`${EDGE_004.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(EDGE_005),
    async () => {
      const user = getEntryUser();

      logger.info(`${EDGE_005.id}: Testing transaction to contract`, {
        user: user.address,
      });

      // Send gasless transaction to contract address with valid function call
      // This tests that gasless transactions to contract addresses are allowed
      // Note: We use balanceOf(address) call since Karma contract has no receive() function
      // The balanceOf function is a view function that won't change state, but proves
      // the RLN system doesn't block transactions to contract addresses
      const karmaAddress = await contracts.karma.getAddress();

      // Encode a balanceOf call - this is a valid function call that will succeed
      const balanceOfData = contracts.karma.interface.encodeFunctionData("balanceOf", [user.address]);

      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: karmaAddress,
        value: 0n,
        data: balanceOfData, // Valid function call to contract
        gasLimit: 50000, // Increased gas limit for contract call
      });

      //  Transaction to contract must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${EDGE_005.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );
});
