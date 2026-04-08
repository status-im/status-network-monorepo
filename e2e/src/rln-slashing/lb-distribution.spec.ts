/**
 * Test Suite: Envoy gRPC Load Balancer
 *
 * Verifies the Envoy LB in front of rln-aggregator-{1,2}:
 *
 *   LB_001: Envoy admin /ready returns LIVE.
 *   LB_002: Both backends are reported as healthy in the rln_aggregators
 *           cluster.
 *   LB_003: Failover — when one backend goes down, Envoy marks it unhealthy
 *           and the other backend stays healthy and serving. The stopped
 *           backend is restarted at the end of the test and we verify it
 *           rejoins the pool.
 *
 * The failover test (LB_003) uses `docker stop` / `docker start` on the
 * existing aggregator container — it does NOT recreate the container, so the
 * production-mode rln-prover (and any other service) is unaffected.
 */

import { describe, it, expect, beforeAll, afterEach } from "@jest/globals";
import { createTestLogger } from "../config/logger";
import { SLASHING_CONFIG, AGGREGATOR_BACKENDS } from "./config/rln-slashing-config";
import { getEnvoyReadyState, getClusterHealth, waitForClusterState } from "./utils/envoy-admin";
import { dockerStop, dockerStart, waitForContainerHealthy, getContainerState } from "./utils/docker-control";
import { formatScenario, LB_001, LB_002, LB_003 } from "./helpers/scenario";

const logger = createTestLogger();

describe("RLN Aggregator Envoy Load Balancer", () => {
  beforeAll(async () => {
    logger.info("=== Envoy LB suite ===", {
      adminUrl: SLASHING_CONFIG.envoy.adminBaseUrl,
      cluster: SLASHING_CONFIG.envoy.clusterName,
      expectedBackends: AGGREGATOR_BACKENDS,
    });
  });

  // Safety net: if any failover test crashes mid-way, make sure the stopped
  // container is restarted so subsequent tests / suites aren't broken.
  afterEach(async () => {
    for (const container of [SLASHING_CONFIG.containers.aggregator1, SLASHING_CONFIG.containers.aggregator2]) {
      const state = await getContainerState(container);
      if (state && state !== "running") {
        logger.warn(`Safety net: restarting ${container} (was ${state})`);
        await dockerStart(container);
        await waitForContainerHealthy(container, { timeoutMs: SLASHING_CONFIG.timeouts.containerHealthyMs });
      }
    }
  });

  it(
    formatScenario(LB_001),
    async () => {
      const ready = await getEnvoyReadyState();
      logger.info(`Envoy /ready: ${ready}`);
      expect(ready).toBe("LIVE");
    },
    SLASHING_CONFIG.timeouts.test.quick,
  );

  it(
    formatScenario(LB_002),
    async () => {
      const cluster = await getClusterHealth();
      logger.info(`Cluster ${cluster.name}: ${cluster.healthyCount}/${cluster.totalCount} healthy`, {
        backends: cluster.backends.map((b) => ({ address: b.address, healthy: b.isHealthy, hostname: b.hostname })),
      });

      // The rln_aggregators cluster has exactly 2 backends configured in
      // docker/config/envoy/envoy.yaml.
      expect(cluster.totalCount).toBe(2);
      expect(cluster.healthyCount).toBe(2);

      // Cross-check that the backend addresses Envoy resolved match the
      // static IPs we configured in compose. Catches accidental drift between
      // envoy.yaml and the compose IPs.
      const observed = cluster.backends.map((b) => b.address).sort();
      expect(observed).toEqual([...AGGREGATOR_BACKENDS].sort());
    },
    SLASHING_CONFIG.timeouts.test.quick,
  );

  it(
    formatScenario(LB_003),
    async () => {
      // Pre-condition: both backends healthy.
      const before = await getClusterHealth();
      expect(before.healthyCount).toBe(2);

      // Stop rln-aggregator-1.
      const target = SLASHING_CONFIG.containers.aggregator1;
      logger.info(`Stopping ${target} to trigger LB failover`);
      await dockerStop(target);

      try {
        // Envoy's TCP health check fires every 5s with unhealthy_threshold=3,
        // so worst-case ~15s + a poll interval before -1 is marked unhealthy.
        // We wait for "exactly 1 healthy backend, and the unhealthy one is -1".
        const failed = await waitForClusterState(
          (c) => {
            if (c.healthyCount !== 1) return false;
            const unhealthy = c.backends.find((b) => !b.isHealthy);
            return !!unhealthy && unhealthy.address === `${SLASHING_CONFIG.networkIps.aggregator1}:50061`;
          },
          { description: `${target} marked unhealthy` },
        );
        logger.info(`Envoy marked ${target} unhealthy after stop`, {
          healthyCount: failed.healthyCount,
        });

        // The other backend (-2) must still be healthy and serving.
        const surviving = failed.backends.find((b) => b.isHealthy);
        expect(surviving).toBeDefined();
        expect(surviving!.address).toBe(`${SLASHING_CONFIG.networkIps.aggregator2}:50061`);
      } finally {
        // Always restart the stopped container, even if assertions failed.
        logger.info(`Restarting ${target}`);
        await dockerStart(target);
        await waitForContainerHealthy(target, { timeoutMs: SLASHING_CONFIG.timeouts.containerHealthyMs });
      }

      // Verify -1 rejoined the pool as healthy after restart.
      const after = await waitForClusterState((c) => c.healthyCount === 2, {
        description: `${target} rejoined as healthy`,
      });
      expect(after.healthyCount).toBe(2);
      logger.info(`${target} rejoined the LB pool successfully`);
    },
    SLASHING_CONFIG.timeouts.test.failover,
  );
});
