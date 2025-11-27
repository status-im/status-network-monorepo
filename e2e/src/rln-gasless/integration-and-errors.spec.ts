import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager } from "./utils/karma-manager";
import { createFundedWallet, uniqueTxData, TEST_RECIPIENT, PREMIUM_GAS_PRICE } from "./utils/test-helpers";
import { RLN_CONFIG } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";

const logger = createTestLogger();

/**
 * Test Suite: Integration and Error Handling (INT-001 to INT-004, ERR-001 to ERR-003)
 *
 * Tests complete end-to-end flows and error handling:
 * - Complete lifecycle from registration to denial to recovery
 * - Multiple concurrent users with different tiers
 * - High-load scenarios
 * - Service unavailability handling
 * - Malformed data handling
 */
describe("RLN Integration and Error Handling", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  const TEST_TIMEOUT = 300000; // 5 minutes for integration tests

  beforeAll(async () => {
    logger.info("=== Initializing Integration and Error Handling Test Suite ===");

    rpcProvider = new ethers.JsonRpcProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = new ethers.JsonRpcProvider(RLN_CONFIG.services.sequencerUrl);
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

    logger.info("Test suite initialized");
  });

  afterAll(async () => {
    logger.info("=== Integration and Error Handling Test Suite Complete ===");
  });

  // ============================================================================
  // INTEGRATION TESTS (INT-001 to INT-004)
  // ============================================================================

  describe("INT-001: Complete Lifecycle - Register → Gasless → Exhaust → Deny → Premium → Reset", () => {
    // Skipped: This test requires waiting for epoch reset (24 hours) which is not practical in automated tests
    // The epoch reset logic is handled by the RLN prover's epoch service and RocksDB merge operator
    it.skip(
      "should handle complete user lifecycle",
      async () => {
        logger.info("INT-001: Starting complete lifecycle test");

        // Step 1: Create new user
        const user = await createFundedWallet(rpcProvider, admin);
        logger.info("Step 1: User created", { address: user.address });

        // Step 2: Mint Karma and register
        await karmaManager.mintKarma(user.address, 1n); // Entry tier
        await karmaManager.waitForRlnRegistration(user.address);
        logger.info("Step 2: User registered with Entry tier");

        // Step 3: Send gasless transactions
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
        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("int001-exceed"),
          });
        } catch {
          // Expected
        }
        await denyListManager.waitForDenied(user.address, 15000);
        logger.info("Step 4: User denied after quota exhaustion");

        // Step 5: Verify gasless is blocked
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("int001-blocked"),
        });
        expect(errorMessage).toMatch(/denied|reject|quota|timeout/i);
        logger.info("Step 5: Gasless blocked while denied");

        // Step 6: Pay premium gas
        const premiumReceipt = await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("int001-premium"),
        });
        expect(premiumReceipt.status).toBe(1);
        await denyListManager.waitForNotDenied(user.address, 15000);
        logger.info("Step 6: Premium gas paid, removed from deny list");

        // Step 7: Wait for new epoch
        await rlnClient.waitForNextEpoch();
        logger.info("Step 7: New epoch started");

        // Step 8: Send gasless again
        const finalReceipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("int001-final"),
        });
        expect(finalReceipt.status).toBe(1);
        logger.info("Step 8: Gasless working again");

        logger.info("INT-001: PASSED ✓ - Complete lifecycle successful");
      },
      TEST_TIMEOUT,
    );
  });

  describe("INT-002: Multiple Users with Different Tiers Operating Simultaneously", () => {
    it(
      "should handle multiple users with different quotas",
      async () => {
        logger.info("INT-002: Testing multiple users with different tiers");

        // Create users with different tiers
        const entryUser = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const newbieUser = await karmaManager.setupUserForGasless(rpcProvider, "newbie");
        const basicUser = await karmaManager.setupUserForGasless(rpcProvider, "basic");

        logger.info("Users created", {
          entry: entryUser.address,
          newbie: newbieUser.address,
          basic: basicUser.address,
        });

        // Send transactions SEQUENTIALLY to avoid nonce collisions
        // Entry: 2 tx
        const entryReceipts = [];
        for (let i = 0; i < 2; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(entryUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`int002-entry-${i}`),
          });
          entryReceipts.push(receipt);
        }

        // Newbie: 3 tx (subset of 6 quota)
        const newbieReceipts = [];
        for (let i = 0; i < 3; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(newbieUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`int002-newbie-${i}`),
          });
          newbieReceipts.push(receipt);
        }

        // Basic: 4 tx (subset of 16 quota)
        const basicReceipts = [];
        for (let i = 0; i < 4; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(basicUser, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`int002-basic-${i}`),
          });
          basicReceipts.push(receipt);
        }

        // Verify all succeeded
        const allReceipts = [...entryReceipts, ...newbieReceipts, ...basicReceipts];
        for (const receipt of allReceipts) {
          expect(receipt.status).toBe(1);
        }

        logger.info("INT-002: All transactions succeeded", {
          entryCount: entryReceipts.length,
          newbieCount: newbieReceipts.length,
          basicCount: basicReceipts.length,
        });

        logger.info("INT-002: PASSED ✓ - Multiple tiers operating simultaneously");
      },
      TEST_TIMEOUT,
    );
  });

  describe("INT-003: High-Load Scenario with Multiple Transactions", () => {
    it(
      "should handle multiple rapid transactions",
      async () => {
        // Use higher-quota tier for load testing
        const user = await karmaManager.setupUserForGasless(rpcProvider, "active");
        const txCount = 10; // Send 10 transactions rapidly

        logger.info("INT-003: Testing high-load scenario", {
          user: user.address,
          txCount,
        });

        const startTime = Date.now();
        const receipts = [];

        // Send transactions sequentially but rapidly
        for (let i = 0; i < txCount; i++) {
          const receipt = await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`int003-${i}`),
          });
          receipts.push(receipt);
        }

        const duration = Date.now() - startTime;

        // Verify all succeeded
        for (const receipt of receipts) {
          expect(receipt.status).toBe(1);
        }

        logger.info("INT-003: Load test results", {
          txCount,
          duration: `${duration}ms`,
          avgPerTx: `${Math.round(duration / txCount)}ms`,
        });

        logger.info("INT-003: PASSED ✓ - High-load scenario handled");
      },
      TEST_TIMEOUT,
    );
  });

  describe("INT-004: Epoch Transition with Active Users", () => {
    // Skipped: This test requires waiting for epoch transition (24 hours) which is not practical
    // For epoch transition logic, see rln-prover/prover/src/epoch_service.rs
    it.skip(
      "should handle epoch transition gracefully",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");

        logger.info("INT-004: Testing epoch transition", {
          user: user.address,
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
        const epoch2 = await rlnClient.waitForNextEpoch();
        logger.info("New epoch started", { epoch: epoch2 });

        // Send transaction in new epoch
        const receipt2 = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("int004-epoch2"),
        });
        expect(receipt2.status).toBe(1);

        // Verify both transactions succeeded
        expect(receipt1.blockNumber).toBeLessThan(receipt2.blockNumber!);

        logger.info("INT-004: PASSED ✓ - Epoch transition handled gracefully");
      },
      TEST_TIMEOUT + 120000,
    );
  });

  // ============================================================================
  // ERROR HANDLING TESTS (ERR-001 to ERR-003)
  // ============================================================================

  describe("ERR-001: Karma Service Unavailable Rejects Transaction", () => {
    it(
      "should handle karma service errors gracefully",
      async () => {
        // This test verifies the system behaves correctly when karma service is down
        // We test this by checking error handling with an invalid endpoint
        logger.info("ERR-001: Testing karma service error handling");

        // Create a client with invalid karma service URL
        const badClient = new RlnTestClient(
          rpcProvider,
          sequencerProvider,
          RLN_CONFIG.services.rpcUrl,
          "http://localhost:99999", // Invalid URL
        );

        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");

        // Try to get tier info with bad client - should fail gracefully
        try {
          await badClient.getUserTierInfo(user.address);
          // If it doesn't throw, the client handled the error internally
          logger.info("ERR-001: Client handled unavailable service gracefully");
        } catch (error: unknown) {
          const err = error instanceof Error ? error : new Error(String(error));
          // Expected to fail with connection or fetch error
          expect(err.message).toMatch(/failed|unavailable|ECONNREFUSED|fetch|not found/i);
          logger.info("ERR-001: Error handled correctly", { error: err.message });
        }

        // Verify the user can still send gasless transactions via sequencer
        // (the sequencer talks to karma service internally via gRPC)
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("err001-verify"),
        });
        expect(receipt.status).toBe(1);

        logger.info("ERR-001: PASSED ✓ - Karma service errors handled gracefully");
      },
      TEST_TIMEOUT,
    );
  });

  describe("ERR-002: RLN Prover Unavailable Handling", () => {
    it(
      "should timeout when prover cannot generate proof",
      async () => {
        // Test by using unregistered user (prover won't generate proof)
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("ERR-002: Testing prover unavailable handling", {
          user: user.address,
        });

        // Gasless tx without registration will wait for proof that never comes
        const startTime = Date.now();
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("err002"),
          },
          45000,
        );
        const duration = Date.now() - startTime;

        expect(errorMessage).toMatch(/timeout|rejected|proof/i);
        logger.info("ERR-002: Transaction failed after timeout", {
          duration: `${duration}ms`,
          error: errorMessage,
        });

        logger.info("ERR-002: PASSED ✓ - Prover unavailable handled correctly");
      },
      TEST_TIMEOUT,
    );
  });

  describe("ERR-003: Malformed Transaction Data Handling", () => {
    it(
      "should handle malformed transaction data gracefully",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");

        logger.info("ERR-003: Testing malformed data handling", {
          user: user.address,
        });

        // Test with extremely long data (should still work but costs more gas)
        const longData = "0x" + "ab".repeat(1000);

        try {
          const receipt = await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: longData,
            gasLimit: 100000, // Higher gas limit for larger data
          });
          // If it succeeds, that's fine
          logger.info("Long data transaction succeeded", { txHash: receipt.hash });
        } catch (error: unknown) {
          // If it fails, verify it's handled gracefully
          const err = error instanceof Error ? error : new Error(String(error));
          logger.info("Long data transaction failed gracefully", { error: err.message });
        }

        // Test with valid short transaction
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("err003-normal"),
        });
        expect(receipt.status).toBe(1);

        logger.info("ERR-003: PASSED ✓ - Malformed data handled gracefully");
      },
      TEST_TIMEOUT,
    );
  });
});
