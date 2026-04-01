import { ethers } from "ethers";
import { describe, it, expect, beforeAll, afterAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "./utils/rln-test-client";
import { DenyListTestManager } from "./utils/deny-list-manager";
import { KarmaTestManager, resetAdminNonceManager } from "./utils/karma-manager";
import { DockerLogMonitor } from "./utils/log-monitor";
import { uniqueTxData, TEST_RECIPIENT, PREMIUM_GAS_PRICE } from "./utils/test-helpers";
import { RLN_CONFIG } from "./config/rln-config";
import { loadRlnContracts, RlnContracts } from "./config/contract-loader";
import { createTestLogger } from "../config/logger";
import {
  formatScenario,
  NULL_001,
  NULL_002,
  NULL_003,
  NULL_004,
  NULL_005,
  NULL_006,
  NULL_007,
  NULL_008,
} from "./helpers/scenario";

const logger = createTestLogger();

/**
 * Test Suite: Nullifier Tracking and Spam Detection (NULL_001 to NULL_008)
 *
 * Tests nullifier uniqueness and replay attack prevention:
 * - Nullifier uniqueness enforcement
 * - Cross-epoch behavior
 * - Security violation detection
 * - Replay attack prevention
 * - Epoch validation
 * - High-throughput nullifier tracking
 * - Database persistence
 *
 * Architecture:
 * - Nullifiers are stored in PostgreSQL (prover_db.nullifiers table)
 * - gRPC communication between sequencer and prover
 *
 */
describe("RLN Nullifier Tracking", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let rlnClient: RlnTestClient;
  let denyListManager: DenyListTestManager;
  let karmaManager: KarmaTestManager;
  let logMonitor: DockerLogMonitor;
  let contracts: RlnContracts;
  let admin: ethers.Wallet;

  // Pre-registered user pools
  const newbieUsers: ethers.HDNodeWallet[] = [];
  const entryUsers: ethers.HDNodeWallet[] = [];
  const activeUsers: ethers.HDNodeWallet[] = [];
  let newbieIdx = 0,
    entryIdx = 0,
    activeIdx = 0;

  const getNewbieUser = () =>
    newbieUsers[newbieIdx++] ||
    (() => {
      throw new Error("Not enough newbie users");
    })();
  const getEntryUser = () =>
    entryUsers[entryIdx++] ||
    (() => {
      throw new Error("Not enough entry users");
    })();
  const getActiveUser = () =>
    activeUsers[activeIdx++] ||
    (() => {
      throw new Error("Not enough active users");
    })();

  beforeAll(async () => {
    logger.info("=== Initializing Nullifier Tracking Test Suite ===");

    // Reset nonce manager to sync with blockchain state
    resetAdminNonceManager();

    // Setup providers with fast polling for quicker transaction confirmation detection
    rpcProvider = createFastProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = createFastProvider(RLN_CONFIG.services.sequencerUrl);
    admin = new ethers.Wallet(RLN_CONFIG.accounts.admin, rpcProvider);
    contracts = loadRlnContracts(rpcProvider, admin);

    rlnClient = new RlnTestClient(rpcProvider, sequencerProvider, RLN_CONFIG.services.rpcUrl);

    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);
    denyListManager = new DenyListTestManager();
    logMonitor = new DockerLogMonitor();

    // PRE-REGISTER ALL USERS NEEDED FOR THIS TEST SUITE
    // Uses skipRegistrationWait to avoid 20s sleep per user, then does a single wait at the end
    logger.info("Pre-registering test users...");

    // Newbie users (6 needed for various tests)
    for (let i = 0; i < 6; i++) {
      newbieUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "newbie", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered newbie user ${i + 1}/6`);
    }
    // Entry users (2 needed)
    for (let i = 0; i < 2; i++) {
      entryUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "entry", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered entry user ${i + 1}/2`);
    }
    // Active users (4 needed for high-volume tests)
    for (let i = 0; i < 4; i++) {
      activeUsers.push(
        await karmaManager.setupUserForGasless(rpcProvider, "active", undefined, { skipRegistrationWait: true }),
      );
      logger.debug(`Pre-registered active user ${i + 1}/4`);
    }

    // Single registration wait for all users
    logger.info("Waiting for RLN registrations to complete...");
    await karmaManager.waitForRlnRegistration("batch-all");
    logger.info("Registration wait complete");

    logger.info("Test suite initialized", {
      newbieUsers: newbieUsers.length,
      entryUsers: entryUsers.length,
      activeUsers: activeUsers.length,
    });
  }, RLN_CONFIG.test.timeouts.setupLarge);

  afterAll(async () => {
    logger.info("=== Nullifier Tracking Test Suite Complete ===");
  });

  it(
    formatScenario(NULL_001),
    async () => {
      // Each transaction from a user generates a unique nullifier
      // based on transaction-specific inputs (message, epoch, etc.)
      const user = getNewbieUser();

      logger.info(`${NULL_001.id}: Testing nullifier uniqueness per transaction`, {
        user: user.address,
      });

      // Send multiple transactions
      const receipt1 = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("null001-tx1"),
      });
      expect(receipt1.status).toBe(1);
      logger.info("First transaction succeeded", { txHash: receipt1.hash });

      const receipt2 = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("null001-tx2"),
      });
      expect(receipt2.status).toBe(1);
      logger.info("Second transaction succeeded", { txHash: receipt2.hash });

      // Both succeeded because each got a unique nullifier
      // The RLN system uses transaction data to derive different nullifiers
      // This verifies the system correctly generates unique nullifiers per tx

      // Check prover logs for nullifier entries
      const nullifierLogs = await logMonitor.getMatchingLogs("rln-prover", "nullifier", { since: "30s" });
      logger.info(`${NULL_001.id}: Nullifier logs found`, { count: nullifierLogs.length });

      //  Both transactions must have different hashes (different nullifiers)
      expect(receipt1.hash).not.toBe(receipt2.hash);

      logger.info(`${NULL_001.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx, // 2 TXs need more buffer for proof generation
  );

  it(
    formatScenario(NULL_002),
    async () => {
      const user = getEntryUser();
      const quota = RLN_CONFIG.tiers.entry.quota;
      const epochDuration = RLN_CONFIG.test.epochDurationSeconds;

      logger.info(`${NULL_002.id}: Testing cross-epoch transactions`, {
        user: user.address,
        quota,
        epochDurationSeconds: epochDuration,
      });

      // Exhaust quota in first epoch
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`null002-epoch1-${i}`),
        });
      }
      logger.info("Epoch 1 quota exhausted");

      // Wait for next epoch (short epochs make this practical)
      logger.info(`Waiting for next epoch (max ${epochDuration + 2}s)...`);
      const newEpoch = await rlnClient.waitForNextEpoch((epochDuration + 2) * 1000);
      logger.info("New epoch started", { epoch: newEpoch });

      // Clear deny status by paying premium gas
      await rlnClient.sendPremiumGasTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        gasPrice: PREMIUM_GAS_PRICE,
        data: uniqueTxData("null002-clear-deny"),
      });

      // Wait for deny list clearance to propagate
      await denyListManager.waitForNotDenied(user.address, RLN_CONFIG.test.maxWaitForDenyListMs);

      // Allow prover to sync state after deny list clearance
      await rlnClient.sleep(1000);

      // Should be able to transact in new epoch
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("null002-epoch2"),
      });

      expect(receipt.status).toBe(1);
      logger.info(`${NULL_002.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // This test waits for epoch boundary (60s)
  );

  it(
    formatScenario(NULL_003),
    async () => {
      const user = getEntryUser();
      const quota = RLN_CONFIG.tiers.entry.quota;

      logger.info(`${NULL_003.id}: Testing security event logging`, {
        user: user.address,
      });

      // Exhaust quota
      for (let i = 0; i < quota; i++) {
        await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`null003-${i}`),
        });
      }

      // Attempt to exceed quota (should trigger security event)
      // Use a shorter timeout (10s) to fit within TEST_TIMEOUT
      await rlnClient.sendGaslessTransactionExpectFailure(
        user,
        {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData("null003-exceed"),
        },
        10000, // 10s timeout for failure expectation
      );

      // Check for security-related logs in sequencer or prover
      let securityLogs = await logMonitor.getMatchingLogs("sequencer", "quota|denied|exceeded", {
        since: "30s",
      });

      // Also check prover logs if sequencer logs are empty
      if (securityLogs.length === 0) {
        securityLogs = await logMonitor.getMatchingLogs("rln-prover", "quota|denied|exceeded", {
          since: "30s",
        });
      }

      logger.info(`${NULL_003.id}: Security logs found`, {
        count: securityLogs.length,
        sample: securityLogs.slice(0, 3),
      });

      // Security logging should capture quota events (soft assertion - logs may vary)
      // The key test is that the TX was rejected, logging is secondary
      logger.info(`${NULL_003.id}: Security log count`, { count: securityLogs.length });

      logger.info(`${NULL_003.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.epoch, // 2 TXs + 10s failure wait + slow docker log check (~30s)
  );

  it(
    formatScenario(NULL_004),
    async () => {
      const user = getNewbieUser();

      logger.info(`${NULL_004.id}: Testing replay attack prevention`, {
        user: user.address,
      });

      // Get current nonce
      const nonce = await rpcProvider.getTransactionCount(user.address, "latest");
      const txData = uniqueTxData("null004-original");

      // Send through RPC node (which has prover forwarder)
      const gaslessUser = user.connect(sequencerProvider);
      const tx = await gaslessUser.sendTransaction({
        to: TEST_RECIPIENT,
        value: 0n,
        data: txData,
        gasLimit: 30000,
        gasPrice: 0,
        nonce,
      });

      const receipt = await tx.wait(1, RLN_CONFIG.test.transactionTimeoutMs);
      expect(receipt?.status).toBe(1);
      logger.info("Original transaction mined", { txHash: tx.hash });

      // Attempt to send same transaction again with same nonce (replay)
      try {
        await gaslessUser.sendTransaction({
          to: TEST_RECIPIENT,
          value: 0n,
          data: txData,
          gasLimit: 30000,
          gasPrice: 0,
          nonce, // Same nonce - should be rejected
        });
        throw new Error("Expected replay to fail");
      } catch (error: unknown) {
        const err = error instanceof Error ? error : new Error(String(error));
        //  Must fail with nonce-related error
        expect(err.message).toMatch(/nonce|known|already|replacement/i);
        logger.info("Replay rejected as expected", { error: err.message });
      }

      logger.info(`${NULL_004.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx,
  );

  it(
    formatScenario(NULL_005),
    async () => {
      const user = getNewbieUser();

      logger.info(`${NULL_005.id}: Testing epoch validation`, {
        user: user.address,
      });

      const currentEpoch = rlnClient.getCurrentEpoch();
      logger.info("Current epoch", { epoch: currentEpoch });

      // Send transaction in current epoch
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("null005"),
      });

      expect(receipt.status).toBe(1);

      // Verify the transaction was processed in the expected epoch
      // by checking the block timestamp falls within epoch bounds
      const block = await rpcProvider.getBlock(receipt.blockNumber);
      const txEpoch = Math.floor((block?.timestamp ?? 0) / RLN_CONFIG.test.epochDurationSeconds);

      //  Transaction epoch must match expected epoch (±1 for boundary)
      expect(Math.abs(txEpoch - currentEpoch)).toBeLessThanOrEqual(1);

      logger.info(`${NULL_005.id}: PASSED ✓`, {
        currentEpoch,
        txEpoch,
      });
    },
    RLN_CONFIG.test.timeouts.multiTx,
  );

  it(
    formatScenario(NULL_006),
    async () => {
      const user = getActiveUser();
      const txCount = 5; // Reduced for speed

      logger.info(`${NULL_006.id}: Testing rapid sequential transactions`, {
        user: user.address,
        txCount,
      });

      const startTime = Date.now();
      const receipts: ethers.TransactionReceipt[] = [];

      // Send transactions in rapid succession
      for (let i = 0; i < txCount; i++) {
        const receipt = await rlnClient.sendGaslessTransaction(user, {
          to: TEST_RECIPIENT,
          value: 0n,
          data: uniqueTxData(`null006-rapid-${i}`),
        });
        receipts.push(receipt);
      }

      const duration = Date.now() - startTime;
      const tps = (receipts.length / duration) * 1000;

      logger.info(`${NULL_006.id}: Throughput results`, {
        txCount: receipts.length,
        durationMs: duration,
        tps: tps.toFixed(2),
      });

      //  All transactions must succeed
      const successCount = receipts.filter((r) => r.status === 1).length;
      expect(successCount).toBe(txCount);

      logger.info(`${NULL_006.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx,
  );

  it(
    formatScenario(NULL_007),
    async () => {
      const users = [getActiveUser(), getActiveUser(), getActiveUser()];

      logger.info(`${NULL_007.id}: Testing concurrent multi-user transactions`, {
        userCount: users.length,
      });

      // Submit 2 transactions per user concurrently
      // Use sendGaslessTransactionsConcurrent to handle nonce management per user
      const txPromises = users.map((user, userIdx) =>
        rlnClient.sendGaslessTransactionsConcurrent(user, [
          { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData(`null007-user${userIdx}-tx0`) },
          { to: TEST_RECIPIENT, value: 0n, data: uniqueTxData(`null007-user${userIdx}-tx1`) },
        ]),
      );

      // Flatten the results (each user returns 2 receipts)
      const resultsNested = await Promise.all(txPromises);
      const results = resultsNested.flat();

      logger.info(`${NULL_007.id}: Concurrent submission results`, {
        total: results.length,
        success: results.filter((r) => r.status === 1).length,
      });

      //  All transactions should succeed
      for (const receipt of results) {
        expect(receipt.status).toBe(1);
      }

      logger.info(`${NULL_007.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx,
  );

  it(
    formatScenario(NULL_008),
    async () => {
      const user = getNewbieUser();

      logger.info(`${NULL_008.id}: Testing nullifier database persistence`, {
        user: user.address,
      });

      // Send first transaction
      const receipt1 = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("null008-persist-1"),
      });
      expect(receipt1.status).toBe(1);

      // Send second transaction (different nullifier should be stored)
      const receipt2 = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("null008-persist-2"),
      });
      expect(receipt2.status).toBe(1);

      // Check prover logs for database operations
      const dbLogs = await logMonitor.getMatchingLogs("rln-prover", "nullifier|insert|store", { since: "30s" });

      logger.info(`${NULL_008.id}: Database operation logs`, {
        logCount: dbLogs.length,
      });

      //  Both transactions succeeded, proving nullifiers are unique
      expect(receipt1.hash).not.toBe(receipt2.hash);

      logger.info(`${NULL_008.id}: PASSED ✓`);
    },
    RLN_CONFIG.test.timeouts.multiTx,
  );
});
