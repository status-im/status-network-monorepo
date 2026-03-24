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

// Karma is an ERC20 with 18 decimals
const ETHER = 10n ** 18n;

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

  beforeAll(async () => {
    logger.info("=== Initializing Integration and Error Handling Test Suite ===");

    // Reset nonce manager to sync with blockchain state
    resetAdminNonceManager();

    // Setup providers with fast polling for quicker transaction confirmation detection
    rpcProvider = createFastProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = createFastProvider(RLN_CONFIG.services.sequencerUrl);
    admin = new ethers.Wallet(RLN_CONFIG.accounts.admin, rpcProvider);
    contracts = loadRlnContracts(rpcProvider, admin);

    rlnClient = new RlnTestClient(rpcProvider, sequencerProvider, RLN_CONFIG.services.rpcUrl);

    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);

    // PRE-REGISTER ALL USERS NEEDED FOR THIS TEST SUITE
    // Uses skipRegistrationWait to avoid 20s sleep per user, then does a single wait at the end
    logger.info("Pre-registering test users...");

    // Entry users (10 needed)
    for (let i = 0; i < 10; i++) {
      entryUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "entry", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered entry user ${i + 1}/10`);
    }
    // Newbie users (5 needed for INT_005)
    for (let i = 0; i < 5; i++) {
      newbieUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "newbie", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered newbie user ${i + 1}/5`);
    }
    // Basic users (2 needed)
    for (let i = 0; i < 2; i++) {
      basicUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "basic", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered basic user ${i + 1}/2`);
    }
    // Active users (3 needed for high-volume tests)
    for (let i = 0; i < 3; i++) {
      activeUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "active", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered active user ${i + 1}/3`);
    }
    // Funded-only users (5 needed)
    for (let i = 0; i < 5; i++) {
      fundedUsers.push(await createFundedWallet(rpcProvider, admin));
      logger.debug(`Pre-funded user ${i + 1}/5`);
    }

    // Single registration wait for all users (prover processes karma events as they arrive,
    // so by the time we get here, most/all users are already registered)
    logger.info("Waiting for RLN registrations to complete...");
    await karmaManager.waitForRlnRegistration("batch-all");
    logger.info("Registration wait complete");

    logger.info("Test suite initialized", {
      entryUsers: entryUsers.length,
      newbieUsers: newbieUsers.length,
      basicUsers: basicUsers.length,
      activeUsers: activeUsers.length,
      fundedUsers: fundedUsers.length,
    });
  }, RLN_CONFIG.test.timeouts.setupLarge);

  afterAll(async () => {
    logger.info("=== Integration and Error Handling Test Suite Complete ===");
  });

  // ============================================================================
  // INTEGRATION TESTS (INT_001 to INT_006)
  // ============================================================================

  it(
    formatScenario(INT_001),
    async () => {
      logger.info(`${INT_001.id}: Starting complete lifecycle test`);

      // Step 1: Create new user
      const user = getFundedUser();
      logger.info("Step 1: User created", { address: user.address });

      // Step 2: Mint 1 Karma and register (Entry tier) - Karma has 18 decimals
      await karmaManager.mintKarma(user.address, 1n * ETHER);
      await karmaManager.waitForRlnRegistration(user.address);
      logger.info("Step 2: User registered with Entry tier");

      // Step 3: Send gasless transactions (exhaust quota)
      // Ensure enough epoch time for quota TXs + expected failure (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);
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

      // Step 4: Verify quota is exhausted (prover rejects proof request)
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int001-exceed"),
      });
      expect(errorMessage).toMatch(/deny|denied|reject|quota|timeout|resource.*exhausted/i);
      logger.info("Step 4: Gasless blocked after quota exhaustion");

      // Step 5: Pay premium gas (removes from deny list AND resets quota)
      const premiumReceipt = await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("int001-premium"),
      });
      expect(premiumReceipt.status).toBe(1);
      logger.info("Step 5: Premium gas paid, quota reset");

      // Step 6: Send gasless again (quota was reset by premium payment)
      const finalReceipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("int001-final"),
      });
      expect(finalReceipt.status).toBe(1);
      logger.info("Step 6: Gasless working again after premium gas");

      logger.info(`${INT_001.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // Registration + gasless + deny + premium + TTL recovery + gasless
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

      // Send transactions from each user based on their tier quota
      // Entry: 1 tx (quota = 2, using subset)
      const entryReceipt = await rlnClient.sendGaslessTransaction(entryUser, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData(`int002-entry-0`),
      });
      expect(entryReceipt.status).toBe(1);

      // Newbie: 3 tx (quota = 6, using subset)
      for (let i = 0; i < 3; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(newbieUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`int002-newbie-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      // Basic: 4 tx (quota = 16, using subset)
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
    RLN_CONFIG.test.timeouts.highVolume, // 1 + 3 + 4 = 8 TXs
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
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 5 TXs
  );

  it(
    formatScenario(INT_004),
    async () => {
      // Use Newbie tier (quota=6) to avoid deny list issues
      // Entry tier (quota=2) would exhaust quota and hit 60s deny list TTL
      const user = getNewbieUser();
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
    RLN_CONFIG.test.timeouts.epoch, // This test waits for epoch boundary (30s + buffer)
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
    RLN_CONFIG.test.timeouts.highVolume, // 5 users × 3 TXs = 15 TXs = ~60s
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
    RLN_CONFIG.test.timeouts.highVolume, // 10 TXs = ~40s
  );

  // ============================================================================
  // ERROR HANDLING TESTS (ERR_001 to ERR_003)
  // ============================================================================

  it(
    formatScenario(ERR_001),
    async () => {
      logger.info(`${ERR_001.id}: Testing deny list manager with unavailable prover`);

      const user = getEntryUser();

      // DenyListTestManager with invalid prover URL should handle errors gracefully
      // (isDenied returns false on connection error, does not throw)
      const badDenyListManager = new DenyListTestManager(
        "http://localhost:99999", // Invalid prover URL
      );
      const isDenied = await badDenyListManager.isDeniedViaProver(user.address);
      expect(isDenied).toBe(false);
      logger.info(`${ERR_001.id}: Deny list manager handled unavailable prover gracefully`);

      // Verify the user can still send gasless transactions via the working client
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("err001-verify"),
      });
      expect(receipt.status).toBe(1);

      logger.info(`${ERR_001.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
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
    RLN_CONFIG.test.timeouts.singleTx,
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
    RLN_CONFIG.test.timeouts.singleTx,
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
    RLN_CONFIG.test.timeouts.singleTx,
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
    RLN_CONFIG.test.timeouts.singleTx,
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
    RLN_CONFIG.test.timeouts.singleTx,
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
      const mintPromises = users.map((user) => karmaManager.mintKarma(user.address, 1n * ETHER));
      await Promise.all(mintPromises);

      // Wait for all registrations in parallel
      await Promise.all(users.map((user) => karmaManager.waitForRlnRegistration(user.address)));

      //  All users must be registered
      for (const user of users) {
        const isRegistered = await karmaManager.isUserRegistered(user.address);
        expect(isRegistered).toBe(true);
      }

      logger.info(`${EDGE_004.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx,
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
        gasLimit: 100000, // Gas limit for contract call (proxies need more gas)
      });

      //  Transaction to contract must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${EDGE_005.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );
});
