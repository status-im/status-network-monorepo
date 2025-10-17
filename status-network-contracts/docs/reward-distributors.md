# Reward Distributors

## Overview

Reward distributors are contracts that integrate with the Karma token system to enable flexible and efficient reward
distribution mechanisms. They serve as intermediaries between the Karma contract and various reward programs, allowing
different applications and systems to distribute Karma rewards using their own custom logic while maintaining
compatibility with the broader Karma ecosystem.

Multiple reward distributor instances exist simultaneously, each implementing different reward strategies tailored to
specific use cases and applications.

## Purpose and Design Goals

The primary purpose of reward distributors is to enable **dynamic Karma accrual** without requiring constant token
transfers. Traditional reward systems require token transfers every time a user's reward balance changes, which:

- Consumes significant gas for frequent reward updates
- Creates scalability challenges for systems with many participants
- Limits the granularity of reward calculations (e.g., per-second accrual becomes impractical)

Reward distributors solve this by tracking "virtual" Karma balances that accrue continuously based on each distributor's
logic. These virtual rewards only become actual tokens when explicitly redeemed. gas costs and enabling real-time reward
calculations.

## Virtual vs Actual Karma

We refer to "actual" Karma as the ERC-20 tokens that exist in the Karma contract and can be transferred, voted with etc.
"Virtual" Karma refers to reward balances tracked by distributors that represent earned rewards. They are fully backed
by actual Karma and can be converted to actual tokens any time. Reward distributors implement their own logic for
determining how virtual rewards accrue to accounts (staking, activity, time-based).

The [staking system](system-overview.md) is one such reward distributor, distributing Karma based on staked SNT and
Multiplier Points (MP).

### Redeeming rewards

By redeeming rewards users convert their virtual Karma to actual Karma. This is necessary if users want to use their
rewards for voting or transfers.

A critical design requirement is that **redeeming rewards must never revert**. This is essential because the Karma
contract's slashing mechanism calls `redeemRewards()` on all registered distributors before burning tokens from a
slashed account.

If a distributor's `redeemRewards()` function reverts:

- The entire slash transaction would fail
- Malicious actors could exploit this to make themselves unslashable
- The slashing mechanism would be effectively disabled (DoS attack)

Therefore, all reward distributor implementations must ensure their `redeemRewards()` function handles all edge cases
gracefully and never reverts, even when:

- The account has no rewards to redeem (should return 0)
- Internal state is inconsistent (should handle gracefully)
- External calls fail (should not propagate reverts)
