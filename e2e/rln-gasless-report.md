# RLN Gasless E2E Test Report

**Generated:** 2025-12-17T21:11:50.899Z

## Summary

| Metric | Value |
|--------|-------|
| Total Scenarios | 65 |
| Passed | 65 |
| Failed | 0 |
| Skipped | 0 |
| Not Run | 0 |
| Pass Rate | 100.0% |
| Duration | 1298.0s |

## Category Breakdown

| Category | Total | Passed | Failed | Skipped | Pass Rate |
|----------|-------|--------|--------|---------|-----------|
| DENY_LIST | 9 | 9 | 0 | 0 | 100.0% |
| EDGE_CASE | 5 | 5 | 0 | 0 | 100.0% |
| ERROR_HANDLING | 3 | 3 | 0 | 0 | 100.0% |
| GASLESS | 10 | 10 | 0 | 0 | 100.0% |
| INTEGRATION | 6 | 6 | 0 | 0 | 100.0% |
| KARMA | 8 | 8 | 0 | 0 | 100.0% |
| NULLIFIER | 8 | 8 | 0 | 0 | 100.0% |
| PREMIUM_GAS | 6 | 6 | 0 | 0 | 100.0% |
| RLN_PROOF | 10 | 10 | 0 | 0 | 100.0% |

## Scenario Details

### DENY_LIST

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| DENY_001 | User exceeding quota is added to deny list | ✅ passed | 16518ms |
| DENY_002 | Denied user cannot send gasless transactions | ✅ passed | 62787ms |
| DENY_003 | Premium gas clears deny status (recovery path) | ✅ passed | 34825ms |
| DENY_004 | Denied user can send premium gas transaction | ✅ passed | 33808ms |
| DENY_005 | Premium gas payment removes user from deny list | ✅ passed | 35538ms |
| DENY_006 | After premium recovery and new epoch, user can send gasless again | ✅ passed | 23632ms |
| DENY_007 | Multiple users can be on deny list simultaneously | ✅ passed | 101010ms |
| DENY_008 | Deny list state is consistent across checks | ✅ passed | 34019ms |
| DENY_009 | Concurrent deny list additions are safe | ✅ passed | 38682ms |

### EDGE_CASE

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| EDGE_001 | Self-transfer gasless transaction allowed | ✅ passed | 842ms |
| EDGE_002 | Empty data gasless transaction allowed | ✅ passed | 1015ms |
| EDGE_003 | Minimum gas limit transaction succeeds | ✅ passed | 980ms |
| EDGE_004 | Rapid user creation and registration without conflicts | ✅ passed | 9823ms |
| EDGE_005 | Transaction to contract address allowed | ✅ passed | 1253ms |

### ERROR_HANDLING

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| ERR_001 | Karma service unavailable handled gracefully | ✅ passed | 982ms |
| ERR_002 | RLN prover unavailable timeout handling | ✅ passed | 3157ms |
| ERR_003 | Transaction with large data payload handled | ✅ passed | 1055ms |

### GASLESS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| GAS_001 | Entry tier user can send exactly 2 gasless transactions | ✅ passed | 2539ms |
| GAS_002 | Entry tier user gets rejected on 3rd transaction (quota exceeded) | ✅ passed | 33499ms |
| GAS_003 | User exceeding quota is added to deny list | ✅ passed | 34123ms |
| GAS_004 | Non-Karma user cannot send gasless transactions | ✅ passed | 4006ms |
| GAS_005 | Basic tier user can send 16 gasless transactions | ✅ passed | 18714ms |
| GAS_006 | Quota resets after epoch boundary | ✅ passed | 23220ms |
| GAS_007 | Concurrent transactions maintain user quota isolation | ✅ passed | 33363ms |
| GAS_008 | Different tiers have different quotas | ✅ passed | 39601ms |
| GAS_009 | Transaction without proof times out fast | ✅ passed | 3149ms |
| GAS_010 | Nonce management for sequential gasless transactions | ✅ passed | 2801ms |

### INTEGRATION

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| INT_001 | Complete lifecycle: Register → Gasless → Exhaust → Deny → Premium → Recovery | ✅ passed | 79048ms |
| INT_002 | Multiple users with different tiers operating simultaneously | ✅ passed | 8754ms |
| INT_003 | Rapid sequential transactions handled correctly | ✅ passed | 5190ms |
| INT_004 | Epoch transition with active users handled gracefully | ✅ passed | 15249ms |
| INT_005 | Concurrent transactions don't corrupt state | ✅ passed | 6632ms |
| INT_006 | High volume user quota tracking | ✅ passed | 11964ms |

### KARMA

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| KARMA_001 | Minting 1 Karma assigns Entry tier | ✅ passed | 7938ms |
| KARMA_002 | Minting 50 Karma assigns Basic tier | ✅ passed | 10849ms |
| KARMA_003 | Karma mint triggers automatic RLN registration | ✅ passed | 8371ms |
| KARMA_004 | Additional Karma mint increases available quota | ✅ passed | 13707ms |
| KARMA_005 | All tier levels have correct quota values | ✅ passed | 8ms |
| KARMA_006 | Tier boundary at exact threshold is handled correctly | ✅ passed | 17169ms |
| KARMA_007 | Zero Karma user cannot use gasless | ✅ passed | 5371ms |
| KARMA_008 | Identity commitment is unique per user | ✅ passed | 8458ms |

### NULLIFIER

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| NULL_001 | Each transaction gets unique nullifier | ✅ passed | 2396ms |
| NULL_002 | Same user can transact across different epochs | ✅ passed | 9760ms |
| NULL_003 | Quota exhaustion triggers security event logging | ✅ passed | 12490ms |
| NULL_004 | Replay attack prevention via nonce enforcement | ✅ passed | 1700ms |
| NULL_005 | Epoch validation in proofs | ✅ passed | 940ms |
| NULL_006 | Rapid sequential transactions handled | ✅ passed | 6793ms |
| NULL_007 | Concurrent transactions from multiple users without interference | ✅ passed | 1165ms |
| NULL_008 | Nullifier database persistence verified | ✅ passed | 2228ms |

### PREMIUM_GAS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| PREM_001 | Transaction with gasPrice >= 10 Gwei bypasses RLN | ✅ passed | 1646ms |
| PREM_002 | Transaction with gasPrice < 10 Gwei requires RLN | ✅ passed | 3322ms |
| PREM_003 | Exactly threshold (10 Gwei) bypasses RLN | ✅ passed | 1209ms |
| PREM_004 | Premium gas works even without Karma registration | ✅ passed | 2940ms |
| PREM_005 | Premium gas transaction from unfunded wallet fails | ✅ passed | 168ms |
| PREM_006 | Gas estimate shows premium multiplier for denied users | ✅ passed | 33935ms |

### RLN_PROOF

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| RLN_001 | Valid RLN proof is accepted | ✅ passed | 1536ms |
| RLN_002 | Unregistered user gets no proof generated | ✅ passed | 4395ms |
| RLN_003 | Transaction with garbage data still needs valid proof | ✅ passed | 3319ms |
| RLN_004 | Proof arrives before transaction (async handling) | ✅ passed | 1048ms |
| RLN_005 | Transaction times out fast without proof | ✅ passed | 3149ms |
| RLN_006 | Multiple sequential proofs are processed | ✅ passed | 2676ms |
| RLN_007 | gRPC stream resilience maintained across transactions | ✅ passed | 4115ms |
| RLN_008 | Proof rejection events are logged | ✅ passed | 3438ms |
| RLN_009 | Zero-value transactions require proof | ✅ passed | 4748ms |
| RLN_010 | Self-transfer with zero gas requires proof | ✅ passed | 3843ms |
