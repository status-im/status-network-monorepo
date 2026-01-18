# RLN Gasless E2E Test Report

**Generated:** 2026-01-18T16:48:13.587Z

## Summary

| Metric | Value |
|--------|-------|
| Total Scenarios | 65 |
| Passed | 55 |
| Failed | 10 |
| Skipped | 0 |
| Not Run | 0 |
| Pass Rate | 84.6% |
| Duration | 1545.8s |

## Category Breakdown

| Category | Total | Passed | Failed | Skipped | Pass Rate |
|----------|-------|--------|--------|---------|-----------|
| DENY_LIST | 9 | 7 | 2 | 0 | 77.8% |
| EDGE_CASE | 5 | 5 | 0 | 0 | 100.0% |
| ERROR_HANDLING | 3 | 3 | 0 | 0 | 100.0% |
| GASLESS | 10 | 7 | 3 | 0 | 70.0% |
| INTEGRATION | 6 | 4 | 2 | 0 | 66.7% |
| KARMA | 8 | 6 | 2 | 0 | 75.0% |
| NULLIFIER | 8 | 7 | 1 | 0 | 87.5% |
| PREMIUM_GAS | 6 | 6 | 0 | 0 | 100.0% |
| RLN_PROOF | 10 | 10 | 0 | 0 | 100.0% |

## ❌ Failed Scenarios

### DENY_007: Multiple users can be on deny list simultaneously

- **Category:** DENY_LIST
- **Test File:** deny-list-and-premium-gas
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

### DENY_009: Concurrent deny list additions are safe

- **Category:** DENY_LIST
- **Test File:** deny-list-and-premium-gas
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

### GAS_002: Entry tier user gets rejected on 3rd transaction (quota exceeded)

- **Category:** GASLESS
- **Test File:** gasless-transactions
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

### GAS_003: User exceeding quota is added to deny list

- **Category:** GASLESS
- **Test File:** gasless-transactions
- **Error:** `Error: Expected transaction to fail but it succeeded`

### GAS_007: Concurrent transactions maintain user quota isolation

- **Category:** GASLESS
- **Test File:** gasless-transactions
- **Error:** `Error: Expected transaction to fail but it succeeded`

### INT_002: Multiple users with different tiers operating simultaneously

- **Category:** INTEGRATION
- **Test File:** integration-and-errors
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

### INT_006: High volume user quota tracking

- **Category:** INTEGRATION
- **Test File:** integration-and-errors
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

### KARMA_004: Additional Karma mint increases available quota

- **Category:** KARMA
- **Test File:** karma-tier-system
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

### KARMA_006: Tier boundary at exact threshold is handled correctly

- **Category:** KARMA
- **Test File:** karma-tier-system
- **Error:** `Error: thrown: "Exceeded timeout of 20000 ms for a test.`

### NULL_006: Rapid sequential transactions handled

- **Category:** NULLIFIER
- **Test File:** nullifier-tracking
- **Error:** `Error: wait for transaction timeout (code=TIMEOUT, version=6.13.7)`

## Scenario Details

### DENY_LIST

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| DENY_001 | User exceeding quota is added to deny list | ✅ passed | 18071ms |
| DENY_002 | Denied user cannot send gasless transactions | ✅ passed | 64014ms |
| DENY_003 | Premium gas clears deny status (recovery path) | ✅ passed | 35291ms |
| DENY_004 | Denied user can send premium gas transaction | ✅ passed | 38432ms |
| DENY_005 | Premium gas payment removes user from deny list | ✅ passed | 36724ms |
| DENY_006 | After premium recovery and new epoch, user can send gasless again | ✅ passed | 21826ms |
| DENY_007 | Multiple users can be on deny list simultaneously | ❌ failed | 43755ms |
| DENY_008 | Deny list state is consistent across checks | ✅ passed | 34116ms |
| DENY_009 | Concurrent deny list additions are safe | ❌ failed | 6721ms |

### EDGE_CASE

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| EDGE_001 | Self-transfer gasless transaction allowed | ✅ passed | 2322ms |
| EDGE_002 | Empty data gasless transaction allowed | ✅ passed | 4661ms |
| EDGE_003 | Minimum gas limit transaction succeeds | ✅ passed | 4617ms |
| EDGE_004 | Rapid user creation and registration without conflicts | ✅ passed | 12469ms |
| EDGE_005 | Transaction to contract address allowed | ✅ passed | 1875ms |

### ERROR_HANDLING

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| ERR_001 | Karma service unavailable handled gracefully | ✅ passed | 1310ms |
| ERR_002 | RLN prover unavailable timeout handling | ✅ passed | 3145ms |
| ERR_003 | Transaction with large data payload handled | ✅ passed | 1878ms |

### GASLESS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| GAS_001 | Entry tier user can send exactly 2 gasless transactions | ✅ passed | 4265ms |
| GAS_002 | Entry tier user gets rejected on 3rd transaction (quota exceeded) | ❌ failed | 7187ms |
| GAS_003 | User exceeding quota is added to deny list | ❌ failed | 10787ms |
| GAS_004 | Non-Karma user cannot send gasless transactions | ✅ passed | 3638ms |
| GAS_005 | Basic tier user can send 16 gasless transactions | ✅ passed | 36534ms |
| GAS_006 | Quota resets after epoch boundary | ✅ passed | 44571ms |
| GAS_007 | Concurrent transactions maintain user quota isolation | ❌ failed | 27034ms |
| GAS_008 | Different tiers have different quotas | ✅ passed | 43945ms |
| GAS_009 | Transaction without proof times out fast | ✅ passed | 3220ms |
| GAS_010 | Nonce management for sequential gasless transactions | ✅ passed | 6348ms |

### INTEGRATION

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| INT_001 | Complete lifecycle: Register → Gasless → Exhaust → Deny → Premium → Recovery | ✅ passed | 99806ms |
| INT_002 | Multiple users with different tiers operating simultaneously | ❌ failed | 7922ms |
| INT_003 | Rapid sequential transactions handled correctly | ✅ passed | 10229ms |
| INT_004 | Epoch transition with active users handled gracefully | ✅ passed | 10814ms |
| INT_005 | Concurrent transactions don't corrupt state | ✅ passed | 11051ms |
| INT_006 | High volume user quota tracking | ❌ failed | 13269ms |

### KARMA

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| KARMA_001 | Minting 1 Karma assigns Entry tier | ✅ passed | 8878ms |
| KARMA_002 | Minting 50 Karma assigns Basic tier | ✅ passed | 15286ms |
| KARMA_003 | Karma mint triggers automatic RLN registration | ✅ passed | 6385ms |
| KARMA_004 | Additional Karma mint increases available quota | ❌ failed | 16796ms |
| KARMA_005 | All tier levels have correct quota values | ✅ passed | 5ms |
| KARMA_006 | Tier boundary at exact threshold is handled correctly | ❌ failed | 20001ms |
| KARMA_007 | Zero Karma user cannot use gasless | ✅ passed | 3657ms |
| KARMA_008 | Identity commitment is unique per user | ✅ passed | 10214ms |

### NULLIFIER

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| NULL_001 | Each transaction gets unique nullifier | ✅ passed | 5397ms |
| NULL_002 | Same user can transact across different epochs | ✅ passed | 40683ms |
| NULL_003 | Quota exhaustion triggers security event logging | ✅ passed | 16867ms |
| NULL_004 | Replay attack prevention via nonce enforcement | ✅ passed | 1661ms |
| NULL_005 | Epoch validation in proofs | ✅ passed | 1815ms |
| NULL_006 | Rapid sequential transactions handled | ❌ failed | 7238ms |
| NULL_007 | Concurrent transactions from multiple users without interference | ✅ passed | 1446ms |
| NULL_008 | Nullifier database persistence verified | ✅ passed | 4786ms |

### PREMIUM_GAS

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| PREM_001 | Transaction with gasPrice >= 12 Gwei bypasses RLN | ✅ passed | 1554ms |
| PREM_002 | Transaction with gasPrice < 12 Gwei requires RLN | ✅ passed | 3155ms |
| PREM_003 | Exactly threshold (12 Gwei) bypasses RLN | ✅ passed | 1767ms |
| PREM_004 | Premium gas works even without Karma registration | ✅ passed | 5361ms |
| PREM_005 | Premium gas transaction from unfunded wallet fails | ✅ passed | 160ms |
| PREM_006 | Gas estimate shows premium multiplier for denied users | ✅ passed | 36556ms |

### RLN_PROOF

| ID | Description | Status | Duration |
|----|-------------|--------|----------|
| RLN_001 | Valid RLN proof is accepted | ✅ passed | 2289ms |
| RLN_002 | Unregistered user gets no proof generated | ✅ passed | 4348ms |
| RLN_003 | Transaction with garbage data still needs valid proof | ✅ passed | 3160ms |
| RLN_004 | Proof arrives before transaction (async handling) | ✅ passed | 1782ms |
| RLN_005 | Transaction times out fast without proof | ✅ passed | 3192ms |
| RLN_006 | Multiple sequential proofs are processed | ✅ passed | 8241ms |
| RLN_007 | gRPC stream resilience maintained across transactions | ✅ passed | 4052ms |
| RLN_008 | Proof rejection events are logged | ✅ passed | 3546ms |
| RLN_009 | Zero-value transactions require proof | ✅ passed | 4552ms |
| RLN_010 | Self-transfer with zero gas requires proof | ✅ passed | 4465ms |
