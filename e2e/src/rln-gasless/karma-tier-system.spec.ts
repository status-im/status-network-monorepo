import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "./utils/rln-test-client";
import { KarmaTestManager, resetAdminNonceManager } from "./utils/karma-manager";
import { createFundedWallet, uniqueTxData, TEST_RECIPIENT } from "./utils/test-helpers";
import { RLN_CONFIG, TierName } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";
import {
  formatScenario,
  KARMA_001,
  KARMA_002,
  KARMA_003,
  KARMA_004,
  KARMA_005,
  KARMA_006,
  KARMA_007,
  KARMA_008,
} from "./helpers/scenario";

const logger = createTestLogger();

/**
 * Test Suite: Karma and Tier System (KARMA_001 to KARMA_008)
 *
 * Tests Karma minting, tier assignment, and registration:
 * - Tier assignment based on Karma amount
 * - RLN registration triggered by Karma
 * - Tier upgrades and quota increases
 * - Tier boundary conditions
 * - Karma balance verification
 *
 */
describe("RLN Karma and Tier System", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let karmaManager: KarmaTestManager;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  // Pre-funded wallets (without karma) - tests mint karma as needed
  const fundedUsers: ethers.HDNodeWallet[] = [];
  let fundedIdx = 0;

  const getFundedUser = () =>
    fundedUsers[fundedIdx++] ||
    (() => {
      throw new Error("Not enough funded users");
    })();

  // Timeouts based on actual TX performance (~4-5s per gasless TX, P95: 4.7s)
  const TEST_TIMEOUT = 20000;

  beforeAll(async () => {
    logger.info("=== Initializing Karma and Tier System Test Suite ===");

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

    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);

    // PRE-FUND WALLETS for karma tier tests
    logger.info("Pre-funding test wallets...");

    // Fund wallets (no karma) - tests mint karma as needed
    for (let i = 0; i < 10; i++) {
      fundedUsers.push(await createFundedWallet(rpcProvider, admin));
      logger.debug(`Pre-funded user ${i + 1}/10`);
    }

    logger.info("Test suite initialized", {
      fundedUsers: fundedUsers.length,
    });
  }, 180000); // 3 minute setup timeout

  afterAll(async () => {
    logger.info("=== Karma and Tier System Test Suite Complete ===");
  });

  it(
    formatScenario(KARMA_001),
    async () => {
      const user = getFundedUser();

      logger.info(`${KARMA_001.id}: Testing Entry tier assignment`, {
        user: user.address,
      });

      // Verify user has no Karma initially
      let karmaBalance = await contracts.karma.balanceOf(user.address);
      expect(karmaBalance).toBe(0n);

      // Mint exactly 1 Karma (Entry tier threshold)
      await karmaManager.mintKarma(user.address, 1n);

      // Verify Karma balance
      karmaBalance = await contracts.karma.balanceOf(user.address);
      expect(karmaBalance).toBe(1n);

      // Wait for RLN registration
      await karmaManager.waitForRlnRegistration(user.address);

      //  User must be registered
      const isRegistered = await karmaManager.isUserRegistered(user.address);
      expect(isRegistered).toBe(true);

      // Verify user can send gasless transaction (Entry tier = 1 quota)
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("karma001"),
      });

      expect(receipt.status).toBe(1);

      logger.info(`${KARMA_001.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(KARMA_002),
    async () => {
      const user = getFundedUser();

      logger.info(`${KARMA_002.id}: Testing Basic tier assignment`, {
        user: user.address,
      });

      // Mint 50 Karma (Basic tier threshold)
      await karmaManager.mintKarma(user.address, 50n);

      // Verify Karma balance
      const karmaBalance = await contracts.karma.balanceOf(user.address);
      expect(karmaBalance).toBe(50n);

      // Wait for RLN registration
      await karmaManager.waitForRlnRegistration(user.address);

      // Verify tier via transaction capability
      // Basic tier has quota of 15 - send 2 to prove higher than Entry (1)
      for (let i = 0; i < 2; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`karma002-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      logger.info(`${KARMA_002.id}: PASSED ✓`);
    },
    30000, // Extended timeout: mint + wait + 2 TXs
  );

  it(
    formatScenario(KARMA_003),
    async () => {
      const user = getFundedUser();

      logger.info(`${KARMA_003.id}: Testing automatic RLN registration`, {
        user: user.address,
      });

      //  User is NOT registered before Karma mint
      let isRegistered = await karmaManager.isUserRegistered(user.address);
      expect(isRegistered).toBe(false);

      // Mint Karma
      await karmaManager.mintKarma(user.address, 1n);

      // Wait for automatic registration
      await karmaManager.waitForRlnRegistration(user.address);

      //  User IS now registered
      isRegistered = await karmaManager.isUserRegistered(user.address);
      expect(isRegistered).toBe(true);

      // Verify identity commitment exists
      const commitment = await karmaManager.getUserIdentityCommitment(user.address);
      expect(commitment).not.toBeNull();
      expect(commitment).not.toBe(ethers.ZeroHash);

      logger.info(`${KARMA_003.id}: PASSED ✓`, {
        identityCommitment: commitment,
      });
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(KARMA_004),
    async () => {
      const user = getFundedUser();

      logger.info(`${KARMA_004.id}: Testing Karma increase effect`, {
        user: user.address,
      });

      // Start with Entry tier (1 Karma, quota = 1)
      await karmaManager.mintKarma(user.address, 1n);
      await karmaManager.waitForRlnRegistration(user.address);

      // Immediately upgrade to Newbie tier BEFORE using quota
      // This tests that tier upgrade increases quota from 1 to 5
      await karmaManager.mintKarma(user.address, 1n);

      // Verify total Karma (2 = Newbie tier)
      const karmaBalance = await contracts.karma.balanceOf(user.address);
      expect(karmaBalance).toBe(2n);

      // Now send 5 transactions (Newbie tier quota)
      // This proves the tier upgrade from Entry (1) to Newbie (5) worked
      for (let i = 0; i < 5; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`karma004-newbie-${i}`),
        });
        expect(receipt.status).toBe(1);
      }

      logger.info(`${KARMA_004.id}: PASSED ✓`);
    },
    40000, // Extended timeout: mint + wait + 5 transactions
  );

  it(
    formatScenario(KARMA_005),
    async () => {
      logger.info(`${KARMA_005.id}: Verifying all tier configurations`);

      const tiers = RLN_CONFIG.tiers;
      const tierNames = Object.keys(tiers) as TierName[];

      // STRONG ASSERTIONS: Verify exact tier configuration
      expect(tiers.entry.quota).toBe(1);
      expect(tiers.newbie.quota).toBe(5);
      expect(tiers.basic.quota).toBe(15);
      expect(tiers.active.quota).toBe(96);
      expect(tiers.regular.quota).toBe(480);
      expect(tiers.power.quota).toBe(960);
      expect(tiers.pro.quota).toBe(10080);
      expect(tiers["high-throughput"].quota).toBe(108000);
      expect(tiers["s-tier"].quota).toBe(240000);
      expect(tiers.legendary.quota).toBe(480000);

      // Verify karma thresholds are ordered correctly
      expect(tiers.entry.karma).toBe(1n);
      expect(tiers.newbie.karma).toBe(2n);
      expect(tiers.basic.karma).toBe(50n);
      expect(tiers.active.karma).toBe(500n);
      expect(tiers.regular.karma).toBe(5000n);
      expect(tiers.power.karma).toBe(20000n);
      expect(tiers.pro.karma).toBe(100000n);
      expect(tiers["high-throughput"].karma).toBe(500000n);
      expect(tiers["s-tier"].karma).toBe(5000000n);
      expect(tiers.legendary.karma).toBe(10000000n);

      // Verify quotas are monotonically increasing with karma
      let prevQuota = 0;
      for (const tierName of tierNames) {
        expect(tiers[tierName].quota).toBeGreaterThan(prevQuota);
        prevQuota = tiers[tierName].quota;
      }

      logger.info(`${KARMA_005.id}: PASSED ✓`, {
        tierCount: tierNames.length,
      });
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(KARMA_006),
    async () => {
      logger.info(`${KARMA_006.id}: Testing tier boundary handling`);

      // Test exact boundary at Entry (1 Karma)
      const user1 = getFundedUser();
      await karmaManager.mintKarma(user1.address, 1n);
      await karmaManager.waitForRlnRegistration(user1.address);

      const balance1 = await contracts.karma.balanceOf(user1.address);
      expect(balance1).toBe(1n);

      // Entry tier has quota 1 - send 1 tx (uses entire quota, user added to deny list)
      const receipt1 = await rlnClient.sendGaslessTransaction(user1, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("karma006-entry-1"),
      });
      expect(receipt1.status).toBe(1);

      // Wait for prover to sync quota state
      await rlnClient.waitForProverSync();

      // 2nd tx should fail (quota exhausted, user now on deny list)
      const entryError = await rlnClient.sendGaslessTransactionExpectFailure(
        user1,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("karma006-entry-exceed"),
        },
        10000, // 10s timeout
      );
      expect(entryError).toMatch(/quota|exceeded|denied|timeout/i);

      logger.info(`${KARMA_006.id}: PASSED ✓`);
    },
    30000, // Extended timeout: mint + wait + tx + prover sync + failure tx
  );

  it(
    formatScenario(KARMA_007),
    async () => {
      const user = getFundedUser();

      logger.info(`${KARMA_007.id}: Testing zero Karma rejection`, {
        user: user.address,
      });

      // Verify user has zero Karma
      const karmaBalance = await contracts.karma.balanceOf(user.address);
      expect(karmaBalance).toBe(0n);

      // User should NOT be registered (no Karma = no RLN registration)
      const isRegistered = await karmaManager.isUserRegistered(user.address);
      expect(isRegistered).toBe(false);

      // Gasless transaction should fail (no proof generated)
      const errorMessage = await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("karma007"),
        },
        RLN_CONFIG.test.proofTimeoutMs,
      );

      //  Must be rejected
      expect(errorMessage).toMatch(/timeout|rejected|proof|invalid|not registered/i);

      logger.info(`${KARMA_007.id}: PASSED ✓`);
    },
    TEST_TIMEOUT,
  );

  it(
    formatScenario(KARMA_008),
    async () => {
      logger.info(`${KARMA_008.id}: Testing identity commitment uniqueness`);

      // Create and register two users
      const user1 = getFundedUser();
      const user2 = getFundedUser();

      await karmaManager.mintKarma(user1.address, 1n);
      await karmaManager.mintKarma(user2.address, 1n);

      await karmaManager.waitForRlnRegistration(user1.address);
      await karmaManager.waitForRlnRegistration(user2.address);

      // Get identity commitments
      const commitment1 = await karmaManager.getUserIdentityCommitment(user1.address);
      const commitment2 = await karmaManager.getUserIdentityCommitment(user2.address);

      // STRONG ASSERTIONS: Both must have commitments and they must be different
      expect(commitment1).not.toBeNull();
      expect(commitment2).not.toBeNull();
      expect(commitment1).not.toBe(commitment2);

      logger.info(`${KARMA_008.id}: PASSED ✓`, {
        user1: user1.address,
        user2: user2.address,
        commitment1,
        commitment2,
      });
    },
    TEST_TIMEOUT,
  );
});
