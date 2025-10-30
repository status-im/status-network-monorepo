# Simple Reward Distributor

## Overview

The `SimpleKarmaDistributor` is a [reward distributor](reward-distributors.md) that enables off-chain services to
distribute Karma tokens based on custom criteria. Unlike the [StakeManager](staking-reward-distributor/overview.md)
which calculates rewards on-chain based on staking positions, this distributor allows operators to mint virtual Karma
rewards for accounts based on conditions verified off-chain.

## Use Cases

This distributor is designed for reward programs where eligibility or reward amounts are determined outside the
blockchain:

- **Proof of Humanity**: Reward verified human users
- **Community Contributions**: Reward forum activity, content creation, or other platform participation
- **Campaign Rewards**: Distribute rewards for completing specific tasks or achievements
- **Off-chain Integrations**: Reward actions from external systems (social media, other chains, etc.)

## How It Works

### Basic Flow

1. **Supply**: The Karma contract supplies the distributor with Karma tokens
2. **Mint**: Operators mint virtual Karma for accounts based on off-chain verification
3. **Redeem**: Users redeem their virtual Karma to receive actual tokens

```
Off-chain Service
      |
      | Verifies conditions
      | (e.g., proof of humanity)
      ↓
  Operator mints virtual Karma
      |
      ↓
Virtual Karma → User redeems → Actual Karma Tokens
```

### Virtual Balance System

Like all reward distributors, this contract tracks "virtual" Karma balances:

- **Virtual Karma**: Tracked internally, fully backed by actual Karma held by the distributor
- **Actual Karma**: ERC-20 tokens received after redemption
- Virtual balances count towards voting power and total supply in the Karma contract

## Supply Management

The contract maintains three supply counters:

- **Available Supply**: Karma that can be minted as virtual rewards
- **Minted Supply**: Virtual Karma that has been allocated but not yet redeemed
- **Redeemed**: Karma that has been converted to actual tokens and transferred to users

Operators cannot mint more than the available supply, ensuring virtual rewards are always backed 1:1 by actual Karma
held by the distributor.
