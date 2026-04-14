# Karma Airdrop

## Table Of Contents

- [Overview](#overview)
- [Features](#features)
- [How It Works](#how-it-works)
  - [Initial Setup](#initial-setup)
  - [Updating the Merkle Root](#updating-the-merkle-root)
- [Epoch System](#epoch-system)
- [Delegation and Voting Power](#delegation-and-voting-power)

## Overview

The Karma Airdrop contract is a Merkle tree-based distribution mechanism that enables efficient token distribution to
eligible users. It allows application teams within the Status Network to distribute [Karma tokens](karma.md) to their
users and those they onboard through a trustless, gas-efficient claiming process.

Each airdrop instance is managed independently by an application team, which can update the Merkle root to include new
eligible claimants as their user base grows and evolves.

**Multiple instances** of this contract can exist simultaneously, each managed by different application teams. Each
instance:

- Operates independently with its own Merkle root
- Is controlled by a specific application team (the owner)
- Can be configured to allow or disallow Merkle root updates
- Maintains its own claim tracking across multiple epochs
- Holds its own allocation of Karma tokens for distribution

## Features

- **Merkle Tree-Based Distribution:**
  - Uses cryptographic Merkle proofs to verify claim eligibility.
  - Highly gas-efficient - only the claimer pays for verification.
  - Supports large-scale distributions without excessive on-chain data.

- **Flexible Merkle Root Updates:**
  - Configurable at deployment: allow updates or single-use only.
  - When updates are allowed, teams can refresh the Merkle root to add new claimants.
  - Epoch-based claim tracking prevents double-claiming across root updates.
  - Updates require contract to be paused to prevent front-running.

- **Automatic Delegation:**
  - Supports optional delegation of voting power on first claim.
  - Can specify a default delegatee address at deployment.
  - Uses EIP-712 signatures for gasless delegation.
  - Only delegates if the claimer has no prior Karma balance.

- **Pausable Operations:**
  - Owner can pause claiming to prepare for Merkle root updates.
  - Prevents front-running when transitioning between epochs.
  - Provides safety mechanism for emergency situations.

- **Bitmap Claim Tracking:**
  - Efficient storage using bitmap to track claimed indices.
  - Separate bitmaps per epoch for independent claim tracking.
  - Minimal gas overhead for checking claim status.

## How It Works

### Initial Setup

When an application team deploys an airdrop instance:

1. **Deployment**: Status deploys the KarmaAirdrop contract instances with:
   - The Karma token address
   - The owner address (typically the app team's multisig)
   - Whether Merkle root updates are allowed
   - Optional default delegatee address for voting power

2. **Funding**: A Status treasury account transfers Karma tokens to the airdrop contract to fund the distribution.

3. **Merkle Root Setting**: The owner sets the initial Merkle root containing all eligible claims.

### Updating the Merkle Root

For airdrop instances that allow updates, teams can refresh the eligible claimants:

1. **Pause**: The owner pauses the contract to prevent claims during the transition.

2. **Update**: The owner calls `setMerkleRoot()` with the new Merkle root:
   - The epoch counter increments
   - A new claim bitmap is created for the new epoch
   - Previous epoch claims remain tracked but separate

3. **Unpause**: The owner unpauses the contract, enabling claims under the new Merkle root.

This process allows users who claimed in previous epochs to claim again in the new epoch if they're included in the
updated tree.

## Epoch System

The epoch system is crucial for managing multiple Merkle root updates:

- **Epoch Counter**: Starts at 0 and increments with each Merkle root update (after the first).

- **Independent Tracking**: Each epoch maintains its own claim bitmap, preventing double-claims within an epoch but
  allowing the same user to claim in different epochs.

## Delegation and Voting Power

The airdrop contract includes automatic delegation features. If a user has zero Karma balance before claiming, their
voting power can be automatically delegated to a default delegatee, which in practice is going to be the account of the
app team.

## Security Considerations

### Front-Running Protection

- Merkle root updates require the contract to be paused
- Prevents users from racing to claim old allocations before a root update
- Ensures clean epoch transitions

### Proof Validation

- All claims require valid Merkle proofs
- Leaf nodes include index, account, and amount to prevent proof reuse
- Invalid proofs are rejected before any state changes

### Access Control

- Only the owner can update Merkle roots
- Only the owner can pause/unpause
- Uses OpenZeppelin's Ownable2Step for safe ownership transfers
