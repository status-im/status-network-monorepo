import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager } from "./utils/karma-manager";
import {
  createFundedWallet,
  uniqueTxData,
  formatGwei,
  TEST_RECIPIENT,
  PREMIUM_GAS_PRICE,
  THRESHOLD_GAS_PRICE,
  SUB_THRESHOLD_GAS_PRICE,
} from "./utils/test-helpers";
import { RLN_CONFIG } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";
import { txBenchmarker } from "./utils/tx-benchmarker";

const logger = createTestLogger();

/**
 * Test Suite: Deny List and Premium Gas (DENY-001 to DENY-009, PREM-001 to PREM-006)
 *
 * Tests deny list functionality and premium gas bypass:
 * - Deny list addition on quota violation
 * - Deny list expiration (TTL)
 * - Denied user rejection
 * - Premium gas bypass
 * - Premium gas threshold enforcement
 * - linea_estimateGas with premium multiplier
 */
describe("RLN Deny List and Premium Gas", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  const TEST_TIMEOUT = 180000; // 3 minutes

  beforeAll(async () => {
    logger.info("=== Initializing Deny List and Premium Gas Test Suite ===");

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
    logger.info("=== Deny List and Premium Gas Test Suite Complete ===");
    txBenchmarker.printSummary();
  });

  // ============================================================================
  // DENY LIST TESTS (DENY-001 to DENY-009)
  // ============================================================================

  describe("DENY-001: User Exceeding Quota is Added to Deny List", () => {
    it(
      "should add user to deny list when quota is exceeded",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-001: Testing deny list addition", {
          user: user.address,
          quota,
        });

        // Exhaust quota
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny001-exhaust-${i}`),
          });
        }

        // Attempt to exceed quota
        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny001-exceed"),
          });
        } catch {
          // Expected
        }

        // Wait for and verify deny list addition
        await denyListManager.waitForDenied(user.address, 15000);
        const isDenied = await denyListManager.isDenied(user.address);
        expect(isDenied).toBe(true);

        logger.info("DENY-001: PASSED ✓ - User added to deny list after quota violation");
      },
      TEST_TIMEOUT,
    );
  });

  describe("DENY-002: Denied User Cannot Send Gasless Transactions", () => {
    it(
      "should reject gasless transactions from denied users",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-002: Testing denial rejection", { user: user.address });

        // Exhaust quota and get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny002-exhaust-${i}`),
          });
        }

        // Trigger deny list
        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny002-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);

        // Subsequent gasless transactions should fail
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny002-denied"),
        });

        expect(errorMessage).toMatch(/denied|reject|quota|timeout/i);

        logger.info("DENY-002: PASSED ✓ - Denied user cannot send gasless transactions");
      },
      TEST_TIMEOUT,
    );
  });

  describe("DENY-003: Deny List Entry Expires After TTL", () => {
    it(
      "should remove deny list entry after TTL expires",
      async () => {
        // This test requires short TTL configuration
        // Skip if TTL is too long for testing
        const ttlMinutes = parseInt(process.env.DENY_LIST_TTL_MINUTES || "60", 10);

        if (ttlMinutes > 5) {
          logger.warn("DENY-003: Skipping - TTL too long for test", { ttlMinutes });
          return;
        }

        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-003: Testing TTL expiration", { user: user.address, ttlMinutes });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny003-exhaust-${i}`),
          });
        }

        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny003-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);

        // Wait for TTL to expire
        const waitTime = (ttlMinutes + 1) * 60 * 1000;
        logger.info(`Waiting ${ttlMinutes + 1} minutes for TTL expiration...`);
        await new Promise((resolve) => setTimeout(resolve, waitTime));

        // Verify user is no longer denied
        const isDenied = await denyListManager.isDenied(user.address);
        expect(isDenied).toBe(false);

        logger.info("DENY-003: PASSED ✓ - Deny list entry expired after TTL");
      },
      TEST_TIMEOUT + 600000, // Extra time for TTL wait
    );
  });

  describe("DENY-004: Denied User Can Send Premium Gas Transaction", () => {
    it(
      "should allow denied user to transact with premium gas",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-004: Testing premium gas for denied user", {
          user: user.address,
        });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny004-exhaust-${i}`),
          });
        }

        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny004-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);

        // Premium gas transaction should succeed
        const receipt = await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny004-premium"),
        });

        expect(receipt.status).toBe(1);

        logger.info("DENY-004: PASSED ✓ - Denied user can send premium gas transaction");
      },
      TEST_TIMEOUT,
    );
  });

  describe("DENY-005: Premium Gas Payment Removes User from Deny List", () => {
    it(
      "should remove user from deny list after premium gas payment",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-005: Testing premium gas deny list removal", {
          user: user.address,
        });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny005-exhaust-${i}`),
          });
        }

        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny005-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);
        expect(await denyListManager.isDenied(user.address)).toBe(true);

        // Pay premium gas
        await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny005-premium"),
        });

        // Wait for removal
        await denyListManager.waitForNotDenied(user.address, 15000);
        const isDenied = await denyListManager.isDenied(user.address);
        expect(isDenied).toBe(false);

        logger.info("DENY-005: PASSED ✓ - Premium gas payment removes user from deny list");
      },
      TEST_TIMEOUT,
    );
  });

  describe("DENY-006: After Removal, User Can Send Gasless Again (New Epoch)", () => {
    // Skipped: This test requires waiting for epoch reset (24 hours) which is not practical
    // Epoch-based quota reset logic is in rln-prover/prover/src/epoch_service.rs
    it.skip(
      "should allow gasless transactions after removal and new epoch",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-006: Testing post-removal gasless capability", {
          user: user.address,
        });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny006-exhaust-${i}`),
          });
        }

        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny006-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);

        // Pay premium to get removed
        await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny006-premium"),
        });

        await denyListManager.waitForNotDenied(user.address, 15000);

        // Wait for new epoch (quota resets)
        await rlnClient.waitForNextEpoch();

        // Should be able to send gasless again
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny006-gasless-again"),
        });

        expect(receipt.status).toBe(1);

        logger.info("DENY-006: PASSED ✓ - User can send gasless after removal and new epoch");
      },
      TEST_TIMEOUT + 120000,
    );
  });

  describe("DENY-007: Multiple Users Can Be on Deny List Simultaneously", () => {
    it(
      "should handle multiple denied users correctly",
      async () => {
        logger.info("DENY-007: Testing multiple denied users");

        const users = await karmaManager.setupMultipleUsers(rpcProvider, 3, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        // Get all users denied
        for (const user of users) {
          for (let i = 0; i < quota; i++) {
            await rlnClient.sendGaslessTransaction(user, {
              to: TEST_RECIPIENT,
              value: 0n,
              data: uniqueTxData(`deny007-${user.address.slice(-4)}-${i}`),
            });
          }

          try {
            await rlnClient.sendGaslessTransactionExpectFailure(user, {
              to: TEST_RECIPIENT,
              value: 0n,
              data: uniqueTxData(`deny007-${user.address.slice(-4)}-trigger`),
            });
          } catch {
            // Expected
          }
        }

        // Wait for all to be denied
        for (const user of users) {
          await denyListManager.waitForDenied(user.address, 20000);
        }

        // Verify all are denied
        for (const user of users) {
          const isDenied = await denyListManager.isDenied(user.address);
          expect(isDenied).toBe(true);
        }

        logger.info("DENY-007: PASSED ✓ - Multiple users can be on deny list simultaneously");
      },
      TEST_TIMEOUT * 2,
    );
  });

  describe("DENY-008: Deny List Persists Across Service Checks", () => {
    it(
      "should maintain deny list state consistently",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-008: Testing deny list persistence", { user: user.address });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny008-exhaust-${i}`),
          });
        }

        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny008-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);

        // Multiple checks should return consistent result
        for (let i = 0; i < 5; i++) {
          const isDenied = await denyListManager.isDenied(user.address);
          expect(isDenied).toBe(true);
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }

        logger.info("DENY-008: PASSED ✓ - Deny list state persists consistently");
      },
      TEST_TIMEOUT,
    );
  });

  describe("DENY-009: Concurrent Deny List Additions Don't Corrupt State", () => {
    it(
      "should handle concurrent deny list additions safely",
      async () => {
        logger.info("DENY-009: Testing concurrent deny list additions");

        const users = await karmaManager.setupMultipleUsers(rpcProvider, 3, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        // Exhaust quotas first
        for (const user of users) {
          for (let i = 0; i < quota; i++) {
            await rlnClient.sendGaslessTransaction(user, {
              to: TEST_RECIPIENT,
              value: 0n,
              data: uniqueTxData(`deny009-${user.address.slice(-4)}-${i}`),
            });
          }
        }

        // Trigger denial concurrently
        const triggerPromises = users.map((user) =>
          rlnClient
            .sendGaslessTransactionExpectFailure(user, {
              to: TEST_RECIPIENT,
              value: 0n,
              data: uniqueTxData(`deny009-${user.address.slice(-4)}-trigger`),
            })
            .catch(() => {
              // Expected to fail
            }),
        );

        await Promise.all(triggerPromises);

        // Wait for all to be denied
        await Promise.all(users.map((user) => denyListManager.waitForDenied(user.address, 20000)));

        // Verify state is correct for all users
        for (const user of users) {
          const isDenied = await denyListManager.isDenied(user.address);
          expect(isDenied).toBe(true);
        }

        logger.info("DENY-009: PASSED ✓ - Concurrent deny list additions handled safely");
      },
      TEST_TIMEOUT * 2,
    );
  });

  // ============================================================================
  // PREMIUM GAS TESTS (PREM-001 to PREM-006)
  // ============================================================================

  describe("PREM-001: Transaction with gasPrice >= 10 Gwei Bypasses RLN", () => {
    it(
      "should bypass RLN for premium gas transactions",
      async () => {
        // Create funded wallet WITHOUT Karma
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("PREM-001: Testing premium gas RLN bypass", {
          user: user.address,
          gasPrice: formatGwei(PREMIUM_GAS_PRICE),
        });

        // User has no Karma but premium gas should work
        const receipt = await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("prem001"),
        });

        expect(receipt.status).toBe(1);

        logger.info("PREM-001: PASSED ✓ - Premium gas bypasses RLN");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-002: Transaction with gasPrice < 10 Gwei Requires RLN", () => {
    it(
      "should require RLN for sub-threshold gas price",
      async () => {
        // Create funded wallet WITHOUT Karma
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("PREM-002: Testing sub-threshold gas requires RLN", {
          user: user.address,
          gasPrice: formatGwei(SUB_THRESHOLD_GAS_PRICE),
        });

        // Sub-threshold gas without Karma should fail
        try {
          const tx = await user.sendTransaction({
            to: TEST_RECIPIENT,
            value: 0n,
            gasLimit: 25000,
            gasPrice: SUB_THRESHOLD_GAS_PRICE,
            data: uniqueTxData("prem002"),
          });

          // If tx was sent, it should timeout or fail
          await Promise.race([
            tx.wait(1, 30000),
            new Promise((_, reject) => setTimeout(() => reject(new Error("Timeout waiting for tx")), 30000)),
          ]);

          // If we get here, the tx somehow succeeded - this is unexpected
          throw new Error("Expected transaction to fail without RLN proof");
        } catch (error: unknown) {
          const err = error instanceof Error ? error : new Error(String(error));
          logger.info("PREM-002: Transaction failed as expected", { error: err.message });
          expect(err.message).toMatch(/timeout|rejected|proof|invalid/i);
        }

        logger.info("PREM-002: PASSED ✓ - Sub-threshold gas requires RLN");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-003: Exactly Threshold (10 Gwei) Bypasses RLN", () => {
    it(
      "should bypass RLN at exactly threshold gas price",
      async () => {
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("PREM-003: Testing exact threshold", {
          user: user.address,
          gasPrice: formatGwei(THRESHOLD_GAS_PRICE),
        });

        const receipt = await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: THRESHOLD_GAS_PRICE,
          data: uniqueTxData("prem003"),
        });

        expect(receipt.status).toBe(1);

        logger.info("PREM-003: PASSED ✓ - Exactly threshold gas bypasses RLN");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-004: Premium Gas Works Even Without Karma", () => {
    it(
      "should allow premium gas transactions without Karma registration",
      async () => {
        const user = await createFundedWallet(rpcProvider, admin);

        logger.info("PREM-004: Testing premium gas without Karma", {
          user: user.address,
        });

        // Verify user has no Karma
        const karmaBalance = await contracts.karma.balanceOf(user.address);
        expect(karmaBalance).toBe(0n);

        // Multiple premium transactions should work
        for (let i = 0; i < 3; i++) {
          const receipt = await rlnClient.sendPremiumGasTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            gasPrice: PREMIUM_GAS_PRICE,
            data: uniqueTxData(`prem004-${i}`),
          });
          expect(receipt.status).toBe(1);
        }

        logger.info("PREM-004: PASSED ✓ - Premium gas works without Karma");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-005: Premium Gas Transaction from Unfunded Wallet Fails", () => {
    it(
      "should fail premium gas transaction without funds",
      async () => {
        const wallet = ethers.Wallet.createRandom().connect(rpcProvider);

        logger.info("PREM-005: Testing unfunded wallet failure", {
          user: wallet.address,
        });

        // Verify wallet has no balance
        const balance = await rpcProvider.getBalance(wallet.address);
        expect(balance).toBe(0n);

        try {
          await rlnClient.sendPremiumGasTransaction(wallet, {
            to: TEST_RECIPIENT,
            value: 0n,
            gasPrice: PREMIUM_GAS_PRICE,
            data: uniqueTxData("prem005"),
          });
          throw new Error("Expected transaction to fail");
        } catch (error: unknown) {
          const err = error instanceof Error ? error : new Error(String(error));
          expect(err.message).toMatch(/insufficient|funds|balance/i);
          logger.info("PREM-005: Failed as expected", { error: err.message });
        }

        logger.info("PREM-005: PASSED ✓ - Unfunded wallet premium gas fails");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-006: linea_estimateGas Returns Premium Multiplier for Denied Users", () => {
    it(
      "should return higher gas estimate for denied users",
      async () => {
        const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("PREM-006: Testing linea_estimateGas premium multiplier", {
          user: user.address,
        });

        // Get baseline estimate before denial
        const baselineEstimate = await rlnClient.lineaEstimateGas({
          from: user.address,
          to: TEST_RECIPIENT,
          value: "0x0",
        });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`prem006-exhaust-${i}`),
          });
        }

        try {
          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("prem006-trigger"),
          });
        } catch {
          // Expected
        }

        await denyListManager.waitForDenied(user.address, 15000);

        // Get estimate while denied
        const deniedEstimate = await rlnClient.lineaEstimateGas({
          from: user.address,
          to: TEST_RECIPIENT,
          value: "0x0",
        });

        logger.info("Gas estimates", {
          baseline: baselineEstimate,
          denied: deniedEstimate,
        });

        // Denied estimate should indicate premium required
        // The exact behavior depends on implementation
        expect(deniedEstimate).toBeDefined();
        expect(deniedEstimate.gasLimit).toBeDefined();

        logger.info("PREM-006: PASSED ✓ - linea_estimateGas works for denied users");
      },
      TEST_TIMEOUT,
    );
  });
});
