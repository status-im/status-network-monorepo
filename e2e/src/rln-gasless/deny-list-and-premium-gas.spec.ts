import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager, resetAdminNonceManager } from "./utils/karma-manager";
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
 * - Deny list expiration (TTL) - NOW TESTABLE with short TTL
 * - Denied user rejection
 * - Premium gas bypass for denied users
 * - Premium gas threshold enforcement
 * - Gas estimate inflation for denied users
 *
 */
describe("RLN Deny List and Premium Gas", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  // Pre-registered user pools - created once in beforeAll
  const registeredUsers: ethers.HDNodeWallet[] = [];
  const fundedOnlyUsers: ethers.HDNodeWallet[] = [];
  let userIndex = 0;
  let fundedUserIndex = 0;

  // Get next pre-registered user from pool
  const getRegisteredUser = () => {
    if (userIndex >= registeredUsers.length) {
      throw new Error(`Not enough pre-registered users. Need more than ${registeredUsers.length}`);
    }
    return registeredUsers[userIndex++];
  };

  // Get next funded-only user from pool (no karma, not registered)
  const getFundedUser = () => {
    if (fundedUserIndex >= fundedOnlyUsers.length) {
      throw new Error(`Not enough funded users. Need more than ${fundedOnlyUsers.length}`);
    }
    return fundedOnlyUsers[fundedUserIndex++];
  };

  // Timeouts based on actual TX performance (~4-5s per gasless TX, P95: 4.7s)
  // Single TX tests: 20s (5s TX + 15s buffer)
  const TEST_TIMEOUT = 20000;
  // Multi-TX tests: ~5s per TX + buffer
  const MULTI_TX_TIMEOUT = 60000;
  // Deny list tests: 2 TXs (~10s) + rejection timeout (~7s) + deny list wait (~20s) = ~40s min
  const DENY_TEST_TIMEOUT = 90000;
  // High volume tests (10+ TXs)
  const HIGH_VOLUME_TIMEOUT = 120000;
  // Extended timeout for tests that need to wait for epoch boundary (60s epoch + buffer)
  const EPOCH_TEST_TIMEOUT = 180000;

  beforeAll(async () => {
    logger.info("=== Initializing Deny List and Premium Gas Test Suite ===");

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
    // This happens ONCE before all tests - no waiting during tests
    logger.info("Pre-registering test users...");

    const REGISTERED_USER_COUNT = 15; // Enough for all DENY tests
    const FUNDED_ONLY_USER_COUNT = 5; // For PREM tests that don't need registration

    // Create registered users (with Karma + RLN registration)
    for (let i = 0; i < REGISTERED_USER_COUNT; i++) {
      const user = await karmaManager.setupUserForGasless(rpcProvider, "entry");
      registeredUsers.push(user);
      logger.debug(`Pre-registered user ${i + 1}/${REGISTERED_USER_COUNT}`, { address: user.address });
    }

    // Create funded-only users (no Karma, for premium gas tests)
    for (let i = 0; i < FUNDED_ONLY_USER_COUNT; i++) {
      const user = await createFundedWallet(rpcProvider, admin);
      fundedOnlyUsers.push(user);
      logger.debug(`Pre-funded user ${i + 1}/${FUNDED_ONLY_USER_COUNT}`, { address: user.address });
    }

    logger.info("Test suite initialized", {
      registeredUsers: registeredUsers.length,
      fundedOnlyUsers: fundedOnlyUsers.length,
    });
  }, 300000); // 5 minute timeout for setup - 20 users × ~12s each = ~240s

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
        const user = getRegisteredUser(); // Use pre-registered user - no waiting!
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
        await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny001-exceed"),
          },
          10000, // 10s timeout for failure expectation
        );

        // Wait for and verify deny list addition
        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);
        const isDenied = await denyListManager.isDenied(user.address);

        //  User MUST be on deny list
        expect(isDenied).toBe(true);

        logger.info("DENY-001: PASSED ✓ - User added to deny list after quota violation");
      },
      DENY_TEST_TIMEOUT, // 2 TXs + rejection (~7s) + deny wait (~20s) = ~35s
    );
  });

  describe("DENY-002: Denied User Cannot Send Gasless Transactions", () => {
    it(
      "should reject gasless transactions from denied users",
      async () => {
        const user = getRegisteredUser();
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
        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny002-trigger"),
        });

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Subsequent gasless transactions should fail
        // Denial manifests as timeout (no proof generated) or explicit rejection
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny002-denied"),
        });

        //  Must be rejected - timeout means proof wasn't generated (denial in effect)
        expect(errorMessage).toMatch(/denied|reject|quota|timeout/i);

        logger.info("DENY-002: PASSED ✓ - Denied user cannot send gasless transactions");
      },
      DENY_TEST_TIMEOUT, // 2 TXs + rejection + deny wait + rejection = ~45s
    );
  });

  describe("DENY-003: Premium Gas Clears Deny Status (Recovery Path)", () => {
    it(
      "should allow user to recover from deny list via premium gas",
      async () => {
        const user = getRegisteredUser();
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-003: Testing premium gas recovery", { user: user.address });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny003-exhaust-${i}`),
          });
        }

        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny003-trigger"),
        });

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        //  User IS denied before premium gas
        expect(await denyListManager.isDenied(user.address)).toBe(true);

        // Pay premium gas to recover
        const premiumReceipt = await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny003-premium"),
        });
        expect(premiumReceipt.status).toBe(1);

        // Wait for deny list removal
        await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        //  User is NOT denied after premium gas
        expect(await denyListManager.isDenied(user.address)).toBe(false);

        logger.info("DENY-003: PASSED ✓ - Premium gas clears deny status");
      },
      DENY_TEST_TIMEOUT, // 2 gasless + rejection + premium + deny waits = ~45s
    );
  });

  describe("DENY-004: Denied User Can Send Premium Gas Transaction", () => {
    it(
      "should allow denied user to transact with premium gas",
      async () => {
        const user = getRegisteredUser();
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

        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny004-trigger"),
        });

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Premium gas transaction should succeed even while denied
        const receipt = await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny004-premium"),
        });

        //  Premium gas tx must succeed
        expect(receipt.status).toBe(1);

        logger.info("DENY-004: PASSED ✓ - Denied user can send premium gas transaction");
      },
      DENY_TEST_TIMEOUT, // 2 gasless + rejection + deny wait + premium = ~45s
    );
  });

  describe("DENY-005: Premium Gas Payment Removes User from Deny List", () => {
    it(
      "should remove user from deny list after premium gas payment",
      async () => {
        const user = getRegisteredUser();
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

        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny005-trigger"),
        });

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        //  User is denied
        expect(await denyListManager.isDenied(user.address)).toBe(true);

        // Pay premium gas
        await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny005-premium"),
        });

        // Wait for removal
        await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        //  User is removed from deny list
        const isDenied = await denyListManager.isDenied(user.address);
        expect(isDenied).toBe(false);

        logger.info("DENY-005: PASSED ✓ - Premium gas payment removes user from deny list");
      },
      DENY_TEST_TIMEOUT, // 2 gasless + rejection + deny wait + premium + removal wait = ~50s
    );
  });

  describe("DENY-006: After Premium Recovery + New Epoch, User Can Send Gasless Again", () => {
    // NOW RUNNABLE with short epochs (10s)
    it(
      "should allow gasless transactions after recovery and new epoch",
      async () => {
        const user = getRegisteredUser();
        const quota = RLN_CONFIG.tiers.entry.quota;
        const epochDuration = RLN_CONFIG.test.epochDurationSeconds;

        logger.info("DENY-006: Testing post-recovery gasless capability", {
          user: user.address,
          epochDurationSeconds: epochDuration,
        });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny006-exhaust-${i}`),
          });
        }

        await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("deny006-trigger"),
          },
          10000, // 10s timeout for failure expectation
        );

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Pay premium to get removed
        await rlnClient.sendPremiumGasTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          gasPrice: PREMIUM_GAS_PRICE,
          data: uniqueTxData("deny006-premium"),
        });

        await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Wait for new epoch (quota resets)
        logger.info(`Waiting for new epoch (max ${epochDuration + 2}s)...`);
        await rlnClient.waitForNextEpoch((epochDuration + 2) * 1000);

        // Allow prover to sync state after deny list clearance + epoch change
        await rlnClient.sleep(1000);

        // Should be able to send gasless again
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny006-gasless-again"),
        });

        //  Gasless must work after recovery + epoch
        expect(receipt.status).toBe(1);

        logger.info("DENY-006: PASSED ✓ - User can send gasless after recovery and new epoch");
      },
      EPOCH_TEST_TIMEOUT, // This test waits for epoch boundary (60s)
    );
  });

  describe("DENY-007: Multiple Users Can Be on Deny List Simultaneously", () => {
    it(
      "should handle multiple denied users correctly",
      async () => {
        logger.info("DENY-007: Testing multiple denied users");

        // Get 3 pre-registered users from pool
        const users = [getRegisteredUser(), getRegisteredUser(), getRegisteredUser()];
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

          await rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny007-${user.address.slice(-4)}-trigger`),
          });
        }

        // Wait for all to be denied
        for (const user of users) {
          await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);
        }

        //  All users must be denied
        for (const user of users) {
          const isDenied = await denyListManager.isDenied(user.address);
          expect(isDenied).toBe(true);
        }

        logger.info("DENY-007: PASSED ✓ - Multiple users can be on deny list simultaneously");
      },
      HIGH_VOLUME_TIMEOUT, // 3 users × 3 TXs = 9 TXs = ~36s
    );
  });

  describe("DENY-008: Deny List State is Consistent", () => {
    it(
      "should maintain deny list state consistently across checks",
      async () => {
        const user = getRegisteredUser();
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("DENY-008: Testing deny list consistency", { user: user.address });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny008-exhaust-${i}`),
          });
        }

        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny008-trigger"),
        });

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Multiple rapid checks should return consistent result
        const results: boolean[] = [];
        for (let i = 0; i < 5; i++) {
          const isDenied = await denyListManager.isDenied(user.address);
          results.push(isDenied);
        }

        //  All checks must return true (consistent state)
        expect(results.every((r) => r === true)).toBe(true);

        logger.info("DENY-008: PASSED ✓ - Deny list state persists consistently");
      },
      DENY_TEST_TIMEOUT, // 2 gasless + rejection + deny wait + 5 checks = ~45s
    );
  });

  describe("DENY-009: Concurrent Deny List Additions Are Safe", () => {
    it(
      "should handle concurrent deny list additions safely",
      async () => {
        logger.info("DENY-009: Testing concurrent deny list additions");

        // Get 3 pre-registered users from pool
        const users = [getRegisteredUser(), getRegisteredUser(), getRegisteredUser()];
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
          rlnClient.sendGaslessTransactionExpectFailure(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny009-${user.address.slice(-4)}-trigger`),
          }),
        );

        await Promise.all(triggerPromises);

        // Wait for all to be denied
        await Promise.all(
          users.map((user) => denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs)),
        );

        //  All users must be denied (no state corruption)
        for (const user of users) {
          const isDenied = await denyListManager.isDenied(user.address);
          expect(isDenied).toBe(true);
        }

        logger.info("DENY-009: PASSED ✓ - Concurrent deny list additions handled safely");
      },
      HIGH_VOLUME_TIMEOUT, // 3 users × 3 TXs = 9 TXs = ~36s
    );
  });

  // ============================================================================
  // PREMIUM GAS TESTS (PREM-001 to PREM-006)
  // ============================================================================

  describe("PREM-001: Transaction with gasPrice >= 10 Gwei Bypasses RLN", () => {
    it(
      "should bypass RLN for premium gas transactions",
      async () => {
        // Get pre-funded wallet WITHOUT Karma
        const user = getFundedUser();

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

        //  Premium gas must succeed without RLN
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
        // Get pre-funded wallet WITHOUT Karma
        const user = getFundedUser();

        logger.info("PREM-002: Testing sub-threshold gas requires RLN", {
          user: user.address,
          gasPrice: formatGwei(SUB_THRESHOLD_GAS_PRICE),
        });

        // Sub-threshold gas without Karma should fail (no proof)
        const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData("prem002"),
          },
          RLN_CONFIG.test.proofTimeoutMs,
        );

        //  Must fail for proof/timeout reason
        expect(errorMessage).toMatch(/timeout|rejected|proof|invalid/i);

        logger.info("PREM-002: PASSED ✓ - Sub-threshold gas requires RLN");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-003: Exactly Threshold (10 Gwei) Bypasses RLN", () => {
    it(
      "should bypass RLN at exactly threshold gas price",
      async () => {
        const user = getFundedUser();

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

        //  Exactly threshold must work
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
        const user = getFundedUser();

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
      MULTI_TX_TIMEOUT, // 3 premium TXs = ~12s
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
          //  Must fail for insufficient funds
          expect(err.message).toMatch(/insufficient|funds|balance/i);
          logger.info("PREM-005: Failed as expected", { error: err.message });
        }

        logger.info("PREM-005: PASSED ✓ - Unfunded wallet premium gas fails");
      },
      TEST_TIMEOUT,
    );
  });

  describe("PREM-006: Gas Estimate Shows Premium Multiplier for Denied Users", () => {
    it(
      "should return inflated gas estimate for denied users",
      async () => {
        const user = getRegisteredUser();
        const quota = RLN_CONFIG.tiers.entry.quota;

        logger.info("PREM-006: Testing gas estimate premium multiplier", {
          user: user.address,
        });

        // Get baseline estimate before denial (user is NOT denied)
        const baselineEstimate = await rlnClient.lineaEstimateGas({
          from: user.address,
          to: TEST_RECIPIENT,
          value: "0x0",
        });

        const baselineGasLimit = BigInt(baselineEstimate.gasLimit);
        logger.info("Baseline gas estimate", { gasLimit: baselineGasLimit.toString() });

        // Get denied
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`prem006-exhaust-${i}`),
          });
        }

        await rlnClient.sendGaslessTransactionExpectFailure(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("prem006-trigger"),
        });

        await denyListManager.waitForDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

        // Get estimate while denied
        const deniedEstimate = await rlnClient.lineaEstimateGas({
          from: user.address,
          to: TEST_RECIPIENT,
          value: "0x0",
        });

        const deniedGasLimit = BigInt(deniedEstimate.gasLimit);
        logger.info("Denied gas estimate", { gasLimit: deniedGasLimit.toString() });

        //  Denied estimate should be significantly higher (premium multiplier)
        // Premium multiplier is 1.5x, so denied should be at least 1.3x baseline
        const minimumExpected = (baselineGasLimit * 130n) / 100n;
        expect(deniedGasLimit).toBeGreaterThanOrEqual(minimumExpected);

        logger.info("PREM-006: PASSED ✓ - Gas estimate shows premium multiplier for denied users", {
          baseline: baselineGasLimit.toString(),
          denied: deniedGasLimit.toString(),
          ratio: (Number(deniedGasLimit) / Number(baselineGasLimit)).toFixed(2),
        });
      },
      DENY_TEST_TIMEOUT, // 2 gasless + rejection + deny wait + estimates = ~45s
    );
  });
});
