# RLN Gasless E2E Test Report

**Generated:** 2026-01-29T12:05:12.613Z

## Summary

| Metric | Value |
|--------|-------|
| Total Scenarios | 65 |
| Passed | 64 |
| Failed | 1 |
| Skipped | 0 |
| Not Run | 0 |
| Pass Rate | 98.5% |
| Duration | 1570.6s |

## Category Breakdown

| Category | Total | Passed | Failed | Skipped | Pass Rate |
|----------|-------|--------|--------|---------|-----------|
| DENY_LIST | 9 | 9 | 0 | 0 | 100.0% |
| EDGE_CASE | 5 | 5 | 0 | 0 | 100.0% |
| ERROR_HANDLING | 3 | 3 | 0 | 0 | 100.0% |
| GASLESS | 10 | 10 | 0 | 0 | 100.0% |
| INTEGRATION | 6 | 6 | 0 | 0 | 100.0% |
| KARMA | 8 | 7 | 1 | 0 | 87.5% |
| NULLIFIER | 8 | 8 | 0 | 0 | 100.0% |
| PREMIUM_GAS | 6 | 6 | 0 | 0 | 100.0% |
| RLN_PROOF | 10 | 10 | 0 | 0 | 100.0% |

## ❌ Failed Scenarios

### KARMA_002: Minting 50 Karma assigns Basic tier

- **Category:** KARMA
- **Test File:** karma-tier-system
- **Error:** `Error: thrown: "Exceeded timeout of 20000 ms for a test.`

## Scenario Details

### DENY_LIST

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| DENY_001 | User exceeding quota is added to deny list | ✅ passed | 2243ms |
| DENY_002 | Denied user cannot send gasless transactions | ✅ passed | 34019ms |
| DENY_003 | Premium gas clears deny status (recovery path) | ✅ passed | 4413ms |
| DENY_004 | Denied user can send premium gas transaction | ✅ passed | 7913ms |
| DENY_005 | Premium gas payment removes user from deny list | ✅ passed | 9668ms |
| DENY_006 | After premium recovery and new epoch, user can send gasless again | ✅ passed | 23764ms |
| DENY_007 | Multiple users can be on deny list simultaneously | ✅ passed | 10994ms |
| DENY_008 | Deny list state is consistent across checks | ✅ passed | 3790ms |
| DENY_009 | Concurrent deny list additions are safe | ✅ passed | 9299ms |

### EDGE_CASE

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| EDGE_001 | Self-transfer gasless transaction allowed | ✅ passed | 3944ms |
| EDGE_002 | Empty data gasless transaction allowed | ✅ passed | 1642ms |
| EDGE_003 | Minimum gas limit transaction succeeds | ✅ passed | 2136ms |
| EDGE_004 | Rapid user creation and registration without conflicts | ✅ passed | 17000ms |
| EDGE_005 | Transaction to contract address allowed | ✅ passed | 3504ms |

### ERROR_HANDLING

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| ERR_001 | Karma service unavailable handled gracefully | ✅ passed | 3098ms |
| ERR_002 | RLN prover unavailable timeout handling | ✅ passed | 5609ms |
| ERR_003 | Transaction with large data payload handled | ✅ passed | 2884ms |

### GASLESS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| GAS_001 | Entry tier user can send exactly 1 gasless transaction | ✅ passed | 1386ms |
| GAS_002 | Entry tier user gets rejected on 2nd transaction (quota exceeded) | ✅ passed | 33300ms |
| GAS_003 | User exceeding quota is added to deny list | ✅ passed | 13959ms |
| GAS_004 | Non-Karma user cannot send gasless transactions | ✅ passed | 6811ms |
| GAS_005 | Basic tier user can send 15 gasless transactions | ✅ passed | 30082ms |
| GAS_006 | Quota resets after epoch boundary | ✅ passed | 38116ms |
| GAS_007 | Concurrent transactions maintain user quota isolation | ✅ passed | 34195ms |
| GAS_008 | Different tiers have different quotas | ✅ passed | 40801ms |
| GAS_009 | Transaction without proof times out fast | ✅ passed | 5216ms |
| GAS_010 | Nonce management for sequential gasless transactions | ✅ passed | 6684ms |

### INTEGRATION

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| INT_001 | Complete lifecycle: Register → Gasless → Exhaust → Deny → Premium → Recovery | ✅ passed | 82174ms |
| INT_002 | Multiple users with different tiers operating simultaneously | ✅ passed | 16550ms |
| INT_003 | Rapid sequential transactions handled correctly | ✅ passed | 13144ms |
| INT_004 | Epoch transition with active users handled gracefully | ✅ passed | 28131ms |
| INT_005 | Concurrent transactions don't corrupt state | ✅ passed | 12490ms |
| INT_006 | High volume user quota tracking | ✅ passed | 24304ms |

### KARMA

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| KARMA_001 | Minting 1 Karma assigns Entry tier | ✅ passed | 13376ms |
| KARMA_002 | Minting 50 Karma assigns Basic tier | ❌ failed | 20004ms |
| KARMA_003 | Karma mint triggers automatic RLN registration | ✅ passed | 11611ms |
| KARMA_004 | Additional Karma mint increases available quota | ✅ passed | 26357ms |
| KARMA_005 | All tier levels have correct quota values | ✅ passed | 24ms |
| KARMA_006 | Tier boundary at exact threshold is handled correctly | ✅ passed | 21346ms |
| KARMA_007 | Zero Karma user cannot use gasless | ✅ passed | 8591ms |
| KARMA_008 | Identity commitment is unique per user | ✅ passed | 19611ms |

### NULLIFIER

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| NULL_001 | Each transaction gets unique nullifier | ✅ passed | 6325ms |
| NULL_002 | Same user can transact across different epochs | ✅ passed | 19884ms |
| NULL_003 | Quota exhaustion triggers security event logging | ✅ passed | 19275ms |
| NULL_004 | Replay attack prevention via nonce enforcement | ✅ passed | 2774ms |
| NULL_005 | Epoch validation in proofs | ✅ passed | 5316ms |
| NULL_006 | Rapid sequential transactions handled | ✅ passed | 15416ms |
| NULL_007 | Concurrent transactions from multiple users without interference | ✅ passed | 2878ms |
| NULL_008 | Nullifier database persistence verified | ✅ passed | 8598ms |

### PREMIUM_GAS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| PREM_001 | Transaction with gasPrice >= 12 Gwei bypasses RLN | ✅ passed | 2560ms |
| PREM_002 | Transaction with gasPrice < 12 Gwei requires RLN | ✅ passed | 5232ms |
| PREM_003 | Exactly threshold (12 Gwei) bypasses RLN | ✅ passed | 2050ms |
| PREM_004 | Premium gas works even without Karma registration | ✅ passed | 7063ms |
| PREM_005 | Premium gas transaction from unfunded wallet fails | ✅ passed | 246ms |
| PREM_006 | Gas estimate shows premium multiplier for denied users | ✅ passed | 40853ms |

### RLN_PROOF

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| RLN_001 | Valid RLN proof is accepted | ✅ passed | 3599ms |
| RLN_002 | Unregistered user gets no proof generated | ✅ passed | 7079ms |
| RLN_003 | Transaction with garbage data still needs valid proof | ✅ passed | 6375ms |
| RLN_004 | Proof arrives before transaction (async handling) | ✅ passed | 5777ms |
| RLN_005 | Transaction times out fast without proof | ✅ passed | 5369ms |
| RLN_006 | Multiple sequential proofs are processed | ✅ passed | 13182ms |
| RLN_007 | gRPC stream resilience maintained across transactions | ✅ passed | 8489ms |
| RLN_008 | Proof rejection events are logged | ✅ passed | 11422ms |
| RLN_009 | Zero-value transactions require proof | ✅ passed | 10494ms |
| RLN_010 | Self-transfer with zero gas requires proof | ✅ passed | 8488ms |
