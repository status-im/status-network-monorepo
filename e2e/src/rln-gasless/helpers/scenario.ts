/**
 * Scenario helper for explicit test scenario encoding.
 *
 * This ensures:
 * - Stable scenario identifiers
 * - Human-readable descriptions
 * - No coupling to Jest internals
 * - Easy report generation from test results
 */

export interface Scenario {
  id: string;
  description: string;
  category: ScenarioCategory;
}

export type ScenarioCategory =
  | "GASLESS"
  | "KARMA"
  | "DENY_LIST"
  | "PREMIUM_GAS"
  | "INTEGRATION"
  | "ERROR_HANDLING"
  | "EDGE_CASE"
  | "NULLIFIER"
  | "RLN_PROOF";

/**
 * Creates a scenario definition for use in tests.
 *
 * @example
 * ```ts
 * const S = scenario('GAS_001', 'Entry tier user can send exactly 2 gasless transactions', 'GASLESS');
 *
 * describe('Gasless Transactions', () => {
 *   it(`[${S.id}] ${S.description}`, async () => {
 *     // test logic
 *   });
 * });
 * ```
 */
export function scenario(id: string, description: string, category: ScenarioCategory = "GASLESS"): Scenario {
  return { id, description, category };
}

/**
 * Formats a scenario for use in Jest test names.
 * Returns a string like "[GAS_001] Entry tier user can send exactly 2 gasless transactions"
 */
export function formatScenario(s: Scenario): string {
  return `[${s.id}] ${s.description}`;
}

// ============================================================================
// GASLESS TRANSACTION SCENARIOS (GAS_001 - GAS_010)
// ============================================================================

export const GAS_001 = scenario("GAS_001", "Entry tier user can send exactly 2 gasless transactions", "GASLESS");

export const GAS_002 = scenario(
  "GAS_002",
  "Entry tier user gets rejected on 3rd transaction (quota exceeded)",
  "GASLESS",
);

export const GAS_003 = scenario("GAS_003", "User exceeding quota is added to deny list", "GASLESS");

export const GAS_004 = scenario("GAS_004", "Non-Karma user cannot send gasless transactions", "GASLESS");

export const GAS_005 = scenario("GAS_005", "Basic tier user can send 16 gasless transactions", "GASLESS");

export const GAS_006 = scenario("GAS_006", "Quota resets after epoch boundary", "GASLESS");

export const GAS_007 = scenario("GAS_007", "Concurrent transactions maintain user quota isolation", "GASLESS");

export const GAS_008 = scenario("GAS_008", "Different tiers have different quotas", "GASLESS");

export const GAS_009 = scenario("GAS_009", "Transaction without proof times out fast", "GASLESS");

export const GAS_010 = scenario("GAS_010", "Nonce management for sequential gasless transactions", "GASLESS");

// ============================================================================
// KARMA TIER SYSTEM SCENARIOS (KARMA_001 - KARMA_008)
// ============================================================================

export const KARMA_001 = scenario("KARMA_001", "Minting 1 Karma assigns Entry tier", "KARMA");

export const KARMA_002 = scenario("KARMA_002", "Minting 50 Karma assigns Basic tier", "KARMA");

export const KARMA_003 = scenario("KARMA_003", "Karma mint triggers automatic RLN registration", "KARMA");

export const KARMA_004 = scenario("KARMA_004", "Additional Karma mint increases available quota", "KARMA");

export const KARMA_005 = scenario("KARMA_005", "All tier levels have correct quota values", "KARMA");

export const KARMA_006 = scenario("KARMA_006", "Tier boundary at exact threshold is handled correctly", "KARMA");

export const KARMA_007 = scenario("KARMA_007", "Zero Karma user cannot use gasless", "KARMA");

export const KARMA_008 = scenario("KARMA_008", "Identity commitment is unique per user", "KARMA");

// ============================================================================
// DENY LIST SCENARIOS (DENY_001 - DENY_009)
// ============================================================================

export const DENY_001 = scenario("DENY_001", "User exceeding quota is added to deny list", "DENY_LIST");

export const DENY_002 = scenario("DENY_002", "Denied user cannot send gasless transactions", "DENY_LIST");

export const DENY_003 = scenario("DENY_003", "Premium gas instantly clears deny status and resets quota", "DENY_LIST");

export const DENY_004 = scenario("DENY_004", "Denied user can send premium gas transaction", "DENY_LIST");

export const DENY_005 = scenario("DENY_005", "Premium gas payment instantly removes user from deny list", "DENY_LIST");

export const DENY_006 = scenario(
  "DENY_006",
  "After premium gas payment, user can immediately send gasless again (quota reset)",
  "DENY_LIST",
);

export const DENY_007 = scenario("DENY_007", "Multiple users can be on deny list simultaneously", "DENY_LIST");

export const DENY_008 = scenario("DENY_008", "Deny list state is consistent across checks", "DENY_LIST");

export const DENY_009 = scenario("DENY_009", "Concurrent deny list additions are safe", "DENY_LIST");

// ============================================================================
// PREMIUM GAS SCENARIOS (PREM_001 - PREM_006)
// ============================================================================

export const PREM_001 = scenario("PREM_001", "Transaction with gasPrice >= threshold bypasses RLN", "PREMIUM_GAS");

export const PREM_002 = scenario("PREM_002", "Transaction with gasPrice < threshold requires RLN", "PREMIUM_GAS");

export const PREM_003 = scenario("PREM_003", "Exactly at threshold gasPrice bypasses RLN", "PREMIUM_GAS");

export const PREM_004 = scenario("PREM_004", "Premium gas works even without Karma registration", "PREMIUM_GAS");

export const PREM_005 = scenario("PREM_005", "Premium gas transaction from unfunded wallet fails", "PREMIUM_GAS");

export const PREM_006 = scenario("PREM_006", "Gas estimate shows premium multiplier for denied users", "PREMIUM_GAS");

// ============================================================================
// INTEGRATION SCENARIOS (INT_001 - INT_006)
// ============================================================================

export const INT_001 = scenario(
  "INT_001",
  "Complete lifecycle: Register → Gasless → Exhaust → Deny → Premium → Recovery",
  "INTEGRATION",
);

export const INT_002 = scenario(
  "INT_002",
  "Multiple users with different tiers operating simultaneously",
  "INTEGRATION",
);

export const INT_003 = scenario("INT_003", "Rapid sequential transactions handled correctly", "INTEGRATION");

export const INT_004 = scenario("INT_004", "Epoch transition with active users handled gracefully", "INTEGRATION");

export const INT_005 = scenario("INT_005", "Concurrent transactions don't corrupt state", "INTEGRATION");

export const INT_006 = scenario("INT_006", "High volume user quota tracking", "INTEGRATION");

// ============================================================================
// ERROR HANDLING SCENARIOS (ERR_001 - ERR_003)
// ============================================================================

export const ERR_001 = scenario("ERR_001", "Karma service unavailable handled gracefully", "ERROR_HANDLING");

export const ERR_002 = scenario("ERR_002", "RLN prover unavailable timeout handling", "ERROR_HANDLING");

export const ERR_003 = scenario("ERR_003", "Transaction with large data payload handled", "ERROR_HANDLING");

// ============================================================================
// EDGE CASE SCENARIOS (EDGE_001 - EDGE_005)
// ============================================================================

export const EDGE_001 = scenario("EDGE_001", "Self-transfer gasless transaction allowed", "EDGE_CASE");

export const EDGE_002 = scenario("EDGE_002", "Empty data gasless transaction allowed", "EDGE_CASE");

export const EDGE_003 = scenario("EDGE_003", "Minimum gas limit transaction succeeds", "EDGE_CASE");

export const EDGE_004 = scenario("EDGE_004", "Rapid user creation and registration without conflicts", "EDGE_CASE");

export const EDGE_005 = scenario("EDGE_005", "Transaction to contract address allowed", "EDGE_CASE");

// ============================================================================
// NULLIFIER TRACKING SCENARIOS (NULL_001 - NULL_008)
// ============================================================================

export const NULL_001 = scenario("NULL_001", "Each transaction gets unique nullifier", "NULLIFIER");

export const NULL_002 = scenario("NULL_002", "Same user can transact across different epochs", "NULLIFIER");

export const NULL_003 = scenario("NULL_003", "Quota exhaustion triggers security event logging", "NULLIFIER");

export const NULL_004 = scenario("NULL_004", "Replay attack prevention via nonce enforcement", "NULLIFIER");

export const NULL_005 = scenario("NULL_005", "Epoch validation in proofs", "NULLIFIER");

export const NULL_006 = scenario("NULL_006", "Rapid sequential transactions handled", "NULLIFIER");

export const NULL_007 = scenario(
  "NULL_007",
  "Concurrent transactions from multiple users without interference",
  "NULLIFIER",
);

export const NULL_008 = scenario("NULL_008", "Nullifier database persistence verified", "NULLIFIER");

// ============================================================================
// RLN PROOF VERIFICATION SCENARIOS (RLN_001 - RLN_010)
// ============================================================================

export const RLN_001 = scenario("RLN_001", "Valid RLN proof is accepted", "RLN_PROOF");

export const RLN_002 = scenario("RLN_002", "Unregistered user gets no proof generated", "RLN_PROOF");

export const RLN_003 = scenario("RLN_003", "Transaction with garbage data still needs valid proof", "RLN_PROOF");

export const RLN_004 = scenario("RLN_004", "Proof arrives before transaction (async handling)", "RLN_PROOF");

export const RLN_005 = scenario("RLN_005", "Transaction times out fast without proof", "RLN_PROOF");

export const RLN_006 = scenario("RLN_006", "Multiple sequential proofs are processed", "RLN_PROOF");

export const RLN_007 = scenario("RLN_007", "gRPC stream resilience maintained across transactions", "RLN_PROOF");

export const RLN_008 = scenario("RLN_008", "Proof rejection events are logged", "RLN_PROOF");

export const RLN_009 = scenario("RLN_009", "Zero-value transactions require proof", "RLN_PROOF");

export const RLN_010 = scenario("RLN_010", "Self-transfer with zero gas requires proof", "RLN_PROOF");

// ============================================================================
// ALL SCENARIOS EXPORT (for report generation)
// ============================================================================

export const ALL_SCENARIOS: Scenario[] = [
  // Gasless
  GAS_001,
  GAS_002,
  GAS_003,
  GAS_004,
  GAS_005,
  GAS_006,
  GAS_007,
  GAS_008,
  GAS_009,
  GAS_010,
  // Karma
  KARMA_001,
  KARMA_002,
  KARMA_003,
  KARMA_004,
  KARMA_005,
  KARMA_006,
  KARMA_007,
  KARMA_008,
  // Deny List
  DENY_001,
  DENY_002,
  DENY_003,
  DENY_004,
  DENY_005,
  DENY_006,
  DENY_007,
  DENY_008,
  DENY_009,
  // Premium Gas
  PREM_001,
  PREM_002,
  PREM_003,
  PREM_004,
  PREM_005,
  PREM_006,
  // Integration
  INT_001,
  INT_002,
  INT_003,
  INT_004,
  INT_005,
  INT_006,
  // Error Handling
  ERR_001,
  ERR_002,
  ERR_003,
  // Edge Cases
  EDGE_001,
  EDGE_002,
  EDGE_003,
  EDGE_004,
  EDGE_005,
  // Nullifier
  NULL_001,
  NULL_002,
  NULL_003,
  NULL_004,
  NULL_005,
  NULL_006,
  NULL_007,
  NULL_008,
  // RLN Proof
  RLN_001,
  RLN_002,
  RLN_003,
  RLN_004,
  RLN_005,
  RLN_006,
  RLN_007,
  RLN_008,
  RLN_009,
  RLN_010,
];
