# Gasless-Only Block Building Bug

## Issue Summary

Blocks containing **only gasless transactions** (`gasPrice=0`) fail to be mined, while blocks containing **both premium and gasless transactions** are mined successfully.

## Environment

- **Besu Sequencer** with RLN plugins enabled
- **Maru Consensus Client** with `allowEmptyBlocks=false`
- Gasless transactions via RLN (Rate Limiting Nullifier) system

## Observed Behavior

| Scenario | Result |
|----------|--------|
| Premium tx + Gasless tx together | ✅ Both mined in same block |
| Gasless tx alone | ❌ Never mined (times out) |

## Root Cause

The issue is a **race condition in Besu's block building pipeline** where the block header and body become desynchronized for gasless-only blocks.

### Evidence from Logs

**1. Transaction Selection Works Correctly:**
```
BlockTransactionSelector | Transaction selection result cumulativeGasUsed=21000, 
  selectedTransactions=0x16cd8c3da79192b35b4c5f9338c2427abcbab8673256a3e97ccaeb74e6a996ae
```

**2. But Block Validation Fails:**
```
MainnetBlockBodyValidator | Invalid block 14: gas used mismatch (expected=0, actual=21000)
MainnetBlockBodyValidator | Invalid block 14: state root mismatch (expected=0x..., actual=0x...)
MainnetBlockBodyValidator | Invalid block 14: transaction root mismatch (expected=0x56e81f171..., actual=0xe98bd2bd...)
```

**3. Fallback to Cached Empty Block:**
```
AbstractEngineGetPayload | Produced #14 | 0 tx | Timing(started at 2026-01-16T20:06:33, empty-block-created=0ms)
```
Note: `started at` timestamp is from **node startup** (14+ minutes ago), not current time.

**4. Maru Rejects Empty Block:**
```
ProposalPayloadValidator | Invalid Proposal Payload: block did not pass validation. 
  Reason Optional[BlockValidationError(message=Block is empty number=34 executionPayloadBlockNumber=14)]
```

### Flow Comparison

**Premium + Gasless (Works):**
```
1. FCU received → Start building proposals
2. Fresh payload context created (new timestamp)
3. Transactions selected synchronously
4. Header built WITH transactions (gasUsed=63000, correct roots)
5. Validation passes
6. Block mined ✅

Log evidence:
Produced #13 | 3 tx | Timing(started at 2026-01-16T20:17:55, txsSelection=21ms, blockAssembled=3ms)
```

**Gasless Only (Fails):**
```
1. FCU received → Start building proposals
2. Uses STALE payload context from node startup
3. Empty block header created (gasUsed=0, empty roots)
4. Transactions selected in background
5. Transactions added to body AFTER header finalized
6. Validation fails: header says gasUsed=0, body has gasUsed=21000
7. Fallback to cached empty block
8. Maru rejects empty block
9. Transaction stays pending forever ❌

Log evidence:
Produced #14 | 0 tx | Timing(started at 2026-01-16T20:06:33, empty-block-created=0ms)
                                        ^^^^^^^^^^^^^^^^^^^^^^^^
                                        This is NODE STARTUP TIME, not current time!
```

## Technical Details

### Header/Body Mismatch Values

| Field | Header Value (Empty Template) | Body Value (With Gasless TX) |
|-------|-------------------------------|------------------------------|
| `gasUsed` | `0` | `21000` |
| `transactionsRoot` | `0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421` (empty trie) | Actual merkle root of transactions |
| `stateRoot` | Empty state root | Actual state root after tx execution |
| `receiptsRoot` | Empty receipts root | Actual receipts root |

### Why Premium Transactions Trigger Fresh Build

When a premium transaction (`gasPrice >= premiumGasThreshold`) is in the pool:
- Something in Besu/MergeCoordinator recognizes "valuable" pending work
- A fresh payload context is created with current timestamp
- Header and body are built synchronously and stay coordinated
- Block passes validation

When only gasless transactions are pending:
- The payload building may not recognize them as "worthy" of fresh build
- Returns pre-cached empty block template from node startup
- Or there's a race where header is finalized before transactions are added

## Configuration Applied

The following configuration was added but **did NOT fix the issue**:

```yaml
# In compose-spec-l2-services-rln.yml for sequencer:
--plugin-linea-profitability-check-enabled=false
```

This disables profitability checks during block selection, and gasless transactions ARE being selected. The issue is AFTER selection, during block assembly/validation.

## Affected Components

1. **Besu Core** - `MergeCoordinator` / `AbstractEngineGetPayload` - Block building and payload caching
2. **Besu Core** - `MainnetBlockBodyValidator` - Detects the mismatch (working correctly)
3. **Not affected**: Linea plugins (transaction selection works correctly)
4. **Not affected**: Maru (correctly rejecting empty blocks)
5. **Not affected**: RLN Prover (proofs are generated correctly)

## Potential Fix Locations

1. **Besu MergeCoordinator**: Investigate why gasless-only pools don't trigger fresh payload builds
2. **Payload Caching Logic**: Check if there's a "worthiness" check based on gas fees
3. **Block Assembly Pipeline**: Ensure header is rebuilt after transaction selection completes
4. **PayloadBuildingCoordinator**: Synchronize header/body building for all transaction types

## Reproduction Steps

1. Start local RLN network with `allowEmptyBlocks=false`
2. Send a gasless transaction (`gasPrice=0`) from a funded account
3. Observe transaction stuck pending
4. Send a premium transaction (`gasPrice >= 12 gwei`)
5. Observe BOTH transactions get mined in the same block

## Workarounds

**Temporary**: Set `allowEmptyBlocks=true` in Maru - This allows the network to progress but gasless-only blocks will still be empty, and gasless transactions will only be mined when bundled with premium transactions.

**Not a fix**: This just masks the symptom; gasless-only blocks still won't contain transactions.

