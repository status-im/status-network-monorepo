import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager, resetAdminNonceManager } from "./utils/karma-manager";
import { createFundedWallet, uniqueTxData, TEST_RECIPIENT } from "./utils/test-helpers";
import { RLN_CONFIG } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";
import { txBenchmarker } from "./utils/tx-benchmarker";

const logger = createTestLogger();

/**
 * Test Suite: Gasless Transactions (GAS-001 to GAS-010)
 *
 * Tests the core gasless transaction functionality including:
 * - Basic gasless transaction flow with different tiers
 * - Quota enforcement and exhaustion
 * - Non-Karma users rejection
 * - Concurrent transactions with quota isolation
 * - Nonce management
 * - Epoch boundary quota reset (using short epochs)
 *
 */
describe("RLN Gasless Transactions", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  // Pre-registered user pools - created once in beforeAll
  const entryUsers: ethers.HDNodeWallet[] = [];
  const basicUsers: ethers.HDNodeWallet[] = [];
  const newbieUsers: ethers.HDNodeWallet[] = [];
  const fundedOnlyUsers: ethers.HDNodeWallet[] = [];
  let entryIdx = 0,
    basicIdx = 0,
    newbieIdx = 0,
    fundedIdx = 0;

  const getEntryUser = () =>
    entryUsers[entryIdx++] ||
    (() => {
      throw new Error("Not enough entry users");
    })();
  const getBasicUser = () =>
    basicUsers[basicIdx++] ||
    (() => {
      throw new Error("Not enough basic users");
    })();
  const getNewbieUser = () =>
    newbieUsers[newbieIdx++] ||
    (() => {
      throw new Error("Not enough newbie users");
    })();
  const getFundedUser = () =>
    fundedOnlyUsers[fundedIdx++] ||
    (() => {
      throw new Error("Not enough funded users");
    })();

  // Timeouts based on actual TX performance (~4-5s per gasless TX, P95: 4.7s)
  // Single TX tests: 20s (5s TX + 15s buffer for proof generation/network)
  const TEST_TIMEOUT = 20000;
  // Multi-TX tests: need ~5s per TX + buffer
  const MULTI_TX_TIMEOUT = 60000; // 5-6 TXs
  const HIGH_VOLUME_TIMEOUT = 120000; // 16+ TXs
  // Extended timeout for epoch boundary tests (60s epoch + buffer)
  const EPOCH_TEST_TIMEOUT = 180000;

  beforeAll(async () => {
    logger.info("=== Initializing Gasless Transactions Test Suite ===");
    logger.info(`Mode: ${RLN_CONFIG.isProductionMode ? "PRODUCTION" : "MOCK"}`);
    logger.info(`Epoch Duration: ${RLN_CONFIG.test.epochDurationSeconds}s`);

    // Reset nonce manager to sync with blockchain state
    resetAdminNonceManager();

    // Setup providers with fast polling for quicker transaction confirmation detection
    rpcProvider = createFastProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = createFastProvider(RLN_CONFIG.services.sequencerUrl);

    // Setup admin wallet
    admin = new ethers.Wallet(RLN_CONFIG.accounts.admin, rpcProvider);

    // Load contracts
    contracts = loadRlnContracts(rpcProvider, admin);

    // Setup test clients
    rlnClient = new RlnTestClient(
      rpcProvider,
      sequencerProvider,
      RLN_CONFIG.services.rpcUrl,
      RLN_CONFIG.services.karmaServiceUrl,
    );

    denyListManager = new DenyListTestManager();
    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);

    // Verify contracts are accessible
    const karmaAddress = await contracts.karma.getAddress();
    const rlnAddress = await contracts.rln.getAddress();
    logger.info("Contracts loaded", { karma: karmaAddress, rln: rlnAddress });

    // PRE-REGISTER ALL USERS NEEDED FOR THIS TEST SUITE
    logger.info("Pre-registering test users...");

    // Entry tier users (8 needed)
    for (let i = 0; i < 8; i++) {
      entryUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "entry"));
      logger.debug(`Pre-registered entry user ${i + 1}/8`);
    }
    // Basic tier user (1 needed)
    basicUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "basic"));
    logger.debug("Pre-registered basic user");

    // Newbie tier users (2 needed)
    for (let i = 0; i < 2; i++) {
      newbieUsers.push(await karmaManager.setupUserForGasless(rpcProvider, "newbie"));
      logger.debug(`Pre-registered newbie user ${i + 1}/2`);
    }
    // Funded-only users (2 needed)
    for (let i = 0; i < 2; i++) {
      fundedOnlyUsers.push(await createFundedWallet(rpcProvider, admin));
      logger.debug(`Pre-funded user ${i + 1}/2`);
    }

    logger.info("Test suite initialized", {
      entryUsers: entryUsers.length,
      basicUsers: basicUsers.length,
      newbieUsers: newbieUsers.length,
      fundedOnlyUsers: fundedOnlyUsers.length,
    });
  }, 180000); // 3 minute timeout for setup

  afterAll(async () => {
    logger.info("=== Gasless Transactions Test Suite Complete ===");
    txBenchmarker.printSummary();
  });

  describe("GAS-001: Entry Tier User Can Send 2 Gasless Transactions", () => {
    it(
      "should allow Entry tier user (quota=2) to send exactly 2 gasless transactions",
      async () => {
        // Setup: Create user with Entry tier (1 Karma, quota = 2)
        const user = getEntryUser();
        const quota = RLN_CONFIG.tiers.entry.quota; // 2

        logger.info("GAS-001: Testing Entry tier quota", {
          user: user.address,
          quota,
        });

        // Send exactly 'quota' number of transactions
        for (let i = 0; i < quota; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas001-${i}`),
          });

          expect(receipt.status).toBe(1);
          logger.info(`Transaction ${i + 1}/${quota} succeeded`, { txHash: receipt.hash });
        }

        // Verify user is NOT on deny list (quota not exceeded yet)
        const isDenied = await denyListManager.isDenied(user.address);
        expect(isDenied).toBe(false);

        logger.info("GAS-001: PASSED ✓ - Entry tier user sent 2 gasless transactions");
      },
      TEST_TIMEOUT,
    );
  });

  describe("GAS-002: Entry Tier User Gets Rejected on 3rd Transaction", () => {
    it(
      "should reject Entry tier user on transaction exceeding quota",
      async () => {
        // Setup: Create user with Entry tier and exhaust quota
        const user = getEntryUser();
        const quota = RLN_CONFIG.tiers.entry.quota; // 2

        logger.info("GAS-002: Testing quota exhaustion rejection", {
          user: user.address,
          quota,
        });

        // Exhaust the quota first
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas002-exhaust-${i}`),
          });
        }

        // Attempt one more transaction (should fail)
        // When quota is exceeded, prover doesn't generate proof → TX times out
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas002-exceed"),
        });

        // Quota exceeded manifests as timeout (no proof generated) or explicit rejection
        expect(errorMessage).toMatch(/quota|exceeded|denied|timeout/i);
        logger.info("GAS-002: Transaction rejected as expected", { error: errorMessage });

        logger.info("GAS-002: PASSED ✓ - Entry tier user rejected on 3rd transaction");
      },
      MULTI_TX_TIMEOUT, // 2 TXs + rejection attempt = ~12s
    );
  });

  describe("GAS-003: Rejected User is Added to Deny List", () => {
    it(
      "should add user to deny list when quota is exceeded",
      async () => {
        // Setup: Create user with Entry tier
        const user = getEntryUser();
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("GAS-003: Testing deny list addition on quota violation", {
          user: user.address,
          quota,
        });

        // Exhaust quota
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas003-exhaust-${i}`),
          });
        }

        // Attempt to exceed quota (triggers deny list addition)
        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas003-exceed"),
        });

        // Wait for deny list update and verify
        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);
        const isDenied = await denyListManager.isDenied(user.address);

        //  User MUST be on deny list - no fallback paths
        expect(isDenied).toBe(true);

        logger.info("GAS-003: PASSED ✓ - User found on deny list after quota violation");
      },
      MULTI_TX_TIMEOUT, // 2 TXs + rejection attempt + deny list check = ~15s
    );
  });

  describe("GAS-004: Non-Karma User Cannot Send Gasless Transactions", () => {
    it(
      "should reject gasless transaction from user without Karma",
      async () => {
        // Create funded wallet WITHOUT Karma
        const user = getFundedUser();

        logger.info("GAS-004: Testing non-Karma user rejection", {
          user: user.address,
        });

        // Verify user has no Karma
        const karmaBalance = await contracts.karma.balanceOf(user.address);
        expect(karmaBalance).toBe(0n);

        // Verify user is NOT registered in RLN
        const isRegistered = await karmaManager.isUserRegistered(user.address);
        expect(isRegistered).toBe(false);

        // Attempt gasless transaction (should fail fast - no proof generated)
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("gas004"),
          },
          RLN_CONFIG.test.proofTimeoutMs,
        );

        //  Must fail for proof/registration reason
        expect(errorMessage).toMatch(/timeout|rejected|not registered|invalid|proof/i);
        logger.info("GAS-004: Transaction rejected as expected", { error: errorMessage });

        logger.info("GAS-004: PASSED ✓ - Non-Karma user cannot send gasless transactions");
      },
      TEST_TIMEOUT,
    );
  });

  describe("GAS-005: Basic Tier User Can Send 16 Transactions", () => {
    it(
      "should allow Basic tier user (quota=16) to send 16 gasless transactions",
      async () => {
        // Setup: Create user with Basic tier (50 Karma, quota = 16)
        const user = getBasicUser();
        const quota = RLN_CONFIG.tiers.basic.quota; // 16

        logger.info("GAS-005: Testing Basic tier quota", {
          user: user.address,
          quota,
        });

        // Send all quota transactions
        for (let i = 0; i < quota; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas005-${i}`),
          });

          expect(receipt.status).toBe(1);

          if ((i + 1) % 4 === 0) {
            logger.info(`Progress: ${i + 1}/${quota} transactions`);
          }
        }

        logger.info("GAS-005: PASSED ✓ - Basic tier user sent 16 gasless transactions");
      },
      HIGH_VOLUME_TIMEOUT, // 16 TXs at ~4s each = ~64s
    );
  });

  describe("GAS-006: Quota Resets After Epoch Boundary", () => {
    // NOW RUNNABLE: Using short epochs (10s default)
    it(
      "should allow gasless transactions again after epoch boundary",
      async () => {
        // Setup: Create user with Entry tier
        const user = getEntryUser();
        const quota = RLN_CONFIG.tiers.entry.quota;
        const epochDuration = RLN_CONFIG.test.epochDurationSeconds;

        logger.info("GAS-006: Testing epoch boundary quota reset", {
          user: user.address,
          quota,
          epochDurationSeconds: epochDuration,
        });

        // Exhaust quota in current epoch
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas006-epoch1-${i}`),
          });
        }

        // Verify quota is exhausted (next tx should fail)
        // Quota exceeded manifests as timeout (no proof generated) or explicit rejection
        const preEpochError = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("gas006-verify-exhausted"),
          },
          10000, // 10s timeout for failure expectation
        );
        expect(preEpochError).toMatch(/quota|exceeded|denied|timeout/i);

        // Wait for next epoch (short epochs make this practical)
        logger.info(`Waiting for next epoch (max ${epochDuration + 2}s)...`);
        const newEpoch = await rlnClient.waitForNextEpoch((epochDuration + 2) * 1000);
        logger.info("New epoch started", { epoch: newEpoch });

        // User needs to be removed from deny list to use gasless again
        // Pay premium gas to clear deny status
        await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: ethers.parseUnits("15", "gwei"),
          data: uniqueTxData("gas006-clear-deny"),
        });

        // Wait for deny list removal
        await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Allow prover to sync state after deny list clearance
        await rlnClient.sleep(1000);

        // Should be able to send transactions again in new epoch
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas006-epoch2"),
        });

        expect(receipt.status).toBe(1);
        logger.info("GAS-006: Transaction in new epoch succeeded", { txHash: receipt.hash });

        logger.info("GAS-006: PASSED ✓ - Quota resets after epoch boundary");
      },
      EPOCH_TEST_TIMEOUT, // This test waits for epoch boundary (60s)
    );
  });

  describe("GAS-007: Concurrent Transactions - User Quota Isolation", () => {
    it(
      "should isolate quotas between concurrent users",
      async () => {
        const userCount = 3;

        logger.info("GAS-007: Testing concurrent transactions with quota isolation", { userCount });

        // Get pre-registered users with Entry tier (quota = 2 each)
        const users = [getEntryUser(), getEntryUser(), getEntryUser()];

        // Send 2 transactions from each user using concurrent helper (handles nonces)
        const txPromises = users.map((user, index) =>
          rlnClient.sendGaslessTransactionsConcurrent(user, [
            { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData(`gas007-user${index}-tx1`) },
            { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData(`gas007-user${index}-tx2`) },
          ]),
        );

        const resultsNested = await Promise.all(txPromises);
        const allReceipts = resultsNested.flat();

        // Verify all 6 transactions (2 per user × 3 users) succeeded
        expect(allReceipts.length).toBe(6);
        for (const receipt of allReceipts) {
          expect(receipt.status).toBe(1);
        }

        // Now all users should have exhausted their quota (2/2 used)
        // Verify each user's 3rd tx fails (isolation verified)
        for (let i = 0; i < users.length; i++) {
          const error = await rlnClient.sendGaslessTransactionExpectFailure(
            users[i],
            {
              to: TEST_RECIPIENT,
              value: 0n,
              data: uniqueTxData(`gas007-user${i}-tx3`),
            },
            10000, // 10s timeout for failure expectation
          );
          // Quota exceeded manifests as timeout (no proof generated) or explicit rejection
          expect(error).toMatch(/quota|exceeded|denied|timeout/i);
        }

        logger.info("GAS-007: PASSED ✓ - User quotas are properly isolated");
      },
      MULTI_TX_TIMEOUT, // 3 users × 3 TXs = 9 TXs
    );
  });

  describe("GAS-008: Different Tiers Have Different Quotas", () => {
    it(
      "should enforce different quotas for different tiers",
      async () => {
        logger.info("GAS-008: Testing tier-based quota differences");

        // Get pre-registered Entry tier user
        const entryUser = getEntryUser();
        const entryQuota = RLN_CONFIG.tiers.entry.quota; // 2

        // Get pre-registered Newbie tier user
        const newbieUser = getNewbieUser();
        const newbieQuota = RLN_CONFIG.tiers.newbie.quota; // 6

        // Verify Entry user can't send more than 2
        for (let i = 0; i < entryQuota; i++) {
          await rlnClient.sendGaslessTransaction(entryUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas008-entry-${i}`),
          });
        }

        // Entry user's 3rd transaction should fail
        // Quota exceeded manifests as timeout (no proof generated) or explicit rejection
        const entryError = await rlnClient.sendGaslessTransactionExpectFailure(entryUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas008-entry-exceed"),
        });
        expect(entryError).toMatch(/quota|exceeded|denied|timeout/i);

        // Verify Newbie user can send more than 2 (up to 6)
        // Send 4 to prove they have higher quota than Entry
        for (let i = 0; i < 4; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(newbieUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas008-newbie-${i}`),
          });
          expect(receipt.status).toBe(1);
        }

        logger.info("GAS-008: PASSED ✓ - Different tiers have correctly different quotas", {
          entryQuota,
          newbieQuota,
        });
      },
      MULTI_TX_TIMEOUT, // 2 + 1 + 4 = 7 TXs = ~28s
    );
  });

  describe("GAS-009: Transaction Without Proof Times Out Fast", () => {
    it(
      "should timeout quickly when no RLN proof is generated",
      async () => {
        // Create a funded wallet but don't register with RLN
        const user = getFundedUser();
        const startTime = Date.now();

        logger.info("GAS-009: Testing proof timeout", { user: user.address });

        // Gasless transaction should timeout fast (no proof generated)
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("gas009"),
          },
          RLN_CONFIG.test.proofTimeoutMs,
        );

        const elapsed = Date.now() - startTime;

        expect(errorMessage).toMatch(/timeout|proof|rejected/i);
        //  Should timeout within proof timeout + buffer
        expect(elapsed).toBeLessThan(RLN_CONFIG.test.proofTimeoutMs + 2000);

        logger.info("GAS-009: Transaction timed out as expected", {
          error: errorMessage,
          elapsedMs: elapsed,
        });

        logger.info("GAS-009: PASSED ✓ - Transaction without proof times out quickly");
      },
      TEST_TIMEOUT,
    );
  });

  describe("GAS-010: Nonce Management for Sequential Gasless Transactions", () => {
    it(
      "should correctly manage nonces for sequential gasless transactions",
      async () => {
        const user = getNewbieUser();
        const txCount = 3;

        logger.info("GAS-010: Testing nonce management", {
          user: user.address,
          txCount,
        });

        const initialNonce = await rpcProvider.getTransactionCount(user.address, "latest");

        // Send sequential transactions
        for (let i = 0; i < txCount; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas010-${i}`),
          });

          expect(receipt.status).toBe(1);

          // Verify nonce incremented correctly
          const currentNonce = await rpcProvider.getTransactionCount(user.address, "latest");
          expect(currentNonce).toBe(initialNonce + i + 1);

          logger.debug(`Transaction ${i + 1} nonce`, {
            expected: initialNonce + i,
            currentNonce,
          });
        }

        logger.info("GAS-010: PASSED ✓ - Nonces managed correctly for sequential transactions");
      },
      MULTI_TX_TIMEOUT, // 3 TXs = ~12s
    );
  });
});
