/**
 * Test Suite: End-to-End Proof Flow
 *
 * Verifies that a real gasless transaction produces a real RLN proof which is
 * delivered through the prover -> aggregator path. We use the existing
 * RlnTestClient to send the gasless tx (the same machinery the rln-gasless
 * suite uses), then grep both aggregator containers' logs for fresh "Received"
 * lines that appeared after the tx.
 *
 *   PROOF_001: A successful gasless tx produces a proof received by both aggregators.
 *
 * The aggregator's main loop logs every received proof at DEBUG:
 *   aggregator: Received: RlnProofReply { ... }
 * (See rln-aggregator/aggregator/src/main.rs:155.)
 *
 * Each prover broadcast goes to ALL connected GetProofs subscribers via a
 * tokio broadcast channel, so we expect both aggregator-1 AND aggregator-2 to
 * see the new proof, not just one of them. This is the key invariant of the
 * fan-out aggregation layer.
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
import { formatScenario, PROOF_001 } from "./helpers/scenario";

const ETHER = 10n ** 18n;
const logger = createTestLogger();
const docker = new DockerLogMonitor();

describe("RLN End-to-End Proof Flow", () => {
  let rpcProvider: ethers.Provider;
  let sequencerProvider: ethers.Provider;
  let admin: ethers.Wallet;
  let contracts: RlnContracts;
  let rlnClient: RlnTestClient;
  let karmaManager: KarmaTestManager;

  beforeAll(async () => {
    logger.info("=== Proof flow suite ===");
    resetAdminNonceManager();
    rpcProvider = createFastProvider(RLN_CONFIG.services.rpcUrl);
    sequencerProvider = createFastProvider(RLN_CONFIG.services.sequencerUrl);
    admin = new ethers.Wallet(RLN_CONFIG.accounts.admin, rpcProvider);
    contracts = loadRlnContracts(rpcProvider, admin);
    rlnClient = new RlnTestClient(rpcProvider, sequencerProvider, RLN_CONFIG.services.rpcUrl);
    karmaManager = new KarmaTestManager(contracts.karma, contracts.rln, admin, rlnClient);
  }, RLN_CONFIG.test.timeouts.setup);

  it(
    formatScenario(PROOF_001),
    async () => {
      // 1. Create a fresh funded wallet, mint Entry tier karma (1 ETHER), wait
      //    for the prover to register the user. This goes through the existing
      //    rln-gasless infrastructure, no slashing-specific setup needed.
      const user = await createFundedWallet(rpcProvider, admin);
      logger.info(`PROOF_001: Created fresh user ${user.address}`);
      await karmaManager.mintKarma(user.address, 1n * ETHER);
      await karmaManager.waitForRlnRegistration(user.address);
      logger.info(`PROOF_001: User registered in RLN`);

      // 2. Snapshot the current count of "Received:" log lines on both
      //    aggregators. We use --since 1m to keep the grep cheap; aggregator
      //    logs can be large after a long test run.
      const receivedFilter = "Received:";
      const before1 = (
        await docker.getLogs(SLASHING_CONFIG.containers.aggregator1, {
          since: "30s",
          filter: receivedFilter,
        })
      ).length;
      const before2 = (
        await docker.getLogs(SLASHING_CONFIG.containers.aggregator2, {
          since: "30s",
          filter: receivedFilter,
        })
      ).length;
      logger.info(`PROOF_001: Pre-tx received counts: agg1=${before1}, agg2=${before2}`);

      // 3. Send one gasless tx. Entry tier has quota 2; one tx is well within.
      const receipt = await rlnClient.sendGaslessTransaction(user, {
        to: TEST_RECIPIENT,
        value: 0n,
        data: uniqueTxData("proof001"),
      });
      expect(receipt.status).toBe(1);
      logger.info(`PROOF_001: Gasless tx mined: ${receipt.hash}`);

      // 4. Wait for the proof to propagate prover -> aggregators. The prover
      //    broadcasts on a tokio channel synchronously after Groth16 finishes,
      //    so this is typically <500ms but we give it a bounded budget.
      const propagationDeadline = Date.now() + SLASHING_CONFIG.timeouts.proofPropagationMs;
      let after1 = before1;
      let after2 = before2;
      while (Date.now() < propagationDeadline) {
        after1 = (
          await docker.getLogs(SLASHING_CONFIG.containers.aggregator1, {
            since: "1m",
            filter: receivedFilter,
          })
        ).length;
        after2 = (
          await docker.getLogs(SLASHING_CONFIG.containers.aggregator2, {
            since: "1m",
            filter: receivedFilter,
          })
        ).length;
        if (after1 > before1 && after2 > before2) break;
        await sleep(SLASHING_CONFIG.timeouts.pollIntervalMs);
      }
      logger.info(`PROOF_001: Post-tx received counts: agg1=${after1}, agg2=${after2}`);

      // 5. Both aggregators must have observed at least one new proof. The
      //    prover's broadcast channel sends every proof to every subscriber,
      //    so the proof for our tx should appear on both backends — this is
      //    the fan-out invariant we want to verify.
      expect(after1).toBeGreaterThan(before1);
      expect(after2).toBeGreaterThan(before2);
    },
    SLASHING_CONFIG.timeouts.test.proofFlow,
  );
});
