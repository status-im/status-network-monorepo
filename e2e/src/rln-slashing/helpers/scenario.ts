/**
 * Scenario helper for the rln-slashing test suite.
 *
 * Mirrors e2e/src/rln-gasless/helpers/scenario.ts but with categories and IDs
 * specific to the proof aggregation layer + slasher path. Kept separate so the
 * two suites can evolve independently.
 */

export interface Scenario {
  id: string;
  description: string;
  category: ScenarioCategory;
}

export type ScenarioCategory =
  | "AGGREGATOR_SUBSCRIPTION"
  | "LB_HEALTH"
  | "LB_FAILOVER"
  | "PROOF_FLOW"
  | "SLASHER_INTEGRATION";

export function scenario(id: string, description: string, category: ScenarioCategory): Scenario {
  return { id, description, category };
}

export function formatScenario(s: Scenario): string {
  return `[${s.id}] ${s.description}`;
}

// ============================================================================
// AGGREGATOR SUBSCRIPTION (AGG_001 - AGG_002)
// ============================================================================

export const AGG_001 = scenario(
  "AGG_001",
  "Both aggregator instances are subscribed to the prover GetProofs stream",
  "AGGREGATOR_SUBSCRIPTION",
);

export const AGG_002 = scenario(
  "AGG_002",
  "Aggregator gRPC server binds and accepts connections on :50061",
  "AGGREGATOR_SUBSCRIPTION",
);

// ============================================================================
// LB HEALTH AND TOPOLOGY (LB_001 - LB_002)
// ============================================================================

export const LB_001 = scenario("LB_001", "Envoy admin /ready returns LIVE and the gRPC listener is bound", "LB_HEALTH");

export const LB_002 = scenario(
  "LB_002",
  "Envoy reports both rln-aggregator backends as healthy in the rln_aggregators cluster",
  "LB_HEALTH",
);

// ============================================================================
// LB FAILOVER (LB_003)
// ============================================================================

export const LB_003 = scenario(
  "LB_003",
  "When one aggregator backend goes down, Envoy marks it unhealthy and the other backend continues to serve",
  "LB_FAILOVER",
);

// ============================================================================
// END-TO-END PROOF FLOW (PROOF_001)
// ============================================================================

export const PROOF_001 = scenario(
  "PROOF_001",
  "A successful gasless transaction produces a proof that reaches both aggregators",
  "PROOF_FLOW",
);

// ============================================================================
// SLASHER INTEGRATION (SLASHER_001 - SLASHER_002)
// Conditional on RLN_SLASHER_RUNNING=true (slasher is opt-in profile).
// ============================================================================

export const SLASHER_001 = scenario(
  "SLASHER_001",
  "Slasher establishes a gRPC connection through Envoy to exactly one aggregator backend",
  "SLASHER_INTEGRATION",
);

export const SLASHER_002 = scenario(
  "SLASHER_002",
  "Slasher receives proofs through the LB after a gasless transaction is sent",
  "SLASHER_INTEGRATION",
);

// Note: the aggregator's reconnect-loop patch is validated by AGG_002, which
// checks for the patch's marker log line "connected to prover; opening
// GetProofs stream" in each aggregator's startup logs. A separate behavioral
// reconnect test would require restarting the prover, which is unsafe in the
// shared local stack (it can silently regress the prover to mock mode).
