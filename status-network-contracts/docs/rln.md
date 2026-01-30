# RLN (Rate-Limiting Nullifier)

## Table Of Contents

- [Overview](#overview)
- [Features](#features)
- [Identity Registration](#identity-registration)
  - [Registration Process](#registration-process)
  - [Identity Commitment Generation](#identity-commitment-generation)
  - [Registry Capacity](#registry-capacity)
- [Commit-Reveal Slashing Mechanism](#commit-reveal-slashing-mechanism)
  - [Why Commit-Reveal?](#why-commit-reveal)
  - [Step 1: Commit Phase](#step-1-commit-phase)
  - [Step 2: Reveal Phase](#step-2-reveal-phase)
  - [Reveal Window Timing](#reveal-window-timing)
  - [Direct Slash (Emergency)](#direct-slash-emergency)
  - [Access Control](#access-control)

## Overview

The RLN (Rate-Limiting Nullifier) contract is a privacy-preserving identity registry that manages a set of identity
commitments for the protocol participants. This is used to prevent spam on the gas-less Status Network protocol. It
enables users to register their identities, and allows authorized accounts to slash misbehaving users through a Status
Network protocol. commit-reveal scheme that prevents front-running attacks.

## Features

- **Identity Registry:**

  - Maintains a registry of identity commitments.
  - Registrations are sequential and tracked with unique indices.
  - Prevents duplicate registrations of the same identity commitment.
  - Supports privacy by storing only cryptographic commitments, not actual identities.

- **Cryptographic Foundation:**

  - Uses the Poseidon hash function to derive identity commitments from private keys.

- **Commit-Reveal Slashing:**

  - Two-step slashing mechanism prevents front-running attacks.
  - Slashers commit to a slash operation with a cryptographic hash.
  - Reveals must occur after a configurable reveal window, enforcing time-lock constraints.
  - Subsequent slashes on the same account are queued and scheduled for future reveal windows.

- **Reveal Window Management:**

  - Configurable reveal window duration (default 1 hour, range 1 second to 1 day).
  - Multiple commits for the same account are automatically queued.
  - System tracks when the last reveal window started for each account.

- **Karma Token Integration:**

  - Integrates with the Karma contract to enforce economic penalties for violations.
  - Slashing burns a percentage of the user's total Karma balance (both actual and virtual tokens).
  - [Slashing rewards](./karma.md#slashing) are minted to a recipient specified as parameter.

The contract can be upgraded by users with the ADMIN role.

## Identity Registration

The RLN contract maintains a registry of identity commitments that represent user identities.

### Registration Process

Accounts with REGISTER_ROLE can register new identity commitments by calling the `register` function.

### Identity Commitment Generation

Identity commitments are generated off-chain.

## Commit-Reveal Slashing Mechanism

The RLN contract implements a two-step commit-reveal scheme to prevent front-running attacks and ensure orderly
processing of slashing operations.

### Why Commit-Reveal?

Without a commit-reveal scheme, a slasher could observe an on-chain transaction that commits to slashing an account and
front-run it with their own slash transaction before the original slash executes. The commit-reveal scheme prevents this
by:

1. Separating the decision to slash (commit) from the execution (reveal)
2. Enforcing a time delay between commitment and revelation
3. Ensuring all slashes on an account are processed in order

### Step 1: Commit Phase

When a slasher initiates a slash operation, they call `slashCommit()`:

1. **Hash Generation**: The slasher creates a keccak256 hash of the private key and reward recipient address.
2. **Commitment Recording**: The contract stores this hash along with the timestamp in a mapping.
3. **Queue Management**: If this is not the first slash commit on this account, the reveal start time is scheduled for
   after the current reveal window closes.

The hash ensures the slasher commits to specific values without revealing them until the reveal phase.

### Step 2: Reveal Phase

After the reveal window has passed, the slasher calls `slashReveal()` to execute the slash:

1. **Data Provision**: The slasher provides the actual private key and reward recipient address.
2. **Hash Verification**: The contract verifies that the hash of these values matches the previously committed hash.
3. **Window Validation**: The contract checks that the current time is within or after the reveal window for this
   commit.
4. **Slash Execution**: If all validations pass, the `slash()` function is called to execute the actual penalty.

### Reveal Window Timing

The reveal window ensures orderly processing:

- **First Commit**: The reveal window starts immediately.
- **Subsequent Commits**: Each new commit on the same account is scheduled to start after the previous window ends.
- **Window Duration**: Configurable by the admin (default 1 hour).
- **Window Range**: Minimum 1 second, maximum 1 day.

This queuing mechanism ensures that even if multiple slashes are committed for the same account, they are processed
sequentially in the order they were committed.

### Direct Slash (Emergency)

The contract also supports a direct `slash()` function for emergency situations or when the slasher wishes to bypass the
commit-reveal mechanism. However, calling `slash` directly is vulrnerable to front-running attacks.

### Access Control

Only accounts with the `SLASHER_ROLE` or the admin can execute slashing operations.

### Configurable Parameters

- **Reveal Window Duration**: The amount of time that must pass between a commit and a reveal.

### Security considerations

- Supports multiple roles: DEFAULT_ADMIN_ROLE, REGISTER_ROLE, and SLASHER_ROLE.
- Ensures only authorized accounts can register identities and execute slashes.
