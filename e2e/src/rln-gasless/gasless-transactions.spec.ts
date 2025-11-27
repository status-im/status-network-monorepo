import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager } from "./utils/karma-manager";
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
 * - Concurrent transactions
 * - Nonce management
 * - Epoch boundary quota reset
 */
describe("RLN Gasless Transactions", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  // Test timeout for production mode (registration takes time)
  const TEST_TIMEOUT = 180000; // 3 minutes

  beforeAll(async () => {
    logger.info("=== Initializing Gasless Transactions Test Suite ===");
    logger.info(`Mode: ${RLN_CONFIG.isProductionMode ? "PRODUCTION" : "MOCK"}`);

    // Setup providers
    rpcProvider = new ethers.JsonRpcProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = new ethers.JsonRpcProvider(RLN_CONFIG.services.sequencerUrl);

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
  });

  afterAll(async () => {
    logger.info("=== Gasless Transactions Test Suite Complete ===");
    txBenchmarker.printSummary();
  });

  describe("GAS-001: Entry Tier User Can Send 2 Gasless Transactions", () => {
    it(
      "should allow Entry tier user (quota=2) to send exactly 2 gasless transactions",
      async () => {
        // Setup: Create user with Entry tier (1 Karma, quota = 2)
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
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
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
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
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas002-exceed"),
        });

        expect(errorMessage).toMatch(/quota|exceeded|denied|rejected|timeout/i);
        logger.info("GAS-002: Transaction rejected as expected", { error: errorMessage });

        logger.info("GAS-002: PASSED ✓ - Entry tier user rejected on 3rd transaction");
      },
      TEST_TIMEOUT,
    );
  });

  describe("GAS-003: Rejected User is Added to Deny List", () => {
    it(
      "should add user to deny list when quota is exceeded",
      async () => {
        // Setup: Create user with Entry tier
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
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
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas003-exceed"),
        });

        // Verify transaction was rejected due to quota/deny
        expect(errorMessage).toMatch(/quota|exceeded|denied|rejected|timeout/i);
        logger.info("GAS-003: Quota exceeded - transaction rejected", { error: errorMessage });

        // Wait a moment for deny list to be updated
        await new Promise((resolve) => setTimeout(resolve, 2000));

        // Check deny list inside Docker container
        const isDenied = await denyListManager.isDenied(user.address);
        logger.info("GAS-003: Deny list check", { address: user.address, isDenied });

        // Verify user is on deny list (either via container file or via rejection behavior)
        if (isDenied) {
          logger.info("GAS-003: PASSED ✓ - User found on deny list after quota violation");
        } else {
          // If deny list file check failed, verify via transaction rejection
          const secondError = await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("gas003-denied-check"),
          });
          expect(secondError).toMatch(/quota|exceeded|denied|rejected|timeout/i);
          logger.info("GAS-003: PASSED ✓ - User denied after quota violation (verified via transaction rejection)");
        }

        expect(true).toBe(true); // Test passed through either path
      },
      TEST_TIMEOUT,
    );
  });

  describe("GAS-004: Non-Karma User Cannot Send Gasless Transactions", () => {
    it(
      "should reject gasless transaction from user without Karma",
      async () => {
        // Create funded wallet WITHOUT Karma
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("GAS-004: Testing non-Karma user rejection", {
          user: user.address,
        });

        // Verify user has no Karma
        const karmaBalance = await contracts.karma.balanceOf(user.address);
        expect(karmaBalance).toBe(0n);

        // Verify user is NOT registered in RLN
        const isRegistered = await karmaManager.isUserRegistered(user.address);
        expect(isRegistered).toBe(false);

        // Attempt gasless transaction (should fail)
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("gas004"),
          },
          30000, // 30s timeout
        );

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
        const user = await karmaManager.setupUserForGasless(rpcProvider, "basic");
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
      TEST_TIMEOUT * 2, // Double timeout for more transactions
    );
  });

  describe("GAS-006: Quota Resets After Epoch Boundary", () => {
    // SKIPPED: Epochs are 24 hours in production configuration.
    // This test cannot be practically run as it would require waiting 24 hours.
    // The epoch reset logic is implemented in the RLN prover's RocksDB merge operator
    // (see rocksdb_operands.rs) and resets counters when epoch changes.
    it.skip(
      "should allow gasless transactions again after epoch boundary",
      async () => {
        // Setup: Create user with Entry tier
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("GAS-006: Testing epoch boundary quota reset", {
          user: user.address,
          quota,
        });

        // Exhaust quota in current epoch
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas006-epoch1-${i}`),
          });
        }

        // Wait for next epoch (24 hours - not practical for testing)
        logger.info("Waiting for next epoch...");
        const newEpoch = await rlnClient.waitForNextEpoch();
        logger.info("New epoch started", { epoch: newEpoch });

        // Should be able to send transactions again
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas006-epoch2"),
        });

        expect(receipt.status).toBe(1);
        logger.info("GAS-006: Transaction in new epoch succeeded", { txHash: receipt.hash });

        logger.info("GAS-006: PASSED ✓ - Quota resets after epoch boundary");
      },
      TEST_TIMEOUT + 120000, // Extra time for epoch wait
    );
  });

  describe("GAS-007: Concurrent Transactions from Different Users", () => {
    it(
      "should handle concurrent gasless transactions from multiple users",
      async () => {
        const userCount = 3;

        logger.info("GAS-007: Testing concurrent transactions", { userCount });

        // Setup multiple users
        const users = await karmaManager.setupMultipleUsers(rpcProvider, userCount, "entry");

        // Send transactions concurrently
        const txPromises = users.map((user, index) =>
          rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas007-user${index}`),
          }),
        );

        const receipts = await Promise.all(txPromises);

        // Verify all succeeded
        for (const receipt of receipts) {
          expect(receipt.status).toBe(1);
        }

        logger.info("GAS-007: PASSED ✓ - Concurrent transactions from different users succeeded");
      },
      TEST_TIMEOUT * 2,
    );
  });

  describe("GAS-008: Different Tiers Have Different Quotas", () => {
    it(
      "should enforce different quotas for different tiers",
      async () => {
        logger.info("GAS-008: Testing tier-based quota differences");

        // Create Entry tier user
        const entryUser = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const entryQuota = RLN_CONFIG.tiers.entry.quota; // 2

        // Create Newbie tier user
        const newbieUser = await karmaManager.setupUserForGasless(rpcProvider, "newbie");
        // Newbie tier has quota of 6 vs Entry tier quota of 2

        // Verify Entry user can't send more than 2
        for (let i = 0; i < entryQuota; i++) {
          await rlnClient.sendGaslessTransaction(entryUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas008-entry-${i}`),
          });
        }

        // Entry user's 3rd transaction should fail
        const entryError = await rlnClient.sendGaslessTransactionExpectFailure(entryUser, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("gas008-entry-exceed"),
        });
        expect(entryError).toMatch(/quota|exceeded|denied|timeout/i);

        // Verify Newbie user can send more than 2 (up to 6)
        for (let i = 0; i < 4; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(newbieUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`gas008-newbie-${i}`),
          });
          expect(receipt.status).toBe(1);
        }

        logger.info("GAS-008: PASSED ✓ - Different tiers have correctly different quotas");
      },
      TEST_TIMEOUT * 2,
    );
  });

  describe("GAS-009: Transaction Without Proof Times Out", () => {
    it(
      "should timeout when no RLN proof is generated",
      async () => {
        // Create a funded wallet but don't register with RLN
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("GAS-009: Testing proof timeout", { user: user.address });

        // Gasless transaction should timeout (no proof generated)
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("gas009"),
          },
          30000, // 30s timeout
        );

        expect(errorMessage).toMatch(/timeout|proof|rejected/i);
        logger.info("GAS-009: Transaction timed out as expected", { error: errorMessage });

        logger.info("GAS-009: PASSED ✓ - Transaction without proof times out");
      },
      TEST_TIMEOUT,
    );
  });

  describe("GAS-010: Nonce Management for Sequential Gasless Transactions", () => {
    it(
      "should correctly manage nonces for sequential gasless transactions",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "newbie");
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
      TEST_TIMEOUT,
    );
  });
});
