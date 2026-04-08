/**
 * Test Suite: Slasher Integration (CONDITIONAL)
 *
 * The rln-slasher container is in an opt-in `rln-slashing` profile in
 * compose-spec-l2-services-rln.yml. It's NOT started by `make
 * start-env-with-rln-production` by default, so most local runs won't have it
 * up. Tests in this file are conditionally enabled via:
 *
 *   RLN_SLASHER_RUNNING=true npx jest --config jest.rln-slashing.config.ts
 *
 * If the env var isn't set, every test in this file is skipped via it.skip().
 *
 *   SLASHER_001: Slasher establishes a gRPC connection through Envoy to
 *                exactly one of the aggregator backends (round-robin pin).
 *   SLASHER_002: Slasher receives proofs through the LB after a gasless tx
 *                is sent.
 *
 * Slasher's debug log line for proof reception (rln-aggregator/slasher/src/proof_process.rs):
 *   slasher: Received proof reply: RlnAggProofReply { ... }
 */

import { ethers } from "ethers";
import { describe, it, expect, beforeAll } from "@jest/globals";
import { RlnTestClient, createFastProvider } from "../rln-gasless/utils/rln-test-client";
import { KarmaTestManager, resetAdminNonceManager } from "../rln-gasless/utils/karma-manager";
import { createFundedWallet, uniqueTxData, TEST_RECIPIENT, sleep } from "../rln-gasless/utils/test-helpers";
import { RLN_CONFIG } from "../rln-gasless/config/rln-config";
import { loadRlnContracts, RlnContracts } from "../rln-gasless/config/contract-loader";
import { DockerLogMonitor } from "../rln-gasless/utils/log-monitor";
import { createTestLogger } from "../config/logger";
import { SLASHING_CONFIG } from "./config/rln-slashing-config";
import { getClusterHealth } from "./utils/envoy-admin";
import { formatScenario, SLASHER_001, SLASHER_002 } from "./helpers/scenario";

const ETHER = 10n ** 18n;
const logger = createTestLogger();
const docker = new DockerLogMonitor();

// Pick the right test runner up-front so the conditional skip is consistent
// for all cases in this file.
const testIfSlasher = SLASHING_CONFIG.slasherRunning ? it : it.skip;

describe("RLN Slasher Integration", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let admin: ethers.Wallet;
  let contracts: RlnContracts;
  let rlnClient: RlnTestClient;
  let karmaManager: KarmaTestManager;

  beforeAll(async () => {
    logger.info("=== Slasher integration suite ===", {
      slasherRunning: SLASHING_CONFIG.slasherRunning,
    });
    if (!SLASHING_CONFIG.slasherRunning) {
      logger.warn(
        "Slasher tests are SKIPPED. Set RLN_SLASHER_RUNNING=true and " +
          "ensure rln-slasher is up via the rln-slashing profile to enable them.",
      );
      return;
    }
    resetAdminNonceManager();
    rpcProvider = createFastProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = createFastProvider(RLN_CONFIG.services.sequencerUrl);
    admin = new ethers.Wallet(RLN_CONFIG.accounts.admin, rpcProvider);
    contracts = loadRlnContracts(rpcProvider, admin);
    rlnClient = new RlnTestClient(rpcProvider, sequencerProvider, RLN_CONFIG.services.rpcUrl);
    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);
  }, RLN_CONFIG.test.timeouts.setup);

  testIfSlasher(
    formatScenario(SLASHER_001),
    async () => {
      // The slasher opens a single long-lived gRPC server-stream against the
      // LB. Envoy's STRICT_DNS + ROUND_ROBIN pins it to exactly one backend
      // for the lifetime of that stream. We verify this by reading
      // /clusters: exactly one backend should have cx_active >= 1, the other
      // should have cx_active == 0.
      const cluster = await getClusterHealth();
      logger.info(`Cluster snapshot`, {
        backends: cluster.backends.map((b) => ({
          address: b.address,
          cxActive: b.cxActive,
          rqActive: b.rqActive,
        })),
      });

      // Both backends still healthy — slasher being there shouldn't change that.
      expect(cluster.healthyCount).toBe(2);

      // Exactly one backend should have an active connection from the slasher.
      // (There may be other transient connections from Envoy's own health
      // checks; cx_active counts long-lived upstream connections.)
      const withConnections = cluster.backends.filter((b) => b.cxActive >= 1);
      expect(withConnections.length).toBeGreaterThanOrEqual(1);

      // And there should be at least one active gRPC request in flight (the
      // slasher's GetProofs server-stream).
      const totalRqActive = cluster.backends.reduce((s, b) => s + b.rqActive, 0);
      expect(totalRqActive).toBeGreaterThanOrEqual(1);

      // Sanity-check the slasher's own logs show it connected to the LB.
      const connectedLines = await docker.getLogs(SLASHING_CONFIG.containers.slasher, {
        filter: `connecting to ${SLASHING_CONFIG.networkIps.aggregatorLb}`,
      });
      expect(connectedLines.length).toBeGreaterThanOrEqual(1);
    },
    SLASHING_CONFIG.timeouts.test.quick,
  );

  testIfSlasher(
    formatScenario(SLASHER_002),
    async () => {
      // Send a fresh gasless tx and verify the slasher's logs show a new
      // "Received proof reply" line for it.
      const user = await createFundedWallet(rpcProvider, admin);
      await karmaManager.mintKarma(user.address, 1n * ETHER);
      await karmaManager.waitForRlnRegistration(user.address);

      const receivedFilter = "Received proof reply";
      const before = (
        await docker.getLogs(SLASHING_CONFIG.containers.slasher, {
          since: "30s",
          filter: receivedFilter,
        })
      ).length;

      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("slasher002"),
      });
      expect(receipt.status).toBe(1);

      // Wait for proof: prover -> aggregator -> Envoy -> slasher.
      const deadline = Date.now() + SLASHING_CONFIG.timeouts.proofPropagationMs;
      let after = before;
      while (Date.now() < deadline) {
        after = (
          await docker.getLogs(SLASHING_CONFIG.containers.slasher, {
            since: "1m",
            filter: receivedFilter,
          })
        ).length;
        if (after > before) break;
        await sleep(SLASHING_CONFIG.timeouts.pollIntervalMs);
      }
      logger.info(`SLASHER_002: slasher proof reception count: before=${before}, after=${after}`);
      expect(after).toBeGreaterThan(before);
    },
    SLASHING_CONFIG.timeouts.test.proofFlow,
  );
});
