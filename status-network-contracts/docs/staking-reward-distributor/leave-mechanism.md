# Leave Mechanism

## Table Of Contents

- [Overview](#overview)
- [Key Characteristics](#key-characteristics)
  - [Leaving Can Happen Anytime](#leaving-can-happen-anytime)
  - [No Incentive to Leave Early](#no-incentive-to-leave-early)
  - [Withdraw Function Respects Locks](#withdraw-function-respects-locks)
  - [Retrieving Locked Funds After Leaving](#retrieving-locked-funds-after-leaving)
  - [Immediate Access Requires Unstaking](#immediate-access-requires-unstaking)
  - [Protected Against Malicious Upgrades](#protected-against-malicious-upgrades)

## Overview

The `leave()` function allows vault owners to permanently exit the staking system. This mechanism exists primarily to
enable users to leave if they disagree with system changes, such as a StakeManager upgrade.

## Key Characteristics

### Leaving Can Happen Anytime

Users can call `leave()` at any time, regardless of lock status. However, **locked funds remain locked in the vault**
until the lock period expires.

When a vault calls `leave()`:

- The vault is immediately marked as having left (`hasLeft = true`)
- Staking tokens (SNT) are only transferred if `lockUntil <= block.timestamp`

### No Incentive to Leave Early

Since locked stakes remain locked even after calling `leave()`, there's no incentive to leave before the lock period
expires. Users calling `leave()` while locked will:

- Be marked as having left the system
- **Not** receive their staking tokens until the lock expires

This ensures the lock period mechanism maintains its integrity even when users exit the system.

### Withdraw Function Respects Locks

The StakeVault's `withdraw()` function allows withdrawing excess tokens at any time, but **staked SNT remains locked**:

- **Excess tokens**: Can be withdrawn anytime
- **Staked SNT**: Locked until `lockUntil` expires, even after calling `leave()`

Users must call `leave()` before they can withdraw staked tokens (after the lock expires).

### Retrieving Locked Funds After Leaving

If `leave()` was called while funds were still locked, the staking tokens remain in the vault until the lock period
expires. Once `lockUntil <= block.timestamp`, users can retrieve their tokens by calling the `withdraw()` function.

**Workflow for locked vaults**:

1. Call `leave(_destination)` - marks vault as left, claims Karma rewards, but SNT stays locked
2. Wait until `lockUntil` expires
3. Call `withdraw(STAKING_TOKEN, _destination, _amount)` - retrieves the locked SNT

This two-step process ensures that even if users leave early, they can still access their funds after the lock period
without needing to interact with the StakeManager again.

### Immediate Access Requires Unstaking

If users want to leave the system **and** get immediate access to their stake, they must use `unstake()` instead of
`leave()`.

**Important**: `unstake()` reverts if funds are locked. Users must wait until `lockUntil` expires before they can
unstake and access their funds immediately.

### 6. Protected Against Malicious Upgrades

The `leave()` function uses a try/catch block to ensure users can always exit, even if the StakeManager has a malicious
or broken upgrade.

This protection ensures that:

- Users can always retrieve their unlocked funds
- A malicious StakeManager upgrade cannot trap user funds
- The vault remains functional even if StakeManager is non-operational
