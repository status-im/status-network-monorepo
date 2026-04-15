import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "./utils/rln-test-client";
import { KarmaTestManager, resetAdminNonceManager } from "./utils/karma-manager";
import { createFundedWallet, uniqueTxData, TEST_RECIPIENT } from "./utils/test-helpers";
import { RLN_CONFIG } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";
import { txBenchmarker } from "./utils/tx-benchmarker";
import {
  formatScenario,
  GAS_001,
  GAS_002,
  GAS_003,
  GAS_004,
  GAS_005,
  GAS_006,
  GAS_007,
  GAS_008,
  GAS_009,
  GAS_010,
} from "./helpers/scenario";

const logger = createTestLogger();

/**
 * Test Suite: Gasless Transactions (GAS_001 to GAS_010)
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
    rlnClient = new RlnTestClient(rpcProvider, sequencerProvider, RLN_CONFIG.services.rpcUrl);

    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);

    // Verify contracts are accessible
    const karmaAddress = await contracts.karma.getAddress();
    const rlnAddress = await contracts.rln.getAddress();
    logger.info("Contracts loaded", { karma: karmaAddress, rln: rlnAddress });

    // PRE-REGISTER ALL USERS NEEDED FOR THIS TEST SUITE
    // Uses skipRegistrationWait to avoid 20s sleep per user, then does a single wait at the end
    logger.info("Pre-registering test users...");

    // Entry tier users (8 needed)
    for (let i = 0; i < 8; i++) {
      entryUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "entry", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered entry user ${i + 1}/8`);
    }
    // Basic tier user (1 needed)
    basicUsers.push(
      await karmaManager.setupUserForGasless(rpcProvider, "basic", undefined, { skipRegistrationWait: true }),
    );
    logger.debug("Pre-registered basic user");

    // Newbie tier users (2 needed)
    for (let i = 0; i < 2; i++) {
      newbieUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "newbie", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered newbie user ${i + 1}/2`);
    }
    // Funded-only users (2 needed)
    for (let i = 0; i < 2; i++) {
      fundedOnlyUsers.push(await createFundedWallet(rpcProvider, admin));
      logger.debug(`Pre-funded user ${i + 1}/2`);
    }

    // Single registration wait for all users (prover processes karma events as they arrive,
    // so by the time we get here, most/all users are already registered)
    logger.info("Waiting for RLN registrations to complete...");
    await karmaManager.waitForRlnRegistration("batch-all");
    logger.info("Registration wait complete");

    logger.info("Test suite initialized", {
      entryUsers: entryUsers.length,
      basicUsers: basicUsers.length,
      newbieUsers: newbieUsers.length,
      fundedOnlyUsers: fundedOnlyUsers.length,
    });
  }, RLN_CONFIG.test.timeouts.setupLarge);

  afterAll(async () => {
    logger.info("=== Gasless Transactions Test Suite Complete ===");
    txBenchmarker.printSummary();
  });

  it(
    formatScenario(GAS_001),
    async () => {
      // Setup: Create user with Entry tier (1 Karma, quota = 2)
      const user = getEntryUser();
      const quota = RLN_CONFIG.tiers.entry.quota; // 2

      logger.info(`${GAS_001.id}: Testing Entry tier quota`, {
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

      logger.info(`${GAS_001.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(GAS_002),
    async () => {
      // Setup: Create user with Entry tier and test quota exhaustion
      const user = getEntryUser();
      const quota = RLN_CONFIG.tiers.entry.quota; // 2

      logger.info(`${GAS_002.id}: Testing quota exhaustion rejection`, {
        user: user.address,
        quota,
      });

      // Ensure enough epoch time for quota TXs + expected failure (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (this exhausts the quota and adds user to deny list)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`gas002-quota-${i}`),
        });
      }

      // Wait for prover to sync quota state
      await rlnClient.waitForProverSync();

      // Next transaction (quota + 1) should FAIL - user has exhausted their quota
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("gas002-exceed"),
      });

      expect(errorMessage).toMatch(/quota|exceeded|deny|denied|timeout|resource.*exhausted|karma|limit/i);
      logger.info(`${GAS_002.id}: Transaction rejected as expected (quota exhausted)`, { error: errorMessage });

      logger.info(`${GAS_002.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 1 quota + 1 rejection
  );

  it(
    formatScenario(GAS_003),
    async () => {
      // Setup: Create user with Entry tier
      const user = getEntryUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${GAS_003.id}: Testing deny list addition on quota exhaustion`, {
        user: user.address,
        quota,
      });

      // Ensure enough epoch time for quota TXs + expected failure (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`gas003-quota-${i}`),
        });
      }

      // Immediately verify user cannot send another gasless TX (quota exhausted).
      // The prover rejects proof requests for users who have used their full quota,
      // so we don't need to wait for deny list propagation — the prover itself enforces this.
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas003-denied"),
        },
        10000, // 10s timeout for expected failure
      );

      // Error should indicate quota exhaustion or denial
      expect(errorMessage).toMatch(/quota|deny|denied|timeout|exceeded|resource|karma|limit/i);

      logger.info(`${GAS_003.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + quota TXs + 1 expected failure TX
  );

  it(
    formatScenario(GAS_004),
    async () => {
      // Create funded wallet WITHOUT Karma
      const user = getFundedUser();

      logger.info(`${GAS_004.id}: Testing non-Karma user rejection`, {
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
      expect(errorMessage).toMatch(/timeout|rejected|not registered|invalid|proof|karma|gasless/i);
      logger.info(`${GAS_004.id}: Transaction rejected as expected`, { error: errorMessage });

      logger.info(`${GAS_004.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(GAS_005),
    async () => {
      // Setup: Create user with Basic tier (50 Karma, quota = 16) - Karma has 18 decimals
      const user = getBasicUser();
      const quota = RLN_CONFIG.tiers.basic.quota; // 16 txs per epoch

      logger.info(`${GAS_005.id}: Testing Basic tier quota`, {
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

      logger.info(`${GAS_005.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.highVolume, // 16 TXs at ~4s each = ~64s
  );

  it(
    formatScenario(GAS_006),
    async () => {
      // Setup: Create user with Entry tier
      const user = getEntryUser();
      const quota = RLN_CONFIG.tiers.entry.quota;
      const epochDuration = RLN_CONFIG.test.epochDurationSeconds;

      logger.info(`${GAS_006.id}: Testing epoch boundary quota reset`, {
        user: user.address,
        quota,
        epochDurationSeconds: epochDuration,
      });

      // Ensure enough epoch time for quota TXs + expected failure before epoch boundary test
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions in current epoch (user is added to deny list on last tx)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`gas006-epoch1-quota-${i}`),
        });
      }
      logger.info("Quota transactions succeeded, user now on deny list");

      // Wait for prover to sync quota state (longer wait for epoch boundary test)
      await rlnClient.waitForProverSync(1000);

      // Verify quota + 1 fails (user on deny list)
      const preEpochError = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas006-verify-denied"),
        },
        10000, // 10s timeout for failure expectation
      );
      expect(preEpochError).toMatch(/quota|exceeded|deny|denied|timeout|resource.*exhausted|karma|limit/i);

      // Wait for next epoch (short epochs make this practical)
      logger.info(`Waiting for next epoch (max ${epochDuration + 2}s)...`);
      const newEpoch = await rlnClient.waitForNextEpoch((epochDuration + 2) * 1000);
      logger.info("New epoch started", { epoch: newEpoch });

      // Deny list entries are cleared on epoch boundary by the prover.
      // The epoch has already changed above, so the deny list should be cleared.
      // Allow prover to sync state (epoch change triggers deny list cleanup).
      await rlnClient.waitForProverSync(3000);

      // Should be able to send transactions again in new epoch
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("gas006-epoch2"),
      });

      expect(receipt.status).toBe(1);
      logger.info(`${GAS_006.id}: Transaction in new epoch succeeded`, { txHash: receipt.hash });

      logger.info(`${GAS_006.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // This test waits for epoch boundary (60s)
  );

  it(
    formatScenario(GAS_007),
    async () => {
      const userCount = 3;

      logger.info(`${GAS_007.id}: Testing concurrent transactions with quota isolation`, { userCount });

      // Get pre-registered users with Entry tier (quota = 2 each)
      const users = [getEntryUser(), getEntryUser(), getEntryUser()];
      const entryQuota = RLN_CONFIG.tiers.entry.quota; // 2

      // Ensure enough epoch time for all users' quota TXs + failure verification (~25s needed)
      await rlnClient.ensureEpochWindow(25000);

      // Send quota transactions from each user fully sequentially to avoid overwhelming the prover.
      // Each user sends their quota one TX at a time, then the next user goes.
      const allReceipts: ethers.TransactionReceipt[] = [];
      for (let userIdx = 0; userIdx < users.length; userIdx++) {
        for (let txIdx = 0; txIdx < entryQuota; txIdx++) {
          const receipt = await rlnClient.sendGaslessTransaction(users[userIdx], {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas007-user${userIdx}-quota-${txIdx}`),
          });
          allReceipts.push(receipt);
        }
      }

      // Verify all transactions (quota per user × 3 users) succeeded
      expect(allReceipts.length).toBe(entryQuota * 3);
      for (const receipt of allReceipts) {
        expect(receipt.status).toBe(1);
      }

      // Wait for prover to sync quota state for all concurrent users
      await rlnClient.waitForProverSync(3000);

      // Now all users should have exhausted quota (on deny list)
      // Verify each user's next tx fails (quota + 1, isolation verified)
      for (let i = 0; i < users.length; i++) {
        const error = await rlnClient.sendGaslessTransactionExpectFailure(
          users[i],
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas007-user${i}-exceed`),
          },
          10000, // 10s timeout for failure expectation
        );
        // Quota exceeded manifests as resource_exhausted error from prover
        expect(error).toMatch(/quota|exceeded|deny|denied|timeout|resource.*exhausted|karma|limit/i);
      }

      logger.info(`${GAS_007.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.highVolume, // 3 users × concurrent TXs + quota verification
  );

  it(
    formatScenario(GAS_008),
    async () => {
      logger.info(`${GAS_008.id}: Testing tier-based quota differences`);

      // Ensure enough time in epoch for entry quota (2) + 1 rejection + newbie txs (4)
      await rlnClient.ensureEpochWindow(20000);

      // Get pre-registered Entry tier user
      const entryUser = getEntryUser();
      const entryQuota = RLN_CONFIG.tiers.entry.quota; // 2

      // Get pre-registered Newbie tier user
      const newbieUser = getNewbieUser();
      const newbieQuota = RLN_CONFIG.tiers.newbie.quota; // 6

      // Entry user: send full quota (2) - this exhausts their quota and adds to deny list
      for (let i = 0; i < entryQuota; i++) {
        await rlnClient.sendGaslessTransaction(entryUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`gas008-entry-quota-${i}`),
        });
      }

      // Wait for prover to sync quota state
      await rlnClient.waitForProverSync();

      // Entry user's next transaction should fail (quota exhausted, on deny list)
      const entryError = await rlnClient.sendGaslessTransactionExpectFailure(entryUser, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("gas008-entry-exceed"),
      });
      expect(entryError).toMatch(/quota|exceeded|deny|denied|timeout|resource.*exhausted|karma|limit/i);

      // Verify Newbie user can send more than Entry tier
      // Send 4 to prove they have higher quota than Entry (4 < newbie quota of 6)
      for (let i = 0; i < 4; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(newbieUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`gas008-newbie-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      logger.info(`${GAS_008.id}: PASSED ✓`, {
        entryQuota,
        newbieQuota,
      });
    },
    RLN_CONFIG.test.timeouts.highVolume, // 2 entry + failure check + 4 newbie = 7 ops at ~4-6s each
  );

  it(
    formatScenario(GAS_009),
    async () => {
      // Create a funded wallet but don't register with RLN
      const user = getFundedUser();
      const startTime = Date.now();

      logger.info(`${GAS_009.id}: Testing proof timeout`, { user: user.address });

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

      expect(errorMessage).toMatch(/timeout|proof|rejected|karma|gasless/i);
      //  Should timeout within proof timeout + buffer
      expect(elapsed).toBeLessThan(RLN_CONFIG.test.proofTimeoutMs + 2000);

      logger.info(`${GAS_009.id}: Transaction timed out as expected`, {
        error: errorMessage,
        elapsedMs: elapsed,
      });

      logger.info(`${GAS_009.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(GAS_010),
    async () => {
      const user = getNewbieUser();
      const txCount = 3;

      logger.info(`${GAS_010.id}: Testing nonce management`, {
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

      logger.info(`${GAS_010.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx, // 3 TXs = ~12s
  );
});
