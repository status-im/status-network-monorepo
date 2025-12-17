# RLN Gasless E2E Test Report

**Generated:** 2025-12-17T14:20:59.263Z

## Summary

| Metric | Value |
|--------|-------|
| Total Scenarios | 65 |
| Passed | 64 |
| Failed | 1 |
| Skipped | 0 |
| Not Run | 0 |
| Pass Rate | 98.5% |
| Duration | 1301.8s |

## Category Breakdown

| Category | Total | Passed | Failed | Skipped | Pass Rate |
|----------|-------|--------|--------|---------|-----------|
| DENY_LIST | 9 | 9 | 0 | 0 | 100.0% |
| EDGE_CASE | 5 | 5 | 0 | 0 | 100.0% |
| ERROR_HANDLING | 3 | 3 | 0 | 0 | 100.0% |
| GASLESS | 10 | 9 | 1 | 0 | 90.0% |
| INTEGRATION | 6 | 6 | 0 | 0 | 100.0% |
| KARMA | 8 | 8 | 0 | 0 | 100.0% |
| NULLIFIER | 8 | 8 | 0 | 0 | 100.0% |
| PREMIUM_GAS | 6 | 6 | 0 | 0 | 100.0% |
| RLN_PROOF | 10 | 10 | 0 | 0 | 100.0% |

## ❌ Failed Scenarios

### GAS_006: Quota resets after epoch boundary

- **Category:** GASLESS
- **Test File:** gasless-transactions
- **Error:** `Error: Expected transaction to fail but it succeeded`

## Scenario Details

### DENY_LIST

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| DENY_001 | User exceeding quota is added to deny list | ✅ passed | 14295ms |
| DENY_002 | Denied user cannot send gasless transactions | ✅ passed | 62840ms |
| DENY_003 | Premium gas clears deny status (recovery path) | ✅ passed | 34182ms |
| DENY_004 | Denied user can send premium gas transaction | ✅ passed | 33663ms |
| DENY_005 | Premium gas payment removes user from deny list | ✅ passed | 33505ms |
| DENY_006 | After premium recovery and new epoch, user can send gasless again | ✅ passed | 47096ms |
| DENY_007 | Multiple users can be on deny list simultaneously | ✅ passed | 105305ms |
| DENY_008 | Deny list state is consistent across checks | ✅ passed | 33347ms |
| DENY_009 | Concurrent deny list additions are safe | ✅ passed | 37039ms |

### EDGE_CASE

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| EDGE_001 | Self-transfer gasless transaction allowed | ✅ passed | 963ms |
| EDGE_002 | Empty data gasless transaction allowed | ✅ passed | 979ms |
| EDGE_003 | Minimum gas limit transaction succeeds | ✅ passed | 973ms |
| EDGE_004 | Rapid user creation and registration without conflicts | ✅ passed | 10600ms |
| EDGE_005 | Transaction to contract address allowed | ✅ passed | 1409ms |

### ERROR_HANDLING

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| ERR_001 | Karma service unavailable handled gracefully | ✅ passed | 963ms |
| ERR_002 | RLN prover unavailable timeout handling | ✅ passed | 3165ms |
| ERR_003 | Transaction with large data payload handled | ✅ passed | 1022ms |

### GASLESS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| GAS_001 | Entry tier user can send exactly 2 gasless transactions | ✅ passed | 2060ms |
| GAS_002 | Entry tier user gets rejected on 3rd transaction (quota exceeded) | ✅ passed | 32125ms |
| GAS_003 | User exceeding quota is added to deny list | ✅ passed | 33043ms |
| GAS_004 | Non-Karma user cannot send gasless transactions | ✅ passed | 3652ms |
| GAS_005 | Basic tier user can send 16 gasless transactions | ✅ passed | 18181ms |
| GAS_006 | Quota resets after epoch boundary | ❌ failed | 2976ms |
| GAS_007 | Concurrent transactions maintain user quota isolation | ✅ passed | 32383ms |
| GAS_008 | Different tiers have different quotas | ✅ passed | 36396ms |
| GAS_009 | Transaction without proof times out fast | ✅ passed | 3150ms |
| GAS_010 | Nonce management for sequential gasless transactions | ✅ passed | 3026ms |

### INTEGRATION

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| INT_001 | Complete lifecycle: Register → Gasless → Exhaust → Deny → Premium → Recovery | ✅ passed | 94354ms |
| INT_002 | Multiple users with different tiers operating simultaneously | ✅ passed | 9687ms |
| INT_003 | Rapid sequential transactions handled correctly | ✅ passed | 6051ms |
| INT_004 | Epoch transition with active users handled gracefully | ✅ passed | 12966ms |
| INT_005 | Concurrent transactions don't corrupt state | ✅ passed | 5978ms |
| INT_006 | High volume user quota tracking | ✅ passed | 11866ms |

### KARMA

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| KARMA_001 | Minting 1 Karma assigns Entry tier | ✅ passed | 8362ms |
| KARMA_002 | Minting 50 Karma assigns Basic tier | ✅ passed | 9742ms |
| KARMA_003 | Karma mint triggers automatic RLN registration | ✅ passed | 8745ms |
| KARMA_004 | Additional Karma mint increases available quota | ✅ passed | 10255ms |
| KARMA_005 | All tier levels have correct quota values | ✅ passed | 6ms |
| KARMA_006 | Tier boundary at exact threshold is handled correctly | ✅ passed | 17045ms |
| KARMA_007 | Zero Karma user cannot use gasless | ✅ passed | 4438ms |
| KARMA_008 | Identity commitment is unique per user | ✅ passed | 9033ms |

### NULLIFIER

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| NULL_001 | Each transaction gets unique nullifier | ✅ passed | 3033ms |
| NULL_002 | Same user can transact across different epochs | ✅ passed | 25872ms |
| NULL_003 | Quota exhaustion triggers security event logging | ✅ passed | 12444ms |
| NULL_004 | Replay attack prevention via nonce enforcement | ✅ passed | 1668ms |
| NULL_005 | Epoch validation in proofs | ✅ passed | 963ms |
| NULL_006 | Rapid sequential transactions handled | ✅ passed | 6880ms |
| NULL_007 | Concurrent transactions from multiple users without interference | ✅ passed | 1156ms |
| NULL_008 | Nullifier database persistence verified | ✅ passed | 2249ms |

### PREMIUM_GAS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| PREM_001 | Transaction with gasPrice >= 10 Gwei bypasses RLN | ✅ passed | 1505ms |
| PREM_002 | Transaction with gasPrice < 10 Gwei requires RLN | ✅ passed | 3162ms |
| PREM_003 | Exactly threshold (10 Gwei) bypasses RLN | ✅ passed | 949ms |
| PREM_004 | Premium gas works even without Karma registration | ✅ passed | 2907ms |
| PREM_005 | Premium gas transaction from unfunded wallet fails | ✅ passed | 223ms |
| PREM_006 | Gas estimate shows premium multiplier for denied users | ✅ passed | 32874ms |

### RLN_PROOF

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| RLN_001 | Valid RLN proof is accepted | ✅ passed | 2117ms |
| RLN_002 | Unregistered user gets no proof generated | ✅ passed | 4262ms |
| RLN_003 | Transaction with garbage data still needs valid proof | ✅ passed | 4009ms |
| RLN_004 | Proof arrives before transaction (async handling) | ✅ passed | 1829ms |
| RLN_005 | Transaction times out fast without proof | ✅ passed | 3177ms |
| RLN_006 | Multiple sequential proofs are processed | ✅ passed | 3454ms |
| RLN_007 | gRPC stream resilience maintained across transactions | ✅ passed | 2240ms |
| RLN_008 | Proof rejection events are logged | ✅ passed | 3382ms |
| RLN_009 | Zero-value transactions require proof | ✅ passed | 4453ms |
| RLN_010 | Self-transfer with zero gas requires proof | ✅ passed | 4153ms |
