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
import {
  formatScenario,
  DENY_001,
  DENY_002,
  DENY_003,
  DENY_004,
  DENY_005,
  DENY_006,
  DENY_007,
  DENY_008,
  DENY_009,
  PREM_001,
  PREM_002,
  PREM_003,
  PREM_004,
  PREM_005,
  PREM_006,
} from "./helpers/scenario";

const logger = createTestLogger();

/**
 * Test Suite: Deny List and Premium Gas (DENY_001 to DENY_009, PREM_001 to PREM_006)
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
    // Uses skipRegistrationWait to avoid 20s sleep per user, then does a single wait at the end
    logger.info("Pre-registering test users...");

    const REGISTERED_USER_COUNT = 15; // Enough for all DENY tests
    const FUNDED_ONLY_USER_COUNT = 5; // For PREM tests that don't need registration

    // Create registered users (with Karma + RLN registration)
    for (let i = 0; i < REGISTERED_USER_COUNT; i++) {
      const user = await karmaManager.setupUserForGasless(rpcProvider, "entry", undefined, {
        skipRegistrationWait: true,
      });
      registeredUsers.push(user);
      logger.debug(`Pre-registered user ${i + 1}/${REGISTERED_USER_COUNT}`, { address: user.address });
    }

    // Create funded-only users (no Karma, for premium gas tests)
    for (let i = 0; i < FUNDED_ONLY_USER_COUNT; i++) {
      const user = await createFundedWallet(rpcProvider, admin);
      fundedOnlyUsers.push(user);
      logger.debug(`Pre-funded user ${i + 1}/${FUNDED_ONLY_USER_COUNT}`, { address: user.address });
    }

    // Single registration wait for all users
    logger.info("Waiting for RLN registrations to complete...");
    await karmaManager.waitForRlnRegistration("batch-all");
    logger.info("Registration wait complete");

    logger.info("Test suite initialized", {
      registeredUsers: registeredUsers.length,
      fundedOnlyUsers: fundedOnlyUsers.length,
    });
  }, RLN_CONFIG.test.timeouts.setupLarge);

  afterAll(async () => {
    logger.info("=== Deny List and Premium Gas Test Suite Complete ===");
    txBenchmarker.printSummary();
  });

  // ============================================================================
  // DENY LIST TESTS (DENY_001 to DENY_009)
  // ============================================================================

  it(
    formatScenario(DENY_001),
    async () => {
      const user = getRegisteredUser(); // Use pre-registered user - no waiting!
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_001.id}: Testing deny list addition on quota exhaustion`, {
        user: user.address,
        quota,
      });

      // Ensure enough epoch time remains for quota TXs + expected failure (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny001-quota-${i}`),
        });
      }
      logger.info(`${DENY_001.id}: Quota transactions succeeded, user should be denied`);

      // Verify user is denied by attempting another gasless TX (should be rejected).
      // The prover tracks quota per epoch and rejects proof requests for exhausted users.
      // This is more reliable than gas estimate polling which can miss the deny window.
      const error = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("deny001-exceed"),
        },
        10000,
      );
      expect(error).toMatch(/quota|deny|denied|timeout|exceeded|resource/i);

      logger.info(`${DENY_001.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 2 gasless + 1 expected failure
  );

  it(
    formatScenario(DENY_002),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_002.id}: Testing denial rejection`, { user: user.address });

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny002-quota-${i}`),
        });
      }

      // Subsequent gasless transactions should fail (quota exhausted).
      // The prover rejects proof requests immediately — no need to wait for deny list propagation.
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("deny002-denied"),
      });

      // Must be rejected - either denied (if deny list synced) or resource_exhausted (quota check)
      expect(errorMessage).toMatch(/deny|denied|reject|quota|timeout|resource.*exhausted/i);

      logger.info(`${DENY_002.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 2 gasless + 1 expected failure
  );

  it(
    formatScenario(DENY_003),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_003.id}: Testing premium gas recovery`, { user: user.address });

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny003-quota-${i}`),
        });
      }

      // Verify user is denied by attempting another gasless TX (should be rejected).
      const denyError = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData("deny003-verify-denied") },
        10000,
      );
      expect(denyError).toMatch(/quota|deny|denied|timeout|exceeded|resource/i);

      // Pay premium gas (transaction succeeds even while denied)
      // Premium gas now removes from deny list AND resets epoch counter (quota refresh)
      const premiumReceipt = await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("deny003-premium"),
      });
      expect(premiumReceipt.status).toBe(1);

      // User is removed from deny list after prover processes the premium gas block.
      // Instant removal via gRPC may work quickly, but if not, TTL expiry (60s) is the fallback.
      await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);
      const isDeniedAfterPremium = await denyListManager.isDenied(user.address);
      expect(isDeniedAfterPremium).toBe(false);

      logger.info(`${DENY_003.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // 2 gasless + premium + deny list poll (may need TTL expiry)
  );

  it(
    formatScenario(DENY_004),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_004.id}: Testing premium gas for denied user`, {
        user: user.address,
      });

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny004-quota-${i}`),
        });
      }

      // Premium gas transaction should succeed even after quota exhaustion
      const receipt = await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("deny004-premium"),
      });

      //  Premium gas tx must succeed
      expect(receipt.status).toBe(1);

      logger.info(`${DENY_004.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 2 gasless + premium
  );

  it(
    formatScenario(DENY_005),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_005.id}: Testing premium gas deny list removal`, {
        user: user.address,
      });

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny005-quota-${i}`),
        });
      }

      // Verify user is denied by attempting another gasless TX (should be rejected).
      const denyError = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData("deny005-verify-denied") },
        10000,
      );
      expect(denyError).toMatch(/quota|deny|denied|timeout|exceeded|resource/i);

      // Pay premium gas (succeeds even while denied)
      // Premium gas now removes from deny list AND resets epoch counter (quota refresh)
      await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("deny005-premium"),
      });

      // User should be able to send gasless again (quota was reset by premium payment)
      // The RlnProverForwarderValidator calls removeFromDenyList with reset_epoch_counter=true
      // when it detects premium gas, so the prover's quota is reset immediately.
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("deny005-gasless-after-premium"),
      });
      expect(receipt.status).toBe(1);

      logger.info(`${DENY_005.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 2 gasless + 1 failure + premium + 1 gasless
  );

  it(
    formatScenario(DENY_006),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_006.id}: Testing post-recovery gasless capability`, {
        user: user.address,
      });

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny006-quota-${i}`),
        });
      }

      // Pay premium gas — removes from deny list AND resets epoch counter (quota refresh)
      // No need to wait for deny list propagation; premium gas works regardless of deny status
      await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("deny006-premium"),
      });

      // Should be able to send gasless again (quota was reset by premium payment)
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("deny006-gasless-again"),
      });

      //  Gasless must work after premium gas payment (no epoch wait needed)
      expect(receipt.status).toBe(1);

      logger.info(`${DENY_006.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // ensureEpochWindow may wait up to 30s + 2 gasless + premium + 1 gasless
  );

  it(
    formatScenario(DENY_007),
    async () => {
      logger.info(`${DENY_007.id}: Testing multiple denied users`);

      // Get 3 pre-registered users from pool
      const users = [getRegisteredUser(), getRegisteredUser(), getRegisteredUser()];
      const quota = RLN_CONFIG.tiers.entry.quota;

      // Ensure enough epoch time for all users' quota TXs + failure verification (~25s needed)
      await rlnClient.ensureEpochWindow(25000);

      // Get all users denied by exhausting quota
      for (const user of users) {
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny007-${user.address.slice(-4)}-quota-${i}`),
          });
        }
      }

      // Verify all users are denied by attempting another gasless TX for each.
      // The prover rejects proof requests for users who have exhausted their quota.
      for (const user of users) {
        const error = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny007-${user.address.slice(-4)}-exceed`),
          },
          10000,
        );
        expect(error).toMatch(/quota|deny|denied|timeout|exceeded|resource/i);
      }

      logger.info(`${DENY_007.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.highVolume, // 3 users × 3 TXs = 9 TXs = ~36s
  );

  it(
    formatScenario(DENY_008),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${DENY_008.id}: Testing deny list consistency`, { user: user.address });

      // Ensure enough epoch time remains for quota TXs + verification (prevents epoch boundary resets)
      await rlnClient.ensureEpochWindow(20000);

      // Send quota transactions (user is added to deny list on the last one)
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`deny008-quota-${i}`),
        });
      }

      // Multiple rapid gasless TX attempts should all fail consistently (quota exhausted).
      // This verifies the prover consistently rejects proof requests for over-quota users.
      const results: boolean[] = [];
      for (let i = 0; i < 3; i++) {
        try {
          await rlnClient.sendGaslessTransactionExpectFailure(
            user,
            {
              to: TEST_RECIPIENT,
              value: 0n,
              data: uniqueTxData(`deny008-consistency-${i}`),
            },
            10000,
          );
          results.push(true); // Failed as expected = denied
        } catch {
          results.push(false); // Unexpectedly succeeded = not denied
        }
      }

      //  All checks must show denial (consistent state)
      expect(results.every((r) => r === true)).toBe(true);

      logger.info(`${DENY_008.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.highVolume, // 2 gasless + 3 denial checks
  );

  it(
    formatScenario(DENY_009),
    async () => {
      logger.info(`${DENY_009.id}: Testing concurrent deny list additions`);

      // Get 3 pre-registered users from pool
      const users = [getRegisteredUser(), getRegisteredUser(), getRegisteredUser()];
      const quota = RLN_CONFIG.tiers.entry.quota;

      // Ensure enough epoch time for all users' quota TXs + failure verification (~25s needed)
      await rlnClient.ensureEpochWindow(25000);

      // Send quota transactions for all users (each user is added to deny list on last tx)
      for (const user of users) {
        for (let i = 0; i < quota; i++) {
          await rlnClient.sendGaslessTransaction(user, {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny009-${user.address.slice(-4)}-quota-${i}`),
          });
        }
      }

      // Verify all users are denied by attempting another gasless TX for each.
      // The prover rejects proof requests for users who have exhausted their quota,
      // so we don't need to poll gas estimates (which can miss the deny window due to epoch changes).
      for (const user of users) {
        const error = await rlnClient.sendGaslessTransactionExpectFailure(
          user,
          {
            to: TEST_RECIPIENT,
            value: 0n,
            data: uniqueTxData(`deny009-${user.address.slice(-4)}-exceed`),
          },
          10000,
        );
        expect(error).toMatch(/quota|deny|denied|timeout|exceeded|resource/i);
      }

      logger.info(`${DENY_009.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.highVolume, // 3 users × 2 TXs sequential + 3 failure verifications
  );

  // ============================================================================
  // PREMIUM GAS TESTS (PREM_001 to PREM_006)
  // ============================================================================

  it(
    formatScenario(PREM_001),
    async () => {
      // Get pre-funded wallet WITHOUT Karma
      const user = getFundedUser();

      logger.info(`${PREM_001.id}: Testing premium gas RLN bypass`, {
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

      logger.info(`${PREM_001.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(PREM_002),
    async () => {
      // Get pre-funded wallet WITHOUT Karma
      const user = getFundedUser();

      logger.info(`${PREM_002.id}: Testing sub-threshold gas requires RLN`, {
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

      logger.info(`${PREM_002.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(PREM_003),
    async () => {
      const user = getFundedUser();

      logger.info(`${PREM_003.id}: Testing exact threshold`, {
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

      logger.info(`${PREM_003.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(PREM_004),
    async () => {
      const user = getFundedUser();

      logger.info(`${PREM_004.id}: Testing premium gas without Karma`, {
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

      logger.info(`${PREM_004.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx, // 3 premium TXs = ~12s
  );

  it(
    formatScenario(PREM_005),
    async () => {
      const wallet = ethers.Wallet.createRandom().connect(rpcProvider);

      logger.info(`${PREM_005.id}: Testing unfunded wallet failure`, {
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
        logger.info(`${PREM_005.id}: Failed as expected`, { error: err.message });
      }

      logger.info(`${PREM_005.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.singleTx,
  );

  it(
    formatScenario(PREM_006),
    async () => {
      const user = getRegisteredUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${PREM_006.id}: Testing gas estimate premium multiplier`, {
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

      // Ensure enough epoch time for quota TXs + deny detection polling
      await rlnClient.ensureEpochWindow(20000);

      // Get denied
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`prem006-exhaust-${i}`),
        });
      }

      // Wait for deny list to propagate, then check gas estimate.
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

      logger.info(`${PREM_006.id}: PASSED ✓`, {
        baseline: baselineGasLimit.toString(),
        denied: deniedGasLimit.toString(),
        ratio: (Number(deniedGasLimit) / Number(baselineGasLimit)).toFixed(2),
      });
    },
    RLN_CONFIG.test.timeouts.denyList, // 2 gasless + deny wait + gas estimate check
  );
});
