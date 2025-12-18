# Karma

## Table Of Contents

- [Overview](#overview)
- [Features](#features)
- [Setting Rewards on Reward Distributors](#setting-rewards-on-reward-distributors)
  - [Reward Distribution](#reward-distribution)
  - [How Setting Rewards Works](#how-setting-rewards-works)
  - [Virtual vs Actual Karma](#virtual-vs-actual-karma)
  - [Why This Design?](#why-this-design)
  - [Balance Calculation Example](#balance-calculation-example)
- [Removing Reward Distributors](#removing-reward-distributors)
  - [What Happens During Removal](#what-happens-during-removal)
  - [Implications](#implications)
- [Slashing](#slashing)
- [Supply and Balance Calculation](#supply-and-balance-calculation)
- [Sources of Karma Tokens](#sources-of-karma-tokens)

## Overview

The Karma contract is an ERC-20 token implementation with a modified supply mechanism that incorporates external
[reward distributors](reward-distributors.md). Karma tokens are not transferrable, but they can be used as voting power
in the Status Network.

## Features

- **Minting:**
  - The contract owner (admin) or operators can mint tokens directly to accounts.
  - Direct minting creates actual Karma tokens that exist on-chain.
- **Voting Capabilities:**
  - Implements ERC20Votes for governance participation.
  - Only actual Karma tokens can be used as voting power (virtual rewards from distributors cannot be used for voting).
  - Supports delegation and checkpoint-based voting.
- **Reward Distributors Integration:**
  - Tracks balances and supplies from external reward distributors.
  - Allows addition and removal of reward distributors by the owner.
  - Supports multiple reward distributors simultaneously.
- **Non-Transferrable Tokens:**
  - Transfers, approvals, and allowances are disabled by default.
  - Users can only receive balances from minting or reward distributions.
  - This ensures Karma remains bound to the earning account.
- **Transfer Whitelisting:**
  - The admin can whitelist specific accounts to enable transfers.
  - Whitelisted accounts can transfer Karma tokens to other addresses.
  - Typically, reward distributors are whitelisted to allow them to transfer tokens as part of their reward mechanisms.
  - Transfers to reward distributors are always prohibited, even for whitelisted accounts.
- **Supply Calculation:**
  - The total supply is the sum of the internal supply and the external supplies that have been distributed so far.
  - Undistributed rewards held by distributors are not counted in the total supply.

## Setting Rewards on Reward Distributors

One of the core functionalities of the Karma contract is its ability to work with external
[reward distributors](reward-distributors.md). This section explains how rewards are set and how the system handles
virtual versus actual Karma tokens.

### Reward Distribution

The Karma contract integrates with external contracts that implement the `IRewardDistributor` interface. These
distributors manage their own reward mechanisms (such as staking rewards) and track "virtual" Karma balances for
participants.

When rewards are set on a distributor, the Karma contract performs a crucial operation: it mints actual Karma tokens to
the distributor contract equal to the reward amount being configured. However, these tokens serve as backing for virtual
rewards rather than being immediately distributed.

### How Setting Rewards Works

When an admin or operator sets rewards for a distributor:

1. **Validation**: The contract verifies that the specified address is a registered reward distributor.
2. **Minting**: The Karma contract mints the specified amount of tokens directly to the reward distributor contract.
3. **Distribution Configuration**: The contract configures the distributor to distribute these rewards over the
   specified duration.

The key insight here is that actual Karma tokens are minted upfront to the distributor, but they serve as backing for
virtual rewards that participants will earn over time.

### Virtual vs Actual Karma

The Karma minted to the distributor represents actual, on-chain tokens. However, as participants earn rewards through
the distributor, they don't immediately receive these actual tokens. Instead:

- **Virtual Rewards**: Participants accumulate "virtual" Karma tracked by the reward distributor. These virtual rewards
  show up in their total Karma balance, but they're not actual ERC-20 tokens yet.

- **Actual Tokens**: The actual Karma tokens remain held by the distributor contract as backing for these virtual
  rewards.

- **Conversion**: Virtual rewards are converted into actual Karma tokens when:
  - A participant explicitly redeems their rewards via the distributor
  - The account is slashed (the system automatically redeems virtual rewards before slashing)

### Why This Design?

This two-tier system provides several benefits:

1. **Efficiency**: Virtual rewards can be tracked and updated without requiring token transfers for every accrual,
   saving gas costs.

2. **Flexibility**: Different reward distributors can implement their own reward mechanisms while using Karma as the
   underlying token.

### Balance Calculation Example

Consider an account that has:

- 100 actual Karma tokens (directly minted or previously redeemed)
- 50 virtual Karma from Distributor A
- 30 virtual Karma from Distributor B

The account's total Karma balance will show 180 (100 + 50 + 30), but only the 100 actual tokens can be used for voting.
The contract provides separate functions to query actual token balances versus total balances including virtual rewards.

## Removing Reward Distributors

The Karma contract allows the owner to remove reward distributors from the system. When a reward distributor is removed,
an important cleanup operation occurs to maintain the integrity of the Karma supply.

### What Happens During Removal

When a reward distributor is removed all remaining Karma tokens held by the distributor are permanently burned. This
includes any rewards that were minted to the distributor but not yet converted to virtual rewards or claimed by users.

### Implications

When planning to remove a reward distributor, operators should be aware that:

- Any Karma tokens held by the distributor that haven't been converted to virtual rewards will be permanently burned.
- Users should be given adequate notice to claim their virtual rewards before a distributor is removed.
- The removal operation is irreversible once executed.

## Slashing

The Karma contract includes a slashing mechanism that allows authorized accounts to reduce an account's Karma balance as
a penalty for certain behaviors, through the [RLN Registry](rln.md). This section explains how slashing works and its
implications.

### Overview of Slashing

Slashing is a punitive mechanism where a percentage of an account's total Karma balance is burned. The system is
designed to:

- Penalize accounts that violate protocol rules or engage in malicious behavior
- Provide incentives to those who identify and report violations
- Work with both actual tokens and virtual rewards from distributors

### How Slashing Works

When an account with the slasher role initiates a slash:

1. **Balance Calculation**: The system calculates the account's total balance, including both actual tokens and virtual
   rewards from all registered reward distributors.

2. **Virtual Reward Redemption**: Before slashing, the system automatically redeems all virtual rewards from all reward
   distributors, converting them into actual Karma tokens. This is necessary as virtual Karma cannot be burned
   otherwise.

3. **Slash Amount Calculation**: The slash amount is calculated as a percentage of the total balance (configurable by
   the admin, defaulting to 50%). There is a minimum slash amount of 1 KARMA to ensure meaningful penalties.

4. **Reward Calculation**: A portion of the slashed amount is allocated as a reward to the reporter who identified the
   violation (configurable by the admin, defaulting to 10% of the slashed amount).

5. **Burning**: The entire slashed amount is burned from the account, reducing the total supply.

6. **Reward Minting**: If a reward recipient is specified, they receive newly minted Karma tokens equal to the reward
   amount.

### Slashing Parameters

The contract maintains two configurable parameters:

- **Slash Percentage**: The percentage of an account's balance to slash (in basis points, where 10000 = 100%). Default
  is 5000 (50%).

- **Slash Reward Percentage**: The percentage of the slashed amount to mint as a reward for the reporter (in basis
  points). Default is 1000 (10%).

Both parameters can only be modified by the admin and are capped at 100% (10000 basis points).

### Minimum Slash Amount

The contract enforces a minimum slash amount of 1 KARMA. If the calculated slash amount (percentage of balance) is less
than 1 KARMA, the system uses 1 KARMA as the slash amount instead. If the account's total balance is less than 1 KARMA,
the entire balance is slashed.

### Slashing Example

Consider an account with:

- 100 actual Karma tokens
- 50 virtual Karma from Distributor A
- 30 virtual Karma from Distributor B
- Total balance: 180 KARMA

With default settings (50% slash percentage, 10% reward percentage):

1. All virtual rewards are redeemed, giving the account 180 actual tokens
2. Slash amount calculated: 180 × 50% = 90 KARMA
3. Reward amount calculated: 90 × 10% = 9 KARMA
4. 90 KARMA is burned from the account (leaving 90 KARMA)
5. 9 KARMA is minted to the reward recipient
6. Net effect on total supply: -81 KARMA (90 burned - 9 minted)

### Access Control

Only accounts with the `SLASHER_ROLE` or the admin can execute slashing operations. This ensures that slashing is
performed by trusted parties and prevents arbitrary penalties.

## Supply and Balance Calculation

The Karma contract calculates supply and balances by aggregating both actual tokens and virtual rewards from all
registered reward distributors.

- **Total Supply**: Sum of all actual tokens in circulation plus the virtual rewards that have been distributed (but not
  yet redeemed) across all reward distributors. Tokens held by distributors that haven't been earned yet are not
  counted.

- **Account Balance**: An account's balance is the sum of their actual Karma tokens plus any virtual rewards they've
  earned from all reward distributors.

This means that as rewards are earned over time through distributors, both individual balances and the total supply
increase, even though no token transfers occur until rewards are explicitly redeemed.

## Sources of Karma Tokens

One of the sources for the generation of Karma tokens is the
[staking protocol](staking/-reward-distributor/overview.md), with more sources planned in the future.
