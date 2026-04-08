/**
 * Test Suite: Aggregator <-> Prover Subscription
 *
 * Verifies that both rln-aggregator instances successfully subscribed to the
 * rln-prover's GetProofs gRPC stream at startup.
 *
 * Mechanism: when an aggregator opens a GetProofs subscription, the prover
 * logs an INFO line:
 *
 *   prover::grpc_service: [gRPC] New get_proofs subscription, total subscribers: N
 *
 * The aggregator uses the tonic Rust gRPC client (user-agent "tonic/..."),
 * vs Besu Java clients which show "grpc-java-netty/...". We count tonic
 * subscriptions in the prover logs and assert there are at least 2 (one per
 * aggregator instance).
 *
 * We also verify each aggregator's own startup log shows it bound its gRPC
 * server on :50061 and successfully connected to the prover (this is the
 * `connected to prover; opening GetProofs stream` line introduced by the
 * reconnect-loop patch in rln-aggregator/aggregator/src/main.rs).
 */

import { describe, it, expect, beforeAll } from "@jest/globals";
import { DockerLogMonitor } from "../rln-gasless/utils/log-monitor";
import { createTestLogger } from "../config/logger";
import { SLASHING_CONFIG } from "./config/rln-slashing-config";
import { formatScenario, AGG_001, AGG_002 } from "./helpers/scenario";

const logger = createTestLogger();
const docker = new DockerLogMonitor();

describe("RLN Aggregator <-> Prover Subscription", () => {
  beforeAll(() => {
    logger.info("=== Aggregator subscription suite ===", {
      prover: SLASHING_CONFIG.containers.prover,
      aggregator1: SLASHING_CONFIG.containers.aggregator1,
      aggregator2: SLASHING_CONFIG.containers.aggregator2,
    });
  });

  it(
    formatScenario(AGG_001),
    async () => {
      // Pull all prover logs since startup and look for "New get_proofs
      // subscription" lines from tonic clients.
      //
      // Tonic vs grpc-java-netty user-agent disambiguates aggregators (Rust)
      // from Besu nodes (Java). Both kinds open GetProofs subscriptions; we
      // only care about the aggregator (tonic) ones here.
      const lines = await docker.getLogs(SLASHING_CONFIG.containers.prover, {
        filter: "New get_proofs subscription",
      });

      logger.debug(`Found ${lines.length} GetProofs subscription log lines on prover`);

      const tonicSubscriptions = lines.filter((l) => l.includes("tonic/"));
      logger.info(`Tonic GetProofs subscriptions on prover: ${tonicSubscriptions.length}`);

      // We expect AT LEAST 2 tonic subscriptions across the prover's lifetime
      // (one per aggregator). The exact count may be higher if either
      // aggregator was restarted by docker (each restart creates a new
      // subscription).
      expect(tonicSubscriptions.length).toBeGreaterThanOrEqual(2);
    },
    SLASHING_CONFIG.timeouts.test.quick,
  );

  it(
    formatScenario(AGG_002),
    async () => {
      // Each aggregator must have logged "Listening on 0.0.0.0:50061" at
      // startup AND "connected to prover; opening GetProofs stream" (the new
      // log line from the reconnect-loop patch).
      for (const container of [SLASHING_CONFIG.containers.aggregator1, SLASHING_CONFIG.containers.aggregator2]) {
        const listening = await docker.getLogs(container, { filter: "Listening on 0.0.0.0:50061" });
        expect(listening.length).toBeGreaterThanOrEqual(1);

        const connected = await docker.getLogs(container, {
          filter: "connected to prover; opening GetProofs stream",
        });
        // The reconnect-loop patch logs this line on every successful (re)connect.
        // At least one occurrence proves the patch is in the running binary AND
        // the initial connect to the prover succeeded.
        expect(connected.length).toBeGreaterThanOrEqual(1);

        logger.info(`${container}: listening=${listening.length}, connected=${connected.length}`);
      }
    },
    SLASHING_CONFIG.timeouts.test.quick,
  );
});
