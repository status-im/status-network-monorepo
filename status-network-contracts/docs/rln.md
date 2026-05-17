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
enables users to register their identities, and allows any account that possesses the private key of a misbehaving user
to slash them through a commit-reveal scheme that prevents front-running attacks.

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
  - Callers commit to a slash operation with a cryptographic hash.
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

Without a commit-reveal scheme, anyone could observe an on-chain transaction that commits to slashing an account and
front-run it with their own slash transaction before the original slash executes. The commit-reveal scheme prevents this
by:

1. Separating the decision to slash (commit) from the execution (reveal)
2. Enforcing a time delay between commitment and revelation
3. Ensuring all slashes on an account are processed in order

### Step 1: Commit Phase

When a caller initiates a slash operation, they call `slashCommit()`:

1. **Hash Generation**: The caller creates a keccak256 hash of the private key and reward recipient address.
2. **Commitment Recording**: The contract stores this hash along with the timestamp in a mapping.
3. **Queue Management**: If this is not the first slash commit on this account, the reveal start time is scheduled for
   after the current reveal window closes.

The hash ensures the caller commits to specific values without revealing them until the reveal phase.

### Step 2: Reveal Phase

After the reveal window has passed, the caller calls `slashReveal()` to execute the slash:

1. **Data Provision**: The caller provides the actual private key and reward recipient address.
2. **Hash Verification**: The contract verifies that the hash of these values matches the previously committed hash.
3. **Window Validation**: The contract checks that the current time is within or after the reveal window for this
   commit.
4. **Slash Execution**: If all validations pass, the Karma `slash()` function is called to execute the actual penalty.
   The RLN contract passes `msg.sender` (the human initiator) explicitly as the `slasher` argument so that any
   [configured tier requirement](./karma.md#slash-tier-requirement) is evaluated against the person who committed the
   slash, not against the RLN contract itself.

### Reveal Window Timing

The reveal window ensures orderly processing:

- **First Commit**: The reveal window starts immediately.
- **Subsequent Commits**: Each new commit on the same account is scheduled to start after the previous window ends.
- **Window Duration**: Configurable by the admin (default 1 hour).
- **Window Range**: Minimum 1 second, maximum 1 day.

This queuing mechanism ensures that even if multiple slashes are committed for the same account, they are processed
sequentially in the order they were committed.

### Access Control

Any account that possesses the private key of a registered member can initiate slashing — no special role is required.
Access is naturally restricted by the information needed to slash: the private key itself. Only accounts that have
observed a misbehaving member's private key (e.g., from a double-spend proof) can produce the valid `privateKey` input.

### Configurable Parameters

- **Reveal Window Duration**: The amount of time that must pass between a commit and a reveal.

### Security considerations

- Supports multiple roles: DEFAULT_ADMIN_ROLE and REGISTER_ROLE.
- Ensures only authorized accounts can register identities.
- Slashing is permissionless but gated by possession of the member's private key.
