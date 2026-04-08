/**
 * RLN Slashing Test Configuration
 *
 * Service URLs, container names, and constants for the decentralized
 * slashing path: rln-prover -> rln-aggregator-{1,2} -> envoy LB -> slasher.
 *
 * All values are overridable via env vars so the same suite can run against
 * a different deployment (e.g., a CI environment with different ports).
 */

export const SLASHING_CONFIG = {
  /**
   * Container names (docker compose service names). The tests use these to
   * grep container logs via DockerLogMonitor and to issue docker stop/start
   * for failover tests.
   *
   * NOTE: these MUST match the service names in
   * docker/compose-spec-l2-services-rln.yml.
   */
  containers: {
    prover: process.env.RLN_PROVER_CONTAINER || "rln-prover",
    aggregator1: process.env.RLN_AGGREGATOR_1_CONTAINER || "rln-aggregator-1",
    aggregator2: process.env.RLN_AGGREGATOR_2_CONTAINER || "rln-aggregator-2",
    aggregatorLb: process.env.RLN_AGGREGATOR_LB_CONTAINER || "rln-aggregator-lb",
    slasher: process.env.RLN_SLASHER_CONTAINER || "rln-slasher",
  },

  /**
   * Static IPs for cross-checking against Envoy /clusters output. These come
   * from the linea network in compose-spec-l2-services-rln.yml.
   */
  networkIps: {
    prover: process.env.RLN_PROVER_IP || "11.11.11.120",
    aggregator1: process.env.RLN_AGGREGATOR_1_IP || "11.11.11.123",
    aggregator2: process.env.RLN_AGGREGATOR_2_IP || "11.11.11.124",
    aggregatorLb: process.env.RLN_AGGREGATOR_LB_IP || "11.11.11.122",
    slasher: process.env.RLN_SLASHER_IP || "11.11.11.125",
  },

  /**
   * Host-mapped ports.
   * - aggregator LB serves the gRPC RlnAggregator service on 50061
   * - envoy admin (clusters/stats/ready/listeners) on 9901
   */
  ports: {
    aggregatorLbGrpc: parseInt(process.env.RLN_AGGREGATOR_LB_PORT || "50061", 10),
    envoyAdmin: parseInt(process.env.ENVOY_ADMIN_PORT || "9901", 10),
  },

  /**
   * Envoy admin URL helpers. The admin interface gives us /clusters (upstream
   * health), /stats (counters), /ready (liveness), /listeners (listener state).
   */
  envoy: {
    adminBaseUrl: process.env.ENVOY_ADMIN_URL || "http://localhost:9901",
    /** Cluster name as configured in docker/config/envoy/envoy.yaml */
    clusterName: "rln_aggregators",
  },

  /**
   * Test timing knobs. The slashing path is asynchronous (proof gen → broadcast
   * → aggregator → LB → slasher), so most tests need to poll/wait for an event
   * to propagate. These caps keep waits bounded.
   */
  timeouts: {
    /** Wait for a proof to flow from prover -> aggregator after a tx is sent */
    proofPropagationMs: parseInt(process.env.PROOF_PROPAGATION_MS || "10000", 10),
    /** Wait for Envoy to mark a backend (un)healthy after stop/start */
    envoyHealthFlipMs: parseInt(process.env.ENVOY_HEALTH_FLIP_MS || "30000", 10),
    /** Wait for an aggregator container to become docker-healthy after start */
    containerHealthyMs: parseInt(process.env.CONTAINER_HEALTHY_MS || "60000", 10),
    /** Polling interval for waitFor* helpers */
    pollIntervalMs: 500,
    /** Jest test timeouts (per category) */
    test: {
      quick: 30_000,
      proofFlow: 60_000,
      failover: 180_000,
    },
  },

  /**
   * Whether the slasher container is expected to be running. The slasher is
   * in an opt-in `rln-slashing` profile so it's not started by default. Tests
   * that depend on the slasher being up should be conditional on this flag.
   *
   * Set RLN_SLASHER_RUNNING=true to enable slasher-dependent tests.
   */
  slasherRunning: process.env.RLN_SLASHER_RUNNING === "true",
} as const;

/**
 * Convenience: list of all aggregator backend addresses (host:port) as Envoy
 * sees them in the cluster. Used for cross-checking /clusters output.
 */
export const AGGREGATOR_BACKENDS = [
  `${SLASHING_CONFIG.networkIps.aggregator1}:50061`,
  `${SLASHING_CONFIG.networkIps.aggregator2}:50061`,
] as const;
